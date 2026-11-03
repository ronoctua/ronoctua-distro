//! THE HELD TERMINALS, from the footer's side — the census of every terminal
//! a holder keeps (`ronoctua_terminals`), the modeline's mark for the ones no
//! window shows, `terminal-show` and `window-of-process`, and what a terminal
//! nobody is looking at asks for, said.
//!
//! The view of them is a region of the terminal's own crate, hosted by its
//! id like any other; what is here lives while the footer does, because a
//! terminal rings and a program waits with no view open. The view reads the
//! same table, lent ([`Held::census`], `held_by_the_footer!`).
//!
//! # A mirror of the files, and nothing older
//!
//! Each holder writes its WHOLE state beside its socket on every change
//! (`<id>.json` in `$XDG_RUNTIME_DIR/<name>-terms/`, per instance). The
//! footer watches that folder (inotify, `Notice::on_dirs`) and reads every
//! state file again on each stir, on a thread of its own, and the table on
//! the loop is what the files said last — never kept across a start, never
//! read on the loop, and no socket opened for it. A reading of the folder is
//! the truth of now, and the footer that starts reads the same thing one
//! that never stopped would.
//!
//! The one question a file cannot answer is whether its holder still lives:
//! one killed with `SIGKILL` leaves its file. The thread asks each socket
//! once when it starts (`Registry::census`, which clears what is dead), and
//! again when somebody reaches for the background terminals — the mark's
//! click, a `terminal-show` of one no window shows — so a dead holder costs
//! one wrong count until the next hand.
//!
//! # What is said, and when
//!
//! A TRANSITION of a terminal that is not seen — in the background, behind
//! another in its window, or in a window without the keyboard — is said
//! once ([`Signals`]): a program's notification (OSC 9/777) as the
//! notification it asked for, one per terminal and the next replacing it; a
//! bell, a program blocked on the person (OSC 7501) and a long command ended
//! (OSC 133) as events of the minibuffer's line. A bell or a notification
//! that comes while a program in the terminal waits on the person is that
//! wait's, and is not said again; a bell that comes with a notification is
//! the notification's sound. The FIRST reading is the baseline: a footer
//! that starts takes in what every terminal already says, and raises
//! nothing for it.

use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc;

use ronoctua_terminals::registry::{Census, Registry};
use ronoctua_terminals::state::{ProgramState, TerminalState};
use ronoctua_terminals::{GroupId, TerminalId};

use super::*;

/// How long a command must have run for its end to be said: the end of a
/// `git status` is not news, the end of a build somebody walked away from is
/// (the question Ghostty's `notify-on-command-finish-after` answers). A time
/// a person waits through, and not a visual value.
pub(super) const LONG_COMMAND_MS: u64 = 10_000;

/// The most windows [`Held::lately`] remembers the focus of: far more than
/// one program shares its process between, and a bound on a list that grows
/// with every focus.
const LATELY_MOST: usize = 32;

/// The background view's door in the footer, which the mark's click opens:
/// the terminal crate's region, registered by its id (`terminals-open`, its
/// `RET b b`). The footer names no feature — this is the command's id, the
/// one a key also reaches.
const TERMINALS_OPEN: &str = "terminals-open";

/// What the census thread is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    /// The folder moved: read the state files again.
    Stir,
    /// Somebody reaches for the background terminals: ask each socket too,
    /// and forget the files of a holder that died.
    Sweep,
}

