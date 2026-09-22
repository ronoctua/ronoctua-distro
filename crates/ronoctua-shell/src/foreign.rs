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
/// keyboard, painting and compositor events — **for the app's lifetime**.
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
