# M19 signed governance and separation of duties

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1102-1123` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M19 signed governance and separation of duties.** The
  foundation replaces M18's single-Admin promotion authority with one
  externally pinned Ed25519 root public key and three distinct stable tenant
  principals bound to policy-author, policy-approver, and promoter users.
  Purpose-bound root-certified public-key
  registrations, prospective root-signed revocations, immutable signed policy
  revisions/approvals, `PromotionContext` v2 bindings, and signed
  promote/reject/rollback intents use strict domain-separated canonical JSON.
  Admin retains trust-registry bootstrap, inspection, reconciliation, and
  recovery but cannot author, approve, or decide a promotion. A separately
  bootstrapped HostAdmin owns tenant lifecycle only and gains no tenant data or
  governance scope. The API and CLI
  accept pre-signed JSON only; the server never accepts or stores a root or
  principal private key. M18 v1 history stays readable and diagnostic, with no
  new v1 evidence or promote/reject authority and only a signed rollback to
  exact prior chain evidence. Signed authority does not prove external
  corpus/graph/oracle claims, snapshot freshness, or custody. A selected head
  remains a singleton, selection-only control-plane pointer: no deployment,
  HA, quorum, or consumer switching is implied. Public-only authoring templates
  live under `fixtures/m19/`. Adversarial tests, the exact workspace Rust
  gates, and release-binary Native/live-Arango lifecycles passed on 2026-07-18;
  the M19 decision records the observed recovery and cleanup evidence.
