//! Starting a holder, in a scope of its own.
//!
//! # The chain
//!
//! ```text
//! setsid --fork systemd-run --user --scope … choom -n 500 -- ronoctua-term-holder serve …
//! ```
//!
//! - `systemd-run --scope` because a scope keeps the CALLER's environment —
//!   the session's `PATH`, `WAYLAND_DISPLAY`, the person's variables — which
//!   a transient service (the user manager's environment) would not. Its
//!   unit is `<name>-term-<id>`, in `app.slice` with the applications, so the
//!   memory policy that watches that slice sees each terminal as a leaf of
//!   its own: one OOM kill takes one terminal, and none takes the desktop.
//! - `choom` because a scope has no exec context and `OOMScoreAdjust=` is
//!   refused on one: the kernel's own OOM killer must reach a terminal's
//!   program before it reaches the compositor (`foreign.rs` has the numbers).
//! - `setsid --fork` because `systemd-run --scope` is SYNCHRONOUS — it
//!   becomes the program — and a terminal lives for hours. Forked into a
//!   session of its own, the chain is nobody's child to wait for: `setsid`
//!   returns at once, the holder is adopted by the user manager (a
//!   subreaper), and a signal to the window's process group never reaches it.
//!
//! # Ready, or not
//!
//! The holder writes [`READY`] on its stdout once its socket listens and its
//! state file is written; [`start`] waits for that line, or for the pipe to
//! close (the chain died: no scope, no holder), or for [`START_DEADLINE`].
//! Readiness is a line, not a clock: nothing polls the socket.
//!
//! # As data first
//!
//! [`command`] is a pure function, and the tests read the command line
//! without running `systemd-run` — a test that ran it would start scopes in
//! the session of whoever ran the tests.

use std::ffi::OsString;
use std::io::{BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

use crate::wire::Size;
use crate::{GroupId, TerminalId};

/// The holder's program, found beside the one that starts it.
pub const HOLDER: &str = "ronoctua-term-holder";

/// What the holder writes on its stdout once it can be attached to.
pub const READY: &str = "ronoctua-term-ready";

/// How long [`start`] waits for a holder to say it is ready. A scope comes up
/// in tens of milliseconds; this is for a machine under load, and past it the
/// caller is told, not left waiting.
pub const START_DEADLINE: Duration = Duration::from_secs(10);

/// The slice the applications go in, which the memory policy watches
/// (`ronoctua_shell_kit::foreign::APP_SLICE` — the same slice, said here
/// because this crate links no shell).
pub const APP_SLICE: &str = "app.slice";

/// How much worse than the desktop a terminal is for the kernel's OOM killer:
/// an application's distance (`foreign::APP_OOM_SCORE_ADJUST`).
pub const OOM_SCORE_ADJUST: i32 = 500;

/// `setsid`, by store path: the caller may be a unit, whose `PATH` is minimal.
const SETSID: &str = match option_env!("RONOCTUA_SETSID") {
    Some(p) => p,
    None => "setsid",
};

/// `systemd-run`, by store path, for `setsid`'s reason.
const SYSTEMD_RUN: &str = match option_env!("RONOCTUA_SYSTEMD_RUN") {
    Some(p) => p,
    None => "systemd-run",
};

/// `choom`, by store path: inside the scope the `PATH` is the caller's, and
/// a `choom` it did not have would read as a terminal that never opens.
const CHOOM: &str = match option_env!("RONOCTUA_CHOOM") {
    Some(p) => p,
    None => "choom",
};

/// `true`, which the probe runs in a scope ([`scopes_work`]).
const TRUE: &str = match option_env!("RONOCTUA_TRUE") {
    Some(p) => p,
    None => "true",
};

/// The variables of a unit's watchdog (`sd_notify(3)`): whoever starts a
/// terminal may be a unit with a watchdog (the footer is), and a terminal —
/// its holder, its shell, every program in it — is nobody's to notify for
/// (`NotifyAccess=main` refuses it, and logs).
const WATCHDOG: [&str; 3] = ["NOTIFY_SOCKET", "WATCHDOG_USEC", "WATCHDOG_PID"];

/// A command for `program`, born without the starter's watchdog.
fn unwatched(program: &std::ffi::OsStr) -> Command {
    let mut command = Command::new(program);
    for name in WATCHDOG {
        command.env_remove(name);
    }
    command
}

/// The shape of a terminal's cursor, as the person chose it for every new
/// terminal (`terminal.json`) and as its holder is told at birth: a block, a
/// bar or an underline — Ghostty's `cursor-style`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CursorShape {
    /// A block over the cell.
    #[default]
    Block,
    /// A bar before it.
    Bar,
    /// A line under it.
    Underline,
}

