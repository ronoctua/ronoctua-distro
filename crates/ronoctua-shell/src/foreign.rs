//! Foreign buffers: systemd scope and PID → cgroup correlation — §7.5.
//!
//! # Why the cgroup, and never `app_id`
//!
//! `app_id` does not distinguish two windows of the same application. Opening two
//! terminals gives two windows with the same `app_id`, and the modeline would start
//! showing one of their states on both — with no error, and with no way to
//! notice by looking.
//!
//! The compositor's event stream carries a window's `pid`. We spawn inside a
//! scope of known name; on seeing the window, we read `/proc/<pid>/cgroup` and
//! match by name. **It survives a fork** — a launcher that execs another
//! process stays in the same cgroup — and it requires no cooperation from the application.
//!
//! # A window we did not spawn
//!
//! Its cgroup goes up: if it lands in a known scope, it attaches; otherwise it becomes an
//! **orphan** buffer, registered on the spot. An orphan is not an error — it is the case of whoever opened
//! something from outside, and ignoring it would make the window exist on screen and not in the
//! modeline.

use std::path::Path;

use anyhow::{Context as _, Result};

/// The prefix of a foreign buffer's scope.
///
/// The name carries the uuid because `app_id` is no use as an identity (see the top
/// of the module) and the pid changes on every `exec`.
pub const SCOPE_PREFIX: &str = "ronoctua-buffer-";

/// How the shell knows a foreign buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Spawned by us, in the scope of the given name.
    Ours(String),
    /// Opened from outside. Registered on the spot, with no known scope.
    Orphan,
}

/// The scope name for a new buffer.
///
/// # Errors
///
/// The system has no usable source of randomness.
pub fn scope_name() -> Result<String> {
    // `/proc/sys/kernel/random/uuid` and not a uuid crate: it is one
    // dependency fewer to generate a name systemd only needs to be
    // unique, and the kernel already offers it.
    let uuid = std::fs::read_to_string("/proc/sys/kernel/random/uuid")
        .context("could not read a uuid from the kernel")?;
    Ok(format!("{SCOPE_PREFIX}{}", uuid.trim()))
}

/// The command that starts a foreign buffer in its own scope.
///
/// Returned as **data**, and not run here, for the same reason as the installer's
/// plan: that way the test checks the shape without spawning anything.
///
/// `--user` because the session is the user's; `--scope` because we want the
/// process in our cgroup and not a service with a life cycle of its own.
///
/// # `Path=` OF THE DESKTOP ENTRY
///
/// `working_directory` carries the `.desktop`'s `Path=`. It used to be read and
/// dropped, and what suffers is the application that expects to start inside its
/// own folder: a Wine wrapper, a game with data next to the binary, a script
/// with a relative path. The symptom is the worst kind — the app **opens and
/// dies**, with nothing on screen, and its error goes to the scope's log.
///
/// `systemd-run` has the option; it only had to be given.
/// Where a foreign buffer's scope goes, said out loud.
///
/// The memory policy (`nix/modules/memory.nix`) monitors this slice and nothing
/// else inside the session, which is what keeps the compositor and the footer
/// out of the candidate set — they live in `session.slice`, a SIBLING.
pub const APP_SLICE: &str = "app.slice";

/// How much worse than the desktop a foreign application is, for the kernel.
///
/// The desktop sits at the 100 it inherits from `user@.service`; this is the
/// distance from it. Any positive distance would do — 500 leaves room for a
/// future "this one first" without renumbering.
pub const APP_OOM_SCORE_ADJUST: i32 = 500;

/// `choom`, by STORE PATH and not by name: inside a systemd scope the `PATH` is
/// minimal, and the failure would look like an application that does not open.
const CHOOM: &str = match option_env!("RONOCTUA_CHOOM") {
    Some(p) => p,
    None => "choom",
};

