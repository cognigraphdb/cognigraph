//! Projection capacity.

use super::*;

#[test]
fn projection_capacity_accepts_exact_chunk_and_total_row_limits_and_rejects_one_over() {
    let exact_chunks = ProjectionCapacityCounts {
        entities: 0,
        chunks: MAX_M26_CHUNKS,
        mentions: 0,
        fact_occurrences: 0,
        semantic_facts: 0,
    };
    validate_projection_capacity_counts(exact_chunks).unwrap();
    assert!(matches!(
        validate_projection_capacity_counts(ProjectionCapacityCounts {
            chunks: MAX_M26_CHUNKS + 1,
            ..exact_chunks
        }),
        Err(CogniGraphError::DocumentConflict(_))
    ));

    let exact_total_rows = ProjectionCapacityCounts {
        entities: MAX_M26_TOTAL_ROWS - MAX_M26_CHUNKS - MAX_M26_MENTIONS - MAX_M26_SEMANTIC_FACTS,
        chunks: MAX_M26_CHUNKS,
        mentions: MAX_M26_MENTIONS,
        fact_occurrences: MAX_M26_SEMANTIC_FACTS,
        semantic_facts: MAX_M26_SEMANTIC_FACTS,
    };
    validate_projection_capacity_counts(exact_total_rows).unwrap();
    assert!(matches!(
        validate_projection_capacity_counts(ProjectionCapacityCounts {
            fact_occurrences: exact_total_rows.fact_occurrences + 1,
            ..exact_total_rows
        }),
        Err(CogniGraphError::DocumentConflict(_))
    ));
}
#[test]
fn impact_is_an_exact_sorted_candidate_baseline_set_difference() {
    let shared = fact("A", "REL", "B", "c1");
    let added = fact("A", "NEW", "C", "c2");
    let removed = fact("D", "OLD", "E", "c3");
    let candidate = vec![added.clone(), shared.clone()];
    let baseline = vec![shared, removed.clone()];
    let projection_digest = digest("projection");
    let impact =
        VerifiedSemanticRepairImpact::new(&candidate, &baseline, &projection_digest).unwrap();
    assert_eq!(impact.added_facts, vec![added]);
    assert_eq!(impact.removed_facts, vec![removed]);
    assert_eq!(impact.added_count, 1);
    assert_eq!(impact.removed_count, 1);
    assert_eq!(impact.unchanged_count, 1);
    impact
        .validate(&candidate, &baseline, &projection_digest)
        .unwrap();

    let mut tampered = impact;
    tampered.unchanged_count = 2;
    assert!(matches!(
        tampered.validate(&candidate, &baseline, &projection_digest),
        Err(CogniGraphError::DocumentConflict(_))
    ));
}
#[test]
fn impact_accepts_exact_candidate_and_baseline_bounds_and_rejects_one_over() {
    let exact = (0..MAX_M26_SEMANTIC_FACTS)
        .map(|index| fact("source", "REL", &format!("target-{index}"), "chunk"))
        .collect::<Vec<_>>();
    let projection_digest = digest("projection");
    VerifiedSemanticRepairImpact::new(&exact, &[], &projection_digest).unwrap();
    VerifiedSemanticRepairImpact::new(&[], &exact, &projection_digest).unwrap();

    let mut oversized = exact;
    oversized.push(fact("source", "REL", "target-over-limit", "chunk"));

    for (candidate, baseline) in [
        (oversized.as_slice(), &[][..]),
        (&[][..], oversized.as_slice()),
    ] {
        assert!(matches!(
            VerifiedSemanticRepairImpact::new(candidate, baseline, &projection_digest),
            Err(CogniGraphError::CapacityExceeded(_))
        ));
    }
}
#[test]
fn canonical_generation_bytes_accept_the_exact_limit_and_reject_one_over() {
    require_canonical_generation_byte_bound(MAX_M26_CANONICAL_GENERATION_BYTES as u64).unwrap();
    assert!(matches!(
        require_canonical_generation_byte_bound(MAX_M26_CANONICAL_GENERATION_BYTES as u64 + 1),
        Err(CogniGraphError::CapacityExceeded(_))
    ));
}
