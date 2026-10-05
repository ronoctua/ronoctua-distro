//! The offline English dictionary: the Open English WordNet, turned by
//! `nix/wordsgen.nix` into two sorted TSV files, and read by binary search —
//! a few pages per word, NOTHING RESIDENT (the footer lives for months, and
//! the two files are seventeen megabytes).
//!
//! # The two files
//!
//! ```text
//! lemmas.tsv    #ronoctua-words-lemmas\t1\t<count>
//!               <word, lowercased>\t<synset>\x1f<synset>…
//! synsets.tsv   #ronoctua-words-synsets\t1\t<count>
//!               <synset>\t<pos>\t<gloss>\t<example>\x1f…\t<member>\x1f…
//! ```
//!
//! Both sorted by their first field, byte by byte, after the header line. A
//! synset is WordNet's id without its `oewn-` prefix (`13882990-n`); the part
//! of speech is WordNet's letter (`n`, `v`, `a`, `s`, `r`). A field never
//! holds a tab or a line break: the generator refuses one.
//!
//! # Where it is
//!
//! The directory `RONOCTUA_WORDS` names, set by `nix/modules/words.nix` on a
//! machine whose profile carries it (`full`). Absent, English is asked online
//! like every other tongue — with consent — and without consent it is
//! [`crate::Why::NoDictionary`], said.

use std::fs::File;
use std::os::unix::fs::FileExt as _;
use std::path::Path;

use crate::{Part, Sense, Source};

/// The lemmas' file, in the directory.
pub const LEMMAS: &str = "lemmas.tsv";
/// The synsets' file, in the directory.
pub const SYNSETS: &str = "synsets.tsv";
/// The first field of the lemmas' header.
pub const LEMMAS_HEADER: &str = "#ronoctua-words-lemmas";
/// The first field of the synsets' header.
pub const SYNSETS_HEADER: &str = "#ronoctua-words-synsets";
/// The format both headers carry, bumped with the generator.
pub const FORMAT: &str = "1";
/// What separates the items of a list field.
pub const ITEMS: char = '\u{1f}';

/// Why the offline dictionary answered nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// The files are not there, cannot be read, or have a header this reader
    /// does not know: there is no offline dictionary.
    NoData,
    /// The dictionary is there and the word is not in it.
    NoWord,
}

