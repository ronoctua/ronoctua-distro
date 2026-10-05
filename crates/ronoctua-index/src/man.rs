//! The manual pages (feature `man`, which `write` brings: a door reads a
//! page's text with [`text_of`], and only the unit makes documents of them).
//!
//! # Where
//!
//! The `share/man` of the system (`/run/current-system/sw`) and of the
//! person's profiles (`/etc/profiles/per-user/<user>`, `~/.nix-profile`, and
//! `$XDG_STATE_HOME/nix/profile` where the new `nix profile` keeps it) —
//! what `man` itself would search on this machine. Only `man<section>/`
//! folders directly under each: a translated tree (`de/man1`) is another
//! language's page of the same name.
//!
//! # What a page becomes
//!
//! `man:<name>.<section>`, named by its file; the NAME line (`ls \- list
//! directory contents`, or mdoc's `.Nm`/`.Nd`) gives the other names it
//! answers to and the one-line description a row shows; the page with its
//! roff taken off is the body. A page that is only `.so man1/other.1` is the
//! other page's text under its own name.
//!
//! # When
//!
//! A relist is whole (every manual page deleted, every one added): a few
//! thousand pages, and it only happens when the folders resolve somewhere
//! new — a system switch or a profile change moves the store paths, and the
//! resolved folders are the signature kept in the commit's state.

use std::io::Read as _;
use std::path::{Path, PathBuf};

#[cfg(feature = "write")]
use crate::Kind;
#[cfg(feature = "write")]
use crate::write::Document;

/// The most bytes of a page read (before and after `gzip`): a manual is tens
/// of kilobytes; `bash(1)` is the largest common one at a third of a
/// megabyte.
const PAGE_BYTES: u64 = 4 * 1024 * 1024;

/// Where the manual folders of this machine may be, for `user` with `home`
/// and the state root `state` (`$XDG_STATE_HOME`). A relist reads the ones
/// that exist at that moment: a profile gains its `share/man` with its first
/// program that has one.
#[must_use]
pub fn candidates(user: Option<&str>, home: Option<&Path>, state: Option<&Path>) -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from("/run/current-system/sw/share/man")];
    if let Some(user) = user {
        out.push(
            Path::new("/etc/profiles/per-user")
                .join(user)
                .join("share/man"),
        );
    }
    if let Some(home) = home {
        out.push(home.join(".nix-profile/share/man"));
    }
    if let Some(state) = state {
        out.push(state.join("nix/profile/share/man"));
    }
    out
}

/// Where the folders resolve, as one string: the relist's signature.
#[must_use]
pub fn signature(folders: &[PathBuf]) -> String {
    folders
        .iter()
        .map(|f| {
            std::fs::canonicalize(f)
                .unwrap_or_else(|_| f.clone())
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every page under `folders`, the first folder winning a name given twice.
#[must_use]
pub fn pages(folders: &[PathBuf]) -> Vec<(String, String, PathBuf)> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in folders {
        let Ok(sections) = std::fs::read_dir(root) else {
            continue;
        };
        let mut sections: Vec<PathBuf> = sections
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("man") && n.len() > 3)
            })
            .collect();
        sections.sort();
        for section in sections {
            let Ok(files) = std::fs::read_dir(&section) else {
                continue;
            };
            let mut files: Vec<PathBuf> = files.filter_map(Result::ok).map(|e| e.path()).collect();
            files.sort();
            for file in files {
                let Some((name, sec)) = file.file_name().and_then(|n| n.to_str()).and_then(name_of)
                else {
                    continue;
                };
                if seen.insert((name.clone(), sec.clone())) {
                    out.push((name, sec, file));
                }
            }
        }
    }
    out
}

/// A page's name and section from its file name (`ls.1.gz`, `printf.3p`).
#[must_use]
pub fn name_of(file: &str) -> Option<(String, String)> {
    let bare = file.strip_suffix(".gz").unwrap_or(file);
    let (name, section) = bare.rsplit_once('.')?;
    let starts_with_digit = section
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == 'n');
    (!name.is_empty() && starts_with_digit).then(|| (name.to_owned(), section.to_owned()))
}

