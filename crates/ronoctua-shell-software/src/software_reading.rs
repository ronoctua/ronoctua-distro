//! WHAT AN OPEN VIEW OF THE PERSON'S PROGRAMS READS, IN EITHER DOOR — the
//! readings, the threads that fill them, the pictures and `software.json`
//! watched ([`Reading`]) — and the ONE handler of what the view asks
//! ([`Reading::carry_out`]), which the footer's region (`software_region`)
//! and the window (`software_window`) both call over their door
//! ([`Doorway`]).
//!
//! # Why one handler
//!
//! The view decides and a door does what it decided (`software.rs`: the view
//! holds no readings, runs nothing and knows no window). The doing was
//! written twice — the footer's `software_do` and the window's `carry_out` —
//! and the two had drifted: a copied line was a brief word in the window and
//! an ordinary one in the footer, a refusal to write `software.json` stayed
//! longer on one line than on the other, a browser that was not there was
//! said in one door and only logged in the other. One handler over a
//! `Doorway` makes the owner's rule (the two doors do the same things) a fact
//! of the types: what differs is what each door IS — the window's line or the
//! footer's minibuffer, the window's list or the footer's page, the window's
//! own launches or the footer's — and the doorway is that difference.
//!
//! # One wake for everything
//!
//! A search's batch, a sheet's details, a picture decoded, the copied image
//! kept, `software.json` moved, the file picker's folder: every thread of the
//! view calls the door's one wake, and the door answers it with
//! [`Reading::woken`], which takes whatever arrived. Nothing is read twice
//! and no thread waits for the loop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use crate::programs_words::Running;
use crate::region::{Doorway, News};
use crate::software::{Errand, Press, Row, Stroke, View};
use crate::software_feed::{Arrival, Feed, Kept, Readings, Want};
use crate::software_pictures::Pictures;

/// What an open view reads, in either door — see the module.
pub struct Reading {
    /// The latest of every answer, handed to the view on every call.
    pub readings: Readings,
    /// The threads that read. `None` for a door that reaches nothing of the
    /// machine (a photograph's) or with no session to read from.
    feed: Option<Feed>,
    /// The programs' pictures, decoded on threads of their own.
    pictures: Option<Pictures>,
    /// The door's wake, which every thread of the view calls.
    wake: crate::wake::Wake,
    /// Where the copied image comes back, kept ([`Errand::PasteImage`]).
    kept: mpsc::Receiver<Kept>,
    keeping: mpsc::Sender<Kept>,
    /// `software.json`, watched: both doors write it, and so does the CLI
    /// when it records a consent.
    _watch: Option<ronoctua_files::notice::Notice>,
    /// The watch saw the file move since the last look.
    stirred: Arc<AtomicBool>,
    /// The language the feed asks in — Flathub answers in the one it is
    /// asked in.
    language: String,
}

impl std::fmt::Debug for Reading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reading")
            .field("feed", &self.feed.is_some())
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// What a wake brought ([`Reading::woken`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Woke {
    /// Something drawn changed: the view settles, and a frame is owed.
    pub changed: bool,
    /// A reading of the machine arrived — what a launcher entry's sheet
    /// waits for (`software_region`).
    pub read: bool,
    /// What the view asked of what arrived — the copied image taken as the
    /// icon, or why it was not — for the door to carry out.
    pub errands: Vec<Errand>,
}

impl Reading {
    /// What a door reads for its view: `readings`, and — with a `context`, a
    /// door that reaches the machine — the threads that fill them, the
    /// pictures and `software.json` watched, each calling `wake`.
    #[must_use]
    pub fn new(
        readings: Readings,
        context: Option<ronoctua_software::Context>,
        language: &str,
        wake: crate::wake::Wake,
    ) -> Self {
        let waker = {
            let wake = wake.clone();
            move || wake.call()
        };
        let feed = context.as_ref().map(|c| {
            let mut feed = Feed::with(c.clone(), language.to_owned(), waker.clone());
            feed.took_preferences(c.prefs.clone());
            feed
        });
        let pictures = context
            .as_ref()
            .map(|c| Pictures::new(&c.roots, &c.tools, waker.clone()));
        let stirred = Arc::new(AtomicBool::new(false));
        let watch = context.as_ref().and_then(|c| {
            let stirred = Arc::clone(&stirred);
            let wake = wake.clone();
            watch_the_preferences(&c.roots, move || {
                stirred.store(true, Ordering::Relaxed);
                wake.call();
                true
            })
        });
        let (keeping, kept) = mpsc::channel();
        Self {
            readings,
            feed,
            pictures,
            wake,
            kept,
            keeping,
            _watch: watch,
            stirred,
            language: language.to_owned(),
        }
    }

