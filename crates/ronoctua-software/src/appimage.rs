//! AppImages in `~/Applications`, integrated into the launcher with our own
//! `.desktop` (`ronoctua-appimage-<key>.desktop`, `Exec=appimage-run <file>`).
//!
//! # The file is read by Rust, never run
//!
//! An AppImage is an ELF runtime with a filesystem image appended. Running it
//! to ask what it is (`--appimage-extract`, `--appimage-updateinformation`)
//! would run a stranger's program before the person said yes — and on this
//! system binfmt hands it to `appimage-run`, which passes those flags to the
//! program's own `AppRun`. So the ELF header, the section table and the
//! payload's magic are read here ([`header_of`]), and only the three things a
//! launcher needs — the top-level `.desktop`, `.DirIcon` and the icon the
//! `.desktop` names — are pulled out of the image by `unsquashfs`
//! ([`extract_argv`]), into a staging directory that is always removed.
//!
//! # What `unsquashfs` 4.7.5 does with a pattern (measured on the store's
//! binary, 2026-09-26)
//!
//! * Patterns are shell globs matched ONE PATH COMPONENT at a time from the
//!   image's root: `*.desktop` is the top level only, which is the spec's rule.
//! * A symlink is extracted AS a symlink (`.DirIcon -> usr/share/…/x.png`) and
//!   its target is not; `-follow-symlinks` would fetch it but implies
//!   `-no-wildcards`. So a link is resolved HERE, component by component and
//!   never outside the staging root ([`walk`]), and a missing target is asked
//!   for by its literal path in another pass.
//! * A pattern that matches nothing is not an error (exit 0).
//! * `-q` alone still draws the progress bar; `-n` is what silences it.
//!
//! # What `appimage-run` leaves behind
//!
//! It extracts the WHOLE image to `$XDG_CACHE_HOME/appimage-run/<sha256 of
//! the file>` on the first run and never cleans it (`appimage-exec.sh` of the
//! pinned nixpkgs). So removing and upgrading drop that directory, and
//! [`Order::Tidy`] drops the ones no integrated AppImage owns.

use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::found::Picture;
use crate::order::{Consent, Scope};
use crate::progress::{Phase, Report};
use crate::run::{self, Call};
use crate::state::Installed;
use crate::{AppKey, Context, Done, Failure, Order, Outcome, Refusal, Roots, Source};

/// The `appimage-run` a `.desktop` names: a STABLE path, never a store path —
/// a `.desktop` written today has to open with the next generation's too.
pub const APPIMAGE_RUN: &str = "/run/current-system/sw/bin/appimage-run";
/// The prefix of our `.desktop` files, which is also how the launcher and a
/// removal recognise them.
pub const PREFIX: &str = "ronoctua-appimage-";
/// The key our `.desktop` carries so the entry can be traced back.
pub const MARK_KEY: &str = "X-Ronoctua-AppImage";
/// The sidecar's name, in `ours()/appimage/<key>/`.
pub const SIDECAR: &str = "app.json";
/// The most an icon may be.
pub const ICON_MOST: u64 = 4 << 20;
/// The most an embedded `.desktop` may be.
pub const DESKTOP_MOST: u64 = 64 << 10;
/// The most a downloaded AppImage may be: FreeCAD's is 820 MB, and this is
/// five of those.
pub const FILE_MOST: u64 = 4 << 30;
/// How old an extraction nobody owns must be before a tidy removes it: an
/// AppImage started by hand from `~/Downloads` is extracted there too, and a
/// running one must not lose its files under it.
pub const GRACE: u64 = 24 * 3600;

/// Seconds since the epoch.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// What an AppImage's own bytes say about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Where the squashfs image starts: `e_shoff + e_shentsize × e_shnum`,
    /// the offset `appimage-run` computes with `readelf`.
    pub payload: u64,
    /// The `.upd_info` section, zeros trimmed (`gh-releases-zsync|…`).
    pub update_info: Option<String>,
    /// The `.sha256_sig` section holds a signature (a type-2 runtime always
    /// has the section; unsigned, it is all zeros).
    pub signed: bool,
}

fn u16_at(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 2)
        .map(|s| u64::from(u16::from_le_bytes([s[0], s[1]])))
}

fn u32_at(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 4)
        .map(|s| u64::from(u32::from_le_bytes([s[0], s[1], s[2], s[3]])))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 8)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

/// Read an AppImage's header from anything seekable.
///
/// The type is the byte after `AI` at offset 8 of the ELF identification,
/// where the ELF specification leaves padding: `\x02` is the one this system
/// runs, `\x01` is an ISO 9660 image of the old kind. The payload's magic
/// decides the rest: `hsqs` is squashfs, and `DWARFS` is what `uruntime`
/// builds and `appimage-run` cannot open.
///
/// # Errors
///
/// `NotAnAppImage`, `LegacyAppImage` or `DwarfsAppImage`.
pub fn header_of(mut file: impl Read + Seek) -> Result<Header, Refusal> {
    let not = Refusal::NotAnAppImage;
    let length = file.seek(SeekFrom::End(0)).map_err(|_| not)?;
    let mut head = [0u8; 64];
    file.seek(SeekFrom::Start(0)).map_err(|_| not)?;
    file.read_exact(&mut head).map_err(|_| not)?;
    if &head[..4] != b"\x7fELF" || &head[8..10] != b"AI" {
        return Err(not);
    }
    match head[10] {
        2 => {}
        1 => return Err(Refusal::LegacyAppImage),
        _ => return Err(not),
    }
    // Little-endian only: every AppImage runtime there is is x86 or ARM.
    if head[5] != 1 {
        return Err(not);
    }
    let (shoff, shentsize, shnum, shstrndx, entry) = match head[4] {
        2 => (
            u64_at(&head, 0x28),
            u16_at(&head, 0x3A),
            u16_at(&head, 0x3C),
            u16_at(&head, 0x3E),
            64,
        ),
        1 => (
            u32_at(&head, 0x20),
            u16_at(&head, 0x2E),
            u16_at(&head, 0x30),
            u16_at(&head, 0x32),
            40,
        ),
        _ => return Err(not),
    };
    let (Some(shoff), Some(shentsize), Some(shnum), Some(shstrndx)) =
        (shoff, shentsize, shnum, shstrndx)
    else {
        return Err(not);
    };
    if shentsize != entry || shnum == 0 || shnum > 512 || shstrndx >= shnum {
        return Err(not);
    }
    let table_end = shoff.checked_add(shentsize * shnum).ok_or(not)?;
    if table_end > length {
        return Err(not);
    }
    let mut table = vec![0u8; usize::try_from(shentsize * shnum).map_err(|_| not)?];
    file.seek(SeekFrom::Start(shoff)).map_err(|_| not)?;
    file.read_exact(&mut table).map_err(|_| not)?;
    // (name offset, data offset, data size) of every section.
    let sections: Vec<(u64, u64, u64)> = table
        .chunks_exact(usize::try_from(shentsize).map_err(|_| not)?)
        .map(|s| {
            if entry == 64 {
                (
                    u32_at(s, 0).unwrap_or(0),
                    u64_at(s, 24).unwrap_or(0),
                    u64_at(s, 32).unwrap_or(0),
                )
            } else {
                (
                    u32_at(s, 0).unwrap_or(0),
                    u32_at(s, 16).unwrap_or(0),
                    u32_at(s, 20).unwrap_or(0),
                )
            }
        })
        .collect();
    let mut read_section = |at: u64, size: u64, most: u64| -> Option<Vec<u8>> {
        if size > most || at.checked_add(size)? > length {
            return None;
        }
        let mut out = vec![0u8; usize::try_from(size).ok()?];
        file.seek(SeekFrom::Start(at)).ok()?;
        file.read_exact(&mut out).ok()?;
        Some(out)
    };
    let (_, names_at, names_size) = sections[usize::try_from(shstrndx).map_err(|_| not)?];
    let names = read_section(names_at, names_size, 64 << 10).unwrap_or_default();
    let mut update_info = None;
    let mut signed = false;
    for &(name, at, size) in &sections {
        match name_at(&names, name) {
            b".upd_info" => {
                if let Some(bytes) = read_section(at, size, 8 << 10) {
                    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                    update_info = std::str::from_utf8(&bytes[..end])
                        .ok()
                        .map(str::trim)
                        .filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
                        .map(str::to_owned);
                }
            }
            b".sha256_sig" => {
                signed = read_section(at, size, 8 << 10).is_some_and(|b| b.iter().any(|&x| x != 0));
            }
            _ => {}
        }
    }
    let mut magic = Vec::with_capacity(6);
    file.seek(SeekFrom::Start(table_end)).map_err(|_| not)?;
    (&mut file)
        .take(6)
        .read_to_end(&mut magic)
        .map_err(|_| not)?;
    if magic == b"DWARFS" {
        return Err(Refusal::DwarfsAppImage);
    }
    if !magic.starts_with(b"hsqs") {
        return Err(not);
    }
    Ok(Header {
        payload: table_end,
        update_info,
        signed,
    })
}

/// The NUL-terminated name at `offset` of a string table.
fn name_at(names: &[u8], offset: u64) -> &[u8] {
    let from = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(names.len());
    let rest = &names[from..];
    &rest[..rest.iter().position(|&b| b == 0).unwrap_or(rest.len())]
}

/// Read the header of a file on disk.
///
/// # Errors
///
/// As [`header_of`]; a file that cannot be opened is `NotAnAppImage`.
pub fn header(path: &Path) -> Result<Header, Refusal> {
    let file = std::fs::File::open(path).map_err(|_| Refusal::NotAnAppImage)?;
    header_of(std::io::BufReader::new(file))
}

/// A repository as the feed and the update information name it:
/// `owner/repo`, nothing else.
#[must_use]
pub fn is_repo(owner_repo: &str) -> bool {
    let fine = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    owner_repo
        .split_once('/')
        .is_some_and(|(o, r)| fine(o) && fine(r))
}

/// The GitHub repository and the asset pattern out of update information of
/// the `gh-releases-zsync|owner|repo|tag|pattern` kind; `None` for any other
/// kind (plain `zsync|url`, Pling), which this package does not follow.
#[must_use]
pub fn github_of_update_info(info: &str) -> Option<(String, Option<String>)> {
    let parts: Vec<&str> = info.split('|').collect();
    if parts.len() < 5 || parts[0] != "gh-releases-zsync" {
        return None;
    }
    let repo = format!("{}/{}", parts[1], parts[2]);
    if !is_repo(&repo) {
        return None;
    }
    let pattern = parts[4]
        .strip_suffix(".zsync")
        .filter(|p| !p.is_empty() && p.chars().all(|c| !c.is_control() && c != '/'))
        .map(str::to_owned);
    Some((repo, pattern))
}

