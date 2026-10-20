//! THE FILE MANAGER IN THE FOOTER'S REGION — `LEADER . .`.
//!
//! A [`Region`] laid out in sections ([`Shape::Sections`]): the sidebar, the
//! listing and the two panels side by side, the strip pinned under them,
//! pictures in the panels and a drag that carries entries out of the footer.
//! The footer lays the sections onto its lines with its own arithmetic and
//! asks what each part is ([`Sectioned`]); what the manager DOES — a key, a
//! click, a command, an answer, a thread's report — is carried out here, by
//! the manager both doors hold ([`Manager::carry_out`]), over the footer's
//! doorway.
//!
//! # What it keeps between its opens
//!
//! The footer's process opens and closes the manager many times, and some of
//! what it holds outlives every open ([`Kept`]): the directory's watch and the
//! git line's keeper are RESTED when it closes and pointed again when it
//! opens — tearing their threads down to raise them again is churn with
//! something to get wrong in between — the disks are watched from the first
//! open on (a stick plugged in while it is closed is there when it opens),
//! and who the numeric ids belong to and how far this machine is from UTC are
//! read once. What the kept watchers say while it is closed goes nowhere.
//!
//! # What it still has to say once it closed
//!
//! An errand started from the manager runs on a thread of its own — a lock
//! or an eject udisks has not answered, a change as root, a seal handed to
//! the queue, a writer of the facts — and the manager may close before it
//! answers: `Esc`, or the footer losing the keyboard. Its answer is still
//! the footer's to say, as the footer always said it with no view open
//! (`Manager::reported_closed`). So the closed region LINGERS
//! ([`Region::lingers`]): the host keeps it, unshown, and tells it what its
//! queue brings, until the last errand's thread let go of the report sink
//! (`Errands`) and nothing is left on the queue (`Outstanding`). What was on
//! the queue when it closed is said as it closes.
//!
//! # A photograph's manager
//!
//! The footer's camera has no loop to hear a thread on, and draws what it was
//! asked for: no thread is raised, nothing is watched, no work is handed over
//! (`Manager::hands_off`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use crate::fields::{Axis, Hit, Pane, Panes, Room, Row, Section, Target};
use crate::files_manager::{Door, Manager, Panels};
use crate::region::{Closed, Doorway, Entry, Framing, Region, Seen, Shape, Taken};
use crate::report::Report;
use crate::sections::Sectioned;
use crate::wake::Out;

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "files";

/// The manager as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What a command does with the region closed.
fn closed(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        // `LEADER . .` — the file manager, in the footer's expanded region.
        "files-open" => Closed::Opens,
        // `LEADER . w` — the same manager in a window of its own, on the
        // folder it starts on. A WINDOW AND NOT A SECOND IMPLEMENTATION: it
        // re-execs this binary with `files`, and what draws there is the
        // same `files::View`. The region has a ceiling in lines (invariant
        // 14), which is right for a footer and not for the job people open a
        // file manager to do.
        "files-window" => {
            way.open_window(vec!["files".to_owned()]);
            Closed::Taken
        }
        _ => Closed::NotMine,
    }
}

/// Opens the manager, for the command that asked — the footer asked whether
/// the region is free first.
fn open(_command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(FilesRegion::open(way))
}

/// What a thread of the manager hands back, on the region's one queue.
enum Came {
    /// A report of its work, a watch, the disks or the git line.
    Report(Report),
    /// What the panels' threads came back with.
    Panels(crate::inspection::Heard),
    /// A picture of the globe.
    Globe(crate::globe::Beat),
    /// A thumbnail, or a frame of one that moves.
    Thumb(crate::thumbs::Made),
    /// A frame of the film under the pointer.
    Tour(crate::motion::Beat),
}

/// Where the kept watchers hand what they heard: the open region's queue,
/// and nowhere while it is closed.
type Reach = Arc<Mutex<Option<Out<Came>>>>;

/// WHAT AN OPEN STILL HAS ON ITS WAY — what a closed region's
/// [`Region::lingers`] reads.
#[derive(Debug, Default)]
struct Outstanding {
    /// Handed to the queue and not yet taken off it — counted BEFORE it is
    /// sent, so nothing can be on the queue that the count does not hold.
    queued: AtomicUsize,
    /// The report sink's last holder let go of it ([`Errands`]): nothing
    /// more of this open's work can come back.
    let_go: AtomicBool,
}

impl Outstanding {
    /// Whether nothing is on its way any more: every errand let go of the
    /// report sink, and the queue is empty.
    fn settled(&self) -> bool {
        // THE ORDER IS THE PROOF: an errand counts its last report before it
        // lets go, so a sink seen let go is a count that already holds it.
        self.let_go.load(Ordering::SeqCst) && self.queued.load(Ordering::SeqCst) == 0
    }
}

/// THE REPORT SINK OF THIS OPEN'S ERRANDS — what the manager hands every
/// thread its work goes to (`Manager::back`).
///
/// Held by the manager while it is open, and by each errand's thread while
/// its work runs; the manager lets go as it closes. Dropped by the LAST of
/// them, it says so ([`Outstanding::let_go`]) and wakes the footer: that is
/// the moment a closed manager has nothing more to say.
struct Errands {
    /// The open's queue.
    to: Out<Came>,
    /// What is still on its way.
    outstanding: Arc<Outstanding>,
    /// The region's wake, rung when the last holder lets go.
    wake: crate::wake::Wake,
}

