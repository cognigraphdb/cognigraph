//! The single frozen normalization + triple-matching function (W6). All triple
//! comparison in the WebNLG scorer goes through here; nothing else normalizes.

use crate::webnlg::model::Triple;

/// Canonical form of one field: replace `_` with space, trim, lowercase; if the
/// result parses as a number, canonicalize it numerically so "2702.0" == "2702".
/// Applied to all three fields — predicates are never numeric, subjects almost
/// never are, so numeric canonicalization is harmless there and keeps one rule.
/// The numeric branch also absorbs non-finite float tokens ("nan"/"inf") and
/// strips leading zeros ("007" -> "7"); this is accepted for the WebNLG corpus,
/// whose numeric objects are finite, unpadded years/counts/measurements.
pub fn canonical_field(raw: &str) -> String {
    let base = raw.replace('_', " ");
    let base = base.trim().to_lowercase();
    if let Ok(x) = base.parse::<f64>() {
        // `{x}` gives "2702" for both 2702.0 and 2702, "507" for 507/507.0.
        return format!("{x}");
    }
    base
}

/// How a constructed triple relates to one oracle triple. Ordered by strength;
/// the caller keeps the strongest match found across the oracle set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Correct,
    Mislabeled,
    EntityPairOnly,
}

/// Classify a constructed triple against one oracle triple, or `None` if the
/// entity pair does not overlap at all.
pub fn classify(constructed: &Triple, oracle: &Triple) -> Option<MatchKind> {
    let cs = canonical_field(&constructed.subject);
    let co = canonical_field(&constructed.object);
    let cp = canonical_field(&constructed.predicate);
    let os = canonical_field(&oracle.subject);
    let oo = canonical_field(&oracle.object);
    let op = canonical_field(&oracle.predicate);

    let same_direction = cs == os && co == oo;
    let reversed = cs == oo && co == os;

    if same_direction && cp == op {
        Some(MatchKind::Correct)
    } else if same_direction {
        Some(MatchKind::Mislabeled)
    } else if reversed {
        // The pair is linked, but the constructed edge runs the other way.
        Some(MatchKind::EntityPairOnly)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::model::Triple;

    fn t(s: &str, p: &str, o: &str) -> Triple {
        Triple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
        }
    }

    #[test]
    fn underscores_and_case_normalize() {
        assert_eq!(
            canonical_field("Harrietstown,_New_York"),
            "harrietstown, new york"
        );
        assert_eq!(canonical_field("  Aarhus_Airport "), "aarhus airport");
    }

    #[test]
    fn numeric_objects_compare_numerically() {
        // 2702.0 == 2702, 507 == 507.0
        assert_eq!(canonical_field("2702.0"), canonical_field("2702"));
        assert_eq!(canonical_field("507"), canonical_field("507.0"));
        assert_ne!(canonical_field("507"), canonical_field("508"));
    }

    #[test]
    fn numeric_object_matches_through_classify() {
        let c = t("Aarhus_Airport", "runwayLength", "2702.0");
        let o = t("Aarhus_Airport", "runwayLength", "2702");
        assert_eq!(classify(&c, &o), Some(MatchKind::Correct));
    }

    #[test]
    fn exact_triple_is_correct() {
        let c = t("Aarhus", "leader", "Jacob_Bundsgaard");
        let o = t("aarhus", "leader", "jacob bundsgaard");
        assert_eq!(classify(&c, &o), Some(MatchKind::Correct));
    }

    #[test]
    fn same_pair_wrong_predicate_is_mislabeled() {
        let c = t("Aarhus", "governor", "Jacob_Bundsgaard");
        let o = t("Aarhus", "leader", "Jacob_Bundsgaard");
        assert_eq!(classify(&c, &o), Some(MatchKind::Mislabeled));
    }

    #[test]
    fn reversed_pair_is_entity_pair_only() {
        let c = t("Jacob_Bundsgaard", "leaderOf", "Aarhus");
        let o = t("Aarhus", "leader", "Jacob_Bundsgaard");
        assert_eq!(classify(&c, &o), Some(MatchKind::EntityPairOnly));
    }

    #[test]
    fn reversed_pair_same_predicate_is_entity_pair_only() {
        let c = t("Jacob_Bundsgaard", "leader", "Aarhus");
        let o = t("Aarhus", "leader", "Jacob_Bundsgaard");
        assert_eq!(classify(&c, &o), Some(MatchKind::EntityPairOnly));
    }

    #[test]
    fn unrelated_triple_does_not_match() {
        let c = t("Berlin", "leader", "Someone");
        let o = t("Aarhus", "leader", "Jacob_Bundsgaard");
        assert_eq!(classify(&c, &o), None);
    }
}
