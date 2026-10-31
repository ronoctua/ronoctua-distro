//! THE DISTRO'S APPS THAT FOLLOW THEIR RELEASES — a page of the update's
//! view, in both doors.
//!
//! # What it offers
//!
//! The owner's decision (2026-10-20): the person may let the routine update
//! move the distro's AppImages whose table entry follows a repository (the
//! Crafting Apps) past the versions the distro tested. The choice is theirs,
//! OFF until they turn it on here, and it travels signed with their options
//! (`distro-apps-follow-releases`, `ronoctua_person::options`) to every
//! machine of theirs. `f` turns the view to this page and back, in the
//! footer's region (`LEADER s u u`, or straight here with `LEADER s u f`) and
//! in the window (`LEADER s u w`), with the same rows and the same keys.
//!
//! # The rows
//!
//! The switch (`RET` turns it), the hosts following asks — turning it on lets
//! them in the person's consent, as every download asks first
//! (`ronoctua_person::distro_pins::HOSTS`) — what this machine was built
//! with (a change reaches it at the next update — and only signed: a machine
//! that trusts no key of the person's, or has not got their signature yet,
//! says so, from `person-applied.json`), and each AppImage that follows its
//! releases with the version this machine runs, whose pin it is and — on
//! the row itself, so both doors say it beside the cursor — where a followed
//! one stands: on its way, its download not fetched, its build failed (its
//! part's record, `ronoctua_parts`), or refused by a rule
//! (`/etc/ronoctua/distro-apps.json`, the registry's report).

use std::path::{Path, PathBuf};

use crate::key;
use ronoctua_person::distro_pins::{Fault, HOSTS};
use ronoctua_person::options::Options;
use ronoctua_person::reports::{DistroApps, PersonApplied, PinFrom};
use ronoctua_person::tree::TreeWhy;
use ronoctua_person::{Document as _, Why as PersonWhy};
use ronoctua_tokens as tokens;

use crate::binary_cache::{PageLine, TopLine};

/// The command that opens the update's view on this page (`LEADER s u f`).
pub const OPEN: &str = "distro-follow-open";

/// The registry's report on this machine (`nix/modules/distro-apps.nix`
/// writes it; `ronoctua_person::names::DISTRO_APPS` under `/etc/ronoctua`).
const REGISTRY: &str = "/etc/ronoctua/distro-apps.json";

/// What the machine made of the person's data, signature included
/// (`ronoctua_person::names::PERSON_APPLIED` under `/etc/ronoctua`).
const APPLIED: &str = "/etc/ronoctua/person-applied.json";

// ---------------------------------------------------------------------------
// WHAT THE PAGE READS
// ---------------------------------------------------------------------------

/// Where the page reads — the person's roots, and the machine's report,
/// under the footer camera's room while it photographs.
#[derive(Debug, Clone)]
pub struct Places {
    /// The person's copy of their options.
    pub mirror: Option<PathBuf>,
    /// The person's config root, whose `consent.json` lets the hosts.
    pub config: Option<PathBuf>,
    /// The registry's report.
    pub registry: PathBuf,
    /// The person's report: what root made of their signature.
    pub applied: PathBuf,
    /// The parts' records, where a followed version's arrival is said.
    pub records: PathBuf,
    /// Where the system's own links and the store are: `/`, or the
    /// camera's room — a system set for the next boot is read there
    /// ([`Readings::waiting`]).
    pub root: PathBuf,
    /// The machine's record, which every system set for the next boot (and
    /// every way back from one) rewrites: what a door watches for it.
    pub machine_state: PathBuf,
}

impl Places {
    /// This person's and this machine's.
    #[must_use]
    pub fn here() -> Self {
        let machine = |p: &str| match crate::photo::room() {
            Some(room) => room.join(p.trim_start_matches('/')),
            None => PathBuf::from(p),
        };
        let at = crate::update_window::record_places();
        Self {
            mirror: ronoctua_xdg::state_dir().map(|d| {
                d.join(ronoctua_person::names::MIRROR)
                    .join(ronoctua_person::names::OPTIONS)
            }),
            config: ronoctua_xdg::config_dir(),
            registry: machine(REGISTRY),
            applied: machine(APPLIED),
            records: machine(ronoctua_parts::names::RECORDS),
            root: at.root,
            machine_state: at.machine_state.join(ronoctua_person::names::STATE),
        }
    }

