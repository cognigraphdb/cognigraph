use std::collections::HashMap;

use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use cognigraph_query::{QueryMode, parse_and_execute_backend_with_mode};
use serde_json::{Value, json};

#[tokio::test]
async fn unsupported_mutation_reads_never_modify_documents() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../cognigraph-query/tests/fixtures/mutation_backend_reads.json"
    ))
    .unwrap();
    let backend = NativeBackend::new();
    for (key, v) in [("a", 1), ("b", 2)] {
        backend
            .create_document("notes", json!({"_key":key,"v":v}))
            .await
            .unwrap();
    }
    let before = backend.list_documents("notes", None, None).await.unwrap();
    for reference in ["notes/b", "notes/absent"] {
        for case in &cases {
            let query = case["query"].as_str().unwrap();
            let result = parse_and_execute_backend_with_mode(
                query,
                &backend,
                &HashMap::from([("ref".into(), json!(reference))]),
                QueryMode::ReadWrite,
            )
            .await;
            assert_eq!(
                result.unwrap_err().to_string(),
                case["error"].as_str().unwrap(),
                "{}: {reference}",
                case["name"]
            );
            assert_eq!(
                backend.list_documents("notes", None, None).await.unwrap(),
                before
            );
        }
    }
}
