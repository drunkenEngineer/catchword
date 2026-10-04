//! Catchword service: the use cases the command-line tool and the desktop
//! app share. It indexes a folder in two stages, words first and then
//! meaning, and searches by both. The front ends hold no indexing or search
//! logic of their own, so both behave as the evaluation measured.

mod index;
mod search;

use std::sync::{Mutex, MutexGuard, PoisonError};

use catchword_embed::{Embedder, DEFAULT_MODEL};

pub use index::{
    embed_missing, index_folder, is_parked, Cutter, EmbedReport, Report, Unreachable, Worker,
    ATTEMPTS_BEFORE_PARKING,
};
pub use search::{group_by_file, search, Answer, FileResults, Note};

/// The embedding model, or why meaning search is off.
pub enum Model {
    /// Locked for one passage or query at a time, so a search waits at most
    /// for one passage of background embedding.
    Ready(Mutex<Embedder>),
    Unavailable(String),
}

impl Model {
    /// Load the app's model from where it is installed (or, in development,
    /// from vendor/). Never fails: without a model, keyword search works.
    pub fn load() -> Model {
        let Some(paths) = catchword_embed::find(&DEFAULT_MODEL) else {
            return Model::Unavailable(
                "the embedding model or ONNX Runtime was not found \
                 (in development, run sh scripts/fetch-embedding.sh)"
                    .to_string(),
            );
        };
        match Embedder::load(&paths, &DEFAULT_MODEL) {
            Ok(model) => Model::Ready(Mutex::new(model)),
            Err(error) => Model::Unavailable(error.to_string()),
        }
    }

    pub fn ready(&self) -> Option<&Mutex<Embedder>> {
        match self {
            Model::Ready(model) => Some(model),
            Model::Unavailable(_) => None,
        }
    }
}

/// Lock the model. A panic while it was locked cannot leave it half
/// changed, so the lock is used anyway.
fn lock(model: &Mutex<Embedder>) -> MutexGuard<'_, Embedder> {
    model.lock().unwrap_or_else(PoisonError::into_inner)
}
