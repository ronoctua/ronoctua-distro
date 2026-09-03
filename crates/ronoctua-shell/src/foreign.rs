//! Foreign buffers: systemd scope and PID → cgroup correlation — §7.5.
//!
//! # Why the cgroup, and never `app_id`
//!
//! `app_id` does not distinguish two windows of the same application. Opening two
//! terminals gives two windows with the same `app_id`, and the modeline would start
//! showing one of their states on both — with no error, and with no way to
//! notice by looking.
//!
//! niri's event stream `Window` carries a `pid`. We spawn inside a
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

use anyhow::{Context as _, Result, bail};

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

#[must_use]
pub fn scope_command(
    scope: &str,
    working_directory: Option<&str>,
    program: &str,
    args: &[String],
) -> Vec<String> {
    let mut c = vec![
        "systemd-run".to_owned(),
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
        // Hardening: a foreign buffer does not need new privileges.
        "--property=NoNewPrivileges=yes".to_owned(),
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
/// keyboard, painting and niri events — **for the app's lifetime**.
///
/// It stayed hidden while there were four fixed callers; the app
/// launcher makes this the main path.
///
/// # Why it is still `status()`, and not `spawn()`
///
/// Because it is `status()` that tells "the scope was not created" from "it was
/// created and the window has not appeared yet". With `spawn` the exit code is
/// lost and the two become identical: the window simply never appears, with no
/// error at all. That is how `SPC g g` "ran" and nothing opened.
///
/// What changes is not the `status()` — it is **who waits for it**.
#[must_use]
pub fn launch(scope: &str, direct: &[String], cmd: &[String]) -> Launch {
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

    let reason = match std::process::Command::new(prog).args(rest).status() {
        Ok(st) if st.success() => {
            return Launch::WithScope {
                scope: scope.to_owned(),
                program: program.clone(),
            };
        }
        Ok(st) => format!("systemd-run exited with {st}"),
        Err(e) => format!("no systemd-run: {e}"),
    };

    // It DEGRADES, and does not give up. See `Launch::Orphan`.
    match std::process::Command::new(program).args(args).spawn() {
        Ok(_) => Launch::Orphan {
            program: program.clone(),
            reason,
        },
        // It does NOT take the session down: a program that does not exist is the user's error,
        // not the shell's.
        Err(e) => Launch::Failed {
            program: program.clone(),
            error: e.to_string(),
        },
    }
}

/// The directory of the X sockets.
const X_SOCKETS: &str = "/tmp/.X11-unix";

/// `xwayland-satellite`, by PATH and not by name.
///
/// The systemd scope comes up with a minimal `PATH`. By name, the satellite did not
/// resolve and the app opened **without** `DISPLAY` — the degradation worked, and that is
/// why the defect only showed up as "cannot open display" in the app, far from the
/// cause. It cost a VM test.
const SATELLITE: &str = match option_env!("RONOCTUA_XWAYLAND_SATELLITE") {
    Some(p) => p,
    None => "xwayland-satellite",
};

/// How many displays to try before giving up.
///
/// A hundred X11 buffers open at once is absurd; the ceiling exists so the
/// loop terminates, and not to limit anyone.
const DISPLAY_CEILING: u8 = 100;

/// The first free display number, given the occupied ones.
///
/// A **pure** function — it is what the test exercises. The alternative would be
/// finding out on a actual system that two satellites on the same display trip over
/// each other: the second fails to create the socket, its app never opens, and the
/// only symptom is a window that does not appear.
///
/// It starts at `:1` because `:0` is usually a actual X server.
#[must_use]
pub fn first_free_display(taken: &[u8]) -> Option<u8> {
    (1..DISPLAY_CEILING).find(|n| !taken.contains(n))
}

/// The display numbers already in use, read from the socket directory.
#[must_use]
pub fn taken_displays_in(dir: &Path) -> Vec<u8> {
    let Ok(inputs) = std::fs::read_dir(dir) else {
        // With no directory there is no X running at all. Zero occupied is the
        // right answer, not an error.
        return Vec::new();
    };
    inputs
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_prefix('X')?.parse::<u8>().ok()
        })
        .collect()
}

