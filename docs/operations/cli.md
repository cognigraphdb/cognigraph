# Cli

`cognigraph import` accepts CogniGraph JSON snapshots. See the
[recovery guide](recovery.md) for restore behavior and the
[external migration guide](../reference/aql-to-cgql.md) for format differences.
`cognigraph import --from-arangodump` turns an ArangoDB dump into a new store
offline; see [ArangoDB dump import](#arangodb-dump-import).

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

`export --out FILE` writes the snapshot only after the whole response has been
received and parsed and is known to hold a `collections` object. The file is
created with mode 0600, because a snapshot contains password hashes and every
document, and it is published without replacing anything: if `FILE` already
exists the command fails and the existing backup is left as it was. Use a new
name per backup, for example a dated one. Any command fails with exit code 1
when a success response cannot be read or is not JSON, such as an HTML page
from a proxy ([decision](../decisions/decision_cli_export_safety.md)).

Connection via `--url`/`--token` flags or `COGNIGRAPH_URL`/`COGNIGRAPH_TOKEN`
env; user/token commands accept usernames or raw user keys; exit code 0 on
success, 1 on server errors, 2 on usage errors. `cognigraph --help` lists
everything. For the offline CG-33 commands, see the [reference-repair procedure](reference-repair.md).

## ArangoDB dump import

Offline, in both editions, with no server, URL or token:

```sh
arangodump --server.database shop --output-directory ./shop-dump   # on the ArangoDB side
cognigraph import --from-arangodump ./shop-dump --output ./shop.redb --dry-run --report check.json
cognigraph import --from-arangodump ./shop-dump --output ./shop.redb --report import.json
COGNIGRAPH_NATIVE_PATH=./shop.redb COGNIGRAPH_ADMIN_PASSWORD=... cognigraph-server
```

Run the dry run first: it validates everything and writes only the report.
The import then builds a new store and publishes it only if every collection
is accepted; it never writes into an existing store. The dump carries no
CogniGraph users, so the server creates its administrator from
`COGNIGRAPH_ADMIN_PASSWORD` on first start. Exit codes: 0 accepted or
published, 2 usage (including an existing `--output`), 3 rejected by the
contract, 4 I/O failure or a dump that changed during the run, 5 published
but the report could not be written. The [contract](../reference/arangodump-import.md)
lists supported dumps, error codes, limits and what is carried over.
