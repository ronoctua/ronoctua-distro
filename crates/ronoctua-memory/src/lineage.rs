//! Whose child a process is, and whether a pid is still the process it was.
//!
//! # Two questions of `/proc/<pid>/stat`
//!
//! A record that names a process by its pid — the managed Claude Code hook's
//! sessions (`ronoctua_llm::sessions`) — outlives the process: the kernel
//! hands a pid to the next process once the first is reaped, and `kill(pid,
//! 0)` then answers yes for a stranger. So a record keeps the pid AND the
//! moment the kernel started that process ([`Stat::start_ticks`], the 22nd
//! field of `stat`, in clock ticks since boot), and [`is_running`] asks both:
//! a reused pid has a later start, and is not the same process.
//!
//! And whose child it is: the hook runs as a grandchild of Claude Code when a
//! shell does not `exec`, and a window is matched to a program started under
//! it (`ancestry`), walking the fourth field upwards.
//!
//! # Why here
//!
//! This crate is the tree's one boundary with `/proc` (the camera's holders
//! walk the same file, [`crate::camera`]); the parser is pure, a `&str` in and
//! numbers out, and the readers take the root as a parameter so a test plants
//! a tree of its own.

use std::path::Path;

/// How many parents [`ancestry`] reads at most: far more than any session's
/// depth (a terminal, a shell, a program, a shell it runs), and a bound on a
/// walk a process tree that changes under it could otherwise keep going.
pub const MOST_ANCESTORS: usize = 32;

/// What `/proc/<pid>/stat` says of a process, as far as these questions go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    /// Its state letter (`R`, `S`, `D`, `Z`…).
    pub state: char,
    /// Its parent's pid (0 for the kernel's own).
    pub parent: u32,
    /// When the kernel started it, in clock ticks since boot: what tells it
    /// from a later process given the same pid.
    pub start_ticks: u64,
}

impl Stat {
    /// Whether it is a process that runs — not one that ended and waits to be
    /// reaped (`Z`), or is being taken away (`X`, `x`).
    #[must_use]
    pub const fn lives(&self) -> bool {
        !matches!(self.state, 'Z' | 'X' | 'x')
    }
}

/// The fields of one `stat` line, counted from the LAST `)`: the name before
/// it is the program's, in parentheses, and may itself hold spaces and
/// parentheses. `None` for a line that is not one.
#[must_use]
pub fn parse_stat(content: &str) -> Option<Stat> {
    let after = &content[content.rfind(')')? + 1..];
    let mut fields = after.split_whitespace();
    // FIELD 3 IS THE STATE, 4 THE PARENT, AND 22 THE START: counted from the
    // state, the start is the twentieth.
    let state = fields.next()?.chars().next()?;
    let parent = fields.next()?.parse().ok()?;
    let start_ticks = fields.nth(17)?.parse().ok()?;
    Some(Stat {
        state,
        parent,
        start_ticks,
    })
}

/// `pid`'s `stat`, under a `/proc` root; `None` when it is gone or not ours
/// to read.
#[must_use]
pub fn stat_in(proc_root: &Path, pid: u32) -> Option<Stat> {
    let text = crate::read(&proc_root.join(pid.to_string()).join("stat"))?;
    parse_stat(&text)
}

/// When the process `pid` started, in clock ticks since boot — `None` when
/// there is no such process now.
#[must_use]
pub fn start_ticks(pid: u32) -> Option<u64> {
    start_ticks_in(Path::new("/proc"), pid)
}

/// [`start_ticks`], under a `/proc` root.
#[must_use]
pub fn start_ticks_in(proc_root: &Path, pid: u32) -> Option<u64> {
    stat_in(proc_root, pid)
        .filter(Stat::lives)
        .map(|s| s.start_ticks)
}

/// Whether `pid` is still the process that started at `start_ticks`: there,
/// running (not a zombie), and not a later one handed the same pid.
#[must_use]
pub fn is_running(pid: u32, start_ticks: u64) -> bool {
    is_running_in(Path::new("/proc"), pid, start_ticks)
}

/// [`is_running`], under a `/proc` root.
#[must_use]
pub fn is_running_in(proc_root: &Path, pid: u32, start_ticks: u64) -> bool {
    pid > 0 && start_ticks_in(proc_root, pid) == Some(start_ticks)
}

/// The pids above `pid` — its parent first, then the parent's — up to
/// [`MOST_ANCESTORS`] of them. The walk ends at the root every process ends
/// at (1, which names no program anybody started), at the kernel's (0), at a
/// process that is gone or not ours to read, and at a pid that would loop.
#[must_use]
pub fn ancestry(pid: u32) -> Vec<u32> {
    ancestry_in(Path::new("/proc"), pid)
}

