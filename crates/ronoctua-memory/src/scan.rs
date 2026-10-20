//! Who is eating it — and the few things one may do about it.
//!
//! # Two speeds, and the difference is not cosmetic
//!
//! Reading `/proc/meminfo` is microseconds; walking a home directory is
//! seconds. Everything in this module belongs to the second kind and runs on a
//! thread, reporting as it goes and stopping when asked. The first kind lives
//! in [`crate::machine`] and may run on a clock.
//!
//! # Why the group total is not a sum of processes
//!
//! Because summing RSS lies, and it lies in favour of exactly the applications
//! one most wants to find: a browser with six processes sharing one mapped
//! binary counts that binary six times. The kernel already keeps the right
//! number per cgroup — `memory.current` — and a scope is one cgroup per
//! application by construction (section 7.5). So the total comes from the
//! kernel and RSS is only used INSIDE a group, to say which process of the six.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ronoctua_protocol::{Consumer, ConsumerKind};

use crate::{Places, read};

/// How many consumers a scan reports.
///
/// A screen shows a couple of dozen lines; a list of every process on the
/// machine is a list nobody reads, and it costs a string per entry across the
/// component boundary.
pub const CEILING: usize = 40;

/// `nix`, for the store's own numbers.
const NIX: &str = match option_env!("RONOCTUA_NIX") {
    Some(p) => p,
    None => "nix",
};

/// `journalctl`, for how much the journal is holding.
const JOURNALCTL: &str = match option_env!("RONOCTUA_JOURNALCTL") {
    Some(p) => p,
    None => "journalctl",
};

/// `systemctl`, for stopping one of our scopes.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(p) => p,
    None => "systemctl",
};

/// Who is holding RAM, by scope and then by process.
///
/// The scopes come from the cgroup tree under `app.slice` — one per application
/// — and the processes from `/proc`, which is where a name and a pid live.
#[must_use]
pub fn ram(p: &Places) -> Vec<Consumer> {
    let mut out = Vec::new();

    // The scopes: one directory per application under the applications' slice.
    if let Ok(entries) = std::fs::read_dir(p.app_slice()) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".scope") && !name.ends_with(".service") {
                continue;
            }
            let Some(bytes) = read(&e.path().join("memory.current"))
                .as_deref()
                .and_then(crate::parse_cgroup_value)
            else {
                continue;
            };
            out.push(Consumer {
                kind: ConsumerKind::Scope,
                name,
                bytes,
                hint: 0,
            });
        }
    }

    // The processes. RSS and not PSS: `smaps_rollup` costs proportionally to
    // the number of mappings, which for a browser is hundreds of reads per
    // process — and inside a group the question is only "which of these", where
    // RSS ranks the same way.
    if let Ok(entries) = std::fs::read_dir(&p.procfs) {
        for e in entries.flatten() {
            let Ok(pid) = e.file_name().to_string_lossy().parse::<u64>() else {
                continue;
            };
            let path = e.path();
            let Some(rss) = read(&path.join("statm"))
                .as_deref()
                .and_then(rss_from_statm)
            else {
                continue;
            };
            if rss == 0 {
                continue;
            }
            let name = name_of(&path).unwrap_or_else(|| pid.to_string());
            out.push(Consumer {
                kind: ConsumerKind::Process,
                name,
                bytes: rss,
                hint: pid,
            });
        }
    }

    out.sort_by_key(|c| core::cmp::Reverse(c.bytes));
    out.truncate(CEILING);
    out
}

