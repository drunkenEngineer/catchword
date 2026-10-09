//! Catchword extraction worker.
//!
//! The engine starts this program for one file at a time, under the limits in
//! docs/adr/0016-worker-limits.md. It reads one request from standard input,
//! extracts the text of each page with PDFium, writes one response to standard
//! output, and exits. It has no network code and never writes to any file.

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use catchword_engine::extract::protocol::{self, Refusal, Request, Response};
use pdfium_render::prelude::{Pdfium, PdfiumError, PdfiumInternalError};

fn main() -> ExitCode {
    // First, before anything untrusted is read: give up the right to write
    // to the user's files, settings and other programs (SEC-6). Windows
    // lets a process lower its integrity level, never raise it.
    #[cfg(windows)]
    catchword_engine::extract::integrity::lower_this_process();

    // Nothing else happens before the request arrives: by then the engine
    // has put this process under its limits.
    let request = match protocol::read_request(&mut io::stdin().lock()) {
        Ok(request) => request,
        Err(_) => return ExitCode::from(2),
    };
    let response = extract(&request);
    match protocol::write_response(&mut io::stdout().lock(), &response) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(3),
    }
}

fn extract(request: &Request) -> Response {
    let Some(pdfium) = load_pdfium() else {
        return Response::Refused(Refusal::LibraryMissing);
    };
    let document = match pdfium.load_pdf_from_file(&request.path, None) {
        Ok(document) => document,
        Err(error) => return Response::Refused(refusal_for(&error)),
    };
    let pages = document.pages();
    let count = pages.len();
    if u32::try_from(count).map_or(true, |count| count > request.max_pages) {
        return Response::Refused(Refusal::TooLarge);
    }

    let mut texts = Vec::new();
    let mut total_bytes = 0usize;
    // By index, so a page that cannot be read stays in place, empty, and the
    // pages after it keep their numbers.
    for index in 0..count {
        let text = pages
            .get(index)
            .and_then(|page| page.text().map(|text| text.all()))
            .unwrap_or_default();
        total_bytes += text.len();
        if total_bytes > request.max_text_bytes as usize {
            return Response::Refused(Refusal::TooLarge);
        }
        texts.push(text);
    }
    Response::Pages(texts)
}

fn refusal_for(error: &PdfiumError) -> Refusal {
    match error {
        PdfiumError::IoError(_)
        | PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FileError) => {
            Refusal::CannotOpen
        }
        PdfiumError::PdfiumLibraryInternalError(
            PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError,
        ) => Refusal::Encrypted,
        _ => Refusal::Damaged,
    }
}

/// Load PDFium by full path from known folders only, so a copy planted
/// anywhere else is never picked up (threat T17).
fn load_pdfium() -> Option<Pdfium> {
    library_folders().into_iter().find_map(|folder| {
        let library = Pdfium::pdfium_platform_library_name_at_path(&folder);
        Pdfium::bind_to_library(library).ok().map(Pdfium::new)
    })
}

/// The worker's own folder; in debug builds also vendor/pdfium, where
/// scripts/fetch-pdfium.sh puts the library for development and tests.
fn library_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(folder) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        folders.push(folder);
    }
    if cfg!(debug_assertions) {
        folders.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium"));
    }
    folders
}
