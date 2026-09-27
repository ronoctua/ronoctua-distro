//! The application launcher — `LEADER a`.
//!
//! # Why it is not a start menu
//!
//! Invariant 14 forbids a dock, a start menu and a floating popup, and this is
//! none of the three: it is the **fifth** view of the expanded region, alongside
//! which-key, `M-x`, the notification history, the clipboard and the
//! manual. Same mechanism, same surface, same gesture — one types and the
//! list narrows. What it adds is the list's source, not chrome.
//!
//! # The list NEVER waits for the icon
//!
//! It is the requirement that decides this module's whole structure. The disk
//! scan happens **once, at startup, on a thread** ([`scan`]), and the
//! catalogue arrives ready; opening with `LEADER a` reads no file at all. Each
//! row's glyph is resolved by table, in memory ([`glyph_of`]). The
//! app's pixmap — when there is one — comes afterwards, on another thread, and while
//! it has not arrived the row shows the glyph. Nothing here blocks on I/O.
//!
//! # About invariant 10
//!
//! *"The superpackage is only a pixmap fallback for foreign apps; no
//! design system component consults it."* What consults it here is the **host**,
//! for a **foreign** app's icon — exactly the case
//! [`ronoctua_notify::icon::Chain`] was written to serve. Node WASM buffer
//! reaches it, and none could: the WIT `world` has no
//! filesystem.

use ronoctua_tokens as tokens;
// The SAME `calloop` as the event loop's, and not a direct dependency: two
// versions of the crate would give two channel types with the same name, and the error
// would show up as a type mismatch without saying they are two crates.
use smithay_client_toolkit::reexports::calloop;
use std::sync::Arc;

/// The catalogue, shared between the event loop and the icon thread.
///
/// An `Arc` and not a copy: the thread needs the icon names of the visible rows,
/// and cloning four hundred entries per request would mean paying for the scan again
/// on every scroll.
pub type Catalogue = Arc<Vec<ronoctua_apps::DesktopEntry>>;

/// What the launcher asks of the host after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nothing changed; do not redraw.
    Nothing,
    /// The list or the text changed.
    Changed,
    /// It left without opening anything.
    Closed,
    /// Open this application.
    ///
    /// The entry comes BY VALUE because the launcher closes along with it: returning
    /// an index would make the host consult a list it has just discarded.
    Open(Box<ronoctua_apps::DesktopEntry>),
    /// Put it on the file manager's sidebar, and STAY OPEN.
    ///
    /// Pinning is not opening: somebody putting three applications there means
    /// to put three, and a launcher that closed after each would make them type
    /// the sequence again twice.
    ///
    /// By value for the same reason as [`Action::Open`] — but what is stored is
    /// the ID, which survives a rebuild, and never the command, whose store
    /// path is a build that gets collected.
    Pin(Box<ronoctua_apps::DesktopEntry>),
    /// Look for what was typed among the programs one can INSTALL — the
    /// launcher's last row, and its only one when nothing installed matches.
    ///
    /// The query as typed, trimmed. The host closes the launcher and opens the
    /// software view's results on it: a person who typed `gimp` and found
    /// nothing is a person about to go and get it, and the launcher is where
    /// they already are.
    SearchToInstall(String),
    /// Show what this program is and where it came from — `C-i`.
    ///
    /// An index into the CATALOGUE (never a row of the list, which the next
    /// letter reorders) — the footer's own `catalogue`, which is the same `Arc`
    /// the finder was opened over and swapped with. The host closes the
    /// launcher and opens the program's sheet.
    Details(usize),
    /// Remove this program — `C-d` —, AFTER a question that opens on no.
    ///
    /// The same index as [`Action::Details`], and the same sheet, with the
    /// question already up. The launcher does not know where a program came
    /// from, so it never refuses here: a program the flake installed is the
    /// sheet's to refuse, with the sentence that says why.
    Remove(usize),
}

/// The launcher's own keys, and the command each one runs.
///
/// # Why they carry `Ctrl`
///
/// In a list of NAMES every bare letter is text — it goes into the query — so
/// anything that is a command has to be something a name never is. It is the
/// file manager's reason for its `C-` commands, and the reason a letter here
/// would be a letter nobody could type.
///
/// # Why the table lives in the view
///
/// `leader::OF_LAUNCHER_VIEW` held it while there was one key. A view's keys
/// live beside the view that answers them — the clipboard's table left
/// `leader.rs` the same way —, and [`Finder::key`] and the registry read ONE
/// table, so a key added here is a key `leader::reachable` counts.
///
/// `C-i` is `details` because it is what the same letter means in the file
/// manager (the facts pane), and `C-d` removes because it is `d` for delete,
/// behind the modifier every command in this list carries.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("C-s", "apps-pin"),
    ("C-i", "apps-details"),
    ("C-d", "apps-remove"),
];

/// The command id of the last row, "search to install".
///
/// It has no key: the row is reached like any other row, by the arrows or the
/// pointer, and `RET` or a click takes it ([`Finder::choose`]). The id exists
/// so `LEADER :` names it and the registry can list it.
pub const SEARCH_TO_INSTALL: &str = "apps-search-to-install";

/// The text of the last row: "Search “gimp” to install".
#[must_use]
pub fn search_row_text(texts: &ronoctua_i18n::Texts, query: &str) -> String {
    texts.t_with(
        ronoctua_i18n::key::APPS_SEARCH_TO_INSTALL,
        &[("query", query)],
    )
}

/// The launcher's state while it is open.
#[derive(Debug)]
pub struct Finder {
    /// Everything that exists, in the scan's alphabetical order.
    catalogue: Catalogue,
    /// What was typed, as it was typed.
    query: String,
    /// The catalogue indices that match, in relevance order.
    result: Vec<usize>,
    /// Which row is focused — a result, or `result.len()` when it is the
    /// "search to install" row after them (see [`Finder::rows`]).
    cursor: usize,
}

impl Finder {
    /// Opens over an already-scanned catalogue.
    #[must_use]
    pub fn fresh(catalogue: Catalogue) -> Self {
        let mut l = Self {
            catalogue,
            query: String::new(),
            result: Vec::new(),
            cursor: 0,
        };
        l.filter_by();
        l
    }

    /// Swaps the catalogue without losing what was typed.
    ///
    /// The rescan arrives on a thread and may arrive with the launcher open and
    /// half a word typed. Discarding the query there would be erasing what
    /// the person is writing because of an event they did not cause.
    pub fn recatalogue(&mut self, catalogue: Catalogue) {
        self.catalogue = catalogue;
        self.filter_by();
    }

    /// What was typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The query as the filter used it — folded and in lower case.
    ///
    /// It exists so the drawing can recompute WHERE it matched and light those letters,
    /// for the same reason as `M-x`: keeping each item's positions would cost
    /// one allocation per row on every keystroke, and they only matter to the ones
    /// on screen.
    #[must_use]
    pub fn normalized_query(&self) -> String {
        ronoctua_apps::fold::fold(&self.query)
    }

