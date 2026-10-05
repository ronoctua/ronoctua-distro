//! `multipart/form-data` with ONE file part, written as it streams.
//!
//! # Why it is here
//!
//! Because a file sent to a host is an HTTP body, and the framer is this
//! crate (`one-http-writer`). The first caller is the security centre's
//! VirusTotal upload: a file of up to 650 MB that must never sit in memory,
//! so the body is three pieces — a preamble with the part's headers, the
//! file's bytes as they are read, and the closing boundary — and the
//! `Content-Length` is their sum, known before the first byte goes.
//!
//! # The boundary is checked, not hoped for
//!
//! A boundary is a string the file must not contain. A random one almost
//! never appears in a file; "almost" is not a guarantee, and a file that did
//! contain it would reach the host cut in two with nothing said. So the bytes
//! are watched as they stream ([`Watch`]), across the edges of the chunks,
//! and a file that carries the boundary is refused before its end is sent —
//! the host then sees a body shorter than announced and discards it.
//!
//! Pure: the pieces are bytes, and the writer is anything that writes.

use std::io::{Read, Write};

/// The one file part of a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part<'a> {
    /// The form field's name (`file`).
    pub field: &'a str,
    /// The file name the host is told. Quotes and line breaks are replaced,
    /// so a name cannot close the header it is written in ([`preamble`]).
    pub filename: &'a str,
    /// The part's `Content-Type`.
    pub content_type: &'a str,
    /// How many bytes the file is.
    pub length: u64,
}

/// A boundary for one body: `ronoctua-` and 32 hex digits from the clock,
/// the process and a counter — different for every body this process frames.
#[must_use]
pub fn boundary() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mixed = nanos
        ^ (u128::from(std::process::id()) << 64)
        ^ (u128::from(NEXT.fetch_add(1, Ordering::Relaxed)) << 96);
    format!("ronoctua-{mixed:032x}")
}

/// The body's `Content-Type`, naming the boundary.
#[must_use]
pub fn content_type(boundary: &str) -> String {
    format!("multipart/form-data; boundary={boundary}")
}

/// A file name as it may stand inside the part's quoted header: every `"`,
/// `\`, control character and non-ASCII character becomes `_`. The host is
/// told a name, not the name: what it matters for is the report's label.
#[must_use]
pub fn plain_name(name: &str) -> String {
    let plain: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if plain.trim().is_empty() {
        "file".to_owned()
    } else {
        plain
    }
}

/// What comes before the file's bytes: the opening boundary and the part's
/// two headers.
#[must_use]
pub fn preamble(boundary: &str, part: &Part<'_>) -> Vec<u8> {
    format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
        plain_name(part.field),
        plain_name(part.filename),
        part.content_type
    )
    .into_bytes()
}

/// What comes after the file's bytes: the closing boundary.
#[must_use]
pub fn epilogue(boundary: &str) -> Vec<u8> {
    format!("\r\n--{boundary}--\r\n").into_bytes()
}

/// The whole body's length: the preamble, the file and the epilogue.
#[must_use]
pub fn length(boundary: &str, part: &Part<'_>) -> u64 {
    preamble(boundary, part).len() as u64 + part.length + epilogue(boundary).len() as u64
}

/// Why a body was not written whole.
#[derive(Debug)]
pub enum Unsent {
    /// The file carries the boundary.
    Boundary,
    /// The file ended before the length it was said to have, or went on past
    /// it.
    Length {
        /// What it was said to be.
        said: u64,
        /// What was read.
        read: u64,
    },
    /// The caller said stop (`sent` answered `false`).
    Stopped,
    /// Reading the file or writing the socket failed.
    Io(std::io::Error),
}

