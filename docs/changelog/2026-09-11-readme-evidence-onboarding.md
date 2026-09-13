# README: evidence links and public source onboarding

- Date: 2026-09-11
- Status: Unreleased
- Kind: Documentation

## Changes

Align the root README with the evidence-linked multi-model database position.
Replace the fruit walkthrough with a small, explicitly synthetic forensics
case: an account links to a handset, and edge metadata references the extraction
report. The CGQL example returns the supporting text through `DOCUMENT()`.
Explain that Community stores application-supplied evidence references;
evidence-bound construction and signed governance are Enterprise workflows.

Add public clone instructions, explicit loopback demo configuration and a
readiness retry. Keep direct Rust startup in a collapsible section. Separate
available capabilities from planned replication and clustering, qualify Native
database storage versus external artifact recovery, and map the main Rust
crates. Replace sibling-checkout navigation with links usable from GitHub or a
standalone code clone. The separate website positioning document now records
public source availability while distinguishing it from prebuilt distribution.
Retain the `five-minute-start` heading anchor used by existing product docs.
Rebuild the sibling product website brief from the approved website positioning,
design principles and current engineering contracts. It defines purpose, scope
and completion criteria, and links to the owning documents instead of restoring
the discarded brief or duplicating detailed strategy.

## Verification

Executed all five README HTTP requests against the Community 2.7.0 release
server in a disposable Docker container with synthetic data and no model
provider. The image was the previously validated, unchanged Community build
`sha256:a9b6aab2658954332aa06d36cffa7cc3f7be6cc2412b496054477e112cb3c802`.
Only the image reference, container/data isolation and host port differed from
the documented Docker startup. The query matched the documented JSON result;
the opening CGQL block, a nonmatching case returning no rows, and `EXPLAIN`
also passed. The container and its disposable data were removed afterward.

Code documentation navigation, decision index and whitespace checks passed.
The missing sibling `product/website-brief.md` was recreated from current sources;
the combined code/product navigation scan passed with 367 documents and 1,478
local links checked against the replacement.
No Rust source, model policy or sealed research capture changed; no new
benchmark, holdout, remote CI or publication is claimed.
