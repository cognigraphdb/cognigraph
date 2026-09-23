//! Side views as a proposal source (CG-88).
//!
//! `POST /api/construct/propose` with `side_views` reads an explicit
//! selection of stored side views (every side view of one source collection,
//! optionally narrowed to some of its documents), runs the read-only
//! detector from `cognigraph-construct` against the space's fact graph and
//! returns the candidate gaps. With `dry_run` that report is the whole
//! response and no provider is needed; otherwise the candidates become the
//! gaps of the ordinary proposal path, and each stored proposal records which
//! side views supported it. Side views stay quarantined: this module only
//! reads `side_views` and `facts`.
//! See `docs/decisions/decision_sideview_gap_detector.md`.

use super::*;
use crate::system_collections::SIDE_VIEWS_COLLECTION;
use cognigraph_construct::{GapReport, SideViewText, detect_sideview_gaps};
use cognigraph_core::GraphBackend;
use std::collections::{BTreeMap, BTreeSet};

pub(super) const DEFAULT_MAX_CANDIDATES: usize = 20;
pub(super) const MAX_CANDIDATES_LIMIT: usize = 100;
pub(super) const MAX_SELECTED_DOCUMENTS: usize = 1_000;
pub(super) const MAX_SCANNED_SIDE_VIEWS: usize = 10_000;

/// Which side views to scan and how strictly.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SideViewSelection {
    /// Source collection whose documents the side views were generated from.
    pub(super) collection: String,
    /// Optional document keys in `collection`; all of them when absent.
    #[serde(default)]
    pub(super) documents: Option<Vec<String>>,
    /// Distinct side views a candidate needs (default 1).
    #[serde(default)]
    pub(super) min_support: Option<usize>,
    /// Candidates handed to the proposer, strongest first (default 20).
    #[serde(default)]
    pub(super) max_candidates: Option<usize>,
    /// Report the candidates without proposing anything.
    #[serde(default)]
    pub(super) dry_run: bool,
}

fn invalid(message: String) -> AppError {
    AppError(CogniGraphError::ValidationError(message))
}

impl SideViewSelection {
    pub(super) fn validate(&self) -> Result<(), AppError> {
        if self.collection.is_empty() || self.collection.contains('/') {
            return Err(invalid(
                "side_views.collection must be a collection name without `/`".into(),
            ));
        }
        if self.min_support == Some(0) {
            return Err(invalid("side_views.min_support must be at least 1".into()));
        }
        if let Some(max) = self.max_candidates
            && !(1..=MAX_CANDIDATES_LIMIT).contains(&max)
        {
            return Err(invalid(format!(
                "side_views.max_candidates must be between 1 and {MAX_CANDIDATES_LIMIT}"
            )));
        }
        if let Some(documents) = &self.documents {
            if documents.is_empty() || documents.len() > MAX_SELECTED_DOCUMENTS {
                return Err(invalid(format!(
                    "side_views.documents must list 1 to {MAX_SELECTED_DOCUMENTS} document keys \
                     (omit it to scan the whole collection)"
                )));
            }
            if documents
                .iter()
                .any(|key| key.is_empty() || key.contains('/'))
            {
                return Err(invalid(
                    "side_views.documents must be document keys without `/`".into(),
                ));
            }
        }
        Ok(())
    }
}

/// Candidate gaps, the side views behind each, and the report to return.
pub(super) struct SideViewGaps {
    pub(super) gaps: Vec<Fact>,
    pub(super) support: BTreeMap<(String, String, String), Vec<String>>,
    pub(super) report: Value,
}

fn triple(fact: &Fact) -> (String, String, String) {
    (
        fact.source.clone(),
        fact.relation.clone(),
        fact.target.clone(),
    )
}

fn arrow(fact: &Fact) -> String {
    format!("{} --{}--> {}", fact.source, fact.relation, fact.target)
}

