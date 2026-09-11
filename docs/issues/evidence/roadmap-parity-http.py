"""CG-23 capability examples, not a CRM performance benchmark.

Run with Python 3 after building the release server. Seeds the public query
corpus in disposable resident/paged Native stores; no external provider calls.
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
import urllib.request


REPO = Path(__file__).resolve().parents[3]
DATASET = REPO / "crates/cognigraph-query/tests/corpus/dataset.json"
CASES = [
    ("D1a endpoint equality", 'FOR e IN relationships FILTER e._from == "documents/a" SORT e._to RETURN e._to', ["documents/b", "documents/c"]),
    ("D1b correlated equality", 'FOR d IN documents LET es=(FOR e IN relationships FILTER e._from == d._id RETURN e._key) SORT d._key RETURN LENGTH(es)', [2, 1, 1, 0]),
    ("D2 one-variable traversal", 'FOR d IN documents FILTER d._key == "a" FOR v IN 1..1 OUTBOUND d._id relationships SORT v._key RETURN v._key', ["b", "c"]),
    ("D2 two-variable traversal", 'FOR d IN documents FILTER d._key == "a" FOR v,e IN 1..1 OUTBOUND d._id relationships SORT v._key RETURN [v._key,e._to]', [["b", "documents/b"], ["c", "documents/c"]]),
    ("D2 three-variable traversal", 'FOR d IN documents FILTER d._key == "a" FOR v,e,p IN 1..1 OUTBOUND d._id relationships SORT v._key RETURN [v._key,e._to,LENGTH(p.edges)]', [["b", "documents/b", 1], ["c", "documents/c", 1]]),
    ("D3 conditional and null helpers", 'FOR x IN [null,4] RETURN [x == null ? 0 : x,COALESCE(x,0),NOT_NULL(x,0)]', [[0, 0, 0], [4, 4, 4]]),
    ("D4 identifiers and DOCUMENT", 'FOR d IN documents FILTER d._key == "a" RETURN [PARSE_IDENTIFIER(d).collection,IS_SAME_COLLECTION("documents",d),DOCUMENT("documents/b").title]', [["documents", True, "Beta"]]),
    ("D5 shorthand and bare count", 'FOR d IN documents COLLECT WITH COUNT INTO n RETURN {n}', [{"n": 4}]),
    ("D5 expression subquery", 'FOR d IN documents FILTER LENGTH((FOR e IN relationships FILTER e._from == d._id RETURN 1)) > 1 RETURN d._key', ["a"]),
    ("D6 array helpers", 'FOR x IN [1] RETURN [SLICE([1,2,3],1,1),FLATTEN([[1],[2]]),SORTED(INTERSECTION([1,2],[2,3])),SORTED(MINUS([1,2],[2]))]', [[[2], [1, 2], [2], [1]]]),
    ("D7 date arithmetic", 'FOR x IN [1] RETURN DATE_DIFF("2026-01-01",DATE_ADD("2026-01-01",90,"days"),"days")', [90]),
    ("D8 casts and regex", 'FOR x IN [1] RETURN [TO_NUMBER("42"),TO_STRING(42),TO_BOOL(1),REGEX_TEST("abc","^a"),REGEX_REPLACE("abc","b","x"),TO_NUMBER("no")]', [[42, "42", True, True, "axc", None]]),
    ("D9 sorted arrays", 'FOR x IN [1] RETURN [SORTED(["b",2,null,false,"A"]),SORTED_UNIQUE([2,1,2.0])]', [[[None, False, 2, "A", "b"], [1, 2]]]),
    ("D10 D11 deferred nested lookup", 'FOR e IN relationships LET d=DOCUMENT(DOCUMENT(e._to)._id) SORT e._from,e._to LIMIT 2 RETURN d.title', ["Beta", "Γάμμα"]),
]


def verify(binary, root, mode):
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
        COGNIGRAPH_ADMIN_PASSWORD="synthetic-cg23-password",
        COGNIGRAPH_JWT_SECRET="synthetic-loopback-cg23-secret",
    )
    token = None

    def call(path, body=None):
        headers = {"Content-Type": "application/json"}
        if token:
            headers["Authorization"] = "Bearer " + token
        req = urllib.request.Request(f"http://127.0.0.1:{port}" + path,
            data=None if body is None else json.dumps(body).encode(), headers=headers)
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
            token = call("/api/auth/login", {"username": "admin", "password": "synthetic-cg23-password"})["token"]
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
            observations = []
            for name, query, expected in CASES:
                actual = call("/api/query", {"query": query})["results"]
                assert actual == expected, (mode, name, expected, actual)
                analysis = call("/api/query", {"query": "EXPLAIN ANALYZE " + query})["results"][0]
                assert analysis["stats"]["result_rows"] == len(expected), (name, analysis)
                observations.append({"case": name, "query": query, "expected": expected,
                    "actual": actual, "analysis_result_rows": analysis["stats"]["result_rows"]})
            query = CASES[2][1]
            lua = call("/api/lua/execute", {"script": "return graph.query([==[" + query + "]==])"})["result"]
            assert lua == CASES[2][2], lua
            return {"checks": len(CASES) * 2 + 1, "examples": observations, "lua_graph_query": lua}
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
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = Path(tempfile.mkdtemp(prefix="cognigraph-cg23-"))
    print(f"Temporary evidence: {root}", flush=True)
    evidence = {"issue": "CG-23", "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
        "binary_profile": "release", "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "dataset_sha256": hashlib.sha256(DATASET.read_bytes()).hexdigest(),
        "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "purpose": "Synthetic capability checks; no CRM or model benchmark", "provider": "none",
        "modes": {mode: verify(binary, root, mode) for mode in ("resident", "paged")}}
    args.output.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
    print(f"Passed {sum(v['checks'] for v in evidence['modes'].values())} HTTP/Lua checks", flush=True)


if __name__ == "__main__":
    main()
