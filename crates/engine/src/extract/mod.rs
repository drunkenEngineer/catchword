//! Text extraction in a separate worker process.
//!
//! Untrusted files are never parsed inside the engine. A worker program reads
//! one file and sends its text back; the engine treats that answer as
//! untrusted and checks it before using it.

pub mod protocol;
