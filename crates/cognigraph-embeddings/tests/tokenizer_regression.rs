//! Offline compatibility boundary for the optional ONNX tokenizer dependency.
#![cfg(feature = "onnx")]

use tokenizers::{PaddingParams, Tokenizer};

#[test]
fn bpe_unicode_merges_unknowns_and_batch_padding_remain_stable() {
    let mut tokenizer = Tokenizer::from_bytes(
        br#"{"model":{"type":"BPE","unk_token":"[UNK]",
        "vocab":{"[UNK]":0,"a":1,"b":2,"ab":3,"\u00e9":4},
        "merges":[["a","b"]]}}"#,
    )
    .unwrap();
    tokenizer.with_padding(Some(PaddingParams::default()));
    let encoded = tokenizer
        .encode_batch(vec!["abé", "ab", "?"], false)
        .unwrap();
    assert_eq!(encoded[0].get_ids(), &[3, 4]);
    assert_eq!(encoded[0].get_offsets(), &[(0, 2), (2, 4)]);
    assert_eq!(encoded[1].get_ids(), &[3, 0]);
    assert_eq!(encoded[1].get_attention_mask(), &[1, 0]);
    assert_eq!(encoded[2].get_tokens(), &["[UNK]", "[PAD]"]);
    assert_eq!(tokenizer.get_vocab_size(false), 5);
}
