//! The footer's two hidden modes: which of its rows are on the screen, which
//! of them push the windows away, and which lie over them on a veil.
//!
//! # What the owner asked for
//!
//! Two modes besides the bars as they always were: one hides the modeline
//! and the minibuffer and makes whatever opens PUSH the windows; the other
//! hides them and makes everything lie OVER the windows. A TAP of `Ctrl` at
//! the leader's root walks the three in turn — normal, push, cover, normal —
//! ([`next_mode`], the owner's scheme of 2026-09-30; before it the leader
//! pressed again toggled one mode per set, and the leader again now walks the
//! SETS, `crate::bindings::Set::next`). In both modes, the bottom edge of a
//! screen reveals the bars under the pointer, the minibuffer shows itself
//! whenever it has something to say, and the leader's list waits for the hand
//! to stop (`crate::leader::Patience`).
//!
//! | | push | cover |
//! |---|---|---|
//! | the minibuffer alone, and the leader's list | over, veiled | over, veiled |
//! | a footer app open | pushes, on the desk | over, veiled |
//! | the notification band, the brain's line | pushes, own ground | over, own ground |
//! | the on-screen keyboard | pushes, on the desk | pushes ALONE (its rows), on the footer's page |
//! | the pointer at the bottom edge | the bars push, on the desk | the bars over, veiled |
//! | a kiosk in focus | the bars push, on the desk | nothing |
//!
//! "On the desk" was "no ground" — a clear of alpha 0, the desk showing
//! through — until the owner asked (2026-09-28) for the desk's colour,
//! opaque: the same look, without the white fringe text drawn on nothing
//! gets. Rows on the desk WEAR the desk's page ([`Shape::page`]), so their
//! ground is simply the page's; the veil is the page's ground at four fifths
//! ([`Shape::grounds`]).
//!
//! "Own ground" is a ground the same on every page: the band's and the brain
//! line's tones are copied from the base palette onto every page. The
//! keyboard has none of its own: it stands on its PAGE's chrome — the desk
//! while its rows push the windows away, the footer's page in `Cover`, where
//! only its rows push — and its caps are bands solved and gated on every
//! page, like the rest of the footer's.
//!
//! # Why everything pushes when anything does
//!
//! The layer shell reserves a zone counted from the ANCHORED edge — the
//! bottom — and the band grows at the TOP of the surface. A zone that covered
//! the band and not the rows under it cannot be asked for: so while anything
//! pushes, the whole surface is the zone, and nothing on it wears the veil.
//! It is the protocol's geometry and it is written down, not worked around.
//!
//! # Where the answer lives
//!
//! [`Shape::of`] is pure, over a handful of facts the footer reads; the
//! footer keeps the APPLIED one in a field and every row sum reads that field
//! — `Footer::metric` may not ask what the region holds
//! (`the-footer-does-not-ask-itself`), and a shape worked out inside the sum
//! would be exactly that question.
//!
//! # What the translucency is not
//!
//! Glass. A frosted veil needs the pixels BEHIND the surface blurred, and
//! the pinned compositor blurs nothing behind a layer surface: its wlroots
//! carries `ext-background-effect-v1`, but nothing in it creates the manager
//! and its scene has no blur pass. The two other ways — patching the
//! compositor, or capturing the screen under the footer from the footer —
//! are invariant 3's, and are the owner's to decide.

use crate::adapt::Hidden;
use crate::bindings::Set;
use ronoctua_tokens as tokens;

/// The command that walks the three modes in turn — the tap of `Ctrl`. The
/// two that toggle one mode each by name are `footer-hide-push` and
/// `footer-hide-cover` (`toggled`).
pub const CYCLE_COMMAND: &str = "footer-bars-cycle";

/// The mode after `now` in the cycle a tap of `Ctrl` walks: the bars as they
/// always were, then pushing, then covering, then back.
///
/// # Why this order
///
/// Push first because it is the gentler of the two: what opens still moves
/// the windows out of its way, exactly as with the bars up, and only the
/// resting bars are gone. Cover is the further step — everything over the
/// windows — and one more tap is the way home.
#[must_use]
pub const fn next_mode(now: Option<Hidden>) -> Option<Hidden> {
    match now {
        None => Some(Hidden::Push),
        Some(Hidden::Push) => Some(Hidden::Cover),
        Some(Hidden::Cover) => None,
    }
}

/// What the leader ARRIVING does: `Some(set)` switches the open list to that
/// set — the one after `set` ([`Set::next`]) — and `None` opens the leader,
/// fresh or starting the sequence over.
///
/// A switch only on a leader already open with NOTHING typed — no key of the
/// tree, and not inside the buffer's `,` or the `w s` states, whose keys the
/// tree does not count — so a person who mistyped and presses the leader to
/// start over still starts over.
#[must_use]
pub const fn leader_again(open: bool, typed_nothing: bool, set: Set) -> Option<Set> {
    if open && typed_nothing {
        Some(set.next())
    } else {
        None
    }
}

/// The mode that follows `now` when `wanted` is toggled: the mode, or back to
/// visible when it is already the mode. From the other mode, the one asked.
///
/// What `footer-hide-push` and `footer-hide-cover` do by name; the tap walks
/// [`next_mode`] instead.
#[must_use]
pub fn toggled(now: Option<Hidden>, wanted: Hidden) -> Option<Hidden> {
    (now != Some(wanted)).then_some(wanted)
}

