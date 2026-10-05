//! Reading what git prints — pure, so every shape is tested without it.
//!
//! Every format read here is one git promises to keep for machines:
//! `status --porcelain=v2 -z`, `--format` with `%x00` between fields and
//! `-z` between records, `diff-tree -z --name-status`, `blame --porcelain`,
//! `for-each-ref --format` with `%00`, `config --null`. A path is BYTES
//! (`OsStr::from_bytes`): a name that is not UTF-8 still names its file.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;

use crate::{
    BlameLine, Change, ChangedFile, Commit, Entry, EntryState, Oid, Ref, RefKind, Remote, Stash,
    Status,
};

fn path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(OsStr::from_bytes(bytes))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Whether `text` is a commit id git would print: 40 (SHA-1) or 64
/// (SHA-256) hexadecimal digits — or an abbreviation of at least 4 when
/// `short` is allowed. Pure.
#[must_use]
pub(crate) fn is_oid(text: &str, short: bool) -> bool {
    let hex = text
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    let len = text.len();
    hex && (len == 40 || len == 64 || (short && (4..=64).contains(&len)))
}

fn oid(bytes: &[u8]) -> Option<Oid> {
    let t = std::str::from_utf8(bytes).ok()?;
    is_oid(t, false).then(|| Oid(t.to_owned()))
}

/// The fields a NUL-separated answer is made of, the last terminator
/// dropped.
fn fields(bytes: &[u8]) -> Vec<&[u8]> {
    let mut all: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
    if all.last().is_some_and(|l| l.is_empty()) {
        all.pop();
    }
    all
}

/// One side of a porcelain `XY`.
fn change_of(c: u8, from: Option<&PathBuf>) -> Option<Change> {
    Some(match c {
        b'M' => Change::Modified,
        b'A' => Change::Added,
        b'D' => Change::Deleted,
        b'T' => Change::TypeChanged,
        b'U' => Change::Unmerged,
        b'R' => Change::Renamed {
            from: from.cloned().unwrap_or_default(),
        },
        b'C' => Change::Copied {
            from: from.cloned().unwrap_or_default(),
        },
        _ => return None,
    })
}

/// `status --porcelain=v2 --branch -z`. `None` for anything it does not
/// know how to read. Pure.
#[must_use]
pub(crate) fn status(bytes: &[u8]) -> Option<Status> {
    let mut out = Status::default();
    let all = fields(bytes);
    let mut i = 0;
    while i < all.len() {
        let field = all[i];
        i += 1;
        if let Some(header) = field.strip_prefix(b"# ") {
            let (key, value) = header.split_at(header.iter().position(|&b| b == b' ')?);
            let value = &value[1..];
            match key {
                b"branch.oid" => out.head = oid(value),
                b"branch.head" => {
                    out.branch = (value != b"(detached)").then(|| text(value));
                }
                b"branch.upstream" => out.upstream = Some(text(value)),
                b"branch.ab" => {
                    let t = std::str::from_utf8(value).ok()?;
                    let mut parts = t.split(' ');
                    out.ahead = parts.next()?.strip_prefix('+')?.parse().ok()?;
                    out.behind = parts.next()?.strip_prefix('-')?.parse().ok()?;
                }
                _ => {}
            }
            continue;
        }
        let kind = *field.first()?;
        match kind {
            // `1 XY sub mH mI mW hH hI path`
            b'1' | b'2' => {
                let wanted = if kind == b'1' { 9 } else { 10 };
                let parts: Vec<&[u8]> = field.splitn(wanted, |&b| b == b' ').collect();
                if parts.len() != wanted {
                    return None;
                }
                let xy = parts[1];
                if xy.len() != 2 {
                    return None;
                }
                let at = path(parts[wanted - 1]);
                // `2 …` is a rename or a copy: its old path is the NEXT field.
                let from = if kind == b'2' {
                    let f = path(all.get(i)?);
                    i += 1;
                    Some(f)
                } else {
                    None
                };
                let staged = change_of(xy[0], from.as_ref());
                let unstaged = change_of(xy[1], from.as_ref());
                if staged.is_some() {
                    out.staged += 1;
                }
                if unstaged.is_some() {
                    out.changed += 1;
                }
                out.entries.push(Entry {
                    path: at,
                    state: EntryState::Tracked { staged, unstaged },
                });
            }
            // `u XY sub m1 m2 m3 mW h1 h2 h3 path`
            b'u' => {
                let parts: Vec<&[u8]> = field.splitn(11, |&b| b == b' ').collect();
                if parts.len() != 11 {
                    return None;
                }
                out.conflicted += 1;
                out.entries.push(Entry {
                    path: path(parts[10]),
                    state: EntryState::Conflicted,
                });
            }
            b'?' => {
                out.untracked += 1;
                out.entries.push(Entry {
                    path: path(field.get(2..)?),
                    state: EntryState::Untracked,
                });
            }
            b'!' => {}
            _ => return None,
        }
    }
    Some(out)
}

