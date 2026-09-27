//! Syncthing — one view, two doors, and the mark the modeline may draw.
//!
//! # What is here
//!
//! Everything the screen decides: which rows exist, what a key does to them,
//! and what has to be asked of Syncthing as a result. Nothing here opens a
//! socket, runs a program or knows what a window is —
//! `crates/ronoctua-syncthing` is the boundary and `syncthing_window.rs` is the
//! second door. It is the shape `power.rs` established, `audio.rs` repeated and
//! `bluetooth.rs` gave levels to.
//!
//! # Why this exists at all
//!
//! The distro has installed and tuned Syncthing for a long time, and offered no
//! way to look at it: four leader commands, one of which opened a WEB PAGE in a
//! browser. Adding a folder, letting a machine in, or changing any of the
//! hundred and twenty-one options meant leaving the system for a page — which
//! is the outside chrome invariant 14 exists to refuse.
//!
//! # Four levels, and `Esc` goes up one
//!
//! The list, one folder, one machine, and the two sheets of settings. A field
//! says where one is; a `match` in [`View::rows`] draws it. That is the
//! Bluetooth arrangement, and the hint line teaches the LEVEL — a page about
//! the service that taught `r` for rescan would be teaching a dead key, which
//! is what the owner photographed in the radio view.
//!
//! # The drawer keeps the chevron, and nothing else may have it
//!
//! Eighty-two of the options are ones almost nobody touches. They sit in a
//! drawer, closed, and the drawer is marked with a chevron — because the owner
//! refused a chevron on a row that acts, in these words: *"não quero mais o
//! chevron, pois isso é utilizado em elementos de descolapsar/colapsar"*. By
//! their own argument the glyph belongs to collapsing, so this is the one place
//! in the view that draws it.
//!
//! # A row is a SETTING, and the setting is declared
//!
//! `assets/syncthing-options.nix` says what every option is: its JSON key, its
//! kind, its range, whether the flake owns it, whether it is advanced. This
//! file turns that into rows and keys into orders, and knows nothing about any
//! individual option. That is what keeps a hundred and twenty-one of them from
//! being a hundred and twenty-one decisions nobody could audit.

use crate::files::{Field, Hit as FilesHit, Ink};
use ronoctua_render::layout;
use ronoctua_syncthing::options::{self, Kind, Setting};
use ronoctua_syncthing::prefs::{Mark, Preferences};
use ronoctua_syncthing::{Invitation, Order, Snapshot, unit};
use ronoctua_tokens as tokens;

/// The keys this view teaches, and where each one applies.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "syncthing-down", Where::Anywhere),
    ("<up>", "syncthing-up", Where::Anywhere),
    // `S-<arrows>` TURN THE RING, which is this system's rule wherever a row
    // has a value: the locale view's clock, the Bluetooth volume, the display
    // view's modes. The plain arrows are left alone on purpose — this view has
    // levels, and a person holding an arrow is walking a list.
    ("S-<left>", "syncthing-less", Where::OnAValue),
    ("S-<right>", "syncthing-more", Where::OnAValue),
    // `RET` IS TAUGHT EVERYWHERE because it works everywhere: it opens a
    // level, turns a ring, does a deed — and on a fact it COPIES, which is
    // this system's rule and earns its keep here more than anywhere, since a
    // machine id is 63 characters nobody retypes.
    ("RET", "syncthing-enter", Where::Anywhere),
    ("p", "syncthing-pause", Where::OnAPausable),
    ("r", "syncthing-rescan", Where::OnAFolder),
    ("a", "syncthing-add", Where::OnAList),
    ("d", "syncthing-forget", Where::OnAForgettable),
    ("w", "syncthing-web", Where::Anywhere),
];

/// Which rows a key means anything on.
///
/// The hint line teaches the keys that WORK where the cursor is. A view that
/// taught `r` while the cursor sat on a machine would be teaching a key that
/// does nothing, and the owner photographed exactly that in the Bluetooth
/// radio page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns.
    OnAValue,
    /// On something that can be stopped and started.
    OnAPausable,
    /// On a folder.
    OnAFolder,
    /// On a list one can be added to.
    OnAList,
    /// On something that can be removed, or an invitation that can be refused.
    OnAForgettable,
}

impl Where {
    /// Whether a key is worth teaching for this row.
    fn shown(self, target: &Target) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnAValue => target.turns(),
            Self::OnAPausable => target.pausable(),
            Self::OnAFolder => matches!(target, Target::Folder(_)),
            Self::OnAList => target.addable().is_some(),
            Self::OnAForgettable => target.forgettable(),
        }
    }
}

/// A key, once the door has read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// The previous value on this row.
    Less,
    /// The next value on this row.
    More,
    /// Do what this row does.
    Enter,
    /// Stop what this row is about, or start it again.
    Pause,
    /// Look at the disk now.
    Rescan,
    /// Add one to the list this row is in.
    Add,
    /// Remove this, or say no to it.
    Forget,
    /// Open the web interface.
    Web,
    /// Back one level, and out.
    Escape,
}

/// Which page of the view is showing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Level {
    /// Everything: the service, the folders, the machines, the invitations.
    #[default]
    List,
    /// One folder's sheet.
    Folder(String),
    /// One machine's sheet.
    Device(String),
    /// The service's own settings.
    Global,
    /// The web interface's settings.
    Interface,
}

impl Level {
    /// The folder or machine this level is about, when it is about one.
    #[must_use]
    pub fn at(&self) -> Option<&str> {
        match self {
            Self::Folder(id) | Self::Device(id) => Some(id),
            _ => None,
        }
    }
}

