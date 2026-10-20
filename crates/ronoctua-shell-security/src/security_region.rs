//! THE SECURITY CENTRE IN EITHER DOOR: what the view keeps while a door
//! shows it ([`Open`]), the one handler of what it asks a door to do
//! ([`carry_out`]), and the footer's region ([`SecurityRegion`], registered
//! as [`ENTRY`]).
//!
//! # One handler, two doors
//!
//! The footer's region and the window (`crate::security_window`) hold the
//! same [`Open`] and hand every errand of the view to [`carry_out`], over
//! the door they are (`crate::region::Doorway`): a root gesture goes through
//! that door's own sudo gate, a check is said on its line, another view
//! takes over through it (the footer leaves its region first; a window
//! stays). Each door then brings the place asked for into view on its own
//! page.
//!
//! # A page that closes with work in flight
//!
//! A leader command run with the page closed (`s e r`, a check, the
//! vault's key) waits for root in the sudo bar, or for a host, and its
//! answer is said on the footer's line — and a check's answer opens the
//! page on it. So the footer's region comes to life HIDDEN for such a
//! command (`Closed::Behind`), and a page closed with work in flight stays
//! hidden, while it waits ([`Open::lingers`]): the threads it started, a
//! question on the line, a seal the queue has not ended.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::region::{Closed, Doorway, Entry, Inbox, Paging, Post, Region, Row, Seen, Taken};
use crate::security::{Came, Errand, Job, Press, Scene, Section, View};

/// The region's id, as the footer's precedence ranks it.
pub const ID: &str = "security";

/// The words that open the centre's window, after the binary's name.
const WINDOW_WORDS: &str = "security";

/// The command that opens the centre in the footer's region.
const OPEN_COMMAND: &str = "security-open";
/// The command that opens it in a window.
const WINDOW_COMMAND: &str = "security-window";
/// The vault, open, in the file manager's window.
const PRIVATE_SHOW: &str = "private-show";

/// What a thread of the view hands back: what it came with, or `None` when
/// it ended (a door that keeps the view only while it waits hears it).
type Back = Option<Came>;

/// THE CENTRE WHILE A DOOR SHOWS IT — or, in the footer, while it still
/// waits for something with its page closed.
pub struct Open {
    /// Where the cursor is, what the page asks, what is in flight.
    pub view: View,
    /// What it draws, as its thread last read it.
    pub scene: Scene,
    /// The zone its dates are drawn in.
    zone: ronoctua_files::when::Zone,
    /// Whether the page is on the screen — a window's always is.
    shown: bool,
    /// What its threads hand back; `None`: a photograph's door, which
    /// starts none.
    inbox: Option<Inbox<Back>>,
    /// The folders it watches, while it is shown.
    watches: Vec<ronoctua_files::notice::Notice>,
    /// How many of its threads are still running.
    running: Arc<AtomicUsize>,
    /// The question it asked on the door's line, until it is answered or
    /// let go.
    asking: Option<&'static str>,
    /// The watch's page as the brain last said it — `None` before the first
    /// mind, which is the baseline.
    watch_seen: Option<Option<u64>>,
}

impl std::fmt::Debug for Open {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Open")
            .field("view", &self.view)
            .field("shown", &self.shown)
            .finish_non_exhaustive()
    }
}

/// One thread of the view's, counted while it runs: its end is counted
/// down and told to the door's loop.
struct Running {
    count: Arc<AtomicUsize>,
    post: Post<Back>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
        let _ = self.post.send(None);
    }
}

impl Open {
    /// The centre over `scene` (an unread one until its thread reads it),
    /// its dates in `zone` — `shown` already (a window) or not yet (the
    /// footer's region, until the footer shows it). With a `wake`, its
    /// threads hand back through it; without one, none is started.
    #[must_use]
    pub fn new(
        view: View,
        scene: Scene,
        zone: ronoctua_files::when::Zone,
        shown: bool,
        wake: Option<crate::wake::Wake>,
    ) -> Self {
        Self {
            view,
            scene,
            zone,
            shown,
            inbox: wake.map(Inbox::new),
            watches: Vec::new(),
            running: Arc::new(AtomicUsize::new(0)),
            asking: None,
            watch_seen: None,
        }
    }

    /// Where a thread of the door's own posts what it came with (the
    /// window's watch of the brain and of the queue).
    #[must_use]
    pub fn post(&self) -> Option<Post<Back>> {
        self.inbox.as_ref().map(Inbox::post)
    }