/// `systemd-run`, by store path, for `choom`'s reason: the footer is a unit,
/// and a unit's `PATH` is whatever its manager was given. Outside Nix —
/// `cargo run` — the name, and the shell's `PATH` resolves it.
const SYSTEMD_RUN: &str = match option_env!("RONOCTUA_SYSTEMD_RUN") {
    Some(p) => p,
    None => "systemd-run",
};

/// `true`, the program the probe runs in a scope — see [`scopes_work`].
const TRUE: &str = match option_env!("RONOCTUA_TRUE") {
    Some(p) => p,
    None => "true",
};

#[must_use]
pub fn scope_command(
    scope: &str,
    working_directory: Option<&str>,
    program: &str,
    args: &[String],
) -> Vec<String> {
    let mut c = vec![
        SYSTEMD_RUN.to_owned(),
        "--user".to_owned(),
        "--scope".to_owned(),
        "--quiet".to_owned(),
        format!("--unit={scope}"),
        // THE SLICE IS NAMED, and not left to the default.
        //
        // `systemd-run(1)` still documents the `--user` default as "the root
        // slice"; the user manager has actually put units in `app.slice` since
        // systemd v247 (`systemd.special(7)`). The memory policy in
        // `nix/modules/memory.nix` monitors `app.slice` and nothing else in the
        // session — leaving this to a default the manual contradicts is how the
        // whole policy would end up applying to an empty set.
        format!("--slice={APP_SLICE}"),
        // NO `NoNewPrivileges` HERE: it is an EXEC property, and a scope has
        // no exec context of its own — `systemd-run --scope` refuses the whole
        // command with "Unknown assignment", before it reaches the manager.
        // With it, the probe answered "no scopes" in every session and every
        // program opened outside `app.slice` (measured on systemd 260.2, and
        // proved by `software-vm`).
        // THE SCOPE DIES WHOLE. Two effects, both wanted: the kernel does not
        // leave half an application running, and a scope that spawns
        // sub-cgroups stays an eligible candidate — `systemd-oomd(8)` considers
        // "only leaf cgroups and cgroups with memory.oom.group set to 1".
        //
        // `OOMPolicy=` is a real `[Scope]` key. `OOMScoreAdjust=` is NOT: a
        // scope has no execution context, so systemd refuses the property.
        // Hence `choom` below.
        "--property=OOMPolicy=kill".to_owned(),
    ];
    if let Some(dir) = working_directory {
        c.push(format!("--working-directory={dir}"));
    }
    // FIRST IN LINE TO DIE, for the kernel's own OOM killer.
    //
    // `user@.service` sets `OOMScoreAdjust=100` for everything in the session,
    // and a user unit cannot go BELOW its inherited value — the kernel returns
    // `EACCES` and systemd swallows the failure at debug level. So the desktop
    // stays at 100 and the applications are RAISED, which needs no privilege.
    //
    // The `--` matters: without it `choom` would read the application's own
    // arguments as its own.
    c.push(CHOOM.to_owned());
    c.push("-n".to_owned());
    c.push(APP_OOM_SCORE_ADJUST.to_string());
    c.push("--".to_owned());
    c.push(program.to_owned());
    c.extend_from_slice(args);
    c
}

/// Starts `program args` in a scope of its own, on a NAMED thread, and hands
/// the verdict to `done` on that thread.
///
/// THE ONE PATH every door launches by — the footer's buffers and
/// applications, the file manager's window, the software window. Each used to
/// carry its own copy of these lines, and a copy is where a thread comes to
/// be forgotten: `systemd-run --scope` is synchronous ([`launch`]), and one
/// run on a loop froze it for the life of what it opened.
///
/// ONE COMMAND, AND NO DISPLAY TO PICK. The compositor serves X11 itself, so
/// an X11 program is launched exactly like a Wayland one — and the PID the
/// compositor reports for its window is the program's own, which is what
/// §7.5's correlation needs. The DIRECT command goes along: it is the one
/// that runs when the scope cannot be created, and rebuilding it from the
/// `systemd-run` would mean taking that apart.
///
/// The thread's NAME shows up in `perf` and in `gdb`: an anonymous thread
/// stuck in `wait4` for hours is exactly what nobody identifies.
///
/// # Errors
///
/// No scope name could be made, or the thread could not be started.
pub fn start_in_scope(
    thread: &str,
    program: &str,
    args: &[String],
    working_directory: Option<&str>,
    done: impl FnOnce(Launch) + Send + 'static,
) -> Result<()> {
    let scope = scope_name()?;
    let cmd = scope_command(&scope, working_directory, program, args);
    let mut direct = vec![program.to_owned()];
    direct.extend_from_slice(args);
    std::thread::Builder::new()
        .name(thread.to_owned())
        .spawn(move || done(launch(&scope, &direct, &cmd)))
        .context("could not create the launch thread")?;
    Ok(())
}