/// The argument vector that pulls `patterns` out of the image at `offset`
/// into `staging`: quiet, no progress bar, no extended attributes (a staging
/// directory on a filesystem without them would make every pass exit 2), and
/// `-f` so a second pass writes into the directory the first one made.
#[must_use]
pub fn extract_argv(offset: u64, staging: &Path, file: &Path, patterns: &[String]) -> Vec<String> {
    let mut argv: Vec<String> = [
        "-q".to_owned(),
        "-n".to_owned(),
        "-no-xattrs".to_owned(),
        "-o".to_owned(),
        offset.to_string(),
        "-d".to_owned(),
        staging.display().to_string(),
        "-f".to_owned(),
        file.display().to_string(),
    ]
    .into();
    argv.extend(patterns.iter().cloned());
    argv
}

/// A path inside the image that may be asked for LITERALLY: relative, no
/// `..`, no component that could be read as an option, no glob character
/// (it would stop being literal), nothing invisible.
fn literal_path(rel: &str) -> bool {
    !rel.is_empty()
        && rel.len() <= 512
        && !rel.starts_with('/')
        && rel
            .split('/')
            .all(|c| !c.is_empty() && c != ".." && !c.starts_with('-'))
        && !rel
            .chars()
            .any(|c| c.is_control() || matches!(c, '*' | '?' | '[' | ']' | '\\'))
}

/// An icon NAME from a `.desktop` that may go into a pattern.
fn icon_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && !name.starts_with(['-', '.'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'+'))
}

/// Where a path inside the staging directory leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Walk {
    /// A regular file, inside the root.
    File(PathBuf),
    /// Not extracted yet: this image path (root-relative, links resolved so
    /// far) is what to ask for.
    Missing(String),
    /// A directory, a device, a loop, or a way out of the root.
    Refused,
}

/// Follow `rel` inside `root` one component at a time, resolving every
/// symlink by hand — an ABSOLUTE link means the image's root, never the
/// machine's — so nothing is ever read outside the staging directory. The
/// operating system is never asked to follow a link: an extracted `usr` that
/// pointed at the person's home would otherwise be walked straight through.
#[must_use]
pub fn walk(root: &Path, rel: &str) -> Walk {
    let mut todo: std::collections::VecDeque<String> = rel.split('/').map(str::to_owned).collect();
    let mut at: Vec<String> = Vec::new();
    let mut hops = 0;
    while let Some(part) = todo.pop_front() {
        match part.as_str() {
            "" | "." => continue,
            ".." => {
                if at.pop().is_none() {
                    return Walk::Refused;
                }
                continue;
            }
            _ => {}
        }
        let here = at
            .iter()
            .fold(root.to_path_buf(), |p, c| p.join(c))
            .join(&part);
        let meta = match std::fs::symlink_metadata(&here) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let missing: Vec<&str> = at
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(part.as_str()))
                    .chain(todo.iter().map(String::as_str))
                    .filter(|c| !c.is_empty() && *c != ".")
                    .collect();
                return Walk::Missing(missing.join("/"));
            }
            Err(_) => return Walk::Refused,
        };
        if meta.file_type().is_symlink() {
            hops += 1;
            if hops > 16 {
                return Walk::Refused;
            }
            let Ok(target) = std::fs::read_link(&here) else {
                return Walk::Refused;
            };
            let Some(target) = target.to_str() else {
                return Walk::Refused;
            };
            if target.starts_with('/') {
                at.clear();
            }
            for c in target.split('/').rev() {
                todo.push_front(c.to_owned());
            }
        } else if meta.is_dir() {
            if todo.iter().all(|c| c.is_empty() || c == ".") {
                return Walk::Refused;
            }
            at.push(part);
        } else if meta.is_file() {
            if todo.iter().any(|c| !c.is_empty() && c != ".") {
                return Walk::Refused;
            }
            return Walk::File(here);
        } else {
            return Walk::Refused;
        }
    }
    Walk::Refused
}

/// What the image's own `.desktop` says, already cleaned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Embedded {
    /// `Name=`.
    pub name: String,
    /// `Name[xx]=`, as `(locale, value)`.
    pub names: Vec<(String, String)>,
    /// `Comment=`.
    pub comment: Option<String>,
    /// `Comment[xx]=`.
    pub comments: Vec<(String, String)>,
    /// `Categories=`.
    pub categories: Vec<String>,
    /// `MimeType=`.
    pub mime_types: Vec<String>,
    /// The first of `%f %F %u %U` in its `Exec=`, which ours passes on.
    pub field_code: Option<String>,
    /// `StartupWMClass=`, so the modeline can name the window.
    pub wm_class: Option<String>,
    /// `Terminal=true`.
    pub terminal: bool,
    /// `X-AppImage-Version=`.
    pub version: Option<String>,
    /// `Icon=`, when it is a name that may be looked for in the image.
    pub icon: Option<String>,
}

/// A value that goes into a `.desktop` we write: no control character (a
/// newline would start a line of its own — `Name=x` then `Exec=anything`),
/// no backslash (the one escape character), at most 200 characters.
#[must_use]
pub fn clean(value: &str) -> String {
    let kept: String = value
        .chars()
        .filter(|c| !c.is_control() && *c != '\\')
        .take(200)
        .collect();
    kept.trim().to_owned()
}

/// A locale suffix that may be copied (`de`, `pt_BR`, `sr@latin`).
fn locale_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 20
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'@' | b'.' | b'-'))
}

/// A list item: a category (`AudioVideo`) or a media type (`image/png`).
fn list_item(item: &str, media: bool) -> bool {
    let token = |s: &str| {
        !s.is_empty()
            && s.len() <= 127
            && s.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || matches!(
                        b,
                        b'-' | b'_' | b'.' | b'+' | b'!' | b'#' | b'$' | b'&' | b'^'
                    )
            })
    };
    if media {
        item.split_once('/')
            .is_some_and(|(t, s)| token(t) && token(s))
    } else {
        token(item) && !item.contains(['.', '/'])
    }
}

/// Read an embedded `.desktop`; `None` without a usable `Name=`.
#[must_use]
pub fn embedded_of(text: &str) -> Option<Embedded> {
    use ronoctua_apps::desktop::{boolean, fields, list, raw_bytes};
    let f = fields(text);
    let name = clean(raw_bytes(&f, "Name")?);
    if name.is_empty() {
        return None;
    }
    let localized = |key: &str| -> Vec<(String, String)> {
        f.iter()
            .filter_map(|(k, v)| {
                let tag = k.strip_prefix(key)?.strip_prefix('[')?.strip_suffix(']')?;
                let value = clean(v);
                (locale_tag(tag) && !value.is_empty()).then(|| (tag.to_owned(), value))
            })
            .collect()
    };
    let exec = raw_bytes(&f, "Exec").unwrap_or_default();
    let field_code = ronoctua_apps::exec::tokenize(exec)
        .into_iter()
        .find(|t| matches!(t.as_str(), "%f" | "%F" | "%u" | "%U"));
    Some(Embedded {
        names: localized("Name"),
        comment: raw_bytes(&f, "Comment")
            .map(clean)
            .filter(|c| !c.is_empty()),
        comments: localized("Comment"),
        categories: list(&f, "Categories")
            .into_iter()
            .filter(|c| list_item(c, false))
            .map(str::to_owned)
            .collect(),
        mime_types: list(&f, "MimeType")
            .into_iter()
            .filter(|m| list_item(m, true))
            .map(str::to_owned)
            .collect(),
        field_code,
        wm_class: raw_bytes(&f, "StartupWMClass")
            .map(clean)
            .filter(|c| !c.is_empty()),
        terminal: boolean(&f, "Terminal"),
        version: raw_bytes(&f, "X-AppImage-Version")
            .map(clean)
            .filter(|v| !v.is_empty()),
        icon: raw_bytes(&f, "Icon")
            .map(str::trim)
            .filter(|i| icon_name(i))
            .map(str::to_owned),
        name,
    })
}

/// One argument of an `Exec=` line, quoted by the specification's rule:
/// inside double quotes `"`, `` ` `` and `$` are escaped with a backslash, and
/// `%` is doubled because it is the field-code character. `None` for what
/// cannot be written so that every reader agrees on it: a backslash (the
/// string level and the quoting level would each take one) or a control
/// character.
#[must_use]
pub fn quoted(arg: &str) -> Option<String> {
    if arg.chars().any(|c| c.is_control() || c == '\\') {
        return None;
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out.push('"');
    Some(out)
}

/// The `.desktop` file name of a key.
#[must_use]
pub fn desktop_name(key: &str) -> String {
    format!("{PREFIX}{key}.desktop")
}

/// Our `.desktop` for an integrated AppImage. Pure: what a test reads is
/// what the launcher reads.
///
/// # Errors
///
/// `NotAnAppImage` when the file's path cannot be written into an `Exec=`
/// line that every reader parses the same way (see [`quoted`]).
pub fn desktop_entry(
    e: &Embedded,
    file: &Path,
    icon: Option<&Path>,
    key: &str,
) -> Result<String, Refusal> {
    let file = quoted(&file.display().to_string()).ok_or(Refusal::NotAnAppImage)?;
    let run = quoted(APPIMAGE_RUN).ok_or(Refusal::NotAnAppImage)?;
    let mut out = String::from("[Desktop Entry]\nType=Application\n");
    out.push_str(&format!("Name={}\n", e.name));
    for (tag, value) in &e.names {
        out.push_str(&format!("Name[{tag}]={value}\n"));
    }
    if let Some(c) = &e.comment {
        out.push_str(&format!("Comment={c}\n"));
    }
    for (tag, value) in &e.comments {
        out.push_str(&format!("Comment[{tag}]={value}\n"));
    }
    out.push_str(&format!("Exec={run} {file}"));
    if let Some(code) = &e.field_code {
        out.push(' ');
        out.push_str(code);
    }
    out.push('\n');
    out.push_str(&format!("TryExec={APPIMAGE_RUN}\n"));
    if let Some(icon) = icon
        .map(|p| clean(&p.display().to_string()))
        .filter(|p| !p.is_empty())
    {
        out.push_str(&format!("Icon={icon}\n"));
    }
    out.push_str(&format!("Terminal={}\n", e.terminal));
    if !e.categories.is_empty() {
        out.push_str(&format!("Categories={};\n", e.categories.join(";")));
    }
    if !e.mime_types.is_empty() {
        out.push_str(&format!("MimeType={};\n", e.mime_types.join(";")));
    }
    if let Some(class) = &e.wm_class {
        out.push_str(&format!("StartupWMClass={class}\n"));
    }
    out.push_str(&format!("{MARK_KEY}={key}\n"));
    Ok(out)
}

/// Where an integrated AppImage came from, which is what its updates follow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// GitHub releases of `owner/repo` — from the catalogue, or from the
    /// file's own `gh-releases-zsync` update information.
    Github(String),
    /// The catalogue named it but no repository.
    Feed(String),
    /// A file the person gave, with no way to update.
    File,
}