    /// The catalogue indices on display, in screen order.
    ///
    /// The APPLICATIONS only: the "search to install" row that may follow them
    /// is not an entry — see [`Finder::rows`] and [`Finder::offers_search`].
    #[must_use]
    pub fn result(&self) -> &[usize] {
        &self.result
    }

    /// What the last row offers to look for, when there is a last row.
    ///
    /// `Some(query)` whenever something is typed: the row is always there
    /// after the matches, and alone when nothing matches, so "not installed"
    /// is never a dead end. With nothing typed there is nothing to look for,
    /// and no row.
    ///
    /// Trimmed, because a space typed after a word is not part of the name
    /// anybody is looking for — and a query of spaces is no query.
    #[must_use]
    pub fn offers_search(&self) -> Option<&str> {
        Some(self.query.trim()).filter(|q| !q.is_empty())
    }

    /// How many rows the list draws: every result, then the "search to
    /// install" row when [`Finder::offers_search`] has one.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.result.len() + usize::from(self.offers_search().is_some())
    }

    /// Is row `n` the "search to install" one?
    #[must_use]
    pub fn is_search_row(&self, n: usize) -> bool {
        n == self.result.len() && self.offers_search().is_some()
    }

    /// Which row is focused.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The entry on row `n` of the visible list. `None` on the "search to
    /// install" row, which is not an entry.
    #[must_use]
    pub fn at(&self, n: usize) -> Option<&ronoctua_apps::DesktopEntry> {
        self.catalogue.get(*self.result.get(n)?)
    }

    /// The focused entry.
    #[must_use]
    pub fn chosen(&self) -> Option<&ronoctua_apps::DesktopEntry> {
        self.at(self.cursor)
    }

    /// The focused entry's index in the CATALOGUE.
    fn chosen_index(&self) -> Option<usize> {
        self.result.get(self.cursor).copied()
    }

    /// How many applications the whole catalogue has — for the log and the echo area.
    #[must_use]
    pub fn total(&self) -> usize {
        self.catalogue.len()
    }

    /// Recomputes the result.
    ///
    /// Two keys per item, **in this order**: the folded name and the desktop
    /// id. The order is what makes the name beat the id when both match —
    /// see `minibuffer::rank`.
    ///
    /// The DESCRIPTION is left out on purpose. `Comment` is prose: a subsequence
    /// search over prose matches half the list, and the highlight would have
    /// nowhere to light — the row would enter the result with nothing on screen saying
    /// why.
    fn filter_by(&mut self) {
        let q = self.normalized_query();
        let keys: Vec<[&str; 2]> = self
            .catalogue
            .iter()
            .map(|e| [e.key.as_str(), e.id.as_str()])
            .collect();
        let items: Vec<&[&str]> = keys.iter().map(<[&str; 2]>::as_slice).collect();
        // Asked of the OLD list: was the cursor past the applications, on the
        // search row? Then it stays on the search row — whoever walked down to
        // it and goes on typing is refining what to look for.
        let was_searching = self.cursor >= self.result.len();
        self.result = crate::minibuffer::rank(&q, &items);
        self.cursor = if was_searching && self.offers_search().is_some() {
            self.result.len()
        } else {
            // An application otherwise, and the LAST one when the list got
            // shorter under the cursor: narrowing the list must not carry the
            // cursor onto the search row by itself, where `RET` would do
            // something else than it did a letter ago. With no application at
            // all this is row zero — which is the search row, alone.
            self.cursor.min(self.result.len().saturating_sub(1))
        };
    }

    /// Picks row `n` and confirms, as a click does.
    ///
    /// It goes through the SAME `Enter` as the keyboard for `M-x`'s reason: nothing may be
    /// reachable by mouse only, and the same row must not have two
    /// behaviours depending on how one got to it.
    pub fn choose(&mut self, n: usize) -> Action {
        if n >= self.rows() {
            return Action::Nothing;
        }
        self.cursor = n;
        self.key(ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Enter))
    }

    /// Applies a key.
    ///
    /// The interpretation comes from `minibuffer::gesture_of`, and not from a `match` here:
    /// `Esc`, `C-g`, `C-n`/`C-p`, the arrows and `Backspace` have to do the
    /// same thing in both launchers. It is the only part muscle
    /// memory notices.
    pub fn key(&mut self, t: ronoctua_keymap::Key) -> Action {
        use crate::minibuffer::Gesture;
        // THE VIEW'S OWN KEYS COME FIRST, before `gesture_of` turns every bare
        // letter into a letter of the query — see `OF_THE_VIEW` for why every
        // one of them carries `Ctrl`.
        //
        // `C-s` for sidebar. In the manager's menu the same idea is a bare `s`,
        // because there the letter is not query — one idea, two notations, and
        // the alternative was `C-s` in both, which costs the menu its one-letter
        // key for a gesture nobody reaches from there anyway.
        if let Some((_, id)) = OF_THE_VIEW
            .iter()
            .find(|(notation, _)| ronoctua_keymap::Key::parse(notation).is_ok_and(|k| k == t))
        {
            return self.command(id);
        }
        match crate::minibuffer::gesture_of(t) {
            Gesture::Cancel => Action::Closed,
            Gesture::Down => self.move_by(1),
            Gesture::Up => self.move_by(-1),
            Gesture::Nothing => Action::Nothing,
            Gesture::Delete => {
                if self.query.pop().is_none() {
                    return Action::Nothing;
                }
                self.filter_by();
                Action::Changed
            }
            Gesture::Insert(c) => {
                self.query.push(c);
                self.filter_by();
                Action::Changed
            }
            // CONFIRMING WITH NO RESULT DOES NOT CLOSE, as in `M-x`: closing would
            // erase what was typed, and whoever got a letter wrong would have to
            // start over. With something typed there is always a row, though —
            // the search one — and confirming there is looking to install.
            Gesture::Confirm => match self.chosen() {
                Some(e) => Action::Open(Box::new(e.clone())),
                None => self.command(SEARCH_TO_INSTALL),
            },
        }
    }

    /// What one of the launcher's commands asks for, on the focused row.
    ///
    /// The ONE place the ids turn into actions: the keys of [`OF_THE_VIEW`]
    /// come through here, and so does the same id run by name or by a click
    /// on a hint, so the three ways in cannot come to mean different things.
    ///
    /// On the "search to install" row the three that act on a program do
    /// nothing — there is no program under the cursor — and away from it
    /// [`SEARCH_TO_INSTALL`] still takes what was typed: the row is always
    /// there when there is a query, and running it by name should not depend
    /// on where the cursor happens to be.
    pub fn command(&self, id: &str) -> Action {
        match id {
            "apps-pin" => self
                .chosen()
                .map_or(Action::Nothing, |e| Action::Pin(Box::new(e.clone()))),
            "apps-details" => self.chosen_index().map_or(Action::Nothing, Action::Details),
            "apps-remove" => self.chosen_index().map_or(Action::Nothing, Action::Remove),
            SEARCH_TO_INSTALL => self
                .offers_search()
                .map_or(Action::Nothing, |q| Action::SearchToInstall(q.to_owned())),
            _ => Action::Nothing,
        }
    }

    /// Moves the cursor, without leaving the list.
    fn move_by(&mut self, d: i32) -> Action {
        let fresh = self.cursor as i32 + d;
        if fresh < 0 || fresh >= self.rows() as i32 {
            return Action::Nothing;
        }
        self.cursor = fresh as usize;
        Action::Changed
    }
}

