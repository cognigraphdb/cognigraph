//! Publication safety and scale: nothing is published or left behind on
//! rejection or failure, an existing store is never touched, a published
//! store survives a failed report, and large split dumps stream in chunks
//! with file-order error precedence.

use super::*;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};

const STEM: &str = "customers_00000000000000000000000000000000";
const EDGE_STEM: &str = "knows_11111111111111111111111111111111";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/arangodump")
        .join(name)
}

fn options(dump: &Path, output: &Path) -> Options {
    Options {
        dump: dump.to_path_buf(),
        output: output.to_path_buf(),
        dry_run: false,
        report: None,
        limits: Default::default(),
    }
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn copy_fixture(name: &str, into: &Path) -> PathBuf {
    let target = into.join("dump");
    std::fs::create_dir(&target).unwrap();
    for entry in std::fs::read_dir(fixture(name)).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
    }
    target
}

/// A synthetic split gzip dump: `count` customers over `parts` files with a
/// unique `email`, and a chain of `count - 1` edges. `lines` may rewrite a
/// customer line by its zero-based index.
fn large_dump(
    root: &Path,
    count: usize,
    parts: usize,
    lines: impl Fn(usize, String) -> String,
) -> PathBuf {
    let dump = root.join("large");
    std::fs::create_dir(&dump).unwrap();
    std::fs::write(
        dump.join("dump.json"),
        r#"{"database":"large","useEnvelope":false,"useVPack":false}"#,
    )
    .unwrap();
    std::fs::write(dump.join("ENCRYPTION"), "none").unwrap();
    let structure = |name: &str, kind: u8, indexes: serde_json::Value| {
        json!({"parameters": {"name": name, "type": kind, "isSystem": false}, "indexes": indexes})
            .to_string()
    };
    std::fs::write(
        dump.join(format!("{STEM}.structure.json")),
        structure(
            "customers",
            2,
            json!([{"type": "persistent", "fields": ["email"], "unique": true, "name": "email"}]),
        ),
    )
    .unwrap();
    std::fs::write(
        dump.join(format!("{EDGE_STEM}.structure.json")),
        structure("knows", 3, json!([])),
    )
    .unwrap();
    let per_part = count.div_ceil(parts);
    for part in 0..parts {
        let file = std::fs::File::create(dump.join(format!("{STEM}.{part}.data.json.gz"))).unwrap();
        let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        for i in part * per_part..((part + 1) * per_part).min(count) {
            let line =
                json!({"_key": format!("c{i}"), "_id": format!("customers/c{i}"), "_rev": "_r",
                              "email": format!("{i}@example.test"), "n": i})
                .to_string();
            writeln!(gz, "{}", lines(i, line)).unwrap();
        }
        gz.finish().unwrap();
    }
    let mut edges = std::fs::File::create(dump.join(format!("{EDGE_STEM}.0.data.json"))).unwrap();
    for i in 1..count {
        writeln!(
            edges,
            "{}",
            json!({"_key": format!("k{i}"), "_from": format!("customers/c{}", i - 1),
                                      "_to": format!("customers/c{i}")})
        )
        .unwrap();
    }
    dump
}

#[test]
fn an_existing_store_is_refused_and_left_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("store.redb");
    std::fs::write(&output, b"precious").unwrap();
    let outcome = import(&options(&fixture("3.12/shop-plain"), &output), Stage::Never);
    assert_eq!(outcome.code, EXIT_USAGE);
    assert!(outcome.report.is_none());
    assert_eq!(std::fs::read(&output).unwrap(), b"precious");
    assert_eq!(entries(temp.path()), ["store.redb"]);
}

