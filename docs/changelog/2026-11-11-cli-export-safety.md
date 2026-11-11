# v2.7.50 — CLI export safety

- Date: 2026-11-11
- Status: v2.7.50
- Kind: Data-integrity fix

## Changes

[CG-114](../issues/CG-114.md) is resolved under the
[CLI export decision](../decisions/decision_cli_export_safety.md).

The CLI treated any 2xx response as success, even when the body could not be
read or was not JSON. `cognigraph export --out FILE` then overwrote `FILE`
with the string `"<non-JSON response>"`, world-readable, and exited 0.

- A success response that cannot be read or parsed is now an error for every
  command (exit 1).
- `export` requires a snapshot with a `collections` object.
- `export --out` writes the file with mode 0600 through a synced staging
  file and never replaces an existing file.

**Breaking:** exporting to a path that already exists fails. Use a new name
per backup.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (845 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,072 passed, 2 ignored)
pass. The new end-to-end test runs the real binary against a stub server for
nine bad responses. The release CLI was also run against a real 2.7.50
server with authentication on: a 0600 snapshot is written, a second export
to the same path is refused, and a wrong URL or token exits 1 without
writing. The registry, docs and decision-index checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
