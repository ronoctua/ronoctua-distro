//! The player in a window (`LEADER p w`) and in the column on the right
//! (`LEADER p c`) — two doors onto `crate::player`, one host.
//!
//! What a key does and which rows exist is `player.rs`; the plumbing of a
//! surface is `crate::door`'s, as every window's is. What this holds is how
//! this door lays the picture out — above a transport line of glyphs and a
//! time bar a hand can drag, the rows under them — and its stage. Whatever the
//! view asks is typed on this door's own line, as the footer types it on its
//! minibuffer (`player_question::Question`).
//!
//! # Two surfaces, one player
//!
//! The window is an `xdg_toplevel`, a `door::Content` of its own. The column
//! is the player as the first TENANT of `crate::column`: the base keeps its
//! size, its control line, its focus line, its workspace and its slot, and
//! the player draws in the `column::Area` it is handed — the same painter as
//! the window's, a line lower. Large, medium and small adapt: the rows
//! reflow, the transport draws the buttons that fit and the picture takes the
//! size's share (`Size::picture_lines`).
//!
//! # The bar is a remote control
//!
//! At the bar's width nothing of the view fits, so the player draws content
//! of its own there (`Shape::Own`), one glyph a line: previous, play or pause
//! and next; a line of air; louder, the mute and quieter; a line of air; and
//! the swap to the window — each taken across the whole line and lit under
//! the pointer, and the wheel is the volume. No sentence explains them (the
//! owner's decision): these are the glyphs every player has. The bar's last
//! line is the column's own close (`column::Area::end`). A key that acts on a
//! row nobody sees puts the column back to its size, and so does a question
//! — both are the base's (`Tenant::acts_without_rows`).
//!
//! # The swap
//!
//! Every door carries the glyph that trades it for the other
//! (`player::Errand::Swap`): the window's transport ends with the column —
//! raised, never toggled, and the window closes — and the column's with the
//! window, which opens as the column closes. A window opened for a stage
//! swaps the same way, and closing it is what closes the stage.
//!
//! # The frames are the clock
//!
//! Each picture has a thread of its own (`crate::player_picture`), and every
//! frame that arrives is what asks for the next paint — no timer.

use std::{
    cell::RefCell,
    sync::{Arc, atomic::AtomicBool},
};

use anyhow::{Context as _, Result};
use ronoctua_player::wire::{Ask, State};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::{self, channel},
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
    shell::{
        WaylandSurface as _,
        xdg::{
            XdgShell,
            window::{Window, WindowDecorations},
        },
    },
};
use wayland_client::protocol::wl_surface;

use crate::column::{Area, Size};
use crate::door::{self as base, Canvas, Content, Hand, LEFT_BUTTON, Shell, Ui, Wiring};
use crate::player_transport::{bar_buttons, glyph_of, lit};

/// The door this module is: the plumbing, and the player in it.
type Host = base::Door<Player>;

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.player";

/// The margin inside the surface, in cells — see `chrome::FIELDS_LEAD`.
const MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Which door the player is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    /// A floating window.
    Window,
    /// The column on the right.
    Column,
    /// The footer's media panel (`crate::player_media`) — never a host of
    /// this module: it is here so the glyphs its transport wears are asked
    /// the one way the other two ask them (`player_transport::glyph_of`).
    Footer,
}

/// How quick a second click on the picture has to be to be a DOUBLE click,
/// which puts it on the whole screen and back.
pub const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

/// How the picture is shown in this door.
///
/// # The picture alone, and the controls over it
///
/// Off, the door is the view: the picture on top, the transport, the list.
/// On stage — `Only`, or `Maximized` (the window filling the screen) — the
/// picture takes the whole surface, letterboxed in black by the player, and
/// everything else is hidden. The transport and the header come back OVER the
/// picture, on the `surface.transport` band, on any activity: the pointer
/// moving, a click, the wheel, a key. They go again [`CONTROLS_LINGER`] after
/// the last one, and never while the hand rests on them — a control that
/// vanished under the pointer about to press it is one nobody can press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The door as it is: the picture, the transport, the list.
    Off,
    /// Only the picture, in this door.
    Only,
    /// Only the picture, on the whole screen.
    Maximized,
    /// Only the picture, in the window made small and floated in the
    /// bottom-right corner — picture-in-picture.
    Small,
    /// A LISTING of the catalogue alone on the whole screen: its rows and
    /// nothing else — no picture, no transport (`ronoctua_shell_catalogue::player_catalog`).
    Listing,
}

/// How many frames the corner is asked for while the window is not yet in
/// the compositor's tree.
const CORNER_TRIES: u8 = 20;

/// How long between two asks for the corner.
const CORNER_WAIT: std::time::Duration = std::time::Duration::from_millis(150);

/// How long the controls stay over the picture after the last activity.
const CONTROLS_LINGER: std::time::Duration = std::time::Duration::from_secs(3);

/// Where everything is, in lines of the grid — ONE answer the painter and
/// the pointer both read, so a control is taken where it was drawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    /// The picture band — its first line and how many lines it takes — when
    /// something is shown on this door's screen: the pictures are placed IN
    /// it (`player_layout::placed`), and a click on it is a click on them.
    pub band: Option<(u16, u16)>,
    /// The transport's buttons and the clock.
    pub transport: u16,
    /// The deck's first line — the time bar, or a set's ruler with its lanes
    /// under it (`crate::player_timeline`).
    pub bar: u16,
    /// How many lines the deck takes: one for the time bar.
    pub deck: u16,
    /// The list's first line.
    pub list: u16,
}

/// The listing on its stage: the rows from line `first` to the last, and
/// nothing else — the transport and the deck past the surface's end, where
/// nothing draws them and no pointer reaches. A line PAST the last whole
/// one: a surface is seldom a whole number of lines, and the part of a line
/// under the last one still shows what is drawn on it.
#[must_use]
pub const fn listing_layout(first: u16, lines: u16) -> Layout {
    Layout {
        band: None,
        transport: lines.saturating_add(1),
        bar: lines.saturating_add(1),
        deck: 1,
        list: first,
    }
}

/// The door on stage: the pictures over the whole surface from line `top` —
/// under a column's control line — the transport and the time bar on its
/// last two lines, and no list.
#[must_use]
pub fn stage_layout(top: u16, lines: u16) -> Layout {
    Layout {
        band: Some((top, lines.saturating_sub(top).max(1))),
        transport: lines.saturating_sub(2),
        bar: lines.saturating_sub(1),
        deck: 1,
        list: lines,
    }
}

/// Lays the door out. `area` is what the door is handed (a window's whole
/// surface, or a column's size), `first` the line under the header, `lines`
/// how many the surface has, `columns` how wide it is in cells,
/// `line_over_cell` the font's line height over its cell width, `shape` the
/// width over the height the pictures shown here want together
/// (`arrange::natural_aspect`) — none, and there is no band — and `deck` how
/// many lines the deck under the transport takes: one, the time bar, but for
/// a set.
#[must_use]
pub fn layout(
    area: &Area,
    first: u16,
    lines: u16,
    columns: u16,
    line_over_cell: f32,
    shape: Option<f32>,
    deck: u16,
) -> Layout {
    let mut at = first;
    let mut band = None;
    if let Some(shape) = shape.filter(|s| s.is_finite() && *s > 0.0) {
        // THE PICTURES take the height their shape wants at this width — in
        // lines, from the width in cells and the font's own proportion — and
        // never more than half the surface: the rows are the controls.
        let wanted = f32::from(columns) / shape / line_over_cell;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let wanted = wanted.ceil().clamp(1.0, f32::from(u16::MAX)) as u16;
        // A COLUMN'S PICTURES are its size's share of the width, and never
        // past half the surface either (`Area::picture_ceiling`): a picture
        // added to a set never pushes the rows off.
        let ceiling = area.picture_ceiling(lines);
        let height = wanted.min(ceiling).max(1);
        band = Some((at, height));
        at += height;
    }
    let deck = deck.max(1);
    Layout {
        band,
        transport: at,
        bar: at + 1,
        deck,
        list: at + 1 + deck,
    }
}