/// The row the root's list grafts for the cycle, as the `,` row is grafted:
/// its key is `CTRL` ([`crate::leader::BARS_TAP_KEY`]), the key a tap of which
/// walks the modes, and its command [`CYCLE_COMMAND`]. In every set: the tap
/// cycles at the root of each.
///
/// It exists for the pointer (invariant 19): the cycle's key is a modifier
/// tapped alone, which the list cannot otherwise show and a click cannot
/// otherwise press. A click on it runs the cycle (`Click::Bars`).
#[must_use]
pub fn bars_row() -> ronoctua_keymap::Entry {
    let ctrl = ronoctua_keymap::Mods {
        ctrl: true,
        ..ronoctua_keymap::Mods::default()
    };
    ronoctua_keymap::Entry {
        key: crate::leader::BARS_TAP_KEY.to_owned(),
        // THE ROW'S KEY AS DATA is never pressed — a click on it is
        // `Click::Bars` — and it is `C-ESC`: `Ctrl` for the key it stands
        // for, and `ESC` because no tree may bind it (`Leader::key` ends on
        // it before any tree is asked), so the data cannot be taken for a
        // binding. It was `SPC`, which the root now leaves to the fast access.
        sym: ronoctua_keymap::Key {
            mods: ctrl,
            sym: ronoctua_keymap::Sym::Esc,
        },
        label: format!("cmd-{CYCLE_COMMAND}"),
        command: Some(CYCLE_COMMAND.to_owned()),
        is_group: false,
    }
}

/// What decides the shape — facts the footer reads, never its rows.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Inputs {
    /// The mode, or `None` for the bars visible.
    pub hidden: Option<Hidden>,
    /// A pointer is on the footer at a screen's bottom edge.
    pub revealed: bool,
    /// The focused window is a kiosk, adapted or not.
    pub kiosk_focused: bool,
    /// A view of the footer that is an APP holds the region — not the
    /// leader's list, the command finder or the window menu.
    pub app_open: bool,
    /// The minibuffer has something to say: a view that has the keyboard, an
    /// echo, a question, a flash, the opened line.
    pub minibuffer_busy: bool,
    /// The notification band or the brain's line is up.
    pub band_or_brain: bool,
    /// The on-screen keyboard is out. It types into the focused window, and
    /// in every mode it pushes that window away rather than lying over the
    /// line being typed into (the owner, 2026-09-28): in `Push` with
    /// everything else, in `Cover` alone — [`geometry`] reserves its rows.
    pub keyboard_out: bool,
    /// The leader's list is held back (`crate::leader::Patience`).
    pub list_withheld: bool,
}

/// Which of the footer's rows are on the screen, and how they are grounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    /// The modeline's rows: one or none.
    pub modeline_rows: u16,
    /// The minibuffer's rows: one or none.
    pub minibuffer_rows: u16,
    /// Whether the region below them is drawn — `false` while the leader's
    /// list waits.
    pub region_shown: bool,
    /// Whether the surface reserves its height: the windows move out of its
    /// way. When it does not, it lies over them.
    pub pushing: bool,
    /// Whether a mode is on: the frame clears with `surface.clear`, the rows
    /// stand on the desk or the veil, and the modeline wears no ground of its
    /// own ([`Shape::page`], [`Shape::grounds`]).
    pub hidden: bool,
}

impl Default for Shape {
    /// The bars visible — the shape every footer had before the modes.
    fn default() -> Self {
        Self::of(Inputs::default())
    }
}

impl Shape {
    /// The shape these facts call for.
    #[must_use]
    pub fn of(i: Inputs) -> Self {
        // THE BARS: always when visible; on the edge; and in `Push` while a
        // kiosk has the focus — a kiosk has no bars of its own, and the
        // modeline is where its controls are. In `Cover` only the edge.
        let bars =
            i.hidden.is_none() || i.revealed || (i.hidden == Some(Hidden::Push) && i.kiosk_focused);
        // AND THE MINIBUFFER WITH THEM, or alone when it has something to
        // say: the modeline never comes without the line under it.
        let minibuffer = bars || i.minibuffer_busy;
        let pushing = match i.hidden {
            None => true,
            Some(Hidden::Cover) => false,
            // THE KEYBOARD PUSHES TOO: it types into the focused window, and
            // lying over it would cover the very line being typed into.
            Some(Hidden::Push) => {
                i.app_open || i.revealed || i.kiosk_focused || i.band_or_brain || i.keyboard_out
            }
        };
        Self {
            modeline_rows: u16::from(bars),
            minibuffer_rows: u16::from(minibuffer),
            region_shown: !i.list_withheld,
            pushing,
            hidden: i.hidden.is_some(),
        }
    }

    /// The rows the two bars take — what `FOOTER_LINES` was.
    #[must_use]
    pub const fn lines(self) -> u16 {
        self.modeline_rows + self.minibuffer_rows
    }

    /// Whether the modeline is drawn. When it is not, what the pointer would
    /// hit on it is nothing: its tables are emptied with it.
    #[must_use]
    pub const fn draws_modeline(self) -> bool {
        self.modeline_rows > 0
    }

    /// Whether the minibuffer's line is drawn — and its shortcuts, its
    /// chevron and its page counter with it.
    #[must_use]
    pub const fn draws_minibuffer(self) -> bool {
        self.minibuffer_rows > 0
    }

    /// Whether the rows from the modeline down wear the veil: hidden, and
    /// lying over the windows. The band and the brain's line keep their own
    /// grounds either way, and the keyboard lays its page's chrome under its
    /// keys (the module header).
    #[must_use]
    pub const fn veiled(self) -> bool {
        self.hidden && !self.pushing
    }

