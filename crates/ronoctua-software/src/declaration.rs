//! The person's declaration — what they declared and declined — as this
//! crate reads it, writes it and says how this machine stands to it.
//!
//! # Where it lives, and who writes it
//!
//! ```text
//! $XDG_STATE_HOME/ronoctua/person/          the MIRROR (this crate and the belongings write it)
//!   software/declared.json                  the person's programs, every machine of theirs
//!   software/declined.json                  the distro's programs they keep uninstalled
//!   software/icons/<sha256>.png             the pictures an entry names, by content
//! ```
//!
//! The mirror is the person's own copy: the belongings carry it to their
//! repository (`plain/person/`) and back, merged by id, and root's applier
//! copies a STAGED tree of it into the machine directory ([`crate::stage`]),
//! which is the only copy Nix reads. Every document is read and written in
//! `ronoctua-person`'s types and its one canonical form (`to_json`), so a
//! rewrite of an unchanged list changes nothing in git and root keeps the
//! bytes it is given.
//!
//! # Changes are made to the file as it is NOW
//!
//! The queue runs orders of different sources at once, and the belongings
//! may land a merge at any time: a change reads the document, changes ONE
//! entry and writes it back, under the declaration's lane
//! ([`crate::lock::take_declaration`]), and a document that does not read here
//! — a newer format, a broken file — is never written over
//! ([`Refusal::DeclarationUnreadable`]): the person's other machine wrote it,
//! and this one would erase what it does not understand.
//!
//! # How this machine stands to it
//!
//! [`Declarations`] is what the screen draws: every declared entry with its
//! [`Standing`] (applied, waiting for the next apply, on other machines only,
//! or skipped and why — `/etc/ronoctua/person-applied.json`), every decline
//! with what it did, and the distro's programs on this machine
//! (`distro-apps.json`). It is read from files only, cheap enough for every
//! reading of the machine ([`crate::state::read`]).

use std::path::{Path, PathBuf};

use ronoctua_person::declared::{Declared, Entry};
use ronoctua_person::declined::{Declined, Declining};
use ronoctua_person::reports::{DeclaredAndroid, DistroApp, DistroApps, PersonApplied, SkipWhy};
use ronoctua_person::tree::caps;
use ronoctua_person::{Document, Stamp, names};
use serde::{Deserialize, Serialize};

use crate::found::Picture;
use crate::state::{Installed, Origin};
use crate::{AppKey, Failure, Refusal, Roots, Source};

/// The person's mirror: `$XDG_STATE_HOME/ronoctua[-dev]/person`.
#[must_use]
pub fn mirror(roots: &Roots) -> PathBuf {
    roots.state.join(ronoctua_xdg::name()).join(names::MIRROR)
}

/// The manifest the person signed of their mirror and its signature, where
/// the belongings keep it: `$XDG_STATE_HOME/ronoctua[-dev]/person-signed`,
/// beside the mirror (`ronoctua_person::names::SIGNED`).
#[must_use]
pub fn signed(roots: &Roots) -> PathBuf {
    roots.state.join(ronoctua_xdg::name()).join(names::SIGNED)
}

/// The person's brain modules, where they write them:
/// `$XDG_CONFIG_HOME/ronoctua[-dev]/brain-modules` — the folder's name is
/// the staged tree's (`ronoctua_person::tree::BRAIN_MODULES`), as the
/// repository's `plain/brain-modules` is.
#[must_use]
pub fn brain_modules(roots: &Roots) -> PathBuf {
    roots
        .config
        .join(ronoctua_xdg::name())
        .join(ronoctua_person::tree::BRAIN_MODULES)
}

/// This machine's name, as the kernel has it (`uname`): what a declaration
/// says it was edited on and what an entry's `machines` names.
#[must_use]
pub fn this_host() -> String {
    rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned()
}

/// The machine's name as a declaration can write it, or the refusal.
///
/// # Errors
///
/// [`Refusal::MachineUnnamed`] for a hostname that is not a lower-case RFC
/// 1123 label — Nix compares it with `networking.hostName` byte for byte, so
/// a mended one would name another machine.
pub fn host_of(host: &str) -> Result<String, Failure> {
    if ronoctua_person::is_hostname(host) {
        Ok(host.to_owned())
    } else {
        Err(Failure {
            refusal: Some(Refusal::MachineUnnamed),
            trouble: format!("this machine is called {host:?}"),
        })
    }
}

/// Now, as the files write it.
#[must_use]
pub fn now_stamp() -> Stamp {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Stamp::from_unix_s(s)
}

/// Who edited an entry, and when: the two fields every entry carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// `edited-at`.
    pub at: Stamp,
    /// `edited-on`: this machine's name.
    pub on: String,
}

/// The id a key is declared under (`<source>:<source-id>`); `None` for what
/// the flake installed.
#[must_use]
pub fn id_of(key: &AppKey) -> Option<String> {
    key.source().map(|s| format!("{}:{}", s.word(), key.id()))
}

/// The key an id names (`<source>:<source-id>`, [`id_of`]'s inverse);
/// `None` for one that names no source.
#[must_use]
pub fn key_of_id(id: &str) -> Option<AppKey> {
    let (word, rest) = id.split_once(':')?;
    if rest.is_empty() {
        return None;
    }
    let rest = rest.to_owned();
    Some(match Source::of_word(word)? {
        Source::Nix => AppKey::Nix { attr: rest },
        Source::Flatpak => AppKey::Flatpak { id: rest },
        Source::AppImage => AppKey::AppImage { key: rest },
        Source::Android => AppKey::Android { package: rest },
        Source::Web => AppKey::Web { slug: rest },
    })
}

/// The key an entry declares.
#[must_use]
pub fn key_of(entry: &Entry) -> AppKey {
    match entry {
        Entry::Nix(a) => AppKey::Nix {
            attr: a.attr.clone(),
        },
        Entry::Flatpak(a) => AppKey::Flatpak {
            id: a.app_id.clone(),
        },
        Entry::AppImage(a) => AppKey::AppImage { key: a.key.clone() },
        Entry::Web(a) => AppKey::Web {
            slug: a.slug.clone(),
        },
        Entry::Android(a) => AppKey::Android {
            package: a.package.clone(),
        },
    }
}

/// The crate's source of an entry.
#[must_use]
pub const fn source_of(entry: &Entry) -> Source {
    match entry {
        Entry::Nix(_) => Source::Nix,
        Entry::Flatpak(_) => Source::Flatpak,
        Entry::AppImage(_) => Source::AppImage,
        Entry::Web(_) => Source::Web,
        Entry::Android(_) => Source::Android,
    }
}

/// The name an entry shows (data): an AppImage's or a web app's own, the
/// identifier otherwise.
#[must_use]
pub fn name_of(entry: &Entry) -> String {
    match entry {
        Entry::AppImage(a) => a.name.clone(),
        Entry::Web(a) => a.name.clone(),
        other => other.source_id().to_owned(),
    }
}

/// The pin of an entry that has one: its version, its address, and when it
/// was taken (the AppImage's).
#[must_use]
pub fn pin_of(entry: &Entry) -> Option<(String, String, Option<Stamp>)> {
    match entry {
        Entry::AppImage(a) => Some((
            a.pin.version.clone(),
            a.pin.url.clone(),
            Some(a.pin.pinned_at.clone()),
        )),
        Entry::Android(a) => Some((a.pin.version.clone(), a.pin.url.clone(), None)),
        _ => None,
    }
}

// ─── the mirror's documents ─────────────────────────────────────────────────

/// One of the mirror's two documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum File {
    /// `software/declared.json`.
    Declared,
    /// `software/declined.json`.
    Declined,
}

impl File {
    /// Its path under the mirror.
    #[must_use]
    pub const fn rel(self) -> &'static str {
        match self {
            Self::Declared => names::DECLARED,
            Self::Declined => names::DECLINED,
        }
    }

    /// Where it is in the person's mirror under `roots` — the declines by
    /// the person's data's own spelling (`ronoctua_person::mirror_declined`),
    /// the one the installer seeds them at.
    #[must_use]
    pub fn path(self, roots: &Roots) -> PathBuf {
        match self {
            Self::Declined => {
                ronoctua_person::mirror_declined(&roots.state.join(ronoctua_xdg::name()))
            }
            Self::Declared => mirror(roots).join(self.rel()),
        }
    }

    /// The most root reads of it — and so the most it may be here.
    #[must_use]
    pub const fn most_bytes(self) -> u64 {
        match self {
            Self::Declared => caps::DECLARED_MOST_BYTES,
            Self::Declined => caps::DECLINED_MOST_BYTES,
        }
    }
}

/// A document of the mirror that does not read here, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Unreadable {
    /// Which.
    pub file: File,
    /// Why (`newer`: a newer machine of the person's wrote it).
    pub why: ronoctua_person::Why,
}

