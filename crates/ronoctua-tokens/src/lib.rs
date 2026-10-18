//! ronoctua-distro's design system tokens.
//!
//! **Invariant 9: zero hard-coded visual values.** Node colour, font,
//! size, spacing or icon codepoint lives outside here.
//!
//! The way to make that hold is not discipline — it is there being no way to write
//! a loose value. [`Color`] has no public constructor; the only instances
//! that exist are the constants generated in [`color`].
//!
//! # Where this comes from
//!
//! The tokens are defined **once**, in `assets/tokens/*.nix` (section 7.6) —
//! the colours as RECIPES that `nix/themes.nix` solves into one palette per
//! theme in `assets/themes/themes.nix`. The build generates this crate's Rust
//! table from the NAMES: one constant per role, the spacing, the sizes, the
//! icons' codepoints. Editing the generated code is pointless: it disappears
//! at the next build.
//!
//! # The palettes are data
//!
//! Not one colour's bytes is compiled in. They are the palettes the colour
//! generator writes (`palettes.bin`), read at run time from
//! [`palettes::ENV`] the first time anything asks a colour or a theme
//! ([`palettes`]): every theme's id, its order, the default, which way it
//! climbs, and every role's RGBA on every page. A seed, a recipe's value, the
//! theme list and the generator itself can change without one crate being
//! compiled again. A binary refuses a file written for other roles, loudly —
//! a palette read through the wrong index is every colour wrong with nothing
//! failing.
//!
//! # A colour is a role; the theme is the process's
//!
//! `color::TEXT_DIM` is not bytes, it is "the dim text", and which bytes that
//! is depends on the theme the process is in ([`theme`]). The 280 places that
//! name a colour never changed for this: they name the role, and the role
//! resolves when it is drawn. Switching theme is one atomic store, and the
//! next frame is in the new palette.
//!
//! # Why zero dependencies
//!
//! This crate sits under the renderer, the shell, the file manager's helper,
//! the installer and the rescue console. Any dependency here becomes a
//! dependency of all of them.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// A design-system colour: a ROLE, resolved through the current theme the
/// moment it is asked for.
///
/// **There is no public constructor.** The only colours that exist are the
/// constants in [`color`], generated from `assets/tokens/colors.nix` — and each
/// is a role, not a value: `SURFACE_BASE` is "the content's ground", and which
/// bytes that is depends on the theme the process is in ([`theme::set`]).
/// Equality and hashing are BY ROLE: two roles that happen to share bytes in
/// one theme are still two roles, which is what every comparison in the shell
/// means by `==` anyway ("is this field drawn as dim text?").
///
/// That is invariant 9 being structural instead of agreed, twice over: no way
/// to write a loose value, and no way to hold a value that a theme change
/// would leave stale.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(u16);

impl Color {
    /// A **crate-private** constructor: only the generated table calls it.
    const fn at(role: u16) -> Self {
        Self(role)
    }

    /// The token's name, `text.dim`, for a log line or a test's message.
    #[must_use]
    pub const fn name(self) -> &'static str {
        color::NAMES[self.0 as usize]
    }

    /// Raw components, in RGBA order, in the CURRENT palette: the theme the
    /// process is in, on the ground it wears ([`theme::Ground`]).
    ///
    /// Not `const`: it reads which palette the process is in, and that is the
    /// whole point.
    ///
    /// # Panics
    ///
    /// When the palettes cannot be read — see [`palettes::load`].
    #[must_use]
    pub fn to_rgba8(self) -> [u8; 4] {
        self.to_rgba8_in(theme::palette())
    }

    /// Raw components, in RGBA order, in the palette `palette` names — a
    /// number [`theme::palette`] gave — whatever the process wears NOW.
    ///
    /// For a painter that is not on the frame's thread: a worker drawing a
    /// picture while the footer paints its screens, each wearing its own page
    /// ([`theme::set_ground`]), would otherwise read whichever page the loop
    /// had just put on — and a texture keyed for one palette could carry
    /// another's colours. The palette travels with the ask, and the bytes are
    /// the ask's.
    ///
    /// # Panics
    ///
    /// On a number no palette has. It only ever comes from
    /// [`theme::palette`], so a guessed one is a defect to see, not a colour.
    /// And when the palettes cannot be read — see [`palettes::load`].
    #[must_use]
    pub fn to_rgba8_in(self, palette: usize) -> [u8; 4] {
        palettes::get().rgba(palette, usize::from(self.0))
    }

    /// Components in `[0.0, 1.0]` in RGBA order, in **LINEAR** space.
    ///
    /// The tokens are written in sRGB, which is how one writes colour. A wgpu
    /// `Rgba8UnormSrgb` target expects LINEAR values and does the sRGB
    /// encoding on write — so passing `r/255` straight through leaves the curve out and
    /// lightens everything: the footer background's `0x14` reaches the screen as `0x4D`. The
    /// mistake does not show up as a failure, only as a colour nobody can
    /// trace back to the token.
    ///
    /// The alpha does NOT go through the curve: sRGB is a luminance transfer, and
    /// opacity is not luminance.
    #[must_use]
    pub fn to_linear(self) -> [f32; 4] {
        linear_of(self.to_rgba8())
    }
}

impl core::fmt::Debug for Color {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "color.{}", self.name())
    }
}

