//! The player — one view, three doors: the footer (`LEADER p p`), a window
//! (`LEADER p w`) and the column on the right (`LEADER p c`).
//!
//! # What is here
//!
//! Which rows exist, what a key does on each, and what a door must do about
//! it. Nothing here plays anything: `ronoctua-player` does, in a process of
//! its own, and every door is a client of its socket. The shape is
//! `camera_settings.rs`, down to the rings.
//!
//! # Every value is a ring
//!
//! `←`/`→` turn the value on the row and `RET` does what a click on it does
//! (invariant 19): on the position it jumps ahead, on the volume it mutes, on
//! a track it takes the next one, on an item of the queue it plays it.
//!
//! # The files, from the player itself
//!
//! A second LEVEL of the view (`f`, the `Browse files` row, or `LEADER p l`):
//! the music, videos and music videos folders as a ring, the folders under
//! the one that is open, and the files in it the player plays
//! (`player_browse::glyph_if_played`). `RET` enters a folder or plays a file alone, `a`
//! puts one in the queue — a folder whole — `DEL` goes up, and `Esc` goes
//! back to the view, the bluetooth view's way with its levels.
//!
//! # Every door types
//!
//! A file or an address to play, one to add to the queue, a video to follow
//! and the other machine a pairing is with are a [`Question`] the view hands
//! its door (`Errand::Ask`): the footer asks it on its minibuffer, a window
//! and the column on their own header's line (`door::Ui::ask`) — the view
//! stays open under it in both. What was typed comes back to ONE function,
//! [`answered`], whichever door typed it, and a pairing's code is a secret
//! there: asked masked, never logged, and only ever sent to the player.
//!
//! # The bars are controls in every door
//!
//! The position and the volume are drawn as bars ([`Slider`]), and a press on
//! one sets the value there and holding drags it — [`slider_at`] and
//! [`slid`], the one arithmetic both doors ask, beside the window's own time
//! bar and volume track.

use crate::files::{Field, Hit as FilesHit, Ink};
use ronoctua_player::{
    remote::Role,
    wire::{Ask, Repeat, State, Track, Trouble},
};
use ronoctua_tokens as tokens;

/// The keys this view teaches, and where each one applies.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "player-down", Where::Anywhere),
    ("<up>", "player-up", Where::Anywhere),
    ("<left>", "player-less", Where::OnAValue),
    ("<right>", "player-more", Where::OnAValue),
    ("RET", "player-enter", Where::OnARow),
    ("SPC", "player-toggle", Where::Anywhere),
    ("s", "player-stop", Where::Anywhere),
    ("n", "player-next", Where::Anywhere),
    ("p", "player-previous", Where::Anywhere),
    ("d", "player-remove", Where::OnRemovable),
    ("f", "player-browse", Where::OutOfTheFiles),
    ("a", "player-add", Where::OnAFile),
    ("DEL", "player-parent", Where::InTheFiles),
    ("P", "player-play-now", Where::OnAFile),
    ("c", "player-clear", Where::OutOfTheFiles),
    ("-", "player-quieter", Where::Anywhere),
    ("=", "player-louder", Where::Anywhere),
    ("o", "player-only", Where::Anywhere),
    ("m", "player-maximize", Where::Anywhere),
    ("i", "player-pip", Where::Anywhere),
    ("0", "player-mute", Where::Anywhere),
];

/// Where a key applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns.
    OnAValue,
    /// On a row that does something.
    OnARow,
    /// On an item of the queue, or a video playing in step.
    OnRemovable,
    /// Anywhere but among the files.
    OutOfTheFiles,
    /// Among the files.
    InTheFiles,
    /// On a folder or a file of the listing, or the folder that is open.
    OnAFile,
}

impl Where {
    fn shown(self, target: &Target) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnAValue => target.turns(),
            Self::OnARow => target.acts(),
            Self::OnRemovable => matches!(
                target,
                Target::Item(_) | Target::Follower(_) | Target::Host(_) | Target::Guest(_)
            ),
            Self::OutOfTheFiles => !target.among_the_files(),
            Self::InTheFiles => target.among_the_files(),
            Self::OnAFile => matches!(
                target,
                Target::Folder(_) | Target::File(_) | Target::PlayFolder | Target::EnqueueFolder
            ),
        }
    }
}

/// A key, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up a row.
    Up,
    /// Down a row.
    Down,
    /// The value back a step.
    Less,
    /// The value on a step.
    More,
    /// What a click on the row does.
    Enter,
    /// Play or pause, wherever the cursor is.
    Toggle,
    /// Stop: the queue stays, and nothing plays.
    Stop,
    /// The next item of the queue.
    Next,
    /// The previous one.
    Previous,
    /// Take the item (or the follower) off.
    Remove,
    /// Open the files.
    Browse,
    /// Put the file, or the folder whole, in the queue.
    Add,
    /// The folder above.
    Parent,
    /// The file (or folder) alone, now: the queue replaced by it.
    PlayNow,
    /// Empty the queue, after a question.
    Clear,
    /// The volume down a step.
    Quieter,
    /// The volume up a step.
    Louder,
    /// Only the picture, in this door.
    Only,
    /// The picture on the whole screen.
    Maximize,
    /// The picture in a small window in the corner.
    Pip,
    /// Mute or unmute.
    Mute,
    /// Leave the view.
    Escape,
}

/// What a key does on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A fact: nothing to do.
    Nothing,
    /// Play or pause.
    Playing,
    /// Where in the item: `←`/`→` seek, `RET` jumps ahead.
    Position,
    /// The volume: `←`/`→` turn it, `RET` mutes.
    Volume,
    /// The speed ring.
    Speed,
    /// The repeat ring.
    Repeat,
    /// Shuffle on or off.
    Shuffle,
    /// The audio track ring.
    Audio,
    /// The subtitle ring, "none" included.
    Subtitles,
    /// An item of the queue, by its index.
    Item(usize),
    /// A video playing in step, by its index.
    Follower(usize),
    /// A paired machine this one can join, by its name: `RET` joins (or
    /// leaves), `←`/`→` turn the role, `d` forgets it.
    Host(String),
    /// A paired machine that can join this one: `d` forgets it.
    Guest(String),
    /// Show a pairing code.
    Invite,
    /// Lyrics asked of `lrclib.net`, on or off.
    LyricsOnline,
    /// Sponsored segments skipped, on or off.
    Sponsors,
    /// One of SponsorBlock's categories, skipped or not.
    Skip(&'static str),
    /// A door: a command of the registry.
    Run(&'static str),
    /// Pair with another machine: its name, then the code it shows.
    Pair,
    /// Open the files.
    Browse,
    /// Which of the three places the files start from: a ring.
    Place,
    /// The folder above the one that is open.
    Up,
    /// Play the open folder, every file under it.
    PlayFolder,
    /// Put the open folder in the queue, every file under it.
    EnqueueFolder,
    /// A folder of the listing: `RET` enters it, `a` queues it whole.
    Folder(std::path::PathBuf),
    /// A file of the listing: `RET` puts it in the queue, `P` plays it alone.
    File(std::path::PathBuf),
    /// A path or an address, typed: a [`Question`] the door asks.
    Typed,
    /// Pick another video to play in step with this sound — the files, for
    /// that purpose.
    FollowPick,
    /// Empty the queue: the glyph beside its title, and `c`.
    ClearQueue,
    /// An answer to the question on screen.
    Answer(bool),
}

impl Target {
    const fn turns(&self) -> bool {
        matches!(
            self,
            Self::Playing
                | Self::Position
                | Self::Volume
                | Self::Speed
                | Self::Repeat
                | Self::Shuffle
                | Self::Audio
                | Self::Subtitles
                | Self::LyricsOnline
                | Self::Sponsors
                | Self::Skip(_)
                | Self::Host(_)
                | Self::Place
        )
    }

    const fn among_the_files(&self) -> bool {
        matches!(
            self,
            Self::Place
                | Self::Up
                | Self::PlayFolder
                | Self::EnqueueFolder
                | Self::Folder(_)
                | Self::File(_)
                | Self::Typed
        )
    }

    const fn acts(&self) -> bool {
        !matches!(self, Self::Nothing)
    }
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Placed and cut.
    pub fields: Vec<Field>,
    /// The cursor is on it.
    pub focused: bool,
    /// The cursor skips it.
    pub heading: bool,
    /// What a key does here.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: String,
    /// When the line is a row of a GRID — the files, laid out in columns —
    /// or carries a control beside its title: where each one is, and what it
    /// does. `target` is then the one the cursor is on.
    pub cells: Vec<Cell>,
    /// The bar the value is drawn as, when it is one — a control a press
    /// sets and a drag turns ([`slider_at`]).
    pub slider: Option<Slider>,
}

/// Which value a bar of the view turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slid {
    /// Where in the item: a press seeks there.
    Position,
    /// The volume, up to [`VOLUME_MOST`].
    Volume,
}

/// A bar on a row: where it is drawn, in the row's own cells (a field's
/// `at`), and which value it turns.
///
/// ONE ANSWER for the painter and the pointer: the bar is the value's first
/// cells, placed by `chrome::field_pair`, and this is read back from the field
/// that was placed — so a press is taken where the bar was drawn, in both
/// doors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slider {
    /// The value it turns.
    pub of: Slid,
    /// Its first cell.
    pub from: u16,
    /// How many cells it takes.
    pub cells: u16,
}

impl Slider {
    /// Whether `column` — fractional cells from the rows' origin — is on it.
    #[must_use]
    pub fn under(self, column: f64) -> bool {
        column >= f64::from(self.from) && column < f64::from(self.from + self.cells)
    }

    /// How far along it `column` is, clamped to its two ends: a drag that
    /// leaves the bar keeps the end it left by.
    #[must_use]
    pub fn along(self, column: f64) -> f64 {
        ((column - f64::from(self.from)) / f64::from(self.cells.max(1))).clamp(0.0, 1.0)
    }
}

/// The bar on row `n` under `column` (fractional cells from the rows'
/// origin), when there is one there — what a press in either door asks.
#[must_use]
pub fn slider_at(rows: &[Row], n: usize, column: f64) -> Option<Slider> {
    rows.get(n)?.slider.filter(|b| b.under(column))
}

/// What turning a bar to `fraction` of its length asks the player — the same
/// arithmetic as the window's time bar and volume track.
#[must_use]
pub fn slid(of: Slid, fraction: f64, state: Option<&State>) -> Errand {
    let fraction = fraction.clamp(0.0, 1.0);
    match of {
        Slid::Position => {
            state
                .and_then(|s| s.duration)
                .filter(|d| *d > 0.0)
                .map_or(Errand::Nothing, |d| {
                    Errand::Tell(Ask::Seek {
                        seconds: d * fraction,
                        relative: false,
                    })
                })
        }
        Slid::Volume => {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let percent = (fraction * f64::from(VOLUME_MOST)).round() as u8;
            Errand::Tell(Ask::Volume { percent })
        }
    }
}

/// One entry of a row laid out in columns, or a control on a title's line.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Its first column, as a field's `at`.
    pub from: u16,
    /// How many columns it takes.
    pub width: u16,
    /// What a key or a click does on it.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: String,
}

/// What a choice of the files is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Purpose {
    /// Into the queue: `RET` adds, `P` plays alone.
    #[default]
    Queue,
    /// Another video, played in step with this sound.
    Follow,
}

/// A yes-or-no question on screen, and what each answer does.
#[derive(Debug, Clone, PartialEq)]
struct Choice {
    /// What is asked, already in words.
    sentence: String,
    /// Where the cursor was, to go back to once it is answered.
    back: (usize, usize),
    /// What `yes` does.
    yes: Box<Errand>,
    /// What `no` does — and `Esc`.
    no: Box<Errand>,
}

/// The command that plays a path or an address in place of the queue.
pub const PLAY: &str = "player-play-file";
/// The one that adds a path or an address to the queue.
pub const ENQUEUE: &str = "player-enqueue";
/// The one that plays another video in step with this sound.
pub const FOLLOW: &str = "player-follow";
/// The one that pairs with another machine: its name, first.
pub const PAIR: &str = "player-pair";
/// The pairing's second question: the code the other machine shows.
pub const PAIR_CODE: &str = "player-pair-code";

/// A line the door asks — its minibuffer in the footer, its header's line in
/// a window — and whether what is typed on it is a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question {
    /// Which question: the `ask-<id>` it is drawn with, and the id its answer
    /// comes back to [`answered`] under.
    pub id: &'static str,
    /// Masked on screen, left out of every log, and never in an argv.
    pub secret: bool,
}

