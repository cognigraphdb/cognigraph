# Lua Scripting Examples

CogniGraph exposes a sandboxed Lua runtime with graph primitives via the
`POST /api/lua/execute` endpoint.

All examples below can be run directly:

```bash
curl -X POST http://localhost:3000/api/lua/execute \
  -H 'Content-Type: application/json' \
  -d '{"script": "<lua code here>"}'
```

## Backend Detection

The `graph.backend` field tells your script which backend is active. The
`graph.query_language` field reports the active backend language.
`graph.query()` is available only for parsed CGQL backends and is read-only by
default. With auth enabled, `documents:write` permits CGQL mutations and every
typed direct CRUD, edge, and batch primitive. All query text is parsed CGQL.
`script-runner` retains the
read-only scripting surface without an opaque-query side door. With auth
disabled, mutation paths remain disabled.

```lua
return {
    backend = graph.backend,
    query_language = graph.query_language
}
-- Returns: { backend = "native", query_language = "cgql" }.
```

All other `graph.*` functions expose document CRUD, edges, traversal, and
search operations:

- `graph.update_document(collection, key, merge)` — partial update (merge)
- `graph.replace_document(collection, key, doc)` — full swap
- `graph.text_search(collection, query, fields?, limit?)` — BM25 full-text
  (defaults: `{"title", "content"}`, limit 10)
- `graph.batch(ops)` — atomic multi-op writes, same op shape as
  `POST /api/batch`; on the native backend either every op applies or none
- Directions passed to `neighbors`/`traverse` must be exactly `outbound`,
  `inbound`, or `any` — typos are errors, not silent defaults
- Binding errors are prefixed with the operation name
  (`graph.text_search: …`) for debuggability
- `COGNIGRAPH_LUA_INSTRUCTION_LIMIT` overrides the per-script instruction
  budget (default 1,000,000; the sandbox disables the JIT so compiled loops
  cannot bypass the hook; accounting uses 1,000-instruction granularity)

---

## Document Operations

These examples use the configured graph backend.

### Create a document

```lua
return graph.create_document("documents", {
    title = "Introduction to Graph Theory",
    content = "Graph theory studies mathematical structures used to model pairwise relations.",
    category = "mathematics"
})
-- Returns: { collection = "documents", key = "12345", _id = "documents/12345" }
```

### Get a document by key

```lua
return graph.get_document("documents", "12345")
-- Returns the full document or nil if not found
```

### Find documents with pagination

```lua
return graph.find_documents("documents", {
    limit = 5,
    offset = 0
})
-- Returns an array of documents
```

### Delete a document

```lua
return graph.delete_document("documents", "12345")
-- Returns: true
```

---

## Edge / Relationship Operations

These examples use the configured graph backend.

### Create or update a relationship

```lua
-- Basic edge
graph.upsert_edge(
    "documents/100",
    "documents/200",
    "related_to"
)

-- With metadata and confidence
return graph.upsert_edge(
    "documents/100",
    "documents/200",
    "cites",
    { confidence = 0.92, source = "auto-extracted" }
)
-- Returns the full edge document
```

### Get neighbors (edges)

```lua
-- Outbound edges (default)
return graph.neighbors("documents/100")

-- Inbound edges
return graph.neighbors("documents/100", "inbound")

-- Both directions
return graph.neighbors("documents/100", "any")

-- Custom edge collection
return graph.neighbors("documents/100", "outbound", "custom_edges")
```

### Batch create relationships

```lua
local pairs = {
    { from = "documents/100", to = "documents/200", type = "related_to" },
    { from = "documents/100", to = "documents/300", type = "cites" },
    { from = "documents/200", to = "documents/300", type = "similar_to" },
}

local results = {}
for _, pair in ipairs(pairs) do
    local edge = graph.upsert_edge(pair.from, pair.to, pair.type, {
        confidence = 0.8,
        created_by = "lua_batch"
    })
    table.insert(results, edge._id)
end

return { created = #results, edge_ids = results }
```

---

## Graph Traversal

These examples work on **all backends**.

### Basic traversal

```lua
return graph.traverse("documents/100", {
    max_depth = 3,
    direction = "outbound"
})
-- Returns array of paths, each with vertices, edges, depth, and score
```

