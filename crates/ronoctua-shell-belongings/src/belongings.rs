//! `LEADER s v` — the belongings: the person's configuration, kept
//! encrypted in a private repository they name, in one view with two doors:
//! the footer's region (`s v v`) and a window (`s v w`,
//! `crate::belongings_window`). `s v s` syncs from anywhere.
//!
//! # What is here
//!
//! Everything the screen decides — which rows exist, what a key does to
//! them, what is asked on the door's line, and which files a gesture writes —
//! and nothing that talks to a network, runs `age` or knows what a window
//! is. The engine is `ronoctua-belongings`'s CLI, which the transfer queue
//! runs as a child (`Work::Belongings`); this crate links only its SHAPES
//! (the `prefs` feature), so `age` never enters the footer.
//!
//! # The file is the broadcast
//!
//! Both doors read the same files ([`Readings::read`]) and watch the same
//! three places ([`watch`]): `belongings.json` (what the person chose), the
//! state folder (`status.json`, how the last run ended; `last.json`, what the
//! last sync left) and the runtime folder (the passphrase's file, the
//! decisions, the session the CLI prepares). A door that writes one of them
//! is heard by the other the way every door hears a preference.
//!
//! # A sync, from the person's side
//!
//! `Sync now` asks the passphrase on the door's own line, writes it into a
//! `0600` runtime file born private ([`ronoctua_belongings::secret`]), and
//! puts `prepare` on the queue. With no conflict the CLI carries it through
//! and deletes the file. With conflicts it waits: this view lists EVERY one —
//! the path, both sides' size and time, the machine that changed the
//! repository's — and the person decides each (keep this machine's, take the
//! repository's, or keep both for what they added), in bulk or one by one,
//! with the difference of a text a `RET` away. `Apply` sends the decisions;
//! `Drop` sends `abandon`, which deletes the passphrase and the session.
//!
//! # What goes to the repository, one listing
//!
//! "What goes to your repository" is ONE section, generated from the walk a
//! sync makes (`ronoctua_belongings::inventory`, over the one reader of the
//! tables, `ronoctua_belongings::scope`): grouped by where each file comes
//! from — a rule of the table, a predefined target, what the person added
//! here or on another machine, the plain part — one row per file, with its
//! state against the last sync and its size, and what is never kept with
//! why. It is counted in a thread of the door's ([`Counting`]): a home can
//! keep a lot, and the loop never waits for a walk.
//!
//! # Around it
//!
//! What a restore offers ("no" first), the files a lenient read repaired
//! ("leave it" first, "write the fix"), the weight of the history and its
//! compaction (asked, "no" first), the
//! targets' "not on this machine" and "not on any machine", every field of a
//! JSON conflict decided on its own, how the repository is reached, the
//! passphrase and whether the session remembers it, and how often the brain
//! reminds.
//!
//! # The signature
//!
//! The plain part is signed with the person's key (`ronoctua_belongings::plain`):
//! the listing names what the repository changed that this machine HELD and
//! why, and what this machine changed that waits for the passphrase to be
//! signed ("sign now"); a section says which key is the person's (the one
//! their passphrase opened, as the last run left it), which one this
//! machine's root pinned (`local/signer.json`), what root last refused or kept
//! back — and pins the person's key, a root gesture asked in the sudo bar,
//! when the machine has none, or another ([`pin_in_background`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use crate::key;
use ronoctua_belongings::Why;
use ronoctua_belongings::inventory::{Here, Item, Origin, Part};
use ronoctua_belongings::kept::{self, Pattern};
use ronoctua_belongings::order::Op;
use ronoctua_belongings::places::{Places, Root, covers};
use ronoctua_belongings::plan::{
    Conflict, Decision, Decisions, DecisionsFile, FieldDecisions, Merging, Plan, Side,
};
use ronoctua_belongings::prefs::{self, Auth, Belongings};
use ronoctua_belongings::requests::{Request, Requests};
use ronoctua_belongings::scope::{self, Scope};
use ronoctua_belongings::secret::Typed;
use ronoctua_belongings::status::{Budget, Last, Offer, Outcome, Status};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::belongings_words as words;
use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::keep::{change_the_file, lexically_clean, shown_path, store_of};

/// The keys this view teaches, and where each one applies. The hint line is
/// GENERATED from it (`chrome::hint_of_marked`), so a key that moves here
/// moves on screen.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "belongings-down", Where::Anywhere),
    ("<up>", "belongings-up", Where::Anywhere),
    // `S-<arrows>` TURN A VALUE — the period, the access — the system's rule
    // wherever a row has a value that turns.
    ("S-<left>", "belongings-less", Where::OnAValue),
    ("S-<right>", "belongings-more", Where::OnAValue),
    ("RET", "belongings-enter", Where::Anywhere),
    // THE THREE DECISIONS, on any row of a conflict — the path's, a
    // decision's, a line of its difference. `C-`, never `M-`: the `M-`
    // family is the compositor's and never reaches a window.
    ("C-l", "belongings-keep-local", Where::OnAConflict),
    ("C-r", "belongings-take-remote", Where::OnAConflict),
    ("C-b", "belongings-keep-both", Where::OnAConflictBoth),
    ("C-s", "belongings-sync", Where::Anywhere),
];

/// The commands the view's ROWS run — every one reachable by name too
/// (`LEADER :`), where it opens the view and does the same.
pub const OF_THE_ROWS: &[&str] = &[
    "belongings-apply",
    "belongings-abandon",
    "belongings-all-local",
    "belongings-all-remote",
    "belongings-repository",
    "belongings-add",
    "belongings-auth",
    "belongings-keygen",
    "belongings-copy-key",
    "belongings-token",
    "belongings-user",
    "belongings-passphrase",
    "belongings-period",
    // A TEXT WHOSE HUNKS CLASH, merged with one side's hunks — rows of a
    // conflict once the belongings-plain-merge package draws them.
    "belongings-take-merged-ours",
    "belongings-take-merged-theirs",
    // ── anchor: L09 ──
    // THE SIGNATURE: pin the person's key on this machine (through the sudo
    // door), and sign this machine's plain part now.
    "belongings-pin",
    "belongings-sign",
    // ── anchor: L21 ──
];

/// The command that opens the view in the footer (`LEADER s v v`).
pub const OPEN_COMMAND: &str = "belongings-open";
/// The command that opens it in a window (`LEADER s v w`).
pub const WINDOW_COMMAND: &str = "belongings-window";
/// The command that syncs: the view, asking the passphrase (`LEADER s v s`,
/// `C-s` inside it, and the brain's mark).
pub const SYNC_COMMAND: &str = "belongings-sync";

/// Every command this view's keys and rows reach — for `bindings::reachable`,
/// which is what says a command has a key.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .chain(OF_THE_ROWS.iter().copied())
}

/// Which rows a key means anything on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns.
    OnAValue,
    /// On any row of a conflict.
    OnAConflict,
    /// On a conflict that may keep both sides.
    OnAConflictBoth,
}

/// A key, once the door has read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// The previous value.
    Less,
    /// The next value.
    More,
    /// Do what the row does.
    Enter,
    /// Decide the conflict under the cursor.
    Decide(Decision),
    /// Sync now.
    Sync,
    /// Let the repository's host be asked — the consent row's own key
    /// (`consent_rows::OF_THE_CONSENT`), meaningful while the row is drawn.
    Allow,
    /// Back one level, and out.
    Escape,
}

