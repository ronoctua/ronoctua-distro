//! A REPOSITORY'S HISTORY OPEN IN A DOOR — what both of its doors keep and
//! do ([`Git`], [`carry_out`]) — its region of the footer ([`GitRegion`],
//! [`ENTRY`]), and `LEADER / c`, the distro's own clone, which the region's
//! entry carries ([`dev_clone`]).
//!
//! # One errand handler, two doors
//!
//! What a key of the history asks — fetch, copy an id, open a version, a
//! terminal at the root, say a sentence — is carried out by [`carry_out`]
//! over the door's [`Doorway`]; what a thread of it comes back with is taken
//! by [`Git::back`]. The window and the footer used to write both out apart,
//! and had drifted: a version that nothing opens said "nothing opens it" in
//! the window and "copied out" in the footer, over that sentence.
//!
//! # The threads
//!
//! Every reading of the repository, the fetch and a version copied out run on
//! threads of their own, which hand back a [`Back`] through what the door
//! gave the history ([`Git::over`]): the window's loop channel, or the
//! region's own channel and the door's wake.
//!
//! # Opened over the file manager
//!
//! The region opens from the file manager's `C-h` ([`OPEN`], with the folder
//! as the argument): the manager waits aside while it is up, and comes back
//! when it closes — the manager's to do, on its side of the footer.

use std::sync::{Arc, mpsc};

use crate::git_browser::{Errand, Heard, Need, View};
use crate::region::{
    Area, Band, Closed, Doorway, Entry, Grip, Lights, Region, RegionEasel, Row, Seen, Shape, Taken,
};
use ronoctua_tokens as tokens;

/// The region's id, as the footer ranks it.
pub const ID: &str = "git";

/// What opens the region over the folder its argument names — the file
/// manager's `C-h` (`git_line::HISTORY`, the order the manager gives). Not a
/// command of the registry: no key reaches it but the manager's own.
pub const OPEN: &str = crate::git_line::HISTORY;

/// What a thread of the history hands back.
#[derive(Debug)]
pub enum Back {
    /// A need, read.
    Heard(Heard),
    /// The repository moved on disk.
    Moved,
    /// The fetch ended.
    Fetched(Result<(), ronoctua_git::Why>),
    /// A version was copied out, with its id and what opens it — chosen
    /// on the copying thread, never on a door's loop — or why not.
    Kept(Result<(std::path::PathBuf, String, crate::launch::Opener), crate::git_browser::KeepWhy>),
}

/// Where the history's threads hand back.
type Backs = Arc<dyn Fn(Back) + Send + Sync>;

/// A REPOSITORY'S HISTORY OPEN IN A DOOR: the view, the repository it reads,
/// the watch that says it moved, and where its threads hand back — the same
/// in the window and in the footer's region.
pub struct Git {
    /// The view.
    pub view: View,
    /// The repository, when the folder is in one.
    pub repo: Option<ronoctua_git::Repo>,
    backs: Backs,
    watch: Option<ronoctua_files::notice::Notice>,
    /// The programs that open a file, swept the first time a version is
    /// opened — never at start: most histories never open one — and swept
    /// on the thread that copies the version out, as the file's type is
    /// read there: a walk of every application's file never runs on a
    /// door's loop.
    catalogue: Arc<std::sync::OnceLock<crate::launch::Catalogue>>,
    /// Whether every reading is made on THIS thread — the photograph's,
    /// which has no loop to hear a thread's answer on.
    at_once: bool,
}

impl std::fmt::Debug for Git {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Git")
            .field("root", &self.view.root())
            .field("at_once", &self.at_once)
            .finish_non_exhaustive()
    }
}

impl Git {
    /// The history of the repository `dir` is in — `wide` for a window's,
    /// which lays its columns out over a whole surface — handing what its
    /// threads find to `backs`. Nothing watched yet ([`Git::watched`]).
    #[must_use]
    pub fn over(
        dir: &std::path::Path,
        wide: bool,
        texts: std::rc::Rc<ronoctua_i18n::Texts>,
        zone: ronoctua_files::when::Zone,
        backs: impl Fn(Back) + Send + Sync + 'static,
    ) -> Self {
        let repo = ronoctua_git::Repo::discover(dir);
        let root = repo.as_ref().map_or(dir, |r| r.root.as_path());
        Self {
            view: View::at(root, wide, texts, zone),
            repo,
            backs: Arc::new(backs),
            watch: None,
            catalogue: Arc::default(),
            at_once: false,
        }
    }

