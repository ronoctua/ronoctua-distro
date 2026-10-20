//! VS Code's files and its CLI — the one crate that talks to them
//! (AGENTS.md's boundary table).
//!
//! # What VS Code keeps, and what travels
//!
//! VS Code installs extensions under `~/.vscode/extensions` and lists them in
//! its manifest there ([`MANIFEST_IN_HOME`]): this machine's install, which
//! never travels — the folder is other people's code, fetched again from the
//! Marketplace. What travels is the person's configuration
//! (`~/.config/Code/User`, the belongings' target `vscode`), and in it ONE
//! file of ours: the list of the Marketplace extensions the person uses
//! ([`LIST_IN_HOME`]), `{"format": 1, "extensions": ["publisher.name", …],
//! "pre-release": [...]}`, sorted, lowercase, written whole and atomically.
//! It follows the DEFAULT profile only: a profile's own extensions are VS
//! Code's to keep.
//!
//! # The rule ([`recorded`])
//!
//! `list' = (list − (seen − installed)) ∪ (installed − seen)`, and `seen' =
//! installed` — `seen` being what this machine saw installed the last time
//! ([`SEEN_FILE`], under the state root: it never travels). Only what CHANGED
//! here moves the list:
//!
//! - an extension installed here since it last looked enters;
//! - one uninstalled here since it last looked leaves;
//! - and nothing else — another machine's id stays in the list though this
//!   machine never had it, and an id another machine took out stays out
//!   though this machine still has it installed. So an uninstall travels:
//!   another machine's copy never puts it back.
//!
//! A machine with no manifest yet (VS Code never ran, nothing was installed)
//! writes nothing ([`Recorded::NothingHere`]). Only what came from the
//! Marketplace is recorded: an extension installed from a file has no id
//! another machine could install by.
//!
//! # The list is a set, merged as one ([`merge`])
//!
//! Two machines that change their extensions between two syncs change the
//! list on both sides; the belongings merge it three ways — the base, plus
//! what either side added, minus what either side took out — and never ask.
//!
//! # Installing what is missing ([`install::install`])
//!
//! The ids of the list that are missing here, one at a time, by `code
//! --install-extension` — VS Code's own `code`, by its store path
//! ([`VARIABLE`]). It reaches the Marketplace's hosts ([`hosts_of`]), so it
//! runs only with a consent `Pass` for every one of them, minted at the
//! moment it asks.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ronoctua_store::Store;
use serde::{Deserialize, Serialize};

pub mod install;

/// The format of the list this build writes and reads; a list of a newer
/// one is refused ([`Why::Newer`]), never rewritten by an older machine.
pub const FORMAT: u64 = 1;

/// The person's list, relative to the home: inside `~/.config/Code/User`,
/// so it travels with the belongings' target `vscode`.
pub const LIST_IN_HOME: &str = ".config/Code/User/ronoctua-extensions.json";

/// VS Code's folder of installed extensions, relative to the home — what
/// `code` is told to install into (`--extensions-dir`), so the manifest read
/// is the one written.
pub const EXTENSIONS_IN_HOME: &str = ".vscode/extensions";

/// VS Code's manifest of the extensions installed here, relative to the
/// home — the default profile's, the file the user path unit watches.
pub const MANIFEST_IN_HOME: &str = ".vscode/extensions/extensions.json";

/// What this machine saw installed, under the state root: it never travels.
pub const SEEN_FILE: &str = "vscode-seen.json";

/// The variable naming VS Code's `code` by its store path, set where VS
/// Code is installed (`nix/modules/vscode.nix`).
pub const VARIABLE: &str = "RONOCTUA_VSCODE";

/// The hosts every install asks, read in the pinned build's `product.json`
/// (`extensionsGallery`): the Marketplace's service — the query and each
/// extension's latest version —, the fallback of the latest version, and the
/// control list of malicious and deprecated extensions.
pub const GALLERY_HOSTS: [&str; 3] = [
    "marketplace.visualstudio.com",
    "www.vscode-unpkg.net",
    "main.vscode-cdn.net",
];

/// How many times [`record`] reads the manifest again when it changed
/// while the list was being written — VS Code writes it in place, several
/// times for one install, and the path unit does not watch while its
/// service runs.
pub const RECORD_TRIES: usize = 5;