/// The question a command of the registry asks, when it asks one — what the
/// footer opens for `LEADER p o` and its siblings, typed from no view.
#[must_use]
pub fn question(id: &str) -> Option<Question> {
    Some(match id {
        PLAY => Question {
            id: PLAY,
            secret: false,
        },
        ENQUEUE => Question {
            id: ENQUEUE,
            secret: false,
        },
        FOLLOW => Question {
            id: FOLLOW,
            secret: false,
        },
        PAIR => Question {
            id: PAIR,
            secret: false,
        },
        PAIR_CODE => Question {
            id: PAIR_CODE,
            secret: true,
        },
        _ => return None,
    })
}

/// A pairing's code: six digits read off another screen, exchanged for the
/// key two machines share. A secret — its `Debug` says nothing, so no log line
/// that prints an errand prints it.
#[derive(Clone, PartialEq, Eq)]
pub struct Code(String);

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Code(…)")
    }
}

/// What a door remembers between a pairing's two questions: the machine's
/// name, until its code is typed. The DOOR's and not the view's, because the
/// footer asks it from the leader (`LEADER p m`) with no view open.
#[derive(Debug, Default)]
pub struct Pairing {
    name: Option<String>,
}

impl Pairing {
    /// The question was let go: the name goes with it, so a code typed later
    /// under another question is not taken for this one.
    pub fn let_go(&mut self) {
        self.name = None;
    }
}

/// What was typed for one of the player's questions, from whichever door
/// typed it — the ONE place an answer turns into an order.
///
/// An empty answer is never mind, in both doors. A path or an address is
/// [`typed`]'s; a machine's name asks for its code next; the code pairs with
/// the name asked before it, and nothing else.
pub fn answered(id: &str, said: &str, pairing: &mut Pairing) -> Errand {
    match id {
        PAIR => {
            let name = said.trim();
            if name.is_empty() {
                pairing.let_go();
                return Errand::Nothing;
            }
            pairing.name = Some(name.to_owned());
            Errand::Ask(Question {
                id: PAIR_CODE,
                secret: true,
            })
        }
        PAIR_CODE => {
            let code = said.trim();
            match pairing.name.take() {
                Some(to) if !code.is_empty() => Errand::Pair {
                    to,
                    code: Code(code.to_owned()),
                },
                _ => Errand::Nothing,
            }
        }
        _ => typed(id, said).map_or(Errand::Nothing, Errand::Tell),
    }
}

/// What the door has to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Draw again.
    Nothing,
    /// Shut the view.
    Close,
    /// Run a command of the registry: a door.
    Run(&'static str),
    /// Ask a line of the door — see [`Question`].
    Ask(Question),
    /// Pair with another machine: its name and the code it shows.
    Pair {
        /// Its name on the tailnet.
        to: String,
        /// The code, a secret.
        code: Code,
    },
    /// Tell the player.
    Tell(Ask),
    /// Tell the player, and say what was done — the sentence already in
    /// words.
    TellSaying(Ask, String),
    /// Say a sentence, already in words.
    Said(String),
    /// Write the preferences, and tell the player to reread them.
    Keep(ronoctua_player::prefs::Preferences),
    /// Show the picture alone, or on the whole screen — the DOOR's to do,
    /// because only a door has a picture to show.
    Show(Show),
}

/// How the picture is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Show {
    /// Alone in the door, everything else hidden.
    Only,
    /// Alone, on the whole screen.
    Maximized,
    /// Alone, in a small window floating in the bottom-right corner —
    /// picture-in-picture.
    Small,
}

/// Where the footer puts the picture, in the region's cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panel {
    /// Its first column.
    pub from: u16,
    /// How many columns it takes.
    pub cells: u16,
}

/// The footer's picture: beside the rows, or the whole region.
///
/// # Beside the rows, the column's width
///
/// The window has a band above its rows for the picture; the footer's region
/// is the other shape — the whole screen across, a few lines down — so the
/// picture goes at the right end of it, beside the rows, as wide as the
/// column the player sits in (`space.column.width`), and the region's own bar
/// keeps its last cell. Only the picture (`o`) takes the whole region, the
/// window's `Only` in this door. A region too narrow to spare the column —
/// the rows would keep less than half — shows the rows whole and no picture.
///
/// A FUNCTION OF THE WIDTH ALONE: the rows are laid out at what is left
/// ([`Panel::from`]), and how many rows there are is what sizes the region —
/// a panel that asked the region's height back would be the footer asking
/// itself.
#[must_use]
pub fn footer_panel(columns: u16, only: bool) -> Option<Panel> {
    let usable = columns.saturating_sub(tokens::space::FILES_EDGERIGHT);
    if only {
        return (usable > 0).then_some(Panel {
            from: 0,
            cells: usable,
        });
    }
    let cells = tokens::space::COLUMN_WIDTH;
    (usable >= cells.saturating_mul(2)).then_some(Panel {
        from: usable - cells,
        cells,
    })
}

/// How far `←`/`→` move the position, in seconds.
pub const SEEK_STEP: f64 = 10.0;
/// How far `RET` on the position jumps, in seconds.
pub const JUMP_STEP: f64 = 60.0;
/// How far `←`/`→` move the volume, in percent.
pub const VOLUME_STEP: u8 = 5;
/// The loudest the volume ring goes: mpv amplifies past 100.
pub const VOLUME_MOST: u8 = 130;
/// The speeds the ring walks.
pub const SPEEDS: [f64; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
/// How many lines of lyrics the view shows around the current one.
pub const LYRICS_AROUND: usize = 2;

/// The view.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    moved: bool,
    /// `player.json`, read when the view opens and kept in step with what
    /// this door writes.
    prefs: ronoctua_player::prefs::Preferences,
    /// The files, when that level is open.
    files: Option<Files>,
    /// Where the files were left: opening them again goes back there.
    left_at: Option<(usize, std::path::PathBuf)>,
    /// The folder just left by going up, for the cursor to land on.
    came_from: Option<std::path::PathBuf>,
    /// Which entry of a grid row the cursor is on.
    cell: usize,
    /// The yes-or-no question on screen, when one is.
    asking: Option<Choice>,
    /// A control on a title's line, clicked: what the next `RET` does.
    clicked_control: Option<Target>,
}

/// The files level: which place, and the folder that is open.
#[derive(Debug, Clone)]
struct Files {
    place: usize,
    listing: crate::player_browse::Listing,
    /// What choosing a file does.
    purpose: Purpose,
}

/// `m:ss`, or `h:mm:ss` from an hour on.
#[must_use]
pub fn clock(seconds: f64) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let s = seconds.max(0.0).floor() as u64;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The name an item is shown by: its title, or the last part of where it is.
#[must_use]
pub fn item_name(item: &ronoctua_player::wire::Item) -> String {
    if let Some(t) = item.title.as_deref().filter(|t| !t.trim().is_empty()) {
        return t.to_owned();
    }
    let l = item.location.trim_end_matches('/');
    l.rsplit('/').next().unwrap_or(l).to_owned()
}

/// What is playing, in the words the modeline's mark says under the pointer.
#[must_use]
pub fn now_playing(state: &State) -> Option<String> {
    let item = state.current.and_then(|i| state.queue.get(i))?;
    let title = state
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| item_name(item));
    Some(
        match state.artist.as_deref().filter(|a| !a.trim().is_empty()) {
            Some(a) => format!("{a} — {title}"),
            None => title,
        },
    )
}

/// A trouble, in a sentence.
#[must_use]
pub fn trouble_sentence(t: &Trouble, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    match t {
        Trouble::Unplayable { why } => {
            texts.t_with(key::PLAYER_TROUBLE_UNPLAYABLE, &[("why", why)])
        }
        Trouble::NoDecoder { program } => {
            texts.t_with(key::PLAYER_TROUBLE_NO_DECODER, &[("program", program)])
        }
        Trouble::Unsupported { format } => {
            texts.t_with(key::PLAYER_TROUBLE_UNSUPPORTED, &[("format", format)])
        }
        Trouble::NoEngine { why } => texts.t_with(key::PLAYER_TROUBLE_NO_ENGINE, &[("why", why)]),
        Trouble::Remote { why } => texts.t_with(key::PLAYER_TROUBLE_REMOTE, &[("why", why)]),
    }
}

/// The glyph the modeline's mark draws: whether something is playing.
#[must_use]
pub fn mark(state: &State) -> Option<char> {
    state.current?;
    Some(if state.playing {
        tokens::icon::PLAYER
    } else {
        tokens::icon::PAUSE
    })
}

/// A track's name: its title, its language, its codec — whichever it has.
fn track_name(t: &Track) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(title) = t.title.as_deref().filter(|s| !s.is_empty()) {
        parts.push(title.to_owned());
    }
    if let Some(lang) = t.lang.as_deref().filter(|s| !s.is_empty()) {
        parts.push(lang.to_owned());
    }
    if parts.is_empty()
        && let Some(codec) = t.codec.as_deref()
    {
        parts.push(codec.to_owned());
    }
    if parts.is_empty() {
        parts.push(t.id.to_string());
    }
    parts.join(" · ")
}

/// The next (or previous) track of `kind`, with `none` in the ring when
/// `with_none` — a subtitle can be switched off, an audio track cannot.
fn next_track(state: &State, kind: &str, forward: bool, with_none: bool) -> Option<Option<i64>> {
    let mut ring: Vec<Option<i64>> = Vec::new();
    if with_none {
        ring.push(None);
    }
    ring.extend(
        state
            .tracks
            .iter()
            .filter(|t| t.kind == kind)
            .map(|t| Some(t.id)),
    );
    if ring.len() < 2 {
        return None;
    }
    let now = state
        .tracks
        .iter()
        .find(|t| t.kind == kind && t.selected)
        .map(|t| t.id);
    let at = ring.iter().position(|x| *x == now).unwrap_or(0);
    let n = ring.len();
    Some(
        ring[if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }],
    )
}