    /// When each file the page reads last moved — what a door compares to
    /// read again only when something did.
    #[must_use]
    pub fn stamp(&self) -> Vec<Option<std::time::SystemTime>> {
        let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        [
            self.mirror.clone(),
            self.config.as_ref().map(|c| c.join(ronoctua_consent::FILE)),
            Some(self.registry.clone()),
            Some(self.applied.clone()),
            Some(self.records.clone()),
            Some(self.machine_state.clone()),
        ]
        .iter()
        .map(|p| p.as_deref().and_then(mtime))
        .collect()
    }
}

/// What the person's copy of their options says of following.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Choice {
    /// Off — or no file yet.
    #[default]
    Off,
    /// On.
    On,
    /// The file is there and does not read.
    Unreadable,
    /// The file is of a newer format than this machine reads.
    Newer,
}

impl Choice {
    /// What the copy at `path` says.
    #[must_use]
    pub fn read(path: &Path) -> Self {
        match std::fs::read(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::Off,
            Err(_) => Self::Unreadable,
            Ok(bytes) => match ronoctua_person::parse::<Options>(&bytes) {
                Ok(o) if o.distro_apps_follow_releases => Self::On,
                Ok(_) => Self::Off,
                Err(r) if r.why == PersonWhy::Newer => Self::Newer,
                Err(_) => Self::Unreadable,
            },
        }
    }
}

/// One AppImage of the distro's that follows its releases, as this machine
/// was built with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// Its registry id — its part's id too.
    pub id: String,
    /// Its name (data, never translated), or its id.
    pub name: String,
    /// Whether this machine carries it.
    pub here: bool,
    /// Whose pin built it.
    pub pin: PinFrom,
    /// The version built.
    pub version: String,
    /// The followed version the registry took, when there is one.
    pub followed: Option<String>,
    /// Where its part stands with the followed version.
    pub arrival: Arrival,
    /// The rule the registry refused a followed pin of it by.
    pub refused: Option<Fault>,
}

/// Where an AppImage's part stands with the followed version — its record
/// (`ronoctua_parts::record`): the system builds the followed pin as the
/// part, and until it arrives the part keeps the version it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Arrival {
    /// What the system wants is the part's — or it follows nothing.
    #[default]
    Here,
    /// On its way: not tried yet, or being tried.
    OnItsWay,
    /// Its last try could not fetch the followed download.
    Missing,
    /// Its last try failed otherwise.
    Failed,
}

impl Arrival {
    /// What the part's record at `path` says.
    #[must_use]
    pub fn of_record(path: &Path) -> Self {
        let record = std::fs::read(path)
            .ok()
            .and_then(|b| ronoctua_person::parse::<ronoctua_parts::record::Record>(&b).ok());
        match record {
            None => Self::OnItsWay,
            Some(r) if r.phase == ronoctua_parts::record::Phase::Here => Self::Here,
            Some(r) if !r.missing.is_empty() => Self::Missing,
            Some(r) if r.last_failure.is_some() => Self::Failed,
            Some(_) => Self::OnItsWay,
        }
    }
}

/// Whether this machine takes the person's signed-only data — their options
/// and their followed pins — as `person-applied.json` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Taken {
    /// It does.
    #[default]
    Yes,
    /// It trusts no signing key of the person's: it never takes them.
    NoKey,
    /// It kept them back: they are not as the person signed them yet.
    Unsigned,
    /// It kept the followed pins back: they do not read here.
    Unreadable,
}

impl Taken {
    /// What a report says.
    #[must_use]
    pub fn of(applied: &PersonApplied) -> Self {
        if applied.signature.pinned.is_none() {
            return Self::NoKey;
        }
        let back = |file: &str| {
            let at = format!("{}/{file}", ronoctua_person::tree::PERSON);
            applied
                .signature
                .kept_back
                .iter()
                .find(|k| k.at == at)
                .map(|k| k.why)
        };
        let whys = [
            back(ronoctua_person::names::OPTIONS),
            back(ronoctua_person::names::DISTRO_PINS),
        ];
        if whys.contains(&Some(TreeWhy::NoKeyPinned)) {
            Self::NoKey
        } else if whys.contains(&Some(TreeWhy::NotAsSigned)) {
            Self::Unsigned
        } else if whys
            .iter()
            .flatten()
            .any(|w| matches!(w, TreeWhy::Document(_)))
        {
            Self::Unreadable
        } else {
            Self::Yes
        }
    }
}

/// Everything the page draws.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Readings {
    /// What the person's copy says.
    pub copy: Choice,
    /// Whether every host following asks is let.
    pub let_in: bool,
    /// Whether this machine was built following them — `None` without a
    /// report that says (none, or a build before the option).
    pub machine: Option<bool>,
    /// The AppImages that follow their releases, in the registry's order.
    pub apps: Vec<App>,
    /// Whether this machine takes the person's signed-only data.
    pub taken: Taken,
    /// When a system waits for the next restart, whether IT was built
    /// following them (`None` inside without a report that says); `None`
    /// when nothing waits.
    pub waiting: Option<Option<bool>>,
}

