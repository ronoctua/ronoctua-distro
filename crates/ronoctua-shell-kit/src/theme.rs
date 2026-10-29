//! Which theme this process draws in: where the answer comes from, how it is
//! kept, and how the other doors hear about a change.
//!
//! # Three sources, in order
//!
//! 1. `$XDG_CONFIG_HOME/ronoctua/theme.json` — what the person chose with
//!    `LEADER s t`. Theirs, hand-editable, and it wins.
//! 2. `RONOCTUA_THEME` — the machine's default, exported by the NixOS option
//!    `ronoctua.theme`. What a fresh machine shows.
//! 3. The first theme in `assets/themes/themes.nix`.
//!
//! A source that names a theme that does not exist, or a file that does not
//! parse, falls through to the next and says so at `warn` — loudly, because a
//! file that stopped parsing means the person's choice is silently back to the
//! default, and the symptom would be "my theme reset" with nothing in the log.
//! What the person's own file had refused is said on the footer's line too
//! ([`refused`]), where the file is read.
//!
//! # The file is the broadcast
//!
//! The footer, the file manager's window, the notification and update windows
//! and every buffer's host are separate processes, and the control socket only
//! goes INTO the footer. So the footer does not tell anybody: it writes the
//! file, atomically, and every door holds an inotify watch on it
//! ([`watch`]). The write is the message; a door that was not running picks
//! it up at its next start, from the same file. No second socket vocabulary,
//! which `leader.rs` has been deliberately removing.
//!
//! # The fragments
//!
//! Two consumers outside the shell follow the theme through files the
//! GENERATOR wrote (`nix/themes.nix`, one renderer): the compositor's border
//! colours as `client.focused` lines its config includes, and Ghostty's
//! palette. On a change the
//! footer COPIES the chosen theme's fragments into the configuration
//! directory — `$RONOCTUA_THEMES` says where the generator's output is. The
//! shell renders nothing itself: two renderers of one fragment is how they
//! diverge.
//!
//! # The answers about the theme
//!
//! The file carries five answers beside the theme, each one a choice ABOUT
//! it and none of them a theme of its own (`LEADER s t`, one column each):
//!
//! - [`Backdrop`]: what the compositor paints where no window is — the
//!   theme's own `surface.backdrop`, the ABSOLUTE desk, `surface.absolute`,
//!   which is black in a dark theme and white in a light one, or — in a light
//!   theme only — the near-white between the two, `surface.near`.
//! - `footer` ([`Ground`]): what the modeline, the minibuffer, the region and
//!   the right column stand on — the theme's chrome, or the desk's colour.
//! - `windows` ([`Ground`]): the same question for our windows, the buffers
//!   they host and the terminal.
//! - [`Borders`]: the windows' outlines — standard, or discreet (a quiet one
//!   on the focused window and an extremely faint one on every other).
//! - [`Gaps`]: the space between the windows and round the screen's edge —
//!   the standing 8px, or none at all. Not a colour, so it alone of the five
//!   is not a palette page but a plain compositor fragment of its own
//!   (`scroll-gaps.conf`), switched the same way the rest are.
//!
//! The generator writes one compositor fragment per desk and borders, and one
//! terminal fragment per ground; which ones the footer copies IS the answer
//! ([`fragments`]). An absent answer is the default ([`Choice::with_theme`]);
//! an answer this system does not know is REFUSED — it is not taken for a
//! default quietly: it is named in the log and on the footer's line
//! ([`Complaint`]), exactly as an unknown theme is.
//!
//! # What the painters ask
//!
//! The answers are the PROCESS'S the way the theme is ([`wear`]): set at
//! start, again when the file changes, and by the chooser while it previews.
//! A surface does not paint a different colour on a desk: it WEARS the desk's
//! page ([`Page`]) — the palette the generator solved with that desk in the
//! chrome's place — and paints exactly as it would on the chrome (the owner,
//! 2026-09-28: the bands follow the ground). What it asks, at the moment it
//! draws, is which page: [`footer_page`], [`windows_page`], [`desk_page`];
//! and [`borders`] and [`focus_line`] for the outlines. A preview and a
//! commit reach them by the same road a theme does.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use ronoctua_tokens::Color;
use ronoctua_tokens::color;
use ronoctua_tokens::theme::{self, Theme};

/// The page a surface WEARS — the palette solved on the chrome, or on a desk
/// (`ronoctua_tokens::theme::Ground`). Named apart from this module's
/// [`Ground`], which is the person's ANSWER: the answer says "the desk", and
/// the page says which desk's palette that is.
pub use ronoctua_tokens::theme::Ground as Page;

/// The file, under `$XDG_CONFIG_HOME/ronoctua`:
/// `{ "theme": "forest", "backdrop": "theme", "footer": "desk", "windows":
/// "theme", "borders": "discreet", "gaps": "none" }`. Only `theme` is
/// required; an absent answer is its default.
pub const FILE: &str = "theme.json";

/// The variable naming the generator's output, where the fragments are.
pub const FRAGMENTS_ENV: &str = "RONOCTUA_THEMES";

/// The fragment that says which way round a theme is — `dark` or `light`.
/// Written by `nix/scripts/gen-theme.py`; read by [`dress_foreign_apps`].
pub const MODE_FRAGMENT: &str = "mode";

/// What the compositor paints where no window is — the option `LEADER s t`
/// offers in its second column. Three answers in a light theme and two in a
/// dark one ([`Self::offered`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backdrop {
    /// THE ABSOLUTE DESK, `surface.absolute`: the extreme the theme climbs
    /// away from — the pixel switched off in a dark theme, the paper's white
    /// in a light one. It was `Black`, spelled `"black"`, until a light theme
    /// made the name wrong; a file that still says `"black"` means this one
    /// ([`Self::LEGACY_ABSOLUTE`]).
    Absolute,
    /// NEAR THE ABSOLUTE ONE, `surface.near`: a light theme's near-white,
    /// halfway from the chrome to the paper in the grounds' own hue — the
    /// owner's middle answer (2026-09-28). OFFERED ONLY IN A LIGHT THEME: a
    /// file that says `"near"` under a dark theme is WORN as the absolute
    /// desk ([`Self::worn_in`]) and keeps saying `"near"`, so going back to a
    /// light theme brings the near-white back.
    Near,
    /// The theme's own desk, `surface.backdrop`: a step off the footer's
    /// chrome in a hue of its own, solved by the generator per theme.
    ///
    /// AND THE DEFAULT, at the owner's word. Every theme solves that desk and
    /// every gate holds it apart from the absolute one, so making it what a
    /// fresh machine shows costs nothing and stops the work being invisible
    /// by default. A fresh machine has no `theme.json` and so no fragment
    /// ([`sync_with_file`]): what it shows is the compositor's STATIC config,
    /// whose desk the generator writes from this answer's desk
    /// (`compositor-background`), so the footer's rows that push the windows
    /// away — painted in this desk's page — lie on the desk they claim. The
    /// check `themes-have-fragments` holds the static desk to it.
    #[default]
    Theme,
}

impl Backdrop {
    /// Every answer the FILE may hold, in the order they were declared — not
    /// the chooser's order, and not what one mode offers ([`Self::offered`]).
    pub const ALL: [Self; 3] = [Self::Absolute, Self::Theme, Self::Near];

    /// What a file written before 2026-09-28 says for [`Self::Absolute`]:
    /// the desk was black in every theme then, and was named for it. Read as
    /// the absolute desk and never written again — the next write says
    /// `"absolute"`.
    pub const LEGACY_ABSOLUTE: &'static str = "black";

    /// The answers the chooser OFFERS under a theme that is `light` or not,
    /// in the order it lists them: the extreme, the near one, the theme's own
    /// colour in a light theme; the extreme and the theme's own in a dark one.
    #[must_use]
    pub const fn offered(light: bool) -> &'static [Self] {
        if light {
            &[Self::Absolute, Self::Near, Self::Theme]
        } else {
            &[Self::Absolute, Self::Theme]
        }
    }

    /// The desk this answer PAINTS under a theme that is `light` or not: the
    /// near-white is a light theme's, and under a dark one it is the absolute
    /// desk. The answer itself is not changed — the file keeps `"near"`.
    #[must_use]
    pub const fn worn_in(self, light: bool) -> Self {
        match self {
            Self::Near if !light => Self::Absolute,
            other => other,
        }
    }

    /// The id, as the file spells it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Absolute => "absolute",
            Self::Near => "near",
            Self::Theme => "theme",
        }
    }

    /// The answer an id names — the legacy `"black"` included.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        if id == Self::LEGACY_ABSOLUTE {
            return Some(Self::Absolute);
        }
        Self::ALL.into_iter().find(|b| b.id() == id)
    }

    /// The text key of its name on the chooser's row (invariant 8), under a
    /// theme that is `light` or not: the absolute desk is "Black" under a
    /// dark theme and "White" under a light one, and the near one is "Near
    /// white" — or, under a dark theme where it is worn as the absolute desk,
    /// that desk's name. Ask with the mode of the theme being SHOWN
    /// (`ronoctua_tokens::theme::is_light`), which during a preview is the one
    /// under the cursor.
    #[must_use]
    pub const fn text_key(self, light: bool) -> &'static str {
        match self.worn_in(light) {
            Self::Absolute if light => crate::key::THEME_BACKDROP_WHITE,
            Self::Absolute => crate::key::THEME_BACKDROP_BLACK,
            Self::Near => crate::key::THEME_BACKDROP_NEAR,
            Self::Theme => crate::key::THEME_BACKDROP_THEME,
        }
    }

    /// Where it sits in [`Self::ALL`] — the file's order, which is no
    /// list's on screen (`Self::offered` is), so only the tests ask.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn position(self) -> usize {
        Self::ALL.iter().position(|b| *b == self).unwrap_or(0)
    }

    /// The PAGE of the desk it names — the palette solved on that desk
    /// (`ronoctua_tokens::theme::Ground`). Ask it of the WORN answer
    /// ([`Self::worn_in`]); [`Choice::desk_page`] does.
    #[must_use]
    pub const fn page(self) -> Page {
        match self {
            Self::Absolute => Page::Absolute,
            Self::Near => Page::Near,
            Self::Theme => Page::Backdrop,
        }
    }
}

