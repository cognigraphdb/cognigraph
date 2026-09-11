//! Model filtering precedes candidate selection; duplicate parents trigger
//! larger candidate windows instead of silently underfilling the result limit.

use std::collections::HashMap;

use cognigraph_core::{Result, SearchHit, VectorSearchOpts};
use serde_json::json;
use tracing::debug;

use super::{ArangoBackend, VectorSearchMode, map_err};

impl ArangoBackend {
    pub(super) async fn search_vectors(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        if opts.limit == 0 {
            return Ok(Vec::new());
        }
        // ArangoDB >= 3.12.6 supports a single pre-filter during the ANN
        // lookup. Filtering after LIMIT can lose every requested-model hit.
        let model_filter = if opts.model_name.is_some() {
            "FILTER embed.model_name == @model_name"
        } else {
            ""
        };
        let aql = match self.vector_mode {
            VectorSearchMode::Native => format!(
                r#"
                LET candidates = (
                    FOR embed IN @@collection
                        {model_filter}
                        LET score = APPROX_NEAR_COSINE(embed.embedding, @query_vector)
                        SORT score DESC
                        LIMIT @fetch_limit
                        RETURN MERGE(embed, {{ score: score }})
                )
                FOR c IN candidates
                    FILTER c.score >= @threshold
                    RETURN c
                "#
            ),
            VectorSearchMode::Fallback => format!(
                r#"
                FOR embed IN @@collection
                    {model_filter}
                    LET score = COSINE_SIMILARITY(embed.embedding, @query_vector)
                    FILTER score >= @threshold
                    SORT score DESC
                    LIMIT @fetch_limit
                    RETURN MERGE(embed, {{ score: score }})
                "#
            ),
        };

        let mut bind_vars = HashMap::from([
            ("@collection".into(), json!(collection)),
            ("query_vector".into(), json!(query_vector)),
            ("threshold".into(), json!(opts.threshold.unwrap_or(-1.0))),
        ]);
        if let Some(model) = &opts.model_name {
            bind_vars.insert("model_name".into(), json!(model));
        }

        let mut fetch_limit = opts.limit;
        let mut seen: HashMap<String, SearchHit> = HashMap::new();
        loop {
            bind_vars.insert("fetch_limit".into(), json!(fetch_limit));
            let rows = self
                .client
                .query(&aql, bind_vars.clone())
                .await
                .map_err(map_err)?;
            let candidate_count = rows.len();
            debug!(
                collection,
                fetch_limit, candidate_count, "Vector candidates"
            );
            for row in rows {
                // Embedding chunks share their string parent handle. Ordinary
                // rows (including nonstring parent values) use their own _id.
                let doc_id = row
                    .get("document_id")
                    .and_then(|v| v.as_str())
                    .or_else(|| row.get("_id").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                let score = row.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let hit = SearchHit {
                    document: row,
                    score,
                    source: Some("vector_search".into()),
                };
                seen.entry(doc_id)
                    .and_modify(|best| {
                        if score > best.score {
                            *best = hit.clone();
                        }
                    })
                    .or_insert(hit);
            }

            // Sorted candidates below the threshold cannot make a later
            // window eligible. ANN may exhaust its searched cells before
            // the whole collection; its configured recall remains unchanged.
            if seen.len() >= opts.limit || candidate_count < fetch_limit {
                break;
            }
            let next_limit = fetch_limit.saturating_mul(2);
            if next_limit == fetch_limit {
                break;
            }
            fetch_limit = next_limit;
        }

        let mut hits: Vec<_> = seen.into_values().collect();
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(opts.limit);
        Ok(hits)
    }
}