/// Why a file or a run was refused, as a closed set — a door says each in
/// its own words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// A file is there and could not be read.
    Unreadable,
    /// A file is there and is not what it should be — the list, or VS Code's
    /// manifest.
    Malformed,
    /// The list was written by a newer format: never rewritten here.
    Newer,
    /// The list could not be written.
    Unwritable,
    /// VS Code's `code` is not on this machine ([`VARIABLE`] names none, or
    /// nothing is there).
    NoCode,
    /// No home to read from.
    NoHome,
    /// A host the install would reach was not let in the person's
    /// `consent.json` ([`Refusal::hosts`] says which): nothing was asked.
    NotLet,
    /// Another install of the extensions runs now.
    Busy,
}

impl Why {
    /// Every one, for the census of a door's sentences.
    pub const ALL: [Self; 8] = [
        Self::Unreadable,
        Self::Malformed,
        Self::Newer,
        Self::Unwritable,
        Self::NoCode,
        Self::NoHome,
        Self::NotLet,
        Self::Busy,
    ];

    /// Its word, for a command line and a journal.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::Malformed => "malformed",
            Self::Newer => "newer",
            Self::Unwritable => "unwritable",
            Self::NoCode => "no-code",
            Self::NoHome => "no-home",
            Self::NotLet => "not-let",
            Self::Busy => "busy",
        }
    }
}

/// A refusal, the file it is about when there is one, and the hosts not let
/// when that is why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// Why.
    pub why: Why,
    /// The file it is about.
    pub at: Option<PathBuf>,
    /// The hosts that were not let ([`Why::NotLet`]), sorted.
    pub hosts: Vec<String>,
}

impl Refusal {
    /// `why`, about `at`.
    #[must_use]
    pub fn about(why: Why, at: &Path) -> Self {
        Self {
            why,
            at: Some(at.to_path_buf()),
            hosts: Vec::new(),
        }
    }

    /// `why`, about no file.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self {
            why,
            at: None,
            hosts: Vec::new(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.why.id())?;
        if let Some(at) = &self.at {
            write!(f, ": {}", at.display())?;
        }
        if !self.hosts.is_empty() {
            write!(f, ": {}", self.hosts.join(" "))?;
        }
        Ok(())
    }
}

/// `code`, by the store path [`VARIABLE`] names — `None` where VS Code is
/// not installed.
#[must_use]
pub fn code() -> Option<PathBuf> {
    std::env::var_os(VARIABLE)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Whether `id` is an extension's id as VS Code's own pattern has it,
/// lowercase — `publisher.name`, each half letters, digits and `-`, starting
/// with a letter or a digit (`EXTENSION_IDENTIFIER_PATTERN`) — and nothing
/// `code --install-extension` would read as something else: it takes an
/// argument ending in `.vsix` for a file on disk, so the name `vsix` is
/// refused (and a leading `-`, a flag, already is). Pure.
#[must_use]
pub fn is_extension_id(id: &str) -> bool {
    let half = |s: &str| {
        s.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    match id.split_once('.') {
        Some((publisher, name)) => {
            id.len() <= 256 && half(publisher) && half(name) && name != "vsix"
        }
        None => false,
    }
}

/// `id` as the list keeps it — lowercase, as VS Code compares ids — or
/// `None` when it is not an extension's id ([`is_extension_id`]). Pure.
#[must_use]
pub fn canonical(id: &str) -> Option<String> {
    let lower = id.to_ascii_lowercase();
    is_extension_id(&lower).then_some(lower)
}

/// The publisher of an extension's id: its first half. Pure.
#[must_use]
pub fn publisher_of(id: &str) -> Option<&str> {
    id.split_once('.').map(|(p, _)| p)
}

/// The hosts an install of `ids` reaches: [`GALLERY_HOSTS`], and for each
/// publisher its asset host (`<publisher>.gallery.vsassets.io`, which the
/// package is asked of) and the CDN it redirects to
/// (`<publisher>.gallerycdn.vsassets.io`) — sorted, each once. Pure.
#[must_use]
pub fn hosts_of<'a>(ids: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut hosts: BTreeSet<String> = GALLERY_HOSTS.iter().map(|h| (*h).to_owned()).collect();
    for id in ids {
        if let Some(publisher) = canonical(id).as_deref().and_then(publisher_of) {
            hosts.insert(format!("{publisher}.gallery.vsassets.io"));
            hosts.insert(format!("{publisher}.gallerycdn.vsassets.io"));
        }
    }
    hosts.into_iter().collect()
}

/// A set of extensions as the list and the manifest carry it: the ids, and
/// those of them installed as pre-releases.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extensions {
    /// The ids, lowercase.
    pub ids: BTreeSet<String>,
    /// The ids installed (or to be installed) as pre-releases — always a
    /// part of [`Extensions::ids`].
    pub pre_release: BTreeSet<String>,
}

