//! ENCRYPT ANYTHING — what the file manager's `E`/`U` and the security
//! centre's Encrypt section (`LEADER s e c`) ask, and what they hand the queue.
//!
//! # Three ways to seal, two to open, one engine
//!
//! A file or a folder is sealed with age ([`Mode`]): FOR YOU (the person's
//! post-quantum recipient, which the belongings' last sync left — no
//! passphrase asked), WITH A PASSPHRASE (scrypt, asked twice: a passphrase
//! mistyped once is a file nobody opens again), or FOR SOMEONE (their
//! `age1pq1…` key, pasted, its checksum checked before anything is queued).
//! A `.age` is opened by what its header says ([`opening`]): a passphrase,
//! or the belongings' key — `key.age` opened with the belongings' passphrase
//! inside `ronoctua-seal`, in memory, never on a disk.
//!
//! Every one is a `Work::Seal` on the transfer queue, whose child is
//! `ronoctua-seal`: neither door links `age` (`transfers-never-link-age`).
//!
//! # Never over anything, never the original
//!
//! The output gets a name nothing in the folder uses ([`sealed_target`],
//! [`opened_target`]) and the program creates it `create_new` besides; the
//! original is never touched. A folder becomes `name.tar.age`, opened back
//! into `name.tar` (which the manager walks into like a folder).
//!
//! # Secrets
//!
//! A passphrase typed on the door's masked line lives as a [`Secret`] and
//! reaches the program as the `0600` file `ronoctua_transfers::secrets`
//! makes — one per job, since a job removes its own when it ends.
//!
//! # Both views, one state
//!
//! [`choose`], [`answer`] and [`sniffed`] are the state machine both views
//! drive: each turns a [`Next`] into its own question, sentence or errand, so
//! the manager and the centre ask the same questions in the same order.

use std::path::{Path, PathBuf};

use ronoctua_consent::keys::Secret;
use ronoctua_files::mode::Explanation;
use ronoctua_i18n::key;
use ronoctua_pq::Kind;
use ronoctua_transfers::Work;
use serde::{Deserialize, Serialize};

/// A new passphrase to seal with (secret) — `ask-seal-passphrase-new`.
pub const PASSPHRASE_NEW: &str = "seal-passphrase-new";
/// The same again (secret).
pub const PASSPHRASE_AGAIN: &str = "seal-passphrase-again";
/// The passphrase a `.age` opens with (secret).
pub const PASSPHRASE: &str = "seal-passphrase";
/// The same, after the one given did not open it (secret).
pub const PASSPHRASE_WRONG: &str = "seal-passphrase-wrong";
/// The belongings' passphrase, which opens the person's key (secret).
pub const KEY_PASSPHRASE: &str = "seal-key-passphrase";
/// The same, after the one given did not open the key (secret).
pub const KEY_PASSPHRASE_WRONG: &str = "seal-key-passphrase-wrong";
/// Someone's `age1pq1…` key, pasted (not secret: a public key).
pub const RECIPIENT: &str = "seal-recipient";

/// Every question this asks — what the doors route back to the view that
/// asked.
pub const QUESTIONS: [&str; 7] = [
    PASSPHRASE_NEW,
    PASSPHRASE_AGAIN,
    PASSPHRASE,
    PASSPHRASE_WRONG,
    KEY_PASSPHRASE,
    KEY_PASSPHRASE_WRONG,
    RECIPIENT,
];

/// How the header of a `.age` is read: its first bytes, which hold every
/// recipient's stanza (a post-quantum one is about 1.6 KiB).
const HEAD_BYTES: u64 = 64 * 1024;

/// The belongings' sealed key, in their clone.
const KEY_FILE: &str = "key.age";

/// For whom a seal is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The person: their post-quantum recipient, from the belongings.
    Myself,
    /// A passphrase.
    Passphrase,
    /// Someone's `age1pq1…` key.
    Someone,
}

impl Mode {
    /// Every one, in the question's order.
    pub const ALL: [Self; 3] = [Self::Myself, Self::Passphrase, Self::Someone];

    /// Its id, as a choice says it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Myself => "myself",
            Self::Passphrase => "passphrase",
            Self::Someone => "someone",
        }
    }

    /// The one named `id`.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    /// Its label's key (`seal-mode-<id>`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Myself => key::SEAL_MODE_MYSELF,
            Self::Passphrase => key::SEAL_MODE_PASSPHRASE,
            Self::Someone => key::SEAL_MODE_SOMEONE,
        }
    }
}

/// What the security centre's file picker is up for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pick {
    /// Files to seal.
    Files,
    /// A folder to seal (as `name.tar.age`).
    Folder,
    /// A `.age` to open.
    Open,
}

/// The person's own recipient, and why there is none.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mine {
    /// Their `age1pq1…`.
    Ready(String),
    /// No belongings set up.
    #[default]
    NoBelongings,
    /// Belongings, and no recipient here yet: this machine has not synced
    /// them.
    NotSynced,
}

impl Mine {
    /// What it says, as a value (`seal-mine-<id>`).
    #[must_use]
    pub const fn value(&self) -> &'static str {
        match self {
            Self::Ready(_) => key::SEAL_MINE_READY,
            Self::NoBelongings => key::SEAL_MINE_NO_BELONGINGS,
            Self::NotSynced => key::SEAL_MINE_NOT_SYNCED,
        }
    }
}

/// The person's recipient, read from the belongings' state. Two small files
/// of the person's own folders.
#[must_use]
pub fn mine() -> Mine {
    ronoctua_belongings::places::Places::from_env().map_or(Mine::NoBelongings, |p| mine_in(&p))
}

/// [`mine`], under `places`.
#[must_use]
pub fn mine_in(places: &ronoctua_belongings::places::Places) -> Mine {
    let configured = std::fs::read_to_string(places.prefs())
        .ok()
        .and_then(|t| ronoctua_belongings::prefs::Belongings::from_file(&t))
        .is_some_and(|b| b.has_repository());
    if !configured {
        return Mine::NoBelongings;
    }
    // THE BELONGINGS SAY WHERE THEIR LAST SYNC LEFT IT (`fleet::recipient`,
    // `Places::recipient`): the file is named in one place, theirs.
    ronoctua_belongings::fleet::recipient(places)
        .filter(|t| ronoctua_pq::is_recipient(t))
        .map_or(Mine::NotSynced, Mine::Ready)
}

/// The belongings' sealed key, when this machine has one.
#[must_use]
pub fn key_file() -> Option<PathBuf> {
    ronoctua_belongings::places::Places::from_env().and_then(|p| key_file_in(&p))
}