/// What a surface of ours stands on — asked twice, for the footer and for the
/// windows (`LEADER s t`, the third and fourth columns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ground {
    /// The theme's own ground: the footer's chrome, a window's page.
    #[default]
    Theme,
    /// The desk's colour — whichever [`Backdrop`] is chosen — so the surface
    /// reads as part of the desk rather than as a band across it.
    Desk,
}

impl Ground {
    /// Every answer, in the order the chooser lists them.
    pub const ALL: [Self; 2] = [Self::Theme, Self::Desk];

    /// The id, as the file spells it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Desk => "desk",
        }
    }

    /// The answer an id names.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.id() == id)
    }

    /// The text key of its name on the chooser's row (invariant 8).
    #[must_use]
    pub const fn text_key(self) -> &'static str {
        match self {
            Self::Theme => crate::key::THEME_GROUND_THEME,
            Self::Desk => crate::key::THEME_GROUND_DESK,
        }
    }

    /// Where it sits in [`Self::ALL`].
    #[must_use]
    pub fn position(self) -> usize {
        Self::ALL.iter().position(|g| *g == self).unwrap_or(0)
    }
}

/// How loud the windows' outlines are (`LEADER s t`, the fifth column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Borders {
    /// `border.active` on the focused window, `border.inactive` on the rest.
    #[default]
    Standard,
    /// `border.discreet` on the focused window and an EXTREMELY faint outline
    /// on every other — `border.faint-<desk>`, measured against the desk it
    /// lies on. Never none: the outline is what the pointer resizes by.
    Discreet,
}

impl Borders {
    /// Every answer, in the order the chooser lists them.
    pub const ALL: [Self; 2] = [Self::Standard, Self::Discreet];

    /// The id, as the file spells it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Discreet => "discreet",
        }
    }

    /// The answer an id names.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.id() == id)
    }

    /// The text key of its name on the chooser's row (invariant 8).
    #[must_use]
    pub const fn text_key(self) -> &'static str {
        match self {
            Self::Standard => crate::key::THEME_BORDERS_STANDARD,
            Self::Discreet => crate::key::THEME_BORDERS_DISCREET,
        }
    }

    /// Where it sits in [`Self::ALL`].
    #[must_use]
    pub fn position(self) -> usize {
        Self::ALL.iter().position(|b| *b == self).unwrap_or(0)
    }
}

/// Whether there is space between the windows and round the screen's edge
/// (`LEADER s t`, the sixth column).
///
/// Unlike the four answers above it, this one is NOT a colour: it is the
/// compositor's `gaps` setting, carried in a fragment of its own
/// ([`fragments`]) rather than in a page of the palette. Nothing the shell
/// paints reads it — only the compositor, through the fragment it includes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gaps {
    /// The standing gap — `gaps inner 8`, the eight pixels every window has
    /// had between it and its neighbours and the edge. The default, because
    /// it is what every machine has shown until now.
    #[default]
    With,
    /// No gap at all — `gaps inner 0 / gaps outer 0`: the windows meet one
    /// another and the screen's edge, with only their own border between.
    None,
}

impl Gaps {
    /// Every answer, in the order the chooser lists them.
    pub const ALL: [Self; 2] = [Self::With, Self::None];

    /// The id, as the file spells it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::With => "with",
            Self::None => "none",
        }
    }

    /// The answer an id names.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.id() == id)
    }

    /// The text key of its name on the chooser's row (invariant 8).
    #[must_use]
    pub const fn text_key(self) -> &'static str {
        match self {
            Self::With => crate::key::THEME_GAPS_WITH,
            Self::None => crate::key::THEME_GAPS_NONE,
        }
    }

    /// Where it sits in [`Self::ALL`].
    #[must_use]
    pub fn position(self) -> usize {
        Self::ALL.iter().position(|g| *g == self).unwrap_or(0)
    }
}

/// What the file says, whole: the theme and the five answers about it. They
/// are written together and the fragments follow the whole of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    /// The palette.
    pub theme: Theme,
    /// The desk under the windows (`backdrop` in the file).
    pub backdrop: Backdrop,
    /// What the footer and the right column stand on (`footer`).
    pub footer: Ground,
    /// What our windows and the terminal stand on (`windows`).
    pub windows: Ground,
    /// How loud the windows' outlines are (`borders`).
    pub borders: Borders,
    /// Whether the windows have a gap round them (`gaps`).
    pub gaps: Gaps,
}

impl Choice {
    /// `theme` with every answer at its default — what a file that names only
    /// a theme says, and what a machine that never chose is on.
    #[must_use]
    pub fn with_theme(theme: Theme) -> Self {
        Self {
            theme,
            backdrop: Backdrop::default(),
            footer: Ground::default(),
            windows: Ground::default(),
            borders: Borders::default(),
            gaps: Gaps::default(),
        }
    }

    /// The desk this choice PAINTS: the answer as its theme wears it — a
    /// near-white asked for under a dark theme is the absolute desk.
    #[must_use]
    pub fn worn_backdrop(self) -> Backdrop {
        self.backdrop.worn_in(self.theme.is_light())
    }

    /// The page of the desk this choice paints: the palette solved on it.
    #[must_use]
    pub fn desk_page(self) -> Page {
        self.worn_backdrop().page()
    }

    /// The page the footer WEARS: the base palette on the chrome, or the
    /// desk's page — where the chrome IS the desk and every band and ink was
    /// solved from it (the owner: the bands follow the ground).
    #[must_use]
    pub fn footer_page(self) -> Page {
        match self.footer {
            Ground::Theme => Page::Chrome,
            Ground::Desk => self.desk_page(),
        }
    }

    /// The page our windows wear — every door, and the buffers it hosts.
    #[must_use]
    pub fn windows_page(self) -> Page {
        match self.windows {
            Ground::Theme => Page::Chrome,
            Ground::Desk => self.desk_page(),
        }
    }

    /// The line across the top of the right column when it has the keys: the
    /// accent, or the discreet outline when the borders are discreet — the
    /// column says "focused" the way a window's outline does.
    #[must_use]
    pub const fn focus_line(self) -> Color {
        match self.borders {
            Borders::Standard => color::STATE_ACCENT,
            Borders::Discreet => color::BORDER_DISCREET,
        }
    }

    /// The five answers in one byte, for the process-wide copy ([`wear`]):
    /// the desk in the two low bits (nought is the default, the theme's own),
    /// then one bit each for the footer, the windows, the borders and the
    /// gaps. The ANSWER is kept, not the desk it paints: a near-white under a
    /// dark theme stays `Near` here and is worn as the absolute desk by
    /// [`Self::worn_backdrop`].
    const fn answers(self) -> u8 {
        let desk = match self.backdrop {
            Backdrop::Theme => 0,
            Backdrop::Absolute => 1,
            Backdrop::Near => 2,
        };
        desk | ((matches!(self.footer, Ground::Desk) as u8) << 2)
            | ((matches!(self.windows, Ground::Desk) as u8) << 3)
            | ((matches!(self.borders, Borders::Discreet) as u8) << 4)
            | ((matches!(self.gaps, Gaps::None) as u8) << 5)
    }

    /// The answers a byte from [`Self::answers`] names, on `theme`. Only
    /// `answers` writes the byte, so the fourth pattern of the desk's two bits
    /// never occurs; it reads as the default like the nought it is not.
    const fn with_answers(theme: Theme, bits: u8) -> Self {
        Self {
            theme,
            backdrop: match bits & 3 {
                1 => Backdrop::Absolute,
                2 => Backdrop::Near,
                _ => Backdrop::Theme,
            },
            footer: if bits & 4 == 0 {
                Ground::Theme
            } else {
                Ground::Desk
            },
            windows: if bits & 8 == 0 {
                Ground::Theme
            } else {
                Ground::Desk
            },
            borders: if bits & 16 == 0 {
                Borders::Standard
            } else {
                Borders::Discreet
            },
            gaps: if bits & 32 == 0 {
                Gaps::With
            } else {
                Gaps::None
            },
        }
    }
}

/// The answers this PROCESS draws in, beside the theme `ronoctua_tokens`
/// keeps — one byte behind an atomic, for the reason the theme is one index:
/// every painter asks at the moment it draws, and threading a value through
/// them would carry something only the draw call reads. The defaults until
/// [`resolve_at_start`] says otherwise.
static WORN: AtomicU8 = AtomicU8::new(0);

/// Makes `choice` what this process draws in — the theme and the five
/// answers, together. `true` if anything changed.
///
/// What the file's readers do ([`resolve_at_start`], [`reload`]) and what the
/// chooser does while it previews and when `Esc` or `RET` ends the preview:
/// one road, so the painters cannot tell a preview from a commit.
pub fn wear(choice: Choice) -> bool {
    let theme_moved = theme::set(choice.theme);
    let answers = choice.answers();
    let answers_moved = WORN.swap(answers, Ordering::Relaxed) != answers;
    theme_moved || answers_moved
}

/// What this process draws in now: the theme and the five answers.
#[must_use]
pub fn worn() -> Choice {
    Choice::with_answers(theme::current(), WORN.load(Ordering::Relaxed))
}

/// The page of the desk this process's choice paints — what a hidden footer
/// wears while its rows push the windows away and stand on the desk.
#[must_use]
pub fn desk_page() -> Page {
    worn().desk_page()
}

/// The page the footer (modeline, minibuffer, region) and the right column
/// wear: the chrome's, or the desk's.
#[must_use]
pub fn footer_page() -> Page {
    worn().footer_page()
}

/// The page our windows — every door and the buffers they host — wear.
#[must_use]
pub fn windows_page() -> Page {
    worn().windows_page()
}

/// How loud the windows' outlines are.
///
/// The painters ask it, and none paints with it yet.
#[must_use]
pub fn borders() -> Borders {
    worn().borders
}

/// The right column's focus line: `state.accent`, or `border.discreet`.
#[must_use]
pub fn focus_line() -> Color {
    worn().focus_line()
}

