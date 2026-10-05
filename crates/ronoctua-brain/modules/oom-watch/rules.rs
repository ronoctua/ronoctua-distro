//! The arithmetic of a kill: what each journal line says, and which lines are
//! one kill. Pure — lines in, kills out — so every rule is a test away.
//!
//! # Four lines, two kills
//!
//! A kill by the KERNEL writes its own line (`Killed process <pid> (<comm>)
//! …, anon-rss:…`) and then, when the process was in a unit, systemd writes
//! that the unit lost a process to it (`MESSAGE_ID` [`UNIT_OOM`]). A kill by
//! `systemd-oomd` writes oomd's decision (`Killed <cgroup> due to …`, or in a
//! newer systemd `Marked <cgroup> for killing due to …`) and systemd's word
//! about the unit (`MESSAGE_ID` [`UNIT_OOMD`]). Each pair is ONE kill: what the
//! first knows (the pid, the memory, the pressure) and what the second knows
//! (the unit) are put together, and it is told once.
//!
//! The two halves of a pair are written within a moment of each other, in
//! either order — the kernel's line reaches the journal through `/dev/kmsg`
//! and systemd's through a socket — so a half waits [`HOLD_S`] for its partner.
//!
//! # systemd's word is not a kill by itself
//!
//! systemd writes [`UNIT_OOM`] for every unit whose group's `oom_kill` count
//! went up, and that count covers the groups inside it: one kill in a scope
//! under the person's manager is also a line about `user@1000.service`, and
//! about the slices above it. And after a `daemon-reload` it writes the line
//! again for units whose count it forgot, with no kill at all (measured on a
//! real journal: `user.slice` and `user-1000.slice` right after "Reloading
//! finished", with no kernel line anywhere near). So:
//!
//! - a line about a unit that holds other units — a slice, or a manager's
//!   `user@<uid>.service` — is never a half of anything ([`inner`]);
//! - a kernel line takes the NEAREST unit line in time, never merely the first;
//! - a kernel line with no partner is told alone (a process in no unit has no
//!   systemd line), but a unit's line about the kernel with no kernel line is
//!   dropped: the kernel writes its line for every kill it makes, so a unit
//!   line without one is a count re-read, not a death.
//!
//! oomd's pairs are joined by identity — the unit is the last part of the
//! cgroup oomd names — and systemd's line about oomd reads a mark oomd leaves
//! on the very group it killed, so that one alone is still a kill.

use std::collections::VecDeque;

use crate::module::Entry;
use crate::moment::Moment;

/// systemd's `MESSAGE_ID` for "a process of this unit has been killed by the
/// OOM killer" (`SD_MESSAGE_UNIT_OUT_OF_MEMORY`, in its catalogue).
pub const UNIT_OOM: &str = "fe6faa94e7774663a0da52717891d8ef";

/// systemd's `MESSAGE_ID` for "systemd-oomd killed some process(es) in this
/// unit" (`SD_MESSAGE_UNIT_OOMD_KILL`, in its catalogue).
pub const UNIT_OOMD: &str = "d989611b15e44c9dbf31e3c81256e4ed";

/// The identifier `systemd-oomd` writes under.
pub const OOMD: &str = "systemd-oomd";

/// What every one of oomd's kill lines holds, in both spellings systemd has
/// used: `Killed <cgroup> due to memory …` and `Marked <cgroup> for killing
/// due to memory …`.
pub const OOMD_SAYS: &str = " due to memory ";

/// What the kernel's kill line holds. Not `oom_reaper`'s "reaped process",
/// which is the same process's memory coming back after the kill was written.
pub const KERNEL_SAYS: &str = "Killed process ";

/// How long, in seconds of the journal's own clock, the two halves of one
/// kill may lie apart — and how long, by the daemon's clock, a half waits for
/// its partner before it is told alone.
pub const HOLD_S: i64 = 3;

/// How far apart two moments may be and still be the same kill, for the
/// footer's matching and for the told keys: the journal's second against the
/// bus's arrival.
pub const SAME_KILL_S: i64 = 60;