impl CursorShape {
    /// Every shape, in the order a ring turns them.
    pub const ALL: [Self; 3] = [Self::Block, Self::Bar, Self::Underline];

    /// Its word on the holder's command line.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Bar => "bar",
            Self::Underline => "underline",
        }
    }

    /// The shape a word names.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.word() == word)
    }
}

/// What the person chose for every new terminal, which its holder takes at
/// birth (`--scrollback`, `--cursor`): how much history its engine keeps —
/// a limit set before the first byte, as Ghostty sets `scrollback-limit` —
/// and the cursor it starts with and a program's reset goes back to. `None`
/// is the engine's own. A window needs no word of them: the engine's
/// snapshot carries both, and the mirror decoded from it keeps them
/// (measured, `ronoctua-term`: a mirror goes back to its holder's cursor, and
/// keeps its holder's history).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Born {
    /// The most history kept, in bytes.
    pub scrollback_bytes: Option<u64>,
    /// The cursor: its shape, and whether it blinks.
    pub cursor: Option<(CursorShape, bool)>,
}

/// What a new terminal runs, where, and where its window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    /// The program.
    pub program: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// Where it starts; `None` for the caller's own folder.
    pub cwd: Option<PathBuf>,
    /// The group its window shows it in.
    pub group: GroupId,
    /// Its place there.
    pub order: u32,
    /// The size it is born at: a shell that starts at 80×24 and is resized a
    /// moment later has already drawn its prompt at the wrong width.
    pub size: Size,
    /// What the person chose for it.
    pub born: Born,
}

/// The holder's own arguments (`ronoctua-term-holder serve …`), written by
/// [`Serve::to_args`] and read by [`Serve::parse`] — one place, so the
/// spawner and the holder cannot come to spell them differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serve {
    /// The terminal.
    pub id: TerminalId,
    /// Its group.
    pub group: GroupId,
    /// Its place there.
    pub order: u32,
    /// Its size at birth.
    pub size: Size,
    /// Where its program starts.
    pub cwd: Option<PathBuf>,
    /// The program.
    pub program: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// What the person chose for it.
    pub born: Born,
}

impl Serve {
    /// The holder's arguments for a terminal: `serve` and its flags, then
    /// `--` and the program. Everything after `--` is the program's, so an
    /// argument of its own that starts with `--` is never read as ours.
    #[must_use]
    pub fn to_args(&self) -> Vec<OsString> {
        let mut out: Vec<OsString> = vec![
            "serve".into(),
            "--id".into(),
            self.id.as_str().into(),
            "--group".into(),
            self.group.as_str().into(),
            "--order".into(),
            self.order.to_string().into(),
            "--size".into(),
            format!(
                "{}x{}@{}x{}",
                self.size.columns,
                self.size.rows,
                self.size.cell_width_px,
                self.size.cell_height_px
            )
            .into(),
        ];
        if let Some(cwd) = &self.cwd {
            out.push("--cwd".into());
            out.push(cwd.clone().into_os_string());
        }
        if let Some(bytes) = self.born.scrollback_bytes {
            out.push("--scrollback".into());
            out.push(bytes.to_string().into());
        }
        if let Some((shape, blinks)) = self.born.cursor {
            out.push("--cursor".into());
            out.push(format!("{}{}", shape.word(), if blinks { BLINKS } else { "" }).into());
        }
        out.push("--".into());
        out.push(self.program.clone().into());
        out.extend(self.args.iter().map(OsString::from));
        out
    }