/// The sRGB curve undone, on bytes. A pure function, so the test that proves
/// it needs no theme.
fn linear_of([r, g, b, a]: [u8; 4]) -> [f32; 4] {
    fn channel(v: u8) -> f32 {
        let c = f32::from(v) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    [channel(r), channel(g), channel(b), f32::from(a) / 255.0]
}

/// Which PALETTE the process resolves its colours through: a theme, and the
/// ground it is worn on.
///
/// One index, process-wide, behind an atomic: a theme is a property of the
/// process the way the language is, and threading it through the 280 places
/// that name a colour would carry a value nothing but the draw call reads.
/// [`set`] is called at start (`theme.json` > `RONOCTUA_THEME` > the first
/// theme) and again when the person chooses; every `Color` resolves through
/// it at the moment it is drawn, so the next frame is in the new palette.
///
/// # The ground
///
/// Every theme is solved on four pages (`nix/scripts/gen-theme.py`): its own
/// chrome, and each desk the person may put the footer or the windows on
/// (`LEADER s t`) — the owner's "the bands follow the ground" (2026-09-28).
/// On a desk's page `color::SURFACE_CHROME` IS that desk, and every text,
/// band and rule was solved from it; the notification band, the brain's
/// line, the glance and the outlines are the same on every page. A surface
/// that stands on a desk wears that page ([`set_ground`]) and paints exactly
/// as it would on the chrome.
pub mod theme {
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// The palette: `theme * PAGES + ground`, one word so a frame never reads
    /// a theme from one store and a ground from another.
    static CURRENT: AtomicUsize = AtomicUsize::new(0);

    /// The page a palette is solved on — the ground everything is measured
    /// against. In the table's order.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub enum Ground {
        /// The theme's own chrome: the base palette.
        #[default]
        Chrome,
        /// The theme's coloured desk, `surface.backdrop`.
        Backdrop,
        /// The near desk, `surface.near` (a light theme's near-white).
        Near,
        /// The absolute desk, `surface.absolute`: black or white.
        Absolute,
    }

    impl Ground {
        /// Every page, in the table's order.
        pub const ALL: [Self; 4] = [Self::Chrome, Self::Backdrop, Self::Near, Self::Absolute];

        /// Where it sits in the table.
        #[must_use]
        pub const fn position(self) -> usize {
            match self {
                Self::Chrome => 0,
                Self::Backdrop => 1,
                Self::Near => 2,
                Self::Absolute => 3,
            }
        }

        /// The page's name, as the generator writes it (`chrome`, `backdrop`,
        /// `near`, `absolute`).
        #[must_use]
        pub const fn id(self) -> &'static str {
            super::themes::PAGES[self.position()]
        }
    }

    /// One of the declared themes. Only this module makes them, which is what
    /// keeps an index inside the table.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Theme {
        index: usize,
    }

    impl Theme {
        /// The id, as `assets/themes/themes.nix` spells it: `owl`. Read from
        /// the palettes' header (`super::palettes`).
        #[must_use]
        pub fn id(self) -> &'static str {
            super::palettes::get().id(self.index)
        }

        /// Where it sits in the declared order; the default is `0`.
        #[must_use]
        pub const fn position(self) -> usize {
            self.index
        }

        /// A LIGHT theme: the paper is its ground and its ladder walks toward
        /// black. Declared in the seed (`mode`), carried by the generator in
        /// the palettes beside the colours — a colour cannot be asked which
        /// side of the ladder it is on.
        #[must_use]
        pub fn is_light(self) -> bool {
            super::palettes::get().is_light(self.index)
        }
    }

    /// Is the theme every `Color` resolves through right now a light one?
    ///
    /// What names the absolute desk — `surface.absolute` is black in a dark
    /// theme and white in a light one, and the chooser has to say which.
    #[must_use]
    pub fn is_light() -> bool {
        current().is_light()
    }

    /// Is the theme at `index` (a [`Theme::position`]) a light one?
    ///
    /// # Panics
    ///
    /// On an index that names no theme — an index is only ever a position a
    /// `Theme` gave, and a guessed one is a defect to see, not a dark theme.
    #[must_use]
    pub fn is_light_at(index: usize) -> bool {
        super::palettes::get().is_light(index)
    }

    /// Every theme, in `assets/themes/themes.nix` order — the chooser's order,
    /// which is not the default's ([`default_one`]).
    pub fn all() -> impl ExactSizeIterator<Item = Theme> {
        (0..super::palettes::get().themes()).map(|index| Theme { index })
    }

    /// What a process starts in before anything says otherwise.
    ///
    /// NOT the first declared theme, which it was until the owner asked for
    /// the chooser's ORDER and the system's DEFAULT to stop being one fact —
    /// see `assets/themes/themes.nix`. The palettes' header carries the
    /// index of the name declared over there.
    #[must_use]
    pub fn default_one() -> Theme {
        Theme {
            index: super::palettes::get().default_theme(),
        }
    }

    /// The theme with this id, if it is declared.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Theme> {
        all().find(|t| t.id() == id)
    }

    /// The theme every `Color` resolves through right now.
    #[must_use]
    pub fn current() -> Theme {
        Theme { index: index().0 }
    }

    /// The ground the process wears right now.
    #[must_use]
    pub fn ground() -> Ground {
        Ground::ALL[index().1]
    }

    /// The palette as ONE number, for a cache whose picture was painted with
    /// token values: it changes when the theme OR the ground does, which is
    /// exactly when such a picture goes stale.
    #[must_use]
    pub fn palette() -> usize {
        CURRENT.load(Ordering::Relaxed)
    }

    /// Makes `t` the theme every `Color` resolves through, process-wide, on
    /// the ground the process already wears.
    ///
    /// `true` if it changed. `Relaxed` is enough: one self-contained word,
    /// nothing published alongside it; a frame that started under the old
    /// theme finishes under it, and the next one is in the new.
    pub fn set(t: Theme) -> bool {
        let pages = super::counts::PAGES;
        let was = CURRENT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |p| {
                Some(t.index * pages + p % pages)
            })
            .unwrap_or_else(|p| p);
        was / pages != t.index
    }

    /// Makes `g` the ground the process wears, in the theme it is in: every
    /// `Color` resolves through that page's palette from here on. `true` if
    /// it changed.
    pub fn set_ground(g: Ground) -> bool {
        let pages = super::counts::PAGES;
        let was = CURRENT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |p| {
                Some(p / pages * pages + g.position())
            })
            .unwrap_or_else(|p| p);
        was % pages != g.position()
    }

    /// The variable a NixOS option exports with the machine's default theme
    /// (`ronoctua.theme`). What the person chose lives in a file the shell
    /// reads first; this is the floor under it.
    pub const ENV: &str = "RONOCTUA_THEME";

    /// The theme [`ENV`] names, if it is set and declared. The caller logs a
    /// name that is set and unknown.
    #[must_use]
    pub fn from_env() -> Option<Theme> {
        std::env::var(ENV).ok().and_then(|id| by_id(&id))
    }

    /// `(theme, ground)`, from the one word.
    pub(crate) fn index() -> (usize, usize) {
        let p = CURRENT.load(Ordering::Relaxed);
        (p / super::counts::PAGES, p % super::counts::PAGES)
    }
}

/// How big the text is, as a percentage of `font::SIZE_BASE`.
///
/// ONE NUMBER, PROCESS-WIDE, BEHIND AN ATOMIC — the arrangement `theme` above
/// uses, and for the same reason. The size the host draws at is asked for in
/// every place that puts a pixel down, and threading a value through all of
/// them would carry something only the draw call reads.
///
/// HOW MANY PLACES IS NOT WRITTEN HERE. Three comments once said eighty-four,
/// eighty-three and eighty-two, and the true number was eighty-one — a count
/// in prose is a number nobody updates. The `one-text-size` check counts them
/// on every build and prints the total, which is the only place it can be
/// right.
///
/// Everything else is already measured against the base: a cell is
/// `size x font::CELL_WIDTH_EM`, a line is `size x font::LINE_HEIGHT`, and
/// `metrics::columns` divides one by the other. So scaling the base scales
/// the whole arrangement, and not one sum in the layout has to change — which
/// is what `font.size.base`'s own note in `assets/tokens/typography.nix`
/// promised long before anything could move it.
pub mod text {
    use core::sync::atomic::{AtomicU16, Ordering};