impl View {
    /// A view with the cursor on the first row that does something.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            cursor: 1,
            moved: false,
            prefs: ronoctua_player::prefs::store().read(),
            files: None,
            left_at: None,
            came_from: None,
            cell: 0,
            asking: None,
            clicked_control: None,
        }
    }

    /// The files, where they were left, or the music folder the first time.
    fn open_the_files(&mut self, purpose: Purpose) {
        let (place, at) = self.left_at.clone().unwrap_or_else(|| {
            let places = crate::player_browse::places();
            (0, places[0].1.clone())
        });
        self.files = Some(Files {
            place,
            listing: crate::player_browse::list(&at),
            purpose,
        });
        self.cursor = 0;
        self.cell = 0;
        self.moved = false;
    }

    /// Another folder, the cursor on `on` when it is one of its rows.
    fn go_to(&mut self, at: &std::path::Path, on: Option<&std::path::Path>) {
        if let Some(f) = self.files.as_mut() {
            f.listing = crate::player_browse::list(at);
            self.left_at = Some((f.place, at.to_path_buf()));
        }
        self.cursor = 0;
        self.cell = 0;
        self.moved = false;
        self.came_from = on.map(std::path::Path::to_path_buf);
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The hint line.
    #[must_use]
    pub fn hint(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(state, columns, texts);
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

    /// A click on row `n`, at `column` (a field's `at`); `true` when the
    /// door should run what `RET` does — the second click on the same place,
    /// or a click on a control beside a title, which acts at once.
    pub fn clicked_at(&mut self, n: usize, column: u16, rows: &[Row]) -> bool {
        let Some(row) = rows.get(n) else {
            return false;
        };
        let cell = row
            .cells
            .iter()
            .position(|c| column >= c.from && column < c.from + c.width);
        // A CONTROL ON A TITLE'S LINE acts on the click: a title is not a
        // place the cursor stops, so there is no first click to spend on it.
        if row.heading {
            if let Some(c) = cell {
                self.clicked_control = Some(row.cells[c].target.clone());
                return true;
            }
            return false;
        }
        let again = self.cursor == n && (row.cells.is_empty() || cell == Some(self.cell));
        self.moved = true;
        self.cursor = n;
        if let Some(c) = cell {
            self.cell = c;
        }
        again
    }

    /// A press on the bar of row `n`: the row becomes the cursor's, and the
    /// door sets the value where the press landed ([`slid`]) — a bar acts on
    /// the press, as the window's time bar does, and holding drags it.
    pub fn grabbed(&mut self, n: usize) {
        self.cursor = n;
        self.cell = 0;
        self.moved = true;
    }

    /// The files, open on `at` for `purpose` — what a test of a door stands
    /// on, with no folder of the machine's read.
    #[cfg(test)]
    pub fn files_at(&mut self, at: &std::path::Path, purpose: Purpose) {
        self.left_at = Some((0, at.to_path_buf()));
        self.open_the_files(purpose);
    }

    /// A click anywhere on row `n` — the first column.
    #[cfg(test)]
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let column = rows
            .get(n)
            .and_then(|r| r.cells.get(self.cell).or_else(|| r.cells.first()))
            .map_or(0, |c| c.from);
        self.clicked_at(n, column, rows)
    }

    /// The cursor back on a real row after the rows changed.
    pub fn settle_in(&mut self, state: Option<&State>, columns: u16, texts: &ronoctua_i18n::Texts) {
        let rows = self.rows(state, columns, texts);
        self.clamp(&rows);
        // AMONG THE FILES the cursor lands on the folder just left, or on the
        // first entry: the rows above them are the same in every folder, and
        // the entries are what a person came to walk.
        if !self.moved && self.files.is_some() {
            let came = self.came_from.take();
            // THE FOLDER JUST LEFT, or else the first real entry — `..` is
            // the way back, not what a person opened the folder for, and it
            // is landed on only when there is nothing else.
            let wanted: [&dyn Fn(&Target) -> bool; 3] = [
                &|t| matches!((t, came.as_deref()), (Target::Folder(p), Some(c)) if p == c),
                &|t| came.is_none() && matches!(t, Target::Folder(_) | Target::File(_)),
                &|t| matches!(t, Target::Up),
            ];
            // THE ENTRIES ARE CELLS of a grid now: they are looked for in
            // each, and the cursor lands on the row AND the column.
            for is_it in wanted {
                for (i, r) in rows.iter().enumerate().filter(|(_, r)| !r.heading) {
                    if let Some(c) = r.cells.iter().position(|c| is_it(&c.target)) {
                        self.cursor = i;
                        self.cell = c;
                        return;
                    }
                }
            }
        }
        if !self.moved
            && rows.get(self.cursor).is_some_and(|r| !r.target.acts())
            && let Some(i) = rows.iter().position(|r| !r.heading && r.target.acts())
        {
            self.cursor = i;
        }
    }

    /// The cursor stays on its row by identity.
    pub fn follow(
        &mut self,
        spot: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(state, columns, texts);
        if let Some(i) = rows.iter().position(|r| r.spot == spot) {
            self.cursor = i;
        }
        self.clamp(&rows);
    }

    fn clamp(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        self.cursor = self.cursor.min(rows.len() - 1);
        if rows[self.cursor].heading {
            let after = (self.cursor..rows.len()).find(|&i| !rows[i].heading);
            let before = (0..=self.cursor).rev().find(|&i| !rows[i].heading);
            if let Some(i) = after.or(before) {
                self.cursor = i;
            }
        }
    }

    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        loop {
            if down {
                if at + 1 >= rows.len() {
                    return;
                }
                at += 1;
            } else {
                if at == 0 {
                    return;
                }
                at -= 1;
            }
            if !rows[at].heading {
                self.cursor = at;
                self.moved = true;
                self.cell = self.cell.min(rows[at].cells.len().saturating_sub(1));
                return;
            }
        }
    }

    /// Across a grid: `←`/`→` move between its columns, and past the foot
    /// of one column `↓` goes on at the head of the next — the order the
    /// names are read in, which is down each column (`Builder::grid`).
    fn across(&mut self, rows: &[Row], press: Press) -> bool {
        let Some(row) = rows.get(self.cursor) else {
            return false;
        };
        if row.cells.len() < 2 && !matches!(press, Press::Up | Press::Down) {
            return false;
        }
        let grid: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.cells.is_empty() && !r.heading)
            .map(|(i, _)| i)
            .collect();
        let (Some(&first), Some(&last)) = (grid.first(), grid.last()) else {
            return false;
        };
        if !grid.contains(&self.cursor) {
            return false;
        }
        match press {
            Press::Less if self.cell > 0 => self.cell -= 1,
            Press::More if self.cell + 1 < row.cells.len() => self.cell += 1,
            Press::Down if self.cursor == last && self.cell + 1 < rows[first].cells.len() => {
                self.cursor = first;
                self.cell += 1;
            }
            Press::Up if self.cursor == first && self.cell > 0 => {
                self.cell -= 1;
                // THE FOOT OF THE COLUMN BEFORE, which may be shorter: the
                // last row that has an entry in it.
                self.cursor = grid
                    .iter()
                    .rev()
                    .copied()
                    .find(|&i| rows[i].cells.len() > self.cell)
                    .unwrap_or(first);
            }
            _ => return false,
        }
        self.moved = true;
        true
    }

    /// A key.
    pub fn key(
        &mut self,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // A QUESTION ON SCREEN takes every key until it is answered.
        if self.asking.is_some() {
            return self.answering(press, state, columns, texts);
        }
        self.settle_in(state, columns, texts);
        let rows = self.rows(state, columns, texts);
        // A CONTROL BESIDE A TITLE, clicked: what it does, now.
        if press == Press::Enter
            && let Some(target) = self.clicked_control.take()
        {
            return self.act(&target, press, state, columns, texts);
        }
        match press {
            // `Esc` AMONG THE FILES goes back to the view, one level, as
            // every view with levels does.
            Press::Escape if self.files.is_some() => {
                self.files = None;
                self.cursor = 0;
                self.moved = false;
                self.settle_in(state, columns, texts);
                return Errand::Nothing;
            }
            Press::Escape => return Errand::Close,
            Press::Browse => {
                if self.files.is_none() {
                    self.open_the_files(Purpose::Queue);
                    self.settle_in(state, columns, texts);
                }
                return Errand::Nothing;
            }
            Press::Clear => return self.ask_to_clear(state, texts),
            Press::Quieter | Press::Louder => {
                let now = state.map_or(100, |s| s.volume);
                return Errand::Tell(Ask::Volume {
                    percent: if press == Press::Louder {
                        now.saturating_add(VOLUME_STEP).min(VOLUME_MOST)
                    } else {
                        now.saturating_sub(VOLUME_STEP)
                    },
                });
            }
            Press::Parent => {
                if let Some(at) = self.files.as_ref().map(|f| f.listing.at.clone())
                    && let Some(up) = at.parent()
                {
                    self.go_to(up, Some(&at));
                    self.settle_in(state, columns, texts);
                }
                return Errand::Nothing;
            }
            Press::Up | Press::Down => {
                if !self.across(&rows, press) {
                    self.walk(&rows, press == Press::Down);
                }
                return Errand::Nothing;
            }
            // `←`/`→` ON A GRID move between its columns; elsewhere they turn
            // the value of the row, below.
            Press::Less | Press::More if self.across(&rows, press) => return Errand::Nothing,
            Press::Toggle => return Errand::Tell(Ask::Toggle),
            Press::Stop => return Errand::Tell(Ask::Stop),
            Press::Only => return Errand::Show(Show::Only),
            Press::Maximize => return Errand::Show(Show::Maximized),
            Press::Pip => return Errand::Show(Show::Small),
            Press::Mute => return Errand::Tell(Ask::Mute),
            Press::Next => return Errand::Tell(Ask::Next),
            Press::Previous => return Errand::Tell(Ask::Previous),
            Press::Less
            | Press::More
            | Press::Enter
            | Press::Remove
            | Press::Add
            | Press::PlayNow => {}
        }
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        self.act(&target, press, state, columns, texts)
    }

    /// What a press does on a target — a row's, a grid cell's, or a
    /// control's beside a title.
    fn act(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let target = target.clone();
        // A NEW FOLDER PUTS THE CURSOR BACK ON ZERO, which is its heading;
        // settling it here is what lands it on an entry. The doors never
        // settle after a key, and a cursor on a heading teaches no key of
        // the level — the photograph showed exactly that.
        if target.among_the_files() {
            let errand = self.among_the_files(&target, press, state, texts);
            self.settle_in(state, columns, texts);
            return errand;
        }
        if press == Press::Enter {
            match target {
                Target::Browse => {
                    self.open_the_files(Purpose::Queue);
                    self.settle_in(state, columns, texts);
                    return Errand::Nothing;
                }
                Target::FollowPick => {
                    self.open_the_files(Purpose::Follow);
                    self.settle_in(state, columns, texts);
                    return Errand::Nothing;
                }
                Target::ClearQueue => return self.ask_to_clear(state, texts),
                _ => {}
            }
        }
        // THE TWO SWITCHES ARE THIS VIEW'S: a ring of two, any key turns it.
        let mut p = self.prefs.clone();
        match target {
            Target::LyricsOnline if press != Press::Remove => {
                p.lyrics_online = !p.lyrics_online;
                self.prefs = p.clone();
                return Errand::Keep(p);
            }
            Target::Sponsors if press != Press::Remove => {
                p.sponsorblock = !p.sponsorblock;
                self.prefs = p.clone();
                return Errand::Keep(p);
            }
            Target::Skip(category) if press != Press::Remove => {
                if let Some(i) = p.skipped.iter().position(|c| c == category) {
                    p.skipped.remove(i);
                } else {
                    p.skipped.push(category.to_owned());
                }
                self.prefs = p.clone();
                return Errand::Keep(p);
            }
            _ => {}
        }
        turn(&target, press, state)
    }

    /// A press on a row of the files.
    fn among_the_files(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use ronoctua_i18n::{Arg, key};
        let Some((open, purpose)) = self
            .files
            .as_ref()
            .map(|f| (f.listing.at.clone(), f.purpose))
        else {
            return Errand::Nothing;
        };
        let queued: Vec<String> = state
            .map(|s| s.queue.iter().map(|i| i.location.clone()).collect())
            .unwrap_or_default();
        let named = |p: &std::path::Path| file_name(p);
        let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
        match (target, press) {
            (Target::Place, Press::Less | Press::More | Press::Enter) => {
                let places = crate::player_browse::places();
                let n = places.len();
                let now = self.files.as_ref().map_or(0, |f| f.place);
                let to = if press == Press::Less {
                    (now + n - 1) % n
                } else {
                    (now + 1) % n
                };
                if let Some(f) = self.files.as_mut() {
                    f.place = to;
                }
                self.go_to(&places[to].1, None);
                Errand::Nothing
            }
            (Target::Up, Press::Enter) => {
                if let Some(up) = open.parent() {
                    self.go_to(up, Some(&open));
                }
                Errand::Nothing
            }
            // THE TYPED PATH OR ADDRESS is a question of the door's own line,
            // for the same purpose the files are open for — and the files stay
            // open under it.
            (Target::Typed, Press::Enter) => Errand::Ask(Question {
                id: match purpose {
                    Purpose::Queue => ENQUEUE,
                    Purpose::Follow => FOLLOW,
                },
                secret: false,
            }),
            (Target::Folder(p), Press::Enter) => {
                self.go_to(&p.clone(), None);
                Errand::Nothing
            }
            // IN STEP, a file is the video to follow: chosen, it plays beside
            // this sound, and the files close — the choice was made.
            (Target::File(p), Press::Enter | Press::Add | Press::PlayNow)
                if purpose == Purpose::Follow =>
            {
                self.files = None;
                self.cursor = 0;
                self.moved = false;
                Errand::TellSaying(
                    Ask::Follow {
                        item: p.to_string_lossy().into_owned(),
                    },
                    texts.t_with(key::PLAYER_FOLLOWING, &[("name", &named(p))]),
                )
            }
            (Target::Folder(_) | Target::PlayFolder | Target::EnqueueFolder, _)
                if purpose == Purpose::Follow =>
            {
                Errand::Nothing
            }
            // A FILE, INTO THE QUEUE — `RET` and a double click. Once, unless
            // somebody says twice.
            (Target::File(p), Press::Enter | Press::Add) => {
                let item = p.to_string_lossy().into_owned();
                let added = Errand::TellSaying(
                    Ask::Open {
                        items: vec![item.clone()],
                        replace: false,
                    },
                    texts.t_with(key::PLAYER_ADDED, &[("name", &named(p))]),
                );
                if queued.contains(&item) {
                    self.ask(
                        texts.t_with(key::PLAYER_ALREADY_QUEUED, &[("name", &named(p))]),
                        added,
                        Errand::Nothing,
                    )
                } else {
                    added
                }
            }
            // `P`: THE FILE ALONE, now — the queue replaced by it.
            (Target::File(p), Press::PlayNow) => Errand::TellSaying(
                Ask::Open {
                    items: vec![p.to_string_lossy().into_owned()],
                    replace: true,
                },
                texts.t_with(key::PLAYER_PLAYING_NOW, &[("name", &named(p))]),
            ),
            // A FOLDER WHOLE, played now or added.
            (Target::Folder(_), Press::PlayNow)
            | (Target::PlayFolder, Press::Enter | Press::PlayNow) => {
                let folder = match target {
                    Target::Folder(f) => f.clone(),
                    _ => open.clone(),
                };
                let items = gathered(&folder);
                if items.is_empty() {
                    return Errand::Said(texts.t(key::PLAYER_BROWSE_NOTHING_TO_PLAY));
                }
                let n = count(items.len());
                Errand::TellSaying(
                    Ask::Open {
                        items,
                        replace: true,
                    },
                    texts.t_args(key::PLAYER_PLAYING_MANY, &[("count", Arg::Number(n))]),
                )
            }
            (Target::Folder(_), Press::Add)
            | (Target::EnqueueFolder | Target::PlayFolder, Press::Add)
            | (Target::EnqueueFolder, Press::Enter) => {
                let folder = match target {
                    Target::Folder(f) => f.clone(),
                    _ => open.clone(),
                };
                let items = gathered(&folder);
                if items.is_empty() {
                    return Errand::Said(texts.t(key::PLAYER_BROWSE_NOTHING_TO_PLAY));
                }
                let added = |items: Vec<String>| {
                    let n = count(items.len());
                    Errand::TellSaying(
                        Ask::Open {
                            items,
                            replace: false,
                        },
                        texts.t_args(key::PLAYER_ADDED_MANY, &[("count", Arg::Number(n))]),
                    )
                };
                let fresh: Vec<String> = items
                    .iter()
                    .filter(|i| !queued.contains(i))
                    .cloned()
                    .collect();
                if fresh.len() == items.len() {
                    return added(items);
                }
                // SOME ARE THERE ALREADY: again, or only the new ones.
                let repeated = count(items.len() - fresh.len());
                let sentence = texts.t_args(
                    key::PLAYER_ALREADY_QUEUED_MANY,
                    &[
                        ("repeated", Arg::Number(repeated)),
                        ("count", Arg::Number(count(items.len()))),
                    ],
                );
                let no = if fresh.is_empty() {
                    Errand::Said(texts.t(key::PLAYER_NOTHING_NEW))
                } else {
                    added(fresh)
                };
                self.ask(sentence, added(items), no)
            }
            _ => Errand::Nothing,
        }
    }

    /// Puts a question on screen; `no` is where the cursor starts, and what
    /// `Esc` answers — a question that appears on its own answers no.
    fn ask(&mut self, sentence: String, yes: Errand, no: Errand) -> Errand {
        self.asking = Some(Choice {
            sentence,
            back: (self.cursor, self.cell),
            yes: Box::new(yes),
            no: Box::new(no),
        });
        self.cursor = 0;
        self.cell = 0;
        self.moved = false;
        Errand::Nothing
    }

    /// A key while a question is on screen.
    fn answering(
        &mut self,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.settle_in(state, columns, texts);
        let rows = self.rows(state, columns, texts);
        let answer = match press {
            Press::Up | Press::Down => {
                self.walk(&rows, press == Press::Down);
                return Errand::Nothing;
            }
            Press::Escape => false,
            Press::Enter => match rows.get(self.cursor).map(|r| &r.target) {
                Some(Target::Answer(yes)) => *yes,
                _ => return Errand::Nothing,
            },
            _ => return Errand::Nothing,
        };
        let Some(q) = self.asking.take() else {
            return Errand::Nothing;
        };
        (self.cursor, self.cell) = q.back;
        self.moved = true;
        if answer { *q.yes } else { *q.no }
    }

    /// `c`, or the glyph beside the queue's title: a question first — a
    /// queue built by hand is not something one key should take away.
    fn ask_to_clear(&mut self, state: Option<&State>, texts: &ronoctua_i18n::Texts) -> Errand {
        use ronoctua_i18n::key;
        if state.is_none_or(|s| s.queue.is_empty()) {
            return Errand::Said(texts.t(key::PLAYER_QUEUE_EMPTY));
        }
        self.ask(
            texts.t(key::PLAYER_ASK_CLEAR),
            Errand::TellSaying(Ask::Clear, texts.t(key::PLAYER_QUEUE_CLEARED)),
            Errand::Nothing,
        )
    }

    /// The rows of the files level: the folder (its place a ring on
    /// `←`/`→`), a path or an address to type, the folder whole, and the
    /// entries — `..` first — in columns.
    fn files_rows(&self, files: &Files, b: &mut Builder<'_>) {
        use ronoctua_i18n::key;
        let l = &files.listing;
        b.pair(
            b.t(key::PLAYER_BROWSE_FOLDER),
            shown_path(&l.at),
            Target::Place,
            "browse-folder",
        );
        b.pair(
            b.t(key::PLAYER_BROWSE_TYPE),
            String::new(),
            Target::Typed,
            "browse-type",
        );
        if files.purpose == Purpose::Queue {
            b.pair(
                b.t(key::PLAYER_BROWSE_PLAY_FOLDER),
                String::new(),
                Target::PlayFolder,
                "browse-play-folder",
            );
            b.pair(
                b.t(key::PLAYER_BROWSE_ENQUEUE_FOLDER),
                String::new(),
                Target::EnqueueFolder,
                "browse-enqueue-folder",
            );
        }
        b.inert(crate::chrome::AIR);
        if let Some(why) = &l.refused {
            let said = b
                .texts
                .t_with(key::PLAYER_BROWSE_REFUSED, &[("why", why.as_str())]);
            for line in crate::chrome::note_lines(&said, b.room) {
                b.inert(vec![Builder::piece(
                    b.left,
                    line,
                    tokens::color::STATE_WARNING,
                )]);
            }
        }
        // `..` FIRST, and always — the way back is where the eye starts.
        let mut entries: Vec<(char, String, Target, String)> = Vec::new();
        if l.at.parent().is_some() {
            entries.push((
                tokens::icon::FOLDER,
                "..".to_owned(),
                Target::Up,
                "browse-..".to_owned(),
            ));
        }
        for f in &l.folders {
            entries.push((
                tokens::icon::FOLDER,
                file_name(f),
                Target::Folder(f.clone()),
                format!("browse-{}", f.display()),
            ));
        }
        for (f, glyph) in l.files.iter().zip(&l.glyphs) {
            entries.push((
                *glyph,
                file_name(f),
                Target::File(f.clone()),
                format!("browse-{}", f.display()),
            ));
        }
        b.grid(&entries);
        if l.refused.is_none() && l.folders.is_empty() && l.files.is_empty() {
            b.note(key::PLAYER_BROWSE_EMPTY);
        }
    }

    /// Every row, in reading order.
    #[must_use]
    pub fn rows(
        &self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        use ronoctua_i18n::key;
        let (left, room) = crate::power::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            cell: self.cell,
            texts,
        };
        // A QUESTION ON SCREEN is all that is drawn: it waits for an answer.
        if let Some(q) = &self.asking {
            b.heading(key::PLAYER_HEADING_QUESTION);
            for line in crate::chrome::note_lines(&q.sentence, b.room) {
                b.inert(vec![Builder::piece(
                    b.left,
                    line,
                    tokens::color::TEXT_SECONDARY,
                )]);
            }
            b.pair(
                b.t(key::PLAYER_ANSWER_NO),
                String::new(),
                Target::Answer(false),
                "no",
            );
            b.pair(
                b.t(key::PLAYER_ANSWER_YES),
                String::new(),
                Target::Answer(true),
                "yes",
            );
            return b.out;
        }
        if let Some(files) = &self.files {
            self.files_rows(files, &mut b);
            return b.out;
        }
        let Some(s) = state else {
            b.heading(key::PLAYER_HEADING_NOW);
            b.note(key::PLAYER_NOT_RUNNING);
            opening(&mut b);
            self.settings(&mut b);
            return b.out;
        };
        // NOW: what plays, and the transport.
        b.heading(key::PLAYER_HEADING_NOW);
        for t in &s.troubles {
            let said = trouble_sentence(t, texts);
            for line in crate::chrome::note_lines(&said, room) {
                b.inert(vec![Builder::piece(
                    left,
                    line,
                    tokens::color::STATE_WARNING,
                )]);
            }
        }
        if s.current.is_none() {
            b.note(key::PLAYER_NOTHING);
        } else {
            if let Some(name) = now_playing(s) {
                b.pair(b.t(key::PLAYER_PLAYING), name, Target::Nothing, "title");
            }
            if let Some(album) = s.album.as_deref().filter(|a| !a.trim().is_empty()) {
                b.pair(
                    b.t(key::PLAYER_ALBUM),
                    album.to_owned(),
                    Target::Nothing,
                    "album",
                );
            }
            b.pair(
                b.t(key::PLAYER_STATE),
                b.t(if s.playing {
                    key::PLAYER_STATE_PLAYING
                } else {
                    key::PLAYER_STATE_PAUSED
                }),
                Target::Playing,
                "playing",
            );
            // THE POSITION IS A BAR when the item has a length: a press
            // seeks there, as on the window's time bar. A stream has none,
            // and says only where it is.
            match s.duration.filter(|d| *d > 0.0) {
                Some(d) => b.slider_pair(
                    b.t(key::PLAYER_POSITION),
                    share(s.position, d),
                    &format!("{} / {}", clock(s.position), clock(d)),
                    Target::Position,
                    "position",
                    Slid::Position,
                ),
                None => b.pair(
                    b.t(key::PLAYER_POSITION),
                    clock(s.position),
                    Target::Position,
                    "position",
                ),
            }
        }
        // THE VOLUME IS A BAR up to the loudest the ring goes, the window's
        // track's length.
        b.slider_pair(
            b.t(key::PLAYER_VOLUME),
            share(f64::from(s.volume), f64::from(VOLUME_MOST)),
            &if s.muted {
                b.t(key::PLAYER_MUTED)
            } else {
                format!("{}%", s.volume)
            },
            Target::Volume,
            "volume",
            Slid::Volume,
        );
        b.pair(
            b.t(key::PLAYER_SPEED),
            format!("{}×", trim_float(s.speed)),
            Target::Speed,
            "speed",
        );
        b.pair(
            b.t(key::PLAYER_REPEAT),
            b.t(match s.repeat {
                Repeat::Nothing => key::PLAYER_REPEAT_NOTHING,
                Repeat::Queue => key::PLAYER_REPEAT_QUEUE,
                Repeat::One => key::PLAYER_REPEAT_ONE,
            }),
            Target::Repeat,
            "repeat",
        );
        b.pair(
            b.t(key::PLAYER_SHUFFLE),
            on_off(s.shuffle, texts),
            Target::Shuffle,
            "shuffle",
        );
        // TRACKS: only the kinds there is a choice of.
        let audio: Vec<&Track> = s.tracks.iter().filter(|t| t.kind == "audio").collect();
        if audio.len() > 1 {
            let now = audio
                .iter()
                .find(|t| t.selected)
                .map_or_else(|| b.t(key::PLAYER_NONE), |t| track_name(t));
            b.pair(b.t(key::PLAYER_AUDIO_TRACK), now, Target::Audio, "audio");
        }
        if s.tracks.iter().any(|t| t.kind == "sub") {
            let now = s
                .tracks
                .iter()
                .find(|t| t.kind == "sub" && t.selected)
                .map_or_else(|| b.t(key::PLAYER_NONE), track_name);
            b.pair(
                b.t(key::PLAYER_SUBTITLES),
                now,
                Target::Subtitles,
                "subtitles",
            );
        }
        // LYRICS: the line being sung in the accent, two on each side.
        if !s.lyrics.is_empty() {
            b.heading(key::PLAYER_HEADING_LYRICS);
            let here = ronoctua_player::lyrics::current(&s.lyrics, s.position);
            let synced = s.lyrics.iter().any(|l| l.at.is_some());
            let (from, to) = match (here, synced) {
                (Some(i), true) => (
                    i.saturating_sub(LYRICS_AROUND),
                    (i + LYRICS_AROUND + 1).min(s.lyrics.len()),
                ),
                _ => (0, (2 * LYRICS_AROUND + 1).min(s.lyrics.len())),
            };
            for (i, l) in s.lyrics.iter().enumerate().take(to).skip(from) {
                let colour = if Some(i) == here && synced {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_DIM
                };
                for line in crate::chrome::note_lines(&l.text, room) {
                    b.inert(vec![Builder::piece(left, line, colour)]);
                }
            }
            if let Some(from) = s.lyrics_from.as_deref() {
                b.inert(vec![Builder::piece(
                    left,
                    texts.t_with(key::PLAYER_LYRICS_FROM, &[("from", from)]),
                    tokens::color::TEXT_DIM,
                )]);
            }
        }
        // THE QUEUE: every item, the current one in the accent, and the
        // glyph that empties it beside the title.
        b.heading_with(
            key::PLAYER_HEADING_QUEUE,
            tokens::icon::PLAYER_CLEAR_QUEUE,
            Target::ClearQueue,
        );
        if s.queue.is_empty() {
            b.note(key::PLAYER_QUEUE_EMPTY);
        }
        for (i, item) in s.queue.iter().enumerate() {
            b.item(
                format!("{}.", i + 1),
                item_name(item),
                s.current == Some(i),
                Target::Item(i),
                &format!("item-{i}-{}", item.location),
            );
        }
        // OPEN, right under the queue: what fills it.
        opening(&mut b);
        // IN STEP: other videos held to this sound.
        b.heading(key::PLAYER_HEADING_IN_STEP);
        b.note(key::PLAYER_ABOUT_IN_STEP);
        for (i, p) in s.participants.iter().enumerate() {
            let name = item_name(&ronoctua_player::wire::Item {
                location: p.item.clone(),
                title: None,
            });
            b.pair(
                name,
                texts.t_with(key::PLAYER_DRIFT, &[("ms", &p.drift_ms.to_string())]),
                Target::Follower(i),
                &format!("follower-{i}-{}", p.item),
            );
        }
        // THE SAME PICKER as `Open`, for another purpose: the file chosen
        // plays in step, and a path or an address can be typed there too.
        b.pair(
            b.t(key::PLAYER_FOLLOW),
            String::new(),
            Target::FollowPick,
            "follow",
        );
        machines(&mut b, s);
        self.settings(&mut b);
        b.out
    }
}

