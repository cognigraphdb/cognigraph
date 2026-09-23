use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.to_string())
    }
}

const RAILWAY: [(&str, &str); 3] = [
    (MARKER, "1"),
    ("RAILWAY_VOLUME_MOUNT_PATH", "/data"),
    ("COGNIGRAPH_NATIVE_PATH", "/data/native/cognigraph.redb"),
];

#[test]
fn nothing_happens_without_the_image_marker_or_as_a_normal_user() {
    assert_eq!(decide(0, &env(&[])), Decision::Skip);
    assert_eq!(decide(0, &env(&[(MARKER, "0")])), Decision::Skip);
    assert_eq!(decide(10001, &env(&RAILWAY)), Decision::Skip);
    assert_eq!(decide(1000, &env(&[(MARKER, "1")])), Decision::Skip);
}

#[test]
fn root_needs_the_exact_railway_volume_contract() {
    assert_eq!(decide(0, &env(&RAILWAY)), Decision::Provision);
    for broken in [
        [
            (MARKER, "1"),
            ("RAILWAY_VOLUME_MOUNT_PATH", "/data/"),
            ("COGNIGRAPH_NATIVE_PATH", "/data/native/cognigraph.redb"),
        ],
        [
            (MARKER, "1"),
            ("RAILWAY_VOLUME_MOUNT_PATH", "/data"),
            ("COGNIGRAPH_NATIVE_PATH", "/data/cognigraph.redb"),
        ],
        [
            (MARKER, "1"),
            ("RAILWAY_VOLUME_MOUNT_PATH", "/data"),
            ("COGNIGRAPH_NATIVE_PATH", ""),
        ],
    ] {
        assert_eq!(
            decide(0, &env(&broken)),
            Decision::Refuse(ROOT_CONTRACT),
            "{broken:?}"
        );
    }
    assert_eq!(
        decide(0, &env(&[(MARKER, "1")])),
        Decision::Refuse(ROOT_CONTRACT)
    );
}

fn owner() -> (u32, u32) {
    (
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    )
}

#[test]
fn provisioning_creates_a_private_directory_once_and_accepts_it_again() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    std::fs::create_dir(&data).unwrap();
    provision(&data, owner()).unwrap();
    let native = data.join("native");
    let meta = std::fs::metadata(&native).unwrap();
    assert_eq!(meta.permissions().mode() & 0o7777, 0o700);
    // Idempotent: an existing correct directory is accepted unchanged.
    std::fs::write(native.join("keep"), b"x").unwrap();
    provision(&data, owner()).unwrap();
    assert!(native.join("keep").exists());
}

#[test]
fn a_symlinked_volume_or_native_directory_is_refused_before_any_change() {
    let temp = tempfile::tempdir().unwrap();
    let real = temp.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let linked_volume = temp.path().join("data");
    symlink(&real, &linked_volume).unwrap();
    assert_eq!(provision(&linked_volume, owner()), Err(ROOT_CONTRACT));
    assert!(!real.join("native").exists());

    let data = temp.path().join("volume");
    std::fs::create_dir(&data).unwrap();
    symlink(&real, data.join("native")).unwrap();
    assert_eq!(provision(&data, owner()), Err(ROOT_CONTRACT));
    assert!(std::fs::read_dir(&real).unwrap().next().is_none());

    assert_eq!(
        provision(&temp.path().join("absent"), owner()),
        Err(ROOT_CONTRACT)
    );
}

#[test]
fn an_existing_directory_with_the_wrong_mode_owner_or_type_is_refused_not_repaired() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let native = data.join("native");
    std::fs::create_dir_all(&native).unwrap();
    std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(provision(&data, owner()), Err(NATIVE_OWNERSHIP));
    assert_eq!(
        std::fs::metadata(&native).unwrap().permissions().mode() & 0o7777,
        0o755,
        "never repaired"
    );
    std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (uid, gid) = owner();
    assert_eq!(provision(&data, (uid + 1, gid)), Err(NATIVE_OWNERSHIP));
    std::fs::remove_dir(&native).unwrap();
    std::fs::write(&native, b"file").unwrap();
    assert_eq!(provision(&data, owner()), Err(NATIVE_OWNERSHIP));
}

#[test]
fn thread_counts_are_read_from_proc_status_text() {
    let status = "Name:\tcognigraph-serv\nThreads:\t1\nSigQ:\t0/1\n";
    assert_eq!(threads_in(status), Some(1));
    assert_eq!(threads_in("Threads:\t12\n"), Some(12));
    assert_eq!(threads_in("Name:\tx\n"), None);
}