    /// The size the base was measured at. A machine that has never been told
    /// otherwise draws here.
    pub const NORMAL: u16 = 100;

    static CHOSEN: AtomicU16 = AtomicU16::new(NORMAL);

    /// The percentage nearest `wanted` that the person can actually arrive at.
    ///
    /// Snapped to `font::SCALE_STEP` and held inside the declared range,
    /// because a value from a hand-edited file has to land somewhere the
    /// arrows can leave: 103% would otherwise be a number no keypress could
    /// reach or return to.
    #[must_use]
    pub fn allowed(wanted: u16) -> u16 {
        let step = super::font::SCALE_STEP.max(1);
        let held = wanted.clamp(super::font::SCALE_FLOOR, super::font::SCALE_CEILING);
        let snapped = ((u32::from(held) + u32::from(step) / 2) / u32::from(step)) * u32::from(step);
        // THE SECOND CLAMP CANNOT FIRE WITH TODAY'S TOKENS, and it is here for
        // the day they move: rounding to a step can carry a value one step
        // past the ceiling when the ceiling is not itself a multiple of the
        // step. The generator refuses that arrangement, so this is the wall
        // behind the gate rather than a live branch.
        u16::try_from(snapped)
            .unwrap_or(super::font::SCALE_CEILING)
            .clamp(super::font::SCALE_FLOOR, super::font::SCALE_CEILING)
    }

    /// What the host is drawing at right now.
    #[must_use]
    pub fn percent() -> u16 {
        CHOSEN.load(Ordering::Relaxed)
    }

    /// Makes `wanted` the size the host draws at, process-wide.
    ///
    /// It is put through [`allowed`] first. `true` if it changed.
    ///
    /// `Relaxed` IS ENOUGH AS AN ORDERING MATTER — one self-contained word,
    /// with nothing published alongside it, exactly as `theme::set` argues.
    ///
    /// WHAT KEEPS A FRAME WHOLE IS NOT THE ATOMIC. A frame reads this many
    /// times, in its layout and again in its painting, so a `set` landing
    /// between two of those reads would lay out on one grid and paint on
    /// another. Nothing here prevents that; what does is WHERE the call is
    /// made — the event loop handles a key, sets the size, and only then
    /// draws, so the two never interleave. A caller that set this from
    /// another thread would break a frame, and that is the rule rather than
    /// an implementation detail.
    pub fn set(wanted: u16) -> bool {
        let held = allowed(wanted);
        CHOSEN.swap(held, Ordering::Relaxed) != held
    }

    /// ANY size of the design, moved by the chosen percentage.
    ///
    /// EVERY size the host draws at goes through here, and not only the base.
    /// `font.size.small` is 15 against a base of 18 — five sixths — so
    /// leaving it fixed means that below 84% the "small" size is TALLER than
    /// the ordinary one: a small run then occupies two cells instead of one,
    /// and a layout that reserves cells by scale loses a line without saying
    /// so.
    ///
    /// MEASURED, not reasoned: `ronoctua-buffer-memory` drawn on a 60x12
    /// grid gave thirteen runs at 100% and ELEVEN at 80%, and the step is
    /// between 80 and 85 — which is 15/18. Two runs vanished off a picture
    /// whose grid had not changed. The buffer is named so the measurement can
    /// be taken again: `ronoctua-render-png <it> --columns 60 --lines 12
    /// --text 80`.
    #[must_use]
    pub fn scaled(size: f32) -> f32 {
        size * f32::from(percent()) / 100.0
    }

    /// The size to draw at, in logical pixels.
    ///
    /// THIS IS WHAT THE HOST ASKS FOR, never `font::SIZE_BASE` — that
    /// constant is the anchor the percentage is measured against, and reading
    /// it directly is drawing at a size the person cannot move. The
    /// `one-text-size` check is what keeps that true.
    ///
    /// # Why there is ONE of these, and not one per door
    ///
    /// There were two. The footer drew at `font.size.base` and every window
    /// of the shell at `font.size.large`, with an enum existing so that a
    /// test could say the doors differed in that one thing and nothing could
    /// quietly add a second difference. The difference was deliberate — "the
    /// footer is peripheral reference, the buffer is what one reads".
    ///
    /// The owner photographed the file manager's two doors side by side and
    /// asked for the footer's size everywhere, buffers included: it is the
    /// one he reads all day, and a system whose two halves are 10% apart
    /// looks assembled out of parts of different origins. The enum had
    /// nothing left to name and went; what replaced it was a constant in the
    /// shell's `chrome`, and what replaced THAT is this — because a size the
    /// person can move is a property of the process and not of the build.
    #[must_use]
    pub fn size() -> f32 {
        scaled(super::font::SIZE_BASE)
    }

    /// One step from `from`, in the direction asked for.
    ///
    /// ONE ARITHMETIC FOR TWO GESTURES, so that holding a key and holding an
    /// arrow arrive at the same numbers — including at 100 exactly, which the
    /// step is chosen to divide.
    ///
    /// # Why it takes a starting point instead of reading the size
    ///
    /// Because the size is a process-wide atomic that only moves after a
    /// round trip — the gesture writes `display.json`, inotify wakes the
    /// footer, the footer sets it — and a caller pressing the key twice
    /// inside that trip would step from the same number both times. The
    /// display view holds what it last asked for and steps from THAT; the
    /// keyboard's own size keys, when they exist, will pass [`percent`].
    ///
    /// The view had its own copy of this for exactly one commit, which is the
    /// second arithmetic this doc was already warning about.
    #[must_use]
    pub fn stepped(from: u16, by: i32) -> u16 {
        let step = i32::from(super::font::SCALE_STEP.max(1));
        // SATURATING, both times. `by * step` with `i32::MIN` panics in debug
        // and wraps in release — and wrapping is the worse of the two,
        // because it turns "as small as possible" into "as large as
        // possible" before anything has a chance to clamp it.
        let moved = i32::from(from).saturating_add(by.saturating_mul(step));
        allowed(u16::try_from(moved.max(0)).unwrap_or(super::font::SCALE_CEILING))
    }
}

include!(concat!(env!("OUT_DIR"), "/tokens.rs"));

// Every role has a name, and the palettes are indexed by both; the generator
// writes them from one list, and this is the wall behind it.
const _: () = assert!(
    color::NAMES.len() == counts::COLORS,
    "the role names and the role count disagree"
);
const _: () = assert!(
    themes::PAGES.len() == counts::PAGES && theme::Ground::ALL.len() == counts::PAGES,
    "the pages, their count and the grounds disagree"
);

