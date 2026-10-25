//! THE MACHINE'S PARTS — a page of the update's view, in both doors.
//!
//! # What it shows
//!
//! What arrives after the installation, each part on its own
//! (`ronoctua_parts`, `nix/modules/parts.nix`): one row per part the system
//! wants, with where it stands — here, on its way, tried and failed with the
//! next try's time, waiting for a network, or needing the person. On top, for
//! the row under the cursor, why the last try failed and each download it is
//! missing, with the hosts it comes from and the ways it can come in besides.
//! `d` turns the view to this page and back, in the footer's region
//! (`LEADER s u u`, or straight here with `LEADER s u d`) and in the window
//! (`LEADER s u w`), with the same rows and the same keys.
//!
//! THE SYSTEM ITSELF is the first row when its last update could not fetch
//! something (`ronoctua-machine apply`, `last-apply.missing` in the
//! machine's record): the update that failed for want of a download, or the
//! hardware that waits for a network, built without again. Its downloads
//! come in the same ways as a part's, and a download already in the store
//! says so: the next update builds with it.
//!
//! # What the person does here
//!
//! - `RET` on the system updates again — the update's own start, as `u`
//!   does on the steps; `h` hands in a file for it, as for a part.
//! - `RET` on a part tries it NOW: its realizer, started through the one
//!   narrow polkit rule (`start`, these units, this user), never waited for.
//!   On a metered link a part waits, and this is the person's yes to that
//!   one try (`ronoctua_parts::names::YES`).
//! - `h` on a part hands in a file the person downloaded elsewhere — a
//!   browser, another computer — for the download it is missing: the file
//!   picker (`crate::file_picker`, the one way this system asks for a file)
//!   comes up over the page, the page puts a copy of the chosen file in the
//!   person's hand-in folder and starts root's `accept`, which takes it only
//!   when Nix finds the hash the flake pins.
//! - Two rows carry downloads between machines on a stick: bring in what the
//!   parts are missing (`import`, downloads only — anything built is refused
//!   by Nix) or write what this machine keeps (`export`). The picker asks for
//!   the stick's folder — the mounted devices are its places —, and root goes
//!   only to a removable medium.
//! - The last row names the person's mirrors (`download-mirrors` in their
//!   options, `ronoctua_person::options`): servers root's realizer asks when
//!   a download's origin fails, taking only the bytes the flake pins. They
//!   travel signed with the person's options and reach a machine at its next
//!   update; naming them is the consent for their hosts.
//!
//! The gestures that wait — a hand-in, a stick — run on a NAMED thread and
//! their verdict comes back to the door that asked ([`carry`], [`said_of`]):
//! root's words are files ([`ronoctua_parts::record`],
//! [`ronoctua_parts::stick`]), never the journal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::key;
use ronoctua_parts::names;
use ronoctua_parts::record::{Phase, Record, Waiting, Why};
use ronoctua_parts::stick::{Outcome, Refusal, Said, Verb};
use ronoctua_tokens as tokens;

use crate::binary_cache::{PageLine, TopLine};

/// The command that opens the update's view on this page (`LEADER s u d`).
pub const OPEN: &str = "distro-parts-open";

/// The question of a file handed in — a command, so the minibuffer's answer
/// comes back by name.
pub const HAND_IN: &str = "distro-parts-hand-in";
/// The question of the stick's folder to bring downloads in from.
pub const IMPORT: &str = "distro-parts-import";
/// The question of the stick's folder to write the downloads on.
pub const EXPORT: &str = "distro-parts-export";
/// The question of the person's mirrors.
pub const MIRRORS: &str = "distro-parts-mirrors";

/// `systemctl`, baked by the build (`nix/rust.nix`).
const SYSTEMCTL: Option<&str> = option_env!("RONOCTUA_SYSTEMCTL");

// ---------------------------------------------------------------------------
// WHAT THE PAGE READS
// ---------------------------------------------------------------------------

/// Where the page reads — root's files, under the footer camera's room while
/// it photographs — and where the person's sticks are mounted, where the
/// picker opens for a stick.
#[derive(Debug, Clone)]
pub struct Places {
    /// The parts' records.
    pub records: PathBuf,
    /// The system's plan, for the parts' names.
    pub plan: PathBuf,
    /// What the last stick gesture did.
    pub stick: PathBuf,
    /// Where the person's removable media are mounted (`/run/media/<user>`).
    pub media: Option<PathBuf>,
    /// The person's copy of their options, which names their mirrors.
    pub options: Option<PathBuf>,
    /// The person's config root, whose `consent.json` lets the mirrors'
    /// hosts.
    pub config: Option<PathBuf>,
    /// The machine's record (`state.json`), whose last apply names what the
    /// system could not fetch.
    pub state: PathBuf,
}

impl Places {
    /// This machine's.
    #[must_use]
    pub fn here() -> Self {
        let machine = |p: &str| match crate::photo::room() {
            Some(room) => room.join(p.trim_start_matches('/')),
            None => PathBuf::from(p),
        };
        Self {
            records: machine(names::RECORDS),
            plan: machine(names::PLAN),
            stick: machine(names::STICK_SAID),
            media: std::env::var_os("USER").map(|u| Path::new("/run/media").join(u)),
            options: ronoctua_xdg::state_dir().map(|d| {
                d.join(ronoctua_person::names::MIRROR)
                    .join(ronoctua_person::names::OPTIONS)
            }),
            config: ronoctua_xdg::config_dir(),
            state: crate::update_window::record_places()
                .machine_state
                .join(ronoctua_person::names::STATE),
        }
    }

    /// When each thing the page reads last moved — what a door compares to
    /// read again only when something did. A record renamed into place moves
    /// its folder.
    #[must_use]
    pub fn stamp(&self) -> Vec<Option<std::time::SystemTime>> {
        let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        [
            Some(self.records.clone()),
            Some(self.plan.clone()),
            Some(self.stick.clone()),
            self.media.clone(),
            self.options.clone(),
        ]
        .iter()
        .map(|p| p.as_deref().and_then(mtime))
        .collect()
    }
}

/// A download a part — or the system — is missing, as the page says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// Its name — the name a file handed in must have.
    pub name: String,
    /// The hosts its addresses name.
    pub hosts: Vec<String>,
    /// Whether it is one file the person can hand in.
    pub by_hand: bool,
    /// Whether the store holds it now — brought in since the record was
    /// written, which only the next build reads.
    pub here: bool,
}

impl Missing {
    /// `d` as the page says it, `here` or not.
    fn of(d: &ronoctua_person::frontier::Download, here: bool) -> Self {
        Self {
            name: d.name.clone(),
            hosts: d.hosts(),
            by_hand: d
                .means()
                .contains(&ronoctua_person::frontier::Means::HandIn),
            here,
        }
    }
}

/// What the machine's last update could not fetch, as the page draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System {
    /// Whether its build failed for it; otherwise the machine was built
    /// without the hardware that waits for a network.
    pub failed: bool,
    /// The downloads.
    pub missing: Vec<Missing>,
}

impl System {
    /// What the machine's `last` apply says, each download `here` or not;
    /// `None` when it was missing nothing. Pure, but for `here`.
    #[must_use]
    pub fn of(
        last: Option<&ronoctua_person::state::LastApply>,
        here: impl Fn(&str) -> bool,
    ) -> Option<Self> {
        let last = last.filter(|a| !a.missing.is_empty())?;
        Some(Self {
            failed: last.result == ronoctua_person::state::ApplyResult::BuildFailed,
            missing: last
                .missing
                .iter()
                .map(|d| Missing::of(d, here(&d.out)))
                .collect(),
        })
    }