/// The display numbers in use on the system.
#[must_use]
pub fn taken_displays() -> Vec<u8> {
    taken_displays_in(Path::new(X_SOCKETS))
}

/// The command that starts an X11 buffer with ITS satellite, in the same scope.
///
/// # Why one satellite per scope
///
/// §9.1 asks for one instance per buffer. A shared satellite would make
/// every X11 window report **its** pid, and the correlation at the top of this
/// module would start pointing them all at the same buffer — with no error, with the
/// modeline showing the wrong state.
///
/// # Why both go through OUR binary
///
/// The scope has a single command. For the satellite and the app to end up in the same
/// cgroup, someone has to be the scope's process and spawn both; that
/// someone is `ronoctua-shell satellite_of`, which waits for the X socket to appear
/// before starting the app — without the wait, the app opens with `DISPLAY` pointing
/// at a socket that does not exist yet and dies saying "cannot open
/// display", intermittently.
#[must_use]
pub fn command_with_satellite(
    scope: &str,
    me: &str,
    display: u8,
    program: &str,
    args: &[String],
    working_directory: Option<&str>,
) -> Vec<String> {
    let mut internal = vec![
        me.to_owned(),
        "satellite".to_owned(),
        display.to_string(),
        program.to_owned(),
    ];
    internal.extend_from_slice(args);
    let (prog, rest) = internal.split_first().expect("never empty");
    scope_command(scope, working_directory, prog, rest)
}

/// Starts the satellite, waits for the socket and runs the program on it.
///
/// It runs INSIDE the scope: this is the process `systemd-run` puts in the cgroup,
/// and that is why both the satellite and the program inherit the right cgroup.
///
/// # Errors
///
/// The satellite does not start, the socket does not appear, or the program does not exist.
pub fn satellite_of(display: u8, program: &str, args: &[String]) -> Result<()> {
    let target = format!(":{display}");
    // The satellite is a CONVENIENCE for whoever is X11. If it does not start, the
    // Wayland app goes on working just the same — taking the buffer down here
    // would make Xwayland a dependency of everything.
    let mut satellite = match wait_for_satellite(display, &target) {
        Ok(s) => {
            tracing::info!(display = target, program, "satellite ready");
            Some(s)
        }
        Err(e) => {
            tracing::warn!(error = %e, program, "no satellite; an X11 app will not open");
            None
        }
    };

    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    if satellite.is_some() {
        cmd.env("DISPLAY", &target);
    } else {
        // Inheriting a `DISPLAY` that is not ours would point the app at another
        // buffer's X — exactly what §9.1 wants to avoid.
        cmd.env_remove("DISPLAY");
    }
    let st = cmd
        .status()
        .with_context(|| format!("could not run `{program}`"));

    // The satellite dies WITH the app: one per scope is only worth anything if it does not
    // outlive the scope. Outliving it, every closed buffer would leave an
    // Xwayland alive.
    if let Some(s) = satellite.as_mut() {
        let _ = s.kill();
        let _ = s.wait();
    }

    if !st?.success() {
        tracing::info!(program, "the buffer exited with an error");
    }
    Ok(())
}

