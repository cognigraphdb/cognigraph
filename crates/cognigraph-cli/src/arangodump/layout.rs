//! Classify a dump directory before any data is read (contract "Layout rules").

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::options::Limits;
use super::report::Problem;

#[derive(Debug, Default)]
pub(crate) struct Layout {
    pub metadata: Value,
    /// Collection file stem (`<name>_<md5>`) to its structure file name.
    pub structures: BTreeMap<String, String>,
    /// Stem to its data files in part order; unsplit files use part `-1`.
    pub data: BTreeMap<String, Vec<(i64, String)>>,
    pub views: Vec<String>,
    pub ignored: Vec<String>,
    /// Every file that is read, in name order.
    pub files: Vec<String>,
}

impl Layout {
    pub fn split(&self) -> bool {
        self.data.values().flatten().any(|(part, _)| *part >= 0)
    }
}

fn stem(value: &str) -> bool {
    value.len() > 33
        && value.as_bytes()[value.len() - 33] == b'_'
        && value[value.len() - 32..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `(stem, part)` of a data file name with the given extension.
fn data_name(name: &str, extension: &str) -> Option<(String, i64)> {
    let base = name.strip_suffix(".gz").unwrap_or(name);
    let prefix = base.strip_suffix(extension)?;
    if stem(prefix) {
        return Some((prefix.to_string(), -1));
    }
    let (head, part) = prefix.rsplit_once('.')?;
    (stem(head) && !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        .then(|| (head.to_string(), part.parse().unwrap_or(i64::MAX)))
}

pub(crate) fn classify(directory: &Path, limits: &Limits) -> Result<Layout, Problem> {
    let mut entries: Vec<(String, PathBuf)> = fs::read_dir(directory)
        .map_err(|e| Problem::new("unknown_layout").detail(e.to_string()))?
        .map(|entry| entry.map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())))
        .collect::<Result<_, _>>()
        .map_err(|e| Problem::new("unknown_layout").detail(e.to_string()))?;
    entries.sort();
    if entries.len() > limits.max_files {
        return Err(Problem::new("dump_too_large").detail("file count"));
    }
    for (name, path) in &entries {
        let meta = fs::symlink_metadata(path).map_err(|e| {
            Problem::new("unknown_layout")
                .file(name)
                .detail(e.to_string())
        })?;
        if meta.file_type().is_symlink() {
            return Err(Problem::new("unsafe_path").file(name));
        }
        if meta.is_dir() {
            let code = if path.join("dump.json").exists() {
                "multiple_databases"
            } else {
                "unknown_layout"
            };
            return Err(Problem::new(code).file(name));
        }
    }
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    if names.contains(&"ENCRYPTION") {
        let marker = fs::read(directory.join("ENCRYPTION")).unwrap_or_default();
        if String::from_utf8_lossy(&marker).trim() != "none" {
            return Err(Problem::new("encrypted_unsupported"));
        }
    }
    if !names.contains(&"dump.json") {
        return Err(Problem::new("missing_dump_metadata"));
    }
    let metadata: Value = fs::read(directory.join("dump.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .filter(Value::is_object)
        .ok_or_else(|| Problem::new("corrupt_dump_metadata"))?;
    let vpack_file = names.iter().any(|n| data_name(n, ".data.vpack").is_some());
    if metadata.get("useVPack").and_then(Value::as_bool) == Some(true) || vpack_file {
        return Err(Problem::new("vpack_unsupported"));
    }
    let mut layout = Layout {
        metadata,
        ..Default::default()
    };
    for name in names {
        if name == "dump.json" || name == "ENCRYPTION" {
            layout.files.push(name.to_string());
        } else if let Some(prefix) = name.strip_suffix(".structure.json").filter(|p| stem(p)) {
            layout
                .structures
                .insert(prefix.to_string(), name.to_string());
            layout.files.push(name.to_string());
        } else if let Some((prefix, part)) = data_name(name, ".data.json") {
            layout
                .data
                .entry(prefix)
                .or_default()
                .push((part, name.to_string()));
            layout.files.push(name.to_string());
        } else if let Some(view) = name.strip_suffix(".view.json").filter(|v| !v.is_empty()) {
            layout.views.push(view.to_string());
            layout.files.push(name.to_string());
        } else if name.starts_with('.') {
            layout.ignored.push(name.to_string());
        } else {
            return Err(Problem::new("unknown_layout").file(name));
        }
    }
    for parts in layout.data.values_mut() {
        parts.sort();
    }
    if let Some(orphan) = layout
        .data
        .keys()
        .find(|s| !layout.structures.contains_key(*s))
    {
        return Err(
            Problem::new("unknown_layout").detail(format!("data without structure: {orphan}"))
        );
    }
    Ok(layout)
}