impl View {
    /// The two switches that reach the internet, each naming its host.
    fn settings(&self, b: &mut Builder<'_>) {
        use ronoctua_i18n::key;
        use ronoctua_player::prefs;
        b.heading(key::PLAYER_HEADING_SETTINGS);
        b.note(key::PLAYER_ABOUT_SETTINGS);
        // THE HOST IN THE VALUE, beside the answer: in the label it pushed the
        // value out of the column every other value starts at.
        let with_host = |on: bool, host: &str| {
            b.texts.t_with(
                key::PLAYER_SWITCH_HOST,
                &[("state", &on_off(on, b.texts)), ("host", host)],
            )
        };
        b.pair(
            b.t(key::PLAYER_LYRICS_ONLINE),
            with_host(self.prefs.lyrics_online, prefs::LRCLIB_HOST),
            Target::LyricsOnline,
            "lyrics-online",
        );
        b.pair(
            b.t(key::PLAYER_SPONSORBLOCK),
            with_host(self.prefs.sponsorblock, prefs::SPONSORBLOCK_HOST),
            Target::Sponsors,
            "sponsorblock",
        );
        // WHAT IS SKIPPED, one row a category, SET IN under the switch they
        // belong to — hierarchy is air and colour here, never a rule.
        let (left, room) = (b.left, b.room);
        b.left += tokens::space::VIEW_NESTCELLS;
        b.room = b.room.saturating_sub(tokens::space::VIEW_NESTCELLS);
        for category in prefs::CATEGORIES {
            b.pair(
                b.t(skip_key(category)),
                on_off(self.prefs.skipped.iter().any(|c| c == category), b.texts),
                Target::Skip(category),
                &format!("skip-{category}"),
            );
        }
        (b.left, b.room) = (left, room);
    }
}

