//! THE NETWORK PROTOCOLS — `ronoctua-shell manual protocols`, which their
//! row of `LEADER h` opens: how the internet works, protocol by protocol
//! (`ronoctua_packets`).
//!
//! # Two levels
//!
//! The window opens on the LIST — every protocol under its group, narrowed by
//! what is typed (its name, what it does, its group, its layer), `RET` or the
//! second click opening the one under the cursor, as every list of the system
//! does. A PROTOCOL open is a small network (`packets_stage`), a panel that
//! says what the protocol is and what each step does (`packets_panel`), and
//! the capture of every packet it sends under them, with the transport that
//! plays it (`packets_table`). The network can be read three ways: the
//! devices with the packets flying between them, the same packets as a
//! sequence diagram (`packets_sequence`), and the packet of the moment opened
//! bit by bit (`packets_anatomy`). `Esc` goes back to the list, and from the
//! list closes the window.
//!
//! # A clock only while it plays
//!
//! Where everything is at a moment is `ronoctua_packets::timeline`'s answer, a
//! pure function of the time; the window keeps only that time. While it
//! plays, each frame asks for the next at the screen's pace (`Ui::pace`) and
//! moves the time by what passed, times the speed; paused, nothing is armed,
//! and a hidden window gets no frame. A photograph poses any moment
//! (`camera`): `--scene http@3:900:anatomy` is HTTP, its third step, 900 ms
//! in, its packet opened.
//!
//! # Words from the book
//!
//! What a protocol says is the book `packets` (`ronoctua_i18n::Book`), opened
//! here and nowhere else — the frame around it, the window's keys, are this
//! crate's texts.

use crate::door::{Canvas, Content, Ui};
use crate::fields::Field;
use anyhow::Result;
use ronoctua_i18n::Book;
use ronoctua_keymap::{Key, Sym};
use ronoctua_packets::timeline::{Pace, Pose, Timeline};
use ronoctua_packets::{Class, Protocol, catalogue};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};
use std::time::Instant;

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.protocols";

/// The keys of the LIST, as its header teaches them.
pub const OF_THE_LIST: &[(&str, &str)] = &[
    ("<down>", "protocols-down"),
    ("<up>", "protocols-up"),
    ("RET", "protocols-open"),
];

/// And of a PROTOCOL open: the transport and the three views.
pub const OF_A_PROTOCOL: &[(&str, &str)] = &[
    ("SPC", "protocols-play"),
    ("<left>", "protocols-previous"),
    ("<right>", "protocols-next"),
    ("r", "protocols-restart"),
    ("s", "protocols-speed"),
    ("n", "protocols-network"),
    ("q", "protocols-sequence"),
    ("a", "protocols-anatomy"),
];

/// The speeds it plays at, and what each is called.
pub const SPEEDS: [(f32, ronoctua_i18n::BookKey); 4] = [
    (0.5, ronoctua_packets::key::PACKETS_SPEED_HALF),
    (1.0, ronoctua_packets::key::PACKETS_SPEED_NORMAL),
    (1.5, ronoctua_packets::key::PACKETS_SPEED_FASTER),
    (2.0, ronoctua_packets::key::PACKETS_SPEED_DOUBLE),
];

/// The title's glyph.
static GLYPH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| tokens::icon::PROTOCOLS.to_string());

/// Opens the protocols in a window, on their list.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|_| Ok(Protocols::default()))
}

/// THE PROTOCOLS' CAMERA (`photograph --door protocols --scene <protocol>`):
/// the list for `list`, or a protocol at a moment — `http`, `http@3`,
/// `http@3:900`, `http@3:900:anatomy` (its step, the milliseconds into it,
/// and the view).
pub fn camera(rest: &[String]) -> ! {
    crate::photo::planted::taken(
        rest,
        "protocols",
        "list | <protocol>[@<step>[:<ms into it>[:network|sequence|anatomy]]]",
        |asked| {
            let mut w = Protocols {
                still: true,
                ..Protocols::default()
            };
            if asked.scene != "list" {
                w.pose(&asked.scene)?;
            }
            crate::photo::planted::shoot(asked, w, None)
        },
    )
}

/// The three ways a protocol is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    /// The devices, with the packets flying between them.
    #[default]
    Network,
    /// The packets as a sequence diagram.
    Sequence,
    /// The packet of the moment, bit by bit.
    Anatomy,
}

