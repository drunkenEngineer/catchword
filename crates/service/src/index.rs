//! Indexing: the keyword stage per folder, then the meaning stage.

use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use catchword_embed::Embedder;
use catchword_engine::extract::{self, Limits, Outcome, Reason};
use catchword_engine::{
    chunk, chunk_pages, document_kind, hash_file, read_text, resolve_folder, scan, DocumentKind,
    FileMeta, Passage, Tokenizer, WordTokenizer, PASSAGE_OVERLAP_TOKENS, PASSAGE_TOKENS,
};
use catchword_store::Store;

use crate::{lock, Model};

/// Without a model, passages are sized in words.
const MAX_WORDS: usize = 200;
const OVERLAP_WORDS: usize = 30;
/// Passages embedded between two saves, so a stopped run loses little.
const EMBED_BATCH: usize = 32;

/// How passages are cut: in the model's tokens, or in words without one.
pub struct Cutter {
    tokenizer: Box<dyn Tokenizer + Send + Sync>,
    max: usize,
    overlap: usize,
    pipeline: String,
}

impl Cutter {
    pub fn for_model(model: &Model) -> Cutter {
        match model.ready() {
            Some(model) => {
                let model = lock(model);
                Cutter {
                    tokenizer: Box::new(model.tokenizer()),
                    max: PASSAGE_TOKENS,
                    overlap: PASSAGE_OVERLAP_TOKENS,
                    pipeline: format!(
                        "{} tokens {PASSAGE_TOKENS}/{PASSAGE_OVERLAP_TOKENS}",
                        model.manifest().id()
                    ),
                }
            }
            None => Cutter {
                tokenizer: Box::new(WordTokenizer),
                max: MAX_WORDS,
                overlap: OVERLAP_WORDS,
                pipeline: format!("words {MAX_WORDS}/{OVERLAP_WORDS}"),
            },
        }
    }
}

/// Where the PDF worker program is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Worker {
    /// Beside the running program, as installed.
    NextToProgram,
    At(PathBuf),
}

impl Worker {
    fn path(&self) -> Result<PathBuf> {
        let path = match self {
            Worker::At(path) => path.clone(),
            Worker::NextToProgram => std::env::current_exe()
                .context("cannot find this program's folder")?
                .with_file_name(format!("catchword-worker{}", std::env::consts::EXE_SUFFIX)),
        };
        if !path.is_file() {
            bail!(
                "cannot find the PDF reader at {}. Build it with `cargo build --workspace`",
                path.display()
            );
        }
        Ok(path)
    }
}

/// What indexing one folder did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub root: PathBuf,
    pub added: usize,
    pub reused: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub unsupported: usize,
    /// Each file that was not indexed, with why.
    pub not_indexed: Vec<(String, Reason)>,
    /// The way passages are cut changed, so the index was cleared first.
    pub rebuilt: bool,
    /// The run was stopped before the end.
    pub stopped: bool,
}

impl Report {
    pub fn failed(&self) -> usize {
        self.not_indexed
            .iter()
            .filter(|(_, r)| r.is_failure())
            .count()
    }

    pub fn skipped(&self) -> usize {
        self.not_indexed.len() - self.failed()
    }
}

