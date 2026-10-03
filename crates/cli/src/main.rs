//! Catchword command-line tool: the first working slice.
//!
//! It indexes text, Markdown and PDF files and searches them by their words
//! and by their meaning. PDFs are read by the extraction worker,
//! `catchword-worker`, which must sit next to this program. Meaning search
//! needs ONNX Runtime and the embedding model (scripts/fetch-embedding.sh);
//! without them, keyword search still works.

use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use catchword_embed::{Embedder, DEFAULT_MODEL};
use catchword_engine::extract::{self, Limits, Outcome, Reason};
use catchword_engine::fuse::{Found, CANDIDATES};
use catchword_engine::{
    chunk, chunk_pages, document_kind, hash_file, read_text, resolve_folder, scan,
    without_controls, DocumentKind, FileMeta, Passage, Tokenizer, WordTokenizer,
    PASSAGE_OVERLAP_TOKENS, PASSAGE_TOKENS,
};
use catchword_store::Store;

/// Without a model, passages are sized in words.
const MAX_WORDS: usize = 200;
const OVERLAP_WORDS: usize = 30;
/// Passages embedded between two saves, so an interrupted run loses little.
const EMBED_BATCH: usize = 32;
const DEFAULT_INDEX: &str = "catchword-index.db";

const USAGE: &str = "Catchword: search your own files. Nothing leaves this computer.

Usage:
  catchword index <folder>     Index the text, Markdown and PDF files in a folder
  catchword search <words>     Find passages by their words and their meaning
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

/// How passages are cut: in the model's tokens, or in words without a model.
struct Cutter<'a> {
    tokenizer: &'a dyn Tokenizer,
    max: usize,
    overlap: usize,
}