impl Extensions {
    /// The set, its pre-releases cut to its ids.
    #[must_use]
    pub fn of(ids: BTreeSet<String>, pre_release: BTreeSet<String>) -> Self {
        let pre_release = pre_release.intersection(&ids).cloned().collect();
        Self { ids, pre_release }
    }
}

/// THE RULE on one set: `list − (seen − installed) ∪ (installed − seen)` —
/// only what changed here moves the list. Pure.
#[must_use]
pub fn moved(
    list: &BTreeSet<String>,
    seen: &BTreeSet<String>,
    installed: &BTreeSet<String>,
) -> BTreeSet<String> {
    let gone: BTreeSet<&String> = seen.difference(installed).collect();
    let new: Vec<&String> = installed.difference(seen).collect();
    list.iter()
        .filter(|id| !gone.contains(id))
        .chain(new)
        .cloned()
        .collect()
}

/// THE RULE ([`moved`]) on the ids and on the pre-releases: the list after
/// this machine's install was seen. Pure.
#[must_use]
pub fn recorded(list: &Extensions, seen: &Extensions, installed: &Extensions) -> Extensions {
    Extensions::of(
        moved(&list.ids, &seen.ids, &installed.ids),
        moved(&list.pre_release, &seen.pre_release, &installed.pre_release),
    )
}

/// What VS Code's manifest says the Marketplace installed here — `None`
/// when there is no manifest (nothing installed here yet). An entry that
/// names no extension's id, or that did not come from the Marketplace
/// (`metadata.source` other than `gallery`; VS Code itself takes an entry
/// with no source and a Marketplace uuid for one), is left out: it is VS
/// Code's file, and one odd entry is no reason to read none.
///
/// # Errors
///
/// [`Why::Unreadable`], and [`Why::Malformed`] for a manifest that is not a
/// JSON array.
pub fn installed(home: &Path) -> Result<Option<Extensions>, Refusal> {
    let at = home.join(MANIFEST_IN_HOME);
    let bytes = match std::fs::read(&at) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Refusal::about(Why::Unreadable, &at)),
    };
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&bytes).map_err(|_| Refusal::about(Why::Malformed, &at))?;
    let mut out = Extensions::default();
    for e in &entries {
        let Some(id) = e
            .pointer("/identifier/id")
            .and_then(serde_json::Value::as_str)
            .and_then(canonical)
        else {
            continue;
        };
        let source = e
            .pointer("/metadata/source")
            .and_then(serde_json::Value::as_str);
        let has_uuid = e
            .pointer("/identifier/uuid")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|u| !u.is_empty());
        let from_the_gallery = match source {
            Some(s) => s == "gallery",
            None => has_uuid,
        };
        if !from_the_gallery {
            continue;
        }
        if e.pointer("/metadata/preRelease")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            out.pre_release.insert(id.clone());
        }
        out.ids.insert(id);
    }
    Ok(Some(out))
}

/// The list as it is written: the format first, then the ids.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    format: u64,
    extensions: Vec<String>,
    #[serde(rename = "pre-release", default, skip_serializing_if = "Vec::is_empty")]
    pre_release: Vec<String>,
}

/// Where the person's list is, under `home`.
#[must_use]
pub fn list_path(home: &Path) -> PathBuf {
    home.join(LIST_IN_HOME)
}