/// An application's glyph — the steps that do NOT touch the disk.
///
/// Three attempts, and never "nothing":
///
/// 1. the `app id -> glyph` table, by the desktop id **and** by the `Icon=`;
/// 2. the first `Categories=` the category table knows;
/// 3. the generic glyph.
///
/// # Why the `Icon=` also consults the table
///
/// Because both forms appear, and neither of them is the "right" one: Nautilus
/// has id `org.gnome.Nautilus` and `Icon=org.gnome.Nautilus`; the Firefox of
/// many distributions has id `firefox` and `Icon=firefox`; and there are those with a vendor
/// id and a short `Icon=`. Consulting both costs two lookups in a
/// thirty-entry table and covers all three cases.
///
/// # Why the CATEGORY comes before the generic
///
/// Because the generic is indistinguishable from "I did not resolve it": a whole list of the
/// same little square looks deliberate, and it is precisely the symptom of the table
/// not having reached the Rust. The category gets it wrong rarely and says something — a
/// terminal that looks like a terminal, a game that looks like a game.
#[must_use]
pub fn glyph_of(e: &ronoctua_apps::DesktopEntry) -> char {
    if let Some(g) = in_table(tokens::tray::TABLE, &e.id) {
        return g;
    }
    if !e.icon.is_empty()
        && let Some(g) = in_table(tokens::tray::TABLE, &e.icon)
    {
        return g;
    }
    // IN THE TABLE'S ORDER, and not in the order of the file's categories: a
    // browser declares `Network;WebBrowser`, and both match. What decides is
    // the table, where `WebBrowser` comes first — see `category-glyphs.nix`.
    for (category, glyph) in tokens::category::TABLE {
        if e.categories.iter().any(|c| c == category) {
            return *glyph;
        }
    }
    tokens::icon::GENERIC
}

/// A table lookup, exact and then by the last dotted component.
fn in_table(table: &[(&str, char)], key: &str) -> Option<char> {
    if let Some((_, g)) = table.iter().find(|(k, _)| *k == key) {
        return Some(*g);
    }
    let short = key.rsplit('.').next()?;
    if short == key {
        return None;
    }
    table.iter().find(|(k, _)| *k == short).map(|(_, g)| *g)
}

/// Scans the applications ON A THREAD and returns the catalogue through the channel.
///
/// # Why at startup, and not on opening
///
/// Measured on this machine: **128 entries in 68 ms**. A frame's budget is
/// 16 ms, and the scan is hundreds of `open()` calls — doing it on pressing
/// `LEADER a` would give a four-frame freeze every time, and the gesture of
/// opening the launcher is precisely the one that has to be instant.
///
/// The thread is named because an anonymous thread parked in `read` is what
/// nobody can identify in a `perf`.
pub fn scan(language: &str, sender: calloop::channel::Sender<Catalogue>) {
    // THE LANGUAGE GOES TO THE THREAD, and is not read in there: `Texts` lives in the
    // `Footer`, and the desktop entry's language is the same as the interface's — two
    // reads of the environment could diverge if someone changed the variable.
    let language = language.replace('-', "_");
    let born = std::thread::Builder::new()
        .name("ronoctua-apps".to_owned())
        .spawn(move || {
            let start = std::time::Instant::now();
            let list = ronoctua_apps::scan(&language);
            tracing::info!(
                apps = list.len(),
                ms = start.elapsed().as_millis(),
                "application catalogue scanned"
            );
            // A closed channel = the footer left. Nothing to deliver.
            let _ = sender.send(Arc::new(list));
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not create the scan thread");
    }
}

/// How a launcher entry is started — the program, its words, and the
/// directory it starts in — inside this binary's terminal when the entry
/// says it needs one. `None` when this binary cannot name itself.
///
/// ONE ANSWER FOR EVERY DOOR: the footer's launcher, a notification's "open
/// it" and the windows that open what an install made all start an entry this
/// way, so a program that needs a terminal gets one whichever door opened it.
#[must_use]
pub fn argv_of(e: &ronoctua_apps::DesktopEntry) -> Option<(String, Vec<String>, Option<String>)> {
    let (program, args) = &e.command;
    if !e.terminal {
        return Some((program.clone(), args.clone(), e.working_directory.clone()));
    }
    let me = std::env::current_exe().ok()?;
    let mut inside = vec!["terminal".to_owned(), program.clone()];
    inside.extend_from_slice(args);
    Some((me.to_string_lossy().into_owned(), inside, None))
}

/// How a window's launch by desktop id ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// The entry was found and started — or failed to start, which the
    /// verdict says.
    Started(crate::foreign::Launch),
    /// No entry of the launcher has that id: installed a moment ago and not
    /// in the catalogue yet, or gone.
    NotInLauncher,
}

