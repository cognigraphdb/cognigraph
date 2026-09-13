use cognigraph_query::parse_query;

#[test]
fn rejects_invalid_traversal_range() {
    let err = parse_query("FOR v, e, p IN 3..1 ANY @start relationships RETURN v").unwrap_err();

    assert!(
        err.message
            .contains("minimum traversal depth exceeds maximum")
    );
}

#[test]
fn rejects_trailing_comma_in_object() {
    let err = parse_query("FOR d IN documents RETURN { id: d._id, }").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_trailing_comma_in_array() {
    let err = parse_query("FOR d IN documents RETURN [d._id,]").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_empty_filter_expression() {
    let err = parse_query("FOR d IN documents FILTER RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_invalid_bind_variable() {
    let err = parse_query("FOR d IN documents FILTER d.category == @ RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_bind_variable_starting_with_digit() {
    let err = parse_query("FOR d IN documents FILTER d.category == @1bad RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_repeated_sort_clause() {
    let err = parse_query("FOR d IN documents SORT d.a SORT d.b RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_repeated_limit_clause() {
    let err = parse_query("FOR d IN documents LIMIT 1 LIMIT 2 RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_filter_after_sort() {
    let err = parse_query("FOR d IN documents SORT d.a FILTER d.b == 1 RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_sort_after_limit() {
    let err = parse_query("FOR d IN documents LIMIT 10 SORT d.a RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_malformed_vector_source_missing_argument() {
    // v2: the expr-FOR grammar parses this as a function-call source; the
    // reserved function name is rejected at validation instead.
    let query = parse_query("FOR d IN VECTOR_SEARCH(documents) RETURN d").unwrap();
    let err = cognigraph_query::validate_query(&query).unwrap_err();
    assert!(err.to_string().contains("reserved"));
}

#[test]
fn rejects_float_without_leading_digit() {
    let err = parse_query("FOR d IN documents FILTER d.score > .5 RETURN d").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_unclosed_string() {
    let err = parse_query("FOR d IN documents RETURN \"unterminated").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn rejects_missing_return() {
    let err = parse_query("FOR d IN documents FILTER d.category == @category").unwrap_err();

    assert!(err.message.contains("RETURN") || err.message.contains("return"));
}

#[test]
fn rejects_unknown_syntax() {
    let err = parse_query("SELECT * FROM documents").unwrap_err();

    assert!(err.message.contains("expected"));
}

#[test]
fn parse_error_reports_line_and_column() {
    let err = parse_query("FOR d IN documents\nRETURN {").unwrap_err();
    let (line, _column) = err.line_col.expect("pest errors carry a position");
    assert_eq!(line, 2);
    assert!(err.to_string().starts_with("parse error at line 2, column"));
}

#[test]
fn rejects_keyword_glued_to_identifier() {
    assert!(parse_query("FORd IN documents RETURN d").is_err());
    assert!(parse_query("FOR d IN documents FILTER d.x INitems RETURN d").is_err());
}

#[test]
fn rejects_chained_comparisons() {
    assert!(parse_query("FOR d IN documents FILTER d.a == d.b == d.c RETURN d").is_err());
}
