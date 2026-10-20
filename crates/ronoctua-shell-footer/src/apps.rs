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

// The SAME `calloop` as the event loop's, and not a direct dependency: two
// versions of the crate would give two channel types with the same name, and the error
// would show up as a type mismatch without saying they are two crates.
use crate::launch::Catalogue;

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
    /// `C-v` or `C-y`: the host reads the clipboard — the LIVE selection,
    /// off the loop (`crate::pasting`) — and hands it to [`Finder::paste`].
    /// Every text input of this system pastes (`every-text-input-pastes`),
    /// and the query was the one that did not.
    WantsPaste,
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
/// table, so a key added here is a key `bindings::reachable` counts.
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
    texts.t_with(crate::key::APPS_SEARCH_TO_INSTALL, &[("query", query)])
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
        // `C-v` AND `C-y` PASTE into the query, as they do on the minibuffer's
        // line (`line::Line::key`) — Emacs's yank and everybody else's paste.
        if crate::line::pastes(t) {
            return Action::WantsPaste;
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

    /// The clipboard's text, typed into the query as if each character had
    /// been — one line of it: a query is one line, and a paste that carried a
    /// newline would be a query nobody can see the end of
    /// (`line::one_line`).
    pub fn paste(&mut self, text: &str) -> Action {
        let typed = crate::line::one_line(text);
        if typed.is_empty() {
            return Action::Nothing;
        }
        self.query.push_str(&typed);
        self.filter_by();
        Action::Changed
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

#[cfg(test)]
mod tests {
    use super::{Action, Finder, OF_THE_VIEW};
    use crate::launch::{glyph_of, pixmap_key, stamp_of};
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
            fallback: true,
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
            crate::launch::argv_of(&gimp),
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
        let (program, args, cwd) = crate::launch::argv_of(&top).expect("this binary names itself");
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
        let none = ronoctua_apps::defaults::Defaults::default();
        assert_eq!(
            crate::launch::opens_by(&c, &[], &none, "video/mp4").map(|e| e.id.as_str()),
            Some("br.ronoctua.player")
        );
        assert_eq!(
            crate::launch::opens_by(&c, &[], &none, "text/plain").map(|e| e.id.as_str()),
            None
        );
    }

    /// FAILS IF: a program that declines the name order
    /// (`X-Ronoctua-Fallback=false`) still takes a type by its name — by the
    /// type or by a parent —, or a default naming it no longer wins, or a
    /// default naming an entry the launcher does not list (the player's, which
    /// is `NoDisplay`) is skipped.
    #[test]
    fn a_program_out_of_the_name_order_opens_only_by_choice() {
        let mut browser = input_of("google-chrome", "Google Chrome", &[]);
        browser.mime_types = vec!["image/png".into()];
        let mut design = input_of("design", "A Design Tool", &[]);
        design.mime_types = vec![
            "image/png".into(),
            "image/svg+xml".into(),
            "text/plain".into(),
        ];
        design.fallback = false;
        let c = Arc::new(vec![design, browser]);
        let none = ronoctua_apps::defaults::Defaults::default();
        let opener = |unlisted: &[DesktopEntry], d: &ronoctua_apps::defaults::Defaults, t: &str| {
            crate::launch::opens_by(&c, unlisted, d, t).map(|e| e.id.clone())
        };
        assert_eq!(
            opener(&[], &none, "image/png").as_deref(),
            Some("google-chrome")
        );
        assert_eq!(
            opener(&[], &none, "image/svg+xml"),
            None,
            "a parent type let it in"
        );
        let chosen = ronoctua_apps::defaults::Defaults::of_text(
            "[Default Applications]\nimage/png=design.desktop\nvideo/mp4=br.ronoctua.player.desktop\n",
        );
        assert_eq!(opener(&[], &chosen, "image/png").as_deref(), Some("design"));
        let mut player = input_of("br.ronoctua.player", "Player", &[]);
        player.mime_types = vec!["video/mp4".into()];
        assert_eq!(opener(&[], &chosen, "video/mp4"), None);
        assert_eq!(
            opener(std::slice::from_ref(&player), &chosen, "video/mp4").as_deref(),
            Some("br.ronoctua.player")
        );
    }

    /// FAILS IF: a `.step` — `model/step`, a `text/plain` by the database —
    /// goes to the editor that declares `text/plain` while a list chose
    /// OrcaSlicer for it; or a default naming a program that is not installed
    /// is obeyed instead of skipped; or the old alias of a type is not read
    /// as the type.
    #[test]
    fn a_default_opens_a_model_before_any_editor() {
        let mut editor = input_of("emacs", "Emacs", &[]);
        editor.mime_types = vec!["text/plain".into()];
        let mut orca = input_of("OrcaSlicer", "OrcaSlicer", &[]);
        orca.mime_types = vec!["model/stl".into(), "model/3mf".into()];
        let c = Arc::new(vec![editor, orca]);
        let opener = |defaults: &ronoctua_apps::defaults::Defaults, t: &str| {
            crate::launch::opens_by(&c, &[], defaults, t).map(|e| e.id.clone())
        };

        let none = ronoctua_apps::defaults::Defaults::default();
        // WITHOUT a list, the parent type answers — the defect.
        assert_eq!(opener(&none, "model/step").as_deref(), Some("emacs"));

        let chosen = ronoctua_apps::defaults::Defaults::of_text(
            "[Default Applications]\n\
             model/step=gone.desktop;OrcaSlicer.desktop;\n\
             application/vnd.ms-3mfdocument=OrcaSlicer.desktop\n",
        );
        assert_eq!(opener(&chosen, "model/step").as_deref(), Some("OrcaSlicer"));
        assert_eq!(opener(&chosen, "model/3mf").as_deref(), Some("OrcaSlicer"));
        // A type no list names still goes by the declarations.
        assert_eq!(opener(&chosen, "text/plain").as_deref(), Some("emacs"));

        // A default that names only what is not installed is no answer.
        let gone = ronoctua_apps::defaults::Defaults::of_text(
            "[Default Applications]\nmodel/step=gone.desktop\n",
        );
        assert_eq!(opener(&gone, "model/step").as_deref(), Some("emacs"));
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
            glyphs.iter().all(|g| *g != ronoctua_tokens::icon::GENERIC),
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

    /// FAILS IF: `C-v` or `C-y` in the query is not a paste — a letter, or
    /// nothing — or the pasted text does not filter as typing does, or a
    /// newline in it reaches the query.
    #[test]
    fn the_query_pastes() {
        let mut l = Finder::fresh(catalogue());
        for c in ['v', 'y'] {
            let key = Key {
                mods: Mods {
                    ctrl: true,
                    ..Mods::default()
                },
                sym: Sym::Char(c),
            };
            assert_eq!(l.key(key), Action::WantsPaste, "C-{c}");
            assert_eq!(l.query(), "", "C-{c} typed a letter");
        }
        assert_eq!(l.paste("fire\nfox"), Action::Changed);
        assert_eq!(l.query(), "fire fox");
        let mut typed = Finder::fresh(catalogue());
        type_in(&mut typed, "ne");
        assert_eq!(typed.paste("o"), Action::Changed);
        assert_eq!(
            typed.query(),
            "neo",
            "the paste did not go on from what was typed"
        );
        assert_eq!(
            typed.paste("  \n "),
            Action::Nothing,
            "a blank paste changed the query"
        );
    }
}
