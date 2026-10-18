//! FAST-ACCESS IN THE FOOTER'S REGION (`LEADER d d`, `Super+d`) — the search
//! over everything as a [`Region`] of this crate, which the footer hosts by
//! its id and names nothing else of.
//!
//! # What is here, and what is every door's
//!
//! What is typed, what came back, the cursor and what a key does are the
//! view's (`fast_access`); the feed, the in-process sources' ground and the
//! file panel's threads, and turning the view's errands into what a door does
//! ([`Out`]), are every door's (`fast_access_window::Search`). This is the
//! footer's door around them: the search laid over the region's blank rows,
//! its pointer and its keys measured against the region's own place, and what
//! an [`Out`] is HERE — something opened closes the view, and what the
//! footer is told to open runs after it closed, so it finds the region free.
//!
//! # What the footer still does for the search
//!
//! Its doors hand the footer what outlives them — an application by its
//! desktop id, a file by what opens its type, a folder in the file manager, a
//! manual page in a terminal, the column of the workspace in focus — because
//! the footer holds the launcher's catalogue, the file manager's opener and
//! the workspaces. Those commands are the footer's; the region answers none
//! of them.
//!
//! # A photograph's search
//!
//! A photograph's footer has no loop to hear a thread on, and reaches
//! nothing of the machine but its room. Its search is the same view over a
//! feed that answers INSIDE the call that asked (`Feed::inline_at`, the
//! window's camera's): the in-process sources as on a screen, the index in
//! the room's own place, no host asked — and a file's panels answered on
//! the spot (`Search::inspect_now`). What came back is taken before the call
//! returns, as the window's camera takes it after each step
//! (`fast_access_photograph::settle`), so the frame drawn after a key is the
//! frame a screen draws once the answers are in.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use crate::fast_access_rows::Target;
use crate::fast_access_search::{Back, Feed, Sink};
use crate::fast_access_window::{Out, Search};
use crate::icons_page::Area;
use crate::region::{Closed, Doorway, Pointed, Pointing, Region, Row, Seen, Shape, Taken};

/// The region's id, as the footer's precedence ranks it.
pub const ID: &str = "fast-access";

/// The region the binary registers (`regions.rs`).
pub const ENTRY: crate::region::Entry = crate::region::Entry {
    id: ID,
    closed,
    open,
};

/// The questions the search asks on the door's line, by the ids the answers
/// come back under.
const QUESTIONS: [&str; 7] = [
    crate::fast_access_detail::ASK_NAME,
    crate::fast_access_detail::ASK_MODIFIED,
    crate::fast_access_detail::ASK_TAG,
    crate::fast_access_detail::ASK_META,
    crate::fast_access_detail::ASK_COLOUR,
    crate::fast_access::ASK_SEARXNG,
    crate::fast_access::ASK_BRAVE,
];

/// What a command does while the search is closed: `fast-access-open` opens
/// it, the window's two open it in a window, and an answer for a search no
/// longer up finds nobody to tell.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "fast-access-open" => Closed::Opens,
        "fast-access-window" | "fast-access-float" => {
            open_the_window(command, argument, None, way);
            Closed::Taken
        }
        id if QUESTIONS.contains(&id) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// The search, raised: its feed, its settings watched, the file panel's
