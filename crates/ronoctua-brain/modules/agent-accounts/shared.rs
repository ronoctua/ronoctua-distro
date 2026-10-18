//! THE LLM ACCOUNTS' SHARED HALF: what the accounts' view and the module both
//! read of the same machine — the lines a mark changes colour at, the colour
//! of a window, and the marks the accounts wear.
//!
//! It is compiled into the brain's LIBRARY (`ronoctua-brain`), beside the
//! manifest, because the footer's accounts view (`agents.rs`, which this
//! module's `namedBy` lists) reads it; `module.rs`, the daemon's half, sees it
//! through `super::`. So the bars of the view and the marks of the modeline
//! are coloured by ONE function, and a line kept by one door is the line the
//! other reads. Nothing here watches or runs: that is `module.rs`.

use std::path::PathBuf;
use std::sync::Arc;

use ronoctua_llm::statusline::Limits;
use ronoctua_llm::{Account, Kind as Agent, Window, usage};
use serde::{Deserialize, Serialize};

use crate::module::{Birth, Mark, MarkId, Tone};
use crate::moment::Moment;

/// The shell's command a click on a mark runs: the accounts' view, which the
/// click hands the mark's digit (`MarkId::opening`).
pub const OPENS: &str = "agents-open";

/// The person's lines, in the module's configuration folder.
pub const LINES_FILE: &str = "lines.json";

/// The two lines, in per cent of a window: a mark turns to a warning from
/// the first and to danger from the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Lines {
    /// A warning from this much of a window used.
    pub warn_percent: u8,
    /// Danger from this much.
    pub danger_percent: u8,
}

impl Default for Lines {
    /// The distro's: 80 and 95.
    fn default() -> Self {
        Self {
            warn_percent: 80,
            danger_percent: 95,
        }
    }
}

impl Lines {
    /// Whether they are two lines: each a whole per cent from 1 to 100, the
    /// warning below the danger.
    #[must_use]
    pub const fn fine(self) -> bool {
        self.warn_percent >= 1
            && self.warn_percent < self.danger_percent
            && self.danger_percent <= 100
    }
}

/// The colour a window used this much wears, by the lines: quiet below the
/// warning, a warning, then danger. Nothing known is quiet — the mark is
/// there, and says nothing yet.
#[must_use]
pub fn tone_of(used_percent: Option<f32>, lines: Lines) -> Tone {
    match used_percent {
        Some(p) if p >= f32::from(lines.danger_percent) => Tone::Danger,
        Some(p) if p >= f32::from(lines.warn_percent) => Tone::Warning,
        _ => Tone::Quiet,
    }
}

/// The most used of `windows` that has not reset at `now_s` — the one an
/// account's colour follows.
#[must_use]
pub fn most_used(windows: &[Window], now_s: i64) -> Option<f32> {
    windows
        .iter()
        .filter(|w| w.resets_at_s > now_s && w.used_percent.is_finite())
        .map(|w| w.used_percent)
        .reduce(f32::max)
}

/// Every window an account's numbers say, reset or not: what its status
/// line (Claude Code) or its logs (Codex) last said, and the usage
/// endpoint's last answer with its weekly windows of a model each. Two
/// readings of one window are its two moments; [`most_used`] takes the
/// higher, since a window only fills until it resets. What the marks are
/// coloured from — and what the accounts' view hands this to, to prove its
/// bars wear the same colour.
#[must_use]
pub fn windows_said(limits: Option<Limits>, usage: Option<&usage::Kept>) -> Vec<Window> {
    let mut out = Vec::new();
    if let Some(limits) = limits {
        out.extend(limits.five_hour);
        out.extend(limits.seven_day);
    }
    if let Some(u) = usage.and_then(|k| k.usage) {
        out.extend(
            [
                u.five_hour,
                u.seven_day,
                u.seven_day_opus,
                u.seven_day_sonnet,
            ]
            .into_iter()
            .flatten(),
        );
    }
    out
}

/// The colour an account's mark wears at `now`: its most used window that
/// has not reset, by the person's lines.
#[must_use]
pub fn mark_tone(windows: &[Window], lines: Lines, now: Moment) -> Tone {
    tone_of(most_used(windows, now), lines)
}

/// The person's lines, as the file says them now — the distro's when there
/// is no file, or one that does not hold two lines.
#[must_use]
pub fn lines() -> Lines {
    read_lines(&lines_store(own_config()))
}

