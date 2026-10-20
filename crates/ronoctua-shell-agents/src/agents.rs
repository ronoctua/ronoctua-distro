//! `LEADER m a` — the person's LLM accounts (Claude Code, Codex), in one
//! view with two doors: the footer's region (`m a a`, [`crate::agents_region`])
//! and a window (`m a w`, [`crate::agents_window`]). `m a n` opens a session of
//! an account and `m a l` logs one in, from anywhere. Both doors hold the view
//! [`Open`] and carry out what it asks through ONE function, [`carry_out`].
//!
//! # What is here
//!
//! Everything the screen decides — which rows exist, what a key does to
//! them, what is asked on the door's line, which file a gesture writes —
//! and nothing that knows where an agent keeps its state: that is
//! `ronoctua-llm`'s alone (`one-llm-boundary`). The view asks the crate for
//! the list, each account's plan, limits, usage and tokens, and asks it to
//! change the list; the colours its bars wear are the brain module's
//! (`agent_accounts::lines`, `tone_of`), the same the marks on the modeline
//! wear — proven by a test that hands the module and the view one reading
//! (`a_bar_and_its_mark_wear_one_colour`).
//!
//! # A clock only while it is open
//!
//! A window that resets changes what the view says (its bar is "not known"
//! from then on), and nothing is written when it does. So a door that shows
//! the view reads again at the nearest reset it shows ([`next_reset`]): the
//! window arms ONE one-shot there, the footer asks on the second it already
//! beats ([`Open::ring_if_due`]); closed, there is none.
//!
//! # Per account
//!
//! Its agent and folder, its plan, the five-hour and the weekly window (and
//! the weekly windows by model the usage endpoint gives) as bars with their
//! reset, its tokens by day and by model, its status line, and the rows that
//! act on it: a session, a login, its name, the default of its kind, remove.
//! Then the consent row for the usage endpoint (unofficial, and the screen
//! says so), the rows that add an account — the name asked, the folder
//! made, the status line put in front and the login opened in the terminal —
//! and the two lines a mark changes colour at.
//!
//! # Nothing on the loop that can take long
//!
//! A reading opens the credentials, the kept numbers and the token count,
//! and a Codex account's newest logs: [`read`] blocks, and both doors call it
//! on a thread. A refresh counts the transcripts and may ask the network
//! ([`refresh`]): a thread too. What a gesture writes — the list, a status
//! line, the lines — is a small file, written where the gesture is.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::key;
use ronoctua_brain::compiled::agent_accounts;
use ronoctua_brain::module::Tone;
use ronoctua_i18n::Arg;
use ronoctua_llm::credentials::Facts;
use ronoctua_llm::statusline::{Limits, Wrapping};
use ronoctua_llm::tokens::Tally;
use ronoctua_llm::{Account, Accounts, Kind as Agent, Refusal, Why, Window, usage};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::agents_words as words;
use crate::echo::Echo;
use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::region::Doorway;
use crate::wake::Wake;
use crate::when::now_s;

/// The keys this view teaches, and where each one applies. The hint line is
/// GENERATED from it (`chrome::hint_of_marked`).
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "agents-down", Where::Anywhere),
    ("<up>", "agents-up", Where::Anywhere),
    ("RET", "agents-enter", Where::Anywhere),
    // ON AN ACCOUNT: `C-`, never `M-` — the `M-` family is the compositor's
    // and never reaches a window.
    ("C-n", SESSION_COMMAND, Where::OnAnAccount),
    ("C-l", LOGIN_COMMAND, Where::OnAnAccount),
    ("C-r", "agents-rename", Where::OnAnAccount),
    ("C-d", "agents-default", Where::OnAnAccount),
    ("C-k", "agents-remove", Where::OnAnAccount),
    ("C-u", "agents-refresh", Where::Anywhere),
];

/// The commands the view's ROWS run — every one reachable by name too
/// (`LEADER :`), where it opens the view and does the same.
pub const OF_THE_ROWS: &[&str] = &[
    "agents-add-claude",
    "agents-add-codex",
    "agents-status-line",
    "agents-warn-at",
    "agents-danger-at",
];

/// The command that opens the view in the footer (`LEADER m a a`) — and
/// the one a mark on the modeline runs, with the account's place.
pub const OPEN_COMMAND: &str = "agents-open";
/// A session of an account (`LEADER m a n`, `C-n` on one).
pub const SESSION_COMMAND: &str = "agents-session";
/// A login of an account (`LEADER m a l`, `C-l` on one).
pub const LOGIN_COMMAND: &str = "agents-login";

/// Every command this view's keys and rows reach — for `bindings::reachable`.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .chain(OF_THE_ROWS.iter().copied())
}

/// Whether `id` is one of the view's commands — the ones a door runs
/// through the view ([`View::command`], [`View::answered`]).
#[must_use]
pub fn is_command(id: &str) -> bool {
    reached().any(|c| c == id)
}

/// Which rows a key means anything on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row of an account.
    OnAnAccount,
}

/// A key, once the door has read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Do what the row does.
    Enter,
    /// A session of the account under the cursor.
    Session,
    /// A login of it.
    Login,
    /// Its name.
    Rename,
    /// Make it the default of its kind.
    Default,
    /// Remove it.
    Remove,
    /// Count and ask again, now.
    Refresh,
    /// Let the usage endpoint's host be asked — the consent row's own key.
    Allow,
    /// Back one level, and out.
    Escape,
}

/// A command of [`OF_THE_VIEW`] as the view's press.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "agents-down" => Press::Down,
        "agents-up" => Press::Up,
        "agents-enter" => Press::Enter,
        SESSION_COMMAND => Press::Session,
        LOGIN_COMMAND => Press::Login,
        "agents-rename" => Press::Rename,
        "agents-default" => Press::Default,
        "agents-remove" => Press::Remove,
        "agents-refresh" => Press::Refresh,
        _ => return None,
    })
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` is the way back whatever is held.
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
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _, _)| *taught == notation)
        .map(|(_, id, _)| *id)?;
    press_of(id)
}

// ---------------------------------------------------------------------------
// WHAT THE VIEW READS
// ---------------------------------------------------------------------------

/// One account, as the view draws it.
#[derive(Debug, Clone)]
pub struct Seen {
    /// The account.
    pub account: Account,
    /// Its plan, tier and token date — or why they are not known.
    pub facts: Result<Facts, Why>,
    /// What its status line (Claude Code) or its logs (Codex) last said.
    pub limits: Result<Limits, Why>,
    /// The usage endpoint's last answer, kept (a Claude Code account's).
    pub usage: Option<usage::Kept>,
    /// Its tokens, by day in the machine's zone and by model.
    pub tally: Tally,
    /// Whether our status line is in front of its own (a Claude Code
    /// account's).
    pub wrapping: Option<Result<Wrapping, Why>>,
}

/// Everything both doors draw from — read the same way by both.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// Whether anything was read yet: the view opens before its first
    /// reading arrives, and says so.
    pub read: bool,
    /// The list is there and does not read: nothing is written over it.
    pub refused: bool,
    /// The accounts, in the person's order.
    pub accounts: Vec<Seen>,
    /// The lines the bars change colour at — the brain module's.
    pub lines: agent_accounts::Lines,
    /// Whether the person let the usage endpoint's host be asked.
    pub let_usage: bool,
    /// The home, to show a folder under it as `~/…`.
    pub home: Option<PathBuf>,
    /// Now, seconds since the epoch.
    pub now_s: i64,
}

impl Readings {
    /// One account, by id.
    #[must_use]
    pub fn seen(&self, id: &str) -> Option<&Seen> {
        self.accounts.iter().find(|s| s.account.id == id)
    }

    /// The account at a place of the list, from 1 — the digit of its mark.
    #[must_use]
    pub fn at_place(&self, place: usize) -> Option<&Seen> {
        place.checked_sub(1).and_then(|n| self.accounts.get(n))
    }

    /// The account a gesture with no row means ([`chosen_of`]).
    #[must_use]
    pub fn chosen(&self) -> Option<&Seen> {
        let accounts: Vec<Account> = self.accounts.iter().map(|s| s.account.clone()).collect();
        let id = chosen_of(&accounts, None)?.id;
        self.seen(&id)
    }

    /// Whether any account is Claude Code's — the usage endpoint is theirs.
    fn has_claude(&self) -> bool {
        self.accounts
            .iter()
            .any(|s| s.account.kind == Agent::Claude)
    }
}

/// The account a gesture means: the one at `place` (from 1, a mark's
/// digit), or with none the default Claude Code account, else the default
/// Codex one, else the first.
#[must_use]
pub fn chosen_of(accounts: &[Account], place: Option<usize>) -> Option<Account> {
    if let Some(place) = place {
        return place.checked_sub(1).and_then(|n| accounts.get(n)).cloned();
    }
    Agent::ALL
        .into_iter()
        .find_map(|k| accounts.iter().find(|a| a.kind == k && a.default))
        .or_else(|| accounts.first())
        .cloned()
}

/// How a door reads the accounts.
pub enum Reader {
    /// On a thread: what it reads — and what the watches hear — is left for
    /// the door, and its loop woken ([`Open::heard`]).
    Thread(Wake),
    /// Here, at once, by this function — a photograph's, which has no loop
    /// turning ([`read`]), or a test's, which reads what it planted.
    Here(fn(&ronoctua_files::when::Zone) -> Readings),
}

/// Where a session, a login and a refresh go.
#[derive(Debug)]
pub enum Reach {
    /// The machine: the terminal, the network.
    Machine,
    /// Nowhere: what would have happened is written down — a photograph's.
    Nowhere(Vec<String>),
}

/// The view OPEN, in either door — the footer's region
/// ([`crate::agents_region`]) or the window ([`crate::agents_window`]): what
/// it reads, the watches that keep that current, and the threads that read
/// and refresh. Both doors hand it every key, click, answer and thread's
/// answer; what the view asks for after them is carried out by
/// [`carry_out`], the same function in both.
pub struct Open {
    /// The view.
    pub view: View,
    /// What it draws.
    pub readings: Readings,
    /// The zone a date is said in.
    pub zone: ronoctua_files::when::Zone,
    /// How it reads.
    reader: Reader,
    /// Where sessions and refreshes go.
    pub reach: Reach,
    /// What the threads and the watches left, for the door's loop.
    inbox: Arc<Mutex<Vec<Heard>>>,
    /// What the watches cover.
    watching: Watching,
    /// The watches.
    watches: Vec<ronoctua_files::notice::Notice>,
    /// A reading in flight.
    reading: bool,
    /// One more reading wanted after the one in flight.
    again: bool,
    /// The account to aim at once the first reading is here.
    place: Option<usize>,
    /// The reset a reading was last asked for ([`Open::ring_if_due`]).
    rung: Option<i64>,
}

impl Open {
    /// The view, read at once — or a reading and a refresh asked of their
    /// threads: the numbers shown are fresh when the person looks. `place`
    /// (from 1) aims at an account once a reading is here; `columns` is the
    /// width the cursor is first settled at.
    #[must_use]
    pub fn new(
        zone: ronoctua_files::when::Zone,
        reader: Reader,
        reach: Reach,
        place: Option<usize>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Self {
        let mut me = Self {
            view: View::default(),
            readings: Readings::default(),
            zone,
            reader,
            reach,
            inbox: Arc::new(Mutex::new(Vec::new())),
            watching: Watching::default(),
            watches: Vec::new(),
            reading: false,
            again: false,
            place,
            rung: None,
        };
        match me.reader {
            Reader::Here(read) => {
                let r = read(&me.zone);
                me.arrived(r, columns, texts);
            }
            Reader::Thread(_) => {
                me.read_again();
                me.refresh();
            }
        }
        me
    }

    /// The rows, at the door's width.
    #[must_use]
    pub fn rows(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        self.view.rows(&self.readings, &self.zone, columns, texts)
    }

    /// Asks for a reading — at once, or of a thread, one at a time: a change
    /// heard while one is in flight is one more after it.
    pub fn read_again(&mut self) {
        match &self.reader {
            Reader::Here(read) => {
                self.readings = read(&self.zone);
            }
            Reader::Thread(wake) => {
                if self.reading {
                    self.again = true;
                    return;
                }
                self.reading = true;
                let (inbox, wake) = (Arc::clone(&self.inbox), wake.clone());
                read_off_the_loop(self.zone.clone(), move |h| leave(&inbox, &wake, h));
            }
        }
    }

    /// A refresh: counted and asked off the loop — or written down. A
    /// photograph reaches no network: reading on the spot, it asks nothing.
    fn refresh(&mut self) {
        match (&mut self.reach, &self.reader) {
            (Reach::Machine, Reader::Thread(wake)) => {
                let (inbox, wake) = (Arc::clone(&self.inbox), wake.clone());
                refresh_off_the_loop(move |h| leave(&inbox, &wake, h));
            }
            (Reach::Nowhere(done), _) => done.push("refresh".to_owned()),
            (Reach::Machine, Reader::Here(_)) => {}
        }
    }

    /// A reading arrived: kept, the cursor settled on what it was on, and
    /// the watches raised again over what the reading covers now.
    fn arrived(&mut self, r: Readings, columns: u16, texts: &ronoctua_i18n::Texts) {
        self.readings = r;
        if let Some(place) = self.place.take() {
            self.view.aim_at_place(place, &self.readings);
        }
        self.view
            .settle_in(&self.readings, &self.zone, columns, texts);
        if let Reader::Thread(wake) = &self.reader {
            let now = watched_now(&self.readings);
            if now != self.watching {
                let (inbox, wake) = (Arc::clone(&self.inbox), wake.clone());
                self.watches = now.raise(move || {
                    leave(&inbox, &wake, Heard::Stirred);
                    true
                });
                self.watching = now;
            }
        }
    }

    /// What the threads and the watches left, taken in: a reading kept, a
    /// refresh said on the door's line and read again after, a stir read.
    pub fn heard(&mut self, way: &mut dyn Doorway) {
        let came = self
            .inbox
            .lock()
            .map(|mut i| std::mem::take(&mut *i))
            .unwrap_or_default();
        for heard in came {
            match heard {
                Heard::Stirred => self.read_again(),
                Heard::Read(r) => {
                    self.reading = false;
                    self.arrived(*r, way.columns(), way.texts());
                    if std::mem::take(&mut self.again) {
                        self.read_again();
                    }
                    way.repaint();
                }
                Heard::Refreshed(done) => {
                    let said = refreshed_said(&done, way.texts());
                    way.say(said, Echo::Normal);
                    self.read_again();
                }
            }
        }
    }

    /// What was read, again, here — after a gesture that wrote, so what it
    /// wrote is drawn at once (on the spot), or as soon as a thread read it.
    fn reread_after_a_write(&mut self, columns: u16, texts: &ronoctua_i18n::Texts) {
        match self.reader {
            Reader::Here(read) => {
                let r = read(&self.zone);
                self.arrived(r, columns, texts);
            }
            Reader::Thread(_) => self.read_again(),
        }
    }

    /// The nearest reset the view shows ([`next_reset`]) — what a door's one
    /// clock is armed at.
    #[must_use]
    pub fn next_reset(&self) -> Option<i64> {
        next_reset(&self.readings)
    }

    /// Reads again once the nearest reset shown is past `now_s` — once per
    /// reset: the footer asks on the second it already beats, and the window
    /// that reset is "not known" from then on. Whether a reading was asked.
    pub fn ring_if_due(&mut self, now_s: i64) -> bool {
        match self.next_reset() {
            Some(at) if now_s > at && self.rung != Some(at) => {
                self.rung = Some(at);
                self.read_again();
                true
            }
            _ => false,
        }
    }

    /// Aims at the account at `place` (from 1) — a mark clicked, `agents-open
    /// <n>` typed with the view open.
    pub fn aim_at(&mut self, place: usize, columns: u16, texts: &ronoctua_i18n::Texts) {
        self.view.aim_at_place(place, &self.readings);
        self.view
            .settle_in(&self.readings, &self.zone, columns, texts);
    }

    /// A new width or another language: new rows, so the cursor is found
    /// again by what it is on.
    pub fn settle(&mut self, columns: u16, texts: &ronoctua_i18n::Texts) {
        self.view
            .settle_in(&self.readings, &self.zone, columns, texts);
    }

    /// A key of the accounts, from the keyboard, the hint or a second click.
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

    /// A command of the view by its name — `LEADER :`, a row's, a hint's —
    /// or, with `argument`, the answer to the view's own question.
    pub fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) {
        // AN ANSWER ONLY TO THE VIEW'S OWN QUESTION.
        let answering = self.view.asking().is_some_and(|a| a.command() == id);
        let errand = match argument.filter(|_| answering) {
            Some(said) => self.view.answered(id, said, way.texts()),
            None => self
                .view
                .command(id, &self.readings, &self.zone, way.columns(), way.texts()),
        };
        carry_out(errand, self, way);
    }

    /// What was typed on the door's line, under the id that asked — the
    /// same `View::answered` in both doors.
    pub fn answered(&mut self, id: &str, text: &str, way: &mut dyn Doorway) {
        let errand = self.view.answered(id, text, way.texts());
        carry_out(errand, self, way);
    }

    /// What would have happened — a photograph's.
    #[must_use]
    pub fn done(&self) -> &[String] {
        match &self.reach {
            Reach::Machine => &[],
            Reach::Nowhere(done) => done,
        }
    }
}