/// Starts the launcher's entry `desktop_id` FROM A WINDOW, off the loop: the
/// catalogue is read on a thread (the sweep the footer keeps at hand costs
/// tens of milliseconds, which a window's loop does not spend), the entry
/// found by the rule the software view names them by
/// (`software::same_desktop`), and started as the footer starts it
/// ([`argv_of`], `foreign::launch_now`). `done` hears how, on that thread.
///
/// The window says what happened on its own line — it used to hand the id
/// to the footer, whose refusal then appeared on the FOOTER's line, and the
/// window the person was using said nothing.
pub fn launch_entry(desktop_id: &str, language: &str, done: impl FnOnce(Opened) + Send + 'static) {
    let desktop_id = desktop_id.to_owned();
    let language = language.replace('-', "_");
    let born = std::thread::Builder::new()
        .name("launch-entry".to_owned())
        .spawn(move || {
            let entry = ronoctua_apps::scan(&language)
                .into_iter()
                .find(|e| crate::software::same_desktop(&e.id, &desktop_id));
            let Some(entry) = entry else {
                done(Opened::NotInLauncher);
                return;
            };
            let opened = match argv_of(&entry) {
                Some((program, args, cwd)) => {
                    tracing::info!(app = %entry.id, program, "opening the application");
                    crate::foreign::launch_now(&program, &args, cwd.as_deref())
                }
                None => crate::foreign::Launch::Failed {
                    program: entry.id.clone(),
                    error: "this binary cannot name itself for the terminal".to_owned(),
                },
            };
            done(Opened::Started(opened));
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not create the launch thread");
    }
}

/// The icon superpackage's root (§6.6), read at RUN TIME.
///
/// # Why not `option_env!`, unlike everything else here
///
/// Because it is the one path the SYSTEM decides. `ronoctua.icons.superpack`
/// exists to leave the 0,61 GiB out of a reduced installation, and a store path
/// compiled into this binary is a reference the closure carries whatever the
/// option says — the switch turned nothing off, and `nix why-depends` is what
/// said so. The other paths here (`setsid`, `choom`, the wasm buffers) are
/// programs the shell cannot run without; this is data it can do without.
///
/// So the value comes from the environment, set by `nix/modules/icons.nix` for
/// the installed system and by `nix/dev.nix` for the nested loop. Unset, the
/// chain starts at `hicolor`: the programs that bring their own icon keep it,
/// and the rest keep their glyphs — which is degradation, and it is what a
/// machine that asked for no superpack wants.
fn superpack() -> Option<String> {
    std::env::var("RONOCTUA_ICONS")
        .ok()
        .filter(|p| !p.is_empty())
}

/// An icon's pixmap side, in pixels.
///
/// The font's body size, and **not** the line height: the line has a `1.45 at`
/// box, and its air is what separates one row from the next — an icon of that
/// height would touch the one above and the one below. A square the size of the body
/// has the height of what the eye reads as a large capital, and fits in the two
/// cells the layout reserves (`18 px` of drawing in `21.6 px` of zone).
#[must_use]
pub fn pixmap_side() -> f32 {
    tokens::text::size()
}

/// The cache key of one app's pixmap: the name AND the side it was drawn at.
///
/// # Why the side is in the key
///
/// Because an `Rgba` is rasterised at a NUMBER OF PIXELS, and `pixmap_side`
/// asks `text::size()` — so the same `Icon=` is a different picture at 100%
/// and at 200%. Keyed by the name alone, the second one can never be uploaded:
/// the renderer's `Images::missing` is `!uploads.contains_key`, and nothing in
/// it removes a key — `clear()` empties the frame's batch and leaves the
/// textures, and `dump()` only acts above its ceiling. Emptying the host's own
/// map does NOT reach that, so the old texture went on being drawn, stretched
/// into the new box, for the life of the footer.
///
/// It is the rule the theme's key already follows for anything a child painted
/// with a token's VALUE, on a second axis — and the one `buffer::svg_key`
/// follows for the same reason on the other side of the process.
#[must_use]
pub fn pixmap_key(icon: &str, side: u32) -> String {
    // `\u{1}` because an `Icon=` is a theme name or an absolute PATH, and a
    // path can hold any byte a filename can — a separator that can appear in
    // one would let two different pairs spell the same key.
    format!("{icon}\u{1}{side}")
}

/// A pixmap request for the thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The raw `Icon=` — a theme name **or** an absolute path.
    pub icon: String,
    /// The square's side, in pixels.
    pub side: u32,
}

/// What the thread returns.
#[derive(Debug, Clone)]
pub struct Painting {
    /// The `Icon=` requested.
    pub icon: String,
    /// The side it was actually drawn at — half of the cache key.
    ///
    /// It travels BACK rather than being read from `pixmap_side()` on arrival,
    /// because the size can move while a request is in flight: read on arrival,
    /// a picture rasterised at 18 would be filed as if it were 36.
    pub side: u32,
    /// The pixels, or `None` when there is no icon for this name.
    ///
    /// `None` is an ANSWER, and not a silent failure: it is what stops
    /// the host from asking again for what already does not exist, on every frame.
    pub rgba: Option<ronoctua_render::image::Rgba>,
}

/// The bridge between the pixel the chain returns and what the renderer consumes.
///
/// The two types are the same field for field, and they are two on purpose:
/// `ronoctua-notify` decodes without depending on the renderer, which drags in the whole
/// of wgpu. The same boundary — and the same crossing — as `ronoctua_clipboard`'s.
fn for_renderer(r: ronoctua_notify::icon::Rgba) -> ronoctua_render::image::Rgba {
    ronoctua_render::image::Rgba {
        width: r.width,
        height: r.height,
        pixels: r.pixels,
    }
}

/// Starts the thread that resolves and rasterises icons.
///
/// # Why one long-lived thread, and not one per request
///
/// Because the expensive part is the SAME for all of them: `icon::look_up` walks the
/// superpackage's size directories, which has 335,031 files, and then every
/// `hicolor` of the session for a name it does not have. One thread
/// per request would pay the creation cost per icon and would still leave twelve rasterisations
/// competing for the processor with the painting.
///
/// The queue is `mpsc` because only it has a blocking `recv`: the thread sleeps
/// while nobody asks.
#[must_use]
pub fn painter(
    sender: calloop::channel::Sender<Painting>,
) -> Option<std::sync::mpsc::Sender<Request>> {
    let (queue, requests) = std::sync::mpsc::channel::<Request>();
    // THE WHOLE CHAIN, and read ONCE: the superpackage, then every `hicolor`
    // and `pixmaps` the session's data directories name — which is where a
    // Flatpak export, `~/.nix-profile` and `~/.local/share/icons` put a
    // program's own icon. The environment is the thread's birth's: it does not
    // change under a running footer, and reading it per icon would be a lookup
    // of variables per row.
    let roots = ronoctua_notify::icon::Roots::of_session(superpack().map(std::path::PathBuf::from));
    let born = std::thread::Builder::new()
        .name("ronoctua-icons".to_owned())
        .spawn(move || {
            while let Ok(p) = requests.recv() {
                let rgba =
                    ronoctua_notify::icon::rasterize_in(&roots, &p.icon, p.side).map(for_renderer);
                if sender
                    .send(Painting {
                        icon: p.icon,
                        side: p.side,
                        rgba,
                    })
                    .is_err()
                {
                    // A closed channel = the footer left.
                    return;
                }
            }
        });
    match born {
        Ok(_) => Some(queue),
        Err(e) => {
            tracing::error!(error = %e, "could not create the icons thread");
            None
        }
    }
}

/// What the launcher remembers about the `applications` directories, to tell
/// whether the catalogue has aged. Compared with `!=`, and nothing else.
pub type Stamp = Vec<DirStamp>;

/// One `applications` directory, as far as "has it changed?" goes.
///
/// # Why four things, when an `mtime` used to be enough
///
/// Comparing the CONTENT would require rescanning, which is what the stamp
/// exists to avoid; a directory's `mtime` changes when a `.desktop` enters or
/// leaves, and for a directory somebody writes into, that is still the case
/// that matters. But the directories the session lists are mostly not written
/// into. They are PROFILES:
///
/// - `~/.nix-profile/share/applications` goes through a link into the store,
///   where every `mtime` is the same second of 1970. `nix profile add` builds a
///   NEW generation and moves the link, and the directory it resolves to is a
///   different one with the same `mtime` — so the stamp never moved, and an
///   installed program stayed out of the list until the footer restarted. What
///   moved is WHERE the path resolves ([`DirStamp::real`]), which is the
///   generation itself; the same is true of the system's and the per-user
///   profile across a rebuild.
/// - A `.desktop` added in a SUBDIRECTORY changes that subdirectory's `mtime`
///   and nothing above it, and the scan reads two levels down
///   ([`ronoctua_apps::DEPTH`]); [`DirStamp::below`] is those directories.
/// - The directory's own `mtime` is kept both ways — of the last component
///   itself ([`DirStamp::link`]) and of what it resolves to
///   ([`DirStamp::mtime`]) — because either can be the one that is a link.
///
/// A directory that does not exist is all `None`s, and that counts: creating
/// `~/.local/share/applications` is exactly the gesture of whoever has just
/// installed their first override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirStamp {
    /// Where the path resolves to, through every link on the way — the
    /// generation, for a profile. `None` when it does not exist.
    pub real: Option<std::path::PathBuf>,
    /// The `mtime` of the path's last component, not followed.
    pub link: Option<std::time::SystemTime>,
    /// The `mtime` of the directory it resolves to.
    pub mtime: Option<std::time::SystemTime>,
    /// Every directory below it that the scan reads, in path order: each with
    /// where it resolves to and its `mtime`.
    pub below: Vec<(
        std::path::PathBuf,
        Option<std::path::PathBuf>,
        Option<std::time::SystemTime>,
    )>,
}