/// The bytes of a file, at most `most` of them; `None` when it is not there.
///
/// # Errors
///
/// The person crate's `Invalid` for a file over its cap (root would refuse
/// it whole), `NotJson` for one that cannot be read.
fn bytes_of(path: &Path, most: u64) -> Result<Option<Vec<u8>>, ronoctua_person::Why> {
    use std::io::Read as _;
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ronoctua_person::Why::NotJson),
    };
    let mut bytes = Vec::new();
    file.take(most + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ronoctua_person::Why::NotJson)?;
    if bytes.len() as u64 > most {
        return Err(ronoctua_person::Why::Invalid);
    }
    Ok(Some(bytes))
}

/// A document of the mirror: its default when it is not there yet.
fn read_doc<T: Document + Default>(roots: &Roots, file: File) -> Result<T, ronoctua_person::Why> {
    match bytes_of(&file.path(roots), file.most_bytes())? {
        None => Ok(T::default()),
        Some(bytes) => ronoctua_person::parse(&bytes).map_err(|r| r.why),
    }
}

/// `declared.json`, as it is now.
///
/// # Errors
///
/// Why it does not read here.
pub fn read_declared(roots: &Roots) -> Result<Declared, ronoctua_person::Why> {
    read_doc(roots, File::Declared)
}

/// `declined.json`, as it is now.
///
/// # Errors
///
/// Why it does not read here.
pub fn read_declined(roots: &Roots) -> Result<Declined, ronoctua_person::Why> {
    read_doc(roots, File::Declined)
}

/// Write a document of the mirror, whole and atomically, in its canonical
/// form. The folder is made when it is missing.
fn write_doc<T: Serialize>(roots: &Roots, file: File, doc: &T) -> Result<(), Failure> {
    let path = file.path(roots);
    let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(Failure::trouble(format!(
            "{} is not a file",
            path.display()
        )));
    };
    let text = ronoctua_person::to_json(doc);
    std::fs::create_dir_all(folder)
        .and_then(|()| {
            ronoctua_xdg::write_atomically(folder, &name.to_string_lossy(), text.as_bytes())
        })
        .map_err(|e| Failure::trouble(format!("could not write {}: {e}", path.display())))
}

/// The refusal of a document that does not read and would be written over
/// — or that an order would have to read to know what to do.
#[must_use]
pub fn unreadable(file: File, why: ronoctua_person::Why) -> Failure {
    Failure {
        refusal: Some(Refusal::DeclarationUnreadable),
        trouble: format!("{}: {}", file.rel(), why.id()),
    }
}

/// How many times a change is made again over a document that moved
/// between its reading and its writing.
pub const CHANGE_TRIES: usize = 5;

/// Change one document of the mirror as it is on disk NOW, and write it back
/// validated — only while the file still holds what the change was made
/// over.
///
/// The declaration's lane orders this crate's orders among themselves; the
/// belongings' plain pull lands a merge without it. So the file is read
/// again just before it is replaced, and a file that moved meanwhile (a
/// merge brought another computer's entry) is changed again from what it
/// holds now, up to [`CHANGE_TRIES`] times — `what` is run once per try.
/// The belongings do the same from their side (they never write over a file
/// that no longer holds what they read). What remains is the instant between
/// the second reading and the rename, which only a lock both take would
/// close (`docs/PENDING.md`).
fn change_doc<T: Document + Default>(
    roots: &Roots,
    file: File,
    mut what: impl FnMut(&mut T) -> Result<(), Failure>,
) -> Result<T, Failure> {
    let path = file.path(roots);
    for _ in 0..CHANGE_TRIES {
        let before = bytes_of(&path, file.most_bytes()).map_err(|why| unreadable(file, why))?;
        let mut doc: T = match &before {
            None => T::default(),
            Some(bytes) => ronoctua_person::parse(bytes).map_err(|r| unreadable(file, r.why))?,
        };
        what(&mut doc)?;
        doc.validate().map_err(|r| Failure {
            refusal: Some(Refusal::DeclarationRefused),
            trouble: r.to_string(),
        })?;
        // STILL WHAT WAS READ, or the change is made again over what is there.
        if bytes_of(&path, file.most_bytes()).ok() != Some(before) {
            continue;
        }
        write_doc(roots, file, &doc)?;
        return Ok(doc);
    }
    Err(Failure::trouble(format!(
        "{} changed under every one of {CHANGE_TRIES} changes",
        file.rel()
    )))
}

/// Change `declared.json` as it is on disk NOW — the caller holds the
/// declaration's lane — and write it back, validated ([`change_doc`]: `what`
/// runs again when the file moved under it). Answers the document as
/// written.
///
/// # Errors
///
/// [`Refusal::DeclarationUnreadable`] for a file that does not read here,
/// `what`'s own refusal, [`Refusal::DeclarationRefused`] when the result
/// would not validate (root would refuse it the same), and a failed write.
pub fn change_declared(
    roots: &Roots,
    what: impl FnMut(&mut Declared) -> Result<(), Failure>,
) -> Result<Declared, Failure> {
    change_doc(roots, File::Declared, what)
}

/// [`change_declared`], for `declined.json`.
///
/// # Errors
///
/// As [`change_declared`].
pub fn change_declined(
    roots: &Roots,
    what: impl FnMut(&mut Declined) -> Result<(), Failure>,
) -> Result<Declined, Failure> {
    change_doc(roots, File::Declined, what)
}

/// `entry` in the list: in the place of the one with its id, or at the end.
/// Answers whether anything changed beyond the stamps (a declaration of
/// what is already declared, the same way, is no edit of it).
pub fn upsert(doc: &mut Declared, entry: Entry) -> bool {
    match doc.apps.iter_mut().find(|e| e.id() == entry.id()) {
        Some(e) if e.settled() == entry.settled() && same_pin(e, &entry) => false,
        Some(e) => {
            *e = entry;
            true
        }
        None => {
            doc.apps.push(entry);
            true
        }
    }
}

/// Whether two entries carry the same download (`settled` sets the pin
/// aside: it is what a later edit may move alone).
fn same_pin(a: &Entry, b: &Entry) -> bool {
    match (a, b) {
        (Entry::AppImage(x), Entry::AppImage(y)) => {
            x.pin.url == y.pin.url && x.pin.hash == y.pin.hash
        }
        (Entry::Android(x), Entry::Android(y)) => x.pin == y.pin,
        _ => true,
    }
}

/// The list without the entry `id`; whether it was there.
pub fn without(doc: &mut Declared, id: &str) -> bool {
    let before = doc.apps.len();
    doc.apps.retain(|e| e.id() != id);
    doc.apps.len() != before
}

/// Keep `png` among the mirror's pictures, named by its SHA-256, and answer
/// that name — `None` for what root would refuse: not a PNG, or over its cap
/// (a declaration without a picture is a declaration; one root refuses is
/// none).
#[must_use]
pub fn keep_icon(roots: &Roots, png: &[u8]) -> Option<String> {
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") || png.len() as u64 > caps::ICON_MOST_BYTES {
        return None;
    }
    let sha = crate::run::sha256(png);
    let folder = mirror(roots).join(names::ICONS);
    let name = format!("{sha}.png");
    if folder.join(&name).is_file() {
        return Some(sha);
    }
    std::fs::create_dir_all(&folder)
        .and_then(|()| ronoctua_xdg::write_atomically(&folder, &name, png))
        .map_err(|e| tracing::warn!(error = %e, "an icon of the declaration was not kept"))
        .ok()?;
    Some(sha)
}

/// The picture a file is, when it is a PNG root would keep.
#[must_use]
pub fn icon_of_file(roots: &Roots, path: &Path) -> Option<String> {
    let len = std::fs::metadata(path).ok()?.len();
    if len > caps::ICON_MOST_BYTES {
        return None;
    }
    keep_icon(roots, &std::fs::read(path).ok()?)
}

/// Where a kept picture is.
#[must_use]
pub fn icon_path(roots: &Roots, sha: &str) -> PathBuf {
    mirror(roots).join(names::ICONS).join(format!("{sha}.png"))
}

// ─── what Nix wrote back ────────────────────────────────────────────────────

/// One of Nix's reports, when it is there and reads; anything else is
/// "nothing known" (and a line in the journal).
fn report<T: Document>(roots: &Roots, name: &str) -> Option<T> {
    report_read(roots, name).ok().flatten()
}

/// A report of the system, `None` when it is not there — or why it does
/// not read: `NotJson` for one that cannot be read at all.
fn report_read<T: Document>(roots: &Roots, name: &str) -> Result<Option<T>, ronoctua_person::Why> {
    let path = roots.system.reports.join(name);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            tracing::warn!(error = %e, file = %path.display(), "a report of the system could not be read");
            return Err(ronoctua_person::Why::NotJson);
        }
    };
    ronoctua_person::parse(&bytes)
        .map(Some)
        .map_err(|r| {
            tracing::warn!(refusal = %r, file = %path.display(), "a report of the system does not read");
            r.why
        })
}

/// The system that waits for the NEXT RESTART, when one does — read off the
/// system profile and the system that runs ([`ronoctua_machine::next_boot`]),
/// never off the machine's last apply: a trial that failed, a rollback or a
/// later apply that built nothing would each make that one wrong.
fn waiting_system(roots: &Roots) -> Option<String> {
    let state = crate::stage::machine_state(&roots.system.machine);
    ronoctua_machine::next_boot(&roots.system.root, state.as_ref()).map(|n| n.toplevel)
}