impl View {
    /// Its name, on its tab.
    #[must_use]
    pub const fn name(self) -> ronoctua_i18n::BookKey {
        match self {
            Self::Network => ronoctua_packets::key::PACKETS_VIEW_NETWORK,
            Self::Sequence => ronoctua_packets::key::PACKETS_VIEW_SEQUENCE,
            Self::Anatomy => ronoctua_packets::key::PACKETS_VIEW_ANATOMY,
        }
    }

    /// Its glyph.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Network => tokens::icon::PROTOCOLS_NETWORK,
            Self::Sequence => tokens::icon::PROTOCOLS_SEQUENCE,
            Self::Anatomy => tokens::icon::PROTOCOLS_ANATOMY,
        }
    }

    /// Every view, in the tabs' order.
    pub const ALL: [Self; 3] = [Self::Network, Self::Sequence, Self::Anatomy];
}

/// What a click on a place of the last frame does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Plays, or pauses.
    Play,
    /// To the step before.
    Previous,
    /// To the next step.
    Next,
    /// Back to the start.
    Restart,
    /// The next speed.
    Speed,
    /// This view.
    View(View),
    /// The start of this step.
    Step(usize),
    /// The moment this packet leaves.
    Packet(usize),
    /// A field of the anatomy: the header and the field.
    Field(usize, usize),
}

/// A box of the window, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Its left edge.
    pub x: f32,
    /// Its top.
    pub y: f32,
    /// Its width.
    pub w: f32,
    /// Its height.
    pub h: f32,
}

