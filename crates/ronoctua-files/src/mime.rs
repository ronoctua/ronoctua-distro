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
//! frame that draws it, which is the shape of defect `ronoctua_shell_door::launch`
//! already paid for once with icon lookups.

use std::collections::{HashMap, VecDeque};
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

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

/// What the NAME says, and whether it SETTLED it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Named {
    /// The winning type.
    pub mime: &'static str,
    /// Whether it won alone.
    ///
    /// `false` when two or more DIFFERENT types tie at the top of the spec's
    /// precedence — same weight, same literalness, same pattern length. The
    /// database is full of these and they are not a defect in it: `*.ogg` is
    /// claimed by `audio/ogg`, `video/ogg`, `audio/x-vorbis+ogg` and
    /// `video/x-theora+ogg` at the same weight, because the extension names a
    /// CONTAINER and the container does not say what is inside.
    pub sure: bool,
}

/// The type suggested by the NAME alone.
///
/// Follows the spec's precedence: the highest weight wins; among equals, a
/// literal name beats a pattern; among those, the longest pattern.
#[must_use]
pub fn by_name(name: &str) -> Option<&'static str> {
    named(name).map(|n| n.mime)
}

/// The same, with the TIE reported instead of dropped.
///
/// # The defect this exists for
///
/// The owner photographed a listing and asked why a VIDEO carried the music
/// glyph. The file is `big_buck_bunny_480p_stereo.ogg`: by name it is
/// `audio/ogg`, by content `video/x-theora+ogg`, and the listing resolves by
/// name only. Nothing was wrong with either answer — what was wrong is that
/// the name never had one. Four types claim `*.ogg` at the same weight and
/// this function kept whichever the table happened to hold first.
///
/// A tie is not "the best guess": it is the NAME saying it cannot tell, and
/// the only thing that can is the bytes. See [`of_path`], which is where that
/// costs a read.
#[must_use]
pub fn named(name: &str) -> Option<Named> {
    let folded = name.to_lowercase();
    let mut best: Option<(&'static Glob, usize)> = None;
    let mut tied = false;
    for g in &GLOBS {
        let against = if g.case_sensitive { name } else { &folded };
        // A case-insensitive row was lowercased by `update-mime-database`, so
        // comparing a lowercased name against it is the whole of the folding.
        if !matches(g.pattern, against) {
            continue;
        }
        let score = g.pattern.len();
        let rank = (g.weight, is_literal(g.pattern), score);
        match best {
            None => best = Some((g, score)),
            Some((current, current_score)) => {
                let theirs = (current.weight, is_literal(current.pattern), current_score);
                if rank > theirs {
                    best = Some((g, score));
                    tied = false;
                } else if rank == theirs && !same_type(g.mime, current.mime) {
                    // TWO NAMES FOR ONE TYPE ARE NOT A TIE. An alias and its
                    // canonical spelling both carry globs, and choosing
                    // between them is choosing between two spellings of the
                    // same answer.
                    tied = true;
                }
            }
        }
    }
    best.map(|(g, _)| Named {
        mime: g.mime,
        sure: !tied,
    })
}

/// Whether two spellings name the same type.
fn same_type(a: &str, b: &str) -> bool {
    canonical(a) == canonical(b)
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

/// Whether any glob of `mime` matches `name`.
///
/// What makes a content answer admissible when the name tied: the bytes are
/// choosing between the types that claim this name, not overruling the name.
/// A `.m3u` whose first line makes the magic say `application/vnd.apple.mpegurl`
/// is a `.m3u` all the same — that type claims `*.m3u8` and nothing else.
#[must_use]
pub fn claims(mime: &str, name: &str) -> bool {
    let folded = name.to_lowercase();
    let wanted = canonical(mime);
    GLOBS.iter().any(|g| {
        canonical(g.mime) == wanted
            && matches(g.pattern, if g.case_sensitive { name } else { &folded })
    })
}

/// Everything known about a path's type.
///
/// A directory and a symlink answer from their kind: no glob describes them,
/// and reading the first bytes of a directory is an error rather than an
/// answer.
///
/// # When the bytes are read, and why the flag does not cover it
///
/// `read_content` says whether a name that matches NOTHING may be looked into.
/// It is `false` for the listing, deliberately: a folder of ten thousand files
/// is ten thousand opens, and the listing is the one place that draws them all.
///
/// A name that TIES is a different question, and it is read either way. A tie
/// is the name saying it cannot tell — `*.ogg` is claimed by four types at one
/// weight, because the extension names a CONTAINER — and keeping whichever of
/// them the table held first is not a cheaper answer, it is a wrong one. The
/// owner photographed it: a Theora video in the listing with the music glyph.
///
/// The cost is bounded by how many names tie, which is a property of the
/// DATABASE and not of the folder: measured over the owner's own directory,
/// eight names of sixty-four, and every one of them a media container, a
/// playlist — or a name that ties on purpose (below): a `.cl`, a `.ts`, a
/// `.tsx`. Everything else — every `.jpg`, `.rs`, `.pdf` — is still answered
/// without touching the file. And a tied file is read ONCE: its answer is
/// kept by the file
/// (device, inode, modification time, size; [`settled`]), because a listing
/// asks again at every reread, on the loop that draws it. Under `/private`
/// nothing is read unless the vault is open, and then on a thread the caller
/// waits for an instant (`listing::Guard`): a suspended vault freezes
/// whoever touches it, and the name's first answer is given instead.
///
/// # A tie made on purpose
///
/// `assets/icons/mime-glyphs.nix` makes them (`claims` with `shares`): `*.cl`
/// is OpenCL's upstream and Common Lisp's too, and `*.ts`/`*.tsx` are an MPEG
/// transport stream's, a Qt Linguist catalogue's and a Tiled tileset's
/// upstream, and TypeScript's too. The magic still decides first; then a row
/// of [`SHARED_CONTENT`] whose pattern matches and one of whose markers is in
/// the head (`(in-package`, which no kernel source writes); and otherwise
/// upstream's type, which is the first of the tie.
#[must_use]
pub fn of_path(path: &Path, kind: Kind, read_content: bool) -> &'static str {
    of_path_by(path, kind, read_content, &crate::listing::Here)
}

/// [`of_path`], each read of the file's bytes through `guard`.
fn of_path_by(
    path: &Path,
    kind: Kind,
    read_content: bool,
    guard: &impl crate::listing::Guard,
) -> &'static str {
    match kind {
        Kind::Directory => return DIRECTORY,
        Kind::Symlink => return SYMLINK,
        _ => {}
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(found) = named(&name) {
        if found.sure {
            return found.mime;
        }
        return settled(path, &name, found.mime, guard);
    }
    if !read_content {
        return UNKNOWN;
    }
    head_by(path, guard)
        .as_deref()
        .and_then(by_content)
        .unwrap_or(UNKNOWN)
}

/// The first bytes of `path`, read through `guard`: here outside `/private`;
/// inside, only while the vault is open, and then on a thread the caller
/// waits for an instant ([`crate::listing::VAULT_PATIENCE`]) — the facts
/// panel asks on the loop that draws, and a vault suspended under the read
/// would hold that loop until the passphrase resumed it.
fn head_by(path: &Path, guard: &impl crate::listing::Guard) -> Option<Vec<u8>> {
    guard.read(path, |at| read_head(&at)).ok().flatten()
}

/// A file whose tied name its bytes settled, as the kernel knows it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TiedFile {
    dev: u64,
    ino: u64,
    mtime_s: i64,
    mtime_ns: i64,
    size: u64,
    name: String,
}

