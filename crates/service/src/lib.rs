//! Catchword service: the use cases the command-line tool and the desktop
//! app share. It indexes a folder in two stages, words first and then
//! meaning, and searches by both. The front ends hold no indexing or search
//! logic of their own, so both behave as the evaluation measured.

mod index;
mod search;

use std::sync::{Mutex, MutexGuard, PoisonError};

pub use catchword_embed::Threads;
use catchword_embed::{Embed, Embedder, DEFAULT_MODEL};

pub use index::{
    embed_missing, index_folder, is_parked, Cutter, EmbedReport, Report, Unreachable, Worker,
    ATTEMPTS_BEFORE_PARKING, COPIES_FROM,
};
pub use search::{group_by_file, search, search_within, search_words, Answer, FileResults, Note};

/// The embedding model, or why meaning search is off.
pub enum Model {
    /// Locked for one passage or query at a time, so a search waits at most
    /// for one passage of background embedding.
    Ready(Mutex<Box<dyn Embed>>),
    Unavailable(String),
}

impl Model {
    /// Load the app's model from where it is installed (or, in development,
    /// from vendor/), to run on `threads`. Never fails: without a model,
    /// keyword search works.
    pub fn load(threads: Threads) -> Model {
        let Some(paths) = catchword_embed::find(&DEFAULT_MODEL) else {
            return Model::Unavailable(
                "the embedding model or ONNX Runtime was not found \
                 (in development, run sh scripts/fetch-embedding.sh)"
                    .to_string(),
            );
        };
        match Embedder::load(&paths, &DEFAULT_MODEL, threads) {
            Ok(model) => Model::Ready(Mutex::new(Box::new(model))),
            Err(error) => Model::Unavailable(error.to_string()),
        }
    }

    pub fn ready(&self) -> Option<&Mutex<Box<dyn Embed>>> {
        match self {
            Model::Ready(model) => Some(model),
            Model::Unavailable(_) => None,
        }
    }
}

/// Copies of the model for a long meaning stage (ADR-25). Each embeds its
/// own passages at the same time as the others, on its own share of the
/// threads, while the shared model stays free for searches. They are made
/// from the shared model when the stage starts, share its tokenizer, and
/// are dropped when the stage ends.
#[derive(Debug, Clone, Copy)]
pub struct Copies {
    /// How many to make. Fewer than two are not worth it: the shared model
    /// alone is as fast.
    pub count: usize,
    /// The threads each copy runs on. Their `background` function is also
    /// called first on each thread that embeds with a copy.
    pub threads: Threads,
}

impl Copies {
    /// No copies: the shared model embeds alone.
    pub const NONE: Copies = Copies {
        count: 0,
        threads: Threads::RUNTIME_DEFAULT,
    };
}

/// Lock the model. A panic while it was locked cannot leave it half
/// changed, so the lock is used anyway.
fn lock(model: &Mutex<Box<dyn Embed>>) -> MutexGuard<'_, Box<dyn Embed>> {
    model.lock().unwrap_or_else(PoisonError::into_inner)
}