    /// Starts `job` on a thread, counted until it ends; what it comes with
    /// is handed back through the door's wake. A photograph's starts nothing.
    pub fn start(&self, job: Job) {
        let Some(post) = self.post() else {
            return;
        };
        self.running.fetch_add(1, Ordering::SeqCst);
        let running = Running {
            count: Arc::clone(&self.running),
            post: post.clone(),
        };
        crate::security::start(job, move |came| {
            let _alive = &running;
            let _ = post.send(Some(came));
        });
    }

    /// Reads again what moves in the scene the page shows. With the page
    /// closed there is nothing to keep fresh: a leader command reads the
    /// file itself (`security::settings_now`).
    pub fn refresh(&self) {
        if self.shown {
            self.start(Job::Refresh(Box::new(self.scene.clone())));
        }
    }

    /// The page is shown: its scene read on a thread, and the folders whose
    /// change moves it watched — a photograph's reads the scene its camera's
    /// environment names, and watches nothing.
    pub fn showing(&mut self, photographing: bool) {
        self.shown = true;
        if photographing {
            if let Some(scene) = Scene::of_the_footer_camera() {
                self.scene = scene;
            }
            return;
        }
        self.start(Job::Read);
        if let Some(post) = self.post() {
            // A FOLDER THAT MOVED is a stir, and what moves is read again —
            // never the slow facts (`Scene::refreshed`).
            self.watches = crate::security::watches(ronoctua_xdg::state_dir(), move || {
                post.send(Some(Came::Stirred))
            });
        }
    }

    /// The page closed: its watches go, and its scene with them — a leader
    /// command run later builds its change over the file as it is THEN
    /// (`security::settings_now`), never over a copy another door may have
    /// changed since; a registration waiting for its touch goes too.
    pub fn hidden(&mut self) {
        self.view.closed();
        self.shown = false;
        self.watches.clear();
        self.scene = Scene::default();
    }

    /// Whether it still waits for something with its page closed: a thread
    /// of its own, a question on the line, the view's work in flight.
    #[must_use]
    pub fn lingers(&self) -> bool {
        self.running.load(Ordering::SeqCst) > 0 || self.asking.is_some() || self.view.waits()
    }

    /// Takes what its threads handed back — each carried out as it came —
    /// and whether anything did.
    pub fn woken(&mut self, way: &mut dyn Doorway) -> bool {
        let came = self.inbox.as_ref().map(Inbox::take).unwrap_or_default();
        let any = !came.is_empty();
        for back in came.into_iter().flatten() {
            self.came(back, way);
        }
        any
    }

    /// What came back from one of its threads, in the door `way` is.
    pub fn came(&mut self, came: Came, way: &mut dyn Doorway) {
        // A SCENE FOR A PAGE THAT CLOSED is dropped: kept, it would be the
        // stale copy a leader command builds its change over.
        if !self.shown && matches!(came, Came::Scene(_) | Came::Stirred) {
            return;
        }
        // A STEP THAT SAYS SOMETHING is the vault's touch, asked once its
        // unit started: HELD — true while the unit waits for the key, with
        // the page open or not — until the work's answer replaces it.
        let waits = matches!(came, Came::Step(_));
        let out = self.view.came(came, &mut self.scene, way.texts());
        if let Some(said) = out.say {
            if waits {
                way.hold(said);
            } else {
                way.say(said, crate::echo::Echo::Long);
            }
        }
        if out.read {
            self.refresh();
        }
        // WHAT CAME IS DRAWN ON THE PAGE — the upload's question, a check's
        // answer — and a leader command ran it with the page closed: the
        // page asks to be shown (or the footer says why its region is not
        // free).
        if out.show && !self.shown {
            way.show();
        }
        carry_out(out.errand, self, way);
    }

    /// The cursor on a row that acts — and, when somebody asked for a place
    /// (a section's word, a check's answer, a question), the row that place
    /// starts on, for the door to bring to the top of its page. A scene that
    /// arrives on its own never moves the page.
    pub fn settle_on(&mut self, columns: u16, texts: &ronoctua_i18n::Texts) -> Option<usize> {
        if !self.shown {
            return None;
        }
        let rows = self.rows(columns, texts);
        self.view.settle_on(&rows, &self.scene)
    }