/// Opens the player in a window, on `stage`.
///
/// # Errors
///
/// No Wayland compositor, or no `xdg_shell`.
pub fn host_window(stage: Stage, screen: Option<String>) -> Result<()> {
    // THIS WINDOW'S FILE, for as long as it is open: closing the player
    // everywhere with no daemon to say farewell finds it there.
    let _slot = crate::player_stage::WindowSlot::hold();
    // AND A SCREEN'S, for a window "show on every screen" opened there:
    // pressing it again finds every one (`player_stage::everywhere`).
    let _screen = screen.as_deref().map(crate::player_stage::ScreenSlot::hold);
    crate::door::run(|w: &Wiring<'_, Player>| wired(w, Door::Window, stage, screen.clone()))
}

/// Opens the player in the column on the right, as `opening` says
/// (`crate::column`).
///
/// # Errors
///
/// No Wayland compositor, or no layer shell.
pub fn host_column(opening: &crate::column::Opening) -> Result<()> {
    crate::column::run::<Player>(opening, |w| wired(w, Door::Column, Stage::Off, None))
}

/// A door the player is in — the window, or the column around it — and how
/// to reach the player inside, so the channels and the timers hung on its
/// loop are written once for both.
trait PlayerHost: Content<Hand = PerPointer> {
    /// The player, what every door keeps, and the pointer being handled.
    fn player(d: &mut base::Door<Self>) -> (&mut Player, &mut Ui, &mut Hand<PerPointer>);
}

impl PlayerHost for Player {
    fn player(d: &mut Host) -> (&mut Player, &mut Ui, &mut Hand<PerPointer>) {
        (&mut d.content, &mut d.ui, &mut d.here)
    }
}

impl PlayerHost for crate::column::Column<Player> {
    fn player(d: &mut base::Door<Self>) -> (&mut Player, &mut Ui, &mut Hand<PerPointer>) {
        (&mut d.content.tenant, &mut d.ui, &mut d.here)
    }
}

/// The player, with its channels hung on the loop of the door `C` it is in.
fn wired<C: PlayerHost>(
    w: &Wiring<'_, C>,
    door: Door,
    stage: Stage,
    screen: Option<String>,
) -> Result<Player> {
    // THE PLAYER'S STATE, on its own thread. A door that opens starts the
    // daemon: the nested loop has no unit, and a door onto nothing is a door
    // that says "not running" for ever.
    let _ = ronoctua_player::client::ensure();
    let (heard_sender, heard) = channel::channel::<ronoctua_player::client::Heard>();
    let watch = ronoctua_player::client::watch(move |h| {
        let _ = heard_sender.send(h);
    });
    w.handle
        .insert_source(heard, |event, (), d: &mut base::Door<C>| {
            if let channel::Event::Msg(h) = event {
                let (p, ui, _) = C::player(d);
                p.heard(ui, h);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the player channel: {e}"))?;

    // HOW A PAIRING ENDED, from the thread that waited for the other
    // machine (`player_machines::pair_then`).
    let (paired, pairings) = channel::channel::<crate::player_machines::Paired>();
    w.handle
        .insert_source(pairings, |event, (), d: &mut base::Door<C>| {
            if let channel::Event::Msg(p) = event {
                let (_, ui, _) = C::player(d);
                match crate::player_machines::paired_said(&p, &ui.texts) {
                    Some(said) => ui.say(said),
                    None => ui.message = None,
                }
                ui.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the pairing channel: {e}"))?;

    // HOW AN ASK TOLD OFF THE LOOP ENDED (`player::tell_then`): a refusal
    // is said here, in the door that asked, in place of what it said it did.
    let (told, answers) = channel::channel::<crate::player::Told>();
    w.handle
        .insert_source(answers, |event, (), d: &mut base::Door<C>| {
            if let channel::Event::Msg(answer) = event {
                let (player, ui, _) = C::player(d);
                // A DRAFT, where the daemon wrote it: into the editor.
                if let crate::player::Told::Draft(path) = &answer {
                    player.draft_opened(ui, path);
                    return;
                }
                player.answer_told(ui, &answer);
                // AN ADULT LISTING HANDED OVER, a password refused: asked
                // again, masked, while it waits.
                if let Some(ask) = player.view.lock_owed() {
                    player.apply(ui, ask);
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the player's answers: {e}"))?;

    // A READ DONE OFF THE LOOP — the folder beside the film — taken by the
    // view where the thread left it, and the rows drawn again with it.
    let (woke, wakes) = channel::channel::<()>();
    w.handle
        .insert_source(wakes, |event, (), d: &mut base::Door<C>| {
            if let channel::Event::Msg(()) = event {
                let (p, ui, _) = C::player(d);
                p.view.settle_in(p.player.as_ref(), ui.columns, &ui.texts);
                ui.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the view's wake: {e}"))?;
    let woke = std::sync::Mutex::new(woke);
    let wake = crate::wake::Wake::new(move || {
        if let Ok(w) = woke.lock() {
            let _ = w.send(());
        }
    });

    // THE GLOBE OF A DIRECTORY'S PAGE, drawn on a thread of its own and
    // handed back on a channel of its own: a drag across the world is a
    // picture every few tens of milliseconds, and none of them is news about
    // the player (`crate::globe`).
    let (globe_sender, globes) = channel::channel::<crate::globe::Beat>();
    w.handle
        .insert_source(globes, |event, (), d: &mut base::Door<C>| {
            if let channel::Event::Msg(b) = event {
                let (p, ui, _) = C::player(d);
                if p.view.saw_globe(&b) {
                    ui.ask_paint();
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the globe channel: {e}"))?;

    // A PING PER FRAME, from whichever picture's thread has one.
    let (frame_sender, frames) = channel::channel::<usize>();
    w.handle
        .insert_source(frames, |event, (), d: &mut base::Door<C>| {
            if let channel::Event::Msg(k) = event {
                let (p, ui, _) = C::player(d);
                p.frame_arrived(ui, k);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the preview channel: {e}"))?;

    // THE TWO TIMERS, armed on this door's loop — built here, where the
    // door's type is known, and kept as closures by a player that does not
    // know which door it is in.
    let handle = w.handle.clone();
    let arm_controls: Arm<Option<calloop::RegistrationToken>> = Box::new(move || {
        handle
            .insert_source(
                calloop::timer::Timer::from_duration(CONTROLS_LINGER),
                |_, (), d: &mut base::Door<C>| {
                    let (p, ui, here) = C::player(d);
                    p.controls_tick(ui, here)
                },
            )
            // NOT SILENT, and the controls STAY: a transport that does not go
            // is a smaller failure than one that cannot be brought back.
            .map_err(|e| tracing::warn!(error = %e, "no timer for the controls; they will stay"))
            .ok()
    });
    let handle = w.handle.clone();
    let arm_corner: Arm<bool> = Box::new(move || {
        handle
            .insert_source(
                calloop::timer::Timer::from_duration(CORNER_WAIT),
                |_, (), d: &mut base::Door<C>| {
                    let (p, _, _) = C::player(d);
                    p.corner_timer = false;
                    p.float_in_the_corner();
                    calloop::timer::TimeoutAction::Drop
                },
            )
            .is_ok()
    });

    // THE STAGE IT WAS OPENED ON (`ronoctua-shell player
    // only|maximized|pip`), and the slot that lets the door that asked
    // close it again. The stage itself is set on the first turn, once the
    // window exists to be made whole-screen or small.
    let opened_for = (stage != Stage::Off).then_some(stage);
    // A SCREEN'S WINDOW holds that screen's slot (`host_window`), and not
    // the one stage's: it is one of several.
    if opened_for.is_some() && screen.is_none() {
        crate::player_stage::hold(match stage {
            Stage::Only | Stage::Off => crate::player::Show::Only,
            Stage::Maximized => crate::player::Show::Maximized,
            Stage::Small => crate::player::Show::Small,
            Stage::Listing => crate::player::Show::Listing,
        });
    }
    // A WINDOW OPENED FOR THE LISTING opens on the one the door that asked
    // handed over (the catalogue's page, `crate::player_seam`) — and a window
    // takes the listing's stage itself: its `F` hands nothing over.
    let mut view = crate::player::View::fresh().waking(wake);
    if door == Door::Window {
        view.takes_its_own_stage();
    }
    if stage == Stage::Listing {
        view.handed_over();
    }

    Ok(Player {
        door,
        arm_controls,
        arm_corner,
        window: RefCell::new(None),
        reach: Reach::machine(),
        view,
        pairing: crate::player_machines::Pairing::default(),
        paired,
        told: crate::player::ToldSender::of_channel(told),
        player: None,
        pictures: Vec::new(),
        placed: Vec::new(),
        screen_wanted: screen,
        placed_on: None,
        sinks: None,
        screens_given: None,
        frame_sender,
        buttons: Vec::new(),
        clock: None,
        picture_folded: false,
        layout: Layout::default(),
        deck: blank_deck(),
        deck_offset: 0,
        dragged: None,
        deck_under: None,
        deck_chosen: None,
        stage: Stage::Off,
        first_stage: opened_for,
        opened_for,
        controls_until: None,
        controls_timer: None,
        corner_owed: 0,
        fullscreen: false,
        corner_timer: false,
        painted: false,
        button_under: None,
        pointer_at: None,
        globe_stage: crate::globe::stage(globe_sender),
        globe_drawn: None,
        _watch: watch,
    })
}

/// The block's deck as a frame draws it ([`Player::block_drawn`]).
struct BlockDrawn {
    /// Its shapes.
    quads: Vec<ronoctua_render::quad::Quad>,
    /// Its words.
    spans: Vec<ronoctua_render::Span>,
    /// Its lanes' bar's track, while they scroll.
    bar: Option<crate::chrome::Rect>,
}

/// The deck before anything was laid out: the bar of nothing, no room.
fn blank_deck() -> crate::player_timeline::Deck {
    crate::player_timeline::of_state(&[], None, crate::player_timeline::Room::wide(0), 0)
}

/// A timer armed on the loop of the door the player is in — see [`wired`].
type Arm<T> = Box<dyn Fn() -> T>;

/// What this door keeps for ONE pointer, past what every door keeps (the
/// list's bar, the wheel's leftover, the row under it) — see `ronoctua_shell_kit::seats`.
#[derive(Debug, Default)]
pub struct PerPointer {
    /// What a press on a DECK holds — the block's, or a row's — until it is
    /// let go (`player_timeline::Grip`), where that deck starts in the
    /// surface's cells, and which deck it is.
    deck: Option<(crate::player_timeline::Grip, f64, DeckAt)>,
    /// Dragging the bar of a row — the position's or the volume's — by its
    /// row.
    sliding: Option<usize>,
    /// The clicks on the picture, one after the other: a second soon after
    /// the first is a double click — the middle maximizes, a side seeks —
    /// and more on the same side seek again (`player_media::PictureClicks`).
    picture_clicked: crate::player_media::PictureClicks,
    /// The transport button under the pointer.
    button_under: Option<&'static str>,
    /// The CONTROL of a row under the pointer — a title's glyph — by the
    /// command id whose sentence line 0 says (`player_rows::control_under`).
    control_under: Option<&'static str>,
    /// A press on the GLOBE of a directory's page, held: where it was pressed,
    /// where the hand was last, and whether it has travelled far enough to
    /// be a drag that turns it rather than a click on a group of marks
    /// (`atlas.clickSlack`).
    globe: Option<GlobeHeld>,
}

/// A group of the globe's marks as a frame drew it: its members, the cell of
/// the surface its centre is in, and the point itself in logical pixels
/// ([`Player::globe_marks_at`]).
pub type MarkDrawn = (Vec<u32>, (u16, u16), (f64, f64));

/// A press on the globe, held ([`PerPointer::globe`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct GlobeHeld {
    /// Where it was pressed, in the surface's logical pixels.
    at: (f64, f64),
    /// Where the hand was last.
    last: (f64, f64),
    /// It travelled past the slack: a drag.
    dragging: bool,
}

/// Which deck of this door a pointer is on: the BLOCK under the transport,
/// or the rows of a page that are one (the timeline's) — by the row's place
/// among the rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeckAt {
    /// The block under the transport.
    Block,
    /// Row `n` of the list.
    Row(usize),
}

/// What a player with NO DAEMON behind it keeps instead of acting — a
/// photograph's (`crate::player_photograph`, [`Player::detached`]). Everything a
/// click would have done past the door is here, to be said.
#[derive(Debug)]
pub struct Outbox {
    /// What it would have asked the player, in order.
    pub asked: Vec<Ask>,
    /// The switches of the preferences it would have turned, in order —
    /// each followed, in `asked`, by the `reload` that tells the daemon, as
    /// on a screen.
    pub kept: Vec<crate::player::Switch>,
    /// The machines it would have paired with, by name. NEVER THE CODE: it
    /// leaves for the player and nowhere else (`player_machines::pair_then`).
    pub paired_with: Vec<String>,
    /// The windows of a stage it would have opened — a column's whole
    /// screen or corner, which is a process of its own.
    pub stages: Vec<crate::player::Show>,
    /// Whether it would have ended the player's other doors that are
    /// processes — closing the player everywhere.
    pub ended_the_doors: bool,
    /// Whether it would have started an older player again.
    pub restarted: bool,
    /// The windows "show on every screen" would have opened — the screens
    /// something goes to, each press — or, pressed again, closed.
    pub every_screen: Vec<Vec<String>>,
    /// Whether it would have held the sound outputs (`player::SinksHeld`):
    /// a row of them was on screen.
    pub sinks_held: bool,
    /// The consents it would have written into `player.json`, in order: the
    /// services, the directories, let or no longer.
    pub lets: Vec<(
        Vec<ronoctua_lookup::Service>,
        Vec<ronoctua_player::find::Directory>,
        bool,
    )>,
    /// The person's keys it would have kept, by which — NEVER WHAT: a key
    /// leaves for its private file and nowhere else.
    pub keys_kept: Vec<ronoctua_lookup::keys::Key>,
    /// The keys it would have forgotten.
    pub keys_forgotten: Vec<ronoctua_lookup::keys::Key>,
    /// The addresses of the website page it would have kept or forgotten.
    pub websites: Vec<crate::player_website::Change>,
    /// The hosts it would have let in the consent file, or no longer.
    pub hosts_let: Vec<(Vec<String>, bool)>,
    /// How a password would have been answered — a photograph's `--pam`:
    /// the adult catalogue opened, or refused with that reason. `None`
    /// answers nothing, and says so.
    pub pam: Option<Result<(), ronoctua_pam::Refusal>>,
    /// The passwords it would have checked — how many, never what.
    pub passwords_checked: usize,
    /// The player's catalogue the view reads — a photograph's `--catalog`.
    pub catalog: Option<std::path::PathBuf>,
    /// The video catalogue the ranking reads — a photograph's `--ranking`
    /// (`ronoctua_shell_video_metrics::player_ranking`); `None` reads none, and the page says so.
    pub ranking: Option<std::path::PathBuf>,
    /// What it would have done to videos of the ranking — hidden, deleted,
    /// downloaded — in order.
    pub ranked: Vec<crate::player_seam::ranking::Act>,
    /// The addresses it would have opened in the Chrome kiosk.
    pub kiosk: Vec<String>,
    /// The stills standing in for the daemon's frames, by the LANE whose
    /// picture each is.
    pub stills: std::collections::BTreeMap<u32, ronoctua_render::image::Rgba>,
    /// A still for every picture that has none of its own.
    pub every: Option<ronoctua_render::image::Rgba>,
    /// Every lane a picture was laid out for — so a still no picture took
    /// is said, and not quietly dropped.
    pub laid_out: std::collections::BTreeSet<u32>,
    /// The photograph's clock. It MOVES ONLY WHEN TOLD ([`Outbox::wait`]):
    /// two clicks are a double click by what the walk says, never by how
    /// long a step took on this machine.
    now: std::time::Instant,
}

impl Default for Outbox {
    fn default() -> Self {
        Self {
            asked: Vec::new(),
            kept: Vec::new(),
            paired_with: Vec::new(),
            stages: Vec::new(),
            ended_the_doors: false,
            restarted: false,
            every_screen: Vec::new(),
            sinks_held: false,
            lets: Vec::new(),
            keys_kept: Vec::new(),
            keys_forgotten: Vec::new(),
            websites: Vec::new(),
            hosts_let: Vec::new(),
            pam: None,
            passwords_checked: 0,
            catalog: None,
            ranking: None,
            ranked: Vec::new(),
            kiosk: Vec::new(),
            stills: std::collections::BTreeMap::new(),
            every: None,
            laid_out: std::collections::BTreeSet::new(),
            // A BASE, and never read as a time of day: only what `wait`
            // adds to it is.
            now: std::time::Instant::now(),
        }
    }
}

impl Outbox {
    /// Time passes on the photograph's clock.
    pub fn wait(&mut self, by: std::time::Duration) {
        self.now += by;
    }

    /// An errand, kept: what would have reached the daemon or the person's
    /// preferences is written down and said as the door says it when the
    /// daemon took it; what stays in the door is handed back, as
    /// `player::carry_out` hands it — the footer's door on a photograph's
    /// doorway keeps its errands the same way (`player_region`).
    pub(crate) fn carry_out(
        &mut self,
        errand: crate::player::Errand,
        texts: &ronoctua_i18n::Texts,
    ) -> Result<Option<String>, crate::player::Errand> {
        use crate::player::Errand;
        match errand {
            Errand::Nothing => Ok(None),
            Errand::Then(first, then) => {
                let said = self.carry_out(*first, texts)?;
                Ok(self.carry_out(*then, texts)?.or(said))
            }
            Errand::Restart => {
                self.restarted = true;
                Ok(Some(texts.t(crate::key::PLAYER_RESTARTING)))
            }
            Errand::Said(sentence) => Ok(Some(sentence)),
            Errand::Tell(ask) => {
                self.asked.push(ask);
                Ok(None)
            }
            Errand::TellSaying(ask, sentence) => {
                self.asked.push(ask);
                Ok(Some(sentence))
            }
            Errand::Keep(switch) => {
                self.kept.push(switch);
                self.asked.push(Ask::Reload);
                Ok(None)
            }
            Errand::Let {
                services,
                directories,
                on,
                then,
            } => {
                self.lets.push((services, directories, on));
                self.carry_out(*then, texts)
            }
            Errand::KeepKey { key, .. } => {
                self.keys_kept.push(key);
                self.asked.push(Ask::KeyCheck {
                    service: key.service(),
                });
                Ok(Some(texts.t_with(
                    crate::key::PLAYER_KEY_CHECKING,
                    &[(
                        "service",
                        &texts.t(crate::player::service_key(key.service())),
                    )],
                )))
            }
            Errand::KeepAccount { .. } => {
                let key = ronoctua_lookup::keys::Key::OpenSubtitlesAccount;
                self.keys_kept.push(key);
                self.asked.push(Ask::KeyCheck {
                    service: key.service(),
                });
                Ok(Some(texts.t_with(
                    crate::key::PLAYER_KEY_CHECKING,
                    &[(
                        "service",
                        &texts.t(crate::player::service_key(key.service())),
                    )],
                )))
            }
            Errand::ForgetKey(key) => {
                self.keys_forgotten.push(key);
                Ok(Some(texts.t(crate::key::PLAYER_KEY_FORGOTTEN)))
            }
            // A TRANSLATION KEY is the machine's: a photograph keeps none,
            // and says what the door would.
            Errand::KeepWordsKey { key, .. } => {
                Ok(Some(crate::player_translator::kept_said(key, texts)))
            }
            Errand::ForgetWordsKey(_) => Ok(Some(texts.t(crate::key::PLAYER_KEY_FORGOTTEN))),
            Errand::Websites(change) => {
                self.websites.push(change);
                Ok(None)
            }
            Errand::LetHosts { hosts, on, then } => {
                self.hosts_let.push((hosts, on));
                self.carry_out(*then, texts)
            }
            // A VIDEO OF THE RANKING: kept, and said as the door says it once
            // it is done.
            Errand::Ranking(act) => {
                let said =
                    crate::player_seam::said(&crate::player::Told::Ranking(act.done()), texts);
                self.ranked.push(act);
                Ok(said)
            }
            Errand::Kiosk(url) => {
                self.kiosk.push(url);
                Ok(None)
            }
            errand @ (Errand::Close
            | Errand::Unlock(_)
            | Errand::Show(_)
            | Errand::Swap
            | Errand::QuitEverywhere
            | Errand::EveryScreen
            | Errand::Paste
            | Errand::FoldPicture
            | Errand::Ask(_)
            | Errand::Pair { .. }) => Err(errand),
        }
    }
}

/// WHERE WHAT THE PLAYER DOES GOES.
///
/// Every act that leaves the door — an ask of the daemon, the preferences
/// written, a pairing, a window for the stage, a question to the
/// compositor, a thread asking for a picture — is a method of this, and
/// nothing else in this file calls the functions that act. So a detached
/// player has NO ROUTE to the machine. It used to be one `Option` checked at
/// six places, and a seventh call would have gone around all six — acting
/// on the person's own daemon from a test, or a photograph pausing their
/// film.
enum Reach {
    /// The machine's: the daemon, the compositor, the stage's window.
    Machine {
        /// The compositor, through the contract — opened the first time it
        /// is needed, and again after one failed.
        wm: Option<Box<dyn ronoctua_compositor::Compositor>>,
    },
    /// Nobody's: what it would have done is kept — a photograph's. Boxed:
    /// the stills are a photograph's, and a window on a screen carries none.
    Kept(Box<Outbox>),
}

impl Reach {
    /// The machine, with the compositor not yet opened.
    const fn machine() -> Self {
        Self::Machine { wm: None }
    }

    /// Tells the player `ask`, off the loop: how it ended comes back down
    /// `told` (`player::tell_then`).
    fn tell(&mut self, ask: &Ask, told: &crate::player::ToldSender) {
        match self {
            Self::Machine { .. } => {
                let told = told.clone();
                crate::player::tell_then(ask.clone(), move |answer| told.send(answer));
            }
            Self::Kept(out) => out.asked.push(ask.clone()),
        }
    }

    /// Tells the player `ask` ONLY IF IT RUNS, and does not wait for its
    /// word (`player::tell_if_running`).
    fn tell_if_running(&mut self, ask: &Ask) {
        match self {
            Self::Machine { .. } => crate::player::tell_if_running(ask),
            Self::Kept(out) => out.asked.push(ask.clone()),
        }
    }

    /// Ends the player's other doors that are processes — the column, every
    /// other window (`player::end_the_doors`).
    fn end_the_doors(&mut self) {
        match self {
            Self::Machine { .. } => crate::player::end_the_doors(),
            Self::Kept(out) => out.ended_the_doors = true,
        }
    }

    /// Carries an errand out as far as the door's edge — see
    /// `player::carry_out` — and hands back what is the door's.
    fn carry_out(
        &mut self,
        errand: crate::player::Errand,
        texts: &ronoctua_i18n::Texts,
        told: &crate::player::ToldSender,
    ) -> Result<Option<String>, crate::player::Errand> {
        match self {
            Self::Machine { .. } => crate::player::carry_out(errand, texts, Some(told)),
            Self::Kept(out) => out.carry_out(errand, texts),
        }
    }

    /// Pairs with `to`, off the loop, and says how it ended on `paired`.
    fn pair(
        &mut self,
        to: String,
        code: crate::player_machines::Code,
        paired: channel::Sender<crate::player_machines::Paired>,
    ) {
        match self {
            Self::Machine { .. } => crate::player_machines::pair_then(to, code, move |p| {
                let _ = paired.send(p);
            }),
            Self::Kept(out) => out.paired_with.push(to),
        }
    }

    /// Opens — or closes, pressed again — the window of a column's stage.
    fn stage_window(&mut self, show: crate::player::Show) -> std::io::Result<()> {
        match self {
            Self::Machine { .. } => crate::player_stage::toggle(show),
            Self::Kept(out) => {
                out.stages.push(show);
                Ok(())
            }
        }
    }

    /// The compositor, opened when it is not — and none for a kept player,
    /// whose window is in no compositor's tree.
    fn compositor(&mut self) -> Option<&mut Box<dyn ronoctua_compositor::Compositor>> {
        match self {
            Self::Machine { wm } => {
                if wm.is_none() {
                    *wm = crate::daemon::open_the_compositor().ok();
                }
                wm.as_mut()
            }
            Self::Kept(_) => None,
        }
    }

    /// The compositor's connection failed: it is opened again next time.
    fn forget_the_compositor(&mut self) {
        if let Self::Machine { wm } = self {
            *wm = None;
        }
    }

    /// Lane `lane`'s picture: a thread that asks the daemon for its frames
    /// and pings `frames` with the lane's number on each — or, kept, the
    /// still that stands in for them.
    fn picture(
        &mut self,
        lane: ronoctua_player::own::LaneId,
        frames: &channel::Sender<usize>,
    ) -> crate::player_picture::Picture {
        match self {
            Self::Machine { .. } => {
                let k = usize::try_from(lane.0).unwrap_or(usize::MAX);
                let frames = std::sync::Mutex::new(frames.clone());
                crate::player_picture::Picture::start(Some(lane), move || {
                    frames.lock().is_ok_and(|f| f.send(k).is_ok())
                })
            }
            Self::Kept(out) => {
                out.laid_out.insert(lane.0);
                let still = out.stills.get(&lane.0).or(out.every.as_ref()).cloned();
                crate::player_picture::Picture::still(Some(lane), still)
            }
        }
    }

    /// "Show on every screen": `connected` screens are known by name, and
    /// something goes to `routed` — the windows opened, or those up closed
    /// (`player_stage::everywhere`). Kept, what it would have opened.
    fn every_screen(
        &mut self,
        connected: usize,
        routed: &[String],
    ) -> std::io::Result<crate::player_stage::Everywhere> {
        match self {
            Self::Machine { .. } => crate::player_stage::everywhere(connected, routed),
            Self::Kept(out) => {
                let plan = crate::player_stage::everywhere_plan(0, connected, routed);
                if let crate::player_stage::Everywhere::Opened(screens) = &plan {
                    out.every_screen.push(screens.clone());
                }
                Ok(plan)
            }
        }
    }

    /// The sound outputs held while `wanted` — a row of them on screen — and
    /// let go when not ([`crate::player::SinksHeld`]). Kept, whether it would
    /// have held them.
    fn sinks(&mut self, wanted: bool, held: &mut Option<crate::player::SinksHeld>) {
        match self {
            Self::Machine { .. } => match (wanted, held.is_some()) {
                (true, false) => *held = Some(crate::player::hold_sinks()),
                (false, true) => *held = None,
                _ => {}
            },
            Self::Kept(out) => out.sinks_held |= wanted,
        }
    }

    /// Now: the machine's clock, or the photograph's.
    fn now(&self) -> std::time::Instant {
        match self {
            Self::Machine { .. } => std::time::Instant::now(),
            Self::Kept(out) => out.now,
        }
    }

    /// Whether this is a photograph's.
    const fn is_kept(&self) -> bool {
        matches!(self, Self::Kept(_))
    }
}

/// The player, as this door shows it.
pub struct Player {
    door: Door,
    /// Arms the timer that takes the controls away — see [`wired`].
    arm_controls: Arm<Option<calloop::RegistrationToken>>,
    /// Arms the timer that asks for the corner again.
    arm_corner: Arm<bool>,
    /// The window, kept to act on it — made whole-screen or small. A CLONE of
    /// what the door holds (both are handles onto one object).
    window: RefCell<Option<Window>>,
    /// Where what it does goes: the machine, or — with no daemon behind it
    /// — an outbox. See [`Reach`].
    reach: Reach,
    view: crate::player::View,
    /// A pairing's name, between its two questions.
    pairing: crate::player_machines::Pairing,
    /// Where the thread that pairs says how it ended.
    paired: channel::Sender<crate::player_machines::Paired>,
    /// Where an ask told off the loop says how it ended.
    told: crate::player::ToldSender,
    player: Option<State>,
    /// A picture per lane shown on this door's screen, each with its thread.
    pictures: Vec<crate::player_picture::Picture>,
    /// Where each picture was PLACED this frame — logical pixels of the
    /// surface, in the order drawn: what the painter draws and a click
    /// reads (`player_layout::placed`).
    placed: ronoctua_player::arrange::Placement,
    /// The screen a window opened for ONE screen shows (`player maximized
    /// --screen`): what it composes and where it goes whole-screen.
    screen_wanted: Option<String>,
    /// The screen the column is on, as its base says it
    /// (`Tenant::placed_on`) — ahead of the surface's own `enter`.
    placed_on: Option<String>,
    /// The sound outputs, held while a row of them is on screen.
    sinks: Option<crate::player::SinksHeld>,
    /// The screens a photograph's door knows and the one it is on
    /// ([`Player::given_screens`]) — `None` on a screen, where the door's
    /// own connection says them.
    screens_given: Option<(Vec<crate::player_lane::Known>, Option<String>)>,
    frame_sender: channel::Sender<usize>,
    /// The transport as last DRAWN: `(line, first column, last column, id)`
    /// — the seeks around the clock among them (`player_media::transport`).
    buttons: Vec<(u16, u16, u16, &'static str)>,
    /// Where the clock was last drawn — its line and first cell — between
    /// its seeks, or alone where they do not fit.
    clock: Option<(u16, u16)>,
    /// THE PICTURE FOLDED AWAY (`z`, the glyph on "Now"): this door's own,
    /// never written anywhere — the rows take the band's lines.
    picture_folded: bool,
    /// Where everything was last laid out.
    layout: Layout,
    /// The DECK under the transport as last laid out — the time bar, or a
    /// set's ruler and lanes (`crate::player_timeline`): what the painter
    /// drew and what the pointer reads back.
    deck: crate::player_timeline::Deck,
    /// The first lane the block shows, when a set has more than it holds.
    deck_offset: usize,
    /// A band being DRAGGED — where its ghost would land — until it is let
    /// go, or `Esc` calls it off.
    dragged: Option<crate::player_timeline::Held>,
    /// The control of a deck under the pointer that spoke last: its glyph
    /// lights, and line 0 says what it does.
    deck_under: Option<(DeckAt, usize, crate::player_timeline::Hit)>,
    /// The lane the BLOCK's deck has chosen — a first click on its band or its
    /// name — which wears the cursor's colour there; a second click on it
    /// opens its page (invariant 19). The rows' decks choose by the cursor.
    deck_chosen: Option<ronoctua_player::own::LaneId>,
    /// How the picture is shown.
    stage: Stage,
    /// The stage to be set on the first turn, once the window exists.
    first_stage: Option<Stage>,
    /// The stage this window was OPENED for, by a door without a window of
    /// its own (`ronoctua-shell player maximized|pip|only`).
    opened_for: Option<Stage>,
    /// On stage, when the controls over the picture go.
    controls_until: Option<std::time::Instant>,
    /// The timer that takes them away, armed only while they are shown.
    controls_timer: Option<calloop::RegistrationToken>,
    /// The window was made small on stage and the compositor has not yet
    /// been told to float it in the corner — a window it has not mapped is
    /// not in its tree to be found.
    corner_owed: u8,
    /// Whether the compositor last configured the window whole-screen — so
    /// leaving it by another way is noticed (`Content::window_configured`).
    fullscreen: bool,
    /// A timer is armed to ask for the corner again.
    corner_timer: bool,
    /// A frame was drawn since the last turn.
    painted: bool,
    /// The transport button under the pointer that spoke last — what the
    /// painter lights, and what line 0 explains.
    button_under: Option<&'static str>,
    /// Where the pointer that spoke last is, while it is over the surface:
    /// between an `Enter` and a `Leave`. A position alone cannot say it —
    /// before the first `Enter` it was `(0, 0)`, which is the header's
    /// corner, and the controls stayed up for good.
    pointer_at: Option<(f64, f64)>,
    /// The thread that draws the GLOBE of a directory's page — `None` in a
    /// detached player, which draws it in place (`View::globe_drawn_here`).
    globe_stage: Option<crate::globe::Stage>,
    /// Where the globe's picture was drawn this frame, and its size in
    /// pixels: what a pointer on it is read back through
    /// (`player_globe::picture_px`).
    globe_drawn: Option<(ronoctua_render::image::Area, (u32, u32))>,
    /// The thread watching the player's state.
    _watch: Arc<AtomicBool>,
}

impl Drop for Player {
    fn drop(&mut self) {
        // THE PICTURES' THREADS go with the pictures (`player_picture`), and
        // the column's slot with the column (`crate::column`).
        if self.opened_for.is_some() && self.screen_wanted.is_none() {
            crate::player_stage::leave();
        }
    }
}

impl Player {
    /// The player with NO DAEMON BEHIND IT, in `door`: nothing is started
    /// and nothing watched, the state arrives by [`Player::state_arrived`] —
    /// the path the watch thread's own states take — and everything that
    /// would leave the door is kept in `outbox` instead ([`Reach::Kept`]).
    /// What `crate::player_photograph` draws: a door that could reach a daemon
    /// would pause the person's film on a click.
    ///
    /// Its file browser's places are under `places` (`Music`, `Videos`),
    /// and the machine's own folders only when that is `None`.
    #[must_use]
    pub fn detached(door: Door, outbox: Outbox, places: Option<std::path::PathBuf>) -> Self {
        let (frame_sender, _) = channel::channel::<usize>();
        let (paired, _) = channel::channel::<crate::player_machines::Paired>();
        let (told, _) = channel::channel::<crate::player::Told>();
        // THE PREFERENCES' DEFAULTS, not the person's: a photograph is the
        // same picture on every machine — and the catalogue it was handed.
        let mut view = crate::player::View::blank(places);
        view.detached_pages(&outbox);
        Self {
            door,
            // NO LOOP TO ARM A TIMER ON: the controls stay until `rest`.
            arm_controls: Box::new(|| None),
            arm_corner: Box::new(|| false),
            window: RefCell::new(None),
            reach: Reach::Kept(Box::new(outbox)),
            view,
            pairing: crate::player_machines::Pairing::default(),
            paired,
            told: crate::player::ToldSender::of_channel(told),
            player: None,
            pictures: Vec::new(),
            placed: Vec::new(),
            screen_wanted: None,
            placed_on: None,
            sinks: None,
            screens_given: None,
            frame_sender,
            buttons: Vec::new(),
            clock: None,
            picture_folded: false,
            layout: Layout::default(),
            deck: blank_deck(),
            deck_offset: 0,
            dragged: None,
            deck_under: None,
            deck_chosen: None,
            stage: Stage::Off,
            first_stage: None,
            opened_for: None,
            controls_until: None,
            controls_timer: None,
            corner_owed: 0,
            fullscreen: false,
            corner_timer: false,
            painted: false,
            button_under: None,
            pointer_at: None,
            // NO THREAD: the globe is drawn in place, as the frame is.
            globe_stage: None,
            globe_drawn: None,
            _watch: Arc::new(AtomicBool::new(false)),
        }
    }

    /// What a detached player kept instead of acting — `None` for a player
    /// on a screen, which acted.
    #[must_use]
    pub const fn outbox(&self) -> Option<&Outbox> {
        match &self.reach {
            Reach::Kept(out) => Some(&**out),
            Reach::Machine { .. } => None,
        }
    }

    /// Whether its file browser is open.
    #[must_use]
    pub fn browsing(&self) -> bool {
        self.view.browsing()
    }

    /// What a detached player would have asked, in order — nothing for a
    /// player on a screen. For a test.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn asked(&self) -> &[Ask] {
        self.outbox().map_or(&[], |out| out.asked.as_slice())
    }

    /// Time passes on a detached player's clock — between two steps of a
    /// photograph's walk. A player on a screen has the machine's.
    pub fn wait(&mut self, by: std::time::Duration) {
        if let Reach::Kept(out) = &mut self.reach {
            out.wait(by);
        }
    }

    /// The stage, set as a key sets it — see [`Player::set_stage`].
    pub fn stage_to(&mut self, ui: &mut Ui, to: Stage) {
        self.set_stage(ui, to);
    }

    /// The window OPENED for the listing's stage on what a door handed over
    /// — a photograph's `--handed` — as `wired` opens one, and its first turn
    /// takes it: the stage, and the password an adult listing waits for.
    pub fn opened_on(&mut self, ui: &mut Ui, open: impl FnOnce(&mut crate::player::View)) {
        open(&mut self.view);
        self.view
            .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
        self.opened_for = Some(Stage::Listing);
        self.first_stage = Some(Stage::Listing);
        self.each_turn_here(ui);
    }

    /// The controls' time runs out, as their timer's last ring takes them —
    /// on stage, the picture alone. A detached player has no loop to ring
    /// that timer, so this is how a photograph shows the stage nobody has
    /// touched.
    pub fn rest(&mut self, ui: &mut Ui) {
        self.controls_until = None;
        self.button_under = None;
        ui.ask_paint();
    }

    /// Whether the picture is shown alone — the listing's stage is rows, and
    /// no picture's.
    fn on_stage(&self) -> bool {
        matches!(self.stage, Stage::Only | Stage::Maximized | Stage::Small)
    }

    /// Whether the transport and the header are drawn — always off stage,
    /// and on stage for a while after activity.
    fn controls_shown(&self) -> bool {
        // A DETACHED PLAYER HAS NO TIMER to take them: they stay until
        // `rest` does, so a picture taken on a slow machine is the picture
        // taken on a fast one.
        !self.on_stage()
            || self
                .controls_until
                .is_some_and(|u| self.reach.is_kept() || std::time::Instant::now() < u)
    }

    /// Whether the pointer rests on the controls drawn over the picture.
    fn pointer_on_controls(&self, ui: &Ui) -> bool {
        let Some((_, y)) = self.pointer_at else {
            return false;
        };
        if y < 0.0 {
            return false;
        }
        let l = line_at(y);
        l < ui.first_of_list() || l >= self.layout.transport
    }

    /// The controls' timer rang: again later while they are to stay — the
    /// hand resting on them keeps them — and otherwise they go.
    fn controls_tick(
        &mut self,
        ui: &mut Ui,
        here: &mut Hand<PerPointer>,
    ) -> calloop::timer::TimeoutAction {
        let now = std::time::Instant::now();
        if self.on_stage() && self.pointer_on_controls(ui) {
            self.controls_until = Some(now + CONTROLS_LINGER);
        }
        match self.controls_until {
            Some(until) if self.on_stage() && until > now => {
                calloop::timer::TimeoutAction::ToDuration(until - now)
            }
            _ => {
                self.controls_until = None;
                self.controls_timer = None;
                self.button_under = None;
                here.extra.button_under = None;
                ui.ask_paint();
                calloop::timer::TimeoutAction::Drop
            }
        }
    }

    /// Activity: on stage, the controls come back over the picture and a
    /// timer is armed to take them away — one timer, for as long as they
    /// are shown, and none at rest.
    fn wake_controls(&mut self, ui: &mut Ui) {
        if !self.on_stage() {
            return;
        }
        if !self.controls_shown() {
            ui.ask_paint();
        }
        self.controls_until = Some(std::time::Instant::now() + CONTROLS_LINGER);
        if self.controls_timer.is_some() {
            return;
        }
        self.controls_timer = (self.arm_controls)();
    }

    /// Shows the picture alone, on the whole screen, small in the corner, or
    /// back in the door.
    ///
    /// A column is a strip of the screen and can neither fill it nor float:
    /// there, the whole screen and the corner are the WINDOW, opened for it —
    /// and a second press closes that window instead of opening another
    /// (`crate::player_stage`).
    ///
    /// A window OPENED for a stage is that stage and nothing else: leaving
    /// it closes the window, which is the way back to the door that asked.
    fn set_stage(&mut self, ui: &mut Ui, to: Stage) {
        if self.door == Door::Column
            && matches!(to, Stage::Maximized | Stage::Small | Stage::Listing)
        {
            let show = match to {
                Stage::Small => crate::player::Show::Small,
                Stage::Listing => crate::player::Show::Listing,
                _ => crate::player::Show::Maximized,
            };
            if let Err(e) = self.reach.stage_window(show) {
                tracing::warn!(error = %e, "could not open the player's window for its stage");
            }
            return;
        }
        // A HAND-OVER LEFT BEHIND is nobody's once this window takes the
        // listing's stage itself — a footer's `F` that closed a listing's
        // window wrote one no window read.
        if to == Stage::Listing
            && self.opened_for.is_none()
            && matches!(self.reach, Reach::Machine { .. })
        {
            crate::player_seam::hand_back();
        }
        // LEAVING A STAGE the window was not opened for goes back to the one
        // it was — the small window made whole-screen and back is the small
        // window again.
        let to = match self.opened_for {
            Some(home) if to == Stage::Off && self.stage != home => home,
            Some(_) if to == Stage::Off => {
                ui.quit = true;
                return;
            }
            _ => to,
        };
        let window = self.window.borrow().clone();
        if let Some(w) = &window {
            if matches!(to, Stage::Maximized | Stage::Listing) {
                // ON ITS OWN SCREEN, for a window "show on every screen"
                // opened there — xdg-shell's own argument — and the
                // compositor's choice for every other.
                let output = self
                    .screen_wanted
                    .as_deref()
                    .and_then(|c| ui.screen_named(c));
                w.set_fullscreen(output.as_ref());
            } else if matches!(self.stage, Stage::Maximized | Stage::Listing) {
                w.unset_fullscreen();
            }
        }
        // THE CORNER is the compositor's to give: floated, sized and placed
        // there, and put back in the layout on the way out.
        //
        // A FLOATING WINDOW'S SIZE IS THE ONE ITS COMMITS CARRY: the
        // compositor adopts it. The window kept committing the tiled size it
        // had been configured, and the corner came out floated, placed, and
        // 512 by 768 — measured in the VM, however many times the corner was
        // asked again. So on this stage the window has to own its size, and
        // tell it as its minimum and maximum.
        //
        // Both halves: it DRAWS at its own size (`Content::own_size`, which
        // the door lays out again at once — `Ui::retake_the_size`), and tells
        // it as its minimum and maximum.
        let small = Some(small_size());
        if to == Stage::Small && self.stage != Stage::Small {
            if let Some(w) = &window {
                w.set_min_size(small);
                w.set_max_size(small);
            }
            self.corner_owed = CORNER_TRIES;
            self.float_in_the_corner();
        } else if to != Stage::Small && self.stage == Stage::Small {
            if let Some(w) = &window {
                w.set_min_size(None);
                w.set_max_size(None);
            }
            self.corner_owed = 0;
            self.with_my_window(|wm, id| wm.back_in_the_layout(id));
        }
        self.stage = to;
        ui.retake_the_size();
        ui.offset = 0;
        self.controls_until = None;
        self.wake_controls(ui);
        ui.ask_paint();
    }

    /// Asks the compositor for the corner; while it cannot find this window
    /// — not yet mapped — the ask is owed, and tried again on a TIMER.
    ///
    /// Not after the next frame: the frame that maps the window is the last
    /// one a paused picture paints, and the ask made right after it reaches
    /// the compositor before the commit does. With nothing playing no frame
    /// follows, and the window stayed in the layout — the VM caught it.
    fn float_in_the_corner(&mut self) {
        // A DETACHED WINDOW HAS NO COMPOSITOR TO FLOAT IT: nothing is owed,
        // and no turn of a photograph's walk counts the tries down to a
        // warning about a compositor that was never there.
        if self.reach.is_kept() {
            self.corner_owed = 0;
        }
        if self.corner_owed == 0 {
            return;
        }
        let corner = ronoctua_compositor::Corner {
            width: u32::from(tokens::space::PLAYER_PIPWIDTH),
            height: u32::from(tokens::space::PLAYER_PIPHEIGHT),
            air: u32::from(tokens::space::PLAYER_PIPAIR),
        };
        if self.with_my_window(|wm, id| wm.float_in_corner(id, corner)) {
            self.corner_owed = 0;
            tracing::info!("the window was floated in the corner");
            return;
        }
        self.corner_owed -= 1;
        if self.corner_owed == 0 {
            // SAID, because a small window that stayed big has no other way
            // of telling anyone why.
            tracing::warn!("the compositor never showed this window to float it in the corner");
            return;
        }
        if self.corner_timer {
            return;
        }
        self.corner_timer = (self.arm_corner)();
    }

    /// Runs `ask` on this window, found in the compositor's tree by this
    /// process and the app id; whether it was found and the ask went.
    fn with_my_window(
        &mut self,
        ask: impl FnOnce(&mut dyn ronoctua_compositor::Compositor, u64) -> Result<()>,
    ) -> bool {
        let Some(wm) = self.reach.compositor() else {
            return false;
        };
        let me = i32::try_from(std::process::id()).ok();
        let Some(id) = wm.windows().ok().and_then(|all| {
            all.into_iter()
                .find(|w| w.pid == me && w.app_id.as_deref() == Some(APP_ID))
                .map(|w| w.id)
        }) else {
            return false;
        };
        match ask(wm.as_mut(), id) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(error = %e, "the compositor refused the window's place");
                // A CONNECTION THAT FAILED is opened again next time.
                self.reach.forget_the_compositor();
                false
            }
        }
    }

    /// A press of `o`, `i` or `m`, or of their buttons: on, or back off.
    fn toggle_stage(&mut self, ui: &mut Ui, show: crate::player::Show) {
        let to = match show {
            crate::player::Show::Only => Stage::Only,
            crate::player::Show::Maximized => Stage::Maximized,
            crate::player::Show::Small => Stage::Small,
            crate::player::Show::Listing => Stage::Listing,
        };
        self.set_stage(ui, if self.stage == to { Stage::Off } else { to });
    }

    /// Where the pictures go on the surface, in logical pixels: the band the
    /// layout gave them, or on stage the whole surface from `area`'s top —
    /// under a column's control line. `None` with no band.
    fn band_area(&self, ui: &Ui, area: &Area) -> Option<ronoctua_player::arrange::Rect> {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        let a = if self.on_stage() {
            stage_area(w, h, area)
        } else {
            let (first, lines) = self.layout.band?;
            ronoctua_render::image::Area {
                x: 0.0,
                y: line_px() * f32::from(first),
                width: w,
                height: line_px() * f32::from(lines),
            }
        };
        Some(ronoctua_player::arrange::Rect {
            x: a.x,
            y: a.y,
            width: a.width,
            height: a.height,
        })
    }

    /// THE SCREEN THIS DOOR SHOWS: the one a window was opened for (`player
    /// maximized --screen`), else the column's — its base says it, ahead of
    /// the surface — else the one its surface entered. `None` is unknown,
    /// which shows every picture (`player_layout::tiles_on`).
    fn screen(&self, ui: &Ui) -> Option<String> {
        if let Some(c) = &self.screen_wanted {
            return Some(c.clone());
        }
        if let Some((_, here)) = &self.screens_given {
            return here.clone();
        }
        match self.door {
            Door::Column => self
                .placed_on
                .clone()
                .or_else(|| ui.screens.here().map(ToOwned::to_owned)),
            Door::Window | Door::Footer => ui.screens.here().map(ToOwned::to_owned),
        }
    }

    /// The screens this door knows BY NAME — what a lane's page routes a
    /// picture to.
    fn known(&self, ui: &Ui) -> Vec<crate::player_lane::Known> {
        if let Some((known, _)) = &self.screens_given {
            return known.clone();
        }
        ui.screens
            .all()
            .iter()
            .filter_map(|s| {
                Some(crate::player_lane::Known {
                    connector: s.connector.clone()?,
                    description: s.description.clone(),
                })
            })
            .collect()
    }

    /// The screens moved — described, gone, the surface on another, the
    /// column placed elsewhere: the view is told, and what is composed here
    /// follows at the next frame.
    fn screens_follow(&mut self, ui: &mut Ui) {
        let (known, here) = (self.known(ui), self.screen(ui));
        if self.view.screens_are(known, here) {
            ui.ask_paint();
        }
        // A WINDOW OPENED FOR ONE SCREEN waited for that screen's name to go
        // whole-screen on it (`each_turn_here`).
        if self.first_stage.is_some() {
            self.each_turn_here(ui);
        }
    }

    /// The screens a PHOTOGRAPH's door knows — no compositor describes an
    /// output to a detached door — and the one it is on (`crate::player_photograph`).
    pub fn given_screens(
        &mut self,
        ui: &mut Ui,
        known: Vec<crate::player_lane::Known>,
        here: Option<String>,
    ) {
        self.screens_given = Some((known, here));
        self.screens_follow(ui);
    }

    /// What the player's watch heard: a state to show — none, and the door
    /// says the player is not running — or the farewell of a player closed
    /// everywhere, and the door closes (`player::hearing`).
    pub fn heard(&mut self, ui: &mut Ui, heard: ronoctua_player::client::Heard) {
        match crate::player::hearing(heard) {
            crate::player::Hearing::Shows(s) => {
                // GONE, and the queue it had with it — and what this door
                // held of ITS sheets and desks: a player that comes back
                // counts them from zero again, and says its own.
                if s.is_none() {
                    self.view.outline_arrived(None);
                    self.view.player_gone();
                }
                self.state_arrived(ui, s.map(|s| *s));
            }
            crate::player::Hearing::Outline(o) => self.outline_arrived(ui, Some(o)),
            crate::player::Hearing::Closes => self.leave(ui),
        }
    }

    /// THIS DOOR CLOSES because the player closed: its pictures let go now,
    /// and the loop ends — the column's `Drop` gives its slot up, a stage's
    /// window its own.
    fn leave(&mut self, ui: &mut Ui) {
        self.pictures.clear();
        ui.quit = true;
        ui.ask_paint();
    }

    /// A state from the player — the watch thread's, or a photograph's
    /// fixture (`crate::player_photograph`): the cursor follows what it was on.
    pub fn state_arrived(&mut self, ui: &mut Ui, s: Option<State>) {
        // A HOLD ON THE OUTPUTS WHOSE PLAYER WENT is no hold: the one that
        // speaks now is held again by the next frame that shows the rows.
        if self
            .sinks
            .as_ref()
            .is_some_and(crate::player::SinksHeld::ended)
        {
            self.sinks = None;
        }
        let spot = self
            .rows(ui)
            .get(self.view.cursor())
            .map(|r| r.spot.clone());
        self.player = s;
        if let Some(spot) = spot {
            self.view
                .follow(&spot, self.player.as_ref(), ui.columns, &ui.texts);
        }
        self.view
            .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// The sheet of the file the Metadata section shows — a photograph's
    /// fixture (`crate::player_photograph`, `--sheet`): the view holds it as a
    /// door's thread would have left it, its cover decoded.
    pub fn sheet_arrived(&mut self, ui: &mut Ui, sheet: ronoctua_tags::Sheet) {
        let location = sheet.location.to_string_lossy().into_owned();
        let generation = self.player.as_ref().map_or(0, |s| s.sheet_generation);
        // THE FILE AS WHAT PLAYS NAMES IT: the ring's own spelling, which is
        // what the section looks for.
        let location = self
            .player
            .as_ref()
            .map(crate::player_metadata::locations)
            .and_then(|all| {
                all.into_iter()
                    .find(|l| std::path::Path::new(l) == std::path::Path::new(&location))
            })
            .unwrap_or(location);
        self.view.meta.given(
            &location,
            generation,
            crate::player_metadata::Got::Sheet(Box::new(sheet)),
        );
        self.view
            .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// A desk's page — a photograph's fixture (`crate::player_photograph`,
    /// `--desk`): the view holds it as a door's thread would have left it,
    /// a suggested cover decoded.
    pub fn desk_arrived(&mut self, ui: &mut Ui, desk: ronoctua_player::find::Desk) {
        self.view.found.given(desk);
        self.view
            .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// A PAGE OPENED over the one on screen, as the row that opens it would
    /// (`photograph --open`): which row that is, and that every page has
    /// one, is `every_page_is_reached_by_a_row`'s to prove.
    pub fn open_page(&mut self, ui: &mut Ui, page: crate::player::PageId) {
        self.view.push(page, None);
        self.view
            .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// A TEXT OF SEVERAL LINES opened over the page, holding `words` — what
    /// a page's "edit here" opens (`player::View::edit_text`); a photograph
    /// opens one by this door's own path.
    pub fn edit_text(&mut self, ui: &mut Ui, words: &str) {
        self.view
            .edit_text(crate::text_area::TextArea::new("photograph", words));
        ui.ask_paint();
    }

    /// How an ask told off the loop ended, in this door: THE VIEW'S SHARE of
    /// it — a list refused at once is its page's to say, a key that was not
    /// kept read again, a refusal the person may overrule asked on the page
    /// — and the refusal said, which expires as every sentence does. A
    /// question the view asks says nothing more, and what the order said
    /// ("added…") goes: it was not.
    pub fn answer_told(&mut self, ui: &mut Ui, answer: &crate::player::Told) {
        let took = self.view.told(answer, &ui.texts);
        if took {
            self.view
                .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
            ui.ask_paint();
        }
        let older = self.player.as_ref().is_some_and(crate::player::older);
        match crate::player::heard(took, answer, older, &ui.texts) {
            // WHAT THE ORDER SAID ("added…") goes: it was not — and on a
            // stage the question's rows come back, for it is drawn there. A
            // window opened for its stage has none, and nothing that asks is
            // sent from it (`PLAYER_STAGE_HAS_NO_ROWS`).
            crate::player::Heard::Asked => {
                ui.message = None;
                if self.on_stage() && self.opened_for.is_none() {
                    self.set_stage(ui, Stage::Off);
                }
                ui.ask_paint();
            }
            crate::player::Heard::Said(Some(said)) => ui.say(said),
            crate::player::Heard::Said(None) => {}
        }
    }

    /// A refusal the person may overrule, as the player would answer the ask
    /// `again` overrules — a photograph's (`--overrule`): what the answer
    /// does in a door on a screen (`player::Told::Asks`).
    pub fn overrule_asked(
        &mut self,
        ui: &mut Ui,
        again: Ask,
        trouble: ronoctua_player::wire::Trouble,
    ) {
        self.answer_told(
            ui,
            &crate::player::Told::Asks {
                again: Box::new(again),
                trouble,
            },
        );
        ui.ask_paint();
    }

    /// The addresses of the website page played lately — a photograph's
    /// (`--websites`): never read from this machine's file.
    pub fn websites_given(&mut self, list: Vec<String>) {
        self.view.websites.given(list);
    }

    /// A directory's list the player refused at once — a photograph's
    /// (`--refused`): what the answer to its `find` does in a door on a
    /// screen (`player::Told::Listing`).
    pub fn listing_refused(
        &mut self,
        ui: &mut Ui,
        query: ronoctua_player::find::Query,
        trouble: ronoctua_player::wire::Trouble,
    ) {
        let told = crate::player::Told::Listing {
            query,
            answer: Box::new(crate::player::Told::Refused(trouble)),
        };
        if self.view.told(&told, &ui.texts) {
            self.view
                .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
        }
        let older = self.player.as_ref().is_some_and(crate::player::older);
        match crate::player::told_sentence(&told, older, &ui.texts) {
            Some(said) => ui.say(said),
            None => ui.message = None,
        }
        ui.ask_paint();
    }

    /// The favourites and the played lately — a photograph's
    /// (`--favourites`, `--recent`): never read from this machine's files.
    pub fn picks_given(
        &mut self,
        favourites: Vec<ronoctua_broadcast::picks::Pick>,
        recent: Vec<ronoctua_broadcast::picks::Pick>,
    ) {
        self.view.directories.picks.given(favourites, recent);
    }

    /// Every group of the globe's marks the last frame drew, and the cell of
    /// the surface its centre is in — what a photograph says it drew, so a
    /// walk can point at one (`--list`).
    #[must_use]
    pub fn globe_marks_at(&self) -> Vec<MarkDrawn> {
        let Some((drawn, size)) = self.globe_drawn else {
            return Vec::new();
        };
        #[allow(clippy::cast_precision_loss)]
        let (sx, sy) = (
            drawn.width / size.0.max(1) as f32,
            drawn.height / size.1.max(1) as f32,
        );
        self.view
            .globe_groups()
            .into_iter()
            .map(|(members, (x, y))| {
                let at = (f64::from(drawn.x + x * sx), f64::from(drawn.y + y * sy));
                (members, (column_at(at.0), line_at(at.1)), at)
            })
            .collect()
    }

    /// The person's keys as kept — a photograph's (`--kept`): never read
    /// from this machine's files.
    pub fn keys_given(&mut self, keys: Vec<ronoctua_lookup::keys::Key>) {
        self.view.kept = crate::player_internet::Kept::given(keys);
    }

    /// A several-line field's draft, where the daemon wrote it: opened in the
    /// editor — `launch::editor_command`, the file manager's own answer — or,
    /// with no editor on this machine, that said.
    fn draft_opened(&mut self, ui: &mut Ui, path: &std::path::Path) {
        let client = std::env::var("RONOCTUA_EMACSCLIENT").ok();
        // A RESULT, SAID — and gone again after the footer's three seconds.
        let said = match crate::launch::editor_command(client.as_deref(), path) {
            Some((program, args, cwd)) => {
                ui.launch(&program, &args, cwd.as_deref());
                ui.texts.t(crate::key::PLAYER_DRAFT_OPENED)
            }
            None => ui.texts.t(crate::key::FILES_NO_EDITOR),
        };
        ui.say(said);
    }

    /// The queue's outline from the player — the watch thread's, or a
    /// photograph's fixture: kept by the view, which draws the queue from it,
    /// and the cursor follows what it was on.
    pub fn outline_arrived(
        &mut self,
        ui: &mut Ui,
        outline: Option<Box<ronoctua_player::queue::Outline>>,
    ) {
        self.view
            .outline_heard(outline, self.player.as_ref(), ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// A frame of lane `k`'s picture arrived.
    fn frame_arrived(&mut self, ui: &mut Ui, k: usize) {
        let lane = u32::try_from(k).ok().map(ronoctua_player::own::LaneId);
        if self
            .pictures
            .iter_mut()
            .find(|p| p.of.is_some() && p.of == lane)
            .is_some_and(crate::player_picture::Picture::took_a_frame)
        {
            ui.ask_paint();
        }
    }

    fn rows(&self, ui: &Ui) -> Vec<crate::player_rows::Row> {
        self.view.rows(self.player.as_ref(), ui.columns, &ui.texts)
    }

    /// Lays the door out from the state, the size and the area it is handed.
    fn relayout(&mut self, ui: &Ui, area: &Area) {
        // THE PICTURES ROUTED TO THIS DOOR'S SCREEN, in the arrangement's
        // order (`player_layout::tiles_on`) — and the view told the screens,
        // for a lane's page.
        let screen = self.screen(ui);
        let known = self.known(ui);
        self.view.screens_are(known, screen.clone());
        let tiles = self
            .player
            .as_ref()
            .map(|s| crate::player_layout::tiles_on(s, screen.as_deref()))
            .unwrap_or_default();
        let form = self.player.as_ref().map_or(
            ronoctua_player::own::Form::Auto,
            crate::player_layout::form_of,
        );
        let shape =
            (!tiles.is_empty()).then(|| ronoctua_player::arrange::natural_aspect(form, &tiles));
        // THE PICTURE FOLDED AWAY: no band, the rows take its lines — and
        // the view told whether there is one to fold, for the glyph on "Now"
        // and `z` (on stage the picture is all there is: nothing folds).
        let stage = self.on_stage();
        self.view
            .picture_foldable((shape.is_some() && !stage).then_some(self.picture_folded));
        let shape = shape.filter(|_| !self.picture_folded || stage);
        self.deck = self.block_deck(ui, area);
        let deck = u16::try_from(self.deck.lines.len()).unwrap_or(1);
        // THE POSTERS ARE DECODED to this screen's pixels.
        self.view
            .scale_is(u32::try_from(ui.scale.max(1)).unwrap_or(1));
        self.layout = if self.stage == Stage::Listing {
            listing_layout(ui.first_of_list(), ui.lines)
        } else if self.on_stage() {
            stage_layout(area.top, ui.lines)
        } else {
            layout(
                area,
                ui.first_of_list(),
                ui.lines,
                ui.columns + 2 * MARGIN,
                line_px() / cell_px(),
                shape,
                deck,
            )
        };
        // WHERE EACH GOES in the band — or on stage the whole surface — by
        // the one placement every door shares: what is painted and what a
        // click reads.
        let band = self.band_area(ui, area);
        self.placed = match (band, self.player.as_ref()) {
            (Some(band), Some(s)) => crate::player_layout::placed(s, &tiles, band),
            _ => Vec::new(),
        };
        // AND THE VIEW TOLD WHERE THEY ARE LAID OUT: the Layout page draws
        // what this door shows, and says what it leaves out.
        self.view
            .pictures_room_is(band.map(|b| (b.width, b.height)));
        // A THREAD PER PICTURE, BY ITS LANE: kept while the lane is shown
        // here — moved to another place of the order, it keeps its frames —
        // ended when it is not; and in a detached player a STILL, standing
        // in for the daemon's frames (`Reach::picture`).
        let placed = &self.placed;
        self.pictures
            .retain(|p| p.of.is_some_and(|l| placed.iter().any(|(x, _)| *x == l)));
        for (lane, _) in &self.placed {
            if !self.pictures.iter().any(|p| p.of == Some(*lane)) {
                let picture = self.reach.picture(*lane, &self.frame_sender);
                self.pictures.push(picture);
            }
        }
        // AND THE SIZE EACH ASKS FOR, in PHYSICAL pixels: the frame is drawn
        // one pixel to one pixel, sharp on a scale-2 screen — its place's,
        // which is already the picture's own shape: a frame asked in a band's
        // shape comes back letterboxed by the player, in black, and the band's
        // surface tone behind it never shows.
        for (lane, rect) in &self.placed {
            if let Some(p) = self.pictures.iter().find(|p| p.of == Some(*lane)) {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let size = (
                    (rect.width * ui.scale as f32) as u32,
                    (rect.height * ui.scale as f32) as u32,
                );
                p.want(size);
            }
        }
    }

    /// THE DECK UNDER THE TRANSPORT, laid out for this frame: a SET's lanes
    /// on one clock — `space.player.deckLanes` of them at most, and never
    /// more than a quarter of the surface's lines, the rest scrolling under
    /// one bar — and otherwise the one time bar: a file alone, a stage (the
    /// picture alone, the whole screen, the corner), and the timeline's own
    /// page, whose rows are every lane already. The small column keeps a
    /// lane's glyph, its band and its mute (`Room::narrow`).
    fn block_deck(&mut self, ui: &Ui, area: &Area) -> crate::player_timeline::Deck {
        use crate::player_timeline::{Room, of_state, set_lanes};
        let s = self.player.as_ref();
        let lanes = match s {
            Some(s) if !self.on_stage() && self.view.page() != crate::player::PageId::Timeline => {
                set_lanes(s, self.view.outline.as_deref())
            }
            _ => &[],
        };
        let room = if area.size == Size::Small {
            Room::narrow(ui.columns)
        } else {
            Room::wide(ui.columns)
        }
        .at_most(tokens::space::PLAYER_DECKLANES.min(ui.lines / 4));
        let deck = of_state(lanes, s, room, self.deck_offset);
        self.deck_offset = deck.scroll.offset;
        deck
    }

    /// Which DECK is under `(x, y)`, which of its lines, and at which column
    /// of it (fractional cells from the deck's own first cell), with the
    /// deck's first cell in the surface — the block under the transport, or a
    /// row that is a line of one. None while the controls are hidden: hidden
    /// controls are not controls.
    fn deck_at(
        &self,
        ui: &Ui,
        rows: &[crate::player_rows::Row],
        x: f64,
        y: f64,
    ) -> Option<(DeckAt, usize, f64, f64)> {
        if !self.controls_shown() || y < 0.0 {
            return None;
        }
        let l = line_at(y);
        let cells = x / f64::from(cell_px()).max(1.0);
        if l >= self.layout.bar && l < self.layout.bar + self.layout.deck {
            let line = usize::from(l - self.layout.bar);
            if line < self.deck.lines.len() {
                let origin = f64::from(MARGIN);
                return Some((DeckAt::Block, line, cells - origin, origin));
            }
        }
        let n = self.row_at(ui, x, y)?;
        let (d, column) = crate::player_rows::deck_at(rows, n, Self::column_in_rows(x))?;
        Some((DeckAt::Row(n), d.line, column, f64::from(MARGIN + d.left)))
    }

    /// What the deck `at` says a press at `column` of its `line` takes — the
    /// block's, or the one a row carries.
    fn deck_hit(
        &self,
        at: DeckAt,
        rows: &[crate::player_rows::Row],
        line: usize,
        column: f64,
    ) -> Option<crate::player_timeline::Hit> {
        match at {
            DeckAt::Block => self.deck.hit(line, column),
            DeckAt::Row(n) => rows.get(n)?.deck.as_ref()?.deck.hit(line, column),
        }
    }

    /// The control of a deck under `(x, y)`, when one is: what lights, and
    /// what line 0 says.
    fn deck_hover(
        &self,
        ui: &Ui,
        rows: &[crate::player_rows::Row],
        x: f64,
        y: f64,
    ) -> Option<(DeckAt, usize, crate::player_timeline::Hit)> {
        let (at, line, column, _) = self.deck_at(ui, rows, x, y)?;
        self.deck_hit(at, rows, line, column)
            .map(|hit| (at, line, hit))
    }

    /// What a deck `at` asked — the player told, a lane's page opened, or a
    /// lane CHOSEN: on a row the first click brings the cursor to it and the
    /// second opens its page; on the block the lane is lit as chosen, and a
    /// second click on it opens its page (invariant 19).
    fn deck_act(&mut self, ui: &mut Ui, act: crate::player_timeline::Act, at: DeckAt) {
        use crate::player_timeline::Act;
        match act {
            Act::Nothing => ui.ask_paint(),
            Act::Tell(ask) => self.tell(ui, &ask),
            Act::Choose(lane) => {
                let again = match at {
                    DeckAt::Row(n) => self.view.band_clicked(n),
                    DeckAt::Block => self.deck_chosen.replace(lane) == Some(lane),
                };
                if again {
                    self.pressed(
                        ui,
                        crate::player::Press::Go(crate::player::PageId::Lane(lane)),
                    );
                } else {
                    ui.ask_paint();
                }
            }
        }
    }

    /// The block's lanes put where a press at height `y` on their bar puts
    /// them — the bar grabbed by its whole track.
    fn deck_bar_to(&mut self, ui: &mut Ui, y: f64) {
        #[allow(clippy::cast_possible_truncation)]
        let y = y as f32;
        self.deck_offset = self.deck.offset_at(
            ui.frame(0),
            self.layout.bar,
            MARGIN,
            0..self.deck.lines.len(),
            y,
        );
        ui.ask_paint();
    }

    /// A COVER over the rows of `cr`'s run — `run` of its lines on screen,
    /// the first at line `y` — as `player_metadata::cover_place` places it:
    /// in its own proportion from the box's corner, the part the scroll cut
    /// cut from the picture too, under a key of its own. Uploaded when the
    /// texture is missing; `None` when the view holds no such picture or
    /// nothing of it shows.
    fn cover_drawn(
        &self,
        cr: &crate::player_rows::CoverRow,
        run: usize,
        y: u16,
        canvas: &mut Canvas<'_>,
    ) -> Option<(String, ronoctua_render::image::Area)> {
        // NO PLAYER NEEDED for a poster of the catalogue: channels by theme
        // are there with none running.
        let (key, rgba) = self.view.picture(self.player.as_ref(), &cr.key)?;
        let first = cr.line;
        let end = first.saturating_add(u16::try_from(run).unwrap_or(u16::MAX));
        let (place, crop) = crate::player_metadata::cover_place(
            (cr.cells, cr.lines),
            (cell_px(), line_px()),
            cr.size,
            (first, end),
        )?;
        let key = if crop == (0.0, 1.0) {
            key.clone()
        } else {
            format!("{key}:{:.4}:{:.4}", crop.0, crop.1)
        };
        if canvas.images.missing(&key) {
            let piece = if crop == (0.0, 1.0) {
                Some((**rgba).clone())
            } else {
                crate::player_metadata::cropped(rgba, crop)
            };
            if let Some(piece) = piece {
                canvas
                    .images
                    .upload_it(canvas.device, canvas.queue, &key, &piece);
            }
        }
        // THE BOX'S TOP is `first` lines above the line drawn first.
        let top = (f32::from(y) - f32::from(first)) * line_px();
        Some((
            key,
            ronoctua_render::image::Area {
                x: f32::from(MARGIN + cr.left) * cell_px() + place.x,
                y: top + place.y,
                width: place.width,
                height: place.height,
            },
        ))
    }

    /// THE BLOCK'S DECK AS IT IS DRAWN: its shapes — a dragged band's ghost
    /// among them — its words, each glyph lit under the pointer as a button
    /// is, and its bar's track. The painter draws this, and nothing else of
    /// the block; the pointer reads the same deck at the same place
    /// ([`Player::deck_at`]).
    fn block_drawn(&self, ui: &Ui) -> BlockDrawn {
        let f = ui.frame(0);
        let s = self.player.as_ref();
        let sound = s.map(|s| crate::player_timeline::Sound {
            volume_percent: s.volume,
            muted: s.muted,
        });
        let nothing = State::default();
        let said = s.unwrap_or(&nothing);
        let deck = self.deck.holding(self.dragged);
        let every = 0..deck.lines.len();
        let mut spans = Vec::new();
        for line in every.clone() {
            let lit = self
                .deck_under
                .filter(|(at, l, _)| *at == DeckAt::Block && *l == line)
                .and_then(|(_, _, hit)| hit.part());
            let y = self.layout.bar + u16::try_from(line).unwrap_or(0);
            // THE LANE A FIRST CLICK CHOSE wears the cursor's colour.
            let chosen = self
                .deck_chosen
                .is_some_and(|c| deck.lane_line(line).is_some_and(|l| l.lane == c));
            for w in deck.words(line, said, &ui.texts, lit, chosen) {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(tokens::text::size()),
                    text: w.text,
                    column: MARGIN + w.at,
                    line: y,
                    color: w.color,
                    edge: false,
                    foot: false,
                });
            }
        }
        BlockDrawn {
            quads: deck.quads(f, self.layout.bar, MARGIN, every.clone(), sound),
            spans,
            bar: deck
                .bar(f, self.layout.bar, MARGIN, every)
                .map(|(track, _)| track),
        }
    }

    /// How many rows fit under the transport.
    fn room(&self, ui: &Ui) -> usize {
        usize::from(ui.lines.saturating_sub(self.layout.list))
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the
    /// edge — the list here starts under the transport, not under the header,
    /// so this is the door's own and not `Ui::follow`.
    fn follow(&self, ui: &mut Ui, rows: usize) {
        const EDGE: usize = 2;
        let room = self.room(ui);
        if room == 0 {
            return;
        }
        let cursor = self.view.cursor();
        if cursor < ui.offset + EDGE {
            ui.offset = cursor.saturating_sub(EDGE);
        } else if cursor + EDGE >= ui.offset + room {
            ui.offset = cursor + EDGE + 1 - room;
        }
        ui.offset = ui.offset.min(rows.saturating_sub(room));
    }

    fn apply(&mut self, ui: &mut Ui, errand: crate::player::Errand) {
        // WHAT REACHES PAST THE DOOR — the daemon, the person's preferences —
        // is `Reach`'s: a detached player keeps it, and says what the door
        // says when the daemon took it. Everything that stays in the door —
        // a stage, a question, an order for the footer queued on a loop that
        // never turns — is the door's, in both.
        let carried = self.reach.carry_out(errand, &ui.texts, &self.told);
        // WHAT THE ERRAND WROTE — a switch, a consent — read back from its
        // file: a write that was refused is not shown as done.
        self.view.errand_carried();
        match carried {
            // WHAT IT DID, SAID — "was added to the queue" — and gone again
            // after the footer's three seconds (`Ui::say`).
            Ok(Some(said)) => ui.say(said),
            Ok(None) => ui.message = None,
            Err(crate::player::Errand::Close) => {
                ui.quit = true;
                return;
            }
            Err(crate::player::Errand::Show(show)) => self.toggle_stage(ui, show),
            Err(crate::player::Errand::Swap) => {
                self.swap(ui);
                return;
            }
            // THE PLAYER CLOSED EVERYWHERE, from this door: the daemon told
            // here — only if it runs, and never waited for — and the other
            // doors that are processes ended here too, so it all goes even
            // with no footer to hear the order; the footer is handed the same
            // order for its own view; and this door goes.
            Err(crate::player::Errand::QuitEverywhere) => {
                self.reach.tell_if_running(&Ask::Quit);
                self.reach.end_the_doors();
                ui.tell_the_footer("player-quit", None);
                self.leave(ui);
                return;
            }
            // WHAT THE VIEW ASKS IS TYPED HERE, on this door's own line — as
            // the footer types it on its minibuffer — and comes back to
            // `Content::answered`, a pairing's code masked.
            Err(crate::player::Errand::Ask(q)) => {
                ui.ask(q.line());
            }
            // THE PAIRING, off the loop: the player answers only once the
            // other machine has, and this window's loop does not wait for it.
            Err(crate::player::Errand::Pair { to, code }) => {
                self.reach.pair(to, code, self.paired.clone());
            }
            // WHAT PLAYS ON EVERY SCREEN — the screens this door knows by
            // name, and those something goes to — or the windows up closed.
            Err(crate::player::Errand::EveryScreen) => {
                let known: Vec<String> = self.known(ui).into_iter().map(|k| k.connector).collect();
                let routed: Vec<String> = self.player.as_ref().map_or_else(Vec::new, |s| {
                    crate::player_layout::screens_with_pictures(s, &known)
                        .into_iter()
                        .map(ToOwned::to_owned)
                        .collect()
                });
                // WHAT IT DID, SAID — and gone again, as every result is.
                ui.say(match self.reach.every_screen(known.len(), &routed) {
                    Ok(done) => crate::player_stage::everywhere_said(&done, &ui.texts),
                    Err(e) => {
                        tracing::warn!(error = %e, "the windows for every screen could not be opened");
                        ui.texts.t_with(
                            crate::key::PLAYER_EVERY_SCREEN_FAILED,
                            &[("why", &e.to_string())],
                        )
                    }
                });
            }
            // THE CLIPBOARD, for the text being edited: read off the loop and
            // handed back to `Content::pasted`.
            Err(crate::player::Errand::Paste) => ui.paste(),
            // THE KIOSK IS THE FOOTER'S: it holds the bridge and the tabs. The
            // order goes down its socket off the loop, and a footer that is
            // not there is said here (`door-no-footer`).
            Err(crate::player::Errand::Kiosk(url)) => ui.tell_the_footer("kiosk-open", Some(&url)),
            // THE PICTURE FOLDED AWAY, or back: this door's, laid out again
            // at the next paint.
            Err(crate::player::Errand::FoldPicture) => {
                self.picture_folded = !self.picture_folded;
            }
            // A PASSWORD KEPT BY A PHOTOGRAPH: answered as its `--pam` says,
            // the way the thread's answer comes back to a door on a screen.
            Err(crate::player::Errand::Unlock(_)) => {
                if let Reach::Kept(out) = &mut self.reach {
                    out.passwords_checked += 1;
                    if let Some(answer) = out.pam {
                        let told = crate::player::Told::Unlock(answer);
                        self.view.told(&told, &ui.texts);
                        self.view
                            .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
                        if let Some(said) = crate::player::told_sentence(&told, false, &ui.texts) {
                            ui.say(said);
                        }
                        if let Some(ask) = self.view.lock_owed() {
                            self.apply(ui, ask);
                        }
                    }
                }
            }
            Err(_) => {}
        }
        self.listing_stage_holds(ui);
        ui.ask_paint();
    }

    /// A click on the picture, as `player_media::PictureClicks` read it:
    /// play or pause, the whole screen, or ten seconds back or ahead — said,
    /// as a key's seek is.
    fn picture_clicked(&mut self, ui: &mut Ui, click: crate::player_media::PictureClick) {
        use crate::player_media::PictureClick;
        match click {
            PictureClick::Toggle => self.tell(ui, &Ask::Toggle),
            PictureClick::ToggleAndMaximize => {
                self.tell(ui, &Ask::Toggle);
                self.toggle_stage(ui, crate::player::Show::Maximized);
            }
            PictureClick::ToggleAndSeek(seconds) => {
                self.tell(ui, &Ask::Toggle);
                let errand = crate::player_transport::seek_errand(seconds > 0.0, &ui.texts);
                self.apply(ui, errand);
            }
            PictureClick::Seek(seconds) => {
                let errand = crate::player_transport::seek_errand(seconds > 0.0, &ui.texts);
                self.apply(ui, errand);
            }
        }
    }

    /// THE LISTING'S STAGE holds the listing and nothing else: a page that is
    /// not it — a row that opened another — ends the stage; but for an adult
    /// listing handed over, waiting for its password.
    fn listing_stage_holds(&mut self, ui: &mut Ui) {
        if self.stage == Stage::Listing
            && self.view.page() != crate::player::PageId::Catalog
            && !self.view.listing_waits()
        {
            self.set_stage(ui, Stage::Off);
        }
    }

    fn tell(&mut self, ui: &mut Ui, ask: &Ask) {
        ui.message = None;
        self.reach.tell(ask, &self.told);
        ui.ask_paint();
    }

    /// THIS DOOR FOR THE OTHER (`player::Errand::Swap`): a window asks the
    /// footer to RAISE the column — never the toggle, which would close one
    /// already up — and closes, which for a window opened for a stage is the
    /// stage closing; the column asks for the window and closes, and its
    /// `Drop` gives the slot up. The footer opens the door; and a footer that
    /// did not take the order keeps this one open, saying so
    /// (`Ui::hand_over`) — never a door gone and nothing in its place.
    fn swap(&mut self, ui: &mut Ui) {
        match self.door {
            Door::Window => ui.hand_over("player-column", Some(crate::player::RAISE)),
            // THE WINDOW, from the column — and from the footer, which
            // carries its own swap out (`Footer::player_errand`).
            Door::Column | Door::Footer => ui.hand_over("player-window", None),
        }
    }

    /// A button pressed, in either door and in the bar: what
    /// `player_transport::button_errand` says it does, carried out as a key's errand.
    fn pressed_button(&mut self, ui: &mut Ui, id: &str) {
        // A KEY OF THE VIEW — the media in the file opens a page — goes to
        // the view, as the key would.
        if let Some(press) = crate::player_transport::button_press(id) {
            self.pressed(ui, press);
            return;
        }
        let errand = crate::player_transport::button_errand(id, self.player.as_ref(), &ui.texts);
        self.apply(ui, errand);
    }

    fn pressed(&mut self, ui: &mut Ui, press: crate::player::Press) {
        ui.message = None;
        // ON STAGE THE ROWS ARE NOT DRAWN: a key that needs them brings them
        // back first — and one that opens a page opens it there, in front of
        // the person (`player::Unseen`).
        if self.on_stage() && press.unseen() != crate::player::Unseen::Acts {
            // A WINDOW OPENED FOR ITS STAGE is that stage and nothing else:
            // leaving it closes it, so there are no rows here to bring back.
            if self.opened_for.is_some() {
                ui.say(ui.texts.t(crate::key::PLAYER_STAGE_HAS_NO_ROWS));
                ui.ask_paint();
                return;
            }
            self.set_stage(ui, Stage::Off);
            if press.unseen() == crate::player::Unseen::ShowsTheRows {
                return;
            }
        }
        // WHAT A PAGE IS, for the picker's `PgUp`/`PgDn`: the lines the list
        // shows.
        self.view.page_is(self.room(ui));
        let errand = self
            .view
            .key(press, self.player.as_ref(), ui.columns, &ui.texts);
        let rows = self.rows(ui).len();
        self.follow(ui, rows);
        self.apply(ui, errand);
    }

    /// Where `x` is among the rows' own cells — a field's `at`, in fractions
    /// of a cell — which is what a bar of a row is measured in.
    fn column_in_rows(x: f64) -> f64 {
        x / f64::from(cell_px()).max(1.0) - f64::from(MARGIN)
    }

    /// Turns the bar of row `row` to where `x` is along it, when the row has
    /// one — the press, and every motion of the drag after it.
    fn slide(&mut self, ui: &mut Ui, row: usize, x: f64) -> bool {
        let rows = self.rows(ui);
        let column = Self::column_in_rows(x);
        let Some(bar) = rows.get(row).and_then(|r| r.slider) else {
            return false;
        };
        let errand = crate::player_rows::slid(bar.of, bar.along(column), self.player.as_ref());
        self.apply(ui, errand);
        true
    }

    /// The row of the list at `(x, y)`.
    ///
    /// `None` above the list — and ON THE BAR'S COLUMN while a bar is drawn
    /// (`ui.room`, written where the bar is painted): the bar has its own
    /// cells, and a pointer on them is on the bar and not on the row beside
    /// it. Without a bar the row answers across the whole width.
    fn row_at(&self, ui: &Ui, x: f64, y: f64) -> Option<usize> {
        #[allow(clippy::cast_possible_truncation)]
        if ui.room.on_the_bar(x as f32) {
            return None;
        }
        let l = line_at(y);
        (l >= self.layout.list).then(|| usize::from(l - self.layout.list) + ui.offset)
    }

    /// The id of the control of a row at `(x, y)` — a title's glyph — by the
    /// same row and column a click there takes (`View::clicked_at`).
    fn control_at(
        &self,
        ui: &Ui,
        rows: &[crate::player_rows::Row],
        x: f64,
        y: f64,
    ) -> Option<&'static str> {
        let row = self.row_at(ui, x, y)?;
        crate::player_rows::control_under(rows, row, column_at(x).saturating_sub(MARGIN))
    }

    /// The transport button at `(x, y)`, where it was DRAWN.
    fn button_at(&self, x: f64, y: f64) -> Option<&'static str> {
        let (l, c) = (line_at(y), column_at(x));
        self.buttons
            .iter()
            .find(|(line, a, b, _)| l == *line && c >= *a && c < *b)
            .map(|(_, _, _, id)| *id)
    }

    /// The transport's panel: the surface inside its margins — the footer's
    /// panel's arithmetic, on this door's line (`player_media::buttons`).
    const fn transport_panel(ui: &Ui) -> crate::player_media::Panel {
        crate::player_media::Panel {
            from: MARGIN,
            cells: ui.columns,
        }
    }

    /// Every button of the transport this frame, where it goes — the ones
    /// the state shows, three cells apart, only the ones whole. None while
    /// the controls are hidden: HIDDEN CONTROLS ARE NOT CONTROLS, so a click
    /// on stage lands on the picture and not on a button nobody can see. The
    /// volume's glyph is the deck's, on its first line (`Deck::press`).
    fn placed_buttons(
        &self,
        ui: &Ui,
        shown: bool,
    ) -> (Vec<crate::player_media::Placed>, Option<(u16, u16)>) {
        if !shown {
            return (Vec::new(), None);
        }
        crate::player_media::transport(
            Self::transport_panel(ui),
            self.layout.transport,
            self.player.as_ref(),
        )
    }

    /// The point `at` of the surface in the globe's picture, as this frame
    /// drew it, when it is on it — hidden controls are not controls, and on
    /// stage no globe is drawn.
    fn globe_px(&self, at: (f64, f64)) -> Option<(f32, f32)> {
        let (drawn, size) = self.globe_drawn?;
        #[allow(clippy::cast_possible_truncation)]
        crate::player_globe::picture_px(drawn, size, (at.0 as f32, at.1 as f32))
    }

    /// Whether `y` is on the pictures' band — the pictures and the ground
    /// they are framed on, which answers the click a picture would.
    fn on_a_picture(&self, y: f64) -> bool {
        let l = line_at(y);
        self.layout
            .band
            .is_some_and(|(first, lines)| l >= first && l < first + lines)
    }

    #[allow(clippy::too_many_lines)]
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<PerPointer>, events: &[PointerEvent]) {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        let head = ui.frame(0);
        let drawn = self.rows(ui);
        // THE CELL LIT, too: a pointer moved from one cell of a grid row to
        // the next is on the same row, and the band has to follow it.
        let before = (
            hand.row_under_pointer,
            hand.extra.button_under,
            self.deck_under,
            hand.extra.control_under,
            lit_cell(&drawn, hand),
        );
        let total = drawn.len();
        let scroll = crate::chrome::list_scroll(total, self.room(ui), ui.offset);
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (x, y) = (e.position.0 as f32, e.position.1 as f32);
            // ACTIVITY IS ANY OF IT but leaving: on stage the controls come
            // back over the picture.
            if !matches!(e.kind, PointerEventKind::Leave { .. }) {
                self.pointer_at = Some(e.position);
                self.wake_controls(ui);
            }
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if ui.hinted.dragged(head, e.position.1) {
                        ui.ask_paint();
                        continue;
                    }
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &ui.texts));
                    if ui.hinted.say(now) {
                        ui.ask_paint();
                    }
                    if hand.dragging {
                        ui.offset =
                            crate::chrome::list_offset_at(scroll, self.layout.list, w, h, y);
                        ui.ask_paint();
                        continue;
                    }
                    // THE GLOBE HELD: past the slack, the hand turns it.
                    if let Some(held) = hand.extra.globe.as_mut() {
                        let far = (e.position.0 - held.at.0).hypot(e.position.1 - held.at.1);
                        if far > f64::from(tokens::space::ATLAS_CLICKSLACK) {
                            held.dragging = true;
                        }
                        if held.dragging {
                            #[allow(clippy::cast_possible_truncation)]
                            let moved = self.view.globe_panned(
                                (e.position.0 - held.last.0) as f32,
                                (e.position.1 - held.last.1) as f32,
                            );
                            held.last = e.position;
                            if moved {
                                ui.ask_paint();
                            }
                        }
                        continue;
                    }
                    // A DECK HELD: the drag goes on along what was pressed.
                    if let Some((grip, origin, at)) = hand.extra.deck {
                        self.deck_dragged(ui, hand, (grip, origin, at), e.position);
                        continue;
                    }
                    if let Some(row) = hand.extra.sliding {
                        self.slide(ui, row, e.position.0);
                        continue;
                    }
                    let rows = self.rows(ui);
                    self.deck_under = self.deck_hover(ui, &rows, e.position.0, e.position.1);
                    // A LINE OF A DECK IS A FIGURE, and no band of the list
                    // lights behind it: its controls say what they do. The
                    // layout's schematic is one too.
                    // A STRIP'S PICTURE is no figure of the line: the words
                    // beside it light (`player_rows::Lit::From`).
                    hand.row_under_pointer =
                        self.row_at(ui, e.position.0, e.position.1).filter(|&n| {
                            rows.get(n).is_none_or(|r| {
                                r.deck.is_none()
                                    && r.schematic.is_none()
                                    && (r.covers.is_empty()
                                        || matches!(r.lit, crate::player_rows::Lit::From(_)))
                                    && r.globe.is_none()
                            })
                        });
                    // AND WHERE ACROSS IT: a grid's row lights one cell.
                    hand.x_under_pointer = hand.row_under_pointer.map(|_| x);
                    // THE GLOBE UNDER THE HAND: the group there lit, and the
                    // line under it saying where it is.
                    let px = self.globe_px(e.position);
                    if self.view.globe_pointed(px) {
                        ui.ask_paint();
                    }
                    hand.extra.button_under = self.button_at(e.position.0, e.position.1);
                    hand.extra.control_under =
                        self.control_at(ui, &rows, e.position.0, e.position.1);
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_at = None;
                    ui.hinted.let_go();
                    if ui.hinted.say(None) {
                        ui.ask_paint();
                    }
                    hand.dragging = false;
                    hand.extra.sliding = None;
                    hand.row_under_pointer = None;
                    hand.x_under_pointer = None;
                    hand.extra.button_under = None;
                    hand.extra.control_under = None;
                    hand.extra.globe = None;
                    if self.view.globe_pointed(None) {
                        ui.ask_paint();
                    }
                    // A DRAG THAT LEFT is a drag called off: nothing is asked.
                    if hand.extra.deck.take().is_some() {
                        self.dragged = None;
                    }
                    self.deck_under = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    hand.dragging = false;
                    hand.extra.sliding = None;
                    // THE GLOBE LET GO where it was pressed: a click on the
                    // group there — the first selects, the second acts.
                    if let Some(held) = hand.extra.globe.take()
                        && !held.dragging
                        && let Some(px) = self.globe_px(held.at)
                    {
                        let errand = self.view.globe_clicked(
                            px,
                            self.player.as_ref(),
                            ui.columns,
                            &ui.texts,
                        );
                        // A FIRST CLICK only brings the cursor: the globe
                        // stays under the hand for the second.
                        if !self.view.globe_keeps_the_scroll() {
                            let n = self.rows(ui).len();
                            self.follow(ui, n);
                        }
                        self.apply(ui, errand);
                        ui.ask_paint();
                    }
                    // A DECK LET GO: a band dragged asks its new start, and
                    // one let go where it was pressed chooses its lane — the
                    // second click opens its page.
                    if let Some((grip, _, at)) = hand.extra.deck.take() {
                        let act = grip.released(self.dragged.take());
                        // A BAND DRAGGED on a row brings the cursor to its
                        // row, as every other control of it does.
                        if let (
                            crate::player_timeline::Grip::Band { moved: true, .. },
                            DeckAt::Row(n),
                        ) = (grip, at)
                        {
                            self.view.grabbed(n);
                        }
                        if self.player.is_some() {
                            self.deck_act(ui, act, at);
                        }
                    }
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        ui.ask_paint();
                        continue;
                    }
                    if crate::chrome::on_the_list_bar(scroll, self.layout.list, w, h, x, y) {
                        hand.dragging = true;
                        ui.offset =
                            crate::chrome::list_offset_at(scroll, self.layout.list, w, h, y);
                        ui.ask_paint();
                        continue;
                    }
                    // A BUTTON of the transport: what its key does.
                    if let Some(id) = self.button_at(e.position.0, e.position.1) {
                        self.pressed_button(ui, id);
                        continue;
                    }
                    // THE GLOBE: held — a drag turns it, and let go where it
                    // was pressed it is a click on the group there.
                    if self.globe_px(e.position).is_some() {
                        hand.extra.globe = Some(GlobeHeld {
                            at: e.position,
                            last: e.position,
                            dragging: false,
                        });
                        continue;
                    }
                    // THE DECK — the time bar, or a set's lanes — under the
                    // transport or as rows: what `Deck::press` says the press
                    // takes, and what it holds until it is let go.
                    if self.deck_pressed(ui, hand, e.position) {
                        continue;
                    }
                    // A PICTURE: a click plays or pauses, as on every player;
                    // a DOUBLE click in its middle puts it on the whole
                    // screen, or back, and on a side seeks ten seconds —
                    // back on the left, ahead on the right — and each more
                    // click there seeks again (`player_media::PictureClicks`).
                    // The first click has already paused it by then, so the
                    // second plays it again: what a double click leaves is
                    // the film as it was, on another screen or elsewhere in
                    // it.
                    if self.on_a_picture(e.position.1) {
                        // THE REACH'S CLOCK: a photograph's moves only
                        // between the steps of its walk.
                        let now = self.reach.now();
                        let zone = crate::player_media::picture_zone(f64::from(w), e.position.0);
                        let click = hand.extra.picture_clicked.clicked(now, zone, DOUBLE_CLICK);
                        self.picture_clicked(ui, click);
                        continue;
                    }
                    // A BAR OF A ROW — the position's, the volume's: a press
                    // sets it there and holding drags it, through the one
                    // answer the footer asks too (`player_rows::slider_at`).
                    if let Some(row) = self.row_at(ui, e.position.0, e.position.1) {
                        let rows = self.rows(ui);
                        let column = Self::column_in_rows(e.position.0);
                        if crate::player_rows::slider_at(&rows, row, column).is_some() {
                            self.view.grabbed(row);
                            hand.extra.sliding = Some(row);
                            self.slide(ui, row, e.position.0);
                            continue;
                        }
                    }
                    // A ROW: the first click selects, the second acts.
                    if let Some(row) = self.row_at(ui, e.position.0, e.position.1) {
                        let rows = self.rows(ui);
                        // BY ITS COLUMN too: the files are a grid, and a title
                        // can carry a control at the end of its line — through
                        // `across`, the answer the hover lights a cell by: the
                        // cell a click takes is the cell the pointer lit.
                        // `Ctrl` MARKS the file there, `Shift` every file up
                        // to it — the picker's several, by the pointer.
                        let mods = ui.pointer_mods;
                        if (mods.ctrl || mods.shift)
                            && self
                                .view
                                .mark_clicked_px(row, x, across(), &rows, mods.shift)
                        {
                            let n = rows.len();
                            self.follow(ui, n);
                            ui.ask_paint();
                            continue;
                        }
                        let again = self.view.clicked_px(row, x, across(), &rows);
                        let n = rows.len();
                        self.follow(ui, n);
                        if again {
                            self.pressed(ui, crate::player::Press::Enter);
                        } else {
                            ui.ask_paint();
                        }
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut hand.lines_owed);
                    if by == 0 {
                        continue;
                    }
                    if ui.hinted.turned(head, e.position.1, by) {
                        ui.ask_paint();
                        continue;
                    }
                    // THE WHEEL OVER THE GLOBE brings it nearer, or further.
                    if self.globe_px(e.position).is_some() {
                        if self.view.globe_wheeled(by) {
                            ui.ask_paint();
                        }
                        continue;
                    }
                    // THE WHEEL OVER THE BLOCK'S DECK is a volume — a lane's
                    // that sounds, the player's elsewhere — and on its bar it
                    // scrolls the lanes. Over the deck's ROWS it is the list's,
                    // which a page longer than the door needs.
                    if self.deck_wheeled(ui, e.position, by) {
                        continue;
                    }
                    // OVER THE PICTURES AND THE TRANSPORT THE WHEEL IS THE
                    // VOLUME — the gesture belongs to what is under the hand,
                    // and there is no list there to walk.
                    if line_at(e.position.1) < self.layout.list {
                        self.turn_the_volume(ui, by);
                        continue;
                    }
                    let last = total.saturating_sub(self.room(ui));
                    ui.offset = usize::try_from(
                        i64::try_from(ui.offset)
                            .unwrap_or(0)
                            .saturating_add(by)
                            .max(0),
                    )
                    .unwrap_or(0)
                    .min(last);
                    ui.ask_paint();
                }
                _ => {}
            }
        }
        let drawn = self.rows(ui);
        if before
            != (
                hand.row_under_pointer,
                hand.extra.button_under,
                self.deck_under,
                hand.extra.control_under,
                lit_cell(&drawn, hand),
            )
        {
            ui.ask_paint();
        }
    }

    /// A press on a DECK — the block's, or a row's — at `(x, y)`: what
    /// `Deck::press` says it takes, done, and what it holds kept on the hand.
    /// `false` when no control of a deck is there. A lane's NAME on a row is
    /// the row's: the first click selects, the second opens its page, as on
    /// every list.
    fn deck_pressed(
        &mut self,
        ui: &mut Ui,
        hand: &mut Hand<PerPointer>,
        (x, y): (f64, f64),
    ) -> bool {
        use crate::player_timeline::{Grip, Hit};
        let rows = self.rows(ui);
        let Some((at, line, column, origin)) = self.deck_at(ui, &rows, x, y) else {
            return false;
        };
        let hit = self.deck_hit(at, &rows, line, column);
        if hit.is_none() || matches!((at, hit), (DeckAt::Row(_), Some(Hit::Name(_)))) {
            return false;
        }
        let (grip, act) = match at {
            DeckAt::Block => self.deck.press(line, column),
            DeckAt::Row(n) => {
                // THE CURSOR COMES TO THE ROW whose control was pressed: it
                // is what the control is about — a BAND's when it is let go,
                // whose first click chooses the row and second opens it.
                if !matches!(hit, Some(Hit::Band { .. })) {
                    self.view.grabbed(n);
                }
                rows[n]
                    .deck
                    .as_ref()
                    .map_or((None, crate::player_timeline::Act::Nothing), |d| {
                        d.deck.press(line, column)
                    })
            }
        };
        if matches!(grip, Some(Grip::Bar)) && at == DeckAt::Block {
            self.deck_bar_to(ui, y);
        }
        hand.extra.deck = grip.map(|g| (g, origin, at));
        self.dragged = None;
        self.deck_act(ui, act, at);
        true
    }

    /// The pointer moved with a deck HELD: the drag goes on along the track
    /// or the meter it was pressed on — a band's ghost follows it once it is
    /// a drag, and nothing is asked until it is let go; the lanes' bar
    /// scrolls them.
    fn deck_dragged(
        &mut self,
        ui: &mut Ui,
        hand: &mut Hand<PerPointer>,
        (grip, origin, at): (crate::player_timeline::Grip, f64, DeckAt),
        (x, y): (f64, f64),
    ) {
        use crate::player_timeline::Grip;
        // NO PLAYER, NO DRAG: what the grip would ask is about lanes that are
        // gone with the state — and telling it would start a player again.
        if self.player.is_none() {
            hand.extra.deck = None;
            self.dragged = None;
            return;
        }
        let mut grip = grip;
        if matches!(grip, Grip::Bar) {
            if at == DeckAt::Block {
                self.deck_bar_to(ui, y);
            }
            return;
        }
        let column = x / f64::from(cell_px()).max(1.0) - origin;
        let was_a_drag = matches!(grip, Grip::Band { moved: true, .. });
        let (act, ghost) = grip.moved(column);
        // A DRAG `Esc` CALLED OFF stays off: its ghost is not brought back.
        if let Some(h) = ghost
            && (!was_a_drag || self.dragged.is_some())
        {
            self.dragged = Some(h);
            ui.ask_paint();
        }
        hand.extra.deck = Some((grip, origin, at));
        self.deck_act(ui, act, at);
    }

    /// The wheel at `(x, y)`, `by` lines, over the block's deck: its bar and
    /// a lane that plays no sound scroll the lanes, and a lane that sounds or
    /// the ruler turn the volume under the pointer (`Deck::wheel`). `false`
    /// off the block.
    fn deck_wheeled(&mut self, ui: &mut Ui, (x, y): (f64, f64), by: i64) -> bool {
        let rows = self.rows(ui);
        let Some((DeckAt::Block, line, column, _)) = self.deck_at(ui, &rows, x, y) else {
            return false;
        };
        // ON THE BAR, or on a lane that plays no sound — which has no volume
        // to turn — the wheel scrolls the lanes, as over any list.
        let silent = self.deck.lane_line(line).is_some_and(|l| !l.sounds);
        if silent || self.deck.hit(line, column) == Some(crate::player_timeline::Hit::Bar) {
            self.deck_offset = usize::try_from(
                i64::try_from(self.deck_offset)
                    .unwrap_or(0)
                    .saturating_add(by)
                    .max(0),
            )
            .unwrap_or(0);
            ui.ask_paint();
            return true;
        }
        // NO STATE, NO VOLUME TO TURN FROM (`turn_the_volume`).
        let Some(volume) = self.player.as_ref().map(|s| s.volume) else {
            ui.say(ui.texts.t(crate::key::PLAYER_NOT_RUNNING));
            ui.ask_paint();
            return true;
        };
        if let Some(ask) = self.deck.wheel(line, by, volume) {
            self.tell(ui, &ask);
        }
        true
    }
}

impl Player {
    /// The wheel over what is not a list: the volume, a step per line.
    fn turn_the_volume(&mut self, ui: &mut Ui, by: i64) {
        // NO STATE, NO VOLUME TO TURN FROM: one guessed from nothing would be
        // sent, and kept (`player_transport::volume_step`).
        let Some(now) = self.player.as_ref().map(|s| s.volume) else {
            ui.say(ui.texts.t(crate::key::PLAYER_NOT_RUNNING));
            ui.ask_paint();
            return;
        };
        let step = i64::from(crate::player::VOLUME_STEP);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let percent =
            (i64::from(now) - by * step).clamp(0, i64::from(crate::player::VOLUME_MOST)) as u8;
        self.tell(ui, &Ask::Volume { percent });
    }

    /// THE BAR: the remote control's glyphs, one per line on the middle cell,
    /// each taken across the whole line — see the module's header. No header,
    /// no picture and no list: the rows, the message and the stage are kept,
    /// and are there again when the column is widened.
    fn paint_bar(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>, area: &Area) {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        canvas.under.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            color: tokens::color::SURFACE_CHROME,
        });
        // NO PICTURE, and no thread decoding one for nobody — and no hold on
        // the sound outputs: the bar shows none of their rows.
        self.pictures.clear();
        self.reach.sinks(false, &mut self.sinks);
        ui.room = crate::chrome::Room::default();
        self.buttons.clear();
        let s = self.player.as_ref();
        let across = ui.columns + 2 * MARGIN;
        for (line, id) in bar_buttons(area.top) {
            // A COLUMN SHORTER THAN ITS BUTTONS draws the ones that fit — above
            // the bar's last line, which is the column's close.
            if line >= area.end(ui.lines) {
                break;
            }
            self.buttons.push((line, 0, across, id));
            let colour = if self.button_under == Some(id) || lit(id, s, self.stage) {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_PRIMARY
            };
            canvas.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(tokens::text::size()),
                text: glyph_of(id, s, self.stage, self.door).to_string(),
                // THE MIDDLE CELL: the bar is five, the margins included.
                column: MARGIN + 1,
                line,
                color: colour,
                edge: false,
                foot: false,
            });
        }
        self.painted = true;
    }

    /// The pointer over the bar: the button under it lights, a press asks
    /// what its transport button asks, and the wheel is the volume.
    fn pointer_bar(&mut self, ui: &mut Ui, hand: &mut Hand<PerPointer>, events: &[PointerEvent]) {
        let before = hand.extra.button_under;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    hand.extra.button_under = self.button_at(e.position.0, e.position.1);
                }
                PointerEventKind::Leave { .. } => hand.extra.button_under = None,
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if let Some(id) = self.button_at(e.position.0, e.position.1) {
                        self.pressed_button(ui, id);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut hand.lines_owed);
                    if by != 0 {
                        self.turn_the_volume(ui, by);
                    }
                }
                _ => {}
            }
        }
        if before != hand.extra.button_under {
            ui.ask_paint();
        }
    }

    /// A key, in either door.
    fn typed_here(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        self.wake_controls(ui);
        // `Esc` CALLS A DRAG OFF first: the band stays where it was, and
        // letting it go asks nothing.
        if event.keysym == Keysym::Escape && self.dragged.take().is_some() {
            ui.ask_paint();
            return;
        }
        // `Esc` ON STAGE brings the door back first — one level, as every
        // view with levels has it — and only then reaches the view.
        // AND THE LISTING'S STAGE is a level of its own: `Esc` leaves the
        // whole screen first, the listing still open under it — and a window
        // opened for the stage goes.
        if event.keysym == Keysym::Escape && (self.on_stage() || self.stage == Stage::Listing) {
            self.set_stage(ui, Stage::Off);
            return;
        }
        // THE FOOTER'S READER: the view answers which press a key is, with
        // its modifiers (`player::press_of_key`), and `Esc` goes back one
        // level and closes the door from the top.
        // A TEXT BEING EDITED takes every key (`View::press_of_key`).
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(|k| self.view.press_of_key(k))
        {
            self.pressed(ui, press);
        }
    }

    /// Once per turn, in either door.
    fn each_turn_here(&mut self, ui: &mut Ui) {
        // A WINDOW OPENED FOR ONE SCREEN waits until its connection has
        // described that screen by name: made whole-screen before, it would
        // go wherever the compositor chose (`screens_follow` tries again).
        let waiting = self
            .screen_wanted
            .as_deref()
            .is_some_and(|c| self.window.borrow().is_some() && ui.screen_named(c).is_none());
        if !waiting && let Some(stage) = self.first_stage.take() {
            self.set_stage(ui, stage);
            // AN ADULT LISTING HANDED OVER is shut in this new door: the
            // person's password is asked first, masked.
            if let Some(ask) = self.view.lock_owed() {
                self.apply(ui, ask);
            }
        }
        // AFTER A FRAME: the window is in the compositor's tree only once a
        // buffer mapped it.
        if std::mem::take(&mut self.painted) {
            self.float_in_the_corner();
        }
    }

    /// Every size is SAID, because a size the compositor computed and never
    /// SENT is the defect the column's settle exists for, and only this line
    /// tells the two apart: the compositor's own tree shows the new size
    /// either way.
    fn configured(&self, ui: &Ui) {
        tracing::info!(width = ui.width, height = ui.height, door = ?self.door, "configured");
    }
}

/// On stage, the picture's band: the whole surface from `area`'s top.
fn stage_area(width: f32, height: f32, area: &Area) -> ronoctua_render::image::Area {
    ronoctua_render::image::Area {
        x: 0.0,
        y: area.top_px,
        width,
        height: (height - area.top_px).max(0.0),
    }
}

fn line_px() -> f32 {
    tokens::metrics::line_height(tokens::text::size())
}

fn cell_px() -> f32 {
    tokens::metrics::cell_width(tokens::text::size())
}

/// The cell of a grid row the hand lights — by `across`, the answer the
/// band is painted with — or `None` off a grid.
fn lit_cell(rows: &[crate::player_rows::Row], hand: &Hand<PerPointer>) -> Option<(usize, usize)> {
    let row = hand.row_under_pointer?;
    let cells = crate::player_rows::grid_cells(rows, row);
    across()
        .cell(&cells, hand.x_under_pointer?)
        .map(|c| (row, c))
}

/// Where a pointer is across this door's rows — the ONE answer the band it
/// paints under the pointer and the cell a click takes both read
/// (`chrome::Across`): the rows start `MARGIN` cells in.
fn across() -> crate::chrome::Across {
    crate::chrome::Across {
        lead: MARGIN,
        cell_px: cell_px(),
    }
}

/// The line of the window's grid a pixel `y` is on, from its top.
#[must_use]
pub fn line_at(y: f64) -> u16 {
    let line = f64::from(line_px()).max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let l = (y.max(0.0) / line) as u16;
    l
}

/// The cell of the window's grid a pixel `x` is in, from its left edge.
#[must_use]
pub fn column_at(x: f64) -> u16 {
    let cell = f64::from(cell_px()).max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let c = (x.max(0.0) / cell) as u16;
    c
}

/// The small window's size: `space.player.pip*`, in logical pixels.
fn small_size() -> (u32, u32) {
    (
        u32::from(tokens::space::PLAYER_PIPWIDTH),
        u32::from(tokens::space::PLAYER_PIPHEIGHT),
    )
}

impl Content for Player {
    /// The size before the compositor tiles it: tall enough for the picture
    /// and the transport under it.
    fn default_size(&self) -> (u32, u32) {
        (
            u32::from(ronoctua_tokens::space::WINDOW_PLAYERWIDTHPX),
            u32::from(ronoctua_tokens::space::WINDOW_PLAYERHEIGHTPX),
        )
    }

    /// THE CORNER'S SIZE IS THE WINDOW'S OWN: a floating window's size is the
    /// one its commits carry, and committing the tiled size it had been
    /// configured made the corner come out 512 by 768 — measured in the VM.
    fn own_size(&self) -> Option<(u32, u32)> {
        (self.stage == Stage::Small).then(small_size)
    }

    /// THE SCREEN LEFT BY ANOTHER WAY — the compositor's own key, another
    /// window taking it — takes the stage with it, or the door would sit on
    /// "whole screen" in a window.
    fn window_configured(
        &mut self,
        _: &mut Ui,
        cfg: &smithay_client_toolkit::shell::xdg::window::WindowConfigure,
    ) {
        let now = cfg.is_fullscreen();
        if self.fullscreen && !now && matches!(self.stage, Stage::Maximized | Stage::Listing) {
            self.stage = Stage::Off;
            self.controls_until = None;
        }
        self.fullscreen = now;
    }

    type Hand = PerPointer;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::PLAYER_TITLE)
    }

    /// The window: an `xdg_toplevel`, kept to be made whole-screen or small.
    /// (The column is `crate::column`'s, around the player.)
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        let xdg = XdgShell::bind(wiring.globals, wiring.qh)
            .context("the compositor does not expose xdg_shell")?;
        let window = xdg.create_window(surface, WindowDecorations::RequestServer, wiring.qh);
        window.set_title(Content::title(self, &wiring.texts));
        window.set_app_id(APP_ID);
        window.commit();
        self.window.replace(Some(window.clone()));
        Ok(Shell::Window {
            window,
            _global: xdg,
        })
    }

    fn margin(&self) -> u16 {
        MARGIN
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        self.typed_here(ui, event, mods);
    }

    /// What was typed on this door's line — the ONE function the footer's
    /// minibuffer reaches too (`player_question::answered`).
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let errand = self.view.answered(
            id,
            text,
            &mut self.pairing,
            self.player.as_ref(),
            ui.columns,
            &ui.texts,
        );
        self.apply(ui, errand);
    }

    /// The line was let go: a pairing's name goes with it — and an adult
    /// listing handed over whose password it asked, with the window opened
    /// for it.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.view.let_go();
        self.pairing.let_go();
        self.listing_stage_holds(ui);
    }

    /// The clipboard, read for the text being edited (`Errand::Paste`).
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        if self.view.text_pasted(text) {
            ui.ask_paint();
        }
    }

    /// CAPS LOCK MEANS CAPITALS while a text is being edited — and nowhere
    /// else here, where a letter is a key of the view.
    fn types_capitals(&self) -> bool {
        self.view.editing_text()
    }

    fn resized(&mut self, ui: &mut Ui) {
        self.configured(ui);
    }

    fn each_turn(&mut self, ui: &mut Ui) {
        self.each_turn_here(ui);
    }

    /// NO SWEEP OF TEXT HERE: a held button is this window's own gesture
    /// (the timeline and the volume are dragged), and a sweep would take its motion away.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<PerPointer>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.button_under = hand.extra.button_under;
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        self.paint_in(ui, canvas, &Area::whole());
    }

    /// A SCREEN described, renamed or gone, or the window on another: what is
    /// composed here follows, and a lane's page lists them anew.
    fn screens_changed(&mut self, ui: &mut Ui) {
        self.screens_follow(ui);
        ui.ask_paint();
    }
}

