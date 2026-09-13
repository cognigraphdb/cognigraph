# Governed server modularity (CG-26)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:90-98` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Governed server modularity (CG-26).** Split six server hotspots into
  compatible module entry points and coherent contracts, validation, lifecycle,
  recovery/storage, and test files. Added a CI size guard with seven explicit
  exceptions; 233 of 240 scoped files meet the soft cap. Code-motion checks,
  formatting, strict Clippy, all 961 reported workspace tests, release build,
  authenticated Native replay/restart checks, API/neuron harnesses, and the live
  Arango M26 rejection boundary passed. Public behavior is preserved. See the
  [verification report](../issues/server-modularity-2026-09-09.md).
