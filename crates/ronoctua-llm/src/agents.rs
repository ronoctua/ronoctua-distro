//! The agents — Claude Code's sessions, each in its account — out of what the
//! managed hook kept and what our terminals heard. One reader, pure.
//!
//! # Two sources, one answer
//!
//! - **The hook's records** ([`crate::sessions`]) say WHO a session is — its
//!   account's folder, its process, our terminal when it runs in one — for a
//!   Claude Code in any terminal, an editor's included; and a status, as far
//!   as the events tell it.
//! - **OSC 7501**, the protocol a program tells its terminal its state in
//!   (`state=working|blocked|done|idle|error`, `app=claude-code`), heard by
//!   our terminals ([`Program`]): Claude Code's own word, sent the moment its
//!   state changes. Claude Code 2.1.314 sends the session's state with NO
//!   `id`, and one record per background task with the task's id; so a
//!   record with no id is the session's, one whose id IS the session's id
//!   is too, and any other is a task of it, not an agent.
//!
//! Inside one of our terminals the program's word wins; anywhere else, or
//! when it said nothing (yet, or since a `clear`), the hook's does.
//!
//! # What is not an agent
//!
//! A session that ended (a tombstone), one whose process is not the one it
//! was ([`Alive::process`]: the pid and its start), one whose terminal is no
//! longer held ([`Alive::terminal`]), and one whose account's folder no
//! listed Claude Code account has — over ssh, a `CLAUDE_CONFIG_DIR` the
//! person never added: not drawn in any account's set (owner, 2026-11-02).
//!
//! # The one place that knows the app
//!
//! [`CLAUDE_CODE`] is spelled here and nowhere else: the terminals keep any
//! program's records, and the census crate is not this one's dependency —
//! its records come in as [`Program`]s.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::sessions::{Session, Status, same_folder};
use crate::{Account, Kind};

/// Claude Code's name for itself in OSC 7501 (`app=`).
pub const CLAUDE_CODE: &str = "claude-code";

/// One record OSC 7501 left in one of our terminals, as the terminals'
/// census keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The terminal it was heard in.
    pub terminal: String,
    /// The record's id, when the program gave one (`id=`).
    pub id: Option<String>,
    /// Which program (`app=`).
    pub app: String,
    /// Its state, as it said it (`state=`).
    pub state: String,
}

/// What the reader asks of the machine.
pub trait Alive {
    /// Whether `pid` is still the process that started at `start_ticks`.
    fn process(&self, pid: u32, start_ticks: u64) -> bool;
    /// Whether one of our terminals is still held.
    fn terminal(&self, id: &str) -> bool;
}

/// The machine's answers: a process by `/proc` ([`crate::sessions::running`]),
/// a terminal by the function it is made with — the caller's question to
/// the terminals' census, which this crate does not read.
pub struct OnThisMachine<F: Fn(&str) -> bool>(pub F);

impl<F: Fn(&str) -> bool> Alive for OnThisMachine<F> {
    fn process(&self, pid: u32, start_ticks: u64) -> bool {
        crate::sessions::running(pid, start_ticks)
    }

    fn terminal(&self, id: &str) -> bool {
        (self.0)(id)
    }
}

/// One agent: a live Claude Code session, in its account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Agent {
    /// Its account's id.
    pub account: String,
    /// Claude Code's id of the session.
    pub session: String,
    /// The folder it works in.
    pub folder: PathBuf,
    /// What it is doing.
    pub status: Status,
    /// Our terminal it runs in, when it is one.
    #[serde(default)]
    pub terminal: Option<String>,
    /// Claude Code's process — what a window running it is found by.
    #[serde(default)]
    pub pid: u32,
    /// When it was first heard, in nanoseconds since the epoch: its place
    /// among its account's agents.
    #[serde(default)]
    pub started_at_ns: u64,
}

/// A program's state, as a session's status — `None` for a state this does
/// not know, which leaves the hook's word in force rather than guessing.
#[must_use]
pub fn status_of_program(state: &str) -> Option<Status> {
    match state {
        "working" => Some(Status::Working),
        "blocked" => Some(Status::Waiting),
        "done" | "idle" => Some(Status::Done),
        "error" => Some(Status::Failed),
        _ => None,
    }
}