/// What a row is, so the cursor can be found again after a rebuild.
///
/// Every row a cursor can stop on has one, and no two rows share one. The rows
/// move under the cursor constantly here — a drawer opens, an invitation
/// arrives, a folder finishes syncing and loses a line — and a cursor moved by
/// INDEX across that lands somewhere nobody pointed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spot {
    /// A heading, an empty line, or a sentence about a whole section.
    Nothing,
    /// A folder in the list.
    Folder(String),
    /// A machine in the list.
    Device(String),
    /// Somebody asking to be let in.
    Invitation(usize),
    /// One of the service's rows.
    Service(&'static str),
    /// A door to another level.
    Door(&'static str),
    /// A drawer's own heading.
    Drawer(&'static str),
    /// A declared option, by its id.
    Setting(&'static str),
    /// A folder sheet's sharing row, by machine.
    Share(String),
    /// A thing done to what this level is about.
    Deed(&'static str),
    /// The modeline mark, which is the only preference this view keeps.
    Mark,
}

/// Something done rather than changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deed {
    /// Stop talking to every machine, or start again.
    PauseEverything(bool),
    /// Ask Syncthing to restart itself. No privilege: it does this to itself.
    Restart,
    /// Start the systemd unit.
    StartUnit,
    /// Stop it.
    StopUnit,
    /// Look at this folder's disk now.
    Rescan(String),
    /// Send this machine's copy out as the truth.
    Override(String),
    /// Throw this machine's changes away.
    Revert(String),
    /// Remove a folder. The files stay; the sharing goes.
    ForgetFolder(String),
    /// Remove a machine.
    ForgetDevice(String),
}

impl Deed {
    /// The id, which is the stem of the row's text key.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::PauseEverything(true) => "resume-everything",
            Self::PauseEverything(false) => "pause-everything",
            Self::Restart => "restart",
            Self::StartUnit => "start-unit",
            Self::StopUnit => "stop-unit",
            Self::Rescan(_) => "rescan",
            Self::Override(_) => "override",
            Self::Revert(_) => "revert",
            Self::ForgetFolder(_) => "forget-folder",
            Self::ForgetDevice(_) => "forget-device",
        }
    }

    /// What it becomes on the wire.
    #[must_use]
    pub fn order(&self) -> Order {
        match self {
            Self::PauseEverything(true) => Order::Resume(None),
            Self::PauseEverything(false) => Order::Pause(None),
            Self::Restart => Order::Restart,
            Self::StartUnit => Order::Start,
            Self::StopUnit => Order::Stop,
            Self::Rescan(id) => Order::Rescan(id.clone()),
            Self::Override(id) => Order::Override(id.clone()),
            Self::Revert(id) => Order::Revert(id.clone()),
            Self::ForgetFolder(id) => Order::ForgetFolder(id.clone()),
            Self::ForgetDevice(id) => Order::ForgetDevice(id.clone()),
        }
    }
}

/// Something the person types, and what it is for.
///
/// Typed on the line of the door the key was pressed in — the footer's
/// minibuffer, the window's header — and answered through the same
/// [`View::answered`]. The view keeps which question it was
/// ([`View::asked`]), because the answer alone cannot say: a path may be a new
/// folder's or an offered one's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// A declared option that is text or a list.
    Setting(&'static str, Option<String>),
    /// A new machine's id.
    NewDevice,
    /// A new folder's path on this disk.
    NewFolder,
    /// Where an offered folder should be put.
    Invited(usize),
}

impl Ask {
    /// Which command carries the answer back.
    ///
    /// THE COMMAND THAT ASKS IS THE COMMAND THAT ANSWERS, which is this
    /// system's rule: the line's question — the minibuffer's or the window's
    /// — comes from that command's own `ask-<id>` key, and the answer arrives
    /// under that id. `s n v` and `s b y` both work this way.
    ///
    /// An earlier draft invented three ids that nothing else reached, and the
    /// keymap's own test found them: a command with no key path is a command
    /// only whoever read the code knows exists. These three all have one —
    /// two are leader keys in their own right, and the third is `RET`.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        match self {
            Self::Setting(..) => "syncthing-enter",
            Self::NewDevice => "syncthing-add-device",
            // AN OFFERED FOLDER IS ASKED THE SAME QUESTION as a new one,
            // because it IS the same question: where does it go on this disk.
            // Which of the two it was is the view's to remember.
            Self::NewFolder | Self::Invited(_) => "syncthing-add-folder",
        }
    }
}

/// What the keys do on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing at all — a fact. `RET` copies it.
    Nothing,
    /// A folder in the list. `RET` opens its sheet.
    Folder(String),
    /// A machine in the list. `RET` opens its sheet.
    Device(String),
    /// An invitation. `RET` takes it, `d` refuses it.
    Invitation(usize),
    /// A door to another level.
    Door(Level),
    /// A drawer, which `RET` opens and closes.
    Drawer(&'static str),
    /// A declared option, on this folder or machine.
    Setting(&'static str, Option<String>),
    /// Something done.
    Deed(Deed),
    /// Whether this folder is shared with this machine.
    Share(String, String, bool),
    /// The modeline mark.
    Mark,
}

impl Target {
    /// Whether `S-<arrows>` mean anything here.
    fn turns(&self) -> bool {
        match self {
            Self::Setting(id, _) => options::by_id(id).is_some_and(|s| s.kind.turns()),
            Self::Mark | Self::Share(..) => true,
            _ => false,
        }
    }

    /// Whether `p` means anything here.
    fn pausable(&self) -> bool {
        matches!(self, Self::Folder(_) | Self::Device(_))
            || matches!(self, Self::Deed(Deed::PauseEverything(_)))
    }

    /// Which list `a` would add to, when the cursor is in one.
    fn addable(&self) -> Option<bool> {
        match self {
            Self::Folder(_) => Some(true),
            Self::Device(_) => Some(false),
            _ => None,
        }
    }

    /// Whether `d` means anything here.
    fn forgettable(&self) -> bool {
        matches!(
            self,
            Self::Folder(_) | Self::Device(_) | Self::Invitation(_)
        )
    }
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// What is drawn, already placed and cut.
    pub fields: Vec<Field>,
    /// Whether the cursor is on it.
    pub focused: bool,
    /// Whether the cursor skips it.
    pub heading: bool,
    /// What a key does here.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: Spot,
    /// Reachable and refusing — drawn dim, and `RET` does not move.
    pub disabled: bool,
}

/// What the door has to do about a key.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Put this on the clipboard.
    Copy(String),
    /// Ask Syncthing.
    Order(Box<Order>),
    /// Write this view's own preference.
    Keep(Preferences),
    /// Say this, and it is already a sentence.
    Speak(&'static str),
    /// Open this question on the door's own line.
    Ask(Ask),
    /// Open the web interface at this address.
    Web(String),
    /// Shut the view.
    Close,
}

/// Where the rows start and how wide they are.
///
/// The same arithmetic as every other view of this shape, and it is
/// `power::extent` because there is one of it.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    crate::power::extent(columns)
}

/// The width the tests build rows at.
///
/// # Why production no longer has one
///
/// A key acts on the row under the cursor, so it has to know the rows — and it
/// used to build them at a fixed eighty columns, on the argument that the row
/// COUNT and the row TARGETS do not depend on the width. That argument held
/// while every note was CUT: an ellipsis changes what a row SAYS and never how
/// many there are.
///
/// A note WRAPS now (`chrome::note_lines`), so one sentence is a row on a wide
/// screen and three on a narrow one, and every row under it moves. A key
/// deciding at eighty would act on a different row from the one the door drew
/// — a control taken somewhere other than where it was drawn, arriving on the
/// keyboard's axis. So the DOOR's width travels in, as it already did for the
/// Bluetooth view, and what is left here is a width the tests agree on.
#[cfg(test)]
const DECIDING_WIDTH: u16 = 80;

/// How many cells a progress bar gets.
const BAR_CELLS: u16 = 10;

/// The view.
#[derive(Debug, Default)]
pub struct View {
    /// Which row the cursor is on.
    cursor: usize,
    /// Which page.
    level: Level,
    /// Which drawers are open, by their own ids.
    ///
    /// View state and not a file: closing the view forgets, which is the same
    /// answer `fold::Fold::toggle` gives. Opening the same drawer twice and
    /// finding it as you left it sounds kind and is one more thing that can be
    /// wrong on the first frame.
    open: Vec<&'static str>,
    /// This view's own preference.
    prefs: Preferences,
    /// What a line is open for, if anything.
    asking: Option<Ask>,
    /// Whether anybody has moved the cursor yet.
    moved: bool,
}

