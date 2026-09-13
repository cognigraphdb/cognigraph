//! Pure corpus-to-graph fact derivation.
//!
//! This module deliberately stops at evidence-bearing fact rows. It does not
//! write to a graph backend, allocate backend keys, or add storage metadata,
//! so callers can reproduce and compare the semantic graph before ingestion.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::evidence::canonical_chunks;
use crate::{Chunk, SpaceType};
use crate::{GroundedFact, VetoRule, ground_chunk};

/// One graph fact bound to the chunk that licensed it.
///
/// Field declaration order matches the canonical sort key. Deriving `Ord`
/// therefore gives stable lexicographic ordering by
/// `(source, relation, target, evidence_chunk_id)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedFactRow {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub evidence_chunk_id: String,
}

impl From<GroundedFact> for DerivedFactRow {
    fn from(grounded: GroundedFact) -> Self {
        Self {
            source: grounded.fact.source,
            relation: grounded.fact.relation,
            target: grounded.fact.target,
            evidence_chunk_id: grounded.chunk_id,
        }
    }
}

/// Resource limits and cooperative scheduling for one derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DerivationOptions {
    /// Maximum number of unique evidence-bearing rows in the result.
    pub max_fact_count: usize,
    /// Yield to the async runtime after this many processed chunks.
    pub yield_every_chunks: usize,
}

/// A closed failure surface for bounded derivation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DerivationError {
    #[error("yield_every_chunks must be greater than zero")]
    InvalidYieldInterval,
    #[error("derived fact row count exceeds configured maximum of {max_fact_count}")]
    FactLimitExceeded { max_fact_count: usize },
}

/// Derive a deterministic set of evidence-bearing graph rows.
///
/// `effective_space_type` is expected to already include any accepted alias
/// and relation-hint neurons. `vetoes` is expected to contain the accepted
/// blocker rules for the same construction recipe. Keeping those transforms
/// outside this primitive makes the exact effective inputs explicit.
/// Chunk evidence text is normalized to NFC, matching ingestion/materialization.
///
/// The result is sorted and unique by all four row fields. Repeated grounding
/// of the same triple in the same evidence chunk collapses to one row, while
/// the same triple grounded by different chunks remains independently
/// evidenced. The configured capacity applies to that unique result set.
pub async fn derive_fact_rows(
    chunks: &[Chunk],
    effective_space_type: &SpaceType,
    vetoes: &[VetoRule],
    options: DerivationOptions,
) -> Result<Vec<DerivedFactRow>, DerivationError> {
    if options.yield_every_chunks == 0 {
        return Err(DerivationError::InvalidYieldInterval);
    }

    let chunks = canonical_chunks(chunks);
    let mut rows = BTreeSet::new();
    for (index, chunk) in chunks.iter().enumerate() {
        for grounded in ground_chunk(&chunk.id, &chunk.text, effective_space_type, vetoes) {
            if rows.insert(DerivedFactRow::from(grounded)) && rows.len() > options.max_fact_count {
                return Err(DerivationError::FactLimitExceeded {
                    max_fact_count: options.max_fact_count,
                });
            }
        }

        if (index + 1) % options.yield_every_chunks == 0 {
            tokio::task::yield_now().await;
        }
    }

    Ok(rows.into_iter().collect())
}
