//! Compatibility re-exports for the shared content-addressed artifact crate.
//!
//! M24 moved the filesystem verifier into `cognigraph-artifacts` so the
//! online evaluation path and offline custody CLI use one path, scope, and
//! digest implementation. The server still treats this source as read-only.

pub use cognigraph_artifacts::{ArtifactEvaluationBudget, LocalArtifactCas};

#[cfg(test)]
pub use cognigraph_artifacts::tenant_scope_hex;
