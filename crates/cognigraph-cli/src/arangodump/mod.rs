//! Offline ArangoDB dump import into a new Native store (CG-66).
//!
//! Implements docs/reference/arangodump-import.md and is qualified against
//! fixtures/arangodump with the same expectations as the reference reader
//! (scripts/arangodump_reader.py). Nothing is ever written outside this
//! invocation's staging directory until publication links a finished store
//! to a path that did not exist.

mod layout;
mod load;
mod names;
pub(crate) mod options;
mod publish;
mod records;
mod report;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use load::{Failure, Loader};
pub use options::Options;
use records::{Budget, Collection};
use report::{CollectionSummary, Constraint, NotCarried, Problem, Report};

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_REJECTED: i32 = 3;
pub const EXIT_FAILED: i32 = 4;
pub const EXIT_REPORT_FAILED: i32 = 5;

const IMPLICIT_INDEXES: [&str; 2] = ["primary", "edge"];
const UNIQUE_TYPES: [&str; 3] = ["persistent", "hash", "skiplist"];

/// Points where tests make the importer fail as a crash or disk error would.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum Stage {
    #[default]
    Never,
    Staged,
    /// Append to the source's dump.json after staging, as a concurrent writer would.
    ChangeSource,
    Verified,
    Report,
}

pub(crate) struct Outcome {
    pub code: i32,
    pub report: Option<Report>,
    pub message: Option<String>,
}

/// Run from `main`: prints the report and returns the process exit code.
pub fn run_cli(options: &Options) -> i32 {
    let outcome = import(options, Stage::Never);
    if let Some(report) = &outcome.report {
        println!(
            "{}",
            serde_json::to_string_pretty(report).unwrap_or_default()
        );
    }
    if let Some(message) = &outcome.message {
        eprintln!("error: {message}");
    }
    outcome.code
}

fn dispositions(name: &str, edge: bool, indexes: &[Value]) -> (Vec<Constraint>, Vec<NotCarried>) {
    let mut carried = Vec::new();
    let mut reported = Vec::new();
    for index in indexes {
        let kind = index
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if IMPLICIT_INDEXES.contains(&kind) {
            continue;
        }
        let fields: Vec<String> = index
            .get("fields")
            .and_then(Value::as_array)
            .map(|f| {
                f.iter()
                    .map(|v| v.as_str().map(str::to_string))
                    .collect::<Option<_>>()
            })
            .unwrap_or(Some(Vec::new()))
            .unwrap_or_default();
        let plain = !fields.is_empty()
            && fields.len() == index["fields"].as_array().map_or(0, Vec::len)
            && fields.iter().all(|f| !f.contains("[*]"));
        let unique = index.get("unique").and_then(Value::as_bool) == Some(true);
        let label = index
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let reason = match (UNIQUE_TYPES.contains(&kind) && plain, unique, edge) {
            (true, true, true) => "edge_unique_unsupported",
            (true, true, false) => {
                let sparse = index.get("sparse").and_then(Value::as_bool) == Some(true);
                let index_name = label
                    .filter(|n| !n.is_empty() && !n.contains('/') && !n.contains('\0'))
                    .unwrap_or_else(|| format!("{}_unique", fields.join("_")));
                carried.push(Constraint {
                    collection: name.to_string(),
                    name: index_name,
                    unique: true,
                    fields,
                    sparse,
                });
                continue;
            }
            (true, false, _) => "non_unique_index",
            _ => "unsupported_index_type",
        };
        reported.push(NotCarried {
            collection: Some(name.to_string()),
            kind: "index",
            detail: label,
            reason,
        });
    }
    (carried, reported)
}

fn metadata_not_carried(name: &str, parameters: &Value) -> Vec<NotCarried> {
    let mut reported = Vec::new();
    let item = |kind, detail: &str| NotCarried {
        collection: Some(name.to_string()),
        kind,
        detail: Some(detail.to_string()),
        reason: "unsupported_metadata",
    };
    if parameters
        .get("schema")
        .is_some_and(|v| !v.is_null() && v != &json!({}))
    {
        reported.push(item("schema", "validation rule"));
    }
    if parameters
        .get("computedValues")
        .is_some_and(|v| !v.is_null() && v != &json!([]))
    {
        reported.push(item("computed_values", "computedValues"));
    }
    let generator = parameters
        .pointer("/keyOptions/type")
        .and_then(Value::as_str)
        .unwrap_or("traditional");
    if generator != "traditional" {
        reported.push(item("key_generator", generator));
    }
    reported
}

