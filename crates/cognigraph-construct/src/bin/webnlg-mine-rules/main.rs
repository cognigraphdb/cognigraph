//! Mine the WebNLG `neuron-authored` candidate rule set from the authoring
//! corpus (train+validation) and write it as a frozen JSON artifact.
//!
//! This is the "propose" half of the neuron-authored lane: it emits a
//! reviewable candidate rule set with per-predicate trigger templates mined from
//! how the training texts actually lexicalize each predicate. A human prunes the
//! artifact before it is treated as the reviewed neuron-authored rules. It never
//! reads the `test` split (W7) — `Corpus::Authoring` restricts I/O to
//! train+validation.
//!
//! Usage:
//!   cargo run --release -p cognigraph-construct --bin webnlg-mine-rules -- \
//!     [--root data/webnlg-pilot] [--out <path>] \
//!     [--min-count 2] [--max-per-predicate 8]

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use cognigraph_construct::webnlg::mining::{MineOpts, mine_ruleset};
use cognigraph_construct::webnlg::{Corpus, load_corpus};
use sha2::{Digest, Sha256};

struct Config {
    root: PathBuf,
    out: PathBuf,
    opts: MineOpts,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: PathBuf::from("data/webnlg-pilot"),
            out: PathBuf::from(
                "crates/cognigraph-construct/fixtures/webnlg/neuron-ruleset.mined.json",
            ),
            opts: MineOpts::default(),
        }
    }
}

fn main() -> Result<()> {
    let config = parse_args()?;

    // W7: authoring corpus only — train + validation, never test.
    let (documents, oracle) = load_corpus(&config.root, Corpus::Authoring)
        .with_context(|| format!("loading authoring corpus from {:?}", config.root))?;

    let ruleset = mine_ruleset(
        "neuron-authored-mined-v3",
        &documents,
        &oracle,
        &config.opts,
    );

    // Pretty JSON with a trailing newline so the committed artifact is a clean,
    // reviewable diff.
    let mut json = serde_json::to_string_pretty(&ruleset)?;
    json.push('\n');

    if let Some(parent) = config.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config.out, &json).with_context(|| format!("writing {:?}", config.out))?;

    let digest: String = Sha256::digest(json.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let templates: usize = ruleset.predicates.values().map(Vec::len).sum();
    println!(
        "mined {} @ min_count={} max_per_predicate={}",
        ruleset.name, config.opts.min_count, config.opts.max_per_predicate
    );
    println!(
        "  {} predicates, {} templates, from {} authoring docs",
        ruleset.predicates.len(),
        templates,
        documents.len()
    );
    println!("  sha256 {digest}");
    println!("  output {}", config.out.display());
    Ok(())
}

fn parse_args() -> Result<Config> {
    let mut config = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => {
                config.root = PathBuf::from(next_value(&mut args, "--root")?);
            }
            "--out" => {
                config.out = PathBuf::from(next_value(&mut args, "--out")?);
            }
            "--min-count" => {
                config.opts.min_count = next_value(&mut args, "--min-count")?.parse()?;
            }
            "--max-per-predicate" => {
                config.opts.max_per_predicate =
                    next_value(&mut args, "--max-per-predicate")?.parse()?;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            unknown => return Err(anyhow!("unknown argument '{unknown}'; use --help")),
        }
    }
    if config.opts.max_per_predicate == 0 {
        return Err(anyhow!("--max-per-predicate must be greater than zero"));
    }
    Ok(config)
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| anyhow!("{flag} requires a value"))
}

fn print_help() {
    println!(
        "Mine the WebNLG neuron-authored candidate rule set from train+validation\n\n\
Usage: cargo run --release -p cognigraph-construct --bin webnlg-mine-rules -- [OPTIONS]\n\n\
Options:\n  \
  --root DIR              Prepared pilot directory [data/webnlg-pilot]\n  \
  --out FILE             Output ruleset JSON [crates/.../fixtures/webnlg/neuron-ruleset.mined.json]\n  \
  --min-count N          Drop templates seen fewer than N times [1]\n  \
  --max-per-predicate N  Keep at most N templates per predicate [20]\n  \
  -h, --help             Print this help"
    );
}
