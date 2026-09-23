//! CG-66 through the real binary: exit codes, stdout report, and a process
//! killed mid-import never exposes a store at the destination.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cognigraph"));
    command
        .env_remove("COGNIGRAPH_URL")
        .env_remove("COGNIGRAPH_TOKEN");
    command
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/arangodump")
        .join(name)
}

fn import(dump: &Path, output: &Path, extra: &[&str]) -> std::process::Output {
    cli()
        .args(["import", "--from-arangodump"])
        .arg(dump)
        .arg("--output")
        .arg(output)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn exit_codes_and_the_stdout_report() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("store.redb");
    let accepted = import(&fixture("3.11/shop-gzip"), &output, &[]);
    assert_eq!(
        accepted.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(report["status"], "published");
    assert!(output.is_file());

    let again = import(&fixture("3.11/shop-gzip"), &output, &[]);
    assert_eq!(again.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&again.stderr).contains("already exists"));

    let rejected = import(
        &fixture("3.12/broken-plain"),
        &temp.path().join("b.redb"),
        &[],
    );
    assert_eq!(rejected.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert_eq!(report["errors"].as_array().unwrap().len(), 5);

    let dry = import(
        &fixture("3.12/vpack"),
        &temp.path().join("v.redb"),
        &["--dry-run"],
    );
    assert_eq!(dry.status.code(), Some(3));
    let usage = cli()
        .args(["import", "--from-arangodump", "x"])
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
    let mut names: Vec<_> = std::fs::read_dir(temp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["store.redb"]);
}

fn big_dump(root: &Path, count: usize) -> PathBuf {
    let dump = root.join("big");
    std::fs::create_dir(&dump).unwrap();
    std::fs::write(dump.join("dump.json"), r#"{"database":"big"}"#).unwrap();
    std::fs::write(
        dump.join("items_22222222222222222222222222222222.structure.json"),
        r#"{"parameters":{"name":"items","type":2},"indexes":[]}"#,
    )
    .unwrap();
    let mut data = std::io::BufWriter::new(
        std::fs::File::create(dump.join("items_22222222222222222222222222222222.data.json"))
            .unwrap(),
    );
    for i in 0..count {
        writeln!(
            data,
            r#"{{"_key":"i{i}","payload":"{}","n":{i}}}"#,
            "x".repeat(64)
        )
        .unwrap();
    }
    dump
}

#[cfg(unix)]
#[test]
fn a_process_killed_mid_import_never_exposes_a_store() {
    let temp = tempfile::tempdir().unwrap();
    let dump = big_dump(temp.path(), 400_000);
    let output = temp.path().join("store.redb");
    let mut child = cli()
        .args(["import", "--from-arangodump"])
        .arg(&dump)
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // Wait until the staging store holds data, then kill without warning.
    let deadline = Instant::now() + Duration::from_secs(60);
    let staging = loop {
        let found = std::fs::read_dir(temp.path()).unwrap().find_map(|e| {
            let path = e.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            name.starts_with(".store.redb.cg-import-").then_some(path)
        });
        if let Some(dir) = found
            && std::fs::metadata(dir.join("store.redb")).is_ok_and(|m| m.len() > 4 << 20)
        {
            break dir;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "import finished before it could be interrupted"
        );
        assert!(Instant::now() < deadline, "staging never grew");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(!output.exists());
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!output.exists(), "a killed import exposed a store");
    assert!(
        staging.exists(),
        "a killed run leaves only its own staging directory"
    );

    // A new run is not blocked by the leftover and publishes the full store.
    let rerun = import(&dump, &output, &["--max-expanded-bytes", "1000000000"]);
    assert_eq!(
        rerun.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&rerun.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&rerun.stdout).unwrap();
    assert_eq!(report["collections"]["items"]["documents"], 400_000);
    assert!(
        staging.exists(),
        "the importer never deletes another run's staging"
    );
}
