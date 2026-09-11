//! Read-only, tenant-scoped access to externally staged artifact blobs.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use cognigraph_core::CogniGraphError;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

pub(crate) const HASH_BUFFER_BYTES: usize = 64 * 1024;
pub(crate) const SHA256_PREFIX: &str = "sha256:";
pub(crate) const SHA256_HEX_BYTES: usize = 64;

/// A read-only local content-addressed artifact source.
///
/// The configured root and its `tenants` child are an external custody
/// boundary: this type never creates, replaces, or deletes content within it.
#[derive(Debug, Clone)]
pub struct LocalArtifactCas {
    root: PathBuf,
    tenants_root: PathBuf,
    max_evaluation_bytes: u64,
}

/// Mutable byte ledger for exactly one tenant-incarnation evaluation.
#[derive(Debug)]
pub struct ArtifactEvaluationBudget {
    cas_root: PathBuf,
    tenant_scope_hex: String,
    max_bytes: u64,
    consumed_bytes: u64,
}

impl ArtifactEvaluationBudget {
    /// Bytes reserved by successful and failed verification attempts.
    pub fn consumed_bytes(&self) -> u64 {
        self.consumed_bytes
    }

    fn reserve(&mut self, bytes: u64) -> Result<(), CogniGraphError> {
        let total = self.consumed_bytes.checked_add(bytes).ok_or_else(|| {
            CogniGraphError::CapacityExceeded(
                "artifact evaluation byte accounting overflowed".into(),
            )
        })?;
        if total > self.max_bytes {
            return Err(CogniGraphError::CapacityExceeded(format!(
                "artifact evaluation requires {total} bytes, exceeding its {} byte limit",
                self.max_bytes
            )));
        }
        // Failed reads deliberately keep their reservation. Evaluations abort
        // after a verification failure, and fail-closed accounting prevents
        // repeated failed reads from becoming unbounded I/O.
        self.consumed_bytes = total;
        Ok(())
    }
}

/// Compact projection of one byte stream that was fully read and verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedArtifactBlob {
    pub blob_digest: String,
    pub byte_length: u64,
    /// Present only when the caller explicitly requested retention and the
    /// declared length fit within its supplied cap.
    pub retained_bytes: Option<Vec<u8>>,
}

impl LocalArtifactCas {
    /// Validate and retain a read-only external CAS root.
    ///
    /// `root` must be an existing absolute, non-symlink directory with an
    /// existing normal `tenants` child. A zero evaluation budget is rejected.
    pub fn open(
        root: impl AsRef<Path>,
        max_evaluation_bytes: u64,
    ) -> Result<Self, CogniGraphError> {
        if max_evaluation_bytes == 0 {
            return Err(validation(
                "local artifact CAS max_evaluation_bytes must be positive",
            ));
        }
        let requested_root = root.as_ref();
        if !requested_root.is_absolute() {
            return Err(validation("local artifact CAS root must be absolute"));
        }
        require_normal_directory(requested_root, "local artifact CAS root")?;
        let canonical_root = canonicalize(requested_root, "local artifact CAS root")?;

        let requested_tenants = canonical_root.join("tenants");
        require_normal_directory(&requested_tenants, "local artifact CAS tenants directory")?;
        let canonical_tenants =
            canonicalize(&requested_tenants, "local artifact CAS tenants directory")?;
        require_direct_child(
            &canonical_root,
            &canonical_tenants,
            "local artifact CAS tenants directory",
        )?;

        Ok(Self {
            root: canonical_root,
            tenants_root: canonical_tenants,
            max_evaluation_bytes,
        })
    }