    /// Whether threads read for the view: a door that reaches the machine,
    /// with a session.
    #[must_use]
    pub const fn reads(&self) -> bool {
        self.feed.is_some()
    }

    /// The person's `$HOME`, as the session's context has it.
    #[must_use]
    pub fn home(&self) -> Option<std::path::PathBuf> {
        self.feed.as_ref().map(|f| f.roots().home.clone())
    }

    /// The pictures' pool, when there is one.
    #[must_use]
    pub const fn pictures(&self) -> Option<&Pictures> {
        self.pictures.as_ref()
    }

    /// One picture at `side` pixels, when it is ready — and asked for when it
    /// is not (`Pictures::get`).
    pub fn picture_now(
        &mut self,
        portrait: &crate::software_pictures::Portrait,
        side: u32,
    ) -> Option<&ronoctua_render::image::Rgba> {
        self.pictures.as_mut().and_then(|p| p.get(portrait, side))
    }

    /// The pictures of `rows` on screen, asked for at `side` pixels — the
    /// visible page's only, before the frame's borrows.
    pub fn ask_pictures<'a>(&mut self, rows: impl Iterator<Item = &'a Row>, side: u32) {
        let Some(pictures) = self.pictures.as_mut() else {
            return;
        };
        for row in rows {
            if let Some(p) = &row.picture {
                let _ = pictures.get(p, side);
            }
        }
    }

    /// Asks the feed, when there is one — with no threads nothing is asked,
    /// and the next opening reads again.
    pub fn want(&mut self, want: Want) {
        if let Some(feed) = self.feed.as_ref() {
            self.readings.want(feed, want);
        }
    }

    /// The feed asks in `language` from now on — checked on every call, so a
    /// language chosen elsewhere reaches the next search and the next sheet.
    pub fn speak(&mut self, language: &str) {
        if self.language != language {
            language.clone_into(&mut self.language);
            if let Some(feed) = self.feed.as_mut() {
                feed.set_language(language);
            }
        }
    }

    /// The cursor put back in step with what is drawn at `columns`. It never
    /// moves the page.
    pub fn settle(&mut self, view: &mut View, columns: u16, texts: &ronoctua_i18n::Texts) {
        self.readings.now = crate::when::seconds_now();
        view.settle(&self.readings, columns, texts);
    }

    /// The view's rows at `columns`, in `texts`.
    #[must_use]
    pub fn rows(&self, view: &View, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        view.rows(&self.readings, columns, texts)
    }

    /// The preferences as the file says them now — the feed's and the
    /// readings'. Answers whether anything drawn changed.
    pub fn took_preferences(&mut self, prefs: ronoctua_software::prefs::Preferences) -> bool {
        if let Some(feed) = self.feed.as_mut() {
            feed.took_preferences(prefs.clone());
        }
        self.readings.took_preferences(prefs)
    }

    /// The queue's list, as it arrives. A job of ours that ENDED changed the
    /// machine: it is read again, and whatever the page itself reads.
    /// Answers whether the view draws a job (one runs, or one just stopped).
    pub fn jobs(&mut self, view: Option<&View>, jobs: &[ronoctua_transfers::Job]) -> bool {
        let working = !self.readings.jobs.is_empty();
        if self.readings.took_jobs(jobs) {
            let wants = view.map_or_else(|| vec![Want::Reread], View::rereads);
            for want in wants {
                self.want(want);
            }
        }
        working || !self.readings.jobs.is_empty()
    }

    /// The view's threads woke the door: take all of what they left — the
    /// readings, the pictures, `software.json`, the copied image, the file
    /// picker's folder — and answer what it changed. A reading never moves
    /// the cursor off its row, nor the page.
    ///
    /// THE MARKS FOLLOW the reading the view made anyway: what the
    /// modeline's two software marks are drawn from is told to the door's
    /// host ([`Doorway::news`]), which in a window is nobody.
    pub fn woken(&mut self, view: &mut View, way: &mut dyn Doorway) -> Woke {
        self.speak(way.texts().language());
        let mut woke = Woke::default();
        let arrivals = self.feed.as_ref().map(Feed::take).unwrap_or_default();
        for a in arrivals {
            if let Arrival::Snapshot(s) = &a {
                way.news(News::of(Running::of(s)));
                woke.read = true;
            }
            woke.changed |= self.readings.took(a);
        }
        woke.changed |= self.pictures.as_mut().is_some_and(|p| !p.take().is_empty());
        if self.stirred.swap(false, Ordering::Relaxed) {
            woke.changed |= self.preferences_changed();
        }
        while let Ok(kept) = self.kept.try_recv() {
            woke.errands.push(self.image_kept(view, kept, way));
            woke.changed = true;
        }
        woke.changed |= view.look();
        woke
    }

    /// `software.json` changed on disk — the other door, or the CLI recording
    /// a consent. Our own write reads back equal, and changes nothing.
    fn preferences_changed(&mut self) -> bool {
        let Some(config) = self.feed.as_ref().map(|f| f.roots().config.clone()) else {
            return false;
        };
        let fresh = ronoctua_software::prefs::read(&config);
        self.took_preferences(fresh)
    }

    // ─── what a person does, in either door ─────────────────────────────────
    //
    // Each answers what the view asked, for the door to carry out
    // ([`Reading::carry_out`], with the door's own rule first where it has
    // one — the footer gives the keyboard back before a program opens).

    /// A key the door read, through the view's reader — the table first, by
    /// notation (`C-RET`, `S-<left>`, `TAB`, the arrows, `ESC`), and only then
    /// is a bare letter the search; the file picker, while it is up over the
    /// view, takes what it reads first.
    pub fn stroke(
        &mut self,
        view: &mut View,
        key: ronoctua_keymap::Key,
        way: &mut dyn Doorway,
    ) -> Errand {
        self.speak(way.texts().language());
        if view.picking()
            && let Some(errand) = view.pick_key(key, &self.readings, way.texts())
        {
            return errand;
        }
        match crate::software::stroke_of(key) {
            Some(Stroke::Press(press)) => self.pressed(view, press, way),
            Some(Stroke::Erase) => view.erase(&self.readings),
            Some(Stroke::Letter(c)) => view.letter(c, &self.readings, way.texts()),
            None => Errand::Nothing,
        }
    }

    /// A key of the view, read — from the keyboard, a command run by name, or
    /// `RET` from a second click on the row the cursor is on (invariant 19).
    ///
    /// THE WIDTH IS THE DOOR'S: a note wraps, so the rows a key acts on are
    /// the rows drawn at this width. And A PAGE THAT OPENED — or one gone back
    /// to — STARTS AT ITS TOP: the door kept the page it had on the page
    /// before, and "Declared by you" opened with its own title scrolled away
    /// (photographed).
    pub fn pressed(&mut self, view: &mut View, press: Press, way: &mut dyn Doorway) -> Errand {
        self.speak(way.texts().language());
        self.readings.now = crate::when::seconds_now();
        let before = view.level().clone();
        let errand = view.key(press, &self.readings, way.columns(), way.texts());
        if *view.level() != before {
            way.first_page();
            way.follow();
        }
        errand
    }

    /// A click on drawn row `n`: it selects, and the second one acts — that
    /// row's `RET`.
    pub fn clicked(&mut self, view: &mut View, n: usize, way: &mut dyn Doorway) -> Errand {
        let rows = view.rows(&self.readings, way.columns(), way.texts());
        if view.clicked(n, &rows) {
            self.pressed(view, Press::Enter, way)
        } else {
            way.repaint();
            Errand::Nothing
        }
    }

    /// What the door's line answered, for the command that asked it — the
    /// footer's minibuffer or the window's header, the same call.
    pub fn typed(
        &mut self,
        view: &mut View,
        id: &str,
        said: &str,
        way: &mut dyn Doorway,
    ) -> Errand {
        self.speak(way.texts().language());
        self.readings.now = crate::when::seconds_now();
        view.typed(id, said, &self.readings, way.texts())
    }

    /// The clipboard's text the view asked for, read off the loop by the door.
    pub fn pasted(&mut self, view: &mut View, text: &str) -> Errand {
        view.pasted(text, &self.readings)
    }

    /// Files let go over the view: a package to install, or an image for a
    /// web app's icon — the view says which, and refuses the rest in words.
    pub fn dropped(
        &mut self,
        view: &mut View,
        paths: &[std::path::PathBuf],
        way: &dyn Doorway,
    ) -> Errand {
        if paths.is_empty() {
            return Errand::Nothing;
        }
        view.dropped(paths, &self.readings, way.texts())
    }

    /// The copied image, kept — the icon, as a dropped file — or the sentence
    /// for why not.
    fn image_kept(&mut self, view: &mut View, kept: Kept, way: &dyn Doorway) -> Errand {
        match kept.file() {
            Ok(path) => self.dropped(view, &[path], way),
            Err(said) => Errand::Say(way.texts().t(said)),
        }
    }

    // ─── what the view asked, done ──────────────────────────────────────────

    /// THE ONE HANDLER of what the view asked, in either door — see the
    /// module.
    pub fn carry_out(&mut self, errand: Errand, way: &mut dyn Doorway) {
        use crate::echo::Echo;
        match errand {
            Errand::Nothing => {}
            Errand::Close => way.close(),
            Errand::Feed(want) => self.want(want),
            Errand::Queue(orders) => self.queue(orders, way),
            // ASKED ON THE DOOR'S OWN LINE — the footer's minibuffer, the
            // window's header — from what the field holds now; the answer
            // comes back to the same `View::typed`.
            Errand::Ask { ask, initial } => way.ask(crate::line::Line::new(
                ask.command(),
                ask.secret(),
                initial.as_deref(),
            )),
            // AN INSTALLED PROGRAM STARTS BY THE LAUNCHER'S OWN ANSWER to
            // "how" (`launch::argv_of`: the terminal for one that needs it),
            // and what went wrong — no such entry yet, a start that failed —
            // is said on THIS door's line.
            Errand::Launch(desktop_id) => way.launch(crate::door::Launching::Entry(desktop_id)),
            Errand::OpenOnce { url, profile } => self.open_once(&url, &profile, way),
            // A WORD THAT CONFIRMS A GESTURE goes sooner.
            Errand::Copy(text) => {
                let said = crate::chrome::copied(&text, way.texts());
                way.say(said, Echo::Brief);
            }
            Errand::Browse(url) => open_the_page(&url, way),
            Errand::Say(sentence) => way.say(sentence, Echo::Normal),
            Errand::Tweak(tweak) => self.tweak(&tweak, way),
            // A COMMAND OF THE REGISTRY the view names — the file manager
            // beside the view (an image dragged out of it lands on the view),
            // the brain's page — is a window of this shell, started by the
            // door the view is in, as each door started it: the window never
            // asks the footer for it, so it opens with the footer down too.
            Errand::Run(id) => open_the_window_of(id, way),
            // THE CLIPBOARD'S TEXT, read off the loop by the door and handed
            // to the view's `pasted`.
            Errand::Paste => way.paste(),
            Errand::PasteImage => self.paste_image(way),
            // A CATALOGUE'S HOSTS LET: its row goes, and the next search asks
            // it. A consent that could not be kept is said, never dropped — and
            // a refusal stays long enough to be read.
            Errand::Let(source) => match crate::consent_rows::allow(source.hosts(), way.texts()) {
                Ok(()) => {
                    self.readings.reread_consents();
                }
                Err(said) => way.say(said, Echo::Long),
            },
        }
    }

    /// Each order to the queue as the software engine's work, and the
    /// sentence that says where to watch it — or that the queue is not there.
    fn queue(&mut self, orders: Vec<crate::software::Queued>, way: &mut dyn Doorway) {
        let mut last = None;
        for q in orders {
            let work = ronoctua_transfers::Work::Software {
                order: Box::new(q.order),
                label: q.label.clone(),
            };
            if let Err(e) = ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work })
            {
                tracing::warn!(error = %e, "the transfer queue did not take the order");
                let said = way.texts().t(crate::key::TRANSFERS_OFFLINE);
                way.say(said, crate::echo::Echo::Normal);
                return;
            }
            last = Some(q.label);
        }
        if let Some(label) = last {
            let said = crate::software::queued_said(&label, &self.readings, way.texts());
            way.say(said, crate::echo::Echo::Normal);
        }
    }

    /// An address opened once, as a web app would open it, without making one.
    fn open_once(&self, url: &str, profile: &str, way: &mut dyn Doorway) {
        let Some(feed) = self.feed.as_ref() else {
            return;
        };
        match ronoctua_software::web::once_argv(feed.context(), url, profile) {
            Ok(argv) => {
                if let Some((program, args)) = argv.split_first() {
                    way.launch(crate::door::Launching::Program {
                        program: program.clone(),
                        args: args.to_vec(),
                        cwd: None,
                    });
                }
            }
            Err(why) => {
                let said = way.texts().t(crate::software_words::refusal_key(why));
                way.say(said, crate::echo::Echo::Normal);
            }
        }
    }

    /// One preference changed, through the crate's locked read-modify-write:
    /// the change is made to the file as it is NOW.
    fn tweak(&mut self, tweak: &crate::software::Tweak, way: &mut dyn Doorway) {
        let Some(config) = self.feed.as_ref().map(|f| f.roots().config.clone()) else {
            return;
        };
        match ronoctua_software::prefs::change(&config, |p| tweak.apply(p)) {
            Ok(prefs) => {
                self.took_preferences(prefs);
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not write software.json");
                let said = way.texts().t(crate::software_words::refusal_key(
                    ronoctua_software::Refusal::WillNotParse,
                ));
                way.say(said, crate::echo::Echo::Long);
            }
        }
    }

    /// THE COPIED IMAGE AS THE ICON — WHAT IS COPIED NOW, as either door reads
    /// it (`pasting::image`): read off the loop, written where the view takes
    /// it as a dropped file (`software_feed::keep_pasted`) on the same thread,
    /// and taken by [`Reading::woken`].
    fn paste_image(&self, way: &mut dyn Doorway) {
        let Some(roots) = self.feed.as_ref().map(|f| f.roots().clone()) else {
            let said = way.texts().t(crate::key::SOFTWARE_IMAGE_NOT_KEPT);
            way.say(said, crate::echo::Echo::Normal);
            return;
        };
        let back = self.keeping.clone();
        let wake = self.wake.clone();
        crate::pasting::image_then(move |read| {
            if back.send(Kept::of(read, &roots)).is_ok() {
                wake.call();
            }
        });
    }
}