/// What the footer keeps of the held terminals, for the session.
#[derive(Default)]
pub(super) struct Held {
    /// THE TABLE: every held terminal's whole state, as its file last said
    /// it, by group and place in it — the mark, the view (lent, by type) and
    /// `terminal-show` read this and nothing else.
    pub(super) census: Census,
    /// What has been said of each terminal already.
    signals: Signals,
    /// The local notification each terminal's notice took — the next one of
    /// the same terminal takes the same id and replaces it.
    notices: BTreeMap<TerminalId, u32>,
    /// The terminal the latest signal was about: what `terminal-show` with no
    /// argument shows, while it still asks for the person.
    last_asked: Option<TerminalId>,
    /// A show waiting for its window: the terminal, and its viewer's pid.
    /// Resolved on the next event of the compositor's windows, and dropped
    /// when the census says the terminal has another viewer — no clock.
    pending_show: Option<(TerminalId, u32)>,
    /// A window being opened for a group (`terminal --show`): a second show
    /// of the group meanwhile — a double `RET` — is not a second window.
    opening: Option<GroupId>,
    /// The windows, by how lately they had the focus, latest first: of the
    /// windows of ONE process (an editor's), the one a person was last in.
    lately: VecDeque<u64>,
    /// The registry, when this session has one (`XDG_RUNTIME_DIR`).
    registry: Option<Registry>,
    /// The census thread's orders.
    orders: Option<mpsc::Sender<Order>>,
    /// The watch on the registry's folder, kept alive.
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl Held {
    /// Starts the census: the registry's folder made (`0700`) and watched,
    /// and a thread that reads it — once at once, and again on every stir —
    /// and hands each reading to `sender`. Nothing happens on the loop but
    /// the folder's making.
    pub(super) fn watch(&mut self, sender: calloop::channel::Sender<Vec<TerminalState>>) {
        let Some(registry) = Registry::of_session() else {
            tracing::warn!("no runtime directory; the held terminals are not counted");
            return;
        };
        // MADE BEFORE IT IS WATCHED: a watch on a folder that is not there
        // is no watch, and the first terminal is born after the footer.
        if let Err(e) = registry.ensure() {
            tracing::warn!(error = %e, dir = %registry.dir().display(), "the terminals' registry could not be made");
            return;
        }
        let (orders, told) = mpsc::channel::<Order>();
        let stirred = orders.clone();
        self._watch = ronoctua_files::notice::Notice::on_dirs(
            &[registry.dir().to_path_buf()],
            &[],
            // COALESCED: a holder writes its state whole on every change it
            // has, and a burst of them (a program retitling itself as it
            // works) is one reading of the folder.
            ronoctua_files::notice::Pace::Coalesced,
            move || stirred.send(Order::Stir).is_ok(),
        )
        .map_err(|e| tracing::warn!(error = %e, "the terminals' registry is not watched"))
        .ok();
        let reading = registry.clone();
        let started = std::thread::Builder::new()
            .name("ronoctua-terminals-census".to_owned())
            .spawn(move || census_thread(&reading, &told, &sender));
        if let Err(e) = started {
            tracing::warn!(error = %e, "the terminals' census thread did not start");
            return;
        }
        self.registry = Some(registry);
        self.orders = Some(orders);
    }

    /// Asks the census thread to ask every holder whether it lives, and to
    /// forget the files of one that does not.
    pub(super) fn sweep(&self) {
        if let Some(orders) = &self.orders {
            let _ = orders.send(Order::Sweep);
        }
    }

    /// The modeline's mark: how many terminals no window shows, and whether
    /// one of them asks for the person — `None` while every one has a window.
    #[must_use]
    pub(super) fn away_mark(&self) -> Option<(usize, bool)> {
        away_mark(&self.census)
    }

    /// Asks `id`'s holder, on a thread, to put it in front of its group: its
    /// viewer fronts it when the holder's `front_asked` moves.
    fn front(&self, id: &TerminalId) {
        let Some(registry) = self.registry.clone() else {
            return;
        };
        let id = id.clone();
        let asked = std::thread::Builder::new()
            .name("ronoctua-terminal-front".to_owned())
            .spawn(move || {
                if let Err(e) = registry.front(&id) {
                    tracing::warn!(error = %e, terminal = %id, "the terminal's holder was not told to front it");
                }
            });
        if let Err(e) = asked {
            tracing::warn!(error = %e, "could not ask a terminal to the front");
        }
    }

    /// The id `terminal`'s next notice takes, and whether it replaces one:
    /// the id its last notice took, else the next the footer counts down to
    /// (`locally_raised`, which the bus's ids never meet — `Footer::arrived`).
    fn notice_id(&mut self, terminal: &TerminalId, locally_raised: &mut u32) -> (u32, bool) {
        if let Some(&id) = self.notices.get(terminal) {
            return (id, true);
        }
        *locally_raised = locally_raised.saturating_sub(1).max(1);
        self.notices.insert(terminal.clone(), *locally_raised);
        (*locally_raised, false)
    }

    /// A window had the focus: first among the ones lately.
    fn focused(&mut self, window: u64) {
        self.lately.retain(|w| *w != window);
        self.lately.push_front(window);
        self.lately.truncate(LATELY_MOST);
    }

    /// The terminal `terminal-show` with no argument shows: the one the
    /// latest signal was about, while it still asks for the person.
    fn last_asking(&self) -> Option<TerminalId> {
        let id = self.last_asked.as_ref()?;
        self.census
            .terminals
            .iter()
            .any(|t| &t.id == id && t.asks_attention())
            .then(|| id.clone())
    }

    /// What a new reading settles of what waited on the old one: a show
    /// waits for a viewer the terminal still has, a window being opened
    /// for a group that still has none, a notice id for a terminal there.
    fn settle(&mut self) {
        let terminals = &self.census.terminals;
        if let Some((id, viewer)) = &self.pending_show
            && !terminals
                .iter()
                .any(|t| &t.id == id && t.viewer == Some(*viewer))
        {
            self.pending_show = None;
        }
        if let Some(group) = &self.opening
            && !terminals
                .iter()
                .any(|t| &t.group == group && t.viewer.is_none())
        {
            self.opening = None;
        }
        self.notices
            .retain(|id, _| terminals.iter().any(|t| &t.id == id));
        if self
            .last_asked
            .as_ref()
            .is_some_and(|id| !terminals.iter().any(|t| &t.id == id))
        {
            self.last_asked = None;
        }
    }
}

/// The census thread: one reading at once — asking every socket, which
/// clears what a holder killed before this footer started left — then one
/// per stir, the stirs that came meanwhile taken together. It ends when the
/// footer is gone (its channel closed) or nothing can stir it any more.
fn census_thread(
    registry: &Registry,
    told: &mpsc::Receiver<Order>,
    sender: &calloop::channel::Sender<Vec<TerminalState>>,
) {
    let mut order = Some(Order::Sweep);
    while let Some(now) = order {
        let mut sweep = now == Order::Sweep;
        while let Ok(more) = told.try_recv() {
            sweep |= more == Order::Sweep;
        }
        let states = if sweep {
            registry.census().terminals
        } else {
            registry.states()
        };
        if sender.send(states).is_err() {
            return;
        }
        order = told.recv().ok();
    }
}

/// The records a terminal's new notice replaces that still count for the
/// bell: unread, of its id AND about it — a record of a past session given
/// the same id is somebody else's, and stays as it is.
fn superseded(
    history: &[ronoctua_notify::Record],
    id: u32,
    subject: &str,
) -> Vec<ronoctua_notify::RecordKey> {
    history
        .iter()
        .filter(|r| {
            !r.read && r.notification.id == id && r.notification.subject.as_deref() == Some(subject)
        })
        .map(|r| r.notification.key())
        .collect()
}

/// [`Held::away_mark`], over a table.
#[must_use]
pub(super) fn away_mark(census: &Census) -> Option<(usize, bool)> {
    let count = census.away() + census.ended().count();
    (count > 0).then(|| (count, census.attention()))
}

/// How a terminal goes by on a line: its program's title, else its program.
fn name_of(t: &TerminalState) -> String {
    if t.title.is_empty() {
        t.program.clone()
    } else {
        t.title.clone()
    }
}

/// What a terminal nobody is looking at asks for, as one transition says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Signal {
    /// A program's notification (OSC 9/777), in its own words.
    Notice {
        /// Its title — the terminal's name when the protocol has none.
        title: String,
        /// Its body.
        body: String,
    },
    /// The bell.
    Bell,
    /// A program blocked on the person (OSC 7501): what it calls itself.
    Waiting {
        /// The blocked record's own label, else the terminal's program.
        program: String,
    },
    /// A long command ended, and how: its exit code.
    Finished {
        /// The status the shell reported.
        code: i32,
    },
}