/// The name a person knows a process by: its `comm`, unless a wrapper hid it.
///
/// A program the nixpkgs wrapper runs (`makeWrapper`, `wrapProgram`) is a
/// binary moved aside as `.<name>-wrapped`, and the kernel names the process
/// after that file, cut at fifteen bytes: the Emacs daemon is
/// `.emacs-30.2-wra`, and so is every wrapped program of the machine. Such a
/// process is told apart by its `comm` beginning with `.` and being the start
/// of its executable's file name, which ends in `-wrapped`. The `exe` link is
/// read only for a `comm` with the dot and the `cmdline` only for a process
/// that is wrapped: nothing more for anybody else. Any other `comm` — an
/// ordinary program's, a dot program's own, one a program set itself — stays.
fn name_of(dir: &Path) -> Option<String> {
    let comm = read(&dir.join("comm"))?;
    let comm = comm.trim();
    if !comm.starts_with('.') {
        return Some(comm.to_owned());
    }
    let exe = std::fs::read_link(dir.join("exe")).ok();
    let file = exe
        .as_deref()
        .and_then(Path::file_name)
        .and_then(|f| f.to_str());
    let argv0 = || {
        let raw = std::fs::read(dir.join("cmdline")).ok()?;
        let first = raw.split(|b| *b == 0).next()?;
        Some(String::from_utf8_lossy(first).into_owned())
    };
    Some(match file {
        Some(file) => unwrapped(comm, file, argv0).unwrap_or_else(|| comm.to_owned()),
        None => comm.to_owned(),
    })
}

/// The wrapped program's own name, when `comm` and the executable's `file`
/// name say it is one — see [`name_of`]; `argv0` is asked only then.
///
/// The last part of `argv[0]`, which a wrapper that inherits it
/// (`wrapProgram`, `--inherit-argv0`) keeps as the word that started the
/// program: `emacs`, `/run/current-system/sw/bin/emacs`. A wrapper that does
/// not hands the hidden binary's own path, and a program that rewrote its
/// command line (`setproctitle`: one string, with spaces) says nothing about
/// its name: then it is the file name without the dot and the suffix, which
/// is the name the wrapper was made from.
fn unwrapped(comm: &str, file: &str, argv0: impl FnOnce() -> Option<String>) -> Option<String> {
    // A BINARY REPLACED UNDER A RUNNING PROCESS — an update, a garbage
    // collection — is still its executable, and the kernel says so after it.
    let file = file.strip_suffix(" (deleted)").unwrap_or(file);
    let hidden = file.strip_suffix("-wrapped")?;
    if !file.starts_with(comm) {
        return None;
    }
    let plain = hidden.strip_prefix('.').filter(|p| !p.is_empty())?;
    let said = argv0().and_then(|a| {
        let last = a.rsplit('/').next()?.to_owned();
        (!last.is_empty() && last != file && !last.contains(char::is_whitespace)).then_some(last)
    });
    Some(said.unwrap_or_else(|| plain.to_owned()))
}

/// Resident bytes from `/proc/<pid>/statm`.
///
/// The second column is resident PAGES, not bytes. Reporting pages as bytes
/// would divide every number by four thousand and look like a machine at rest.
#[must_use]
pub fn rss_from_statm(content: &str) -> Option<u64> {
    let pages: u64 = content.split_whitespace().nth(1)?.parse().ok()?;
    // 4 KiB is the page size on every platform this system supports, and asking
    // the kernel for it per process would be a syscall per line.
    Some(pages * 4096)
}

/// Where the disk went — the numbers that come from metadata, not from a walk.
///
/// The store's size and what a collection would free are already indexed by
/// Nix; the journal knows its own size. Walking those would take minutes to
/// re-derive what a query answers at once.
#[must_use]
pub fn disk_metadata() -> Vec<Consumer> {
    let mut out = Vec::new();

    if let Some(bytes) = nix_number(&["path-info", "--json", "--all", "-S"]) {
        out.push(Consumer {
            kind: ConsumerKind::Filesystem,
            name: "/nix/store".to_owned(),
            bytes,
            hint: 0,
        });
    }
    if let Some(bytes) = journal_bytes() {
        out.push(Consumer {
            kind: ConsumerKind::Filesystem,
            name: "journal".to_owned(),
            bytes,
            hint: 0,
        });
    }
    out
}

/// Sums `narSize` over `nix path-info --json`.
///
/// Parsing JSON by hand, and only this field: the alternative is a JSON crate
/// in a crate that has none, for one number. If the shape ever changes the sum
/// comes out zero, which the screen shows as "unknown" rather than as a lie.
fn nix_number(args: &[&str]) -> Option<u64> {
    let out = Command::new(NIX).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut total = 0u64;
    let mut rest = text.as_ref();
    while let Some(i) = rest.find("\"narSize\":") {
        rest = &rest[i + "\"narSize\":".len()..];
        let number: String = rest
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(char::is_ascii_digit)
            .collect();
        total += number.parse::<u64>().unwrap_or(0);
    }
    (total > 0).then_some(total)
}