    /// The rows, at `columns` in `texts`.
    #[must_use]
    pub fn rows(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<crate::security::Row> {
        self.view.rows(&self.scene, columns, texts, &self.zone)
    }

    /// What a key asks.
    pub fn press(&mut self, press: Press, columns: u16, texts: &ronoctua_i18n::Texts) -> Errand {
        self.view
            .press(press, &self.scene, columns, texts, &self.zone)
    }

    /// A question of the centre's by its name: asked when it came with no
    /// argument (a check from the leader), answered when it came with one.
    pub fn answer(
        &mut self,
        id: &str,
        argument: Option<&str>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let asked = |id| Errand::Ask {
            id,
            secret: id == crate::security::CHECK_PASSWORD,
            initial: None,
        };
        match (id, argument) {
            (crate::security::CHECK_PASSWORD, None) => asked(crate::security::CHECK_PASSWORD),
            (crate::security::CHECK_EMAIL, None) => asked(crate::security::CHECK_EMAIL),
            (crate::security::CHECK_FILE, None) => asked(crate::security::CHECK_FILE),
            (_, None) => Errand::Nothing,
            (_, Some(text)) => self.answered(id, text, texts),
        }
    }

    /// What was typed on the door's line for the question `id`.
    pub fn answered(&mut self, id: &str, text: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        self.asking = None;
        self.view.answered(id, text, &self.scene, texts)
    }

    /// The question on the door's line was let go.
    pub const fn let_go(&mut self) {
        self.asking = None;
    }
}

/// CARRIES OUT what the centre asked for, in the door `way` is — the ONE
/// handler both doors call, for a key of the page and a command of the
/// leader alike. The page each door then brings into view is its own
/// ([`Open::settle_on`]).
pub fn carry_out(errand: Errand, open: &mut Open, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => way.close(),
        Errand::Ask {
            id,
            secret,
            initial,
        } => {
            open.asking = Some(id);
            way.ask(crate::line::Line::new(id, secret, initial.as_deref()));
        }
        // ROOT, ASKED IN THE SUDO BAR above the footer, through this door's
        // gate: the verdict comes back to this door.
        Errand::Door(work) => {
            open.view.sent(&work);
            let gesture = way.sudo().gesture(crate::sudo::Asker::Security);
            open.start(Job::Door(work, gesture));
        }
        // A KEY'S REGISTRATION, as the person, on a thread: its cancel comes
        // back to the view.
        Errand::Register => open.start(Job::Register),
        Errand::Shields => open.start(Job::Shields),
        Errand::Hash(path) => {
            way.say(
                crate::security::checking(way.texts()),
                crate::echo::Echo::Brief,
            );
            open.view.check_started();
            open.start(Job::Hash(path));
        }
        Errand::Upload { path, sha256 } => {
            let said = crate::security::queue_upload(path, sha256, way.texts());
            way.say(said, crate::echo::Echo::Long);
        }
        Errand::Email(address) => {
            way.say(
                crate::security::checking(way.texts()),
                crate::echo::Echo::Brief,
            );
            open.view.check_started();
            open.start(Job::Email(address));
        }
        Errand::Password(secret) => {
            way.say(
                crate::security::checking(way.texts()),
                crate::echo::Echo::Brief,
            );
            open.view.check_started();
            open.start(Job::Password(secret));
        }
        Errand::KeepKey(secret) => {
            let said = crate::security::keep_key(&secret, way.texts());
            way.say(said, crate::echo::Echo::Long);
            open.refresh();
        }
        Errand::ForgetKey => {
            let said = crate::security::forget_key(way.texts());
            way.say(said, crate::echo::Echo::Long);
            open.refresh();
        }
        // ANOTHER VIEW TAKES OVER: the footer leaves the page first, as
        // `Esc` leaves it, so its region is free for the view the row names;
        // a window stays, and the footer opens that view.
        Errand::Command(id) => way.give_way(id, id),
        Errand::Show(section) => {
            open.view.open_on(section);
            way.show();
        }
        Errand::Url(url) => match option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) {
            Some(opener) => way.launch(crate::door::Launching::Program {
                program: opener.to_owned(),
                args: vec![url],
                cwd: None,
            }),
            None => tracing::error!("RONOCTUA_XDG_OPEN was not set at build time"),
        },
        Errand::Say(said) => way.say(said, crate::echo::Echo::Long),
        // A SEAL, to the queue on a thread — what it answered comes back
        // (`Came::SealHanded`) and is said in this door.
        Errand::Seal(doing) => open.start(Job::Seal(doing)),
        Errand::Sniff(path) => open.start(Job::Sniff(path)),
        // THE CLIPBOARD, off the loop, for the file picker over the page.
        Errand::Paste => way.paste(),
    }
    way.repaint();
}

