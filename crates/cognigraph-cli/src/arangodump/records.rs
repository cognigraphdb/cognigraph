//! Stream one collection's data files line by line within the limits.
//!
//! No line is read past `max_record_bytes + 1`, and the decompressed total is
//! counted against `max_expanded_bytes` across the whole dump. Each accepted
//! document is handed to `sink` without `_id` and `_rev`.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use flate2::read::MultiGzDecoder;
use serde_json::Value;

use super::options::Limits;
use super::report::Problem;

const MARKER_DOCUMENT: u64 = 2300;

pub(crate) struct Collection<'a> {
    pub name: &'a str,
    pub edge: bool,
    pub envelope: bool,
}

/// Running totals shared by every collection of one dump.
#[derive(Default)]
pub(crate) struct Budget {
    pub expanded: u64,
    pub revisions: u64,
}

fn reader(path: &Path) -> std::io::Result<Box<dyn BufRead>> {
    let file = File::open(path)?;
    Ok(if path.extension().is_some_and(|e| e == "gz") {
        Box::new(BufReader::new(MultiGzDecoder::new(BufReader::new(file))))
    } else {
        Box::new(BufReader::new(file))
    })
}

fn document(
    collection: &Collection<'_>,
    record: Value,
    file: &str,
    line: u64,
    seen: &mut HashSet<String>,
) -> Result<(String, Value), Problem> {
    let Value::Object(mut object) = record else {
        return Err(Problem::new("invalid_document").file(file).line(line));
    };
    let key = match object.get("_key") {
        Some(Value::String(key)) if !key.is_empty() => key.clone(),
        _ => return Err(Problem::new("invalid_document").file(file).line(line)),
    };
    let problem = |code| Problem::new(code).collection(collection.name).key(&key);
    // Same order as the reference reader: key, duplicate, identity, edge shape.
    if !seen.insert(key.clone()) {
        return Err(problem("duplicate_key"));
    }
    if let Some(id) = object.remove("_id")
        && id.as_str() != Some(&format!("{}/{key}", collection.name))
    {
        return Err(problem("identity_mismatch"));
    }
    if collection.edge
        && !["_from", "_to"].iter().all(|end| {
            object
                .get(*end)
                .and_then(Value::as_str)
                .is_some_and(|v| v.contains('/'))
        })
    {
        return Err(problem("invalid_edge"));
    }
    object.remove("_rev");
    Ok((key, Value::Object(object)))
}

/// Read every part of one collection in order. `seen` holds the collection's
/// keys so a repeated key is refused at the line where it repeats.
pub(crate) fn read(
    directory: &Path,
    files: &[String],
    collection: &Collection<'_>,
    limits: &Limits,
    budget: &mut Budget,
    mut sink: impl FnMut(String, Value) -> Result<(), Problem>,
) -> Result<(), Problem> {
    let mut seen = HashSet::new();
    for file in files {
        let corrupt = |e: std::io::Error| {
            Problem::new("corrupt_data_file")
                .file(file)
                .detail(e.kind().to_string())
        };
        let mut input = reader(&directory.join(file)).map_err(corrupt)?;
        let mut buffer = Vec::new();
        let mut line = 0u64;
        loop {
            buffer.clear();
            let read = (&mut input)
                .take(limits.max_record_bytes + 1)
                .read_until(b'\n', &mut buffer)
                .map_err(corrupt)? as u64;
            if read == 0 {
                break;
            }
            line += 1;
            budget.expanded += read;
            if read > limits.max_record_bytes {
                return Err(Problem::new("record_too_large").file(file).line(line));
            }
            if budget.expanded > limits.max_expanded_bytes {
                return Err(Problem::new("dump_too_large").file(file));
            }
            if buffer.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let mut record: Value = serde_json::from_slice(&buffer)
                .map_err(|_| Problem::new("corrupt_data_file").file(file).line(line))?;
            if collection.envelope {
                if record.get("type").and_then(Value::as_u64) != Some(MARKER_DOCUMENT) {
                    return Err(Problem::new("unsupported_marker").file(file).line(line));
                }
                record = record
                    .get_mut("data")
                    .map(Value::take)
                    .unwrap_or(Value::Null);
            }
            let has_revision = record.get("_rev").is_some();
            let (key, doc) = document(collection, record, file, line, &mut seen)?;
            sink(key, doc)?;
            budget.revisions += u64::from(has_revision);
        }
    }
    Ok(())
}
