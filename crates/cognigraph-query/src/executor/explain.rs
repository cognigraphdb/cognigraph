//! EXPLAIN: a static, engine-independent description of the logical plan
//! and its pushdown decisions. Never touches data, never resolves bind
//! variables — both executors return the identical document, so the
//! dual-engine corpus can pin EXPLAIN output like any other query.

use serde_json::{Value, json};

use crate::ast::MutationClause;
use crate::planner::{LogicalPlan, PlanOp, PlanSource};

use super::materialize::{
    PushValue, ScanPushdown, analyze_scan, is_correlated_start, is_synthetic_var,
    traversal_min_confidence, vector_threshold,
};
use super::run::AnalyzeState;

pub(super) fn explain_plan(plan: &LogicalPlan) -> Vec<Value> {
    let mut sources = Vec::new();
    let mut pipeline = Vec::new();
    describe_plan(plan, 0, &mut sources, &mut pipeline);

    if let Some(mutation) = &plan.mutation {
        pipeline.push(mutation_desc(mutation));
    }
    if plan.projection.is_some() {
        pipeline.push(if plan.distinct {
            "RETURN DISTINCT".into()
        } else {
            "RETURN".into()
        });
    }

    vec![json!({
        "sources": sources,
        "pipeline": pipeline,
        "bind_vars": plan.bind_vars,
    })]
}

/// Walk one plan level: body ops into `pipeline` strings, every For source
/// into `sources` (pre-order over subqueries, matching execution's site
/// numbering; `depth` marks subquery nesting).
/// Marks an op the planner moved past SORT/LIMIT (move-calculations-down).
fn deferred_note(plan: &LogicalPlan, index: usize) -> &'static str {
    match plan.deferred_start {
        Some(start) if index >= start => " [deferred past LIMIT]",
        _ => "",
    }
}

fn describe_plan(
    plan: &LogicalPlan,
    depth: usize,
    sources: &mut Vec<Value>,
    pipeline: &mut Vec<String>,
) {
    for (index, op) in plan.body.iter().enumerate() {
        match op {
            PlanOp::For { var, source } => {
                sources.push(describe_source(plan, index, var, source, depth));
                pipeline.push(format!("FOR {var}"));
            }
            PlanOp::Let(let_clause) => pipeline.push(format!(
                "LET {}{}",
                let_clause.name,
                deferred_note(plan, index)
            )),
            PlanOp::LetSubquery {
                name,
                plan: subplan,
                correlated,
            } => {
                pipeline.push(format!(
                    "LET {name} = subquery ({}){}",
                    if *correlated {
                        "correlated, per row"
                    } else {
                        "uncorrelated, once"
                    },
                    deferred_note(plan, index)
                ));
                describe_plan(subplan, depth + 1, sources, pipeline);
            }
            PlanOp::Filter(_) => pipeline.push("FILTER".into()),
        }
    }
    if let Some(collect) = &plan.collect {
        let mut desc = format!("COLLECT ({} keys", collect.bindings.len());
        if !collect.aggregates.is_empty() {
            desc.push_str(&format!(", {} aggregates", collect.aggregates.len()));
        }
        if collect.into.is_some() {
            desc.push_str(", INTO");
        }
        if collect.count_into.is_some() {
            desc.push_str(", WITH COUNT");
        }
        desc.push(')');
        pipeline.push(desc);
    }
    if let Some(sort) = &plan.sort {
        let mut desc = format!("SORT ({} keys", sort.keys.len());
        if let Some(locale) = &sort.collation {
            desc.push_str(&format!(", COLLATE {locale}"));
        }
        desc.push(')');
        pipeline.push(desc);
    }
    if let Some(limit) = &plan.limit {
        match limit.offset {
            Some(offset) => pipeline.push(format!("LIMIT {offset}, {}", limit.count)),
            None => pipeline.push(format!("LIMIT {}", limit.count)),
        }
    }
    // Subquery RETURNs are inner; the outermost RETURN is appended by the
    // caller after the mutation entry.
    if depth > 0 && plan.projection.is_some() {
        pipeline.push(if plan.distinct {
            "RETURN DISTINCT (subquery)".into()
        } else {
            "RETURN (subquery)".into()
        });
    }
}