/// THE SECURITY CENTRE IN THE FOOTER'S REGION (`LEADER s e e`) — and, while
/// it waits for something with its page closed, hidden.
pub struct SecurityRegion {
    open: Open,
    /// Moves with every change of the view — what [`SecurityRegion::drawn`]
    /// is kept against.
    generation: u64,
    /// The rows last built, at their width and generation: the footer asks
    /// each row what it does (`Region::about`) right after the rows.
    drawn: RefCell<Option<Drawn>>,
    /// Whether the footer's last second found the passwordless window's
    /// count moving on the shown page — so the second it stops is drawn
    /// once more ([`Region::tick`]).
    counting: bool,
}

/// Rows built at a width (`.0`) for a generation of the view (`.1`).
type Drawn = (u16, u64, Rc<Vec<crate::security::Row>>);

thread_local! {
    /// THE CENTRE BETWEEN TWO OPENINGS of the footer's region. The footer
    /// always kept one view of the centre, from its first opening to its
    /// last — the row the cursor was on, the last answers of the online
    /// checks, the finger the ring offers, the page's own question — and a
    /// region lives only while it is open or waits for something. So the
    /// view a region leaves with is kept here ([`SecurityRegion`]'s `Drop`)
    /// and the next opening takes it ([`open`]). A `thread_local` because
    /// `Entry::open` is a function pointer with no state of its own, and the
    /// footer's loop is one thread (each test, its own). What it draws — the
    /// scene, the watches — is not kept: a page closed reads its scene again
    /// when it is shown ([`Open::hidden`]).
    static KEPT: RefCell<Option<View>> = const { RefCell::new(None) };
}

impl Drop for SecurityRegion {
    /// THE VIEW KEPT for the next opening — unless the thread is ending and
    /// its keeping place is gone already, when there is no next opening.
    fn drop(&mut self) {
        let view = std::mem::take(&mut self.open.view);
        let _ = KEPT.try_with(|kept| {
            if let Ok(mut kept) = kept.try_borrow_mut() {
                *kept = Some(view);
            }
        });
    }
}

impl SecurityRegion {
    /// The centre, to be changed — and the rows kept for it no longer its own.
    const fn open(&mut self) -> &mut Open {
        self.generation = self.generation.wrapping_add(1);
        &mut self.open
    }

    /// The rows at `seen`'s width, built now and kept for the questions that
    /// follow them.
    fn rows_at(&self, seen: &Seen<'_>) -> Rc<Vec<crate::security::Row>> {
        if let Some((columns, generation, rows)) = self.drawn.borrow().as_ref()
            && *columns == seen.columns
            && *generation == self.generation
        {
            return Rc::clone(rows);
        }
        let rows = Rc::new(self.open.rows(seen.columns, seen.texts));
        *self.drawn.borrow_mut() = Some((seen.columns, self.generation, Rc::clone(&rows)));
        rows
    }

    /// The place asked for, brought to the top of the footer's page.
    fn settled(&mut self, way: &mut dyn Doorway) {
        let columns = way.columns();
        if let Some(top) = self.open().settle_on(columns, way.texts()) {
            way.page(Paging::Top(top));
        }
    }

    /// An errand of the view's, carried out, and its place settled.
    fn carry(&mut self, errand: Errand, way: &mut dyn Doorway) {
        carry_out(errand, self.open(), way);
        self.settled(way);
    }

    /// A key of the centre — from the keyboard, the hint or a second click.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let columns = way.columns();
        let errand = self.open().press(press, columns, way.texts());
        self.carry(errand, way);
    }

    /// The page shown on `section` (or where it is): asked of the footer
    /// when it is hidden.
    fn show_on(&mut self, section: Option<Section>, way: &mut dyn Doorway) {
        if let Some(s) = section {
            self.open().view.open_on(s);
        }
        if !self.open.shown {
            way.show();
        }
        self.settled(way);
    }
}

