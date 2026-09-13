//! Single-tenant context, without store registries or tenant routing.

pub fn current_tenant() -> String {
    cognigraph_auth::DEFAULT_TENANT.to_owned()
}

#[derive(Clone)]
pub struct TenantContext;

impl TenantContext {
    pub fn capture(tenant: String) -> Self {
        assert_eq!(tenant, cognigraph_auth::DEFAULT_TENANT);
        Self
    }

    pub async fn scope<F: std::future::Future>(self, future: F) -> F::Output {
        future.await
    }
}
