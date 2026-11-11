//! CG-111: the interruptible sort orders exactly like the standard sort.

use std::cmp::Ordering;
use std::time::{Duration, Instant};

use super::*;

fn far_deadline() -> Deadline {
    Deadline(Some(Instant::now() + Duration::from_secs(600)))
}

/// A small deterministic generator, so the cases do not need a random crate.
fn sequence(len: usize, modulus: u64, mut seed: u64) -> Vec<(u64, usize)> {
    (0..len)
        .map(|position| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) % modulus, position)
        })
        .collect()
}

fn by_key(left: &(u64, usize), right: &(u64, usize)) -> Ordering {
    left.0.cmp(&right.0)
}

#[test]
fn merged_runs_match_the_standard_stable_sort() {
    // Lengths around the run size and its multiples, with few distinct keys
    // (many ties), all-equal keys and already-ordered input.
    for len in [
        0,
        1,
        2,
        SORT_RUN - 1,
        SORT_RUN,
        SORT_RUN + 1,
        2 * SORT_RUN,
        2 * SORT_RUN + 1,
        3 * SORT_RUN + 17,
        5 * SORT_RUN - 1,
    ] {
        for modulus in [1, 2, 7, 1_000, u64::MAX] {
            let input = sequence(len, modulus, len as u64 ^ modulus);
            let mut expected = input.clone();
            expected.sort_by(by_key);
            let sorted = sort_within_deadline(input, far_deadline(), by_key).unwrap();
            // Positions are unique, so equality also proves stability.
            assert_eq!(sorted, expected, "len {len} modulus {modulus}");
        }
    }
    let ordered: Vec<(u64, usize)> = (0..3 * SORT_RUN).map(|i| (i as u64, i)).collect();
    assert_eq!(
        sort_within_deadline(ordered.clone(), far_deadline(), by_key).unwrap(),
        ordered
    );
    let reversed: Vec<(u64, usize)> = ordered.iter().rev().copied().collect();
    assert_eq!(
        sort_within_deadline(reversed, far_deadline(), by_key).unwrap(),
        ordered
    );
}

#[test]
fn without_a_deadline_the_standard_sort_runs() {
    let input = sequence(3 * SORT_RUN, 5, 9);
    let mut expected = input.clone();
    expected.sort_by(by_key);
    assert_eq!(
        sort_within_deadline(input, Deadline(None), by_key).unwrap(),
        expected
    );
}

#[test]
fn an_expired_deadline_stops_a_long_sort_but_not_a_short_one() {
    let expired = Deadline(Some(Instant::now() - Duration::from_millis(1)));
    let long = sequence(2 * SORT_RUN, 1_000, 3);
    assert_eq!(
        sort_within_deadline(long, expired, by_key).unwrap_err(),
        ExecutionError::TimeBudgetExceeded
    );
    // One run is sorted in a single uninterruptible step.
    let short = sequence(SORT_RUN, 1_000, 3);
    assert!(sort_within_deadline(short, expired, by_key).is_ok());
}