impl Region for SecurityRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// Each row a click that selects and a second that acts — a heading
    /// none.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        // FRESH ON EVERY FRAME: the count of the passwordless window moves
        // with no change of the view's.
        *self.drawn.borrow_mut() = None;
        self.rows_at(seen)
            .iter()
            .map(|r| Row {
                fields: r.fields.clone(),
                takes: r.takes(),
            })
            .collect()
    }

    /// Its keys, generated from its table — the file picker's while it is
    /// up over the page.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open.view.hint_now(&self.open.scene, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.open.view.cursor())
    }

    /// What its `RET` does, said under the pointer.
    fn about(&self, row: usize, seen: &Seen<'_>) -> Option<String> {
        View::about(&self.rows_at(seen), row, seen.texts)
    }

    /// Read by the whole notation through the reader the window calls too —
    /// the file picker's, while it is up over the page, takes what it
    /// reads (`security::View::press_of`).
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match self.open.view.press_of(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// Its doors, the leader's toggles and checks, the vault's two keys,
    /// encrypt anything, the view's keys by name and the answers to its
    /// questions — every one through [`carry_out`], the one path a key in
    /// the view takes too.
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        use crate::security::{CHECK_EMAIL, CHECK_FILE, CHECK_PASSWORD, HOME, RULE};
        let errand = match id {
            OPEN_COMMAND => {
                self.show_on(argument.and_then(Section::of_word), way);
                return Taken::Yes;
            }
            WINDOW_COMMAND => {
                way.open_window(window_words(argument));
                return Taken::Yes;
            }
            PRIVATE_SHOW => {
                way.open_window(private_words());
                return Taken::Yes;
            }
            // ENCRYPT ANYTHING: the centre on its section, the file picker
            // up for what to seal — then "for whom" (`crate::seal`).
            "security-encrypt" => {
                self.open().view.encrypt_now();
                self.show_on(Some(Section::Encrypt), way);
                return Taken::Yes;
            }
            // THE POST-QUANTUM SECTION: the centre, opened on it.
            "security-quantum" => {
                self.show_on(Some(Section::Quantum), way);
                return Taken::Yes;
            }
            "security-relax" => crate::security::relax_errand(&self.open.scene),
            "security-shields" => crate::security::shields_errand(&self.open.scene),
            "security-firewall" => crate::security::firewall_errand(&self.open.scene, way.texts()),
            // THE WATCH'S EVENT: "a network came up, treated as public —
            // treat it as home?", from the minibuffer's line or by name.
            HOME => crate::security::home_errand(&self.open.scene, way.texts()),
            "security-vt-forget" => Errand::ForgetKey,
            CHECK_PASSWORD
            | CHECK_EMAIL
            | CHECK_FILE
            | RULE
            | crate::security_words::VT_KEY
            | crate::vault::PASSPHRASE
            | crate::vault::NEW_PASSPHRASE
            | crate::vault::REPEAT_PASSPHRASE
            | crate::vault::CAP => {
                let texts = way.texts();
                let errand = self.open.answer(id, argument, texts);
                self.generation = self.generation.wrapping_add(1);
                errand
            }
            // THE /private VAULT: its two keys, the page open or not.
            // Opening by the key, "touch your key" is said when the unit
            // starts — past the password question sudo may ask first.
            "private-open" => {
                let errand = self.open.view.private_open(&self.open.scene, way.texts());
                self.generation = self.generation.wrapping_add(1);
                errand
            }
            "private-lock" => {
                let errand = self.open.view.private_lock(&self.open.scene, way.texts());
                self.generation = self.generation.wrapping_add(1);
                errand
            }
            other => match crate::security::press_of(other) {
                Some(press) if other.starts_with("security-") => {
                    self.press(press, way);
                    return Taken::Yes;
                }
                _ => return Taken::No,
            },
        };
        self.carry(errand, way);
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — through the press a key
    /// takes.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self.open.rows(way.columns(), way.texts());
        if self.open().view.clicked(row, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let errand = self.open().answered(id, text, way.texts());
        self.carry(errand, way);
    }

    /// Every question of the page goes with the line — a rule being edited,
    /// the vault's questions, a seal's: an answer typed later under the same
    /// command, by name, must not be taken for them.
    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.open().let_go();
        self.open().view.let_go();
        way.repaint();
    }

    /// The clipboard's text, for the file picker up over the page.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let errand = self.open().view.picker_pasted(text, way.texts());
        self.carry(errand, way);
        way.page(Paging::Follow);
    }

    fn woken(&mut self, way: &mut dyn Doorway) {
        if self.open().woken(way) {
            self.settled(way);
        }
    }

    /// The brain's mind — its watch's page moved, and the lists are read
    /// again while the page is on screen — and the end of a seal or an open
    /// this page sent: said here, a wrong passphrase asked again while the
    /// page is up, said rather than asked while it is not (the answer's line
    /// is the page's).
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(mind) = news.downcast_ref::<ronoctua_brain::Mind>() {
            let now = crate::security::watch_generation(mind);
            let moved = self.open.watch_seen.is_some_and(|seen| seen != now);
            self.open.watch_seen = Some(now);
            if moved {
                self.open.refresh();
            }
            return;
        }
        if let Some(report) = news.downcast_ref::<crate::report::SealReport>() {
            if self.open.shown {
                self.open().came(Came::Sealed(report.clone()), way);
                self.settled(way);
            } else if let Some(said) = self.open().view.seal_ended_unseen(report, way.texts()) {
                way.say(said, crate::echo::Echo::Long);
            }
        }
    }

    /// THE PASSWORDLESS WINDOW'S LIVE COUNT, on the footer's second: a frame
    /// while the page is shown and a window with an end is open — and ONE
    /// MORE on the second the window ends under the shown page, so the page
    /// says it closed instead of keeping its last count ("1 s"), as the
    /// window's own count does (`security_window`). Nothing watched changes
    /// at that moment to draw it otherwise.
    fn tick(&mut self, _: std::time::Instant, _: &mut dyn Doorway) -> bool {
        let counting = self.open.shown && View::counts_down(&self.open.scene);
        let was = std::mem::replace(&mut self.counting, counting);
        counting || (was && self.open.shown)
    }

    /// Its scene read, its folders watched — and the cursor on a row that
    /// acts, the place asked for (while it was hidden, too) brought into view.
    fn shown(&mut self, way: &mut dyn Doorway) {
        self.open().showing(way.photographing());
        self.settled(way);
    }

    fn closing(&mut self, _: &mut dyn Doorway) {
        self.open().hidden();
    }

    fn lingers(&self) -> bool {
        self.open.lingers()
    }
}

