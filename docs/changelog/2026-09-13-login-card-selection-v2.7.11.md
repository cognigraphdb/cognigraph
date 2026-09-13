# Select the login form before measuring logout layout — v2.7.11

- Date: 2026-09-13
- Status: v2.7.11
- Kind: Browser regression reliability and publication preparation

## Changes

[CG-74](../issues/CG-74.md) fixes a selector race reproduced during the v2.7.10
release promotion. The geometry test now requires the exact login heading
inside the card, excluding the transient connection-status panel rendered
while logout revalidates the session. Production source, styles and geometry
tolerances are unchanged. Twenty repeated real-server journeys pass across
Community and Enterprise; the [browser report](../evidence/ui-2026-09-13-login-card-selection.md#artifact-e88d1680df2562dd58a3)
retains evidence and the remaining qualification boundary.

The candidate includes the [Docker Hub account setup record](2026-09-13-docker-hub-setup.md)
and corrects obsolete backend wording in the licensing component guide without
changing either operative license. Full local CI and both Linux/amd64 Docker
suites pass. The owner approved the corrected v2.7.11 promotion, Railway update
and publication of both editions. Remote qualification remains required. No
Docker images were published by the failed v2.7.10 promotion, and Railway
remains on v2.7.7 at preparation time.
