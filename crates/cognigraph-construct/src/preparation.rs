//! Pure raw-document-to-prepared-chunk processing.
//!
//! The first governed preparation generation is deliberately text-only. It
//! accepts exact UTF-8 bytes, applies pinned mechanical normalization, and
//! emits deterministic byte-bounded chunks. It does not parse containers,
//! HTML, PDF, office formats, compressed input, or OCR output.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use unicode_normalization::{UnicodeNormalization, is_nfc};

const MAX_METADATA_BYTES: usize = 1_024;

/// Unicode data generation frozen into the M23 preparation contract.
pub const PREPARATION_UNICODE_VERSION: (u8, u8, u8) = (17, 0, 0);

// Both normalization and `char::is_whitespace` are observable preparation
// semantics. Refuse to compile if either dependency moves to a different
// Unicode generation without an explicit plan/version migration.
const _: () = {
    assert!(
        unicode_normalization::UNICODE_VERSION.0 == PREPARATION_UNICODE_VERSION.0
            && unicode_normalization::UNICODE_VERSION.1 == PREPARATION_UNICODE_VERSION.1
            && unicode_normalization::UNICODE_VERSION.2 == PREPARATION_UNICODE_VERSION.2,
        "M23 normalization tables must remain on Unicode 17.0.0"
    );
    assert!(
        char::UNICODE_VERSION.0 == PREPARATION_UNICODE_VERSION.0
            && char::UNICODE_VERSION.1 == PREPARATION_UNICODE_VERSION.1
            && char::UNICODE_VERSION.2 == PREPARATION_UNICODE_VERSION.2,
        "M23 whitespace tables must remain on Unicode 17.0.0"
    );
};

/// One exact raw document selected by the caller's verified artifact layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawDocumentBytes {
    pub id: String,
    pub title: String,
    pub bytes: Vec<u8>,
}

/// One prepared chunk suitable for the M22 grounding input.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedChunkRow {
    pub id: String,
    pub title: String,
    pub text: String,
}

/// Resource limits and cooperative scheduling for one preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparationOptions {
    pub max_document_count: usize,
    pub max_document_bytes: usize,
    pub max_total_document_bytes: usize,
    pub max_normalized_document_bytes: usize,
    pub max_total_normalized_bytes: usize,
    pub max_chunk_bytes: usize,
    pub max_chunk_count: usize,
    pub max_total_prepared_bytes: usize,
    pub yield_every_documents: usize,
}

/// Closed failures produced by deterministic preparation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PreparationError {
    #[error("preparation limits must be positive and max_chunk_bytes must be at least four")]
    InvalidLimits,
    #[error("raw document set must not be empty")]
    EmptyDocumentSet,
    #[error("raw document count exceeds configured maximum of {max_document_count}")]
    DocumentCountExceeded { max_document_count: usize },
    #[error("raw document `{document_id}` is empty")]
    EmptyDocument { document_id: String },
    #[error("raw document `{document_id}` repeats an id")]
    DuplicateDocumentId { document_id: String },
    #[error(
        "raw document metadata must be NFC, control-free, and at most 1024 bytes; ids must also be non-blank"
    )]
    InvalidDocumentMetadata,
    #[error(
        "raw document `{document_id}` exceeds configured maximum of {max_document_bytes} bytes"
    )]
    DocumentBytesExceeded {
        document_id: String,
        max_document_bytes: usize,
    },
    #[error("raw document set exceeds configured maximum of {max_total_document_bytes} bytes")]
    TotalDocumentBytesExceeded { max_total_document_bytes: usize },
    #[error("raw document `{document_id}` is not valid UTF-8")]
    InvalidUtf8 { document_id: String },
    #[error("raw document `{document_id}` contains an unsupported control character")]
    UnsupportedControl { document_id: String },
    #[error("raw document `{document_id}` is empty after normalization")]
    EmptyNormalizedDocument { document_id: String },
    #[error(
        "normalized document `{document_id}` exceeds configured maximum of {max_normalized_document_bytes} bytes"
    )]
    NormalizedDocumentBytesExceeded {
        document_id: String,
        max_normalized_document_bytes: usize,
    },
    #[error(
        "normalized document set exceeds configured maximum of {max_total_normalized_bytes} bytes"
    )]
    TotalNormalizedBytesExceeded { max_total_normalized_bytes: usize },
    #[error("prepared chunk count exceeds configured maximum of {max_chunk_count}")]
    ChunkCountExceeded { max_chunk_count: usize },
    #[error(
        "prepared chunk text exceeds configured aggregate maximum of {max_total_prepared_bytes} bytes"
    )]
    TotalPreparedBytesExceeded { max_total_prepared_bytes: usize },
}

