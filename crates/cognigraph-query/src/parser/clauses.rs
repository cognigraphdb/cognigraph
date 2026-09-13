use pest::iterators::Pair;

use crate::ast::*;

use super::expr::{
    build_expr, first_non_keyword, is_keyword_rule, next_non_keyword, next_text, parse_string,
};
use super::{ParseError, Rule};

pub(super) fn build_for_clause(pair: Pair<'_, Rule>) -> Result<ForClause, ParseError> {
    let source = first_non_keyword(pair, "FOR source")?;
    match source.as_rule() {
        Rule::expr_for => {
            let mut inner = source.into_inner();
            let var = next_text(&mut inner, "FOR variable")?;
            let expr = build_expr(next_non_keyword(&mut inner, "FOR source expression")?)?;
            // A bare single-segment identifier is a name: an in-scope
            // variable or a collection, resolved at plan time.
            match expr {
                Expr::Identifier(parts) if parts.len() == 1 => Ok(ForClause::Collection {
                    var,
                    collection: parts.into_iter().next().unwrap(),
                }),
                expr => Ok(ForClause::Expression { var, expr }),
            }
        }
        Rule::vector_for => {
            let mut inner = source.into_inner();
            let var = next_text(&mut inner, "vector variable")?;
            let vector_source = next_non_keyword(&mut inner, "VECTOR_SEARCH source")?;
            let mut vector_inner = vector_source.into_inner();
            let collection = next_text(&mut vector_inner, "vector collection")?;
            let vector = build_expr(next_non_keyword(&mut vector_inner, "vector expression")?)?;
            Ok(ForClause::VectorSearch {
                var,
                collection,
                vector,
            })
        }
        Rule::traversal_for => {
            let mut inner = source.into_inner();
            let vertex_var = next_text(&mut inner, "vertex variable")?;
            // The edge and path variables are optional. When absent they still
            // need names to bind under, so they get ones a user cannot collide
            // with: `$` is not a legal identifier character.
            let mut optional_vars = Vec::new();
            let range = loop {
                let pair = next_non_keyword(&mut inner, "traversal range")?;
                if pair.as_rule() == Rule::ident {
                    optional_vars.push(pair.as_str().to_string());
                    continue;
                }
                break pair;
            };
            // Derived from the vertex variable, which is unique within its
            // scope — two traversals in one query would otherwise both invent
            // `$edge` and collide on the duplicate-variable check.
            let mut optional_vars = optional_vars.into_iter();
            let edge_var = optional_vars
                .next()
                .unwrap_or_else(|| format!("$edge_{vertex_var}"));
            let path_var = optional_vars
                .next()
                .unwrap_or_else(|| format!("$path_{vertex_var}"));
            let (min_depth, max_depth) = build_depth_range(range)?;
            let direction = build_direction(next_non_keyword(&mut inner, "traversal direction")?)?;
            let start = build_expr(next_non_keyword(&mut inner, "traversal start")?)?;
            let edge_collection = next_text(&mut inner, "edge collection")?;
            Ok(ForClause::Traversal {
                vertex_var,
                edge_var,
                path_var,
                min_depth,
                max_depth,
                direction,
                start,
                edge_collection,
            })
        }
        _ => Err(ParseError::new("unknown FOR clause form")),
    }
}

fn build_depth_range(pair: Pair<'_, Rule>) -> Result<(u32, u32), ParseError> {
    let mut inner = pair.into_inner();
    let min = next_text(&mut inner, "minimum traversal depth")?
        .parse::<u32>()
        .map_err(|e| ParseError::new(format!("invalid minimum traversal depth: {e}")))?;
    let max = next_text(&mut inner, "maximum traversal depth")?
        .parse::<u32>()
        .map_err(|e| ParseError::new(format!("invalid maximum traversal depth: {e}")))?;
    if min > max {
        return Err(ParseError::new("minimum traversal depth exceeds maximum"));
    }
    Ok((min, max))
}

fn build_direction(pair: Pair<'_, Rule>) -> Result<Direction, ParseError> {
    match pair.as_str().to_ascii_uppercase().as_str() {
        "OUTBOUND" => Ok(Direction::Outbound),
        "INBOUND" => Ok(Direction::Inbound),
        "ANY" => Ok(Direction::Any),
        _ => Err(ParseError::new("invalid traversal direction")),
    }
}

pub(super) fn build_sort_clause(pair: Pair<'_, Rule>) -> Result<SortClause, ParseError> {
    let mut keys = Vec::new();
    let mut collation = None;
    for key_pair in pair.into_inner() {
        if key_pair.as_rule() == Rule::string {
            collation = Some(parse_string(key_pair.as_str())?);
            continue;
        }
        if key_pair.as_rule() != Rule::sort_key {
            continue;
        }
        let mut expr = None;
        let mut direction = SortDirection::Asc;
        for part in key_pair.into_inner() {
            match part.as_rule() {
                Rule::sort_direction => {
                    if part.as_str().eq_ignore_ascii_case("DESC") {
                        direction = SortDirection::Desc;
                    }
                }
                rule if is_keyword_rule(rule) => {}
                _ => expr = Some(build_expr(part)?),
            }
        }
        keys.push(SortKey {
            expr: expr.ok_or_else(|| ParseError::new("missing SORT expression"))?,
            direction,
        });
    }
    if keys.is_empty() {
        return Err(ParseError::new("missing SORT expression"));
    }
    Ok(SortClause { keys, collation })
}

