//! Indexing: the keyword stage per folder, then the meaning stage.

use std::fmt;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use catchword_embed::Embed;
use catchword_engine::extract::{Extractor, Limits, Outcome, PdfReader, Reason, TextFiles};
use catchword_engine::{
    chunk_pages_while, chunk_while, document_kind, hash_file, resolve_folder, scan, DocumentKind,
    Exclusions, FileMeta, Passage, Tokenizer, WordTokenizer, PASSAGE_OVERLAP_TOKENS,
    PASSAGE_TOKENS,
};
use catchword_store::{Problem, Store};

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
                    tokenizer: model.tokenizer(),
                    max: PASSAGE_TOKENS,
                    overlap: PASSAGE_OVERLAP_TOKENS,
                    // "in blocks": tokens counted 2,000 words at a time, so
                    // long texts are counted to the end. Passages cut before
                    // then are cut again (see Store::use_pipeline).
                    pipeline: format!(
                        "{} tokens {PASSAGE_TOKENS}/{PASSAGE_OVERLAP_TOKENS} in blocks",
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

/// A file whose reading failed is tried this many times, then parked until
/// the user asks for a retry (spec: lifecycle of a file).
pub const ATTEMPTS_BEFORE_PARKING: u32 = 2;

/// True for a file whose reading failed too often: it is not read again
/// until the user retries, or the file changes.
pub fn is_parked(problem: &Problem) -> bool {
    problem.reason.is_failure() && problem.attempts >= ATTEMPTS_BEFORE_PARKING
}

/// Whether a file that was not indexed before, and has not changed since, is
/// read again. One skipped by a rule is not: the same rule would skip it
/// again. One that could not be opened is: it was probably in use, and trying
/// costs nothing. One whose reading failed gets a second try, then is parked.
fn read_again(problem: &Problem) -> bool {
    match problem.reason {
        // A cloud-only file is checked on every run, from its attributes
        // alone: once it is on this computer again it is read.
        Reason::CannotOpen | Reason::CloudOnly => true,
        reason if reason.is_failure() => !is_parked(problem),
        _ => false,
    }
}

/// The folder could not be reached: an unplugged drive, a moved folder or
/// no access. Nothing in the index was changed: its files stay, offline,
/// not deleted (SRC-5).
#[derive(Debug)]
pub struct Unreachable {
    pub folder: PathBuf,
    pub cause: std::io::Error,
}

impl fmt::Display for Unreachable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot open folder {}: {}",
            self.folder.display(),
            self.cause
        )
    }
}

impl std::error::Error for Unreachable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
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
    /// How many of those were not read again this time, because the index
    /// already knew why: skipped by a rule, or parked after failing.
    pub known_problems: usize,
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
/// its passages, and forget files that are gone. Files that `exclusions`
/// leave out count as gone, so excluding a folder removes its text.
///
/// `progress(done, total)` is called after each file; returning false stops
/// the run. Nothing is lost: each file is its own transaction, and the next
/// run carries on. A stopped run forgets no file, since it has not seen
/// them all.
///
/// A folder that cannot be reached is an [`Unreachable`] error, and changes
/// nothing. Files in a sub-folder that could not be read are kept.
/// Cloud-only files are never opened (SRC-4).
pub fn index_folder(
    store: &mut Store,
    folder: &Path,
    exclusions: &Exclusions,
    cutter: &Cutter,
    worker: &Worker,
    limits: &Limits,
    mut progress: impl FnMut(usize, usize) -> bool,
) -> Result<Report> {
    let unreachable = |cause| Unreachable {
        folder: folder.to_path_buf(),
        cause,
    };
    let root = resolve_folder(folder).map_err(unreachable)?;
    let found = scan(&root, exclusions).map_err(unreachable)?;
    let mut report = Report {
        rebuilt: store.use_pipeline(&cutter.pipeline)?,
        root: root.clone(),
        ..Report::default()
    };
    let mut reader = Reader {
        worker,
        pdf: None,
        limits,
    };
    let files = found.files;
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
        let known = store.known_problem(&path, file.size, file.modified_secs)?;
        if file.cloud_only {
            // Opening it would download it. Recorded once, not every run.
            if known.as_ref().map(|known| known.reason) == Some(Reason::CloudOnly) {
                report.known_problems += 1;
            } else {
                store.record_problem(&path, file.size, file.modified_secs, Reason::CloudOnly)?;
            }
            report.not_indexed.push((path, Reason::CloudOnly));
            continue;
        }
        if let Some(known) = known {
            if !read_again(&known) {
                report.known_problems += 1;
                report.not_indexed.push((path, known.reason));
                continue;
            }
        }
        // Asked again while a long file is read, so a pause need not wait
        // for the file to end (IDX-5).
        let mut keep_going = || progress(done, files.len());
        let read = read_document(store, file, kind, &mut reader, cutter, &mut keep_going)?;
        let (hash, passages) = match read {
            Read::Content(hash, passages) => (hash, passages),
            Read::NotIndexed(reason) => {
                store.record_problem(&path, file.size, file.modified_secs, reason)?;
                report.not_indexed.push((path, reason));
                continue;
            }
            // Nothing is stored for it; the next run reads it again.
            Read::Stopped => {
                report.stopped = true;
                return Ok(report);
            }
        };
        if store.put_file(&path, file.size, file.modified_secs, &hash, &passages)? {
            report.added += 1;
        } else {
            report.reused += 1;
        }
    }
    progress(files.len(), files.len());

    // What could not be read is unknown, not gone: keep it.
    for unreadable in &found.unreadable {
        let unreadable = unreadable.to_string_lossy().to_string();
        let inside = format!("{unreadable}{MAIN_SEPARATOR}");
        seen.extend(
            store
                .known_paths(&unreadable)?
                .into_iter()
                .filter(|path| *path == unreadable || path.starts_with(&inside)),
        );
    }
    let mut prefix = root.to_string_lossy().to_string();
    if !prefix.ends_with(MAIN_SEPARATOR) {
        prefix.push(MAIN_SEPARATOR);
    }
    report.removed = store.purge_missing(&prefix, &seen)?;
    Ok(report)
}

