//! The embedding runtime with the real model and ONNX Runtime. Needs
//! `sh scripts/fetch-embedding.sh` to have been run once.

use std::sync::{Mutex, OnceLock};

use catchword_embed::{find, Embedder, ModelManifest, E5_SMALL, GRANITE_97M};
use catchword_engine::{chunk, Tokenizer};

/// The model takes a second or two to load, so the tests share one.
fn embedder() -> &'static Mutex<Embedder> {
    static EMBEDDER: OnceLock<Mutex<Embedder>> = OnceLock::new();
    EMBEDDER.get_or_init(|| {
        let paths =
            find(&GRANITE_97M).expect("no model or runtime; run sh scripts/fetch-embedding.sh");
        Mutex::new(Embedder::load(&paths, &GRANITE_97M).unwrap())
    })
}

fn passages(texts: &[&str]) -> Vec<Vec<f32>> {
    embedder().lock().unwrap().embed_passages(texts).unwrap()
}

fn query(text: &str) -> Vec<f32> {
    embedder().lock().unwrap().embed_query(text).unwrap()
}

/// Vectors have unit length, so cosine similarity is their dot product.
fn similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[test]
fn vectors_have_the_model_size_and_unit_length() {
    for vector in passages(&["A short passage.", "Another, slightly longer passage."]) {
        assert_eq!(vector.len(), GRANITE_97M.dimensions);
        let length: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((length - 1.0).abs() < 1e-4, "length {length}");
    }
}

#[test]
fn related_meaning_scores_higher_than_unrelated() {
    let q = query("When will my tax refund arrive?");
    let found = passages(&[
        "Your income tax reimbursement was approved and will be paid next week.",
        "The cat slept on the warm windowsill all afternoon.",
    ]);
    let (related, unrelated) = (similarity(&q, &found[0]), similarity(&q, &found[1]));
    assert!(related > unrelated + 0.1, "{related} vs {unrelated}");
}

#[test]
fn a_query_finds_a_passage_in_another_language() {
    let q = query("When will my tax refund arrive?");
    let found = passages(&[
        "Votre remboursement d'impôt a été approuvé et sera versé la semaine prochaine.",
        "Le chat a dormi tout l'après-midi sur le rebord de la fenêtre.",
    ]);
    let (related, unrelated) = (similarity(&q, &found[0]), similarity(&q, &found[1]));
    assert!(related > unrelated + 0.1, "{related} vs {unrelated}");
}

#[test]
fn a_passage_gets_the_same_vector_whatever_it_is_embedded_with() {
    // The 8-bit model sets its rounding from everything in one run, so
    // passages must not be run together or padded to each other's length.
    let alone = passages(&["The notice period is three months."]);
    let long = "Terms and conditions of the lease agreement. ".repeat(40);
    let together = passages(&[long.as_str(), "The notice period is three months."]);
    let same = similarity(&alone[0], &together[1]);
    assert!(same > 0.9999, "similarity {same}");
}

#[test]
fn the_same_text_always_gets_the_same_vector() {
    let first = query("invoice INV-2026-0042");
    let second = query("invoice INV-2026-0042");
    assert!(similarity(&first, &second) > 0.9999);
}

#[test]
fn a_very_long_input_is_cut_not_refused() {
    let long = "word ".repeat(5_000);
    let vector = passages(&[long.as_str()]).remove(0);
    assert_eq!(vector.len(), GRANITE_97M.dimensions);
}

#[test]
fn token_ends_are_byte_offsets_in_order() {
    let text = "Hello wonderful world, résumé";
    let ends = embedder().lock().unwrap().token_ends(text);
    assert!(ends.len() >= 4, "{ends:?}");
    assert!(ends.windows(2).all(|pair| pair[0] < pair[1]), "{ends:?}");
    assert_eq!(*ends.last().unwrap(), text.len());
    assert!(ends.iter().all(|&end| text.is_char_boundary(end)));
}

#[test]
fn passages_cut_with_the_model_tokenizer_fit_the_limit() {
    let text = "The landlord may increase the rent once a year, with written notice. \
                Le locataire doit donner un préavis de trois mois. "
        .repeat(60);
    let model = embedder().lock().unwrap();
    let cut = chunk(&text, 64, 8, &*model);
    assert!(cut.len() > 5);
    for passage in &cut {
        let tokens = model.token_ends(&passage.text).len();
        assert!(tokens <= 64, "{tokens} tokens: {}", passage.text);
    }
}

// The baseline model, used by the benchmark. Needs `sh scripts/fetch-eval.sh`.

fn baseline() -> &'static Mutex<Embedder> {
    static EMBEDDER: OnceLock<Mutex<Embedder>> = OnceLock::new();
    EMBEDDER.get_or_init(|| load(&E5_SMALL, "sh scripts/fetch-eval.sh"))
}

fn load(manifest: &ModelManifest, fetch: &str) -> Mutex<Embedder> {
    let paths = find(manifest).unwrap_or_else(|| panic!("model not found; run {fetch}"));
    Mutex::new(Embedder::load(&paths, manifest).unwrap())
}

#[test]
fn the_baseline_model_ranks_by_meaning_across_languages() {
    let mut model = baseline().lock().unwrap();
    let q = model
        .embed_query("When will my tax refund arrive?")
        .unwrap();
    let found = model
        .embed_passages(&[
            "Votre remboursement d'impôt a été approuvé et sera versé la semaine prochaine.",
            "Le chat a dormi tout l'après-midi sur le rebord de la fenêtre.",
        ])
        .unwrap();
    assert_eq!(q.len(), E5_SMALL.dimensions);
    let length: f32 = q.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((length - 1.0).abs() < 1e-4);
    let (related, unrelated) = (similarity(&q, &found[0]), similarity(&q, &found[1]));
    assert!(related > unrelated, "{related} vs {unrelated}");
}

#[test]
fn the_baseline_model_marks_queries_and_passages_differently() {
    // e5 reads "query: " and "passage: " prefixes, so the same words get
    // different vectors depending on their role.
    let mut model = baseline().lock().unwrap();
    let as_query = model.embed_query("notice period").unwrap();
    let as_passage = model.embed_passages(&["notice period"]).unwrap().remove(0);
    let same = similarity(&as_query, &as_passage);
    assert!(same < 0.9999, "similarity {same}");
}
