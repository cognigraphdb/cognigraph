//! CG-114 through the real binary: `export` must fail on anything that is not
//! a complete snapshot, must never replace an existing file, and must write
//! the snapshot privately.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};

const SNAPSHOT: &str =
    r#"{"collections":{"notes":{"type":"document","documents":{"a":{"_key":"a","n":1}}}}}"#;

/// Serve one canned raw HTTP response to the next connection, then stop.
fn serve_once(response: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => request.extend_from_slice(&buffer[..read]),
                }
            }
            let _ = stream.write_all(&response);
            let _ = stream.flush();
            // Dropping the stream closes the connection, also mid-body.
        }
    });
    format!("http://{address}")
}

fn response(status: &str, content_type: &str, declared_length: usize, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {declared_length}\r\nConnection: close\r\n\r\n{body}"
    )
    .into_bytes()
}

fn json_ok(body: &str) -> Vec<u8> {
    response("200 OK", "application/json", body.len(), body)
}

fn export(url: &str, out: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cognigraph"));
    command
        .env_remove("COGNIGRAPH_URL")
        .env_remove("COGNIGRAPH_TOKEN")
        .args(["--url", url, "export"]);
    if let Some(out) = out {
        command.arg("--out").arg(out);
    }
    command.output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn leftovers(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_complete_snapshot_is_written_privately() {
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("snapshot.json");
    let result = export(&serve_once(json_ok(SNAPSHOT)), Some(&out));
    assert_eq!(result.status.code(), Some(0), "{}", stderr(&result));

    let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(SNAPSHOT).unwrap();
    assert_eq!(written, expected);
    assert_eq!(leftovers(temp.path()), ["snapshot.json"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&out).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "a snapshot holds password hashes");
    }
}

#[test]
fn anything_but_a_complete_snapshot_fails_and_writes_nothing() {
    let half = &SNAPSHOT[..SNAPSHOT.len() / 2];
    let cases: Vec<(&str, Vec<u8>)> = vec![
        // The server promised the whole body and closed half-way.
        (
            "truncated body",
            response("200 OK", "application/json", SNAPSHOT.len(), half),
        ),
        // Complete as HTTP, but the JSON itself is cut off.
        ("cut-off json", json_ok(half)),
        (
            "html page",
            response(
                "200 OK",
                "text/html",
                31,
                "<!doctype html><html>console</html>",
            ),
        ),
        ("empty body", json_ok("")),
        ("json string", json_ok("\"<non-JSON response>\"")),
        ("not a snapshot", json_ok(r#"{"results":[],"count":0}"#)),
        (
            "collections is not an object",
            json_ok(r#"{"collections":[]}"#),
        ),
        (
            "server error",
            response(
                "500 Internal Server Error",
                "application/json",
                16,
                r#"{"error":"boom"}"#,
            ),
        ),
        (
            "gateway error page",
            response("502 Bad Gateway", "text/html", 15, "<h1>Bad gw</h1>"),
        ),
    ];
    for (name, canned) in cases {
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path().join("snapshot.json");
        let result = export(&serve_once(canned.clone()), Some(&out));
        assert_eq!(result.status.code(), Some(1), "{name}: {}", stderr(&result));
        assert!(!out.exists(), "{name}: nothing may be written");
        assert!(
            leftovers(temp.path()).is_empty(),
            "{name}: {:?}",
            leftovers(temp.path())
        );
        assert!(
            !stderr(&result).contains("snapshot written"),
            "{name}: {}",
            stderr(&result)
        );

        // Without --out the failure must not print a body to stdout either.
        let result = export(&serve_once(canned), None);
        assert_eq!(result.status.code(), Some(1), "{name}");
        assert!(result.stdout.is_empty(), "{name}: {:?}", result.stdout);
    }
}

#[test]
fn an_existing_backup_is_never_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("snapshot.json");
    std::fs::write(&out, "yesterday's good backup").unwrap();

    // Neither a bad response nor a good one may touch the existing file.
    for canned in [
        response("200 OK", "text/html", 20, "<html>console</html>"),
        json_ok(SNAPSHOT),
    ] {
        let result = export(&serve_once(canned), Some(&out));
        assert_eq!(result.status.code(), Some(1), "{}", stderr(&result));
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "yesterday's good backup"
        );
        assert_eq!(leftovers(temp.path()), ["snapshot.json"]);
    }
}

#[test]
fn an_unreachable_server_is_an_error() {
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("snapshot.json");
    // Bind and drop to get a port nothing listens on.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let result = export(&format!("http://127.0.0.1:{port}"), Some(&out));
    assert_eq!(result.status.code(), Some(1));
    assert!(!out.exists());
}
