//! Search speed and index size at the spec's reference size (section 14),
//! with synthetic passages: random words and random unit vectors. No model
//! is involved, so this measures the store alone.

use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use catchword_engine::Passage;
use catchword_store::Store;

const DIMENSIONS: usize = 384;
const WORDS_PER_PASSAGE: usize = 60;
const PASSAGES_PER_FILE: usize = 25;
const VOCABULARY: usize = 20_000;
const QUERIES: usize = 50;

/// A small, fixed random number generator (xorshift), so every run
/// builds the same index.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn unit_vector(&mut self) -> Vec<f32> {
        let raw: Vec<f32> = (0..DIMENSIONS)
            .map(|_| (self.next() % 2001) as f32 / 1000.0 - 1.0)
            .collect();
        let length = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
        raw.iter().map(|x| x / length).collect()
    }
}

/// A made-up word for each number: "ka", "kb", ... all distinct.
fn word(number: usize) -> String {
    let mut text = String::from("k");
    let mut rest = number;
    loop {
        text.push((b'a' + (rest % 26) as u8) as char);
        rest /= 26;
        if rest == 0 {
            break text;
        }
    }
}

/// Build an index of `passages` synthetic passages in `folder`, time the
/// three kinds of search, and report the timings and the index size.
pub fn run(passages: usize, folder: &Path) -> Result<String> {
    fs::create_dir_all(folder)?;
    let path = folder.join("scale.db");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", path.display()));
    }
    let vocabulary: Vec<String> = (0..VOCABULARY).map(word).collect();
    let mut random = Random(0x9E37_79B9_7F4A_7C15);

    let started = Instant::now();
    let mut store = Store::open(&path)?;
    store.use_model("synthetic", DIMENSIONS)?;
    let files = passages.div_ceil(PASSAGES_PER_FILE);
    for file in 0..files {
        let cut: Vec<Passage> = (0..PASSAGES_PER_FILE)
            .map(|ordinal| Passage {
                ordinal: ordinal as u32,
                page: None,
                start_line: ordinal as u32 + 1,
                end_line: ordinal as u32 + 1,
                text: (0..WORDS_PER_PASSAGE)
                    .map(|_| vocabulary[random.below(VOCABULARY)].as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            })
            .collect();
        store.put_file(
            &format!("/scale/{file}.txt"),
            1,
            0,
            &format!("h{file}"),
            &cut,
        )?;
    }
    let keyword_seconds = started.elapsed().as_secs_f64();

    // A fresh index numbers passages from 1, in order.
    let started = Instant::now();
    let total = files * PASSAGES_PER_FILE;
    for first in (1..=total).step_by(5_000) {
        let vectors: Vec<(i64, Vec<f32>)> = (first..(first + 5_000).min(total + 1))
            .map(|id| (id as i64, random.unit_vector()))
            .collect();
        store.put_vectors(&vectors)?;
    }
    let vector_seconds = started.elapsed().as_secs_f64();

    let mut words = Vec::new();
    let mut meaning = Vec::new();
    let mut combined = Vec::new();
    for _ in 0..QUERIES {
        let query = format!(
            "{} {}",
            vocabulary[random.below(VOCABULARY)],
            vocabulary[random.below(VOCABULARY)]
        );
        let vector = random.unit_vector();
        let started = Instant::now();
        store.search_keyword(&query, 10)?;
        words.push(started.elapsed().as_secs_f64());
        let started = Instant::now();
        store.search_vector(&vector, 10)?;
        meaning.push(started.elapsed().as_secs_f64());
        let started = Instant::now();
        store.search_combined(&query, Some(&vector), 50)?;
        combined.push(started.elapsed().as_secs_f64());
    }
    drop(store);
    let bytes = fs::metadata(&path)?.len()
        + fs::metadata(format!("{}-wal", path.display()))
            .map(|m| m.len())
            .unwrap_or(0);

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{total} synthetic passages of {WORDS_PER_PASSAGE} words, {DIMENSIONS}-number vectors.\n"
    );
    let _ = writeln!(out, "| Measure | Result |");
    let _ = writeln!(out, "| --- | ---: |");
    let _ = writeln!(
        out,
        "| Keyword search, median | {} |",
        ms(percentile(&mut words, 0.5))
    );
    let _ = writeln!(
        out,
        "| Keyword search, 95th percentile | {} |",
        ms(percentile(&mut words, 0.95))
    );
    let _ = writeln!(
        out,
        "| Vector search, median | {} |",
        ms(percentile(&mut meaning, 0.5))
    );
    let _ = writeln!(
        out,
        "| Vector search, 95th percentile | {} |",
        ms(percentile(&mut meaning, 0.95))
    );
    let _ = writeln!(
        out,
        "| Combined search (50 + 50 candidates), median | {} |",
        ms(percentile(&mut combined, 0.5))
    );
    let _ = writeln!(
        out,
        "| Combined search, 95th percentile | {} |",
        ms(percentile(&mut combined, 0.95))
    );
    let _ = writeln!(
        out,
        "| Index size | {:.0} MB, {:.1} KB a passage |",
        bytes as f64 / 1e6,
        bytes as f64 / 1e3 / total as f64
    );
    let _ = writeln!(
        out,
        "| Writing passages and keyword index | {keyword_seconds:.0} s |"
    );
    let _ = writeln!(out, "| Writing vectors | {vector_seconds:.0} s |");
    Ok(out)
}

fn percentile(values: &mut [f64], share: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    let index = ((values.len() as f64 * share).ceil() as usize).clamp(1, values.len()) - 1;
    values[index]
}

fn ms(seconds: f64) -> String {
    format!("{:.0} ms", 1000.0 * seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn made_up_words_are_all_different() {
        let words: std::collections::HashSet<String> = (0..VOCABULARY).map(word).collect();
        assert_eq!(words.len(), VOCABULARY);
        assert_eq!(word(0), "ka");
        assert_eq!(word(27), "kbb");
    }

    #[test]
    fn percentiles_pick_from_the_sorted_values() {
        let mut values: Vec<f64> = (1..=100).map(f64::from).collect();
        values.reverse();
        assert_eq!(percentile(&mut values, 0.95), 95.0);
        assert_eq!(percentile(&mut values, 0.5), 50.0);
    }

    #[test]
    fn a_small_index_reports_every_measure() {
        let folder = std::env::temp_dir().join("catchword-eval-test-scale");
        let report = run(100, &folder).unwrap();
        assert!(report.contains("100 synthetic passages"), "{report}");
        assert!(
            report.contains("Combined search, 95th percentile"),
            "{report}"
        );
        fs::remove_dir_all(&folder).unwrap();
    }
}