impl View {
    /// A view with the cursor on the first row that DOES something.
    #[must_use]
    pub fn fresh(prefs: Preferences) -> Self {
        Self {
            cursor: 1,
            level: Level::List,
            open: Vec::new(),
            prefs,
            asking: None,
            moved: false,
        }
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// This view's preference, as it stands.
    #[must_use]
    pub const fn preferences(&self) -> Preferences {
        self.prefs
    }

    /// Take the preference back off disk.
    pub const fn took_preferences(&mut self, prefs: Preferences) {
        self.prefs = prefs;
    }

    /// What was typed on a line, under the line's id, as an order — one call
    /// for both doors: the footer's minibuffer answers through the command of
    /// that id, the window through `Content::answered`.
    ///
    /// THE QUESTION THE VIEW ASKED ANSWERS FIRST, and only when the id is
    /// its own: a question let go, then `s s f` typed with the view still
    /// open, was answered as the old question — a path sent as a machine's
    /// id. With no question of the view's, the id decides ([`unasked`]).
    ///
    /// `None` when what came back cannot be used — an empty path for a new
    /// folder, say, which is a question somebody answered with nothing.
    pub fn answered(&mut self, id: &str, said: &str, snap: Option<&Snapshot>) -> Option<Order> {
        match self.asking.take().filter(|ask| ask.command() == id) {
            Some(ask) => Self::typed(ask, said, snap),
            None => unasked(id, said),
        }
    }

    /// The line was let go — `Esc`, `C-g`, a click on `ESC` — and the
    /// question with it.
    pub fn let_go(&mut self) {
        self.asking = None;
    }

    /// The question a line is open for — read by the window's own tests.
    #[cfg(test)]
    pub(crate) const fn asking(&self) -> Option<&Ask> {
        self.asking.as_ref()
    }

    /// What was typed for `ask`, as an order.
    fn typed(ask: Ask, said: &str, snap: Option<&Snapshot>) -> Option<Order> {
        let said = said.trim();
        if said.is_empty() {
            return None;
        }
        match ask {
            Ask::Setting(id, at) => {
                let setting = options::by_id(id)?;
                Some(Order::Set {
                    what: setting.id.to_owned(),
                    scope: setting.scope,
                    at,
                    body: setting.written(setting.typed(said)?),
                })
            }
            // THE SAME ORDER THE LEADER'S OWN QUESTION GIVES — one place.
            Ask::NewDevice | Ask::NewFolder => unasked(ask.command(), said),
            Ask::Invited(n) => match snap?.invitations.get(n)? {
                Invitation::Folder {
                    folder,
                    label,
                    from,
                    ..
                } => Some(Order::AcceptFolder {
                    folder: folder.clone(),
                    from: from.clone(),
                    label: label.clone(),
                    path: said.to_owned(),
                }),
                Invitation::Device { .. } => None,
            },
        }
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(snap, columns, texts);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        // A KEY THAT IS ONLY HERE BECAUSE OF THE ROW UNDER THE CURSOR reads as
        // one (`Register::Contextual`): the owner asked for the passing guests to
        // look apart from the keys that are always on the line.
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&target))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// Put the cursor on a row, if that row is one.
    ///
    /// `rows` is what the door DREW and `columns` is the width it drew them
    /// at; the two travel together, because everything downstream rebuilds
    /// the list to find the row again.
    ///
    /// Answers whether the cursor was ALREADY on that row: that is the second
    /// click, which the door turns into `RET` (invariant 19).
    pub fn clicked(
        &mut self,
        n: usize,
        rows: &[Row],
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        let Some(row) = rows.get(n).filter(|r| !r.heading) else {
            return false;
        };
        let again = self.cursor == n;
        self.moved = true;
        self.aim_at(&row.spot, snap, columns, texts);
        again
    }

    /// Put the cursor somewhere real.
    pub fn settle_in(
        &mut self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        // A LEVEL ABOUT SOMETHING THAT IS GONE GOES BACK TO THE LIST. A folder
        // removed from another machine, or by the person on this one, would
        // otherwise leave a sheet of empty rows with no way to tell why.
        if let Some(at) = self.level.at() {
            let still_there = snap.is_some_and(|s| match self.level {
                Level::Folder(_) => s.folders.iter().any(|f| f.id == at),
                Level::Device(_) => s.devices.iter().any(|d| d.id == at),
                _ => true,
            });
            if snap.is_some() && !still_there {
                self.level = Level::List;
                self.cursor = 1;
                self.moved = false;
            }
        }
        let rows = self.rows(snap, columns, texts);
        // AND `clamp` IS THE WHOLE OF IT. The sound view walks on to the first
        // row that ACTS, because its first rows are facts; here every row acts
        // — `RET` on a fact copies it — so the first row that is not a heading
        // is already the right one. An earlier draft walked to the first row
        // that TURNS instead, and the only such row in the list is the
        // modeline mark, at the very bottom: opening the view put the cursor
        // on the last line of the page.
        self.clamp(&rows);
        let want = rows
            .get(self.cursor)
            .map_or(Spot::Nothing, |r| r.spot.clone());
        self.aim_at(&want, snap, columns, texts);
    }

    /// Put the cursor on the row with this identity.
    fn aim_at(
        &mut self,
        want: &Spot,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(snap, columns, texts);
        if *want != Spot::Nothing
            && let Some(i) = rows.iter().position(|r| r.spot == *want)
        {
            self.cursor = i;
        }
        self.clamp(&rows);
    }

    /// Clamp the cursor into the rows, and off a heading. The INDEX only.
    fn clamp(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        if self.cursor >= rows.len() {
            self.cursor = rows.len() - 1;
        }
        if rows[self.cursor].heading {
            let after = (self.cursor..rows.len()).find(|&i| !rows[i].heading);
            let before = (0..=self.cursor).rev().find(|&i| !rows[i].heading);
            if let Some(i) = after.or(before) {
                self.cursor = i;
            }
        }
    }

