//! H3: OpenAPI drift test. The spec is hand-maintained and axum routers
//! don't expose their route tables, so this check is source-derived:
//! every `.nest("/prefix", …)` in main.rs plus every `.route("…", …)` in
//! the module that prefix maps to yields a full path, and the set must
//! correspond BOTH WAYS with the `paths:` section of openapi.yaml.
//! The nest→file mapping below is asserted complete against main.rs, so
//! adding a new router module without updating this test fails loudly.

/// (nest prefix, module source) — extend when mounting a new router.
const MODULES: &[(&str, &str)] = &[
    ("/health", include_str!("routes/health.rs")),
    ("/auth", include_str!("routes/auth.rs")),
    ("/cache", include_str!("routes/cache.rs")),
    ("/users", include_str!("routes/users.rs")),
    ("/admin", include_str!("routes/admin.rs")),
    ("/tenants", include_str!("routes/tenants.rs")),
    ("/documents", include_str!("routes/documents.rs")),
    ("/collections", include_str!("routes/collections.rs")),
    ("/graph", include_str!("routes/graph.rs")),
    ("/neurons", include_str!("routes/neurons.rs")),
    ("/construct", include_str!("routes/construct/routing.rs")),
    ("/jobs", include_str!("routes/jobs.rs")),
    ("/promotions", include_str!("routes/promotions.rs")),
    ("/governance", include_str!("routes/governance.rs")),
    (
        "/semantic-repairs",
        include_str!("routes/semantic_repairs.rs"),
    ),
    ("/sideviews", include_str!("routes/sideviews.rs")),
    ("/search", include_str!("routes/search/mod.rs")),
    ("/batch", include_str!("routes/batch.rs")),
    ("/lua", include_str!("routes/lua.rs")),
    ("/query", include_str!("routes/query.rs")),
];

/// Router fragments merged by one of the primary modules rather than nested
/// directly in `main.rs`. They share the parent's mount prefix but remain a
/// separate source file for bounded feature ownership.
const MERGED_MODULES: &[(&str, &str)] = &[(
    "/semantic-repairs",
    include_str!("routes/materialized_repairs.rs"),
)];

const MAIN: &str = include_str!("main.rs");
const SPEC: &str = crate::edition::OPENAPI;
#[path = "edition_paths.rs"]
mod edition_paths;

#[cfg(feature = "enterprise")]
mod contracts;

#[cfg(feature = "enterprise")]
struct M18Route {
    full_path: &'static str,
    local_path: &'static str,
    source: &'static str,
    methods: &'static [&'static str],
}

