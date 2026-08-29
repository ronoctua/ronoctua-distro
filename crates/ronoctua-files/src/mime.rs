//! What a file IS, by freedesktop's own database.
//!
//! # The tables are GENERATED, and the package is a build input
//!
//! `nix/scripts/gen-mimedb.py` turns `globs2`, `magic`, `subclasses`, `aliases`
//! and `generic-icons` into the arrays this module reads. The 75 MiB closure of
//! `shared-mime-info` is glib and libxml2, which only `update-mime-database`
//! needs and which already ran upstream: what is left is 125 KB of data, and
//! `files-closure-has-no-mime-db` asserts none of the rest reaches what runs.
//!
//! # Two speeds, and the boundary between them
//!
//! [`by_name`] costs no I/O: it matches the name against 1,231 patterns, in
//! memory, and it is what a listing of two hundred rows uses. [`by_content`]
//! reads the first bytes of the file and is asked for DELIBERATELY — the info
//! panel, "open with", and the case where the name says nothing at all.
//!
//! Doing magic for every row of a listing would be two hundred opens inside the
//! frame that draws it, which is the shape of defect `ronoctua_shell::apps`
//! already paid for once with icon lookups.

use std::path::Path;

use crate::Kind;

include!(concat!(env!("OUT_DIR"), "/mimedb.rs"));

// The tables came out of the generator whole or the build failed. These make
// a truncated one a COMPILE error rather than a file manager that recognises
// eleven formats.
const _: () = assert!(counts::GLOBS >= 900, "the glob table came out truncated");
const _: () = assert!(counts::MAGIC >= 300, "the magic table came out truncated");
const _: () = assert!(
    counts::SUBCLASSES >= 400,
    "the subclass table came out truncated"
);

/// What is read from a file before asking [`by_content`].
///
/// The rules reach further than this in a handful of cases and those simply do
/// not match; the alternative is reading a megabyte off a spinning disk to name
/// an icon. A cap that is stated is a limit; an unbounded read is a surprise.
pub const HEAD: usize = 4096;

/// The type nothing else resolved to.
///
/// Named rather than returned as `None`, because "I could not tell" is a real
/// answer a screen has to be able to show — and because `application/octet-stream`
/// is what the spec itself uses for it.
pub const UNKNOWN: &str = "application/octet-stream";

/// The type of a directory, which no glob will ever answer.
pub const DIRECTORY: &str = "inode/directory";

/// The type of a symlink.
pub const SYMLINK: &str = "inode/symlink";

/// The type suggested by the NAME alone.
///
/// Follows the spec's precedence: the highest weight wins; among equals, a
/// literal name beats a pattern; among those, the longest pattern.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static str> {
    let folded = name.to_lowercase();
    let mut best: Option<(&'static Glob, usize)> = None;
    for g in &GLOBS {
        let against = if g.case_sensitive { name } else { &folded };
        // A case-insensitive row was lowercased by `update-mime-database`, so
        // comparing a lowercased name against it is the whole of the folding.
        if !matches(g.pattern, against) {
            continue;
        }
        let score = g.pattern.len();
        let better = match best {
            None => true,
            Some((current, current_score)) => {
                (g.weight, is_literal(g.pattern), score)
                    > (current.weight, is_literal(current.pattern), current_score)
            }
        };
        if better {
            best = Some((g, score));
        }
    }
    best.map(|(g, _)| g.mime)
}

/// The type the CONTENT says it is.
///
/// `head` is the beginning of the file — [`HEAD`] bytes is enough for every
/// rule that matters.
#[must_use]
pub fn by_content(head: &[u8]) -> Option<&'static str> {
    // `MAGIC` comes out of the generator sorted by descending priority, so the
    // first rule that matches is the most specific one that does.
    MAGIC.iter().find(|m| rule_matches(m, head)).map(|m| m.mime)
}

/// Everything known about a path's type, name first and content only if asked.
///
/// A directory and a symlink answer from their kind: no glob describes them,
/// and reading the first bytes of a directory is an error rather than an
/// answer.
#[must_use]
pub fn of_path(path: &Path, kind: Kind, read_content: bool) -> &'static str {
    match kind {
        Kind::Directory => return DIRECTORY,
        Kind::Symlink => return SYMLINK,
        _ => {}
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(found) = by_name(&name) {
        return found;
    }
    if !read_content {
        return UNKNOWN;
    }
    read_head(path)
        .as_deref()
        .and_then(by_content)
        .unwrap_or(UNKNOWN)
}