/// A list's bytes, read: the extensions, or why not — for the person's file
/// and for the three sides a merge is handed. Pure.
///
/// # Errors
///
/// [`Why::Newer`] for a list of a newer format, [`Why::Malformed`] for one
/// that is not this format's object or names something that is not an
/// extension's id. The refusal is about no file: the caller says which.
pub fn parse_list(bytes: &[u8]) -> Result<Extensions, Refusal> {
    let malformed = || Refusal::of(Why::Malformed);
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| malformed())?;
    // THE FORMAT FIRST: a newer one says so before its fields are judged.
    match value.get("format").and_then(serde_json::Value::as_u64) {
        Some(format) if format > FORMAT => return Err(Refusal::of(Why::Newer)),
        Some(FORMAT) => {}
        _ => return Err(malformed()),
    }
    let written: Written = serde_json::from_value(value).map_err(|_| malformed())?;
    let read = |ids: &[String]| -> Result<BTreeSet<String>, Refusal> {
        ids.iter()
            .map(|id| canonical(id).ok_or_else(malformed))
            .collect()
    };
    Ok(Extensions::of(
        read(&written.extensions)?,
        read(&written.pre_release)?,
    ))
}

/// The person's list under `home` — empty when there is none.
///
/// # Errors
///
/// [`Why::Unreadable`], and [`parse_list`]'s — the list is written only by
/// [`record`] and the belongings' merge, so a list that does not read is
/// somebody's hand, and it is never overwritten blind.
pub fn read_list(home: &Path) -> Result<Extensions, Refusal> {
    let at = list_path(home);
    match std::fs::read(&at) {
        Ok(bytes) => parse_list(&bytes).map_err(|r| Refusal::about(r.why, &at)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Extensions::default()),
        Err(_) => Err(Refusal::about(Why::Unreadable, &at)),
    }
}

/// The list's bytes: pretty, one id a line, sorted. Pure.
#[must_use]
pub fn list_bytes(list: &Extensions) -> Vec<u8> {
    let written = Written {
        format: FORMAT,
        extensions: list.ids.iter().cloned().collect(),
        pre_release: list.pre_release.iter().cloned().collect(),
    };
    let mut text = serde_json::to_string_pretty(&written).unwrap_or_default();
    text.push('\n');
    text.into_bytes()
}

/// One set three ways: the base, plus what either side added, minus what
/// either side took out. Pure.
#[must_use]
pub fn three_way(
    base: &BTreeSet<String>,
    ours: &BTreeSet<String>,
    theirs: &BTreeSet<String>,
) -> BTreeSet<String> {
    let removed =
        |side: &BTreeSet<String>| -> BTreeSet<String> { base.difference(side).cloned().collect() };
    let (gone_ours, gone_theirs) = (removed(ours), removed(theirs));
    base.iter()
        .chain(ours.difference(base))
        .chain(theirs.difference(base))
        .filter(|id| !gone_ours.contains(*id) && !gone_theirs.contains(*id))
        .cloned()
        .collect()
}

/// THE LIST'S MERGE, for the belongings: three versions of the list joined
/// as sets ([`three_way`], on the ids and on the pre-releases) — never a
/// conflict. `None` when one of the three does not read as this format's
/// list ([`parse_list`]): the caller then merges it by its general rules.
/// Pure.
#[must_use]
pub fn merge(base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
    let (b, o, t) = (
        parse_list(base).ok()?,
        parse_list(ours).ok()?,
        parse_list(theirs).ok()?,
    );
    let joined = Extensions::of(
        three_way(&b.ids, &o.ids, &t.ids),
        three_way(&b.pre_release, &o.pre_release, &t.pre_release),
    );
    Some(list_bytes(&joined))
}

/// Writes the person's list under `home`, whole and atomically.
fn write_list(home: &Path, list: &Extensions) -> Result<(), Refusal> {
    let at = list_path(home);
    let (Some(dir), Some(name)) = (at.parent(), at.file_name().and_then(|n| n.to_str())) else {
        return Err(Refusal::about(Why::Unwritable, &at));
    };
    ronoctua_xdg::write_atomically(dir, name, &list_bytes(list))
        .map_err(|_| Refusal::about(Why::Unwritable, &at))
}

/// What this machine saw installed, the last time it looked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    /// The ids, sorted.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Those of them installed as pre-releases.
    #[serde(default, rename = "pre-release")]
    pub pre_release: Vec<String>,
}

impl Seen {
    /// What was seen, as a set — leniently: an id that does not read is
    /// nothing seen, the safe way, which takes nothing out of the list.
    fn set(&self) -> Extensions {
        let read = |ids: &[String]| ids.iter().filter_map(|id| canonical(id)).collect();
        Extensions::of(read(&self.extensions), read(&self.pre_release))
    }

