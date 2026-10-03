//! Catchword command-line tool: the first working slice.
//!
//! It indexes text, Markdown and PDF files and searches them by their words
//! and by their meaning. PDFs are read by the extraction worker,
//! `catchword-worker`, which must sit next to this program. Meaning search
//! needs ONNX Runtime and the embedding model (scripts/fetch-embedding.sh);
//! without them, keyword search still works. The work itself is done by
//! catchword-service, shared with the desktop app.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use catchword_engine::extract::Limits;
use catchword_engine::fuse::Found;
use catchword_engine::{without_controls, Exclusions};
use catchword_service::{embed_missing, index_folder, search, Cutter, Model, Worker};
use catchword_store::Store;

const DEFAULT_INDEX: &str = "catchword-index.db";

const USAGE: &str = "Catchword: search your own files. Nothing leaves this computer.

Usage:
  catchword index <folder>     Index the text, Markdown and PDF files in a folder
  catchword search <words>     Find passages by their words and their meaning
  catchword status             Show what the index holds

Options:
  --db <file>      Index file to use (default: catchword-index.db)
  --limit <n>      Number of results to show (default: 10)
  --retry          With index: read again the files whose reading failed before";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let index_file =
        take_option(&mut args, "--db").map_or_else(|| PathBuf::from(DEFAULT_INDEX), PathBuf::from);
    let limit = match take_option(&mut args, "--limit") {
        Some(value) => value
            .parse::<usize>()
            .context("--limit needs a whole number")?,
        None => 10,
    };
    let retry = take_flag(&mut args, "--retry");

    match args.first().map(String::as_str) {
        Some("index") => {
            let folder = args.get(1).context("index needs a folder")?;
            index(&index_file, Path::new(folder), retry)
        }
        Some("search") => {
            if args.len() < 2 {
                bail!("search needs at least one word");
            }
            search_index(&index_file, &args[1..].join(" "), limit)
        }
        Some("status") => status(&index_file),
        Some(other) => bail!("unknown command: {other}\n\n{USAGE}"),
        None => {
            println!("{USAGE}");
            Ok(())
        }
    }
}

/// Remove `--name` from the arguments; true if it was there.
fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    let before = args.len();
    args.retain(|arg| arg != name);
    args.len() != before
}

/// Remove `--name value` from the arguments and return the value.
fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let at = args.iter().position(|arg| arg == name)?;
    if at + 1 >= args.len() {
        args.remove(at);
        return None;
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Some(value)
}

fn index(index_file: &Path, folder: &Path, retry: bool) -> Result<()> {
    let mut store = open_store(index_file)?;
    if retry {
        let forgotten = store.forget_failures()?;
        println!("Files that failed before, to read again: {forgotten}");
    }
    let model = Model::load();
    if let Model::Unavailable(why) = &model {
        println!("Note: meaning search is off: {why}.");
    }
    let cutter = Cutter::for_model(&model);
    let report = index_folder(
        &mut store,
        folder,
        &Exclusions::with_default_patterns(),
        &cutter,
        &Worker::NextToProgram,
        &Limits::default(),
        |_, _| true,
    )?;
    if report.rebuilt {
        println!("Note: passages are now cut differently, so the index is rebuilt from the start.");
    }
    println!("Indexed {}", report.root.display());
    println!("  new or changed: {}", report.added);
    println!("  same content as another file: {}", report.reused);
    println!("  unchanged: {}", report.unchanged);
    println!("  removed: {}", report.removed);
    println!("  not a supported type: {}", report.unsupported);
    println!("  skipped: {}", report.skipped());
    println!("  failed: {}", report.failed());
    if !report.not_indexed.is_empty() {
        println!("Not indexed:");
        for (path, reason) in &report.not_indexed {
            println!("  {}: {}", without_controls(path), reason.describe());
        }
    }
    if report.known_problems > 0 {
        println!(
            "{} of these were not read again: nothing changed since the last try.              To read failed files again, add --retry.",
            report.known_problems
        );
    }

    // Keyword search works from here on. Meaning search follows.
    if let Some(model) = model.ready() {
        let mut shown = false;
        let embedded = embed_missing(&mut store, model, |done, total| {
            if done < total {
                eprint!("\rEmbedding: {done} of {total} passages");
                let _ = std::io::stderr().flush();
                shown = true;
            }
            true
        })?;
        if shown {
            eprintln!();
        }
        if embedded.model_changed {
            println!("Note: the embedding model changed, so every passage was embedded again.");
        }
    }
    let counts = store.counts()?;
    println!(
        "  searchable by meaning: {} of {} passages",
        counts.vectors, counts.passages
    );
    Ok(())
}

fn open_store(index_file: &Path) -> Result<Store> {
    let store = Store::open(index_file).context("cannot open the index")?;
    if store.was_reset() {
        println!(
            "Note: the index was made by an older version of Catchword and has been cleared. \
             Index your folders again to fill it."
        );
    }
    Ok(store)
}

/// Search by words and by meaning, combined by rank (ADR-5).
fn search_index(index_file: &Path, query: &str, limit: usize) -> Result<()> {
    let store = open_store(index_file)?;
    let answer = search(&store, &Model::load(), query)?;
    for note in &answer.notes {
        println!("Note: {}", note.describe());
    }
    if answer.results.is_empty() {
        println!("Nothing found for: {query}");
        return Ok(());
    }

    for (number, (hit, found)) in answer.results.iter().take(limit).enumerate() {
        let copies = if hit.copies > 1 {
            format!("  (+{} identical)", hit.copies - 1)
        } else {
            String::new()
        };
        let location = match hit.page {
            Some(page) => format!("page {page}"),
            None => format!("lines {}-{}", hit.start_line, hit.end_line),
        };
        let found = match found {
            Found::Keyword => "words",
            Found::Meaning => "meaning",
            Found::Both => "words and meaning",
        };
        println!(
            "{}. {}{}  {location}  ({found})",
            number + 1,
            without_controls(&hit.path),
            copies
        );
        // Matches are marked with two control characters; show them as brackets.
        let snippet = hit.snippet.replace('\u{2}', "[").replace('\u{3}', "]");
        println!("   {snippet}");
    }
    Ok(())
}

fn status(index_file: &Path) -> Result<()> {
    let store = open_store(index_file)?;
    let counts = store.counts()?;
    println!("Index: {}", index_file.display());
    println!("  files: {}", counts.files);
    println!("  distinct contents: {}", counts.contents);
    println!("  passages: {}", counts.passages);
    println!(
        "  searchable by meaning: {} of {} passages",
        counts.vectors, counts.passages
    );
    if let Some(model) = store.embedding_model()? {
        println!("  embedding model: {model}");
    }
    Ok(())
}
