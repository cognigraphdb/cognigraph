# Public evidence boundary and hosted QA acceptance — 2026-09-13

This local acceptance resolves [CG-75](../issues/CG-75.md),
[CG-76](../issues/CG-76.md) and [CG-77](../issues/CG-77.md).
The code changes are unreleased; no code push, historical rewrite, new image
publication or new database deployment is claimed here.

## Preserved evidence and public checkout

The private archive preserves 1,899 original files with checked SHA-256 and byte
length, including uncommitted release receipts. Two differing files from the
published v2.7.11 tree are preserved separately. The public catalog identifies
1,867 private artifacts; 25 small JSON/JSONL verification inputs remain public,
along with their attribution, public harnesses, contracts and research summaries.
Original provider exchanges and captured prompt bodies are outside the public
candidate. Enterprise source remains public, including its prompt constants.

The v2.7.11 tree has 111,770,686 uncompressed file bytes. The candidate measured
before this acceptance summary has 15,009,022 bytes. These are current file-tree
sizes, not Git transfer or full-history clone sizes. Historical public blobs and
previously distributed copies have not been removed by this work.

## Executed qualification

A detached public candidate was prepared without environment files or a private
evidence sibling. It shared only the existing compiler target cache. The
candidate file manifest identifies the uncommitted input bytes; the base commit
and v2.7.11 test image labels do not identify a new publishable release.

- Shared CI passed: Rust formatting, strict Clippy and tests in both editions;
  80 Python script tests; documentation, decision, issue and distribution checks;
  edition/modularity checks, actionlint, Helm and dependency advisories.
- UI verification passed: Biome, TypeScript, 161 Bun tests, production build and
  all 15 real-server browser journeys across Community and Enterprise.
- Native release binaries passed 514 HTTP/runtime checks and 12 startup rejection
  cases. The relocated public harnesses ran without private inputs.
- Both Linux/amd64 images built and passed auth, edition, CLI, licence, CGQL,
  persistence, container hardening, console assets and live Helm backup checks.
- The changed DailyMed example ran on three fabricated documents, wrote to
  `data/dailymed/` and did not create a tracked fixture output directory. This is
  an output-location smoke check, not a new scientific DailyMed measurement.

An initial isolated run failed because the fixture allowlist included ignored
Python caches. The corrected policy lists only public JSON/JSONL inputs and has
a regression test for that failure. The failed attempt is retained privately.
The advisory gate retains its existing documented `paste` maintenance warning.
Provider integrations, model benchmarks and research holdouts were not executed;
they are outside this public CI suite. No private test dependency was silently
substituted or counted as executed coverage.

## Hosted lifecycle

Railway has no active database deployment or database source trigger. The same
attached `/data` volume is retained; the former health endpoint returns HTTP 404.
The unused Railway connector, its staged variables, Cloudflare tunnel, Access
application and policy were removed. Existing unrelated resources were preserved.
The website's active deployment is unchanged and its homepage loaded in the
browser; a non-browser HTTP probe returned 403 and is not counted as availability.

The [operating guide](../operations/railway.md) specifies deliberate qualification,
startup, authenticated synthetic checks and shutdown. Applications deploy their
own instances. This lifecycle change did not reset the store or perform a new
restore drill. Both owned CI image tags were removed after testing; original
containers and persistent volumes were preserved.

## Private evidence identities

Paths below are relative to the private `cognigraphdb/cognigraph-evidence`
repository. Digests and sizes were checked against the local originals. Access
is required to inspect raw evidence; these hashes establish byte identity, not
a signature, timestamp or independent attestation.

| Private path | Bytes | SHA-256 |
|---|---:|---|
| `archives/2026-09-13-public-boundary/manifest.json` | 408804 | `a50b675ee56144d43b81732b14a8f2db4b7cf37a04d78b09f9b5b2789b1f5de3` |
| `archives/2026-09-13-public-boundary/published-variants.json` | 446 | `584143f8cf712b232a36f208a267dc9ab1e77d99c88ebfedfeb708b7a8d3057d` |
| `runs/2026-09-13-qa-lifecycle/manifest.json` | 1350 | `55321df13471806f2c4a06cd7e1206975b6a1eca338587b00649206973133575` |
| `runs/2026-09-13-public-checkout/ci.log` | 191081 | `723db3e4474549f82145cd3fc94f543a6b081f828afbb643ee8fb73b52573c26` |
| `runs/2026-09-13-public-checkout/docker.log` | 101552 | `f2a9c1a68d566cc32c6da7d5c7b87861c09fca996c79fe54686614eda2c310fc` |
| `runs/2026-09-13-public-checkout/candidate-files.json` | 261725 | `bf3b9644a8c1ffcaf43686840d73716a946780739d314ceac6a420e212e18dc9` |
| `runs/2026-09-13-public-checkout/synthetic-example.json` | 446 | `8ac968344a707183a41d153e87b4cdcb327f92b0f8302d6dc6c046fd0f01e1a2` |
| `runs/2026-09-13-public-checkout/docker-cleanup.json` | 380 | `ef3bdbcda1a731cd145e1cd011f20863b2eb0ffda454769f006e77d53b2504a5` |
