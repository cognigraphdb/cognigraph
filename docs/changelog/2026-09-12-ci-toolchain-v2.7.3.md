# Align local verification with current stable Rust

- Date: 2026-09-12
- Status: v2.7.3
- Kind: CI compatibility

## Change

The first [CG-69](../issues/CG-69.md) GitHub run used Rust 1.98 and rejected the
embedding cache's constant-size `chunks_exact` loop under strict Clippy. Local
2.7.2 checks had used the older installed stable 1.97. The cache decoder now
iterates fixed-size byte arrays, preserving its little-endian decoding and
existing remainder behavior without a conversion unwrap.

The [CI guide](../operations/ci.md) explicitly requires refreshing stable before
local qualification. Local stable is updated to 1.98.1 for the corrected
candidate. Strict warnings remain enforced; neither the lint nor the failing
aggregate is bypassed. Version 2.7.3 follows the per-push increment policy.
The initial failure and subsequent qualification remain separate records in
CG-69 and the PR checks. Deployment and image publication remain on hold.

## Verification

Both full local suites passed on Rust 1.98.1: strict Clippy/tests in both
editions, UI and browser checks, Native release acceptance and startup
rejections, and both Linux/AMD64 images with packaged runtime and Helm backups.
The installed pre-push hook repeats those gates on the committed candidate.
Remote qualification is tracked by the corrected PR check, separately from
the failed first run.