/// The report `name` of the system `toplevel`, read inside it, where its
/// `/etc` keeps it; `None` when it is not there or does not read.
fn report_of<T: Document>(roots: &Roots, toplevel: &str, name: &str) -> Option<T> {
    let file = roots
        .system
        .root
        .join(toplevel.trim_start_matches('/'))
        .join(crate::REPORTS.trim_start_matches('/'))
        .join(name);
    let bytes = std::fs::read(&file).ok()?;
    ronoctua_person::parse(&bytes).ok()
}

/// The `person-applied.json` of the system that waits for the NEXT RESTART
/// ([`waiting_system`]), when one does.
#[must_use]
pub fn staged_applied(roots: &Roots) -> Option<PersonApplied> {
    report_of(roots, &waiting_system(roots)?, names::PERSON_APPLIED)
}

/// `person-applied.json`: what this system applied, skipped, and did with
/// the declines.
#[must_use]
pub fn person_applied(roots: &Roots) -> Option<PersonApplied> {
    report(roots, names::PERSON_APPLIED)
}

/// `distro-apps.json`: the distro's programs on this machine.
#[must_use]
pub fn distro_apps(roots: &Roots) -> Option<DistroApps> {
    report(roots, names::DISTRO_APPS)
}

/// `declared-android.json`: the apks this machine's Android is to have.
#[must_use]
pub fn declared_android(roots: &Roots) -> Option<DeclaredAndroid> {
    report(roots, names::DECLARED_ANDROID)
}

// ─── how this machine stands ────────────────────────────────────────────────

/// How this machine stands to one declared entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "standing"
)]
pub enum Standing {
    /// In this machine's system — and, for a part of the person's, its part
    /// arrived.
    Applied,
    /// A part of the person's that the system took and that has not
    /// arrived: on its way, or failed — said on the parts' page, with the
    /// last thing its try said (data, English) when it failed.
    Arriving {
        /// What the part's last try said, when it failed.
        failure: Option<String>,
    },
    /// Declared since the system was built: the next apply takes it (the
    /// update, or the rebuild a removal starts).
    Waiting,
    /// In the system the machine's last apply set for the NEXT BOOT, and not
    /// in the one that runs: it is here from the next restart.
    NextRestart,
    /// Declared on the person's other machines only.
    Elsewhere,
    /// Left out by this machine, and why (`person-applied.json`).
    Skipped {
        /// The reason, from the closed set.
        reason: SkipWhy,
        /// What Nix said, or the hold's reason (data, English).
        detail: String,
    },
}

/// One declared entry, and how this machine stands to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeclaredRow {
    /// The entry, as the mirror has it.
    pub entry: Entry,
    /// How this machine stands to it.
    pub standing: Standing,
}

/// What one decline did on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "state"
)]
pub enum DeclineState {
    /// The program left this machine.
    Honoured,
    /// It left the distro's list and is still here, through another program
    /// that brings it.
    StillPresent,
    /// The distro needs it, and it stays — the reason's id
    /// (`software-essential-why-<reason>`).
    Essential {
        /// The reason's id.
        reason: String,
    },
    /// The distro no longer knows the id (a program it stopped carrying).
    Unknown,
    /// Declined on the person's other machines only.
    Elsewhere,
    /// Declined since the system was built: the next apply takes it.
    Waiting,
    /// Declined since the system was built, and the system that waits for
    /// the next restart already leaves it out: it leaves at that restart.
    NextRestart,
}

/// One decline, named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeclinedRow {
    /// The decline, as the mirror has it.
    pub declining: Declining,
    /// The program's name, as the system's launcher entry says it, else its
    /// id (data).
    pub name: String,
    /// What it did here.
    pub state: DeclineState,
}

/// One of the distro's programs, named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DistroRow {
    /// The registry's entry, as `distro-apps.json` has it.
    pub app: DistroApp,
    /// Its name: its launcher entry's, else the registry's, else its id
    /// ([`distro_name`]; data).
    pub name: String,
}

/// The person's declaration and how this machine stands to it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Declarations {
    /// This machine's name.
    pub host: String,
    /// What the person declared, in the file's order.
    pub declared: Vec<DeclaredRow>,
    /// What the person declined.
    pub declined: Vec<DeclinedRow>,
    /// The distro's programs, on this machine or not.
    pub distro: Vec<DistroRow>,
    /// The mirror's documents that do not read here.
    pub unreadable: Vec<Unreadable>,
    /// Why this system's list of the distro's programs
    /// (`/etc/ronoctua/distro-apps.json`) does not read, when it does not —
    /// `NotJson` too for one that is not there: none of the distro's
    /// programs is known then, and the screen says so instead of listing
    /// none.
    #[serde(default)]
    pub registry_unreadable: Option<ronoctua_person::Why>,
    /// Whether this system says what it applied (`person-applied.json`): a
    /// machine whose system predates the declaration says nothing, and every
    /// entry waits.
    pub reported: bool,
    /// The declared Android apps this machine is to have and does not yet,
    /// whose part arrived: they go in while Android runs
    /// ([`crate::reconcile`]).
    pub android_pending: Vec<String>,
    /// What the system this machine runs applied and the declaration no
    /// longer names, in the report's order: removed for good — here while
    /// the update held the machine, or on another computer — and still
    /// here until this machine's next apply builds a system without it.
    #[serde(default)]
    pub leaving: Vec<String>,
    /// Whether a system waits for the next restart ([`staged_applied`]):
    /// what it brings in or takes away is here from then, and the page says
    /// so.
    #[serde(default)]
    pub next_restart: bool,
    /// What is [`Declarations::leaving`] and the system that waits for the
    /// next restart already left out: it leaves at that restart, not at the
    /// next update.
    #[serde(default)]
    pub leaving_at_restart: Vec<String>,
    /// The distro's programs the system that runs leaves out here and the
    /// one that waits for the next restart has: a decline undone, back from
    /// that restart.
    #[serde(default)]
    pub back_at_restart: Vec<String>,
}

impl Declarations {
    /// The declared entry a key is, when it is one.
    #[must_use]
    pub fn declared_as(&self, key: &AppKey) -> Option<&DeclaredRow> {
        let id = id_of(key)?;
        self.declared.iter().find(|r| r.entry.id() == id)
    }

    /// The decline of the distro's `id`, when there is one.
    #[must_use]
    pub fn decline_of(&self, id: &str) -> Option<&DeclinedRow> {
        self.declined.iter().find(|d| d.declining.id == id)
    }

    /// The distro's program of a registry id.
    #[must_use]
    pub fn distro(&self, id: &str) -> Option<&DistroRow> {
        self.distro.iter().find(|d| d.app.id == id)
    }

    /// The distro's program a key is: what the flake installed, by its
    /// launcher entry, or a program of a source whose ONE installation the
    /// distro shares, with one of the same entries.
    ///
    /// The distro's Flatpaks go into the person's `--user` installation
    /// (nix-flatpak) and its Android apps into the same Android (the
    /// reconciler): a Flatpak or an app the registry carries IS the distro's,
    /// whoever installed it first. The distro's Nix packages, AppImages and
    /// web apps are in the system (`systemPackages`), never in the person's
    /// profile, `~/Applications` or `~/.local/share/applications`, which is
    /// all those lists read — so a key of theirs is the person's own copy
    /// whatever the registry says, and calling it the distro's would leave
    /// it with no way out (no "remove", only "keep uninstalled", which
    /// declines the system's copy and leaves the person's).
    #[must_use]
    pub fn distro_of(&self, key: &AppKey, desktop_ids: &[String]) -> Option<&DistroRow> {
        let ids: Vec<&str> = match key {
            AppKey::System { desktop_id } => vec![desktop_id.as_str()],
            AppKey::Flatpak { .. } | AppKey::Android { .. } => {
                desktop_ids.iter().map(String::as_str).collect()
            }
            AppKey::Nix { .. } | AppKey::AppImage { .. } | AppKey::Web { .. } => return None,
        };
        let wanted = key.source().map(source_word);
        self.distro.iter().find(|d| {
            (wanted.is_none() || wanted == Some(d.app.source.word()))
                && d.app
                    .desktop_ids
                    .iter()
                    .any(|own| ids.contains(&own.as_str()))
        })
    }

    /// Who put a program there. `flake` is what its own list said (a
    /// Flatpak nix-flatpak declared): the person's declaration and the
    /// distro's registry are asked first, because both go through the same
    /// installation.
    #[must_use]
    pub fn origin_of(&self, key: &AppKey, desktop_ids: &[String], flake: bool) -> Origin {
        if self.declared_as(key).is_some() {
            return Origin::MineDeclared;
        }
        if let Some(d) = self.distro_of(key, desktop_ids) {
            return Origin::Distro {
                declinable: d.app.essential.is_none(),
            };
        }
        if flake || matches!(key, AppKey::System { .. }) {
            Origin::Declared
        } else {
            Origin::Mine
        }
    }
}