/// Prepare exact UTF-8 documents into sorted, unique, byte-bounded chunks.
///
/// Semantics are intentionally mechanical and locale-independent:
///
/// 1. strip one leading UTF-8 BOM;
/// 2. map CRLF and bare CR to LF;
/// 3. normalize Unicode to NFC;
/// 4. treat blank lines as paragraph boundaries, trim Unicode whitespace at
///    each line edge, collapse interior runs to one ASCII space, and reject a
///    document with no non-blank paragraph;
/// 5. greedily pack paragraphs to `max_chunk_bytes`; overlong paragraphs split
///    first after `.`, `!`, or `?` followed by whitespace or end of input, then
///    at whitespace, then at the largest valid UTF-8 boundary that fits.
///
/// Chunk ids are the full SHA-256 of the raw document id plus a zero-based
/// eight-digit ordinal. Results are sorted by chunk id, so input permutation
/// cannot affect the canonical prepared corpus.
pub async fn prepare_documents(
    documents: &[RawDocumentBytes],
    options: PreparationOptions,
) -> Result<Vec<PreparedChunkRow>, PreparationError> {
    if options.max_document_count == 0
        || options.max_document_bytes == 0
        || options.max_total_document_bytes == 0
        || options.max_normalized_document_bytes == 0
        || options.max_total_normalized_bytes == 0
        || options.max_chunk_bytes < 4
        || options.max_chunk_count == 0
        || options.max_total_prepared_bytes == 0
        || options.yield_every_documents == 0
    {
        return Err(PreparationError::InvalidLimits);
    }
    if documents.is_empty() {
        return Err(PreparationError::EmptyDocumentSet);
    }
    if documents.len() > options.max_document_count {
        return Err(PreparationError::DocumentCountExceeded {
            max_document_count: options.max_document_count,
        });
    }

    let mut ordered = documents.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let mut seen_ids = HashSet::with_capacity(ordered.len());
    let mut total_bytes = 0_usize;
    let mut total_normalized_bytes = 0_usize;
    let mut total_prepared_bytes = 0_usize;
    let mut chunks = Vec::new();

    for (document_index, document) in ordered.into_iter().enumerate() {
        if document.id.trim().is_empty()
            || document.id.len() > MAX_METADATA_BYTES
            || document.id.chars().any(char::is_control)
            || !is_nfc(&document.id)
            || document.title.len() > MAX_METADATA_BYTES
            || document.title.chars().any(char::is_control)
            || !is_nfc(&document.title)
        {
            return Err(PreparationError::InvalidDocumentMetadata);
        }
        if !seen_ids.insert(document.id.as_str()) {
            return Err(PreparationError::DuplicateDocumentId {
                document_id: document.id.clone(),
            });
        }
        if document.bytes.is_empty() {
            return Err(PreparationError::EmptyDocument {
                document_id: document.id.clone(),
            });
        }
        if document.bytes.len() > options.max_document_bytes {
            return Err(PreparationError::DocumentBytesExceeded {
                document_id: document.id.clone(),
                max_document_bytes: options.max_document_bytes,
            });
        }
        total_bytes = total_bytes.checked_add(document.bytes.len()).ok_or(
            PreparationError::TotalDocumentBytesExceeded {
                max_total_document_bytes: options.max_total_document_bytes,
            },
        )?;
        if total_bytes > options.max_total_document_bytes {
            return Err(PreparationError::TotalDocumentBytesExceeded {
                max_total_document_bytes: options.max_total_document_bytes,
            });
        }

        let normalized = normalize_document(document, options.max_normalized_document_bytes)?;
        total_normalized_bytes = total_normalized_bytes.checked_add(normalized.len()).ok_or(
            PreparationError::TotalNormalizedBytesExceeded {
                max_total_normalized_bytes: options.max_total_normalized_bytes,
            },
        )?;
        if total_normalized_bytes > options.max_total_normalized_bytes {
            return Err(PreparationError::TotalNormalizedBytesExceeded {
                max_total_normalized_bytes: options.max_total_normalized_bytes,
            });
        }
        let paragraphs = normalized.split('\n').filter(|part| !part.is_empty());
        let mut current = String::new();
        let document_key = digest_hex(document.id.as_bytes());
        let mut ordinal = 0_usize;
        for paragraph in paragraphs {
            visit_bounded_pieces(paragraph, options.max_chunk_bytes, |piece| {
                let separator = usize::from(!current.is_empty());
                if !current.is_empty()
                    && current.len() + separator + piece.len() > options.max_chunk_bytes
                {
                    emit_chunk(
                        &mut chunks,
                        &document_key,
                        &document.title,
                        ordinal,
                        std::mem::take(&mut current),
                        &mut total_prepared_bytes,
                        options,
                    )?;
                    ordinal += 1;
                }
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(piece);
                Ok(())
            })?;
        }
        if !current.is_empty() {
            emit_chunk(
                &mut chunks,
                &document_key,
                &document.title,
                ordinal,
                current,
                &mut total_prepared_bytes,
                options,
            )?;
        }

        if (document_index + 1) % options.yield_every_documents == 0 {
            tokio::task::yield_now().await;
        }
    }

    chunks.sort();
    Ok(chunks)
}