    /// The shape as ONE screen paints it: `over_a_fullscreen_window` is the
    /// fullscreen edge holding that screen's footer out over the window
    /// (`Footer::edge`, the pointer on it or a region with the keyboard).
    ///
    /// # Why visible there
    ///
    /// Rows on the veil, over a window that fills the screen, are rows over
    /// whatever that window shows — a film, a slide — and rows on the desk
    /// are a strip of desk cut into it, which the window never moved off.
    /// The fullscreen edge is the footer it always was: the visible footer's
    /// ground and, on the chrome, the modeline's own ([`Shape::grounds`]).
    #[must_use]
    pub const fn painted(self, over_a_fullscreen_window: bool) -> Self {
        if over_a_fullscreen_window {
            Self {
                hidden: false,
                pushing: true,
                ..self
            }
        } else {
            self
        }
    }

    /// The PAGE one screen painted with this shape wears
    /// (`crate::theme::Page`): the footer's — `footer`, the chrome's or the
    /// desk's, as the person answered (`LEADER s t`) — except while its rows
    /// PUSH the windows away with the bars hidden: those rows stand on the
    /// desk the windows moved off, so they wear the desk's page (`desk`).
    ///
    /// # Why a page and not a colour
    ///
    /// Because a row on the desk is not the chrome's row on another ground.
    /// The owner's answer (2026-09-28): the bands follow the ground. Every
    /// page is a palette the generator solved with that desk in the chrome's
    /// place, and a screen wearing it paints exactly as it would on the
    /// chrome — the text, the hover, the selection all measured from the desk.
    #[must_use]
    pub const fn page(
        self,
        footer: crate::theme::Page,
        desk: crate::theme::Page,
    ) -> crate::theme::Page {
        if self.hidden && self.pushing {
            desk
        } else {
            footer
        }
    }

    /// The page the footer's PICTURES are painted in and looked up under —
    /// the thumbnails, the preview, the globe, the gestures' drawings: every
    /// picture a child or a worker paints with a token's VALUE, whose key
    /// names the palette (`ronoctua_tokens::theme::palette`).
    ///
    /// # Why one page for the pictures, when each screen wears its own
    ///
    /// Because a picture is ASKED FOR before the frame and LOOKED UP inside
    /// it, per screen. Asked under the page the last screen of the previous
    /// frame left on, and looked up under the page each screen wears, the two
    /// keys disagreed on the frame the page changed — a grid drawn blank, the
    /// same file decoded twice — and for as long as two screens wore two
    /// pages (the fullscreen edge held out on one), one grid never found a
    /// picture at all. One page per frame is asked for and looked up under.
    ///
    /// # Which one
    ///
    /// The page of the screen the person is at: where the fullscreen edge is
    /// held out (`edge_out`, the pointer on it or a region with the keyboard)
    /// that screen's, painted as the visible footer ([`Self::painted`]); on
    /// every other frame every screen wears the same page, and it is that
    /// one. A screen wearing another page shows the pictures in their inks,
    /// which is a picture in a hair's different ink rather than no picture.
    #[must_use]
    pub const fn pictures_page(
        self,
        edge_out: bool,
        footer: crate::theme::Page,
        desk: crate::theme::Page,
    ) -> crate::theme::Page {
        self.painted(edge_out).page(footer, desk)
    }

    /// What ONE screen painted with this shape lays FIRST, on the page it
    /// wears ([`Self::page`]).
    ///
    /// - VISIBLE, the frame clears with the page's ground — `surface.chrome`,
    ///   which on a desk's page IS the desk — and the modeline is raised a
    ///   tone above it on the chrome only: on the desk the modeline has the
    ///   desk's colour (the owner), no band of chrome across it.
    /// - HIDDEN, the frame clears with nothing (`surface.clear`) — the
    ///   one-pixel edge shows nothing, and the veil is laid over nothing so
    ///   the window shows through it. The rows from the modeline down stand
    ///   on the page's veil while they lie over the windows, and on the
    ///   page's ground, opaque, while they push them away — which is the desk,
    ///   since pushing rows wear the desk's page.
    ///
    /// # Why opaque under the rows that push
    ///
    /// They used to stand on nothing: the clear of alpha 0, the desk the
    /// windows moved off showing through. It looked like the desk and it was
    /// not: text blended onto alpha 0 is not validly premultiplied on the
    /// sRGB target (`ronoctua_render::gpu::clear_of`), and every glyph came
    /// out with a white fringe. The desk's colour, opaque, is the same look
    /// with nothing to fringe (the owner, 2026-09-28).
    #[must_use]
    pub fn grounds(self, page: crate::theme::Page) -> Grounds {
        if !self.hidden {
            return Grounds {
                clear: tokens::color::SURFACE_CHROME,
                rows: None,
                modeline: (self.draws_modeline() && page == crate::theme::Page::Chrome)
                    .then_some(tokens::color::MODELINE_BACKGROUND),
            };
        }
        Grounds {
            clear: tokens::color::SURFACE_CLEAR,
            rows: Some(if self.veiled() {
                tokens::color::SURFACE_VEIL
            } else {
                tokens::color::SURFACE_CHROME
            }),
            modeline: None,
        }
    }
}

