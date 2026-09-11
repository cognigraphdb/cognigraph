# Administrative relations are no longer drafted — precision 71.5% → 85.0%

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:724-742` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Administrative relations are no longer drafted — precision 71.5% → 85.0%.**
  The drafter proposed relations for regulatory scaffolding as readily as for
  pharmacology (`drug --REPORT_ADVERSE_REACTIONS_TO--> label manufacturer`,
  `Amnesteem --APPROVED_BY--> FDA`, `Warnings and Precautions --CONTAINS_INFO_ON-->
  …`). `finalize_draft` now drops any rule whose subject or object is an entity of
  an administrative type. The check keys on the ontology's **entity types**, not
  relation labels — measured on the pilot, endpoint type identifies these at 4.5%
  precision versus 19.4% for a relation-name pattern, and relation names are an
  unbounded invented vocabulary while types are the structured axis the drafter
  already commits to. Types match **exactly, never as substrings** (the drafter
  emits `anatomical site` and `parasite`; a substring match on "site" would
  silently destroy both), and `person` is excluded because it holds the patient
  populations `CONTRAINDICATED_IN` depends on. Rebuilt end to end on the same 100
  labels: **1575 facts (−10%), 85.0% evidence-supported (+13.5), 92.5%
  label-correct (+10.0)**, `boilerplate` failures **18 → 0** and `wrong_entity`
  15 → 8. It beat its own 83.4% prediction because removing administrative
  *entities* also took a class of entity confusion with them. `wrong_relation` did
  not move (22 → 21) and is now 70% of all remaining failures — the next lever.