/// What is kept beside an integrated AppImage (`ours()/appimage/<key>/app.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Sidecar {
    /// Its key.
    pub key: String,
    /// Its name (data).
    pub name: String,
    /// The file, in `~/Applications`.
    pub file: PathBuf,
    /// The file's SHA-256 — also the name of `appimage-run`'s extraction.
    pub sha256: String,
    /// The file's own update information.
    #[serde(default)]
    pub update_info: Option<String>,
    /// Where it came from.
    pub source: Provenance,
    /// The release tag it was downloaded from.
    #[serde(default)]
    pub tag: Option<String>,
    /// When it was integrated (seconds since the epoch).
    pub installed: u64,
    /// The version its `.desktop` states.
    #[serde(default)]
    pub version: Option<String>,
    /// The asset-name pattern of its update information.
    #[serde(default)]
    pub pattern: Option<String>,
    /// The icon file we wrote.
    #[serde(default)]
    pub icon: Option<PathBuf>,
    /// Its download was checked against a digest. An upgrade of one that was
    /// never verified may stay unverified (the person already said yes to that
    /// source); one that was verified may not silently stop being.
    #[serde(default)]
    pub verified: bool,
}

/// `ours()/appimage`: one directory per key, holding the sidecar and the
/// icon. Named by the source's word, which is also why it is not `appimages`
/// — that is an update step's id, and a step is named in one place only
/// (`update-has-one-list`).
#[must_use]
pub fn sidecars_dir(roots: &Roots) -> PathBuf {
    roots.ours().join(Source::AppImage.word())
}

/// A key as a directory and file name: what [`crate::slug_of`] makes.
fn is_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 40
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// One sidecar, by key.
#[must_use]
pub fn sidecar(roots: &Roots, key: &str) -> Option<Sidecar> {
    if !is_key(key) {
        return None;
    }
    let bytes = std::fs::read(sidecars_dir(roots).join(key).join(SIDECAR)).ok()?;
    serde_json::from_slice(&bytes)
        .ok()
        .filter(|s: &Sidecar| s.key == key)
}

/// Every sidecar, by key.
#[must_use]
pub fn sidecars(roots: &Roots) -> Vec<Sidecar> {
    let Ok(dir) = std::fs::read_dir(sidecars_dir(roots)) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = dir
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    keys.sort();
    keys.iter().filter_map(|k| sidecar(roots, k)).collect()
}

fn write_sidecar(roots: &Roots, s: &Sidecar) -> Result<(), Failure> {
    let dir = sidecars_dir(roots).join(&s.key);
    let bytes = serde_json::to_vec_pretty(s).map_err(|e| Failure::trouble(e.to_string()))?;
    ronoctua_xdg::write_atomically(&dir, SIDECAR, &bytes)
        .map_err(|e| Failure::trouble(format!("could not write {}: {e}", dir.display())))
}

/// `appimage-run`'s extraction of a file with this hash.
#[must_use]
pub fn extraction(roots: &Roots, sha256: &str) -> Option<PathBuf> {
    (sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| roots.cache.join("appimage-run").join(sha256))
}

/// Remove a tree, never following a link, making each directory writable
/// first (`unsquashfs` keeps the image's modes, and a `0555` directory cannot
/// lose its entries). Answers how many bytes of regular files went.
///
/// # Errors
///
/// What the filesystem says.
pub fn remove_tree(path: &Path) -> std::io::Result<u64> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_dir() {
        let bytes = if meta.is_file() { meta.len() } else { 0 };
        std::fs::remove_file(path)?;
        return Ok(bytes);
    }
    if meta.permissions().mode() & 0o700 != 0o700 {
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(meta.permissions().mode() | 0o700),
        )?;
    }
    let mut bytes = 0;
    for entry in std::fs::read_dir(path)? {
        bytes += remove_tree(&entry?.path())?;
    }
    std::fs::remove_dir(path)?;
    Ok(bytes)
}

/// A staging directory that is removed whatever happens.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = remove_tree(&self.0);
        }
    }
}

fn staging(roots: &Roots, what: &str) -> Staging {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    Staging(
        roots
            .ours_cache()
            .join("staging")
            .join(format!("{what}-{}-{n}", std::process::id())),
    )
}

/// What kind of picture some bytes are, if they are one a launcher may show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    /// A PNG.
    Png,
    /// An SVG that reaches nothing outside itself.
    Svg,
}

impl IconKind {
    /// Its extension.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }
}

/// PNG by its signature; SVG by its root element, and only when every `href`
/// points inside the file (`#…`) or at inline data — an SVG renderer resolves
/// a file path in an `<image href>`, and an icon has no business reading one.
#[must_use]
pub fn picture_of(bytes: &[u8]) -> Option<IconKind> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(IconKind::Png);
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let start = text.trim_start_matches('\u{feff}').trim_start();
    if !(start.starts_with("<?xml") || start.starts_with("<svg") || start.starts_with("<!--"))
        || !text.contains("<svg")
    {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(at) = rest.find("href") {
        let after = rest[at + 4..].trim_start();
        if let Some(value) = after.strip_prefix('=') {
            let value = value.trim_start().trim_start_matches(['"', '\'']);
            if !(value.starts_with('#') || value.starts_with("data:")) {
                return None;
            }
        }
        rest = &rest[at + 4..];
    }
    Some(IconKind::Svg)
}

/// The icons to try, best first, for an `Icon=` name: hicolor PNGs from 256
/// px down, `.DirIcon`, the top-level PNG, then the SVGs.
fn icon_candidates(icon: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(i) = icon {
        for size in [
            "256x256", "512x512", "192x192", "128x128", "96x96", "64x64", "48x48",
        ] {
            out.push(format!("usr/share/icons/hicolor/{size}/apps/{i}.png"));
        }
    }
    out.push(".DirIcon".to_owned());
    if let Some(i) = icon {
        out.push(format!("{i}.png"));
        out.push(format!("usr/share/icons/hicolor/scalable/apps/{i}.svg"));
        out.push(format!("{i}.svg"));
    }
    out
}

/// What was read out of an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pulled {
    /// Its `.desktop`, when it has one that reads.
    pub embedded: Option<Embedded>,
    /// Its icon's bytes and kind.
    pub icon: Option<(Vec<u8>, IconKind)>,
}

/// Pull the `.desktop` and the icon out of an image. Runs `unsquashfs`: call
/// it on a thread.
///
/// # Errors
///
/// `SourceAbsent` without `unsquashfs`; `ToolFailed` when it fails.
pub fn read_image(ctx: &Context, file: &Path, head: &Header) -> Result<Pulled, Failure> {
    let unsquashfs = run::tool(ctx.tools.unsquashfs.as_ref())?;
    let stage = staging(&ctx.roots, "appimage");
    let root = stage.0.clone();
    if let Some(parent) = root.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            Failure::trouble(format!("could not prepare {}: {e}", parent.display()))
        })?;
    }
    let extract = |patterns: &[String]| -> Result<(), Failure> {
        let ran = Call::new(
            unsquashfs,
            extract_argv(head.payload, &root, file, patterns),
        )
        .quietly(ctx)?;
        // 2 is "non-fatal errors" (a mode or a time it could not set): the
        // files are there.
        if matches!(ran.code, Some(0 | 2)) {
            Ok(())
        } else {
            Err(Failure {
                refusal: Some(Refusal::ToolFailed),
                trouble: ran.tail.join("\n"),
            })
        }
    };
    let mut asked: BTreeSet<String> = BTreeSet::new();
    let mut fetch = |rel: &str| -> Result<Option<PathBuf>, Failure> {
        for _ in 0..4 {
            match walk(&root, rel) {
                Walk::File(p) => return Ok(Some(p)),
                Walk::Missing(m) => {
                    if !literal_path(&m) || !asked.insert(m.clone()) {
                        return Ok(None);
                    }
                    extract(&[m])?;
                }
                Walk::Refused => return Ok(None),
            }
        }
        Ok(None)
    };
    extract(&["*.desktop".to_owned(), ".DirIcon".to_owned()])?;
    let mut tops: Vec<String> = std::fs::read_dir(&root)
        .map(|d| {
            d.filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|n| n.ends_with(".desktop") && literal_path(n))
                .collect()
        })
        .unwrap_or_default();
    tops.sort();
    let mut embedded = None;
    for name in &tops {
        if let Some(path) = fetch(name)? {
            let text = run::head_of(&path, usize::try_from(DESKTOP_MOST).unwrap_or(usize::MAX))
                .unwrap_or_default();
            if let Some(e) = embedded_of(&String::from_utf8_lossy(&text)) {
                embedded = Some(e);
                break;
            }
        }
    }
    let icon_name = embedded.as_ref().and_then(|e| e.icon.clone());
    if let Some(i) = &icon_name {
        extract(&[
            format!("usr/share/icons/hicolor/*/apps/{i}.png"),
            format!("usr/share/icons/hicolor/scalable/apps/{i}.svg"),
            format!("{i}.png"),
            format!("{i}.svg"),
        ])?;
    }
    let mut icon = None;
    for candidate in icon_candidates(icon_name.as_deref()) {
        let Some(path) = fetch(&candidate)? else {
            continue;
        };
        let fits = std::fs::metadata(&path).is_ok_and(|m| m.len() <= ICON_MOST);
        let bytes = if fits {
            std::fs::read(&path).unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(kind) = picture_of(&bytes) {
            icon = Some((bytes, kind));
            break;
        }
    }
    Ok(Pulled { embedded, icon })
}

/// How a file gets to `~/Applications`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Copied: the person's file stays where it was.
    Copy,
    /// Moved: our own download, or a file already in `~/Applications`.
    Move,
}

/// Put `from` at `to` with mode 0755, atomically: a copy goes to a `.part`
/// beside the target and is renamed over it, so a launcher never sees half
/// a file under the real name.
fn place(from: &Path, to: &Path, how: Placement) -> Result<(), Failure> {
    let trouble =
        |e: std::io::Error| Failure::trouble(format!("could not place {}: {e}", to.display()));
    let same = std::fs::canonicalize(from)
        .ok()
        .zip(std::fs::canonicalize(to).ok())
        .is_some_and(|(a, b)| a == b);
    if !same {
        let moved = how == Placement::Move && std::fs::rename(from, to).is_ok();
        if !moved {
            let name = to
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("appimage");
            let part = to.with_file_name(format!(".{name}.{}.part", std::process::id()));
            if let Err(e) = std::fs::copy(from, &part).and_then(|_| std::fs::rename(&part, to)) {
                let _ = std::fs::remove_file(&part);
                return Err(trouble(e));
            }
            if how == Placement::Move {
                let _ = std::fs::remove_file(from);
            }
        }
    }
    std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o755)).map_err(trouble)
}