/// threads — all waking the region through its own wake — and the ground
/// the in-process sources read (the launcher's applications are told at
/// opening, `Region::heard`).
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    if way.photographing() {
        return Box::new(FastAccessRegion::photographed(
            way,
            None,
            crate::fast_access::store().read(),
        ));
    }
    let wake = way.wake();
    let (sender, backs) = mpsc::channel::<Back>();
    let sender = Mutex::new(sender);
    let woken = wake.clone();
    let sink: Sink = Arc::new(move |b| {
        if let Ok(s) = sender.lock()
            && s.send(b).is_ok()
        {
            woken.call();
        }
    });
    // `fast-access.json`, WATCHED while the view is open: a window's
    // remembered openings and switches reach this door too.
    let stirred = Arc::new(AtomicBool::new(false));
    let watch = {
        let stirred = Arc::clone(&stirred);
        let wake = wake.clone();
        ronoctua_store::config_dir().and_then(|dir| {
            ronoctua_files::notice::Notice::on_file(
                &dir,
                crate::fast_access::FILE,
                ronoctua_files::notice::Pace::Coalesced,
                move || {
                    stirred.store(true, Ordering::Relaxed);
                    wake.call();
                    true
                },
            )
            .ok()
        })
    };
    // THE FILE PANEL'S THREADS, raised for this opening: what they say goes
    // to the region's own queue and wakes it, and they end with the region
    // (`Threads::to`).
    let (told, heard) = mpsc::channel::<crate::inspection::Heard>();
    let told = Mutex::new(told);
    let threads = crate::inspection::Threads::to(&ronoctua_shell_kit::wake::Out::new(move |h| {
        let heard = told.lock().is_ok_and(|t| t.send(h).is_ok());
        if heard {
            wake.call();
        }
        heard
    }));
    let feed = Feed::start(sink);
    let view = crate::fast_access::View::new(crate::fast_access::store().read(), way.texts());
    // SAID ONCE PER OPENING: what `fast-access-vm` reads to know the leader's
    // sequence and the chord reached this view.
    tracing::info!("fast-access opened in the region");
    Box::new(FastAccessRegion {
        search: Search::new(view, Some(feed), ground(), threads),
        backs,
        heard,
        stirred,
        _watch: watch,
        rests: None,
        photographed: false,
    })
}

/// What the in-process sources read in the footer: the commands with their
/// keys, the person's zone and the queue's keys — the launcher's
/// applications are told at opening (`Region::heard`).
fn ground() -> crate::fast_access::Ground {
    let commands = crate::bindings::read(crate::fast_access_search::commands_of)
        .map_err(|e| tracing::warn!(error = %e, "the search finds no command"))
        .unwrap_or_default();
    crate::fast_access::Ground {
        catalogue: Arc::default(),
        commands,
        zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        zoneinfo: std::env::var_os("RONOCTUA_ZONEINFO").map(std::path::PathBuf::from),
        queue_keys: crate::queue::queue_keys(),
    }
}

/// THE WINDOW OR THE FLOATING ONE, typed with the words given — or with what
/// the region's own box held, `typed`, when none were: one search, in the
/// door asked for. The region open, it gives way to the window (`typed` is
/// the open region's).
fn open_the_window(
    command: &str,
    argument: Option<&str>,
    typed: Option<&str>,
    way: &mut dyn Doorway,
) {
    if std::env::current_exe().is_err() {
        let said = way.texts().t(crate::key::FAST_ACCESS_NO_SHELL);
        way.say(said, crate::echo::Echo::Normal);
        return;
    }
    if typed.is_some() {
        way.close();
    }
    let query = argument.map(str::trim).filter(|a| !a.is_empty());
    let mut args = vec!["fast-access".to_owned()];
    if command == "fast-access-float" {
        args.push("--float".to_owned());
    }
    args.extend(query.map(ToOwned::to_owned));
    if query.is_none()
        && let Some(typed) = typed.filter(|t| !t.trim().is_empty())
    {
        args.push(typed.to_owned());
    }
    way.open_window(args);
}

/// The search's place in the region: from the region's first line to the
/// surface's bottom, between the two insets — from the footer's measure,
/// never from what the region holds (`the-footer-does-not-ask-itself`).
fn area_of(at: &crate::region::Area) -> Area {
    let line = ronoctua_tokens::metrics::line_height(ronoctua_tokens::text::size());
    let top = Area::y_of(at.first);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lines = ((at.bottom - top).max(0.0) / line).floor() as u16;
    let columns = crate::chrome::Frame::of(at.width, at.bottom, at.first).columns();
    Area {
        left: crate::chrome::INSET,
        top: at.first,
        columns: columns.saturating_sub(2 * crate::chrome::INSET),
        lines,
        right_px: at.width,
        bottom_px: at.bottom,
    }
}