/// Leaves what a thread or a watch brought, and wakes the door.
fn leave(inbox: &Arc<Mutex<Vec<Heard>>>, wake: &Wake, heard: Heard) {
    if let Ok(mut i) = inbox.lock() {
        i.push(heard);
    }
    wake.call();
}

/// A terminal with these words — the door gives the keyboard up for it
/// first (the footer's region closes: the launcher's rule) — or, a
/// photograph's, written down.
fn terminal(open: &mut Open, words: Vec<String>, way: &mut dyn Doorway) {
    match &mut open.reach {
        Reach::Machine => {
            way.step_aside();
            way.open_window(words);
        }
        Reach::Nowhere(done) => done.push(words.join(" ")),
    }
}

/// CARRIES OUT what the accounts' view asked for, on the line of the door it
/// was asked in — the footer's region and the window alike
/// (`errands-are-handled-alike` reads that both hand it the errand whole) —
/// and reads again only after a write: a cursor walking writes nothing, and a
/// reading opens every account's files.
pub fn carry_out(errand: Errand, open: &mut Open, way: &mut dyn Doorway) {
    let wrote = errand.writes();
    match errand {
        Errand::Nothing | Errand::Wrote => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Ask(line) => way.ask(line),
        Errand::Say(said) => way.say(said, Echo::Long),
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, Echo::Normal);
        }
        Errand::Edit(edit) => {
            let applied = apply(&edit, way.texts());
            if let Some(said) = applied.said {
                way.say(said, Echo::Long);
            }
            if let Some(words) = applied.terminal {
                terminal(open, words, way);
            }
        }
        Errand::Open(id, login) => {
            let opened = open
                .readings
                .seen(&id)
                .map(|s| terminal_of(&s.account, login));
            match opened {
                Some(Ok(words)) => terminal(open, words, way),
                Some(Err(r)) => {
                    let said = words::refused(r.why, way.texts());
                    way.say(said, Echo::Long);
                }
                None => {
                    let said = way.texts().t(key::AGENTS_NONE);
                    way.say(said, Echo::Normal);
                }
            }
        }
        Errand::Refresh => {
            let said = way.texts().t(key::AGENTS_REFRESHING);
            way.say(said, Echo::Normal);
            open.refresh();
        }
    }
    // WHAT THIS DOOR WROTE, read back at once — the watches ring for it too,
    // and find nothing new.
    if wrote {
        open.reread_after_a_write(way.columns(), way.texts());
    }
    way.repaint();
}

/// `agents-session [n]` / `agents-login [n]` from anywhere — the view closed,
/// or a place given: the account at that place, or the default one, in the
/// terminal (the view, when open, steps aside for it); or the sentence that
/// says why not.
pub fn session_from_anywhere(login: bool, place: Option<usize>, way: &mut dyn Doorway) {
    let list = Accounts::store().read();
    match from_anywhere(&list.accounts, place, login, &terminal_of, way.texts()) {
        Ok(words) => {
            way.step_aside();
            way.open_window(words);
        }
        Err(said) => way.say(said, Echo::Long),
    }
}

/// A place given to a command (`agents-open 3`, a mark's digit).
#[must_use]
pub fn place_of(argument: Option<&str>) -> Option<usize> {
    argument.and_then(|a| a.trim().parse::<usize>().ok())
}

/// The window's words: `agents`, and the place when one is given.
#[must_use]
pub fn window_words(argument: Option<&str>) -> Vec<String> {
    let mut words = vec!["agents".to_owned()];
    words.extend(
        argument
            .map(str::trim)
            .filter(|a| a.parse::<usize>().is_ok())
            .map(ToOwned::to_owned),
    );
    words
}

/// Reads everything, as it is now. BLOCKS on files — a credentials file, the
/// kept numbers, a token count, a Codex account's newest logs: a door calls
/// it on a thread.
#[must_use]
pub fn read(zone: &ronoctua_files::when::Zone) -> Readings {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|h| h.is_absolute());
    let mut r = Readings {
        read: true,
        lines: agent_accounts::lines(),
        let_usage: ronoctua_consent::allows(usage::HOST),
        home,
        now_s: now_s(),
        ..Readings::default()
    };
    let Some(list) = Accounts::store().parsed() else {
        r.refused = true;
        return r;
    };
    r.accounts = list
        .accounts
        .into_iter()
        .map(|account| {
            let claude = account.kind == Agent::Claude;
            let cache = ronoctua_llm::tokens::kept(&account);
            Seen {
                facts: ronoctua_llm::credentials::facts(&account.dir).map_err(|r| r.why),
                limits: ronoctua_llm::statusline::limits(&account).map_err(|r| r.why),
                usage: claude.then(|| usage::kept(&account)).flatten(),
                tally: cache.tally_in(&|at| zone.offset_at(at)),
                wrapping: claude
                    .then(|| ronoctua_llm::statusline::wrapped(&account).map_err(|r| r.why)),
                account,
            }
        })
        .collect();
    r
}

/// What the doors watch while the view is open, to read again when it
/// moves: the list, the consents and the lines, and per account what the
/// crate says its numbers and logs are (`ronoctua_llm::watched`) — never a
/// path written here.
#[must_use]
pub fn watched_now(r: &Readings) -> Watching {
    let files: Vec<PathBuf> = [
        Accounts::store().path(),
        ronoctua_consent::store().path(),
        agent_accounts::lines_file(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut w = Watching {
        // A FILE WHOSE FOLDER IS NOT THERE YET is watched from the nearest
        // folder that is: its making is heard, the reading after it covers
        // a different `Watching`, and the doors raise their watches again —
        // over the file itself this time.
        ancestors: files.iter().filter_map(|f| waiting_folder(f)).collect(),
        files,
        ..Watching::default()
    };
    for s in &r.accounts {
        let watched = ronoctua_llm::watched(&s.account);
        w.dirs.extend(
            watched
                .files
                .iter()
                .filter_map(|f| f.parent())
                .filter(|d| d.is_dir())
                .map(Path::to_path_buf),
        );
        w.trees
            .extend(watched.trees.into_iter().filter(|t| t.is_dir()));
        if s.account.dir.is_dir() {
            w.dirs.push(s.account.dir.clone());
        }
    }
    for list in [&mut w.files, &mut w.dirs, &mut w.trees, &mut w.ancestors] {
        list.sort();
        list.dedup();
    }
    w
}

/// For a file whose folder is not there: the nearest folder above it that
/// is, to hear its folder being made. `None` when its folder is there.
fn waiting_folder(file: &Path) -> Option<PathBuf> {
    let folder = file.parent()?;
    if folder.is_dir() {
        return None;
    }
    folder
        .ancestors()
        .skip(1)
        .find(|a| a.is_dir())
        .map(Path::to_path_buf)
}

/// What a door watches.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watching {
    /// Files, each from its folder, by name.
    pub files: Vec<PathBuf>,
    /// Folders, alone.
    pub dirs: Vec<PathBuf>,
    /// Folders, whole.
    pub trees: Vec<PathBuf>,
    /// For each file whose folder is not there yet, the nearest folder that
    /// is — alone, until the file's own folder is made.
    pub ancestors: Vec<PathBuf>,
}

impl Watching {
    /// The notices, each ringing `ring`. A file whose folder is not there
    /// yet is not watched — its nearest folder is ([`Watching::ancestors`]),
    /// until a reading finds the file's folder made.
    #[must_use]
    pub fn raise<F>(&self, ring: F) -> Vec<ronoctua_files::notice::Notice>
    where
        F: Fn() -> bool + Send + Clone + 'static,
    {
        use ronoctua_files::notice::{Notice, Pace};
        let mut out = Vec::new();
        for file in &self.files {
            let (Some(dir), Some(name)) =
                (file.parent(), file.file_name().and_then(|n| n.to_str()))
            else {
                continue;
            };
            if !dir.is_dir() {
                continue;
            }
            match Notice::on_file(dir, name, Pace::Coalesced, ring.clone()) {
                Ok(n) => out.push(n),
                Err(e) => tracing::warn!(error = %e, file = %file.display(), "not watched"),
            }
        }
        let dirs: Vec<PathBuf> = self.dirs.iter().chain(&self.ancestors).cloned().collect();
        if !dirs.is_empty() || !self.trees.is_empty() {
            match Notice::on_dirs(&dirs, &self.trees, Pace::Coalesced, ring) {
                Ok(n) => out.push(n),
                Err(e) => tracing::warn!(error = %e, "the LLM accounts are not watched"),
            }
        }
        out
    }
}

/// What a door hears from its threads and its watches, on its own loop.
#[derive(Debug)]
pub enum Heard {
    /// Something it watches moved: read again.
    Stirred,
    /// A reading, done.
    Read(Box<Readings>),
    /// A refresh, done.
    Refreshed(Result<Vec<ronoctua_llm::refresh::Refreshed>, Refusal>),
}

/// Reads on a thread, and sends the reading.
pub fn read_off_the_loop(
    zone: ronoctua_files::when::Zone,
    to: impl FnOnce(Heard) + Send + 'static,
) {
    let spawned = std::thread::Builder::new()
        .name("agents-read".to_owned())
        .spawn(move || to(Heard::Read(Box::new(read(&zone)))));
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "the LLM accounts could not be read");
    }
}

