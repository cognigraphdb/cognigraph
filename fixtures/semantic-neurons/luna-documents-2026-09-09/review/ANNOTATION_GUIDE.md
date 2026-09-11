# Independent review before truth or restraint claims

The source human annotations are a benchmark reference, not proof of exhaustive
truth. The frozen near-miss candidates are unreviewed. Do not convert an absent
source label into a negative verdict automatically.

Use two independent readers who can interpret factual English documents and the
twelve relation definitions. A third reader resolves disagreements. Record the
readers' actual identities or stable pseudonyms and roles in the new review
package. An LLM pass is not a substitute for independent human review.

1. Make separate copies of the development or holdout input JSON and the frozen
   taxonomy in `settings.json`. Each reviewer reads the **whole** document.
   Do not expose source gold, Luna nominations, skips, or scores during this pass.
2. Identify every explicit in-scope fact, its source and target names, exact
   contiguous evidence text, and direction. Record alternate mentions and
   coreference explicitly. Add all in-scope facts; do not restrict the review
   to the existing source entity list or the two retrieved candidate pairs.
3. Distinguish facts explicitly supported by a quote from document-level
   inference. Record uncertain or contradictory cases separately. A document
   that has no in-scope facts must be explicitly marked as fully reviewed.
4. Freeze both independently completed files before comparing them. Adjudicate
   disagreements and missing facts against the document and taxonomy. Preserve
   both original files and every adjudication decision.
5. After that reference is frozen, inspect the preselected near-miss candidates
   and benchmark-unmatched predictions against it. Record additional reference
   gaps as a new version; do not silently amend the reference to improve scores.

A completed record must contain a document ID and input-text digest, reviewer
ID, explicit completeness attestation, entities/aliases, directed facts and
evidence, uncertain cases, and notes on excluded inference. The holdout needs
the same reference policy as development. Review files remain a separate
versioned package; preserve this trial's source gold and recorded results.

Only after these records exist can the trial support independently adjudicated
precision, false-positive and abstention claims. Model candidate selection and
runtime changes must be frozen before any holdout inference. An untouched
holdout here means no model execution or tuning on it; its source annotations
are public and potential model-training exposure is unknown.
