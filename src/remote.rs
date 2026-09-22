//! Authenticated control API. Only explicit server processes execute database operations.
use crate::{Args, R};
use serde_json::{json, Value};
use std::{
    env, fs,
    io::{Read, Write},
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};
use tiny_http::{Header, Method, Request, Response, Server};

const MAX_REQUEST: u64 = 64 * 1024;

pub fn worker_lock(nonblocking: bool) -> R<fs::File> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(crate::home().join(".worker.lock"))
        .map_err(|e| e.to_string())?;
    if nonblocking {
        fs2::FileExt::try_lock_exclusive(&file)
    } else {
        fs2::FileExt::lock_exclusive(&file)
    }
    .map_err(|_| {
        "a database operation from the previous server is still running; wait before restarting"
    })?;
    Ok(file)
}

pub fn validate_name(name: &str) -> R<()> {
    if name.is_empty()
        || name.len() > 40
        || name.starts_with(['.', '_'])
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err("branch names must be 1-40 letters, digits, '-', '_', or '.', and cannot start with '.' or '_'".into());
    }
    Ok(())
}

fn normalize(mut args: Vec<String>) -> R<Vec<String>> {
    if args.first().map(String::as_str) == Some("clone") {
        if args.len() != 3 {
            return Err("usage: snapshotdb clone <name> <connection-string>".into());
        }
        let engine = match args[2].split_once("://").map(|x| x.0) {
            Some("postgres" | "postgresql") => "postgres",
            Some("mysql") => "mysql",
            Some("mongodb" | "mongodb+srv") => "mongodb",
            _ => {
                return Err("clone needs a PostgreSQL, MySQL, or MongoDB connection string".into())
            }
        };
        args = vec![
            "sync".into(),
            engine.into(),
            args[1].clone(),
            args[2].clone(),
        ];
    }
    if !matches!(
        args.first().map(String::as_str),
        Some(
            "preflight"
                | "import"
                | "sync"
                | "create"
                | "prepare"
                | "info"
                | "url"
                | "switch"
                | "list"
                | "status"
                | "repair"
                | "reconcile"
                | "reset"
                | "settings"
                | "lock"
                | "unlock"
                | "start"
                | "stop"
                | "rm"
        )
    ) {
        return Err("unsupported server command".into());
    }
    if args.len() > 32 || args.iter().any(|x| x.contains('\0')) {
        return Err("invalid command arguments".into());
    }
    if args.iter().any(|x| x.starts_with(crate::pool::PREFIX)) {
        return Err("abwarm- is reserved for prepared branch storage".into());
    }
    Ok(args)
}

/// Some means enter the existing engine dispatcher; None means client/server work is done.
pub fn route(raw: Vec<String>) -> R<Option<Vec<String>>> {
    let first = raw.first().map(String::as_str).unwrap_or("");
    if first == "_worker" && env::var("SNAPSHOTDB_INTERNAL").as_deref() == Ok("1") {
        let args: Vec<String> = serde_json::from_reader(std::io::stdin().take(MAX_REQUEST))
            .map_err(|e| e.to_string())?;
        return Ok(Some(normalize(args)?));
    }
    if matches!(first, "_hosted_up" | "_hosted_down") && crate::hosted::enabled() && env::var("SNAPSHOTDB_INTERNAL").as_deref() == Ok("1") {
        return Ok(Some(raw));
    }
    if first == "_proxy" && env::var("SNAPSHOTDB_INTERNAL").as_deref() == Ok("1") {
        return Ok(Some(raw));
    }
    if raw.is_empty() || matches!(first, "--help" | "--version") {
        return Ok(Some(raw));
    }
    if first == "serve" {
        serve(&Args::parse(raw))?;
        return Ok(None);
    }
    client(raw)?;
    Ok(None)
}

fn allowed_origin<'a>(request: &'a Request, origins: &[String]) -> Option<&'a str> {
    request.headers().iter().find(|h| h.field.equiv("Origin"))
        .map(|h| h.value.as_str()).filter(|origin| origins.iter().any(|v| v == origin))
}

fn reply(request: Request, status: u16, body: Value, origins: &[String]) {
    let mut response = Response::from_string(if status == 204 { String::new() } else { body.to_string() })
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
        .with_header(Header::from_bytes("Vary", "Origin").unwrap());
    if let Some(origin) = allowed_origin(&request, origins) {
        response.add_header(Header::from_bytes("Access-Control-Allow-Origin", origin).unwrap());
        if status == 204 {
            response.add_header(Header::from_bytes("Access-Control-Allow-Methods", "GET, POST").unwrap());
            response.add_header(Header::from_bytes("Access-Control-Allow-Headers", "Authorization, Content-Type").unwrap());
            response.add_header(Header::from_bytes("Access-Control-Max-Age", "600").unwrap());
        }
    }
    let _ = request.respond(response);
}

