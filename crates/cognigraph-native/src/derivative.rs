//! Collection identities are data, never filesystem path components.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// A fixed-length name under the database's directory. Callers supply a
/// structured identity and a constant derivative kind, never a raw path.
pub(crate) fn derivative_path(db_path: &Path, identity: &str, kind: &str) -> PathBuf {
    let stem = db_path.file_name().unwrap_or_default();
    let digest: String = Sha256::digest(identity.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let mut name = stem.to_os_string();
    name.push(format!(".{digest}.{kind}"));
    db_path.with_file_name(name)
}
