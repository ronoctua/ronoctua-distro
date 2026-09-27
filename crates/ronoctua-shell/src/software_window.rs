//! The person's own programs, in a window of its own — the second door onto
//! `crate::software`.
//!
//! # What is here and what is not
//!
//! The plumbing of a window is `crate::door`'s; what is here is the view, the
//! threads that read for it, the pictures, and the one file of its own this
//! window follows. It DECIDES nothing — which rows exist and what a key does
//! to them is `software.rs`, and both doors ask the same view the same
//! questions. The view does not know which door it is in.
//!
//! # Its own readings, in its own process
//!
//! The footer's feed lives with the footer's view and dies with it; this
//! window is another process, and it reads for itself: a [`Feed`] of its own
//! for the machine, the searches and a sheet's details, a [`Pictures`] pool
//! for the icons, the queue's list for the jobs it gave, and `software.json`
//! watched, because both doors write it.
//!
//! # It does what the footer does (the owner's rule)
//!
//! **It types.** A letter, `DEL` and `C-v` go to the same view calls the
//! footer makes (`letter`, `erase`, `pasted`). A question — a new web app's
//! address, the draft's name and address, an image's path, a file to
//! install — is asked on this window's header (`door::Ui::ask`), and the
//! answer goes to the same `View::typed` the footer's minibuffer reaches. The
//! clipboard is read off the loop (`crate::pasting`): its text into the
//! search or the line, its image into the icon.
//!
//! **It opens.** A web app once, a page, the file manager and the brain's
//! page start in a scope of their own, by the path every door launches by
//! (`Ui::launch`, over `foreign::start_in_scope`) — and so does an installed
//! program, found in the launcher's catalogue off the loop and started as
//! the footer starts it (`Ui::launch_entry`, `apps::argv_of`). What went
//! wrong — no such entry yet, a start that failed — is said on this window's
//! header, never on the footer's line.
//!
//! An order goes to the queue through the same client, a preference is
//! written through the same locked change, and a file dropped on it is taken
//! through `wl_data_device` (`crate::carry`), the one addition a window may
//! make to its door.

use crate::door::{Canvas, Content, Door, Ui};
use crate::software::{Errand, Opening, Press, View};
use crate::software_feed::{Feed, Kept, Readings, Taught, Want};
use crate::software_pictures::Pictures;
use anyhow::Result;
use smithay_client_toolkit::data_device_manager::{
    WritePipe,
    data_device::{DataDeviceData, DataDeviceHandler},
    data_offer::{DataOfferHandler, DragOffer},
    data_source::DataSourceHandler,
};
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    protocol::{
        wl_data_device::WlDataDevice, wl_data_device_manager::DndAction,
        wl_data_source::WlDataSource, wl_seat::WlSeat, wl_surface::WlSurface,
    },
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.software";

/// The levels `ronoctua-shell software <word>` opens on, by their word — the
/// command line's, so English.
pub const LEVEL_WORDS: [&str; 5] = ["home", "trials", "maintenance", "android", "advanced"];

/// The word before a search's own words: `software search <words>` opens on
/// what a search for them finds — what typing them into the window would,
/// with the words given on the command line.
pub const SEARCH_WORD: &str = "search";

/// The word before an address: `software web <address>` opens a new web app's
/// editor on it — what an address typed into the search offers, with the
/// address already given.
pub const WEB_WORD: &str = "web";

/// The opening the words after `software` name, or `None` for words that
/// name none.
#[must_use]
pub fn opening_of(said: &str) -> Option<Opening> {
    let said = said.trim();
    // WHAT FOLLOWS ONE OF THE TWO WORDS, when something does.
    let after = |word: &str| {
        said.strip_prefix(word)
            .filter(|rest| rest.starts_with(' '))
            .map(str::trim)
            .filter(|rest| !rest.is_empty())
            .map(str::to_owned)
    };
    if let Some(query) = after(SEARCH_WORD) {
        return Some(Opening::Results(query));
    }
    if let Some(address) = after(WEB_WORD) {
        return Some(Opening::WebNew(address));
    }
    Some(match said {
        "home" => Opening::Home,
        "trials" => Opening::Trials,
        "maintenance" => Opening::Maintenance,
        "android" => Opening::Android,
        "advanced" => Opening::Advanced,
        _ => return None,
    })
}

/// The opening for a file to install — what `ronoctua-shell software-file`
/// is handed by the file manager's double click. A path given relative is
/// taken from where the command was run.
#[must_use]
pub fn file_opening(given: &std::path::Path) -> Opening {
    let path = if given.is_absolute() {
        given.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| given.to_path_buf(), |d| d.join(given))
    };
    Opening::File(path)
}