/// One thing to say, of one terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Raised {
    /// Which terminal — the subject of what is said.
    pub terminal: TerminalId,
    /// How it goes by ([`name_of`]).
    pub name: String,
    /// What it asks for.
    pub signal: Signal,
}

/// What has been said of one terminal: the counters a transition is told
/// against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Told {
    /// The last notification's generation.
    notice: u64,
    /// The bells since it was last seen.
    bell: u64,
    /// A program in it waits on the person.
    waiting: bool,
    /// When the last command that ENDED started — a command is said once.
    finished: Option<i64>,
}

impl Told {
    fn of(t: &TerminalState) -> Self {
        Self {
            notice: t.notice.as_ref().map_or(0, |n| n.generation),
            bell: t.attention.bell,
            waiting: t.attention.waiting,
            finished: t.command.filter(|c| !c.running).map(|c| c.since_s),
        }
    }
}

/// What has been said of every terminal, and the rule of what is said next.
#[derive(Debug, Default)]
pub(super) struct Signals {
    /// Whether a reading came: the first is the baseline, and says nothing.
    read: bool,
    told: BTreeMap<TerminalId, Told>,
}

impl Signals {
    /// A new reading of every terminal: what its transitions ask for, of the
    /// ones nobody is looking at — nothing at all for the first reading.
    pub(super) fn heard(&mut self, states: &[TerminalState]) -> Vec<Raised> {
        let baseline = !std::mem::replace(&mut self.read, true);
        let mut out = Vec::new();
        let mut told = BTreeMap::new();
        for t in states {
            let now = Told::of(t);
            if !baseline && !t.seen {
                let before = self.told.get(&t.id).copied().unwrap_or_default();
                out.extend(signals_of(before, now, t).into_iter().map(|signal| Raised {
                    terminal: t.id.clone(),
                    name: name_of(t),
                    signal,
                }));
            }
            told.insert(t.id.clone(), now);
        }
        self.told = told;
        out
    }
}

/// What one terminal's transition from `before` to `now` asks for.
fn signals_of(before: Told, now: Told, t: &TerminalState) -> Vec<Signal> {
    let mut out = Vec::new();
    // A PROGRAM BLOCKED ON THE PERSON is said when it begins; while it lasts,
    // a bell and a notification are that same wait's (an agent that asks for
    // a permission rings, and tells the desktop, about it), and say nothing
    // more.
    if now.waiting && !before.waiting {
        out.push(Signal::Waiting {
            program: t
                .statuses
                .iter()
                .rev()
                .find(|r| r.state == ProgramState::Blocked && !r.title.is_empty())
                .map_or_else(|| t.program.clone(), |r| r.title.clone()),
        });
    }
    if !now.waiting {
        let noticed = now.notice > before.notice;
        if noticed && let Some(n) = &t.notice {
            out.push(Signal::Notice {
                title: if n.title.is_empty() {
                    name_of(t)
                } else {
                    n.title.clone()
                },
                body: n.body.clone(),
            });
        }
        // A BELL WITH A NOTIFICATION is the notification's sound.
        //
        // MOVED, and not GREW: the holder counts the bells since the terminal
        // was last seen and clears them when it is, so a glance and a new
        // bell between two readings read as fewer bells than before — and
        // are one bell nobody heard. Only a count cleared to nothing is
        // silence.
        if now.bell > 0 && now.bell != before.bell && !noticed {
            out.push(Signal::Bell);
        }
    }
    if now.finished.is_some()
        && now.finished != before.finished
        && let Some(c) = t.command
        && c.duration_ms.is_some_and(|ms| ms >= LONG_COMMAND_MS)
        && let Some(code) = c.last_exit
    {
        out.push(Signal::Finished { code });
    }
    out
}

