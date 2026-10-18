//! THE PLAYER'S CAMERA: its window and its column in a PNG, with no
//! compositor and no VM — `ronoctua-shell photograph`, whose door the binary
//! dispatches (`photograph.rs` there, which hands this camera the doors that
//! are not the player's: [`OtherDoors`]).
//!
//! # Why this exists
//!
//! `ronoctua-render-png` photographs a buffer in a fifth of a second, and it
//! cannot reach a native door: it is a binary of its own that borrows two
//! pure modules, and the player's window and column pull in the whole shell.
//! Every page of the player, at every size of the column and in both
//! languages, could be seen only in a VM — three minutes a picture — and a
//! rule that costs that much is a rule that gets skipped. So the camera lives
//! beside the doors it photographs, in the player's crate.
//!
//! # What makes the picture worth trusting
//!
//! That nothing here is a second implementation. The door is the production
//! one, DETACHED (`door::Detached`): its `Ui` is on a connection nobody
//! serves, its column is the production column around the player, and its
//! player has no daemon behind it. A key goes down the path a key takes, a
//! click down the one a click takes — the header's shortcuts, the bars'
//! columns, the first click that selects and the second that acts — and the
//! frame is composed by `door::compose`, the function `Door::paint` calls.
//! What differs is where the pixels land: a texture read back into a PNG,
//! drawn through `ronoctua_render::offscreen` as `render-png` draws.
//!
//! # What reaches nothing
//!
//! A detached player SENDS NOTHING: a photograph that clicked play on the
//! person's own daemon would pause their film. Everything past the door goes
//! through one field (`player_window::Reach`), and a photograph's keeps it:
//! what a click would have asked, written, paired, opened or told the footer
//! is printed instead (`asked the player: …`, `would have …`), which is what
//! a click in a photograph proves. What the daemon would ANSWER is the next
//! `--state`, given in the walk after the click.
//!
//! It starts no daemon, opens no window for a stage, asks no compositor
//! anything and holds no column's slot; it reads the player's preferences
//! and the date's shape as their defaults, and browses `--places` instead of
//! this machine's folders — a photograph is the same picture on every
//! machine. Its clock moves only between the steps of the walk, so two
//! clicks are a double click because the walk says `double`, never because
//! a step was quick. The one thing it cannot show is a real film: the
//! daemon's frames arrive through a memfd, and here a still stands in for
//! them (`--frame`).
//!
//! # The doors that are not the player's
//!
//! The binary's: a door with a camera of its own takes the rest of the words
//! (`--door files`), and a few native doors RIDE this walk — keys, the
//! pointer, the frames — drawn in the player window's place (the update's
//! window, the footer's settings, the calculator: [`Rider`]). The pages
//! written above this crate answer their own words through the seam
//! (`--make-catalog`, `--handed`: `player_seam::Page`). So nothing here names
//! another feature, and a door added to the binary recompiles nothing of the
//! player.
//!
//! Usage: `ronoctua-shell photograph [options]`, `--help` for the list.

use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_player::queue::Outline;
use ronoctua_player::wire::{Say, State};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::column::{Column, Size};
use crate::door::{Composed, Content, Detached, LEFT_BUTTON, RIGHT_BUTTON, Ui};
use crate::player_window::{Door as Which, Outbox, Player, Stage};

/// THIS CAMERA'S WORDS, after the head the binary hands it ([`OtherDoors::usage`]):
/// what `--help` prints, the head first.
pub const USAGE: &str = "  --size <size>         the column's: large, medium, small or bar
                                                      (default: medium)
  --screen <px>         the column's screen, in logical pixels: it clamps
                        the column and refuses a size that leaves the
                        windows no room          (default: no screen at all)
  --stage <stage>       off, only, maximized, small or listing (default:
                        off);
                        a column shows only `off` and `only`, `maximized`
                        wants the screen's size (--width and --height), and
                        `small` is a size of its own, and takes none
  --frame <lane>=<png>  a still standing in for the daemon's frames of a
                        lane's picture, by the lane's id — or `every`, for
                        every picture with no still of its own; refused
                        when no picture of that lane is ever laid out
  --output <connector>[=<description>]  a screen the door knows by name,
                        as a compositor describes one — repeatable, and the
                        first is the one the door is on: what is routed to
                        it is what the door shows, and a lane's page lists
                        them (default: no screen known, every picture shown)
  --kept <key,...>      the person's keys as kept — opensubtitles,
                        opensubtitles-account, tmdb, subdl — never read from
                        this machine's files (default: none kept)
  --favourites <file>   the favourites, as the player keeps them
                        (`player-favourites.json`) — never this machine's
  --recent <file>       the played lately, the same way
                        (`player-recent.json`)          (default: none of either)
  --places <dir>        the file picker's home, and its places the folders
                        in it (Music, Videos…), never this machine's; a
                        walk that opens the picker without it is refused
  --websites <file>     the website page's addresses played lately, as it
                        keeps them (`player-websites.json`) — never this
                        machine's                      (default: none)
  --catalog <dir>       the player's catalogue is this folder's, never this
                        machine's (`--make-catalog <dir>` writes one there,
                        with posters, and ends)
  --ranking <file>      the video catalogue the ranking's pages read
                        (`LEADER p k`), never this machine's; without it
                        they read none, and say so
  --pam <answer>        how a password typed for the adult catalogue is
                        answered: right, wrong, absent (no lock service) or
                        unavailable (default: none, nothing answers)
  --handed <file>       the window was opened for the listing's whole
                        screen, handed everything of the catalogue's
                        person's source (`--make-catalog`'s) in the `main`
                        or the `adult` file, as a footer's `F` hands it
  --out <file>          where to write                (default: door.png)
  --columns <n>         the window's width, in cells
  --lines <n>           the height, in lines
  --width <px>          the window's width, in logical pixels
  --height <px>         the height, in logical pixels
                        (one of each pair; default: the door's own size
                        before a compositor says one; a column's width is
                        its size's)
  --language <id>       pt-BR or en-GB                (default: pt-BR)
  --theme <id>          a theme of assets/themes      (default: the default)
  --page <id>           chrome, backdrop, near or absolute (default: chrome)
  --text <percent>      text size, in percent         (default: 100)
  --scale <n>           physical px per logical       (default: 1)
  --list                prints the frame instead of drawing it — and every
                        group of the globe's marks it drew, at the cell its
                        centre is in and the point itself (`globe mark [ids]
                        at column,line px x,y`)

Steps — repeatable, and walked IN THE ORDER they appear, the way a person
would; that is how a page that only exists after walking to it is reached:

  --state <file>        the player's state: one answer of its socket to
                        {\"ask\":\"look\"}, as it says it. Before any other
                        step it is the door's first; after one, it is the
                        daemon's answer to it
  --queue <file>        the queue's outline: one answer of its socket to
                        {\"ask\":\"queue\"} — what the queue's rows are
                        drawn from; before or after a step, as --state
  --sheet <file>        a file's sheet: one answer of its socket to
                        {\"ask\":\"sheet\",...} — what the Metadata section
                        shows of the file that plays, its cover drawn from
                        the `cover_file` it names; after the --state whose
                        file it is
  --desk <file>         a desk's page: one answer of its socket to
                        {\"ask\":\"desk\",...} — the suggestions, the
                        subtitles or the lyrics found, a suggested cover
                        drawn from the file it names; after the --state
                        whose desks it is
  --refused <file>      a directory's list the player refused at once:
                        {\"query\":{...},\"trouble\":{...}} — as the answer
                        to its find comes back to the door that asked
  --overrule <file>     a refusal the person may overrule, as the answer to
                        the ask it refused comes back to the door:
                        {\"again\":{...},\"trouble\":{...}} — the ask that
                        overrules it (a put without `once`, a save with
                        `force`) and the refusal; the door asks, \"no\" first
  --key <notation>      a key, as the keymap writes it: SPC, RET, <down>,
                        C-<left>, o
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line[,right|double]>  a press and a release there (twice,
                        for `double`)
  --hover-at <x,y>      rests the pointer on a POINT of the surface, in
                        logical pixels — a mark of a globe is smaller than a
                        cell, and a cell's middle may miss it (`--list`
                        says where each one was drawn)
  --click-at <x,y>      a press and a release on such a point
  --drag <col,line:col,line>  a press on the first cell and the pointer
                        moved to the second, the button still held — a
                        band's ghost, a bar being dragged
  --let-go              the held button comes up where the drag left it
  --rest                the controls over a picture time out, as they do
                        when nobody touches them
  --open <page>         a page of the player opened over the one on screen,
                        by its own key (its title, `player-page-…`): the
                        walk the row that opens it would take
                        (`player-pages-fit-every-size` photographs every page
                        this way; the home is the frame without it)
  --list-pages          print the key of every page `--open` opens, one a
                        line, and end
  --edit <file>         a text of several lines opened over the page,
                        holding the file's words — what a view's \"edit here\"
                        opens (`text_area`); the keys and clicks after it
                        type and place the caret

A cell is counted from the surface's own corner, margins included, and one
outside the surface is refused. A step that closes the door — `Esc` from the
top, a column's `C-q` or its close glyph, the swap — ends the walk: it says it
would have closed the door, the picture is the last frame it drew — before that
step — and a step after it is refused. The clock moves a second between two steps
and not inside one: two `--click`s are two clicks.

A click on the daemon's controls changes nothing in the picture by itself —
the daemon is not there to answer. What it would have asked is printed
(`asked the player: …`), as is everything else it would have done past the
door; the answer is the `--state` that follows it.";

/// A DOOR'S CAMERA OF ITS OWN: it reads the rest of the words after
/// `--door <word>` and ends the process.
pub type OwnCamera = fn(&[String]) -> !;

/// THE DOORS THAT ARE NOT THE PLAYER'S, as the binary hands them to this
/// camera (`photograph.rs` there): the cameras of their own, the doors that
/// ride this walk, and their words.
#[derive(Clone, Copy)]
pub struct OtherDoors {
    /// The head of `--help`: the usage line, the doors and the words of the
    /// doors that ride this walk — this camera's own follow ([`USAGE`]).
    pub usage: &'static str,
    /// Every door's word, as a door that is none of them is refused with.
    pub named: &'static str,
    /// The camera of its own a door's word names — it reads the rest of the
    /// words and ends the process — or `None`.
    pub camera: fn(&str) -> Option<OwnCamera>,
    /// The doors that ride this walk, drawn in the player window's place.
    pub riders: &'static [Rider],
    /// The riders' own words, each with whether a value follows it — kept
    /// in the order given ([`Asked::given`], [`Asked::value_of`],
    /// [`Asked::values_of`]).
    pub words: &'static [(&'static str, bool)],
    /// What the riders' words refuse once every word is read: `door` the
    /// rider asked, `None` the player's own door.
    pub read: fn(door: Option<&'static str>, asked: &Asked) -> Result<()>,
    /// What they refuse when the player's own door is about to be drawn.
    pub drawing: fn(asked: &Asked) -> Result<()>,
}

impl std::fmt::Debug for OtherDoors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OtherDoors")
            .field("named", &self.named)
            .field("riders", &self.riders)
            .finish_non_exhaustive()
    }
}

/// A DOOR THAT IS NO PLAYER'S AND RIDES THIS WALK — keys, the pointer, the
/// frames, `--list` — drawn in the player window's place: the binary's
/// update window, the footer's settings, the calculator.
#[derive(Clone, Copy)]
pub struct Rider {
    /// Its word after `--door`.
    pub door: &'static str,
    /// Drives and draws it, in the language and at the scale asked: its
    /// door made over what the words gave, then [`shoot`].
    pub shoot: fn(texts: ronoctua_i18n::Texts, scale: i32, asked: &Asked) -> Result<()>,
}

impl std::fmt::Debug for Rider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rider")
            .field("door", &self.door)
            .finish_non_exhaustive()
    }
}

/// One step of the walk, in the order asked.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// A key pressed and let go.
    Key(ronoctua_keymap::Key),
    /// The pointer rests on a cell.
    Hover {
        /// The cell's column, from the surface's left edge.
        column: u16,
        /// The cell's line, from the top.
        line: u16,
    },
    /// A press and a release on a cell.
    Click {
        /// The cell's column.
        column: u16,
        /// The cell's line.
        line: u16,
        /// Which button.
        button: u32,
        /// Twice, with no time between.
        double: bool,
    },
    /// A press on one cell and the pointer moved to another, the button
    /// still held.
    Drag {
        /// Where it is pressed: column and line.
        from: (u16, u16),
        /// Where it is moved to.
        to: (u16, u16),
    },
    /// The pointer on a point of the surface, in logical pixels — resting
    /// there, or pressed and let go there.
    Point {
        /// The point.
        at: (f64, f64),
        /// A click, and not only a rest.
        click: bool,
    },
    /// The held button comes up where the drag left it.
    LetGo,
    /// The controls' time runs out (`Player::rest`).
    Rest,
    /// A page of the player opened over the one on screen, as the row that
    /// opens it would (`Player::open_page`).
    Open(crate::player::PageId),
    /// A text of several lines opened over the page, holding these words
    /// (`player::View::edit_text`).
    Text(String),
    /// A state from the player: the door's first, or its answer to a step.
    State(Box<State>),
    /// The queue's outline, as the player says it.
    Queue(Box<Outline>),
    /// A file's sheet, as the player answers `{"ask":"sheet"}`.
    Sheet(Box<ronoctua_tags::Sheet>),
    /// A desk's page, as the player answers `{"ask":"desk"}`.
    Desk(Box<ronoctua_player::find::Desk>),
    /// A directory's list the player REFUSED at once, as the answer to its
    /// `find` comes back to the door that asked (`player::Told::Listing`).
    Refused(Box<(ronoctua_player::find::Query, ronoctua_player::wire::Trouble)>),
    /// A refusal the person may OVERRULE, as the player answers the ask it
    /// refused (`player::Told::Asks`): the ask that overrules it and why.
    Overrule(Box<(ronoctua_player::wire::Ask, ronoctua_player::wire::Trouble)>),
}

