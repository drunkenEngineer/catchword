//! Catchword command-line tool: the first working slice.
//!
//! It indexes text and Markdown files and searches them by keyword. PDF
//! extraction and meaning-based search are the next two steps.

use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use catchword_engine::{chunk, is_supported, read_text, scan};
use catchword_store::Store;

/// Passage size in words. Replaced by a token limit once a model is chosen.
const MAX_WORDS: usize = 200;
const OVERLAP_WORDS: usize = 30;
const DEFAULT_INDEX: &str = "catchword-index.db";

const USAGE: &str = "Catchword: search your own files. Nothing leaves this computer.

Usage:
  catchword index <folder>     Index the text and Markdown files in a folder
  catchword search <words>     Find passages containing all the words
  catchword status             Show what the index holds

Options:
  --db <file>      Index file to use (default: catchword-index.db)
  --limit <n>      Number of results to show (default: 10)";

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

    match args.first().map(String::as_str) {
        Some("index") => {
            let folder = args.get(1).context("index needs a folder")?;
            index(&index_file, Path::new(folder))
        }
        Some("search") => {
            if args.len() < 2 {
                bail!("search needs at least one word");
            }
            search(&index_file, &args[1..].join(" "), limit)
        }
        Some("status") => status(&index_file),
        Some(other) => bail!("unknown command: {other}\n\n{USAGE}"),
        None => {
            println!("{USAGE}");
            Ok(())
        }
    }
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

fn index(index_file: &Path, folder: &Path) -> Result<()> {
    let root = folder
        .canonicalize()
        .with_context(|| format!("cannot open folder {}", folder.display()))?;
    let mut store = Store::open(index_file).context("cannot open the index")?;

    let (mut added, mut reused, mut unchanged, mut unsupported, mut failed) = (0, 0, 0, 0, 0);
    let mut seen = Vec::new();

    for file in scan(&root) {
        if !is_supported(&file.path) {
            unsupported += 1;
            continue;
        }
        let path = file.path.to_string_lossy().to_string();
        // Seen even if reading fails below: a locked file is retried on the
        // next run, not forgotten.
        seen.push(path.clone());

        if store.is_unchanged(&path, file.size, file.modified_secs)? {
            unchanged += 1;
            continue;
        }
        let (text, hash) = match read_text(&file.path) {
            Ok(read) => read,
            Err(error) => {
                eprintln!("could not read {path}: {error}");
                failed += 1;
                continue;
            }
        };
        // Known content (a copy or a moved file) costs nothing to index again.
        let passages = if store.has_content(&hash)? {
            Vec::new()
        } else {
            chunk(&text, MAX_WORDS, OVERLAP_WORDS)
        };
        if store.put_file(&path, file.size, file.modified_secs, &hash, &passages)? {
            added += 1;
        } else {
            reused += 1;
        }
    }

    let mut prefix = root.to_string_lossy().to_string();
    if !prefix.ends_with(MAIN_SEPARATOR) {
        prefix.push(MAIN_SEPARATOR);
    }
    let removed = store.purge_missing(&prefix, &seen)?;

    println!("Indexed {}", root.display());
    println!("  new or changed: {added}");
    println!("  same content as another file: {reused}");
    println!("  unchanged: {unchanged}");
    println!("  removed: {removed}");
    println!("  not a supported type: {unsupported}");
    println!("  could not be read: {failed}");
    Ok(())
}

fn search(index_file: &Path, query: &str, limit: usize) -> Result<()> {
    let store = Store::open(index_file).context("cannot open the index")?;
    let hits = store.search_keyword(query, limit)?;
    if hits.is_empty() {
        println!("No passage contains all of: {query}");
        return Ok(());
    }
    for (number, hit) in hits.iter().enumerate() {
        let copies = if hit.copies > 1 {
            format!("  (+{} identical)", hit.copies - 1)
        } else {
            String::new()
        };
        println!(
            "{}. {}{}  lines {}-{}",
            number + 1,
            hit.path,
            copies,
            hit.start_line,
            hit.end_line
        );
        println!("   {}", hit.snippet);
    }
    Ok(())
}

fn status(index_file: &Path) -> Result<()> {
    let store = Store::open(index_file).context("cannot open the index")?;
    let counts = store.counts()?;
    println!("Index: {}", index_file.display());
    println!("  files: {}", counts.files);
    println!("  distinct contents: {}", counts.contents);
    println!("  passages: {}", counts.passages);
    Ok(())
}