/// The same launch, NOW, on the thread that calls it — BLOCKING for the life
/// of what it opened ([`launch`]). For a thread that already had its own
/// work to do first: the window's launch by desktop id reads the catalogue,
/// and then starts what it found here.
#[must_use]
pub fn launch_now(program: &str, args: &[String], working_directory: Option<&str>) -> Launch {
    let scope = match scope_name() {
        Ok(s) => s,
        Err(e) => {
            return Launch::Failed {
                program: program.to_owned(),
                error: e.to_string(),
            };
        }
    };
    let cmd = scope_command(&scope, working_directory, program, args);
    let mut direct = vec![program.to_owned()];
    direct.extend_from_slice(args);
    launch(&scope, &direct, &cmd)
}

/// How a launch ended.
///
/// It comes back from the thread to the event loop through the channel, and not as a return
/// value — see [`launch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// It came up in its own cgroup. It is the good path.
    WithScope {
        /// The scope's name, for the PID → cgroup correlation.
        scope: String,
        /// The program, for the log.
        program: String,
    },
    /// `systemd-run` did not create the scope; it came up orphaned.
    ///
    /// A first-class state, and not an error: with no `systemd --user` — a session
    /// with no manager, a container, a test — the scope does not exist and the correlation
    /// does not find the buffer. A window that does not open is worse than one whose name
    /// in the modeline comes from the title.
    Orphan {
        /// The program, for the log.
        program: String,
        /// Why the scope did not come out.
        reason: String,
    },
    /// Not even the direct command came up. The user needs to know.
    Failed {
        /// The program, for the message in the echo area.
        program: String,
        /// The operating system's error.
        error: String,
    },
}

/// Runs the launch — **and BLOCKS**. Call it from a thread.
///
/// # Why this cannot run on the event loop
///
/// Because `systemd-run --scope` is **synchronous**. From `systemd-run(1)`:
///
/// > Execution in this case is synchronous, and will return only when the
/// > command finishes. This mode is enabled via the `--scope` switch.
///
/// The comment that was here said the opposite — *"`systemd-run` is
/// ephemeral, it creates the scope and exits in milliseconds"* — and that is true in
/// SERVICE mode, which is the default, and false with `--scope`, which is what we use. Since
/// `calloop` is a single thread, opening an app froze the whole footer —
/// keyboard, painting and compositor events — **for the app's lifetime**.
///
/// It stayed hidden while there were four fixed callers; the app
/// launcher makes this the main path.
///
/// # The exit status is the APPLICATION'S, and nothing is launched twice
///
/// `systemd-run --scope` creates the scope and then BECOMES the program — it
/// `exec`s it —, so once the scope exists, the status that comes back is the
/// program's own. This used to read any failure as "the scope was not
/// created" and spawn the program AGAIN, directly: every application that
/// exited with a code — a closed game, an editor asked to open a file it could
/// not, a program that ended with `exit 3` — opened a second time, orphaned,
/// right after the person closed it.
///
/// Whether scopes work at all is asked ONCE per process, before the first
/// launch, by a scope around `true` ([`scopes_work`]). With them, whatever the
/// status is, it is logged and that is all ([`next`]); without them — no user
/// manager, a container, a session started by hand — the program is spawned
/// directly from the start, and the window is an orphan.
#[must_use]
pub fn launch(scope: &str, direct: &[String], cmd: &[String]) -> Launch {
    // The shapes are checked BEFORE the probe: a command that could never run
    // does not need to know whether scopes work.
    if cmd.is_empty() || direct.is_empty() {
        return launch_with(false, scope, direct, cmd);
    }
    launch_with(scopes_work(), scope, direct, cmd)
}