/// What `--overrule` reads: the ask that overrules the refusal, and the
/// refusal.
#[derive(serde::Deserialize)]
struct Overruled {
    again: ronoctua_player::wire::Ask,
    trouble: ronoctua_player::wire::Trouble,
}

/// What `--refused` reads: the list asked, and the player's refusal of it.
#[derive(serde::Deserialize)]
struct RefusedList {
    query: ronoctua_player::find::Query,
    trouble: ronoctua_player::wire::Trouble,
}

/// A person's pause between two steps of the walk, on the photograph's
/// clock: longer than a double click, so two steps are never one.
const APART: std::time::Duration = std::time::Duration::from_secs(1);

const _: () = assert!(
    APART.as_millis() > crate::player_window::DOUBLE_CLICK.as_millis(),
    "two steps of a walk must never make a double click"
);

/// A length the person gave: in cells, or in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measure {
    /// Cells (columns) or lines.
    Cells(u16),
    /// Logical pixels.
    Px(u32),
}

/// What was asked for on the command line — handed to a door that rides
/// the walk ([`Rider::shoot`]), which reads its own words off it.
#[derive(Debug)]
pub struct Asked {
    door: Which,
    /// A door that is no player's (`--door footer-settings`), drawn in place
    /// of the player's window.
    rider: Option<&'static Rider>,
    /// The riders' own words (`--modeline <file>`), in the order given, each
    /// with its value when it takes one ([`OtherDoors::words`]).
    riding: Vec<(&'static str, Option<String>)>,
    size: Option<Size>,
    screen: Option<u32>,
    stage: Stage,
    stills: BTreeMap<u32, std::path::PathBuf>,
    every: Option<std::path::PathBuf>,
    outputs: Vec<crate::player_lane::Known>,
    kept: Vec<ronoctua_lookup::keys::Key>,
    favourites: Vec<ronoctua_broadcast::picks::Pick>,
    /// The website page's addresses played lately (`--websites`).
    websites: Vec<String>,
    recent: Vec<ronoctua_broadcast::picks::Pick>,
    places: Option<std::path::PathBuf>,
    /// `--catalog`: the player's catalogue the door reads, never this
    /// machine's.
    catalog: Option<std::path::PathBuf>,
    /// `--ranking`: the video catalogue the ranking reads, never this
    /// machine's (the ranking's page, `ronoctua-shell-video-metrics`).
    ranking: Option<std::path::PathBuf>,
    /// `--pam`: how a password typed for the adult catalogue is answered.
    pam: Option<Result<(), ronoctua_pam::Refusal>>,
    /// `--handed`: the window opened for the listing's stage on a listing of
    /// this file, handed over.
    handed: Option<ronoctua_broadcast::catalog::Store>,
    steps: Vec<Step>,
    out: std::path::PathBuf,
    width: Option<Measure>,
    height: Option<Measure>,
    language: String,
    theme: tokens::theme::Theme,
    page: tokens::theme::Ground,
    text: u16,
    scale: u16,
    list: bool,
}

impl Asked {
    /// The door asked — a rider's is drawn in the window's place.
    #[must_use]
    pub const fn door(&self) -> Which {
        self.door
    }

    /// The door that rides the walk, when one was asked.
    #[must_use]
    pub const fn rider(&self) -> Option<&'static Rider> {
        self.rider
    }

    /// The column's size, when one was asked — a word of the player's.
    #[must_use]
    pub const fn size(&self) -> Option<Size> {
        self.size
    }

    /// The stage asked — a word of the player's.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        self.stage
    }

    /// Whether the rider's word `word` was given.
    #[must_use]
    pub fn given(&self, word: &str) -> bool {
        self.riding.iter().any(|(w, _)| *w == word)
    }

    /// The value the rider's word `word` was given last — a later one is
    /// the answer, as a second `--modeline` replaces the first.
    #[must_use]
    pub fn value_of(&self, word: &str) -> Option<&str> {
        self.riding
            .iter()
            .rev()
            .find(|(w, _)| *w == word)
            .and_then(|(_, v)| v.as_deref())
    }

    /// Every value the rider's word `word` was given, in order — a word
    /// that repeats (`--report`).
    #[must_use]
    pub fn values_of(&self, word: &str) -> Vec<&str> {
        self.riding
            .iter()
            .filter(|(w, _)| *w == word)
            .filter_map(|(_, v)| v.as_deref())
            .collect()
    }
}

/// A whole number the person typed, inside `range`.
fn number<T>(flag: &str, raw: &str, range: std::ops::RangeInclusive<T>) -> Result<T>
where
    T: std::str::FromStr + PartialOrd + std::fmt::Display + Copy,
{
    let n: T = raw
        .trim()
        .parse()
        .ok()
        .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
    ensure!(
        range.contains(&n),
        "`{flag} {n}` is out of range; the range is {}..={}",
        range.start(),
        range.end()
    );
    Ok(n)
}

/// A cell, `column,line` — and, for a click, `,right` or `,double` after it.
fn cell_of(flag: &str, raw: &str) -> Result<(u16, u16, Option<String>)> {
    let mut parts = raw.split(',');
    let (Some(column), Some(line)) = (parts.next(), parts.next()) else {
        bail!("`{flag}` wants `column,line`, not `{raw}`");
    };
    let rest = parts.next().map(|p| p.trim().to_owned());
    ensure!(parts.next().is_none(), "`{flag} {raw}` has a word too many");
    Ok((
        number(flag, column, 0..=999)?,
        number(flag, line, 0..=999)?,
        rest,
    ))
}

/// The page a `--page` names, by the generator's word for it — refused,
/// with the words it knows, when it is none of them.
fn page_by_id(id: &str) -> Result<tokens::theme::Ground> {
    tokens::theme::Ground::ALL
        .into_iter()
        .find(|g| g.id() == id)
        .with_context(|| {
            let known: Vec<&str> = tokens::theme::Ground::ALL.iter().map(|g| g.id()).collect();
            format!("unknown page `{id}`; the pages are {}", known.join(", "))
        })
}

/// The player's state, out of a file that holds one answer of its socket to
/// `{"ask":"look"}` — in the daemon's own spelling, so the fixture is not a
/// second shape invented for this tool.
fn state_of(raw: &str, file: &str) -> Result<State> {
    let said: Say = serde_json::from_str(raw.trim())
        .with_context(|| format!("`{file}` is not an answer of the player"))?;
    let Say::State(state) = said else {
        bail!("`{file}` is an answer of the player, but not its state (`{{\"ask\":\"look\"}}`)");
    };
    Ok(*state)
}

