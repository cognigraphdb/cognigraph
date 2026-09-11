//! Thin blocking HTTP wrapper over the server API: adds the bearer token,
//! surfaces non-2xx responses as readable errors, parses JSON.

use serde_json::Value;

pub struct Api {
    base: String,
    token: Option<String>,
    http: reqwest::blocking::Client,
}

impl Api {
    pub fn new(base: String, token: Option<String>) -> Self {
        Self {
            base,
            token,
            http: reqwest::blocking::Client::new(),
        }
    }

    /// GET an application endpoint (served under `/api`).
    pub fn get(&self, path: &str) -> Result<Value, String> {
        self.send(self.http.get(self.api_url(path)))
    }

    /// GET an operational endpoint that lives at the server root
    /// (`/health`, `/health/database`, `/metrics`).
    pub fn get_root(&self, path: &str) -> Result<Value, String> {
        self.send(self.http.get(self.root_url(path)))
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.send(self.http.post(self.api_url(path)).json(body))
    }

    /// POST a durable operation with the caller-provided replay key.
    #[cfg(feature = "enterprise")]
    pub fn post_idempotent(
        &self,
        path: &str,
        body: &Value,
        idempotency_key: &str,
    ) -> Result<Value, String> {
        self.send(self.idempotent_post_request(path, body, idempotency_key))
    }

    pub fn delete(&self, path: &str) -> Result<Value, String> {
        self.send(self.http.delete(self.api_url(path)))
    }

    /// COGNIGRAPH_URL stays the plain server base; the application API
    /// lives under `/api` and the prefix is applied here, at the one
    /// URL choke point, so call sites keep their route-relative paths.
    fn api_url(&self, path: &str) -> String {
        format!("{}/api{path}", self.base.trim_end_matches('/'))
    }

    fn root_url(&self, path: &str) -> String {
        format!("{}{path}", self.base.trim_end_matches('/'))
    }

    #[cfg(any(test, feature = "enterprise"))]
    fn idempotent_post_request(
        &self,
        path: &str,
        body: &Value,
        idempotency_key: &str,
    ) -> reqwest::blocking::RequestBuilder {
        self.http
            .post(self.api_url(path))
            .header("Idempotency-Key", idempotency_key)
            .json(body)
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
        let request = match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        };
        let response = request.send().map_err(|e| format!("request failed: {e}"))?;
        let status = response.status();
        let body: Value = response
            .json()
            .unwrap_or_else(|_| Value::String("<non-JSON response>".into()));
        if !status.is_success() {
            let detail = body
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| body.to_string());
            return Err(format!("HTTP {status}: {detail}"));
        }
        Ok(body)
    }
}

/// Password source for `login` / `user create`: COGNIGRAPH_PASSWORD, or a
/// single line from stdin (pipeable; the prompt goes to stderr so stdout
/// stays clean for scripting).
pub fn read_password() -> Result<String, String> {
    if let Ok(password) = std::env::var("COGNIGRAPH_PASSWORD")
        && !password.is_empty()
    {
        return Ok(password);
    }
    eprint!("password: ");
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("failed to read password: {e}"))?;
    let password = line.trim_end_matches(['\r', '\n']).to_string();
    if password.is_empty() {
        return Err("empty password (set COGNIGRAPH_PASSWORD or pipe one line)".into());
    }
    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the CLI kept requesting application endpoints at the
    /// server root after the API moved under `/api`, so commands received
    /// the console's index.html (or a 404) instead of API JSON.
    #[test]
    fn application_paths_get_the_api_prefix_and_health_stays_at_root() {
        let api = Api::new("http://localhost:3000/".into(), None);
        assert_eq!(
            api.api_url("/admin/export"),
            "http://localhost:3000/api/admin/export"
        );
        assert_eq!(
            api.api_url("/documents?collection=docs"),
            "http://localhost:3000/api/documents?collection=docs"
        );
        assert_eq!(api.root_url("/health"), "http://localhost:3000/health");
    }

    #[test]
    fn idempotent_posts_carry_the_replay_key() {
        let api = Api::new("http://localhost:3000".into(), None);
        let request = api
            .idempotent_post_request(
                "/jobs",
                &serde_json::json!({ "kind": "construct.evaluate", "input": {} }),
                "evaluation-42",
            )
            .build()
            .unwrap();

        assert_eq!(request.url().as_str(), "http://localhost:3000/api/jobs");
        assert_eq!(
            request
                .headers()
                .get("idempotency-key")
                .and_then(|value| value.to_str().ok()),
            Some("evaluation-42")
        );
    }
}
