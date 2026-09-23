use cognigraph_lua::LuaEngine;
use serde_json::json;

// ---------------------------------------------------------------------------
// 1. Basic execution -- scalar returns
// ---------------------------------------------------------------------------

#[test]
fn execute_return_integer() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return 42").unwrap();
    assert_eq!(result, json!(42));
}

#[test]
fn execute_return_string() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return \"hello\"").unwrap();
    assert_eq!(result, json!("hello"));
}

#[test]
fn execute_return_array() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return {1,2,3}").unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

// ---------------------------------------------------------------------------
// 2. Table return -- verify JSON output
// ---------------------------------------------------------------------------

#[test]
fn execute_return_table_as_json() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine
        .execute(r#"return {name = "alice", age = 30}"#)
        .unwrap();
    assert_eq!(result["name"], json!("alice"));
    assert_eq!(result["age"], json!(30));
}

// ---------------------------------------------------------------------------
// 3-9. Sandbox: dangerous modules/functions are blocked
// ---------------------------------------------------------------------------

#[test]
fn sandbox_os_blocked() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return os.execute('ls')");
    assert!(result.is_err(), "os.execute should be blocked");
}

#[test]
fn sandbox_io_blocked() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return io.open('/etc/passwd')");
    assert!(result.is_err(), "io.open should be blocked");
}

#[test]
fn sandbox_debug_blocked() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return debug.getinfo(1)");
    assert!(result.is_err(), "debug.getinfo should be blocked");
}

#[test]
fn sandbox_load_blocked() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return load('return 1')()");
    assert!(result.is_err(), "load() should be blocked");
}

#[test]
fn sandbox_loadfile_blocked() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return loadfile('/etc/passwd')");
    assert!(result.is_err(), "loadfile() should be blocked");
}

#[test]
fn sandbox_dofile_blocked() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("dofile('/etc/passwd')");
    assert!(result.is_err(), "dofile() should be blocked");
}

#[test]
fn sandbox_require_blocked() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("require('os')");
    assert!(result.is_err(), "require() should be blocked");
}

// ---------------------------------------------------------------------------
// 10. Instruction limit -- infinite loop is terminated
// ---------------------------------------------------------------------------

#[test]
fn instruction_limit_terminates_infinite_loop() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("while true do end");
    assert!(result.is_err(), "infinite loop should be terminated");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("instruction limit"),
        "error should mention instruction limit, got: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// 11. Custom instruction limit
// ---------------------------------------------------------------------------

#[test]
fn custom_instruction_limit_allows_short_work() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    // 100_000 instructions is plenty for a simple loop
    engine.set_instruction_limit(100_000);
    let result = engine.execute(
        r#"
        local sum = 0
        for i = 1, 100 do sum = sum + i end
        return sum
        "#,
    );
    assert!(
        result.is_ok(),
        "short loop should succeed with generous limit, got err: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap(), json!(5050));
}

#[test]
fn custom_instruction_limit_blocks_long_work() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    // Very tight limit -- 1000 instructions
    engine.set_instruction_limit(1_000);
    let result = engine.execute(
        r#"
        local sum = 0
        for i = 1, 1000000 do sum = sum + i end
        return sum
        "#,
    );
    assert!(
        result.is_err(),
        "long loop should fail with tight instruction limit"
    );
}

// ---------------------------------------------------------------------------
// 12. Math works in sandbox
// ---------------------------------------------------------------------------

#[test]
fn math_sqrt_works() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return math.sqrt(144)").unwrap();
    // LuaJIT may return integer 12 or float 12.0 depending on mode.
    let value = result.as_f64().expect("expected a number");
    assert!(
        (value - 12.0).abs() < f64::EPSILON,
        "math.sqrt(144) should be 12, got: {value}"
    );
}

// ---------------------------------------------------------------------------
// 13. String ops work in sandbox
// ---------------------------------------------------------------------------

#[test]
fn string_upper_works() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("return string.upper('hello')").unwrap();
    assert_eq!(result, json!("HELLO"));
}

// ---------------------------------------------------------------------------
// 14. Empty script returns null
// ---------------------------------------------------------------------------

#[test]
fn empty_script_returns_null() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("").unwrap();
    assert!(
        result.is_null(),
        "empty script should return null, got: {result}"
    );
}

// ---------------------------------------------------------------------------
// 15. Syntax error returns an error
// ---------------------------------------------------------------------------

#[test]
fn syntax_error_returns_error() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    let result = engine.execute("this is not valid lua!!!");
    assert!(result.is_err(), "syntax error should produce an Err");
}