impl std::fmt::Display for Unsent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boundary => f.write_str("the file carries the form's boundary"),
            Self::Length { said, read } => {
                write!(
                    f,
                    "the file was said to be {said} bytes and {read} were read"
                )
            }
            Self::Stopped => f.write_str("stopped by its caller"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Unsent {}

/// Watches a stream of chunks for one needle, across the chunks' edges.
#[derive(Debug)]
pub struct Watch {
    needle: Vec<u8>,
    tail: Vec<u8>,
}

impl Watch {
    /// A watch for `needle`.
    #[must_use]
    pub fn new(needle: &[u8]) -> Self {
        Self {
            needle: needle.to_vec(),
            tail: Vec::new(),
        }
    }

    /// Whether the needle is in what came so far, with this chunk.
    pub fn seen(&mut self, chunk: &[u8]) -> bool {
        if self.needle.is_empty() {
            return false;
        }
        let mut joined = std::mem::take(&mut self.tail);
        joined.extend_from_slice(chunk);
        let found = joined
            .windows(self.needle.len())
            .any(|w| w == self.needle.as_slice());
        let keep = self.needle.len() - 1;
        let from = joined.len().saturating_sub(keep);
        self.tail = joined[from..].to_vec();
        found
    }
}

/// A file on its way: the part it is, where its bytes are read from, and who
/// hears how far it got.
pub struct Sending<'a> {
    /// The part.
    pub part: Part<'a>,
    /// Its bytes.
    pub file: &'a mut dyn Read,
    /// Told the bytes of the FILE written so far and its length after every
    /// chunk; answering `false` stops.
    pub sent: &'a mut dyn FnMut(u64, u64) -> bool,
}

impl std::fmt::Debug for Sending<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sending")
            .field("part", &self.part)
            .finish_non_exhaustive()
    }
}