/// [`launch`], with the answer about scopes handed in — so a test can take
/// the direct path without a probe ever reaching the machine.
fn launch_with(scopes: bool, scope: &str, direct: &[String], cmd: &[String]) -> Launch {
    let Some((prog, rest)) = cmd.split_first() else {
        return Launch::Failed {
            program: String::new(),
            error: "an empty command".to_owned(),
        };
    };
    let Some((program, args)) = direct.split_first() else {
        return Launch::Failed {
            program: String::new(),
            error: "an empty direct command".to_owned(),
        };
    };

    // It is still `status()`, and not `spawn()`: waiting is what keeps the
    // scope's name meaning "this application is running", and the wait is on
    // this thread, never on the loop's.
    let ran = scopes.then(|| {
        crate::watchdog::unshared(&mut std::process::Command::new(prog))
            .args(rest)
            .status()
    });
    let reason = match next(scopes, ran.as_ref()) {
        Next::Ended => {
            if let Some(Ok(st)) = &ran
                && !st.success()
            {
                // THE APPLICATION'S OWN ENDING, and it stays in the log: it is
                // how a program that "just closes" gets diagnosed, and a
                // relaunch is exactly what must not follow it.
                tracing::info!(program, scope, status = %st, "the application ended with a failure status");
            }
            return Launch::WithScope {
                scope: scope.to_owned(),
                program: program.clone(),
            };
        }
        Next::Direct(reason) => reason,
    };

    // It DEGRADES, and does not give up. See `Launch::Orphan`.
    match crate::watchdog::unshared(&mut std::process::Command::new(program))
        .args(args)
        .spawn()
    {
        // WAITED FOR, aside: with no scope nothing else ever reaps it.
        Ok(child) => {
            let _ = waited(child);
            Launch::Orphan {
                program: program.clone(),
                reason,
            }
        }
        // It does NOT take the session down: a program that does not exist is the user's error,
        // not the shell's.
        Err(e) => Launch::Failed {
            program: program.clone(),
            error: e.to_string(),
        },
    }
}

/// What a launch does after `systemd-run` — or instead of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// The application ran in its scope and has ENDED; whatever its status,
    /// that status was its own. Nothing more is launched.
    Ended,
    /// Spawn the program directly, for the reason given — the scope was never
    /// there, so the program never ran.
    Direct(String),
}

/// The decision after a launch in a scope, as a pure function.
///
/// - Without scopes, the scope command never ran: spawn directly.
/// - With scopes, a `systemd-run` that could not even be STARTED (`Err`) never
///   ran the program either: spawn directly.
/// - With scopes, ANY exit status — success or not — belongs to the program,
///   which has run and ended: nothing else.
///
/// `ran` is `None` when nothing was run, which only a caller without scopes
/// has any reason to hand in.
#[must_use]
pub fn next(scopes: bool, ran: Option<&std::io::Result<std::process::ExitStatus>>) -> Next {
    match (scopes, ran) {
        (true, Some(Ok(_))) => Next::Ended,
        (true, Some(Err(e))) => Next::Direct(format!("no systemd-run: {e}")),
        (true, None) => Next::Direct("systemd-run was not run".to_owned()),
        (false, _) => Next::Direct("this session cannot give a program a scope".to_owned()),
    }
}