/// What ONE screen of the footer lays first ([`Shape::grounds`]): the frame's
/// clear and the two grounds laid FIRST, under everything else — so the
/// band, the brain's line, the glance's stripes, the keyboard, a selection
/// and a hover paint their own grounds over them. Every colour here is a role
/// of the page the screen wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grounds {
    /// What the frame clears with.
    pub clear: tokens::Color,
    /// The ground under the rows from the modeline's line down to the
    /// region's floor: the page's ground (the desk) while they push, the
    /// page's veil while they lie over; `None` with the bars visible, where
    /// the clear is their ground.
    pub rows: Option<tokens::Color>,
    /// The modeline's own band, one tone above the chrome — only with the
    /// bars visible and the footer on the chrome.
    pub modeline: Option<tokens::Color>,
}

/// Whether a pointer at `y` on a footer `height` pixels tall is at the
/// screen's bottom edge — the last row of pixels, which at rest is the whole
/// one-pixel surface.
///
/// # Why the edge and not the whole surface
///
/// Because the surface is not always bare when the hand goes to it. With the
/// band up, the pointer crossing the band to press one of its buttons would
/// bring the bars up UNDER the band — and the surface, anchored at the
/// bottom, grows upwards: the button would move away from the hand that was
/// reaching for it. So only the bottom row reveals, and once revealed the
/// bars stay while the pointer is anywhere on the footer; leaving it hides
/// them.
#[must_use]
pub fn at_the_edge(y: f64, height: u32) -> bool {
    y >= f64::from(height) - 1.0
}

/// What ONE screen's surface asks the compositor for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// The height, in pixels.
    pub height: u32,
    /// The exclusive zone: the height while pushing, `-1` while lying over.
    pub zone: i32,
    /// Whether the surface goes on the `overlay` layer rather than `top`.
    pub overlay: bool,
}