/// Refreshes on a thread, and sends what came of it.
pub fn refresh_off_the_loop(to: impl FnOnce(Heard) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("agents-refresh".to_owned())
        .spawn(move || to(Heard::Refreshed(refresh())));
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "the LLM accounts could not be refreshed");
    }
}

/// Counts every account's transcripts and asks the usage endpoint for the
/// Claude Code ones the person let — what the usage unit does, from a door.
/// BLOCKS, on files and on the network: a thread's.
///
/// # Errors
///
/// The crate's: the list does not read.
pub fn refresh() -> Result<Vec<ronoctua_llm::refresh::Refreshed>, Refusal> {
    ronoctua_llm::refresh::run(
        &ronoctua_llm::Roots::of_env(),
        None,
        &|| ronoctua_consent::pass(usage::HOST).ok(),
        &mut |pass, dir| usage::fetch(pass, dir),
    )
}

/// What a refresh came back with, as the door's line says it: done, or the
/// first reason an account was not counted or asked.
#[must_use]
pub fn refreshed_said(
    done: &Result<Vec<ronoctua_llm::refresh::Refreshed>, Refusal>,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let all = match done {
        Ok(all) => all,
        Err(r) => return words::refused(r.why, texts),
    };
    let first = all.iter().find_map(|one| {
        one.counted
            .err()
            .or_else(|| one.asked.and_then(Result::err))
            .map(|r| (one.id.as_str(), r.why))
    });
    match first {
        Some((id, why)) => texts.t_with(
            key::AGENTS_REFRESHED_BUT,
            &[("id", id), ("why", &words::refused(why, texts))],
        ),
        None => texts.t(key::AGENTS_REFRESHED),
    }
}

// ---------------------------------------------------------------------------
// ROWS
// ---------------------------------------------------------------------------

/// A window of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// The five hours.
    FiveHours,
    /// The week.
    Week,
    /// The week, of Opus.
    WeekOpus,
    /// The week, of Sonnet.
    WeekSonnet,
}

impl Span {
    /// Its label.
    const fn label(self) -> &'static str {
        match self {
            Self::FiveHours => key::AGENTS_ROW_FIVE_HOURS,
            Self::Week => key::AGENTS_ROW_WEEK,
            Self::WeekOpus => key::AGENTS_ROW_WEEK_OPUS,
            Self::WeekSonnet => key::AGENTS_ROW_WEEK_SONNET,
        }
    }
}

/// A fact of an account a row shows; `RET` copies its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact {
    /// Which agent.
    Agent,
    /// Its folder.
    Folder,
    /// Its plan.
    Plan,
    /// A window.
    Window(Span),
    /// Its tokens on a day (`YYYY-MM-DD`).
    Day(String),
    /// Its tokens of a model.
    Model(String),
    /// No token counted yet.
    NoTokens,
}

/// What the keys do on a row — and, since it carries what the row is about,
/// its identity across a rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A fact of an account: `RET` copies it.
    Fact(String, Fact),
    /// Its status line: `RET` puts ours in front, or gives its own back.
    StatusLine(String),
    /// A session of it, in the terminal.
    Session(String),
    /// Its login, in the terminal.
    Login(String),
    /// Its name: `RET` asks it.
    Rename(String),
    /// The default of its kind: `RET` makes it so.
    Default(String),
    /// Remove it: `RET` asks first, "no" drawn first.
    Remove(String),
    /// The usage endpoint's host, not let yet: `RET` lets it be asked.
    Let,
    /// The usage endpoint, let: a fact.
    Endpoint,
    /// Add an account of an agent: `RET` asks its name.
    Add(Agent),
    /// Count and ask again, now.
    Refresh,
    /// The warning's line: `RET` asks it.
    Warn,
    /// The danger's line: `RET` asks it.
    Danger,
    /// An answer to the question: yes, or no.
    Answer(bool),
}

impl Target {
    /// The account this row is about, when it is about one.
    #[must_use]
    pub fn account(&self) -> Option<&str> {
        match self {
            Self::Fact(id, _)
            | Self::StatusLine(id)
            | Self::Session(id)
            | Self::Login(id)
            | Self::Rename(id)
            | Self::Default(id)
            | Self::Remove(id) => Some(id),
            _ => None,
        }
    }

    /// The command this row's `RET` is, when it is one — what the pointer
    /// resting on it reads (`cmd-<id>`).
    const fn command(&self) -> Option<&'static str> {
        Some(match self {
            Self::StatusLine(_) => "agents-status-line",
            Self::Session(_) => SESSION_COMMAND,
            Self::Login(_) => LOGIN_COMMAND,
            Self::Rename(_) => "agents-rename",
            Self::Default(_) => "agents-default",
            Self::Remove(_) => "agents-remove",
            Self::Add(Agent::Claude) => "agents-add-claude",
            Self::Add(Agent::Codex) => "agents-add-codex",
            Self::Refresh => "agents-refresh",
            Self::Warn => "agents-warn-at",
            Self::Danger => "agents-danger-at",
            _ => return None,
        })
    }

    /// What the pointer resting on the row reads.
    fn about(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        if let Some(id) = self.command() {
            return Some(texts.t(&format!("cmd-{id}")));
        }
        match self {
            Self::Let => Some(texts.t(key::ABOUT_CONSENT_ALLOW)),
            Self::Fact(_, Fact::Window(_)) => Some(texts.t(key::ABOUT_AGENTS_WINDOW)),
            Self::Fact(..) | Self::Endpoint => Some(texts.t(key::ABOUT_AGENTS_FACT)),
            _ => None,
        }
    }
}

/// One drawn line.
#[derive(Debug, Clone)]
pub struct Row {
    /// The pieces, already placed and cut.
    pub fields: Vec<Field>,
    /// What a key does here — `None` on a title, a sentence, the rest of a
    /// wrapped label: the cursor skips it.
    pub target: Option<Target>,
}

impl Row {
    /// Whether the cursor skips it.
    #[must_use]
    pub const fn heading(&self) -> bool {
        self.target.is_none()
    }

    /// Whether a click takes it — a row the cursor stands on — and so
    /// whether a resting pointer lights it: ONE ANSWER for both doors' light
    /// and click (the region's `Row::takes`, the window's `Content::takes`,
    /// [`View::clicked`]).
    #[must_use]
    pub const fn takes(&self) -> bool {
        !self.heading()
    }
}

/// One entry of a section, before the section is laid out whole.
enum Entry {
    /// A `label  value` row.
    Pair {
        label: String,
        value: String,
        kind: crate::chrome::Kind,
        target: Target,
        tint: Option<tokens::Color>,
    },
    /// A sentence under the row before it, wrapped.
    Note(String),
    /// A line of air between two parts of one section.
    Air,
}

/// A pair entry.
fn pair(label: String, value: String, target: Target, acts: bool) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::of(acts),
        target,
        tint: None,
    }
}

/// A pair whose value says a state in its colour — a window at a line.
fn tinted(label: String, value: String, target: Target, tint: Option<tokens::Color>) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::Fact,
        target,
        tint,
    }
}

/// A dim sentence.
const fn note(text: String) -> Entry {
    Entry::Note(text)
}

/// The rows, as they are built.
struct Builder<'a> {
    out: Vec<Row>,
    cursor: usize,
    left: u16,
    room: u16,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    /// A section: its title, and its entries laid out whole — the values of
    /// every row in one column after the widest label (`section_column`),
    /// a label that does not fit wrapped under itself.
    fn section(&mut self, title: &str, entries: Vec<Entry>) {
        let end = self.left + self.room;
        // AIR BEFORE EVERY TITLE but the first — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                target: None,
            });
        }
        self.out.push(Row {
            fields: vec![piece(
                self.left,
                title.to_owned(),
                tokens::color::TEXT_HEADING,
                self.room,
            )],
            target: None,
        });
        let pairs: Vec<(u16, u16, u16)> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::Pair { label, value, .. } => {
                    Some((self.left, layout::width(label), layout::width(value)))
                }
                _ => None,
            })
            .collect();
        let column = crate::chrome::section_column(self.left, self.room, &pairs);
        for entry in entries {
            match entry {
                Entry::Pair {
                    label,
                    value,
                    kind,
                    target,
                    tint,
                } => {
                    let focused = self.cursor == self.out.len();
                    let lines = crate::chrome::aligned_pair(
                        self.left, column, end, &label, &value, kind, focused,
                    );
                    for (n, fields) in lines.into_iter().enumerate() {
                        let fields = if n == 0 {
                            crate::chrome::tinted(fields, tint, focused)
                        } else {
                            fields
                        };
                        self.out.push(Row {
                            fields,
                            target: (n == 0).then(|| target.clone()),
                        });
                    }
                }
                Entry::Note(text) => {
                    for line in crate::chrome::note_lines(&text, self.room) {
                        self.out.push(Row {
                            fields: vec![piece(
                                self.left,
                                line,
                                tokens::color::TEXT_DIM,
                                self.room,
                            )],
                            target: None,
                        });
                    }
                }
                Entry::Air => self.out.push(Row {
                    fields: crate::chrome::AIR,
                    target: None,
                }),
            }
        }
    }

    fn t(&self, k: &str) -> String {
        self.texts.t(k)
    }
}

/// One piece of text, elided to `room`.
fn piece(at: u16, text: String, color: tokens::Color, room: u16) -> Field {
    Field {
        at,
        text: layout::elide_end(&text, room),
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

/// A folder as a person reads it: under their home, `~/…`.
fn shown_dir(dir: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| dir.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    }
}

/// The two readings of one window — the status line's (or the logs') and
/// the endpoint's — as one: the fuller, since a window only fills until it
/// resets. A window whose reset has passed is not known.
fn merged(a: Option<Window>, b: Option<Window>, now_s: i64) -> Option<Window> {
    [a, b]
        .into_iter()
        .flatten()
        .filter(|w| w.resets_at_s > now_s && w.used_percent.is_finite())
        .reduce(|x, y| {
            if y.used_percent > x.used_percent {
                y
            } else {
                x
            }
        })
}

/// The windows of an account, by span, as known now.
#[must_use]
pub fn windows_of(s: &Seen, now_s: i64) -> Vec<(Span, Option<Window>)> {
    let limits = s.limits.as_ref().ok().copied().unwrap_or_default();
    let usage = s.usage.and_then(|k| k.usage).unwrap_or_default();
    let mut out = vec![
        (
            Span::FiveHours,
            merged(limits.five_hour, usage.five_hour, now_s),
        ),
        (Span::Week, merged(limits.seven_day, usage.seven_day, now_s)),
    ];
    // THE WEEKS BY MODEL exist only where the endpoint said them.
    for (span, w) in [
        (Span::WeekOpus, usage.seven_day_opus),
        (Span::WeekSonnet, usage.seven_day_sonnet),
    ] {
        if let Some(w) = merged(w, None, now_s) {
            out.push((span, Some(w)));
        }
    }
    out
}

/// The nearest reset of a window the view shows, after the reading's now —
/// when the one clock a door arms while the view is open rings: the bar
/// that reset is "not known" from then on, and nothing writes a file to say
/// so. `None` when no window is shown: no clock.
#[must_use]
pub fn next_reset(r: &Readings) -> Option<i64> {
    r.accounts
        .iter()
        .flat_map(|s| windows_of(s, r.now_s))
        .filter_map(|(_, w)| w.map(|w| w.resets_at_s))
        .filter(|at| *at > r.now_s)
        .min()
}

/// How long from now until `at` (seconds since the epoch) and one second
/// more — so the reading the clock asks for is past it — as a door's timer
/// wants it.
#[must_use]
pub fn until(at: i64) -> std::time::Duration {
    std::time::Duration::from_secs(u64::try_from(at.saturating_sub(now_s())).unwrap_or(0) + 1)
}

/// A window's value: a bar, how full, and when it resets — or "not known".
fn window_value(
    w: Option<Window>,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let Some(w) = w else {
        return texts.t(key::AGENTS_WINDOW_UNKNOWN);
    };
    let filled = u8::try_from(words::whole(w.used_percent).clamp(0, 100)).unwrap_or(100);
    texts.t_with(
        key::AGENTS_WINDOW,
        &[
            (
                "bar",
                &crate::chrome::meter(filled, crate::sizes::BAR_CELLS),
            ),
            ("percent", &words::percent_text(w.used_percent, texts)),
            (
                "when",
                &crate::chrome::when_text(w.resets_at_s, zone, false, texts),
            ),
        ],
    )
}

/// The colour a window's value wears: the brain module's, by the lines —
/// `None` when it is quiet, and the value says it in its own tone.
#[must_use]
pub fn window_tint(w: Option<Window>, lines: agent_accounts::Lines) -> Option<tokens::Color> {
    match agent_accounts::tone_of(w.map(|w| w.used_percent), lines) {
        Tone::Quiet => None,
        tone => Some(crate::brainlink::tone_colour(tone)),
    }
}

/// A day of the count (`YYYY-MM-DD`, already the zone's) as the person reads
/// dates.
fn day_text(day: &str, texts: &ronoctua_i18n::Texts) -> String {
    let mut parts = day.splitn(3, '-').map(str::parse::<i64>);
    let civil = match (parts.next(), parts.next(), parts.next()) {
        (Some(Ok(y)), Some(Ok(m)), Some(Ok(d))) => u32::try_from(m)
            .ok()
            .zip(u32::try_from(d).ok())
            .and_then(|(m, d)| ronoctua_files::when::epoch_of(y, m, d))
            // DAYS since the epoch: noon of the day, as an instant.
            .map(|days| days * 86_400 + 43_200),
        _ => None,
    };
    civil.map_or_else(
        || day.to_owned(),
        |secs| {
            crate::chrome::date_text(
                secs,
                &ronoctua_files::when::Zone::utc(),
                crate::when::prefs::current().order,
                texts,
            )
        },
    )
}

/// How many days of the count are drawn — a week, the newest first.
const DAYS_SHOWN: usize = 7;

/// How many models are drawn — the most used first.
const MODELS_SHOWN: usize = 6;

/// A count beside a bar relative to the largest of its list.
fn counted(n: u64, most: u64, texts: &ronoctua_i18n::Texts) -> String {
    let share = n
        .saturating_mul(100)
        .checked_div(most)
        .map_or(0, |p| u8::try_from(p.min(100)).unwrap_or(100));
    texts.t_with(
        key::AGENTS_TOKENS,
        &[
            ("bar", &crate::chrome::meter(share, crate::sizes::BAR_CELLS)),
            ("count", &words::tokens_text(n, texts)),
        ],
    )
}

/// The question the view asks before something it cannot undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// Remove this account — its id, its name and its agent.
    Remove(String, String, Agent),
}