impl Errands {
    /// Hands a report to the queue.
    fn hand(&self, report: Report) -> bool {
        self.to.send(Came::Report(report)).is_ok()
    }
}

impl Drop for Errands {
    fn drop(&mut self) {
        self.outstanding.let_go.store(true, Ordering::SeqCst);
        self.wake.call();
    }
}

/// Hands `came` to whoever is open now — `true` when nobody is, as the
/// watcher's thread goes on watching for the next open.
fn reach_with(reach: &Reach, came: Came) -> bool {
    match reach.lock().ok().and_then(|r| r.clone()) {
        Some(out) => out.send(came).is_ok(),
        None => true,
    }
}

/// WHAT THE MANAGER KEEPS BETWEEN ITS OPENS, in the footer's process — see
/// the module.
struct Kept {
    /// The directory on screen reporting itself — rested while closed.
    notice: Option<ronoctua_files::notice::Notice>,
    /// The git line's keeper — rested while closed.
    git: Option<crate::git_line::Keeper>,
    /// Where those two, and the disks, hand what they heard.
    reach: Reach,
    /// Whether the disks' thread is up: raised at most once, and it outlives
    /// every open.
    disks: bool,
    /// Who the numeric ids belong to, read once: `/etc/passwd` and
    /// `/etc/group` do not change while a listing is on screen.
    roster: ronoctua_files::who::Roster,
    /// How far ahead of UTC this machine is, read once from `/etc/localtime`.
    zone: ronoctua_files::when::Zone,
}

impl Kept {
    /// What the first open finds.
    fn first() -> Self {
        Self {
            notice: None,
            git: None,
            reach: Arc::new(Mutex::new(None)),
            disks: false,
            roster: ronoctua_files::who::Roster::read_from(&ronoctua_files::who::Files::default()),
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        }
    }
}

thread_local! {
    /// The footer's thread's — the one the footer's loop and its regions live
    /// on.
    static KEPT: RefCell<Option<Kept>> = const { RefCell::new(None) };
}

/// THE FILE MANAGER IN THE FOOTER'S REGION — see the module.
pub struct FilesRegion {
    /// The view and what stands beside it, as both doors hold it.
    manager: Manager,
    /// Where this open's threads hand what they made.
    came: mpsc::Receiver<Came>,
    /// What is still on its way to `came`.
    outstanding: Arc<Outstanding>,
    /// It closed, and lingers while an errand of it is out.
    closed: bool,
    /// What outlives this open, given back when it closes.
    kept: Option<Kept>,
}

