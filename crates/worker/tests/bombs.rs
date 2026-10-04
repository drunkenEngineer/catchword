//! Files made to exhaust resources (threat T2), read by the real worker:
//! a decompression bomb, deep nesting, and a false page count. Each must end
//! as a reason within the limits; nothing may hang or fail the supervisor.
//! The files are generated here, not committed.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use catchword_engine::extract::{run, Limits, Outcome};
use catchword_test_support::scratch_folder;

fn worker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_catchword-worker"))
}

/// A PDF of one page whose content stream is `content`, with `filter` (such
/// as "/Filter /FlateDecode") in its dictionary. `pages_count` is what the
/// page tree claims, true or not.
fn pdf_with_content(content: &[u8], filter: &str, pages_count: u64) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    let mut object = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    object(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(
        &mut out,
        format!("<< /Type /Pages /Kids [3 0 R] /Count {pages_count} >>").as_bytes(),
    );
    object(
        &mut out,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>",
    );
    let mut stream = format!("<< /Length {} {filter} >>\nstream\n", content.len()).into_bytes();
    stream.extend_from_slice(content);
    stream.extend_from_slice(b"\nendstream");
    object(&mut out, &stream);
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// A zlib stream that expands to `1 + copies * 258` zero bytes: one literal
/// zero, then "copy 258 bytes from 1 back" again and again, in a deflate
/// block with the fixed codes (RFC 1951). About 160 to 1.
fn zero_bomb(copies: usize) -> Vec<u8> {
    struct Bits {
        bytes: Vec<u8>,
        used: u32,
    }
    impl Bits {
        fn push(&mut self, bit: bool) {
            if self.used == 0 {
                self.bytes.push(0);
            }
            if bit {
                *self.bytes.last_mut().unwrap() |= 1 << self.used;
            }
            self.used = (self.used + 1) % 8;
        }
        /// A Huffman code goes in from its first, highest bit.
        fn code(&mut self, value: u32, length: u32) {
            for bit in (0..length).rev() {
                self.push(value >> bit & 1 == 1);
            }
        }
    }
    let mut bits = Bits {
        bytes: Vec::new(),
        used: 0,
    };
    bits.push(true); // the last block
    bits.push(true); // fixed codes: type 01, low bit first
    bits.push(false);
    bits.code(0x30, 8); // the literal byte 0
    for _ in 0..copies {
        bits.code(0xC5, 8); // length 258 (symbol 285)
        bits.code(0, 5); // distance 1
    }
    bits.code(0, 7); // end of block
    let total = 1 + copies as u64 * 258;
    // Adler-32 of that many zeros: a stays 1, b adds 1 per byte.
    let adler: u32 = (((total % 65_521) as u32) << 16) | 1;
    let mut zlib = vec![0x78, 0x01];
    zlib.extend_from_slice(&bits.bytes);
    zlib.extend_from_slice(&adler.to_be_bytes());
    zlib
}

fn read(folder: &Path, name: &str, bytes: &[u8], limits: &Limits) -> (Outcome, Duration) {
    let file = folder.join(name);
    fs::write(&file, bytes).unwrap();
    let started = Instant::now();
    let outcome = run(worker(), &file, limits)
        .unwrap_or_else(|error| panic!("{name}: the supervisor failed: {error}"));
    (outcome, started.elapsed())
}

#[test]
fn files_made_to_exhaust_resources_end_as_reasons_within_the_limits() {
    let folder = scratch_folder("bombs");
    let limits = Limits {
        timeout: Duration::from_secs(20),
        ..Limits::default()
    };
    // Well under the 20 s limit plus the time to stop a worker.
    let in_time = |elapsed: Duration| elapsed < Duration::from_secs(25);

    // About 600 MB of zeros from under 4 MB: more than the worker's 512 MB.
    let bomb = pdf_with_content(&zero_bomb(2_400_000), "/Filter /FlateDecode", 1);
    let (outcome, elapsed) = read(&folder, "bomb.pdf", &bomb, &limits);
    println!("decompression bomb: {outcome:?} in {elapsed:?}");
    assert!(in_time(elapsed), "{elapsed:?}");
    assert!(!matches!(outcome, Outcome::Pages(_)), "{outcome:?}");
    // On Windows the job object stops it at 512 MB (ADR-16). Elsewhere
    // there is no memory limit yet (spec: 1.0); the zeros then read as a
    // page without text.
    #[cfg(windows)]
    assert_eq!(
        outcome,
        Outcome::NotIndexed(catchword_engine::extract::Reason::MemoryLimit)
    );

    // 100,000 arrays, one inside the next.
    let mut nested = b"BT /F1 12 Tf ".to_vec();
    nested.extend(std::iter::repeat_n(b'[', 100_000));
    nested.extend(std::iter::repeat_n(b']', 100_000));
    nested.extend_from_slice(b" TJ ET");
    let (outcome, elapsed) = read(
        &folder,
        "nested.pdf",
        &pdf_with_content(&nested, "", 1),
        &limits,
    );
    println!("deep nesting: {outcome:?} in {elapsed:?}");
    assert!(in_time(elapsed), "{elapsed:?}");

    // A page tree that claims a billion pages.
    let content = b"BT /F1 12 Tf 72 720 Td (one real page) Tj ET";
    let (outcome, elapsed) = read(
        &folder,
        "count.pdf",
        &pdf_with_content(content, "", 1_000_000_000),
        &limits,
    );
    println!("false page count: {outcome:?} in {elapsed:?}");
    assert!(in_time(elapsed), "{elapsed:?}");
}

#[test]
fn the_bomb_is_what_it_claims_to_be() {
    // Check the generator itself: a small bomb, read back by hand.
    let zlib = zero_bomb(3);
    assert_eq!(&zlib[..2], &[0x78, 0x01]);
    assert_eq!(u16::from_be_bytes([zlib[0], zlib[1]]) % 31, 0);
    let total = 1 + 3 * 258u32;
    let adler = u32::from_be_bytes(zlib[zlib.len() - 4..].try_into().unwrap());
    assert_eq!(adler, (total << 16) | 1);
}