/// The queue's outline, out of a file that holds one answer of its socket to
/// `{"ask":"queue"}` — the daemon's own spelling, as a state's.
fn outline_of(raw: &str, file: &str) -> Result<Outline> {
    let said: Say = serde_json::from_str(raw.trim())
        .with_context(|| format!("`{file}` is not an answer of the player"))?;
    let Say::Queue(outline) = said else {
        bail!("`{file}` is an answer of the player, but not its queue (`{{\"ask\":\"queue\"}}`)");
    };
    Ok(*outline)
}

/// A file's sheet, out of a file that holds one answer of its socket to
/// `{"ask":"sheet"}` — the daemon's own spelling, as a state's.
fn sheet_of(raw: &str, file: &str) -> Result<ronoctua_tags::Sheet> {
    let said: Say = serde_json::from_str(raw.trim())
        .with_context(|| format!("`{file}` is not an answer of the player"))?;
    let Say::Sheet(sheet) = said else {
        bail!("`{file}` is an answer of the player, but not a sheet (`{{\"ask\":\"sheet\"}}`)");
    };
    Ok(*sheet)
}

/// A desk's page, out of a file that holds one answer of its socket to
/// `{"ask":"desk"}` — the daemon's own spelling, as a state's.
fn desk_of(raw: &str, file: &str) -> Result<ronoctua_player::find::Desk> {
    let said: Say = serde_json::from_str(raw.trim())
        .with_context(|| format!("`{file}` is not an answer of the player"))?;
    let Say::Desk(desk) = said else {
        bail!("`{file}` is an answer of the player, but not a desk (`{{\"ask\":\"desk\"}}`)");
    };
    Ok(*desk)
}

/// Reads the words after `photograph`, the doors that are not the player's
/// as `doors` says them.
///
/// # Errors
///
/// A word not understood, a value that is not what it says, a file that
/// cannot be read, or what no door can be.
#[allow(clippy::too_many_lines)]
pub fn parse(words: &[String], doors: &OtherDoors) -> Result<Asked> {
    let mut me = Asked {
        door: Which::Window,
        rider: None,
        riding: Vec::new(),
        size: None,
        screen: None,
        stage: Stage::Off,
        stills: BTreeMap::new(),
        every: None,
        outputs: Vec::new(),
        kept: Vec::new(),
        favourites: Vec::new(),
        websites: Vec::new(),
        recent: Vec::new(),
        places: None,
        catalog: None,
        ranking: None,
        pam: None,
        handed: None,
        steps: Vec::new(),
        out: std::path::PathBuf::from("door.png"),
        width: None,
        height: None,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        page: tokens::theme::Ground::Chrome,
        text: tokens::text::NORMAL,
        scale: 1,
        list: false,
    };
    let mut words = words.iter();
    while let Some(flag) = words.next() {
        let mut value = || {
            words
                .next()
                .cloned()
                .with_context(|| format!("`{flag}` with no value"))
        };
        match flag.as_str() {
            "--help" | "-h" => {
                println!("{}{USAGE}", doors.usage);
                std::process::exit(0);
            }
            "--door" => {
                me.door = match value()?.as_str() {
                    "window" => Which::Window,
                    "column" => Which::Column,
                    // EVERY OTHER DOOR IS THE BINARY'S: one that rides this
                    // walk is drawn in the window's place, and one with a
                    // camera of its own takes the rest of the words.
                    other => {
                        if let Some(rider) = doors.riders.iter().find(|r| r.door == other) {
                            me.rider = Some(rider);
                            Which::Window
                        } else if let Some(take) = (doors.camera)(other) {
                            take(words.as_slice())
                        } else {
                            bail!("`{other}` is not a door; the doors are {}", doors.named)
                        }
                    }
                };
            }
            "--size" => {
                let said = value()?;
                me.size = Some(Size::named(&said).with_context(|| {
                    format!(
                        "`{said}` is not a size; the sizes are {}",
                        Size::ALL.map(Size::name).join(", ")
                    )
                })?);
            }
            "--stage" => {
                me.stage = match value()?.as_str() {
                    "off" => Stage::Off,
                    "only" => Stage::Only,
                    "maximized" => Stage::Maximized,
                    "small" => Stage::Small,
                    "listing" => Stage::Listing,
                    other => bail!(
                        "`{other}` is not a stage; the stages are off, only, maximized, small \
                         and listing"
                    ),
                };
            }
            "--screen" => me.screen = Some(number("--screen", &value()?, 1..=65535)?),
            "--state" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                me.steps.push(Step::State(Box::new(state_of(&raw, &file)?)));
            }
            "--queue" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                me.steps
                    .push(Step::Queue(Box::new(outline_of(&raw, &file)?)));
            }
            "--sheet" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                me.steps.push(Step::Sheet(Box::new(sheet_of(&raw, &file)?)));
            }
            "--desk" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                me.steps.push(Step::Desk(Box::new(desk_of(&raw, &file)?)));
            }
            "--refused" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                let r: RefusedList = serde_json::from_str(&raw).with_context(|| {
                    format!("`{file}` is not a list asked and the player's refusal of it")
                })?;
                me.steps.push(Step::Refused(Box::new((r.query, r.trouble))));
            }
            "--overrule" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                let r: Overruled = serde_json::from_str(&raw).with_context(|| {
                    format!("`{file}` is not an ask and the refusal it overrules")
                })?;
                me.steps
                    .push(Step::Overrule(Box::new((r.again, r.trouble))));
            }
            "--websites" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                let kept: crate::player_website::Addresses = serde_json::from_str(&raw)
                    .with_context(|| {
                        format!("`{file}` is not a list of addresses as the website page keeps one")
                    })?;
                me.websites = kept.addresses;
            }
            "--favourites" | "--recent" => {
                let file = value()?;
                let raw = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                let picks: ronoctua_broadcast::picks::Picks = serde_json::from_str(&raw)
                    .with_context(|| {
                        format!("`{file}` is not a list of picks as the player writes one")
                    })?;
                if flag == "--favourites" {
                    me.favourites = picks.picks;
                } else {
                    me.recent = picks.picks;
                }
            }
            "--kept" => {
                use ronoctua_lookup::keys::Key;
                for word in value()?.split(',').map(str::trim).filter(|w| !w.is_empty()) {
                    let key = Key::ALL
                        .into_iter()
                        .find(|k| k.file() == word)
                        .with_context(|| {
                            format!(
                                "`--kept {word}` is not a key; the keys are {}",
                                Key::ALL.map(Key::file).join(", ")
                            )
                        })?;
                    ensure!(!me.kept.contains(&key), "`--kept {word}` was given twice");
                    me.kept.push(key);
                }
            }
            "--places" => {
                let dir = std::path::PathBuf::from(value()?);
                ensure!(dir.is_dir(), "`--places {}` is not a folder", dir.display());
                me.places = Some(dir);
            }
            // THE PLAYER'S CATALOGUE, a folder of its own — made with
            // `--make-catalog` — and how the adult lock's password is
            // answered (the catalogue's page, `ronoctua-shell-catalogue`).
            "--catalog" => {
                let dir = std::path::PathBuf::from(value()?);
                ensure!(
                    dir.is_dir(),
                    "`--catalog {}` is not a folder",
                    dir.display()
                );
                me.catalog = Some(dir);
            }
            // THE VIDEO CATALOGUE THE RANKING READS (its page,
            // `ronoctua-shell-video-metrics`).
            "--ranking" => {
                let file = std::path::PathBuf::from(value()?);
                ensure!(
                    file.is_file(),
                    "`--ranking {}` is not a file",
                    file.display()
                );
                me.ranking = Some(file);
            }
            "--list-pages" => {
                for page in crate::player::PageId::PAGES {
                    println!("{}", page.key());
                }
                std::process::exit(0);
            }
            // THE PAGE THAT KEEPS A CATALOGUE writes its fixture — a page
            // written above this crate (`player_seam::Page::write_fixture`).
            "--make-catalog" => {
                let dir = std::path::PathBuf::from(value()?);
                let made = crate::player_seam::installed()
                    .iter()
                    .find_map(|page| page.write_fixture(&dir))
                    .context("no page of the player keeps a catalogue to write")?;
                made.map_err(|t| {
                    anyhow::anyhow!("the catalogue was not made in {}: {t}", dir.display())
                })?;
                println!("made a catalogue in {}", dir.display());
                std::process::exit(0);
            }
            "--pam" => {
                use ronoctua_pam::Refusal;
                me.pam = Some(match value()?.as_str() {
                    "right" => Ok(()),
                    "wrong" => Err(Refusal::Wrong),
                    "absent" => Err(Refusal::NoService),
                    "unavailable" => Err(Refusal::Unavailable),
                    other => bail!(
                        "`--pam {other}` is not an answer; the answers are right, wrong, absent \
                         and unavailable"
                    ),
                });
            }
            "--handed" => {
                use ronoctua_broadcast::catalog::Store;
                me.handed = Some(match value()?.as_str() {
                    "main" => Store::Main,
                    "adult" => Store::Adult,
                    other => bail!("`--handed {other}` is no file; the files are main and adult"),
                });
            }
            "--frame" => {
                let said = value()?;
                let Some((lane, file)) = said.split_once('=') else {
                    bail!("`--frame` wants `<lane>=<png>`, not `{said}`");
                };
                if lane == "every" {
                    ensure!(me.every.is_none(), "`--frame every=` was given twice");
                    me.every = Some(std::path::PathBuf::from(file));
                } else {
                    let lane = number("--frame", lane, 1..=u32::MAX)?;
                    ensure!(
                        me.stills
                            .insert(lane, std::path::PathBuf::from(file))
                            .is_none(),
                        "`--frame {lane}=` was given twice"
                    );
                }
            }
            "--output" => {
                let said = value()?;
                let (connector, description) = match said.split_once('=') {
                    Some((c, d)) => (c.trim(), Some(d.trim().to_owned())),
                    None => (said.trim(), None),
                };
                ensure!(
                    !connector.is_empty() && !connector.contains(char::is_whitespace),
                    "`--output {said}` names no connector"
                );
                ensure!(
                    !me.outputs.iter().any(|k| k.connector == connector),
                    "`--output {connector}` was given twice"
                );
                me.outputs.push(crate::player_lane::Known {
                    connector: connector.to_owned(),
                    description: description.filter(|d| !d.is_empty()),
                });
            }
            "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
            "--columns" => {
                let n = number("--columns", &value()?, 1..=1000)?;
                once(&mut me.width, Measure::Cells(n), "--columns", "--width")?;
            }
            "--lines" => {
                let n = number("--lines", &value()?, 1..=1000)?;
                once(&mut me.height, Measure::Cells(n), "--lines", "--height")?;
            }
            "--width" => {
                let px = number("--width", &value()?, 1..=16384)?;
                once(&mut me.width, Measure::Px(px), "--columns", "--width")?;
            }
            "--height" => {
                let px = number("--height", &value()?, 1..=16384)?;
                once(&mut me.height, Measure::Px(px), "--lines", "--height")?;
            }
            "--language" => me.language = value()?,
            "--theme" => {
                let id = value()?;
                me.theme = tokens::theme::by_id(&id).with_context(|| {
                    let known: Vec<&str> = tokens::theme::all().map(|t| t.id()).collect();
                    format!("unknown theme `{id}`; the themes are {}", known.join(", "))
                })?;
            }
            "--page" => me.page = page_by_id(&value()?)?,
            "--text" => {
                // REFUSED RATHER THAN SNAPPED, as `render-png` refuses it: a
                // photograph at a size nobody asked for is worse than none.
                let asked = number(
                    "--text",
                    &value()?,
                    tokens::font::SCALE_FLOOR..=tokens::font::SCALE_CEILING,
                )?;
                let held = tokens::text::allowed(asked);
                ensure!(
                    held == asked,
                    "`--text {asked}` is not a size the person can reach; the nearest is {held}"
                );
                me.text = held;
            }
            "--scale" => {
                let most = u16::try_from(ronoctua_render::MOST_SCALE).unwrap_or(1);
                me.scale = number("--scale", &value()?, 1..=most)?;
            }
            "--list" => me.list = true,
            "--key" => {
                let notation = value()?;
                let key = ronoctua_keymap::Key::parse(&notation)
                    .map_err(|e| anyhow::anyhow!("`--key {notation}` is not a key: {e}"))?;
                me.steps.push(Step::Key(key));
            }
            "--hover" => {
                let (column, line, rest) = cell_of("--hover", &value()?)?;
                ensure!(rest.is_none(), "`--hover` wants `column,line`");
                me.steps.push(Step::Hover { column, line });
            }
            "--click" => {
                let (column, line, rest) = cell_of("--click", &value()?)?;
                let (button, double) = match rest.as_deref() {
                    None | Some("left") => (LEFT_BUTTON, false),
                    Some("right") => (RIGHT_BUTTON, false),
                    Some("double") => (LEFT_BUTTON, true),
                    Some(other) => {
                        bail!("`{other}` is not a click; the clicks are left, right and double")
                    }
                };
                me.steps.push(Step::Click {
                    column,
                    line,
                    button,
                    double,
                });
            }
            "--drag" => {
                let said = value()?;
                let Some((from, to)) = said.split_once(':') else {
                    bail!("`--drag` wants `column,line:column,line`, not `{said}`");
                };
                let (fc, fl, rest) = cell_of("--drag", from)?;
                ensure!(rest.is_none(), "`--drag` wants `column,line:column,line`");
                let (tc, tl, rest) = cell_of("--drag", to)?;
                ensure!(rest.is_none(), "`--drag` wants `column,line:column,line`");
                me.steps.push(Step::Drag {
                    from: (fc, fl),
                    to: (tc, tl),
                });
            }
            "--hover-at" | "--click-at" => {
                let said = value()?;
                let at = said
                    .split_once(',')
                    .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
                    .with_context(|| format!("`{flag}` wants `x,y` in logical pixels"))?;
                me.steps.push(Step::Point {
                    at,
                    click: flag == "--click-at",
                });
            }
            "--open" => {
                let key = value()?;
                let page = crate::player::PageId::by_key(&key).with_context(|| {
                    format!(
                        "`--open {key}` names no page of the player: a page is named by its \
                         own key, its title (`player-page-…`)"
                    )
                })?;
                me.steps.push(Step::Open(page));
            }
            "--let-go" => me.steps.push(Step::LetGo),
            "--rest" => me.steps.push(Step::Rest),
            "--edit" => {
                let file = value()?;
                let words = std::fs::read_to_string(&file)
                    .with_context(|| format!("could not read `{file}`"))?;
                me.steps.push(Step::Text(words));
            }
            // A WORD OF A DOOR THAT RIDES THIS WALK, kept for it.
            other => match doors.words.iter().find(|(w, _)| *w == other) {
                Some(&(word, takes)) => {
                    let given = if takes { Some(value()?) } else { None };
                    me.riding.push((word, given));
                }
                None => bail!("unknown option `{other}`.\n\n{}{USAGE}", doors.usage),
            },
        }
    }
    // THE RIDERS' WORDS, on a door that reads none of them.
    (doors.read)(me.rider.map(|r| r.door), &me)?;
    // WHAT A DOOR CANNOT BE is refused, never quietly dropped: a photograph
    // of something nobody asked for is worse than none.
    match me.door {
        // THE FOOTER'S PANEL is no door of this camera: the footer is a layer
        // of the compositor's, and its photograph is the VM's (`.#shot`).
        Which::Footer => bail!("the footer is photographed in the VM (`nix run .#shot`)"),
        Which::Window => {
            ensure!(
                me.size.is_none(),
                "`--size` is a column's; a window's size is `--columns`/`--width`"
            );
            ensure!(
                me.screen.is_none(),
                "`--screen` is a column's: a window's screen is the size it is given"
            );
            ensure!(
                me.stage != Stage::Small || (me.width.is_none() && me.height.is_none()),
                "the corner's size is its own (`space.player.pip*`), and it takes no \
                 `--columns`/`--width`/`--lines`/`--height`"
            );
            ensure!(
                me.stage != Stage::Maximized || (me.width.is_some() && me.height.is_some()),
                "`--stage maximized` is the whole screen, and a screen has a size: give it \
                 with `--width` and `--height` (or `--columns` and `--lines`)"
            );
        }

        Which::Column => {
            ensure!(
                me.width.is_none(),
                "a column's width is its size's (`--size`), not `--columns`/`--width`"
            );
            ensure!(
                matches!(me.stage, Stage::Off | Stage::Only),
                "a column's whole screen and corner are the WINDOW's: photograph them with \
                 `--door window --stage maximized|small`"
            );
        }
    }
    Ok(me)
}