/// Opens the software view in a window, on `opening`.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(opening: Opening) -> Result<()> {
    crate::door::run(move |w| {
        // THE SESSION'S CONTEXT: its tools and its directories. Without a
        // `$HOME` nothing can be read, and the view says it is still reading —
        // the footer's arrangement, for the same reason.
        let context = ronoctua_software::Context::of_session()
            .map_err(
                |e| tracing::warn!(error = %e, "no software context; the window reads nothing"),
            )
            .ok();

        // ONE PING FOR EVERYTHING THE THREADS BRING — a search's batch, a
        // sheet's details, a picture decoded — answered by taking whatever
        // arrived.
        let (wake, woke) = channel::channel::<()>();
        w.handle
            .insert_source(woke, |event, (), door: &mut Door<Software>| {
                if let channel::Event::Msg(()) = event {
                    door.content.woke(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the software channel: {e}"))?;
        let feed = context.as_ref().map(|c| {
            let mut feed = Feed::with(c.clone(), w.texts.language().to_owned(), waker(&wake));
            feed.took_preferences(c.prefs.clone());
            feed
        });
        let pictures = context
            .as_ref()
            .map(|c| Pictures::new(&c.roots, &c.tools, waker(&wake)));

        // THE QUEUE'S LIST: a job of ours that ENDED changed the machine, and
        // the sheet draws a job's progress while it runs.
        let (jobs_sender, jobs) = channel::channel::<Vec<ronoctua_transfers::Job>>();
        crate::transfers::listen(jobs_sender);
        w.handle
            .insert_source(jobs, |event, (), door: &mut Door<Software>| {
                if let channel::Event::Msg(jobs) = event {
                    door.content.jobs_arrived(&mut door.ui, &jobs);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the queue channel: {e}"))?;

        // `software.json`, watched the way the footer watches it: the footer
        // writes it too, and so does the CLI when it records a consent.
        let (wish, wish_changed) = channel::channel::<()>();
        let watch = context
            .as_ref()
            .and_then(|c| watch_the_preferences(&c.roots, move || wish.send(()).is_ok()));
        w.handle
            .insert_source(wish_changed, |event, (), door: &mut Door<Software>| {
                if let channel::Event::Msg(()) = event {
                    door.content.preferences_changed(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the software.json watch: {e}"))?;

        // THE COPIED IMAGE, read and kept by a thread (`Software::paste_image`).
        let (pasted_image, images) = channel::channel::<Kept>();
        w.handle
            .insert_source(images, |event, (), door: &mut Door<Software>| {
                if let channel::Event::Msg(kept) = event {
                    door.content.image_kept(&mut door.ui, kept);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the image channel: {e}"))?;

        let mut readings = Readings::fresh(
            context
                .as_ref()
                .map(|c| c.prefs.clone())
                .unwrap_or_default(),
            Taught::of_this_leader(),
        );
        readings.zone =
            ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        readings.now = crate::software_feed::seconds_now();
        readings.home = context.as_ref().map(|c| c.roots.home.clone());

        // A FILE NO SOURCE INSTALLS opens the first page and says so, rather
        // than a question whose yes the CLI would refuse.
        let (opening, refused) = match opening {
            Opening::File(path) if ronoctua_software::order::file_source(&path).is_none() => (
                Opening::Home,
                Some(Errand::Say(
                    w.texts.t(ronoctua_i18n::key::SOFTWARE_NOT_A_PACKAGE_FILE),
                )),
            ),
            other => (other, None),
        };
        let (view, errand) = View::fresh(opening, &readings, &w.texts);
        let mut me = Software {
            view,
            readings,
            feed,
            pictures,
            carrying: crate::carry::Carrying::bind(w.globals, w.qh),
            cursor: 0,
            first: std::iter::once(errand).chain(refused).collect(),
            pasted_image,
            _watch: watch,
        };
        // EVERY OPENING READS THE MACHINE: what the view says about what is
        // installed is never older than the view.
        me.want(Want::Reread);
        Ok(me)
    })
}

/// What a thread calls to wake this loop. Behind a lock, because every
/// thread holds the same one.
fn waker(wake: &channel::Sender<()>) -> impl Fn() + Send + Sync + 'static {
    let sender = std::sync::Mutex::new(wake.clone());
    move || {
        if let Ok(s) = sender.lock() {
            let _ = s.send(());
        }
    }
}

/// The watch on `software.json`, in the directory the crate's store writes.
fn watch_the_preferences(
    roots: &ronoctua_software::Roots,
    stirred: impl Fn() -> bool + Send + 'static,
) -> Option<ronoctua_files::notice::Notice> {
    let dir = roots.config.join(ronoctua_xdg::name());
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, "no configuration directory to watch software.json in");
        return None;
    }
    ronoctua_files::notice::Notice::on_file(
        &dir,
        ronoctua_software::prefs::FILE,
        ronoctua_files::notice::Pace::Coalesced,
        stirred,
    )
    .ok()
}

/// The window.
struct Software {
    view: View,
    /// The latest of every answer, handed to the view on every call.
    readings: Readings,
    /// The threads that read. `None` with no session to read from.
    feed: Option<Feed>,
    /// The icons, fetched and decoded on threads of their own.
    pictures: Option<Pictures>,
    /// What a file dropped on the window comes through. `None` where the
    /// compositor has no `wl_data_device_manager` — absence is not an error.
    carrying: Option<crate::carry::Carrying>,
    /// Which drawn row the cursor is on, as of the last key, click or reading
    /// — what the door follows. The view keeps the cursor by IDENTITY, and an
    /// index needs the rows, which need the width and the words; so it is
    /// counted where those are at hand.
    cursor: usize,
    /// What the opening asked for, done on the first turn of the loop — where
    /// there is a message line to say it on.
    first: Vec<Errand>,
    /// Where a launch's verdict comes back — see [`Software::start`].
    /// Where the copied image comes back, kept — see [`Software::paste_image`].
    pasted_image: channel::Sender<Kept>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl Software {
    /// Ask the feed, when there is one.
    fn want(&mut self, want: Want) {
        if let Some(feed) = self.feed.as_ref() {
            self.readings.want(feed, want);
        }
    }

    /// The rows, at this window's width, in its language.
    fn view_rows(&self, ui: &Ui) -> Vec<crate::software::Row> {
        self.view.rows(&self.readings, ui.columns, &ui.texts)
    }

    /// Count the cursor's row again, against the rows as they are now.
    fn recount(&mut self, ui: &Ui) {
        self.cursor = View::cursor(&self.view_rows(ui));
    }

    /// The threads brought something: take all of it, keep the cursor on its
    /// row, and draw — a reading never moves the page.
    fn woke(&mut self, ui: &mut Ui) {
        let arrivals = self.feed.as_ref().map(Feed::take).unwrap_or_default();
        let mut changed = false;
        for a in arrivals {
            changed |= self.readings.took(a);
        }
        changed |= self.pictures.as_mut().is_some_and(|p| !p.take().is_empty());
        if changed {
            self.settled(ui);
        }
    }

    /// The cursor put back in step with what is drawn, and a frame. The
    /// offset is only kept inside the list, by the paint.
    fn settled(&mut self, ui: &mut Ui) {
        self.readings.now = crate::software_feed::seconds_now();
        self.view.settle(&self.readings, ui.columns, &ui.texts);
        self.recount(ui);
        ui.ask_paint();
    }

    /// The queue's list, as it arrives. A job of ours that ended changed the
    /// machine, so the machine is read again, and whatever the page itself
    /// reads.
    fn jobs_arrived(&mut self, ui: &mut Ui, jobs: &[ronoctua_transfers::Job]) {
        let working = !self.readings.jobs.is_empty();
        if self.readings.took_jobs(jobs) {
            for want in self.view.rereads() {
                self.want(want);
            }
        }
        if working || !self.readings.jobs.is_empty() {
            self.settled(ui);
        }
    }

    /// `software.json` changed on disk — the footer, or the CLI recording a
    /// consent. Our own write reads back equal, and changes nothing.
    fn preferences_changed(&mut self, ui: &mut Ui) {
        let Some(config) = self.feed.as_ref().map(|f| f.roots().config.clone()) else {
            return;
        };
        let fresh = ronoctua_software::prefs::read(&config);
        if self.took_preferences(fresh) {
            self.settled(ui);
        }
    }

    /// The preferences as the file says them now: the feed's and the
    /// readings'. Answers whether anything drawn changed.
    fn took_preferences(&mut self, prefs: ronoctua_software::prefs::Preferences) -> bool {
        if let Some(feed) = self.feed.as_mut() {
            feed.took_preferences(prefs.clone());
        }
        self.readings.took_preferences(prefs)
    }

    /// One key, from the keyboard or from a click on the header.
    ///
    /// THE FOOTER'S ORDER, so the two doors answer one key the same way: the
    /// view's table first, by notation — `C-RET`, `S-<left>`, `TAB`, the
    /// arrows, `ESC` — and only then the minibuffer's gestures, where a letter
    /// is typing and `C-g` is `Esc`.
    fn key(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        use crate::software::Stroke;
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        ui.message = None;
        let errand = match crate::software::stroke_of(key) {
            Some(Stroke::Press(press)) => {
                self.pressed(ui, press);
                return;
            }
            Some(Stroke::Erase) => self.view.erase(&self.readings),
            Some(Stroke::Letter(c)) => self.view.letter(c, &self.readings, &ui.texts),
            None => return,
        };
        self.apply(ui, errand);
    }

    /// A key of the view, read — from the keyboard, or `RET` from a second
    /// click on the row the cursor is already on (invariant 19).
    ///
    /// THE WIDTH IS THE WINDOW'S: a note wraps, so the rows a key acts on are
    /// the rows drawn at this width.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        self.readings.now = crate::software_feed::seconds_now();
        let errand = self.view.key(press, &self.readings, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    /// Files let go over the window: a package to install, or an image for a
    /// web app's icon — the view says which, and refuses the rest in words.
    fn dropped(&mut self, ui: &mut Ui, paths: &[std::path::PathBuf]) {
        tracing::info!(
            dropped = paths.len(),
            "files were let go over the software window"
        );
        if paths.is_empty() {
            return;
        }
        ui.message = None;
        let errand = self.view.dropped(paths, &self.readings, &ui.texts);
        self.apply(ui, errand);
    }

    /// Carries out what the view asked for, and the frame after it.
    ///
    /// A LOOP AND NOT A CALL BACK INTO ITSELF, the footer's shape: a paste
    /// hands the clipboard to the view, whose answer is one more errand.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        let mut next = Some(errand);
        while let Some(e) = next.take() {
            next = self.carry_out(ui, e);
        }
        self.recount(ui);
        ui.ask_paint();
    }

    /// One errand. Answers the next, when doing this one produced it.
    fn carry_out(&mut self, ui: &mut Ui, errand: Errand) -> Option<Errand> {
        match errand {
            Errand::Nothing => {}
            Errand::Close => ui.quit = true,
            Errand::Feed(want) => self.want(want),
            Errand::Queue(orders) => self.queue(ui, orders),
            // ASKED ON THIS WINDOW'S OWN LINE, from what the field holds now;
            // the answer comes back in `answered`, to the same `View::typed`
            // the footer's minibuffer reaches.
            Errand::Ask { ask, initial } => ui.ask(crate::line::Line::new(
                ask.command(),
                ask.secret(),
                initial.as_deref(),
            )),
            // AN INSTALLED PROGRAM STARTS FROM HERE, by the launcher's own
            // answer to "how" (`apps::argv_of`: the terminal for one that
            // needs it), and what went wrong — no such entry yet, or a start
            // that failed — is said on THIS window's line. It used to be
            // handed to the footer, which said it on its own line, in the
            // other door, and closed its own software view on the way.
            Errand::Launch(desktop_id) => ui.launch_entry(&desktop_id),
            Errand::OpenOnce { url, profile } => self.open_once(ui, &url, &profile),
            Errand::Copy(text) => ui.message = Some(crate::chrome::copied(&text, &ui.texts)),
            Errand::Browse(url) => Self::open_the_page(ui, &url),
            Errand::Say(sentence) => ui.message = Some(sentence),
            Errand::Tweak(tweak) => self.tweak(ui, &tweak),
            Errand::Run(id) => Self::run(ui, id),
            // THE CLIPBOARD'S TEXT, into the search: read off the loop by the
            // door, and handed to `pasted`.
            Errand::Paste => ui.paste(),
            Errand::PasteImage => self.paste_image(ui),
        }
        None
    }

    /// A command of the registry the view names, done by this door as the
    /// footer's `software_run` does it.
    fn run(ui: &mut Ui, id: &str) {
        let Ok(me) = std::env::current_exe() else {
            tracing::error!("I do not know my own binary");
            return;
        };
        let me = me.to_string_lossy().into_owned();
        match id {
            // THE FILE MANAGER, beside the view: an image dragged out of it
            // lands on the view.
            "files-window" => ui.launch(&me, &["files".to_owned()], None),
            // THE BRAIN'S PAGE, where the automatic updates are.
            "brain-open" => match crate::surface::buffer_component("RONOCTUA_BUFFER_BRAIN") {
                Some(w) => ui.launch(&me, &["buffer".to_owned(), w], None),
                None => {
                    tracing::error!("RONOCTUA_BUFFER_BRAIN is not in the environment");
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::ECHO_NO_BUFFER));
                }
            },
            other => {
                tracing::warn!(
                    command = other,
                    "the software view named a command with no door here"
                );
            }
        }
    }

    /// THE COPIED IMAGE AS THE ICON: read off the loop (`crate::pasting`),
    /// written where the view takes it as a dropped file
    /// (`software_feed::keep_pasted`) on the same thread, and handed back to
    /// `image_kept`.
    fn paste_image(&self, ui: &mut Ui) {
        let Some(roots) = self.feed.as_ref().map(|f| f.roots().clone()) else {
            ui.message = Some(ui.texts.t(ronoctua_i18n::key::SOFTWARE_IMAGE_NOT_KEPT));
            return;
        };
        let back = self.pasted_image.clone();
        crate::pasting::image_then(move |read| {
            let _ = back.send(Kept::of(read, &roots));
        });
    }

    /// The copied image, kept — or the sentence for why not.
    fn image_kept(&mut self, ui: &mut Ui, kept: Kept) {
        match kept.file() {
            Ok(path) => {
                let errand = self.view.dropped(&[path], &self.readings, &ui.texts);
                self.apply(ui, errand);
            }
            Err(said) => {
                ui.message = Some(ui.texts.t(said));
                ui.ask_paint();
            }
        }
    }

    /// Each order to the queue as the software engine's work, through the same
    /// client the footer uses, and the sentence that says where to watch it —
    /// or that the queue is not there.
    fn queue(&mut self, ui: &mut Ui, orders: Vec<crate::software::Queued>) {
        let mut last = None;
        for q in orders {
            let work = ronoctua_transfers::Work::Software {
                order: Box::new(q.order),
                label: q.label.clone(),
            };
            if let Err(e) = ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work })
            {
                tracing::warn!(error = %e, "the transfer queue did not take the order");
                ui.message = Some(ui.texts.t(ronoctua_i18n::key::TRANSFERS_OFFLINE));
                return;
            }
            last = Some(q.label);
        }
        if let Some(label) = last {
            ui.message = Some(crate::software::queued_said(
                &label,
                &self.readings,
                &ui.texts,
            ));
        }
    }

    /// An address opened once, as a web app would open it, without making one.
    fn open_once(&self, ui: &mut Ui, url: &str, profile: &str) {
        let Some(feed) = self.feed.as_ref() else {
            return;
        };
        match ronoctua_software::web::once_argv(feed.context(), url, profile) {
            Ok(argv) => {
                if let Some((program, args)) = argv.split_first() {
                    ui.launch(program, args, None);
                }
            }
            Err(why) => {
                ui.message = Some(ui.texts.t(crate::software_words::refusal_key(why)));
            }
        }
    }

    /// An address, in whatever opens links here.
    fn open_the_page(ui: &mut Ui, url: &str) {
        match option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) {
            Some(opener) => ui.launch(opener, &[url.to_owned()], None),
            None => {
                tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
                ui.message = Some(
                    ui.texts
                        .t_with(ronoctua_i18n::key::ECHO_NO_OPENED, &[("program", url)]),
                );
            }
        }
    }

    /// One preference changed, through the crate's locked read-modify-write:
    /// the change is made to the file as it is NOW.
    fn tweak(&mut self, ui: &mut Ui, tweak: &crate::software::Tweak) {
        let Some(config) = self.feed.as_ref().map(|f| f.roots().config.clone()) else {
            return;
        };
        match ronoctua_software::prefs::change(&config, |p| tweak.apply(p)) {
            Ok(prefs) => {
                self.took_preferences(prefs);
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not write software.json");
                ui.message = Some(ui.texts.t(crate::software_words::refusal_key(
                    ronoctua_software::Refusal::WillNotParse,
                )));
            }
        }
    }
}

/// Where a row's picture goes: the cell its glyph had, on its line, centred
/// in the line's height — the footer's arithmetic, in this window's frame.
fn picture_area(
    frame: crate::chrome::Frame,
    column: u16,
    line: u16,
    side: f32,
) -> ronoctua_render::image::Area {
    ronoctua_render::image::Area {
        x: frame.x_of(column),
        // CENTRED ON THE LINE, and not flush with its top: the line's box is
        // taller than the drawing, and flush the icon would touch the row above.
        y: frame.y_of(line) + (frame.line() - side) / 2.0,
        width: side,
        height: side,
    }
}

impl Content for Software {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::SOFTWARE_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        self.key(ui, event, mods);
    }

    /// WHAT WAS TYPED ON THE HEADER'S LINE, to the same view call the
    /// footer's minibuffer reaches (`software_typed`).
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        self.readings.now = crate::software_feed::seconds_now();
        let errand = self.view.typed(id, text, &self.readings, &ui.texts);
        self.apply(ui, errand);
    }

    /// The clipboard's text, read off the loop for `C-v`: into the search.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        let errand = self.view.pasted(text, &self.readings);
        self.apply(ui, errand);
    }

    /// THE WAY BACK FIRST, and clickable: `Esc` leaves a level, and a pointer
    /// that could not leave one would be a pointer that goes in and stays.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        crate::chrome::with_the_way_back(
            self.view.hint(&self.readings, ui.columns, &ui.texts),
            &ui.texts,
        )
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer draws for
    /// these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.cursor)
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click
    /// puts the cursor on the row, and a click on the row it is already on
    /// is that row's `RET`, through the same `pressed` a key goes through.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, Press::Enter);
        } else {
            self.recount(ui);
        }
    }

    /// The standard window, and the pictures: a row with one draws it in the
    /// cell its glyph had, once it is here — the glyph until then, never both.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        crate::door::list::header(self, ui, canvas);
        let rows = self.view_rows(ui);
        // THE OFFSET KEPT INSIDE THE LIST before the visible rows are known —
        // what `list::body` would do first, and which rows those are decides
        // which pictures are asked for.
        ui.settle_offset(rows.len());
        let (offset, room, first) = (ui.offset, ui.room_for_rows(), ui.first_of_list());
        // THE PICTURE AT THE SCREEN'S OWN PIXELS, and drawn in the window's
        // logical ones: the key carries the side, so a screen of another
        // scale asks for another picture instead of stretching this one.
        let side = crate::apps::pixmap_side();
        #[allow(clippy::cast_precision_loss)]
        let scale = ui.scale.max(1) as f32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pixels_across = (side * scale).ceil() as u32;
        let mut placed: Vec<(String, u16, u16)> = Vec::new();
        let mut fields: Vec<Vec<crate::files::Field>> = Vec::with_capacity(rows.len());
        for (i, row) in rows.into_iter().enumerate() {
            let mut f = row.fields;
            let shown = (offset..offset + room).contains(&i);
            if shown
                && let (Some(portrait), Some(pool)) = (row.picture.as_ref(), self.pictures.as_mut())
                && pool.get(portrait, pixels_across).is_some()
                && let Some(key) = pool.ready(portrait, pixels_across)
                && !f.is_empty()
            {
                // THE GLYPH'S CELL IS WHERE THE PICTURE GOES, and the glyph
                // leaves the row the moment the picture takes it.
                let at = f.remove(0).at;
                let line = first + u16::try_from(i - offset).unwrap_or(0);
                placed.push((key, ui.margin + at, line));
            }
            fields.push(f);
        }
        crate::door::list::body(ui, canvas, &fields);
        let frame = ui.frame(0);
        let pool = self.pictures.as_ref();
        for (key, column, line) in placed {
            if canvas.upload(&key, || pool.and_then(|p| p.pixels(&key)).cloned()) {
                canvas
                    .pictures
                    .push((key, picture_area(frame, column, line, side)));
            }
        }
    }

    /// A NOTE WRAPS at the width, so the row the cursor is on moves with it.
    fn resized(&mut self, ui: &mut Ui) {
        self.recount(ui);
    }

    /// A LANGUAGE CHOSEN ELSEWHERE: searches and details ask in it, and the
    /// rows are counted again in its words.
    fn language_changed(&mut self, ui: &mut Ui) {
        if let Some(feed) = self.feed.as_mut() {
            feed.set_language(ui.texts.language());
        }
        self.settled(ui);
    }

    /// What the opening asked for, once, where there is a line to say it on.
    fn each_turn(&mut self, ui: &mut Ui) {
        for errand in std::mem::take(&mut self.first) {
            self.apply(ui, errand);
        }
    }

    /// THE DATA DEVICE IS THE SEAT'S, and so cannot be made before one arrives.
    /// Asked on every capability, which is harmless: `took_a_seat` makes one
    /// device per seat and refuses a second.
    fn seat_capability(door: &mut Door<Self>, qh: &QueueHandle<Door<Self>>, seat: &WlSeat) {
        if let Some(c) = door.content.carrying.as_mut() {
            c.took_a_seat(qh, seat);
        }
    }
}