/// Writes the body to `to`: the preamble, `part.length` bytes of the file as
/// they are read (in 64 KiB chunks), then the epilogue.
///
/// # Errors
///
/// [`Unsent`]: the boundary is in the file, the file's length is not the one
/// said, the caller stopped, or reading or writing failed.
pub fn write(to: &mut dyn Write, boundary: &str, sending: Sending<'_>) -> Result<(), Unsent> {
    let Sending { part, file, sent } = sending;
    let part = &part;
    to.write_all(&preamble(boundary, part))
        .map_err(Unsent::Io)?;
    // THE BOUNDARY AS IT WOULD STAND IN THE BODY: a line break and two
    // dashes before it. Only that sequence ends the part early; the bare
    // string inside a line does not.
    let mut watch = Watch::new(format!("\r\n--{boundary}").as_bytes());
    // THE PREAMBLE ENDS IN A LINE BREAK, so a file that BEGINS with the
    // boundary makes the delimiter with it.
    watch.seen(b"\r\n");
    let mut buffer = vec![0u8; 64 << 10];
    let mut read = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(Unsent::Io)?;
        if n == 0 {
            break;
        }
        read += n as u64;
        if read > part.length {
            return Err(Unsent::Length {
                said: part.length,
                read,
            });
        }
        if watch.seen(&buffer[..n]) {
            return Err(Unsent::Boundary);
        }
        to.write_all(&buffer[..n]).map_err(Unsent::Io)?;
        if !sent(read, part.length) {
            return Err(Unsent::Stopped);
        }
    }
    if read != part.length {
        return Err(Unsent::Length {
            said: part.length,
            read,
        });
    }
    to.write_all(&epilogue(boundary)).map_err(Unsent::Io)?;
    to.flush().map_err(Unsent::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sending<'a>(
        part: Part<'a>,
        file: &'a mut dyn Read,
        sent: &'a mut dyn FnMut(u64, u64) -> bool,
    ) -> Sending<'a> {
        Sending { part, file, sent }
    }

    fn part(length: u64) -> Part<'static> {
        Part {
            field: "file",
            filename: "report.pdf",
            content_type: "application/octet-stream",
            length,
        }
    }

    /// FAILS IF: the body stops being the three pieces in order, its length
    /// stops being their sum, or the progress stops counting the file.
    #[test]
    fn a_body_is_the_preamble_the_file_and_the_epilogue() {
        let file = b"%PDF-1.7 hello".to_vec();
        let p = part(file.len() as u64);
        let mut out = Vec::new();
        let mut heard = Vec::new();
        write(
            &mut out,
            "B0",
            Sending {
                part: p,
                file: &mut file.as_slice(),
                sent: &mut |n, of| {
                    heard.push((n, of));
                    true
                },
            },
        )
        .expect("written");
        let expected = "--B0\r\nContent-Disposition: form-data; name=\"file\"; filename=\"report.pdf\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n%PDF-1.7 hello\r\n--B0--\r\n";
        assert_eq!(String::from_utf8(out.clone()).unwrap(), expected);
        assert_eq!(length("B0", &p), out.len() as u64);
        assert_eq!(heard, [(14, 14)]);
    }

    /// FAILS IF: a file that carries the boundary is sent anyway — even when
    /// the boundary is split between two chunks of the read.
    #[test]
    fn a_file_carrying_the_boundary_is_refused() {
        let boundary = "ronoctua-abc";
        let mut file = vec![b'x'; (64 << 10) - 3];
        file.extend_from_slice(b"\r\n--ronoctua-abc tail");
        let p = part(file.len() as u64);
        let mut out = Vec::new();
        let said = write(
            &mut out,
            boundary,
            sending(p, &mut file.as_slice(), &mut |_, _| true),
        );
        assert!(matches!(said, Err(Unsent::Boundary)), "{said:?}");
        // AND A FILE THAT BEGINS WITH IT, against the preamble's last line
        // break.
        let first = b"--ronoctua-abc".to_vec();
        let p = part(first.len() as u64);
        let mut out = Vec::new();
        let said = write(
            &mut out,
            boundary,
            sending(p, &mut first.as_slice(), &mut |_, _| true),
        );
        assert!(matches!(said, Err(Unsent::Boundary)), "{said:?}");
        // AND THE BARE WORD INSIDE A LINE is no boundary.
        let fine = b"text --ronoctua-abc is only text here".to_vec();
        let p = part(fine.len() as u64);
        let mut out = Vec::new();
        write(
            &mut out,
            boundary,
            sending(p, &mut fine.as_slice(), &mut |_, _| true),
        )
        .expect("a bare word is not a boundary line");
    }

    /// FAILS IF: a file shorter or longer than said is sent as if it were
    /// whole, or a stop is not honoured.
    #[test]
    fn a_wrong_length_or_a_stop_is_refused() {
        let file = b"12345".to_vec();
        let mut out = Vec::new();
        let short = write(
            &mut out,
            "b",
            sending(part(9), &mut file.as_slice(), &mut |_, _| true),
        );
        assert!(matches!(short, Err(Unsent::Length { said: 9, read: 5 })));
        let mut out = Vec::new();
        let long = write(
            &mut out,
            "b",
            sending(part(2), &mut file.as_slice(), &mut |_, _| true),
        );
        assert!(matches!(long, Err(Unsent::Length { said: 2, .. })));
        let mut out = Vec::new();
        let stopped = write(
            &mut out,
            "b",
            sending(part(5), &mut file.as_slice(), &mut |_, _| false),
        );
        assert!(matches!(stopped, Err(Unsent::Stopped)));
    }

    /// FAILS IF: a file name can close its quoted header, or start a new
    /// header line.
    #[test]
    fn a_file_name_cannot_close_its_header() {
        assert_eq!(plain_name("a\"b\r\nX: y.pdf"), "a_b__X: y.pdf");
        assert_eq!(plain_name("naïve.txt"), "na_ve.txt");
        assert_eq!(plain_name("  "), "file");
        let head = String::from_utf8(preamble(
            "b",
            &Part {
                filename: "evil\"\r\nInjected: 1",
                ..part(0)
            },
        ))
        .unwrap();
        assert_eq!(head.matches("\r\n").count(), 4, "{head:?}");
    }

    /// FAILS IF: two bodies framed in a row share a boundary.
    #[test]
    fn every_body_has_its_own_boundary() {
        let a = boundary();
        let b = boundary();
        assert_ne!(a, b);
        assert!(a.starts_with("ronoctua-") && a.len() == 41, "{a}");
        assert_eq!(
            content_type(&a),
            format!("multipart/form-data; boundary={a}")
        );
    }

    #[test]
    fn a_watch_sees_across_the_edges() {
        let mut w = Watch::new(b"abcd");
        assert!(!w.seen(b"xxab"));
        assert!(w.seen(b"cdyy"));
        let mut w = Watch::new(b"abcd");
        assert!(!w.seen(b"a"));
        assert!(!w.seen(b"b"));
        assert!(!w.seen(b"c"));
        assert!(w.seen(b"d"));
    }
}
