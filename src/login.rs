//! `snapshotdb login` — browser sign-in via GitHub.
//!
//! Starts a loopback HTTP server, opens the browser to the SnapshotDB web app
//! (which holds the GitHub OAuth secret and does the exchange), and receives the
//! resulting session token back on the loopback. GitHub only ever redirects to
//! the web app's registered callback — never to this loopback — so no extra
//! OAuth redirect URI is required.

use crate::{home, random_password, R};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, Instant};
use std::os::unix::fs::OpenOptionsExt;

pub fn run(args: &[String]) -> R<()> {
    let mut web = std::env::var("SNAPSHOTDB_WEB")
        .unwrap_or_else(|_| "https://www.snapshotdb.io".to_string());
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--web" {
            web = it.next().cloned().ok_or("--web needs a URL")?;
        }
    }
    let web = web.trim_end_matches('/').to_string();

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let state = random_password()?;

    let url = format!("{web}/api/auth/cli?port={port}&state={}", pct(&state));
    eprintln!("Opening your browser to sign in with GitHub…");
    eprintln!("If it doesn't open, visit:\n  {url}\n");
    let _ = open_browser(&url);

    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(600);
    let params = loop {
        if Instant::now() >= deadline { return Err("login timed out; run snapshotdb login again".into()); }
        let (mut stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => { std::thread::sleep(Duration::from_millis(50)); continue; }
            Err(e) => return Err(e.to_string()),
        };
        stream.set_read_timeout(Some(Duration::from_secs(2))).map_err(|e| e.to_string())?;
        stream.set_write_timeout(Some(Duration::from_secs(2))).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 8192];
        let n = match stream.read(&mut buf) { Ok(n) => n, Err(_) => continue };
        let line = String::from_utf8_lossy(&buf[..n]);
        let path = line.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("");
        let params = parse_query(path);
        if line.starts_with("GET /callback?") && params.get("state") == Some(&state)
            && params.get("token").is_some_and(|token| !token.is_empty()) {
            reply(&mut stream, 200,
                "<!doctype html><meta charset=utf-8><body style=\"font-family:ui-monospace,monospace;background:#0b0a09;color:#efe9df;display:flex;min-height:100vh;align-items:center;justify-content:center\"><div><h2>Signed in to SnapshotDB.</h2><p style=\"color:#8f887c\">You can close this tab and return to your terminal.</p></div>");
            break params;
        }
        reply(&mut stream, 404, "not found");
    };

    let got = params.get("state").cloned().unwrap_or_default();
    if got != state {
        return Err("state mismatch — login aborted".into());
    }
    let token = params.get("token").cloned().ok_or("no token returned from the browser")?;
    let user = params.get("user").cloned().unwrap_or_default();

    let dir = home();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("credentials.json");
    let temporary = dir.join(format!(".credentials-{state}.tmp"));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(&temporary).map_err(|e| e.to_string())?;
    file.write_all(serde_json::json!({"user":user,"token":token}).to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, &path).map_err(|e| e.to_string())?;

    println!("Logged in as {}.", if user.is_empty() { "your GitHub account" } else { &user });
    Ok(())
}

fn reply(stream: &mut std::net::TcpStream, code: u16, body: &str) {
    let status = if code == 200 { "200 OK" } else { "404 Not Found" };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = Command::new("cmd").args(["/C", "start", "", url]).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let status = Command::new("xdg-open").arg(url).status();
    status.map(|_| ())
}

fn parse_query(path: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if let Some(qs) = path.split_once('?').map(|x| x.1) {
        for pair in qs.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                m.insert(k.to_string(), urldecode(v));
            }
        }
    }
    m
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                Some(x) => {
                    out.push(x);
                    i += 3;
                }
                None => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_query_roundtrips_and_handles_malformed_unicode() {
        let value = "user + quoted \" name / é";
        assert_eq!(urldecode(&pct(value)), value);
        assert_eq!(urldecode("%é%Q1"), "%é%Q1");
        let params = parse_query("/callback?state=abc&token=payload.signature&user=a%2Bb");
        assert_eq!(params["user"], "a+b");
        assert_eq!(params["token"], "payload.signature");
    }
}