    /// The holder's arguments, back — what `ronoctua-term-holder` was given,
    /// without its own name.
    ///
    /// # Errors
    ///
    /// A sentence naming what is missing or malformed.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut args = args.into_iter();
        if args.next().as_deref() != Some("serve".as_ref()) {
            return Err("the first argument is not `serve`".to_owned());
        }
        let (mut id, mut group, mut order, mut size, mut cwd) = (None, None, None, None, None);
        let mut born = Born::default();
        let text = |v: Option<OsString>, flag: &str| -> Result<String, String> {
            v.ok_or_else(|| format!("{flag} has no value"))?
                .into_string()
                .map_err(|_| format!("{flag} is not text"))
        };
        loop {
            let Some(flag) = args.next() else {
                return Err("no `--` and no program".to_owned());
            };
            match flag.to_str() {
                Some("--id") => {
                    id = Some(
                        text(args.next(), "--id")?
                            .parse()
                            .map_err(|e| format!("--id: {e}"))?,
                    );
                }
                Some("--group") => {
                    group = Some(
                        text(args.next(), "--group")?
                            .parse()
                            .map_err(|e| format!("--group: {e}"))?,
                    );
                }
                Some("--order") => {
                    order = Some(
                        text(args.next(), "--order")?
                            .parse()
                            .map_err(|_| "--order is not a number".to_owned())?,
                    );
                }
                Some("--size") => size = Some(size_of(&text(args.next(), "--size")?)?),
                Some("--cwd") => {
                    cwd = Some(PathBuf::from(
                        args.next().ok_or_else(|| "--cwd has no value".to_owned())?,
                    ));
                }
                Some("--scrollback") => {
                    born.scrollback_bytes = Some(
                        text(args.next(), "--scrollback")?
                            .parse()
                            .map_err(|_| "--scrollback is not a number of bytes".to_owned())?,
                    );
                }
                Some("--cursor") => {
                    let said = text(args.next(), "--cursor")?;
                    let (word, blinks) = said
                        .strip_suffix(BLINKS)
                        .map_or((said.as_str(), false), |w| (w, true));
                    let shape = CursorShape::of_word(word)
                        .ok_or_else(|| format!("--cursor {said:?} is no cursor"))?;
                    born.cursor = Some((shape, blinks));
                }
                Some("--") => break,
                _ => return Err(format!("{} is no flag of `serve`", flag.to_string_lossy())),
            }
        }
        let program = text(args.next(), "the program")?;
        let args = args
            .map(|a| {
                a.into_string()
                    .map_err(|_| "an argument is not text".to_owned())
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            id: id.ok_or("no --id")?,
            group: group.ok_or("no --group")?,
            order: order.ok_or("no --order")?,
            size: size.ok_or("no --size")?,
            cwd,
            program,
            args,
            born,
        })
    }
}

/// What a blinking cursor's word ends in on the holder's command line
/// (`--cursor bar-blinking`).
const BLINKS: &str = "-blinking";

/// `COLUMNSxROWS@WIDTHxHEIGHT`, back.
fn size_of(text: &str) -> Result<Size, String> {
    let wrong = || format!("--size {text:?} is not COLUMNSxROWS@WIDTHxHEIGHT");
    let (cells, px) = text.split_once('@').ok_or_else(wrong)?;
    let pair = |t: &str| -> Option<(u16, u16)> {
        let (a, b) = t.split_once('x')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    };
    let ((columns, rows), (cell_width_px, cell_height_px)) =
        (pair(cells).ok_or_else(wrong)?, pair(px).ok_or_else(wrong)?);
    Ok(Size {
        columns,
        rows,
        cell_width_px,
        cell_height_px,
    })
}

