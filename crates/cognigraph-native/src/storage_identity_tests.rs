use super::*;

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cg-storage-id-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn schema_one_upgrade_preserves_rows_and_commits_identity_atomically() {
    let dir = TempDir::new();
    let path = dir.0.join("old.redb");
    let doc = serde_json::json!({"_key":"a", "text":"legacy", "embedding":[1.0,0.0]});
    {
        let db = Database::create(&path).unwrap();
        let txn = db.begin_write().unwrap();
        txn.open_table(META)
            .unwrap()
            .insert("schema_version", 1)
            .unwrap();
        txn.open_table(META)
            .unwrap()
            .insert(DATA_GENERATION_KEY, 17)
            .unwrap();
        txn.open_table(COLLECTIONS)
            .unwrap()
            .insert("docs", 0)
            .unwrap();
        txn.open_table(DOCUMENTS)
            .unwrap()
            .insert("docs\0a", doc.to_string().as_str())
            .unwrap();
        txn.commit().unwrap();
    }
    let store = RedbStore::open(&path).unwrap();
    let database_id = store.database_id().to_string();
    let revision = store.data_revision().unwrap();
    assert!(!Uuid::parse_str(&database_id).unwrap().is_nil());
    assert_eq!(store.data_generation().unwrap(), 17);
    assert_eq!(
        store.get_document_raw("docs", "a").unwrap(),
        Some(doc.clone())
    );
    assert_eq!(
        store
            .db
            .begin_read()
            .unwrap()
            .open_table(META)
            .unwrap()
            .get("schema_version")
            .unwrap()
            .unwrap()
            .value(),
        3
    );
    drop(store);
    let reopened = RedbStore::open(&path).unwrap();
    assert_eq!(reopened.database_id(), database_id);
    assert_eq!(reopened.data_revision().unwrap(), revision);
    reopened
        .apply(&[StoreOp::PutDocument {
            collection: "docs",
            key: "a",
            doc: &doc,
        }])
        .unwrap();
    let changed_revision = reopened.data_revision().unwrap();
    assert_ne!(changed_revision, revision);
    assert_eq!(reopened.database_id(), database_id);
    assert_eq!(reopened.data_generation().unwrap(), 18);
    assert!(
        reopened
            .apply(&[StoreOp::PutDocument {
                collection: "invalid\0name",
                key: "a",
                doc: &doc
            }])
            .is_err()
    );
    assert_eq!(reopened.data_revision().unwrap(), changed_revision);
    assert_eq!(reopened.data_generation().unwrap(), 18);
    assert_eq!(reopened.get_document_raw("docs", "a").unwrap(), Some(doc));
}

#[test]
fn schema_two_missing_or_corrupt_identity_fails_closed() {
    for missing in [true, false] {
        let dir = TempDir::new();
        let path = dir.0.join("corrupt.redb");
        drop(RedbStore::open(&path).unwrap());
        {
            let db = Database::create(&path).unwrap();
            let txn = db.begin_write().unwrap();
            if missing {
                txn.open_table(IDENTITY)
                    .unwrap()
                    .remove("database_id")
                    .unwrap();
            } else {
                txn.open_table(IDENTITY)
                    .unwrap()
                    .insert("data_revision", "not-a-uuid")
                    .unwrap();
            }
            txn.commit().unwrap();
        }
        assert!(
            RedbStore::open(&path).is_err(),
            "invalid schema-2 identity was silently regenerated"
        );
    }
}
