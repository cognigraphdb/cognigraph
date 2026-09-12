//! Exercise the stored-document cache behind Native's text-search hit lookup.
//! CG-70 advances Tantivy's lru dependency across minor versions.
use tantivy::directory::RamDirectory;
use tantivy::schema::{STORED, Schema, Value as _};
use tantivy::{DocAddress, Index, IndexSettings, Searcher, TantivyDocument};

#[test]
fn stored_documents_survive_cache_hits_eviction_and_reopen() -> tantivy::Result<()> {
    let directory = RamDirectory::create();
    let mut builder = Schema::builder();
    let key = builder.add_text_field("_key", STORED);
    let schema = builder.build();
    let index = Index::create(
        directory.clone(),
        schema,
        IndexSettings {
            docstore_blocksize: 64,
            ..IndexSettings::default()
        },
    )?;
    // Each key exceeds the block size, guaranteeing distinct stored blocks.
    let keys: Vec<String> = (0..4)
        .map(|i| format!("key-{i}-{}", "x".repeat(256)))
        .collect();
    let mut writer = index.writer_with_num_threads(1, 15_000_000)?;
    for value in &keys {
        let mut document = TantivyDocument::default();
        document.add_text(key, value);
        writer.add_document(document)?;
    }
    writer.commit()?;
    writer.wait_merging_threads()?;

    let reader = index
        .reader_builder()
        .doc_store_cache_num_blocks(2)
        .try_into()?;
    let searcher = reader.searcher();
    assert_eq!(searcher.segment_readers().len(), 1);
    let read = |searcher: &Searcher, id: u32| -> tantivy::Result<()> {
        let document: TantivyDocument = searcher.doc(DocAddress::new(0, id))?;
        assert_eq!(
            document.get_first(key).and_then(|v| v.as_str()),
            Some(keys[id as usize].as_str())
        );
        Ok(())
    };
    for id in [0, 1, 0, 2, 0] {
        read(&searcher, id)?;
    }
    let stats = searcher.doc_store_cache_stats();
    assert_eq!(
        (stats.cache_hits, stats.cache_misses, stats.num_entries),
        (2, 3, 2)
    );
    // Reading 0 promoted it; inserting 2 evicted 1, whose next read must miss.
    read(&searcher, 1)?;
    assert_eq!(searcher.doc_store_cache_stats().cache_misses, 4);
    drop(searcher);
    drop(reader);
    drop(index);

    let reopened = Index::open(directory)?;
    let reader = reopened
        .reader_builder()
        .doc_store_cache_num_blocks(2)
        .try_into()?;
    let searcher = reader.searcher();
    for id in [3, 2, 1, 0, 3] {
        read(&searcher, id)?;
    }
    assert_eq!(searcher.doc_store_cache_stats().cache_misses, 5);
    assert_eq!(searcher.doc_store_cache_stats().num_entries, 2);
    Ok(())
}
