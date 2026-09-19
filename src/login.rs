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

    let params = loop {
        let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        let line = String::from_utf8_lossy(&buf[..n]);
        let path = line.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("");
        if path.contains("token=") {
            reply(&mut stream, 200,
                "<!doctype html><meta charset=utf-8><body style=\"font-family:ui-monospace,monospace;background:#0b0a09;color:#efe9df;display:flex;min-height:100vh;align-items:center;justify-content:center\"><div><h2>Signed in to SnapshotDB.</h2><p style=\"color:#8f887c\">You can close this tab and return to your terminal.</p></div>");
            break parse_query(path);
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
    std::fs::write(&path, format!("{{\"user\":{:?},\"token\":{:?}}}\n", user, token))
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }

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
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(x) => {
                    out.push(x);
                    i += 3;
                }
                Err(_) => {
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
