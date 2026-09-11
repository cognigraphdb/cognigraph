//! Lua scripting engine for CogniGraph.
//!
//! Provides a sandboxed Lua (LuaJIT) environment for graph queries.
//! Graph primitives are exposed as Lua functions via a `graph` global table.
//!
//! # Example Lua scripts
//!
//! ```lua
//! -- Find all documents in a collection
//! local docs = graph.query("FOR doc IN documents RETURN doc", {})
//!
//! -- Get a specific document
//! local doc = graph.get_document("documents", "123")
//!
//! -- Traverse relationships from a document
//! local paths = graph.traverse("documents/123", {
//!     edge_collection = "document_relations",
//!     max_depth = 3,
//!     direction = "outbound"
//! })
//!
//! -- Get neighbors of a vertex
//! local edges = graph.neighbors("documents/123", "outbound")
//!
//! -- Custom processing with full Lua capabilities
//! local results = {}
//! for _, path in ipairs(paths) do
//!     if path.score > 0.5 then
//!         table.insert(results, path)
//!     end
//! end
//! return results
//! ```

pub mod bindings;
mod control;
pub mod runtime;

pub use control::LuaExecutionControl;
pub use runtime::LuaEngine;

/// Private marker preserved through mlua's `CallbackError` wrapper when a
/// graph binding refuses an unauthorized write.
#[derive(Debug)]
pub(crate) struct WriteAccessDenied {
    operation: String,
    required_scope: String,
}

impl WriteAccessDenied {
    pub(crate) fn new(operation: &str, required_scope: &str) -> Self {
        Self {
            operation: operation.into(),
            required_scope: required_scope.into(),
        }
    }
}

impl std::fmt::Display for WriteAccessDenied {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} requires {}",
            self.operation, self.required_scope
        )
    }
}

impl std::error::Error for WriteAccessDenied {}

/// Returns true only for the typed authorization denial raised by graph
/// mutation bindings. String matching would risk turning unrelated Lua
/// failures into HTTP 403 responses.
pub fn is_write_access_denied(error: &mlua::Error) -> bool {
    error.downcast_ref::<WriteAccessDenied>().is_some()
}

/// General name for the typed RBAC denial marker. The older
/// `is_write_access_denied` export remains for compatibility.
pub fn is_access_denied(error: &mlua::Error) -> bool {
    is_write_access_denied(error)
}

/// Marker for a protected backend collection rejected below a Lua binding.
#[derive(Debug)]
pub(crate) struct ForbiddenBackendAccess {
    operation: String,
    message: String,
}

impl ForbiddenBackendAccess {
    pub(crate) fn new(operation: &str, message: String) -> Self {
        Self {
            operation: operation.into(),
            message,
        }
    }
}

impl std::fmt::Display for ForbiddenBackendAccess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.message)
    }
}

impl std::error::Error for ForbiddenBackendAccess {}

pub fn is_forbidden_backend_access(error: &mlua::Error) -> bool {
    error.downcast_ref::<ForbiddenBackendAccess>().is_some()
}

/// Recognize a typed connection failure through mlua's callback wrappers.
/// User-authored error strings must not turn ordinary script errors into 503s.
pub fn is_backend_unavailable(error: &mlua::Error) -> bool {
    matches!(
        error.downcast_ref::<cognigraph_core::CogniGraphError>(),
        Some(cognigraph_core::CogniGraphError::ConnectionError(_))
    )
}