/// Each category's name on screen.
fn skip_key(category: &str) -> &'static str {
    use ronoctua_i18n::key;
    match category {
        "sponsor" => key::PLAYER_SKIP_SPONSOR,
        "selfpromo" => key::PLAYER_SKIP_SELFPROMO,
        "interaction" => key::PLAYER_SKIP_INTERACTION,
        "intro" => key::PLAYER_SKIP_INTRO,
        "outro" => key::PLAYER_SKIP_OUTRO,
        "preview" => key::PLAYER_SKIP_PREVIEW,
        "hook" => key::PLAYER_SKIP_HOOK,
        "music_offtopic" => key::PLAYER_SKIP_MUSIC_OFFTOPIC,
        _ => key::PLAYER_SKIP_FILLER,
    }
}

/// Other machines: the code this one shows, the ones it can join, the ones
/// that can join it.
fn machines(b: &mut Builder<'_>, s: &State) {
    use ronoctua_i18n::key;
    b.heading(key::PLAYER_HEADING_MACHINES);
    if s.tailnet.is_none() {
        b.note(key::PLAYER_NO_TAILNET);
        return;
    }
    b.note(key::PLAYER_ABOUT_MACHINES);
    for host in &s.hosts {
        let joined = s.joined.as_ref().filter(|j| &j.host == host);
        let value = joined.map_or_else(
            || b.t(key::PLAYER_JOIN),
            |j| {
                b.texts.t_with(
                    key::PLAYER_JOINED_AS,
                    &[
                        ("role", &b.t(role_key(j.role))),
                        ("ms", &j.drift_ms.to_string()),
                    ],
                )
            },
        );
        b.pair(
            host.clone(),
            value,
            Target::Host(host.clone()),
            &format!("host-{host}"),
        );
    }
    for guest in &s.guests {
        b.pair(
            guest.clone(),
            b.t(key::PLAYER_GUEST),
            Target::Guest(guest.clone()),
            &format!("guest-{guest}"),
        );
    }
    b.pair(
        b.t(key::PLAYER_INVITE),
        s.invite.clone().unwrap_or_default(),
        Target::Invite,
        "invite",
    );
    b.pair(b.t(key::PLAYER_PAIR), String::new(), Target::Pair, "pair");
}

/// A role's name.
const fn role_key(r: Role) -> &'static str {
    match r {
        Role::Both => ronoctua_i18n::key::PLAYER_ROLE_BOTH,
        Role::Screen => ronoctua_i18n::key::PLAYER_ROLE_SCREEN,
        Role::Sound => ronoctua_i18n::key::PLAYER_ROLE_SOUND,
    }
}

/// The rows that open something, and the three doors — the same place in
/// every state (above the switches), so a person always finds them.
fn opening(b: &mut Builder<'_>) {
    use ronoctua_i18n::key;
    b.heading(key::PLAYER_HEADING_OPEN);
    b.pair(
        b.t(key::PLAYER_OPEN_BROWSE),
        String::new(),
        Target::Browse,
        "browse",
    );

    b.pair(
        b.t(key::PLAYER_OPEN_COLUMN),
        String::new(),
        Target::Run("player-column"),
        "column",
    );
    b.pair(
        b.t(key::PLAYER_OPEN_WINDOW),
        String::new(),
        Target::Run("player-window"),
        "window",
    );
}

/// What a press does on a row. Pure: it reads the state and says what to ask.
fn turn(target: &Target, press: Press, state: Option<&State>) -> Errand {
    let forward = !matches!(press, Press::Less);
    let default = State::default();
    let s = state.unwrap_or(&default);
    match (target, press) {
        (Target::Nothing, _) => Errand::Nothing,
        (Target::Run(id), Press::Enter) => Errand::Run(id),
        (Target::Run(_), _) => Errand::Nothing,
        (Target::Pair, Press::Enter) => Errand::Ask(Question {
            id: PAIR,
            secret: false,
        }),
        (Target::Playing, _) => Errand::Tell(Ask::Toggle),
        (Target::Position, Press::Enter) => Errand::Tell(Ask::Seek {
            seconds: JUMP_STEP,
            relative: true,
        }),
        (Target::Position, Press::Less | Press::More) => Errand::Tell(Ask::Seek {
            seconds: if forward { SEEK_STEP } else { -SEEK_STEP },
            relative: true,
        }),
        (Target::Volume, Press::Enter) => Errand::Tell(Ask::Mute),
        (Target::Volume, Press::Less | Press::More) => Errand::Tell(Ask::Volume {
            percent: if forward {
                s.volume.saturating_add(VOLUME_STEP).min(VOLUME_MOST)
            } else {
                s.volume.saturating_sub(VOLUME_STEP)
            },
        }),
        (Target::Speed, _) => {
            let at = SPEEDS
                .iter()
                .position(|x| (x - s.speed).abs() < 0.01)
                .unwrap_or(2);
            let n = SPEEDS.len();
            Errand::Tell(Ask::Speed {
                factor: SPEEDS[if forward {
                    (at + 1) % n
                } else {
                    (at + n - 1) % n
                }],
            })
        }
        (Target::Repeat, _) => Errand::Tell(Ask::Repeat),
        (Target::Shuffle, _) => Errand::Tell(Ask::Shuffle),
        (Target::Audio, _) => next_track(s, "audio", forward, false)
            .map_or(Errand::Nothing, |id| Errand::Tell(Ask::AudioTrack { id })),
        (Target::Subtitles, _) => next_track(s, "sub", forward, true)
            .map_or(Errand::Nothing, |id| {
                Errand::Tell(Ask::SubtitleTrack { id })
            }),
        (Target::Item(i), Press::Enter) => Errand::Tell(Ask::Jump { index: *i }),
        (Target::Item(i), Press::Remove) => Errand::Tell(Ask::Remove { index: *i }),
        (Target::Item(i), Press::Less | Press::More) => {
            let to = if forward {
                (*i + 1).min(s.queue.len().saturating_sub(1))
            } else {
                i.saturating_sub(1)
            };
            if to == *i {
                Errand::Nothing
            } else {
                Errand::Tell(Ask::Move { from: *i, to })
            }
        }
        (Target::Follower(i), Press::Remove | Press::Enter) => {
            Errand::Tell(Ask::Unfollow { index: *i })
        }
        // A MACHINE: `RET` joins it, or leaves it when joined; the arrows turn
        // what this machine plays of it; `d` forgets the pairing.
        (Target::Host(h), Press::Enter) => {
            if s.joined.as_ref().is_some_and(|j| &j.host == h) {
                Errand::Tell(Ask::Leave)
            } else {
                Errand::Tell(Ask::Join {
                    host: h.clone(),
                    role: Role::Both,
                })
            }
        }
        (Target::Host(h), Press::Less | Press::More) => {
            let now = s
                .joined
                .as_ref()
                .filter(|j| &j.host == h)
                .map_or(Role::Both, |j| j.role);
            let role = if forward {
                now.next()
            } else {
                now.next().next()
            };
            Errand::Tell(Ask::Join {
                host: h.clone(),
                role,
            })
        }
        (Target::Host(h) | Target::Guest(h), Press::Remove) => {
            Errand::Tell(Ask::Forget { name: h.clone() })
        }
        (Target::Invite, Press::Enter) => Errand::Tell(Ask::Invite),
        _ => Errand::Nothing,
    }
}

/// Carries out what is the same through every door — telling the player — and
/// gives back what the door should say. `Run` and `Close` are the door's own,
/// and come back untouched as `Err`.
///
/// # Errors
///
/// The errand is one the DOOR carries out: a command to run, or closing.
pub fn carry_out(errand: Errand, texts: &ronoctua_i18n::Texts) -> Result<Option<String>, Errand> {
    match errand {
        Errand::Nothing => Ok(None),
        // A PAIRING IS THE DOOR'S TOO: it is sent off the loop
        // ([`pair_then`]), because the player answers it only after a round
        // trip to the other machine.
        Errand::Run(_) | Errand::Close | Errand::Show(_) | Errand::Ask(_) | Errand::Pair { .. } => {
            Err(errand)
        }
        Errand::Said(sentence) => Ok(Some(sentence)),
        // THE SENTENCE UNLESS THE PLAYER REFUSED: then the refusal is what is
        // true, and it is what is said.
        Errand::TellSaying(ask, sentence) => Ok(Some(tell(&ask, texts).unwrap_or(sentence))),
        Errand::Tell(ask) => Ok(tell(&ask, texts)),
        Errand::Keep(p) => Ok(match ronoctua_player::prefs::store().write(&p) {
            Ok(()) => {
                let _ = ronoctua_player::client::tell(&Ask::Reload);
                None
            }
            Err(e) => Some(texts.t_with(
                ronoctua_i18n::key::PLAYER_TROUBLE_WRITE,
                &[("why", &e.to_string())],
            )),
        }),
    }
}

/// How long a pairing may take: the player answers only once the other
/// machine has — its own patience is five seconds to connect and five to
/// hear back — and the three seconds every other ask gets ended the wait
/// while the pairing went on, and said the player was not running.
const PAIR_PATIENCE: std::time::Duration = std::time::Duration::from_secs(12);

/// How a pairing sent off the loop ended — the words are said on the loop,
/// where the language is ([`paired_said`]).
#[derive(Debug)]
pub enum Paired {
    /// The player answered: paired, or refused with its reason.
    Answered(ronoctua_player::wire::Say),
    /// No player on this machine to pair.
    NotInstalled,
    /// The player could not be started or reached.
    NotRunning,
    /// The player was asked and had not answered by [`PAIR_PATIENCE`].
    NoAnswer,
}

