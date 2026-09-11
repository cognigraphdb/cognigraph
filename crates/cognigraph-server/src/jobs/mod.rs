//! Durable, tenant-scoped governed operations (M16 foundation, M17 queue
//! governance).
//!
//! `_cognigraph_jobs` is deliberately accessed through the raw backend retained by the
//! server, never through `AppState::backend` (which protects every underscore
//! collection). A job embeds its transition history so a state change and its
//! audit event are one document replacement on every backend.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cognigraph_auth::{AuthProvider, DEFAULT_TENANT, Role, Tenant, TenantStatus, User};
use cognigraph_cache::QueryCache;
use cognigraph_construct::{
    Chunk, EvalSpec, Neuron, QaPair, SpaceType, effective_config, effective_vetoes, evaluate,
    generate_sideviews,
};
use cognigraph_core::{CogniGraphError, CollectionType, GraphBackend};
use cognigraph_embeddings::EmbeddingProvider;
use cognigraph_embeddings::completion::CompletionProvider;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex as AsyncMutex;

use crate::state::AppState;
use crate::system_collections::{
    is_generated_collection, is_managed_collection, is_system_collection,
};
use crate::tenancy::{CURRENT_TENANT, TenantScoped};

pub const JOBS_COLLECTION: &str = "_cognigraph_jobs";
pub const JOB_ARCHIVE_COLLECTION: &str = "_cognigraph_job_archive";
pub const JOB_CATALOG_COLLECTION: &str = "_cognigraph_job_catalog";
const JOB_SCHEMA_VERSION: u32 = 1;
const M21_JOB_SCHEMA_VERSION: u32 = 2;
const M22_JOB_SCHEMA_VERSION: u32 = 3;
const M23_JOB_SCHEMA_VERSION: u32 = 4;
const JOB_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const MAX_JOB_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_JOB_LIST_OFFSET: usize = 10_000;
pub const MAX_JOB_CURSOR_SCAN: usize = 2_048;
pub const MAX_JOB_OPERATOR_BATCH: usize = 1_000;
const MAX_INGEST_CHUNKS: usize = 100_000;
/// Upper bound on source documents a single side-view generation job enumerates.
/// Keeps one job's frozen key list (and its embedding + completion fan-out)
/// bounded; larger corpora are generated in multiple explicit batch calls.
const MAX_SIDEVIEW_DOCS: usize = 10_000;
/// Upper bound on source documents one `construct.draft` job drafts. Every
/// document costs two LLM completions, and the accumulating draft is rewritten
/// each pass, so the frozen document list stays bounded; larger corpora are
/// drafted in multiple explicit submissions.
const MAX_DRAFT_DOCUMENTS: usize = 2_000;
/// Chunks shown to the model per document when the request omits `sample_cap`
/// — the same default the synchronous route applies.
const DEFAULT_DRAFT_SAMPLE_CAP: usize = 40;
/// Accepted vocabulary. Read-only here: `construct.draft` refuses ids that
/// already exist (D3) and NEVER writes this collection.
const SPACE_TYPES: &str = "space_types";
/// Where drafts live — inert, no grounding path reads it (D1). Doubles as the
/// durable job's accumulator between passes.
const SPACE_TYPE_DRAFTS: &str = "space_type_drafts";
const MAX_OPERATOR_REASON_BYTES: usize = 1_024;
const MAX_CURSOR_POSITION_BYTES: usize = 1_024;
const MAX_RETRIES: usize = 100;
const M21_CONSUMPTION_TIMEOUT_SECS: u64 = 300;
const M21_CANCELLATION_POLL_MS: u64 = 1_000;

#[cfg(test)]
mod tests;

mod contracts;
pub use contracts::*;

mod payload;
use payload::*;

mod records;
pub use records::*;

mod operation_contracts;
pub use operation_contracts::*;

mod runtime;
use runtime::*;

mod metrics;
pub use metrics::*;

mod manager;
pub use manager::*;

mod admission;

mod evaluation_source;

mod listing;

mod status;

mod reconciliation;

mod catalog_repair;

mod cancellation_retry;

mod recovery;

mod recovery_validation;

mod tenant_lifecycle;

mod test_controls;

mod dispatch;

mod scheduled_work;

mod worker;

mod worker_transitions;

mod capacity;

mod storage;

mod catalog_listing;

mod archive_validation;
pub(crate) use archive_validation::*;

mod catalog_keys;
use catalog_keys::*;

mod cursors;
use cursors::*;

mod record_fields;
use record_fields::*;

mod tenant_identity;
use tenant_identity::*;

mod prepare;
use prepare::*;

mod validation;
use validation::*;

mod archival;
