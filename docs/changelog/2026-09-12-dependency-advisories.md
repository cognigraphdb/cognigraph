# Resolve Native lru unsoundness and review optional ONNX maintenance

- Date: 2026-09-12
- Status: v2.7.6
- Kind: Dependencies and verification

## Changes

[CG-70](../issues/CG-70.md) backports Tantivy's one-line upstream lru requirement
fix to a checksummed 0.26.1 release snapshot. Both Native and the direct cache
now resolve lru 0.18.4. Tantivy source and storage formats are unchanged; the
snapshot includes upstream license/provenance and an offline integrity gate.
Docker builds include the snapshot and preserve its MIT notice.

CI now fails unsoundness advisories. The
[dependency decision](../decisions/decision_dependency_advisories.md) explicitly
retains the optional ONNX paste dependency under a dated maintenance review,
without suppressing its warning or claiming it fixed.

## Verification

The focused stored-document cache regression verifies hits, LRU eviction,
readback and reopen. Final suite results and the fresh all-feature/target
inventory are recorded in [CG-70](../issues/CG-70.md). No version bump,
publication, merge or deployment is part of this local change.