// ---------------------------------------------------------------------------
// QUESTIONS ON THE LINE, AND WHAT THE DOOR DOES
// ---------------------------------------------------------------------------

/// Something the person types, and what it is for — asked on the line of
/// the door the key was pressed in, answered through [`View::answered`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// An account's new name.
    Name(String),
    /// A new account's name.
    New(Agent),
    /// The warning's line.
    Warn,
    /// The danger's line.
    Danger,
}

impl Ask {
    /// The command that asks — and that answers: the question is its
    /// `ask-<id>` key.
    #[must_use]
    pub const fn command(&self) -> &'static str {
        match self {
            Self::Name(_) => "agents-rename",
            Self::New(Agent::Claude) => "agents-add-claude",
            Self::New(Agent::Codex) => "agents-add-codex",
            Self::Warn => "agents-warn-at",
            Self::Danger => "agents-danger-at",
        }
    }

    /// The line a door opens for it — a name being changed starts from what
    /// it is, a line from where it is.
    #[must_use]
    pub fn line(&self, r: &Readings) -> crate::line::Line {
        let initial = match self {
            Self::Name(id) => r.seen(id).map(|s| s.account.name.clone()),
            Self::Warn => Some(r.lines.warn_percent.to_string()),
            Self::Danger => Some(r.lines.danger_percent.to_string()),
            Self::New(_) => None,
        };
        crate::line::Line::new(self.command(), false, initial.as_deref())
    }
}

/// A change to the list, or to an account's status line — applied by the
/// door ([`apply`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// A new account of an agent, named.
    Add(Agent, String),
    /// An account's new name.
    Rename(String, String),
    /// An account forgotten (its folder stays).
    Remove(String),
    /// An account made the default of its kind.
    Default(String),
    /// Our status line put in front of an account's.
    Wrap(String),
    /// An account's own status line given back.
    Unwrap(String),
}

/// What the door has to do after a key, a click or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Something was written here (a line, a consent): read again — the one
    /// errand besides [`Errand::Edit`] that does — and draw.
    Wrote,
    /// Shut the view.
    Close,
    /// Open this question on the door's own line.
    Ask(crate::line::Line),
    /// Say this sentence on the door's line.
    Say(String),
    /// Put this on the clipboard.
    Copy(String),
    /// Change the list.
    Edit(Edit),
    /// A session of this account (`false`) or its login (`true`), in the
    /// terminal.
    Open(String, bool),
    /// Count and ask again, off the loop.
    Refresh,
}

impl Errand {
    /// Whether carrying it out writes a file this view reads — the doors
    /// read again after these only: a cursor walking writes nothing, and a
    /// reading opens every account's files (and a Codex account's logs).
    #[must_use]
    pub const fn writes(&self) -> bool {
        matches!(self, Self::Edit(_) | Self::Wrote)
    }
}

/// What applying an edit came to: what the door's line says, and the
/// terminal it opens (a new account's login).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Applied {
    /// The sentence.
    pub said: Option<String>,
    /// `ronoctua-shell terminal …`'s words, when a terminal opens.
    pub terminal: Option<Vec<String>>,
}

/// What opens a terminal for an account: its session (`false`) or its login
/// (`true`) — the words of `ronoctua-shell terminal …`, or why not. The
/// machine's is [`terminal_of`]; a test hands its own, and never starts an
/// agent.
pub type Terminal<'a> = &'a dyn Fn(&Account, bool) -> Result<Vec<String>, Refusal>;

/// Applies an edit to the list, as the file says it NOW
/// (`Accounts::keep`), and says what came of it.
#[must_use]
pub fn apply(edit: &Edit, texts: &ronoctua_i18n::Texts) -> Applied {
    apply_in(&ronoctua_llm::Roots::of_env(), &terminal_of, edit, texts)
}

/// [`apply`], under given roots and with a given terminal — what a test
/// drives, over a folder of its own.
#[must_use]
pub fn apply_in(
    roots: &ronoctua_llm::Roots,
    terminal: Terminal<'_>,
    edit: &Edit,
    texts: &ronoctua_i18n::Texts,
) -> Applied {
    let said = |k: &str, name: &str| Some(texts.t_with(k, &[("name", name)]));
    let refused = |r: Refusal| Applied {
        said: Some(words::refused(r.why, texts)),
        terminal: None,
    };
    let store = Accounts::store_in(roots.config.clone());
    let name_of = |id: &str| {
        store
            .read()
            .find(id)
            .map_or_else(|| id.to_owned(), |a| a.name.clone())
    };
    match edit {
        Edit::Add(agent, name) => match add_in(roots, *agent, name) {
            Ok((account, wrapped)) => {
                let mut sentence = texts.t_with(key::AGENTS_ADDED, &[("name", &account.name)]);
                if let Err(why) = wrapped {
                    sentence.push(' ');
                    sentence.push_str(&words::refused(why, texts));
                }
                match terminal(&account, true) {
                    Ok(line) => Applied {
                        said: Some(sentence),
                        terminal: Some(line),
                    },
                    Err(r) => {
                        sentence.push(' ');
                        sentence.push_str(&words::refused(r.why, texts));
                        Applied {
                            said: Some(sentence),
                            terminal: None,
                        }
                    }
                }
            }
            Err(r) => refused(r),
        },
        Edit::Rename(id, name) => match Accounts::keep_in(roots, |l| l.rename(id, name)) {
            Ok(_) => Applied {
                said: said(key::AGENTS_RENAMED, name.trim()),
                terminal: None,
            },
            Err(r) => refused(r),
        },
        Edit::Remove(id) => {
            let name = name_of(id);
            match Accounts::keep_in(roots, |l| l.remove(id)) {
                Ok(_) => Applied {
                    said: said(key::AGENTS_REMOVED, &name),
                    terminal: None,
                },
                Err(r) => refused(r),
            }
        }
        Edit::Default(id) => match Accounts::keep_in(roots, |l| l.set_default(id)) {
            Ok(_) => Applied {
                said: said(key::AGENTS_MADE_DEFAULT, &name_of(id)),
                terminal: None,
            },
            Err(r) => refused(r),
        },
        Edit::Wrap(id) | Edit::Unwrap(id) => {
            let wrap = matches!(edit, Edit::Wrap(_));
            let list = store.read();
            let Some(account) = list.find(id) else {
                return refused(Refusal::of(Why::Unknown));
            };
            let done = if wrap {
                ronoctua_llm::statusline::wrap(account)
            } else {
                ronoctua_llm::statusline::unwrap(account)
            };
            match done {
                Ok(()) => Applied {
                    said: said(
                        if wrap {
                            key::AGENTS_WRAPPED
                        } else {
                            key::AGENTS_UNWRAPPED
                        },
                        &account.name,
                    ),
                    terminal: None,
                },
                Err(r) => refused(r),
            }
        }
    }
}

/// A new account: its folder made first (`0700`, the agent's own mode for
/// its credentials' folder) — so the watches hear it from the start, and
/// Codex, which wants its folder to exist, finds it — then added to the list
/// as the file says it now, and, for Claude Code, our status line put in
/// front: the limits come from it.
fn add_in(
    roots: &ronoctua_llm::Roots,
    agent: Agent,
    name: &str,
) -> Result<(Account, Result<(), Why>), Refusal> {
    let home = roots.home.as_deref().ok_or(Refusal::of(Why::Failed))?;
    let list = Accounts::store_in(roots.config.clone())
        .parsed()
        .ok_or(Refusal::of(Why::Unreadable))?;
    let account = list.fresh(agent, name, home)?;
    made_private(&account.dir)?;
    Accounts::keep_in(roots, |l| l.add(account.clone()))?;
    let wrapped = if agent == Agent::Claude {
        ronoctua_llm::statusline::wrap(&account).map_err(|r| r.why)
    } else {
        Ok(())
    };
    Ok((account, wrapped))
}

/// A folder, made `0700` when it is not there.
fn made_private(dir: &Path) -> Result<(), Refusal> {
    use std::os::unix::fs::DirBuilderExt as _;
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| {
            tracing::warn!(error = %e, dir = %dir.display(), "an account's folder could not be made");
            Refusal::of(Why::Failed)
        })
}

/// `ronoctua-shell terminal …`'s words for a session of `account`, or its
/// login: the crate's own command line (`launch_arguments`,
/// `login_arguments`) — the agent by the store path the session handed
/// over, its folder by its variable — in our terminal. The folder is made
/// first: Codex refuses one that is not there.
///
/// # Errors
///
/// The crate's: the agent is not installed, the account is not one.
pub fn terminal_of(account: &Account, login: bool) -> Result<Vec<String>, Refusal> {
    let argv = if login {
        ronoctua_llm::login_arguments(account)?
    } else {
        ronoctua_llm::launch_arguments(account)?
    };
    made_private(&account.dir)?;
    let mut line = vec!["terminal".to_owned()];
    line.extend(argv);
    Ok(line)
}

/// What `agents-session [n]` / `agents-login [n]` open from anywhere but the
/// view: the account at place `n`, or the one a gesture with no row means
/// ([`chosen_of`]) — the terminal's words, or the sentence that says why not.
///
/// # Errors
///
/// The sentence: no account yet, or the terminal's refusal.
pub fn from_anywhere(
    accounts: &[Account],
    place: Option<usize>,
    login: bool,
    terminal: Terminal<'_>,
    texts: &ronoctua_i18n::Texts,
) -> Result<Vec<String>, String> {
    let account = chosen_of(accounts, place).ok_or_else(|| texts.t(key::AGENTS_NONE))?;
    terminal(&account, login).map_err(|r| words::refused(r.why, texts))
}

/// The marks the accounts' brain module would put on the modeline now,
/// read from the files under this process's roots, laid over a mind in
/// place of any of the module's it had — what the footer's camera draws
/// (`--accounts-marks`), so a picture's marks are the module's own and
/// never written by hand.
pub fn with_accounts_marks(mind: &mut ronoctua_brain::Mind, now: i64) {
    mind.marks.retain(|m| m.module != agent_accounts::ID);
    mind.marks
        .extend(agent_accounts::marks_read_now(now).into_iter().map(|what| {
            ronoctua_brain::mind::Posted {
                module: agent_accounts::ID.to_owned(),
                what,
            }
        }));
}

/// The line of the door's question, for a door that asks it.
#[must_use]
pub fn question_text(q: &Question, texts: &ronoctua_i18n::Texts) -> String {
    match q {
        // A CLAUDE CODE ACCOUNT gives its own status line back as it goes;
        // Codex has none, and is not told it does.
        Question::Remove(_, name, Agent::Claude) => {
            texts.t_with(key::AGENTS_ASK_REMOVE_CLAUDE, &[("name", name)])
        }
        Question::Remove(_, name, Agent::Codex) => {
            texts.t_with(key::AGENTS_ASK_REMOVE, &[("name", name)])
        }
    }
}

/// The view: where the cursor is, what is asked, what it is aimed at.
/// Everything it DRAWS is read from the [`Readings`] each door hands it.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    asking: Option<Ask>,
    question: Option<Question>,
    /// The answer the question's cursor is on.
    yes: bool,
    /// The row the cursor is on, by what it is — kept across a rebuild.
    aim: Option<Target>,
}

impl View {
    /// Where the cursor is, for the door that draws.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The question a line is open for.
    #[must_use]
    pub const fn asking(&self) -> Option<&Ask> {
        self.asking.as_ref()
    }

    /// The question drawn in the view, "no" first.
    #[cfg(test)]
    #[must_use]
    pub const fn question(&self) -> Option<&Question> {
        self.question.as_ref()
    }

    /// Aims at the account at `place` (from 1) — a mark clicked, `agents-open
    /// <n>` typed. A place with no account keeps the cursor where it is.
    pub fn aim_at_place(&mut self, place: usize, r: &Readings) {
        if let Some(s) = r.at_place(place) {
            self.question = None;
            self.aim = Some(Target::Fact(s.account.id.clone(), Fact::Agent));
        }
    }