/// The fragments copied on a change: `(name in the theme's directory, name in
/// the configuration directory)`. The compositor `include`s the first and the
/// third — absent is not fatal there by construction — and Ghostty the second
/// with `?`, so a machine that never chose has none of them
/// and is on the default — and [`sync_with_file`] keeps that true, removing
/// them all when there is no file. Which generated file is copied depends on
/// the answers: one compositor fragment per desk and borders, one terminal
/// fragment per ground the windows stand on, one gaps fragment per spacing
/// answer — copying is choosing. A PREVIEW
/// writes them all, because a preview writes the file (see [`show_everywhere`]) —
/// a terminal opened over one wears the palette on screen, which is the
/// point, and `Esc` puts them all back.
///
/// THE GAPS FRAGMENT IS NOT A COLOUR, so it is the same two lines in every
/// theme's directory; it is copied per theme only because every fragment is,
/// and it is `with` or `none` always, never absent — a prior `none` must be
/// put back to `with`, so the `with` variant restates the standing gap rather
/// than leaving the file off (`themes-have-fragments` holds it to the static
/// config's own `gaps`).
#[must_use]
pub fn fragments(choice: Choice) -> [(&'static str, &'static str); 3] {
    // THE DESK AS WORN, not as asked: a near-white under a dark theme is
    // the absolute desk, and that is the fragment the compositor gets.
    let desk = choice.worn_backdrop();
    let compositor = match (desk, choice.borders) {
        (Backdrop::Theme, Borders::Standard) => "scroll-backdrop-standard.conf",
        (Backdrop::Theme, Borders::Discreet) => "scroll-backdrop-discreet.conf",
        (Backdrop::Near, Borders::Standard) => "scroll-near-standard.conf",
        (Backdrop::Near, Borders::Discreet) => "scroll-near-discreet.conf",
        (Backdrop::Absolute, Borders::Standard) => "scroll-absolute-standard.conf",
        (Backdrop::Absolute, Borders::Discreet) => "scroll-absolute-discreet.conf",
    };
    // The terminal's background: the theme's own page, or the desk's colour.
    let terminal = match (choice.windows, desk) {
        (Ground::Theme, _) => "ghostty-theme.conf",
        (Ground::Desk, Backdrop::Theme) => "ghostty-backdrop.conf",
        (Ground::Desk, Backdrop::Near) => "ghostty-near.conf",
        (Ground::Desk, Backdrop::Absolute) => "ghostty-absolute.conf",
    };
    // The windows' spacing: the standing gap, or none.
    let gaps = match choice.gaps {
        Gaps::With => "scroll-gaps-with.conf",
        Gaps::None => "scroll-gaps-none.conf",
    };
    [
        (compositor, "scroll-theme.conf"),
        (terminal, "ghostty-theme.conf"),
        (gaps, "scroll-gaps.conf"),
    ]
}

/// The inner and outer gap, in pixels, the compositor must put on the
/// workspaces ALREADY on screen for `choice`'s spacing.
///
/// # Why this, beside the fragment
///
/// Because this compositor's `gaps` DIRECTIVE is new-workspace only (its
/// manual: "This affects new workspaces only"). The fragment a re-read picks
/// up moves the default the NEXT workspace will have; this moves the
/// here-and-now, through `ronoctua_compositor::Compositor::set_gaps`.
///
/// # Why it is READ and not spelled here
///
/// So this process never names the number. The eight lives in the static
/// config and in the generator's fragment, held equal by
/// `themes-have-fragments`; a third copy in this crate is a third thing to
/// keep in step. The value comes from the SOURCE fragment — always there,
/// every theme has both — so a machine that never chose still gets the
/// default answer's gap, which is what it shows.
///
/// `None` when the generator's output is not where [`FRAGMENTS_ENV`] says, or
/// the fragment does not parse: the compositor then keeps what the re-read
/// gave it, which for the current workspace is the gap it was born with.
#[must_use]
pub fn gaps_pixels(choice: Choice) -> Option<(u16, u16)> {
    gaps_pixels_in(fragments_root()?.as_path(), choice)
}

/// [`gaps_pixels`], with the generator's output root as a parameter — the
/// rule every reader here follows, so a test never reads the real one.
fn gaps_pixels_in(root: &Path, choice: Choice) -> Option<(u16, u16)> {
    let [.., (gaps, _)] = fragments(choice);
    parse_gaps(&std::fs::read_to_string(root.join(choice.theme.id()).join(gaps)).ok()?)
}

/// The `inner` and `outer` a gaps fragment's two lines name, the header
/// comment and any blank line skipped. `None` if either line is absent or not
/// a number — a half-read fragment is not a gap this says to set.
fn parse_gaps(text: &str) -> Option<(u16, u16)> {
    let mut inner = None;
    let mut outer = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("gaps inner ") {
            inner = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("gaps outer ") {
            outer = v.trim().parse().ok();
        }
    }
    Some((inner?, outer?))
}

/// The configuration directory, found ONCE per process in
/// [`resolve_at_start`] and kept here for every reader and writer below.
///
/// The rule is the repository's (`persistence-takes-its-root-as-a-parameter`):
/// a function that reads or writes does not decide where — the application
/// finds the directory once and hands it over, or `cargo test` writes into the
/// home of whoever runs it. The pure functions take the directory as an
/// argument; the process-facing ones take it from here.
static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();

/// The directory, found on the first call and never again — so nothing
/// depends on WHO called first, and a door that somehow ran before
/// [`resolve_at_start`] would still find its file rather than fail quietly.
fn home() -> Option<&'static Path> {
    HOME.get_or_init(ronoctua_xdg::config_dir).as_deref()
}

/// Where the answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The person's file.
    File,
    /// The machine's default, from the environment.
    Env,
    /// The first declared theme.
    Default,
}

/// What was wrong with a source that was passed over — by name, so it can be
/// said where it matters: in the log always, and on the footer's line when
/// it is the person's own file ([`Self::is_the_files`], [`Self::said`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Complaint {
    /// The file is there and does not read as a choice — not JSON, or no
    /// `theme` in it. Every answer in it was refused with it.
    Unreadable(String),
    /// The file names, for `field`, an answer this system does not know. It
    /// was REFUSED — the field is at its default, and this says so; a value
    /// that quietly became a default is how "my choice reset" happens with
    /// nothing anywhere saying why.
    Unknown {
        /// The file's own key: `theme`, `backdrop`, `footer`, `windows`,
        /// `borders` or `gaps`.
        field: &'static str,
        /// What it said, as it said it.
        value: String,
    },
    /// `RONOCTUA_THEME` names a theme that is not declared.
    UnknownDefault(String),
}

impl Complaint {
    /// Is it about the person's file — the one the footer says on its line?
    #[must_use]
    pub const fn is_the_files(&self) -> bool {
        !matches!(self, Self::UnknownDefault(_))
    }

    /// The sentence the footer's line says (invariant 8: the frame is a text
    /// key; the file's name, its key and what it said are the file's own
    /// words, carried as they are).
    #[must_use]
    pub fn said(&self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Unreadable(error) => texts.t_with(
                crate::key::ECHO_THEME_UNREADABLE,
                &[("file", FILE), ("error", error)],
            ),
            Self::Unknown { field, value } => texts.t_with(
                crate::key::ECHO_THEME_REFUSED,
                &[("file", FILE), ("field", field), ("value", value)],
            ),
            Self::UnknownDefault(id) => texts.t_with(
                crate::key::ECHO_THEME_REFUSED,
                &[("file", theme::ENV), ("field", "theme"), ("value", id)],
            ),
        }
    }
}

impl std::fmt::Display for Complaint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(e) => write!(f, "{FILE}: {e}"),
            Self::Unknown { field, value } => {
                write!(
                    f,
                    "{FILE} names an unknown {field} `{value}`; it was refused"
                )
            }
            Self::UnknownDefault(id) => write!(f, "{} names an unknown theme `{id}`", theme::ENV),
        }
    }
}

/// The choice the sources name, where its THEME came from, and what was
/// wrong with the sources that were passed over.
///
/// The theme falls through the three sources; the four answers about it come
/// from the file alone — the machine's default says a theme and nothing
/// else — and an answer the file does not give is its default.
///
/// PURE: the directory and the environment value are parameters, so a test
/// never reads the real home of whoever runs it — the rule every reader in
/// this system follows.
#[must_use]
pub fn resolve_in(dir: Option<&Path>, env: Option<&str>) -> (Choice, Source, Vec<Complaint>) {
    let mut complaints = Vec::new();
    let (written, mut of_the_file) = match dir.map(|dir| read_file(dir, env)) {
        Some(FileSays::Read { written, refused }) => (Some(written), refused),
        Some(FileSays::Broken(c)) => {
            complaints.push(c);
            (None, Vec::new())
        }
        Some(FileSays::Absent) | None => (None, Vec::new()),
    };
    // THE THEME'S OWN COMPLAINT FIRST, then the machine's default's, then the
    // answers' — the order the footer's line has always said them in.
    let theme_refused = of_the_file
        .iter()
        .position(|c| matches!(c, Complaint::Unknown { field: "theme", .. }));
    if let Some(i) = theme_refused {
        complaints.push(of_the_file.remove(i));
    }
    let from_the_file = if theme_refused.is_some() {
        None
    } else {
        written
            .as_ref()
            .and_then(Written::theme_id)
            .and_then(theme::by_id)
    };
    let (t, source) = if let Some(t) = from_the_file {
        (t, Source::File)
    } else if let Some(t) = env.and_then(|id| {
        let found = theme::by_id(id);
        if found.is_none() {
            complaints.push(Complaint::UnknownDefault(id.to_owned()));
        }
        found
    }) {
        (t, Source::Env)
    } else {
        (theme::default_one(), Source::Default)
    };
    complaints.extend(of_the_file);
    let choice = match written {
        Some(w) => answered(t, &w),
        None => Choice::with_theme(t),
    };
    (choice, source, complaints)
}

/// The four answers the file gave, looked up, on `theme` — an answer the
/// store repaired is absent here, so it is its default (and the repair is a
/// complaint, said by [`resolve_in`]).
fn answered(theme: Theme, w: &Written) -> Choice {
    Choice {
        theme,
        backdrop: answer(w.backdrop.as_ref(), Backdrop::by_id),
        footer: answer(w.footer.as_ref(), Ground::by_id),
        windows: answer(w.windows.as_ref(), Ground::by_id),
        borders: answer(w.borders.as_ref(), Borders::by_id),
        gaps: answer(w.gaps.as_ref(), Gaps::by_id),
    }
}

