//! Worker protocol, version 1: how the engine and the extraction worker talk.
//!
//! The engine writes one request to the worker's standard input and reads one
//! response from its standard output. Each message is a frame: a 4-byte
//! little-endian length, then that many bytes of payload. A payload starts
//! with the protocol version (2 bytes) and a kind (1 byte).
//!
//! The worker parses untrusted files, so its output is untrusted too.
//! `read_response` checks every length against a limit before trusting it,
//! requires valid UTF-8, and rejects anything malformed or left over.
//! Page numbers are not sent: the first page in a response is page 1, so they
//! cannot be out of order. See docs/adr/0015-worker-protocol.md.

use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub const VERSION: u16 = 1;

/// A request is a path and two numbers. 64 KiB holds the longest Windows path.
pub const MAX_REQUEST_BYTES: u32 = 64 * 1024;

const KIND_EXTRACT: u8 = 1;
const KIND_PAGES: u8 = 1;
const KIND_REFUSED: u8 = 2;

/// Extract the text of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub path: PathBuf,
    /// Refuse with `TooLarge` above this many pages.
    pub max_pages: u32,
    /// Refuse with `TooLarge` above this many bytes of text.
    pub max_text_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// The text of each page, in order: the first entry is page 1.
    Pages(Vec<String>),
    /// The worker could not or would not extract the file.
    Refused(Refusal),
}

/// Why the worker returned no text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A password is needed to open the file.
    Encrypted,
    /// More pages or text than the request allows.
    TooLarge,
    /// The file is not a readable PDF.
    Damaged,
    /// The file could not be opened, for example because access was denied.
    CannotOpen,
    /// The PDFium library could not be loaded: an installation problem.
    LibraryMissing,
}

impl Refusal {
    fn code(self) -> u8 {
        match self {
            Refusal::Encrypted => 1,
            Refusal::TooLarge => 2,
            Refusal::Damaged => 3,
            Refusal::CannotOpen => 4,
            Refusal::LibraryMissing => 5,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Refusal::Encrypted,
            2 => Refusal::TooLarge,
            3 => Refusal::Damaged,
            4 => Refusal::CannotOpen,
            5 => Refusal::LibraryMissing,
            _ => return None,
        })
    }
}

/// A message that breaks the protocol.
#[derive(Debug)]
pub enum ProtocolError {
    /// The frame says it is longer than the reader allows.
    TooLong {
        length: u32,
        max: u32,
    },
    /// The message ended early.
    Truncated,
    /// Bytes followed the message.
    TrailingData,
    WrongVersion(u16),
    UnknownKind(u8),
    UnknownRefusal(u8),
    /// More pages than the reader allows.
    TooManyPages(u32),
    /// Page text is not valid UTF-8.
    InvalidText,
    /// The path in a request is empty or badly encoded.
    InvalidPath,
    Io(io::Error),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::TooLong { length, max } => {
                write!(f, "message of {length} bytes is over the {max}-byte limit")
            }
            ProtocolError::Truncated => write!(f, "message ended early"),
            ProtocolError::TrailingData => write!(f, "unexpected bytes after the message"),
            ProtocolError::WrongVersion(v) => write!(f, "protocol version {v}, expected {VERSION}"),
            ProtocolError::UnknownKind(k) => write!(f, "unknown message kind {k}"),
            ProtocolError::UnknownRefusal(c) => write!(f, "unknown refusal code {c}"),
            ProtocolError::TooManyPages(n) => write!(f, "{n} pages is over the limit"),
            ProtocolError::InvalidText => write!(f, "page text is not valid UTF-8"),
            ProtocolError::InvalidPath => write!(f, "the path is empty or badly encoded"),
            ProtocolError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ProtocolError {}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ProtocolError::Truncated
        } else {
            ProtocolError::Io(error)
        }
    }
}

pub fn write_request(out: &mut impl Write, request: &Request) -> io::Result<()> {
    let mut payload = header(KIND_EXTRACT);
    payload.extend(request.max_pages.to_le_bytes());
    payload.extend(request.max_text_bytes.to_le_bytes());
    payload.extend(path_to_bytes(&request.path));
    write_frame(out, &payload)
}

pub fn read_request(input: &mut impl Read) -> Result<Request, ProtocolError> {
    let payload = read_frame(input, MAX_REQUEST_BYTES)?;
    let mut fields = Fields::new(&payload);
    fields.expect_kind(KIND_EXTRACT)?;
    let max_pages = fields.u32()?;
    let max_text_bytes = fields.u32()?;
    let path = path_from_bytes(fields.rest())?;
    Ok(Request {
        path,
        max_pages,
        max_text_bytes,
    })
}

pub fn write_response(out: &mut impl Write, response: &Response) -> io::Result<()> {
    let payload = match response {
        Response::Pages(pages) => {
            let mut payload = header(KIND_PAGES);
            payload.extend(length_of(pages.len())?.to_le_bytes());
            for page in pages {
                payload.extend(length_of(page.len())?.to_le_bytes());
                payload.extend(page.as_bytes());
            }
            payload
        }
        Response::Refused(refusal) => {
            let mut payload = header(KIND_REFUSED);
            payload.push(refusal.code());
            payload
        }
    };
    write_frame(out, &payload)
}