/// The format every history is asked in: id, parents, author, time,
/// subject — `%x00` between them, and `-z` between commits.
pub(crate) const COMMIT_FORMAT: &str = "--format=%H%x00%P%x00%an%x00%at%x00%s";

/// A history in [`COMMIT_FORMAT`]. `None` when it is not one. Pure.
#[must_use]
pub(crate) fn commits(bytes: &[u8]) -> Option<Vec<Commit>> {
    let all = fields(bytes);
    if !all.len().is_multiple_of(5) {
        return None;
    }
    all.chunks(5).map(commit_of).collect()
}

/// One record of [`COMMIT_FORMAT`], its five fields.
fn commit_of(c: &[&[u8]]) -> Option<Commit> {
    // `--follow` and a merge print the record after a line end.
    let id = c.first()?;
    let id = id.strip_prefix(b"\n").unwrap_or(id);
    Some(Commit {
        id: oid(id)?,
        parents: c
            .get(1)?
            .split(|&b| b == b' ')
            .filter(|p| !p.is_empty())
            .map(oid)
            .collect::<Option<Vec<_>>>()?,
        author: text(c.get(2)?),
        at_s: std::str::from_utf8(c.get(3)?).ok()?.parse().ok()?,
        subject: text(c.get(4)?),
    })
}

/// One path's history in [`COMMIT_FORMAT`] with `--name-status`: each
/// commit, and the name the path had in it — the NEW name of a rename or a
/// copy — or `None` where git printed no change (a merge). `None` when it is
/// not that. Pure.
///
/// UNAMBIGUOUS BY THE FIRST LETTER: a status (`M`, `A`, `D`, `T`, `U`,
/// `R100`, `C075`) begins with an upper-case letter, and a commit id — the
/// token that begins the next record — never does.
#[must_use]
pub(crate) fn named_commits(bytes: &[u8]) -> Option<Vec<(Commit, Option<PathBuf>)>> {
    let all = fields(bytes);
    let mut out = Vec::new();
    let mut i = 0;
    while i < all.len() {
        let commit = commit_of(all.get(i..i + 5)?)?;
        i += 5;
        let mut name = None;
        while let Some(token) = all.get(i) {
            let status = token.strip_prefix(b"\n").unwrap_or(token);
            let Some(&letter) = status.first() else {
                break;
            };
            if !letter.is_ascii_uppercase() {
                break;
            }
            let paths = if matches!(letter, b'R' | b'C') { 2 } else { 1 };
            let named = all.get(i + 1..i + 1 + paths)?;
            if name.is_none() {
                name = named.last().map(|p| path(p));
            }
            i += 1 + paths;
        }
        out.push((commit, name));
    }
    Some(out)
}

/// `diff-tree -r -z --name-status -M`. `None` when it is not that. Pure.
#[must_use]
pub(crate) fn name_status(bytes: &[u8]) -> Option<Vec<ChangedFile>> {
    let all = fields(bytes);
    let mut out = Vec::new();
    let mut i = 0;
    while i < all.len() {
        let status = all[i];
        i += 1;
        let mut next = || {
            let p = all.get(i).map(|p| path(p));
            i += 1;
            p
        };
        let file = match status.first()? {
            b'A' => ChangedFile {
                path: next()?,
                change: Change::Added,
            },
            b'M' => ChangedFile {
                path: next()?,
                change: Change::Modified,
            },
            b'D' => ChangedFile {
                path: next()?,
                change: Change::Deleted,
            },
            b'T' => ChangedFile {
                path: next()?,
                change: Change::TypeChanged,
            },
            b'U' => ChangedFile {
                path: next()?,
                change: Change::Unmerged,
            },
            b'R' => {
                let from = next()?;
                ChangedFile {
                    path: next()?,
                    change: Change::Renamed { from },
                }
            }
            b'C' => {
                let from = next()?;
                ChangedFile {
                    path: next()?,
                    change: Change::Copied { from },
                }
            }
            _ => return None,
        };
        out.push(file);
    }
    Some(out)
}