/// The live agents, each in its listed account: in the accounts' order, then
/// by when each was first heard, then by session.
#[must_use]
pub fn merged(
    accounts: &[Account],
    sessions: &[Session],
    programs: &[Program],
    alive: &dyn Alive,
) -> Vec<Agent> {
    let mut out: Vec<(usize, Agent)> = Vec::new();
    for s in sessions {
        if s.ended || !alive.process(s.pid, s.pid_start_ticks) {
            continue;
        }
        if s.terminal.as_deref().is_some_and(|t| !alive.terminal(t)) {
            continue;
        }
        let Some((place, account)) = accounts.iter().enumerate().find(|(_, a)| {
            matches!(a.kind, Kind::Claude | Kind::ClaudeApi) && same_folder(&a.dir, &s.account_dir)
        }) else {
            continue;
        };
        let told = said_of(s, programs).and_then(|p| status_of_program(&p.state));
        out.push((
            place,
            Agent {
                account: account.id.clone(),
                session: s.session.clone(),
                folder: s.folder.clone(),
                status: told.unwrap_or(s.status),
                terminal: s.terminal.clone(),
                pid: s.pid,
                started_at_ns: s.started_at_ns,
            },
        ));
    }
    out.sort_by(|(pa, a), (pb, b)| {
        (pa, a.started_at_ns, &a.session).cmp(&(pb, b.started_at_ns, &b.session))
    });
    out.into_iter().map(|(_, a)| a).collect()
}

