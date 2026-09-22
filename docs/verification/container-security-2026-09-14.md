# Container vulnerability remediation — 2026-09-14

This is local verification of [CG-80](../issues/CG-80.md). The public images
remain v2.7.14 until a new release is authorized and qualified. No hosted QA was
run. Raw reports live privately under
`cognigraph-evidence/runs/2026-09-14-container-cves/`.

That package's `manifest.json` seals 32 artifacts. Its SHA-256 is
`145d369330a1772e7ebc65bea47a70b5c25a3aae387bc24cda70ad582fdc252f`.
Authorized reviewers can verify raw report bytes against that manifest; the
digest is not a signature or a claim of public reproducibility.

## Baseline and correction

Docker Hub's Community v2.7.14 digest
`sha256:9539d08db8a5441cd99ee1871c1b3f03c22305eb890a0d908cb4095fb77a6335`
has 68 Scout findings: 5 critical, 10 high, 6 medium, 44 low, 3 unspecified.
The UI's fixable filter lists 26 unique CVEs. All critical/high findings come
from the Debian base layer. A pulled-image inventory confirms the old versions.

Trivy 0.74.0 independently reports 251 binary-package/CVE pairs in that image,
28 with fixes, representing exactly those same 26 unique IDs. The extra pairs
come from libc6/libc-bin sharing glibc CVEs. Severity and remaining totals differ
between scanners; these are not interchangeable measurements.

Upgrading installed Debian stable packages during the build produces these
fixed versions in both local Linux/amd64 editions:

| Source package | Previous version | Patched version | Hub CVEs addressed |
|---|---|---|---:|
| glibc | 2.41-12+deb13u3 | 2.41-12+deb13u4 | 2 |
| perl | 5.40.1-6 | 5.40.1-6+deb13u1 | 13 |
| sqlite3 | 3.46.1-7+deb13u1 | 3.46.1-7+deb13u2 | 2 |
| pcre2 | 10.46-1~deb13u1 | 10.46-1~deb13u2 | 7 |
| gzip | 1.13-1 | 1.13-1+deb13u1 | 2 |

Debian's [glibc](https://security-tracker.debian.org/tracker/CVE-2026-5450)
and [Perl](https://security-tracker.debian.org/tracker/CVE-2026-8376) records
confirm the critical advisory fixes. Comparison by CVE ID confirms all 26 IDs
are absent from both rebuilt-image Trivy reports. Both have **zero fixable
findings and zero Trivy critical findings** at this checkpoint.

## Remaining findings

Each rebuilt image still has 223 Trivy binary-package/CVE pairs, representing
103 unique IDs: 51 high, 81 medium, 90 low, 1 unknown. None has an available
Debian fix in that database. The original Scout report contains 42 non-fixable
findings, including high-severity zlib CVE-2026-85091, which Trivy does not list.
Debian also marks that zlib package affected without a fixed version.

[CG-81](../issues/CG-81.md) retains these findings for package/reachability
review. There are no added ignore entries or suppression statements. The
[gate policy](../decisions/decision_container_vulnerability_gate.md) blocks
available remediation at every severity; it does not certify zero CVEs.
Exploitability was not tested. Cargo/Bun dependency scans remain separate from
the image scanner's OS package coverage.

## Validation

- Python scanner/workflow regression tests: 99 passed, including absent
  inventory, wrong-image identity, scanner failure, suppression overrides and
  fixable findings in either edition.
- Both local Linux/amd64 images built successfully and passed independent scans.
- Real containers in both editions passed auth, edition API, CLI, licenses,
  restart/CGQL persistence, console assets/deep links, UID/capability checks,
  root/symlink rejection, private volume provisioning and shutdown.
- Rendered Helm backup checks passed in both editions, including private
  backup permissions and denial to a different UID.
- The actual scanner rejects the pulled vulnerable baseline with 28 fixable
  package/CVE pairs; both newly rebuilt editions pass the same scan policy.
- Full shared local CI passed: formatting, strict Clippy and Rust tests in both
  editions (725 Community and 916 Enterprise tests; two opt-in tests ignored
  per edition), 161 UI tests, 15 Chromium journeys, 514 Native checks and 12
  startup rejections, workflow/docs checks and dependency advisory gates.
  The existing optional `paste` maintenance warning remains visible.
- The final shared Docker suite passed both builds, both vulnerability scans,
  packaged acceptance, volume/startup checks and both live Helm backups.
- Final documentation, decision index, issue registry, public-distribution and
  whitespace checks passed after the documentation updates.
- Session-owned image references were removed after verification. Existing
  containers, volumes and the shared builder cache were preserved.

These initial rebuilt images retain the workspace's v2.7.14 metadata for local
testing only. They have not overwritten immutable public tags. A new corrective
release must bump the version, repeat its release gates, publish both editions,
advance `latest` and verify the registry reports/pulled digests independently.