/// The keyword stage for one folder: read every new or changed file, store
/// its passages, and forget files that are gone.
///
/// `progress(done, total)` is called after each file; returning false stops
/// the run. Nothing is lost: each file is its own transaction, and the next
/// run carries on. A stopped run forgets no file, since it has not seen
/// them all.
pub fn index_folder(
    store: &mut Store,
    folder: &Path,
    cutter: &Cutter,
    worker: &Worker,
    limits: &Limits,
    mut progress: impl FnMut(usize, usize) -> bool,
) -> Result<Report> {
    let root = resolve_folder(folder)
        .with_context(|| format!("cannot open folder {}", folder.display()))?;
    let mut report = Report {
        rebuilt: store.use_pipeline(&cutter.pipeline)?,
        root: root.clone(),
        ..Report::default()
    };
    // Found when the first PDF needs it, so text-only folders work without it.
    let mut worker_path = None;
    let files = scan(&root);
    let mut seen = Vec::new();

    for (done, file) in files.iter().enumerate() {
        if !progress(done, files.len()) {
            report.stopped = true;
            return Ok(report);
        }
        let Some(kind) = document_kind(&file.path) else {
            report.unsupported += 1;
            continue;
        };
        let path = file.path.to_string_lossy().to_string();
        // Seen even if reading fails below: a locked file is retried on the
        // next run, not forgotten.
        seen.push(path.clone());

        if store.is_unchanged(&path, file.size, file.modified_secs)? {
            report.unchanged += 1;
            continue;
        }
        let read = read_document(store, file, kind, worker, &mut worker_path, limits, cutter)?;
        let (hash, passages) = match read {
            Ok(read) => read,
            Err(reason) => {
                report.not_indexed.push((path, reason));
                continue;
            }
        };
        if store.put_file(&path, file.size, file.modified_secs, &hash, &passages)? {
            report.added += 1;
        } else {
            report.reused += 1;
        }
    }
    progress(files.len(), files.len());

    let mut prefix = root.to_string_lossy().to_string();
    if !prefix.ends_with(MAIN_SEPARATOR) {
        prefix.push(MAIN_SEPARATOR);
    }
    report.removed = store.purge_missing(&prefix, &seen)?;
    Ok(report)
}

/// Read one document: the hash of its content, and its passages unless that
/// content is already in the index. The inner error says why a file was not
/// indexed; the outer one stops the whole run.
fn read_document(
    store: &Store,
    file: &FileMeta,
    kind: DocumentKind,
    worker: &Worker,
    worker_path: &mut Option<PathBuf>,
    limits: &Limits,
    cutter: &Cutter,
) -> Result<std::result::Result<(String, Vec<Passage>), Reason>> {
    if file.size > limits.max_file_bytes {
        return Ok(Err(Reason::TooLarge));
    }
    let tokenizer = cutter.tokenizer.as_ref();
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
                chunk(&text, cutter.max, cutter.overlap, tokenizer)
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
            let program = match worker_path {
                Some(path) => path,
                None => worker_path.insert(worker.path()?),
            };
            let outcome =
                extract::run(program, &file.path, limits).context("cannot start the PDF reader")?;
            Ok(match outcome {
                Outcome::Pages(pages) => Ok((
                    hash,
                    chunk_pages(&pages, cutter.max, cutter.overlap, tokenizer),
                )),
                Outcome::NotIndexed(reason) => Err(reason),
            })
        }
    }
}

/// What the meaning stage did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EmbedReport {
    /// The model differs from the one the index was embedded with, so every
    /// passage was embedded again.
    pub model_changed: bool,
    pub stopped: bool,
}

/// The meaning stage: embed every passage that has no vector yet, saving
/// every few dozen, so a stopped run carries on where it stopped.
///
/// The model is locked for one passage at a time, so a search never waits
/// for more than one. `progress(done, total)` is called after each save;
/// returning false stops the run.
pub fn embed_missing(
    store: &mut Store,
    model: &Mutex<Embedder>,
    mut progress: impl FnMut(i64, i64) -> bool,
) -> Result<EmbedReport> {
    let manifest = *lock(model).manifest();
    let mut report = EmbedReport {
        model_changed: store.use_model(&manifest.id(), manifest.dimensions)?,
        stopped: false,
    };
    let counts = store.counts()?;
    let mut done = counts.vectors;
    loop {
        if !progress(done, counts.passages) {
            report.stopped = true;
            break;
        }
        let batch = store.passages_without_vectors(EMBED_BATCH)?;
        if batch.is_empty() {
            break;
        }
        let mut vectors = Vec::with_capacity(batch.len());
        for (id, text) in &batch {
            let vector = lock(model)
                .embed_passages(&[text.as_str()])
                .context("cannot embed passages")?
                .remove(0);
            vectors.push((*id, vector));
        }
        store.put_vectors(&vectors)?;
        done += vectors.len() as i64;
    }
    Ok(report)
}