impl Rect {
    /// Is `(x, y)` in it?
    #[must_use]
    pub fn holds(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    /// The box as the renderer's area.
    #[must_use]
    pub const fn area(self) -> ronoctua_render::image::Area {
        ronoctua_render::image::Area {
            x: self.x,
            y: self.y,
            width: self.w,
            height: self.h,
        }
    }

    /// A box of the grid: `columns` cells from `column`, `lines` lines from `line`.
    #[must_use]
    pub fn of_cells(column: u16, line: u16, columns: u16, lines: u16) -> Self {
        let (cell, height) = grid();
        Self {
            x: f32::from(column) * cell,
            y: f32::from(line) * height,
            w: f32::from(columns) * cell,
            h: f32::from(lines) * height,
        }
    }
}

/// A cell's width and a line's height, in logical pixels, at the text's size.
#[must_use]
pub fn grid() -> (f32, f32) {
    let size = tokens::text::size();
    (
        tokens::metrics::cell_width(size),
        tokens::metrics::line_height(size),
    )
}

/// Text on the grid, from `column` of `line`.
#[must_use]
pub fn said(text: String, column: u16, line: u16, color: tokens::Color) -> ronoctua_render::Span {
    ronoctua_render::Span {
        size: ronoctua_render::Size::Px(tokens::text::size()),
        text,
        column,
        line,
        color,
        edge: false,
        foot: false,
    }
}

/// Text anywhere: its left edge at `x`, its middle at `y`, in logical pixels.
#[must_use]
pub fn placed(text: String, x: f32, y: f32, color: tokens::Color) -> ronoctua_render::Span {
    let (cell, line) = grid();
    let x = x.max(0.0);
    let y = y.max(0.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let column = (x / cell).floor() as u16;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let row = (y / line).floor() as u16;
    let across = x - f32::from(column) * cell;
    let by = y - f32::from(row) * line - line / 2.0;
    ronoctua_render::Span {
        size: ronoctua_render::Size::Placed(across, by, tokens::text::size()),
        text,
        column,
        line: row,
        color,
        edge: false,
        foot: false,
    }
}

/// How wide text is drawn, in logical pixels.
#[must_use]
pub fn wide(text: &str) -> f32 {
    f32::from(ronoctua_render::layout::width(text)) * grid().0
}

/// A flat rectangle.
#[must_use]
pub const fn quad(r: Rect, color: tokens::Color) -> ronoctua_render::quad::Quad {
    ronoctua_render::quad::Quad {
        x: r.x,
        y: r.y,
        width: r.w,
        height: r.h,
        color,
    }
}

/// The colour a packet's class is drawn in — the theme's categorical hues,
/// and the error's for what is refused.
#[must_use]
pub const fn ink_of(class: Class) -> tokens::Color {
    match class {
        Class::Lookup => tokens::color::CHART_ONE,
        Class::Handshake => tokens::color::CHART_TWO,
        Class::Request => tokens::color::CHART_THREE,
        Class::Response => tokens::color::CHART_FOUR,
        Class::Data => tokens::color::CHART_FIVE,
        Class::Control => tokens::color::CHART_SIX,
        Class::Crypto => tokens::color::CHART_SEVEN,
        Class::Refusal => tokens::color::STATE_ERROR,
    }
}

/// How long things take — the tokens, as the timeline takes them.
#[must_use]
pub fn pace() -> Pace {
    Pace {
        camera: u32::from(tokens::space::PACKETS_CAMERAMS),
        hop: u32::from(tokens::space::PACKETS_HOPMS),
        key: u32::from(tokens::space::PACKETS_KEYMS),
        light: u32::from(tokens::space::PACKETS_LIGHTMS),
        rest: u32::from(tokens::space::PACKETS_RESTMS),
    }
}

/// A protocol open, and where in it.
pub struct Open {
    /// The protocol.
    pub protocol: &'static Protocol,
    /// Its steps on a clock.
    pub line: Timeline,
    /// Where on it, in milliseconds.
    pub t: u32,
    /// Playing.
    pub playing: bool,
    /// When the last frame moved the time.
    pub last: Option<Instant>,
    /// The speed, by its place in [`SPEEDS`].
    pub speed: usize,
    /// How it is read.
    pub view: View,
    /// The anatomy's field under the pointer or chosen: its header and field.
    pub field: Option<(usize, usize)>,
    /// The anatomy's and the sequence's first row shown.
    pub scrolled: usize,
}

impl Open {
    /// A protocol, at its start.
    #[must_use]
    pub fn at_start(protocol: &'static Protocol) -> Self {
        Self {
            protocol,
            line: Timeline::of(protocol, pace()),
            t: 0,
            playing: false,
            last: None,
            speed: 1,
            view: View::Network,
            field: None,
            scrolled: 0,
        }
    }

    /// The scene now.
    #[must_use]
    pub fn pose(&self) -> Pose {
        self.line.at(self.t)
    }

    /// To the moment `t`, paused there or still playing.
    fn seek(&mut self, t: u32) {
        self.t = t.min(self.line.total);
        self.last = None;
        self.field = None;
        self.scrolled = 0;
    }
}

/// The window: the book, the list, and the protocol open.
#[derive(Default)]
pub struct Protocols {
    /// The book, and the language it was opened in.
    book: Option<(String, Result<Book, String>)>,
    /// What was typed to narrow the list.
    query: String,
    /// The list's cursor, among what the filter left.
    cursor: usize,
    /// The protocol open, if any.
    open: Option<Open>,
    /// No clock: a photograph's moment stays where it was posed.
    still: bool,
    /// Where the pointer rests, in logical pixels.
    pointer: Option<(f32, f32)>,
    /// Where the last frame put what a click takes.
    hits: Vec<(Rect, Hit)>,
    /// The wheel's fraction of a line, kept.
    wheel_left: f64,
    /// The drawings, by what they draw: made once, rasterised per key.
    pub(crate) drawings: crate::packets_stage::Drawings,
}

impl Protocols {
    /// The book in the language of now — opened again when that changed.
    fn book(&mut self, texts: &ronoctua_i18n::Texts) {
        let language = texts.language().to_owned();
        if self.book.as_ref().is_none_or(|(l, _)| *l != language) {
            let book = Book::open(&ronoctua_packets::BOOK, &language).map_err(|e| e.to_string());
            self.book = Some((language, book));
        }
    }

    /// Poses a photograph's scene: `<protocol>[@<step>[:<ms>[:<view>]]]`.
    fn pose(&mut self, scene: &str) -> Result<()> {
        let (id, at) = scene.split_once('@').unwrap_or((scene, ""));
        let protocol = catalogue::by_id(id).ok_or_else(|| {
            anyhow::anyhow!(
                "no protocol is called `{id}`; the words are {}",
                catalogue::ALL
                    .iter()
                    .map(|p| p.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        let mut open = Open::at_start(protocol);
        let mut parts = at.split(':').filter(|s| !s.is_empty());
        if let Some(step) = parts.next() {
            let step: usize = step.parse()?;
            anyhow::ensure!(
                (1..=protocol.steps.len()).contains(&step),
                "`{id}` has steps 1 to {}",
                protocol.steps.len()
            );
            let into: u32 = parts.next().map_or(Ok(0), str::parse)?;
            open.t = (open.line.start_of(step - 1) + into).min(open.line.total);
        }
        if let Some(view) = parts.next() {
            open.view = match view {
                "network" => View::Network,
                "sequence" => View::Sequence,
                "anatomy" => View::Anatomy,
                other => anyhow::bail!("`{other}` is no view: network, sequence or anatomy"),
            };
        }
        self.open = Some(open);
        Ok(())
    }

    /// The protocols the filter leaves, in the list's order: group by group.
    fn found(&self) -> Vec<&'static Protocol> {
        let query = self.query.to_lowercase();
        let book = self.book.as_ref().and_then(|(_, b)| b.as_ref().ok());
        let mut found: Vec<&'static Protocol> = catalogue::ALL
            .iter()
            .copied()
            .filter(|p| {
                if query.is_empty() {
                    return true;
                }
                let mut said = vec![p.id.to_owned()];
                if let Some(book) = book {
                    for k in [p.name, p.title, p.tagline, p.group.name(), p.layer.name()] {
                        said.push(book.t(k));
                    }
                }
                said.iter().any(|s| s.to_lowercase().contains(&query))
            })
            .collect();
        found.sort_by_key(|p| p.group);
        found
    }

    /// The list's rows: the heading, what was typed, and every protocol the
    /// filter leaves under its group's name — and the row of each protocol.
    fn list(&self, ui: &Ui) -> (Vec<Vec<Field>>, Vec<Option<usize>>) {
        let piece = |at: u16, text: String, color: tokens::Color| Field {
            at,
            text,
            color,
            hit: crate::fields::Hit::Nothing,
            explain: None,
            ink: crate::fields::Ink::Grid,
        };
        let columns = ui.columns.saturating_sub(2 * ui.margin + 1);
        let mut rows = vec![
            vec![piece(
                0,
                ui.texts.t(crate::key::PROTOCOLS_LIST),
                tokens::color::TEXT_HEADING,
            )],
            if self.query.is_empty() {
                vec![piece(
                    0,
                    ui.texts.t(crate::key::PROTOCOLS_FILTER_EMPTY),
                    tokens::color::TEXT_DIM,
                )]
            } else {
                vec![piece(
                    0,
                    ronoctua_render::layout::elide_start(&self.query, columns),
                    tokens::color::QUESTION_TYPED,
                )]
            },
        ];
        let mut of = vec![None, None];
        let Some((_, Ok(book))) = &self.book else {
            let why = match &self.book {
                Some((_, Err(why))) => why.clone(),
                _ => String::new(),
            };
            rows.push(vec![piece(0, why, tokens::color::TEXT_SECONDARY)]);
            of.push(None);
            return (rows, of);
        };
        let found = self.found();
        if found.is_empty() {
            rows.push(vec![piece(
                0,
                ui.texts.t(crate::key::PROTOCOLS_NONE),
                tokens::color::TEXT_DIM,
            )]);
            of.push(None);
            return (rows, of);
        }
        // THE NAMES' COLUMN is as wide as the widest name of every protocol,
        // and not of what the filter left: nothing jumps as one types.
        let names = catalogue::ALL
            .iter()
            .map(|p| ronoctua_render::layout::width(&book.t(p.name)))
            .max()
            .unwrap_or(0);
        let mut group = None;
        for (k, p) in found.iter().enumerate() {
            if group != Some(p.group) {
                group = Some(p.group);
                rows.push(Vec::new());
                of.push(None);
                rows.push(vec![piece(
                    0,
                    book.t(p.group.name()),
                    tokens::color::TEXT_HEADING,
                )]);
                of.push(None);
            }
            let title = book.t(p.title);
            let at = names + 2;
            let rest = columns.saturating_sub(at);
            let line = format!("{title} — {}", book.t(p.tagline));
            let layer = book.t(p.layer.name());
            let layer_cells = ronoctua_render::layout::width(&layer);
            let room = rest.saturating_sub(layer_cells + 2);
            let mut row = vec![
                piece(0, book.t(p.name), tokens::color::TEXT_KEY),
                piece(
                    at,
                    ronoctua_render::layout::elide_end(&line, room),
                    tokens::color::TEXT_SECONDARY,
                ),
            ];
            if rest > layer_cells + 12 {
                row.push(piece(
                    columns.saturating_sub(layer_cells),
                    layer,
                    tokens::color::TEXT_DIM,
                ));
            }
            rows.push(row);
            of.push(Some(k));
        }
        (rows, of)
    }

    /// Opens the protocol under the cursor.
    fn open_here(&mut self, ui: &mut Ui) {
        if let Some(p) = self.found().get(self.cursor).copied() {
            self.open = Some(Open::at_start(p));
            ui.offset = 0;
            ui.ask_paint();
        }
    }

    /// A key of the list.
    fn list_key(&mut self, ui: &mut Ui, key: Key) {
        let chord = key.mods.ctrl || key.mods.alt || key.mods.logo;
        if crate::line::pastes(key) {
            crate::region::Doorway::paste(ui);
            return;
        }
        let last = self.found().len().saturating_sub(1);
        match key.sym {
            Sym::Esc if !self.query.is_empty() => {
                self.query.clear();
                self.cursor = 0;
            }
            Sym::Esc => {
                crate::region::Doorway::close(ui);
                return;
            }
            Sym::Down => self.cursor = (self.cursor + 1).min(last),
            Sym::Up => self.cursor = self.cursor.saturating_sub(1),
            Sym::Enter => {
                self.open_here(ui);
                return;
            }
            Sym::Backspace if crate::line::pop_cluster(&mut self.query) => self.cursor = 0,
            _ if chord => return,
            Sym::Space => {
                self.query.push(' ');
                self.cursor = 0;
            }
            Sym::Char(c) => {
                self.query.push(c);
                self.cursor = 0;
            }
            _ => return,
        }
        let (rows, of) = self.list(ui);
        if let Some(row) = of.iter().position(|o| *o == Some(self.cursor)) {
            ui.follow(row, rows.len());
        }
        ui.ask_paint();
    }

    /// A key of a protocol open.
    fn protocol_key(&mut self, ui: &mut Ui, key: Key) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        match key.sym {
            Sym::Esc => {
                self.open = None;
                ui.offset = 0;
            }
            Sym::Space => self.act(ui, Hit::Play),
            Sym::Left => self.act(ui, Hit::Previous),
            Sym::Right => self.act(ui, Hit::Next),
            Sym::Home => self.act(ui, Hit::Restart),
            Sym::End => {
                let end = open.line.total;
                open.seek(end);
                open.playing = false;
            }
            Sym::Down | Sym::Up => {
                let pose = open.pose();
                let count = open.protocol.packets().count();
                let to = match (key.sym, pose.current) {
                    (Sym::Down, None) => 0,
                    (Sym::Down, Some(k)) => (k + 1).min(count.saturating_sub(1)),
                    (_, Some(k)) => k.saturating_sub(1),
                    (_, None) => 0,
                };
                self.act(ui, Hit::Packet(to));
            }
            Sym::Char('r') => self.act(ui, Hit::Restart),
            Sym::Char('s') => self.act(ui, Hit::Speed),
            Sym::Char('n') => self.act(ui, Hit::View(View::Network)),
            Sym::Char('q') => self.act(ui, Hit::View(View::Sequence)),
            Sym::Char('a') => self.act(ui, Hit::View(View::Anatomy)),
            _ => return,
        }
        ui.ask_paint();
    }

    /// Does what a click or a key asked of a protocol open.
    pub(crate) fn act(&mut self, ui: &mut Ui, hit: Hit) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let step = open.line.step_at(open.t);
        match hit {
            Hit::Play => {
                if open.t >= open.line.total {
                    open.seek(0);
                }
                open.playing = !open.playing;
                open.last = None;
            }
            Hit::Previous => {
                // THE START OF THIS STEP, or of the one before when already
                // there.
                let here = open.line.start_of(step);
                let to = if open.t > here + 50 {
                    step
                } else {
                    step.saturating_sub(1)
                };
                open.seek(open.line.start_of(to));
            }
            Hit::Next => {
                let to = (step + 1).min(open.protocol.steps.len());
                open.seek(open.line.start_of(to));
                if to == open.protocol.steps.len() {
                    open.playing = false;
                }
            }
            Hit::Restart => {
                open.seek(0);
                open.playing = false;
            }
            Hit::Speed => open.speed = (open.speed + 1) % SPEEDS.len(),
            Hit::View(view) => {
                open.view = view;
                open.scrolled = 0;
            }
            Hit::Step(s) => open.seek(open.line.start_of(s)),
            Hit::Packet(k) => {
                if let Some(t) = open.line.leaves(k) {
                    // JUST AFTER IT LEAVES, so it is the packet of the moment.
                    open.seek(t + 1);
                    open.playing = false;
                }
            }
            Hit::Field(h, f) => open.field = Some((h, f)),
        }
        ui.ask_paint();
    }

    /// The time, moved by what passed since the last frame, times the speed.
    fn tick(&mut self) {
        if self.still {
            return;
        }
        let Some(open) = self.open.as_mut() else {
            return;
        };
        if !open.playing {
            open.last = None;
            return;
        }
        let now = Instant::now();
        if let Some(last) = open.last {
            let passed = now.duration_since(last).as_secs_f32() * 1000.0 * SPEEDS[open.speed].0;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let moved = passed.round() as u32;
            open.t = (open.t + moved).min(open.line.total);
            if open.t >= open.line.total {
                open.playing = false;
            }
        }
        open.last = Some(now);
    }
}

impl Content for Protocols {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::MANUAL_PROTOCOLS)
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        self.book(&ui.texts);
        if self.open.is_some() {
            self.protocol_key(ui, key);
        } else {
            self.list_key(ui, key);
        }
    }

    /// What was copied, into the list's filter — as one line.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        if self.open.is_none() {
            self.query.push_str(&crate::line::one_line(text));
            self.cursor = 0;
            ui.ask_paint();
        }
    }

    /// The level's keys, the way back first — every one a click.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        let keys = if self.open.is_some() {
            OF_A_PROTOCOL
        } else {
            OF_THE_LIST
        };
        crate::chrome::with_the_way_back(crate::chrome::hint_of(keys, &ui.texts), &ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<Field>> {
        if self.open.is_some() {
            return Vec::new();
        }
        self.list(ui).0
    }

    /// A click takes a protocol of the list, and only its rows light.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        if self.open.is_some() {
            return Vec::new();
        }
        self.list(ui).1.iter().map(Option::is_some).collect()
    }

    fn cursor(&self) -> Option<usize> {
        None
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE OPENS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        if self.open.is_some() {
            return;
        }
        let (_, of) = self.list(ui);
        let Some(Some(k)) = of.get(row).copied() else {
            return;
        };
        if k == self.cursor {
            self.open_here(ui);
        } else {
            self.cursor = k;
            ui.ask_paint();
        }
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        self.book(&ui.texts);
        self.hits.clear();
        if self.open.is_none() {
            // THE LIST: the door's own, the cursor's row lit as the cursor.
            let (rows, of) = self.list(ui);
            let mut rows = rows;
            if let Some(row) = of.iter().position(|o| *o == Some(self.cursor)) {
                for f in &mut rows[row] {
                    f.color = if f.at == 0 {
                        tokens::color::TEXT_HERE_ACTS
                    } else {
                        tokens::color::TEXT_HERE_READ
                    };
                }
            }
            let hint = self.hint(ui);
            crate::door::list::header_of(ui, canvas, None, |_| hint);
            let takes: Vec<bool> = of.iter().map(Option::is_some).collect();
            crate::door::list::body(ui, canvas, &rows, &takes, None);
            return;
        }
        self.tick();
        let label = ui.texts.t(crate::key::PROTOCOLS_LEAD);
        let hint = self.hint(ui);
        let Self {
            book,
            open,
            hits,
            drawings,
            pointer,
            still,
            ..
        } = self;
        let (Some((_, Ok(book))), Some(open)) = (book.as_ref(), open.as_ref()) else {
            crate::door::list::header_of(ui, canvas, None, |_| hint);
            let why = match book {
                Some((_, Err(why))) => why.clone(),
                _ => String::new(),
            };
            canvas.spans.push(said(
                why,
                ui.margin,
                ui.first_of_list(),
                tokens::color::TEXT_SECONDARY,
            ));
            return;
        };
        let name = book.t(open.protocol.name);
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: Some(GLYPH.as_str()),
                label: Some(&label),
                name: Some(&name),
            }),
            |_| hint,
        );
        let layout = Layout::of(ui);
        let pose = open.pose();
        let band = canvas
            .lit
            .first()
            .map_or(tokens::color::SURFACE_SELECTION, |(c, _)| *c);
        let mut paint = Paint {
            canvas,
            hits,
            hover: *pointer,
            band,
            scale: ui.scale.max(1),
        };
        crate::packets_panel::top_line(&mut paint, &layout, book, open);
        match open.view {
            View::Network => {
                crate::packets_stage::paint(&mut paint, &layout, book, open, &pose, drawings);
            }
            View::Sequence => {
                crate::packets_sequence::paint(&mut paint, &layout, book, open, &pose)
            }
            View::Anatomy => crate::packets_anatomy::paint(&mut paint, &layout, book, open, &pose),
        }
        crate::packets_panel::paint(&mut paint, &layout, book, &ui.texts, open, &pose);
        crate::packets_table::paint(&mut paint, &layout, book, &ui.texts, open, &pose);
        if open.playing && !*still {
            ui.pace();
        }
    }

    /// While it plays, the frame shown asks for the next.
    fn frame_done(&mut self, ui: &mut Ui) {
        if self.open.as_ref().is_some_and(|o| o.playing) && !self.still {
            ui.ask_paint();
        }
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut crate::door::Hand<()>, events: &[PointerEvent]) {
        if self.open.is_none() {
            crate::door::list::pointer(self, ui, hand, events);
            return;
        }
        for e in events {
            let (took, said) = crate::door::list::header_pointer(self, ui, e);
            if took || said {
                ui.ask_paint();
            }
            if took {
                continue;
            }
            #[allow(clippy::cast_possible_truncation)]
            let (x, y) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    let before = self.under(self.pointer);
                    self.pointer = Some((x, y));
                    let now = self.under(self.pointer);
                    if before != now {
                        if let (Some(Hit::Field(h, f)), Some(open)) = (now, self.open.as_mut()) {
                            open.field = Some((h, f));
                        }
                        ui.ask_paint();
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer = None;
                    ui.ask_paint();
                }
                PointerEventKind::Press { button, .. } if button == crate::door::LEFT_BUTTON => {
                    if let Some(hit) = self.under(Some((x, y))) {
                        self.act(ui, hit);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut self.wheel_left, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut self.wheel_left);
                    if let Some(open) = self.open.as_mut().filter(|_| by != 0) {
                        open.scrolled = usize::try_from(
                            i64::try_from(open.scrolled)
                                .unwrap_or(0)
                                .saturating_add(by)
                                .max(0),
                        )
                        .unwrap_or(0);
                        ui.ask_paint();
                    }
                }
                _ => {}
            }
        }
    }
}

