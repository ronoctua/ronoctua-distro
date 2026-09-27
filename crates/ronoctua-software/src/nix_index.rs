//! The search index built from the system's own nixpkgs (`RONOCTUA_NIX_INDEX`).
//!
//! See `nix/scripts/software-index.py` for the format: a header line, then one
//! package per line, TAB-separated and sorted by attribute —
//! `attr pname version flags program license homepage description`.
//!
//! # Why a text file and not a database
//!
//! Because the two questions it answers are both cheap on a sorted text file:
//! "the row of this attribute" is a binary search that reads a few pages
//! ([`row`]), and "what matches this query" is one linear pass ([`search`]).
//! Measured over the real index (144 198 packages, 18 MB), optimised build
//! (`--profile loop`): one search reads and walks the whole file in 27–58 ms on
//! the caller's thread (160–360 ms unoptimised), and a lookup by attribute takes
//! 0.08 ms. That is what makes it affordable to keep NOTHING resident — the
//! footer is a process that lives for months, and 18 MB held for a search typed
//! once a week is 18 MB held for nothing. The caller debounces the keystrokes.
//!
//! # Who calls it, and where
//!
//! The shell's search, on its search thread (never the event loop), and the
//! CLI before an install or a trial, to learn whether the attribute exists and
//! whether its licence needs the person's yes.

use std::fs::File;
use std::os::unix::fs::FileExt as _;
use std::path::Path;

use crate::found::{Badge, Found};
use crate::{AppKey, Refusal};

/// What the first field of the header says.
pub const HEADER: &str = "#ronoctua-software-nix-index";
/// The format this reader understands. Another number is refused, never
/// guessed at: columns read in the wrong order are a search that lies.
pub const FORMAT: &str = "1";

/// One package of the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The attribute path.
    pub attr: String,
    /// The package name.
    pub pname: String,
    /// The version.
    pub version: String,
    /// Unfree.
    pub unfree: bool,
    /// Broken.
    pub broken: bool,
    /// Insecure.
    pub insecure: bool,
    /// Not available on this platform.
    pub unavailable: bool,
    /// A library or language package (a nested set).
    pub library: bool,
    /// `meta.mainProgram`.
    pub program: String,
    /// SPDX licence (or the short name).
    pub license: String,
    /// The first homepage.
    pub homepage: String,
    /// One line on what it does.
    pub description: String,
}

impl Row {
    /// One line of the index, or `None` for a line with the wrong number of
    /// columns (a truncated file is refused line by line, not trusted).
    #[must_use]
    pub fn of_line(line: &str) -> Option<Self> {
        Fields::of(line).map(|f| f.row())
    }

    /// The refusal that makes installing or trying it impossible, if one does.
    /// Unfree is NOT one: it is a question, and the person may say yes.
    #[must_use]
    pub const fn refusal(&self) -> Option<Refusal> {
        if self.broken {
            Some(Refusal::Broken)
        } else if self.insecure {
            Some(Refusal::Insecure)
        } else if self.unavailable {
            Some(Refusal::Unavailable)
        } else {
            None
        }
    }

    /// The badges a result row carries.
    #[must_use]
    pub fn badges(&self) -> Vec<Badge> {
        badges_of(self.unfree, self.broken, self.insecure, self.library)
    }

    /// The row as a search result.
    #[must_use]
    pub fn found(&self, by_description: bool) -> Found {
        Found {
            key: AppKey::Nix {
                attr: self.attr.clone(),
            },
            name: if self.pname.is_empty() {
                self.attr.clone()
            } else {
                self.pname.clone()
            },
            summary: self.description.clone(),
            version: self.version.clone(),
            badges: self.badges(),
            download_bytes: None,
            icon: None,
            homepage: some(&self.homepage),
            license: some(&self.license),
            publisher: None,
            program: some(&self.program),
            by_description,
        }
    }
}

