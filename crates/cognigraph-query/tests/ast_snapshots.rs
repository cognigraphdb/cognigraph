use cognigraph_query::{parse_query, validate_query};
use serde_json::json;

fn parsed_json(query: &str) -> serde_json::Value {
    let parsed = parse_query(query).unwrap();
    validate_query(&parsed).unwrap();
    serde_json::to_value(parsed).unwrap()
}

#[test]
fn collection_query_ast_shape() {
    let ast = parsed_json(
        r#"
        FOR d IN documents
        FILTER d.category == @category
        SORT d.created_at DESC
        LIMIT 20
        RETURN { id: d._id, title: d.title }
        "#,
    );

    assert_eq!(
        ast,
        json!({
            "explain": false,
            "analyze": false,
            "body": [
                {
                    "For": {
                        "Collection": {
                            "var": "d",
                            "collection": "documents"
                        }
                    }
                },
                {
                    "Filter": {
                        "Binary": {
                            "left": { "Identifier": ["d", "category"] },
                            "op": "Eq",
                            "right": { "BindVar": "category" }
                        }
                    }
                }
            ],
            "distinct": false,
            "collect": null,
            "mutation": null,
            "sort": {
                "collation": null,
                "keys": [{
                    "expr": { "Identifier": ["d", "created_at"] },
                    "direction": "Desc"
                }]
            },
            "limit": {
                "offset": null,
                "count": 20
            },
            "return_expr": {
                "Object": [
                    {
                        "name": "id",
                        "value": { "Identifier": ["d", "_id"] }
                    },
                    {
                        "name": "title",
                        "value": { "Identifier": ["d", "title"] }
                    }
                ]
            },
            "bind_vars": ["category"]
        })
    );
}

#[test]
fn traversal_query_ast_shape() {
    let ast = parsed_json(
        r#"
        FOR v, e, p IN 1..3 OUTBOUND @start relationships
        FILTER e.confidence >= @min_confidence
        RETURN { vertex: v, edge: e, path: p }
        "#,
    );

    assert_eq!(
        ast,
        json!({
            "explain": false,
            "analyze": false,
            "body": [
                {
                    "For": {
                        "Traversal": {
                            "vertex_var": "v",
                            "edge_var": "e",
                            "path_var": "p",
                            "min_depth": 1,
                            "max_depth": 3,
                            "direction": "Outbound",
                            "start": { "BindVar": "start" },
                            "edge_collection": "relationships"
                        }
                    }
                },
                {
                    "Filter": {
                        "Binary": {
                            "left": { "Identifier": ["e", "confidence"] },
                            "op": "Ge",
                            "right": { "BindVar": "min_confidence" }
                        }
                    }
                }
            ],
            "distinct": false,
            "collect": null,
            "mutation": null,
            "sort": null,
            "limit": null,
            "return_expr": {
                "Object": [
                    {
                        "name": "vertex",
                        "value": { "Identifier": ["v"] }
                    },
                    {
                        "name": "edge",
                        "value": { "Identifier": ["e"] }
                    },
                    {
                        "name": "path",
                        "value": { "Identifier": ["p"] }
                    }
                ]
            },
            "bind_vars": ["min_confidence", "start"]
        })
    );
}

#[test]
fn vector_query_ast_shape() {
    let ast = parsed_json(
        r#"
        FOR d IN vector_search(documents, @embedding)
        LIMIT 10
        RETURN { document: d, score: d._score }
        "#,
    );

    assert_eq!(
        ast,
        json!({
            "explain": false,
            "analyze": false,
            "body": [
                {
                    "For": {
                        "VectorSearch": {
                            "var": "d",
                            "collection": "documents",
                            "vector": { "BindVar": "embedding" }
                        }
                    }
                }
            ],
            "distinct": false,
            "collect": null,
            "mutation": null,
            "sort": null,
            "limit": {
                "offset": null,
                "count": 10
            },
            "return_expr": {
                "Object": [
                    {
                        "name": "document",
                        "value": { "Identifier": ["d"] }
                    },
                    {
                        "name": "score",
                        "value": { "Identifier": ["d", "_score"] }
                    }
                ]
            },
            "bind_vars": ["embedding"]
        })
    );
}
