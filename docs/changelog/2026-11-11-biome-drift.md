# v2.7.58 — Biome 2.6.7 and @babel/runtime drift before CG-197 and CG-198 are published

- Date: 2026-11-11
- Status: v2.7.58
- Kind: Dependencies

## Changes

The freshness gate refused the v2.7.56–v2.7.57 candidate because Biome
2.6.7 was published after the last push the same day. Biome is a direct
development dependency of the console UI and the TypeScript client; it
formats and lints, and is not part of any shipped artifact.

- `ui` and `clients/typescript`: `@biomejs/biome` `^2.6.6` → `^2.6.7`, the
  schema reference in both `biome.json` files, and the thirteen Biome
  platform packages in each lockfile.

- `ui`: `@babel/runtime` 7.29.13 → 7.29.14, and 8.1.0 → 8.1.2 where three
  `@rc-component` packages resolve their own copy (transitive, part of the
  console bundle).

Nothing else changed in either lockfile, and the Cargo resolution is
unchanged. The workspace version moves to 2.7.58.

## Validation

`biome check` and `tsc --noEmit` pass in both projects with no fixes
applied. `scripts/dependency_freshness.py` passes on these lockfiles. The
push gate runs the full CI suite, which builds the console and runs its unit
and browser tests on the new `@babel/runtime`, and the Docker suite on this
candidate before the branch leaves the machine.
