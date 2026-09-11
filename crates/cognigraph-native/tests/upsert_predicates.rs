use std::collections::HashMap;

use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use cognigraph_query::{QueryMode, parse_and_execute_backend_with_mode};
use serde_json::json;

#[tokio::test]
async fn keyed_and_scanned_upsert_apply_the_same_search_predicates() {
    for keyed in [false, true] {
        for (predicate, matches) in [
            (json!({"v": 1}), true),
            (json!({"v": 1.0}), true),
            (json!({"v": 999}), false),
            (json!({"nullable": null}), true),
            (json!({"absent": null}), false),
            (json!({"v": 1, "nullable": false}), false),
        ] {
            let backend = NativeBackend::new();
            let original = json!({"_key": "a", "v": 1, "nullable": null});
            backend.create_document("notes", original).await.unwrap();
            let before = backend.get_document("notes", "a").await.unwrap().unwrap();
            let mut search = predicate.clone();
            if keyed {
                search["_key"] = json!("a");
            }
            let rows = parse_and_execute_backend_with_mode(
                r#"UPSERT @search INSERT {_key: "c", v: 3} UPDATE {matched: true} IN notes RETURN NEW._key"#,
                &backend,
                &HashMap::from([("search".into(), search)]),
                QueryMode::ReadWrite,
            ).await.unwrap();
            assert_eq!(
                rows,
                vec![json!(if matches { "a" } else { "c" })],
                "keyed={keyed} predicate={predicate}"
            );
            let a = backend.get_document("notes", "a").await.unwrap().unwrap();
            if matches {
                assert_eq!(a["matched"], true);
                assert!(backend.get_document("notes", "c").await.unwrap().is_none());
            } else {
                assert_eq!(a, before, "nonmatching document must stay unchanged");
                assert_eq!(
                    backend.get_document("notes", "c").await.unwrap().unwrap()["v"].as_f64(),
                    Some(3.0)
                );
            }
        }
    }
}