/// Sets a length the person gave, ONCE: the same length in cells and in
/// pixels is two answers to one question, and the last one given would
/// otherwise win without a word.
fn once(slot: &mut Option<Measure>, given: Measure, cells: &str, px: &str) -> Result<()> {
    ensure!(
        slot.replace(given).is_none(),
        "`{cells}` and `{px}` are one length, given twice: give one of them, once"
    );
    Ok(())
}

/// A door the player is in, as the photograph drives it — or one that rides
/// the walk with no player inside ([`Rider`]).
///
/// # Why every door is one, found by its type
///
/// Because a door that rides the walk is often another crate's (the
/// calculator's window, the update's): this trait is the player's crate's
/// and the door its own crate's type, so neither the binary nor the door's
/// crate may write the impl — and the door's crate standing on the player to
/// write it would recompile it with every line of the player. So the walk
/// asks every door the same two questions and finds the player's own doors,
/// the window and the column, by their type; what a rider would have asked
/// past itself is handed to [`shoot_saying`] by the rider.
pub trait Photographed: Content {
    /// The player inside — `None` for a door that is no player's (the
    /// footer's settings), whose walk takes keys and the pointer and no
    /// daemon's answer.
    fn player(&mut self) -> Option<&mut Player>;

    /// The size the compositor would configure it at now, from the one it
    /// was asked at: a column's width follows the size it is drawn at.
    fn configured(&self, asked: (u32, u32)) -> (u32, u32);
}

impl<C: Content + 'static> Photographed for C {
    fn player(&mut self) -> Option<&mut Player> {
        let me: &mut dyn std::any::Any = self;
        if me.is::<Column<Player>>() {
            return me
                .downcast_mut::<Column<Player>>()
                .map(|column| &mut column.tenant);
        }
        me.downcast_mut::<Player>()
    }

    fn configured(&self, asked: (u32, u32)) -> (u32, u32) {
        let me: &dyn std::any::Any = self;
        me.downcast_ref::<Column<Player>>()
            .map_or(asked, |column| (column.drawn_width(), asked.1))
    }
}

/// `ronoctua-shell photograph …`: draws the door the words ask for into a
/// PNG — or prints its frame — the doors that are not the player's as
/// `doors` says them.
///
/// # Errors
///
/// A word not understood, a file that is not what it says, no graphics
/// adapter, or a picture larger than the GPU takes.
pub fn run(words: &[String], doors: &OtherDoors) -> Result<()> {
    let asked = parse(words, doors)?;
    // THE THEME, THE PAGE AND THE SIZE OF THE TEXT, before anything resolves
    // a colour or a cell — the ones asked, never the person's files: a
    // photograph is deterministic. And the date's shape, which `main` read
    // from the person's file before it knew this was a photograph.
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(asked.page);
    tokens::text::set(asked.text);
    crate::when::prefs::set(crate::when::Choice::default());
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let mut outbox = Outbox::default();
    for (lane, file) in &asked.stills {
        outbox.stills.insert(*lane, still_of(file)?);
    }
    if let Some(file) = &asked.every {
        outbox.every = Some(still_of(file)?);
    }
    let scale = i32::from(asked.scale);
    let places = asked.places.clone();
    outbox.catalog.clone_from(&asked.catalog);
    outbox.ranking.clone_from(&asked.ranking);
    outbox.pam = asked.pam;
    // A DOOR THAT IS NO PLAYER'S, over the words it was given.
    if let Some(rider) = asked.rider {
        return (rider.shoot)(texts, scale, &asked);
    }
    (doors.drawing)(&asked)?;
    match asked.door {
        Which::Footer => bail!("the footer is photographed in the VM (`nix run .#shot`)"),
        Which::Window => {
            let ui = Ui::detached(texts, (1, 1), scale)?;
            let d = Detached::new(ui, Player::detached(Which::Window, outbox, places));
            shoot(d, &asked)
        }

        Which::Column => {
            let ui = Ui::detached(texts, (1, 1), scale)?;
            let size = asked.size.unwrap_or(Size::Medium);
            let player = Player::detached(Which::Column, outbox, places);
            let column = Column::detached(player, size, &ui, asked.screen)?;
            shoot(Detached::new(ui, column), &asked)
        }
    }
}