    /// A key.
    ///
    /// THE WIDTH IS THE DOOR'S. A key acts on the row under the cursor and the
    /// cursor is an index into the rows that were DRAWN, so the two have to be
    /// the same list — and since a note wraps (`chrome::note_lines`), how many
    /// rows there are is a function of the width.
    pub fn key(
        &mut self,
        press: Press,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // A KEY THAT REACHES THE VIEW LETS ITS QUESTION GO: while a line is
        // open every key is the line's, in both doors, so a key here means
        // the line was answered or let go.
        self.asking = None;
        self.settle_in(snap, columns, texts);
        let rows = self.rows(snap, columns, texts);
        if rows.is_empty() {
            return match press {
                Press::Escape => Errand::Close,
                _ => Errand::Nothing,
            };
        }
        let target = rows[self.cursor].target.clone();
        let disabled = rows[self.cursor].disabled;

        match press {
            // `Esc` GOES UP ONE, and out of the list. The Bluetooth rule.
            Press::Escape => {
                if self.level == Level::List {
                    return Errand::Close;
                }
                self.level = Level::List;
                self.cursor = 1;
                self.moved = false;
                self.settle_in(snap, columns, texts);
                return Errand::Nothing;
            }
            Press::Up | Press::Down => {
                self.moved = true;
                self.walk(&rows, press == Press::Down);
                let spot = rows[self.cursor].spot.clone();
                self.aim_at(&spot, snap, columns, texts);
                return Errand::Nothing;
            }
            // THE WEB INTERFACE IS A PAGE, and it stays one key away. It has
            // graphs and a history that a list of labels and values does not
            // replace, and pretending otherwise would be worse than saying so.
            Press::Web => {
                return snap
                    .and_then(|s| s.service.running())
                    .map(|r| r.address.clone())
                    .filter(|a| !a.is_empty())
                    .map_or(
                        Errand::Speak(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED),
                        Errand::Web,
                    );
            }
            _ => {}
        }

        // A DISABLED ROW REFUSES AND SAYS WHY — sending the order would only
        // bring back a refusal, or worse, would go through and be undone by
        // the next `nixos-rebuild` with nothing on screen saying so.
        if disabled {
            return match press {
                Press::Enter | Press::Less | Press::More => {
                    Errand::Speak(ronoctua_i18n::key::ECHO_SYNCTHING_FLAKE_OWNS)
                }
                _ => Errand::Nothing,
            };
        }

        match press {
            Press::Less | Press::More => self.turn(&target, press == Press::More, snap),
            Press::Enter => self.enter(&target, &rows, snap),
            Press::Pause => Self::pause(&target, snap),
            Press::Rescan => match &target {
                Target::Folder(id) => Errand::Order(Box::new(Order::Rescan(id.clone()))),
                _ => Errand::Nothing,
            },
            Press::Add => match target.addable() {
                Some(true) => Errand::Ask(Ask::NewFolder),
                Some(false) => Errand::Ask(Ask::NewDevice),
                None => Errand::Nothing,
            },
            Press::Forget => match &target {
                Target::Folder(id) => Errand::Order(Box::new(Order::ForgetFolder(id.clone()))),
                Target::Device(id) => Errand::Order(Box::new(Order::ForgetDevice(id.clone()))),
                Target::Invitation(n) => Self::refuse(*n, snap),
                _ => Errand::Nothing,
            },
            Press::Up | Press::Down | Press::Escape | Press::Web => Errand::Nothing,
        }
    }

    /// One step around the value on this row.
    fn turn(&mut self, target: &Target, forward: bool, snap: Option<&Snapshot>) -> Errand {
        match target {
            Target::Setting(id, at) => {
                let Some(setting) = options::by_id(id) else {
                    return Errand::Nothing;
                };
                let now = snap.and_then(|s| s.setting(setting, at.as_deref()));
                let Some(next) = setting.turned(now, forward) else {
                    // TEXT AND LISTS ARE TYPED, and `S-<arrows>` on one opens
                    // the line rather than being a key that does nothing on a
                    // row that plainly has a value.
                    return Errand::Ask(Ask::Setting(setting.id, at.clone()));
                };
                Errand::Order(Box::new(Order::Set {
                    what: setting.id.to_owned(),
                    scope: setting.scope,
                    at: at.clone(),
                    body: setting.written(next),
                }))
            }
            Target::Mark => {
                let all: Vec<Mark> = Mark::all().collect();
                let at = all.iter().position(|m| *m == self.prefs.mark).unwrap_or(0);
                let step = if forward { 1 } else { all.len() - 1 };
                self.prefs.mark = all[(at + step) % all.len()];
                Errand::Keep(self.prefs)
            }
            Target::Share(folder, device, with) => Errand::Order(Box::new(Order::Share {
                folder: folder.clone(),
                device: device.clone(),
                with: !with,
            })),
            _ => Errand::Nothing,
        }
    }

    /// What `RET` does.
    fn enter(&mut self, target: &Target, rows: &[Row], snap: Option<&Snapshot>) -> Errand {
        match target {
            // A FACT COPIES ITSELF. `RET` here used to be the one key on the
            // one row that did nothing, in a list where half of what is drawn
            // is something somebody wants to paste — a device id above all,
            // which is 63 characters nobody types twice.
            Target::Nothing => rows
                .get(self.cursor)
                .and_then(|r| crate::chrome::information_of(&r.fields))
                .map_or(Errand::Nothing, Errand::Copy),
            Target::Folder(id) => {
                self.level = Level::Folder(id.clone());
                self.cursor = 1;
                self.moved = false;
                Errand::Nothing
            }
            Target::Device(id) => {
                self.level = Level::Device(id.clone());
                self.cursor = 1;
                self.moved = false;
                Errand::Nothing
            }
            Target::Door(level) => {
                self.level = level.clone();
                self.cursor = 1;
                self.moved = false;
                Errand::Nothing
            }
            Target::Drawer(id) => {
                if let Some(at) = self.open.iter().position(|open| open == id) {
                    self.open.remove(at);
                } else {
                    self.open.push(id);
                }
                Errand::Nothing
            }
            Target::Deed(deed) => Errand::Order(Box::new(deed.order())),
            // `RET` TURNS THE RING FORWARD, because `RET` is what a CLICK
            // becomes: a row only reachable with a modifier is a row the mouse
            // cannot use at all.
            Target::Setting(..) | Target::Mark | Target::Share(..) => self.turn(target, true, snap),
            Target::Invitation(n) => Self::accept(*n, snap),
        }
    }

    /// What `p` does.
    fn pause(target: &Target, snap: Option<&Snapshot>) -> Errand {
        match target {
            Target::Folder(id) => {
                let paused = snap
                    .and_then(|s| s.folders.iter().find(|f| f.id == *id))
                    .is_some_and(|f| f.paused);
                Errand::Order(Box::new(if paused {
                    Order::ResumeFolder(id.clone())
                } else {
                    Order::PauseFolder(id.clone())
                }))
            }
            Target::Device(id) => {
                let paused = snap
                    .and_then(|s| s.devices.iter().find(|d| d.id == *id))
                    .is_some_and(|d| d.paused);
                Errand::Order(Box::new(if paused {
                    Order::Resume(Some(id.clone()))
                } else {
                    Order::Pause(Some(id.clone()))
                }))
            }
            Target::Deed(deed @ Deed::PauseEverything(_)) => Errand::Order(Box::new(deed.order())),
            _ => Errand::Nothing,
        }
    }

    /// Letting somebody in.
    fn accept(n: usize, snap: Option<&Snapshot>) -> Errand {
        match snap.and_then(|s| s.invitations.get(n)) {
            Some(Invitation::Device { id, name, .. }) => {
                Errand::Order(Box::new(Order::AcceptDevice {
                    id: id.clone(),
                    name: name.clone(),
                }))
            }
            // A FOLDER NEEDS SOMEWHERE TO GO, and only the person knows where.
            Some(Invitation::Folder { .. }) => Errand::Ask(Ask::Invited(n)),
            None => Errand::Nothing,
        }
    }