### Traversal with confidence filtering

```lua
return graph.traverse("documents/100", {
    max_depth = 2,
    min_depth = 1,
    direction = "any",
    min_confidence = 0.7,
    path_decay = 0.85,
    edge_collection = "document_relations"
})
```

### Find all documents within 2 hops

```lua
local paths = graph.traverse("documents/100", {
    max_depth = 2,
    direction = "outbound"
})

-- Collect unique document IDs from all paths
local seen = {}
local results = {}
for _, path in ipairs(paths) do
    for _, vertex in ipairs(path.vertices) do
        local id = vertex._id
        if not seen[id] then
            seen[id] = true
            table.insert(results, {
                id = id,
                title = vertex.title,
                depth = path.depth,
                score = path.score
            })
        end
    end
end

return results
```

---

## Vector Similarity Search

These examples work on **all backends**.

### Search by embedding vector

```lua
return graph.similarity("embeddings", {0.1, 0.2, 0.3, --[[ ... ]]}, {
    threshold = 0.5,
    limit = 10
})
```

### Find similar documents to a given one

```lua
local doc_key = "100"

-- Get the document and its embedding
local emb = graph.query([[
    FOR e IN embeddings
        FILTER e.document_id == CONCAT("documents/", @key)
        LIMIT 1
        RETURN e.embedding
]], { key = doc_key })

if not emb or #emb == 0 then
    return { error = "no embedding found for document " .. doc_key }
end

-- Search for similar (works on any backend)
local hits = graph.similarity("embeddings", emb[1], {
    threshold = 0.6,
    limit = 5
})

-- Fetch full documents for each hit
local results = {}
for _, hit in ipairs(hits) do
    local doc_id = hit.document.document_id
    if doc_id then
        local parts = {}
        for part in doc_id:gmatch("[^/]+") do
            table.insert(parts, part)
        end
        if #parts == 2 then
            local doc = graph.get_document(parts[1], parts[2])
            if doc then
                table.insert(results, {
                    title = doc.title,
                    score = hit.score,
                    document_id = doc_id
                })
            end
        end
    end
end

return results
```

---

## Parsed CGQL Queries

`graph.query()` parses CGQL with read/write permissions and execution limits.
Opaque query passthrough is unavailable, including to embedders.

### CGQL

```lua
return graph.query(
    "FOR d IN documents FILTER d.category == @cat SORT d.title ASC RETURN d",
    { cat = "mathematics" }
)
```

```lua
-- Grouping with aggregates and counts
return graph.query([[
    FOR d IN documents
    COLLECT category = d.category
    AGGREGATE avg_score = AVG(d.score)
    WITH COUNT INTO count
    SORT count DESC
    RETURN { category: category, avg_score: avg_score, count: count }
]], {})
```

```lua
-- LET bindings, functions, DISTINCT
return graph.query([[
    FOR d IN documents
    LET words = LENGTH(SPLIT(d.content, " "))
    FILTER words > 100
    RETURN DISTINCT d.category
]], {})
```

```lua
-- Mutations run only when the caller's role grants documents:write
-- (RBAC); read-only callers get a clear error.
return graph.query([[
    UPSERT { slug: @slug }
    INSERT { slug: @slug, hits: 1 }
    UPDATE { hits: 1 }
    IN pages
    RETURN NEW
]], { slug = "home" })
```

## Complex Workflows

These examples use Native graph primitives.

### Build a knowledge subgraph from a seed document

```lua
local seed = "documents/100"
local max_hops = 2

-- Traverse outbound to find related docs
local paths = graph.traverse(seed, {
    max_depth = max_hops,
    direction = "outbound",
    min_confidence = 0.5
})

-- Build a subgraph summary
local nodes = {}
local edges = {}
local seen_nodes = {}

for _, path in ipairs(paths) do
    for _, v in ipairs(path.vertices) do
        if not seen_nodes[v._id] then
            seen_nodes[v._id] = true
            table.insert(nodes, {
                id = v._id,
                title = v.title or v._key
            })
        end
    end
    for _, e in ipairs(path.edges) do
        table.insert(edges, {
            from = e._from,
            to = e._to,
            type = e.relation_type,
            confidence = e.confidence
        })
    end
end

return {
    seed = seed,
    node_count = #nodes,
    edge_count = #edges,
    nodes = nodes,
    edges = edges
}
```

