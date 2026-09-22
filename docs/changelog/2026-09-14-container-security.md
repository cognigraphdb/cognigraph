# Refresh container packages and gate fixable image vulnerabilities

- Date: 2026-09-14
- Status: Unreleased
- Kind: Security fix

## Changes

[CG-80](../issues/CG-80.md) upgrades packages already installed in the Debian
runtime, instead of refreshing only newly requested dependencies. Runtime layers
are rebuilt without cache while the Rust and console build caches remain usable.

Both edition images now pass an account-independent Trivy gate before packaged
runtime acceptance and publication. Every available fix is blocking at every
severity. Full reports remain visible, including unfixed vulnerabilities;
[CG-81](../issues/CG-81.md) tracks their further review. CI retains the reports.
Scanner failure, missing OS coverage and wrong-image reports cannot qualify.

The [verification record](../verification/container-security-2026-09-14.md)
compares the published image with both rebuilt editions and records the
remaining limitations. This change does not publish images or run hosted QA.

## Validation

Full local CI and both Linux/amd64 Docker/Helm suites pass, including 99 Python
regressions. The actual image scan rejects the published vulnerable baseline;
both rebuilt editions have no fixable findings. All 26 fixable IDs from the
original Docker Hub report are absent. Unfixed findings remain under CG-81.