/// The search in the footer's region — see the module.
pub struct FastAccessRegion {
    /// The view, its feed, its ground and its panel's threads.
    search: Search,
    /// What the feed's threads came back with.
    backs: mpsc::Receiver<Back>,
    /// What the file panel's threads came back with.
    heard: mpsc::Receiver<crate::inspection::Heard>,
    /// `fast-access.json` moved since the last look.
    stirred: Arc<AtomicBool>,
    _watch: Option<ronoctua_files::notice::Notice>,
    /// What the hand of the last pointer frame rests on ([`Region::rests_on`]).
    rests: Option<Option<usize>>,
    /// A photograph's search: its feed answers inside the call, and what
    /// came back is taken before the call returns (the module says why).
    photographed: bool,
}

impl std::fmt::Debug for FastAccessRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastAccessRegion")
            .field("search", &self.search)
            .finish_non_exhaustive()
    }
}

impl FastAccessRegion {
    /// A photograph's search — see the module: the view over `settings`, a
    /// feed that answers inside the call and reads the index in `index` (the
    /// room's own place when `None`), no panel's threads and no watch.
    fn photographed(
        way: &dyn Doorway,
        index: Option<std::path::PathBuf>,
        settings: crate::fast_access::Settings,
    ) -> Self {
        let (sender, backs) = mpsc::channel::<Back>();
        let sender = Mutex::new(sender);
        let sink: Sink = Arc::new(move |b| {
            if let Ok(s) = sender.lock() {
                let _ = s.send(b);
            }
        });
        let feed = Feed::inline_at(sink, index);
        let view = crate::fast_access::View::new(settings, way.texts());
        let (_, heard) = mpsc::channel();
        tracing::info!("fast-access opened in the region, for a photograph");
        Self {
            search: Search::new(
                view,
                Some(feed),
                ground(),
                crate::inspection::Threads::none(),
            ),
            backs,
            heard,
            stirred: Arc::new(AtomicBool::new(false)),
            _watch: None,
            rests: None,
            photographed: true,
        }
    }

    /// Whatever the feed, the file panel's threads and the settings file
    /// left, taken and done. Whether anything had come back.
    fn took_what_came_back(&mut self, way: &mut dyn Doorway) -> bool {
        let texts = way.words();
        let mut came = false;
        while let Ok(back) = self.backs.try_recv() {
            came = true;
            if let Some(outs) = self.search.back(back, &texts) {
                self.carry_out(outs, way);
            }
        }
        while let Ok(h) = self.heard.try_recv() {
            came = true;
            if self.search.panels_heard(h) {
                way.repaint();
            }
        }
        if self.stirred.swap(false, Ordering::Relaxed) {
            came = true;
            self.search.settings_changed(&texts);
            way.repaint();
        }
        came
    }

    /// A PHOTOGRAPH'S ANSWERS, taken before the call that asked returns —
    /// see the module — and a file's panels answered on the spot. Rounds
    /// while something came back, a bounded number of them: an answer that
    /// asks again is asked once more, not for ever (the window's camera's
    /// rule). A search on a screen hears its threads on the loop instead.
    fn settle_the_photograph(&mut self, way: &mut dyn Doorway) {
        /// The rounds a settle takes at most.
        const ROUNDS: usize = 8;
        if !self.photographed {
            return;
        }
        for _ in 0..ROUNDS {
            let texts = way.words();
            let mut moved = self.took_what_came_back(way);
            moved |= self.search.inspect_now(&texts);
            if !moved {
                return;
            }
            self.search.look_again(&texts);
            way.repaint();
        }
    }

    /// What the search asked, done in this door — and the view settled for
    /// the frame. SOMETHING OPENED CLOSES THE VIEW (`Out::Done`), and what the
    /// footer is told to open runs after it closed, so it finds the region
    /// free — as a window's order reaches it (`Ui::tell_the_footer`).
    fn carry_out(&mut self, outs: Vec<Out>, way: &mut dyn Doorway) {
        let mut close = false;
        let mut orders: Vec<(&'static str, Option<String>)> = Vec::new();
        for o in outs {
            match o {
                Out::Close | Out::Done => close = true,
                Out::Footer(id, arg) => orders.push((id, arg)),
                Out::Software(attr) => {
                    orders.push(("software-open", Some(format!("search {attr}"))));
                }
                Out::Say(text) => way.say(text, crate::echo::Echo::Normal),
                Out::Ask(line) => way.ask(line),
                Out::Paste => way.paste(),
            }
        }
        if close {
            way.close();
        } else {
            self.search.look_again(&way.words());
            way.repaint();
        }
        for (id, arg) in orders {
            way.run(id, arg);
        }
    }

    /// What a key would be at, on the screen the pointer is on.
    fn area(way: &dyn Doorway) -> Option<Area> {
        way.area().as_ref().map(area_of)
    }
}

impl Region for FastAccessRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// THE SEARCH: blank rows, as many as the region may hold — the view
    /// draws what is typed, its results and its detail over them itself
    /// ([`Region::paint`]), with the layout its pointer reads.
    fn rows(&self, _: &Seen<'_>) -> Vec<Row> {
        Vec::new()
    }

