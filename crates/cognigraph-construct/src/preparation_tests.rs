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
