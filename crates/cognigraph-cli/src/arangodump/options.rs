//! `cognigraph import --from-arangodump DIR --output STORE [...]` arguments.

use std::path::PathBuf;

/// Bounds from docs/reference/arangodump-import.md (defaults match the
/// reference reader).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_record_bytes: u64,
    pub max_expanded_bytes: u64,
    pub max_files: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_record_bytes: 16 << 20,
            max_expanded_bytes: 64 << 30,
            max_files: 100_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub dump: PathBuf,
    pub output: PathBuf,
    pub dry_run: bool,
    pub report: Option<PathBuf>,
    pub limits: Limits,
}

fn positive(flag: &str, value: &str) -> Result<u64, String> {
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("{flag} expects a positive integer, got `{value}`")),
    }
}

/// Parse the words after `import`. The first must be `--from-arangodump`.
pub fn parse(tail: &[&str]) -> Result<Options, String> {
    let mut dump = None;
    let mut output = None;
    let mut dry_run = false;
    let mut report = None;
    let mut limits = Limits::default();
    let mut rest = tail;
    while let Some((flag, more)) = rest.split_first() {
        let (value, after) = match (*flag, more) {
            ("--dry-run", after) => {
                dry_run = true;
                rest = after;
                continue;
            }
            (_, [value, after @ ..]) if !value.starts_with("--") => (*value, after),
            _ if flag.starts_with("--") => {
                return Err(match *flag {
                    "--from-arangodump"
                    | "--output"
                    | "--report"
                    | "--max-record-bytes"
                    | "--max-expanded-bytes"
                    | "--max-files" => format!("{flag} expects a value"),
                    other => format!("unknown flag `{other}` for import --from-arangodump"),
                });
            }
            _ => {
                return Err(format!(
                    "unexpected argument `{flag}` for import --from-arangodump"
                ));
            }
        };
        let slot = match *flag {
            "--from-arangodump" => &mut dump,
            "--output" => &mut output,
            "--report" => &mut report,
            "--max-record-bytes" => {
                limits.max_record_bytes = positive(flag, value)?;
                rest = after;
                continue;
            }
            "--max-expanded-bytes" => {
                limits.max_expanded_bytes = positive(flag, value)?;
                rest = after;
                continue;
            }
            "--max-files" => {
                limits.max_files = positive(flag, value)? as usize;
                rest = after;
                continue;
            }
            other => {
                return Err(format!(
                    "unknown flag `{other}` for import --from-arangodump"
                ));
            }
        };
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(format!("{flag} given more than once"));
        }
        rest = after;
    }
    Ok(Options {
        dump: dump.ok_or("import --from-arangodump needs a dump directory")?,
        output: output.ok_or("import --from-arangodump needs --output STORE")?,
        dry_run,
        report,
        limits,
    })
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod tests;
