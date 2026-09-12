# CG-70 dependency inventory — 2026-09-12

Captured from the local CG-70 working tree based on `0d5a60a` (workspace 2.7.5).
This is new evidence; earlier captures remain unchanged. `${REPO}` replaces
the absolute checkout prefix in text trees. No package identities are rewritten.
The [candidate identity](candidate.json) records manifest, lockfile and snapshot
provenance hashes and the local Rust/Clippy/audit tool versions.

| Capture | Method and result |
|---|---|
| [Final audit](cargo-audit.json) | `cargo audit --deny unsound --json`: exit 0, zero vulnerability entries and no unsound warning; one unmaintained warning for paste 1.0.15. |
| [Pre-fix negative control](before-audit-control.json) | Audit of `git show 0d5a60a:Cargo.lock` in a temporary file, with `--deny unsound --no-fetch --json`: nonzero exit for RUSTSEC-2026-0253. Same advisory database as the final audit. |
| [lru paths](lru-tree.txt) | `cargo tree --locked --all-features --target all -i lru`: only 0.18.4, used by both Tantivy and the direct cache. |
| [paste paths](paste-tree.txt) | Same command with `-i paste`: tokenizers 0.23.1 directly and through macro_rules_attribute 0.2.2. |
| [Community packages](community-packages.txt) | `cargo tree --locked --target all -p cognigraph-server --prefix none --format '{p}'`: 362 distinct display entries; paste/tokenizers absent. |
| [Enterprise packages](enterprise-packages.txt) | Same server command with `--features enterprise`: 372 distinct entries; paste/tokenizers absent. |
| [All-feature packages](all-features-packages.txt) | `cargo tree --locked --target all --workspace --all-features --prefix none --format '{p}'`: 441 distinct entries, including optional ONNX. |

Package-list captures remove Cargo's repeated-node `(*)` marker, sort and
deduplicate display entries. They inventory target-specific dependencies without
claiming to compile every target. The audit scans 456 lockfile packages against
RustSec database commit `b50980aad8b8f14f77e25a97b32dd94bf008b0af` (1,243 advisories).
This dated result is not a promise about future advisories.

The [decision](../../../decisions/decision_dependency_advisories.md) owns the
Tantivy patch removal criteria and ONNX maintenance review triggers. The
[issue](../../CG-70.md) owns Rust, optional-feature and runtime verification.