/// [`ancestry`], under a `/proc` root.
#[must_use]
pub fn ancestry_in(proc_root: &Path, pid: u32) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut at = pid;
    while out.len() < MOST_ANCESTORS {
        let Some(parent) = stat_in(proc_root, at)
            .map(|s| s.parent)
            .filter(|p| *p > 1 && *p != pid && !out.contains(p))
        else {
            break;
        };
        out.push(parent);
        at = parent;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `/proc` of `(pid, name, state, parent, start)`, planted.
    fn planted(tag: &str, tree: &[(u32, &str, char, u32, u64)]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-memory-lineage-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        for (pid, name, state, parent, start) in tree {
            let at = root.join(pid.to_string());
            std::fs::create_dir_all(&at).unwrap();
            // The fields between the parent and the start, as the kernel
            // writes them: pgrp, session, tty, tpgid, flags, four fault
            // counts, four times, priority, nice, threads, itrealvalue.
            std::fs::write(
                at.join("stat"),
                format!(
                    "{pid} ({name}) {state} {parent} {pid} {pid} 34816 {pid} 4194560 \
                     120 0 0 0 3 1 0 0 20 0 1 0 {start} 9584640 812 18446744073709551615\n"
                ),
            )
            .unwrap();
        }
        root
    }

    /// FAILS IF: the start is read from any field but the 22nd, the parent
    /// from any but the 4th, or a name with spaces and parentheses in it
    /// shifts the count — a record would then be matched to a stranger.
    #[test]
    fn a_stat_line_says_its_parent_and_its_start_whatever_its_name() {
        let line = "4242 (a (weird) name) S 17 4242 4242 34816 4242 4194560 120 0 0 0 3 1 0 0 \
                    20 0 1 0 987654 9584640 812 18446744073709551615\n";
        assert_eq!(
            parse_stat(line),
            Some(Stat {
                state: 'S',
                parent: 17,
                start_ticks: 987_654,
            })
        );
        assert_eq!(parse_stat("4242 (cut) S 17 4242"), None, "a line cut short");
        assert_eq!(parse_stat("no parenthesis at all"), None);
    }

    /// FAILS IF: a pid the kernel handed to a later process reads as the
    /// process a record named, or a process that ended and waits to be
    /// reaped reads as running — an agent that is gone would stay drawn.
    #[test]
    fn a_reused_pid_is_not_the_same_process() {
        let root = planted(
            "reused",
            &[
                (500, "claude", 'S', 400, 1_000),
                (501, "claude", 'Z', 400, 2_000),
            ],
        );
        assert!(is_running_in(&root, 500, 1_000));
        assert!(
            !is_running_in(&root, 500, 999),
            "the same pid, started later: another process"
        );
        assert!(!is_running_in(&root, 501, 2_000), "a zombie has ended");
        assert!(!is_running_in(&root, 502, 1_000), "no such process");
        assert!(!is_running_in(&root, 0, 0), "0 is nobody");
        assert_eq!(start_ticks_in(&root, 500), Some(1_000));
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: the walk up does not give the parents nearest first, goes
    /// past the root every process ends at, or loops on a tree that names
    /// itself.
    #[test]
    fn the_ancestry_is_read_upwards_and_ends() {
        let root = planted(
            "ancestry",
            &[
                (10, "systemd", 'S', 1, 5),
                (20, "holder", 'S', 10, 6),
                (30, "zsh", 'S', 20, 7),
                (40, "claude", 'S', 30, 8),
                (50, "sh", 'S', 40, 9),
                (60, "loop", 'S', 61, 10),
                (61, "loop", 'S', 60, 11),
            ],
        );
        assert_eq!(ancestry_in(&root, 50), [40, 30, 20, 10]);
        assert_eq!(ancestry_in(&root, 10), Vec::<u32>::new(), "1 is no one's");
        assert_eq!(ancestry_in(&root, 99), Vec::<u32>::new(), "gone");
        assert_eq!(ancestry_in(&root, 60), [61], "a loop ends");
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: the machine's own `/proc` does not read the way the planted
    /// one does — this process would not be the process it is.
    #[test]
    fn this_process_reads_as_itself() {
        let me = std::process::id();
        let started = start_ticks(me).expect("this process's stat");
        assert!(is_running(me, started));
        assert!(!is_running(me, started + 1));
    }
}
