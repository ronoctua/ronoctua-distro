//! The packages one can install (feature `write`): the Nix index of the
//! system's own nixpkgs (`RONOCTUA_NIX_INDEX`), read by
//! `ronoctua_software::nix_index` — the one reader of that file, so a new
//! column is learnt in one place.
//!
//! A package is `pkg:<attribute>`, named by its attribute and found also by
//! its `pname` and its program; its description is both the body and the
//! row's note. What cannot be installed here is left out — broken, not for
//! this platform — and so are libraries (a nested set's `python3Packages.x`):
//! `/p` offers programs.
//!
//! A relist is whole and happens when the file's signature (where it
//! resolves, its size and time) is not the one the last commit kept.

use std::io::BufRead as _;
use std::path::Path;

use ronoctua_software::nix_index::{Row, header_is_ours};

use crate::Kind;
use crate::write::Document;

/// The variable that names the index file.
pub const VARIABLE: &str = "RONOCTUA_NIX_INDEX";

/// The file's signature: where it resolves, its size and its time.
#[must_use]
pub fn signature(path: &Path) -> Option<String> {
    let real = std::fs::canonicalize(path).ok()?;
    let meta = std::fs::metadata(&real).ok()?;
    let mtime = std::os::unix::fs::MetadataExt::mtime(&meta);
    Some(format!("{} {} {mtime}", real.display(), meta.len()))
}

/// A row as a document, or `None` for one that is not offered.
#[must_use]
pub fn document(row: &Row, body_bytes: usize) -> Option<Document> {
    if row.library || row.broken || row.unavailable || row.attr.is_empty() {
        return None;
    }
    let mut also: Vec<String> = Vec::new();
    for word in [&row.pname, &row.program] {
        if !word.is_empty() && *word != row.attr && !also.contains(word) {
            also.push(word.clone());
        }
    }
    let mut body = row.description.clone();
    if body.len() > body_bytes {
        let mut at = body_bytes;
        while at > 0 && !body.is_char_boundary(at) {
            at -= 1;
        }
        body.truncate(at);
    }
    Some(Document {
        id: format!("pkg:{}", row.attr),
        kind: Kind::Package,
        name: row.attr.clone(),
        also,
        path_terms: Vec::new(),
        body: (!body.is_empty()).then_some(body),
        note: (!row.description.is_empty()).then(|| row.description.clone()),
        mime: None,
        mtime_s: 0,
        size_bytes: 0,
    })
}

/// Every offered package of the index at `path`, or `None` when the file is
/// not there or not an index this reader understands (its header).
#[must_use]
pub fn documents(path: &Path, body_bytes: usize) -> Option<Vec<Document>> {
    let file = std::fs::File::open(path).ok()?;
    let mut lines = std::io::BufReader::new(file).lines();
    let header = lines.next()?.ok()?;
    if !header_is_ours(&header) {
        tracing::warn!(path = %path.display(), "the Nix index is not of a format this build reads");
        return None;
    }
    Some(
        lines
            .map_while(Result::ok)
            .filter_map(|l| Row::of_line(&l))
            .filter_map(|r| document(&r, body_bytes))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(attr: &str) -> Row {
        Row {
            attr: attr.to_owned(),
            pname: "ripgrep".to_owned(),
            version: "14.1.1".to_owned(),
            unfree: false,
            broken: false,
            insecure: false,
            unavailable: false,
            library: false,
            program: "rg".to_owned(),
            license: "MIT".to_owned(),
            homepage: "https://example.org".to_owned(),
            description: "Utility that combines the usability of The Silver Searcher with the raw speed of grep".to_owned(),
        }
    }

    #[test]
    fn a_program_is_named_by_its_attribute_and_found_by_its_other_names() {
        let doc = document(&row("ripgrep"), 1024).unwrap();
        assert_eq!(doc.id, "pkg:ripgrep");
        assert_eq!(doc.kind, Kind::Package);
        assert_eq!(doc.name, "ripgrep");
        assert_eq!(doc.also, ["rg"]);
        assert!(doc.note.as_deref().unwrap().starts_with("Utility"));
        assert_eq!(
            document(&row("ripgrep"), 7).unwrap().body.as_deref(),
            Some("Utility")
        );
    }

    #[test]
    fn what_cannot_be_installed_here_and_libraries_are_left_out() {
        let mut broken = row("x");
        broken.broken = true;
        assert!(document(&broken, 64).is_none());
        let mut library = row("python3Packages.requests");
        library.library = true;
        assert!(document(&library, 64).is_none());
        let mut elsewhere = row("y");
        elsewhere.unavailable = true;
        assert!(document(&elsewhere, 64).is_none());
        // Unfree is a question at install time, not a refusal here.
        let mut unfree = row("z");
        unfree.unfree = true;
        assert!(document(&unfree, 64).is_some());
    }

    #[test]
    fn the_file_is_read_through_the_software_centres_reader() {
        let dir = std::env::temp_dir().join(format!("ronoctua-pkgs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.tsv");
        std::fs::write(
            &path,
            format!(
                "{}\t{}\t2\n\
                 ripgrep\tripgrep\t14.1.1\t\trg\tMIT\thttps://example.org\tFast grep\n\
                 python3Packages.foo\tfoo\t1\tl\t\tMIT\t\tA library\n",
                ronoctua_software::nix_index::HEADER,
                ronoctua_software::nix_index::FORMAT,
            ),
        )
        .unwrap();
        let docs = documents(&path, 64).unwrap();
        let ids: Vec<&str> = docs.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["pkg:ripgrep"]);
        assert!(signature(&path).is_some());

        std::fs::write(&path, "#something-else\t9\nripgrep\n").unwrap();
        assert!(documents(&path, 64).is_none());
        assert!(documents(&dir.join("absent.tsv"), 64).is_none());
        assert!(signature(&dir.join("absent.tsv")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
