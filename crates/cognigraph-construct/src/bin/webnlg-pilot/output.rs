use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use sha2::{Digest, Sha256};

use super::model::*;
use super::source::{
    CONFIG, DATASET, EXCLUDED_CHALLENGE_SPLITS, HUB_REVISION, SPLITS, cached_split_sha256,
};

pub fn compile_split(
    output: &Path,
    split: &str,
    pages: Vec<ViewerResponse>,
) -> Result<SplitSummary> {
    fs::create_dir_all(output.join("documents"))?;
    fs::create_dir_all(output.join("oracle"))?;
    let documents_path = output.join("documents").join(format!("{split}.jsonl"));
    let oracle_path = output.join("oracle").join(format!("{split}.jsonl"));
    let manifest_path = output.join(format!("manifest-{split}.jsonl"));
    let mut documents = BufWriter::new(File::create(&documents_path)?);
    let mut oracle = BufWriter::new(File::create(&oracle_path)?);
    let mut manifest = BufWriter::new(File::create(&manifest_path)?);
    let mut predicates = BTreeSet::new();
    let mut categories = BTreeSet::new();
    let mut row_count = 0usize;
    let mut triple_count = 0usize;

    for viewer_row in pages.into_iter().flat_map(|page| page.rows) {
        let source = viewer_row.row;
        let triples: Vec<OracleTriple> = source
            .input
            .iter()
            .map(|raw| parse_triple(raw))
            .collect::<Result<_>>()?;
        if triples.is_empty() || triples.len() > 7 {
            bail!(
                "{} has {} triples; canonical WebNLG rows must contain 1..=7",
                source.gem_id,
                triples.len()
            );
        }
        predicates.extend(triples.iter().map(|triple| triple.predicate.clone()));
        categories.insert(source.category.clone());
        triple_count += triples.len();
        let text_sha256 = sha256(source.target.as_bytes());
        let document = PilotDocument {
            schema_version: SCHEMA_VERSION.to_string(),
            document_id: source.gem_id.clone(),
            parent_id: source.gem_parent_id,
            split: split.to_string(),
            category: source.category.clone(),
            webnlg_id: source.webnlg_id.clone(),
            source_row_index: viewer_row.row_idx,
            source_url: "https://huggingface.co/datasets/GEM/web_nlg".to_string(),
            char_count: source.target.chars().count(),
            text_sha256: text_sha256.clone(),
            text: source.target,
        };
        let oracle_record = OracleRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            document_id: source.gem_id.clone(),
            split: split.to_string(),
            triples,
            references: source.references,
        };
        let entry = ManifestEntry {
            document_id: source.gem_id,
            split: split.to_string(),
            source_row_index: viewer_row.row_idx,
            category: source.category,
            webnlg_id: source.webnlg_id,
            triple_count: oracle_record.triples.len(),
            char_count: document.char_count,
            text_sha256,
            documents_path: format!("documents/{split}.jsonl"),
            oracle_path: format!("oracle/{split}.jsonl"),
        };
        write_json_line(&mut documents, &document)?;
        write_json_line(&mut oracle, &oracle_record)?;
        write_json_line(&mut manifest, &entry)?;
        row_count += 1;
    }
    documents.flush()?;
    oracle.flush()?;
    manifest.flush()?;

    Ok(SplitSummary {
        split: split.to_string(),
        rows: row_count,
        triples: triple_count,
        unique_predicates: predicates.len(),
        unique_categories: categories.len(),
        raw_sha256: cached_split_sha256(output, split, row_count)?,
        documents_sha256: sha256(&fs::read(documents_path)?),
        oracle_sha256: sha256(&fs::read(oracle_path)?),
        manifest_sha256: sha256(&fs::read(manifest_path)?),
    })
}

