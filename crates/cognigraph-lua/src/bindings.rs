use std::collections::HashMap;
use std::sync::Arc;

use mlua::prelude::*;
use tokio::runtime::Handle;

use crate::LuaExecutionControl;

use cognigraph_core::{Direction, GraphBackend, TraversalOpts, VectorSearchOpts};

/// Registers graph primitives on the Lua `graph` global table.
///
/// Bridges the async `GraphBackend` into synchronous Lua functions
/// by using `Handle::block_on()` from the captured tokio runtime.
pub fn register_graph_bindings(
    lua: &Lua,
    backend: Arc<dyn GraphBackend>,
    handle: Handle,
) -> Result<(), mlua::Error> {
    register_graph_bindings_with_mode(lua, backend, handle, false)
}

/// `allow_writes` enables every mutation-capable binding. The caller must
/// derive it from an authenticated write scope; it is deliberately false by
/// default, including when authentication is disabled.
pub fn register_graph_bindings_with_mode(
    lua: &Lua,
    backend: Arc<dyn GraphBackend>,
    handle: Handle,
    allow_writes: bool,
) -> Result<(), mlua::Error> {
    register_graph_bindings_with_control(
        lua,
        backend,
        handle,
        allow_writes,
        LuaExecutionControl::default(),
    )
}

