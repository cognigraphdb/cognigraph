# Prepare v2.7.2 Native-only CI candidate

- Date: 2026-09-12
- Status: v2.7.2
- Kind: CI and dependency maintenance

## Change

The PR candidate includes the locally qualified [Native-only runtime](2026-09-12-native-runtime.md),
[readiness](2026-09-12-native-readiness.md) and
[alignment](2026-09-12-native-alignment.md) work. [CG-69](../issues/CG-69.md)
adds automatic PR/main verification, Native release acceptance, Helm rendering
and packaged backup checks, advisory scans, pinned Actions and an aggregate
required check. Docker image publication remains explicit manual-main only;
there is no deployment step.

Both local push validation and GitHub use the same CI and Docker suites. Native
failures retain per-edition build/runtime logs and a failure summary in an
ignored capture directory. Historical CG-67/CG-68 evidence is unchanged.

The lockfile advances `h2` from 0.4.15 to 0.4.19 to address
[RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258.html),
and replaces yanked `chacha20` 0.10.1 with 0.10.2. Transitive informational
advisories remain visible and are tracked in [CG-70](../issues/CG-70.md).

## Verification and publication scope

The [CI guide](../operations/ci.md) defines the checks and GitHub settings.
This version identifies an authorized PR candidate; it does not claim that
main, Docker Hub or a deployed environment has been updated. CG-69 records
executed local and remote validation as it completes. No tag or image publication
is requested. Deployment remains on hold.