    /// The account under the cursor, or the one a gesture with no row means.
    fn account_here<'a>(&self, rows: &[Row], r: &'a Readings) -> Option<&'a Seen> {
        rows.get(self.cursor)
            .and_then(|row| row.target.as_ref())
            .and_then(Target::account)
            .and_then(|id| r.seen(id))
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(r, zone, columns, texts);
        let on_one = self.account_here(&rows, r).is_some();
        let mut here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, id, w)| {
                // A QUESTION UP takes walking and answering, nothing else.
                if self.question.is_some() {
                    return matches!(*id, "agents-down" | "agents-up" | "agents-enter");
                }
                match w {
                    Where::Anywhere => true,
                    Where::OnAnAccount => on_one,
                }
            })
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        // THE CONSENT ROW'S OWN KEY, taught while the row is drawn.
        if self.question.is_none() && r.has_claude() && !r.let_usage {
            here.extend(
                crate::consent_rows::OF_THE_CONSENT
                    .iter()
                    .map(|(k, id)| (*k, *id, false)),
            );
        }
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// Puts the cursor where a click landed. Answers whether the cursor was
    /// ALREADY on that row: that is the second click, which the door turns
    /// into `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let Some(target) = rows
            .get(n)
            .filter(|r| r.takes())
            .and_then(|r| r.target.clone())
        else {
            return false;
        };
        let again = self.cursor == n;
        self.cursor = n;
        if let Target::Answer(yes) = target {
            self.yes = yes;
        }
        self.aim = Some(target);
        again
    }

    /// The sentence the pointer resting on row `n` reads.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        rows.get(n)?.target.as_ref()?.about(texts)
    }

    /// Puts the cursor back on the row it was aimed at — found by what it
    /// is, since the list may have changed under it — or on the nearest row
    /// that acts. Called after every reading.
    pub fn settle_in(
        &mut self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        // A QUESTION ABOUT AN ACCOUNT THAT IS GONE goes with it.
        if let Some(Question::Remove(id, ..)) = &self.question
            && r.seen(id).is_none()
        {
            self.question = None;
        }
        let rows = self.rows(r, zone, columns, texts);
        if let Some(aim) = self.aim.as_ref()
            && let Some(n) = rows.iter().position(|row| row.target.as_ref() == Some(aim))
        {
            self.cursor = n;
            return;
        }
        if rows.get(self.cursor).is_some_and(|row| !row.heading()) {
            self.aim = rows[self.cursor].target.clone();
            return;
        }
        let at = self.cursor.min(rows.len().saturating_sub(1));
        let found = (at..rows.len())
            .chain((0..at).rev())
            .find(|n| !rows[*n].heading());
        self.cursor = found.unwrap_or(0);
        self.aim = rows.get(self.cursor).and_then(|row| row.target.clone());
    }

    /// The rows to draw, at this width, from what is read now.
    #[must_use]
    pub fn rows(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = crate::chrome::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            cursor: self.cursor,
            left,
            room,
            texts,
        };
        if let Some(q) = &self.question {
            self.question_rows(&mut b, q);
            return b.out;
        }
        if !r.read {
            b.section(
                &b.t(key::AGENTS_HEADING),
                vec![note(texts.t(key::AGENTS_READING))],
            );
            return b.out;
        }
        if r.refused {
            b.section(
                &b.t(key::AGENTS_HEADING),
                vec![note(words::refused(Why::Unreadable, texts))],
            );
        } else if r.accounts.is_empty() {
            b.section(
                &b.t(key::AGENTS_HEADING),
                vec![note(texts.t(key::AGENTS_NONE))],
            );
        }
        for (n, s) in r.accounts.iter().enumerate() {
            account_section(&mut b, r, s, n + 1, zone);
        }
        if r.has_claude() {
            endpoint_section(&mut b, r);
        }
        if !r.refused {
            add_section(&mut b);
        }
        lines_section(&mut b, r);
        b.out
    }

    /// The question's page: its sentence, then "no" and "yes" — "no" first,
    /// and the cursor born on it.
    fn question_rows(&self, b: &mut Builder<'_>, q: &Question) {
        let t = b.texts;
        b.section(
            &t.t(key::AGENTS_HEADING),
            vec![Entry::Note(question_text(q, t))],
        );
        for (yes, k) in [
            (false, key::AGENTS_ANSWER_NO),
            (true, key::AGENTS_ANSWER_YES),
        ] {
            let on = b.cursor == b.out.len();
            // THE CURSOR IS A COLOUR; "yes" is in the colour of danger.
            let color = if on {
                tokens::color::TEXT_HERE_ACTS
            } else if yes {
                tokens::color::STATE_ERROR
            } else {
                crate::chrome::Kind::Setting.tones(false).0
            };
            b.out.push(Row {
                fields: vec![piece(b.left, t.t(k), color, b.room)],
                target: Some(Target::Answer(yes)),
            });
        }
    }

    /// What a key does. THE WIDTH IS THE DOOR'S: the cursor is an index into
    /// the rows that were DRAWN.
    pub fn press(
        &mut self,
        p: Press,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(r, zone, columns, texts);
        if self.question.is_some() {
            return self.answer_key(p, &rows);
        }
        let target = rows.get(self.cursor).and_then(|row| row.target.clone());
        let here = self.account_here(&rows, r).map(|s| s.account.id.clone());
        let on_an_account = |make: fn(String) -> Target| here.clone().map(make);
        match p {
            Press::Up | Press::Down => {
                self.walk(&rows, p == Press::Down);
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            Press::Enter => match target {
                Some(t) => self.enter(t, r, zone, columns, texts),
                None => Errand::Nothing,
            },
            Press::Allow => {
                if r.has_claude() && !r.let_usage {
                    allow(texts)
                } else {
                    Errand::Nothing
                }
            }
            Press::Refresh => Errand::Refresh,
            Press::Session | Press::Login => {
                // ON A ROW OF AN ACCOUNT, that account; anywhere else, the
                // one a gesture with no row means.
                let id = here.or_else(|| r.chosen().map(|s| s.account.id.clone()));
                match id {
                    Some(id) => Errand::Open(id, p == Press::Login),
                    None => Errand::Say(texts.t(key::AGENTS_NONE)),
                }
            }
            Press::Rename | Press::Default | Press::Remove => {
                let made = match p {
                    Press::Rename => on_an_account(Target::Rename),
                    Press::Default => on_an_account(Target::Default),
                    _ => on_an_account(Target::Remove),
                };
                match made {
                    Some(t) => self.enter(t, r, zone, columns, texts),
                    None => Errand::Say(texts.t(key::AGENTS_NOT_ON_AN_ACCOUNT)),
                }
            }
        }
    }

    /// A command by its name — `LEADER :`, a leader key, a click on a hint,
    /// a mark. What `RET` on its row does, whatever row the cursor is on.
    pub fn command(
        &mut self,
        id: &str,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if let Some(p) = press_of(id) {
            return self.press(p, r, zone, columns, texts);
        }
        let target = match id {
            "agents-add-claude" => Target::Add(Agent::Claude),
            "agents-add-codex" => Target::Add(Agent::Codex),
            "agents-warn-at" => Target::Warn,
            "agents-danger-at" => Target::Danger,
            "agents-status-line" => {
                let rows = self.rows(r, zone, columns, texts);
                let claude = self
                    .account_here(&rows, r)
                    .or_else(|| r.chosen())
                    .filter(|s| s.account.kind == Agent::Claude);
                match claude {
                    Some(s) => Target::StatusLine(s.account.id.clone()),
                    None => return Errand::Say(words::refused(Why::Invalid, texts)),
                }
            }
            _ => return Errand::Nothing,
        };
        self.enter(target, r, zone, columns, texts)
    }

    /// `RET` on a row.
    fn enter(
        &mut self,
        target: Target,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match target {
            // A WINDOW ONLY TELLS: its bar is not a value to copy.
            Target::Fact(_, Fact::Window(_)) | Target::Answer(_) | Target::Endpoint => {
                Errand::Nothing
            }
            Target::Fact(..) => {
                let rows = self.rows(r, zone, columns, texts);
                rows.iter()
                    .find(|row| row.target.as_ref() == Some(&target))
                    .and_then(|row| row.fields.last())
                    .map(|f| f.text.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .map_or(Errand::Nothing, Errand::Copy)
            }
            Target::StatusLine(id) => match r.seen(&id).and_then(|s| s.wrapping) {
                Some(Ok(Wrapping::Ours)) => Errand::Edit(Edit::Unwrap(id)),
                Some(Ok(_)) => Errand::Edit(Edit::Wrap(id)),
                Some(Err(why)) => Errand::Say(words::refused(why, texts)),
                None => Errand::Say(words::refused(Why::Invalid, texts)),
            },
            Target::Session(id) => Errand::Open(id, false),
            Target::Login(id) => Errand::Open(id, true),
            Target::Rename(id) => self.ask(Ask::Name(id), r),
            Target::Default(id) => {
                if r.seen(&id).is_some_and(|s| s.account.default) {
                    Errand::Say(texts.t(key::AGENTS_ALREADY_DEFAULT))
                } else {
                    Errand::Edit(Edit::Default(id))
                }
            }
            Target::Remove(id) => {
                let (name, agent) = r.seen(&id).map_or_else(
                    || (id.clone(), Agent::Codex),
                    |s| (s.account.name.clone(), s.account.kind),
                );
                self.question = Some(Question::Remove(id, name, agent));
                // "NO" FIRST, and the cursor is born on it.
                self.yes = false;
                self.aim = Some(Target::Answer(false));
                self.settle_in(r, zone, columns, texts);
                Errand::Nothing
            }
            Target::Let => allow(texts),
            Target::Add(agent) => self.ask(Ask::New(agent), r),
            Target::Refresh => Errand::Refresh,
            Target::Warn => self.ask(Ask::Warn, r),
            Target::Danger => self.ask(Ask::Danger, r),
        }
    }

    /// A key while the question is up: walking moves between "no" and
    /// "yes", `RET` answers, and every other key is "no".
    fn answer_key(&mut self, p: Press, rows: &[Row]) -> Errand {
        match p {
            Press::Up | Press::Down => {
                self.walk(rows, p == Press::Down);
                if let Some(Target::Answer(yes)) =
                    rows.get(self.cursor).and_then(|r| r.target.clone())
                {
                    self.yes = yes;
                }
                Errand::Nothing
            }
            Press::Enter => {
                let yes = matches!(
                    rows.get(self.cursor).and_then(|r| r.target.clone()),
                    Some(Target::Answer(true))
                );
                let q = self.question.take();
                self.aim = None;
                match (q, yes) {
                    (Some(Question::Remove(id, ..)), true) => Errand::Edit(Edit::Remove(id)),
                    _ => Errand::Nothing,
                }
            }
            // A QUESTION SOMEBODY WALKED AWAY FROM is not a yes.
            _ => {
                self.question = None;
                self.aim = None;
                Errand::Nothing
            }
        }
    }

    /// Opens a question on the line, remembered: the answer alone cannot
    /// say what it answers.
    fn ask(&mut self, ask: Ask, r: &Readings) -> Errand {
        let line = ask.line(r);
        self.asking = Some(ask);
        Errand::Ask(line)
    }

    /// What was typed on a line, under the line's id — one call for both
    /// doors. THE QUESTION THE VIEW ASKED ANSWERS, and only when the id is
    /// its own.
    pub fn answered(&mut self, id: &str, said: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(ask) = self.asking.take().filter(|a| a.command() == id) else {
            return Errand::Nothing;
        };
        let typed = said.trim();
        match ask {
            Ask::Name(id) => Errand::Edit(Edit::Rename(id, typed.to_owned())),
            Ask::New(agent) => {
                if typed.is_empty() {
                    return Errand::Say(words::refused(Why::Invalid, texts));
                }
                Errand::Edit(Edit::Add(agent, typed.to_owned()))
            }
            Ask::Warn | Ask::Danger => {
                let Some(p) = typed
                    .trim_end_matches('%')
                    .trim()
                    .parse::<u8>()
                    .ok()
                    .filter(|p| (1..=100).contains(p))
                else {
                    return Errand::Say(texts.t(key::AGENTS_NOT_A_PERCENT));
                };
                let which = if ask == Ask::Warn {
                    agent_accounts::Which::Warn
                } else {
                    agent_accounts::Which::Danger
                };
                // ONE LINE, laid over what the file says NOW: the other door
                // and the module's page write it too.
                match agent_accounts::keep_line(which, p) {
                    Ok(_) => Errand::Wrote,
                    Err(agent_accounts::Unkept::NotInOrder) => {
                        Errand::Say(texts.t(key::AGENTS_LINES_CROSS))
                    }
                    Err(agent_accounts::Unkept::Unreadable) => {
                        Errand::Say(texts.t(key::AGENTS_LINES_UNREADABLE))
                    }
                    Err(agent_accounts::Unkept::Unwritten) => {
                        Errand::Say(texts.t(key::AGENTS_LINES_UNKEPT))
                    }
                }
            }
        }
    }

    /// The line was let go — `Esc`, `C-g`, a click on `ESC` — and the
    /// question with it.
    pub fn let_go(&mut self) {
        self.asking = None;
    }

    /// Moves the cursor to the next row that acts, round the ends.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows.get(at).is_some_and(|r| !r.heading()) {
                self.cursor = at;
                self.aim = rows[at].target.clone();
                return;
            }
        }
    }
}