#[tokio::test(flavor = "multi_thread")]
async fn graph_query_write_mode_is_rbac_gated() {
    use std::sync::Arc;
    let backend = Arc::new(cognigraph_native::NativeBackend::new());
    let handle = tokio::runtime::Handle::current();

    // Read-only engine (default): mutations are rejected.
    let ro = backend.clone();
    let ro_handle = handle.clone();
    let err = tokio::task::spawn_blocking(move || {
        let engine = cognigraph_lua::LuaEngine::with_backend(ro, ro_handle).unwrap();
        engine
            .execute(r#"return graph.query('INSERT { _key: "x" } INTO docs', {})"#)
            .map_err(|e| e.to_string())
    })
    .await
    .unwrap()
    .unwrap_err();
    assert!(err.contains("not allowed"));

    // Write-mode engine (granted by RBAC at the route): mutations work.
    let rw = backend.clone();
    let result = tokio::task::spawn_blocking(move || {
        let engine = cognigraph_lua::LuaEngine::with_backend_mode(rw, handle, true).unwrap();
        engine
            .execute(
                r#"return graph.query('INSERT { _key: "x", n: 1 } INTO docs RETURN NEW.n', {})"#,
            )
            .map_err(|e| e.to_string())
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result, serde_json::json!([1]));
    assert!(
        cognigraph_core::GraphBackend::get_document(backend.as_ref(), "docs", "x")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_graph_mutations_require_write_mode() {
    use std::sync::Arc;

    let backend = Arc::new(cognigraph_native::NativeBackend::new());
    let handle = tokio::runtime::Handle::current();
    let cases = [
        (
            "graph.create_document",
            r#"return graph.create_document("docs", { _key = "new" })"#,
        ),
        (
            "graph.update_document",
            r#"return graph.update_document("docs", "existing", { n = 2 })"#,
        ),
        (
            "graph.replace_document",
            r#"return graph.replace_document("docs", "existing", { n = 3 })"#,
        ),
        (
            "graph.delete_document",
            r#"return graph.delete_document("docs", "existing")"#,
        ),
        (
            "graph.upsert_edge",
            r#"return graph.upsert_edge("docs/a", "docs/b", "links")"#,
        ),
        (
            "graph.batch",
            r#"return graph.batch({ { op = "insert", collection = "docs", doc = { _key = "batch" } } })"#,
        ),
    ];

    for (operation, script) in cases {
        let backend = backend.clone();
        let handle = handle.clone();
        let err = tokio::task::spawn_blocking(move || {
            let engine = cognigraph_lua::LuaEngine::with_backend(backend, handle).unwrap();
            engine.execute(script).map_err(|error| {
                (
                    cognigraph_lua::is_write_access_denied(&error),
                    error.to_string(),
                )
            })
        })
        .await
        .unwrap()
        .unwrap_err();
        assert!(
            err.0,
            "{operation} must preserve the typed denial marker: {}",
            err.1
        );
        let err = err.1;
        assert!(err.contains(operation), "{operation} error: {err}");
        assert!(
            err.contains("documents:write"),
            "{operation} must name the missing write scope: {err}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn new_bindings_and_strict_directions() {
    use std::sync::Arc;
    let backend = Arc::new(cognigraph_native::NativeBackend::new());
    let handle = tokio::runtime::Handle::current();
    let run = |script: &'static str| {
        let backend = backend.clone();
        let handle = handle.clone();
        tokio::task::spawn_blocking(move || {
            let engine =
                cognigraph_lua::LuaEngine::with_backend_mode(backend, handle, true).unwrap();
            engine.execute(script).map_err(|e| e.to_string())
        })
    };

    // update/replace/text_search/batch end to end.
    let result = run(r#"
        graph.create_document("articles", { _key = "a1", title = "rust engine", content = "fast" })
        local updated = graph.update_document("articles", "a1", { reviewed = true })
        assert(updated.title == "rust engine", "merge keeps fields")
        local replaced = graph.replace_document("articles", "a1", { title = "rewritten rust" })
        assert(replaced.reviewed == nil, "replace swaps")
        local hits = graph.text_search("articles", "rust", { "title" }, 5)
        assert(#hits == 1 and hits[1].document._key == "a1")
        local results = graph.batch({
            { op = "insert", collection = "articles", doc = { _key = "a2", title = "two" } },
            { op = "delete", collection = "articles", key = "a1" },
        })
        assert(#results == 2)
        return graph.get_document("articles", "a2").title
    "#)
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result, serde_json::json!("two"));

    // Batch atomicity surfaces in Lua: conflict rolls the insert back.
    let err = run(r#"
        return graph.batch({
            { op = "insert", collection = "articles", doc = { _key = "a3" } },
            { op = "insert", collection = "articles", doc = { _key = "a2" } },
        })
    "#)
    .await
    .unwrap()
    .unwrap_err();
    assert!(
        err.contains("graph.batch"),
        "errors carry the operation name: {err}"
    );
    let gone = run(r#"return graph.get_document("articles", "a3") == nil"#)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(gone, serde_json::json!(true));

    // Direction typos now error instead of silently meaning outbound.
    let err = run(r#"return graph.neighbors("articles/a2", "in")"#)
        .await
        .unwrap()
        .unwrap_err();
    assert!(err.contains("invalid direction `in`"), "{err}");
}

/// CG-81: the container CVE dispositions for glibc `popen`, `system`, `fopen`
/// mode strings and `dlopen` rely on no script reaching LuaJIT's io, os or
/// package libraries by any route, not only by their global names.
#[test]
fn sandbox_closes_every_route_to_process_file_and_library_loading() {
    let engine = LuaEngine::new().expect("failed to create LuaEngine");
    for script in [
        "return io.popen('id')",
        "return package.loadlib('libc.so.6', 'system')",
        "return _G.io.open('/etc/hostname')",
        "return _G.os.execute('id')",
        "return getfenv(0).os.execute('id')",
        "return rawget(_G, 'io').open('/etc/hostname')",
        "return debug.getregistry()._LOADED.io",
    ] {
        let result = engine.execute(script);
        assert!(result.is_err(), "{script} must be refused, got {result:?}");
    }
    let visible = engine
        .execute("return {type(io), type(os), type(package), type(debug), type(require)}")
        .unwrap();
    assert_eq!(
        visible,
        serde_json::json!(["nil", "nil", "nil", "nil", "nil"])
    );
}