/// The answers kept, and the order they came in (the oldest goes first).
type Settled = (HashMap<TiedFile, &'static str>, VecDeque<TiedFile>);

fn kept() -> &'static Mutex<Settled> {
    static KEPT: OnceLock<Mutex<Settled>> = OnceLock::new();
    KEPT.get_or_init(|| Mutex::new((HashMap::new(), VecDeque::new())))
}

/// What a tied name is, by the file's first bytes — read once per file.
///
/// THE BYTES BREAK THE TIE, and only among the types that claim this name: a
/// file whose magic names something with no glob for it is a file lying about
/// its extension, and the extension is what the person typed. Then a tie made
/// on purpose ([`SHARED_CONTENT`]), then `first`, the name's own answer.
///
/// `guard` stands between the file and the `/private` vault
/// ([`crate::listing::Guard`]): the file's `stat` and its bytes are touched
/// only while the vault is open, and then on a thread waited for an
/// instant — this machine's ([`crate::listing::Here`]), or a vault a test
/// planted.
fn settled(
    path: &Path,
    name: &str,
    first: &'static str,
    guard: &impl crate::listing::Guard,
) -> &'static str {
    let name = name.to_owned();
    guard
        .read(path, move |at| settle(&at, &name, first))
        .unwrap_or(first)
}