/// A still out of a PNG (or any picture the `image` crate reads).
fn still_of(file: &std::path::Path) -> Result<ronoctua_render::image::Rgba> {
    let picture = image::ImageReader::open(file)
        .with_context(|| format!("could not open `{}`", file.display()))?
        .with_guessed_format()
        .with_context(|| format!("could not read `{}`", file.display()))?
        .decode()
        .with_context(|| format!("`{}` is not a picture", file.display()))?
        .to_rgba8();
    let (w, h) = picture.dimensions();
    ronoctua_render::image::Rgba::new(w, h, picture.into_raw())
        .with_context(|| format!("`{}` has no pixels", file.display()))
}

/// The size the door is laid out at: what was asked, in cells or pixels, or
/// the door's own before a compositor says one.
fn size_of<C: Content>(content: &C, asked: &Asked) -> (u32, u32) {
    let px = tokens::text::size();
    let (cell, line) = (
        tokens::metrics::cell_width(px),
        tokens::metrics::line_height(px),
    );
    let (default_w, default_h) = content.default_size();
    // ROUNDED UP, as `render-png` rounds: a window a hair short of `n` lines
    // is laid out at `n − 1` by the door's own `floor`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = match asked.width {
        None => default_w,
        Some(Measure::Px(w)) => w,
        Some(Measure::Cells(n)) => (f32::from(n + 2 * content.margin()) * cell).ceil() as u32,
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = match asked.height {
        None => default_h,
        Some(Measure::Px(h)) => h,
        Some(Measure::Cells(n)) => (f32::from(n) * line).ceil() as u32,
    };
    (width, height)
}

/// Where the middle of a cell is, in logical pixels.
fn middle_of(column: u16, line: u16) -> (f64, f64) {
    let px = tokens::text::size();
    let (cell, height) = (
        f64::from(tokens::metrics::cell_width(px)),
        f64::from(tokens::metrics::line_height(px)),
    );
    (
        (f64::from(column) + 0.5) * cell,
        (f64::from(line) + 0.5) * height,
    )
}

/// The cell a step points at, refused when it is not on the surface: a
/// click past the edge would reach nothing, and say nothing about it.
fn on_the_surface(step: &Step, ui: &Ui) -> Result<()> {
    if let Step::Drag { from, to } = *step {
        for (column, line) in [from, to] {
            on_the_surface(&Step::Hover { column, line }, ui)?;
        }
        return Ok(());
    }
    if let Step::Point { at: (x, y), .. } = *step {
        ensure!(
            (0.0..f64::from(ui.width)).contains(&x) && (0.0..f64::from(ui.height)).contains(&y),
            "the point {x},{y} is outside the door, which is {}x{} px",
            ui.width,
            ui.height
        );
        return Ok(());
    }
    let (Step::Hover { column, line } | Step::Click { column, line, .. }) = *step else {
        return Ok(());
    };
    let (x, y) = middle_of(column, line);
    let px = tokens::text::size();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (across, down) = (
        (f64::from(ui.width) / f64::from(tokens::metrics::cell_width(px))) as u32,
        (f64::from(ui.height) / f64::from(tokens::metrics::line_height(px))) as u32,
    );
    ensure!(
        x < f64::from(ui.width) && y < f64::from(ui.height),
        "the cell {column},{line} is outside the door, which is {across} columns by {down} \
         lines at this step, margins included"
    );
    Ok(())
}

/// The pointer's frames for one step, as a compositor sends them — one
/// event a frame: it ENTERS the first time and moves after, then presses
/// and lets go for a click, twice for a double one.
fn frames_of(step: &Step, entered: &mut bool, ui: &Ui) -> Vec<Vec<PointerEvent>> {
    let (position, button, times) = match *step {
        Step::Hover { column, line } => (middle_of(column, line), None, 0),
        Step::Click {
            column,
            line,
            button,
            double,
        } => (
            middle_of(column, line),
            Some(button),
            if double { 2 } else { 1 },
        ),
        Step::Point { at, click } => (at, click.then_some(LEFT_BUTTON), u8::from(click)),
        Step::Drag { from, to } => {
            let event = |position, kind| {
                vec![PointerEvent {
                    surface: ui.surface.clone(),
                    position,
                    kind,
                }]
            };
            let (a, b) = (middle_of(from.0, from.1), middle_of(to.0, to.1));
            let arrive = if std::mem::replace(entered, true) {
                PointerEventKind::Motion { time: 0 }
            } else {
                PointerEventKind::Enter { serial: 0 }
            };
            return vec![
                event(a, arrive),
                event(
                    a,
                    PointerEventKind::Press {
                        time: 0,
                        button: LEFT_BUTTON,
                        serial: 0,
                    },
                ),
                event(b, PointerEventKind::Motion { time: 0 }),
            ];
        }
        Step::Key(_)
        | Step::LetGo
        | Step::Rest
        | Step::Open(_)
        | Step::Text(_)
        | Step::State(_)
        | Step::Queue(_)
        | Step::Sheet(_)
        | Step::Desk(_)
        | Step::Refused(_)
        | Step::Overrule(_) => {
            return Vec::new();
        }
    };
    let event = |kind| {
        vec![PointerEvent {
            surface: ui.surface.clone(),
            position,
            kind,
        }]
    };
    let mut out = vec![event(if std::mem::replace(entered, true) {
        PointerEventKind::Motion { time: 0 }
    } else {
        PointerEventKind::Enter { serial: 0 }
    })];
    if let Some(button) = button {
        for _ in 0..times {
            out.push(event(PointerEventKind::Press {
                time: 0,
                button,
                serial: 0,
            }));
            out.push(event(PointerEventKind::Release {
                time: 0,
                button,
                serial: 0,
            }));
        }
    }
    out
}

/// Lays the door out, hands it its first state and its stage, and walks it
/// — `frame` composing a frame after each step, as a door draws one after
/// the events that came together. The last frame is what is handed back.
///
/// # Errors
///
/// A cell outside the surface.
fn walk<C: Photographed, F>(
    d: &mut Detached<C>,
    asked: &Asked,
    mut frame: impl FnMut(&mut Detached<C>) -> F,
) -> Result<F> {
    // CONFIGURED, then the state, then the stage: a door is on the screen
    // before the daemon answers, and a window opened for a stage takes it on
    // its first turn.
    let mut configured = d.content.configured(size_of(&d.content, asked));
    d.lay_out_at(configured);
    // THE SCREENS IT KNOWS, as a compositor would have described them, the
    // first the one it is on.
    if !asked.outputs.is_empty() {
        let here = asked.outputs.first().map(|k| k.connector.clone());
        let ui = &mut d.ui;
        if let Some(p) = d.content.player() {
            p.given_screens(ui, asked.outputs.clone(), here);
        }
    }
    // THE PERSON'S KEYS, as given: a photograph reads none of this machine's.
    if let Some(p) = d.content.player() {
        p.keys_given(asked.kept.clone());
        // AND THE FAVOURITES AND THE PLAYED LATELY, the same way.
        p.picks_given(asked.favourites.clone(), asked.recent.clone());
        // AND THE WEBSITE PAGE'S ADDRESSES.
        p.websites_given(asked.websites.clone());
    }
    let mut steps = asked.steps.iter().peekable();
    while let Some(
        step @ (Step::State(_)
        | Step::Queue(_)
        | Step::Sheet(_)
        | Step::Desk(_)
        | Step::Refused(_)
        | Step::Overrule(_)),
    ) = steps.peek()
    {
        let ui = &mut d.ui;
        let Some(p) = d.content.player() else {
            bail!(
                "`{}` is an answer of the player, and this door is no player's",
                told(step)
            );
        };
        heard(p, ui, step);
        steps.next();
    }
    if asked.stage != Stage::Off {
        let ui = &mut d.ui;
        if let Some(p) = d.content.player() {
            p.stage_to(ui, asked.stage);
        }
    }
    // A WINDOW OPENED FOR THE LISTING'S STAGE on what a door handed over —
    // the fixture's person's source, everything of it — as `wired` opens one.
    if let Some(store) = asked.handed {
        ensure!(
            asked.catalog.is_some(),
            "`--handed` is a listing of the catalogue: give it one with `--catalog`"
        );
        let ui = &mut d.ui;
        let Some(p) = d.content.player() else {
            bail!("`--handed` opens the player's window, and this door is no player's");
        };
        // BY THE PAGE THAT HANDS ONE OVER, written above this crate
        // (`player_seam::Page::handed_fixture`).
        let mut handed = false;
        p.opened_on(ui, |v| {
            handed = crate::player_seam::installed()
                .iter()
                .any(|page| page.handed_fixture(v, store));
        });
        ensure!(
            handed,
            "`--handed` opens a listing of the catalogue, and no page of the player hands one over"
        );
    }
    d.turn();
    // THE FIRST FRAME, as a door draws one: what the pointer and the keys
    // find next is what it placed.
    let mut last = frame(d);

    let mut entered = false;
    // WHERE A DRAG LEFT THE BUTTON HELD, for `--let-go`.
    let mut held: Option<(f64, f64)> = None;
    for step in steps {
        // A DOOR THAT CLOSED takes no more steps: what came after would be
        // walked on a door nobody sees.
        ensure!(
            !d.ui.quit,
            "the door closed at the step before `{}`, and a step after it reaches nothing",
            told(step)
        );
        if let Some(p) = d.content.player() {
            p.wait(APART);
        }
        match step {
            Step::Key(key) => d.press(*key),
            Step::Hover { .. } | Step::Click { .. } | Step::Drag { .. } | Step::Point { .. } => {
                on_the_surface(step, &d.ui)?;
                for events in frames_of(step, &mut entered, &d.ui) {
                    d.pointer(&events);
                }
                // A HOVER WITH THE BUTTON HELD goes on dragging, and the
                // button is held where it went; a click lets it go.
                held = match *step {
                    Step::Drag { to, .. } => Some(middle_of(to.0, to.1)),
                    Step::Hover { column, line } => held.map(|_| middle_of(column, line)),
                    Step::Point { at, click: false } => held.map(|_| at),
                    _ => None,
                };
            }
            // THE BUTTON COMES UP where the drag left it — and nowhere
            // else: a walk that held nothing has nothing to let go.
            Step::LetGo => {
                let Some(position) = held.take() else {
                    bail!("`--let-go` with no `--drag` holding the button");
                };
                d.pointer(&[PointerEvent {
                    surface: d.ui.surface.clone(),
                    position,
                    kind: PointerEventKind::Release {
                        time: 0,
                        button: LEFT_BUTTON,
                        serial: 0,
                    },
                }]);
            }
            Step::Rest => {
                let ui = &mut d.ui;
                if let Some(p) = d.content.player() {
                    p.rest(ui);
                }
            }
            Step::Text(words) => {
                let ui = &mut d.ui;
                let Some(p) = d.content.player() else {
                    bail!("`--edit` opens a text in the player, and this door is no player's");
                };
                p.edit_text(ui, words);
            }
            Step::Open(page) => {
                let ui = &mut d.ui;
                let Some(p) = d.content.player() else {
                    bail!("`--open` opens a page of the player, and this door is no player's");
                };
                p.open_page(ui, *page);
            }
            Step::State(_)
            | Step::Queue(_)
            | Step::Sheet(_)
            | Step::Desk(_)
            | Step::Refused(_)
            | Step::Overrule(_) => {
                let ui = &mut d.ui;
                let Some(p) = d.content.player() else {
                    bail!(
                        "`{}` is an answer of the player, and this door is no player's",
                        told(step)
                    );
                };
                heard(p, ui, step);
            }
        }
        d.turn();
        // A DOOR THAT CLOSED DRAWS NOTHING MORE: the picture is the last
        // frame it drew, before the step that closed it.
        if d.ui.quit {
            continue;
        }
        // A COLUMN THAT CHANGED ITS SIZE is configured at the new width, as
        // the compositor configures it.
        let now = d.content.configured(configured);
        if now != configured {
            configured = now;
            d.lay_out_at(configured);
        }
        last = frame(d);
    }
    if d.ui.quit {
        return Ok(last);
    }
    Ok(frame(d))
}