/// The session's `applications` directories, stamped.
///
/// # What it costs, and where it runs
///
/// It runs on the event loop, when `LEADER a` opens (`rescan_if_changed`), so it
/// must stay a handful of system calls. Per directory: one `canonicalize`, two
/// `stat`s, and a `readdir` that `stat`s only what is not a `.desktop` — a
/// profile's directory is a hundred links to `.desktop` files and two or three
/// subdirectories, and the file names say which is which without asking.
#[must_use]
pub fn stamp() -> Stamp {
    stamp_of(&ronoctua_apps::directories())
}

/// [`stamp`], over the directories given — the tests' door.
#[must_use]
pub fn stamp_of(dirs: &[std::path::PathBuf]) -> Stamp {
    dirs.iter()
        .map(|d| {
            let mut below = Vec::new();
            subdirectories(d, 1, &mut below);
            below.sort();
            DirStamp {
                real: std::fs::canonicalize(d).ok(),
                link: std::fs::symlink_metadata(d).and_then(|m| m.modified()).ok(),
                mtime: std::fs::metadata(d).and_then(|m| m.modified()).ok(),
                below,
            }
        })
        .collect()
}

/// The directories under `dir` that the scan reads, down to its depth.
///
/// `level` is the depth of what is LISTED here: the scan reads the
/// `applications` directory (level 0) and the directories down to
/// `DEPTH - 1`, following links as it goes — `is_dir` follows them, and so
/// does `metadata` here.
fn subdirectories(
    dir: &std::path::Path,
    level: usize,
    out: &mut Vec<(
        std::path::PathBuf,
        Option<std::path::PathBuf>,
        Option<std::time::SystemTime>,
    )>,
) {
    if level >= ronoctua_apps::DEPTH {
        return;
    }
    let Ok(items) = std::fs::read_dir(dir) else {
        return;
    };
    for item in items.filter_map(Result::ok) {
        let path = item.path();
        // A `.desktop` is a file, and asking would be a `stat` per program.
        if path.extension().is_some_and(|e| e == "desktop") {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        out.push((
            path.clone(),
            std::fs::canonicalize(&path).ok(),
            meta.modified().ok(),
        ));
        subdirectories(&path, level + 1, out);
    }
}

/// Which application declares that it opens this type.
///
/// # Why the catalogue answers, and not `xdg-open`
///
/// Because the catalogue is already in memory: the sweep at startup read every
/// `.desktop` there is, and `MimeType=` came along with the rest. Spawning
/// `xdg-open` would walk the same directories again, in another process, to
/// reach the same answer — and it would do it inside the frame that draws.
///
/// # The order, and why the FIRST match is the right one
///
/// The catalogue is deduplicated by id with the first occurrence winning, and
/// `directories()` puts `$XDG_DATA_HOME` first — so an entry the person wrote
/// themselves already beats the system's. That is the freedesktop override
/// mechanism, and it is the whole of the preference this needs: a real
/// `mimeapps.list` would add a per-type default on top, and it is the obvious
/// next row of this function rather than a different design.
#[must_use]
pub fn opens<'a>(
    catalogue: &'a Catalogue,
    of_type: &str,
) -> Option<&'a ronoctua_apps::DesktopEntry> {
    let wanted = ronoctua_files::mime::canonical(of_type);
    // OURS FIRST, then anybody's. The player claims every audio and video
    // type, and so do the mpv and the VLC the full profile installs; the
    // catalogue's order between entries of one directory is the order the
    // directory lists them in, which is nobody's decision. A program of this
    // system opening a file this system knows how to play is.
    let ours = |e: &&ronoctua_apps::DesktopEntry| e.id.starts_with(OURS);
    let exact = |e: &&ronoctua_apps::DesktopEntry| e.mime_types.iter().any(|m| m == wanted);
    let parent = |e: &&ronoctua_apps::DesktopEntry| {
        e.mime_types
            .iter()
            .any(|m| ronoctua_files::mime::is_a(wanted, m))
    };
    catalogue
        .iter()
        .filter(ours)
        .find(exact)
        // EXACTLY the type first. A handler for `text/plain` will open a
        // `text/x-rust` — every editor does — but so will fifty other
        // applications, and picking one of those over the one that named the
        // type is how a `.rs` file ends up in a mail client.
        .or_else(|| catalogue.iter().find(exact))
        .or_else(|| catalogue.iter().filter(ours).find(parent))
        .or_else(|| catalogue.iter().find(parent))
}

/// The prefix of this system's own desktop entries.
pub const OURS: &str = "br.ronoctua.";

/// The command line for opening `path` with this entry.
///
/// It re-assembles the entry's `Exec=` with the path in the field codes, rather
/// than appending it: an `Exec=` of `foo --open %f --window` puts the file in
/// the MIDDLE, and appending would hand `--window` the filename.
#[must_use]
pub fn command_for(
    entry: &ronoctua_apps::DesktopEntry,
    path: &std::path::Path,
) -> (String, Vec<String>) {
    let open = vec![path.to_string_lossy().into_owned()];
    // The raw `Exec=` is not kept on the entry — only the assembled command is
    // — so a line with no field code at all gets the path appended, which is
    // what the spec says to do with an entry that declares a type and no
    // placeholder.
    let (program, mut args) = entry.command.clone();
    if args.iter().any(|a| a.contains('%')) {
        // Should not happen: `assemble` consumed the codes at scan time. If it
        // ever does, appending is still better than passing a literal `%f`.
        tracing::debug!(id = %entry.id, "a field code was left over in the command");
    }
    args.extend(open);
    (program, args)
}

#[cfg(test)]
mod tests {
    use super::{Action, Finder, OF_THE_VIEW, glyph_of, pixmap_key, stamp_of};
    use ronoctua_apps::DesktopEntry;
    use ronoctua_keymap::{Key, Mods, Sym};
    use std::sync::Arc;

    fn input_of(id: &str, name: &str, categories: &[&str]) -> DesktopEntry {
        DesktopEntry {
            mime_types: Vec::new(),
            id: id.to_owned(),
            name: name.to_owned(),
            description: format!("description of {name}"),
            icon: id.to_owned(),
            categories: categories.iter().map(|c| (*c).to_owned()).collect(),
            command: (id.to_owned(), Vec::new()),
            working_directory: None,
            terminal: false,
            startup_wm_class: None,
            key: ronoctua_apps::fold::fold(name),
        }
    }