    /// The same history, reading on this thread ([`Git::ask_now`]) — the
    /// photograph's.
    #[must_use]
    pub const fn at_once(mut self) -> Self {
        self.at_once = true;
        self
    }

    /// The same history, with the repository REPORTING ITSELF: its git
    /// files, every folder of its refs and its top folder, coalesced — a
    /// commit is an index, a ref and a log in one burst (`git_line::watch`,
    /// the line's own watch).
    #[must_use]
    pub fn watched(mut self) -> Self {
        if let Some(repo) = self.repo.as_ref() {
            let backs = Arc::clone(&self.backs);
            self.watch = crate::git_line::watch(
                &repo.git_dir,
                std::slice::from_ref(&repo.root),
                move || {
                    backs(Back::Moved);
                    true
                },
            )
            .map_err(|e| tracing::warn!(error = %e, "no watch on the repository"))
            .ok();
        }
        self
    }

    /// Reads what the view wants, each on a thread of its own. Outside a
    /// repository every answer is the refusal that says so — at once.
    pub fn ask(&mut self) {
        if self.at_once {
            self.ask_now();
            return;
        }
        for need in self.view.wanted() {
            let generation = self.view.generation();
            let Some(repo) = self.repo.clone() else {
                self.view.heard(refused(generation, need));
                continue;
            };
            let backs = Arc::clone(&self.backs);
            let started = std::thread::Builder::new()
                .name("git-browser".to_owned())
                .spawn(move || {
                    let answer = crate::git_browser::read(&repo, &need);
                    backs(Back::Heard(Heard {
                        generation,
                        need,
                        answer,
                    }));
                });
            if let Err(e) = started {
                tracing::warn!(error = %e, "a git reading could not start");
            }
        }
    }

    /// Reads what the view wants ON THIS THREAD, until it wants nothing —
    /// the photograph's, which must not draw before the answers it shows.
    pub fn ask_now(&mut self) {
        for _ in 0..8 {
            let wanted = self.view.wanted();
            if wanted.is_empty() {
                return;
            }
            for need in wanted {
                let generation = self.view.generation();
                let heard = match self.repo.as_ref() {
                    Some(repo) => Heard {
                        generation,
                        answer: crate::git_browser::read(repo, &need),
                        need,
                    },
                    None => refused(generation, need),
                };
                self.view.heard(heard);
            }
        }
    }

    /// What a thread came back with, taken — and what it needs read next.
    /// Whether a frame is owed.
    pub fn back(&mut self, b: Back, way: &mut dyn Doorway) -> bool {
        match b {
            Back::Heard(h) => {
                if !self.view.heard(h) {
                    return false;
                }
            }
            Back::Moved => self.view.stale(),
            Back::Fetched(done) => {
                let said = self.view.fetched(done);
                way.say(said, crate::echo::Echo::Normal);
            }
            Back::Kept(Ok((path, id, opener))) => Self::open_the_copy(way, &path, &id, opener),
            Back::Kept(Err(why)) => {
                let said = way.texts().t_with(
                    crate::key::FILES_GIT_KEEP_FAILED,
                    &[("why", &why.said(way.texts()))],
                );
                way.say(said, crate::echo::Echo::Long);
            }
        }
        self.ask();
        way.repaint();
        true
    }

