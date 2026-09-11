//! Prepare the English WebNLG corpus as text documents plus a physically
//! separate RDF-triple oracle for the Semantic Neurons pilot.

mod model;
mod output;
mod source;

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use output::{compile_split, finish_run, verify_output, write_request};
use source::{SPLITS, assert_hub_revision, client, load_split};

#[derive(Debug)]
struct Config {
    output: PathBuf,
    concurrency: usize,
    retries: usize,
    refresh: bool,
    verify_only: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output: PathBuf::from("data/webnlg-pilot"),
            concurrency: 4,
            retries: 8,
            refresh: false,
            verify_only: false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = parse_args()?;
    if config.verify_only {
        let run = verify_output(&config.output)?;
        print_summary("verified", &config.output, &run);
        return Ok(());
    }

    std::fs::create_dir_all(&config.output)?;
    write_request(&config.output)?;
    let client = client()?;
    assert_hub_revision(&client).await?;
    let mut summaries = Vec::new();
    for (split, expected_rows) in SPLITS {
        eprintln!("fetching {split}: {expected_rows} rows");
        let pages = load_split(
            &client,
            &config.output,
            split,
            expected_rows,
            config.concurrency,
            config.retries,
            config.refresh,
        )
        .await?;
        summaries.push(compile_split(&config.output, split, pages)?);
    }
    let run = finish_run(&config.output, summaries)?;
    verify_output(&config.output)?;
    print_summary("prepared", &config.output, &run);
    Ok(())
}

fn print_summary(action: &str, output: &std::path::Path, run: &model::RunMetadata) {
    println!("{action} WebNLG {} @ {}", run.config, run.hub_revision);
    for split in &run.splits {
        println!(
            "  {:10} {:>6} documents {:>7} triples {:>3} predicates {:>2} categories",
            split.split,
            split.rows,
            split.triples,
            split.unique_predicates,
            split.unique_categories
        );
    }
    println!(
        "  total      {:>6} documents {:>7} triples {:>3} predicates {:>2} categories",
        run.total_documents, run.total_triples, run.unique_predicates, run.unique_categories
    );
    println!("output      {}", output.display());
    println!("license     {} (research/development use)", run.license);
}

fn parse_args() -> Result<Config> {
    let mut config = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output" => {
                config.output = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow!("--output requires a path"))?,
                );
            }
            "--concurrency" => {
                config.concurrency = parse_value(&mut args, "--concurrency")?;
            }
            "--retries" => config.retries = parse_value(&mut args, "--retries")?,
            "--refresh" => config.refresh = true,
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
    Ok(config)
}

fn parse_value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    let raw = args
        .next()
        .ok_or_else(|| anyhow!("{flag} requires a value"))?;
    raw.parse()
        .map_err(|error| anyhow!("invalid value for {flag}: {error}"))
}

fn print_help() {
    println!(
        "WebNLG English pilot corpus generator\n\n\
Usage: cargo run --release -p cognigraph-construct --bin webnlg-pilot -- [OPTIONS]\n\n\
Options:\n  \
  --output DIR          Output directory [data/webnlg-pilot]\n  \
  --concurrency N       Concurrent Dataset Viewer page requests [4]\n  \
  --retries N           Retries after the first request [8]\n  \
  --refresh             Replace cached source pages\n  \
  --verify-only         Verify generated files without network access\n  \
  -h, --help            Print this help"
    );
}