impl Readings {
    /// What `places` say now.
    #[must_use]
    pub fn read(places: &Places) -> Self {
        let registry: Option<DistroApps> = std::fs::read(&places.registry)
            .ok()
            .and_then(|b| ronoctua_person::parse(&b).ok());
        let (machine, mut apps) = registry.map_or((None, Vec::new()), |r| of_registry(&r));
        // A FOLLOWED VERSION IS ITS PART'S to bring.
        for a in &mut apps {
            if a.pin == PinFrom::Followed {
                a.arrival = Arrival::of_record(&places.records.join(format!("{}.json", a.id)));
            }
        }
        let taken = std::fs::read(&places.applied)
            .ok()
            .and_then(|b| ronoctua_person::parse::<PersonApplied>(&b).ok())
            .map_or(Taken::Yes, |a| Taken::of(&a));
        // THE SYSTEM THAT WAITS FOR THE RESTART, by its own report.
        let waiting = crate::update_window::waiting_system(&places.root).map(|top| {
            let file = places
                .root
                .join(top.trim_start_matches('/'))
                .join(REGISTRY.trim_start_matches('/'));
            std::fs::read(file)
                .ok()
                .and_then(|b| ronoctua_person::parse::<DistroApps>(&b).ok())
                .and_then(|r| of_registry(&r).0)
        });
        Self {
            taken,
            waiting,
            copy: places.mirror.as_deref().map_or(Choice::Off, Choice::read),
            let_in: places
                .config
                .as_deref()
                .is_some_and(|c| HOSTS.iter().all(|h| ronoctua_consent::allows_under(c, h))),
            machine,
            apps,
        }
    }

    /// Whether the person follows, as their copy says.
    #[must_use]
    pub const fn on(&self) -> bool {
        matches!(self.copy, Choice::On)
    }
}

/// What a registry's report says of following: whether the machine was
/// built following, and every AppImage that follows its releases. Pure.
#[must_use]
pub fn of_registry(r: &DistroApps) -> (Option<bool>, Vec<App>) {
    let mut known = false;
    let apps = r
        .apps
        .iter()
        .filter_map(|a| {
            let image = a.appimage.as_ref()?;
            known = true;
            image.follows.as_ref()?;
            Some(App {
                id: a.id.clone(),
                name: a.name.clone().unwrap_or_else(|| a.id.clone()),
                here: a.on_this_machine && !a.declined_here,
                pin: image.pin,
                version: image.version.clone(),
                followed: image.followed_version.clone(),
                arrival: Arrival::Here,
                refused: r
                    .followed
                    .refused
                    .iter()
                    .find(|f| f.id == a.id)
                    .map(|f| f.fault),
            })
        })
        .collect();
    (known.then_some(r.followed.on), apps)
}

// ---------------------------------------------------------------------------
// THE PAGE
// ---------------------------------------------------------------------------

/// One row of the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The switch — turned.
    Follow,
    /// The hosts following asks.
    Hosts,
    /// What this machine was built with.
    Machine,
    /// An AppImage that follows its releases, by its index.
    App(usize),
}

impl Row {
    /// Whether a `RET` on it does something.
    const fn acts(self) -> bool {
        matches!(self, Self::Follow)
    }
}

/// The rows `r` has: the three of the choice, then one per AppImage. Pure.
#[must_use]
pub fn rows_of(r: &Readings) -> Vec<Row> {
    [Row::Follow, Row::Hosts, Row::Machine]
        .into_iter()
        .chain((0..r.apps.len()).map(Row::App))
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
    /// Back to the steps.
    Escape,
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
    /// Following turned on (`true`) or off ([`apply`]).
    Turn(bool),
}