fn index(index_file: &Path, folder: &Path) -> Result<()> {
    let root = resolve_folder(folder)
        .with_context(|| format!("cannot open folder {}", folder.display()))?;
    let mut store = open_store(index_file)?;
    let mut model = load_model();
    let limits = Limits::default();
    // Found when the first PDF needs it, so text-only folders work without it.
    let mut worker = None;

    let cutter = match &model {
        Some(model) => Cutter {
            tokenizer: model,
            max: PASSAGE_TOKENS,
            overlap: PASSAGE_OVERLAP_TOKENS,
        },
        None => Cutter {
            tokenizer: &WordTokenizer,
            max: MAX_WORDS,
            overlap: OVERLAP_WORDS,
        },
    };
    let pipeline = match &model {
        Some(model) => format!(
            "{} tokens {PASSAGE_TOKENS}/{PASSAGE_OVERLAP_TOKENS}",
            model.manifest().id()
        ),
        None => format!("words {MAX_WORDS}/{OVERLAP_WORDS}"),
    };
    if store.use_pipeline(&pipeline)? {
        println!("Note: passages are now cut differently, so the index is rebuilt from the start.");
    }

    let (mut added, mut reused, mut unchanged, mut unsupported) = (0, 0, 0, 0);
    let mut not_indexed: Vec<(String, Reason)> = Vec::new();
    let mut seen = Vec::new();

    for file in scan(&root) {
        let Some(kind) = document_kind(&file.path) else {
            unsupported += 1;
            continue;
        };
        let path = file.path.to_string_lossy().to_string();
        // Seen even if reading fails below: a locked file is retried on the
        // next run, not forgotten.
        seen.push(path.clone());

        if store.is_unchanged(&path, file.size, file.modified_secs)? {
            unchanged += 1;
            continue;
        }
        let read = read_document(&store, &file, kind, &mut worker, &limits, &cutter)?;
        let (hash, passages) = match read {
            Ok(read) => read,
            Err(reason) => {
                not_indexed.push((path, reason));
                continue;
            }
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

    let failed = not_indexed.iter().filter(|(_, r)| r.is_failure()).count();
    println!("Indexed {}", root.display());
    println!("  new or changed: {added}");
    println!("  same content as another file: {reused}");
    println!("  unchanged: {unchanged}");
    println!("  removed: {removed}");
    println!("  not a supported type: {unsupported}");
    println!("  skipped: {}", not_indexed.len() - failed);
    println!("  failed: {failed}");
    if !not_indexed.is_empty() {
        println!("Not indexed:");
        for (path, reason) in &not_indexed {
            println!("  {}: {}", without_controls(path), reason.describe());
        }
    }

    // Keyword search works from here on. Meaning search follows.
    if let Some(model) = model.as_mut() {
        embed_missing(&mut store, model)?;
    }
    let counts = store.counts()?;
    println!(
        "  searchable by meaning: {} of {} passages",
        counts.vectors, counts.passages
    );
    Ok(())
}

/// Load the embedding model, or say why meaning search is off.
fn load_model() -> Option<Embedder> {
    let Some(paths) = catchword_embed::find(&DEFAULT_MODEL) else {
        println!(
            "Note: meaning search is off: the embedding model or ONNX Runtime was not found. \
             Run `sh scripts/fetch-embedding.sh`."
        );
        return None;
    };
    match Embedder::load(&paths, &DEFAULT_MODEL) {
        Ok(model) => Some(model),
        Err(error) => {
            println!("Note: meaning search is off: {error}.");
            None
        }
    }
}

/// The meaning stage: embed every passage that has no vector yet, saving
/// after each batch, so an interrupted run carries on where it stopped.
fn embed_missing(store: &mut Store, model: &mut Embedder) -> Result<()> {
    let manifest = *model.manifest();
    if store.use_model(&manifest.id(), manifest.dimensions)? {
        println!("Note: the embedding model changed, so every passage is embedded again.");
    }
    let counts = store.counts()?;
    let mut done = counts.vectors;
    loop {
        let batch = store.passages_without_vectors(EMBED_BATCH)?;
        if batch.is_empty() {
            break;
        }
        let texts: Vec<&str> = batch.iter().map(|(_, text)| text.as_str()).collect();
        let vectors = model
            .embed_passages(&texts)
            .context("cannot embed passages")?;
        let vectors: Vec<(i64, Vec<f32>)> = batch.iter().map(|(id, _)| *id).zip(vectors).collect();
        store.put_vectors(&vectors)?;
        done += vectors.len() as i64;
        eprint!("\rEmbedding: {done} of {} passages", counts.passages);
    }
    if done > counts.vectors {
        eprintln!();
    }
    Ok(())
}

/// Read one document: the hash of its content, and its passages unless that
/// content is already in the index. The inner error says why a file was not
/// indexed; the outer one stops the whole run.
fn read_document(
    store: &Store,
    file: &FileMeta,
    kind: DocumentKind,
    worker: &mut Option<PathBuf>,
    limits: &Limits,
    cutter: &Cutter,
) -> Result<std::result::Result<(String, Vec<Passage>), Reason>> {
    if file.size > limits.max_file_bytes {
        return Ok(Err(Reason::TooLarge));
    }
    match kind {
        DocumentKind::Text => {
            let (text, hash) = match read_text(&file.path, limits.max_file_bytes) {
                Ok(read) => read,
                Err(reason) => return Ok(Err(reason)),
            };
            // Known content (a copy or a moved file) costs nothing to index again.
            let passages = if store.has_content(&hash)? {
                Vec::new()
            } else {
                chunk(&text, cutter.max, cutter.overlap, cutter.tokenizer)
            };
            Ok(Ok((hash, passages)))
        }
        DocumentKind::Pdf => {
            let Ok(hash) = hash_file(&file.path) else {
                return Ok(Err(Reason::CannotOpen));
            };
            if store.has_content(&hash)? {
                return Ok(Ok((hash, Vec::new())));
            }
            let worker = match worker {
                Some(worker) => worker,
                None => worker.insert(worker_path()?),
            };
            let outcome =
                extract::run(worker, &file.path, limits).context("cannot start the PDF reader")?;
            Ok(match outcome {
                Outcome::Pages(pages) => Ok((
                    hash,
                    chunk_pages(&pages, cutter.max, cutter.overlap, cutter.tokenizer),
                )),
                Outcome::NotIndexed(reason) => Err(reason),
            })
        }
    }
}

/// The extraction worker sits next to this program.
fn worker_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot find this program's folder")?;
    let worker = exe.with_file_name(format!("catchword-worker{}", std::env::consts::EXE_SUFFIX));
    if !worker.is_file() {
        bail!(
            "cannot find the PDF reader at {}. Build it with `cargo build --workspace`",
            worker.display()
        );
    }
    Ok(worker)
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
fn search(index_file: &Path, query: &str, limit: usize) -> Result<()> {
    let store = open_store(index_file)?;
    let vector = query_vector(&store, query)?;
    let results = store.search_combined(query, vector.as_deref(), CANDIDATES)?;
    if results.is_empty() {
        println!("Nothing found for: {query}");
        return Ok(());
    }

    for (number, (hit, found)) in results.iter().take(limit).enumerate() {
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
        println!("   {}", hit.snippet);
    }
    Ok(())
}

/// The query as a vector, for meaning search. None, with a note, when this
/// index has no vectors from the installed model.
fn query_vector(store: &Store, query: &str) -> Result<Option<Vec<f32>>> {
    let Some(mut model) = load_model() else {
        return Ok(None);
    };
    let Some(indexed_with) = store.embedding_model()? else {
        println!("Note: this index is not searchable by meaning yet. Run `catchword index` again.");
        return Ok(None);
    };
    if model.manifest().id() != indexed_with {
        println!(
            "Note: this index was embedded with another model ({indexed_with}).              Run `catchword index` again to update it. Showing matches by words only."
        );
        return Ok(None);
    }
    let counts = store.counts()?;
    if counts.vectors < counts.passages {
        println!(
            "Note: {} of {} passages are searchable by meaning so far.",
            counts.vectors, counts.passages
        );
    }
    let vector = model.embed_query(query).context("cannot embed the query")?;
    Ok(Some(vector))
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