    /// The downloads still to come.
    fn to_come(&self) -> impl Iterator<Item = &Missing> {
        self.missing.iter().filter(|m| !m.here)
    }
}

/// A part, as the page draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// Its id — what its realizer's unit is named by.
    pub id: String,
    /// What the page calls it: the program's own name (data).
    pub name: String,
    /// Where it stands.
    pub phase: Phase,
    /// What it waits for, while it waits.
    pub waiting_for: Option<Waiting>,
    /// Failed tries since it last arrived.
    pub tries: u32,
    /// When the next try is due, in seconds since the epoch.
    pub due: Option<i64>,
    /// Why the last try failed, and what `nix` said.
    pub failure: Option<(Why, String)>,
    /// The downloads it is missing.
    pub missing: Vec<Missing>,
}

/// What the person's copy of their options says of their mirrors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mirrors {
    /// These, in the order they are asked — none, or no file yet.
    Named(Vec<String>),
    /// The file is there and does not read: never rewritten.
    Unreadable,
    /// The file is of a newer format than this machine reads.
    Newer,
}

impl Default for Mirrors {
    fn default() -> Self {
        Self::Named(Vec::new())
    }
}

impl Mirrors {
    /// What the person's copy at `path` says.
    #[must_use]
    pub fn read(path: &Path) -> Self {
        match std::fs::read(path) {
            Err(_) => Self::default(),
            Ok(bytes) => {
                match ronoctua_person::parse::<ronoctua_person::options::Options>(&bytes) {
                    Ok(o) => Self::Named(o.download_mirrors),
                    Err(r) if r.why == ronoctua_person::Why::Newer => Self::Newer,
                    Err(_) => Self::Unreadable,
                }
            }
        }
    }
}

/// Everything the page draws.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Readings {
    /// What the system's last update could not fetch, when something.
    pub system: Option<System>,
    /// Every part the system wants, in id order.
    pub parts: Vec<Part>,
    /// What the last stick gesture did.
    pub stick: Option<Said>,
    /// The person's mirrors.
    pub mirrors: Mirrors,
}

impl Readings {
    /// What `places` say now.
    #[must_use]
    pub fn read(places: &Places) -> Self {
        let names: BTreeMap<String, String> = ronoctua_parts::plan::read(&places.plan)
            .map(|plan| plan.parts.into_iter().map(|(id, w)| (id, w.name)).collect())
            .unwrap_or_default();
        let mut r = Self::of(
            ronoctua_parts::record::read_all(&places.records),
            &names,
            ronoctua_parts::stick::read(&places.stick),
        );
        r.mirrors = places
            .options
            .as_deref()
            .map_or_else(Mirrors::default, Mirrors::read);
        let state = std::fs::read(&places.state)
            .ok()
            .and_then(|b| ronoctua_person::parse::<ronoctua_person::state::MachineState>(&b).ok());
        r.system = System::of(state.as_ref().and_then(|s| s.last_apply.as_ref()), |out| {
            Path::new(out).exists()
        });
        r
    }

    /// The readings of these records, named by `names` (the id where it says
    /// none) — what a photograph builds. Pure.
    #[must_use]
    pub fn of(records: Vec<Record>, names: &BTreeMap<String, String>, stick: Option<Said>) -> Self {
        let parts = records
            .into_iter()
            // A PART THE SYSTEM NO LONGER WANTS is no row: nothing to do.
            .filter(|r| r.phase != Phase::Unwanted)
            .map(|r| Part {
                name: names.get(&r.id).cloned().unwrap_or_else(|| r.id.clone()),
                phase: r.phase,
                waiting_for: r.waiting_for,
                tries: r.attempts,
                due: r.due_at(),
                failure: r.last_failure.as_ref().map(|f| (f.why, f.detail.clone())),
                missing: r.missing.iter().map(|d| Missing::of(d, false)).collect(),
                id: r.id,
            })
            .collect();
        Self {
            system: None,
            parts,
            stick,
            mirrors: Mirrors::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// THE PAGE
// ---------------------------------------------------------------------------

/// One row of the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// What the system's last update could not fetch.
    System,
    /// A part, by its index.
    Part(usize),
    /// Bring downloads in from a stick.
    Import,
    /// Write this machine's downloads on a stick.
    Export,
    /// The person's mirrors.
    Mirrors,
}

/// The rows `r` has: the system's when it is missing something, one per
/// part, then the stick's two and the mirrors'. Pure.
#[must_use]
pub fn rows_of(r: &Readings) -> Vec<Row> {
    r.system
        .as_ref()
        .map(|_| Row::System)
        .into_iter()
        .chain((0..r.parts.len()).map(Row::Part))
        .chain([Row::Import, Row::Export, Row::Mirrors])
        .collect()
}

/// A key the page reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The row above.
    Up,
    /// The row below.
    Down,
    /// Does what the row says.
    Enter,
    /// Hands in a file for the part, or the system, under the cursor.
    HandIn,
    /// Back to the steps.
    Escape,
}

/// Something the person asked the machine to do — carried out on a thread
/// ([`carry`]), its verdict said on the door that asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Try the part now.
    Retry {
        /// Its id.
        id: String,
        /// What the page calls it.
        name: String,
    },
    /// Hand in the file at `from` as the part's download `download`.
    HandIn {
        /// The part's id ([`names::SYSTEM`] for the system's).
        id: String,
        /// The download's name.
        download: String,
        /// The file the person named.
        from: PathBuf,
    },
    /// Bring downloads in from the stick folder named.
    Import(PathBuf),
    /// Write this machine's downloads on the stick folder named.
    Export(PathBuf),
}

/// Why the mirrors typed were not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotMirrors {
    /// One is not a web address a mirror may have.
    NotOne,
    /// More than a file may name.
    TooMany,
}

/// The mirrors as typed on the line — one or more addresses, separated by
/// spaces; nothing typed names none. Duplicates are kept once, in order.
/// Pure.
///
/// # Errors
///
/// [`NotMirrors`].
pub fn mirrors_of(typed: &str) -> Result<Vec<String>, NotMirrors> {
    let mut out: Vec<String> = Vec::new();
    for url in typed.split_whitespace() {
        if !ronoctua_person::options::is_mirror(url) {
            return Err(NotMirrors::NotOne);
        }
        if !out.iter().any(|u| u == url) {
            out.push(url.to_owned());
        }
    }
    if out.len() > ronoctua_person::options::MIRRORS_MOST {
        return Err(NotMirrors::TooMany);
    }
    Ok(out)
}

impl Act {
    /// What a photograph prints instead of doing it.
    #[must_use]
    pub fn word(&self) -> String {
        match self {
            Self::Retry { id, .. } => format!("try the part {id} now"),
            Self::HandIn { id, download, .. } if id == names::SYSTEM => {
                format!("hand in {download} for the system")
            }
            Self::HandIn { id, download, .. } => format!("hand in {download} for the part {id}"),
            Self::Import(from) => format!("bring downloads in from {}", from.display()),
            Self::Export(to) => format!("write the downloads on {}", to.display()),
        }
    }
}

/// What the page asks of the door that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing.
    Nothing,
    /// The cursor moved: the page follows, the register says the row.
    Moved,
    /// Back to the steps.
    Close,
    /// A sentence, by its key, said on the door's line.
    Say(&'static str),
    /// A question on the door's line — the mirrors'; the answer comes back
    /// to [`Page::answered`].
    Ask(crate::line::Line),
    /// The file picker, up over the page, for this ([`request_of`]); what
    /// it picks comes back to [`picked`].
    Pick(Picking),
    /// What the picker said — a folder it could not read —, said on the
    /// door's line.
    Explain(ronoctua_files::mode::Explanation),
    /// `C-v` in the picker: the door reads the clipboard and hands it over.
    Paste,
    /// Something for the machine to do ([`carry`]).
    Run(Act),
    /// The person's mirrors to be these ([`name_mirrors`]).
    Mirrors(Vec<String>),
    /// The whole update again — the update's own start, as `u` on its
    /// steps, which the page gives way to.
    Update,
}

