# Decision: Bounded dependency patches and optional ONNX maintenance

Date: 2026-09-12 · Status: ACCEPTED

## Context

[CG-70](../issues/CG-70.md) found an unsound `lru` dependency in Native's Tantivy
tree and unmaintained `paste` in the optional ONNX tree. The owner requested
resolution before continuing the CI batch. These require different remedies:
one has a reviewed upstream fix; the other is a maintenance advisory with no
patched release. Neither finding establishes an application exploit.

Tantivy's only lru use in 0.26.1 is a stored-block cache keyed by `usize`, with
`get`/`put` operations. That inspected path does not meet the advisory's
panicking-key-destructor and `pop` conditions. We still remove the affected
package instead of relying on that current usage restriction.

## Decision (engineering owner: CogniGraph maintainers)

1. **Backport the upstream manifest fix to the published Tantivy 0.26.1 source.**
   [Upstream commit 5ca3933](https://github.com/quickwit-oss/tantivy/commit/5ca39332002c2c87fb5d2abc707cf527b3319d42)
   changes only `lru ^0.16.3` to `^0.18.2`. The root lockfile selects 0.18.4.
   Keep published source bytes, the MIT license, archive checksum and file hashes
   in the [bounded snapshot](../../vendor/README.md). CI verifies its integrity.
   Do not adopt unrelated unreleased Tantivy changes or relabel vulnerable lru
   source as a safe version. Retire the patch when a published, reviewed Tantivy
   upgrade provides fixed lru and passes the same Native and image checks.
2. **Retain paste 1.0.15 temporarily for optional ONNX compilation.** The pinned
   `tokenizers 0.23.1` uses it directly and through `macro_rules_attribute 0.2.2`.
   It is a build-time procedural macro; normal Community and Enterprise server
   dependency trees do not enable it. This limits exposure but does not establish
   that build-time code is risk-free. The
   [RustSec maintenance advisory](https://rustsec.org/advisories/RUSTSEC-2024-0436.html)
   stays visible. Published
   [macro_rules_attribute 0.2.3](https://crates.io/api/v1/crates/macro_rules_attribute/0.2.3/dependencies)
   has switched to `pastey`, but
   [tokenizers 0.23.2](https://crates.io/api/v1/crates/tokenizers/0.23.2/dependencies)
   still directly requires paste. Updating only the macro helper therefore does
   not remove the warning. Keep the locked optional chain for this correction;
   prefer a reviewed upstream tokenizer replacement over adding another source
   fork solely to eliminate a maintenance warning.
3. **Review the ONNX exception before any ONNX/tokenizer dependency change or
   first ONNX-enabled distribution, and by 2026-10-12 if it remains unchanged.**
   The maintainer doing that work owns the review. Check upstream replacement
   progress, new RustSec entries and all-feature/target dependency paths. Compile,
   lint and test the optional feature; a changed tokenizer/provider also needs
   deterministic tokenization and real inference with a local model. If a
   security advisory appears, resolve it or disable ONNX rather than extending
   this maintenance exception. No routine provider calls or research holdouts
   are authorized by it.
4. **Fail CI on unsoundness as well as vulnerability errors.** The shared audit
   command is `cargo audit --deny unsound`. No advisory ID is ignored and the
   scanner must be available. Unmaintained warnings remain visible and require
   a documented review; an audit pass is not an advisory-free claim.

## Outcome

The bounded patch and integrity gate are implemented. The
[CG-70 record](../issues/CG-70.md) owns final validation and the dated inventory.
This decision accepts a maintenance strategy for paste; it does not claim that
paste is maintained, replaced or absent from Cargo.lock. Commit, publication,
merge and deployment remain separate from local resolution.
