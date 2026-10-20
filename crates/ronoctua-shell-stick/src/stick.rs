//! `LEADER s z` — the installer on a USB stick: the latest version, the
//! drives it can go on, and the write.
//!
//! # Two doors, one view
//!
//! The shape every view of this kind has: this file knows what the rows are,
//! what a key does to them and what the threads read; `s z z` draws it in
//! the footer's region ([`crate::stick_region`]), `s z w` in a window
//! ([`crate::stick_window`]), and the file manager's sidebar opens the window
//! on the drive its `W` was pressed on. Both doors hold the view [`Open`]
//! and carry out its [`Errand`]s through ONE function, [`carry_out`] —
//! consent, the queue, and root asked in the footer's sudo bar (the
//! binary's `sudo_bar`).
//!
//! # What it reads, and on which thread
//!
//! A [`Feed`] holds the threads, and only while the view is open: GitHub's
//! latest release (on the person's pass for `api.github.com`, the consent row
//! first), the disk border's rack (udisks' own signal, never a clock), the
//! root writer's `progress.json` and the cache folder the image lands in
//! (inotify). Each leaves a [`Came`] and wakes the door.
//!
//! # The write, from here
//!
//! A drive is pressed → the question, `no` first and the cursor born on it
//! (the whole drive is erased, which cannot be undone) → `yes` → the sudo
//! door starts the root writer AT ONCE (`RootVerb::StickWrite`), asked in
//! the sudo bar when sudo has no fresh timestamp — the password, a key or a
//! finger, once, up front, never after an hour of download → the queue takes
//! the download
//! with the write armed (`Work::Stick`). The queue verifies, stages `ready`,
//! and the writer, which validated nothing yet, validates everything itself.
//!
//! # One write at a time
//!
//! The machine has one writer unit, one staging folder and one
//! `progress.json`. A `yes` while another write is under way would start
//! nothing (the unit is already running) and stage a second request for a
//! writer busy with the first — so before the password, the arm thread asks
//! the writer's last answer and the queue ([`busy_of`]) and refuses with a
//! sentence: a stick being written, a write waiting for its image. A writer
//! left waiting with no job to feed it (the queue restarted) is told to stop,
//! and the person to choose again.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ronoctua_render::layout;
use ronoctua_stick::{Judged, Phase, Release, Target, Unfit, Why};
use ronoctua_tokens as tokens;

use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::region::Doorway;
use crate::wake::Wake;

/// The keys this view teaches. The hint line is GENERATED from it, and the
/// consent row's own key joins it while that row is drawn.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "stick-down"),
    ("<up>", "stick-up"),
    ("RET", "stick-enter"),
    ("C-r", "stick-reread"),
];

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row that acts.
    Up,
    /// Down one row that acts.
    Down,
    /// Act on the row: let the hosts, download, choose a drive, answer.
    Enter,
    /// Ask GitHub again — and read the drives again when udisks did not
    /// answer. Nothing while the question is open: it is about the release
    /// it was asked with.
    Reread,
    /// Let the hosts (the consent row's key).
    Allow,
    /// Leave the question, or close.
    Escape,
}

