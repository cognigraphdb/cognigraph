//! CG-104: what a script can and cannot reach.

use cognigraph_lua::LuaEngine;
use serde_json::json;

fn engine() -> LuaEngine {
    LuaEngine::new().expect("failed to create LuaEngine")
}

/// The exact set of globals is pinned. A dependency update that adds one, or
/// a change that loads another library, fails here before it ships.
#[test]
fn script_globals_are_an_exact_allow_list() {
    let names = engine()
        .execute(
            "local names = {} for name in pairs(_G) do names[#names + 1] = name end \
             table.sort(names) return names",
        )
        .unwrap();
    assert_eq!(
        names,
        json!([
            "_G",
            "_VERSION",
            "assert",
            "bit",
            "collectgarbage",
            "coroutine",
            "error",
            "gcinfo",
            "getfenv",
            "getmetatable",
            "ipairs",
            "math",
            "newproxy",
            "next",
            "pairs",
            "pcall",
            "print",
            "rawequal",
            "rawget",
            "rawset",
            "select",
            "setfenv",
            "setmetatable",
            "string",
            "table",
            "tonumber",
            "tostring",
            "type",
            "unpack",
            "xpcall"
        ])
    );
}

/// The package library's `module()` used to replace the chunk environment with
/// any library that was loaded but hidden. None of these may run.
#[test]
fn module_and_require_cannot_reach_hidden_libraries() {
    let engine = engine();
    for script in [
        r#"module("os"); return execute("true")"#,
        r#"module("os"); return getenv("HOME")"#,
        r#"module("io"); return popen("echo probe"):read("*a")"#,
        r#"module("io"); return open("/etc/hostname")"#,
        r#"module("jit"); on(); return status()"#,
        r#"module("package"); return loadlib("libc.so.6", "system")"#,
        r#"module("debug"); return getregistry()"#,
        r#"return require("os")"#,
        r#"return require("ffi")"#,
        r#"return getfenv(0).module("os")"#,
        r#"return rawget(_G, "module")("io")"#,
        r#"return _G["module"]("os")"#,
        r#"return pcall(module, "os")"#,
        r#"return coroutine.wrap(function() module("os") return execute("true") end)()"#,
        r#"return getmetatable("").__index.rep == string.rep and os.execute("true")"#,
        r#"return load("return 1")()"#,
        r#"return loadstring("return 1")()"#,
        r#"return dofile("/etc/hostname")"#,
        r#"return string.dump(function() end) and loadstring"#,
    ] {
        let result = engine.execute(script);
        match result {
            Err(_) => {}
            // `pcall(module, ...)` returns false plus an error message; a
            // truthy first value would mean the call went through.
            Ok(value) => assert!(
                value == json!(false) || value.is_null(),
                "{script} must be refused, got {value}"
            ),
        }
    }
}

/// With the JIT library gone a script cannot switch compilation back on, so
/// the instruction limit holds in the main chunk, inside `pcall` and inside a
/// coroutine.
#[test]
fn instruction_limit_cannot_be_lifted_by_reenabling_the_jit() {
    for script in [
        "while true do end",
        "pcall(function() jit.on() end) while true do end",
        "pcall(function() module('jit') on() end) while true do end",
        "coroutine.wrap(function() while true do end end)()",
    ] {
        let engine = engine();
        engine.set_instruction_limit(50_000);
        let error = engine.execute(script).unwrap_err().to_string();
        assert!(error.contains("instruction limit"), "{script}: {error}");
    }
}

/// The libraries a script is meant to use still work.
#[test]
fn allowed_libraries_remain_usable() {
    let value = engine()
        .execute(
            r#"local parts = {}
               for word in string.gmatch("alpha beta", "%a+") do table.insert(parts, word) end
               local co = coroutine.wrap(function() coroutine.yield(1) return 2 end)
               return {
                 upper = string.upper(table.concat(parts, "-")),
                 floor = math.floor(2.7),
                 band = bit.band(6, 3),
                 first = co(),
                 second = co(),
                 ok = select(1, pcall(error, "x")) == false,
               }"#,
        )
        .unwrap();
    assert_eq!(
        value,
        json!({"upper": "ALPHA-BETA", "floor": 2, "band": 2, "first": 1, "second": 2, "ok": true})
    );
}