    /// Its keys, the strip's and YouTube's while they act, the consent row's
    /// while one is drawn — `Esc` first.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.search.hint(seen.texts)
    }

    /// THE SEARCH DRAWS ITS OWN ROWS: the region never scrolls, the view
    /// moves its results and its detail itself.
    fn cursor(&self) -> Option<usize> {
        None
    }

    /// A key in the search, read through the reader the window calls too
    /// (`fast_access::press_of_key`), at the place the pointer's screen
    /// gives the region.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(area) = Self::area(way) else {
            return Taken::Yes;
        };
        let Some(press) = crate::fast_access::press_of_key(key) else {
            return Taken::Yes;
        };
        let outs = self.search.key(press, area, &way.words());
        self.carry_out(outs, way);
        self.settle_the_photograph(way);
        Taken::Yes
    }

    /// Its commands: the opening — with words, typed; without, the box as it
    /// is — the window's two, and the answers to the questions its detail and
    /// its settings ask on the minibuffer.
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            "fast-access-open" => {
                let texts = way.words();
                let outs = match argument.map(str::trim).filter(|q| !q.is_empty()) {
                    Some(q) => {
                        self.search.view.set_text("");
                        let errands = self.search.view.typed(q, &self.search.ground, &texts);
                        self.search.errands(errands, &texts)
                    }
                    _ => Vec::new(),
                };
                way.first_page();
                self.carry_out(outs, way);
            }
            // THE REGION'S SEARCH GIVES WAY TO THE WINDOW, typed with what it
            // had: one search, in the door asked for.
            "fast-access-window" | "fast-access-float" => {
                let typed = self.search.view.text().to_owned();
                open_the_window(id, argument, Some(&typed), way);
            }
            id if QUESTIONS.contains(&id) => {
                self.answered_by_name(id, argument.unwrap_or_default(), way);
            }
            _ => return Taken::No,
        }
        self.settle_the_photograph(way);
        Taken::Yes
    }

    /// No row is a list's: a press is the pointer's (`Region::pointer`).
    fn clicked(&mut self, _: usize, _: &mut dyn Doorway) {}

    /// The minibuffer answered a question the search asked.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.answered_by_name(id, text, way);
    }

    /// The clipboard's text, into what is typed.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let texts = way.words();
        let errands = self.search.view.pasted(text, &self.search.ground, &texts);
        let outs = self.search.errands(errands, &texts);
        self.carry_out(outs, way);
        self.settle_the_photograph(way);
    }

    /// The search's threads, its file panel's and its settings file woke the
    /// door: whatever they brought, taken — and a wake that brought nothing
    /// asks nothing of the door.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let _ = self.took_what_came_back(way);
    }

    /// IT SAYS WHAT A WAKE BROUGHT: a frame for an answer that changed what
    /// it draws (`Doorway::repaint`), and nothing for a word of the file's
    /// panel it had no use for.
    fn says_what_a_wake_brought(&self) -> bool {
        true
    }

    /// THE LAUNCHER'S APPLICATIONS, as the footer has them — told at opening:
    /// the in-process ground the applications are searched in.
    fn heard(&mut self, news: &dyn std::any::Any, _: &mut dyn Doorway) {
        if let Some(catalogue) = news.downcast_ref::<crate::launch::Catalogue>() {
            self.search.ground.catalogue = Arc::clone(catalogue);
        }
    }

    /// Blank rows the search draws its own cells over.
    fn shape(&self) -> Shape {
        Shape::Cells
    }

    /// The search over its blank rows: what is typed, the results, the
    /// divider and the detail — the drawing the window paints too, from the
    /// one layout its pointer reads — and its pictures. EVERY HAND'S TARGET,
    /// in its own colour.
    fn paint(&self, easel: &mut crate::region::RegionEasel<'_>) {
        let area = area_of(&easel.area);
        let lit: Vec<(ronoctua_tokens::Color, Target)> = easel
            .lit
            .iter()
            .map(|(c, n)| (*c, Target::of_code(*n)))
            .collect();
        let mut drawing = crate::fast_access_rows::Drawing::default();
        crate::fast_access_rows::draw(
            &self.search.view,
            area,
            true,
            &lit,
            easel.texts,
            &mut drawing,
        );
        easel.under.extend(drawing.under);
        easel.over.extend(drawing.over);
        for track in drawing.tracks {
            easel.bars.record(track);
        }
        easel.spans.extend(drawing.spans);
        easel.pictures.extend(drawing.pictures);
    }

    /// The pointer over the search — its rows, cells, strips, grid, divider
    /// and bars, by the layout its drawing used. A frame that leaves the
    /// region is the footer's own, unless a bar is held.
    fn pointer(
        &mut self,
        events: &[Pointed],
        at: &crate::region::Area,
        way: &mut dyn Doorway,
    ) -> bool {
        let texts = way.words();
        let area = area_of(at);
        let top = f64::from(Area::y_of(area.top));
        self.rests = None;
        let held = self.search.view.grabbed.is_some();
        let inside = |e: &Pointed| {
            !matches!(e.pointing, Pointing::Left)
                && e.at.1 >= top
                && e.at.1 < f64::from(area.bottom_px)
        };
        if !held && !events.iter().all(inside) {
            // THE HOVER GOES WITH THE POINTER, and the footer takes the rest.
            crate::fast_access_rows::hovered(&mut self.search.view, area, None, 0.0, &texts);
            self.rests = Some(None);
            return false;
        }
        let mut outs: Vec<Out> = Vec::new();
        let mut turned: Option<(f32, f32)> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.at.0 as f32, e.at.1 as f32);
            let view = &mut self.search.view;
            match e.pointing {
                Pointing::Moved => {
                    if !crate::fast_access_rows::held(view, area, (px, py), &texts) {
                        let lay = crate::fast_access_rows::Lay::of(view, area, &texts);
                        let target = lay.target_at(view, px, py, &texts);
                        crate::fast_access_rows::hovered(view, area, target, py, &texts);
                        // EACH HAND KEEPS ITS OWN TARGET, as a window's does.
                        self.rests = Some(target.map(Target::code));
                    }
                }
                Pointing::Left => {
                    crate::fast_access_rows::hovered(view, area, None, py, &texts);
                    self.rests = Some(None);
                }
                Pointing::Released(button) if button == crate::door::LEFT_BUTTON => {
                    view.grabbed = None;
                }
                Pointing::Pressed(button) if button == crate::door::LEFT_BUTTON => {
                    let lay = crate::fast_access_rows::Lay::of(view, area, &texts);
                    if let Some(t) = lay.target_at(view, px, py, &texts) {
                        let errands = crate::fast_access_rows::pressed(
                            view,
                            t,
                            area,
                            (px, py),
                            &self.search.ground,
                            &texts,
                        );
                        outs.extend(self.search.errands(errands, &texts));
                    }
                }
                // THE BACK BUTTON IS `Esc` over the region, as everywhere.
                Pointing::Pressed(button) if button == crate::door::BACK_BUTTON => {
                    outs.extend(
                        self.search
                            .key(crate::fast_access::Press::Escape, area, &texts),
                    );
                }
                Pointing::Wheel {
                    vertical,
                    horizontal,
                } => {
                    crate::wheel::wheel_arrived(&mut view.lines_owed, &vertical, &horizontal);
                    turned = Some((px, py));
                }
                Pointing::Pressed(_) | Pointing::Released(_) => {}
            }
        }
        if let Some((x, y)) = turned {
            let by = crate::wheel::wheel_lines(&mut self.search.view.lines_owed);
            if by != 0 {
                crate::fast_access_rows::wheel_at(&mut self.search.view, area, x, y, by, &texts);
            }
        }
        if outs.is_empty() {
            self.search.look_again(&texts);
            way.repaint();
        } else {
            self.carry_out(outs, way);
        }
        self.settle_the_photograph(way);
        true
    }

    fn rests_on(&self) -> Option<Option<usize>> {
        self.rests
    }

    /// What the pointer rests on has to add, whole: a fact of the file's
    /// panels that its column cut, an icon's collection and name.
    fn about_the_pointer(&self, at: &crate::region::Area, seen: &Seen<'_>) -> Option<String> {
        self.search.about(area_of(at), seen.texts)
    }

    /// The view leaves the screen: its threads end with the feed — the
    /// index's reader closes — the film and the shape stop, and the panel's
    /// threads go with the region.
    fn closing(&mut self, _: &mut dyn Doorway) {
        tracing::info!("fast-access closed in the region");
        self.search.let_go();
    }
}