    /// Saying no.
    fn refuse(n: usize, snap: Option<&Snapshot>) -> Errand {
        match snap.and_then(|s| s.invitations.get(n)) {
            Some(Invitation::Device { id, .. }) => {
                Errand::Order(Box::new(Order::RejectDevice(id.clone())))
            }
            Some(Invitation::Folder { folder, from, .. }) => {
                Errand::Order(Box::new(Order::RejectFolder {
                    folder: folder.clone(),
                    from: from.clone(),
                }))
            }
            None => Errand::Nothing,
        }
    }

    /// Remember what is being asked for, so the answer knows what it answers.
    pub fn asked(&mut self, ask: Ask) {
        self.asking = Some(ask);
    }

    /// One row up or down, past headings.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        loop {
            let next = if down {
                if at + 1 >= rows.len() {
                    return;
                }
                at + 1
            } else {
                if at == 0 {
                    return;
                }
                at - 1
            };
            at = next;
            if !rows[at].heading {
                self.cursor = at;
                return;
            }
        }
    }

    /// Every row, for this level.
    #[must_use]
    pub fn rows(
        &self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };
        let Some(s) = snap else {
            b.heading(ronoctua_i18n::key::SYNCTHING_TITLE);
            b.note(ronoctua_i18n::key::SYNCTHING_NO_SERVICE);
            return b.out;
        };
        match &self.level {
            Level::List => self.list(&mut b, s),
            Level::Folder(id) => self.folder_sheet(&mut b, s, id),
            Level::Device(id) => self.device_sheet(&mut b, s, id),
            Level::Global => {
                b.heading(ronoctua_i18n::key::SYNCTHING_GLOBAL);
                self.settings(&mut b, s, options::Scope::Global, None, "global");
            }
            Level::Interface => {
                b.heading(ronoctua_i18n::key::SYNCTHING_INTERFACE);
                b.note(ronoctua_i18n::key::SYNCTHING_INTERFACE_ABOUT);
                self.settings(&mut b, s, options::Scope::Interface, None, "interface");
            }
        }
        b.out
    }

    /// The first page.
    fn list(&self, b: &mut Builder<'_>, s: &Snapshot) {
        use ronoctua_i18n::key;
        b.heading(key::SYNCTHING_TITLE);
        if !s.service.is_up() {
            b.note(key::SYNCTHING_NO_SERVICE);
        }
        b.pair(
            b.texts.t(key::SYNCTHING_STATE),
            b.texts.t(state_key(s)),
            Target::Nothing,
            Spot::Service("state"),
            false,
        );
        if s.service.is_up() {
            let others = s.others().count();
            b.pair(
                b.texts.t(key::SYNCTHING_CONNECTED),
                b.texts.t_with(
                    key::SYNCTHING_CONNECTED_OF,
                    &[
                        ("connected", &s.connected().to_string()),
                        ("total", &others.to_string()),
                    ],
                ),
                Target::Nothing,
                Spot::Service("connected"),
                false,
            );
            let mark = b.texts.decimal_mark();
            b.pair(
                b.texts.t(key::SYNCTHING_TRAFFIC),
                b.texts.t_with(
                    key::SYNCTHING_TRAFFIC_BOTH,
                    &[
                        ("up", &crate::network::readable_bytes(s.traffic.sent, mark)),
                        (
                            "down",
                            &crate::network::readable_bytes(s.traffic.received, mark),
                        ),
                    ],
                ),
                Target::Nothing,
                Spot::Service("traffic"),
                false,
            );
        }

        // WHO IS ASKING TO BE LET IN — the one thing that, until this view,
        // only the web page could show, and the one thing that cannot wait:
        // the other end is a person at another machine wondering whether it
        // worked. The section EXISTS ONLY WHEN IT HAS SOMETHING IN IT.
        if !s.invitations.is_empty() {
            b.heading(key::SYNCTHING_INVITATIONS);
            for (n, invitation) in s.invitations.iter().enumerate() {
                let (label, value) = match invitation {
                    Invitation::Device { id, name, address } => (
                        if name.is_empty() {
                            id.clone()
                        } else {
                            name.clone()
                        },
                        b.texts.t_with(
                            key::SYNCTHING_WANTS_IN,
                            &[("address", if address.is_empty() { "?" } else { address })],
                        ),
                    ),
                    Invitation::Folder {
                        folder,
                        label,
                        who,
                        from,
                    } => (
                        if label.is_empty() {
                            folder.clone()
                        } else {
                            label.clone()
                        },
                        b.texts.t_with(
                            key::SYNCTHING_OFFERED_BY,
                            &[("who", if who.is_empty() { from } else { who })],
                        ),
                    ),
                };
                b.pair(
                    label,
                    value,
                    Target::Invitation(n),
                    Spot::Invitation(n),
                    false,
                );
            }
        }

        b.heading(key::SYNCTHING_FOLDERS);
        if s.folders.is_empty() {
            b.note(key::SYNCTHING_NO_FOLDERS);
        }
        for folder in &s.folders {
            b.pair(
                folder.name().to_owned(),
                folder_said(folder, b.room, b.texts),
                Target::Folder(folder.id.clone()),
                Spot::Folder(folder.id.clone()),
                false,
            );
        }

        b.heading(key::SYNCTHING_DEVICES);
        if s.others().count() == 0 {
            b.note(key::SYNCTHING_NO_DEVICES);
        }
        for device in s.others() {
            b.pair(
                device.label().to_owned(),
                b.texts.t(device_key(device)),
                Target::Device(device.id.clone()),
                Spot::Device(device.id.clone()),
                false,
            );
        }

        b.heading(key::SYNCTHING_SERVICE);
        self.service(b, s);

        b.heading(key::SYNCTHING_SETTINGS);
        b.door(
            key::SYNCTHING_GLOBAL,
            Level::Global,
            "global",
            !s.service.is_up(),
        );
        b.door(
            key::SYNCTHING_INTERFACE,
            Level::Interface,
            "interface",
            !s.service.is_up(),
        );
        // OURS, AND THE ONLY ONE. Everything above belongs to Syncthing and is
        // kept by Syncthing; this is a decision it has no opinion about.
        b.pair(
            b.texts.t(key::SYNCTHING_MARK),
            b.texts.t(mark_key(self.prefs.mark)),
            Target::Mark,
            Spot::Mark,
            false,
        );
    }

    /// The rows about the service itself.
    fn service(&self, b: &mut Builder<'_>, s: &Snapshot) {
        use ronoctua_i18n::key;
        // STOPPING THE UNIT IS THE ONE PRIVILEGED THING IN THIS VIEW, and it
        // is why the module carries a polkit rule. Everything else goes over
        // the loopback and asks nobody.
        //
        // MOVING OR UNASKABLE OFFERS NEITHER BUTTON, and says which it is.
        // `systemctl` that cannot be reached is not the same as a unit that is
        // off, and a button that guessed between them would be a button that
        // lies half the time.
        match s.unit {
            unit::Standing::Running => b.deed(key::SYNCTHING_STOP_UNIT, Deed::StopUnit, false),
            unit::Standing::Stopped => b.deed(key::SYNCTHING_START_UNIT, Deed::StartUnit, false),
            unit::Standing::Moving | unit::Standing::Absent | unit::Standing::Unknown => b.pair(
                b.texts.t(key::SYNCTHING_UNIT),
                b.texts.t(unit_key(s.unit)),
                Target::Nothing,
                Spot::Service("unit"),
                false,
            ),
        }

        if s.service.is_up() {
            let paused = s.paused();
            b.deed(
                if paused {
                    key::SYNCTHING_RESUME_EVERYTHING
                } else {
                    key::SYNCTHING_PAUSE_EVERYTHING
                },
                Deed::PauseEverything(paused),
                s.others().count() == 0,
            );
            // RESTART IS SYNCTHING DOING IT TO ITSELF, over the loopback, and
            // it is drawn ONCE. An earlier draft reached this line by two
            // paths and put the same row on the screen twice.
            b.deed(key::SYNCTHING_RESTART, Deed::Restart, false);
        }

        let Some(running) = s.service.running() else {
            return;
        };
        b.pair(
            b.texts.t(key::SYNCTHING_VERSION),
            running.version.clone(),
            Target::Nothing,
            Spot::Service("version"),
            false,
        );
        b.pair(
            b.texts.t(key::SYNCTHING_THIS_MACHINE),
            running.id.clone(),
            Target::Nothing,
            Spot::Service("id"),
            false,
        );
        b.pair(
            b.texts.t(key::SYNCTHING_UPTIME),
            for_how_long(running.uptime, b.texts),
            Target::Nothing,
            Spot::Service("uptime"),
            false,
        );
        if !running.listeners.is_empty() {
            b.pair(
                b.texts.t(key::SYNCTHING_LISTENING),
                running.listeners.join(", "),
                Target::Nothing,
                Spot::Service("listening"),
                false,
            );
        }
        b.pair(
            b.texts.t(key::SYNCTHING_WEB),
            running.address.clone(),
            Target::Nothing,
            Spot::Service("web"),
            false,
        );
        for (n, complaint) in running.errors.iter().enumerate() {
            b.said(
                complaint.clone(),
                tokens::color::STATE_ERROR,
                Spot::Service(match n {
                    0 => "error-0",
                    1 => "error-1",
                    _ => "error-more",
                }),
            );
        }
    }

    /// One folder's sheet.
    fn folder_sheet(&self, b: &mut Builder<'_>, s: &Snapshot, id: &str) {
        use ronoctua_i18n::key;
        let Some(folder) = s.folders.iter().find(|f| f.id == id) else {
            b.heading(key::SYNCTHING_FOLDERS);
            b.note(key::SYNCTHING_GONE);
            return;
        };
        b.title(folder.name().to_owned());
        b.pair(
            b.texts.t(key::SYNCTHING_FOLDER_ID),
            folder.id.clone(),
            Target::Nothing,
            Spot::Service("folder-id"),
            false,
        );
        b.pair(
            b.texts.t(key::SYNCTHING_STATE),
            b.texts.t(&format!("syncthing-state-{}", folder.state.id())),
            Target::Nothing,
            Spot::Service("folder-state"),
            false,
        );
        let mark = b.texts.decimal_mark();
        b.pair(
            b.texts.t(key::SYNCTHING_SIZE),
            crate::network::readable_bytes(folder.global_bytes, mark),
            Target::Nothing,
            Spot::Service("folder-size"),
            false,
        );
        if folder.errors > 0 {
            b.pair(
                b.texts.t(key::SYNCTHING_ERRORS),
                folder.errors.to_string(),
                Target::Nothing,
                Spot::Service("folder-errors"),
                false,
            );
        }

        b.heading(key::SYNCTHING_SHARED_WITH);
        let others: Vec<_> = s.others().collect();
        if others.is_empty() {
            b.note(key::SYNCTHING_NO_DEVICES);
        }
        for device in others {
            let with = folder.devices.iter().any(|d| d == &device.id);
            b.pair(
                device.label().to_owned(),
                b.texts.t(if with {
                    key::SYNCTHING_SHARED
                } else {
                    key::SYNCTHING_NOT_SHARED
                }),
                Target::Share(folder.id.clone(), device.id.clone(), with),
                Spot::Share(device.id.clone()),
                false,
            );
        }

        b.heading(key::SYNCTHING_ACTIONS);
        b.deed(
            key::SYNCTHING_RESCAN,
            Deed::Rescan(folder.id.clone()),
            false,
        );
        // OVERRIDE IS FOR A SEND-ONLY FOLDER AND REVERT FOR A RECEIVE-ONLY
        // ONE, and each is drawn refusing on the other — reachable, saying
        // what it needs, rather than a key that is simply not there.
        b.deed(
            key::SYNCTHING_OVERRIDE,
            Deed::Override(folder.id.clone()),
            folder.kind != "sendonly",
        );
        b.deed(
            key::SYNCTHING_REVERT,
            Deed::Revert(folder.id.clone()),
            folder.kind != "receiveonly",
        );
        b.deed(
            key::SYNCTHING_FORGET_FOLDER,
            Deed::ForgetFolder(folder.id.clone()),
            false,
        );

        self.settings(b, s, options::Scope::Folder, Some(&folder.id), "folder");
    }

    /// One machine's sheet.
    fn device_sheet(&self, b: &mut Builder<'_>, s: &Snapshot, id: &str) {
        use ronoctua_i18n::key;
        let Some(device) = s.devices.iter().find(|d| d.id == id) else {
            b.heading(key::SYNCTHING_DEVICES);
            b.note(key::SYNCTHING_GONE);
            return;
        };
        b.title(device.label().to_owned());
        b.pair(
            b.texts.t(key::SYNCTHING_DEVICE_ID),
            device.id.clone(),
            Target::Nothing,
            Spot::Service("device-id"),
            false,
        );
        b.pair(
            b.texts.t(key::SYNCTHING_STATE),
            b.texts.t(device_key(device)),
            Target::Nothing,
            Spot::Service("device-state"),
            false,
        );
        if device.connected {
            b.pair(
                b.texts.t(key::SYNCTHING_ADDRESS),
                device.address.clone(),
                Target::Nothing,
                Spot::Service("device-address"),
                false,
            );
            if !device.client.is_empty() {
                b.pair(
                    b.texts.t(key::SYNCTHING_RUNNING),
                    device.client.clone(),
                    Target::Nothing,
                    Spot::Service("device-client"),
                    false,
                );
            }
            let mark = b.texts.decimal_mark();
            b.pair(
                b.texts.t(key::SYNCTHING_TRAFFIC),
                b.texts.t_with(
                    key::SYNCTHING_TRAFFIC_BOTH,
                    &[
                        ("up", &crate::network::readable_bytes(device.sent, mark)),
                        (
                            "down",
                            &crate::network::readable_bytes(device.received, mark),
                        ),
                    ],
                ),
                Target::Nothing,
                Spot::Service("device-traffic"),
                false,
            );
        }

        b.heading(key::SYNCTHING_ACTIONS);
        b.deed(
            key::SYNCTHING_FORGET_DEVICE,
            Deed::ForgetDevice(device.id.clone()),
            false,
        );

        self.settings(b, s, options::Scope::Device, Some(&device.id), "device");
    }

    /// The declared options of one scope: the plain ones, then the drawer.
    fn settings(
        &self,
        b: &mut Builder<'_>,
        s: &Snapshot,
        scope: options::Scope,
        at: Option<&str>,
        drawer: &'static str,
    ) {
        let plain: Vec<&Setting> = options::of_scope(scope).filter(|o| !o.advanced).collect();
        let hidden: Vec<&Setting> = options::of_scope(scope).filter(|o| o.advanced).collect();

        if !plain.is_empty() {
            b.heading(ronoctua_i18n::key::SYNCTHING_OPTIONS);
            for setting in plain {
                self.setting(b, s, setting, at);
            }
        }
        if hidden.is_empty() {
            return;
        }
        let open = self.open.contains(&drawer);
        b.drawer(ronoctua_i18n::key::SYNCTHING_ADVANCED, drawer, open);
        if !open {
            return;
        }
        // THE NOTE IS THE OWNER'S, and it is one sentence for the whole
        // drawer rather than a hundred and twenty-one of them: what it warns
        // about is the same for every row inside it.
        b.note(ronoctua_i18n::key::SYNCTHING_ADVANCED_ABOUT);
        for setting in hidden {
            self.setting(b, s, setting, at);
        }
    }

    /// One declared option's row.
    fn setting(&self, b: &mut Builder<'_>, s: &Snapshot, setting: &Setting, at: Option<&str>) {
        let now = s.setting(setting, at);
        let value = match setting.kind {
            Kind::Switch => b
                .texts
                .t(if now.and_then(serde_json::Value::as_bool) == Some(true) {
                    ronoctua_i18n::key::SYNCTHING_ON
                } else {
                    ronoctua_i18n::key::SYNCTHING_OFF
                }),
            // A RING'S VALUE IS A WORD OF OURS, and Syncthing's own when we
            // have none — the shape `audio.rs` uses for a sound port. A word
            // a release adds draws as itself rather than as a missing key.
            Kind::Ring(_) => {
                let raw = now.and_then(serde_json::Value::as_str).unwrap_or("");
                let key = options::word_key(raw);
                let said = b.texts.t(&key);
                if said.starts_with('[') && !raw.is_empty() {
                    raw.to_owned()
                } else {
                    said
                }
            }
            // A NUMBER CARRIES WHAT IT IS COUNTED IN. `look again every  3600`
            // says nothing at all — that came off a printed screen, and no
            // assertion had a way to see it.
            Kind::Number { unit, .. } => {
                let n = setting.readable(now);
                match unit {
                    Some(key) => format!("{n} {}", b.texts.t(key)),
                    None => n,
                }
            }
            _ => setting.readable(now),
        };
        b.pair(
            b.texts.t(setting.label),
            value,
            Target::Setting(setting.id, at.map(ToOwned::to_owned)),
            Spot::Setting(setting.id),
            setting.fixed,
        );
    }
}

