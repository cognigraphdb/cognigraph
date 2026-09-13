# Login card selection regression — 2026-09-13

[CG-74](../../../docs/issues/CG-74.md) records a reproduced CI timing defect in
v2.7.10 promotion [PR #12](https://github.com/cognigraphdb/cognigraph/pull/12).
The [failed run](https://github.com/cognigraphdb/cognigraph/actions/runs/34751310005)
passed both Rust editions and 514 Native checks, then failed the Community
logout layout test while reading `boundingBox()`. The remaining Community case,
Enterprise browser cases and Docker checks were not executed in that run.

## Cause and change

`ConnectionGate` and `LoginScreen` both use `.auth-card`. The v2.7.10 visibility
wait could see the connection panel while logout revalidated the session. React
could replace that element before geometry was read. The captured final
[login screenshot](ci-failure-final-login.png) shows the actual form centered in
the 390 × 844 viewport; the failure is not evidence of a broken login layout.

The locator now requires the exact `CogniGraph Console` heading inside the card.
It waits for the intended form. Production code, styles, geometry tolerance,
viewport matrix and test failure policy are unchanged.

## Local evidence

The repeated and standalone full browser runs used base commit `7238514`
(v2.7.10) plus the selector repair. [Source fingerprints](source.json) identify
the repaired test and production UI inputs. The subsequent versioned candidate
is v2.7.11; its final release gates are recorded separately below.

The [repeated browser check](repeated-browser-checks.txt) runs the affected
real-server journey ten times per edition: all twenty executions passed. Each
execution includes eight desktop/phone sizes, a real Admin login, preservation
of the desktop workspace minimum width, logout and phone-size centering.
Chromium runs on macOS against the production UI served by disposable Native
Community/Enterprise servers. Provider calls are disabled. The runner removes
its own stores and processes. No production data or real operator credentials
are used by these local tests.

Biome and TypeScript checks pass. The [full browser suite](full-browser-checks.txt)
passes all seven Community and eight Enterprise cases. The subsequent full versioned local CI and Docker suites pass as recorded
below. Revised release approval and remote qualification remain pending. This is browser emulation,
not physical-phone or software-keyboard acceptance. The requested image release
is paused pending corrected candidate qualification; Railway remains v2.7.7.

## Versioned candidate qualification

The [qualification receipt](qualification.json) records the v2.7.11 working-tree
candidate. Full local CI passes: both strict Rust editions, workflow/docs/issue
checks, UI checks, 514 Native release checks, twelve startup rejection cases and
all fifteen browser cases. Both Linux/amd64 images pass packaged API/CLI,
authentication, license, restart, console/startup and Helm backup checks.
The existing CG-70 maintenance-advisory boundary remains unchanged. No provider
or holdout execution is claimed.

The initial versioned invocation caught the not-yet-refreshed Cargo.lock;
resolving the local workspace versions fixed that preparation mismatch, and the
fresh full run passed. During the rebuild, only this checkout's regenerable
incremental cache was removed between active checks to free disk space; the
verification driver resumed without skipping a gate. Database volumes and user
data were not changed.

No corrected commit, remote branch or image has been published at this
checkpoint. The v2.7.10 promotion remains blocked; the owner was asked to approve
v2.7.11 because changed source cannot reuse an already-pushed product version.

## Release authorization

The owner subsequently approved v2.7.11 promotion, the Railway update and both
Docker edition publications, with cleanup of unused test containers and images.
The local measurements above are preserved. Remote qualification and publication
remain separate from those measurements.
