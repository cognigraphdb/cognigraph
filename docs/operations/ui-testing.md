# Console verification

The shared `ci` suite runs the frozen Bun install, Biome/TypeScript, Bun unit
tests and production build, the Rust/documentation gates, then Chromium browser
regressions against real Community and Enterprise binaries. CI runs on PRs to
main, main pushes and manual dispatch; these checks also run through the local
pre-push gate. [Continuous verification](ci.md) owns the complete workflow.

## Setup and commands

Install stable Rust with Clippy/rustfmt and current stable Bun. CI explicitly
installs Bun with `oven-sh/setup-bun@v2` (`latest`); the lockfile selects the
Playwright version and its matching Chromium. From the code root:

```sh
cd ui
bun install --frozen-lockfile
bun --bun x --no-install playwright install chromium
cd ..
python3 scripts/verify.py --suite ui
python3 scripts/verify.py --suite ui-browser
```

On Linux, install browser system libraries with
`bun --bun x --no-install playwright install --with-deps chromium` from `ui/`;
CI does this on `ubuntu-latest`. Missing Bun, browser binaries or system libraries
fail verification. They are not treated as skipped browser coverage.

Run `python3 scripts/verify.py --suite ci` for the complete shared gate. Running
only `--suite ui` is useful while editing, but does not include browser coverage.
The browser runner requires the current production `ui/dist` build. It builds
each Rust server with `cargo build --locked -p cognigraph-server`, Community
without default features and Enterprise with `--features enterprise`, copying
each executable before switching features. It reads Cargo's reported executable
path, including a configured `CARGO_TARGET_DIR`. These are development-profile
binaries serving production UI assets, not release-binary qualification.

## Deterministic browser scope

[Playwright configuration](../../ui/playwright.config.ts) runs Chromium with one
worker, no retries and failure on focused tests. Bun runs the Playwright CLI;
its ordinary unit runner discovers `src/` only. TypeScript and Biome also cover
the browser fixtures/configuration.

The [runner](../../scripts/ui_browser.py) creates a temporary working directory
and Native store per edition, a random loopback port and ephemeral bootstrap
credentials. Readiness verifies the actual authenticated edition. It uses an
environment allowlist, disables Bun dotenv loading for the browser process and
never starts inside the code checkout's dotenv directory. It stops its server
and removes its store on success or failure. It does not reuse a local API.

Seven cases run in both editions, with an eighth HostAdmin case in Enterprise:

- Fresh production-origin login, invalid-login recovery, off-page encoded-key
  deep links, keyboard navigation, Back/Forward and refresh.
- UI document creation, raw JSON type preservation on edit, fresh API readback,
  reload and cancelled/confirmed deletion.
- Viewer read access with disabled mutations, direct route denial, actual API
  write/admin denials, unchanged stored data and logout/reload.
- Edition-specific Review access/empty-catalog guidance and named collapsed
  navigation at 1067 × 667.
- Login centering from 1600 down to 320 CSS pixels, phone-sized fields, and the
  desktop workspace minimum width across login/logout.
- Short-screen scrolling through required-field and real invalid-login errors,
  plus a presentation-only long-server-name fixture that keeps the API local.
- CodeMirror edits submit Unicode CGQL and Lua scripts to the real server and
  render their returned values.
- Enterprise HostAdmin landing/reload and tenant-data denial in UI and API.

Browser page exceptions, unexpected HTTP/transport failures and requests to
other origins fail the test. Expected 401/403 and verified empty-catalog 404
behavior are scoped explicitly. Browser contexts are fresh per case. Synthetic
collection/user identifiers are isolated; fixture data is deleted with the store.

This suite makes no provider requests and does not run model benchmarks
or research holdouts. It does not cover every governance/job workflow or every
browser/OS. Manual scoped QA remains required for behavior outside these cases.
The [CG-72 report](../../ui/audit/2026-09-13-login-responsive/audit.md) adds local
WebKit touch/viewport emulation for login only; it is separate from CI's Chromium
suite and does not qualify the rest of the console for mobile devices.

The combined CI runner also overrides `COGNIGRAPH_LIVE_LLM=0` for existing Rust
control-loop tests. The two external embedding tests are marked ignored before
dotenv loading. Separate, explicitly authorized provider qualification uses
`cargo test -p cognigraph-embeddings --test live_providers -- --ignored`; missing
keys fail that opted-in command. Normal `cargo test --all` reports these tests as
ignored and makes no calls from them. The two construction live-loop cases
return early with `COGNIGRAPH_LIVE_LLM=0`; the shared CI runner sets this value.

## Results and failure qualification

Ignored `ui/test-results/{community,enterprise}/` holds Playwright JSON results,
screenshots, server logs and runtime hashes. CI uploads results for seven days,
including failure diagnostics. Traces/videos and saved authentication state are
disabled to avoid retaining credentials or bearer headers. Review artifacts
before copying selected sanitized evidence to a dated `ui/audit/` directory.

For isolated diagnosis, `python3 scripts/ui_browser.py --edition community`
runs one edition; report that as partial coverage. The gate always uses both.
`--dist /absolute/path/to/built-candidate` can test a deliberately broken copied
bundle without modifying the workspace assets. A browser failure propagates
through the runner and shared CI gate; later stages must not report success.

[CG-60](../issues/CG-60.md) owns the initial positive and negative qualification
evidence. [The push guide](push.md) owns publication gates and authorization.
