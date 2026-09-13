use cognigraph_query::{
    ValidationError, ValidationOptions, parse_query, validate_query, validate_query_with_options,
};

#[test]
fn validates_collection_query() {
    let query = parse_query(
        r#"
        FOR d IN documents
        FILTER d.category == @category
        SORT d.created_at DESC
        LIMIT 20
        RETURN { id: d._id, title: d.title }
        "#,
    )
    .unwrap();

    validate_query(&query).unwrap();
}

#[test]
fn validates_traversal_query() {
    let query = parse_query(
        r#"
        FOR v, e, p IN 1..3 OUTBOUND @start relationships
        FILTER e.confidence >= @min_confidence
        RETURN { vertex: v, edge: e, path: p }
        "#,
    )
    .unwrap();

    validate_query(&query).unwrap();
}

#[test]
fn validates_vector_query_with_bind_var() {
    let query = parse_query("FOR d IN VECTOR_SEARCH(documents, @embedding) RETURN d").unwrap();

    validate_query(&query).unwrap();
}

#[test]
fn validates_vector_query_with_numeric_literal_array() {
    let query =
        parse_query("FOR d IN VECTOR_SEARCH(documents, [0.1, 0.2, -0.3]) RETURN d").unwrap();

    validate_query(&query).unwrap();
}

#[test]
fn rejects_unknown_root_identifier_in_filter() {
    let query = parse_query("FOR d IN documents FILTER e.confidence > 0 RETURN d").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(err, ValidationError::UnknownIdentifier("e".into()));
}

#[test]
fn rejects_unknown_root_identifier_in_return() {
    let query = parse_query("FOR d IN documents RETURN other").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(err, ValidationError::UnknownIdentifier("other".into()));
}

#[test]
fn rejects_duplicate_traversal_variables() {
    let query = parse_query("FOR v, v, p IN 1..2 ANY @start relationships RETURN v").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(err, ValidationError::DuplicateVariable("v".into()));
}

#[test]
fn rejects_zero_limit() {
    let query = parse_query("FOR d IN documents LIMIT 0 RETURN d").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(err, ValidationError::ZeroLimit);
}

#[test]
fn rejects_excessive_limit() {
    let query = parse_query("FOR d IN documents LIMIT 100 RETURN d").unwrap();
    let err = validate_query_with_options(
        &query,
        ValidationOptions {
            max_limit: 10,
            max_traversal_depth: 10,
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        ValidationError::LimitTooLarge {
            count: 100,
            max: 10
        }
    );
}

#[test]
fn rejects_excessive_traversal_depth() {
    let query = parse_query("FOR v, e, p IN 1..5 ANY @start relationships RETURN v").unwrap();
    let err = validate_query_with_options(
        &query,
        ValidationOptions {
            max_limit: 100,
            max_traversal_depth: 3,
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        ValidationError::TraversalDepthTooLarge { depth: 5, max: 3 }
    );
}

#[test]
fn rejects_duplicate_object_fields() {
    let query = parse_query("FOR d IN documents RETURN { id: d._id, id: d.title }").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(err, ValidationError::DuplicateObjectField("id".into()));
}

#[test]
fn rejects_vector_search_with_document_field_vector() {
    let query = parse_query("FOR d IN VECTOR_SEARCH(documents, d.embedding) RETURN d").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(err, ValidationError::InvalidVectorSearchInput);
}

#[test]
fn rejects_vector_search_with_non_numeric_literal_array() {
    let query = parse_query(r#"FOR d IN VECTOR_SEARCH(documents, [0.1, "bad"]) RETURN d"#).unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(err, ValidationError::InvalidVectorSearchInput);
}

#[test]
fn rejects_reserved_collection_name() {
    let query = parse_query("FOR d IN RETURN RETURN d").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(
        err,
        ValidationError::ReservedCollectionName("RETURN".into())
    );
}

#[test]
fn rejects_reserved_vector_function_call() {
    let query = parse_query("FOR d IN documents RETURN VECTOR_SEARCH(d.embedding)").unwrap();
    let err = validate_query(&query).unwrap_err();

    assert_eq!(
        err,
        ValidationError::ReservedFunctionName("VECTOR_SEARCH".into())
    );
}

#[test]
fn rejects_reserved_variable_name() {
    let query = parse_query("FOR filter IN documents RETURN filter").unwrap();
    assert!(matches!(
        validate_query(&query),
        Err(ValidationError::ReservedVariableName(name)) if name == "filter"
    ));
}
