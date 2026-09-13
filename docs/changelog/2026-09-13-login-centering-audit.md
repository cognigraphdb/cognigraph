# Local and hosted login centering audit

- Date: 2026-09-13
- Status: Unreleased
- Kind: UI verification

## Findings

The [login audit](../../ui/audit/2026-09-13-login-centering/audit.md) checks the
real local packaged console and Railway Community v2.7.7 at seven viewport sizes
each. Both center correctly at widths of 960 CSS pixels and above. Narrower
windows inherit the desktop workspace's minimum width, shifting the card right
and clipping it at 640 pixels. [CG-72](../issues/CG-72.md) records the reproduced
defect and remediation criteria. No application or deployment changes were made.
