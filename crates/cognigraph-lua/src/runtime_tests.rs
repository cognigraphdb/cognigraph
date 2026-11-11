//! Runtime construction and sandbox invariants.

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

fn loaded_names(lua: &Lua, registry_table: &str) -> Vec<String> {
    let mlua::Value::Table(table) = lua
        .named_registry_value::<mlua::Value>(registry_table)
        .unwrap()
    else {
        return Vec::new();
    };
    let mut names: Vec<String> = table
        .pairs::<String, mlua::Value>()
        .map(|pair| pair.unwrap().0)
        .collect();
    names.sort();
    names
}

/// CG-104: the registry is what `module()` and `require()` consult, so it must
/// hold nothing but the libraries a script is allowed to use.
#[test]
fn registry_keeps_only_the_allowed_libraries() {
    let engine = LuaEngine::new().unwrap();
    assert_eq!(
        loaded_names(&engine.lua, "_LOADED"),
        ["_G", "bit", "coroutine", "math", "string", "table"]
    );
    assert!(loaded_names(&engine.lua, "_PRELOAD").is_empty());
}

/// A state built the old way (everything loaded, globals hidden) must be
/// refused, so a future change to the library set cannot reopen CG-104.
#[test]
fn verification_rejects_hidden_but_loaded_libraries() {
    let lua = Lua::new();
    for name in FORBIDDEN_GLOBALS {
        lua.globals().raw_set(name, mlua::Value::Nil).unwrap();
    }
    let error = verify_sandbox(&lua).unwrap_err().to_string();
    assert!(error.contains("left a library registered"), "{error}");

    let lua = Lua::new();
    sandbox(&lua).unwrap();
    verify_sandbox(&lua).unwrap();
    lua.globals().raw_set("module", "anything").unwrap();
    let error = verify_sandbox(&lua).unwrap_err().to_string();
    assert!(error.contains("`module` reachable"), "{error}");
}