/// The file as a WRITER has to read it: whole, or not at all.
///
/// `Ok(None)` — there is no file; `Ok(Some(choice))` — there is one and every
/// answer in it is known; `Err` — there is one and it does not read WHOLE: it
/// does not parse, it names no theme, or it names an answer this system
/// refuses.
///
/// # Why a writer may not use [`resolve_in`]
///
/// Because `resolve_in` is a READER's answer, and a reader degrades: a field
/// it refuses is its default, and the footer says so. A writer that took that
/// default and wrote the whole choice back would turn the person's refused
/// `"Discreet"` into `"standard"` the moment they turned another column — the
/// first keystroke in the chooser erasing what the footer had just told the
/// person it could not read. The store's write would put an UNTOUCHED answer
/// back, but the chooser writes all five on every arrow and the preview is
/// a write; so the chooser keeps refusing, and the refusal now points
/// somewhere: the repair is in the ledger the belongings' page shows, where
/// "write the fix" makes the file read whole.
fn read_whole(dir: &Path) -> Result<Option<Choice>, Complaint> {
    let env = std::env::var(theme::ENV).ok();
    match read_file(dir, env.as_deref()) {
        FileSays::Absent => Ok(None),
        FileSays::Broken(c) => Err(c),
        FileSays::Read { written, refused } => {
            if let Some(c) = refused.into_iter().next() {
                return Err(c);
            }
            let Some(t) = written.theme_id().and_then(theme::by_id) else {
                return Err(Complaint::Unreadable(NO_THEME.to_owned()));
            };
            Ok(Some(answered(t, &written)))
        }
    }
}

/// Why the file was not written.
#[derive(Debug)]
pub enum NotWritten {
    /// The file is there and does not read whole (`read_whole`): writing
    /// the whole choice would lose what it says. It is left as it is — the
    /// person's to fix or remove, and said on the footer's line.
    Kept(Complaint),
    /// The write itself failed: no configuration directory, or a disk that
    /// refused.
    Io(std::io::Error),
}

impl NotWritten {
    /// The sentence the footer's line says (invariant 8: the frame is a text
    /// key; the file's name and the reason are carried as they are).
    #[must_use]
    pub fn said(&self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Kept(why) => texts.t_with(
                crate::key::ECHO_THEME_KEPT,
                &[("file", FILE), ("why", &why.said(texts))],
            ),
            Self::Io(e) => {
                texts.t_with(crate::key::ECHO_THEME_UNSAVED, &[("error", &e.to_string())])
            }
        }
    }
}

impl std::fmt::Display for NotWritten {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kept(why) => write!(f, "{FILE} was left as it is: {why}"),
            Self::Io(e) => write!(f, "{FILE} could not be written: {e}"),
        }
    }
}

impl From<std::io::Error> for NotWritten {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// What the file was when the chooser opened — what `Esc` puts back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Before {
    /// No file: a machine that never chose. `Esc` takes the preview's file
    /// away again.
    Nothing,
    /// A file that read whole. `Esc` writes back what it said.
    Chosen,
    /// A file that did not read whole. Nothing the chooser does writes over
    /// it ([`NotWritten::Kept`]), so `Esc` has nothing to put back — and
    /// never takes it away.
    Unreadable,
}

/// One answer the file gave, looked up: absent is the default, and known is
/// itself. An answer this system does not know never reaches here — the
/// store repaired it, and [`read_file`] made it a complaint.
fn answer<T: Default>(said: Option<&ronoctua_store::Node>, by_id: fn(&str) -> Option<T>) -> T {
    match said {
        Some(ronoctua_store::Node::String(id)) => by_id(id).unwrap_or_default(),
        _ => T::default(),
    }
}

/// What a file that names no theme is refused with.
const NO_THEME: &str = "no `theme` field";

/// The file's fields as it spells them, before any is looked up — read by the
/// store ([`store_at`]). Each is the JSON it holds, so an answer that is not
/// a string (`3`, `null`) is refused BY NAME, never read as absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Written {
    #[serde(deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    theme: Option<ronoctua_store::Node>,
    #[serde(deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    backdrop: Option<ronoctua_store::Node>,
    #[serde(deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    footer: Option<ronoctua_store::Node>,
    #[serde(deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    windows: Option<ronoctua_store::Node>,
    #[serde(deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    borders: Option<ronoctua_store::Node>,
    #[serde(deserialize_with = "present", skip_serializing_if = "Option::is_none")]
    gaps: Option<ronoctua_store::Node>,
}

/// A field that is THERE is `Some`, whatever it holds — `null` included,
/// which serde's `Option` would read as absent.
fn present<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<ronoctua_store::Node>, D::Error> {
    <ronoctua_store::Node as serde::Deserialize>::deserialize(d).map(Some)
}

impl Written {
    /// The file as a choice writes it: all five, by id.
    fn of(choice: Choice) -> Self {
        let id = |s: &str| Some(ronoctua_store::Node::String(s.to_owned()));
        Self {
            theme: id(choice.theme.id()),
            backdrop: id(choice.backdrop.id()),
            footer: id(choice.footer.id()),
            windows: id(choice.windows.id()),
            borders: id(choice.borders.id()),
            gaps: id(choice.gaps.id()),
        }
    }

    /// The theme's id, when it is a word.
    fn theme_id(&self) -> Option<&str> {
        match &self.theme {
            Some(ronoctua_store::Node::String(id)) => Some(id),
            _ => None,
        }
    }
}

/// The five fields, as the file names them, in the order a complaint is said.
const FIELDS: [&str; 6] = ["theme", "backdrop", "footer", "windows", "borders", "gaps"];

/// What the store refuses in the file: a theme that is not declared, an
/// answer this system does not know, a value that is not a word. Each is a
/// REPAIR — read as absent (the default answer), kept in the file, and
/// written into the ledger; a theme's is read as the machine's default theme,
/// which is what "write the fix" puts in its place.
fn refusals(w: &Written, env: Option<&str>) -> Vec<ronoctua_store::Refused> {
    let mut out = Vec::new();
    for field in FIELDS {
        let value = match field {
            "theme" => &w.theme,
            "backdrop" => &w.backdrop,
            "footer" => &w.footer,
            "windows" => &w.windows,
            "borders" => &w.borders,
            _ => &w.gaps,
        };
        let Some(value) = value else {
            continue;
        };
        let known = match (field, value) {
            ("theme", ronoctua_store::Node::String(id)) => theme::by_id(id).is_some(),
            ("backdrop", ronoctua_store::Node::String(id)) => Backdrop::by_id(id).is_some(),
            ("footer" | "windows", ronoctua_store::Node::String(id)) => Ground::by_id(id).is_some(),
            ("borders", ronoctua_store::Node::String(id)) => Borders::by_id(id).is_some(),
            ("gaps", ronoctua_store::Node::String(id)) => Gaps::by_id(id).is_some(),
            _ => false,
        };
        if known {
            continue;
        }
        let why = if matches!(value, ronoctua_store::Node::String(_)) {
            ronoctua_store::repair::Why::UnknownVariant
        } else {
            ronoctua_store::repair::Why::WrongType
        };
        let kept = (field == "theme").then(|| {
            let fallback = env
                .and_then(theme::by_id)
                .unwrap_or_else(theme::default_one);
            ronoctua_store::Node::String(fallback.id().to_owned())
        });
        out.push(ronoctua_store::Refused {
            pointer: format!("/{field}"),
            why,
            kept,
        });
    }
    out
}

/// `theme.json` as a [`ronoctua_store::Store`] in `dir`, judged by
/// [`refusals`] — on every read and every write alike, which is why the
/// check travels with the store (`ronoctua_store::Store::checked`).
fn store_at(dir: &Path, env: Option<&str>) -> ronoctua_store::Store<Written> {
    let env = env.map(str::to_owned);
    ronoctua_store::Store::at(Some(dir.to_path_buf()), FILE)
        .checked(move |w| refusals(w, env.as_deref()))
}

/// What the person's file says, as the store read it.
enum FileSays {
    /// There is no file.
    Absent,
    /// It gives nothing: not JSON, unreadable, nothing like a choice, or no
    /// theme in it — every answer refused with it.
    Broken(Complaint),
    /// It was read: the fields that were taken, and a complaint for each
    /// that was refused (repaired), in the order of [`FIELDS`].
    Read {
        /// The fields, a refused one absent (a theme's at the fallback).
        written: Written,
        /// What was refused.
        refused: Vec<Complaint>,
    },
}

/// Reads the file through the store — which keeps what reads, repairs what
/// does not (into the ledger) and says so in the log — and names what it
/// refused.
fn read_file(dir: &Path, env: Option<&str>) -> FileSays {
    let reading = store_at(dir, env).reading();
    if !reading.found {
        return FileSays::Absent;
    }
    if let Some(trouble) = &reading.trouble {
        return FileSays::Broken(Complaint::Unreadable(trouble.to_string()));
    }
    if reading.lost_everything() {
        return FileSays::Broken(Complaint::Unreadable(NO_THEME.to_owned()));
    }
    let mut refused = Vec::new();
    for field in FIELDS {
        let pointer = format!("/{field}");
        if let Some(r) = reading.repairs.iter().find(|r| r.pointer == pointer) {
            let value = match &r.found {
                ronoctua_store::Node::String(s) => s.clone(),
                other => other.to_string(),
            };
            refused.push(Complaint::Unknown { field, value });
        }
    }
    // A FILE THAT NAMES NO THEME gives nothing, as it always has: its answers
    // are about a theme nobody named.
    if reading.value.theme.is_none() && refused.is_empty() {
        return FileSays::Broken(Complaint::Unreadable(NO_THEME.to_owned()));
    }
    FileSays::Read {
        written: reading.value,
        refused,
    }
}

/// What the file says, whole, for this process — the footer asks when its
/// chooser opens and when the file changes under it. What was refused is
/// logged at `warn` where the file is READ ([`resolve_at_start`], [`reload`])
/// and said by the footer ([`refused`]); here only at `debug`, or every
/// change of the file would log it three times.
#[must_use]
pub fn committed() -> Choice {
    let env = std::env::var(theme::ENV).ok();
    let (choice, _, complaints) = resolve_in(home(), env.as_deref());
    for c in complaints {
        tracing::debug!(complaint = %c, "a theme source was passed over");
    }
    choice
}