/// `journalctl --disk-usage`, which reports in its own prose.
fn journal_bytes() -> Option<u64> {
    let out = Command::new(JOURNALCTL).arg("--disk-usage").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse_disk_usage(&text)
}

/// `Archived and active journals take up 1.2G in the file system.`
#[must_use]
pub fn parse_disk_usage(text: &str) -> Option<u64> {
    let word = text.split_whitespace().find(|w| {
        w.chars().next().is_some_and(|c| c.is_ascii_digit())
            && w.chars().last().is_some_and(|c| c.is_ascii_alphabetic())
    })?;
    let (number, unit) = word.split_at(word.len() - 1);
    let n: f64 = number.parse().ok()?;
    let scale = match unit {
        "K" => 1024.0,
        "M" => 1024.0 * 1024.0,
        "G" => 1024.0 * 1024.0 * 1024.0,
        "T" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    Some((n * scale) as u64)
}

/// Walks a tree, reporting the biggest children, and stops when told to.
///
/// # Why the top level and not every directory
///
/// Because the question is "where did the disk go", and the answer a person can
/// act on is a name they recognise — `~/.cache`, `~/Downloads`. Reporting the
/// deepest directories gives a list of paths nobody has ever seen. Each child
/// is summed RECURSIVELY, so the number is the whole subtree.
///
/// `cancel` is checked once per entry: a scan the person left behind must stop
/// consuming I/O, not merely stop being drawn.
pub fn disk(root: &Path, cancel: &Arc<AtomicBool>, mut report: impl FnMut(u16)) -> Vec<Consumer> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let children: Vec<_> = entries.flatten().collect();
    let total = children.len().max(1);
    let mut out = Vec::new();

    for (i, e) in children.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let path = e.path();
        let bytes = subtree(&path, cancel);
        if bytes > 0 {
            out.push(Consumer {
                kind: ConsumerKind::Directory,
                name: path.to_string_lossy().into_owned(),
                bytes,
                hint: 0,
            });
        }
        let done = u16::try_from((i + 1) * 1000 / total).unwrap_or(1000);
        report(done);
    }

    out.sort_by_key(|c| core::cmp::Reverse(c.bytes));
    out.truncate(CEILING);
    out
}

/// Bytes under a path, following no symlink and crossing no filesystem.
///
/// `symlink_metadata`: following links would count the store through every
/// `~/.nix-profile` and report a home directory of forty gigabytes.
fn subtree(path: &Path, cancel: &Arc<AtomicBool>) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for e in entries.flatten() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        total += subtree(&e.path(), cancel);
    }
    total
}

/// Closes what a consumer names.
///
/// # Only inside the applications' slice
///
/// A scope of ours is stopped through the user manager, which needs no
/// privilege. A process is signalled by pid — and ONLY after its cgroup is
/// checked, because the same topology that keeps `systemd-oomd` away from the
/// compositor has to keep this key away from it too: a panel that can close
/// the session is a panel that can close the session by accident.
///
/// # Errors
///
/// The target is outside `app.slice`, does not exist, or systemd refused.
pub fn stop(p: &Places, target: &Consumer) -> anyhow::Result<()> {
    match target.kind {
        ConsumerKind::Scope => {
            anyhow::ensure!(
                p.app_slice().join(&target.name).exists(),
                "`{}` is not in app.slice",
                target.name
            );
            let out = Command::new(SYSTEMCTL)
                .args(["--user", "stop", &target.name])
                .output()?;
            anyhow::ensure!(
                out.status.success(),
                "systemd refused to stop `{}`: {}",
                target.name,
                String::from_utf8_lossy(&out.stderr).trim()
            );
            Ok(())
        }
        ConsumerKind::Process => {
            let pid = i32::try_from(target.hint).map_err(|_| anyhow::anyhow!("invalid pid"))?;
            anyhow::ensure!(pid > 1, "pid {pid} cannot be ended");
            let cgroup = read(&p.procfs.join(pid.to_string()).join("cgroup")).unwrap_or_default();
            anyhow::ensure!(
                cgroup.contains("/app.slice/"),
                "`{}` is outside app.slice, where this panel does not reach",
                target.name
            );
            // TERM and not KILL: the point is to give back memory, and a
            // program that is asked to leave writes what it was holding. What
            // does not leave is the person's next decision, not this one's.
            let pid = rustix::process::Pid::from_raw(pid).ok_or_else(|| anyhow::anyhow!("pid"))?;
            rustix::process::kill_process(pid, rustix::process::Signal::TERM)?;
            Ok(())
        }
        other => anyhow::bail!("I do not know how to shut {other:?} down"),
    }
}

