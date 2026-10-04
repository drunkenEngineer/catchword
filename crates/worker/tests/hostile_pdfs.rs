//! Hostile PDFs (REL-2, threat T1): a valid PDF cut short and corrupted in
//! hundreds of ways, read by the real worker with PDFium. Each must end as
//! text or as a reason, within the time limit; the supervisor must never
//! fail, whatever the worker does. The variants come from a seeded
//! generator, so a failure repeats exactly.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use catchword_engine::extract::{run, Limits, Outcome};
use catchword_test_support::{pdf, scratch_folder};

fn worker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_catchword-worker"))
}

#[test]
fn damaged_pdfs_end_as_text_or_a_reason_never_a_failure() {
    let folder = scratch_folder("hostile-pdfs");
    let good = pdf(&[
        "Dear customer, your tax refund was approved.",
        "It will be paid within ten working days.",
        "Kind regards",
    ]);
    let limits = Limits {
        timeout: Duration::from_secs(10),
        ..Limits::default()
    };
    let mut state: u64 = 11;
    let mut next = |below: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % below.max(1)
    };

    let mut variants: Vec<Vec<u8>> = Vec::new();
    // Cut short at forty places.
    for step in 1..=40 {
        variants.push(good[..good.len() * step / 41].to_vec());
    }
    // Bytes changed at random: one, a few, or many.
    for flips in [1, 4, 32] {
        for _ in 0..30 {
            let mut bad = good.clone();
            for _ in 0..flips {
                let at = next(bad.len());
                bad[at] = next(256) as u8;
            }
            variants.push(bad);
        }
    }
    // Parts repeated or swapped, as in a file stitched together wrongly.
    for _ in 0..20 {
        let (a, b) = (next(good.len()), next(good.len()));
        let (start, end) = (a.min(b), a.max(b));
        let mut stitched = good[..end].to_vec();
        stitched.extend_from_slice(&good[start..]);
        variants.push(stitched);
    }

    let started = Instant::now();
    let mut read = 0;
    let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
    for (number, bytes) in variants.iter().enumerate() {
        let file = folder.join(format!("variant-{number:03}.pdf"));
        fs::write(&file, bytes).unwrap();
        let outcome = run(worker(), &file, &limits)
            .unwrap_or_else(|error| panic!("variant {number}: the supervisor failed: {error}"));
        match outcome {
            Outcome::Pages(_) => read += 1,
            other => *reasons.entry(format!("{other:?}")).or_default() += 1,
        }
    }
    println!(
        "{} variants: {read} read; not indexed: {reasons:?}",
        variants.len()
    );
    // Some damage still reads, as PDFium repairs what it can; some does not.
    // On 4 October 2026: 73 read, 74 damaged, 3 without text; no crash or timeout.
    assert!(read > 0);
    assert!(read < variants.len());
    assert!(
        started.elapsed() < Duration::from_secs(300),
        "{:?}",
        started.elapsed()
    );
}