    fn catalogue() -> Arc<Vec<DesktopEntry>> {
        Arc::new(vec![
            input_of("org.gnome.Nautilus", "Arquivos", &["System", "FileManager"]),
            input_of("firefox", "Firefox", &["Network", "WebBrowser"]),
            input_of("galculator", "Cálculo", &["Utility", "Calculator"]),
            input_of("nvim", "Neovim", &["Development", "TextEditor"]),
        ])
    }

    fn t(sym: Sym) -> Key {
        Key {
            mods: Mods::default(),
            sym,
        }
    }

    fn type_in(l: &mut Finder, s: &str) {
        for c in s.chars() {
            l.key(t(Sym::Char(c)));
        }
    }

    /// FAILS IF: an entry is started one way by the footer and another by a
    /// window — the program, its words and its directory as the entry says;
    /// one that needs a terminal inside this binary's own.
    #[test]
    fn an_entry_is_started_one_way_by_every_door() {
        let mut gimp = input_of("org.gimp.GIMP", "GIMP", &[]);
        gimp.command = ("/bin/gimp".to_owned(), vec!["-n".to_owned()]);
        gimp.working_directory = Some("/srv".to_owned());
        assert_eq!(
            super::argv_of(&gimp),
            Some((
                "/bin/gimp".to_owned(),
                vec!["-n".to_owned()],
                Some("/srv".to_owned())
            ))
        );
        let mut top = input_of("htop", "htop", &[]);
        top.command = (
            "/bin/htop".to_owned(),
            vec!["-d".to_owned(), "5".to_owned()],
        );
        top.terminal = true;
        let (program, args, cwd) = super::argv_of(&top).expect("this binary names itself");
        assert_eq!(
            std::path::PathBuf::from(program),
            std::env::current_exe().unwrap()
        );
        assert_eq!(args, ["terminal", "/bin/htop", "-d", "5"]);
        assert_eq!(cwd, None);
    }

    #[test]
    fn opens_with_everything_and_in_catalogue_order() {
        let l = Finder::fresh(catalogue());
        assert_eq!(l.result().len(), 4);
        assert_eq!(l.chosen().map(|e| e.name.as_str()), Some("Arquivos"));
    }

    /// OURS WIN a type another program also claims, whatever order the
    /// directory listed them in — and only then does the exact type beat a
    /// parent type.
    #[test]
    fn our_player_opens_a_film_before_anybody_elses() {
        let mut mpv = input_of("mpv", "mpv", &[]);
        mpv.mime_types = vec!["video/mp4".into()];
        let mut ours = input_of("br.ronoctua.player", "Player", &[]);
        ours.mime_types = vec!["video/mp4".into()];
        let c = Arc::new(vec![mpv, ours]);
        assert_eq!(
            crate::apps::opens(&c, "video/mp4").map(|e| e.id.as_str()),
            Some("br.ronoctua.player")
        );
        assert_eq!(
            crate::apps::opens(&c, "text/plain").map(|e| e.id.as_str()),
            None
        );
    }

