//! Paths whose implementation requires the Enterprise build.

pub fn enterprise_path(path: &str) -> bool {
    [
        "/health/jobs",
        "/health/promotions",
        "/api/tenants",
        "/api/neurons",
        "/api/construct",
        "/api/sideviews",
        "/api/jobs",
        "/api/promotions",
        "/api/governance",
        "/api/semantic-repairs",
        "/api/admin/jobs",
        "/api/admin/promotions",
        "/api/admin/artifact-custody",
    ]
    .iter()
    .any(|prefix| {
        path == *prefix
            || path
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}