pub(super) fn build_mutation_clause(pair: Pair<'_, Rule>) -> Result<MutationClause, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or_else(|| ParseError::new("missing mutation clause"))?;
    let rule = inner.as_rule();
    let mut parts = inner.into_inner();
    Ok(match rule {
        Rule::insert_clause => MutationClause::Insert {
            doc: build_expr(next_non_keyword(&mut parts, "INSERT document")?)?,
            collection: next_text(&mut parts, "INSERT collection")?,
        },
        Rule::update_clause => MutationClause::Update {
            key: build_expr(next_non_keyword(&mut parts, "UPDATE key")?)?,
            with: build_expr(next_non_keyword(&mut parts, "UPDATE document")?)?,
            collection: next_text(&mut parts, "UPDATE collection")?,
        },
        Rule::replace_clause => MutationClause::Replace {
            key: build_expr(next_non_keyword(&mut parts, "REPLACE key")?)?,
            with: build_expr(next_non_keyword(&mut parts, "REPLACE document")?)?,
            collection: next_text(&mut parts, "REPLACE collection")?,
        },
        Rule::upsert_clause => MutationClause::Upsert {
            search: build_expr(next_non_keyword(&mut parts, "UPSERT search")?)?,
            insert: build_expr(next_non_keyword(&mut parts, "UPSERT insert document")?)?,
            update: build_expr(next_non_keyword(&mut parts, "UPSERT update document")?)?,
            collection: next_text(&mut parts, "UPSERT collection")?,
        },
        Rule::remove_clause => MutationClause::Remove {
            key: build_expr(next_non_keyword(&mut parts, "REMOVE key")?)?,
            collection: next_text(&mut parts, "REMOVE collection")?,
        },
        _ => return Err(ParseError::new("unknown mutation clause")),
    })
}

pub(super) fn build_collect_clause(pair: Pair<'_, Rule>) -> Result<CollectClause, ParseError> {
    let mut bindings = Vec::new();
    let mut aggregates = Vec::new();
    let mut into = None;
    let mut count_into = None;
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::collect_binding => {
                let mut inner = part.into_inner();
                let name = next_text(&mut inner, "COLLECT variable")?;
                let expr = build_expr(next_non_keyword(&mut inner, "COLLECT expression")?)?;
                bindings.push(CollectBinding { name, expr });
            }
            Rule::collect_aggregate => {
                for agg in part.into_inner() {
                    if agg.as_rule() != Rule::aggregate_binding {
                        continue;
                    }
                    let mut inner = agg.into_inner();
                    let name = next_text(&mut inner, "AGGREGATE variable")?;
                    let function = next_text(&mut inner, "AGGREGATE function")?;
                    let expr = build_expr(next_non_keyword(&mut inner, "AGGREGATE expression")?)?;
                    aggregates.push(AggregateBinding {
                        name,
                        function,
                        expr,
                    });
                }
            }
            Rule::collect_into => {
                let mut inner = part.into_inner();
                let name = next_text(&mut inner, "INTO variable")?;
                let projection = inner
                    .find(|p| !is_keyword_rule(p.as_rule()))
                    .map(build_expr)
                    .transpose()?;
                into = Some(CollectInto { name, projection });
            }
            Rule::collect_count => {
                let mut inner = part.into_inner();
                count_into = Some(next_text(&mut inner, "COUNT variable")?);
            }
            _ => {}
        }
    }
    Ok(CollectClause {
        bindings,
        aggregates,
        into,
        count_into,
    })
}

pub(super) fn build_let_clause(pair: Pair<'_, Rule>) -> Result<LetClause, ParseError> {
    let mut inner = pair.into_inner();
    let name = next_text(&mut inner, "LET variable")?;
    let value = next_non_keyword(&mut inner, "LET expression")?;
    let expr = match value.as_rule() {
        Rule::subquery => {
            let body = value
                .into_inner()
                .find(|p| p.as_rule() == Rule::read_query)
                .ok_or_else(|| ParseError::new("empty subquery"))?;
            Expr::Subquery(Box::new(super::build_query_body(body)?))
        }
        _ => build_expr(value)?,
    };
    Ok(LetClause { name, expr })
}

pub(super) fn build_limit_clause(pair: Pair<'_, Rule>) -> Result<LimitClause, ParseError> {
    let nums = pair
        .into_inner()
        .filter(|pair| !is_keyword_rule(pair.as_rule()))
        .map(|n| {
            n.as_str()
                .parse::<u64>()
                .map_err(|e| ParseError::new(format!("invalid LIMIT value: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    match nums.as_slice() {
        [count] => Ok(LimitClause {
            offset: None,
            count: *count,
        }),
        [offset, count] => Ok(LimitClause {
            offset: Some(*offset),
            count: *count,
        }),
        _ => Err(ParseError::new("LIMIT expects one or two integer values")),
    }
}