fn describe_source(
    plan: &LogicalPlan,
    for_index: usize,
    var: &str,
    source: &PlanSource,
    depth: usize,
) -> Value {
    match source {
        PlanSource::CollectionScan { collection } => json!({
            "kind": "collection_scan",
            "collection": collection,
            "var": var,
            "depth": depth,
            "pushdown": pushdown_json(&analyze_scan(plan, for_index, var)),
        }),
        PlanSource::VarRef(name) => json!({
            "kind": "var_ref",
            "var": var,
            "of": name,
            "depth": depth,
        }),
        PlanSource::Expression(_) => json!({
            "kind": "expression",
            "var": var,
            "depth": depth,
        }),
        PlanSource::VectorSearch { collection, .. } => json!({
            "kind": "vector_search",
            "collection": collection,
            "var": var,
            "depth": depth,
            "limit_pushdown": plan.limit.as_ref().map(|l| l.offset.unwrap_or(0) + l.count),
            "threshold_pushdown": vector_threshold(plan, var).map(|v| push_value_string(&v)),
        }),
        PlanSource::Traversal {
            edge_var,
            path_var,
            min_depth,
            max_depth,
            direction,
            edge_collection,
            start,
            ..
        } => {
            // Only the variables the query actually named: the parser invents
            // the others, and showing invented names in a plan is noise.
            let vars: Vec<&str> = [var, edge_var.as_str(), path_var.as_str()]
                .into_iter()
                .filter(|name| !is_synthetic_var(name))
                .collect();
            json!({
            "kind": "traversal",
            "edge_collection": edge_collection,
            "vars": vars,
            // A correlated start resolves once per distinct start value during
            // the run instead of once before it — the single biggest thing to
            // know about what this traversal will cost.
            "correlated": is_correlated_start(start),
            "depth_range": format!("{min_depth}..{max_depth}"),
            "direction": format!("{direction:?}").to_uppercase(),
            "depth": depth,
            // Only exact depth 1..1 may prune by confidence (deeper
            // pruning removes paths THROUGH weak edges — post-FILTER
            // semantics differ).
            "min_confidence_pushdown": (*min_depth == 1 && *max_depth == 1)
                .then(|| traversal_min_confidence(plan, edge_var).map(|v| push_value_string(&v)))
                .flatten(),
            })
        }
    }
}

fn pushdown_json(pushdown: &ScanPushdown) -> Value {
    json!({
        "projection": pushdown.fields,
        "predicates": pushdown
            .predicates
            .iter()
            .map(|p| {
                let op = match p.op {
                    cognigraph_core::PredicateOp::Eq => "==",
                    cognigraph_core::PredicateOp::Ne => "!=",
                    cognigraph_core::PredicateOp::Lt => "<",
                    cognigraph_core::PredicateOp::Le => "<=",
                    cognigraph_core::PredicateOp::Gt => ">",
                    cognigraph_core::PredicateOp::Ge => ">=",
                    cognigraph_core::PredicateOp::In => "IN",
                };
                let value = match &p.value {
                    PushValue::Literal(value) => value.to_string(),
                    PushValue::Bind(name) => format!("@{name}"),
                };
                format!(".{} {op} {value}", p.path.join("."))
            })
            .collect::<Vec<_>>(),
        "all_filters_pushed": pushdown.all_filters_pushed,
        "fetch_limit": pushdown.fetch_limit,
    })
}

