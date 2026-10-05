//! The passwordless-sudo window.
//!
//! An approved exception to invariant 6: the file lives in `/run` and expires
//! by itself. The root helper writes [`sudoers_line`] into the draft
//! [`DRAFT`] (a name with a dot, which sudo's `@includedir` skips), checks it
//! with `visudo -cf` by store path, and renames it to [`FILE`] (`0440`,
//! root). Four locks close it: a transient timer armed for the end
//! ([`arm_arguments`]), the `NOTAFTER` in the line itself, `/run` being
//! tmpfs, and a unit that removes it before sleep (autologin, no screen
//! lock). The modeline shows the END time, read from [`UNTIL`]
//! ([`open_until`]).
//!
//! # Why the line is written and never escaped
//!
//! A syntax error in an included sudoers file locks EVERY use of sudo out,
//! the rescue included. So the line has one fixed shape, its only variable
//! parts are a [`crate::users::plain`] name and a date of digits, the draft
//! is checked by the same `visudo` before it can be read, and
//! `relax-line-parses` puts the formatter's output through `visudo` at build
//! time.
//!
//! # Why `SETENV`
//!
//! The window's line comes after the wheel rule, and the last rule that
//! matches is the one sudo follows. The wheel rule NixOS writes carries
//! `SETENV` (`security.sudo.defaultOptions`), so a line without it would
//! refuse `sudo -E` and `--preserve-env` for exactly as long as the window is
//! open. The window takes the password away and nothing else.

use std::path::{Path, PathBuf};

use crate::tools::{self, Tools};
use crate::users::plain;
use crate::{Refusal, Why, refused};

/// The folder sudo includes (`@includedir`).
pub const DIR: &str = "/run/ronoctua-sudo";
/// The window's file in [`DIR`].
pub const FILE: &str = "relax";
/// The draft, checked before it is renamed to [`FILE`].
pub const DRAFT: &str = ".relax";
/// When the window closes, in seconds since the epoch, for the modeline
/// (`0644`).
pub const UNTIL: &str = "/run/ronoctua-security/sudo-until";
/// The expiry timer's name before its end time (`<prefix>-<until>`): every
/// window has its own, so arming a new one never meets the old one's name.
pub const EXPIRY_UNIT_PREFIX: &str = "ronoctua-sudo-expire";

/// `(year, month, day)` of a day count since the epoch (proleptic
/// Gregorian).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// sudo's `Date_Spec` of an instant, in UTC: `yyyymmddHHMMSSZ`.
#[must_use]
pub fn notafter(until_unix_s: i64) -> String {
    let (year, month, day) = civil(until_unix_s.div_euclid(86_400));
    let second = until_unix_s.rem_euclid(86_400);
    format!(
        "{year:04}{month:02}{day:02}{:02}{:02}{:02}Z",
        second / 3600,
        second % 3600 / 60,
        second % 60
    )
}

/// The window's one line for `user`, closing at `until_unix_s` (the file's
/// content is this line and a line break): the wheel rule's `SETENV`, and no
/// password.
///
/// # Errors
///
/// [`Why::Invalid`] for a user name that is not plain, or an instant outside
/// the years 1970–9999.
pub fn sudoers_line(user: &str, until_unix_s: i64) -> Result<String, Refusal> {
    // 253402300799 is 9999-12-31 23:59:59: past it the year has five digits
    // and the date is not one sudo reads.
    if !plain(user) || !(0..=253_402_300_799).contains(&until_unix_s) {
        return Err(refused(Why::Invalid));
    }
    Ok(format!(
        "{user} ALL=(ALL:ALL) NOTAFTER={} NOPASSWD:SETENV: ALL",
        notafter(until_unix_s)
    ))
}

/// The expiry timer of the window that closes at `until_unix_s`.
#[must_use]
pub fn expiry_unit(until_unix_s: i64) -> String {
    format!("{EXPIRY_UNIT_PREFIX}-{until_unix_s}")
}

/// `systemd-run`'s arguments for the expiry timer: in `minutes`, to the
/// second, `helper relax-end` as root. Pure.
#[must_use]
pub fn arm_arguments(until_unix_s: i64, minutes: u32, helper: &Path) -> Vec<String> {
    vec![
        format!("--unit={}", expiry_unit(until_unix_s)),
        format!("--on-active={minutes}m"),
        "--timer-property=AccuracySec=1s".to_owned(),
        "--description=Ends the passwordless sudo window".to_owned(),
        "--collect".to_owned(),
        "--quiet".to_owned(),
        "--".to_owned(),
        helper.display().to_string(),
        "relax-end".to_owned(),
    ]
}

