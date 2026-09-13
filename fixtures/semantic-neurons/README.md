# Semantic Neurons public test inputs

This directory contains small deterministic regression fixtures, generic authoring
templates and explicitly attributed public-text examples required by tests.
The [generalization fixture](generalization/README.md) retains its source and
adaptation licence. Synthetic forensics and pharma examples remain public.

Large labelled research packages, provider traces and stored judge verdicts live
in the private evidence repository. The [catalog](../../docs/evidence/README.md)
records original paths, bytes and hashes; [research guides](../../docs/research/README.md)
and dated public reading copies retain measured results and limitations.
These private runs cannot be replayed from this checkout alone.

Ordinary tests do not require private research. The explicit allowlist in
`scripts/policies/public-distribution.json` identifies public inputs by digest.
New research output belongs in an explicit private or ignored scratch location,
as described by the [evidence policy](../../docs/operations/evidence-policy.md).