/// A command of [`OF_THE_VIEW`] as the view's press — a click on the hint, or
/// the id run by name.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "belongings-down" => Press::Down,
        "belongings-up" => Press::Up,
        "belongings-less" => Press::Less,
        "belongings-more" => Press::More,
        "belongings-enter" => Press::Enter,
        "belongings-keep-local" => Press::Decide(Decision::KeepLocal),
        "belongings-take-remote" => Press::Decide(Decision::TakeRemote),
        "belongings-keep-both" => Press::Decide(Decision::KeepBoth),
        "belongings-sync" => Press::Sync,
        _ => return None,
    })
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` is the way back whatever is held.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    if crate::consent_rows::OF_THE_CONSENT
        .iter()
        .any(|(taught, _)| *taught == notation)
    {
        return Some(Press::Allow);
    }
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _, _)| *taught == notation)
        .map(|(_, id, _)| *id)?;
    press_of(id)
}

// ---------------------------------------------------------------------------
// WHAT THE VIEW READS
// ---------------------------------------------------------------------------

/// What `last.json` says, in the few numbers the view shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    /// The repository it was made with.
    pub repository: String,
    /// When, seconds since the epoch.
    pub at_s: i64,
    /// How many paths are kept.
    pub files: u64,
    /// What they weigh.
    pub bytes: u64,
}

/// Everything both doors draw from — the files, read the same way by both.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// Where everything is, on this machine; `None` with no home or no
    /// runtime directory, which the view says.
    pub places: Option<Places>,
    /// What the person chose.
    pub prefs: Belongings,
    /// `belongings.json` is there and does not read WHOLE — not JSON, or a
    /// value of it the store would repair: nothing is written over it, and
    /// the view says so. Whole, because the engine reads it strictly
    /// (`engine::read_prefs`): a file it refuses must not look like
    /// settings a sync will use.
    pub prefs_refused: bool,
    /// How the last run ended.
    pub status: Option<Status>,
    /// What the last sync left.
    pub last: Option<Summary>,
    /// A sync prepared and waiting for decisions.
    pub plan: Option<Plan>,
    /// What the person decided of it so far.
    pub decisions: Decisions,
    /// The deploy key's public half, when there is one.
    pub public_key: Option<String>,
    /// Whether the deploy key exists.
    pub has_key: bool,
    /// Whether a token is kept.
    pub has_token: bool,
    /// Whether a run of the belongings holds this machine's lock now
    /// (`ronoctua_belongings::running`): a second sync, an apply, a drop or a
    /// change of passphrase waits for it to end.
    pub busy: bool,
    /// What a person added on any machine that this machine followed at its
    /// last sync (`last.json`'s roots) — shown with what is kept, and taken
    /// out by the same row.
    pub followed: Vec<String>,
    /// The plain part as its last run left it, for the repository set now
    /// (`last.json`'s plain section): what it left out for its size, and
    /// what waits for a sync with the person.
    pub plain: ronoctua_belongings::plain::PlainBase,
    /// The repository's host, while the person has not let it be asked
    /// (`consent.json` under the config root): no sync reaches it, and the
    /// view draws the consent row instead (`consent_rows`).
    pub unlet: Option<String>,
    /// Now, seconds since the epoch.
    pub now_s: i64,
    /// `last.json` whole, when it is of the repository set now — what the
    /// listing is compared with, and what this machine follows.
    pub last_file: Option<Last>,
    /// What a door asked of the next sync (`requests.json`): a target kept
    /// on no machine, or on every machine again.
    pub requests: Requests,
    /// The fields of a JSON conflict decided one by one, by path and pointer
    /// (`decisions.json`'s `fields`).
    pub fields: FieldDecisions,
    /// The files a lenient read repaired that wait for "write the fix" or
    /// "leave it", whose file is still the one judged.
    pub repairs: Vec<ronoctua_store::Ledger>,
    /// Whether this machine runs the newest distro of the person's machines
    /// (`fleet.json`): its sync writes those repairs by itself.
    pub newest: bool,
    /// Until when the session keeps the identity of this repository
    /// (`runtime/unlocked`), when it does now: a sync asks nothing.
    pub kept_until: Option<i64>,
    /// What the person answered of the offers the last sync made.
    pub answered: Answered,
    /// What goes to the repository, one row per file — counted off the
    /// loop ([`Counting`]); `None` until the first count is in.
    pub count: Option<Arc<Counted>>,
    /// The person's signing key this machine's root pinned
    /// (`local/signer.json` in the machine directory, read with the person
    /// crate's one reader), and what its last apply made of the signature;
    /// `None` with no key pinned (or a pin that does not read).
    pub pin: Option<ronoctua_person::local::Signer>,
    /// The pin the person asked for through the sudo door, and how it went
    /// ([`PIN_ANSWERED_FILE`]).
    pub pinning: Option<Pinning>,
    /// What the listing is made of ([`Readings::scope`]), made once per
    /// reading: it holds what the last sync left, and the page is drawn on
    /// every frame.
    scope: Scope,
}

impl Readings {
    /// Reads every file of the belongings under `places`, at `now_s`.
    ///
    /// Small files, read whole: the settings, two records, a plan. A file
    /// that is missing is the ordinary state of a machine that never synced.
    #[must_use]
    pub fn read(places: Option<Places>, now_s: i64) -> Self {
        let Some(p) = places else {
            return Self {
                now_s,
                ..Self::default()
            };
        };
        let (prefs, prefs_refused) = match store_of(&p).whole() {
            Some(prefs) => (prefs, false),
            None => (Belongings::default(), true),
        };
        let last_file = Last::read(&p.last());
        let followed = last_file
            .as_ref()
            .filter(|l| l.repository == prefs.repository)
            .map(|l| l.roots.iter().cloned().collect())
            .unwrap_or_default();
        let plain = last_file
            .as_ref()
            .and_then(|l| l.plain.of(&prefs.repository).cloned())
            .unwrap_or_default();
        let last = last_file.as_ref().map(|l| Summary {
            files: l.entries.len() as u64,
            bytes: l.entries.values().map(|k| k.size_bytes).sum(),
            repository: l.repository.clone(),
            at_s: l.at_s,
        });
        let last_file = last_file.filter(|l| l.repository == prefs.repository);
        let plan: Option<Plan> = std::fs::read(p.plan())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        // THE PATHS' DECISIONS of the file both doors write — and the
        // fields' (`DecisionsFile`), kept when it is written again.
        let file = DecisionsFile::read(&p.decisions()).unwrap_or_default();
        let (decisions, fields) = (file.paths, file.fields);
        let public_key = std::fs::read_to_string(p.deploy_public())
            .ok()
            .map(|k| k.trim().to_owned())
            .filter(|k| !k.is_empty());
        // THE HOST, ASKED OF THE SAME ROOT the settings come from: in the
        // product it is the person's `consent.json` the CLI mints its pass
        // from; under a photograph's folder or a test's, that folder's.
        let unlet = prefs::host_of(&prefs.repository)
            .filter(|host| !ronoctua_consent::allows_under(&p.config, host));
        let kept_until = ronoctua_belongings::unlock::Marker::read(&p)
            .filter(|m| prefs.keep_unlocked && m.holds(&prefs.repository, now_s))
            .map(|m| m.until_s);
        Self {
            last_file,
            requests: Requests::read(&p.requests()),
            fields,
            repairs: ronoctua_belongings::inventory::repairs_waiting(&p),
            newest: ronoctua_belongings::fleet::Fleet::read(&p.fleet())
                .is_some_and(|f| f.newest_here()),
            kept_until,
            answered: Answered::read(&p),
            count: None,
            pin: machine_pin(),
            pinning: Pinning::read(&p),
            scope: Scope::default(),
            status: Status::read(&p.status()),
            unlet,
            has_key: p.deploy_key().is_file(),
            has_token: p.token().is_file(),
            busy: ronoctua_belongings::running::busy(&p),
            followed,
            plain,
            public_key,
            last,
            plan,
            decisions,
            prefs,
            prefs_refused,
            places: Some(p),
            now_s,
        }
        .scoped()
    }

    /// The same readings with their scope made.
    fn scoped(mut self) -> Self {
        let waiting = self
            .plan
            .as_ref()
            .map(|p| {
                p.conflicts
                    .iter()
                    .map(|c| c.path.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.scope =
            Scope::of(&self.prefs, self.last_file.as_ref(), &self.requests).waiting_on(waiting);
        self
    }

    /// Whether this machine has never synced with the repository set now —
    /// its first sync restores.
    #[must_use]
    pub fn first_run(&self) -> bool {
        self.prefs.has_repository()
            && self
                .last
                .as_ref()
                .is_none_or(|l| l.repository != self.prefs.repository)
    }

    /// The decision a conflict has now: the person's, else its suggestion —
    /// which a field decided by the person sets aside (they decide it field
    /// by field).
    #[must_use]
    pub fn decision_of(&self, c: &Conflict) -> Option<Decision> {
        self.decisions
            .get(&c.path)
            .copied()
            .filter(|d| d.allowed(c))
            .or_else(|| c.suggested.filter(|_| !self.fields.contains_key(&c.path)))
    }

    /// Whether a conflict is decided: a decision of its whole path, or —
    /// a JSON conflict — one of every field (what the CLI reads,
    /// `Plan::settled`).
    #[must_use]
    pub fn settled(&self, c: &Conflict) -> bool {
        self.decision_of(c).is_some() || self.fields_file().fields_settle(c)
    }

    /// The decisions file as the person left it: paths and fields.
    #[must_use]
    pub fn fields_file(&self) -> DecisionsFile {
        DecisionsFile {
            paths: self.decisions.clone(),
            fields: self.fields.clone(),
        }
    }

    /// What the listing is made of: the settings, what the last sync left,
    /// what the person asked of the next one, and what a prepared sync waits
    /// on (`ronoctua_belongings::scope`).
    #[must_use]
    pub const fn scope(&self) -> &Scope {
        &self.scope
    }

    /// The same readings, with the listing of `before` (a count is not
    /// thrown away by a reading: the next one replaces it).
    #[must_use]
    pub fn keeping(mut self, before: &Self) -> Self {
        self.count.clone_from(&before.count);
        self
    }

    /// The files of the last count — none before the first.
    #[cfg(test)]
    #[must_use]
    pub fn items(&self) -> &[Item] {
        self.count.as_deref().map_or(&[], |c| c.items.as_slice())
    }

    /// What the last sync weighed, when it was measured.
    #[must_use]
    pub fn budget(&self) -> Option<Budget> {
        match self.status.as_ref().map(|s| &s.outcome) {
            Some(Outcome::Synced { budget, .. }) => *budget,
            _ => None,
        }
    }

    /// The offers the last sync made that the person has not answered, with
    /// their place in the list.
    #[cfg(test)]
    #[must_use]
    pub fn offers_waiting(&self) -> Vec<(usize, &Offer)> {
        self.offers()
            .iter()
            .enumerate()
            .filter(|(_, o)| self.answer_of(o).is_none())
            .collect()
    }

    /// The offers the last sync made.
    #[must_use]
    pub fn offers(&self) -> &[Offer] {
        match self.status.as_ref().map(|s| &s.outcome) {
            Some(Outcome::Synced { offers, .. }) => offers,
            _ => &[],
        }
    }

    /// What the person answered of `offer`, for the sync that made it.
    #[must_use]
    pub fn answer_of(&self, offer: &Offer) -> Option<&Said> {
        let at_s = self.status.as_ref()?.at_s;
        (self.answered.at_s == at_s)
            .then(|| {
                self.answered
                    .answers
                    .iter()
                    .find(|a| a.offer == *offer)
                    .map(|a| &a.said)
            })
            .flatten()
    }

    /// Whether a sync may go with the identity the session keeps, asking
    /// nothing: kept for this repository now, not a first sync (its
    /// passphrase is asked twice), and the last run not refused for want of
    /// one — a marker the kernel outlived, said once and then asked.
    #[must_use]
    pub fn asks_nothing(&self) -> bool {
        self.kept_until.is_some()
            && !self.first_run()
            && !matches!(
                self.status.as_ref().map(|s| &s.outcome),
                Some(Outcome::Refused {
                    why: Why::NoPassphrase | Why::WrongPassphrase
                })
            )
    }

    /// Every decision as it stands — the person's over the suggestions.
    #[must_use]
    pub fn decided(&self) -> Decisions {
        let Some(plan) = self.plan.as_ref() else {
            return Decisions::new();
        };
        plan.conflicts
            .iter()
            .filter_map(|c| self.decision_of(c).map(|d| (c.path.clone(), d)))
            .collect()
    }

    /// Whether the access the settings ask for is here: the deploy key, or
    /// the token.
    #[must_use]
    pub const fn has_access(&self) -> bool {
        match self.prefs.auth {
            Auth::Ssh => self.has_key,
            Auth::Token => self.has_token,
        }
    }
}

/// Seconds since the epoch, now.
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

// ---------------------------------------------------------------------------
// WHAT A RESTORE OFFERED, AND WHAT THE PERSON SAID
// ---------------------------------------------------------------------------

/// The file of the person's answers to the last sync's offers, under the
/// belongings' own folder: both doors draw an offer answered alike, and a
/// "no" stops asking until a sync offers it again.
pub const ANSWERED_FILE: &str = "offers-answered.json";

/// What the person said to an offer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "said", rename_all = "kebab-case")]
pub enum Said {
    /// No: it stays off.
    No,
    /// Yes, and it is being turned on (a thread runs git).
    Turning,
    /// Yes, and it is on.
    Yes,
    /// Yes, and it could not be turned on, and why.
    Refused {
        /// Why.
        why: Why,
    },
    /// Yes, and the sudo door refused what it needed — the sudo bar
    /// cancelled, the password wrong, the footer not there — and why.
    RootRefused {
        /// Why.
        why: ronoctua_security::Why,
    },
}

/// One answer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Answer {
    /// What was offered.
    pub offer: Offer,
    /// What the person said.
    pub said: Said,
}

/// The person's answers to the offers of the sync that ended at `at_s` —
/// another sync's answers are nobody's now.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Answered {
    /// When the sync that offered ended, seconds since the epoch
    /// (`status.json`'s `at_s`).
    pub at_s: i64,
    /// What was said, one per offer.
    pub answers: Vec<Answer>,
}

impl Answered {
    /// The answers under `places`; none when there is no file or it does not
    /// read.
    #[must_use]
    pub fn read(places: &Places) -> Self {
        std::fs::read(places.own().join(ANSWERED_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
}

/// Writes what the person said to `offer` of the sync that ended at `at_s`,
/// over what they said of it before — `0600`, like every file of the
/// belongings' own folder.
///
/// # Errors
///
/// The key of the sentence that says it could not be written.
pub fn answer(places: &Places, at_s: i64, offer: &Offer, said: Said) -> Result<(), &'static str> {
    let mut now = Answered::read(places);
    if now.at_s != at_s {
        now = Answered {
            at_s,
            answers: Vec::new(),
        };
    }
    now.answers.retain(|a| a.offer != *offer);
    now.answers.push(Answer {
        offer: offer.clone(),
        said,
    });
    let text = serde_json::to_vec_pretty(&now).map_err(|_| key::BELONGINGS_NOT_WRITTEN)?;
    ronoctua_belongings::secret::make_private_dir(&places.own())
        .and_then(|()| {
            ronoctua_belongings::secret::write_private(&places.own().join(ANSWERED_FILE), &text)
        })
        .map_err(|e| {
            tracing::warn!(error = %e, "an answer to an offer could not be written");
            key::BELONGINGS_NOT_WRITTEN
        })
}

/// "Yes" to an offer: said at once ("turning on"), and turned on in a thread
/// of its own — SSH signing runs git; the security keys are root's list, so
/// their yes is a root gesture of the door's (`gate`), asked in the sudo bar
/// in the same gesture as the yes — whose ending is written where both
/// doors read it ([`ANSWERED_FILE`], the file is the broadcast).
pub fn accept_in_background(places: Places, at_s: i64, offer: Offer, gate: crate::sudo::Gate) {
    if let Err(said) = answer(&places, at_s, &offer, Said::Turning) {
        tracing::warn!(said, "the offer is turned on without saying so first");
    }
    let spawned = std::thread::Builder::new()
        .name("belongings-offer".to_owned())
        .spawn(move || {
            let said = match (&offer, ronoctua_git::Tools::from_env()) {
                (Offer::PamU2f, _) => {
                    bring_the_keys(&places, gate.gesture(crate::sudo::Asker::Belongings))
                }
                (_, None) => Said::Refused { why: Why::NoTools },
                (_, Some(tools)) => {
                    match ronoctua_belongings::offers::accept(&offer, &places, &tools) {
                        Ok(()) => Said::Yes,
                        Err(r) => Said::Refused { why: r.why },
                    }
                }
            };
            if let Err(said) = answer(&places, at_s, &offer, said) {
                tracing::warn!(said, "what an offer came to could not be written");
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "no thread to turn an offer on");
    }
}

/// The security keys' yes: the keys of the person's own list that root's
/// list lacks (`offers::security_keys`) go into root's list WITH the ones it
/// has for the person (`u2f::listed`) — one root gesture, the sudo door's
/// `u2f-keys` verb, asked in the sudo bar. BLOCKS: the offer's thread.
fn bring_the_keys(places: &Places, mut gesture: crate::sudo::Gesture) -> Said {
    use ronoctua_security::u2f;
    let user = std::env::var("USER").unwrap_or_default();
    let Ok(mut credentials) = u2f::listed(&user) else {
        return Said::RootRefused {
            why: ronoctua_security::Why::Unreadable,
        };
    };
    let brought = ronoctua_belongings::offers::security_keys(places);
    if brought.is_empty() {
        // BROUGHT ALREADY (another door's yes, a key registered meanwhile).
        return Said::Yes;
    }
    for credential in &brought {
        credentials = u2f::with_key(&credentials, credential);
    }
    if credentials.len() > u2f::MOST_KEYS {
        return Said::RootRefused {
            why: ronoctua_security::Why::Invalid,
        };
    }
    match gesture.run(
        ronoctua_security::verb::RootVerb::U2fKeys { credentials }.into(),
        &mut |_| {},
    ) {
        Ok(()) => Said::Yes,
        Err(why) => Said::RootRefused { why },
    }
}

// ---------------------------------------------------------------------------
// THE PERSON'S KEY, PINNED ON THIS MACHINE through the sudo bar
// ---------------------------------------------------------------------------

/// The file of the pin the person asked for, under the belongings' own
/// folder: both doors draw it alike, and it outlives the door that asked.
pub const PIN_ANSWERED_FILE: &str = "pin-answered.json";

/// How a pin the person asked for went.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "said", rename_all = "kebab-case")]
pub enum PinSaid {
    /// The door is asked (a thread runs the gesture; the sudo bar asks).
    Pinning,
    /// The machine trusts the key now.
    Pinned,
    /// The door said yes and the machine's pin is not that key (the unit
    /// refused it: its reason is in the journal).
    NotPinned,
    /// The door refused — the sudo bar cancelled, the password wrong, the
    /// footer not there — and why.
    Refused {
        /// Why.
        why: ronoctua_security::Why,
    },
}

/// The pin the person asked for: of which key, and how it went.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pinning {
    /// The fingerprint of the key asked to be pinned.
    pub fingerprint: String,
    /// How it went.
    pub said: PinSaid,
}

impl Pinning {
    /// The pin asked for under `places`; none when there is no file or it
    /// does not read.
    #[must_use]
    pub fn read(places: &Places) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(places.own().join(PIN_ANSWERED_FILE)).ok()?).ok()
    }

    /// Writes it — `0600`, like every file of the belongings' own folder.
    fn write(&self, places: &Places) {
        let written = serde_json::to_vec_pretty(self)
            .map_err(std::io::Error::other)
            .and_then(|text| {
                ronoctua_belongings::secret::make_private_dir(&places.own())?;
                ronoctua_belongings::secret::write_private(
                    &places.own().join(PIN_ANSWERED_FILE),
                    &text,
                )
            });
        if let Err(e) = written {
            tracing::warn!(error = %e, "how a pin went could not be written");
        }
    }
}

/// The machine directory a photograph reads the pin from, instead of this
/// machine's (`photograph --door belongings --machine <folder>`).
static PHOTOGRAPHED_MACHINE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Points this process's reading of the pin at `dir` — the belongings'
/// camera only, before anything is read.
pub fn photograph_machine(dir: PathBuf) {
    let _ = PHOTOGRAPHED_MACHINE.set(dir);
}

/// The key this machine's root pinned (`local/signer.json` under the
/// machine directory, by its one name); `None` with none pinned. Everybody
/// may read it; only `ronoctua-person`, as root, writes it.
#[must_use]
pub fn machine_pin() -> Option<ronoctua_person::local::Signer> {
    // THE FOOTER CAMERA'S ROOM has a machine directory of its own: its
    // picture reads the pin planted there, never this machine's.
    let machine = PHOTOGRAPHED_MACHINE
        .get()
        .cloned()
        .or_else(|| crate::photo::room().map(|room| room.join(crate::photo::MACHINE_DIR)))
        .unwrap_or_else(|| PathBuf::from(env!("RONOCTUA_MACHINE_DIR")));
    machine_pin_in(&machine)
}

/// [`machine_pin`] of the machine directory `machine`.
#[must_use]
pub fn machine_pin_in(machine: &Path) -> Option<ronoctua_person::local::Signer> {
    let bytes = std::fs::read(machine.join(ronoctua_person::names::SIGNER)).ok()?;
    ronoctua_person::parse(&bytes).ok()
}

/// "Pin my key": said at once ("pinning"), and run in a thread of its own as
/// a root gesture of the door's (`gate`), asked in the sudo bar —
/// `RootVerb::PersonSignerPin` with the key the person's passphrase opened
/// (what the last run left, never the repository's copy) — whose ending is
/// written where both doors read it ([`PIN_ANSWERED_FILE`]): pinned when the
/// machine's pin is that key afterwards, else why not.
pub fn pin_in_background(
    places: Places,
    key: String,
    fingerprint: String,
    gate: crate::sudo::Gate,
) {
    Pinning {
        fingerprint: fingerprint.clone(),
        said: PinSaid::Pinning,
    }
    .write(&places);
    let spawned = std::thread::Builder::new()
        .name("belongings-pin".to_owned())
        .spawn(move || {
            let mut gesture = gate.gesture(crate::sudo::Asker::Belongings);
            let said = match gesture.run(
                ronoctua_security::verb::RootVerb::PersonSignerPin { key }.into(),
                &mut |_| {},
            ) {
                Ok(()) => {
                    if machine_pin().is_some_and(|p| p.fingerprint == fingerprint) {
                        PinSaid::Pinned
                    } else {
                        PinSaid::NotPinned
                    }
                }
                Err(why) => PinSaid::Refused { why },
            };
            Pinning { fingerprint, said }.write(&places);
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "no thread to pin the key");
    }
}

// ---------------------------------------------------------------------------
// THE COUNT OF WHAT GOES TO THE REPOSITORY, off the loop
// ---------------------------------------------------------------------------

/// One count of what goes to the repository, made off the loop and read by
/// both doors: the files ([`ronoctua_belongings::inventory`]), and what
/// drawing them asks — so that a frame, a key or a pointer's motion pays
/// for no `stat` and formats no file twice.
#[derive(Debug, Default)]
pub struct Counted {
    /// Every file, by path.
    pub items: Vec<Item>,
    /// Where each origin's files are in `items`, in path order: the page
    /// draws group by group.
    by_origin: BTreeMap<Origin, Vec<usize>>,
    /// The paths of the targets and of what is never kept that are on this
    /// machine, and whether each is a folder (drawn ending in `/`).
    there: BTreeMap<&'static str, bool>,
    /// The folders a door hears what the listing is made of in.
    pub heard: Heard,
    /// The files' rows, formatted once per language: a large addition is
    /// thousands of rows, and the page is drawn on every key.
    formatted: Mutex<Option<(String, Arc<Vec<Formatted>>)>>,
}

impl Counted {
    /// Counts what goes to the repository from `places` — a walk: a door's
    /// thread's, or a photograph's and a test's in place.
    #[must_use]
    pub fn of(scope: &Scope, places: &Places) -> Self {
        let items = ronoctua_belongings::inventory::inventory(scope, places);
        let mut by_origin: BTreeMap<Origin, Vec<usize>> = BTreeMap::new();
        for (n, item) in items.iter().enumerate() {
            by_origin.entry(item.origin.clone()).or_default().push(n);
        }
        let named = scope::targets()
            .iter()
            .flat_map(|t| t.paths.iter().copied())
            .chain(scope::never_rules().iter().filter_map(|n| match n.pattern {
                Pattern::Path(path) => Some(path),
                Pattern::Name(_) => None,
            }));
        let there = named
            .filter_map(|logical| {
                let meta = std::fs::symlink_metadata(places.resolve(logical)?).ok()?;
                Some((logical, meta.is_dir()))
            })
            .collect();
        Self {
            items,
            by_origin,
            there,
            heard: Heard::of(scope, places),
            formatted: Mutex::new(None),
        }
    }

    /// The files of `origin`, in path order.
    fn of_origin(&self, origin: &Origin) -> &[usize] {
        self.by_origin.get(origin).map_or(&[], Vec::as_slice)
    }

    /// Whether `path` is a file of the listing.
    fn lists(&self, path: &str) -> bool {
        self.items
            .binary_search_by(|i| i.path.as_str().cmp(path))
            .is_ok()
    }

    /// Whether the logical `path` is a folder on this machine; `None` when it
    /// is not here (or nothing was counted).
    fn folder(&self, path: &str) -> Option<bool> {
        self.there.get(path).copied()
    }

    /// The files' rows in the language of `texts`, made the first time this
    /// count is drawn in it.
    fn formatted(
        &self,
        places: Option<&Places>,
        texts: &ronoctua_i18n::Texts,
    ) -> Arc<Vec<Formatted>> {
        let mut slot = self
            .formatted
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some((language, rows)) = slot.as_ref()
            && language == texts.language()
        {
            return Arc::clone(rows);
        }
        let rows: Arc<Vec<Formatted>> = Arc::new(
            self.items
                .iter()
                .map(|item| Formatted::of(item, places, texts))
                .collect(),
        );
        *slot = Some((texts.language().to_owned(), Arc::clone(&rows)));
        rows
    }
}

/// The most folders of each kind a door hears the listing's files in
/// ([`Heard`]) — each tree itself bounded by the notice
/// (`ronoctua_files::notice::TREE_MOST`): a watch is one of the session's
/// `max_user_watches`.
const HEARD_MOST: usize = 64;

/// The folders a door hears what the listing is made of in, while it shows
/// it: every root of the walk that is a folder here, as a TREE (a file
/// changed in `~/.ssh` or deep in an addition), and the folder of every root
/// that is a file (`~/.gitconfig`, `~/.shellrc.local`) — or the nearest one
/// that is there, so that its making is heard, and the count after it hears
/// the folder itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Heard {
    dirs: Vec<PathBuf>,
    trees: Vec<PathBuf>,
}

impl Heard {
    /// The folders of what `scope` walks under `places` — a `stat` per root,
    /// in the count's thread.
    #[must_use]
    pub fn of(scope: &Scope, places: &Places) -> Self {
        let mut dirs = BTreeSet::new();
        let mut trees = BTreeSet::new();
        for root in scope.roots() {
            let Some(at) = places.resolve(&root) else {
                continue;
            };
            if std::fs::symlink_metadata(&at).is_ok_and(|m| m.is_dir()) {
                trees.insert(at);
            } else if let Some(dir) = at.ancestors().skip(1).find(|a| a.is_dir()) {
                dirs.insert(dir.to_path_buf());
            }
        }
        // A FOLDER INSIDE A TREE is heard by the tree.
        dirs.retain(|d| !trees.iter().any(|t| d.starts_with(t)));
        Self {
            dirs: dirs.into_iter().take(HEARD_MOST).collect(),
            trees: trees.into_iter().take(HEARD_MOST).collect(),
        }
    }

    /// One notice over these folders, `ring` told of every change; `None`
    /// with nothing to hear, or when the notice could not be raised (said in
    /// the journal).
    #[must_use]
    pub fn raise<F>(&self, ring: F) -> Option<ronoctua_files::notice::Notice>
    where
        F: Fn() -> bool + Send + 'static,
    {
        use ronoctua_files::notice::{Notice, Pace};
        if self.dirs.is_empty() && self.trees.is_empty() {
            return None;
        }
        Notice::on_dirs(&self.dirs, &self.trees, Pace::Coalesced, ring)
            .map_err(|e| tracing::warn!(error = %e, "what the belongings keep is not heard"))
            .ok()
    }
}

/// What a door hears the listing's files by: one notice over the folders of
/// the last count ([`Heard`]), raised again when a count finds others — a
/// folder the person added, one made since — and gone with the door.
#[derive(Debug, Default)]
pub struct Ears {
    heard: Heard,
    notice: Option<ronoctua_files::notice::Notice>,
}

impl Ears {
    /// Hears the folders of the count in `r`, when they are not the ones
    /// heard already.
    pub fn follow<F>(&mut self, r: &Readings, ring: F)
    where
        F: Fn() -> bool + Send + 'static,
    {
        let Some(count) = r.count.as_ref() else {
            return;
        };
        if count.heard == self.heard {
            return;
        }
        // THE OLD NOTICE GOES FIRST: two over one tree would ring twice.
        self.notice = None;
        self.notice = count.heard.raise(ring);
        self.heard = count.heard.clone();
    }
}

/// How long until the session forgets the passphrase it remembers, and the
/// note that says until when is not true any more: a door arms one timer at
/// it while the view shows it (`Readings::kept_until`), and reads again when
/// it rings — event, not clock: nothing else says the marker ran out.
#[must_use]
pub fn until(at_s: i64) -> std::time::Duration {
    std::time::Duration::from_secs(u64::try_from(at_s.saturating_sub(now_s())).unwrap_or(0) + 1)
}

/// What the belongings' threads hand back to a door's loop
/// (`crate::region::Inbox`): a watch rang, or a count came back.
#[derive(Debug)]
pub enum Came {
    /// A watched file changed — a watch's, or an ear's of the listing.
    Rang,
    /// A count of what goes to the repository, made on its thread.
    Counted(Arc<Counted>),
}

/// The listing's count: a walk of what the belongings keep, in a thread
/// of its own (`ronoctua_belongings::inventory` — `stat` alone, but a home
/// can keep a lot). One at a time: a reading that comes while one runs asks
/// for one more after it, never two at once. A photograph and a test count
/// in place, at once.
pub struct Counting {
    /// Where a thread's count is posted, in either door; `None`: counted in
    /// place.
    sender: Option<crate::region::Post<Came>>,
    running: bool,
    again: bool,
}

impl Counting {
    /// Counted in place — a photograph's, a test's.
    #[must_use]
    pub const fn in_place() -> Self {
        Self {
            sender: None,
            running: false,
            again: false,
        }
    }

    /// Counted in a thread, posted through `sender` to the door's loop.
    #[must_use]
    pub const fn through(sender: crate::region::Post<Came>) -> Self {
        Self {
            sender: Some(sender),
            running: false,
            again: false,
        }
    }

    /// Something the listing is made of changed (a watch rang, the view
    /// opened): count again.
    pub fn ask(&mut self, r: &mut Readings) {
        let Some(places) = r.places.clone() else {
            r.count = Some(Arc::new(Counted::default()));
            return;
        };
        let scope = r.scope().clone();
        let Some(sender) = self.sender.clone() else {
            r.count = Some(Arc::new(Counted::of(&scope, &places)));
            return;
        };
        if self.running {
            self.again = true;
            return;
        }
        self.running = true;
        let spawned = std::thread::Builder::new()
            .name("belongings-count".to_owned())
            .spawn(move || {
                let _ = sender.send(Came::Counted(Arc::new(Counted::of(&scope, &places))));
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "no thread to count the belongings in");
            self.running = false;
            r.count = Some(Arc::new(Counted::default()));
        }
    }

    /// The files were read again by a gesture of this door: counted again at
    /// once in place; a thread's count waits for a watch to ring — a key
    /// that only moved the cursor walks nothing.
    pub fn reread(&mut self, r: &mut Readings) {
        if self.sender.is_none() {
            self.ask(r);
        }
    }

    /// A thread's count came back: it is the listing now — and another
    /// starts when a reading asked for one while it ran.
    pub fn came(&mut self, r: &mut Readings, counted: Arc<Counted>) {
        self.running = false;
        r.count = Some(counted);
        if std::mem::take(&mut self.again) {
            self.ask(r);
        }
    }
}

/// The watches that make a door hear the belongings' files: the settings,
/// the state folder and the runtime folder, each made first — a watch on a
/// folder that does not exist never fires. `ring` is told of every change.
///
/// The folders are made the way the CLI makes them (`0700`): they hold the
/// record of a sync and, in the runtime folder, the passphrase's file.
#[must_use]
pub fn watch<F>(places: &Places, ring: F) -> Vec<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + Clone + 'static,
{
    use ronoctua_belongings::secret::make_private_dir;
    use ronoctua_files::notice::{Notice, Pace};
    let mut out = Vec::new();
    if std::fs::create_dir_all(&places.config).is_ok() {
        // THE CONFIGURATION FOLDER: the settings, the consents beside them —
        // a host let in the other door, or in any app's consent row, takes
        // this one's row away — and every preference the listing says the
        // state of (a theme changed is "changed" at once). Its folders below
        // are the next count's: inotify is not recursive.
        match Notice::watching(Some(&places.config), ring.clone()) {
            Ok(n) => out.push(n),
            Err(e) => {
                tracing::warn!(error = %e, "the configuration folder cannot be watched");
                for file in [prefs::FILE, ronoctua_consent::FILE] {
                    if let Ok(n) =
                        Notice::on_file(&places.config, file, Pace::Coalesced, ring.clone())
                    {
                        out.push(n);
                    }
                }
            }
        }
    }
    // THE STATE FOLDER (a sync's record, the offers answered, the
    // machines), THE RUNTIME FOLDER (the session, the decisions, the marker
    // of a remembered passphrase) AND THE REPAIR LEDGER (a store that
    // repaired a file, a fix written).
    for dir in [places.own(), places.runtime.clone(), places.repairs()] {
        if let Err(e) = make_private_dir(&dir) {
            tracing::warn!(dir = %dir.display(), error = %e, "no folder to watch the belongings in");
            continue;
        }
        match Notice::watching(Some(&dir), ring.clone()) {
            Ok(n) => out.push(n),
            Err(e) => {
                tracing::warn!(dir = %dir.display(), error = %e, "the belongings cannot be watched");
            }
        }
    }
    out
}

/// Whether `id` is one of the belongings' commands — the ones a door runs
/// through the view ([`View::command`], [`View::answered`]).
#[must_use]
pub fn is_command(id: &str) -> bool {
    id == SYNC_COMMAND || reached().any(|c| c == id)
}

// ---------------------------------------------------------------------------
// ROWS
// ---------------------------------------------------------------------------

/// What the keys do on a row — and, since it carries what the row is about,
/// its identity across a rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A fact: `RET` copies its value. The word says which.
    Fact(String),
    /// The repository's address: `RET` asks it.
    Repository,
    /// Sync now.
    Sync,
    /// Apply the decisions.
    Apply,
    /// Drop the prepared sync.
    Abandon,
    /// Decide every conflict alike.
    All(Decision),
    /// A conflict's path: `RET` shows the difference of a text.
    Conflict(String),
    /// Line `n` of a conflict's difference: the conflict's keys decide it,
    /// `RET` copies the line.
    DiffLine(String, usize),
    /// One decision of a conflict.
    Decide(String, Decision),
    /// A path the table keeps: `RET` takes it out.
    Rule(String),
    /// A path the person added: `RET` stops keeping it.
    Kept(String),
    /// A path the person took out: `RET` keeps it again.
    Excluded(String),
    /// A path never kept by default, that a person may add: `RET` adds it.
    Never(String),
    /// Add a path: `RET` asks it.
    Add,
    /// How the repository is reached.
    Auth,
    /// Make the deploy key.
    Keygen,
    /// Copy the deploy key's public half.
    CopyKey,
    /// The token: `RET` asks it.
    Token,
    /// The HTTPS user: `RET` asks it.
    User,
    /// Change the passphrase.
    Passphrase,
    /// How often the brain reminds.
    Period,
    /// The repository's host, not let yet: `RET` lets it be asked.
    Let(String),
    /// A file of the listing, and what `RET` does to it.
    File(String, FileAct),
    /// A predefined target, "not on this machine" (its id).
    NotHere(&'static str),
    /// A target, "not on any machine": a question first, "no" first.
    Nowhere(&'static str),
    /// A target taken out here, "keep it on this machine again".
    HereAgain(&'static str),
    /// A target kept on no machine, "keep it on every machine again".
    Everywhere(&'static str),
    /// One field of a JSON conflict: its path and its pointer. The
    /// conflict's keys decide THE FIELD.
    Field(String, String),
    /// One field decided: this machine's value or the repository's.
    DecideField(String, String, Decision),
    /// A file a lenient read repaired: "write the fix".
    WriteFix(String),
    /// The same: "leave it".
    LeaveIt(String),
    /// What a sync offered — the offer itself, never its place in the
    /// list, which a later sync may reorder: yes or no.
    Offer(Offer, bool),
    /// Whether the session remembers the passphrase.
    Unlock,
    /// Compact the history: a question first, "no" first.
    Compact,
    /// The answer to the question on the page.
    Answer(bool),
    /// Pin the person's key on this machine (root asked in the sudo bar).
    Pin,
    /// Sign this machine's plain part now (the passphrase asked when the
    /// session does not keep the identity).
    Sign,
}

/// What `RET` does to a file of the listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAct {
    /// It is kept: stop keeping it (it stays where it is).
    StopKeeping,
    /// The person took it out here: keep it again.
    KeepAgain,
    /// Never kept by default, and a person may add it: keep it anyway.
    KeepAnyway,
}

impl Target {
    /// Whether `S-<arrows>` mean anything here.
    const fn turns(&self) -> bool {
        matches!(self, Self::Period | Self::Auth | Self::Unlock)
    }

    /// The conflict this row is about, when it is about one.
    fn conflict(&self) -> Option<&str> {
        match self {
            Self::Conflict(p)
            | Self::Decide(p, _)
            | Self::DiffLine(p, _)
            | Self::Field(p, _)
            | Self::DecideField(p, _, _) => Some(p),
            _ => None,
        }
    }

    /// The field of a conflict this row is about, when it is about one.
    fn field(&self) -> Option<(&str, &str)> {
        match self {
            Self::Field(p, f) | Self::DecideField(p, f, _) => Some((p, f)),
            _ => None,
        }
    }

    /// Whether `other` is the same row: a file's by its path alone — what
    /// `RET` does to it changes when it is done, and the cursor stays.
    fn same_row(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::File(a, _), Self::File(b, _)) => a == b,
            _ => self == other,
        }
    }

    /// The command this row's `RET` is, when it is one — what the pointer
    /// resting on it reads (`cmd-<id>`).
    const fn command(&self) -> Option<&'static str> {
        Some(match self {
            Self::Repository => "belongings-repository",
            Self::Sync => SYNC_COMMAND,
            Self::Apply => "belongings-apply",
            Self::Abandon => "belongings-abandon",
            Self::All(Decision::KeepLocal) => "belongings-all-local",
            Self::All(_) => "belongings-all-remote",
            Self::Decide(_, d) => words::decide_command(*d),
            Self::Add => "belongings-add",
            Self::Auth => "belongings-auth",
            Self::Keygen => "belongings-keygen",
            Self::CopyKey => "belongings-copy-key",
            Self::Token => "belongings-token",
            Self::User => "belongings-user",
            Self::Passphrase => "belongings-passphrase",
            Self::Period => "belongings-period",
            Self::Pin => "belongings-pin",
            Self::Sign => "belongings-sign",
            _ => return None,
        })
    }

    /// What the pointer resting on the row reads.
    fn about(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        if let Some(id) = self.command() {
            return Some(texts.t(&format!("cmd-{id}")));
        }
        Some(texts.t(match self {
            Self::Let(_) => key::ABOUT_CONSENT_ALLOW,
            Self::Conflict(_) => key::ABOUT_BELONGINGS_CONFLICT,
            Self::Rule(_) => key::ABOUT_BELONGINGS_RULE,
            Self::Kept(_) | Self::File(_, FileAct::StopKeeping) => key::ABOUT_BELONGINGS_KEPT,
            Self::Excluded(_) | Self::File(_, FileAct::KeepAgain) | Self::HereAgain(_) => {
                key::ABOUT_BELONGINGS_EXCLUDED
            }
            Self::Never(_) | Self::File(_, FileAct::KeepAnyway) => key::ABOUT_BELONGINGS_NEVER,
            Self::NotHere(_) => key::ABOUT_BELONGINGS_NOT_HERE,
            Self::Nowhere(_) => key::ABOUT_BELONGINGS_NOWHERE,
            Self::Everywhere(_) => key::ABOUT_BELONGINGS_EVERYWHERE,
            Self::Field(..) => key::ABOUT_BELONGINGS_FIELD,
            Self::DecideField(_, _, Decision::TakeRemote) => key::ABOUT_BELONGINGS_FIELD_THEIRS,
            Self::DecideField(..) => key::ABOUT_BELONGINGS_FIELD_OURS,
            Self::WriteFix(_) => key::ABOUT_BELONGINGS_WRITE_FIX,
            Self::LeaveIt(_) => key::ABOUT_BELONGINGS_LEAVE_IT,
            Self::Offer(_, false) => key::ABOUT_BELONGINGS_OFFER_NO,
            Self::Offer(_, true) => key::ABOUT_BELONGINGS_OFFER_YES,
            Self::Unlock => key::ABOUT_BELONGINGS_UNLOCK,
            Self::Compact => key::ABOUT_BELONGINGS_COMPACT,
            Self::Answer(false) => key::ABOUT_BELONGINGS_ANSWER_NO,
            Self::Answer(true) => key::ABOUT_BELONGINGS_ANSWER_YES,
            _ => return None,
        }))
    }
}

/// One drawn line.
#[derive(Debug, Clone)]
pub struct Row {
    /// The pieces, already placed and cut.
    pub fields: Vec<Field>,
    /// What a key does here — `None` on a title, a sentence, the rest of a
    /// wrapped label: the cursor skips it.
    pub target: Option<Target>,
}

impl Row {
    /// Whether the cursor skips it.
    #[must_use]
    pub const fn heading(&self) -> bool {
        self.target.is_none()
    }
}

/// One entry of a section, before the section is laid out whole.
enum Entry {
    /// A `label  value` row.
    Pair {
        label: String,
        value: String,
        kind: crate::chrome::Kind,
        target: Target,
        tint: Option<tokens::Color>,
        /// Cells in from the section's left: a decision under its conflict.
        indent: u16,
    },
    /// A value of a field of a JSON conflict, in under the field: the JSON
    /// elided to the room the section's value column leaves it — one line,
    /// however long the list it holds ([`field_text`]).
    Field {
        label: String,
        /// The JSON; `None`: the field is not on that side.
        value: Option<String>,
        /// Whether the person chose it.
        chosen: bool,
        target: Target,
        indent: u16,
    },
    /// A line of air between two items of one list.
    Air,
    /// A sentence under the row before it, wrapped.
    Note(String, tokens::Color),
    /// A sentence in under the rows it belongs to, wrapped in what is left:
    /// a field's value before both sides changed it, under that field's two
    /// values.
    NoteIn(String, u16),
    /// Lines drawn as they are — a difference.
    Lines(Vec<(Vec<Field>, Target)>),
}

/// The rows, as they are built.
struct Builder<'a> {
    out: Vec<Row>,
    cursor: usize,
    left: u16,
    room: u16,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    /// A section: its title, and its entries laid out whole — the values of
    /// every row in one column after the widest label (`section_column`),
    /// a label that does not fit wrapped under itself.
    fn section(&mut self, title: &str, entries: Vec<Entry>) {
        let end = self.left + self.room;
        // AIR BEFORE EVERY TITLE but the first — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                target: None,
            });
        }
        self.out.push(Row {
            fields: vec![piece(
                self.left,
                title.to_owned(),
                tokens::color::TEXT_HEADING,
                self.room,
            )],
            target: None,
        });
        let pairs: Vec<(u16, u16, u16)> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::Pair {
                    label,
                    value,
                    indent,
                    ..
                } => Some((
                    self.left + indent,
                    layout::width(label),
                    layout::width(value),
                )),
                // A FIELD'S VALUE TAKES THE ROOM IT IS LEFT, and moves no
                // column: a long list would push every value of the page to
                // the floor.
                Entry::Field { label, indent, .. } => {
                    Some((self.left + indent, layout::width(label), 0))
                }
                _ => None,
            })
            .collect();
        let column = crate::chrome::section_column(self.left, self.room, &pairs);
        let room = end.saturating_sub(column);
        let texts = self.texts;
        let entries = entries.into_iter().map(|entry| match entry {
            Entry::Field {
                label,
                value,
                chosen,
                target,
                indent,
            } => Entry::Pair {
                label,
                value: field_text(value.as_deref(), chosen, room, texts),
                kind: crate::chrome::Kind::Setting,
                target,
                tint: None,
                indent,
            },
            other => other,
        });
        for entry in entries {
            match entry {
                Entry::Pair {
                    label,
                    value,
                    kind,
                    target,
                    tint,
                    indent,
                } => {
                    let focused = self.cursor == self.out.len();
                    let lines = crate::chrome::aligned_pair(
                        self.left + indent,
                        column,
                        end,
                        &label,
                        &value,
                        kind,
                        focused,
                    );
                    // EVERY LINE of a wrapped value is said in its state's
                    // colour, not only the first.
                    for (n, fields) in lines.into_iter().enumerate() {
                        self.out.push(Row {
                            fields: crate::chrome::tinted_value(fields, column, tint, focused),
                            target: (n == 0).then(|| target.clone()),
                        });
                    }
                }
                // MADE A PAIR ABOVE, once the column was known.
                Entry::Field { .. } => {}
                Entry::Air => self.out.push(Row {
                    fields: crate::chrome::AIR,
                    target: None,
                }),
                Entry::Note(text, colour) => {
                    for line in crate::chrome::note_lines(&text, self.room) {
                        self.out.push(Row {
                            fields: vec![piece(self.left, line, colour, self.room)],
                            target: None,
                        });
                    }
                }
                Entry::NoteIn(text, indent) => {
                    let room = self.room.saturating_sub(indent).max(1);
                    for line in crate::chrome::note_lines(&text, room) {
                        self.out.push(Row {
                            fields: vec![piece(
                                self.left + indent,
                                line,
                                tokens::color::TEXT_DIM,
                                room,
                            )],
                            target: None,
                        });
                    }
                }
                Entry::Lines(lines) => {
                    for (mut fields, target) in lines {
                        // THE CURSOR IS A COLOUR here too: a line of a
                        // difference is all label, and under the cursor it
                        // takes the accent a label takes — its `+` or `-`
                        // still says which side it is.
                        if self.cursor == self.out.len() {
                            for f in &mut fields {
                                f.color = crate::chrome::Kind::Setting.tones(true).0;
                            }
                        }
                        self.out.push(Row {
                            fields,
                            target: Some(target),
                        });
                    }
                }
            }
        }
    }

    fn t(&self, k: &str) -> String {
        self.texts.t(k)
    }
}

/// A pair entry.
fn pair(label: String, value: String, target: Target, acts: bool) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::of(acts),
        target,
        tint: None,
        indent: 0,
    }
}

/// A decision's row, in under its conflict.
fn indented(label: String, value: String, target: Target) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::Setting,
        target,
        tint: None,
        indent: tokens::space::VIEW_NESTCELLS,
    }
}

/// The same, with its value in a colour that says a state.
fn tinted(label: String, value: String, target: Target, acts: bool, tint: tokens::Color) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::of(acts),
        target,
        tint: Some(tint),
        indent: 0,
    }
}

/// A dim sentence.
fn note(text: String) -> Entry {
    Entry::Note(text, tokens::color::TEXT_DIM)
}

/// One piece of text, elided to `room`.
fn piece(at: u16, text: String, color: tokens::Color, room: u16) -> Field {
    Field {
        at,
        text: layout::elide_end(&text, room),
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

/// A size, in the language's own marks.
fn bytes_text(bytes: u64, texts: &ronoctua_i18n::Texts) -> String {
    crate::sizes::readable_bytes(bytes, texts.decimal_mark())
}

/// A moment, as the person reads dates.
fn when(secs: i64, zone: &ronoctua_files::when::Zone, texts: &ronoctua_i18n::Texts) -> String {
    crate::chrome::when_text(secs, zone, false, texts)
}

/// One side of a conflict, in words: its size and when it changed — or
/// "deleted".
fn side_text(
    side: Option<&Side>,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    match side {
        None => texts.t(key::BELONGINGS_SIDE_DELETED),
        Some(s) if s.modified_s > 0 => texts.t_with(
            key::BELONGINGS_SIDE,
            &[
                ("size", &bytes_text(s.size_bytes, texts)),
                ("when", &when(s.modified_s, zone, texts)),
            ],
        ),
        Some(s) => bytes_text(s.size_bytes, texts),
    }
}

/// The sentence a run's ending says.
#[must_use]
pub fn outcome_text(outcome: &Outcome, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::Arg;
    let n = |v: u32| Arg::Number(i64::from(v));
    match outcome {
        Outcome::Synced {
            pulled,
            pushed,
            deleted_here,
            deleted_there,
            ..
        } => texts.t_args(
            key::BELONGINGS_OUTCOME_SYNCED,
            &[
                ("pulled", n(*pulled)),
                ("pushed", n(*pushed)),
                ("deleted", n(deleted_here + deleted_there)),
            ],
        ),
        Outcome::Waiting { conflicts } => {
            texts.t_args(key::BELONGINGS_OUTCOME_WAITING, &[("n", n(*conflicts))])
        }
        Outcome::Refused { why } => texts.t(words::refusal_key(*why)),
        Outcome::Abandoned => texts.t(key::BELONGINGS_OUTCOME_ABANDONED),
        Outcome::Keyed => texts.t(key::BELONGINGS_OUTCOME_KEYED),
        Outcome::Rekeyed => texts.t(key::BELONGINGS_OUTCOME_REKEYED),
        Outcome::Compacted {
            before_bytes,
            after_bytes,
        } => texts.t_with(
            key::BELONGINGS_OUTCOME_COMPACTED,
            &[
                ("before", &bytes_text(*before_bytes, texts)),
                ("after", &bytes_text(*after_bytes, texts)),
            ],
        ),
        Outcome::PlainSynced { pulled, pushed } => texts.t_args(
            key::BELONGINGS_OUTCOME_PLAIN_SYNCED,
            &[("pulled", n(*pulled)), ("pushed", n(*pushed))],
        ),
        // ── anchor: L08 ──
        Outcome::NothingToCompact { keep_days } => texts.t_args(
            key::BELONGINGS_OUTCOME_NOTHING_TO_COMPACT,
            &[(
                "days",
                Arg::Number(i64::try_from(*keep_days).unwrap_or(i64::MAX)),
            )],
        ),
        // ── anchor: L09 ──
    }
}

// ---------------------------------------------------------------------------
// QUESTIONS
// ---------------------------------------------------------------------------

/// Something the person types, and what it is for — asked on the line of
/// the door the key was pressed in, answered through [`View::answered`].
///
/// A SECRET is held only until it is written, and its bytes are overwritten
/// when it is dropped ([`Typed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// The repository's address.
    Repository,
    /// The passphrase, for a sync.
    Passphrase,
    /// The passphrase again, on a machine's first sync: an empty repository
    /// takes what is typed as its passphrase for good, and nothing recovers
    /// one that is lost.
    Confirm(Typed),
    /// The passphrase as it is, to change it.
    Old,
    /// The new passphrase, the old one held.
    New(Typed),
    /// The new one again.
    Again(Typed, Typed),
    /// The HTTPS token.
    Token,
    /// The HTTPS user.
    User,
    /// A path to keep.
    Path,
    /// The passphrase, to compact the history (asked when the session does
    /// not remember it).
    Compact,
    /// The passphrase, to sign this machine's plain part (asked when the
    /// session does not remember it).
    Sign,
}