/// The palettes: every theme's colours, read at RUN time.
///
/// # Why they are not compiled in
///
/// They were, as a table in the generated `tokens.rs`, and so a seed's hue —
/// a number in `assets/themes/<id>.nix` — rebuilt this crate and every crate
/// above it, the whole workspace, for bytes that no line of code names. The
/// code names ROLES ([`color`]); which bytes a role is on a given page of a
/// given theme is data, and data is read when the program runs.
///
/// The colour generator (`nix/scripts/gen-theme.py`, `nix/themes.nix`) writes
/// [`FILE`] into its output, and every binary that paints is pointed at it by
/// [`ENV`] — its wrapper sets it, as the session and `nix develop` do. It is
/// read ONCE, the first time a colour or a theme is asked for, into memory that
/// lives as long as the process; asking again costs an atomic load.
///
/// # Why a wrong file is refused, and loudly
///
/// A palette is an array indexed by role, so a file written for another set of
/// roles would put one role's bytes under another's name: every colour on
/// screen wrong, and nothing failing. The file carries a hash of the role
/// names it was written for (FNV-1a over [`color`]'s names, in order), its
/// pages and its version, and a binary that was compiled with other ones
/// REFUSES it, naming the variable — as it refuses a missing variable or an
/// unreadable file. There is no fallback palette: a guessed colour is the
/// failure this guards against.
pub mod palettes {
    use std::ffi::OsString;
    use std::fmt;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    /// The variable naming the directory the palettes are in: the colour
    /// generator's output (`nix/themes.nix`), which the wrappers set.
    pub const ENV: &str = "RONOCTUA_THEMES";

    /// The file in that directory.
    pub const FILE: &str = "palettes.bin";

    /// What the file starts with.
    const MAGIC: [u8; 8] = *b"RONOPAL\0";

    /// The one shape this crate reads; another is refused, never guessed at.
    const VERSION: u16 = 1;

    /// The magic, the version, four counts and the role hash.
    const HEADER: usize = MAGIC.len() + 2 * 5 + 8;

    /// FNV-1a, 64 bits, over `names` joined by `\n` — what a file written
    /// for those roles carries, by the arithmetic of `fnv1a64` in
    /// `nix/scripts/gen-theme.py`. Asked once per process, of
    /// [`super::color`]'s names.
    pub(crate) fn roles_hash(names: &[&str]) -> u64 {
        names
            .join("\n")
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            })
    }

    /// Every theme's palettes, as read.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct Palettes {
        ids: Vec<Box<str>>,
        light: Vec<bool>,
        default: usize,
        /// `[theme][page][role]`, flattened.
        rgba: Vec<[u8; 4]>,
        /// Roles per page.
        roles: usize,
        /// Pages per theme.
        pages: usize,
    }

    impl Palettes {
        /// How many themes there are.
        pub(crate) fn themes(&self) -> usize {
            self.ids.len()
        }

        /// The id of the theme at `theme`.
        pub(crate) fn id(&self, theme: usize) -> &str {
            &self.ids[theme]
        }

        /// Whether the theme at `theme` is a light one.
        pub(crate) fn is_light(&self, theme: usize) -> bool {
            self.light[theme]
        }

        /// The index of the theme a fresh machine shows.
        pub(crate) fn default_theme(&self) -> usize {
            self.default
        }

        /// The bytes of `role` in `palette` — `theme * pages + page`, the
        /// number [`super::theme::palette`] gives.
        pub(crate) fn rgba(&self, palette: usize, role: usize) -> [u8; 4] {
            assert!(
                palette < self.ids.len() * self.pages && role < self.roles,
                "palette {palette}, role {role}: the palettes hold {} palettes of {} roles",
                self.ids.len() * self.pages,
                self.roles
            );
            self.rgba[palette * self.roles + role]
        }

        /// Every colour as the file lays it out, `[theme][page][role]`.
        #[cfg(test)]
        pub(crate) fn flat(&self) -> &[[u8; 4]] {
            &self.rgba
        }
    }

    /// Why the palettes could not be had. Each one says so naming [`ENV`],
    /// which is the one thing a person can fix.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Refusal {
        /// The variable is not set, or is empty.
        Unset,
        /// The file could not be read.
        Unreadable {
            /// The file asked for.
            file: PathBuf,
            /// What the system answered.
            why: String,
        },
        /// The file is not palettes this program can read.
        Invalid {
            /// The file read.
            file: PathBuf,
            /// What is wrong with it, as a clause.
            why: String,
        },
    }

    impl fmt::Display for Refusal {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Unset => write!(
                    f,
                    "{ENV} is not set: the colours are data the build writes beside \
                     the program (`{FILE}` in the themes' generated directory), and \
                     nothing here can draw one without them — run the program \
                     through its wrapper or the session, or inside `nix develop`"
                ),
                Self::Unreadable { file, why } => write!(
                    f,
                    "{ENV} points at {}, and {} cannot be read: {why}",
                    file.parent()
                        .map_or_else(String::new, |d| d.display().to_string()),
                    file.display()
                ),
                Self::Invalid { file, why } => write!(
                    f,
                    "{ENV} points at {}, and its {FILE} {why}",
                    file.parent()
                        .map_or_else(String::new, |d| d.display().to_string())
                ),
            }
        }
    }

    impl std::error::Error for Refusal {}

    static LOADED: OnceLock<Palettes> = OnceLock::new();

    /// The palettes, read the first time anything asks.
    pub(crate) fn get() -> &'static Palettes {
        LOADED.get_or_init(|| {
            read(std::env::var_os(ENV)).unwrap_or_else(|refusal| panic!("{refusal}"))
        })
    }

    /// Reads the palettes NOW, so that a missing or wrong file is said before
    /// anything is drawn — a program's `main` calls it first. Every colour and
    /// every theme asked for reads them anyway; this only chooses the moment.
    ///
    /// # Panics
    ///
    /// When [`ENV`] is not set, its [`FILE`] cannot be read, or it was written
    /// for other colour roles, other pages or another version — with a
    /// sentence that names the variable ([`Refusal`]).
    pub fn load() {
        let _ = get();
    }

    /// The palettes in the directory `dir` names.
    pub(crate) fn read(dir: Option<OsString>) -> Result<Palettes, Refusal> {
        let dir = PathBuf::from(dir.filter(|d| !d.is_empty()).ok_or(Refusal::Unset)?);
        let file = dir.join(FILE);
        let bytes = std::fs::read(&file).map_err(|e| Refusal::Unreadable {
            file: file.clone(),
            why: e.to_string(),
        })?;
        parse(&bytes, &super::color::NAMES, &super::themes::PAGES)
            .map_err(|why| Refusal::Invalid { file, why })
    }

    /// The file's bytes, for a program compiled with `roles` and `pages`.
    /// `Err` is a clause saying what is wrong.
    pub(crate) fn parse(bytes: &[u8], roles: &[&str], pages: &[&str]) -> Result<Palettes, String> {
        let different = "the program and its palettes come from different builds";
        if bytes.len() < HEADER || bytes[..MAGIC.len()] != MAGIC {
            return Err("is not a palettes file the colour generator wrote".to_owned());
        }
        let mut at = MAGIC.len();
        let u16_at = |at: &mut usize| {
            let v = u16::from_le_bytes([bytes[*at], bytes[*at + 1]]);
            *at += 2;
            usize::from(v)
        };
        let version = u16_at(&mut at);
        if version != usize::from(VERSION) {
            return Err(format!(
                "is version {version}, and this program reads version {VERSION}: {different}"
            ));
        }
        let (n_pages, n_themes, n_roles, default) = (
            u16_at(&mut at),
            u16_at(&mut at),
            u16_at(&mut at),
            u16_at(&mut at),
        );
        let mut hash = [0_u8; 8];
        hash.copy_from_slice(&bytes[at..at + 8]);
        at += 8;
        let hash = u64::from_le_bytes(hash);
        if n_roles != roles.len() || hash != roles_hash(roles) {
            return Err(format!(
                "was written for {n_roles} other colour roles (hash {hash:#018x}; this \
                 program's {} roles are {:#018x}): {different}",
                roles.len(),
                roles_hash(roles)
            ));
        }
        // A NAME, `len` bytes after a length byte, read with every bound said.
        let name = |at: &mut usize, what: &str| -> Result<String, String> {
            let len = usize::from(
                *bytes
                    .get(*at)
                    .ok_or_else(|| format!("ends inside its {what}"))?,
            );
            let text = bytes
                .get(*at + 1..*at + 1 + len)
                .ok_or_else(|| format!("ends inside its {what}"))?;
            *at += 1 + len;
            String::from_utf8(text.to_vec()).map_err(|_| format!("has a {what} that is not text"))
        };
        let mut found = Vec::with_capacity(n_pages);
        for _ in 0..n_pages {
            found.push(name(&mut at, "page names")?);
        }
        if found.iter().map(String::as_str).ne(pages.iter().copied()) {
            return Err(format!(
                "is solved on the pages {found:?}, and this program draws on {pages:?}: {different}"
            ));
        }
        if n_themes == 0 {
            return Err("holds no theme".to_owned());
        }
        let mut ids: Vec<Box<str>> = Vec::with_capacity(n_themes);
        let mut light = Vec::with_capacity(n_themes);
        for _ in 0..n_themes {
            let mode = *bytes.get(at).ok_or("ends inside its themes")?;
            at += 1;
            let id = name(&mut at, "theme ids")?;
            let kebab = id.starts_with(|c: char| c.is_ascii_lowercase())
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !kebab {
                return Err(format!("names a theme {id:?}, which is not a theme id"));
            }
            if ids.iter().any(|known| &**known == id.as_str()) {
                return Err(format!("names the theme {id:?} twice"));
            }
            light.push(match mode {
                0 => false,
                1 => true,
                other => return Err(format!("says the theme {id:?} is of mode {other}")),
            });
            ids.push(id.into_boxed_str());
        }
        if default >= n_themes {
            return Err(format!(
                "names theme {default} the default, and it holds {n_themes}"
            ));
        }
        let wanted = n_themes * n_pages * n_roles * 4;
        let data = &bytes[at..];
        if data.len() != wanted {
            return Err(format!(
                "holds {} bytes of colour where {n_themes} themes of {n_pages} pages of \
                 {n_roles} roles are {wanted}",
                data.len()
            ));
        }
        let rgba = data
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();
        Ok(Palettes {
            ids,
            light,
            default,
            rgba,
            roles: n_roles,
            pages: n_pages,
        })
    }

    /// The palettes the tests run against: the real ones, from [`ENV`] in
    /// the tests' environment.
    #[cfg(test)]
    pub(crate) fn load_for_tests() -> &'static Palettes {
        get()
    }
}