/// What the file picker is up for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picking {
    /// The file the person downloaded elsewhere, for a part's download.
    HandIn {
        /// The part's id ([`names::SYSTEM`] for the system's).
        id: String,
        /// The download's name — what the file is checked as.
        download: String,
    },
    /// The stick to bring downloads in from.
    Import,
    /// The stick to write the downloads on.
    Export,
}

/// What the picker is asked for `picking`: a file, opening in the downloads
/// folder; or a folder, opening where the person's sticks are mounted
/// (`media`) when it is there.
#[must_use]
pub fn request_of(picking: &Picking, media: Option<&Path>) -> crate::file_picker::Request {
    use crate::file_picker::{EVERY_FILE, Mode, Request, Start, Xdg};
    let stick = || match media.filter(|m| m.is_dir()) {
        Some(m) => Start::At(m.to_path_buf()),
        None => Start::Remembered,
    };
    let (purpose, mode, start) = match picking {
        Picking::HandIn { .. } => (
            "distro-parts-hand-in",
            Mode::Open { many: false },
            Start::Place(Xdg::DOWNLOAD),
        ),
        Picking::Import => ("distro-parts-import", Mode::Folder, stick()),
        Picking::Export => ("distro-parts-export", Mode::Folder, stick()),
    };
    Request {
        purpose,
        mode,
        filters: EVERY_FILE,
        start,
        addresses: false,
        extras: &[],
        alternate: None,
    }
}

/// The title the picker's rows go under.
#[must_use]
pub const fn title_of(picking: &Picking) -> &'static str {
    match picking {
        Picking::HandIn { .. } => key::DISTRO_PARTS_PICK_HAND_IN,
        Picking::Import => key::DISTRO_PARTS_PICK_IMPORT,
        Picking::Export => key::DISTRO_PARTS_PICK_EXPORT,
    }
}

/// What the picker's answer does: the file handed in, the stick read or
/// written. Pure.
#[must_use]
pub fn picked(picking: Picking, paths: &[PathBuf]) -> Errand {
    let Some(path) = paths.first().cloned() else {
        return Errand::Nothing;
    };
    Errand::Run(match picking {
        Picking::HandIn { id, download } => Act::HandIn {
            id,
            download,
            from: path,
        },
        Picking::Import => Act::Import(path),
        Picking::Export => Act::Export(path),
    })
}

/// The lines on top of the page while the picker is up: what to choose,
/// wrapped.
#[must_use]
pub fn picking_lines(picking: &Picking, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<TopLine> {
    let said = match picking {
        Picking::HandIn { download, .. } => texts.t_with(
            key::DISTRO_PARTS_ABOUT_PICK_HAND_IN,
            &[("name", download.as_str())],
        ),
        Picking::Import => texts.t(key::DISTRO_PARTS_ABOUT_IMPORT),
        Picking::Export => texts.t(key::DISTRO_PARTS_ABOUT_EXPORT),
    };
    crate::chrome::note_lines(&said, room)
        .into_iter()
        .map(|text| TopLine {
            text,
            color: tokens::color::TEXT_SECONDARY,
            answer: None,
        })
        .collect()
}

/// The page: its cursor, and whether the mirrors' question is open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    cursor: usize,
    asking_mirrors: bool,
}

impl Page {
    /// The row under the cursor, by its index.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The cursor kept on a row `r` has.
    pub fn settle(&mut self, r: &Readings) {
        self.cursor = self.cursor.min(rows_of(r).len().saturating_sub(1));
    }