    /// Opens the read-only copy with whatever opens its type — the file
    /// manager's own answer (`launch::opener_of`, asked on the copying
    /// thread) — and says it was copied out; a type nothing opens is said
    /// instead.
    fn open_the_copy(
        way: &mut dyn Doorway,
        path: &std::path::Path,
        id: &str,
        opener: crate::launch::Opener,
    ) {
        let launching = match opener {
            crate::launch::Opener::Editor => {
                let client = std::env::var("RONOCTUA_EMACSCLIENT").ok();
                let Some((program, args, cwd)) =
                    crate::launch::editor_command(client.as_deref(), path)
                else {
                    let said = way.texts().t(crate::key::FILES_NO_EDITOR);
                    way.say(said, crate::echo::Echo::Normal);
                    return;
                };
                crate::door::Launching::Program { program, args, cwd }
            }
            crate::launch::Opener::App(entry) => {
                let (program, args) = crate::launch::command_for(&entry, path);
                crate::door::Launching::Program {
                    program,
                    args,
                    cwd: entry.working_directory.clone(),
                }
            }
            crate::launch::Opener::Nothing(of_type) => {
                let said = way
                    .texts()
                    .t_with(crate::key::FILES_NOTHING_OPENS, &[("type", of_type)]);
                way.say(said, crate::echo::Echo::Normal);
                return;
            }
        };
        way.launch(launching);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let said = way.texts().t_with(
            crate::key::FILES_GIT_KEPT,
            &[("path", &name), ("id", crate::git_line::short(id))],
        );
        way.say(said, crate::echo::Echo::Normal);
    }
}

/// Every need refused as outside a repository.
fn refused(generation: u64, need: Need) -> Heard {
    Heard {
        generation,
        answer: crate::git_browser::refused_answer(&need),
        need,
    }
}

/// The session's language, for the catalogue's localised names.
fn language() -> String {
    std::env::var("LANG")
        .ok()
        .and_then(|l| l.split('.').next().map(ToOwned::to_owned))
        .unwrap_or_else(|| "en_GB".to_owned())
}

/// WHAT THE HISTORY ASKED, carried out in either door — the one errand
/// handler the window and the footer's region share.
pub fn carry_out(errand: Errand, git: &mut Git, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => return,
        Errand::Paint => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Say(key) => {
            let said = way.texts().t(key);
            way.say(said, crate::echo::Echo::Normal);
        }
        Errand::Copy(text) => {
            let ok = crate::clipboard_copy::copy_to_clipboard(&text).is_ok();
            let said = way.texts().t(if ok {
                crate::key::FILES_COPIED
            } else {
                crate::key::FILES_NOT_COPIED
            });
            way.say(said, crate::echo::Echo::Normal);
        }
        Errand::Terminal(root) => {
            if let Ok(me) = std::env::current_exe() {
                way.launch(crate::door::Launching::Program {
                    program: me.to_string_lossy().into_owned(),
                    args: vec!["terminal".to_owned()],
                    cwd: Some(root.to_string_lossy().into_owned()),
                });
            }
        }
        Errand::Fetch => match git.repo.clone() {
            Some(repo) => {
                let backs = Arc::clone(&git.backs);
                let _ = std::thread::Builder::new()
                    .name("git-fetch".to_owned())
                    .spawn(move || {
                        let done = repo.fetch(None).map_err(|r| r.why);
                        backs(Back::Fetched(done));
                    });
            }
            None => {
                let said = git.view.fetched(Err(ronoctua_git::Why::NotARepo));
                way.say(said, crate::echo::Echo::Normal);
            }
        },
        Errand::Keep { commit, path } => {
            let (Some(repo), Some(cache)) = (git.repo.clone(), crate::git_browser::versions_dir())
            else {
                let said = way.texts().t(crate::key::FILES_GIT_NO_VERSION);
                way.say(said, crate::echo::Echo::Normal);
                way.repaint();
                return;
            };
            let backs = Arc::clone(&git.backs);
            let catalogue = Arc::clone(&git.catalogue);
            let _ = std::thread::Builder::new()
                .name("git-version".to_owned())
                .spawn(move || {
                    // WHAT OPENS IT, chosen here with the copy: the type is
                    // read off the file and the catalogue swept the first
                    // time — both off the loop.
                    let kept =
                        crate::git_browser::keep_version(&repo, &commit, &path, &cache).map(|p| {
                            let apps = catalogue
                                .get_or_init(|| Arc::new(ronoctua_apps::scan(&language())));
                            let opener = crate::launch::opener_of(apps, &p);
                            (p, commit.0.clone(), opener)
                        });
                    backs(Back::Kept(kept));
                });
        }
    }
    git.ask();
    way.repaint();
}

