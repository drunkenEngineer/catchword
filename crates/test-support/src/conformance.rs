//! Shared conformance suites (MNT-2): what every extractor and every
//! embedding model must do, whatever is inside it. Each implementation's
//! own tests call the suite for it, so a new one is held to the same terms.
//!
//! The index store has one implementation by design (ADR-3); its contract
//! is its public API, and its tests are the suite (ADR-21).

use std::fs;

use catchword_embed::Embed;
use catchword_engine::extract::{Extractor, Limits, Outcome, Reason};

use crate::scratch_folder;

/// An extractor for files ending in `extension`, which `make` writes from
/// text. It must:
/// - return the text, with no control characters but tab and line breaks,
///   and the same text each time;
/// - say a missing file cannot be opened, and a file over the size limit
///   is too large;
/// - stop, without reading, if asked to stop before it starts.
pub fn extractor(extractor: &dyn Extractor, extension: &str, make: &dyn Fn(&str) -> Vec<u8>) {
    let folder = scratch_folder(&format!("conformance-{extension}"));
    let limits = Limits::default();
    let file = folder.join(format!("letter.{extension}"));
    fs::write(&file, make("Your tax refund was approved \u{1b}[2J today")).unwrap();

    let first = extractor.extract(&file, &limits, &mut || true).unwrap();
    let Outcome::Pages(pages) = &first else {
        panic!("no text: {first:?}");
    };
    let text = pages.concat();
    assert!(text.contains("tax refund was approved"), "{text:?}");
    assert!(
        text.chars()
            .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r')),
        "control characters in {text:?}"
    );
    if !extractor.paged() {
        assert_eq!(pages.len(), 1, "text without pages comes as one");
    }
    assert_eq!(
        extractor.extract(&file, &limits, &mut || true).unwrap(),
        first
    );

    let missing = folder.join(format!("gone.{extension}"));
    assert_eq!(
        extractor.extract(&missing, &limits, &mut || true).unwrap(),
        Outcome::NotIndexed(Reason::CannotOpen)
    );
    let tiny = Limits {
        max_file_bytes: 10,
        ..Limits::default()
    };
    assert_eq!(
        extractor.extract(&file, &tiny, &mut || true).unwrap(),
        Outcome::NotIndexed(Reason::TooLarge)
    );
    assert_eq!(
        extractor.extract(&file, &limits, &mut || false).unwrap(),
        Outcome::Stopped
    );
}

/// An embedding model. It must:
/// - give one vector per passage, of the manifest's size and unit length;
/// - give the same vector for a passage alone as in a batch, and each time;
/// - give a query a vector nearer the passage that answers it;
/// - count tokens for any text, at its own word boundaries.
pub fn embedder(model: &mut dyn Embed) {
    let size = model.manifest().dimensions;
    let passages = [
        "The notice period for the lease is three months.",
        "Invoice INV-7 is due within thirty days of delivery.",
    ];
    let together = model.embed_passages(&passages).unwrap();
    assert_eq!(together.len(), passages.len());
    for vector in &together {
        assert_eq!(vector.len(), size);
        assert_unit(vector);
    }
    for (passage, in_batch) in passages.iter().zip(&together) {
        let alone = model.embed_passages(&[passage]).unwrap().remove(0);
        assert_close(&alone, in_batch);
        let again = model.embed_passages(&[passage]).unwrap().remove(0);
        assert_close(&again, &alone);
    }

    let query = model
        .embed_query("how long is the notice period of the lease")
        .unwrap();
    assert_eq!(query.len(), size);
    assert_unit(&query);
    assert!(
        dot(&query, &together[0]) > dot(&query, &together[1]),
        "the query is not nearer the passage that answers it"
    );

    let text = "notice period";
    let ends = model.tokenizer().token_ends(text);
    assert!(!ends.is_empty());
    assert!(ends.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(ends.iter().all(|&end| end <= text.len()));
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn assert_unit(vector: &[f32]) {
    let length = dot(vector, vector).sqrt();
    assert!((length - 1.0).abs() < 1e-3, "length {length}");
}

fn assert_close(a: &[f32], b: &[f32]) {
    let apart: f32 = a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum();
    assert!(apart < 1e-3, "vectors differ by {apart}");
}