/// Stage labels in exactly the order `run_body_staged`/`apply_tail` record
/// statistics — body ops (subquery plans expanded inline after their LET
/// entry), then COLLECT/SORT/LIMIT/RETURN per plan level.
fn stage_labels(plan: &LogicalPlan, depth: usize, out: &mut Vec<(usize, String)>) {
    for op in &plan.body {
        match op {
            PlanOp::For { var, .. } => out.push((depth, format!("FOR {var}"))),
            PlanOp::Let(let_clause) => out.push((depth, format!("LET {}", let_clause.name))),
            PlanOp::LetSubquery {
                name,
                plan: subplan,
                correlated,
            } => {
                out.push((
                    depth,
                    format!(
                        "LET {name} = subquery ({})",
                        if *correlated {
                            "correlated, per row"
                        } else {
                            "uncorrelated, once"
                        }
                    ),
                ));
                stage_labels(subplan, depth + 1, out);
            }
            PlanOp::Filter(_) => out.push((depth, "FILTER".into())),
        }
    }
    if plan.collect.is_some() {
        out.push((depth, "COLLECT".into()));
    }
    if plan.sort.is_some() {
        out.push((depth, "SORT".into()));
    }
    if let Some(limit) = &plan.limit {
        match limit.offset {
            Some(offset) => out.push((depth, format!("LIMIT {offset}, {}", limit.count))),
            None => out.push((depth, format!("LIMIT {}", limit.count))),
        }
    }
    if plan.projection.is_some() {
        out.push((
            depth,
            if plan.distinct {
                "RETURN DISTINCT".into()
            } else {
                "RETURN".into()
            },
        ));
    }
}

/// Sources in site pre-order, mirroring materialization's numbering.
fn source_descriptions(plan: &LogicalPlan, depth: usize, out: &mut Vec<Value>) {
    for (index, op) in plan.body.iter().enumerate() {
        match op {
            PlanOp::For { var, source } => {
                out.push(describe_source(plan, index, var, source, depth));
            }
            PlanOp::LetSubquery { plan: subplan, .. } => {
                source_descriptions(subplan, depth + 1, out);
            }
            _ => {}
        }
    }
}

/// The EXPLAIN ANALYZE report: static plan description merged with the
/// measured statistics. Logical stage counts exclude speculative read rounds;
/// attempted stage counts and source work retain them. Backend fetch work and
/// every `*_ms` field may differ from the in-memory engine.
pub(super) fn analyze_report(
    plan: &LogicalPlan,
    state: &AnalyzeState,
    total_ms: f64,
    source_rows: u64,
    result_rows: usize,
) -> Vec<Value> {
    let mut sources = Vec::new();
    source_descriptions(plan, 0, &mut sources);
    for (site, source) in sources.iter_mut().enumerate() {
        if let (Some(stat), Some(obj)) = (state.sites.get(&site), source.as_object_mut()) {
            obj.insert("rows".into(), json!(stat.rows));
            obj.insert("fetch_ms".into(), json!(round_ms(stat.fetch_ms)));
            obj.insert("fetches".into(), json!(stat.fetches));
        }
    }
    let mut labels = Vec::new();
    stage_labels(plan, 0, &mut labels);
    let stages: Vec<Value> = labels
        .iter()
        .zip(&state.stages)
        .map(|((depth, label), stat)| {
            json!({
                "stage": label,
                "depth": depth,
                "rows": stat.rows,
                "runs": stat.runs,
                "attempted_rows": stat.attempted_rows,
                "attempted_runs": stat.attempted_runs,
            })
        })
        .collect();
    vec![json!({
        "analyze": true,
        "sources": sources,
        "stages": stages,
        "stats": {
            "total_ms": round_ms(total_ms),
            "source_rows": source_rows,
            "result_rows": result_rows,
            "document_fetches": state.document_fetches,
            "document_fetch_ms": round_ms(state.document_fetch_ms),
        },
        "bind_vars": plan.bind_vars,
    })]
}

fn push_value_string(value: &PushValue) -> String {
    match value {
        PushValue::Literal(value) => value.to_string(),
        PushValue::Bind(name) => format!("@{name}"),
    }
}

fn round_ms(ms: f64) -> f64 {
    (ms * 1000.0).round() / 1000.0
}

fn mutation_desc(mutation: &MutationClause) -> String {
    match mutation {
        MutationClause::Insert { collection, .. } => format!("INSERT INTO {collection}"),
        MutationClause::Update { collection, .. } => format!("UPDATE IN {collection}"),
        MutationClause::Replace { collection, .. } => format!("REPLACE IN {collection}"),
        MutationClause::Remove { collection, .. } => format!("REMOVE IN {collection}"),
        MutationClause::Upsert { collection, .. } => format!("UPSERT IN {collection}"),
    }
}
