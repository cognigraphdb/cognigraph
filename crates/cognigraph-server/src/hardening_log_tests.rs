//! CG-108: who may see which recent error event, and what is redacted.

use super::*;

fn event(tenant: Option<&str>, path: &str) -> ErrorEvent {
    ErrorEvent {
        seq: 0,
        at: 0,
        method: "DELETE".into(),
        path: path.into(),
        status: 403,
        latency_ms: 1,
        message: "rejected".into(),
        tenant: tenant.map(str::to_string),
    }
}

fn paths(events: &[ErrorEvent]) -> Vec<&str> {
    events.iter().map(|event| event.path.as_str()).collect()
}

#[test]
fn a_tenant_sees_its_own_events_in_full_and_never_another_tenants() {
    let errors = RecentErrors::new(16);
    errors.record(event(Some("acme"), "/api/documents/acme_secret/x"));
    errors.record(event(Some("beta"), "/api/documents/patients/p-123"));
    errors.record(event(None, "/api/documents/unknown_caller/doc-9"));
    errors.record(event(None, "/api/auth/login"));

    for redact in [false, true] {
        let seen = errors.snapshot(Some("acme"), redact);
        assert!(paths(&seen).contains(&"/api/documents/acme_secret/x"));
        assert!(
            !paths(&seen).iter().any(|path| path.contains("patients")),
            "{:?}",
            paths(&seen)
        );
    }
}

#[test]
fn unattributed_events_are_redacted_only_on_a_multi_tenant_server() {
    let errors = RecentErrors::new(16);
    errors.record(event(None, "/api/documents/unknown_caller/doc-9"));
    errors.record(event(None, "/api/auth/login"));

    // Single tenant: the admin is the operator and sees the request as made.
    assert_eq!(
        paths(&errors.snapshot(Some("default"), false)),
        ["/api/auth/login", "/api/documents/unknown_caller/doc-9"]
    );
    // Multi-tenant: the route stays, what was asked of it does not.
    let redacted = errors.snapshot(Some("acme"), true);
    assert_eq!(paths(&redacted), ["/api/auth/…", "/api/documents/…"]);
    // Everything else about the event is kept.
    assert_eq!(redacted[1].status, 403);
    assert_eq!(redacted[1].method, "DELETE");
    // With authentication off there is no caller tenant and nothing to hide.
    assert_eq!(
        paths(&errors.snapshot(None, true)),
        ["/api/auth/login", "/api/documents/unknown_caller/doc-9"]
    );
    // The stored event is not altered by a redacted read.
    assert_eq!(
        paths(&errors.snapshot(None, false))[1],
        "/api/documents/unknown_caller/doc-9"
    );
}

#[test]
fn redaction_keeps_the_route_and_drops_the_rest() {
    for (path, expected) in [
        ("/api/documents/patients/p-123", "/api/documents/…"),
        ("/api/documents/patients", "/api/documents/…"),
        ("/api/search/text", "/api/search/…"),
        ("/api/auth/login", "/api/auth/…"),
        ("/api/auth", "/api/auth"),
        ("/api/documents", "/api/documents"),
        ("/health", "/health"),
        ("/", "/"),
        ("", ""),
        ("//api//documents//secret", "/api/documents/…"),
        ("/api/documents/a/b/c/d", "/api/documents/…"),
    ] {
        assert_eq!(redacted_path(path), expected, "{path}");
    }
}
