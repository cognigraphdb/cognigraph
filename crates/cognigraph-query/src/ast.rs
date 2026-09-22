use serde::{Deserialize, Serialize};

/// A parsed CGQL query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    /// EXPLAIN prefix: describe the plan instead of executing it.
    pub explain: bool,
    /// EXPLAIN ANALYZE: execute the query and report per-stage statistics
    /// instead of rows. Requires bind variables; rejects mutations.
    pub analyze: bool,
    /// FOR / LET / FILTER in written order — v2 positional semantics: each
    /// clause sees exactly the variables bound above it. Zero FORs means
    /// the body runs on one synthetic row (`RETURN 1`, `LET x = (…)`).
    pub body: Vec<BodyClause>,
    pub collect: Option<CollectClause>,
    pub sort: Option<SortClause>,
    pub limit: Option<LimitClause>,
    pub distinct: bool,
    pub mutation: Option<MutationClause>,
    /// Absent only for mutations without a RETURN.
    pub return_expr: Option<Expr>,
    pub bind_vars: Vec<String>,
}

impl Query {
    /// First FOR of the body (v1-shaped convenience view).
    pub fn for_clause(&self) -> Option<ForClause> {
        self.body.iter().find_map(|clause| match clause {
            BodyClause::For(for_clause) => Some(for_clause.clone()),
            _ => None,
        })
    }

    /// All FILTER expressions of the body, in order.
    pub fn filters(&self) -> Vec<Expr> {
        self.body
            .iter()
            .filter_map(|clause| match clause {
                BodyClause::Filter(filter) => Some(filter.clone()),
                _ => None,
            })
            .collect()
    }

    /// All LET clauses of the body, in order.
    pub fn lets(&self) -> Vec<LetClause> {
        self.body
            .iter()
            .filter_map(|clause| match clause {
                BodyClause::Let(let_clause) => Some(let_clause.clone()),
                _ => None,
            })
            .collect()
    }

    /// Every collection this query reads or writes, resolved with the
    /// planner's rule: a bare `FOR x IN name` source is an in-scope
    /// variable first, a collection otherwise. Covers vector-search
    /// sources, traversal edge collections, mutation targets, and
    /// subqueries (which see outer bindings).
    pub fn referenced_collections(&self) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        self.collect_collections(&mut std::collections::BTreeSet::new(), &mut out);
        out
    }

    fn collect_collections(
        &self,
        scope: &mut std::collections::BTreeSet<String>,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        fn walk_expr(
            expr: &Expr,
            scope: &std::collections::BTreeSet<String>,
            out: &mut std::collections::BTreeSet<String>,
        ) {
            match expr {
                Expr::Array(items) => items.iter().for_each(|item| walk_expr(item, scope, out)),
                Expr::Object(fields) => fields
                    .iter()
                    .for_each(|field| walk_expr(&field.value, scope, out)),
                Expr::Unary { expr, .. } => walk_expr(expr, scope, out),
                Expr::Binary { left, right, .. } => {
                    walk_expr(left, scope, out);
                    walk_expr(right, scope, out);
                }
                Expr::FunctionCall { args, .. } => {
                    args.iter().for_each(|arg| walk_expr(arg, scope, out))
                }
                Expr::Subquery(query) => query.collect_collections(&mut scope.clone(), out),
                _ => {}
            }
        }

        for clause in &self.body {
            match clause {
                BodyClause::For(ForClause::Collection { var, collection }) => {
                    if !scope.contains(collection) {
                        out.insert(collection.clone());
                    }
                    scope.insert(var.clone());
                }
                BodyClause::For(ForClause::Expression { var, expr }) => {
                    walk_expr(expr, scope, out);
                    scope.insert(var.clone());
                }
                BodyClause::For(ForClause::VectorSearch {
                    var,
                    collection,
                    vector,
                }) => {
                    out.insert(collection.clone());
                    walk_expr(vector, scope, out);
                    scope.insert(var.clone());
                }
                BodyClause::For(ForClause::Traversal {
                    vertex_var,
                    edge_var,
                    path_var,
                    start,
                    edge_collection,
                    ..
                }) => {
                    out.insert(edge_collection.clone());
                    walk_expr(start, scope, out);
                    scope.insert(vertex_var.clone());
                    scope.insert(edge_var.clone());
                    scope.insert(path_var.clone());
                }
                BodyClause::Let(let_clause) => {
                    walk_expr(&let_clause.expr, scope, out);
                    scope.insert(let_clause.name.clone());
                }
                BodyClause::Filter(filter) => walk_expr(filter, scope, out),
            }
        }
        if let Some(collect) = &self.collect {
            for binding in &collect.bindings {
                walk_expr(&binding.expr, scope, out);
            }
            for aggregate in &collect.aggregates {
                walk_expr(&aggregate.expr, scope, out);
            }
            if let Some(into) = &collect.into
                && let Some(projection) = &into.projection
            {
                walk_expr(projection, scope, out);
            }
        }
        if let Some(sort) = &self.sort {
            for key in &sort.keys {
                walk_expr(&key.expr, scope, out);
            }
        }
        if let Some(mutation) = &self.mutation {
            match mutation {
                MutationClause::Insert { doc, collection } => {
                    out.insert(collection.clone());
                    walk_expr(doc, scope, out);
                }
                MutationClause::Update {
                    key,
                    with,
                    collection,
                }
                | MutationClause::Replace {
                    key,
                    with,
                    collection,
                } => {
                    out.insert(collection.clone());
                    walk_expr(key, scope, out);
                    walk_expr(with, scope, out);
                }
                MutationClause::Remove { key, collection } => {
                    out.insert(collection.clone());
                    walk_expr(key, scope, out);
                }
                MutationClause::Upsert {
                    search,
                    insert,
                    update,
                    collection,
                } => {
                    out.insert(collection.clone());
                    walk_expr(search, scope, out);
                    walk_expr(insert, scope, out);
                    walk_expr(update, scope, out);
                }
            }
        }
        if let Some(return_expr) = &self.return_expr {
            walk_expr(return_expr, scope, out);
        }
    }
}