    /// A set, as it is kept.
    fn of(set: &Extensions) -> Self {
        Self {
            extensions: set.ids.iter().cloned().collect(),
            pre_release: set.pre_release.iter().cloned().collect(),
        }
    }
}

/// The store of what this machine saw installed, under this instance's
/// state root.
#[must_use]
pub fn seen_store() -> Store<Seen> {
    Store::at(ronoctua_xdg::state_dir(), SEEN_FILE)
}

/// What [`record`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// No manifest here: nothing was written.
    NothingHere,
    /// Nothing changed here since the last look.
    Unchanged,
    /// The list was written, with this many extensions.
    Written(usize),
}

/// Records this machine's install into the person's list under `home`
/// (the rule, [`recorded`]): the list first, and only then what was seen —
/// so a list that could not be written leaves `seen` as it was, and the
/// change is recorded at the next run. Then the manifest is read AGAIN: VS
/// Code writes it in place, several times for one install, and a write
/// during this run is one the path unit did not see (it does not watch
/// while its service runs) — so the record goes round until the manifest
/// holds still, at most [`RECORD_TRIES`] times.
///
/// # Errors
///
/// A manifest or a list that does not read ([`installed`], [`read_list`]):
/// nothing is written; [`Why::Unwritable`].
pub fn record(home: &Path, seen: &Store<Seen>) -> Result<Recorded, Refusal> {
    let mut wrote = None;
    let mut installed_now = match installed(home)? {
        None => return Ok(Recorded::NothingHere),
        Some(now) => now,
    };
    for _ in 0..RECORD_TRIES {
        let list = read_list(home)?;
        let before = seen.read().set();
        let next = recorded(&list, &before, &installed_now);
        if next != list {
            write_list(home, &next)?;
            wrote = Some(next.ids.len());
        }
        if before != installed_now {
            let _ = seen.write(&Seen::of(&installed_now));
        }
        match installed(home)? {
            Some(again) if again != installed_now => installed_now = again,
            _ => break,
        }
    }
    Ok(wrote.map_or(Recorded::Unchanged, Recorded::Written))
}

