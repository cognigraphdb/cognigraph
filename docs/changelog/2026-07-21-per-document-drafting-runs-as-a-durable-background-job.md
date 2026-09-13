# Per-document drafting runs as a durable background job

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:776-789` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Per-document drafting runs as a durable background job.** Drafting makes two
  LLM completions per source document, so a large corpus exceeded the request
  timeout (10 documents → HTTP 408). `JobKind::ConstructDraft` runs it in the
  M16/M17 `JobManager`, batched and checkpointed like `Ingest`, with every read and
  write through `TenantScoped` on the trusted handle. **The accumulator is the
  stored draft** (`space_type_drafts/{space}`): each pass merges its documents in
  and writes the artifact back, so a restart resumes from the partial draft instead
  of re-spending LLM calls (merging is idempotent, so a crash between artifact write
  and checkpoint only re-drafts that batch). Intermediate drafts carry
  `status: "drafting"` — only the final pass flips to `"draft"`, so a half-drafted
  ontology is structurally unacceptable to `draft_accept`; the job never writes
  `space_types` and never auto-accepts. `POST /api/construct/draft` gains an
  `async` flag (Idempotency-Key required) returning the job submission envelope.