fn save_job(dir: &Path, id: &str, value: &Value) -> R<()> {
    let temporary = dir.join(format!("{id}.tmp"));
    fs::write(&temporary, value.to_string()).map_err(|e| e.to_string())?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, dir.join(format!("{id}.json"))).map_err(|e| e.to_string())
}

fn run_job(dir: &Path, id: &str, args: Vec<String>, tenant: Option<&str>) -> R<()> {
    save_job(dir, id, &json!({"id": id, "state": "running"}))?;
    let mut process = Command::new(env::current_exe().map_err(|e| e.to_string())?);
    if let Some(tenant) = tenant {
        process.env("SNAPSHOTDB_HOME", dir.parent().ok_or("missing workspace")?).env("SNAPSHOTDB_TENANT", tenant);
    }
    let mut child = process.arg("_worker")
        .env("SNAPSHOTDB_INTERNAL", "1")
        .env_remove("SNAPSHOTDB_TOKEN")
        .env_remove("SNAPSHOTDB_HOSTED_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let input = serde_json::to_vec(&args).map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&input)
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    save_job(
        dir,
        id,
        &json!({"id": id, "state": "done", "exit_code": output.status.code().unwrap_or(1),
        "stdout": String::from_utf8_lossy(&output.stdout), "stderr": String::from_utf8_lossy(&output.stderr)}),
    )
}

fn token() -> R<String> {
    let token = env::var("SNAPSHOTDB_TOKEN")
        .map_err(|_| "set SNAPSHOTDB_TOKEN to the server access token")?;
    if token.len() < 32 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(
            "SNAPSHOTDB_TOKEN must contain at least 32 printable non-space ASCII characters".into(),
        );
    }
    Ok(token)
}