/// Everything one integration is told.
#[derive(Debug, Clone)]
pub struct Integration {
    /// The key to give it; `None` derives it from the embedded name.
    pub key: Option<String>,
    /// Where it came from.
    pub source: Provenance,
    /// The release tag.
    pub tag: Option<String>,
    /// The asset pattern, when the source has one.
    pub pattern: Option<String>,
    /// Its download was checked against a digest.
    pub verified: bool,
    /// Copy or move.
    pub placement: Placement,
}

/// Integrate one AppImage: check it, read its `.desktop` and icon, put it in
/// `~/Applications`, write the icon, our `.desktop` and the sidecar, and drop
/// the extraction of the file it replaces. Runs `unsquashfs`: call it on a
/// thread.
///
/// # Errors
///
/// The header's refusal, `SourceAbsent` without the tools, and the
/// filesystem's words.
pub fn integrate(
    ctx: &Context,
    file: &Path,
    how: &Integration,
    report: &mut dyn Report,
) -> Result<Sidecar, Failure> {
    run::tool(ctx.tools.appimage_run.as_ref())?;
    let roots = &ctx.roots;
    let label = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    report.step(Phase::Verifying, 0, 0, &label);
    let head = header(file).map_err(Failure::refused)?;
    let sha256 = run::sha256_of(file)
        .map_err(|e| Failure::trouble(format!("could not read {}: {e}", file.display())))?;
    report.step(Phase::Unpacking, 0, 0, &label);
    let read = read_image(ctx, file, &head)?;
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let embedded = read.embedded.unwrap_or_else(|| Embedded {
        name: clean(stem),
        ..Embedded::default()
    });
    let key = how
        .key
        .clone()
        .filter(|k| is_key(k))
        .or_else(|| Some(crate::slug_of(&embedded.name)).filter(|k| is_key(k)))
        .or_else(|| Some(crate::slug_of(stem)).filter(|k| is_key(k)))
        .unwrap_or_else(|| "appimage".to_owned());
    report.step(Phase::Integrating, 0, 0, &embedded.name);
    let before = sidecar(roots, &key);
    let dir = roots.appimages();
    std::fs::create_dir_all(&dir)
        .map_err(|e| Failure::trouble(format!("could not create {}: {e}", dir.display())))?;
    let dest = dir.join(format!("{key}.AppImage"));
    let icon_dir = sidecars_dir(roots).join(&key);
    let icon = read.icon.map(|(bytes, kind)| {
        let name = format!("icon-{}.{}", &run::sha256(&bytes)[..8], kind.extension());
        (icon_dir.join(&name), name, bytes)
    });
    // EVERYTHING THAT CAN BE REFUSED IS DECIDED BEFORE ANYTHING IS WRITTEN: a
    // path our `Exec=` cannot hold must not leave a file placed and no entry.
    let text = desktop_entry(
        &embedded,
        &dest,
        icon.as_ref().map(|(p, ..)| p.as_path()),
        &key,
    )
    .map_err(Failure::refused)?;
    place(file, &dest, how.placement)?;
    if let Some((_, name, bytes)) = &icon {
        ronoctua_xdg::write_atomically(&icon_dir, name, bytes)
            .map_err(|e| Failure::trouble(format!("could not write the icon: {e}")))?;
    }
    let icon = icon.map(|(path, ..)| path);
    // THE OLD ICONS GO: the name carries the hash so the launcher's pixmap
    // cache sees a new file, and the old one would otherwise stay for ever.
    if let Ok(entries) = std::fs::read_dir(&icon_dir) {
        for e in entries.flatten() {
            let p = e.path();
            let ours = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("icon-"));
            if ours && Some(&p) != icon.as_ref() {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    // The entry last but one, the sidecar last: a launcher never lists an
    // entry whose file is not there yet.
    ronoctua_xdg::write_atomically(&roots.applications(), &desktop_name(&key), text.as_bytes())
        .map_err(|e| Failure::trouble(format!("could not write the launcher entry: {e}")))?;
    let (source, pattern) = match (
        &how.source,
        head.update_info.as_deref().and_then(github_of_update_info),
    ) {
        // A file that names its own releases can be updated after all.
        (Provenance::File, Some((repo, pattern))) => (Provenance::Github(repo), pattern),
        (s, _) => (s.clone(), how.pattern.clone()),
    };
    let kept = Sidecar {
        key,
        name: embedded.name.clone(),
        file: dest,
        sha256: sha256.clone(),
        update_info: head.update_info,
        source,
        tag: how.tag.clone(),
        installed: now(),
        version: embedded.version.clone(),
        pattern,
        icon,
        verified: how.verified,
    };
    write_sidecar(roots, &kept)?;
    if let Some(old) = before.filter(|b| b.sha256 != sha256)
        && let Some(path) = extraction(roots, &old.sha256)
        && path.exists()
    {
        let _ = remove_tree(&path);
    }
    Ok(kept)
}

/// The AppImages this crate integrated.
#[must_use]
pub fn installed(ctx: &Context) -> Vec<Installed> {
    sidecars(&ctx.roots)
        .into_iter()
        .map(|s| {
            let meta = std::fs::metadata(&s.file).ok();
            let trouble =
                (meta.is_none() || header(&s.file).is_err()).then_some(Refusal::NotAnAppImage);
            Installed {
                key: AppKey::AppImage { key: s.key.clone() },
                name: s.name.clone(),
                version: s
                    .tag
                    .clone()
                    .or_else(|| s.version.clone())
                    .unwrap_or_default(),
                desktop_ids: vec![desktop_name(&s.key)],
                size_bytes: meta.map(|m| m.len()),
                update: known_update(ctx, &s),
                unfree: false,
                origin: crate::state::Origin::Mine,
                icon: s.icon.clone().map(|path| Picture::File { path }),
                trouble,
            }
        })
        .collect()
}

/// A newer tag already KNOWN (from the kept GitHub answer), never asked for.
#[cfg(feature = "net")]
fn known_update(ctx: &Context, s: &Sidecar) -> Option<String> {
    let Provenance::Github(repo) = &s.source else {
        return None;
    };
    let kept = crate::github::kept(&github_cache(&ctx.roots), repo)?;
    let asset = crate::github::choose(kept.release.as_ref()?, s.pattern.as_deref())?;
    let same = s.tag.as_deref() == Some(asset.tag.as_str())
        || asset.sha256().as_deref() == Some(s.sha256.as_str());
    (!same).then_some(asset.tag)
}

#[cfg(not(feature = "net"))]
fn known_update(_: &Context, _: &Sidecar) -> Option<String> {
    None
}

/// Where GitHub's answers are kept.
#[must_use]
pub fn github_cache(roots: &Roots) -> PathBuf {
    roots.ours_cache().join("github")
}

/// AppImages in `~/Applications` that nothing integrated — put there by hand.
/// An `InstallFile` of one moves it into our name instead of copying it.
#[must_use]
pub fn strays(roots: &Roots) -> Vec<PathBuf> {
    let owned: BTreeSet<PathBuf> = sidecars(roots).into_iter().map(|s| s.file).collect();
    let Ok(dir) = std::fs::read_dir(roots.appimages()) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = dir
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("appimage"))
                && std::fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
                && !owned.contains(p)
        })
        .collect();
    out.sort();
    out
}

/// Remove one integrated AppImage: the file, our `.desktop`, the icon and
/// sidecar directory, and `appimage-run`'s extraction. Nothing outside
/// `~/Applications` is ever deleted, whatever a sidecar says. Answers the
/// bytes freed.
///
/// # Errors
///
/// `UnknownItem` for a key nothing integrated.
pub fn remove(roots: &Roots, key: &str) -> Result<u64, Failure> {
    let s = sidecar(roots, key).ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
    let mut freed = 0;
    let ours = s.file.parent() == Some(roots.appimages().as_path())
        && s.file
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("appimage"));
    if ours && std::fs::symlink_metadata(&s.file).is_ok() {
        freed += remove_tree(&s.file)
            .map_err(|e| Failure::trouble(format!("could not remove {}: {e}", s.file.display())))?;
    }
    let desktop = roots.applications().join(desktop_name(key));
    if desktop.exists() {
        std::fs::remove_file(&desktop).map_err(|e| {
            Failure::trouble(format!("could not remove {}: {e}", desktop.display()))
        })?;
    }
    if let Some(path) = extraction(roots, &s.sha256).filter(|p| p.exists()) {
        freed += remove_tree(&path).unwrap_or(0);
    }
    let dir = sidecars_dir(roots).join(key);
    freed += remove_tree(&dir)
        .map_err(|e| Failure::trouble(format!("could not remove {}: {e}", dir.display())))?;
    Ok(freed)
}

/// Remove `appimage-run` extractions no integrated AppImage owns and nothing
/// touched for [`GRACE`]. Answers the bytes freed.
#[must_use]
pub fn tidy(roots: &Roots, now: u64) -> u64 {
    let owned: BTreeSet<String> = sidecars(roots).into_iter().map(|s| s.sha256).collect();
    let Ok(dir) = std::fs::read_dir(roots.cache.join("appimage-run")) else {
        return 0;
    };
    let mut freed = 0;
    for entry in dir.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Some(path) = extraction(roots, &name) else {
            continue;
        };
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir() || owned.contains(&name.to_ascii_lowercase()) {
            continue;
        }
        // The newest of the directory's own times: `appimage-run` changes its
        // mode right after extracting, so the change time is the extraction's.
        let newest = [meta.modified(), meta.accessed()]
            .into_iter()
            .filter_map(|t| t.ok()?.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .chain(std::iter::once(
                std::os::unix::fs::MetadataExt::ctime(&meta)
                    .max(0)
                    .unsigned_abs(),
            ))
            .max()
            .unwrap_or(now);
        if now.saturating_sub(newest) < GRACE {
            continue;
        }
        freed += remove_tree(&path).unwrap_or(0);
    }
    freed
}

fn rehearsed(order: &Order, key: Option<AppKey>) -> Done {
    use std::io::Write as _;
    let _ = writeln!(std::io::stderr(), "rehearse: appimage {}", order.word());
    Ok(Outcome {
        key,
        rehearsed: true,
        ..Outcome::default()
    })
}

fn key_of_order(order: &Order) -> Option<AppKey> {
    match order {
        Order::Install { key, .. } | Order::Remove { key, .. } => Some(key.clone()),
        Order::Upgrade {
            scope: Scope::One { key },
        } => Some(key.clone()),
        _ => None,
    }
}

/// Run one order whose lane is AppImage: `Install` (from the catalogue),
/// `InstallFile`, `Remove`, `Upgrade`, `Tidy`, `Refresh`.
pub fn run(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    #[cfg(feature = "net")]
    {
        run_with(ctx, order, report, &crate::github::Net)
    }
    #[cfg(not(feature = "net"))]
    {
        run_offline(ctx, order, report)
    }
}

/// [`run`] without the network: the orders that need it are `Offline`.
#[cfg(not(feature = "net"))]
fn run_offline(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    if ctx.rehearse && order.changes_something() {
        return rehearsed(order, key_of_order(order));
    }
    match order {
        Order::Install { .. } | Order::Refresh { .. } | Order::Upgrade { .. } => {
            Err(Failure::refused(Refusal::Offline))
        }
        other => local(ctx, other, report),
    }
}