/// What the player said, on a step: a state, or the queue's outline — down
/// the paths the watch thread's own take.
fn heard(player: &mut Player, ui: &mut Ui, step: &Step) {
    match step {
        Step::State(state) => player.state_arrived(ui, Some((**state).clone())),
        Step::Queue(outline) => player.outline_arrived(ui, Some(outline.clone())),
        Step::Sheet(sheet) => player.sheet_arrived(ui, (**sheet).clone()),
        Step::Desk(desk) => player.desk_arrived(ui, (**desk).clone()),
        Step::Refused(r) => player.listing_refused(ui, r.0.clone(), r.1.clone()),
        Step::Overrule(r) => player.overrule_asked(ui, r.0.clone(), r.1.clone()),
        Step::Key(_)
        | Step::Hover { .. }
        | Step::Click { .. }
        | Step::Point { .. }
        | Step::Drag { .. }
        | Step::LetGo
        | Step::Rest
        | Step::Open(_)
        | Step::Text(_) => {}
    }
}

/// A step as the command line asks it — what a refusal names.
fn told(step: &Step) -> String {
    match step {
        Step::Key(key) => format!("--key {}", key.notation()),
        Step::Hover { column, line } => format!("--hover {column},{line}"),
        Step::Click { column, line, .. } => format!("--click {column},{line}"),
        Step::Point { at, click } => format!(
            "--{}-at {},{}",
            if *click { "click" } else { "hover" },
            at.0,
            at.1
        ),
        Step::Drag { from, to } => format!("--drag {},{}:{},{}", from.0, from.1, to.0, to.1),
        Step::LetGo => "--let-go".to_owned(),
        Step::Rest => "--rest".to_owned(),
        Step::Open(page) => format!("--open {}", page.key()),
        Step::Text(_) => "--edit".to_owned(),
        Step::State(_) => "--state".to_owned(),
        Step::Queue(_) => "--queue".to_owned(),
        Step::Sheet(_) => "--sheet".to_owned(),
        Step::Desk(_) => "--desk".to_owned(),
        Step::Refused(_) => "--refused".to_owned(),
        Step::Overrule(_) => "--overrule".to_owned(),
    }
}

/// What the walk left undone that was asked — refused, never dropped: a
/// still no picture took, and a browser on this machine's own folders.
fn nothing_was_dropped<C: Photographed>(d: &mut Detached<C>, asked: &Asked) -> Result<()> {
    let Some(player) = d.content.player() else {
        return Ok(());
    };
    ensure!(
        asked.places.is_some() || !player.browsing(),
        "the walk opened the file browser, and its places are this machine's own folders — a \
         picture that differs on every machine; give it a tree with `--places <dir>`"
    );
    if let Some(out) = player.outbox() {
        let mut unused: Vec<String> = out
            .stills
            .keys()
            .filter(|k| !out.laid_out.contains(k))
            .map(ToString::to_string)
            .collect();
        if asked.every.is_some() && out.laid_out.is_empty() {
            unused.push("every".to_owned());
        }
        ensure!(
            unused.is_empty(),
            "`--frame {}` was given, and the door laid out no picture of that lane: the state \
             has no such picture lane, or it is not routed to the door's screen",
            unused.join(", ")
        );
    }
    Ok(())
}

/// Everything the walk would have done PAST THE DOOR, one line each: what
/// the daemon would have been asked — in its own spelling, the line a
/// socket carries — the preferences written, a pairing, a stage's window,
/// an order for the footer, a program started.
///
/// # Errors
///
/// An ask that does not serialise.
pub fn said_past_the_door<C: Photographed>(d: &mut Detached<C>) -> Result<Vec<String>> {
    said_past_the_door_with(d, |_| Vec::new())
}