/// M18 is deliberately checked at method level as well as by the general
/// path-set test below. Promotion mutations carry governance semantics that a
/// stale GET/POST declaration would materially misrepresent.
#[cfg(feature = "enterprise")]
const M18_ROUTES: &[M18Route] = &[
    M18Route {
        full_path: "/health/promotions",
        local_path: "/promotions",
        source: include_str!("routes/health.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/promotions/evidence",
        local_path: "/evidence",
        source: include_str!("routes/promotions.rs"),
        methods: &["get", "post"],
    },
    M18Route {
        full_path: "/api/promotions/evidence/{id}",
        local_path: "/evidence/{id}",
        source: include_str!("routes/promotions.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/promotions/evidence/{id}/promote",
        local_path: "/evidence/{id}/promote",
        source: include_str!("routes/promotions.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/promotions/evidence/{id}/reject",
        local_path: "/evidence/{id}/reject",
        source: include_str!("routes/promotions.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/promotions/decisions",
        local_path: "/decisions",
        source: include_str!("routes/promotions.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/promotions/decisions/{id}",
        local_path: "/decisions/{id}",
        source: include_str!("routes/promotions.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/promotions/current/{space_type}/{channel}",
        local_path: "/current/{space_type}/{channel}",
        source: include_str!("routes/promotions.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/promotions/current/{space_type}/{channel}/rollback",
        local_path: "/current/{space_type}/{channel}/rollback",
        source: include_str!("routes/promotions.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/admin/promotions/status",
        local_path: "/promotions/status",
        source: include_str!("routes/admin.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/admin/promotions/reconcile",
        local_path: "/promotions/reconcile",
        source: include_str!("routes/admin.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/admin/promotions/recover",
        local_path: "/promotions/recover",
        source: include_str!("routes/admin.rs"),
        methods: &["post"],
    },
];

/// M19 trust-registry and policy-authority routes are method checked because a
/// stale read/write declaration would misrepresent role and signing duties.
#[cfg(feature = "enterprise")]
const M19_ROUTES: &[M18Route] = &[
    M18Route {
        full_path: "/api/governance/status",
        local_path: "/status",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/governance/keys",
        local_path: "/keys",
        source: include_str!("routes/governance.rs"),
        methods: &["get", "post"],
    },
    M18Route {
        full_path: "/api/governance/keys/{id}",
        local_path: "/keys/{id}",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/governance/keys/{id}/revoke",
        local_path: "/keys/{id}/revoke",
        source: include_str!("routes/governance.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/governance/revocations",
        local_path: "/revocations",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/governance/revocations/{id}",
        local_path: "/revocations/{id}",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/governance/policies",
        local_path: "/policies",
        source: include_str!("routes/governance.rs"),
        methods: &["get", "post"],
    },
    M18Route {
        full_path: "/api/governance/policies/{id}",
        local_path: "/policies/{id}",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/governance/policies/{id}/approve",
        local_path: "/policies/{id}/approve",
        source: include_str!("routes/governance.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/governance/approvals/{id}",
        local_path: "/approvals/{id}",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/governance/bindings/{approval_id}",
        local_path: "/bindings/{approval_id}",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
];

/// M20 external-artifact authority routes are method checked because their
/// signed write path and read-only active-binding resolver have materially
/// different authorization and trust semantics.
#[cfg(feature = "enterprise")]
const M20_ROUTES: &[M18Route] = &[
    M18Route {
        full_path: "/api/governance/artifact-attestations",
        local_path: "/artifact-attestations",
        source: include_str!("routes/governance.rs"),
        methods: &["get", "post"],
    },
    M18Route {
        full_path: "/api/governance/artifact-attestations/{id}",
        local_path: "/artifact-attestations/{id}",
        source: include_str!("routes/governance.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/governance/artifact-bindings/resolve",
        local_path: "/artifact-bindings/resolve",
        source: include_str!("routes/governance.rs"),
        methods: &["post"],
    },
];

/// M24 recovery-plan derivation is deliberately GET-only: it projects signed
/// historical authority and must not imply a server-side backup mutation.
#[cfg(feature = "enterprise")]
const M24_ROUTES: &[M18Route] = &[M18Route {
    full_path: "/api/admin/artifact-custody/evidence/{id}",
    local_path: "/artifact-custody/evidence/{id}",
    source: include_str!("routes/admin.rs"),
    methods: &["get"],
}];

/// M25 signed Semantic Repair routes are checked at method level because
/// PolicyAuthor submission, independent PolicyApprover review, and governed
/// read resolution are intentionally distinct authority operations.
#[cfg(feature = "enterprise")]
const M25_ROUTES: &[M18Route] = &[
    M18Route {
        full_path: "/api/construct/governed-ingest",
        local_path: "/governed-ingest",
        source: include_str!("routes/construct/routing.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/revisions",
        local_path: "/revisions",
        source: include_str!("routes/semantic_repairs.rs"),
        methods: &["get", "post"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/revisions/{id}",
        local_path: "/revisions/{id}",
        source: include_str!("routes/semantic_repairs.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/revisions/{id}/review",
        local_path: "/revisions/{id}/review",
        source: include_str!("routes/semantic_repairs.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/reviews",
        local_path: "/reviews",
        source: include_str!("routes/semantic_repairs.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/reviews/{id}",
        local_path: "/reviews/{id}",
        source: include_str!("routes/semantic_repairs.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/current/{space_type}/{channel}",
        local_path: "/current/{space_type}/{channel}",
        source: include_str!("routes/semantic_repairs.rs"),
        methods: &["get"],
    },
];

#[cfg(feature = "enterprise")]
const M26_ROUTES: &[M18Route] = &[
    M18Route {
        full_path: "/api/semantic-repairs/generations",
        local_path: "/generations",
        source: include_str!("routes/materialized_repairs.rs"),
        methods: &["get", "post"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/generations/{id}",
        local_path: "/generations/{id}",
        source: include_str!("routes/materialized_repairs.rs"),
        methods: &["get"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/generations/{id}/deploy",
        local_path: "/generations/{id}/deploy",
        source: include_str!("routes/materialized_repairs.rs"),
        methods: &["post"],
    },
    M18Route {
        full_path: "/api/semantic-repairs/deployments/current/{space_type}",
        local_path: "/deployments/current/{space_type}",
        source: include_str!("routes/materialized_repairs.rs"),
        methods: &["get"],
    },
];

/// First string-literal argument of each `pattern(…)` call in `source`.
/// rustfmt may break the line after the paren, so whitespace between the
/// paren and the opening quote is tolerated.
fn literal_args(source: &str, pattern: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(pattern) {
        rest = rest[at + pattern.len()..].trim_start();
        if let Some(arg) = rest.strip_prefix('"')
            && let Some(end) = arg.find('"')
        {
            out.push(arg[..end].to_string());
            rest = &arg[end..];
        }
    }
    out
}

/// Extract `(path, route-expression)` pairs while respecting nested handler
/// combinators such as `post(handler).get(other)`. This is intentionally small
/// and source-oriented; it only needs to understand ordinary Rust strings and
/// balanced parentheses used by the router declarations in this crate.
#[cfg(feature = "enterprise")]
fn route_calls(source: &str) -> Vec<(String, String)> {
    const PATTERN: &str = ".route(";
    let mut calls = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(PATTERN) {
        let arguments = &rest[at + PATTERN.len()..];
        let trimmed = arguments.trim_start();
        let Some(literal) = trimmed.strip_prefix('"') else {
            rest = &arguments[1..];
            continue;
        };
        let Some(path_end) = literal.find('"') else {
            break;
        };
        let path = literal[..path_end].to_string();

        let mut depth = 1_usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut call_end = None;
        for (index, character) in arguments.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
                continue;
            }
            match character {
                '"' => in_string = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        call_end = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(call_end) = call_end else {
            break;
        };
        calls.push((path, arguments[..call_end].to_string()));
        rest = &arguments[call_end + 1..];
    }
    calls
}

#[cfg(feature = "enterprise")]
fn route_methods(expression: &str) -> std::collections::BTreeSet<String> {
    ["delete", "get", "patch", "post", "put"]
        .into_iter()
        .filter(|method| expression.contains(&format!("{method}(")))
        .map(str::to_string)
        .collect()
}

/// The application API is mounted under this prefix; `/health` stays at root.
const API_PREFIX: &str = "/api";

/// A module's mount base: root for the operational `/health` router, `/api` for
/// every application router (which main.rs nests inside the `/api` sub-router).
fn base_for(prefix: &str) -> &'static str {
    if prefix == "/health" { "" } else { API_PREFIX }
}

fn mounted_paths() -> Vec<String> {
    let mut paths = Vec::new();
    for (prefix, source) in MODULES {
        let base = base_for(prefix);
        for route in literal_args(source, ".route(") {
            let full = if route == "/" {
                format!("{base}{prefix}")
            } else {
                format!("{base}{prefix}{route}")
            };
            paths.push(full);
        }
    }
    for (prefix, source) in MERGED_MODULES {
        let base = base_for(prefix);
        for route in literal_args(source, ".route(") {
            paths.push(format!("{base}{prefix}{route}"));
        }
    }
    // Top-level routes registered directly in main.rs (`/openapi.yaml`,
    // `/metrics`) — deliberately at root, not under `/api`.
    for route in literal_args(MAIN, ".route(") {
        paths.push(route);
    }
    if !cfg!(feature = "enterprise") {
        paths.retain(|path| !edition_paths::enterprise_path(path));
    }
    paths.sort();
    paths.dedup();
    paths
}

fn spec_paths() -> Vec<String> {
    let spec: serde_json::Value = serde_json::from_str(SPEC).unwrap();
    spec["paths"].as_object().unwrap().keys().cloned().collect()
}

#[cfg(feature = "enterprise")]
fn spec_method_map() -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    let spec: serde_json::Value = serde_json::from_str(SPEC).unwrap();
    spec["paths"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(path, item)| {
            let methods = item
                .as_object()
                .unwrap()
                .keys()
                .filter(|m| {
                    ["get", "post", "put", "patch", "delete", "head", "options"]
                        .contains(&m.as_str())
                })
                .cloned()
                .collect();
            (path.clone(), methods)
        })
        .collect()
}

#[test]
fn nest_mapping_is_complete() {
    // Every .nest in main.rs must have an entry in MODULES — otherwise a
    // new router module would silently escape the drift check. The `/api`
    // wrapper is the structural mount point for the application routers, not a
    // module of its own, so it is excluded.
    let nested: Vec<String> = literal_args(MAIN, ".nest(")
        .into_iter()
        .filter(|prefix| prefix != API_PREFIX)
        .collect();
    for prefix in &nested {
        assert!(
            MODULES.iter().any(|(p, _)| p == prefix),
            "main.rs nests `{prefix}` but openapi_drift::MODULES does not list it — add the module"
        );
    }
    assert_eq!(
        nested.len(),
        MODULES.len(),
        "MODULES lists a prefix main.rs no longer nests"
    );
}

#[test]
fn extraction_is_not_vacuous() {
    // If a style change ever breaks both extractors the same way, the
    // directional checks would pass on empty sets. 30 paths exist today.
    assert!(mounted_paths().len() >= 25, "route extraction broke");
    assert!(spec_paths().len() >= 25, "spec path parsing broke");
}

#[test]
fn every_mounted_route_is_in_the_spec() {
    let spec = spec_paths();
    let missing: Vec<String> = mounted_paths()
        .into_iter()
        .filter(|path| !spec.contains(path))
        .collect();
    assert!(
        missing.is_empty(),
        "mounted routes absent from openapi.yaml: {missing:?}"
    );
}

#[test]
fn every_spec_path_is_mounted() {
    let mounted = mounted_paths();
    let stale: Vec<String> = spec_paths()
        .into_iter()
        .filter(|path| !mounted.contains(path))
        .collect();
    assert!(
        stale.is_empty(),
        "openapi.yaml documents paths no code mounts: {stale:?}"
    );
}

#[test]
#[cfg(feature = "enterprise")]
fn m18_route_methods_match_source_and_spec() {
    let documented = spec_method_map();
    let expected_paths = M18_ROUTES
        .iter()
        .map(|route| route.full_path.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let documented_m18_paths = documented
        .keys()
        .filter(|path| {
            path.as_str() == "/health/promotions"
                || path.starts_with("/api/promotions/")
                || path.starts_with("/api/admin/promotions/")
        })
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        documented_m18_paths, expected_paths,
        "M18 path inventory changed; update the method-level contract table"
    );

    for route in M18_ROUTES {
        let expression = route_calls(route.source)
            .into_iter()
            .find_map(|(path, expression)| (path == route.local_path).then_some(expression))
            .unwrap_or_else(|| {
                panic!(
                    "M18 source route `{}` is not registered for `{}`",
                    route.local_path, route.full_path
                )
            });
        let expected = route
            .methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            route_methods(&expression),
            expected,
            "M18 source methods drifted for {}",
            route.full_path
        );
        assert_eq!(
            documented.get(route.full_path),
            Some(&expected),
            "M18 OpenAPI methods drifted for {}",
            route.full_path
        );
    }
}

#[test]
#[cfg(feature = "enterprise")]
fn m19_governance_route_methods_match_source_and_spec() {
    let documented = spec_method_map();
    let expected_paths = M19_ROUTES
        .iter()
        .map(|route| route.full_path.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let documented_m19_paths = documented
        .keys()
        .filter(|path| {
            path.starts_with("/api/governance/") && !path.starts_with("/api/governance/artifact-")
        })
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        documented_m19_paths, expected_paths,
        "M19 governance path inventory changed; update the method-level contract table"
    );

    for route in M19_ROUTES {
        let expression = route_calls(route.source)
            .into_iter()
            .find_map(|(path, expression)| (path == route.local_path).then_some(expression))
            .unwrap_or_else(|| {
                panic!(
                    "M19 source route `{}` is not registered for `{}`",
                    route.local_path, route.full_path
                )
            });
        let expected = route
            .methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            route_methods(&expression),
            expected,
            "M19 source methods drifted for {}",
            route.full_path
        );
        assert_eq!(
            documented.get(route.full_path),
            Some(&expected),
            "M19 OpenAPI methods drifted for {}",
            route.full_path
        );
    }
}

#[test]
#[cfg(feature = "enterprise")]
fn m20_artifact_governance_route_methods_match_source_and_spec() {
    let documented = spec_method_map();
    let expected_paths = M20_ROUTES
        .iter()
        .map(|route| route.full_path.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let documented_m20_paths = documented
        .keys()
        .filter(|path| path.starts_with("/api/governance/artifact-"))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        documented_m20_paths, expected_paths,
        "M20 artifact-governance path inventory changed; update the method-level contract table"
    );

    for route in M20_ROUTES {
        let expression = route_calls(route.source)
            .into_iter()
            .find_map(|(path, expression)| (path == route.local_path).then_some(expression))
            .unwrap_or_else(|| {
                panic!(
                    "M20 source route `{}` is not registered for `{}`",
                    route.local_path, route.full_path
                )
            });
        let expected = route
            .methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            route_methods(&expression),
            expected,
            "M20 source methods drifted for {}",
            route.full_path
        );
        assert_eq!(
            documented.get(route.full_path),
            Some(&expected),
            "M20 OpenAPI methods drifted for {}",
            route.full_path
        );
    }
}

#[test]
#[cfg(feature = "enterprise")]
fn m24_artifact_custody_route_methods_match_source_and_spec() {
    let documented = spec_method_map();
    for route in M24_ROUTES {
        let expression = route_calls(route.source)
            .into_iter()
            .find_map(|(path, expression)| (path == route.local_path).then_some(expression))
            .unwrap_or_else(|| {
                panic!(
                    "M24 source route `{}` is not registered for `{}`",
                    route.local_path, route.full_path
                )
            });
        let expected = route
            .methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            route_methods(&expression),
            expected,
            "M24 source methods drifted for {}",
            route.full_path
        );
        assert_eq!(
            documented.get(route.full_path),
            Some(&expected),
            "M24 OpenAPI methods drifted for {}",
            route.full_path
        );
    }
}

#[test]
#[cfg(feature = "enterprise")]
fn m25_semantic_repair_route_methods_match_source_and_spec() {
    let documented = spec_method_map();
    let expected_paths = M25_ROUTES
        .iter()
        .map(|route| route.full_path.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let documented_m25_paths = documented
        .keys()
        .filter(|path| {
            path.starts_with("/api/semantic-repairs/revisions")
                || path.starts_with("/api/semantic-repairs/reviews")
                || path.starts_with("/api/semantic-repairs/current/")
                || path.as_str() == "/api/construct/governed-ingest"
        })
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        documented_m25_paths, expected_paths,
        "M25 Semantic Repair path inventory changed; update the method-level contract table"
    );

    for route in M25_ROUTES {
        let expression = route_calls(route.source)
            .into_iter()
            .find_map(|(path, expression)| (path == route.local_path).then_some(expression))
            .unwrap_or_else(|| {
                panic!(
                    "M25 source route `{}` is not registered for `{}`",
                    route.local_path, route.full_path
                )
            });
        let expected = route
            .methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            route_methods(&expression),
            expected,
            "M25 source methods drifted for {}",
            route.full_path
        );
        assert_eq!(
            documented.get(route.full_path),
            Some(&expected),
            "M25 OpenAPI methods drifted for {}",
            route.full_path
        );
    }
}

#[test]
#[cfg(feature = "enterprise")]
fn m26_materialized_repair_route_methods_match_source_and_spec() {
    let documented = spec_method_map();
    let expected_paths = M26_ROUTES
        .iter()
        .map(|route| route.full_path.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let documented_m26_paths = documented
        .keys()
        .filter(|path| {
            path.starts_with("/api/semantic-repairs/generations")
                || path.starts_with("/api/semantic-repairs/deployments/")
        })
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        documented_m26_paths, expected_paths,
        "M26 materialized Semantic Repair path inventory changed"
    );
    for route in M26_ROUTES {
        let expression = route_calls(route.source)
            .into_iter()
            .find_map(|(path, expression)| (path == route.local_path).then_some(expression))
            .unwrap_or_else(|| panic!("M26 source route `{}` is not registered", route.local_path));
        let expected = route
            .methods
            .iter()
            .map(|method| (*method).to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(route_methods(&expression), expected);
        assert_eq!(documented.get(route.full_path), Some(&expected));
    }
}