impl Ask {
    /// The command that asks — and that answers: the question is its
    /// `ask-<id>` key unless it has its own ([`Ask::asks`]).
    #[must_use]
    pub const fn command(&self) -> &'static str {
        match self {
            Self::Repository => "belongings-repository",
            // THE PASSPHRASE OF A COMPACTION answers as a sync's does: one
            // command reaches the view's question, and the question says
            // which.
            Self::Passphrase | Self::Confirm(_) | Self::Compact => SYNC_COMMAND,
            Self::Old | Self::New(_) | Self::Again(..) => "belongings-passphrase",
            Self::Token => "belongings-token",
            Self::User => "belongings-user",
            Self::Path => "belongings-add",
            Self::Sign => "belongings-sign",
        }
    }

    /// The question's own key, when it is not `ask-<command>` — the three
    /// steps of a change of passphrase are one command.
    #[must_use]
    pub const fn asks(&self) -> Option<&'static str> {
        match self {
            Self::New(_) => Some(key::ASK_BELONGINGS_PASSPHRASE_NEW),
            Self::Again(..) => Some(key::ASK_BELONGINGS_PASSPHRASE_AGAIN),
            Self::Confirm(_) => Some(key::ASK_BELONGINGS_SYNC_AGAIN),
            Self::Compact => Some(key::ASK_BELONGINGS_COMPACT),
            _ => None,
        }
    }

    /// Whether what is typed is hidden.
    #[must_use]
    pub const fn secret(&self) -> bool {
        matches!(
            self,
            Self::Passphrase
                | Self::Confirm(_)
                | Self::Old
                | Self::New(_)
                | Self::Again(..)
                | Self::Token
                | Self::Compact
                | Self::Sign
        )
    }

    /// The line a door opens for it — a changed address starts from what it
    /// is.
    #[must_use]
    pub fn line(&self, r: &Readings) -> crate::line::Line {
        let initial = match self {
            Self::Repository => Some(r.prefs.repository.as_str()).filter(|a| !a.is_empty()),
            Self::User => r.prefs.user.as_deref(),
            _ => None,
        };
        let mut line = crate::line::Line::new(self.command(), self.secret(), initial);
        line.asks = self.asks();
        line
    }
}

// ---------------------------------------------------------------------------
// WHAT THE DOOR DOES
// ---------------------------------------------------------------------------

/// What the door has to do after a key, a click or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Shut the view.
    Close,
    /// Open this question on the door's own line.
    Ask(crate::line::Line),
    /// Say this sentence on the door's line.
    Say(String),
    /// Put this on the clipboard.
    Copy(String),
    /// Put this order on the queue — every file it reads is written already.
    Queue(Op),
    /// "Yes" to what the sync that ended at this moment offered: turn it on
    /// ([`accept_in_background`] — a photograph writes it down instead).
    Accept(i64, Offer),
    /// Pin this key (its text and fingerprint) on this machine, a root
    /// gesture asked in the sudo bar ([`pin_in_background`] — a photograph
    /// writes it down instead).
    Pin {
        /// The public key, as the last run that opened `key.age` left it.
        key: String,
        /// Its fingerprint.
        fingerprint: String,
    },
}

/// Which page of the view is showing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Mode {
    /// Every section.
    #[default]
    List,
    /// The difference of one conflicting text: its path and its lines.
    Diff(String, Vec<String>),
    /// A question about something that is not undone at once, "no" first
    /// and the cursor born on it.
    Question(Question),
}

/// A question the view asks on a page of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// Stop keeping this target on every machine?
    Nowhere(&'static str),
    /// Compact the history?
    Compact,
}

impl Question {
    /// The row that asked it — where the cursor goes back to.
    const fn asker(self) -> Target {
        match self {
            Self::Nowhere(id) => Target::Nowhere(id),
            Self::Compact => Target::Compact,
        }
    }
}

/// How many lines of a difference are read.
const DIFF_MOST_LINES: usize = 4000;

/// The view: where the cursor is, which page shows, what is being asked.
/// Everything it DRAWS is read from the [`Readings`] each door hands it, so
/// the two doors draw one set of files.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    mode: Mode,
    asking: Option<Ask>,
    /// The row the cursor is on, by what it is — kept across a rebuild, so a
    /// list that changes under it does not move it to another row.
    aim: Option<Target>,
}

impl View {
    /// Opens it, on the first row that acts.
    #[must_use]
    pub fn fresh(
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Self {
        let mut me = Self::default();
        me.settle_in(r, zone, columns, texts);
        // A QUESTION THAT CAME BY ITSELF — what a sync offered, a file a read
        // repaired — draws its "no" first, and the cursor is born on it.
        let rows = me.rows(r, zone, columns, texts);
        if let Some(n) = rows.iter().position(|row| {
            matches!(
                row.target,
                Some(Target::Offer(_, false) | Target::LeaveIt(_))
            )
        }) {
            me.cursor = n;
            me.aim = rows[n].target.clone();
        }
        me
    }

    /// Where the cursor is, for the door that draws.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The question a line is open for.
    #[must_use]
    pub const fn asking(&self) -> Option<&Ask> {
        self.asking.as_ref()
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(r, zone, columns, texts);
        let target = rows.get(self.cursor).and_then(|row| row.target.clone());
        let both = target
            .as_ref()
            .and_then(Target::conflict)
            .and_then(|p| conflict_of(r, p))
            .is_some_and(|c| Decision::KeepBoth.allowed(c));
        let asking = matches!(self.mode, Mode::Question(_));
        let mut here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            // ON A QUESTION'S PAGE, only what answers it.
            .filter(|(_, id, _)| {
                !asking
                    || matches!(
                        *id,
                        "belongings-down" | "belongings-up" | "belongings-enter"
                    )
            })
            .filter(|(_, _, w)| match w {
                Where::Anywhere => true,
                Where::OnAValue => target.as_ref().is_some_and(Target::turns),
                Where::OnAConflict => target.as_ref().and_then(Target::conflict).is_some(),
                Where::OnAConflictBoth => both,
            })
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        // THE CONSENT ROW'S OWN KEY, taught while the row is drawn and only
        // then: a key that does nothing is not taught.
        if self.mode == Mode::List && r.unlet.is_some() {
            here.extend(
                crate::consent_rows::OF_THE_CONSENT
                    .iter()
                    .map(|(k, id)| (*k, *id, false)),
            );
        }
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// Puts the cursor where a click landed. Answers whether the cursor was
    /// ALREADY on that row: that is the second click, which the door turns
    /// into `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let Some(target) = rows.get(n).and_then(|r| r.target.clone()) else {
            return false;
        };
        let again = self.cursor == n;
        self.cursor = n;
        self.aim = Some(target);
        again
    }

    /// The sentence the pointer resting on row `n` reads.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        rows.get(n)?.target.as_ref()?.about(texts)
    }

    /// Puts the cursor back on the row it was aimed at — found by what it is,
    /// since the list may have changed under it — or on the nearest row that
    /// acts. Called after every reading.
    pub fn settle_in(
        &mut self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        // A DIFFERENCE OF A CONFLICT THAT IS GONE goes back to the list.
        if let Mode::Diff(path, _) = &self.mode
            && conflict_of(r, path).is_none()
        {
            self.mode = Mode::List;
        }
        let rows = self.rows(r, zone, columns, texts);
        if let Some(aim) = self.aim.as_ref()
            && let Some(n) = rows
                .iter()
                .position(|row| row.target.as_ref().is_some_and(|t| t.same_row(aim)))
        {
            self.cursor = n;
            self.aim = rows[n].target.clone();
            return;
        }
        if rows.get(self.cursor).is_some_and(|row| !row.heading()) {
            self.aim = rows[self.cursor].target.clone();
            return;
        }
        let at = self.cursor.min(rows.len().saturating_sub(1));
        let found = (at..rows.len())
            .chain((0..at).rev())
            .find(|n| !rows[*n].heading());
        self.cursor = found.unwrap_or(0);
        self.aim = rows.get(self.cursor).and_then(|row| row.target.clone());
    }

    /// The rows to draw, at this width, from what is read now.
    #[must_use]
    pub fn rows(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = crate::chrome::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            cursor: self.cursor,
            left,
            room,
            texts,
        };
        if let Mode::Diff(path, lines) = &self.mode {
            diff_page(&mut b, r, path, lines);
            return b.out;
        }
        if let Mode::Question(q) = self.mode {
            question_page(&mut b, q);
            return b.out;
        }
        if r.places.is_none() {
            b.section(
                &b.t(key::BELONGINGS_STATUS),
                vec![note(texts.t(key::BELONGINGS_NO_PLACES))],
            );
            return b.out;
        }
        status_section(&mut b, r, zone);
        offers_section(&mut b, r);
        sync_section(&mut b, r);
        if r.plan.as_ref().is_some_and(|p| !p.conflicts.is_empty()) {
            conflicts_section(&mut b, r, zone);
        }
        repairs_section(&mut b, r);
        history_section(&mut b, r);
        listing_section(&mut b, r);
        signature_section(&mut b, r);
        access_section(&mut b, r);
        passphrase_section(&mut b, r, zone);
        period_section(&mut b, r);
        b.out
    }

    /// What a key does. THE WIDTH IS THE DOOR'S: the cursor is an index into
    /// the rows that were DRAWN.
    pub fn press(
        &mut self,
        p: Press,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(r, zone, columns, texts);
        let target = rows.get(self.cursor).and_then(|row| row.target.clone());
        // A QUESTION'S PAGE: up, down, the answer, and `Esc` for "no".
        if let Mode::Question(q) = self.mode {
            return match p {
                Press::Up | Press::Down => {
                    self.walk(&rows, p == Press::Down);
                    Errand::Nothing
                }
                Press::Escape => self.answer(q, false, r, zone, columns, texts),
                Press::Enter => match target {
                    Some(Target::Answer(yes)) => self.answer(q, yes, r, zone, columns, texts),
                    _ => Errand::Nothing,
                },
                _ => Errand::Nothing,
            };
        }
        match p {
            Press::Up | Press::Down => {
                self.walk(&rows, p == Press::Down);
                Errand::Nothing
            }
            Press::Escape => match self.mode {
                Mode::Diff(ref path, _) => {
                    let path = path.clone();
                    self.mode = Mode::List;
                    self.aim = Some(Target::Conflict(path));
                    self.settle_in(r, zone, columns, texts);
                    Errand::Nothing
                }
                Mode::List | Mode::Question(_) => Errand::Close,
            },
            Press::Less | Press::More => match target {
                Some(Target::Period) => self.change(r, texts, |prefs| {
                    prefs.period_days = if p == Press::More {
                        Belongings::next_period(prefs.period_days)
                    } else {
                        previous_period(prefs.period_days)
                    };
                }),
                Some(Target::Auth) => self.change(r, texts, flip_auth),
                Some(Target::Unlock) => self.flip_unlock(r, texts),
                _ => Errand::Nothing,
            },
            // ON A FIELD'S ROWS the conflict's keys decide THE FIELD.
            Press::Decide(d) => match target.as_ref().and_then(Target::field) {
                Some((path, pointer)) => {
                    let (path, pointer) = (path.to_owned(), pointer.to_owned());
                    decide_field(r, &path, &pointer, d, texts)
                }
                None => match target.as_ref().and_then(Target::conflict) {
                    Some(path) => {
                        let path = path.to_owned();
                        self.decide(r, &path, d, texts)
                    }
                    None => Errand::Say(texts.t(key::BELONGINGS_NO_CONFLICT_HERE)),
                },
            },
            Press::Sync => self.start_sync(r, texts),
            Press::Allow => match r.unlet.as_deref() {
                Some(host) => let_host(r, host, texts),
                None => Errand::Nothing,
            },
            Press::Enter => match target {
                Some(t) => self.enter(t, r, zone, columns, texts),
                None => Errand::Nothing,
            },
        }
    }

    /// A command by its name — `LEADER :`, a leader key, a click on a hint,
    /// the brain's mark. What `RET` on its row does, whatever row the cursor
    /// is on.
    pub fn command(
        &mut self,
        id: &str,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if let Some(p) = press_of(id) {
            return self.press(p, r, zone, columns, texts);
        }
        // ── anchor: L09 ──
        // THE TWO MERGED DECISIONS, on the conflict under the cursor — as its
        // rows decide it (`Target::Decide`).
        let merged = match id {
            "belongings-take-merged-ours" => Some(Decision::TakeMergedOurs),
            "belongings-take-merged-theirs" => Some(Decision::TakeMergedTheirs),
            _ => None,
        };
        if let Some(d) = merged {
            return self.press(Press::Decide(d), r, zone, columns, texts);
        }
        let target = match id {
            "belongings-apply" => Target::Apply,
            "belongings-abandon" => Target::Abandon,
            "belongings-all-local" => Target::All(Decision::KeepLocal),
            "belongings-all-remote" => Target::All(Decision::TakeRemote),
            "belongings-repository" => Target::Repository,
            "belongings-add" => Target::Add,
            "belongings-auth" => Target::Auth,
            "belongings-keygen" => Target::Keygen,
            "belongings-copy-key" => Target::CopyKey,
            "belongings-token" => Target::Token,
            "belongings-user" => Target::User,
            "belongings-passphrase" => Target::Passphrase,
            "belongings-period" => Target::Period,
            "belongings-pin" => Target::Pin,
            "belongings-sign" => Target::Sign,
            _ => return Errand::Nothing,
        };
        self.enter(target, r, zone, columns, texts)
    }

    /// `RET` on a row.
    fn enter(
        &mut self,
        target: Target,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match target {
            Target::Fact(word) => {
                let rows = self.rows(r, zone, columns, texts);
                rows.iter()
                    .find(|row| row.target.as_ref() == Some(&Target::Fact(word.clone())))
                    .and_then(|row| row.fields.last())
                    .map(|f| f.text.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .map_or(Errand::Nothing, Errand::Copy)
            }
            Target::DiffLine(path, n) => {
                let rows = self.rows(r, zone, columns, texts);
                let target = Target::DiffLine(path, n);
                rows.iter()
                    .find(|row| row.target.as_ref() == Some(&target))
                    .map(|row| {
                        row.fields
                            .iter()
                            .map(|f| f.text.as_str())
                            .collect::<String>()
                    })
                    .filter(|t| !t.trim().is_empty())
                    .map_or(Errand::Nothing, Errand::Copy)
            }
            Target::Repository => self.ask(Ask::Repository, r),
            Target::Sync => self.start_sync(r, texts),
            Target::Apply => self.apply(r, texts),
            Target::Abandon => {
                if r.plan.is_none() {
                    return Errand::Say(texts.t(key::BELONGINGS_NOTHING_PREPARED));
                }
                if r.busy {
                    return Errand::Say(texts.t(words::refusal_key(Why::Busy)));
                }
                Errand::Queue(Op::Abandon)
            }
            Target::All(d) => {
                let Some(plan) = r.plan.as_ref() else {
                    return Errand::Say(texts.t(key::BELONGINGS_NOTHING_PREPARED));
                };
                let mut decided = r.decided();
                decided.extend(plan.all(d));
                write_decisions(r, &decided, texts)
            }
            Target::Conflict(path) => self.open_diff(r, &path, zone, columns, texts),
            Target::Decide(path, d) => self.decide(r, &path, d, texts),
            Target::Rule(path) => self.change(r, texts, |p| {
                p.stop_keeping(&path);
            }),
            Target::Kept(path) => self.change(r, texts, |p| {
                p.stop_keeping(&path);
            }),
            Target::Excluded(path) | Target::Never(path) => self.change(r, texts, |p| {
                p.keep(&path);
            }),
            Target::Add => self.ask(Ask::Path, r),
            Target::Auth => self.change(r, texts, flip_auth),
            Target::Keygen => {
                if r.has_key {
                    return Errand::Say(texts.t(key::BELONGINGS_KEY_ALREADY));
                }
                Errand::Queue(Op::Keygen)
            }
            Target::CopyKey => r.public_key.clone().map_or_else(
                || Errand::Say(texts.t(key::BELONGINGS_NO_KEY_YET)),
                Errand::Copy,
            ),
            Target::Token => self.ask(Ask::Token, r),
            Target::User => self.ask(Ask::User, r),
            Target::Passphrase => {
                if !r.prefs.has_repository() {
                    return Errand::Say(texts.t(words::refusal_key(Why::NoRepository)));
                }
                if r.first_run() {
                    return Errand::Say(texts.t(key::BELONGINGS_PASSPHRASE_FIRST));
                }
                // A CHANGE OF PASSPHRASE WRITES THE PASSPHRASE'S FILE, which a
                // waiting sync and a running one are using.
                if let Some(said) = in_the_way(r, texts) {
                    return Errand::Say(said);
                }
                self.ask(Ask::Old, r)
            }
            Target::Period => self.change(r, texts, |p| {
                p.period_days = Belongings::next_period(p.period_days);
            }),
            Target::Let(host) => let_host(r, &host, texts),
            Target::File(path, act) => self.change(r, texts, |p| match act {
                FileAct::StopKeeping => {
                    p.stop_keeping(&path);
                }
                FileAct::KeepAgain | FileAct::KeepAnyway => {
                    p.keep(&path);
                }
            }),
            // "NOT ON THIS MACHINE": its paths taken out here; the
            // repository's copies stay for the others.
            Target::NotHere(id) => self.change(r, texts, |p| {
                for path in target_paths(id) {
                    p.stop_keeping(path);
                }
            }),
            Target::HereAgain(id) => self.change(r, texts, |p| {
                for path in target_paths(id) {
                    p.keep(path);
                }
            }),
            // "NOT ON ANY MACHINE" deletes the repository's copies: asked
            // first, "no" first.
            Target::Nowhere(id) => self.ask_page(Question::Nowhere(id), r, zone, columns, texts),
            // "EVERY MACHINE AGAIN": asked of the next sync, which this door
            // starts now — kept here too.
            Target::Everywhere(id) => {
                if let Err(said) = ask_of_the_next_sync(
                    r,
                    Request::KeepEverywhere {
                        target: id.to_owned(),
                    },
                ) {
                    return Errand::Say(texts.t(said));
                }
                if let Err(said) = change_the_file(r.places.as_ref(), |p| {
                    for path in target_paths(id) {
                        p.keep(path);
                    }
                }) {
                    return Errand::Say(said.sentence(texts));
                }
                self.start_sync(r, texts)
            }
            Target::Field(path, pointer) => {
                // `RET` ON A FIELD takes the other value: this machine's
                // first.
                let d = match r.fields_file().field(&path, &pointer) {
                    Some(Decision::KeepLocal) => Decision::TakeRemote,
                    _ => Decision::KeepLocal,
                };
                decide_field(r, &path, &pointer, d, texts)
            }
            Target::DecideField(path, pointer, d) => decide_field(r, &path, &pointer, d, texts),
            Target::WriteFix(path) => repair(r, &path, true, texts),
            Target::LeaveIt(path) => repair(r, &path, false, texts),
            Target::Offer(offer, yes) => {
                let (Some(places), Some(status)) = (r.places.as_ref(), r.status.as_ref()) else {
                    return Errand::Nothing;
                };
                // AN OFFER THE LAST SYNC DID NOT MAKE is answered by nobody:
                // the row the person read is gone.
                if !r.offers().contains(&offer) {
                    return Errand::Nothing;
                }
                if yes {
                    return Errand::Accept(status.at_s, offer);
                }
                match answer(places, status.at_s, &offer, Said::No) {
                    Ok(()) => Errand::Nothing,
                    Err(said) => Errand::Say(texts.t(said)),
                }
            }
            Target::Unlock => self.flip_unlock(r, texts),
            Target::Compact => self.ask_page(Question::Compact, r, zone, columns, texts),
            // AN ANSWER ON THE LIST is no answer: the page is gone.
            Target::Answer(_) => Errand::Nothing,
            // THE PIN IS A ROOT GESTURE: its password, key or finger is the
            // sudo bar's to ask, never this view's line.
            Target::Pin => match pin_refusal(r) {
                Some(said) => Errand::Say(texts.t(said)),
                None => Errand::Pin {
                    key: r.plain.signer.clone(),
                    fingerprint: r.plain.fingerprint.clone(),
                },
            },
            Target::Sign => {
                if let Some(said) = unready(r, texts) {
                    return Errand::Say(said);
                }
                if r.kept_until.is_some() {
                    return Errand::Queue(Op::Sign);
                }
                self.ask(Ask::Sign, r)
            }
        }
    }

    /// A question on a page of its own, "no" first and the cursor on it.
    fn ask_page(
        &mut self,
        q: Question,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // A COMPACTION THAT COULD NOT START is refused before anything is
        // asked.
        if matches!(q, Question::Compact)
            && let Some(said) = unready(r, texts).or_else(|| not_let(r, texts))
        {
            return Errand::Say(said);
        }
        self.mode = Mode::Question(q);
        self.aim = Some(Target::Answer(false));
        self.cursor = 0;
        self.settle_in(r, zone, columns, texts);
        Errand::Nothing
    }

    /// The question answered: back to the list, on the row that asked —
    /// and, for a "yes", what it said.
    fn answer(
        &mut self,
        q: Question,
        yes: bool,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.mode = Mode::List;
        self.aim = Some(q.asker());
        self.settle_in(r, zone, columns, texts);
        if !yes {
            return Errand::Nothing;
        }
        match q {
            // ASKED OF THE NEXT SYNC, which this door starts now: only a sync
            // with the passphrase writes the repository.
            Question::Nowhere(id) => {
                if let Err(said) = ask_of_the_next_sync(
                    r,
                    Request::DropEverywhere {
                        target: id.to_owned(),
                    },
                ) {
                    return Errand::Say(texts.t(said));
                }
                self.start_sync(r, texts)
            }
            Question::Compact => {
                if let Some(said) = unready(r, texts).or_else(|| not_let(r, texts)) {
                    return Errand::Say(said);
                }
                if r.asks_nothing() {
                    return Errand::Queue(Op::Compact);
                }
                self.ask(Ask::Compact, r)
            }
        }
    }

    /// Turns "remember the passphrase for this session": off forgets what
    /// the session keeps, at once (`forget` on the queue).
    fn flip_unlock(&self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let on = !r.prefs.keep_unlocked;
        match change_the_file(r.places.as_ref(), |p| p.keep_unlocked = on) {
            Ok(()) if on => Errand::Nothing,
            Ok(()) => Errand::Queue(Op::Forget),
            Err(said) => Errand::Say(said.sentence(texts)),
        }
    }

    /// Opens a question, remembered: the answer alone cannot say what it
    /// answers.
    fn ask(&mut self, ask: Ask, r: &Readings) -> Errand {
        let line = ask.line(r);
        self.asking = Some(ask);
        Errand::Ask(line)
    }

    /// Sync now: refused with its sentence when there is nothing to sync
    /// with, else the passphrase is asked.
    fn start_sync(&mut self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        if let Some(said) = unready(r, texts).or_else(|| not_let(r, texts)) {
            return Errand::Say(said);
        }
        // THE SESSION REMEMBERS IT: the sync asks nothing
        // (`ronoctua_belongings::unlock`).
        if r.asks_nothing() {
            return Errand::Queue(Op::Prepare);
        }
        self.ask(Ask::Passphrase, r)
    }

    /// Apply the decisions: refused while one is missing, or while a run
    /// holds the lock.
    fn apply(&self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(plan) = r.plan.as_ref() else {
            return Errand::Say(texts.t(key::BELONGINGS_NOTHING_PREPARED));
        };
        if r.busy {
            return Errand::Say(texts.t(words::refusal_key(Why::Busy)));
        }
        // AN APPLY PUSHES: refused by the CLI with no pass, it would take the
        // session and the decisions with it — so it is refused here first.
        if let Some(said) = not_let(r, texts) {
            return Errand::Say(said);
        }
        let decided = r.decided();
        if !plan.conflicts.iter().all(|c| r.settled(c)) {
            let left = plan.conflicts.iter().filter(|c| !r.settled(c)).count();
            return Errand::Say(texts.t_args(
                key::BELONGINGS_UNDECIDED_LEFT,
                &[(
                    "n",
                    ronoctua_i18n::Arg::Number(i64::try_from(left).unwrap_or(i64::MAX)),
                )],
            ));
        }
        // WHAT IS DECIDED, WRITTEN BEFORE THE ORDER: the CLI reads the file,
        // and a suggestion nobody touched is a decision too.
        match write_decisions_file(r, &decided) {
            Ok(()) => Errand::Queue(Op::Apply),
            Err(said) => Errand::Say(texts.t(said)),
        }
    }

    /// One conflict decided.
    fn decide(
        &self,
        r: &Readings,
        path: &str,
        d: Decision,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(c) = conflict_of(r, path) else {
            return Errand::Say(texts.t(key::BELONGINGS_NO_CONFLICT_HERE));
        };
        if !d.allowed(c) {
            // EACH REFUSAL WITH ITS OWN WORDS: a merge nobody reached, or
            // "keep both" on what the person did not add.
            let why = match d {
                Decision::TakeMergedOurs | Decision::TakeMergedTheirs => {
                    key::BELONGINGS_NO_MERGE_HERE
                }
                _ => key::BELONGINGS_BOTH_ONLY_ADDED,
            };
            return Errand::Say(texts.t(why));
        }
        let mut decided = r.decided();
        decided.insert(path.to_owned(), d);
        write_decisions(r, &decided, texts)
    }

    /// The difference of a conflicting text, on a page of its own.
    fn open_diff(
        &mut self,
        r: &Readings,
        path: &str,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(c) = conflict_of(r, path) else {
            return Errand::Nothing;
        };
        let file = c
            .theirs
            .as_ref()
            .zip(r.places.as_ref())
            .map(|(name, p)| p.theirs(&format!("{name}.diff")));
        let Some(text) = file.and_then(|f| std::fs::read_to_string(f).ok()) else {
            return Errand::Say(texts.t(if c.text {
                key::BELONGINGS_NO_DIFFERENCE
            } else {
                key::BELONGINGS_NOT_TEXT
            }));
        };
        let lines: Vec<String> = text
            .lines()
            .take(DIFF_MOST_LINES)
            .map(|l| l.replace('\t', "    "))
            .collect();
        self.mode = Mode::Diff(path.to_owned(), lines);
        self.aim = Some(Target::Decide(path.to_owned(), Decision::KeepLocal));
        self.cursor = 0;
        self.settle_in(r, zone, columns, texts);
        Errand::Nothing
    }

    /// One change to `belongings.json`, laid over the file as it is NOW.
    fn change(
        &self,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
        change: impl FnOnce(&mut Belongings),
    ) -> Errand {
        match change_the_file(r.places.as_ref(), change) {
            Ok(()) => Errand::Nothing,
            Err(said) => Errand::Say(said.sentence(texts)),
        }
    }

    /// What was typed on a line, under the line's id — one call for both
    /// doors: the footer's minibuffer answers through the command of that
    /// id, the window through `Content::answered`.
    ///
    /// THE QUESTION THE VIEW ASKED ANSWERS, and only when the id is its own:
    /// a question let go, then another command typed, is not answered as the
    /// old one.
    pub fn answered(
        &mut self,
        id: &str,
        said: &str,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(ask) = self.asking.take().filter(|a| a.command() == id) else {
            return Errand::Nothing;
        };
        let typed = Typed::new(said.to_owned());
        match ask {
            Ask::Repository => {
                let address = said.trim();
                if let Err(why) = prefs::repository_is_plain(address) {
                    return Errand::Say(texts.t(words::invalid_key(why)));
                }
                // TYPING THE ADDRESS IS THE CONSENT to ask its host, kept in
                // `consent.json` like any other: no sync or compaction
                // reaches a host the person did not let.
                if let Some(host) = prefs::host_of(address)
                    && let Errand::Say(said) = let_host(r, &host, texts)
                {
                    return Errand::Say(said);
                }
                self.change(r, texts, |p| address.clone_into(&mut p.repository))
            }
            Ask::User => {
                let user = said.trim().to_owned();
                self.change(r, texts, |p| p.user = (!user.is_empty()).then_some(user))
            }
            Ask::Path => {
                let Some(places) = r.places.as_ref() else {
                    return Errand::Say(texts.t(key::BELONGINGS_NO_PLACES));
                };
                match logical_of_typed(places, said.trim()) {
                    Some(logical) if kept::plain_root(&logical).is_some() => {
                        Errand::Say(texts.t(key::BELONGINGS_PLAIN_ALREADY))
                    }
                    Some(logical) if kept::may_add(&logical) => self.change(r, texts, |p| {
                        p.keep(&logical);
                    }),
                    Some(_) => Errand::Say(texts.t(key::BELONGINGS_NEVER_KEPT)),
                    None => Errand::Say(texts.t(key::BELONGINGS_OUTSIDE_HOME)),
                }
            }
            Ask::Token => {
                if typed.expose().trim().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                match write_secret(r, Places::token, typed.expose().trim()) {
                    Ok(()) => Errand::Say(texts.t(key::BELONGINGS_TOKEN_KEPT)),
                    Err(said) => Errand::Say(texts.t(said)),
                }
            }
            Ask::Passphrase => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                if let Some(refused) = unready(r, texts) {
                    return Errand::Say(refused);
                }
                // A FIRST SYNC ASKS IT TWICE: when the repository is empty,
                // what is typed becomes its passphrase for good.
                if r.first_run() {
                    return self.ask(Ask::Confirm(typed), r);
                }
                send_passphrase(r, &typed, texts)
            }
            Ask::Compact => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                if let Some(refused) = unready(r, texts) {
                    return Errand::Say(refused);
                }
                match write_secret(r, Places::passphrase, typed.expose()) {
                    Ok(()) => Errand::Queue(Op::Compact),
                    Err(said) => Errand::Say(texts.t(said)),
                }
            }
            Ask::Confirm(first) => {
                if typed != first {
                    return Errand::Say(texts.t(key::BELONGINGS_FIRST_PASSPHRASES_DIFFER));
                }
                if let Some(refused) = unready(r, texts) {
                    return Errand::Say(refused);
                }
                send_passphrase(r, &typed, texts)
            }
            Ask::Old => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                self.ask(Ask::New(typed), r)
            }
            Ask::New(old) => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                self.ask(Ask::Again(old, typed), r)
            }
            Ask::Sign => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                if let Some(refused) = unready(r, texts) {
                    return Errand::Say(refused);
                }
                match write_secret(r, Places::passphrase, typed.expose()) {
                    Ok(()) => Errand::Queue(Op::Sign),
                    Err(said) => Errand::Say(texts.t(said)),
                }
            }
            Ask::Again(old, new) => {
                if typed != new {
                    return Errand::Say(texts.t(key::BELONGINGS_PASSPHRASES_DIFFER));
                }
                let written = write_secret(r, Places::passphrase, old.expose())
                    .and_then(|()| write_secret(r, Places::new_passphrase, new.expose()));
                match written {
                    Ok(()) => Errand::Queue(Op::Rekey),
                    Err(said) => Errand::Say(texts.t(said)),
                }
            }
        }
    }

    /// The line was let go — `Esc`, `C-g`, a click on `ESC` — and the
    /// question with it.
    pub fn let_go(&mut self) {
        self.asking = None;
    }

    /// Moves the cursor to the next row that acts, round the ends.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows.get(at).is_some_and(|r| !r.heading()) {
                self.cursor = at;
                self.aim = rows[at].target.clone();
                return;
            }
        }
    }
}

