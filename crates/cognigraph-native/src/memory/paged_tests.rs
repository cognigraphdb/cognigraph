//! Controlled interleavings at the commit/fill-to-publication boundaries.
//! Hooks and channels are compiled only into native unit tests.

use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use cognigraph_core::{CollectionType, GraphBackend, VectorSearchOpts};
use serde_json::{Value, json};

use super::{NativeBackend, StorageMode, VectorMode, paged::DocCache};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PausePoint {
    ReadFill,
    WritePublication,
    DeletePublication,
    SidecarPublication,
}

pub(super) struct Pause {
    point: PausePoint,
    arrived: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
}

impl NativeBackend {
    pub(super) fn pause_paged_test(&self, point: PausePoint) {
        let pause = {
            let mut slot = self.paged_pause.lock().unwrap();
            if slot.as_ref().is_some_and(|pause| pause.point == point) {
                slot.take()
            } else {
                None
            }
        };
        if let Some(pause) = pause {
            pause.arrived.send(()).unwrap();
            pause
                .resume
                .recv_timeout(Duration::from_secs(10))
                .expect("test did not release paused operation");
        }
    }
}

struct Database(PathBuf);

impl Database {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cg-paged-race-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn open(&self) -> NativeBackend {
        NativeBackend::open_with_modes(
            self.0.join("data.redb"),
            VectorMode::Sidecar,
            StorageMode::Paged,
            1 << 20,
        )
        .unwrap()
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn document(version: u64) -> Value {
    json!({"_key": "a", "version": version, "embedding": if version == 1 { [1.0, 0.0] } else { [0.0, 1.0] }})
}

fn search(backend: &NativeBackend, query: &[f64]) -> Vec<cognigraph_core::SearchHit> {
    backend
        .vector_search_impl(
            "docs",
            query,
            &VectorSearchOpts {
                limit: 1,
                threshold: Some(0.9),
                model_name: None,
            },
        )
        .unwrap()
}

/// On the unfixed backend, complete B before releasing A to reproduce stale
/// publication. With the state fence, B must wait until A is released.
fn interleave(
    backend: &Arc<NativeBackend>,
    point: PausePoint,
    first: impl FnOnce(&NativeBackend) + Send + 'static,
    second: impl FnOnce(&NativeBackend) + Send + 'static,
) {
    let (arrived_tx, arrived_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    *backend.paged_pause.lock().unwrap() = Some(Pause {
        point,
        arrived: arrived_tx,
        resume: resume_rx,
    });
    let a_backend = backend.clone();
    let a = std::thread::spawn(move || first(&a_backend));
    arrived_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let fenced = backend.state.try_write().is_err();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let b_backend = backend.clone();
    let b = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        second(&b_backend);
        done_tx.send(()).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    if fenced {
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(25)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
    } else {
        done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    resume_tx.send(()).unwrap();
    a.join().unwrap();
    b.join().unwrap();
}

fn assert_current(backend: &NativeBackend, expected: Option<u64>) {
    let truth = backend
        .require_store()
        .unwrap()
        .get_document_raw("docs", "a")
        .unwrap();
    assert_eq!(
        truth.as_ref().map(|doc| doc["version"].as_u64().unwrap()),
        expected
    );
    assert_eq!(
        backend.get_document_impl("docs", "a").unwrap(),
        truth.map(|doc| backend.strip_embedding_if_sidecar(doc)),
        "cached document differs from committed redb truth"
    );
    if expected.is_some() {
        assert_eq!(
            search(backend, &[0.0, 1.0]).len(),
            1,
            "new vector missing from sidecar candidates"
        );
        assert!(
            search(backend, &[1.0, 0.0]).is_empty(),
            "old vector survived publication"
        );
    }
}

fn check_reopen(db: &Database, backend: Arc<NativeBackend>, expected: Option<u64>) {
    assert_current(&backend, expected);
    let before = block_on(backend.export_json()).unwrap();
    drop(backend);
    let reopened = db.open();
    assert_current(&reopened, expected);
    assert_eq!(block_on(reopened.export_json()).unwrap(), before);
}

fn seeded(db: &Database) -> Arc<NativeBackend> {
    let backend = Arc::new(db.open());
    backend.create_document_impl("docs", document(0)).unwrap();
    search(&backend, &[0.0, 1.0]);
    backend
}

#[test]
fn update_update_orders_cache_and_sidecar_publication() {
    let db = Database::new();
    let backend = seeded(&db);
    interleave(
        &backend,
        PausePoint::WritePublication,
        |b| {
            b.update_document_impl("docs", "a", document(1)).unwrap();
        },
        |b| {
            b.update_document_impl("docs", "a", document(2)).unwrap();
        },
    );
    check_reopen(&db, backend, Some(2));
}

#[test]
fn replace_delete_cannot_resurrect_cached_document() {
    let db = Database::new();
    let backend = seeded(&db);
    interleave(
        &backend,
        PausePoint::WritePublication,
        |b| {
            b.replace_document_impl("docs", "a", document(1)).unwrap();
        },
        |b| {
            assert!(b.delete_document_impl("docs", "a").unwrap());
        },
    );
    assert!(search(&backend, &[1.0, 0.0]).is_empty());
    check_reopen(&db, backend, None);
}

#[test]
fn delete_create_preserves_new_cache_and_vector() {
    let db = Database::new();
    let backend = seeded(&db);
    interleave(
        &backend,
        PausePoint::DeletePublication,
        |b| {
            assert!(b.delete_document_impl("docs", "a").unwrap());
        },
        |b| {
            b.create_document_impl("docs", document(2)).unwrap();
        },
    );
    check_reopen(&db, backend, Some(2));
}

#[test]
fn cache_miss_cannot_publish_stale_after_update_or_delete() {
    for delete in [false, true] {
        let db = Database::new();
        let backend = seeded(&db);
        *backend.doc_cache.as_ref().unwrap().lock().unwrap() = DocCache::new(1 << 20);
        interleave(
            &backend,
            PausePoint::ReadFill,
            |b| {
                assert_eq!(
                    b.get_document_impl("docs", "a").unwrap().unwrap()["version"],
                    0
                );
            },
            move |b| {
                if delete {
                    b.delete_document_impl("docs", "a").unwrap();
                } else {
                    b.update_document_impl("docs", "a", document(2)).unwrap();
                }
            },
        );
        check_reopen(&db, backend, (!delete).then_some(2));
    }
}

#[test]
fn drop_and_recreate_clear_warm_cache_and_sidecar() {
    let db = Database::new();
    let backend = seeded(&db);
    backend
        .create_document_impl("other", json!({"_key": "keep", "value": 7}))
        .unwrap();
    let other_before = backend.get_document_impl("other", "keep").unwrap();
    block_on(backend.drop_collection("docs")).unwrap();
    assert!(backend.get_document_impl("docs", "a").unwrap().is_none());
    block_on(backend.ensure_collection("docs", CollectionType::Document)).unwrap();
    assert!(backend.get_document_impl("docs", "a").unwrap().is_none());
    assert!(search(&backend, &[0.0, 1.0]).is_empty());
    assert_eq!(
        backend.get_document_impl("other", "keep").unwrap(),
        other_before
    );
    backend.create_document_impl("docs", document(2)).unwrap();
    check_reopen(&db, backend, Some(2));
}

#[test]
fn drop_recreate_cannot_be_repopulated_by_a_pending_cache_miss() {
    let db = Database::new();
    let backend = seeded(&db);
    *backend.doc_cache.as_ref().unwrap().lock().unwrap() = DocCache::new(1 << 20);
    interleave(
        &backend,
        PausePoint::ReadFill,
        |b| {
            b.get_document_impl("docs", "a").unwrap();
        },
        |b| {
            block_on(b.drop_collection("docs")).unwrap();
            block_on(b.ensure_collection("docs", CollectionType::Document)).unwrap();
        },
    );
    check_reopen(&db, backend, None);
}

#[test]
fn sidecar_rebuild_cannot_lose_an_intervening_write() {
    let db = Database::new();
    let backend = seeded(&db);
    for i in 0..40 {
        backend
            .create_document_impl(
                "docs",
                json!({"_key": format!("distractor-{i:02}"), "embedding": [0.6, 0.8]}),
            )
            .unwrap();
    }
    backend.sidecars.write().unwrap().clear();
    interleave(
        &backend,
        PausePoint::SidecarPublication,
        |b| {
            search(b, &[0.0, 1.0]);
        },
        |b| {
            b.update_document_impl("docs", "a", document(1)).unwrap();
        },
    );
    // Advance the stale sidecar's stamp using a different key. Without the
    // build fence this falsely marks the missing update to `a` as applied.
    backend
        .create_document_impl("docs", json!({"_key": "b", "embedding": [0.0, 1.0]}))
        .unwrap();
    let hits = search(&backend, &[1.0, 0.0]);
    assert_eq!(
        hits.len(),
        1,
        "intervening write disappeared from sidecar candidates"
    );
    assert_eq!(hits[0].document["_key"], "a");
    backend
        .update_document_impl("docs", "a", document(2))
        .unwrap();
    check_reopen(&db, backend, Some(2));
}

#[test]
fn concurrent_cold_searches_share_one_sidecar_build() {
    let db = Database::new();
    let backend = Arc::new(db.open());
    backend.create_document_impl("docs", document(2)).unwrap();
    interleave(
        &backend,
        PausePoint::SidecarPublication,
        |b| {
            assert_eq!(search(b, &[0.0, 1.0]).len(), 1);
        },
        |b| {
            assert_eq!(search(b, &[0.0, 1.0]).len(), 1);
        },
    );
    assert_eq!(backend.sidecar_rebuild_count(), 1);
    check_reopen(&db, backend, Some(2));
}