/// Where the person's lines are kept — what a door watches to repaint its
/// bars when they change.
#[must_use]
pub fn lines_file() -> Option<PathBuf> {
    lines_store(own_config()).path()
}

/// One of the two lines, changed alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// The warning's.
    Warn,
    /// The danger's.
    Danger,
}

/// Why a line was not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unkept {
    /// The warning would not come before the danger.
    NotInOrder,
    /// The file is there and does not read: nothing is written over it.
    Unreadable,
    /// The file could not be written.
    Unwritten,
}

impl Unkept {
    /// The module's word for it — the page's refusal.
    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::NotInOrder => super::why::NOT_IN_ORDER,
            Self::Unreadable => super::why::UNREADABLE,
            Self::Unwritten => super::why::UNKEPT,
        }
    }
}

/// Keeps ONE of the person's lines, laid over what the file says NOW — the
/// footer's view, the window and the module's page all write it, and a copy
/// read earlier by one of them must not put back the line another changed.
///
/// # Errors
///
/// [`Unkept`]: the two lines would cross, the file does not read, or it could
/// not be written.
pub fn keep_line(which: Which, percent: u8) -> Result<Lines, Unkept> {
    write_line(&lines_store(own_config()), which, percent)
}

/// The mark an agent's accounts wear — its glyph token is what the
/// accounts' view draws them with.
#[must_use]
pub fn mark_of(agent: Agent) -> Option<&'static MarkId> {
    super::MANIFEST.mark(agent.id())
}

/// The module's configuration folder, under the machine's roots.
fn own_config() -> Option<PathBuf> {
    Birth {
        zone: Arc::new(crate::moment::Fixed(0)),
        roots: crate::prefs::Where::of_env(),
    }
    .config_dir(super::ID)
}

/// The lines' store, in a configuration folder.
#[must_use]
pub fn lines_store(dir: Option<PathBuf>) -> ronoctua_store::Store<Lines> {
    ronoctua_store::Store::at(dir, LINES_FILE)
}

/// The marks this module would put on the modeline now, read once from the
/// machine's files — what a photograph's mind is made of
/// (`photograph --door footer --accounts-marks`), so a picture never shows a
/// mark the module cannot draw. The module's own reading ([`accounts_now`],
/// [`windows_now`], [`lines`]) and its own [`marks_for`], and no watch.
#[must_use]
pub fn marks_read_now(now: Moment) -> Vec<Mark> {
    // A LIST THAT DOES NOT READ keeps no marks it cannot vouch for — the
    // module's own rule (`Kept::unreadable`).
    let seen: Vec<Seen> = accounts_now()
        .unwrap_or_default()
        .iter()
        .map(|a| Seen::of(a, windows_now(a)))
        .collect();
    marks_for(&seen, lines(), now)
}

/// What a store says, two lines or the distro's.
#[must_use]
pub fn read_lines(store: &ronoctua_store::Store<Lines>) -> Lines {
    Some(store.read()).filter(|l| l.fine()).unwrap_or_default()
}

/// Writes one line over what the file says now — `parsed`, never `read`:
/// this writes the whole file back, and a file that does not read is
/// refused, never replaced. A file that reads and does not hold two lines is
/// the distro's, as every reader takes it.
///
/// # Errors
///
/// [`Unkept`]: the two lines would cross, the file does not read, or it could
/// not be written.
pub fn write_line(
    store: &ronoctua_store::Store<Lines>,
    which: Which,
    percent: u8,
) -> Result<Lines, Unkept> {
    let now = store.parsed().ok_or(Unkept::Unreadable)?;
    let mut lines = if now.fine() { now } else { Lines::default() };
    match which {
        Which::Warn => lines.warn_percent = percent,
        Which::Danger => lines.danger_percent = percent,
    }
    if !lines.fine() {
        return Err(Unkept::NotInOrder);
    }
    store.write(&lines).map_err(|e| {
        tracing::warn!(error = %e, "the LLM accounts' lines could not be written");
        Unkept::Unwritten
    })?;
    Ok(lines)
}

// ---------------------------------------------------------------------------
// The accounts, as the marks need them — read by the module's watches and by
// a photograph alike.
// ---------------------------------------------------------------------------

/// One account, as the marks need it.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    /// The account's id in the person's list.
    pub id: String,
    /// Whose agent it is.
    pub agent: Agent,
    /// The person's word for it — the mark's label.
    pub name: String,
    /// Every window its numbers say, reset or not ([`windows_said`]).
    pub windows: Vec<Window>,
}

