//! Parity with the reference reader: every fixture reaches its recorded
//! outcome, and every accepted fixture publishes exactly the expected
//! documents, collection types and constraints (fixtures/arangodump).

use super::*;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::Value;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/arangodump")
}

fn load(path: PathBuf) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn canonical(items: impl IntoIterator<Item = Value>) -> Vec<String> {
    let mut out: Vec<String> = items.into_iter().map(|v| v.to_string()).collect();
    out.sort();
    out
}

fn options(dump: PathBuf, output: PathBuf, dry_run: bool) -> Options {
    Options {
        dump,
        output,
        dry_run,
        report: None,
        limits: Default::default(),
    }
}

fn check_errors(key: &str, report: &Report, wanted: &[Value]) {
    assert_eq!(
        report.errors.len(),
        wanted.len(),
        "{key}: {:?}",
        report.errors
    );
    for want in wanted {
        let want = want.as_object().unwrap();
        let hit = report.errors.iter().any(|e| {
            let got = serde_json::to_value(e).unwrap();
            want.iter().all(|(k, v)| got.get(k) == Some(v))
        });
        assert!(hit, "{key}: expected {want:?} in {:?}", report.errors);
    }
}

fn check_accepted(key: &str, entry: &Value, report: &Report) {
    let oracle = load(fixtures().join(format!(
        "expected/{}.json",
        entry["dataset"].as_str().unwrap()
    )));
    let empty = entry["expected"]["empty"].as_bool() == Some(true);
    let collections: Vec<(String, &str, u64)> = oracle["collections"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, c)| {
            let count = if empty {
                0
            } else {
                c["documents"].as_object().unwrap().len() as u64
            };
            (name.clone(), c["type"].as_str().unwrap(), count)
        })
        .collect();
    let got: Vec<(String, &str, u64)> = report
        .collections
        .iter()
        .map(|(n, c)| (n.clone(), c.kind, c.documents))
        .collect();
    assert_eq!(got, collections, "{key}");
    let constraints = report.constraints.iter().map(|c| {
        serde_json::json!({"collection": c.collection, "unique": c.unique, "fields": c.fields, "sparse": c.sparse})
    });
    assert_eq!(
        canonical(constraints),
        canonical(oracle["constraints"].as_array().unwrap().clone()),
        "{key}"
    );
    let reported = report
        .not_carried
        .iter()
        .map(|n| serde_json::to_value(n).unwrap());
    assert_eq!(
        canonical(reported),
        canonical(oracle["not_carried"].as_array().unwrap().clone()),
        "{key}"
    );
    let ignored: Vec<String> = entry["expected"]["ignored"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect())
        .unwrap_or_default();
    assert_eq!(report.ignored, ignored, "{key}");
    let warnings: Vec<&str> = entry["expected"]["warnings"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap()).collect())
        .unwrap_or_default();
    assert_eq!(report.warnings, warnings, "{key}");
    let system = key.ends_with("shop-system");
    assert_eq!(
        !report.excluded.is_empty(),
        system,
        "{key}: {:?}",
        report.excluded
    );
    assert!(
        report
            .excluded
            .iter()
            .all(|e| e["collection"].as_str().unwrap().starts_with('_'))
    );
}

/// The published store holds exactly the expected documents with `_id`
/// derived from the collection and key, plus the carried constraints.
async fn check_store(key: &str, entry: &Value, store: &std::path::Path) {
    let oracle = load(fixtures().join(format!(
        "expected/{}.json",
        entry["dataset"].as_str().unwrap()
    )));
    let empty = entry["expected"]["empty"].as_bool() == Some(true);
    let backend = NativeBackend::open(store).unwrap();
    for (name, collection) in oracle["collections"].as_object().unwrap() {
        let mut stored = backend.list_documents(name, None, None).await.unwrap();
        let expected = if empty {
            serde_json::Map::new()
        } else {
            collection["documents"].as_object().unwrap().clone()
        };
        assert_eq!(stored.len(), expected.len(), "{key}/{name}");
        for doc in &mut stored {
            let object = doc.as_object_mut().unwrap();
            let id = object.remove("_id").unwrap();
            let doc_key = object["_key"].as_str().unwrap().to_string();
            assert_eq!(id, Value::String(format!("{name}/{doc_key}")), "{key}");
            assert_eq!(
                Some(&*doc),
                expected.get(&doc_key),
                "{key}/{name}/{doc_key}"
            );
        }
    }
    for constraint in oracle["constraints"].as_array().unwrap() {
        let name = constraint["collection"].as_str().unwrap();
        let declared = backend.list_indexes(name).await.unwrap();
        assert!(
            declared.iter().any(|d| d.unique
                && serde_json::to_value(&d.fields).unwrap() == constraint["fields"]
                && Value::Bool(d.sparse) == constraint["sparse"]),
            "{key}: {constraint} not declared in {declared:?}"
        );
    }
}

#[test]
fn every_fixture_reaches_its_recorded_outcome_and_accepted_ones_publish_exactly() {
    let reader = tokio::runtime::Runtime::new().unwrap();
    let manifest = load(fixtures().join("manifest.json"));
    let fixtures_list = manifest["fixtures"].as_object().unwrap();
    assert_eq!(fixtures_list.len(), 25);
    for (key, entry) in fixtures_list {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("store.redb");
        let dump = fixtures().join(key);
        let dry = import(&options(dump.clone(), output.clone(), true), Stage::Never);
        let report = dry.report.expect("report");
        let outcome = entry["expected"]["outcome"].as_str().unwrap();
        assert!(!output.exists(), "{key}: a dry run created the store");
        if outcome == "rejected" {
            assert_eq!(
                (dry.code, report.status),
                (EXIT_REJECTED, "rejected"),
                "{key}: {:?}",
                report.errors
            );
            check_errors(
                key,
                &report,
                entry["expected"]["errors"].as_array().unwrap(),
            );
            let real = import(&options(dump, output.clone(), false), Stage::Never);
            assert_eq!(real.code, EXIT_REJECTED, "{key}");
            assert!(!output.exists(), "{key}: a rejected import published");
            assert_eq!(
                std::fs::read_dir(temp.path()).unwrap().count(),
                0,
                "{key}: staging left behind"
            );
            continue;
        }
        assert_eq!(
            (dry.code, report.status),
            (EXIT_OK, "accepted"),
            "{key}: {:?}",
            report.errors
        );
        check_accepted(key, entry, &report);
        let real = import(&options(dump, output.clone(), false), Stage::Never);
        let published = real.report.expect("report");
        assert_eq!(
            (real.code, published.status),
            (EXIT_OK, "published"),
            "{key}: {:?}",
            real.message
        );
        check_accepted(key, entry, &published);
        let leftovers: Vec<_> = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .filter(|n| n != "store.redb")
            .collect();
        assert!(leftovers.is_empty(), "{key}: {leftovers:?}");
        reader.block_on(check_store(key, entry, &output));
    }
}
