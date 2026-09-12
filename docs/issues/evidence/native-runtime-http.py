"""CG-67 Native runtime checks on disposable stores with a loopback embedding stub.

Run once per release edition. Captures contain assertions and hashes, never
passwords, bearer tokens or exported authentication records. Existing captures
are not overwritten. No backend selector or external database is configured.
"""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile
import threading
from http.server import ThreadingHTTPServer

REPO = Path(__file__).resolve().parents[3]
HELPER = REPO / "scripts/check-editions-live.py"
spec = importlib.util.spec_from_file_location("edition_probe", HELPER)
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)
probe.ENV = {key: os.environ[key] for key in ("PATH", "HOME", "TMPDIR") if key in os.environ}
MODES = ("memory", "resident-embedded", "resident-sidecar", "paged-sidecar")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(binary, edition, mode, directory, provider_url):
    config = {
        "COGNIGRAPH_EMBEDDING_PROVIDER": "openai", "OPENAI_API_KEY": "synthetic",
        "OPENAI_BASE_URL": provider_url, "COGNIGRAPH_QUERY_CACHE_ENABLED": "true",
        "COGNIGRAPH_CGQL_MAX_SOURCE_ROWS": "4",
    }
    if mode != "memory":
        storage, vector = mode.split("-")
        config.update(COGNIGRAPH_NATIVE_PATH=str(directory / "store.redb"),
                      COGNIGRAPH_STORAGE_MODE=storage, COGNIGRAPH_VECTOR_MODE=vector,
                      COGNIGRAPH_CACHE_BYTES="1024")
    events = []

    def expect(label, condition):
        assert condition, (mode, label)
        events.append({"assertion": label, "passed": True})

    with probe.server(binary, directory, config) as base:
        tokens = {"admin": probe.login(base)}

        def call(path, body=None, method=None, status=200, actor="admin"):
            value = probe.http(base, path, body, tokens.get(actor), method, status)
            events.append({"path": path, "method": method or ("POST" if body is not None else "GET"),
                           "actor": actor, "expected_status": status, "passed": True})
            return value

        health = call("/health", actor="anonymous")
        expect("Edition identity", health["edition"] == edition)
        call("/api/documents?collection=notes", status=401, actor="anonymous")
        for role in ("viewer", "script-runner"):
            call("/api/users", {"username": role, "password": probe.PASSWORD, "role": role})
            tokens[role] = probe.login(base, role)
        for key, vector in (("a", [1, 0]), ("b", [0, 1])):
            call("/api/documents", {"collection": "notes", "_key": key, "obsolete": True,
                 "title": key, "content": "anchor evidence" if key == "a" else "unrelated prose",
                 "embedding": vector})
        call("/api/documents/notes/a", {"title": "updated"}, "PATCH")
        replaced = call("/api/documents/notes/b", {"title": "replacement", "content": "unrelated prose",
                        "embedding": [0, 1]}, "PUT")
        expect("Replacement removes omitted user fields", "obsolete" not in replaced)
        expect("Viewer reads persisted update", call("/api/documents/notes/a", actor="viewer")["title"] == "updated")
        call("/api/documents/notes/a", {"title": "forbidden"}, "PATCH", 403, "viewer")
        call("/api/documents/_users/admin", status=403)

        query = 'FOR d IN notes SORT d._key RETURN d._key'
        expect("Parsed query rows", call("/api/search/query", {"query": query}, actor="viewer")["results"] == ["a", "b"])
        for language in ("aql", "unknown"):
            call("/api/search/query", {"query": "RETURN 1", "language": language}, status=403)
        denied = call("/api/search/query", {"query": 'INSERT { _key: "forbidden" } INTO notes'}, status=500)
        expect("Read endpoint rejects mutation before persistence", "mutations are not allowed" in denied["error"])
        call("/api/documents/notes/forbidden", status=404)
        call("/api/query", {"query": 'INSERT { _key: "q" } INTO work'})
        expect("Mutation persisted", call("/api/documents/work/q")["_key"] == "q")
        call("/api/query", {"query": 'INSERT {} INTO work'}, status=403, actor="viewer")
        expect("Lua language identity", call("/api/lua/execute", {"script":
               "return {graph.backend, graph.query_language}"})["result"] == ["native", "cgql"])
        expect("Read-only Lua query", call("/api/lua/execute", {"script":
               "return graph.query('FOR d IN notes SORT d._key RETURN d._key')"},
               actor="script-runner")["result"] == ["a", "b"])
        call("/api/lua/execute", {"script": "return graph.create_document('work', {_key='denied'})"},
             status=403, actor="script-runner")
        call("/api/lua/execute", {"script": "return graph.create_document('work', {_key='lua'})"})
        expect("Lua write persisted", call("/api/documents/work/lua")["_key"] == "lua")
        for path in ("/api/search/query", "/api/query"):
            exhausted = call(path, {"query": "FOR n IN [1,2,3,4,5] RETURN n"}, status=500)
            expect("HTTP source-row budget enforced", "source row budget (4 rows)" in exhausted["error"])
        exhausted = call("/api/lua/execute", {"script": "return graph.query('FOR n IN [1,2,3,4,5] RETURN n')"}, status=500)
        expect("Lua source-row budget enforced", "source row budget (4 rows)" in exhausted["error"])
        call("/api/batch", {"ops": [
            {"op": "insert", "collection": "notes", "doc": {"_key": "rollback"}},
            {"op": "insert", "collection": "notes", "doc": {"_key": "a"}},
        ]}, status=409)
        call("/api/documents/notes/rollback", status=404)

        call("/api/graph/relationships", {"collection": "links", "from": "notes/a", "to": "notes/b",
             "relation_type": "RELATES", "confidence": 0.9})
        paths = call("/api/graph/traverse", {"start_vertex": "notes/a", "edge_collection": "links",
                     "direction": "outbound", "min_depth": 1, "max_depth": 1, "path_decay": 0.8})
        expect("Traversal depth and confidence", paths["count"] == 1 and paths["results"][0]["depth"] == 1
               and math.isclose(paths["results"][0]["score"], 0.72, abs_tol=1e-12))
        expect("CGQL traversal", call("/api/search/query", {"query":
               'FOR v IN 1..1 OUTBOUND "notes/a" links RETURN v._key'})["results"] == ["b"])
        text = call("/api/search/text", {"collection": "notes", "query": "anchor", "fields": ["content"]})
        expect("Native BM25", text["count"] == 1 and text["results"][0]["document"]["_key"] == "a")
        vector = call("/api/search/vector", {"collection": "notes", "vector": [1, 0], "threshold": 0.5})
        expect("Native vector retrieval", vector["count"] == 1 and vector["results"][0]["document"]["_key"] == "a")
        hybrid = {"query": "anchor", "documents_collection": "notes", "embeddings_collection": "notes",
                  "search_fields": ["content"], "threshold": 0.5, "limit": 1}
        result = call("/api/search/hybrid", hybrid)
        expect("Native hybrid retrieval", result["count"] == 1 and result["results"][0]["document_id"] == "notes/a"
               and "bm25" not in result)
        expect("Hybrid cache reuse", call("/api/search/hybrid", hybrid)["cached"] is True)
        snapshot = call("/api/admin/export")
        expect("Snapshot retains vectors", snapshot["collections"]["notes"]["documents"]["a"]["embedding"] == [1, 0])
        call("/api/documents/notes/a", method="DELETE")
        call("/api/documents/notes/a", status=404)
        call("/api/admin/import", snapshot)
        tokens["admin"] = probe.login(base)
        expect("Snapshot restore", call("/api/documents/notes/a")["title"] == "updated")

    with probe.server(binary, directory, config) as base:
        token = probe.login(base)
        if mode == "memory":
            probe.http(base, "/api/documents/notes/a", token=token, status=404)
            expect("Memory store is ephemeral", not (directory / "store.redb").exists())
        else:
            rows = probe.http(base, "/api/search/query", {"query": query}, token)["results"]
            expect("Rows survive restart", rows == ["a", "b"])
            expect("Update survives restart", probe.http(base, "/api/documents/notes/a", token=token)["title"] == "updated")
            result = probe.http(base, "/api/search/vector", {"collection": "notes", "vector": [1, 0], "threshold": 0.5}, token)
            expect("Vectors survive restart", result["count"] == 1 and result["results"][0]["document"]["_key"] == "a")
            result = probe.http(base, "/api/search/query", {"query":
                   'FOR v IN 1..1 OUTBOUND "notes/a" links RETURN v._key'}, token)
            expect("Edges survive restart", result["results"] == ["b"])
    return {"mode": mode, "checks": len(events), "events": events}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--edition", required=True, choices=("community", "enterprise"))
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output already exists; choose a new capture path")
    binary = args.binary.resolve()
    fixture = ThreadingHTTPServer(("127.0.0.1", 0), probe.Embeddings)
    thread = threading.Thread(target=fixture.serve_forever, daemon=True)
    thread.start()
    report = {"issue": "CG-67", "edition": args.edition, "binary_sha256": digest(binary),
              "harness_sha256": digest(Path(__file__)), "helper_sha256": digest(HELPER),
              "base_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
              "provider": "deterministic loopback embedding stub; no hosted model calls", "runs": []}
    try:
        with tempfile.TemporaryDirectory(prefix="cg67-native-") as root:
            for mode in MODES:
                directory = Path(root) / mode
                directory.mkdir()
                result = verify(binary, args.edition, mode, directory, f"http://127.0.0.1:{fixture.server_port}/v1")
                report["runs"].append(result)
                print(args.edition, mode, result["checks"], "checks passed", flush=True)
    finally:
        fixture.shutdown()
        fixture.server_close()
        thread.join()
    with args.output.open("x") as output:
        output.write(json.dumps(report, indent=2) + "\n")
    print("PASS", sum(run["checks"] for run in report["runs"]), "checks;", args.output, flush=True)


if __name__ == "__main__":
    main()