fn some(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

fn badges_of(unfree: bool, broken: bool, insecure: bool, library: bool) -> Vec<Badge> {
    [
        (unfree, Badge::Unfree),
        (broken, Badge::Broken),
        (insecure, Badge::Insecure),
        (library, Badge::Library),
    ]
    .into_iter()
    .filter_map(|(on, b)| on.then_some(b))
    .collect()
}

/// A line split into its columns, borrowed — what the linear pass reads, so a
/// row that does not match costs no allocation.
struct Fields<'a> {
    attr: &'a str,
    pname: &'a str,
    version: &'a str,
    flags: &'a str,
    program: &'a str,
    license: &'a str,
    homepage: &'a str,
    description: &'a str,
}

impl<'a> Fields<'a> {
    fn of(line: &'a str) -> Option<Self> {
        let mut f = line.trim_end_matches(['\r', '\n']).splitn(8, '\t');
        let out = Self {
            attr: f.next()?,
            pname: f.next()?,
            version: f.next()?,
            flags: f.next()?,
            program: f.next()?,
            license: f.next()?,
            homepage: f.next()?,
            description: f.next()?,
        };
        (!out.attr.is_empty()).then_some(out)
    }

    fn row(&self) -> Row {
        Row {
            attr: self.attr.to_owned(),
            pname: self.pname.to_owned(),
            version: self.version.to_owned(),
            unfree: self.flags.contains('u'),
            broken: self.flags.contains('b'),
            insecure: self.flags.contains('i'),
            unavailable: self.flags.contains('n'),
            library: self.flags.contains('l'),
            program: self.program.to_owned(),
            license: self.license.to_owned(),
            homepage: self.homepage.to_owned(),
            description: self.description.to_owned(),
        }
    }
}

/// Whether a header line is one this reader understands. It carries a third
/// field, the count, which the reader does not need.
#[must_use]
pub fn header_is_ours(line: &str) -> bool {
    let mut f = line.trim_end_matches(['\r', '\n']).split('\t');
    f.next() == Some(HEADER) && f.next() == Some(FORMAT)
}

/// How many packages the index says it holds, or why it cannot be read: a
/// missing or foreign file is [`Refusal::SourceAbsent`] — to a person, an
/// index that cannot be read and no index are the same missing search.
///
/// # Errors
///
/// The refusal, as a value.
pub fn check(index: &Path) -> Result<usize, Refusal> {
    let first = first_line(index).ok_or(Refusal::SourceAbsent)?;
    if !header_is_ours(&first) {
        return Err(Refusal::SourceAbsent);
    }
    Ok(first
        .split('\t')
        .nth(2)
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0))
}

fn first_line(index: &Path) -> Option<String> {
    let head = crate::run::head_of(index, 256).ok()?;
    let end = head.iter().position(|&b| b == b'\n')?;
    String::from_utf8(head[..end].to_vec()).ok()
}

/// How well a row matched, lower is better — the sort key of a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    /// The query IS the attribute, the name or the command (`rg`).
    Exact,
    /// One of them starts with it — or the attribute's last segment does, so
    /// `requests` finds `python3Packages.requests` when libraries are asked for.
    Prefix,
    /// One of them contains it.
    Contains,
    /// Every word of it is in what the package says it does.
    Description,
}

/// The query, folded once.
struct Query {
    whole: String,
    words: Vec<String>,
}

impl Query {
    fn of(text: &str) -> Option<Self> {
        let whole = ronoctua_apps::fold::fold(text.trim());
        if whole.is_empty() {
            return None;
        }
        let words = whole.split_whitespace().map(str::to_owned).collect();
        Some(Self { whole, words })
    }