pub(crate) fn register_graph_bindings_with_control(
    lua: &Lua,
    backend: Arc<dyn GraphBackend>,
    handle: Handle,
    allow_writes: bool,
    control: LuaExecutionControl,
) -> Result<(), mlua::Error> {
    let graph = lua.create_table()?;

    // graph.backend -> the active Native store identity
    graph.set("backend", backend.backend_name())?;
    graph.set("query_language", backend.query_language().as_str())?;

    // graph.query(query_string, bind_vars) -> results
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let cgql = backend.query_language() == cognigraph_core::QueryLanguage::Cgql;
        let query_fn =
            lua.create_function(move |lua, (query, bind_vars): (String, LuaValue)| {
                // A declared language cannot authorize opaque passthrough.
                require_access(cgql, "graph.query", "a backend with parsed CGQL support")?;
                let vars = lua_to_bind_vars(lua, bind_vars)?;
                let results = {
                    let mode = if allow_writes {
                        cognigraph_query::QueryMode::ReadWrite
                    } else {
                        cognigraph_query::QueryMode::ReadOnly
                    };
                    control
                        .block_on(
                            &handle,
                            cognigraph_query::parse_and_execute_backend_with_options(
                                &query,
                                backend.as_ref(),
                                &vars,
                                mode,
                                control.query_budget()?,
                            ),
                        )?
                        .map_err(|error| query_error("graph.query", error))?
                };
                lua.to_value(&results)
            })?;
        graph.set("query", query_fn)?;
    }

    // graph.get_document(collection, key) -> document or nil
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let get_fn = lua.create_function(move |lua, (collection, key): (String, String)| {
            let doc = control
                .block_on(&handle, backend.get_document(&collection, &key))?
                .map_err(|error| backend_error("graph.get_document", error))?;
            match doc {
                Some(d) => lua.to_value(&d),
                None => Ok(LuaValue::Nil),
            }
        })?;
        graph.set("get_document", get_fn)?;
    }

    // graph.find_documents(collection, opts_table) -> documents
    // opts_table: { limit = 10, offset = 0 }
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let find_fn =
            lua.create_function(move |lua, (collection, opts): (String, Option<LuaTable>)| {
                let limit = opts
                    .as_ref()
                    .and_then(|t| t.get::<usize>("limit").ok())
                    .or(Some(100));
                let offset = opts.as_ref().and_then(|t| t.get::<usize>("offset").ok());
                let results = control
                    .block_on(&handle, backend.list_documents(&collection, limit, offset))?
                    .map_err(|error| backend_error("graph.find_documents", error))?;
                lua.to_value(&results)
            })?;
        graph.set("find_documents", find_fn)?;
    }

    // graph.create_document(collection, doc_table) -> { collection, key }
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let create_fn =
            lua.create_function(move |lua, (collection, doc): (String, LuaValue)| {
                require_access(allow_writes, "graph.create_document", "documents:write")?;
                let doc_json: serde_json::Value = lua.from_value(doc)?;
                let id = control
                    .block_on(&handle, backend.create_document(&collection, doc_json))?
                    .map_err(|error| backend_error("graph.create_document", error))?;
                let result = lua.create_table()?;
                result.set("collection", id.collection.as_str())?;
                result.set("key", id.key.as_str())?;
                result.set("_id", id.full_id())?;
                Ok(LuaValue::Table(result))
            })?;
        graph.set("create_document", create_fn)?;
    }

    // graph.neighbors(vertex_id, direction?) -> edges
    // direction: "outbound" (default), "inbound", "any"
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let neighbors_fn =
            lua.create_function(
                move |lua,
                      (vertex_id, direction, collection): (
                    String,
                    Option<String>,
                    Option<String>,
                )| {
                    let dir = parse_direction(direction.as_deref())?;
                    let col = collection.unwrap_or_else(|| "document_relations".into());
                    let edges = control
                        .block_on(&handle, backend.get_edges(&col, &vertex_id, dir))?
                        .map_err(|error| backend_error("graph.neighbors", error))?;
                    lua.to_value(&edges)
                },
            )?;
        graph.set("neighbors", neighbors_fn)?;
    }

    // graph.traverse(start_vertex, opts_table) -> paths
    // opts_table: { edge_collection, max_depth, min_depth, direction, min_confidence, path_decay }
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let traverse_fn = lua.create_function(
            move |lua, (start_vertex, opts): (String, Option<LuaTable>)| {
                let opts_table = opts.as_ref();
                let traversal_opts = TraversalOpts {
                    edge_collection: opts_table
                        .and_then(|t| t.get::<String>("edge_collection").ok())
                        .unwrap_or_else(|| "document_relations".into()),
                    max_depth: opts_table
                        .and_then(|t| t.get::<u32>("max_depth").ok())
                        .unwrap_or(3),
                    min_depth: opts_table
                        .and_then(|t| t.get::<u32>("min_depth").ok())
                        .unwrap_or(1),
                    direction: opts_table
                        .and_then(|t| t.get::<String>("direction").ok())
                        .map(|s| parse_direction(Some(&s)))
                        .transpose()?
                        .unwrap_or_default(),
                    min_confidence: opts_table.and_then(|t| t.get::<f64>("min_confidence").ok()),
                    path_decay: opts_table
                        .and_then(|t| t.get::<f64>("path_decay").ok())
                        .unwrap_or(0.8),
                };
                let paths = control
                    .block_on(&handle, backend.traverse(&start_vertex, &traversal_opts))?
                    .map_err(|error| backend_error("graph.traverse", error))?;
                lua.to_value(&paths)
            },
        )?;
        graph.set("traverse", traverse_fn)?;
    }

    // graph.upsert_edge(from, to, relation_type, data?) -> edge
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let upsert_fn = lua.create_function(
            move |lua,
                  (from, to, relation_type, data, collection): (
                String,
                String,
                String,
                Option<LuaValue>,
                Option<String>,
            )| {
                require_access(allow_writes, "graph.upsert_edge", "documents:write")?;
                let col = collection.unwrap_or_else(|| "document_relations".into());
                let data_json: serde_json::Value = match data {
                    Some(v) => lua.from_value(v)?,
                    None => serde_json::json!({}),
                };
                let edge = control
                    .block_on(
                        &handle,
                        backend.upsert_edge(&col, &from, &to, &relation_type, data_json),
                    )?
                    .map_err(|error| backend_error("graph.upsert_edge", error))?;
                lua.to_value(&edge)
            },
        )?;
        graph.set("upsert_edge", upsert_fn)?;
    }

    // graph.similarity(collection, vector, opts?) -> hits
    // opts: { threshold, limit, model_name }
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let similarity_fn = lua.create_function(
            move |lua, (collection, vector, opts): (String, Vec<f64>, Option<LuaTable>)| {
                let opts_table = opts.as_ref();
                let search_opts = VectorSearchOpts {
                    threshold: Some(
                        opts_table
                            .and_then(|t| t.get::<f64>("threshold").ok())
                            .unwrap_or(0.7),
                    ),
                    limit: opts_table
                        .and_then(|t| t.get::<usize>("limit").ok())
                        .unwrap_or(10),
                    model_name: opts_table.and_then(|t| t.get::<String>("model_name").ok()),
                };
                let hits = control
                    .block_on(
                        &handle,
                        backend.vector_search(&collection, &vector, &search_opts),
                    )?
                    .map_err(|error| backend_error("graph.similarity", error))?;
                lua.to_value(&hits)
            },
        )?;
        graph.set("similarity", similarity_fn)?;
    }

    // graph.delete_document(collection, key) -> boolean
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let delete_fn = lua.create_function(move |_lua, (collection, key): (String, String)| {
            require_access(allow_writes, "graph.delete_document", "documents:write")?;
            let deleted = control
                .block_on(&handle, backend.delete_document(&collection, &key))?
                .map_err(|error| backend_error("graph.delete_document", error))?;
            Ok(deleted)
        })?;
        graph.set("delete_document", delete_fn)?;
    }

    // graph.update_document(collection, key, merge) -> updated document
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let update_fn = lua.create_function(
            move |lua, (collection, key, merge): (String, String, LuaValue)| {
                require_access(allow_writes, "graph.update_document", "documents:write")?;
                let merge: serde_json::Value = lua.from_value(merge)?;
                let updated = control
                    .block_on(&handle, backend.update_document(&collection, &key, merge))?
                    .map_err(|error| backend_error("graph.update_document", error))?;
                lua.to_value(&updated)
            },
        )?;
        graph.set("update_document", update_fn)?;
    }

    // graph.replace_document(collection, key, doc) -> replaced document
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let replace_fn = lua.create_function(
            move |lua, (collection, key, doc): (String, String, LuaValue)| {
                require_access(allow_writes, "graph.replace_document", "documents:write")?;
                let doc: serde_json::Value = lua.from_value(doc)?;
                let replaced = control
                    .block_on(&handle, backend.replace_document(&collection, &key, doc))?
                    .map_err(|error| backend_error("graph.replace_document", error))?;
                lua.to_value(&replaced)
            },
        )?;
        graph.set("replace_document", replace_fn)?;
    }

    // graph.text_search(collection, query, fields?, limit?) -> hits (BM25)
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let text_fn = lua.create_function(
            move |lua,
                  (collection, query, fields, limit): (
                String,
                String,
                Option<Vec<String>>,
                Option<usize>,
            )| {
                let fields =
                    fields.unwrap_or_else(|| vec!["title".to_string(), "content".to_string()]);
                let hits = control
                    .block_on(
                        &handle,
                        backend.text_search(&collection, &query, &fields, limit.unwrap_or(10)),
                    )?
                    .map_err(|error| backend_error("graph.text_search", error))?;
                lua.to_value(&hits)
            },
        )?;
        graph.set("text_search", text_fn)?;
    }

    // graph.batch(ops) -> results; atomic on backends that support it
    // (ops use the same shape as POST /batch: {op=..., collection=..., ...})
    {
        let backend = backend.clone();
        let handle = handle.clone();
        let control = control.clone();
        let batch_fn = lua.create_function(move |lua, ops: LuaValue| {
            require_access(allow_writes, "graph.batch", "documents:write")?;
            let ops: Vec<cognigraph_core::BatchOp> = lua.from_value(ops)?;
            let results = control
                .block_on(&handle, backend.execute_batch(ops))?
                .map_err(|error| backend_error("graph.batch", error))?;
            lua.to_value(&results)
        })?;
        graph.set("batch", batch_fn)?;
    }

    lua.globals().set("graph", graph)?;
    Ok(())
}

