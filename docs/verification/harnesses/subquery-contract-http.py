"""CG-28/CG-37 release verification of checked-in subquery examples.

Uses disposable authenticated Native resident/paged stores, the shared query
corpus, and no providers. --expect-collisions reproduces CG-37 on a saved old
binary; the default requires its corrected results. CG-38 remains open.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

REPO = Path(__file__).resolve().parents[3]
CORPUS = REPO / "crates/cognigraph-query/tests/corpus"
DATASET = CORPUS / "dataset.json"
REJECTED = {
    "parse_err/sq_postcollect_return.cgql": "after COLLECT cannot be lifted",
    "parse_err/sq_postcollect_sort.cgql": "after COLLECT cannot be lifted",
    "parse_err/sq_contains_mutation.cgql": "parse error",
    "validate_err/sq_into_inline.cgql": "subqueries are only allowed as LET values",
    "validate_err/sq_mutation_operand.cgql": "subqueries are only allowed as LET values",
    "validate_err/sq_mutation_document.cgql": "DOCUMENT() is not supported in mutation queries",
}
COLLISIONS = {"sq_nested_binding_collision", "sq_nested_correlated", "sq_nested_binds",
    "sq_nested_siblings", "sq_nested_document"}
MUTATIONS = {
    "parse_ok/sq_mutation_let.cgql": [["a", "b", "c", "d"]],
    "parse_ok/sq_mutation_for.cgql": ["a", "b", "c", "d"],
}


def bind_vars(query):
    return {k: v for line in query.splitlines() if line.strip().startswith("// binds:")
        for k, v in json.loads(line.strip().removeprefix("// binds:")).items()}


def lua_value(value):
    if value is None:
        return "nil"
    if isinstance(value, dict):
        return "{" + ",".join("[" + lua_value(k) + "]=" + lua_value(v) for k, v in value.items()) + "}"
    if isinstance(value, list):
        return "{" + ",".join(map(lua_value, value)) + "}"
    return json.dumps(value, ensure_ascii=False)


def verify(binary, root, mode, expect_collisions, expect_plan_errors_500):
    directory = root / mode
    directory.mkdir()
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    env = {k: os.environ[k] for k in ("PATH", "HOME", "TMPDIR") if k in os.environ}
    env.update(
        COGNIGRAPH_HOST="127.0.0.1", COGNIGRAPH_PORT=str(port),
        COGNIGRAPH_NATIVE_PATH=str(directory / "db.redb"),
        COGNIGRAPH_STORAGE_MODE=mode, COGNIGRAPH_VECTOR_MODE="sidecar",
        COGNIGRAPH_EMBEDDING_PROVIDER="none", COGNIGRAPH_AUTH_ENABLED="true",
        COGNIGRAPH_CGQL_MUTATIONS_ENABLED="true",
        COGNIGRAPH_ADMIN_PASSWORD="synthetic-cg28-password",
        COGNIGRAPH_JWT_SECRET="synthetic-loopback-cg28-secret",
    )
    token = None

    def call(path, body=None, method=None):
        headers = {"Content-Type": "application/json"}
        if token:
            headers["Authorization"] = "Bearer " + token
        req = urllib.request.Request(f"http://127.0.0.1:{port}" + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
        with urllib.request.urlopen(req, timeout=20) as response:
            return json.load(response)

    with (directory / "server.log").open("w") as log:
        process = subprocess.Popen([str(binary)], cwd=directory, env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if process.poll() is not None:
                    raise RuntimeError(f"server exited; see {directory / 'server.log'}")
                try:
                    call("/health")
                    break
                except OSError:
                    time.sleep(0.1)
            else:
                raise RuntimeError("server startup timeout")
            token = call("/api/auth/login", {"username": "admin", "password": "synthetic-cg28-password"})["token"]
            for name, collection in json.loads(DATASET.read_text())["collections"].items():
                call("/api/collections", {"name": name, "collection_type": collection["type"]})
                for document in collection["documents"].values():
                    if collection["type"] == "edge":
                        call("/api/graph/relationships", {"collection": name,
                            "from": document["_from"], "to": document["_to"],
                            "relation_type": document["relation_type"],
                            "confidence": document["confidence"]})
                    else:
                        call("/api/documents", {"collection": name, **document})
            events = []

            def invoke(surface, query):
                if surface == "lua":
                    response = call("/api/lua/execute", {"script":
                        "local ok,value=pcall(function() return graph.query([==[" + query +
                        "]==]," + lua_value(bind_vars(query)) +
                        ") end); if not ok then value=tostring(value) end; return {ok=ok,value=value}"})["result"]
                    return response["ok"], response["value"], 200
                try:
                    return True, call(surface, {"query": query, "bind_vars": bind_vars(query)})["results"], 200
                except urllib.error.HTTPError as error:
                    return False, json.loads(error.read()), error.code

            def rows(query):
                return call("/api/query", {"query": query, "bind_vars": bind_vars(query)})["results"]

            def reset_results():
                call("/api/collections/sq_results", method="DELETE")
                call("/api/collections", {"name": "sq_results"})

            def snapshot():
                return {name: rows(f"FOR d IN {name} SORT d._key RETURN d")
                    for name in ("documents", "sq_results")}

            call("/api/collections", {"name": "sq_results"})
            for path in sorted((CORPUS / "exec").glob("sq_*.cgql")):
                query = path.read_text()
                expected = json.loads(path.with_suffix(".json").read_text())
                for surface in ("/api/query", "/api/search/query", "lua"):
                    ok, actual, status = invoke(surface, query)
                    if expect_collisions and path.stem in COLLISIONS:
                        assert not ok and "is declared more than once" in json.dumps(actual), (path.name, actual)
                        assert status == {"lua": 200, "/api/query": 400, "/api/search/query": 500}[surface]
                        events.append({"kind": "baseline_collision", "case": path.stem,
                            "surface": surface, "status": status, "error": actual})
                        continue
                    assert ok and actual == expected, (path.name, surface, actual, expected)
                    events.append({"kind": "read", "case": path.stem, "surface": surface,
                        "status": status, "expected": expected, "actual": actual})
                if expect_collisions and path.stem in COLLISIONS:
                    ok, error, status = invoke("/api/query", "EXPLAIN ANALYZE\n" + query)
                    assert not ok and status == 400 and "is declared more than once" in json.dumps(error)
                    events.append({"kind": "baseline_analysis_collision", "case": path.stem,
                        "status": status, "error": error})
                    continue
                report = rows("EXPLAIN ANALYZE\n" + query)[0]
                assert report["stats"]["result_rows"] == len(expected), report
                events.append({"kind": "analysis", "case": path.stem,
                    "result_rows": report["stats"]["result_rows"]})

            for name, expected_error in REJECTED.items():
                query = (CORPUS / name).read_text()
                for surface in ("/api/query", "lua"):
                    before = snapshot()
                    ok, error, status = invoke(surface, query)
                    assert not ok and (surface == "lua" or status == 400), (name, surface, error)
                    assert expected_error in json.dumps(error), (name, error)
                    events.append({"kind": "rejected", "case": name, "surface": surface,
                        "status": status, "error": error})
                    after = snapshot()
                    assert after == before, (name, "unexpected write")
                    events.append({"kind": "unchanged", "case": name, "surface": surface,
                        "snapshot_sha256": hashlib.sha256(json.dumps(after, sort_keys=True).encode()).hexdigest()})

            for name, expected in MUTATIONS.items():
                query = (CORPUS / name).read_text()
                for surface in ("/api/query", "lua"):
                    reset_results()
                    ok, actual, status = invoke(surface, query)
                    assert ok and actual == expected, (name, surface, actual)
                    events.append({"kind": "mutation", "case": name, "surface": surface,
                        "expected": expected, "actual": actual})
                    stored = rows("FOR r IN sq_results SORT r._key RETURN " +
                        ("r.keys" if name.endswith("_let.cgql") else "r._key"))
                    assert stored == expected, (name, stored)
                    events.append({"kind": "persisted", "case": name, "surface": surface,
                        "actual": stored})
            # CG-38: default runs require 400 and identical diagnostics on both
            # routes. The explicit baseline option preserves the old defect.
            plan_errors = [("syntax", "RETURN", "parse error"),
                ("scope", "RETURN unknown", "identifier `unknown` is not in scope")]
            plan_errors += [(name, (CORPUS / name).read_text(), REJECTED[name]) for name in
                ("parse_err/sq_postcollect_return.cgql", "validate_err/sq_into_inline.cgql")]
            for name, query, detail in plan_errors:
                for prefix in ("", "EXPLAIN ", "EXPLAIN ANALYZE "):
                    responses = []
                    for surface in ("/api/query", "/api/search/query"):
                        before = snapshot()
                        ok, error, status = invoke(surface, prefix + query)
                        expected_status = 500 if expect_plan_errors_500 and surface == "/api/search/query" else 400
                        assert not ok and status == expected_status, (name, prefix, surface, status, error)
                        assert detail in error["error"], error
                        assert error["error"].startswith("Query execution error:" if expected_status == 500 else "Validation error:"), error
                        responses.append(error)
                        events.append({"kind": "client_plan_error", "case": name, "prefix": prefix,
                            "surface": surface, "desired_status": 400, "actual_status": status, "error": error})
                        after = snapshot()
                        assert after == before, (name, "unexpected write")
                        events.append({"kind": "plan_error_unchanged", "case": name, "prefix": prefix,
                            "surface": surface, "snapshot_sha256": hashlib.sha256(json.dumps(after, sort_keys=True).encode()).hexdigest()})
                    if not expect_plan_errors_500:
                        assert responses[0] == responses[1], responses

            for name, query, expected_status, detail in [
                ("forbidden", 'RETURN DOCUMENT("_users/cg38-missing")', 403, "system-reserved"),
                ("runtime", "FOR x IN 1 RETURN x", 500, "FOR source must be an array"),
            ]:
                for prefix in ("", "EXPLAIN ANALYZE "):
                    for surface in ("/api/query", "/api/search/query"):
                        ok, error, status = invoke(surface, prefix + query)
                        assert not ok and status == expected_status and detail in error["error"], (name, status, error)
                        events.append({"kind": "error_class_control", "case": name, "prefix": prefix,
                            "surface": surface, "status": status, "error": error})
            return {"checks": len(events), "events": events}
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=REPO / "target/release/cognigraph-server")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expect-collisions", action="store_true")
    parser.add_argument("--expect-plan-errors-500", action="store_true",
        help="Reproduce CG-38 with a pre-fix release; default runs require HTTP 400")
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = Path(tempfile.mkdtemp(prefix="cognigraph-cg28-"))
    print(f"Temporary evidence: {root}", flush=True)
    fixtures = [DATASET] + sorted(CORPUS.glob("*/sq_*"))
    assert COLLISIONS <= {p.stem for p in (CORPUS / "exec").glob("sq_*.cgql")}
    expect_plan_errors_500 = args.expect_plan_errors_500 or args.expect_collisions
    evidence = {"issues": ["CG-28", "CG-37", "CG-38"],
        "known_defects": ["CG-38"] if expect_plan_errors_500 else [],
        "expect_collisions": args.expect_collisions,
        "expect_plan_errors_500": expect_plan_errors_500,
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
        "binary_profile": "release", "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "fixtures": {str(p.relative_to(REPO)): hashlib.sha256(p.read_bytes()).hexdigest() for p in fixtures},
        "provider": "none", "modes": {mode: verify(binary, root, mode, args.expect_collisions, expect_plan_errors_500) for mode in ("resident", "paged")}}
    args.output.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
    print(f"Passed {sum(v['checks'] for v in evidence['modes'].values())} checks", flush=True)


if __name__ == "__main__":
    main()
