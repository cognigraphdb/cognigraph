use std::sync::Arc;
use std::time::{Duration, Instant};

use cognigraph_lua::{LuaEngine, LuaExecutionControl};
use cognigraph_native::NativeBackend;
use cognigraph_query::ExecutionBudget;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn write_permission_never_enables_opaque_query_passthrough() {
    for allow_writes in [false, true] {
        let backend = Arc::new(cognigraph_core::contract::NoAccessBackend::default());
        let probe = backend.clone();
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let engine = LuaEngine::with_backend_mode(backend, handle, allow_writes).unwrap();
            for query in ["RETURN 1", "INSERT {} INTO notes"] {
                let script = format!("return graph.query({query:?})");
                let error = engine.execute(&script).unwrap_err();
                assert!(cognigraph_lua::is_access_denied(&error));
                assert!(error.to_string().contains("parsed CGQL support"));
            }
        })
        .await
        .unwrap();
        probe.assert_unused();
    }
}

#[test]
fn jit_and_loader_restoration_are_unavailable() {
    let engine = LuaEngine::new().unwrap();
    for script in [
        "jit.on(); return 1",
        "return require('jit')",
        "return package.loaded.jit",
        "return loadstring('jit.on(); return 1')()",
        "return load('jit.on(); return 1')()",
    ] {
        assert!(engine.execute(script).is_err(), "{script}");
    }
    assert_eq!(
        engine
            .execute("return {type(jit), type(package), type(require), type(loadstring)}")
            .unwrap(),
        json!(["nil", "nil", "nil", "nil"])
    );
}

#[test]
fn functions_coroutines_and_protected_calls_cannot_swallow_instruction_termination() {
    let engine = LuaEngine::new().unwrap();
    engine.set_instruction_limit(10_000);
    for script in [
        "local n=0; for i=1,2000000 do n=n+i end; return n",
        "local function f() local n=0; for i=1,2000000 do n=n+i end end; f(); return 'escaped'",
        "local f=coroutine.create(function() local n=0; for i=1,2000000 do n=n+i end end); coroutine.resume(f); return 'escaped'",
        "coroutine.wrap(function() local n=0; for i=1,2000000 do n=n+i end end)(); return 'escaped'",
        "pcall(function() local n=0; for i=1,2000000 do n=n+i end end); return 'escaped'",
        "xpcall(function() local n=0; for i=1,2000000 do n=n+i end end, function() return 'caught' end); return 'escaped'",
        "for i=1,200000 do pcall(function() local n=0; for j=1,100 do n=n+j end end) end; return 'escaped'",
    ] {
        let error = engine.execute(script).unwrap_err().to_string();
        assert!(error.contains("instruction limit"), "{script}: {error}");
        // Limits reset for independent executions; the VM remains usable.
        assert_eq!(engine.execute("return 42").unwrap(), json!(42));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn both_query_permissions_receive_row_limits_and_share_elapsed_time() {
    for allow_writes in [false, true] {
        let backend = Arc::new(NativeBackend::new());
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let control = LuaExecutionControl::new(ExecutionBudget {
                max_source_rows: Some(3),
                time_budget_ms: Some(100),
                ..ExecutionBudget::default()
            });
            let engine =
                LuaEngine::with_backend_control(backend, handle, allow_writes, control).unwrap();
            engine.set_instruction_limit(u32::MAX);
            let err = engine
                .execute(r#"return graph.query("FOR n IN [1,2,3,4,5] RETURN n")"#)
                .unwrap_err();
            assert!(err.to_string().contains("row budget"), "{err}");
            assert_eq!(
                engine
                    .execute(r#"return graph.query("FOR n IN [1,2] RETURN n")"#)
                    .unwrap(),
                json!([1, 2])
            );
            let start = Instant::now();
            let err = engine
                .execute(
                    r#"
                for i=1,1000000 do
                    pcall(function() graph.query("FOR n IN [1,2] RETURN n") end)
                end
                return "escaped"
            "#,
                )
                .unwrap_err();
            assert!(err.to_string().contains("time budget"), "{err}");
            assert!(start.elapsed() < Duration::from_secs(2));
        })
        .await
        .unwrap();
    }
}

/// CG-111: a query run from Lua draws on the same intermediate-row and
/// memory budgets as one sent over HTTP.
#[tokio::test(flavor = "multi_thread")]
async fn queries_from_lua_inherit_the_row_and_memory_budgets() {
    let backend = Arc::new(NativeBackend::new());
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let control = LuaExecutionControl::new(ExecutionBudget {
            max_intermediate_rows: Some(4),
            max_memory_bytes: Some(4_096),
            ..ExecutionBudget::unlimited()
        });
        let engine = LuaEngine::with_backend_control(backend, handle, false, control).unwrap();
        engine.set_instruction_limit(u32::MAX);
        // 3 rows from the first FOR and 9 from the second.
        let err = engine
            .execute(r#"return graph.query("FOR a IN [1,2,3] FOR b IN [1,2,3] RETURN 1")"#)
            .unwrap_err();
        assert!(
            err.to_string().contains("intermediate row budget (4 rows)"),
            "{err}"
        );
        // Three tenfold concatenations make 10,000 bytes.
        let err = engine
            .execute(
                r#"return graph.query([[
                    LET a = "0123456789"
                    LET b = CONCAT(a, a, a, a, a, a, a, a, a, a)
                    LET c = CONCAT(b, b, b, b, b, b, b, b, b, b)
                    LET d = CONCAT(c, c, c, c, c, c, c, c, c, c)
                    RETURN LENGTH(d)
                ]])"#,
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("memory budget (4096 bytes)"),
            "{err}"
        );
        // Each query has its own allowance; the engine stays usable.
        assert_eq!(
            engine
                .execute(r#"return graph.query("FOR a IN [1,2,3] RETURN a")"#)
                .unwrap(),
            json!([1, 2, 3])
        );
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn cancellation_stops_the_worker_even_inside_protected_calls() {
    let control = LuaExecutionControl::default();
    let worker_control = control.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let task = tokio::task::spawn_blocking(move || {
        let engine = LuaEngine::with_control(worker_control).unwrap();
        engine.set_instruction_limit(u32::MAX);
        started.send(()).unwrap();
        engine.execute("for i=1,100000000 do pcall(function() local n=0; for j=1,100 do n=n+j end end) end")
            .map_err(|e| e.to_string())
    });
    ready.await.unwrap();
    control.cancel();
    let err = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(err.contains("cancelled"), "{err}");
}