impl crate::column::Tenant for Player {
    type Hand = PerPointer;

    const WHO: crate::right_column::Tenant = crate::right_column::Tenant::Player;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::PLAYER_TITLE)
    }

    /// Every size: the three widths adapt, and the bar is a remote control
    /// of its own.
    fn shape(&self, size: Size) -> Option<crate::column::Shape> {
        Some(if size == Size::Bar {
            crate::column::Shape::Own
        } else {
            crate::column::Shape::Adapted
        })
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>, area: &Area) {
        if area.size == Size::Bar {
            self.paint_bar(ui, canvas, area);
        } else {
            self.paint_in(ui, canvas, area);
        }
    }

    fn pointer(
        &mut self,
        ui: &mut Ui,
        hand: &mut Hand<PerPointer>,
        events: &[PointerEvent],
        area: &Area,
    ) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        if area.size == Size::Bar {
            self.pointer_bar(ui, hand, events);
        } else {
            self.pointer_here(ui, hand, events);
        }
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.button_under = hand.extra.button_under;
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers, area: &Area) {
        // IN THE BAR `Esc` CLOSES THE COLUMN, whatever page is open under it:
        // the bar draws no page, and a key that went back through pages
        // nobody sees left the column up and did nothing a person could see.
        if area.size == Size::Bar && event.keysym == Keysym::Escape {
            self.apply(ui, crate::player::Errand::Close);
            return;
        }
        self.typed_here(ui, event, mods);
    }

    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        Content::answered(self, ui, id, text);
    }

    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        Content::unanswered(self, ui, id);
    }

    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        Content::pasted(self, ui, text);
    }

    /// AS IN THE WINDOW: capitals while a text is being edited.
    fn types_capitals(&self) -> bool {
        Content::types_capitals(self)
    }

    /// NO SWEEP OF TEXT, as in the window.
    fn text_area(&self, _: &Ui, _: u16, _: u16, _: &Area) -> Option<crate::text_select::Area> {
        None
    }

    fn each_turn(&mut self, ui: &mut Ui) {
        self.each_turn_here(ui);
    }

    fn resized(&mut self, ui: &mut Ui, _: &Area) {
        self.configured(ui);
    }

    /// OFF THE SCREEN NOTHING IS DECODED for it: a picture's thread ends
    /// with the picture, and the next frame drawn starts them again.
    fn parked(&mut self, _: &mut Ui) {
        self.pictures.clear();
        self.reach.sinks(false, &mut self.sinks);
    }

    /// THE COLUMN CLOSES and the pictures go with it, now: what plays goes
    /// on playing, and nothing decodes for a column that is not there.
    fn closing(&mut self, _: &mut Ui) {
        self.pictures.clear();
    }

    /// A NEW SURFACE IS A NEW GPU SURFACE: everything drawn on the old one is
    /// made again on the first frame of this one.
    fn shown(&mut self, _: &mut Ui) {
        for pic in &mut self.pictures {
            pic.uploaded = false;
        }
    }

    /// THE COLUMN'S SCREEN, as its base knows it: what is routed there is
    /// what it shows.
    fn placed_on(&mut self, ui: &mut Ui, on: Option<&str>) {
        self.placed_on = on.map(ToOwned::to_owned);
        self.screens_follow(ui);
        ui.ask_paint();
    }

    fn screens_changed(&mut self, ui: &mut Ui) {
        self.screens_follow(ui);
        ui.ask_paint();
    }

    /// IN THE BAR, a key acts when it is the view's everywhere
    /// (`player::Where::Anywhere`) — play, the next, the volume, the mute,
    /// the window's stages — and `Esc`. The cursor's keys walk rows the bar
    /// does not draw, `o` shows a picture it has no room for, and the rest
    /// act on a row; those put the column back instead.
    fn acts_without_rows(&self, key: ronoctua_keymap::Key) -> bool {
        use crate::player::{Press, Unseen, press_of_key};
        match press_of_key(key) {
            None => true,
            // THE PICTURE ALONE needs a picture, and the bar has no room for
            // one: the column comes back.
            Some(Press::Only) => false,
            // THE ONE ANSWER the window's stage and the footer's `o` read.
            Some(press) => press.unseen() == Unseen::Acts,
        }
    }

    /// A QUESTION OF THE VIEW'S — "already in the queue?", "forget the key?",
    /// a text being edited — never waits unseen in the bar: the column comes
    /// back to its size.
    fn waits_for_an_answer(&self) -> bool {
        self.view.waits_for_an_answer()
    }
}

