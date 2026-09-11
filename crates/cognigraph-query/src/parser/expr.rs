use pest::iterators::{Pair, Pairs};

use crate::ast::*;

use super::{ParseError, Rule};

pub(super) fn build_expr(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    match pair.as_rule() {
        Rule::ternary_expr => build_ternary(pair),
        Rule::expr
        | Rule::or_expr
        | Rule::and_expr
        | Rule::comparison_expr
        | Rule::add_expr
        | Rule::mul_expr => build_fold_expr(pair),
        Rule::unary_expr => build_expr(single_inner(pair, "expression")?),
        Rule::postfix_expr => build_postfix(pair),
        Rule::primary => build_expr(single_inner(pair, "primary expression")?),
        Rule::path => Ok(Expr::Identifier(
            pair.into_inner().map(|p| p.as_str().to_string()).collect(),
        )),
        Rule::bind_var => Ok(Expr::BindVar(pair.as_str()[1..].to_string())),
        Rule::string => parse_string(pair.as_str()).map(Expr::String),
        Rule::number => pair
            .as_str()
            .parse::<f64>()
            .map(Expr::Number)
            .map_err(|e| ParseError::new(format!("invalid number: {e}"))),
        Rule::boolean => Ok(Expr::Bool(pair.as_str().eq_ignore_ascii_case("true"))),
        Rule::null => Ok(Expr::Null),
        Rule::array => pair
            .into_inner()
            .map(build_expr)
            .collect::<Result<Vec<_>, _>>()
            .map(Expr::Array),
        Rule::object => pair
            .into_inner()
            .map(build_object_field)
            .collect::<Result<Vec<_>, _>>()
            .map(Expr::Object),
        Rule::function_call => build_function_call(pair),
        Rule::subquery => crate::parser::build_subquery(pair),
        Rule::unary_operation => build_unary_expr(pair),
        _ => Err(ParseError::new(format!(
            "unsupported expression rule: {:?}",
            pair.as_rule()
        ))),
    }
}

/// `cond ? then : else`, or just the condition when no `?` follows.
/// `primary` followed by zero or more `.field` accesses.
fn build_postfix(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(
        inner
            .next()
            .ok_or_else(|| ParseError::new("missing postfix base"))?,
    )?;
    for access in inner {
        let name = access
            .into_inner()
            .next()
            .ok_or_else(|| ParseError::new("missing field name"))?
            .as_str()
            .to_string();
        expr = Expr::Field {
            base: Box::new(expr),
            name,
        };
    }
    Ok(expr)
}

fn build_ternary(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let condition = build_expr(
        inner
            .next()
            .ok_or_else(|| ParseError::new("missing conditional operand"))?,
    )?;
    let Some(then_pair) = inner.next() else {
        return Ok(condition);
    };
    let then_branch = build_expr(then_pair)?;
    let else_branch = build_expr(
        inner
            .next()
            .ok_or_else(|| ParseError::new("conditional is missing its else branch"))?,
    )?;
    Ok(Expr::Conditional {
        condition: Box::new(condition),
        then_branch: Box::new(then_branch),
        else_branch: Box::new(else_branch),
    })
}