### Document pipeline: create, link, and verify

```lua
-- Create two documents
local doc_a = graph.create_document("documents", {
    title = "Rust Programming",
    content = "Rust is a systems programming language focused on safety.",
    category = "programming"
})

local doc_b = graph.create_document("documents", {
    title = "Memory Safety",
    content = "Memory safety prevents bugs like buffer overflows and use-after-free.",
    category = "programming"
})

-- Link them
graph.upsert_edge(doc_a._id, doc_b._id, "related_to", {
    confidence = 0.95,
    source = "lua_script"
})

-- Verify the relationship exists
local edges = graph.neighbors(doc_a._id, "outbound")

return {
    doc_a = doc_a,
    doc_b = doc_b,
    relationships = #edges,
    backend = graph.backend
}
```

### Multi-hop context assembly for RAG

```lua
-- Given a seed document, collect context from nearby graph nodes
local seed_key = "100"
local seed = graph.get_document("documents", seed_key)
if not seed then
    return { error = "seed document not found" }
end

-- Traverse to find related content
local paths = graph.traverse("documents/" .. seed_key, {
    max_depth = 2,
    direction = "any",
    min_confidence = 0.6,
    path_decay = 0.8
})

-- Assemble context chunks sorted by score
local context = {}
local seen = {}

for _, path in ipairs(paths) do
    for _, v in ipairs(path.vertices) do
        if v._id and not seen[v._id] then
            seen[v._id] = true
            table.insert(context, {
                id = v._id,
                title = v.title or "",
                content = v.content or "",
                score = path.score
            })
        end
    end
end

-- Sort by score descending
table.sort(context, function(a, b) return a.score > b.score end)

-- Take top 5 context chunks
local top = {}
for i = 1, math.min(5, #context) do
    table.insert(top, context[i])
end

return {
    seed = { title = seed.title, key = seed_key },
    context_chunks = top,
    total_reachable = #context
}
```

---

## Sandbox Restrictions

The Lua runtime is sandboxed. The following are **not available**:

- `os` — no system calls
- `io` — no file I/O
- `debug` — no debug library
- `load`, `loadfile`, `dofile` — no dynamic code loading
- `package`, `require` — no module loading

Scripts are also subject to an instruction count limit (default: 1M instructions) to prevent infinite loops.

```lua
-- These will all fail:
os.execute("ls")           -- attempt to index global 'os' (a nil value)
io.open("/etc/passwd")     -- attempt to index global 'io' (a nil value)
require("socket")          -- attempt to call global 'require' (a nil value)
```

---

## Available Functions Reference

| Function | Description | Backend |
|----------|-------------|---------|
| `graph.backend` | Native backend identity (`"native"`) | All |
| `graph.query_language` | Query language (`"cgql"`) | All |
| `graph.query(query, bind_vars)` | Execute parsed CGQL; mutations require `documents:write`. | Native/CGQL |
| `graph.get_document(collection, key)` | Fetch a single document | All |
| `graph.find_documents(collection, opts)` | List documents with limit/offset | All |
| `graph.create_document(collection, doc)` | Create a document (`documents:write`) | All |
| `graph.update_document(collection, key, merge)` | Partially update a document (`documents:write`) | All |
| `graph.replace_document(collection, key, doc)` | Replace a document (`documents:write`) | All |
| `graph.delete_document(collection, key)` | Delete a document | All |
| `graph.neighbors(vertex_id, direction?, collection?)` | Get adjacent edges | All |
| `graph.traverse(start_vertex, opts)` | Multi-hop graph traversal | All |
| `graph.upsert_edge(from, to, type, data?, collection?)` | Create or update an edge | All |
| `graph.similarity(collection, vector, opts)` | Vector similarity search | All |
| `graph.text_search(collection, query, fields?, limit?)` | Backend BM25 text search | Capability-dependent |
| `graph.batch(ops)` | Atomic multi-operation write batch | Capability-dependent |