/// A terminal's unit: `<name>-term-<id>`, the instance's name first, so a
/// nested loop's terminals are never the installed session's.
#[must_use]
pub fn unit_name(name: &str, id: &TerminalId) -> String {
    format!("{name}-term-{id}")
}

/// The options that put a program in a terminal's kind of scope — the start
/// and the probe use the same ones, so a property this systemd refuses fails
/// the probe exactly as it would fail every start.
fn scope_options(unit: &str) -> Vec<OsString> {
    vec![
        "--user".into(),
        "--scope".into(),
        "--quiet".into(),
        // A scope that ended — or failed — leaves no unit behind to collect
        // by hand: a terminal that ended with a code is not a failure to
        // keep.
        "--collect".into(),
        format!("--unit={unit}").into(),
        format!("--slice={APP_SLICE}").into(),
        // The scope dies whole, and stays a leaf the OOM policy can pick.
        // (`OOMScoreAdjust=` is an exec property, refused on a scope:
        // hence `choom`.)
        "--property=OOMPolicy=kill".into(),
    ]
}

/// The whole command line that starts a holder for `serve`, in this
/// instance (`name`), with the holder program at `holder`.
#[must_use]
pub fn command(name: &str, serve: &Serve, holder: &Path) -> Vec<OsString> {
    let mut c: Vec<OsString> = vec![SETSID.into(), "--fork".into(), SYSTEMD_RUN.into()];
    c.extend(scope_options(&unit_name(name, &serve.id)));
    c.extend([
        CHOOM.into(),
        "-n".into(),
        OOM_SCORE_ADJUST.to_string().into(),
        // Without it `choom` would read the holder's own flags as its.
        "--".into(),
        holder.as_os_str().to_owned(),
    ]);
    c.extend(serve.to_args());
    c
}

/// The holder program, beside the running one — `None` if it is not there.
///
/// Beside, and not a store path baked in: the shell's bundle copies the
/// holder next to it (`beside` in this crate's metadata), and a path to its
/// own unit would be a derivation naming its own output.
#[must_use]
pub fn holder_beside() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    let it = me.parent()?.join(HOLDER);
    it.is_file().then_some(it)
}

/// Starts a holder for `spawn`, in a scope of its own, and answers its id
/// once it can be attached to.
///
/// It BLOCKS — up to [`START_DEADLINE`] — so it is called where a launch is:
/// on a thread of its own, never on an event loop. Ask [`scopes_work`]
/// first; without scopes, the terminal runs in the window's own process.
///
/// # Errors
///
/// No id could be made, no holder is beside this program, the chain could
/// not be run, or the holder did not say it was ready in time.
pub fn start(spawn: &Spawn) -> Result<TerminalId> {
    let id = TerminalId::fresh().context("a fresh terminal id")?;
    let serve = Serve {
        id: id.clone(),
        group: spawn.group.clone(),
        order: spawn.order,
        size: spawn.size,
        cwd: spawn.cwd.clone(),
        program: spawn.program.clone(),
        args: spawn.args.clone(),
        born: spawn.born,
    };
    let holder =
        holder_beside().with_context(|| format!("there is no {HOLDER} beside this program"))?;
    let argv = command(&ronoctua_xdg::name(), &serve, &holder);
    launch(&argv, START_DEADLINE)?;
    Ok(id)
}