/// Why a sync cannot start now — no repository, no access, a settings file
/// that does not parse, a sync prepared and waiting, a run going on — as its
/// sentence; `None` when it can.
fn unready(r: &Readings, texts: &ronoctua_i18n::Texts) -> Option<String> {
    if r.places.is_none() {
        return Some(texts.t(key::BELONGINGS_NO_PLACES));
    }
    if r.prefs_refused {
        return Some(texts.t(key::BELONGINGS_PREFS_REFUSED));
    }
    if !r.prefs.has_repository() {
        return Some(texts.t(words::refusal_key(Why::NoRepository)));
    }
    // AN ADDRESS KEPT BEFORE A RULE REFUSED IT — a host no consent can name,
    // which format 1 took — says why here, as it would when typed, instead of
    // the CLI's "no repository" under an address the page shows.
    if let Err(why) = prefs::repository_is_plain(&r.prefs.repository) {
        return Some(texts.t(words::invalid_key(why)));
    }
    if !r.has_access() {
        return Some(texts.t(match r.prefs.auth {
            Auth::Ssh => key::BELONGINGS_NEEDS_KEY,
            Auth::Token => key::BELONGINGS_NEEDS_TOKEN,
        }));
    }
    in_the_way(r, texts)
}

/// The sentence of a run the repository's host would refuse: the person
/// has not let it be asked yet (the row under Sync asks it).
fn not_let(r: &Readings, texts: &ronoctua_i18n::Texts) -> Option<String> {
    r.unlet
        .as_ref()
        .map(|_| texts.t(words::refusal_key(Why::NotLet)))
}

/// Why the person's key cannot be pinned now, as its sentence's key: no key
/// is known here yet (only a run that opened `key.age` knows it), the
/// machine trusts it already, or a pin is on its way. `None` when it can —
/// a door with no root helper says so when it is asked.
fn pin_refusal(r: &Readings) -> Option<&'static str> {
    if r.places.is_none() {
        return Some(key::BELONGINGS_NO_PLACES);
    }
    if r.plain.signer.is_empty() || r.plain.fingerprint.is_empty() {
        return Some(key::BELONGINGS_SIGNATURE_SYNC_FIRST);
    }
    if r.pin
        .as_ref()
        .is_some_and(|p| p.fingerprint == r.plain.fingerprint)
    {
        return Some(key::BELONGINGS_SIGNATURE_ALREADY);
    }
    if r.pinning
        .as_ref()
        .is_some_and(|p| p.said == PinSaid::Pinning)
    {
        return Some(key::BELONGINGS_PINNING_BUSY);
    }
    None
}

/// Lets `host` be asked: the consent row pressed, or the address typed —
/// merged into `consent.json` under the config root the view reads, which in
/// the product is the person's own, where the CLI mints its pass. What could
/// not be kept is said (`consent_rows::unkept`), never silent.
fn let_host(r: &Readings, host: &str, texts: &ronoctua_i18n::Texts) -> Errand {
    let Some(places) = r.places.as_ref() else {
        return Errand::Say(texts.t(key::BELONGINGS_NO_PLACES));
    };
    match ronoctua_consent::let_hosts_under(&places.config, &[host], true) {
        Ok(()) => Errand::Nothing,
        Err(why) => {
            tracing::warn!(why = why.id(), host, "a consent could not be kept");
            Errand::Say(crate::consent_rows::unkept(why, &[host], texts))
        }
    }
}

/// What stands in the way of a new run: a sync prepared and waiting — a new
/// `prepare` would throw the person's decisions away — or a run that holds
/// the lock now.
fn in_the_way(r: &Readings, texts: &ronoctua_i18n::Texts) -> Option<String> {
    if r.plan.is_some() {
        return Some(texts.t(key::BELONGINGS_PLAN_WAITS));
    }
    r.busy.then(|| texts.t(words::refusal_key(Why::Busy)))
}

/// The passphrase into its runtime file, and the order after it.
fn send_passphrase(r: &Readings, typed: &Typed, texts: &ronoctua_i18n::Texts) -> Errand {
    match write_secret(r, Places::passphrase, typed.expose()) {
        Ok(()) => Errand::Queue(Op::Prepare),
        Err(said) => Errand::Say(texts.t(said)),
    }
}

/// The conflict of `path` in the plan that waits.
fn conflict_of<'a>(r: &'a Readings, path: &str) -> Option<&'a Conflict> {
    r.plan.as_ref()?.conflicts.iter().find(|c| c.path == path)
}

/// The period before `days`, round the list.
fn previous_period(days: Option<u16>) -> Option<u16> {
    let all = prefs::PERIODS;
    let at = all.iter().position(|p| *p == days).unwrap_or(0);
    all[(at + all.len() - 1) % all.len()]
}

/// The other access.
fn flip_auth(p: &mut Belongings) {
    p.auth = match p.auth {
        Auth::Ssh => Auth::Token,
        Auth::Token => Auth::Ssh,
    };
}

/// A path the person typed as a logical path: `~/notes`, `notes` (under the
/// home), or an absolute path under the home. `None` outside it.
#[must_use]
pub fn logical_of_typed(places: &Places, typed: &str) -> Option<String> {
    if typed.is_empty() {
        return None;
    }
    let path = if let Some(rest) = typed.strip_prefix("~/") {
        places.home.join(rest)
    } else if typed == "~" {
        places.home.clone()
    } else if Path::new(typed).is_absolute() {
        PathBuf::from(typed)
    } else {
        places.home.join(typed)
    };
    let clean = lexically_clean(&path)?;
    places.logical_of(&clean)
}

// ---------------------------------------------------------------------------
// WRITES — the ones both doors make, so they are made once
// ---------------------------------------------------------------------------

/// A secret into its runtime or access file, `0600` from birth.
fn write_secret(r: &Readings, at: fn(&Places) -> PathBuf, text: &str) -> Result<(), &'static str> {
    let places = r.places.as_ref().ok_or(key::BELONGINGS_NO_PLACES)?;
    ronoctua_belongings::secret::write_private(&at(places), text.as_bytes()).map_err(|e| {
        tracing::warn!(error = %e, "a belongings secret could not be written");
        key::BELONGINGS_NOT_WRITTEN
    })
}

/// The decisions into their file — the paths' as decided here, the fields'
/// as the file has them.
fn write_decisions_file(r: &Readings, decided: &Decisions) -> Result<(), &'static str> {
    let places = r.places.as_ref().ok_or(key::BELONGINGS_NO_PLACES)?;
    let file = DecisionsFile {
        paths: decided.clone(),
        fields: DecisionsFile::read(&places.decisions())
            .map(|f| f.fields)
            .unwrap_or_default(),
    };
    let text = serde_json::to_vec_pretty(&file).map_err(|_| key::BELONGINGS_NOT_WRITTEN)?;
    ronoctua_belongings::secret::write_private(&places.decisions(), &text).map_err(|e| {
        tracing::warn!(error = %e, "the belongings' decisions could not be written");
        key::BELONGINGS_NOT_WRITTEN
    })
}

/// The same, as an errand.
fn write_decisions(r: &Readings, decided: &Decisions, texts: &ronoctua_i18n::Texts) -> Errand {
    match write_decisions_file(r, decided) {
        Ok(()) => Errand::Nothing,
        Err(said) => Errand::Say(texts.t(said)),
    }
}

/// The order on the transfer queue — the one call both doors make. Nothing
/// secret crosses: the order names a verb. When the queue does not take it,
/// what the door typed for it is deleted at once ([`forget_unsent`]): nobody
/// else would.
///
/// # Errors
///
/// The queue is not there.
pub fn queue(op: Op, places: Option<&Places>) -> anyhow::Result<()> {
    let sent = ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add {
        work: ronoctua_transfers::Work::Belongings { op },
    })
    .map(|_| ());
    if sent.is_err()
        && let Some(places) = places
    {
        forget_unsent(op, places);
    }
    sent
}

/// Deletes the secrets a door wrote for an order the queue did not take: the
/// passphrase of a sync, both of a change of passphrase. An apply's
/// passphrase is the waiting sync's, written before, and stays.
pub fn forget_unsent(op: Op, places: &Places) {
    let typed: Vec<PathBuf> = match op {
        Op::Prepare => vec![places.passphrase()],
        Op::Rekey => vec![places.passphrase(), places.new_passphrase()],
        Op::Apply | Op::Abandon | Op::Keygen => Vec::new(),
        // THE 2026-10-05 WAVE'S ORDERS: every one that may read a typed
        // passphrase loses it with the order (their packages refine this).
        // ── anchor: L08 ──
        // ── anchor: L09 ──
        Op::Sign => vec![places.passphrase()],
        // ── anchor: L10 ──
        Op::Compact | Op::Restore | Op::Init | Op::VerifyPassphrase => {
            vec![places.passphrase()]
        }
        Op::SyncPlain | Op::Forget | Op::Probe => Vec::new(),
    };
    for file in typed {
        if let Err(e) = std::fs::remove_file(&file)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = %e, "a belongings secret the queue did not take could not be deleted");
        }
    }
}

/// The sentence that says an order was taken.
#[must_use]
pub fn queued_said(op: Op, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        key::BELONGINGS_QUEUED,
        &[("what", &texts.t(words::op_key(op)))],
    )
}

// ---------------------------------------------------------------------------
// THE SECTIONS
// ---------------------------------------------------------------------------

/// Status: the repository, the last sync, the next reminder, the last run.
fn status_section(b: &mut Builder<'_>, r: &Readings, zone: &ronoctua_files::when::Zone) {
    let t = b.texts;
    let mut e = Vec::new();
    let repository = if r.prefs.has_repository() {
        r.prefs.repository.clone()
    } else {
        t.t(key::BELONGINGS_NONE_YET)
    };
    e.push(pair(
        t.t(key::BELONGINGS_REPOSITORY),
        repository,
        Target::Repository,
        true,
    ));
    if r.prefs_refused {
        e.push(Entry::Note(
            t.t(key::BELONGINGS_PREFS_REFUSED),
            tokens::color::STATE_WARNING,
        ));
    }
    let last = r
        .last
        .as_ref()
        .filter(|l| l.repository == r.prefs.repository);
    e.push(pair(
        t.t(key::BELONGINGS_LAST_SYNC),
        last.map_or_else(
            || t.t(key::BELONGINGS_NEVER_HERE),
            |l| when(l.at_s, zone, t),
        ),
        Target::Fact("last".to_owned()),
        false,
    ));
    let next = match (
        r.prefs.has_repository(),
        ronoctua_belongings::status::due_at(last.map(|l| l.at_s), r.prefs.period_days),
    ) {
        (false, _) => None,
        (true, Some(at)) if at <= r.now_s => Some(t.t(key::BELONGINGS_DUE_NOW)),
        (true, Some(at)) => Some(when(at, zone, t)),
        (true, None) if r.prefs.period_days.is_none() => Some(t.t(key::BELONGINGS_NO_REMINDER)),
        (true, None) => Some(t.t(key::BELONGINGS_DUE_NOW)),
    };
    if let Some(next) = next {
        e.push(pair(
            t.t(key::BELONGINGS_NEXT),
            next,
            Target::Fact("next".to_owned()),
            false,
        ));
    }
    if let Some(l) = last {
        e.push(pair(
            t.t(key::BELONGINGS_KEPT_NOW),
            t.t_args(
                key::BELONGINGS_FILES_WEIGH,
                &[
                    (
                        "n",
                        ronoctua_i18n::Arg::Number(i64::try_from(l.files).unwrap_or(i64::MAX)),
                    ),
                    ("size", ronoctua_i18n::Arg::Text(&bytes_text(l.bytes, t))),
                ],
            ),
            Target::Fact("kept".to_owned()),
            false,
        ));
    }
    if let Some(s) = r.status.as_ref() {
        // A SYNC THAT WAITED, ITS SESSION GONE — the runtime folder is emptied
        // when the person's session ends — waits for nothing any more.
        let said = match s.outcome {
            Outcome::Waiting { .. } if r.plan.is_none() => t.t(key::BELONGINGS_PREPARED_GONE),
            _ => outcome_text(&s.outcome, t),
        };
        if s.failed() {
            e.push(tinted(
                t.t(key::BELONGINGS_LAST_RUN),
                when(s.at_s, zone, t),
                Target::Fact("run".to_owned()),
                false,
                tokens::color::STATE_WARNING,
            ));
            e.push(Entry::Note(said, tokens::color::STATE_WARNING));
        } else {
            e.push(pair(
                t.t(key::BELONGINGS_LAST_RUN),
                when(s.at_s, zone, t),
                Target::Fact("run".to_owned()),
                false,
            ));
            e.push(note(said));
        }
        if let Outcome::Synced {
            large,
            unreadable,
            unwritten,
            ..
        } = &s.outcome
        {
            for (path, bytes) in large {
                e.push(Entry::Note(
                    t.t_with(
                        key::BELONGINGS_LARGE,
                        &[
                            ("name", &shown_path(path, r.places.as_ref(), false)),
                            ("size", &bytes_text(*bytes, t)),
                        ],
                    ),
                    tokens::color::STATE_WARNING,
                ));
            }
            if !unreadable.is_empty() {
                e.push(Entry::Note(
                    t.t_args(
                        key::BELONGINGS_UNREADABLE,
                        &[("n", count(unreadable.len()))],
                    ),
                    tokens::color::STATE_WARNING,
                ));
            }
            if !unwritten.is_empty() {
                e.push(Entry::Note(
                    t.t_args(key::BELONGINGS_UNWRITTEN, &[("n", count(unwritten.len()))]),
                    tokens::color::STATE_WARNING,
                ));
            }
        }
    }
    if r.first_run() {
        e.push(note(t.t(key::BELONGINGS_FIRST_RUN)));
    }
    b.section(&t.t(key::BELONGINGS_STATUS), e);
}

/// A count as a Fluent number.
fn count(n: usize) -> ronoctua_i18n::Arg<'static> {
    ronoctua_i18n::Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

/// Sync: now, or — while a sync waits — apply and drop.
fn sync_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let mut e = Vec::new();
    match r.plan.as_ref() {
        Some(plan) => {
            let done = plan.conflicts.iter().filter(|c| r.settled(c)).count();
            e.push(pair(
                t.t(key::BELONGINGS_APPLY),
                t.t_args(
                    key::BELONGINGS_DECIDED_OF,
                    &[("n", count(done)), ("of", count(plan.conflicts.len()))],
                ),
                Target::Apply,
                true,
            ));
            e.push(pair(
                t.t(key::BELONGINGS_ABANDON),
                String::new(),
                Target::Abandon,
                true,
            ));
            e.push(note(t.t(key::BELONGINGS_WAITING_NOTE)));
            if r.busy {
                e.push(note(t.t(words::refusal_key(Why::Busy))));
            }
        }
        None => {
            e.push(pair(
                t.t(key::BELONGINGS_SYNC_NOW),
                String::new(),
                Target::Sync,
                true,
            ));
            if let Some(said) = unready(r, t) {
                e.push(note(said));
            }
        }
    }
    // THE HOST NOBODY LET YET: the row every app draws for it
    // (`consent_rows`) — its label is the press, its note what for. Without
    // it no sync, apply or compaction reaches the repository.
    if let Some(host) = r.unlet.as_deref() {
        e.push(pair(
            crate::consent_rows::label(&[host], t),
            String::new(),
            Target::Let(host.to_owned()),
            true,
        ));
        e.push(note(crate::consent_rows::note(
            key::BELONGINGS_CONSENT_FOR,
            t,
        )));
    }
    b.section(&t.t(key::BELONGINGS_SYNC_HEADING), e);
}

/// Conflicts: every one, with both sides, and its three decisions — and the
/// two in bulk.
fn conflicts_section(b: &mut Builder<'_>, r: &Readings, zone: &ronoctua_files::when::Zone) {
    let t = b.texts;
    let Some(plan) = r.plan.as_ref() else {
        return;
    };
    let mut e = vec![
        note(t.t(key::BELONGINGS_CONFLICTS_NOTE)),
        pair(
            t.t(key::BELONGINGS_ALL_KEEP_LOCAL),
            String::new(),
            Target::All(Decision::KeepLocal),
            true,
        ),
        pair(
            t.t(key::BELONGINGS_ALL_TAKE_REMOTE),
            String::new(),
            Target::All(Decision::TakeRemote),
            true,
        ),
    ];
    for c in &plan.conflicts {
        // AIR BETWEEN TWO CONFLICTS: each is a path, its two sides and its
        // decisions, read as one.
        e.push(Entry::Air);
        let decision = r.decision_of(c);
        let fields = r.fields_file();
        let value = match decision {
            Some(d) => t.t(words::decided_key(d)),
            None if fields.fields_settle(c) => t.t(key::BELONGINGS_DECIDED_FIELDS),
            None => match &c.merge {
                Some(Merging::Json { fields: all }) if r.fields.contains_key(&c.path) => t.t_args(
                    key::BELONGINGS_FIELDS_DECIDED_OF,
                    &[
                        (
                            "n",
                            count(
                                all.iter()
                                    .filter(|f| fields.field(&c.path, &f.pointer).is_some())
                                    .count(),
                            ),
                        ),
                        ("of", count(all.len())),
                    ],
                ),
                _ => t.t(key::BELONGINGS_UNDECIDED),
            },
        };
        let label = shown_path(&c.path, r.places.as_ref(), false);
        e.push(if r.settled(c) {
            pair(label, value, Target::Conflict(c.path.clone()), true)
        } else {
            tinted(
                label,
                value,
                Target::Conflict(c.path.clone()),
                true,
                tokens::color::STATE_WARNING,
            )
        });
        // THE MACHINE THAT CHANGED IT, when one is known: a plain file's
        // commit names none, and a sentence never says "changed on" nobody.
        let here = side_text(c.local.as_ref(), zone, t);
        let there = side_text(c.remote.as_ref(), zone, t);
        e.push(note(if c.machine.is_empty() {
            t.t_with(
                key::BELONGINGS_SIDES_UNNAMED,
                &[("here", &here), ("there", &there)],
            )
        } else {
            t.t_with(
                key::BELONGINGS_SIDES,
                &[("here", &here), ("there", &there), ("machine", &c.machine)],
            )
        }));
        // WHAT A MERGE REACHED: where the two still clash.
        if let Some(said) = merging_text(c, t) {
            e.push(note(said));
        }
        // WHY THE REPOSITORY'S SIDE WAITS FOR THE PERSON: the signature did
        // not vouch for it — said, so "take it" is a choice made knowing.
        if let Some(why) = c.held {
            e.push(note(t.t(words::held_key(why))));
        }
        // EVERY FIELD THAT CLASHES, on rows of its own: the three values,
        // and this machine's or the repository's decided for it alone.
        if let Some(Merging::Json { fields: clashing }) = &c.merge {
            for f in clashing {
                let chosen = fields.field(&c.path, &f.pointer);
                e.push(indented(
                    f.pointer.clone(),
                    chosen.map_or_else(
                        || t.t(key::BELONGINGS_UNDECIDED),
                        |d| {
                            t.t(if d == Decision::TakeRemote {
                                key::BELONGINGS_FIELD_THEIRS
                            } else {
                                key::BELONGINGS_FIELD_OURS
                            })
                        },
                    ),
                    Target::Field(c.path.clone(), f.pointer.clone()),
                ));
                for (d, label, value) in [
                    (Decision::KeepLocal, key::BELONGINGS_FIELD_OURS, &f.ours),
                    (
                        Decision::TakeRemote,
                        key::BELONGINGS_FIELD_THEIRS,
                        &f.theirs,
                    ),
                ] {
                    e.push(Entry::Field {
                        label: t.t(label),
                        value: value.clone(),
                        chosen: chosen == Some(d),
                        target: Target::DecideField(c.path.clone(), f.pointer.clone(), d),
                        indent: 2 * tokens::space::VIEW_NESTCELLS,
                    });
                }
                // UNDER THE TWO VALUES, in where they are: at the section's
                // left it read as a sentence about the whole conflict.
                let indent = 2 * tokens::space::VIEW_NESTCELLS;
                e.push(Entry::NoteIn(
                    before_text(f.base.as_deref(), b.room.saturating_sub(indent), t),
                    indent,
                ));
            }
        }
        for d in Decision::ALL.into_iter().filter(|d| d.allowed(c)) {
            let chosen = decision == Some(d);
            e.push(indented(
                t.t(words::decide_key(d)),
                if chosen {
                    t.t(key::BELONGINGS_CHOSEN)
                } else {
                    String::new()
                },
                Target::Decide(c.path.clone(), d),
            ));
        }
    }
    b.section(
        &t.t_args(
            key::BELONGINGS_CONFLICTS,
            &[("n", count(plan.conflicts.len()))],
        ),
        e,
    );
}

/// What a merge of a conflict reached, as a sentence: how many hunks of a
/// text clash, or which fields of a document. `None` when no merge reached
/// it.
fn merging_text(c: &Conflict, t: &ronoctua_i18n::Texts) -> Option<String> {
    Some(match c.merge.as_ref()? {
        Merging::Text { hunks } => t.t_args(
            key::BELONGINGS_MERGE_HUNKS,
            &[("n", ronoctua_i18n::Arg::Number(i64::from(*hunks)))],
        ),
        Merging::Json { fields } => {
            let named: Vec<&str> = fields.iter().map(|f| f.pointer.as_str()).collect();
            let named = named.join(", ");
            t.t_args(
                key::BELONGINGS_MERGE_FIELDS,
                &[
                    (
                        "n",
                        ronoctua_i18n::Arg::Number(i64::try_from(fields.len()).unwrap_or(i64::MAX)),
                    ),
                    ("fields", ronoctua_i18n::Arg::Text(&named)),
                ],
            )
        }
    })
}

/// The difference of one conflicting text: its decisions first, then its
/// lines, in the diff's colours and the file's own.
fn diff_page(b: &mut Builder<'_>, r: &Readings, path: &str, lines: &[String]) {
    let t = b.texts;
    let places = r.places.as_ref();
    let mut e = vec![note(t.t(key::BELONGINGS_DIFF_NOTE))];
    if let Some(c) = conflict_of(r, path) {
        let decision = r.decision_of(c);
        for d in Decision::ALL.into_iter().filter(|d| d.allowed(c)) {
            e.push(pair(
                t.t(words::decide_key(d)),
                if decision == Some(d) {
                    t.t(key::BELONGINGS_CHOSEN)
                } else {
                    String::new()
                },
                Target::Decide(path.to_owned(), d),
                true,
            ));
        }
    }
    let language = language_of(path, lines);
    let drawn: Vec<(Vec<Field>, Target)> = lines
        .iter()
        .enumerate()
        .map(|(n, line)| {
            (
                diff_fields(line, language, b.left, b.room),
                Target::DiffLine(path.to_owned(), n),
            )
        })
        .collect();
    e.push(Entry::Lines(drawn));
    b.section(
        &t.t_with(
            key::BELONGINGS_DIFF_TITLE,
            &[("name", &shown_path(path, places, false))],
        ),
        e,
    );
}

/// The file's own language, for the unchanged lines: by its extension as a
/// fence would name it, else by what its text looks like.
fn language_of(path: &str, lines: &[String]) -> Option<ronoctua_syntax::Language> {
    let extension = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    extension
        .as_deref()
        .and_then(ronoctua_syntax::of_tag)
        .or_else(|| {
            let body: String = lines
                .iter()
                .filter(|l| l.starts_with(' '))
                .map(|l| &l[1..])
                .collect::<Vec<_>>()
                .join("\n");
            ronoctua_syntax::guess(&body)
        })
}

