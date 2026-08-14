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
    /// Which result is focused.
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
    #[must_use]
    pub fn result(&self) -> &[usize] {
        &self.result
    }

    /// Which row is focused.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The entry on row `n` of the visible list.
    #[must_use]
    pub fn at(&self, n: usize) -> Option<&ronoctua_apps::DesktopEntry> {
        self.catalogue.get(*self.result.get(n)?)
    }

    /// The focused entry.
    #[must_use]
    pub fn chosen(&self) -> Option<&ronoctua_apps::DesktopEntry> {
        self.at(self.cursor)
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
        self.result = crate::minibuffer::rank(&q, &items);
        self.cursor = self.cursor.min(self.result.len().saturating_sub(1));
    }

    /// Picks row `n` and confirms, as a click does.
    ///
    /// It goes through the SAME `Enter` as the keyboard for `M-x`'s reason: nothing may be
    /// reachable by mouse only, and the same row must not have two
    /// behaviours depending on how one got to it.
    pub fn choose(&mut self, n: usize) -> Action {
        if n >= self.result.len() {
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
            // start over.
            Gesture::Confirm => match self.chosen() {
                Some(e) => Action::Open(Box::new(e.clone())),
                None => Action::Nothing,
            },
        }
    }

    /// Moves the cursor, without leaving the list.
    fn move_by(&mut self, d: i32) -> Action {
        let fresh = self.cursor as i32 + d;
        if fresh < 0 || fresh >= self.result.len() as i32 {
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
pub fn scan(language: &str, envio: calloop::channel::Sender<Catalogue>) {
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
                "catalogo de aplicativos varrido"
            );
            // A closed channel = the footer left. Nothing to deliver.
            let _ = envio.send(Arc::new(list));
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "nao consegui criar a thread de varredura");
    }
}

/// The icon superpackage's root (§6.6), by store path.
///
/// The same contract as the others: `option_env!` so `cargo build` outside Nix
/// goes on compiling. Without it, the pixmap step simply does not happen
/// and the list is left with the glyphs — which is degradation, not a defect.
const SUPERPACK: Option<&str> = option_env!("RONOCTUA_ICONS");