/// The type, with the content consulted even when the name already answered.
///
/// This is what the info panel shows, and the one place where a name that LIES
/// gets caught: a `.txt` holding a PNG is a real thing, and the listing's fast
/// path would keep calling it text forever.
#[must_use]
pub fn examined(path: &Path, kind: Kind) -> (&'static str, Option<&'static str>) {
    let named = of_path(path, kind, false);
    if !matches!(kind, Kind::File) {
        return (named, None);
    }
    let sniffed = read_head(path).as_deref().and_then(by_content);
    (named, sniffed)
}

/// Whether `mime` is `parent`, or descends from it.
///
/// This is what makes a context menu extensible by FORMAT without listing every
/// format: a row that applies to `application/zip` also applies to the fifty
/// types that are a zip underneath, because the database says they are.
#[must_use]
pub fn is_a(mime: &str, parent: &str) -> bool {
    let mime = canonical(mime);
    let parent = canonical(parent);
    if mime == parent {
        return true;
    }
    // Breadth-first over the parents, with a ceiling. The database is a DAG
    // today; a cycle introduced upstream would otherwise hang the process that
    // draws the screen.
    let mut edge = vec![mime];
    for _ in 0..8 {
        let mut next = Vec::new();
        for m in edge {
            for (child, up) in &SUBCLASS {
                if *child == m {
                    if *up == parent {
                        return true;
                    }
                    next.push(*up);
                }
            }
        }
        if next.is_empty() {
            return false;
        }
        edge = next;
    }
    false
}

/// An old name traded for the current one.
#[must_use]
pub fn canonical(mime: &str) -> &str {
    ALIASES
        .binary_search_by(|(old, _)| (*old).cmp(mime))
        .map_or(mime, |i| ALIASES[i].1)
}

/// The glyph a type is drawn with.
///
/// Four steps, specific first: this type; the generic icon freedesktop assigns
/// it; the `<media>-x-generic` the spec derives for a type it does not list;
/// and finally the neutral square. The last one is deliberately reachable —
/// a type nobody has a drawing for is an honest state — but the three steps
/// before it are what keep a whole listing from becoming the same square, which
/// is indistinguishable from a table that never arrived.
#[must_use]
pub fn icon_of(mime: &str) -> char {
    let mime = canonical(mime);
    if let Ok(i) = ICON_OF_MIME.binary_search_by(|(m, _)| (*m).cmp(mime)) {
        return ICON_OF_MIME[i].1;
    }
    if let Ok(i) = GENERIC.binary_search_by(|(m, _)| (*m).cmp(mime)) {
        let name = GENERIC[i].1;
        if let Ok(j) = ICON_OF_GENERIC.binary_search_by(|(n, _)| (*n).cmp(name)) {
            return ICON_OF_GENERIC[j].1;
        }
    }
    if let Some((media, _)) = mime.split_once('/') {
        let derived = format!("{media}-x-generic");
        if let Ok(j) = ICON_OF_GENERIC.binary_search_by(|(n, _)| (*n).cmp(derived.as_str())) {
            return ICON_OF_GENERIC[j].1;
        }
    }
    ronoctua_tokens::icon::GENERIC
}

/// The first bytes of a file, or nothing.
fn read_head(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; HEAD];
    let read = file.read(&mut buffer).ok()?;
    buffer.truncate(read);
    Some(buffer)
}

/// Whether a rule's chain of matches holds.
///
/// The indent is a TREE, and a nested match is REQUIRED and not optional: a
/// match with children only counts when one of those children also matches.
///
/// # The defect this shape exists to prevent, found by a test
///
/// `image/apng` and `image/png` open with the same eight bytes. What tells them
/// apart is APNG's nested rule, which looks for an `acTL` chunk further in.
/// Treating the nested match as optional made the outer one enough — and every
/// PNG in a listing came back as an animated PNG, with the right icon and the
/// wrong name.
///
/// So a match "counts" only when it succeeded AND has no children. Whether it
/// has children is the next match's indent, which is why this walks the list
/// with a look-ahead instead of just a stack.
fn rule_matches(rule: &Magic, head: &[u8]) -> bool {
    // `held[d]` — whether the most recent match at depth `d` succeeded.
    let mut held = [false; 16];
    for (i, m) in rule.matches.iter().enumerate() {
        let depth = usize::from(m.indent).min(held.len() - 1);
        if depth > 0 && !held[depth - 1] {
            held[depth] = false;
            continue;
        }
        let ok = one_matches(m, head);
        held[depth] = ok;
        if !ok {
            continue;
        }
        let has_children = rule
            .matches
            .get(i + 1)
            .is_some_and(|next| usize::from(next.indent) > depth);
        if !has_children {
            return true;
        }
    }
    false
}