/// The person crate's word for a source — the same words, spelled once on
/// each side of the schema.
const fn source_word(source: Source) -> &'static str {
    source.word()
}

/// How this machine stands to one entry, from what its system reported.
#[must_use]
pub fn standing_of(entry: &Entry, host: &str, applied: Option<&PersonApplied>) -> Standing {
    if !entry.applies_to(host) {
        return Standing::Elsewhere;
    }
    let Some(report) = applied else {
        return Standing::Waiting;
    };
    if report.applied.iter().any(|id| id == entry.id()) {
        return Standing::Applied;
    }
    match report.skipped.iter().find(|s| s.id == entry.id()) {
        // DECLARED HERE SINCE: the system was built when it named other
        // machines only.
        Some(s) if s.reason == SkipWhy::OtherMachine => Standing::Waiting,
        Some(s) => Standing::Skipped {
            reason: s.reason,
            detail: s.detail.clone(),
        },
        None => Standing::Waiting,
    }
}

/// What one decline did here, from what the system reported.
#[must_use]
pub fn decline_state_of(
    declining: &Declining,
    host: &str,
    applied: Option<&PersonApplied>,
) -> DeclineState {
    if !declining.applies_to(host) {
        return DeclineState::Elsewhere;
    }
    let Some(report) = applied else {
        return DeclineState::Waiting;
    };
    let d = &report.declined;
    let id = declining.id.as_str();
    if let Some(r) = d.refused.iter().find(|r| r.id == id) {
        return DeclineState::Essential {
            reason: r.reason.clone(),
        };
    }
    if d.still_present.iter().any(|x| x == id) {
        return DeclineState::StillPresent;
    }
    if d.honoured.iter().any(|x| x == id) {
        return DeclineState::Honoured;
    }
    if d.unknown.iter().any(|x| x == id) {
        return DeclineState::Unknown;
    }
    DeclineState::Waiting
}

/// The name a launcher entry gives, looked for in the folders of
/// `$XDG_DATA_DIRS` (`applications/<id>`).
#[must_use]
pub fn entry_name(data_dirs: &[PathBuf], desktop_id: &str) -> Option<String> {
    if desktop_id.contains('/') || !desktop_id.ends_with(".desktop") {
        return None;
    }
    data_dirs.iter().find_map(|dir| {
        let text = std::fs::read_to_string(dir.join("applications").join(desktop_id)).ok()?;
        let fields = ronoctua_apps::desktop::fields(&text);
        ronoctua_apps::desktop::raw_bytes(&fields, "Name")
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
    })
}

/// What one of the distro's programs is called: the name its launcher
/// entry gives where the entry is on this machine, else the registry's
/// (`distro-apps.json`'s `name` — a program declined before it was ever
/// installed has no entry here), else its id.
#[must_use]
pub fn distro_name(app: &DistroApp, data_dirs: &[PathBuf]) -> String {
    app.desktop_ids
        .iter()
        .find_map(|id| entry_name(data_dirs, id))
        .or_else(|| app.name.clone())
        .unwrap_or_else(|| app.id.clone())
}