/// The browser's region in cells and lines, on a door that is `columns`
/// wide (its margins included) with `room` lines for the region — ONE
/// answer for the rows, the paint, the hover, the click and the keys: two
/// of them measuring differently is a control grabbed somewhere other than
/// where it is drawn.
const fn shape_of(columns: u16, room: usize) -> (u16, usize) {
    (crate::git_browser::footer_columns(columns), room)
}

/// Where the region is, in pixels — what its bands, rules and bars are
/// painted and grabbed from.
fn px_of(area: &Area) -> crate::git_browser::Px {
    let f = area.frame();
    crate::git_browser::Px {
        lead: crate::chrome::FIELDS_LEAD,
        top: f.y_of(area.first),
        bottom: f.bottom(),
        edge: area.width,
        cell: f.cell(),
        line: f.line(),
    }
}

/// A REPOSITORY'S HISTORY IN THE FOOTER'S REGION, over the file manager.
pub struct GitRegion {
    git: Git,
    /// Where the threads' answers wait for the door's wake.
    heard: mpsc::Receiver<Back>,
}

impl std::fmt::Debug for GitRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitRegion")
            .field("git", &self.git)
            .finish_non_exhaustive()
    }
}

impl GitRegion {
    /// The history of the repository `root` is in, its threads waking the
    /// door, and the repository watched — or, in a photograph's footer,
    /// which has no loop to hear a thread on and reaches nothing of the
    /// machine, read at once and watched by nothing, as the window's camera
    /// reads it (`git_photograph`).
    fn at(root: &std::path::Path, way: &mut dyn Doorway) -> Self {
        let (sender, heard) = mpsc::channel();
        let wake = way.wake();
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        let git = Git::over(root, false, way.words(), zone, move |b| {
            if sender.send(b).is_ok() {
                wake.call();
            }
        });
        let mut git = if way.photographing() {
            git.at_once()
        } else {
            git.watched()
        };
        tracing::info!(root = %git.view.root().display(), "git browser opened");
        git.ask();
        Self { git, heard }
    }

    /// The region's cells and lines at what the door shows now.
    fn measured(&self, way: &dyn Doorway) -> (u16, usize) {
        let (columns, room) = shape_of(way.columns(), way.page_shown().lines);
        let _ = self.git.view.drawn(columns, room);
        (columns, room)
    }
}