impl FilesRegion {
    /// The manager on the folder it starts at — `RONOCTUA_FILES_START`,
    /// relative to home, a machine's preference (invariant 13) and never a
    /// literal; a name that is not there lands in the home rather than on an
    /// error.
    fn open(way: &mut dyn Doorway) -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let preferred = std::env::var("RONOCTUA_FILES_START").ok();
        let start = ronoctua_files::listing::start_at(home.as_deref(), preferred.as_deref());
        tracing::info!(dir = %start.display(), "file manager opened");
        // The trash's root, resolved ONCE and handed over — `$XDG_DATA_HOME`
        // does not move while a listing is up.
        let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
        let bin = ronoctua_files::trash::Bin::of_home(data.as_deref(), home.as_deref());
        let store = ronoctua_files::sidebar::store::config_dir();
        let side = crate::file_picker::sidebar_of(home.as_deref(), bin.as_ref(), store.clone());
        Self::at(&start, bin, side, store.as_deref(), way)
    }

    /// The manager on `start`, with the trash `bin`, the sidebar `side` and
    /// where the sidebar persists (`store`) — what [`FilesRegion::open`]
    /// found, or what a test hands it.
    fn at(
        start: &std::path::Path,
        bin: Option<ronoctua_files::trash::Bin>,
        side: ronoctua_files::sidebar::Sidebar,
        store: Option<&std::path::Path>,
        way: &mut dyn Doorway,
    ) -> Self {
        let photographing = way.photographing();
        let mut kept = KEPT
            .with(|k| k.borrow_mut().take())
            .unwrap_or_else(Kept::first);

        // THIS OPEN'S QUEUE: every thread hands what it made here and wakes
        // the footer's loop for this region (`Region::woken`) — counted on
        // its way in, so a closed region knows when nothing more is coming
        // (`Outstanding`).
        let (sender, came) = mpsc::channel::<Came>();
        let wake = way.wake();
        let outstanding = Arc::new(Outstanding::default());
        let counted = Arc::clone(&outstanding);
        let ring = wake.clone();
        let out = Out::new(move |c| {
            counted.queued.fetch_add(1, Ordering::SeqCst);
            let heard = sender.send(c).is_ok();
            if !heard {
                counted.queued.fetch_sub(1, Ordering::SeqCst);
            }
            ring.call();
            heard
        });
        // AND THE ERRANDS' REPORT SINK, which says when its last holder let
        // go of it (`Errands`).
        let errands = Errands {
            to: out.clone(),
            outstanding: Arc::clone(&outstanding),
            wake,
        };
        let back = Out::new(move |r| errands.hand(r));
        if let Ok(mut r) = kept.reach.lock() {
            *r = (!photographing).then(|| out.clone());
        }

        let panels = if photographing {
            Panels::none()
        } else {
            Panels::raise(
                &out.map(Came::Panels),
                out.map(Came::Globe),
                &out.map(Came::Thumb),
                out.map(Came::Tour),
            )
        };
        if !photographing {
            // THE DIRECTORY IS WATCHED, raised at most once and rested between
            // opens: what the footer's one-second `stat` cannot see — a file
            // rewritten in place, a permission changed, a `vfat` stick's
            // two-second stamps. `for_listing`: the /private vault's status
            // too, which says what the listing may read and how its row is
            // drawn.
            if kept.notice.is_none() {
                let reach = Arc::clone(&kept.reach);
                match ronoctua_files::notice::Notice::for_listing(store, move || {
                    reach_with(&reach, Came::Report(Report::Stirred))
                }) {
                    Ok(n) => kept.notice = Some(n),
                    // NOT FATAL and not worth a sentence: the footer's second
                    // still reads the directory's mtime (`Region::tick`).
                    Err(e) => {
                        tracing::warn!(error = %e, "no directory watcher; the one-second stat is on its own");
                    }
                }
            }
            // THE GIT LINE'S KEEPER, raised the first time and kept.
            if kept.git.is_none() {
                let reach = Arc::clone(&kept.reach);
                kept.git = Some(crate::git_line::Keeper::new(move |line| {
                    reach_with(&reach, Came::Report(Report::Git(Box::new(line))))
                }));
            }
        }
        let view = crate::files::View::at(
            start,
            kept.roster.clone(),
            kept.zone.clone(),
            bin,
            // THE FOOTER'S CATALOGUE arrives as soon as the region is open
            // (`Region::heard`), before any key.
            std::sync::Arc::new(Vec::new()),
            way.words(),
            side,
        );
        let mut region = Self {
            manager: Manager {
                view,
                catalogue: std::sync::Arc::new(Vec::new()),
                back,
                notice: kept.notice.take(),
                git: kept.git.take(),
                panels,
                in_place: false,
                hands_off: photographing,
                door: Door::Footer,
                vault: None,
                planted: None,
            },
            came,
            outstanding,
            closed: false,
            kept: None,
        };
        // THE DISKS ARE WATCHED from the first open on, and only once: at
        // startup it would open a bus connection for a view nobody asked
        // for, and per open it would leave one running per open.
        if !photographing && !kept.disks {
            kept.disks = true;
            let reach = Arc::clone(&kept.reach);
            if let Err(e) = ronoctua_disks::watch(move |rack| {
                let _ = reach_with(&reach, Came::Report(Report::Racked(Box::new(rack))));
            }) {
                // NOT FATAL, and not silent either: the sidebar reads once
                // when it opens and does not refresh by itself. A stick
                // plugged in that never appears, with nothing said, is the
                // silent fallback this project refuses.
                tracing::warn!(error = %e, "no disk watcher");
                let said = way.texts().t_with(
                    crate::key::FILES_DISKS_NOT_WATCHING,
                    &[("why", e.to_string().as_str())],
                );
                way.say(said, crate::echo::Echo::Normal);
            }
        }
        region.kept = Some(kept);
        // AND THE WATCH IS POINTED AT WHERE THE VIEW OPENED, here and not only
        // from the next key: on a machine with no disks' bus nothing else
        // would ever point it.
        region.manager.follow_the_place();
        region
    }

    /// What this open keeps, given back for the next — the watchers rested,
    /// pointing nowhere.
    fn give_back(&mut self) {
        self.manager.rest();
        let Some(mut kept) = self.kept.take() else {
            return;
        };
        if let Ok(mut r) = kept.reach.lock() {
            *r = None;
        }
        kept.notice = self.manager.notice.take();
        kept.git = self.manager.git.take();
        // A THREAD THAT IS ENDING keeps nothing: its keep is going with it.
        let _ = KEPT.try_with(|k| *k.borrow_mut() = Some(kept));
    }

    /// The next thing on the queue, taken off its count.
    fn next_came(&self) -> Option<Came> {
        let came = self.came.try_recv().ok()?;
        self.outstanding.queued.fetch_sub(1, Ordering::SeqCst);
        Some(came)
    }

    /// What the queue brought with the manager CLOSED: each report said as
    /// a closed manager says it (`Manager::reported_closed`), and what the
    /// panels made let go — nobody will look at it.
    fn heard_closed(&self, way: &mut dyn Doorway) {
        while let Some(came) = self.next_came() {
            match came {
                Came::Report(r) => Manager::reported_closed(r, way),
                Came::Panels(_) | Came::Globe(_) | Came::Thumb(_) | Came::Tour(_) => {}
            }
        }
    }
}

impl Drop for FilesRegion {
    fn drop(&mut self) {
        self.give_back();
    }
}

