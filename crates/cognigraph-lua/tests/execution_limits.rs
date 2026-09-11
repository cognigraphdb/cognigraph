use std::sync::Arc;
use std::time::{Duration, Instant};

use cognigraph_lua::{LuaEngine, LuaExecutionControl};
use cognigraph_native::NativeBackend;
use cognigraph_query::ExecutionBudget;
use serde_json::json;

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
            });
            let engine =
                LuaEngine::with_backend_control(backend, handle, allow_writes, false, control)
                    .unwrap();
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
