//! THE MANAGER BOTH DOORS HOLD — the view, what stands beside it in either
//! door, and the ONE handler of what the view asks ([`Manager::carry_out`]) and
//! of what its threads come back with ([`Manager::reported`]).
//!
//! # Why one handler
//!
//! The view answers a key, a click, a paste or a thread's report with a
//! [`Step`]: a sentence to say, a question to ask, a program to start, a job
//! to hand the queue. Each door used to carry the step out with a handler of
//! its own — the footer's `apply_files_step` and `do_about_files`, the
//! window's `apply` — and two handlers of the same steps are how the two
//! doors come to do different things. Now the step is carried out here, once,
//! over the door's [`Doorway`]: the window's is its `Ui`, the footer's writes
//! down what is asked and does it once the region is back in its slot.
//!
//! # What still differs, and says so
//!
//! Two things are the door's own and are written here as such
//! ([`Door`]): the history of a repository opens in the footer's region from
//! the footer and in a window of its own from the window, and the `/private`
//! vault's `m` and `l` run the security centre's flow in the window that
//! holds it ([`crate::vault::Flow`]) and its two leader commands in the
//! footer, where the centre itself asks.

use std::path::Path;

use crate::door::Launching;
use crate::echo::Echo;
use crate::region::{Doorway, Paging};
use crate::report::{Doing, Report, Step};
use crate::wake::Out;

/// Which door holds the manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    /// The footer's region.
    Footer,
    /// A window of its own.
    Window,
}

/// What a step came to, for the door that carried it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    /// It left the view as it was — nothing, a question asked, a program
    /// started, the manager closed.
    Left,
    /// It moved or changed the view: a window's listing follows its cursor
    /// (the footer's page followed already, where its door does).
    Moved,
}

/// THE PANELS' THREADS — the inspector, the film and the shape
/// ([`crate::inspection::Threads`]), the thumbnails' hands and the one that
/// walks what moves, the player that tours the film under the pointer, and
/// the globe — and what each was last told.
pub struct Panels {
    /// The inspector, the film and the shape.
    pub threads: crate::inspection::Threads,
    /// The hands that make thumbnails — `None` when not one could be raised:
    /// the grid then draws its glyphs, which is honest.
    pub thumbing: Option<std::sync::mpsc::Sender<crate::thumbs::Ask>>,
    /// The thread that walks the frames of what moves by itself.
    pub winding: Option<std::sync::mpsc::Sender<crate::thumbs::Wind>>,
    /// What it was last told to animate, so an unchanged screen costs no
    /// message.
    pub wound: crate::thumbs::Wind,
    /// The player that tours the film under the POINTER.
    pub touring: Option<crate::motion::Player>,
    /// The thread that draws the globe.
    pub globe: Option<crate::globe::Stage>,
}

impl Panels {
    /// NO THREAD AT ALL — a door with nothing behind it (a photograph), whose
    /// panels are answered on the spot.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            threads: crate::inspection::Threads::none(),
            thumbing: None,
            winding: None,
            wound: crate::thumbs::Wind(Vec::new()),
            touring: None,
            globe: None,
        }
    }

    /// Raises them, each handing what it made to its sink — a channel of a
    /// window's loop, or a region's queue that wakes it.
    #[must_use]
    pub fn raise(
        panels: &Out<crate::inspection::Heard>,
        globes: Out<crate::globe::Beat>,
        thumbs: &Out<crate::thumbs::Made>,
        tours: Out<crate::motion::Beat>,
    ) -> Self {
        Self {
            threads: crate::inspection::Threads::to(panels),
            thumbing: crate::thumbs::pool(
                thumbs.clone(),
                ronoctua_xdg::cache_dir(),
                ronoctua_xdg::cache_root(),
            ),
            winding: crate::thumbs::animator(thumbs.clone()),
            wound: crate::thumbs::Wind(Vec::new()),
            touring: crate::motion::player(tours),
            globe: crate::globe::stage(globes),
        }
    }

    /// WHAT THE PANELS NEED, asked once a frame: the inspector about the file
    /// under the cursor, the thumbnails on screen, the film and the shape.
    ///
    /// Every other arrangement was a list of the ways the cursor can move — a
    /// key, a click, a re-read, a directory entered — and the one that got
    /// forgotten would be a panel showing the previous file with nothing
    /// saying why. Each wish answers `None` when the answer is already in
    /// hand, so this costs a comparison.
    pub fn look(&mut self, view: &mut crate::files::View, columns: u16, room: usize, scale: f32) {
        self.threads
            .look(|| view.wants_look(u32::from(ronoctua_tokens::space::FILES_PANELPIXELS)));
        self.thumbs(view, columns, room, scale);
        self.motion(view);
        self.turn(view);
    }

    /// Asks for the thumbnails on screen that are not in hand yet, the
    /// pointer's film, and what moves when the set changed.
    ///
    /// THE SCALE IS PART OF IT: a grid decoded in physical pixels is sharp
    /// beside sharp letters (`files::thumb_side`).
    pub fn thumbs(&mut self, view: &mut crate::files::View, columns: u16, room: usize, scale: f32) {
        let Some(queue) = self.thumbing.clone() else {
            return;
        };
        let size = ronoctua_tokens::text::size();
        let cell = ronoctua_tokens::metrics::cell_width(size) * scale;
        let line = ronoctua_tokens::metrics::line_height(size) * scale;
        for ask in view.wants_thumbs(columns, room, cell, line) {
            let _ = queue.send(ask);
        }
        // AND THE POINTER'S FILM, which is a wish like the preview's: asked
        // once a frame and answered `None` when nothing changed.
        if let Some(player) = self.touring.as_mut()
            && let Some(ask) = view.wants_tour()
        {
            player.tell(ask);
        }
        // AND WHAT MOVES, when the set has changed.
        if let Some(wind) = self.winding.as_ref() {
            let now = view.moving_thumbs(columns, room, cell, line);
            if now != self.wound {
                let _ = wind.send(now.clone());
                self.wound = now;
            }
        }
    }

    /// Starts, or stops, the player that follows a moving file — one
    /// question, one answer, passed straight on: the view decides what
    /// should be running by comparing values.
    pub fn motion(&mut self, view: &mut crate::files::View) {
        self.threads.play(|| view.wants_motion());
    }

    /// Starts, or stops, the thread that draws a shape — and tells the globe
    /// where it is, by the same value diff.
    pub fn turn(&mut self, view: &mut crate::files::View) {
        self.threads.turn(|| view.wants_turn());
        if let Some(stage) = self.globe.as_mut()
            && let Some(ask) = view.wants_globe()
        {
            stage.tell(ask);
        }
    }
}