fn normalize_document(
    document: &RawDocumentBytes,
    max_normalized_document_bytes: usize,
) -> Result<String, PreparationError> {
    let raw = std::str::from_utf8(&document.bytes).map_err(|_| PreparationError::InvalidUtf8 {
        document_id: document.id.clone(),
    })?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    if raw
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\r' | '\n' | '\t'))
    {
        return Err(PreparationError::UnsupportedControl {
            document_id: document.id.clone(),
        });
    }

    let newline_normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let nfc = newline_normalized.nfc().collect::<String>();
    if nfc.len() > max_normalized_document_bytes {
        return Err(PreparationError::NormalizedDocumentBytesExceeded {
            document_id: document.id.clone(),
            max_normalized_document_bytes,
        });
    }

    let mut paragraphs = Vec::new();
    let mut paragraph = String::new();
    for line in nfc.split('\n') {
        let collapsed = collapse_whitespace(line);
        if collapsed.is_empty() {
            if !paragraph.is_empty() {
                paragraphs.push(std::mem::take(&mut paragraph));
            }
        } else {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(&collapsed);
        }
    }
    if !paragraph.is_empty() {
        paragraphs.push(paragraph);
    }
    if paragraphs.is_empty() {
        return Err(PreparationError::EmptyNormalizedDocument {
            document_id: document.id.clone(),
        });
    }
    Ok(paragraphs.join("\n"))
}

fn collapse_whitespace(value: &str) -> String {
    let mut output = String::new();
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            pending_space = !output.is_empty();
        } else {
            if pending_space {
                output.push(' ');
                pending_space = false;
            }
            output.push(character);
        }
    }
    output
}

fn visit_bounded_pieces<E>(
    value: &str,
    max_bytes: usize,
    mut visit: impl FnMut(&str) -> Result<(), E>,
) -> Result<(), E> {
    let mut remaining = value;
    while remaining.len() > max_bytes {
        let hard = largest_boundary_at_or_before(remaining, max_bytes);
        let prefix = &remaining[..hard];
        let sentence = prefix
            .char_indices()
            .filter_map(|(index, character)| {
                matches!(character, '.' | '!' | '?').then_some(index + character.len_utf8())
            })
            .rfind(|end| {
                *end == remaining.len()
                    || remaining[*end..]
                        .chars()
                        .next()
                        .is_some_and(char::is_whitespace)
            });
        let whitespace = prefix
            .char_indices()
            .filter_map(|(index, character)| character.is_whitespace().then_some(index))
            .next_back();
        let cut = sentence
            .or(whitespace)
            .filter(|cut| *cut > 0)
            .unwrap_or(hard);
        let piece = remaining[..cut].trim_end();
        if !piece.is_empty() {
            visit(piece)?;
        }
        remaining = remaining[cut..].trim_start();
    }
    if !remaining.is_empty() {
        visit(remaining)?;
    }
    Ok(())
}