fn require_access(allowed: bool, operation: &str, required_scope: &str) -> Result<(), mlua::Error> {
    if allowed {
        Ok(())
    } else {
        Err(mlua::Error::external(crate::WriteAccessDenied::new(
            operation,
            required_scope,
        )))
    }
}

fn backend_error(operation: &str, error: cognigraph_core::CogniGraphError) -> mlua::Error {
    match error {
        cognigraph_core::CogniGraphError::Forbidden(message) => {
            mlua::Error::external(crate::ForbiddenBackendAccess::new(operation, message))
        }
        cognigraph_core::CogniGraphError::ConnectionError(message) => mlua::Error::external(
            cognigraph_core::CogniGraphError::ConnectionError(format!("{operation}: {message}")),
        ),
        other => mlua::Error::external(format!("{operation}: {other}")),
    }
}

fn query_error(operation: &str, error: cognigraph_query::ExecutionError) -> mlua::Error {
    match error {
        cognigraph_query::ExecutionError::Forbidden(message) => {
            mlua::Error::external(crate::ForbiddenBackendAccess::new(operation, message))
        }
        cognigraph_query::ExecutionError::Connection(message) => backend_error(
            operation,
            cognigraph_core::CogniGraphError::ConnectionError(message),
        ),
        other => mlua::Error::external(format!("{operation}: {other}")),
    }
}

fn parse_direction(s: Option<&str>) -> Result<Direction, mlua::Error> {
    match s {
        None | Some("outbound") => Ok(Direction::Outbound),
        Some("inbound") => Ok(Direction::Inbound),
        Some("any") => Ok(Direction::Any),
        Some(other) => Err(mlua::Error::external(format!(
            "invalid direction `{other}` (expected outbound, inbound, or any)"
        ))),
    }
}

/// Convert a Lua value (table or nil) into a HashMap for CGQL bind variables.
fn lua_to_bind_vars(
    lua: &Lua,
    value: LuaValue,
) -> Result<HashMap<String, serde_json::Value>, mlua::Error> {
    match value {
        LuaValue::Table(_) => {
            let map: HashMap<String, serde_json::Value> = lua.from_value(value)?;
            Ok(map)
        }
        LuaValue::Nil => Ok(HashMap::new()),
        _ => Err(mlua::Error::external("bind_vars must be a table or nil")),
    }
}