/// What showing a terminal comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Showing {
    /// Its window is listed: focus it (which also takes it out of the
    /// drawer), and ask its holder to put it in front.
    Focus {
        /// The compositor's id of the window.
        window: u64,
    },
    /// It has a viewer whose window the compositor has not listed yet: wait
    /// for that window, by the viewer's pid.
    Wait {
        /// The viewer's process.
        viewer: u32,
    },
    /// No window shows it: open one with its whole group, it in front.
    Open {
        /// The group the window is opened for.
        group: GroupId,
        /// The arguments of `ronoctua-shell` that open it.
        args: Vec<String>,
    },
    /// A window is being opened for its group already.
    Opening,
    /// No such terminal.
    Gone,
}

/// What showing terminal `id` comes to, over the table, the compositor's
/// windows and the group a window is being opened for.
///
/// The window opened for a group in the background takes the group's
/// terminals no window shows, in their order — never one another window
/// shows, which the newest attach would steal from it.
#[must_use]
pub(super) fn showing(
    census: &Census,
    windows: &[ronoctua_compositor::Window],
    id: &TerminalId,
    opening: Option<&GroupId>,
) -> Showing {
    let Some(t) = census.terminals.iter().find(|t| &t.id == id) else {
        return Showing::Gone;
    };
    if let Some(viewer) = t.viewer {
        return windows
            .iter()
            .find(|w| w.pid.and_then(|p| u32::try_from(p).ok()) == Some(viewer))
            .map_or(Showing::Wait { viewer }, |w| Showing::Focus {
                window: w.id,
            });
    }
    if opening == Some(&t.group) {
        return Showing::Opening;
    }
    let mut together: Vec<&TerminalState> = census
        .terminals
        .iter()
        .filter(|o| o.group == t.group && o.viewer.is_none())
        .collect();
    together.sort_by(|a, b| (a.order, a.started_s, &a.id).cmp(&(b.order, b.started_s, &b.id)));
    let ids: Vec<&str> = together.iter().map(|o| o.id.as_str()).collect();
    Showing::Open {
        group: t.group.clone(),
        args: vec![
            "terminal".to_owned(),
            "--show".to_owned(),
            ids.join(","),
            "--front".to_owned(),
            id.to_string(),
        ],
    }
}

/// The window of process `lineage[0]`, or of the nearest of the processes
/// above it (`lineage`, nearest first): a program in a terminal of somebody
/// else's (Ghostty, an editor) has the terminal's window above it. Of the
/// windows of one process — an editor shares one between its windows —
/// the one that had the focus last (`lately`, latest first), else the
/// compositor's first.
#[must_use]
pub(super) fn window_of(
    windows: &[ronoctua_compositor::Window],
    lineage: &[u32],
    lately: &VecDeque<u64>,
) -> Option<u64> {
    lineage.iter().find_map(|pid| {
        let pid = i32::try_from(*pid).ok();
        let mine: Vec<u64> = windows
            .iter()
            .filter(|w| w.pid.is_some() && w.pid == pid)
            .map(|w| w.id)
            .collect();
        lately
            .iter()
            .find(|w| mine.contains(w))
            .copied()
            .or_else(|| mine.first().copied())
    })
}

impl Footer {
    /// A reading of the census came: the table taken, what its transitions
    /// ask for said, and what waited on the old table settled.
    pub(super) fn terminals_heard(&mut self, mut states: Vec<TerminalState>) {
        states.sort_by(|a, b| {
            (&a.group, a.order, a.started_s, &a.id).cmp(&(&b.group, b.order, b.started_s, &b.id))
        });
        let raised = self.terminals.signals.heard(&states);
        let mark = self.terminals.away_mark();
        self.terminals.census = Census { terminals: states };
        self.terminals.settle();
        let now = std::time::Instant::now();
        let mut said = Vec::new();
        let mut noticed = false;
        for r in raised {
            self.terminals.last_asked = Some(r.terminal.clone());
            let subject = r.terminal.to_string();
            let event = |tag| crate::events::Event::of(tag, crate::key::NOTIF_TERMINAL_APP);
            match r.signal {
                Signal::Notice { title, body } => {
                    self.terminal_notice(&r.terminal, title, body, now);
                    noticed = true;
                }
                Signal::Bell => said.push(
                    event(crate::events::TERMINAL_BELL)
                        .about("title", r.name)
                        .on(subject),
                ),
                Signal::Waiting { program } => said.push(
                    event(crate::events::TERMINAL_WAITING)
                        .about("title", r.name)
                        .about("program", program)
                        .on(subject),
                ),
                Signal::Finished { code } => said.push(
                    event(crate::events::TERMINAL_FINISHED)
                        .about("title", r.name)
                        .counting("code", i64::from(code))
                        .on(subject),
                ),
            }
        }
        // AND THE VIEW, when it is up, is told: it lists these.
        let census = self.terminals.census.clone();
        self.hosted_heard::<ronoctua_terminals::registry::Census>(&census);
        self.said_now(said);
        if noticed {
            self.after_the_queue_moved();
        }
        if noticed || mark != self.terminals.away_mark() {
            self.draw_and_paint();
        }
    }