impl Region for GitRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// Its columns laid onto the region's lines, each line every column's
    /// fields at the region's cells (`git_browser::Drawn`) — one click per
    /// line, and the view says which column and row the pointer's cell is.
    /// A line takes a click where something along it does
    /// (`Drawn::takes_line`), and the band then lights only what the click
    /// at the pointer's cell takes ([`Self::band`]).
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let (columns, room) = shape_of(seen.columns, usize::from(seen.lines));
        let drawn = self.git.view.drawn(columns, room);
        (0..drawn.room)
            .map(|n| Row {
                fields: drawn.line(n),
                takes: drawn.takes_line(n),
            })
            .collect()
    }

    /// Its table, the one its window teaches.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(crate::git_browser::OF_THE_VIEW, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        None
    }

    /// THE WHOLE REGION, in one column of lines that are its own columns
    /// laid side by side: one page of the whole ceiling.
    fn shape(&self) -> Shape {
        Shape::Whole
    }

    /// A key of the browser's, from the keyboard, the hint or `Esc`,
    /// measured against the region as it is drawn now.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let _ = self.measured(way);
        let errand = self.git.view.key(key);
        carry_out(errand, &mut self.git, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // OPEN ALREADY, on the folder it was opened for.
            OPEN => Taken::Yes,
            crate::dev_clone::COMMAND => {
                dev_clone(argument, way);
                Taken::Yes
            }
            _ => Taken::No,
        }
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    /// The CELL says which column and row — the view resolves it
    /// (`git_browser::Drawn::target_at`); the first click selects and the
    /// second is `RET` (invariant 19).
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let _ = self.measured(way);
        let errand = self.git.view.clicked(row, cell.unwrap_or(0));
        carry_out(errand, &mut self.git, way);
    }

    /// `LEADER / c`'s question, answered while the history is up.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        if id == crate::dev_clone::COMMAND {
            dev_clone(Some(text), way);
        }
    }

    /// ITS COLUMN'S BAND, on a row — and none where a click does nothing
    /// (`git_browser::hover_band`).
    fn band(&self, row: Option<usize>, cell: Option<u16>, area: &Area, seen: &Seen<'_>) -> Band {
        let (Some(line), Some(cell)) = (row, cell) else {
            return Band::Nothing;
        };
        let (columns, room) = shape_of(seen.columns, usize::from(seen.lines));
        let drawn = self.git.view.drawn(columns, room);
        crate::git_browser::hover_band(
            &drawn,
            &px_of(area),
            line,
            cell,
            tokens::color::SURFACE_HOVER,
        )
        .map_or(Band::Nothing, |q| {
            Band::Placed((q.x, q.y, q.width, q.height))
        })
    }

    /// ITS CURSOR IS THE ACCENT (`git_browser::item_fields`), and its
    /// pointer's mark is its own band — the window lights no field either,
    /// and the two doors draw the same.
    fn lights(&self) -> Lights {
        Lights::Nothing
    }

    /// The column under the pointer, and only it, takes the wheel: whether
    /// it moved (a column at its end leaves the footer's page, which is one
    /// whole page, where it is).
    fn wheel(&mut self, _row: usize, cell: u16, lines: i64, way: &mut dyn Doorway) -> bool {
        let _ = self.measured(way);
        self.git.view.wheel(cell, lines)
    }

    /// One of its bars, one per column that is cut, pressed where it is
    /// drawn — and moved there. The grip is the bar's own name, its column
    /// and its direction ([`grip_of`]): the hand that holds it finds it again
    /// among the bars drawn.
    fn grab(&mut self, at: (f64, f64), area: &Area, way: &mut dyn Doorway) -> Option<Grip> {
        let (columns, room) = self.measured(way);
        let drawn = self.git.view.drawn(columns, room);
        let px = px_of(area);
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = (at.0 as f32, at.1 as f32);
        let bar = crate::git_browser::bars(&drawn, &px)
            .into_iter()
            .find(|b| b.holds(x, y))?;
        if self.git.view.scroll_bar(&bar, bar.offset_at(x, y, &px)) {
            way.repaint();
        }
        Some(grip_of(&bar))
    }

    /// The bar `grip` names, dragged to `at`: whether it moved.
    fn drag(&mut self, grip: Grip, at: (f64, f64), area: &Area, way: &mut dyn Doorway) -> bool {
        let (columns, room) = self.measured(way);
        let drawn = self.git.view.drawn(columns, room);
        let px = px_of(area);
        let Some(bar) = crate::git_browser::bars(&drawn, &px)
            .into_iter()
            .find(|b| grip_of(b) == grip)
        else {
            return false;
        };
        #[allow(clippy::cast_possible_truncation)]
        let to = bar.offset_at(at.0 as f32, at.1 as f32, &px);
        self.git.view.scroll_bar(&bar, to)
    }

    /// The diff's bands, the rules between its columns and a bar per column
    /// that is cut — from the same `git_browser` functions its window paints
    /// with.
    fn paint(&self, easel: &mut RegionEasel<'_>) {
        let (columns, room) = shape_of(easel.area.columns, usize::from(easel.area.lines));
        let drawn = self.git.view.drawn(columns, room);
        let px = px_of(&easel.area);
        easel.under.extend(crate::git_browser::bands(&drawn, &px));
        easel
            .under
            .extend(crate::git_browser::dividers(&drawn, &px));
        for b in crate::git_browser::bars(&drawn, &px) {
            easel.bars.record(crate::bars::Track::of(b.track));
            if let Some((x, y, width, height)) = b.thumb {
                easel.under.push(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width,
                    height,
                    color: tokens::color::RULE_STRONG,
                });
            }
        }
    }

    /// What a thread came back with.
    fn woken(&mut self, way: &mut dyn Doorway) {
        while let Ok(b) = self.heard.try_recv() {
            let _ = self.git.back(b, way);
        }
    }

    /// THE WORDS CHANGED: the history keeps the ones it was opened with.
    fn speak(&mut self, texts: std::rc::Rc<ronoctua_i18n::Texts>) {
        self.git.view.speak(texts);
    }

    /// IT GOES WITH THE KEYBOARD, as the file manager it was opened over
    /// does: a history left on screen over nothing, answering no key, is
    /// worse than none.
    fn closes_without_the_keyboard(&self) -> bool {
        true
    }
}