/// Read and check the worker's answer. Nothing larger than `max_bytes` or with
/// more than `max_pages` pages is accepted.
pub fn read_response(
    input: &mut impl Read,
    max_bytes: u32,
    max_pages: u32,
) -> Result<Response, ProtocolError> {
    let payload = read_frame(input, max_bytes)?;
    let mut fields = Fields::new(&payload);
    let kind = fields.version_and_kind()?;
    let response = match kind {
        KIND_PAGES => {
            let count = fields.u32()?;
            if count > max_pages {
                return Err(ProtocolError::TooManyPages(count));
            }
            // Every page takes at least 4 bytes, so a count the payload cannot
            // hold is rejected before any memory is reserved for it.
            if count as usize > fields.remaining() / 4 {
                return Err(ProtocolError::Truncated);
            }
            let mut pages = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let length = fields.u32()?;
                let bytes = fields.take(length as usize)?;
                let text = std::str::from_utf8(bytes).map_err(|_| ProtocolError::InvalidText)?;
                pages.push(text.to_string());
            }
            Response::Pages(pages)
        }
        KIND_REFUSED => {
            let code = fields.u8()?;
            Response::Refused(Refusal::from_code(code).ok_or(ProtocolError::UnknownRefusal(code))?)
        }
        other => return Err(ProtocolError::UnknownKind(other)),
    };
    fields.finish()?;
    Ok(response)
}

fn header(kind: u8) -> Vec<u8> {
    let mut payload = VERSION.to_le_bytes().to_vec();
    payload.push(kind);
    payload
}

fn length_of(length: usize) -> io::Result<u32> {
    u32::try_from(length).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "too long"))
}

fn write_frame(out: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    out.write_all(&length_of(payload.len())?.to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

/// Read one frame, then require the end of the stream.
fn read_frame(input: &mut impl Read, max_bytes: u32) -> Result<Vec<u8>, ProtocolError> {
    let mut length = [0u8; 4];
    input.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length);
    if length > max_bytes {
        return Err(ProtocolError::TooLong {
            length,
            max: max_bytes,
        });
    }
    // The buffer grows as bytes arrive, so a false length reserves nothing.
    let mut payload = Vec::new();
    input
        .by_ref()
        .take(u64::from(length))
        .read_to_end(&mut payload)?;
    if payload.len() != length as usize {
        return Err(ProtocolError::Truncated);
    }
    if !at_end(input)? {
        return Err(ProtocolError::TrailingData);
    }
    Ok(payload)
}

fn at_end(input: &mut impl Read) -> io::Result<bool> {
    let mut byte = [0u8; 1];
    loop {
        match input.read(&mut byte) {
            Ok(read) => return Ok(read == 0),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

/// Reads fields from a payload, failing instead of reading past its end.
struct Fields<'a> {
    bytes: &'a [u8],
}

impl<'a> Fields<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn remaining(&self) -> usize {
        self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        if count > self.bytes.len() {
            return Err(ProtocolError::Truncated);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn version_and_kind(&mut self) -> Result<u8, ProtocolError> {
        let version = self.u16()?;
        if version != VERSION {
            return Err(ProtocolError::WrongVersion(version));
        }
        self.u8()
    }

    fn expect_kind(&mut self, kind: u8) -> Result<(), ProtocolError> {
        match self.version_and_kind()? {
            found if found == kind => Ok(()),
            other => Err(ProtocolError::UnknownKind(other)),
        }
    }

    fn rest(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.bytes)
    }

    fn finish(&self) -> Result<(), ProtocolError> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::TrailingData)
        }
    }
}

// Paths travel in the operating system's own form, so no file name is ever
// altered on the way, even one that is not valid Unicode.

#[cfg(windows)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(windows)]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, ProtocolError> {
    use std::os::windows::ffi::OsStringExt;
    let (pairs, odd_byte) = bytes.as_chunks::<2>();
    if pairs.is_empty() || !odd_byte.is_empty() {
        return Err(ProtocolError::InvalidPath);
    }
    let wide: Vec<u16> = pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    Ok(std::ffi::OsString::from_wide(&wide).into())
}