/// A word as the lemmas are keyed: trimmed, lowercased, `_` as a space, runs
/// of white space as one. Pure.
#[must_use]
pub fn key_of(word: &str) -> String {
    word.replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Whether the directory holds a dictionary this reader knows.
#[must_use]
pub fn is_there(dir: &Path) -> bool {
    [(LEMMAS, LEMMAS_HEADER), (SYNSETS, SYNSETS_HEADER)]
        .iter()
        .all(|(file, header)| opened(&dir.join(file), header).is_some())
}

/// Every sense of `word`, in WordNet's order (the commonest first).
///
/// # Errors
///
/// [`Missing::NoData`] or [`Missing::NoWord`].
pub fn senses(dir: &Path, word: &str) -> Result<Vec<Sense>, Missing> {
    let key = key_of(word);
    if key.is_empty() || key.contains(['\t', '\n']) {
        return Err(Missing::NoWord);
    }
    let (lemmas, len) = opened(&dir.join(LEMMAS), LEMMAS_HEADER).ok_or(Missing::NoData)?;
    let line = row(&lemmas, len, &key)
        .map_err(|()| Missing::NoData)?
        .ok_or(Missing::NoWord)?;
    let ids = line.split('\t').nth(1).unwrap_or_default();
    let (synsets, len) = opened(&dir.join(SYNSETS), SYNSETS_HEADER).ok_or(Missing::NoData)?;
    let mut out = Vec::new();
    for id in ids.split(ITEMS).filter(|s| !s.is_empty()) {
        // A lemma naming a synset the file does not hold is a file this
        // reader cannot trust: the generator refuses one.
        let line = row(&synsets, len, id)
            .map_err(|()| Missing::NoData)?
            .ok_or(Missing::NoData)?;
        out.push(sense_of(&line).ok_or(Missing::NoData)?);
    }
    if out.is_empty() {
        return Err(Missing::NoWord);
    }
    Ok(out)
}

/// The words of the same senses as `word`, each once, the word itself left
/// out, in WordNet's order.
///
/// # Errors
///
/// As [`senses`]; [`Missing::NoWord`] too when the word has no other.
pub fn synonyms(dir: &Path, word: &str) -> Result<Vec<String>, Missing> {
    let key = key_of(word);
    let mut out: Vec<String> = Vec::new();
    for sense in senses(dir, word)? {
        for member in sense.synonyms {
            if key_of(&member) != key && !out.contains(&member) {
                out.push(member);
            }
        }
    }
    if out.is_empty() {
        return Err(Missing::NoWord);
    }
    Ok(out)
}

/// A synset's line as a sense. Pure.
fn sense_of(line: &str) -> Option<Sense> {
    let mut f = line.split('\t');
    let _id = f.next()?;
    let pos = f.next()?;
    let gloss = f.next()?.to_owned();
    let list = |s: Option<&str>| -> Vec<String> {
        s.unwrap_or_default()
            .split(ITEMS)
            .filter(|x| !x.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let examples = list(f.next());
    let synonyms = list(f.next());
    Some(Sense {
        part: Some(part_of(pos)),
        gloss,
        examples,
        synonyms,
        source: Source::WordNet,
    })
}

/// WordNet's letter for a part of speech. Pure.
#[must_use]
pub fn part_of(pos: &str) -> Part {
    match pos {
        "n" => Part::Noun,
        "v" => Part::Verb,
        "a" | "s" => Part::Adjective,
        "r" => Part::Adverb,
        "c" => Part::Conjunction,
        "p" => Part::Preposition,
        _ => Part::Other,
    }
}

/// The file and its length, when its header is `header` in this format.
fn opened(path: &Path, header: &str) -> Option<(File, u64)> {
    let file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let first = line_at(&file, 0, len).ok()??;
    let mut f = first.split('\t');
    (f.next() == Some(header) && f.next() == Some(FORMAT)).then_some((file, len))
}

/// The line whose first field is `key` (binary search: the file is sorted
/// after its header), reading a few pages and never the whole file.
fn row(file: &File, len: u64, key: &str) -> Result<Option<String>, ()> {
    let first = line_at(file, 0, len).map_err(|_| ())?.ok_or(())?;
    let mut lo = first.len() as u64 + 1;
    let mut hi = len;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let start = line_start_from(file, mid, len).map_err(|_| ())?;
        if start >= hi {
            hi = mid;
            continue;
        }
        let line = line_at(file, start, len).map_err(|_| ())?.ok_or(())?;
        let here = line.split('\t').next().unwrap_or_default();
        match here.as_bytes().cmp(key.as_bytes()) {
            std::cmp::Ordering::Equal => return Ok(Some(line)),
            std::cmp::Ordering::Less => lo = start + line.len() as u64 + 1,
            std::cmp::Ordering::Greater => hi = start,
        }
    }
    Ok(None)
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
pub(crate) mod fixture {
    //! A dictionary of a few words, written the generator's way, and one
    //! large enough that the search crosses pages.

    use std::path::PathBuf;

    use super::{FORMAT, ITEMS, LEMMAS, LEMMAS_HEADER, SYNSETS, SYNSETS_HEADER};

    /// Writes a dictionary into a fresh directory named by `tag`, with
    /// `filler` extra lemmas and synsets around the real ones.
    pub(crate) fn dictionary(tag: &str, filler: usize) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-words-wordnet-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = ITEMS;
        let mut lemmas = vec![
            format!("house\t03549540-n{s}08028148-n{s}02733213-v"),
            format!("home\t03264542-n"),
            format!("put up\t02733213-v"),
            format!("abode\t03264542-n"),
            format!("family\t08028148-n"),
        ];
        let mut synsets = vec![
            format!(
                "03549540-n\tn\ta dwelling that serves as living quarters for one or more families\the has a house on Cape Cod{s}she felt she had to get out of the house\thouse"
            ),
            format!(
                "08028148-n\tn\ta social unit living together\the moved his family to Virginia\tfamily{s}household{s}house{s}home"
            ),
            format!("02733213-v\tv\tcontain or cover\t\thouse{s}put up"),
            format!("03264542-n\tn\twhere you live at a particular time\t\thome{s}abode"),
        ];
        for k in 0..filler {
            lemmas.push(format!("zz{k:07}\t9{k:07}-n"));
            synsets.push(format!("9{k:07}-n\tn\tfiller number {k}\t\tzz{k:07}"));
        }
        lemmas.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        synsets.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        let write = |file: &str, header: &str, rows: &[String]| {
            let mut text = format!("{header}\t{FORMAT}\t{}\n", rows.len());
            for r in rows {
                text.push_str(r);
                text.push('\n');
            }
            std::fs::write(dir.join(file), text).unwrap();
        };
        write(LEMMAS, LEMMAS_HEADER, &lemmas);
        write(SYNSETS, SYNSETS_HEADER, &synsets);
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_is_its_senses_in_wordnets_order() {
        let dir = fixture::dictionary("senses", 0);
        assert!(is_there(&dir));
        let senses = senses(&dir, "  House ").unwrap();
        assert_eq!(senses.len(), 3);
        assert_eq!(senses[0].part, Some(Part::Noun));
        assert!(senses[0].gloss.starts_with("a dwelling"));
        assert_eq!(senses[0].examples.len(), 2);
        assert_eq!(senses[2].part, Some(Part::Verb));
        assert_eq!(senses[2].source, Source::WordNet);
        assert_eq!(super::senses(&dir, "put_up").unwrap().len(), 1);
        assert_eq!(super::senses(&dir, "housing"), Err(Missing::NoWord));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn synonyms_are_the_other_members_each_once() {
        let dir = fixture::dictionary("synonyms", 0);
        assert_eq!(
            synonyms(&dir, "house").unwrap(),
            ["family", "household", "home", "put up"]
        );
        assert_eq!(synonyms(&dir, "abode").unwrap(), ["home"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the binary search stops finding rows once the file is larger
    /// than a page — the real dictionary is thousands of pages.
    #[test]
    fn the_search_crosses_pages_and_finds_every_row() {
        let dir = fixture::dictionary("pages", 3000);
        for k in [0, 1, 777, 1500, 2998, 2999] {
            let word = format!("zz{k:07}");
            let s = senses(&dir, &word).unwrap();
            assert_eq!(s[0].gloss, format!("filler number {k}"), "{word}");
        }
        assert_eq!(senses(&dir, "house").unwrap().len(), 3);
        assert_eq!(senses(&dir, "zz9999999"), Err(Missing::NoWord));
        assert_eq!(senses(&dir, "aaa"), Err(Missing::NoWord));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_files_or_a_foreign_header_is_no_dictionary() {
        let dir = std::env::temp_dir().join(format!("ronoctua-words-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!is_there(&dir));
        assert_eq!(senses(&dir, "house"), Err(Missing::NoData));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(LEMMAS), "#something-else\t1\t0\nhouse\tx\n").unwrap();
        std::fs::write(dir.join(SYNSETS), "#ronoctua-words-synsets\t1\t0\n").unwrap();
        assert!(!is_there(&dir));
        assert_eq!(senses(&dir, "house"), Err(Missing::NoData));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_key_is_the_generators_key() {
        assert_eq!(key_of("  Put_Up "), "put up");
        assert_eq!(key_of("Cartesian   plane"), "cartesian plane");
        assert_eq!(part_of("s"), Part::Adjective);
    }

    /// On the real dictionary, when `RONOCTUA_WORDS` names it (the Nix tests
    /// set it; without it this proves nothing and says so by returning).
    #[test]
    fn the_real_dictionary_answers_in_a_few_reads() {
        let Some(dir) = std::env::var_os(crate::OFFLINE_VARIABLE).map(std::path::PathBuf::from)
        else {
            return;
        };
        assert!(is_there(&dir), "{} holds no dictionary", dir.display());
        let t0 = std::time::Instant::now();
        let house = senses(&dir, "house").unwrap();
        let took = t0.elapsed();
        assert_eq!(house[0].part, Some(Part::Noun));
        assert!(house[0].gloss.contains("dwelling"), "{:?}", house[0].gloss);
        assert!(house.len() > 5);
        assert!(
            took < std::time::Duration::from_secs(1),
            "a binary search, not a scan: {took:?}"
        );
        assert!(synonyms(&dir, "abode").unwrap().iter().any(|s| s == "home"));
        assert_eq!(
            senses(&dir, "Cartesian_plane").unwrap()[0].part,
            Some(Part::Noun)
        );
        assert_eq!(senses(&dir, "qwxzv"), Err(Missing::NoWord));
    }
}