/// An icon's pixmap side, in pixels.
///
/// The font's body size, and **not** the line height: the line has a `1.45 at`
/// box, and its air is what separates one row from the next — an icon of that
/// height would touch the one above and the one below. A square the size of the body
/// has the height of what the eye reads as a large capital, and fits in the two
/// cells the layout reserves (`18 px` of drawing in `21.6 px` of zone).
#[must_use]
pub fn pixmap_side() -> f32 {
    tokens::font::SIZE_BASE
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
    /// The `Icon=` requested — it is the cache key on this side.
    pub icon: String,
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
/// superpackage's size directories, which has 335,031 files. One thread
/// per request would pay the creation cost per icon and would still leave twelve rasterisations
/// competing for the processor with the painting.
///
/// The queue is `mpsc` because only it has a blocking `recv`: the thread sleeps
/// while nobody asks.
#[must_use]
pub fn painter(
    envio: calloop::channel::Sender<Painting>,
) -> Option<std::sync::mpsc::Sender<Request>> {
    let (fila, requests) = std::sync::mpsc::channel::<Request>();
    let root = SUPERPACK.map(std::path::PathBuf::from);
    let born = std::thread::Builder::new()
        .name("ronoctua-icones".to_owned())
        .spawn(move || {
            while let Ok(p) = requests.recv() {
                let rgba = ronoctua_notify::icon::rasterize(root.as_deref(), &p.icon, p.side)
                    .map(for_renderer);
                if envio.send(Painting { icon: p.icon, rgba }).is_err() {
                    // A closed channel = the footer left.
                    return;
                }
            }
        });
    match born {
        Ok(_) => Some(fila),
        Err(e) => {
            tracing::error!(error = %e, "nao consegui criar a thread de icones");
            None
        }
    }
}

/// When each `applications` directory last changed.
///
/// It is what says whether the catalogue has aged. Comparing the CONTENT would require
/// rescanning, which is what we want to avoid; a directory's `mtime` changes when a
/// `.desktop` enters or leaves, which is the case that matters.
///
/// A directory that does not exist comes in as `None`, and that counts: creating
/// `~/.local/share/applications` is exactly the gesture of whoever has just
/// installed their first override.
#[must_use]
pub fn stamp() -> Vec<Option<std::time::SystemTime>> {
    ronoctua_apps::directories()
        .iter()
        .map(|d| std::fs::metadata(d).and_then(|m| m.modified()).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Action, Finder, glyph_of};
    use ronoctua_apps::DesktopEntry;
    use ronoctua_keymap::{Key, Mods, Sym};
    use std::sync::Arc;

    fn input_of(id: &str, name: &str, categories: &[&str]) -> DesktopEntry {
        DesktopEntry {
            id: id.to_owned(),
            name: name.to_owned(),
            description: format!("descricao de {name}"),
            icon: id.to_owned(),
            categories: categories.iter().map(|c| (*c).to_owned()).collect(),
            command: (id.to_owned(), Vec::new()),
            working_directory: None,
            terminal: false,
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

    #[test]
    fn opens_with_everything_and_in_catalogue_order() {
        let l = Finder::fresh(catalogue());
        assert_eq!(l.result().len(), 4);
        assert_eq!(l.chosen().map(|e| e.name.as_str()), Some("Arquivos"));
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
        type_in(&mut l, "descricao");
        assert!(
            l.result().is_empty(),
            "casou pela descricao: {:?}",
            l.result()
        );
    }

    #[test]
    fn confirm_gives_back_the_input_and_cancel_does_not() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "firefox");
        let Action::Open(e) = l.key(t(Sym::Enter)) else {
            panic!("nao devolveu o aplicativo");
        };
        assert_eq!(e.command.0, "firefox");
        assert_eq!(l.key(t(Sym::Esc)), Action::Closed);
    }

    /// As in `M-x`: closing would erase what was typed.
    #[test]
    fn confirm_with_no_result_does_not_close() {
        let mut l = Finder::fresh(catalogue());
        type_in(&mut l, "zzzznaoexiste");
        assert!(l.result().is_empty());
        assert_eq!(l.key(t(Sym::Enter)), Action::Nothing);
    }

    #[test]
    fn the_cursor_neither_goes_past_the_ends_nor_ends_outside_on_filtering() {
        let mut l = Finder::fresh(catalogue());
        assert_eq!(l.key(t(Sym::Up)), Action::Nothing, "subiu do topo");
        for _ in 0..l.result().len() + 3 {
            l.key(t(Sym::Down));
        }
        assert_eq!(l.cursor(), l.result().len() - 1);
        assert_eq!(l.key(t(Sym::Down)), Action::Nothing, "passou do fim");
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
            "alguem caiu no generico: {glyphs:?}"
        );
        // Four different categories have to give four different glyphs.
        let unicos: std::collections::BTreeSet<char> = glyphs.iter().copied().collect();
        assert_eq!(unicos.len(), 4, "glifos repetidos: {glyphs:?}");
    }

    /// THE TABLE'S ORDER decides: `Network;WebBrowser` matches both, and what
    /// has to win is the specific one.
    #[test]
    fn the_specific_category_beats_the_generic_one() {
        let navegador = input_of("x", "X", &["Network", "WebBrowser"]);
        let network = input_of("y", "Y", &["Network"]);
        assert_ne!(
            glyph_of(&navegador),
            glyph_of(&network),
            "o navegador saiu com o glifo de rede"
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
                .expect("o SVG nao rasterizou");
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
                .expect("o SVG nao rasterizou");
        // A pixel well inside the painted half, far from the antialiased edge.
        let i = (9 * 18 + 4) * 4;
        assert_eq!(
            &r.pixels[i..i + 4],
            &[255, 0, 0, 255],
            "o vermelho opaco saiu alterado"
        );
        // And the other side stays transparent — the fit did not paint a background.
        let j = (9 * 18 + 15) * 4;
        assert_eq!(r.pixels[j + 3], 0, "o transparente virou opaco");
    }

    /// A broken SVG is `None`, and not a panic: the superpackage has 335 thousand
    /// files from five different themes, and one of them is going to be corrupt.
    #[test]
    fn an_invalid_file_does_not_bring_the_footer_down() {
        assert!(
            ronoctua_notify::icon::from_svg(b"nao sou svg", 18, ronoctua_notify::icon::Fit::Fill)
                .is_none()
        );
        assert!(ronoctua_notify::icon::from_raster(b"nem png", 18).is_none());
        assert!(
            ronoctua_notify::icon::from_svg(SVG.as_bytes(), 0, ronoctua_notify::icon::Fit::Fill)
                .is_none(),
            "lado zero"
        );
    }

    /// The tray table beats the category: the app has a glyph of its own.
    #[test]
    fn the_app_table_beats_the_category() {
        let ff = input_of("firefox", "Firefox", &["Network", "WebBrowser"]);
        let otherwise = input_of("qualquer", "Qualquer", &["Network", "WebBrowser"]);
        assert_ne!(glyph_of(&ff), glyph_of(&otherwise));
    }
}
