# Contributing

## Before your first commit

Every commit needs a `Signed-off-by:` trailer (`git commit -s`). The trailer
means you accept the [Contributor License Agreement](CLA.md), which lets
Gedank Rayze keep CogniGraph under the [Functional Source License](LICENSE)
with its future Apache 2.0 grant and under the
[Enterprise License](LICENSE-COMMERCIAL). Read [LICENSING.md](LICENSING.md)
for which crates fall under which license.

Pull requests without sign-off on every commit are not merged.

## Where things live

- `crates/` — the Rust workspace. `cognigraph-governance`, `cognigraph-construct`
  and `cognigraph-artifacts` are Enterprise Components.
- `ui/` — the management console (Bun, React, Biome).
- `docs/` — engineering documentation. Product, sales and papers live in the
  sibling product repository.
- `fixtures/` — sealed evidence. Do not edit bytes; add a new dated package.
- `scripts/` — the checks CI runs.

## Engineering rules

The rules that apply to every change are in [AGENTS.md](AGENTS.md). The short
version:

- Every logical change gets a record in `docs/changelog/` and, when it
  changes a contract, a decision record in `docs/decisions/`. Regenerate the
  changelog index with `python3 scripts/check-docs.py --write-changelog-index`.
- Defects go through the [issue registry](docs/issues/README.md) with
  reproduction evidence.
- Claims about measured behavior cite their fixture, corpus, model and unit.
- Server modules stay within the modularity convention checked by
  `scripts/check-server-modularity.py`.

## Local gate before pushing

```sh
python3 scripts/verify.py --suite ci     # fmt, modularity, docs, decision index, clippy -D warnings, tests
python3 scripts/verify.py --suite ui     # bun install/check/test/build in ui/
```

`python3 scripts/install-hooks.py` installs the tracked pre-push hook that
runs the same gate. The push policy itself is in
[AGENTS.md](AGENTS.md#push-workflow).

## Reporting security issues

Do not open a public issue. Write to info@skitsanos.com with the details and,
if you have one, a reproduction. You will get an acknowledgement within three
working days.