/// The window a command of the registry the view names opens — the file
/// manager, the brain's page — started by this door. A command with no window
/// here is logged, as both doors logged it.
fn open_the_window_of(id: &str, way: &mut dyn Doorway) {
    match id {
        "files-window" => way.open_window(vec!["files".to_owned()]),
        "brain-open" => match crate::launch::buffer_component("RONOCTUA_BUFFER_BRAIN") {
            Some(component) => way.open_window(vec!["buffer".to_owned(), component]),
            None => {
                tracing::error!("RONOCTUA_BUFFER_BRAIN is not in the environment");
                let said = way.texts().t(crate::key::ECHO_NO_BUFFER);
                way.say(said, crate::echo::Echo::Normal);
            }
        },
        other => tracing::warn!(
            command = other,
            "the software view named a command with no window here"
        ),
    }
}

/// An address, in whatever opens links here — and, when nothing does, said.
fn open_the_page(url: &str, way: &mut dyn Doorway) {
    match option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) {
        Some(opener) => way.launch(crate::door::Launching::Program {
            program: opener.to_owned(),
            args: vec![url.to_owned()],
            cwd: None,
        }),
        None => {
            tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
            let said = way
                .texts()
                .t_with(crate::key::ECHO_NO_OPENED, &[("program", url)]);
            way.say(said, crate::echo::Echo::Normal);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::Effect;
    use crate::software::Opening;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    fn footer_way() -> crate::region::Recorder {
        crate::region::Recorder::new(
            std::rc::Rc::new(texts()),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn nothing_read() -> Reading {
        Reading::new(
            Readings::default(),
            None,
            "en-GB",
            crate::wake::Wake::new(|| {}),
        )
    }

    /// FAILS IF: the two doors do two things for one errand — the footer's
    /// doorway and the window's `Ui` are handed the same calls by the one
    /// handler, each doing them its own way.
    #[test]
    fn one_errand_is_the_same_call_in_both_doors() {
        let errands = || {
            vec![
                Errand::Say("said".to_owned()),
                Errand::Ask {
                    ask: crate::software::Ask::WebName,
                    initial: Some("Mail".to_owned()),
                },
                Errand::Launch("org.gimp.GIMP.desktop".to_owned()),
                Errand::Run("files-window"),
                Errand::Paste,
                Errand::Close,
            ]
        };
        // THE FOOTER'S DOOR writes each down, in order.
        let mut reading = nothing_read();
        let mut way = footer_way();
        for e in errands() {
            reading.carry_out(e, &mut way);
        }
        assert_eq!(
            way.owed,
            vec![
                Effect::Say("said".to_owned(), crate::echo::Echo::Normal),
                Effect::Ask(crate::line::Line::new(
                    "software-web-name",
                    false,
                    Some("Mail")
                )),
                Effect::Launch(crate::door::Launching::Entry(
                    "org.gimp.GIMP.desktop".to_owned()
                )),
                Effect::OpenWindow(vec!["files".to_owned()]),
                Effect::Paste,
                Effect::Close,
            ]
        );
        // AND THE WINDOW'S does them: its line, its question, its launch,
        // the file manager's window started by this door — never an order
        // for a footer that may be down — the clipboard asked, the window
        // closed.
        let mut ui = crate::door::Ui::for_tests(texts());
        let mut reading = nothing_read();
        for e in errands() {
            reading.carry_out(e, &mut ui);
        }
        assert_eq!(ui.asking.as_ref().map(|l| l.id), Some("software-web-name"));
        assert!(
            matches!(
                ui.launches(),
                [
                    crate::door::Launching::Entry(entry),
                    crate::door::Launching::Program { args, .. },
                ] if entry == "org.gimp.GIMP.desktop" && args == &["files".to_owned()]
            ),
            "{:?}",
            ui.launches()
        );
        assert!(ui.orders().is_empty(), "{:?}", ui.orders());
        assert!(ui.wants_the_clipboard());
        assert!(ui.quit);
    }

    /// FAILS IF: a page the view opens keeps the page of the one before — the
    /// door is not told to start at the top (`Doorway::first_page`) and to
    /// follow the cursor from there (`Doorway::follow`), and a page's title
    /// can open scrolled away.
    #[test]
    fn a_page_that_opens_starts_at_its_top() {
        let t = texts();
        let mut reading = nothing_read();
        let (mut view, _) = View::fresh(Opening::Home, &reading.readings, &t);
        let mut way = footer_way();
        // `↓` walks the first page: no new page, no top.
        let _ = reading.pressed(&mut view, Press::Down, &mut way);
        assert!(!way.owed.contains(&Effect::FirstPage), "{:?}", way.owed);
        // `Esc` from a page opened on the first one goes back: a page.
        let (mut view, _) = View::fresh(Opening::Trials, &reading.readings, &t);
        let mut way = footer_way();
        let _ = reading.pressed(&mut view, Press::Escape, &mut way);
        assert_eq!(way.owed, vec![Effect::FirstPage, Effect::Follow]);
    }

    /// FAILS IF: a wake that brought nothing says something changed — the
    /// door would draw a frame for every empty wake — or one that brought
    /// something (the copied image, here not an image) says nothing, and the
    /// sentence is never said.
    #[test]
    fn a_wake_says_what_it_brought() {
        let t = texts();
        let mut reading = nothing_read();
        let (mut view, _) = View::fresh(Opening::Home, &reading.readings, &t);
        let mut way = footer_way();
        assert_eq!(reading.woken(&mut view, &mut way), Woke::default());
        assert!(way.owed.is_empty(), "{:?}", way.owed);
        reading
            .keeping
            .send(Kept::NoImage)
            .expect("the reading hears its own channel");
        let woke = reading.woken(&mut view, &mut way);
        assert!(woke.changed, "{woke:?}");
        assert!(
            matches!(woke.errands.as_slice(), [Errand::Say(_)]),
            "{woke:?}"
        );
    }

    /// FAILS IF: the word that confirms a copy is an ordinary message in one
    /// door — it confirms a gesture, and goes sooner, in both.
    #[test]
    fn a_copy_is_a_brief_word() {
        let mut reading = nothing_read();
        let mut way = footer_way();
        reading.carry_out(Errand::Copy("x".to_owned()), &mut way);
        assert!(
            matches!(
                way.owed.as_slice(),
                [Effect::Say(_, crate::echo::Echo::Brief)]
            ),
            "{:?}",
            way.owed
        );
    }

    /// FAILS IF: the queue's list is not drawn while a job of ours runs, or
    /// once it ended — the row would go on showing its progress, or nothing
    /// would say it stopped.
    #[test]
    fn a_job_of_ours_is_drawn_while_it_runs_and_when_it_ends() {
        use ronoctua_transfers::{Engine, Job, Stage, Work};
        let job = |stage| {
            let mut j = Job::fresh_one(
                7,
                Engine::Software,
                Work::Software {
                    order: Box::new(ronoctua_software::Order::RefreshPins),
                    label: "x".to_owned(),
                },
            );
            j.stage = stage;
            j
        };
        let mut reading = nothing_read();
        assert!(!reading.jobs(None, &[]), "no job, nothing drawn");
        assert!(reading.jobs(None, &[job(Stage::Running)]), "a job running");
        assert!(reading.jobs(None, &[]), "the job that just stopped");
        assert!(!reading.jobs(None, &[]), "and then nothing");
    }
}