/// Starts the satellite and waits for the X socket to appear.
fn wait_for_satellite(display: u8, target: &str) -> Result<std::process::Child> {
    let mut satellite = std::process::Command::new(SATELLITE)
        .arg(target)
        .spawn()
        .context("could not start xwayland-satellite")?;

    let socket = Path::new(X_SOCKETS).join(format!("X{display}"));
    // An ACTIVE, short wait, with a ceiling. The satellite comes up in tens of
    // milliseconds; a fixed `sleep` would be slow in the good case and too short
    // in the bad one.
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !socket.exists() {
        if std::time::Instant::now() > limit {
            let _ = satellite.kill();
            bail!(
                "xwayland-satellite did not create {} in 10 s",
                socket.display()
            );
        }
        if let Some(st) = satellite.try_wait().context("ask the satellite")? {
            bail!("xwayland-satellite exited with {st} before creating the socket");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Ok(satellite)
}

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

/// Which buffer a window belongs to, by the pid niri reported.
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

#[cfg(test)]
mod tests {
    use super::{
        APP_OOM_SCORE_ADJUST, APP_SLICE, Launch, Origin, SCOPE_PREFIX, launch, origin_at,
        scope_command, scope_from_cgroup,
    };

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
        // The process dies between niri's event and the procfs read.
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
    fn the_free_display_skips_the_taken_ones() {
        // Two satellites on the same display trip over each other: the second does not create
        // the socket, its app never opens, and the only symptom is a window
        // that does not appear.
        assert_eq!(super::first_free_display(&[]), Some(1));
        assert_eq!(super::first_free_display(&[1]), Some(2));
        assert_eq!(super::first_free_display(&[1, 2, 4]), Some(3));
        // `:0` is never chosen — it is usually a actual X.
        assert_ne!(super::first_free_display(&[]), Some(0));
    }

    #[test]
    fn the_taken_ones_come_from_the_socket_directory() {
        let dir = std::env::temp_dir().join(format!("ronoctua-x11-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("X1"), "").unwrap();
        std::fs::write(dir.join("X7"), "").unwrap();
        // A file that is not a display socket: it cannot become a number.
        std::fs::write(dir.join("outro"), "").unwrap();

        let mut v = super::taken_displays_in(&dir);
        v.sort_unstable();
        assert_eq!(v, vec![1, 7]);
        assert_eq!(super::first_free_display(&v), Some(2));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_socket_directory_is_not_an_error() {
        // With no `/tmp/.X11-unix` there is no X running. Zero occupied is the right
        // answer; failing here would prevent opening the first X11 buffer.
        let nothing = std::env::temp_dir().join("ronoctua-x11-that-does-not-exist");
        assert!(super::taken_displays_in(&nothing).is_empty());
    }

    #[test]
    fn the_satellite_and_the_app_stay_in_the_same_scope() {
        // It is section 9.1's whole point: different scopes would make the X11
        // window report the satellite's pid, and the correlation would point every
        // window at the same buffer.
        let c = super::command_with_satellite(
            "ronoctua-buffer-y",
            "/nix/store/x/bin/ronoctua-shell",
            3,
            "xterm",
            &[],
            None,
        );
        assert_eq!(c[0], "systemd-run");
        assert!(c.contains(&"--unit=ronoctua-buffer-y".to_owned()));
        // ONE scope only in the command: two `--unit`s would be two cgroups.
        assert_eq!(c.iter().filter(|a| a.starts_with("--unit=")).count(), 1);
        assert!(c.contains(&"satellite".to_owned()));
        assert!(
            c.contains(&"3".to_owned()),
            "the display was not passed: {c:?}"
        );
        // The app comes AFTER the display, otherwise `satellite_of` would treat it as a
        // display number.
        let i = c.iter().position(|x| x == "3").unwrap();
        assert_eq!(c[i + 1], "xterm");
    }

    /// THE APPLICATION IS FIRST IN LINE TO DIE, and the desktop is not.
    ///
    /// Three separate mechanisms, asserted together because each is silent on
    /// its own: a slice left to a default the manual documents WRONGLY would
    /// make the whole memory policy apply to an empty set; a missing
    /// `OOMPolicy=kill` would leave half an application alive; and a `choom`
    /// in the wrong place would either not run or eat the program's arguments.
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
        assert_eq!(c[0], "systemd-run");
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
        let v = launch(
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
        assert!(matches!(
            launch("s", &[], &["x".to_owned()]),
            Launch::Failed { .. }
        ));
        assert!(matches!(
            launch("s", &["x".to_owned()], &[]),
            Launch::Failed { .. }
        ));
    }
}