fn finish(
    options: &Options,
    mut report: Report,
    code: i32,
    fail: Stage,
    message: Option<String>,
) -> Outcome {
    let Some(path) = &options.report else {
        return Outcome {
            code,
            report: Some(report),
            message,
        };
    };
    report.destination =
        (report.status == "published").then(|| options.output.display().to_string());
    let body = serde_json::to_vec_pretty(&report).unwrap_or_default();
    let written = if fail == Stage::Report {
        Err(std::io::Error::other("injected report failure"))
    } else {
        publish::write_report(path, &body)
    };
    match written {
        Ok(()) => Outcome {
            code,
            report: Some(report),
            message,
        },
        Err(e) if report.status == "published" => Outcome {
            code: EXIT_REPORT_FAILED,
            report: Some(report),
            message: Some(format!(
                "the store at {} is complete, but the report {} could not be written: {e}",
                options.output.display(),
                path.display()
            )),
        },
        Err(e) => Outcome {
            code: EXIT_FAILED,
            report: Some(report),
            message: Some(format!(
                "report {} could not be written: {e}",
                path.display()
            )),
        },
    }
}

fn failed(options: &Options, mut report: Report, fail: Stage, message: String) -> Outcome {
    report.status = "failed";
    finish(options, report, EXIT_FAILED, fail, Some(message))
}