/// Pairs with `to`, OFF THE LOOP — both doors send the code this way — and
/// hands `done` how it ended, on the thread that waited.
///
/// THE CODE LEAVES FOR THE PLAYER, and only there: it is the one path it has,
/// down the player's socket, and it is never printed on the way.
pub fn pair_then(to: String, code: Code, done: impl FnOnce(Paired) + Send + 'static) {
    let born = std::thread::Builder::new()
        .name("player-pair".to_owned())
        .spawn(move || {
            let paired = match ronoctua_player::client::ensure() {
                Err(e) if e.is::<ronoctua_player::client::NotInstalled>() => Paired::NotInstalled,
                Err(_) => Paired::NotRunning,
                Ok(_) => match ronoctua_player::client::ask_within(
                    &Ask::PairWith { to, code: code.0 },
                    PAIR_PATIENCE,
                ) {
                    Ok(say) => Paired::Answered(say),
                    Err(e) if timed_out(&e) => Paired::NoAnswer,
                    Err(_) => Paired::NotRunning,
                },
            };
            done(paired);
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that pairs");
    }
}

/// Whether an ask ended because the answer did not come in time.
fn timed_out(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>().is_some_and(|io| {
        matches!(
            io.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        )
    })
}

/// What a door says of a pairing that ended: nothing when it paired, the
/// player's reason when it refused, and which of the other three it was.
#[must_use]
pub fn paired_said(paired: &Paired, texts: &ronoctua_i18n::Texts) -> Option<String> {
    use ronoctua_i18n::key;
    match paired {
        Paired::Answered(ronoctua_player::wire::Say::Refused(t)) => {
            Some(trouble_sentence(t, texts))
        }
        Paired::Answered(_) => None,
        Paired::NotInstalled => Some(texts.t(key::PLAYER_NOT_INSTALLED)),
        Paired::NotRunning => Some(texts.t(key::PLAYER_NOT_RUNNING)),
        Paired::NoAnswer => Some(texts.t(key::PLAYER_PAIR_NO_ANSWER)),
    }
}

/// Tells the player one thing, starting it when nobody answers; what it
/// refused, in a sentence.
#[must_use]
pub fn tell(ask: &Ask, texts: &ronoctua_i18n::Texts) -> Option<String> {
    if let Err(e) = ronoctua_player::client::ensure() {
        return Some(texts.t(if e.is::<ronoctua_player::client::NotInstalled>() {
            ronoctua_i18n::key::PLAYER_NOT_INSTALLED
        } else {
            ronoctua_i18n::key::PLAYER_NOT_RUNNING
        }));
    }
    match ronoctua_player::client::ask(ask) {
        Ok(ronoctua_player::wire::Say::Refused(t)) => Some(trouble_sentence(&t, texts)),
        Ok(_) => None,
        Err(_) => Some(texts.t(ronoctua_i18n::key::PLAYER_NOT_RUNNING)),
    }
}

/// A path or an address typed for one of the player's questions, turned into
/// an order — [`answered`]'s, for the three that type one.
///
/// A path that exists is made absolute against home rather than against the
/// footer's working directory, which is nobody's; anything else is handed to
/// the player as typed — an address, which mpv and yt-dlp understand.
#[must_use]
pub fn typed(id: &str, said: &str) -> Option<Ask> {
    let said = said.trim();
    if said.is_empty() || !matches!(id, PLAY | ENQUEUE | FOLLOW) {
        return None;
    }
    let item = expand(said);
    if id == FOLLOW {
        return Some(Ask::Follow { item });
    }
    // A FOLDER IS EVERY FILE UNDER IT the player plays; one with none goes
    // through as typed, and the player's refusal is what gets said.
    let mut items = crate::player_browse::items_of(std::slice::from_ref(&item));
    if items.is_empty() {
        items.push(item);
    }
    Some(Ask::Open {
        items,
        replace: id != ENQUEUE,
    })
}

/// `~` and a relative path, against home; a `file://` URL, as the path it
/// names.
fn expand(said: &str) -> String {
    // A `file://` URL IS A PATH spelled long — what a drag and many programs
    // copy — and only as a path does a folder become its files.
    if let Some(rest) = said.strip_prefix("file://") {
        return unescaped(rest);
    }
    if said.contains("://") {
        return said.to_owned();
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let path = if let Some(rest) = said.strip_prefix("~/") {
        home.map_or_else(|| said.into(), |h| h.join(rest))
    } else if std::path::Path::new(said).is_absolute() {
        said.into()
    } else {
        home.map_or_else(|| said.into(), |h| h.join(said))
    };
    path.to_string_lossy().into_owned()
}

/// `%20` and its kin, back to the bytes they stand for.
fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(Some(h)), Some(Some(l))) = (
                bytes.get(i + 1).map(|b| hex(*b)),
                bytes.get(i + 2).map(|b| hex(*b)),
            )
        {
            out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Every file a folder plays, as the player's items.
fn gathered(folder: &std::path::Path) -> Vec<String> {
    crate::player_browse::gather(folder)
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// A folder's name, or the whole path for the root.
fn file_name(p: &std::path::Path) -> String {
    p.file_name().map_or_else(
        || p.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// A path as a person reads it: under home, from `~`.
fn shown_path(p: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from)
        && let Ok(rest) = p.strip_prefix(&home)
    {
        return if rest.as_os_str().is_empty() {
            "~".to_owned()
        } else {
            format!("~/{}", rest.display())
        };
    }
    p.to_string_lossy().into_owned()
}

/// `part` of `whole`, in per cent, for a bar.
fn share(part: f64, whole: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (part / whole.max(f64::MIN_POSITIVE) * 100.0)
        .clamp(0.0, 100.0)
        .round() as u8;
    percent
}

fn trim_float(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn on_off(on: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(if on {
        ronoctua_i18n::key::POINTERS_ON
    } else {
        ronoctua_i18n::key::POINTERS_OFF
    })
}

/// Which press a command id is — the one map every door reaches.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "player-up" => Press::Up,
        "player-down" => Press::Down,
        "player-less" => Press::Less,
        "player-more" => Press::More,
        "player-enter" => Press::Enter,
        "player-toggle" => Press::Toggle,
        "player-stop" => Press::Stop,
        "player-next" => Press::Next,
        "player-previous" => Press::Previous,
        "player-remove" => Press::Remove,
        "player-browse" => Press::Browse,
        "player-add" => Press::Add,
        "player-parent" => Press::Parent,
        "player-play-now" => Press::PlayNow,
        "player-clear" => Press::Clear,
        "player-quieter" => Press::Quieter,
        "player-louder" => Press::Louder,
        "player-only" => Press::Only,
        "player-maximize" => Press::Maximize,
        "player-pip" => Press::Pip,
        "player-mute" => Press::Mute,
        _ => return None,
    })
}

/// A key, as the view's press — the ONE reader every door calls: the window
/// and the column with `door::keys::key_of`, the footer with the key its own
/// reader built.
///
/// `Esc` is every level's way back, and the arrows, `RET`, `SPC` and `DEL`
/// are read by their symbol whatever is held, as both doors always read them.
/// A LETTER is read by its whole notation: `C-s` is not `s`. The footer used
/// to drop every modifier and answer a letter held with `Ctrl` as the bare
/// letter, while the window read xkb's control character and answered
/// nothing — one key, two answers.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::{Key, Sym};
    let notation = match key.sym {
        Sym::Esc => return Some(Press::Escape),
        Sym::Char(_) => key.notation(),
        other => Key::simple(other).notation(),
    };
    pressed(&notation)
}

/// Which press a notation is, through [`OF_THE_VIEW`].
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    /// Which entry of a grid row the cursor is on.
    cell: usize,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn t(&self, key: &str) -> String {
        self.texts.t(key)
    }

    const fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    fn inert(&mut self, fields: Vec<Field>) {
        self.out.push(Row {
            fields,
            focused: false,
            heading: true,
            target: Target::Nothing,
            spot: String::new(),
            cells: Vec::new(),
            slider: None,
        });
    }

    fn heading(&mut self, key: &str) {
        if !self.out.is_empty() {
            self.inert(crate::chrome::AIR);
        }
        let text = self.texts.t(key);
        self.inert(vec![Self::piece(
            self.left,
            text,
            tokens::color::TEXT_HEADING,
        )]);
    }

    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        for line in crate::chrome::note_lines(&text, self.room) {
            self.inert(vec![Self::piece(self.left, line, tokens::color::TEXT_DIM)]);
        }
    }

    fn pair(&mut self, label: String, value: String, target: Target, spot: &str) {
        let focused = self.cursor == self.out.len();
        let acts = target.acts();
        let kind = crate::chrome::Kind::of(acts);
        self.out.push(Row {
            fields: crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
            focused,
            heading: false,
            target,
            spot: spot.to_owned(),
            cells: Vec::new(),
            slider: None,
        });
    }

    /// A pair whose value is a BAR, `filled` per cent of it, and `text` after
    /// it — the bar is a control, and the row says where it was drawn.
    fn slider_pair(
        &mut self,
        label: String,
        filled: u8,
        text: &str,
        target: Target,
        spot: &str,
        of: Slid,
    ) {
        let cells = tokens::space::PLAYER_VOLUMEBAR;
        let bar = crate::chrome::meter(filled, cells);
        self.pair(label, format!("{bar} {text}"), target, spot);
        // WHERE THE VALUE WAS PLACED is where the bar begins — read back from
        // the field, and never worked out a second time — and a value cut
        // short keeps only the cells it drew.
        if let Some(row) = self.out.last_mut()
            && let Some(value) = row.fields.get(1)
        {
            let drawn = ronoctua_render::layout::width(&value.text).min(cells);
            row.slider = (drawn > 0).then_some(Slider {
                of,
                from: value.at,
                cells: drawn,
            });
        }
    }

    /// The files' entries in up to three columns, filled DOWN each column
    /// — the order a listing is read in, so `↓` is the next name — and
    /// `←`/`→` across. Three where the room holds three of the longest name,
    /// two where names are long, one where a column would cut every name to
    /// a stub (`space.player.browseColumn`).
    fn grid(&mut self, entries: &[(char, String, Target, String)]) {
        if entries.is_empty() {
            return;
        }
        let glyph_cells: u16 = 2;
        let gap: u16 = 2;
        let widest = entries
            .iter()
            .map(|(_, name, _, _)| ronoctua_protocol::cells(name))
            .max()
            .unwrap_or(0)
            .saturating_add(glyph_cells + gap);
        let floor = tokens::space::PLAYER_BROWSECOLUMN;
        let columns: u16 = if self.room / 3 >= widest.max(floor) {
            3
        } else if self.room / 2 >= floor {
            2
        } else {
            1
        };
        let width = self.room / columns;
        let count = entries.len();
        let per = count.div_ceil(usize::from(columns));
        for r in 0..per {
            let focused_row = self.cursor == self.out.len();
            let mut fields = Vec::new();
            let mut cells = Vec::new();
            for c in 0..columns {
                let Some((glyph, name, target, spot)) = entries.get(usize::from(c) * per + r)
                else {
                    continue;
                };
                let from = self.left + c * width;
                let here = focused_row && usize::from(c) == self.cell.min(usize::from(columns) - 1);
                let colour = if here {
                    tokens::color::TEXT_HERE_ACTS
                } else {
                    tokens::color::TEXT_PRIMARY
                };
                let name = ronoctua_render::layout::elide_end(
                    name,
                    width.saturating_sub(glyph_cells + gap),
                );
                fields.push(Self::piece(
                    from,
                    glyph.to_string(),
                    tokens::color::TEXT_DIM,
                ));
                fields.push(Self::piece(from + glyph_cells, name, colour));
                cells.push(Cell {
                    from,
                    width,
                    target: target.clone(),
                    spot: spot.clone(),
                });
            }
            let on = self.cell.min(cells.len().saturating_sub(1));
            let target = if focused_row {
                cells.get(on).map_or(Target::Nothing, |c| c.target.clone())
            } else {
                cells.first().map_or(Target::Nothing, |c| c.target.clone())
            };
            let spot = cells.first().map(|c| c.spot.clone()).unwrap_or_default();
            self.out.push(Row {
                fields,
                focused: focused_row,
                heading: false,
                target,
                spot,
                cells,
                slider: None,
            });
        }
    }

    /// A title with a control at the end of its line — a glyph that acts on a
    /// click (`View::clicked_at`); the key that does the same is taught in the
    /// hint line.
    fn heading_with(&mut self, key: &str, glyph: char, target: Target) {
        self.heading(key);
        if let Some(row) = self.out.last_mut() {
            let at = self.left + self.room.saturating_sub(2);
            // THE GLYPH WEARS ITS TITLE'S TONE, as the drawers' chevrons do
            // (`network::drawer`): a second, quieter text tone on a title
            // line is the title read as body text.
            row.fields.push(Self::piece(
                at,
                glyph.to_string(),
                tokens::color::TEXT_HEADING,
            ));
            row.cells.push(Cell {
                from: at,
                width: 2,
                target,
                spot: "control".to_owned(),
            });
        }
    }

    /// An item of a list: the NAME is the strong half, as every list here
    /// has it, and the number in front says where it is.
    fn item(&mut self, number: String, name: String, current: bool, target: Target, spot: &str) {
        let focused = self.cursor == self.out.len();
        let colour = match (focused, current) {
            (true, _) => tokens::color::TEXT_HERE_ACTS,
            (false, true) => tokens::color::STATE_ACCENT,
            (false, false) => tokens::color::TEXT_PRIMARY,
        };
        let number_cells = 5;
        let name =
            ronoctua_render::layout::elide_end(&name, self.room.saturating_sub(number_cells));
        self.out.push(Row {
            fields: vec![
                Self::piece(self.left, number, tokens::color::TEXT_DIM),
                Self::piece(self.left + number_cells, name, colour),
            ],
            focused,
            heading: false,
            target,
            spot: spot.to_owned(),
            cells: Vec::new(),
            slider: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_player::wire::Item;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn playing() -> State {
        State {
            queue: vec![
                Item {
                    location: "/music/one.flac".into(),
                    title: None,
                },
                Item {
                    location: "/music/two.flac".into(),
                    title: Some("Two".into()),
                },
            ],
            current: Some(0),
            playing: true,
            position: 75.0,
            duration: Some(3725.0),
            volume: 80,
            speed: 1.0,
            tracks: vec![
                Track {
                    id: 1,
                    kind: "sub".into(),
                    title: None,
                    lang: Some("en".into()),
                    codec: None,
                    selected: false,
                },
                Track {
                    id: 2,
                    kind: "sub".into(),
                    title: None,
                    lang: Some("pt".into()),
                    codec: None,
                    selected: true,
                },
            ],
            ..State::default()
        }
    }

    #[test]
    fn a_clock_has_hours_only_from_an_hour_on() {
        assert_eq!(clock(75.4), "1:15");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(clock(-3.0), "0:00");
    }

    #[test]
    fn the_cursor_is_born_on_a_row_that_acts() {
        let t = texts();
        let s = playing();
        let mut v = View::fresh();
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(rows[v.cursor()].target.acts(), "{:?}", rows[v.cursor()]);
    }

    #[test]
    fn every_target_of_the_queue_is_one_item_and_a_click_plays_it() {
        let t = texts();
        let s = playing();
        let mut v = View::fresh();
        let rows = v.rows(Some(&s), 80, &t);
        let second = rows
            .iter()
            .position(|r| r.target == Target::Item(1))
            .expect("the second item is a row");
        assert!(!v.clicked(second, &rows), "the first click selects");
        assert!(v.clicked(second, &rows), "the second acts");
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Jump { index: 1 })
        );
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            Errand::Tell(Ask::Remove { index: 1 })
        );
    }

    #[test]
    fn the_subtitle_ring_goes_through_none() {
        let s = playing();
        assert_eq!(next_track(&s, "sub", true, true), Some(None));
        assert_eq!(next_track(&s, "sub", false, true), Some(Some(1)));
        assert_eq!(
            next_track(&s, "audio", true, false),
            None,
            "no choice, no ring"
        );
    }

    #[test]
    fn the_volume_stops_at_both_ends() {
        let mut s = playing();
        s.volume = VOLUME_MOST;
        assert_eq!(
            turn(&Target::Volume, Press::More, Some(&s)),
            Errand::Tell(Ask::Volume {
                percent: VOLUME_MOST
            })
        );
        s.volume = 2;
        assert_eq!(
            turn(&Target::Volume, Press::Less, Some(&s)),
            Errand::Tell(Ask::Volume { percent: 0 })
        );
    }

    /// A view with the files open on `at`, the cursor where it lands.
    fn among(at: &std::path::Path) -> View {
        let mut v = View::fresh();
        v.files = Some(Files {
            place: 0,
            listing: crate::player_browse::list(at),
            purpose: Purpose::Queue,
        });
        v.settle_in(None, 80, &texts());
        v
    }

    /// A library of its own per test: the tests run at once, and one
    /// removing a shared folder pulls it from under another.
    fn a_library(test: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("ronoctua-library-{}-{test}", std::process::id()));
        for name in [
            "one.flac",
            "Album/01.ogg",
            "Album/02.ogg",
            "Album/cover.jpg",
        ] {
            let p = root.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"").unwrap();
        }
        root
    }

    fn on(v: &View) -> Target {
        v.rows(None, 80, &texts())[v.cursor()].target.clone()
    }

    /// FAILS IF: the files stop being walked the way a person walks them —
    /// into a folder, back up onto it, a file added or played alone, and a
    /// folder queued whole.
    #[test]
    fn the_files_are_walked_played_and_queued() {
        let t = texts();
        let root = a_library("walked");
        let album = root.join("Album");
        let song = album.join("01.ogg").to_string_lossy().into_owned();
        let mut v = among(&root);
        // THE CURSOR LANDS ON THE FIRST ENTRY, the folder — not on `..`.
        assert_eq!(on(&v), Target::Folder(album.clone()));
        // `a` QUEUES IT WHOLE, pictures left out, and says so.
        let Errand::TellSaying(Ask::Open { items, replace }, said) =
            v.key(Press::Add, None, 80, &t)
        else {
            panic!("a folder was not queued");
        };
        assert!(!replace);
        assert_eq!(items.len(), 2, "{items:?}");
        assert!(!said.is_empty());
        // `RET` ENTERS IT, and the cursor is on its first file.
        assert_eq!(v.key(Press::Enter, None, 80, &t), Errand::Nothing);
        assert_eq!(on(&v), Target::File(album.join("01.ogg")));
        // `RET` ON A FILE ADDS IT; `P` PLAYS IT ALONE.
        let Errand::TellSaying(ask, _) = v.key(Press::Enter, None, 80, &t) else {
            panic!("RET did not add");
        };
        assert_eq!(
            ask,
            Ask::Open {
                items: vec![song.clone()],
                replace: false,
            }
        );
        let Errand::TellSaying(ask, _) = v.key(Press::PlayNow, None, 80, &t) else {
            panic!("P did not play");
        };
        assert_eq!(
            ask,
            Ask::Open {
                items: vec![song],
                replace: true,
            }
        );
        // `DEL` GOES UP, and lands on the folder it came from.
        v.key(Press::Parent, None, 80, &t);
        assert_eq!(on(&v), Target::Folder(album));
        // `Esc` GOES BACK ONE LEVEL, and only the second one closes.
        assert_eq!(v.key(Press::Escape, None, 80, &t), Errand::Nothing);
        assert!(v.files.is_none());
        assert_eq!(v.key(Press::Escape, None, 80, &t), Errand::Close);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a file already in the queue goes in again without a
    /// question, or the question opens on `yes`, or `Esc` adds it.
    #[test]
    fn a_repeated_file_is_asked_about() {
        let t = texts();
        let root = a_library("repeated");
        let one = root.join("one.flac");
        let mut v = among(&root);
        let state = State {
            queue: vec![Item {
                location: one.to_string_lossy().into_owned(),
                title: None,
            }],
            ..State::default()
        };
        let rows = v.rows(Some(&state), 80, &t);
        let (row, cell) = rows
            .iter()
            .enumerate()
            .find_map(|(i, r)| {
                r.cells
                    .iter()
                    .position(|c| c.target == Target::File(one.clone()))
                    .map(|c| (i, c))
            })
            .unwrap();
        v.cursor = row;
        v.cell = cell;
        v.moved = true;
        // THE QUESTION, and nothing sent yet.
        assert_eq!(v.key(Press::Enter, Some(&state), 80, &t), Errand::Nothing);
        assert!(v.asking.is_some());
        let rows = v.rows(Some(&state), 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Answer(false));
        // `Esc` IS NO: nothing added, the files back.
        assert_eq!(v.key(Press::Escape, Some(&state), 80, &t), Errand::Nothing);
        assert!(v.asking.is_none() && v.files.is_some());
        assert_eq!(on(&v), Target::File(one.clone()));
        // ASKED AGAIN, `yes` adds it a second time.
        v.key(Press::Enter, Some(&state), 80, &t);
        v.key(Press::Down, Some(&state), 80, &t);
        let Errand::TellSaying(Ask::Open { items, replace }, _) =
            v.key(Press::Enter, Some(&state), 80, &t)
        else {
            panic!("yes did not add");
        };
        assert!(!replace);
        assert_eq!(items, vec![one.to_string_lossy().into_owned()]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the entries stop being laid out in columns where there is
    /// room, or `..` stops being the first of them, or `→` stops crossing.
    #[test]
    fn the_entries_are_a_grid_with_the_way_back_first() {
        let t = texts();
        let root = a_library("grid");
        let mut v = among(&root);
        let rows = v.rows(None, 200, &t);
        let first = rows
            .iter()
            .find(|r| !r.cells.is_empty() && !r.heading)
            .unwrap();
        assert_eq!(first.cells[0].target, Target::Up);
        assert!(first.cells.len() > 1, "a wide door gave one column");
        // NARROW, one column.
        let rows = v.rows(None, 30, &t);
        assert!(rows.iter().all(|r| r.cells.len() <= 1 || r.heading));
        // `→` MOVES ACROSS on a wide door.
        v.settle_in(None, 200, &t);
        let before = v.rows(None, 200, &t)[v.cursor()].target.clone();
        v.key(Press::More, None, 200, &t);
        let after = v.rows(None, 200, &t)[v.cursor()].target.clone();
        assert_ne!(before, after);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: clearing the queue stops asking first, or clears an empty
    /// queue with a question about nothing.
    #[test]
    fn clearing_the_queue_asks_first() {
        let t = texts();
        let mut v = View::fresh();
        let state = playing();
        assert_eq!(v.key(Press::Clear, Some(&state), 80, &t), Errand::Nothing);
        assert!(v.asking.is_some());
        v.key(Press::Down, Some(&state), 80, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&state), 80, &t),
            Errand::TellSaying(Ask::Clear, _)
        ));
        let mut v = View::fresh();
        assert!(matches!(
            v.key(Press::Clear, Some(&State::default()), 80, &t),
            Errand::Said(_)
        ));
    }

    /// FAILS IF: the files open with the cursor on their heading — where the
    /// hint line teaches none of the level's keys, as the photograph of the
    /// first form showed.
    #[test]
    fn the_files_open_with_the_cursor_on_an_entry() {
        let t = texts();
        let root = a_library("open");
        let mut v = View::fresh();
        v.left_at = Some((0, root.clone()));
        v.key(Press::Browse, None, 80, &t);
        assert_eq!(on(&v), Target::Folder(root.join("Album")));
        // AND `Esc` LANDS BACK ON A ROW THAT ACTS.
        v.key(Press::Escape, None, 80, &t);
        assert!(on(&v).acts());
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a folder with nothing to play is handed to the player as an
    /// empty order instead of said.
    #[test]
    fn a_folder_with_nothing_to_play_says_so() {
        let t = texts();
        let root = std::env::temp_dir().join(format!("ronoctua-silent-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("notes.txt"), b"").unwrap();
        let mut v = among(&root);
        let rows = v.rows(None, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::PlayFolder)
            .unwrap();
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Said(t.t(ronoctua_i18n::key::PLAYER_BROWSE_NOTHING_TO_PLAY))
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_door_row_runs_its_command_and_only_on_enter() {
        assert_eq!(
            turn(&Target::Run("player-play-file"), Press::Enter, None),
            Errand::Run("player-play-file")
        );
        assert_eq!(
            turn(&Target::Run("player-play-file"), Press::More, None),
            Errand::Nothing
        );
    }

    #[test]
    fn with_no_player_the_rows_still_open_something() {
        let t = texts();
        let mut v = View::fresh();
        v.settle_in(None, 80, &t);
        let rows = v.rows(None, 80, &t);
        // THE FILES ARE THE FIRST THING OFFERED: with nothing loaded, finding
        // something to play is what the view is opened for.
        assert_eq!(rows[v.cursor()].target, Target::Browse);
    }

    /// FAILS IF: a pasted `file://` URL reaches the player as an address
    /// instead of the path it names.
    #[test]
    fn a_file_url_is_the_path_it_names() {
        assert_eq!(expand("file:///music/a%20b.flac"), "/music/a b.flac");
        assert_eq!(expand("file:///m%C3%BAsica/x.ogg"), "/música/x.ogg");
        assert_eq!(expand("https://x.org/a%20b"), "https://x.org/a%20b");
    }

    #[test]
    fn what_is_typed_is_an_address_or_a_path_against_home() {
        assert_eq!(
            typed("player-play-file", " https://example.org/a "),
            Some(Ask::Open {
                items: vec!["https://example.org/a".into()],
                replace: true
            })
        );
        assert_eq!(typed("player-play-file", "   "), None);
        let Some(Ask::Follow { item }) = typed("player-follow", "/v/b.mkv") else {
            panic!("a follow");
        };
        assert_eq!(item, "/v/b.mkv");
    }

    /// FAILS IF: a category the daemon can skip has no row of its own —
    /// the table's last arm answers for anything, so a forgotten one would be
    /// drawn under another's name — or a row stops flipping its category.
    #[test]
    fn every_category_is_a_row_of_its_own_and_turns() {
        use ronoctua_player::prefs::CATEGORIES;
        let names: std::collections::HashSet<&str> =
            CATEGORIES.iter().map(|c| super::skip_key(c)).collect();
        assert_eq!(names.len(), CATEGORIES.len(), "two categories share a name");
        let t = texts();
        let s = playing();
        let mut v = View::fresh();
        v.prefs = ronoctua_player::prefs::Preferences::default();
        let rows = v.rows(Some(&s), 80, &t);
        for c in CATEGORIES {
            assert!(
                rows.iter().any(|r| r.target == Target::Skip(c)),
                "{c} has no row"
            );
        }
        let at = rows
            .iter()
            .position(|r| r.target == Target::Skip("intro"))
            .expect("intro is a row");
        v.clicked(at, &rows);
        let Errand::Keep(p) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("a category writes the preferences");
        };
        assert!(
            p.skipped.iter().any(|c| c == "intro"),
            "intro was not added"
        );
        assert!(p.skipped.iter().any(|c| c == "sponsor"), "the others moved");
    }

    #[test]
    fn a_network_switch_turns_and_says_so() {
        let t = texts();
        let s = playing();
        let mut v = View::fresh();
        v.prefs = ronoctua_player::prefs::Preferences::default();
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Sponsors)
            .expect("the switch is a row");
        v.clicked(at, &rows);
        let Errand::Keep(p) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("a switch writes the preferences");
        };
        assert!(p.sponsorblock && !p.lyrics_online);
        assert!(
            rows[at]
                .fields
                .iter()
                .any(|f| f.text.contains(ronoctua_player::prefs::SPONSORBLOCK_HOST)),
            "the row names the host it reaches"
        );
    }

    #[test]
    fn a_paired_machine_joins_turns_and_leaves() {
        let mut s = playing();
        s.tailnet = Some("here".into());
        s.hosts = vec!["den".into()];
        let join = turn(&Target::Host("den".into()), Press::Enter, Some(&s));
        assert_eq!(
            join,
            Errand::Tell(Ask::Join {
                host: "den".into(),
                role: Role::Both
            })
        );
        s.joined = Some(ronoctua_player::wire::Joined {
            host: "den".into(),
            role: Role::Both,
            drift_ms: 12,
        });
        assert_eq!(
            turn(&Target::Host("den".into()), Press::More, Some(&s)),
            Errand::Tell(Ask::Join {
                host: "den".into(),
                role: Role::Screen
            })
        );
        assert_eq!(
            turn(&Target::Host("den".into()), Press::Less, Some(&s)),
            Errand::Tell(Ask::Join {
                host: "den".into(),
                role: Role::Sound
            })
        );
        assert_eq!(
            turn(&Target::Host("den".into()), Press::Enter, Some(&s)),
            Errand::Tell(Ask::Leave)
        );
    }

    /// FAILS IF: a pairing is sent on a door's loop — `carry_out` telling
    /// the player, which answers only after the other machine does — or a
    /// wait that ran out is said as "the player is not running" while the
    /// pairing may still succeed.
    #[test]
    fn a_pairing_is_the_doors_to_send_off_the_loop_and_a_slow_one_is_said_so() {
        let t = texts();
        let pair = Errand::Pair {
            to: "den".into(),
            code: Code("123456".into()),
        };
        assert_eq!(carry_out(pair.clone(), &t), Err(pair));
        let slow = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert!(timed_out(&slow));
        let also = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::TimedOut));
        assert!(timed_out(&also));
        let gone = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert!(!timed_out(&gone));
        assert!(!timed_out(&anyhow::anyhow!("the player did not answer")));
        let said = |p: Paired| paired_said(&p, &t);
        assert_eq!(
            said(Paired::NoAnswer),
            Some(t.t(ronoctua_i18n::key::PLAYER_PAIR_NO_ANSWER))
        );
        assert_ne!(said(Paired::NoAnswer), said(Paired::NotRunning));
        assert_eq!(
            said(Paired::NotRunning),
            Some(t.t(ronoctua_i18n::key::PLAYER_NOT_RUNNING))
        );
    }

    /// FAILS IF: pairing stops being two questions — the name in the clear,
    /// then the code MASKED — or the code pairs with no name asked before it,
    /// or a code's `Debug` prints the digits.
    #[test]
    fn pairing_asks_the_name_and_then_the_secret_code() {
        let t = texts();
        let mut s = playing();
        s.tailnet = Some("here".into());
        let mut v = View::fresh();
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Pair)
            .expect("the pairing row");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question {
                id: PAIR,
                secret: false
            })
        );
        let mut pairing = Pairing::default();
        assert_eq!(
            answered(PAIR, " den ", &mut pairing),
            Errand::Ask(Question {
                id: PAIR_CODE,
                secret: true
            })
        );
        let paired = answered(PAIR_CODE, " 123456 ", &mut pairing);
        assert_eq!(
            paired,
            Errand::Pair {
                to: "den".into(),
                code: Code("123456".into())
            }
        );
        assert!(!format!("{paired:?}").contains("123456"), "{paired:?}");
        // A CODE WITH NO NAME BEFORE IT is never mind, and so is an empty one.
        assert_eq!(answered(PAIR_CODE, "123456", &mut pairing), Errand::Nothing);
        answered(PAIR, "den", &mut pairing);
        pairing.let_go();
        assert_eq!(answered(PAIR_CODE, "123456", &mut pairing), Errand::Nothing);
        answered(PAIR, "den", &mut pairing);
        assert_eq!(answered(PAIR_CODE, "  ", &mut pairing), Errand::Nothing);
        assert_eq!(question(PAIR_CODE).map(|q| q.secret), Some(true));
    }

    /// FAILS IF: the typed path or address row stops being a question of the
    /// door's own line — for the purpose the files are open for — or closes
    /// the files under it, or an answer stops becoming the same order in
    /// whichever door typed it.
    #[test]
    fn a_typed_path_is_asked_with_the_files_still_open() {
        let t = texts();
        let root = a_library("typed");
        let mut v = among(&root);
        let rows = v.rows(None, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Typed)
            .expect("the typed row");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Ask(Question {
                id: ENQUEUE,
                secret: false
            })
        );
        assert!(v.files.is_some(), "the files closed under the question");
        let mut pairing = Pairing::default();
        assert_eq!(
            answered(ENQUEUE, "https://example.org/a", &mut pairing),
            Errand::Tell(Ask::Open {
                items: vec!["https://example.org/a".into()],
                replace: false
            })
        );
        assert_eq!(answered(ENQUEUE, "   ", &mut pairing), Errand::Nothing);
        assert_eq!(
            answered(FOLLOW, "/v/b.mkv", &mut pairing),
            Errand::Tell(Ask::Follow {
                item: "/v/b.mkv".into()
            })
        );
        // IN STEP, the same row asks the other question.
        v.files = None;
        v.open_the_files(Purpose::Follow);
        v.settle_in(None, 80, &t);
        let rows = v.rows(None, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Typed)
            .expect("the typed row");
        v.clicked(at, &rows);
        assert!(matches!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Ask(Question { id: FOLLOW, .. })
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: the position and the volume stop being bars a press sets —
    /// the bar found where the value was drawn, the press read along it, and
    /// the order the same arithmetic as the window's time bar and track.
    #[test]
    fn the_position_and_the_volume_are_bars_a_press_sets() {
        let t = texts();
        let s = playing();
        let v = View::fresh();
        let rows = v.rows(Some(&s), 80, &t);
        let position = rows
            .iter()
            .position(|r| r.target == Target::Position)
            .expect("the position row");
        let bar = rows[position].slider.expect("the position is a bar");
        assert_eq!(bar.of, Slid::Position);
        assert_eq!(
            Some(bar.from),
            rows[position].fields.get(1).map(|f| f.at),
            "the bar is not where the value was drawn"
        );
        let middle = f64::from(bar.from) + f64::from(bar.cells) / 2.0;
        let found = slider_at(&rows, position, middle).expect("a press on the bar");
        assert_eq!(
            slid(found.of, found.along(middle), Some(&s)),
            Errand::Tell(Ask::Seek {
                seconds: 3725.0 / 2.0,
                relative: false
            })
        );
        // OFF THE BAR, it is the row's click, and not the bar's.
        assert_eq!(slider_at(&rows, position, f64::from(bar.from) - 1.0), None);
        // A DRAG PAST ITS END keeps the end.
        assert_eq!(bar.along(f64::from(bar.from + bar.cells) + 40.0), 1.0);
        let volume = rows
            .iter()
            .position(|r| r.target == Target::Volume)
            .expect("the volume row");
        let bar = rows[volume].slider.expect("the volume is a bar");
        assert_eq!(
            slid(bar.of, 1.0, Some(&s)),
            Errand::Tell(Ask::Volume {
                percent: VOLUME_MOST
            })
        );
        // A STREAM HAS NO LENGTH, and its position is no bar.
        let mut live = playing();
        live.duration = None;
        let rows = v.rows(Some(&live), 80, &t);
        assert!(
            rows.iter()
                .find(|r| r.target == Target::Position)
                .is_some_and(|r| r.slider.is_none())
        );
        assert_eq!(slid(Slid::Position, 0.5, Some(&live)), Errand::Nothing);
    }

    /// FAILS IF: the footer's picture stops being the column's width at the
    /// region's right end with the bar's cell left free, or leaves the rows
    /// less than half, or Only stops taking the whole region.
    #[test]
    fn the_footers_picture_is_beside_the_rows_or_the_whole_region() {
        let wide = 200;
        let beside = footer_panel(wide, false).expect("room for the picture");
        assert_eq!(beside.cells, tokens::space::COLUMN_WIDTH);
        assert_eq!(
            beside.from + beside.cells,
            wide - tokens::space::FILES_EDGERIGHT,
            "the bar's cell is taken"
        );
        assert!(beside.from >= beside.cells, "the rows keep less than half");
        let narrow = tokens::space::COLUMN_WIDTH * 2 - 1;
        assert_eq!(footer_panel(narrow, false), None);
        let only = footer_panel(wide, true).expect("the whole region");
        assert_eq!(
            (only.from, only.cells),
            (0, wide - tokens::space::FILES_EDGERIGHT)
        );
    }

    /// FAILS IF: the view stops being able to stop — the key the window's
    /// transport glyph runs — or a chord is read as the letter under it.
    #[test]
    fn stop_is_a_key_of_the_view_and_a_chord_is_not_its_letter() {
        use ronoctua_keymap::{Key, Mods, Sym};
        let t = texts();
        let s = playing();
        let mut v = View::fresh();
        let stop = press_of_key(Key::simple(Sym::Char('s')));
        assert_eq!(stop, Some(Press::Stop));
        assert_eq!(
            v.key(Press::Stop, Some(&s), 80, &t),
            Errand::Tell(Ask::Stop)
        );
        let chord = Key {
            sym: Sym::Char('s'),
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
        };
        assert_eq!(press_of_key(chord), None, "C-s was read as s");
        assert_eq!(press_of_key(Key::simple(Sym::Esc)), Some(Press::Escape));
    }

    #[test]
    fn every_key_of_the_table_is_a_press() {
        for (k, id, _) in OF_THE_VIEW {
            assert!(pressed(k).is_some(), "{k} → {id}");
        }
    }

    #[test]
    fn the_mark_says_play_or_pause_and_nothing_without_an_item() {
        let mut s = playing();
        assert_eq!(mark(&s), Some(tokens::icon::PLAYER));
        s.playing = false;
        assert_eq!(mark(&s), Some(tokens::icon::PAUSE));
        s.current = None;
        assert_eq!(mark(&s), None);
    }
}