    /// One key — the same in both doors.
    pub fn press(&mut self, press: Press, r: &Readings) -> Errand {
        let len = rows_of(r).len();
        match press {
            Press::Down => {
                self.cursor = (self.cursor + 1).min(len.saturating_sub(1));
                Errand::Moved
            }
            Press::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Errand::Moved
            }
            Press::Enter => self.enter(r),
            Press::HandIn => self.hand_in(r),
            Press::Escape => Errand::Close,
        }
    }

    /// Whether a click takes row `n` — and so whether a resting pointer
    /// lights it, in both doors: every row of the page (a click puts the
    /// cursor there, and its register says what it is).
    #[must_use]
    pub fn takes(&self, n: usize, r: &Readings) -> bool {
        n < rows_of(r).len()
    }

    /// A click on the row `n`: the first puts the cursor there, the second
    /// does what `RET` does (invariant 19).
    pub fn clicked(&mut self, n: usize, r: &Readings) -> Errand {
        if !self.takes(n, r) {
            return Errand::Nothing;
        }
        if n == self.cursor {
            self.enter(r)
        } else {
            self.cursor = n;
            Errand::Moved
        }
    }

    /// `RET` on the row under the cursor: the system is updated again; a
    /// part is tried now; a stick's row asks for the stick's folder.
    fn enter(&mut self, r: &Readings) -> Errand {
        match rows_of(r).get(self.cursor) {
            Some(Row::System) => Errand::Update,
            Some(Row::Part(i)) => match r.parts.get(*i) {
                Some(p) if p.phase == Phase::Here => {
                    Errand::Say(key::DISTRO_PARTS_SAID_ALREADY_HERE)
                }
                Some(p) => Errand::Run(Act::Retry {
                    id: p.id.clone(),
                    name: p.name.clone(),
                }),
                None => Errand::Nothing,
            },
            Some(Row::Import) => Errand::Pick(Picking::Import),
            Some(Row::Export) => Errand::Pick(Picking::Export),
            // A COPY THAT DOES NOT READ IS NEVER REWRITTEN, and says why.
            Some(Row::Mirrors) => match &r.mirrors {
                Mirrors::Unreadable => Errand::Say(key::DISTRO_PARTS_WHY_UNREADABLE),
                Mirrors::Newer => Errand::Say(key::DISTRO_PARTS_WHY_NEWER),
                Mirrors::Named(now) => {
                    self.asking_mirrors = true;
                    Errand::Ask(crate::line::Line::new(MIRRORS, false, Some(&now.join(" "))))
                }
            },
            None => Errand::Nothing,
        }
    }

    /// The part (or the system) under the cursor and the first download it
    /// is missing that is one file — or the sentence that says why there is
    /// none to hand in.
    fn to_hand_in(&self, r: &Readings) -> Result<(String, String), &'static str> {
        let (id, missing): (&str, Vec<&Missing>) = match rows_of(r).get(self.cursor).copied() {
            Some(Row::System) => {
                let system = r.system.as_ref().ok_or(key::DISTRO_PARTS_SAID_NOT_A_PART)?;
                (names::SYSTEM, system.to_come().collect())
            }
            Some(Row::Part(i)) => {
                let part = r.parts.get(i).ok_or(key::DISTRO_PARTS_SAID_NOT_A_PART)?;
                (part.id.as_str(), part.missing.iter().collect())
            }
            _ => return Err(key::DISTRO_PARTS_SAID_NOT_A_PART),
        };
        if missing.is_empty() {
            return Err(key::DISTRO_PARTS_SAID_NOTHING_MISSING);
        }
        let download = missing
            .iter()
            .find(|m| m.by_hand)
            .ok_or(key::DISTRO_PARTS_SAID_NOT_ONE_FILE)?;
        Ok((id.to_owned(), download.name.clone()))
    }

    /// `h` on the row under the cursor: the picker, for the file — or why
    /// there is none to hand in.
    fn hand_in(&self, r: &Readings) -> Errand {
        match self.to_hand_in(r) {
            Ok((id, download)) => Errand::Pick(Picking::HandIn { id, download }),
            Err(why) => Errand::Say(why),
        }
    }

    /// The question `id`, asked as its row's key would — a command run by
    /// its name with nothing typed (`LEADER :`).
    pub fn ask_for(&mut self, id: &str, r: &Readings) -> Errand {
        let rows = rows_of(r);
        match id {
            HAND_IN => self.hand_in(r),
            IMPORT | EXPORT | MIRRORS => {
                let wanted = match id {
                    IMPORT => Row::Import,
                    EXPORT => Row::Export,
                    _ => Row::Mirrors,
                };
                if let Some(n) = rows.iter().position(|row| *row == wanted) {
                    self.cursor = n;
                }
                self.enter(r)
            }
            _ => Errand::Nothing,
        }
    }

    /// What was typed on the line, answering the mirrors' question — only
    /// while it is open. Nothing typed names none.
    pub fn answered(&mut self, id: &str, text: &str) -> Errand {
        if id != MIRRORS || !std::mem::take(&mut self.asking_mirrors) {
            return Errand::Nothing;
        }
        match mirrors_of(text.trim()) {
            Ok(named) => Errand::Mirrors(named),
            Err(NotMirrors::NotOne) => Errand::Say(key::DISTRO_PARTS_SAID_NOT_A_MIRROR),
            Err(NotMirrors::TooMany) => Errand::Say(key::DISTRO_PARTS_SAID_TOO_MANY_MIRRORS),
        }
    }

    /// A path typed after one of the picker's commands, run by its name
    /// (`LEADER : distro-parts-import <folder>`): the picker's answer
    /// without the picker — the file handed in for the part under the
    /// cursor, or the stick's folder.
    pub fn typed(&mut self, id: &str, text: &str, r: &Readings) -> Errand {
        let typed = text.trim();
        let path = match typed.strip_prefix("~/") {
            Some(rest) => std::env::var_os("HOME").map_or_else(
                || PathBuf::from(typed),
                |home| PathBuf::from(home).join(rest),
            ),
            None => PathBuf::from(typed),
        };
        if typed.is_empty() {
            return Errand::Nothing;
        }
        match id {
            HAND_IN => match self.to_hand_in(r) {
                Ok((id, download)) => picked(Picking::HandIn { id, download }, &[path]),
                Err(why) => Errand::Say(why),
            },
            IMPORT => picked(Picking::Import, &[path]),
            EXPORT => picked(Picking::Export, &[path]),
            MIRRORS => {
                self.asking_mirrors = true;
                self.answered(MIRRORS, typed)
            }
            _ => Errand::Nothing,
        }
    }

    /// The line was let go.
    pub fn unanswered(&mut self) {
        self.asking_mirrors = false;
    }

    /// The lines on top of the page: what the row under the cursor is, why a
    /// part's last try failed, and each download it is missing with the ways
    /// it can come in. Notes are WRAPPED, never cut.
    #[must_use]
    pub fn top_lines(
        &self,
        r: &Readings,
        room: u16,
        zone: &ronoctua_files::when::Zone,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<TopLine> {
        let Some(row) = rows_of(r).get(self.cursor).copied() else {
            return Vec::new();
        };
        let mut said: Vec<(String, tokens::Color)> = Vec::new();
        let secondary = tokens::color::TEXT_SECONDARY;
        match row {
            Row::System => {
                // THE UPDATE THAT FAILED left the machine as it was; the
                // hardware that waits did not hold the update back.
                let about = match &r.system {
                    Some(system) if !system.failed => key::DISTRO_PARTS_ABOUT_SYSTEM_WAITS,
                    _ => key::DISTRO_PARTS_ABOUT_SYSTEM,
                };
                said.push((texts.t(about), secondary));
                if let Some(system) = &r.system {
                    if system.to_come().next().is_none() {
                        said.push((
                            texts.t(key::DISTRO_PARTS_ABOUT_SYSTEM_ALL_HERE),
                            tokens::color::TEXT_PRIMARY,
                        ));
                    }
                    said.extend(missing_said(&system.missing, texts));
                }
            }
            Row::Part(i) => {
                said.push((texts.t(key::DISTRO_PARTS_ABOUT_PART), secondary));
                if let Some(p) = r.parts.get(i) {
                    // A METERED LINK: why it waits, and what choosing it does.
                    if p.phase == Phase::Waiting && p.waiting_for == Some(Waiting::Metered) {
                        said.push((
                            texts.t(key::DISTRO_PARTS_ABOUT_METERED),
                            tokens::color::TEXT_PRIMARY,
                        ));
                    }
                    if p.phase != Phase::Here
                        && let Some((why, detail)) = &p.failure
                    {
                        said.push((why_said(*why, detail, texts), tokens::color::STATE_WARNING));
                    }
                    said.extend(missing_said(&p.missing, texts));
                }
            }
            Row::Import => said.push((texts.t(key::DISTRO_PARTS_ABOUT_IMPORT), secondary)),
            Row::Export => said.push((texts.t(key::DISTRO_PARTS_ABOUT_EXPORT), secondary)),
            Row::Mirrors => {
                said.push((texts.t(key::DISTRO_PARTS_ABOUT_MIRRORS), secondary));
                let why = match r.mirrors {
                    Mirrors::Unreadable => Some(key::DISTRO_PARTS_WHY_UNREADABLE),
                    Mirrors::Newer => Some(key::DISTRO_PARTS_WHY_NEWER),
                    Mirrors::Named(_) => None,
                };
                if let Some(k) = why {
                    said.push((texts.t(k), tokens::color::STATE_WARNING));
                }
            }
        }
        // THE LAST STICK GESTURE, on the stick's rows: how it ended, whole.
        if matches!(row, Row::Import | Row::Export)
            && let Some(s) = &r.stick
        {
            said.push((stick_said(s, zone, texts), secondary));
        }
        said.into_iter()
            .flat_map(|(text, color)| {
                crate::chrome::note_lines(&text, room)
                    .into_iter()
                    .map(move |text| TopLine {
                        text,
                        color,
                        answer: None,
                    })
            })
            .collect()
    }

    /// The page under the top lines: a heading, and a row a line — the label
    /// and the value, the cursor a colour — and, without one part, a line
    /// that says so.
    #[must_use]
    pub fn lines(
        &self,
        r: &Readings,
        left: u16,
        room: u16,
        zone: &ronoctua_files::when::Zone,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<PageLine> {
        let plain = |text: String, color| PageLine {
            fields: vec![crate::fields::Field {
                at: left,
                text: ronoctua_render::layout::elide_end(&text, room),
                color,
                hit: crate::fields::Hit::Nothing,
                explain: None,
                ink: crate::fields::Ink::Grid,
            }],
            row: None,
        };
        // A LINE OF AIR before the heading, which is how a heading stands
        // out: hierarchy is colour and air, never size.
        let mut out = vec![
            PageLine {
                fields: crate::chrome::AIR,
                row: None,
            },
            plain(
                texts.t(key::DISTRO_PARTS_HEADING),
                tokens::color::TEXT_HEADING,
            ),
        ];
        if r.parts.is_empty() {
            for said in crate::chrome::note_lines(&texts.t(key::DISTRO_PARTS_NONE), room) {
                out.push(plain(said, tokens::color::TEXT_DIM));
            }
        }
        for (n, row) in rows_of(r).into_iter().enumerate() {
            let focused = n == self.cursor;
            let (label, value, tint) = label_and_value(row, r, zone, texts);
            out.push(PageLine {
                fields: crate::chrome::tinted(
                    crate::chrome::field_pair(
                        left,
                        room,
                        &label,
                        &value,
                        crate::chrome::Kind::of(true),
                        focused,
                    ),
                    tint,
                    focused,
                ),
                row: Some(n),
            });
        }
        out
    }
}

/// The key that hands in a file, as the page teaches it.
pub const HAND_KEY: &str = "h";

/// Each download of `missing`, in the page's words, with the ways it can
/// come in — or that it is here now.
fn missing_said(missing: &[Missing], texts: &ronoctua_i18n::Texts) -> Vec<(String, tokens::Color)> {
    missing
        .iter()
        .map(|m| {
            let hosts = m.hosts.join(", ");
            // ONE FILE, with an address, is handed in; what has none (an npm
            // cache, assembled from hundreds) comes on a stick.
            let k = if m.here {
                key::DISTRO_PARTS_ABOUT_MISSING_HERE
            } else if m.by_hand {
                key::DISTRO_PARTS_ABOUT_MISSING_BY_HAND
            } else {
                key::DISTRO_PARTS_ABOUT_MISSING_STICK
            };
            (
                texts.t_with(
                    k,
                    &[
                        ("name", m.name.as_str()),
                        ("hosts", hosts.as_str()),
                        ("keys", HAND_KEY),
                    ],
                ),
                tokens::color::TEXT_PRIMARY,
            )
        })
        .collect()
}

/// A row's label and value, and the colour of a state it says (a part that
/// needs the person), when it says one.
fn label_and_value(
    row: Row,
    r: &Readings,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> (String, String, Option<tokens::Color>) {
    match row {
        Row::System => {
            let Some(system) = &r.system else {
                return (String::new(), String::new(), None);
            };
            let count = system.to_come().count();
            let n = ronoctua_i18n::Arg::Number(i64::try_from(count).unwrap_or(i64::MAX));
            let warning = Some(tokens::color::STATE_WARNING);
            let (value, tint) = match (count, system.failed) {
                (0, _) => (texts.t(key::DISTRO_PARTS_SYSTEM_ALL_HERE), None),
                (_, true) => (
                    texts.t_args(key::DISTRO_PARTS_SYSTEM_FAILED, &[("count", n)]),
                    warning,
                ),
                (_, false) => (
                    texts.t_args(key::DISTRO_PARTS_SYSTEM_WAITS, &[("count", n)]),
                    None,
                ),
            };
            (texts.t(key::DISTRO_PARTS_ROW_SYSTEM), value, tint)
        }
        Row::Part(i) => {
            let Some(p) = r.parts.get(i) else {
                return (String::new(), String::new(), None);
            };
            let when = p
                .due
                .map(|s| crate::chrome::when_text(s, zone, false, texts))
                .unwrap_or_default();
            let tries = ronoctua_i18n::Arg::Number(i64::from(p.tries));
            let (value, tint) = match p.phase {
                Phase::Here => (texts.t(key::DISTRO_PARTS_PHASE_HERE), None),
                Phase::Waiting if p.waiting_for == Some(Waiting::Metered) => {
                    (texts.t(key::DISTRO_PARTS_PHASE_METERED), None)
                }
                Phase::Waiting => (texts.t(key::DISTRO_PARTS_PHASE_WAITING), None),
                Phase::Arriving if p.due.is_some() => (
                    texts.t_args(
                        key::DISTRO_PARTS_PHASE_RETRYING,
                        &[("tries", tries), ("when", ronoctua_i18n::Arg::Text(&when))],
                    ),
                    None,
                ),
                Phase::Arriving => (texts.t(key::DISTRO_PARTS_PHASE_ARRIVING), None),
                Phase::NeedsYou => (
                    texts.t_args(
                        key::DISTRO_PARTS_PHASE_NEEDS_YOU,
                        &[("tries", tries), ("when", ronoctua_i18n::Arg::Text(&when))],
                    ),
                    Some(tokens::color::STATE_WARNING),
                ),
                Phase::Unwanted => (String::new(), None),
            };
            (p.name.clone(), value, tint)
        }
        Row::Import => (
            texts.t(key::DISTRO_PARTS_ROW_IMPORT),
            stick_value(r, Verb::Import, zone, texts),
            None,
        ),
        Row::Export => (
            texts.t(key::DISTRO_PARTS_ROW_EXPORT),
            stick_value(r, Verb::Export, zone, texts),
            None,
        ),
        Row::Mirrors => {
            let warning = Some(tokens::color::STATE_WARNING);
            let (value, tint) = match &r.mirrors {
                Mirrors::Named(none) if none.is_empty() => {
                    (texts.t(key::DISTRO_PARTS_MIRRORS_NONE), None)
                }
                Mirrors::Named(named) => (named.join(" "), None),
                Mirrors::Unreadable => (texts.t(key::DISTRO_PARTS_MIRRORS_UNREADABLE), warning),
                Mirrors::Newer => (texts.t(key::DISTRO_PARTS_MIRRORS_NEWER), warning),
            };
            (texts.t(key::DISTRO_PARTS_ROW_MIRRORS), value, tint)
        }
    }
}

/// A stick row's value: how its last gesture ended — or that there was
/// none.
fn stick_value(
    r: &Readings,
    verb: Verb,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let Some(said) = r.stick.as_ref().filter(|s| s.verb == verb) else {
        return texts.t(key::DISTRO_PARTS_STICK_NEVER);
    };
    let when = crate::chrome::when_text(said.at.unix_s(), zone, false, texts);
    match said.outcome {
        Outcome::Done { count } => texts.t_args(
            key::DISTRO_PARTS_STICK_MOVED,
            &[
                ("count", ronoctua_i18n::Arg::Number(i64::from(count))),
                ("when", ronoctua_i18n::Arg::Text(&when)),
            ],
        ),
        Outcome::Refused { .. } => texts.t_with(
            key::DISTRO_PARTS_STICK_REFUSED_SHORT,
            &[("when", when.as_str())],
        ),
    }
}

/// Why a part's last try failed, in the page's words.
fn why_said(why: Why, detail: &str, texts: &ronoctua_i18n::Texts) -> String {
    // THE LAST LINE NIX SAID that says something: a whole log is the
    // journal's, and the record keeps only its tail.
    let last = detail
        .lines()
        // A BUILDER'S LINE comes quoted (`> `): the words are what it said.
        .map(|l| l.trim().trim_start_matches('>').trim_start())
        .rfind(|l| !l.is_empty() && !l.starts_with("For full logs"))
        .unwrap_or_default();
    match why {
        Why::BuildFailed => texts.t_with(
            key::DISTRO_PARTS_ABOUT_WHY_BUILD_FAILED,
            &[("detail", last)],
        ),
        Why::OutputDiffers => texts.t(key::DISTRO_PARTS_ABOUT_WHY_OUTPUT_DIFFERS),
        Why::ProfileFailed => texts.t(key::DISTRO_PARTS_ABOUT_WHY_PROFILE_FAILED),
        Why::HandInRefused => texts.t_with(
            key::DISTRO_PARTS_ABOUT_WHY_HAND_IN_REFUSED,
            &[("detail", last)],
        ),
    }
}

/// How the last stick gesture ended, in the page's words.
fn stick_said(s: &Said, zone: &ronoctua_files::when::Zone, texts: &ronoctua_i18n::Texts) -> String {
    let when = crate::chrome::when_text(s.at.unix_s(), zone, false, texts);
    match (s.verb, s.outcome) {
        (Verb::Import, Outcome::Done { count }) => texts.t_args(
            key::DISTRO_PARTS_STICK_LAST_IMPORT,
            &[
                ("count", ronoctua_i18n::Arg::Number(i64::from(count))),
                ("when", ronoctua_i18n::Arg::Text(&when)),
            ],
        ),
        (Verb::Export, Outcome::Done { count }) => texts.t_args(
            key::DISTRO_PARTS_STICK_LAST_EXPORT,
            &[
                ("count", ronoctua_i18n::Arg::Number(i64::from(count))),
                ("when", ronoctua_i18n::Arg::Text(&when)),
            ],
        ),
        (_, Outcome::Refused { why }) => format!(
            "{} {}",
            texts.t_with(key::DISTRO_PARTS_STICK_REFUSED, &[("when", when.as_str())]),
            texts.t(refusal_key(why))
        ),
    }
}

/// A stick refusal's sentence.
const fn refusal_key(why: Refusal) -> &'static str {
    match why {
        Refusal::NoFolderNamed => key::DISTRO_PARTS_REFUSED_NO_FOLDER_NAMED,
        Refusal::FolderMissing => key::DISTRO_PARTS_REFUSED_FOLDER_MISSING,
        Refusal::FolderThroughALink => key::DISTRO_PARTS_REFUSED_FOLDER_THROUGH_A_LINK,
        Refusal::NotARemovableMedium => key::DISTRO_PARTS_REFUSED_NOT_A_REMOVABLE_MEDIUM,
        Refusal::NoDownloadsThere => key::DISTRO_PARTS_REFUSED_NO_DOWNLOADS_THERE,
        Refusal::Unwritable => key::DISTRO_PARTS_REFUSED_UNWRITABLE,
    }
}

// ---------------------------------------------------------------------------
// THE MIRRORS — the person's copy of their options
// ---------------------------------------------------------------------------

/// Why the person's copy was not changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unchanged {
    /// It does not read: never rewritten.
    Unreadable,
    /// It is of a newer format: never rewritten.
    Newer,
    /// It could not be written.
    Unwritable,
}

