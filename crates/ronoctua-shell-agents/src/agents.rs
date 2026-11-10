//! `LEADER / a` — the person's LLM accounts (Claude Code on a subscription or
//! on an API key, Codex), in one view with two doors: the footer's region (`/ a a`, [`crate::agents_region`])
//! and a window (`/ a w`, [`crate::agents_window`]). `/ a n` opens a session of
//! an account and `/ a l` logs one in, from anywhere. Both doors hold the view
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
//! # Two columns, and a tab per account
//!
//! Side by side from `space.agents.besideFrom` columns (owner, 2026-11-09):
//! on the left what is the whole list's — adding an account, the refresh,
//! the usage endpoint, the lines a mark changes colour at — and on the right
//! the accounts, ONE at a time under a row of tabs, one per account. `TAB`
//! crosses from column to column, each keeping where its cursor was; `←` `→`
//! and a click on a tab go from account to account. A mark clicked on the
//! modeline opens its account's tab. Narrower, the two are one list, the
//! accounts first. The view is still ONE list of rows, each knowing its
//! column ([`Pane`]); only the doors lay them side by side ([`Page`]).
//!
//! # A folder before every agent
//!
//! A session and a login — a new account's included — open in a folder the
//! person chooses first, in the distro's file picker (`file_picker`, the
//! folder mode): walked to, or typed. It opens where the account's last
//! agent was (`ronoctua_llm::workspaces`), else where the picker was left.
//! A right click on an account's mark on the modeline opens it too
//! (`agents-session <n>`).
//!
//! # Per account
//!
//! Its agent and folder, its plan, its agents running now — each a row with
//! its state's glyph, its folder and its state, and its task under it in a
//! quiet line, whose `RET` goes to it
//! (`agents-show <session>`: the keyboard's way to what a click on the
//! agent's glyph on the modeline does) — the windows its plan has — five hours and
//! a week, or the week alone (Claude's Ultra) — and the weekly windows by
//! model the usage endpoint gives, as bars with their reset, its tokens by day and by model, its status line, and the rows that
//! act on it: a session, a login, its name, the default of its agent, remove.
//! An API key's account has no window and no status line of ours: in their
//! place, the one word anybody has on its credits — whether Claude Code said
//! they ran out, and since when (`ronoctua_llm::credits`).
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
// THE BRAIN MODULE'S AGENTS, by the names this crate's other files use: this
// file is the one the module's `namedBy` lets name it, and the words and the
// camera reach its agents through here.
pub use agent_accounts::{AgentSeen, Doing, Whereabouts};
use ronoctua_i18n::Arg;
use ronoctua_llm::credentials::{ApiKey, Facts};
use ronoctua_llm::statusline::{Limits, Wrapping};
use ronoctua_llm::tokens::Tally;
use ronoctua_llm::{Account, Accounts, Kind as Agent, Refusal, Why, Window, usage};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::agents_words as words;
use crate::echo::Echo;
use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::file_picker::{Hosted, Hosting};
use crate::region::Doorway;
use crate::wake::Wake;
use crate::when::now_s;

/// The keys this view teaches, and where each one applies. The hint line is
/// GENERATED from it (`chrome::hint_of_marked`).
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "agents-down", Where::Anywhere),
    ("<up>", "agents-up", Where::Anywhere),
    ("RET", "agents-enter", Where::Anywhere),
    // THE TWO COLUMNS AND THE TABS (owner, 2026-11-09): `TAB` crosses from
    // one column to the other, the arrows across go from account to account.
    ("TAB", CROSS_COMMAND, Where::Anywhere),
    ("<left>", "agents-previous-account", Where::Tabs),
    ("<right>", "agents-next-account", Where::Tabs),
    // ON AN ACCOUNT: single keys, the distro-wide standard for a list view.
    ("n", SESSION_COMMAND, Where::OnAnAccount),
    // A LOGIN ONLY WHILE THERE IS NONE, a logout only while there is one
    // (owner, 2026-11-09).
    ("l", LOGIN_COMMAND, Where::LoggedOut),
    ("o", LOGOUT_COMMAND, Where::LoggedIn),
    ("e", "agents-rename", Where::OnAnAccount),
    ("f", "agents-default", Where::OnAnAccount),
    ("d", "agents-remove", Where::OnAnAccount),
    ("r", "agents-refresh", Where::Anywhere),
];

/// The commands the view's ROWS run — every one reachable by name too
/// (`LEADER :`), where it opens the view and does the same.
pub const OF_THE_ROWS: &[&str] = &[
    SHOW_COMMAND,
    "agents-add-claude",
    "agents-add-claude-api",
    "agents-add-codex",
    "agents-status-line",
    "agents-warn-at",
    "agents-danger-at",
];

/// The command `TAB` is: from one column to the other.
pub const CROSS_COMMAND: &str = "agents-cross";

/// The command that opens the view in the footer (`LEADER / a a`) — and
/// the one a mark on the modeline runs, with the account's place.
pub const OPEN_COMMAND: &str = "agents-open";
/// A session of an account (`LEADER / a n`, `n` on one) — in a folder the
/// person chooses first; what the OTHER button on an account's mark runs
/// (the brain module's `STARTS`, one spelling: a test holds the two).
pub const SESSION_COMMAND: &str = "agents-session";
/// A login of an account (`LEADER / a l`, `l` on one) — one not logged in.
pub const LOGIN_COMMAND: &str = "agents-login";
/// A logout of an account (`o` on one logged in) — after a question.
pub const LOGOUT_COMMAND: &str = "agents-logout";
/// An agent, by its session: the window it works in brought forward, or
/// its terminal opened in one — what a click on its glyph on the modeline
/// runs (the brain module's `SHOWS`, one spelling), and its row's `RET`.
pub const SHOW_COMMAND: &str = agent_accounts::SHOWS;

/// The terminal's command that shows one of OUR terminals by its id — its
/// window brought forward and the terminal put in front of its group, or
/// the group restored in a window. The terminal's to answer; this view only
/// names it, as a region runs any command of the registry (`Doorway::run`).
pub const TERMINAL_SHOW: &str = "terminal-show";

/// The footer's command that brings forward the window of a process — the
/// one the process, or a process above it, opened: a Ghostty, an editor —
/// for an agent outside our terminals. The footer's to answer.
pub const WINDOW_OF_PROCESS: &str = "window-of-process";

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
    /// While an account is shown.
    OnAnAccount,
    /// While there are two accounts or more: a tab to go to.
    Tabs,
    /// While the account shown is logged in ([`logged_in`]).
    LoggedIn,
    /// While it is not — or that is not told.
    LoggedOut,
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
    /// Its logout, after a question.
    Logout,
    /// Its name.
    Rename,
    /// Make it the default of its agent.
    Default,
    /// Remove it.
    Remove,
    /// Count and ask again, now.
    Refresh,
    /// From one column to the other.
    Cross,
    /// The account before, its tab.
    Previous,
    /// The account after.
    Next,
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
        LOGOUT_COMMAND => Press::Logout,
        "agents-rename" => Press::Rename,
        "agents-default" => Press::Default,
        "agents-remove" => Press::Remove,
        "agents-refresh" => Press::Refresh,
        CROSS_COMMAND => Press::Cross,
        "agents-previous-account" => Press::Previous,
        "agents-next-account" => Press::Next,
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
    /// subscription's account's).
    pub wrapping: Option<Result<Wrapping, Why>>,
    /// Since when its credits ran out, as Claude Code last said — or that
    /// they have not (an API key's account's; `None` for every other).
    pub credits: Option<Result<Option<i64>, Why>>,
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
    /// The agents running in them now, as the brain module reads them
    /// (`agent_accounts::agents_now`) — the same the modeline's sets draw.
    pub agents: Vec<AgentSeen>,
    /// The lines the bars change colour at — the brain module's.
    pub lines: agent_accounts::Lines,
    /// What of `llm-accounts.json` and of the lines' file did not read, when
    /// part of either did not — each said in its own section (`partly_read`).
    pub accounts_unread: Option<crate::partly_read::PartlyRead>,
    /// The lines' file's, likewise.
    pub lines_unread: Option<crate::partly_read::PartlyRead>,
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

    /// Whether any account is a Claude Code subscription's — the usage
    /// endpoint is theirs (an API key's says nothing there).
    fn has_claude(&self) -> bool {
        self.accounts
            .iter()
            .any(|s| s.account.kind == Agent::Claude)
    }
}

/// WHETHER AN ACCOUNT IS LOGGED IN, as its facts say: `Some(true)` with a
/// login of its own — a token, or a key the Console login kept — `Some(false)`
/// never logged in, and `None` where that is not told: facts that do not
/// read, or a key the account's settings give (no login to log out of).
#[must_use]
pub fn logged_in(s: &Seen) -> Option<bool> {
    match &s.facts {
        Ok(Facts {
            api_key: Some(ApiKey::Helper | ApiKey::Settings),
            ..
        }) => None,
        Ok(_) => Some(true),
        Err(Why::NoCredentials) => Some(false),
        Err(_) => None,
    }
}

