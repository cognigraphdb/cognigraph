use std::sync::Arc;

use cognigraph_auth::{AuthProvider, Role};
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;

#[tokio::test]
async fn full_auth_lifecycle() {
    let backend = Arc::new(NativeBackend::new());
    let auth = AuthProvider::new(backend).await.unwrap();
    auth.bootstrap_admin("s3cret").await.unwrap();
    // Bootstrap is idempotent.
    auth.bootstrap_admin("other").await.unwrap();
    assert_eq!(auth.list_users().await.unwrap().len(), 1);

    let admin = auth.verify_password("admin", "s3cret").await.unwrap();
    assert_eq!(admin.role, Role::Admin);
    assert!(auth.verify_password("admin", "wrong").await.is_err());
    assert!(auth.verify_password("ghost", "s3cret").await.is_err());

    let viewer = auth.create_user("vera", "pw", Role::Viewer).await.unwrap();
    assert!(auth.create_user("vera", "pw", Role::Editor).await.is_err());

    let grant = auth.create_token(&viewer.key, "ci", None).await.unwrap();
    assert!(grant.token.starts_with("cg_"));
    assert!(grant.expires_at.is_none());
    let resolved = auth.validate_token(&grant.token).await.unwrap();
    assert_eq!(resolved.username, "vera");
    assert_eq!(resolved.role, Role::Viewer);
    assert!(auth.validate_token("cg_bogus").await.is_err());

    let tokens = auth.list_tokens(&viewer.key).await.unwrap();
    assert_eq!(tokens.len(), 1);
    assert!(!tokens[0].contains_key("token_hash"));
    assert!(tokens[0].contains_key("created_at"));
    assert!(tokens[0].contains_key("expires_at"));

    assert!(auth.revoke_token(&grant.key).await.unwrap());
    assert!(auth.validate_token(&grant.token).await.is_err());

    // Deleting a user revokes remaining tokens.
    let t2 = auth.create_token(&viewer.key, "x", None).await.unwrap();
    assert!(auth.delete_user(&viewer.key).await.unwrap());
    assert!(auth.validate_token(&t2.token).await.is_err());
}

#[tokio::test]
async fn token_expiry_and_rotation() {
    let backend = Arc::new(NativeBackend::new());
    let auth = AuthProvider::new(backend.clone()).await.unwrap();
    let user = auth.create_user("bot", "pw", Role::Viewer).await.unwrap();

    // TTL stamps an expiry; 0 means explicitly non-expiring.
    let short = auth
        .create_token(&user.key, "short", Some(3600))
        .await
        .unwrap();
    assert!(short.expires_at.is_some());
    assert!(auth.validate_token(&short.token).await.is_ok());
    let forever = auth
        .create_token(&user.key, "forever", Some(0))
        .await
        .unwrap();
    assert!(forever.expires_at.is_none());

    // Force the clock past the expiry by editing the stored record — an
    // expired token fails exactly like a revoked one, but its record
    // stays listed (flagged) for audit.
    backend
        .update_document(
            cognigraph_auth::TOKENS_COLLECTION,
            &short.key,
            serde_json::json!({ "expires_at": 1 }),
        )
        .await
        .unwrap();
    assert!(auth.validate_token(&short.token).await.is_err());
    let listed = auth.list_tokens(&user.key).await.unwrap();
    let flagged = listed
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("short"))
        .unwrap();
    assert_eq!(flagged.get("expired"), Some(&serde_json::json!(true)));

    // Rotation: same key, fresh secret and window; the old plaintext dies
    // immediately, the expired token comes back to life with a new secret.
    let rotated = auth.rotate_token(&short.key, Some(3600)).await.unwrap();
    assert_eq!(rotated.key, short.key);
    assert_ne!(rotated.token, short.token);
    assert!(auth.validate_token(&short.token).await.is_err());
    let revived = auth.validate_token(&rotated.token).await.unwrap();
    assert_eq!(revived.username, "bot");

    // Rotating with no TTL clears the expiry (fresh grant semantics).
    let cleared = auth.rotate_token(&short.key, None).await.unwrap();
    assert!(cleared.expires_at.is_none());
    assert!(auth.validate_token(&cleared.token).await.is_ok());

    assert!(auth.rotate_token("nope", None).await.is_err());
}
