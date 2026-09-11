use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use super::model::ViewerResponse;

pub const DATASET: &str = "GEM/web_nlg";
pub const CONFIG: &str = "en";
pub const HUB_REVISION: &str = "1d41f28b06efb62d39cc83a0c00b231e825720fe";
pub const PAGE_SIZE: usize = 100;
pub const SPLITS: [(&str, usize); 3] = [("train", 35_426), ("validation", 1_667), ("test", 1_779)];
pub const EXCLUDED_CHALLENGE_SPLITS: [&str; 4] = [
    "challenge_train_sample",
    "challenge_validation_sample",
    "challenge_test_scramble",
    "challenge_test_numbers",
];

#[derive(Debug, Deserialize)]
struct HubDataset {
    sha: String,
}

pub fn client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!(
            "cognigraph-webnlg-pilot/",
            env!("CARGO_PKG_VERSION")
        ))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(120))
        .build()
        .context("build WebNLG HTTP client")
}

pub async fn assert_hub_revision(client: &Client) -> Result<()> {
    let metadata: HubDataset = client
        .get("https://huggingface.co/api/datasets/GEM/web_nlg")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if metadata.sha != HUB_REVISION {
        bail!(
            "GEM/web_nlg moved from pinned revision {HUB_REVISION} to {}; review the dataset change before creating a new baseline",
            metadata.sha
        );
    }
    Ok(())
}

pub async fn load_split(
    client: &Client,
    output: &Path,
    split: &str,
    expected_rows: usize,
    concurrency: usize,
    retries: usize,
    refresh: bool,
) -> Result<Vec<ViewerResponse>> {
    let raw_dir = output.join("raw").join(split);
    if refresh && raw_dir.exists() {
        tokio::fs::remove_dir_all(&raw_dir).await?;
    }
    tokio::fs::create_dir_all(&raw_dir).await?;

    let offsets: Vec<usize> = (0..expected_rows).step_by(PAGE_SIZE).collect();
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut tasks = JoinSet::new();
    for offset in offsets {
        let permit = semaphore.clone().acquire_owned().await?;
        let client = client.clone();
        let path = page_path(&raw_dir, offset);
        let split = split.to_string();
        tasks.spawn(async move {
            let _permit = permit;
            let page = load_page(&client, &split, offset, retries, &path).await?;
            Ok::<_, anyhow::Error>((offset, page))
        });
    }

    let mut pages = Vec::new();
    while let Some(result) = tasks.join_next().await {
        pages.push(result??);
    }
    pages.sort_by_key(|(offset, _)| *offset);

    let responses: Vec<ViewerResponse> = pages.into_iter().map(|(_, page)| page).collect();
    validate_pages(split, expected_rows, &responses)?;
    Ok(responses)
}

pub fn cached_split_sha256(output: &Path, split: &str, expected_rows: usize) -> Result<String> {
    let raw_dir = output.join("raw").join(split);
    let mut hasher = Sha256::new();
    let mut pages = Vec::new();
    for offset in (0..expected_rows).step_by(PAGE_SIZE) {
        let path = page_path(&raw_dir, offset);
        let bytes =
            std::fs::read(&path).with_context(|| format!("read cached page {}", path.display()))?;
        let page: ViewerResponse = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse cached page {}", path.display()))?;
        hasher.update((offset as u64).to_le_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
        pages.push(page);
    }
    validate_pages(split, expected_rows, &pages)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn page_path(raw_dir: &Path, offset: usize) -> PathBuf {
    raw_dir.join(format!("page-{offset:06}.json"))
}

async fn load_page(
    client: &Client,
    split: &str,
    offset: usize,
    retries: usize,
    path: &Path,
) -> Result<ViewerResponse> {
    if path.exists() {
        let bytes = tokio::fs::read(path).await?;
        return serde_json::from_slice(&bytes)
            .with_context(|| format!("parse cached page {}", path.display()));
    }

    let url = format!(
        "https://datasets-server.huggingface.co/rows?dataset=GEM%2Fweb_nlg&config={CONFIG}&split={split}&offset={offset}&length={PAGE_SIZE}"
    );
    let mut last_error = None;
    for attempt in 0..=retries {
        match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status();
                match response.error_for_status() {
                    Ok(response) => {
                        let bytes = response.bytes().await?;
                        let page: ViewerResponse = serde_json::from_slice(&bytes)
                            .with_context(|| format!("parse viewer page {split}@{offset}"))?;
                        let temporary = path.with_extension("json.tmp");
                        tokio::fs::write(&temporary, &bytes).await?;
                        tokio::fs::rename(&temporary, path).await?;
                        return Ok(page);
                    }
                    Err(error) => last_error = Some(anyhow!(error)),
                }
                if attempt < retries && status == StatusCode::TOO_MANY_REQUESTS {
                    let delay = rate_limit_delay(attempt);
                    eprintln!(
                        "rate limited fetching {split}@{offset}; retrying in {}s ({}/{})",
                        delay.as_secs(),
                        attempt + 1,
                        retries
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
            }
            Err(error) => last_error = Some(anyhow!(error)),
        }
        if attempt < retries {
            tokio::time::sleep(Duration::from_secs(attempt as u64 + 1)).await;
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("failed to fetch {split}@{offset}")))
}

fn rate_limit_delay(attempt: usize) -> Duration {
    let multiplier = 1u64.checked_shl(attempt.min(5) as u32).unwrap_or(32);
    Duration::from_secs((5 * multiplier).min(120))
}

fn validate_pages(split: &str, expected_rows: usize, pages: &[ViewerResponse]) -> Result<()> {
    let mut next_index = 0usize;
    for page in pages {
        if page.partial {
            bail!("Dataset Viewer returned a partial page for {split}");
        }
        if page.num_rows_total != expected_rows {
            bail!(
                "{split} row count changed: expected {expected_rows}, viewer reports {}",
                page.num_rows_total
            );
        }
        for row in &page.rows {
            if row.row_idx != next_index {
                bail!(
                    "{split} row sequence broke: expected index {next_index}, found {}",
                    row.row_idx
                );
            }
            if !row.truncated_cells.is_empty() {
                bail!("{split} row {} contains truncated cells", row.row_idx);
            }
            next_index += 1;
        }
    }
    if next_index != expected_rows {
        bail!("{split} contains {next_index} rows, expected {expected_rows}");
    }
    Ok(())
}
