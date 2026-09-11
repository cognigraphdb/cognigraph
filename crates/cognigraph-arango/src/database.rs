use cognigraph_core::{CollectionType, GraphBackend, IndexDef, IndexType, Result};
use tracing::info;

use crate::backend::ArangoBackend;

/// Default collection names used by CogniGraph.
pub mod collections {
    pub const DOCUMENTS: &str = "documents";
    pub const EMBEDDINGS: &str = "embeddings";
    pub const DOCUMENT_RELATIONS: &str = "document_relations";
}

/// Initialize the CogniGraph database schema.
///
/// Creates all required collections and indexes if they don't exist.
pub async fn initialize(backend: &ArangoBackend) -> Result<()> {
    info!("Initializing CogniGraph database schema");

    // --- Collections ---

    backend
        .ensure_collection(collections::DOCUMENTS, CollectionType::Document)
        .await?;

    backend
        .ensure_collection(collections::EMBEDDINGS, CollectionType::Document)
        .await?;

    backend
        .ensure_collection(collections::DOCUMENT_RELATIONS, CollectionType::Edge)
        .await?;

    // --- Indexes on embeddings ---

    backend
        .ensure_index(
            collections::EMBEDDINGS,
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["document_id".into()],
                unique: false,
                sparse: false,
                name: Some("idx_embedding_document_id".into()),
            },
        )
        .await?;

    backend
        .ensure_index(
            collections::EMBEDDINGS,
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["model_name".into()],
                unique: false,
                sparse: false,
                name: Some("idx_embedding_model_name".into()),
            },
        )
        .await?;

    backend
        .ensure_index(
            collections::EMBEDDINGS,
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["dimension".into()],
                unique: false,
                sparse: false,
                name: Some("idx_embedding_dimension".into()),
            },
        )
        .await?;

    // --- Indexes on document_relations ---

    backend
        .ensure_index(
            collections::DOCUMENT_RELATIONS,
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["relation_type".into()],
                unique: false,
                sparse: false,
                name: Some("idx_relation_type".into()),
            },
        )
        .await?;

    backend
        .ensure_index(
            collections::DOCUMENT_RELATIONS,
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["confidence".into()],
                unique: false,
                sparse: false,
                name: Some("idx_relation_confidence".into()),
            },
        )
        .await?;

    // Composite index for upsert deduplication
    backend
        .ensure_index(
            collections::DOCUMENT_RELATIONS,
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["_from".into(), "_to".into(), "relation_type".into()],
                unique: true,
                sparse: false,
                name: Some("idx_relation_upsert".into()),
            },
        )
        .await?;

    // --- Indexes on documents ---

    backend
        .ensure_index(
            collections::DOCUMENTS,
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["category".into()],
                unique: false,
                sparse: true,
                name: Some("idx_document_category".into()),
            },
        )
        .await?;

    info!("Database schema initialization complete");
    Ok(())
}