impl Player {
    /// One frame of the view — the window's whole surface, or a column's
    /// area at its three widths: the header on `area.top`, the picture under
    /// it, the transport, and the rows.
    #[allow(clippy::too_many_lines)]
    fn paint_in(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>, area: &Area) {
        if ui.message.is_some() {
            ui.hinted.say(None);
        }
        self.relayout(ui, area);
        let size = tokens::text::size();
        let line = line_px();
        let rows = self.rows(ui);
        let room = self.room(ui);
        ui.offset = ui.offset.min(rows.len().saturating_sub(room));
        // THE SOUND OUTPUTS, held while a row of them is on screen and let go
        // when none is (`player_lane::sinks_in_sight`) — read here, where the
        // rows on screen are known.
        let in_sight =
            !self.on_stage() && crate::player_lane::sinks_in_sight(&rows, ui.offset, room);
        self.reach.sinks(in_sight, &mut self.sinks);
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        let span =
            |text: String, column: u16, line: u16, color: tokens::Color| ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text,
                column,
                line,
                color,
                edge: false,
                foot: false,
            };

        canvas.under.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            color: tokens::color::SURFACE_CHROME,
        });

        // THE BUTTONS, PLACED FIRST — the transport's and the volume's glyph
        // — and the one under a pointer that has NOT moved read again against
        // them: the media glyph comes and goes with the file, every button
        // after it moves three cells, and a resting hand kept lighting and
        // explaining the button that had been there while a press took the
        // one that is (`the-pointer-lights-what-the-click-takes`).
        let stage = self.on_stage();
        let shown = self.controls_shown();
        (self.buttons, self.clock) = self.placed_buttons(ui, shown);
        self.button_under = self.pointer_at.and_then(|(x, y)| self.button_at(x, y));
        // AND THE DECK'S CONTROL under a resting pointer, read against the
        // deck just laid out: a state that moved the playhead under the hand
        // is what the hand now points at.
        self.deck_under = self
            .pointer_at
            .and_then(|(x, y)| self.deck_hover(ui, &rows, x, y));
        // THE HEADER'S LINE — 0 in the window, 1 in a column: the keys, what
        // the last order said, or — with the pointer on a button or on a
        // control of a deck — what that does.
        let under_button = self.button_under;
        // AND A CONTROL OF A ROW — a title's glyph — read against the rows
        // just built, as the button is: what a click there would take.
        let under_control = self
            .pointer_at
            .and_then(|(x, y)| self.control_at(ui, &rows, x, y));
        let pointed = match under_button {
            Some(id) => crate::chrome::hint_sentence(id, &ui.texts),
            None => self.deck_under.and_then(|(_, _, hit)| {
                crate::player_timeline::hover_sentence(hit, self.player.as_ref(), &ui.texts)
            }),
        }
        .or_else(|| under_control.and_then(|id| crate::chrome::hint_sentence(id, &ui.texts)));
        let question = crate::door::list::the_question(ui);
        let (pieces, hinting) = match (question, &ui.message, pointed) {
            // A QUESTION BEING TYPED OUTRANKS THE REST: the keyboard is its.
            (Some(q), _, _) => (q, true),
            (None, Some(t), _) => (vec![crate::chrome::Piece::said(t.clone())], false),
            (None, None, Some(said)) => (vec![crate::chrome::Piece::said(said)], false),
            // THE WAY BACK FIRST, and clickable: `Esc` leaves a page of the
            // view, and a pointer that could not leave one would go in and
            // stay. AND IN A COLUMN, its own keys after the view's.
            (None, None, None) => {
                let mut hint = crate::chrome::with_the_way_back(
                    self.view.hint(self.player.as_ref(), ui.columns, &ui.texts),
                    &ui.texts,
                );
                let column = area.teach(&ui.texts);
                if !hint.is_empty() && !column.is_empty() {
                    hint.push(crate::chrome::Piece::separator());
                }
                hint.extend(column);
                (hint, true)
            }
        };
        // ON STAGE, what is drawn over the picture: the translucent band
        // under the header and the transport, and the time bar — after the
        // images, or the picture would cover them.
        let mut top: Vec<ronoctua_render::quad::Quad> = Vec::new();
        if shown {
            canvas.spans.extend(
                ui.hinted
                    .head(&pieces, hinting, MARGIN, ui.columns, &ui.texts),
            );
        } else {
            // HIDDEN CONTROLS ARE NOT CONTROLS, the header's shortcuts
            // included: a click on the picture's top line is on the picture.
            ui.hinted.took(Vec::new());
        }
        if stage && shown {
            let header = line * f32::from(1 + ui.hinted.rows());
            let foot = line * f32::from(self.layout.transport);
            for (y, height) in [(area.top_px, header), (foot, h - foot)] {
                top.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y,
                    width: w,
                    height,
                    color: tokens::color::SURFACE_TRANSPORT,
                });
            }
        }

        // THE PICTURES: the band they are framed on, and each frame in the
        // place `relayout` gave it (`player_layout::placed`). The band is a
        // surface of the theme in the door, and the content's ground on
        // stage, where pictures alone are framed in black.
        //
        // THE THEME'S DESK AS THE LETTERBOX, deliberately, on every page: it
        // is a frame for a picture and carries no letter (`every-ground-is-
        // measured` names it among the grounds nothing is written on). On the
        // desk's own page it is the window's ground, and the letterbox simply
        // reads as the page around the film.
        if let Some(band) = self.band_area(ui, area).filter(|_| !self.placed.is_empty()) {
            canvas.under.push(ronoctua_render::quad::Quad {
                x: band.x,
                y: band.y,
                width: band.width,
                height: band.height,
                color: if stage {
                    tokens::color::SURFACE_BASE
                } else {
                    tokens::color::SURFACE_BACKDROP
                },
            });
        }
        // ONE KEY A LANE A SCREEN (`Picture::key`): the screen this door
        // shows, or the surface's own while it knows none.
        let output = self
            .screen(ui)
            .unwrap_or_else(|| crate::player_picture::THIS_SURFACE.to_owned());
        for (lane, rect) in &self.placed {
            let Some(p) = self.pictures.iter_mut().find(|p| p.of == Some(*lane)) else {
                continue;
            };
            if let Some(f) = &p.frame {
                // A NEW FRAME UNDER THE SAME KEY is sent again: the key is the
                // picture, and what is in it moves every frame.
                let key = p.key(&output);
                if !p.uploaded || canvas.images.missing(&key) {
                    canvas
                        .images
                        .upload_it(canvas.device, canvas.queue, &key, f);
                    p.uploaded = true;
                }
                // THE FRAME IS ALREADY ITS PLACE'S SHAPE (the player rendered
                // it at this size); `picture_in` only keeps it inside the place
                // while a new size is on its way.
                let fit =
                    crate::chrome::picture_in(rect.width, rect.height, (f.width, f.height), false);
                canvas.pictures.push((
                    key,
                    ronoctua_render::image::Area {
                        x: rect.x + fit.x,
                        y: rect.y + fit.y,
                        width: fit.width,
                        height: fit.height,
                    },
                ));
            }
        }

        // THE TRANSPORT: glyph buttons, then the clock — drawn where
        // `placed_buttons` put them, the answer the pointer reads back.
        let tl = self.layout.transport;
        let s = self.player.as_ref();
        let transport: Vec<crate::player_media::Placed> = self
            .buttons
            .iter()
            .copied()
            .filter(|(line, _, _, _)| *line == tl)
            .collect();
        for (line, at, _, id) in &transport {
            let colour = if under_button == Some(*id) || lit(id, s, self.stage) {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_PRIMARY
            };
            canvas.spans.push(span(
                glyph_of(id, s, self.stage, self.door).to_string(),
                *at,
                *line,
                colour,
            ));
        }
        // THE CLOCK WHERE `transport` PUT IT — between its two seeks, or
        // alone where they do not fit, or not at all: the footer's panel's
        // own answer (`player_media::transport`). The small window in the
        // corner is narrower than the buttons and the clock, and a clock cut
        // at the edge reads as a wrong time.
        if let (Some(clock), Some((line, from))) =
            (s.and_then(crate::player_media::clock_text), self.clock)
        {
            canvas
                .spans
                .push(span(clock, from, line, tokens::color::TEXT_SECONDARY));
        }

        // THE DECK under the transport: the time bar — the part played in the
        // accent, the player's mute and volume at its end — or a set's ruler
        // and a line a lane, the playhead one line across them all, drawn
        // from the deck `relayout` laid out: the one the pointer reads. A
        // band being dragged shows its ghost. Under the pictures in the door;
        // over the picture on stage, and there only the one bar. Hidden with
        // the rest of the controls.
        let f = ui.frame(0);
        let sound = s.map(|s| crate::player_timeline::Sound {
            volume_percent: s.volume,
            muted: s.muted,
        });
        if shown {
            let block = self.block_drawn(ui);
            if stage {
                top.extend(block.quads);
            } else {
                canvas.under.extend(block.quads);
            }
            // THE LANES' BAR IS A CONTROL, and its column its own.
            canvas.bar(block.bar.map(crate::bars::Track::of));
            canvas.spans.extend(block.spans);
        }

        // THE ROWS, under all of it, scrolling — and a subtitle lane's colour
        // chip under its glyph (`chrome::chip`, the footer's drawing too),
        // painted over the bands below.
        let first = self.layout.list;
        let mut chips = Vec::new();
        // THE GLOBE'S BOX among the rows on screen, once for its run of lines
        // (`player_globe::box_of`): what is painted in it and what the
        // pointer reads back.
        let mut globe_box: Option<(ronoctua_render::image::Area, u32)> = None;
        for (i, row) in rows.iter().skip(ui.offset).take(room).enumerate() {
            let y = first + u16::try_from(i).unwrap_or(0);
            if let Some(g) = &row.globe {
                let starts = i
                    .checked_sub(1)
                    .and_then(|k| rows.get(ui.offset + k))
                    .is_none_or(|r| r.globe.is_none());
                if starts {
                    let run = rows
                        .iter()
                        .skip(ui.offset + i)
                        .take(room - i)
                        .take_while(|r| r.globe.is_some())
                        .count();
                    globe_box = Some(crate::player_globe::box_of(
                        g,
                        run,
                        (f32::from(MARGIN) * cell_px(), f32::from(y) * line),
                        (cell_px(), line),
                    ));
                }
            }
            for f in &row.fields {
                canvas
                    .spans
                    .push(span(f.text.clone(), MARGIN + f.at, y, f.color));
            }
            // A LINE OF A DECK — the timeline's rows: its shapes under its
            // words, from the deck the rows were built with. The playhead is
            // one line across the deck's rows on screen: each run of them is
            // drawn at once, from its first.
            if let Some(d) = &row.deck {
                let before = i
                    .checked_sub(1)
                    .and_then(|k| rows.get(ui.offset + k))
                    .and_then(|r| r.deck.as_ref());
                let starts_a_run = before
                    .is_none_or(|b| !std::rc::Rc::ptr_eq(&b.deck, &d.deck) || b.line + 1 != d.line);
                if starts_a_run {
                    let run = rows
                        .iter()
                        .skip(ui.offset + i)
                        .take(room - i)
                        .map_while(|r| r.deck.as_ref())
                        .take_while(|r| std::rc::Rc::ptr_eq(&r.deck, &d.deck))
                        .count();
                    let deck = d.deck.holding(self.dragged);
                    canvas.under.extend(deck.quads(
                        f,
                        y,
                        MARGIN + d.left,
                        d.line..d.line + run,
                        sound,
                    ));
                }
            }
            // A LINE OF THE LAYOUT'S SCHEMATIC: its tiles, once for each run
            // of its lines on screen, from the first — cut to what is shown.
            if let Some(sc) = &row.schematic {
                let before = i
                    .checked_sub(1)
                    .and_then(|k| rows.get(ui.offset + k))
                    .and_then(|r| r.schematic.as_ref());
                if before.is_none_or(|b| !std::rc::Rc::ptr_eq(&b.figure, &sc.figure)) {
                    let run = rows
                        .iter()
                        .skip(ui.offset + i)
                        .take(room - i)
                        .map_while(|r| r.schematic.as_ref())
                        .take_while(|r| std::rc::Rc::ptr_eq(&r.figure, &sc.figure))
                        .count();
                    canvas.under.extend(sc.figure.quads(
                        (f32::from(MARGIN + sc.left) * cell_px(), f32::from(y) * line),
                        (sc.line, u16::try_from(run).unwrap_or(u16::MAX)),
                        (cell_px(), line),
                    ));
                }
            }
            // A LINE OF A COVER — of each picture on it, two side by side
            // where a suggested cover stands beside the file's own: the
            // picture the view holds, once for each run of its lines on
            // screen, from the first — cut to what shows.
            for cr in &row.covers {
                let of = |r: &crate::player_rows::Row| {
                    r.covers.iter().find(|c| c.key == cr.key).cloned()
                };
                let before = i
                    .checked_sub(1)
                    .and_then(|k| rows.get(ui.offset + k))
                    .and_then(of);
                if before.is_none_or(|b| b.line + 1 != cr.line) {
                    let run = rows
                        .iter()
                        .skip(ui.offset + i)
                        .take(room - i)
                        .map_while(of)
                        .count();
                    if let Some(picture) = self.cover_drawn(cr, run, y, canvas) {
                        canvas.pictures.push(picture);
                    }
                }
            }
            for chip in &row.chips {
                chips.push(crate::chrome::chip(
                    f32::from(MARGIN + chip.at) * cell_px(),
                    f32::from(y) * line,
                    cell_px(),
                    line,
                ));
            }
            // THE CARET of a text being edited, where its row says.
            if let Some(at) = row.caret {
                chips.push(crate::chrome::caret(
                    f32::from(MARGIN) * cell_px(),
                    f32::from(y) * line,
                    cell_px(),
                    line,
                    at,
                ));
            }
        }
        // THE GLOBE, measured in the box its lines on screen make, asked of its
        // thread — or drawn in place by a door with none — and painted there:
        // a moving picture, sent again each frame under its one key.
        self.globe_drawn = None;
        if let Some((area, gone)) = globe_box.filter(|_| !stage) {
            self.view.globe_measured(
                &rows,
                crate::fields::globe_texture(area),
                gone,
                tokens::theme::palette(),
                crate::globe::tongue(&ui.texts),
            );
            match self.globe_stage.as_mut() {
                Some(stage) => {
                    if let Some(ask) = self.view.wants_globe() {
                        stage.tell(ask);
                    }
                }
                None => self.view.globe_drawn_here(),
            }
            if let Some((key, rgba)) = self.view.globe_picture() {
                let drawn = crate::sizes::picture_in(area, rgba.width, rgba.height);
                canvas
                    .images
                    .upload_it(canvas.device, canvas.queue, &key, &rgba);
                canvas.pictures.push((key, drawn));
                self.globe_drawn = Some((drawn, (rgba.width, rgba.height)));
            }
        }
        // THE BAR'S COLUMN IS ITS OWN WHILE IT IS DRAWN, and only then —
        // written down here, where it is drawn, for the pointer to read. On
        // stage there is no list and no bar.
        let scroll = crate::chrome::list_scroll(rows.len(), room, ui.offset);
        ui.room = if stage {
            crate::chrome::Room::default()
        } else {
            crate::chrome::Room::of(scroll, w, h)
        };
        // A ROW OF SEVERAL LINES — a label or a value wrapped, what is said
        // under a result — lights WHOLE under the pointer, as a click takes
        // it whole: one lit line of three said the click takes one.
        //
        // AND A FIGURE LIGHTS NONE, here too and not only where the pointer
        // moved: a pointer that stayed still while a key changed the page
        // under it may rest on a shelf of posters now, and a band drawn over
        // the pictures hid them.
        let figure = |n: usize| {
            rows.get(n).is_some_and(|r| {
                r.deck.is_some()
                    || r.schematic.is_some()
                    || (!r.covers.is_empty() && !matches!(r.lit, crate::player_rows::Lit::From(_)))
                    || r.globe.is_some()
            })
        };
        let lit = canvas
            .lit
            .iter()
            .zip(canvas.lit_x.iter().copied().chain(std::iter::repeat(None)))
            .flat_map(|(&(colour, row), x)| {
                let lines = crate::player_rows::whole_row(&rows, row);
                let figured = lines.clone().any(figure);
                lines
                    .filter(move |_| !figured)
                    .map(move |line| (colour, Some(line), x))
            })
            // WHAT EACH LINE LIGHTS (`player_rows::lighting`): a line beside
            // a figure from its words on, wherever on the row the pointer
            // rests — so its band stands at its words' first column — and a
            // line of controls with none lights nothing.
            .filter_map(|(colour, line, x)| {
                match line.map(|n| crate::player_rows::lighting(&rows, n)) {
                    Some(crate::player_rows::Lighting::From(column)) => {
                        let (at, _) = across().band_of(column, 1);
                        Some((colour, line, Some(at + cell_px() / 2.0)))
                    }
                    Some(crate::player_rows::Lighting::Cell(cells)) if cells.is_empty() => None,
                    _ => Some((colour, line, x)),
                }
            });
        // A GRID'S ROW LIGHTS THE ONE CELL under the pointer — the picker's
        // files — by the answer the click takes it with (`across`); a line
        // beside a figure, from its words on; every other row, whole.
        let bands = crate::chrome::hover_cells(
            lit,
            |n| match crate::player_rows::lighting(&rows, n) {
                crate::player_rows::Lighting::Whole => Vec::new(),
                crate::player_rows::Lighting::Cell(cells) => cells,
                crate::player_rows::Lighting::From(column) => {
                    vec![(column, u16::MAX - column)]
                }
            },
            ui.offset,
            room,
            rows.len(),
            first,
            ui.room.band_width(),
            across(),
        );
        // A GRID'S CURSOR CELL WEARS THE SELECTION, the file manager's cursor,
        // on its own cell — under the hover, which says where the pointer is.
        let selection = crate::chrome::cell_selection(
            crate::player_rows::grid_cursor(&rows),
            ui.offset,
            room,
            first,
            across(),
        );
        // UNDER THE PICTURES the grounds and the time bar; OVER them the bands
        // and the bars; the text last. No depth test: later is on top.
        if !stage || shown {
            canvas.over.extend(top);
        }
        canvas.over.extend(selection);
        canvas.over.extend(bands);
        canvas.over.extend(chips);
        if !stage {
            canvas
                .over
                .extend(crate::chrome::list_bar(scroll, first, w, h));
            canvas.bar(crate::chrome::list_track(scroll, first, w, h));
        }
        if shown {
            canvas.over.extend(ui.hinted.bar(ui.frame(0)));
        }

        self.painted = true;
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    /// FAILS IF: a photograph's door — which reaches no machine — writes a
    /// key, or keeps WHAT was typed anywhere it says or prints, or stops
    /// saying the consent and the key it would have kept, in order, then the
    /// ask that follows them.
    #[test]
    fn a_kept_door_keeps_which_key_and_never_the_key() {
        use crate::player::Errand;
        use ronoctua_lookup::{Secret, Service, keys::Key};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut out = Outbox::default();
        let said = out.carry_out(
            Errand::Let {
                services: vec![Service::Tmdb],
                directories: Vec::new(),
                on: true,
                then: Box::new(Errand::KeepKey {
                    key: Key::Tmdb,
                    secret: Secret::new("eyJh.very.secret"),
                }),
            },
            &t,
        );
        assert_eq!(
            said,
            Ok(Some(t.t_with(
                crate::key::PLAYER_KEY_CHECKING,
                &[("service", "TMDB")]
            )))
        );
        assert_eq!(out.lets, vec![(vec![Service::Tmdb], Vec::new(), true)]);
        assert_eq!(out.keys_kept, vec![Key::Tmdb]);
        assert_eq!(
            out.asked,
            vec![Ask::KeyCheck {
                service: Service::Tmdb
            }]
        );
        assert!(
            !format!("{out:?}").contains("very.secret"),
            "a key is kept where it is printed"
        );
        assert_eq!(
            out.carry_out(Errand::ForgetKey(Key::SubDl), &t),
            Ok(Some(t.t(crate::key::PLAYER_KEY_FORGOTTEN)))
        );
        assert_eq!(out.keys_forgotten, vec![Key::SubDl]);
    }

    /// A column's area at `size`, the header on line 1 as the column puts it.
    fn column(size: Size) -> Area {
        Area::of_a_column(size, 1, line_px())
    }

    #[test]
    fn with_no_video_the_transport_is_under_the_header() {
        let l = layout(&column(Size::Medium), 2, 40, 50, 2.0, None, 1);
        assert!(l.band.is_none());
        assert_eq!((l.transport, l.bar, l.list), (2, 3, 4));
    }

    #[test]
    fn a_wide_video_takes_the_height_its_shape_wants() {
        // 60 cells wide, a line twice a cell: 16:9 wants 60 × 9/16 / 2 ≈ 17.
        let l = layout(&Area::whole(), 1, 60, 60, 2.0, Some(16.0 / 9.0), 1);
        assert_eq!(l.band, Some((1, 17)));
        assert_eq!(l.transport, 18);
    }

    /// FAILS IF: the pictures take more than half the surface, or a column's
    /// pictures stop taking its SIZE's share — medium's lines at medium, more
    /// at large, fewer at small.
    #[test]
    fn the_pictures_never_take_more_than_half() {
        let tall = Some(1080.0 / 1920.0);
        let l = layout(&Area::whole(), 1, 20, 200, 2.0, tall, 1);
        assert_eq!(l.band.map(|b| b.1), Some(10));
        for size in [Size::Large, Size::Medium, Size::Small] {
            let c = layout(&column(size), 2, 80, 50, 2.0, tall, 1);
            assert_eq!(c.band.map(|b| b.1), Some(size.picture_lines()), "{size:?}");
            let short = layout(&column(size), 2, 10, 50, 2.0, tall, 1);
            assert_eq!(short.band.map(|b| b.1), Some(5), "{size:?} past half");
        }
        assert_eq!(Size::Medium.picture_lines(), tokens::space::COLUMN_PICTURES);
    }

    /// The screens a photograph's door is told of: the built-in one and a
    /// monitor.
    fn two_screens() -> Vec<crate::player_lane::Known> {
        vec![
            crate::player_lane::Known {
                connector: "eDP-1".into(),
                description: None,
            },
            crate::player_lane::Known {
                connector: "DP-2".into(),
                description: Some("A wide monitor".into()),
            },
        ]
    }

    /// The lanes a door placed, in the order drawn.
    fn placed_lanes(w: &Player) -> Vec<u32> {
        w.placed.iter().map(|(l, _)| l.0).collect()
    }

    /// FAILS IF: the window shows a picture routed to another screen, stops
    /// showing one routed to its own, stops placing them in the band in the
    /// form of what plays and in each picture's own shape, asks a picture's
    /// frames of any lane but its own, or a window that does not know its
    /// screen stops showing them all.
    #[test]
    fn only_lanes_routed_here_are_tiles_in_the_window() {
        let s = crate::player::fixture::routed();
        let (mut w, mut ui) = a_photographs(Door::Window, s);
        w.relayout(&ui, &Area::whole());
        assert_eq!(
            placed_lanes(&w),
            vec![1, 2, 3],
            "no screen known: every one"
        );
        w.given_screens(&mut ui, two_screens(), Some("DP-2".into()));
        w.relayout(&ui, &Area::whole());
        assert_eq!(placed_lanes(&w), vec![1, 2]);
        let of: Vec<Option<u32>> = w.pictures.iter().map(|p| p.of.map(|l| l.0)).collect();
        assert_eq!(of, vec![Some(1), Some(2)], "a picture a lane, by its lane");
        w.given_screens(&mut ui, two_screens(), Some("eDP-1".into()));
        w.relayout(&ui, &Area::whole());
        assert_eq!(placed_lanes(&w), vec![1, 3]);
        // SIDE BY SIDE in the band, each in its own shape.
        let (first, lines) = w.layout.band.expect("a band");
        let top = line_px() * f32::from(first);
        let bottom = top + line_px() * f32::from(lines);
        let (a, b) = (w.placed[0].1, w.placed[1].1);
        assert!(a.right() <= b.x, "{a:?} {b:?}");
        assert!((a.width / a.height - 16.0 / 9.0).abs() < 0.01);
        assert!(
            (b.width / b.height - 1.0).abs() < 0.01,
            "the cover is square"
        );
        for (_, r) in &w.placed {
            assert!(r.y >= top - 0.01 && r.bottom() <= bottom + 0.01, "{r:?}");
        }
        assert_eq!(
            w.outbox()
                .map(|o| o.laid_out.iter().copied().collect::<Vec<_>>()),
            Some(vec![1, 2, 3]),
            "every picture laid out asked its own lane"
        );
    }

    /// FAILS IF: the column stops showing what is routed to the screen its
    /// base says it is on, or a window opened for ONE screen shows anything
    /// but what goes there.
    #[test]
    fn only_lanes_routed_here_are_tiles_in_the_column_and_a_screen_s_window() {
        let s = crate::player::fixture::routed();
        let (mut c, mut ui) = a_photographs(Door::Column, s.clone());
        crate::column::Tenant::placed_on(&mut c, &mut ui, Some("DP-2"));
        c.relayout(&ui, &column(Size::Medium));
        assert_eq!(placed_lanes(&c), vec![1, 2]);
        crate::column::Tenant::placed_on(&mut c, &mut ui, Some("HDMI-A-1"));
        c.relayout(&ui, &column(Size::Medium));
        assert_eq!(placed_lanes(&c), vec![1]);
        let (mut w, ui) = a_photographs(Door::Window, s);
        w.screen_wanted = Some("eDP-1".into());
        w.relayout(&ui, &Area::whole());
        assert_eq!(placed_lanes(&w), vec![1, 3]);
    }

    /// FAILS IF: "show on every screen" stops being the door's own — the
    /// screens it knows, each something goes to a window — or one screen
    /// stops being said instead of opening a window on it.
    #[test]
    fn every_screen_opens_a_window_where_something_goes() {
        use crate::player::Press;
        let mut s = crate::player::fixture::routed();
        let (mut w, mut ui) = a_photographs(Door::Window, s.clone());
        w.given_screens(&mut ui, two_screens(), Some("eDP-1".into()));
        w.pressed(&mut ui, Press::EveryScreen);
        assert_eq!(
            w.outbox().map(|o| o.every_screen.clone()),
            Some(vec![vec!["eDP-1".to_owned(), "DP-2".to_owned()]])
        );
        assert!(ui.message.is_some(), "it says what it did");
        // ONE SCREEN: said, and nothing opened.
        let (mut one, mut ui) = a_photographs(Door::Window, s.clone());
        one.given_screens(&mut ui, two_screens()[..1].to_vec(), Some("eDP-1".into()));
        one.pressed(&mut ui, Press::EveryScreen);
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::PLAYER_EVERY_SCREEN_ONE).as_str())
        );
        assert!(one.outbox().is_some_and(|o| o.every_screen.is_empty()));
        // NOTHING WITH A PICTURE: said too.
        s.lane_states.clear();
        let (mut none, mut ui) = a_photographs(Door::Window, s);
        none.given_screens(&mut ui, two_screens(), None);
        none.pressed(&mut ui, Press::EveryScreen);
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::PLAYER_EVERY_SCREEN_NOTHING).as_str())
        );
    }

    /// FAILS IF: a bar button stops being taken across the whole line —
    /// the bar is five cells, and a target the size of the glyph is one to
    /// aim at — or a line of air answers as a button.
    #[test]
    fn a_bar_button_is_taken_across_its_line() {
        let (mut w, ui) = a_window(State::default());
        w.buttons = bar_buttons(1)
            .map(|(line, id)| (line, 0, ui.columns + 2 * MARGIN, id))
            .collect();
        let line = f64::from(line_px());
        let cell = f64::from(cell_px());
        for (l, id) in bar_buttons(1) {
            let y = line * (f64::from(l) + 0.5);
            assert_eq!(w.button_at(cell * 0.5, y), Some(id), "the first cell");
            assert_eq!(w.button_at(cell * 4.5, y), Some(id), "the last cell");
        }
        for air in [5.5, 9.5] {
            assert_eq!(
                w.button_at(cell * 2.5, line * air),
                None,
                "the air at {air}"
            );
        }
        assert_eq!(w.button_at(cell * 2.5, line * 1.5), None, "the air on top");
    }

    /// FAILS IF: the transport stops fitting a column at its small size —
    /// the size exists because the eleven buttons still fit there, the last
    /// one's air on the last cell — or a twelfth would fit too (the room is
    /// exact, and a new button is a decision about what goes), or a button
    /// that does not fit a clamped column is drawn cut.
    #[test]
    fn the_transport_fits_the_small_column() {
        assert_eq!(crate::player_transport::BUTTONS.len(), 11);
        // EVERY BUTTON SHOWN: two sounds make the media in the file one.
        let s = crate::player::fixture::alone(
            "/films/a.mkv",
            [1, 2]
                .map(|id| {
                    crate::player::fixture::track(
                        id,
                        ronoctua_player::wire::TrackKind::Audio,
                        None,
                        id == 1,
                    )
                })
                .to_vec(),
        );
        let panel = |cells| crate::player_media::Panel {
            from: MARGIN,
            cells,
        };
        let small = crate::player_media::buttons(panel(tokens::space::COLUMN_SMALL), 0, Some(&s));
        assert_eq!(small.len(), 11, "{small:?}");
        // ZERO SLACK: the eleventh ends on the last cell, a twelfth is out.
        assert_eq!(
            small.last().map(|b| b.2),
            Some(MARGIN + tokens::space::COLUMN_SMALL)
        );
        let short =
            crate::player_media::buttons(panel(tokens::space::COLUMN_SMALL - 1), 0, Some(&s));
        assert_eq!(short.len(), 10, "a button cut at the edge");
        assert_eq!(
            crate::player_media::buttons(panel(27), 0, Some(&s)).len(),
            9
        );
    }

    /// FAILS IF: in the bar a key acts on a row nobody sees, or a key the
    /// bar does show — play, next, the volume — stops acting there. The
    /// rule is `Where::Anywhere`, less the cursor's keys, the picture, and
    /// saving the queue again — which may ask, on the rows.
    #[test]
    fn in_the_bar_only_the_keys_of_everywhere_act() {
        use crate::column::Tenant as _;
        use crate::player::OF_THE_VIEW;
        let (w, _) = a_window(State::default());
        for (notation, id, at) in OF_THE_VIEW {
            let key = ronoctua_keymap::Key::parse(notation).expect(notation);
            let everywhere = at.of_no_row()
                && !matches!(
                    *id,
                    "player-up" | "player-down" | "player-only" | "player-save-queue-again"
                );
            assert_eq!(w.acts_without_rows(key), everywhere, "{notation} ({id})");
        }
        let esc = ronoctua_keymap::Key::parse("ESC").expect("parses");
        assert!(
            w.acts_without_rows(esc),
            "Esc closes the column in every size"
        );
    }

    /// A film with two sounds: the media in the file is a choice.
    fn with_two_sounds() -> State {
        crate::player::fixture::alone(
            "/films/a.mkv",
            [1, 2]
                .map(|id| {
                    crate::player::fixture::track(
                        id,
                        ronoctua_player::wire::TrackKind::Audio,
                        Some(&format!("sound {id}")),
                        id == 1,
                    )
                })
                .to_vec(),
        )
    }

    /// FAILS IF: on stage — the picture alone — the media glyph or `k` opens
    /// a page behind the picture, or a key that acts on the cursor's row acts
    /// on one nobody sees; or a window opened for its stage, which has no
    /// rows to bring back, leaves it (which closes it) instead of saying so.
    #[test]
    fn on_stage_a_key_that_needs_the_rows_brings_them_back() {
        use crate::player::{PageId, Press, TrackKind};
        let (mut w, mut ui) = a_photographs(Door::Window, with_two_sounds());
        w.stage_to(&mut ui, Stage::Only);
        w.pressed_button(&mut ui, "player-tracks");
        assert_eq!(w.stage, Stage::Off, "the page opened behind the picture");
        assert_eq!(w.view.page(), PageId::Tracks(TrackKind::Sound));
        // A KEY OF THE CURSOR'S ROW brings the rows back and is spent on it.
        w.stage_to(&mut ui, Stage::Only);
        w.pressed(&mut ui, Press::Enter);
        assert_eq!(w.stage, Stage::Off);
        assert!(
            w.asked().is_empty(),
            "acted on a row nobody saw: {:?}",
            w.asked()
        );
        // A KEY OF EVERYWHERE acts on stage.
        w.stage_to(&mut ui, Stage::Only);
        w.pressed(&mut ui, Press::Toggle);
        assert_eq!(w.stage, Stage::Only);
        assert_eq!(w.asked(), [Ask::Toggle]);
        // A WINDOW OPENED FOR ITS STAGE says where the rows are.
        let (mut w, mut ui) = a_photographs(Door::Window, with_two_sounds());
        w.opened_for = Some(Stage::Maximized);
        w.stage_to(&mut ui, Stage::Maximized);
        w.pressed(&mut ui, Press::Tracks);
        assert!(!ui.quit, "the stage's window closed");
        assert_eq!(w.view.page(), PageId::Home);
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::PLAYER_STAGE_HAS_NO_ROWS).as_str())
        );
    }

    /// FAILS IF: a question the view asks late — "already in the queue?",
    /// "write over it?" — leaves the order's "added…" on the line, or waits
    /// on a stage that does not draw the rows it is asked on.
    #[test]
    fn a_question_asked_on_stage_brings_the_rows_back() {
        use ronoctua_player::{
            queue::{EntryId, Name},
            wire::Trouble,
        };
        let (mut w, mut ui) = a_photographs(Door::Window, with_two_sounds());
        w.stage_to(&mut ui, Stage::Only);
        ui.message = Some("added".into());
        let mut again = crate::player::put("/m/two.flac".into(), false);
        if let Ask::Put { once, .. } = &mut again {
            *once = false;
        }
        w.overrule_asked(
            &mut ui,
            again,
            Trouble::AlreadyQueued {
                entry: EntryId(2),
                name: Name::Titled("Two".into()),
            },
        );
        assert_eq!(w.stage, Stage::Off, "asked behind the picture");
        assert_eq!(ui.message, None, "the order's sentence stayed");
        let rows = w.rows(&ui);
        assert_eq!(
            rows[w.view.cursor()].target,
            crate::player::Target::Answer(false),
            "no first"
        );
    }

    /// FAILS IF: `Esc` in the column's bar goes back through a page the bar
    /// does not draw — the column stays up and nothing a person sees changes
    /// — instead of closing the column, as it does on the home.
    #[test]
    fn in_the_bar_esc_closes_the_column_over_any_page() {
        use crate::player::{PageId, Press};
        let (mut c, mut ui) = a_photographs(Door::Column, with_two_sounds());
        c.pressed(&mut ui, Press::Go(PageId::Machines));
        assert_eq!(c.view.page(), PageId::Machines);
        let key = ronoctua_keymap::Key::parse("ESC").expect("parses");
        let (event, mods) = crate::door::keys::event_of(key);
        crate::column::Tenant::typed(&mut c, &mut ui, &event, mods, &column(Size::Bar));
        assert!(ui.quit, "the column stayed");
    }

    /// A window onto a player that answered, with no daemon, no compositor,
    /// no loop and no thread behind it — its timers arm nothing.
    fn a_window(state: State) -> (Player, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::for_tests(texts);
        let (frame_sender, _) = channel::channel::<usize>();
        let (paired, _) = channel::channel::<crate::player_machines::Paired>();
        let (told, _) = channel::channel::<crate::player::Told>();
        let me = Player {
            door: Door::Window,
            arm_controls: Box::new(|| None),
            arm_corner: Box::new(|| false),
            window: RefCell::new(None),
            reach: Reach::machine(),
            view: crate::player::View::fresh(),
            pairing: crate::player_machines::Pairing::default(),
            paired,
            told: crate::player::ToldSender::of_channel(told),
            player: Some(state),
            pictures: Vec::new(),
            placed: Vec::new(),
            screen_wanted: None,
            placed_on: None,
            sinks: None,
            screens_given: None,
            frame_sender,
            buttons: Vec::new(),
            clock: None,
            picture_folded: false,
            layout: Layout::default(),
            deck: blank_deck(),
            deck_offset: 0,
            dragged: None,
            deck_under: None,
            deck_chosen: None,
            stage: Stage::Off,
            first_stage: None,
            opened_for: None,
            controls_until: None,
            controls_timer: None,
            corner_owed: 0,
            fullscreen: false,
            corner_timer: false,
            painted: false,
            button_under: None,
            pointer_at: None,
            globe_stage: None,
            globe_drawn: None,
            _watch: Arc::new(AtomicBool::new(false)),
        };
        (me, ui)
    }

    /// Opens `page` by its row on the home, as two clicks do.
    fn go_to(w: &mut Player, ui: &mut Ui, page: crate::player::PageId) {
        stand_on(w, ui, &crate::player::Target::Go(page));
        w.pressed(ui, crate::player::Press::Enter);
        assert_eq!(w.view.page(), page, "the row did not open its page");
    }

    /// Puts the cursor on the row whose target is `target`, as a click does.
    fn stand_on(w: &mut Player, ui: &Ui, target: &crate::player::Target) {
        let rows = w.rows(ui);
        let at = rows
            .iter()
            .position(|r| &r.target == target)
            .unwrap_or_else(|| panic!("no row for {target:?}"));
        w.view.clicked(at, &rows);
    }

    /// FAILS IF: pairing in the window stops being typed HERE — the name on
    /// this window's line, then the code masked on it — or anything of it goes
    /// to the footer, or a line let go keeps the name for a later code.
    #[test]
    fn pairing_is_typed_on_the_windows_own_line() {
        use crate::player::{PageId, Press, Target};
        use crate::player_question::{PAIR, PAIR_CODE};
        let state = State {
            tailnet: Some("here".into()),
            ..State::default()
        };
        let (mut w, mut ui) = a_window(state);
        go_to(&mut w, &mut ui, PageId::Machines);
        stand_on(&mut w, &ui, &Target::Pair);
        w.pressed(&mut ui, Press::Enter);
        let line = ui.asking.as_ref().expect("the name is asked here");
        assert_eq!((line.id, line.secret), (PAIR, false));
        // THE LINE HANDS BACK what was typed on it, and asks the code next.
        ui.asking = None;
        Content::answered(&mut w, &mut ui, PAIR, "den");
        let line = ui.asking.as_ref().expect("the code is asked here");
        assert_eq!((line.id, line.secret), (PAIR_CODE, true));
        assert!(ui.orders().is_empty(), "the pairing went to the footer");
        // LET GO, the name goes with it: a code typed later pairs with nothing.
        ui.asking = None;
        Content::unanswered(&mut w, &mut ui, PAIR_CODE);
        Content::answered(&mut w, &mut ui, PAIR_CODE, "123456");
        assert!(ui.asking.is_none() && ui.orders().is_empty());
        assert_eq!(ui.message, None);
    }

    /// FAILS IF: a letter typed in this window is not the picker's — a
    /// path is typed HERE, with the files open, and nothing goes to the
    /// footer — or `C-v` does not ask this window's clipboard for the picker.
    #[test]
    fn a_path_is_typed_here_with_the_files_open() {
        use crate::player::{Press, Purpose, Target};
        use ronoctua_keymap::{Key, Sym};
        let root =
            std::env::temp_dir().join(format!("ronoctua-window-library-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Album")).expect("a library");
        std::fs::write(root.join("song.flac"), b"").expect("a song");
        let (mut w, mut ui) = a_window(State::default());
        w.view.pick_at(&root, Purpose::Queue);
        for c in "son".chars() {
            let key = Key::simple(Sym::Char(c));
            let press = w.view.press_of_key(key).expect("a letter is read");
            assert_eq!(press, Press::Pick(key));
            w.pressed(&mut ui, press);
        }
        let rows = w.rows(&ui);
        assert_eq!(
            rows[w.view.cursor()].target,
            Target::Picker(crate::file_picker::Target::File(root.join("song.flac"))),
            "typing narrows to the song"
        );
        assert!(ui.orders().is_empty(), "the letters went to the footer");
        let paste = w
            .view
            .press_of_key(Key::parse("C-v").expect("a key"))
            .expect("C-v is read");
        w.pressed(&mut ui, paste);
        assert!(ui.wants_the_clipboard(), "C-v did not ask the clipboard");
        std::fs::remove_dir_all(&root).expect("the library goes");
    }

    /// FAILS IF: louder and quieter stop being ONE step of the volume, the
    /// same through the bar's buttons, the transport's mapping and the
    /// view's `=` and `-` — or a step goes past the loudest or under nothing.
    #[test]
    fn louder_and_quieter_step_the_volume() {
        use crate::player::{Errand, Press, VOLUME_MOST, VOLUME_STEP};
        use crate::player_transport::{button_errand, stepped_volume};
        assert_eq!(stepped_volume(72, true), 72 + VOLUME_STEP);
        assert_eq!(stepped_volume(72, false), 72 - VOLUME_STEP);
        assert_eq!(stepped_volume(VOLUME_MOST - 1, true), VOLUME_MOST);
        assert_eq!(stepped_volume(VOLUME_STEP - 1, false), 0);
        let state = State {
            volume: 72,
            ..State::default()
        };
        let louder = Ask::Volume {
            percent: 72 + VOLUME_STEP,
        };
        let quieter = Ask::Volume {
            percent: 72 - VOLUME_STEP,
        };
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        assert_eq!(
            button_errand("player-louder", Some(&state), &texts),
            Errand::Tell(louder.clone())
        );
        assert_eq!(
            button_errand("player-quieter", Some(&state), &texts),
            Errand::Tell(quieter.clone())
        );
        // THE VIEW'S KEYS, the same step.
        let mut view = crate::player::View::blank(None);
        assert_eq!(
            view.key(Press::Louder, Some(&state), 40, &texts),
            Errand::Tell(louder.clone())
        );
        assert_eq!(
            view.key(Press::Quieter, Some(&state), 40, &texts),
            Errand::Tell(quieter.clone())
        );
        // AND THE BAR'S BUTTONS, clicked on their lines.
        let (mut c, mut ui) = a_photographs(Door::Column, state);
        c.buttons = bar_buttons(1)
            .map(|(line, id)| (line, 0, ui.columns + 2 * MARGIN, id))
            .collect();
        let line = f64::from(line_px());
        let mut hand = Hand::<PerPointer>::default();
        for l in [6.5, 8.5] {
            let e = PointerEvent {
                surface: ui.surface.clone(),
                position: (f64::from(cell_px()) * 2.5, line * l),
                kind: PointerEventKind::Press {
                    time: 0,
                    button: LEFT_BUTTON,
                    serial: 0,
                },
            };
            c.pointer_bar(&mut ui, &mut hand, &[e]);
        }
        assert_eq!(c.asked(), [louder, quieter]);
    }

    /// FAILS IF: louder or quieter, with no state from the player yet (the
    /// bar after a crash, a door before the first state), sends a volume
    /// guessed from nothing — the player keeps it, and a person at 20% who
    /// pressed "quieter" played next at 95% — instead of saying the player
    /// is not answering. Through the buttons, the view's keys and the wheel.
    #[test]
    fn no_state_no_volume_is_guessed() {
        use crate::player::{Errand, Press};
        use crate::player_transport::button_errand;
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let not_running = texts.t(crate::key::PLAYER_NOT_RUNNING);
        for id in ["player-louder", "player-quieter"] {
            assert_eq!(
                button_errand(id, None, &texts),
                Errand::Said(not_running.clone()),
                "{id}"
            );
        }
        let mut view = crate::player::View::blank(None);
        for press in [Press::Louder, Press::Quieter] {
            assert_eq!(
                view.key(press, None, 40, &texts),
                Errand::Said(not_running.clone()),
                "{press:?}"
            );
        }
        let (mut c, mut ui) = a_photographs(Door::Column, State::default());
        c.player = None;
        c.turn_the_volume(&mut ui, 1);
        assert!(c.asked().is_empty(), "the wheel asked {:?}", c.asked());
        assert_eq!(ui.message, Some(not_running));
    }

    /// FAILS IF: the swap stops trading the door for the other — a window
    /// asking the footer to RAISE the column (never the toggle, which closes
    /// one already up) and closing, a window opened for a stage doing the
    /// same, the column asking for the window and closing — by its button or
    /// by `w`, or it asks the player anything.
    #[test]
    fn swap_from_a_window_raises_the_column_and_quits() {
        let raise = format!("command player-column {}", crate::player::RAISE);
        // THE WINDOW, by its button.
        let (mut w, mut ui) = a_photographs(Door::Window, State::default());
        w.pressed_button(&mut ui, "player-swap");
        assert_eq!(ui.orders(), std::slice::from_ref(&raise));
        assert!(ui.quit);
        assert!(w.asked().is_empty(), "the swap asked the player");
        // BY ITS KEY, `w`.
        let (mut w, mut ui) = a_photographs(Door::Window, State::default());
        let key = ronoctua_keymap::Key::parse("w").expect("parses");
        let (event, mods) = crate::door::keys::event_of(key);
        w.typed_here(&mut ui, &event, mods);
        assert_eq!(ui.orders(), std::slice::from_ref(&raise));
        assert!(ui.quit);
        // A WINDOW OPENED FOR A STAGE closes, which closes the stage.
        let (mut w, mut ui) = a_photographs(Door::Window, State::default());
        w.opened_for = Some(Stage::Maximized);
        w.pressed_button(&mut ui, "player-swap");
        assert_eq!(ui.orders(), [raise]);
        assert!(ui.quit);
        w.opened_for = None;
        // THE COLUMN: the window, and it closes.
        let (mut c, mut ui) = a_photographs(Door::Column, State::default());
        c.pressed_button(&mut ui, "player-swap");
        assert_eq!(ui.orders(), ["command player-window".to_owned()]);
        assert!(ui.quit);
    }

    /// FAILS IF: a door stays open when the player closed everywhere — the
    /// window, a stage's window or the column left over a player that is
    /// gone — or one closes when the player merely went away: a crash is a
    /// player the door says is not running, and nothing closed it on
    /// purpose. The footer reads the same answer (`player::hearing`).
    #[test]
    fn a_farewell_closes_every_door_and_a_crash_does_not() {
        use crate::player::{Hearing, hearing};
        use ronoctua_player::client::Heard;
        let playing = State {
            current: None,
            playing: true,
            ..State::default()
        };
        for door in [Door::Window, Door::Column] {
            // A CRASH: the door stays, and says the player is not running.
            let (mut d, mut ui) = a_photographs(door, playing.clone());
            d.heard(&mut ui, Heard::Gone);
            assert!(!ui.quit, "{door:?} closed on a crash");
            let not_running = ui.texts.t(crate::key::PLAYER_NOT_RUNNING);
            let said = d
                .rows(&ui)
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(said.contains(&not_running), "{door:?}: {said}");
            // AND A STATE AGAIN brings it back, still open.
            d.heard(&mut ui, Heard::State(Box::new(playing.clone())));
            assert!(!ui.quit);
            // A FAREWELL: the door goes, and asks the player nothing on
            // its way out.
            d.heard(&mut ui, Heard::Farewell);
            assert!(ui.quit, "{door:?} stayed after the farewell");
            assert!(d.asked().is_empty());
            assert!(d.pictures.is_empty());
        }
        // A STAGE'S WINDOW closes the same way, which closes the stage.
        let (mut w, mut ui) = a_photographs(Door::Window, playing);
        w.opened_for = Some(Stage::Small);
        w.heard(&mut ui, Heard::Farewell);
        assert!(ui.quit);
        w.opened_for = None;
        // THE FOOTER'S VIEW reads the same three answers.
        assert_eq!(hearing(Heard::Farewell), Hearing::Closes);
        assert_eq!(hearing(Heard::Gone), Hearing::Shows(None));
        assert_eq!(
            hearing(Heard::State(Box::default())),
            Hearing::Shows(Some(Box::default()))
        );
    }

    /// FAILS IF: closing the player everywhere from a window or the column
    /// stops asking first — "no" drawn first and the cursor on it — or, told
    /// yes, the door does not tell the player itself (only if it runs), end
    /// the other doors that are processes itself (so they go with no footer
    /// too), hand the footer the same order for its view, and close; or "no"
    /// does anything but put the question away.
    #[test]
    fn closing_everywhere_asks_then_tells_the_player_and_the_footer() {
        use crate::player::{Press, Target};
        for door in [Door::Window, Door::Column] {
            // WITH NO PLAYER RUNNING the row is there: it still closes the
            // doors that are up.
            let (mut d, mut ui) = a_photographs(door, State::default());
            d.state_arrived(&mut ui, None);
            stand_on(&mut d, &ui, &Target::Quit);
            d.pressed(&mut ui, Press::Enter);
            let rows = d.rows(&ui);
            let answers: Vec<_> = rows
                .iter()
                .filter_map(|r| match r.target {
                    Target::Answer(yes) => Some(yes),
                    _ => None,
                })
                .collect();
            assert_eq!(answers, [false, true], "{door:?}: no is drawn first");
            assert_eq!(
                rows.get(d.view.cursor()).map(|r| &r.target),
                Some(&Target::Answer(false)),
                "{door:?}: the cursor is born on no"
            );
            // NO puts it away, and nothing is done.
            d.pressed(&mut ui, Press::Enter);
            assert!(!ui.quit);
            assert!(d.asked().is_empty());
            assert!(ui.orders().is_empty());
            assert!(d.outbox().is_some_and(|o| !o.ended_the_doors));
            // YES closes the player everywhere.
            stand_on(&mut d, &ui, &Target::Quit);
            d.pressed(&mut ui, Press::Enter);
            d.pressed(&mut ui, Press::Down);
            d.pressed(&mut ui, Press::Enter);
            assert_eq!(d.asked(), [Ask::Quit], "{door:?}");
            assert!(
                d.outbox().is_some_and(|o| o.ended_the_doors),
                "{door:?}: the other doors were left to the footer"
            );
            assert_eq!(ui.orders(), ["command player-quit".to_owned()]);
            assert!(ui.quit, "{door:?} stayed");
        }
    }

    /// A player with NO DAEMON behind it, on a `Ui` of a test.
    /// FAILS IF: the listing's stage stops being a level of its own — `Esc`
    /// reaches the view instead of leaving the whole screen, a window opened
    /// for the stage stays when it is left, a page that is not the listing
    /// keeps the stage, or the column opens it in its strip instead of
    /// handing it to a window.
    #[test]
    fn the_listing_s_stage_is_left_by_esc_and_by_leaving_the_listing() {
        use crate::player::{PageId, Show, View};
        // A LISTING ON ITS PAGE, over channels by theme — where a listing
        // handed over opens (the catalogue's page, `crate::player_seam`):
        // the stage asks only which page is on top.
        let listing = |v: &mut View| {
            v.push(PageId::Channels, None);
            v.push(PageId::Catalog, Some(PageId::Channels.spot().to_owned()));
        };
        let esc =
            || crate::door::keys::event_of(ronoctua_keymap::Key::parse("ESC").expect("parses"));
        // ONLY ITS ROWS: the transport and the deck a whole line past the
        // last one, where not even the sliver of a line under it shows them.
        let l = listing_layout(3, 30);
        assert_eq!(l.list, 3);
        assert!(l.transport > 30 && l.bar > 30 && l.band.is_none(), "{l:?}");
        // THE WINDOW'S OWN: `Esc` leaves the whole screen, the listing stays.
        let (mut w, mut ui) = a_photographs(Door::Window, State::default());
        listing(&mut w.view);
        w.stage_to(&mut ui, Stage::Listing);
        assert_eq!(w.stage, Stage::Listing);
        let (event, mods) = esc();
        w.typed_here(&mut ui, &event, mods);
        assert_eq!(w.stage, Stage::Off);
        assert_eq!(
            w.view.page(),
            PageId::Catalog,
            "the listing went with the stage"
        );
        // A PAGE THAT IS NOT THE LISTING ends the stage.
        w.stage_to(&mut ui, Stage::Listing);
        w.view.push(PageId::Sources, None);
        w.apply(&mut ui, crate::player::Errand::Nothing);
        assert_eq!(w.stage, Stage::Off);
        // A WINDOW OPENED FOR THE STAGE goes with it.
        let (mut w, mut ui) = a_photographs(Door::Window, State::default());
        w.opened_for = Some(Stage::Listing);
        listing(&mut w.view);
        w.stage_to(&mut ui, Stage::Listing);
        let (event, mods) = esc();
        w.typed_here(&mut ui, &event, mods);
        assert!(ui.quit, "the listing's window stayed");
        w.opened_for = None;
        // THE COLUMN hands it to a window opened that way.
        let (mut c, mut ui) = a_photographs(Door::Column, State::default());
        c.stage_to(&mut ui, Stage::Listing);
        assert_eq!(c.stage, Stage::Off, "the column is no whole screen");
        let Reach::Kept(out) = &c.reach else {
            panic!("a photograph's reach");
        };
        assert_eq!(out.stages, [Show::Listing]);
    }

    fn a_photographs(door: Door, state: State) -> (Player, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut ui = Ui::for_tests(texts);
        let mut me = Player::detached(door, Outbox::default(), None);
        me.state_arrived(&mut ui, Some(state));
        (me, ui)
    }

    /// FAILS IF: anything a detached player does past its door is done —
    /// the preferences written, a pairing sent, a window of the stage
    /// started — instead of kept (`Reach::Kept`). Each of them, done, lands
    /// on the machine the tests run on, and `daemon::about_to_act` fails
    /// the test before it does.
    #[test]
    fn a_photographs_player_keeps_every_act_past_its_door() {
        use crate::player::{PageId, Press, Show, Target};
        use crate::player_question::{PAIR, PAIR_CODE};
        let state = State {
            playing: true,
            tailnet: Some("here".into()),
            ..State::default()
        };
        let (mut w, mut ui) = a_photographs(Door::Window, state.clone());
        // A SWITCH writes the preferences, and tells the daemon to read them.
        go_to(&mut w, &mut ui, PageId::Internet);
        stand_on(&mut w, &ui, &Target::Sponsors);
        w.pressed(&mut ui, Press::Enter);
        let out = w.outbox().expect("a photograph's");
        assert_eq!(out.kept, [crate::player::Switch::Sponsors(true)], "{out:?}");
        assert_eq!(out.asked, [Ask::Reload]);
        // A PAIRING, by name — and the code nowhere: the Machines page, back
        // on the home first.
        w.pressed(&mut ui, Press::Escape);
        go_to(&mut w, &mut ui, PageId::Machines);
        stand_on(&mut w, &ui, &Target::Pair);
        w.pressed(&mut ui, Press::Enter);
        ui.asking = None;
        Content::answered(&mut w, &mut ui, PAIR, "den");
        ui.asking = None;
        Content::answered(&mut w, &mut ui, PAIR_CODE, "123456");
        let out = w.outbox().expect("a photograph's");
        assert_eq!(out.paired_with, ["den".to_owned()]);
        assert!(!format!("{out:?}").contains("123456"), "the code was kept");
        // A COLUMN'S WHOLE SCREEN is a window of its own: not started.
        let (mut c, mut ui) = a_photographs(Door::Column, state);
        c.toggle_stage(&mut ui, Show::Maximized);
        assert_eq!(
            c.outbox().expect("a photograph's").stages,
            [Show::Maximized]
        );
        assert_eq!(c.stage, Stage::Off, "the column itself stays");
    }

    /// FAILS IF: a detached player reads the machine's clock for a double
    /// click on the picture — two clicks a step apart became one double
    /// click on a fast machine and two on a slow one — or clicks with no
    /// time between stop being one. The photograph's walk says which.
    #[test]
    fn a_photographs_clock_moves_only_when_told() {
        use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};
        let state = State {
            duration: Some(60.0),
            ..crate::player::fixture::alone(
                "/films/a.mkv",
                vec![crate::player::fixture::track(
                    1,
                    ronoctua_player::wire::TrackKind::Video,
                    None,
                    true,
                )],
            )
        };
        let (mut w, mut ui) = a_photographs(Door::Window, state);
        w.relayout(&ui, &Area::whole());
        let (first, _) = w.layout.band.expect("a picture is laid out");
        // IN THE PICTURE'S MIDDLE THIRD, where a double click is the whole
        // screen; a side seeks (`player_media::picture_zone`).
        let middle = (
            f64::from(ui.width) / 2.0,
            (f64::from(first) + 0.5) * f64::from(line_px()),
        );
        let left = (f64::from(cell_px()) * 2.5, middle.1);
        let at = middle;
        assert!(w.on_a_picture(at.1), "the click is on the picture");
        let mut hand = Hand::<PerPointer>::default();
        let mut click_at = |w: &mut Player, ui: &mut Ui, at: (f64, f64)| {
            for kind in [
                PointerEventKind::Press {
                    time: 0,
                    button: LEFT_BUTTON,
                    serial: 0,
                },
                PointerEventKind::Release {
                    time: 0,
                    button: LEFT_BUTTON,
                    serial: 0,
                },
            ] {
                let e = PointerEvent {
                    surface: ui.surface.clone(),
                    position: at,
                    kind,
                };
                w.pointer_here(ui, &mut hand, &[e]);
            }
        };
        // TWO CLICKS, A STEP APART: two presses of play, and the door as it
        // was — however long this machine took between them.
        click_at(&mut w, &mut ui, at);
        w.wait(std::time::Duration::from_secs(1));
        click_at(&mut w, &mut ui, at);
        assert_eq!(w.asked(), [Ask::Toggle, Ask::Toggle]);
        assert_eq!(w.stage, Stage::Off);
        // TWO WITH NO TIME BETWEEN: a double click, the whole screen.
        w.wait(std::time::Duration::from_secs(1));
        click_at(&mut w, &mut ui, at);
        click_at(&mut w, &mut ui, at);
        assert_eq!(w.stage, Stage::Maximized);
        // ON THE LEFT THIRD a double click plays again what its first
        // paused and seeks ten seconds back — the stage as it was — and a
        // third there seeks again, and only that.
        w.wait(std::time::Duration::from_secs(1));
        let before = w.asked().len();
        click_at(&mut w, &mut ui, left);
        click_at(&mut w, &mut ui, left);
        click_at(&mut w, &mut ui, left);
        let back = Ask::Seek {
            seconds: -crate::player::SEEK_STEP,
            relative: true,
        };
        assert_eq!(
            w.asked()[before..],
            [Ask::Toggle, Ask::Toggle, back.clone(), back]
        );
        assert_eq!(w.stage, Stage::Maximized);
    }

    /// FAILS IF: the stage stops being the picture over the whole surface
    /// with the transport on its last two lines — where the controls are
    /// drawn over the picture, and where the pointer keeps them up.
    #[test]
    fn on_stage_the_picture_is_everything_and_the_transport_is_its_foot() {
        let l = super::stage_layout(0, 20);
        assert_eq!(l.band, Some((0, 20)));
        assert_eq!((l.transport, l.bar, l.list), (18, 19, 20));
        // IN A COLUMN, under its control line — which stays the column's.
        let c = super::stage_layout(1, 20);
        assert_eq!(c.band, Some((1, 19)));
        assert_eq!((c.transport, c.bar), (18, 19));
        let band = stage_area(300.0, 400.0, &column(Size::Medium));
        assert_eq!((band.y, band.height), (line_px(), 400.0 - line_px()));
    }

    /// Three lanes played as a SET: a picture of a minute from 0, a dub of
    /// forty seconds from 10 (muted, at 60 %), and subtitles that are off —
    /// the clock at 20 s of 60.
    fn a_set_of_three() -> (State, Box<ronoctua_player::queue::Outline>) {
        use ronoctua_player::own::{Lane, LaneId, LaneKind};
        let mut video = Lane::new(LaneId(1), LaneKind::Video, "/films/one.mkv");
        video.title = Some("One".into());
        let mut audio = Lane::new(LaneId(2), LaneKind::Audio, "/films/dub.flac");
        audio.start_seconds = 10.0;
        audio.muted = true;
        audio.volume_percent = 60;
        let mut words = Lane::new(LaneId(3), LaneKind::Subtitle, "/films/one.srt");
        words.start_seconds = 5.0;
        words.enabled = false;
        crate::player::fixture::set(
            vec![video, audio, words],
            &[Some(60.0), Some(40.0), Some(45.0)],
            Some(60.0),
            20.0,
        )
    }

    /// A window onto `state` and its outline, laid out once — a photograph's.
    fn a_laid_window(
        door: Door,
        (state, outline): (State, Box<ronoctua_player::queue::Outline>),
    ) -> (Player, Ui) {
        let (mut w, mut ui) = a_photographs(door, state);
        w.outline_arrived(&mut ui, Some(outline));
        w.relayout(&ui, &Area::whole());
        (w, ui)
    }

    /// One pointer event at `(x, y)`.
    fn pointed(
        w: &mut Player,
        ui: &mut Ui,
        hand: &mut Hand<PerPointer>,
        kind: PointerEventKind,
        position: (f64, f64),
    ) {
        let e = PointerEvent {
            surface: ui.surface.clone(),
            position,
            kind,
        };
        w.pointer_here(ui, hand, &[e]);
    }

    const PRESS: PointerEventKind = PointerEventKind::Press {
        time: 0,
        button: LEFT_BUTTON,
        serial: 0,
    };
    const RELEASE: PointerEventKind = PointerEventKind::Release {
        time: 0,
        button: LEFT_BUTTON,
        serial: 0,
    };
    const MOTION: PointerEventKind = PointerEventKind::Motion { time: 0 };

    /// The middle of the cell `column` (fractional) on `line`, in pixels.
    fn at_cell(column: f32, line: u16) -> (f64, f64) {
        (
            f64::from(column * cell_px()),
            (f64::from(line) + 0.5) * f64::from(line_px()),
        )
    }

    /// Where the span `text` on `line` was DRAWN: its first cell.
    fn drawn_at(spans: &[ronoctua_render::Span], line: u16, text: char) -> f32 {
        let span = spans
            .iter()
            .find(|sp| sp.line == line && sp.text == text.to_string())
            .unwrap_or_else(|| panic!("no `{text}` drawn on line {line}: {spans:?}"));
        f32::from(span.column) + 0.5
    }

    /// FAILS IF: a pointer moved from one cell of a grid row to the next —
    /// the same row, so no row changed under it — asks no frame, and the
    /// band stays on the cell it left; or a move inside one cell asks one.
    #[test]
    fn a_move_between_two_cells_of_a_row_repaints() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-window-cells-{}", std::process::id()));
        for n in 0..9 {
            let p = root.join(format!("track {n}.flac"));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(p, b"").unwrap();
        }
        let (mut w, mut ui) = a_laid_window(Door::Window, a_set_of_three());
        w.view.pick_at(&root, crate::player::Purpose::Queue);
        w.relayout(&ui, &Area::whole());
        let rows = w.rows(&ui);
        let n = (0..rows.len())
            .find(|&r| crate::player_rows::grid_cells(&rows, r).len() > 1)
            .expect("a grid row");
        ui.offset = n;
        let cells = crate::player_rows::grid_cells(&rows, n);
        let middle = |(from, width): (u16, u16)| {
            let (x, wide) = across().band_of(from, width);
            f64::from(x + wide / 2.0)
        };
        let y = (f64::from(w.layout.list) + 0.5) * f64::from(line_px());
        let mut hand = Hand::<PerPointer>::default();
        pointed(&mut w, &mut ui, &mut hand, MOTION, (middle(cells[0]), y));
        assert_eq!(hand.row_under_pointer, Some(n));
        let _ = ui.take_the_paint();
        pointed(&mut w, &mut ui, &mut hand, MOTION, (middle(cells[1]), y));
        assert!(
            ui.take_the_paint(),
            "the band follows the pointer to the next cell"
        );
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            MOTION,
            (middle(cells[1]) + 1.0, y),
        );
        assert!(!ui.take_the_paint(), "inside one cell nothing changed");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// FAILS IF: a control of the window's DECK answers a press anywhere but
    /// where the frame drew it — the lane's glyph, its mute, the player's
    /// mute, a band dragged by its ghost, a band clicked open — or the drag
    /// asks anything before it is let go, or `Esc` fails to call it off.
    #[test]
    fn the_deck_is_taken_where_the_window_draws_it() {
        use crate::player_timeline::band_color;
        use ronoctua_player::own::{LaneId, LaneKind};
        use ronoctua_player::wire::LaneChange;
        let (mut w, mut ui) = a_laid_window(Door::Window, a_set_of_three());
        assert_eq!(w.layout.deck, 4, "the ruler and three lanes");
        assert_eq!(w.layout.list, w.layout.bar + 4, "the list under the deck");
        let bar = w.layout.bar;
        let drawn = w.block_drawn(&ui);
        let mut hand = Hand::<PerPointer>::default();
        let click = |w: &mut Player, ui: &mut Ui, hand: &mut Hand<PerPointer>, p| {
            pointed(w, ui, hand, MOTION, p);
            pointed(w, ui, hand, PRESS, p);
            pointed(w, ui, hand, RELEASE, p);
        };
        let lane = |id, change| Ask::Lane {
            lane: LaneId(id),
            change,
        };
        // THE PICTURE'S GLYPH, where it was drawn: switched off.
        let c = drawn_at(
            &drawn.spans,
            bar + 1,
            crate::player_queue::lane_glyph(LaneKind::Video),
        );
        click(&mut w, &mut ui, &mut hand, at_cell(c, bar + 1));
        assert_eq!(
            w.asked().last(),
            Some(&lane(1, LaneChange::Enabled { on: false }))
        );
        // THE DUB'S MUTE — it is muted, so the glyph says so — given back.
        let c = drawn_at(&drawn.spans, bar + 2, tokens::icon::VOLUME_OFF);
        click(&mut w, &mut ui, &mut hand, at_cell(c, bar + 2));
        assert_eq!(
            w.asked().last(),
            Some(&lane(2, LaneChange::Mute { muted: false }))
        );
        // THE SUBTITLES PLAY NO SOUND and draw no mute.
        assert!(!drawn.spans.iter().any(|sp| {
            sp.line == bar + 3
                && [tokens::icon::VOLUME, tokens::icon::VOLUME_OFF]
                    .map(|g| g.to_string())
                    .contains(&sp.text)
        }));
        // THE PLAYER'S MUTE, on the ruler.
        let c = drawn_at(&drawn.spans, bar, tokens::icon::VOLUME);
        click(&mut w, &mut ui, &mut hand, at_cell(c, bar));
        assert_eq!(w.asked().last(), Some(&Ask::Mute));
        // THE DUB'S BAND, a cell into where it was painted.
        let band = drawn
            .quads
            .iter()
            .find(|q| q.color == band_color(LaneKind::Audio, true))
            .expect("the dub's band");
        let into = band.x / cell_px() + 1.0;
        let asked = w.asked().len();
        // HELD AND MOVED five cells: a ghost, and nothing asked yet.
        pointed(&mut w, &mut ui, &mut hand, PRESS, at_cell(into, bar + 2));
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            MOTION,
            at_cell(into + 5.0, bar + 2),
        );
        assert_eq!(w.asked().len(), asked, "a drag asked before it was let go");
        let ghost = w.dragged.expect("a ghost");
        assert!(
            w.block_drawn(&ui)
                .quads
                .iter()
                .any(|q| q.color == tokens::color::PLAYER_LANE_HELD),
            "the ghost is painted"
        );
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            RELEASE,
            at_cell(into + 5.0, bar + 2),
        );
        match w.asked().last() {
            Some(Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Start { seconds, .. },
            }) => assert!((seconds - ghost.start()).abs() < 1e-9 && *seconds > 10.0),
            other => panic!("the drag asked {other:?}"),
        }
        assert!(w.dragged.is_none());
        // CALLED OFF by `Esc`: nothing asked when it is let go.
        let asked = w.asked().len();
        pointed(&mut w, &mut ui, &mut hand, PRESS, at_cell(into, bar + 2));
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            MOTION,
            at_cell(into + 5.0, bar + 2),
        );
        let (event, mods) =
            crate::door::keys::event_of(ronoctua_keymap::Key::parse("ESC").expect("parses"));
        w.typed_here(&mut ui, &event, mods);
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            MOTION,
            at_cell(into + 7.0, bar + 2),
        );
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            RELEASE,
            at_cell(into + 7.0, bar + 2),
        );
        assert_eq!(
            w.asked().len(),
            asked,
            "a drag called off asked {:?}",
            w.asked().last()
        );
        assert_eq!(
            w.view.page(),
            crate::player::PageId::Home,
            "Esc left the page"
        );
        // CLICKED WITHOUT MOVING: the FIRST click chooses the lane — it wears
        // the cursor's colour on the block — and the second opens its page
        // (invariant 19).
        click(&mut w, &mut ui, &mut hand, at_cell(into, bar + 2));
        assert_eq!(
            w.view.page(),
            crate::player::PageId::Home,
            "one click on a band opened its page"
        );
        assert_eq!(w.deck_chosen, Some(LaneId(2)), "the lane was not chosen");
        let chosen = w.block_drawn(&ui);
        assert!(
            chosen
                .spans
                .iter()
                .any(|s| s.color == crate::chrome::Kind::Setting.tones(true).0),
            "the chosen lane does not wear the cursor's colour"
        );
        click(&mut w, &mut ui, &mut hand, at_cell(into, bar + 2));
        assert_eq!(w.view.page(), crate::player::PageId::Lane(LaneId(2)));
        // AND A POINTER RESTING ON THE BAND is what line 0 explains.
        assert_eq!(
            w.deck_under.map(|(_, _, hit)| hit.about()),
            Some(Some("player-lane-band"))
        );
        assert!(crate::chrome::hint_sentence("player-lane-band", &ui.texts).is_some());
    }

    /// FAILS IF: the track of the deck stops seeking where it is pressed, or
    /// the wheel over a lane that sounds stops turning that lane's volume and
    /// over the ruler the player's.
    #[test]
    fn the_deck_seeks_and_the_wheel_turns_the_volume_under_it() {
        use ronoctua_player::own::LaneId;
        use ronoctua_player::wire::LaneChange;
        let (mut w, mut ui) = a_laid_window(Door::Window, a_set_of_three());
        let bar = w.layout.bar;
        let mut hand = Hand::<PerPointer>::default();
        let (first, _) = w.deck.track();
        let column = f32::from(first) + 4.0;
        let x = f64::from(MARGIN) + f64::from(column);
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            PRESS,
            at_cell(f32::from(MARGIN) + column, bar),
        );
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            RELEASE,
            at_cell(f32::from(MARGIN) + column, bar),
        );
        match w.asked().last() {
            Some(Ask::Seek {
                seconds,
                relative: false,
            }) => assert!((seconds - w.deck.seconds_at(x - f64::from(MARGIN))).abs() < 1e-6),
            other => panic!("the ruler asked {other:?}"),
        }
        let notch = |n: i32| PointerEventKind::Axis {
            time: 0,
            horizontal: smithay_client_toolkit::seat::pointer::AxisScroll::default(),
            vertical: smithay_client_toolkit::seat::pointer::AxisScroll {
                value120: n * 120,
                ..Default::default()
            },
            source: None,
        };
        // A NOTCH IS THE LINES THE WHEEL'S ONE ARITHMETIC SAYS, a step each.
        let lines = {
            let mut owed = 0.0;
            if let PointerEventKind::Axis {
                horizontal,
                vertical,
                ..
            } = notch(1)
            {
                crate::wheel::wheel_arrived(&mut owed, &vertical, &horizontal);
            }
            u8::try_from(crate::wheel::wheel_lines(&mut owed)).expect("a few lines")
        };
        let step = crate::player::VOLUME_STEP * lines;
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            notch(-1),
            at_cell(f32::from(MARGIN) + column, bar + 2),
        );
        assert_eq!(
            w.asked().last(),
            Some(&Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Volume { percent: 60 + step }
            })
        );
        pointed(
            &mut w,
            &mut ui,
            &mut hand,
            notch(1),
            at_cell(f32::from(MARGIN) + column, bar),
        );
        assert_eq!(
            w.asked().last(),
            Some(&Ask::Volume {
                percent: 100 - step
            })
        );
    }

    /// FAILS IF: a file alone stops being the one time bar under the
    /// transport, or a stage — the corner, the whole screen, the picture
    /// alone — draws more than the one global bar; or a set past the room
    /// stops scrolling under ONE bar whose track is recorded where it is
    /// drawn.
    #[test]
    fn a_file_alone_and_every_stage_keep_the_one_bar_and_lanes_past_the_room_scroll() {
        use ronoctua_player::own::{Lane, LaneId, LaneKind};
        let film = crate::player::fixture::alone(
            "/films/a.mkv",
            vec![crate::player::fixture::track(
                1,
                ronoctua_player::wire::TrackKind::Video,
                None,
                true,
            )],
        );
        let (mut w, ui) = a_photographs(Door::Window, film);
        w.relayout(&ui, &Area::whole());
        assert_eq!(
            (w.layout.deck, w.deck.lines.len()),
            (1, 1),
            "a file alone is the bar"
        );
        // A SET ON EVERY STAGE: the one bar.
        let (mut w, mut ui) = a_laid_window(Door::Window, a_set_of_three());
        for stage in [Stage::Only, Stage::Maximized, Stage::Small] {
            w.stage_to(&mut ui, stage);
            w.relayout(&ui, &Area::whole());
            assert_eq!(w.deck.lines.len(), 1, "{stage:?} draws lanes");
            assert!(w.deck.playhead.is_none(), "{stage:?}");
            w.stage_to(&mut ui, Stage::Off);
        }
        // FIVE LANES in a window of 24 lines: four, and one bar.
        let five: Vec<Lane> = (1..=5)
            .map(|i| Lane::new(LaneId(i), LaneKind::Audio, format!("/m/{i}.flac")))
            .collect();
        let (mut w, ui) = a_laid_window(
            Door::Window,
            crate::player::fixture::set(five, &[Some(30.0); 5], Some(30.0), 3.0),
        );
        w.relayout(&ui, &Area::whole());
        assert_eq!(w.layout.deck, 5, "the ruler and four lanes");
        assert!(w.deck.scrolls());
        let drawn = w.block_drawn(&ui);
        let track = drawn.bar.expect("the lanes' bar is recorded");
        let (x, y) = (
            f64::from(track.0 + track.2 / 2.0),
            f64::from(track.1 + track.3 - 1.0),
        );
        let rows = w.rows(&ui);
        let (at, line, column, _) = w.deck_at(&ui, &rows, x, y).expect("on the deck");
        assert_eq!(at, DeckAt::Block);
        assert_eq!(
            w.deck_hit(at, &rows, line, column),
            Some(crate::player_timeline::Hit::Bar)
        );
    }

    /// FAILS IF: the timeline's own page stops drawing every lane as a row
    /// that carries its line of the deck — the block keeping the one bar —
    /// or a press on a lane's band there stops being the window's gesture,
    /// or `←`/`→` on a lane's row stop moving when it begins, half a second
    /// a step.
    #[test]
    fn the_timeline_s_rows_are_the_deck_and_the_keys_move_a_lane() {
        use crate::player::{PageId, Press};
        use crate::player_timeline::band_color;
        use ronoctua_player::own::{LaneId, LaneKind};
        use ronoctua_player::wire::LaneChange;
        let (mut w, mut ui) = a_laid_window(Door::Window, a_set_of_three());
        let key = ronoctua_keymap::Key::parse("t").expect("parses");
        let (event, mods) = crate::door::keys::event_of(key);
        w.typed_here(&mut ui, &event, mods);
        assert_eq!(w.view.page(), PageId::Timeline);
        w.relayout(&ui, &Area::whole());
        assert_eq!(w.layout.deck, 1, "the block keeps the one bar");
        let rows = w.rows(&ui);
        assert_eq!(
            rows.iter().filter(|r| r.deck.is_some()).count(),
            4,
            "the ruler and a row a lane"
        );
        // THE DUB'S ROW, and its band where the row's deck paints it.
        let n = rows
            .iter()
            .position(|r| r.target == crate::player::Target::Lane(LaneId(2)))
            .expect("the dub's row");
        let d = rows[n].deck.clone().expect("a line of the deck");
        let y = w.layout.list + u16::try_from(n - ui.offset).expect("on screen");
        let quads = d
            .deck
            .quads(ui.frame(0), y, MARGIN + d.left, d.line..d.line + 1, None);
        let band = quads
            .iter()
            .find(|q| q.color == band_color(LaneKind::Audio, true))
            .expect("the band");
        let into = band.x / cell_px() + 1.0;
        let mut hand = Hand::<PerPointer>::default();
        // HALF A CELL BEFORE THE BAND is the track: a seek, and no band.
        pointed(&mut w, &mut ui, &mut hand, PRESS, at_cell(into - 1.5, y));
        pointed(&mut w, &mut ui, &mut hand, RELEASE, at_cell(into - 1.5, y));
        assert!(
            matches!(w.asked().last(), Some(Ask::Seek { .. })),
            "before the band: {:?}",
            w.asked().last()
        );
        pointed(&mut w, &mut ui, &mut hand, PRESS, at_cell(into, y));
        pointed(&mut w, &mut ui, &mut hand, MOTION, at_cell(into + 4.0, y));
        pointed(&mut w, &mut ui, &mut hand, RELEASE, at_cell(into + 4.0, y));
        // THE START MOVED BY THE FOUR CELLS the pointer moved, on the row's
        // own deck.
        let origin = f64::from(MARGIN + d.left);
        let pressed = f64::from(into) - origin;
        let want = 10.0 + d.deck.seconds_at(pressed + 4.0) - d.deck.seconds_at(pressed);
        match w.asked().last() {
            Some(Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Start { seconds, .. },
            }) => assert!((seconds - want).abs() < 1e-3, "{seconds} against {want}"),
            other => panic!("the drag on the row asked {other:?}"),
        }
        assert_eq!(w.view.cursor(), n, "the cursor came to the row pressed");
        // A CLICK ON ANOTHER LANE'S BAND chooses its row — the cursor goes
        // there, the page stays — and a second click opens its page
        // (invariant 19).
        let m = rows
            .iter()
            .position(|r| r.target == crate::player::Target::Lane(LaneId(1)))
            .expect("the picture's row");
        let dm = rows[m].deck.clone().expect("a line of the deck");
        let ym = w.layout.list + u16::try_from(m - ui.offset).expect("on screen");
        let picture = dm
            .deck
            .quads(
                ui.frame(0),
                ym,
                MARGIN + dm.left,
                dm.line..dm.line + 1,
                None,
            )
            .into_iter()
            .find(|q| q.color == band_color(LaneKind::Video, true))
            .expect("the picture's band");
        let on = picture.x / cell_px() + 1.0;
        let asked = w.asked().len();
        let click = |w: &mut Player, ui: &mut Ui, hand: &mut Hand<PerPointer>, p| {
            pointed(w, ui, hand, MOTION, p);
            pointed(w, ui, hand, PRESS, p);
            pointed(w, ui, hand, RELEASE, p);
        };
        click(&mut w, &mut ui, &mut hand, at_cell(on, ym));
        assert_eq!(w.view.page(), PageId::Timeline, "one click opened the page");
        assert_eq!(w.view.cursor(), m, "the first click did not choose the row");
        assert_eq!(w.asked().len(), asked, "a click on a band asked something");
        click(&mut w, &mut ui, &mut hand, at_cell(on, ym));
        assert_eq!(w.view.page(), PageId::Lane(LaneId(1)));
        w.pressed(&mut ui, Press::Escape);
        let rows = w.rows(&ui);
        let n = rows
            .iter()
            .position(|r| r.target == crate::player::Target::Lane(LaneId(2)))
            .expect("the dub's row");
        w.view.grabbed(n);
        // THE KEYS, on the same row.
        w.pressed(&mut ui, Press::More);
        assert_eq!(
            w.asked().last(),
            Some(&Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Start {
                    seconds: 10.5,
                    voice: false
                }
            })
        );
        w.pressed(&mut ui, Press::Less);
        assert_eq!(
            w.asked().last(),
            Some(&Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Start {
                    seconds: 9.5,
                    voice: false
                }
            })
        );
        w.pressed(&mut ui, Press::Remove);
        assert_eq!(
            w.asked().last(),
            Some(&Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Remove
            })
        );
        w.pressed(&mut ui, Press::Enter);
        assert_eq!(w.view.page(), PageId::Lane(LaneId(2)));
    }
}
