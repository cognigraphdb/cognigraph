//! Parsed schema checks and production-route probes for construction/job contracts.
use axum::{Router, body::Body, http::Request};
use cognigraph_native::NativeBackend;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::OnceLock};
use tower::ServiceExt;

use crate::{jobs, routes, state::AppState};

fn spec() -> &'static Value {
    static SPEC: OnceLock<Value> = OnceLock::new();
    SPEC.get_or_init(|| serde_saphyr::from_str(super::SPEC).expect("OpenAPI must be valid YAML"))
}
fn schema(name: &str) -> &'static Value {
    &spec()["components"]["schemas"][name]
}
fn strings(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect()
}
fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/construction")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

// Ask Serde for the actual wire variants. New enum variants therefore fail the
// spec comparison without maintaining a second hand-written Rust variant list.
struct EnumNames<'a>(&'a mut BTreeSet<String>);
impl<'de> serde::Deserializer<'de> for EnumNames<'_> {
    type Error = serde::de::value::Error;
    fn deserialize_enum<V: serde::de::Visitor<'de>>(
        self,
        _: &'static str,
        variants: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Self::Error> {
        self.0.extend(variants.iter().map(|v| (*v).to_string()));
        Err(serde::de::Error::custom("enum names captured"))
    }
    fn deserialize_any<V: serde::de::Visitor<'de>>(self, _: V) -> Result<V::Value, Self::Error> {
        Err(serde::de::Error::custom("expected enum"))
    }
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}
fn enum_names<T: DeserializeOwned>() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let _ = T::deserialize(EnumNames(&mut names));
    assert!(!names.is_empty());
    names
}

#[test]
fn construction_methods_and_openapi_parameter_response_shapes_stay_valid() {
    let documented = super::spec_method_map();
    for (prefix, source) in super::MODULES
        .iter()
        .filter(|(prefix, _)| ["/construct", "/jobs", "/sideviews"].contains(prefix))
    {
        for (path, expression) in super::route_calls(source) {
            let suffix = if path == "/" { "" } else { &path };
            assert_eq!(
                documented[&format!("/api{prefix}{suffix}")],
                super::route_methods(&expression)
            );
        }
    }
    for (path, item) in spec()["paths"].as_object().unwrap() {
        for method in ["get", "post", "put", "patch", "delete"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let parameters = item["parameters"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(operation["parameters"].as_array().into_iter().flatten())
                .collect::<Vec<_>>();
            for segment in path.split('/') {
                if let Some(name) = segment.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    assert!(
                        parameters.iter().any(|p| p["in"] == "path"
                            && p["name"] == name
                            && p["required"] == true),
                        "{method} {path}: missing {name}"
                    );
                }
            }
            for response in operation["responses"].as_object().unwrap().values() {
                // Unquoted commas in YAML flow descriptions used to become
                // accidental response fields while the path-only test passed.
                for key in response.as_object().unwrap().keys() {
                    assert!(
                        ["$ref", "description", "content", "headers", "links"]
                            .contains(&key.as_str())
                            || key.starts_with("x-"),
                        "{method} {path}: unexpected response field {key}"
                    );
                }
            }
        }
    }
}