/// The account a gesture means: the one at `place` (from 1, a mark's
/// digit), or with none the default Claude Code account (on a subscription
/// or an API key: the two share one), else the default Codex one, else the
/// first.
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

    /// The rows and the page the doors draw them on ([`Page`]).
    #[must_use]
    pub fn page(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> (Vec<Row>, Page) {
        let rows = self.rows(columns, texts);
        let page = Page::of(&rows, columns);
        (rows, page)
    }

    /// An account by its id — as read, or, before the first reading is
    /// here (a mark's right click opens the view on it), as the list says.
    #[must_use]
    pub fn account(&self, id: &str) -> Option<Account> {
        self.readings
            .seen(id)
            .map(|s| s.account.clone())
            .or_else(|| match self.reader {
                Reader::Thread(_) => Accounts::store().read().find(id).cloned(),
                Reader::Here(_) => None,
            })
    }

    /// The wake of a door whose loop turns: what the file picker watches its
    /// folder through.
    #[must_use]
    pub fn wake(&self) -> Option<Wake> {
        match &self.reader {
            Reader::Thread(wake) => Some(wake.clone()),
            Reader::Here(_) => None,
        }
    }

    /// A key a door read, by its whole notation — the file picker's first,
    /// while one is up (every letter is typed into it), then the view's
    /// (`press_of_key`). Whether it was one.
    pub fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> bool {
        if self.view.picker_takes(key) {
            let errand =
                self.view
                    .pick_key(key, &self.readings, &self.zone, way.columns(), way.texts());
            way.follow();
            carry_out(errand, self, way);
            return true;
        }
        match press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                true
            }
            None => false,
        }
    }

    /// The clipboard's text, read off the loop for the file picker's `C-v`.
    pub fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let errand =
            self.view
                .picker_pasted(text, &self.readings, &self.zone, way.columns(), way.texts());
        carry_out(errand, self, way);
    }

    /// `agents-session [n]` / `agents-login [n]`: a folder chosen for the
    /// account at place `n` — the view open on it, or opened for it (a mark's
    /// right click) — or, with no place, for the account shown; and, before
    /// the first reading is here (`LEADER / a n` with the view closed), for
    /// the one a gesture with no row means ([`chosen_of`]), as the list says.
    pub fn choose_at(&mut self, place: Option<usize>, login: bool, way: &mut dyn Doorway) {
        if place.is_none() && self.readings.read {
            self.press(if login { Press::Login } else { Press::Session }, way);
            return;
        }
        // THE LIST AS READ — or, while a door's thread has not read it yet,
        // as the file says it now (a small file, on the loop, as a gesture
        // reads it); a photograph's has only what it planted.
        let accounts: Vec<Account> = match self.reader {
            Reader::Thread(_) if !self.readings.read => Accounts::store().read().accounts,
            _ => self
                .readings
                .accounts
                .iter()
                .map(|s| s.account.clone())
                .collect(),
        };
        let errand = match chosen_of(&accounts, place) {
            Some(account) => {
                self.view.show(&account.id);
                Errand::Choose(account.id, login)
            }
            None => Errand::Say(way.texts().t(key::AGENTS_NONE).into()),
        };
        carry_out(errand, self, way);
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
        // THE FILE PICKER'S FOLDER, heard through the same wake.
        if self.view.look() {
            self.settle(way.columns(), way.texts());
            way.repaint();
        }
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
                Heard::LoggedOut(name, done) => {
                    let said = logged_out_said(&name, done, way.texts());
                    way.say(said, Echo::Long);
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

    /// A click on row `n` of the view: the cursor there, and the second
    /// click is `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, way: &mut dyn Doorway) {
        let rows = self.rows(way.columns(), way.texts());
        self.clicked_on(n, None, &rows, way);
    }

    /// A click on LINE `line` of the page the doors draw, at cell `cell`:
    /// the row of the view there ([`Page::row_at`]) — a tab shows its
    /// account at once, every other row is selected by the first click and
    /// acted on by the second.
    pub fn clicked_at(&mut self, line: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let (rows, page) = self.page(way.columns(), way.texts());
        if let Some(n) = page.row_at(line, cell) {
            self.clicked_on(n, cell, &rows, way);
        }
    }

    /// The click, on row `n`.
    fn clicked_on(&mut self, n: usize, cell: Option<u16>, rows: &[Row], way: &mut dyn Doorway) {
        if self.view.clicked_at(n, cell, rows) {
            self.press(Press::Enter, way);
        } else {
            self.settle(way.columns(), way.texts());
            way.repaint();
        }
    }

    /// A command of the view by its name — `LEADER :`, a row's, a hint's —
    /// or, with `argument`, the answer to the view's own question.
    pub fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) {
        // AN ANSWER ONLY TO THE VIEW'S OWN QUESTION.
        let answering = self.view.asking().is_some_and(|a| a.command() == id);
        // AN AGENT NAMED BY ITS SESSION — a glyph on the modeline clicked
        // with the view open: that agent, whatever row is under the cursor.
        let session = argument
            .map(str::trim)
            .filter(|s| id == SHOW_COMMAND && !s.is_empty());
        let errand = match (argument.filter(|_| answering), session) {
            (Some(said), _) => self.view.answered(id, said, way.texts()),
            (None, Some(session)) => Errand::Show(session.to_owned()),
            (None, None) => {
                self.view
                    .command(id, &self.readings, &self.zone, way.columns(), way.texts())
            }
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

/// Opens the file picker for a session (`false`) or the login (`true`) of
/// `account` — once the machine says a terminal would open for it (the
/// agent installed, the account one): a folder chosen for an agent that
/// cannot start would be a question asked for nothing. It opens where the
/// account's last agent was, else where the picker was left. A photograph's
/// asks the machine nothing, and remembers nothing.
fn choose(open: &mut Open, account: &Account, login: bool, way: &mut dyn Doorway) {
    let start = match &open.reach {
        Reach::Machine => {
            // A LOGIN OF AN ACCOUNT LOGGED IN makes no sense (owner,
            // 2026-11-09): said, and nothing opened — `/ a l` from anywhere.
            if login && ronoctua_llm::logged_in(account) == Some(true) {
                let said = way.texts().t(key::AGENTS_ALREADY_LOGGED_IN);
                way.say(said, Echo::Long);
                return;
            }
            if let Err(r) = terminal_of(account, login, None) {
                let said = words::refused(r.why, way.texts());
                way.say(said, Echo::Long);
                return;
            }
            ronoctua_llm::workspaces::Workspaces::last_in(
                &ronoctua_llm::Roots::of_env(),
                &account.id,
            )
        }
        Reach::Nowhere(_) => None,
    };
    let wake = open.wake();
    open.view.choose(account, login, start, wake.as_ref());
    open.settle(way.columns(), way.texts());
}

/// LOGS AN ACCOUNT OUT, by its agent's own logout, off the loop (the
/// agent's program runs and is waited for): what came of it is said, and
/// the view reads again — the watches hear the login file go too. A
/// photograph's writes down what would have happened.
fn log_out(open: &mut Open, id: &str, way: &mut dyn Doorway) {
    let Some(account) = open.account(id) else {
        let said = way.texts().t(key::AGENTS_NONE);
        way.say(said, Echo::Normal);
        return;
    };
    match (&mut open.reach, &open.reader) {
        (Reach::Nowhere(done), _) => done.push(format!("{LOGOUT_COMMAND} {id}")),
        (Reach::Machine, Reader::Thread(wake)) => {
            let (inbox, wake) = (Arc::clone(&open.inbox), wake.clone());
            let spawned = std::thread::Builder::new()
                .name("agents-logout".to_owned())
                .spawn(move || {
                    let done = ronoctua_llm::logout(&account).map_err(|r| r.why);
                    leave(&inbox, &wake, Heard::LoggedOut(account.name, done));
                });
            if let Err(e) = spawned {
                tracing::warn!(error = %e, "the account could not be logged out");
                let said = words::refused(Why::Failed, way.texts());
                way.say(said, Echo::Long);
            }
        }
        (Reach::Machine, Reader::Here(_)) => {
            let done = ronoctua_llm::logout(&account).map_err(|r| r.why);
            let said = logged_out_said(&account.name, done, way.texts());
            way.say(said, Echo::Long);
        }
    }
}

/// What a logout came to, as the door's line says it — the account's name
/// told (`Arg::Name`).
fn logged_out_said(
    name: &str,
    done: Result<(), Why>,
    texts: &ronoctua_i18n::Texts,
) -> crate::chrome::Named {
    match done {
        Ok(()) => texts
            .t_args_marked(key::AGENTS_LOGGED_OUT, &[("name", Arg::Name(name))])
            .into(),
        Err(why) => words::refused(why, texts).into(),
    }
}

/// THE FOLDER CHOSEN: remembered for the account — its next agent is
/// offered there — and its session or its login opened in it, in the
/// terminal (the door gives the keyboard up for it first: the footer's
/// region closes, the launcher's rule); or, a photograph's, written down.
fn start(open: &mut Open, id: &str, login: bool, dir: &Path, way: &mut dyn Doorway) {
    let Some(account) = open.account(id) else {
        let said = way.texts().t(key::AGENTS_NONE);
        way.say(said, Echo::Normal);
        return;
    };
    match &mut open.reach {
        Reach::Machine => {
            // A FOLDER WHOSE NAME IS NOT TEXT cannot cross to the terminal
            // as an argument: said, and nothing opened in the wrong place.
            let opened = dir
                .to_str()
                .ok_or(Refusal::of(Why::Invalid))
                .and_then(|_| terminal_of(&account, login, Some(dir)));
            match opened {
                Ok(words) => {
                    if let Err(r) = ronoctua_llm::workspaces::Workspaces::remember_in(
                        &ronoctua_llm::Roots::of_env(),
                        id,
                        dir,
                    ) {
                        tracing::warn!(why = ?r.why, "the agent's folder is not remembered");
                    }
                    way.step_aside();
                    way.open_window(words);
                }
                Err(r) => {
                    let said = words::refused(r.why, way.texts());
                    way.say(said, Echo::Long);
                }
            }
        }
        Reach::Nowhere(done) => done.push(format!(
            "{} {id} {}",
            if login {
                LOGIN_COMMAND
            } else {
                SESSION_COMMAND
            },
            dir.display()
        )),
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
            // A NEW ACCOUNT'S LOGIN, in a folder the person chooses first.
            if let Some(account) = applied.login {
                choose(open, &account, true, way);
            }
        }
        Errand::Choose(id, login) => match open.account(&id) {
            Some(account) => choose(open, &account, login, way),
            None => {
                let said = way.texts().t(key::AGENTS_NONE);
                way.say(said, Echo::Normal);
            }
        },
        Errand::Start {
            account,
            login,
            dir,
        } => start(open, &account, login, &dir, way),
        Errand::Paste => way.paste(),
        Errand::Refresh => {
            let said = way.texts().t(key::AGENTS_REFRESHING);
            way.say(said, Echo::Normal);
            open.refresh();
        }
        Errand::Logout(id) => log_out(open, &id, way),
        Errand::Show(session) => match &mut open.reach {
            Reach::Machine => show_agent(&session, way),
            Reach::Nowhere(done) => done.push(format!("{SHOW_COMMAND} {session}")),
        },
    }
    // WHAT THIS DOOR WROTE, read back at once — the watches ring for it too,
    // and find nothing new.
    if wrote {
        open.reread_after_a_write(way.columns(), way.texts());
    }
    way.repaint();
}

/// `agents-show <session>` — a click on an agent's glyph on the modeline, the
/// `RET` of its row: the agent read NOW by the module's own reader
/// (`agent_accounts::agent_of`), so the click goes where it is, not where it
/// was when its mark was drawn. On the door's loop: the folders read are the
/// runtime's (a tmpfs), a few small files.
pub fn show_agent(session: &str, way: &mut dyn Doorway) {
    show_found(
        agent_accounts::agent_of(session).and_then(|a| a.whereabouts()),
        way,
    );
}

/// Goes where an agent was found: the door steps aside (the footer's region
/// would keep the keyboard over the window that comes forward) and the
/// command that shows it runs — or, for an agent that ended, the sentence.
pub fn show_found(found: Option<Whereabouts>, way: &mut dyn Doorway) {
    match found {
        Some(at) => {
            let (command, argument) = order_of(&at);
            way.step_aside();
            way.run(command, Some(argument));
        }
        None => {
            let said = way.texts().t(key::AGENTS_AGENT_ENDED);
            way.say(said, Echo::Long);
        }
    }
}

/// The command, and its argument, that shows an agent where it is: our
/// terminal by its id, any other window by the process.
#[must_use]
pub fn order_of(at: &Whereabouts) -> (&'static str, String) {
    match at {
        Whereabouts::Terminal(id) => (TERMINAL_SHOW, id.clone()),
        Whereabouts::Process(pid) => (WINDOW_OF_PROCESS, pid.to_string()),
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

/// Reads everything, as it is now. BLOCKS on files — a credentials file (or
/// an API key's global configuration), the kept numbers, a token count, a
/// Codex account's newest logs: a door calls it on a thread.
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
        // THE TWO FILES JUDGED beside the reads that say nothing: what of
        // each did not read, when part of it did not.
        accounts_unread: crate::partly_read::judged(&Accounts::store()),
        lines_unread: crate::partly_read::judged(&agent_accounts::lines_store(
            agent_accounts::lines_file().and_then(|f| f.parent().map(std::path::Path::to_path_buf)),
        )),
        ..Readings::default()
    };
    let Some(list) = Accounts::store().parsed() else {
        r.refused = true;
        return r;
    };
    // THE AGENTS, by the module's own reader: the hook's sessions and what
    // our terminals heard, merged once — never a second reading of either.
    r.agents = agent_accounts::agents_now(&list.accounts);
    r.accounts = list
        .accounts
        .into_iter()
        .map(|account| {
            let claude = account.kind == Agent::Claude;
            let cache = ronoctua_llm::tokens::kept(&account);
            Seen {
                facts: ronoctua_llm::credentials::facts_of(&account).map_err(|r| r.why),
                limits: ronoctua_llm::statusline::limits(&account).map_err(|r| r.why),
                usage: claude.then(|| usage::kept(&account)).flatten(),
                tally: cache.tally_in(&|at| zone.offset_at(at)),
                wrapping: claude
                    .then(|| ronoctua_llm::statusline::wrapped(&account).map_err(|r| r.why)),
                credits: (account.kind == Agent::ClaudeApi)
                    .then(|| ronoctua_llm::credits::out_since(&account).map_err(|r| r.why)),
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
    // THE AGENTS' TWO FOLDERS, alone: a session the hook wrote, a state a
    // terminal's holder wrote.
    for folder in [
        agent_accounts::sessions_folder(),
        agent_accounts::terminals_folder(),
    ]
    .into_iter()
    .flatten()
    {
        watch_folder(folder, &mut w);
    }
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

/// A folder watched alone — or, while it is not there yet, the nearest one
/// above it that is, to hear it being made: the reading after its making
/// covers a different `Watching`, and the doors raise their watches again.
fn watch_folder(folder: PathBuf, w: &mut Watching) {
    if folder.is_dir() {
        w.dirs.push(folder);
    } else if let Some(above) = folder.ancestors().skip(1).find(|a| a.is_dir()) {
        w.ancestors.push(above.to_path_buf());
    }
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
    /// A logout, done — the account's name, and how it went.
    LoggedOut(String, Result<(), Why>),
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
            &[("id", id), ("why", &words::clause(why, texts))],
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
    /// An API key's credits: run out, or not.
    Credits,
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
    /// Its login, in the terminal — drawn only while it has none.
    Login(String),
    /// Its logout: `RET` asks first, "no" drawn first — drawn only while it
    /// has a login.
    Logout(String),
    /// Its name: `RET` asks it.
    Rename(String),
    /// The default of its agent: `RET` makes it so.
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
    /// An agent running in an account — its account's id and its session:
    /// `RET` goes to it.
    Agent(String, String),
    /// The warning's line: `RET` asks it.
    Warn,
    /// The danger's line: `RET` asks it.
    Danger,
    /// An answer to the question: yes, or no.
    Answer(bool),
    /// An account's tab: a click shows it.
    Tab(String),
    /// A row of the file picker, while a folder is chosen.
    Pick(crate::file_picker::Target),
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
            | Self::Logout(id)
            | Self::Rename(id)
            | Self::Default(id)
            | Self::Remove(id)
            | Self::Tab(id)
            | Self::Agent(id, _) => Some(id),
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
            Self::Logout(_) => LOGOUT_COMMAND,
            Self::Rename(_) => "agents-rename",
            Self::Default(_) => "agents-default",
            Self::Remove(_) => "agents-remove",
            Self::Add(Agent::Claude) => "agents-add-claude",
            Self::Add(Agent::ClaudeApi) => "agents-add-claude-api",
            Self::Add(Agent::Codex) => "agents-add-codex",
            Self::Refresh => "agents-refresh",
            Self::Agent(..) => SHOW_COMMAND,
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
            Self::Tab(_) => Some(texts.t(key::ABOUT_AGENTS_TAB)),
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
    /// The line CONTINUES the row above it: the rest of its label or value,
    /// wrapped (`chrome::aligned_pair`). The cursor skips it, but it is a
    /// line of that row's control — it lights with it, and a click on it
    /// takes the same thing ([`View::control_of`]).
    pub continues: bool,
    /// Which column it stands in.
    pub pane: Pane,
    /// STRETCHES OF IT THAT ARE CONTROLS OF THEIR OWN — the tabs: the cells
    /// `[from, to)` and what a click there takes. The cursor skips such a
    /// row (the arrows across reach the tabs); the pointer lights the
    /// stretch under it.
    pub zones: Vec<(u16, u16, Target)>,
}

impl Row {
    /// A line of the column `pane`.
    #[must_use]
    pub const fn of(
        fields: Vec<Field>,
        target: Option<Target>,
        continues: bool,
        pane: Pane,
    ) -> Self {
        Self {
            fields,
            target,
            continues,
            pane,
            zones: Vec::new(),
        }
    }

    /// Whether the cursor skips it.
    #[must_use]
    pub const fn heading(&self) -> bool {
        self.target.is_none()
    }

    /// Whether a click takes it — a row the cursor stands on, a line that
    /// continues one, a row of tabs — and so whether a resting pointer
    /// lights it: ONE ANSWER for both doors' light and click (the region's
    /// `Row::takes`, the window's `Content::takes`, [`View::clicked`]).
    #[must_use]
    pub fn takes(&self) -> bool {
        !self.heading() || self.continues || !self.zones.is_empty()
    }

    /// The stretch of it at `cell`, when it is a row of stretches.
    #[must_use]
    pub fn zone_at(&self, cell: u16) -> Option<&(u16, u16, Target)> {
        self.zones
            .iter()
            .find(|(from, to, _)| (*from..*to).contains(&cell))
    }
}

/// Which column a row stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// The accounts — the right column, one account under its tabs.
    Accounts,
    /// What is the whole list's — the left column.
    Options,
    /// The whole width: a question, the file picker, the first reading.
    Whole,
}

/// Where the accounts' column starts, when the two stand side by side at
/// `columns` (`space.agents.besideFrom`).
#[must_use]
pub fn split_of(columns: u16) -> Option<u16> {
    (columns >= tokens::space::AGENTS_BESIDEFROM)
        .then(|| columns * tokens::space::AGENTS_OPTIONSPCT / 100)
}

/// Where a column's rows are drawn at `columns`: side by side, the options
/// from the left edge to the split (with the modeline's air before it) and
/// the accounts from the split to the right edge; stacked, or the whole
/// width, the door's whole extent.
#[must_use]
pub fn extent_of(pane: Pane, columns: u16) -> (u16, u16) {
    let (left, room) = crate::chrome::extent(columns);
    let Some(split) = split_of(columns).filter(|_| pane != Pane::Whole) else {
        return (left, room);
    };
    let end = left + room;
    match pane {
        Pane::Options => (
            left,
            split.saturating_sub(left + tokens::space::MODELINE_SEGMENTGAP),
        ),
        Pane::Accounts | Pane::Whole => (split, end.saturating_sub(split)),
    }
}

/// THE ROWS AS THE DOORS DRAW THEM — the two columns side by side, the
/// options on the left and the accounts on the right, line by line; or the
/// rows one under the other, when they are one list (stacked, a question,
/// the picker). Both doors read light, click and cursor through it, so the
/// line a pointer is on and the row the view acts on are one answer.
#[derive(Debug, Clone, Default)]
pub struct Page {
    /// Each line's fields, left to right.
    pub lines: Vec<Vec<Field>>,
    /// Each line's rows of the view: the cell each stands from, and its index.
    at: Vec<Vec<(u16, usize)>>,
    /// Where the accounts' column starts, side by side.
    pub split: Option<u16>,
    /// The width it was laid out at.
    columns: u16,
}

impl Page {
    /// The page of `rows` at `columns`: side by side when the width affords
    /// it and the rows are of both columns.
    #[must_use]
    pub fn of(rows: &[Row], columns: u16) -> Self {
        let split = split_of(columns).filter(|_| rows.iter().any(|r| r.pane == Pane::Options));
        let Some(split) = split else {
            return Self {
                lines: rows.iter().map(|r| r.fields.clone()).collect(),
                at: (0..rows.len()).map(|n| vec![(0, n)]).collect(),
                split: None,
                columns,
            };
        };
        let of = |pane: Pane| -> Vec<usize> {
            (0..rows.len()).filter(|n| rows[*n].pane == pane).collect()
        };
        let (options, accounts) = (of(Pane::Options), of(Pane::Accounts));
        let mut page = Self {
            split: Some(split),
            columns,
            ..Self::default()
        };
        for line in 0..options.len().max(accounts.len()) {
            let mut fields = Vec::new();
            let mut at = Vec::new();
            for (from, column) in [(0, &options), (split, &accounts)] {
                if let Some(&n) = column.get(line) {
                    fields.extend(rows[n].fields.iter().cloned());
                    at.push((from, n));
                }
            }
            page.lines.push(fields);
            page.at.push(at);
        }
        page
    }

    /// The row of the view on line `line` at cell `cell` — with no cell,
    /// the line's only one (none where two stand side by side).
    #[must_use]
    pub fn row_at(&self, line: usize, cell: Option<u16>) -> Option<usize> {
        self.stretch(line, cell).map(|(_, _, n)| n)
    }

    /// The stretch `[from, to)` of line `line` the row at `cell` stands in,
    /// and the row.
    #[must_use]
    pub fn stretch(&self, line: usize, cell: Option<u16>) -> Option<(u16, u16, usize)> {
        let here = self.at.get(line)?;
        let Some(split) = self.split else {
            return here.first().map(|&(_, n)| (0, self.columns, n));
        };
        let half = match cell {
            Some(c) if c >= split => (split, self.columns),
            Some(_) => (0, split),
            None if here.len() == 1 => {
                if here[0].0 == 0 {
                    (0, split)
                } else {
                    (split, self.columns)
                }
            }
            None => return None,
        };
        here.iter()
            .find(|(from, _)| *from == half.0)
            .map(|&(_, n)| (half.0, half.1, n))
    }

    /// The line row `row` of the view is drawn on.
    #[must_use]
    pub fn line_of(&self, row: usize) -> Option<usize> {
        self.at
            .iter()
            .position(|here| here.iter().any(|(_, n)| *n == row))
    }

    /// Whether a click anywhere on line `line` takes something.
    #[must_use]
    pub fn takes(&self, line: usize, rows: &[Row]) -> bool {
        self.at
            .get(line)
            .is_some_and(|here| here.iter().any(|(_, n)| rows[*n].takes()))
    }

    /// WHAT THE POINTER AT `cell` OF LINE `line` LIGHTS — the stretch a
    /// click there takes: a tab, the half of a line side by side, the whole
    /// line in one list; nothing where a click takes nothing. The footer's
    /// region answers with it, and the window lays its bands by it.
    #[must_use]
    pub fn lit(&self, line: usize, cell: Option<u16>, rows: &[Row]) -> crate::region::Lit {
        use crate::region::Lit;
        let Some((from, to, n)) = self.stretch(line, cell) else {
            return Lit::Nothing;
        };
        let row = &rows[n];
        if !row.takes() {
            return Lit::Nothing;
        }
        if !row.zones.is_empty() {
            return cell
                .and_then(|c| row.zone_at(c))
                .map_or(Lit::Nothing, |(f, t, _)| Lit::Cells(*f, *t));
        }
        if self.split.is_some() {
            Lit::Cells(from, to)
        } else {
            Lit::Row
        }
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
    /// A sentence under the row before it, wrapped — the names it quotes in
    /// a name's ink.
    Note(crate::chrome::Named),
    /// A QUIET LINE OF THE ROW BEFORE IT, `indent` cells in — an agent's
    /// task under its folder: part of that row's control, lit and clicked
    /// with it.
    Under {
        /// What it says.
        text: String,
        /// How far in from the column's edge.
        indent: u16,
    },
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
fn note(text: impl Into<crate::chrome::Named>) -> Entry {
    Entry::Note(text.into())
}

/// The rows, as they are built — a column at a time.
struct Builder<'a> {
    out: Vec<Row>,
    cursor: usize,
    left: u16,
    room: u16,
    texts: &'a ronoctua_i18n::Texts,
    /// The column being built.
    pane: Pane,
    /// Where its rows start in [`Builder::out`].
    start: usize,
}

impl Builder<'_> {
    /// The rows that follow are column `pane`'s, at its extent — its first
    /// title with no air before it when the columns stand side by side, and
    /// after the air every title has when they are one list.
    fn column(&mut self, pane: Pane, columns: u16) {
        (self.left, self.room) = extent_of(pane, columns);
        self.pane = pane;
        self.start = if split_of(columns).is_some() {
            self.out.len()
        } else {
            0
        };
    }

    /// A line of the column being built.
    fn push(&mut self, fields: Vec<Field>, target: Option<Target>, continues: bool) {
        self.out.push(Row::of(fields, target, continues, self.pane));
    }

    /// A section: its title, and its entries laid out whole — the values of
    /// every row in one column after the widest label (`section_column`),
    /// a label that does not fit wrapped under itself.
    fn section(&mut self, title: &str, entries: Vec<Entry>) {
        // AIR BEFORE EVERY TITLE but the column's first — `chrome::AIR`.
        if self.out.len() > self.start {
            self.push(crate::chrome::AIR, None, false);
        }
        let heading = vec![piece(
            self.left,
            title.to_owned(),
            tokens::color::TEXT_HEADING,
            self.room,
        )];
        self.push(heading, None, false);
        self.entries(entries);
    }

    /// Entries with no title of their own — an account's, under its tabs.
    fn entries(&mut self, entries: Vec<Entry>) {
        let end = self.left + self.room;
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
                        self.push(fields, (n == 0).then(|| target.clone()), n > 0);
                    }
                }
                Entry::Note(text) => {
                    for note in crate::chrome::note_fields(
                        self.left,
                        &text,
                        tokens::color::TEXT_DIM,
                        self.room,
                    ) {
                        self.push(note, None, false);
                    }
                }
                Entry::Under { text, indent } => {
                    let room = self.room.saturating_sub(indent);
                    let under = vec![piece(
                        self.left + indent,
                        text,
                        tokens::color::TEXT_DIM,
                        room,
                    )];
                    self.push(under, None, true);
                }
                Entry::Air => self.push(crate::chrome::AIR, None, false),
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
///
/// THE FIVE HOURS ARE DRAWN while they are known, or while nothing says the
/// plan lacks them: a fresh account still reads "not known yet", and an
/// Ultra one (the week alone, said by its status line or the endpoint) has
/// no row that would say so forever. An API key has no window at all, and
/// none is drawn: every row would say "not known yet" forever.
#[must_use]
pub fn windows_of(s: &Seen, now_s: i64) -> Vec<(Span, Option<Window>)> {
    if !s.account.kind.has_windows() {
        return Vec::new();
    }
    let limits = s.limits.as_ref().ok().copied().unwrap_or_default();
    let usage = s.usage.and_then(|k| k.usage).unwrap_or_default();
    let five = merged(limits.five_hour, usage.five_hour, now_s);
    let lacks_five = limits.no_five_hour || usage.no_five_hour;
    let mut out = Vec::new();
    if five.is_some() || !lacks_five {
        out.push((Span::FiveHours, five));
    }
    out.push((Span::Week, merged(limits.seven_day, usage.seven_day, now_s)));
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

/// The colour an API key's credits wear: the brain module's, the mark's —
/// danger while they are out, and `None` otherwise.
#[must_use]
pub fn credits_tint(out_since: Option<i64>) -> Option<tokens::Color> {
    match agent_accounts::credits_tone(out_since) {
        Tone::Quiet => None,
        tone => Some(crate::brainlink::tone_colour(tone)),
    }
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
    /// Log this account out — its id and its name.
    Logout(String, String),
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
            Self::New(Agent::ClaudeApi) => "agents-add-claude-api",
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
    /// An account made the default of its agent.
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
    /// Say this sentence on the door's line — with the names it quotes told.
    Say(crate::chrome::Named),
    /// Put this on the clipboard.
    Copy(String),
    /// Change the list.
    Edit(Edit),
    /// Choose the folder a session (`false`) or the login (`true`) of this
    /// account opens in — the file picker, over the view.
    Choose(String, bool),
    /// The folder chosen: a session or the login of the account, in the
    /// terminal, there — and the folder remembered for its next agent.
    Start {
        /// The account's id.
        account: String,
        /// Its login, and not a session.
        login: bool,
        /// The folder.
        dir: PathBuf,
    },
    /// The clipboard, for the file picker's `C-v` (`Doorway::paste`).
    Paste,
    /// Count and ask again, off the loop.
    Refresh,
    /// Go to the agent of this session ([`show_agent`]).
    Show(String),
    /// Log this account out, off the loop ([`ronoctua_llm::logout`]).
    Logout(String),
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
/// account whose login opens next — a new account's, in a folder the person
/// chooses first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Applied {
    /// The sentence, with the account's name told.
    pub said: Option<crate::chrome::Named>,
    /// The new account, whose login the folder is chosen for.
    pub login: Option<Account>,
}

/// Applies an edit to the list, as the file says it NOW
/// (`Accounts::keep`), and says what came of it.
#[must_use]
pub fn apply(edit: &Edit, texts: &ronoctua_i18n::Texts) -> Applied {
    apply_in(&ronoctua_llm::Roots::of_env(), edit, texts)
}

/// [`apply`], under given roots — what a test drives, over a folder of its
/// own.
#[must_use]
pub fn apply_in(roots: &ronoctua_llm::Roots, edit: &Edit, texts: &ronoctua_i18n::Texts) -> Applied {
    // THE ACCOUNT'S NAME IS A NAME the sentence quotes (`Arg::Name`).
    let said = |k: &str, name: &str| -> crate::chrome::Named {
        texts.t_args_marked(k, &[("name", Arg::Name(name))]).into()
    };
    let refused = |r: Refusal| Applied {
        said: Some(words::refused(r.why, texts).into()),
        login: None,
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
                let mut sentence = said(key::AGENTS_ADDED, &account.name);
                if let Err(why) = wrapped {
                    sentence = sentence.then(" ", words::refused(why, texts));
                }
                Applied {
                    said: Some(sentence),
                    login: Some(account),
                }
            }
            Err(r) => refused(r),
        },
        Edit::Rename(id, name) => match Accounts::keep_in(roots, |l| l.rename(id, name)) {
            Ok(_) => Applied {
                said: Some(said(key::AGENTS_RENAMED, name.trim())),
                login: None,
            },
            Err(r) => refused(r),
        },
        Edit::Remove(id) => {
            let name = name_of(id);
            match Accounts::keep_in(roots, |l| l.remove(id)) {
                Ok(_) => Applied {
                    said: Some(said(key::AGENTS_REMOVED, &name)),
                    login: None,
                },
                Err(r) => refused(r),
            }
        }
        Edit::Default(id) => match Accounts::keep_in(roots, |l| l.set_default(id)) {
            Ok(_) => Applied {
                said: Some(said(key::AGENTS_MADE_DEFAULT, &name_of(id))),
                login: None,
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
                    said: Some(said(
                        if wrap {
                            key::AGENTS_WRAPPED
                        } else {
                            key::AGENTS_UNWRAPPED
                        },
                        &account.name,
                    )),
                    login: None,
                },
                Err(r) => refused(r),
            }
        }
    }
}

/// A new account: its folder made first (`0700`, the agent's own mode for
/// its credentials' folder) — so the watches hear it from the start, and
/// Codex, which wants its folder to exist, finds it — then added to the list
/// as the file says it now, and, for a Claude Code subscription, our status
/// line put in front: the limits come from it. An API key's has no window
/// for a status line to tell, and its `settings.json` is left as it is.
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
/// login, in the folder `cwd` when one was chosen: the crate's own command
/// line (`launch_arguments`, `login_arguments`) — the agent by the store
/// path the session handed over, its folder by its variable — in our
/// terminal. The account's folder is made first: Codex refuses one that is
/// not there.
///
/// # Errors
///
/// The crate's: the agent is not installed, the account is not one.
pub fn terminal_of(
    account: &Account,
    login: bool,
    cwd: Option<&Path>,
) -> Result<Vec<String>, Refusal> {
    let argv = if login {
        ronoctua_llm::login_arguments(account)?
    } else {
        ronoctua_llm::launch_arguments(account)?
    };
    made_private(&account.dir)?;
    Ok(terminal_words(argv, cwd))
}

/// `terminal [--cwd D] -- argv…`: the agent's command line, in a folder.
/// `--` before the program, whose path could be taken for a flag of the
/// terminal's.
#[must_use]
pub fn terminal_words(argv: Vec<String>, cwd: Option<&Path>) -> Vec<String> {
    let mut line = vec!["terminal".to_owned()];
    if let Some(dir) = cwd {
        line.extend(["--cwd".to_owned(), crate::region::path_argument(dir)]);
    }
    line.push("--".to_owned());
    line.extend(argv);
    line
}

/// The agents a photograph plants (`--agents <file>`), read as the brain
/// module reads them.
///
/// # Errors
///
/// The reason, in English, when the text is not a list of them.
pub fn planted_agents(text: &str) -> Result<Vec<AgentSeen>, String> {
    agent_accounts::planted(text)
}

/// The marks the accounts' brain module would put on the modeline now,
/// read from the files under this process's roots, laid over a mind in
/// place of any of the module's it had — what the footer's camera draws
/// (`--accounts-marks`), so a picture's marks are the module's own and
/// never written by hand. With `agents`, the text of a list of them
/// (`--agents`), the agents are planted as data and the module draws them in
/// their accounts' sets (`agent_accounts::marks_read_with`).
///
/// # Errors
///
/// The planted agents do not read as the module reads them.
pub fn with_accounts_marks(
    mind: &mut ronoctua_brain::Mind,
    now: i64,
    agents: Option<&str>,
) -> anyhow::Result<()> {
    let marks = match agents {
        Some(text) => agent_accounts::marks_read_with(
            now,
            &agent_accounts::planted(text).map_err(anyhow::Error::msg)?,
        ),
        None => agent_accounts::marks_read_now(now),
    };
    mind.marks.retain(|m| m.module != agent_accounts::ID);
    mind.marks
        .extend(marks.into_iter().map(|what| ronoctua_brain::mind::Posted {
            module: agent_accounts::ID.to_owned(),
            what,
        }));
    Ok(())
}

/// The line of the door's question, for a door that asks it — the
/// account's name told (`Arg::Name`), so the page draws it in a name's ink.
#[must_use]
pub fn question_text(q: &Question, texts: &ronoctua_i18n::Texts) -> crate::chrome::Named {
    let (k, name) = match q {
        // A CLAUDE CODE SUBSCRIPTION gives its own status line back as it
        // goes; Codex and an API key have none of ours, and are not told
        // they do.
        Question::Remove(_, name, Agent::Claude) => (key::AGENTS_ASK_REMOVE_CLAUDE, name),
        Question::Remove(_, name, Agent::ClaudeApi | Agent::Codex) => {
            (key::AGENTS_ASK_REMOVE, name)
        }
        Question::Logout(_, name) => (key::AGENTS_ASK_LOGOUT, name),
    };
    texts.t_args_marked(k, &[("name", Arg::Name(name))]).into()
}

/// What a folder is being chosen for: a session or the login of an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picking {
    /// The account's id.
    pub account: String,
    /// Its name, for the page's title.
    pub name: String,
    /// Its login, and not a session.
    pub login: bool,
}

/// What the file picker remembers its folder under, when the account has
/// none of its own yet (`file-picker.json`).
pub const PICKER_PURPOSE: &str = "agents-workspace";

/// The view: where the cursor is, what is asked, what it is aimed at, which
/// account is shown and whether a folder is being chosen. Everything it
/// DRAWS is read from the [`Readings`] each door hands it.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    asking: Option<Ask>,
    question: Option<Question>,
    /// The answer the question's cursor is on.
    yes: bool,
    /// The row the cursor is on, by what it is — kept across a rebuild.
    aim: Option<Target>,
    /// THE ACCOUNT SHOWN, by its id — its tab. `None` (or an id the list no
    /// longer has) shows the first.
    tab: Option<String>,
    /// Where the cursor was in the OTHER column, by what it was on — where
    /// `TAB` takes it back (each column keeps its own place).
    kept: Option<Target>,
    /// The file picker, over the view, while a folder is chosen for an
    /// agent.
    picking: Option<Hosted<Picking>>,
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

    /// What a folder is being chosen for, while the file picker is up.
    #[must_use]
    pub fn picking(&self) -> Option<&Picking> {
        self.picking.as_ref().map(|h| &h.asked)
    }

    /// The account shown — its tab's, or the first.
    #[must_use]
    pub fn shown<'a>(&self, r: &'a Readings) -> Option<&'a Seen> {
        self.tab
            .as_deref()
            .and_then(|id| r.seen(id))
            .or_else(|| r.accounts.first())
    }

    /// Shows the account `id` — its tab — with the cursor, when it stood on
    /// the account shown before, on the same row of this one.
    pub fn show(&mut self, id: &str) {
        self.tab = Some(id.to_owned());
        for aim in [&mut self.aim, &mut self.kept] {
            if let Some(t) = aim.take() {
                *aim = Some(retarget(t, id));
            }
        }
    }

    /// Aims at the account at `place` (from 1) — a mark clicked, `agents-open
    /// <n>` typed: its tab, the cursor on its first row. A place with no
    /// account keeps the cursor where it is.
    pub fn aim_at_place(&mut self, place: usize, r: &Readings) {
        if let Some(s) = r.at_place(place) {
            self.question = None;
            self.picking = None;
            self.tab = Some(s.account.id.clone());
            self.aim = Some(Target::Fact(s.account.id.clone(), Fact::Agent));
        }
    }

    /// The account the keys of an account act on: the one shown — none while
    /// a question or the picker is up.
    fn account_here<'a>(&self, r: &'a Readings) -> Option<&'a Seen> {
        if self.question.is_some() || self.picking.is_some() {
            return None;
        }
        self.shown(r)
    }

    /// The hint line: the keys that work where the cursor is — the file
    /// picker's while it is up.
    #[must_use]
    pub fn hint(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let _ = (zone, columns);
        if let Some(h) = &self.picking {
            return h.picker.hint(texts);
        }
        let here = self.account_here(r);
        let on_one = here.is_some();
        let logged = here.and_then(logged_in);
        let tabs = r.accounts.len() > 1;
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
                    Where::Tabs => tabs,
                    Where::LoggedIn => logged == Some(true),
                    Where::LoggedOut => on_one && logged != Some(true),
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

    /// The row whose control row `n` is a line of: the first line of a pair
    /// wrapped over several ([`Row::continues`]), which light together and
    /// take one click (`Region::control_of`, `Content::controls`) — and `n`
    /// itself for every other row.
    #[must_use]
    pub fn control_of(n: usize, rows: &[Row]) -> usize {
        let mut first = n;
        while first > 0 && rows.get(first).is_some_and(|r| r.continues) {
            first -= 1;
        }
        first
    }

    /// Puts the cursor where a click landed. Answers whether the cursor was
    /// ALREADY on that row: that is the second click, which the door turns
    /// into `RET` (invariant 19). A click on a line that continues a row is
    /// a click on that row ([`Self::control_of`]).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        self.clicked_at(n, None, rows)
    }

    /// The same, at cell `cell` of the row: on a row of tabs, the tab there
    /// shows its account at once — one click, as a terminal's strip — and on
    /// the file picker's rows the picker answers.
    pub fn clicked_at(&mut self, n: usize, cell: Option<u16>, rows: &[Row]) -> bool {
        let n = Self::control_of(n, rows);
        let Some(row) = rows.get(n).filter(|r| r.takes()) else {
            return false;
        };
        if let Some((_, _, Target::Tab(id))) = cell.and_then(|c| row.zone_at(c)) {
            let id = id.clone();
            self.show(&id);
            return false;
        }
        let Some(target) = row.target.clone() else {
            return false;
        };
        if let (Some(h), Target::Pick(t)) = (self.picking.as_mut(), &target) {
            return h.clicked(t.clone());
        }
        let again = self.cursor == n;
        self.cursor = n;
        if let Target::Answer(yes) = target {
            self.yes = yes;
        }
        self.aim = Some(target);
        again
    }

    /// The sentence the pointer resting on row `n` reads — on any line of
    /// its control, the control's.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        Self::about_at(rows, n, None, texts)
    }

    /// The same, at cell `cell` of the row: a tab's own.
    #[must_use]
    pub fn about_at(
        rows: &[Row],
        n: usize,
        cell: Option<u16>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String> {
        let row = rows.get(Self::control_of(n, rows))?;
        if let Some((_, _, zone)) = cell.and_then(|c| row.zone_at(c)) {
            return zone.about(texts);
        }
        row.target.as_ref()?.about(texts)
    }

    /// Puts the cursor back on the row it was aimed at — found by what it
    /// is, since the list may have changed under it — or on the nearest row
    /// that acts; on the file picker's row with the cursor while it is up.
    /// Called after every reading.
    pub fn settle_in(
        &mut self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        if let Some(h) = &self.picking {
            // UNDER THE PAGE'S TITLE, the picker's own rows.
            if let Some(n) = h.picker.rows(columns, texts).iter().position(|p| p.focused) {
                self.cursor = n + 1;
            }
            return;
        }
        // A QUESTION ABOUT AN ACCOUNT THAT IS GONE goes with it.
        if let Some(Question::Remove(id, ..) | Question::Logout(id, _)) = &self.question
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

    /// The rows to draw, at this width, from what is read now: the
    /// accounts' column — the tabs, and the account shown — then the
    /// options' ([`Page`] lays them side by side); or the whole width's, for
    /// a question, the file picker and the first reading.
    #[must_use]
    pub fn rows(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        if let Some(h) = &self.picking {
            return picker_rows(h, columns, texts);
        }
        let (left, room) = crate::chrome::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            cursor: self.cursor,
            left,
            room,
            texts,
            pane: Pane::Whole,
            start: 0,
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
        // THE ACCOUNTS: one, under the tabs of all.
        b.column(Pane::Accounts, columns);
        // A PART OF THE LIST NOT READ: an account it could not take has no
        // tab, and the file keeps it.
        if let Some(partly) = &r.accounts_unread {
            b.section(
                &b.t(key::AGENTS_HEADING),
                vec![note(partly.said_named(texts))],
            );
        }
        if r.refused {
            b.section(
                &b.t(key::AGENTS_HEADING),
                vec![note(words::refused(Why::Unreadable, texts))],
            );
        } else if let Some(s) = self.shown(r) {
            tabs(&mut b, r, s);
            b.push(crate::chrome::AIR, None, false);
            account_entries(&mut b, r, s, zone);
        } else {
            b.section(
                &b.t(key::AGENTS_HEADING),
                vec![note(texts.t(key::AGENTS_NONE))],
            );
        }
        // AND WHAT IS THE WHOLE LIST'S.
        b.column(Pane::Options, columns);
        if !r.refused {
            add_section(&mut b);
        }
        if r.has_claude() {
            endpoint_section(&mut b, r);
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
        let yes_word = match q {
            Question::Remove(..) => key::AGENTS_ANSWER_YES,
            Question::Logout(..) => key::AGENTS_ANSWER_LOGOUT,
        };
        for (yes, k) in [(false, key::AGENTS_ANSWER_NO), (true, yes_word)] {
            let on = b.cursor == b.out.len();
            // THE CURSOR IS A COLOUR; "yes" is in the colour of danger.
            let color = if on {
                tokens::color::TEXT_HERE_ACTS
            } else if yes {
                tokens::color::STATE_ERROR
            } else {
                crate::chrome::Kind::Setting.tones(false).0
            };
            let answer = vec![piece(b.left, t.t(k), color, b.room)];
            b.push(answer, Some(Target::Answer(yes)), false);
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
        // THE PICKER takes the view's keys that walk, answer and go back — a
        // second click's `RET`, a command's arrows, the pointer's way back.
        if self.picking.is_some() {
            use ronoctua_keymap::{Key, Sym};
            let key = match p {
                Press::Up => Key::simple(Sym::Up),
                Press::Down => Key::simple(Sym::Down),
                Press::Enter => Key::simple(Sym::Enter),
                Press::Escape => Key::simple(Sym::Esc),
                _ => return Errand::Nothing,
            };
            return self.pick_key(key, r, zone, columns, texts);
        }
        let rows = self.rows(r, zone, columns, texts);
        if self.question.is_some() {
            return self.answer_key(p, &rows);
        }
        let target = rows.get(self.cursor).and_then(|row| row.target.clone());
        let here = self.account_here(r).map(|s| s.account.id.clone());
        let on_an_account = |make: fn(String) -> Target| here.clone().map(make);
        match p {
            Press::Up | Press::Down => {
                self.walk(&rows, p == Press::Down, split_of(columns).is_some());
                Errand::Nothing
            }
            Press::Cross => {
                self.cross(&rows);
                Errand::Nothing
            }
            Press::Previous | Press::Next => {
                self.turn(r, p == Press::Next);
                self.settle_in(r, zone, columns, texts);
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
            Press::Session | Press::Login | Press::Logout => match here {
                Some(id) => {
                    let target = match p {
                        Press::Session => Target::Session(id),
                        Press::Login => Target::Login(id),
                        _ => Target::Logout(id),
                    };
                    self.enter(target, r, zone, columns, texts)
                }
                None => Errand::Say(texts.t(key::AGENTS_NONE).into()),
            },
            Press::Rename | Press::Default | Press::Remove => {
                let made = match p {
                    Press::Rename => on_an_account(Target::Rename),
                    Press::Default => on_an_account(Target::Default),
                    _ => on_an_account(Target::Remove),
                };
                match made {
                    Some(t) => self.enter(t, r, zone, columns, texts),
                    None => Errand::Say(texts.t(key::AGENTS_NOT_ON_AN_ACCOUNT).into()),
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
            "agents-add-claude-api" => Target::Add(Agent::ClaudeApi),
            "agents-add-codex" => Target::Add(Agent::Codex),
            "agents-warn-at" => Target::Warn,
            "agents-danger-at" => Target::Danger,
            // BY NAME, with no session: the agent under the cursor.
            SHOW_COMMAND => {
                let rows = self.rows(r, zone, columns, texts);
                match rows.get(self.cursor).and_then(|row| row.target.clone()) {
                    Some(target @ Target::Agent(..)) => target,
                    _ => return Errand::Say(texts.t(key::AGENTS_NOT_ON_AN_AGENT).into()),
                }
            }
            "agents-status-line" => {
                let claude = self
                    .account_here(r)
                    .filter(|s| s.account.kind == Agent::Claude);
                match claude {
                    Some(s) => Target::StatusLine(s.account.id.clone()),
                    None => return Errand::Say(words::refused(Why::Invalid, texts).into()),
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
            Target::Fact(_, Fact::Window(_))
            | Target::Answer(_)
            | Target::Endpoint
            | Target::Pick(_) => Errand::Nothing,
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
                Some(Err(why)) => Errand::Say(words::refused(why, texts).into()),
                None => Errand::Say(words::refused(Why::Invalid, texts).into()),
            },
            Target::Session(id) => Errand::Choose(id, false),
            // A LOGIN ONLY WHILE THERE IS NONE, a logout only while there is
            // one — said, by a key or a name, where the row is not drawn.
            Target::Login(id) => match r.seen(&id).and_then(logged_in) {
                Some(true) => Errand::Say(texts.t(key::AGENTS_ALREADY_LOGGED_IN).into()),
                _ => Errand::Choose(id, true),
            },
            Target::Logout(id) => match r.seen(&id) {
                Some(s) if logged_in(s) == Some(true) => {
                    self.question = Some(Question::Logout(id, s.account.name.clone()));
                    // "NO" FIRST, and the cursor is born on it.
                    self.yes = false;
                    self.aim = Some(Target::Answer(false));
                    self.settle_in(r, zone, columns, texts);
                    Errand::Nothing
                }
                _ => Errand::Say(texts.t(key::AGENTS_NOTHING_TO_LOG_OUT).into()),
            },
            Target::Rename(id) => self.ask(Ask::Name(id), r),
            Target::Default(id) => {
                if r.seen(&id).is_some_and(|s| s.account.default) {
                    Errand::Say(texts.t(key::AGENTS_ALREADY_DEFAULT).into())
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
            Target::Agent(_, session) => Errand::Show(session),
            Target::Warn => self.ask(Ask::Warn, r),
            Target::Danger => self.ask(Ask::Danger, r),
            Target::Tab(id) => {
                self.show(&id);
                self.settle_in(r, zone, columns, texts);
                Errand::Nothing
            }
        }
    }

    /// A key while the question is up: walking moves between "no" and
    /// "yes", `RET` answers, and every other key is "no".
    fn answer_key(&mut self, p: Press, rows: &[Row]) -> Errand {
        match p {
            Press::Up | Press::Down => {
                self.walk(rows, p == Press::Down, false);
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
                    (Some(Question::Logout(id, _)), true) => Errand::Logout(id),
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
                    return Errand::Say(words::refused(Why::Invalid, texts).into());
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
                    return Errand::Say(texts.t(key::AGENTS_NOT_A_PERCENT).into());
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
                        Errand::Say(texts.t(key::AGENTS_LINES_CROSS).into())
                    }
                    Err(agent_accounts::Unkept::Unreadable) => {
                        Errand::Say(texts.t(key::AGENTS_LINES_UNREADABLE).into())
                    }
                    Err(agent_accounts::Unkept::Unwritten) => {
                        Errand::Say(texts.t(key::AGENTS_LINES_UNKEPT).into())
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

    /// Moves the cursor to the next row that acts, round the ends — inside
    /// the column it is in, when the two stand side by side (`beside`).
    fn walk(&mut self, rows: &[Row], down: bool, beside: bool) {
        let pane = rows.get(self.cursor).map(|r| r.pane);
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows
                .get(at)
                .is_some_and(|r| !r.heading() && (!beside || Some(r.pane) == pane))
            {
                self.cursor = at;
                self.aim = rows[at].target.clone();
                return;
            }
        }
    }

    /// `TAB`: the cursor to the other column — to the row it was on there,
    /// or that column's first that acts.
    fn cross(&mut self, rows: &[Row]) {
        let other = match rows.get(self.cursor).map(|r| r.pane) {
            Some(Pane::Options) => Pane::Accounts,
            _ => Pane::Options,
        };
        let back = self.kept.take();
        let found = back
            .as_ref()
            .and_then(|t| {
                rows.iter()
                    .position(|r| r.pane == other && r.target.as_ref() == Some(t))
            })
            .or_else(|| rows.iter().position(|r| r.pane == other && !r.heading()));
        if let Some(n) = found {
            self.kept = self.aim.take();
            self.cursor = n;
            self.aim = rows[n].target.clone();
        } else {
            self.kept = back;
        }
    }

    /// The account before or after the one shown, round the ends.
    fn turn(&mut self, r: &Readings, next: bool) {
        let Some(shown) = self.shown(r) else {
            return;
        };
        let count = r.accounts.len();
        let Some(at) = r
            .accounts
            .iter()
            .position(|s| s.account.id == shown.account.id)
        else {
            return;
        };
        let to = if next {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        };
        let id = r.accounts[to].account.id.clone();
        self.show(&id);
    }

    /// Opens the file picker over the view, for a session or the login of
    /// `account`: at `start` — its last agent's folder — or where the picker
    /// was left; its folder watched through `wake` when the door's loop turns.
    pub fn choose(
        &mut self,
        account: &Account,
        login: bool,
        start: Option<PathBuf>,
        wake: Option<&Wake>,
    ) {
        use crate::file_picker::{EVERY_FILE, Mode, Request, Start};
        let request = Request {
            purpose: PICKER_PURPOSE,
            mode: Mode::Folder,
            filters: EVERY_FILE,
            start: start.map_or(Start::Remembered, Start::At),
            addresses: false,
            extras: &[],
            alternate: None,
        };
        self.question = None;
        self.asking = None;
        self.picking = Some(Hosted::open(
            request,
            Picking {
                account: account.id.clone(),
                name: account.name.clone(),
                login,
            },
            key::AGENTS_HEADING,
            wake,
        ));
    }

    /// What the file picker's watch brought: whether its rows changed.
    pub fn look(&mut self) -> bool {
        self.picking.as_mut().is_some_and(Hosted::look)
    }

    /// How many lines the door shows — the picker's `PgUp` and `PgDn` walk
    /// half of them.
    pub fn page_is(&self, lines: usize) {
        if let Some(h) = &self.picking {
            h.picker.page_is(u16::try_from(lines).unwrap_or(u16::MAX));
        }
    }

    /// Whether the file picker, while it is up, takes this key — every
    /// letter, and its own keys.
    #[must_use]
    pub fn picker_takes(&self, key: ronoctua_keymap::Key) -> bool {
        self.picking.as_ref().is_some_and(|h| h.picker.claims(key))
    }

    /// A key for the file picker.
    pub fn pick_key(
        &mut self,
        key: ronoctua_keymap::Key,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(h) = self.picking.as_mut() else {
            return Errand::Nothing;
        };
        let hosting = h.key(key);
        let errand = self.hosting(hosting, texts);
        self.settle_in(r, zone, columns, texts);
        errand
    }

    /// The clipboard's text, for the file picker.
    pub fn picker_pasted(
        &mut self,
        text: &str,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(h) = self.picking.as_mut() else {
            return Errand::Nothing;
        };
        let hosting = h.pasted(text);
        let errand = self.hosting(hosting, texts);
        self.settle_in(r, zone, columns, texts);
        errand
    }

    /// What a step of the picker is to the view: the folder chosen starts
    /// the agent there; back, with nothing chosen, is the view again.
    fn hosting(&mut self, hosting: Hosting<Picking>, texts: &ronoctua_i18n::Texts) -> Errand {
        match hosting {
            Hosting::Nothing => Errand::Nothing,
            Hosting::Back => {
                self.picking = None;
                Errand::Nothing
            }
            Hosting::Say(e) => Errand::Say(crate::chrome::said_of(texts, &e)),
            Hosting::Paste => Errand::Paste,
            Hosting::Picked(asked, crate::file_picker::Picked::Paths { paths, .. }) => {
                self.picking = None;
                paths
                    .into_iter()
                    .next()
                    .map_or(Errand::Nothing, |dir| Errand::Start {
                        account: asked.account,
                        login: asked.login,
                        dir,
                    })
            }
            Hosting::Picked(..) => Errand::Nothing,
        }
    }
}

/// A row's target, about account `id` in place of the one it was about —
/// the same row of the account a tab shows next.
fn retarget(t: Target, id: &str) -> Target {
    let id = id.to_owned();
    match t {
        Target::Fact(_, Fact::Day(_) | Fact::Model(_)) | Target::Agent(..) => {
            Target::Fact(id, Fact::Agent)
        }
        Target::Fact(_, fact) => Target::Fact(id, fact),
        Target::StatusLine(_) => Target::StatusLine(id),
        Target::Session(_) => Target::Session(id),
        Target::Login(_) => Target::Login(id),
        Target::Rename(_) => Target::Rename(id),
        Target::Default(_) => Target::Default(id),
        Target::Remove(_) => Target::Remove(id),
        other => other,
    }
}

/// The file picker's page: its title — whose agent, and whether a session
/// or the login — then the picker's own rows, the whole width.
fn picker_rows(h: &Hosted<Picking>, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
    let (left, room) = crate::chrome::extent(columns);
    // THE ACCOUNT IS A NAME the title quotes (`Arg::Name`): it wears what a
    // name wears in a heading, the heading's own ink (`chrome::quoted_ink`
    // answers itself there) — cut at the room, as the title always was.
    let title: crate::chrome::Named = texts
        .t_args_marked(
            if h.asked.login {
                key::AGENTS_PICK_LOGIN
            } else {
                key::AGENTS_PICK_SESSION
            },
            &[("name", Arg::Name(&h.asked.name))],
        )
        .into();
    let mut fields: Vec<Field> = Vec::new();
    let mut at = left;
    for (text, ink) in title.elided_end(room).runs(tokens::color::TEXT_HEADING) {
        match fields.last_mut() {
            Some(last) if last.color == ink => last.text.push_str(text),
            _ => fields.push(piece(at, text.to_owned(), ink, room)),
        }
        at = at.saturating_add(layout::width(text));
    }
    let mut out = vec![Row::of(fields, None, false, Pane::Whole)];
    out.extend(h.picker.rows(columns, texts).into_iter().map(|p| {
        Row::of(
            p.fields,
            (!p.heading).then_some(Target::Pick(p.target)),
            p.continues,
            Pane::Whole,
        )
    }));
    out
}

/// The row of tabs at the top of the accounts' column: one per account —
/// its glyph, its place and its name — the one shown in the cursor's
/// accent; a tab that does not fit goes to the next line, never off it. The
/// tab shown is where the person is, and no control: a click there would do
/// nothing, and nothing lights that a click does not take.
fn tabs(b: &mut Builder<'_>, r: &Readings, shown: &Seen) {
    let t = b.texts;
    let end = b.left + b.room;
    let air = tokens::space::MODELINE_SEGMENTGAP;
    let (mut fields, mut zones) = (Vec::new(), Vec::new());
    let mut at = b.left;
    for (n, s) in r.accounts.iter().enumerate() {
        let label = account_title(s, n + 1, t);
        let width = layout::width(&label).min(b.room);
        if at > b.left && at + width > end {
            let row = Row {
                zones: std::mem::take(&mut zones),
                ..Row::of(std::mem::take(&mut fields), None, false, b.pane)
            };
            b.out.push(row);
            at = b.left;
        }
        let is_shown = s.account.id == shown.account.id;
        let color = if is_shown {
            tokens::color::STATE_ACCENT
        } else {
            crate::chrome::Kind::Setting.tones(false).0
        };
        fields.push(piece(at, label, color, end.saturating_sub(at)));
        if !is_shown {
            zones.push((at, at + width, Target::Tab(s.account.id.clone())));
        }
        at += width + air;
    }
    if !fields.is_empty() {
        let row = Row {
            zones,
            ..Row::of(fields, None, false, b.pane)
        };
        b.out.push(row);
    }
}

/// An account as its tab says it: its company's glyph, its place in the list
/// (its mark's set) and its name.
fn account_title(s: &Seen, place: usize, t: &ronoctua_i18n::Texts) -> String {
    let glyph = agent_accounts::mark_of(s.account.kind)
        .and_then(|m| crate::brainlink::glyph_of(&m.glyph))
        .map(String::from)
        .unwrap_or_default();
    t.t_args(
        key::AGENTS_ACCOUNT,
        &[
            ("glyph", Arg::Text(&glyph)),
            (
                "place",
                Arg::Number(i64::try_from(place).unwrap_or(i64::MAX)),
            ),
            ("name", Arg::Text(&s.account.name)),
        ],
    )
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

/// One account, under its tab: its facts, its agents, its windows, its
/// tokens and the rows that act on it.
fn account_entries(b: &mut Builder<'_>, r: &Readings, s: &Seen, zone: &ronoctua_files::when::Zone) {
    let t = b.texts;
    let a = &s.account;
    let id = || a.id.clone();
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
        // AN API KEY'S "PLAN" is the token, and where its key comes from —
        // that it has one is all that was read of it.
        Ok(Facts {
            api_key: Some(from),
            ..
        }) => t.t(match from {
            ApiKey::Login => key::AGENTS_PLAN_API_LOGIN,
            ApiKey::Helper => key::AGENTS_PLAN_API_HELPER,
            ApiKey::Settings => key::AGENTS_PLAN_API_SETTINGS,
        }),
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

    // ITS AGENTS, NOW — the keyboard's way to each glyph of its set on the
    // modeline: the same glyph, the same colour, and `RET` goes to it.
    let mine: Vec<&AgentSeen> = r.agents.iter().filter(|g| g.account == a.id).collect();
    if !mine.is_empty() {
        e.push(Entry::Air);
    }
    for agent in mine {
        e.push(agent_entry(agent, t));
        // ITS TASK, quietly under its folder (owner, 2026-11-09): what
        // Claude Code calls the work, when it named it.
        if let Some(title) = agent.title.as_deref().filter(|t| !t.trim().is_empty()) {
            e.push(Entry::Under {
                text: t.t_with(key::AGENTS_AGENT_TASK, &[("title", title)]),
                indent: layout::width(&tokens::icon::AGENT.to_string()) + 1,
            });
        }
    }

    // THE WINDOWS, as bars in the colour the marks wear — or, for an API
    // key, which has none, its credits, in the colour its mark wears.
    e.push(Entry::Air);
    if let Some(credits) = &s.credits {
        let (value, out) = match credits {
            Ok(Some(at)) => (
                t.t_with(
                    key::AGENTS_CREDITS_OUT,
                    &[("when", &crate::chrome::when_text(*at, zone, false, t))],
                ),
                Some(*at),
            ),
            Ok(None) => (t.t(key::AGENTS_CREDITS_OK), None),
            Err(why) => (words::refused(*why, t), None),
        };
        e.push(tinted(
            t.t(key::AGENTS_ROW_CREDITS),
            value,
            Target::Fact(id(), Fact::Credits),
            credits_tint(out),
        ));
        e.push(note(t.t(key::AGENTS_CREDITS_NOTE)));
    }
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
    // A LOGIN WHILE THERE IS NONE, A LOGOUT WHILE THERE IS ONE (owner,
    // 2026-11-09): the agent renews its own token, so a login over a login
    // would only open a session.
    if logged_in(s) == Some(true) {
        e.push(pair(
            t.t(key::AGENTS_ROW_LOGOUT),
            String::new(),
            Target::Logout(id()),
            true,
        ));
    } else {
        e.push(pair(
            t.t(key::AGENTS_ROW_LOGIN),
            String::new(),
            Target::Login(id()),
            true,
        ));
    }
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
    b.entries(e);
}

/// An agent's row: the agent's glyph (a robot's head, the terminals' own and
/// its mark's on the modeline) and its folder, what it is doing in the colour
/// its mark wears (`Doing::tone`), and `RET` going to it.
fn agent_entry(agent: &AgentSeen, t: &ronoctua_i18n::Texts) -> Entry {
    let glyph = tokens::icon::AGENT.to_string();
    Entry::Pair {
        label: t.t_args(
            key::AGENTS_AGENT,
            &[
                ("glyph", Arg::Text(&glyph)),
                ("folder", Arg::Text(&agent.folder)),
            ],
        ),
        value: t.t(words::doing_key(agent.doing)),
        kind: crate::chrome::Kind::of(true),
        target: Target::Agent(agent.account.clone(), agent.session.clone()),
        tint: doing_tint(agent.doing),
    }
}

/// The colour an agent's state wears: its mark's on the modeline — `None`
/// when it is quiet, and the value says it in its own tone.
#[must_use]
pub fn doing_tint(doing: Doing) -> Option<tokens::Color> {
    match doing.tone() {
        Tone::Quiet => None,
        tone => Some(crate::brainlink::tone_colour(tone)),
    }
}

/// Where an account's numbers came from, and when — or why they did not.
/// An API key has no numbers but its credits, which say so themselves.
fn source_note(
    s: &Seen,
    r: &Readings,
    zone: &ronoctua_files::when::Zone,
    t: &ronoctua_i18n::Texts,
) -> Option<String> {
    match s.account.kind {
        Agent::Codex => return Some(t.t(key::AGENTS_FROM_LOGS)),
        Agent::ClaudeApi => return None,
        Agent::Claude => {}
    }
    let asked = s.usage.and_then(|k| k.usage_at_s.map(|at| (at, k.refused)));
    let refused = s.usage.and_then(|k| k.refused);
    Some(match (asked, refused) {
        (_, Some(why)) => t.t_with(
            key::AGENTS_ENDPOINT_REFUSED,
            &[("why", &words::clause(why, t))],
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
    // A PART OF THE LINES' FILE NOT READ: the two rows show the distro's
    // lines in that place, and the file keeps what it says.
    let e = r
        .lines_unread
        .iter()
        .map(|partly| note(partly.said_named(t)))
        .chain(e)
        .collect();
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
        ronoctua_cmd::Command {
            id: "agents-logout",
            label: crate::key::CMD_AGENTS_LOGOUT,
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
        // THE TWO COLUMNS AND THE TABS (owner, 2026-11-09).
        ronoctua_cmd::Command {
            id: CROSS_COMMAND,
            label: crate::key::CMD_AGENTS_CROSS,
        },
        ronoctua_cmd::Command {
            id: "agents-previous-account",
            label: crate::key::CMD_AGENTS_PREVIOUS_ACCOUNT,
        },
        ronoctua_cmd::Command {
            id: "agents-next-account",
            label: crate::key::CMD_AGENTS_NEXT_ACCOUNT,
        },
        ronoctua_cmd::Command {
            id: "agents-add-claude",
            label: crate::key::CMD_AGENTS_ADD_CLAUDE,
        },
        ronoctua_cmd::Command {
            id: "agents-add-claude-api",
            label: crate::key::CMD_AGENTS_ADD_CLAUDE_API,
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
        // AN AGENT, by its session: what a glyph of an account's set runs,
        // and its row here.
        ronoctua_cmd::Command {
            id: "agents-show",
            label: crate::key::CMD_AGENTS_SHOW,
        },
    ],
    bindings: &[
        ("/ a a", "agents-open", None),
        ("/ a w", "agents-window", None),
        ("/ a n", "agents-session", None),
        ("/ a l", "agents-login", None),
    ],
    groups: &[("/ a", crate::key::GROUP_AGENTS)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::agents::reached().collect()
}

#[cfg(test)]
pub(crate) mod tests {
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
                api_key: None,
            }),
            limits: Ok(Limits {
                five_hour: Some(window(85.0, NOW + 3600)),
                seven_day: Some(window(40.0, NOW + 86_400)),
                no_five_hour: false,
            }),
            usage: (kind == Agent::Claude).then_some(usage::Kept {
                format: 1,
                at_s: NOW - 60,
                usage: Some(usage::Usage {
                    five_hour: Some(window(80.0, NOW + 3600)),
                    seven_day: Some(window(41.0, NOW + 86_400)),
                    seven_day_opus: Some(window(97.0, NOW + 86_400)),
                    seven_day_sonnet: None,
                    no_five_hour: false,
                }),
                usage_at_s: Some(NOW - 60),
                refused: None,
            }),
            tally,
            wrapping: (kind == Agent::Claude).then_some(Ok(Wrapping::Ours)),
            credits: (kind == Agent::ClaudeApi).then_some(Ok(None)),
        }
    }

    pub(crate) fn readings() -> Readings {
        Readings {
            read: true,
            refused: false,
            accounts: vec![
                seen("claude-1", Agent::Claude, "Work", true),
                seen("codex-1", Agent::Codex, "Home", true),
            ],
            agents: Vec::new(),
            lines: agent_accounts::Lines::default(),
            accounts_unread: None,
            lines_unread: None,
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

    /// The rows of every account's tab, one after another — what the view
    /// draws across its tabs.
    fn every_tab(r: &Readings, columns: u16, t: &ronoctua_i18n::Texts) -> Vec<Row> {
        let mut out = View::default().rows(r, &zone(), columns, t);
        for s in r.accounts.iter().skip(1) {
            let mut v = View::default();
            v.show(&s.account.id);
            out.extend(v.rows(r, &zone(), columns, t));
        }
        out
    }

    /// The cursor on the row that is `target` — its account's tab shown
    /// first, when it is an account's.
    fn stand_on(v: &mut View, r: &Readings, target: &Target, t: &ronoctua_i18n::Texts) {
        if let Some(id) = target.account() {
            v.show(id);
        }
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
            // AN API KEY in each of its states: its key from a helper and its
            // credits out, no key yet and a word that does not read.
            let mut keys = readings();
            let mut helped = seen("claude-api-1", Agent::ClaudeApi, "Console", false);
            helped.facts = Ok(Facts {
                api_key: Some(ApiKey::Helper),
                ..Facts::default()
            });
            helped.credits = Some(Ok(Some(NOW - 600)));
            let mut unread = seen("claude-api-2", Agent::ClaudeApi, "Spare", false);
            unread.facts = Err(Why::NoCredentials);
            unread.credits = Some(Err(Why::Unreadable));
            keys.accounts.extend([helped, unread]);
            // A long value WRAPS onto the next line at a space: read as one.
            let said = drawn(&every_tab(&keys, 100, &t)).replace('\n', " ");
            for k in [
                key::AGENTS_PLAN_API_HELPER,
                key::AGENTS_KIND_CLAUDE_API,
                key::AGENTS_ROW_CREDITS,
            ] {
                assert!(
                    said.contains(&t.t(k)),
                    "{language}: {k} not drawn in\n{said}"
                );
            }
            states.push(keys);
            for r in &states {
                let v = View::default();
                let rows = every_tab(r, 100, &t);
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

    /// FAILS IF: a mark's place does not open its account's tab with the
    /// cursor on it, a session or a login is of another account than the one
    /// shown — wherever the cursor is, the options' column included — opens
    /// anything but the folder's question first, or `←` `→` do not go from
    /// account to account, round the ends.
    #[test]
    fn a_place_opens_its_accounts_tab_and_a_session_is_the_shown_ones() {
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
        assert_eq!(v.shown(&r).map(|s| s.account.id.as_str()), Some("codex-1"));
        assert_eq!(
            v.press(Press::Session, &r, &zone(), 100, &t),
            Errand::Choose("codex-1".to_owned(), false)
        );
        let mut out = r.clone();
        out.accounts[1].facts = Err(Why::NoCredentials);
        assert_eq!(
            v.press(Press::Login, &out, &zone(), 100, &t),
            Errand::Choose("codex-1".to_owned(), true)
        );
        stand_on(&mut v, &r, &Target::Refresh, &t);
        assert_eq!(
            v.press(Press::Session, &r, &zone(), 100, &t),
            Errand::Choose("codex-1".to_owned(), false),
            "the options' column: still the account shown"
        );
        assert!(matches!(
            v.press(Press::Rename, &r, &zone(), 100, &t),
            Errand::Ask(_)
        ));
        v.let_go();
        // THE TABS, round the ends.
        let _ = v.press(Press::Next, &r, &zone(), 100, &t);
        assert_eq!(v.shown(&r).map(|s| s.account.id.as_str()), Some("claude-1"));
        let _ = v.press(Press::Previous, &r, &zone(), 100, &t);
        assert_eq!(v.shown(&r).map(|s| s.account.id.as_str()), Some("codex-1"));
        // A PLACE WITH NO ACCOUNT keeps everything where it was.
        v.aim_at_place(9, &r);
        assert_eq!(v.shown(&r).map(|s| s.account.id.as_str()), Some("codex-1"));
        assert_eq!(
            v.command("agents-refresh", &r, &zone(), 100, &t),
            Errand::Refresh
        );
    }

    /// FAILS IF: side by side, a row of one column is drawn in the other's
    /// cells, `TAB` does not cross to the other column — back to the row its
    /// cursor was on there — the arrows up and down leave the column they
    /// walk, a click on another account's tab does not show it at once (or
    /// waits for a second), or the tab shown is a control that does nothing.
    #[test]
    fn side_by_side_each_column_keeps_its_cursor_and_a_tab_shows_its_account() {
        let t = texts("en-GB");
        let r = readings();
        let wide = 160;
        let split = split_of(wide).expect("side by side at 160 columns");
        assert!(
            split_of(100).is_none(),
            "the tests' 100 columns are one list"
        );
        let mut v = View::default();
        v.settle_in(&r, &zone(), wide, &t);
        let rows = v.rows(&r, &zone(), wide, &t);
        assert_eq!(Page::of(&rows, wide).split, Some(split));
        for row in &rows {
            for f in &row.fields {
                match row.pane {
                    Pane::Options => assert!(f.at < split, "{:?} past the split", f.text),
                    Pane::Accounts => assert!(f.at >= split, "{:?} before the split", f.text),
                    Pane::Whole => panic!("a whole line side by side: {:?}", f.text),
                }
            }
        }
        let here = |v: &View| {
            let rows = v.rows(&r, &zone(), wide, &t);
            (rows[v.cursor()].pane, rows[v.cursor()].target.clone())
        };
        let born = here(&v);
        assert_eq!(born.0, Pane::Accounts, "the cursor is born on the account");
        let _ = v.press(Press::Cross, &r, &zone(), wide, &t);
        assert_eq!(here(&v).0, Pane::Options);
        let _ = v.press(Press::Down, &r, &zone(), wide, &t);
        let walked = here(&v);
        for _ in 0..40 {
            let _ = v.press(Press::Down, &r, &zone(), wide, &t);
            assert_eq!(here(&v).0, Pane::Options, "walked out of its column");
        }
        while here(&v) != walked {
            let _ = v.press(Press::Down, &r, &zone(), wide, &t);
        }
        let _ = v.press(Press::Cross, &r, &zone(), wide, &t);
        assert_eq!(here(&v), born, "the accounts' cursor was not kept");
        let _ = v.press(Press::Cross, &r, &zone(), wide, &t);
        assert_eq!(here(&v), walked, "the options' cursor was not kept");
        // ANOTHER ACCOUNT'S TAB: one click shows it.
        let rows = v.rows(&r, &zone(), wide, &t);
        let codex = Target::Tab("codex-1".to_owned());
        let (n, from) = rows
            .iter()
            .enumerate()
            .find_map(|(n, row)| row.zones.iter().find(|z| z.2 == codex).map(|z| (n, z.0)))
            .expect("codex-1's tab");
        assert!(
            !v.clicked_at(n, Some(from), &rows),
            "a tab waited for a second click"
        );
        assert_eq!(v.shown(&r).map(|s| s.account.id.as_str()), Some("codex-1"));
        let rows = v.rows(&r, &zone(), wide, &t);
        assert!(
            rows.iter()
                .all(|row| row.zones.iter().all(|z| z.2 != codex)),
            "the tab shown is a control"
        );
        assert!(
            rows.iter().any(|row| row
                .zones
                .iter()
                .any(|z| z.2 == Target::Tab("claude-1".to_owned()))),
            "the other account has no tab to go back by"
        );
    }

    /// FAILS IF: one list — narrower than the columns side by side — draws
    /// the options' first title right under the account's last row, with no
    /// air, or side by side draws air above the options' first title.
    #[test]
    fn the_two_parts_are_apart_in_one_list_and_level_side_by_side() {
        let t = texts("en-GB");
        let r = readings();
        let title = t.t(key::AGENTS_HEADING_ADD);
        for (columns, air) in [(100, true), (160, false)] {
            let rows = View::default().rows(&r, &zone(), columns, &t);
            let n = rows
                .iter()
                .position(|row| row.fields.iter().any(|f| f.text == title))
                .expect("the options' first title");
            let options = rows.iter().position(|row| row.pane == Pane::Options);
            if air {
                assert!(
                    rows[n - 1].fields.is_empty(),
                    "no air above it at {columns}"
                );
            } else {
                assert_eq!(options, Some(n), "air above it side by side");
            }
        }
    }

    /// FAILS IF: an agent's task is not drawn under its row, quietly, or a
    /// click on that line does not take the agent's row — or an agent that
    /// named nothing draws a line for it.
    #[test]
    fn an_agents_task_is_a_quiet_line_of_its_row() {
        let t = texts("en-GB");
        let mut r = readings();
        r.agents = vec![
            AgentSeen {
                title: Some("Contas em duas colunas".to_owned()),
                ..running("claude-1", 1, Doing::Working, Some("0123456789abcdef"))
            },
            running("claude-1", 2, Doing::Done, None),
        ];
        let mut v = View::default();
        v.settle_in(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        let task = rows
            .iter()
            .position(|row| drawn(std::slice::from_ref(row)).contains("Contas em duas colunas"))
            .expect("the task is not drawn");
        let row = &rows[task];
        assert!(
            row.continues && row.heading(),
            "the task is a row of its own"
        );
        assert!(
            row.fields
                .iter()
                .all(|f| f.color == tokens::color::TEXT_DIM)
        );
        let agent = View::control_of(task, &rows);
        assert!(matches!(rows[agent].target, Some(Target::Agent(..))));
        assert!(!v.clicked(task, &rows), "the first click acted");
        assert_eq!(v.cursor(), agent, "a click on the task took another row");
        let quiet = r.agents[1].session.clone();
        let second = rows
            .iter()
            .position(|row| matches!(&row.target, Some(Target::Agent(_, s)) if *s == quiet))
            .expect("the second agent's row");
        assert!(
            !rows.get(second + 1).is_some_and(|row| row.continues),
            "an agent that named nothing drew a task"
        );
    }

    /// FAILS IF: an account logged in draws a login row, or one logged out
    /// draws a logout row (or not its login); logging out goes through
    /// without a question, the question does not draw "no" first, or "yes"
    /// does not log THAT account out — and a login by key on an account
    /// logged in, or a logout on one logged out, does anything but say so
    /// (owner, 2026-11-09).
    #[test]
    fn a_login_while_there_is_none_and_a_logout_while_there_is_one() {
        let t = texts("en-GB");
        let mut r = readings();
        r.accounts[1].facts = Err(Why::NoCredentials);
        let rows_of = |v: &mut View, id: &str| {
            v.show(id);
            v.rows(&r, &zone(), 100, &t)
        };
        let has = |rows: &[Row], target: &Target| {
            rows.iter().any(|row| row.target.as_ref() == Some(target))
        };
        let mut v = View::default();
        let rows = rows_of(&mut v, "claude-1");
        assert!(has(&rows, &Target::Logout("claude-1".to_owned())));
        assert!(
            !has(&rows, &Target::Login("claude-1".to_owned())),
            "a login over a login"
        );
        let rows = rows_of(&mut v, "codex-1");
        assert!(has(&rows, &Target::Login("codex-1".to_owned())));
        assert!(
            !has(&rows, &Target::Logout("codex-1".to_owned())),
            "a logout of nothing"
        );
        // BY KEY, where the row is not drawn: said.
        v.show("claude-1");
        assert_eq!(
            v.press(Press::Login, &r, &zone(), 100, &t),
            Errand::Say(t.t(key::AGENTS_ALREADY_LOGGED_IN).into())
        );
        v.show("codex-1");
        assert_eq!(
            v.press(Press::Logout, &r, &zone(), 100, &t),
            Errand::Say(t.t(key::AGENTS_NOTHING_TO_LOG_OUT).into())
        );
        // THE LOGOUT: asked, "no" first, and "yes" is THAT account's.
        v.show("claude-1");
        assert_eq!(
            v.press(Press::Logout, &r, &zone(), 100, &t),
            Errand::Nothing
        );
        assert!(matches!(v.question(), Some(Question::Logout(id, _)) if id == "claude-1"));
        let rows = v.rows(&r, &zone(), 100, &t);
        assert_eq!(rows[v.cursor()].target, Some(Target::Answer(false)));
        assert!(drawn(&rows).contains(&t.t(key::AGENTS_ANSWER_LOGOUT)));
        let _ = v.press(Press::Down, &r, &zone(), 100, &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Logout("claude-1".to_owned())
        );
        assert!(v.question().is_none());
    }

    /// FAILS IF: a logout the view asked for is carried anywhere but to its
    /// account — a photograph's writes it down, and opens nothing.
    #[test]
    fn a_logout_is_carried_out_for_its_account() {
        fn planted(_: &ronoctua_files::when::Zone) -> Readings {
            readings()
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
        let mut way = recorder("en-GB");
        carry_out(Errand::Logout("codex-1".to_owned()), &mut open, &mut way);
        assert_eq!(open.done(), [format!("{LOGOUT_COMMAND} codex-1")]);
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

    /// FAILS IF: an Ultra account — the week alone, said by its status line
    /// or the endpoint — draws a "5 hours: not known yet" row that can never
    /// become known; or a fresh account, which nothing has said anything
    /// about, loses the row it still owes.
    #[test]
    fn an_ultra_account_draws_no_five_hours_row() {
        let spans = |s: &Seen| {
            windows_of(s, NOW)
                .into_iter()
                .map(|(span, _)| span)
                .collect::<Vec<_>>()
        };
        let mut ultra = seen("claude-1", Agent::Claude, "Work", true);
        ultra.limits = Ok(Limits {
            five_hour: None,
            seven_day: Some(window(71.0, NOW + 86_400)),
            no_five_hour: true,
        });
        ultra.usage = None;
        assert_eq!(spans(&ultra), [Span::Week]);

        // The endpoint alone says so too.
        let mut ultra_endpoint = seen("claude-1", Agent::Claude, "Work", true);
        ultra_endpoint.limits = Ok(Limits::default());
        if let Some(k) = ultra_endpoint.usage.as_mut()
            && let Some(u) = k.usage.as_mut()
        {
            u.five_hour = None;
            u.no_five_hour = true;
        }
        assert!(!spans(&ultra_endpoint).contains(&Span::FiveHours));

        // A window that IS known is drawn whatever the flag says.
        let mut known = ultra;
        known.limits = Ok(Limits {
            five_hour: Some(window(10.0, NOW + 3600)),
            ..Limits::default()
        });
        known.usage = None;
        assert_eq!(spans(&known), [Span::FiveHours, Span::Week]);
    }

    /// An API key's account, logged in by the Console, its credits fine.
    fn api_seen() -> Seen {
        let mut api = seen("claude-api-1", Agent::ClaudeApi, "Console", false);
        api.facts = Ok(Facts {
            api_key: Some(ApiKey::Login),
            ..Facts::default()
        });
        api.limits = Ok(Limits::default());
        api
    }

    /// FAILS IF: an API key's account draws a window — every one would say
    /// "not known yet" forever — or a status line of ours, or no credits
    /// row; its credits are not said (fine, out since when, or why not), or
    /// wear another colour than its mark on the modeline; its plan is not
    /// said as the token and the way its key comes; or any of it draws a
    /// raw key in a language.
    #[test]
    fn an_api_key_draws_its_credits_and_no_window() {
        let mut r = readings();
        r.accounts.push(api_seen());
        let about_api = |rows: &[Row]| -> Vec<Target> {
            rows.iter()
                .filter_map(|row| row.target.clone())
                .filter(|t| t.account() == Some("claude-api-1"))
                .collect()
        };
        let credits = Target::Fact("claude-api-1".to_owned(), Fact::Credits);
        for language in ronoctua_i18n::LANGUAGES {
            let t = texts(language);
            let rows = every_tab(&r, 100, &t);
            let api = about_api(&rows);
            assert!(api.contains(&credits), "{language}: no credits row");
            assert!(
                !api.iter()
                    .any(|t| matches!(t, Target::Fact(_, Fact::Window(_)) | Target::StatusLine(_))),
                "{language}: a window or a status line on a key: {api:?}"
            );
            let text = drawn(&rows);
            assert!(
                !text.contains("agents-"),
                "{language}: a raw key in\n{text}"
            );
            assert!(
                text.contains(&t.t(key::AGENTS_PLAN_API_LOGIN)),
                "{language}"
            );
            assert!(
                text.contains(&t.t(key::AGENTS_KIND_CLAUDE_API)),
                "{language}"
            );
        }
        let t = texts("en-GB");
        let value_of = |r: &Readings| {
            let rows = every_tab(r, 100, &t);
            let row = rows
                .iter()
                .find(|row| row.target.as_ref() == Some(&credits))
                .expect("the credits row")
                .clone();
            let last = row.fields.last().expect("a value").clone();
            (last.text, last.color)
        };
        let (fine, fine_colour) = value_of(&r);
        assert!(fine.contains(&t.t(key::AGENTS_CREDITS_OK)), "{fine}");
        assert_ne!(
            Some(fine_colour),
            credits_tint(Some(NOW)),
            "fine credits in the colour of danger"
        );

        // OUT: since when, in the colour the mark wears.
        r.accounts[2].credits = Some(Ok(Some(NOW - 600)));
        let (out, colour) = value_of(&r);
        let when = crate::chrome::when_text(NOW - 600, &zone(), false, &t);
        assert!(out.contains(&when), "{out}");
        assert_eq!(
            Some(colour),
            Some(crate::brainlink::tone_colour(agent_accounts::credits_tone(
                Some(NOW - 600)
            )))
        );
        assert_eq!(agent_accounts::credits_tone(Some(NOW)), Tone::Danger);
        // A WORD THAT DOES NOT READ is said, never taken for fine credits.
        r.accounts[2].credits = Some(Err(Why::Unreadable));
        assert_eq!(value_of(&r).0, words::refused(Why::Unreadable, &t));
        // NO CLOCK for a key: it has no window to reset.
        let mut only = readings();
        only.accounts = vec![api_seen()];
        assert_eq!(next_reset(&only), None);
        assert!(
            !drawn(&View::default().rows(&only, &zone(), 100, &t)).contains(usage::HOST),
            "the usage endpoint asked to be let for a key"
        );
    }

    /// FAILS IF: adding an account does not offer an API key's — by its
    /// row, its command and its question on the door's own line — or the
    /// one added is not an API key's in a folder of its own, private, with
    /// its login opened, no status line of ours in its `settings.json`, and
    /// the subscription's default left where it was.
    #[test]
    fn adding_offers_an_api_key_and_puts_no_status_line_on_it() {
        let t = texts("en-GB");
        let r = readings();
        let mut v = View::default();
        let add = Target::Add(Agent::ClaudeApi);
        stand_on(&mut v, &r, &add, &t);
        let Errand::Ask(line) = v.press(Press::Enter, &r, &zone(), 100, &t) else {
            panic!("the name was not asked");
        };
        assert_eq!(line.id, "agents-add-claude-api");
        assert_eq!(
            View::about(&v.rows(&r, &zone(), 100, &t), v.cursor(), &t),
            Some(t.t("cmd-agents-add-claude-api"))
        );
        assert_eq!(
            v.answered(line.id, " Console ", &t),
            Errand::Edit(Edit::Add(Agent::ClaudeApi, "Console".to_owned()))
        );
        let mut by_name = View::default();
        assert!(matches!(
            by_name.command("agents-add-claude-api", &r, &zone(), 100, &t),
            Errand::Ask(ref l) if l.id == "agents-add-claude-api"
        ));

        let dir = scratch("api");
        let roots = roots_in(&dir);
        std::fs::create_dir_all(dir.join("home")).unwrap();
        let _ = apply_in(&roots, &Edit::Add(Agent::Claude, "Work".to_owned()), &t);
        let done = apply_in(
            &roots,
            &Edit::Add(Agent::ClaudeApi, "Console".to_owned()),
            &t,
        );
        assert_eq!(
            done.said.as_deref(),
            Some(t.t_with(key::AGENTS_ADDED, &[("name", "Console")]).as_str())
        );
        // THE ACCOUNT'S NAME IS TOLD as the name it is, and only it.
        let said = done.said.as_ref().expect("said");
        assert_eq!(
            said.names()
                .iter()
                .map(|r| &said.as_str()[r.clone()])
                .collect::<Vec<_>>(),
            ["Console"]
        );
        let list = Accounts::store_in(roots.config).parsed().expect("the list");
        let api = list.find("claude-api-1").expect("listed").clone();
        assert_eq!(api.kind, Agent::ClaudeApi);
        assert_eq!(api.dir, dir.join("home").join(".claude-api"));
        assert_eq!(mode_of(&api.dir), 0o700);
        assert!(
            !api.dir
                .join(ronoctua_llm::statusline::SETTINGS_FILE)
                .exists(),
            "a status line of ours on a key"
        );
        assert!(!api.default, "the subscription's default moved");
        assert!(list.find("claude-1").is_some_and(|a| a.default));
        assert_eq!(done.login.as_ref(), Some(&api), "not its login next");
        // REMOVED, it is asked as an account with no status line of ours —
        // its name told as one, so the page draws it in a name's ink.
        let asked = question_text(
            &Question::Remove(api.id.clone(), api.name.clone(), api.kind),
            &t,
        );
        assert_eq!(
            asked,
            t.t_with(key::AGENTS_ASK_REMOVE, &[("name", "Console")])
        );
        assert_eq!(
            asked
                .names()
                .iter()
                .map(|r| &asked.as_str()[r.clone()])
                .collect::<Vec<_>>(),
            ["Console"]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: an account nothing is known of yet stops saying "not known
    /// yet" for its five hours — the Ultra rule hides the row only on the
    /// plan's own word.
    #[test]
    fn a_fresh_account_still_says_not_known_yet() {
        let mut fresh = seen("claude-2", Agent::Claude, "New", false);
        fresh.limits = Ok(Limits::default());
        fresh.usage = None;
        assert_eq!(
            windows_of(&fresh, NOW),
            [(Span::FiveHours, None), (Span::Week, None)]
        );
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
            no_five_hour: false,
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
        assert!(!Errand::Say("x".into()).writes());
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
    /// Code account (or puts one on Codex), or does not hand THAT account's
    /// login on, to a folder chosen first; or a rename, a default, a status line given back and
    /// put in front again, and a removal do not land in the files — or an
    /// account whose folder could not be made is said to be added.
    #[test]
    fn the_gestures_land_in_the_files() {
        use ronoctua_llm::statusline::{SETTINGS_FILE, wrapped};
        let dir = scratch("gestures");
        let roots = roots_in(&dir);
        std::fs::create_dir_all(dir.join("home")).unwrap();
        let t = texts("en-GB");
        let list = || {
            Accounts::store_in(roots.config.clone())
                .parsed()
                .expect("the list")
        };

        // CLAUDE CODE: the folder, the list, our status line, the login.
        let done = apply_in(&roots, &Edit::Add(Agent::Claude, "Work".to_owned()), &t);
        let work = list().accounts[0].clone();
        assert_eq!((work.kind, work.name.as_str()), (Agent::Claude, "Work"));
        assert!(
            work.dir.starts_with(dir.join("home")),
            "{}",
            work.dir.display()
        );
        assert_eq!(mode_of(&work.dir), 0o700, "the folder is not private");
        assert_eq!(wrapped(&work), Ok(Wrapping::Ours), "no status line of ours");
        assert_eq!(done.login.as_ref(), Some(&work), "not its login next");
        assert_eq!(
            done.said.as_deref(),
            Some(t.t_with(key::AGENTS_ADDED, &[("name", "Work")]).as_str())
        );

        // CODEX: a folder, the list, and no status line.
        let done = apply_in(&roots, &Edit::Add(Agent::Codex, "Home".to_owned()), &t);
        let home = list().accounts[1].clone();
        assert_eq!(home.kind, Agent::Codex);
        assert_eq!(mode_of(&home.dir), 0o700);
        assert!(
            !home.dir.join(SETTINGS_FILE).exists(),
            "Codex has no status line"
        );
        assert_eq!(done.login.as_ref(), Some(&home));

        // NAME, DEFAULT, THE STATUS LINE BOTH WAYS.
        let side = {
            let _ = apply_in(&roots, &Edit::Add(Agent::Claude, "Side".to_owned()), &t);
            list().accounts[2].clone()
        };
        assert!(!side.default, "the first of its kind stays the default");
        let _ = apply_in(
            &roots,
            &Edit::Rename(work.id.clone(), "Office".to_owned()),
            &t,
        );
        assert_eq!(
            list().find(&work.id).map(|a| a.name.clone()),
            Some("Office".to_owned())
        );
        let _ = apply_in(&roots, &Edit::Default(side.id.clone()), &t);
        assert_eq!(
            list().default_of(Agent::Claude).map(|a| a.id.clone()),
            Some(side.id)
        );
        let _ = apply_in(&roots, &Edit::Unwrap(work.id.clone()), &t);
        assert_ne!(wrapped(&work), Ok(Wrapping::Ours), "ours still in front");
        let _ = apply_in(&roots, &Edit::Wrap(work.id.clone()), &t);
        assert_eq!(wrapped(&work), Ok(Wrapping::Ours));

        // REMOVED: off the list, its folder kept, its own status line back.
        let done = apply_in(&roots, &Edit::Remove(work.id.clone()), &t);
        assert!(list().find(&work.id).is_none());
        assert!(work.dir.is_dir(), "the folder holds its sessions and login");
        assert_ne!(wrapped(&work), Ok(Wrapping::Ours));
        assert_eq!(
            done.said.as_deref(),
            Some(
                t.t_with(key::AGENTS_REMOVED, &[("name", "Office")])
                    .as_str()
            )
        );

        // A FOLDER THAT CANNOT BE MADE: refused, said, nothing listed or opened.
        let blocked = scratch("blocked");
        std::fs::write(blocked.join("home"), b"a file, not a home").unwrap();
        let done = apply_in(
            &roots_in(&blocked),
            &Edit::Add(Agent::Codex, "Nowhere".to_owned()),
            &t,
        );
        assert_eq!(
            done.said.as_deref(),
            Some(words::refused(Why::Failed, &t).as_str())
        );
        assert_eq!(done.login, None, "a login for no account");
        assert!(
            Accounts::store_in(roots_in(&blocked).config)
                .parsed()
                .is_some_and(|l| l.accounts.is_empty())
        );
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(blocked);
    }

    /// FAILS IF: a session or a login opens anything before its folder is
    /// chosen — in the distro's file picker, over the view, for the account
    /// at the place given (a mark's right click) and with its tab shown —
    /// `Esc` on the picker does not give the view back, or the folder chosen
    /// is not where that account's session or login opens.
    #[test]
    fn a_session_is_a_folder_chosen_first_and_opens_there() {
        fn planted(_: &ronoctua_files::when::Zone) -> Readings {
            readings()
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
        let mut way = recorder("en-GB");
        open.choose_at(Some(2), false, &mut way);
        assert_eq!(
            open.view.picking().map(|p| (p.account.as_str(), p.login)),
            Some(("codex-1", false))
        );
        assert_eq!(
            open.view
                .shown(&open.readings)
                .map(|s| s.account.id.as_str()),
            Some("codex-1")
        );
        let rows = open.rows(100, &t);
        assert!(
            drawn(&rows[..1]).contains("Home"),
            "the page does not say whose agent: {}",
            drawn(&rows[..1])
        );
        let here = Some(Target::Pick(crate::file_picker::Target::Here));
        assert!(rows.iter().any(|r| r.target == here), "no folder to choose");
        assert!(open.done().is_empty(), "something opened before the folder");
        // BACK, with nothing chosen: the view again.
        let esc = ronoctua_keymap::Key::parse("ESC").expect("a key");
        assert!(open.key(esc, &mut way));
        assert!(open.view.picking().is_none());
        // THE LOGIN, the folder chosen with two clicks: opened there.
        open.choose_at(Some(1), true, &mut way);
        let rows = open.rows(100, &t);
        let n = rows
            .iter()
            .position(|r| r.target == here)
            .expect("the folder's row");
        open.clicked(n, &mut way);
        open.clicked(n, &mut way);
        assert!(open.view.picking().is_none(), "the picker stayed");
        let [done] = open.done() else {
            panic!("not one thing opened: {:?}", open.done());
        };
        let opened = done
            .strip_prefix(&format!("{LOGIN_COMMAND} claude-1 "))
            .unwrap_or_else(|| panic!("not claude-1's login: {done}"));
        assert!(Path::new(opened).is_absolute(), "{done}");
        // A PLACE WITH NO ACCOUNT opens nothing, and says so.
        open.choose_at(Some(9), false, &mut way);
        assert!(open.view.picking().is_none());
    }

    /// FAILS IF: what the other button on an account's mark runs is not the
    /// command this view takes for a session — a right click would open
    /// nothing.
    #[test]
    fn the_marks_other_button_is_this_views_session() {
        assert_eq!(SESSION_COMMAND, agent_accounts::STARTS);
        assert!(is_command(SESSION_COMMAND));
    }

    /// FAILS IF: the terminal's words lose the folder, put it after the
    /// program (where it would be the agent's own argument), or let the
    /// agent's path be read as a flag of the terminal's.
    #[test]
    fn the_terminal_opens_the_agent_in_its_folder() {
        let argv = vec!["/nix/store/x-env/bin/env".to_owned(), "-u".to_owned()];
        assert_eq!(
            terminal_words(argv.clone(), Some(Path::new("/home/p/src"))),
            [
                "terminal",
                "--cwd",
                "/home/p/src",
                "--",
                "/nix/store/x-env/bin/env",
                "-u"
            ]
        );
        assert_eq!(
            terminal_words(argv, None),
            ["terminal", "--", "/nix/store/x-env/bin/env", "-u"]
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

    /// FAILS IF: a row wrapped over several lines in a narrow door lights
    /// and takes the click on its first line only — a line it continues
    /// onto takes nothing, says nothing under the pointer, or a click there
    /// lands anywhere but on the row.
    #[test]
    fn a_wrapped_row_is_one_control() {
        let t = texts("pt-BR");
        let r = readings();
        let mut v = View::default();
        let rows = v.rows(&r, &zone(), 30, &t);
        let (first, cont) = rows
            .windows(2)
            .enumerate()
            .find_map(|(n, w)| (w[0].target.is_some() && w[1].continues).then_some((n, n + 1)))
            .unwrap_or_else(|| panic!("no row wrapped at 30:\n{}", drawn(&rows)));
        assert!(
            rows[cont].takes(),
            "the line it continues onto takes nothing"
        );
        assert!(rows[cont].heading(), "the cursor stops on a continued line");
        assert_eq!(View::control_of(cont, &rows), first);
        assert_eq!(
            View::about(&rows, cont, &t),
            View::about(&rows, first, &t),
            "the continued line says another sentence"
        );
        // THE FIRST CLICK ON THE CONTINUED LINE SELECTS THE ROW, the second
        // is its `RET` — from a cursor somewhere else.
        let other = rows
            .iter()
            .position(|row| row.target.is_some())
            .filter(|n| *n != first)
            .or_else(|| rows.iter().rposition(|row| row.target.is_some()))
            .expect("another row that acts");
        assert_ne!(other, first);
        assert!(!v.clicked(other, &rows));
        assert!(!v.clicked(cont, &rows), "the first click was the `RET`");
        assert_eq!(v.cursor(), first, "the click landed off the row");
        assert!(v.clicked(cont, &rows), "the second click was not the `RET`");
    }

    /// An agent of `account`, its session made of `n`.
    fn running(account: &str, n: u32, doing: Doing, terminal: Option<&str>) -> AgentSeen {
        AgentSeen {
            account: account.to_owned(),
            session: format!("5f0c1e2a-0000-4000-8000-{n:012x}"),
            folder: format!("~/src/p{n}"),
            doing,
            since: NOW,
            terminal: terminal.map(str::to_owned),
            pid: 4000 + n,
            title: None,
            doing_now: None,
        }
    }

    /// A footer's doorway that writes down what it is asked.
    fn recorder(language: &str) -> crate::region::Recorder {
        crate::region::Recorder::new(
            std::rc::Rc::new(texts(language)),
            100,
            true,
            crate::sudo::Gate::shut(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// FAILS IF: an agent running in an account has no row in the account's
    /// section — its state's glyph (the one its mark wears on the modeline)
    /// and its folder, what it is doing in both languages and in the colour
    /// its mark wears — or its row's `RET`, its second click and the command
    /// by name with the cursor on it do not go to THAT agent by its session;
    /// or an agent of no listed account is drawn.
    #[test]
    fn an_agent_has_a_row_in_its_account_that_goes_to_it() {
        use Doing;
        for language in ronoctua_i18n::LANGUAGES {
            let t = texts(language);
            let mut r = readings();
            r.agents = vec![
                running("claude-1", 1, Doing::Waiting, Some("0123456789abcdef")),
                running("claude-1", 2, Doing::Failed, None),
                running("claude-1", 3, Doing::Working, None),
                running("claude-1", 4, Doing::Done, None),
                running("nobody", 5, Doing::Working, None),
            ];
            let mut v = View::default();
            let rows = v.rows(&r, &zone(), 100, &t);
            let of = |n: u32| {
                rows.iter()
                    .find(|row| {
                        row.target
                            == Some(Target::Agent(
                                "claude-1".to_owned(),
                                format!("5f0c1e2a-0000-4000-8000-{n:012x}"),
                            ))
                    })
                    .unwrap_or_else(|| {
                        panic!("{language}: agent {n} has no row:\n{}", drawn(&rows))
                    })
            };
            for (n, doing) in [
                (1, Doing::Waiting),
                (2, Doing::Failed),
                (3, Doing::Working),
                (4, Doing::Done),
            ] {
                let row = of(n);
                let text: String = row.fields.iter().map(|f| f.text.as_str()).collect();
                let glyph = tokens::icon::AGENT;
                assert!(
                    text.contains(glyph) && text.contains(&format!("~/src/p{n}")),
                    "{language}: {text}"
                );
                assert!(
                    text.contains(&t.t(words::doing_key(doing))),
                    "{language}: {text}"
                );
                // THE GLYPH ITS MARK WEARS on the modeline.
                assert_eq!(
                    crate::brainlink::glyph_of(&doing.mark().glyph),
                    Some(glyph),
                    "{doing:?}"
                );
            }
            assert!(
                rows.iter().all(
                    |row| !matches!(&row.target, Some(Target::Agent(id, _)) if id == "nobody")
                ),
                "{language}: an agent of no listed account was drawn"
            );
            // A SECTION WITH NO AGENT draws none: Codex's.
            assert_eq!(
                rows.iter()
                    .filter(
                        |row| matches!(&row.target, Some(Target::Agent(id, _)) if id == "codex-1")
                    )
                    .count(),
                0
            );
            // RET, AND THE SECOND CLICK, go to it.
            let waiting = Target::Agent(
                "claude-1".to_owned(),
                "5f0c1e2a-0000-4000-8000-000000000001".to_owned(),
            );
            stand_on(&mut v, &r, &waiting, &t);
            assert_eq!(
                v.press(Press::Enter, &r, &zone(), 100, &t),
                Errand::Show("5f0c1e2a-0000-4000-8000-000000000001".to_owned())
            );
            assert_eq!(
                View::about(&rows, v.cursor(), &t),
                Some(t.t("cmd-agents-show")),
                "{language}: the pointer resting on it says another thing"
            );
            assert!(
                v.clicked(v.cursor(), &rows),
                "the second click is not its RET"
            );
            assert_eq!(
                v.command(SHOW_COMMAND, &r, &zone(), 100, &t),
                Errand::Show("5f0c1e2a-0000-4000-8000-000000000001".to_owned())
            );
            // BY NAME off an agent, said.
            stand_on(&mut v, &r, &Target::Refresh, &t);
            assert_eq!(
                v.command(SHOW_COMMAND, &r, &zone(), 100, &t),
                Errand::Say(t.t(key::AGENTS_NOT_ON_AN_AGENT).into())
            );
        }
    }

    /// FAILS IF: an agent's row does not wear the colour its mark wears on
    /// the modeline — the brain module's tone for its state — for any state.
    #[test]
    fn an_agents_row_and_its_mark_wear_one_colour() {
        for doing in Doing::ALL {
            let mark =
                (doing.tone() != Tone::Quiet).then(|| crate::brainlink::tone_colour(doing.tone()));
            assert_eq!(doing_tint(doing), mark, "{doing:?}");
        }
        assert_eq!(doing_tint(Doing::Done), None);
        assert_eq!(
            doing_tint(Doing::Failed),
            Some(crate::brainlink::tone_colour(Tone::Danger))
        );
    }

    /// FAILS IF: an agent is not gone to where it is — one of our terminals
    /// by the terminal's own command and its id, any other by the footer's
    /// command for a process's window and its pid — after the door steps
    /// aside (the footer's region would keep the keyboard over the window
    /// that comes forward); or an agent that ended is gone to at all instead
    /// of said, in either language.
    #[test]
    fn an_agent_is_gone_to_where_it_is() {
        for (at, command, argument) in [
            (
                Whereabouts::Terminal("0123456789abcdef".to_owned()),
                TERMINAL_SHOW,
                "0123456789abcdef",
            ),
            (Whereabouts::Process(4242), WINDOW_OF_PROCESS, "4242"),
        ] {
            let mut w = recorder("en-GB");
            show_found(Some(at), &mut w);
            assert_eq!(
                w.owed,
                [
                    crate::region::Effect::StepAside,
                    crate::region::Effect::Run(command, Some(argument.to_owned()))
                ]
            );
        }
        for language in ronoctua_i18n::LANGUAGES {
            let mut w = recorder(language);
            show_found(None, &mut w);
            assert_eq!(
                w.owed,
                [crate::region::Effect::Say(
                    texts(language).t(key::AGENTS_AGENT_ENDED).into(),
                    Echo::Long
                )]
            );
        }
        // A PHOTOGRAPH'S agent is gone to nowhere, and written down.
        assert_eq!(
            running("claude-1", 1, Doing::Done, None)
                .whereabouts()
                .map(|w| order_of(&w)),
            Some((WINDOW_OF_PROCESS, "4001".to_owned()))
        );
        fn planted(_: &ronoctua_files::when::Zone) -> Readings {
            Readings {
                read: true,
                ..Readings::default()
            }
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
        let mut w = recorder("en-GB");
        open.command(SHOW_COMMAND, Some(" abc-1 "), &mut w);
        assert_eq!(open.done(), ["agents-show abc-1"]);
        assert!(
            !w.owed
                .iter()
                .any(|e| matches!(e, crate::region::Effect::Run(..))),
            "a photograph reached a window"
        );
    }

    /// FAILS IF: the view's command for an agent is not the one its mark
    /// opens on the modeline (the brain module's `SHOWS`), is not reached
    /// from the view — the keyboard's twin of the glyphs — or is not a
    /// command of the registry.
    #[test]
    fn the_agents_command_is_the_marks_and_the_registrys() {
        assert_eq!(SHOW_COMMAND, agent_accounts::SHOWS);
        assert!(reached().any(|c| c == SHOW_COMMAND));
        assert!(KEYS.commands.iter().any(|c| c.id == SHOW_COMMAND));
    }

    /// FAILS IF: an agents' folder that is there is not watched alone, or one
    /// not made yet is watched nowhere — its making (the first session the
    /// hook writes, the first terminal held) would never reach the view.
    #[test]
    fn the_agents_folders_are_watched_or_their_making_is() {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-shell-agents-folders-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("there")).unwrap();
        let mut w = Watching::default();
        watch_folder(root.join("there"), &mut w);
        watch_folder(root.join("later").join("agents"), &mut w);
        assert_eq!(w.dirs, [root.join("there")]);
        assert_eq!(w.ancestors, std::slice::from_ref(&root));
        let _ = std::fs::remove_dir_all(root);
    }
}
