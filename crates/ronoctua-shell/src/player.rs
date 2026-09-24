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
//! # What is typed, is typed in the footer
//!
//! A file or an address to play, one to add to the queue and a video to
//! follow are asked in the minibuffer (`ask-player-play-file`,
//! `ask-player-enqueue`, `ask-player-follow`); a
//! window runs the command and says so, because letters typed there would go
//! nowhere (the network view's rule).

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
    ("n", "player-next", Where::Anywhere),
    ("p", "player-previous", Where::Anywhere),
    ("d", "player-remove", Where::OnRemovable),
    ("f", "player-browse", Where::OutOfTheFiles),
    ("a", "player-add", Where::OnAFile),
    ("DEL", "player-parent", Where::InTheFiles),
    ("o", "player-only", Where::Anywhere),
    ("m", "player-maximize", Where::Anywhere),
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
    /// Only the picture, in this door.
    Only,
    /// The picture on the whole screen.
    Maximize,
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
    /// A door or a question: a command of the registry.
    Run(&'static str),
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
    /// A file of the listing: `RET` plays it alone, `a` queues it.
    File(std::path::PathBuf),
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
}

/// What the door has to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Draw again.
    Nothing,
    /// Shut the view.
    Close,
    /// Run a command of the registry (a question in the footer, or a door).
    Run(&'static str),
    /// Tell the player.
    Tell(Ask),
    /// Write the preferences, and tell the player to reread them.
    Keep(ronoctua_player::prefs::Preferences),
    /// Say a sentence, by its key.
    Say(&'static str),
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
}

/// The files level: which place, and the folder that is open.
#[derive(Debug, Clone)]
struct Files {
    place: usize,
    listing: crate::player_browse::Listing,
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
        }
    }

    /// The files, where they were left, or the music folder the first time.
    fn open_the_files(&mut self) {
        let (place, at) = self.left_at.clone().unwrap_or_else(|| {
            let places = crate::player_browse::places();
            (0, places[0].1.clone())
        });
        self.files = Some(Files {
            place,
            listing: crate::player_browse::list(&at),
        });
        self.cursor = 0;
        self.moved = false;
    }

    /// Another folder, the cursor on `on` when it is one of its rows.
    fn go_to(&mut self, at: &std::path::Path, on: Option<&std::path::Path>) {
        if let Some(f) = self.files.as_mut() {
            f.listing = crate::player_browse::list(at);
            self.left_at = Some((f.place, at.to_path_buf()));
        }
        self.cursor = 0;
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
        let here: Vec<(&str, &str)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&target))
            .map(|(k, id, _)| (*k, *id))
            .collect();
        crate::chrome::hint_of(&here, texts)
    }

    /// A click; `true` when it was the second on the same row.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(|r| !r.heading) {
            let again = self.cursor == n;
            self.moved = true;
            self.cursor = n;
            return again;
        }
        false
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
            let here = rows
                .iter()
                .position(|r| match (&r.target, came.as_deref()) {
                    (Target::Folder(p), Some(c)) => p == c,
                    (Target::Folder(_) | Target::File(_), None) => true,
                    _ => false,
                });
            if let Some(i) = here {
                self.cursor = i;
                return;
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
                return;
            }
        }
    }

    /// A key.
    pub fn key(
        &mut self,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.settle_in(state, columns, texts);
        let rows = self.rows(state, columns, texts);
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
                    self.open_the_files();
                    self.settle_in(state, columns, texts);
                }
                return Errand::Nothing;
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
            Press::Up => {
                self.walk(&rows, false);
                return Errand::Nothing;
            }
            Press::Down => {
                self.walk(&rows, true);
                return Errand::Nothing;
            }
            Press::Toggle => return Errand::Tell(Ask::Toggle),
            Press::Only => return Errand::Show(Show::Only),
            Press::Maximize => return Errand::Show(Show::Maximized),
            Press::Next => return Errand::Tell(Ask::Next),
            Press::Previous => return Errand::Tell(Ask::Previous),
            Press::Less | Press::More | Press::Enter | Press::Remove | Press::Add => {}
        }
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        // A NEW FOLDER PUTS THE CURSOR BACK ON ZERO, which is its heading;
        // settling it here is what lands it on an entry. The doors never
        // settle after a key, and a cursor on a heading teaches no key of
        // the level — the photograph showed exactly that.
        if target.among_the_files() {
            let errand = self.among_the_files(&target, press);
            self.settle_in(state, columns, texts);
            return errand;
        }
        if target == Target::Browse && press == Press::Enter {
            self.open_the_files();
            self.settle_in(state, columns, texts);
            return Errand::Nothing;
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
            _ => {}
        }
        turn(&target, press, state)
    }

    /// A press on a row of the files.
    fn among_the_files(&mut self, target: &Target, press: Press) -> Errand {
        let Some(open) = self.files.as_ref().map(|f| f.listing.at.clone()) else {
            return Errand::Nothing;
        };
        // A FOLDER BECOMES ITS FILES here, walked down, and one with none is
        // a sentence rather than an empty order.
        let order = |folder: &std::path::Path, replace: bool| {
            let items: Vec<String> = crate::player_browse::gather(folder)
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            if items.is_empty() {
                Errand::Say(ronoctua_i18n::key::PLAYER_BROWSE_NOTHING_TO_PLAY)
            } else {
                Errand::Tell(Ask::Open { items, replace })
            }
        };
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
            (Target::PlayFolder, Press::Enter) => order(&open, true),
            (Target::PlayFolder | Target::EnqueueFolder, Press::Add)
            | (Target::EnqueueFolder, Press::Enter) => order(&open, false),
            (Target::Folder(p), Press::Enter) => {
                self.go_to(&p.clone(), None);
                Errand::Nothing
            }
            (Target::Folder(p), Press::Add) => order(p, false),
            // A FILE ALONE: `RET` plays it and only it, which is what the
            // question `Play a file…` has always done with a path.
            (Target::File(p), Press::Enter) => Errand::Tell(Ask::Open {
                items: vec![p.to_string_lossy().into_owned()],
                replace: true,
            }),
            (Target::File(p), Press::Add) => Errand::Tell(Ask::Open {
                items: vec![p.to_string_lossy().into_owned()],
                replace: false,
            }),
            _ => Errand::Nothing,
        }
    }

    /// The rows of the files level.
    fn files_rows(&self, files: &Files, b: &mut Builder<'_>) {
        use ronoctua_i18n::key;
        let places = crate::player_browse::places();
        let l = &files.listing;
        b.heading(key::PLAYER_HEADING_BROWSE);
        b.pair(
            b.t(key::PLAYER_BROWSE_PLACE),
            b.t(places[files.place.min(places.len() - 1)].0),
            Target::Place,
            "browse-place",
        );
        b.pair(
            b.t(key::PLAYER_BROWSE_FOLDER),
            shown_path(&l.at),
            Target::Nothing,
            "browse-folder",
        );
        if let Some(up) = l.at.parent() {
            b.pair(
                b.t(key::PLAYER_BROWSE_UP),
                shown_path(up),
                Target::Up,
                "browse-up",
            );
        }
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
            return;
        }
        if l.folders.is_empty() && l.files.is_empty() {
            b.note(key::PLAYER_BROWSE_EMPTY);
            return;
        }
        for f in &l.folders {
            b.entry(
                tokens::icon::FOLDER,
                file_name(f),
                Target::Folder(f.clone()),
                &format!("browse-{}", f.display()),
            );
        }
        for (f, glyph) in l.files.iter().zip(&l.glyphs) {
            b.entry(
                *glyph,
                file_name(f),
                Target::File(f.clone()),
                &format!("browse-{}", f.display()),
            );
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
            texts,
        };
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
            b.pair(
                b.t(key::PLAYER_POSITION),
                s.duration.map_or_else(
                    || clock(s.position),
                    |d| format!("{} / {}", clock(s.position), clock(d)),
                ),
                Target::Position,
                "position",
            );
        }
        b.pair(
            b.t(key::PLAYER_VOLUME),
            if s.muted {
                b.t(key::PLAYER_MUTED)
            } else {
                format!("{}%", s.volume)
            },
            Target::Volume,
            "volume",
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
        // THE QUEUE: every item, the current one in the accent.
        b.heading(key::PLAYER_HEADING_QUEUE);
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
        b.pair(
            b.t(key::PLAYER_FOLLOW),
            String::new(),
            Target::Run("player-follow"),
            "follow",
        );
        machines(&mut b, s);
        opening(&mut b);
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
    b.pair(
        b.t(key::PLAYER_PAIR),
        String::new(),
        Target::Run("player-pair"),
        "pair",
    );
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
        b.t(key::PLAYER_OPEN_FILE),
        String::new(),
        Target::Run("player-play-file"),
        "open-file",
    );
    b.pair(
        b.t(key::PLAYER_ENQUEUE),
        String::new(),
        Target::Run("player-enqueue"),
        "enqueue",
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
        Errand::Run(_) | Errand::Close | Errand::Show(_) => Err(errand),
        Errand::Say(key) => Ok(Some(texts.t(key))),
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

/// Tells the player one thing, starting it when nobody answers; what it
/// refused, in a sentence.
#[must_use]
pub fn tell(ask: &Ask, texts: &ronoctua_i18n::Texts) -> Option<String> {
    if ronoctua_player::client::ensure().is_err() {
        return Some(texts.t(ronoctua_i18n::key::PLAYER_NOT_RUNNING));
    }
    match ronoctua_player::client::ask(ask) {
        Ok(ronoctua_player::wire::Say::Refused(t)) => Some(trouble_sentence(&t, texts)),
        Ok(_) => None,
        Err(_) => Some(texts.t(ronoctua_i18n::key::PLAYER_NOT_RUNNING)),
    }
}

/// What was typed for one of the player's questions, turned into an order.
///
/// A path that exists is made absolute against home rather than against the
/// footer's working directory, which is nobody's; anything else is handed to
/// the player as typed — an address, which mpv and yt-dlp understand.
#[must_use]
pub fn typed(id: &str, said: &str) -> Option<Ask> {
    let said = said.trim();
    if said.is_empty() {
        return None;
    }
    if id == "player-pair" {
        // THE NAME AND THE CODE, in that order: the name is what is on the
        // other screen's heading, and the code is under it.
        let (to, code) = said.rsplit_once(char::is_whitespace)?;
        return Some(Ask::PairWith {
            to: to.trim().to_owned(),
            code: code.trim().to_owned(),
        });
    }
    let item = expand(said);
    if id == "player-follow" {
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
        replace: id != "player-enqueue",
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
        "player-next" => Press::Next,
        "player-previous" => Press::Previous,
        "player-remove" => Press::Remove,
        "player-browse" => Press::Browse,
        "player-add" => Press::Add,
        "player-parent" => Press::Parent,
        "player-only" => Press::Only,
        "player-maximize" => Press::Maximize,
        _ => return None,
    })
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
        let (label_colour, value_colour) = match (focused, acts) {
            (true, true) => (tokens::color::STATE_ACCENT, tokens::color::TEXT_HERE_ACTS),
            (true, false) => (tokens::color::STATE_ACCENT, tokens::color::TEXT_HERE_READ),
            (false, true) => (tokens::color::TEXT_LABEL_ACTS, tokens::color::TEXT_ANSWER),
            (false, false) => (tokens::color::TEXT_SECONDARY, tokens::color::TEXT_DIM),
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                &label,
                &value,
                label_colour,
                value_colour,
            ),
            focused,
            heading: false,
            target,
            spot: spot.to_owned(),
        });
    }

    /// An entry of the files: its glyph, and its name as the strong half.
    fn entry(&mut self, glyph: char, name: String, target: Target, spot: &str) {
        let focused = self.cursor == self.out.len();
        let colour = if focused {
            tokens::color::TEXT_HERE_ACTS
        } else {
            tokens::color::TEXT_PRIMARY
        };
        let glyph_cells = 2;
        let name = ronoctua_render::layout::elide_end(&name, self.room.saturating_sub(glyph_cells));
        self.out.push(Row {
            fields: vec![
                Self::piece(self.left, glyph.to_string(), tokens::color::TEXT_DIM),
                Self::piece(self.left + glyph_cells, name, colour),
            ],
            focused,
            heading: false,
            target,
            spot: spot.to_owned(),
        });
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
    /// into a folder, back up onto it, a file played alone or queued, and a
    /// folder queued whole.
    #[test]
    fn the_files_are_walked_played_and_queued() {
        let t = texts();
        let root = a_library("walked");
        let album = root.join("Album");
        let mut v = among(&root);
        // THE CURSOR LANDS ON THE FIRST ENTRY, the folder.
        assert_eq!(on(&v), Target::Folder(album.clone()));
        // `a` QUEUES IT WHOLE, pictures left out.
        let Errand::Tell(Ask::Open { items, replace }) = v.key(Press::Add, None, 80, &t) else {
            panic!("a folder was not queued");
        };
        assert!(!replace);
        assert_eq!(items.len(), 2, "{items:?}");
        // `RET` ENTERS IT, and the cursor is on its first file.
        assert_eq!(v.key(Press::Enter, None, 80, &t), Errand::Nothing);
        assert_eq!(on(&v), Target::File(album.join("01.ogg")));
        // `RET` ON A FILE PLAYS IT ALONE; `a` PUTS IT IN THE QUEUE.
        assert_eq!(
            v.key(Press::Enter, None, 80, &t),
            Errand::Tell(Ask::Open {
                items: vec![album.join("01.ogg").to_string_lossy().into_owned()],
                replace: true,
            })
        );
        assert_eq!(
            v.key(Press::Add, None, 80, &t),
            Errand::Tell(Ask::Open {
                items: vec![album.join("01.ogg").to_string_lossy().into_owned()],
                replace: false,
            })
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
            Errand::Say(ronoctua_i18n::key::PLAYER_BROWSE_NOTHING_TO_PLAY)
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
        assert_eq!(
            typed("player-pair", " den  123456 "),
            Some(Ask::PairWith {
                to: "den".into(),
                code: "123456".into()
            })
        );
        assert_eq!(typed("player-pair", "123456"), None, "a name and a code");
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