    /// Canonical local CAS root retained at open time.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn tenants_root(&self) -> &Path {
        &self.tenants_root
    }

    /// Start a byte ledger fixed to one tenant incarnation.
    pub fn begin_evaluation(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<ArtifactEvaluationBudget, CogniGraphError> {
        let tenant_scope_hex = tenant_scope_hex(tenant, incarnation)?;
        Ok(ArtifactEvaluationBudget {
            cas_root: self.root.clone(),
            tenant_scope_hex,
            max_bytes: self.max_evaluation_bytes,
            consumed_bytes: 0,
        })
    }

    /// Fully read, length-check, and SHA-256-check one staged blob.
    ///
    /// When `retain_cap` is `Some`, the exact verified bytes are retained for a
    /// downstream consumer. Exceeding that cap fails before filesystem I/O;
    /// `None` hashes without retaining the byte stream.
    pub async fn verify_blob(
        &self,
        budget: &mut ArtifactEvaluationBudget,
        blob_digest: &str,
        declared_byte_length: u64,
        retain_cap: Option<u64>,
    ) -> Result<VerifiedArtifactBlob, CogniGraphError> {
        if budget.cas_root != self.root || budget.max_bytes != self.max_evaluation_bytes {
            return Err(validation(
                "artifact evaluation budget belongs to a different local CAS",
            ));
        }
        let digest_hex = parse_sha256_digest(blob_digest)?;
        let retained_capacity = match retain_cap {
            Some(cap) if declared_byte_length > cap => {
                return Err(CogniGraphError::CapacityExceeded(format!(
                    "artifact blob length {declared_byte_length} exceeds the {cap} byte retention cap"
                )));
            }
            Some(_) => Some(usize::try_from(declared_byte_length).map_err(|_| {
                CogniGraphError::CapacityExceeded(
                    "artifact blob is too large to retain on this platform".into(),
                )
            })?),
            None => None,
        };
        budget.reserve(declared_byte_length)?;

        let std_file = self.open_checked_blob_in_scope(
            &budget.tenant_scope_hex,
            digest_hex,
            declared_byte_length,
        )?;
        let mut file = tokio::fs::File::from_std(std_file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
        let mut seen = 0_u64;
        let mut retained = retained_capacity.map(Vec::with_capacity);
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|error| storage_error("cannot read staged artifact blob", error))?;
            if read == 0 {
                break;
            }
            seen = checked_seen(seen, read, declared_byte_length)?;
            hasher.update(&buffer[..read]);
            if let Some(bytes) = &mut retained {
                bytes.extend_from_slice(&buffer[..read]);
            }
        }
        require_exact_stream(blob_digest, declared_byte_length, seen, hasher.finalize())?;
        self.recheck_parent_chain(&budget.tenant_scope_hex, digest_hex)?;

        Ok(VerifiedArtifactBlob {
            blob_digest: blob_digest.to_string(),
            byte_length: seen,
            retained_bytes: retained,
        })
    }

    pub(crate) fn open_checked_blob(
        &self,
        tenant: &str,
        incarnation: &str,
        blob_digest: &str,
        declared_byte_length: u64,
    ) -> Result<std::fs::File, CogniGraphError> {
        let scope = tenant_scope_hex(tenant, incarnation)?;
        let digest_hex = parse_sha256_digest(blob_digest)?;
        self.open_checked_blob_in_scope(&scope, digest_hex, declared_byte_length)
    }

    fn open_checked_blob_in_scope(
        &self,
        tenant_scope_hex: &str,
        digest_hex: &str,
        declared_byte_length: u64,
    ) -> Result<std::fs::File, CogniGraphError> {
        let scope_dir = checked_child_directory(
            &self.tenants_root,
            tenant_scope_hex,
            "artifact tenant scope directory",
        )?;
        let algorithm_dir =
            checked_child_directory(&scope_dir, "sha256", "artifact SHA-256 directory")?;
        let shard_dir = checked_child_directory(
            &algorithm_dir,
            &digest_hex[..2],
            "artifact SHA-256 shard directory",
        )?;
        let blob_path = shard_dir.join(&digest_hex[2..]);

        let before = std::fs::symlink_metadata(&blob_path)
            .map_err(|error| storage_error("cannot inspect staged artifact blob", error))?;
        if before.file_type().is_symlink() || !before.is_file() {
            return Err(storage_conflict(
                "staged artifact blob must be a regular non-symlink file",
            ));
        }
        if before.len() != declared_byte_length {
            return Err(storage_conflict(format!(
                "staged artifact blob length {} does not match declared length {declared_byte_length}",
                before.len()
            )));
        }
        let canonical_blob = canonicalize(&blob_path, "staged artifact blob")?;
        require_direct_child(&shard_dir, &canonical_blob, "staged artifact blob")?;

        let file = std::fs::File::open(&canonical_blob)
            .map_err(|error| storage_error("cannot open staged artifact blob", error))?;
        let opened = file
            .metadata()
            .map_err(|error| storage_error("cannot inspect opened artifact blob", error))?;
        if !opened.is_file() || opened.len() != declared_byte_length {
            return Err(storage_conflict(
                "opened artifact blob type or length changed during verification",
            ));
        }
        if !same_file_identity(&before, &opened) {
            return Err(storage_conflict(
                "staged artifact blob changed while it was being opened",
            ));
        }
        self.recheck_parent_chain(tenant_scope_hex, digest_hex)?;
        Ok(file)
    }

    pub(crate) fn recheck_parent_chain(
        &self,
        tenant_scope_hex: &str,
        digest_hex: &str,
    ) -> Result<(), CogniGraphError> {
        require_normal_directory(&self.root, "local artifact CAS root")?;
        require_normal_directory(&self.tenants_root, "local artifact CAS tenants directory")?;
        require_direct_child(
            &self.root,
            &canonicalize(&self.tenants_root, "local artifact CAS tenants directory")?,
            "local artifact CAS tenants directory",
        )?;
        let scope = checked_child_directory(
            &self.tenants_root,
            tenant_scope_hex,
            "artifact tenant scope directory",
        )?;
        let algorithm = checked_child_directory(&scope, "sha256", "artifact SHA-256 directory")?;
        let _ = checked_child_directory(
            &algorithm,
            &digest_hex[..2],
            "artifact SHA-256 shard directory",
        )?;
        Ok(())
    }
}