/// The extensions of the person's list under `home` that are not installed
/// here — every one of them when there is no manifest — with the
/// pre-releases among them.
///
/// # Errors
///
/// A list or a manifest that does not read ([`read_list`], [`installed`]).
pub fn missing(home: &Path) -> Result<Extensions, Refusal> {
    let list = read_list(home)?;
    let here = installed(home)?.unwrap_or_default();
    Ok(Extensions::of(
        list.ids.difference(&here.ids).cloned().collect(),
        list.pre_release,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| (*s).to_owned()).collect()
    }

    pub(crate) fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-vscode-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// One entry of VS Code's manifest, shaped as VS Code writes it.
    pub(crate) fn entry(
        home: &Path,
        id: &str,
        source: &str,
        pre_release: bool,
    ) -> serde_json::Value {
        let folder = format!("{id}-1.0.0");
        serde_json::json!({
            "identifier": { "id": id, "uuid": "8d4c2c27-0000-4000-8000-000000000000" },
            "version": "1.0.0",
            "location": {
                "$mid": 1,
                "fsPath": home.join(EXTENSIONS_IN_HOME).join(&folder),
                "path": home.join(EXTENSIONS_IN_HOME).join(&folder),
                "scheme": "file"
            },
            "relativeLocation": folder,
            "metadata": {
                "installedTimestamp": 1_790_000_000_000_u64,
                "pinned": false,
                "source": source,
                "id": "8d4c2c27-0000-4000-8000-000000000000",
                "publisherId": "00000000-0000-4000-8000-000000000001",
                "publisherDisplayName": "someone",
                "targetPlatform": "undefined",
                "updated": false,
                "private": false,
                "isPreReleaseVersion": pre_release,
                "hasPreReleaseVersion": pre_release,
                "preRelease": pre_release
            }
        })
    }

    /// VS Code's manifest under `home`, written IN PLACE as VS Code writes
    /// it — Marketplace extensions, `(id, pre-release)`.
    pub(crate) fn manifest(home: &Path, ids: &[(&str, bool)]) {
        let entries: Vec<serde_json::Value> = ids
            .iter()
            .map(|(id, pre)| entry(home, id, "gallery", *pre))
            .collect();
        let at = home.join(MANIFEST_IN_HOME);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, serde_json::to_vec(&entries).unwrap()).unwrap();
    }

    fn seen_in(dir: &Path) -> Store<Seen> {
        Store::at(Some(dir.join("state")), SEEN_FILE).with_roots(None, Some(dir.join("state")))
    }

    /// One case of the rule: the list, what was seen, what is installed,
    /// the list wanted, and what it says.
    type Case<'a> = (
        &'a [&'a str],
        &'a [&'a str],
        &'a [&'a str],
        &'a [&'a str],
        &'a str,
    );

    /// FAILS IF: an extension installed here since the last look does not
    /// enter the list, one uninstalled here does not leave it, another
    /// machine's id leaves it, an id another machine took out comes back
    /// because this machine still has it (the uninstall would never stick),
    /// or one this machine saw and still has moves.
    #[test]
    fn only_what_changed_here_moves_the_list() {
        let cases: [Case<'_>; 7] = [
            (&[], &[], &["a.x"], &["a.x"], "an install here enters"),
            (
                &["a.x", "b.y"],
                &["a.x", "b.y"],
                &["b.y"],
                &["b.y"],
                "an uninstall here leaves",
            ),
            (
                &["a.x", "far.away"],
                &["a.x"],
                &["a.x"],
                &["a.x", "far.away"],
                "another machine's stays",
            ),
            (
                &["far.away"],
                &[],
                &[],
                &["far.away"],
                "an empty install takes out nothing unseen",
            ),
            (
                &["a.x"],
                &["a.x"],
                &["a.x", "c.z"],
                &["a.x", "c.z"],
                "what is still here stays",
            ),
            (
                &["a.x"],
                &["a.x", "z.z"],
                &["a.x", "z.z"],
                &["a.x"],
                "an id taken out elsewhere stays out",
            ),
            (
                &["a.x", "b.y"],
                &["a.x", "b.y"],
                &[],
                &[],
                "everything seen and gone leaves",
            ),
        ];
        for (list, seen, installed, want, what) in cases {
            assert_eq!(
                moved(&set(list), &set(seen), &set(installed)),
                set(want),
                "{what}"
            );
        }
    }

    /// FAILS IF: the reviewer's two machines re-add an id one of them
    /// uninstalled: A uninstalls `z.z`, its list loses it, the list travels
    /// to B, which still has `z.z`, and B's record puts it back — so A
    /// would be offered it again, for ever.
    #[test]
    fn an_uninstall_travels_and_is_never_put_back() {
        let (a, b) = (scratch("sim-a"), scratch("sim-b"));
        let (seen_a, seen_b) = (seen_in(&a), seen_in(&b));
        manifest(&a, &[("a.x", false), ("z.z", false)]);
        manifest(&b, &[("a.x", false), ("z.z", false), ("b.y", false)]);
        record(&a, &seen_a).unwrap();
        // B RECEIVES A's LIST, records its own.
        std::fs::create_dir_all(list_path(&b).parent().unwrap()).unwrap();
        std::fs::copy(list_path(&a), list_path(&b)).unwrap();
        record(&b, &seen_b).unwrap();
        assert_eq!(read_list(&b).unwrap().ids, set(&["a.x", "b.y", "z.z"]));
        // A RECEIVES IT BACK, then uninstalls `z.z`.
        std::fs::copy(list_path(&b), list_path(&a)).unwrap();
        record(&a, &seen_a).unwrap();
        manifest(&a, &[("a.x", false)]);
        record(&a, &seen_a).unwrap();
        assert_eq!(read_list(&a).unwrap().ids, set(&["a.x", "b.y"]));
        // B RECEIVES IT, still with `z.z` installed: the list stays without.
        std::fs::copy(list_path(&a), list_path(&b)).unwrap();
        assert_eq!(record(&b, &seen_b), Ok(Recorded::Unchanged));
        assert_eq!(read_list(&b).unwrap().ids, set(&["a.x", "b.y"]));
        // AND A IS NOT OFFERED IT AGAIN.
        std::fs::copy(list_path(&b), list_path(&a)).unwrap();
        assert!(!missing(&a).unwrap().ids.contains("z.z"));
        let _ = std::fs::remove_dir_all(a);
        let _ = std::fs::remove_dir_all(b);
    }

    /// FAILS IF: an id `code` would read as a flag, a path or a file is
    /// taken, VS Code's own pattern is not the one judged by (an
    /// underscore), a plain id is refused, or the list keeps another casing
    /// than the lowercase VS Code compares by.
    #[test]
    fn only_an_extensions_id_is_one() {
        for good in [
            "ms-python.python",
            "rust-lang.rust-analyzer",
            "a.b",
            "pub.myvsix",
            "pub.vsix-viewer",
        ] {
            assert!(is_extension_id(good), "{good}");
        }
        for bad in [
            "",
            "nodot",
            "-flag.name",
            "pub.-name",
            "pub.name.more",
            "../x.y",
            "pub/x.y",
            "pub.vsix",
            "x_y.z",
            "pub.name with space",
            "pub.",
            ".name",
            "Pub.Name",
        ] {
            assert!(!is_extension_id(bad), "{bad:?} was taken");
        }
        assert_eq!(
            canonical("GitHub.Copilot").as_deref(),
            Some("github.copilot")
        );
        assert_eq!(canonical("Pub.VSIX"), None);
        assert_eq!(canonical("--install-extension"), None);
    }

    /// FAILS IF: the hosts of an install are not the gallery's three and,
    /// per publisher, its asset host and CDN — each once, whatever the
    /// casing or the number of its extensions.
    #[test]
    fn the_hosts_are_the_gallerys_and_each_publishers() {
        assert_eq!(
            hosts_of([
                "Rust-Lang.rust-analyzer",
                "rust-lang.other",
                "ms-python.python",
                "not an id"
            ]),
            [
                "main.vscode-cdn.net",
                "marketplace.visualstudio.com",
                "ms-python.gallery.vsassets.io",
                "ms-python.gallerycdn.vsassets.io",
                "rust-lang.gallery.vsassets.io",
                "rust-lang.gallerycdn.vsassets.io",
                "www.vscode-unpkg.net",
            ]
        );
        assert_eq!(hosts_of([]).len(), GALLERY_HOSTS.len());
    }

    /// FAILS IF: a refusal's word on a command line is not the one a door
    /// reads back from the record the belongings write, or one is said
    /// twice.
    #[test]
    fn a_refusal_has_one_word() {
        let mut words = BTreeSet::new();
        for why in Why::ALL {
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
            assert!(words.insert(why.id()), "{why:?} said twice");
        }
    }

    /// FAILS IF: a machine with no manifest writes anything; a recorded
    /// list is not sorted, lowercase and readable back; an extension
    /// installed from a file (no Marketplace id to install it by) enters it;
    /// a pre-release is not kept as one; or a second run with nothing new
    /// writes again.
    #[test]
    fn record_follows_the_manifest_and_keeps_the_rest() {
        let home = scratch("record");
        let seen = seen_in(&home);
        assert_eq!(record(&home, &seen), Ok(Recorded::NothingHere));
        assert!(
            !list_path(&home).exists(),
            "a machine with nothing wrote a list"
        );
        assert!(seen.path().is_some_and(|p| !p.exists()));

        let mut entries = vec![
            entry(&home, "Rust-Lang.rust-analyzer", "gallery", true),
            entry(&home, "ms-python.python", "gallery", false),
            entry(&home, "local.hand-made", "vsix", false),
        ];
        // A GALLERY ENTRY WITH NO SOURCE, AND ITS UUID: VS Code's own reading.
        let mut old = entry(&home, "old.one", "gallery", false);
        old["metadata"].as_object_mut().unwrap().remove("source");
        entries.push(old);
        let at = home.join(MANIFEST_IN_HOME);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, serde_json::to_vec(&entries).unwrap()).unwrap();
        assert_eq!(record(&home, &seen), Ok(Recorded::Written(3)));
        let list = read_list(&home).unwrap();
        assert_eq!(
            list.ids,
            set(&["ms-python.python", "old.one", "rust-lang.rust-analyzer"])
        );
        assert_eq!(list.pre_release, set(&["rust-lang.rust-analyzer"]));
        let text = std::fs::read_to_string(list_path(&home)).unwrap();
        assert!(text.starts_with("{\n  \"format\": 1,"), "{text}");
        assert!(text.ends_with("]\n}\n"), "{text}");
        assert_eq!(record(&home, &seen), Ok(Recorded::Unchanged));
        let drafts: Vec<_> = std::fs::read_dir(list_path(&home).parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".new"))
            .collect();
        assert!(drafts.is_empty(), "a draft was left: {drafts:?}");
        let _ = std::fs::remove_dir_all(home);
    }

    /// FAILS IF: a list or a manifest that does not read is written over, or
    /// what was seen moves past it — a hand-edited list, one with an id
    /// `code` would misread, one of a newer format, a manifest that is not
    /// VS Code's array.
    #[test]
    fn what_does_not_read_is_never_written_over() {
        let home = scratch("refuse");
        let seen = seen_in(&home);
        manifest(&home, &[("a.x", false)]);
        let list = list_path(&home);
        std::fs::create_dir_all(list.parent().unwrap()).unwrap();
        for (text, why) in [
            (
                "{\"format\": 2, \"extensions\": [], \"more\": true}",
                Why::Newer,
            ),
            (
                "{\"format\": 1, \"extensions\": [\"-x.y\"]}",
                Why::Malformed,
            ),
            (
                "{\"format\": 1, \"extensions\": [\"pub.vsix\"]}",
                Why::Malformed,
            ),
            (
                "{\"format\": 1, \"extensions\": [], \"stray\": 1}",
                Why::Malformed,
            ),
            ("{\"extensions\": []}", Why::Malformed),
            ("not json", Why::Malformed),
        ] {
            std::fs::write(&list, text).unwrap();
            assert_eq!(record(&home, &seen).map_err(|r| r.why), Err(why), "{text}");
            assert_eq!(std::fs::read_to_string(&list).unwrap(), text);
            assert!(
                seen.path().is_some_and(|p| !p.exists()),
                "seen moved past {text}"
            );
        }
        std::fs::remove_file(&list).unwrap();
        std::fs::write(home.join(MANIFEST_IN_HOME), "{\"not\": \"an array\"}").unwrap();
        assert_eq!(record(&home, &seen).map_err(|r| r.why), Err(Why::Malformed));
        assert!(!list.exists());
        let _ = std::fs::remove_dir_all(home);
    }

    /// FAILS IF: what is missing here is not the list minus the manifest —
    /// every id when VS Code never ran here — or forgets which of them are
    /// pre-releases.
    #[test]
    fn missing_is_the_list_minus_what_is_here() {
        let home = scratch("missing");
        assert!(missing(&home).unwrap().ids.is_empty());
        std::fs::create_dir_all(list_path(&home).parent().unwrap()).unwrap();
        let list = Extensions::of(set(&["a.x", "b.y", "c.z"]), set(&["c.z"]));
        std::fs::write(list_path(&home), list_bytes(&list)).unwrap();
        assert_eq!(missing(&home).unwrap(), list);
        manifest(&home, &[("B.y", false)]);
        assert_eq!(
            missing(&home).unwrap(),
            Extensions::of(set(&["a.x", "c.z"]), set(&["c.z"]))
        );
        let _ = std::fs::remove_dir_all(home);
    }

    /// FAILS IF: two machines' changes to the list are not joined as sets —
    /// an addition on either side lost, a removal on either side undone by
    /// the other side's copy, a pre-release lost — or a list that does not
    /// read is joined at all.
    #[test]
    fn the_list_merges_as_a_set() {
        let bytes = |ids: &[&str], pre: &[&str]| list_bytes(&Extensions::of(set(ids), set(pre)));
        let base = bytes(&["a.x", "b.y", "z.z"], &[]);
        let ours = bytes(&["a.x", "b.y", "n.ours"], &["n.ours"]);
        let theirs = bytes(&["a.x", "z.z", "n.theirs"], &[]);
        let joined = parse_list(&merge(&base, &ours, &theirs).unwrap()).unwrap();
        assert_eq!(joined.ids, set(&["a.x", "n.ours", "n.theirs"]));
        assert_eq!(joined.pre_release, set(&["n.ours"]));
        assert_eq!(merge(&base, &ours, b"not json"), None);
        assert_eq!(
            three_way(&set(&["a"]), &set(&["a"]), &set(&["a"])),
            set(&["a"])
        );
        assert_eq!(
            three_way(&set(&[]), &set(&["a"]), &set(&["a"])),
            set(&["a"])
        );
    }
}