/// One line of a difference as fields: a line that adds or removes, and the
/// headings, in the diff's own roles, whole; an unchanged line in the file's
/// language. Never past `room`.
fn diff_fields(
    line: &str,
    language: Option<ronoctua_syntax::Language>,
    left: u16,
    room: u16,
) -> Vec<Field> {
    let text = layout::elide_end(line, room);
    let changed = line.starts_with('+') || line.starts_with('-') || line.starts_with('@');
    // THE MARK OF AN UNCHANGED LINE is the diff's, dim; what follows it is the
    // file's own text.
    let (grammar, body_from) = if changed {
        (ronoctua_syntax::of_tag("diff"), 0)
    } else if text.starts_with(' ') {
        (language, 1)
    } else {
        (language, 0)
    };
    let ranges = grammar.map_or_else(Vec::new, |g| {
        ronoctua_syntax::line(
            &text[body_from..],
            g.grammar(),
            ronoctua_syntax::State::fresh(),
        )
        .0
    });
    let mut out = Vec::new();
    let mut at = left;
    let mut push = |piece_text: &str, color: tokens::Color, out: &mut Vec<Field>| {
        if piece_text.is_empty() {
            return;
        }
        out.push(Field {
            at,
            text: piece_text.to_owned(),
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        });
        at += layout::width(piece_text);
    };
    let plain = tokens::color::TEXT_SECONDARY;
    push(&text[..body_from], tokens::color::TEXT_DIM, &mut out);
    let body = &text[body_from..];
    let mut from = 0;
    for range in ranges {
        if range.start > from {
            push(&body[from..range.start], plain, &mut out);
        }
        push(
            &body[range.start..range.end],
            crate::chrome::syntax_color(range.role),
            &mut out,
        );
        from = range.end;
    }
    if from < body.len() {
        push(&body[from..], plain, &mut out);
    }
    out
}

/// What goes to your repository: ONE listing, generated from the walk a
/// sync makes — grouped by where each file comes from (the rules, the
/// targets, what the person added here and elsewhere, the plain part), one
/// row per file with its state and size, then what is never kept and why.
///
/// THE FILES' ROWS ARE THE COUNT'S, formatted once per count and language
/// ([`Counted::formatted`]); a folder is told by what the count found, never
/// by a `stat` here: this runs on every frame and every key.
fn listing_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let places = r.places.as_ref();
    let scope = r.scope();
    let reason = |id: &str| t.t(&words::reason_key(id));
    let mut e = vec![note(t.t(key::BELONGINGS_WHAT_LEAKS))];
    let nothing = Counted::default();
    let count = r.count.as_deref().unwrap_or(&nothing);
    if r.count.is_none() {
        e.push(note(t.t(key::BELONGINGS_COUNTING)));
    }
    let rows = count.formatted(places, t);
    let files = |e: &mut Vec<Entry>, origin: &Origin| {
        e.extend(count.of_origin(origin).iter().map(|n| rows[*n].entry()));
    };

    // THE RULES OF THE TABLE: each tree and file, with its reason — or
    // taken out by the person — and its files.
    for (place, folder) in scope::rules() {
        let label = shown_path(place.path, places, folder);
        let taken = r.prefs.excluded.iter().any(|x| x == place.path);
        e.push(if taken {
            pair(
                label,
                t.t(key::BELONGINGS_TAKEN_OUT),
                Target::Excluded(place.path.to_owned()),
                true,
            )
        } else {
            pair(
                label,
                reason(place.reason),
                Target::Rule(place.path.to_owned()),
                true,
            )
        });
        files(&mut e, &Origin::Rule(place.path));
    }

    // THE PREDEFINED TARGETS, each with where it is kept and what may be
    // done about it — and what the passphrase guards now.
    e.push(Entry::Air);
    e.push(note(t.t(key::BELONGINGS_TARGETS_NOTE)));
    for target in scope::targets() {
        let id = target.id;
        let asked = scope.asked(id);
        // A TARGET IS "NOT ON THIS MACHINE" ONLY WHEN EVERY PATH OF IT IS
        // taken out here; one path taken out (`~/.gitconfig`, by its file's
        // row) leaves the others travelling, and the target says so.
        let out = target
            .paths
            .iter()
            .filter(|p| kept::excluded_here(p, &r.prefs))
            .count();
        let (all_out, some_out) = (out == target.paths.len(), out > 0);
        let dropped = scope.dropped(id);
        let state = match asked {
            Some(Request::DropEverywhere { .. }) => key::BELONGINGS_TARGET_ASKED_NOWHERE,
            Some(Request::KeepEverywhere { .. }) => key::BELONGINGS_TARGET_ASKED_EVERYWHERE,
            _ if dropped => key::BELONGINGS_HERE_NOWHERE,
            _ if all_out => key::BELONGINGS_HERE_NOT_HERE,
            _ if some_out => key::BELONGINGS_TARGET_PARTLY_HERE,
            _ => key::BELONGINGS_TARGET_EVERYWHERE,
        };
        e.push(pair(
            t.t(&words::target_key(id)),
            t.t(state),
            Target::Fact(format!("target:{id}")),
            false,
        ));
        // A TARGET'S PATH ENDS IN `/` only when it is a folder HERE: the
        // table does not say, and `~/.gitconfig` has no extension.
        let paths: Vec<String> = target
            .paths
            .iter()
            .map(|p| shown_path(p, places, count.folder(p) == Some(true)))
            .collect();
        e.push(note(paths.join(", ")));
        let act = |label: &str, target: Target| indented(t.t(label), String::new(), target);
        if dropped {
            e.push(act(
                key::BELONGINGS_TARGET_ACT_EVERYWHERE,
                Target::Everywhere(id),
            ));
        } else {
            // SOME PATHS OUT: both — every path back, or every path out.
            if some_out {
                e.push(act(
                    key::BELONGINGS_TARGET_ACT_HERE_AGAIN,
                    Target::HereAgain(id),
                ));
            }
            if !all_out {
                e.push(act(
                    key::BELONGINGS_TARGET_ACT_NOT_HERE,
                    Target::NotHere(id),
                ));
            }
            e.push(act(key::BELONGINGS_TARGET_ACT_NOWHERE, Target::Nowhere(id)));
        }
        files(&mut e, &Origin::Target(id));
    }

    // WHAT THE PERSON ADDED HERE, and what another machine added and this
    // one follows: each taken out by its own row, for every machine.
    for extra in &r.prefs.extra {
        e.push(Entry::Air);
        e.push(pair(
            shown_path(extra, places, false),
            t.t(key::BELONGINGS_ADDED_BY_YOU),
            Target::Kept(extra.clone()),
            true,
        ));
        files(&mut e, &Origin::Added(extra.clone()));
    }
    for root in r
        .followed
        .iter()
        .filter(|f| !r.prefs.extra.contains(f) && !r.prefs.excluded.contains(f))
    {
        e.push(Entry::Air);
        e.push(pair(
            shown_path(root, places, false),
            t.t(key::BELONGINGS_ADDED_ELSEWHERE),
            Target::Kept(root.clone()),
            true,
        ));
        files(&mut e, &Origin::AddedElsewhere(root.clone()));
    }
    // WHAT THE PERSON TOOK OUT that no rule, target or file row says.
    let named = |x: &str| {
        scope::rules().any(|(p, _)| p.path == x)
            || scope::targets().iter().any(|t| t.paths.contains(&x))
            || count.lists(x)
    };
    for excluded in r.prefs.excluded.iter().filter(|x| !named(x)) {
        e.push(pair(
            shown_path(excluded, places, false),
            t.t(key::BELONGINGS_TAKEN_OUT),
            Target::Excluded(excluded.clone()),
            true,
        ));
    }
    e.push(Entry::Air);
    e.push(pair(
        t.t(key::BELONGINGS_ADD),
        String::new(),
        Target::Add,
        true,
    ));
    e.push(note(t.t_with(
        key::BELONGINGS_LARGE_NOTE,
        &[("size", &bytes_text(kept::LARGE_BYTES, t))],
    )));

    // THE PLAIN PART, kept IN CLEAR — and the page says who reads it.
    e.push(Entry::Air);
    e.push(note(t.t_with(
        key::BELONGINGS_PLAIN_NOTE,
        &[("size", &bytes_text(kept::PLAIN_MOST_BYTES, t))],
    )));
    for root in scope::plain_roots() {
        e.push(pair(
            shown_path(root.path, places, true),
            reason(root.reason),
            Target::Fact(format!("plain:{}", root.path)),
            false,
        ));
        files(&mut e, &Origin::Plain(root.path));
    }
    // WHAT ITS LAST RUN LEFT OUT FOR ITS SIZE on the repository's side —
    // this machine's are rows above.
    for path in r.plain.too_large.iter().filter(|p| !count.lists(p)) {
        e.push(tinted(
            shown_path(path, places, false),
            t.t(words::refusal_key(Why::PlainTooLarge)),
            Target::Fact(format!("plain-too-large:{path}")),
            false,
            tokens::color::STATE_WARNING,
        ));
    }
    // AND WHAT WAITS FOR THE PERSON: a run with nobody to ask left it.
    if !r.plain.conflicts.is_empty() {
        e.push(note(t.t_args(
            key::BELONGINGS_PLAIN_CONFLICTS,
            &[("n", self::count(r.plain.conflicts.len()))],
        )));
    }
    // WHAT THE REPOSITORY CHANGED AND THIS MACHINE HELD, each with why: the
    // person's signature did not vouch for it.
    for (path, why) in &r.plain.held {
        e.push(tinted(
            shown_path(path, places, false),
            t.t(words::held_key(*why)),
            Target::Fact(format!("plain-held:{path}")),
            false,
            tokens::color::STATE_WARNING,
        ));
    }
    // WHAT THIS MACHINE CHANGED AND NOTHING SIGNED yet: it waits, and "sign
    // now" signs it here.
    for path in &r.plain.unsigned {
        e.push(tinted(
            shown_path(path, places, false),
            t.t(key::BELONGINGS_UNSIGNED),
            Target::Fact(format!("plain-unsigned:{path}")),
            false,
            tokens::color::STATE_WARNING,
        ));
    }
    if !r.plain.unsigned.is_empty() {
        e.push(indented(
            t.t(key::BELONGINGS_SIGN),
            String::new(),
            Target::Sign,
        ));
    }

    // NEVER KEPT: the rules that take things out, with their reasons. A
    // hard one is a fact; the rest a person may add anyway.
    e.push(Entry::Air);
    e.push(note(t.t(key::BELONGINGS_NEVER)));
    for never in scope::never_rules() {
        let (label, target) = match never.pattern {
            Pattern::Name(name) => (name.to_owned(), Target::Fact(format!("never:{name}"))),
            Pattern::Path(path) => {
                let added = r.prefs.extra.iter().any(|x| covers(x, path));
                if added {
                    continue;
                }
                let target = if never.hard {
                    Target::Fact(format!("never:{path}"))
                } else {
                    Target::Never(path.to_owned())
                };
                let folder = count
                    .folder(path)
                    .unwrap_or_else(|| looks_like_a_folder(path));
                (shown_path(path, places, folder), target)
            }
        };
        let acts = matches!(target, Target::Never(_));
        e.push(pair(label, reason(never.reason), target, acts));
    }
    b.section(&t.t(key::BELONGINGS_LISTING), e);
}

/// One file of the listing as a row, formatted once per count and
/// language ([`Counted::formatted`]): its path below its group's root, and
/// what it is — its state and size when it travels, else why not — and what
/// `RET` does to it.
#[derive(Debug, Clone)]
struct Formatted {
    label: String,
    value: String,
    target: Target,
    acts: bool,
    tint: Option<tokens::Color>,
}

impl Formatted {
    fn of(item: &Item, places: Option<&Places>, t: &ronoctua_i18n::Texts) -> Self {
        let root = group_root(item);
        let label = item
            .path
            .strip_prefix(root)
            .map(|rest| rest.trim_start_matches('/'))
            .filter(|rest| !rest.is_empty())
            .map_or_else(|| shown_path(&item.path, places, false), str::to_owned);
        let value = match item.here {
            Here::Kept => t.t_with(
                key::BELONGINGS_ITEM,
                &[
                    ("state", &t.t(words::state_key(item.state))),
                    ("size", &bytes_text(item.size_bytes, t)),
                ],
            ),
            Here::NotHere => t.t(key::BELONGINGS_HERE_NOT_HERE),
            Here::Nowhere => t.t(key::BELONGINGS_HERE_NOWHERE),
            Here::Never(why) => t.t_with(
                key::BELONGINGS_NEVER_BECAUSE,
                &[("reason", &t.t(&words::reason_key(why)))],
            ),
        };
        // WHAT `RET` DOES: a file kept stops being kept (one deleted here
        // then stays in the repository for the others), one taken out comes
        // back, one never kept by a rule a person may go over is kept anyway;
        // the plain part and the rest are facts.
        let act = match (item.part, item.here) {
            (Part::Plain, _) | (_, Here::Nowhere) => None,
            (_, Here::Kept) => Some(FileAct::StopKeeping),
            (_, Here::NotHere) => Some(FileAct::KeepAgain),
            (_, Here::Never(_)) => match kept::by_default(&item.path) {
                kept::Verdict::Never(n) if !n.hard && matches!(n.pattern, Pattern::Path(_)) => {
                    Some(FileAct::KeepAnyway)
                }
                _ => None,
            },
        };
        let target = act.map_or_else(
            || Target::Fact(format!("file:{}", item.path)),
            |a| Target::File(item.path.clone(), a),
        );
        Self {
            label,
            value,
            target,
            acts: act.is_some(),
            tint: (item.here.travels() && item.state.troubles())
                .then_some(tokens::color::STATE_WARNING),
        }
    }

    /// The row, in under its group.
    fn entry(&self) -> Entry {
        Entry::Pair {
            label: self.label.clone(),
            value: self.value.clone(),
            kind: crate::chrome::Kind::of(self.acts),
            target: self.target.clone(),
            tint: self.tint,
            indent: tokens::space::VIEW_NESTCELLS,
        }
    }
}

/// The root of the group a file is drawn under — its row's path is said
/// below it: the rule, the target's path that covers it, the addition, the
/// plain root.
fn group_root(item: &Item) -> &str {
    match &item.origin {
        Origin::Rule(root) | Origin::Plain(root) => root,
        Origin::Added(root) | Origin::AddedElsewhere(root) => root,
        Origin::Target(id) => target_paths(id)
            .iter()
            .copied()
            .find(|p| covers(p, &item.path))
            .unwrap_or(&item.path),
    }
}

/// The paths of the target `id`.
fn target_paths(id: &str) -> &'static [&'static str] {
    kept::target_by_id(id).map_or(&[], |t| t.paths)
}

/// Asks `request` of the next sync (`requests.json`, `0600`): one answer per
/// target, the later one in place of the earlier.
///
/// # Errors
///
/// The key of the sentence that says why not.
fn ask_of_the_next_sync(r: &Readings, request: Request) -> Result<(), &'static str> {
    let places = r.places.as_ref().ok_or(key::BELONGINGS_NO_PLACES)?;
    let mut now = Requests::read(&places.requests());
    if !now.ask(request) {
        return Err(key::BELONGINGS_NOT_WRITTEN);
    }
    ronoctua_belongings::secret::make_private_dir(&places.own())
        .and_then(|()| now.write(&places.requests()))
        .map_err(|e| {
            tracing::warn!(error = %e, "a request of the next sync could not be written");
            key::BELONGINGS_NOT_WRITTEN
        })
}

/// One field of a JSON conflict decided: this machine's value or the
/// repository's — written beside the paths' decisions, which leave that
/// path to its fields (a path's decision would go over them).
fn decide_field(
    r: &Readings,
    path: &str,
    pointer: &str,
    d: Decision,
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    if !matches!(d, Decision::KeepLocal | Decision::TakeRemote) {
        return Errand::Say(texts.t(key::BELONGINGS_FIELD_ONLY_TWO));
    }
    let Some(places) = r.places.as_ref() else {
        return Errand::Say(texts.t(key::BELONGINGS_NO_PLACES));
    };
    if conflict_of(r, path).is_none() {
        return Errand::Say(texts.t(key::BELONGINGS_NO_CONFLICT_HERE));
    }
    let mut file = DecisionsFile::read(&places.decisions()).unwrap_or_default();
    file.paths.remove(path);
    file.fields
        .entry(path.to_owned())
        .or_default()
        .insert(pointer.to_owned(), d);
    let written = serde_json::to_vec_pretty(&file)
        .map_err(|_| key::BELONGINGS_NOT_WRITTEN)
        .and_then(|text| {
            ronoctua_belongings::secret::write_private(&places.decisions(), &text).map_err(|e| {
                tracing::warn!(error = %e, "a field's decision could not be written");
                key::BELONGINGS_NOT_WRITTEN
            })
        });
    match written {
        Ok(()) => Errand::Nothing,
        Err(said) => Errand::Say(texts.t(said)),
    }
}

/// "Write the fix" (`write`) or "leave it" of the file at `path`'s repairs.
fn repair(r: &Readings, path: &str, write: bool, texts: &ronoctua_i18n::Texts) -> Errand {
    let Some(places) = r.places.as_ref() else {
        return Errand::Say(texts.t(key::BELONGINGS_NO_PLACES));
    };
    let name = shown_path(path, Some(places), false);
    let stale = || Errand::Say(texts.t_with(key::BELONGINGS_REPAIR_STALE, &[("file", &name)]));
    let (Some(ledger), Some(file)) = (
        r.repairs.iter().find(|l| l.path == path),
        places.resolve(path),
    ) else {
        return stale();
    };
    let dir = places.repairs();
    let done = if write {
        ronoctua_store::repair::apply(&dir, &file, ledger)
    } else {
        ronoctua_store::repair::dismiss(&dir, &file, ledger)
    };
    match done {
        Ok(()) if write => {
            Errand::Say(texts.t_with(key::BELONGINGS_REPAIR_WRITTEN, &[("file", &name)]))
        }
        Ok(()) => Errand::Nothing,
        Err(ronoctua_store::repair::Stale) => stale(),
    }
}

/// A question's page: what is asked, then "no" and "yes" — "no" first, the
/// cursor born on it, "yes" in the colour of what does not come back.
fn question_page(b: &mut Builder<'_>, q: Question) {
    let t = b.texts;
    let (title, said) = match q {
        Question::Nowhere(id) => {
            let target = t.t(&words::target_key(id));
            (
                target.clone(),
                t.t_with(key::BELONGINGS_NOWHERE_QUESTION, &[("target", &target)]),
            )
        }
        Question::Compact => (
            t.t(key::BELONGINGS_HISTORY),
            t.t_args(
                key::BELONGINGS_COMPACT_QUESTION,
                &[(
                    "days",
                    ronoctua_i18n::Arg::Number(i64::try_from(kept::KEEP_DAYS).unwrap_or(i64::MAX)),
                )],
            ),
        ),
    };
    b.section(&title, vec![note(said)]);
    for (yes, k) in [
        (false, key::BELONGINGS_ANSWER_NO),
        (true, key::BELONGINGS_ANSWER_YES),
    ] {
        let on = b.cursor == b.out.len();
        // THE CURSOR IS A COLOUR; "yes" is in the colour of danger.
        let color = if on {
            tokens::color::TEXT_HERE_ACTS
        } else if yes {
            tokens::color::STATE_ERROR
        } else {
            crate::chrome::Kind::Setting.tones(false).0
        };
        b.out.push(Row {
            fields: vec![piece(b.left, t.t(k), color, b.room)],
            target: Some(Target::Answer(yes)),
        });
    }
}

/// What the last sync offered to turn on — each a question that came by
/// itself, so "no" first.
fn offers_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let offers = r.offers();
    if offers.is_empty() {
        return;
    }
    let mut e = Vec::new();
    for offer in offers {
        let said = match offer {
            Offer::SshSigning { key: public } => t.t_with(
                key::BELONGINGS_OFFER_SSH_SIGNING,
                &[("key", &shown_path(public, r.places.as_ref(), false))],
            ),
            Offer::PamU2f => t.t(key::BELONGINGS_OFFER_PAM_U2F),
        };
        match r.answer_of(offer) {
            None => {
                if !e.is_empty() {
                    e.push(Entry::Air);
                }
                e.push(note(said));
                e.push(pair(
                    t.t(key::BELONGINGS_ANSWER_NO),
                    String::new(),
                    Target::Offer(offer.clone(), false),
                    true,
                ));
                e.push(pair(
                    t.t(key::BELONGINGS_ANSWER_YES),
                    String::new(),
                    Target::Offer(offer.clone(), true),
                    true,
                ));
            }
            Some(Said::No) => {}
            Some(Said::Turning) => e.push(note(t.t(key::BELONGINGS_OFFER_TURNING))),
            Some(Said::Yes) => e.push(note(t.t(key::BELONGINGS_OFFER_TAKEN))),
            Some(Said::Refused { why }) => e.push(Entry::Note(
                t.t_with(
                    key::BELONGINGS_OFFER_REFUSED,
                    &[("why", &t.t(words::refusal_key(*why)))],
                ),
                tokens::color::STATE_WARNING,
            )),
            Some(Said::RootRefused { why }) => e.push(Entry::Note(
                t.t_with(
                    key::BELONGINGS_OFFER_REFUSED,
                    &[("why", &t.t(crate::security_words::refusal(*why)))],
                ),
                tokens::color::STATE_WARNING,
            )),
        }
    }
    if !e.is_empty() {
        b.section(&t.t(key::BELONGINGS_OFFERS), e);
    }
}

/// The files a lenient read repaired: each with what it could not read and
/// why — never the value itself, which may be a secret — then "leave it"
/// first and "write the fix".
fn repairs_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    if r.repairs.is_empty() {
        return;
    }
    let mut e = vec![
        note(t.t(key::BELONGINGS_REPAIRS_NOTE)),
        note(t.t(if r.newest {
            key::BELONGINGS_REPAIRS_NEWEST
        } else {
            key::BELONGINGS_REPAIRS_OLDER
        })),
    ];
    for ledger in &r.repairs {
        e.push(Entry::Air);
        e.push(pair(
            shown_path(&ledger.path, r.places.as_ref(), false),
            t.t_args(
                key::BELONGINGS_REPAIR_COUNT,
                &[("n", count(ledger.repairs.len()))],
            ),
            Target::Fact(format!("repair:{}", ledger.path)),
            false,
        ));
        for repair in &ledger.repairs {
            // THE EMPTY POINTER IS THE WHOLE DOCUMENT (RFC 6901): `/` would
            // name a member called "" at its root.
            let pointer = if repair.pointer.is_empty() {
                t.t(key::BELONGINGS_REPAIR_WHOLE_FILE)
            } else {
                repair.pointer.clone()
            };
            e.push(note(t.t_with(
                key::BELONGINGS_REPAIR_LINE,
                &[
                    ("pointer", &pointer),
                    (
                        "why",
                        &t.t(&format!("belongings-repair-why-{}", repair.why.id())),
                    ),
                ],
            )));
        }
        e.push(indented(
            t.t(key::BELONGINGS_REPAIR_LEAVE),
            String::new(),
            Target::LeaveIt(ledger.path.clone()),
        ));
        e.push(indented(
            t.t(key::BELONGINGS_REPAIR_WRITE),
            String::new(),
            Target::WriteFix(ledger.path.clone()),
        ));
    }
    b.section(&t.t(key::BELONGINGS_REPAIRS), e);
}

/// The history: what the repository weighs, against the budget, and its
/// compaction — offered when a compaction would fold something, recommended
/// past the line, never done by itself.
fn history_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let Some(budget) = r.budget() else {
        return;
    };
    let mut e = vec![pair(
        t.t(key::BELONGINGS_WEIGHS),
        t.t_with(
            key::BELONGINGS_WEIGHS_VALUE,
            &[
                ("tree", &bytes_text(budget.tree_bytes, t)),
                ("history", &bytes_text(budget.history_bytes, t)),
            ],
        ),
        Target::Fact("weighs".to_owned()),
        false,
    )];
    let history = bytes_text(budget.history_bytes, t);
    let recommend = bytes_text(budget.recommend_bytes, t);
    match kept::weight(budget.tree_bytes, budget.history_bytes, budget.foldable) {
        kept::Weight::Light => {}
        kept::Weight::Heavy => e.push(Entry::Note(
            t.t_with(
                key::BELONGINGS_WEIGHT_HEAVY,
                &[("history", &history), ("recommend", &recommend)],
            ),
            tokens::color::STATE_WARNING,
        )),
        kept::Weight::Compact => e.push(Entry::Note(
            t.t_with(key::BELONGINGS_WEIGHT_COMPACT, &[("history", &history)]),
            tokens::color::STATE_WARNING,
        )),
    }
    if budget.foldable {
        let days = ronoctua_i18n::Arg::Number(i64::try_from(kept::KEEP_DAYS).unwrap_or(i64::MAX));
        e.push(pair(
            t.t(key::BELONGINGS_COMPACT),
            t.t_args(key::BELONGINGS_COMPACT_KEEPS, &[("days", days)]),
            Target::Compact,
            true,
        ));
        e.push(note(t.t(key::BELONGINGS_COMPACT_NOTE)));
    }
    b.section(&t.t(key::BELONGINGS_HISTORY), e);
}

/// One value of a field of a JSON conflict in `room` cells, on one line:
/// the JSON elided to what is left beside "chosen" — or "none". The JSON is
/// data, abbreviated as a long path is; the room is the row's, never a
/// fixed length.
fn field_text(value: Option<&str>, chosen: bool, room: u16, t: &ronoctua_i18n::Texts) -> String {
    let raw = value.map_or_else(|| t.t(key::BELONGINGS_FIELD_NONE), str::to_owned);
    if !chosen {
        return layout::elide_end(&raw, room);
    }
    let frame = layout::width(&t.t_with(key::BELONGINGS_FIELD_CHOSEN, &[("value", "")]));
    let shown = layout::elide_end(&raw, room.saturating_sub(frame));
    t.t_with(key::BELONGINGS_FIELD_CHOSEN, &[("value", &shown)])
}

/// The value a field had before both sides changed it, as the note under
/// it says it — on one line of a note `room` cells wide: the JSON elided to
/// what the sentence leaves, so the sentence itself is never cut.
fn before_text(value: Option<&str>, room: u16, t: &ronoctua_i18n::Texts) -> String {
    let raw = value.map_or_else(|| t.t(key::BELONGINGS_FIELD_NONE), str::to_owned);
    let frame = layout::width(&t.t_with(key::BELONGINGS_FIELD_BEFORE, &[("value", "")]));
    let shown = layout::elide_end(&raw, room.saturating_sub(frame));
    t.t_with(key::BELONGINGS_FIELD_BEFORE, &[("value", &shown)])
}

/// Whether a logical path NOT on this machine reads as a folder: its last
/// name has no extension (`syncthing`, never `belongings.json`; a dot that
/// begins a name is not an extension). One that is here is what the count
/// found ([`Counted::folder`]).
fn looks_like_a_folder(logical: &str) -> bool {
    Root::split(logical).is_some_and(|(_, rest)| {
        rest.rsplit('/')
            .next()
            .is_some_and(|name| !name.trim_start_matches('.').contains('.'))
    })
}

