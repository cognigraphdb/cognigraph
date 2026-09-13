# Full UI code and backend-contract review

- Date: 2026-09-11
- Status: Unreleased
- Kind: Review

## Findings

Review all 13 console screen modules, shared components, helpers, styles and
build/CI integration against current backend contracts. Record CG-49–CG-63:
15 open issues, comprising four P1 and eleven P2. The P1 findings concern
production API origin, lossy document editing, misleading tenant deletion and
construction imports that replace earlier evidence through reused chunk IDs.
No application fixes or publication are included.

The [audit report](../evidence/ui-2026-09-11-full-review.md#artifact-2a08bd45b55e9bd0cb9c) contains the
capability map, browser/API evidence, reproduction boundaries and remaining
qualification work. Update the implementation plan and UI tracker to distinguish
implemented UI, verified journeys and backend workflows without a UI.

## Verification

The local UI suite passes Biome/TypeScript, 72 helper tests (161 assertions) and
production bundling. Browser checks use Rust-served production assets on
isolated Native Community/Enterprise instances with synthetic data, plus 60
role/edition API probes. A temporary forwarding proxy works around the confirmed
port-3001 defect; it does not resolve it. Evidence includes independent document
reads, a 201-neuron queue, 101 search matches, duplicate Lua execution, tenant
quarantine/recreation and deterministic construction replacement.

Bun dependency audit fails for one React Router advisory; the affected unstable
RSC APIs are not used by this static console, so exploitability here is not
established. Export file contents, external-provider flows, ArangoDB, exhaustive
accessibility and complete role/capability coverage remain unverified. Current
CI does not run the UI suite; CG-60 records that omission.

Documentation/navigation, decision-index and issue checks pass after recording
the findings. Disposable containers/volumes, proxy and temporary credentials
were removed. Existing data and earlier pending README/Docker work were preserved.