/// The window's words: the binary's `security`, then the section asked for.
fn window_words(argument: Option<&str>) -> Vec<String> {
    let mut args = vec![WINDOW_WORDS.to_owned()];
    args.extend(argument.map(ToOwned::to_owned));
    args
}

/// The open vault, in the file manager's window — never listed by the
/// footer's loop (a suspended vault would freeze it).
fn private_words() -> Vec<String> {
    vec!["files".to_owned(), ronoctua_private::MOUNT_POINT.to_owned()]
}

/// What a command does to the centre while its region is closed: its two
/// windows open on their own; the page's commands open it; and a command
/// that acts for the view WITHOUT its page — a toggle of the leader's, a
/// check, the vault's two keys, a key of the view's by name — brings the
/// region to life hidden, where it waits for its answer.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    use crate::security::{CHECK_EMAIL, CHECK_FILE, CHECK_PASSWORD, HOME, RULE};
    match command {
        WINDOW_COMMAND => {
            way.open_window(window_words(argument));
            Closed::Taken
        }
        PRIVATE_SHOW => {
            way.open_window(private_words());
            Closed::Taken
        }
        OPEN_COMMAND | "security-encrypt" | "security-quantum" => Closed::Opens,
        "security-relax"
        | "security-shields"
        | "security-firewall"
        | HOME
        | "security-vt-forget"
        | CHECK_PASSWORD
        | CHECK_EMAIL
        | CHECK_FILE
        | RULE
        | crate::security_words::VT_KEY
        | "private-open"
        | "private-lock"
        | crate::vault::PASSPHRASE
        | crate::vault::NEW_PASSPHRASE
        | crate::vault::REPEAT_PASSPHRASE
        | crate::vault::CAP => Closed::Behind,
        other if other.starts_with("security-") && crate::security::press_of(other).is_some() => {
            Closed::Behind
        }
        _ => Closed::NotMine,
    }
}

/// The centre, for the footer's slot host — over the view the last opening
/// left with ([`KEPT`]), not shown yet: the footer shows it
/// (`Region::shown`, which reads its scene and watches its folders) or keeps
/// it hidden. A photograph's footer starts no thread.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let wake = (!way.photographing()).then(|| way.wake());
    // THE VIEW THE LAST OPENING LEFT WITH, when there was one.
    let view = KEPT
        .with(|kept| kept.borrow_mut().take())
        .unwrap_or_else(View::fresh);
    Box::new(SecurityRegion {
        open: Open::new(
            view,
            Scene::default(),
            ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            false,
            wake,
        ),
        generation: 0,
        drawn: RefCell::new(None),
        counting: false,
    })
}