pub fn verify_output(output: &Path) -> Result<RunMetadata> {
    let run: RunMetadata = read_json(&output.join("run.json"))?;
    let request: RunRequest = read_json(&output.join("request.json"))?;
    if request != expected_request() {
        bail!("request.json does not describe the pinned WebNLG baseline");
    }
    if run.schema_version != SCHEMA_VERSION
        || run.dataset != DATASET
        || run.config != CONFIG
        || run.hub_revision != HUB_REVISION
        || run.license != "CC-BY-NC-4.0"
        || run.source != "Hugging Face Dataset Viewer API"
        || run.challenge_splits_excluded
            != EXCLUDED_CHALLENGE_SPLITS
                .iter()
                .map(|split| (*split).to_string())
                .collect::<Vec<_>>()
        || run.splits.len() != SPLITS.len()
    {
        bail!("run.json does not describe the pinned WebNLG baseline");
    }
    chrono::DateTime::parse_from_rfc3339(&run.generated_at)
        .context("run.json generated_at is not RFC 3339")?;
    let mut all_ids = HashSet::new();
    let mut total_documents = 0usize;
    let mut total_triples = 0usize;
    let mut all_predicates = BTreeSet::new();
    let mut all_categories = BTreeSet::new();

    for (split, expected_rows) in SPLITS {
        let documents_path = output.join("documents").join(format!("{split}.jsonl"));
        let oracle_path = output.join("oracle").join(format!("{split}.jsonl"));
        let manifest_path = output.join(format!("manifest-{split}.jsonl"));
        let documents: Vec<PilotDocument> = read_jsonl(&documents_path)?;
        let oracle: Vec<OracleRecord> = read_jsonl(&oracle_path)?;
        let manifest: Vec<ManifestEntry> = read_jsonl(&manifest_path)?;
        if documents.len() != expected_rows
            || oracle.len() != expected_rows
            || manifest.len() != expected_rows
        {
            bail!("{split} output does not contain {expected_rows} aligned rows");
        }

        let mut split_predicates = BTreeSet::new();
        let mut split_categories = BTreeSet::new();
        let mut split_triples = 0usize;
        for (index, ((document, gold), entry)) in
            documents.iter().zip(&oracle).zip(&manifest).enumerate()
        {
            if document.document_id != gold.document_id
                || document.document_id != entry.document_id
                || document.split != split
                || gold.split != split
                || entry.split != split
                || document.source_row_index != index
                || entry.source_row_index != index
                || document.category != entry.category
                || document.webnlg_id != entry.webnlg_id
                || document.char_count != entry.char_count
                || entry.documents_path != format!("documents/{split}.jsonl")
                || entry.oracle_path != format!("oracle/{split}.jsonl")
            {
                bail!("{split} contains a misaligned document/oracle/manifest row");
            }
            if !all_ids.insert(document.document_id.clone()) {
                bail!("duplicate document id {}", document.document_id);
            }
            if document.schema_version != SCHEMA_VERSION || gold.schema_version != SCHEMA_VERSION {
                bail!("{} has an unsupported schema version", document.document_id);
            }
            if document.source_url != "https://huggingface.co/datasets/GEM/web_nlg" {
                bail!("{} has an unexpected source URL", document.document_id);
            }
            if document.char_count != document.text.chars().count()
                || document.text_sha256 != sha256(document.text.as_bytes())
                || document.text_sha256 != entry.text_sha256
            {
                bail!("{} text checksum or length mismatch", document.document_id);
            }
            if gold.triples.is_empty()
                || gold.triples.len() > 7
                || gold.triples.len() != entry.triple_count
                || gold.triples.iter().any(|triple| {
                    triple.subject.is_empty()
                        || triple.predicate.is_empty()
                        || triple.object.is_empty()
                })
            {
                bail!("{} has invalid oracle triples", document.document_id);
            }
            split_predicates.extend(gold.triples.iter().map(|triple| triple.predicate.clone()));
            split_categories.insert(document.category.clone());
            split_triples += gold.triples.len();
            all_predicates.extend(gold.triples.iter().map(|triple| triple.predicate.clone()));
            all_categories.insert(document.category.clone());
            total_triples += gold.triples.len();
        }
        total_documents += documents.len();

        let summary = run
            .splits
            .iter()
            .find(|summary| summary.split == split)
            .with_context(|| format!("run.json is missing {split}"))?;
        if summary.rows != expected_rows
            || summary.triples != split_triples
            || summary.unique_predicates != split_predicates.len()
            || summary.unique_categories != split_categories.len()
            || summary.raw_sha256 != cached_split_sha256(output, split, expected_rows)?
            || summary.documents_sha256 != sha256(&fs::read(&documents_path)?)
            || summary.oracle_sha256 != sha256(&fs::read(&oracle_path)?)
            || summary.manifest_sha256 != sha256(&fs::read(&manifest_path)?)
        {
            bail!("{split} digest or summary differs from run.json");
        }
    }

    if run.total_documents != total_documents
        || run.total_triples != total_triples
        || run.unique_predicates != all_predicates.len()
        || run.unique_categories != all_categories.len()
    {
        bail!("run.json aggregate counts do not match the generated corpus");
    }
    Ok(run)
}