fn serve(args: &Args) -> R<()> {
    // Explicit opt-in: never allow arbitrary websites to read an admin API.
    let origins: Vec<String> = env::var("SNAPSHOTDB_CONSOLE_ORIGINS").unwrap_or_default()
        .split(',').map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).collect();
    for origin in &origins {
        let authority = origin.strip_prefix("https://").or_else(|| origin.strip_prefix("http://"));
        if !authority.is_some_and(|host| !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))) {
            return Err("SNAPSHOTDB_CONSOLE_ORIGINS must contain exact http(s) origins, without paths or wildcards".into());
        }
    }
    let reply = |request, status, body| reply(request, status, body, &origins);
    let stopping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(signal, stopping.clone()).map_err(|e| e.to_string())?;
    }
    let token = token()?;
    let gateway_token = env::var("SNAPSHOTDB_HOSTED_TOKEN").ok();
    if gateway_token.as_ref().is_some_and(|t| t.len() < 32 || t == &token) { return Err("SNAPSHOTDB_HOSTED_TOKEN must be a separate secret of at least 32 characters".into()); }
    let bind: SocketAddr = args
        .flags
        .get("bind")
        .map(String::as_str)
        .unwrap_or("127.0.0.1:7432")
        .parse()
        .map_err(|_| "--bind must be an IP address and port")?;
    let public = args.flags.get("public-host").ok_or(
        "serve needs --public-host: the server hostname clients use for database connections",
    )?;
    if public.is_empty()
        || !public
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
    {
        return Err("invalid --public-host".into());
    }
    let db_bind = args
        .flags
        .get("db-bind")
        .map(String::as_str)
        .unwrap_or("127.0.0.1");
    db_bind
        .parse::<std::net::Ipv4Addr>()
        .map_err(|_| "--db-bind must be an IPv4 address")?;
    env::set_var("SNAPSHOTDB_PUBLIC_HOST", public);
    env::set_var("SNAPSHOTDB_DB_BIND", db_bind);
    env::set_var("SNAPSHOTDB_INTERNAL", "1");
    fs::create_dir_all(crate::home()).map_err(|e| e.to_string())?;
    fs::set_permissions(crate::home(), fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    let storage_lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(crate::home().join(".server.lock"))
        .map_err(|e| e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&storage_lock)
        .map_err(|_| "another SnapshotDB server is using this data directory")?;
    let startup_lock = worker_lock(true)?;
    let jobs = crate::home().join(".jobs");
    fs::create_dir_all(&jobs).map_err(|e| e.to_string())?;
    // Interrupted commands are never replayed automatically: sync can modify its source.
    for entry in fs::read_dir(&jobs).map_err(|e| e.to_string())?.flatten() {
        if entry.path().extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        if let Ok(mut value) = fs::read(entry.path())
            .map_err(|e| e.to_string())
            .and_then(|v| serde_json::from_slice::<Value>(&v).map_err(|e| e.to_string()))
        {
            if matches!(value["state"].as_str(), Some("queued" | "running")) {
                value["state"] = json!("done");
                value["exit_code"] = json!(1);
                value["stderr"] = json!(
                    "server restarted during this job; inspect branch state before retrying\n"
                );
                let id = value["id"].as_str().unwrap_or("").to_owned();
                if valid_job_id(&id) {
                    save_job(&jobs, &id, &value)?;
                }
            }
        }
    }
    let server = Server::http(bind).map_err(|e| e.to_string())?;
    // One worker serializes filesystem/engine changes; polling remains responsive.
    let (tx, rx) = mpsc::sync_channel::<(std::path::PathBuf, String, Vec<String>, Option<String>)>(32);
    let worker = thread::spawn(move || {
        for (worker_dir, id, command, tenant) in rx {
            if let Err(error) = run_job(&worker_dir, &id, command, tenant.as_deref()) {
                let _ = save_job(
                    &worker_dir,
                    &id,
                    &json!({"id": id, "state": "done", "exit_code": 1, "stderr": error}),
                );
            }
        }
    });
    crate::up()?;
    hosted_lifecycle("_hosted_up");
    drop(startup_lock);
    let hosted_root = crate::home();
    let meter_stop = stopping.clone();
    let monitor = thread::spawn(move || {
        while !meter_stop.load(std::sync::atomic::Ordering::Relaxed) {
            for entry in fs::read_dir(hosted_root.join(".tenants")).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    if let Err(e) = crate::hosted::enforce(&entry.path()) { eprintln!("hosted metering: {e}"); }
                }
            }
            for _ in 0..50 {
                if meter_stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                thread::sleep(Duration::from_millis(100));
            }
        }
    });
    eprintln!(
        "SnapshotDB server listening on {bind}; database files: {}",
        crate::home().display()
    );
    while !stopping.load(std::sync::atomic::Ordering::Relaxed) {
        let Some(mut request) = server.recv_timeout(Duration::from_millis(100)).map_err(|e| e.to_string())? else { continue; };
        if request.method() == &Method::Options {
            let method = request.headers().iter().find(|h| h.field.equiv("Access-Control-Request-Method"))
                .map(|h| h.value.as_str());
            let headers_ok = request.headers().iter().filter(|h| h.field.equiv("Access-Control-Request-Headers"))
                .all(|h| h.value.as_str().split(',').all(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "authorization" | "content-type")));
            let path_ok = match method {
                Some("GET") => request.url() == "/v1/health" || request.url().starts_with("/v1/jobs/"),
                Some("POST") => request.url() == "/v1/commands",
                _ => false,
            };
            if allowed_origin(&request, &origins).is_some() && headers_ok && path_ok {
                reply(request, 204, Value::Null);
            } else {
                reply(request, 403, json!({"error": "console origin, method, or headers not allowed"}));
            }
            continue;
        }
        let authorized = request.headers().iter().any(|h| {
            h.field.equiv("Authorization")
                && same_token(
                    h.value.as_str().as_bytes(),
                    format!("Bearer {token}").as_bytes(),
                )
        });
        let gateway = gateway_token.as_ref().is_some_and(|token| request.headers().iter().any(|h|
            h.field.equiv("Authorization") && same_token(h.value.as_str().as_bytes(), format!("Bearer {token}").as_bytes())));
        if !authorized && !gateway {
            reply(request, 401, json!({"error": "unauthorized"}));
            continue;
        }
        let tenant = if gateway {
            match request.headers().iter().find(|h| h.field.equiv("X-SnapshotDB-Tenant")).map(|h| h.value.as_str().to_owned()) {
                Some(t) => Some(t),
                None => { reply(request, 400, json!({"error":"missing hosted identity"})); continue; }
            }
        } else { None };
        let workspace = match tenant.as_ref().map(|t| crate::hosted::tenant_home(&crate::home(),t)).transpose() {
            Ok(h) => h,
            Err(e) => { reply(request, 400, json!({"error":e})); continue; }
        };
        let request_jobs = workspace.as_ref().map(|h| h.join(".jobs")).unwrap_or_else(|| jobs.clone());
        // One tenant's I/O failure must not take the shared server down: answer 500 and move on.
        if let Err(e) = fs::create_dir_all(&request_jobs) { reply(request, 500, json!({"error": e.to_string()})); continue; }
        let path = request.url().to_owned();
        if let Some(home) = workspace.as_ref() {
            if request.method() == &Method::Get && path == "/v1/account" {
                match crate::hosted::usage(home) {
                    Ok(v) => reply(request,200,v), Err(e) => reply(request,500,json!({"error":e}))
                }
                continue;
            }
            if request.method() == &Method::Post && matches!(path.as_str(), "/v1/billing/entitlement" | "/v1/billing/checkout") {
                let mut body = Vec::new();
                let result = request.as_reader().take(MAX_REQUEST+1).read_to_end(&mut body)
                    .map_err(|e|e.to_string()).and_then(|_| {
                        if body.len() as u64 > MAX_REQUEST { return Err("request too large".into()); }
                        let v = serde_json::from_slice(&body).map_err(|e|e.to_string())?;
                        if path == "/v1/billing/checkout" { crate::hosted::checkout(home,&v) } else { crate::hosted::entitlement(home,&v).map(|_| json!({"ok":true})) }
                    });
                match result { Ok(v) => reply(request,200,v), Err(e) => reply(request,400,json!({"error":e})) }
                continue;
            }
        }
        if request.method() == &Method::Get && path == "/v1/health" {
            reply(request, 200, json!({"version": env!("CARGO_PKG_VERSION")}));
        } else if request.method() == &Method::Get && path.starts_with("/v1/jobs/") {
            let id = &path[9..];
            let value = if valid_job_id(id) {
                fs::read(request_jobs.join(format!("{id}.json")))
                    .ok()
                    .and_then(|v| serde_json::from_slice::<Value>(&v).ok())
            } else {
                None
            };
            match value {
                Some(v) => reply(request, 200, v),
                None => reply(request, 404, json!({"error": "unknown job"})),
            }
        } else if request.method() == &Method::Post && path == "/v1/commands" {
            // Require a bounded body, including for chunked requests.
            let mut bytes = Vec::new();
            if request
                .as_reader()
                .take(MAX_REQUEST + 1)
                .read_to_end(&mut bytes)
                .is_err()
                || bytes.len() as u64 > MAX_REQUEST
            {
                reply(request, 413, json!({"error": "request too large"}));
                continue;
            }
            let command = serde_json::from_slice::<Vec<String>>(&bytes)
                .map_err(|e| e.to_string())
                .and_then(normalize);
            let command = match command {
                Ok(c) => c,
                Err(e) => {
                    reply(request, 400, json!({"error": e}));
                    continue;
                }
            };
            let id = match crate::random_password().and_then(|id| save_job(&request_jobs, &id, &json!({"id": id, "state": "queued"})).map(|_| id)) {
                Ok(id) => id,
                Err(e) => { reply(request, 500, json!({"error": e})); continue; }
            };
            match tx.try_send((request_jobs.clone(), id.clone(), command, tenant)) {
                Ok(()) => reply(request, 202, json!({"id": id, "state": "queued"})),
                Err(_) => {
                    let _ = fs::remove_file(request_jobs.join(format!("{id}.json")));
                    reply(request, 503, json!({"error": "job queue full"}));
                }
            }
        } else {
            reply(request, 404, json!({"error": "unknown endpoint"}));
        }
    }
    let _ = monitor.join();
    drop(tx);
    worker
        .join()
        .map_err(|_| "command worker panicked during shutdown")?;
    hosted_lifecycle("_hosted_down");
    let _shutdown_lock = worker_lock(false)?;
    crate::down()
}