/// Whether `systemd-run --user --scope` works in this session — asked once per
/// process, and remembered.
///
/// # Why ask at all, and why this way
///
/// Because after a launch the exit status cannot say it: it is the program's
/// (see [`launch`]). The one moment the answer is clean is a scope around a
/// program whose status is known in advance — `true` —, with the SAME
/// properties, slice and `choom` a real launch uses: a property this systemd
/// refuses, or a `choom` that is not there, fails the probe exactly as it
/// would fail every launch, and the launches then go direct instead of failing
/// in silence.
///
/// It BLOCKS for the life of `true` — tens of milliseconds, once — so it is
/// called where [`launch`] is: on the launching thread, never the loop's. The
/// first launch pays it, and a footer that never opens anything never does.
#[must_use]
pub fn scopes_work() -> bool {
    static SCOPES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SCOPES.get_or_init(|| {
        let argv = probe_command(std::process::id());
        let Some((program, args)) = argv.split_first() else {
            return false;
        };
        let answer = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|st| st.success());
        tracing::info!(
            scopes = answer,
            "asked whether this session can give a program a scope"
        );
        answer
    })
}

/// The probe's command: [`scope_command`] around `true`, in a scope of its own
/// name.
///
/// Not a `ronoctua-buffer-` name: a window is matched to a buffer by that
/// prefix, and `true` has no window — the probe is not a buffer. The process
/// id keeps two footers' probes apart.
#[must_use]
pub fn probe_command(pid: u32) -> Vec<String> {
    scope_command(&format!("ronoctua-probe-{pid}"), None, TRUE, &[])
}

// THERE IS NO SATELLITE ANY MORE, and its absence is the point.
//
// X11 used to need one process per buffer scope: the compositor had no
// XWayland of its own, so `xwayland-satellite` was started beside every X11
// app and §9.1's PID → cgroup correlation depended on the two landing in the
// same scope.
//
// The compositor has XWayland built in, and — this is the part that made the
// removal safe rather than merely simpler — it reports the X CLIENT'S pid for
// an X11 window, not XWayland's, because wlroots reads it out of the XRes
// extension. So `Window::pid` is still the real program's, one server serves
// the session, and the correlation is unchanged with two hundred lines and a
// package fewer.

/// Extracts the scope's name from the contents of a `/proc/<pid>/cgroup`.
///
/// A **pure** function: it is what the tests exercise, because the alternative
/// would be finding out on a actual system that the format has two columns
/// before the path, or that the scope appears nested under the user's slice.
#[must_use]
pub fn scope_from_cgroup(content: &str) -> Option<String> {
    // cgroup v2 has ONE line, in the form `0::<path>`. v1 has several,
    // with the controller in front. Taking the last component of the path that
    // ends in `.scope` covers both without telling the versions apart.
    content
        .lines()
        .filter_map(|l| l.rsplit(':').next())
        .flat_map(|path| path.split('/'))
        .find(|c| c.starts_with(SCOPE_PREFIX) && c.ends_with(".scope"))
        .map(|c| c.trim_end_matches(".scope").to_owned())
}

/// Which buffer a window belongs to, by the pid the compositor reported.
///
/// It reads `/proc/<pid>/cgroup`. A pid that no longer exists, or that is not in a
/// scope of ours, becomes [`Origin::Orphan`] — which is a record, not an error.
#[must_use]
pub fn origin_of_pid(pid: i32) -> Origin {
    origin_at(Path::new("/proc"), pid)
}

/// [`origin_of_pid`], with the `procfs` root made explicit.
///
/// The root is a parameter only so the test can build a fake `/proc`.
/// Without that, proving the correlation would require spawning a actual process — and the
/// test would come to depend on the systemd of whatever machine runs it.
#[must_use]
pub fn origin_at(procfs: &Path, pid: i32) -> Origin {
    let path = procfs.join(pid.to_string()).join("cgroup");
    match std::fs::read_to_string(&path) {
        Ok(c) => match scope_from_cgroup(&c) {
            Some(s) => Origin::Ours(s),
            None => Origin::Orphan,
        },
        // The process died between the event and the read. It happens, and it is no
        // reason to complain in the log on every window that closes.
        Err(_) => Origin::Orphan,
    }
}

