//! Catchword retrieval evaluation and benchmark (TST-1, ARC-5, CORE-8).
//! A development tool: it reads the source tree's eval/ and vendor/ folders.

mod metrics;
mod report;
mod run;
mod scale;
mod set;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use catchword_embed::{ModelManifest, DEFAULT_MODEL, E5_SMALL, GRANITE_97M};
use catchword_engine::fuse::CANDIDATES;
use catchword_engine::{PASSAGE_OVERLAP_TOKENS, PASSAGE_TOKENS};

use crate::run::Config;

const USAGE: &str = "catchword-eval: measure search quality and speed.

Commands:
  run        Score one configuration
  benchmark  Score both models at 200, 350 and 500 tokens (ARC-5)
  check      Score the app's settings against eval/thresholds.txt; fails below
  scale      Time searches on a synthetic index of reference size

Options:
  --model granite|e5   Model for run (default: the app's)
  --tokens <n>         Passage size for run (default: the app's)
  --overlap <n>        Overlap for run (default: the app's)
  --candidates <n>     Results each search adds before combining (default: the app's)
  --passages <n>       Size of the scale index (default: 250000)
  --out <file>         Also write the report to a file

Needs sh scripts/fetch-embedding.sh and sh scripts/fetch-eval.sh first.";

fn main() -> ExitCode {
    match start() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn start() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    let number = |name: &str, default: usize| -> Result<usize> {
        option(name).map_or(Ok(default), |value| {
            value
                .parse()
                .with_context(|| format!("{name} needs a number"))
        })
    };
    let vendor = root().join("vendor");

    let report = match args.first().map(String::as_str) {
        Some("run") => {
            let model = match option("--model").as_deref() {
                None => DEFAULT_MODEL,
                Some(name) => model_named(name)?,
            };
            let config = Config {
                model,
                tokens: number("--tokens", PASSAGE_TOKENS)?,
                overlap: number("--overlap", PASSAGE_OVERLAP_TOKENS)?,
                candidates: number("--candidates", CANDIDATES)?,
            };
            let set = load_set()?;
            let outcome = run::evaluate(&set, &config, &vendor)?;
            report::detail(&set, &outcome)
        }
        Some("benchmark") => benchmark(&vendor)?,
        Some("check") => return check(&vendor),
        Some("scale") => {
            let passages = number("--passages", 250_000)?;
            scale::run(passages, &std::env::temp_dir().join("catchword-scale"))?
        }
        _ => {
            println!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        }
    };
    println!("{report}");
    if let Some(file) = option("--out") {
        fs::write(&file, &report).with_context(|| format!("cannot write {file}"))?;
    }
    Ok(ExitCode::SUCCESS)
}

fn model_named(name: &str) -> Result<ModelManifest> {
    Ok(match name {
        "granite" => GRANITE_97M,
        "e5" => E5_SMALL,
        _ => bail!("unknown model {name}; use granite or e5"),
    })
}

fn load_set() -> Result<set::EvalSet> {
    let set = set::load(
        &root().join("vendor/eval/xquad"),
        &root().join("eval/domain"),
    )?;
    eprintln!(
        "Evaluation set: {} documents, {} queries.",
        set.docs.len(),
        set.queries.len()
    );
    Ok(set)
}

/// Both models at the three passage sizes the spec names (section 26),
/// overlap about a seventh of the passage.
fn benchmark(vendor: &Path) -> Result<String> {
    let set = load_set()?;
    let mut outcomes = Vec::new();
    for model in [GRANITE_97M, E5_SMALL] {
        for (tokens, overlap) in [(200, 30), (350, 50), (500, 70)] {
            let config = Config {
                model,
                tokens,
                overlap,
                candidates: CANDIDATES,
            };
            eprintln!("Scoring {} ...", config.label());
            outcomes.push(run::evaluate(&set, &config, vendor)?);
        }
    }
    let mut out = String::from("## Summary\n\n");
    out.push_str(&report::summary(&set, &outcomes));
    out.push_str("\n## Each configuration\n\n");
    for outcome in &outcomes {
        out.push_str(&report::detail(&set, outcome));
        out.push('\n');
    }
    Ok(out)
}

/// The app's own settings against the committed minimums.
fn check(vendor: &Path) -> Result<ExitCode> {
    let path = root().join("eval/thresholds.txt");
    let thresholds =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let set = load_set()?;
    let config = Config {
        model: DEFAULT_MODEL,
        tokens: PASSAGE_TOKENS,
        overlap: PASSAGE_OVERLAP_TOKENS,
        candidates: CANDIDATES,
    };
    let outcome = run::evaluate(&set, &config, vendor)?;
    let failed = report::check(&set, &outcome, &thresholds)?;
    if failed.is_empty() {
        println!(
            "All {} thresholds met.",
            thresholds
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .count()
        );
        Ok(ExitCode::SUCCESS)
    } else {
        println!(
            "{} thresholds not met. See ADR-19: a drop needs a written justification.",
            failed.len()
        );
        Ok(ExitCode::FAILURE)
    }
}