#[cfg(unix)]
fn path_to_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> Result<PathBuf, ProtocolError> {
    use std::os::unix::ffi::OsStringExt;
    if bytes.is_empty() {
        return Err(ProtocolError::InvalidPath);
    }
    Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: u32 = 1024;

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
        bytes.extend(payload);
        bytes
    }

    fn response_bytes(response: &Response) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_response(&mut bytes, response).unwrap();
        bytes
    }

    fn read(bytes: &[u8]) -> Result<Response, ProtocolError> {
        read_response(&mut &bytes[..], MAX, 10)
    }

    #[test]
    fn request_round_trips() {
        let request = Request {
            path: PathBuf::from("folder with spaces/résumé 2026.pdf"),
            max_pages: 5000,
            max_text_bytes: 64 << 20,
        };
        let mut bytes = Vec::new();
        write_request(&mut bytes, &request).unwrap();
        assert_eq!(read_request(&mut &bytes[..]).unwrap(), request);
    }

    #[cfg(windows)]
    #[test]
    fn a_path_that_is_not_valid_unicode_arrives_unchanged() {
        use std::os::windows::ffi::OsStringExt;
        // A lone surrogate: allowed in Windows file names, invalid in UTF-8.
        let name = std::ffi::OsString::from_wide(&[0xD800, 'a' as u16]);
        let request = Request {
            path: PathBuf::from(name),
            max_pages: 1,
            max_text_bytes: 1,
        };
        let mut bytes = Vec::new();
        write_request(&mut bytes, &request).unwrap();
        assert_eq!(read_request(&mut &bytes[..]).unwrap(), request);
    }

    #[test]
    fn responses_round_trip() {
        let pages = Response::Pages(vec![
            "Page one".to_string(),
            String::new(),
            "Seite drei: Größe ✓".to_string(),
        ]);
        assert_eq!(read(&response_bytes(&pages)).unwrap(), pages);
        for refusal in [
            Refusal::Encrypted,
            Refusal::TooLarge,
            Refusal::Damaged,
            Refusal::CannotOpen,
            Refusal::LibraryMissing,
        ] {
            let response = Response::Refused(refusal);
            assert_eq!(read(&response_bytes(&response)).unwrap(), response);
        }
    }

    #[test]
    fn an_oversized_length_is_rejected_before_the_payload_is_read() {
        /// Four length bytes, then a reader that fails the test if touched.
        struct Untouchable(Vec<u8>);
        impl Read for Untouchable {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                assert!(!self.0.is_empty(), "read past the length prefix");
                let count = buf.len().min(self.0.len());
                buf[..count].copy_from_slice(&self.0[..count]);
                self.0.drain(..count);
                Ok(count)
            }
        }
        let mut input = Untouchable(u32::MAX.to_le_bytes().to_vec());
        let result = read_response(&mut input, MAX, 10);
        assert!(matches!(
            result,
            Err(ProtocolError::TooLong {
                length: u32::MAX,
                ..
            })
        ));
    }

    #[test]
    fn a_short_message_is_truncated() {
        let mut bytes = response_bytes(&Response::Pages(vec!["some text".to_string()]));
        bytes.truncate(bytes.len() - 3);
        assert!(matches!(read(&bytes), Err(ProtocolError::Truncated)));
        assert!(matches!(read(&[1, 0]), Err(ProtocolError::Truncated)));
    }

    #[test]
    fn bytes_after_the_message_are_rejected() {
        let mut bytes = response_bytes(&Response::Refused(Refusal::Damaged));
        bytes.push(0);
        assert!(matches!(read(&bytes), Err(ProtocolError::TrailingData)));

        // Extra bytes inside the frame are rejected too.
        let mut payload = header(KIND_REFUSED);
        payload.extend([3, 0]);
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::TrailingData)
        ));
    }

    #[test]
    fn another_version_is_rejected() {
        let mut payload = 2u16.to_le_bytes().to_vec();
        payload.extend([KIND_REFUSED, 3]);
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::WrongVersion(2))
        ));
    }

    #[test]
    fn unknown_kinds_and_codes_are_rejected() {
        let mut payload = header(9);
        payload.push(0);
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::UnknownKind(9))
        ));
        let mut payload = header(KIND_REFUSED);
        payload.push(77);
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::UnknownRefusal(77))
        ));
    }

    #[test]
    fn page_counts_are_checked_before_memory_is_reserved() {
        let mut payload = header(KIND_PAGES);
        payload.extend(11u32.to_le_bytes());
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::TooManyPages(11))
        ));
        // Ten pages claimed, but there are no bytes for them.
        let mut payload = header(KIND_PAGES);
        payload.extend(10u32.to_le_bytes());
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::Truncated)
        ));
    }

    #[test]
    fn page_text_must_be_valid_utf8() {
        let mut payload = header(KIND_PAGES);
        payload.extend(1u32.to_le_bytes());
        payload.extend(2u32.to_le_bytes());
        payload.extend([0xC3, 0x28]);
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::InvalidText)
        ));
    }

    #[test]
    fn a_page_longer_than_the_message_is_truncated() {
        let mut payload = header(KIND_PAGES);
        payload.extend(1u32.to_le_bytes());
        payload.extend(500u32.to_le_bytes());
        payload.extend(b"short");
        assert!(matches!(
            read(&frame(&payload)),
            Err(ProtocolError::Truncated)
        ));
    }

    #[test]
    fn requests_need_a_path() {
        let mut payload = header(KIND_EXTRACT);
        payload.extend(1u32.to_le_bytes());
        payload.extend(1u32.to_le_bytes());
        assert!(matches!(
            read_request(&mut &frame(&payload)[..]),
            Err(ProtocolError::InvalidPath)
        ));
    }
}