/// Access: how the repository is reached — this machine's deploy key, or a
/// token.
/// A fingerprint as a screen shows it: its first 16 digits, in groups of
/// four — enough to tell two keys apart by eye.
fn short_fingerprint(fingerprint: &str) -> String {
    fingerprint
        .chars()
        .take(16)
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The logical path of a path under the staged root (`person/options.json`
/// → `state/person/options.json`) — what root names a file it kept back by.
fn logical_of_staged(staged: &str) -> String {
    ronoctua_belongings::plain::logical_of_repo(&format!(
        "{}/{staged}",
        ronoctua_belongings::plain::DIR
    ))
    .unwrap_or_else(|| staged.to_owned())
}

/// The person's signature: their key (as the last run that opened
/// `key.age` left it), the key this machine's root pinned, what root last
/// refused or kept back, a pin on its way — and "pin my key" when the
/// machine trusts none, or another.
fn signature_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let places = r.places.as_ref();
    let mut e = vec![note(t.t(key::BELONGINGS_SIGNATURE_NOTE))];
    let mine = &r.plain.fingerprint;
    e.push(pair(
        t.t(key::BELONGINGS_SIGNATURE_KEY),
        if mine.is_empty() {
            t.t(key::BELONGINGS_SIGNATURE_KEY_UNKNOWN)
        } else {
            short_fingerprint(mine)
        },
        Target::Fact("signer".to_owned()),
        false,
    ));
    let (state, tint) = match &r.pin {
        Some(pin) if !mine.is_empty() && pin.fingerprint == *mine => {
            (key::BELONGINGS_SIGNATURE_PINNED, None)
        }
        Some(_) if !mine.is_empty() => (
            key::BELONGINGS_SIGNATURE_OTHER,
            Some(tokens::color::STATE_WARNING),
        ),
        Some(_) => (key::BELONGINGS_SIGNATURE_PINNED_SOME, None),
        None => (
            key::BELONGINGS_SIGNATURE_NONE,
            Some(tokens::color::STATE_WARNING),
        ),
    };
    let machine = t.t(key::BELONGINGS_SIGNATURE_MACHINE);
    let target = Target::Fact("pin".to_owned());
    e.push(match tint {
        Some(tint) => tinted(machine, t.t(state), target, false, tint),
        None => pair(machine, t.t(state), target, false),
    });
    if let Some(pin) = &r.pin {
        // WHAT ROOT LAST REFUSED, for its signature: the machine kept the
        // data it had.
        if let Some(said) = pin
            .refused
            .as_ref()
            .and_then(|refused| words::root_refused_key(refused.why))
        {
            e.push(tinted(
                t.t(key::BELONGINGS_SIGNATURE_LAST),
                t.t(key::BELONGINGS_SIGNATURE_REFUSED),
                Target::Fact("pin-refused".to_owned()),
                false,
                tokens::color::STATE_ERROR,
            ));
            // THE WHOLE SENTENCE, wrapped: a value would be cut.
            e.push(note(t.t(said)));
        }
        // AND WHAT IT KEPT BACK, file by file.
        for back in &pin.kept_back {
            let Some(said) = words::kept_back_key(back.why) else {
                continue;
            };
            e.push(tinted(
                shown_path(&logical_of_staged(&back.at), places, false),
                t.t(said),
                Target::Fact(format!("kept-back:{}", back.at)),
                false,
                tokens::color::STATE_WARNING,
            ));
        }
    }
    // A PIN ASKED FOR, and how it went.
    if let Some(pinning) = r.pinning.as_ref().filter(|p| p.fingerprint == *mine) {
        let (said, tint) = match &pinning.said {
            PinSaid::Pinning => (t.t(key::BELONGINGS_PINNING_NOW), None),
            PinSaid::Pinned => (t.t(key::BELONGINGS_PINNING_DONE), None),
            PinSaid::NotPinned => (
                t.t(key::BELONGINGS_PINNING_NOT),
                Some(tokens::color::STATE_ERROR),
            ),
            PinSaid::Refused { why } => (
                t.t(crate::security_words::refusal(*why)),
                Some(tokens::color::STATE_ERROR),
            ),
        };
        let label = t.t(key::BELONGINGS_PINNING);
        let target = Target::Fact("pinning".to_owned());
        e.push(match tint {
            Some(tint) => tinted(label, said, target, false, tint),
            None => pair(label, said, target, false),
        });
    }
    let pinned_mine = r.pin.as_ref().is_some_and(|p| p.fingerprint == *mine);
    if !mine.is_empty() && !pinned_mine {
        e.push(pair(
            t.t(key::BELONGINGS_PIN),
            String::new(),
            Target::Pin,
            true,
        ));
    }
    b.section(&t.t(key::BELONGINGS_SIGNATURE), e);
}

fn access_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let mut e = vec![pair(
        t.t(key::BELONGINGS_AUTH),
        t.t(words::auth_key(r.prefs.auth)),
        Target::Auth,
        true,
    )];
    match r.prefs.auth {
        Auth::Ssh => {
            if r.has_key {
                e.push(pair(
                    t.t(key::BELONGINGS_DEPLOY_KEY),
                    t.t(key::BELONGINGS_COPY_IT),
                    Target::CopyKey,
                    true,
                ));
                if let Some(public) = r.public_key.as_ref() {
                    e.push(Entry::Note(public.clone(), tokens::color::TEXT_SECONDARY));
                }
                e.push(note(t.t(key::BELONGINGS_DEPLOY_NOTE)));
            } else {
                e.push(pair(
                    t.t(key::BELONGINGS_DEPLOY_KEY),
                    t.t(key::BELONGINGS_MAKE_IT),
                    Target::Keygen,
                    true,
                ));
                e.push(note(t.t(key::BELONGINGS_KEYGEN_NOTE)));
            }
        }
        Auth::Token => {
            e.push(pair(
                t.t(key::BELONGINGS_TOKEN),
                t.t(if r.has_token {
                    key::BELONGINGS_TOKEN_IS_KEPT
                } else {
                    key::BELONGINGS_NONE_YET
                }),
                Target::Token,
                true,
            ));
            e.push(pair(
                t.t(key::BELONGINGS_USER),
                r.prefs
                    .user
                    .clone()
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| t.t(key::BELONGINGS_NONE_YET)),
                Target::User,
                true,
            ));
            e.push(note(t.t(key::BELONGINGS_TOKEN_NOTE)));
        }
    }
    b.section(&t.t(key::BELONGINGS_ACCESS), e);
}

/// The passphrase: change it — what it guards now — and whether the
/// session remembers it.
fn passphrase_section(b: &mut Builder<'_>, r: &Readings, zone: &ronoctua_files::when::Zone) {
    let t = b.texts;
    let mut e = vec![
        pair(
            t.t(key::BELONGINGS_CHANGE_PASSPHRASE),
            String::new(),
            Target::Passphrase,
            true,
        ),
        note(t.t(key::BELONGINGS_PASSPHRASE_NOTE)),
        note(t.t(key::BELONGINGS_TARGETS_NOTE)),
        pair(
            t.t(key::BELONGINGS_UNLOCK_SWITCH),
            t.t(if r.prefs.keep_unlocked {
                key::BELONGINGS_UNLOCK_ON
            } else {
                key::BELONGINGS_UNLOCK_OFF
            }),
            Target::Unlock,
            true,
        ),
        note(t.t_args(
            key::BELONGINGS_UNLOCK_NOTE,
            &[(
                "hours",
                ronoctua_i18n::Arg::Number(i64::try_from(kept::UNLOCK_HOURS).unwrap_or(i64::MAX)),
            )],
        )),
    ];
    if let Some(until) = r.kept_until {
        e.push(note(t.t_with(
            key::BELONGINGS_UNLOCK_KEPT,
            &[("until", &when(until, zone, t))],
        )));
    }
    b.section(&t.t(key::BELONGINGS_PASSPHRASE), e);
}

