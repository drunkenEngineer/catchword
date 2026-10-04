//! Hostile input (REL-2): thousands of generated inputs for everything that
//! reads data from outside, which must never panic, hang or misbehave. The
//! inputs come from a seeded generator, so a failure repeats exactly.

use catchword_engine::exclude::{check_pattern, matches};
use catchword_engine::extract::protocol::{read_request, read_response, write_response, Response};
use catchword_engine::{chunk_while, decode_text, without_controls, Tokenizer, WordTokenizer};

/// A small, seeded random generator: the same seed, the same inputs.
struct Generator(u64);

impl Generator {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }

    fn bytes(&mut self, most: usize) -> Vec<u8> {
        let length = self.below(most + 1);
        (0..length).map(|_| self.next() as u8).collect()
    }

    /// Text mixing the awkward: quotes, operators, brackets, control
    /// characters, right-to-left script, combining marks, emoji.
    fn text(&mut self, most: usize) -> String {
        const PIECES: [&str; 24] = [
            "lease",
            "\"",
            "\u{201c}",
            "*",
            "(",
            ")",
            "AND",
            "OR",
            "NOT",
            "NEAR",
            ":",
            "^",
            "-",
            "\u{1b}[2J",
            "\0",
            "\t",
            "\n",
            " ",
            "عقد",
            "e\u{301}",
            "\u{1f600}",
            "\\",
            "'",
            "{}",
        ];
        let count = self.below(most + 1);
        (0..count)
            .map(|_| PIECES[self.below(PIECES.len())])
            .collect()
    }
}

#[test]
fn worker_answers_of_any_shape_are_refused_without_panic() {
    let mut random = Generator(1);
    for _ in 0..5_000 {
        let bytes = random.bytes(64);
        let _ = read_response(&mut bytes.as_slice(), 1 << 20, 100);
        let _ = read_request(&mut bytes.as_slice());
    }
    // A good answer, cut or corrupted at every byte, is refused or read,
    // never a panic.
    let mut good = Vec::new();
    write_response(
        &mut good,
        &Response::Pages(vec!["page one".into(), "two".into()]),
    )
    .unwrap();
    for cut in 0..good.len() {
        let _ = read_response(&mut &good[..cut], 1 << 20, 100);
        let mut corrupted = good.clone();
        corrupted[cut] ^= 0xFF;
        let _ = read_response(&mut corrupted.as_slice(), 1 << 20, 100);
    }
    assert!(matches!(
        read_response(&mut good.as_slice(), 1 << 20, 100),
        Ok(Response::Pages(_))
    ));
}

#[test]
fn any_bytes_decode_to_text_without_control_characters() {
    let mut random = Generator(2);
    for _ in 0..3_000 {
        let bytes = random.bytes(300);
        let text = without_controls(&decode_text(&bytes));
        assert!(text
            .chars()
            .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r')));
    }
}

#[test]
fn any_text_is_cut_into_passages_within_their_size() {
    let mut random = Generator(3);
    for _ in 0..2_000 {
        let text = random.text(80);
        let passages = chunk_while(&text, 12, 3, &WordTokenizer, &mut || true).unwrap();
        for passage in &passages {
            assert!(!passage.text.trim().is_empty());
            assert!(passage.start_line <= passage.end_line);
            // No passage over the limit, unless one word alone is longer.
            let tokens = WordTokenizer.token_ends(&passage.text).len();
            assert!(tokens <= 12 || passage.text.split_whitespace().count() == 1);
        }
    }
}

#[test]
fn name_patterns_never_panic_or_take_long() {
    let mut random = Generator(4);
    let started = std::time::Instant::now();
    for _ in 0..5_000 {
        let pattern = random.text(12);
        let name = random.text(30);
        let _ = check_pattern(&pattern);
        let _ = matches(&pattern, &name);
    }
    // The worst case for a matcher with stars.
    let stars = "*a".repeat(50) + "b";
    let name = "a".repeat(5_000);
    assert!(!matches(&stars, &name));
    assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());
}
