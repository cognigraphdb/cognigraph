use super::*;
use crate::tenancy::{RoutedBackend, TenantContext, TenantRegistry};

#[tokio::test]
async fn tenant_and_incarnation_fences_are_independent() {
    let registry = Arc::new(TenantRegistry::new(
        Box::new(|_| Ok(Arc::new(NativeBackend::new()))),
        None,
    ));
    let state = AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())));
    let a = TenantContext::admitted("alpha".into(), "a1".into(), Some(&registry)).unwrap();
    let b = TenantContext::admitted("beta".into(), "b1".into(), Some(&registry)).unwrap();
    a.clone().scope(seed(&state, "notes", "a", 0)).await;
    b.clone().scope(seed(&state, "notes", "a", 0)).await;
    let old = a
        .clone()
        .scope(state.side_views.prepare(
            state.managed_backend.as_ref(),
            "alpha",
            "a1",
            DocumentId::new("notes", "a"),
            false,
        ))
        .await
        .unwrap()
        .unwrap();
    b.clone()
        .scope(state.backend.delete_document("notes", "a"))
        .await
        .unwrap();
    assert_eq!(
        a.clone()
            .scope(state.side_views.publish(
                state.managed_backend.as_ref(),
                old,
                surfaces("Alpha"),
                false
            ))
            .await
            .unwrap(),
        1
    );
    let old = a
        .clone()
        .scope(state.side_views.prepare(
            state.managed_backend.as_ref(),
            "alpha",
            "a1",
            DocumentId::new("notes", "a"),
            true,
        ))
        .await
        .unwrap()
        .unwrap();
    // Retiring/recreating a tenant does not move admitted old requests to its new store.
    registry.retire_store("alpha").unwrap();
    let new = TenantContext::admitted("alpha".into(), "a2".into(), Some(&registry)).unwrap();
    new.clone().scope(seed(&state, "notes", "a", 0)).await;
    let fresh = new
        .clone()
        .scope(state.side_views.prepare(
            state.managed_backend.as_ref(),
            "alpha",
            "a2",
            DocumentId::new("notes", "a"),
            false,
        ))
        .await
        .unwrap()
        .unwrap();
    a.clone()
        .scope(state.backend.delete_document("notes", "a"))
        .await
        .unwrap();
    assert!(
        a.scope(state.side_views.publish(
            state.managed_backend.as_ref(),
            old,
            surfaces("Old"),
            false
        ))
        .await
        .is_err()
    );
    assert_eq!(
        new.clone()
            .scope(state.side_views.publish(
                state.managed_backend.as_ref(),
                fresh,
                surfaces("New"),
                false
            ))
            .await
            .unwrap(),
        1
    );
    assert_eq!(new.scope(rows(&state)).await.len(), 1);
    assert!(b.scope(rows(&state)).await.is_empty());
}