/// How often the brain reminds.
fn period_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let e = vec![pair(
        t.t(key::BELONGINGS_REMIND_EVERY),
        words::period_text(r.prefs.period_days, t),
        Target::Period,
        true,
    )];
    b.section(&t.t(key::BELONGINGS_PERIOD), e);
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "belongings",
    commands: &[
        // THE BELONGINGS, `LEADER s v` (`crate::belongings`).
        ronoctua_cmd::Command {
            id: "belongings-open",
            label: crate::key::CMD_BELONGINGS_OPEN,
        },
        ronoctua_cmd::Command {
            id: "belongings-window",
            label: crate::key::CMD_BELONGINGS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "belongings-sync",
            label: crate::key::CMD_BELONGINGS_SYNC,
        },
        ronoctua_cmd::Command {
            id: "belongings-up",
            label: crate::key::CMD_BELONGINGS_UP,
        },
        ronoctua_cmd::Command {
            id: "belongings-down",
            label: crate::key::CMD_BELONGINGS_DOWN,
        },
        ronoctua_cmd::Command {
            id: "belongings-less",
            label: crate::key::CMD_BELONGINGS_LESS,
        },
        ronoctua_cmd::Command {
            id: "belongings-more",
            label: crate::key::CMD_BELONGINGS_MORE,
        },
        ronoctua_cmd::Command {
            id: "belongings-enter",
            label: crate::key::CMD_BELONGINGS_ENTER,
        },
        ronoctua_cmd::Command {
            id: "belongings-keep-local",
            label: crate::key::CMD_BELONGINGS_KEEP_LOCAL,
        },
        ronoctua_cmd::Command {
            id: "belongings-take-remote",
            label: crate::key::CMD_BELONGINGS_TAKE_REMOTE,
        },
        ronoctua_cmd::Command {
            id: "belongings-keep-both",
            label: crate::key::CMD_BELONGINGS_KEEP_BOTH,
        },
        ronoctua_cmd::Command {
            id: "belongings-apply",
            label: crate::key::CMD_BELONGINGS_APPLY,
        },
        ronoctua_cmd::Command {
            id: "belongings-abandon",
            label: crate::key::CMD_BELONGINGS_ABANDON,
        },
        ronoctua_cmd::Command {
            id: "belongings-all-local",
            label: crate::key::CMD_BELONGINGS_ALL_LOCAL,
        },
        ronoctua_cmd::Command {
            id: "belongings-all-remote",
            label: crate::key::CMD_BELONGINGS_ALL_REMOTE,
        },
        ronoctua_cmd::Command {
            id: "belongings-repository",
            label: crate::key::CMD_BELONGINGS_REPOSITORY,
        },
        ronoctua_cmd::Command {
            id: "belongings-add",
            label: crate::key::CMD_BELONGINGS_ADD,
        },
        ronoctua_cmd::Command {
            id: "belongings-auth",
            label: crate::key::CMD_BELONGINGS_AUTH,
        },
        ronoctua_cmd::Command {
            id: "belongings-keygen",
            label: crate::key::CMD_BELONGINGS_KEYGEN,
        },
        ronoctua_cmd::Command {
            id: "belongings-copy-key",
            label: crate::key::CMD_BELONGINGS_COPY_KEY,
        },
        ronoctua_cmd::Command {
            id: "belongings-token",
            label: crate::key::CMD_BELONGINGS_TOKEN,
        },
        ronoctua_cmd::Command {
            id: "belongings-user",
            label: crate::key::CMD_BELONGINGS_USER,
        },
        ronoctua_cmd::Command {
            id: "belongings-passphrase",
            label: crate::key::CMD_BELONGINGS_PASSPHRASE,
        },
        ronoctua_cmd::Command {
            id: "belongings-period",
            label: crate::key::CMD_BELONGINGS_PERIOD,
        },
        ronoctua_cmd::Command {
            id: "belongings-pin",
            label: crate::key::CMD_BELONGINGS_PIN,
        },
        ronoctua_cmd::Command {
            id: "belongings-sign",
            label: crate::key::CMD_BELONGINGS_SIGN,
        },
        ronoctua_cmd::Command {
            id: "belongings-take-merged-ours",
            label: crate::key::CMD_BELONGINGS_TAKE_MERGED_OURS,
        },
        ronoctua_cmd::Command {
            id: "belongings-take-merged-theirs",
            label: crate::key::CMD_BELONGINGS_TAKE_MERGED_THEIRS,
        },
    ],
    bindings: &[
        // THE BELONGINGS: the view in the footer, in a window, and a sync
        // from anywhere — the passphrase asked on the door's own line.
        ("s v v", "belongings-open", None),
        ("s v w", "belongings-window", None),
        ("s v s", "belongings-sync", None),
    ],
    groups: &[("s v", crate::key::GROUP_BELONGINGS)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::belongings::reached().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keep::{NotChanged, keep_under};
    use ronoctua_belongings::plan::Kind as PathKind;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn zone() -> ronoctua_files::when::Zone {
        ronoctua_files::when::Zone::utc()
    }

    /// A scratch home, empty.
    fn scratch(name: &str) -> Places {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-shell-belongings-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let places = Places::under(&root);
        std::fs::create_dir_all(&places.config).expect("config");
        std::fs::create_dir_all(&places.home).expect("home");
        places
    }

    fn side(size: u64) -> Side {
        Side {
            size_bytes: size,
            modified_s: 1_700_000_000,
            sha256: "x".into(),
            kind: PathKind::File,
            mode: 0o644,
        }
    }

    /// A machine with a repository, a key, and a sync waiting on two
    /// conflicts — one of them a path the person added.
    fn waiting(places: &Places) -> Readings {
        let prefs = Belongings {
            repository: "git@example.org:someone/belongings.git".into(),
            extra: vec!["home/notes".into()],
            ..Belongings::default()
        };
        store_of(places).write(&prefs).expect("prefs");
        // THE HOST LET, as typing the address does.
        ronoctua_consent::let_hosts_under(&places.config, &["example.org"], true).expect("consent");
        std::fs::create_dir_all(places.keys()).expect("keys");
        std::fs::write(places.deploy_key(), "k").expect("key");
        std::fs::write(
            places.deploy_public(),
            "ssh-ed25519 AAAA ronoctua-belongings\n",
        )
        .expect("public");
        let plan = Plan {
            remote_commit: Some("abc".into()),
            conflicts: vec![
                Conflict {
                    path: "config/theme.json".into(),
                    local: Some(side(40)),
                    remote: Some(side(42)),
                    machine: "laptop".into(),
                    text: true,
                    theirs: Some("0".into()),
                    ..Conflict::default()
                },
                Conflict {
                    path: "home/notes/todo.md".into(),
                    local: Some(side(10)),
                    remote: None,
                    machine: "laptop".into(),
                    added_by_person: true,
                    ..Conflict::default()
                },
            ],
            ..Plan::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.plan(),
            &serde_json::to_vec(&plan).expect("plan"),
        )
        .expect("plan written");
        ronoctua_belongings::secret::write_private(
            &places.theirs("0.diff"),
            b"@@ -1 +1 @@\n-{\"theme\":\"paper\"}\n+{\"theme\":\"owl\"}\n",
        )
        .expect("diff");
        Readings::read(Some(places.clone()), 1_700_000_500)
    }

    fn drawn(rows: &[Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn stand_on(v: &mut View, r: &Readings, target: &Target, t: &ronoctua_i18n::Texts) {
        let rows = v.rows(r, &zone(), 100, t);
        let n = rows
            .iter()
            .position(|row| row.target.as_ref() == Some(target))
            .unwrap_or_else(|| panic!("no row is {target:?}: {}", drawn(&rows)));
        v.clicked(n, &rows);
    }

    /// FAILS IF: a section of the design is missing — status, sync, what
    /// goes to the repository (never kept inside it), access, passphrase,
    /// period — a word draws as its raw key in a language, or the cursor is
    /// born on a title.
    #[test]
    fn every_section_is_drawn_in_every_language() {
        let places = scratch("sections");
        let r = Readings::read(Some(places.clone()), 1_700_000_000);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let v = View::fresh(&r, &zone(), 100, &t);
            let rows = v.rows(&r, &zone(), 100, &t);
            let text = drawn(&rows);
            for heading in [
                key::BELONGINGS_STATUS,
                key::BELONGINGS_SYNC_HEADING,
                key::BELONGINGS_LISTING,
                key::BELONGINGS_NEVER,
                key::BELONGINGS_ACCESS,
                key::BELONGINGS_PASSPHRASE,
                key::BELONGINGS_PERIOD,
            ] {
                assert!(
                    text.contains(&t.t(heading)),
                    "{language}: no {heading}: {text}"
                );
            }
            assert!(!text.contains('['), "{language}: a raw key: {text}");
            assert!(
                !rows[v.cursor()].heading(),
                "the cursor was born on a title"
            );
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a waiting sync does not list EVERY conflict with both sides
    /// and the machine, offers "keep both" on a path the person did not add,
    /// or a decision, one by one or in bulk, is not written where the CLI
    /// reads it.
    #[test]
    fn every_conflict_is_listed_and_every_decision_is_written() {
        let places = scratch("conflicts");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        let text = drawn(&rows);
        assert!(
            text.contains("theme.json") && text.contains("todo.md"),
            "{text}"
        );
        assert!(text.contains("laptop"), "the machine is not said: {text}");
        let both: Vec<&Target> = rows
            .iter()
            .filter_map(|r| r.target.as_ref())
            .filter(|t| matches!(t, Target::Decide(_, Decision::KeepBoth)))
            .collect();
        assert_eq!(
            both,
            [&Target::Decide(
                "home/notes/todo.md".into(),
                Decision::KeepBoth
            )],
            "keep both on a path the person did not add"
        );
        // ONE BY ONE, by the row and by the key.
        stand_on(
            &mut v,
            &r,
            &Target::Decide("config/theme.json".into(), Decision::TakeRemote),
            &t,
        );
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            r.decisions.get("config/theme.json"),
            Some(&Decision::TakeRemote)
        );
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("home/notes/todo.md".into()),
            &t,
        );
        v.press(Press::Decide(Decision::KeepBoth), &r, &zone(), 100, &t);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(r.decisions.len(), 2, "{:?}", r.decisions);
        // `C-b` ON A PATH THE PERSON DID NOT ADD is refused, with its words.
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("config/theme.json".into()),
            &t,
        );
        assert_eq!(
            v.press(Press::Decide(Decision::KeepBoth), &r, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_BOTH_ONLY_ADDED))
        );
        // IN BULK, over what was decided.
        stand_on(&mut v, &r, &Target::All(Decision::KeepLocal), &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert!(r.decisions.values().all(|d| *d == Decision::KeepLocal));
        let mode = std::os::unix::fs::PermissionsExt::mode(
            &std::fs::metadata(places.decisions()).unwrap().permissions(),
        );
        assert_eq!(mode & 0o777, 0o600, "the decisions were born readable");
        // AND APPLY SENDS THE ORDER, the decisions written first.
        stand_on(&mut v, &r, &Target::Apply, &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Queue(Op::Apply)
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: apply is sent with a conflict undecided, or the sentence
    /// does not say how many are left.
    #[test]
    fn apply_is_refused_while_a_conflict_is_undecided() {
        let places = scratch("undecided");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &Target::Apply, &t);
        match v.press(Press::Enter, &r, &zone(), 100, &t) {
            Errand::Say(said) => assert!(said.contains('2'), "{said}"),
            other => panic!("apply with nothing decided did {other:?}"),
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: `RET` on a conflicting text does not open its difference —
    /// the decisions first, the lines in the diff's colours — or `Esc` there
    /// closes the view instead of going back to the conflict.
    #[test]
    fn a_text_shows_its_difference_and_esc_goes_back_to_it() {
        let places = scratch("diff");
        let r = waiting(&places);
        let t = texts("pt-BR");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("config/theme.json".into()),
            &t,
        );
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let rows = v.rows(&r, &zone(), 100, &t);
        let text = drawn(&rows);
        assert!(text.contains("owl") && text.contains("paper"), "{text}");
        let added = rows
            .iter()
            .flat_map(|r| &r.fields)
            .find(|f| f.text.contains("owl"))
            .expect("the added line");
        assert_eq!(added.color, tokens::color::SYNTAX_ADDED);
        assert!(matches!(
            rows[v.cursor()].target,
            Some(Target::Decide(_, _))
        ));
        // A DECISION ON THIS PAGE is the conflict's.
        assert_eq!(
            v.press(Press::Decide(Decision::KeepLocal), &r, &zone(), 100, &t),
            Errand::Nothing
        );
        assert_eq!(
            v.press(Press::Escape, &r, &zone(), 100, &t),
            Errand::Nothing
        );
        let rows = v.rows(&r, &zone(), 100, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Some(Target::Conflict("config/theme.json".into()))
        );
        assert_eq!(v.press(Press::Escape, &r, &zone(), 100, &t), Errand::Close);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: on a LINE of a difference the decisions the hint teaches are
    /// refused — a taught key works where it is taught — or keeping both is
    /// taught on a conflict that may not keep both.
    #[test]
    fn a_line_of_a_difference_decides_its_conflict() {
        let places = scratch("diff-line");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("config/theme.json".into()),
            &t,
        );
        v.press(Press::Enter, &r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::DiffLine("config/theme.json".into(), 1),
            &t,
        );
        let taught: Vec<&str> = v
            .hint(&r, &zone(), 100, &t)
            .iter()
            .filter_map(|p| p.id)
            .collect();
        assert!(
            taught.contains(&"belongings-keep-local") && taught.contains(&"belongings-take-remote"),
            "{taught:?}"
        );
        assert!(
            !taught.contains(&"belongings-keep-both"),
            "keep both taught on the configuration: {taught:?}"
        );
        // AND THE CURSOR SHOWS on it: the accent a label takes.
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(
            rows[v.cursor()]
                .fields
                .iter()
                .all(|f| f.color == crate::chrome::Kind::Setting.tones(true).0),
            "the cursor on a line of the difference is not drawn"
        );
        assert_eq!(
            v.press(Press::Decide(Decision::TakeRemote), &r, &zone(), 100, &t),
            Errand::Nothing
        );
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            r.decisions.get("config/theme.json"),
            Some(&Decision::TakeRemote)
        );
        // `RET` ON IT copies the line, whole.
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Copy("-{\"theme\":\"paper\"}".to_owned())
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a new sync, or a change of passphrase, starts while a sync
    /// waits for decisions — its `prepare` would throw them away — or while a
    /// run holds the lock; or apply and drop go while one runs.
    #[test]
    fn nothing_new_starts_over_a_waiting_or_a_running_sync() {
        let places = scratch("in-the-way");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let waits = Errand::Say(t.t(key::BELONGINGS_PLAN_WAITS));
        assert_eq!(v.press(Press::Sync, &r, &zone(), 100, &t), waits);
        assert_eq!(v.command(SYNC_COMMAND, &r, &zone(), 100, &t), waits);
        let summary = Last {
            repository: r.prefs.repository.clone(),
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&summary).unwrap(),
        )
        .unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            v.command("belongings-passphrase", &r, &zone(), 100, &t),
            waits
        );
        // A RUN HOLDS THE LOCK: apply and drop wait for it too.
        let held = ronoctua_belongings::running::hold(&places).unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert!(r.busy);
        let busy = Errand::Say(t.t(words::refusal_key(Why::Busy)));
        let decided: Decisions = r.plan.as_ref().unwrap().all(Decision::KeepLocal);
        write_decisions_file(&r, &decided).unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(v.command("belongings-apply", &r, &zone(), 100, &t), busy);
        assert_eq!(v.command("belongings-abandon", &r, &zone(), 100, &t), busy);
        // AND WITH NOTHING WAITING, a sync is refused as busy.
        std::fs::remove_dir_all(places.session()).unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(v.press(Press::Sync, &r, &zone(), 100, &t), busy);
        drop(held);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert!(!r.busy);
        assert!(matches!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Ask(_)
        ));
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a status that says a sync waits is drawn as waiting once its
    /// session is gone (a reboot empties the runtime folder).
    #[test]
    fn a_waiting_sync_whose_session_is_gone_says_so() {
        let places = scratch("gone");
        let r = waiting(&places);
        let status = Status {
            at_s: 1_700_000_100,
            outcome: Outcome::Waiting { conflicts: 2 },
        };
        ronoctua_belongings::secret::write_private(
            &places.status(),
            &serde_json::to_vec(&status).unwrap(),
        )
        .unwrap();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = Readings::read(Some(places.clone()), r.now_s);
            // A NOTE WRAPS at a space: the rows joined by one space read whole.
            let text = drawn(&View::fresh(&r, &zone(), 100, &t).rows(&r, &zone(), 100, &t))
                .replace(" | ", " ");
            assert!(text.contains(&outcome_text(&status.outcome, &t)), "{text}");
        }
        std::fs::remove_dir_all(places.session()).unwrap();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = Readings::read(Some(places.clone()), r.now_s);
            let text = drawn(&View::fresh(&r, &zone(), 100, &t).rows(&r, &zone(), 100, &t))
                .replace(" | ", " ");
            assert!(
                !text.contains(&outcome_text(&status.outcome, &t)),
                "{language}: still waiting: {text}"
            );
            assert!(text.contains(&t.t(key::BELONGINGS_PREPARED_GONE)), "{text}");
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: what a door typed for an order the queue did not take stays
    /// in the runtime folder, or an apply's — the waiting sync's — goes.
    #[test]
    fn an_order_the_queue_did_not_take_leaves_no_secret() {
        let places = scratch("unsent");
        let write = |at: &Path| ronoctua_belongings::secret::write_private(at, b"x").unwrap();
        write(&places.passphrase());
        forget_unsent(Op::Apply, &places);
        assert!(
            places.passphrase().exists(),
            "the waiting sync's passphrase went"
        );
        forget_unsent(Op::Prepare, &places);
        assert!(!places.passphrase().exists());
        write(&places.passphrase());
        write(&places.new_passphrase());
        forget_unsent(Op::Rekey, &places);
        assert!(!places.passphrase().exists() && !places.new_passphrase().exists());
        forget_unsent(Op::Rekey, &places);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a `belongings.json` with a value the store would REPAIR —
    /// one the engine, which reads it strictly, refuses whole — is shown as
    /// settings a sync will use, or written over by a gesture here, instead
    /// of being refused with its sentence as one that does not parse is.
    #[test]
    fn a_settings_file_the_engine_refuses_is_refused_here_too() {
        let places = scratch("repaired-prefs");
        std::fs::create_dir_all(&places.config).unwrap();
        let file = places.config.join(prefs::FILE);
        let text = br#"{"repository": "git@example.org:a/b.git", "auth": "passkey"}"#;
        std::fs::write(&file, text).unwrap();
        assert!(
            serde_json::from_slice::<Belongings>(text).is_err(),
            "the engine would read this file"
        );
        let r = Readings::read(Some(places.clone()), 1);
        assert!(
            r.prefs_refused,
            "a file the engine refuses reads as settings"
        );
        let t = texts("en-GB");
        assert_eq!(
            unready(&r, &t),
            Some(t.t(key::BELONGINGS_PREFS_REFUSED)),
            "a sync was offered over it"
        );
        assert!(matches!(
            change_the_file(Some(&places), |p| p.extra.push("home/notes".to_owned())),
            Err(NotChanged::Unparsed)
        ));
        assert_eq!(std::fs::read(&file).unwrap(), text, "it was written over");
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a folder another machine added, which this one follows, is
    /// not shown with what is kept, or its row does not take it out — or
    /// keeping a path the table keeps turns it into "added by you".
    #[test]
    fn what_another_machine_added_is_shown_and_taken_out_here() {
        let places = scratch("followed");
        let prefs = Belongings::with_repository("git@example.org:a/b.git");
        store_of(&places).write(&prefs).unwrap();
        let last = Last {
            repository: prefs.repository.clone(),
            roots: ["home/notes".to_owned()].into_iter().collect(),
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&last).unwrap(),
        )
        .unwrap();
        let r = Readings::read(Some(places.clone()), 1);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let text = drawn(&View::fresh(&r, &zone(), 100, &t).rows(&r, &zone(), 100, &t));
            assert!(
                text.contains("~/notes") && text.contains(&t.t(key::BELONGINGS_ADDED_ELSEWHERE)),
                "{language}: {text}"
            );
        }
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &Target::Kept("home/notes".into()), &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        assert_eq!(store_of(&places).read().excluded, ["home/notes"]);
        // THE FILE MANAGER SEES IT KEPT, and its gesture is the same.
        let notes = places.home.join("notes");
        store_of(&places).write(&prefs).unwrap();
        assert_eq!(
            keep_under(&places, std::slice::from_ref(&notes)).key,
            key::BELONGINGS_STOPPED_KEEPING
        );
        // A PATH THE TABLE KEEPS, kept again, is never an addition.
        let theme = places.config.join("theme.json");
        assert_eq!(
            keep_under(&places, std::slice::from_ref(&theme)).key,
            key::BELONGINGS_STOPPED_KEEPING
        );
        assert_eq!(
            keep_under(&places, std::slice::from_ref(&theme)).key,
            key::BELONGINGS_NOW_KEPT
        );
        let now = store_of(&places).read();
        assert!(now.extra.is_empty(), "{now:?}");
        assert!(!now.excluded.contains(&"config/theme.json".to_owned()));
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a sync is started with no repository or no access instead of
    /// saying why, the passphrase is not asked hidden, or its answer is not
    /// written `0600` before the order goes to the queue.
    #[test]
    fn a_sync_asks_the_passphrase_hidden_and_writes_it_private() {
        let places = scratch("sync");
        let t = texts("en-GB");
        let r = Readings::read(Some(places.clone()), 1);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        assert!(matches!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Say(_)
        ));
        store_of(&places)
            .write(&Belongings::with_repository("git@example.org:a/b.git"))
            .unwrap();
        let r = Readings::read(Some(places.clone()), 1);
        assert_eq!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_NEEDS_KEY)),
            "a sync with no deploy key"
        );
        std::fs::create_dir_all(places.keys()).unwrap();
        std::fs::write(places.deploy_key(), "k").unwrap();
        // AN ADDRESS SET BEFORE ITS HOST WAS LET — a machine from before the
        // consent: the sync is refused with its sentence, and the row under
        // Sync asks the host.
        let r = Readings::read(Some(places.clone()), 1);
        assert_eq!(r.unlet.as_deref(), Some("example.org"));
        assert_eq!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Say(t.t(words::refusal_key(Why::NotLet))),
            "a sync to a host nobody let"
        );
        assert_eq!(v.press(Press::Allow, &r, &zone(), 100, &t), Errand::Nothing);
        assert!(ronoctua_consent::allows_under(
            &places.config,
            "example.org"
        ));
        let r = Readings::read(Some(places.clone()), 1);
        assert_eq!(r.unlet, None, "the row stays after the host was let");
        assert!(r.first_run());
        let Errand::Ask(line) = v.press(Press::Sync, &r, &zone(), 100, &t) else {
            panic!("the passphrase was not asked");
        };
        assert!(line.secret);
        assert_eq!(line.id, SYNC_COMMAND);
        // A FIRST SYNC ASKS IT TWICE — an empty repository keeps it for good —
        // and two that differ send nothing.
        let Errand::Ask(again) = v.answered(line.id, "correct horse", &r, &t) else {
            panic!("a first sync did not ask the passphrase again");
        };
        assert!(again.secret);
        assert_eq!(again.asks, Some(key::ASK_BELONGINGS_SYNC_AGAIN));
        assert_eq!(
            v.answered(again.id, "correct h0rse", &r, &t),
            Errand::Say(t.t(key::BELONGINGS_FIRST_PASSPHRASES_DIFFER))
        );
        assert!(!places.passphrase().exists(), "written though they differ");
        let Errand::Ask(line) = v.press(Press::Sync, &r, &zone(), 100, &t) else {
            panic!("the passphrase was not asked");
        };
        let Errand::Ask(again) = v.answered(line.id, "correct horse", &r, &t) else {
            panic!("a first sync did not ask the passphrase again");
        };
        assert_eq!(
            v.answered(again.id, "correct horse", &r, &t),
            Errand::Queue(Op::Prepare)
        );
        let meta = std::fs::metadata(places.passphrase()).unwrap();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_to_string(places.passphrase()).unwrap(),
            "correct horse"
        );
        // AN ANSWER UNDER ANOTHER ID answers nothing.
        assert_eq!(v.answered("belongings-token", "x", &r, &t), Errand::Nothing);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a change of passphrase does not ask the old one and the new
    /// one twice, goes on when the two new ones differ, or does not write
    /// both files private before the order.
    #[test]
    fn a_new_passphrase_is_asked_twice_and_both_are_written_private() {
        let places = scratch("rekey");
        let t = texts("en-GB");
        store_of(&places)
            .write(&Belongings::with_repository("git@example.org:a/b.git"))
            .unwrap();
        let summary = Last {
            repository: "git@example.org:a/b.git".into(),
            at_s: 5,
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&summary).unwrap(),
        )
        .unwrap();
        let r = Readings::read(Some(places.clone()), 10);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let Errand::Ask(first) = v.command("belongings-passphrase", &r, &zone(), 100, &t) else {
            panic!("the old passphrase was not asked");
        };
        assert!(first.secret && first.asks.is_none());
        let Errand::Ask(second) = v.answered(first.id, "old one", &r, &t) else {
            panic!("the new passphrase was not asked");
        };
        assert_eq!(second.asks, Some(key::ASK_BELONGINGS_PASSPHRASE_NEW));
        let Errand::Ask(third) = v.answered(second.id, "new one", &r, &t) else {
            panic!("the new passphrase was not asked again");
        };
        assert_eq!(
            v.answered(third.id, "new 0ne", &r, &t),
            Errand::Say(t.t(key::BELONGINGS_PASSPHRASES_DIFFER))
        );
        assert!(!places.new_passphrase().exists());
        // AND AGAIN, THIS TIME ALIKE.
        let Errand::Ask(first) = v.command("belongings-passphrase", &r, &zone(), 100, &t) else {
            panic!();
        };
        let Errand::Ask(second) = v.answered(first.id, "old one", &r, &t) else {
            panic!();
        };
        let Errand::Ask(third) = v.answered(second.id, "new one", &r, &t) else {
            panic!();
        };
        assert_eq!(
            v.answered(third.id, "new one", &r, &t),
            Errand::Queue(Op::Rekey)
        );
        assert_eq!(
            std::fs::read_to_string(places.passphrase()).unwrap(),
            "old one"
        );
        assert_eq!(
            std::fs::read_to_string(places.new_passphrase()).unwrap(),
            "new one"
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: an address with a credential, `http://` or a local path is
    /// kept, or a plain one is not.
    #[test]
    fn only_a_plain_address_is_kept() {
        let places = scratch("address");
        let t = texts("en-GB");
        let r = Readings::read(Some(places.clone()), 1);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        for (typed, refused) in [
            ("https://me:pw@example.org/b.git", true),
            ("http://example.org/b.git", true),
            ("/srv/b.git", true),
            ("ssh://git@[2001:db8::1]/b.git", true),
            ("git@example.org:me/b.git", false),
        ] {
            let Errand::Ask(line) = v.command("belongings-repository", &r, &zone(), 100, &t) else {
                panic!("the address was not asked");
            };
            assert!(!line.secret);
            let errand = v.answered(line.id, typed, &r, &t);
            assert_eq!(
                matches!(errand, Errand::Say(_)),
                refused,
                "{typed}: {errand:?}"
            );
        }
        assert_eq!(
            store_of(&places).read().repository,
            "git@example.org:me/b.git"
        );
        // TYPING IT LET ITS HOST — in the consent file beside the settings,
        // never another one.
        assert!(ronoctua_consent::allows_under(
            &places.config,
            "example.org"
        ));
        assert_eq!(Readings::read(Some(places.clone()), 1).unlet, None);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a path typed or pressed in the file manager is kept under a
    /// name that is not its logical one, one outside the home or under the
    /// access folder is kept, or the same gesture on what is kept does not
    /// take it out.
    #[test]
    fn a_path_is_kept_by_its_logical_name_and_the_same_gesture_takes_it_out() {
        let places = scratch("keep");
        let t = texts("en-GB");
        assert_eq!(
            logical_of_typed(&places, "~/notes").as_deref(),
            Some("home/notes")
        );
        assert_eq!(
            logical_of_typed(&places, "notes/../music").as_deref(),
            Some("home/music")
        );
        assert_eq!(
            logical_of_typed(&places, &places.config.join("x.json").to_string_lossy()).as_deref(),
            Some("config/x.json")
        );
        assert_eq!(logical_of_typed(&places, "/etc/passwd"), None);
        assert_eq!(logical_of_typed(&places, "../../etc/passwd"), None);

        let notes = places.home.join("notes");
        let e = keep_under(&places, std::slice::from_ref(&notes));
        assert_eq!(e.key, key::BELONGINGS_NOW_KEPT);
        assert_eq!(e.args, [("name", "~/notes".to_owned())]);
        assert!(!t.t_with(e.key, &e.borrowed()).starts_with('['));
        assert_eq!(store_of(&places).read().extra, ["home/notes"]);
        let e = keep_under(&places, &[notes]);
        assert_eq!(e.key, key::BELONGINGS_STOPPED_KEEPING);
        let now = store_of(&places).read();
        assert!(now.extra.is_empty());
        assert_eq!(
            now.excluded,
            ["home/notes"],
            "stopped without being taken out: the next sync would send it again"
        );
        assert_eq!(
            keep_under(&places, &[PathBuf::from("/etc/hosts")]).key,
            key::BELONGINGS_OUTSIDE_HOME
        );
        assert_eq!(
            keep_under(&places, &[places.keys()]).key,
            key::BELONGINGS_NEVER_KEPT
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a row that acts has no sentence under the pointer, or a
    /// command a row runs, or a key of the view, is not in the registry.
    #[test]
    fn every_row_explains_itself_and_every_command_is_registered() {
        let places = scratch("about");
        let r = waiting(&places);
        let t = texts("en-GB");
        let v = View::fresh(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        for (n, row) in rows.iter().enumerate() {
            if row
                .target
                .as_ref()
                .is_some_and(|t| !matches!(t, Target::Fact(_) | Target::DiffLine(..)))
            {
                let about = View::about(&rows, n, &t).expect("a row that acts says nothing");
                assert!(!about.starts_with('['), "{about}");
            }
        }
        for id in reached().chain([OPEN_COMMAND, WINDOW_COMMAND]) {
            assert!(
                KEYS.commands.iter().any(|c| c.id == id),
                "`{id}` is not registered"
            );
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a repository whose host nobody let draws no consent row —
    /// every sync would be refused with no way out on screen — the row is
    /// not the press (`RET`, a second click, `C-a` on any row) or its key is
    /// not taught while it is drawn, or the row stays once the host is let.
    #[test]
    fn a_host_nobody_let_is_asked_on_its_row() {
        let places = scratch("consent-row");
        let r = waiting(&places);
        let _ = std::fs::remove_file(places.config.join(ronoctua_consent::FILE));
        let r = Readings::read(r.places.clone(), r.now_s);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let v = View::fresh(&r, &zone(), 100, &t);
            let rows = v.rows(&r, &zone(), 100, &t);
            let label = crate::consent_rows::label(&["example.org"], &t);
            let at = rows
                .iter()
                .position(|row| row.target == Some(Target::Let("example.org".into())))
                .expect("no consent row");
            assert!(drawn(&rows[at..=at]).contains(&label), "{language}");
            let about = View::about(&rows, at, &t).unwrap();
            assert_eq!(about, t.t(key::ABOUT_CONSENT_ALLOW));
            let taught = v.hint(&r, &zone(), 100, &t);
            assert!(
                taught
                    .iter()
                    .any(|p| p.id == Some(crate::consent_rows::ALLOW_ID)),
                "{language}: the row's key is not taught"
            );
            // AN APPLY WOULD PUSH: refused here, the session kept.
            let mut w = View::fresh(&r, &zone(), 100, &t);
            assert_eq!(
                w.command("belongings-apply", &r, &zone(), 100, &t),
                Errand::Say(t.t(words::refusal_key(Why::NotLet)))
            );
            assert!(places.plan().is_file());
        }
        // THE PRESS, as a second click on the row does it.
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        let at = rows
            .iter()
            .position(|row| row.target == Some(Target::Let("example.org".into())))
            .unwrap();
        assert!(!v.clicked(at, &rows), "the first click is a selection");
        assert!(v.clicked(at, &rows));
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let r = Readings::read(r.places.clone(), r.now_s);
        assert_eq!(r.unlet, None);
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row.target, Some(Target::Let(_)))),
            "the row stays once the host is let"
        );
        assert!(
            !v.hint(&r, &zone(), 100, &t)
                .iter()
                .any(|p| p.id == Some(crate::consent_rows::ALLOW_ID)),
            "a key that does nothing is taught"
        );
        // AN ADDRESS KEPT BEFORE ITS HOST COULD BE REFUSED says why, and no
        // row asks a host no consent can name.
        store_of(&places)
            .write(&Belongings::with_repository(
                "ssh://git@[2001:db8::1]/b.git",
            ))
            .unwrap();
        let r = Readings::read(r.places.clone(), r.now_s);
        assert_eq!(r.unlet, None);
        assert_eq!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Say(t.t(words::invalid_key(prefs::Invalid::UnnamedHost)))
        );
        // `C-a` on a page with no row does nothing.
        assert_eq!(v.press(Press::Allow, &r, &zone(), 100, &t), Errand::Nothing);
        let key = ronoctua_keymap::Key::parse("C-a").unwrap();
        assert_eq!(press_of_key(key), Some(Press::Allow));
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a conflict a merge reached does not say where it clashes
    /// and offer the two merged decisions, a merged command does not decide
    /// the conflict under the cursor — or decides one no merge reached —
    /// a door's write loses the fields another decided, or the page of what
    /// is kept does not show the plain part and who reads it.
    #[test]
    fn a_merged_conflict_is_offered_both_merges_and_the_fields_survive() {
        let places = scratch("merged");
        let r = waiting(&places);
        let mut plan = r.plan.clone().unwrap();
        plan.conflicts[0].merge = Some(Merging::Json {
            fields: vec![ronoctua_belongings::plan::FieldConflict {
                pointer: "/theme".into(),
                base: Some("\"owl\"".into()),
                ours: Some("\"paper\"".into()),
                theirs: Some("\"cyber\"".into()),
            }],
        });
        // A CONFLICT NO MACHINE IS NAMED FOR (a plain file's: no plain
        // commit says which one wrote it).
        plan.conflicts[1].machine = String::new();
        ronoctua_belongings::secret::write_private(
            &places.plan(),
            &serde_json::to_vec(&plan).expect("plan"),
        )
        .expect("plan written");
        // ANOTHER DOOR decided a field: this one keeps it.
        let mut file = DecisionsFile::default();
        file.fields
            .entry("config/theme.json".into())
            .or_default()
            .insert("/theme".into(), Decision::TakeRemote);
        ronoctua_belongings::secret::write_private(
            &places.decisions(),
            &serde_json::to_vec(&file).expect("decisions"),
        )
        .expect("decisions written");
        // A PLAIN RUN left a file out for its size, and a clash for later.
        let last = Last {
            plain: ronoctua_belongings::plain::PlainBase {
                repository: r.prefs.repository.clone(),
                too_large: vec!["state/person/software/icons/big.png".into()],
                conflicts: vec!["state/person/options.json".into()],
                ..Default::default()
            },
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&last).expect("last"),
        )
        .expect("last written");
        let r = Readings::read(Some(places.clone()), r.now_s);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let v = View::fresh(&r, &zone(), 100, &t);
            let rows = v.rows(&r, &zone(), 100, &t);
            let text = drawn(&rows);
            let fields = t.t_args(
                key::BELONGINGS_MERGE_FIELDS,
                &[
                    ("n", ronoctua_i18n::Arg::Number(1)),
                    ("fields", ronoctua_i18n::Arg::Text("/theme")),
                ],
            );
            assert!(text.contains(&fields), "{language}: {text}");
            let unnamed = &plan.conflicts[1];
            let sides = t.t_with(
                key::BELONGINGS_SIDES_UNNAMED,
                &[
                    ("here", &side_text(unnamed.local.as_ref(), &zone(), &t)),
                    ("there", &side_text(unnamed.remote.as_ref(), &zone(), &t)),
                ],
            );
            assert!(
                text.contains(&sides),
                "{language}: a conflict with no machine is not said without one: {text}"
            );
            assert!(
                text.contains(&t.t(key::BELONGINGS_REASON_PERSON_DATA))
                    && text.contains(&t.t(key::BELONGINGS_REASON_BRAIN_MODULE)),
                "{language}: the plain part is not on the page: {text}"
            );
            let left_out =
                Target::Fact("plain-too-large:state/person/software/icons/big.png".into());
            assert!(
                text.contains(&t.t(words::refusal_key(Why::PlainTooLarge)))
                    && rows
                        .iter()
                        .any(|row| row.target.as_ref() == Some(&left_out)),
                "{language}: a file left out for its size is not said: {text}"
            );
            let waiting = t.t_args(
                key::BELONGINGS_PLAIN_CONFLICTS,
                &[("n", ronoctua_i18n::Arg::Number(1))],
            );
            assert!(text.contains(&waiting), "{language}: {text}");
            for d in [Decision::TakeMergedOurs, Decision::TakeMergedTheirs] {
                let target = Target::Decide("config/theme.json".into(), d);
                assert!(
                    rows.iter().any(|row| row.target.as_ref() == Some(&target)),
                    "{language}: {d:?} is not offered"
                );
            }
        }
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("config/theme.json".into()),
            &t,
        );
        assert_eq!(
            v.command("belongings-take-merged-theirs", &r, &zone(), 100, &t),
            Errand::Nothing
        );
        let after = DecisionsFile::read(&places.decisions()).expect("decisions");
        assert_eq!(
            after.paths.get("config/theme.json"),
            Some(&Decision::TakeMergedTheirs)
        );
        assert_eq!(after.fields, file.fields, "a door's write lost the fields");
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            r.decisions.get("config/theme.json"),
            Some(&Decision::TakeMergedTheirs)
        );
        // NO MERGE REACHED THE OTHER: refused, with its own words.
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("home/notes/todo.md".into()),
            &t,
        );
        assert_eq!(
            v.command("belongings-take-merged-ours", &r, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_NO_MERGE_HERE))
        );
        // A PLAIN PATH given to "keep" travels already.
        let explained = keep_under(&places, &[places.state.join("person/options.json")]);
        assert_eq!(explained.key, key::BELONGINGS_PLAIN_ALREADY);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the period stops walking round, both ways.
    #[test]
    fn the_period_turns_both_ways() {
        for p in prefs::PERIODS {
            assert_eq!(previous_period(Belongings::next_period(p)), p);
        }
    }
    /// The readings of `places`, with the listing counted in place.
    fn counted(places: &Places, now_s: i64) -> Readings {
        let mut r = Readings::read(Some(places.clone()), now_s);
        Counting::in_place().ask(&mut r);
        r
    }

    /// A machine set up to sync: a repository, its host let, a deploy key,
    /// and a sync behind it (`last.json`), so a sync is not a first.
    fn ready(places: &Places) {
        let prefs = Belongings::with_repository("git@example.org:someone/belongings.git");
        store_of(places).write(&prefs).expect("prefs");
        ronoctua_consent::let_hosts_under(&places.config, &["example.org"], true).expect("consent");
        std::fs::create_dir_all(places.keys()).expect("keys");
        std::fs::write(places.deploy_key(), "k").expect("key");
        let last = Last {
            repository: prefs.repository,
            at_s: 1_700_000_000,
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&last).expect("last"),
        )
        .expect("last written");
    }

    fn status(places: &Places, outcome: Outcome) {
        let s = Status {
            at_s: 1_700_000_100,
            outcome,
        };
        ronoctua_belongings::secret::write_private(
            &places.status(),
            &serde_json::to_vec(&s).expect("status"),
        )
        .expect("status written");
    }

    fn synced(offers: Vec<Offer>, budget: Option<Budget>) -> Outcome {
        Outcome::Synced {
            pulled: 0,
            pushed: 0,
            deleted_here: 0,
            deleted_there: 0,
            kept: 0,
            kept_bytes: 0,
            large: vec![],
            unreadable: vec![],
            unwritten: vec![],
            offers,
            repaired: vec![],
            budget,
        }
    }

    fn put(places: &Places, logical: &str, bytes: &[u8]) {
        let at = places.resolve(logical).expect("logical");
        std::fs::create_dir_all(at.parent().expect("parent")).expect("folder");
        std::fs::write(at, bytes).expect("file");
    }

    fn row_of<'a>(rows: &'a [Row], target: &Target) -> &'a Row {
        rows.iter()
            .find(|row| row.target.as_ref() == Some(target))
            .unwrap_or_else(|| panic!("no row is {target:?}: {}", drawn(rows)))
    }

    /// A key as a run that opened `key.age` leaves it in `last.json`, and
    /// what that run held and left unsigned.
    fn signed_last(places: &Places, fingerprint: &str) {
        let mut last = Last::read(&places.last()).expect("a last.json");
        last.plain = ronoctua_belongings::plain::PlainBase {
            repository: last.repository.clone(),
            signer: format!("{}{}", ronoctua_person::local::KEY_PREFIX, "A".repeat(2603)),
            fingerprint: fingerprint.to_owned(),
            held: BTreeMap::from([(
                "state/person/options.json".to_owned(),
                ronoctua_belongings::plain::HeldWhy::NotAsSigned,
            )]),
            unsigned: vec!["config/brain-modules/my-tea/module.rs".to_owned()],
            ..ronoctua_belongings::plain::PlainBase::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&last).expect("last"),
        )
        .expect("last written");
    }

    /// A pin of `fingerprint`, as root writes it.
    fn pin_of(fingerprint: &str) -> ronoctua_person::local::Signer {
        ronoctua_person::local::Signer::pinned(
            format!("{}{}", ronoctua_person::local::KEY_PREFIX, "B".repeat(2603)),
            fingerprint.to_owned(),
            ronoctua_person::Stamp::parse("2026-10-11T12:00:00Z").unwrap(),
        )
    }

    /// FAILS IF: the page does not say, in every language, what the
    /// repository changed that this machine held and why, what waits for the
    /// passphrase to be signed, whose key this is and whether the machine
    /// trusts it, what root last refused or kept back — or "pin my key" does
    /// not ask the password hidden and hand the door THE KEY THE PASSPHRASE
    /// OPENED, or shows where the machine already trusts it; or "sign now"
    /// does not ask the passphrase hidden (and write it private) when the
    /// session keeps no identity, and ask nothing when it does.
    #[test]
    fn the_signature_says_whose_key_and_pins_it_through_the_door() {
        let places = scratch("signature");
        ready(&places);
        let mine = "ab".repeat(32);
        signed_last(&places, &mine);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut r = Readings::read(Some(places.clone()), 1_700_000_200);
            r.pin = None;
            let v = View::fresh(&r, &zone(), 100, &t);
            // A VALUE THAT WRAPS is read whole: the rows' pieces joined.
            let flat = |rows: &[Row]| drawn(rows).replace(" | ", " ");
            let rows = v.rows(&r, &zone(), 100, &t);
            let text = flat(&rows);
            for k in [
                key::BELONGINGS_SIGNATURE,
                key::BELONGINGS_SIGNATURE_NONE,
                key::BELONGINGS_HELD_NOT_AS_SIGNED,
                key::BELONGINGS_UNSIGNED,
                key::BELONGINGS_SIGN,
                key::BELONGINGS_PIN,
            ] {
                assert!(text.contains(&t.t(k)), "{language}: no {k}: {text}");
            }
            assert!(text.contains("abab abab abab abab"), "{language}: {text}");
            assert!(!text.contains('['), "{language}: a raw key: {text}");
            // TRUSTED: no pin offered; ANOTHER KEY: offered, and said.
            r.pin = Some(pin_of(&mine));
            let text = flat(&v.rows(&r, &zone(), 100, &t));
            assert!(text.contains(&t.t(key::BELONGINGS_SIGNATURE_PINNED)));
            assert!(
                !text.contains(&t.t(key::BELONGINGS_PIN)),
                "{language}: {text}"
            );
            r.pin = Some(pin_of(&"cd".repeat(32)));
            let text = flat(&v.rows(&r, &zone(), 100, &t));
            assert!(text.contains(&t.t(key::BELONGINGS_SIGNATURE_OTHER)));
            assert!(text.contains(&t.t(key::BELONGINGS_PIN)));
            // WHAT ROOT LAST REFUSED, and kept back.
            let mut refused = pin_of(&mine);
            refused.refused = Some(ronoctua_person::local::SignatureRefused {
                at: ronoctua_person::Stamp::parse("2026-10-11T13:00:00Z").unwrap(),
                why: ronoctua_person::tree::TreeWhy::RolledBack,
                detail: "counter 3, took 9".to_owned(),
            });
            refused.kept_back.push(ronoctua_person::tree::KeptBack {
                at: "person/software/declared.json".to_owned(),
                why: ronoctua_person::tree::TreeWhy::NotAsSigned,
            });
            r.pin = Some(refused);
            let text = flat(&v.rows(&r, &zone(), 100, &t));
            assert!(text.contains(&t.t(key::BELONGINGS_ROOT_REFUSED_ROLLED_BACK)));
            assert!(text.contains(&t.t(key::BELONGINGS_KEPT_BACK_NOT_AS_SIGNED)));
            assert!(
                !text.contains("counter 3"),
                "the journal's detail is not the page's"
            );
        }

        // PIN: the door handed the key the passphrase opened at once — the
        // password, key or finger is the sudo bar's to ask, never a line of
        // this view's.
        let t = texts("en-GB");
        let mut r = Readings::read(Some(places.clone()), 1_700_000_200);
        r.pin = None;
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &Target::Pin, &t);
        let Errand::Pin { key, fingerprint } = v.press(Press::Enter, &r, &zone(), 100, &t) else {
            panic!("the door was not asked");
        };
        assert_eq!(key, r.plain.signer);
        assert_eq!(fingerprint, mine);
        // TRUSTED ALREADY: said, and nothing asked.
        r.pin = Some(pin_of(&mine));
        assert_eq!(
            v.command("belongings-pin", &r, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_SIGNATURE_ALREADY))
        );
        // A KEY NOBODY OPENED HERE: refused, never the repository's copy.
        let mut unknown = r.clone();
        unknown.plain.signer.clear();
        unknown.plain.fingerprint.clear();
        unknown.pin = None;
        assert_eq!(
            v.command("belongings-pin", &unknown, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_SIGNATURE_SYNC_FIRST))
        );
        // HOW A PIN WENT, as the door's thread wrote it.
        r.pin = None;
        r.pinning = Some(Pinning {
            fingerprint: mine,
            said: PinSaid::Refused {
                why: ronoctua_security::Why::WrongPassword,
            },
        });
        let text = drawn(&v.rows(&r, &zone(), 100, &t)).replace(" | ", " ");
        assert!(text.contains(&t.t(crate::security_words::refusal(
            ronoctua_security::Why::WrongPassword
        ))));

        // SIGN: the passphrase asked hidden and written private, then the
        // order; with the identity kept, the order at once.
        let Errand::Ask(line) = v.command("belongings-sign", &r, &zone(), 100, &t) else {
            panic!("the passphrase was not asked");
        };
        assert!(line.secret);
        assert_eq!(
            v.answered(line.id, "correct horse", &r, &t),
            Errand::Queue(Op::Sign)
        );
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(places.passphrase())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "the passphrase is readable by others");
        let _ = std::fs::remove_file(places.passphrase());
        let mut kept = r.clone();
        kept.kept_until = Some(1_700_009_999);
        assert_eq!(
            v.command("belongings-sign", &kept, &zone(), 100, &t),
            Errand::Queue(Op::Sign)
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the listing is not one section, grouped by where each file
    /// comes from — a rule, a target with its actions, the person's
    /// addition, the plain part — with each file's state and size, what is
    /// never kept and why; or `RET` on a file does not take it out and bring
    /// it back; or a word draws as its raw key.
    #[test]
    fn the_listing_is_grouped_by_origin_one_row_per_file() {
        let places = scratch("listing");
        ready(&places);
        store_of(&places)
            .write(&Belongings {
                extra: vec!["home/notes".into()],
                ..store_of(&places).read()
            })
            .expect("prefs");
        put(&places, "config/theme.json", b"{\"theme\":\"owl\"}");
        put(&places, "config/column-player.pid", b"1");
        put(&places, "home/.ssh/config", b"Host *\n");
        put(&places, "home/notes/a.md", b"# a");
        put(&places, "state/person/options.json", b"{\"format\":1}");
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = counted(&places, 1_700_000_500);
            let v = View::fresh(&r, &zone(), 100, &t);
            let rows = v.rows(&r, &zone(), 100, &t);
            let text = drawn(&rows);
            assert!(
                text.contains(&t.t(key::BELONGINGS_LISTING)),
                "{language}: {text}"
            );
            assert!(
                !text.contains(&t.t(key::BELONGINGS_COUNTING)),
                "{language}: still counting"
            );
            let new = t.t_with(
                key::BELONGINGS_ITEM,
                &[
                    ("state", &t.t(key::BELONGINGS_STATE_NEW)),
                    ("size", &bytes_text(15, &t)),
                ],
            );
            let theme = row_of(
                &rows,
                &Target::File("config/theme.json".into(), FileAct::StopKeeping),
            );
            assert!(
                drawn(std::slice::from_ref(theme)).contains(&new),
                "{language}: {text}"
            );
            row_of(
                &rows,
                &Target::File("home/.ssh/config".into(), FileAct::StopKeeping),
            );
            row_of(
                &rows,
                &Target::File("home/notes/a.md".into(), FileAct::StopKeeping),
            );
            row_of(
                &rows,
                &Target::Fact("file:state/person/options.json".into()),
            );
            row_of(&rows, &Target::NotHere("ssh"));
            row_of(&rows, &Target::Nowhere("ssh"));
            let pid = t.t_with(
                key::BELONGINGS_NEVER_BECAUSE,
                &[("reason", &t.t("belongings-reason-draft"))],
            );
            assert!(
                text.contains(&pid),
                "{language}: the draft's reason: {text}"
            );
            assert!(
                text.contains(&t.t("belongings-target-ssh")),
                "{language}: {text}"
            );
            assert!(!text.contains('['), "{language}: a raw key: {text}");
            // EVERY ROW THAT ACTS says what it does under the pointer.
            for (n, row) in rows.iter().enumerate() {
                if row
                    .target
                    .as_ref()
                    .is_some_and(|t| !matches!(t, Target::Fact(_) | Target::DiffLine(..)))
                {
                    let about = View::about(&rows, n, &t)
                        .unwrap_or_else(|| panic!("{language}: {:?} says nothing", row.target));
                    assert!(!about.starts_with('['), "{about}");
                }
            }
        }
        // `RET` ON A FILE takes it out — and on the same row brings it back.
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::File("config/theme.json".into(), FileAct::StopKeeping),
            &t,
        );
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        assert!(
            store_of(&places)
                .read()
                .excluded
                .contains(&"config/theme.json".to_owned())
        );
        let r = counted(&places, 1_700_000_500);
        v.settle_in(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(
            !rows
                .iter()
                .any(|row| row.target == Some(Target::Excluded("config/theme.json".into()))),
            "a file taken out is listed twice"
        );
        assert_eq!(
            rows[v.cursor()].target,
            Some(Target::File("config/theme.json".into(), FileAct::KeepAgain)),
            "the cursor left the file it acted on"
        );
        v.press(Press::Enter, &r, &zone(), 100, &t);
        assert!(
            !store_of(&places)
                .read()
                .excluded
                .contains(&"config/theme.json".to_owned())
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: "not on this machine" does not take a target's paths out
    /// here, or does not come back; "not on any machine" acts without a
    /// question, draws "yes" first, or — answered yes — asks nothing of the
    /// next sync and starts none; or "on every machine again" asks nothing.
    #[test]
    fn a_targets_actions_are_here_nowhere_and_back() {
        let places = scratch("targets");
        ready(&places);
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &Target::NotHere("ssh"), &t);
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        assert!(
            store_of(&places)
                .read()
                .excluded
                .contains(&"home/.ssh".to_owned())
        );
        let r = counted(&places, 1_700_000_500);
        stand_on(&mut v, &r, &Target::HereAgain("ssh"), &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        assert!(
            !store_of(&places)
                .read()
                .excluded
                .contains(&"home/.ssh".to_owned())
        );
        // "NOT ON ANY MACHINE": a question, "no" first and the cursor on it.
        let r = counted(&places, 1_700_000_500);
        stand_on(&mut v, &r, &Target::Nowhere("gnupg"), &t);
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let rows = v.rows(&r, &zone(), 100, &t);
        assert_eq!(rows[v.cursor()].target, Some(Target::Answer(false)));
        let answers: Vec<&Target> = rows.iter().filter_map(|row| row.target.as_ref()).collect();
        assert_eq!(answers, [&Target::Answer(false), &Target::Answer(true)]);
        let taught: Vec<&str> = v
            .hint(&r, &zone(), 100, &t)
            .iter()
            .filter_map(|p| p.id)
            .collect();
        assert!(!taught.contains(&"belongings-sync"), "{taught:?}");
        // `Esc` is "no": nothing asked of the next sync.
        assert_eq!(
            v.press(Press::Escape, &r, &zone(), 100, &t),
            Errand::Nothing
        );
        assert!(!places.requests().exists());
        let rows = v.rows(&r, &zone(), 100, &t);
        assert_eq!(rows[v.cursor()].target, Some(Target::Nowhere("gnupg")));
        // YES: asked of the next sync, which starts — asking the passphrase.
        v.press(Press::Enter, &r, &zone(), 100, &t);
        v.press(Press::Down, &r, &zone(), 100, &t);
        let Errand::Ask(line) = v.press(Press::Enter, &r, &zone(), 100, &t) else {
            panic!("the sync did not start");
        };
        assert!(line.secret);
        assert_eq!(
            Requests::read(&places.requests()).requests,
            [Request::DropEverywhere {
                target: "gnupg".into()
            }]
        );
        v.let_go();
        let r = counted(&places, 1_700_000_500);
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(drawn(&rows).contains(&t.t(key::BELONGINGS_TARGET_ASKED_NOWHERE)));
        // AND BACK, ON EVERY MACHINE.
        stand_on(&mut v, &r, &Target::Everywhere("gnupg"), &t);
        assert!(matches!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Ask(_)
        ));
        assert_eq!(
            Requests::read(&places.requests()).requests,
            [Request::KeepEverywhere {
                target: "gnupg".into()
            }]
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: what a sync offered is not drawn with "no" first and the
    /// cursor born on it; "no" is not kept for both doors (the file) or the
    /// offer stays; "yes" does not reach the door as the offer to turn on; or
    /// what it came to is not said.
    #[test]
    fn an_offer_is_asked_no_first_and_answered_for_both_doors() {
        let places = scratch("offers");
        ready(&places);
        let key_offer = Offer::SshSigning {
            key: "home/.ssh/id_ed25519.pub".into(),
        };
        status(
            &places,
            synced(vec![Offer::PamU2f, key_offer.clone()], None),
        );
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = counted(&places, 1_700_000_500);
            let v = View::fresh(&r, &zone(), 100, &t);
            let rows = v.rows(&r, &zone(), 100, &t);
            assert_eq!(
                rows[v.cursor()].target,
                Some(Target::Offer(Offer::PamU2f, false)),
                "{language}: the cursor is not born on \"no\""
            );
            let no = rows
                .iter()
                .position(|row| row.target == Some(Target::Offer(Offer::PamU2f, false)));
            let yes = rows
                .iter()
                .position(|row| row.target == Some(Target::Offer(Offer::PamU2f, true)));
            assert!(no < yes, "{language}: \"yes\" is drawn first");
            // A NOTE WRAPS at a space: the rows joined by one space read whole.
            assert!(
                drawn(&rows)
                    .replace(" | ", " ")
                    .contains(&t.t(key::BELONGINGS_OFFER_PAM_U2F)),
                "{language}"
            );
        }
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let r = counted(&places, 1_700_000_500);
        assert_eq!(r.answer_of(&Offer::PamU2f), Some(&Said::No));
        assert!(
            !v.rows(&r, &zone(), 100, &t)
                .iter()
                .any(|row| row.target == Some(Target::Offer(Offer::PamU2f, false))),
            "an offer answered no is still asked"
        );
        stand_on(&mut v, &r, &Target::Offer(key_offer.clone(), true), &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Accept(1_700_000_100, key_offer.clone())
        );
        // WHAT IT CAME TO, said.
        answer(
            &places,
            1_700_000_100,
            &key_offer,
            Said::Refused { why: Why::Failed },
        )
        .unwrap();
        let r = counted(&places, 1_700_000_500);
        let said = t.t_with(
            key::BELONGINGS_OFFER_REFUSED,
            &[("why", &t.t(words::refusal_key(Why::Failed)))],
        );
        let text = drawn(&v.rows(&r, &zone(), 100, &t)).replace(" | ", " ");
        assert!(text.contains(&said), "{text}");
        // A LATER SYNC'S OFFERS are asked again: the answers were the other's.
        status(&places, synced(vec![Offer::PamU2f], None));
        let mut s = Status::read(&places.status()).unwrap();
        s.at_s += 1;
        ronoctua_belongings::secret::write_private(
            &places.status(),
            &serde_json::to_vec(&s).unwrap(),
        )
        .unwrap();
        assert_eq!(counted(&places, 1_700_000_500).offers_waiting().len(), 1);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Mode {
        #[default]
        Calm,
        Busy,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Thing {
        mode: Mode,
        other: u32,
    }

    /// A store of `places` that keeps its ledger where the view reads it.
    fn thing_store(places: &Places) -> ronoctua_store::Store<Thing> {
        ronoctua_store::Store::<Thing>::at(Some(places.config.clone()), "thing.json")
            .with_roots(Some(places.config.clone()), Some(places.state.clone()))
    }

    /// FAILS IF: a file a lenient read repaired is not listed with what
    /// could not be read and why — never the value — with "leave it" before
    /// "write the fix" and the cursor born on it; or either answer does not
    /// do what it says to the file and the ledger.
    #[test]
    fn a_repair_is_listed_and_left_or_written() {
        let places = scratch("repairs");
        ready(&places);
        std::fs::write(
            places.config.join("thing.json"),
            br#"{"mode": "frantic-secret-word", "other": 3}"#,
        )
        .unwrap();
        let _ = thing_store(&places).read();
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        assert_eq!(r.repairs.len(), 1, "the ledger is not read");
        let v = View::fresh(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        let text = drawn(&rows);
        assert!(text.contains("/mode"), "{text}");
        assert!(
            text.contains(&t.t("belongings-repair-why-unknown-variant")),
            "{text}"
        );
        assert!(
            !text.contains("frantic-secret-word"),
            "a value was drawn: {text}"
        );
        assert_eq!(
            rows[v.cursor()].target,
            Some(Target::LeaveIt("config/thing.json".into()))
        );
        let leave = rows
            .iter()
            .position(|row| row.target == Some(Target::LeaveIt("config/thing.json".into())));
        let write = rows
            .iter()
            .position(|row| row.target == Some(Target::WriteFix("config/thing.json".into())));
        assert!(leave < write, "\"write the fix\" is drawn first");
        // LEAVE IT: asked no more, the file as it was.
        let mut v = View::fresh(&r, &zone(), 100, &t);
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        assert!(counted(&places, 1).repairs.is_empty());
        assert!(
            std::fs::read_to_string(places.config.join("thing.json"))
                .unwrap()
                .contains("frantic-secret-word")
        );
        // A NEW BREAK, and the fix written.
        std::fs::write(
            places.config.join("thing.json"),
            br#"{"mode": "other", "other": 4}"#,
        )
        .unwrap();
        let _ = thing_store(&places).read();
        let r = counted(&places, 1_700_000_500);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::WriteFix("config/thing.json".into()),
            &t,
        );
        assert!(matches!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Say(_)
        ));
        let now = std::fs::read_to_string(places.config.join("thing.json")).unwrap();
        assert!(now.contains("calm") && now.contains('4'), "{now}");
        assert!(counted(&places, 1).repairs.is_empty());
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the history's weight is not drawn, its compaction is not
    /// offered when it would fold something, it starts without a question
    /// ("no" first), or a "yes" does not ask the passphrase and send the
    /// order — or asks it while the session remembers it.
    #[test]
    fn the_history_is_weighed_and_compacted_after_a_question() {
        let places = scratch("history");
        ready(&places);
        let budget = Budget {
            tree_bytes: 10 << 20,
            history_bytes: 1_200_000_000,
            recommend_bytes: 1_000_000_000,
            foldable: true,
        };
        status(&places, synced(vec![], Some(budget)));
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let text = drawn(&v.rows(&r, &zone(), 100, &t)).replace(" | ", " ");
        assert!(text.contains(&t.t(key::BELONGINGS_HISTORY)), "{text}");
        assert!(
            text.contains(&t.t_with(
                key::BELONGINGS_WEIGHT_COMPACT,
                &[("history", &bytes_text(budget.history_bytes, &t))]
            )),
            "{text}"
        );
        stand_on(&mut v, &r, &Target::Compact, &t);
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        assert_eq!(
            v.rows(&r, &zone(), 100, &t)[v.cursor()].target,
            Some(Target::Answer(false))
        );
        v.press(Press::Down, &r, &zone(), 100, &t);
        let Errand::Ask(line) = v.press(Press::Enter, &r, &zone(), 100, &t) else {
            panic!("the passphrase was not asked");
        };
        assert!(line.secret);
        assert_eq!(line.asks, Some(key::ASK_BELONGINGS_COMPACT));
        assert_eq!(
            v.answered(line.id, "correct horse", &r, &t),
            Errand::Queue(Op::Compact)
        );
        assert!(places.passphrase().is_file());
        // NOTHING TO FOLD: nothing offered.
        status(
            &places,
            synced(
                vec![],
                Some(Budget {
                    foldable: false,
                    ..budget
                }),
            ),
        );
        let r = counted(&places, 1_700_000_500);
        assert!(
            !v.rows(&r, &zone(), 100, &t)
                .iter()
                .any(|row| row.target == Some(Target::Compact))
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the session's switch does not turn both ways, turning it
    /// off does not forget what the session keeps, or a sync asks the
    /// passphrase while the session remembers it — or asks nothing when the
    /// last run said there was none.
    #[test]
    fn the_session_remembers_the_passphrase_until_told_not_to() {
        let places = scratch("unlock");
        ready(&places);
        let t = texts("en-GB");
        let repository = store_of(&places).read().repository;
        ronoctua_belongings::secret::make_private_dir(&places.runtime).unwrap();
        ronoctua_belongings::unlock::Marker {
            format: ronoctua_belongings::unlock::MARKER_FORMAT,
            repository: ronoctua_belongings::unlock::repository_tag(&repository),
            until_s: 1_700_009_999,
        }
        .write(&places)
        .unwrap();
        let r = counted(&places, 1_700_000_500);
        assert_eq!(r.kept_until, Some(1_700_009_999));
        let mut v = View::fresh(&r, &zone(), 100, &t);
        assert_eq!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Queue(Op::Prepare)
        );
        assert!(!places.passphrase().exists(), "a passphrase was written");
        let text = drawn(&v.rows(&r, &zone(), 100, &t));
        assert!(
            text.contains(&t.t_with(
                key::BELONGINGS_UNLOCK_KEPT,
                &[("until", &when(1_700_009_999, &zone(), &t))]
            )),
            "{text}"
        );
        // THE MARKER OUTLIVED BY THE KERNEL: said once, then asked.
        status(
            &places,
            Outcome::Refused {
                why: Why::NoPassphrase,
            },
        );
        let r = counted(&places, 1_700_000_500);
        assert!(matches!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Ask(_)
        ));
        v.let_go();
        // OFF: written, and what the session keeps forgotten.
        stand_on(&mut v, &r, &Target::Unlock, &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Queue(Op::Forget)
        );
        assert!(!store_of(&places).read().keep_unlocked);
        let r = counted(&places, 1_700_000_500);
        assert_eq!(r.kept_until, None, "a switch off still remembers");
        assert_eq!(v.press(Press::More, &r, &zone(), 100, &t), Errand::Nothing);
        assert!(store_of(&places).read().keep_unlocked);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a JSON conflict's fields are not each drawn with both
    /// values and the one they came from, a field's keys decide the whole
    /// path instead of the field, a decided field leaves a decision of the
    /// path that would go over it, or every field decided does not let the
    /// sync be applied with the fields written for the CLI.
    #[test]
    fn every_field_of_a_json_conflict_is_decided_on_its_own() {
        let places = scratch("fields");
        let r = waiting(&places);
        let mut plan = r.plan.clone().unwrap();
        plan.conflicts[0].merge = Some(Merging::Json {
            fields: vec![
                ronoctua_belongings::plan::FieldConflict {
                    pointer: "/theme".into(),
                    base: Some("\"owl\"".into()),
                    ours: Some("\"paper\"".into()),
                    theirs: Some("\"cyber\"".into()),
                },
                ronoctua_belongings::plan::FieldConflict {
                    pointer: "/size".into(),
                    base: Some("100".into()),
                    ours: Some("110".into()),
                    theirs: None,
                },
            ],
        });
        ronoctua_belongings::secret::write_private(
            &places.plan(),
            &serde_json::to_vec(&plan).unwrap(),
        )
        .unwrap();
        let path = "config/theme.json".to_owned();
        let t = texts("en-GB");
        let r = Readings::read(Some(places.clone()), r.now_s);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let text = drawn(&v.rows(&r, &zone(), 100, &t));
        for needle in ["\"paper\"", "\"cyber\"", "\"owl\"", "/size"] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
        assert!(
            text.contains(&t.t(key::BELONGINGS_FIELD_NONE)),
            "a deleted value: {text}"
        );
        // `C-r` ON A FIELD decides the field.
        stand_on(
            &mut v,
            &r,
            &Target::Field(path.clone(), "/theme".into()),
            &t,
        );
        assert_eq!(
            v.press(Press::Decide(Decision::TakeRemote), &r, &zone(), 100, &t),
            Errand::Nothing
        );
        assert_eq!(
            v.press(Press::Decide(Decision::KeepBoth), &r, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_FIELD_ONLY_TWO))
        );
        let file = DecisionsFile::read(&places.decisions()).unwrap();
        assert_eq!(file.field(&path, "/theme"), Some(Decision::TakeRemote));
        assert!(!file.paths.contains_key(&path));
        let r = Readings::read(Some(places.clone()), r.now_s);
        stand_on(
            &mut v,
            &r,
            &Target::DecideField(path.clone(), "/size".into(), Decision::KeepLocal),
            &t,
        );
        v.press(Press::Enter, &r, &zone(), 100, &t);
        let r = Readings::read(Some(places.clone()), r.now_s);
        let c = &r.plan.as_ref().unwrap().conflicts[0];
        assert!(
            r.settled(c),
            "every field decided is not a decided conflict"
        );
        assert!(
            drawn(&v.rows(&r, &zone(), 100, &t)).contains(&t.t(key::BELONGINGS_DECIDED_FIELDS))
        );
        // THE OTHER CONFLICT, and the sync applied: the fields for the CLI.
        stand_on(
            &mut v,
            &r,
            &Target::Decide("home/notes/todo.md".into(), Decision::KeepLocal),
            &t,
        );
        v.press(Press::Enter, &r, &zone(), 100, &t);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            v.command("belongings-apply", &r, &zone(), 100, &t),
            Errand::Queue(Op::Apply)
        );
        let file = DecisionsFile::read(&places.decisions()).unwrap();
        assert!(
            r.plan.as_ref().unwrap().settled(&file),
            "the CLI would refuse {file:?}"
        );
        assert!(
            !file.paths.contains_key(&path),
            "a path's decision goes over its fields"
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a target with one path taken out here reads "not on this
    /// machine", or does not offer both "keep it here again" and "not on
    /// this machine"; or one with every path out still offers to take it out.
    #[test]
    fn a_target_partly_taken_out_says_so_and_offers_both() {
        let places = scratch("partly");
        ready(&places);
        put(&places, "home/.gitconfig", b"[user]\n");
        put(&places, "home/.config/git/ignore", b"*.o\n");
        store_of(&places)
            .write(&Belongings {
                excluded: vec!["home/.gitconfig".into()],
                ..store_of(&places).read()
            })
            .expect("prefs");
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        let v = View::fresh(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        let git = drawn(std::slice::from_ref(row_of(
            &rows,
            &Target::Fact("target:git".into()),
        )));
        assert!(
            git.contains(&t.t(key::BELONGINGS_TARGET_PARTLY_HERE)),
            "{git}"
        );
        row_of(&rows, &Target::HereAgain("git"));
        row_of(&rows, &Target::NotHere("git"));
        row_of(&rows, &Target::Nowhere("git"));
        // EVERY PATH OUT: "not on this machine", and only "keep it again".
        store_of(&places)
            .write(&Belongings {
                excluded: target_paths("git")
                    .iter()
                    .map(|p| (*p).to_owned())
                    .collect(),
                ..store_of(&places).read()
            })
            .expect("prefs");
        let r = counted(&places, 1_700_000_500);
        let rows = v.rows(&r, &zone(), 100, &t);
        let git = drawn(std::slice::from_ref(row_of(
            &rows,
            &Target::Fact("target:git".into()),
        )));
        assert!(git.contains(&t.t(key::BELONGINGS_HERE_NOT_HERE)), "{git}");
        row_of(&rows, &Target::HereAgain("git"));
        assert!(
            !rows
                .iter()
                .any(|row| row.target == Some(Target::NotHere("git"))),
            "a target all out offers to take it out"
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a file the last sync left and deleted here is not a row of
    /// the listing, in the colour of a state to look at, saying it leaves
    /// the repository — or its `RET` does not stop keeping it (the
    /// repository's copy then stays for the other machines).
    #[test]
    fn a_file_deleted_here_is_a_row_that_says_it_leaves() {
        let places = scratch("deleted");
        ready(&places);
        put(&places, "config/theme.json", b"{}");
        let mut last = Last::read(&places.last()).expect("last");
        last.entries.insert(
            "config/keyboards.json".into(),
            ronoctua_belongings::status::Known {
                size_bytes: 31,
                ..Default::default()
            },
        );
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&last).expect("last"),
        )
        .expect("last written");
        let target = Target::File("config/keyboards.json".into(), FileAct::StopKeeping);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = counted(&places, 1_700_000_500);
            let v = View::fresh(&r, &zone(), 100, &t);
            let rows = v.rows(&r, &zone(), 100, &t);
            let row = row_of(&rows, &target);
            let said = t.t_with(
                key::BELONGINGS_ITEM,
                &[
                    ("state", &t.t(key::BELONGINGS_STATE_DELETED)),
                    ("size", &bytes_text(31, &t)),
                ],
            );
            assert!(
                drawn(std::slice::from_ref(row)).contains(&said),
                "{language}"
            );
            assert!(
                row.fields
                    .iter()
                    .any(|f| f.color == tokens::color::STATE_WARNING),
                "{language}: a deletion is drawn as nothing to look at"
            );
        }
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &target, &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        assert!(
            store_of(&places)
                .read()
                .excluded
                .contains(&"config/keyboards.json".to_owned())
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a field's value of a JSON conflict wraps over several rows
    /// or runs past the door's width instead of being elided to the room its
    /// row has — at any width — or "chosen" is cut off with it.
    #[test]
    fn a_fields_value_takes_the_room_its_row_has() {
        let places = scratch("long-field");
        let r = waiting(&places);
        let mut plan = r.plan.clone().unwrap();
        let long = format!(
            "[{}]",
            (0..400)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        plan.conflicts[0].merge = Some(Merging::Json {
            fields: vec![ronoctua_belongings::plan::FieldConflict {
                pointer: "/list".into(),
                base: Some(long.clone()),
                ours: Some(long),
                theirs: Some("[]".into()),
            }],
        });
        ronoctua_belongings::secret::write_private(
            &places.plan(),
            &serde_json::to_vec(&plan).unwrap(),
        )
        .unwrap();
        let path = "config/theme.json".to_owned();
        let mut file = DecisionsFile::default();
        file.fields
            .entry(path.clone())
            .or_default()
            .insert("/list".into(), Decision::KeepLocal);
        ronoctua_belongings::secret::write_private(
            &places.decisions(),
            &serde_json::to_vec(&file).unwrap(),
        )
        .unwrap();
        let t = texts("en-GB");
        let r = Readings::read(Some(places.clone()), r.now_s);
        for columns in [60_u16, 100, 240] {
            let v = View::fresh(&r, &zone(), columns, &t);
            let rows = v.rows(&r, &zone(), columns, &t);
            let ours = Target::DecideField(path.clone(), "/list".into(), Decision::KeepLocal);
            let at = rows
                .iter()
                .position(|row| row.target.as_ref() == Some(&ours))
                .expect("the field's row");
            let row = &rows[at];
            let end = row
                .fields
                .iter()
                .map(|f| f.at + layout::width(&f.text))
                .max()
                .unwrap_or(0);
            assert!(end <= columns, "{columns}: the value runs to {end}");
            let text = drawn(std::slice::from_ref(row));
            assert!(text.contains('…'), "{columns}: not elided: {text}");
            let chosen = t.t_with(key::BELONGINGS_FIELD_CHOSEN, &[("value", "")]);
            assert!(text.contains(chosen.trim()), "{columns}: {text}");
            // ONE ROW: the next is the other side's value, not the rest.
            assert!(
                rows.get(at + 1).is_some_and(|n| n.target.is_some()),
                "{columns}: the value wrapped"
            );
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a repair of the whole file names a pointer `/` (a member
    /// called "" at the root) instead of saying the whole file.
    #[test]
    fn a_repair_of_the_whole_file_says_so() {
        let places = scratch("whole");
        ready(&places);
        std::fs::write(places.config.join("thing.json"), b"[1, 2]").unwrap();
        let _ = thing_store(&places).read();
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        assert_eq!(r.repairs.len(), 1, "the ledger is not read");
        let text = drawn(&View::fresh(&r, &zone(), 100, &t).rows(&r, &zone(), 100, &t));
        assert!(
            text.contains(&t.t(key::BELONGINGS_REPAIR_WHOLE_FILE)),
            "{text}"
        );
        assert!(!text.contains("/: "), "{text}");
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the cursor on an offer's answer follows its PLACE when a
    /// sync reorders the offers — `RET` then turns on another offer than the
    /// one the person read.
    #[test]
    fn an_offers_answer_is_the_offer_and_not_its_place() {
        let places = scratch("offer-order");
        ready(&places);
        let key_offer = Offer::SshSigning {
            key: "home/.ssh/id_ed25519.pub".into(),
        };
        status(
            &places,
            synced(vec![Offer::PamU2f, key_offer.clone()], None),
        );
        let t = texts("en-GB");
        let r = counted(&places, 1_700_000_500);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &Target::Offer(Offer::PamU2f, true), &t);
        status(&places, synced(vec![key_offer, Offer::PamU2f], None));
        let r = counted(&places, 1_700_000_500);
        v.settle_in(&r, &zone(), 100, &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Accept(1_700_000_100, Offer::PamU2f)
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: drawing the page formats the count's files again on every
    /// call in one language, or keeps one language's rows for another.
    #[test]
    fn a_counts_rows_are_formatted_once_per_language() {
        let places = scratch("formatted");
        ready(&places);
        put(&places, "config/theme.json", b"{}");
        let r = counted(&places, 1_700_000_500);
        let count = r.count.as_ref().expect("counted");
        let (en, pt) = (texts("en-GB"), texts("pt-BR"));
        let first = count.formatted(r.places.as_ref(), &en);
        let _ = View::fresh(&r, &zone(), 100, &en).rows(&r, &zone(), 100, &en);
        assert!(Arc::ptr_eq(
            &first,
            &count.formatted(r.places.as_ref(), &en)
        ));
        let other = count.formatted(r.places.as_ref(), &pt);
        assert!(!Arc::ptr_eq(&first, &other), "pt-BR drew en-GB's rows");
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a root of the walk that is a folder is not heard as a tree
    /// (a file changed deep in it), one that is a file is not heard by its
    /// folder, one not made yet is not heard from the nearest folder that is
    /// — or a folder inside a tree is heard twice.
    #[test]
    fn what_the_listing_is_made_of_is_heard() {
        let places = scratch("heard");
        ready(&places);
        put(&places, "home/.ssh/config", b"Host *\n");
        put(&places, "home/.gitconfig", b"[user]\n");
        put(&places, "config/studio/scenes.json", b"{}");
        let r = Readings::read(Some(places.clone()), 1_700_000_500);
        let heard = Heard::of(r.scope(), &places);
        let ssh = places.resolve("home/.ssh").unwrap();
        assert!(heard.trees.contains(&ssh), "{heard:?}");
        assert!(heard.trees.contains(&places.config), "{heard:?}");
        assert!(heard.dirs.contains(&places.home), "{heard:?}");
        // `~/.config/doom/local.el` is not there: `~/.config` is heard.
        let dot_config = places.resolve("home/.config").unwrap();
        assert!(heard.dirs.contains(&dot_config), "{heard:?}");
        assert!(
            !heard
                .dirs
                .iter()
                .any(|d| heard.trees.iter().any(|t| d.starts_with(t))),
            "a folder inside a tree is heard twice: {heard:?}"
        );
        assert!(heard.dirs.len() <= HEARD_MOST && heard.trees.len() <= HEARD_MOST);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a file changed deep in what is kept (`~/.ssh/config`) or
    /// beside a root that is a file (`~/.gitconfig`) rings no door — the row
    /// would say "in step" until something else rang — or a count that finds
    /// the same folders raises a second notice over them.
    #[test]
    fn a_change_in_what_is_kept_rings_the_door() {
        let places = scratch("rings");
        ready(&places);
        put(&places, "home/.ssh/config", b"Host *\n");
        put(&places, "home/.gitconfig", b"[user]\n");
        let r = counted(&places, 1_700_000_500);
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let mut ears = Ears::default();
        ears.follow(&r, move || tx.send(()).is_ok());
        assert!(ears.notice.is_some(), "nothing is heard");
        let heard_after = |logical: &str| {
            // THE WATCHES ARE ADDED BY THE NOTICE'S THREAD: an edit before
            // it is ready is not an edit missed by the door, so it is made
            // again until it rings, for a few seconds at most.
            while rx.try_recv().is_ok() {}
            (0..25).any(|n| {
                put(&places, logical, format!("# edit {n}\n").as_bytes());
                rx.recv_timeout(std::time::Duration::from_millis(200))
                    .is_ok()
            })
        };
        assert!(
            heard_after("home/.ssh/config"),
            "a file in ~/.ssh rang nothing"
        );
        assert!(heard_after("home/.gitconfig"), "~/.gitconfig rang nothing");
        // THE SAME FOLDERS: the notice stays — the first ring is still the
        // one told, never the one a second notice would carry.
        let (tx2, rx2) = std::sync::mpsc::channel::<()>();
        let again = counted(&places, 1_700_000_500);
        ears.follow(&again, move || tx2.send(()).is_ok());
        assert!(heard_after("home/.ssh/config"), "the notice went");
        assert!(
            rx2.try_recv().is_err(),
            "a second notice over the same folders"
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a count asked while one runs starts a second walk at once
    /// instead of one more after it, or a count that came back is not the
    /// listing.
    #[test]
    fn one_count_at_a_time_and_one_more_after_it() {
        let places = scratch("counting");
        ready(&places);
        put(&places, "config/theme.json", b"{}");
        let inbox = crate::region::Inbox::new(crate::wake::Wake::new(|| {}));
        let mut counting = Counting::through(inbox.post());
        let mut r = Readings::read(Some(places.clone()), 1);
        counting.ask(&mut r);
        counting.ask(&mut r);
        assert!(r.count.is_none(), "a thread's count was waited for");
        let next = || loop {
            let mut came = inbox.take();
            if let Some(Came::Counted(items)) = came.pop() {
                assert!(came.is_empty(), "two walks at once");
                break items;
            }
            std::thread::yield_now();
        };
        let first = next();
        assert!(inbox.take().is_empty(), "two walks at once");
        counting.came(&mut r, first);
        assert!(r.items().iter().any(|i| i.path == "config/theme.json"));
        // THE ONE ASKED WHILE IT RAN, after it.
        let second = next();
        counting.came(&mut r, second);
        assert!(inbox.take().is_empty());
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }
}