/// A command of the registry, as the view's press.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "stick-down" => Press::Down,
        "stick-up" => Press::Up,
        "stick-enter" => Press::Enter,
        "stick-reread" => Press::Reread,
        _ => return None,
    })
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` is every level's way back.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    if crate::consent_rows::OF_THE_CONSENT
        .iter()
        .any(|(taught, _)| *taught == notation)
    {
        return Some(Press::Allow);
    }
    OF_THE_VIEW
        .iter()
        .find(|(taught, _)| *taught == notation)
        .and_then(|(_, id)| press_of(id))
}

/// What GitHub said of the latest release.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Latest {
    /// Not asked: the hosts are not let yet.
    #[default]
    Unasked,
    /// Asked, on a thread.
    Asking,
    /// This one.
    Got(Box<Release>),
    /// Refused, and why.
    Refused(Why),
}

/// What the view draws from — every reading in one place, so a camera can
/// hand a whole one in.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// The three hosts are let.
    pub consent: bool,
    /// The latest release.
    pub latest: Latest,
    /// The latest release's image is in the cache, verified: its digest.
    pub cached: Option<String>,
    /// The drives, judged; `None` until the first reading.
    pub drives: Option<Vec<Judged>>,
    /// udisks did not answer.
    pub disks_down: bool,
    /// What the root writer last said.
    pub progress: Option<ronoctua_stick::Progress>,
    /// Where images land; `None` with no cache folder.
    pub cache: Option<PathBuf>,
}

impl Readings {
    /// What the machine says now, before any thread answered.
    #[must_use]
    pub fn of_the_machine() -> Self {
        let consent = ronoctua_stick::HOSTS
            .iter()
            .all(|h| ronoctua_consent::allows(h));
        Self {
            consent,
            latest: if consent {
                Latest::Asking
            } else {
                Latest::Unasked
            },
            progress: ronoctua_stick::staging::read_progress(Path::new(
                ronoctua_stick::PROGRESS_FILE,
            )),
            cache: ronoctua_xdg::cache_dir().map(|d| ronoctua_stick::cache_dir_of(&d)),
            ..Self::default()
        }
    }

    /// The release, when it is known.
    #[must_use]
    pub fn release(&self) -> Option<&Release> {
        match &self.latest {
            Latest::Got(r) => Some(r),
            _ => None,
        }
    }

    /// Reads again whether the latest image is in the cache.
    pub fn reread_cache(&mut self) {
        self.cached = match (self.cache.as_deref(), self.release()) {
            (Some(dir), Some(r)) => ronoctua_stick::release::cached(dir, r),
            _ => None,
        };
    }
}

/// A write the person said yes to: what the queue is handed once the sudo
/// door started the writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Armed {
    /// The release.
    pub release: Release,
    /// The drive.
    pub target: Target,
}

/// The view OPEN, in either door — the footer's region
/// ([`crate::stick_region`]) or the window ([`crate::stick_window`]): the
/// view, what it reads, the threads that read it and the zone its dates are
/// said in. Both doors hold one and hand it every key, click and answer of
/// their threads; what the view asks for after them is carried out by
/// [`carry_out`], the same function in both.
pub struct Open {
    /// The view.
    pub view: View,
    /// What it reads.
    pub readings: Readings,
    /// Its threads; `None` when the door has no loop to hear them on (a
    /// photograph's).
    pub feed: Option<Feed>,
    /// The zone a date is said in.
    pub zone: ronoctua_files::when::Zone,
}

impl Open {
    /// The view over `readings`, on `drive` when one is named, its cursor
    /// settled at `columns`.
    #[must_use]
    pub fn over(
        readings: Readings,
        feed: Option<Feed>,
        drive: Option<String>,
        zone: ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Self {
        let mut view = View::on(drive);
        view.settle(&readings, &zone, columns, texts);
        Self {
            view,
            readings,
            feed,
            zone,
        }
    }

    /// The rows, at the door's width.
    #[must_use]
    pub fn rows(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        self.view.rows(&self.readings, &self.zone, columns, texts)
    }

    /// A key of the view, from the keyboard, the hint or a second click.
    pub fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self.view.press(
            press,
            &self.readings,
            &self.zone,
            way.columns(),
            way.texts(),
        );
        carry_out(errand, self, way);
    }

    /// A click on row `n`: the cursor there, and the second click is `RET`
    /// (invariant 19).
    pub fn clicked(&mut self, n: usize, way: &mut dyn Doorway) {
        let rows = self.rows(way.columns(), way.texts());
        if self.view.clicked(n, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// What the threads left — the drives, GitHub, the writer, the cache,
    /// the door — taken in, and what the view asks for after each.
    pub fn heard(&mut self, way: &mut dyn Doorway) {
        let came = self.feed.as_ref().map(Feed::take).unwrap_or_default();
        for c in came {
            let errand = self.view.heard(
                &mut self.readings,
                c,
                &self.zone,
                way.columns(),
                way.texts(),
            );
            carry_out(errand, self, way);
        }
        way.repaint();
    }

    /// GitHub asked again (when it is let), the cache read again — and the
    /// drives from a new watch when udisks did not answer — the cursor kept
    /// on what it stood on.
    pub fn reread(&mut self, columns: u16, texts: &ronoctua_i18n::Texts) {
        let feed = self.feed.as_mut();
        self.view
            .kept(&mut self.readings, &self.zone, columns, texts, |r| {
                reread(r, feed);
            });
    }
}

/// CARRIES OUT what the view asked for, on the line of the door it was asked
/// in — the footer's region and the window alike (`errands-are-handled-alike`
/// reads that both hand it the errand whole): consent, the queue, and root,
/// asked in the footer's sudo bar for either door ([`Doorway::sudo`]).
pub fn carry_out(errand: Errand, open: &mut Open, way: &mut dyn Doorway) {
    use crate::echo::Echo;
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Speak(key) => {
            let said = way.texts().t(key);
            way.say(said, Echo::Long);
        }
        Errand::Allow => match crate::consent_rows::allow(&ronoctua_stick::HOSTS, way.texts()) {
            Ok(()) => {
                open.readings.consent = true;
                open.reread(way.columns(), way.texts());
            }
            Err(said) => way.say(said, Echo::Long),
        },
        Errand::Reread => open.reread(way.columns(), way.texts()),
        Errand::Fetch(release) => {
            let said = fetch_said(&release, way.texts());
            way.say(said, Echo::Long);
        }
        Errand::Arm(armed) => {
            if let Some(said) = cannot_arm(way.texts()) {
                way.say(said, Echo::Long);
            } else if let Some(feed) = open.feed.as_ref() {
                // ROOT, ASKED IN THE FOOTER'S SUDO BAR — above this very
                // footer, or down its socket from a window, the verdict said
                // on the door that asked.
                feed.arm(*armed, way.sudo().gesture(crate::sudo::Asker::Stick));
                let said = way.texts().t(crate::key::STICK_STARTING);
                way.say(said, Echo::Normal);
            }
        }
        Errand::Door(why) => {
            let said = way.texts().t(crate::security_words::refusal(why));
            way.say(said, Echo::Long);
        }
    }
    way.repaint();
}

/// What a thread of the [`Feed`] left.
#[derive(Debug)]
pub enum Came {
    /// GitHub answered.
    Latest(Result<Release, Why>),
    /// The disk border read the drives.
    Rack(ronoctua_disks::Rack),
    /// udisks is not there.
    DisksDown,
    /// `progress.json` moved.
    Progress,
    /// The cache folder moved.
    Cache,
    /// The writer is started and the queue took the job (`true` when it took
    /// it; `false` when the queue is not answering).
    Queued(bool),
    /// No write was started: another is under way.
    Busy(Busy),
    /// The sudo door refused (the bar was cancelled, the password was
    /// wrong, the footer is not there…).
    Door(ronoctua_security::Why),
}

/// Why a write is not started now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    /// A stick is being written or read back.
    Writing,
    /// A write is armed: the writer waits while its image downloads.
    Waiting,
    /// The writer was waiting for an image nobody is downloading (the queue
    /// restarted): it was told to stop, and the person chooses again.
    Stopped,
    /// The queue does not answer, so nothing could feed a writer.
    QueueDown,
}

impl Busy {
    /// Every one — for the test that each has its sentence in every language.
    #[cfg(test)]
    pub const ALL: [Self; 4] = [Self::Writing, Self::Waiting, Self::Stopped, Self::QueueDown];

    /// Its sentence.
    #[must_use]
    pub const fn key(self) -> &'static str {
        use crate::key;
        match self {
            Self::Writing => key::STICK_BUSY_WRITING,
            Self::Waiting => key::STICK_BUSY_WAITING,
            Self::Stopped => key::STICK_BUSY_STOPPED,
            Self::QueueDown => key::TRANSFERS_OFFLINE,
        }
    }
}

/// Whether a new write may start, from the writer's last answer and whether
/// the queue holds an ARMED stick job that is not over. The writer's answer
/// is always final once its unit is gone (its `ExecStopPost`), so one that is
/// not final is a writer at work.
#[must_use]
pub fn busy_of(progress: Option<&ronoctua_stick::Progress>, armed_afoot: bool) -> Option<Busy> {
    match progress.map(|p| p.phase) {
        Some(Phase::Writing | Phase::Verifying) => Some(Busy::Writing),
        Some(Phase::Waiting) if !armed_afoot => Some(Busy::Stopped),
        Some(Phase::Waiting) => Some(Busy::Waiting),
        _ if armed_afoot => Some(Busy::Waiting),
        _ => None,
    }
}

/// [`busy_of`], read off the machine — off the loop, on the arm thread: the
/// queue is a socket. A writer waiting for nobody is told to stop here.
fn busy() -> Option<Busy> {
    let Ok(ronoctua_transfers::Say::Ledger { jobs }) =
        ronoctua_transfers::client::ask(&ronoctua_transfers::Ask::List)
    else {
        return Some(Busy::QueueDown);
    };
    let armed_afoot = jobs.iter().any(|j| {
        j.stage.afoot()
            && matches!(
                j.work,
                ronoctua_transfers::Work::Stick { write: Some(_), .. }
            )
    });
    let progress = ronoctua_stick::staging::read_progress(Path::new(ronoctua_stick::PROGRESS_FILE));
    let busy = busy_of(progress.as_ref(), armed_afoot);
    if busy == Some(Busy::Stopped) {
        let _ = ronoctua_stick::staging::cancel(Path::new(ronoctua_stick::STAGING));
    }
    busy
}

/// Leaves a thread's answer and wakes the door.
fn leave(inbox: &Arc<Mutex<Vec<Came>>>, wake: &Wake, came: Came) {
    if let Ok(mut i) = inbox.lock() {
        i.push(came);
    }
    wake.call();
}

/// The threads behind the view — only while it is open: dropping it lets
/// the disks' thread go at the next signal and closes the watches.
pub struct Feed {
    inbox: Arc<Mutex<Vec<Came>>>,
    wake: Wake,
    /// The drives' watch now; a reread raises a new one and lets this go.
    drives: Arc<AtomicBool>,
    _notices: Vec<ronoctua_files::notice::Notice>,
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.drives.store(false, Ordering::SeqCst);
    }
}

impl Feed {
    /// Raises the watches: the drives, the writer's progress, the cache.
    #[must_use]
    pub fn start(wake: Wake, cache: Option<&Path>) -> Self {
        let inbox: Arc<Mutex<Vec<Came>>> = Arc::new(Mutex::new(Vec::new()));
        let mut notices = Vec::new();
        let progress = Path::new(ronoctua_stick::PROGRESS_FILE);
        if let (Some(dir), Some(name)) = (progress.parent(), progress.file_name())
            && dir.is_dir()
        {
            let (inbox, wake) = (Arc::clone(&inbox), wake.clone());
            match ronoctua_files::notice::Notice::on_file(
                dir,
                &name.to_string_lossy(),
                ronoctua_files::notice::Pace::Coalesced,
                move || {
                    leave(&inbox, &wake, Came::Progress);
                    true
                },
            ) {
                Ok(n) => notices.push(n),
                Err(e) => tracing::info!(error = %e, "the writer's progress is not watched"),
            }
        }
        if let Some(dir) = cache
            && std::fs::create_dir_all(dir).is_ok()
        {
            let (inbox, wake) = (Arc::clone(&inbox), wake.clone());
            match ronoctua_files::notice::Notice::on_dirs(
                &[dir.to_path_buf()],
                &[],
                ronoctua_files::notice::Pace::Coalesced,
                move || {
                    leave(&inbox, &wake, Came::Cache);
                    true
                },
            ) {
                Ok(n) => notices.push(n),
                Err(e) => tracing::info!(error = %e, "the stick's cache is not watched"),
            }
        }
        let mut feed = Self {
            inbox,
            wake,
            drives: Arc::new(AtomicBool::new(false)),
            _notices: notices,
        };
        feed.watch_drives();
        feed
    }

    /// Watches the drives — every reading, a FAILED one included: udisks
    /// not answering is said (`Came::DisksDown`), never drawn as a reading
    /// that is still coming. The watch before this one goes at its next
    /// signal.
    fn watch_drives(&mut self) {
        self.drives.store(false, Ordering::SeqCst);
        let alive = Arc::new(AtomicBool::new(true));
        self.drives = Arc::clone(&alive);
        let (heard, rung) = (Arc::clone(&self.inbox), self.wake.clone());
        let watched = ronoctua_disks::watch_readings(move |reading| {
            if !alive.load(Ordering::SeqCst) {
                return false;
            }
            let came = match reading {
                Ok(rack) => Came::Rack(rack),
                Err(_) => Came::DisksDown,
            };
            leave(&heard, &rung, came);
            true
        });
        if let Err(e) = watched {
            tracing::info!(error = %e, "the stick view reads no drives");
            leave(&self.inbox, &self.wake, Came::DisksDown);
        }
    }

    /// Reads the drives again from a new watch — what `C-r` does when udisks
    /// did not answer.
    pub fn reread_drives(&mut self) {
        self.watch_drives();
    }

    /// What the threads left, oldest first.
    #[must_use]
    pub fn take(&self) -> Vec<Came> {
        self.inbox
            .lock()
            .map(|mut i| std::mem::take(&mut *i))
            .unwrap_or_default()
    }

    fn raise(&self, name: &str, work: impl FnOnce() -> Came + Send + 'static) {
        let (inbox, wake) = (Arc::clone(&self.inbox), self.wake.clone());
        let raised = std::thread::Builder::new()
            .name(format!("stick-{name}"))
            .spawn(move || {
                let came = work();
                if let Ok(mut i) = inbox.lock() {
                    i.push(came);
                }
                wake.call();
            });
        if let Err(e) = raised {
            tracing::error!(error = %e, "could not start the stick's thread");
        }
    }

    /// Asks GitHub for the latest release, on the person's pass.
    pub fn ask_latest(&self) {
        self.raise("latest", || {
            Came::Latest(match ronoctua_consent::pass(ronoctua_stick::API_HOST) {
                Ok(pass) => ronoctua_stick::latest(&pass).map_err(|r| r.why),
                Err(_) => Err(Why::NotLet),
            })
        });
    }

    /// Starts the writer through the sudo door — `gesture`, asked in the
    /// sudo bar — and, once it runs, hands the queue the download with the
    /// write armed. Another write under way comes back as [`Came::Busy`]
    /// BEFORE anything is asked.
    pub fn arm(&self, armed: Armed, mut gesture: crate::sudo::Gesture) {
        self.raise("arm", move || {
            if let Some(busy) = busy() {
                return Came::Busy(busy);
            }
            let staging = Path::new(ronoctua_stick::STAGING);
            // THE FOLDER EMPTIED BEFORE THE WRITER STARTS, so it waits for
            // THIS write's `ready` — and only now, with no write under way.
            ronoctua_stick::staging::clear(staging);
            match gesture.run(
                ronoctua_security::verb::RootVerb::StickWrite.into(),
                &mut |_| {},
            ) {
                Ok(()) => match queue(&armed.release, Some(&armed.target)) {
                    Ok(()) => Came::Queued(true),
                    Err(e) => {
                        // THE WRITER RUNS AND NOTHING WILL FEED IT: told now,
                        // and not after its twelve hours of waiting.
                        tracing::warn!(error = %e, "the queue did not take the stick's write");
                        let _ = ronoctua_stick::staging::cancel_for(staging, Some(Why::Failed));
                        Came::Queued(false)
                    }
                },
                Err(why) => Came::Door(why),
            }
        });
    }
}

/// What `C-r` reads again, in either door (through [`View::kept`]): GitHub,
/// when it is let; the cache; and the drives from a new watch when udisks
/// did not answer — or has not yet — so a refusal there is not for ever.
pub fn reread(r: &mut Readings, feed: Option<&mut Feed>) {
    if let Some(feed) = feed {
        if r.consent {
            r.latest = Latest::Asking;
            feed.ask_latest();
        }
        if r.disks_down || r.drives.is_none() {
            r.disks_down = false;
            feed.reread_drives();
        }
    }
    r.reread_cache();
}

/// The host the person has not let, of the two the queue's download asks.
#[must_use]
pub fn unlet_download_host() -> Option<&'static str> {
    [ronoctua_stick::DOWNLOAD_HOST, ronoctua_stick::ASSET_HOST]
        .into_iter()
        .find(|h| !ronoctua_consent::allows(h))
}

/// Hands the queue the image (and the write, when armed).
///
/// # Errors
///
/// No cache folder, or a queue that does not answer.
pub fn queue(release: &Release, write: Option<&Target>) -> anyhow::Result<()> {
    let into = ronoctua_xdg::cache_dir()
        .map(|d| ronoctua_stick::cache_dir_of(&d))
        .ok_or_else(|| anyhow::anyhow!("no cache folder"))?;
    let work = ronoctua_transfers::Work::Stick {
        release: Box::new(release.clone()),
        into,
        write: write.map(|t| Box::new(t.clone())),
    };
    ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work })
}

/// What `RET` on a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Nothing: a heading, a fact, a sentence.
    Nothing,
    /// The consent row: let the hosts.
    Allow,
    /// Ask GitHub again.
    Retry,
    /// Download the image only.
    Fetch,
    /// Choose this drive (its index in the readings' drives).
    Drive(usize),
    /// A drive that may not be written: say why.
    Unfit(usize),
    /// A drive too small for the image: say so.
    Small(usize),
    /// One of the question's two answers.
    Answer(bool),
}

/// One row of the view.
pub struct Row {
    /// The pieces, placed in cells.
    pub fields: Vec<Field>,
    /// A heading or a sentence: the cursor skips it.
    pub heading: bool,
    /// What `RET` does here.
    pub act: Act,
}

/// What the view wants done, after a key, a click or a thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Redraw and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Say this key's sentence.
    Speak(&'static str),
    /// Let the three hosts, then ask GitHub.
    Allow,
    /// Ask GitHub again and read the cache again.
    Reread,
    /// Queue the download alone.
    Fetch(Box<Release>),
    /// The person said yes: start the writer, then queue.
    Arm(Box<Armed>),
    /// The sudo door refused, and why.
    Door(ronoctua_security::Why),
}

/// What the cursor stands on, by what it IS and not by its line: a drive by
/// the disk border's id — the list reorders when a stick comes or goes —
/// and any other row by what it does and says.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Anchor {
    Drive(String),
    Row(Act, String),
}

/// The drive a row is about, by its id.
fn drive_of<'a>(act: &Act, r: &'a Readings) -> Option<&'a str> {
    match act {
        Act::Drive(n) | Act::Unfit(n) | Act::Small(n) => r
            .drives
            .as_ref()
            .and_then(|d| d.get(*n))
            .map(|j| j.target.drive.as_str()),
        _ => None,
    }
}

/// The anchor of a row the cursor can stand on.
fn anchor_of(row: &Row, r: &Readings) -> Option<Anchor> {
    if row.heading {
        return None;
    }
    Some(match drive_of(&row.act, r) {
        Some(id) => Anchor::Drive(id.to_owned()),
        None => Anchor::Row(
            row.act.clone(),
            row.fields
                .first()
                .map(|f| f.text.clone())
                .unwrap_or_default(),
        ),
    })
}

/// The view.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    /// The question: the drive, and the release it was asked about — kept
    /// whole, so a reading that comes meanwhile changes neither what it says
    /// nor what `yes` writes.
    asking: Option<Armed>,
    /// The drive the view was opened on (the sidebar's `W`), until the
    /// cursor stood on it once.
    chosen: Option<String>,
}

impl View {
    /// Opened on a drive — the file manager's `W` on its heading.
    #[must_use]
    pub fn on(drive: Option<String>) -> Self {
        Self {
            chosen: drive,
            ..Self::default()
        }
    }

    /// The row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The hint line, generated: the view's keys, and the consent row's
    /// while it is drawn.
    #[must_use]
    pub fn hint(&self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        let mut table: Vec<(&'static str, &'static str)> = OF_THE_VIEW.to_vec();
        if self.asking.is_some() {
            // THE QUESTION IS ABOUT THE RELEASE IT WAS ASKED WITH: asking
            // GitHub again is not taught over it.
            table.retain(|(_, id)| *id != "stick-reread");
        } else if !r.consent {
            table.extend_from_slice(crate::consent_rows::OF_THE_CONSENT);
        }
        crate::chrome::hint_of(&table, texts)
    }

    /// The row a click on row `n` of `rows` lands on — `n` itself, a row the
    /// cursor stands on (a fact included: it is read there), or the consent
    /// row a line of its note belongs to — and `None` where the click takes
    /// nothing: a heading, a sentence.
    fn landing(n: usize, rows: &[Row]) -> Option<usize> {
        // A LINE OF THE CONSENT ROW'S NOTE is the row: its first line is
        // where the cursor stands.
        let n = match rows.get(n) {
            Some(r) if r.heading && r.act != Act::Nothing => rows[..n]
                .iter()
                .rposition(|p| !p.heading && p.act == r.act)
                .unwrap_or(n),
            _ => n,
        };
        rows.get(n).is_some_and(|r| !r.heading).then_some(n)
    }

    /// Which of `rows` a click takes ([`View::clicked`]) — and so what a
    /// resting pointer lights, in both doors: a row the cursor stands on,
    /// the facts among them, and every line of the consent row; never a
    /// heading or a sentence.
    ///
    /// ONE ANSWER FOR THE LIGHT AND THE CLICK. The region lit only the rows
    /// whose `RET` acts, while the click selected a fact too: the pointer
    /// on a fact lit nothing, and its click moved the cursor there.
    #[must_use]
    pub fn takes(rows: &[Row]) -> Vec<bool> {
        (0..rows.len())
            .map(|n| Self::landing(n, rows).is_some())
            .collect()
    }

    /// The row whose control row `n` is: itself, or the consent row a line
    /// of its note belongs to — the lines of one control light together,
    /// and a click on any takes the same thing (`Region::control_of`).
    #[must_use]
    pub fn control_of(n: usize, rows: &[Row]) -> usize {
        Self::landing(n, rows).unwrap_or(n)
    }

    /// Puts the cursor where a click landed; `true` when it was already
    /// there — the second click, which is `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let Some(n) = Self::landing(n, rows) else {
            return false;
        };
        let again = self.cursor == n;
        self.cursor = n;
        again
    }

    /// What the pointer resting on row `n` says.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        use crate::key;
        let k = match rows.get(n)?.act {
            Act::Allow => key::STICK_ABOUT_ALLOW,
            Act::Retry => key::STICK_ABOUT_RETRY,
            Act::Fetch => key::STICK_ABOUT_FETCH,
            Act::Drive(_) => key::STICK_ABOUT_DRIVE,
            Act::Unfit(_) | Act::Small(_) => key::STICK_ABOUT_UNFIT,
            Act::Answer(false) => key::STICK_ABOUT_NO,
            Act::Answer(true) => key::STICK_ABOUT_YES,
            Act::Nothing => return None,
        };
        Some(texts.t(k))
    }

    /// A thread's answer, taken into the readings; what to say of it. The
    /// cursor stays on what it stood on — the same drive, by its id, when
    /// the list reorders; and when that drive left, on no other drive.
    pub fn heard(
        &mut self,
        r: &mut Readings,
        came: Came,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let stood = self.anchor(r, zone, columns, texts);
        let errand = match came {
            Came::Latest(Ok(release)) => {
                r.latest = Latest::Got(Box::new(release));
                r.reread_cache();
                Errand::Nothing
            }
            Came::Latest(Err(why)) => {
                r.latest = Latest::Refused(why);
                Errand::Nothing
            }
            Came::Rack(rack) => {
                let drives = ronoctua_stick::judge(&rack);
                // THE DRIVE ASKED ABOUT LEFT: the question goes with it.
                let gone = self.asking.as_ref().is_some_and(|q| {
                    !drives
                        .iter()
                        .any(|j| j.target.drive == q.target.drive && j.unfit.is_none())
                });
                r.drives = Some(drives);
                r.disks_down = false;
                if gone {
                    self.asking = None;
                    self.cursor = 0;
                    Errand::Speak(crate::key::STICK_DRIVE_LEFT)
                } else {
                    Errand::Nothing
                }
            }
            Came::DisksDown => {
                r.disks_down = true;
                Errand::Nothing
            }
            Came::Progress => {
                r.progress = ronoctua_stick::staging::read_progress(Path::new(
                    ronoctua_stick::PROGRESS_FILE,
                ));
                Errand::Nothing
            }
            Came::Cache => {
                r.reread_cache();
                Errand::Nothing
            }
            Came::Queued(true) => Errand::Speak(crate::key::STICK_QUEUED_WRITE),
            Came::Queued(false) => Errand::Speak(crate::key::TRANSFERS_OFFLINE),
            Came::Busy(busy) => Errand::Speak(busy.key()),
            Came::Door(why) => Errand::Door(why),
        };
        self.stand(stood, r, zone, columns, texts);
        errand
    }

    /// Changes the readings the way a DOOR does — `C-r`, the hosts just
    /// let ([`reread`]) — keeping the cursor on what it stood on, as
    /// [`View::heard`] does for a thread: while GitHub is asked again the
    /// release's rows fold into one line, and every drive's line moves up
    /// under a cursor that would otherwise stay where it was.
    pub fn kept(
        &mut self,
        r: &mut Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
        change: impl FnOnce(&mut Readings),
    ) {
        let stood = self.anchor(r, zone, columns, texts);
        change(r);
        self.stand(stood, r, zone, columns, texts);
    }

    /// What the cursor stands on, by what it is.
    fn anchor(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Anchor> {
        self.rows(r, zone, columns, texts)
            .get(self.cursor)
            .and_then(|row| anchor_of(row, r))
    }

    /// Puts the cursor back on what it stood on, in the rows of readings
    /// that changed.
    fn stand(
        &mut self,
        stood: Option<Anchor>,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(r, zone, columns, texts);
        if let Some(stood) = stood
            && self.asking.is_none()
        {
            self.cursor = rows
                .iter()
                .position(|row| anchor_of(row, r).as_ref() == Some(&stood))
                // GONE: the first row that is no drive's — never the
                // neighbour of the one that left, where a `RET` would ask
                // about a drive nobody stood on.
                .or_else(|| {
                    rows.iter()
                        .position(|row| !row.heading && drive_of(&row.act, r).is_none())
                })
                .unwrap_or(usize::MAX);
        }
        self.settle_on(&rows, r);
    }

    /// The rows, at this width.
    #[must_use]
    pub fn rows(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        use crate::key;
        let (left, room) = crate::chrome::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };
        if let Some(Armed { release, target }) = self.asking.as_ref() {
            // THE QUESTION OPENS ON `no`, and the cursor is born there: the
            // whole drive is erased, and nothing brings it back.
            b.heading_of(texts.t_with(
                key::STICK_SURE,
                &[
                    ("version", &release.label),
                    ("drive", &target.label),
                    ("size", &size_text(target.size_bytes, texts)),
                ],
            ));
            b.note(key::STICK_SURE_NOTE);
            b.pair(
                texts.t(key::STICK_NO),
                String::new(),
                Act::Answer(false),
                true,
            );
            b.pair(
                texts.t(key::STICK_YES),
                String::new(),
                Act::Answer(true),
                true,
            );
            return b.out;
        }

        b.heading(key::STICK_HEADING_RELEASE);
        if r.consent {
            match &r.latest {
                Latest::Unasked | Latest::Asking => b.note(key::STICK_ASKING),
                Latest::Refused(why) => {
                    b.sentence(texts.t(refusal(*why)));
                    b.pair(texts.t(key::STICK_RETRY), String::new(), Act::Retry, true);
                }
                Latest::Got(release) => {
                    b.pair(
                        texts.t(key::STICK_VERSION),
                        release.label.clone(),
                        Act::Nothing,
                        false,
                    );
                    if let Some(s) = published_s(&release.published_at) {
                        b.pair(
                            texts.t(key::STICK_PUBLISHED),
                            crate::chrome::when_text(s, zone, false, texts),
                            Act::Nothing,
                            false,
                        );
                    }
                    b.pair(
                        texts.t(key::STICK_SIZE),
                        size_text(release.image.size_bytes(), texts),
                        Act::Nothing,
                        false,
                    );
                    b.pair(
                        texts.t(key::STICK_IMAGE),
                        texts.t(if r.cached.is_some() {
                            key::STICK_IMAGE_CACHED
                        } else {
                            key::STICK_IMAGE_ABSENT
                        }),
                        Act::Nothing,
                        false,
                    );
                    if r.cached.is_none() {
                        b.pair(
                            texts.t(key::STICK_FETCH),
                            texts.t(key::STICK_FETCH_VALUE),
                            Act::Fetch,
                            true,
                        );
                    }
                }
            }
        } else {
            b.consent(texts);
        }

        b.heading(key::STICK_HEADING_DRIVES);
        match &r.drives {
            _ if r.disks_down => b.note(key::STICK_DRIVES_DOWN),
            None => b.note(key::STICK_DRIVES_READING),
            Some(drives) if drives.is_empty() => b.note(key::STICK_DRIVES_NONE),
            Some(drives) => {
                let image = r.release().map(|rel| rel.image.size_bytes());
                for (n, j) in drives.iter().enumerate() {
                    let size = size_text(j.target.size_bytes, texts);
                    let (value, act, acts) = match (j.unfit, image) {
                        (Some(u), _) => (
                            texts.t_with(unfit_value(u), &[("size", &size)]),
                            Act::Unfit(n),
                            false,
                        ),
                        (None, Some(bytes)) if !j.target.fits(bytes) => (
                            texts.t_with(key::STICK_DRIVE_SMALL, &[("size", &size)]),
                            Act::Small(n),
                            false,
                        ),
                        (None, _) => (
                            texts.t_with(key::STICK_DRIVE_READY, &[("size", &size)]),
                            Act::Drive(n),
                            true,
                        ),
                    };
                    b.pair(j.target.label.clone(), value, act, acts);
                }
                b.note(key::STICK_ERASES);
            }
        }

        if let Some(p) = r.progress {
            b.heading(key::STICK_HEADING_WRITE);
            let percent = p
                .done_bytes
                .saturating_mul(100)
                .checked_div(p.total_bytes)
                .map_or(0, |n| i64::try_from(n).unwrap_or(100));
            let state = match (p.phase, p.why) {
                (Phase::Failed, None) => texts.t(key::STICK_PHASE_GIVEN_UP),
                (phase, _) => texts.t_args(
                    phase_key(phase),
                    &[("percent", ronoctua_i18n::Arg::Number(percent))],
                ),
            };
            b.pair(texts.t(key::STICK_STATE), state, Act::Nothing, false);
            // WHY, IN A SENTENCE THAT WRAPS — never cut in a value's cell —
            // and in the colour of a failure: danger is a colour, and the
            // same reason in the grey of a note read as a footnote.
            if let (Phase::Failed, Some(why)) = (p.phase, p.why) {
                b.sentence_in(texts.t(refusal(why)), tokens::color::STATE_ERROR);
            }
        }
        b.out
    }

    /// What a key does. The width is the door's: a note wraps, and the cursor
    /// is an index into the rows drawn.
    pub fn press(
        &mut self,
        p: Press,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(r, zone, columns, texts);
        self.settle_on(&rows, r);
        match p {
            Press::Up => {
                self.walk(&rows, false);
                Errand::Nothing
            }
            Press::Down => {
                self.walk(&rows, true);
                Errand::Nothing
            }
            // NOT OVER THE QUESTION: it holds the release it was asked
            // with, and a reread would only move the cursor under it.
            Press::Reread if self.asking.is_some() => Errand::Nothing,
            Press::Reread => Errand::Reread,
            Press::Allow if !r.consent => Errand::Allow,
            Press::Allow => Errand::Nothing,
            Press::Escape => {
                if self.asking.take().is_some() {
                    self.cursor = 0;
                    let rows = self.rows(r, zone, columns, texts);
                    self.settle_on(&rows, r);
                    return Errand::Nothing;
                }
                Errand::Close
            }
            Press::Enter => {
                let act = rows
                    .get(self.cursor)
                    .map_or(Act::Nothing, |row| row.act.clone());
                let errand = self.act(act, r);
                let rows = self.rows(r, zone, columns, texts);
                self.settle_on(&rows, r);
                errand
            }
        }
    }

    /// `RET`, and what a second click becomes.
    fn act(&mut self, act: Act, r: &Readings) -> Errand {
        use crate::key;
        let drive = |n: usize| r.drives.as_ref().and_then(|d| d.get(n));
        match act {
            Act::Nothing => Errand::Nothing,
            Act::Allow => Errand::Allow,
            Act::Retry => Errand::Reread,
            Act::Fetch => r
                .release()
                .map_or(Errand::Nothing, |rel| Errand::Fetch(Box::new(rel.clone()))),
            Act::Unfit(n) => drive(n)
                .and_then(|j| j.unfit)
                .map_or(Errand::Nothing, |u| Errand::Speak(unfit_why(u))),
            Act::Small(_) => Errand::Speak(key::STICK_SMALL_WHY),
            Act::Drive(n) => {
                let Some(release) = r.release() else {
                    return Errand::Speak(key::STICK_NO_RELEASE_YET);
                };
                let Some(j) = drive(n) else {
                    return Errand::Nothing;
                };
                self.asking = Some(Armed {
                    release: release.clone(),
                    target: j.target.clone(),
                });
                // ON `no`: the heading, its note's lines, then `no`.
                self.cursor = usize::MAX;
                Errand::Nothing
            }
            Act::Answer(false) => {
                self.asking = None;
                self.cursor = 0;
                Errand::Nothing
            }
            Act::Answer(true) => {
                let Some(armed) = self.asking.take() else {
                    return Errand::Nothing;
                };
                self.cursor = 0;
                Errand::Arm(Box::new(armed))
            }
        }
    }

    /// Moves the cursor to the next row it can stand on.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows.get(at).is_some_and(|r| !r.heading) {
                self.cursor = at;
                return;
            }
        }
    }

    /// Puts the cursor on a row that acts: the drive the view was opened on,
    /// once it is drawn; `no` while the question is up; otherwise where it
    /// is, or the first row it can stand on.
    pub fn settle_on(&mut self, rows: &[Row], r: &Readings) {
        if self.asking.is_some() {
            if !rows
                .get(self.cursor)
                .is_some_and(|r| r.act == Act::Answer(false) || r.act == Act::Answer(true))
            {
                self.cursor = rows
                    .iter()
                    .position(|r| r.act == Act::Answer(false))
                    .unwrap_or(0);
            }
            return;
        }
        if self.chosen.is_some()
            && let Some(at) = rows.iter().position(|row| {
                drive_of(&row.act, r).is_some() && drive_of(&row.act, r) == self.chosen.as_deref()
            })
        {
            self.cursor = at;
            self.chosen = None;
            return;
        }
        if rows.get(self.cursor).is_some_and(|r| !r.heading) {
            return;
        }
        // THE FIRST ROW THAT ACTS, before the first fact: a view opened on a
        // version number would teach `RET` on a row where it does nothing.
        self.cursor = rows
            .iter()
            .position(|r| !r.heading && r.act != Act::Nothing)
            .or_else(|| rows.iter().position(|r| !r.heading))
            .unwrap_or(0);
    }

    /// Sets the drive the cursor goes to once it is drawn.
    pub fn choose(&mut self, drive: Option<String>) {
        self.chosen = drive;
    }

    /// Puts the cursor on a row that acts, against the rows of these
    /// readings at this width — what a door does once, when it opens, so
    /// the first frame's cursor is not on a title.
    pub fn settle(
        &mut self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(r, zone, columns, texts);
        self.settle_on(&rows, r);
    }
}