/// How many told kills are remembered, so a line heard twice — a cursor
/// written a few seconds before a restart — is not told twice.
pub const TOLD_KEPT: usize = 64;

/// How many kills its page shows.
pub const RECENT_KEPT: usize = 20;

/// Who killed it, and against what — the selector of its sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// The kernel, for a reason its line did not reach us with — only
    /// systemd's word about the unit was heard.
    Kernel,
    /// The kernel, because the whole machine ran out of memory.
    Machine,
    /// The kernel, because the process's group reached its own limit
    /// (`MemoryMax=`): "Memory cgroup out of memory".
    Limit,
    /// systemd-oomd, for a reason its line did not reach us with.
    Oomd,
    /// systemd-oomd, because the machine was stalling for memory.
    Pressure,
    /// systemd-oomd, because memory and swap were nearly full.
    Full,
}

impl By {
    /// The word the sentence selects on.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Machine => "machine",
            Self::Limit => "limit",
            Self::Oomd => "oomd",
            Self::Pressure => "pressure",
            Self::Full => "full",
        }
    }
}

/// One line of a kill, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// The kernel's own line.
    Kernel {
        /// Machine-wide, or the group's limit.
        by: By,
        /// The process.
        pid: u32,
        /// Its name, as the kernel knows it (`comm`, fifteen bytes at most).
        comm: String,
        /// What it held in memory when it died, in bytes: anonymous, file and
        /// shared, added up.
        rss_bytes: u64,
    },
    /// systemd's line about the unit that lost a process to the kernel.
    UnitKernel {
        /// The unit.
        unit: String,
    },
    /// oomd's decision.
    Oomd {
        /// Pressure or fullness.
        by: By,
        /// The cgroup it killed, as a path under the cgroup root.
        cgroup: String,
        /// The stall it measured, in per mille of the time, when it was
        /// pressure.
        pressure_per_mille: Option<u16>,
        /// The limit that stall crossed, in per mille.
        limit_per_mille: Option<u16>,
        /// The memory in use when it was fullness, in bytes.
        used_bytes: Option<u64>,
    },
    /// systemd's line about the unit oomd took processes from.
    UnitOomd {
        /// The unit.
        unit: String,
    },
}

impl Piece {
    /// The line an entry is, if it is one of the four.
    #[must_use]
    pub fn of(entry: &Entry) -> Option<Self> {
        let field = |name: &str| entry.fields.get(name).map(String::as_str);
        match field("MESSAGE_ID") {
            Some(UNIT_OOM) => {
                return unit_of(entry)
                    .filter(|u| !inner(u))
                    .map(|unit| Self::UnitKernel { unit });
            }
            Some(UNIT_OOMD) => {
                return unit_of(entry)
                    .filter(|u| !inner(u))
                    .map(|unit| Self::UnitOomd { unit });
            }
            _ => {}
        }
        if field("_TRANSPORT") == Some("kernel") {
            return kernel(&entry.message);
        }
        if field("SYSLOG_IDENTIFIER") == Some(OOMD) {
            return oomd(&entry.message);
        }
        None
    }

    /// The key it is remembered under once told, with the second it was
    /// written: the pid for the kernel's line, the cgroup for oomd's, the
    /// unit for systemd's.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Kernel { pid, .. } => format!("pid:{pid}"),
            Self::Oomd { cgroup, .. } => format!("cgroup:{cgroup}"),
            Self::UnitKernel { unit } => format!("unit-kernel:{unit}"),
            Self::UnitOomd { unit } => format!("unit-oomd:{unit}"),
        }
    }

    /// Whether this and `other` are the two halves of one kill, whatever their
    /// order: the kernel's line with systemd's word about the kernel, oomd's
    /// with systemd's word about oomd — and for oomd, only when the unit is
    /// the cgroup's last part.
    fn pairs_with(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Kernel { .. }, Self::UnitKernel { .. })
            | (Self::UnitKernel { .. }, Self::Kernel { .. }) => true,
            (Self::Oomd { cgroup, .. }, Self::UnitOomd { unit })
            | (Self::UnitOomd { unit }, Self::Oomd { cgroup, .. }) => last_part(cgroup) == unit,
            _ => false,
        }
    }
}