/// What ONE screen's surface asks the compositor for: its height, its
/// exclusive zone, and its layer.
///
/// `lines` is every row the footer shows (the region included, the height of
/// a pixel NOT yet applied), `revealing` how many of them only the pointer's
/// edge adds, and `revealed_here` whether that pointer is on THIS screen.
///
/// # One arithmetic, the height cut per screen
///
/// The rows are counted once for every screen — the fullscreen edge already
/// works this way: a covered screen keeps the footer's rows and shows one
/// pixel of them. Here, a screen the pointer is NOT on, whose footer would
/// show nothing at all without the pointer elsewhere, stays one pixel: the
/// bars come up where the hand is. Anything else on screen — an echo, a
/// view, the band — is on every screen, as the two lines always were, and
/// THEN the edge of one screen brings the bars to every screen showing it —
/// pushing, with their zone. One `Shape` for every screen is that price, and
/// the owner kept it (2026-09-28): a shape per screen would have to reach
/// the paint as well as this arithmetic.
///
/// # Nothing shown
///
/// One transparent pixel, reserving nothing: the edge the pointer finds.
///
/// # Lying over: the overlay layer and a zone of `-1` (invariant 14)
///
/// A surface with a zone of 0 is placed inside the room the EXCLUSIVE
/// surfaces leave, and a right column is one (`column.rs`: `top`, anchored
/// top, right and bottom, its width reserved). A hidden footer that reserved
/// nothing came out narrowed by the column — the modeline, the minibuffer,
/// the band and the list without the column's width, the marks counted from
/// the right edge moved, and the one-pixel edge stopping short of the corner.
/// Invariant 14 is that the column never narrows them. A zone of `-1` is the
/// protocol's "ignore the other zones": the whole width. And the `overlay`
/// layer puts it ABOVE the column, which is on `top` — on one layer, the
/// column created after the footer would stack over its right end.
///
/// # One layer for as long as the bars are hidden
///
/// Pushing, a hidden footer STAYS on the overlay and only its zone changes
/// (its height instead of `-1`). It used to go back to `top` to push, and
/// that was a loop: a surface that changes layer under the pointer is given
/// a `leave` by the compositor, the `leave` hides what the edge revealed,
/// the pointer is on the edge again, and the footer turned about every 15 ms
/// — 1044 turns in three seconds, measured in the stage, with the windows
/// jumping by the footer's height each time. On the overlay it is also
/// reserved FIRST (the overlay is the first layer arranged), so a right
/// column on `top` still stops above it (invariant 14).
///
/// With the bars visible the footer is `top` and reserves its height, as it
/// always did; the layer moves only when a mode is switched, by a key.
///
/// THE PRICE, written down: hidden, the footer ignores the resizer's
/// curtains (`resizer.rs`, a development tool, `LEADER / r`). Lying over, its
/// zone of `-1` ignores every other zone; pushing, it is reserved before the
/// curtains, made later on the same `overlay` layer. Either way it does not
/// sit on the shorter screen the curtains make: it keeps the real bottom
/// edge, and the bottom curtain is drawn over it.
#[must_use]
pub fn geometry(
    shape: Shape,
    lines: u32,
    revealing: u32,
    revealed_here: bool,
    keyboard_lines: u32,
    line_px: f32,
) -> Geometry {
    let bare_without_the_edge = lines.saturating_sub(revealing) == 0;
    let own = if shape.hidden && bare_without_the_edge && !revealed_here {
        0
    } else {
        lines
    };
    // NOTHING ON THIS SCREEN: only a hidden footer is ever bare, and it lies
    // over — the whole width, above the column.
    if own == 0 {
        return Geometry {
            height: 1,
            zone: -1,
            overlay: true,
        };
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let height = (line_px * own as f32).ceil() as u32;
    if shape.pushing {
        Geometry {
            height,
            zone: i32::try_from(height).unwrap_or(i32::MAX),
            overlay: shape.hidden,
        }
    } else if keyboard_lines > 0 {
        // THE KEYBOARD PUSHES IN EVERY MODE (the owner, 2026-09-28), and here
        // it pushes alone: it is the bottom row of the surface, the zone
        // counts from the bottom edge, so reserving its height and no more
        // leaves what is above it lying over the windows.
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let keys = (line_px * keyboard_lines as f32).ceil() as u32;
        Geometry {
            height,
            zone: i32::try_from(keys.min(height)).unwrap_or(i32::MAX),
            overlay: true,
        }
    } else {
        Geometry {
            height,
            zone: -1,
            overlay: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(hidden: Option<Hidden>) -> Inputs {
        Inputs {
            hidden,
            ..Inputs::default()
        }
    }

    /// FAILS IF: the visible footer stops being the two lines that push,
    /// whatever else is true — the shape every footer had before the modes.
    #[test]
    fn visible_is_what_there_always_was() {
        for bits in 0u8..128 {
            let i = Inputs {
                hidden: None,
                revealed: bits & 1 != 0,
                kiosk_focused: bits & 2 != 0,
                app_open: bits & 4 != 0,
                minibuffer_busy: bits & 8 != 0,
                band_or_brain: bits & 16 != 0,
                keyboard_out: bits & 64 != 0,
                list_withheld: false,
            };
            let s = Shape::of(i);
            assert_eq!(s.lines(), crate::surface::FOOTER_LINES, "{i:?}");
            assert!(
                s.pushing && !s.hidden && !s.veiled() && s.region_shown,
                "{i:?}"
            );
        }
        assert_eq!(Shape::default(), Shape::of(inputs(None)));
    }

    /// FAILS IF: any cell of the owner's table moves — the whole of it,
    /// mode × edge × kiosk × app × busy × band × keyboard × withheld, in both
    /// modes.
    #[test]
    fn the_hidden_shapes_are_the_owners_table() {
        for mode in [Hidden::Push, Hidden::Cover] {
            for bits in 0u8..128 {
                let i = Inputs {
                    hidden: Some(mode),
                    revealed: bits & 1 != 0,
                    kiosk_focused: bits & 2 != 0,
                    app_open: bits & 4 != 0,
                    minibuffer_busy: bits & 8 != 0,
                    band_or_brain: bits & 16 != 0,
                    list_withheld: bits & 32 != 0,
                    keyboard_out: bits & 64 != 0,
                };
                let s = Shape::of(i);
                let bars = i.revealed || (mode == Hidden::Push && i.kiosk_focused);
                assert_eq!(s.modeline_rows, u16::from(bars), "{i:?}");
                assert_eq!(
                    s.minibuffer_rows,
                    u16::from(bars || i.minibuffer_busy),
                    "{i:?}"
                );
                // THE ON-SCREEN KEYBOARD PUSHES the whole footer in Push; in
                // Cover the shape still lies over — only the keyboard's own
                // rows are reserved, by `geometry`
                // (`the_keyboard_pushes_in_every_mode`).
                let pushes = mode == Hidden::Push
                    && (i.app_open
                        || i.revealed
                        || i.kiosk_focused
                        || i.band_or_brain
                        || i.keyboard_out);
                assert_eq!(s.pushing, pushes, "{i:?}");
                assert_eq!(s.veiled(), !pushes, "{i:?}");
                assert_eq!(s.region_shown, !i.list_withheld, "{i:?}");
                assert!(s.hidden, "{i:?}");
            }
        }
    }

    /// FAILS IF: at rest a hidden footer draws a bar — and so keeps a table
    /// the pointer could hit — or a busy minibuffer brings the modeline.
    #[test]
    fn at_rest_nothing_is_drawn_and_nothing_is_hit() {
        for mode in [Hidden::Push, Hidden::Cover] {
            let rest = Shape::of(inputs(Some(mode)));
            assert!(!rest.draws_modeline() && !rest.draws_minibuffer());
            assert_eq!(rest.lines(), 0);
            let busy = Shape::of(Inputs {
                minibuffer_busy: true,
                ..inputs(Some(mode))
            });
            assert!(!busy.draws_modeline() && busy.draws_minibuffer());
        }
    }

    /// FAILS IF: the list waiting leaves the region drawn, or the list
    /// waiting touches the bars.
    #[test]
    fn a_waiting_list_takes_only_the_region() {
        let waiting = Shape::of(Inputs {
            minibuffer_busy: true,
            list_withheld: true,
            ..inputs(Some(Hidden::Push))
        });
        assert!(!waiting.region_shown);
        assert!(waiting.draws_minibuffer() && !waiting.pushing);
    }

    /// FAILS IF: the tap stops walking normal → push → cover → normal, or a
    /// mode toggled by name stops coming back to visible.
    #[test]
    fn a_tap_walks_the_three_modes_round() {
        assert_eq!(next_mode(None), Some(Hidden::Push));
        assert_eq!(next_mode(Some(Hidden::Push)), Some(Hidden::Cover));
        assert_eq!(next_mode(Some(Hidden::Cover)), None);
        // THREE TAPS ARE HOME.
        assert_eq!(next_mode(next_mode(next_mode(None))), None);
        // AND BY NAME, EACH MODE STILL TOGGLES.
        assert_eq!(toggled(None, Hidden::Push), Some(Hidden::Push));
        assert_eq!(toggled(Some(Hidden::Push), Hidden::Push), None);
        // FROM THE OTHER MODE, THE ONE ASKED — not back to visible.
        assert_eq!(
            toggled(Some(Hidden::Cover), Hidden::Push),
            Some(Hidden::Push)
        );
        assert_eq!(
            toggled(Some(Hidden::Push), Hidden::Cover),
            Some(Hidden::Cover)
        );
        assert_eq!(toggled(Some(Hidden::Cover), Hidden::Cover), None);
    }

    /// FAILS IF: the leader arriving switches anything but an open leader
    /// with nothing typed — a fresh opening, a sequence under way — or
    /// switches to any set but the next one.
    #[test]
    fn the_leader_again_switches_only_at_the_root() {
        for set in [Set::Footer, Set::Window, Set::Full] {
            assert_eq!(
                leader_again(false, true, set),
                None,
                "a fresh opening, {set:?}"
            );
            assert_eq!(
                leader_again(true, false, set),
                None,
                "a sequence under way, {set:?}"
            );
            assert_eq!(leader_again(true, true, set), Some(set.next()), "{set:?}");
        }
        // AND ROUND: from the whole tree, back to the default.
        assert_eq!(leader_again(true, true, Set::Full), Some(Set::Footer));
    }

    /// FAILS IF: the grafted row names a command the registry does not have,
    /// teaches a key other than `CTRL`, is a group, or carries `SPC` as its
    /// data — the root's `SPC` is the fast access's.
    #[test]
    fn the_root_teaches_the_cycle() {
        let row = bars_row();
        assert_eq!(row.key, crate::leader::BARS_TAP_KEY);
        assert_eq!(row.key, "CTRL");
        assert_eq!(row.command.as_deref(), Some(CYCLE_COMMAND));
        assert_eq!(row.label, format!("cmd-{CYCLE_COMMAND}"));
        assert!(!row.is_group);
        assert_ne!(row.sym.sym, ronoctua_keymap::Sym::Space);
        assert_eq!(row.sym.sym, ronoctua_keymap::Sym::Esc);
        for command in [CYCLE_COMMAND, "footer-hide-push", "footer-hide-cover"] {
            assert!(
                crate::leader::BASE.commands.iter().any(|c| c.id == command),
                "{command} is not registered"
            );
            assert!(crate::leader::BASE.by_name.contains(&command));
        }
    }

    /// FAILS IF: the bare pixel does not count as the edge, the last row of a
    /// taller footer does not, or a row above it — the band's buttons — does.
    #[test]
    fn only_the_bottom_row_is_the_edge() {
        assert!(at_the_edge(0.0, 1));
        assert!(at_the_edge(0.5, 1));
        assert!(at_the_edge(59.2, 60));
        assert!(!at_the_edge(58.0, 60));
        assert!(!at_the_edge(0.0, 60));
    }

    /// FAILS IF: a hidden footer with nothing to show asks for more than one
    /// pixel or reserves anything, the screen under the pointer does not
    /// grow, another screen grows with it, or what is shown for its own sake
    /// is left off a screen.
    #[test]
    fn each_screen_shows_what_is_its() {
        let px = 20.0;
        let size = |g: Geometry| (g.height, g.zone);
        let rest = Shape::of(inputs(Some(Hidden::Push)));
        assert_eq!(size(geometry(rest, 0, 0, false, 0, px)), (1, -1));
        // THE EDGE, in push: the bars push where the pointer is, and only there.
        let edge = Shape::of(Inputs {
            revealed: true,
            ..inputs(Some(Hidden::Push))
        });
        assert_eq!(size(geometry(edge, 2, 2, true, 0, px)), (40, 40));
        assert_eq!(size(geometry(edge, 2, 2, false, 0, px)), (1, -1));
        // IN COVER, over the windows: the height and no zone of its own.
        let cover = Shape::of(Inputs {
            revealed: true,
            ..inputs(Some(Hidden::Cover))
        });
        assert_eq!(size(geometry(cover, 2, 2, true, 0, px)), (40, -1));
        // AN ECHO IS ON EVERY SCREEN, as the two lines always were: one row of
        // its own, and the edge adding the modeline over it.
        let echo = Shape::of(Inputs {
            minibuffer_busy: true,
            revealed: true,
            ..inputs(Some(Hidden::Cover))
        });
        assert_eq!(size(geometry(echo, 2, 1, false, 0, px)), (40, -1));
        // VISIBLE: the lines, and the zone the same height.
        assert_eq!(
            size(geometry(Shape::default(), 5, 0, false, 0, 21.6)),
            (108, 108)
        );
    }

    /// FAILS IF: a hidden footer that lies over the windows reserves a zone
    /// of 0 — which a right column's zone narrows (invariant 14) — or stays
    /// on the column's layer, under it; or a hidden footer that pushes
    /// leaves the overlay; or a visible one leaves `top` and its zone of its
    /// own height.
    #[test]
    fn a_footer_lying_over_takes_the_whole_width_above_the_column() {
        let px = 20.0;
        for mode in [Hidden::Push, Hidden::Cover] {
            // AT REST: the one-pixel edge, the whole width, above the column.
            let rest = geometry(Shape::of(inputs(Some(mode))), 0, 0, false, 0, px);
            assert_eq!(
                rest,
                Geometry {
                    height: 1,
                    zone: -1,
                    overlay: true
                },
                "{mode:?}"
            );
            // THE MINIBUFFER ALONE, veiled over the windows.
            let busy = Shape::of(Inputs {
                minibuffer_busy: true,
                ..inputs(Some(mode))
            });
            assert!(!busy.pushing);
            assert_eq!(
                geometry(busy, 1, 0, false, 0, px),
                Geometry {
                    height: 20,
                    zone: -1,
                    overlay: true
                },
                "{mode:?}"
            );
        }
        // PUSHING, HIDDEN: still the overlay (see the test below), and the
        // height reserved — reserved FIRST, on the first layer arranged, so
        // the column stops above it.
        let pushing = Shape::of(Inputs {
            app_open: true,
            ..inputs(Some(Hidden::Push))
        });
        assert_eq!(
            geometry(pushing, 4, 0, false, 0, px),
            Geometry {
                height: 80,
                zone: 80,
                overlay: true
            }
        );
        // VISIBLE: exactly what it always was.
        assert_eq!(
            geometry(Shape::default(), 2, 0, false, 0, px),
            Geometry {
                height: 40,
                zone: 40,
                overlay: false
            }
        );
    }

    /// FAILS IF: the on-screen keyboard lies over the windows in any mode.
    /// The owner's rule (2026-09-28): it pushes the windows up in all of them
    /// — it types into the focused window, and lying over it would cover the
    /// very line being typed into. In `Cover` it pushes ALONE: it is the
    /// bottom row of the surface, so the zone can be its height and nothing
    /// more, and what is above it still lies over the windows, veiled.
    #[test]
    fn the_keyboard_pushes_in_every_mode() {
        let px = 20.0;
        // COVER: four rows of keys under the minibuffer; the keys reserve
        // their own height, the minibuffer stays over the windows.
        let cover = Shape::of(Inputs {
            keyboard_out: true,
            minibuffer_busy: true,
            ..inputs(Some(Hidden::Cover))
        });
        assert!(!cover.pushing && cover.veiled());
        assert_eq!(
            geometry(cover, 5, 0, false, 4, px),
            Geometry {
                height: 100,
                zone: 80,
                overlay: true
            }
        );
        // PUSH: it pushes, and so does everything with it, as before.
        let push = Shape::of(Inputs {
            keyboard_out: true,
            ..inputs(Some(Hidden::Push))
        });
        assert_eq!(geometry(push, 5, 0, false, 4, px).zone, 100);
        // VISIBLE: the whole footer, as it always was.
        assert_eq!(geometry(Shape::default(), 6, 0, false, 4, px).zone, 120);
        // AND NO KEYBOARD, nothing reserved in cover.
        let bare = Shape::of(Inputs {
            minibuffer_busy: true,
            ..inputs(Some(Hidden::Cover))
        });
        assert_eq!(geometry(bare, 1, 0, false, 0, px).zone, -1);
    }

    /// FAILS IF: revealing a hidden footer at the edge moves it to another
    /// layer. A surface that changes layer under the pointer is given a
    /// `leave` by the compositor, the `leave` hides the bars, the pointer is
    /// on the edge again, and the footer flickered between shown and hidden
    /// about every 15 ms — measured in the stage: 1044 turns in three seconds
    /// in `Push`, with the windows jumping by the footer's height each time.
    #[test]
    fn the_edge_never_moves_a_hidden_footer_to_another_layer() {
        let px = 20.0;
        for mode in [Hidden::Push, Hidden::Cover] {
            let rest = geometry(Shape::of(inputs(Some(mode))), 0, 0, false, 0, px);
            let edge = Shape::of(Inputs {
                revealed: true,
                ..inputs(Some(mode))
            });
            let shown = geometry(edge, 2, 2, true, 0, px);
            assert_eq!(rest.overlay, shown.overlay, "{mode:?}");
            // NOR DOES ANYTHING ELSE THAT COMES AND GOES while hidden: an app,
            // the band, the keyboard — each of them could be what appears
            // under a resting pointer.
            for pushes in [
                Inputs {
                    app_open: true,
                    ..inputs(Some(mode))
                },
                Inputs {
                    band_or_brain: true,
                    ..inputs(Some(mode))
                },
                Inputs {
                    keyboard_out: true,
                    ..inputs(Some(mode))
                },
            ] {
                assert!(
                    geometry(Shape::of(pushes), 3, 0, true, 0, px).overlay,
                    "{mode:?}"
                );
            }
        }
    }

    /// FAILS IF: the fullscreen edge paints a hidden footer on the desk or
    /// on the veil — rows over a window that fills the screen — or the
    /// painting of any other screen changes.
    #[test]
    fn over_a_fullscreen_window_the_footer_wears_the_chrome() {
        for mode in [None, Some(Hidden::Push), Some(Hidden::Cover)] {
            let shape = Shape::of(Inputs {
                minibuffer_busy: true,
                ..inputs(mode)
            });
            let over = shape.painted(true);
            assert!(!over.hidden && !over.veiled(), "{mode:?}");
            // THE ROWS ARE THE SAME ROWS: only the ground changes.
            assert_eq!(
                (over.modeline_rows, over.minibuffer_rows, over.region_shown),
                (
                    shape.modeline_rows,
                    shape.minibuffer_rows,
                    shape.region_shown
                )
            );
            assert_eq!(shape.painted(false), shape, "{mode:?}");
        }
    }

    /// FAILS IF: a row of the footer leaves the page chosen for it, or the
    /// ground it lays on that page moves. With the bars visible: the page
    /// off the footer's answer (`LEADER s t`), the frame off the page's
    /// ground, or a raised modeline band on a desk's page — a strip of chrome
    /// across a footer asked to be the desk. Hidden: a pushing row off the
    /// desk's page or back on nothing (a clear of alpha 0 draws white fringes
    /// round every glyph, `ronoctua_render::gpu::clear_of`); a row lying over
    /// the windows on anything but the veil of the footer's page; the clear no
    /// longer the nothing the one-pixel edge and the veil need; or a modeline
    /// band of its own. And the fullscreen edge not painted as the visible
    /// footer is.
    #[test]
    fn every_row_stands_on_the_page_chosen_for_it() {
        use crate::theme::{Backdrop, Borders, Choice, Ground, Page};
        use tokens::color;
        for t in tokens::theme::all() {
            for backdrop in Backdrop::ALL {
                for footer in Ground::ALL {
                    let c = Choice {
                        theme: t,
                        backdrop,
                        footer,
                        windows: Ground::Theme,
                        borders: Borders::Standard,
                    };
                    let page_of = |s: Shape| s.page(c.footer_page(), c.desk_page());
                    let band = |s: Shape, page: Page| {
                        (page == Page::Chrome && s.draws_modeline())
                            .then_some(color::MODELINE_BACKGROUND)
                    };
                    // VISIBLE: the footer's page, cleared with its ground, and
                    // the modeline raised on the chrome's page and no other.
                    let visible = Shape::default();
                    let page = page_of(visible);
                    assert_eq!(page, c.footer_page(), "{c:?}");
                    assert_eq!(
                        visible.grounds(page),
                        Grounds {
                            clear: color::SURFACE_CHROME,
                            rows: None,
                            modeline: band(visible, page),
                        },
                        "{c:?}"
                    );
                    for mode in [Hidden::Push, Hidden::Cover] {
                        for bits in 0u8..128 {
                            let s = Shape::of(Inputs {
                                hidden: Some(mode),
                                revealed: bits & 1 != 0,
                                kiosk_focused: bits & 2 != 0,
                                app_open: bits & 4 != 0,
                                minibuffer_busy: bits & 8 != 0,
                                band_or_brain: bits & 16 != 0,
                                list_withheld: bits & 32 != 0,
                                keyboard_out: bits & 64 != 0,
                            });
                            let why = format!("{c:?} {s:?}");
                            let page = page_of(s);
                            let g = s.grounds(page);
                            assert_eq!(g.clear, color::SURFACE_CLEAR, "{why}");
                            assert_eq!(g.modeline, None, "{why}");
                            if s.pushing {
                                // ON THE DESK'S PAGE, whatever the footer's
                                // answer: the rows stand on the desk the
                                // windows moved off, opaque.
                                assert_eq!(page, c.desk_page(), "{why}");
                                assert_ne!(page, Page::Chrome, "{why}");
                                assert_eq!(g.rows, Some(color::SURFACE_CHROME), "{why}");
                            } else {
                                assert_eq!(page, c.footer_page(), "{why}");
                                assert_eq!(g.rows, Some(color::SURFACE_VEIL), "{why}");
                            }
                            // THE FULLSCREEN EDGE is the visible footer, on
                            // the footer's page, in every mode.
                            let edge = s.painted(true);
                            assert_eq!(page_of(edge), c.footer_page(), "{why}");
                            assert_eq!(
                                edge.grounds(c.footer_page()),
                                Grounds {
                                    clear: color::SURFACE_CHROME,
                                    rows: None,
                                    modeline: band(s, c.footer_page()),
                                },
                                "{why}"
                            );
                        }
                    }
                }
            }
        }
        // AND WHAT MAKES THE PAGE HONEST, in the tables: the veil is
        // translucent and the clear is nothing, on every page.
        let opaque = |c: tokens::Color| c.to_rgba8()[3] == u8::MAX;
        assert!(!opaque(color::SURFACE_VEIL));
        assert_eq!(color::SURFACE_CLEAR.to_rgba8()[3], 0);
        assert!(opaque(color::SURFACE_CHROME));
    }

    /// FAILS IF: the footer's pictures are asked for under one page and
    /// looked up under another — the grid drawn blank on the frame the page
    /// changes (the rows begin to push, a preview of the footer's answer),
    /// and for as long as the fullscreen edge holds one screen on another
    /// page than the rest.
    #[test]
    fn the_pictures_wear_the_page_of_the_screen_the_person_is_at() {
        use crate::theme::{Backdrop, Borders, Choice, Ground};
        for t in tokens::theme::all() {
            for backdrop in Backdrop::ALL {
                for footer in Ground::ALL {
                    let c = Choice {
                        theme: t,
                        backdrop,
                        footer,
                        windows: Ground::Theme,
                        borders: Borders::Standard,
                    };
                    let (f, d) = (c.footer_page(), c.desk_page());
                    for hidden in [None, Some(Hidden::Push), Some(Hidden::Cover)] {
                        for bits in 0u8..128 {
                            let s = Shape::of(Inputs {
                                hidden,
                                revealed: bits & 1 != 0,
                                kiosk_focused: bits & 2 != 0,
                                app_open: bits & 4 != 0,
                                minibuffer_busy: bits & 8 != 0,
                                band_or_brain: bits & 16 != 0,
                                list_withheld: bits & 32 != 0,
                                keyboard_out: bits & 64 != 0,
                            });
                            let why = format!("{c:?} {s:?}");
                            // NO EDGE HELD OUT: every screen wears one page,
                            // and the pictures wear it.
                            assert_eq!(s.pictures_page(false, f, d), s.page(f, d), "{why}");
                            // THE EDGE HELD OUT: the pictures are the page of
                            // the screen the edge is on — the visible footer's.
                            assert_eq!(
                                s.pictures_page(true, f, d),
                                s.painted(true).page(f, d),
                                "{why}"
                            );
                            assert_eq!(s.pictures_page(true, f, d), f, "{why}");
                        }
                    }
                }
            }
        }
    }
}
