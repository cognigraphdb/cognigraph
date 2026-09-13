//! Preparation.

use super::*;

#[tokio::test]
async fn m23_static_raw_package_reproduces_the_golden_canonical_corpus() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/m23/preparation-golden.json"
    ))
    .unwrap();
    assert_eq!(fixture["schema_version"], json!(1));
    assert_eq!(fixture["unicode_version"], json!("17.0.0"));
    assert_eq!(
        cognigraph_construct::PREPARATION_UNICODE_VERSION,
        (17, 0, 0)
    );

    let decode_fixture_bytes = |field: &str| {
        let encoded = fixture[field].as_str().unwrap();
        let bytes = URL_SAFE_NO_PAD.decode(encoded).unwrap();
        assert_eq!(URL_SAFE_NO_PAD.encode(&bytes), encoded);
        bytes
    };
    let documents_bytes = decode_fixture_bytes("documents_json_base64url");
    let corpus_bytes = decode_fixture_bytes("corpus_json_base64url");
    assert_eq!(
        digest_bytes(&documents_bytes),
        fixture["documents_json_digest"].as_str().unwrap()
    );
    assert_eq!(
        digest_bytes(&corpus_bytes),
        fixture["corpus_json_digest"].as_str().unwrap()
    );

    let documents: RawDocumentSetArtifact = serde_json::from_slice(&documents_bytes).unwrap();
    let claimed: PreparedChunkCorpusArtifact = serde_json::from_slice(&corpus_bytes).unwrap();
    require_canonical_json("golden documents.json", &documents_bytes, &documents).unwrap();
    require_canonical_json("golden corpus.json", &corpus_bytes, &claimed).unwrap();
    let plan = RawCorpusPreparationPlan::supported_v1();
    assert_eq!(documents.preparation_plan_digest, plan.plan_digest);
    assert_eq!(claimed.preprocessing_digest, plan.plan_digest);

    let raw = documents
        .documents
        .iter()
        .map(|document| {
            Ok(RawDocumentBytes {
                id: document.id.clone(),
                title: document.title.clone(),
                bytes: decode_raw_document_bytes(document)?,
            })
        })
        .collect::<Result<Vec<_>, CogniGraphError>>()
        .unwrap();
    let prepared = prepare_documents(
        &raw,
        PreparationOptions {
            max_document_count: plan.max_documents as usize,
            max_document_bytes: plan.max_raw_document_bytes as usize,
            max_total_document_bytes: plan.max_total_raw_document_bytes as usize,
            max_normalized_document_bytes: plan.max_normalized_document_bytes as usize,
            max_total_normalized_bytes: plan.max_total_normalized_bytes as usize,
            max_chunk_bytes: plan.max_chunk_bytes as usize,
            max_chunk_count: plan.max_chunks as usize,
            max_total_prepared_bytes: plan.max_total_prepared_text_bytes as usize,
            yield_every_documents: plan.yield_every_documents as usize,
        },
    )
    .await
    .unwrap();
    let reproduced = PreparedChunkCorpusArtifact {
        schema_version: 1,
        space_type: documents.space_type,
        corpus_revision_id: documents.corpus_revision_id,
        preprocessing_digest: plan.plan_digest,
        chunks: prepared
            .into_iter()
            .map(|chunk| PreparedChunkArtifact {
                id: chunk.id,
                title: chunk.title,
                text: chunk.text,
            })
            .collect(),
    };
    assert_eq!(reproduced, claimed);
    let reproduced_bytes = cognigraph_governance::canonical_json_bytes(&reproduced).unwrap();
    assert_eq!(reproduced_bytes, corpus_bytes);
    assert_eq!(
        digest_bytes(&reproduced_bytes),
        fixture["corpus_json_digest"].as_str().unwrap()
    );
}
