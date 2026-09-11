//! Reproducibility.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReproducibilityContext {
    pub deterministic: bool,
    pub source_commit: String,
    pub source_tree_digest: String,
    pub dirty_tree: bool,
    pub executable_digest: String,
    pub toolchain: String,
    pub target: String,
    pub backend: String,
    pub backend_version: String,
    pub command_digest: String,
    pub resolved_config_digest: String,
    pub seed: Option<u64>,
    pub provider: Option<String>,
    pub model: Option<String>,
}
impl ReproducibilityContext {
    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        if !self.deterministic || self.dirty_tree || self.provider.is_some() || self.model.is_some()
        {
            return Err(validation(
                "M18 v1 only accepts deterministic, clean-tree, non-provider-backed evaluation",
            ));
        }
        for (label, value) in [
            ("source_commit", &self.source_commit),
            ("toolchain", &self.toolchain),
            ("target", &self.target),
            ("backend", &self.backend),
            ("backend_version", &self.backend_version),
        ] {
            validate_text(
                &format!("reproducibility.{label}"),
                value,
                MAX_IDENTIFIER_BYTES,
            )?;
        }
        for (label, digest) in [
            ("source_tree_digest", &self.source_tree_digest),
            ("executable_digest", &self.executable_digest),
            ("command_digest", &self.command_digest),
            ("resolved_config_digest", &self.resolved_config_digest),
        ] {
            validate_digest(&format!("reproducibility.{label}"), digest)?;
        }
        Ok(())
    }
}