fn emit_chunk(
    chunks: &mut Vec<PreparedChunkRow>,
    document_key: &str,
    title: &str,
    ordinal: usize,
    text: String,
    total_prepared_bytes: &mut usize,
    options: PreparationOptions,
) -> Result<(), PreparationError> {
    if chunks.len() >= options.max_chunk_count {
        return Err(PreparationError::ChunkCountExceeded {
            max_chunk_count: options.max_chunk_count,
        });
    }
    *total_prepared_bytes = total_prepared_bytes.checked_add(text.len()).ok_or(
        PreparationError::TotalPreparedBytesExceeded {
            max_total_prepared_bytes: options.max_total_prepared_bytes,
        },
    )?;
    if *total_prepared_bytes > options.max_total_prepared_bytes {
        return Err(PreparationError::TotalPreparedBytesExceeded {
            max_total_prepared_bytes: options.max_total_prepared_bytes,
        });
    }
    chunks.push(PreparedChunkRow {
        id: format!("d-{document_key}-c{ordinal:08}"),
        title: title.to_string(),
        text,
    });
    Ok(())
}

fn largest_boundary_at_or_before(value: &str, max_bytes: usize) -> usize {
    let mut boundary = max_bytes.min(value.len());
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary.max(value.chars().next().map_or(0, char::len_utf8))
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write as _;
            let _ = write!(output, "{byte:02x}");
            output
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(max_chunk_bytes: usize) -> PreparationOptions {
        PreparationOptions {
            max_document_count: 8,
            max_document_bytes: 4096,
            max_total_document_bytes: 8192,
            max_normalized_document_bytes: 8192,
            max_total_normalized_bytes: 16384,
            max_chunk_bytes,
            max_chunk_count: 64,
            max_total_prepared_bytes: 16384,
            yield_every_documents: 1,
        }
    }

    #[tokio::test]
    async fn preparation_is_permutation_stable_and_normalizes_mechanically() {
        let a = RawDocumentBytes {
            id: "alpha".into(),
            title: "Alpha".into(),
            bytes: "\u{feff}Cafe\u{301}\r\n\r\n  First\t sentence.   Second sentence!"
                .as_bytes()
                .to_vec(),
        };
        let b = RawDocumentBytes {
            id: "beta".into(),
            title: "Beta".into(),
            bytes: b"Third paragraph? Tail".to_vec(),
        };
        let forward = prepare_documents(&[a.clone(), b.clone()], options(24))
            .await
            .unwrap();
        let reverse = prepare_documents(&[b, a], options(24)).await.unwrap();

        assert_eq!(forward, reverse);
        assert!(forward.iter().all(|chunk| chunk.text.len() <= 24));
        assert!(forward.iter().any(|chunk| chunk.text.contains("Caf\u{e9}")));
        assert!(
            forward
                .iter()
                .any(|chunk| chunk.text == "Caf\u{e9} First sentence.")
        );
    }

    #[tokio::test]
    async fn overlong_unicode_and_unbroken_tokens_split_on_utf8_boundaries() {
        let documents = [RawDocumentBytes {
            id: "unicode".into(),
            title: String::new(),
            bytes: "éééééééé".as_bytes().to_vec(),
        }];
        let chunks = prepare_documents(&documents, options(5)).await.unwrap();

        assert_eq!(chunks.len(), 4);
        assert!(chunks.iter().all(|chunk| chunk.text == "éé"));
        assert!(chunks.iter().all(|chunk| chunk.text.len() <= 5));
    }

    #[tokio::test]
    async fn preparation_fails_closed_on_invalid_or_excessive_input() {
        let invalid = RawDocumentBytes {
            id: "invalid".into(),
            title: String::new(),
            bytes: vec![0xff],
        };
        assert!(matches!(
            prepare_documents(&[invalid], options(32)).await,
            Err(PreparationError::InvalidUtf8 { .. })
        ));

        let duplicate = RawDocumentBytes {
            id: "duplicate".into(),
            title: String::new(),
            bytes: b"text".to_vec(),
        };
        assert!(matches!(
            prepare_documents(&[duplicate.clone(), duplicate], options(32)).await,
            Err(PreparationError::DuplicateDocumentId { .. })
        ));

        let mut bounded = options(4);
        bounded.max_chunk_count = 1;
        let long = RawDocumentBytes {
            id: "long".into(),
            title: String::new(),
            bytes: b"one two three".to_vec(),
        };
        assert_eq!(
            prepare_documents(&[long], bounded).await,
            Err(PreparationError::ChunkCountExceeded { max_chunk_count: 1 })
        );
    }

    #[tokio::test]
    async fn metadata_controls_empty_text_and_every_size_boundary_fail_closed() {
        let document = |id: &str, bytes: &[u8]| RawDocumentBytes {
            id: id.into(),
            title: String::new(),
            bytes: bytes.to_vec(),
        };

        assert_eq!(
            prepare_documents(&[document("control", b"safe\0unsafe")], options(32)).await,
            Err(PreparationError::UnsupportedControl {
                document_id: "control".into()
            })
        );
        assert_eq!(
            prepare_documents(&[document("blank", b" \t\r\n\r\n")], options(32)).await,
            Err(PreparationError::EmptyNormalizedDocument {
                document_id: "blank".into()
            })
        );
        assert_eq!(
            prepare_documents(&[document("Cafe\u{301}", b"text")], options(32)).await,
            Err(PreparationError::InvalidDocumentMetadata)
        );
        let mut non_nfc_title = document("title", b"text");
        non_nfc_title.title = "Cafe\u{301}".into();
        assert_eq!(
            prepare_documents(&[non_nfc_title], options(32)).await,
            Err(PreparationError::InvalidDocumentMetadata)
        );

        let mut bounded = options(32);
        bounded.max_document_count = 1;
        assert!(matches!(
            prepare_documents(&[document("a", b"a"), document("b", b"b")], bounded).await,
            Err(PreparationError::DocumentCountExceeded { .. })
        ));
        bounded = options(32);
        bounded.max_document_bytes = 3;
        assert!(matches!(
            prepare_documents(&[document("large", b"four")], bounded).await,
            Err(PreparationError::DocumentBytesExceeded { .. })
        ));
        bounded = options(32);
        bounded.max_total_document_bytes = 3;
        assert!(matches!(
            prepare_documents(&[document("a", b"aa"), document("b", b"bb")], bounded).await,
            Err(PreparationError::TotalDocumentBytesExceeded { .. })
        ));
        bounded = options(32);
        bounded.max_normalized_document_bytes = 3;
        assert!(matches!(
            prepare_documents(&[document("normalized", b"four")], bounded).await,
            Err(PreparationError::NormalizedDocumentBytesExceeded { .. })
        ));
        bounded = options(32);
        bounded.max_total_normalized_bytes = 3;
        assert!(matches!(
            prepare_documents(&[document("a", b"aa"), document("b", b"bb")], bounded).await,
            Err(PreparationError::TotalNormalizedBytesExceeded { .. })
        ));
        bounded = options(4);
        bounded.max_total_prepared_bytes = 3;
        assert!(matches!(
            prepare_documents(&[document("prepared", b"four")], bounded).await,
            Err(PreparationError::TotalPreparedBytesExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn sentence_boundaries_protect_decimals_and_dotted_names_and_ids_are_stable() {
        let document = RawDocumentBytes {
            id: "source-alpha".into(),
            title: "Boundary cases".into(),
            bytes: b"Dose 3.09 works. Chorus.ai works. OpenProtein.AI works.".to_vec(),
        };
        let chunks = prepare_documents(&[document], options(20)).await.unwrap();

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Dose 3.09 works.",
                "Chorus.ai works.",
                "OpenProtein.AI",
                "works."
            ]
        );
        let document_key = digest_hex(b"source-alpha");
        for (ordinal, chunk) in chunks.iter().enumerate() {
            assert_eq!(
                chunk.id,
                format!("d-{document_key}-c{ordinal:08}"),
                "chunk ids must bind the NFC document id and zero-based ordinal"
            );
            assert!(
                chunk
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            );
        }
    }
}
