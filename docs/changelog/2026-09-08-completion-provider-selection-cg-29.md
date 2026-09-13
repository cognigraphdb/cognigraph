# Completion-provider selection (CG-29)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:335-344` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Completion-provider selection (CG-29).** Server construction, side-views,
  and harness helpers share provider/key/model/base-URL resolution. Explicit
  overrides take precedence; separate side-view providers use their own model
  defaults. Invalid explicit providers, missing selected keys, and invalid
  selected URLs fail startup before stores open. No keys and no override keeps
  the optional lanes disabled. Formatting, Clippy, all 888 tests, 13 release
  HTTP configurations, 13 rejected startups, and 3 benchmark runs passed with
  synthetic loopback providers. See
  [verification and compatibility](../issues/completion-provider-2026-09-08.md).
