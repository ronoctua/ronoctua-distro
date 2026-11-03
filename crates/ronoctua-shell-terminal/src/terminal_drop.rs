//! FILES DROPPED ON A TERMINAL become their paths, typed: shell-quoted, one
//! after the other, through the same paste a `C-S-v` makes — the program
//! receives what the person would have typed, and a shell runs nothing until
//! they press `RET` (Ghostty's way, and every terminal's since xterm).
//!
//! The drag itself is the door's (`crate::carry`, the one file that speaks
//! `wl_data_device`, invariant 3); the terminal's window answers it
//! (`Carries for TerminalWindow`), reads the list OFF the loop — the other
//! end of the pipe is somebody else's program — and hands the paths here.
//!
//! # How a path is quoted
//!
//! Left as it is when every byte is one a POSIX shell takes literally
//! (`[A-Za-z0-9_@%+=:,./-]`), the path a person reads; in single quotes
//! otherwise, a quote inside closed, escaped and opened again (`'\''`). A
//! name with a control character or with bytes that are no text cannot be
//! typed as itself — a line break pasted would be Enter to a shell — so it
//! is written in `$'…'`, every such byte as `\xNN`: bash, zsh, ksh and
//! POSIX.1-2024's `sh` read that back to the same bytes.

use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

/// What a drop of `paths` types: each quoted, a space between them.
#[must_use]
pub fn typed(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| quoted(p))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One path, quoted for a shell — see the head of the module.
#[must_use]
pub fn quoted(path: &Path) -> String {
    let bytes = path.as_os_str().as_bytes();
    if !bytes.is_empty() && bytes.iter().all(|b| literal(*b)) {
        // Every byte is ASCII here, so the text is the path.
        return String::from_utf8_lossy(bytes).into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) if !text.chars().any(char::is_control) => {
            format!("'{}'", text.replace('\'', r"'\''"))
        }
        _ => escaped(bytes),
    }
}

/// A byte a shell takes as itself, unquoted.
const fn literal(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'@' | b'%' | b'+' | b'=' | b':' | b',' | b'.' | b'/' | b'-')
}

/// `$'…'`: printable text as it is (a quote and a backslash escaped), and
/// every control byte or byte that is no text as `\xNN`.
fn escaped(bytes: &[u8]) -> String {
    let mut out = String::from("$'");
    let mut rest = bytes;
    while !rest.is_empty() {
        let (good, bad) = match std::str::from_utf8(rest) {
            Ok(text) => (text, &[][..]),
            Err(e) => {
                let (good, bad) = rest.split_at(e.valid_up_to());
                // `valid_up_to` is where the text ends: what comes before is
                // text by the decoder's own word.
                (std::str::from_utf8(good).unwrap_or_default(), bad)
            }
        };
        for c in good.chars() {
            match c {
                '\'' => out.push_str(r"\'"),
                '\\' => out.push_str(r"\\"),
                c if c.is_control() => {
                    let mut buf = [0u8; 4];
                    for b in c.encode_utf8(&mut buf).bytes() {
                        out.push_str(&format!("\\x{b:02x}"));
                    }
                }
                c => out.push(c),
            }
        }
        // ONE BYTE that is no text, and the rest read again from after it.
        match bad.split_first() {
            Some((b, after)) => {
                out.push_str(&format!("\\x{b:02x}"));
                rest = after;
            }
            None => rest = &[],
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    /// What `sh` reads back from `typed` — the only judge of a quoting that
    /// matters, run as a child: `printf '%s\0'` of the words it parses.
    fn read_back(typed: &str) -> Vec<Vec<u8>> {
        let out = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("printf '%s\\0' {typed}")])
            .output()
            .expect("a /bin/sh to read the words back");
        assert!(out.status.success(), "sh refused {typed:?}");
        out.stdout
            .split(|b| *b == 0)
            .filter(|w| !w.is_empty())
            .map(<[u8]>::to_vec)
            .collect()
    }

    #[test]
    fn a_plain_path_is_typed_as_itself() {
        // FAILS IF a path every shell reads as it is gets quotes around it —
        // the line the person reads is the path they dropped.
        assert_eq!(quoted(Path::new("/home/a/notes.md")), "/home/a/notes.md");
        assert_eq!(
            quoted(Path::new("/tmp/x-1_2,3@4+5=6:7")),
            "/tmp/x-1_2,3@4+5=6:7"
        );
    }

    #[test]
    fn a_dropped_path_comes_back_from_the_shell_byte_for_byte() {
        // FAILS IF a name with a space, a quote, a glob, a `$`, a backtick,
        // a line break or bytes that are no text reaches the shell as
        // anything but itself: two words, an expansion, a command run, a
        // line that ends early. The ones `/bin/sh` here can read back are
        // read back by it; `$'…'`'s own spelling is held below.
        let paths: Vec<PathBuf> = [
            "/home/a/my notes.md",
            "/home/a/it's.txt",
            "/home/a/*.rs",
            "/home/a/$HOME `id` ; rm -rf ~",
            "/home/a/ação 日本.txt",
            "/home/a/back\\slash",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let line = typed(&paths);
        let back = read_back(&line);
        let wanted: Vec<Vec<u8>> = paths
            .iter()
            .map(|p| p.as_os_str().as_bytes().to_vec())
            .collect();
        assert_eq!(back, wanted, "{line}");
    }

    #[test]
    fn a_name_that_is_no_text_is_written_by_its_bytes() {
        // FAILS IF a line break or a byte that is no UTF-8 is pasted raw: a
        // line break is Enter to a shell (the command runs half typed), and
        // a byte that is no text cannot be a `String` at all.
        let newline = quoted(Path::new("/tmp/a\nb"));
        assert_eq!(newline, "$'/tmp/a\\x0ab'");
        assert!(!newline.contains('\n'));
        let raw = quoted(Path::new(OsStr::from_bytes(b"/tmp/\xff'x")));
        assert_eq!(raw, "$'/tmp/\\xff\\'x'");
        assert!(crate::terminal::paste_is_safe(
            &typed(&[PathBuf::from("/tmp/a\nb")]),
            false
        ));
    }
}