impl FastAccessRegion {
    /// The answer to one of the search's questions, by the id it asked under.
    fn answered_by_name(&mut self, id: &str, text: &str, way: &mut dyn Doorway) {
        let outs = self.search.answered(id, text, &way.words());
        self.carry_out(outs, way);
        self.settle_the_photograph(way);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    fn way(photographing: bool) -> Recorder {
        Recorder::new(
            std::rc::Rc::new(texts()),
            100,
            photographing,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
        .at(Some(crate::region::Area {
            width: 1280.0,
            bottom: 720.0,
            first: 2,
            lines: 20,
            columns: 100,
        }))
    }

    /// The search with nothing behind it — no feed, no panel's threads, its
    /// settings kept nowhere — over one application found.
    fn a_search() -> FastAccessRegion {
        use crate::fast_access::{Filled, Hit, Section, SectionId, What};
        let t = texts();
        let mut view = crate::fast_access::View::new(crate::fast_access::Settings::default(), &t);
        view.set_text("fire");
        view.with_sections(vec![Section {
            id: SectionId::Apps,
            filled: Filled::Hits(vec![Hit {
                id: "app:firefox.desktop".to_owned(),
                title: "Firefox".to_owned(),
                note: None,
                what: What::App {
                    desktop_id: "firefox.desktop".to_owned(),
                    glyph: ronoctua_tokens::icon::GENERIC,
                },
            }]),
        }]);
        let ground = crate::fast_access::Ground {
            catalogue: Arc::default(),
            commands: Vec::new(),
            zone: ronoctua_files::when::Zone::utc(),
            zoneinfo: None,
            queue_keys: String::new(),
        };
        let mut search = Search::new(view, None, ground, crate::inspection::Threads::none());
        search.store = ronoctua_store::Store::at(None, crate::fast_access::FILE);
        let (_, backs) = mpsc::channel();
        let (_, heard) = mpsc::channel();
        FastAccessRegion {
            search,
            backs,
            heard,
            stirred: Arc::new(AtomicBool::new(false)),
            _watch: None,
            rests: None,
            photographed: false,
        }
    }

    /// FAILS IF: the search does not open on its command, or the window's
    /// two commands open the region instead of a window.
    #[test]
    fn what_a_command_does_with_the_search_closed() {
        let mut w = way(false);
        assert_eq!(closed("fast-access-open", None, &mut w), Closed::Opens);
        assert_eq!(
            closed("fast-access-open", Some("2+2"), &mut w),
            Closed::Opens
        );
        assert_eq!(
            closed("fast-access-float", Some("lofi"), &mut w),
            Closed::Taken
        );
        assert_eq!(closed("fast-access-window", None, &mut w), Closed::Taken);
        assert_eq!(
            w.owed,
            vec![
                Effect::OpenWindow(vec![
                    "fast-access".to_owned(),
                    "--float".to_owned(),
                    "lofi".to_owned()
                ]),
                Effect::OpenWindow(vec!["fast-access".to_owned()]),
            ]
        );
        // AN ANSWER FOR A SEARCH NO LONGER UP finds nobody, and is no
        // footer's either.
        assert_eq!(
            closed(crate::fast_access::ASK_BRAVE, Some("k"), &mut w),
            Closed::Taken
        );
        // THE FOOTER'S OWN ORDERS are not the region's.
        for id in [
            "fast-access-launch",
            "fast-access-open-path",
            "fast-access-reveal",
            "fast-access-man",
            "fast-access-column",
        ] {
            assert_eq!(closed(id, Some("x"), &mut w), Closed::NotMine, "{id}");
        }
    }

    /// FAILS IF: a photograph's search closes on its command — nothing of
    /// the region would ever be drawn — or what its feed answered inside the
    /// call is left for a loop a photograph does not have: the frame after
    /// the command would draw the index's sections still searching.
    #[test]
    fn a_photographs_search_answers_inside_the_call() {
        use crate::fast_access::{Filled, SectionId};
        let mut w = way(true);
        assert_eq!(
            closed("fast-access-open", Some("fire"), &mut w),
            Closed::Opens
        );
        // NO INDEX THERE, and no settings of this machine's.
        let nowhere = std::env::temp_dir().join(format!(
            "ronoctua-fast-access-region-no-index-{}",
            std::process::id()
        ));
        let mut r = FastAccessRegion::photographed(
            &w,
            Some(nowhere),
            crate::fast_access::Settings::default(),
        );
        r.search.store = ronoctua_store::Store::at(None, crate::fast_access::FILE);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
        assert_eq!(
            r.command("fast-access-open", Some("fire"), &mut w),
            Taken::Yes
        );
        assert!(!w.owed.contains(&Effect::Close), "{:?}", w.owed);
        assert!(w.owed.contains(&Effect::Repaint), "{:?}", w.owed);
        assert_eq!(r.search.view.text(), "fire");
        // EVERY SECTION OF THE INDEX ANSWERED — refused, with no index there
        // — and nothing left on the channel for a loop.
        let indexed: Vec<&Filled> = r
            .search
            .view
            .sections()
            .iter()
            .filter(|s| matches!(s.id, SectionId::Files | SectionId::Folders))
            .map(|s| &s.filled)
            .collect();
        assert!(!indexed.is_empty(), "no section of the index is drawn");
        assert!(
            indexed.iter().all(|f| matches!(f, Filled::Refused(_))),
            "{indexed:?}"
        );
        assert!(r.backs.try_recv().is_err(), "an answer left for a loop");
    }

    /// FAILS IF: a wake that brought nothing asks anything of the door — the
    /// footer would draw a frame for every empty wake, a thread's second
    /// wake for what the first one took — or one that brought an answer
    /// asks for no frame.
    #[test]
    fn a_wake_asks_only_for_what_it_brought() {
        let (sender, backs) = mpsc::channel();
        let mut r = FastAccessRegion {
            backs,
            ..a_search()
        };
        let mut w = way(false);
        r.woken(&mut w);
        assert!(w.owed.is_empty(), "an empty wake: {:?}", w.owed);
        sender.send(Back::Icons(Err(()))).expect("the region hears");
        r.woken(&mut w);
        assert_eq!(w.owed, vec![Effect::Repaint]);
        // AND THE NEXT WAKE, for what this one already took, asks nothing.
        let mut w = way(false);
        r.woken(&mut w);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
    }

    /// FAILS IF: `RET` on a hit leaves the view open over what it opened, or
    /// the footer is told to open it BEFORE the view closed — what runs would
    /// find the region taken.
    #[test]
    fn enter_closes_the_view_and_then_the_footer_opens_it() {
        let mut r = a_search();
        let mut w = way(false);
        let key = ronoctua_keymap::Key::parse("RET").expect("RET");
        assert_eq!(r.key(key, &mut w), Taken::Yes);
        assert_eq!(
            w.owed,
            vec![
                Effect::Close,
                Effect::Run("fast-access-launch", Some("firefox.desktop".to_owned())),
            ]
        );
    }

    /// FAILS IF: `Esc` leaves the search open.
    #[test]
    fn escape_closes_the_search() {
        let mut r = a_search();
        let mut w = way(false);
        let key = ronoctua_keymap::Key::parse("ESC").expect("ESC");
        let _ = r.key(key, &mut w);
        assert_eq!(w.owed, vec![Effect::Close]);
    }

    /// FAILS IF: the region gives way to the window without what its box
    /// held, or opens the window before it closed.
    #[test]
    fn the_window_takes_what_the_box_held() {
        let mut r = a_search();
        let mut w = way(false);
        assert_eq!(r.command("fast-access-window", None, &mut w), Taken::Yes);
        assert_eq!(
            w.owed,
            vec![
                Effect::Close,
                Effect::OpenWindow(vec!["fast-access".to_owned(), "fire".to_owned()]),
            ]
        );
        // AND WORDS GIVEN are what is typed there.
        let mut r = a_search();
        let mut w = way(false);
        let _ = r.command("fast-access-float", Some("2+2"), &mut w);
        assert_eq!(
            w.owed.last(),
            Some(&Effect::OpenWindow(vec![
                "fast-access".to_owned(),
                "--float".to_owned(),
                "2+2".to_owned()
            ]))
        );
    }

    /// FAILS IF: a pointer frame that leaves the search is taken from the
    /// footer, or does not take the hand's target with it — or one over the
    /// box is not the region's.
    #[test]
    fn the_pointer_is_the_regions_inside_it_and_the_footers_outside() {
        let mut r = a_search();
        let mut w = way(false);
        let at = w.area().expect("an area");
        let above = [Pointed {
            at: (10.0, 1.0),
            pointing: Pointing::Moved,
        }];
        assert!(!r.pointer(&above, &at, &mut w));
        assert_eq!(r.rests_on(), Some(None));
        let top = f64::from(Area::y_of(at.first));
        let over = [Pointed {
            at: (40.0, top + 4.0),
            pointing: Pointing::Moved,
        }];
        assert!(r.pointer(&over, &at, &mut w));
        assert!(
            r.rests_on().is_some(),
            "a motion says what the hand rests on"
        );
        // A PRESS MOVES NO HAND: what it rested on stays.
        let press = [Pointed {
            at: (40.0, top + 4.0),
            pointing: Pointing::Pressed(crate::door::BACK_BUTTON),
        }];
        assert!(r.pointer(&press, &at, &mut w));
        assert_eq!(r.rests_on(), None);
    }

    /// FAILS IF: the launcher's applications the footer told the region at
    /// opening do not reach the ground its applications are searched in.
    #[test]
    fn the_launchers_catalogue_is_the_ground() {
        let mut r = a_search();
        let mut w = way(false);
        let catalogue: crate::launch::Catalogue = Arc::new(vec![ronoctua_apps::DesktopEntry {
            id: "firefox".to_owned(),
            name: "Firefox".to_owned(),
            description: String::new(),
            icon: String::new(),
            categories: Vec::new(),
            mime_types: Vec::new(),
            command: ("firefox".to_owned(), Vec::new()),
            terminal: false,
            working_directory: None,
            startup_wm_class: None,
            key: "firefox".to_owned(),
        }]);
        r.heard(&catalogue, &mut w);
        assert_eq!(r.search.ground.catalogue.len(), 1);
        assert!(w.owed.is_empty());
    }

    /// FAILS IF: the search over its blank rows draws nothing — the box, what
    /// was typed — or its picture rules are not the door's own.
    #[test]
    fn the_search_paints_over_its_rows() {
        let r = a_search();
        assert_eq!(r.shape(), Shape::Cells);
        assert_eq!(r.cursor(), None);
        let t = texts();
        let (mut under, mut over, mut spans) = (Vec::new(), Vec::new(), Vec::new());
        let mut bars = crate::bars::Drawn::default();
        let mut pictures = Vec::new();
        let mut easel = crate::region::RegionEasel {
            area: way(false).area().expect("an area"),
            texts: &t,
            offset: 0,
            under: &mut under,
            over: &mut over,
            spans: &mut spans,
            bars: &mut bars,
            lent: crate::region::Lent::none(),
            lit: &[],
            pictures: &mut pictures,
            placed: &[],
            screen: crate::region::Screen {
                connector: None,
                key: "screen-0".to_owned(),
                scale: 1,
            },
            hand: None,
        };
        r.paint(&mut easel);
        assert!(
            spans.iter().any(|s| s.text.contains("fire")),
            "what is typed is not drawn"
        );
    }
}