// An empty or truncated generation takes COMPILATION down, not a test.
//
// The counts are compile-time constants, so an `assert!` in a `#[test]`
// would be folded by the compiler and would verify nothing at run time — clippy
// warns about exactly that. As a const assertion, the error shows up at build time, which
// is where a broken generator ought to show up.
const _: () = assert!(
    counts::COLORS >= 40,
    "the colour generation came out truncated"
);
const _: () = assert!(
    counts::ICONS >= 40,
    "the icon generation came out truncated"
);
const _: () = assert!(
    counts::SPACES >= 5,
    "the spacing generation came out truncated"
);

/// Geometry derived from the tokens.
///
/// Everything here is a pure function of the font size. Node text measurement,
/// no query to the renderer — and that is what makes the footer's layout cheap
/// enough to fit in one frame (section 5.2).
pub mod metrics {
    use super::font;

    /// The width of one cell, in logical px.
    ///
    /// Exact, not approximate: the font has a single advance per width.
    #[must_use]
    pub fn cell_width(font_size: f32) -> f32 {
        font_size * font::CELL_WIDTH_EM
    }

    /// The height of one line, in logical px.
    #[must_use]
    pub fn line_height(font_size: f32) -> f32 {
        font_size * font::LINE_HEIGHT
    }

    /// The footer's height: **exactly two lines** (section 4.1).
    ///
    /// This is the layer-shell surface's `exclusive zone` value, and it
    /// **does not change when the minibuffer expands** (section 4.3): the expansion
    /// grows upwards over the content, without reflowing what is behind.
    #[must_use]
    pub fn footer_height(font_size: f32) -> f32 {
        line_height(font_size) * 2.0
    }