async fn selected_side_views(
    backend: &dyn GraphBackend,
    selection: &SideViewSelection,
) -> Result<Vec<SideViewText>, AppError> {
    let exists = backend
        .list_collections()
        .await?
        .iter()
        .any(|info| info.name == SIDE_VIEWS_COLLECTION);
    if !exists {
        return Ok(Vec::new());
    }
    let parents: Option<BTreeSet<String>> = selection.documents.as_ref().map(|keys| {
        keys.iter()
            .map(|key| format!("{}/{key}", selection.collection))
            .collect()
    });
    let prefix = format!("{}/", selection.collection);
    let fields = ["_key", "document_id", "question", "answer"].map(String::from);
    let text = |row: &Value, field: &str| {
        row.get(field)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut views: Vec<SideViewText> = backend
        .list_documents_projected(SIDE_VIEWS_COLLECTION, &fields, None, None)
        .await?
        .into_iter()
        .filter(|row| {
            row.get("document_id")
                .and_then(Value::as_str)
                .is_some_and(|parent| match &parents {
                    Some(parents) => parents.contains(parent),
                    None => parent.starts_with(&prefix),
                })
        })
        .map(|row| SideViewText {
            id: text(&row, "_key"),
            question: text(&row, "question"),
            answer: text(&row, "answer"),
        })
        .collect();
    if views.len() > MAX_SCANNED_SIDE_VIEWS {
        return Err(invalid(format!(
            "the selection covers {} side views; the limit is {MAX_SCANNED_SIDE_VIEWS} \
             (narrow it with side_views.documents)",
            views.len()
        )));
    }
    views.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(views)
}

/// Run the detector over the selection and cap the candidates.
pub(super) async fn side_view_gaps(
    backend: &dyn GraphBackend,
    space: &SpaceType,
    space_id: &str,
    selection: &SideViewSelection,
) -> Result<SideViewGaps, AppError> {
    let views = selected_side_views(backend, selection).await?;
    let min_support = selection.min_support.unwrap_or(1);
    let GapReport {
        scanned,
        mut candidates,
        skipped,
    } = detect_sideview_gaps(backend, space, space_id, &views, min_support)
        .await
        .map_err(|e| {
            AppError(CogniGraphError::BackendError(format!(
                "side-view gap detection: {e}"
            )))
        })?;
    let mut skipped: Vec<Value> = skipped
        .iter()
        .map(|skip| serde_json::to_value(skip).unwrap_or(Value::Null))
        .collect();
    let cap = selection.max_candidates.unwrap_or(DEFAULT_MAX_CANDIDATES);
    for over in candidates.split_off(cap.min(candidates.len())) {
        skipped.push(json!({
            "reason": "over_max_candidates",
            "source": over.fact.source,
            "target": over.fact.target,
            "detail": arrow(&over.fact),
            "side_views": over.side_views,
        }));
    }
    let report = json!({
        "collection": selection.collection,
        "scanned": scanned,
        "min_support": min_support,
        "candidates": candidates.iter().map(|candidate| json!({
            "fact": arrow(&candidate.fact),
            "support": candidate.side_views.len(),
            "side_views": candidate.side_views,
        })).collect::<Vec<_>>(),
        "skipped": skipped,
    });
    let support = candidates
        .iter()
        .map(|candidate| (triple(&candidate.fact), candidate.side_views.clone()))
        .collect();
    Ok(SideViewGaps {
        gaps: candidates
            .into_iter()
            .map(|candidate| candidate.fact)
            .collect(),
        support,
        report,
    })
}

/// Provenance stored on a proposal that came from side views.
pub(super) fn side_view_provenance(
    collection: &str,
    support: &BTreeMap<(String, String, String), Vec<String>>,
    neuron: &Neuron,
) -> Option<Value> {
    let key = (
        neuron.source.clone(),
        neuron.relation.clone(),
        neuron.target.clone(),
    );
    support.get(&key).map(|ids| {
        json!({
            "collection": collection,
            "side_views": ids,
            "support": ids.len(),
        })
    })
}