/// The page: its cursor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    cursor: usize,
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

    /// `RET` on the row under the cursor: the switch turns; a copy that
    /// does not read is never rewritten, and says why.
    fn enter(&self, r: &Readings) -> Errand {
        match rows_of(r).get(self.cursor) {
            Some(Row::Follow) => match r.copy {
                Choice::Off => Errand::Turn(true),
                Choice::On => Errand::Turn(false),
                Choice::Unreadable => Errand::Say(key::DISTRO_FOLLOW_WHY_UNREADABLE),
                Choice::Newer => Errand::Say(key::DISTRO_FOLLOW_WHY_NEWER),
            },
            _ => Errand::Nothing,
        }
    }

    /// The lines on top of the page: what the row under the cursor is, and
    /// — where its value only names a refusal or a hold — why, whole. Notes
    /// are WRAPPED, never cut.
    #[must_use]
    pub fn top_lines(&self, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<TopLine> {
        let Some(row) = rows_of(r).get(self.cursor).copied() else {
            return Vec::new();
        };
        let mut said = texts.t(match row {
            Row::Follow => key::DISTRO_FOLLOW_ABOUT_FOLLOW,
            Row::Hosts => key::DISTRO_FOLLOW_ABOUT_HOSTS,
            Row::Machine => key::DISTRO_FOLLOW_ABOUT_MACHINE,
            Row::App(_) => key::DISTRO_FOLLOW_ABOUT_APP,
        });
        // WHY THE CHOICE CANNOT BE CHANGED: said whole. An app's own reason is
        // on its row, beside the cursor in both doors.
        let why = match (row, r.copy) {
            (Row::Follow, Choice::Unreadable) => Some(texts.t(key::DISTRO_FOLLOW_WHY_UNREADABLE)),
            (Row::Follow, Choice::Newer) => Some(texts.t(key::DISTRO_FOLLOW_WHY_NEWER)),
            _ => None,
        };
        if let Some(why) = why {
            said.push(' ');
            said.push_str(&why);
        }
        crate::chrome::note_lines(&said, room)
            .into_iter()
            .map(|text| TopLine {
                text,
                color: tokens::color::TEXT_SECONDARY,
                answer: None,
            })
            .collect()
    }

    /// The page under the top lines: a heading, and a row a line — the label
    /// and the value, the cursor a colour — and, without one AppImage that
    /// follows its releases, a line that says so.
    #[must_use]
    pub fn lines(
        &self,
        r: &Readings,
        left: u16,
        room: u16,
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
                texts.t(key::DISTRO_FOLLOW_HEADING),
                tokens::color::TEXT_HEADING,
            ),
        ];
        for (n, row) in rows_of(r).into_iter().enumerate() {
            let focused = n == self.cursor;
            let (label, value, tint) = label_and_value(row, r, texts);
            out.push(PageLine {
                fields: crate::chrome::tinted(
                    crate::chrome::field_pair(
                        left,
                        room,
                        &label,
                        &value,
                        crate::chrome::Kind::of(row.acts()),
                        focused,
                    ),
                    tint,
                    focused,
                ),
                row: Some(n),
            });
        }
        if r.apps.is_empty() {
            for said in crate::chrome::note_lines(&texts.t(key::DISTRO_FOLLOW_NONE), room) {
                out.push(plain(said, tokens::color::TEXT_DIM));
            }
        }
        out
    }
}

