use std::sync::Arc;

use cognigraph_auth::{AuthProvider, Role, USERS_COLLECTION};
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::json;

// Generated with Argon2 0.5.3, its default parameters, and a synthetic fixed salt.
const LEGACY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c3ludGhldGljLWNnODMhISE$j2Z+qnyr7YSQbAmYYVkpBLEhwltZJ5i/yhJEIDllOBc";

#[tokio::test]
async fn legacy_phc_passwords_verify_without_rewriting_the_stored_hash() {
    let backend = Arc::new(NativeBackend::new());
    let auth = AuthProvider::new(backend.clone()).await.unwrap();
    let id = backend
        .create_document(
            USERS_COLLECTION,
            json!({"username": "legacy", "password_hash": LEGACY_HASH,
                   "role": "viewer", "tenant": "default"}),
        )
        .await
        .unwrap();
    let user = auth
        .verify_password("legacy", "legacy-CG83-Δ!")
        .await
        .unwrap();
    assert_eq!(user.role, Role::Viewer);
    assert!(auth.verify_password("legacy", "wrong").await.is_err());
    let document = backend
        .get_document(USERS_COLLECTION, &id.key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(document["password_hash"], LEGACY_HASH);

    backend
        .update_document(
            USERS_COLLECTION,
            &id.key,
            json!({"password_hash": "invalid"}),
        )
        .await
        .unwrap();
    assert!(
        auth.verify_password("legacy", "legacy-CG83-Δ!")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn new_hashes_keep_phc_parameters_and_use_independent_salts() {
    let backend = Arc::new(NativeBackend::new());
    let auth = AuthProvider::new(backend.clone()).await.unwrap();
    let mut hashes = Vec::new();
    for name in ["first", "second"] {
        let user = auth
            .create_user(name, "same-password", Role::Viewer)
            .await
            .unwrap();
        auth.verify_password(name, "same-password").await.unwrap();
        let doc = backend
            .get_document(USERS_COLLECTION, &user.key)
            .await
            .unwrap()
            .unwrap();
        let hash = doc["password_hash"].as_str().unwrap().to_owned();
        assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        hashes.push(hash);
    }
    assert_ne!(hashes[0], hashes[1]);
}
