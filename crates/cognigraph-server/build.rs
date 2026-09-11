use std::collections::{BTreeSet, VecDeque};
use std::{env, fs, path::PathBuf};

use serde_json::Value;

#[path = "src/edition_paths.rs"]
mod edition_paths;

fn references(value: &Value, pending: &mut VecDeque<String>) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                pending.push_back(reference.to_owned());
            }
            for value in map.values() {
                references(value, pending);
            }
        }
        Value::Array(values) => {
            for value in values {
                references(value, pending);
            }
        }
        _ => {}
    }
}

fn main() {
    println!("cargo:rerun-if-changed=openapi.yaml");
    println!("cargo:rerun-if-changed=src/edition_paths.rs");
    let enterprise = env::var_os("CARGO_FEATURE_ENTERPRISE").is_some();
    let original = fs::read_to_string("openapi.yaml").expect("read OpenAPI source");
    let mut spec: Value = serde_saphyr::from_str(&original).expect("parse OpenAPI source");
    spec["info"]["version"] = env::var("CARGO_PKG_VERSION").unwrap().into();
    if !enterprise {
        spec["info"]["title"] = "CogniGraph Community".into();
        spec["info"]["description"] = "Single-tenant documents, graph traversal, CGQL, vector/text/hybrid search, cache, Lua, authentication and snapshots. Tenant lifecycle rejection endpoints return enterprise_feature_required and are not advertised as Community operations.".into();
        spec["paths"]
            .as_object_mut()
            .expect("OpenAPI paths")
            .retain(|path, _| !edition_paths::enterprise_path(path));
        let graph = &mut spec["paths"]["/api/search/graph-augmented"]["post"];
        graph["description"] = "Semantic seeds and multi-hop traversal with base edge ranking. Strong cache hits reuse graph facts; weak hits rerun live retrieval and traversal. Cache reuse requires identical result parameters.".into();
        graph["requestBody"]["content"]["application/json"]["schema"]["properties"]
            .as_object_mut()
            .unwrap()
            .remove("neurons_collection");
        // Keep only schemas/parameters reachable from the Community paths.
        let components = spec.as_object_mut().unwrap().remove("components").unwrap();
        let mut pending = VecDeque::new();
        references(&spec, &mut pending);
        let mut needed = BTreeSet::new();
        while let Some(reference) = pending.pop_front() {
            if !needed.insert(reference.clone()) {
                continue;
            }
            let pointer = reference
                .strip_prefix("#/components/")
                .expect("local OpenAPI reference");
            let value = components
                .pointer(&format!("/{pointer}"))
                .expect("resolvable OpenAPI reference");
            references(value, &mut pending);
        }
        let mut retained = components.clone();
        for (category, entries) in retained.as_object_mut().unwrap() {
            if category != "securitySchemes" {
                entries
                    .as_object_mut()
                    .unwrap()
                    .retain(|name, _| needed.contains(&format!("#/components/{category}/{name}")));
            }
        }
        spec["components"] = retained;
    }
    // JSON is valid YAML 1.2 and lets runtime consumers parse the same artifact
    // with either parser; no runtime YAML dependency is linked into the server.
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("openapi.json");
    fs::write(output, serde_json::to_string_pretty(&spec).unwrap()).unwrap();
}