/// Names `mirrors` in the person's copy at `path`, the rest of their options
/// as they were — read with `ronoctua_person::parse`, written whole with
/// `ronoctua_person::to_json`, through a draft.
///
/// # Errors
///
/// [`Unchanged`].
pub fn write_mirrors(path: &Path, mirrors: &[String]) -> Result<(), Unchanged> {
    use ronoctua_person::options::Options;
    let mut options = match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Options::default(),
        Err(_) => return Err(Unchanged::Unreadable),
        Ok(bytes) => ronoctua_person::parse::<Options>(&bytes).map_err(|r| {
            if r.why == ronoctua_person::Why::Newer {
                Unchanged::Newer
            } else {
                Unchanged::Unreadable
            }
        })?,
    };
    options.download_mirrors = mirrors.to_vec();
    ronoctua_person::Document::validate(&options).map_err(|_| Unchanged::Unreadable)?;
    let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
        return Err(Unchanged::Unwritable);
    };
    std::fs::create_dir_all(dir).map_err(|_| Unchanged::Unwritable)?;
    ronoctua_xdg::write_atomically(dir, file, ronoctua_person::to_json(&options).as_bytes())
        .map_err(|_| Unchanged::Unwritable)
}

/// Carries out the mirrors the person typed: their copy of their options
/// and — naming a mirror is the consent for its host — their consent.
/// Answers the sentence the door says.
#[must_use]
pub fn name_mirrors(mirrors: &[String], places: &Places, texts: &ronoctua_i18n::Texts) -> String {
    let Some(path) = places.options.as_deref() else {
        return texts.t(key::DISTRO_PARTS_WHY_UNWRITABLE);
    };
    if let Err(why) = write_mirrors(path, mirrors) {
        return texts.t(match why {
            Unchanged::Unreadable => key::DISTRO_PARTS_WHY_UNREADABLE,
            Unchanged::Newer => key::DISTRO_PARTS_WHY_NEWER,
            Unchanged::Unwritable => key::DISTRO_PARTS_WHY_UNWRITABLE,
        });
    }
    if mirrors.is_empty() {
        return texts.t(key::DISTRO_PARTS_SAID_NO_MIRROR);
    }
    let hosts: Vec<&str> = mirrors
        .iter()
        .filter_map(|m| ronoctua_person::options::mirror_host(m))
        .collect();
    let kept = places
        .config
        .as_deref()
        .is_some_and(|c| ronoctua_consent::let_hosts_under(c, &hosts, true).is_ok());
    texts.t(if kept {
        key::DISTRO_PARTS_SAID_MIRRORS
    } else {
        key::DISTRO_PARTS_SAID_MIRRORS_NOT_LET
    })
}