#[test]
fn job_wire_enums_and_tagged_inputs_match_openapi() {
    let kinds = enum_names::<jobs::JobKind>();
    assert_eq!(strings(&schema("JobKind")["enum"]), kinds);
    assert_eq!(
        strings(&schema("JobStatus")["enum"]),
        enum_names::<jobs::JobStatus>()
    );
    let mut tagged = BTreeSet::new();
    for branch in schema("JobSubmissionRequest")["oneOf"].as_array().unwrap() {
        let request = spec()
            .pointer(branch["$ref"].as_str().unwrap().strip_prefix('#').unwrap())
            .unwrap();
        assert_eq!(
            strings(&request["required"]),
            BTreeSet::from(["input".into(), "kind".into()])
        );
        assert_eq!(request["additionalProperties"], false);
        tagged.extend(strings(&request["properties"]["kind"]["enum"]));
        assert!(request["properties"]["input"]["$ref"].as_str().is_some());
    }
    assert_eq!(tagged, kinds);
    assert!(
        schema("JobOperationInput")["oneOf"].is_null(),
        "ingest and draft inputs overlap"
    );
    assert_eq!(
        schema("JobOperationInput")["anyOf"]
            .as_array()
            .unwrap()
            .len(),
        kinds.len()
    );
    let progress = jobs::JobProgress {
        phase: "queued".into(),
        completed: 0,
        total: 0,
        facts_grounded: 0,
        side_views_written: 0,
    };
    let fields = |v: &Value| {
        v.as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        fields(&serde_json::to_value(progress).unwrap()),
        fields(&schema("JobProgress")["properties"])
    );
}

struct Stub;
#[async_trait::async_trait]
impl cognigraph_embeddings::completion::CompletionProvider for Stub {
    async fn complete_json(&self, _: &str, _: &str, _: &Value) -> anyhow::Result<Value> {
        Ok(json!({"facts": []}))
    }
    fn model_name(&self) -> &str {
        "cg22-synthetic"
    }
}
#[async_trait::async_trait]
impl cognigraph_embeddings::EmbeddingProvider for Stub {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>> {
        Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
    }
}
fn state() -> AppState {
    AppState::new(NativeBackend::new())
        .with_completion(Stub)
        .with_embedder(Stub)
        .with_sideviews_completion(std::sync::Arc::new(Stub))
}
async fn post(app: Router, path: &str, body: Value, key: Option<&str>) -> (u16, Value) {
    let mut request = Request::post(path).header("content-type", "application/json");
    if let Some(key) = key {
        request = request.header("Idempotency-Key", key);
    }
    let response = app
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let body =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)));
    (status, body)
}

#[tokio::test]
async fn documented_job_required_fields_and_unknown_field_policy_match_routes() {
    let state = state();
    // Seed accepted vocabulary through the existing typed construction route.
    let construct = routes::construct::router().with_state(state.clone());
    assert_eq!(
        post(construct, "/directed", fixture("directed.json"), None)
            .await
            .0,
        200
    );
    state
        .managed_backend
        .create_document("cg22-notes", json!({"_key":"n","text":"Synthetic note"}))
        .await
        .unwrap();
    let app = routes::jobs::router().with_state(state.clone());
    for (file, input_schema) in [
        ("ingest-job.json", "ConstructIngestJobInput"),
        ("evaluate-job.json", "ConstructEvaluateJobInput"),
        ("draft-job.json", "ConstructDraftJobInput"),
        ("sideviews-job.json", "SideviewsGenerateJobInput"),
    ] {
        let body = fixture(file);
        let branch = schema("JobSubmissionRequest")["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                spec()
                    .pointer(entry["$ref"].as_str().unwrap().strip_prefix('#').unwrap())
                    .unwrap()
            })
            .find(|entry| entry["properties"]["kind"]["enum"][0] == body["kind"])
            .unwrap();
        assert_eq!(
            branch["properties"]["input"]["$ref"],
            format!("#/components/schemas/{input_schema}")
        );
        let (status, submitted) = post(app.clone(), "/", body.clone(), Some(file)).await;
        assert_eq!(status, 202, "{file}: {submitted}");
        assert_eq!(submitted["job"]["kind"], body["kind"]);
        assert_eq!(schema(input_schema)["additionalProperties"], false);
        for field in strings(&schema(input_schema)["required"]) {
            let mut missing = body.clone();
            missing["input"].as_object_mut().unwrap().remove(&field);
            assert_eq!(
                post(app.clone(), "/", missing, Some(&format!("{file}-{field}")))
                    .await
                    .0,
                400
            );
        }
        let mut extra = body;
        extra["input"]["not_a_field"] = json!(true);
        assert_eq!(
            post(app.clone(), "/", extra, Some(&format!("{file}-extra")))
                .await
                .0,
            400
        );
    }
    state.jobs.shutdown().await;
}