/// THE MANAGER BOTH DOORS HOLD — see the module.
pub struct Manager {
    /// What is drawn, in both doors.
    pub view: crate::files::View,
    /// Everything the system offers, for "open with" and the sidebar's
    /// programs.
    pub catalogue: crate::launch::Catalogue,
    /// Where the threads the view's work goes to hand their report back.
    pub back: Out<Report>,
    /// The directory on screen reporting itself, when it could be watched.
    pub notice: Option<ronoctua_files::notice::Notice>,
    /// The git line's keeper — the repository the folder is in, read off the
    /// loop (`git_line::Keeper`); `None` in a photograph.
    pub git: Option<crate::git_line::Keeper>,
    /// The panels' threads.
    pub panels: Panels,
    /// Whether an archive's listing and a member's extraction are done IN
    /// PLACE, on this thread — the window's camera, whose report channel has
    /// no loop behind it.
    pub in_place: bool,
    /// NO WORK LEAVES THIS MANAGER — the footer's camera, which has no loop
    /// to hear a job end, and draws what it was asked for.
    pub hands_off: bool,
    /// Which door holds it.
    pub door: Door,
    /// The security centre's flow for the vault's `m` and `l`, when this door
    /// runs it — the window; the footer runs the centre's two commands.
    pub vault: Option<Box<dyn crate::vault::Flow>>,
    /// The vault's status a camera planted (`--private`), read in place of
    /// the file — no work is ever started from it.
    pub planted: Option<Option<ronoctua_private::Status>>,
}

impl Manager {
    /// Carries out what the view asked for, over `way`.
    ///
    /// Whatever costs more than a syscall goes to a NAMED thread and comes
    /// back through [`Manager::back`] — the lesson `systemd-run --scope`
    /// taught this project: waiting inside the loop that draws freezes the
    /// keyboard, the painting and the compositor's events for as long as the
    /// work takes.
    pub fn carry_out(&mut self, step: Step, way: &mut dyn Doorway) -> Carried {
        match step {
            Step::Nothing => return Carried::Left,
            // AND THE PAGE FOLLOWS THE CURSOR, as every other list does —
            // without it the cursor walks past the last line, which looks
            // exactly like a key that stopped working.
            Step::Changed => way.page(Paging::Follow),
            Step::Closed => {
                way.close();
                return Carried::Left;
            }
            // A `Say` IS NOT ONLY A MESSAGE: arriving on a row whose name
            // does not fit answers with the whole name, and that is a cursor
            // that moved — so the page follows it too.
            Step::Say(e) => {
                let said = way.texts().t_with(e.key, &e.borrowed());
                way.say(said, Echo::Normal);
                way.page(Paging::Follow);
            }
            // THE OPENED LINE IS THE DOOR'S — see `fields::Pane::Fold`. The
            // view holds the focus that reached its control and nothing else.
            Step::Fold(ask) => way.fold(ask),
            // THE CLIPBOARD, read off the loop — the live selection — for the
            // filter or the question's query, handed back as a paste.
            Step::Paste => {
                way.paste();
                return Carried::Left;
            }
            // A SECRET, on the door's own masked line. The answer comes back
            // to [`Manager::answered`].
            Step::Secret(id) => {
                way.ask(crate::line::Line::new(id, true, None));
                return Carried::Left;
            }
            Step::Do(doing) => {
                if !self.carry_out_doing(doing, way) {
                    self.follow_the_place();
                    way.repaint();
                    return Carried::Left;
                }
            }
        }
        self.follow_the_place();
        way.repaint();
        Carried::Moved
    }