fn hosted_lifecycle(action: &str) {
    for entry in fs::read_dir(crate::home().join(".tenants")).into_iter().flatten().flatten() {
        let tenant = entry.file_name().to_string_lossy().to_string();
        if crate::hosted::tenant_home(&crate::home(), &tenant).is_err() { continue; }
        let result = env::current_exe().map_err(|e|e.to_string()).and_then(|exe| {
            Command::new(exe).arg(action).env("SNAPSHOTDB_HOME",entry.path())
                .env("SNAPSHOTDB_TENANT",tenant).env("SNAPSHOTDB_INTERNAL","1")
                .env_remove("SNAPSHOTDB_TOKEN").env_remove("SNAPSHOTDB_HOSTED_TOKEN")
                .status().map_err(|e|e.to_string())
        });
        if !result.is_ok_and(|s|s.success()) { eprintln!("hosted lifecycle {action} failed for {}",entry.path().display()); }
    }
}

fn same_token(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

fn valid_job_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_alphanumeric())
}

fn client(mut args: Vec<String>) -> R<()> {
    let server = env::var("SNAPSHOTDB_SERVER").map_err(|_|
        "no server configured: set SNAPSHOTDB_SERVER and SNAPSHOTDB_TOKEN. Database copies run only on a deployed SnapshotDB server")?;
    let server = server.trim_end_matches('/');
    let uri: ureq::http::Uri = server.parse().map_err(|_| "invalid SNAPSHOTDB_SERVER URL")?;
    let loopback = matches!(uri.host(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if uri.scheme_str() != Some("https") && !(uri.scheme_str() == Some("http") && loopback) {
        return Err("SNAPSHOTDB_SERVER must use HTTPS (HTTP is allowed only on loopback, e.g. an SSH tunnel)".into());
    }
    if uri.authority().is_none()
        || uri.authority().unwrap().as_str().contains('@')
        || uri.query().is_some()
    {
        return Err("invalid SNAPSHOTDB_SERVER URL".into());
    }
    let token = token()?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .max_redirects(0)
        .build()
        .into();
    let detach = args.iter().any(|a| a == "--detach");
    args.retain(|a| a != "--detach");
    let get = |path: &str| -> R<Value> {
        let text = agent
            .get(format!("{server}{path}"))
            .header("Authorization", format!("Bearer {token}"))
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    };
    let id = if args.first().map(String::as_str) == Some("job") {
        if args.len() != 2 || !valid_job_id(&args[1]) {
            return Err("usage: snapshotdb job <id>".into());
        }
        args[1].clone()
    } else {
        let args = normalize(args)?;
        let body = serde_json::to_string(&args).map_err(|e| e.to_string())?;
        let text = agent.post(format!("{server}/v1/commands")).header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json").send(&body)
            .map_err(|e| format!("submission failed ({e}); no local database was created. Check the server before retrying"))?
            .body_mut().read_to_string().map_err(|e| e.to_string())?;
        let response: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        response["id"]
            .as_str()
            .filter(|id| valid_job_id(id))
            .ok_or("server returned no job ID")?
            .to_owned()
    };
    if detach {
        println!("{id}");
        return Ok(());
    }
    let mut polls = 0;
    let waiting_since = std::time::Instant::now();
    loop {
        let value = get(&format!("/v1/jobs/{id}"))
            .map_err(|e| format!("{e}; resume with: snapshotdb job {id}"))?;
        if value["state"] == "done" {
            print!("{}", value["stdout"].as_str().unwrap_or(""));
            eprint!("{}", value["stderr"].as_str().unwrap_or(""));
            let code = value["exit_code"].as_i64().unwrap_or(1) as i32;
            if code != 0 {
                std::process::exit(code);
            }
            return Ok(());
        }
        if polls % 60 == 0 {
            eprintln!(
                "Server job {id}: {}",
                value["state"].as_str().unwrap_or("pending")
            );
        }
        polls += 1;
        // Do not impose a one-second floor on ready branches. Long-running copies
        // still back off so a multi-hour clone does not continuously hammer the API.
        thread::sleep(if waiting_since.elapsed() < Duration::from_secs(2) {
            Duration::from_millis(20)
        } else if waiting_since.elapsed() < Duration::from_secs(10) {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(1)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_cannot_invoke_server_internals() {
        for command in ["serve", "_worker", "_proxy", "service", "up", "../rm"] {
            assert!(normalize(vec![command.into()]).is_err());
        }
        assert_eq!(
            normalize(vec![
                "clone".into(),
                "prod".into(),
                "postgresql://db/app".into()
            ])
            .unwrap(),
            ["sync", "postgres", "prod", "postgresql://db/app"]
        );
    }
    #[test]
    fn names_and_job_ids_cannot_escape_storage() {
        for name in ["../x", "/tmp/x", "x/y", "", ".jobs", "hello world"] {
            assert!(validate_name(name).is_err());
        }
        assert!(validate_name("feature-123").is_ok());
        assert!(!valid_job_id("../source"));
        assert!(same_token(b"token", b"token"));
        assert!(!same_token(b"token", b"other"));
    }
}