/// `blame --porcelain`. `None` when it is not that. Pure.
#[must_use]
pub(crate) fn blame(bytes: &[u8]) -> Option<Vec<BlameLine>> {
    use std::collections::HashMap;
    let mut who: HashMap<String, (String, i64)> = HashMap::new();
    let mut out = Vec::new();
    let mut current: Option<(String, u32)> = None;
    for line in bytes.split(|&b| b == b'\n') {
        if let Some(content) = line.strip_prefix(b"\t") {
            let (id, number) = current.take()?;
            let (author, at_s) = who.get(&id).cloned().unwrap_or_default();
            out.push(BlameLine {
                commit: Oid(id),
                author,
                at_s,
                number,
                text: text(content),
            });
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let t = String::from_utf8_lossy(line);
        let mut parts = t.split(' ');
        let first = parts.next()?;
        if current.is_none() && is_oid(first, false) {
            let _original: u32 = parts.next()?.parse().ok()?;
            let number: u32 = parts.next()?.parse().ok()?;
            current = Some((first.to_owned(), number));
            who.entry(first.to_owned()).or_default();
            continue;
        }
        let (id, _) = current.as_ref()?;
        let entry = who.entry(id.clone()).or_default();
        if let Some(name) = t.strip_prefix("author ") {
            entry.0 = name.to_owned();
        } else if let Some(time) = t.strip_prefix("author-time ") {
            entry.1 = time.trim().parse().ok()?;
        }
    }
    current.is_none().then_some(out)
}

/// The format [`refs`] reads, `%00` between fields and a line per ref.
pub(crate) const REF_FORMAT: &str = "--format=%(refname)%00%(objectname)%00%(*objectname)%00%(upstream:short)%00%(contents:subject)%00%(creatordate:unix)";

/// `for-each-ref` in [`REF_FORMAT`]. `None` when it is not that. Pure.
#[must_use]
pub(crate) fn refs(bytes: &[u8]) -> Option<Vec<Ref>> {
    let mut out = Vec::new();
    for line in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let f: Vec<&[u8]> = line.split(|&b| b == 0).collect();
        if f.len() != 6 {
            return None;
        }
        let name = text(f[0]);
        let (kind, short) = if let Some(s) = name.strip_prefix("refs/heads/") {
            (RefKind::Branch, s)
        } else if let Some(s) = name.strip_prefix("refs/remotes/") {
            // `origin/HEAD` is a pointer at a branch already listed.
            if s.ends_with("/HEAD") {
                continue;
            }
            (RefKind::Remote, s)
        } else if let Some(s) = name.strip_prefix("refs/tags/") {
            (RefKind::Tag, s)
        } else {
            continue;
        };
        // An annotated tag points at the tag object; `*objectname` is the
        // commit behind it.
        let target = oid(f[2]).or_else(|| oid(f[1]))?;
        out.push(Ref {
            name: short.to_owned(),
            kind,
            target,
            upstream: (!f[3].is_empty()).then(|| text(f[3])),
            subject: text(f[4]),
            at_s: std::str::from_utf8(f[5]).ok()?.trim().parse().unwrap_or(0),
        });
    }
    Some(out)
}

/// The stash's reflog in `--format=%H%x00%gs` with `-z`. Pure.
#[must_use]
pub(crate) fn stashes(bytes: &[u8]) -> Option<Vec<Stash>> {
    let all = fields(bytes);
    if !all.len().is_multiple_of(2) {
        return None;
    }
    all.chunks(2)
        .enumerate()
        .map(|(n, c)| {
            let id = c[0].strip_prefix(b"\n").unwrap_or(c[0]);
            Some(Stash {
                index: u32::try_from(n).ok()?,
                id: oid(id)?,
                subject: text(c[1]),
            })
        })
        .collect()
}

/// `config --null --get-regexp ^remote\..*\.(url|pushurl)$`: `key\nvalue`
/// records. Pure.
#[must_use]
pub(crate) fn remotes(bytes: &[u8]) -> Vec<Remote> {
    let mut out: Vec<Remote> = Vec::new();
    for record in fields(bytes) {
        let Some(split) = record.iter().position(|&b| b == b'\n') else {
            continue;
        };
        let (key, value) = (text(&record[..split]), text(&record[split + 1..]));
        let Some(rest) = key.strip_prefix("remote.") else {
            continue;
        };
        let (name, push) = if let Some(n) = rest.strip_suffix(".pushurl") {
            (n, true)
        } else if let Some(n) = rest.strip_suffix(".url") {
            (n, false)
        } else {
            continue;
        };
        let at = match out.iter().position(|r| r.name == name) {
            Some(at) => at,
            None => {
                out.push(Remote {
                    name: name.to_owned(),
                    fetch_url: String::new(),
                    push_url: None,
                });
                out.len() - 1
            }
        };
        let remote = &mut out[at];
        if push {
            remote.push_url.get_or_insert(value);
        } else if remote.fetch_url.is_empty() {
            remote.fetch_url = value;
        }
    }
    out.retain(|r| !r.fetch_url.is_empty());
    out
}