impl Region for FilesRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// No rows of its own: a region in sections is laid onto the lines by the
    /// footer's sectioned arithmetic ([`Region::sections`]).
    fn rows(&self, _: &Seen<'_>) -> Vec<crate::region::Row> {
        Vec::new()
    }

    /// THE MANAGER TEACHES ITS OWN KEYS — while it teaches at all
    /// ([`Region::teaches`]).
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.manager.view.hint(seen.texts)
    }

    /// Only while it is plain: filtering and the menu have their own lines —
    /// and then it teaches nothing, and the line says what is typed.
    fn teaches(&self) -> bool {
        let view = &self.manager.view;
        view.plain() && !view.filtering()
    }

    /// ASKED OF THE VIEW, because the answer is the VISUAL position and the
    /// view is what knows which list is on screen — with a menu up, the
    /// chosen row of the menu, never the listing nobody can see.
    fn cursor(&self) -> Option<usize> {
        self.manager.view.visual_cursor()
    }

    /// Its keys are NOT commands of the registry — they navigate a listing
    /// (`files::OF_THE_VIEW`) — and every one is the view's while it holds
    /// the keyboard: it holds a directory somebody navigated to, a set of
    /// marks and maybe an armed confirmation.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let step = self.manager.view.key(key);
        let _ = self.manager.carry_out(step, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        // CLOSED, IT TAKES NOTHING: what it still does is say its errands'
        // answers (`Region::lingers`).
        if self.closed {
            return Taken::No;
        }
        match id {
            // `LEADER . w` — the same manager in a window, on the folder this
            // one is on.
            "files-window" => {
                way.open_window(vec![
                    "files".to_owned(),
                    self.manager.view.watched().to_string_lossy().into_owned(),
                ]);
                Taken::Yes
            }
            // `LEADER . .` — the command that opened it, handed over as it
            // opens, with nothing left to do: the manager is on the folder it
            // starts at. AGAIN, with the manager open, it is the footer's to
            // refuse, as every opening is while the region is held
            // (`Footer::region_is_free`) — so it is never taken here.
            "files-open" => Taken::No,
            _ => Taken::No,
        }
    }

    /// A region in sections is clicked through its target
    /// ([`Sectioned::click`]); it has no row of its own to click.
    fn clicked(&mut self, _: usize, _: &mut dyn Doorway) {}

    /// A question of a CLOSED manager starts nothing: its view is gone.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        if self.closed {
            return;
        }
        let _ = self.manager.answered(id, text, way);
    }

    fn unanswered(&mut self, id: &'static str, way: &mut dyn Doorway) {
        self.manager.unanswered(id);
        way.repaint();
    }

    /// THE CLIPBOARD, into the filter or the question's query — the window's
    /// own `files::View::pasted`, on the same view.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let step = self.manager.view.pasted(text);
        let _ = self.manager.carry_out(step, way);
    }

    /// Whether files dragged over the manager may land where they are.
    fn takes_drops(&self) -> bool {
        self.manager.view.landing_now()
    }

    fn dropped(&mut self, paths: &[PathBuf], _copies: bool, way: &mut dyn Doorway) -> bool {
        tracing::info!(dropped = paths.len(), "files were let go over the listing");
        let step = self.manager.view.dropped(paths.to_vec());
        let _ = self.manager.carry_out(step, way);
        true
    }

    /// What its threads made, from its queue — closed, only what a closed
    /// manager still says ([`FilesRegion::heard_closed`]).
    fn woken(&mut self, way: &mut dyn Doorway) {
        if self.closed {
            self.heard_closed(way);
            return;
        }
        while let Some(came) = self.next_came() {
            match came {
                Came::Report(r) => {
                    let _ = self.manager.reported(r, way);
                }
                Came::Panels(h) => self.manager.panels_heard(h, way),
                Came::Globe(b) => {
                    if self.manager.view.saw_globe(&b) {
                        way.repaint();
                    }
                }
                Came::Thumb(m) => {
                    if self.manager.view.thumbed(m) {
                        way.repaint();
                    }
                }
                Came::Tour(b) => {
                    if self.manager.view.played_tour(&b) {
                        way.repaint();
                    }
                }
            }
        }
    }

    /// What the footer heard: the jobs that ended since its last list, as
    /// reports (`jobs::Ended`, compared by the footer against ITS list, so a
    /// job that ended while the manager was closed is not told again when it
    /// opens — a failure the footer said itself), and the footer's catalogue.
    ///
    /// Closed, nothing: what ended is the footer's to say, and the view that
    /// would read its folder again is gone.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if self.closed {
            return;
        }
        if let Some(crate::jobs::Ended(reports)) = news.downcast_ref::<crate::jobs::Ended>() {
            for report in reports {
                let _ = self.manager.reported(report.clone(), way);
            }
        } else if let Some(catalogue) = news.downcast_ref::<crate::launch::Catalogue>() {
            self.manager.catalogue = Arc::clone(catalogue);
            self.manager.view.catalogued(Arc::clone(catalogue));
        }
    }

    fn shape(&self) -> Shape {
        Shape::Sections
    }

    /// AND THE DIRECTORY, on the footer's second. One `stat` against the
    /// directory's mtime: something else moved a file into it — over NFS,
    /// sshfs or SMB, where inotify hears nothing and the mtime is the only
    /// thing that moves. AND THE SIDEBAR'S FILE: `LEADER a` pins an
    /// application without this view being open.
    ///
    /// Closed, no `stat`: nobody looks at the folder.
    fn tick(&mut self, _now: std::time::Instant, _way: &mut dyn Doorway) -> bool {
        if self.closed {
            return false;
        }
        let view = &mut self.manager.view;
        let changed = view.stale();
        let bar = view.sidebar_stale();
        if changed {
            view.reread();
        }
        if bar {
            view.reread_sidebar();
        }
        changed || bar
    }

    /// It closes: its watchers rest, and what it keeps waits for the next
    /// open. The panels' threads go with the view; the errands' threads are
    /// left the only holders of the report sink, and what the queue already
    /// holds is said now — a lock that answered as `Esc` landed is still
    /// this door's to say.
    fn closing(&mut self, way: &mut dyn Doorway) {
        self.give_back();
        self.closed = true;
        self.manager.panels = Panels::none();
        self.manager.back = Out::new(|_| false);
        self.heard_closed(way);
    }

    /// CLOSED WITH AN ERRAND OUT — see the module: until the last errand's
    /// thread let go of the report sink and the queue is empty.
    fn lingers(&self) -> bool {
        self.closed && !self.outstanding.settled()
    }

    fn sections(&self) -> Option<&dyn Sectioned> {
        Some(self)
    }

    fn sections_mut(&mut self) -> Option<&mut dyn Sectioned> {
        Some(self)
    }

    fn before_frame(&mut self, framing: &Framing) {
        let Manager { view, panels, .. } = &mut self.manager;
        panels.look(view, framing.columns, framing.room, framing.scale);
    }

    /// The pictures the frame replaced are up now — after the paint, because
    /// what asks for thumbnails runs before it.
    fn after_paint(&mut self) {
        self.manager.view.thumbs_uploaded();
    }

    /// THE GLOBE IS ASKED FOR AFTER THE FRAME: what its stage draws is the
    /// box the LAYOUT measured, and the layout is this frame — and so is a
    /// shape turned by the hand.
    fn after_frame(&mut self) {
        let Manager { view, panels, .. } = &mut self.manager;
        panels.turn(view);
    }

    /// THE WALKING NAME'S CLOCK, while a name too long for its column walks:
    /// five cells a second, and nothing at all when none is walking.
    fn clock(&self) -> Option<std::time::Duration> {
        self.manager.view.sliding().then_some(crate::slide::STEP)
    }

    fn clocked(&mut self, now: std::time::Instant) -> bool {
        self.manager.view.slide(now)
    }

    /// THE LANGUAGE MOVED, and the manager speaks it — the facts panel too,
    /// which kept the sheet it had read in the old one.
    fn speak(&mut self, texts: std::rc::Rc<ronoctua_i18n::Texts>) {
        self.manager.view.speak(texts);
    }

    /// IT GOES WITH THE KEYBOARD, and it is the one that loses the most: a
    /// directory, a set of marks, an armed confirmation. Left open without
    /// the keyboard it would be a listing that answers nothing, with an armed
    /// `delete` nobody can see is armed.
    fn closes_without_the_keyboard(&self) -> bool {
        true
    }
}

