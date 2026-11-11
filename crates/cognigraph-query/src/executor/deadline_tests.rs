//! CG-197: the deadline every stage reads also carries the caller's
//! cancellation.

use std::time::{Duration, Instant};

use super::pipeline::sort_within_deadline;
use super::{Cancellation, Deadline, ExecutionBudget, ExecutionError};

fn cancellable(cancellation: &Cancellation) -> Deadline {
    Deadline::start(&ExecutionBudget::unlimited(), Some(cancellation))
}

#[test]
fn without_a_time_budget_or_a_cancellation_nothing_stops_a_query() {
    let deadline = Deadline::start(&ExecutionBudget::unlimited(), None);
    assert!(!deadline.can_stop());
    assert_eq!(deadline.check(), Ok(()));
    assert!(!Deadline::none().can_stop());
}

#[test]
fn a_cancellation_stops_a_query_that_has_no_time_budget() {
    let cancellation = Cancellation::new();
    let deadline = cancellable(&cancellation);
    assert!(deadline.can_stop());
    assert_eq!(deadline.check(), Ok(()));
    // A copy handed to another thread raises the same flag.
    let remote = cancellation.clone();
    assert!(!remote.is_cancelled());
    remote.cancel();
    assert!(cancellation.is_cancelled());
    assert_eq!(deadline.check(), Err(ExecutionError::Cancelled));
    // The context of a subquery is a clone of the deadline.
    assert_eq!(deadline.clone().check(), Err(ExecutionError::Cancelled));
    // Raising it twice changes nothing.
    remote.cancel();
    assert_eq!(deadline.check(), Err(ExecutionError::Cancelled));
}

#[test]
fn a_cancelled_query_that_is_also_out_of_time_reports_the_cancellation() {
    let cancellation = Cancellation::new();
    let deadline = Deadline {
        at: Some(Instant::now() - Duration::from_millis(1)),
        cancellation: Some(cancellation.clone()),
    };
    assert_eq!(deadline.check(), Err(ExecutionError::TimeBudgetExceeded));
    cancellation.cancel();
    assert_eq!(deadline.check(), Err(ExecutionError::Cancelled));
}

#[test]
fn inner_loops_read_the_flag_once_every_1024_rows() {
    let cancellation = Cancellation::new();
    cancellation.cancel();
    let deadline = cancellable(&cancellation);
    let mut ticks = 0u32;
    for row in 1..1_024 {
        assert_eq!(deadline.check_every(&mut ticks), Ok(()), "row {row}");
    }
    assert_eq!(
        deadline.check_every(&mut ticks),
        Err(ExecutionError::Cancelled)
    );
}

#[test]
fn a_cancellation_stops_a_long_sort_and_an_unraised_one_does_not_change_it() {
    let rows: Vec<u32> = (0..40_000u32).rev().collect();
    let mut sorted = rows.clone();
    sorted.sort();

    let cancellation = Cancellation::new();
    let deadline = cancellable(&cancellation);
    assert_eq!(
        sort_within_deadline(rows.clone(), &deadline, u32::cmp).unwrap(),
        sorted
    );
    cancellation.cancel();
    assert_eq!(
        sort_within_deadline(rows, &deadline, u32::cmp).unwrap_err(),
        ExecutionError::Cancelled
    );
    // A sort of one run is a single step and finishes.
    assert_eq!(
        sort_within_deadline(vec![3, 1, 2], &deadline, u32::cmp).unwrap(),
        vec![1, 2, 3]
    );
}
