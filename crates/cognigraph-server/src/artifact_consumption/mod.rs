//! M21 verified artifact consumption, M22 reproducible derivation, and M23
//! reproducible raw-document preparation contracts.
//!
//! M20 authenticates exact-byte manifest claims. M21 additionally requires a
//! pinned loader plan, verifies every referenced blob from a tenant-scoped
//! local CAS, evaluates the graph snapshot and oracle loaded from those bytes,
//! and binds startup-pinned scorer/verifier executable-path bytes into a
//! durable receipt.
//! Signed location observations are never dereferenced.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cognigraph_construct::{
    Chunk, EndpointRef, EntityDef, EvalOutcome, EvalSpec, Fact, Neuron, NeuronKind, NeuronSet,
    NeuronStatus, PreparationOptions, RawDocumentBytes, RelationRule, SpaceType, TriggerProvenance,
    VetoRule, effective_config, effective_vetoes, evaluate_facts, prepare_documents,
    validate_neurons,
};
use cognigraph_core::CogniGraphError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use unicode_normalization::is_nfc;

use crate::artifact_attestations::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ActiveArtifactAttestations, ArtifactAttestationBinding,
    ArtifactAttestationRecord, ArtifactKind, ArtifactManifest,
};
use crate::artifact_cas::{ArtifactEvaluationBudget, LocalArtifactCas};
use crate::promotions::{
    DIGEST_ALGORITHM, PromotionContext, PromotionManager, PromotionTarget, canonical_digest,
    canonical_json_bytes, digest_bytes, now_millis, record_digest,
};

pub const M21_CONSUMPTION_PLAN_SCHEMA_VERSION: u32 = 1;
pub const M21_CONSUMPTION_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const M22_CONSUMPTION_PLAN_SCHEMA_VERSION: u32 = 2;
pub const M22_CONSUMPTION_RECEIPT_SCHEMA_VERSION: u32 = 2;
pub const M22_DERIVATION_PLAN_SCHEMA_VERSION: u32 = 1;
pub const M22_DERIVATION_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const M23_CONSUMPTION_PLAN_SCHEMA_VERSION: u32 = 3;
pub const M23_CONSUMPTION_RECEIPT_SCHEMA_VERSION: u32 = 3;
pub const M23_DERIVATION_RECEIPT_SCHEMA_VERSION: u32 = 2;
pub const M23_PREPARATION_PLAN_SCHEMA_VERSION: u32 = 1;
pub const M23_PREPARATION_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const LOCAL_CAS_RESOLVER: &str = "tenant-local-cas-v1";
pub const CORPUS_ARTIFACT_FORMAT: &str = "cognigraph.corpus.v1";
pub const GRAPH_ARTIFACT_FORMAT: &str = "cognigraph.evaluation-graph.v1";
pub const M22_CORPUS_ARTIFACT_FORMAT: &str = "cognigraph.prepared-chunk-corpus.v1";
pub const M22_GRAPH_ARTIFACT_FORMAT: &str = "cognigraph.reproducible-evaluation-graph.v1";
pub const M23_CORPUS_ARTIFACT_FORMAT: &str = "cognigraph.reproducible-prepared-chunk-corpus.v1";
pub const ORACLE_ARTIFACT_FORMAT: &str = "cognigraph.promotion-oracle.v1";
pub const EXECUTABLE_ARTIFACT_FORMAT: &str = "cognigraph.server-executable.v1";
pub const CORPUS_ENTRYPOINT: &str = "corpus.json";
pub const DOCUMENTS_ENTRYPOINT: &str = "documents.json";
pub const CANDIDATE_ENTRYPOINT: &str = "candidate.json";
pub const GRAPH_ENTRYPOINT: &str = "graph.json";
pub const ORACLE_ENTRYPOINT: &str = "oracle.json";
pub const EXECUTABLE_ENTRYPOINT: &str = "cognigraph-server";
pub const ARTIFACT_LOADER_ID: &str = "cognigraph.verified-artifact-loader";
pub const ARTIFACT_LOADER_VERSION: &str = "1";
pub const M22_ARTIFACT_LOADER_VERSION: &str = "2";
pub const M22_DERIVER_ID: &str = "cognigraph.prepared-corpus-grounder";
pub const M22_DERIVER_VERSION: &str = "1";
pub const M23_ARTIFACT_LOADER_VERSION: &str = "3";
pub const M23_PREPARER_ID: &str = "cognigraph.utf8-document-preparer";
pub const M23_PREPARER_VERSION: &str = "1";
const ARTIFACT_LOADER_SEMANTICS: &str =
    "tenant-local-cas/sha256-stream/graph-json/oracle-json/current-executable/v1";