/// Stable tenant-incarnation scope: SHA-256 over `tenant`, one NUL separator,
/// and `incarnation`, encoded as lowercase hexadecimal without a prefix.
pub fn tenant_scope_hex(tenant: &str, incarnation: &str) -> Result<String, CogniGraphError> {
    validate_scope_inputs(tenant, incarnation)?;
    let mut hasher = Sha256::new();
    hasher.update(tenant.as_bytes());
    hasher.update([0]);
    hasher.update(incarnation.as_bytes());
    Ok(hex_encode(hasher.finalize()))
}

/// Stable tenant-incarnation scope in the public `sha256:<hex>` digest form.
pub fn tenant_scope_digest(tenant: &str, incarnation: &str) -> Result<String, CogniGraphError> {
    Ok(format!(
        "{SHA256_PREFIX}{}",
        tenant_scope_hex(tenant, incarnation)?
    ))
}

fn validate_scope_inputs(tenant: &str, incarnation: &str) -> Result<(), CogniGraphError> {
    if tenant.is_empty() || incarnation.is_empty() {
        return Err(validation(
            "artifact tenant and incarnation must both be non-empty",
        ));
    }
    if tenant.contains('\0') || incarnation.contains('\0') {
        return Err(validation(
            "artifact tenant and incarnation cannot contain NUL bytes",
        ));
    }
    Ok(())
}

pub(crate) fn parse_sha256_digest(digest: &str) -> Result<&str, CogniGraphError> {
    let Some(hex) = digest.strip_prefix(SHA256_PREFIX) else {
        return Err(validation(
            "artifact digest must use the sha256:<64 lowercase hex> form",
        ));
    };
    if hex.len() != SHA256_HEX_BYTES
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(validation(
            "artifact digest must use the sha256:<64 lowercase hex> form",
        ));
    }
    Ok(hex)
}

pub(crate) fn checked_child_directory(
    parent: &Path,
    component: &str,
    label: &str,
) -> Result<PathBuf, CogniGraphError> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.contains(['/', '\\'])
    {
        return Err(validation(format!("{label} has an unsafe path component")));
    }
    let child = parent.join(component);
    require_normal_directory(&child, label)?;
    let canonical = canonicalize(&child, label)?;
    require_direct_child(parent, &canonical, label)?;
    Ok(canonical)
}

pub(crate) fn require_normal_directory(path: &Path, label: &str) -> Result<(), CogniGraphError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| storage_error(format!("cannot inspect {label}"), error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(validation(format!(
            "{label} must be an existing normal non-symlink directory"
        )));
    }
    Ok(())
}

pub(crate) fn canonicalize(path: &Path, label: &str) -> Result<PathBuf, CogniGraphError> {
    std::fs::canonicalize(path)
        .map_err(|error| storage_error(format!("cannot canonicalize {label}"), error))
}