/// Everything, read from the files: the mirror, the system's reports and
/// this crate's ledger.
#[must_use]
pub fn read(roots: &Roots, host: &str) -> Declarations {
    let applied = person_applied(roots);
    let waiting = waiting_system(roots);
    let staged: Option<PersonApplied> = waiting
        .as_deref()
        .and_then(|top| report_of(roots, top, names::PERSON_APPLIED));
    let waiting_registry: Option<DistroApps> = waiting
        .as_deref()
        .and_then(|top| report_of(roots, top, names::DISTRO_APPS));
    let mut unreadable = Vec::new();
    let declared = read_declared(roots).unwrap_or_else(|why| {
        unreadable.push(Unreadable {
            file: File::Declared,
            why,
        });
        Declared::default()
    });
    // LEAVING: applied by the system, gone from the declaration — asked only
    // of a declaration that read, or every applied program would look gone.
    let leaving: Vec<String> = match &applied {
        Some(report) if unreadable.is_empty() => report
            .applied
            .iter()
            .filter(|id| declared.entry(id).is_none())
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    let declined = read_declined(roots).unwrap_or_else(|why| {
        unreadable.push(Unreadable {
            file: File::Declined,
            why,
        });
        Declined::default()
    });
    let (registry, registry_unreadable) = match report_read::<DistroApps>(roots, names::DISTRO_APPS)
    {
        Ok(Some(d)) => (d.apps, None),
        Ok(None) => (Vec::new(), Some(ronoctua_person::Why::NotJson)),
        Err(why) => (Vec::new(), Some(why)),
    };
    let distro: Vec<DistroRow> = registry
        .into_iter()
        .map(|app| {
            let name = distro_name(&app, &roots.data_dirs);
            DistroRow { app, name }
        })
        .collect();
    let declined_rows = declined
        .apps
        .into_iter()
        .map(|declining| {
            let name = distro
                .iter()
                .find(|d| d.app.id == declining.id)
                .map_or_else(|| declining.id.clone(), |d| d.name.clone());
            let state = match decline_state_of(&declining, host, applied.as_ref()) {
                // THE SYSTEM THAT WAITS FOR THE RESTART already leaves it out.
                DeclineState::Waiting
                    if staged
                        .as_ref()
                        .is_some_and(|next| next.declined.honoured.contains(&declining.id)) =>
                {
                    DeclineState::NextRestart
                }
                state => state,
            };
            DeclinedRow {
                state,
                name,
                declining,
            }
        })
        .collect();
    let rows = declared
        .apps
        .into_iter()
        .map(|entry| {
            let standing = standing_of(&entry, host, applied.as_ref());
            // IN THE SYSTEM THAT WAITS FOR THE NEXT RESTART, and not in the
            // one that runs: here from the restart, not at the next update.
            let standing = match &staged {
                Some(next)
                    if !matches!(standing, Standing::Applied | Standing::Elsewhere)
                        && next.applied.iter().any(|id| id == entry.id()) =>
                {
                    Standing::NextRestart
                }
                _ => standing,
            };
            // A PART OF THE PERSON'S is in the system once it ARRIVED.
            let standing = match crate::parts::of_entry(applied.as_ref(), entry.id()) {
                Some(part) if standing == Standing::Applied && !crate::parts::here(roots, part) => {
                    Standing::Arriving {
                        failure: crate::parts::failure(roots, part),
                    }
                }
                _ => standing,
            };
            DeclaredRow { standing, entry }
        })
        .collect();
    // WHAT THE SYSTEM THAT WAITS ALREADY TOOK OUT, or already brought back.
    let leaving_at_restart = staged.as_ref().map_or_else(Vec::new, |next| {
        leaving
            .iter()
            .filter(|id| !next.applied.contains(id))
            .cloned()
            .collect()
    });
    let back_at_restart = waiting_registry.map_or_else(Vec::new, |next| {
        next.apps
            .into_iter()
            .filter(|app| app.on_this_machine)
            .filter(|app| {
                distro
                    .iter()
                    .any(|d| d.app.id == app.id && !d.app.on_this_machine)
            })
            .map(|app| app.id)
            .collect()
    });
    let kept = crate::reconcile::ledger(roots);
    // WAITING FOR ANDROID: not in yet, and here to go in — one whose part is
    // still arriving waits for its part, and its row says so.
    let android_pending = declared_android(roots)
        .map(|d| d.apps)
        .unwrap_or_default()
        .into_iter()
        .filter(|a| !kept.has(&a.package, a.version_code))
        .filter(|a| crate::parts::file(roots, &a.part, &a.apk).is_ok())
        .map(|a| a.id)
        .collect();
    Declarations {
        host: host.to_owned(),
        declared: rows,
        declined: declined_rows,
        distro,
        unreadable,
        registry_unreadable,
        reported: applied.is_some(),
        android_pending,
        leaving,
        next_restart: staged.is_some(),
        leaving_at_restart,
        back_at_restart,
    }
}

/// The files whose change changes what [`read`] answers, for a door that
/// shows the declaration to watch: the mirror's two documents (an order of
/// this crate, the belongings' plain pull), the system's three reports, the
/// link a switch replaces (on a machine the reports' own links never move —
/// `/etc/static` does — so the switch is what says they changed) and the
/// reconciler's ledger (an Android app gone in).
#[must_use]
pub fn watched(roots: &Roots) -> Vec<PathBuf> {
    let reports = &roots.system.reports;
    vec![
        File::Declared.path(roots),
        File::Declined.path(roots),
        reports.join(names::PERSON_APPLIED),
        reports.join(names::DISTRO_APPS),
        reports.join(names::DECLARED_ANDROID),
        roots.system.current.clone(),
        // AN APPLY FOR THE NEXT BOOT moves no link of the running system:
        // the machine's record is what says one waits.
        roots.system.machine.join(names::STATE),
        roots.ours_state().join(crate::reconcile::LEDGER),
    ]
}

/// The folders whose change changes what [`read`] answers, watched whole:
/// the parts' records ([`crate::parts`]) — a declared program's part that
/// arrives turns its row from arriving to applied, and an apk's from
/// arriving to waiting for Android. Root writes a record when a part's
/// phase changes, never at each step of a download.
#[must_use]
pub fn watched_folders(roots: &Roots) -> Vec<PathBuf> {
    vec![roots.system.parts.clone()]
}

/// Who put each listed program there, and the declared copies no list of a
/// source sees: a declared Nix package is in the person's `home.packages`
/// and not in their profile; a declared AppImage or web app is the system's
/// launcher entry and not a file of theirs. A Flatpak and an Android app
/// are in their own lists already (the same installation, the same Android).
/// What is LEAVING ([`Declarations::leaving`]) is listed too, with
/// [`Refusal::LeavesAtNextApply`] as its trouble: it is still here.
pub fn shape(installed: &mut Vec<Installed>, d: &Declarations, roots: &Roots) {
    let leaving = |key: &AppKey| id_of(key).is_some_and(|id| d.leaving.contains(&id));
    // LEAVING AT THE NEXT RESTART, when the system that waits for it already
    // left it out; at the next update otherwise.
    let why_leaving = |id: &str| {
        if d.leaving_at_restart.iter().any(|x| x == id) {
            Refusal::LeavesAtNextRestart
        } else {
            Refusal::LeavesAtNextApply
        }
    };
    for i in installed.iter_mut() {
        i.origin = d.origin_of(&i.key, &i.desktop_ids, i.origin == Origin::Declared);
        // A FLATPAK OR AN ANDROID APP LEAVING is the system's copy in the
        // one installation there is: it says so, and "remove for good"
        // asks again for the rebuild that takes it. A Nix package, an
        // AppImage or a web app a list sees is the person's own copy, which
        // stays theirs.
        if matches!(i.key, AppKey::Flatpak { .. } | AppKey::Android { .. })
            && matches!(i.origin, Origin::Mine | Origin::Declared)
            && leaving(&i.key)
        {
            i.origin = Origin::MineDeclared;
            i.trouble = id_of(&i.key).map(|id| why_leaving(&id));
        }
    }
    // THE LEAVING COPIES NO LIST SEES — a Nix package of `home.packages`, a
    // wrapped AppImage, a web app's system entry — are still here until the
    // next apply, and say so rather than vanish.
    for id in &d.leaving {
        let Some(key) = key_of_id(id) else { continue };
        if installed.iter().any(|i| i.key == key) {
            continue;
        }
        let desktop_ids = match &key {
            AppKey::AppImage { key } => vec![crate::appimage::desktop_name(key)],
            AppKey::Web { slug } => vec![crate::web::desktop_id(slug)],
            AppKey::Nix { .. } => Vec::new(),
            _ => continue,
        };
        let name = desktop_ids
            .iter()
            .find_map(|x| entry_name(&roots.data_dirs, x))
            .unwrap_or_else(|| key.id().to_owned());
        installed.push(Installed {
            key,
            name,
            version: String::new(),
            desktop_ids,
            size_bytes: None,
            update: None,
            unfree: false,
            origin: Origin::MineDeclared,
            icon: None,
            trouble: Some(why_leaving(id)),
        });
    }
    for row in d
        .declared
        .iter()
        .filter(|r| r.standing == Standing::Applied)
    {
        let key = key_of(&row.entry);
        if installed.iter().any(|i| i.key == key) {
            continue;
        }
        let icon = |sha: Option<&String>| {
            sha.map(|s| icon_path(roots, s))
                .filter(|p| p.is_file())
                .map(|path| Picture::File { path })
        };
        let (desktop_ids, version, picture) = match &row.entry {
            Entry::Nix(_) => (Vec::new(), String::new(), None),
            Entry::AppImage(a) => (
                vec![crate::appimage::desktop_name(&a.key)],
                a.pin.version.clone(),
                icon(a.icon.as_ref()),
            ),
            Entry::Web(a) => (
                vec![crate::web::desktop_id(&a.slug)],
                String::new(),
                icon(a.icon.as_ref()),
            ),
            Entry::Flatpak(_) | Entry::Android(_) => continue,
        };
        installed.push(Installed {
            key,
            name: name_of(&row.entry),
            version,
            desktop_ids,
            size_bytes: None,
            update: None,
            unfree: matches!(&row.entry, Entry::Nix(a) if a.unfree),
            origin: Origin::MineDeclared,
            icon: picture,
            trouble: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_person::declared::{NixApp, WebApp};
    use ronoctua_person::reports::{DeclinedReport, RefusedDecline, Skipped};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-declaration-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn stamp() -> Stamp {
        Stamp::parse("2026-10-05T12:00:00Z").unwrap()
    }

    fn nix(attr: &str, machines: Option<Vec<String>>) -> Entry {
        Entry::Nix(NixApp {
            id: format!("nix:{attr}"),
            machines,
            edited_at: stamp(),
            edited_on: "noctua".into(),
            attr: attr.into(),
            unfree: false,
        })
    }

    fn web(slug: &str) -> Entry {
        Entry::Web(WebApp {
            id: format!("web:{slug}"),
            machines: None,
            edited_at: stamp(),
            edited_on: "noctua".into(),
            slug: slug.into(),
            name: "Mail".into(),
            url: "https://mail.example.org/".into(),
            profile: "Default".into(),
            icon: None,
            category: "Network".into(),
        })
    }

    fn report() -> PersonApplied {
        PersonApplied {
            format: 1,
            machine: "noctua".into(),
            distro_rev: "abc1234".into(),
            applied: vec!["nix:ripgrep".into(), "web:mail".into()],
            parts: Default::default(),
            skipped: vec![
                Skipped {
                    id: "nix:gone".into(),
                    reason: SkipWhy::AttrMissing,
                    detail: String::new(),
                },
                Skipped {
                    id: "nix:here-now".into(),
                    reason: SkipWhy::OtherMachine,
                    detail: "declared for laptop".into(),
                },
            ],
            declined: DeclinedReport {
                honoured: vec!["gimp".into(), "vlc".into()],
                refused: vec![RefusedDecline {
                    id: "zsh".into(),
                    reason: "the-login-shell".into(),
                }],
                unknown: vec!["old-thing".into()],
                still_present: vec!["vlc".into()],
            },
            brain: Default::default(),
            signature: Default::default(),
        }
    }

    /// FAILS IF: an entry's standing does not follow the report — applied,
    /// skipped with its reason, declared here since an "other machine" skip
    /// (waits), on other machines only, or not reported at all (waits).
    #[test]
    fn an_entry_stands_as_the_system_reported_it() {
        let r = report();
        assert_eq!(
            standing_of(&nix("ripgrep", None), "noctua", Some(&r)),
            Standing::Applied
        );
        assert_eq!(
            standing_of(&nix("gone", None), "noctua", Some(&r)),
            Standing::Skipped {
                reason: SkipWhy::AttrMissing,
                detail: String::new()
            }
        );
        assert_eq!(
            standing_of(&nix("here-now", None), "noctua", Some(&r)),
            Standing::Waiting
        );
        assert_eq!(
            standing_of(&nix("new", None), "noctua", Some(&r)),
            Standing::Waiting
        );
        assert_eq!(
            standing_of(
                &nix("ripgrep", Some(vec!["laptop".into()])),
                "noctua",
                Some(&r)
            ),
            Standing::Elsewhere
        );
        assert_eq!(
            standing_of(&nix("ripgrep", None), "noctua", None),
            Standing::Waiting
        );
    }

    /// FAILS IF: a decline's state is not what the report says of it — the
    /// essential reason kept, still present beating honoured, unknown,
    /// elsewhere, and waiting when nothing was reported.
    #[test]
    fn a_decline_says_what_it_did_here() {
        let r = report();
        let d = |id: &str, machines: Option<Vec<String>>| Declining {
            id: id.into(),
            machines,
            edited_at: stamp(),
            edited_on: "noctua".into(),
        };
        assert_eq!(
            decline_state_of(&d("gimp", None), "noctua", Some(&r)),
            DeclineState::Honoured
        );
        assert_eq!(
            decline_state_of(&d("vlc", None), "noctua", Some(&r)),
            DeclineState::StillPresent
        );
        assert_eq!(
            decline_state_of(&d("zsh", None), "noctua", Some(&r)),
            DeclineState::Essential {
                reason: "the-login-shell".into()
            }
        );
        assert_eq!(
            decline_state_of(&d("old-thing", None), "noctua", Some(&r)),
            DeclineState::Unknown
        );
        assert_eq!(
            decline_state_of(&d("gimp", Some(vec!["laptop".into()])), "noctua", Some(&r)),
            DeclineState::Elsewhere
        );
        assert_eq!(
            decline_state_of(&d("htop", None), "noctua", Some(&r)),
            DeclineState::Waiting
        );
        assert_eq!(
            decline_state_of(&d("gimp", None), "noctua", None),
            DeclineState::Waiting
        );
    }

    /// FAILS IF: an upsert moves an entry, appends a second of the same id,
    /// or calls a re-declaration of the same thing an edit.
    #[test]
    fn an_entry_replaces_its_own_in_place() {
        let mut doc = Declared::default();
        assert!(upsert(&mut doc, nix("a", None)));
        assert!(upsert(&mut doc, nix("b", None)));
        assert!(!upsert(&mut doc, nix("a", None)), "the same thing again");
        assert!(upsert(&mut doc, nix("a", Some(vec!["noctua".into()]))));
        let ids: Vec<&str> = doc.apps.iter().map(Entry::id).collect();
        assert_eq!(ids, ["nix:a", "nix:b"]);
        assert_eq!(doc.apps[0].machines(), Some(&["noctua".to_owned()][..]));
        assert!(without(&mut doc, "nix:a"));
        assert!(!without(&mut doc, "nix:a"));
    }

    /// FAILS IF: a change is written over a file this machine cannot read (a
    /// newer machine's), a change that would not validate is written, or a
    /// written file is not the canonical form root keeps byte for byte.
    #[test]
    fn a_change_is_made_to_the_file_as_it_is_and_never_over_one_unread() {
        let root = scratch("change");
        let roots = Roots::under(&root);
        let written = change_declared(&roots, |d| {
            upsert(d, nix("ripgrep", None));
            Ok(())
        })
        .unwrap();
        let path = mirror(&roots).join(names::DECLARED);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            ronoctua_person::to_json(&written)
        );
        // A NEWER FORMAT stays as it is, and the change is refused.
        std::fs::write(&path, br#"{"format": 2, "apps": [], "more": true}"#).unwrap();
        let refused = change_declared(&roots, |d| {
            upsert(d, nix("fd", None));
            Ok(())
        })
        .unwrap_err();
        assert_eq!(refused.refusal, Some(Refusal::DeclarationUnreadable));
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"more\""));
        // A CHANGE THAT WOULD NOT VALIDATE is not written.
        std::fs::remove_file(&path).unwrap();
        let invalid = change_declared(&roots, |d| {
            upsert(d, nix("not valid", None));
            Ok(())
        })
        .unwrap_err();
        assert_eq!(invalid.refusal, Some(Refusal::DeclarationRefused));
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a change is written over a merge the belongings landed
    /// between its reading and its writing — the other computer's entry
    /// would be lost — instead of being made again over what is there now;
    /// or a file that never stops moving is written anyway.
    #[test]
    fn a_change_is_made_again_over_a_merge_that_landed_meanwhile() {
        let root = scratch("raced");
        let roots = Roots::under(&root);
        let path = mirror(&roots).join(names::DECLARED);
        change_declared(&roots, |d| {
            upsert(d, nix("ripgrep", None));
            Ok(())
        })
        .unwrap();
        let mut tries = 0;
        let written = change_declared(&roots, |d| {
            tries += 1;
            if tries == 1 {
                // THE BELONGINGS LAND A MERGE: another computer declared fd.
                let mut merged = read_declared(&roots).unwrap();
                upsert(&mut merged, nix("fd", Some(vec!["laptop".into()])));
                write_doc(&roots, File::Declared, &merged).unwrap();
            }
            upsert(d, nix("hello", None));
            Ok(())
        })
        .unwrap();
        assert_eq!(tries, 2, "made again once");
        let ids: Vec<&str> = written.apps.iter().map(Entry::id).collect();
        assert_eq!(ids, ["nix:ripgrep", "nix:fd", "nix:hello"]);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            ronoctua_person::to_json(&written)
        );
        // A FILE THAT MOVES UNDER EVERY TRY is left to whoever moves it.
        let mut n = 0u32;
        let refused = change_declared(&roots, |d| {
            n += 1;
            let mut other = read_declared(&roots).unwrap();
            upsert(&mut other, nix(&format!("moved{n}"), None));
            write_doc(&roots, File::Declared, &other).unwrap();
            upsert(d, nix("never", None));
            Ok(())
        })
        .unwrap_err();
        assert_eq!(refused.refusal, None);
        assert!(read_declared(&roots).unwrap().entry("nix:never").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a declared program that only the system that waits for the
    /// next restart applied is said to wait for the next update — it is here
    /// from the restart —, what that system already left out (a removal, a
    /// decline) or already brought back (a decline undone) is said to change
    /// at the next update, the page does not say a system waits — or what
    /// waits is read off the last apply: a system that runs, or that the
    /// profile no longer names (its trial failed, the profile went back),
    /// waits for nothing, whatever the record says.
    #[test]
    fn what_the_next_boot_brings_arrives_at_the_restart() {
        let root = scratch("next-restart");
        let roots = Roots::under(&root);
        let mut doc = Declared::default();
        upsert(&mut doc, nix("ripgrep", None));
        upsert(&mut doc, nix("fresh", None));
        upsert(&mut doc, nix("later", None));
        write_doc(&roots, File::Declared, &doc).unwrap();
        let mut declined = Declined::default();
        declined.decline("krita", None, &stamp(), "noctua");
        write_doc(&roots, File::Declined, &declined).unwrap();
        std::fs::create_dir_all(&roots.system.reports).unwrap();
        // THE SYSTEM THAT RUNS applied web:mail, which the declaration no
        // longer names; it has kdenlive left out by a decline since undone.
        std::fs::write(
            roots.system.reports.join(names::PERSON_APPLIED),
            ronoctua_person::to_json(&report()),
        )
        .unwrap();
        let registry = |kdenlive_here: bool| {
            ronoctua_person::to_json(&DistroApps {
                format: 1,
                apps: vec![DistroApp {
                    id: "kdenlive".into(),
                    name: None,
                    source: ronoctua_person::declared::Source::Nix,
                    desktop_ids: Vec::new(),
                    essential: None,
                    group: "video".into(),
                    on_this_machine: kdenlive_here,
                    declined_here: !kdenlive_here,
                    appimage: None,
                }],
                followed: ronoctua_person::reports::FollowedReport::default(),
            })
        };
        std::fs::write(
            roots.system.reports.join(names::DISTRO_APPS),
            registry(false),
        )
        .unwrap();
        // THE SYSTEM THAT WAITS applied nix:fresh, left web:mail and krita
        // out, and has kdenlive back.
        let store = |name: &str| {
            let top =
                format!("/nix/store/0123456789abcdfghijklmnpqrsvwxy{name}-nixos-system-{name}");
            std::fs::create_dir_all(
                roots
                    .system
                    .root
                    .join(top.trim_start_matches('/'))
                    .join("etc/ronoctua"),
            )
            .unwrap();
            top
        };
        let (a, top) = (store("a"), store("b"));
        let inside = roots
            .system
            .root
            .join(top.trim_start_matches('/'))
            .join("etc/ronoctua");
        let mut next = report();
        next.applied = vec!["nix:ripgrep".into(), "nix:fresh".into()];
        next.declined.honoured.push("krita".into());
        std::fs::write(
            inside.join(names::PERSON_APPLIED),
            ronoctua_person::to_json(&next),
        )
        .unwrap();
        std::fs::write(inside.join(names::DISTRO_APPS), registry(true)).unwrap();
        let link = |at: &str, to: &str| {
            let at = roots.system.root.join(at);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            let _ = std::fs::remove_file(&at);
            std::os::unix::fs::symlink(roots.system.root.join(to.trim_start_matches('/')), at)
                .unwrap();
        };
        let standing = |d: &Declarations, id: &str| {
            d.declared
                .iter()
                .find(|r| r.entry.id() == id)
                .map(|r| r.standing.clone())
        };
        let decline = |d: &Declarations| d.declined[0].state.clone();
        link("run/current-system", &a);
        link("nix/var/nix/profiles/system", &top);
        let d = read(&roots, "noctua");
        assert!(d.next_restart);
        assert_eq!(standing(&d, "nix:fresh"), Some(Standing::NextRestart));
        assert_eq!(standing(&d, "nix:ripgrep"), Some(Standing::Applied));
        assert_eq!(standing(&d, "nix:later"), Some(Standing::Waiting));
        assert_eq!(d.leaving, ["web:mail"]);
        assert_eq!(d.leaving_at_restart, ["web:mail"]);
        assert_eq!(decline(&d), DeclineState::NextRestart);
        assert_eq!(d.back_at_restart, ["kdenlive"]);
        let mut installed = Vec::new();
        shape(&mut installed, &d, &roots);
        assert_eq!(
            installed
                .iter()
                .find(|i| i.key
                    == AppKey::Web {
                        slug: "mail".into()
                    })
                .and_then(|i| i.trouble),
            Some(Refusal::LeavesAtNextRestart)
        );
        assert!(watched(&roots).contains(&roots.system.machine.join(names::STATE)));
        // A STAGED RECORD whose system the profile no longer names (its
        // trial failed and the profile went back): nothing waits.
        std::fs::create_dir_all(&roots.system.machine).unwrap();
        std::fs::write(
            roots.system.machine.join(names::STATE),
            format!(
                r#"{{"format": 1, "last-good": null, "trial": null, "hold": null,
                   "last-apply": {{"at": "2026-10-30T12:00:00Z", "result": "staged", "toplevel": "{top}"}}}}"#
            ),
        )
        .unwrap();
        link("nix/var/nix/profiles/system", &a);
        let d = read(&roots, "noctua");
        assert!(!d.next_restart);
        assert_eq!(standing(&d, "nix:fresh"), Some(Standing::Waiting));
        assert!(d.leaving_at_restart.is_empty() && d.back_at_restart.is_empty());
        assert_eq!(decline(&d), DeclineState::Waiting);
        // RESTARTED INTO IT: it runs, and its own reports say so.
        link("nix/var/nix/profiles/system", &top);
        link("run/current-system", &top);
        assert!(!read(&roots, "noctua").next_restart);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: what the system applied and the declaration no longer names
    /// is not said as leaving — or vanishes from the list while it is still
    /// here — or a person's own copy of it (a profile entry) is taken for
    /// the leaving one; or a declaration that does not read makes every
    /// applied program look gone.
    #[test]
    fn what_left_the_declaration_is_here_until_the_next_apply() {
        let root = scratch("leaving");
        let roots = Roots::under(&root);
        let mut doc = Declared::default();
        upsert(&mut doc, nix("ripgrep", None));
        write_doc(&roots, File::Declared, &doc).unwrap();
        std::fs::create_dir_all(&roots.system.reports).unwrap();
        let mut r = report();
        r.applied.push("nix:hello".into());
        r.applied.push("flatpak:org.gnome.Maps".into());
        std::fs::write(
            roots.system.reports.join(names::PERSON_APPLIED),
            ronoctua_person::to_json(&r),
        )
        .unwrap();
        let d = read(&roots, "noctua");
        assert_eq!(
            d.leaving,
            ["web:mail", "nix:hello", "flatpak:org.gnome.Maps"]
        );
        let maps = AppKey::Flatpak {
            id: "org.gnome.Maps".into(),
        };
        let mut installed = vec![Installed {
            key: maps.clone(),
            name: "Maps".into(),
            version: String::new(),
            desktop_ids: vec!["org.gnome.Maps.desktop".into()],
            size_bytes: None,
            update: None,
            unfree: false,
            origin: Origin::Declared,
            icon: None,
            trouble: None,
        }];
        shape(&mut installed, &d, &roots);
        let of = |key: &AppKey| installed.iter().find(|i| i.key == *key).cloned();
        let maps = of(&maps).unwrap();
        assert_eq!(maps.origin, Origin::MineDeclared);
        assert_eq!(maps.trouble, Some(Refusal::LeavesAtNextApply));
        let hello = of(&AppKey::Nix {
            attr: "hello".into(),
        })
        .expect("the leaving Nix package is listed");
        assert_eq!(hello.trouble, Some(Refusal::LeavesAtNextApply));
        assert_eq!(hello.origin, Origin::MineDeclared);
        assert_eq!(
            of(&AppKey::Web {
                slug: "mail".into()
            })
            .unwrap()
            .desktop_ids,
            [crate::web::desktop_id("mail")]
        );
        // THE PERSON'S OWN PROFILE COPY of a leaving package stays theirs.
        let mut mine = vec![Installed {
            key: AppKey::Nix {
                attr: "hello".into(),
            },
            trouble: None,
            origin: Origin::Mine,
            ..hello
        }];
        shape(&mut mine, &d, &roots);
        assert_eq!(mine[0].origin, Origin::Mine);
        assert_eq!(mine[0].trouble, None);
        // A DECLARATION THAT DOES NOT READ leaves nothing leaving.
        std::fs::write(mirror(&roots).join(names::DECLARED), b"{").unwrap();
        assert!(read(&roots, "noctua").leaving.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a declared program that is a part of the person's is said
    /// to be on this computer before its part arrived — on its way, or
    /// failed with the words its try said — or still on its way once it is
    /// here.
    #[test]
    fn a_part_stands_as_its_record_says() {
        use ronoctua_parts::record::{Record, Target, Why};
        let root = scratch("part-standing");
        let roots = Roots::under(&root);
        let mut doc = Declared::default();
        upsert(&mut doc, nix("ripgrep", None));
        write_doc(&roots, File::Declared, &doc).unwrap();
        std::fs::create_dir_all(&roots.system.reports).unwrap();
        let mut r = report();
        r.parts
            .insert("nix:ripgrep".into(), "person-0000000a-ripgrep".into());
        std::fs::write(
            roots.system.reports.join(names::PERSON_APPLIED),
            ronoctua_person::to_json(&r),
        )
        .unwrap();
        let standing = |roots: &Roots| {
            read(roots, "noctua")
                .declared
                .iter()
                .find(|row| row.entry.id() == "nix:ripgrep")
                .map(|row| row.standing.clone())
        };
        assert_eq!(
            standing(&roots),
            Some(Standing::Arriving { failure: None }),
            "no record yet: on its way"
        );
        let fp = "a".repeat(64);
        let at = ronoctua_person::Stamp::parse("2026-10-25T12:00:00Z").unwrap();
        let write = |record: &Record| {
            std::fs::create_dir_all(&roots.system.parts).unwrap();
            std::fs::write(
                roots.system.parts.join("person-0000000a-ripgrep.json"),
                ronoctua_person::to_json(record),
            )
            .unwrap();
        };
        let failed = Record::new("person-0000000a-ripgrep")
            .planned_entry(&fp)
            .failed(
                Why::EvalFailed,
                "error: attribute 'ripgrep' missing",
                at.clone(),
                true,
            );
        write(&failed);
        assert_eq!(
            standing(&roots),
            Some(Standing::Arriving {
                failure: Some("error: attribute 'ripgrep' missing".into())
            })
        );
        let here = failed
            .resolved(Target {
                drv: "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-ripgrep.drv".into(),
                out: "/nix/store/zyxwvsrqpnmlkjihgfdcba9876543210-ripgrep".into(),
                interface: 1,
            })
            .arrived(at);
        write(&here);
        assert_eq!(standing(&roots), Some(Standing::Applied));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: an id does not come back as the key it was made of, or one
    /// that names no source is taken for one.
    #[test]
    fn an_id_is_its_key() {
        for key in [
            AppKey::Nix {
                attr: "python3Packages.requests".into(),
            },
            AppKey::Flatpak {
                id: "org.gnome.Maps".into(),
            },
            AppKey::AppImage {
                key: "obsidian".into(),
            },
            AppKey::Android {
                package: "org.fdroid.fdroid".into(),
            },
            AppKey::Web {
                slug: "mail".into(),
            },
        ] {
            assert_eq!(key_of_id(&id_of(&key).unwrap()), Some(key));
        }
        assert_eq!(key_of_id("system:x.desktop"), None);
        assert_eq!(key_of_id("nix:"), None);
        assert_eq!(key_of_id("ripgrep"), None);
    }

    /// FAILS IF: a file the declaration's reading depends on is not watched
    /// — the switch's link included, since a machine's reports never move
    /// their own links — or the parts' records, where a declared program
    /// arrives.
    #[test]
    fn what_the_reading_depends_on_is_watched() {
        let roots = Roots::under(Path::new("/room"));
        let watched = watched(&roots);
        for wanted in [
            "/room/home/.local/state/ronoctua/person/software/declared.json",
            "/room/home/.local/state/ronoctua/person/software/declined.json",
            "/room/sys/etc/ronoctua/person-applied.json",
            "/room/sys/etc/ronoctua/distro-apps.json",
            "/room/sys/etc/ronoctua/declared-android.json",
            "/room/sys/run/current-system",
            "/room/sys/var/lib/ronoctua-machine/state.json",
        ] {
            assert!(
                watched.iter().any(|w| w == Path::new(wanted)),
                "{wanted} in {watched:?}"
            );
        }
        assert!(
            watched
                .iter()
                .any(|w| w.ends_with(crate::reconcile::LEDGER))
        );
        assert_eq!(
            watched_folders(&roots),
            [Path::new("/room/sys/var/lib/ronoctua-parts/parts")]
        );
    }

    /// FAILS IF: an icon root would refuse (not a PNG, over the cap) is
    /// kept, or a kept one is not named by its content.
    #[test]
    fn an_icon_is_kept_only_as_root_would_keep_it() {
        let root = scratch("icon");
        let roots = Roots::under(&root);
        let png = b"\x89PNG\r\n\x1a\n-the-rest".to_vec();
        let sha = keep_icon(&roots, &png).unwrap();
        assert_eq!(sha, crate::run::sha256(&png));
        assert_eq!(std::fs::read(icon_path(&roots, &sha)).unwrap(), png);
        assert_eq!(keep_icon(&roots, b"GIF89a"), None);
        let mut big = png;
        big.resize(usize::try_from(caps::ICON_MOST_BYTES).unwrap() + 1, 0);
        assert_eq!(keep_icon(&roots, &big), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: an installed program's origin is not the declaration's
    /// first, then the distro's, then the flake's; or a declared copy the
    /// lists cannot see (a web app the system's entry carries) is missing.
    #[test]
    fn who_put_it_there_is_the_declaration_then_the_distro_then_the_flake() {
        let root = scratch("origin");
        let roots = Roots::under(&root);
        let d = Declarations {
            host: "noctua".into(),
            declared: vec![
                DeclaredRow {
                    entry: nix("ripgrep", None),
                    standing: Standing::Applied,
                },
                DeclaredRow {
                    entry: web("mail"),
                    standing: Standing::Applied,
                },
            ],
            distro: vec![DistroRow {
                app: DistroApp {
                    id: "gimp".into(),
                    name: None,
                    source: ronoctua_person::declared::Source::Nix,
                    desktop_ids: vec!["gimp.desktop".into()],
                    essential: None,
                    group: "graphics".into(),
                    on_this_machine: true,
                    declined_here: false,
                    appimage: None,
                },
                name: "GIMP".into(),
            }],
            ..Declarations::default()
        };
        let mine = AppKey::Nix {
            attr: "hello".into(),
        };
        assert_eq!(d.origin_of(&mine, &[], false), Origin::Mine);
        let declared = AppKey::Nix {
            attr: "ripgrep".into(),
        };
        assert_eq!(d.origin_of(&declared, &[], true), Origin::MineDeclared);
        let gimp = AppKey::System {
            desktop_id: "gimp.desktop".into(),
        };
        assert_eq!(
            d.origin_of(&gimp, &[], false),
            Origin::Distro { declinable: true }
        );
        let other = AppKey::System {
            desktop_id: "x.desktop".into(),
        };
        assert_eq!(d.origin_of(&other, &[], false), Origin::Declared);
        // THE PERSON'S OWN COPY of a program the distro also ships — `nix
        // profile add nixpkgs#gimp`, an AppImage of `~/Applications`, a web
        // app of their own of the same name — is theirs: the distro's copies
        // of those sources are in the system, never in those lists.
        let mut d = d;
        for (id, source, desktop) in [
            ("mpv", ronoctua_person::declared::Source::Nix, "mpv.desktop"),
            (
                "obsidian",
                ronoctua_person::declared::Source::AppImage,
                "ronoctua-appimage-obsidian.desktop",
            ),
            (
                "notes",
                ronoctua_person::declared::Source::Web,
                "ronoctua-web-notes.desktop",
            ),
            (
                "calculator",
                ronoctua_person::declared::Source::Flatpak,
                "org.gnome.Calculator.desktop",
            ),
        ] {
            d.distro.push(DistroRow {
                app: DistroApp {
                    id: id.into(),
                    name: None,
                    source,
                    desktop_ids: vec![desktop.into()],
                    essential: None,
                    group: "base".into(),
                    on_this_machine: true,
                    declined_here: false,
                    appimage: None,
                },
                name: id.into(),
            });
        }
        let own = |key: AppKey, desktop: &str| d.origin_of(&key, &[desktop.to_owned()], false);
        assert_eq!(
            own(AppKey::Nix { attr: "mpv".into() }, "mpv.desktop"),
            Origin::Mine
        );
        assert_eq!(
            own(
                AppKey::Nix {
                    attr: "gimp".into()
                },
                "gimp.desktop"
            ),
            Origin::Mine
        );
        assert_eq!(
            own(
                AppKey::AppImage {
                    key: "obsidian".into()
                },
                "ronoctua-appimage-obsidian.desktop"
            ),
            Origin::Mine
        );
        assert_eq!(
            own(
                AppKey::Web {
                    slug: "notes".into()
                },
                "ronoctua-web-notes.desktop"
            ),
            Origin::Mine
        );
        // A FLATPAK the registry carries is the one installation there is.
        assert_eq!(
            own(
                AppKey::Flatpak {
                    id: "org.gnome.Calculator".into()
                },
                "org.gnome.Calculator.desktop"
            ),
            Origin::Distro { declinable: true }
        );
        // AND THE SYSTEM'S ENTRY of the distro's Nix program is the distro's.
        assert_eq!(
            d.origin_of(
                &AppKey::System {
                    desktop_id: "mpv.desktop".into()
                },
                &[],
                false
            ),
            Origin::Distro { declinable: true }
        );

        let mut installed = vec![Installed {
            key: declared,
            name: "ripgrep".into(),
            version: String::new(),
            desktop_ids: vec![],
            size_bytes: None,
            update: None,
            unfree: false,
            origin: Origin::Mine,
            icon: None,
            trouble: None,
        }];
        shape(&mut installed, &d, &roots);
        assert_eq!(installed[0].origin, Origin::MineDeclared);
        let web = installed
            .iter()
            .find(|i| {
                i.key
                    == AppKey::Web {
                        slug: "mail".into(),
                    }
            })
            .expect("the declared web app is listed");
        assert_eq!(web.origin, Origin::MineDeclared);
        assert_eq!(web.desktop_ids, [crate::web::desktop_id("mail")]);
        assert_eq!(installed.len(), 2, "ripgrep is listed once");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the reading of a room — the mirror, the reports, a launcher
    /// entry's name — does not come back as the screen draws it.
    #[test]
    fn a_machine_is_read_from_its_files() {
        let root = scratch("read");
        let roots = Roots::under(&root);
        let mut doc = Declared::default();
        upsert(&mut doc, nix("ripgrep", None));
        upsert(&mut doc, nix("gone", None));
        write_doc(&roots, File::Declared, &doc).unwrap();
        let mut declined = Declined::default();
        declined.decline("gimp", None, &stamp(), "noctua");
        declined.decline(
            "libreoffice",
            Some(vec!["noctua".into()]),
            &stamp(),
            "noctua",
        );
        declined.decline("htop", None, &stamp(), "noctua");
        write_doc(&roots, File::Declined, &declined).unwrap();
        std::fs::create_dir_all(&roots.system.reports).unwrap();
        std::fs::write(
            roots.system.reports.join(names::PERSON_APPLIED),
            ronoctua_person::to_json(&report()),
        )
        .unwrap();
        let app = |id: &str, name: Option<&str>, desktop: &str| DistroApp {
            id: id.into(),
            name: name.map(str::to_owned),
            source: ronoctua_person::declared::Source::Nix,
            desktop_ids: vec![desktop.into()],
            essential: None,
            group: "graphics".into(),
            on_this_machine: true,
            declined_here: true,
            appimage: None,
        };
        // GIMP's entry is here, and wins over the registry's name;
        // LibreOffice was declined before it was ever installed: no entry,
        // the registry's name; htop has neither, and is its id.
        let registry = DistroApps {
            format: 1,
            apps: vec![
                app("gimp", Some("GIMP"), "gimp.desktop"),
                app("libreoffice", Some("LibreOffice"), "startcenter.desktop"),
                app("htop", None, "htop.desktop"),
            ],
            followed: ronoctua_person::reports::FollowedReport::default(),
        };
        std::fs::write(
            roots.system.reports.join(names::DISTRO_APPS),
            ronoctua_person::to_json(&registry),
        )
        .unwrap();
        let apps = roots.data_dirs[0].join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join("gimp.desktop"),
            "[Desktop Entry]\nType=Application\nName=GNU Image Manipulation Program\n",
        )
        .unwrap();
        let d = read(&roots, "noctua");
        assert!(d.reported);
        assert!(d.unreadable.is_empty());
        let standings: Vec<&Standing> = d.declared.iter().map(|r| &r.standing).collect();
        assert_eq!(standings[0], &Standing::Applied);
        assert!(matches!(
            standings[1],
            Standing::Skipped {
                reason: SkipWhy::AttrMissing,
                ..
            }
        ));
        assert_eq!(d.declined[0].name, "GNU Image Manipulation Program");
        assert_eq!(d.declined[0].state, DeclineState::Honoured);
        assert_eq!(d.distro[0].name, "GNU Image Manipulation Program");
        assert_eq!(d.declined[1].name, "LibreOffice", "the registry's name");
        assert_eq!(d.declined[2].name, "htop", "no name at all: the id");
        assert_eq!(d.registry_unreadable, None);
        // THE SYSTEM'S LIST THAT DOES NOT READ is said, never an empty one:
        // broken, newer, or not there.
        let registry_file = roots.system.reports.join(names::DISTRO_APPS);
        for (bytes, why) in [
            (&b"{"[..], ronoctua_person::Why::NotJson),
            (
                &b"{\"format\": 9, \"apps\": []}"[..],
                ronoctua_person::Why::Newer,
            ),
        ] {
            std::fs::write(&registry_file, bytes).unwrap();
            let d = read(&roots, "noctua");
            assert_eq!(d.registry_unreadable, Some(why));
            assert!(d.distro.is_empty());
            assert_eq!(
                d.declined[1].name, "libreoffice",
                "named by nothing but its id"
            );
        }
        std::fs::remove_file(&registry_file).unwrap();
        assert_eq!(
            read(&roots, "noctua").registry_unreadable,
            Some(ronoctua_person::Why::NotJson)
        );
        // A FILE THAT DOES NOT READ is said, and the rest is still read.
        std::fs::write(File::Declined.path(&roots), b"{").unwrap();
        let d = read(&roots, "noctua");
        assert_eq!(
            d.unreadable,
            [Unreadable {
                file: File::Declined,
                why: ronoctua_person::Why::NotJson
            }]
        );
        assert_eq!(d.declared.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
