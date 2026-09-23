# v2.7.28 — Compatible dependency drift before integration

- Date: 2026-09-23
- Status: v2.7.28
- Kind: Dependencies

## Changes

The develop freshness gate refused the integration candidate for the
v2.7.21–v2.7.27 batch because two compatible Cargo resolutions had moved
since local qualification:

- `lru` 0.18.4 → 0.18.5 (the vendored Tantivy manifest patch is unchanged
  and `scripts/check-vendored.py` still passes).
- `icu_time_data` 2.3.0 → 2.3.1.

`cargo update` applied only these two identities; `allocator-api2` and
`equivalent` drop out of the lockfile as `lru` no longer pulls them. No
direct dependency requirement, feature or source changed.

The workspace version moves to 2.7.28.

## Validation

`scripts/dependency_freshness.py` passes (58 direct identities current or
reviewed). The push gate reruns the full CI and Docker suites on this
lockfile before the branch leaves the machine.