fn build_fold_expr(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let mut expr = build_expr(
        inner
            .next()
            .ok_or_else(|| ParseError::new("missing expression operand"))?,
    )?;
    while let Some(op_pair) = inner.next() {
        let op = build_binary_op(op_pair)?;
        let right = build_expr(
            inner
                .next()
                .ok_or_else(|| ParseError::new("missing binary right operand"))?,
        )?;
        expr = Expr::Binary {
            left: Box::new(expr),
            op,
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn build_binary_op(pair: Pair<'_, Rule>) -> Result<BinaryOp, ParseError> {
    match pair.as_str().to_ascii_uppercase().as_str() {
        "OR" => Ok(BinaryOp::Or),
        "AND" => Ok(BinaryOp::And),
        "==" => Ok(BinaryOp::Eq),
        "!=" => Ok(BinaryOp::Ne),
        "<" => Ok(BinaryOp::Lt),
        "<=" => Ok(BinaryOp::Le),
        ">" => Ok(BinaryOp::Gt),
        ">=" => Ok(BinaryOp::Ge),
        "IN" => Ok(BinaryOp::In),
        "+" => Ok(BinaryOp::Add),
        "-" => Ok(BinaryOp::Sub),
        "*" => Ok(BinaryOp::Mul),
        "/" => Ok(BinaryOp::Div),
        _ => Err(ParseError::new("unknown binary operator")),
    }
}

fn build_unary_expr(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let op_pair = inner
        .next()
        .ok_or_else(|| ParseError::new("missing unary operator"))?;
    let op = match op_pair.as_str().to_ascii_uppercase().as_str() {
        "NOT" => UnaryOp::Not,
        "-" => UnaryOp::Neg,
        _ => return Err(ParseError::new("unknown unary operator")),
    };
    let expr = build_expr(
        inner
            .next()
            .ok_or_else(|| ParseError::new("missing unary operand"))?,
    )?;
    Ok(Expr::Unary {
        op,
        expr: Box::new(expr),
    })
}

fn build_function_call(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let name = next_text(&mut inner, "function name")?;
    let args = inner.map(build_expr).collect::<Result<Vec<_>, _>>()?;
    Ok(Expr::FunctionCall { name, args })
}

fn build_object_field(pair: Pair<'_, Rule>) -> Result<ObjectField, ParseError> {
    let mut inner = pair.into_inner();
    let name = next_text(&mut inner, "object field name")?;
    // Shorthand `{ x }` is `{ x: x }` (AQL parity, D5).
    let value = match inner.next() {
        Some(value) => build_expr(value)?,
        None => Expr::Identifier(vec![name.clone()]),
    };
    Ok(ObjectField { name, value })
}

pub(super) fn parse_string(raw: &str) -> Result<String, ParseError> {
    // Decode JSON escapes only. Literal handles must match bind variables and
    // stored identifiers exactly; text normalization is an explicit function.
    serde_json::from_str(raw).map_err(|e| ParseError::new(format!("invalid string literal: {e}")))
}

pub(super) fn single_inner<'a>(
    pair: Pair<'a, Rule>,
    context: &str,
) -> Result<Pair<'a, Rule>, ParseError> {
    first_non_keyword(pair, context)
}

pub(super) fn first_non_keyword<'a>(
    pair: Pair<'a, Rule>,
    context: &str,
) -> Result<Pair<'a, Rule>, ParseError> {
    let mut inner = pair.into_inner();
    inner
        .find(|pair| !is_keyword_rule(pair.as_rule()))
        .ok_or_else(|| ParseError::new(format!("missing {context}")))
}

pub(super) fn next_non_keyword<'a>(
    inner: &mut Pairs<'a, Rule>,
    context: &str,
) -> Result<Pair<'a, Rule>, ParseError> {
    inner
        .find(|pair| !is_keyword_rule(pair.as_rule()))
        .ok_or_else(|| ParseError::new(format!("missing {context}")))
}

pub(super) fn next_text(inner: &mut Pairs<'_, Rule>, context: &str) -> Result<String, ParseError> {
    inner
        .find(|pair| !is_keyword_rule(pair.as_rule()))
        .map(|p| p.as_str().to_string())
        .ok_or_else(|| ParseError::new(format!("missing {context}")))
}

pub(super) fn is_keyword_rule(rule: Rule) -> bool {
    matches!(
        rule,
        Rule::kw_aggregate
            | Rule::kw_analyze
            | Rule::kw_collect
            | Rule::kw_insert
            | Rule::kw_remove
            | Rule::kw_replace
            | Rule::kw_update
            | Rule::kw_upsert
            | Rule::kw_count
            | Rule::kw_distinct
            | Rule::kw_explain
            | Rule::kw_filter
            | Rule::kw_into
            | Rule::kw_with
            | Rule::kw_for
            | Rule::kw_let
            | Rule::kw_in
            | Rule::kw_limit
            | Rule::kw_return
            | Rule::kw_sort
            | Rule::kw_vector_search
    )
}