impl Protocols {
    /// What a click at `at` takes, by the last frame.
    fn under(&self, at: Option<(f32, f32)>) -> Option<Hit> {
        let (x, y) = at?;
        self.hits
            .iter()
            .find(|(r, _)| r.holds(x, y))
            .map(|(_, h)| *h)
    }
}

/// Where the parts of a protocol open go: the line of its title and views,
/// the stage, the panel, the transport and the table — in logical pixels and
/// in cells.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    /// The title's line.
    pub title_line: u16,
    /// The window's first and last columns of content, and its margin.
    pub left: u16,
    /// One past the last column.
    pub right: u16,
    /// The stage, in cells: its columns and lines.
    pub stage: (u16, u16, u16, u16),
    /// The panel, in cells.
    pub panel: (u16, u16, u16, u16),
    /// The transport's line.
    pub controls_line: u16,
    /// The table: its first line and how many.
    pub table: (u16, u16),
}

impl Layout {
    /// The layout of a window this big.
    #[must_use]
    pub fn of(ui: &Ui) -> Self {
        let left = ui.margin;
        let right = ui.columns.saturating_sub(ui.margin + 1);
        let width = right.saturating_sub(left);
        let title_line = ui.first_of_list();
        let bottom = ui.lines;
        let free = bottom.saturating_sub(title_line + 2);
        let table_lines = tokens::space::PACKETS_TABLELINES.min(free / 3);
        let table_top = bottom.saturating_sub(table_lines);
        let controls_line = table_top.saturating_sub(2);
        let top = title_line + 2;
        let middle = controls_line.saturating_sub(top + 1);
        let panel_cells = tokens::space::PACKETS_PANELCELLS;
        let side_by_side = width >= panel_cells + tokens::space::PACKETS_STAGELEASTCELLS + 3;
        let (stage, panel) = if side_by_side {
            let stage_w = width - panel_cells - 3;
            (
                (left, top, stage_w, middle),
                (left + stage_w + 3, top, panel_cells, middle),
            )
        } else {
            let stage_h = middle * 3 / 5;
            (
                (left, top, width, stage_h),
                (
                    left,
                    top + stage_h + 1,
                    width,
                    middle.saturating_sub(stage_h + 1),
                ),
            )
        };
        Self {
            title_line,
            left,
            right,
            stage,
            panel,
            controls_line,
            table: (table_top, table_lines),
        }
    }

