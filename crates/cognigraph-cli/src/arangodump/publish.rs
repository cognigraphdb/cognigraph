//! Path checks, source fingerprints, staging and no-overwrite publication.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::report::SourceFile;

/// A staging directory owned by this invocation; removed on drop, never
/// anything else.
pub(crate) struct Staging {
    pub dir: PathBuf,
}

impl Staging {
    /// Beside `output` for an import (same filesystem, so publication is a
    /// link), under the system temp directory for a dry run.
    pub fn create(output: &Path, dry_run: bool) -> io::Result<Self> {
        let id = uuid::Uuid::new_v4();
        let dir = if dry_run {
            std::env::temp_dir().join(format!("cognigraph-arangodump-{id}"))
        } else {
            let name = output
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            output.with_file_name(format!(".{name}.cg-import-{id}"))
        };
        fs::create_dir(&dir)?;
        Ok(Self { dir })
    }

    pub fn store(&self) -> PathBuf {
        self.dir.join("store.redb")
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Refusals that are usage errors (exit 2): nothing is read or created.
pub(crate) fn check_paths(dump: &Path, output: &Path, report: Option<&Path>) -> Result<(), String> {
    if !dump.is_dir() {
        return Err(format!("{} is not a directory", dump.display()));
    }
    match fs::symlink_metadata(output) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(format!(
                "{} already exists; the importer never overwrites a store",
                output.display()
            ));
        }
        Err(e) => return Err(format!("{}: {e}", output.display())),
    }
    let parent = |path: &Path| match path.parent() {
        Some(p) if p.as_os_str().is_empty() => PathBuf::from("."),
        Some(p) => p.to_path_buf(),
        None => PathBuf::from("."),
    };
    let output_dir = parent(output)
        .canonicalize()
        .map_err(|e| format!("{}: {e}", parent(output).display()))?;
    let dump_dir = dump
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dump.display()))?;
    if output_dir.starts_with(&dump_dir) {
        return Err("--output must not be inside the dump directory".into());
    }
    if let Some(report) = report {
        if !parent(report).is_dir() {
            return Err(format!(
                "report directory {} does not exist",
                parent(report).display()
            ));
        }
        if report == output {
            return Err("--report and --output must differ".into());
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> io::Result<(u64, String)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        hasher.update(&buffer[..n]);
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((bytes, digest))
}

pub(crate) fn fingerprint(dump: &Path, files: &[String]) -> io::Result<Vec<SourceFile>> {
    files
        .iter()
        .map(|name| {
            let (bytes, sha256) = sha256_file(&dump.join(name))?;
            Ok(SourceFile {
                path: name.clone(),
                bytes,
                sha256,
            })
        })
        .collect()
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    // Directory fsync makes the new name durable; not every platform allows it.
    match File::open(dir).and_then(|d| d.sync_all()) {
        Err(e) if e.kind() != io::ErrorKind::PermissionDenied => Err(e),
        _ => Ok(()),
    }
}

/// Make the closed staging store durable and give it the final name. A hard
/// link fails if `output` exists, so a store that appeared meanwhile is never
/// replaced.
pub(crate) fn publish(store: &Path, output: &Path) -> io::Result<()> {
    File::open(store)?.sync_all()?;
    fs::hard_link(store, output)?;
    sync_dir(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
}

/// Write the report under a temporary name in its directory, then rename.
pub(crate) fn write_report(path: &Path, body: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.tmp-{}", uuid::Uuid::new_v4()));
    let written = (|| {
        let mut file = File::create(&temporary)?;
        file.write_all(body)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}