    /// A program's notification, in its own words, as the notification it
    /// asked for — taking the id the terminal's last one took, which leaves
    /// the band and the line and stops counting: one per terminal, replaced
    /// and not stacked, as a `replaces_id` on the bus replaces.
    fn terminal_notice(
        &mut self,
        terminal: &TerminalId,
        title: String,
        body: String,
        now: std::time::Instant,
    ) {
        let subject = terminal.to_string();
        let (id, replacing) = self.terminals.notice_id(terminal, &mut self.locally_raised);
        if replacing {
            self.dismiss_notification(id, now);
            // AND THE ONES THAT LEFT UNREAD: the bell counts what is unread,
            // and the old words of this terminal are replaced, not waiting.
            for key in superseded(self.notifications.history(), id, &subject) {
                self.notifications.mark_read(key);
            }
        }
        let mut n = ronoctua_notify::Notification::fresh_one(
            id,
            self.texts.t(crate::key::NOTIF_TERMINAL_APP),
            None,
            title,
            body,
            ronoctua_notify::Urgency::Normal,
        );
        n.tags = vec![crate::events::TERMINAL_NOTICE.to_owned()];
        n.subject = Some(subject);
        self.arrived(n, now);
    }

    /// `terminal-show [ID]`: the terminal's window in front — out of the
    /// drawer, the terminal in front of its group — or a window opened for
    /// its group when none shows it. With no id, the terminal that asked for
    /// the person last, while it still asks; else the background terminals'
    /// view.
    pub(super) fn terminal_show(&mut self, argument: Option<&str>) {
        let raw = argument.map(str::trim).filter(|a| !a.is_empty());
        let id = match raw {
            Some(raw) => match raw.parse::<TerminalId>() {
                Ok(id) => id,
                Err(_) => {
                    let said = self
                        .texts
                        .t_with(crate::key::ECHO_TERMINAL_NOT_ONE, &[("id", raw)]);
                    self.echo_message(said);
                    return;
                }
            },
            None => match self.terminals.last_asking() {
                Some(id) => id,
                None => {
                    self.run_command(TERMINALS_OPEN);
                    return;
                }
            },
        };
        match showing(
            &self.terminals.census,
            self.windows.all_windows(),
            &id,
            self.terminals.opening.as_ref(),
        ) {
            Showing::Focus { window } => {
                self.terminals.pending_show = None;
                self.compositor_action(|wm| wm.focus_window(window));
                self.terminals.front(&id);
            }
            // ITS FRONT IS ASKED NOW, and its window focused the moment the
            // compositor lists it (`terminals_windows_moved`).
            Showing::Wait { viewer } => {
                self.terminals.front(&id);
                self.terminals.pending_show = Some((id, viewer));
            }
            Showing::Open { group, args } => {
                // A HOLDER THAT DIED leaves its file: the sockets are asked,
                // and the next reading forgets what nobody answers for.
                self.terminals.sweep();
                match std::env::current_exe() {
                    Ok(me) => {
                        tracing::info!(terminal = %id, ?args, "a window for a terminal in the background");
                        self.terminals.opening = Some(group);
                        self.open_foreign(&me.to_string_lossy(), &args, None);
                    }
                    Err(e) => tracing::error!(error = %e, "I do not know my own binary"),
                }
            }
            // THE SECOND SHOW OF A GROUP BEING OPENED — a double `RET` — is
            // not a second window; it lets the next one try again, so a
            // window that never came does not hold the group shut.
            Showing::Opening => {
                tracing::info!(terminal = %id, "a window is being opened for its group already");
                self.terminals.opening = None;
            }
            Showing::Gone => {
                let said = self.texts.t(crate::key::ECHO_TERMINAL_GONE);
                self.echo_message(said);
            }
        }
    }

    /// `window-of-process PID`: the window of that process, or of the
    /// nearest process above it, focused — an agent running in somebody
    /// else's terminal is a process under that terminal's window.
    pub(super) fn window_of_process(&mut self, argument: Option<&str>) {
        let raw = argument.map(str::trim).unwrap_or_default();
        let Some(pid) = raw.parse::<u32>().ok().filter(|p| *p > 1) else {
            let said = self.texts.t(crate::key::ECHO_WINDOW_OF_PROCESS_WANTS_A_PID);
            self.echo_message(said);
            return;
        };
        // THE ANCESTRY IS A FEW SMALL READS OF `/proc` (at most
        // `lineage::MOST_ANCESTORS`), asked once per click.
        let lineage: Vec<u32> = std::iter::once(pid)
            .chain(ronoctua_memory::lineage::ancestry(pid))
            .collect();
        match window_of(self.windows.all_windows(), &lineage, &self.terminals.lately) {
            Some(window) => self.compositor_action(|wm| wm.focus_window(window)),
            None => {
                let said = self
                    .texts
                    .t_with(crate::key::ECHO_NO_WINDOW_OF_PROCESS, &[("pid", raw)]);
                self.echo_message(said);
            }
        }
    }

    /// A program the footer started did not start — maybe the window that
    /// was to show a group of terminals: the group is not held shut waiting
    /// for a window that will not come (`Held::opening`).
    pub(super) fn terminals_window_failed(&mut self) {
        self.terminals.opening = None;
    }

