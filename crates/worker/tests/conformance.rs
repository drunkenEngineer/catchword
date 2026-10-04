//! The PDF reader, with the real worker and PDFium, meets the extractor
//! contract (MNT-2).

use std::path::PathBuf;

use catchword_engine::extract::PdfReader;
use catchword_test_support::{conformance, pdf};

#[test]
fn the_pdf_reader_meets_the_extractor_contract() {
    let reader = PdfReader {
        program: PathBuf::from(env!("CARGO_BIN_EXE_catchword-worker")),
    };
    conformance::extractor(&reader, "pdf", &|text| pdf(&[text]));
}