pub(crate) fn import(options: &Options, fail: Stage) -> Outcome {
    if let Err(message) =
        publish::check_paths(&options.dump, &options.output, options.report.as_deref())
    {
        return Outcome {
            code: EXIT_USAGE,
            report: None,
            message: Some(message),
        };
    }
    let dump = options.dump.as_path();
    let mut report = Report::new(&dump.display().to_string(), options.dry_run);
    let layout = match layout::classify(dump, &options.limits) {
        Ok(layout) => layout,
        Err(problem) => return finish(options, report.reject(problem), EXIT_REJECTED, fail, None),
    };
    report.database = layout
        .metadata
        .get("database")
        .and_then(Value::as_str)
        .map(str::to_string);
    report.ignored = layout.ignored.clone();
    report.files = match publish::fingerprint(dump, &layout.files) {
        Ok(files) => files,
        Err(e) => return failed(options, report, fail, format!("read dump: {e}")),
    };
    for view in &layout.views {
        report.not_carried.push(NotCarried {
            collection: None,
            kind: "view",
            detail: Some(view.clone()),
            reason: "unsupported_metadata",
        });
    }
    let staging = match publish::Staging::create(&options.output, options.dry_run) {
        Ok(staging) => staging,
        Err(e) => {
            return failed(
                options,
                report,
                fail,
                format!("create staging directory: {e}"),
            );
        }
    };
    let loader = match Loader::open(&staging.store()) {
        Ok(loader) => loader,
        Err(e) => return failed(options, report, fail, format!("open staging store: {e}")),
    };
    let envelope = layout.metadata.get("useEnvelope").and_then(Value::as_bool) == Some(true);
    let (mut loaded, mut failed_names, mut edges) = (BTreeSet::new(), BTreeSet::new(), Vec::new());
    let mut budget = Budget::default();
    for (stem, structure_file) in &layout.structures {
        let structure: Option<Value> = fs::read(dump.join(structure_file))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let parsed = structure.as_ref().and_then(|s| {
            let parameters = s.get("parameters")?;
            let name = parameters.get("name")?.as_str()?;
            let edge = match parameters.get("type")?.as_u64()? {
                2 => false,
                3 => true,
                _ => return None,
            };
            Some((parameters, name, edge))
        });
        let Some((parameters, name, edge)) = parsed else {
            report
                .errors
                .push(Problem::new("corrupt_structure_file").file(structure_file));
            continue;
        };
        if name.starts_with('_')
            || parameters.get("isSystem").and_then(Value::as_bool) == Some(true)
        {
            report
                .excluded
                .push(json!({"collection": name, "reason": "system_collection"}));
            continue;
        }
        if let Some(problem) = names::check(name) {
            report.errors.push(problem);
            continue;
        }
        report
            .not_carried
            .extend(metadata_not_carried(name, parameters));
        let parts: Vec<String> = layout
            .data
            .get(stem)
            .into_iter()
            .flatten()
            .map(|(_, f)| f.clone())
            .collect();
        if parts.is_empty() && !layout.split() {
            report
                .errors
                .push(Problem::new("missing_data_file").collection(name));
            failed_names.insert(name.to_string());
            continue;
        }
        let indexes = structure
            .as_ref()
            .and_then(|s| s.get("indexes"))
            .and_then(Value::as_array);
        let (constraints, not_carried) =
            dispositions(name, edge, indexes.map_or(&[][..], Vec::as_slice));
        let collection = Collection {
            name,
            edge,
            envelope,
        };
        match loader.load(
            dump,
            &parts,
            &collection,
            &constraints,
            &options.limits,
            &mut budget,
        ) {
            Ok(count) => {
                let kind = if edge { "edge" } else { "document" };
                report.collections.insert(
                    name.to_string(),
                    CollectionSummary {
                        kind,
                        documents: count,
                    },
                );
                report.not_carried.extend(not_carried);
                report.constraints.extend(constraints);
                loaded.insert(name.to_string());
                if edge {
                    edges.push(name.to_string());
                }
            }
            Err(Failure::Rejected(problem)) => {
                report.errors.push(problem);
                failed_names.insert(name.to_string());
            }
            Err(Failure::Internal(e)) => return failed(options, report, fail, e),
        }
    }
    match loader.resolve(&edges, &loaded, &failed_names) {
        Ok(problems) => report.errors.extend(problems),
        Err(Failure::Internal(e)) => return failed(options, report, fail, e),
        Err(Failure::Rejected(problem)) => report.errors.push(problem),
    }
    report.add_dropped_rev(budget.revisions);
    if !report.errors.is_empty() {
        report.status = "rejected";
        drop(loader);
        drop(staging);
        return finish(options, report, EXIT_REJECTED, fail, None);
    }
    if !report.collections.is_empty() && report.collections.values().all(|c| c.documents == 0) {
        report.warnings.push("no_documents");
    }
    if fail == Stage::Staged {
        return failed(
            options,
            report,
            fail,
            "injected failure after staging".into(),
        );
    }
    if fail == Stage::ChangeSource {
        use std::io::Write;
        let _ = fs::OpenOptions::new()
            .append(true)
            .open(dump.join("dump.json"))
            .and_then(|mut f| f.write_all(b"\n"));
    }
    if options.dry_run {
        return finish(options, report, EXIT_OK, fail, None);
    }
    match publish::fingerprint(dump, &layout.files) {
        Ok(again)
            if again == report.files && layout_unchanged(dump, &layout.files, &layout.ignored) => {}
        Ok(_) => {
            report.errors.push(Problem::new("source_changed"));
            return failed(
                options,
                report,
                fail,
                "the dump changed during the import".into(),
            );
        }
        Err(e) => return failed(options, report, fail, format!("re-read dump: {e}")),
    }
    if fail == Stage::Verified {
        return failed(
            options,
            report,
            fail,
            "injected failure before publication".into(),
        );
    }
    let store = staging.store();
    drop(loader);
    if let Err(e) = publish::publish(&store, &options.output) {
        return failed(
            options,
            report,
            fail,
            format!("publish {}: {e}", options.output.display()),
        );
    }
    drop(staging);
    report.status = "published";
    finish(options, report, EXIT_OK, fail, None)
}

fn layout_unchanged(dump: &Path, files: &[String], ignored: &[String]) -> bool {
    let Ok(entries) = fs::read_dir(dump) else {
        return false;
    };
    let mut now: Vec<String> = entries
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|name| !ignored.contains(name) && !name.starts_with('.'))
        .collect();
    now.sort();
    let mut before = files.to_vec();
    before.sort();
    now == before
}

#[cfg(test)]
#[path = "fixtures_tests.rs"]
mod fixtures_tests;

#[cfg(test)]
#[path = "publication_tests.rs"]
mod publication_tests;
