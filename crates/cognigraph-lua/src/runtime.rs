use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::Arc;

use mlua::prelude::*;
use tokio::runtime::Handle;

use cognigraph_core::GraphBackend;

use crate::{LuaExecutionControl, bindings};

/// Sandboxed Lua engine for executing graph queries.
pub struct LuaEngine {
    lua: Lua,
    control: LuaExecutionControl,
}

impl LuaEngine {
    /// Create a new sandboxed Lua engine without graph bindings.
    ///
    /// Suitable for pure computation scripts that don't need database access.
    pub fn new() -> Result<Self, mlua::Error> {
        Self::with_control(LuaExecutionControl::default())
    }

    /// Create a new sandboxed Lua engine with graph primitives available.
    ///
    /// Exposes a `graph` global table with functions:
    /// - `graph.query(cgql, bind_vars)` — execute parsed CGQL when supported
    /// - `graph.get_document(collection, key)` — fetch a document
    /// - `graph.find_documents(collection, opts)` — list documents
    /// - `graph.create_document(collection, doc)` — create a document
    /// - `graph.delete_document(collection, key)` — delete a document
    /// - `graph.neighbors(vertex_id, direction?, collection?)` — get adjacent edges
    /// - `graph.traverse(start_vertex, opts)` — multi-hop graph traversal
    /// - `graph.upsert_edge(from, to, type, data?, collection?)` — create/update edge
    /// - `graph.similarity(collection, vector, opts)` — vector similarity search
    pub fn with_backend(
        backend: Arc<dyn GraphBackend>,
        handle: Handle,
    ) -> Result<Self, mlua::Error> {
        Self::with_backend_mode(backend, handle, false)
    }

    /// `allow_writes` enables mutation-capable graph bindings. Queries always
    /// use parsed CGQL under the same read/write permission and execution limits.
    pub fn with_backend_mode(
        backend: Arc<dyn GraphBackend>,
        handle: Handle,
        allow_writes: bool,
    ) -> Result<Self, mlua::Error> {
        Self::with_backend_control(
            backend,
            handle,
            allow_writes,
            LuaExecutionControl::default(),
        )
    }

    /// Server entry point: all graph callbacks share execution controls.
    pub fn with_backend_control(
        backend: Arc<dyn GraphBackend>,
        handle: Handle,
        allow_writes: bool,
        control: LuaExecutionControl,
    ) -> Result<Self, mlua::Error> {
        let engine = Self::with_control(control.clone())?;
        bindings::register_graph_bindings_with_control(
            &engine.lua,
            backend,
            handle,
            allow_writes,
            control,
        )?;
        Ok(engine)
    }

    pub fn with_control(control: LuaExecutionControl) -> Result<Self, mlua::Error> {
        // Resource termination must propagate past Lua pcall/xpcall.
        let lua = Lua::new_with(
            mlua::StdLib::ALL_SAFE,
            mlua::LuaOptions::default().catch_rust_panics(false),
        )?;
        disable_jit(&lua)?;
        sandbox(&lua)?;
        let hook_control = control.clone();
        lua.set_global_hook(
            mlua::HookTriggers::new().every_nth_instruction(1_000),
            move |_, _| {
                if let Err(error) = hook_control.tick() {
                    // mlua transports Rust unwinds safely across Lua callbacks.
                    // A typed unwind is caught only at execute's host boundary,
                    // so a script cannot catch its resource limit and continue.
                    resume_unwind(Box::new(ExecutionStopped(error.to_string())));
                }
                Ok(mlua::VmState::Continue)
            },
        )?;
        Ok(Self { lua, control })
    }

    /// Set custom instruction limit (default: 1,000,000).
    pub fn set_instruction_limit(&self, limit: u32) {
        self.control.set_instruction_limit(limit);
    }

    /// Execute a Lua script and return the result as JSON.
    ///
    /// Resets the instruction counter before each execution so that the
    /// limit applies per-call rather than accumulating across calls.
    pub fn execute(&self, script: &str) -> Result<serde_json::Value, mlua::Error> {
        self.control.start()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.lua
                .load(script)
                .set_mode(mlua::chunk::ChunkMode::Text)
                .eval::<mlua::Value>()
        }));
        let value = match result {
            Ok(result) => {
                self.control.check()?;
                result?
            }
            Err(payload) => match payload.downcast::<ExecutionStopped>() {
                Ok(stopped) => return Err(mlua::Error::external(stopped.0)),
                Err(other) => resume_unwind(other),
            },
        };
        let json = self.lua.from_value(value)?;
        self.control.check()?;
        Ok(json)
    }
}

/// Remove dangerous modules/functions from the Lua environment.
fn sandbox(lua: &Lua) -> Result<(), mlua::Error> {
    let globals = lua.globals();
    globals.set("os", mlua::Value::Nil)?;
    globals.set("io", mlua::Value::Nil)?;
    globals.set("debug", mlua::Value::Nil)?;
    globals.set("loadfile", mlua::Value::Nil)?;
    globals.set("dofile", mlua::Value::Nil)?;
    globals.set("load", mlua::Value::Nil)?;
    globals.set("loadstring", mlua::Value::Nil)?;
    globals.set("jit", mlua::Value::Nil)?;
    globals.set("package", mlua::Value::Nil)?;
    globals.set("require", mlua::Value::Nil)?;
    Ok(())
}

/// Disable JIT compilation so that instruction-count hooks fire reliably.
///
/// LuaJIT's JIT compiler can bypass debug hooks for tight loops, which
/// would allow `while true do end` to run forever despite the limit.
fn disable_jit(lua: &Lua) -> mlua::Result<()> {
    let jit: LuaTable = lua.globals().get("jit")?;
    jit.get::<LuaFunction>("off")?.call::<()>(())?;
    if jit.get::<LuaFunction>("status")?.call::<bool>(())? {
        return Err(mlua::Error::external("Cannot disable LuaJIT"));
    }
    Ok(())
}

struct ExecutionStopped(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_jit_setup_is_not_ignored() {
        let lua = Lua::new();
        lua.load("jit.off = function() error('cannot disable') end")
            .exec()
            .unwrap();
        assert!(disable_jit(&lua).is_err());
        lua.load("jit.off = function() end; jit.status = function() return true end")
            .exec()
            .unwrap();
        assert!(
            disable_jit(&lua)
                .unwrap_err()
                .to_string()
                .contains("Cannot disable LuaJIT")
        );
    }
}