const M22_ARTIFACT_LOADER_SEMANTICS: &str = "tenant-local-cas/sha256-stream/canonical-prepared-corpus-json/canonical-candidate-json/canonical-derived-graph-json/oracle-json/current-executable/v2";
const M22_DERIVER_SEMANTICS: &str = "prepared-chunks/indexed-effective-config/indexed-effective-vetoes/indexed-clause-negation/lazy-literal-template-expansion/cached-sentence-gate/text-weighted-bounded-ground-chunk/sorted-unique-evidence-facts/canonical-json/v1";
const M22_DERIVATION_ABI: &str = "cognigraph.prepared-corpus-to-evaluation-facts.v1";
const M23_ARTIFACT_LOADER_SEMANTICS: &str = "tenant-local-cas/sha256-stream/canonical-base64url-raw-documents-json/canonical-reproduced-prepared-corpus-json/canonical-candidate-json/canonical-derived-graph-json/oracle-json/current-executable/v3";
const M23_PREPARER_SEMANTICS: &str = "nfc-control-free-nonblank-document-id-max-1024-bytes/nfc-control-free-title-max-1024-bytes/nonempty-exact-utf8-bytes/strip-one-leading-utf8-bom/reject-controls-except-cr-lf-tab/crlf-and-cr-to-lf/unicode-17.0.0-nfc/precollapse-per-document-normalized-byte-limit/blank-line-paragraphs/trim-line-edge-unicode-whitespace/collapse-interior-unicode-17.0.0-whitespace-runs-to-one-ascii-space/nonblank-lines-and-packed-paragraphs-joined-by-one-ascii-space/reject-no-nonblank-paragraphs/postcollapse-aggregate-normalized-byte-limit/greedy-byte-bounded-packing/sentence-punctuation-before-whitespace-then-whitespace-then-utf8-boundary-split/no-overlap/chunk-id-d-lowercase-full-sha256-of-nfc-document-id-utf8-c-zero-based-eight-digit-decimal/sorted-chunks/v1";
const M23_PREPARATION_ABI: &str = "cognigraph.raw-utf8-documents-to-prepared-chunks.v1";
const SCORER_ABI: &str = "cognigraph.distinct-fact-set.v1";
const VERIFIER_ABI: &str = "cognigraph.graph-evidence-verifier.v1";
const MAX_GRAPH_FACTS: usize = 100_000;
const MAX_TEXT_BYTES: usize = 1_024;
const MAX_GRAPH_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ORACLE_ARTIFACT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_M22_CORPUS_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_M22_CANDIDATE_ARTIFACT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_M22_CHUNKS: usize = 100_000;
const MAX_M22_CHUNK_TEXT_BYTES: usize = 1024 * 1024;
const MAX_M22_CONFIG_ITEMS: usize = 100_000;
const MAX_M22_GROUNDING_WORK: u64 = 10_000_000;
const MAX_M22_CHUNK_GROUNDING_WORK: u64 = 250_000;
const MAX_M23_DOCUMENTS_ARTIFACT_BYTES: u64 = 96 * 1024 * 1024;
const MAX_M23_DOCUMENTS: usize = 100_000;
const MAX_M23_RAW_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_M23_TOTAL_RAW_DOCUMENT_BYTES: usize = 64 * 1024 * 1024;
const MAX_M23_NORMALIZED_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_M23_TOTAL_NORMALIZED_BYTES: usize = 64 * 1024 * 1024;
const MAX_M23_CHUNK_BYTES: usize = 8 * 1024;
const MAX_M23_TOTAL_PREPARED_TEXT_BYTES: usize = 48 * 1024 * 1024;
const M23_YIELD_EVERY_DOCUMENTS: usize = 16;
const MAX_EVALUATION_MANIFEST_ENTRIES: u64 = 10_000;
const MAX_EVALUATION_UNIQUE_BLOBS: usize = 4_096;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[cfg(test)]
mod tests;

mod plan_contracts;
pub use plan_contracts::*;

mod preparation_plan;
pub use preparation_plan::*;

mod derivation_plan;
pub use derivation_plan::*;

mod supported_plans;

mod consumed_receipt;
pub use consumed_receipt::*;

mod preparation_receipt;
pub use preparation_receipt::*;

mod derivation_receipt;
pub use derivation_receipt::*;

mod manifest_receipt;
use manifest_receipt::*;

mod receipt;
pub use receipt::*;

mod evidence_contracts;
pub use evidence_contracts::*;

mod preparation_authority;

mod derivation_authority;

mod consumption_authority;

mod evaluation_graph;
pub use evaluation_graph::*;

mod corpus;
pub use corpus::*;

mod construction_contracts;
pub use construction_contracts::*;

mod construction_neuron;

mod construction_config;

mod reproducible_graph;
pub use reproducible_graph::*;

mod oracle;
pub use oracle::*;

mod consumption_contracts;
pub(crate) use consumption_contracts::*;

mod consume;

mod result;
use result::*;

mod manifest;
use manifest::*;

mod executable;
pub(crate) use executable::*;

mod validation;
pub(crate) use validation::*;

mod grounding_work;
use grounding_work::*;

mod graph_validation;
use graph_validation::*;