/// A bar's grip ([`Grip`]): its column and whether it runs across the
/// column's foot — what names it among the bars drawn, whatever its thumb
/// shows now.
fn grip_of(bar: &crate::git_browser::BarBox) -> Grip {
    Grip(bar.col.index() * 2 + usize::from(bar.across))
}

/// `LEADER / c` — the distro's repository, cloned: asked where, the host let
/// by the answer, cloned on a thread whose answer comes back as this same
/// command (an argument no person types, `dev_clone::BACK`), and the folder
/// opened when it is there (`crate::dev_clone`).
pub fn dev_clone(argument: Option<&str>, way: &mut dyn Doorway) {
    clone_of(crate::dev_clone::address(), argument, way);
}

/// [`dev_clone`] with the repository's address GIVEN: the build's — which a
/// build is told once, when it is compiled (`option_env!`) — or a test's,
/// so a build with no address is proven whatever the build was told.
fn clone_of(address: Option<&str>, argument: Option<&str>, way: &mut dyn Doorway) {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let running = crate::dev_clone::running();
    let step = crate::dev_clone::step(
        argument,
        address,
        home.as_deref(),
        running.as_deref(),
        way.texts(),
    );
    match step {
        crate::dev_clone::Step::Ask { first, host } => {
            // THE QUESTION NAMES THE HOST: answering it lets it.
            let mut line = crate::line::Line::new(crate::dev_clone::COMMAND, false, Some(&first));
            line.asks_with = vec![("host", host)];
            way.ask(line);
        }
        crate::dev_clone::Step::Say(said) => way.say(said, crate::echo::Echo::Normal),
        // A DOOR WITH NO LOOP TO HEAR THE ANSWER ON — a photograph's, a
        // test's — starts no clone: nothing of the machine is reached, and
        // the sentence says the answer would not be heard.
        crate::dev_clone::Step::Clone(_) if way.photographing() => {
            let said = way.texts().t(crate::key::DEV_CLONE_UNHEARD);
            way.say(said, crate::echo::Echo::Normal);
        }
        crate::dev_clone::Step::Clone(plan) => {
            match crate::dev_clone::consent(&plan.host, way.texts()) {
                Err(said) => way.say(said, crate::echo::Echo::Long),
                Ok(pass) => {
                    let said = crate::dev_clone::cloning(&plan, way.texts());
                    let into = plan.into.clone();
                    // THE LOOP'S EAR for what the thread ends with: the
                    // door's orders, which run this command again with the
                    // answer.
                    let orders = way.orders();
                    let spawned =
                        std::thread::Builder::new()
                            .name("dev-clone".into())
                            .spawn(move || {
                                let answer = crate::dev_clone::run(&plan, &pass);
                                orders.run(crate::dev_clone::COMMAND, answer);
                            });
                    match spawned {
                        // HELD, as a state and not a gesture: the sentence
                        // stays until the answer (or another message)
                        // replaces it — no deadline takes it off while the
                        // clone is still running.
                        Ok(_) => {
                            crate::dev_clone::started(&into);
                            way.hold(said);
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "could not start the clone");
                            let said = way.texts().t(crate::key::DEV_CLONE_UNHEARD);
                            way.say(said, crate::echo::Echo::Normal);
                        }
                    }
                }
            }
        }
        crate::dev_clone::Step::Done { said, open } => {
            crate::dev_clone::ended();
            way.say(said, crate::echo::Echo::Long);
            if let Some(folder) = open {
                way.open_window(vec![
                    "files".to_owned(),
                    folder.to_string_lossy().into_owned(),
                ]);
            }
        }
    }
}

/// What a command does with the history closed: the manager's `C-h` opens
/// it, over the folder its argument carries; `LEADER / c` is carried out
/// without it.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        OPEN if root_of(argument).is_some() => Closed::Opens,
        crate::dev_clone::COMMAND => {
            dev_clone(argument, way);
            Closed::Taken
        }
        _ => Closed::NotMine,
    }
}

