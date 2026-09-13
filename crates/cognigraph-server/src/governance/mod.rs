//! M19 signed policy governance and tenant-scoped trust registry.
//!
//! The database contains only root-certified public keys and signed immutable
//! statements. The one trust anchor is configured outside tenant storage. All
//! mutations share `PromotionManager::transition_lock`, preserving the
//! singleton/non-HA boundary used by M18 promotion decisions.

use std::collections::{BTreeMap, HashSet};

use cognigraph_auth::{Role, User};
use cognigraph_core::{CogniGraphError, CollectionType};
use cognigraph_governance::{
    GovernanceStatement, KeyPurpose, SignatureEnvelope, VerificationKey, key_id_from_public_key,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use unicode_normalization::is_nfc;

use crate::artifact_attestations::{
    ARTIFACT_ATTESTATIONS_COLLECTION, ArtifactAttestationRecord,
    MAX_ARTIFACT_ATTESTATIONS_PER_TENANT, MAX_ARTIFACT_MANIFEST_BYTES_PER_TENANT,
};
use crate::promotions::{
    DIGEST_ALGORITHM, PolicyGovernanceBinding, PromotionActor, PromotionDecision,
    PromotionEvidence, PromotionManager, PromotionPage, PromotionTarget, ResolvedPromotionPolicy,
    canonical_digest, canonical_json_bytes, digest_bytes, now_millis, record_digest,
    same_stored_record, scoped_key, validate_idempotency_key, validate_reason,
};
use crate::semantic_repairs::{
    MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT, MAX_SEMANTIC_REPAIR_REVIEWS_PER_TENANT,
    MAX_SEMANTIC_REPAIR_REVISIONS_PER_TENANT, SEMANTIC_REPAIR_REVIEWS_COLLECTION,
    SEMANTIC_REPAIR_REVISIONS_COLLECTION, SemanticRepairReviewRecord, SemanticRepairRevisionRecord,
    deserialize_semantic_repair_stored, ensure_semantic_repair_candidate_capacity,
};

pub const GOVERNANCE_KEYS_COLLECTION: &str = "_cognigraph_governance_keys";
pub const GOVERNANCE_KEY_REVOCATIONS_COLLECTION: &str = "_cognigraph_governance_key_revocations";
pub const POLICY_REVISIONS_COLLECTION: &str = "_cognigraph_promotion_policy_revisions";
pub const POLICY_APPROVALS_COLLECTION: &str = "_cognigraph_promotion_policy_approvals";

pub const KEY_REGISTRATION_DOMAIN: &str = "cognigraph.key-registration.v1";
pub const M20_KEY_REGISTRATION_DOMAIN: &str = "cognigraph.key-registration.v2";
pub const KEY_REVOCATION_DOMAIN: &str = "cognigraph.key-revocation.v1";
pub const POLICY_REVISION_DOMAIN: &str = "cognigraph.policy-revision.v1";
pub const POLICY_APPROVAL_DOMAIN: &str = "cognigraph.policy-approval.v1";
pub const PROMOTION_INTENT_DOMAIN: &str = "cognigraph.promotion-intent.v1";
pub const M21_PROMOTION_INTENT_DOMAIN: &str = "cognigraph.promotion-intent.v2";
pub const M22_PROMOTION_INTENT_DOMAIN: &str = "cognigraph.promotion-intent.v3";
pub const M23_PROMOTION_INTENT_DOMAIN: &str = "cognigraph.promotion-intent.v4";

pub const GOVERNANCE_RECORD_SCHEMA_VERSION: u32 = 1;
const MAX_KEYS_PER_TENANT: usize = 1_024;
const MAX_POLICY_REVISIONS_PER_TENANT: usize = 10_000;
const MAX_PAGE_SIZE: usize = 200;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_CLOCK_SKEW_MS: u64 = 5 * 60 * 1_000;
const GOVERNANCE_SCAN_PAGE_SIZE: usize = 256;

#[cfg(test)]
mod tests;

mod identities;
pub use identities::*;

mod key_contracts;
pub use key_contracts::*;

mod policy_contracts;
pub use policy_contracts::*;

mod intent_contracts;
pub use intent_contracts::*;

mod repository;

mod key_registration;

mod key_revocation;

mod policy_revision;

mod policy_approval;

mod policy_binding;

mod promotion_intent;

mod queries;

mod key_validation;

mod policy_validation;

mod scoped_records;

mod listing;

mod authority_loading;

mod authority_validation;

mod historical_authority;

mod historical_binding;

mod snapshot;

mod historical_heads;
use historical_heads::*;

mod validation;
pub(crate) use validation::*;