/// [`settled`], touching the file here — on the guard's thread.
fn settle(path: &Path, name: &str, first: &'static str) -> &'static str {
    let Ok(meta) = std::fs::metadata(path) else {
        return first;
    };
    let file = TiedFile {
        dev: meta.dev(),
        ino: meta.ino(),
        mtime_s: meta.mtime(),
        mtime_ns: meta.mtime_nsec(),
        size: meta.size(),
        name: name.to_owned(),
    };
    if let Some(known) = kept().lock().ok().and_then(|k| k.0.get(&file).copied()) {
        return known;
    }
    // A FILE THAT COULD NOT BE READ is not kept: what changes its permission
    // does not change what the key is made of.
    let Some(head) = read_head(path) else {
        return first;
    };
    let said = by_content(&head)
        .filter(|said| claims(said, name))
        .or_else(|| shared_by_content(name, &head))
        .unwrap_or(first);
    if let Ok(mut k) = kept().lock() {
        let (answers, order) = &mut *k;
        if answers.insert(file.clone(), said).is_none() {
            order.push_back(file);
        }
        while order.len() > crate::archive::policy::MIME_TIE_ENTRIES {
            if let Some(oldest) = order.pop_front() {
                answers.remove(&oldest);
            }
        }
    }
    said
}

/// The type a tie made on purpose resolves to, by a marker in the head.
///
/// Case is not asked: Common Lisp reads `(IN-PACKAGE` as `(in-package`.
fn shared_by_content(name: &str, head: &[u8]) -> Option<&'static str> {
    let folded = name.to_lowercase();
    SHARED_CONTENT
        .iter()
        .find(|(pattern, _, markers)| {
            matches(pattern, &folded)
                && markers.iter().any(|m| {
                    head.windows(m.len())
                        .any(|w| w.eq_ignore_ascii_case(m.as_bytes()))
                })
        })
        .map(|(_, mime, _)| *mime)
}

/// The type, with the content consulted even when the name already answered.
///
/// This is what the info panel shows, and the one place where a name that LIES
/// gets caught: a `.txt` holding a PNG is a real thing, and the listing's fast
/// path would keep calling it text forever.
///
/// THE VAULT IS ASKED FIRST, as a listing asks it ([`head_by`]): the panel
/// asks this on the loop that draws, about a file that may be inside
/// `/private` — locked, or suspended at sleep while in use — long after the
/// listing that showed it was read.
#[must_use]
pub fn examined(path: &Path, kind: Kind) -> (&'static str, Option<&'static str>) {
    examined_by(path, kind, &crate::listing::Here)
}