    /// The tier a row reaches, if any. `buf` is a scratch string reused for
    /// every row, so folding costs no allocation per line.
    fn tier(&self, f: &Fields<'_>, buf: &mut String) -> Option<Tier> {
        let q = self.whole.as_str();
        let names = [f.attr, f.pname, f.program];
        let mut best: Option<Tier> = None;
        for name in names {
            if name.is_empty() {
                continue;
            }
            fold_into(buf, name);
            let tier = if buf == q {
                Some(Tier::Exact)
            } else if buf.starts_with(q) {
                Some(Tier::Prefix)
            } else if buf.contains(q) {
                Some(Tier::Contains)
            } else {
                None
            };
            best = min_tier(best, tier);
            if best == Some(Tier::Exact) {
                return best;
            }
        }
        // The last segment of a nested attribute (`python3Packages.requests`).
        if let Some((_, last)) = f.attr.rsplit_once('.') {
            fold_into(buf, last);
            let tier = if buf == q {
                Some(Tier::Exact)
            } else if buf.starts_with(q) {
                Some(Tier::Prefix)
            } else {
                None
            };
            best = min_tier(best, tier);
        }
        if best.is_some() {
            return best;
        }
        // What it does: every word somewhere in the description or the names,
        // so `image editor` finds GIMP and `pdf viewer` finds zathura.
        fold_into(buf, f.description);
        buf.push('\t');
        for name in names {
            fold_into_append(buf, name);
            buf.push('\t');
        }
        self.words
            .iter()
            .all(|w| buf.contains(w.as_str()))
            .then_some(Tier::Description)
    }
}

fn min_tier(a: Option<Tier>, b: Option<Tier>) -> Option<Tier> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, None) => a,
        (None, b) => b,
    }
}

/// `ronoctua_apps::fold::fold`, into a reused buffer — the same ruler, one
/// character for one character.
fn fold_into(buf: &mut String, s: &str) {
    buf.clear();
    fold_into_append(buf, s);
}

fn fold_into_append(buf: &mut String, s: &str) {
    if s.is_ascii() {
        buf.extend(s.bytes().map(|b| char::from(b.to_ascii_lowercase())));
    } else {
        buf.extend(
            s.chars()
                .map(|c| ronoctua_apps::fold::folded_char(c).to_ascii_lowercase()),
        );
    }
}

/// Search the index. Libraries are left out unless `libraries`; at most
/// `limit` rows, name matches before description matches.
///
/// Packages that are not available on this platform are ALWAYS left out: no
/// order could install or try them here, and there is no badge that would
/// say so before the person asked. Broken and insecure ones are kept, with
/// their badge, and sorted after the healthy ones of the same tier — seeing
/// that nixpkgs has the program and that it is broken is an answer.
///
/// Reads the whole file (18 MB): call it on a thread. A missing or foreign
/// index answers nothing; [`check`] says why.
#[must_use]
pub fn search(index: &Path, query: &str, libraries: bool, limit: usize) -> Vec<Found> {
    let Some(query) = Query::of(query) else {
        return Vec::new();
    };
    if limit == 0 {
        return Vec::new();
    }
    let text = match std::fs::read_to_string(index) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(index = %index.display(), error = %e, "the Nix index cannot be read");
            return Vec::new();
        }
    };
    search_text(&text, &query, libraries, limit)
}

