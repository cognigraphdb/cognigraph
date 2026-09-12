# Cli

Direct ArangoDB dump ingestion is a [deferred design](../plans/arangodump-import-design.md)
under CG-64/CG-66, not a prerequisite for Native-only readiness. Current
`cognigraph import` accepts CogniGraph JSON snapshots only.

The default CLI is Community. Ordinary database, auth and snapshot commands work
in both builds; neuron, construction, jobs, tenant and governance commands need
`cargo build --release -p cognigraph-cli --features enterprise` and an Enterprise
server. `--help` follows the compiled edition. Community recognizes Enterprise
command syntax but rejects execution locally with `enterprise_feature_required`.
The examples below include both editions. See [build editions](running.md#build-editions).


## Administration CLI

`cognigraph` (built from `crates/cognigraph-cli`, also inside the Docker
image) wraps the HTTP API for scripting and operators:

```sh
export COGNIGRAPH_URL=http://127.0.0.1:3000
export COGNIGRAPH_TOKEN=$(COGNIGRAPH_PASSWORD=... cognigraph login admin | jq -r .token)

cognigraph health
cognigraph export --out snapshot.json        # hot backup
cognigraph import snapshot.json              # restore
cognigraph query --bind min=3 'FOR d IN notes FILTER d.score >= @min RETURN d.title'
cognigraph query 'EXPLAIN FOR d IN notes RETURN d'    # plan + pushdown
cognigraph doc put notes '{"_key":"n1","title":"hello"}'
cognigraph embed notes chunks.jsonl          # server-side embed + atomic store
cognigraph user create alice viewer          # password: COGNIGRAPH_PASSWORD or stdin
cognigraph token issue alice --ttl-secs 7776000   # secret printed once
cognigraph token rotate alice TOKENKEY       # same key, fresh secret
cognigraph neuron pending                    # review queue; `neuron show KEY` = evidence inline
```

Connection via `--url`/`--token` flags or `COGNIGRAPH_URL`/`COGNIGRAPH_TOKEN`
env; user/token commands accept usernames or raw user keys; exit code 0 on
success, 1 on server errors, 2 on usage errors. `cognigraph --help` lists
everything. For the offline CG-33 commands, see the [reference-repair procedure](reference-repair.md).