/// [`examined`], each read of the file through `guard`.
fn examined_by(
    path: &Path,
    kind: Kind,
    guard: &impl crate::listing::Guard,
) -> (&'static str, Option<&'static str>) {
    let named = of_path_by(path, kind, false, guard);
    if !matches!(kind, Kind::File) {
        return (named, None);
    }
    let sniffed = head_by(path, guard).as_deref().and_then(by_content);
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

    /// FAILS IF: a sound only the player's decoders open falls back to
    /// "some bytes", which no program offers to open — or a family's glob
    /// starts taking a name that belongs to somebody else.
    ///
    /// The types are `assets/player-types.nix`'s, joined through `ours`.
    #[test]
    fn the_players_own_sounds_have_a_type_and_nothing_else_does() {
        assert_eq!(by_name("song.d00"), Some("audio/x-adlib"));
        assert_eq!(by_name("mdat.title"), Some("audio/x-amiga-music"));
        assert_eq!(by_name("track.minipsf2"), Some("audio/x-console-rip"));
        assert_eq!(by_name("theme.brstm"), Some("audio/x-game-stream"));
        assert_eq!(by_name("tune.rsid"), Some("audio/x-rsid"));
        assert!(is_a("audio/x-rsid", "audio/prs.sid"));
        // LEFT OUT ON PURPOSE: a font, a genome, a disc image.
        assert_ne!(by_name("face.cff"), Some("audio/x-adlib"));
        assert_ne!(by_name("reads.bam"), Some("audio/x-adlib"));
        assert_ne!(
            by_name("disc.iso").map(|m| m.starts_with("audio/")),
            Some(true)
        );
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
        assert_eq!(by_name("anything.gz"), Some("application/gzip"));
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
        assert_eq!(by_content(b"nothing special here"), None);
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

    /// FAILS IF: the generator stops writing the spec's implicit "every
    /// `text/*` is a `text/plain`" into the table. shared-mime-info 2.5.1
    /// leaves it implicit on a handful of types, and without the rows a `.sh`
    /// (`text/x-shellscript`, explicitly only an `application/x-executable`)
    /// and a `.gradle` (no parent at all) stop being text to every `is_a`.
    #[test]
    fn every_text_type_is_plain_text() {
        assert!(is_a("text/x-shellscript", "text/plain"));
        assert!(is_a("application/x-shellscript", "text/plain"));
        assert!(is_a("text/x-gradle", "text/plain"));
        // THROUGH ITS PARENT, which is what got the row.
        assert!(is_a("text/x-csh", "text/plain"));
        let mut text = 0;
        for g in &GLOBS {
            if g.mime.starts_with("text/") {
                text += 1;
                assert!(is_a(g.mime, "text/plain"), "`{}` is not text", g.mime);
            }
        }
        assert!(text > 100, "only {text} names of `text/*` types");
        // AND THE RULE IS THE SPEC'S, not a wider one.
        assert!(!is_a("image/png", "text/plain"));
    }

    /// FAILS IF: a 3D model's extension stops naming the model alone — the
    /// shape shared-mime-info 2.5.1 brought twice: `application/x-coff`
    /// claims `*.obj` at `model/obj`'s weight and ahead of it (the second
    /// `prefer` row), and `*.amf` became upstream's `application/x-amf`, which
    /// a type invented here would tie with in silence.
    #[test]
    fn a_model_is_named_by_its_own_type() {
        for (name, mime) in [
            ("part.obj", "model/obj"),
            ("part.amf", "application/x-amf"),
            ("part.step", "model/step"),
            ("part.stp", "model/step"),
        ] {
            let found = named(name).expect("the database knows the name");
            assert_eq!(found.mime, mime, "{name}");
            assert!(found.sure, "`{name}` came back as a tie");
        }
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
        let path = Path::new("/tmp/anything-at-all.png");
        assert_eq!(of_path(path, Kind::Directory, false), DIRECTORY);
        assert_eq!(of_path(path, Kind::Symlink, false), SYMLINK);
        // And a regular file still answers by name, without touching the disk.
        assert_eq!(of_path(path, Kind::File, false), "image/png");
    }

    /// FAILS IF: a name that several types claim goes back to being answered
    /// by whichever of them the table holds first.
    #[test]
    fn a_name_several_types_claim_says_it_did_not_settle_it() {
        // AN EXTENSION THAT NAMES A CONTAINER cannot say what is inside, and
        // the database is honest about it: four types claim `*.ogg` at one
        // weight. A single claimant is a settled answer.
        let tied = named("song.ogg").expect("the database knows `*.ogg`");
        assert!(!tied.sure, "`*.ogg` came back as settled");
        let sure = named("photo.png").expect("the database knows `*.png`");
        assert!(sure.sure, "`*.png` came back as a tie");
        assert_eq!(sure.mime, "image/png");
        // AND `by_name` STILL ANSWERS, because most callers only want a type.
        assert_eq!(by_name("photo.png"), Some("image/png"));
    }

    /// FAILS IF: the bytes stop breaking a tie, or start overruling a name
    /// that settled one.
    ///
    /// THE DEFECT, photographed by the owner: a listing where a VIDEO carried
    /// the music glyph. `big_buck_bunny_480p_stereo.ogg` is `audio/ogg` by
    /// name and `video/x-theora+ogg` by content, and the listing resolves by
    /// name alone.
    #[test]
    fn the_bytes_break_a_tie_and_only_a_tie() {
        let dir = std::env::temp_dir().join(format!("ronoctua-mime-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test directory");

        // A TIED NAME, and bytes that name one of its claimants: `OggS` plus
        // the Theora identification header is what the database matches on.
        let ogg = dir.join("clip.ogg");
        let mut bytes = b"OggS\x00\x02".to_vec();
        bytes.resize(28, 0);
        bytes.extend_from_slice(b"\x80theora");
        bytes.resize(4096, 0);
        std::fs::write(&ogg, &bytes).expect("write the fixture");
        assert_eq!(of_path(&ogg, Kind::File, false), "video/x-theora+ogg");

        // A SETTLED NAME IS NOT OVERRULED. The same bytes under a name only
        // one type claims stay what the name says — an extension is what the
        // person typed, and a file that lies about it is still called that.
        let png = dir.join("clip.png");
        std::fs::write(&png, &bytes).expect("write the fixture");
        assert_eq!(of_path(&png, Kind::File, false), "image/png");

        // AND A CONTENT ANSWER THAT DOES NOT CLAIM THIS NAME IS NOT TAKEN.
        let lying = dir.join("lying.ogg");
        std::fs::write(&lying, b"\x89PNG\r\n\x1a\n").expect("write the fixture");
        assert_eq!(
            of_path(&lying, Kind::File, false),
            by_name("lying.ogg").unwrap()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the claims `mime-glyphs.nix` adds or removes stop reaching
    /// the table: a `.lsp` back to no type, a `.fasl` back to Lisp SOURCE (and
    /// the preview showing compiled bytes as text), or `*.cl` settled by
    /// the table's order instead of reported as a tie.
    #[test]
    fn common_lisp_names_are_claimed_and_a_fasl_is_not_source() {
        assert_eq!(by_name("init.lsp"), Some("text/x-common-lisp"));
        assert_eq!(by_name("app.asd"), Some("text/x-common-lisp"));
        assert_eq!(by_name("app.lisp"), Some("text/x-common-lisp"));
        assert_eq!(by_name("app.fasl"), None);
        let cl = named("kernel.cl").expect("the database knows `*.cl`");
        assert!(!cl.sure, "`*.cl` came back as settled");
        // UPSTREAM'S TYPE IS THE FIRST OF THE TIE, so a name alone still says
        // what it said before.
        assert_eq!(cl.mime, "text/x-opencl-csrc");
    }

    /// FAILS IF: the tie `*.cl` is made on stops being broken by the head —
    /// a Lisp file keeps OpenCL's type and draws as C — or starts taking a
    /// kernel source, or anything that says neither, for Lisp.
    #[test]
    fn a_cl_file_is_lisp_only_when_its_head_says_so() {
        let dir = std::env::temp_dir().join(format!("ronoctua-mime-cl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test directory");
        let lisp = dir.join("package.cl");
        std::fs::write(&lisp, b";;;; The package.\n\n(IN-PACKAGE #:cl-user)\n").expect("write");
        assert_eq!(of_path(&lisp, Kind::File, false), "text/x-common-lisp");
        let system = dir.join("app.cl");
        std::fs::write(
            &system,
            b"(defsystem \"app\" :components ((:file \"a\")))\n",
        )
        .expect("write");
        assert_eq!(of_path(&system, Kind::File, false), "text/x-common-lisp");
        let kernel = dir.join("blur.cl");
        std::fs::write(
            &kernel,
            b"__kernel void blur(__global float *a) { a[0] = 1; }\n",
        )
        .expect("write");
        assert_eq!(of_path(&kernel, Kind::File, false), "text/x-opencl-csrc");
        let neither = dir.join("notes.cl");
        std::fs::write(&neither, b"nothing that says what this is\n").expect("write");
        assert_eq!(of_path(&neither, Kind::File, false), "text/x-opencl-csrc");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a TypeScript file stops being TypeScript — named by the Qt
    /// Linguist catalogue or the MPEG transport stream that claim `*.ts` at
    /// 50 upstream (TypeScript's own claim is at 40), or by the Tiled tileset
    /// that claims `*.tsx` — or a catalogue, a stream or a tileset starts
    /// being taken for TypeScript.
    ///
    /// Before the `claims` rows, the name `.ts` was a tie TypeScript (then an
    /// invented `text/x-typescript`) never won: a script came out as a Qt
    /// catalogue, and was coloured as XML.
    #[test]
    fn a_typescript_file_is_typescript_and_only_it() {
        let dir = std::env::temp_dir().join(format!("ronoctua-mime-ts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test directory");
        let of = |name: &str, bytes: &[u8]| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).expect("write the fixture");
            of_path(&path, Kind::File, false)
        };
        const TYPESCRIPT: &str = "application/typescript";

        // A NAME ONLY TYPESCRIPT GIVES is settled by the name.
        let cts = named("config.cts").expect("the database knows `*.cts`");
        assert_eq!((cts.mime, cts.sure), (TYPESCRIPT, true));
        // AND THE TWO THAT TIE are reported as ties.
        assert!(!named("main.ts").expect("`*.ts`").sure);
        assert!(!named("view.tsx").expect("`*.tsx`").sure);

        // A SCRIPT, by what a script writes — led by an import, which no
        // magic names, and by a comment, which TypeScript's own magic does.
        assert_eq!(
            of(
                "main.ts",
                b"import { a } from \"./a\";\n\nconsole.log(a);\n"
            ),
            TYPESCRIPT
        );
        assert_eq!(
            of(
                "lib.ts",
                b"// The library.\nexport function f(): number { return 1; }\n"
            ),
            TYPESCRIPT
        );
        assert_eq!(
            of(
                "view.tsx",
                b"import React from 'react';\n\nexport default function View() { return <div />; }\n"
            ),
            TYPESCRIPT
        );

        // A CATALOGUE is named by its magic before any marker is looked for,
        // and this one says `Import ` in its text.
        assert_eq!(
            of(
                "app_pt.ts",
                b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE TS>\n\
                  <TS version=\"2.1\" language=\"pt_BR\">\n<context>\n<name>Main</name>\n\
                  <message><source>Import settings</source>\
                  <translation>Importar ajustes</translation></message>\n\
                  </context>\n</TS>\n"
            ),
            "text/vnd.trolltech.linguist"
        );
        // A STREAM, by its sync bytes every 188.
        let mut stream = vec![0u8; 1024];
        for at in [0, 188, 376, 564, 752] {
            stream[at] = 0x47;
        }
        assert_eq!(of("clip.ts", &stream), "video/mp2t");
        // A TILESET writes `<export target=…>`, and is still a tileset.
        assert_eq!(
            of(
                "tiles.tsx",
                b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                  <tileset version=\"1.10\" name=\"t\" tilewidth=\"16\" tileheight=\"16\" \
                  tilecount=\"4\" columns=\"2\">\n <export target=\"t.json\" format=\"json\"/>\n \
                  <image source=\"t.png\" width=\"32\" height=\"32\"/>\n</tileset>\n"
            ),
            "application/x-tiled-tsx"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a tied name's file is read again at every reread of the
    /// listing — on the loop that draws it — or its kept answer outlives a
    /// change of the file; or a file under a vault that may not be touched is
    /// read at all.
    #[test]
    fn a_tied_file_is_read_once_and_never_in_a_closed_vault() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("ronoctua-mime-kept-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test directory");
        let lisp = dir.join("kept.cl");
        std::fs::write(&lisp, b"(in-package #:kept)\n").expect("write");
        assert_eq!(of_path(&lisp, Kind::File, false), "text/x-common-lisp");
        // UNREADABLE NOW, and the same file by its stamp: the answer kept is
        // the answer given, with no read.
        std::fs::set_permissions(&lisp, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        let unreadable = std::fs::read(&lisp).is_err();
        let again = of_path(&lisp, Kind::File, false);
        std::fs::set_permissions(&lisp, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        if unreadable {
            assert_eq!(again, "text/x-common-lisp", "the file was read again");
        }
        // A CHANGED FILE is read again.
        std::fs::write(&lisp, b"__kernel void k(void) {}\n/* a longer kernel */\n").expect("write");
        assert_eq!(of_path(&lisp, Kind::File, false), "text/x-opencl-csrc");
        // AND A VAULT THAT MAY NOT BE TOUCHED is not read: the name's answer.
        let _one = crate::listing::VAULT_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (vault, status) = vault_under(&dir);
        let guard = crate::listing::Planted {
            mount_point: &vault,
            status_file: &status,
            patience: std::time::Duration::from_secs(5),
        };
        let vaulted = vault.join("vaulted.cl");
        std::fs::write(&vaulted, b"(in-package #:vaulted)\n").expect("write");
        say_state(&status, ronoctua_private::State::Locked);
        assert_eq!(
            settled(&vaulted, "vaulted.cl", "text/x-opencl-csrc", &guard),
            "text/x-opencl-csrc"
        );
        say_state(&status, ronoctua_private::State::Open);
        assert_eq!(
            settled(&vaulted, "vaulted.cl", "text/x-opencl-csrc", &guard),
            "text/x-common-lisp"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A `/private` vault planted under `root`: its mount point, made, and
    /// the status file that will say its state.
    fn vault_under(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let mount = root.join("private");
        std::fs::create_dir_all(&mount).expect("the vault");
        (mount, root.join("status.json"))
    }

    /// The planted vault's status file says `state`.
    fn say_state(status: &Path, state: ronoctua_private::State) {
        let written = ronoctua_private::Status {
            state,
            ..ronoctua_private::Status::absent()
        };
        std::fs::write(status, serde_json::to_vec(&written).expect("a status"))
            .expect("write the status");
    }

    /// FAILS IF: what the facts panel asks of a file inside the `/private`
    /// vault — its type by name and by its bytes ([`examined`]) — touches
    /// the file while the vault is locked or suspended, or touches it on the
    /// thread that asked while it is open: a read the vault holds (a pipe
    /// here, which holds whoever opens it as a suspended vault does) must let
    /// the asking loop go within its patience, and the vault answers again
    /// once it let go. A file outside the vault is read as before.
    #[test]
    fn the_facts_panel_never_touches_the_vault_on_the_loop() {
        use std::sync::mpsc;
        use std::time::Duration;
        let _one = crate::listing::VAULT_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = std::env::temp_dir().join(format!("ronoctua-mime-vault-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (vault, status) = vault_under(&root);
        // A PICTURE NAMED AS TEXT: its bytes say so when it is read, and the
        // name alone answers when it is not.
        let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        let notes = vault.join("notes.txt");
        std::fs::write(&notes, png).expect("write");
        let outside = root.join("outside.txt");
        std::fs::write(&outside, png).expect("write");
        let held = vault.join("held.txt");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &held,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .expect("a pipe");
        let guard = crate::listing::Planted {
            mount_point: &vault,
            status_file: &status,
            patience: Duration::from_millis(50),
        };
        let asked = |path: &Path| examined_by(path, Kind::File, &guard);
        assert_eq!(asked(&outside), ("text/plain", Some("image/png")));
        for shut in [
            ronoctua_private::State::Locked,
            ronoctua_private::State::SuspendedInUse,
        ] {
            say_state(&status, shut);
            assert_eq!(asked(&notes), ("text/plain", None), "{shut:?}: read");
            // NOT EVEN OPENED: the pipe would hold this thread for ever.
            assert_eq!(asked(&held), ("text/plain", None), "{shut:?}: opened");
        }
        say_state(&status, ronoctua_private::State::Open);
        assert_eq!(asked(&notes), ("text/plain", Some("image/png")));
        // HELD UNDER THE READ: the asking thread is let go — on a thread of
        // its own, so a read made here would fail the wait, not hang it.
        let (said, heard) = mpsc::channel();
        let (mount, file) = (vault.clone(), status.clone());
        let pipe = held.clone();
        std::thread::spawn(move || {
            let guard = crate::listing::Planted {
                mount_point: &mount,
                status_file: &file,
                patience: Duration::from_millis(50),
            };
            let _ = said.send(examined_by(&pipe, Kind::File, &guard));
        });
        let answer = heard
            .recv_timeout(Duration::from_secs(20))
            .expect("the panel's read of the vault held the thread that asked");
        assert_eq!(answer, ("text/plain", None));
        // WHILE IT IS HELD, the vault is not touched again.
        assert_eq!(asked(&notes), ("text/plain", None), "touched while held");
        // THE VAULT LETS GO (a writer comes, and goes): it answers again.
        drop(
            std::fs::OpenOptions::new()
                .write(true)
                .open(&held)
                .expect("the pipe's other end"),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while asked(&notes).1.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(asked(&notes), ("text/plain", Some("image/png")));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: `claims` starts answering for a type that has no glob for the
    /// name — which is what admits a content answer when the name tied.
    #[test]
    fn a_type_claims_only_the_names_its_globs_match() {
        assert!(claims("audio/x-mpegurl", "a.m3u"));
        assert!(claims("application/vnd.apple.mpegurl", "a.m3u8"));
        assert!(!claims("image/png", "a.m3u"));
        // AND AN ALIAS IS THE SAME TYPE, so it claims the same names.
        assert!(claims("image/png", "a.png"));
    }
}