// ---------------------------------------------------------------------------
// THE GESTURES — on a thread, never the loop that draws
// ---------------------------------------------------------------------------

/// What came of a gesture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Came {
    /// The part's realizer was started.
    Retrying(String),
    /// The machine did not start the unit (polkit, or no `systemctl`).
    NotStarted,
    /// No file is at the path named.
    NoFile,
    /// The file could not be put in the hand-in folder.
    NotCopied,
    /// The download handed in was taken.
    Taken(String),
    /// The system's download handed in was taken: the next update builds
    /// with it.
    TakenForTheSystem(String),
    /// The download handed in was refused: other bytes.
    Refused(String),
    /// What root said of a stick gesture.
    Stick(Said),
    /// The stick's folder could not be named for root.
    NotNamed,
}

/// Carries `act` out — BLOCKING: a hand-in and a stick wait for root's
/// oneshot. Run it on a thread.
#[must_use]
pub fn carry(act: &Act, places: &Places) -> Came {
    match act {
        Act::Retry { id, name } => {
            // THE PERSON'S YES to this one try, on a metered link too.
            if let Err(e) = std::fs::write(Path::new(names::YES).join(id), b"") {
                tracing::warn!(error = %e, "the yes to a try on a metered link was not written");
            }
            if start(&names::realizer_unit(id), false) {
                Came::Retrying(name.clone())
            } else {
                Came::NotStarted
            }
        }
        Act::HandIn { id, download, from } => hand_in(id, download, from, places),
        Act::Import(from) => stick(names::IMPORT_FROM, names::IMPORT_UNIT, from, places),
        Act::Export(to) => stick(names::EXPORT_TO, names::EXPORT_UNIT, to, places),
    }
}

/// Starts `unit` as the person — never asking — and waits for it when
/// `wait`. Whether systemd took it.
fn start(unit: &str, wait: bool) -> bool {
    let Some(systemctl) = SYSTEMCTL else {
        tracing::warn!("this build does not know where systemctl is");
        return false;
    };
    let argv = ronoctua_parts::argv::start_as_the_person(systemctl, unit, wait);
    match std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .output()
    {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            tracing::warn!(unit, said = %String::from_utf8_lossy(&out.stderr).trim(), "systemd did not start it");
            false
        }
        Err(e) => {
            tracing::warn!(unit, error = %e, "systemctl did not run");
            false
        }
    }
}

/// The file at `from` handed in as `id`'s download `download`: a copy in
/// the person's hand-in folder, root's `accept` waited for, and the part's
/// record read for what it made of it.
fn hand_in(id: &str, download: &str, from: &Path, places: &Places) -> Came {
    if !from.is_file() {
        return Came::NoFile;
    }
    let folder = Path::new(names::HANDIN).join(id);
    let target = folder.join(download);
    let copied = std::fs::create_dir_all(&folder)
        .and_then(|()| match std::fs::remove_file(&target) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        })
        .and_then(|()| std::fs::copy(from, &target));
    if let Err(e) = copied {
        tracing::warn!(error = %e, "the file was not put in the hand-in folder");
        return Came::NotCopied;
    }
    if !start(names::ACCEPT_UNIT, true) {
        let _ = std::fs::remove_file(&target);
        return Came::NotStarted;
    }
    // THE SYSTEM'S RECORD is the machine's, rewritten only by the next
    // update: what root took is in the store.
    let still_missing = if id == names::SYSTEM {
        std::fs::read(&places.state)
            .ok()
            .and_then(|b| ronoctua_person::parse::<ronoctua_person::state::MachineState>(&b).ok())
            .and_then(|s| s.last_apply)
            .is_none_or(|a| {
                a.missing
                    .iter()
                    .any(|d| d.name == download && !Path::new(&d.out).exists())
            })
    } else {
        std::fs::read(places.records.join(format!("{id}.json")))
            .ok()
            .and_then(|b| ronoctua_person::parse::<Record>(&b).ok())
            .is_some_and(|r| r.missing.iter().any(|d| d.name == download))
    };
    if still_missing {
        Came::Refused(download.to_owned())
    } else if id == names::SYSTEM {
        Came::TakenForTheSystem(download.to_owned())
    } else {
        Came::Taken(download.to_owned())
    }
}