/// [`key_file`], under `places`.
#[must_use]
pub fn key_file_in(places: &ronoctua_belongings::places::Places) -> Option<PathBuf> {
    let key = places.repo().join(KEY_FILE);
    key.is_file().then_some(key)
}

/// Why something was not sealed or opened — a sentence each
/// (`seal-refusal-<id>`, naming the file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// "For you" with no belongings.
    NoBelongings,
    /// "For you" with belongings this machine has not synced yet.
    NotSynced,
    /// What was pasted is not an `age1pq1…` key.
    NotARecipient,
    /// Sealed to a key, and no belongings key here.
    NoKey,
    /// Post-quantum and classical recipients together.
    Mixed,
    /// Sealed to a classical key alone: no key of the person's opens it.
    Classical,
    /// Not an age file.
    NotSealed,
    /// It could not be read.
    Unreadable,
    /// No free name beside it.
    NoName,
    /// The two passphrases differ.
    Differ,
    /// The queue is not up.
    Offline,
    /// The job could not be handed over.
    Failed,
}

impl Refusal {
    /// Every one — what the test of the sentences walks.
    #[cfg(test)]
    pub const ALL: [Self; 12] = [
        Self::NoBelongings,
        Self::NotSynced,
        Self::NotARecipient,
        Self::NoKey,
        Self::Mixed,
        Self::Classical,
        Self::NotSealed,
        Self::Unreadable,
        Self::NoName,
        Self::Differ,
        Self::Offline,
        Self::Failed,
    ];

    /// Its sentence's key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::NoBelongings => key::SEAL_REFUSAL_NO_BELONGINGS,
            Self::NotSynced => key::SEAL_REFUSAL_NOT_SYNCED,
            Self::NotARecipient => key::SEAL_REFUSAL_NOT_A_RECIPIENT,
            Self::NoKey => key::SEAL_REFUSAL_NO_KEY,
            Self::Mixed => key::SEAL_REFUSAL_MIXED,
            Self::Classical => key::SEAL_REFUSAL_CLASSICAL,
            Self::NotSealed => key::SEAL_REFUSAL_NOT_SEALED,
            Self::Unreadable => key::SEAL_REFUSAL_UNREADABLE,
            Self::NoName => key::SEAL_REFUSAL_NO_NAME,
            Self::Differ => key::SEAL_REFUSAL_DIFFER,
            Self::Offline => key::SEAL_REFUSAL_OFFLINE,
            Self::Failed => key::SEAL_REFUSAL_FAILED,
        }
    }

    /// The sentence, naming `path`.
    #[must_use]
    pub fn said(self, path: &Path) -> Explanation {
        Explanation {
            key: self.key(),
            args: vec![("name", name_of(path))],
        }
    }
}

/// The sentence of the queue's refusal (`seal-refused-<id>`), naming the
/// file — the transfer panel's row and the manager's line say the same.
#[must_use]
pub const fn why_key(why: ronoctua_pq::Why) -> &'static str {
    use ronoctua_pq::Why;
    match why {
        Why::WrongKey => key::SEAL_REFUSED_WRONG_KEY,
        Why::WrongPassphrase => key::SEAL_REFUSED_WRONG_PASSPHRASE,
        Why::Corrupt => key::SEAL_REFUSED_CORRUPT,
        Why::Mixed => key::SEAL_REFUSED_MIXED,
        Why::Failed => key::SEAL_REFUSED_FAILED,
    }
}

/// A path's last part, for a sentence.
#[must_use]
pub fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// A name nothing in `dir` uses: `stem<tail>`, or `stem (2)<tail>`.
fn free(dir: &Path, stem: &str, tail: &str) -> Option<PathBuf> {
    let taken = |p: &Path| p.symlink_metadata().is_ok();
    let wanted = dir.join(format!("{stem}{tail}"));
    if !taken(&wanted) {
        return Some(wanted);
    }
    (2..10_000u32)
        .map(|n| dir.join(format!("{stem} ({n}){tail}")))
        .find(|p| !taken(p))
}

/// Where sealing `from` writes: `name.age` beside a file, `name.tar.age`
/// beside a folder — or `name (2).age`, never a name that is there. `None`
/// for a path with no name (`/`) or one that is not text.
#[must_use]
pub fn sealed_target(from: &Path) -> Option<PathBuf> {
    let dir = from.parent()?;
    let name = from.file_name()?.to_str()?;
    if from.is_dir() {
        free(dir, name, ".tar.age")
    } else {
        // `note.txt` → `note.txt.age`, and a second `note (2).txt.age`: the
        // number goes before the extension the person sees.
        let (stem, extension) = split_extension(name);
        free(dir, stem, &format!("{extension}.age"))
    }
}

/// Where opening `from` writes: `note.txt.age` → `note.txt`,
/// `photos.tar.age` → `photos.tar` — or `note (2).txt`, never a name that is
/// there. `None` for a name that does not end in `.age`, or is not text.
#[must_use]
pub fn opened_target(from: &Path) -> Option<PathBuf> {
    let dir = from.parent()?;
    let name = from.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let plain = lower
        .strip_suffix(".age")
        .map(|kept| &name[..kept.len()])
        .filter(|p| !p.is_empty())?;
    let (stem, extension) = if let Some(stem) = plain.strip_suffix(".tar") {
        (stem, ".tar")
    } else {
        split_extension(plain)
    };
    free(dir, stem, extension)
}

/// `note.txt` → (`note`, `.txt`); a name with no extension, or a dot file,
/// keeps its whole name as the stem.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 => name.split_at(at),
        _ => (name, ""),
    }
}

/// What opens a `.age`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Opening {
    /// Its passphrase.
    Passphrase,
    /// The belongings' post-quantum key.
    Key,
}

/// What a header says opens it, or why nothing here does.
///
/// # Errors
///
/// [`Refusal::Mixed`], [`Refusal::Classical`] (the person's key is
/// post-quantum alone: asking its passphrase would only end in "wrong key"),
/// [`Refusal::NotSealed`].
pub const fn opening(kind: Kind) -> Result<Opening, Refusal> {
    match kind {
        Kind::Passphrase => Ok(Opening::Passphrase),
        Kind::Pq => Ok(Opening::Key),
        Kind::Classical => Err(Refusal::Classical),
        Kind::Mixed => Err(Refusal::Mixed),
        Kind::NotAge => Err(Refusal::NotSealed),
    }
}

