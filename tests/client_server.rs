use std::{
    fs,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

const TOKEN: &str = "integration-test-token-32-characters-long";
// These fixtures release an OS-selected API port before the child binds it.
// Serializing them prevents another fixture's DB proxy from taking that port.
static SERVER_FIXTURE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn graceful_shutdown_stops_proxies_and_preserves_database() {
    let _fixture = SERVER_FIXTURE.lock().unwrap();
    let base = std::env::temp_dir().join(format!("anybranch-shutdown-{}", std::process::id()));
    fs::create_dir_all(&base).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let start = || Command::new(env!("CARGO_BIN_EXE_anybranch"))
        .env("ANYBRANCH_HOME", base.join("server"))
        .env("ANYBRANCH_TOKEN", TOKEN)
        .args(["serve", "--bind", &addr.to_string(), "--public-host", "127.0.0.1"])
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let mut server = start();
    let ready = || {
        for _ in 0..100 {
            if TcpListener::bind(addr).is_err() { return; }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("server failed to listen");
    };
    ready();
    let command = |args: &[&str]| {
        let out = cli(&base).env("ANYBRANCH_SERVER", format!("http://{addr}"))
            .env("ANYBRANCH_TOKEN", TOKEN).args(args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    let url = command(&["import", "sqlite", "retained", "--new", "--print-url"]);
    ureq::post(&url).send(r#"{"statements":[{"sql":"CREATE TABLE t(x)"},{"sql":"INSERT INTO t VALUES(42)"}]}"#).unwrap();
    let port: u16 = fs::read_to_string(base.join("server/retained/run/port")).unwrap().parse().unwrap();
    for attempt in 0..2 {
        Command::new("kill").args(["-TERM", &server.id().to_string()]).status().unwrap();
        let mut exited = false;
        for _ in 0..100 {
            if let Some(status) = server.try_wait().unwrap() {
                assert!(status.success()); exited = true; break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(exited, "graceful stop did not finish");
        for _ in 0..100 {
            if TcpListener::bind(("127.0.0.1", port)).is_ok() { break; }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(TcpListener::bind(("127.0.0.1", port)).is_ok(), "proxy survived shutdown");
        if attempt == 0 {
            server = start(); ready();
            assert_eq!(url, command(&["url", "retained"]));
            let body = ureq::post(&url).send(r#"{"statements":[{"sql":"SELECT x FROM t"}]}"#).unwrap().body_mut().read_to_string().unwrap();
            assert!(body.contains("[[42]]"));
        }
    }
    fs::remove_dir_all(base).unwrap();
}

struct Fixture {
    server: Child,
    base: PathBuf,
    url: String,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn cli(base: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_anybranch"));
    c.env("ANYBRANCH_HOME", base.join("client-must-not-exist"))
        .env_remove("ANYBRANCH_SERVER")
        .env_remove("ANYBRANCH_INTERNAL")
        .env_remove("ANYBRANCH_TOKEN");
    c
}

#[test]
fn client_requires_server_and_never_falls_back() {
    let base = std::env::temp_dir().join(format!("anybranch-no-server-{}", std::process::id()));
    for args in [
        vec!["import", "sqlite", "test", "--new"],
        vec!["_proxy", "test"],
    ] {
        let out = cli(&base).args(args).output().unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("no server configured"));
    }
    let out = cli(&base)
        .env("ANYBRANCH_SERVER", "http://127.0.0.1:1")
        .env("ANYBRANCH_TOKEN", TOKEN)
        .args(["import", "sqlite", "test", "--new"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!base.exists(), "client created local storage");
    let out = cli(&base)
        .env("ANYBRANCH_SERVER", "http://db.example.com")
        .env("ANYBRANCH_TOKEN", TOKEN)
        .arg("list")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("must use HTTPS"));
}

#[test]
fn authenticated_jobs_execute_only_in_server_storage() {
    let _fixture = SERVER_FIXTURE.lock().unwrap();
    let base = std::env::temp_dir().join(format!("anybranch-api-{}", std::process::id()));
    fs::create_dir_all(&base).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let server = Command::new(env!("CARGO_BIN_EXE_anybranch"))
        .env("ANYBRANCH_HOME", base.join("server"))
        .env("ANYBRANCH_TOKEN", TOKEN)
        .args([
            "serve",
            "--bind",
            &addr.to_string(),
            "--public-host",
            "127.0.0.1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut f = Fixture {
        server,
        base,
        url: format!("http://{addr}"),
    };
    for _ in 0..100 {
        if std::net::TcpStream::connect(addr).is_ok() {
            break;
        }
        assert!(
            f.server.try_wait().unwrap().is_none(),
            "server exited during startup"
        );
        thread::sleep(Duration::from_millis(50));
    }
    assert!(matches!(
        ureq::get(format!("{}/v1/health", f.url)).call(),
        Err(ureq::Error::StatusCode(401))
    ));
    let invalid = ureq::post(format!("{}/v1/commands", f.url))
        .header("Authorization", format!("Bearer {TOKEN}"))
        .send("[\"_proxy\",\"x\"]");
    assert!(matches!(invalid, Err(ureq::Error::StatusCode(400))));
    let mut command = cli(&f.base);
    command
        .env("ANYBRANCH_SERVER", &f.url)
        .env("ANYBRANCH_TOKEN", TOKEN);
    let out = command
        .args(["import", "sqlite", "fixture", "--new", "--detach"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let job = String::from_utf8(out.stdout).unwrap();
    let out = cli(&f.base)
        .env("ANYBRANCH_SERVER", &f.url)
        .env("ANYBRANCH_TOKEN", TOKEN)
        .args(["job", job.trim()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(f.base.join("server/fixture/data/db.sqlite").exists());
    assert!(!f.base.join("client-must-not-exist").exists());
    let run = |args: &[&str]| {
        let out = cli(&f.base)
            .env("ANYBRANCH_SERVER", &f.url)
            .env("ANYBRANCH_TOKEN", TOKEN)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    let query = |url: &str, sql: &str| {
        let body = serde_json::json!({"statements":[{"sql":sql}]}).to_string();
        ureq::post(url)
            .header("Content-Type", "application/json")
            .send(body)
            .unwrap()
            .body_mut()
            .read_to_string()
            .unwrap()
    };
    run(&["settings", "fixture", "set", "branch_sql", "SELECT missing_initialization_function()"]);
    let failed = cli(&f.base).env("ANYBRANCH_SERVER", &f.url).env("ANYBRANCH_TOKEN", TOKEN)
        .args(["create", "failed-hook", "--from", "fixture", "--print-url"]).output().unwrap();
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty(), "failed initialization must not return a URL");
    assert!(!f.base.join("server/failed-hook").exists());
    run(&["settings", "fixture", "remove", "branch_sql"]);
    let source = run(&["url", "fixture"]);
    assert!(source.starts_with("http://127.0.0.1:"));
    query(&source, "CREATE TABLE t(x INTEGER)");
    query(&source, "INSERT INTO t VALUES(1)");
    run(&["settings", "fixture", "set", "branch_sql", "CREATE TABLE initialized(x); INSERT INTO initialized VALUES(1)"]);
    run(&["prepare", "snap", "--from", "fixture", "--count", "2"]);
    run(&["settings", "fixture", "remove", "branch_sql"]);
    query(&source, "UPDATE t SET x=99");
    let first = run(&["create", "agent1", "--from", "snap", "--print-url"]);
    let second = run(&["create", "agent2", "--from", "snap", "--print-url"]);
    assert_ne!(first, second);
    assert!(query(&first, "SELECT count(*) FROM initialized").contains("[[1]]"));
    assert!(query(&second, "SELECT count(*) FROM initialized").contains("[[1]]"));
    assert!(query(&first, "SELECT x FROM t").contains("[[1]]"));
    query(&first, "UPDATE t SET x=42");
    assert!(query(&second, "SELECT x FROM t").contains("[[1]]"));
    assert!(query(&source, "SELECT x FROM t").contains("[[99]]"));
    assert_eq!(
        first,
        run(&["create", "agent1", "--from", "snap", "--print-url"])
    );
    for args in [
        vec!["create", "overflow", "--from", "snap"],
        vec!["start", "snap"],
        vec!["rm", "snap"],
        vec!["reset", "agent1"],
    ] {
        assert!(!cli(&f.base)
            .env("ANYBRANCH_SERVER", &f.url)
            .env("ANYBRANCH_TOKEN", TOKEN)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    for statement in [
        "ATTACH DATABASE '/tmp/escape.sqlite' AS escape",
        "PRAGMA writable_schema=1",
        "VACUUM INTO '/tmp/escape.sqlite'",
    ] {
        let body = serde_json::json!({"statements":[{"sql":statement}]}).to_string();
        assert!(matches!(
            ureq::post(&first).send(body),
            Err(ureq::Error::StatusCode(400))
        ));
    }
    let wrong = first.replace(
        first.split("token=").nth(1).unwrap(),
        second.split("token=").nth(1).unwrap(),
    );
    assert!(matches!(
        ureq::post(wrong).send("{}"),
        Err(ureq::Error::StatusCode(401))
    ));
    let rollback = serde_json::json!({"statements":[{"sql":"UPDATE t SET x=500"},{"sql":"SELECT * FROM missing_table"}]}).to_string();
    assert!(ureq::post(&first).send(rollback).is_err());
    assert!(query(&first, "SELECT x FROM t").contains("[[42]]"));
    let legacy_url = run(&["create", "legacy", "--from", "fixture", "--print-url"]);
    run(&["settings", "fixture", "set", "branch_sql", "UPDATE t SET x=0"]);
    let legacy_run = f.base.join("server/legacy/run");
    fs::remove_file(legacy_run.join("branch_sql.done-v1")).unwrap();
    fs::remove_file(legacy_run.join("ready-v1")).unwrap();
    fs::write(legacy_run.join("branch_sql.done"), "").unwrap();
    for args in [["start", "legacy"], ["url", "legacy"]] {
        assert!(!cli(&f.base).env("ANYBRANCH_SERVER", &f.url).env("ANYBRANCH_TOKEN", TOKEN)
            .args(args).output().unwrap().status.success());
    }
    assert!(ureq::post(legacy_url).send(r#"{"statements":[{"sql":"SELECT * FROM t"}]}"#).is_err());
    run(&["rm", "legacy"]);
    for name in ["agent1", "agent2", "snap", "fixture"] {
        run(&["rm", name]);
    }
    assert!(!f.base.join("client-must-not-exist").exists());
    let duplicate = Command::new(env!("CARGO_BIN_EXE_anybranch"))
        .env("ANYBRANCH_HOME", f.base.join("server"))
        .env("ANYBRANCH_TOKEN", TOKEN)
        .args([
            "serve",
            "--bind",
            "127.0.0.1:0",
            "--public-host",
            "db.example.test",
        ])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("another Anybranch server"));
}