    /// One operation: `true` when it went to the view's own work (a syscall
    /// done, a job handed to a thread), `false` when it left for a program,
    /// a window or a question.
    fn carry_out_doing(&mut self, doing: Doing, way: &mut dyn Doorway) -> bool {
        match doing {
            Doing::Open(path) => self.open_the_file(&path, way),
            // THE MAP, through the opener this system already has — the road
            // `s s a` takes to the Syncthing page, and the one thing the
            // manager does that leaves the machine.
            Doing::Web(address) => {
                match option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) {
                    Some(opener) => way.launch(Launching::Program {
                        program: opener.to_owned(),
                        args: vec![address],
                        cwd: None,
                    }),
                    None => tracing::error!("RONOCTUA_XDG_OPEN was not set at build time"),
                }
            }
            Doing::OpenWith(path, which) => {
                if let Some(entry) = self.catalogue.iter().find(|e| e.id == which).cloned() {
                    let (program, args) = crate::launch::command_for(&entry, &path);
                    way.launch(Launching::Program {
                        program,
                        args,
                        cwd: entry.working_directory,
                    });
                }
            }
            // AN APPLICATION PINNED TO THE SIDEBAR, by its id, through the
            // launcher's own `command_for` — never a second reader of a
            // `.desktop` file's `Exec`. An id the catalogue no longer has is
            // an application uninstalled since the row was pinned, and it
            // SAYS so: a row pressed that does nothing is the silent failure
            // this project refuses.
            Doing::Launch(which) => match self.catalogue.iter().find(|e| e.id == which).cloned() {
                Some(entry) => {
                    let (program, args) = crate::launch::command_for(&entry, Path::new(""));
                    way.launch(Launching::Program {
                        program,
                        args,
                        cwd: entry.working_directory,
                    });
                }
                None => {
                    let said = way
                        .texts()
                        .t_with(crate::key::FILES_SIDEBAR_GONE, &[("name", which.as_str())]);
                    way.say(said, Echo::Normal);
                }
            },
            // FORMATTING IS A BUFFER, and this door only opens it: the
            // questions belong on a surface with room for them, and the work
            // to the host, which re-checks the volume is one the running
            // system does not need.
            Doing::Format(id) => {
                match (
                    std::env::current_exe(),
                    crate::launch::buffer_component("RONOCTUA_BUFFER_FORMAT"),
                ) {
                    (Ok(me), Some(component)) => way.launch(Launching::Program {
                        program: me.to_string_lossy().into_owned(),
                        args: vec!["buffer".to_owned(), component, id],
                        cwd: None,
                    }),
                    (_, None) => {
                        tracing::error!(
                            variable = "RONOCTUA_BUFFER_FORMAT",
                            "the component's variable is not in the environment"
                        );
                        let said = way.texts().t(crate::key::ECHO_NO_BUFFER);
                        way.say(said, Echo::Normal);
                    }
                    (Err(e), _) => tracing::error!(error = %e, "I do not know my own binary"),
                }
            }
            // THE INSTALLER STICK, ON THIS DRIVE: its window, which asks its
            // own question.
            Doing::WriteInstaller(drive) => way.open_window(crate::doors::stick_window_on(&drive)),
            // CHECK WITH VIRUSTOTAL: the security centre's window, checking
            // the file — it asks its own questions.
            Doing::VirusTotal(path) => way.open_window(crate::doors::security_checking(&path)),
            Doing::Terminal(dir) => {
                if let Ok(me) = std::env::current_exe() {
                    way.launch(Launching::Program {
                        program: me.to_string_lossy().into_owned(),
                        args: vec!["terminal".to_owned()],
                        cwd: Some(dir.to_string_lossy().into_owned()),
                    });
                }
            }
            Doing::RootTerminal(dir) => Self::root_terminal(&dir, way),
            // THE /private VAULT'S `m` AND `l`: the centre's flow, where this
            // door runs it — its passphrase asked on this door's line — and
            // the centre's two leader commands where the centre itself asks.
            Doing::Vault(open) => match self.vault.as_mut() {
                Some(flow) => {
                    let walk = flow.gesture(open, self.planted, way);
                    self.walk(walk);
                }
                None => way.run(if open { "private-open" } else { "private-lock" }, None),
            },
            // THE HISTORY, in this door's kind of door: the footer's region
            // from the footer, a window of its own from the window. The folder
            // goes WHOLE to the region (`region::path_argument`): a name that
            // is not text would otherwise open the history of a folder that
            // is not there.
            Doing::History(root) => match self.door {
                Door::Footer => way.run(
                    crate::git_line::HISTORY,
                    Some(crate::region::path_argument(&root)),
                ),
                Door::Window => {
                    if let Ok(me) = std::env::current_exe() {
                        way.launch(Launching::Program {
                            program: me.to_string_lossy().into_owned(),
                            args: vec![
                                "git-browser".to_owned(),
                                root.to_string_lossy().into_owned(),
                            ],
                            cwd: Some(root.to_string_lossy().into_owned()),
                        });
                    }
                }
            },
            Doing::Copy(text) => {
                let ok = crate::clipboard_copy::copy_to_clipboard(&text).is_ok();
                let said = way.texts().t(if ok {
                    crate::key::FILES_COPIED
                } else {
                    crate::key::FILES_NOT_COPIED
                });
                way.say(said, Echo::Normal);
            }
            // IN PLACE, for the window's camera: the listing and the member
            // are what the frame shows, and there is no loop to wait on.
            Doing::Archive(
                doing @ (crate::report::ArchiveDoing::List { .. }
                | crate::report::ArchiveDoing::Open { .. }),
            ) if self.in_place => {
                let report = match doing {
                    crate::report::ArchiveDoing::List { archive, secret } => {
                        crate::files_archive::list_now(&archive, secret.as_ref())
                    }
                    crate::report::ArchiveDoing::Open {
                        archive,
                        member,
                        secret,
                    } => crate::files_archive::open_now(&archive, &member, secret.as_ref()),
                    crate::report::ArchiveDoing::Queue { .. } => return false,
                };
                self.reported(Report::Archive(Box::new(report)), way);
            }
            // THE FOOTER'S CAMERA hands nothing over: it has no loop to hear
            // the job end, and it draws what it was asked for.
            _ if self.hands_off => {}
            other => {
                let clip = self.view.clip().clone();
                if matches!(other, Doing::Paste(_)) {
                    self.view.clip_used();
                }
                // ROOT, if it comes to that, asked in the footer's sudo bar
                // whichever door pressed — the verdict said in this one.
                let gate = way.sudo();
                match crate::files::carry_out(other, &self.back, &clip, &gate) {
                    // A syscall that answered already: read again now, so
                    // the new value is on screen before the hand leaves the
                    // key. What went to a thread reads again when its report
                    // arrives instead.
                    crate::files::Errand::Done => self.view.reread(),
                    crate::files::Errand::Say(e) => {
                        let said = way.texts().t_with(e.key, &e.borrowed());
                        way.say(said, Echo::Normal);
                    }
                }
                return true;
            }
        }
        false
    }

    /// Opens a file with whatever handles its type — the editor, or the
    /// application that declares it: `launch::opener_of`, one answer for
    /// both doors.
    fn open_the_file(&self, path: &Path, way: &mut dyn Doorway) {
        match crate::launch::opener_of(&self.catalogue, path) {
            crate::launch::Opener::Editor => {
                // FROM THE ENVIRONMENT, and it may not be there: then a file
                // that opens nothing SAYS so — silence reads as a key that
                // stopped working.
                let client = std::env::var("RONOCTUA_EMACSCLIENT").ok();
                match crate::launch::editor_command(client.as_deref(), path) {
                    Some((program, args, cwd)) => {
                        tracing::info!(file = %path.display(), "opening in emacs");
                        way.launch(Launching::Program { program, args, cwd });
                    }
                    None => {
                        let said = way.texts().t(crate::key::FILES_NO_EDITOR);
                        way.say(said, Echo::Normal);
                    }
                }
            }
            crate::launch::Opener::App(entry) => {
                let (program, args) = crate::launch::command_for(&entry, path);
                way.launch(Launching::Program {
                    program,
                    args,
                    cwd: entry.working_directory.clone(),
                });
            }
            crate::launch::Opener::Nothing(of_type) => {
                let said = way
                    .texts()
                    .t_with(crate::key::FILES_NOTHING_OPENS, &[("type", of_type)]);
                way.say(said, Echo::Normal);
            }
        }
    }

    /// A root shell in this directory, through polkit — `--keep-cwd`, or the
    /// root shell lands in `/` and the gesture stops meaning "here". polkit
    /// is what asks; this process never sees a password.
    fn root_terminal(dir: &Path, way: &mut dyn Doorway) {
        let (Ok(me), Some(pkexec), Some(shell)) = (
            std::env::current_exe(),
            option_env!("RONOCTUA_PKEXEC"),
            option_env!("RONOCTUA_RESCUE_SHELL"),
        ) else {
            let said = way.texts().t(crate::key::FILES_NO_PKEXEC);
            way.say(said, Echo::Normal);
            return;
        };
        way.launch(Launching::Program {
            program: me.to_string_lossy().into_owned(),
            args: vec![
                "terminal".to_owned(),
                pkexec.to_owned(),
                "--keep-cwd".to_owned(),
                shell.to_owned(),
            ],
            cwd: Some(dir.to_string_lossy().into_owned()),
        });
    }

    /// Where the vault's flow sent the door: into the open vault, which this
    /// door's view walks into.
    fn walk(&mut self, walk: crate::vault::Walk) {
        if let crate::vault::Walk::Into(path) = walk {
            self.view.mounted(&path);
            self.follow_the_place();
        }
    }

    /// A SECRET the view asked for, typed on the door's line — an archive's
    /// or a disk's password, a seal's passphrase, the vault's (where this
    /// door runs the vault's flow). Root is never asked here: the footer's
    /// sudo bar asks it.
    pub fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) -> Carried {
        if crate::vault::QUESTIONS.contains(&id)
            && let Some(flow) = self.vault.as_mut()
        {
            let walk = flow.answered(id, text, way);
            self.walk(walk);
            way.repaint();
            return Carried::Left;
        }
        if crate::files_archive::QUESTIONS.contains(&id) || crate::seal::QUESTIONS.contains(&id) {
            let step = self.view.secret_answered(id, text);
            return self.carry_out(step, way);
        }
        Carried::Left
    }

    /// The secret question was let go: nothing waits for it any more.
    pub fn unanswered(&mut self, id: &str) {
        match self.vault.as_mut() {
            Some(flow) if crate::vault::QUESTIONS.contains(&id) => flow.unanswered(id),
            _ => self.view.secret_let_go(id),
        }
    }

    /// The vault's work answered: what came back is said, and the next
    /// question asked, in this door.
    pub fn vault_woken(&mut self, way: &mut dyn Doorway) {
        if let Some(flow) = self.vault.as_mut() {
            let walk = flow.woken(way);
            self.walk(walk);
            way.repaint();
        }
    }

    /// What a thread came back with.
    pub fn reported(&mut self, report: Report, way: &mut dyn Doorway) -> Carried {
        match report {
            Report::Done => self.view.reread(),
            Report::Measured(name, bytes) => self.view.measured(&name, bytes),
            // A SENTENCE BY ITSELF, said as it is said with the manager
            // closed: one wording, open or not.
            Report::Failed(_) | Report::Said(_) | Report::Door(_) => {
                Self::reported_closed(report, way);
            }
            // A REFUSAL WITH A NAME, turned into a sentence here — where the
            // language bundle is. `AlreadyMounted` is the odd one: something
            // else mounted it between the reading and the press, so the
            // honest thing is to read again and let the row become what it
            // now is.
            Report::Refused(which, why) => {
                if matches!(which, ronoctua_disks::Refusal::AlreadyMounted) {
                    self.view.reread();
                } else {
                    let e = self.view.refusal_said(which, &why);
                    let said = way.texts().t_with(e.key, &e.borrowed());
                    way.say(said, Echo::Normal);
                }
            }
            Report::Mounted(at) => self.view.mounted(&at),
            // A DISK MOVED — and the sidebar's file, read when it changed:
            // the window has no clock, so a disk's signal is the cheapest
            // honest moment to notice that something was pinned elsewhere,
            // and in the footer it is one `stat` before its second does it.
            Report::Racked(rack) => {
                if self.view.sidebar_stale() {
                    self.view.reread_sidebar();
                }
                self.view.racked(&rack);
            }
            // SOMEBODY ELSE TOUCHED THE DIRECTORY. Answered with a plain
            // re-read and NOT with `stale()`: that asks the directory's
            // mtime, which a file rewritten in place does not move.
            //
            // The sidebar keeps its `stale()`, deliberately: `save_sidebar`
            // writes our OWN file, so an unconditional re-read here would
            // throw the sidebar's cursor away every time something is pinned.
            Report::Stirred => {
                // THE VAULT'S STATUS may be what moved: its row first, then
                // the folder — which may be the vault's.
                self.view.vault_moved();
                self.view.reread();
                if self.view.sidebar_stale() {
                    self.view.reread_sidebar();
                }
                // A FILE OF THE WORK TREE MOVED: the git line reads again.
                if let Some(k) = self.git.as_ref() {
                    k.nudge();
                }
            }
            Report::Git(line) => {
                if !self.view.git_heard(*line) {
                    return Carried::Left;
                }
            }
            // A WRITER OF THE FACTS CAME BACK: the file is read again either
            // way, and what it says — a refusal, the copy's name — is said
            // in the words both doors use (`inspection_edit::sentence`).
            Report::Amended(_) => {
                self.view.reread();
                Self::reported_closed(report, way);
            }
            // AN ARCHIVE'S ANSWER — a listing, a member out, a job refused —
            // turned into a step by the view.
            Report::Archive(r) => {
                let step = self.view.archive_reported(*r);
                return self.carry_out(step, way);
            }
            // A SEAL'S ANSWER — a header read, the queue's refusal.
            Report::Seal(r) => {
                let step = self.view.seal_reported(*r);
                return self.carry_out(step, way);
            }
        }
        // A MOUNT MOVES THE VIEW: `Report::Mounted` goes into the directory it
        // just mounted, so the watch follows from here too.
        self.follow_the_place();
        way.repaint();
        Carried::Left
    }

    /// What a thread's report says with the manager CLOSED — in the footer,
    /// where an errand outlives the view that started it: a lock or an eject
    /// udisks has not answered yet, a change as root, a seal handed to the
    /// queue, a writer of the facts (`files_region`, `Region::lingers`).
    ///
    /// ONLY WHAT IS A SENTENCE BY ITSELF is said — the failure, the sentence,
    /// the sudo door's refusal and what a writer of the facts did — as the
    /// footer always said it with no view open, and in the words the open
    /// manager says it in ([`Manager::reported`] asks this). The rest needs
    /// the view it is gone with: a listing to read again, a refusal worded
    /// from its row, a mount to walk into, an archive's or a seal's answer
    /// to turn into a step.
    pub fn reported_closed(report: Report, way: &mut dyn Doorway) {
        match report {
            Report::Failed(why) => way.say(why, Echo::Normal),
            Report::Said(e) => {
                let said = way.texts().t_with(e.key, &e.borrowed());
                way.say(said, Echo::Normal);
            }
            Report::Door(why) => {
                let said = way.texts().t(crate::security_words::refusal(why));
                way.say(said, Echo::Normal);
            }
            Report::Amended(amended) => {
                if let Some(said) = crate::inspection_edit::sentence(&amended, way.texts()) {
                    way.say(said, Echo::Normal);
                }
            }
            Report::Done
            | Report::Measured(..)
            | Report::Refused(..)
            | Report::Mounted(_)
            | Report::Racked(_)
            | Report::Stirred
            | Report::Git(_)
            | Report::Archive(_)
            | Report::Seal(_) => {}
        }
    }

    /// What the panels' threads came back with — what is inside the file, a
    /// frame of its film, a view of its shape.
    ///
    /// AND THE PLAYER IS ASKED AGAIN when it was the inspector's answer:
    /// that is the moment the answer arrives, and the answer is what says the
    /// file moves. Without it the player only ever started when the cursor
    /// moved AFTER the answer.
    pub fn panels_heard(&mut self, heard: crate::inspection::Heard, way: &mut dyn Doorway) {
        if self.view.heard(heard) {
            self.panels.motion(&mut self.view);
            self.panels.turn(&mut self.view);
            self.follow_the_place();
        }
        way.repaint();
    }

    /// Points the watch, and the git line, at whatever directory the view is
    /// on now.
    ///
    /// Called after anything that could have navigated, rather than at the
    /// one place that navigates: `View::enter` is inside the view, which
    /// knows nothing about watches. When the place has not moved this is a
    /// comparison of two paths and nothing else, which is what makes calling
    /// it after every key honest.
    pub fn follow_the_place(&mut self) {
        if let Some(n) = self.notice.as_mut() {
            // THE ARCHIVE'S OWN FOLDER while the view stands inside one: a
            // path inside an archive is not on the disk to be watched.
            n.look_at(self.view.watched());
        }
        if let Some(g) = self.git.as_mut() {
            g.follow(self.view.where_());
        }
    }

    /// Stops watching, keeping the threads: the manager closed. A watch on
    /// the last directory would wake a door that has no view to read again.
    pub fn rest(&mut self) {
        if let Some(n) = self.notice.as_mut() {
            n.rest();
        }
        if let Some(k) = self.git.as_mut() {
            k.rest();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-manager-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    fn texts() -> std::rc::Rc<ronoctua_i18n::Texts> {
        std::rc::Rc::new(
            ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language"),
        )
    }

    /// A manager on `dir`, in `door`, with no thread behind it.
    fn manager(dir: &Path, door: Door) -> Manager {
        let mut prefs = ronoctua_files::sidebar::Preferences::fresh();
        prefs.showing = false;
        let side = ronoctua_files::sidebar::Sidebar::of(&[], None, None, prefs, None);
        Manager {
            view: crate::files::View::at(
                dir,
                ronoctua_files::who::Roster::default(),
                ronoctua_files::when::Zone::utc(),
                None,
                std::sync::Arc::new(Vec::new()),
                texts(),
                side,
            ),
            catalogue: std::sync::Arc::new(Vec::new()),
            back: Out::new(|_| true),
            notice: None,
            git: None,
            panels: Panels::none(),
            in_place: false,
            hands_off: false,
            door,
            vault: None,
            planted: None,
        }
    }

    /// The footer's doorway, which writes down what it is asked.
    fn way() -> Recorder {
        Recorder::new(
            texts(),
            100,
            true,
            crate::sudo::Gate::shut(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// FAILS IF: a step the view gives is not carried out over the doorway —
    /// a question asked on one door's own line, a follow done to one door's
    /// own page — or a step that leaves the view says it moved it (the
    /// window would follow a cursor nothing moved).
    #[test]
    fn every_step_is_carried_out_over_the_doorway() {
        let dir = scratch("steps");
        let mut m = manager(&dir, Door::Footer);
        let cases: Vec<(Step, Vec<Effect>, Carried)> = vec![
            (Step::Nothing, vec![], Carried::Left),
            (
                Step::Changed,
                vec![Effect::Page(Paging::Follow), Effect::Repaint],
                Carried::Moved,
            ),
            (Step::Paste, vec![Effect::Paste], Carried::Left),
            (
                Step::Secret(crate::files_archive::PASSWORD),
                vec![Effect::Ask(crate::line::Line::new(
                    crate::files_archive::PASSWORD,
                    true,
                    None,
                ))],
                Carried::Left,
            ),
            (
                Step::Fold(crate::fold::Ask::By(2)),
                vec![Effect::Fold(crate::fold::Ask::By(2)), Effect::Repaint],
                Carried::Moved,
            ),
            (Step::Closed, vec![Effect::Close], Carried::Left),
        ];
        for (step, owed, carried) in cases {
            let mut w = way();
            let said = format!("{step:?}");
            assert_eq!(m.carry_out(step, &mut w), carried, "{said}");
            assert_eq!(w.owed, owed, "{said}");
        }
        // A SENTENCE IS SAID, and the page follows the cursor it may have
        // moved (a name too long for its column, answered whole).
        let mut w = way();
        let e = ronoctua_files::mode::Explanation {
            key: crate::key::FILES_COPIED,
            args: Vec::new(),
        };
        assert_eq!(m.carry_out(Step::Say(e), &mut w), Carried::Moved);
        assert!(
            matches!(
                w.owed.first(),
                Some(Effect::Say(_, crate::echo::Echo::Normal))
            ),
            "{:?}",
            w.owed
        );
        assert!(
            w.owed.contains(&Effect::Page(Paging::Follow)),
            "{:?}",
            w.owed
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the history of a repository opens in the wrong kind of door —
    /// the footer's region from a window, a window from the footer — which is
    /// the one difference between the doors this handler writes down.
    #[test]
    fn the_history_opens_in_the_doors_own_kind() {
        let dir = scratch("history");
        let root = dir.join("repo");
        let mut footer = manager(&dir, Door::Footer);
        let mut w = way();
        assert_eq!(
            footer.carry_out(Step::Do(Doing::History(root.clone())), &mut w),
            Carried::Left
        );
        assert!(
            w.owed.contains(&Effect::Run(
                crate::git_line::HISTORY,
                Some(crate::region::path_argument(&root))
            )),
            "{:?}",
            w.owed
        );
        let mut window = manager(&dir, Door::Window);
        let mut w = way();
        let _ = window.carry_out(Step::Do(Doing::History(root.clone())), &mut w);
        let launched = w.owed.iter().find_map(|e| match e {
            Effect::Launch(Launching::Program { args, cwd, .. }) => {
                Some((args.clone(), cwd.clone()))
            }
            _ => None,
        });
        assert_eq!(
            launched,
            Some((
                vec![
                    "git-browser".to_owned(),
                    root.to_string_lossy().into_owned()
                ],
                Some(root.to_string_lossy().into_owned())
            )),
            "{:?}",
            w.owed
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A vault flow that remembers it was asked, and walks into `into`.
    struct Fake {
        asked: std::rc::Rc<std::cell::Cell<bool>>,
        into: PathBuf,
    }

    impl crate::vault::Flow for Fake {
        fn gesture(
            &mut self,
            _: bool,
            _: Option<Option<ronoctua_private::Status>>,
            _: &mut dyn Doorway,
        ) -> crate::vault::Walk {
            self.asked.set(true);
            crate::vault::Walk::Into(self.into.clone())
        }
        fn answered(
            &mut self,
            _: &'static str,
            _: &str,
            _: &mut dyn Doorway,
        ) -> crate::vault::Walk {
            crate::vault::Walk::Stay
        }
        fn unanswered(&mut self, _: &str) {}
        fn woken(&mut self, _: &mut dyn Doorway) -> crate::vault::Walk {
            crate::vault::Walk::Stay
        }
    }

    /// FAILS IF: the vault's `m` runs anywhere but where the door holds its
    /// flow — the footer runs the centre's command, a window its flow — or
    /// the open vault the flow walks into is not where the view goes.
    #[test]
    fn the_vault_runs_where_the_door_holds_its_flow() {
        let dir = scratch("vault");
        let inside = dir.join("inside");
        std::fs::create_dir_all(&inside).expect("the vault's folder");
        let mut footer = manager(&dir, Door::Footer);
        let mut w = way();
        let _ = footer.carry_out(Step::Do(Doing::Vault(true)), &mut w);
        assert!(
            w.owed.contains(&Effect::Run("private-open", None)),
            "{:?}",
            w.owed
        );
        let asked = std::rc::Rc::new(std::cell::Cell::new(false));
        let mut window = manager(&dir, Door::Window);
        window.vault = Some(Box::new(Fake {
            asked: std::rc::Rc::clone(&asked),
            into: inside.clone(),
        }));
        let mut w = way();
        let _ = window.carry_out(Step::Do(Doing::Vault(false)), &mut w);
        assert!(asked.get(), "the window's flow was not asked");
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Run(..))),
            "the window sent the vault to the footer: {:?}",
            w.owed
        );
        assert_eq!(window.view.where_(), inside.as_path());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a window of another feature is opened by any words but its
    /// own — the stick's, the security centre's — which both doors write.
    #[test]
    fn another_features_window_opens_by_its_words() {
        let dir = scratch("windows");
        let mut m = manager(&dir, Door::Footer);
        let file = dir.join("a.exe");
        for (doing, words) in [
            (
                Doing::WriteInstaller("sdz".to_owned()),
                crate::doors::stick_window_on("sdz"),
            ),
            (
                Doing::VirusTotal(file.clone()),
                crate::doors::security_checking(&file),
            ),
        ] {
            let mut w = way();
            assert_eq!(m.carry_out(Step::Do(doing), &mut w), Carried::Left);
            assert!(w.owed.contains(&Effect::OpenWindow(words)), "{:?}", w.owed);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the footer's camera hands an operation over — it has no loop
    /// to hear one end, and draws what it was asked for — or a manager that
    /// may hand it over does not do it, synchronously when it is a syscall.
    #[test]
    fn the_footers_camera_hands_nothing_over() {
        use std::os::unix::fs::MetadataExt as _;
        let dir = scratch("hands-off");
        let file = dir.join("dated");
        std::fs::write(&file, b"x").expect("a file");
        let before = std::fs::metadata(&file).expect("its mtime").mtime();
        let mut m = manager(&dir, Door::Footer);
        m.hands_off = true;
        let mut w = way();
        let _ = m.carry_out(Step::Do(Doing::Modified(file.clone(), 981_173_106)), &mut w);
        assert_eq!(std::fs::metadata(&file).expect("its mtime").mtime(), before);
        m.hands_off = false;
        assert_eq!(
            m.carry_out(Step::Do(Doing::Modified(file.clone(), 981_173_106)), &mut w),
            Carried::Moved
        );
        assert_eq!(
            std::fs::metadata(&file).expect("its mtime").mtime(),
            981_173_106
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a report that is a sentence by itself — a failure, a
    /// sentence, the sudo door's refusal, what a writer of the facts did — is
    /// not said with the manager closed, or is said in other words than the
    /// open manager says it; or one that needs the view (a re-read, a refusal
    /// worded from its row, a mount to walk into) is said with no view.
    #[test]
    fn a_closed_manager_says_what_needs_no_view() {
        let dir = scratch("closed");
        let said = vec![
            Report::Failed("udisks is not there".to_owned()),
            Report::Said(ronoctua_files::mode::Explanation {
                key: crate::key::FILES_UNLOCK_LOCKED,
                args: vec![("name", "STICK".to_owned())],
            }),
            Report::Door(ronoctua_security::Why::WrongPassword),
            Report::Amended(crate::report::Amended::Located {
                landed: vec![dir.join("a.jpg")],
                copies: false,
            }),
        ];
        for report in said {
            let mut closed = way();
            Manager::reported_closed(report.clone(), &mut closed);
            let [Effect::Say(words, Echo::Normal)] = closed.owed.as_slice() else {
                panic!("{report:?} was not said closed: {:?}", closed.owed);
            };
            assert!(!words.is_empty(), "{report:?} was said as nothing");
            let mut open = way();
            let _ = manager(&dir, Door::Footer).reported(report.clone(), &mut open);
            assert!(
                open.owed
                    .contains(&Effect::Say(words.clone(), Echo::Normal)),
                "{report:?} is said otherwise open: {:?}",
                open.owed
            );
        }
        let unsaid = vec![
            Report::Done,
            Report::Measured("a".to_owned(), 7),
            Report::Refused(ronoctua_disks::Refusal::Busy, "busy".to_owned()),
            Report::Mounted(dir.clone()),
            Report::Stirred,
            Report::Amended(crate::report::Amended::Written),
        ];
        for report in unsaid {
            let mut closed = way();
            Manager::reported_closed(report.clone(), &mut closed);
            assert!(closed.owed.is_empty(), "{report:?}: {:?}", closed.owed);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
