use super::*;

#[test]
fn find_loose_recovers_original_offsets_across_whitespace() {
    let hay = "Grant of  rights.\nThe Producer grants   ConvergTV\nexclusive rights.";
    let (a, b) = find_loose(hay, "producer grants convergtv exclusive").unwrap();
    assert_eq!(&hay[a..b], "Producer grants   ConvergTV\nexclusive");
    assert!(find_loose(hay, "grants everything").is_none());
    assert!(find_loose(hay, "").is_none());
}

#[test]
fn endpoint_boundaries_work_independently_of_storage_key_restrictions() {
    assert_eq!(find_endpoint("(東京)", "東京"), Some((1, 7)));
    assert!(find_endpoint("東京市", "東京").is_none());
    assert_eq!(find_endpoint("東京市 東京", "東京"), Some((10, 16)));
    assert!(find_endpoint("", "東京").is_none());
    assert!(find_endpoint("東京", "  ").is_none());
}