/// A child WAITED FOR on a thread of its own ([`waited`]), and what the one
/// who started it can still ask: its pid, and whether it has ended.
#[derive(Debug)]
pub struct Waited {
    /// The child's pid.
    pub pid: u32,
    ended: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Waited {
    /// Whether the child has ended — and been reaped, by its thread.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.ended.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// `child` waited for on a thread of its own until it ends — never on the
/// loop —, with its pid and an "ended" that is asked without waiting.
///
/// # Why
///
/// A `Child` dropped is NOT reaped: what it started ends as a ZOMBIE of this
/// process, and the footer lives for the whole session. The final battery's
/// `modeline-pair` found two: the player's column, kept for `try_wait` and
/// asked only when a second raise came, and the player's window started
/// without a scope ([`Launch::Orphan`]), whose `Child` was dropped at once —
/// each still answered `kill -0` after `player-quit` had closed it.
///
/// A thread that cannot be started says the child ENDED: nothing would ever
/// say so otherwise, and a door asking "is it still coming?" would refuse
/// every raise after it.
pub fn waited(mut child: std::process::Child) -> Waited {
    let pid = child.id();
    let ended = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let said = std::sync::Arc::clone(&ended);
    let born = std::thread::Builder::new()
        .name("child-wait".to_owned())
        .spawn(move || {
            let _ = child.wait();
            said.store(true, std::sync::atomic::Ordering::Release);
        });
    if let Err(e) = born {
        tracing::warn!(error = %e, pid, "could not start the thread that waits for a child");
        ended.store(true, std::sync::atomic::Ordering::Release);
    }
    Waited { pid, ended }
}

#[cfg(test)]
mod tests {
    use super::{
        APP_OOM_SCORE_ADJUST, APP_SLICE, Launch, Next, Origin, SCOPE_PREFIX, launch_with, next,
        origin_at, probe_command, scope_command, scope_from_cgroup, waited,
    };
    use std::os::unix::process::ExitStatusExt as _;

    #[test]
    fn reads_the_scope_from_cgroup_v2() {
        // The actual format: one line, `0::` and the path, with the scope
        // nested under the user's slice.
        let c = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/\
                 ronoctua-buffer-9f3a.scope\n";
        assert_eq!(
            scope_from_cgroup(c),
            Some("ronoctua-buffer-9f3a".to_owned())
        );
    }

    #[test]
    fn reads_the_scope_from_cgroup_v1() {
        // v1 has several lines and the controller before the path. A parser
        // that assumed a single line would return `None` here — silently, and
        // every window would become an orphan.
        let c = "12:pids:/user.slice/ronoctua-buffer-aa.scope\n\
                 11:memory:/user.slice/ronoctua-buffer-aa.scope\n\
                 0::/user.slice/ronoctua-buffer-aa.scope\n";
        assert_eq!(scope_from_cgroup(c), Some("ronoctua-buffer-aa".to_owned()));
    }

    #[test]
    fn a_process_outside_our_scope_is_an_orphan() {
        // Whoever opened something from outside. An orphan is a record, not an error: ignoring it
        // would make the window exist on screen and not in the modeline.
        assert_eq!(
            scope_from_cgroup("0::/user.slice/app-firefox.scope\n"),
            None
        );
        assert_eq!(scope_from_cgroup("0::/\n"), None);
        assert_eq!(scope_from_cgroup(""), None);
    }

    #[test]
    fn a_scope_of_another_prefix_does_not_count() {
        // Somebody else's scope that happens to end in `.scope` is not ours.
        assert_eq!(
            scope_from_cgroup("0::/user.slice/something-else.scope\n"),
            None
        );
    }

    #[test]
    fn a_pid_that_does_not_exist_becomes_an_orphan() {
        // The process dies between the compositor's event and the procfs read.
        // It happens every time a window closes quickly.
        let tmp = std::env::temp_dir().join("ronoctua-test-empty-procfs");
        assert_eq!(origin_at(&tmp, 999_999), Origin::Orphan);
    }

    #[test]
    fn correlates_through_procfs() {
        let root = std::env::temp_dir().join(format!("ronoctua-proc-{}", std::process::id()));
        let dir = root.join("4242");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("cgroup"),
            "0::/user.slice/ronoctua-buffer-teste.scope\n",
        )
        .unwrap();

        assert_eq!(
            origin_at(&root, 4242),
            Origin::Ours("ronoctua-buffer-teste".to_owned())
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_application_dies_before_the_desktop_does() {
        let c = scope_command("ronoctua-buffer-x", None, "alacritty", &["-e".into()]);

        assert!(
            c.contains(&format!("--slice={APP_SLICE}")),
            "the slice has to be SAID: --user's default is documented wrongly"
        );
        assert!(
            c.contains(&"--property=OOMPolicy=kill".to_owned()),
            "without this the scope dies half-way"
        );

        let choom = c
            .iter()
            .position(|a| a.ends_with("choom"))
            .expect("without choom the app sits at the same level as the compositor");
        assert_eq!(c[choom + 1], "-n");
        assert_eq!(c[choom + 2], APP_OOM_SCORE_ADJUST.to_string());
        // The `--` is what stops `choom` from reading `-e` as its own.
        assert_eq!(c[choom + 3], "--");
        assert_eq!(c[choom + 4], "alacritty");

        // And it is POSITIVE: the desktop inherits 100 from `user@.service` and
        // cannot go below it, so the ordering only exists if this one is above.
        // A `const` block, so a future edit that lowers it fails to COMPILE
        // rather than failing a test somebody might be tempted to adjust.
        const { assert!(APP_OOM_SCORE_ADJUST > 100) };
    }

    #[test]
    fn the_command_puts_the_process_in_a_named_scope() {
        let c = scope_command("ronoctua-buffer-x", None, "alacritty", &["-e".into()]);
        // By PATH under Nix, by name outside it — either way, systemd-run.
        assert!(c[0].ends_with("systemd-run"), "{}", c[0]);
        assert!(
            c.contains(&"--scope".to_owned()),
            "with no --scope there is no cgroup"
        );
        assert!(
            c.contains(&"--user".to_owned()),
            "the session belongs to the user"
        );
        assert!(c.contains(&"--unit=ronoctua-buffer-x".to_owned()));
        // The program comes AFTER the options; inverted, systemd-run would treat
        // our options as the program's arguments.
        let i = c.iter().position(|x| x == "alacritty").unwrap();
        assert!(i > 0 && c[i + 1] == "-e");
        assert!(
            c.iter()
                .all(|a| a.starts_with(SCOPE_PREFIX) || !a.contains("uuid"))
        );
    }

    /// THE DEGRADED PATH, without depending on `systemd-run` being there.
    ///
    /// `launch` decides between three outcomes, and the two that matter to the
    /// user are the bottom ones: "it came up orphaned" and "it did not come up". A program that
    /// does not exist is the only input that produces both reproducibly
    /// on any machine — including in the Nix sandbox, which has no
    /// `systemd --user` and does not guarantee `/bin/sh`.
    #[test]
    fn when_nothing_comes_up_the_verdict_says_which_program() {
        // WITHOUT scopes, so nothing but the missing program is ever asked
        // for: a test that ran the probe would start `systemd-run` on the
        // machine that runs the tests.
        let v = launch_with(
            false,
            "scope-x",
            &["ronoctua-program-that-does-not-exist".to_owned()],
            &["ronoctua-systemd-run-that-does-not-exist".to_owned()],
        );
        match v {
            Launch::Failed { program, error } => {
                assert_eq!(program, "ronoctua-program-that-does-not-exist");
                assert!(!error.is_empty(), "the system's error has to come along");
            }
            otherwise => panic!("expected Failed, got {otherwise:?}"),
        }
    }

    /// An empty command must not panic.
    ///
    /// The `split_first().expect(...)` that was here ran on the EVENT
    /// LOOP; now it runs on a thread, where a panic would take the thread down
    /// silently and the launch would disappear with no verdict at all.
    #[test]
    fn an_empty_command_gives_back_a_verdict_instead_of_panicking() {
        for scopes in [false, true] {
            assert!(matches!(
                launch_with(scopes, "s", &[], &["x".to_owned()]),
                Launch::Failed { .. }
            ));
            assert!(matches!(
                launch_with(scopes, "s", &["x".to_owned()], &[]),
                Launch::Failed { .. }
            ));
        }
    }

    /// An exit status, as `wait` would hand it back for `exit(code)`.
    fn exited(code: i32) -> std::io::Result<std::process::ExitStatus> {
        Ok(std::process::ExitStatus::from_raw(code << 8))
    }

    /// THE DEFECT: with a scope, the status is the APPLICATION'S — and a
    /// program that ended with `exit 3` was read as "the scope was not
    /// created" and opened a second time, directly, right after it closed.
    #[test]
    fn with_scopes_any_status_is_the_applications_and_nothing_relaunches() {
        for code in [0, 1, 3, 127] {
            assert_eq!(next(true, Some(&exited(code))), Next::Ended, "exit {code}");
        }
        // Killed by a signal is an ending too.
        let killed: std::io::Result<std::process::ExitStatus> =
            Ok(std::process::ExitStatus::from_raw(9));
        assert_eq!(next(true, Some(&killed)), Next::Ended);
    }

    /// The two ways the program never ran — no scopes, or a `systemd-run` that
    /// could not be started — are the only ones that spawn it directly.
    #[test]
    fn without_a_scope_the_program_is_spawned_directly() {
        assert!(matches!(next(false, None), Next::Direct(_)));
        assert!(matches!(next(false, Some(&exited(0))), Next::Direct(_)));
        let missing: std::io::Result<std::process::ExitStatus> =
            Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        let Next::Direct(why) = next(true, Some(&missing)) else {
            panic!("a systemd-run that never started ran nothing, and nothing ran");
        };
        assert!(why.contains("systemd-run"), "{why}");
    }

    /// The probe is a real launch's SHAPE around `true`: the same properties,
    /// slice and `choom`, so what would fail every launch fails the probe.
    #[test]
    fn the_probe_is_a_launch_around_true() {
        let probe = probe_command(42);
        let launch = scope_command("ronoctua-probe-42", None, super::TRUE, &[]);
        assert_eq!(probe, launch);
        assert!(
            probe.last().is_some_and(|p| p.ends_with("true")),
            "{probe:?}"
        );
        assert!(probe.contains(&"--unit=ronoctua-probe-42".to_owned()));
        // NOT a buffer's scope: a window is matched to a buffer by that prefix.
        assert!(probe.iter().all(|a| !a.contains(SCOPE_PREFIX)), "{probe:?}");
    }

    /// FAILS IF: a child that ends stays a ZOMBIE of whoever started it (its
    /// `/proc` entry says `Z`), `ended` says so while it still runs, or it
    /// never says so once the child is gone — the footer would then refuse
    /// every raise of the column as "still coming".
    #[test]
    fn a_child_that_ends_is_reaped_and_says_so() {
        let state = |pid: u32| {
            std::fs::read_to_string(format!("/proc/{pid}/status"))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find_map(|l| l.strip_prefix("State:").map(|v| v.trim().to_owned()))
                })
        };
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "read -r _"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("sh");
        let stdin = child.stdin.take().expect("stdin");
        let door = waited(child);
        assert!(!door.ended(), "a child still reading said it had ended");
        drop(stdin);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !door.ended() {
            assert!(
                std::time::Instant::now() < until,
                "the child ended and nothing said so"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let left = state(door.pid);
        assert!(
            !left.as_deref().is_some_and(|s| s.starts_with('Z')),
            "the ended child is a zombie: {left:?}"
        );
    }
}