/// The unit a systemd line is about: `USER_UNIT` from a person's manager,
/// `UNIT` from the system's.
fn unit_of(entry: &Entry) -> Option<String> {
    entry
        .fields
        .get("USER_UNIT")
        .or_else(|| entry.fields.get("UNIT"))
        .filter(|u| !u.is_empty())
        .cloned()
}

/// Whether a unit holds other units, so that its `oom_kill` count is the sum
/// of theirs: a slice, or the service a person's manager runs as
/// (`user@<uid>.service`), under which every unit of the session lives.
#[must_use]
pub fn inner(unit: &str) -> bool {
    unit.ends_with(".slice") || (unit.starts_with("user@") && unit.ends_with(".service"))
}

/// The last part of a cgroup's path: the unit it is.
#[must_use]
pub fn last_part(cgroup: &str) -> &str {
    cgroup.rsplit('/').next().unwrap_or(cgroup)
}

/// The kernel's kill line:
///
/// ```text
/// Memory cgroup out of memory: Killed process 4242 (tail) total-vm:1070592kB, anon-rss:31744kB, file-rss:1280kB, shmem-rss:0kB, UID:0 pgtables:1104kB oom_score_adj:0
/// ```
///
/// `None` for any other kernel line — or one whose pid is not a number, which
/// is not a kill this can name.
#[must_use]
pub fn kernel(message: &str) -> Option<Piece> {
    let at = message.find(KERNEL_SAYS)?;
    let before = message[..at].trim_end().trim_end_matches(':');
    let by = if before.starts_with("Memory cgroup out of memory") {
        By::Limit
    } else if before.starts_with("Out of memory") {
        By::Machine
    } else {
        By::Kernel
    };
    let rest = &message[at + KERNEL_SAYS.len()..];
    let (pid, rest) = rest.split_once(' ')?;
    let pid: u32 = pid.parse().ok()?;
    // THE NAME IS BETWEEN THE FIRST `(` AND THE `)` BEFORE THE FIGURES: a
    // process may call itself anything, parentheses and spaces included.
    let rest = rest.strip_prefix('(')?;
    let (comm, figures) = match rest.find(") total-vm:") {
        Some(end) => (&rest[..end], &rest[end + 2..]),
        None => {
            let end = rest.rfind(')')?;
            (&rest[..end], &rest[end + 1..])
        }
    };
    let kib = |name: &str| -> u64 {
        figures
            .split([',', ' '])
            .find_map(|f| f.strip_prefix(name))
            .and_then(|v| v.strip_suffix("kB"))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let rss_bytes = (kib("anon-rss:") + kib("file-rss:") + kib("shmem-rss:")).saturating_mul(1024);
    Some(Piece::Kernel {
        by,
        pid,
        comm: comm.to_owned(),
        rss_bytes,
    })
}

/// One of oomd's kill lines, in either spelling:
///
/// ```text
/// Killed /…/hog.service due to memory pressure for /…/app.slice being 58.63% > 50.00% for > 15s with reclaim activity
/// Marked /…/hog.service for killing due to memory used (1987654321) / total (2079612928) and swap used (0) / total (0) being more than 90.00%
/// ```
#[must_use]
pub fn oomd(message: &str) -> Option<Piece> {
    let rest = message
        .strip_prefix("Killed ")
        .or_else(|| message.strip_prefix("Marked "))?;
    let (cgroup, why) = rest.split_once(OOMD_SAYS)?;
    let cgroup = cgroup.strip_suffix(" for killing").unwrap_or(cgroup).trim();
    if cgroup.is_empty() {
        return None;
    }
    if let Some(pressure) = why.strip_prefix("pressure for ") {
        let (_, figures) = pressure.split_once(" being ")?;
        let mut shares = figures.split('%');
        let measured = shares.next().and_then(|s| per_mille(s.trim()));
        let limit = shares
            .next()
            .and_then(|s| per_mille(s.trim().trim_start_matches('>').trim()));
        return Some(Piece::Oomd {
            by: By::Pressure,
            cgroup: cgroup.to_owned(),
            pressure_per_mille: measured,
            limit_per_mille: limit,
            used_bytes: None,
        });
    }
    if let Some(used) = why.strip_prefix("used (") {
        let used_bytes = used.split(')').next().and_then(|n| n.parse().ok());
        return Some(Piece::Oomd {
            by: By::Full,
            cgroup: cgroup.to_owned(),
            pressure_per_mille: None,
            limit_per_mille: None,
            used_bytes,
        });
    }
    None
}

/// `58.63` as 586 per mille — rounded to the nearest, and never past the
/// whole.
fn per_mille(share: &str) -> Option<u16> {
    let (whole, fraction) = share.split_once('.').unwrap_or((share, ""));
    let whole: u32 = whole.parse().ok()?;
    let digits: Vec<u32> = fraction
        .chars()
        .take(2)
        .map(|c| c.to_digit(10))
        .collect::<Option<_>>()?;
    let tenths = digits.first().copied().unwrap_or(0);
    let hundredths = digits.get(1).copied().unwrap_or(0);
    let rounded = whole * 10 + tenths + u32::from(hundredths >= 5);
    u16::try_from(rounded.min(1000)).ok()
}

/// A line waiting for its partner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// What it said.
    pub piece: Piece,
    /// When the journal wrote it.
    pub written: Moment,
    /// When the brain heard it — its patience is counted from here.
    pub heard: Moment,
    /// The machine's stall when it was heard, in per mille, when it was
    /// heard fresh enough for that to be about the kill.
    pub stall_per_mille: Option<u16>,
}