/// What was typed under `id` when the view asked nothing — the leader's own
/// questions, `s s d` and `s s f`, which work with no view open at all.
///
/// NOTHING TYPED IS NOT A FAILURE: it is a question answered with nothing,
/// which is how a person says never mind.
#[must_use]
pub fn unasked(id: &str, said: &str) -> Option<Order> {
    let said = said.trim();
    if said.is_empty() {
        return None;
    }
    match id {
        "syncthing-add-device" => Some(Order::AddDevice {
            id: said.to_owned(),
            name: String::new(),
        }),
        // SYNCTHING MAKES THE ID, and it makes a random one when the field is
        // empty. Typing one is how two machines end up sharing a folder they
        // never meant to share.
        "syncthing-add-folder" => Some(Order::AddFolder {
            id: String::new(),
            label: String::new(),
            path: said.to_owned(),
        }),
        _ => None,
    }
}

/// Which key a command id is.
///
/// THE ONE MAP, and both doors reach it. The footer reads a `Sym` and the
/// window reads a `Keysym`; what they share is the NOTATION in
/// [`OF_THE_VIEW`], so each door turns its own key into that notation and this
/// turns the notation into something the view understands.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "syncthing-up" => Press::Up,
        "syncthing-down" => Press::Down,
        "syncthing-less" => Press::Less,
        "syncthing-more" => Press::More,
        "syncthing-enter" => Press::Enter,
        "syncthing-pause" => Press::Pause,
        "syncthing-rescan" => Press::Rescan,
        "syncthing-add" => Press::Add,
        "syncthing-forget" => Press::Forget,
        "syncthing-web" => Press::Web,
        _ => return None,
    })
}