#[test]
fn unsafe_or_ambiguous_paths_are_usage_errors() {
    let temp = tempfile::tempdir().unwrap();
    let dump = copy_fixture("3.12/shop-plain", temp.path());
    let inside = options(&dump, &dump.join("store.redb"));
    assert_eq!(import(&inside, Stage::Never).code, EXIT_USAGE);
    let mut missing_dir = options(&dump, &temp.path().join("store.redb"));
    missing_dir.report = Some(temp.path().join("absent/report.json"));
    assert_eq!(import(&missing_dir, Stage::Never).code, EXIT_USAGE);
    let mut same = options(&dump, &temp.path().join("store.redb"));
    same.report = Some(temp.path().join("store.redb"));
    assert_eq!(import(&same, Stage::Never).code, EXIT_USAGE);
    let absent = options(&temp.path().join("nope"), &temp.path().join("store.redb"));
    assert_eq!(import(&absent, Stage::Never).code, EXIT_USAGE);
    assert_eq!(entries(temp.path()), ["dump"]);
}

#[test]
fn a_dry_run_writes_only_its_report() {
    let temp = tempfile::tempdir().unwrap();
    let mut dry = options(
        &fixture("3.11/shop-envelope"),
        &temp.path().join("store.redb"),
    );
    dry.dry_run = true;
    dry.report = Some(temp.path().join("report.json"));
    let outcome = import(&dry, Stage::Never);
    assert_eq!(outcome.code, EXIT_OK);
    assert_eq!(entries(temp.path()), ["report.json"]);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(temp.path().join("report.json")).unwrap()).unwrap();
    assert_eq!(
        (report["mode"].as_str(), report["status"].as_str()),
        (Some("dry_run"), Some("accepted"))
    );
    assert_eq!(report["destination"], serde_json::Value::Null);
    assert!(
        report["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["sha256"].as_str().unwrap().len() == 64)
    );
    let text = serde_json::to_string(&report).unwrap();
    assert!(
        !text.contains("ana@example.test"),
        "reports never carry document bodies"
    );
}

#[test]
fn failures_before_publication_leave_no_store_and_no_staging() {
    for stage in [Stage::Staged, Stage::Verified] {
        let temp = tempfile::tempdir().unwrap();
        let mut run = options(&fixture("3.12/shop-gzip"), &temp.path().join("store.redb"));
        run.report = Some(temp.path().join("report.json"));
        let outcome = import(&run, stage);
        assert_eq!(outcome.code, EXIT_FAILED, "{stage:?}");
        assert_eq!(entries(temp.path()), ["report.json"], "{stage:?}");
        assert_eq!(outcome.report.unwrap().status, "failed");
    }
}

#[test]
fn a_source_changed_during_the_import_is_not_published() {
    let temp = tempfile::tempdir().unwrap();
    let dump = copy_fixture("3.12/shop-plain", temp.path());
    let output = temp.path().join("store.redb");
    let outcome = import(&options(&dump, &output), Stage::ChangeSource);
    assert_eq!(outcome.code, EXIT_FAILED);
    let report = outcome.report.unwrap();
    assert_eq!(
        report.errors.iter().map(|e| e.code).collect::<Vec<_>>(),
        ["source_changed"]
    );
    assert_eq!(entries(temp.path()), ["dump"]);
}

#[test]
fn a_failed_report_after_publication_keeps_the_complete_store_and_blocks_a_retry() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("store.redb");
    let mut run = options(&fixture("3.12/shop-split"), &output);
    run.report = Some(temp.path().join("report.json"));
    let outcome = import(&run, Stage::Report);
    assert_eq!(outcome.code, EXIT_REPORT_FAILED);
    assert!(outcome.message.unwrap().contains("is complete"));
    assert_eq!(entries(temp.path()), ["store.redb"]);
    let backend = NativeBackend::open(&output).unwrap();
    let customers = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(backend.list_documents("customers", None, None))
        .unwrap();
    assert_eq!(customers.len(), 5);
    drop(backend);
    assert_eq!(import(&run, Stage::Never).code, EXIT_USAGE);
}

#[test]
fn a_rejection_reports_and_publishes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let mut run = options(
        &fixture("derived/unique-violation"),
        &temp.path().join("store.redb"),
    );
    run.report = Some(temp.path().join("report.json"));
    assert_eq!(import(&run, Stage::Never).code, EXIT_REJECTED);
    assert_eq!(entries(temp.path()), ["report.json"]);
}