/// What the header of `path` says — `None` when it could not be read, or is
/// not a file: a pipe named `x.age` would hold the thread forever, and a
/// device has no header. BLOCKS on the file: a door's thread (a `.age` on a
/// stick, or in a vault that is suspended, is not the loop's to wait on).
#[must_use]
pub fn sniff_now(path: &Path) -> Option<Kind> {
    use std::io::Read as _;
    // A LINK IS FOLLOWED, as the manager shows what it points at — and what
    // it points at must be a file.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    // AND THE LOOK ABOVE IS NOT TRUSTED to still be true: a pipe swapped in
    // since opens at once (`NONBLOCK`) and is refused by what the descriptor
    // itself is (`git_browser`'s rule).
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let file = std::fs::File::from(fd);
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut head = Vec::new();
    file.take(HEAD_BYTES).read_to_end(&mut head).ok()?;
    Some(ronoctua_pq::sniff(&head))
}

/// How a seal is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    /// To these recipients (the person's, or someone's).
    To(Vec<String>),
    /// To this passphrase.
    Passphrase(Secret),
}

/// What a `.age` is opened with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum With {
    /// Its passphrase.
    Passphrase(Secret),
    /// The belongings' key, opened with their passphrase.
    Key(Secret),
}

/// What a view hands its door for the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealDoing {
    /// Seal each of these, the same way.
    Seal {
        /// What.
        paths: Vec<PathBuf>,
        /// How.
        how: How,
    },
    /// Open this.
    Open {
        /// What.
        path: PathBuf,
        /// With what.
        with: With,
    },
}

/// What a view is waiting for an answer about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanting {
    /// A new passphrase, the first time, for these.
    New(Vec<PathBuf>),
    /// The same again, with the first.
    Again(Vec<PathBuf>, Secret),
    /// Someone's key, for these.
    Recipient(Vec<PathBuf>),
    /// What opens this file.
    Open(PathBuf, Opening),
}

/// What a view does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Nothing.
    Nothing,
    /// Ask on the door's line — masked when `secret` — and wait as `wanting`.
    Ask {
        /// The question's id (`ask-<id>`).
        id: &'static str,
        /// Masked.
        secret: bool,
        /// What the answer is for.
        wanting: Wanting,
    },
    /// Hand this to the queue.
    Do(SealDoing),
    /// Say this.
    Say(Explanation),
}

/// The first path's name, for a sentence about a set.
fn first_of(paths: &[PathBuf]) -> PathBuf {
    paths.first().cloned().unwrap_or_default()
}

/// The person chose `mode` for `paths`.
#[must_use]
pub fn choose(mode: Mode, paths: Vec<PathBuf>) -> Next {
    if paths.is_empty() {
        return Next::Nothing;
    }
    match mode {
        Mode::Myself => match mine() {
            Mine::Ready(me) => Next::Do(SealDoing::Seal {
                paths,
                how: How::To(vec![me]),
            }),
            Mine::NoBelongings => Next::Say(Refusal::NoBelongings.said(&first_of(&paths))),
            Mine::NotSynced => Next::Say(Refusal::NotSynced.said(&first_of(&paths))),
        },
        Mode::Passphrase => Next::Ask {
            id: PASSPHRASE_NEW,
            secret: true,
            wanting: Wanting::New(paths),
        },
        Mode::Someone => Next::Ask {
            id: RECIPIENT,
            secret: false,
            wanting: Wanting::Recipient(paths),
        },
    }
}

/// The answer `text` to the question `id`, which was asked for `wanting`.
/// An empty answer lets the question go.
#[must_use]
pub fn answer(wanting: Wanting, id: &str, text: &str) -> Next {
    if text.is_empty() {
        return Next::Nothing;
    }
    match (id, wanting) {
        (PASSPHRASE_NEW, Wanting::New(paths)) => Next::Ask {
            id: PASSPHRASE_AGAIN,
            secret: true,
            wanting: Wanting::Again(paths, Secret::new(text)),
        },
        (PASSPHRASE_AGAIN, Wanting::Again(paths, first)) => {
            if first.expose() == text {
                Next::Do(SealDoing::Seal {
                    paths,
                    how: How::Passphrase(first),
                })
            } else {
                Next::Say(Refusal::Differ.said(&first_of(&paths)))
            }
        }
        (RECIPIENT, Wanting::Recipient(paths)) => {
            let key = text.trim();
            if ronoctua_pq::is_recipient(key) {
                Next::Do(SealDoing::Seal {
                    paths,
                    how: How::To(vec![key.to_ascii_lowercase()]),
                })
            } else {
                Next::Say(Refusal::NotARecipient.said(&first_of(&paths)))
            }
        }
        (PASSPHRASE | PASSPHRASE_WRONG, Wanting::Open(path, Opening::Passphrase)) => {
            Next::Do(SealDoing::Open {
                path,
                with: With::Passphrase(Secret::new(text)),
            })
        }
        (KEY_PASSPHRASE | KEY_PASSPHRASE_WRONG, Wanting::Open(path, Opening::Key)) => {
            Next::Do(SealDoing::Open {
                path,
                with: With::Key(Secret::new(text)),
            })
        }
        _ => Next::Nothing,
    }
}

/// What the header of `path` said (`None`: it could not be read): the
/// question that opens it, or why nothing here does.
#[must_use]
pub fn sniffed(path: PathBuf, kind: Option<Kind>) -> Next {
    let Some(kind) = kind else {
        return Next::Say(Refusal::Unreadable.said(&path));
    };
    match opening(kind) {
        Ok(Opening::Passphrase) => Next::Ask {
            id: PASSPHRASE,
            secret: true,
            wanting: Wanting::Open(path, Opening::Passphrase),
        },
        Ok(Opening::Key) if key_file().is_some() => Next::Ask {
            id: KEY_PASSPHRASE,
            secret: true,
            wanting: Wanting::Open(path, Opening::Key),
        },
        Ok(Opening::Key) => Next::Say(Refusal::NoKey.said(&path)),
        Err(refusal) => Next::Say(refusal.said(&path)),
    }
}

/// The queue refused to open `path` for `why`: asked again when it was the
/// passphrase, said otherwise.
#[must_use]
pub fn refused(path: PathBuf, opening: Option<Opening>, why: ronoctua_pq::Why) -> Next {
    match (why, opening) {
        (ronoctua_pq::Why::WrongPassphrase, Some(Opening::Passphrase)) => Next::Ask {
            id: PASSPHRASE_WRONG,
            secret: true,
            wanting: Wanting::Open(path, Opening::Passphrase),
        },
        (ronoctua_pq::Why::WrongPassphrase, Some(Opening::Key)) => Next::Ask {
            id: KEY_PASSPHRASE_WRONG,
            secret: true,
            wanting: Wanting::Open(path, Opening::Key),
        },
        _ => Next::Say(Explanation {
            key: why_key(why),
            args: vec![("name", name_of(&path))],
        }),
    }
}