impl Seen {
    /// An account and the windows read for it.
    #[must_use]
    pub fn of(account: &Account, windows: Vec<Window>) -> Self {
        Self {
            id: account.id.clone(),
            agent: account.kind,
            name: account.name.clone(),
            windows,
        }
    }
}

/// The person's accounts, as the one crate that reads the agents' state keeps
/// the list; `None` when the list is there and does not read.
#[must_use]
pub fn accounts_now() -> Option<Vec<Account>> {
    ronoctua_llm::Accounts::store()
        .parsed()
        .map(|list| list.accounts)
}

/// Every window an account's numbers say now: the status line's (Claude Code)
/// or the logs' (Codex), and the endpoint's last answer — put together by the
/// one function the view asks too ([`windows_said`]).
#[must_use]
pub fn windows_now(account: &Account) -> Vec<Window> {
    windows_said(
        ronoctua_llm::statusline::limits(account).ok(),
        ronoctua_llm::usage::kept(account).as_ref(),
    )
}

/// How many accounts carry a mark: the instances the manifest declares (one
/// digit), the same for both agents.
#[must_use]
pub fn marked_most() -> usize {
    super::MANIFEST
        .marks
        .iter()
        .map(|m| usize::from(m.instances))
        .min()
        .unwrap_or(0)
}

/// The marks: one per account, by its place in the list, coloured by the
/// person's lines.
#[must_use]
pub fn marks_for(accounts: &[Seen], lines: Lines, now: Moment) -> Vec<Mark> {
    accounts
        .iter()
        .take(marked_most())
        .enumerate()
        .filter_map(|(n, seen)| {
            let declared = mark_of(seen.agent)?;
            let place = u8::try_from(n + 1).ok()?;
            Some(Mark {
                id: declared
                    .numbered(place, Some(seen.name.clone()))
                    .opening(OPENS),
                tone: mark_tone(&seen.windows, lines, now),
                count: 0,
                act: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noon on 2026-10-05, UTC.
    const NOON: Moment = 1_791_201_600;

    fn window(used_percent: f32, resets_at_s: i64) -> Window {
        Window {
            used_percent,
            resets_at_s,
        }
    }

    fn seen(id: &str, agent: Agent, name: &str, windows: Vec<Window>) -> Seen {
        Seen {
            id: id.to_owned(),
            agent,
            name: name.to_owned(),
            windows,
        }
    }

    /// FAILS IF: the marks a photograph draws (`marks_read_now`, through
    /// `marks_for`) are numbered by anything but the place in the list,
    /// named by anything but the person's word, open anything but the
    /// accounts' view, are coloured by a window that already reset, or go
    /// past the one digit the manifest declares.
    #[test]
    fn the_marks_are_numbered_by_place_and_coloured_by_the_lines() {
        let mut list = vec![
            seen(
                "claude-1",
                Agent::Claude,
                "Work",
                vec![window(96.0, NOON + 60)],
            ),
            seen(
                "codex-1",
                Agent::Codex,
                "Home",
                vec![window(99.0, NOON - 60), window(81.0, NOON + 60)],
            ),
        ];
        for n in 2..=10 {
            list.push(seen(
                &format!("codex-{n}"),
                Agent::Codex,
                &format!("C{n}"),
                Vec::new(),
            ));
        }
        let marks = marks_for(&list, Lines::default(), NOON);
        assert_eq!(marks.len(), marked_most(), "a digit past the manifest's");
        assert!(marked_most() <= 9, "a mark's digit is one");
        let (first, second) = (&marks[0], &marks[1]);
        assert_eq!(
            (first.id.instance, first.id.label.as_deref()),
            (1, Some("Work"))
        );
        assert_eq!(first.id.opens.as_deref(), Some(OPENS));
        assert_eq!(first.tone, Tone::Danger);
        // THE RESET WINDOW IS GONE: 99 % that reset a minute ago, and 81 %
        // that has not, is a warning.
        assert_eq!((second.id.instance, second.tone), (2, Tone::Warning));
        assert_eq!(
            second.id.name,
            mark_of(Agent::Codex).expect("codex has a mark").name
        );
        // AND A WINDOW NOBODY KNOWS is quiet: the mark is there, and says
        // nothing yet.
        assert_eq!(marks[2].tone, Tone::Quiet);
    }
}