#[test]
fn large_split_dumps_stream_in_chunks_and_resolve_every_edge() {
    let temp = tempfile::tempdir().unwrap();
    let dump = large_dump(temp.path(), 5000, 3, |_, line| line);
    let output = temp.path().join("store.redb");
    let outcome = import(&options(&dump, &output), Stage::Never);
    let report = outcome.report.unwrap();
    assert_eq!(
        outcome.code, EXIT_OK,
        "{:?} {:?}",
        report.errors, outcome.message
    );
    assert_eq!(report.collections["customers"].documents, 5000);
    assert_eq!(report.collections["knows"].documents, 4999);
    assert_eq!(report.dropped["_rev"], json!(5000));
    let backend = NativeBackend::open(&output).unwrap();
    let rows = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(backend.query(
            r#"FOR v IN 1..3 OUTBOUND "customers/c0" knows RETURN v._key"#,
            Default::default(),
        ))
        .unwrap();
    assert_eq!(rows, vec![json!("c1"), json!("c2"), json!("c3")]);
}

#[test]
fn a_unique_violation_is_located_in_file_order_across_chunks() {
    let temp = tempfile::tempdir().unwrap();
    let dump = large_dump(temp.path(), 3000, 2, |i, line| {
        if i == 2400 {
            line.replace("\"2400@example.test\"", "\"10@example.test\"")
        } else {
            line
        }
    });
    let outcome = import(
        &options(&dump, &temp.path().join("store.redb")),
        Stage::Never,
    );
    let report = outcome.report.unwrap();
    assert_eq!(outcome.code, EXIT_REJECTED);
    let errors: Vec<_> = report
        .errors
        .iter()
        .map(|e| (e.code, e.key.as_deref()))
        .collect();
    assert_eq!(errors, [("unique_violation", Some("c2400"))]);
}

#[test]
fn within_a_collection_the_first_problem_in_file_order_wins() {
    // A pending (unflushed) violation at line 1501 precedes a corrupt line 1601.
    let temp = tempfile::tempdir().unwrap();
    let dump = large_dump(temp.path(), 2000, 1, |i, line| match i {
        1500 => line.replace("\"1500@example.test\"", "\"3@example.test\""),
        1600 => "{not json".to_string(),
        _ => line,
    });
    let report = import(&options(&dump, &temp.path().join("s.redb")), Stage::Never)
        .report
        .unwrap();
    assert_eq!(
        report.errors.iter().map(|e| e.code).collect::<Vec<_>>(),
        ["unique_violation"]
    );
    // And the other way round: the corrupt line comes first.
    let temp = tempfile::tempdir().unwrap();
    let dump = large_dump(temp.path(), 2000, 1, |i, line| match i {
        1400 => "{not json".to_string(),
        1500 => line.replace("\"1500@example.test\"", "\"3@example.test\""),
        _ => line,
    });
    let report = import(&options(&dump, &temp.path().join("s.redb")), Stage::Never)
        .report
        .unwrap();
    let errors: Vec<_> = report.errors.iter().map(|e| (e.code, e.line)).collect();
    assert_eq!(errors, [("corrupt_data_file", Some(1401))]);
}

#[test]
fn record_expansion_and_file_count_limits_reject() {
    let temp = tempfile::tempdir().unwrap();
    let dump = large_dump(temp.path(), 2000, 2, |_, line| line);
    for (limits, code) in [
        (
            Limits {
                max_record_bytes: 40,
                ..Default::default()
            },
            "record_too_large",
        ),
        (
            Limits {
                max_expanded_bytes: 50_000,
                ..Default::default()
            },
            "dump_too_large",
        ),
        (
            Limits {
                max_files: 4,
                ..Default::default()
            },
            "dump_too_large",
        ),
    ] {
        let mut run = options(&dump, &temp.path().join("s.redb"));
        run.limits = limits;
        let outcome = import(&run, Stage::Never);
        assert_eq!(outcome.code, EXIT_REJECTED, "{limits:?}");
        assert_eq!(outcome.report.unwrap().errors[0].code, code, "{limits:?}");
        assert!(!temp.path().join("s.redb").exists());
    }
}

use super::options::Limits;