/// What opens the `.age` a job was opening — `None` for a seal.
#[must_use]
pub const fn opening_of(job: &ronoctua_pq::Job) -> Option<Opening> {
    match job.action {
        ronoctua_pq::Action::OpenPassphrase { .. } => Some(Opening::Passphrase),
        ronoctua_pq::Action::OpenWithKey { .. } | ronoctua_pq::Action::Open { .. } => {
            Some(Opening::Key)
        }
        ronoctua_pq::Action::SealTo { .. } | ronoctua_pq::Action::SealPassphrase { .. } => None,
    }
}

/// How many jobs one view remembers handing the queue. Only the ones still
/// on it: each leaves when its job ends ([`SealReport::Ended`], or its
/// refusal), so this bounds a selection larger than anybody makes, and the
/// oldest goes first.
pub const SENT_MOST: usize = 1024;

/// The seals and opens one view handed the queue, by what each was about —
/// so that a refusal is said, or a wrong passphrase asked again, in the view
/// that sent the job, and only there: another door open on the same folder
/// hears the same end and says nothing (the archives' rule,
/// `files_archive`). A SEAL is kept as well as an open: its refusal (a file
/// that cannot be read, a folder that is not writable) is a sentence the
/// person is owed where they asked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sent(std::collections::VecDeque<(PathBuf, bool)>);

impl Sent {
    /// `doing` went to the queue: each of its files, and whether it opens.
    pub fn record(&mut self, doing: &SealDoing) {
        let (paths, opens) = match doing {
            SealDoing::Seal { paths, .. } => (paths.as_slice(), false),
            SealDoing::Open { path, .. } => (std::slice::from_ref(path), true),
        };
        for path in paths {
            self.0.retain(|(p, o)| !(p == path && *o == opens));
            if self.0.len() >= SENT_MOST {
                self.0.pop_front();
            }
            self.0.push_back((path.clone(), opens));
        }
    }

    /// The job on `from` (an open when `opens`) ended: whether this view
    /// sent it. Either way it is not kept any longer.
    pub fn claim(&mut self, from: &Path, opens: bool) -> bool {
        let Some(at) = self.0.iter().position(|(p, o)| p == from && *o == opens) else {
            return false;
        };
        self.0.remove(at);
        true
    }
}

/// A job of the queue ended as `report` says: what the view that keeps
/// `sent` does next — the question again (a wrong passphrase), or the
/// refusal said, naming the file — when the job was its own. `None` when it
/// was somebody else's, when it ended without a refusal (which only lets go
/// of it), or for a header read, which is no end of a job. The one rule both
/// views follow ([`crate::files::View::seal_reported`],
/// [`crate::security::View::came`]).
pub fn job_ended(sent: &mut Sent, report: &SealReport) -> Option<Next> {
    match report {
        SealReport::Sniffed { .. } => None,
        SealReport::Refused { from, opening, why } => sent
            .claim(from, opening.is_some())
            .then(|| refused(from.clone(), *opening, *why)),
        SealReport::Ended { from, opens } => {
            sent.claim(from, *opens);
            None
        }
    }
}

/// The file manager's two questions about a seal, on its own list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealAsked {
    /// For whom, for these.
    Mode(Vec<PathBuf>),
    /// Someone's key, for these — typed (pasted), the query being the answer.
    Recipient(Vec<PathBuf>),
}

impl SealAsked {
    /// The question's title.
    #[must_use]
    pub fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Mode(paths) => {
                texts.t_with(key::SEAL_ASK_MODE, &[("what", &what_of(paths, texts))])
            }
            Self::Recipient(_) => texts.t(key::ASK_SEAL_RECIPIENT),
        }
    }
}

/// What a set is called in a sentence: its name, or how many.
#[must_use]
pub fn what_of(paths: &[PathBuf], texts: &ronoctua_i18n::Texts) -> String {
    if let [only] = paths {
        name_of(only)
    } else {
        texts.t_with(key::FILES_ITEMS, &[("n", &paths.len().to_string())])
    }
}

/// The three ways, as the file manager's list offers them — "for you" with
/// what it can do now beside it (a row offered and then refused says why
/// on its own face).
#[must_use]
pub fn mode_choices(texts: &ronoctua_i18n::Texts) -> Vec<crate::files::Choice> {
    let mine = mine();
    Mode::ALL
        .into_iter()
        .map(|m| crate::files::Choice {
            id: m.id().to_owned(),
            // THE SEPARATOR IS THE TOKEN'S, with its air on both sides —
            // the one every other seam on screen draws (`chrome::Piece`).
            label: match m {
                Mode::Myself => format!(
                    "{} {} {}",
                    texts.t(m.label()),
                    ronoctua_tokens::mark::SEPARATOR,
                    texts.t(mine.value())
                ),
                _ => texts.t(m.label()),
            },
        })
        .collect()
}

/// What came back to the file manager about a seal, from a thread or from
/// the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealReport {
    /// The header of `path` was read (`None`: it could not be).
    Sniffed {
        /// The `.age`.
        path: PathBuf,
        /// What it says.
        kind: Option<Kind>,
    },
    /// The queue refused a job on `from`.
    Refused {
        /// What was sealed or opened.
        from: PathBuf,
        /// What opened it, for an open.
        opening: Option<Opening>,
        /// Why.
        why: ronoctua_pq::Why,
    },
    /// A job on `from` ended with no refusal — done, stopped, or a program
    /// that died without a verdict (whose failure is said as any job's is):
    /// the view that sent it lets go of it.
    Ended {
        /// What was sealed or opened.
        from: PathBuf,
        /// It was an open.
        opens: bool,
    },
}

/// A passphrase written for one job, removed when this is dropped unless the
/// queue took the job ([`Written::hand_over`]).
struct Written(Option<PathBuf>);

impl Written {
    fn of(secret: Option<&Secret>) -> Result<Self, Refusal> {
        match secret {
            None => Ok(Self(None)),
            Some(s) => ronoctua_transfers::secrets::keep(s.expose())
                .map(|p| Self(Some(p)))
                .map_err(|e| {
                    tracing::warn!(error = %e, "a passphrase could not be written for a seal");
                    Refusal::Failed
                }),
        }
    }

    fn path(&self) -> PathBuf {
        self.0.clone().unwrap_or_default()
    }

    fn hand_over(mut self) {
        self.0 = None;
    }
}

impl Drop for Written {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            ronoctua_transfers::secrets::forget(&p);
        }
    }
}

