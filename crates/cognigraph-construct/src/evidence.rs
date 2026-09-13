//! Construction evidence uses NFC text before any byte-sensitive derivation.
use std::borrow::Cow;

use unicode_normalization::{UnicodeNormalization, is_nfc};

use crate::Chunk;

pub(crate) fn canonical_text(text: &str) -> Cow<'_, str> {
    if is_nfc(text) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.nfc().collect())
    }
}

/// Preserve caller-owned identifiers and metadata. Only the evidence text is
/// canonicalized; offsets and content hashes refer to these UTF-8 bytes.
pub(crate) fn canonical_chunks(chunks: &[Chunk]) -> Cow<'_, [Chunk]> {
    if chunks.iter().all(|chunk| is_nfc(&chunk.text)) {
        Cow::Borrowed(chunks)
    } else {
        Cow::Owned(
            chunks
                .iter()
                .map(|chunk| Chunk {
                    text: canonical_text(&chunk.text).into_owned(),
                    ..chunk.clone()
                })
                .collect(),
        )
    }
}