    /// The compositor said something of its windows: the focus remembered,
    /// and a show that waited for its window answered.
    pub(super) fn terminals_windows_moved(&mut self, e: &ronoctua_compositor::Event) {
        use ronoctua_compositor::Event;
        match e {
            Event::FocusChanged(Some(window)) => self.terminals.focused(*window),
            Event::WindowChanged(w) if w.is_focused => self.terminals.focused(w.id),
            Event::WindowClosed(window) => self.terminals.lately.retain(|w| w != window),
            _ => {}
        }
        let Some((id, viewer)) = self.terminals.pending_show.clone() else {
            return;
        };
        if let Showing::Focus { window } = showing(
            &self.terminals.census,
            self.windows.all_windows(),
            &id,
            None,
        ) {
            tracing::info!(terminal = %id, viewer, window, "the terminal's window came: focused");
            self.terminals.pending_show = None;
            self.compositor_action(|wm| wm.focus_window(window));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_terminals::state::{
        Attention, BlockedOn, CommandMark, Ended, Notice, StatusRecord,
    };

    const A: &str = "aaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccc";
    const GROUP: &str = "0000000000000001";
    const OTHER_GROUP: &str = "0000000000000002";

    /// A terminal in the background of `GROUP`, at `order`.
    fn held(id: &str, order: u32) -> TerminalState {
        TerminalState {
            protocol: ronoctua_terminals::PROTOCOL,
            id: id.parse().unwrap(),
            group: GROUP.parse().unwrap(),
            order,
            viewer: None,
            front: false,
            seen: false,
            title: String::new(),
            program: "zsh".to_owned(),
            cwd: None,
            started_s: 1_790_000_000,
            alone_since_s: Some(1_790_000_100),
            attention: Attention::default(),
            notice: None,
            command: None,
            front_asked: 0,
            ended: None,
            statuses: Vec::new(),
            progress: None,
        }
    }

    fn census(terminals: Vec<TerminalState>) -> Census {
        Census { terminals }
    }

    fn window(id: u64, pid: i32) -> ronoctua_compositor::Window {
        ronoctua_compositor::Window {
            id,
            title: None,
            app_id: None,
            pid: Some(pid),
            is_focused: false,
            floating: false,
            fullscreen: false,
            sticky: false,
            drawer: false,
            hidden: false,
            trailmark: false,
            rect: None,
        }
    }

    fn noticing(mut t: TerminalState, generation: u64) -> TerminalState {
        t.notice = Some(Notice {
            title: String::new(),
            body: format!("built {generation}"),
            generation,
        });
        t.attention.notice = generation;
        t
    }

    /// FAILS IF: the mark counts a terminal a window shows, leaves out one
    /// that ended with no window (it stays listed until it is seen once),
    /// says nobody asks while one in the background does — or is drawn with
    /// every terminal in a window.
    #[test]
    fn the_census_counts_terminals_with_no_viewer_and_ended_ones() {
        let mut shown = held(A, 0);
        shown.viewer = Some(4242);
        shown.attention.bell = 3;
        let away = held(B, 1);
        let mut ended = held(C, 2);
        ended.ended = Some(Ended {
            code: 1,
            at_s: 1_790_000_200,
        });
        let all = census(vec![shown.clone(), away.clone(), ended.clone()]);
        assert_eq!(
            away_mark(&all),
            Some((2, false)),
            "the shown one asks nothing here"
        );
        let mut asking = away;
        asking.attention.waiting = true;
        assert_eq!(
            away_mark(&census(vec![shown.clone(), asking, ended])),
            Some((2, true))
        );
        assert_eq!(away_mark(&census(vec![shown])), None);
        assert_eq!(away_mark(&Census::default()), None);
    }

    /// FAILS IF: a footer that starts, or reads the folder again, says what
    /// the terminals had already said before it read them — every start of
    /// the footer would ring every terminal's old bell again.
    #[test]
    fn a_reconnect_raises_nothing() {
        let mut rang = noticing(held(A, 0), 4);
        rang.attention.bell = 2;
        rang.attention.waiting = true;
        rang.command = Some(CommandMark {
            since_s: 1_790_000_000,
            running: false,
            last_exit: Some(0),
            duration_ms: Some(60_000),
        });
        let states = vec![rang, noticing(held(B, 1), 1)];
        let mut fresh = Signals::default();
        assert!(
            fresh.heard(&states).is_empty(),
            "the baseline said something"
        );
        assert!(
            fresh.heard(&states).is_empty(),
            "the same reading again said something"
        );
        // AND A FOOTER STARTED AGAIN is a fresh baseline.
        assert!(Signals::default().heard(&states).is_empty());
    }

    /// FAILS IF: a program's notification in a terminal nobody looks at is
    /// not said, is said twice, or is said while somebody looks — or a new
    /// one is not said again (it replaces the first, by the terminal's one
    /// id: `terminal_notice`).
    #[test]
    fn a_notice_on_an_unseen_terminal_raises_one_and_the_next_raises_one_again() {
        let mut signals = Signals::default();
        assert!(signals.heard(&[held(A, 0)]).is_empty());
        let first = signals.heard(&[noticing(held(A, 0), 1)]);
        assert_eq!(
            first,
            vec![Raised {
                terminal: A.parse().unwrap(),
                name: "zsh".to_owned(),
                signal: Signal::Notice {
                    title: "zsh".to_owned(),
                    body: "built 1".to_owned(),
                },
            }]
        );
        assert!(
            signals.heard(&[noticing(held(A, 0), 1)]).is_empty(),
            "said twice"
        );
        let second = signals.heard(&[noticing(held(A, 0), 2)]);
        assert_eq!(second.len(), 1, "{second:?}");
        // SEEN: taken in, and not said.
        let mut looked_at = noticing(held(A, 0), 3);
        looked_at.seen = true;
        looked_at.viewer = Some(4242);
        assert!(signals.heard(&[looked_at]).is_empty());
        assert!(
            signals.heard(&[noticing(held(A, 0), 3)]).is_empty(),
            "the seen one came back"
        );
    }

    /// FAILS IF: a terminal's second notice takes an id of its own (it would
    /// stack beside the first instead of replacing it), two terminals share
    /// one, an id is one the bus could give — or what the new notice marks
    /// read reaches a record that is not this terminal's, or one read already.
    #[test]
    fn a_terminals_next_notice_replaces_its_last() {
        let mut held_now = Held::default();
        let mut counter = u32::MAX;
        let a: TerminalId = A.parse().unwrap();
        let b: TerminalId = B.parse().unwrap();
        let (first, replacing) = held_now.notice_id(&a, &mut counter);
        assert!(!replacing && first == u32::MAX - 1, "{first}");
        assert_eq!(held_now.notice_id(&a, &mut counter), (first, true));
        let (other, replacing) = held_now.notice_id(&b, &mut counter);
        assert!(!replacing && other != first);
        let record = |id: u32, subject: Option<&str>, read: bool, at: u64| {
            let mut n = ronoctua_notify::Notification::fresh_one(
                id,
                "terminal".to_owned(),
                None,
                "build".to_owned(),
                String::new(),
                ronoctua_notify::Urgency::Normal,
            );
            n.subject = subject.map(ToOwned::to_owned);
            n.at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(at);
            ronoctua_notify::Record {
                notification: n,
                presentation: ronoctua_notify::Presentation::default(),
                read,
                place: ronoctua_notify::Place::Line,
            }
        };
        let history = [
            record(first, Some(A), false, 1),
            record(first, Some(A), true, 2),
            record(first, None, false, 3),
            record(first, Some(B), false, 4),
            record(other, Some(A), false, 5),
        ];
        let keys: Vec<u64> = superseded(&history, first, A)
            .iter()
            .map(|k| {
                k.at.duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            })
            .collect();
        assert_eq!(keys, [1]);
    }

    /// FAILS IF: a bell or a notification during a program's wait on the
    /// person is said beside the wait (one event, not three), a bell with a
    /// notification is a second line, or a long command's end — and only a
    /// long one's, with how it ended — is not said once.
    #[test]
    fn a_wait_folds_its_bell_and_a_long_command_is_said_once() {
        let mut signals = Signals::default();
        assert!(signals.heard(&[held(A, 0)]).is_empty());
        let mut waiting = noticing(held(A, 0), 1);
        waiting.attention.bell = 1;
        waiting.attention.waiting = true;
        waiting.statuses = vec![StatusRecord {
            id: String::new(),
            app: "claude-code".to_owned(),
            state: ProgramState::Blocked,
            kind: Some(BlockedOn::Permission),
            title: "Claude".to_owned(),
            msg: "Apply this edit?".to_owned(),
            percent: None,
            at_ms: 1_790_000_150_000,
        }];
        let said = signals.heard(std::slice::from_ref(&waiting));
        let kinds: Vec<&Signal> = said.iter().map(|r| &r.signal).collect();
        assert_eq!(
            kinds,
            [&Signal::Waiting {
                program: "Claude".to_owned()
            }]
        );
        // STILL WAITING: the next bell is the same wait's.
        waiting.attention.bell = 2;
        assert!(signals.heard(std::slice::from_ref(&waiting)).is_empty());
        // A BELL WITH A NOTIFICATION, out of the wait: one line.
        let mut rung = noticing(held(A, 0), 2);
        rung.attention.bell = 3;
        let said = signals.heard(&[rung]);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(matches!(said[0].signal, Signal::Notice { .. }));
        // A LONG COMMAND, ended: said once, with its code.
        let ended = |since_s: i64, ms: u64| {
            let mut t = noticing(held(A, 0), 2);
            t.attention.bell = 3;
            t.command = Some(CommandMark {
                since_s,
                running: false,
                last_exit: Some(2),
                duration_ms: Some(ms),
            });
            t
        };
        let said = signals.heard(&[ended(10, LONG_COMMAND_MS)]);
        assert_eq!(
            said.iter().map(|r| &r.signal).collect::<Vec<_>>(),
            [&Signal::Finished { code: 2 }]
        );
        assert!(
            signals.heard(&[ended(10, LONG_COMMAND_MS)]).is_empty(),
            "said twice"
        );
        assert!(
            signals.heard(&[ended(20, LONG_COMMAND_MS - 1)]).is_empty(),
            "a short command's end was news"
        );
    }

    /// FAILS IF: a bell is missed when the terminal was glanced at and rang
    /// again between two readings — the holder clears its count when the
    /// terminal is seen, so the new count is SMALLER than the one told — or
    /// the clearing itself, with no bell after it, is said as one.
    #[test]
    fn a_bell_after_a_glance_between_two_readings_is_said() {
        let rung = |bell: u64| {
            let mut t = held(A, 0);
            t.attention.bell = bell;
            t
        };
        let mut signals = Signals::default();
        assert!(signals.heard(&[rung(0)]).is_empty());
        assert_eq!(signals.heard(&[rung(3)]).len(), 1, "three bells unheard");
        // SEEN, cleared and left between readings: nothing to say.
        assert!(signals.heard(&[rung(0)]).is_empty(), "the clearing rang");
        assert_eq!(signals.heard(&[rung(2)]).len(), 1);
        // SEEN, cleared, left AND RUNG AGAIN between readings: 2 → 1.
        let said = signals.heard(&[rung(1)]);
        assert_eq!(
            said.iter().map(|r| &r.signal).collect::<Vec<_>>(),
            [&Signal::Bell],
            "the bell after the glance was missed"
        );
    }

    /// FAILS IF: showing a terminal a window shows does not focus THAT
    /// window — by the viewer's pid — or waits for one the compositor lists;
    /// one with no window opens anything but one window for its whole group
    /// in order (and never a terminal another window shows), a second show
    /// while it opens opens a second, or a terminal nobody holds is not
    /// refused.
    #[test]
    fn terminal_show_focuses_by_pid_or_opens_the_group_once() {
        let mut viewed = held(A, 0);
        viewed.viewer = Some(4242);
        let mut elsewhere = held("dddddddddddddddd", 1);
        elsewhere.viewer = Some(5151);
        let mut last = held(C, 3);
        last.started_s += 1;
        let mut apart = held("eeeeeeeeeeeeeeee", 0);
        apart.group = OTHER_GROUP.parse().unwrap();
        let all = census(vec![last, held(B, 2), viewed, elsewhere, apart]);
        let windows = [window(10, 777), window(11, 4242)];
        let a: TerminalId = A.parse().unwrap();
        let b: TerminalId = B.parse().unwrap();
        assert_eq!(
            showing(&all, &windows, &a, None),
            Showing::Focus { window: 11 }
        );
        assert_eq!(
            showing(&all, &windows[..1], &a, None),
            Showing::Wait { viewer: 4242 }
        );
        let group: GroupId = GROUP.parse().unwrap();
        assert_eq!(
            showing(&all, &windows, &b, None),
            Showing::Open {
                group: group.clone(),
                args: vec![
                    "terminal".to_owned(),
                    "--show".to_owned(),
                    format!("{B},{C}"),
                    "--front".to_owned(),
                    B.to_owned(),
                ],
            }
        );
        assert_eq!(showing(&all, &windows, &b, Some(&group)), Showing::Opening);
        assert_eq!(
            showing(&all, &windows, &"ffffffffffffffff".parse().unwrap(), None),
            Showing::Gone
        );
    }

    /// FAILS IF: a show waiting for its window outlives the viewer it waits
    /// for, or a window being opened for a group keeps the group shut once
    /// a viewer holds it — or a notice's id outlives its terminal.
    #[test]
    fn what_waited_on_the_table_is_settled_by_the_next() {
        let mut held_now = Held {
            pending_show: Some((A.parse().unwrap(), 4242)),
            opening: Some(GROUP.parse().unwrap()),
            notices: BTreeMap::from([(B.parse().unwrap(), 7)]),
            ..Held::default()
        };
        let mut viewed = held(A, 0);
        viewed.viewer = Some(4242);
        held_now.census = census(vec![viewed.clone(), held(B, 1)]);
        held_now.settle();
        assert!(held_now.pending_show.is_some() && held_now.opening.is_some());
        assert_eq!(held_now.notices.len(), 1);
        let mut taken = held(B, 1);
        taken.viewer = Some(6000);
        viewed.viewer = Some(6000);
        held_now.census = census(vec![viewed, taken]);
        held_now.settle();
        assert_eq!(
            held_now.pending_show, None,
            "it waits for a viewer that left"
        );
        assert_eq!(held_now.opening, None, "the group stays shut with a viewer");
        held_now.census = Census::default();
        held_now.settle();
        assert!(held_now.notices.is_empty());
    }

    /// A `/proc` planted under `root`: each `(pid, parent)` a `stat` line,
    /// as the kernel writes one.
    fn planted_proc(root: &std::path::Path, tree: &[(u32, u32)]) {
        let _ = std::fs::remove_dir_all(root);
        for (pid, parent) in tree {
            let at = root.join(pid.to_string());
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(
                at.join("stat"),
                format!(
                    "{pid} (a (b) c) S {parent} 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 {pid}00 0 0\n"
                ),
            )
            .unwrap();
        }
    }

    /// FAILS IF: the window of a process is not found by walking up from it
    /// — an agent's pid under a terminal's shell finds no window — the
    /// nearest window is not the one taken, or of one process's windows the
    /// one last focused is not.
    #[test]
    fn window_of_process_walks_a_planted_ancestry() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-footer-ancestry-{}", std::process::id()));
        // 900 the agent, under 800 a shell, under 700 the terminal, under
        // 600 the session — and 650 an editor of two windows.
        planted_proc(&root, &[(900, 800), (800, 700), (700, 600), (600, 1)]);
        let lineage: Vec<u32> = std::iter::once(900)
            .chain(ronoctua_memory::lineage::ancestry_in(&root, 900))
            .collect();
        assert_eq!(lineage, [900, 800, 700, 600]);
        let windows = [
            window(1, 600),
            window(2, 700),
            window(3, 650),
            window(4, 650),
        ];
        let mut lately = VecDeque::new();
        assert_eq!(
            window_of(&windows, &lineage, &lately),
            Some(2),
            "the nearest"
        );
        assert_eq!(window_of(&windows[2..], &lineage, &lately), None);
        assert_eq!(window_of(&windows, &[650], &lately), Some(3));
        lately.push_front(3);
        lately.push_front(4);
        assert_eq!(
            window_of(&windows, &[650], &lately),
            Some(4),
            "the one last in front"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