/// The orders that never reach the network.
fn local(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    match order {
        Order::InstallFile { path, consent } => install_file(ctx, path, *consent, report),
        // `keep_data` has nothing to keep apart: an AppImage writes its
        // settings wherever the program chose, which nothing here knows, and
        // removing touches only what integrating wrote.
        Order::Remove {
            key: AppKey::AppImage { key },
            ..
        } => {
            report.step(Phase::Removing, 0, 0, key);
            let freed = remove(&ctx.roots, key)?;
            Ok(Outcome {
                key: Some(AppKey::AppImage { key: key.clone() }),
                amount: Some(freed),
                ..Outcome::default()
            })
        }
        Order::Tidy {
            source: Source::AppImage,
        } => {
            report.step(Phase::Removing, 0, 0, "appimage-run");
            Ok(Outcome {
                amount: Some(tidy(&ctx.roots, now())),
                ..Outcome::default()
            })
        }
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

fn install_file(ctx: &Context, path: &Path, consent: Consent, report: &mut dyn Report) -> Done {
    // A FILE FROM NOWHERE is never verified, and saying so is the question.
    if !consent.unverified {
        return Err(Failure::refused(Refusal::UnverifiedNeedsConsent));
    }
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(Failure::refused(Refusal::NotAnAppImage));
    }
    let adopted = path.parent() == Some(ctx.roots.appimages().as_path());
    let how = Integration {
        key: None,
        source: Provenance::File,
        tag: None,
        pattern: None,
        verified: false,
        placement: if adopted {
            Placement::Move
        } else {
            Placement::Copy
        },
    };
    let s = integrate(ctx, path, &how, report)?;
    Ok(Outcome {
        key: Some(AppKey::AppImage { key: s.key.clone() }),
        desktop_ids: vec![desktop_name(&s.key)],
        ..Outcome::default()
    })
}

/// [`run`] over any network (a test hands fixtures).
#[cfg(feature = "net")]
pub fn run_with(
    ctx: &Context,
    order: &Order,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
) -> Done {
    if ctx.rehearse && order.changes_something() {
        return rehearsed(order, key_of_order(order));
    }
    match order {
        Order::Install {
            key: AppKey::AppImage { key },
            consent,
        } => install(ctx, key, *consent, report, net),
        Order::Upgrade { scope } => upgrade(ctx, scope, report, net),
        Order::Refresh {
            source: Source::AppImage,
        } => {
            report.step(Phase::Indexing, 0, 0, "appimage.github.io");
            let items = crate::appimagehub::refresh_with(net, &ctx.roots)?;
            Ok(Outcome {
                amount: Some(items.len() as u64),
                ..Outcome::default()
            })
        }
        other => local(ctx, other, report),
    }
}

/// Download an asset into a staging file, checking its digest when there is
/// one. The consent questions are answered BEFORE a byte is fetched.
#[cfg(feature = "net")]
fn fetch_asset(
    ctx: &Context,
    asset: &crate::github::Asset,
    consent: Consent,
    allow_unverified: bool,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
) -> Result<(Staging, bool), Failure> {
    let digest = asset.sha256();
    if digest.is_none() && !(consent.unverified || allow_unverified) {
        return Err(Failure::refused(Refusal::UnverifiedNeedsConsent));
    }
    let ask_above = ctx.prefs.ask_above_mb.saturating_mul(1 << 20);
    if ask_above > 0 && asset.size > ask_above && !consent.big {
        return Err(Failure::refused(Refusal::BigNeedsConsent));
    }
    let stage = staging(&ctx.roots, "download");
    std::fs::create_dir_all(&stage.0)
        .map_err(|e| Failure::trouble(format!("could not prepare {}: {e}", stage.0.display())))?;
    let file = stage
        .0
        .join(format!("{}.AppImage", crate::slug_of(&asset.name)));
    let most = if asset.size > 0 {
        asset.size.min(FILE_MOST)
    } else {
        FILE_MOST
    };
    let mut last = 0u64;
    net.download(&asset.url, most, &file, &mut |done, total| {
        // A beat per mebibyte: the queue redraws per line, and a line per
        // 64 KiB chunk would be two thousand of them for one AppImage.
        if done >= last + (1 << 20) || Some(done) == total {
            last = done;
            report.step(
                Phase::Downloading,
                done,
                total.unwrap_or(asset.size),
                &asset.name,
            );
        }
        true
    })?;
    if let Some(want) = digest {
        report.step(Phase::Verifying, 0, 0, &asset.name);
        let got = run::sha256_of(&file).map_err(|e| Failure::trouble(e.to_string()))?;
        if got != want {
            return Err(Failure {
                refusal: Some(Refusal::Integrity),
                trouble: format!("{}: sha256 {got}, the release says {want}", asset.name),
            });
        }
        return Ok((stage, true));
    }
    Ok((stage, false))
}

#[cfg(feature = "net")]
fn install(
    ctx: &Context,
    key: &str,
    consent: Consent,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
) -> Done {
    report.step(Phase::Resolving, 0, 0, key);
    let item = match crate::appimagehub::item(&ctx.roots, key) {
        Some(i) => i,
        None => crate::appimagehub::catalogue_with(net, &ctx.roots, ctx.prefs.online, now())?
            .into_iter()
            .find(|i| i.key == key)
            .ok_or_else(|| Failure::refused(Refusal::UnknownItem))?,
    };
    let repo = item
        .github
        .ok_or_else(|| Failure::refused(Refusal::NoUpdateInfo))?;
    let asset = crate::github::latest_with(net, &repo, &github_cache(&ctx.roots), None, now())?;
    let (stage, verified) = fetch_asset(ctx, &asset, consent, false, report, net)?;
    let file = stage
        .0
        .join(format!("{}.AppImage", crate::slug_of(&asset.name)));
    let how = Integration {
        key: Some(key.to_owned()),
        source: Provenance::Github(repo),
        tag: Some(asset.tag.clone()),
        pattern: None,
        verified,
        placement: Placement::Move,
    };
    let s = integrate(ctx, &file, &how, report)?;
    drop(stage);
    Ok(Outcome {
        key: Some(AppKey::AppImage { key: s.key.clone() }),
        desktop_ids: vec![desktop_name(&s.key)],
        ..Outcome::default()
    })
}

/// Bring one integrated AppImage up to its source's latest release. Answers
/// whether it moved.
#[cfg(feature = "net")]
fn upgrade_one(
    ctx: &Context,
    s: &Sidecar,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
) -> Result<bool, Failure> {
    let Provenance::Github(repo) = &s.source else {
        return Err(Failure::refused(Refusal::NoUpdateInfo));
    };
    report.step(Phase::Resolving, 0, 0, &s.name);
    let asset = crate::github::latest_with(
        net,
        repo,
        &github_cache(&ctx.roots),
        s.pattern.as_deref(),
        now(),
    )?;
    let same_tag = s.tag.as_deref() == Some(asset.tag.as_str());
    let same_bytes = asset.sha256().as_deref() == Some(s.sha256.as_str());
    if same_tag || same_bytes {
        if !same_tag {
            // THE FILE IS THE RELEASE, it only did not know its tag (a file the
            // person gave): remember it, so the next check is a comparison.
            let mut known = s.clone();
            known.tag = Some(asset.tag.clone());
            write_sidecar(&ctx.roots, &known)?;
        }
        return Ok(false);
    }
    let consent = Consent {
        big: true,
        ..Consent::default()
    };
    let (stage, verified) = fetch_asset(ctx, &asset, consent, !s.verified, report, net)?;
    let file = stage
        .0
        .join(format!("{}.AppImage", crate::slug_of(&asset.name)));
    let how = Integration {
        key: Some(s.key.clone()),
        source: s.source.clone(),
        tag: Some(asset.tag.clone()),
        pattern: s.pattern.clone(),
        verified,
        placement: Placement::Move,
    };
    integrate(ctx, &file, &how, report)?;
    drop(stage);
    Ok(true)
}

#[cfg(feature = "net")]
fn upgrade(
    ctx: &Context,
    scope: &Scope,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
) -> Done {
    match scope {
        Scope::One {
            key: AppKey::AppImage { key },
        } => {
            let s =
                sidecar(&ctx.roots, key).ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
            let moved = upgrade_one(ctx, &s, report, net)?;
            Ok(Outcome {
                key: Some(AppKey::AppImage { key: key.clone() }),
                amount: Some(u64::from(moved)),
                ..Outcome::default()
            })
        }
        Scope::Source {
            source: Source::AppImage,
        }
        | Scope::Everything => {
            let all: Vec<Sidecar> = sidecars(&ctx.roots)
                .into_iter()
                .filter(|s| matches!(s.source, Provenance::Github(_)))
                .collect();
            let (mut moved, mut troubles) = (0u64, Vec::new());
            for s in &all {
                match upgrade_one(ctx, s, report, net) {
                    Ok(true) => moved += 1,
                    Ok(false) => {}
                    // ONE FAILURE DOES NOT STOP THE REST: an update run that
                    // stopped at the first rate limit would leave every later
                    // AppImage behind for a reason that has nothing to do with it.
                    Err(f) => troubles.push(format!("{}: {f}", s.name)),
                }
            }
            if troubles.is_empty() {
                Ok(Outcome {
                    amount: Some(moved),
                    ..Outcome::default()
                })
            } else {
                Err(Failure {
                    refusal: Some(Refusal::ToolFailed),
                    trouble: troubles.join("\n"),
                })
            }
        }
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! An AppImage made in memory, and a fake `unsquashfs` that "extracts" a
    //! prepared tree — shared by this module's tests and the upgrade tests.

    use std::path::{Path, PathBuf};

    /// A scratch directory, fresh.
    pub(crate) fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-appimage-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An ELF64 AppImage of `kind` (1 or 2), with an update information and a
    /// signature section, and `payload` where the image goes.
    pub(crate) fn elf64(kind: u8, update_info: &str, signed: bool, payload: &[u8]) -> Vec<u8> {
        let names = b"\0.shstrtab\0.upd_info\0.sha256_sig\0";
        let data_at = 64usize;
        let upd_at = data_at + names.len();
        let sig_at = upd_at + 1024;
        let shoff = sig_at + 1024;
        let mut out = vec![0u8; shoff];
        out[..4].copy_from_slice(b"\x7fELF");
        out[4] = 2;
        out[5] = 1;
        out[6] = 1;
        out[8..11].copy_from_slice(&[b'A', b'I', kind]);
        out[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes());
        out[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes());
        out[0x3C..0x3E].copy_from_slice(&4u16.to_le_bytes());
        out[0x3E..0x40].copy_from_slice(&1u16.to_le_bytes());
        out[data_at..data_at + names.len()].copy_from_slice(names);
        out[upd_at..upd_at + update_info.len()].copy_from_slice(update_info.as_bytes());
        if signed {
            out[sig_at] = 0x42;
        }
        let section = |name: u32, at: usize, size: usize| {
            let mut s = vec![0u8; 64];
            s[..4].copy_from_slice(&name.to_le_bytes());
            s[24..32].copy_from_slice(&(at as u64).to_le_bytes());
            s[32..40].copy_from_slice(&(size as u64).to_le_bytes());
            s
        };
        out.extend(vec![0u8; 64]);
        out.extend(section(1, data_at, names.len()));
        out.extend(section(11, upd_at, 1024));
        out.extend(section(21, sig_at, 1024));
        out.extend_from_slice(payload);
        out
    }

    /// A PNG signature and enough after it to be a file.
    pub(crate) const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    /// A prepared image tree like a real AppImage's top level.
    pub(crate) fn image(dir: &Path, name: &str) -> PathBuf {
        let root = dir.join("image");
        let icons = root.join("usr/share/icons/hicolor/256x256/apps");
        std::fs::create_dir_all(&icons).unwrap();
        std::fs::create_dir_all(root.join("usr/share/icons/hicolor/scalable/apps")).unwrap();
        std::fs::write(
            root.join("demo.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName={name}\nName[de]=Demo Betrachter\nComment=Looks at demos\n\
                 Exec=AppRun %U\nIcon=demo\nCategories=Graphics;Viewer;Not A Category;\nMimeType=image/png;bad;\n\
                 StartupWMClass=demo-viewer\nX-AppImage-Version=1.2.3\n[Desktop Action x]\nName=Other\n"
            ),
        )
        .unwrap();
        std::fs::write(icons.join("demo.png"), PNG).unwrap();
        std::fs::write(
            root.join("usr/share/icons/hicolor/scalable/apps/demo.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        )
        .unwrap();
        std::os::unix::fs::symlink(
            "usr/share/icons/hicolor/256x256/apps/demo.png",
            root.join(".DirIcon"),
        )
        .unwrap();
        root
    }

    /// A fake `unsquashfs` that behaves like the real one where it matters:
    /// it logs its arguments, and copies into `-d` only what the patterns
    /// name — globs expanded one component at a time, links copied AS links.
    pub(crate) fn fake_unsquashfs(dir: &Path, image: &Path) -> PathBuf {
        let script = dir.join("fake-unsquashfs");
        let log = dir.join("unsquashfs.log");
        crate::write_executable(
            &script,
            &format!(
                "#!/bin/sh\necho \"$@\" >> '{log}'\ndest=''\nfile=''\n\
                 while [ $# -gt 0 ]; do\n  case \"$1\" in\n    -d) dest=\"$2\"; shift 2;;\n    -o) shift 2;;\n    \
                 -*) shift;;\n    *) if [ -z \"$file\" ]; then file=\"$1\"; shift; else break; fi;;\n  esac\ndone\n\
                 mkdir -p \"$dest\"\ncd '{image}' || exit 1\nfor p in \"$@\"; do\n  for f in $p; do\n    \
                 if [ -e \"$f\" ] || [ -L \"$f\" ]; then cp -a --parents \"$f\" \"$dest/\"; fi\n  done\ndone\n",
                log = log.display(),
                image = image.display()
            ),
        );
        script
    }

    /// A context under `dir` with the fake tool and a stand-in `appimage-run`.
    pub(crate) fn context(dir: &Path, unsquashfs: PathBuf) -> crate::Context {
        crate::Context {
            tools: crate::Tools {
                unsquashfs: Some(unsquashfs),
                appimage_run: Some(PathBuf::from("/bin/sh")),
                ..crate::Tools::default()
            },
            roots: crate::Roots::under(dir),
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::progress::Beat;

    #[test]
    fn the_header_is_read_by_rust_and_every_kind_is_told_apart() {
        let good = elf64(
            2,
            "gh-releases-zsync|o|r|latest|Demo-*x86_64.AppImage.zsync",
            true,
            b"hsqs\0\0rest",
        );
        let h = header_of(std::io::Cursor::new(&good)).unwrap();
        assert_eq!(&good[usize::try_from(h.payload).unwrap()..][..4], b"hsqs");
        assert_eq!(
            h.update_info.as_deref(),
            Some("gh-releases-zsync|o|r|latest|Demo-*x86_64.AppImage.zsync")
        );
        assert!(h.signed);
        let unsigned = header_of(std::io::Cursor::new(elf64(2, "", false, b"hsqs"))).unwrap();
        assert_eq!((unsigned.update_info, unsigned.signed), (None, false));
        assert_eq!(
            header_of(std::io::Cursor::new(elf64(1, "", false, b"hsqs"))),
            Err(Refusal::LegacyAppImage)
        );
        assert_eq!(
            header_of(std::io::Cursor::new(elf64(2, "", false, b"DWARFS"))),
            Err(Refusal::DwarfsAppImage)
        );
        assert_eq!(
            header_of(std::io::Cursor::new(elf64(2, "", false, b"PK\x03\x04"))),
            Err(Refusal::NotAnAppImage)
        );
        let mut plain = good.clone();
        plain[8] = 0;
        assert_eq!(
            header_of(std::io::Cursor::new(plain)),
            Err(Refusal::NotAnAppImage),
            "an ELF that is not one"
        );
        assert_eq!(
            header_of(std::io::Cursor::new(&good[..100])),
            Err(Refusal::NotAnAppImage),
            "cut short"
        );
        assert_eq!(
            header_of(std::io::Cursor::new(b"#!/bin/sh\n".to_vec())),
            Err(Refusal::NotAnAppImage)
        );
    }

    #[test]
    fn update_information_names_a_repository_and_a_pattern() {
        assert_eq!(
            github_of_update_info(
                "gh-releases-zsync|obsidianmd|obsidian-releases|latest|Obsidian-*.AppImage.zsync"
            ),
            Some((
                "obsidianmd/obsidian-releases".to_owned(),
                Some("Obsidian-*.AppImage".to_owned())
            ))
        );
        assert_eq!(
            github_of_update_info("zsync|https://example.org/x.zsync"),
            None
        );
        assert_eq!(
            github_of_update_info("gh-releases-zsync|..|x|latest|y"),
            None
        );
    }

    #[test]
    fn the_extraction_argv_is_quiet_offset_and_literal() {
        let argv = extract_argv(
            188_392,
            Path::new("/s/t"),
            Path::new("/d/x.AppImage"),
            &["*.desktop".to_owned(), ".DirIcon".to_owned()],
        );
        assert_eq!(
            argv,
            [
                "-q",
                "-n",
                "-no-xattrs",
                "-o",
                "188392",
                "-d",
                "/s/t",
                "-f",
                "/d/x.AppImage",
                "*.desktop",
                ".DirIcon"
            ]
        );
        assert!(literal_path("usr/share/icons/hicolor/256x256/apps/x.png"));
        for bad in ["../etc/passwd", "/etc/passwd", "a/-rf", "x*", "a//b", ""] {
            assert!(
                !literal_path(bad),
                "{bad} is not a literal inside the image"
            );
        }
    }

    #[test]
    fn a_link_is_followed_inside_the_staging_root_and_never_out_of_it() {
        let dir = scratch("walk");
        let root = dir.join("root");
        std::fs::create_dir_all(root.join("usr/share")).unwrap();
        std::fs::write(root.join("usr/share/x.png"), PNG).unwrap();
        std::os::unix::fs::symlink("usr/share/x.png", root.join(".DirIcon")).unwrap();
        std::os::unix::fs::symlink("/usr/share/x.png", root.join("absolute")).unwrap();
        std::os::unix::fs::symlink("../../../../etc/passwd", root.join("escape")).unwrap();
        std::os::unix::fs::symlink("loop", root.join("loop")).unwrap();
        std::os::unix::fs::symlink("usr/share/icons/y.png", root.join("later")).unwrap();
        std::os::unix::fs::symlink("/", root.join("usr/host")).unwrap();
        assert_eq!(
            walk(&root, ".DirIcon"),
            Walk::File(root.join("usr/share/x.png"))
        );
        assert_eq!(
            walk(&root, "absolute"),
            Walk::File(root.join("usr/share/x.png")),
            "absolute is the image's root"
        );
        assert_eq!(walk(&root, "escape"), Walk::Refused);
        assert_eq!(walk(&root, "loop"), Walk::Refused);
        assert_eq!(
            walk(&root, "later"),
            Walk::Missing("usr/share/icons/y.png".to_owned())
        );
        assert_eq!(
            walk(&root, "usr/host/etc/passwd"),
            Walk::Missing("etc/passwd".to_owned()),
            "never the machine's"
        );
        assert_eq!(
            walk(&root, "usr"),
            Walk::Refused,
            "a directory is not an icon"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hostile_desktop_cannot_add_a_line_of_its_own() {
        let e = embedded_of("[Desktop Entry]\nName=Evil\u{0}Exec=rm -rf ~\nComment=a\\nExec=rm\nIcon=../../x\nExec=x %F\n").unwrap();
        let text = desktop_entry(
            &e,
            Path::new("/home/p/Applications/evil.AppImage"),
            None,
            "evil",
        )
        .unwrap();
        let execs: Vec<&str> = text.lines().filter(|l| l.starts_with("Exec=")).collect();
        assert_eq!(
            execs,
            [format!(
                "Exec=\"{APPIMAGE_RUN}\" \"/home/p/Applications/evil.AppImage\" %F"
            )],
            "{text}"
        );
        assert!(
            text.lines()
                .all(|l| !l.contains("rm -rf") || l.starts_with("Name=")),
            "{text}"
        );
        assert_eq!(e.icon, None, "a path is not an icon name");
    }

    #[test]
    fn our_desktop_reads_back_as_the_launcher_reads_it() {
        let e = embedded_of(
            &std::fs::read_to_string(image(&scratch("desk"), "Demo Viewer").join("demo.desktop"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            e.categories,
            ["Graphics", "Viewer"],
            "a category with spaces is dropped"
        );
        assert_eq!(e.mime_types, ["image/png"]);
        assert_eq!(e.field_code.as_deref(), Some("%U"));
        assert_eq!(e.names, [("de".to_owned(), "Demo Betrachter".to_owned())]);
        let file = Path::new("/home/a \"b\" $c 100%/Applications/demo.AppImage");
        let text = desktop_entry(&e, file, Some(Path::new("/i/icon-1.png")), "demo").unwrap();
        let fields = ronoctua_apps::desktop::fields(&text);
        let exec = ronoctua_apps::desktop::raw_bytes(&fields, "Exec").unwrap();
        let ctx = ronoctua_apps::exec::Context {
            icon: "",
            name: "",
            file: "",
            open: &["/tmp/p.png".to_owned()],
        };
        let (program, args) = ronoctua_apps::exec::assemble(exec, ctx).unwrap();
        assert_eq!(program, APPIMAGE_RUN);
        assert_eq!(args, [file.display().to_string(), "/tmp/p.png".to_owned()]);
        assert_eq!(
            ronoctua_apps::desktop::raw_bytes(&fields, MARK_KEY),
            Some("demo")
        );
        assert_eq!(
            ronoctua_apps::desktop::raw_bytes(&fields, "TryExec"),
            Some(APPIMAGE_RUN)
        );
        assert_eq!(
            ronoctua_apps::desktop::raw_bytes(&fields, "Name"),
            Some("Demo Viewer")
        );
        assert!(
            desktop_entry(&e, Path::new("/a\\b"), None, "demo").is_err(),
            "a backslash has no one reading"
        );
    }

    #[test]
    fn an_svg_that_reaches_out_is_not_an_icon() {
        assert_eq!(picture_of(PNG), Some(IconKind::Png));
        assert_eq!(
            picture_of(b"<svg><use href=\"#a\"/></svg>"),
            Some(IconKind::Svg)
        );
        assert_eq!(
            picture_of(b"<?xml version=\"1.0\"?><svg><image href=\"/home/p/.ssh/id\"/></svg>"),
            None
        );
        assert_eq!(
            picture_of(b"<svg><image xlink:href='file:///etc/passwd'/></svg>"),
            None
        );
        assert_eq!(picture_of(b"GIF89a"), None);
    }

    #[test]
    fn a_file_is_integrated_copied_and_named_by_its_own_name() {
        let dir = scratch("integrate");
        let image = image(&dir, "Demo Viewer");
        let ctx = context(&dir, fake_unsquashfs(&dir, &image));
        let file = dir.join("Demo-1.2.3-x86_64.AppImage");
        std::fs::write(&file, elf64(2, "", false, b"hsqs....")).unwrap();
        let order = Order::InstallFile {
            path: file.clone(),
            consent: Consent::default(),
        };
        assert_eq!(
            run(&ctx, &order, &mut Vec::<Beat>::new())
                .unwrap_err()
                .refusal,
            Some(Refusal::UnverifiedNeedsConsent)
        );
        let order = Order::InstallFile {
            path: file.clone(),
            consent: Consent {
                unverified: true,
                ..Consent::default()
            },
        };
        let mut beats = Vec::<Beat>::new();
        let done = run(&ctx, &order, &mut beats).unwrap();
        assert_eq!(
            done.key,
            Some(AppKey::AppImage {
                key: "demo-viewer".into()
            })
        );
        assert!(file.exists(), "COPIED, not moved: the person's file stays");
        let placed = ctx.roots.appimages().join("demo-viewer.AppImage");
        assert_eq!(
            std::fs::metadata(&placed).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let s = sidecar(&ctx.roots, "demo-viewer").unwrap();
        assert_eq!(s.sha256, run::sha256_of(&file).unwrap());
        assert_eq!(s.source, Provenance::File);
        let icon = s.icon.unwrap();
        assert_eq!(
            std::fs::read(&icon).unwrap(),
            PNG,
            "the hicolor 256 PNG wins"
        );
        assert!(
            icon.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".png")
        );
        let desktop = std::fs::read_to_string(
            ctx.roots
                .applications()
                .join("ronoctua-appimage-demo-viewer.desktop"),
        )
        .unwrap();
        assert!(desktop.contains(&format!("Icon={}", icon.display())));
        assert!(desktop.contains("StartupWMClass=demo-viewer"));
        let log = std::fs::read_to_string(dir.join("unsquashfs.log")).unwrap();
        assert!(
            log.lines().next().unwrap().ends_with("*.desktop .DirIcon"),
            "{log}"
        );
        assert!(
            log.contains("usr/share/icons/hicolor/*/apps/demo.png"),
            "{log}"
        );
        assert!(
            !ctx.roots
                .ours_cache()
                .join("staging")
                .read_dir()
                .unwrap()
                .any(|_| true),
            "the staging is gone"
        );
        let listed = installed(&ctx);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].version, "1.2.3");
        assert_eq!(listed[0].trouble, None);
        std::fs::remove_file(&placed).unwrap();
        assert_eq!(
            installed(&ctx)[0].trouble,
            Some(Refusal::NotAnAppImage),
            "a vanished file says so"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dir_icon_pointing_elsewhere_is_fetched_by_its_target() {
        let dir = scratch("diricon");
        let image = image(&dir, "Demo");
        // No hicolor PNG and a `.DirIcon` into `pixmaps`: the icon is only
        // reachable by asking for the link's target in a pass of its own.
        std::fs::remove_file(image.join("usr/share/icons/hicolor/256x256/apps/demo.png")).unwrap();
        std::fs::remove_file(image.join(".DirIcon")).unwrap();
        std::fs::create_dir_all(image.join("usr/share/pixmaps")).unwrap();
        std::fs::write(image.join("usr/share/pixmaps/other.png"), PNG).unwrap();
        std::os::unix::fs::symlink("usr/share/pixmaps/other.png", image.join(".DirIcon")).unwrap();
        let ctx = context(&dir, fake_unsquashfs(&dir, &image));
        let file = dir.join("x.AppImage");
        std::fs::write(&file, elf64(2, "", false, b"hsqs")).unwrap();
        let pulled = read_image(&ctx, &file, &header(&file).unwrap()).unwrap();
        assert_eq!(pulled.icon, Some((PNG.to_vec(), IconKind::Png)));
        let log = std::fs::read_to_string(dir.join("unsquashfs.log")).unwrap();
        assert!(
            log.lines()
                .any(|l| l.ends_with(" usr/share/pixmaps/other.png")),
            "{log}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_already_in_applications_is_adopted_by_moving() {
        let dir = scratch("adopt");
        let image = image(&dir, "Demo");
        let ctx = context(&dir, fake_unsquashfs(&dir, &image));
        std::fs::create_dir_all(ctx.roots.appimages()).unwrap();
        let file = ctx.roots.appimages().join("Demo-9.AppImage");
        std::fs::write(
            &file,
            elf64(
                2,
                "gh-releases-zsync|o|demo|latest|Demo-*.AppImage.zsync",
                false,
                b"hsqs",
            ),
        )
        .unwrap();
        assert_eq!(strays(&ctx.roots), std::slice::from_ref(&file));
        let order = Order::InstallFile {
            path: file.clone(),
            consent: Consent {
                unverified: true,
                ..Consent::default()
            },
        };
        run(&ctx, &order, &mut Vec::<Beat>::new()).unwrap();
        assert!(!file.exists(), "moved into our name");
        assert!(strays(&ctx.roots).is_empty());
        let s = sidecar(&ctx.roots, "demo").unwrap();
        assert_eq!(
            s.source,
            Provenance::Github("o/demo".into()),
            "its own update information names its releases"
        );
        assert_eq!(s.pattern.as_deref(), Some("Demo-*.AppImage"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn removing_takes_everything_and_the_extraction_too() {
        let dir = scratch("remove");
        let image = image(&dir, "Demo");
        let ctx = context(&dir, fake_unsquashfs(&dir, &image));
        let file = dir.join("x.AppImage");
        std::fs::write(&file, elf64(2, "", false, b"hsqs")).unwrap();
        let consent = Consent {
            unverified: true,
            ..Consent::default()
        };
        run(
            &ctx,
            &Order::InstallFile {
                path: file.clone(),
                consent,
            },
            &mut Vec::<Beat>::new(),
        )
        .unwrap();
        let s = sidecar(&ctx.roots, "demo").unwrap();
        // What appimage-run leaves, with a read-only directory inside.
        let ex = extraction(&ctx.roots, &s.sha256).unwrap();
        std::fs::create_dir_all(ex.join("usr/lib")).unwrap();
        std::fs::write(ex.join("usr/lib/big"), vec![0u8; 1000]).unwrap();
        std::fs::set_permissions(ex.join("usr"), std::fs::Permissions::from_mode(0o555)).unwrap();
        let done = run(
            &ctx,
            &Order::Remove {
                key: AppKey::AppImage { key: "demo".into() },
                keep_data: true,
            },
            &mut Vec::<Beat>::new(),
        )
        .unwrap();
        assert!(done.amount.unwrap() >= 1000);
        assert!(!ex.exists() && !s.file.exists());
        assert!(!ctx.roots.applications().join(desktop_name("demo")).exists());
        assert!(!sidecars_dir(&ctx.roots).join("demo").exists());
        assert!(file.exists(), "the person's own copy is never touched");
        let unknown = Order::Remove {
            key: AppKey::AppImage { key: "demo".into() },
            keep_data: true,
        };
        assert_eq!(
            run(&ctx, &unknown, &mut Vec::<Beat>::new())
                .unwrap_err()
                .refusal,
            Some(Refusal::UnknownItem)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sidecar_pointing_outside_applications_deletes_nothing_there() {
        let dir = scratch("guard");
        let roots = Roots::under(&dir);
        let victim = dir.join("precious.AppImage");
        std::fs::write(&victim, b"mine").unwrap();
        let s = Sidecar {
            key: "evil".into(),
            name: "Evil".into(),
            file: victim.clone(),
            sha256: "0".repeat(64),
            update_info: None,
            source: Provenance::File,
            tag: None,
            installed: 0,
            version: None,
            pattern: None,
            icon: None,
            verified: false,
        };
        write_sidecar(&roots, &s).unwrap();
        remove(&roots, "evil").unwrap();
        assert!(victim.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tidy_takes_only_old_extractions_nobody_owns() {
        let dir = scratch("tidy");
        let roots = Roots::under(&dir);
        let cache = roots.cache.join("appimage-run");
        let owned = "a".repeat(64);
        let orphan = "b".repeat(64);
        for d in [&owned, &orphan, &"not-a-hash".to_owned()] {
            std::fs::create_dir_all(cache.join(d)).unwrap();
            std::fs::write(cache.join(d).join("AppRun"), vec![1u8; 10]).unwrap();
        }
        let s = Sidecar {
            key: "x".into(),
            name: "X".into(),
            file: roots.appimages().join("x.AppImage"),
            sha256: owned.clone(),
            update_info: None,
            source: Provenance::File,
            tag: None,
            installed: 0,
            version: None,
            pattern: None,
            icon: None,
            verified: false,
        };
        write_sidecar(&roots, &s).unwrap();
        assert_eq!(tidy(&roots, now()), 0, "an orphan younger than a day stays");
        assert_eq!(tidy(&roots, now() + GRACE + 10), 10);
        assert!(
            cache.join(&owned).exists()
                && !cache.join(&orphan).exists()
                && cache.join("not-a-hash").exists()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rehearsal_changes_nothing() {
        let dir = scratch("rehearse");
        let mut ctx = context(&dir, dir.join("never"));
        ctx.rehearse = true;
        let order = Order::InstallFile {
            path: dir.join("x.AppImage"),
            consent: Consent::default(),
        };
        assert!(
            run(&ctx, &order, &mut Vec::<Beat>::new())
                .unwrap()
                .rehearsed
        );
        assert!(!ctx.roots.appimages().exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "net")]
    mod from_the_network {
        use super::*;
        use crate::github::canned::Canned;

        const LATEST: &str = "https://api.github.com/repos/audacity/audacity/releases/latest";
        const ASSET: &str =
            "https://github.com/audacity/audacity/releases/download/v/Demo-x86_64.AppImage";

        fn release(tag: &str, size: usize, digest: Option<&str>) -> Vec<u8> {
            serde_json::to_vec(&serde_json::json!({
                "tag_name": tag, "draft": false, "prerelease": false,
                "assets": [
                    {"name": "Demo-aarch64.AppImage", "browser_download_url": "https://github.com/a/b/c", "size": 1, "digest": null},
                    {"name": "Demo-x86_64.AppImage", "browser_download_url": ASSET, "size": size, "digest": digest},
                ]
            }))
            .unwrap()
        }

        /// A machine with the catalogue already fetched, and the fake tools.
        fn machine(tag: &str) -> (PathBuf, Context) {
            let dir = scratch(tag);
            let image = image(&dir, "Demo Viewer");
            let ctx = context(&dir, fake_unsquashfs(&dir, &image));
            let feed = std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/appimage/feed-cut.json"),
            )
            .unwrap();
            let kept = crate::appimagehub::kept_at(&ctx.roots);
            std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
            std::fs::write(&kept, feed).unwrap();
            (dir, ctx)
        }

        fn install(ctx: &Context, net: &Canned, consent: Consent) -> Done {
            let order = Order::Install {
                key: AppKey::AppImage {
                    key: "audacity".into(),
                },
                consent,
            };
            run_with(ctx, &order, &mut Vec::<Beat>::new(), net)
        }

        #[test]
        fn a_catalogue_install_is_verified_and_keeps_the_catalogues_key() {
            let (dir, ctx) = machine("net-install");
            let bytes = elf64(2, "", false, b"hsqs-one");
            let digest = format!("sha256:{}", run::sha256(&bytes));
            let net = Canned::new(vec![(
                LATEST,
                200,
                vec![],
                release("v1", bytes.len(), Some(&digest)),
            )])
            .serving(ASSET, bytes.clone());
            let done = install(&ctx, &net, Consent::default()).unwrap();
            assert_eq!(
                done.key,
                Some(AppKey::AppImage {
                    key: "audacity".into()
                }),
                "the row's key, not the embedded name's"
            );
            let s = sidecar(&ctx.roots, "audacity").unwrap();
            assert_eq!(
                (s.source.clone(), s.tag.as_deref(), s.verified),
                (
                    Provenance::Github("audacity/audacity".into()),
                    Some("v1"),
                    true
                )
            );
            assert_eq!(
                std::fs::read(ctx.roots.appimages().join("audacity.AppImage")).unwrap(),
                bytes
            );
            assert_eq!(
                net.fetched.borrow().as_slice(),
                [ASSET],
                "never the other machine's file"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn a_digest_that_does_not_match_leaves_nothing_behind() {
            let (dir, ctx) = machine("net-integrity");
            let bytes = elf64(2, "", false, b"hsqs-one");
            let wrong = format!("sha256:{}", "0".repeat(64));
            let net = Canned::new(vec![(
                LATEST,
                200,
                vec![],
                release("v1", bytes.len(), Some(&wrong)),
            )])
            .serving(ASSET, bytes);
            assert_eq!(
                install(&ctx, &net, Consent::default()).unwrap_err().refusal,
                Some(Refusal::Integrity)
            );
            assert!(!ctx.roots.appimages().join("audacity.AppImage").exists());
            assert!(sidecar(&ctx.roots, "audacity").is_none());
            assert!(
                !ctx.roots
                    .ours_cache()
                    .join("staging")
                    .read_dir()
                    .unwrap()
                    .any(|_| true)
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn no_digest_is_asked_about_before_a_byte_is_fetched() {
            let (dir, ctx) = machine("net-unverified");
            let bytes = elf64(2, "", false, b"hsqs");
            let net = Canned::new(vec![(
                LATEST,
                200,
                vec![],
                release("v1", bytes.len(), None),
            )])
            .serving(ASSET, bytes);
            assert_eq!(
                install(&ctx, &net, Consent::default()).unwrap_err().refusal,
                Some(Refusal::UnverifiedNeedsConsent)
            );
            assert!(net.fetched.borrow().is_empty());
            // Asked and answered: it is kept, and it says it was not verified.
            let net = Canned::default().serving(ASSET, elf64(2, "", false, b"hsqs"));
            install(
                &ctx,
                &net,
                Consent {
                    unverified: true,
                    ..Consent::default()
                },
            )
            .unwrap();
            assert!(!sidecar(&ctx.roots, "audacity").unwrap().verified);
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn an_upgrade_swaps_the_file_and_drops_the_old_extraction() {
            let (dir, ctx) = machine("net-upgrade");
            let one = elf64(2, "", false, b"hsqs-one");
            let net = Canned::new(vec![(
                LATEST,
                200,
                vec![],
                release(
                    "v1",
                    one.len(),
                    Some(&format!("sha256:{}", run::sha256(&one))),
                ),
            )])
            .serving(ASSET, one);
            install(&ctx, &net, Consent::default()).unwrap();
            let old = sidecar(&ctx.roots, "audacity").unwrap();
            let ex = extraction(&ctx.roots, &old.sha256).unwrap();
            std::fs::create_dir_all(&ex).unwrap();
            let two = elf64(2, "", false, b"hsqs-two");
            // The kept answer is fresh; make it stale so the release is asked again.
            let kept = crate::github::kept(&github_cache(&ctx.roots), "audacity/audacity").unwrap();
            let stale = crate::github::Kept { when: 0, ..kept };
            let path = github_cache(&ctx.roots).join(format!(
                "v{}-audacity__audacity.json",
                crate::github::FORMAT
            ));
            std::fs::write(&path, serde_json::to_vec(&stale).unwrap()).unwrap();
            let net = Canned::new(vec![(
                LATEST,
                200,
                vec![],
                release(
                    "v2",
                    two.len(),
                    Some(&format!("sha256:{}", run::sha256(&two))),
                ),
            )])
            .serving(ASSET, two.clone());
            let order = Order::Upgrade {
                scope: Scope::Source {
                    source: Source::AppImage,
                },
            };
            assert_eq!(
                run_with(&ctx, &order, &mut Vec::<Beat>::new(), &net)
                    .unwrap()
                    .amount,
                Some(1)
            );
            let new = sidecar(&ctx.roots, "audacity").unwrap();
            assert_eq!(new.tag.as_deref(), Some("v2"));
            assert_eq!(std::fs::read(&new.file).unwrap(), two);
            assert!(!ex.exists(), "the old extraction is gone");
            // Up to date: nothing is downloaded (the answer is fresh now).
            let none = Canned::default();
            assert_eq!(
                run_with(&ctx, &order, &mut Vec::<Beat>::new(), &none)
                    .unwrap()
                    .amount,
                Some(0)
            );
            assert!(none.fetched.borrow().is_empty() && none.asked.borrow().is_empty());
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn a_file_with_no_source_has_no_update() {
            let dir = scratch("net-nosource");
            let image = image(&dir, "Demo");
            let ctx = context(&dir, fake_unsquashfs(&dir, &image));
            let file = dir.join("x.AppImage");
            std::fs::write(&file, elf64(2, "", false, b"hsqs")).unwrap();
            let consent = Consent {
                unverified: true,
                ..Consent::default()
            };
            run(
                &ctx,
                &Order::InstallFile {
                    path: file,
                    consent,
                },
                &mut Vec::<Beat>::new(),
            )
            .unwrap();
            let order = Order::Upgrade {
                scope: Scope::One {
                    key: AppKey::AppImage { key: "demo".into() },
                },
            };
            let f =
                run_with(&ctx, &order, &mut Vec::<Beat>::new(), &Canned::default()).unwrap_err();
            assert_eq!(f.refusal, Some(Refusal::NoUpdateInfo));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// A REAL AppImage, integrated by the real `unsquashfs` into a scratch
    /// home, with our `.desktop` printed. Ignored: it needs the file and the
    /// program. Run it with `RONOCTUA_APPIMAGE=<file> RONOCTUA_UNSQUASHFS=<store
    /// path> cargo test … -- --ignored --nocapture a_real_appimage`.
    #[test]
    #[ignore = "needs a real AppImage and the machine's unsquashfs"]
    fn a_real_appimage_is_integrated() {
        let (Some(file), Some(unsquashfs)) = (
            std::env::var_os("RONOCTUA_APPIMAGE").map(PathBuf::from),
            std::env::var_os("RONOCTUA_UNSQUASHFS").map(PathBuf::from),
        ) else {
            return;
        };
        let dir = scratch("real-appimage");
        let ctx = context(&dir, unsquashfs);
        let head = header(&file).unwrap();
        println!(
            "payload at {}, update information {:?}, signed {}",
            head.payload, head.update_info, head.signed
        );
        let consent = Consent {
            unverified: true,
            ..Consent::default()
        };
        let done = run(
            &ctx,
            &Order::InstallFile {
                path: file,
                consent,
            },
            &mut Vec::<Beat>::new(),
        )
        .unwrap();
        let Some(AppKey::AppImage { key }) = done.key else {
            panic!("no key")
        };
        let s = sidecar(&ctx.roots, &key).unwrap();
        println!("sidecar: {s:?}");
        println!(
            "{}",
            std::fs::read_to_string(ctx.roots.applications().join(desktop_name(&key))).unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The REAL `unsquashfs` against a real squashfs image (the fixture,
    /// 4 KiB, made with `mksquashfs` from files written for it). Ignored: a
    /// test starts no program of the machine. Run it with
    /// `RONOCTUA_UNSQUASHFS=<store path> cargo test … -- --ignored`.
    #[test]
    #[ignore = "runs the machine's unsquashfs"]
    fn the_real_unsquashfs_honours_the_patterns() {
        let Some(unsquashfs) = std::env::var_os("RONOCTUA_UNSQUASHFS").map(PathBuf::from) else {
            return;
        };
        let dir = scratch("real");
        let payload = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/appimage/payload.squashfs"),
        )
        .unwrap();
        let file = dir.join("Demo.AppImage");
        std::fs::write(&file, elf64(2, "", false, &payload)).unwrap();
        let ctx = context(&dir, unsquashfs);
        let head = header(&file).unwrap();
        let read = read_image(&ctx, &file, &head).unwrap();
        let e = read.embedded.unwrap();
        assert_eq!(e.name, "Demo Viewer");
        let (bytes, kind) = read.icon.unwrap();
        assert_eq!(kind, IconKind::Png);
        assert!(bytes.starts_with(b"\x89PNG"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