/// What the person's file says that was refused — for the footer to say on
/// its line where it reads the file (at its start and when the file
/// changes), because a refusal only the journal knows is a choice that
/// "reset itself".
#[must_use]
pub fn refused() -> Vec<Complaint> {
    let env = std::env::var(theme::ENV).ok();
    let (_, _, complaints) = resolve_in(home(), env.as_deref());
    complaints
        .into_iter()
        .filter(Complaint::is_the_files)
        .collect()
}

/// The three sources read once, and the choice worn by the process. Called
/// at the top of `main`, before any door; every process of this binary goes
/// through it.
pub fn resolve_at_start() {
    let env = std::env::var(theme::ENV).ok();
    let (choice, source, complaints) = resolve_in(home(), env.as_deref());
    for c in complaints {
        tracing::warn!(complaint = %c, "a theme source was passed over");
    }
    wear(choice);
    // `debug`, not `info`: the short-lived subcommands (`leader`, `command`)
    // pass through here on every keypress, and a line per keypress in the
    // compositor's journal is noise; a CHANGE is logged at `info` by `reload`.
    tracing::debug!(theme = choice.theme.id(), ?source, ?choice, "theme");
}

/// Reads the sources again and wears them. `true` if the theme OR any answer
/// about it changed — a door repaints on either: its ground may be the desk.
///
/// What a door does when its watch fires — and what the footer's own write
/// makes it do too, harmlessly: the same choice comes back and nothing
/// repaints.
pub fn reload() -> bool {
    let env = std::env::var(theme::ENV).ok();
    let (choice, _, complaints) = resolve_in(home(), env.as_deref());
    for c in complaints {
        tracing::warn!(complaint = %c, "a theme source was passed over");
    }
    let changed = wear(choice);
    if changed {
        tracing::info!(
            theme = choice.theme.id(),
            ?choice,
            "the theme changed under this process"
        );
    }
    changed
}

/// Writes the choice, atomically, and refreshes the fragments beside it.
///
/// # Errors
///
/// The file is there and does not read whole, and was left as it is
/// ([`NotWritten::Kept`]); or there is no configuration directory, or it
/// cannot be written.
pub fn persist(choice: Choice) -> Result<(), NotWritten> {
    let dir = home().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no configuration directory")
    })?;
    persist_in(dir, fragments_root().as_deref(), choice)
}

/// [`persist`], with its two roots as parameters — see [`put_back_in`] for why
/// the pair of them exists.
fn persist_in(dir: &Path, root: Option<&Path>, choice: Choice) -> Result<(), NotWritten> {
    persist_with(dir, root, choice, &mut crate::toolkits::wear)
}

/// [`persist_in`], with the hand that dresses the toolkits as a parameter: a
/// test hands its own, because the real one writes the dconf of whoever runs
/// the test.
///
/// THE FILE IS READ FIRST, WHOLE ([`read_whole`]), and a file that does not
/// read whole is not written over — nor are the fragments, nor the toolkits:
/// a choice half shown is a second thing to put back.
fn persist_with(
    dir: &Path,
    root: Option<&Path>,
    choice: Choice,
    dress: &mut dyn FnMut(bool),
) -> Result<(), NotWritten> {
    read_whole(dir).map_err(NotWritten::Kept)?;
    // THROUGH THE STORE: a field a newer machine added to the file survives
    // this write, and the file keeps the order its person wrote it in.
    let env = std::env::var(theme::ENV).ok();
    store_at(dir, env.as_deref()).write(&Written::of(choice))?;
    if let Some(root) = root {
        copy_fragments(root, choice, dir, &fragments(choice));
        dress_foreign_apps(root, choice, dress);
    }
    Ok(())
}

/// Shows a choice to EVERYTHING, which means WRITING THE FILE.
///
/// # Why a preview writes the file a choice writes
///
/// Because the file IS the broadcast. The footer changes its own colours in
/// memory; every other door — the six windows and the compositor — learns of
/// a theme by watching `theme.json` and reading it back. A preview that kept
/// off the file was a preview of one strip of the screen, which is what the
/// owner reported: the minibuffer changed and the windows, the desk and the
/// terminals did not.
///
/// A second channel was the alternative and was refused for the reason this
/// repository refuses it elsewhere: it is a second thing to keep in step, and
/// every reader would have to learn a precedence between the two.
///
/// # What it costs, and what pays it back
///
/// The file that means "chosen" is written while nobody has chosen yet. A
/// footer that dies mid-preview leaves the previewed choice persisted — the
/// same class of loose end the desk's own preview had, one file down. It is a
/// PREFERENCE, reversible with the same two keys that set it, and never data:
/// what is left behind is exactly what `RET` would have written.
///
/// `Esc` is what undoes it: see [`put_everything_back`], which needs to be
/// told whether there was a file before, because by then there always is one.
///
/// # And a file that does not read whole is not written over
///
/// A preview is a write like any other ([`persist`]): over a file the person
/// wrote and this system could not read whole, it writes nothing, and the
/// footer says why. The footer still wears the preview itself; the other
/// doors keep what the file says.
///
/// # Errors
///
/// What [`persist`] refuses or fails with — for the footer's line.
pub fn show_everywhere(choice: Choice) -> Result<(), NotWritten> {
    persist(choice).inspect_err(|e| {
        tracing::warn!(error = %e, "the preview could not be shown to the other doors");
    })
}

/// `Esc` after a preview: everything is shown what it was shown before.
///
/// `before` is the chooser's, taken when it opened — see
/// `ronoctua_shell_footer::theme_chooser::View::before`. It cannot be worked out here any
/// more: the preview wrote the file, so the file always exists by now, and a
/// machine that never chose must not be given one by having looked at the
/// list.
///
/// # Errors
///
/// The file is not what the chooser left — it no longer reads whole, so
/// somebody else wrote it — and it was left alone; or it could not be
/// written. Either is said on the footer's line.
pub fn put_everything_back(committed: Choice, before: Before) -> Result<(), NotWritten> {
    let Some(dir) = home() else {
        return Ok(());
    };
    put_everything_back_in(
        dir,
        fragments_root().as_deref(),
        committed,
        before,
        &mut crate::toolkits::wear,
    )
}

/// [`put_everything_back`], with its two roots and the toolkits' dresser as
/// parameters (see [`persist_with`] for the last).
fn put_everything_back_in(
    dir: &Path,
    root: Option<&Path>,
    committed: Choice,
    before: Before,
    dress: &mut dyn FnMut(bool),
) -> Result<(), NotWritten> {
    match before {
        // WHAT THE FILE SAID, written back — by the same refusing write: a
        // file that stopped reading whole since the chooser opened is not
        // the chooser's any more.
        Before::Chosen => persist_with(dir, root, committed, dress),
        // NOTHING WAS WRITTEN OVER IT, because every preview was refused
        // (`read_whole`), so there is nothing to put back — and it is never
        // taken away: it is the person's, with what they wrote in it.
        Before::Unreadable => Ok(()),
        Before::Nothing => {
            // THE PREVIEW'S FILE, and only that: one that does not read whole
            // was not written by a preview, and is left where it is.
            read_whole(dir).map_err(NotWritten::Kept)?;
            if let Err(e) = std::fs::remove_file(dir.join(FILE))
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(NotWritten::Io(e));
            }
            remove_fragments(dir);
            // AND THE TOOLKITS GO BACK TOO. A preview dressed GTK and Qt for
            // the theme under the cursor (dconf, outside this process), and
            // taking the file away does not undress them: without this, one
            // look at a light theme left every foreign window light on a
            // machine that never chose one.
            if let Some(root) = root {
                dress_foreign_apps(root, committed, dress);
            }
            Ok(())
        }
    }
}

/// What the file is, asked ONCE, when the chooser opens.
#[must_use]
pub fn before() -> Before {
    home().map_or(Before::Nothing, before_in)
}

/// [`before`], with the directory as a parameter.
fn before_in(dir: &Path) -> Before {
    match read_whole(dir) {
        Ok(None) => Before::Nothing,
        Ok(Some(_)) => Before::Chosen,
        Err(_) => Before::Unreadable,
    }
}

/// [`sync_with_file`], with its two roots as parameters.
///
/// It WRITES NOTHING of the person's: the fragments and the toolkits follow
/// the choice the process wears — a refused answer at its default, which the
/// footer has said — and with no file there are no fragments. `theme.json`
/// itself is never written or removed here.
fn put_back_in(dir: &Path, root: Option<&Path>, committed: Choice) {
    let env = std::env::var(theme::ENV).ok();
    if matches!(read_file(dir, env.as_deref()), FileSays::Read { .. }) {
        if let Some(root) = root {
            copy_fragments(root, committed, dir, &fragments(committed));
            dress_foreign_apps(root, committed, &mut crate::toolkits::wear);
        }
    } else {
        remove_fragments(dir);
    }
}

/// Where the generator's output is, or `None` with the reason in the log.
fn fragments_root() -> Option<PathBuf> {
    let root = std::env::var_os(FRAGMENTS_ENV).map(PathBuf::from);
    if root.is_none() {
        tracing::warn!(
            variable = FRAGMENTS_ENV,
            "not set; the compositor and the terminal keep the default theme"
        );
    }
    root
}

/// Copies `which` of the choice's fragments from the generator's output `root`
/// into `dir`. The roots are parameters, so a test never touches the home of
/// whoever runs it.
fn copy_fragments(root: &Path, choice: Choice, dir: &Path, which: &[(&str, &str)]) {
    for (from, to) in which {
        let source = root.join(choice.theme.id()).join(from);
        match std::fs::read(&source) {
            Ok(bytes) => {
                // UNCHANGED BYTES ARE NOT WRITTEN: the footer's own write rings
                // its own watch, and a second copy would be a second reload of
                // the compositor for nothing.
                if std::fs::read(dir.join(to)).is_ok_and(|had| had == bytes) {
                    continue;
                }
                if let Err(e) = ronoctua_xdg::write_atomically(dir, to, &bytes) {
                    tracing::warn!(fragment = to, error = %e, "could not write the fragment");
                }
            }
            Err(e) => {
                tracing::warn!(fragment = %source.display(), error = %e, "could not read the fragment");
            }
        }
    }
}