// ---------------------------------------------------------------------------
// THE DROP — a package to install, or an image for the icon
// ---------------------------------------------------------------------------
//
// The three traits, for this door and no other, reached through the door's
// generic dispatch because the data device's objects ask for exactly these.
// The window takes files and gives none: it never starts a drag, so the
// source's half is empty.

impl DataDeviceHandler for Door<Software> {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &WlDataDevice,
        _: f64,
        _: f64,
        _: &WlSurface,
    ) {
        let Some(offer) = device
            .data::<DataDeviceData>()
            .and_then(DataDeviceData::drag_offer)
        else {
            return;
        };
        // WHAT WE CAN TAKE, said out loud: a list of files, and nothing else.
        let takeable =
            offer.with_mime_types(|kinds| kinds.iter().any(|k| k == crate::carry::URI_LIST));
        offer.accept_mime_type(
            offer.serial,
            takeable.then(|| crate::carry::URI_LIST.to_owned()),
        );
        // A COPY, ALWAYS. Installing a file reads it; a MOVE would tell the
        // program it came from that it may delete what was dropped.
        if takeable {
            offer.set_actions(DndAction::Copy, DndAction::Copy);
        }
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    /// NOTHING CHANGES WITH THE POINT: a package is taken anywhere on the
    /// window, and an image on the icon's levels — which the view says on
    /// the drop, in words, for anything else.
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}

    /// THE SELECTION IS NOT THIS MODULE'S — see `crate::carry`.
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        let Some(offer) = device
            .data::<DataDeviceData>()
            .and_then(DataDeviceData::drag_offer)
        else {
            return;
        };
        // READ WITH A DEADLINE: the other end is somebody else's program — see
        // `carry::read`.
        let paths = offer
            .receive(crate::carry::URI_LIST.to_owned())
            .ok()
            .and_then(crate::carry::read)
            .unwrap_or_default();
        offer.finish();
        self.content.dropped(&mut self.ui, &paths);
    }
}

