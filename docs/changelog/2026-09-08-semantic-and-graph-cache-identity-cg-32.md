# Semantic and graph cache identity (CG-32)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:345-354` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Semantic and graph cache identity (CG-32).** Versioned JSON cache
  parameters preserve optional model filters and complete edge/neuron
  collection names. An empty semantic model filter keeps its literal meaning
  and cannot reuse an omitted filter's answer. Graph results and trace metadata
  stay within the requested traversal's parameter identity. Formatting,
  Clippy, all 876 tests, OpenAPI checks, and release HTTP regressions passed
  across resident/paged storage and memory/persistent caches, including
  restarts and valid assisted reuse. No cache migration is required. See
  [verification](../issues/search-cache-2026-09-08.md).