/// Runs `argv` and waits until what it started says [`READY`] on its stdout,
/// or the pipe closes, or `deadline` passes.
///
/// The waiting happens on a thread of its own: a chain that hangs before it
/// says anything would otherwise hold the caller past the deadline. That
/// thread ends when the pipe does.
///
/// # Errors
///
/// The command could not be run, the pipe closed without the line (the
/// chain died), or the deadline passed.
pub fn launch(argv: &[OsString], deadline: Duration) -> Result<()> {
    let (program, args) = argv.split_first().context("an empty command")?;
    let mut child = unwatched(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("could not run {}", program.to_string_lossy()))?;
    let stdout = child.stdout.take().context("the chain's stdout")?;
    let (said, heard) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("term-start".to_owned())
        .spawn(move || {
            let mut line = String::new();
            let ready =
                BufReader::new(stdout).read_line(&mut line).is_ok() && line.trim_end() == READY;
            let _ = said.send(ready);
        })
        .context("could not start the thread that waits for the holder")?;
    let answer = heard.recv_timeout(deadline);
    // `setsid --fork` returns as soon as it forked: reaping it is immediate,
    // and the holder is nobody's child from here on.
    let _ = child.wait();
    match answer {
        Ok(true) => Ok(()),
        Ok(false) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            bail!("the terminal's holder ended before it was ready")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => bail!(
            "the terminal's holder did not say it was ready in {} s",
            deadline.as_secs()
        ),
    }
}

/// The probe's command: a terminal's kind of scope around `true`, under a
/// name of its own (the process id keeps two callers' probes apart).
#[must_use]
pub fn probe_command(name: &str, pid: u32) -> Vec<OsString> {
    let mut c: Vec<OsString> = vec![SYSTEMD_RUN.into()];
    c.extend(scope_options(&format!("{name}-term-probe-{pid}")));
    c.extend([
        CHOOM.into(),
        "-n".into(),
        OOM_SCORE_ADJUST.to_string().into(),
        "--".into(),
        TRUE.into(),
    ]);
    c
}