/// The sentence of a refusal.
#[must_use]
pub const fn refusal(why: Why) -> &'static str {
    use crate::key;
    match why {
        Why::NotLet => key::STICK_REFUSED_NOT_LET,
        Why::Offline => key::STICK_REFUSED_OFFLINE,
        Why::Download => key::STICK_REFUSED_DOWNLOAD,
        Why::NoRelease => key::STICK_REFUSED_NO_RELEASE,
        Why::Mismatch => key::STICK_REFUSED_MISMATCH,
        Why::NotWritable => key::STICK_REFUSED_NOT_WRITABLE,
        Why::Changed => key::STICK_REFUSED_CHANGED,
        Why::ReadBackDiffers => key::STICK_REFUSED_READ_BACK_DIFFERS,
        Why::Failed => key::STICK_REFUSED_FAILED,
    }
}

/// What a drive's row says when it may not be written ({ $size } first).
#[must_use]
pub const fn unfit_value(u: Unfit) -> &'static str {
    use crate::key;
    match u {
        Unfit::Bus => key::STICK_UNFIT_BUS,
        Unfit::System => key::STICK_UNFIT_SYSTEM,
        Unfit::Mounted => key::STICK_UNFIT_MOUNTED,
        Unfit::Open => key::STICK_UNFIT_OPEN,
        Unfit::ReadOnly => key::STICK_UNFIT_READ_ONLY,
        Unfit::Unnamed => key::STICK_UNFIT_UNNAMED,
    }
}