/// The OSC 7501 record that is `session`'s own: Claude Code's, with the
/// session's id or with none — in the terminal the session runs in for the
/// latter. A task's record (another id, or one with a `/`, a subagent's) is
/// never the session's.
fn said_of<'a>(session: &Session, programs: &'a [Program]) -> Option<&'a Program> {
    let mine =
        |p: &&Program| p.app == CLAUDE_CODE && !p.id.as_deref().is_some_and(|i| i.contains('/'));
    programs
        .iter()
        .filter(mine)
        .find(|p| p.id.as_deref() == Some(session.session.as_str()))
        .or_else(|| {
            let terminal = session.terminal.as_deref()?;
            programs
                .iter()
                .filter(mine)
                .find(|p| p.terminal == terminal && p.id.as_deref().is_none_or(str::is_empty))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::tests::record;

    const T1: &str = "0123456789abcdef";
    const T2: &str = "fedcba9876543210";

    /// Every process alive but the ones named; every terminal held but the
    /// ones named.
    struct Planted {
        dead: Vec<(u32, u64)>,
        gone: Vec<String>,
    }

    impl Alive for Planted {
        fn process(&self, pid: u32, start_ticks: u64) -> bool {
            !self.dead.contains(&(pid, start_ticks))
        }
        fn terminal(&self, id: &str) -> bool {
            !self.gone.contains(&id.to_owned())
        }
    }

    const ALL_ALIVE: Planted = Planted {
        dead: Vec::new(),
        gone: Vec::new(),
    };

    fn account(id: &str, kind: Kind, dir: &str) -> Account {
        Account {
            id: id.to_owned(),
            kind,
            name: id.to_owned(),
            dir: dir.into(),
            default: false,
        }
    }

    fn accounts() -> Vec<Account> {
        vec![
            account("claude-1", Kind::Claude, "/home/p/.claude"),
            account("claude-api-1", Kind::ClaudeApi, "/home/p/.claude-api"),
            account("codex-1", Kind::Codex, "/home/p/.codex"),
        ]
    }

    fn session(id: &str, status: Status, terminal: Option<&str>) -> Session {
        Session {
            status,
            terminal: terminal.map(str::to_owned),
            ..record(id)
        }
    }

    fn program(terminal: &str, id: Option<&str>, state: &str) -> Program {
        Program {
            terminal: terminal.to_owned(),
            id: id.map(str::to_owned),
            app: CLAUDE_CODE.to_owned(),
            state: state.to_owned(),
        }
    }

    fn statuses(agents: &[Agent]) -> Vec<(&str, Status)> {
        agents
            .iter()
            .map(|a| (a.session.as_str(), a.status))
            .collect()
    }

    /// FAILS IF: inside one of our terminals the hook's word beats Claude
    /// Code's own, a background task's record (another id) or another
    /// program's is taken for the session's, a record of another terminal is
    /// joined by terminal, one carrying the session's id is not joined, or
    /// outside our terminals the hook's word is not the one drawn.
    #[test]
    fn merged_prefers_osc_7501_inside_a_terminal() {
        let sessions = [
            session("aa", Status::Done, Some(T1)),
            session("bb", Status::Working, Some(T2)),
            session("cc", Status::Waiting, None),
            session("dd", Status::Working, None),
        ];
        let programs = [
            program(T1, Some("task-1"), "working"),
            Program {
                app: "another".to_owned(),
                ..program(T2, None, "error")
            },
            program(T2, None, "blocked"),
            program(T1, Some("dd"), "idle"),
        ];
        let agents = merged(&accounts(), &sessions, &programs, &ALL_ALIVE);
        assert_eq!(
            statuses(&agents),
            [
                ("aa", Status::Done),
                ("bb", Status::Waiting),
                ("cc", Status::Waiting),
                ("dd", Status::Done),
            ]
        );
        assert!(agents.iter().all(|a| a.account == "claude-1"));
    }

    /// FAILS IF: every state OSC 7501 says is not the status it means, or
    /// one this does not know overrides the hook's word.
    #[test]
    fn every_program_state_is_the_status_it_means() {
        for (state, status) in [
            ("working", Some(Status::Working)),
            ("blocked", Some(Status::Waiting)),
            ("done", Some(Status::Done)),
            ("idle", Some(Status::Done)),
            ("error", Some(Status::Failed)),
            ("clear", None),
            ("WORKING", None),
        ] {
            assert_eq!(status_of_program(state), status, "{state}");
        }
        let agents = merged(
            &accounts(),
            &[session("aa", Status::Waiting, Some(T1))],
            &[program(T1, None, "pondering")],
            &ALL_ALIVE,
        );
        assert_eq!(statuses(&agents), [("aa", Status::Waiting)]);
    }

    /// FAILS IF: a subagent's record, a tombstone, a process that is gone
    /// (or a later one with its pid) or a terminal no longer held is drawn
    /// as an agent.
    #[test]
    fn subagents_and_tombstones_are_dropped() {
        let ended = Session {
            ended: true,
            ..session("aa", Status::Working, None)
        };
        let reused = Session {
            pid: 77,
            pid_start_ticks: 2,
            ..session("bb", Status::Working, None)
        };
        let alive = Planted {
            dead: vec![(77, 2)],
            gone: vec![T2.to_owned()],
        };
        let sessions = [
            ended,
            reused,
            session("cc", Status::Working, Some(T2)),
            session("dd", Status::Done, Some(T1)),
        ];
        let programs = [program(T1, Some("dd/agent-1"), "error")];
        let agents = merged(&accounts(), &sessions, &programs, &alive);
        assert_eq!(
            statuses(&agents),
            [("dd", Status::Done)],
            "the subagent's error is not dd's"
        );
    }

    /// FAILS IF: an agent is put in an account by anything but the folder it
    /// runs in, as written (slashes and `.` aside), a folder no listed
    /// account has is drawn in one, or a Codex account takes a Claude Code
    /// session.
    #[test]
    fn an_agent_belongs_to_the_account_whose_folder_it_runs_in() {
        let mut list = accounts();
        list.push(account("claude-2", Kind::Claude, "/home/p/.codex-like"));
        let sessions = [
            Session {
                account_dir: "/home/p/.claude-api/".into(),
                ..session("aa", Status::Done, None)
            },
            Session {
                account_dir: "/home/p/./.claude".into(),
                ..session("bb", Status::Done, None)
            },
            Session {
                account_dir: "/home/p/.codex".into(),
                ..session("cc", Status::Done, None)
            },
            Session {
                account_dir: "/srv/elsewhere/.claude".into(),
                ..session("dd", Status::Done, None)
            },
        ];
        let agents = merged(&list, &sessions, &[], &ALL_ALIVE);
        let placed: Vec<(&str, &str)> = agents
            .iter()
            .map(|a| (a.session.as_str(), a.account.as_str()))
            .collect();
        assert_eq!(placed, [("bb", "claude-1"), ("aa", "claude-api-1")]);
    }

    /// FAILS IF: the agents are not in their accounts' order and, within
    /// one, by when each was first heard — a set's glyphs would trade places
    /// at every reading.
    #[test]
    fn the_agents_keep_their_places() {
        let at = |id: &str, dir: &str, started: u64| Session {
            account_dir: dir.into(),
            started_at_ns: started,
            ..session(id, Status::Done, None)
        };
        let sessions = [
            at("aa", "/home/p/.claude-api", 1),
            at("bb", "/home/p/.claude", 30),
            at("cc", "/home/p/.claude", 20),
            at("dd", "/home/p/.claude", 20),
        ];
        let order: Vec<String> = merged(&accounts(), &sessions, &[], &ALL_ALIVE)
            .into_iter()
            .map(|a| a.session)
            .collect();
        assert_eq!(order, ["cc", "dd", "bb", "aa"]);
    }

    /// FAILS IF: an agent does not cross a file and back — what a
    /// photograph plants — or one written with only what the picture needs
    /// is refused.
    #[test]
    fn an_agent_crosses_for_a_photograph() {
        let agents = merged(
            &accounts(),
            &[session("aa", Status::Waiting, Some(T1))],
            &[],
            &ALL_ALIVE,
        );
        let text = serde_json::to_string(&agents).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Agent>>(&text).unwrap(), agents);
        let planted: Vec<Agent> = serde_json::from_str(
            r#"[{"account":"claude-1","session":"aa","folder":"/home/p/work","status":"waiting"}]"#,
        )
        .unwrap();
        assert_eq!((planted[0].status, planted[0].pid), (Status::Waiting, 0));
    }
}