/// One byte comparison, across the range the rule allows.
fn one_matches(m: &Match, head: &[u8]) -> bool {
    let range = m.range.max(1) as usize;
    let start = m.offset as usize;
    for at in start..start.saturating_add(range) {
        let Some(window) = head.get(at..at.saturating_add(m.value.len())) else {
            break;
        };
        let same = match m.mask {
            None => window == m.value,
            Some(mask) => window
                .iter()
                .zip(m.value.iter())
                .zip(mask.iter())
                .all(|((got, want), bit)| got & bit == want & bit),
        };
        if same {
            return true;
        }
    }
    false
}

/// Whether a pattern has no wildcard at all.
fn is_literal(pattern: &str) -> bool {
    !pattern.contains(['*', '?', '['])
}

/// Shell-style matching: `*`, `?` and `[...]`.
///
/// Written out because the patterns need it: ninety-one of the database's rows
/// are not a plain `*.ext`, and among them are `makefile`, `*.tar.gz`,
/// `sconscript.*` and `*.so.[0-9]*`. Matching only `*.ext` would drop every one
/// of them, and a `Makefile` with no icon looks like a missing table.
fn matches(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    walk(&p, &n)
}

fn walk(p: &[char], n: &[char]) -> bool {
    // Iterative on the pattern, recursive only on `*`, and the recursion is
    // bounded by the name's length. The patterns here have at most two stars.
    let mut pi = 0;
    let mut ni = 0;
    while pi < p.len() {
        match p[pi] {
            '*' => {
                // A trailing star matches whatever is left.
                if pi + 1 == p.len() {
                    return true;
                }
                for skip in ni..=n.len() {
                    if walk(&p[pi + 1..], &n[skip..]) {
                        return true;
                    }
                }
                return false;
            }
            '?' => {
                if ni >= n.len() {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
            '[' => {
                let Some(close) = p[pi..].iter().position(|c| *c == ']') else {
                    // An unclosed bracket is a literal bracket, which is what
                    // every shell does and what a file called `[` needs.
                    if ni >= n.len() || n[ni] != '[' {
                        return false;
                    }
                    pi += 1;
                    ni += 1;
                    continue;
                };
                if ni >= n.len() {
                    return false;
                }
                let set = &p[pi + 1..pi + close];
                if !in_set(set, n[ni]) {
                    return false;
                }
                pi += close + 1;
                ni += 1;
            }
            literal => {
                if ni >= n.len() || n[ni] != literal {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
        }
    }
    ni == n.len()
}

/// Whether a character is in a `[...]` set, ranges and negation included.
fn in_set(set: &[char], c: char) -> bool {
    let (negated, set) = match set.first() {
        Some('!' | '^') => (true, &set[1..]),
        _ => (false, set),
    };
    let mut at = 0;
    let mut found = false;
    while at < set.len() {
        if at + 2 < set.len() && set[at + 1] == '-' {
            if set[at] <= c && c <= set[at + 2] {
                found = true;
            }
            at += 3;
        } else {
            if set[at] == c {
                found = true;
            }
            at += 1;
        }
    }
    found != negated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glob_matcher_handles_what_the_database_actually_contains() {
        // Ninety-one rows are not `*.ext`, and these are the shapes among them.
        assert!(matches("*.txt", "notas.txt"));
        assert!(!matches("*.txt", "notas.txtx"));
        assert!(matches("makefile", "makefile"));
        assert!(matches("*.tar.gz", "backup.tar.gz"));
        assert!(matches("sconscript.*", "sconscript.py"));
        assert!(matches("*~", "notas.txt~"));
        assert!(matches("*.so.[0-9]*", "libc.so.6"));
        assert!(!matches("*.so.[0-9]*", "libc.so.abc"));
        assert!(matches("?ata", "data"));
        assert!(!matches("?ata", "ata"));
        // A negated set, and a literal bracket.
        assert!(in_set(&['!', 'a', '-', 'z'], '0'));
        assert!(!in_set(&['!', 'a', '-', 'z'], 'q'));
        assert!(matches("[", "["));
    }

    #[test]
    fn the_name_answers_for_the_things_a_person_has() {
        assert_eq!(by_name("relatorio.pdf"), Some("application/pdf"));
        assert_eq!(by_name("foto.PNG"), Some("image/png"));
        assert_eq!(by_name("flake.nix"), Some("text/x-nix"));
        assert_eq!(by_name("lib.rs"), Some("text/rust"));
        assert_eq!(by_name("nothing-named-like-this"), None);
    }

    #[test]
    fn case_matters_where_the_database_says_it_does() {
        // Five rows carry the `cs` flag, and these two are why: without it a
        // file manager cannot tell C from C++.
        assert_eq!(by_name("main.c"), Some("text/x-csrc"));
        assert_eq!(by_name("main.C"), Some("text/x-c++src"));
    }

    #[test]
    fn a_longer_pattern_beats_a_shorter_one() {
        // THE DEFECT THIS CATCHES: `.tar.gz` answering `application/gzip`
        // because `*.gz` matched first. Both patterns match; the specific one
        // has to win, or a compressed archive stops being an archive.
        assert_eq!(
            by_name("backup.tar.gz"),
            Some("application/x-compressed-tar")
        );
        assert_eq!(by_name("qualquer.gz"), Some("application/gzip"));
    }

    #[test]
    fn the_content_answers_when_the_name_lies() {
        // A PNG called `.txt` — a real thing, and the whole reason magic exists.
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        assert_eq!(by_content(&png), Some("image/png"));
        // AND NOT `image/apng`, which opens with the SAME eight bytes and is
        // told apart only by a nested rule looking for an `acTL` chunk. This
        // assertion is the one that caught the nested match being treated as
        // optional — every PNG came back as an animated one.
        assert_ne!(by_content(&png), Some("image/apng"));
        let elf = [0x7F, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0];
        assert_eq!(by_content(&elf), Some("application/x-executable"));
        assert_eq!(by_content(b"nada em especial aqui"), None);
    }

    #[test]
    fn a_type_knows_what_it_descends_from() {
        // What makes a menu extensible by FORMAT: a row written for
        // `application/zip` reaches the fifty types that ARE a zip.
        assert!(is_a("application/zip", "application/zip"));
        assert!(is_a(
            "application/vnd.oasis.opendocument.text",
            "application/zip"
        ));
        assert!(!is_a("image/png", "application/zip"));
        // And it terminates on something that has no parents at all.
        assert!(!is_a("application/octet-stream", "application/zip"));
    }

    #[test]
    fn an_old_name_resolves_to_the_current_one() {
        assert_eq!(canonical("application/acrobat"), "application/pdf");
        assert_eq!(canonical("application/pdf"), "application/pdf");
        assert_eq!(canonical("does/not-exist"), "does/not-exist");
    }

    #[test]
    fn every_type_in_the_database_gets_a_glyph_that_is_not_the_fallback() {
        // THE DEFECT THIS CATCHES: a listing where every row is the same
        // neutral square, which looks deliberate and is the exact symptom of a
        // table that never reached the Rust. Measured over the whole database
        // rather than over the handful of types somebody remembered.
        let fallback = ronoctua_tokens::icon::GENERIC;
        let mut resolved = 0;
        let mut total = 0;
        for g in &GLOBS {
            total += 1;
            if icon_of(g.mime) != fallback {
                resolved += 1;
            }
        }
        let per_cent = resolved * 100 / total;
        assert!(
            per_cent >= 99,
            "only {per_cent}% of the types have a glyph of their own ({resolved} of {total})"
        );
    }

    #[test]
    fn a_directory_and_a_link_answer_from_their_kind() {
        let path = Path::new("/tmp/qualquer-coisa.png");
        assert_eq!(of_path(path, Kind::Directory, false), DIRECTORY);
        assert_eq!(of_path(path, Kind::Symlink, false), SYMLINK);
        // And a regular file still answers by name, without touching the disk.
        assert_eq!(of_path(path, Kind::File, false), "image/png");
    }
}