/// What pressing such a row says: why, and what to do about it.
#[must_use]
pub const fn unfit_why(u: Unfit) -> &'static str {
    use crate::key;
    match u {
        Unfit::Bus => key::STICK_WHY_BUS,
        Unfit::System => key::STICK_WHY_SYSTEM,
        Unfit::Mounted => key::STICK_WHY_MOUNTED,
        Unfit::Open => key::STICK_WHY_OPEN,
        Unfit::ReadOnly => key::STICK_WHY_READ_ONLY,
        Unfit::Unnamed => key::STICK_WHY_UNNAMED,
    }
}

/// The writer's phase, as a sentence with `{ $percent }`.
#[must_use]
pub const fn phase_key(p: Phase) -> &'static str {
    use crate::key;
    match p {
        Phase::Waiting => key::STICK_PHASE_WAITING,
        Phase::Writing => key::STICK_PHASE_WRITING,
        Phase::Verifying => key::STICK_PHASE_VERIFYING,
        Phase::Done => key::STICK_PHASE_DONE,
        Phase::Failed => key::STICK_PHASE_FAILED,
    }
}

/// A size, in the language's own marks.
fn size_text(bytes: u64, texts: &ronoctua_i18n::Texts) -> String {
    crate::sizes::readable_bytes(bytes, texts.decimal_mark())
}