/// One job for the queue and the passphrase file it carries — or why not.
type Made = Result<(Work, Written), Refusal>;

/// One job for the queue — the work and the secret it carries — or why not.
///
/// # Errors
///
/// [`Refusal::NoName`], [`Refusal::NoKey`], [`Refusal::Failed`].
fn job_of(
    from: &Path,
    action: impl FnOnce(PathBuf) -> ronoctua_pq::Action,
    secret: Option<&Secret>,
    seals: bool,
) -> Made {
    let into = if seals {
        sealed_target(from)
    } else {
        opened_target(from)
    }
    .ok_or(Refusal::NoName)?;
    let written = Written::of(secret)?;
    let job = ronoctua_pq::Job {
        action: action(written.path()),
        from: from.to_path_buf(),
        into,
    };
    Ok((
        Work::Seal {
            job: Box::new(job),
            label: name_of(from),
        },
        written,
    ))
}

/// The jobs `doing` makes, one per file.
fn jobs_of(doing: &SealDoing) -> Vec<(PathBuf, Made)> {
    match doing {
        SealDoing::Seal { paths, how } => paths
            .iter()
            .map(|from| {
                let made = match how {
                    How::To(recipients) => job_of(
                        from,
                        |_| ronoctua_pq::Action::SealTo {
                            recipients: recipients.clone(),
                        },
                        None,
                        true,
                    ),
                    How::Passphrase(secret) => job_of(
                        from,
                        |file| ronoctua_pq::Action::SealPassphrase {
                            passphrase_file: file,
                            work: None,
                        },
                        Some(secret),
                        true,
                    ),
                };
                (from.clone(), made)
            })
            .collect(),
        SealDoing::Open { path, with } => {
            let made = match with {
                With::Passphrase(secret) => job_of(
                    path,
                    |file| ronoctua_pq::Action::OpenPassphrase {
                        passphrase_file: file,
                    },
                    Some(secret),
                    false,
                ),
                With::Key(secret) => match key_file() {
                    Some(key_file) => job_of(
                        path,
                        |file| ronoctua_pq::Action::OpenWithKey {
                            key_file,
                            passphrase_file: file,
                        },
                        Some(secret),
                        false,
                    ),
                    None => Err(Refusal::NoKey),
                },
            };
            vec![(path.clone(), made)]
        }
    }
}

/// Hands `doing` to the queue, a job per file, each passphrase written into
/// that job's own `0600` file on the way — and answers what to say. BLOCKS on
/// the folder (a free name is asked of it, and a folder in a vault that is
/// suspended does not answer) and on the queue's socket: a door's thread.
#[must_use]
pub fn carry_out(doing: SealDoing) -> Explanation {
    let opens = matches!(doing, SealDoing::Open { .. });
    let mut handed: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (from, made) in jobs_of(&doing) {
        let (work, written) = match made {
            Ok(m) => m,
            Err(refusal) => return refusal.said(&from),
        };
        let into = work.landing().map(Path::to_path_buf).unwrap_or_default();
        match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
            Ok(()) => {
                written.hand_over();
                handed.push((from, into));
            }
            Err(e) => {
                tracing::warn!(error = %e, "the transfer queue did not take a seal");
                return Refusal::Offline.said(&from);
            }
        }
    }
    match handed.as_slice() {
        [(from, into)] => Explanation {
            key: if opens {
                key::SEAL_QUEUED_OPEN
            } else {
                key::SEAL_QUEUED_SEAL
            },
            args: vec![("name", name_of(from)), ("into", name_of(into))],
        },
        many => Explanation {
            key: key::SEAL_QUEUED_MANY,
            args: vec![("n", many.len().to_string())],
        },
    }
}

/// The files the picker shows a person who opens a `.age`: those, by their
/// name — the header decides the rest.
pub const SEALED: crate::file_picker::Filter = crate::file_picker::Filter {
    label: key::SEAL_FILTER_SEALED,
    keeps: |path, _| {
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("age"))
            .then_some(ronoctua_tokens::icon::FILE)
    },
};

/// The ring of the picker that opens a `.age`.
pub const SEALED_FILES: &[crate::file_picker::Filter] = &[SEALED, crate::file_picker::Filter::ANY];

#[cfg(test)]
mod tests {
    use super::*;