/// A page's roff, gunzipped when it is gzip, at most [`PAGE_BYTES`].
fn raw(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    if path.extension().is_some_and(|e| e == "gz") {
        flate2::read::GzDecoder::new(file.take(PAGE_BYTES))
            .take(PAGE_BYTES)
            .read_to_end(&mut bytes)
            .ok()?;
    } else {
        file.take(PAGE_BYTES).read_to_end(&mut bytes).ok()?;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The page at `path`'s roff, `.so` followed once (relative to the manual
/// root above its section folder).
fn roff_of(path: &Path) -> Option<String> {
    let roff = raw(path)?;
    let Some(target) = roff
        .trim_start()
        .strip_prefix(".so ")
        .map(|t| t.lines().next().unwrap_or("").trim().to_owned())
    else {
        return Some(roff);
    };
    let root = path.parent()?.parent()?;
    let mut other = root.join(&target);
    if !other.exists() {
        other = root.join(format!("{target}.gz"));
    }
    raw(&other)
}

/// The page `name` of `section` under `folders` — the first folder that has
/// it — as plain text, its roff taken off: what fast-access's detail shows of
/// a manual page. `None` when no folder has it.
#[must_use]
pub fn text_of(folders: &[PathBuf], name: &str, section: &str) -> Option<String> {
    let (_, _, path) = pages(folders)
        .into_iter()
        .find(|(n, s, _)| n == name && s == section)?;
    roff_of(&path).map(|roff| strip(&roff))
}

/// The page at `path` as a document, `.so` followed once (relative to the
/// manual root above its section folder).
#[cfg(feature = "write")]
#[must_use]
pub fn document(name: &str, section: &str, path: &Path, body_bytes: usize) -> Option<Document> {
    let roff = roff_of(path)?;
    let page = strip(&roff);
    let (names, description) = name_line(&roff, &page);
    let mut body = page;
    if body.len() > body_bytes {
        let mut at = body_bytes;
        while at > 0 && !body.is_char_boundary(at) {
            at -= 1;
        }
        body.truncate(at);
    }
    Some(Document {
        id: format!("man:{name}.{section}"),
        kind: Kind::Man,
        name: name.to_owned(),
        also: names.into_iter().filter(|n| n != name).collect(),
        path_terms: Vec::new(),
        body: Some(body),
        note: description,
        mime: None,
        mtime_s: 0,
        size_bytes: 0,
    })
}

/// The names a page answers to and its one-line description: the text of
/// its NAME section (`ls, dir \- list directory contents`), or mdoc's
/// `.Nm`/`.Nd`.
#[cfg(feature = "write")]
fn name_line(roff: &str, page: &str) -> (Vec<String>, Option<String>) {
    let mut nm = Vec::new();
    let mut nd = None;
    for line in roff.lines() {
        if let Some(rest) = line.strip_prefix(".Nm ") {
            let n = unescape(rest).trim().to_owned();
            if !n.is_empty() && !nm.contains(&n) && nd.is_none() {
                nm.push(n);
            }
        } else if let Some(rest) = line.strip_prefix(".Nd ") {
            nd = Some(unescape(rest).trim().to_owned());
            break;
        }
    }
    if nd.is_some() {
        return (nm, nd);
    }
    // The stripped page keeps headings as lines: find NAME, take what follows
    // up to the next heading.
    let mut lines = page
        .lines()
        .skip_while(|l| !l.trim().eq_ignore_ascii_case("name"));
    if lines.next().is_none() {
        return (Vec::new(), None);
    }
    let text: String = lines
        .take_while(|l| !l.trim().is_empty() && !is_heading(l))
        .collect::<Vec<_>>()
        .join(" ");
    let Some((names, description)) = text.split_once(" - ") else {
        return (Vec::new(), None);
    };
    let names = names
        .split(',')
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .collect();
    let description = description.trim();
    (
        names,
        (!description.is_empty()).then(|| description.to_owned()),
    )
}

/// Whether a stripped line was a section heading (all capitals).
#[cfg(feature = "write")]
fn is_heading(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && t.chars().any(char::is_alphabetic) && t.chars().all(|c| !c.is_lowercase())
}

/// The macros whose arguments are not the page's text.
const SILENT: [&str; 26] = [
    "TH", "Dd", "Dt", "Os", "de", "ds", "nr", "so", "ft", "ps", "ta", "ti", "in", "ll", "ne", "hy",
    "nh", "ad", "na", "if", "ie", "el", "tr", "cc", "rr", "am",
];

/// The page's text: comments and layout requests gone, the text of the
/// others kept, escapes read.
#[must_use]
pub fn strip(roff: &str) -> String {
    let mut out = String::new();
    let mut skipping_definition = false;
    for line in roff.lines() {
        if skipping_definition {
            if line.starts_with("..") {
                skipping_definition = false;
            }
            continue;
        }
        let trimmed = line.trim_end();
        if trimmed.starts_with(".\\\"")
            || trimmed.starts_with("'\\\"")
            || trimmed.starts_with("\\\"")
        {
            continue;
        }
        let text = if let Some(request) = trimmed
            .strip_prefix('.')
            .or_else(|| trimmed.strip_prefix('\''))
        {
            let request = request.trim_start();
            let (macro_name, rest) = request
                .split_once(char::is_whitespace)
                .unwrap_or((request, ""));
            if macro_name == "de" || macro_name == "am" {
                skipping_definition = true;
                continue;
            }
            if SILENT.contains(&macro_name) || rest.trim().is_empty() {
                continue;
            }
            arguments(rest)
        } else {
            trimmed.to_owned()
        };
        let text = unescape(&text);
        let text = text.trim();
        if !text.is_empty() {
            out.push_str(text);
            out.push('\n');
        }
    }
    out
}

/// A request's arguments as words: quotes off, `"a b"` kept together.
fn arguments(rest: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    for c in rest.chars() {
        if c == '"' {
            quoted = !quoted;
            continue;
        }
        if c.is_whitespace() && !quoted {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// A named special character (`\(em`, `\[bu]`) as the letter it draws, when
/// it is one a page writes words with.
fn special(name: &str) -> &'static str {
    match name {
        "em" => "\u{2014}",
        "en" | "hy" | "mi" => "-",
        "bu" => "\u{2022}",
        "co" => "\u{a9}",
        "rg" => "\u{ae}",
        "lq" | "rq" | "dq" => "\"",
        "oq" | "cq" | "aq" => "'",
        "ga" => "`",
        "ti" => "~",
        "ha" => "^",
        "rs" => "\\",
        "sl" => "/",
        "lB" => "[",
        "rB" => "]",
        "fo" => "<",
        "fc" => ">",
        _ => "",
    }
}

/// Reads roff's escapes: fonts, sizes and strings go, hyphens and named
/// characters stay as their letters.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(e) = chars.next() else {
            break;
        };
        match e {
            // A comment runs to the end of the line.
            '"' => break,
            '-' | '\\' | 'e' => out.push(if e == '-' { '-' } else { '\\' }),
            ' ' | '~' | '0' => out.push(' '),
            '&' | '|' | '^' | '%' | 'c' | ':' | ')' | ',' | '/' => {}
            '(' => {
                let name: String = chars.by_ref().take(2).collect();
                out.push_str(special(&name));
            }
            '[' => {
                let name: String = chars.by_ref().take_while(|&c| c != ']').collect();
                out.push_str(special(&name));
            }
            // Font, string, size, and the other one-argument escapes: the
            // argument goes with them.
            'f' | '*' | 'n' | 'F' | 'm' | 'M' | 'g' | 'k' | 'Y' | 'V' => match chars.next() {
                Some('(') => {
                    chars.next();
                    chars.next();
                }
                Some('[') => while chars.next().is_some_and(|c| c != ']') {},
                _ => {}
            },
            's' => {
                if matches!(chars.peek(), Some('+' | '-')) {
                    chars.next();
                }
                match chars.peek() {
                    Some('(') => {
                        chars.next();
                        chars.next();
                        chars.next();
                    }
                    Some('[') => while chars.next().is_some_and(|c| c != ']') {},
                    Some(c) if c.is_ascii_digit() => {
                        chars.next();
                    }
                    _ => {}
                }
            }
            // Escapes with a delimited argument (`\h'1m'`, `\w'…'`): gone.
            'h' | 'v' | 'w' | 'l' | 'L' | 'o' | 'b' | 'D' | 'x' | 'X' | 'N' | 'Z' | 'A' | 'B'
            | 'C' | 'R' | 'S' => {
                if let Some(delimiter) = chars.next() {
                    while chars.next().is_some_and(|c| c != delimiter) {}
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(all(test, feature = "write"))]
mod tests {
    use super::*;

    const LS: &str = r#".\" Generated by hand for a test
.TH LS "1" "2024" "coreutils" "User Commands"
.SH NAME
ls, dir \- list directory contents
.SH SYNOPSIS
.B ls
[\fI\,OPTION\/\fR]... [\fI\,FILE\/\fR]...
.SH DESCRIPTION
.\" a comment inside
List information about the FILEs (the current directory by default).
.TP
\fB\-a\fR, \fB\-\-all\fR
do not ignore entries starting with \(em a dot
"#;

    #[test]
    fn a_page_loses_its_roff_and_keeps_its_words() {
        let page = strip(LS);
        assert!(page.contains("List information about the FILEs"), "{page}");
        assert!(page.contains("-a, --all"), "{page}");
        assert!(page.contains("\u{2014} a dot"), "{page}");
        assert!(page.contains("[OPTION]... [FILE]..."), "{page}");
        assert!(!page.contains("coreutils"), "{page}");
        assert!(!page.contains("comment"), "{page}");
        assert!(!page.contains("\\f"), "{page}");
    }

    #[test]
    fn the_name_line_gives_the_names_and_the_description() {
        let (names, description) = name_line(LS, &strip(LS));
        assert_eq!(names, ["ls", "dir"]);
        assert_eq!(description.as_deref(), Some("list directory contents"));
        let mdoc = ".Dd June 1, 2024\n.Dt TMUX 1\n.Sh NAME\n.Nm tmux\n.Nd terminal multiplexer\n";
        let (names, description) = name_line(mdoc, &strip(mdoc));
        assert_eq!(names, ["tmux"]);
        assert_eq!(description.as_deref(), Some("terminal multiplexer"));
    }

    #[test]
    fn a_file_name_gives_the_name_and_the_section() {
        assert_eq!(name_of("ls.1.gz"), Some(("ls".into(), "1".into())));
        assert_eq!(name_of("printf.3p"), Some(("printf".into(), "3p".into())));
        assert_eq!(
            name_of("git-commit.1"),
            Some(("git-commit".into(), "1".into()))
        );
        assert_eq!(
            name_of("Tcl_Eval.n.gz"),
            Some(("Tcl_Eval".into(), "n".into()))
        );
        assert_eq!(name_of("README"), None);
        assert_eq!(name_of("index.db"), None);
    }

    #[test]
    fn pages_are_read_gzipped_and_a_redirect_is_followed() {
        use std::io::Write as _;
        let root = std::env::temp_dir().join(format!("ronoctua-man-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("man1")).unwrap();
        std::fs::create_dir_all(root.join("de/man1")).unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(LS.as_bytes()).unwrap();
        std::fs::write(root.join("man1/ls.1.gz"), gz.finish().unwrap()).unwrap();
        std::fs::write(root.join("man1/dir.1"), ".so man1/ls.1\n").unwrap();
        std::fs::write(root.join("de/man1/ls.1"), ".SH NAME\nls \\- Verzeichnis\n").unwrap();

        let found = pages(std::slice::from_ref(&root));
        let names: Vec<&str> = found.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["dir", "ls"],
            "a translated tree is not this language's page"
        );

        let (name, sec, path) = &found[1];
        let doc = document(name, sec, path, 1 << 20).unwrap();
        assert_eq!(doc.id, "man:ls.1");
        assert_eq!(doc.kind, Kind::Man);
        assert_eq!(doc.also, ["dir"]);
        assert_eq!(doc.note.as_deref(), Some("list directory contents"));
        assert!(doc.body.as_deref().unwrap().contains("List information"));

        let (name, sec, path) = &found[0];
        let redirected = document(name, sec, path, 1 << 20).unwrap();
        assert_eq!(redirected.id, "man:dir.1");
        assert!(
            redirected
                .body
                .as_deref()
                .unwrap()
                .contains("List information")
        );

        let short = document("ls", "1", &root.join("man1/ls.1.gz"), 10).unwrap();
        assert!(short.body.as_deref().unwrap().len() <= 10);
        let _ = std::fs::remove_dir_all(&root);
    }
}
