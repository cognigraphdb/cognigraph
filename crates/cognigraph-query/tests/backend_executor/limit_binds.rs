//! CG-85: a bound LIMIT is pushed down exactly like a literal one, with the
//! value resolved at run time, and the same conditions still disable it.

use super::*;

#[tokio::test]
async fn bound_limit_pushes_the_resolved_fetch_limit_down() {
    let backend = RecordingBackend::default();
    parse_and_execute_backend(
        "FOR d IN documents LIMIT @o, @n RETURN d",
        &backend,
        &binds([("o", json!(3)), ("n", json!(2))]),
    )
    .await
    .unwrap();
    assert_eq!(*backend.last_list_limit.lock().unwrap(), Some(Some(5)));

    parse_and_execute_backend(
        "FOR d IN documents LIMIT @n RETURN d",
        &backend,
        &binds([("n", json!(7))]),
    )
    .await
    .unwrap();
    assert_eq!(*backend.last_list_limit.lock().unwrap(), Some(Some(7)));
}

#[tokio::test]
async fn bound_limit_pushdown_is_disabled_by_filters_and_sort_like_a_literal() {
    let backend = RecordingBackend::default();
    parse_and_execute_backend(
        "FOR d IN documents FILTER d.x == 1 LIMIT @n RETURN d",
        &backend,
        &binds([("n", json!(2))]),
    )
    .await
    .unwrap();
    assert_eq!(*backend.last_list_limit.lock().unwrap(), Some(None));

    parse_and_execute_backend(
        "FOR d IN documents SORT d.x ASC LIMIT @n RETURN d",
        &backend,
        &binds([("n", json!(2))]),
    )
    .await
    .unwrap();
    assert_eq!(*backend.last_list_limit.lock().unwrap(), Some(None));
}

#[tokio::test]
async fn an_invalid_bound_limit_fails_before_the_backend_is_touched() {
    let backend = RecordingBackend::default();
    let err = parse_and_execute_backend(
        "FOR d IN documents LIMIT @n RETURN d",
        &backend,
        &binds([("n", json!(0))]),
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "LIMIT count must be greater than zero");
    assert_eq!(
        *backend.last_list_limit.lock().unwrap(),
        None,
        "no scan may start with an invalid limit"
    );
}