/// One kill, whole: everything its lines said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kill {
    /// Who killed it, and against what.
    pub by: By,
    /// What died: the process's name, else the unit.
    pub what: String,
    /// The process, when the kernel said.
    pub pid: Option<u32>,
    /// Its unit, when systemd said.
    pub unit: Option<String>,
    /// Its cgroup, when oomd said.
    pub cgroup: Option<String>,
    /// Memory: what it held (the kernel), or what was in use (oomd).
    pub memory_bytes: Option<u64>,
    /// How stalled the machine was, in per mille of the time.
    pub stall_per_mille: Option<u16>,
    /// oomd's limit that stall crossed, in per mille.
    pub limit_per_mille: Option<u16>,
    /// When it happened, by the journal.
    pub at: Moment,
    /// The keys of the lines it was made of — what is remembered once told.
    pub keys: Vec<String>,
}

impl Kill {
    /// The kill one line, or two, make.
    #[must_use]
    pub fn of(first: &Waiting, second: Option<&Waiting>) -> Self {
        let mut kill = Self {
            by: By::Kernel,
            what: String::new(),
            pid: None,
            unit: None,
            cgroup: None,
            memory_bytes: None,
            stall_per_mille: first.stall_per_mille,
            limit_per_mille: None,
            at: first.written,
            keys: vec![first.piece.key()],
        };
        let mut unit_by = None;
        for w in std::iter::once(first).chain(second) {
            kill.at = kill.at.min(w.written);
            kill.stall_per_mille = kill.stall_per_mille.or(w.stall_per_mille);
            match &w.piece {
                Piece::Kernel {
                    by,
                    pid,
                    comm,
                    rss_bytes,
                } => {
                    kill.by = *by;
                    kill.pid = Some(*pid);
                    kill.what.clone_from(comm);
                    kill.memory_bytes = Some(*rss_bytes);
                }
                Piece::Oomd {
                    by,
                    cgroup,
                    pressure_per_mille,
                    limit_per_mille,
                    used_bytes,
                } => {
                    kill.by = *by;
                    kill.cgroup = Some(cgroup.clone());
                    if kill.what.is_empty() {
                        last_part(cgroup).clone_into(&mut kill.what);
                    }
                    // oomd's own measure of the stall is the one it acted on.
                    kill.stall_per_mille = pressure_per_mille.or(kill.stall_per_mille);
                    kill.limit_per_mille = *limit_per_mille;
                    kill.memory_bytes = used_bytes.or(kill.memory_bytes);
                }
                Piece::UnitKernel { unit } => {
                    kill.unit = Some(unit.clone());
                    unit_by = Some(By::Kernel);
                }
                Piece::UnitOomd { unit } => {
                    kill.unit = Some(unit.clone());
                    unit_by = Some(By::Oomd);
                }
            }
            if !std::ptr::eq(w, first) {
                kill.keys.push(w.piece.key());
            }
        }
        // A UNIT'S LINE ALONE: what died is the unit, and who killed it is
        // what the line says, without saying why. Only oomd's reaches here
        // from [`Pairing::due`]; the kernel's alone is dropped there.
        if kill.what.is_empty() {
            kill.what = kill.unit.clone().unwrap_or_default();
            if let (Some(by), None, None) = (unit_by, &kill.cgroup, kill.pid) {
                kill.by = by;
            }
        }
        kill
    }

