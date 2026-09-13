//! Tantivy-backed BM25 text index: a lazily built, in-RAM, rebuildable
//! derivative of the collection (same pattern as the quantized vector
//! index — any write invalidates via the backend's write version).

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use cognigraph_core::{CogniGraphError, Result};
use serde_json::Value;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, STORED, STRING, Schema, TEXT, Value as _};
use tantivy::{Index, IndexReader, TantivyDocument, Term};

fn index_err(e: impl std::fmt::Display) -> CogniGraphError {
    CogniGraphError::BackendError(format!("text index: {e}"))
}

/// Validate Native BM25 fields before schema construction or cache lookup.
/// `_key` is reserved for the index's internal document identity.
pub fn validate_text_search_fields(fields: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    for field in fields {
        if field == "_key" || !seen.insert(field) {
            return Err(CogniGraphError::ValidationError(format!(
                "text search field `{field}` is reserved or duplicated"
            )));
        }
    }
    Ok(())
}

pub(crate) fn index_identity(
    database_id: Option<&str>,
    collection: &str,
    fields: &[String],
) -> String {
    serde_json::to_string(&(database_id, collection, fields)).expect("string tuple is serializable")
}

pub(crate) struct TextIndex {
    pub(crate) version: String,
    reader: IndexReader,
    key_field: Field,
    text_fields: Vec<Field>,
}

fn build_schema(fields: &[String]) -> (Schema, Field, Vec<Field>) {
    let mut schema_builder = Schema::builder();
    let key_field = schema_builder.add_text_field("_key", STRING | STORED);
    let text_fields: Vec<Field> = fields
        .iter()
        .map(|name| schema_builder.add_text_field(name, TEXT))
        .collect();
    (schema_builder.build(), key_field, text_fields)
}

impl TextIndex {
    /// Open a persistent index directory if its generation stamp matches
    /// the live data (instant warm start after restart).
    pub(crate) fn open_if_current(
        dir: &Path,
        fields: &[String],
        version: &str,
        identity: &str,
    ) -> Option<Self> {
        validate_text_search_fields(fields).ok()?;
        if std::fs::read_to_string(dir.join("IDENTITY")).ok()? != identity {
            return None;
        }
        let stamp = std::fs::read_to_string(dir.join("REVISION")).ok()?;
        if stamp != version {
            return None;
        }
        let index = Index::open_in_dir(dir).ok()?;
        let schema = index.schema();
        if schema != build_schema(fields).0 {
            return None;
        }
        let key_field = schema.get_field("_key").ok()?;
        let text_fields: Vec<Field> = fields
            .iter()
            .map(|name| schema.get_field(name))
            .collect::<std::result::Result<_, _>>()
            .ok()?;
        let reader = index.reader().ok()?;
        Some(Self {
            version: version.to_string(),
            reader,
            key_field,
            text_fields,
        })
    }

    /// Build an index over the given fields of a collection: in RAM, or in
    /// a persistent directory (recreated from scratch — it is a
    /// rebuildable derivative, generation-stamped for warm starts).
    pub(crate) fn build(
        docs: &BTreeMap<String, Value>,
        fields: &[String],
        version: &str,
        dir: Option<&Path>,
        identity: &str,
    ) -> Result<Self> {
        validate_text_search_fields(fields)?;
        let (schema, key_field, text_fields) = build_schema(fields);
        let index = match dir {
            Some(dir) => {
                let _ = std::fs::remove_dir_all(dir);
                std::fs::create_dir_all(dir).map_err(index_err)?;
                Index::create_in_dir(dir, schema).map_err(index_err)?
            }
            None => Index::create_in_ram(schema),
        };
        let mut writer = index.writer(30_000_000).map_err(index_err)?;
        for (key, doc) in docs {
            let mut entry = TantivyDocument::default();
            entry.add_text(key_field, key);
            let mut any = false;
            for (field, name) in text_fields.iter().zip(fields) {
                if let Some(text) = doc.get(name).and_then(Value::as_str) {
                    entry.add_text(*field, text);
                    any = true;
                }
            }
            if any {
                writer.add_document(entry).map_err(index_err)?;
            }
        }
        writer.commit().map_err(index_err)?;
        if let Some(dir) = dir {
            std::fs::write(dir.join("IDENTITY"), identity).map_err(index_err)?;
            std::fs::write(dir.join("REVISION"), version).map_err(index_err)?;
        }
        let reader = index.reader().map_err(index_err)?;
        Ok(Self {
            version: version.to_string(),
            reader,
            key_field,
            text_fields,
        })
    }

    /// BM25 search: OR of every (field, token) term. Returns
    /// (score, key) best-first with a deterministic key tie-break.
    pub(crate) fn search(&self, query: &str, limit: usize) -> Result<Vec<(f64, String)>> {
        let tokens: Vec<String> = query
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let clauses: Vec<(Occur, Box<dyn Query>)> = self
            .text_fields
            .iter()
            .flat_map(|field| {
                tokens.iter().map(|token| {
                    (
                        Occur::Should,
                        Box::new(TermQuery::new(
                            Term::from_field_text(*field, token),
                            IndexRecordOption::WithFreqs,
                        )) as Box<dyn Query>,
                    )
                })
            })
            .collect();
        let searcher = self.reader.searcher();
        let top = searcher
            .search(
                &BooleanQuery::new(clauses),
                &TopDocs::with_limit(limit.max(1)).order_by_score(),
            )
            .map_err(index_err)?;
        let mut hits: Vec<(f64, String)> = top
            .into_iter()
            .filter_map(|(score, address)| {
                let doc: TantivyDocument = searcher.doc(address).ok()?;
                let key = doc
                    .get_first(self.key_field)
                    .and_then(|v| v.as_str())
                    .map(str::to_string)?;
                Some((f64::from(score), key))
            })
            .collect();
        hits.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_open_requires_exact_identity_schema_and_generation() {
        let dir = std::env::temp_dir().join(format!("cognigraph-index-{}", uuid::Uuid::new_v4()));
        let fields = vec!["a".into(), "b".into()];
        let identity = index_identity(Some("database-one"), "docs", &fields);
        let docs = BTreeMap::from([("one".into(), serde_json::json!({"a": "rust"}))]);
        drop(TextIndex::build(&docs, &fields, "revision-one", Some(&dir), &identity).unwrap());
        assert!(TextIndex::open_if_current(&dir, &fields, "revision-one", &identity).is_some());
        assert!(TextIndex::open_if_current(&dir, &fields, "revision-two", &identity).is_none());
        assert!(
            TextIndex::open_if_current(&dir, &fields, "revision-one", "different collection")
                .is_none()
        );
        assert!(
            TextIndex::open_if_current(
                &dir,
                &fields,
                "revision-one",
                &index_identity(Some("database-two"), "docs", &fields)
            )
            .is_none()
        );
        // Merely finding the requested field in an old schema is insufficient.
        assert!(
            TextIndex::open_if_current(&dir, &["a".into()], "revision-one", &identity).is_none()
        );
        std::fs::remove_file(dir.join("REVISION")).unwrap();
        std::fs::write(dir.join("GENERATION"), "1").unwrap();
        assert!(TextIndex::open_if_current(&dir, &fields, "revision-one", &identity).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