/// What reading one document gave.
enum Read {
    /// The hash of its content, and its passages unless that content is
    /// already in the index.
    Content(String, Vec<Passage>),
    NotIndexed(Reason),
    /// Stopped part way, as for a pause.
    Stopped,
}

/// How files are read: the limits, and an extractor for each kind. The
/// PDF reader is found when the first PDF needs it, so text-only folders
/// work without it.
struct Reader<'a> {
    worker: &'a Worker,
    pdf: Option<PdfReader>,
    limits: &'a Limits,
}

impl Reader<'_> {
    fn extractor(&mut self, kind: DocumentKind) -> Result<&dyn Extractor> {
        Ok(match kind {
            DocumentKind::Text => &TextFiles,
            DocumentKind::Pdf => {
                if self.pdf.is_none() {
                    self.pdf = Some(PdfReader {
                        program: self.worker.path()?,
                    });
                }
                self.pdf.as_ref().expect("set just above")
            }
        })
    }
}

/// Read one document. An error stops the whole run; a file that cannot be
/// read is `Read::NotIndexed`, with why.
fn read_document(
    store: &Store,
    file: &FileMeta,
    kind: DocumentKind,
    reader: &mut Reader,
    cutter: &Cutter,
    keep_going: &mut dyn FnMut() -> bool,
) -> Result<Read> {
    let limits = reader.limits;
    if file.size > limits.max_file_bytes {
        return Ok(Read::NotIndexed(Reason::TooLarge));
    }
    let Ok(hash) = hash_file(&file.path) else {
        return Ok(Read::NotIndexed(Reason::CannotOpen));
    };
    // Known content (a copy or a moved file) costs nothing to index again.
    if store.has_content(&hash)? {
        return Ok(Read::Content(hash, Vec::new()));
    }
    let extractor = reader.extractor(kind)?;
    let pages = match extractor
        .extract(&file.path, limits, keep_going)
        .context("cannot start the reader")?
    {
        Outcome::Pages(pages) => pages,
        Outcome::NotIndexed(reason) => return Ok(Read::NotIndexed(reason)),
        Outcome::Stopped => return Ok(Read::Stopped),
    };
    let tokenizer = cutter.tokenizer.as_ref();
    let passages = if extractor.paged() {
        chunk_pages_while(&pages, cutter.max, cutter.overlap, tokenizer, keep_going)
    } else {
        chunk_while(
            &pages.concat(),
            cutter.max,
            cutter.overlap,
            tokenizer,
            keep_going,
        )
    };
    Ok(match passages {
        Some(passages) => Read::Content(hash, passages),
        None => Read::Stopped,
    })
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
    model: &Mutex<Box<dyn Embed>>,
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
            // Asked after every passage, so a pause takes effect at once;
            // what is done so far is saved below.
            if !progress(done + vectors.len() as i64, counts.passages) {
                report.stopped = true;
                break;
            }
        }
        store.put_vectors(&vectors)?;
        done += vectors.len() as i64;
        if report.stopped {
            break;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(reason: Reason, attempts: u32) -> Problem {
        Problem {
            path: "/docs/file.pdf".to_string(),
            reason,
            attempts,
        }
    }

    #[test]
    fn a_failed_file_gets_a_second_try_then_is_parked() {
        assert!(read_again(&problem(Reason::Crashed, 1)));
        assert!(!is_parked(&problem(Reason::Crashed, 1)));
        assert!(!read_again(&problem(Reason::TimedOut, 2)));
        assert!(is_parked(&problem(Reason::TimedOut, 2)));
    }

    #[test]
    fn a_file_skipped_by_a_rule_is_not_read_again_nor_parked() {
        for reason in [Reason::NeedsOcr, Reason::Encrypted, Reason::TooLarge] {
            assert!(!read_again(&problem(reason, 1)), "{reason:?}");
            assert!(!is_parked(&problem(reason, 5)), "{reason:?}");
        }
    }

    #[test]
    fn a_file_that_could_not_be_opened_is_always_tried_again() {
        assert!(read_again(&problem(Reason::CannotOpen, 9)));
        assert!(!is_parked(&problem(Reason::CannotOpen, 9)));
    }

    #[test]
    fn a_file_that_was_cloud_only_is_read_once_it_is_here() {
        assert!(read_again(&problem(Reason::CloudOnly, 3)));
        assert!(!is_parked(&problem(Reason::CloudOnly, 3)));
    }
}