/// THE MODE FRAGMENT, and it is not a colour.
///
/// A palette cannot be asked which side of the ladder it is on — every gate in
/// the generator measures a MAGNITUDE — so the generator writes the answer
/// down, one word (`mode`), beside the colour fragments: the static config's
/// three values, the cursors', the six compositor fragments and the four
/// terminal ones. This is what reads it.
///
/// It is not copied into the configuration directory like the compositor's
/// and the terminal's: they learn a theme by being given a file, and a
/// toolkit learns one through dconf. Nothing would ever read the copy.
///
/// `dress` is the hand that tells the toolkits — `crate::toolkits::wear`,
/// whose argument is "dark" — a parameter so a test can hold it.
fn dress_foreign_apps(root: &Path, choice: Choice, dress: &mut dyn FnMut(bool)) {
    let source = root.join(choice.theme.id()).join(MODE_FRAGMENT);
    match std::fs::read_to_string(&source) {
        Ok(word) => match word.trim() {
            "dark" => dress(true),
            "light" => dress(false),
            other => tracing::warn!(
                mode = other,
                "the theme's mode fragment says something this shell does not know"
            ),
        },
        // A shell built before the generator wrote this, or a store path that
        // is not there: the toolkits keep what the build gave them, which is
        // the mode of the theme the machine was declared with.
        Err(e) => {
            tracing::debug!(fragment = %source.display(), error = %e, "no mode fragment; the toolkits keep what they had")
        }
    }
}

