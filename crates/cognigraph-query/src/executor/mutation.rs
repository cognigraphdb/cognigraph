use serde_json::Value;

use crate::ast::MutationClause;
use crate::planner::LogicalPlan;
use cognigraph_core::GraphBackend;

use super::eval::{EvalCtx, eval_expr, values_equal};
use super::materialize::materialize_backend;
use super::run::{RunCx, run_body};
use super::{BindVars, Deadline, Env, ExecutionBudget, ExecutionError};

/// Execute a mutation plan: the positional body produces the row set (or
/// one synthetic row for standalone mutations), then one backend operation
/// runs per row. Atomicity is per document.
pub(super) async fn execute_mutation(
    plan: &LogicalPlan,
    backend: &dyn GraphBackend,
    bind_vars: &BindVars,
    budget: &ExecutionBudget,
    deadline: Deadline,
) -> Result<Vec<Value>, ExecutionError> {
    let mutation = plan
        .mutation
        .as_ref()
        .ok_or_else(|| ExecutionError::Backend("missing mutation clause".into()))?;
    let cx = RunCx::new(bind_vars, deadline, budget);
    let mut materialized = materialize_backend(plan, backend, &cx).await?;
    let mut site = 0usize;
    let kept = run_body(plan, Env::new(), &mut materialized, &mut site, &cx, true)?;

    let backend_err = super::backend_execution_error;
    let mut out = Vec::new();
    for mut env in kept {
        deadline.check()?;
        let (old, new) = match mutation {
            MutationClause::Insert { doc, collection } => {
                let doc_value = eval_expr(doc, &env, &EvalCtx::new(bind_vars))?;
                let id = backend
                    .create_document(collection, doc_value)
                    .await
                    .map_err(backend_err)?;
                let new = backend
                    .get_document(collection, &id.key)
                    .await
                    .map_err(backend_err)?
                    .unwrap_or(Value::Null);
                (Value::Null, new)
            }
            MutationClause::Update {
                key,
                with,
                collection,
            } => {
                let key = document_key_of(&eval_expr(key, &env, &EvalCtx::new(bind_vars))?)?;
                let old = backend
                    .get_document(collection, &key)
                    .await
                    .map_err(backend_err)?
                    .unwrap_or(Value::Null);
                let new = backend
                    .update_document(
                        collection,
                        &key,
                        eval_expr(with, &env, &EvalCtx::new(bind_vars))?,
                    )
                    .await
                    .map_err(backend_err)?;
                (old, new)
            }
            MutationClause::Replace {
                key,
                with,
                collection,
            } => {
                let key = document_key_of(&eval_expr(key, &env, &EvalCtx::new(bind_vars))?)?;
                let old = backend
                    .get_document(collection, &key)
                    .await
                    .map_err(backend_err)?
                    .unwrap_or(Value::Null);
                let new = backend
                    .replace_document(
                        collection,
                        &key,
                        eval_expr(with, &env, &EvalCtx::new(bind_vars))?,
                    )
                    .await
                    .map_err(backend_err)?;
                (old, new)
            }
            MutationClause::Remove { key, collection } => {
                let key = document_key_of(&eval_expr(key, &env, &EvalCtx::new(bind_vars))?)?;
                let old = backend
                    .get_document(collection, &key)
                    .await
                    .map_err(backend_err)?
                    .unwrap_or(Value::Null);
                backend
                    .delete_document(collection, &key)
                    .await
                    .map_err(backend_err)?;
                (old, Value::Null)
            }
            MutationClause::Upsert {
                search,
                insert,
                update,
                collection,
            } => {
                let search_value = eval_expr(search, &env, &EvalCtx::new(bind_vars))?;
                let Value::Object(search_fields) = &search_value else {
                    return Err(ExecutionError::Backend(
                        "UPSERT search expression must be an object".into(),
                    ));
                };
                let matches_search = |doc: &Value| {
                    search_fields.iter().all(|(field, expected)| {
                        doc.get(field)
                            .is_some_and(|actual| values_equal(actual, expected))
                    })
                };
                // _key narrows the candidates; every search predicate
                // still applies, using the same equality as the scan.
                let existing = match search_fields.get("_key").and_then(Value::as_str) {
                    Some(key) => backend
                        .get_document(collection, key)
                        .await
                        .map_err(backend_err)?
                        .filter(matches_search),
                    None => backend
                        .list_documents(collection, None, None)
                        .await
                        .map_err(backend_err)?
                        .into_iter()
                        .find(matches_search),
                };
                match existing {
                    Some(old) => {
                        let key = old
                            .get("_key")
                            .and_then(Value::as_str)
                            .ok_or(ExecutionError::InvalidDocumentKey)?
                            .to_string();
                        let new = backend
                            .update_document(
                                collection,
                                &key,
                                eval_expr(update, &env, &EvalCtx::new(bind_vars))?,
                            )
                            .await
                            .map_err(backend_err)?;
                        (old, new)
                    }
                    None => {
                        let id = backend
                            .create_document(
                                collection,
                                eval_expr(insert, &env, &EvalCtx::new(bind_vars))?,
                            )
                            .await
                            .map_err(backend_err)?;
                        let new = backend
                            .get_document(collection, &id.key)
                            .await
                            .map_err(backend_err)?
                            .unwrap_or(Value::Null);
                        (Value::Null, new)
                    }
                }
            }
        };
        if let Some(projection) = &plan.projection {
            env.insert("OLD".to_string(), old);
            env.insert("NEW".to_string(), new);
            out.push(eval_expr(projection, &env, &EvalCtx::new(bind_vars))?);
        }
    }
    Ok(out)
}

/// Accepts a plain key or a full `collection/key` id.
fn document_key_of(value: &Value) -> Result<String, ExecutionError> {
    let raw = value.as_str().ok_or(ExecutionError::InvalidDocumentKey)?;
    Ok(raw.rsplit_once('/').map_or(raw, |(_, key)| key).to_string())
}