/// The stick folder `folder` named in the person's `file` for root, `unit`
/// waited for, and what root said of it read back.
fn stick(file: &str, unit: &str, folder: &Path, places: &Places) -> Came {
    let mut line = folder.as_os_str().as_encoded_bytes().to_vec();
    line.push(b'\n');
    if let Err(e) = std::fs::write(file, line) {
        tracing::warn!(error = %e, "the stick's folder was not named");
        return Came::NotNamed;
    }
    let asked = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    if !start(unit, true) {
        return Came::NotStarted;
    }
    // WHAT ROOT SAID OF THIS GESTURE, and never of the one before it.
    match ronoctua_parts::stick::read(&places.stick) {
        Some(said) if said.at.unix_s() >= asked.saturating_sub(1) => Came::Stick(said),
        _ => Came::NotStarted,
    }
}

/// What came of a gesture, in the door's words.
#[must_use]
pub fn said_of(came: &Came, texts: &ronoctua_i18n::Texts) -> String {
    match came {
        Came::Retrying(name) => {
            texts.t_with(key::DISTRO_PARTS_SAID_RETRYING, &[("name", name.as_str())])
        }
        Came::NotStarted => texts.t(key::DISTRO_PARTS_SAID_NOT_STARTED),
        Came::NoFile => texts.t(key::DISTRO_PARTS_SAID_NO_FILE),
        Came::NotCopied => texts.t(key::DISTRO_PARTS_SAID_NOT_COPIED),
        Came::Taken(name) => texts.t_with(key::DISTRO_PARTS_SAID_TAKEN, &[("name", name.as_str())]),
        Came::TakenForTheSystem(name) => texts.t_with(
            key::DISTRO_PARTS_SAID_TAKEN_FOR_THE_SYSTEM,
            &[("name", name.as_str())],
        ),
        Came::Refused(name) => {
            texts.t_with(key::DISTRO_PARTS_SAID_REFUSED, &[("name", name.as_str())])
        }
        Came::NotNamed => texts.t(key::DISTRO_PARTS_SAID_NOT_NAMED),
        Came::Stick(said) => match (said.verb, said.outcome) {
            (Verb::Import, Outcome::Done { count }) => texts.t_args(
                key::DISTRO_PARTS_SAID_IMPORTED,
                &[("count", ronoctua_i18n::Arg::Number(i64::from(count)))],
            ),
            (Verb::Export, Outcome::Done { count }) => texts.t_args(
                key::DISTRO_PARTS_SAID_EXPORTED,
                &[("count", ronoctua_i18n::Arg::Number(i64::from(count)))],
            ),
            (_, Outcome::Refused { why }) => texts.t(refusal_key(why)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_parts::record::{Failure, Target};
    use ronoctua_person::Stamp;
    use ronoctua_person::frontier::{Download, Mode};

    const OUT: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-x-1";

    fn record(id: &str, phase: Phase, attempts: u32, missing: Vec<Download>) -> Record {
        let mut r = Record::new(id).planned(Some(Target {
            drv: format!("{OUT}.drv"),
            out: OUT.to_owned(),
            interface: 1,
        }));
        r.phase = phase;
        r.attempts = attempts;
        if attempts > 0 {
            r.last_failure = Some(Failure {
                why: Why::BuildFailed,
                at: Stamp::from_unix_s(1_000),
                detail: "error: Cannot build\n> curl: (22) 404\nFor full logs, run:".to_owned(),
            });
        }
        r.missing = missing;
        r
    }

    fn download(name: &str, urls: &[&str], mode: Mode) -> Download {
        Download {
            drv: format!("{OUT}-{name}.drv"),
            out: format!("{OUT}-{name}"),
            name: name.to_owned(),
            hash: "sha256-AAAA".to_owned(),
            mode,
            urls: urls.iter().map(|u| (*u).to_owned()).collect(),
        }
    }

    fn readings() -> Readings {
        let names = BTreeMap::from([("yt".to_owned(), "yt-dlp".to_owned())]);
        Readings::of(
            vec![
                record(
                    "pot",
                    Phase::NeedsYou,
                    5,
                    vec![download("pot-npm-deps", &[], Mode::Recursive)],
                ),
                record(
                    "yt",
                    Phase::Arriving,
                    1,
                    vec![download(
                        "yt-dlp.tar.gz",
                        &["https://github.com/a/b.tar.gz"],
                        Mode::Flat,
                    )],
                ),
                record("gone", Phase::Unwanted, 0, Vec::new()),
                record("here", Phase::Here, 0, Vec::new()),
            ],
            &names,
            None,
        )
    }

    /// The readings with the system missing two downloads — one file, and
    /// a tree no address gives — after a build that failed.
    fn with_the_system(here: &[&str]) -> Readings {
        let mut r = readings();
        let last = ronoctua_person::state::LastApply {
            at: Stamp::from_unix_s(2_000),
            result: ronoctua_person::state::ApplyResult::BuildFailed,
            missing: vec![
                download(
                    "crate-x.tar.gz",
                    &["https://static.crates.io/x"],
                    Mode::Flat,
                ),
                download("pot-npm-deps", &[], Mode::Recursive),
            ],
        };
        r.system = System::of(Some(&last), |out| here.contains(&out));
        r
    }

    /// FAILS IF: the system's row is not the first when its last update
    /// missed a download, is there when it missed none, or a download the
    /// store holds now is still counted as one to bring.
    #[test]
    fn the_system_comes_first_while_it_misses_a_download() {
        let r = with_the_system(&[]);
        assert_eq!(rows_of(&r)[0], Row::System);
        assert_eq!(rows_of(&r)[1], Row::Part(0));
        let system = r.system.as_ref().unwrap();
        assert!(system.failed);
        assert_eq!(system.to_come().count(), 2);
        let crate_out = format!("{OUT}-crate-x.tar.gz");
        let some_here = with_the_system(&[crate_out.as_str()]);
        assert_eq!(some_here.system.as_ref().unwrap().to_come().count(), 1);
        let nothing = ronoctua_person::state::LastApply {
            at: Stamp::from_unix_s(2_000),
            result: ronoctua_person::state::ApplyResult::Switched,
            missing: Vec::new(),
        };
        assert_eq!(System::of(Some(&nothing), |_| false), None);
        assert_eq!(System::of(None, |_| false), None);
    }

    /// FAILS IF: `RET` on the system does not update again, or `h` on it
    /// asks a file for a download that is no file, one already here — or
    /// for none, when everything came.
    #[test]
    fn the_system_updates_again_and_takes_a_file_for_what_it_misses() {
        let r = with_the_system(&[]);
        let mut p = Page::default();
        assert_eq!(p.press(Press::Enter, &r), Errand::Update);
        assert_eq!(
            p.press(Press::HandIn, &r),
            Errand::Pick(Picking::HandIn {
                id: names::SYSTEM.into(),
                download: "crate-x.tar.gz".into()
            })
        );
        let crate_out = format!("{OUT}-crate-x.tar.gz");
        let file_here = with_the_system(&[crate_out.as_str()]);
        assert_eq!(
            p.press(Press::HandIn, &file_here),
            Errand::Say(key::DISTRO_PARTS_SAID_NOT_ONE_FILE)
        );
        let npm_out = format!("{OUT}-pot-npm-deps");
        let all_here = with_the_system(&[crate_out.as_str(), npm_out.as_str()]);
        assert_eq!(
            p.press(Press::HandIn, &all_here),
            Errand::Say(key::DISTRO_PARTS_SAID_NOTHING_MISSING)
        );
        assert_eq!(p.press(Press::Enter, &all_here), Errand::Update);
    }

    /// FAILS IF: a part the system no longer wants is a row, a part loses
    /// the proper name the plan gives it, or the stick's two rows are not
    /// the last.
    #[test]
    fn the_rows_are_the_wanted_parts_then_the_stick() {
        let r = readings();
        let ids: Vec<&str> = r.parts.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["pot", "yt", "here"]);
        assert_eq!(r.parts[1].name, "yt-dlp");
        assert_eq!(r.parts[0].name, "pot", "no name in the plan: the id");
        let rows = rows_of(&r);
        assert_eq!(&rows[3..], [Row::Import, Row::Export, Row::Mirrors]);
        assert_eq!(r.parts[1].due, Some(1_000 + 60), "the record's backoff");
    }

    /// FAILS IF: `RET` on a part does not try it now, tries one that is
    /// here, or `h` asks a file for a download that is no file — or for none.
    #[test]
    fn a_part_is_tried_now_and_a_file_is_asked_only_for_one_file() {
        let r = readings();
        let mut p = Page::default();
        assert_eq!(
            p.press(Press::Enter, &r),
            Errand::Run(Act::Retry {
                id: "pot".into(),
                name: "pot".into()
            })
        );
        assert_eq!(
            p.press(Press::HandIn, &r),
            Errand::Say(key::DISTRO_PARTS_SAID_NOT_ONE_FILE)
        );
        p.press(Press::Down, &r);
        // THE FILE PICKER, for the download that is one file.
        let asked = Picking::HandIn {
            id: "yt".into(),
            download: "yt-dlp.tar.gz".into(),
        };
        assert_eq!(p.press(Press::HandIn, &r), Errand::Pick(asked.clone()));
        let request = request_of(&asked, None);
        assert_eq!(request.mode, crate::file_picker::Mode::Open { many: false });
        let handed = Errand::Run(Act::HandIn {
            id: "yt".into(),
            download: "yt-dlp.tar.gz".into(),
            from: PathBuf::from("/home/x/yt-dlp.tar.gz"),
        });
        assert_eq!(
            picked(asked, &[PathBuf::from("/home/x/yt-dlp.tar.gz")]),
            handed
        );
        // AND TYPED after the command's name, for the part under the cursor.
        assert_eq!(p.typed(HAND_IN, "/home/x/yt-dlp.tar.gz", &r), handed);
        p.press(Press::Down, &r);
        assert_eq!(
            p.press(Press::Enter, &r),
            Errand::Say(key::DISTRO_PARTS_SAID_ALREADY_HERE)
        );
        assert_eq!(
            p.press(Press::HandIn, &r),
            Errand::Say(key::DISTRO_PARTS_SAID_NOTHING_MISSING)
        );
    }

    /// FAILS IF: a stick's row does not bring up the picker for a FOLDER,
    /// or a folder picked (or typed after the command's name) is taken for
    /// the other gesture.
    #[test]
    fn a_stick_row_picks_a_folder_for_its_own_gesture() {
        let r = readings();
        let mut p = Page::default();
        assert_eq!(p.ask_for(IMPORT, &r), Errand::Pick(Picking::Import));
        assert_eq!(p.ask_for(EXPORT, &r), Errand::Pick(Picking::Export));
        assert_eq!(
            request_of(&Picking::Export, None).mode,
            crate::file_picker::Mode::Folder
        );
        let stick = PathBuf::from("/run/media/noctua/STICK");
        assert_eq!(
            picked(Picking::Import, std::slice::from_ref(&stick)),
            Errand::Run(Act::Import(stick.clone()))
        );
        assert_eq!(
            p.typed(EXPORT, "/run/media/noctua/STICK", &r),
            Errand::Run(Act::Export(stick))
        );
        assert_eq!(p.answered(IMPORT, "/x"), Errand::Nothing, "no line asks it");
    }

    /// FAILS IF: the mirrors' row does not ask with the mirrors named, an
    /// address that may not be a mirror is taken, nothing typed does not
    /// name none, or a copy that does not read is offered for rewriting.
    #[test]
    fn the_mirrors_are_typed_checked_and_written_whole() {
        let mut r = readings();
        r.mirrors = Mirrors::Named(vec!["https://m.org".to_owned()]);
        let mut p = Page::default();
        let Errand::Ask(line) = p.ask_for(MIRRORS, &r) else {
            panic!("the mirrors' row asks");
        };
        assert_eq!((line.id, line.text.as_str()), (MIRRORS, "https://m.org"));
        assert_eq!(
            p.answered(MIRRORS, " https://a.org  http://lan:8080/f https://a.org "),
            Errand::Mirrors(vec![
                "https://a.org".to_owned(),
                "http://lan:8080/f".to_owned()
            ])
        );
        p.ask_for(MIRRORS, &r);
        assert_eq!(p.answered(MIRRORS, ""), Errand::Mirrors(Vec::new()), "none");
        p.ask_for(MIRRORS, &r);
        assert_eq!(
            p.answered(MIRRORS, "https://u:p@localhost/f"),
            Errand::Say(key::DISTRO_PARTS_SAID_NOT_A_MIRROR)
        );
        r.mirrors = Mirrors::Unreadable;
        assert_eq!(
            p.ask_for(MIRRORS, &r),
            Errand::Say(key::DISTRO_PARTS_WHY_UNREADABLE)
        );

        // WRITTEN WHOLE: what else the person's options say stays.
        let dir = std::env::temp_dir().join(format!("ronoctua-parts-page-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("options.json");
        std::fs::write(
            &path,
            br#"{"format": 1, "ssh-signing": null, "distro-apps-follow-releases": true}"#,
        )
        .unwrap();
        write_mirrors(&path, &["https://m.org".to_owned()]).unwrap();
        let back: ronoctua_person::options::Options =
            ronoctua_person::parse(&std::fs::read(&path).unwrap()).unwrap();
        assert!(back.distro_apps_follow_releases);
        assert_eq!(back.download_mirrors, ["https://m.org"]);
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(write_mirrors(&path, &[]), Err(Unchanged::Unreadable));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"{not json",
            "never rewritten"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a click does not take the cursor first and act second
    /// (invariant 19), or takes a row the page does not have.
    #[test]
    fn the_first_click_selects_and_the_second_acts() {
        let r = readings();
        let mut p = Page::default();
        assert_eq!(p.clicked(1, &r), Errand::Moved);
        assert!(matches!(p.clicked(1, &r), Errand::Run(Act::Retry { .. })));
        assert!(!p.takes(rows_of(&r).len(), &r));
        assert_eq!(p.clicked(99, &r), Errand::Nothing);
    }

    /// FAILS IF: the reason the page gives is nix's "for full logs" line
    /// instead of the last thing it said.
    #[test]
    fn the_reason_is_the_last_thing_nix_said() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let said = why_said(
            Why::BuildFailed,
            "a\n> curl: (22) 404\nFor full logs, run:\n",
            &texts,
        );
        assert!(said.ends_with(": curl: (22) 404"), "{said}");
    }
}