/// The security centre, as the binary registers it in the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::Effect;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    fn way() -> crate::region::Recorder {
        crate::region::Recorder::new(
            Rc::new(texts()),
            100,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn region(scene: Scene) -> SecurityRegion {
        SecurityRegion {
            open: Open::new(
                View::fresh(),
                scene,
                ronoctua_files::when::Zone::utc(),
                false,
                None,
            ),
            generation: 0,
            drawn: RefCell::new(None),
            counting: false,
        }
    }

    /// FAILS IF: the leader's commands that act WITHOUT the page stop
    /// bringing the region to life hidden — so a root gesture asked from
    /// the leader would open the page over whatever is there, or never be
    /// heard back — or the page's own commands stop opening it, or a
    /// command that is not the centre's is taken.
    #[test]
    fn what_a_command_does_to_the_closed_centre() {
        for id in [
            "security-relax",
            "security-shields",
            crate::security::CHECK_PASSWORD,
            "private-open",
            crate::vault::PASSPHRASE,
        ] {
            assert_eq!(closed(id, None, &mut way()), Closed::Behind, "{id}");
        }
        for id in [OPEN_COMMAND, "security-encrypt", "security-quantum"] {
            assert_eq!(closed(id, None, &mut way()), Closed::Opens, "{id}");
        }
        let mut w = way();
        assert_eq!(closed(WINDOW_COMMAND, Some("admin"), &mut w), Closed::Taken);
        assert_eq!(
            w.owed,
            vec![Effect::OpenWindow(vec![
                "security".to_owned(),
                "admin".to_owned()
            ])]
        );
        assert_eq!(closed("stick-open", None, &mut way()), Closed::NotMine);
        assert_eq!(
            closed(crate::seal::QUESTIONS[0], None, &mut way()),
            Closed::NotMine,
            "a seal's question is the asker's, never the closed centre's"
        );
    }

    /// FAILS IF: a check asked from the leader with the page closed is not
    /// asked on the footer's line, or the hidden region lets go of it while
    /// the question is up — its answer would reach nobody.
    #[test]
    fn a_check_from_the_leader_is_asked_and_waited_for() {
        let mut r = region(Scene::default());
        let mut w = way();
        assert!(r.command(crate::security::CHECK_EMAIL, None, &mut w).took());
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Ask(line) if line.id == crate::security::CHECK_EMAIL)),
            "{:?}",
            w.owed
        );
        assert!(r.lingers(), "the hidden centre let go of its question");
        r.unanswered(crate::security::CHECK_EMAIL, &mut way());
        assert!(!r.lingers(), "a question let go still holds the centre");
    }

    /// FAILS IF: what came back with the page closed is drawn over a scene
    /// that closed, or a check's answer that the page has to be seen for
    /// does not ask for the page.
    #[test]
    fn a_closed_page_drops_its_scene_and_asks_to_be_seen() {
        let mut r = region(Scene::default());
        let mut w = way();
        r.open().came(
            Came::Scene(Box::new(Scene {
                read: true,
                ..Scene::default()
            })),
            &mut w,
        );
        assert!(!r.open.scene.read, "a scene for a closed page was kept");
        r.open().came(Came::Password(Ok(3)), &mut w);
        assert!(w.owed.contains(&Effect::Show), "{:?}", w.owed);
    }

    /// FAILS IF: the view a region of the centre leaves with is not the one
    /// the next opening gets — the footer kept ONE view of the centre from
    /// its first opening to its last (the cursor's row, the last answers of
    /// the checks) — or the centre is drawn over a kept scene.
    #[test]
    fn the_next_opening_gets_the_view_the_last_one_left_with() {
        let mut first = open(OPEN_COMMAND, None, &mut way());
        assert!(!first.lingers(), "a fresh centre waits for something");
        let mut w = way();
        assert!(
            first
                .command(crate::security::CHECK_EMAIL, None, &mut w)
                .took()
        );
        first.unanswered(crate::security::CHECK_EMAIL, &mut way());
        assert!(!first.lingers());
        let mut r = region(Scene {
            read: true,
            ..Scene::default()
        });
        r.open.view.check_started();
        drop(first);
        drop(r);
        let again = open(OPEN_COMMAND, None, &mut way());
        assert!(
            again.lingers(),
            "the view the last region left with was not the next one's"
        );
        let seen = KEPT.with(|kept| kept.borrow().is_some());
        assert!(!seen, "the kept view was handed out and kept as well");
        drop(again);
        let third = open(OPEN_COMMAND, None, &mut way());
        assert!(third.lingers(), "a view kept once was not kept again");
    }

    /// FAILS IF: the footer's second stops drawing the page once more when
    /// the passwordless window ends under it — the page would keep its last
    /// count ("1 s") until something else drew it, where the window's own
    /// count draws the end — or it draws a hidden page, or keeps drawing
    /// after the end.
    #[test]
    fn the_second_the_window_ends_is_drawn_once_more() {
        use ronoctua_security::relax::Opened;
        use ronoctua_security::verb::RelaxSpan;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let ending = |until: i64| Scene {
            read: true,
            window: Some(Opened {
                until_unix_s: Some(until),
                ..Opened::of(RelaxSpan::Fifteen, now)
            }),
            ..Scene::default()
        };
        let at = std::time::Instant::now();
        let mut r = region(ending(now + 600));
        assert!(!r.tick(at, &mut way()), "a hidden page was drawn");
        r.open.shown = true;
        assert!(r.tick(at, &mut way()), "the count does not move");
        assert!(r.tick(at, &mut way()), "the count stopped while it runs");
        r.open.scene = ending(now - 1);
        assert!(r.tick(at, &mut way()), "the end was not drawn once more");
        assert!(!r.tick(at, &mut way()), "the page is drawn after the end");
        // AN END WITH THE PAGE CLOSED draws nothing.
        r.open.scene = ending(now + 600);
        assert!(r.tick(at, &mut way()));
        r.open.shown = false;
        r.open.scene = ending(now - 1);
        assert!(
            !r.tick(at, &mut way()),
            "a closed page was drawn at the end"
        );
    }

    /// FAILS IF: the region and the window stop handing a root gesture to
    /// their own door's gate through the one handler — the work recorded as
    /// in flight, so the page says it waits.
    #[test]
    fn a_root_gesture_goes_through_the_doors_gate() {
        let mut r = region(Scene {
            read: true,
            ..Scene::default()
        });
        let mut w = way();
        carry_out(
            Errand::Door(crate::security::Work::RelaxEnd),
            r.open(),
            &mut w,
        );
        assert_eq!(
            r.open.view.in_flight(),
            Some(crate::security::Work::RelaxEnd)
        );
        assert!(r.lingers(), "a work in flight does not keep the centre");
        assert!(w.owed.contains(&Effect::Repaint));
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, the air between sections, a note,
    /// a fact, the lines a pair wrapped onto — or leaves unlit one it takes.
    /// THE LIGHT IS THE CLICK: every row clicked on a view of its own, and
    /// lit exactly where something was seen to happen.
    ///
    /// ONCE, AND NEVER ON THE CURSOR'S ROW (as `crate::region::held_to_the_click`
    /// clicks): a second click would be the row's `RET`, and a `RET` on
    /// this page forgets the person's VirusTotal key, queues an upload, opens
    /// the file picker over the home — what a test must not reach. One click
    /// is enough for the light: from any other row it selects, and the cursor
    /// moving is seen; on the cursor's row it is already the `RET`, which is
    /// why that row is not clicked.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        // THE RECORDER'S WIDTH, which is what a click builds its rows at.
        let seen = Seen {
            texts: &t,
            columns: 100,
            lines: 60,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        // SHOWN AND SETTLED, as the footer draws it: the cursor on the first
        // row that acts, which is the one a first click is the `RET` on.
        let fresh = || {
            let mut r = region(Scene {
                read: true,
                settings: Some(ronoctua_security::firewall::Settings::default()),
                ..Scene::default()
            });
            r.open.shown = true;
            let _ = r.open.settle_on(seen.columns, &t);
            (r, way())
        };
        let first = fresh().0;
        let cursor = first.cursor_in(&seen);
        let held: Vec<crate::region::Held> = first
            .rows(&seen)
            .iter()
            .enumerate()
            .map(|(row, drawn)| {
                let here = cursor == Some(row);
                let (mut r, mut w) = fresh();
                let before = crate::region::seen(&r, &w.owed, &seen);
                if !here {
                    r.clicked(row, &mut w);
                }
                crate::region::Held {
                    row,
                    lit: drawn.takes,
                    did: crate::region::seen(&r, &w.owed, &seen) != before,
                    here,
                }
            })
            .collect();
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        assert!(
            held.iter().any(|h| h.here && h.lit),
            "the cursor stands on no row a click takes: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