    fn place(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("ronoctua-seal-shell-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// FAILS IF: a seal or an open is pointed at a name that is there — the
    /// door's half of "never over anything" — or a folder is not named as
    /// the tar it becomes.
    #[test]
    fn a_target_is_a_name_nothing_uses() {
        let dir = place("names");
        let note = dir.join("note.txt");
        std::fs::write(&note, b"x").unwrap();
        assert_eq!(sealed_target(&note), Some(dir.join("note.txt.age")));
        std::fs::write(dir.join("note.txt.age"), b"x").unwrap();
        assert_eq!(sealed_target(&note), Some(dir.join("note (2).txt.age")));
        let photos = dir.join("photos");
        std::fs::create_dir(&photos).unwrap();
        assert_eq!(sealed_target(&photos), Some(dir.join("photos.tar.age")));
        let readme = dir.join("README");
        std::fs::write(&readme, b"x").unwrap();
        assert_eq!(sealed_target(&readme), Some(dir.join("README.age")));
        // OPENING: the name without `.age`, the number before the extension.
        let sealed = dir.join("note.txt.age");
        assert_eq!(opened_target(&sealed), Some(dir.join("note (2).txt")));
        assert_eq!(
            opened_target(&dir.join("photos.tar.age")),
            Some(dir.join("photos.tar"))
        );
        std::fs::write(dir.join("photos.tar"), b"x").unwrap();
        assert_eq!(
            opened_target(&dir.join("photos.tar.age")),
            Some(dir.join("photos (2).tar"))
        );
        assert_eq!(opened_target(&dir.join("A.AGE")), Some(dir.join("A")));
        for nothing in [dir.join("note.txt"), dir.join(".age"), PathBuf::from("/")] {
            assert_eq!(opened_target(&nothing), None, "{}", nothing.display());
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a header is answered with the wrong question — a
    /// passphrase asked of a file sealed to a key, a mixed file opened, or
    /// the belongings' passphrase asked for a file sealed to a classical key
    /// that no key of the person's opens.
    #[test]
    fn the_header_picks_the_question() {
        assert_eq!(opening(Kind::Passphrase), Ok(Opening::Passphrase));
        assert_eq!(opening(Kind::Pq), Ok(Opening::Key));
        assert_eq!(opening(Kind::Classical), Err(Refusal::Classical));
        assert_eq!(opening(Kind::Mixed), Err(Refusal::Mixed));
        assert_eq!(opening(Kind::NotAge), Err(Refusal::NotSealed));
        let path = PathBuf::from("/home/p/a.age");
        assert!(matches!(
            sniffed(path.clone(), Some(Kind::Passphrase)),
            Next::Ask {
                id: PASSPHRASE,
                secret: true,
                ..
            }
        ));
        assert!(matches!(sniffed(path.clone(), None), Next::Say(_)));
        assert!(matches!(sniffed(path, Some(Kind::Mixed)), Next::Say(_)));
    }

    /// FAILS IF: `U` reads anything but a file — a pipe named `x.age` would
    /// hold the reading thread for ever with nothing said on screen, and a
    /// device behind a link has no header — or a file's header is not read.
    #[test]
    fn only_a_file_s_header_is_read() {
        let dir = place("sniff");
        let pipe = dir.join("pipe.age");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &pipe,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        let device = dir.join("zero.age");
        std::os::unix::fs::symlink("/dev/zero", &device).unwrap();
        let sealed = dir.join("note.txt.age");
        std::fs::write(
            &sealed,
            b"age-encryption.org/v1\n-> scrypt c2FsdA 18\nYmFk\n--- x\n",
        )
        .unwrap();
        for (path, kind) in [
            (pipe, None),
            (device, None),
            (sealed, Some(Kind::Passphrase)),
        ] {
            let (said, heard) = std::sync::mpsc::channel();
            let asked = path.clone();
            std::thread::spawn(move || {
                let _ = said.send(sniff_now(&asked));
            });
            assert_eq!(
                heard
                    .recv_timeout(std::time::Duration::from_secs(20))
                    .unwrap_or_else(|_| panic!("{} held the thread", path.display())),
                kind,
                "{}",
                path.display()
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a passphrase is taken after one typing, or two that differ
    /// seal anything; or a pasted key that is not one is queued; or a wrong
    /// passphrase from the queue is not asked again.
    #[test]
    fn the_questions_come_in_order_and_refuse_what_cannot_be_used() {
        let paths = vec![PathBuf::from("/home/p/a.txt")];
        let Next::Ask { id, wanting, .. } = choose(Mode::Passphrase, paths.clone()) else {
            panic!("a passphrase was not asked");
        };
        assert_eq!(id, PASSPHRASE_NEW);
        let Next::Ask { id, wanting, .. } = answer(wanting, id, "horse") else {
            panic!("the passphrase was not asked again");
        };
        assert_eq!(id, PASSPHRASE_AGAIN);
        assert!(matches!(
            answer(wanting.clone(), id, "hose"),
            Next::Say(Explanation {
                key: key::SEAL_REFUSAL_DIFFER,
                ..
            })
        ));
        assert_eq!(
            answer(wanting, id, "horse"),
            Next::Do(SealDoing::Seal {
                paths: paths.clone(),
                how: How::Passphrase(Secret::new("horse")),
            })
        );
        let Next::Ask {
            id,
            secret,
            wanting,
        } = choose(Mode::Someone, paths.clone())
        else {
            panic!("a key was not asked");
        };
        assert!(!secret, "a public key is not a secret");
        assert!(matches!(
            answer(wanting, id, "age1pq1nope"),
            Next::Say(Explanation {
                key: key::SEAL_REFUSAL_NOT_A_RECIPIENT,
                ..
            })
        ));
        assert!(matches!(
            refused(
                PathBuf::from("/a.age"),
                Some(Opening::Key),
                ronoctua_pq::Why::WrongPassphrase
            ),
            Next::Ask {
                id: KEY_PASSPHRASE_WRONG,
                ..
            }
        ));
        assert!(matches!(
            refused(PathBuf::from("/a.age"), None, ronoctua_pq::Why::Failed),
            Next::Say(_)
        ));
        assert_eq!(
            answer(Wanting::New(paths), PASSPHRASE_NEW, ""),
            Next::Nothing
        );
    }

    /// FAILS IF: "for you" is offered as ready without the belongings'
    /// post-quantum recipient, or the recipient is read from anything else.
    #[test]
    fn for_you_is_the_belongings_recipient_or_says_why_not() {
        let root = place("mine");
        let places = ronoctua_belongings::places::Places::under(&root);
        assert_eq!(mine_in(&places), Mine::NoBelongings);
        std::fs::create_dir_all(places.prefs().parent().unwrap()).unwrap();
        std::fs::write(
            places.prefs(),
            ronoctua_belongings::prefs::Belongings::with_repository("git@example.org:me/b.git")
                .to_file(),
        )
        .unwrap();
        assert_eq!(mine_in(&places), Mine::NotSynced);
        std::fs::create_dir_all(places.own()).unwrap();
        std::fs::write(places.recipient(), "age1pq1notakey\n").unwrap();
        assert_eq!(mine_in(&places), Mine::NotSynced, "a bad key was taken");
        assert_eq!(key_file_in(&places), None);
        let _ = std::fs::remove_dir_all(root);
    }

    /// What the security centre's page draws, as text.
    fn drawn(rows: &[crate::security::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// FAILS IF: the post-quantum section leaves a channel out, draws one
    /// without its status and its sentence, or a key is missing in a
    /// language — under the facts of a classical machine and of a
    /// post-quantum one, with the machine's secrets and without.
    #[test]
    fn the_quantum_section_draws_every_channel_in_both_languages() {
        use ronoctua_security::quantum::{self, Belongings, Built, Facts, Syncthing, Tls};
        let shipped = Built {
            openssh: Some("10.4p1".into()),
            openssl: Some("3.6.3".into()),
            chrome: Some("151.0.7922.71".into()),
            syncthing: Some(Syncthing {
                version: "2.1.1".into(),
                go: "1.26.5".into(),
                go_mod: "1.25.0".into(),
                pins_curves: false,
            }),
            tor: Some("0.4.9.11".into()),
            tailscale: Some("1.98.10".into()),
            ships: None,
        };
        let machines = [
            Facts::default(),
            Facts {
                belongings: Belongings::PostQuantum,
                root_disk: Some(true),
                tls: Tls {
                    quantum: 3,
                    classical: 2,
                    classical_hosts: vec!["old.example".into(), "older.example".into()],
                },
                ssh_kex: quantum::SSH_PQ_KEX.map(ToOwned::to_owned).to_vec(),
                sops: true,
                built: shipped,
            },
        ];
        for language in ["en-GB", "pt-BR"] {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for facts in &machines {
                let scene = crate::security::Scene {
                    read: true,
                    quantum: facts.clone(),
                    ..crate::security::Scene::default()
                };
                let view = crate::security::View::at(Some(crate::security::Section::Quantum));
                let rows = view.rows(&scene, 120, &texts, &ronoctua_files::when::Zone::utc());
                let page = drawn(&rows);
                assert!(!page.contains('['), "{language}: a key is missing: {page}");
                for reading in quantum::channels(facts) {
                    let label = texts.t(&format!(
                        "security-quantum-channel-{}",
                        reading.channel.id()
                    ));
                    let status =
                        texts.t(&format!("security-quantum-status-{}", reading.status.id()));
                    // UNDER THE SECTION'S HEADING: another section may say
                    // the same word (the vault's own is `/private`).
                    let heading = texts.t(key::SECURITY_HEADING_QUANTUM);
                    let from = rows
                        .iter()
                        .position(|r| r.fields.iter().any(|f| f.text == heading))
                        .unwrap_or_else(|| panic!("{language}: no post-quantum heading"));
                    let at = rows
                        .iter()
                        .skip(from)
                        .position(|r| r.fields.iter().any(|f| f.text == label))
                        .map(|n| n + from)
                        .unwrap_or_else(|| panic!("{language}: no row for {:?}", reading.channel));
                    assert!(
                        rows[at].fields.iter().any(|f| f.text == status),
                        "{language}: {:?} without its status",
                        reading.channel
                    );
                    // ITS SENTENCE, with its numbers and names, starting on
                    // the line right after it (a note wraps, never cuts).
                    let sentence = crate::security::quantum_sentence(&reading, &texts);
                    assert!(
                        !sentence.contains('{') && !sentence.starts_with('['),
                        "{language}: {:?} has a hole: {sentence}",
                        reading.why
                    );
                    let next: String = rows
                        .get(at + 1)
                        .map(|r| r.fields.iter().map(|f| f.text.trim()).collect())
                        .unwrap_or_default();
                    assert!(
                        !next.is_empty() && sentence.starts_with(&next),
                        "{language}: {:?} without its sentence: {next:?} / {sentence:?}",
                        reading.channel
                    );
                }
            }
        }
    }

    /// FAILS IF: a count reaches its sentence as text — "every one of the 1
    /// hosts heard", the state right after the first request — or a plural
    /// branch is missing in a language.
    #[test]
    fn one_host_is_said_as_one_in_both_languages() {
        use ronoctua_security::quantum::{self, Channel, Facts, Tls};
        let heard = |quantum: usize, classical: usize| {
            let facts = Facts {
                tls: Tls {
                    quantum,
                    classical,
                    classical_hosts: vec!["old.example".into(); classical],
                },
                ..Facts::default()
            };
            quantum::channels(&facts)
                .into_iter()
                .find(|r| r.channel == Channel::Https)
                .unwrap()
        };
        for (language, one, one_old, some_old) in [
            (
                "en-GB",
                "The one host heard agreed X25519MLKEM768.",
                "The one host heard still agrees a classical key: old.example.",
                "One of the 3 hosts heard still agrees a classical key: old.example.",
            ),
            (
                "pt-BR",
                "O único host ouvido combinou X25519MLKEM768.",
                "O único host ouvido ainda combina uma chave clássica: old.example.",
                "Um dos 3 hosts ouvidos ainda combina uma chave clássica: old.example.",
            ),
        ] {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let said = |q, c| crate::security::quantum_sentence(&heard(q, c), &texts);
            assert_eq!(said(1, 0), one, "{language}");
            assert_eq!(said(0, 1), one_old, "{language}");
            assert_eq!(said(2, 1), some_old, "{language}");
            assert!(said(2, 0).contains('2'), "{language}: {}", said(2, 0));
            assert!(said(1, 2).contains("2 "), "{language}: {}", said(1, 2));
        }
    }

    /// FAILS IF: the section counts as post-quantum an SSH key exchange the
    /// belongings' transport refuses, or misses one it takes — the two lists
    /// are one list (`assets/ssh.nix`'s `postQuantum`).
    #[test]
    fn the_section_s_ssh_exchanges_are_the_belongings_ones() {
        assert_eq!(ronoctua_security::quantum::SSH_PQ_KEX, ronoctua_git::PQ_KEX);
    }

    /// FAILS IF: the security centre does not ask again for a passphrase the
    /// queue refused on an open IT sent, or asks again about one somebody
    /// else sent (the file manager's), or asks twice for one refusal.
    #[test]
    fn the_centre_asks_again_only_about_its_own_opens() {
        use crate::security::{Came, Errand, Scene, View};
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("a language");
        let mut view = View::fresh();
        let mut scene = Scene::default();
        let path = PathBuf::from("/home/p/letter.txt.age");
        let out = view.came(
            Came::Sniffed {
                path: path.clone(),
                kind: Some(Kind::Passphrase),
            },
            &mut scene,
            &texts,
        );
        assert!(matches!(
            out.errand,
            Errand::Ask {
                id: PASSPHRASE,
                secret: true,
                ..
            }
        ));
        let sent = view.answered(PASSPHRASE, "not it", &scene, &texts);
        assert!(
            matches!(&sent, Errand::Seal(SealDoing::Open { path: p, .. }) if *p == path),
            "the open was not handed to the queue"
        );
        let refusal = || {
            Came::Sealed(SealReport::Refused {
                from: path.clone(),
                opening: Some(Opening::Passphrase),
                why: ronoctua_pq::Why::WrongPassphrase,
            })
        };
        let somebody_else = Came::Sealed(SealReport::Refused {
            from: PathBuf::from("/home/p/other.age"),
            opening: Some(Opening::Passphrase),
            why: ronoctua_pq::Why::WrongPassphrase,
        });
        assert!(matches!(
            view.came(somebody_else, &mut scene, &texts).errand,
            Errand::Nothing
        ));
        assert!(matches!(
            view.came(refusal(), &mut scene, &texts).errand,
            Errand::Ask {
                id: PASSPHRASE_WRONG,
                secret: true,
                ..
            }
        ));
        assert!(matches!(
            view.came(refusal(), &mut scene, &texts).errand,
            Errand::Nothing
        ));
    }

    /// FAILS IF: the queue's refusal of a SEAL is not said in the view that
    /// sent it — a file that could not be read, a folder not writable: the
    /// person asked, and was told nothing — or it is said in a view that did
    /// not send it, or twice; or a job that ended well is still taken for
    /// this view's, so a later refusal from another door is said here too;
    /// or a seal and an open of the same file are taken for each other.
    #[test]
    fn a_refusal_is_said_in_the_view_that_sent_the_job_and_only_there() {
        let photos = PathBuf::from("/home/p/photos");
        let note = PathBuf::from("/home/p/note.txt");
        let mut sender = Sent::default();
        let mut bystander = Sent::default();
        sender.record(&SealDoing::Seal {
            paths: vec![photos.clone(), note.clone()],
            how: How::Passphrase(Secret::new("horse")),
        });
        let refused = |from: &Path, opening| SealReport::Refused {
            from: from.to_path_buf(),
            opening,
            why: ronoctua_pq::Why::Failed,
        };
        assert_eq!(
            job_ended(&mut bystander, &refused(&photos, None)),
            None,
            "a view that sent nothing said it"
        );
        // AN OPEN of the same name is not this seal.
        assert_eq!(
            job_ended(&mut sender, &refused(&photos, Some(Opening::Passphrase))),
            None
        );
        match job_ended(&mut sender, &refused(&photos, None)) {
            Some(Next::Say(e)) => {
                assert_eq!(e.key, key::SEAL_REFUSED_FAILED);
                assert_eq!(e.args, vec![("name", "photos".to_owned())]);
            }
            other => panic!("the seal's refusal was not said: {other:?}"),
        }
        assert_eq!(
            job_ended(&mut sender, &refused(&photos, None)),
            None,
            "one refusal said twice"
        );
        // ENDED WELL: let go of, so a later refusal is somebody else's.
        let ended = SealReport::Ended {
            from: note.clone(),
            opens: false,
        };
        assert_eq!(job_ended(&mut sender, &ended), None);
        assert_eq!(job_ended(&mut sender, &refused(&note, None)), None);
        assert_eq!(sender, Sent::default(), "a job that ended was kept");
    }

    /// FAILS IF: the queue's end of a seal does not reach the views as the
    /// rule reads it — a refusal as `Refused`, with no opening for a seal;
    /// an end without a refusal as `Ended`, after the report that reads the
    /// folder again (or says the failure).
    #[test]
    fn the_queue_reports_every_end_of_a_seal() {
        use crate::files::Report;
        use ronoctua_transfers::{Engine, Job, Stage};
        let from = PathBuf::from("/home/p/photos");
        let job = |id, stage, why: Option<ronoctua_pq::Why>| {
            let mut j = Job::fresh_one(
                id,
                Engine::Seal,
                Work::Seal {
                    job: Box::new(ronoctua_pq::Job {
                        action: ronoctua_pq::Action::SealTo {
                            recipients: Vec::new(),
                        },
                        from: from.clone(),
                        into: PathBuf::from("/home/p/photos.tar.age"),
                    }),
                    label: "photos".to_owned(),
                },
            );
            j.stage = stage;
            j.seal = why;
            j
        };
        let running = [job(1, Stage::Running, None)];
        let ended = |now: Job| crate::transfers::what_ended(Some(&running), &[now]);
        match ended(job(1, Stage::Failed, Some(ronoctua_pq::Why::Failed))).as_slice() {
            [Report::Seal(r)] => assert_eq!(
                **r,
                SealReport::Refused {
                    from: from.clone(),
                    opening: None,
                    why: ronoctua_pq::Why::Failed,
                }
            ),
            other => panic!("a refused seal reported {other:?}"),
        }
        for stage in [Stage::Done, Stage::Cancelled, Stage::Failed] {
            match ended(job(1, stage, None)).as_slice() {
                [first, Report::Seal(r)] => {
                    assert!(
                        matches!(first, Report::Done | Report::Failed(_)),
                        "{first:?}"
                    );
                    assert_eq!(
                        **r,
                        SealReport::Ended {
                            from: from.clone(),
                            opens: false,
                        }
                    );
                }
                other => panic!("a seal {stage:?} reported {other:?}"),
            }
        }
    }

    /// FAILS IF: the security centre hands a seal to the queue and then says
    /// nothing when the queue refuses it — the case the file manager's
    /// passphrase seal of an unreadable file left silent.
    #[test]
    fn the_centre_says_a_refused_seal_it_sent() {
        use crate::security::{Act, Came, Errand, Press, Scene, View};
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("a language");
        let zone = ronoctua_files::when::Zone::utc();
        let mut scene = Scene::default();
        let file = PathBuf::from("/home/p/unreadable.txt");
        let mut view = View::fresh();
        view.seal_these(vec![file.clone()]);
        let rows = view.rows(&scene, 120, &texts, &zone);
        let at = rows
            .iter()
            .position(|r| r.act == Some(Act::SealMode(Mode::Passphrase)))
            .expect("no 'with a passphrase' row");
        view.clicked(at, &rows);
        assert!(matches!(
            view.press(Press::Enter, &scene, 120, &texts, &zone),
            Errand::Ask {
                id: PASSPHRASE_NEW,
                ..
            }
        ));
        assert!(matches!(
            view.answered(PASSPHRASE_NEW, "horse", &scene, &texts),
            Errand::Ask {
                id: PASSPHRASE_AGAIN,
                ..
            }
        ));
        assert!(matches!(
            view.answered(PASSPHRASE_AGAIN, "horse", &scene, &texts),
            Errand::Seal(SealDoing::Seal { .. })
        ));
        let refusal = || {
            Came::Sealed(SealReport::Refused {
                from: file.clone(),
                opening: None,
                why: ronoctua_pq::Why::Failed,
            })
        };
        assert!(matches!(
            View::fresh().came(refusal(), &mut scene, &texts).errand,
            Errand::Nothing
        ));
        match view.came(refusal(), &mut scene, &texts).errand {
            Errand::Say(said) => assert!(said.contains("unreadable.txt"), "{said}"),
            other => panic!("the refused seal was not said: {other:?}"),
        }
    }

    #[test]
    fn every_sentence_and_question_exists_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for r in Refusal::ALL {
                let said = texts.t_with(r.key(), &[("name", "x.age")]);
                assert!(!said.starts_with('['), "{language}: {r:?}");
            }
            for why in ronoctua_pq::Why::ALL {
                let said = texts.t_with(why_key(why), &[("name", "x.age")]);
                assert!(!said.starts_with('['), "{language}: {why:?}");
            }
            for id in QUESTIONS {
                assert!(
                    !texts.t(&format!("ask-{id}")).starts_with('['),
                    "{language}: {id}"
                );
            }
            for m in Mode::ALL {
                assert!(!texts.t(m.label()).starts_with('['), "{language}: {m:?}");
            }
            for mine in [
                Mine::Ready(String::new()),
                Mine::NoBelongings,
                Mine::NotSynced,
            ] {
                assert!(
                    !texts.t(mine.value()).starts_with('['),
                    "{language}: {mine:?}"
                );
            }
        }
    }
}