/// A row's label and value, and the colour of a state it says (a refusal,
/// a hold, hosts not let while following), when it says one.
fn label_and_value(
    row: Row,
    r: &Readings,
    texts: &ronoctua_i18n::Texts,
) -> (String, String, Option<tokens::Color>) {
    let warning = Some(tokens::color::STATE_WARNING);
    match row {
        Row::Follow => {
            let (value, tint) = match r.copy {
                Choice::On => (key::DISTRO_FOLLOW_ON, None),
                Choice::Off => (key::DISTRO_FOLLOW_OFF, None),
                Choice::Unreadable => (key::DISTRO_FOLLOW_COPY_UNREADABLE, warning),
                Choice::Newer => (key::DISTRO_FOLLOW_COPY_NEWER, warning),
            };
            (texts.t(key::DISTRO_FOLLOW_ROW_FOLLOW), texts.t(value), tint)
        }
        Row::Hosts => {
            let hosts = HOSTS.join(", ");
            let (value, tint) = match (r.let_in, r.on()) {
                (true, _) => (key::DISTRO_FOLLOW_HOSTS_LET, None),
                (false, true) => (key::DISTRO_FOLLOW_HOSTS_NOT_LET, warning),
                (false, false) => (key::DISTRO_FOLLOW_HOSTS_NOT_LET, None),
            };
            (
                texts.t(key::DISTRO_FOLLOW_ROW_HOSTS),
                texts.t_with(value, &[("hosts", hosts.as_str())]),
                tint,
            )
        }
        Row::Machine => {
            // A CHOICE THE MACHINE DOES NOT TAKE says so first: the options and
            // the followed pins reach root only signed.
            let held_back = match (r.on(), r.taken) {
                (true, Taken::NoKey) => Some(key::DISTRO_FOLLOW_MACHINE_NO_KEY),
                (true, Taken::Unsigned) => Some(key::DISTRO_FOLLOW_MACHINE_UNSIGNED),
                (true, Taken::Unreadable) => Some(key::DISTRO_FOLLOW_MACHINE_UNREADABLE),
                _ => None,
            };
            let value = held_back.unwrap_or(match (r.copy, r.machine) {
                (Choice::On, Some(true)) => key::DISTRO_FOLLOW_MACHINE_IN_USE,
                // THE SYSTEM THAT WAITS FOR THE RESTART already follows them,
                // or already does not: from that restart, not the next update.
                (Choice::On, _) if r.waiting == Some(Some(true)) => {
                    key::DISTRO_FOLLOW_MACHINE_NEXT_RESTART
                }
                (Choice::On, _) => key::DISTRO_FOLLOW_MACHINE_NEXT,
                (Choice::Off, Some(true)) if r.waiting == Some(Some(false)) => {
                    key::DISTRO_FOLLOW_MACHINE_LEAVING_RESTART
                }
                (Choice::Off, Some(true)) => key::DISTRO_FOLLOW_MACHINE_LEAVING,
                // A COPY THAT DOES NOT READ says nothing of what comes next:
                // what the machine was built with is all that is known.
                (Choice::Unreadable | Choice::Newer, Some(true)) => {
                    key::DISTRO_FOLLOW_MACHINE_IN_USE
                }
                _ => key::DISTRO_FOLLOW_MACHINE_NONE,
            });
            (
                texts.t(key::DISTRO_FOLLOW_ROW_MACHINE),
                texts.t(value),
                held_back.map(|_| tokens::color::STATE_WARNING),
            )
        }
        Row::App(i) => {
            let Some(a) = r.apps.get(i) else {
                return (String::new(), String::new(), None);
            };
            if !a.here {
                return (
                    a.name.clone(),
                    texts.t(key::DISTRO_FOLLOW_APP_ELSEWHERE),
                    None,
                );
            }
            let followed = a.followed.as_deref().unwrap_or_default();
            // WHERE A FOLLOWED ONE STANDS, on the row itself.
            let arriving = |k| Some(texts.t_with(k, &[("followed", followed)]));
            let (note, tint) = match (a.arrival, a.refused) {
                (Arrival::OnItsWay, _) => (arriving(key::DISTRO_FOLLOW_NOTE_ARRIVING), None),
                (Arrival::Missing, _) => (arriving(key::DISTRO_FOLLOW_NOTE_MISSING), warning),
                (Arrival::Failed, _) => (arriving(key::DISTRO_FOLLOW_NOTE_FAILED), warning),
                (Arrival::Here, Some(fault)) => (Some(texts.t(note_refused(fault))), warning),
                (Arrival::Here, None) => (None, None),
            };
            let version = a.version.as_str();
            let value = match (a.pin, note) {
                (PinFrom::Followed, None) => {
                    texts.t_with(key::DISTRO_FOLLOW_APP_FOLLOWED, &[("version", version)])
                }
                (PinFrom::Distro, None) => {
                    texts.t_with(key::DISTRO_FOLLOW_APP_DISTRO, &[("version", version)])
                }
                (PinFrom::Followed, Some(note)) => texts.t_with(
                    key::DISTRO_FOLLOW_APP_FOLLOWED_BUT,
                    &[("version", version), ("note", note.as_str())],
                ),
                (PinFrom::Distro, Some(note)) => texts.t_with(
                    key::DISTRO_FOLLOW_APP_DISTRO_BUT,
                    &[("version", version), ("note", note.as_str())],
                ),
            };
            (a.name.clone(), value, tint)
        }
    }
}

/// The rule the registry refused a followed pin by, one sentence each.
const fn note_refused(fault: Fault) -> &'static str {
    match fault {
        Fault::Id => key::DISTRO_FOLLOW_NOTE_REFUSED_ID,
        Fault::Url => key::DISTRO_FOLLOW_NOTE_REFUSED_URL,
        Fault::Asset => key::DISTRO_FOLLOW_NOTE_REFUSED_ASSET,
        Fault::Version => key::DISTRO_FOLLOW_NOTE_REFUSED_VERSION,
        Fault::Hash => key::DISTRO_FOLLOW_NOTE_REFUSED_HASH,
        Fault::Duplicate => key::DISTRO_FOLLOW_NOTE_REFUSED_DUPLICATE,
    }
}

// ---------------------------------------------------------------------------
// THE CHANGE
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