/// `systemctl`'s arguments that stop every window's expiry timer. Pure.
#[must_use]
pub fn disarm_arguments() -> Vec<String> {
    vec!["stop".to_owned(), format!("{EXPIRY_UNIT_PREFIX}-*.timer")]
}

/// Where the window lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// The folder sudo includes.
    pub dir: PathBuf,
    /// The end time's file.
    pub until: PathBuf,
}

impl Window {
    /// The machine's.
    #[must_use]
    pub fn system() -> Self {
        Self {
            dir: PathBuf::from(DIR),
            until: PathBuf::from(UNTIL),
        }
    }

    /// Opens the window for `user` until `until_unix_s`: the draft, `visudo`,
    /// the rename, the end time. The timer is armed by the caller, after.
    ///
    /// A window already open stays as it was when the new line never goes
    /// live (a refused draft); once the new line is live, a failure closes
    /// the window whole — an end time that says another hour is worse than
    /// no window.
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`] (see [`sudoers_line`]), [`Why::Failed`] when `visudo`
    /// refuses the draft or a file cannot be written (no draft is left
    /// behind), [`Why::Unstartable`] without `visudo`.
    pub fn open(&self, user: &str, until_unix_s: i64, visudo: &Path) -> Result<(), Refusal> {
        let line = sudoers_line(user, until_unix_s)?;
        let draft = self.dir.join(DRAFT);
        let file = self.dir.join(FILE);
        let failed = |e: std::io::Error| {
            eprintln!("<3>the window could not be written: {e}");
            refused(Why::Failed)
        };
        let _ = std::fs::remove_file(&draft);
        {
            use std::io::Write as _;
            use std::os::unix::fs::OpenOptionsExt as _;
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o440)
                .open(&draft)
                .map_err(failed)?;
            out.write_all(format!("{line}\n").as_bytes())
                .and_then(|()| out.sync_all())
                .map_err(failed)?;
        }
        let draft_text = draft.display().to_string();
        if let Err(refusal) = tools::run(visudo, &["-c", "-f", &draft_text], None) {
            let _ = std::fs::remove_file(&draft);
            return Err(refusal);
        }
        if let Err(e) = std::fs::rename(&draft, &file) {
            let _ = std::fs::remove_file(&draft);
            return Err(failed(e));
        }
        crate::write_whole(&self.until, format!("{until_unix_s}\n").as_bytes(), 0o644).map_err(
            |e| {
                let _ = self.close();
                failed(e)
            },
        )
    }

    /// Closes the window: the line, a draft left over, the end time. Closing
    /// a closed window is not an error.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when the line could not be removed (the others are
    /// tried all the same).
    pub fn close(&self) -> Result<(), Refusal> {
        let gone = |path: &Path| match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => {
                eprintln!("<3>{} could not be removed: {e}", path.display());
                Err(refused(Why::Failed))
            }
        };
        let line = gone(&self.dir.join(FILE));
        let draft = gone(&self.dir.join(DRAFT));
        let until = gone(&self.until);
        line.and(draft).and(until)
    }

    /// When the open window closes; `None` when none is open (or the file
    /// says something that is not a time).
    #[must_use]
    pub fn open_until(&self) -> Option<i64> {
        std::fs::read_to_string(&self.until)
            .ok()?
            .trim()
            .parse()
            .ok()
    }
}

/// When the machine's open window closes, for the modeline and the page;
/// `None` when no window is open. Read as the person (`0644`).
#[must_use]
pub fn open_until() -> Option<i64> {
    Window::system().open_until()
}

/// Opens the window for `minutes`: writes the line, stops every old expiry
/// timer, and arms the new one to run `helper relax-end`. The old timers
/// stop only once the new line is live — a reopening that fails leaves the
/// open window with its own timer. If the new timer cannot be armed the
/// window is closed again: three locks are not the four the person was
/// promised.
///
/// # Errors
///
/// What [`Window::open`] refuses; [`Why::Unstartable`] or [`Why::Failed`]
/// for the timer.
pub fn open(
    window: &Window,
    tools: &Tools,
    user: &str,
    minutes: u32,
    helper: &Path,
) -> Result<i64, Refusal> {
    let visudo = tools::given(tools.visudo.as_ref())?;
    let systemd_run = tools::given(tools.systemd_run.as_ref())?;
    let until = crate::now_unix_s() + i64::from(minutes) * 60;
    window.open(user, until, visudo)?;
    disarm(tools);
    let arguments = arm_arguments(until, minutes, helper);
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    if let Err(refusal) = tools::run(systemd_run, &arguments, None) {
        let _ = window.close();
        return Err(refusal);
    }
    Ok(until)
}

/// Closes the window and stops its timers.
///
/// # Errors
///
/// What [`Window::close`] refuses.
pub fn close(window: &Window, tools: &Tools) -> Result<(), Refusal> {
    let closed = window.close();
    disarm(tools);
    closed
}

/// Stops every window's expiry timer. A timer that is not there is not a
/// failure, and neither is a machine without `systemctl`: the line's own
/// `NOTAFTER` still holds.
fn disarm(tools: &Tools) {
    if let Some(systemctl) = tools.systemctl.as_deref() {
        let arguments = disarm_arguments();
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        let _ = tools::run(systemctl, &arguments, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn the_line_closes_itself_at_the_end() {
        assert_eq!(
            sudoers_line("someone", 1_790_000_000).unwrap(),
            "someone ALL=(ALL:ALL) NOTAFTER=20260921141320Z NOPASSWD:SETENV: ALL"
        );
        assert_eq!(notafter(0), "19700101000000Z");
        assert_eq!(notafter(951_782_400), "20000229000000Z", "a leap day");
        assert_eq!(notafter(4_102_444_799), "20991231235959Z");
        assert_eq!(notafter(253_402_300_799), "99991231235959Z");
        for bad in [
            "",
            "ALL",
            "root ALL",
            "a\nb",
            "-x",
            "9lives",
            "x,y",
            &"a".repeat(33),
        ] {
            assert_eq!(
                sudoers_line(bad, 0).unwrap_err().why,
                Why::Invalid,
                "{bad:?}"
            );
        }
        for bad_time in [-1, 253_402_300_800, i64::MAX] {
            assert_eq!(
                sudoers_line("someone", bad_time).unwrap_err().why,
                Why::Invalid
            );
        }
    }

    #[test]
    fn the_timer_runs_this_helper_to_the_second() {
        assert_eq!(
            arm_arguments(
                1_790_000_000,
                15,
                Path::new("/nix/store/x/bin/ronoctua-security-root")
            ),
            [
                "--unit=ronoctua-sudo-expire-1790000000",
                "--on-active=15m",
                "--timer-property=AccuracySec=1s",
                "--description=Ends the passwordless sudo window",
                "--collect",
                "--quiet",
                "--",
                "/nix/store/x/bin/ronoctua-security-root",
                "relax-end",
            ]
        );
        assert_eq!(disarm_arguments(), ["stop", "ronoctua-sudo-expire-*.timer"]);
    }

    /// A `visudo` that passes what the real one passes for this line and
    /// refuses a draft with anything else in it.
    fn fake_visudo(dir: &Scratch) -> PathBuf {
        dir.program(
            "visudo",
            "#!/bin/sh\n\
             [ \"$1 $2\" = \"-c -f\" ] || exit 2\n\
             IFS= read -r line < \"$3\"\n\
             case \"$line\" in\n\
             *' ALL=(ALL:ALL) NOTAFTER='*'Z NOPASSWD:SETENV: ALL') exit 0 ;;\n\
             esac\n\
             exit 1\n",
        )
    }

    #[test]
    fn a_window_opens_whole_and_closes_whole() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = Scratch::new("relax");
        let visudo = fake_visudo(&dir);
        let window = Window {
            dir: dir.0.join("sudo"),
            until: dir.0.join("sudo-until"),
        };
        std::fs::create_dir(&window.dir).unwrap();
        assert_eq!(window.open_until(), None);
        window.open("someone", 1_790_000_000, &visudo).unwrap();
        let file = window.dir.join(FILE);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "someone ALL=(ALL:ALL) NOTAFTER=20260921141320Z NOPASSWD:SETENV: ALL\n"
        );
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o440);
        assert!(!window.dir.join(DRAFT).exists(), "the draft was renamed");
        assert_eq!(window.open_until(), Some(1_790_000_000));
        window.close().unwrap();
        assert!(!file.exists());
        assert_eq!(window.open_until(), None);
        window.close().unwrap();
    }

    #[test]
    fn a_line_visudo_refuses_never_goes_live() {
        let dir = Scratch::new("relax-refused");
        let refusing = dir.program("visudo", "#!/bin/sh\nexit 1\n");
        let window = Window {
            dir: dir.0.join("sudo"),
            until: dir.0.join("sudo-until"),
        };
        std::fs::create_dir(&window.dir).unwrap();
        assert_eq!(
            window
                .open("someone", 1_790_000_000, &refusing)
                .unwrap_err()
                .why,
            Why::Failed
        );
        let left: Vec<_> = std::fs::read_dir(&window.dir).unwrap().collect();
        assert!(left.is_empty(), "nothing is left in the included folder");
        assert!(!window.until.exists());
    }

    #[test]
    fn a_refused_reopening_leaves_the_open_window_and_its_timer() {
        let dir = Scratch::new("relax-reopen");
        let window = Window {
            dir: dir.0.join("sudo"),
            until: dir.0.join("sudo-until"),
        };
        std::fs::create_dir(&window.dir).unwrap();
        let stopped = dir.0.join("stopped");
        let tools = Tools {
            visudo: Some(fake_visudo(&dir)),
            systemd_run: Some(dir.program("systemd-run", "#!/bin/sh\nexit 0\n")),
            systemctl: Some(dir.program(
                "systemctl",
                &format!(
                    "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\n",
                    stopped.display()
                ),
            )),
            nft: None,
        };
        let until = open(&window, &tools, "someone", 5, Path::new("/helper")).unwrap();
        let line = std::fs::read_to_string(window.dir.join(FILE)).unwrap();
        std::fs::remove_file(&stopped).unwrap();

        // visudo refuses the new line: the open window, its end time and its
        // timer are all as they were.
        let refusing = Tools {
            visudo: Some(dir.program("visudo-no", "#!/bin/sh\nexit 1\n")),
            ..tools.clone()
        };
        assert_eq!(
            open(&window, &refusing, "someone", 60, Path::new("/helper"))
                .unwrap_err()
                .why,
            Why::Failed
        );
        assert_eq!(
            std::fs::read_to_string(window.dir.join(FILE)).unwrap(),
            line
        );
        assert_eq!(window.open_until(), Some(until));
        assert!(!stopped.exists(), "no timer was stopped");

        // Accepted, the old timers stop — after the new line is live.
        open(&window, &tools, "someone", 60, Path::new("/helper")).unwrap();
        assert_eq!(
            std::fs::read_to_string(&stopped).unwrap(),
            "stop ronoctua-sudo-expire-*.timer\n"
        );
    }

    #[test]
    fn a_window_whose_timer_cannot_be_armed_is_closed_again() {
        let dir = Scratch::new("relax-timer");
        let window = Window {
            dir: dir.0.join("sudo"),
            until: dir.0.join("sudo-until"),
        };
        std::fs::create_dir(&window.dir).unwrap();
        let tools = Tools {
            visudo: Some(fake_visudo(&dir)),
            systemd_run: Some(dir.program("systemd-run", "#!/bin/sh\nexit 1\n")),
            systemctl: Some(dir.program("systemctl", "#!/bin/sh\nexit 0\n")),
            nft: None,
        };
        assert_eq!(
            open(&window, &tools, "someone", 5, Path::new("/helper"))
                .unwrap_err()
                .why,
            Why::Failed
        );
        assert!(!window.dir.join(FILE).exists());
        let armed = dir.0.join("armed");
        let tools = Tools {
            systemd_run: Some(dir.program(
                "systemd-run",
                &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n", armed.display()),
            )),
            ..tools
        };
        let until = open(&window, &tools, "someone", 5, Path::new("/helper")).unwrap();
        assert!(window.dir.join(FILE).exists());
        let said = std::fs::read_to_string(&armed).unwrap();
        assert!(
            said.contains(&format!("--unit=ronoctua-sudo-expire-{until}")),
            "{said}"
        );
        assert!(said.ends_with("/helper\nrelax-end\n"), "{said}");
        close(&window, &tools).unwrap();
        assert!(!window.dir.join(FILE).exists());
    }
}