/// The filter drivers a repository's configuration names, from `config
/// --null --name-only --get-regexp ^filter\.`: `filter.<name>.<key>` keys,
/// the name being everything between the first dot and the last. AS BYTES,
/// and nothing dropped: a name need not be UTF-8, may hold `=` or a space,
/// and may be EMPTY (`[filter ""]` runs for `filter=`, MEASURED) — each one
/// a driver git runs, so each one a driver to empty. `filter.<key>`, with no
/// name, is no driver. Anything else is not what git prints, and refuses
/// the whole answer (`None`). Pure.
#[must_use]
pub(crate) fn filter_drivers(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut names = Vec::new();
    for key in fields(bytes) {
        let rest = key.strip_prefix(b"filter.")?;
        let Some(last) = rest.iter().rposition(|&b| b == b'.') else {
            continue;
        };
        if rest[last + 1..].is_empty() {
            return None;
        }
        names.push(rest[..last].to_vec());
    }
    names.sort();
    names.dedup();
    Some(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn a_followed_history_names_the_path_as_each_commit_knew_it() {
        // AS GIT 2.54 PRINTS IT: the record, a line end, the status and the
        // path(s) — and the next record straight after, its id unprefixed.
        let bytes = format!(
            "{A}\0{B}\0Ada\0170\0more\0\nM\0b.txt\0\
             {B}\0\0Ada\0160\0renamed\0\nR100\0a.txt\0b.txt\0\
             \n{A}\0\0Ada\0150\0a merge prints no change\0\
             {B}\0\0Ada\0140\0first\0\nA\0a.txt\0"
        );
        let named = named_commits(bytes.as_bytes()).unwrap();
        let names: Vec<Option<&str>> = named
            .iter()
            .map(|(_, p)| p.as_deref().and_then(|p| p.to_str()))
            .collect();
        assert_eq!(
            names,
            [Some("b.txt"), Some("b.txt"), None, Some("a.txt")],
            "a rename is known by its new name, and what came before by the old"
        );
        assert_eq!(named[1].0.subject, "renamed");
        assert_eq!(named[3].0.at_s, 140);
        // A PATH THAT LOOKS LIKE AN ID is still a path: it follows a status.
        let bytes = format!("{A}\0\0Ada\01\0x\0\nM\0{B}\0{B}\0\0Ada\00\0y\0");
        let named = named_commits(bytes.as_bytes()).unwrap();
        assert_eq!(named.len(), 2);
        assert_eq!(named[0].1, Some(PathBuf::from(B)));
        // AND WHAT IS NOT A RECORD is refused, never guessed at.
        assert!(named_commits(b"not a history\0").is_none());
    }

    #[test]
    fn a_porcelain_status_is_counted_and_listed() {
        let bytes = format!(
            "# branch.oid {A}\0# branch.head main\0# branch.upstream origin/main\0\
             # branch.ab +2 -3\0\
             1 M. N... 100644 100644 100644 {A} {B} staged.txt\0\
             1 .M N... 100644 100644 100644 {A} {A} with space.txt\0\
             2 R. N... 100644 100644 100644 {A} {A} R100 new\nname\0old name\0\
             u UU N... 100644 100644 100644 100644 {A} {B} {A} fight.txt\0\
             ? new file\0! ignored\0"
        );
        let s = status(bytes.as_bytes()).unwrap();
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.head, Some(Oid(A.to_owned())));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (2, 3));
        assert_eq!(
            (s.staged, s.changed, s.untracked, s.conflicted),
            (2, 1, 1, 1)
        );
        assert_eq!(s.entries.len(), 5);
        assert_eq!(s.entries[1].path, PathBuf::from("with space.txt"));
        assert_eq!(
            s.entries[2],
            Entry {
                path: PathBuf::from("new\nname"),
                state: EntryState::Tracked {
                    staged: Some(Change::Renamed {
                        from: PathBuf::from("old name")
                    }),
                    unstaged: None
                }
            },
            "the old path is the field after the new one"
        );
        assert_eq!(s.entries[4].state, EntryState::Untracked);

        let detached = format!("# branch.oid {A}\0# branch.head (detached)\0");
        let s = status(detached.as_bytes()).unwrap();
        assert_eq!(s.branch, None);
        let unborn = "# branch.oid (initial)\0# branch.head main\0";
        let s = status(unborn.as_bytes()).unwrap();
        assert_eq!((s.head, s.branch.as_deref()), (None, Some("main")));
        assert_eq!(status(b"what is this\0"), None);
    }

    #[test]
    fn a_history_is_five_fields_a_commit() {
        let bytes =
            format!("{A}\0{B}\0Ada\x001700000000\0first line\0{B}\0\0Bo\x001600000000\0root\0");
        let c = commits(bytes.as_bytes()).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].parents, [Oid(B.to_owned())]);
        assert_eq!(c[0].at_s, 1_700_000_000);
        assert!(c[1].parents.is_empty());
        assert_eq!(commits(b"").unwrap(), []);
        assert_eq!(commits(b"a\0b\0"), None);
    }

    #[test]
    fn name_status_reads_renames_as_two_paths() {
        let bytes = b"M\0a.txt\0R087\0old\0new\nline\0A\0added\0D\0gone\0";
        let files = name_status(bytes).unwrap();
        assert_eq!(files.len(), 4);
        assert_eq!(
            files[1],
            ChangedFile {
                path: PathBuf::from("new\nline"),
                change: Change::Renamed {
                    from: PathBuf::from("old")
                }
            }
        );
        assert_eq!(files[3].change, Change::Deleted);
        assert_eq!(name_status(b"R100\0only-one\0"), None);
    }

    #[test]
    fn a_blame_names_each_line() {
        let bytes = format!(
            "{A} 1 1 2\nauthor Ada\nauthor-time 1700000000\nsummary x\nfilename f\n\tfirst\n\
             {A} 2 2\n\tsecond\n{B} 1 3 1\nauthor Bo\nauthor-time 1600000000\nfilename f\n\tthird\n"
        );
        let lines = blame(bytes.as_bytes()).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].author, "Ada", "a repeated commit keeps its author");
        assert_eq!(lines[1].number, 2);
        assert_eq!(lines[2].text, "third");
        assert_eq!(lines[2].at_s, 1_600_000_000);
    }

    #[test]
    fn refs_are_kinds_and_a_tag_is_its_commit() {
        let bytes = format!(
            "refs/heads/main\0{A}\0\0origin/main\0subject\x001700000000\n\
             refs/remotes/origin/HEAD\0{A}\0\0\0subject\x001700000000\n\
             refs/remotes/origin/main\0{A}\0\0\0subject\x001700000000\n\
             refs/tags/v1\0{B}\0{A}\0\0tagged\x001600000000\n\
             refs/notes/x\0{A}\0\0\0n\x000\n"
        );
        let r = refs(bytes.as_bytes()).unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].upstream.as_deref(), Some("origin/main"));
        assert_eq!(r[1].kind, RefKind::Remote);
        assert_eq!(r[2].target, Oid(A.to_owned()), "peeled to the commit");
    }

    #[test]
    fn remotes_and_stashes_and_filters() {
        let r = remotes(
            b"remote.origin.url\nhttps://example.org/a.git\0remote.my.fork.url\ngit@h:x\0\
              remote.my.fork.pushurl\nssh://h/y\0",
        );
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].name, "my.fork", "a name with dots");
        assert_eq!(r[1].push_url.as_deref(), Some("ssh://h/y"));
        let s = stashes(format!("{A}\0On main: wip\0{B}\0second\0").as_bytes()).unwrap();
        assert_eq!(s[1].index, 1);
        assert_eq!(
            filter_drivers(b"filter.lfs.clean\0filter.lfs.smudge\0filter.a.b.process\0").unwrap(),
            [b"a.b".to_vec(), b"lfs".to_vec()]
        );
        // What git's own `-c` would split or a `from_utf8` would drop.
        assert_eq!(
            filter_drivers(b"filter.x=y.clean\0filter.\xff.clean\0filter. sp .smudge\0").unwrap(),
            [b" sp ".to_vec(), b"x=y".to_vec(), b"\xff".to_vec()]
        );
        assert_eq!(filter_drivers(b"").unwrap(), Vec::<Vec<u8>>::new());
        assert_eq!(
            filter_drivers(b"filter..clean\0filter.clean\0").unwrap(),
            [Vec::<u8>::new()],
            "the empty name is a driver; no name at all is none"
        );
        assert_eq!(filter_drivers(b"core.x.y\0"), None);
        assert_eq!(filter_drivers(b"filter.x.\0"), None);
        assert!(is_oid(A, false) && !is_oid("--output=x", true) && is_oid("abcd", true));
        assert!(!is_oid("ABCD", true));
    }
}