/// Which key a notation is, through [`OF_THE_VIEW`].
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

/// What a key means in the view — THE ONE READER, which both doors call:
/// the footer with the key it translated, the window with
/// `door::keys::key_of`.
///
/// BY THE WHOLE NOTATION, modifiers and all. The footer built it with shift
/// alone, so `C-d` arrived as `d` and forgot a folder or a machine, while
/// the window matched the text a key typed — a control character under Ctrl
/// — and the same chord did nothing there. The value keys ARE the shifted
/// arrows, so the shift has to be in it.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    match key.sym {
        ronoctua_keymap::Sym::Esc => Some(Press::Escape),
        _ => pressed(&key.notation()),
    }
}

/// What the whole thing is doing, as one word.
#[must_use]
pub fn state_key(s: &Snapshot) -> &'static str {
    use ronoctua_i18n::key;
    if !s.service.is_up() {
        return key::SYNCTHING_STATE_DOWN;
    }
    if s.paused() {
        return key::SYNCTHING_STATE_PAUSED;
    }
    if s.folders.iter().any(|f| f.need_bytes > 0) {
        return key::SYNCTHING_STATE_SYNCING;
    }
    key::SYNCTHING_STATE_IDLE
}

/// What a machine is doing, as one word.
#[must_use]
pub fn device_key(device: &ronoctua_syncthing::Device) -> &'static str {
    use ronoctua_i18n::key;
    if device.paused {
        key::SYNCTHING_DEVICE_PAUSED
    } else if device.connected {
        key::SYNCTHING_DEVICE_CONNECTED
    } else {
        key::SYNCTHING_DEVICE_AWAY
    }
}

/// What the systemd unit is doing, as one word.
#[must_use]
pub const fn unit_key(standing: unit::Standing) -> &'static str {
    use ronoctua_i18n::key;
    match standing {
        unit::Standing::Running => key::SYNCTHING_UNIT_RUNNING,
        unit::Standing::Stopped => key::SYNCTHING_UNIT_STOPPED,
        unit::Standing::Moving => key::SYNCTHING_UNIT_MOVING,
        unit::Standing::Absent => key::SYNCTHING_UNIT_ABSENT,
        unit::Standing::Unknown => key::SYNCTHING_UNIT_UNKNOWN,
    }
}

/// The word for one setting of the modeline mark.
#[must_use]
pub const fn mark_key(mark: Mark) -> &'static str {
    use ronoctua_i18n::key;
    match mark {
        Mark::Never => key::SYNCTHING_MARK_NEVER,
        Mark::WhenOff => key::SYNCTHING_MARK_WHEN_OFF,
        Mark::WhenOn => key::SYNCTHING_MARK_WHEN_ON,
    }
}