pub(crate) fn require_direct_child(
    parent: &Path,
    child: &Path,
    label: &str,
) -> Result<(), CogniGraphError> {
    if child.parent() != Some(parent) || !child.starts_with(parent) {
        return Err(validation(format!(
            "{label} escapes its canonical artifact parent"
        )));
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn same_file_identity(before: &std::fs::Metadata, opened: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    before.dev() == opened.dev() && before.ino() == opened.ino()
}

#[cfg(not(unix))]
pub(crate) fn same_file_identity(before: &std::fs::Metadata, opened: &std::fs::Metadata) -> bool {
    before.len() == opened.len()
        && before.created().ok() == opened.created().ok()
        && before.modified().ok() == opened.modified().ok()
}

pub(crate) fn checked_seen(
    seen: u64,
    read: usize,
    declared_byte_length: u64,
) -> Result<u64, CogniGraphError> {
    let seen = seen.checked_add(read as u64).ok_or_else(|| {
        storage_conflict("artifact blob byte count overflowed during verification")
    })?;
    if seen > declared_byte_length {
        return Err(storage_conflict(
            "artifact blob grew beyond its declared length during verification",
        ));
    }
    Ok(seen)
}

pub(crate) fn require_exact_stream(
    expected_digest: &str,
    declared_byte_length: u64,
    seen: u64,
    observed_hash: impl AsRef<[u8]>,
) -> Result<(), CogniGraphError> {
    if seen != declared_byte_length {
        return Err(storage_conflict(format!(
            "artifact blob produced {seen} bytes but declared {declared_byte_length}"
        )));
    }
    let observed_digest = format!("{SHA256_PREFIX}{}", hex_encode(observed_hash));
    if observed_digest != expected_digest {
        return Err(storage_conflict(format!(
            "artifact blob digest mismatch: expected {expected_digest}, observed {observed_digest}"
        )));
    }
    Ok(())
}

pub(crate) fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

pub(crate) fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}

pub(crate) fn storage_conflict(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::DocumentConflict(message.into())
}

pub(crate) fn storage_error(message: impl Into<String>, error: std::io::Error) -> CogniGraphError {
    CogniGraphError::BackendError(format!("{}: {error}", message.into()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn test_root() -> TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("tenants")).unwrap();
        root
    }

    fn digest(bytes: &[u8]) -> String {
        format!("{SHA256_PREFIX}{}", hex_encode(Sha256::digest(bytes)))
    }

    fn stage(root: &Path, tenant: &str, incarnation: &str, bytes: &[u8]) -> String {
        let digest = digest(bytes);
        let hex = parse_sha256_digest(&digest).unwrap();
        let scope = tenant_scope_hex(tenant, incarnation).unwrap();
        let directory = root
            .join("tenants")
            .join(scope)
            .join("sha256")
            .join(&hex[..2]);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(&hex[2..]), bytes).unwrap();
        digest
    }

    fn cas(max_bytes: u64) -> (TempDir, LocalArtifactCas) {
        let root = test_root();
        let cas = LocalArtifactCas::open(root.path(), max_bytes).unwrap();
        (root, cas)
    }

    #[test]
    fn open_requires_absolute_normal_root_and_tenants_directory() {
        assert!(LocalArtifactCas::open("relative", 1).is_err());
        let root = test_root();
        assert!(LocalArtifactCas::open(root.path(), 0).is_err());
        fs::remove_dir(root.path().join("tenants")).unwrap();
        assert!(LocalArtifactCas::open(root.path(), 1).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn open_rejects_root_and_tenants_symlinks() {
        use std::os::unix::fs::symlink;

        let target = test_root();
        let parent = tempfile::tempdir().unwrap();
        let root_link = parent.path().join("root-link");
        symlink(target.path(), &root_link).unwrap();
        assert!(LocalArtifactCas::open(&root_link, 1).is_err());

        let root = test_root();
        fs::remove_dir(root.path().join("tenants")).unwrap();
        symlink(target.path().join("tenants"), root.path().join("tenants")).unwrap();
        assert!(LocalArtifactCas::open(root.path(), 1).is_err());
    }

    #[tokio::test]
    async fn verifies_without_retention_and_with_bounded_retention() {
        let (root, cas) = cas(64);
        let bytes = b"verified artifact";
        let digest = stage(root.path(), "tenant-a", "inc-1", bytes);

        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        let verified = cas
            .verify_blob(&mut budget, &digest, bytes.len() as u64, None)
            .await
            .unwrap();
        assert_eq!(verified.blob_digest, digest);
        assert_eq!(verified.byte_length, bytes.len() as u64);
        assert_eq!(verified.retained_bytes, None);

        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        let retained = cas
            .verify_blob(
                &mut budget,
                &digest,
                bytes.len() as u64,
                Some(bytes.len() as u64),
            )
            .await
            .unwrap();
        assert_eq!(retained.retained_bytes.as_deref(), Some(bytes.as_slice()));
    }

    #[tokio::test]
    async fn invalid_digest_cannot_become_path_traversal() {
        let (_root, cas) = cas(64);
        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        for digest in [
            "../../etc/passwd",
            "sha256:../../etc/passwd",
            "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "sha512:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(cas.verify_blob(&mut budget, digest, 1, None).await.is_err());
        }
        assert_eq!(budget.consumed_bytes(), 0);
    }

    #[tokio::test]
    async fn tenant_scope_is_hash_only_and_cross_tenant_reads_fail() {
        let (root, cas) = cas(64);
        let bytes = b"tenant private";
        let digest = stage(root.path(), "../tenant-a", "inc/1", bytes);
        let scope = tenant_scope_hex("../tenant-a", "inc/1").unwrap();
        assert_eq!(scope.len(), 64);
        assert!(scope.bytes().all(|byte| byte.is_ascii_hexdigit()));

        let mut owner = cas.begin_evaluation("../tenant-a", "inc/1").unwrap();
        cas.verify_blob(&mut owner, &digest, bytes.len() as u64, None)
            .await
            .unwrap();

        let mut other = cas.begin_evaluation("tenant-b", "inc/1").unwrap();
        assert!(
            cas.verify_blob(&mut other, &digest, bytes.len() as u64, None)
                .await
                .is_err()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_symlinked_parent_and_blob() {
        use std::os::unix::fs::symlink;

        let (root, cas) = cas(64);
        let bytes = b"symlink target";
        let digest = digest(bytes);
        let hex = parse_sha256_digest(&digest).unwrap();
        let scope = tenant_scope_hex("tenant-a", "inc-1").unwrap();
        let external = root.path().join("external");
        fs::create_dir_all(external.join(&hex[..2])).unwrap();
        fs::write(external.join(&hex[..2]).join(&hex[2..]), bytes).unwrap();

        let scope_parent = root.path().join("tenants").join(&scope);
        fs::create_dir_all(&scope_parent).unwrap();
        symlink(&external, scope_parent.join("sha256")).unwrap();
        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        assert!(
            cas.verify_blob(&mut budget, &digest, bytes.len() as u64, None)
                .await
                .is_err()
        );

        fs::remove_file(scope_parent.join("sha256")).unwrap();
        let shard = scope_parent.join("sha256").join(&hex[..2]);
        fs::create_dir_all(&shard).unwrap();
        symlink(
            external.join(&hex[..2]).join(&hex[2..]),
            shard.join(&hex[2..]),
        )
        .unwrap();
        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        assert!(
            cas.verify_blob(&mut budget, &digest, bytes.len() as u64, None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_wrong_length_digest_and_budget_overrun() {
        let (root, cas) = cas(5);
        let first = stage(root.path(), "tenant-a", "inc-1", b"abc");
        let second = stage(root.path(), "tenant-a", "inc-1", b"def");
        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        cas.verify_blob(&mut budget, &first, 3, None).await.unwrap();
        assert!(
            cas.verify_blob(&mut budget, &second, 3, None)
                .await
                .is_err()
        );
        assert_eq!(budget.consumed_bytes(), 3);

        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        assert!(cas.verify_blob(&mut budget, &first, 4, None).await.is_err());
    }

    #[tokio::test]
    async fn retained_bytes_cannot_exceed_caller_cap() {
        let (root, cas) = cas(64);
        let bytes = b"retain me";
        let digest = stage(root.path(), "tenant-a", "inc-1", bytes);
        let mut budget = cas.begin_evaluation("tenant-a", "inc-1").unwrap();
        let error = cas
            .verify_blob(
                &mut budget,
                &digest,
                bytes.len() as u64,
                Some(bytes.len() as u64 - 1),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, CogniGraphError::CapacityExceeded(_)));
        assert_eq!(budget.consumed_bytes(), 0);
    }
}