/// The folder [`OPEN`]'s argument carries, every byte of it — the
/// manager's folder, sent by `region::path_argument`: a folder whose name is
/// not text is that folder still, and not one that is not there.
fn root_of(argument: Option<&str>) -> Option<std::path::PathBuf> {
    argument.and_then(crate::region::argument_path)
}

/// The history of the repository the argument's folder is in — opened only
/// for an argument that carries one ([`closed`]).
fn open(_command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let root = root_of(argument).unwrap_or_default();
    Box::new(GitRegion::at(&root, way))
}

/// The history of a repository, as the binary registers it with the footer.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        Recorder::new(
            std::rc::Rc::new(texts),
            120,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
        .with_page(crate::region::Page {
            offset: 0,
            lines: 12,
        })
    }

    /// FAILS IF: the history opens without the folder it is for, or a
    /// command that is not its own opens it.
    #[test]
    fn the_history_opens_only_over_a_folder() {
        let mut w = way();
        assert_eq!(closed(OPEN, Some("/"), &mut w), Closed::Opens);
        assert_eq!(closed(OPEN, None, &mut w), Closed::NotMine);
        assert_eq!(closed(OPEN, Some("\u{0}zz"), &mut w), Closed::NotMine);
        assert_eq!(closed("files-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: outside a repository the region draws an empty history
    /// instead of every column saying so — or its rows are not one page of
    /// the room it was given, each taking a click.
    #[test]
    fn outside_a_repository_the_region_says_so() {
        let mut w = way();
        let mut r = GitRegion::at(std::path::Path::new("/"), &mut w);
        r.git.repo = None;
        r.git.ask_now();
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 120,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let rows = r.rows(&seen);
        assert_eq!(rows.len(), 12, "not one page of the room");
        assert!(rows.iter().all(|row| row.takes));
        let said: String = rows
            .iter()
            .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        let sentence = crate::git_line::why_said(ronoctua_git::Why::NotARepo, &texts);
        assert!(said.contains(&sentence), "{said}");
        assert_eq!(r.shape(), Shape::Whole);
        assert_eq!(r.lights(), Lights::Nothing);
    }

    /// A repository with one commit, in a room of its own — the git the
    /// tests run (`RONOCTUA_GIT`).
    fn a_repository(name: &str) -> std::path::PathBuf {
        a_repository_at(
            std::env::temp_dir().join(format!("ronoctua-git-region-{name}-{}", std::process::id())),
        )
    }

    /// The same repository, in the folder `work`.
    fn a_repository_at(work: std::path::PathBuf) -> std::path::PathBuf {
        let tools = ronoctua_git::Tools::from_env()
            .expect("RONOCTUA_GIT is not set: run inside `nix develop` or the tests derivation");
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).expect("a room");
        let git = |args: &[&str]| {
            let out = std::process::Command::new(&tools.git)
                .arg("-C")
                .arg(&work)
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Ada")
                .env("GIT_AUTHOR_EMAIL", "ada@example.org")
                .env("GIT_COMMITTER_NAME", "Ada")
                .env("GIT_COMMITTER_EMAIL", "ada@example.org")
                .output()
                .expect("git");
            assert!(out.status.success(), "{args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(work.join("a.txt"), "one\n").expect("a file");
        git(&["add", "a.txt"]);
        git(&["commit", "-qm", "one"]);
        work
    }

    /// FAILS IF: a photograph's footer opens the history on threads it has
    /// no loop to hear — every column `Reading…` for good — or watches the
    /// repository; or a live footer reads it on its own loop instead of
    /// threads, or forgets to watch it.
    #[test]
    fn a_photograph_reads_the_history_at_once_and_a_live_door_watches_it() {
        let work = a_repository("doors");
        let mut shot = way();
        let mut r = GitRegion::at(&work, &mut shot);
        assert!(r.git.repo.is_some(), "the room is no repository");
        assert!(
            r.git.view.wanted().is_empty(),
            "the photograph left readings for a thread"
        );
        assert!(
            r.git.watch.is_none(),
            "the photograph watches the repository"
        );
        // WHAT IT READ IS DRAWN: the commit's subject, in its history column.
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 120,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let said: Vec<String> = r
            .rows(&seen)
            .iter()
            .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(
            said.iter().any(|t| t.trim() == "one"),
            "the commit is not drawn: {said:?}"
        );

        let texts = std::rc::Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let mut live = Recorder::new(
            texts,
            120,
            false,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        );
        let l = GitRegion::at(&work, &mut live);
        assert!(!l.git.at_once, "a live footer reads on its own loop");
        assert!(
            l.git.watch.is_some(),
            "a live footer does not watch the repository"
        );
        let _ = std::fs::remove_dir_all(&work);
    }

    /// FAILS IF: the file manager's `C-h` over a folder whose name is not
    /// text opens the history of another folder — the name lossily turned
    /// into one that is not there, and every column saying it is no
    /// repository — instead of that repository's own.
    #[test]
    fn a_folder_whose_name_is_not_text_opens_its_own_history() {
        use std::os::unix::ffi::OsStrExt as _;
        let room =
            std::env::temp_dir().join(format!("ronoctua-git-region-bytes-{}", std::process::id()));
        let work = a_repository_at(room.join(std::ffi::OsStr::from_bytes(b"caf\xe9")));
        assert!(work.to_str().is_none(), "the folder's name is text");
        let argument = crate::region::path_argument(&work);
        let mut w = way();
        assert_eq!(closed(OPEN, Some(&argument), &mut w), Closed::Opens);
        let r = open(OPEN, Some(&argument), &mut w);
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 120,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let said: Vec<String> = r
            .rows(&seen)
            .iter()
            .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
            .collect();
        let _ = std::fs::remove_dir_all(&room);
        assert!(
            said.iter().any(|t| t.trim() == "one"),
            "the repository's commit is not drawn: {said:?}"
        );
    }

    /// FAILS IF: in a build with no address the clone is silence, asks a
    /// question it cannot answer or starts anything — on the door, the
    /// sentence says why, whatever was typed; or `LEADER / c` with the
    /// history closed is not carried out without it — taken, and the door
    /// told (this build's question, or its refusal).
    #[test]
    fn a_clone_this_build_cannot_make_is_said() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let said = texts.t(crate::key::DEV_CLONE_NO_ADDRESS);
        for argument in [None, Some("/home/ada/distro")] {
            let mut w = way();
            clone_of(None, argument, &mut w);
            assert_eq!(
                w.owed,
                vec![Effect::Say(said.clone(), crate::echo::Echo::Normal)],
                "{argument:?}"
            );
        }
        assert!(crate::dev_clone::running().is_none(), "a clone started");
        let mut w = way();
        assert_eq!(
            closed(crate::dev_clone::COMMAND, None, &mut w),
            Closed::Taken
        );
        assert!(
            matches!(w.owed.as_slice(), [Effect::Ask(_) | Effect::Say(_, _)]),
            "{:?}",
            w.owed
        );
    }

    /// FAILS IF: a door with no loop to hear the answer on — a photograph's
    /// — starts a clone: it would reach the network from a photograph, and
    /// its answer would go nowhere. The sentence says it would not be heard.
    #[test]
    fn a_door_with_no_loop_starts_no_clone() {
        let mut w = way();
        let into = std::env::temp_dir().join(format!(
            "ronoctua-git-region-no-clone-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&into);
        clone_of(
            Some("https://example.org/ronoctua-distro.git"),
            Some(&into.to_string_lossy()),
            &mut w,
        );
        assert!(crate::dev_clone::running().is_none(), "a clone started");
        assert!(!into.exists(), "the folder was made");
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        assert_eq!(
            w.owed,
            vec![Effect::Say(
                texts.t(crate::key::DEV_CLONE_UNHEARD),
                crate::echo::Echo::Normal
            )]
        );
    }

    /// FAILS IF: a finished clone's answer does not open the folder it made,
    /// in the file manager's window, with the sentence that says so.
    #[test]
    fn a_clone_that_ended_opens_its_folder() {
        let mut w = way();
        let answer = crate::dev_clone::back_of(Ok(()), std::path::Path::new("/home/ada/distro"));
        dev_clone(Some(&answer), &mut w);
        match w.owed.as_slice() {
            [Effect::Say(_, _), Effect::OpenWindow(args)] => {
                assert_eq!(
                    args,
                    &vec!["files".to_owned(), "/home/ada/distro".to_owned()]
                );
            }
            other => panic!("{other:?}"),
        }
    }
}