/// Seconds since the epoch of GitHub's `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn published_s(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    let n = |from: usize, to: usize| text.get(from..to)?.parse::<u32>().ok();
    let (h, mi, s) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let days = ronoctua_files::when::epoch_of(i64::from(n(0, 4)?), n(5, 7)?, n(8, 10)?)?;
    Some(days * 86_400 + i64::from(h * 3600 + mi * 60 + s))
}

/// Why a write cannot be armed here and now, as a sentence — the hosts the
/// download asks not let, no writer on this machine (the nested loop on a
/// machine of another distro), no sudo door — or `None`. Asked BEFORE the
/// password: a password typed for a write that cannot happen is a password
/// asked for nothing.
#[must_use]
pub fn cannot_arm(texts: &ronoctua_i18n::Texts) -> Option<String> {
    use crate::key;
    if let Some(host) = unlet_download_host() {
        return Some(texts.t_with(key::STICK_NOT_LET_HOST, &[("host", host)]));
    }
    if !Path::new(ronoctua_stick::STAGING).is_dir() {
        return Some(texts.t(key::STICK_NO_WRITER));
    }
    if !ronoctua_security::has_helper() {
        return Some(texts.t(crate::security_words::refusal(
            ronoctua_security::Why::NoHelper,
        )));
    }
    if ronoctua_xdg::cache_dir().is_none() {
        return Some(texts.t(key::STICK_NO_CACHE));
    }
    None
}