/// Lets the usage endpoint's host be asked — merged into `consent.json`,
/// which the unit and the doors mint their pass from. What could not be
/// kept is said.
fn allow(texts: &ronoctua_i18n::Texts) -> Errand {
    match crate::consent_rows::allow(&[usage::HOST], texts) {
        Ok(()) => Errand::Wrote,
        Err(said) => Errand::Say(said),
    }
}

/// One account: its heading, its facts, its windows, its tokens and the
/// rows that act on it.
fn account_section(
    b: &mut Builder<'_>,
    r: &Readings,
    s: &Seen,
    place: usize,
    zone: &ronoctua_files::when::Zone,
) {
    let t = b.texts;
    let a = &s.account;
    let id = || a.id.clone();
    let glyph = agent_accounts::mark_of(a.kind)
        .and_then(|m| crate::brainlink::glyph_of(&m.glyph))
        .map(String::from)
        .unwrap_or_default();
    let title = t.t_args(
        key::AGENTS_ACCOUNT,
        &[
            ("glyph", Arg::Text(&glyph)),
            (
                "place",
                Arg::Number(i64::try_from(place).unwrap_or(i64::MAX)),
            ),
            ("name", Arg::Text(&a.name)),
        ],
    );
    let mut e = Vec::new();
    e.push(pair(
        t.t(key::AGENTS_ROW_AGENT),
        t.t(words::agent_key(a.kind)),
        Target::Fact(id(), Fact::Agent),
        false,
    ));
    e.push(pair(
        t.t(key::AGENTS_ROW_FOLDER),
        shown_dir(&a.dir, r.home.as_deref()),
        Target::Fact(id(), Fact::Folder),
        false,
    ));
    let plan = match &s.facts {
        Ok(f) => match (&f.subscription, &f.tier) {
            (Some(sub), Some(tier)) => t.t_with(key::AGENTS_PLAN, &[("plan", sub), ("tier", tier)]),
            (Some(sub), None) => sub.clone(),
            _ => t.t(key::AGENTS_PLAN_UNSAID),
        },
        Err(Why::NoCredentials) => t.t(key::AGENTS_NOT_LOGGED_IN),
        Err(why) => words::refused(*why, t),
    };
    e.push(pair(
        t.t(key::AGENTS_ROW_PLAN),
        plan,
        Target::Fact(id(), Fact::Plan),
        false,
    ));
    // A LOGIN PAST ITS DATE: Claude Code renews it when it next runs.
    if let Ok(f) = &s.facts
        && f.expires_at_s.is_some_and(|at| at <= r.now_s)
    {
        e.push(note(words::refused(Why::Expired, t)));
    }

    // THE WINDOWS, as bars in the colour the marks wear.
    e.push(Entry::Air);
    for (span, w) in windows_of(s, r.now_s) {
        e.push(tinted(
            t.t(span.label()),
            window_value(w, zone, t),
            Target::Fact(id(), Fact::Window(span)),
            window_tint(w, r.lines),
        ));
    }
    if let Some(said) = source_note(s, r, zone, t) {
        e.push(note(said));
    }

    // THE TOKENS: a week by day, newest first, and the models, most first.
    e.push(Entry::Air);
    let days: Vec<(&String, &u64)> = s.tally.by_day.iter().rev().take(DAYS_SHOWN).collect();
    let most = days.iter().map(|(_, n)| **n).max().unwrap_or(0);
    for (day, n) in &days {
        e.push(pair(
            day_text(day, t),
            counted(**n, most, t),
            Target::Fact(id(), Fact::Day((*day).clone())),
            false,
        ));
    }
    let mut models: Vec<(&String, &u64)> = s.tally.by_model.iter().collect();
    models.sort_by(|x, y| y.1.cmp(x.1).then_with(|| x.0.cmp(y.0)));
    let most = models.first().map_or(0, |(_, n)| **n);
    for (model, n) in models.into_iter().take(MODELS_SHOWN) {
        let label = if model.is_empty() {
            t.t(key::AGENTS_MODEL_UNNAMED)
        } else {
            model.clone()
        };
        e.push(pair(
            label,
            counted(*n, most, t),
            Target::Fact(id(), Fact::Model(model.clone())),
            false,
        ));
    }
    if days.is_empty() {
        e.push(pair(
            t.t(key::AGENTS_ROW_TOKENS),
            t.t(key::AGENTS_TOKENS_NONE),
            Target::Fact(id(), Fact::NoTokens),
            false,
        ));
    }

    // WHAT ACTS ON IT.
    e.push(Entry::Air);
    if let Some(w) = &s.wrapping {
        let value = match w {
            Ok(w) => t.t(words::wrapping_key(*w)),
            Err(why) => words::refused(*why, t),
        };
        e.push(pair(
            t.t(key::AGENTS_ROW_STATUS_LINE),
            value,
            Target::StatusLine(id()),
            true,
        ));
    }
    e.push(pair(
        t.t(key::AGENTS_ROW_SESSION),
        String::new(),
        Target::Session(id()),
        true,
    ));
    e.push(pair(
        t.t(key::AGENTS_ROW_LOGIN),
        String::new(),
        Target::Login(id()),
        true,
    ));
    e.push(pair(
        t.t(key::AGENTS_ROW_NAME),
        a.name.clone(),
        Target::Rename(id()),
        true,
    ));
    e.push(pair(
        t.t(key::AGENTS_ROW_DEFAULT),
        t.t(if a.default {
            key::AGENTS_DEFAULT_YES
        } else {
            key::AGENTS_DEFAULT_NO
        }),
        Target::Default(id()),
        !a.default,
    ));
    e.push(pair(
        t.t(key::AGENTS_ROW_REMOVE),
        String::new(),
        Target::Remove(id()),
        true,
    ));
    b.section(&title, e);
}

/// Where an account's numbers came from, and when — or why they did not.
fn source_note(
    s: &Seen,
    r: &Readings,
    zone: &ronoctua_files::when::Zone,
    t: &ronoctua_i18n::Texts,
) -> Option<String> {
    if s.account.kind == Agent::Codex {
        return Some(t.t(key::AGENTS_FROM_LOGS));
    }
    let asked = s.usage.and_then(|k| k.usage_at_s.map(|at| (at, k.refused)));
    let refused = s.usage.and_then(|k| k.refused);
    Some(match (asked, refused) {
        (_, Some(why)) => t.t_with(
            key::AGENTS_ENDPOINT_REFUSED,
            &[("why", &words::refused(why, t))],
        ),
        (Some((at, None)), None) => t.t_with(
            key::AGENTS_FROM_ENDPOINT,
            &[("when", &crate::chrome::when_text(at, zone, false, t))],
        ),
        _ if !r.let_usage => t.t(key::AGENTS_ENDPOINT_NOT_LET),
        _ => t.t(key::AGENTS_ENDPOINT_NOT_ASKED),
    })
}

/// The usage endpoint: the consent row while its host is not let, and what
/// it is — unofficial — said either way.
fn endpoint_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let mut e = Vec::new();
    if r.let_usage {
        e.push(pair(
            t.t(key::AGENTS_ROW_ENDPOINT),
            t.t(key::AGENTS_ENDPOINT_LET),
            Target::Endpoint,
            false,
        ));
    } else {
        // THE HOST NOBODY LET YET: the row every app draws for it
        // (`consent_rows`) — its label is the press, its note what for.
        e.push(pair(
            crate::consent_rows::label(&[usage::HOST], t),
            String::new(),
            Target::Let,
            true,
        ));
    }
    e.push(note(crate::consent_rows::note(key::AGENTS_CONSENT_FOR, t)));
    b.section(&t.t(key::AGENTS_HEADING_ENDPOINT), e);
}

/// The rows that add an account, and the refresh.
fn add_section(b: &mut Builder<'_>) {
    let t = b.texts;
    let mut e: Vec<Entry> = Agent::ALL
        .into_iter()
        .map(|agent| {
            pair(
                t.t(words::adding_key(agent)),
                String::new(),
                Target::Add(agent),
                true,
            )
        })
        .collect();
    e.push(note(t.t(key::AGENTS_ADD_NOTE)));
    e.push(Entry::Air);
    e.push(pair(
        t.t(key::AGENTS_ROW_REFRESH),
        String::new(),
        Target::Refresh,
        true,
    ));
    b.section(&t.t(key::AGENTS_HEADING_ADD), e);
}