pub fn expected_request() -> RunRequest {
    RunRequest {
        dataset: DATASET.to_string(),
        config: CONFIG.to_string(),
        hub_revision: HUB_REVISION.to_string(),
        splits: SPLITS
            .iter()
            .map(|(split, _)| (*split).to_string())
            .collect(),
        page_size: super::source::PAGE_SIZE,
    }
}

pub fn write_request(output: &Path) -> Result<()> {
    let path = output.join("request.json");
    let expected = expected_request();
    if path.exists() {
        let current: RunRequest = read_json(&path)?;
        if current != expected {
            bail!(
                "{} belongs to a different WebNLG baseline",
                output.display()
            );
        }
    } else {
        write_json_pretty(&path, &expected)?;
    }
    Ok(())
}

pub fn finish_run(output: &Path, splits: Vec<SplitSummary>) -> Result<RunMetadata> {
    let mut predicates = BTreeSet::new();
    let mut categories = BTreeSet::new();
    for (split, _) in SPLITS {
        let oracle: Vec<OracleRecord> =
            read_jsonl(&output.join("oracle").join(format!("{split}.jsonl")))?;
        let documents: Vec<PilotDocument> =
            read_jsonl(&output.join("documents").join(format!("{split}.jsonl")))?;
        predicates.extend(
            oracle
                .iter()
                .flat_map(|record| &record.triples)
                .map(|triple| triple.predicate.clone()),
        );
        categories.extend(documents.iter().map(|document| document.category.clone()));
    }
    let run = RunMetadata {
        schema_version: SCHEMA_VERSION.to_string(),
        dataset: DATASET.to_string(),
        config: CONFIG.to_string(),
        hub_revision: HUB_REVISION.to_string(),
        license: "CC-BY-NC-4.0".to_string(),
        source: "Hugging Face Dataset Viewer API".to_string(),
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        total_documents: splits.iter().map(|summary| summary.rows).sum(),
        total_triples: splits.iter().map(|summary| summary.triples).sum(),
        unique_predicates: predicates.len(),
        unique_categories: categories.len(),
        challenge_splits_excluded: EXCLUDED_CHALLENGE_SPLITS
            .iter()
            .map(|split| (*split).to_string())
            .collect(),
        splits,
    };
    write_json_pretty(&output.join("run.json"), &run)?;
    Ok(run)
}

pub fn parse_triple(raw: &str) -> Result<OracleTriple> {
    let parts: Vec<&str> = raw.split(" | ").collect();
    if parts.len() != 3 || parts.iter().any(|part| part.trim().is_empty()) {
        bail!("invalid WebNLG triple: {raw}");
    }
    Ok(OracleTriple {
        subject: parts[0].trim().to_string(),
        predicate: parts[1].trim().to_string(),
        object: parts[2].trim().to_string(),
    })
}

fn write_json_line(writer: &mut impl Write, value: &impl serde::Serialize) -> Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub fn write_json_pretty(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&fs::read(path)?).with_context(|| format!("parse {}", path.display()))
}

fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    BufReader::new(File::open(path)?)
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Ok(line) if line.trim().is_empty() => None,
            Ok(line) => Some(
                serde_json::from_str(&line)
                    .with_context(|| format!("parse {} line {}", path.display(), index + 1)),
            ),
            Err(error) => Some(Err(error.into())),
        })
        .collect()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entity_and_literal_triples_without_normalizing_the_oracle() {
        assert_eq!(
            parse_triple("Aarhus_Airport | cityServed | \"Aarhus, Denmark\"").unwrap(),
            OracleTriple {
                subject: "Aarhus_Airport".into(),
                predicate: "cityServed".into(),
                object: "\"Aarhus, Denmark\"".into(),
            }
        );
    }

    #[test]
    fn refuses_malformed_triples() {
        assert!(parse_triple("subject | predicate").is_err());
        assert!(parse_triple("subject |  | object").is_err());
    }
}
