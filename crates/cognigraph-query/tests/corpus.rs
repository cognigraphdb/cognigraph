//! File-driven CGQL corpus runner (in-memory executor).
//!
//! See `tests/corpus/README.md` for the corpus layout. The same exec cases
//! also run through the native backend in
//! `crates/cognigraph-native/tests/cgql_corpus.rs`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use cognigraph_query::{InMemoryDataset, parse_and_execute, parse_and_plan, parse_query};
use serde_json::Value;

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn cgql_files(sub: &str) -> Vec<PathBuf> {
    let dir = corpus_dir().join(sub);
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("missing corpus dir {dir:?}: {e}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cgql"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .cgql files in {dir:?}");
    files
}

fn binds_from_header(source: &str) -> HashMap<String, Value> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("// binds:"))
        .flat_map(|json| {
            serde_json::from_str::<serde_json::Map<String, Value>>(json)
                .expect("invalid `// binds:` header")
        })
        .collect()
}

fn dataset() -> InMemoryDataset {
    let raw: Value =
        serde_json::from_str(&fs::read_to_string(corpus_dir().join("dataset.json")).unwrap())
            .unwrap();
    let mut dataset = InMemoryDataset::new();
    for (name, entry) in raw["collections"].as_object().unwrap() {
        let rows: Vec<Value> = entry["documents"]
            .as_object()
            .unwrap()
            .values()
            .cloned()
            .collect();
        dataset.insert_collection(name.clone(), rows);
    }
    dataset
}

#[test]
fn corpus_parse_ok() {
    for file in cgql_files("parse_ok") {
        let source = fs::read_to_string(&file).unwrap();
        parse_and_plan(&source)
            .unwrap_or_else(|e| panic!("{file:?} should parse, validate, and plan: {e}"));
    }
}

#[test]
fn corpus_parse_err() {
    for file in cgql_files("parse_err") {
        let source = fs::read_to_string(&file).unwrap();
        assert!(
            parse_query(&source).is_err(),
            "{file:?} should fail to parse"
        );
    }
}

#[test]
fn corpus_validate_err() {
    for file in cgql_files("validate_err") {
        let source = fs::read_to_string(&file).unwrap();
        parse_query(&source)
            .unwrap_or_else(|e| panic!("{file:?} must parse (it is a validation case): {e}"));
        assert!(
            parse_and_plan(&source).is_err(),
            "{file:?} should fail validation"
        );
    }
}

#[test]
fn corpus_exec() {
    let dataset = dataset();
    for file in cgql_files("exec") {
        let source = fs::read_to_string(&file).unwrap();
        let binds = binds_from_header(&source);
        let expected_path = file.with_extension("json");
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("missing expected output {expected_path:?}: {e}")),
        )
        .unwrap();
        let rows = parse_and_execute(&source, &dataset, &binds)
            .unwrap_or_else(|e| panic!("{file:?} failed to execute: {e}"));
        assert_eq!(
            Value::Array(rows),
            expected,
            "unexpected results for {file:?}"
        );
    }
}
