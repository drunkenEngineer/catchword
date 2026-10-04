//! Text files meet the extractor contract (MNT-2).

use catchword_engine::extract::TextFiles;
use catchword_test_support::conformance;

#[test]
fn text_files_meet_the_extractor_contract() {
    conformance::extractor(&TextFiles, "txt", &|text| text.as_bytes().to_vec());
}