    /// The stage, in logical pixels.
    #[must_use]
    pub fn stage_rect(&self) -> Rect {
        let (c, l, w, h) = self.stage;
        Rect::of_cells(c, l, w, h)
    }
}

/// What a part of the window paints with: the canvas, the places a click
/// takes, the pointer, the colour of a band under it, and the screen's scale.
pub struct Paint<'p, 'c> {
    /// The frame.
    pub canvas: &'p mut Canvas<'c>,
    /// Where a click takes what.
    pub hits: &'p mut Vec<(Rect, Hit)>,
    /// Where the pointer rests.
    pub hover: Option<(f32, f32)>,
    /// The colour of the band under the pointer.
    pub band: tokens::Color,
    /// Physical pixels a logical one.
    pub scale: i32,
}

impl Paint<'_, '_> {
    /// A place a click takes — lit, under the pointer, by a band.
    pub fn takes(&mut self, r: Rect, hit: Hit) {
        if self.hover.is_some_and(|(x, y)| r.holds(x, y)) {
            self.canvas.under.push(quad(r, self.band));
        }
        self.hits.push((r, hit));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui() -> Ui {
        Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"))
    }

    fn key(w: &mut Protocols, ui: &mut Ui, notation: &str) {
        let (event, mods) = crate::door::keys::event_of(Key::parse(notation).expect("a key"));
        w.typed(ui, &event, mods);
    }

    /// FAILS IF: the list does not narrow to what is typed, `RET` does not
    /// open the protocol under the cursor, or `Esc` does not go back to the
    /// list before it closes the window.
    #[test]
    fn the_list_narrows_and_opens_and_esc_goes_back() {
        let mut w = Protocols::default();
        let mut ui = ui();
        let all = w.found().len();
        for c in ["d", "n", "s", "s", "e", "c"] {
            key(&mut w, &mut ui, c);
        }
        let found: Vec<&str> = w.found().iter().map(|p| p.id).collect();
        assert!(found.len() < all, "nothing was narrowed");
        assert_eq!(found.first(), Some(&"dnssec"), "{found:?}");
        key(&mut w, &mut ui, "RET");
        assert_eq!(w.open.as_ref().map(|o| o.protocol.id), Some("dnssec"));
        key(&mut w, &mut ui, "ESC");
        assert!(w.open.is_none());
        assert!(!ui.quit, "Esc out of a protocol closed the window");
    }

    /// FAILS IF: play does not move the time, the arrows do not walk the
    /// steps, or a packet chosen is not the packet of the moment.
    #[test]
    fn the_transport_walks_the_steps_and_the_packets() {
        let mut w = Protocols::default();
        let mut ui = ui();
        w.pose("http").expect("http");
        let open = w.open.as_ref().expect("open");
        assert_eq!(open.line.step_at(open.t), 0);
        key(&mut w, &mut ui, "<right>");
        let open = w.open.as_ref().expect("open");
        assert_eq!(open.line.step_at(open.t), 1);
        key(&mut w, &mut ui, "<right>");
        key(&mut w, &mut ui, "<left>");
        let open = w.open.as_ref().expect("open");
        assert_eq!(open.line.step_at(open.t), 1);
        w.act(&mut ui, Hit::Packet(3));
        let open = w.open.as_ref().expect("open");
        assert_eq!(open.pose().current, Some(3));
        w.act(&mut ui, Hit::Play);
        assert!(w.open.as_ref().is_some_and(|o| o.playing));
    }

    /// FAILS IF: a photograph's scene is not posed where it says, or a
    /// protocol it does not have is let through.
    #[test]
    fn a_photograph_poses_the_moment_it_names() {
        let mut w = Protocols::default();
        w.pose("tcp@4:100:anatomy").expect("posed");
        let open = w.open.as_ref().expect("open");
        assert_eq!(open.line.step_at(open.t), 3);
        assert_eq!(open.view, View::Anatomy);
        assert!(w.pose("gopher").is_err());
        assert!(w.pose("tcp@99").is_err());
    }
}
