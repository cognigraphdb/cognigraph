# M12 — UPSERT + batch transactions (closing M7's deferrals)

- [x] CGQL UPSERT: `UPSERT search INSERT doc UPDATE merge IN collection [RETURN NEW/OLD]`. Search object with `_key` → direct lookup; otherwise first document (key order) matching all fields (numeric-aware). Found → partial UPDATE merge (OLD = prior); absent → INSERT expr as-is (OLD = null). ReadWrite mode only, like all mutations.
- [x] Batch transactions: `BatchOp` in core + `GraphBackend::execute_batch` (default: unsupported — capability pattern). Native: validate everything first, ONE redb transaction (RedbStore::apply already commits op-slices atomically), memory/cache/sidecar applied only after commit. All-or-nothing proven by a mid-batch-failure test. POST /batch endpoint (documents:write scope).
- [x] Corpus/validation cases, spec + design-doc/status updates, decision outcomes, gates, push.
