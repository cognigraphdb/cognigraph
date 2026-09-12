# Packaged console qualification — 2026-09-12

Scope: [CG-71](../../../docs/issues/CG-71.md), bundling the existing console in
the Community and Enterprise images. No UI layout or interaction code changes.

| Field | Evidence |
|---|---|
| Build | v2.7.6 working candidate over CG-70 commit `529b1c4`; Dockerfile and entrypoint changes; [image identities](images.json) |
| Runtime | Actual Linux/AMD64 release containers on Docker Desktop; Native, authentication enabled, hosted providers disabled |
| Browser | Playwright Chromium, CSS viewport 1280 × 800; isolated random loopback origins; standard browser scaling |
| Actors | Community/Enterprise Admin and Viewer; Enterprise HostAdmin; disposable synthetic records |
| Result | [Executed checks](checks.txt): 4 Community and 5 Enterprise cases passed |
| Cleanup | Both containers and their test volumes removed by the runner |

PASS: fresh-origin login and intentional wrong-password recovery; server-enforced
Viewer mutation/admin denial; edition/HostAdmin route boundaries; accessible
collapsed navigation; direct collection URLs and encoded keys; query/Lua history;
document creation, raw structured JSON preservation, cancellation and confirmed
deletion. Mutation readback and hard reload verify actual persistence. The
diagnostic fixture rejects unexpected page errors, failed requests, HTTP errors
and external origins; none occurred. Expected authentication/authorization errors
are explicitly scoped by the tests.

The container harness additionally verified served HTML and JS/CSS assets, direct
routes, root-owned mount provisioning, private directory permissions, unprivileged
PID 1, zero effective capabilities, no-new-privileges, database restart/readback,
clean SIGTERM and rejection of invalid root settings and a symlink target.

Representative final states: [Community](community-collections.png) and
[Enterprise](enterprise-collections.png). These are local container evidence.
They do not establish live Railway acceptance, a new responsive-design audit,
mobile/Windows rendering coverage, model execution, or complete UI/backend parity.
The final committed candidate is requalified by pre-push and GitHub CI; hosted
verification belongs to CG-71's separate deployment checkpoint.