/// The sentence one refusal gets.
///
/// ONE PLACE, so the two doors cannot disagree about what a failure was. It is
/// a `_words` file in the sound and radio views because those have several
/// sentences to make; this has one, and a file for one function is a file.
#[must_use]
pub const fn refusal_key(why: ronoctua_syncthing::Refusal) -> &'static str {
    use ronoctua_i18n::key;
    match why {
        ronoctua_syncthing::Refusal::NotRunning => key::ECHO_SYNCTHING_NOT_RUNNING,
        ronoctua_syncthing::Refusal::NotAllowed => key::ECHO_SYNCTHING_NOT_ALLOWED,
        ronoctua_syncthing::Refusal::Refused | ronoctua_syncthing::Refusal::Unknown => {
            key::ECHO_SYNCTHING_UNKNOWN
        }
    }
}

/// Where Syncthing's own page is, or nothing.
///
/// The LEADER key reaches this with no view open, so it cannot go through the
/// view. The address is Syncthing's own — read out of the `config.xml` it
/// wrote — which is why nothing here has to agree with the module's option.
#[must_use]
pub fn web_address(snap: Option<&Snapshot>) -> String {
    snap.and_then(|s| s.service.running())
        .map(|r| r.address.clone())
        .unwrap_or_default()
}

/// How long the service has been up, in the largest unit that fits.
///
/// One unit and not three. `3 d 4 h 17 min` is a measurement; what a person
/// wants off this row is whether it restarted recently, and the largest unit
/// answers that. The number is formatted by Fluent, which is what knows the
/// language's own separators — `fluent-bundle` does not localise a number that
/// arrives already turned into a string.
#[must_use]
pub fn for_how_long(seconds: u64, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    let (key, n) = if seconds >= 86_400 {
        (key::SYNCTHING_FOR_DAYS, seconds / 86_400)
    } else if seconds >= 3_600 {
        (key::SYNCTHING_FOR_HOURS, seconds / 3_600)
    } else if seconds >= 60 {
        (key::SYNCTHING_FOR_MINUTES, seconds / 60)
    } else {
        (key::SYNCTHING_FOR_SECONDS, seconds)
    };
    texts.t_with(key, &[("n", &n.to_string())])
}

/// A folder's value column: a bar while it is moving, a word when it is not.
fn folder_said(
    folder: &ronoctua_syncthing::Folder,
    room: u16,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let word = texts.t(&format!("syncthing-state-{}", folder.state.id()));
    if folder.completion >= 100 {
        return word;
    }
    let bar = crate::chrome::meter(folder.completion, BAR_CELLS.min(room));
    format!("{bar} {}% {word}", folder.completion)
}

struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    /// A section title, from a key.
    fn heading(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.title(text);
    }

    /// A section title, from words that are already the person's.
    ///
    /// A folder's label and a machine's name are TEXT somebody typed, not a
    /// key — looking one up would draw `[my pictures]` on the screen.
    fn title(&mut self, text: String) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: Spot::Nothing,
                disabled: false,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                layout::elide_end(&text, self.room),
                tokens::color::TEXT_HEADING,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            spot: Spot::Nothing,
            disabled: false,
        });
    }

    /// A sentence that stands for a whole section.
    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.said(text, tokens::color::TEXT_DIM, Spot::Nothing);
    }

    /// Prose, in a tone of its own.
    ///
    /// IT WRAPS, and it used to be cut — one `Row` per line, through the one
    /// function the eight views of this shape share. See
    /// [`crate::chrome::note_lines`]: what an ellipsis takes off a sentence
    /// like `SYNCTHING_INTERFACE_ABOUT` is the end, which is where the
    /// consequence is. A daemon's complaint comes through here too, and those
    /// are the longest strings this view ever draws.
    ///
    /// THE SPOT IS THE FIRST LINE'S. A spot is a row's identity, kept so the
    /// cursor can find it again after a redraw; the same one on four rows
    /// would name four things at once.
    fn said(&mut self, text: String, colour: tokens::Color, spot: Spot) {
        for (n, line) in crate::chrome::note_lines(&text, self.room)
            .into_iter()
            .enumerate()
        {
            self.out.push(Row {
                fields: vec![Self::piece(self.left, line, colour)],
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: if n == 0 { spot.clone() } else { Spot::Nothing },
                disabled: false,
            });
        }
    }

    /// The drawer's own line: a chevron, and the word.
    ///
    /// THE CHEVRON IS THE DRAWER'S AND NOBODY ELSE'S. The owner refused one on
    /// a row that acts, saying it is what collapse and expand use; by that
    /// argument it belongs here, and this is the only place in the view that
    /// draws one.
    fn drawer(&mut self, key: &str, id: &'static str, open: bool) {
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: Spot::Nothing,
                disabled: false,
            });
        }
        let focused = self.cursor == self.out.len();
        let glyph = if open {
            tokens::icon::COLLAPSE
        } else {
            tokens::icon::EXPAND
        };
        let colour = if focused {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_HEADING
        };
        // THE GLYPH IS ONE CELL AND THE WORD STARTS AFTER IT, with a space
        // between: a mark against the letters is the "glifo encostado na
        // borda" the owner has had to point out before.
        self.out.push(Row {
            fields: vec![
                Self::piece(self.left, glyph.to_string(), colour),
                Self::piece(
                    self.left + 2,
                    layout::elide_end(&self.texts.t(key), self.room.saturating_sub(2)),
                    colour,
                ),
            ],
            focused,
            heading: false,
            target: Target::Drawer(id),
            spot: Spot::Drawer(id),
            disabled: false,
        });
    }

    /// A row that opens another level.
    fn door(&mut self, key: &str, level: Level, id: &'static str, disabled: bool) {
        let label = self.texts.t(key);
        let value = self.texts.t(ronoctua_i18n::key::SYNCTHING_OPEN);
        self.pair(label, value, Target::Door(level), Spot::Door(id), disabled);
    }

    /// A row that does something.
    fn deed(&mut self, key: &str, deed: Deed, disabled: bool) {
        let label = self.texts.t(key);
        let value = self.texts.t(ronoctua_i18n::key::SYNCTHING_DO);
        let id = deed.id();
        self.pair(label, value, Target::Deed(deed), Spot::Deed(id), disabled);
    }

    /// A row a key acts on.
    fn pair(&mut self, label: String, value: String, target: Target, spot: Spot, disabled: bool) {
        let focused = self.cursor == self.out.len();
        // THE LABEL SAYS WHETHER THE ROW IS A DOOR, in a hue no value wears.
        // It replaced a mark in front of the row, which the owner refused —
        // and the chevron they named is the drawer's.
        let acts = !matches!(target, Target::Nothing);
        let kind = if disabled {
            crate::chrome::Kind::Disabled
        } else {
            crate::chrome::Kind::of(acts)
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
            focused,
            heading: false,
            target,
            spot,
            disabled,
        });
    }
}

#[cfg(test)]
mod tests;