    /// Where it was: its unit — or, when the unit is what died (oomd kills a
    /// whole unit), the cgroup that says which slice it was in.
    #[must_use]
    pub fn place(&self) -> Option<&str> {
        match (self.unit.as_deref(), self.cgroup.as_deref()) {
            (Some(unit), Some(cgroup)) if unit == self.what => Some(cgroup),
            (Some(unit), _) => Some(unit),
            (None, cgroup) => cgroup,
        }
    }

    /// Whether the machine's stall it carries was READ when it was heard —
    /// oomd's own measure is in its sentence already.
    #[must_use]
    pub fn stall_read(&self) -> bool {
        self.stall_per_mille.is_some() && !matches!(self.by, By::Pressure)
    }
}

/// The kills being put together, and the ones already told.
#[derive(Debug, Default)]
pub struct Pairing {
    /// Lines waiting for a partner, oldest first.
    pub waiting: Vec<Waiting>,
    /// What was told: each line's key with the second it was written.
    pub told: VecDeque<(String, Moment)>,
}

impl Pairing {
    /// Whether a line was already part of a kill that was told — heard again
    /// after a restart.
    #[must_use]
    pub fn was_told(&self, piece: &Piece, written: Moment) -> bool {
        let key = piece.key();
        self.told
            .iter()
            .any(|(k, at)| *k == key && (at - written).abs() <= HOLD_S)
    }

    /// One line heard. A kill when it completed one; otherwise it waits.
    pub fn heard(&mut self, line: Waiting) -> Option<Kill> {
        if self.was_told(&line.piece, line.written) {
            return None;
        }
        // THE SAME LINE TWICE, while it still waits, is one line.
        if self
            .waiting
            .iter()
            .any(|w| w.piece == line.piece && w.written == line.written)
        {
            return None;
        }
        // THE NEAREST IN TIME, never merely the first: two kills a second
        // apart each take their own unit's line.
        let partner = self
            .waiting
            .iter()
            .enumerate()
            .filter(|(_, w)| {
                w.piece.pairs_with(&line.piece) && (w.written - line.written).abs() <= HOLD_S
            })
            .min_by_key(|(_, w)| (w.written - line.written).abs())
            .map(|(at, _)| at);
        match partner {
            Some(at) => {
                let first = self.waiting.remove(at);
                let kill = Kill::of(&first, Some(&line));
                self.remember(&kill);
                Some(kill)
            }
            None => {
                self.waiting.push(line);
                None
            }
        }
    }

