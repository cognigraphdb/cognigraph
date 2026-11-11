# Lua execution architecture

## Lua Query Engine

### Runtime Configuration

- **Engine:** mlua with LuaJIT. Construction verifies JIT is disabled and
  propagates sandbox/hook setup failures.
- **Sandboxing:** the Lua state loads only `table`, `string`, `math`, `bit`
  and LuaJIT's base library, which includes `coroutine`. `io`, `os`,
  `package`, `debug` and `ffi` are never loaded, so `require` and `module`
  do not exist. `jit` is loaded only to switch compilation off and is then
  removed from the globals and from the registry. `load`, `loadstring`,
  `loadfile` and `dofile` are removed; host scripts are text-only. Engine
  construction fails if any of these is still reachable
  ([decision](../decisions/decision_lua_sandbox_libraries.md)).
- **Resource limits:** A shared hook checks every 1,000 Lua instructions
  (default cap 1M), including coroutines. Limits reset per execution. Resource
  termination escapes `pcall`/`xpcall` through a typed host unwind. Both query
  permission modes apply the server's CGQL row budget, and all callbacks share
  one script deadline. Dropping the HTTP request signals cancellation; a
  supervisor joins the worker and invalidates the caller's result cache.
  Cancellation is cooperative at hooks/future yields, and a running
  `graph.query()` reads it at its deadline checks; other synchronous backend
  operations must return, and already committed writes are not rolled back.
- **Authorization:** read primitives require `lua:execute`; typed CRUD, edge,
  and batch mutations additionally require `documents:write`. `graph.query()`
  always executes parsed CGQL; opaque passthrough is unavailable. With auth disabled, Lua graph writes remain disabled.

### Exposed Primitives

| Function | Description |
|---|---|
| `graph.query(query, bind_vars)` | Parsed CGQL query; mutations require the write scope |
| `graph.get_document(collection, key)` | Fetch single document |
| `graph.find_documents(collection, opts)` | List with limit/offset |
| `graph.create_document(collection, doc)` | Create document |
| `graph.update_document(collection, key, merge)` | Partially update document |
| `graph.replace_document(collection, key, doc)` | Replace document |
| `graph.delete_document(collection, key)` | Delete document |
| `graph.traverse(start_vertex, opts)` | Multi-hop traversal with scoring; `max_depth` at most 10, and at most 10,000 expanded paths |
| `graph.neighbors(vertex_id, direction?, collection?)` | Get edges |
| `graph.upsert_edge(from, to, type, data?, collection?)` | Create/update edge |
| `graph.similarity(collection, vector, opts)` | Vector search |
| `graph.text_search(collection, query, fields?, limit?)` | BM25 text search when supported by the backend |
| `graph.batch(ops)` | Execute an atomic backend batch |

---