impl Sectioned for FilesRegion {
    fn panes(&self, width: u16) -> Panes {
        self.manager.view.panes(width)
    }
    fn plain(&self) -> bool {
        self.manager.view.plain()
    }
    fn chrome_rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        self.manager.view.chrome_rows(width, texts)
    }
    fn chrome_lines(&self) -> usize {
        self.manager.view.chrome_lines()
    }
    fn sections(
        &self,
        chrome: &[Row],
        take: usize,
        width: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Section> {
        self.manager.view.sections(chrome, take, width, texts)
    }
    fn strip_rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        self.manager.view.strip_rows(width, texts)
    }
    fn modal_len(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Option<usize> {
        self.manager.view.modal_len(width, texts)
    }
    fn region_len(&self, chrome: usize, width: u16) -> Option<usize> {
        self.manager.view.region_len(chrome, width)
    }
    fn list_offset(&self) -> usize {
        self.manager.view.list_offset()
    }
    fn scroll_listing_to(&mut self, to: usize) {
        self.manager.view.scroll_listing_to(to);
    }
    fn hit(
        &self,
        width: u16,
        room: usize,
        line: usize,
        cell: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Target> {
        self.manager.view.hit(width, room, line, cell, texts)
    }
    fn pane_at(&self, width: u16, room: usize, line: usize, cell: u16) -> Option<Pane> {
        self.manager.view.pane_at(width, room, line, cell)
    }
    fn band_row(&self, width: u16, room: usize, line: usize, cell: u16) -> Option<(Pane, usize)> {
        self.manager.view.band_row(width, room, line, cell)
    }
    fn band_belongs(&self, pane: Pane) -> bool {
        self.manager.view.band_belongs(pane)
    }
    fn takes_at(&self, width: u16, room: usize, line: usize, cell: u16) -> bool {
        self.manager.view.takes_at(width, room, line, cell)
    }
    fn grid_lit_at(&self, width: u16, room: usize, line: usize, cell: u16) -> Option<Hit> {
        self.manager.view.grid_lit_at(width, room, line, cell)
    }
    fn lit_at(&self, width: u16, room: usize, line: usize, cell: u16) -> crate::sections::LitAt {
        self.manager.view.lit_at(width, room, line, cell)
    }
    fn band_cells_at(&self, width: u16, room: usize, line: usize, cell: u16) -> Option<(u16, u16)> {
        self.manager.view.band_cells_at(width, room, line, cell)
    }
    fn band_lines_at(&self, width: u16, room: usize, line: usize, cell: u16) -> usize {
        self.manager.view.band_lines_at(width, room, line, cell)
    }
    fn pointing(&self, pane: Option<Pane>) {
        self.manager.view.pointing(pane);
    }
    fn pointing_at(&self, row: Option<usize>) {
        self.manager.view.pointing_at(row);
    }
    fn pointing_globe(&self, on: bool) {
        self.manager.view.pointing_globe(on);
    }
    fn hover_thumb(&mut self, width: u16, room: usize, at: Option<(usize, u16)>) {
        self.manager.view.hover_thumb(width, room, at);
    }
    fn drag_to(&mut self, pane: Pane, axis: Axis, fraction: f64, width: u16, room: Room) -> bool {
        self.manager.view.drag_to(pane, axis, fraction, width, room)
    }
    fn wheel(&mut self, which: Pane, by: i64, width: u16, room: usize) -> bool {
        self.manager.view.wheel(which, by, width, room)
    }
    fn pressed(
        &mut self,
        width: u16,
        room: usize,
        line: usize,
        cell: u16,
        mods: ronoctua_keymap::Mods,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        self.manager
            .view
            .pressed(width, room, line, cell, mods, texts)
    }
    fn moved_to(&mut self, width: u16, room: usize, line: usize, cell: u16) -> bool {
        self.manager.view.moved_to(width, room, line, cell)
    }
    fn let_go(&mut self) -> Option<Target> {
        self.manager.view.let_go()
    }
    fn gave_up(&mut self) -> bool {
        self.manager.view.gave_up()
    }
    fn shape_here(&self) -> bool {
        self.manager.view.shape_here()
    }
    fn spin(&mut self, right: f32, down: f32) -> bool {
        self.manager.view.spin(right, down)
    }
    fn pan_globe(&mut self, right: f32, down: f32) -> bool {
        self.manager.view.pan_globe(right, down)
    }
    /// EVERYTHING THE MANAGER OWNS, through ONE resolution: the target says
    /// the section, the row inside it and the field, and this presses the key
    /// that goes with it — nothing a click does that the keyboard cannot.
    fn click(&mut self, target: &Target, held: ronoctua_keymap::Mods, way: &mut dyn Doorway) {
        let step = self.manager.view.click(target.row, &target.hit, held);
        let _ = self.manager.carry_out(step, way);
    }
    /// The right button: the menu, on what is under the pointer — `None` is
    /// an answer and not a failure: the empty directory still has a menu,
    /// about the directory itself.
    fn menu_at(&mut self, target: Option<&Target>, way: &mut dyn Doorway) {
        let step = self.manager.view.menu_at(target);
        let _ = self.manager.carry_out(step, way);
    }
    fn begun_carrying(&mut self) -> Option<(Vec<PathBuf>, bool)> {
        self.manager.view.begun_carrying()
    }
    fn carry_to(
        &mut self,
        width: u16,
        room: usize,
        line: usize,
        cell: u16,
        copies: bool,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        self.manager
            .view
            .carry_to(width, room, line, cell, copies, texts)
    }
    fn carrying_from_afar(&mut self, copies: bool) {
        self.manager.view.carrying_from_afar(copies);
    }
    fn carried_away(&mut self) -> bool {
        self.manager.view.carried_away()
    }
    fn landing_now(&self) -> bool {
        self.manager.view.landing_now()
    }
    fn sweep_box(&self, width: u16, room: usize) -> Option<(u16, u16, usize, usize)> {
        self.manager.view.sweep_box(width, room)
    }
    fn landing_box(&self, width: u16, room: usize) -> Option<(u16, u16, usize, usize)> {
        self.manager.view.landing_box(width, room)
    }
    fn focus_line(&self, width: u16, room: usize) -> Option<(Pane, usize)> {
        self.manager.view.focus_line(width, room)
    }
    fn focus_cells(&self, width: u16, room: usize) -> Option<(u16, u16)> {
        self.manager.view.focus_cells(width, room)
    }
    fn focus_lines(&self, width: u16, room: usize) -> usize {
        self.manager.view.focus_lines(width, room)
    }
    fn resting_line(&self, width: u16, room: usize) -> Option<usize> {
        self.manager.view.resting_line(width, room)
    }
    fn resting_cells(&self, width: u16, room: usize) -> Option<(u16, u16)> {
        self.manager.view.resting_cells(width, room)
    }
    fn resting_lines(&self, width: u16, room: usize) -> usize {
        self.manager.view.resting_lines(width, room)
    }
    fn pane(&self) -> Pane {
        self.manager.view.pane()
    }
    fn cursor_explains(&self) -> Option<ronoctua_files::mode::Explanation> {
        self.manager.view.cursor_explains()
    }
    fn film_trouble(&self) -> Option<ronoctua_files::mode::Explanation> {
        self.manager.view.film_trouble()
    }
    fn shape_said(&self) -> Option<ronoctua_files::mode::Explanation> {
        self.manager.view.shape_said()
    }
    fn fold_is_shown(&self, shown: bool) {
        self.manager.view.fold_is_shown(shown);
    }
    fn fold_focused(&self) -> bool {
        self.manager.view.fold_focused()
    }
    fn transport_showing(&self) -> bool {
        self.manager.view.transport_showing()
    }
    fn preview_pixels(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.manager.view.preview_pixels()
    }
    fn live_frame(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.manager.view.live_frame()
    }
    fn preview_at_scale(&self, scale: i32) {
        self.manager.view.preview_at_scale(scale);
    }
    fn measured_preview(&self, wide: f32, tall: f32) {
        self.manager.view.measured_preview(wide, tall);
    }
    fn preview_key(&self) -> Option<String> {
        self.manager.view.preview_key()
    }
    fn preview_now(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.manager.view.preview_now()
    }
    fn globe_place(&self) -> Option<(usize, u16, usize)> {
        self.manager.view.globe_place()
    }
    fn measured_globe(&self, size: (u32, u32), from: u32, theme: usize) {
        self.manager.view.measured_globe(size, from, theme);
    }
    fn globe_key(&self) -> Option<String> {
        self.manager.view.globe_key()
    }
    fn globe_frame(&self) -> Option<ronoctua_render::image::Rgba> {
        self.manager.view.globe_frame()
    }
    fn grid_pictures(
        &self,
        width: u16,
        room: usize,
        lead: u16,
        cell: f32,
        line: f32,
        top: f32,
        page: ronoctua_tokens::theme::Ground,
    ) -> Vec<(String, ronoctua_render::image::Area)> {
        self.manager
            .view
            .grid_pictures(width, room, lead, cell, line, top, page)
    }
    fn thumb_keys(&self) -> Vec<String> {
        self.manager.view.thumb_keys()
    }
    fn thumb_moves(&self, key: &str) -> bool {
        self.manager.view.thumb_moves(key)
    }
    fn thumb_pixels(&self, key: &str) -> Option<&ronoctua_render::image::Rgba> {
        self.manager.view.thumb_pixels(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-region-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    /// The footer's doorway — a photograph's, so nothing of the machine is
    /// reached: no thread, no watch, no bus.
    fn way() -> Recorder {
        Recorder::new(
            std::rc::Rc::new(
                ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language"),
            ),
            100,
            true,
            crate::sudo::Gate::shut(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn opened(dir: &std::path::Path, w: &mut Recorder) -> FilesRegion {
        let mut prefs = ronoctua_files::sidebar::Preferences::fresh();
        prefs.showing = false;
        let side = ronoctua_files::sidebar::Sidebar::of(&[], None, None, prefs, None);
        FilesRegion::at(dir, None, side, None, w)
    }

    /// FAILS IF: the closed manager's commands do anything but what the two
    /// keys do — `LEADER . .` opens the region, `LEADER . w` a window — or it
    /// claims a command that is not its own.
    #[test]
    fn the_closed_manager_answers_its_two_keys() {
        let mut w = way();
        assert_eq!((ENTRY.closed)("files-open", None, &mut w), Closed::Opens);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
        assert_eq!((ENTRY.closed)("files-window", None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["files".to_owned()])]);
        assert_eq!((ENTRY.closed)("stick-open", None, &mut w), Closed::NotMine);
        assert_eq!(ENTRY.id, ID);
    }

    /// FAILS IF: the open manager is not a region in sections, opens its
    /// window anywhere but on the folder it is on, takes its own opening
    /// command (the footer refuses it, the region being held), stays when
    /// the keyboard goes, or asks for a clock with nothing walking.
    #[test]
    fn the_open_manager_is_a_region_in_sections() {
        let dir = scratch("open");
        std::fs::write(dir.join("a.txt"), b"x").expect("a file");
        let mut w = way();
        let mut r = opened(&dir, &mut w);
        assert_eq!(r.id(), ID);
        assert_eq!(r.shape(), Shape::Sections);
        assert!(Region::sections(&r).is_some());
        assert!(r.closes_without_the_keyboard());
        assert_eq!(r.clock(), None);
        let mut w = way();
        assert_eq!(r.command("files-open", None, &mut w), Taken::No);
        assert_eq!(r.command("files-window", None, &mut w), Taken::Yes);
        assert_eq!(
            w.owed,
            vec![Effect::OpenWindow(vec![
                "files".to_owned(),
                dir.to_string_lossy().into_owned()
            ])]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a key does not reach the view or the page does not follow the
    /// cursor it moved, or the hint is taught while a filter is being typed
    /// (the line says what is typed then).
    #[test]
    fn a_key_moves_the_view_and_the_page_follows() {
        let dir = scratch("keys");
        for n in 0..5 {
            std::fs::write(dir.join(format!("{n}.txt")), b"x").expect("a file");
        }
        let mut w = way();
        let mut r = opened(&dir, &mut w);
        let seen_texts =
            ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &seen_texts,
            columns: 100,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        assert!(r.teaches(), "a plain manager teaches nothing");
        assert!(!r.hint(&seen).is_empty(), "a plain manager's hint is empty");
        let before = r.cursor();
        let mut w = way();
        let down = ronoctua_keymap::Key::parse("<down>").expect("a key");
        assert_eq!(r.key(down, &mut w), Taken::Yes);
        assert_ne!(r.cursor(), before, "the key did not move the cursor");
        assert!(
            w.owed
                .contains(&Effect::Page(crate::region::Paging::Follow)),
            "{:?}",
            w.owed
        );
        let mut w = way();
        let letter = ronoctua_keymap::Key::parse("a").expect("a key");
        let _ = r.key(letter, &mut w);
        assert!(
            r.manager.view.filtering(),
            "a letter did not start the filter"
        );
        assert!(!r.teaches(), "the hint is taught over a filter");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: what the manager keeps between its opens is not given back
    /// when it closes — the next open would raise its watchers again — or is
    /// not taken by the next open.
    #[test]
    fn what_outlives_an_open_is_given_back() {
        let dir = scratch("kept");
        let mut w = way();
        let mut r = opened(&dir, &mut w);
        assert!(
            KEPT.with(|k| k.borrow().is_none()),
            "the open did not take it"
        );
        r.closing(&mut w);
        assert!(
            KEPT.with(|k| k.borrow().is_some()),
            "the close did not give it back"
        );
        drop(r);
        let again = opened(&dir, &mut w);
        assert!(
            KEPT.with(|k| k.borrow().is_none()),
            "the next open did not take it"
        );
        drop(again);
        assert!(
            KEPT.with(|k| k.borrow().is_some()),
            "the drop did not give it back"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: an errand's answer is lost because the manager closed before
    /// it came — one already on the queue as `Esc` landed, or one that comes
    /// after — or a report that needs the view is said with no view, or the
    /// closed manager is let go while an errand of it is out, or kept once
    /// the last one let go of the report sink; or a manager closed with
    /// nothing out lingers at all.
    #[test]
    fn an_errand_out_is_said_after_the_close() {
        let dir = scratch("lingers");
        let mut w = way();
        let mut r = opened(&dir, &mut w);
        // AN ERRAND'S THREAD holds the report sink, as `files::carry_out`
        // hands it one.
        let errand = r.manager.back.clone();
        assert!(errand.send(Report::Failed("no bus".to_owned())).is_ok());
        let mut w = way();
        r.closing(&mut w);
        assert_eq!(
            w.owed,
            vec![Effect::Say("no bus".to_owned(), crate::echo::Echo::Normal)],
            "what was on the queue as it closed was not said"
        );
        assert!(r.lingers(), "closed with an errand out, and let go");
        assert!(errand.send(Report::Done).is_ok());
        assert!(
            errand
                .send(Report::Said(ronoctua_files::mode::Explanation {
                    key: crate::key::FILES_UNLOCK_LOCKED,
                    args: vec![("name", "STICK".to_owned())],
                }))
                .is_ok()
        );
        let mut w = way();
        r.woken(&mut w);
        let [Effect::Say(words, crate::echo::Echo::Normal)] = w.owed.as_slice() else {
            panic!("the answer after the close: {:?}", w.owed);
        };
        assert!(words.contains("STICK"), "{words}");
        assert!(r.lingers(), "let go while its errand still runs");
        drop(errand);
        assert!(!r.lingers(), "kept after its last errand let go");
        let mut w = way();
        let mut idle = opened(&dir, &mut w);
        idle.closing(&mut w);
        assert!(w.owed.is_empty(), "{:?}", w.owed);
        assert!(!idle.lingers(), "closed with nothing out, and kept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a closed manager that lingers does anything but say its
    /// errands' answers — reads its folder on the footer's second, takes a
    /// command, or tells what the footer heard ended (the footer said that
    /// itself).
    #[test]
    fn a_closed_manager_answers_only_its_errands() {
        let dir = scratch("closed-quiet");
        let mut w = way();
        let mut r = opened(&dir, &mut w);
        let errand = r.manager.back.clone();
        r.closing(&mut w);
        assert!(r.lingers());
        // THE FOLDER MOVED: an open manager would read it again.
        std::fs::write(dir.join("new.txt"), b"x").expect("a file");
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::File::open(&dir)
            .and_then(|d| d.set_modified(later))
            .expect("the folder's mtime");
        let mut w = way();
        assert!(
            !r.tick(std::time::Instant::now(), &mut w),
            "a closed manager read its folder"
        );
        assert_eq!(r.command("files-window", None, &mut w), Taken::No);
        r.heard(
            &crate::jobs::Ended(vec![Report::Failed("ended".to_owned())]),
            &mut w,
        );
        assert!(w.owed.is_empty(), "a closed manager acted: {:?}", w.owed);
        drop(errand);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: what the footer heard ended does not reach the manager's one
    /// handler — a refusal with a name not said, a folder measured not kept —
    /// or the queue's bare list is taken for news of what ended: the footer
    /// compares the lists, once, against ITS last one, so a job that ended
    /// while the manager was closed is never told again when it opens.
    #[test]
    fn what_ended_is_heard_from_the_footer() {
        let dir = scratch("ended");
        let mut w = way();
        let mut r = opened(&dir, &mut w);
        let mut w = way();
        r.heard(
            &crate::jobs::Ended(vec![Report::Said(ronoctua_files::mode::Explanation {
                key: crate::key::FILES_COPIED,
                args: Vec::new(),
            })]),
            &mut w,
        );
        assert!(
            w.owed.iter().any(|e| matches!(e, Effect::Say(..))),
            "what ended was not said: {:?}",
            w.owed
        );
        let mut w = way();
        r.heard(&Vec::<ronoctua_transfers::Job>::new(), &mut w);
        assert!(
            w.owed.is_empty(),
            "the bare list was taken for news: {:?}",
            w.owed
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the footer's catalogue does not reach the view's "open with"
    /// and the handler's programs, or another kind of news is taken for it.
    #[test]
    fn the_footers_catalogue_is_heard() {
        let dir = scratch("catalogue");
        let mut w = way();
        let mut r = opened(&dir, &mut w);
        let id = "org.example.Viewer";
        let entry = ronoctua_apps::DesktopEntry {
            mime_types: vec!["image/png".to_owned()],
            id: id.to_owned(),
            name: id.to_owned(),
            description: String::new(),
            icon: id.to_owned(),
            categories: Vec::new(),
            command: (id.to_owned(), vec!["%f".to_owned()]),
            working_directory: None,
            terminal: false,
            startup_wm_class: None,
            key: ronoctua_apps::fold::fold(id),
        };
        let catalogue: crate::launch::Catalogue = std::sync::Arc::new(vec![entry]);
        r.heard(&catalogue, &mut w);
        assert_eq!(r.manager.catalogue.len(), 1);
        r.heard(&7_u32, &mut w);
        assert_eq!(r.manager.catalogue.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