    /// The lines whose patience ran out by `now`, each told alone — but for
    /// systemd's word about the kernel, which alone is no kill (see the
    /// header) and is dropped.
    pub fn due(&mut self, now: Moment) -> Vec<Kill> {
        let (due, kept): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|w| w.heard + HOLD_S <= now);
        self.waiting = kept;
        due.iter()
            .filter(|w| !matches!(w.piece, Piece::UnitKernel { .. }))
            .map(|w| {
                let kill = Kill::of(w, None);
                self.remember(&kill);
                kill
            })
            .collect()
    }

    /// When the oldest waiting line runs out of patience.
    #[must_use]
    pub fn next_due(&self) -> Option<Moment> {
        self.waiting.iter().map(|w| w.heard + HOLD_S).min()
    }

    /// Remembers a told kill's lines, bounded.
    fn remember(&mut self, kill: &Kill) {
        for key in &kill.keys {
            self.told.push_back((key.clone(), kill.at));
        }
        while self.told.len() > TOLD_KEPT {
            self.told.pop_front();
        }
    }
}

/// Whether the journal wrote a line recently enough that the machine's stall
/// NOW says something about it: a line read after a restart, an hour late,
/// would carry the stall of a calm machine.
#[must_use]
pub const fn fresh(written: Moment, now: Moment) -> bool {
    now - written <= SAME_KILL_S
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waiting(piece: Piece, written: Moment) -> Waiting {
        Waiting {
            piece,
            written,
            heard: written,
            stall_per_mille: None,
        }
    }

    #[test]
    fn the_kernels_line_gives_the_pid_the_name_and_the_memory() {
        let got = kernel(
            "Memory cgroup out of memory: Killed process 4242 (tail) total-vm:1070592kB, \
             anon-rss:31744kB, file-rss:1280kB, shmem-rss:16kB, UID:0 pgtables:1104kB oom_score_adj:0",
        );
        assert_eq!(
            got,
            Some(Piece::Kernel {
                by: By::Limit,
                pid: 4242,
                comm: "tail".to_owned(),
                rss_bytes: (31_744 + 1_280 + 16) * 1024,
            })
        );
        // THE WHOLE MACHINE, and a name with a parenthesis and a space in it.
        let Some(Piece::Kernel { by, comm, .. }) = kernel(
            "Out of memory: Killed process 77 (Web Content (x)) total-vm:1kB, anon-rss:1kB, \
             file-rss:0kB, shmem-rss:0kB, UID:1000 pgtables:4kB oom_score_adj:100",
        ) else {
            panic!("a machine-wide kill was not read");
        };
        assert_eq!((by, comm.as_str()), (By::Machine, "Web Content (x)"));
        // THE REAPER'S LINE IS NOT A KILL, and neither is a line without a pid.
        assert_eq!(
            kernel("oom_reaper: reaped process 4242 (tail), now anon-rss:0kB, file-rss:0kB"),
            None
        );
        assert_eq!(kernel("Out of memory: Killed process x (y)"), None);
    }

    #[test]
    fn oomds_line_gives_the_cgroup_and_why_in_both_spellings() {
        let cg = "/user.slice/user-1000.slice/user@1000.service/app.slice/hog.service";
        for said in [
            format!(
                "Killed {cg} due to memory pressure for /user.slice/user-1000.slice/user@1000.service/app.slice \
                 being 58.63% > 50.00% for > 15s with reclaim activity"
            ),
            format!(
                "Marked {cg} for killing due to memory pressure for /user.slice/user-1000.slice/user@1000.service/app.slice \
                 being 58.63% > 50.00% for > 15s with reclaim activity"
            ),
        ] {
            assert_eq!(
                oomd(&said),
                Some(Piece::Oomd {
                    by: By::Pressure,
                    cgroup: cg.to_owned(),
                    pressure_per_mille: Some(586),
                    limit_per_mille: Some(500),
                    used_bytes: None,
                }),
                "{said}"
            );
        }
        let full = oomd(&format!(
            "Killed {cg} due to memory used (1987654321) / total (2079612928) and swap used (0) / total (0) being more than 90.00%"
        ));
        assert_eq!(
            full,
            Some(Piece::Oomd {
                by: By::Full,
                cgroup: cg.to_owned(),
                pressure_per_mille: None,
                limit_per_mille: None,
                used_bytes: Some(1_987_654_321),
            })
        );
        // oomd's other lines are not kills.
        assert_eq!(
            oomd(
                "Memory pressure for /app.slice is 60.00% > 50.00% for > 15s with reclaim activity"
            ),
            None
        );
        assert_eq!(
            oomd("Nothing killed when attempting to kill /app.slice/x.service"),
            None
        );
        assert_eq!(per_mille("0.04"), Some(0));
        assert_eq!(per_mille("0.05"), Some(1));
        assert_eq!(per_mille("100.00"), Some(1000));
        assert_eq!(per_mille("x"), None);
    }

    #[test]
    fn two_halves_are_one_kill_in_either_order() {
        let k = Piece::Kernel {
            by: By::Limit,
            pid: 9,
            comm: "tail".to_owned(),
            rss_bytes: 4096,
        };
        let u = Piece::UnitKernel {
            unit: "hog.service".to_owned(),
        };
        for (first, second) in [(k.clone(), u.clone()), (u.clone(), k.clone())] {
            let mut p = Pairing::default();
            assert_eq!(
                p.heard(waiting(first, 100)),
                None,
                "told before its partner came"
            );
            let kill = p
                .heard(waiting(second, 101))
                .expect("the pair made no kill");
            assert_eq!(kill.what, "tail");
            assert_eq!(kill.pid, Some(9));
            assert_eq!(kill.unit.as_deref(), Some("hog.service"));
            assert_eq!(kill.memory_bytes, Some(4096));
            assert_eq!(kill.by, By::Limit);
            assert_eq!(kill.at, 100);
            assert!(p.waiting.is_empty());
            // AND EITHER HALF HEARD AGAIN — a restart re-reads a few seconds —
            // is not a second kill.
            assert_eq!(p.heard(waiting(k.clone(), 100)), None);
            assert_eq!(p.heard(waiting(u.clone(), 101)), None);
            assert!(p.waiting.is_empty(), "a told line waits again");
        }
    }

    #[test]
    fn a_half_alone_is_told_when_its_patience_runs_out() {
        let mut p = Pairing::default();
        let alone = Piece::Kernel {
            by: By::Kernel,
            pid: 5,
            comm: "hog".to_owned(),
            rss_bytes: 1,
        };
        assert_eq!(p.heard(waiting(alone, 10)), None);
        assert_eq!(p.next_due(), Some(10 + HOLD_S));
        assert!(p.due(10 + HOLD_S - 1).is_empty(), "told before its time");
        let told = p.due(10 + HOLD_S);
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].unit, None);
        assert_eq!(p.next_due(), None, "a clock stays armed for nothing");
        // A KERNEL LINE AND A UNIT'S LINE FAR APART are two kills.
        let mut p = Pairing::default();
        let k = Piece::Kernel {
            by: By::Kernel,
            pid: 6,
            comm: "a".to_owned(),
            rss_bytes: 1,
        };
        let u = Piece::UnitKernel {
            unit: "b.service".to_owned(),
        };
        assert_eq!(p.heard(waiting(k, 10)), None);
        assert_eq!(p.heard(waiting(u, 10 + HOLD_S + 1)), None);
        assert_eq!(p.waiting.len(), 2);
    }

    #[test]
    fn oomds_halves_pair_only_by_the_unit_the_cgroup_ends_in() {
        let o = Piece::Oomd {
            by: By::Pressure,
            cgroup: "/app.slice/hog.service".to_owned(),
            pressure_per_mille: Some(700),
            limit_per_mille: Some(500),
            used_bytes: None,
        };
        let elsewhere = Piece::UnitOomd {
            unit: "other.service".to_owned(),
        };
        let mine = Piece::UnitOomd {
            unit: "hog.service".to_owned(),
        };
        let mut p = Pairing::default();
        assert_eq!(p.heard(waiting(o, 50)), None);
        assert_eq!(
            p.heard(waiting(elsewhere, 50)),
            None,
            "paired with another unit"
        );
        let kill = p
            .heard(waiting(mine, 51))
            .expect("its own unit made no kill");
        assert_eq!(kill.what, "hog.service");
        assert_eq!(kill.cgroup.as_deref(), Some("/app.slice/hog.service"));
        assert_eq!(kill.stall_per_mille, Some(700));
        assert_eq!(kill.limit_per_mille, Some(500));
        // THE UNIT IS WHAT DIED: where it was is the cgroup, with its slice.
        assert_eq!(kill.place(), Some("/app.slice/hog.service"));
        assert!(!kill.stall_read(), "oomd's measure read as the machine's");
        // A UNIT'S LINE ABOUT OOMD, ALONE, says oomd did it — and not why.
        let lone = Kill::of(
            &waiting(
                Piece::UnitOomd {
                    unit: "x.scope".to_owned(),
                },
                1,
            ),
            None,
        );
        assert_eq!((lone.what.as_str(), lone.by), ("x.scope", By::Oomd));
    }

    #[test]
    fn a_unit_that_holds_units_is_never_a_half() {
        for unit in [
            "user.slice",
            "user-1000.slice",
            "-.slice",
            "app.slice",
            "system.slice",
            "user@1000.service",
        ] {
            assert!(inner(unit), "{unit} taken for a leaf");
        }
        for unit in [
            "hog.service",
            "run-p5150-i9745490.scope",
            "session-2.scope",
            "user-runtime-dir@1000.service",
            "app-firefox@1a2b.service",
        ] {
            assert!(!inner(unit), "{unit} taken for a unit that holds units");
        }
    }

    #[test]
    fn systemds_word_about_the_kernel_alone_is_no_kill() {
        // A count re-read after a reload: no kernel line comes, and nothing is
        // told — while oomd's word alone still is, since oomd marks the very
        // group it killed.
        let mut p = Pairing::default();
        let unit = Piece::UnitKernel {
            unit: "hog.service".to_owned(),
        };
        assert_eq!(p.heard(waiting(unit, 10)), None);
        assert!(
            p.due(10 + HOLD_S).is_empty(),
            "a unit line alone was a kill"
        );
        assert!(p.waiting.is_empty(), "it still waits");
        assert_eq!(p.next_due(), None);
        let oomd = Piece::UnitOomd {
            unit: "hog.service".to_owned(),
        };
        assert_eq!(p.heard(waiting(oomd, 20)), None);
        assert_eq!(p.due(20 + HOLD_S).len(), 1);
    }

    #[test]
    fn a_kernel_line_takes_the_nearest_unit_line() {
        let kernel = |pid| Piece::Kernel {
            by: By::Limit,
            pid,
            comm: format!("p{pid}"),
            rss_bytes: 1,
        };
        let unit = |u: &str| Piece::UnitKernel { unit: u.to_owned() };
        let mut p = Pairing::default();
        assert_eq!(p.heard(waiting(kernel(1), 100)), None);
        assert_eq!(p.heard(waiting(kernel(2), 102)), None);
        let second = p
            .heard(waiting(unit("two.service"), 102))
            .expect("no kill of the second");
        assert_eq!(
            (second.pid, second.unit.as_deref()),
            (Some(2), Some("two.service")),
            "the first waiting line took a unit that was not its own"
        );
        let first = p
            .heard(waiting(unit("one.service"), 100))
            .expect("no kill of the first");
        assert_eq!(
            (first.pid, first.unit.as_deref()),
            (Some(1), Some("one.service"))
        );
    }

    #[test]
    fn what_is_told_is_remembered_and_bounded() {
        let mut p = Pairing::default();
        for pid in 0..(TOLD_KEPT as u32 + 10) {
            let _ = p.heard(waiting(
                Piece::Kernel {
                    by: By::Kernel,
                    pid,
                    comm: "x".to_owned(),
                    rss_bytes: 0,
                },
                1,
            ));
            let _ = p.due(1 + HOLD_S);
        }
        assert_eq!(p.told.len(), TOLD_KEPT);
        assert!(fresh(100, 100 + SAME_KILL_S));
        assert!(!fresh(100, 101 + SAME_KILL_S));
    }
}
