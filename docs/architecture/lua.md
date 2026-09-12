# Lua execution architecture

## Lua Query Engine

### Runtime Configuration

- **Engine:** mlua with LuaJIT. Construction verifies JIT is disabled and
  propagates sandbox/hook setup failures.
- **Sandboxing:** `os`, `io`, `debug`, `jit`, `load`, `loadstring`, `loadfile`,
  `dofile`, `require`, and `package` are removed; host scripts are text-only.
- **Resource limits:** A shared hook checks every 1,000 Lua instructions
  (default cap 1M), including coroutines. Limits reset per execution. Resource
  termination escapes `pcall`/`xpcall` through a typed host unwind. Both query
  permission modes apply the server's CGQL row budget, and all callbacks share
  one script deadline. Dropping the HTTP request signals cancellation; a
  supervisor joins the worker and invalidates the caller's result cache.
  Cancellation is cooperative at hooks/future yields: synchronous backend
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
| `graph.traverse(start_vertex, opts)` | Multi-hop traversal with scoring |
| `graph.neighbors(vertex_id, direction?, collection?)` | Get edges |
| `graph.upsert_edge(from, to, type, data?, collection?)` | Create/update edge |
| `graph.similarity(collection, vector, opts)` | Vector search |
| `graph.text_search(collection, query, fields?, limit?)` | BM25 text search when supported by the backend |
| `graph.batch(ops)` | Execute an atomic backend batch |

---
