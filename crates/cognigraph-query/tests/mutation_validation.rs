use cognigraph_query::{parse_and_plan, parse_query, validate_query};
use serde_json::Value;

#[test]
fn mutation_backend_reads_are_rejected_by_validation_and_planning() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/mutation_backend_reads.json")).unwrap();
    for case in cases {
        let query = case["query"].as_str().unwrap();
        let expected = case["error"].as_str().unwrap();
        let ast = parse_query(query).unwrap_or_else(|e| panic!("{query}: {e}"));
        assert_eq!(
            validate_query(&ast).unwrap_err().to_string(),
            expected,
            "{query}"
        );
        assert_eq!(
            parse_and_plan(query).unwrap_err().to_string(),
            expected,
            "{query}"
        );
    }
}

#[test]
fn ordinary_mutations_and_backend_reads_remain_valid() {
    for query in [
        r#"INSERT {_key:"c",v:@value} INTO notes RETURN NEW"#,
        r#"UPDATE "a" WITH {v:@value} IN notes RETURN {old:OLD,new:NEW}"#,
        r#"REPLACE "a" WITH {v:2} IN notes RETURN OLD"#,
        r#"REMOVE "a" IN notes RETURN OLD"#,
        r#"UPSERT {_key:"a"} INSERT {_key:"c"} UPDATE {v:2} IN notes RETURN NEW"#,
        r#"INSERT {DOCUMENT:"DOCUMENT(@ref)",v:@DOCUMENT} INTO notes RETURN NEW"#,
        r#"FOR seed IN [1] LET refs=(FOR d IN notes RETURN d) INSERT {refs:refs} INTO notes"#,
        r#"FOR v IN 1..1 OUTBOUND @ref links REMOVE v._key IN notes"#,
        r#"FOR v IN 1..1 OUTBOUND CONCAT("notes/", "a") links REMOVE v._key IN notes"#,
        r#"RETURN DOCUMENT(@ref)"#,
        r#"LET start=@ref FOR v IN 1..1 OUTBOUND start links RETURN v"#,
    ] {
        parse_and_plan(query).unwrap_or_else(|e| panic!("{query}: {e}"));
    }
}

#[test]
fn explain_does_not_bypass_mutation_validation() {
    let query = r#"EXPLAIN UPDATE "a" WITH {v:DOCUMENT(@ref).v} IN notes"#;
    assert_eq!(
        parse_and_plan(query).unwrap_err().to_string(),
        "DOCUMENT() is not supported in mutation queries"
    );
}