/// Turns following `on` or off in the person's copy at `path`, the rest of
/// their options as they were — read with `ronoctua_person::parse`, written
/// whole with `ronoctua_person::to_json`, through a draft.
///
/// # Errors
///
/// [`Unchanged`].
pub fn turn(path: &Path, on: bool) -> Result<(), Unchanged> {
    let mut options = match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Options::default(),
        Err(_) => return Err(Unchanged::Unreadable),
        Ok(bytes) => ronoctua_person::parse::<Options>(&bytes).map_err(|r| {
            if r.why == PersonWhy::Newer {
                Unchanged::Newer
            } else {
                Unchanged::Unreadable
            }
        })?,
    };
    options.distro_apps_follow_releases = on;
    options.validate().map_err(|_| Unchanged::Unreadable)?;
    let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
        return Err(Unchanged::Unwritable);
    };
    std::fs::create_dir_all(dir).map_err(|_| Unchanged::Unwritable)?;
    ronoctua_xdg::write_atomically(dir, file, ronoctua_person::to_json(&options).as_bytes())
        .map_err(|_| Unchanged::Unwritable)
}

/// Carries out a turn of the switch: the person's copy of their options,
/// and — turning it on, which is the person's gesture — their consent for
/// the hosts following asks. Answers the sentence the door says.
#[must_use]
pub fn apply(on: bool, places: &Places, texts: &ronoctua_i18n::Texts) -> String {
    let Some(mirror) = places.mirror.as_deref() else {
        return texts.t(key::DISTRO_FOLLOW_WHY_UNWRITABLE);
    };
    if let Err(why) = turn(mirror, on) {
        return texts.t(match why {
            Unchanged::Unreadable => key::DISTRO_FOLLOW_WHY_UNREADABLE,
            Unchanged::Newer => key::DISTRO_FOLLOW_WHY_NEWER,
            Unchanged::Unwritable => key::DISTRO_FOLLOW_WHY_UNWRITABLE,
        });
    }
    if !on {
        return texts.t(key::DISTRO_FOLLOW_SAID_OFF);
    }
    // TURNING IT ON IS THE CONSENT for the hosts following asks, recorded
    // like any other.
    let kept = places
        .config
        .as_deref()
        .is_some_and(|c| ronoctua_consent::let_hosts_under(c, &HOSTS, true).is_ok());
    texts.t(if kept {
        key::DISTRO_FOLLOW_SAID_ON
    } else {
        key::DISTRO_FOLLOW_SAID_NOT_LET
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_person::declared::GithubReleases;
    use ronoctua_person::reports::{DistroApp, DistroAppImage, FollowedRefused, FollowedReport};

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-shell-update-follow-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn image(id: &str, pin: PinFrom) -> DistroApp {
        DistroApp {
            id: id.to_owned(),
            name: Some(id.to_uppercase()),
            source: ronoctua_person::declared::Source::AppImage,
            desktop_ids: Vec::new(),
            essential: None,
            group: "crafts".to_owned(),
            on_this_machine: true,
            declined_here: false,
            appimage: Some(DistroAppImage {
                key: id.to_owned(),
                follows: Some(GithubReleases {
                    repo: format!("storytold/{id}"),
                    asset_pattern: format!("{id}-*-linux-x86_64.AppImage"),
                }),
                pin,
                version: "0.9.0".to_owned(),
                distro_version: "0.8.0".to_owned(),
                distro_url: format!("https://github.com/storytold/{id}/releases/download/v0.8.0/x"),
                followed_version: Some("0.9.0".to_owned()),
            }),
        }
    }

    /// FAILS IF: an AppImage that follows its releases is left off the page,
    /// one that follows nothing (or a package) is put on it, whose pin built
    /// it and why a followed one is not used are not read, or a report of a
    /// build before the option says the machine follows anything.
    #[test]
    fn the_page_lists_what_follows_and_whose_pin_built_it() {
        let mut by_hand = image("byhand", PinFrom::Distro);
        if let Some(i) = by_hand.appimage.as_mut() {
            i.follows = None;
        }
        let mut package = image("gimp", PinFrom::Distro);
        package.appimage = None;
        let report = DistroApps {
            format: 1,
            apps: vec![
                image("cadcraft", PinFrom::Followed),
                image("figcraft", PinFrom::Distro),
                image("photocraft", PinFrom::Distro),
                by_hand,
                package,
            ],
            followed: FollowedReport {
                on: true,
                refused: vec![FollowedRefused {
                    id: "photocraft".to_owned(),
                    fault: ronoctua_person::distro_pins::Fault::Asset,
                    detail: String::new(),
                }],
            },
        };
        let (machine, apps) = of_registry(&report);
        assert_eq!(machine, Some(true));
        let names: Vec<&str> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["CADCRAFT", "FIGCRAFT", "PHOTOCRAFT"]);
        assert_eq!(
            apps[1].arrival,
            Arrival::Here,
            "the registry alone says no arrival"
        );
        assert_eq!(
            (apps[2].refused, apps[0].refused),
            (Some(ronoctua_person::distro_pins::Fault::Asset), None)
        );
        let mut older = report;
        for a in &mut older.apps {
            a.appimage = None;
        }
        assert_eq!(of_registry(&older), (None, Vec::new()));
    }

    /// FAILS IF: an app's row does not say, beside its version and whose it
    /// is, where a followed one stands — on its way, its download missing,
    /// its build failed — or why a newer one is not in use — refused by a
    /// rule —, or the machine's
    /// row does not say a choice it does not take: no key pinned, not
    /// signed yet, followed pins that do not read.
    #[test]
    fn a_row_says_why_and_the_machine_says_what_it_takes() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let app = |pin, arrival, refused| App {
            id: "figcraft".to_owned(),
            name: "FigCraft".to_owned(),
            here: true,
            pin,
            version: "0.7.0".to_owned(),
            followed: Some("0.7.2".to_owned()),
            arrival,
            refused,
        };
        let mut r = Readings {
            copy: Choice::On,
            let_in: true,
            machine: Some(true),
            waiting: None,
            apps: vec![
                app(PinFrom::Followed, Arrival::Missing, None),
                app(PinFrom::Followed, Arrival::OnItsWay, None),
                app(PinFrom::Distro, Arrival::Here, Some(Fault::Asset)),
                app(PinFrom::Followed, Arrival::Here, None),
            ],
            taken: Taken::Yes,
        };
        let value = |r: &Readings, row| label_and_value(row, r, &texts).1;
        assert_eq!(
            value(&r, Row::App(0)),
            texts.t_with(
                key::DISTRO_FOLLOW_APP_FOLLOWED_BUT,
                &[
                    ("version", "0.7.0"),
                    (
                        "note",
                        &texts.t_with(key::DISTRO_FOLLOW_NOTE_MISSING, &[("followed", "0.7.2")])
                    ),
                ]
            )
        );
        assert!(
            value(&r, Row::App(1)).contains("0.7.2"),
            "the one on its way is named"
        );
        assert_eq!(
            value(&r, Row::App(2)),
            texts.t_with(
                key::DISTRO_FOLLOW_APP_DISTRO_BUT,
                &[
                    ("version", "0.7.0"),
                    ("note", &texts.t(key::DISTRO_FOLLOW_NOTE_REFUSED_ASSET))
                ]
            )
        );
        assert_eq!(
            value(&r, Row::App(3)),
            texts.t_with(key::DISTRO_FOLLOW_APP_FOLLOWED, &[("version", "0.7.0")])
        );
        for (taken, k) in [
            (Taken::NoKey, key::DISTRO_FOLLOW_MACHINE_NO_KEY),
            (Taken::Unsigned, key::DISTRO_FOLLOW_MACHINE_UNSIGNED),
            (Taken::Unreadable, key::DISTRO_FOLLOW_MACHINE_UNREADABLE),
            (Taken::Yes, key::DISTRO_FOLLOW_MACHINE_IN_USE),
        ] {
            r.taken = taken;
            assert_eq!(value(&r, Row::Machine), texts.t(k), "{taken:?}");
        }
        // A SYSTEM THAT WAITS FOR THE RESTART and already does what the copy
        // says: from that restart, not the next update.
        for (copy, machine, waiting, k) in [
            (
                Choice::On,
                Some(false),
                Some(Some(true)),
                key::DISTRO_FOLLOW_MACHINE_NEXT_RESTART,
            ),
            (
                Choice::On,
                Some(false),
                Some(Some(false)),
                key::DISTRO_FOLLOW_MACHINE_NEXT,
            ),
            (
                Choice::On,
                Some(false),
                None,
                key::DISTRO_FOLLOW_MACHINE_NEXT,
            ),
            (
                Choice::Off,
                Some(true),
                Some(Some(false)),
                key::DISTRO_FOLLOW_MACHINE_LEAVING_RESTART,
            ),
            (
                Choice::Off,
                Some(true),
                Some(None),
                key::DISTRO_FOLLOW_MACHINE_LEAVING,
            ),
            (
                Choice::Off,
                Some(true),
                None,
                key::DISTRO_FOLLOW_MACHINE_LEAVING,
            ),
        ] {
            let r = Readings {
                copy,
                machine,
                waiting,
                ..r.clone()
            };
            assert_eq!(
                value(&r, Row::Machine),
                texts.t(k),
                "{copy:?} {machine:?} {waiting:?}"
            );
        }
        // WHAT A REPORT SAYS of the signature.
        let mut applied: PersonApplied = ronoctua_person::parse(
            br#"{"format": 1, "machine": "noctua", "distro-rev": "abc", "applied": [], "skipped": [],
                "declined": {"honoured": [], "refused": [], "unknown": [], "still-present": []},
                "brain": {"included": [], "awaiting-approval": []}}"#,
        )
        .unwrap();
        assert_eq!(Taken::of(&applied), Taken::NoKey);
        applied.signature.pinned = Some("a".repeat(64));
        assert_eq!(Taken::of(&applied), Taken::Yes);
        applied
            .signature
            .kept_back
            .push(ronoctua_person::tree::KeptBack {
                at: "person/options.json".to_owned(),
                why: TreeWhy::NotAsSigned,
            });
        assert_eq!(Taken::of(&applied), Taken::Unsigned);
        applied.signature.kept_back[0] = ronoctua_person::tree::KeptBack {
            at: "person/software/distro-pins.json".to_owned(),
            why: TreeWhy::Document(PersonWhy::Newer),
        };
        assert_eq!(Taken::of(&applied), Taken::Unreadable);
    }

    /// FAILS IF: `RET` on the switch does not turn it, turns it on a copy
    /// that does not read, a fact row acts, or a click acts before it has
    /// put the cursor there.
    #[test]
    fn the_switch_turns_and_a_strange_copy_says_why() {
        let mut r = Readings::default();
        let mut p = Page::default();
        assert_eq!(p.press(Press::Enter, &r), Errand::Turn(true));
        r.copy = Choice::On;
        assert_eq!(p.press(Press::Enter, &r), Errand::Turn(false));
        r.copy = Choice::Newer;
        assert_eq!(
            p.press(Press::Enter, &r),
            Errand::Say(key::DISTRO_FOLLOW_WHY_NEWER)
        );
        assert_eq!(p.press(Press::Down, &r), Errand::Moved);
        assert_eq!(p.press(Press::Enter, &r), Errand::Nothing, "the hosts row");
        assert_eq!(p.clicked(0, &r), Errand::Moved, "the first click selects");
        assert_eq!(p.clicked(0, &r), Errand::Say(key::DISTRO_FOLLOW_WHY_NEWER));
        assert_eq!(p.clicked(9, &r), Errand::Nothing);
        assert_eq!(p.press(Press::Escape, &r), Errand::Close);
    }

    /// FAILS IF: turning following on loses the person's other options,
    /// does not let both hosts, or a copy of a newer format is rewritten;
    /// or turning it off leaves the switch written.
    #[test]
    fn turning_it_on_lets_the_hosts_and_keeps_the_other_options() {
        let root = scratch("turn");
        let places = Places {
            mirror: Some(root.join("state/person/options.json")),
            config: Some(root.join("config")),
            registry: root.join("etc/distro-apps.json"),
            applied: root.join("etc/person-applied.json"),
            records: root.join("parts"),
            root: root.join("sys"),
            machine_state: root.join("state.json"),
        };
        let mirror = places.mirror.clone().unwrap();
        std::fs::create_dir_all(mirror.parent().unwrap()).unwrap();
        std::fs::write(
            &mirror,
            "{\"format\": 1, \"ssh-signing\": {\"key\": \".ssh/id.pub\"}}",
        )
        .unwrap();
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        assert_eq!(
            apply(true, &places, &texts),
            texts.t(key::DISTRO_FOLLOW_SAID_ON)
        );
        let r = Readings::read(&places);
        assert_eq!((r.copy, r.let_in), (Choice::On, true));
        let text = std::fs::read_to_string(&mirror).unwrap();
        assert!(text.contains(".ssh/id.pub"), "{text}");
        assert_eq!(
            apply(false, &places, &texts),
            texts.t(key::DISTRO_FOLLOW_SAID_OFF)
        );
        assert_eq!(Choice::read(&mirror), Choice::Off);
        assert!(
            !std::fs::read_to_string(&mirror)
                .unwrap()
                .contains("distro-apps-follow-releases")
        );
        std::fs::write(&mirror, "{\"format\": 9}").unwrap();
        assert_eq!(
            apply(true, &places, &texts),
            texts.t(key::DISTRO_FOLLOW_WHY_NEWER)
        );
        assert_eq!(std::fs::read_to_string(&mirror).unwrap(), "{\"format\": 9}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
