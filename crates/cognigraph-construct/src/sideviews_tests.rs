use super::*;
use cognigraph_embeddings::completion::supports_openai_strict_mode;

#[test]
fn schema_qualifies_for_openai_strict_mode() {
    // A closed, fully-required schema is what lets both families enforce the
    // shape server-side instead of us parsing prose JSON.
    assert!(supports_openai_strict_mode(&sideviews_schema()));
}

#[test]
fn user_turn_embeds_count_and_text() {
    let prompt = sideviews_user("Ada Lovelace wrote the first algorithm.", 7);
    assert!(prompt.contains("exactly 7"));
    assert!(prompt.contains("Ada Lovelace"));
}

#[test]
fn qa_pair_round_trips_through_schema_shape() {
    let value = json!({ "question": "Who wrote it?", "answer": "Ada Lovelace." });
    let pair: QaPair = serde_json::from_value(value).unwrap();
    assert_eq!(pair.question, "Who wrote it?");
    assert_eq!(pair.answer, "Ada Lovelace.");
}