/// One positional body clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BodyClause {
    For(ForClause),
    Let(LetClause),
    Filter(Expr),
}

/// A per-row variable binding, evaluated at its position in the body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LetClause {
    pub name: String,
    pub expr: Expr,
}

/// The query source and variables introduced by `FOR`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ForClause {
    /// `FOR x IN name` where name is a bare identifier: an in-scope
    /// variable if one is bound, otherwise a collection (resolved at plan
    /// time; shadowing is rejected by validation).
    Collection { var: String, collection: String },
    /// `FOR x IN <expr>` over any array-valued expression.
    Expression { var: String, expr: Expr },
    VectorSearch {
        var: String,
        collection: String,
        vector: Expr,
    },
    Traversal {
        vertex_var: String,
        edge_var: String,
        path_var: String,
        min_depth: u32,
        max_depth: u32,
        direction: Direction,
        start: Expr,
        edge_collection: String,
    },
}

/// Traversal direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Outbound,
    Inbound,
    Any,
}

/// A data-modification clause. UPDATE merges (partial update); REPLACE
/// swaps the whole document. Atomicity is per document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MutationClause {
    Insert {
        doc: Expr,
        collection: String,
    },
    Update {
        key: Expr,
        with: Expr,
        collection: String,
    },
    Replace {
        key: Expr,
        with: Expr,
        collection: String,
    },
    Remove {
        key: Expr,
        collection: String,
    },
    /// Search object (`_key` fast path, else first all-fields match in key
    /// order); found → partial UPDATE merge, absent → INSERT as-is.
    Upsert {
        search: Expr,
        insert: Expr,
        update: Expr,
        collection: String,
    },
}

/// Grouping: rows collapse to one row per distinct key tuple. After
/// COLLECT only the binding names (and the count variable) are in scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectClause {
    pub bindings: Vec<CollectBinding>,
    pub aggregates: Vec<AggregateBinding>,
    pub into: Option<CollectInto>,
    pub count_into: Option<String>,
}

/// `INTO name [= expr]`: binds each group's captured rows as an array.
/// With a projection expression, captures that value per row; without one,
/// captures an object of all in-scope variables per row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectInto {
    pub name: String,
    pub projection: Option<Expr>,
}

/// Per-group aggregate: `name = FUNC(expr)` where FUNC is SUM/MIN/MAX/AVG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateBinding {
    pub name: String,
    pub function: String,
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectBinding {
    pub name: String,
    pub expr: Expr,
}

/// A sort clause: one or more keys, compared lexicographically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortClause {
    pub keys: Vec<SortKey>,
    /// Optional BCP-47 locale for string comparisons (`COLLATE "de"`).
    pub collation: Option<String>,
}

/// A single sort key with its direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortKey {
    pub expr: Expr,
    pub direction: SortDirection,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// A LIMIT operand: a literal, or a bind variable resolved before execution
/// (CG-85). Literals keep their bare JSON number shape; binds serialize as
/// `{"bind": "name"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LimitValue {
    Literal(u64),
    Bind { bind: String },
}

impl std::fmt::Display for LimitValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Literal(n) => write!(f, "{n}"),
            Self::Bind { bind } => write!(f, "@{bind}"),
        }
    }
}

/// A limit clause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitClause {
    pub offset: Option<LimitValue>,
    pub count: LimitValue,
}

/// CGQL expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Identifier(Vec<String>),
    BindVar(String),
    String(String),
    Number(f64),
    Bool(bool),
    Null,
    Array(Vec<Expr>),
    Object(Vec<ObjectField>),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
    /// Field access on a non-identifier base: `DOCUMENT(x).name`, `FIRST(xs).y`.
    /// A missing field is null, exactly as for a dotted identifier path.
    Field {
        base: Box<Expr>,
        name: String,
    },
    /// `cond ? then : otherwise`. Only `true` takes the then-branch, matching
    /// how FILTER treats truth — a non-boolean condition is not "truthy".
    Conditional {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    /// A parenthesized read query; grammar restricts it to LET values.
    Subquery(Box<Query>),
}

/// Object projection field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectField {
    pub name: String,
    pub value: Expr,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Neg,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Add,
    Sub,
    Mul,
    Div,
}