    /// How many whole cells fit in a given width.
    #[must_use]
    pub fn columns(width_px: f32, font_size: f32) -> u16 {
        let w = cell_width(font_size);
        if w <= 0.0 {
            return 0;
        }
        // `floor`: half a cell is no use to anyone, and rounding up
        // would make the last segment spill off the screen.
        (width_px / w).floor().max(0.0) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_of_undoes_the_srgb_curve() {
        // Without this the footer's background (0x14171C) reaches the screen as
        // ~0x4D grey. The right linear value for 0x14 is ~0.006995, not 0.0784.
        let [r, _, _, a] = linear_of([0x14, 0x17, 0x1C, 0xFF]);
        assert!((r - 0.006_995).abs() < 1e-5, "r linear = {r}");
        // Alpha does NOT go through the curve.
        assert!((a - 1.0).abs() < 1e-6);

        // Black and white are fixed points in both spaces; if these
        // fail, the formula is inverted.
        let black = linear_of([0, 0, 0, 0xFF]);
        let blank = linear_of([255, 255, 255, 0xFF]);
        assert_eq!(black[0], 0.0);
        assert!((blank[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_declared_default_is_a_real_theme_and_every_id_round_trips() {
        // THE DEFAULT IS NOT THE FIRST, and this assertion said it was. The
        // chooser's order and the system's default are two declarations now
        // (`assets/themes/themes.nix`), so what is left to guard is that the
        // palettes' default names a theme that exists.
        assert!(theme::default_one().position() < theme::all().len());
        assert_eq!(
            theme::by_id(theme::default_one().id()),
            Some(theme::default_one())
        );
        for t in theme::all() {
            assert_eq!(theme::by_id(t.id()), Some(t), "{}", t.id());
        }
        assert_eq!(theme::by_id("no-such-theme"), None);
    }

    #[test]
    fn a_desks_page_is_its_desk_and_the_band_and_the_brain_never_move() {
        // ON A DESK'S PAGE THE CHROME IS THE DESK: that is what wearing the
        // page means, and it is the one fact the painters rely on — they
        // clear with the chrome and are on the desk.
        let desks = [
            (theme::Ground::Backdrop, color::SURFACE_BACKDROP),
            (theme::Ground::Near, color::SURFACE_NEAR),
            (theme::Ground::Absolute, color::SURFACE_ABSOLUTE),
        ];
        let palettes = palettes::load_for_tests();
        for t in theme::all() {
            let page = |g: theme::Ground, role: Color| {
                palettes.rgba(
                    t.position() * counts::PAGES + g.position(),
                    usize::from(role.0),
                )
            };
            for (g, desk) in desks {
                assert_eq!(
                    page(g, color::SURFACE_CHROME),
                    page(theme::Ground::Chrome, desk),
                    "{} on {g:?}",
                    t.id()
                );
            }
            // AND WHAT THE OWNER SAID NEVER MOVES, does not: the band, the
            // brain's line, the glance, the outlines and the desks.
            for (role, name) in color::NAMES.iter().enumerate() {
                let fixed = ["notification.", "brain.", "glance.", "border."]
                    .iter()
                    .any(|p| name.starts_with(p))
                    || matches!(
                        *name,
                        "surface.backdrop" | "surface.near" | "surface.absolute"
                    );
                if fixed {
                    let role = Color(u16::try_from(role).expect("a role fits"));
                    for g in theme::Ground::ALL {
                        assert_eq!(
                            page(g, role),
                            page(theme::Ground::Chrome, role),
                            "{name} moved on {g:?} in {}",
                            t.id()
                        );
                    }
                }
            }
        }
        // THE GENERATOR'S PAGES ARE THE ENUM'S, in the enum's order: the
        // palettes are indexed by `Ground::position`, and a generator that
        // wrote its pages in another order would put every desk's palette
        // under another desk's name — the file's header names its pages, and
        // the loader refuses others than these. Said against the words
        // themselves — `Ground::id` reads `themes::PAGES`, so comparing the
        // two would compare one expression with itself.
        assert_eq!(themes::PAGES, ["chrome", "backdrop", "near", "absolute"]);
        assert_eq!(theme::Ground::ALL.len(), counts::PAGES);
        assert_eq!(
            theme::Ground::ALL.map(theme::Ground::position),
            [0, 1, 2, 3],
            "the enum's order is not the table's"
        );
        assert_eq!(
            theme::Ground::ALL.map(theme::Ground::id),
            ["chrome", "backdrop", "near", "absolute"]
        );
    }

    #[test]
    fn a_colour_asked_in_a_palette_is_that_palettes_bytes() {
        // THE WORKER'S READ (`Color::to_rgba8_in`): the bytes of the palette
        // it names, and never the one the process happens to wear — every
        // theme on every page, measured against the file's own layout
        // (`[theme][page][role]`, four bytes each) so no test has to move the
        // process-wide palette (`set` belongs to one test).
        let palettes = palettes::load_for_tests();
        for t in theme::all() {
            for g in theme::Ground::ALL {
                let palette = t.position() * counts::PAGES + g.position();
                for (role, name) in color::NAMES.iter().enumerate() {
                    let c = Color(u16::try_from(role).expect("a role fits"));
                    assert_eq!(
                        c.to_rgba8_in(palette),
                        palettes.flat()[(palette * counts::COLORS) + role],
                        "{name} in {} on {g:?}",
                        t.id()
                    );
                }
            }
        }
    }

    #[test]
    fn every_theme_says_which_way_it_climbs_and_its_absolute_desk_agrees() {
        // THE MODE AND THE COLOURS COME FROM ONE GENERATOR RUN, and this is
        // what proves they travelled together: the absolute desk is the
        // extreme a theme climbs AWAY from, so it is black exactly when the
        // theme is dark and white exactly when it is light. A mode read
        // wrong would name a white desk "black" on the chooser's row.
        for t in theme::all() {
            let bytes = color::SURFACE_ABSOLUTE.to_rgba8_in(t.position() * counts::PAGES);
            let expected = if t.is_light() {
                [0xFF, 0xFF, 0xFF, 0xFF]
            } else {
                [0x00, 0x00, 0x00, 0xFF]
            };
            assert_eq!(bytes, expected, "{}", t.id());
            assert_eq!(theme::is_light_at(t.position()), t.is_light(), "{}", t.id());
        }
    }

    #[test]
    fn a_colour_is_a_role_and_the_theme_says_which_bytes() {
        // THE ONE TEST THAT CALLS `set`: the theme is process-wide, and the
        // workspace's tests run in parallel. It restores what it found.
        let before = theme::current();
        for t in theme::all() {
            theme::set(t);
            assert_eq!(theme::current(), t);
            // Every role, in every theme, resolves to that theme's row.
            for (role, name) in color::NAMES.iter().enumerate() {
                let handle = Color::at(u16::try_from(role).unwrap());
                assert_eq!(handle.name(), *name);
                assert_eq!(
                    handle.to_rgba8(),
                    palettes::load_for_tests().rgba(t.position() * counts::PAGES, role),
                    "{name} in {}",
                    t.id()
                );
            }
        }
        assert!(
            !theme::set(theme::current()),
            "setting the same theme is not a change"
        );
        // AND THE MODE FOLLOWS THE THEME THE PROCESS IS IN.
        for t in theme::all() {
            theme::set(t);
            assert_eq!(theme::is_light(), t.is_light(), "{}", t.id());
        }
        // AND THE GROUND PICKS THE PAGE, keeping the theme — and a theme set
        // after it keeps the ground: the two are one palette, moved one half
        // at a time.
        let t = theme::current();
        for g in theme::Ground::ALL {
            theme::set_ground(g);
            assert_eq!((theme::current(), theme::ground()), (t, g));
            let role = color::SURFACE_CHROME;
            assert_eq!(
                role.to_rgba8(),
                palettes::load_for_tests().rgba(
                    t.position() * counts::PAGES + g.position(),
                    usize::from(role.0)
                ),
                "{g:?}"
            );
        }
        assert!(
            !theme::set_ground(theme::Ground::Absolute),
            "the same ground is not a change"
        );
        theme::set(before);
        assert_eq!(
            theme::ground(),
            theme::Ground::Absolute,
            "a theme set kept the ground"
        );
        // AND THE PALETTE NUMBER MOVES WITH EITHER HALF, which is what a
        // cache keyed by it needs.
        let on_absolute = theme::palette();
        theme::set_ground(theme::Ground::Chrome);
        assert_ne!(theme::palette(), on_absolute);
        // Role equality does not care which theme is in.
        assert_eq!(color::TEXT_DIM, color::TEXT_DIM);
        assert_ne!(color::TEXT_DIM, color::TEXT_PRIMARY);
        assert_eq!(format!("{:?}", color::TEXT_DIM), "color.text.dim");
    }

    #[test]
    fn the_cell_is_six_tenths_of_an_em() {
        // Measured in the inventory of the 135 faces. If this changes, the whole
        // footer layout has to be reviewed — it is not a constant of taste.
        assert!((font::CELL_WIDTH_EM - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn the_footer_has_exactly_two_lines() {
        let s = font::SIZE_BASE;
        assert!((metrics::footer_height(s) - metrics::line_height(s) * 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn columns_never_overflow_the_width() {
        let s = font::SIZE_BASE;
        for width in [1.0_f32, 13.0, 100.0, 800.0, 3840.0] {
            let n = metrics::columns(width, s);
            let used = f32::from(n) * metrics::cell_width(s);
            assert!(
                used <= width + f32::EPSILON,
                "{n} columns take {used} px in a width of {width}"
            );
        }
    }

    #[test]
    fn the_systems_font_is_the_patched_one() {
        // Section 6.2 says TTF-nerdfonts is the default everywhere. The
        // name has to be that of the CORRECTED family (the 27 SemiExtended faces
        // fell into phantom families before the correction).
        assert!(font::FAMILY_MONO.contains("Nerd Font Mono"));
        assert_eq!(font::FAMILY_MONO, font::FAMILY_UI);
    }
}

#[cfg(test)]
mod palettes_file {
    //! THE FILE'S SHAPE, against bytes written here the way
    //! `nix/scripts/gen-theme.py`'s `palettes_bin` writes them — and the
    //! refusals, each a sentence naming the variable. Three roles and two
    //! pages, so a test reads every byte it wrote.

    use super::palettes::{self, ENV, Refusal};

    const ROLES: [&str; 3] = ["a.one", "a.two", "b"];
    const PAGES: [&str; 2] = ["chrome", "desk"];
    const THEMES: [(&str, u8); 2] = [("night", 0), ("paper", 1)];

    /// The header and the data, every byte its own coordinates:
    /// `[theme, page, role, 0xFF]`. `version`, `hash` and the lists are the
    /// knobs a test turns.
    fn file(
        version: u16,
        pages: &[&str],
        themes: &[(&str, u8)],
        default: u16,
        roles: &[&str],
        hash: u64,
    ) -> Vec<u8> {
        let mut out = b"RONOPAL\0".to_vec();
        for n in [
            version,
            u16::try_from(pages.len()).unwrap(),
            u16::try_from(themes.len()).unwrap(),
            u16::try_from(roles.len()).unwrap(),
            default,
        ] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(&hash.to_le_bytes());
        for page in pages {
            out.push(u8::try_from(page.len()).unwrap());
            out.extend_from_slice(page.as_bytes());
        }
        for (id, mode) in themes {
            out.push(*mode);
            out.push(u8::try_from(id.len()).unwrap());
            out.extend_from_slice(id.as_bytes());
        }
        for t in 0..themes.len() {
            for p in 0..pages.len() {
                for r in 0..roles.len() {
                    out.extend_from_slice(&[t as u8, p as u8, r as u8, 0xFF]);
                }
            }
        }
        out
    }

    fn good() -> Vec<u8> {
        file(1, &PAGES, &THEMES, 1, &ROLES, palettes::roles_hash(&ROLES))
    }

    fn refusal(bytes: &[u8]) -> String {
        palettes::parse(bytes, &ROLES, &PAGES).expect_err("a refused file")
    }

    /// FAILS IF: a file written for this program's roles does not read back
    /// every byte at its `[theme][page][role]`, or the header's ids, modes and
    /// default are not the ones written.
    #[test]
    fn a_file_for_these_roles_reads_back_every_byte() {
        let p = palettes::parse(&good(), &ROLES, &PAGES).expect("the palettes");
        assert_eq!(p.themes(), 2);
        assert_eq!((p.id(0), p.id(1)), ("night", "paper"));
        assert_eq!((p.is_light(0), p.is_light(1)), (false, true));
        assert_eq!(p.default_theme(), 1);
        for t in 0..2 {
            for g in 0..2 {
                for r in 0..3 {
                    assert_eq!(p.rgba(t * 2 + g, r), [t as u8, g as u8, r as u8, 0xFF]);
                }
            }
        }
    }

    /// FAILS IF: a file written for other roles — another name, another
    /// order, one more — is read through this program's index instead of
    /// being refused as another build's.
    #[test]
    fn a_file_for_other_roles_is_another_builds() {
        for roles in [
            &["a.one", "a.two", "c"][..],
            &["a.two", "a.one", "b"][..],
            &["a.one", "a.two", "b", "b.more"][..],
        ] {
            let why = refusal(&file(
                1,
                &PAGES,
                &THEMES,
                0,
                roles,
                palettes::roles_hash(roles),
            ));
            assert!(
                why.contains("other colour roles") && why.contains("different builds"),
                "{why}"
            );
        }
        // AND THE COUNT ALONE IS NOT ENOUGH: three roles under a hash of
        // three others.
        let why = refusal(&file(
            1,
            &PAGES,
            &THEMES,
            0,
            &ROLES,
            palettes::roles_hash(&["x", "y", "z"]),
        ));
        assert!(why.contains("other colour roles"), "{why}");
    }

    /// FAILS IF: a file of another version, other pages or another shape is
    /// read instead of refused with what is wrong.
    #[test]
    fn every_other_shape_is_refused_with_what_is_wrong() {
        let hash = palettes::roles_hash(&ROLES);
        let cases: [(Vec<u8>, &str); 9] = [
            (b"PNG\r\n".to_vec(), "is not a palettes file"),
            (good()[..20].to_vec(), "is not a palettes file"),
            (file(2, &PAGES, &THEMES, 0, &ROLES, hash), "is version 2"),
            (
                file(1, &["desk", "chrome"], &THEMES, 0, &ROLES, hash),
                "is solved on the pages",
            ),
            (file(1, &PAGES, &[], 0, &ROLES, hash), "holds no theme"),
            (
                file(1, &PAGES, &THEMES, 2, &ROLES, hash),
                "names theme 2 the default",
            ),
            (
                file(1, &PAGES, &[("night", 0), ("night", 1)], 0, &ROLES, hash),
                "twice",
            ),
            (
                file(1, &PAGES, &[("Night", 0)], 0, &ROLES, hash),
                "not a theme id",
            ),
            (
                file(1, &PAGES, &[("night", 2)], 0, &ROLES, hash),
                "of mode 2",
            ),
        ];
        for (bytes, said) in cases {
            let why = refusal(&bytes);
            assert!(why.contains(said), "wanted {said:?}, got {why:?}");
        }
        // A BYTE TOO FEW OR TOO MANY is a file cut or grown, never a palette.
        let mut short = good();
        short.pop();
        assert!(refusal(&short).contains("bytes of colour"));
        let mut long = good();
        long.push(0);
        assert!(refusal(&long).contains("bytes of colour"));
        // AND A FILE CUT INSIDE ITS NAMES says so, never panics.
        let mut cut = good();
        cut.truncate(26 + 3);
        assert!(
            refusal(&cut).contains("ends inside its page names"),
            "{}",
            refusal(&cut)
        );
    }

    /// FAILS IF: the role hash is not FNV-1a, 64 bits, over the names joined
    /// by a newline — the arithmetic `gen-theme.py` writes the file with.
    #[test]
    fn the_role_hash_is_fnv1a_over_the_names_joined() {
        // The published vectors.
        assert_eq!(palettes::roles_hash(&[""]), 0xcbf2_9ce4_8422_2325);
        assert_eq!(palettes::roles_hash(&["a"]), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(palettes::roles_hash(&["foobar"]), 0x8594_4171_f739_67e8);
        // The separator is part of it: two names are not their concatenation.
        assert_ne!(
            palettes::roles_hash(&["foo", "bar"]),
            palettes::roles_hash(&["foobar"])
        );
        assert_eq!(palettes::roles_hash(&[]), palettes::roles_hash(&[""]));
    }

    /// FAILS IF: a refusal does not name the variable — the one thing a
    /// person can set — or a missing or empty one is read as some directory.
    #[test]
    fn every_refusal_names_the_variable() {
        assert_eq!(palettes::read(None), Err(Refusal::Unset));
        assert_eq!(palettes::read(Some("".into())), Err(Refusal::Unset));
        let nowhere = std::env::temp_dir().join(format!(
            "ronoctua-tokens-no-palettes-{}",
            std::process::id()
        ));
        let unreadable =
            palettes::read(Some(nowhere.clone().into_os_string())).expect_err("no file there");
        assert!(
            matches!(unreadable, Refusal::Unreadable { ref file, .. } if *file == nowhere.join(palettes::FILE))
        );
        let invalid = Refusal::Invalid {
            file: nowhere.join(palettes::FILE),
            why: refusal(&file(2, &PAGES, &THEMES, 0, &ROLES, 0)),
        };
        for r in [Refusal::Unset, unreadable, invalid] {
            let said = r.to_string();
            assert!(said.starts_with(ENV), "{said}");
        }
        assert!(Refusal::Unset.to_string().contains(palettes::FILE));
    }

    /// FAILS IF: the palettes the tests run against are not this program's
    /// own — read again from the variable, they are the same.
    #[test]
    fn the_palettes_in_the_environment_are_this_programs() {
        let loaded = palettes::load_for_tests();
        assert!(loaded.themes() >= 1);
        assert_eq!(
            palettes::read(std::env::var_os(ENV)).as_ref(),
            Ok(loaded),
            "the environment's palettes read twice disagree"
        );
    }
}

#[cfg(test)]
mod text_tests {
    use super::text;

    // ONE TEST TOUCHES `text::set`, and it is this one — the same rule
    // `theme::set` carries. The size is process-wide and the tests run in
    // parallel, so a second one calling it would change the size under this.
    #[test]
    fn the_size_moves_in_steps_a_key_can_reach_and_returns_to_a_hundred() {
        assert_eq!(text::percent(), text::NORMAL);
        assert!((text::size() - font::SIZE_BASE).abs() < f32::EPSILON);

        // A value from a hand-edited file lands where the arrows can leave.
        assert_eq!(text::allowed(103), 105);
        assert_eq!(text::allowed(102), 100);
        assert_eq!(text::allowed(0), font::SCALE_FLOOR);
        assert_eq!(text::allowed(u16::MAX), font::SCALE_CEILING);
        assert_eq!(text::allowed(font::SCALE_FLOOR), font::SCALE_FLOOR);
        assert_eq!(text::allowed(font::SCALE_CEILING), font::SCALE_CEILING);

        // Both ends are reachable by pressing one key, and so is the middle.
        let mut seen = alloc_free_walk(1);
        assert!(seen.contains(&font::SCALE_CEILING), "cannot reach the top");
        assert!(
            seen.contains(&text::NORMAL),
            "cannot reach a hundred going up"
        );
        seen = alloc_free_walk(-1);
        assert!(seen.contains(&font::SCALE_FLOOR), "cannot reach the bottom");
        assert!(
            seen.contains(&text::NORMAL),
            "cannot reach a hundred coming down"
        );

        // And the size really does follow.
        text::set(200);
        assert_eq!(text::percent(), 200);
        assert!((text::size() - font::SIZE_BASE * 2.0).abs() < 0.001);
        // A cell and a line follow with it, which is what makes the layout
        // need no other change.
        assert!(
            (super::metrics::cell_width(text::size())
                - super::metrics::cell_width(font::SIZE_BASE) * 2.0)
                .abs()
                < 0.001
        );
        text::set(text::NORMAL);
        assert_eq!(text::percent(), text::NORMAL);
    }

    /// Walks the range one step at a time from where it is now, collecting
    /// every value it lands on. It restores the size before returning.
    fn alloc_free_walk(direction: i32) -> Vec<u16> {
        let began = text::percent();
        let mut seen = Vec::new();
        let mut at = if direction > 0 {
            font::SCALE_FLOOR
        } else {
            font::SCALE_CEILING
        };
        text::set(at);
        for _ in 0..500 {
            seen.push(at);
            let next = text::stepped(text::percent(), direction);
            if next == at {
                break;
            }
            text::set(next);
            at = next;
        }
        text::set(began);
        seen
    }

    use super::font;
}
