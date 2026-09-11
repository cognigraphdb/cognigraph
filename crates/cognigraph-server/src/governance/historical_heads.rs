//! Historical heads.

use super::*;

/// Return every head that could have been current at `at_ms` when immutable
/// authority records share a millisecond timestamp. Decisions strictly before
/// the revision are definite. A contiguous prefix of decisions accepted in
/// the same millisecond can be either before or after the revision, so each
/// reachable prefix head is valid historical evidence.
pub(super) fn possible_promotion_heads_at(
    actionable: &[(&str, Option<&str>, u64)],
    at_ms: u64,
) -> Result<HashSet<Option<String>>, CogniGraphError> {
    let strict = actionable
        .iter()
        .copied()
        .filter(|(_, _, created_at_ms)| *created_at_ms < at_ms)
        .collect::<Vec<_>>();
    let strict_predecessors = strict
        .iter()
        .filter_map(|(_, predecessor, _)| *predecessor)
        .collect::<HashSet<_>>();
    let strict_heads = strict
        .iter()
        .filter(|(id, _, _)| !strict_predecessors.contains(id))
        .map(|(id, _, _)| (*id).to_string())
        .collect::<Vec<_>>();
    let strict_head = match strict_heads.as_slice() {
        [] => None,
        [head] => Some(head.clone()),
        _ => {
            return Err(conflict(
                "semantic repair revision history has multiple promotion heads",
            ));
        }
    };

    let mut possible = HashSet::from([strict_head]);
    let mut remaining = actionable
        .iter()
        .copied()
        .filter(|(_, _, created_at_ms)| *created_at_ms == at_ms)
        .collect::<Vec<_>>();
    loop {
        let mut deferred = Vec::new();
        let mut progressed = false;
        for (id, predecessor, created_at_ms) in remaining {
            let predecessor_key = predecessor.map(str::to_string);
            if possible.contains(&predecessor_key) {
                possible.insert(Some(id.to_string()));
                progressed = true;
            } else {
                deferred.push((id, predecessor, created_at_ms));
            }
        }
        if !progressed {
            break;
        }
        remaining = deferred;
    }
    Ok(possible)
}