/// Queues the image alone, and the sentence that says how that went.
#[must_use]
pub fn fetch_said(release: &Release, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    if let Some(host) = unlet_download_host() {
        return texts.t_with(key::STICK_NOT_LET_HOST, &[("host", host)]);
    }
    match queue(release, None) {
        Ok(()) => texts.t(key::STICK_QUEUED_FETCH),
        Err(e) => {
            tracing::warn!(error = %e, "the queue did not take the stick's image");
            texts.t(key::TRANSFERS_OFFLINE)
        }
    }
}

/// Builds the rows — `geo.rs`'s builder, and the same reasons.
struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, colour: tokens::Color, room: u16) -> Field {
        Field {
            at,
            text: layout::elide_end(&text, room),
            color: colour,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    fn heading(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.heading_of(text);
    }

    fn heading_of(&mut self, text: String) {
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                heading: true,
                act: Act::Nothing,
            });
        }
        for line in crate::chrome::note_lines(&text, self.room) {
            self.out.push(Row {
                fields: vec![Self::piece(
                    self.left,
                    line,
                    tokens::color::TEXT_HEADING,
                    self.room,
                )],
                heading: true,
                act: Act::Nothing,
            });
        }
    }

    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.sentence(text);
    }

    /// It wraps, and is never cut (`chrome::note_lines`).
    fn sentence(&mut self, text: String) {
        self.sentence_in(text, tokens::color::TEXT_DIM);
    }

    /// [`Builder::sentence`], in a tone of its own.
    fn sentence_in(&mut self, text: String, colour: tokens::Color) {
        for line in crate::chrome::note_lines(&text, self.room) {
            self.out.push(Row {
                fields: vec![Self::piece(self.left, line, colour, self.room)],
                heading: true,
                act: Act::Nothing,
            });
        }
    }

    /// A row of label and value. `acts`: drawn as a row that acts, or as a
    /// refusing one (reachable, so a press says why). Which drive a row is
    /// about is its [`Act`]'s index into the readings ([`drive_of`]), which
    /// is how the cursor finds it again when the list moves.
    fn pair(&mut self, label: String, value: String, act: Act, acts: bool) {
        let focused = self.cursor == self.out.len();
        let kind = match act {
            Act::Nothing => crate::chrome::Kind::Fact,
            _ if acts => crate::chrome::Kind::Setting,
            _ => crate::chrome::Kind::Disabled,
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
            heading: false,
            act,
        });
    }

    /// The consent row: "Ask {hosts}", and what for — every line the press.
    fn consent(&mut self, texts: &ronoctua_i18n::Texts) {
        let focused = self.cursor == self.out.len();
        let ask = crate::consent_rows::Ask {
            hosts: &ronoctua_stick::HOSTS,
            what: crate::key::STICK_CONSENT_FOR,
        };
        for (n, (line, colour)) in crate::consent_rows::lines(ask, self.room, focused, texts)
            .into_iter()
            .enumerate()
        {
            self.out.push(Row {
                fields: vec![Self::piece(self.left, line, colour, self.room)],
                heading: n > 0,
                act: Act::Allow,
            });
        }
    }
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "stick",
    commands: &[
        ronoctua_cmd::Command {
            id: "stick-open",
            label: crate::key::CMD_STICK_OPEN,
        },
        ronoctua_cmd::Command {
            id: "stick-window",
            label: crate::key::CMD_STICK_WINDOW,
        },
        // THE STICK VIEW'S KEYS, which its hint line teaches.
        ronoctua_cmd::Command {
            id: "stick-down",
            label: crate::key::CMD_STICK_DOWN,
        },
        ronoctua_cmd::Command {
            id: "stick-up",
            label: crate::key::CMD_STICK_UP,
        },
        ronoctua_cmd::Command {
            id: "stick-enter",
            label: crate::key::CMD_STICK_ENTER,
        },
        ronoctua_cmd::Command {
            id: "stick-reread",
            label: crate::key::CMD_STICK_REREAD,
        },
    ],
    bindings: &[
        ("s z z", "stick-open", None),
        ("s z w", "stick-window", None),
    ],
    groups: &[("s z", crate::key::GROUP_STICK)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::stick::OF_THE_VIEW
        .iter()
        .map(|(_, id)| *id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_stick::{Asset, Image};

    const WIDTH: u16 = 100;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("the language")
    }

    fn release() -> Release {
        let asset = |name: &str, size| Asset {
            name: name.to_owned(),
            url: format!("https://github.com/ronoctua/ronoctua-distro/releases/download/v1/{name}"),
            size_bytes: size,
        };
        Release {
            tag: "v1".to_owned(),
            label: "26.05.20261005".to_owned(),
            published_at: "2026-10-05T12:00:00Z".to_owned(),
            image: Image::Whole {
                asset: asset("ronoctua-distro-26.05.20261005-x86_64.iso", 3_000_000_000),
            },
            sums: asset("SHA256SUMS", 100),
        }
    }

    fn judged(n: &str, size_bytes: u64, unfit: Option<Unfit>) -> Judged {
        Judged {
            target: Target {
                drive: format!("/drive/{n}"),
                by_id: format!("/dev/disk/by-id/usb-{n}").into(),
                serial: Some(n.to_owned()),
                size_bytes,
                label: format!("Stick {n}"),
            },
            unfit,
        }
    }

    fn readings() -> Readings {
        Readings {
            consent: true,
            latest: Latest::Got(Box::new(release())),
            drives: Some(vec![
                judged("a", 32_000_000_000, None),
                judged("b", 16_000_000_000, Some(Unfit::Mounted)),
                judged("c", 1_000_000_000, None),
            ]),
            ..Readings::default()
        }
    }

    fn zone() -> ronoctua_files::when::Zone {
        ronoctua_files::when::Zone::utc()
    }

    fn row_of(v: &View, r: &Readings, t: &ronoctua_i18n::Texts, act: &Act) -> usize {
        v.rows(r, &zone(), WIDTH, t)
            .iter()
            .position(|row| row.act == *act)
            .unwrap_or_else(|| panic!("no row for {act:?}"))
    }

    fn press(v: &mut View, r: &Readings, t: &ronoctua_i18n::Texts, p: Press) -> Errand {
        v.press(p, r, &zone(), WIDTH, t)
    }

    /// THE QUESTION OPENS ON `no`: the whole drive is erased, and the next
    /// key after choosing it must not be the yes nobody meant. `no` closes
    /// it; `yes` arms the write with the drive chosen and the release.
    #[test]
    fn the_question_opens_on_no_and_yes_arms_that_drive() {
        let t = texts("en-GB");
        let r = readings();
        let mut v = View::on(None);
        v.cursor = row_of(&v, &r, &t, &Act::Drive(0));
        assert_eq!(press(&mut v, &r, &t, Press::Enter), Errand::Nothing);
        assert!(v.asking.is_some());
        let asked = v.rows(&r, &zone(), WIDTH, &t);
        assert_eq!(
            asked[v.cursor()].act,
            Act::Answer(false),
            "the cursor is born on no"
        );
        let no_at = asked
            .iter()
            .position(|row| row.act == Act::Answer(false))
            .unwrap();
        let yes_at = asked
            .iter()
            .position(|row| row.act == Act::Answer(true))
            .unwrap();
        assert!(no_at < yes_at, "no is drawn first");

        assert_eq!(press(&mut v, &r, &t, Press::Enter), Errand::Nothing);
        assert!(!v.asking.is_some(), "no leaves the question");

        v.cursor = row_of(&v, &r, &t, &Act::Drive(0));
        let _ = press(&mut v, &r, &t, Press::Enter);
        let _ = press(&mut v, &r, &t, Press::Down);
        match press(&mut v, &r, &t, Press::Enter) {
            Errand::Arm(armed) => {
                assert_eq!(armed.target.drive, "/drive/a");
                assert_eq!(armed.release.label, "26.05.20261005");
            }
            other => panic!("{other:?}"),
        }
        assert!(!v.asking.is_some());
    }

    /// A DRIVE THAT MAY NOT BE WRITTEN IS DRAWN, AND SAYS WHY: pressing a
    /// mounted stick or one too small for the image says what to do, and
    /// opens no question.
    #[test]
    fn a_refused_drive_says_why_and_asks_nothing() {
        let t = texts("en-GB");
        let r = readings();
        let mut v = View::on(None);
        v.cursor = row_of(&v, &r, &t, &Act::Unfit(1));
        assert_eq!(
            press(&mut v, &r, &t, Press::Enter),
            Errand::Speak(crate::key::STICK_WHY_MOUNTED)
        );
        v.cursor = row_of(&v, &r, &t, &Act::Small(2));
        assert_eq!(
            press(&mut v, &r, &t, Press::Enter),
            Errand::Speak(crate::key::STICK_SMALL_WHY)
        );
        assert!(!v.asking.is_some());
        // WITH NO RELEASE YET, a drive is not chosen: there is nothing to
        // write to it.
        let mut early = readings();
        early.latest = Latest::Asking;
        let mut v = View::on(None);
        v.cursor = row_of(&v, &early, &t, &Act::Drive(0));
        assert_eq!(
            press(&mut v, &early, &t, Press::Enter),
            Errand::Speak(crate::key::STICK_NO_RELEASE_YET)
        );
    }

    /// NOTHING ASKS GITHUB BEFORE CONSENT: without it the consent row is
    /// drawn, and it — or its key — is what lets the hosts.
    #[test]
    fn the_consent_row_comes_first_and_lets_the_hosts() {
        let t = texts("en-GB");
        let r = Readings::default();
        let mut v = View::on(None);
        let rows = v.rows(&r, &zone(), WIDTH, &t);
        let first = rows
            .iter()
            .position(|row| row.act == Act::Allow)
            .expect("a consent row");
        assert!(!rows.iter().any(|row| row.act == Act::Fetch));
        v.cursor = first;
        assert_eq!(press(&mut v, &r, &t, Press::Enter), Errand::Allow);
        let ctrl_a = ronoctua_keymap::Key::parse("C-a").unwrap();
        assert_eq!(press_of_key(ctrl_a), Some(Press::Allow));
        assert_eq!(press(&mut v, &r, &t, Press::Allow), Errand::Allow);
        // A LINE OF THE ROW'S NOTE is the row too.
        if let Some(note) = rows
            .iter()
            .rposition(|row| row.act == Act::Allow && row.heading)
        {
            let mut v = View::on(None);
            let _ = v.clicked(note, &rows);
            assert_eq!(v.cursor(), first);
        }
    }

    /// OPENED ON A DRIVE (the sidebar's `W`), the cursor is on that drive
    /// as soon as the drives are read.
    #[test]
    fn opened_on_a_drive_the_cursor_finds_it() {
        let t = texts("en-GB");
        let mut r = readings();
        r.drives = None;
        let mut v = View::on(Some("/drive/c".to_owned()));
        let mut rack = ronoctua_disks::Rack::default();
        rack.drives.push(ronoctua_disks::Drive {
            at: "/drive/c".to_owned(),
            bytes: 1_000_000_000,
            bus: ronoctua_disks::Bus::Usb,
            removable: true,
            ..ronoctua_disks::Drive::default()
        });
        let _ = v.heard(&mut r, Came::Rack(rack), &zone(), WIDTH, &t);
        let rows = v.rows(&r, &zone(), WIDTH, &t);
        assert!(
            matches!(
                rows[v.cursor()].act,
                Act::Unfit(0) | Act::Small(0) | Act::Drive(0)
            ),
            "{:?}",
            rows[v.cursor()].act
        );
    }

    /// A DRIVE THAT LEAVES while the question is up takes the question with
    /// it, and says so.
    #[test]
    fn a_drive_that_leaves_takes_its_question() {
        let t = texts("en-GB");
        let mut r = readings();
        let mut v = View::on(None);
        v.cursor = row_of(&v, &r, &t, &Act::Drive(0));
        let _ = press(&mut v, &r, &t, Press::Enter);
        assert!(v.asking.is_some());
        let errand = v.heard(
            &mut r,
            Came::Rack(ronoctua_disks::Rack::default()),
            &zone(),
            WIDTH,
            &t,
        );
        assert_eq!(errand, Errand::Speak(crate::key::STICK_DRIVE_LEFT));
        assert!(!v.asking.is_some());
        // AND `Esc` leaves a question before it closes the view.
        let r = readings();
        v.cursor = row_of(&v, &r, &t, &Act::Drive(0));
        let _ = press(&mut v, &r, &t, Press::Enter);
        assert_eq!(press(&mut v, &r, &t, Press::Escape), Errand::Nothing);
        assert!(!v.asking.is_some());
        assert_eq!(press(&mut v, &r, &t, Press::Escape), Errand::Close);
    }

    /// EVERY CLOSED SET THAT BECOMES A SENTENCE has one in every language,
    /// and every row of every state is drawn in both.
    #[test]
    fn every_word_is_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for why in Why::ALL {
                assert!(t.has(refusal(why)), "{language}: {why:?}");
            }
            for u in Unfit::ALL {
                assert!(
                    t.has(unfit_value(u)) && t.has(unfit_why(u)),
                    "{language}: {u:?}"
                );
            }
            for p in Phase::ALL {
                assert!(t.has(phase_key(p)), "{language}: {p:?}");
            }
            for b in Busy::ALL {
                assert!(t.has(b.key()), "{language}: {b:?}");
            }
            let mut r = readings();
            r.progress = Some(ronoctua_stick::staging::progress(
                Some(1),
                Phase::Writing,
                5,
                10,
                None,
            ));
            let rows = View::on(None).rows(&r, &zone(), WIDTH, &t);
            let all: String = rows
                .iter()
                .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
                .collect();
            assert!(
                all.contains("50"),
                "{language}: the percent of the write: {all}"
            );
            assert!(
                !all.contains("stick-"),
                "{language}: a key drawn as text: {all}"
            );
        }
    }

    /// NOT A SECOND WRITE OVER ONE UNDER WAY: a stick being written, or a
    /// write armed and waiting for its image, refuses a new `yes`; a writer
    /// waiting with no armed job to feed it is told to stop; a writer whose
    /// last answer is final, with nothing armed, lets one start.
    #[test]
    fn a_write_is_not_armed_over_another() {
        let said = |phase| Some(ronoctua_stick::staging::progress(None, phase, 0, 0, None));
        assert_eq!(
            busy_of(said(Phase::Writing).as_ref(), false),
            Some(Busy::Writing)
        );
        assert_eq!(
            busy_of(said(Phase::Verifying).as_ref(), true),
            Some(Busy::Writing)
        );
        assert_eq!(
            busy_of(said(Phase::Waiting).as_ref(), true),
            Some(Busy::Waiting)
        );
        assert_eq!(
            busy_of(said(Phase::Waiting).as_ref(), false),
            Some(Busy::Stopped),
            "a writer nobody will feed"
        );
        assert_eq!(
            busy_of(said(Phase::Done).as_ref(), true),
            Some(Busy::Waiting),
            "an armed job not yet staged"
        );
        assert_eq!(busy_of(None, true), Some(Busy::Waiting));
        for over in [Phase::Done, Phase::Failed] {
            assert_eq!(busy_of(said(over).as_ref(), false), None, "{over:?}");
        }
        assert_eq!(busy_of(None, false), None);
        // AND THE REFUSAL IS SAID, in both doors' one path.
        let t = texts("en-GB");
        let mut r = readings();
        let mut v = View::on(None);
        assert_eq!(
            v.heard(&mut r, Came::Busy(Busy::Writing), &zone(), WIDTH, &t),
            Errand::Speak(crate::key::STICK_BUSY_WRITING)
        );
    }

    /// THE QUESTION KEEPS THE RELEASE IT WAS ASKED WITH: `C-r` is neither
    /// taught nor heard over it, and GitHub refusing meanwhile changes
    /// nothing — the question is still drawn, `no` still under the cursor,
    /// and `yes` writes the release the person read.
    #[test]
    fn the_question_keeps_its_release_and_cr_is_not_taught_over_it() {
        let t = texts("en-GB");
        let mut r = readings();
        let mut v = View::on(None);
        v.cursor = row_of(&v, &r, &t, &Act::Drive(0));
        let _ = press(&mut v, &r, &t, Press::Enter);
        let taught: String = v.hint(&r, &t).iter().map(|p| p.text.clone()).collect();
        assert!(!taught.contains("C-r"), "{taught}");
        assert_eq!(press(&mut v, &r, &t, Press::Reread), Errand::Nothing);
        let _ = v.heard(&mut r, Came::Latest(Err(Why::Offline)), &zone(), WIDTH, &t);
        let rows = v.rows(&r, &zone(), WIDTH, &t);
        assert_eq!(
            rows[v.cursor()].act,
            Act::Answer(false),
            "the question is still up"
        );
        let _ = press(&mut v, &r, &t, Press::Down);
        match press(&mut v, &r, &t, Press::Enter) {
            Errand::Arm(armed) => assert_eq!(armed.release.label, "26.05.20261005"),
            other => panic!("{other:?}"),
        }
        // OUT OF THE QUESTION, `C-r` is taught again.
        let taught: String = v.hint(&r, &t).iter().map(|p| p.text.clone()).collect();
        assert!(taught.contains("C-r"), "{taught}");
    }

    /// A rack of USB sticks, in this order.
    fn rack_of(names: &[&str]) -> ronoctua_disks::Rack {
        let mut rack = ronoctua_disks::Rack::default();
        for n in names {
            rack.drives.push(ronoctua_disks::Drive {
                at: format!("/drive/{n}"),
                model: format!("Stick {n}"),
                bytes: 32_000_000_000,
                bus: ronoctua_disks::Bus::Usb,
                removable: true,
                ..ronoctua_disks::Drive::default()
            });
            rack.volumes.push(ronoctua_disks::Volume {
                at: format!("/block/{n}"),
                node: format!("/dev/sd{n}"),
                bytes: 32_000_000_000,
                drive: Some(format!("/drive/{n}")),
                symlinks: vec![format!("/dev/disk/by-id/usb-Stick_{n}-0:0")],
                ..ronoctua_disks::Volume::default()
            });
        }
        rack
    }

    /// THE CURSOR KEEPS ITS DRIVE, NOT ITS LINE: a stick pulled out above it
    /// moves the line, and the cursor follows the drive; the drive itself
    /// pulled out leaves the cursor on no other drive, so the next `RET`
    /// asks about nothing nobody stood on.
    #[test]
    fn the_cursor_keeps_its_drive_when_the_list_reorders() {
        let t = texts("en-GB");
        let mut r = readings();
        r.drives = None;
        let mut v = View::on(None);
        let _ = v.heard(
            &mut r,
            Came::Rack(rack_of(&["a", "b", "c"])),
            &zone(),
            WIDTH,
            &t,
        );
        v.cursor = row_of(&v, &r, &t, &Act::Drive(2));
        let _ = v.heard(&mut r, Came::Rack(rack_of(&["b", "c"])), &zone(), WIDTH, &t);
        let rows = v.rows(&r, &zone(), WIDTH, &t);
        assert_eq!(drive_of(&rows[v.cursor()].act, &r), Some("/drive/c"));
        let _ = v.heard(&mut r, Came::Rack(rack_of(&["a", "b"])), &zone(), WIDTH, &t);
        let rows = v.rows(&r, &zone(), WIDTH, &t);
        assert_eq!(
            drive_of(&rows[v.cursor()].act, &r),
            None,
            "the drive left: the cursor is on no other drive"
        );
        assert!(!rows[v.cursor()].heading);
    }

    /// `C-r` FOLDS THE RELEASE'S ROWS INTO ONE LINE while GitHub is asked
    /// again, and every drive's line moves up: the cursor moves with its
    /// drive, never onto the drive that slid under it. The LAST drive, so
    /// that "the first row that acts", where a lost cursor lands, is not it.
    #[test]
    fn a_reread_keeps_the_cursor_on_its_drive() {
        let t = texts("en-GB");
        let mut r = readings();
        let mut v = View::on(None);
        // Too small for the release; with no release known it is a drive
        // like any other — the anchor is the drive, not what its row does.
        v.cursor = row_of(&v, &r, &t, &Act::Small(2));
        let before = v.cursor();
        v.kept(&mut r, &zone(), WIDTH, &t, |r| {
            reread(r, None);
            r.latest = Latest::Asking;
        });
        let rows = v.rows(&r, &zone(), WIDTH, &t);
        assert!(
            v.cursor() < before,
            "the line moved: {before} → {}",
            v.cursor()
        );
        assert_eq!(drive_of(&rows[v.cursor()].act, &r), Some("/drive/c"));
    }

    /// UDISKS NOT ANSWERING IS SAID, never drawn as a reading on its way.
    #[test]
    fn disks_that_do_not_answer_are_said() {
        let t = texts("en-GB");
        let mut r = readings();
        r.drives = None;
        let mut v = View::on(None);
        let _ = v.heard(&mut r, Came::DisksDown, &zone(), WIDTH, &t);
        let all: String = v
            .rows(&r, &zone(), WIDTH, &t)
            .iter()
            .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(all.contains("udisks"), "{all}");
        assert!(!all.contains("Reading the drives"), "{all}");
    }

    #[test]
    fn the_keys_read_as_the_footer_reads_them() {
        let read = |n: &str| press_of_key(ronoctua_keymap::Key::parse(n).expect("a key"));
        assert_eq!(read("<down>"), Some(Press::Down));
        assert_eq!(read("RET"), Some(Press::Enter));
        assert_eq!(read("C-r"), Some(Press::Reread));
        assert_eq!(read("ESC"), Some(Press::Escape));
        assert_eq!(read("r"), None, "a bare letter is not the chord");
        for (taught, id) in OF_THE_VIEW {
            assert!(read(taught).is_some() && press_of(id).is_some(), "{taught}");
        }
        assert_eq!(published_s("1970-01-02T00:00:01Z"), Some(86_401));
        assert_eq!(published_s("2026-13-01T00:00:00Z"), None);
        assert_eq!(published_s("yesterday"), None);
    }
}