#[tokio::test]
async fn directed_schema_bounds_and_required_fields_are_enforced_by_route() {
    let state = state();
    let app = routes::construct::router().with_state(state.clone());
    let request = schema("DirectedConstructRequest");
    assert_eq!(
        spec()["paths"]["/api/construct/directed"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"]["$ref"],
        "#/components/schemas/DirectedConstructRequest"
    );
    let max = request["properties"]["chunks"]["maxItems"]
        .as_u64()
        .unwrap() as usize;
    assert_eq!(request["properties"]["chunks"]["minItems"], 1);
    for count in [0, max, max + 1] {
        let mut body = fixture("directed.json");
        body["chunks"] = json!(
            (0..count)
                .map(|i| json!({"id":format!("c{i}"),"text":"Synthetic note"}))
                .collect::<Vec<_>>()
        );
        let (status, result) = post(app.clone(), "/directed", body, None).await;
        assert_eq!(
            status,
            if count == max { 200 } else { 400 },
            "count {count}: {result}"
        );
    }
    for field in strings(&request["required"]) {
        let mut body = fixture("directed.json");
        body.as_object_mut().unwrap().remove(&field);
        assert_eq!(post(app.clone(), "/directed", body, None).await.0, 422);
    }
    assert_eq!(request["additionalProperties"], false);
    let mut body = fixture("directed.json");
    body["async"] = json!(true);
    assert_eq!(post(app, "/directed", body, None).await.0, 422);
    state.jobs.shutdown().await;
}

#[tokio::test]
async fn sideview_clamping_defaults_and_draft_wrapper_match_schema() {
    let state = state();
    state
        .managed_backend
        .create_document("cg22-notes", json!({"_key":"n"}))
        .await
        .unwrap();
    let app = routes::sideviews::router().with_state(state.clone());
    let fields = &schema("SideviewsGenerateJobInput")["properties"];
    assert_eq!(fields["count"]["minimum"], 0);
    assert!(
        fields["count"]["maximum"].is_null(),
        "count is clamped, not rejected"
    );
    for (i, count) in [Value::Null, json!(0), json!(51)].into_iter().enumerate() {
        let (status, body) = post(
            app.clone(),
            "/generate",
            json!({"collection":"cg22-notes","count":count,"text_field":" "}),
            Some(&format!("count-{i}")),
        )
        .await;
        assert_eq!(status, 202, "{body}");
        let job = state
            .managed_backend
            .get_document(jobs::JOBS_COLLECTION, body["job"]["id"].as_str().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            job["_execution"]["count"],
            if count.is_null() {
                fields["count"]["default"].clone()
            } else if count == 0 {
                json!(1)
            } else {
                json!(50)
            }
        );
        assert_eq!(
            job["_execution"]["text_field"],
            fields["text_field"]["default"]
        );
        assert_eq!(
            job["_execution"]["regenerate"],
            fields["regenerate"]["default"]
        );
    }
    let app = routes::construct::router().with_state(state.clone());
    let body = fixture("draft-async.json");
    assert_eq!(
        schema("ConstructDraftRequest")["properties"]["async"]["default"],
        false
    );
    assert_eq!(post(app.clone(), "/draft", body.clone(), None).await.0, 400);
    let (status, result) = post(app.clone(), "/draft", body.clone(), Some("draft-wrapper")).await;
    assert_eq!(status, 202, "{result}");
    assert_eq!(result["job"]["kind"], "construct.draft");
    assert!(result["job"]["input"].get("async").is_none());
    assert!(result["job"]["input"].get("per_document").is_none());
    assert_eq!(
        post(app, "/draft", body, Some("draft-wrapper")).await.0,
        200
    );
    state.jobs.shutdown().await;
}
