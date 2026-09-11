# Dedicated judge endpoint routing (CG-36)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:138-148` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Dedicated judge endpoint routing (CG-36).** Primary and partner judges now
  use the shared validated `OPENAI_BASE_URL`, retaining explicit OpenAI model
  selection and qualification rules. Nonempty dedicated model settings require
  a nonempty OpenAI key and valid selected endpoint before storage opens;
  blank/absent models retain fallback/absent-partner behavior. Formatting, strict
  Clippy, and 961 reported workspace tests passed; eight unconfigured Arango
  entries early-returned and two existing live embedding tests passed. Release
  verification passed 22 review scenarios and 12 startup rejections, with zero
  outbound judge attempts. See the
  [verification report](../issues/judge-endpoint-2026-09-09.md).
