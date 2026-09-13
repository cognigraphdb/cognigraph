//! Generation capacity.

use super::*;

#[test]
fn generation_capacity_accepts_exact_tenant_and_space_counts_and_rejects_one_over() {
    let exact_space =
        generation_capacity_entries(MAX_M26_GENERATIONS_PER_SPACE, |_| "one-space".into(), |_| 1);
    validate_test_generation_capacity(&exact_space).unwrap();
    let over_space = generation_capacity_entries(
        MAX_M26_GENERATIONS_PER_SPACE + 1,
        |_| "one-space".into(),
        |_| 1,
    );
    assert_eq!(
        validate_test_generation_capacity(&over_space),
        Err(GenerationCapacityViolation::SpaceCount)
    );

    let exact_tenant = generation_capacity_entries(
        MAX_M26_GENERATIONS_PER_TENANT,
        |index| format!("space-{index}"),
        |_| 1,
    );
    validate_test_generation_capacity(&exact_tenant).unwrap();
    let over_tenant = generation_capacity_entries(
        MAX_M26_GENERATIONS_PER_TENANT + 1,
        |index| format!("space-{index}"),
        |_| 1,
    );
    assert_eq!(
        validate_test_generation_capacity(&over_tenant),
        Err(GenerationCapacityViolation::TenantCount)
    );
}
#[test]
fn generation_capacity_accepts_exact_aggregate_bytes_and_rejects_one_over() {
    let mut exact = generation_capacity_entries(
        MAX_M26_GENERATIONS_PER_SPACE,
        |_| "full-space".into(),
        |_| MAX_M26_CANONICAL_GENERATION_BYTES as u64,
    );
    assert_eq!(
        exact.iter().map(|(_, _, bytes)| *bytes).sum::<u64>(),
        MAX_M26_GENERATION_BYTES_PER_TENANT as u64
    );
    validate_test_generation_capacity(&exact).unwrap();

    exact.push(("next-space".into(), "idempotency-over".into(), 1));
    assert_eq!(
        validate_test_generation_capacity(&exact),
        Err(GenerationCapacityViolation::AggregateBytes)
    );
}
#[test]
fn deployment_chain_capacity_accepts_exact_counts_and_rejects_one_over() {
    validate_deployment_chain_capacities([("one-space", MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE)])
        .unwrap();
    assert_eq!(
        validate_deployment_chain_capacities([(
            "one-space",
            MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE + 1,
        )]),
        Err(DeploymentChainCapacityViolation::DecisionCount)
    );

    let exact_spaces = (0..MAX_M26_DEPLOYMENT_SPACES_PER_TENANT)
        .map(|index| format!("space-{index}"))
        .collect::<Vec<_>>();
    validate_deployment_chain_capacities(
        exact_spaces
            .iter()
            .map(|space_type| (space_type.as_str(), 1)),
    )
    .unwrap();
    let over_spaces = (0..=MAX_M26_DEPLOYMENT_SPACES_PER_TENANT)
        .map(|index| format!("space-{index}"))
        .collect::<Vec<_>>();
    assert_eq!(
        validate_deployment_chain_capacities(
            over_spaces
                .iter()
                .map(|space_type| (space_type.as_str(), 1)),
        ),
        Err(DeploymentChainCapacityViolation::SpaceCount)
    );
}