/// Whether this session can give a terminal a scope — asked once per
/// process, by a scope around `true` with the very options a start uses,
/// and remembered.
///
/// It BLOCKS for the life of `true`, once: call it where [`start`] is
/// called. `false` — no user manager, a session started by hand — means the
/// terminal runs in the window's own process, and the window says so.
#[must_use]
pub fn scopes_work() -> bool {
    static SCOPES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SCOPES.get_or_init(|| {
        let argv = probe_command(&ronoctua_xdg::name(), std::process::id());
        let Some((program, args)) = argv.split_first() else {
            return false;
        };
        unwatched(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|st| st.success())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serve() -> Serve {
        Serve {
            id: "0123456789abcdef".parse().unwrap(),
            group: "fedcba9876543210".parse().unwrap(),
            order: 3,
            size: Size {
                columns: 120,
                rows: 40,
                cell_width_px: 10,
                cell_height_px: 21,
            },
            cwd: Some(PathBuf::from("/home/someone/src")),
            program: "zsh".to_owned(),
            args: vec!["--login".to_owned(), "-c".to_owned(), "--id x".to_owned()],
            born: Born {
                scrollback_bytes: Some(50_000_000),
                cursor: Some((CursorShape::Bar, true)),
            },
        }
    }

    fn texts(c: &[OsString]) -> Vec<String> {
        c.iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn the_holders_arguments_read_back_as_they_were_written() {
        // FAILS IF the spawner and the holder spell an argument differently,
        // or a program's own `--flag` is read as the holder's.
        let s = serve();
        assert_eq!(Serve::parse(s.to_args()).unwrap(), s);
        let bare = Serve {
            cwd: None,
            args: Vec::new(),
            born: Born::default(),
            ..serve()
        };
        assert_eq!(Serve::parse(bare.to_args()).unwrap(), bare);
        // EVERY CURSOR, blinking or not: a shape spelt one way by the spawner
        // and read another by the holder would start every terminal with
        // the engine's block.
        for shape in CursorShape::ALL {
            for blinks in [false, true] {
                let s = Serve {
                    born: Born {
                        scrollback_bytes: None,
                        cursor: Some((shape, blinks)),
                    },
                    ..serve()
                };
                assert_eq!(Serve::parse(s.to_args()).unwrap(), s);
            }
        }
        assert!(Serve::parse(vec![OsString::from("serve"), "--id".into()]).is_err());
        assert!(Serve::parse(vec![OsString::from("attach")]).is_err());
        let mut wrong = serve().to_args();
        let at = wrong.iter().position(|a| a == "--cursor").unwrap();
        wrong[at + 1] = "hollow".into();
        assert!(
            Serve::parse(wrong).is_err(),
            "a cursor that is none was taken"
        );
    }

    #[test]
    fn the_command_puts_the_holder_in_a_scope_of_its_own() {
        // FAILS IF the holder is started outside a scope (it would die with
        // the window that started it), outside the slice the memory policy
        // watches, without `choom`, under a unit name another instance could
        // share, or with a holder other than the one beside this program.
        let c = texts(&command(
            "ronoctua-dev",
            &serve(),
            Path::new("/bin/dir/ronoctua-term-holder"),
        ));
        assert!(c[0].ends_with("setsid") && c[1] == "--fork", "{c:?}");
        assert!(c[2].ends_with("systemd-run"), "{c:?}");
        for want in [
            "--user",
            "--scope",
            "--collect",
            "--unit=ronoctua-dev-term-0123456789abcdef",
            "--slice=app.slice",
            "--property=OOMPolicy=kill",
        ] {
            assert!(c.iter().any(|a| a == want), "{want} is missing: {c:?}");
        }
        let choom = c.iter().position(|a| a.ends_with("choom")).unwrap();
        assert_eq!(
            c[choom + 1..choom + 5],
            ["-n", "500", "--", "/bin/dir/ronoctua-term-holder"]
        );
        let serve_at = choom + 5;
        assert_eq!(c[serve_at], "serve");
        assert!(
            c.iter()
                .position(|a| a == "--unit=ronoctua-dev-term-0123456789abcdef")
                .unwrap()
                < choom,
            "systemd-run's options come before the program"
        );
        assert_eq!(
            Serve::parse(command("ronoctua", &serve(), Path::new("/h"))[serve_at..].to_vec())
                .unwrap(),
            serve()
        );
    }

    #[test]
    fn the_probe_asks_with_the_starts_own_options() {
        // FAILS IF the probe and a start drift apart: a property this systemd
        // refuses would pass the probe and fail every terminal in silence.
        let probe = texts(&probe_command("ronoctua", 77));
        let start = texts(&command("ronoctua", &serve(), Path::new("/h")));
        let options = |c: &[String]| -> Vec<String> {
            c.iter()
                .filter(|a| a.starts_with("--") && !a.starts_with("--unit="))
                .take_while(|a| *a != "--")
                .cloned()
                .collect()
        };
        assert_eq!(options(&probe), options(&start[2..]));
        assert!(probe.iter().any(|a| a == "--unit=ronoctua-term-probe-77"));
        assert!(probe.last().unwrap().ends_with("true"));
    }

    #[test]
    fn a_terminal_is_born_without_its_starters_watchdog() {
        // FAILS IF a holder started by the footer inherits the footer's
        // watchdog: every program in the terminal could then speak to systemd
        // for the footer's unit. Asked of the command as data: it is the
        // command that removes them, whatever this process has.
        let command = unwatched("/bin/true".as_ref());
        let removed: Vec<_> = command
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        for name in WATCHDOG {
            assert!(
                removed.iter().any(|r| r == name),
                "{name} reaches it: {removed:?}"
            );
        }
    }

    #[test]
    fn a_start_waits_for_the_ready_line_and_no_longer() {
        // FAILS IF a start returns before the holder said it is ready, waits
        // for a chain that died, or takes a different line for readiness —
        // tested with `sh` standing in for the chain, never `systemd-run`.
        let sh =
            |script: &str| -> Vec<OsString> { vec!["/bin/sh".into(), "-c".into(), script.into()] };
        let long = Duration::from_secs(20);
        launch(&sh(&format!("echo {READY}; exec sleep 0")), long).unwrap();
        let died = launch(&sh("exit 3"), long).unwrap_err();
        assert!(died.to_string().contains("ended before"), "{died}");
        let other = launch(&sh("echo something-else"), long).unwrap_err();
        assert!(other.to_string().contains("ended before"), "{other}");
    }
}
