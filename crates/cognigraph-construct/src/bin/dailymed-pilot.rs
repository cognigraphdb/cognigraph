//! Build a deterministic, resumable DailyMed corpus for the Semantic Neurons pilot.
//!
//! The collector enumerates current human prescription and OTC labels through
//! DailyMed API v2, ranks each population by a seeded SHA-256 digest, downloads
//! current SPL XML with bounded concurrency, and emits normalized JSON documents.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const API_ROOT: &str = "https://dailymed.nlm.nih.gov/dailymed/services/v2";
const RX_DOCTYPE: &str = "34391-3";
const OTC_DOCTYPE: &str = "34390-5";
const PAGE_SIZE: usize = 100;
const DEFAULT_OUTPUT: &str = "data/dailymed-pilot";
const USER_AGENT: &str = concat!("cognigraph-dailymed-pilot/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
struct Config {
    output: PathBuf,
    rx: usize,
    otc: usize,
    seed: String,
    concurrency: usize,
    retries: usize,
    min_sections: usize,
    min_chars: usize,
    refresh_catalog: bool,
    catalog_only: bool,
    verify_only: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output: PathBuf::from(DEFAULT_OUTPUT),
            rx: 9_000,
            otc: 1_000,
            seed: "cognigraph-dailymed-pilot-v1".to_string(),
            concurrency: 8,
            retries: 4,
            min_sections: 2,
            min_chars: 500,
            refresh_catalog: false,
            catalog_only: false,
            verify_only: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LabelKind {
    HumanPrescription,
    HumanOtc,
}

impl LabelKind {
    fn doctype(self) -> &'static str {
        match self {
            Self::HumanPrescription => RX_DOCTYPE,
            Self::HumanOtc => OTC_DOCTYPE,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::HumanPrescription => "rx",
            Self::HumanOtc => "otc",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::HumanPrescription => "HUMAN PRESCRIPTION DRUG LABEL",
            Self::HumanOtc => "HUMAN OTC DRUG LABEL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogEntry {
    setid: String,
    spl_version: u64,
    published_date: String,
    title: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CatalogPage {
    data: Vec<CatalogEntry>,
    metadata: CatalogMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CatalogMetadata {
    db_published_date: String,
    total_elements: usize,
    total_pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Section {
    code: Option<String>,
    title: String,
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PilotDocument {
    document_id: String,
    set_id: String,
    spl_version: u64,
    published_date: String,
    title: String,
    label_kind: LabelKind,
    doctype_code: String,
    source_url: String,
    retrieved_at: String,
    raw_sha256: String,
    text_sha256: String,
    char_count: usize,
    sections: Vec<Section>,
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestEntry {
    document_id: String,
    set_id: String,
    spl_version: u64,
    published_date: String,
    title: String,
    label_kind: LabelKind,
    doctype_code: String,
    source_url: String,
    retrieved_at: String,
    raw_sha256: String,
    text_sha256: String,
    char_count: usize,
    section_count: usize,
    document_path: String,
    raw_path: String,
    selection_rank: usize,
}

#[derive(Debug, Serialize)]
struct Rejection {
    set_id: String,
    label_kind: LabelKind,
    selection_rank: usize,
    reason: String,
}

#[derive(Debug, Serialize)]
struct RunMetadata<'a> {
    dataset: &'static str,
    generated_at: String,
    api_root: &'static str,
    seed: &'a str,
    requested_rx: usize,
    requested_otc: usize,
    accepted_rx: usize,
    accepted_otc: usize,
    min_sections: usize,
    min_chars: usize,
    catalog_dates: BTreeMap<&'static str, String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RunRequest {
    seed: String,
    requested_rx: usize,
    requested_otc: usize,
    min_sections: usize,
    min_chars: usize,
    rx_catalog_date: String,
    otc_catalog_date: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = parse_args()?;
    if config.verify_only {
        verify_output(&config.output)?;
        return Ok(());
    }
    prepare_output(&config.output).await?;
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(120))
        .build()?;

    let (rx_catalog, rx_date) =
        load_or_fetch_catalog(&client, &config, LabelKind::HumanPrescription).await?;
    let (otc_catalog, otc_date) =
        load_or_fetch_catalog(&client, &config, LabelKind::HumanOtc).await?;

    eprintln!(
        "catalogs: {} prescription, {} OTC labels (DailyMed snapshot RX={rx_date}, OTC={otc_date})",
        rx_catalog.len(),
        otc_catalog.len()
    );
    if config.catalog_only {
        eprintln!("catalog-only run complete: {}", config.output.display());
        return Ok(());
    }

    ensure_run_request(
        &config.output,
        &RunRequest {
            seed: config.seed.clone(),
            requested_rx: config.rx,
            requested_otc: config.otc,
            min_sections: config.min_sections,
            min_chars: config.min_chars,
            rx_catalog_date: rx_date.clone(),
            otc_catalog_date: otc_date.clone(),
        },
    )?;

    let rx_ranked = rank_catalog(rx_catalog, &config.seed, LabelKind::HumanPrescription);
    let otc_ranked = rank_catalog(otc_catalog, &config.seed, LabelKind::HumanOtc);
    let rx_docs = collect_kind(
        &client,
        &config,
        LabelKind::HumanPrescription,
        &rx_ranked,
        config.rx,
    )
    .await?;
    let otc_docs = collect_kind(
        &client,
        &config,
        LabelKind::HumanOtc,
        &otc_ranked,
        config.otc,
    )
    .await?;

    let mut metadata = BTreeMap::new();
    metadata.insert("human_prescription", rx_date);
    metadata.insert("human_otc", otc_date);
    write_json_pretty(
        &config.output.join("run.json"),
        &RunMetadata {
            dataset: "DailyMed current human drug labels",
            generated_at: now(),
            api_root: API_ROOT,
            seed: &config.seed,
            requested_rx: config.rx,
            requested_otc: config.otc,
            accepted_rx: rx_docs,
            accepted_otc: otc_docs,
            min_sections: config.min_sections,
            min_chars: config.min_chars,
            catalog_dates: metadata,
        },
    )?;
    eprintln!(
        "complete: {} documents in {}",
        rx_docs + otc_docs,
        config.output.display()
    );
    Ok(())
}

fn parse_args() -> Result<Config> {
    let mut config = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => config.output = PathBuf::from(value(&mut args, "--output")?),
            "--rx" => config.rx = parse_value(&mut args, "--rx")?,
            "--otc" => config.otc = parse_value(&mut args, "--otc")?,
            "--seed" => config.seed = value(&mut args, "--seed")?,
            "--concurrency" => config.concurrency = parse_value(&mut args, "--concurrency")?,
            "--retries" => config.retries = parse_value(&mut args, "--retries")?,
            "--min-sections" => config.min_sections = parse_value(&mut args, "--min-sections")?,
            "--min-chars" => config.min_chars = parse_value(&mut args, "--min-chars")?,
            "--refresh-catalog" => config.refresh_catalog = true,
            "--catalog-only" => config.catalog_only = true,
            "--verify-only" => config.verify_only = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            unknown => bail!("unknown argument '{unknown}'; use --help"),
        }
    }
    if config.concurrency == 0 {
        bail!("--concurrency must be greater than zero");
    }
    if config.rx + config.otc == 0 && !config.catalog_only && !config.verify_only {
        bail!("at least one of --rx or --otc must be greater than zero");
    }
    Ok(config)
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| anyhow!("{flag} requires a value"))
}

fn parse_value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    let raw = value(args, flag)?;
    raw.parse()
        .map_err(|error| anyhow!("invalid value for {flag}: {error}"))
}

fn print_help() {
    println!(
        "DailyMed 10k pilot corpus generator\n\n\
Usage: cargo run --release -p cognigraph-construct --bin dailymed-pilot -- [OPTIONS]\n\n\
Options:\n  \
  --output DIR          Output directory [{DEFAULT_OUTPUT}]\n  \
  --rx N                Accepted prescription labels [9000]\n  \
  --otc N               Accepted OTC labels [1000]\n  \
  --seed TEXT           Deterministic selection seed\n  \
  --concurrency N       Maximum concurrent requests [8]\n  \
  --retries N           Retries after the first request [4]\n  \
  --min-sections N      Minimum non-empty narrative sections [2]\n  \
  --min-chars N         Minimum normalized document characters [500]\n  \
  --refresh-catalog     Replace cached DailyMed catalogs\n  \
  --catalog-only        Fetch catalogs without label documents\n  \
  --verify-only         Verify an existing output without network access\n  \
  -h, --help            Print this help"
    );
}

async fn prepare_output(output: &Path) -> Result<()> {
    for child in ["catalog", "raw", "documents"] {
        tokio::fs::create_dir_all(output.join(child)).await?;
    }
    Ok(())
}

async fn load_or_fetch_catalog(
    client: &Client,
    config: &Config,
    kind: LabelKind,
) -> Result<(Vec<CatalogEntry>, String)> {
    let path = config
        .output
        .join("catalog")
        .join(format!("{}.json", kind.slug()));
    if path.exists() && !config.refresh_catalog {
        let bytes = tokio::fs::read(&path).await?;
        let cached: CatalogSnapshot = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse cached catalog {}", path.display()))?;
        return Ok((cached.entries, cached.db_published_date));
    }

    let pages_dir = config
        .output
        .join("catalog")
        .join(format!("pages-{}", kind.slug()));
    if config.refresh_catalog && pages_dir.exists() {
        tokio::fs::remove_dir_all(&pages_dir).await?;
    }
    tokio::fs::create_dir_all(&pages_dir).await?;
    let first =
        load_or_fetch_catalog_page(client, kind, 1, config.retries, &pages_dir, None).await?;
    if first.metadata.total_elements < requested_for_kind(config, kind) {
        bail!(
            "DailyMed reports only {} {} labels, fewer than requested",
            first.metadata.total_elements,
            kind.slug()
        );
    }
    let expected_date = first.metadata.db_published_date.clone();
    let expected_elements = first.metadata.total_elements;
    let total_pages = first.metadata.total_pages;
    let semaphore = Arc::new(Semaphore::new(config.concurrency));
    let mut tasks = JoinSet::new();
    for page in 2..=total_pages {
        let client = client.clone();
        let semaphore = semaphore.clone();
        let pages_dir = pages_dir.clone();
        let expected_date = expected_date.clone();
        let retries = config.retries;
        tasks.spawn(async move {
            let _permit = semaphore.acquire_owned().await?;
            load_or_fetch_catalog_page(
                &client,
                kind,
                page,
                retries,
                &pages_dir,
                Some(&expected_date),
            )
            .await
        });
    }

    let mut entries = first.data;
    while let Some(result) = tasks.join_next().await {
        let page = result.context("catalog request task failed")??;
        if page.metadata.db_published_date != expected_date {
            bail!(
                "DailyMed catalog changed during enumeration ({} -> {}); rerun with --refresh-catalog",
                expected_date,
                page.metadata.db_published_date
            );
        }
        entries.extend(page.data);
    }
    entries.sort_by(|a, b| a.setid.cmp(&b.setid));
    entries.dedup_by(|a, b| a.setid == b.setid);
    if entries.len() != expected_elements {
        bail!(
            "catalog count mismatch for {}: expected {}, got {} unique entries",
            kind.slug(),
            expected_elements,
            entries.len()
        );
    }
    let snapshot = CatalogSnapshot {
        db_published_date: expected_date.clone(),
        doctype_code: kind.doctype().to_string(),
        entries: entries.clone(),
    };
    write_json_pretty(&path, &snapshot)?;
    Ok((entries, expected_date))
}

fn requested_for_kind(config: &Config, kind: LabelKind) -> usize {
    match kind {
        LabelKind::HumanPrescription => config.rx,
        LabelKind::HumanOtc => config.otc,
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CatalogSnapshot {
    db_published_date: String,
    doctype_code: String,
    entries: Vec<CatalogEntry>,
}

async fn fetch_catalog_page(
    client: &Client,
    kind: LabelKind,
    page: usize,
    retries: usize,
) -> Result<CatalogPage> {
    let url = format!(
        "{API_ROOT}/spls.json?doctype={}&pagesize={PAGE_SIZE}&page={page}",
        kind.doctype()
    );
    let bytes = fetch_bytes(client, &url, retries).await?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse catalog page {page}"))
}

async fn load_or_fetch_catalog_page(
    client: &Client,
    kind: LabelKind,
    page: usize,
    retries: usize,
    pages_dir: &Path,
    expected_date: Option<&str>,
) -> Result<CatalogPage> {
    let path = pages_dir.join(format!("{page:05}.json"));
    if path.exists() {
        let bytes = tokio::fs::read(&path).await?;
        let cached: CatalogPage = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse cached catalog page {}", path.display()))?;
        if expected_date.is_none_or(|date| cached.metadata.db_published_date == date) {
            return Ok(cached);
        }
        bail!(
            "cached catalog page {} belongs to a different DailyMed snapshot; use --refresh-catalog",
            path.display()
        );
    }
    let fetched = fetch_catalog_page(client, kind, page, retries).await?;
    if let Some(expected) = expected_date
        && fetched.metadata.db_published_date != expected
    {
        bail!(
            "DailyMed catalog changed during enumeration ({} -> {}); rerun with --refresh-catalog",
            expected,
            fetched.metadata.db_published_date
        );
    }
    write_json_pretty(&path, &fetched)?;
    Ok(fetched)
}

fn rank_catalog(entries: Vec<CatalogEntry>, seed: &str, kind: LabelKind) -> Vec<CatalogEntry> {
    let mut ranked: Vec<_> = entries
        .into_iter()
        .map(|entry| {
            let digest = hash_bytes(format!("{seed}\0{}\0{}", kind.slug(), entry.setid).as_bytes());
            (digest, entry)
        })
        .collect();
    ranked.sort_by(|(hash_a, entry_a), (hash_b, entry_b)| {
        hash_a
            .cmp(hash_b)
            .then_with(|| entry_a.setid.cmp(&entry_b.setid))
    });
    ranked.into_iter().map(|(_, entry)| entry).collect()
}

async fn collect_kind(
    client: &Client,
    config: &Config,
    kind: LabelKind,
    ranked: &[CatalogEntry],
    target: usize,
) -> Result<usize> {
    if target == 0 {
        return Ok(0);
    }
    let mut accepted = 0;
    let mut cursor = 0;
    let batch_size = config.concurrency * 2;
    let manifest_path = config
        .output
        .join(format!("manifest-{}.jsonl", kind.slug()));
    let rejection_path = config
        .output
        .join(format!("rejections-{}.jsonl", kind.slug()));
    let mut manifest = std::fs::File::create(&manifest_path)?;
    let mut rejections = std::fs::File::create(&rejection_path)?;

    while accepted < target && cursor < ranked.len() {
        let end = (cursor + batch_size).min(ranked.len());
        let mut tasks = JoinSet::new();
        for (offset, entry) in ranked[cursor..end].iter().cloned().enumerate() {
            let client = client.clone();
            let output = config.output.clone();
            let retries = config.retries;
            tasks.spawn(async move {
                let rank = cursor + offset + 1;
                let result = fetch_and_parse(&client, &output, kind, &entry, retries).await;
                (rank, entry, result)
            });
        }
        let mut batch = Vec::with_capacity(end - cursor);
        while let Some(result) = tasks.join_next().await {
            batch.push(result.context("label request task failed")?);
        }
        batch.sort_by_key(|(rank, _, _)| *rank);

        for (rank, entry, result) in batch {
            if accepted == target {
                break;
            }
            match result {
                Ok(document)
                    if document.sections.len() >= config.min_sections
                        && document.char_count >= config.min_chars =>
                {
                    let document_rel = format!("documents/{}.json", entry.setid);
                    let raw_rel = format!("raw/{}.xml", entry.setid);
                    write_json_pretty(&config.output.join(&document_rel), &document)?;
                    write_json_line(
                        &mut manifest,
                        &ManifestEntry {
                            document_id: document.document_id.clone(),
                            set_id: document.set_id.clone(),
                            spl_version: document.spl_version,
                            published_date: document.published_date.clone(),
                            title: document.title.clone(),
                            label_kind: document.label_kind,
                            doctype_code: document.doctype_code.clone(),
                            source_url: document.source_url.clone(),
                            retrieved_at: document.retrieved_at.clone(),
                            raw_sha256: document.raw_sha256.clone(),
                            text_sha256: document.text_sha256.clone(),
                            char_count: document.char_count,
                            section_count: document.sections.len(),
                            document_path: document_rel,
                            raw_path: raw_rel,
                            selection_rank: rank,
                        },
                    )?;
                    accepted += 1;
                    if accepted % 100 == 0 || accepted == target {
                        eprintln!(
                            "{}: accepted {accepted}/{target} (rank {rank})",
                            kind.slug()
                        );
                    }
                }
                Ok(document) => write_json_line(
                    &mut rejections,
                    &Rejection {
                        set_id: entry.setid,
                        label_kind: kind,
                        selection_rank: rank,
                        reason: format!(
                            "quality threshold: {} sections, {} chars",
                            document.sections.len(),
                            document.char_count
                        ),
                    },
                )?,
                Err(error) if error.to_string().starts_with("snapshot mismatch:") => {
                    return Err(error);
                }
                Err(error) => write_json_line(
                    &mut rejections,
                    &Rejection {
                        set_id: entry.setid,
                        label_kind: kind,
                        selection_rank: rank,
                        reason: format!("download or parse: {error:#}"),
                    },
                )?,
            }
        }
        cursor = end;
    }
    if accepted != target {
        bail!(
            "exhausted {} catalog after accepting {accepted}/{target} labels",
            kind.slug()
        );
    }
    Ok(accepted)
}

async fn fetch_and_parse(
    client: &Client,
    output: &Path,
    kind: LabelKind,
    entry: &CatalogEntry,
    retries: usize,
) -> Result<PilotDocument> {
    let raw_path = output.join("raw").join(format!("{}.xml", entry.setid));
    let source_url = format!("{API_ROOT}/spls/{}.xml", entry.setid);
    let raw = if raw_path.exists() {
        tokio::fs::read(&raw_path).await?
    } else {
        let bytes = fetch_bytes(client, &source_url, retries).await?;
        let temporary = raw_path.with_extension("xml.part");
        tokio::fs::write(&temporary, &bytes).await?;
        tokio::fs::rename(&temporary, &raw_path).await?;
        bytes
    };
    parse_spl(&raw, kind, entry, source_url)
}

async fn fetch_bytes(client: &Client, url: &str, retries: usize) -> Result<Vec<u8>> {
    let mut last_error = None;
    for attempt in 0..=retries {
        match client.get(url).send().await {
            Ok(response) if response.status().is_success() => {
                return Ok(response.bytes().await?.to_vec());
            }
            Ok(response) => {
                last_error = Some(anyhow!("HTTP {} from {url}", response.status()));
            }
            Err(error) => last_error = Some(error.into()),
        }
        if attempt < retries {
            let backoff_ms = 250_u64.saturating_mul(1_u64 << attempt.min(6));
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("request failed: {url}")))
}

fn parse_spl(
    raw: &[u8],
    expected_kind: LabelKind,
    entry: &CatalogEntry,
    source_url: String,
) -> Result<PilotDocument> {
    let mut reader = Reader::from_reader(raw);
    reader.config_mut().trim_text(true);
    let mut sections = Vec::new();
    let mut current: Option<SectionBuilder> = None;
    let mut section_depth = 0_usize;
    let mut title_depth = 0_usize;
    let mut text_depth = 0_usize;
    let mut observed_doctype = None;
    let mut observed_set_id = None;
    let mut observed_version = None;

    loop {
        match reader.read_event()? {
            Event::Start(event) => {
                let name = event.local_name();
                match name.as_ref() {
                    "section" => {
                        section_depth += 1;
                        if section_depth == 1 {
                            current = Some(SectionBuilder::default());
                        }
                    }
                    "title" if section_depth == 1 => title_depth += 1,
                    "text" if section_depth == 1 => text_depth += 1,
                    "code" => {
                        inspect_code(&event, section_depth, &mut observed_doctype, &mut current)?;
                    }
                    "setId" if observed_set_id.is_none() => {
                        observed_set_id = attribute(&event, "root")?;
                    }
                    "versionNumber" if observed_version.is_none() => {
                        observed_version = attribute(&event, "value")?
                            .map(|value| value.parse())
                            .transpose()
                            .context("parse SPL versionNumber")?;
                    }
                    _ => {}
                }
            }
            Event::Empty(event) => match event.local_name().as_ref() {
                "code" => {
                    inspect_code(&event, section_depth, &mut observed_doctype, &mut current)?;
                }
                "setId" if observed_set_id.is_none() => {
                    observed_set_id = attribute(&event, "root")?;
                }
                "versionNumber" if observed_version.is_none() => {
                    observed_version = attribute(&event, "value")?
                        .map(|value| value.parse())
                        .transpose()
                        .context("parse SPL versionNumber")?;
                }
                _ => {}
            },
            Event::Text(text) => {
                if let Some(section) = &mut current {
                    let decoded = unescape(&text.xml10_content())?.into_owned();
                    if title_depth > 0 {
                        push_text(&mut section.title, &decoded);
                    }
                    if text_depth > 0 {
                        push_text(&mut section.text, &decoded);
                    }
                }
            }
            Event::CData(text) => {
                if let Some(section) = &mut current {
                    let decoded = text.into_inner().into_owned();
                    if title_depth > 0 {
                        push_text(&mut section.title, &decoded);
                    }
                    if text_depth > 0 {
                        push_text(&mut section.text, &decoded);
                    }
                }
            }
            Event::End(event) => match event.local_name().as_ref() {
                "title" if section_depth == 1 => title_depth = title_depth.saturating_sub(1),
                "text" if section_depth == 1 => text_depth = text_depth.saturating_sub(1),
                "section" => {
                    if section_depth == 1
                        && let Some(section) = current.take().and_then(SectionBuilder::finish)
                    {
                        sections.push(section);
                    }
                    section_depth = section_depth.saturating_sub(1);
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }

    let observed = observed_doctype.ok_or_else(|| anyhow!("missing human-drug doctype"))?;
    if observed != expected_kind {
        bail!(
            "doctype mismatch: catalog requested {}, SPL declared {}",
            expected_kind.display_name(),
            observed.display_name()
        );
    }
    let set_id_matches = observed_set_id
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(&entry.setid));
    if !set_id_matches || observed_version != Some(entry.spl_version) {
        bail!(
            "snapshot mismatch: catalog has set ID {} version {}, XML has set ID {:?} version {:?}; create a fresh baseline",
            entry.setid,
            entry.spl_version,
            observed_set_id,
            observed_version
        );
    }
    let mut text = String::new();
    for section in &sections {
        if !section.title.is_empty() {
            push_text(&mut text, &section.title);
        }
        push_text(&mut text, &section.text);
        text.push_str("\n\n");
    }
    let text = text.trim().to_string();
    let raw_sha256 = hex_hash(raw);
    let text_sha256 = hex_hash(text.as_bytes());
    Ok(PilotDocument {
        document_id: format!("dailymed:{}", entry.setid),
        set_id: entry.setid.clone(),
        spl_version: entry.spl_version,
        published_date: entry.published_date.clone(),
        title: entry.title.clone(),
        label_kind: expected_kind,
        doctype_code: expected_kind.doctype().to_string(),
        source_url,
        retrieved_at: now(),
        raw_sha256,
        text_sha256,
        char_count: text.chars().count(),
        sections,
        text,
    })
}

#[derive(Default)]
struct SectionBuilder {
    code: Option<String>,
    title: String,
    text: String,
}

impl SectionBuilder {
    fn finish(self) -> Option<Section> {
        let title = collapse_whitespace(&self.title);
        let text = collapse_whitespace(&self.text);
        (!text.is_empty()).then_some(Section {
            code: self.code,
            title,
            text,
        })
    }
}

fn inspect_code(
    event: &BytesStart<'_>,
    section_depth: usize,
    observed_doctype: &mut Option<LabelKind>,
    current: &mut Option<SectionBuilder>,
) -> Result<()> {
    let code = attribute(event, "code")?;
    let display_name = attribute(event, "displayName")?;
    match display_name.as_deref() {
        Some("HUMAN PRESCRIPTION DRUG LABEL") => {
            *observed_doctype = Some(LabelKind::HumanPrescription);
        }
        Some("HUMAN OTC DRUG LABEL") => *observed_doctype = Some(LabelKind::HumanOtc),
        _ => {}
    }
    if section_depth == 1
        && let Some(section) = current
        && section.code.is_none()
    {
        section.code = code;
    }
    Ok(())
}

fn attribute(event: &BytesStart<'_>, name: &str) -> Result<Option<String>> {
    for attribute in event.attributes() {
        let attribute = attribute?;
        if attribute.key.local_name().as_ref() == name {
            return Ok(Some(
                attribute
                    .normalized_value(XmlVersion::default())?
                    .into_owned(),
            ));
        }
    }
    Ok(None)
}

fn push_text(target: &mut String, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if !target.is_empty() && !target.chars().last().is_some_and(char::is_whitespace) {
        target.push(' ');
    }
    target.push_str(value);
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn hash_bytes(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

fn hex_hash(value: &[u8]) -> String {
    hash_bytes(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn write_json_pretty(path: &Path, value: &impl Serialize) -> Result<()> {
    let temporary = path.with_extension("json.part");
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn ensure_run_request(output: &Path, request: &RunRequest) -> Result<()> {
    let path = output.join("request.json");
    if path.exists() {
        let existing: RunRequest = serde_json::from_slice(&std::fs::read(&path)?)?;
        if existing != *request {
            bail!(
                "output {} belongs to a different pilot request; use a new --output directory",
                output.display()
            );
        }
        return Ok(());
    }
    write_json_pretty(&path, request)
}

fn write_json_line(output: &mut impl std::io::Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

fn verify_output(output: &Path) -> Result<()> {
    let mut seen = HashSet::new();
    let mut verified = 0_usize;
    for kind in [LabelKind::HumanPrescription, LabelKind::HumanOtc] {
        let manifest_path = output.join(format!("manifest-{}.jsonl", kind.slug()));
        let manifest = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?;
        for (index, line) in manifest.lines().enumerate() {
            let entry: ManifestEntry = serde_json::from_str(line)
                .with_context(|| format!("parse {} line {}", manifest_path.display(), index + 1))?;
            if entry.label_kind != kind {
                bail!("wrong label kind for {}", entry.set_id);
            }
            if !seen.insert(entry.set_id.clone()) {
                bail!("duplicate Set ID in manifests: {}", entry.set_id);
            }
            let raw_path = output.join(&entry.raw_path);
            let raw =
                std::fs::read(&raw_path).with_context(|| format!("read {}", raw_path.display()))?;
            if hex_hash(&raw) != entry.raw_sha256 {
                bail!("raw checksum mismatch for {}", entry.set_id);
            }
            let document_path = output.join(&entry.document_path);
            let document: PilotDocument = serde_json::from_slice(
                &std::fs::read(&document_path)
                    .with_context(|| format!("read {}", document_path.display()))?,
            )
            .with_context(|| format!("parse {}", document_path.display()))?;
            if document.set_id != entry.set_id
                || document.spl_version != entry.spl_version
                || document.label_kind != entry.label_kind
                || document.sections.len() != entry.section_count
                || document.text.chars().count() != entry.char_count
                || hex_hash(document.text.as_bytes()) != entry.text_sha256
                || document.raw_sha256 != entry.raw_sha256
            {
                bail!("document/manifest mismatch for {}", entry.set_id);
            }
            verified += 1;
        }
    }
    let document_files = std::fs::read_dir(output.join("documents"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == "json")
        })
        .count();
    if document_files != verified {
        bail!("document file count mismatch: {document_files} files, {verified} manifest entries");
    }
    eprintln!("verified {verified} documents in {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RX_SPL: &[u8] = br#"<?xml version="1.0"?>
<document xmlns="urn:hl7-org:v3">
  <code code="34391-3" displayName="HUMAN PRESCRIPTION DRUG LABEL"/>
  <setId root="TEST-SET-ID"/>
  <versionNumber value="3"/>
  <section>
    <code code="34067-9" displayName="INDICATIONS AND USAGE SECTION"/>
    <title>1 INDICATIONS &amp; USAGE</title>
    <text><paragraph>Alpha is indicated for condition beta.</paragraph>
      <table><tbody><tr><td>Structured evidence</td></tr></tbody></table>
    </text>
  </section>
  <section><title>2 WARNINGS</title><text>Not indicated for gamma.</text></section>
  <document><setId root="nested-reference"/><versionNumber value="99"/></document>
</document>"#;

    fn entry() -> CatalogEntry {
        CatalogEntry {
            setid: "test-set-id".to_string(),
            spl_version: 3,
            published_date: "Jul 10, 2026".to_string(),
            title: "TEST LABEL [EXAMPLE]".to_string(),
        }
    }

    #[test]
    fn parses_sections_and_preserves_negation() {
        let doc = parse_spl(
            RX_SPL,
            LabelKind::HumanPrescription,
            &entry(),
            "https://example.test/test.xml".to_string(),
        )
        .unwrap();
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].code.as_deref(), Some("34067-9"));
        assert!(doc.text.contains("Structured evidence"));
        assert!(doc.text.contains("Not indicated for gamma."));
        assert_eq!(doc.doctype_code, RX_DOCTYPE);
    }

    #[test]
    fn preserves_utf8_cdata_and_normalized_attributes() {
        let xml = String::from_utf8(RX_SPL.to_vec())
            .unwrap()
            .replace(
                "Alpha is indicated for condition beta.",
                "Café is indicated for condition β.",
            )
            .replace(
                "Not indicated for gamma.",
                "<![CDATA[Not indicated for γ < 2.]]>",
            );
        let doc = parse_spl(
            xml.as_bytes(),
            LabelKind::HumanPrescription,
            &entry(),
            "https://example.test/unicode.xml".into(),
        )
        .unwrap();
        assert!(doc.text.contains("Café is indicated for condition β."));
        assert!(doc.text.contains("Not indicated for γ < 2."));
        let event = BytesStart::from_content("code displayName=\"A &amp; B\"", 4);
        assert_eq!(
            attribute(&event, "displayName").unwrap().as_deref(),
            Some("A & B")
        );
    }

    #[test]
    fn rejects_malformed_and_non_utf8_xml() {
        let invalid = String::from_utf8(RX_SPL.to_vec())
            .unwrap()
            .replace("</paragraph>", "</broken>");
        let mut non_utf8 = RX_SPL.to_vec();
        let offset = non_utf8
            .windows(5)
            .position(|part| part == b"Alpha")
            .unwrap();
        non_utf8[offset] = 0xff;
        for raw in [invalid.as_bytes(), non_utf8.as_slice()] {
            assert!(
                parse_spl(
                    raw,
                    LabelKind::HumanPrescription,
                    &entry(),
                    "https://example.test/invalid.xml".into()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_a_doctype_mismatch() {
        let error = parse_spl(
            RX_SPL,
            LabelKind::HumanOtc,
            &entry(),
            "https://example.test/test.xml".to_string(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("doctype mismatch"));
    }

    #[test]
    fn seeded_ranking_is_repeatable_and_kind_specific() {
        let entries = vec![
            CatalogEntry {
                setid: "a".into(),
                ..entry()
            },
            CatalogEntry {
                setid: "b".into(),
                ..entry()
            },
            CatalogEntry {
                setid: "c".into(),
                ..entry()
            },
        ];
        let first = rank_catalog(entries.clone(), "seed", LabelKind::HumanPrescription);
        let second = rank_catalog(entries.clone(), "seed", LabelKind::HumanPrescription);
        let otc = rank_catalog(entries, "seed", LabelKind::HumanOtc);
        let ids = |values: &[CatalogEntry]| {
            values
                .iter()
                .map(|value| value.setid.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&first), ids(&second));
        assert_ne!(ids(&first), ids(&otc));
    }
}