/// Removes every fragment from `dir`: what "never chose" looks like on disk.
/// A fragment that is not there is not an error.
fn remove_fragments(dir: &Path) {
    for (_, to) in fragments(Choice::with_theme(theme::default_one())) {
        match std::fs::remove_file(dir.join(to)) {
            Ok(()) => tracing::info!(fragment = to, "removed: the file says no theme"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(fragment = to, error = %e, "could not remove the fragment"),
        }
    }
}

/// The fragments are made to agree with the file — at the footer's start,
/// because a rebuild may have moved the generator's output, and whenever the
/// file changes under it from outside, because the fragments must never say
/// a theme the file does not. No file, no fragments: a footer that died in
/// the middle of a preview, or a file the person deleted by hand, leaves
/// nothing behind that the compositor would keep obeying.
pub fn sync_with_file() {
    let Some(dir) = home() else {
        return;
    };
    put_back_in(dir, fragments_root().as_deref(), committed());
}

/// How fast the theme's watch answers, and it is the ONE watch in the shell
/// that does not coalesce.
///
/// The chooser previews by WRITING this file on every keystroke, and the
/// hold-off that serves a listing puts a quarter of a second between the arrow
/// and the colour — measured, 5 ms for the first change and 253 for each one
/// behind it. A listing absorbs a burst because its answer is a `readdir` per
/// event; this one's answer is a repaint, and the burst is a person's hand.
///
/// Named, rather than written into the call, so a test can hold it: the
/// difference the pace makes is measured where the watcher lives, and nothing
/// else would notice this line going back to the other value.
const PACE: ronoctua_files::notice::Pace = ronoctua_files::notice::Pace::AtOnce;

/// A watch on the file, ringing `on_stir` from the watcher's thread when it
/// changes. `None`, with a line in the log, when there is no configuration
/// directory or no inotify instance left — the door then adopts a change at
/// its next start, which is not silent and not the reported bug coming back.
pub fn watch<F>(on_stir: F) -> Option<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + 'static,
{
    let dir = home()?;
    // The directory has to exist for a watch to sit on it, and a door that
    // starts before anyone ever chose is the common case.
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no configuration directory to watch the theme in");
        return None;
    }
    match ronoctua_files::notice::Notice::on_file(dir, FILE, PACE, on_stir) {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::warn!(error = %e, "no theme watcher; a change reaches this door at its next start");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-theme-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn second() -> Option<Theme> {
        theme::all().nth(1)
    }

    /// Every choice the file can hold, on `theme`: three desks, two grounds
    /// twice, two borders and two gaps — forty-eight.
    fn every_choice(theme: Theme) -> Vec<Choice> {
        let mut all = Vec::new();
        for backdrop in Backdrop::ALL {
            for footer in Ground::ALL {
                for windows in Ground::ALL {
                    for borders in Borders::ALL {
                        for gaps in Gaps::ALL {
                            all.push(Choice {
                                theme,
                                backdrop,
                                footer,
                                windows,
                                borders,
                                gaps,
                            });
                        }
                    }
                }
            }
        }
        all
    }

    #[test]
    fn the_file_beats_the_environment_which_beats_the_default() {
        let Some(other) = second() else {
            return; // one theme declared: nothing to prefer
        };
        let dir = scratch("precedence");
        let (c, source, complaints) = resolve_in(Some(&dir), None);
        assert_eq!((c.theme, source), (theme::default_one(), Source::Default));
        assert!(complaints.is_empty());

        let (c, source, _) = resolve_in(Some(&dir), Some(other.id()));
        assert_eq!((c.theme, source), (other, Source::Env));

        std::fs::write(
            dir.join(FILE),
            format!("{{\"theme\":\"{}\"}}", theme::default_one().id()),
        )
        .unwrap();
        let (c, source, complaints) = resolve_in(Some(&dir), Some(other.id()));
        assert_eq!((c.theme, source), (theme::default_one(), Source::File));
        assert!(complaints.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_source_is_passed_over_and_named() {
        let dir = scratch("complaints");
        std::fs::write(dir.join(FILE), "{\"theme\":\"no-such-theme\"}").unwrap();
        let (c, source, complaints) = resolve_in(Some(&dir), Some("also-unknown"));
        assert_eq!((c.theme, source), (theme::default_one(), Source::Default));
        assert_eq!(complaints.len(), 2, "{complaints:?}");
        assert!(complaints[0].to_string().contains("no-such-theme"));
        assert!(complaints[1].to_string().contains("also-unknown"));
        // THE FILE'S IS SAID ON THE FOOTER'S LINE, the machine's default's is
        // only logged: the person did not write that one.
        assert!(complaints[0].is_the_files());
        assert!(!complaints[1].is_the_files());

        std::fs::write(dir.join(FILE), "not json").unwrap();
        let (_, source, complaints) = resolve_in(Some(&dir), None);
        assert_eq!(source, Source::Default);
        assert_eq!(complaints.len(), 1);
        assert!(matches!(complaints[0], Complaint::Unreadable(_)));

        std::fs::write(dir.join(FILE), "{\"colour\":\"owl\"}").unwrap();
        let (_, _, complaints) = resolve_in(Some(&dir), None);
        assert!(
            complaints[0].to_string().contains("no `theme` field"),
            "{complaints:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A generator's output with the fragments of the default theme, faked:
    /// every file `fragments` may name, each saying which one it is.
    fn fake_root(tag: &str) -> PathBuf {
        let root = scratch(tag).join("themed");
        let dir = root.join(theme::default_one().id());
        std::fs::create_dir_all(&dir).unwrap();
        for desk in ["backdrop", "near", "absolute"] {
            for borders in ["standard", "discreet"] {
                let name = format!("scroll-{desk}-{borders}.conf");
                std::fs::write(dir.join(&name), format!("{name}\n")).unwrap();
            }
        }
        for ground in ["theme", "backdrop", "near", "absolute"] {
            let name = format!("ghostty-{ground}.conf");
            std::fs::write(dir.join(&name), format!("{name}\n")).unwrap();
        }
        for spacing in ["with", "none"] {
            let name = format!("scroll-gaps-{spacing}.conf");
            std::fs::write(dir.join(&name), format!("{name}\n")).unwrap();
        }
        root
    }

    #[test]
    fn the_themes_watch_does_not_coalesce() {
        // Every other door repaints from this watch, and the chooser writes
        // the file once per keystroke. `notice::Pace` carries the measurement;
        // this is the line that would silently undo it.
        assert_eq!(PACE, ronoctua_files::notice::Pace::AtOnce);
    }

    #[test]
    fn a_preview_reaches_every_door_and_leaves_nothing_behind_without_a_file() {
        // IT USED TO WRITE THE COMPOSITOR'S FRAGMENT AND NOTHING ELSE, and
        // that is what the owner reported: the minibuffer changed colour, the
        // windows did not, and what they were shown was a theme worn by one
        // strip of the screen. The file is the channel every other door
        // watches, so a preview writes it — and `Esc` is what makes that safe.
        let dir = scratch("preview");
        let root = fake_root("preview-root");
        let committed = Choice {
            backdrop: Backdrop::Absolute,
            ..Choice::with_theme(theme::default_one())
        };
        let tried = Choice {
            backdrop: Backdrop::Theme,
            windows: Ground::Desk,
            borders: Borders::Discreet,
            ..committed
        };

        // A MACHINE THAT NEVER CHOSE. The preview writes the file and both
        // fragments — everything the doors read.
        assert_eq!(before_in(&dir), Before::Nothing);
        persist_in(&dir, Some(&root), tried).unwrap();
        assert!(dir.join(FILE).exists(), "the other doors are told nothing");
        assert_eq!(
            std::fs::read_to_string(dir.join("scroll-theme.conf")).unwrap(),
            "scroll-backdrop-discreet.conf\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("ghostty-theme.conf")).unwrap(),
            "ghostty-backdrop.conf\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("scroll-gaps.conf")).unwrap(),
            "scroll-gaps-with.conf\n"
        );
        // `Esc`: EVERYTHING goes away again. Looking at a list must not be
        // what gives a machine a choice it never made — and by now the file
        // exists, so only the chooser can still say it did not.
        put_everything_back_in(&dir, Some(&root), committed, Before::Nothing, &mut |_| {}).unwrap();
        assert!(
            !dir.join(FILE).exists(),
            "a machine that never chose was given a file by previewing"
        );
        assert!(!dir.join("scroll-theme.conf").exists());
        assert!(!dir.join("ghostty-theme.conf").exists());
        assert!(!dir.join("scroll-gaps.conf").exists());

        // A MACHINE THAT CHOSE. `Esc` puts back what it had, in all three.
        persist_in(&dir, Some(&root), committed).unwrap();
        assert_eq!(before_in(&dir), Before::Chosen);
        persist_in(&dir, Some(&root), tried).unwrap();
        put_everything_back_in(&dir, Some(&root), committed, Before::Chosen, &mut |_| {}).unwrap();
        assert!(dir.join(FILE).exists());
        assert_eq!(
            resolve_in(Some(&dir), None).0,
            committed,
            "the file kept the previewed answers"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("scroll-theme.conf")).unwrap(),
            "scroll-absolute-standard.conf\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("ghostty-theme.conf")).unwrap(),
            "ghostty-theme.conf\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("scroll-gaps.conf")).unwrap(),
            "scroll-gaps-with.conf\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    /// The file's bytes, to say "nothing touched it".
    fn bytes(dir: &Path) -> Vec<u8> {
        std::fs::read(dir.join(FILE)).unwrap_or_default()
    }

    #[test]
    fn a_file_that_does_not_read_whole_is_never_written_over_nor_taken_away() {
        // THE PERSON'S FILE, AS THEY WROTE IT: an answer this system refuses,
        // a file that does not parse, a theme that is not declared. The
        // footer says each is refused — and until this, the chooser then
        // wrote the defaults over the refused answer at the first arrow (or
        // at `Esc` with no arrow at all), and TOOK AWAY a file that did not
        // parse. A reader degrades; a writer refuses (`Store::parsed`).
        let root = fake_root("kept-root");
        let owl = theme::default_one().id();
        let committed = Choice::with_theme(theme::default_one());
        let tried = Choice {
            borders: Borders::Discreet,
            footer: Ground::Desk,
            ..committed
        };
        for (tag, written) in [
            (
                "refused",
                format!("{{\"theme\":\"{owl}\",\"borders\":\"Discreet\"}}"),
            ),
            ("unparsed", format!("{{\"theme\":\"{owl}\",}}")),
            ("no-theme", "{\"colour\":\"owl\"}".to_owned()),
            ("unknown-theme", "{\"theme\":\"no-such-theme\"}".to_owned()),
        ] {
            let dir = scratch(&format!("kept-{tag}"));
            std::fs::write(dir.join(FILE), &written).unwrap();
            assert_eq!(before_in(&dir), Before::Unreadable, "{tag}");
            // A PREVIEW AND `RET` WRITE NOTHING, and say why.
            let refused = persist_in(&dir, Some(&root), tried);
            assert!(
                matches!(refused, Err(NotWritten::Kept(_))),
                "{tag}: {refused:?}"
            );
            assert_eq!(
                bytes(&dir),
                written.as_bytes(),
                "{tag}: the preview wrote over it"
            );
            assert!(
                !dir.join("scroll-theme.conf").exists(),
                "{tag}: a refused preview still showed the compositor a choice"
            );
            // `Esc` HAS NOTHING TO PUT BACK, and takes nothing away.
            put_everything_back_in(
                &dir,
                Some(&root),
                committed,
                Before::Unreadable,
                &mut |_| {},
            )
            .unwrap();
            assert_eq!(bytes(&dir), written.as_bytes(), "{tag}: `Esc` rewrote it");
            // AND NEITHER DOES AN `Esc` THAT EXPECTED SOMETHING ELSE: a file
            // that stopped reading whole since the chooser opened is not the
            // chooser's to rewrite, and not the preview's to take away.
            for before in [Before::Chosen, Before::Nothing] {
                let refused =
                    put_everything_back_in(&dir, Some(&root), committed, before, &mut |_| {});
                assert!(
                    matches!(refused, Err(NotWritten::Kept(_))),
                    "{tag}, {before:?}: {refused:?}"
                );
                assert_eq!(bytes(&dir), written.as_bytes(), "{tag}, {before:?}");
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        // AND THE REFUSAL IS SAID, in the person's language, naming the file
        // and what did not read.
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let said = NotWritten::Kept(Complaint::Unknown {
            field: "borders",
            value: "Discreet".to_owned(),
        })
        .said(&texts);
        assert!(
            said.contains(FILE) && said.contains("borders") && said.contains("Discreet"),
            "{said}"
        );
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn esc_on_a_machine_that_never_chose_dresses_the_toolkits_back() {
        // ONE LOOK AT A THEME OF THE OTHER MODE dressed GTK and Qt for it —
        // dconf, outside this process — and `Esc` took the file away and left
        // them dressed: every foreign window stayed light on a machine that
        // never chose a light theme. `Esc` puts the toolkits back too.
        let home = theme::default_one();
        let Some(other) = theme::all().find(|t| t.is_light() != home.is_light()) else {
            return; // one polarity declared: nothing to cross
        };
        let root = fake_root("dress-root");
        for t in [home, other] {
            let d = root.join(t.id());
            std::fs::create_dir_all(&d).unwrap();
            let word = if t.is_light() { "light" } else { "dark" };
            std::fs::write(d.join(MODE_FRAGMENT), word).unwrap();
        }
        let dir = scratch("dress");
        let committed = Choice::with_theme(home);
        let mut dressed = Vec::new();
        {
            let mut dress = |dark: bool| dressed.push(dark);
            persist_with(&dir, Some(&root), Choice::with_theme(other), &mut dress).unwrap();
            put_everything_back_in(&dir, Some(&root), committed, Before::Nothing, &mut dress)
                .unwrap();
        }
        assert!(!dir.join(FILE).exists());
        assert_eq!(
            dressed,
            [!other.is_light(), !home.is_light()],
            "the toolkits were not dressed back for the theme the machine is on"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn an_absent_answer_is_its_default_and_the_defaults_are_the_owners() {
        // THE THEME'S OWN DESK, the chrome under the footer, the chrome under
        // the windows, the standard borders and the standing gap: what every
        // machine was on before the answers existed, and what a file that
        // names only a theme still says.
        let dir = scratch("defaults");
        let owl = theme::default_one();
        std::fs::write(dir.join(FILE), format!("{{\"theme\":\"{}\"}}", owl.id())).unwrap();
        let (c, _, complaints) = resolve_in(Some(&dir), None);
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            c,
            Choice {
                theme: owl,
                backdrop: Backdrop::Theme,
                footer: Ground::Theme,
                windows: Ground::Theme,
                borders: Borders::Standard,
                gaps: Gaps::With,
            }
        );
        assert_eq!(c, Choice::with_theme(owl));
        // No file and no directory: the same answers.
        assert_eq!(resolve_in(None, None).0, Choice::with_theme(owl));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_answer_is_refused_by_name_and_never_quietly_a_default() {
        // AN UNKNOWN THEME HAS ALWAYS BEEN A COMPLAINT, and an unknown answer
        // about the theme is one too: the field is at its default and the
        // complaint names the field and what it said — which is what the
        // footer's line says (`refused`), where the file is read.
        let dir = scratch("refused");
        let owl = theme::default_one().id();
        for field in ["backdrop", "footer", "windows", "borders", "gaps"] {
            std::fs::write(
                dir.join(FILE),
                format!("{{\"theme\":\"{owl}\",\"{field}\":\"purple\"}}"),
            )
            .unwrap();
            let (c, _, complaints) = resolve_in(Some(&dir), None);
            assert_eq!(c, Choice::with_theme(theme::default_one()), "{field}");
            assert_eq!(
                complaints,
                vec![Complaint::Unknown {
                    field,
                    value: "purple".to_owned()
                }],
                "{field}"
            );
            assert!(complaints[0].is_the_files());
        }
        // A VALUE THAT IS NOT A STRING is refused too, as the JSON it is —
        // it was read as absent, which is a default nobody was told about.
        std::fs::write(
            dir.join(FILE),
            format!("{{\"theme\":\"{owl}\",\"borders\":3,\"footer\":null}}"),
        )
        .unwrap();
        let (_, _, complaints) = resolve_in(Some(&dir), None);
        assert_eq!(
            complaints,
            vec![
                Complaint::Unknown {
                    field: "footer",
                    value: "null".to_owned()
                },
                Complaint::Unknown {
                    field: "borders",
                    value: "3".to_owned()
                },
            ]
        );
        // AND IT IS SAID IN THE PERSON'S LANGUAGE, with the file's own words.
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let said = complaints[1].said(&texts);
        assert!(
            said.contains(FILE) && said.contains("borders") && said.contains('3'),
            "{said}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_legacy_black_is_the_absolute_desk_and_is_written_as_absolute() {
        // A FILE FROM BEFORE 2026-09-28 says `"black"`: the absolute desk was
        // black in every theme then, and was named for it. It is read as the
        // absolute desk with no complaint, and the next write names it.
        let dir = scratch("legacy");
        let owl = theme::default_one();
        std::fs::write(
            dir.join(FILE),
            format!("{{\"theme\":\"{}\",\"backdrop\":\"black\"}}", owl.id()),
        )
        .unwrap();
        let (c, _, complaints) = resolve_in(Some(&dir), None);
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(c.backdrop, Backdrop::Absolute);
        persist_in(&dir, None, c).unwrap();
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(FILE)).unwrap()).unwrap();
        assert_eq!(written["backdrop"], "absolute");
        assert_eq!(
            Backdrop::by_id(Backdrop::LEGACY_ABSOLUTE),
            Some(Backdrop::Absolute)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the chooser's write erases a field a newer machine added to
    /// `theme.json`, or reorders the file — the theme moved into the store,
    /// and the store keeps what it does not know.
    #[test]
    fn a_field_a_newer_machine_wrote_survives_the_choosers_write() {
        let dir = scratch("newer-field");
        let owl = theme::default_one().id();
        std::fs::write(
            dir.join(FILE),
            format!("{{\"shade\":{{\"warmth\":7}},\"theme\":\"{owl}\"}}"),
        )
        .unwrap();
        let (c, _, complaints) = resolve_in(Some(&dir), None);
        assert!(
            complaints.is_empty(),
            "an unknown FIELD is not a complaint: {complaints:?}"
        );
        persist_in(
            &dir,
            None,
            Choice {
                borders: Borders::Discreet,
                ..c
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        let raw: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            raw["shade"]["warmth"], 7,
            "the newer field was erased: {text}"
        );
        assert_eq!(raw["borders"], "discreet");
        assert!(
            text.find("shade") < text.find("borders"),
            "the file's own order was not kept: {text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_choice_is_read_back_as_it_was_written() {
        let dir = scratch("round-trip");
        let t = second().unwrap_or_else(theme::default_one);
        for choice in every_choice(t) {
            persist_in(&dir, None, choice).unwrap();
            let (read, source, complaints) = resolve_in(Some(&dir), None);
            assert_eq!(read, choice);
            assert_eq!(source, Source::File);
            assert!(complaints.is_empty(), "{complaints:?}");
        }
        assert!(!dir.join(format!("{FILE}.new")).exists());
        for b in Backdrop::ALL {
            assert_eq!(Backdrop::by_id(b.id()), Some(b));
            assert_eq!(Backdrop::ALL[b.position()], b);
        }
        for g in Ground::ALL {
            assert_eq!(Ground::by_id(g.id()), Some(g));
            assert_eq!(Ground::ALL[g.position()], g);
        }
        for b in Borders::ALL {
            assert_eq!(Borders::by_id(b.id()), Some(b));
            assert_eq!(Borders::ALL[b.position()], b);
        }
        for g in Gaps::ALL {
            assert_eq!(Gaps::by_id(g.id()), Some(g));
            assert_eq!(Gaps::ALL[g.position()], g);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_answers_fit_in_one_byte_and_come_back_whole() {
        // THE PROCESS-WIDE COPY IS ONE BYTE, and every choice has to survive
        // it — a bit that aliased another would make one column of the
        // chooser move another's colour.
        let t = theme::default_one();
        let mut seen = std::collections::HashSet::new();
        for choice in every_choice(t) {
            let bits = choice.answers();
            assert!(seen.insert(bits), "two choices share {bits:#b}");
            assert_eq!(Choice::with_answers(t, bits), choice);
        }
        // AND NOUGHT IS THE DEFAULTS, which is what the atomic starts at
        // before `resolve_at_start` has read anything.
        assert_eq!(Choice::with_answers(t, 0), Choice::with_theme(t));
    }

    #[test]
    fn the_page_each_surface_wears_follows_every_answer() {
        use ronoctua_tokens::color;
        for c in theme::all().flat_map(every_choice) {
            let desk = match c.backdrop.worn_in(c.theme.is_light()) {
                Backdrop::Theme => Page::Backdrop,
                Backdrop::Near => Page::Near,
                Backdrop::Absolute => Page::Absolute,
            };
            assert_eq!(c.desk_page(), desk);
            let on_the_desk = |g: Ground| match g {
                Ground::Theme => Page::Chrome,
                Ground::Desk => desk,
            };
            assert_eq!(c.footer_page(), on_the_desk(c.footer), "{c:?}");
            assert_eq!(c.windows_page(), on_the_desk(c.windows), "{c:?}");
            assert_eq!(
                c.focus_line(),
                match c.borders {
                    Borders::Standard => color::STATE_ACCENT,
                    Borders::Discreet => color::BORDER_DISCREET,
                }
            );
        }
    }

    #[test]
    fn the_fragments_are_the_generators_names_for_every_answer() {
        // THE NAMES `gen-theme.py` WRITES: `scroll-<desk>-<borders>.conf`,
        // `ghostty-<ground>.conf` (the terminal's ground being the theme's
        // own page or the chosen desk) and `scroll-gaps-<with|none>.conf`. The
        // check `themes-have-fragments` holds the generator to the same list.
        for c in theme::all().flat_map(every_choice) {
            let [
                (compositor, to_compositor),
                (terminal, to_terminal),
                (gaps, to_gaps),
            ] = fragments(c);
            let desk = match c.backdrop.worn_in(c.theme.is_light()) {
                Backdrop::Theme => "backdrop",
                Backdrop::Near => "near",
                Backdrop::Absolute => "absolute",
            };
            assert_eq!(compositor, format!("scroll-{desk}-{}.conf", c.borders.id()));
            let ground = match c.windows {
                Ground::Theme => "theme",
                Ground::Desk => desk,
            };
            assert_eq!(terminal, format!("ghostty-{ground}.conf"));
            assert_eq!(gaps, format!("scroll-gaps-{}.conf", c.gaps.id()));
            assert_eq!(
                (to_compositor, to_terminal, to_gaps),
                (
                    "scroll-theme.conf",
                    "ghostty-theme.conf",
                    "scroll-gaps.conf"
                )
            );
        }
    }

    #[test]
    fn the_gap_pixels_are_read_from_the_generators_fragment_not_spelled_here() {
        // THE NUMBER IS THE GENERATOR'S, read back — not a constant in this
        // crate that could drift from the static config and the fragment
        // (`themes-have-fragments` holds those two equal; this reads what they
        // agree on). The header comment and blank lines are skipped.
        assert_eq!(
            parse_gaps("# generated\ngaps inner 8\ngaps outer 0\n"),
            Some((8, 0))
        );
        assert_eq!(parse_gaps("gaps inner 0\ngaps outer 0"), Some((0, 0)));
        assert_eq!(parse_gaps("gaps inner 8"), None, "a half fragment is None");
        assert_eq!(parse_gaps("nothing about gaps"), None);

        // AND READ FROM THE SOURCE FRAGMENT the generator wrote, by choice.
        let root = scratch("gap-pixels").join("themed");
        let dir = root.join(theme::default_one().id());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("scroll-gaps-with.conf"),
            "gaps inner 8\ngaps outer 0\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("scroll-gaps-none.conf"),
            "gaps inner 0\ngaps outer 0\n",
        )
        .unwrap();
        let with = Choice::with_theme(theme::default_one());
        assert_eq!(gaps_pixels_in(&root, with), Some((8, 0)));
        assert_eq!(
            gaps_pixels_in(
                &root,
                Choice {
                    gaps: Gaps::None,
                    ..with
                }
            ),
            Some((0, 0))
        );
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
    }

    #[test]
    fn every_answer_has_its_name_in_the_texts_and_the_absolute_desk_follows_the_mode() {
        // "PRETO" UNDER A DARK THEME AND "BRANCO" UNDER A LIGHT ONE: the
        // absolute desk is the extreme the theme climbs away from.
        assert_eq!(
            Backdrop::Absolute.text_key(false),
            crate::key::THEME_BACKDROP_BLACK
        );
        assert_eq!(
            Backdrop::Absolute.text_key(true),
            crate::key::THEME_BACKDROP_WHITE
        );
        assert_eq!(
            Backdrop::Theme.text_key(true),
            Backdrop::Theme.text_key(false)
        );
        // "QUASE BRANCO" UNDER A LIGHT THEME, and under a dark one the name of
        // the desk it is worn as.
        assert_eq!(
            Backdrop::Near.text_key(true),
            crate::key::THEME_BACKDROP_NEAR
        );
        assert_eq!(
            Backdrop::Near.text_key(false),
            crate::key::THEME_BACKDROP_BLACK
        );
        let keys = Backdrop::ALL
            .iter()
            .flat_map(|b| [b.text_key(false), b.text_key(true)])
            .chain(Ground::ALL.iter().map(|g| g.text_key()))
            .chain(Borders::ALL.iter().map(|b| b.text_key()))
            .chain(Gaps::ALL.iter().map(|g| g.text_key()))
            .chain([
                crate::key::THEME_OPTION_FOOTER,
                crate::key::THEME_OPTION_WINDOWS,
                crate::key::THEME_OPTION_BORDERS,
                crate::key::THEME_OPTION_GAPS,
            ]);
        let every: Vec<ronoctua_i18n::Texts> = ronoctua_i18n::LANGUAGES
            .iter()
            .map(|l| ronoctua_i18n::Texts::fresh(l).expect("a declared language"))
            .collect();
        for key in keys {
            for texts in &every {
                assert!(texts.has(key), "{key} has no text in {}", texts.language());
            }
        }
    }

    #[test]
    fn a_light_theme_offers_three_desks_and_a_dark_one_two() {
        assert_eq!(
            Backdrop::offered(true),
            &[Backdrop::Absolute, Backdrop::Near, Backdrop::Theme]
        );
        assert_eq!(
            Backdrop::offered(false),
            &[Backdrop::Absolute, Backdrop::Theme]
        );
        // EVERYTHING OFFERED IS AN ANSWER THE FILE CAN HOLD, and every answer
        // is offered somewhere.
        for b in Backdrop::ALL {
            assert!(
                Backdrop::offered(true).contains(&b),
                "{b:?} is offered nowhere"
            );
        }
    }

    #[test]
    fn a_near_desk_under_a_dark_theme_is_worn_as_the_absolute_one_and_kept() {
        // THE NEAR-WHITE IS A LIGHT THEME'S. Asked for under a dark theme it
        // is PAINTED as the absolute desk — its colour, its veil, its
        // fragments — and the file keeps saying "near", so going back to a
        // light theme brings it back.
        let (Some(dark), Some(light)) = (
            theme::all().find(|t| !t.is_light()),
            theme::all().find(|t| t.is_light()),
        ) else {
            return; // one polarity declared: nothing to carry across
        };
        let dir = scratch("near");
        let asked = Choice {
            backdrop: Backdrop::Near,
            footer: Ground::Desk,
            windows: Ground::Desk,
            ..Choice::with_theme(dark)
        };
        persist_in(&dir, None, asked).unwrap();
        let (read, _, complaints) = resolve_in(Some(&dir), None);
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            read.backdrop,
            Backdrop::Near,
            "the file forgot the near-white"
        );
        assert_eq!(read.worn_backdrop(), Backdrop::Absolute);
        assert_eq!(read.desk_page(), Page::Absolute);
        assert_eq!(read.footer_page(), Page::Absolute);
        assert_eq!(
            fragments(read).map(|(from, _)| from),
            [
                "scroll-absolute-standard.conf",
                "ghostty-absolute.conf",
                "scroll-gaps-with.conf"
            ]
        );
        // BACK TO A LIGHT THEME: the same answer is the near-white again.
        let back = Choice {
            theme: light,
            ..read
        };
        assert_eq!(back.worn_backdrop(), Backdrop::Near);
        assert_eq!(back.desk_page(), Page::Near);
        assert_eq!(
            fragments(back).map(|(from, _)| from),
            [
                "scroll-near-standard.conf",
                "ghostty-near.conf",
                "scroll-gaps-with.conf"
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