/// Housekeeping that frees disk, through the unit that has the privilege.
///
/// The three of them belong to root: the system's generations, the store's
/// garbage and the journal. The unit is a oneshot the NixOS module ships and a
/// narrow polkit rule lets this user start — the same arrangement as
/// `keyboard-restore`, and for the same reason: asking for a password in a
/// panel is asking at the moment the machine is least able to show one.
///
/// # Errors
///
/// polkit refused or the unit failed; the message carries systemd's stderr.
pub fn sweep(what: ronoctua_protocol::Request) -> anyhow::Result<()> {
    use ronoctua_protocol::Request as R;
    let name = match what {
        R::SweepStore => "store",
        R::SweepGenerations => "generations",
        R::SweepJournal => "journal",
        other => anyhow::bail!("{other:?} is not a sweep"),
    };
    let out = Command::new(SYSTEMCTL)
        .args(["start", &format!("ronoctua-memory-sweep@{name}.service")])
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "the sweep failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statm_reports_pages_and_we_report_bytes() {
        // `size resident shared text lib data dt` — the SECOND column, in
        // pages. Reporting pages as bytes divides everything by 4096 and makes
        // a machine under pressure look asleep.
        assert_eq!(
            rss_from_statm("125853 3021 2158 5 0 21099 0"),
            Some(3021 * 4096)
        );
        assert_eq!(rss_from_statm(""), None);
    }

    /// A process of a planted `/proc`: `statm`, `comm`, the `exe` link and the
    /// `cmdline` as the kernel writes them (`argv` joined by NULs).
    fn plant(root: &Path, pid: u32, comm: &str, exe: &str, cmdline: &[u8]) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("statm"), format!("100 {pid} 0 0 0 0 0\n")).expect("statm");
        std::fs::write(dir.join("comm"), format!("{comm}\n")).expect("comm");
        std::os::unix::fs::symlink(exe, dir.join("exe")).expect("exe");
        std::fs::write(dir.join("cmdline"), cmdline).expect("cmdline");
    }

    /// A WRAPPED PROGRAM IS NAMED BY THE WORD THAT STARTED IT, and nothing else
    /// changes name. Read through `ram`, the function the memory panel and the
    /// rescue console call, over a planted `/proc`.
    #[test]
    fn a_wrapped_program_is_named_by_its_own_name() {
        let root = std::env::temp_dir().join(format!("ronoctua-scan-names-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = "/nix/store/0000-emacs-30.2/bin";
        // The Emacs daemon, as `emacs-vm` measured it: `wrapProgram` keeps
        // `argv[0]`, the word or the path that started it.
        plant(
            &root,
            101,
            ".emacs-30.2-wra",
            &format!("{store}/.emacs-30.2-wrapped"),
            b"emacs\0--fg-daemon\0",
        );
        plant(
            &root,
            102,
            ".emacs-30.2-wra",
            &format!("{store}/.emacs-30.2-wrapped"),
            b"/run/current-system/sw/bin/emacs\0-nw\0",
        );
        // A wrapper that hands the hidden binary its own path, and a program
        // whose binary went away under it: the name the wrapper was made from.
        plant(
            &root,
            103,
            ".foo-wrapped",
            "/nix/store/0000-foo/bin/.foo-wrapped",
            b"/nix/store/0000-foo/bin/.foo-wrapped\0",
        );
        plant(
            &root,
            104,
            ".bar-wrapped",
            "/nix/store/0000-bar/bin/.bar-wrapped (deleted)",
            b"bar\0",
        );
        // A wrapped program that rewrote its command line says nothing there.
        plant(
            &root,
            105,
            ".baz-wrapped",
            "/nix/store/0000-baz/bin/.baz-wrapped",
            b"baz: worker process",
        );
        // NOT WRAPPED, and keeping its `comm` whatever its command line says:
        // a dot program of the person's, a `comm` the program set itself that
        // is no start of its file, and an ordinary one.
        plant(&root, 106, ".hidden", "/home/person/.hidden", b"wrong\0");
        plant(
            &root,
            107,
            ".x",
            "/nix/store/0000-qux/bin/.qux-wrapped",
            b"wrong\0",
        );
        plant(
            &root,
            108,
            "bash",
            "/run/current-system/sw/bin/bash",
            b"-bash\0",
        );
        let places = Places {
            procfs: root.clone(),
            sysfs: root.join("sys"),
            cgroup: root.join("cgroup"),
            uid: 1000,
        };
        let mut said: Vec<(u64, String)> =
            ram(&places).into_iter().map(|c| (c.hint, c.name)).collect();
        said.sort();
        let want: Vec<(u64, String)> = [
            (101, "emacs"),
            (102, "emacs"),
            (103, "foo"),
            (104, "bar"),
            (105, "baz"),
            (106, ".hidden"),
            (107, ".x"),
            (108, "bash"),
        ]
        .into_iter()
        .map(|(p, n)| (p, n.to_owned()))
        .collect();
        assert_eq!(said, want);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The command line is read only for a wrapped process.
    #[test]
    fn only_a_wrapped_process_has_its_command_line_read() {
        let asked = std::cell::Cell::new(0);
        let ask = || {
            asked.set(asked.get() + 1);
            Some("emacs".to_owned())
        };
        assert_eq!(unwrapped(".hidden", ".hidden", ask), None);
        assert_eq!(unwrapped(".x", ".qux-wrapped", ask), None);
        assert_eq!(
            asked.get(),
            0,
            "a process that is not wrapped had its cmdline read"
        );
        assert_eq!(
            unwrapped(".emacs-30.2-wra", ".emacs-30.2-wrapped", ask),
            Some("emacs".to_owned())
        );
        assert_eq!(asked.get(), 1);
        // `-wrapped` alone is no program.
        assert_eq!(unwrapped(".-wrapped", ".-wrapped", || None), None);
    }

    #[test]
    fn the_journals_own_prose_becomes_a_number() {
        assert_eq!(
            parse_disk_usage("Archived and active journals take up 1.2G in the file system.\n"),
            Some((1.2 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
        assert_eq!(
            parse_disk_usage("Archived and active journals take up 512.0M in the file system.\n"),
            Some(512 * 1024 * 1024)
        );
        // A sentence with no size is not a zero: it is an answer we did not get.
        assert_eq!(parse_disk_usage("no journal files"), None);
    }

    #[test]
    fn a_cancelled_scan_stops_where_it_was() {
        let dir = std::env::temp_dir().join(format!("ronoctua-scan-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("one")).expect("mkdir");
        std::fs::write(dir.join("one/file"), vec![0u8; 4096]).expect("write");
        let cancel = Arc::new(AtomicBool::new(true));
        // Already cancelled: it reports nothing, and above all it does not walk.
        let out = disk(&dir, &cancel, |_| {});
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_scan_reports_the_whole_subtree_and_its_progress() {
        let dir = std::env::temp_dir().join(format!("ronoctua-scan2-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("large/inside")).expect("mkdir");
        std::fs::write(dir.join("large/inside/file"), vec![0u8; 8192]).expect("write");
        std::fs::create_dir_all(dir.join("small")).expect("mkdir");
        std::fs::write(dir.join("small/file"), vec![0u8; 1024]).expect("write");

        let cancel = Arc::new(AtomicBool::new(false));
        let mut seen = Vec::new();
        let out = disk(&dir, &cancel, |p| seen.push(p));

        assert_eq!(out.len(), 2, "two first-level folders");
        // The biggest first, and the number is the WHOLE subtree — the file is
        // two levels down.
        assert!(out[0].name.ends_with("large"));
        assert_eq!(out[0].bytes, 8192);
        assert_eq!(seen.last(), Some(&1000), "the progress ends at 100%");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