/// The two lines a bar — and a mark — changes colour at.
fn lines_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let percent = |p: u8| words::percent_text(f32::from(p), t);
    let e = vec![
        Entry::Pair {
            label: t.t(key::AGENTS_ROW_WARN),
            value: percent(r.lines.warn_percent),
            kind: crate::chrome::Kind::Setting,
            target: Target::Warn,
            tint: Some(crate::brainlink::tone_colour(Tone::Warning)),
        },
        Entry::Pair {
            label: t.t(key::AGENTS_ROW_DANGER),
            value: percent(r.lines.danger_percent),
            kind: crate::chrome::Kind::Setting,
            target: Target::Danger,
            tint: Some(crate::brainlink::tone_colour(Tone::Danger)),
        },
        note(t.t(key::AGENTS_LINES_NOTE)),
    ];
    b.section(&t.t(key::AGENTS_HEADING_LINES), e);
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "agents",
    commands: &[
        ronoctua_cmd::Command {
            id: "agents-open",
            label: crate::key::CMD_AGENTS_OPEN,
        },
        ronoctua_cmd::Command {
            id: "agents-window",
            label: crate::key::CMD_AGENTS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "agents-session",
            label: crate::key::CMD_AGENTS_SESSION,
        },
        ronoctua_cmd::Command {
            id: "agents-login",
            label: crate::key::CMD_AGENTS_LOGIN,
        },
        // THE VIEW'S KEYS AND ROWS (`agents::OF_THE_VIEW`, `OF_THE_ROWS`).
        ronoctua_cmd::Command {
            id: "agents-up",
            label: crate::key::CMD_AGENTS_UP,
        },
        ronoctua_cmd::Command {
            id: "agents-down",
            label: crate::key::CMD_AGENTS_DOWN,
        },
        ronoctua_cmd::Command {
            id: "agents-enter",
            label: crate::key::CMD_AGENTS_ENTER,
        },
        ronoctua_cmd::Command {
            id: "agents-rename",
            label: crate::key::CMD_AGENTS_RENAME,
        },
        ronoctua_cmd::Command {
            id: "agents-default",
            label: crate::key::CMD_AGENTS_DEFAULT,
        },
        ronoctua_cmd::Command {
            id: "agents-remove",
            label: crate::key::CMD_AGENTS_REMOVE,
        },
        ronoctua_cmd::Command {
            id: "agents-refresh",
            label: crate::key::CMD_AGENTS_REFRESH,
        },
        ronoctua_cmd::Command {
            id: "agents-add-claude",
            label: crate::key::CMD_AGENTS_ADD_CLAUDE,
        },
        ronoctua_cmd::Command {
            id: "agents-add-codex",
            label: crate::key::CMD_AGENTS_ADD_CODEX,
        },
        ronoctua_cmd::Command {
            id: "agents-status-line",
            label: crate::key::CMD_AGENTS_STATUS_LINE,
        },
        ronoctua_cmd::Command {
            id: "agents-warn-at",
            label: crate::key::CMD_AGENTS_WARN_AT,
        },
        ronoctua_cmd::Command {
            id: "agents-danger-at",
            label: crate::key::CMD_AGENTS_DANGER_AT,
        },
    ],
    bindings: &[
        ("m a a", "agents-open", None),
        ("m a w", "agents-window", None),
        ("m a n", "agents-session", None),
        ("m a l", "agents-login", None),
    ],
    groups: &[("m a", crate::key::GROUP_AGENTS)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::agents::reached().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_791_201_600;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("the language")
    }

    fn zone() -> ronoctua_files::when::Zone {
        ronoctua_files::when::Zone::utc()
    }

    fn window(used_percent: f32, resets_at_s: i64) -> Window {
        Window {
            used_percent,
            resets_at_s,
        }
    }

    fn seen(id: &str, kind: Agent, name: &str, default: bool) -> Seen {
        let mut tally = Tally::default();
        tally.by_day.insert("2026-10-04".to_owned(), 120_000);
        tally.by_day.insert("2026-10-05".to_owned(), 2_400_000);
        tally
            .by_model
            .insert("claude-opus-4-1".to_owned(), 2_000_000);
        tally.by_model.insert(String::new(), 520_000);
        Seen {
            account: Account {
                id: id.to_owned(),
                kind,
                name: name.to_owned(),
                dir: PathBuf::from(format!("/home/p/.{id}")),
                default,
            },
            facts: Ok(Facts {
                subscription: Some("max".to_owned()),
                tier: Some("default_claude_max_20x".to_owned()),
                expires_at_s: Some(NOW + 3600),
            }),
            limits: Ok(Limits {
                five_hour: Some(window(85.0, NOW + 3600)),
                seven_day: Some(window(40.0, NOW + 86_400)),
            }),
            usage: (kind == Agent::Claude).then_some(usage::Kept {
                format: 1,
                at_s: NOW - 60,
                usage: Some(usage::Usage {
                    five_hour: Some(window(80.0, NOW + 3600)),
                    seven_day: Some(window(41.0, NOW + 86_400)),
                    seven_day_opus: Some(window(97.0, NOW + 86_400)),
                    seven_day_sonnet: None,
                }),
                usage_at_s: Some(NOW - 60),
                refused: None,
            }),
            tally,
            wrapping: (kind == Agent::Claude).then_some(Ok(Wrapping::Ours)),
        }
    }

    fn readings() -> Readings {
        Readings {
            read: true,
            refused: false,
            accounts: vec![
                seen("claude-1", Agent::Claude, "Work", true),
                seen("codex-1", Agent::Codex, "Home", true),
            ],
            lines: agent_accounts::Lines::default(),
            let_usage: false,
            home: Some(PathBuf::from("/home/p")),
            now_s: NOW,
        }
    }

    fn drawn(rows: &[Row]) -> String {
        rows.iter()
            .map(|r| r.fields.iter().map(|f| f.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn stand_on(v: &mut View, r: &Readings, target: &Target, t: &ronoctua_i18n::Texts) {
        let rows = v.rows(r, &zone(), 100, t);
        let n = rows
            .iter()
            .position(|row| row.target.as_ref() == Some(target))
            .unwrap_or_else(|| panic!("no row is {target:?}"));
        if v.cursor() != n {
            assert!(!v.clicked(n, &rows));
        }
    }

    /// FAILS IF: a section, a window, a count or a refusal draws a raw key in
    /// either language, or a state of the view draws nothing.
    #[test]
    fn every_state_is_drawn_in_every_language() {
        for language in ronoctua_i18n::LANGUAGES {
            let t = texts(language);
            let mut states = vec![readings(), Readings::default()];
            let mut empty = readings();
            empty.accounts.clear();
            states.push(empty);
            let mut refused = readings();
            refused.refused = true;
            refused.accounts.clear();
            states.push(refused);
            let mut odd = readings();
            odd.let_usage = true;
            odd.accounts[0].facts = Err(Why::NoCredentials);
            odd.accounts[0].limits = Err(Why::Unreadable);
            odd.accounts[0].usage = Some(usage::Kept {
                format: 1,
                at_s: NOW,
                usage: None,
                usage_at_s: None,
                refused: Some(Why::Expired),
            });
            odd.accounts[0].wrapping = Some(Ok(Wrapping::Replaced));
            odd.accounts[0].tally = Tally::default();
            states.push(odd);
            for r in &states {
                let v = View::default();
                let rows = v.rows(r, &zone(), 100, &t);
                assert!(!rows.is_empty(), "{language}: nothing drawn");
                let text = drawn(&rows);
                assert!(
                    !text.contains("agents-"),
                    "{language}: a raw key in\n{text}"
                );
                let hint = v.hint(r, &zone(), 100, &t);
                assert!(!hint.is_empty());
            }
        }
    }

    /// FAILS IF: an account's windows do not take the fuller of their two
    /// readings, keep one that already reset, lose the endpoint's weekly
    /// window of a model, or a bar is not in the colour the brain's mark
    /// wears at the same per cent.
    #[test]
    fn the_windows_are_the_fuller_reading_in_the_marks_colour() {
        let r = readings();
        let w = windows_of(&r.accounts[0], NOW);
        assert_eq!(w[0], (Span::FiveHours, Some(window(85.0, NOW + 3600))));
        assert_eq!(w[1], (Span::Week, Some(window(41.0, NOW + 86_400))));
        assert_eq!(w[2], (Span::WeekOpus, Some(window(97.0, NOW + 86_400))));
        assert_eq!(w.len(), 3, "no Sonnet window was said");
        assert_eq!(
            windows_of(&r.accounts[0], NOW + 3600)[0],
            (Span::FiveHours, None)
        );
        let lines = agent_accounts::Lines::default();
        assert_eq!(
            window_tint(Some(window(85.0, NOW + 1)), lines),
            Some(crate::brainlink::tone_colour(Tone::Warning))
        );
        assert_eq!(
            window_tint(Some(window(97.0, NOW + 1)), lines),
            Some(crate::brainlink::tone_colour(Tone::Danger))
        );
        assert_eq!(window_tint(Some(window(20.0, NOW + 1)), lines), None);
        // THE PERSON'S LINES are the view's too.
        let theirs = agent_accounts::Lines {
            warn_percent: 10,
            danger_percent: 30,
        };
        assert_eq!(
            window_tint(Some(window(20.0, NOW + 1)), theirs),
            Some(crate::brainlink::tone_colour(Tone::Warning))
        );
        // AND DRAWN: the five hours' value wears the warning.
        let t = texts("en-GB");
        let rows = View::default().rows(&r, &zone(), 100, &t);
        let five = rows
            .iter()
            .find(|row| {
                row.target.as_ref()
                    == Some(&Target::Fact(
                        "claude-1".to_owned(),
                        Fact::Window(Span::FiveHours),
                    ))
            })
            .expect("the five hours' row");
        assert!(five.fields.iter().any(|f| f.text.contains("85%")));
        assert_eq!(
            five.fields.last().map(|f| f.color),
            Some(crate::brainlink::tone_colour(Tone::Warning))
        );
    }

    /// FAILS IF: a mark's place does not open its account, a session or a
    /// login opens another account than the one under the cursor (or the
    /// default with no row), or a key that acts on an account acts with
    /// the cursor on none.
    #[test]
    fn a_place_opens_its_account_and_a_session_is_the_cursors() {
        let t = texts("en-GB");
        let r = readings();
        let mut v = View::default();
        v.settle_in(&r, &zone(), 100, &t);
        v.aim_at_place(2, &r);
        v.settle_in(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Some(Target::Fact("codex-1".to_owned(), Fact::Agent))
        );
        assert_eq!(
            v.press(Press::Session, &r, &zone(), 100, &t),
            Errand::Open("codex-1".to_owned(), false)
        );
        assert_eq!(
            v.press(Press::Login, &r, &zone(), 100, &t),
            Errand::Open("codex-1".to_owned(), true)
        );
        stand_on(&mut v, &r, &Target::Refresh, &t);
        assert_eq!(
            v.press(Press::Session, &r, &zone(), 100, &t),
            Errand::Open("claude-1".to_owned(), false),
            "no row of an account: the default Claude Code one"
        );
        assert!(matches!(
            v.press(Press::Rename, &r, &zone(), 100, &t),
            Errand::Say(_)
        ));
        v.aim_at_place(9, &r);
        assert_eq!(
            v.command("agents-refresh", &r, &zone(), 100, &t),
            Errand::Refresh
        );
    }

    /// FAILS IF: removing an account goes through without a question, the
    /// question does not draw "no" first with the cursor on it, a key other
    /// than `RET` on "yes" removes it, or "yes" does not.
    #[test]
    fn removing_asks_first_and_no_is_first() {
        let t = texts("en-GB");
        let r = readings();
        let mut v = View::default();
        stand_on(&mut v, &r, &Target::Remove("codex-1".to_owned()), &t);
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let rows = v.rows(&r, &zone(), 100, &t);
        let answers: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| matches!(row.target, Some(Target::Answer(_))))
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            rows[answers[0]].target,
            Some(Target::Answer(false)),
            "no first"
        );
        assert_eq!(v.cursor(), answers[0], "the cursor is born on no");
        assert!(drawn(&rows).contains("Home"));
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        assert!(v.question().is_none(), "RET on no let the question go");
        stand_on(&mut v, &r, &Target::Remove("codex-1".to_owned()), &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        v.press(Press::Down, &r, &zone(), 100, &t);
        assert_eq!(
            v.press(Press::Refresh, &r, &zone(), 100, &t),
            Errand::Nothing
        );
        assert!(v.question().is_none(), "another key is no");
        stand_on(&mut v, &r, &Target::Remove("codex-1".to_owned()), &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        v.press(Press::Down, &r, &zone(), 100, &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Edit(Edit::Remove("codex-1".to_owned()))
        );
    }

    /// FAILS IF: a name, a new account or a line is not asked on the door's
    /// line, an answer to another question is taken for it, or what is
    /// typed does not become the edit.
    #[test]
    fn names_accounts_and_lines_are_typed_on_the_doors_line() {
        let t = texts("en-GB");
        let r = readings();
        let mut v = View::default();
        stand_on(&mut v, &r, &Target::Rename("claude-1".to_owned()), &t);
        let Errand::Ask(line) = v.press(Press::Enter, &r, &zone(), 100, &t) else {
            panic!("the name was not asked");
        };
        assert_eq!((line.id, line.text.as_str()), ("agents-rename", "Work"));
        assert_eq!(v.answered("agents-add-claude", "x", &t), Errand::Nothing);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        assert_eq!(
            v.answered("agents-rename", "  Office ", &t),
            Errand::Edit(Edit::Rename("claude-1".to_owned(), "Office".to_owned()))
        );
        let Errand::Ask(line) = v.command("agents-add-codex", &r, &zone(), 100, &t) else {
            panic!("a new account's name was not asked");
        };
        assert_eq!(
            v.answered(line.id, "Side", &t),
            Errand::Edit(Edit::Add(Agent::Codex, "Side".to_owned()))
        );
        v.command("agents-add-codex", &r, &zone(), 100, &t);
        assert!(matches!(
            v.answered("agents-add-codex", " ", &t),
            Errand::Say(_)
        ));
        let Errand::Ask(line) = v.command("agents-warn-at", &r, &zone(), 100, &t) else {
            panic!("the warning's line was not asked");
        };
        assert_eq!(line.text, "80");
        assert!(matches!(v.answered(line.id, "eighty", &t), Errand::Say(_)));
        v.command("agents-danger-at", &r, &zone(), 100, &t);
        v.let_go();
        assert_eq!(v.answered("agents-danger-at", "90", &t), Errand::Nothing);
        assert!(v.asking().is_none());
    }

    /// FAILS IF: the consent row is not drawn for a Claude Code account
    /// whose endpoint was not let, drawn for one that was, or its key is not
    /// taught while it is drawn.
    #[test]
    fn the_endpoint_is_asked_on_its_row_and_said_unofficial() {
        let t = texts("en-GB");
        let mut r = readings();
        let v = View::default();
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(rows.iter().any(|row| row.target == Some(Target::Let)));
        assert!(drawn(&rows).to_lowercase().contains("unofficial"));
        let hint: String = v
            .hint(&r, &zone(), 100, &t)
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        assert!(hint.contains("C-a"), "{hint}");
        r.let_usage = true;
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(!rows.iter().any(|row| row.target == Some(Target::Let)));
        r.accounts.retain(|s| s.account.kind == Agent::Codex);
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(
            !drawn(&rows).contains(usage::HOST),
            "no Claude Code account: no endpoint"
        );
    }

    /// FAILS IF: the status line's row does not offer what its state needs —
    /// give it back when ours, put ours in front otherwise — or a Codex
    /// account, which has none, draws one.
    #[test]
    fn the_status_line_turns_by_its_state() {
        let t = texts("en-GB");
        let mut r = readings();
        let mut v = View::default();
        let line = Target::StatusLine("claude-1".to_owned());
        stand_on(&mut v, &r, &line, &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Edit(Edit::Unwrap("claude-1".to_owned()))
        );
        for state in [Wrapping::Replaced, Wrapping::Not] {
            r.accounts[0].wrapping = Some(Ok(state));
            assert_eq!(
                v.press(Press::Enter, &r, &zone(), 100, &t),
                Errand::Edit(Edit::Wrap("claude-1".to_owned()))
            );
        }
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(
            !rows
                .iter()
                .any(|row| row.target == Some(Target::StatusLine("codex-1".to_owned())))
        );
    }

    /// FAILS IF: a row says nothing under the pointer that its command says
    /// in the registry, or a key of the view is not a command the registry
    /// has words for.
    #[test]
    fn every_row_explains_itself_and_every_key_has_words() {
        let t = texts("en-GB");
        let r = readings();
        let rows = View::default().rows(&r, &zone(), 100, &t);
        for (n, row) in rows.iter().enumerate() {
            if row.target.as_ref().is_some_and(|tg| tg.command().is_some()) {
                let about = View::about(&rows, n, &t).expect("an about");
                assert!(!about.starts_with("cmd-"), "{about}");
            }
        }
        for id in reached() {
            let k = format!("cmd-{id}");
            assert_ne!(t.t(&k), k, "no words for {k}");
        }
        // THE HINT'S WORD of every key it teaches.
        for (_, id, _) in OF_THE_VIEW {
            assert_ne!(t.t(id), *id, "no hint word for {id}");
        }
    }

    /// The rank of a bar's colour: none, a warning, danger.
    fn rank(colour: Option<tokens::Color>) -> u8 {
        match colour {
            None => 0,
            Some(c) if c == crate::brainlink::tone_colour(Tone::Warning) => 1,
            Some(c) if c == crate::brainlink::tone_colour(Tone::Danger) => 2,
            Some(c) => panic!("a bar in a colour no mark wears: {c:?}"),
        }
    }

    /// FAILS IF: for ONE reading, at one moment and by one pair of lines, the
    /// account's mark on the modeline — the brain module's own windows and
    /// colouring (`windows_said`, `mark_tone`) — and the most coloured of
    /// its bars in the view disagree: a bar coloured by lines of its own, a
    /// window the view draws and the mark ignores (the weekly Opus one), or
    /// one that reset counted by one side and not the other.
    #[test]
    fn a_bar_and_its_mark_wear_one_colour() {
        let r = readings();
        let claude = r.accounts[0].clone();
        let codex = r.accounts[1].clone();
        let mut quiet = claude.clone();
        quiet.limits = Err(Why::Unreadable);
        quiet.usage = None;
        // THE PHOTOGRAPH'S ACCOUNT: the five hours at 84 %, the week of Opus
        // at 96 % — danger, by the Opus bar.
        let mut opus = claude.clone();
        opus.limits = Ok(Limits {
            five_hour: Some(window(84.2, NOW + 8100)),
            seven_day: Some(window(41.0, NOW + 3 * 86_400)),
        });
        if let Some(k) = opus.usage.as_mut()
            && let Some(u) = k.usage.as_mut()
        {
            u.seven_day_opus = Some(window(96.0, NOW + 3 * 86_400));
        }
        let theirs = agent_accounts::Lines {
            warn_percent: 30,
            danger_percent: 60,
        };
        let mut cases = 0;
        let mut seen_ranks = std::collections::BTreeSet::new();
        for s in [&claude, &codex, &quiet, &opus] {
            for lines in [agent_accounts::Lines::default(), theirs] {
                for now in [NOW, NOW + 3600, NOW + 86_400, NOW + 3 * 86_400] {
                    let mark = agent_accounts::mark_tone(
                        &agent_accounts::windows_said(
                            s.limits.as_ref().ok().copied(),
                            s.usage.as_ref(),
                        ),
                        lines,
                        now,
                    );
                    let mark = rank(match mark {
                        Tone::Quiet => None,
                        tone => Some(crate::brainlink::tone_colour(tone)),
                    });
                    let bars = windows_of(s, now)
                        .into_iter()
                        .map(|(_, w)| rank(window_tint(w, lines)))
                        .max()
                        .unwrap_or(0);
                    assert_eq!(
                        bars, mark,
                        "{} at {now} by {lines:?}: the bars say {bars}, the mark {mark}",
                        s.account.id
                    );
                    seen_ranks.insert(mark);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 32);
        assert_eq!(
            seen_ranks.into_iter().collect::<Vec<_>>(),
            [0, 1, 2],
            "every colour was compared"
        );
        assert_eq!(
            agent_accounts::mark_tone(
                &agent_accounts::windows_said(opus.limits.ok(), opus.usage.as_ref()),
                agent_accounts::Lines::default(),
                NOW
            ),
            Tone::Danger,
            "the photograph's account is in danger"
        );
    }

    /// FAILS IF: the one clock is armed for a window that already reset, for
    /// a later one than the nearest shown, or at all with no window shown.
    #[test]
    fn the_one_clock_is_the_nearest_reset_shown() {
        let mut r = readings();
        assert_eq!(next_reset(&r), Some(NOW + 3600), "the five hours");
        r.now_s = NOW + 3600;
        assert_eq!(next_reset(&r), Some(NOW + 86_400), "then the week");
        r.now_s = NOW + 86_400;
        assert_eq!(next_reset(&r), None, "nothing shown: no clock");
        assert_eq!(next_reset(&Readings::default()), None);
        assert!(until(now_s() - 100) <= std::time::Duration::from_secs(1));
    }

    /// FAILS IF: the footer's clock (`Open::ring_if_due`, on the second it
    /// beats) reads again before the nearest reset shown is past, more than
    /// once for one reset, or at all with no window shown.
    #[test]
    fn the_footers_clock_reads_again_once_past_a_reset() {
        fn planted(_: &ronoctua_files::when::Zone) -> Readings {
            readings()
        }
        fn nothing(_: &ronoctua_files::when::Zone) -> Readings {
            Readings::default()
        }
        let t = texts("en-GB");
        let mut open = Open::new(
            zone(),
            Reader::Here(planted),
            Reach::Nowhere(Vec::new()),
            None,
            100,
            &t,
        );
        assert_eq!(open.next_reset(), Some(NOW + 3600));
        assert!(!open.ring_if_due(NOW + 3600), "not past yet");
        assert!(open.ring_if_due(NOW + 3601), "past the five hours");
        assert!(!open.ring_if_due(NOW + 3602), "once for one reset");
        let mut idle = Open::new(
            zone(),
            Reader::Here(nothing),
            Reach::Nowhere(Vec::new()),
            None,
            100,
            &t,
        );
        assert!(!idle.ring_if_due(i64::MAX), "nothing shown: no clock");
    }

    /// FAILS IF: a key that writes nothing makes a door read every account's
    /// files again, or one that wrote does not.
    #[test]
    fn only_a_write_reads_again() {
        let t = texts("en-GB");
        let r = readings();
        let mut v = View::default();
        v.settle_in(&r, &zone(), 100, &t);
        for p in [Press::Down, Press::Up, Press::Down] {
            assert!(!v.press(p, &r, &zone(), 100, &t).writes(), "{p:?}");
        }
        assert!(!v.press(Press::Refresh, &r, &zone(), 100, &t).writes());
        assert!(Errand::Wrote.writes());
        assert!(Errand::Edit(Edit::Remove("codex-1".to_owned())).writes());
        assert!(!Errand::Copy("x".to_owned()).writes());
        assert!(!Errand::Say("x".to_owned()).writes());
    }

    /// FAILS IF: a file whose folder is not there is not watched from the
    /// nearest folder that is — so the folder's making is heard and the
    /// file watched from then on — or a file whose folder is there is.
    #[test]
    fn a_file_waits_for_its_folder_from_the_nearest_one() {
        let dir = scratch("waiting");
        let file = dir.join("config/ronoctua/brain/agent-accounts/lines.json");
        std::fs::create_dir_all(dir.join("config/ronoctua")).unwrap();
        assert_eq!(waiting_folder(&file), Some(dir.join("config/ronoctua")));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        assert_eq!(waiting_folder(&file), None, "its folder is made: none");
        let _ = std::fs::remove_dir_all(dir);
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-shell-agents-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn roots_in(dir: &Path) -> ronoctua_llm::Roots {
        ronoctua_llm::Roots {
            config: Some(dir.join("config")),
            state: Some(dir.join("state")),
            cache: Some(dir.join("cache")),
            home: Some(dir.join("home")),
        }
    }

    fn mode_of(dir: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(dir).unwrap().permissions().mode() & 0o777
    }

    /// FAILS IF: adding an account does not make its folder `0700` before
    /// anything, does not list it, puts no status line in front of a Claude
    /// Code account (or puts one on Codex), or opens no LOGIN terminal for
    /// THAT account; or a rename, a default, a status line given back and
    /// put in front again, and a removal do not land in the files — or an
    /// account whose folder could not be made is said to be added.
    #[test]
    fn the_gestures_land_in_the_files() {
        use ronoctua_llm::statusline::{SETTINGS_FILE, wrapped};
        let dir = scratch("gestures");
        let roots = roots_in(&dir);
        std::fs::create_dir_all(dir.join("home")).unwrap();
        let t = texts("en-GB");
        let opened = std::cell::RefCell::new(Vec::new());
        let terminal = |a: &Account, login: bool| -> Result<Vec<String>, Refusal> {
            opened.borrow_mut().push((a.id.clone(), login));
            Ok(vec!["terminal".to_owned(), a.id.clone()])
        };
        let list = || {
            Accounts::store_in(roots.config.clone())
                .parsed()
                .expect("the list")
        };

        // CLAUDE CODE: the folder, the list, our status line, the login.
        let done = apply_in(
            &roots,
            &terminal,
            &Edit::Add(Agent::Claude, "Work".to_owned()),
            &t,
        );
        let work = list().accounts[0].clone();
        assert_eq!((work.kind, work.name.as_str()), (Agent::Claude, "Work"));
        assert!(
            work.dir.starts_with(dir.join("home")),
            "{}",
            work.dir.display()
        );
        assert_eq!(mode_of(&work.dir), 0o700, "the folder is not private");
        assert_eq!(wrapped(&work), Ok(Wrapping::Ours), "no status line of ours");
        assert_eq!(
            done.terminal,
            Some(vec!["terminal".to_owned(), work.id.clone()])
        );
        assert_eq!(
            opened.borrow().as_slice(),
            [(work.id.clone(), true)],
            "not its login"
        );
        assert_eq!(
            done.said,
            Some(t.t_with(key::AGENTS_ADDED, &[("name", "Work")]))
        );

        // CODEX: a folder, the list, and no status line.
        let done = apply_in(
            &roots,
            &terminal,
            &Edit::Add(Agent::Codex, "Home".to_owned()),
            &t,
        );
        let home = list().accounts[1].clone();
        assert_eq!(home.kind, Agent::Codex);
        assert_eq!(mode_of(&home.dir), 0o700);
        assert!(
            !home.dir.join(SETTINGS_FILE).exists(),
            "Codex has no status line"
        );
        assert_eq!(opened.borrow().last(), Some(&(home.id, true)));
        assert!(done.terminal.is_some());

        // NAME, DEFAULT, THE STATUS LINE BOTH WAYS.
        let side = {
            let _ = apply_in(
                &roots,
                &terminal,
                &Edit::Add(Agent::Claude, "Side".to_owned()),
                &t,
            );
            list().accounts[2].clone()
        };
        assert!(!side.default, "the first of its kind stays the default");
        let _ = apply_in(
            &roots,
            &terminal,
            &Edit::Rename(work.id.clone(), "Office".to_owned()),
            &t,
        );
        assert_eq!(
            list().find(&work.id).map(|a| a.name.clone()),
            Some("Office".to_owned())
        );
        let _ = apply_in(&roots, &terminal, &Edit::Default(side.id.clone()), &t);
        assert_eq!(
            list().default_of(Agent::Claude).map(|a| a.id.clone()),
            Some(side.id)
        );
        let _ = apply_in(&roots, &terminal, &Edit::Unwrap(work.id.clone()), &t);
        assert_ne!(wrapped(&work), Ok(Wrapping::Ours), "ours still in front");
        let _ = apply_in(&roots, &terminal, &Edit::Wrap(work.id.clone()), &t);
        assert_eq!(wrapped(&work), Ok(Wrapping::Ours));

        // REMOVED: off the list, its folder kept, its own status line back.
        let done = apply_in(&roots, &terminal, &Edit::Remove(work.id.clone()), &t);
        assert!(list().find(&work.id).is_none());
        assert!(work.dir.is_dir(), "the folder holds its sessions and login");
        assert_ne!(wrapped(&work), Ok(Wrapping::Ours));
        assert_eq!(
            done.said,
            Some(t.t_with(key::AGENTS_REMOVED, &[("name", "Office")]))
        );

        // A FOLDER THAT CANNOT BE MADE: refused, said, nothing listed or opened.
        let blocked = scratch("blocked");
        std::fs::write(blocked.join("home"), b"a file, not a home").unwrap();
        let before = opened.borrow().len();
        let done = apply_in(
            &roots_in(&blocked),
            &terminal,
            &Edit::Add(Agent::Codex, "Nowhere".to_owned()),
            &t,
        );
        assert_eq!(done.said, Some(words::refused(Why::Failed, &t)));
        assert_eq!(done.terminal, None);
        assert_eq!(opened.borrow().len(), before, "a terminal for no account");
        assert!(
            Accounts::store_in(roots_in(&blocked).config)
                .parsed()
                .is_some_and(|l| l.accounts.is_empty())
        );
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(blocked);
    }

    /// FAILS IF: a session or a login opened from anywhere opens another
    /// account than the one at the place given, or the default with none —
    /// or says nothing when there is no account, or when it cannot open.
    #[test]
    fn a_session_from_anywhere_is_the_places_or_the_defaults() {
        let t = texts("en-GB");
        let accounts: Vec<Account> = readings().accounts.into_iter().map(|s| s.account).collect();
        let terminal = |a: &Account, login: bool| -> Result<Vec<String>, Refusal> {
            Ok(vec![a.id.clone(), login.to_string()])
        };
        assert_eq!(
            from_anywhere(&accounts, Some(2), true, &terminal, &t),
            Ok(vec!["codex-1".to_owned(), "true".to_owned()])
        );
        assert_eq!(
            from_anywhere(&accounts, None, false, &terminal, &t),
            Ok(vec!["claude-1".to_owned(), "false".to_owned()])
        );
        assert_eq!(
            from_anywhere(&[], None, false, &terminal, &t),
            Err(t.t(key::AGENTS_NONE))
        );
        assert_eq!(
            from_anywhere(&accounts, Some(9), false, &terminal, &t),
            Err(t.t(key::AGENTS_NONE)),
            "no account at that place"
        );
        let refused = |_: &Account, _: bool| -> Result<Vec<String>, Refusal> {
            Err(Refusal::of(Why::NotInstalled))
        };
        assert_eq!(
            from_anywhere(&accounts, None, false, &refused, &t),
            Err(words::refused(Why::NotInstalled, &t))
        );
    }

    /// FAILS IF: a day of the count is drawn as another day — the date's
    /// arithmetic is days, and an instant is seconds — or a key the file did
    /// not write is not drawn as it is.
    #[test]
    fn a_day_reads_as_that_date() {
        let t = texts("en-GB");
        let said = day_text("2026-10-05", &t);
        for part in ["2026", "10", "05"] {
            assert!(said.contains(part), "{said}");
        }
        assert_eq!(day_text("not a day", &t), "not a day");
    }
}
