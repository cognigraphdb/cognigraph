//! Container start-up as root (CG-81).
//!
//! The runtime image has no shell, so the Railway volume contract that the
//! former entrypoint script enforced runs here, before any thread exists. It
//! only acts when the image's `COGNIGRAPH_CONTAINER_INIT=1` marker is set and
//! the process is root; bare binaries and ordinary non-root containers are
//! untouched. As root it requires the exact Railway mount and Native path,
//! provisions the private `/data/native` directory once without repairing
//! anything, then permanently drops to 10001:10001 with no supplementary
//! groups, no capabilities in any set and `no_new_privs`.

use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::Path;

pub(crate) const MARKER: &str = "COGNIGRAPH_CONTAINER_INIT";
pub(crate) const ROOT_CONTRACT: &str =
    "Root startup requires the Railway /data volume and its fixed Native path";
pub(crate) const NATIVE_OWNERSHIP: &str =
    "Native directory must be owned by 10001:10001 with mode 0700";
const VOLUME: &str = "/data";
const NATIVE_PATH: &str = "/data/native/cognigraph.redb";
const RUNTIME_ID: u32 = 10001;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Skip,
    Provision,
    Refuse(&'static str),
}

pub(crate) fn decide(euid: u32, env: &dyn Fn(&str) -> Option<String>) -> Decision {
    if euid != 0 || env(MARKER).as_deref() != Some("1") {
        return Decision::Skip;
    }
    let mount = env("RAILWAY_VOLUME_MOUNT_PATH");
    let native = env("COGNIGRAPH_NATIVE_PATH");
    if mount.as_deref() != Some(VOLUME) || native.as_deref() != Some(NATIVE_PATH) {
        return Decision::Refuse(ROOT_CONTRACT);
    }
    Decision::Provision
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// Create `<volume>/native` (0700, owned by `owner`) when absent; otherwise
/// accept it only when it already has exactly that owner and mode.
pub(crate) fn provision(volume: &Path, owner: (u32, u32)) -> Result<(), &'static str> {
    let native = volume.join("native");
    let native_link = fs::symlink_metadata(&native).is_ok_and(|m| m.file_type().is_symlink());
    if !is_real_dir(volume) || native_link {
        return Err(ROOT_CONTRACT);
    }
    let old = rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o077));
    let created = match fs::DirBuilder::new().mode(0o700).create(&native) {
        Ok(()) => std::os::unix::fs::chown(&native, Some(owner.0), Some(owner.1)),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    };
    rustix::process::umask(old);
    created.map_err(|_| NATIVE_OWNERSHIP)?;
    match fs::symlink_metadata(&native) {
        Ok(m) if m.is_dir() && (m.uid(), m.gid()) == owner && m.mode() & 0o7777 == 0o700 => Ok(()),
        _ => Err(NATIVE_OWNERSHIP),
    }
}

/// Only the Linux privilege drop reads it outside tests.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn threads_in(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Threads:"))
        .and_then(|count| count.trim().parse().ok())
}

#[cfg(target_os = "linux")]
fn drop_privileges() -> io::Result<()> {
    use rustix::process::{Gid, Uid};
    use rustix::thread::{self, CapabilitySet, CapabilitySets};
    // These are per-thread system calls: dropping with more than one thread
    // alive would leave the others privileged.
    let status = fs::read_to_string("/proc/self/status")?;
    if threads_in(&status) != Some(1) {
        return Err(io::Error::other(
            "privilege drop needs a single-threaded process",
        ));
    }
    let gid = Gid::from_raw(RUNTIME_ID);
    let uid = Uid::from_raw(RUNTIME_ID);
    thread::set_thread_groups(&[])?;
    thread::set_thread_res_gid(gid, gid, gid)?;
    thread::set_thread_res_uid(uid, uid, uid)?;
    let none = CapabilitySets {
        effective: CapabilitySet::empty(),
        permitted: CapabilitySet::empty(),
        inheritable: CapabilitySet::empty(),
    };
    thread::set_capabilities(None, none)?;
    thread::clear_ambient_capability_set()?;
    thread::set_no_new_privs(true)?;
    // Verify rather than trust: every id and capability set is as intended.
    let sets = thread::capabilities(None)?;
    let dropped = rustix::process::getuid() == uid
        && rustix::process::geteuid() == uid
        && rustix::process::getgid() == gid
        && rustix::process::getegid() == gid
        && sets.effective.is_empty()
        && sets.permitted.is_empty()
        && sets.inheritable.is_empty();
    if !dropped {
        return Err(io::Error::other("privileges were not fully dropped"));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn drop_privileges() -> io::Result<()> {
    Err(io::Error::other(
        "container start-up as root is supported on Linux only",
    ))
}

/// Run before anything else in `main`. Exits the process with status 1 and a
/// message on stderr when the root contract is not met.
pub(crate) fn apply_or_exit() {
    let euid = rustix::process::geteuid().as_raw();
    let outcome = match decide(euid, &|name| std::env::var(name).ok()) {
        Decision::Skip => return,
        Decision::Refuse(message) => Err(message.to_string()),
        Decision::Provision => provision(Path::new(VOLUME), (RUNTIME_ID, RUNTIME_ID))
            .map_err(str::to_string)
            .and_then(|()| drop_privileges().map_err(|e| e.to_string())),
    };
    if let Err(message) = outcome {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[path = "container_init_tests.rs"]
mod tests;