    /// THE DEFECT: without folding the query, `calculo` does not find `Cálculo` — and
    /// whoever types does not put an accent into a launcher.
    #[test]
    fn the_search_ignores_accents() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "calculo");
        assert_eq!(l.chosen().map(|e| e.name.as_str()), Some("Cálculo"));
    }

    /// The id is usually the binary: `nvim` has to find "Neovim", and
    /// `nautilus` has to find "Arquivos" — which has none of the letters.
    #[test]
    fn the_desktop_id_finds_it_too() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "nautilus");
        assert_eq!(l.chosen().map(|e| e.name.as_str()), Some("Arquivos"));
    }

    /// THE DEFECT THE TIER PREVENTS: `firefox` matches one item's name and
    /// another's id. Without the tier, the order would come from the length, and an item
    /// that matches through text that is NOT ON SCREEN would appear ahead of the
    /// one that matches by the name the person is reading.
    #[test]
    fn the_name_beats_the_id() {
        let mut c = catalogue().as_ref().clone();
        // An item whose NAME does not match and whose ID matches by subsequence.
        c.push(input_of("firefox-wrapper", "Zzz", &[]));
        let mut l = Finder::fresh(Arc::new(c));
        type_in(&mut l, "firefox");
        assert_eq!(l.chosen().map(|e| e.name.as_str()), Some("Firefox"));
    }

    /// The description stays OUT of the search: `Comment` is prose, and a subsequence
    /// over prose matches half the list without saying why.
    #[test]
    fn the_description_does_not_enter_the_search() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "description");
        assert!(
            l.result().is_empty(),
            "it matched by description: {:?}",
            l.result()
        );
    }

    #[test]
    fn confirm_gives_back_the_input_and_cancel_does_not() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "firefox");
        let Action::Open(e) = l.key(t(Sym::Enter)) else {
            panic!("it did not give back the application");
        };
        assert_eq!(e.command.0, "firefox");
        assert_eq!(l.key(t(Sym::Esc)), Action::Closed);
    }

    /// As in `M-x`: closing would erase what was typed. With nothing typed
    /// and nothing to list there is no row at all, and `RET` does nothing.
    #[test]
    fn confirm_with_no_row_does_nothing() {
        let mut l = Finder::fresh(Arc::new(Vec::new()));
        assert_eq!(l.rows(), 0);
        assert_eq!(l.key(t(Sym::Enter)), Action::Nothing);
        // And the three commands on nothing are nothing, not a panic.
        for (_, id) in OF_THE_VIEW {
            assert_eq!(l.command(id), Action::Nothing, "{id}");
        }
    }

    /// THE LAST ROW: whoever typed `gimp` and found nothing is about to go and
    /// get it, and the launcher is where they already are. With nothing
    /// matching it is the ONLY row, and `RET` takes it — as typed, trimmed.
    #[test]
    fn a_query_that_matches_nothing_offers_to_search_for_it() {
        let mut l = Finder::fresh(catalogue());
        assert_eq!(
            l.offers_search(),
            None,
            "nothing typed, nothing to look for"
        );
        assert_eq!(l.rows(), l.result().len());
        type_in(&mut l, "gimp ");
        assert!(l.result().is_empty());
        assert_eq!(l.offers_search(), Some("gimp"));
        assert_eq!(l.rows(), 1);
        assert!(l.is_search_row(0) && l.cursor() == 0);
        assert_eq!(l.chosen(), None, "the search row is not an application");
        assert_eq!(
            l.key(t(Sym::Enter)),
            Action::SearchToInstall("gimp".to_owned())
        );
        // A query of spaces is no query.
        let mut blank = Finder::fresh(catalogue());
        type_in(&mut blank, "   ");
        assert_eq!(blank.offers_search(), None);
    }

    /// With matches, the search row comes AFTER them, and is reached like any
    /// other row: by the arrow and by the pointer.
    #[test]
    fn the_search_row_follows_the_matches() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "fire");
        let apps = l.result().len();
        assert_eq!(apps, 1);
        assert_eq!(l.rows(), apps + 1);
        assert!(!l.is_search_row(0) && l.is_search_row(apps));
        // `RET` on the match still opens it: the new row changes nothing above it.
        assert!(matches!(l.key(t(Sym::Enter)), Action::Open(_)));
        assert_eq!(l.key(t(Sym::Down)), Action::Changed);
        assert!(l.is_search_row(l.cursor()));
        assert_eq!(l.key(t(Sym::Down)), Action::Nothing, "it went past the end");
        assert_eq!(
            l.key(t(Sym::Enter)),
            Action::SearchToInstall("fire".to_owned())
        );
        // THE POINTER, by the same `Enter`.
        let mut m = Finder::fresh(catalogue());
        type_in(&mut m, "fire");
        assert_eq!(m.choose(apps), Action::SearchToInstall("fire".to_owned()));
        assert_eq!(m.choose(apps + 1), Action::Nothing, "past the last row");
    }

    /// Narrowing the list must not carry the cursor onto the search row by
    /// itself, where `RET` would do something else than a letter ago; but
    /// whoever walked down to it and goes on typing stays there.
    #[test]
    fn typing_keeps_the_cursor_on_its_side_of_the_search_row() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "i");
        let before = l.result().len();
        assert!(before >= 2, "the fixture has to give two matches");
        for _ in 0..before - 1 {
            l.key(t(Sym::Down));
        }
        assert!(l.chosen().is_some());
        type_in(&mut l, "r");
        assert!(!l.result().is_empty());
        assert!(
            l.chosen().is_some(),
            "a letter moved the cursor onto the search row"
        );

        let mut m = Finder::fresh(catalogue());
        type_in(&mut m, "fire");
        m.key(t(Sym::Down));
        assert!(m.is_search_row(m.cursor()));
        type_in(&mut m, "f");
        assert!(m.is_search_row(m.cursor()), "it left the row it was on");
        assert_eq!(
            m.key(t(Sym::Enter)),
            Action::SearchToInstall("firef".to_owned())
        );
    }

    /// `C-i` and `C-d` name the FOCUSED program by its place in the catalogue
    /// — never by its row, which the next letter reorders — and `C-s` pins it.
    #[test]
    fn the_view_keys_act_on_the_focused_program() {
        let chord = |c: char| Key {
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
            sym: Sym::Char(c),
        };
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "neovim");
        let i = l.result()[0];
        assert_eq!(
            catalogue()[i].name,
            "Neovim",
            "the index is the catalogue's"
        );
        assert_eq!(l.key(chord('i')), Action::Details(i));
        assert_eq!(l.key(chord('d')), Action::Remove(i));
        assert!(
            matches!(l.key(chord('s')), Action::Pin(e) if e.name == "Neovim"),
            "pinning went elsewhere"
        );
        // NONE OF THEM IS A LETTER OF THE QUERY.
        assert_eq!(l.query(), "neovim");
        // On the search row there is no program under the cursor.
        l.key(t(Sym::Down));
        assert!(l.is_search_row(l.cursor()));
        assert_eq!(l.key(chord('i')), Action::Nothing);
        assert_eq!(l.key(chord('d')), Action::Nothing);
        assert_eq!(l.key(chord('s')), Action::Nothing);
    }

    /// EVERY KEY OF THE TABLE DOES SOMETHING, and what it does is what its id
    /// says when run by name: the key and `LEADER :` cannot come to mean two
    /// things, because both go through `command`.
    #[test]
    fn every_key_of_the_view_runs_its_command() {
        let mut l = Finder::fresh(catalogue());
        for (notation, id) in OF_THE_VIEW {
            let key = Key::parse(notation).expect("the notation parses");
            assert!(
                key.mods.ctrl,
                "{notation}: a bare key here is a letter of the query"
            );
            let by_name = l.command(id);
            assert_ne!(by_name, Action::Nothing, "{id} does nothing on a program");
            assert_eq!(l.key(key), by_name, "{notation} and {id} disagree");
        }
        // The search row's id works by name wherever the cursor is — the row
        // is there whenever something is typed.
        type_in(&mut l, "zzz");
        l.key(t(Sym::Backspace));
        assert_eq!(
            l.command(super::SEARCH_TO_INSTALL),
            Action::SearchToInstall("zz".to_owned())
        );
        assert_eq!(l.command("apps-no-such-thing"), Action::Nothing);
    }

    /// A scratch directory of this test's own.
    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ronoctua-stamp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("mkdir");
        d
    }

    /// Sets a directory's `mtime`, so a test does not depend on the clock
    /// ticking between two writes.
    fn touch(dir: &std::path::Path, seconds: u64) {
        std::fs::File::open(dir)
            .and_then(|f| {
                f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            })
            .expect("set the mtime");
    }

    /// THE DEFECT: `~/.nix-profile` is a link into the store, where every
    /// `mtime` is the same, so `nix profile add` — which moves the link to a
    /// new generation — never changed the stamp, and the program stayed out of
    /// the list until the footer restarted.
    ///
    /// Both generations here have the SAME `mtime` on purpose: only where the
    /// path resolves can tell them apart, which is the half that was missing.
    #[test]
    fn a_new_profile_generation_changes_the_stamp() {
        let root = scratch("generation");
        for generation in ["gen-1", "gen-2"] {
            let apps = root.join(generation).join("share/applications");
            std::fs::create_dir_all(&apps).expect("mkdir");
            std::fs::write(apps.join("a.desktop"), "[Desktop Entry]\n").expect("write");
            touch(&apps, 1);
        }
        let profile = root.join("profile");
        std::os::unix::fs::symlink(root.join("gen-1"), &profile).expect("link");
        let dirs = [profile.join("share/applications")];
        let before = stamp_of(&dirs);

        std::fs::remove_file(&profile).expect("unlink");
        std::os::unix::fs::symlink(root.join("gen-2"), &profile).expect("link");
        let after = stamp_of(&dirs);
        std::fs::remove_dir_all(&root).ok();

        assert_eq!(
            (before[0].mtime, before[0].link),
            (after[0].mtime, after[0].link),
            "the fixture has to hide the change from the mtimes, as the store does"
        );
        assert_ne!(before, after, "a new generation went unnoticed");
    }

    /// THE DEFECT: the scan reads two levels down, and a `.desktop` added in a
    /// subdirectory changes that subdirectory's `mtime` and nothing above it.
    #[test]
    fn a_desktop_file_added_in_a_subdirectory_changes_the_stamp() {
        let root = scratch("subdirectory");
        let deep = root.join("applications/vendor/more");
        std::fs::create_dir_all(&deep).expect("mkdir");
        // Old, and fixed: the write below is then a change whatever the clock's
        // resolution.
        touch(&deep, 1);
        touch(&root.join("applications/vendor"), 1);
        let dirs = [root.join("applications")];
        let before = stamp_of(&dirs);
        std::fs::write(deep.join("x.desktop"), "[Desktop Entry]\n").expect("write");
        let after = stamp_of(&dirs);
        std::fs::remove_dir_all(&root).ok();

        assert_eq!(
            before[0].mtime, after[0].mtime,
            "the top directory did not change — the old stamp would have missed this"
        );
        assert_ne!(before, after, "a file two levels down went unnoticed");
    }

    /// A directory that does not exist counts: creating
    /// `~/.local/share/applications` is the first override anybody writes.
    #[test]
    fn a_directory_appearing_changes_the_stamp() {
        let root = scratch("appearing");
        let dirs = [root.join("applications")];
        let before = stamp_of(&dirs);
        assert_eq!(before[0].real, None);
        std::fs::create_dir_all(&dirs[0]).expect("mkdir");
        let after = stamp_of(&dirs);
        // And nothing changed is no change: the comparison is what decides a
        // rescan, and a stamp that differed from itself would rescan every time.
        let again = stamp_of(&dirs);
        std::fs::remove_dir_all(&root).ok();
        assert_ne!(before, after);
        assert_eq!(after, again);
    }

    #[test]
    fn the_cursor_neither_goes_past_the_ends_nor_ends_outside_on_filtering() {
        let mut l = Finder::fresh(catalogue());
        assert_eq!(
            l.key(t(Sym::Up)),
            Action::Nothing,
            "it came up from the top"
        );
        for _ in 0..l.result().len() + 3 {
            l.key(t(Sym::Down));
        }
        assert_eq!(l.cursor(), l.result().len() - 1);
        assert_eq!(l.key(t(Sym::Down)), Action::Nothing, "it went past the end");
        // Filtering after moving down must not leave the cursor outside: `Enter`
        // would do nothing, without saying why.
        type_in(&mut l, "firefox");
        assert!(l.chosen().is_some() || l.result().is_empty());
    }

    /// THE DEFECT: rescanning with the launcher open erased the query, and
    /// whoever was typing watched the letters disappear on their own.
    #[test]
    fn recataloguing_keeps_what_was_typed() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "fire");
        l.recatalogue(catalogue());
        assert_eq!(l.query(), "fire");
        assert_eq!(l.chosen().map(|e| e.name.as_str()), Some("Firefox"));
    }

    /// THE DEFECT THIS CATCHES: a table that does not reach the Rust makes EVERYTHING become
    /// the same little square — and generic looks deliberate.
    #[test]
    fn the_glyph_is_not_only_the_generic_one() {
        let c = catalogue();
        let glyphs: Vec<char> = c.iter().map(glyph_of).collect();
        assert!(
            glyphs.iter().all(|g| *g != super::tokens::icon::GENERIC),
            "somebody fell back to the generic one: {glyphs:?}"
        );
        // Four different categories have to give four different glyphs.
        let unique: std::collections::BTreeSet<char> = glyphs.iter().copied().collect();
        assert_eq!(unique.len(), 4, "repeated glyphs: {glyphs:?}");
    }

    /// THE TABLE'S ORDER decides: `Network;WebBrowser` matches both, and what
    /// has to win is the specific one.
    #[test]
    fn the_specific_category_beats_the_generic_one() {
        let browser = input_of("x", "X", &["Network", "WebBrowser"]);
        let network = input_of("y", "Y", &["Network"]);
        assert_ne!(
            glyph_of(&browser),
            glyph_of(&network),
            "the browser came out with the network glyph"
        );
    }

    /// A test SVG: an OPAQUE red square over half the canvas, with the
    /// rest transparent. The two halves prove different things.
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
         <rect x="0" y="0" width="32" height="64" fill="rgb(255,0,0)"/>
       </svg>"#;

    #[test]
    fn the_svg_rasterises_at_the_requested_size() {
        let r =
            ronoctua_notify::icon::from_svg(SVG.as_bytes(), 18, ronoctua_notify::icon::Fit::Fill)
                .expect("the SVG did not rasterise");
        assert_eq!((r.width, r.height), (18, 18));
        assert_eq!(r.pixels.len(), 18 * 18 * 4);
    }

    /// THE DEFECT: `tiny-skia` delivers **premultiplied** RGBA and the pipeline
    /// expects straight. Without unmultiplying, every semitransparent pixel comes out
    /// too dark — and the symptom is "the icons look dirty", which nobody
    /// connects to alpha multiplication.
    ///
    /// A pure, opaque red has to come out as a whole `(255, 0, 0, 255)`.
    #[test]
    fn the_colour_does_not_arrive_premultiplied_by_alpha() {
        let r =
            ronoctua_notify::icon::from_svg(SVG.as_bytes(), 18, ronoctua_notify::icon::Fit::Fill)
                .expect("the SVG did not rasterise");
        // A pixel well inside the painted half, far from the antialiased edge.
        let i = (9 * 18 + 4) * 4;
        assert_eq!(
            &r.pixels[i..i + 4],
            &[255, 0, 0, 255],
            "the opaque red came out changed"
        );
        // And the other side stays transparent — the fit did not paint a background.
        let j = (9 * 18 + 15) * 4;
        assert_eq!(r.pixels[j + 3], 0, "the transparent one became opaque");
    }

    /// A broken SVG is `None`, and not a panic: the superpackage has 335 thousand
    /// files from five different themes, and one of them is going to be corrupt.
    #[test]
    fn an_invalid_file_does_not_bring_the_footer_down() {
        assert!(
            ronoctua_notify::icon::from_svg(b"i am not svg", 18, ronoctua_notify::icon::Fit::Fill)
                .is_none()
        );
        assert!(ronoctua_notify::icon::from_raster(b"not a png either", 18).is_none());
        assert!(
            ronoctua_notify::icon::from_svg(SVG.as_bytes(), 0, ronoctua_notify::icon::Fit::Fill)
                .is_none(),
            "a side of zero"
        );
    }

    /// The tray table beats the category: the app has a glyph of its own.
    #[test]
    fn the_app_table_beats_the_category() {
        let ff = input_of("firefox", "Firefox", &["Network", "WebBrowser"]);
        let otherwise = input_of("anything", "Anything", &["Network", "WebBrowser"]);
        assert_ne!(glyph_of(&ff), glyph_of(&otherwise));
    }

    #[test]
    fn the_pixmap_key_changes_with_the_side() {
        // THE WHOLE POINT: one name, two sizes, two keys. Equal keys here mean
        // the renderer is asked "is this already up?" about a picture of the
        // other size, answers yes, and draws it stretched — and no later call
        // can undo that, because `Images` has no per-key removal.
        assert_ne!(pixmap_key("firefox", 18), pixmap_key("firefox", 36));
        // And one side, two names, two keys — the half that was never broken.
        assert_ne!(pixmap_key("firefox", 18), pixmap_key("thunderbird", 18));
        // The same pair is the same key, or nothing would ever be cached.
        assert_eq!(pixmap_key("firefox", 18), pixmap_key("firefox", 18));
        // AND NO PAIR SPELLS ANOTHER PAIR'S KEY. An `Icon=` can be an absolute
        // path, so the separator has to be a byte a filename cannot hold: with
        // a printable one, ("a", 1) and ("a<SEP>1", ...) collide.
        assert_ne!(pixmap_key("a\u{1}1", 8), pixmap_key("a", 18));
    }
}