fn search_text(text: &str, query: &Query, libraries: bool, limit: usize) -> Vec<Found> {
    let mut lines = text.lines();
    if !lines.next().is_some_and(header_is_ours) {
        tracing::warn!(
            "the Nix index has a header this reader does not know; refusing to read its columns"
        );
        return Vec::new();
    }
    let mut buf = String::with_capacity(256);
    // (tier, sick, library, attribute length, the fields) — the sort key
    // then the row; only the rows that make the cut become a `Found`.
    let mut hits: Vec<(Tier, bool, bool, usize, Fields<'_>)> = Vec::new();
    for line in lines {
        let Some(f) = Fields::of(line) else { continue };
        let library = f.flags.contains('l');
        if f.flags.contains('n') || (library && !libraries) {
            continue;
        }
        if let Some(tier) = query.tier(&f, &mut buf) {
            let sick = f.flags.contains('b') || f.flags.contains('i');
            hits.push((tier, sick, library, f.attr.len(), f));
        }
    }
    let key = |h: &(Tier, bool, bool, usize, Fields<'_>)| (h.0, h.1, h.2, h.3);
    let order = |a: &(Tier, bool, bool, usize, Fields<'_>),
                 b: &(Tier, bool, bool, usize, Fields<'_>)| {
        key(a).cmp(&key(b)).then_with(|| a.4.attr.cmp(b.4.attr))
    };
    if hits.len() > limit {
        hits.select_nth_unstable_by(limit - 1, order);
        hits.truncate(limit);
    }
    hits.sort_unstable_by(order);
    hits.into_iter()
        .map(|(tier, _, _, _, f)| f.row().found(tier == Tier::Description))
        .collect()
}

/// The row of one attribute (binary search: the file is sorted), reading a
/// few pages and never the whole file. `None` when there is no such
/// attribute, or the index cannot be read or is foreign.
#[must_use]
pub fn row(index: &Path, attr: &str) -> Option<Row> {
    if attr.is_empty() || attr.contains(['\t', '\n']) {
        return None;
    }
    let file = File::open(index).ok()?;
    let len = file.metadata().ok()?.len();
    let first = line_at(&file, 0, len).ok()??;
    if !header_is_ours(&first) {
        tracing::warn!(index = %index.display(), "the Nix index has a header this reader does not know");
        return None;
    }
    let mut lo = first.len() as u64 + 1;
    let mut hi = len;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let start = line_start_from(&file, mid, len).ok()?;
        if start >= hi {
            // No line starts in [mid, hi): what is left starts in [lo, mid).
            hi = mid;
            continue;
        }
        let line = line_at(&file, start, len).ok()??;
        let here = line.split('\t').next().unwrap_or_default();
        match here.cmp(attr) {
            std::cmp::Ordering::Equal => return Row::of_line(&line),
            std::cmp::Ordering::Less => lo = start + line.len() as u64 + 1,
            std::cmp::Ordering::Greater => hi = start,
        }
    }
    None
}

/// Every row of `attrs` that exists, in one pass per attribute — for the
/// installed list, which asks about each element of the profile.
#[must_use]
pub fn rows(index: &Path, attrs: &[&str]) -> std::collections::BTreeMap<String, Row> {
    attrs
        .iter()
        .filter_map(|a| row(index, a).map(|r| ((*a).to_owned(), r)))
        .collect()
}

const PAGE: usize = 4096;

/// The first position `>= at` where a line starts.
fn line_start_from(file: &File, at: u64, len: u64) -> std::io::Result<u64> {
    if at == 0 {
        return Ok(0);
    }
    let mut pos = at - 1;
    let mut page = [0u8; PAGE];
    while pos < len {
        let n = file.read_at(&mut page, pos)?;
        if n == 0 {
            break;
        }
        if let Some(i) = page[..n].iter().position(|&b| b == b'\n') {
            return Ok(pos + i as u64 + 1);
        }
        pos += n as u64;
    }
    Ok(len)
}

/// The line starting at `start`, without its newline; `None` past the end or
/// for bytes that are not UTF-8.
fn line_at(file: &File, start: u64, len: u64) -> std::io::Result<Option<String>> {
    if start >= len {
        return Ok(None);
    }
    let mut out = Vec::new();
    let mut pos = start;
    let mut page = [0u8; PAGE];
    loop {
        let n = file.read_at(&mut page, pos)?;
        if n == 0 {
            break;
        }
        if let Some(i) = page[..n].iter().position(|&b| b == b'\n') {
            out.extend_from_slice(&page[..i]);
            break;
        }
        out.extend_from_slice(&page[..n]);
        pos += n as u64;
    }
    Ok(String::from_utf8(out).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = "\
#ronoctua-software-nix-index\t1\t9
CuboCore.coreaction\tcoreaction\t5.0.1\tl\tcoreaction\tGPL-3.0-or-later\thttps://gitlab.com/cubocore\tSide bar for showing widgets
aerospace\taerospace\t0.20.3\tn\taerospace\tMIT\thttps://example.org\tTiling window manager for another platform
alexandria\talexandria\t0.13.1\tb\talexandria\tGPL-2.0-or-later\t\tBook collection manager
gimp\tgimp\t3.0.8\t\tgimp\tGPL-3.0-or-later\thttps://www.gimp.org/\tGNU Image Manipulation Program
google-chrome\tgoogle-chrome\t151.0.7922.71\tu\tgoogle-chrome-stable\tunfree\thttps://www.google.com/chrome/browser/\tFreeware web browser developed by Google
hello\thello\t2.12.3\t\thello\tGPL-3.0-or-later\thttps://www.gnu.org/software/hello/manual/\tProgram that produces a familiar, friendly greeting
python3Packages.requests\trequests\t2.32.5\tl\t\tApache-2.0\thttps://requests.readthedocs.io\tHTTP library for Python
ripgrep\tripgrep\t15.1.0\t\trg\tUnlicense\thttps://github.com/BurntSushi/ripgrep\tUtility that combines the usability of The Silver Searcher with the raw speed of grep
zathura\tzathura\t0.5.14\t\tzathura\tZlib\thttps://pwmt.org/projects/zathura\tA highly customizable and functional PDF viewer — café
";

    fn scratch(tag: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-nix-index-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.tsv");
        std::fs::write(&path, text).unwrap();
        path
    }

    fn attrs(found: &[Found]) -> Vec<&str> {
        found.iter().map(|f| f.key.id()).collect()
    }

    #[test]
    fn a_command_finds_its_package_and_the_exact_name_comes_first() {
        let index = scratch("command", INDEX);
        let found = search(&index, "rg", false, 10);
        assert_eq!(
            found[0].key,
            AppKey::Nix {
                attr: "ripgrep".into()
            }
        );
        assert_eq!(found[0].program.as_deref(), Some("rg"));
        assert!(!found[0].by_description);
        let found = search(&index, "Hello", false, 10);
        assert_eq!(attrs(&found)[0], "hello");
    }

    #[test]
    fn names_come_before_descriptions_and_every_word_must_be_there() {
        let index = scratch("words", INDEX);
        let found = search(&index, "image manipulation", false, 10);
        assert_eq!(attrs(&found), ["gimp"]);
        assert!(found[0].by_description);
        assert!(search(&index, "image spaceship", false, 10).is_empty());
        // The accent folds on both sides.
        assert_eq!(attrs(&search(&index, "CAFE", false, 10)), ["zathura"]);
    }

    #[test]
    fn libraries_are_left_out_unless_asked_and_the_last_segment_counts() {
        let index = scratch("libraries", INDEX);
        assert!(search(&index, "requests", false, 10).is_empty());
        let found = search(&index, "requests", true, 10);
        assert_eq!(attrs(&found), ["python3Packages.requests"]);
        assert!(found[0].badges.contains(&Badge::Library));
        assert_eq!(found[0].program, None, "an empty mainProgram is no program");
    }

    #[test]
    fn a_package_for_another_platform_is_never_found_and_a_broken_one_says_so() {
        let index = scratch("sick", INDEX);
        assert!(search(&index, "aerospace", true, 10).is_empty());
        let found = search(&index, "alexandria", false, 10);
        assert_eq!(found[0].badges, [Badge::Broken]);
        let found = search(&index, "chrome", false, 10);
        assert_eq!(found[0].badges, [Badge::Unfree]);
        assert_eq!(found[0].license.as_deref(), Some("unfree"));
    }

    #[test]
    fn the_limit_keeps_the_best() {
        let index = scratch("limit", INDEX);
        // `e` is in nearly everything; the exact and prefix tiers survive the cut.
        let found = search(&index, "g", false, 2);
        assert_eq!(found.len(), 2);
        assert_eq!(attrs(&found), ["gimp", "google-chrome"]);
        assert!(search(&index, "g", false, 0).is_empty());
        assert!(search(&index, "   ", false, 5).is_empty());
    }

    #[test]
    fn a_foreign_header_is_refused_not_read() {
        let index = scratch("foreign", &INDEX.replacen("\t1\t9", "\t2\t9", 1));
        assert!(search(&index, "hello", false, 10).is_empty());
        assert_eq!(row(&index, "hello"), None);
        assert_eq!(check(&index), Err(Refusal::SourceAbsent));
        assert_eq!(check(&scratch("ours", INDEX)), Ok(9));
        assert_eq!(
            check(Path::new("/nonexistent/index.tsv")),
            Err(Refusal::SourceAbsent)
        );
    }

    #[test]
    fn every_row_is_found_by_its_attribute_and_nothing_else_is() {
        let index = scratch("rows", INDEX);
        for line in INDEX.lines().skip(1) {
            let attr = line.split('\t').next().unwrap();
            let r = row(&index, attr).unwrap_or_else(|| panic!("{attr} was not found"));
            assert_eq!(r.attr, attr);
        }
        for missing in [
            "",
            "a",
            "zzz",
            "hell",
            "hello2",
            "CuboCore",
            "python3Packages",
        ] {
            assert_eq!(row(&index, missing), None, "{missing}");
        }
        let chrome = row(&index, "google-chrome").unwrap();
        assert!(chrome.unfree && chrome.refusal().is_none());
        assert_eq!(chrome.program, "google-chrome-stable");
        assert_eq!(
            row(&index, "alexandria").unwrap().refusal(),
            Some(Refusal::Broken)
        );
        assert_eq!(
            row(&index, "aerospace").unwrap().refusal(),
            Some(Refusal::Unavailable)
        );
    }

    #[test]
    fn the_binary_search_crosses_page_boundaries() {
        // Long descriptions push lines across the 4 KiB pages the search reads.
        let mut text = String::from("#ronoctua-software-nix-index\t1\t300\n");
        for i in 0..300 {
            let pad = "x".repeat((i * 37) % 5000);
            text.push_str(&format!("pkg{i:04}\tpkg\t1\t\t\t\t\t{pad}\n"));
        }
        let index = scratch("pages", &text);
        for i in (0..300).step_by(7) {
            assert_eq!(
                row(&index, &format!("pkg{i:04}")).map(|r| r.attr),
                Some(format!("pkg{i:04}"))
            );
        }
        assert_eq!(row(&index, "pkg9999"), None);
        assert_eq!(row(&index, "pkg"), None);
    }

    /// The real index, when this machine has one: `RONOCTUA_NIX_INDEX=<path>
    /// cargo test -p ronoctua-software --release -- --ignored --nocapture
    /// the_real_index`. It prints the time of a search and of a lookup.
    #[test]
    #[ignore = "needs the built index; run by hand with RONOCTUA_NIX_INDEX"]
    fn the_real_index_answers_quickly() {
        let Some(index) = std::env::var_os("RONOCTUA_NIX_INDEX").map(std::path::PathBuf::from)
        else {
            return;
        };
        let count = check(&index).expect("the index reads");
        assert!(count > 100_000, "{count}");
        for (q, first) in [("rg", "ripgrep"), ("hello", "hello"), ("gimp", "gimp")] {
            let t = std::time::Instant::now();
            let found = search(&index, q, false, 50);
            let took = t.elapsed();
            println!(
                "search {q:?}: {} rows in {took:?}: {:?}",
                found.len(),
                attrs(&found[..found.len().min(6)])
            );
            assert_eq!(found[0].key.id(), first);
        }
        let t = std::time::Instant::now();
        let found = search(&index, "e", true, 50);
        println!(
            "search \"e\" with libraries: {} rows in {:?}",
            found.len(),
            t.elapsed()
        );
        let t = std::time::Instant::now();
        let found = search(&index, "image editor", false, 50);
        println!(
            "search \"image editor\": {} rows in {:?}: {:?}",
            found.len(),
            t.elapsed(),
            attrs(&found[..found.len().min(6)])
        );
        for q in ["firefox", "pdf viewer", "python"] {
            let found = search(&index, q, false, 50);
            println!("search {q:?}: {:?}", attrs(&found[..found.len().min(6)]));
        }
        let t = std::time::Instant::now();
        let chrome = row(&index, "google-chrome").expect("chrome is in nixpkgs");
        println!("row google-chrome in {:?}", t.elapsed());
        assert!(chrome.unfree);
    }
}