/// [`said_past_the_door`], with what a door that is no player's would have
/// asked past itself (`past`, a line each, as the photograph prints it — the
/// update's window's run asked for) after what its `Ui` was asked: the
/// player's own is its outbox.
///
/// # Errors
///
/// An ask that does not serialise.
pub fn said_past_the_door_with<C: Photographed>(
    d: &mut Detached<C>,
    past: impl Fn(&C) -> Vec<String>,
) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for order in d.ui.orders() {
        lines.push(format!("would have told the footer: {order}"));
    }
    if d.ui.quit {
        lines.push("would have closed the door".to_owned());
    }
    for launch in d.ui.launches() {
        lines.push(format!("would have started: {launch:?}"));
    }
    lines.extend(past(&d.content));
    let Some(out) = d.content.player().and_then(|p| p.outbox()) else {
        return Ok(lines);
    };
    for ask in &out.asked {
        lines.push(format!("asked the player: {}", serde_json::to_string(ask)?));
    }
    for switch in &out.kept {
        lines.push(format!(
            "would have turned the player's preference: {switch:?}"
        ));
    }
    for to in &out.paired_with {
        lines.push(format!("would have paired with: {to}"));
    }
    for show in &out.stages {
        lines.push(format!("would have opened the stage's window: {show:?}"));
    }
    if out.ended_the_doors {
        lines.push("would have ended the player's column and other windows".to_owned());
    }
    if out.restarted {
        lines.push("would have started the player again".to_owned());
    }
    for screens in &out.every_screen {
        lines.push(format!(
            "would have opened a window on each screen: {}",
            screens.join(", ")
        ));
    }
    if out.sinks_held {
        lines.push("would have held the sound outputs while their rows were on screen".to_owned());
    }
    for (services, directories, on) in &out.lets {
        lines.push(format!(
            "would have {} the player ask: {}",
            if *on { "let" } else { "stopped letting" },
            services
                .iter()
                .map(|s| s.id())
                .chain(directories.iter().map(|d| d.id()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for key in &out.keys_kept {
        lines.push(format!("would have kept the key: {}", key.file()));
    }
    for key in &out.keys_forgotten {
        lines.push(format!("would have forgotten the key: {}", key.file()));
    }
    for change in &out.websites {
        lines.push(format!("would have kept the website address: {change:?}"));
    }
    for (hosts, on) in &out.hosts_let {
        lines.push(format!(
            "would have {} the hosts: {}",
            if *on { "let" } else { "stopped letting" },
            hosts.join(", ")
        ));
    }
    Ok(lines)
}

/// The door, driven as asked and drawn.
///
/// # Errors
///
/// A step the door cannot take, no graphics adapter, or a picture larger
/// than the GPU takes.
pub fn shoot<C: Photographed>(d: Detached<C>, asked: &Asked) -> Result<()> {
    shoot_saying(d, asked, |_| Vec::new())
}

/// [`shoot`], a door that is no player's saying what it would have asked
/// past itself (`past`, [`said_past_the_door_with`]).
///
/// # Errors
///
/// A step the door cannot take, no graphics adapter, or a picture larger
/// than the GPU takes.
pub fn shoot_saying<C: Photographed>(
    mut d: Detached<C>,
    asked: &Asked,
    past: impl Fn(&C) -> Vec<String>,
) -> Result<()> {
    // THE DEVICE FIRST, and ONE: a picture a frame uploads during the walk
    // is the one the last frame draws, and the target follows the size the
    // walk leaves the door at (`Offscreen::resize`).
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let format = ronoctua_render::offscreen::FORMAT;
    // IN THE LANGUAGE ASKED, as a door's renderer is born in its own: the
    // locale picks the Han faces.
    let mut text = ronoctua_render::Text::new(
        &gpu.device,
        &gpu.queue,
        format,
        d.ui.scale,
        tokens::text::size(),
        &asked.language,
    )?;
    let mut quads = ronoctua_render::quad::Quads::new(&gpu.device, format)?;
    let mut images = ronoctua_render::image::Images::new(&gpu.device, format)?;

    let composed = walk(&mut d, asked, |d| {
        d.compose(&gpu.device, &gpu.queue, &mut images)
    })?;
    nothing_was_dropped(&mut d, asked)?;
    let past = said_past_the_door_with(&mut d, past)?;

    if asked.list {
        print_the_list(&d.ui, &composed);
        if let Some(p) = d.content.player() {
            for (members, (column, line), (x, y)) in p.globe_marks_at() {
                println!("globe mark {members:?} at {column},{line} px {x:.1},{y:.1}");
            }
        }
        for line in &past {
            println!("{line}");
        }
        return Ok(());
    }

    // THE TEXTURE IS PHYSICAL, as a door's buffer is: the same picture with
    // more pixels in every glyph.
    let logical = (d.ui.width, d.ui.height);
    let by = u32::from(asked.scale);
    let physical = (logical.0.saturating_mul(by), logical.1.saturating_mul(by));
    gpu.resize(physical.0, physical.1);
    // A PICTURE THE GPU CANNOT HOLD IS REFUSED, not quietly cut.
    ensure!(
        (gpu.width, gpu.height) == physical,
        "the picture asked for is {}x{} px and this GPU takes at most {} on either side; ask \
         for a smaller door or a smaller --scale",
        physical.0,
        physical.1,
        gpu.ceiling,
    );
    // THE TEXT OVER THE PHYSICAL PIXELS, the rectangles and the pictures
    // over the LOGICAL ones — as `Door::present` hands them, and as
    // `render-png` does (the kit's `hidpi` carries the whole of why).
    text.prepare(
        &gpu.device,
        &gpu.queue,
        gpu.width,
        gpu.height,
        ronoctua_render::Atlas::Pinned,
        &composed.spans,
    )?;
    quads.prepare(
        &gpu.device,
        &gpu.queue,
        logical.0,
        logical.1,
        &composed.quads,
    );
    images.prepare(&gpu.queue, logical.0, logical.1, &composed.pictures);
    // AND THE DOOR'S CLEAR: the page's own ground, as `Door::present` clears.
    let (drawn, rects, pictures) = (&text, &quads, &images);
    gpu.frame(tokens::color::SURFACE_CHROME.to_linear(), |pass| {
        composed.draw(pass, drawn, rects, pictures)
    })?;
    let rgba = gpu.read_rgba()?;
    ronoctua_render::offscreen::write_png(&asked.out, &rgba, gpu.width, gpu.height)?;
    println!(
        "{}: {}x{} px, {} columns by {} lines, scale {}, {} runs, language {}, theme {} on the {} \
         page",
        asked.out.display(),
        gpu.width,
        gpu.height,
        d.ui.columns,
        d.ui.lines,
        asked.scale,
        composed.spans.len(),
        d.ui.texts.language(),
        asked.theme.id(),
        asked.page.id(),
    );
    // SAID, because a detached door does nothing past itself: what a click
    // did there is these lines, and not a daemon's answer.
    for line in &past {
        println!("{line}");
    }
    Ok(())
}

/// The frame, as lines a person or a check reads — what `render-png --list`
/// is for a buffer: a band nobody can see under a run, a run past the last
/// line, two runs in one cell.
fn print_the_list(ui: &Ui, composed: &Composed) {
    println!(
        "{}x{} px logical, {} columns by {} lines, scale {}",
        ui.width, ui.height, ui.columns, ui.lines, ui.scale
    );
    for s in &composed.spans {
        println!(
            "run line {} column {}{}{} {:?} {:?} {:?}",
            s.line,
            s.column,
            if s.edge { " from-the-right" } else { "" },
            if s.foot { " from-the-bottom" } else { "" },
            s.size,
            s.color,
            s.text
        );
    }
    for (i, q) in composed.quads.iter().enumerate() {
        let layer = if u32::try_from(i).unwrap_or(u32::MAX) < composed.split {
            "under"
        } else {
            "over"
        };
        println!(
            "rect {layer} {:.1},{:.1} {:.1}x{:.1} {:?}",
            q.x, q.y, q.width, q.height, q.color
        );
    }
    for (key, a) in &composed.pictures {
        println!(
            "picture {key} {:.1},{:.1} {:.1}x{:.1}",
            a.x, a.y, a.width, a.height
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    /// No camera of its own.
    fn no_camera(_: &str) -> Option<OwnCamera> {
        None
    }

    /// Nothing refused.
    fn nothing(_: Option<&'static str>, _: &Asked) -> Result<()> {
        Ok(())
    }

    /// Nothing refused before the door is drawn.
    fn nothing_drawn(_: &Asked) -> Result<()> {
        Ok(())
    }

    /// The player's own doors and no other.
    const ONLY: OtherDoors = OtherDoors {
        usage: "",
        named: "window and column",
        camera: no_camera,
        riders: &[],
        words: &[],
        read: nothing,
        drawing: nothing_drawn,
    };

    /// `parse`, over the player's own doors.
    fn parse(words: &[String]) -> Result<Asked> {
        super::parse(words, &ONLY)
    }

    /// FAILS IF: a door the binary hands in is not reached — a rider not
    /// drawn in the window's place, its words not kept in order with their
    /// values, the riders' check not asked once the words are read, a door
    /// that is none refused without the doors named, a word of none taken.
    #[test]
    fn the_doors_handed_in_are_reached() {
        fn shot(_: ronoctua_i18n::Texts, _: i32, _: &Asked) -> Result<()> {
            Ok(())
        }
        fn notes_ride(door: Option<&'static str>, asked: &Asked) -> Result<()> {
            ensure!(
                door == Some("rides") || !asked.given("--note"),
                "notes are the rider's"
            );
            Ok(())
        }
        const RIDES: &[Rider] = &[Rider {
            door: "rides",
            shoot: shot,
        }];
        let doors = OtherDoors {
            usage: "",
            named: "window, column and rides",
            camera: no_camera,
            riders: RIDES,
            words: &[("--note", true), ("--flag", false)],
            read: notes_ride,
            drawing: nothing_drawn,
        };
        let a = super::parse(
            &words(&["--note", "a", "--door", "rides", "--flag", "--note", "b"]),
            &doors,
        )
        .expect("understood");
        assert_eq!(
            (a.door(), a.rider().map(|r| r.door)),
            (Which::Window, Some("rides"))
        );
        assert_eq!(a.values_of("--note"), ["a", "b"]);
        assert_eq!(a.value_of("--note"), Some("b"));
        assert!(a.given("--flag") && !a.given("--other"));
        let refused = super::parse(&words(&["--door", "window", "--note", "a"]), &doors)
            .expect_err("a rider's word on the window");
        assert!(
            refused.to_string().contains("notes are the rider's"),
            "{refused}"
        );
        let refused = super::parse(&words(&["--door", "nowhere"]), &doors).expect_err("no door");
        assert!(
            refused.to_string().contains("window, column and rides"),
            "{refused}"
        );
        assert!(super::parse(&words(&["--other"]), &doors).is_err());
    }

    /// FAILS IF: the fixture stops being the daemon's own answer to `look`
    /// — a `State` read in a spelling of this tool's — or an answer that is
    /// not the state is taken for an empty one.
    #[test]
    fn a_state_fixture_is_the_daemons_answer() {
        let state = State {
            playing: true,
            position: 12.5,
            duration: Some(90.0),
            volume: 70,
            title: Some("A film".into()),
            ..State::default()
        };
        let said = serde_json::to_string(&Say::State(Box::new(state.clone()))).expect("says");
        assert_eq!(state_of(&said, "look.json").expect("a state"), state);
        let done = serde_json::to_string(&Say::Done).expect("says");
        let refused = state_of(&done, "done.json").expect_err("not a state");
        assert!(refused.to_string().contains("not its state"), "{refused}");
        assert!(
            state_of("{\"volume\": 3}", "bare.json").is_err(),
            "a bare state"
        );
    }

    /// FAILS IF: the walk stops being kept in the order it was asked — a
    /// state after a click is the daemon's answer to it, and must not be
    /// taken first — a click's button is read wrong, or what a door cannot
    /// be is drawn anyway: a window at a column's size, a column at a
    /// window's width, a column on a stage that is a window of its own, one
    /// length given twice, a corner at a size, a whole screen at none.
    #[test]
    fn the_words_are_the_walk_and_what_no_door_is_is_refused() {
        let look = std::env::temp_dir().join(format!("ronoctua-look-{}.json", std::process::id()));
        let state = State {
            playing: true,
            ..State::default()
        };
        std::fs::write(
            &look,
            serde_json::to_string(&Say::State(Box::new(state.clone()))).expect("says"),
        )
        .expect("a fixture");
        let look_arg = look.to_string_lossy().into_owned();
        let a = parse(&words(&[
            "--door",
            "column",
            "--size",
            "bar",
            "--key",
            "SPC",
            "--hover",
            "2,3",
            "--click",
            "2,4,right",
            "--state",
            &look_arg,
            "--click",
            "5,6,double",
            "--rest",
            "--lines",
            "30",
            "--screen",
            "1280",
        ]))
        .expect("understood");
        std::fs::remove_file(&look).expect("the fixture goes");
        assert_eq!(
            (a.door, a.size, a.screen),
            (Which::Column, Some(Size::Bar), Some(1280))
        );
        assert_eq!(a.height, Some(Measure::Cells(30)));
        assert_eq!(
            a.steps,
            [
                Step::Key(ronoctua_keymap::Key::parse("SPC").expect("a key")),
                Step::Hover { column: 2, line: 3 },
                Step::Click {
                    column: 2,
                    line: 4,
                    button: RIGHT_BUTTON,
                    double: false,
                },
                Step::State(Box::new(state)),
                Step::Click {
                    column: 5,
                    line: 6,
                    button: LEFT_BUTTON,
                    double: true,
                },
                Step::Rest,
            ]
        );
        for wrong in [
            &["--size", "small"][..],
            &["--screen", "1280"],
            &["--door", "column", "--columns", "40"],
            &["--door", "column", "--stage", "maximized"],
            &["--columns", "40", "--width", "400"],
            &["--lines", "40", "--height", "400"],
            &["--width", "400", "--width", "500"],
            &["--stage", "small", "--lines", "20"],
            &["--stage", "maximized"],
            &["--stage", "maximized", "--width", "1280"],
            &["--click", "3"],
            &["--click", "3,4,middle"],
            &["--key", "not a key at all"],
            &["--stage", "whole"],
            &["--frame", "still.png"],
            &["--frame", "main=a.png"],
            &["--frame", "0=a.png"],
            &["--frame", "2=a.png", "--frame", "2=b.png"],
            &["--frame", "every=a.png", "--frame", "every=b.png"],
            &["--output", ""],
            &["--output", "eDP-1", "--output", "eDP-1=Built in"],
            &["--places", "/this/folder/is/not/there"],
            &["--scale", "0"],
            &["--page", "black"],
        ] {
            assert!(parse(&words(wrong)).is_err(), "{wrong:?} was taken");
        }
        assert!(
            parse(&words(&[
                "--stage",
                "maximized",
                "--columns",
                "120",
                "--lines",
                "40"
            ]))
            .is_ok()
        );
    }

    /// FAILS IF: a point of the surface — what a globe's mark smaller than a
    /// cell is clicked at — is not a press and a release exactly there, or a
    /// point off the surface is walked instead of refused.
    #[test]
    fn a_point_is_pressed_where_it_says() {
        let asked =
            parse(&words(&["--hover-at", "10,20", "--click-at", "12.5,40.25"])).expect("parses");
        assert_eq!(
            asked.steps,
            vec![
                Step::Point {
                    at: (10.0, 20.0),
                    click: false
                },
                Step::Point {
                    at: (12.5, 40.25),
                    click: true
                },
            ]
        );
        assert!(parse(&words(&["--click-at", "12"])).is_err());
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::for_tests(texts);
        let mut entered = false;
        let frames = frames_of(&asked.steps[1], &mut entered, &ui);
        let kinds: Vec<_> = frames
            .iter()
            .flatten()
            .map(|e| (e.position, std::mem::discriminant(&e.kind)))
            .collect();
        assert_eq!(kinds.len(), 3, "enter, press, release");
        assert!(kinds.iter().all(|(p, _)| *p == (12.5, 40.25)));
        let far = Step::Point {
            at: (f64::from(ui.width) + 1.0, 0.0),
            click: true,
        };
        assert!(on_the_surface(&far, &ui).is_err());
    }

    /// FAILS IF: a click stops landing on the cell it names — the middle of
    /// `column,line`, counted from the surface's corner, read back by the
    /// door's own arithmetic as another cell — or a double click stops
    /// being two presses in frames of their own, as a compositor sends them.
    #[test]
    fn a_click_lands_on_the_cell_it_names() {
        for (column, line) in [(0, 0), (1, 2), (4, 14), (45, 0), (79, 39)] {
            let (x, y) = middle_of(column, line);
            assert_eq!(
                crate::player_window::column_at(x),
                column,
                "{column},{line}"
            );
            assert_eq!(crate::player_window::line_at(y), line, "{column},{line}");
        }
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::detached(texts, (400, 400), 1).expect("a detached ui");
        let mut entered = false;
        let double = Step::Click {
            column: 1,
            line: 1,
            button: LEFT_BUTTON,
            double: true,
        };
        let kinds: Vec<&str> = frames_of(&double, &mut entered, &ui)
            .iter()
            .map(|frame| {
                assert_eq!(frame.len(), 1, "one event a frame");
                match frame[0].kind {
                    PointerEventKind::Enter { .. } => "enter",
                    PointerEventKind::Motion { .. } => "motion",
                    PointerEventKind::Press { .. } => "press",
                    PointerEventKind::Release { .. } => "release",
                    _ => "other",
                }
            })
            .collect();
        assert_eq!(kinds, ["enter", "press", "release", "press", "release"]);
        let again = frames_of(&Step::Hover { column: 2, line: 2 }, &mut entered, &ui);
        assert!(matches!(again[0][0].kind, PointerEventKind::Motion { .. }));
        // A DRAG: arrives, presses, and moves with the button held — and
        // lets go only on its own step.
        let drag = Step::Drag {
            from: (3, 4),
            to: (9, 4),
        };
        let frames = frames_of(&drag, &mut entered, &ui);
        let kinds: Vec<&str> = frames
            .iter()
            .map(|frame| match frame[0].kind {
                PointerEventKind::Motion { .. } => "motion",
                PointerEventKind::Press { .. } => "press",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["motion", "press", "motion"]);
        assert_eq!(frames[2][0].position, middle_of(9, 4));
        assert!(frames_of(&Step::LetGo, &mut entered, &ui).is_empty());
        assert_eq!(
            parse(&words(&["--drag", "3,4:9,4", "--let-go"]))
                .expect("understood")
                .steps,
            [drag, Step::LetGo]
        );
        assert!(parse(&words(&["--drag", "3,4"])).is_err());
        assert!(
            on_the_surface(
                &Step::Drag {
                    from: (1, 1),
                    to: (900, 1)
                },
                &ui
            )
            .is_err()
        );
        // AND ONE PAST THE EDGE is refused, not sent to nothing.
        assert!(on_the_surface(&Step::Hover { column: 1, line: 1 }, &ui).is_ok());
        assert!(
            on_the_surface(
                &Step::Hover {
                    column: 900,
                    line: 1
                },
                &ui
            )
            .is_err()
        );
        assert!(
            on_the_surface(
                &Step::Hover {
                    column: 1,
                    line: 900
                },
                &ui
            )
            .is_err()
        );
    }

    /// FAILS IF: a column that changed its size in the walk is photographed
    /// at the width it had — the compositor configures it at the new one —
    /// or a column on a screen stops being clamped by it.
    #[test]
    fn the_walk_lays_a_column_out_again_at_its_new_size() {
        let asked = parse(&words(&[
            "--door",
            "column",
            "--size",
            "medium",
            "--lines",
            "30",
            "--key",
            "C-<right>",
        ]))
        .expect("understood");
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::detached(texts, (1, 1), 1).expect("a detached ui");
        let player = Player::detached(Which::Column, Outbox::default(), None);
        let column = Column::detached(player, Size::Medium, &ui, None).expect("a column");
        let mut d = Detached::new(ui, column);
        let mut widths = Vec::new();
        walk(&mut d, &asked, |d| widths.push(d.ui.width)).expect("walked");
        let (medium, small) = (
            crate::column::width_px(Size::Medium, None),
            crate::column::width_px(Size::Small, None),
        );
        assert_ne!(medium, small);
        assert_eq!(widths.first(), Some(&medium), "{widths:?}");
        assert_eq!(widths.last(), Some(&small), "{widths:?}");
        assert_eq!(d.content.drawn(), Size::Small);
    }

    /// FAILS IF: a door that closes goes on being walked — a step after it
    /// taken instead of refused — its picture is a frame drawn after it
    /// closed, or it stops saying it would have closed.
    #[test]
    fn a_door_that_closes_ends_the_walk() {
        let column = |ui: &Ui| {
            let player = Player::detached(Which::Column, Outbox::default(), None);
            Column::detached(player, Size::Medium, ui, None).expect("a column")
        };
        let texts = || ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        // CLOSED BY ITS KEY: the last frame is the one before the key.
        let asked = parse(&words(&[
            "--door", "column", "--hover", "20,5", "--key", "C-q",
        ]))
        .expect("understood");
        let ui = Ui::detached(texts(), (1, 1), 1).expect("a detached ui");
        let c = column(&ui);
        let mut d = Detached::new(ui, c);
        let mut frames = 0;
        let last = walk(&mut d, &asked, |_| {
            frames += 1;
            frames
        })
        .expect("walked");
        assert!(d.ui.quit, "C-q did not close the column");
        assert_eq!(
            (frames, last),
            (2, 2),
            "a frame drawn after the door closed"
        );
        assert!(
            said_past_the_door(&mut d)
                .expect("said")
                .contains(&"would have closed the door".to_owned())
        );
        // AND A STEP AFTER IT IS REFUSED, by its name on the command line.
        let asked = parse(&words(&[
            "--door", "column", "--key", "C-q", "--key", "SPC",
        ]))
        .expect("understood");
        let ui = Ui::detached(texts(), (1, 1), 1).expect("a detached ui");
        let c = column(&ui);
        let mut d = Detached::new(ui, c);
        let refused = walk(&mut d, &asked, |_| ()).expect_err("walked past the close");
        assert!(refused.to_string().contains("--key SPC"), "{refused}");
    }

    /// FAILS IF: a detached player reaches anything: a key that plays or
    /// pauses is SENT (to the person's own daemon), or a column's stage opens
    /// a window of its own — instead of the ask being kept and printed, in
    /// the daemon's own spelling.
    #[test]
    fn a_detached_player_keeps_what_it_would_have_sent() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::detached(texts, (760, 720), 1).expect("a detached ui");
        let mut d = Detached::new(ui, Player::detached(Which::Window, Outbox::default(), None));
        let ui = &mut d.ui;
        d.content.state_arrived(
            ui,
            Some(State {
                playing: true,
                ..State::default()
            }),
        );
        d.press(ronoctua_keymap::Key::parse("SPC").expect("a key"));
        d.press(ronoctua_keymap::Key::parse("n").expect("a key"));
        assert_eq!(
            d.content.asked(),
            [
                ronoctua_player::wire::Ask::Toggle,
                ronoctua_player::wire::Ask::Next
            ]
        );
        assert_eq!(d.ui.message, None, "a detached ask is said as a taken one");
        assert_eq!(
            said_past_the_door(&mut d).expect("said"),
            [
                "asked the player: {\"ask\":\"toggle\"}",
                "asked the player: {\"ask\":\"next\"}"
            ]
        );

        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::detached(texts, (400, 720), 1).expect("a detached ui");
        let column = Column::detached(
            Player::detached(Which::Column, Outbox::default(), None),
            Size::Medium,
            &ui,
            None,
        )
        .expect("a detached column");
        let mut d = Detached::new(ui, column);
        let ui = &mut d.ui;
        // A COLUMN'S WHOLE SCREEN is the window's, a process of its own: a
        // detached column starts none, and says it would have.
        d.content.tenant.stage_to(ui, Stage::Maximized);
        d.press(ronoctua_keymap::Key::parse("m").expect("a key"));
        assert!(d.content.tenant.asked().is_empty());
        assert!(d.ui.orders().is_empty() && d.ui.launches().is_empty());
        assert_eq!(
            said_past_the_door(&mut d).expect("said"),
            [
                "would have opened the stage's window: Maximized",
                "would have opened the stage's window: Maximized"
            ]
        );
    }
}
