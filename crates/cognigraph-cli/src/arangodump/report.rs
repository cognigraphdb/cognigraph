//! The `cognigraph-arangodump-report-v1` document (never document bodies).

use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "cognigraph-arangodump-report-v1";

/// A contract error; codes are listed in docs/reference/arangodump-import.md.
/// Context strings are boxed to keep `Result<_, Problem>` small.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<Box<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Box<str>>,
}

impl Problem {
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            collection: None,
            key: None,
            file: None,
            line: None,
            detail: None,
        }
    }
    pub fn collection(mut self, name: &str) -> Self {
        self.collection = Some(name.into());
        self
    }
    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn file(mut self, file: &str) -> Self {
        self.file = Some(file.into());
        self
    }
    pub fn line(mut self, line: u64) -> Self {
        self.line = Some(line);
        self
    }
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into().into_boxed_str());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Constraint {
    pub collection: String,
    pub name: String,
    pub unique: bool,
    pub fields: Vec<String>,
    pub sparse: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotCarried {
    pub collection: Option<String>,
    pub kind: &'static str,
    pub detail: Option<String>,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CollectionSummary {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub documents: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub importer: String,
    pub mode: &'static str,
    pub status: &'static str,
    pub source: String,
    pub database: Option<String>,
    pub destination: Option<String>,
    pub collections: BTreeMap<String, CollectionSummary>,
    pub constraints: Vec<Constraint>,
    pub not_carried: Vec<NotCarried>,
    pub excluded: Vec<Value>,
    pub ignored: Vec<String>,
    pub warnings: Vec<&'static str>,
    pub dropped: Map<String, Value>,
    pub errors: Vec<Problem>,
    pub files: Vec<SourceFile>,
}

impl Report {
    pub fn new(source: &str, dry_run: bool) -> Self {
        let mut dropped = Map::new();
        dropped.insert("_rev".into(), json!(0));
        Self {
            schema: SCHEMA,
            importer: env!("CARGO_PKG_VERSION").to_string(),
            mode: if dry_run { "dry_run" } else { "import" },
            status: "accepted",
            source: source.to_string(),
            dropped,
            ..Default::default()
        }
    }

    pub fn reject(mut self, problem: Problem) -> Self {
        self.errors.push(problem);
        self.status = "rejected";
        self
    }

    pub fn add_dropped_rev(&mut self, count: u64) {
        let current = self.dropped["_rev"].as_u64().unwrap_or(0);
        self.dropped.insert("_rev".into(), json!(current + count));
    }
}