impl DataSourceHandler for Door<Software> {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
    }

    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: String,
        _: WritePipe,
    ) {
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl DataOfferHandler for Door<Software> {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }

    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EVERY KEY THE VIEW TEACHES reaches it from this door as itself: the
    /// header's click presses it through `door::keys::event_of`, the keyboard
    /// through the same reader, and both come back as the notation the table
    /// has — `C-RET` above all, which a reader that dropped the modifier would
    /// turn into a plain `RET`.
    #[test]
    fn every_key_the_view_teaches_comes_back_as_itself() {
        for (notation, id, _) in crate::software::OF_THE_VIEW {
            let key = ronoctua_keymap::Key::parse(notation).expect(notation);
            let (event, mods) = crate::door::keys::event_of(key);
            let read = crate::door::keys::key_of(&event, mods).expect(notation);
            assert_eq!(read.notation(), *notation, "{id}");
            assert_eq!(
                crate::software::pressed(&read.notation()),
                crate::software::press_of(id),
                "{notation} is not {id} in the window"
            );
        }
        // AND THE WAY BACK, which no table teaches.
        let (event, mods) =
            crate::door::keys::event_of(ronoctua_keymap::Key::parse("ESC").expect("ESC"));
        let read = crate::door::keys::key_of(&event, mods).expect("ESC");
        assert_eq!(
            crate::software::pressed(&read.notation()),
            Some(Press::Escape)
        );
    }

    /// A LETTER IS TYPING, here as in the footer: no key of the view's table,
    /// the minibuffer's gesture, and the view's search takes it — and `DEL`
    /// and `C-v` reach the same view calls the footer's do.
    #[test]
    fn a_letter_del_and_paste_reach_the_search_as_in_the_footer() {
        use ronoctua_keymap::{Key, Sym};
        let (event, mods) = crate::door::keys::event_of(Key::parse("g").expect("g"));
        let read = crate::door::keys::key_of(&event, mods).expect("g");
        assert_eq!(read.sym, Sym::Char('g'));
        assert_eq!(crate::software::pressed(&read.notation()), None);
        assert_eq!(
            crate::minibuffer::gesture_of(read),
            crate::minibuffer::Gesture::Insert('g')
        );
        assert_eq!(
            crate::software::stroke_of(read),
            Some(crate::software::Stroke::Letter('g'))
        );
        let (event, mods) = crate::door::keys::event_of(Key::parse("DEL").expect("DEL"));
        let read = crate::door::keys::key_of(&event, mods).expect("DEL");
        assert_eq!(crate::software::pressed(&read.notation()), None);
        assert_eq!(
            crate::minibuffer::gesture_of(read),
            crate::minibuffer::Gesture::Delete
        );
        assert_eq!(
            crate::software::stroke_of(read),
            Some(crate::software::Stroke::Erase)
        );
        // `C-g` IS `Esc`, through the same reader.
        let (event, mods) = crate::door::keys::event_of(Key::parse("C-g").expect("C-g"));
        let read = crate::door::keys::key_of(&event, mods).expect("C-g");
        assert_eq!(
            crate::software::stroke_of(read),
            Some(crate::software::Stroke::Press(Press::Escape))
        );
        for paste in ["C-v", "C-y"] {
            let (event, mods) = crate::door::keys::event_of(Key::parse(paste).expect(paste));
            let read = crate::door::keys::key_of(&event, mods).expect(paste);
            assert_eq!(
                crate::software::pressed(&read.notation()),
                Some(Press::Paste),
                "{paste}"
            );
        }

        // AND THE WINDOW'S VIEW TAKES THEM: the letter is a search, drawn on
        // the search's own row.
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let r = Readings::default();
        let (mut v, _) = View::fresh(Opening::Home, &r, &texts);
        assert!(matches!(
            v.letter('g', &r, &texts),
            Errand::Feed(Want::Search { .. })
        ));
        assert_eq!(v.query(), "g");
        assert_eq!(v.key(Press::Paste, &r, 100, &texts), Errand::Paste);
    }

    /// EVERY WORD OF THE COMMAND LINE opens a level, no two the same, and a
    /// word that names none is refused rather than opening the first page.
    #[test]
    fn every_level_word_opens_its_own_level() {
        let openings: Vec<Opening> = LEVEL_WORDS
            .iter()
            .map(|w| opening_of(w).unwrap_or_else(|| panic!("{w} opens nothing")))
            .collect();
        for (i, a) in openings.iter().enumerate() {
            for b in &openings[i + 1..] {
                assert_ne!(a, b, "two words open one level");
            }
        }
        assert_eq!(opening_of("home"), Some(Opening::Home));
        assert_eq!(opening_of("sheet"), None);
        assert_eq!(opening_of(""), None);
        // AND A SEARCH, whose words are the query — never the word itself.
        assert_eq!(
            opening_of("search gimp photo"),
            Some(Opening::Results("gimp photo".to_owned()))
        );
        assert_eq!(opening_of("search"), None, "a search for nothing");
        assert_eq!(opening_of("search   "), None, "a search for nothing");
        assert_eq!(opening_of("searchgimp"), None);
        // AND A NEW WEB APP, whose address the view makes canonical.
        assert_eq!(
            opening_of("web https://example.org/mail"),
            Some(Opening::WebNew("https://example.org/mail".to_owned()))
        );
        assert_eq!(opening_of("web"), None, "a web app of nothing");
    }

    /// A NEW WEB APP OPENED FROM THE COMMAND LINE is the editor, reading the
    /// page — what the footer's `i b` does once the address is typed.
    #[test]
    fn a_web_address_opens_the_editor_in_the_window() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let r = Readings::default();
        let (v, errand) = View::fresh(
            opening_of("web https://example.org").expect("an opening"),
            &r,
            &texts,
        );
        assert!(
            matches!(errand, Errand::Feed(Want::Site(_))),
            "the page is not read: {errand:?}"
        );
        let rows = v.rows(&r, 100, &texts);
        assert!(
            rows.iter()
                .any(|row| row.fields.iter().any(|f| f.text.contains("example.org"))),
            "the editor does not show the address"
        );
    }

    /// A FILE FROM THE MANAGER is already absolute, and one typed on a
    /// command line is taken from where it was typed.
    #[test]
    fn a_file_to_install_is_an_absolute_path() {
        let given = std::path::Path::new("/tmp/Some App.AppImage");
        assert_eq!(file_opening(given), Opening::File(given.to_path_buf()));
        let Opening::File(p) = file_opening(std::path::Path::new("x.apk")) else {
            panic!("a file opens a file");
        };
        assert!(p.is_absolute(), "{}", p.display());
        assert!(p.ends_with("x.apk"));
    }

    /// A window with no session behind it: no feed, no pictures, no data
    /// device — the view and the door's own channels, which is all a test of
    /// the wiring needs.
    fn a_window(opening: Opening) -> (Software, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let readings = Readings::default();
        let (view, first) = View::fresh(opening, &readings, &texts);
        let (pasted_image, _) = channel::channel();
        let me = Software {
            view,
            readings,
            feed: None,
            pictures: None,
            carrying: None,
            cursor: 0,
            first: vec![first],
            pasted_image,
            _watch: None,
        };
        (me, Ui::for_tests(texts))
    }

    /// EVERY QUESTION IS ASKED ON THIS WINDOW'S OWN LINE — none is sent to
    /// the footer — and its answer reaches the same `View::typed` the
    /// footer's minibuffer reaches.
    #[test]
    fn a_question_is_asked_and_answered_in_the_window() {
        for ask in [
            crate::software::Ask::WebNew,
            crate::software::Ask::WebAddress,
            crate::software::Ask::WebName,
            crate::software::Ask::IconPath,
            crate::software::Ask::FilePath,
        ] {
            let (mut w, mut ui) = a_window(Opening::Home);
            w.apply(
                &mut ui,
                Errand::Ask {
                    ask,
                    initial: Some("from here".into()),
                },
            );
            let line = ui.asking.as_ref().expect("asked on the window's line");
            assert_eq!(line.id, ask.command());
            assert_eq!(line.text, "from here");
            assert!(
                ui.orders().is_empty(),
                "{} went to the footer",
                ask.command()
            );
        }
        // AND THE ANSWER: a new web app's address opens the editor on it.
        let (mut w, mut ui) = a_window(Opening::Home);
        w.answered(&mut ui, "software-web-new", "example.org/mail");
        let rows = w.view.rows(&w.readings, 100, &ui.texts);
        assert!(
            rows.iter()
                .any(|row| row.fields.iter().any(|f| f.text.contains("example.org"))),
            "the answer did not reach the view"
        );
        // AND A FILE'S PATH that no source installs is refused in words.
        let (mut w, mut ui) = a_window(Opening::Home);
        w.answered(&mut ui, "software-file-path", "/tmp/not-a-package.txt");
        assert_eq!(
            ui.message.as_deref(),
            Some(
                ui.texts
                    .t(ronoctua_i18n::key::SOFTWARE_NOT_A_PACKAGE_FILE)
                    .as_str()
            )
        );
    }

    /// THE WINDOW'S OWN PASTE: `C-v` asks the door for the clipboard — read
    /// off the loop, never here — and the text it hands back is the search.
    #[test]
    fn a_paste_is_asked_of_the_door_and_lands_in_the_search() {
        let (mut w, mut ui) = a_window(Opening::Home);
        w.pressed(&mut ui, Press::Paste);
        assert!(
            ui.wants_the_clipboard(),
            "C-v did not ask for the clipboard"
        );
        w.pasted(&mut ui, "gimp\nsomething else");
        assert_eq!(w.view.query(), "gimp");
        // AN EMPTY CLIPBOARD changes nothing.
        w.pasted(&mut ui, "");
        assert_eq!(w.view.query(), "gimp");
    }

    /// FAILS IF: starting an installed program is handed to the footer —
    /// whose "not in the launcher yet" and "could not open" were then said on
    /// the footer's line, in the other door — instead of started from here,
    /// where the door says both on this window's line.
    #[test]
    fn launching_is_this_windows_own_and_no_order_for_the_footer() {
        let (mut w, mut ui) = a_window(Opening::Home);
        w.apply(&mut ui, Errand::Launch("org.gimp.GIMP.desktop".into()));
        assert!(ui.orders().is_empty(), "{:?}", ui.orders());
        assert_eq!(
            ui.launches(),
            [crate::door::Launching::Entry(
                "org.gimp.GIMP.desktop".to_owned()
            )]
        );
    }

    /// THE PICTURE TAKES ITS GLYPH'S CELL and sits in the middle of its line,
    /// inside the line's height.
    #[test]
    fn a_picture_sits_in_its_glyphs_cell_and_inside_its_line() {
        let frame = crate::chrome::Frame::of(900.0, 600.0, 0);
        let side = crate::apps::pixmap_side();
        let a = picture_area(frame, 3, 5, side);
        assert!((a.x - frame.x_of(3)).abs() < f32::EPSILON);
        assert!(a.y >= frame.y_of(5), "above its line");
        assert!(
            a.y + a.height <= frame.y_of(6) + f32::EPSILON,
            "into the next line"
        );
        assert!((a.width - side).abs() < f32::EPSILON);
    }
}
