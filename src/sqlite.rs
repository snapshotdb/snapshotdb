//! SQLite stays on the server. Its branch URL accepts authenticated SQL-over-HTTP
//! requests; no file URL or database download is exposed to the client.
use crate::*;
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    types::{Value as SqlValue, ValueRef},
    Connection,
};
use serde_json::{json, Value};
use std::io::Read;
use tiny_http::{Header, Method, Response, Server};

pub fn lock(b: &Branch) -> R<Option<fs::File>> {
    if b.engine != Engine::Sqlite {
        return Ok(None);
    }
    let f = io(fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(b.run().join("sqlite.lock")))?;
    io(fs2::FileExt::lock_exclusive(&f))?;
    Ok(Some(f))
}

pub fn url(b: &Branch) -> R<String> {
    let host = env::var("ANYBRANCH_PUBLIC_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    Ok(format!(
        "http://{host}:{}/v1/query?token={}",
        b.port().ok_or("SQLite endpoint is stopped")?,
        b.password().ok_or("SQLite credentials missing")?
    ))
}

pub fn serve(b: &Branch) -> R<()> {
    let bind = env::var("ANYBRANCH_DB_BIND").unwrap_or_else(|_| "127.0.0.1".into());
    let server = Server::http((bind.as_str(), b.port().ok_or("no SQLite port")?))
        .map_err(|e| e.to_string())?;
    let token = b.password().ok_or("SQLite credentials missing")?;
    for mut req in server.incoming_requests() {
        let expected = format!("/v1/query?token={token}");
        let auth = req.url().as_bytes();
        let valid = auth.len() == expected.len()
            && auth
                .iter()
                .zip(expected.bytes())
                .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                == 0;
        let (status, result) = if !valid {
            (401, json!({"error":"unauthorized"}))
        } else if req.method() != &Method::Post {
            (
                405,
                json!({"error":"POST a JSON object with a statements array"}),
            )
        } else {
            let mut bytes = Vec::new();
            if req.as_reader().take(65537).read_to_end(&mut bytes).is_err() || bytes.len() > 65536 {
                (413, json!({"error":"request too large"}))
            } else {
                match serde_json::from_slice(&bytes)
                    .map_err(|e| e.to_string())
                    .and_then(|v| query(b, v))
                {
                    Ok(value) => (200, value),
                    Err(e) => (400, json!({"error":e})),
                }
            }
        };
        let _ = req.respond(
            Response::from_string(result.to_string())
                .with_status_code(status)
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap()),
        );
    }
    Ok(())
}

fn query(b: &Branch, body: Value) -> R<Value> {
    pool::writable(b)?;
    let _lock = lock(b)?;
    let statements = body["statements"]
        .as_array()
        .filter(|s| !s.is_empty() && s.len() <= 100)
        .ok_or("statements must contain 1-100 SQL statements")?;
    let mut db = Connection::open_with_flags(
        b.data().join("db.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .map_err(|e| e.to_string())?;
    db.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, 1_000_000)
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now();
    db.progress_handler(
        1000,
        Some(move || deadline.elapsed() > Duration::from_secs(2)),
    )
    .map_err(|e| e.to_string())?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    tx.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Attach { .. }
        | AuthAction::Detach { .. }
        | AuthAction::Pragma { .. }
        | AuthAction::Transaction { .. }
        | AuthAction::Savepoint { .. } => Authorization::Deny,
        _ => Authorization::Allow,
    }))
    .map_err(|e| e.to_string())?;
    let mut results = vec![];
    let mut response_bytes = 0;
    for statement in statements {
        let sql = statement["sql"]
            .as_str()
            .ok_or("each statement needs sql")?;
        let params = statement
            .get("params")
            .map(|p| p.as_array().ok_or("params must be an array"))
            .transpose()?
            .cloned()
            .unwrap_or_default();
        let params: R<Vec<SqlValue>> = params
            .into_iter()
            .map(|p| {
                Ok(match p {
                    Value::Null => SqlValue::Null,
                    Value::Bool(b) => SqlValue::Integer(b as i64),
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            SqlValue::Integer(i)
                        } else {
                            SqlValue::Real(n.as_f64().ok_or("invalid number")?)
                        }
                    }
                    Value::String(s) => SqlValue::Text(s),
                    _ => return Err("parameters must be JSON scalar values".into()),
                })
            })
            .collect();
        let mut stmt = tx.prepare(sql).map_err(|e| e.to_string())?;
        let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt
            .query(rusqlite::params_from_iter(params?))
            .map_err(|e| e.to_string())?;
        let mut output = vec![];
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            if output.len() >= 10000 {
                return Err("result exceeds 10000 rows; use LIMIT".into());
            }
            let values: R<Vec<Value>> = (0..columns.len())
                .map(|i| {
                    Ok(match row.get_ref(i).map_err(|e| e.to_string())? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(v) => json!(v),
                        ValueRef::Real(v) => json!(v),
                        ValueRef::Text(v) => json!(String::from_utf8_lossy(v)),
                        ValueRef::Blob(v) => json!({"bytes":v}),
                    })
                })
                .collect();
            let values = values?;
            response_bytes += serde_json::to_vec(&values)
                .map_err(|e| e.to_string())?
                .len();
            if response_bytes > 4_000_000 {
                return Err("result exceeds 4 MB; use LIMIT".into());
            }
            output.push(values);
        }
        results.push(json!({"columns":columns,"rows":output}));
    }
    tx.authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(json!({"results":results}))
}
