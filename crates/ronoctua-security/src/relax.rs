//! The passwordless-sudo window.
//!
//! An approved exception to invariant 6 (owner, 2026-09-30, amended
//! 2026-10-11): one line in a root folder that survives a reboot. The root
//! helper writes [`sudoers_line`] into the draft [`DRAFT`] (a name with a
//! dot, which sudo's `@includedir` skips), checks it with `visudo -cf` by
//! store path, renames it to [`FILE`] (`0440`, root), and writes what is open
//! to [`STATE`] (`0644`, read by the doors, the modeline and the brain).
//!
//! # How it ends
//!
//! At its end, or when the person ends it — never because the machine slept
//! or restarted (owner, 2026-10-11). A timed window has sudo's own
//! `NOTAFTER` in the line, which is the guard that holds whatever happens to
//! the rest; a transient timer armed for the end by the wall clock
//! ([`arm_arguments`], which a suspend does not delay) takes the line away
//! then; and at every boot `ronoctua-sudo-window` looks at it again
//! ([`boot`]): an expired line goes, a live one gets its timer back (a
//! transient timer does not survive a reboot). A window with no limit has no
//! `NOTAFTER` and no timer: only the person closes it.
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

use serde::{Deserialize, Serialize};

use crate::tools::{self, Tools};
use crate::users::plain;
use crate::verb::{RelaxSpan, RootVerb};
use crate::{Refusal, Why, refused};

/// The folder sudo includes (`@includedir`): root's, persisted
/// (`nix/modules/persist.nix`), so the window outlives a reboot.
pub const DIR: &str = "/var/lib/ronoctua-sudo";
/// The window's file in [`DIR`].
pub const FILE: &str = "relax";
/// The draft, checked before it is renamed to [`FILE`].
pub const DRAFT: &str = ".relax";
/// What is open — its end and its span, as [`Opened`] — for the doors, the
/// modeline and the brain (`0644`). NEVER in [`DIR`]: sudo would read it as
/// sudoers, and a syntax error there locks every sudo out.
pub const STATE: &str = "/var/lib/ronoctua-security/sudo-window";
/// The format of [`STATE`].
pub const STATE_FORMAT: u32 = 1;
/// The expiry timer's name before its end time (`<prefix>-<until>`): every
/// window has its own, so arming a new one never meets the old one's name.
pub const EXPIRY_UNIT_PREFIX: &str = "ronoctua-sudo-expire";

/// The last instant a sudoers date can say: 9999-12-31 23:59:59. Past it the
/// year has five digits and the date is not one sudo reads.
const LAST_INSTANT_S: i64 = 253_402_300_799;

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

/// `(year, month, day, hour, minute, second)` of an instant, in UTC.
fn utc(at_unix_s: i64) -> (i64, u32, u32, i64, i64, i64) {
    let (year, month, day) = civil(at_unix_s.div_euclid(86_400));
    let second = at_unix_s.rem_euclid(86_400);
    (
        year,
        month,
        day,
        second / 3600,
        second % 3600 / 60,
        second % 60,
    )
}

/// sudo's `Date_Spec` of an instant, in UTC: `yyyymmddHHMMSSZ`.
#[must_use]
pub fn notafter(until_unix_s: i64) -> String {
    let (year, month, day, hour, minute, second) = utc(until_unix_s);
    format!("{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}Z")
}

/// systemd's calendar spec of an instant, in UTC: `yyyy-mm-dd HH:MM:SS UTC`
/// — a timer on the wall clock, which a suspend does not hold back the way
/// it holds a monotonic one.
#[must_use]
pub fn calendar(at_unix_s: i64) -> String {
    let (year, month, day, hour, minute, second) = utc(at_unix_s);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

/// The window's one line for `user`, closing at `until_unix_s` — or never,
/// for `None` (the file's content is this line and a line break): the wheel
/// rule's `SETENV`, and no password.
///
/// # Errors
///
/// [`Why::Invalid`] for a user name that is not plain, or an instant outside
/// the years 1970–9999.
pub fn sudoers_line(user: &str, until_unix_s: Option<i64>) -> Result<String, Refusal> {
    if !plain(user) || until_unix_s.is_some_and(|u| !(0..=LAST_INSTANT_S).contains(&u)) {
        return Err(refused(Why::Invalid));
    }
    Ok(match until_unix_s {
        Some(until) => format!(
            "{user} ALL=(ALL:ALL) NOTAFTER={} NOPASSWD:SETENV: ALL",
            notafter(until)
        ),
        None => format!("{user} ALL=(ALL:ALL) NOPASSWD:SETENV: ALL"),
    })
}

/// The expiry timer of the window that closes at `until_unix_s`.
#[must_use]
pub fn expiry_unit(until_unix_s: i64) -> String {
    format!("{EXPIRY_UNIT_PREFIX}-{until_unix_s}")
}

/// `systemd-run`'s arguments for the expiry timer: at `until_unix_s` by the
/// wall clock, to the second, `helper relax-expire <until>` as root. Pure.
#[must_use]
pub fn arm_arguments(until_unix_s: i64, helper: &Path) -> Vec<String> {
    vec![
        format!("--unit={}", expiry_unit(until_unix_s)),
        format!("--on-calendar={}", calendar(until_unix_s)),
        "--timer-property=AccuracySec=1s".to_owned(),
        "--description=Ends the passwordless sudo window".to_owned(),
        "--collect".to_owned(),
        "--quiet".to_owned(),
        "--".to_owned(),
        helper.display().to_string(),
        "relax-expire".to_owned(),
        until_unix_s.to_string(),
    ]
}

/// `systemctl`'s arguments that stop every window's expiry timer. Pure.
#[must_use]
pub fn disarm_arguments() -> Vec<String> {
    vec!["stop".to_owned(), format!("{EXPIRY_UNIT_PREFIX}-*.timer")]
}

/// An open window, as [`STATE`] says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Opened {
    /// Always [`STATE_FORMAT`].
    pub format: u32,
    /// When it closes, in seconds since the epoch; `None` for no limit.
    pub until_unix_s: Option<i64>,
    /// The span it was opened for — what "start the count again" opens.
    pub span: RelaxSpan,
}

impl Opened {
    /// The window of `span`, opened at `now_unix_s`. Pure.
    #[must_use]
    pub fn of(span: RelaxSpan, now_unix_s: i64) -> Self {
        Self {
            format: STATE_FORMAT,
            until_unix_s: span
                .minutes()
                .map(|minutes| now_unix_s.saturating_add(i64::from(minutes) * 60)),
            span,
        }
    }

    /// What a state file says; `None` for anything that is not one of this
    /// format, whose end does not agree with its span, or that ends past
    /// what a sudoers date can say. Pure.
    #[must_use]
    pub fn of_bytes(bytes: &[u8]) -> Option<Self> {
        let opened: Self = serde_json::from_slice(bytes).ok()?;
        let agrees = opened.span.minutes().is_some() == opened.until_unix_s.is_some();
        let in_range = opened
            .until_unix_s
            .is_none_or(|u| (0..=LAST_INSTANT_S).contains(&u));
        (opened.format == STATE_FORMAT && agrees && in_range).then_some(opened)
    }

    /// The file's bytes. Pure.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(self).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    /// Whether it is still open at `now_unix_s`.
    #[must_use]
    pub fn is_open(&self, now_unix_s: i64) -> bool {
        self.until_unix_s.is_none_or(|until| until > now_unix_s)
    }

    /// The seconds left at `now_unix_s` — none below zero — for a live
    /// countdown; `None` for no limit.
    #[must_use]
    pub fn left_s(&self, now_unix_s: i64) -> Option<i64> {
        self.until_unix_s
            .map(|until| until.saturating_sub(now_unix_s).max(0))
    }

    /// The verb that starts the count again — the window opened anew for
    /// the same span; `None` for no limit, which has no count. Through the
    /// door with nothing typed: `sudo -n` lets it in while the window is
    /// open.
    #[must_use]
    pub const fn again(&self) -> Option<RootVerb> {
        match self.span {
            RelaxSpan::NoLimit => None,
            span => Some(RootVerb::Relax { span }),
        }
    }
}

/// Where the window lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// The folder sudo includes.
    pub dir: PathBuf,
    /// What is open.
    pub state: PathBuf,
}

impl Window {
    /// The machine's.
    #[must_use]
    pub fn system() -> Self {
        Self {
            dir: PathBuf::from(DIR),
            state: PathBuf::from(STATE),
        }
    }

    /// Opens `opened` for `user`: the draft, `visudo`, the rename, the state.
    /// The timer is armed by the caller, after.
    ///
    /// A window already open stays as it was when the new line never goes
    /// live (a refused draft); once the new line is live, a failure closes
    /// the window whole — a state that says another hour is worse than no
    /// window.
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`] (see [`sudoers_line`]), [`Why::Failed`] when `visudo`
    /// refuses the draft or a file cannot be written (no draft is left
    /// behind), [`Why::Unstartable`] without `visudo`.
    pub fn open(&self, user: &str, opened: &Opened, visudo: &Path) -> Result<(), Refusal> {
        let line = sudoers_line(user, opened.until_unix_s)?;
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
        crate::write_whole(&self.state, &opened.to_bytes(), 0o644).map_err(|e| {
            let _ = self.close();
            failed(e)
        })
    }

    /// Closes the window: the line, a draft left over, the state. Closing a
    /// closed window is not an error.
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
        let state = gone(&self.state);
        line.and(draft).and(state)
    }

    /// What is open, as the state says — `None` when nothing is (or the
    /// state says something that is not a window). It may have ended
    /// already: ask [`Opened::is_open`].
    #[must_use]
    pub fn opened(&self) -> Option<Opened> {
        Opened::of_bytes(&std::fs::read(&self.state).ok()?)
    }

    /// Whether the line is in the folder sudo reads (a link included: it is
    /// root's folder, and anything there is what sudo would read).
    fn has_line(&self) -> bool {
        std::fs::symlink_metadata(self.dir.join(FILE)).is_ok()
    }
}

/// What is open on this machine, for the doors, the modeline and the
/// brain; `None` when nothing is. Read as the person (`0644`). It may have
/// ended a moment ago, before its timer ran: ask [`Opened::is_open`].
#[must_use]
pub fn opened() -> Option<Opened> {
    Window::system().opened()
}

/// Opens the window for `span`: writes the line and the state, stops every
/// old expiry timer, and — for a timed window — arms the new one to run
/// `helper relax-expire <until>`. The old timers stop only once the new line
/// is live — a reopening that fails leaves the open window with its own
/// timer. If the new timer cannot be armed the window is closed again: the
/// person was promised an end.
///
/// # Errors
///
/// What [`Window::open`] refuses; [`Why::Unstartable`] or [`Why::Failed`]
/// for the timer.
pub fn open(
    window: &Window,
    tools: &Tools,
    user: &str,
    span: RelaxSpan,
    helper: &Path,
) -> Result<Opened, Refusal> {
    let visudo = tools::given(tools.visudo.as_ref())?;
    let opened = Opened::of(span, crate::now_unix_s());
    // ASKED BEFORE THE LINE GOES LIVE: a timed window this machine could not
    // end is never opened.
    let systemd_run = match opened.until_unix_s {
        Some(_) => Some(tools::given(tools.systemd_run.as_ref())?),
        None => None,
    };
    window.open(user, &opened, visudo)?;
    disarm(tools);
    if let (Some(until), Some(systemd_run)) = (opened.until_unix_s, systemd_run)
        && let Err(refusal) = arm(systemd_run, until, helper)
    {
        let _ = window.close();
        return Err(refusal);
    }
    Ok(opened)
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

/// The timer armed for `until_unix_s` fired: the window closes when it is
/// still the one that ends then, or when no state says what is open while a
/// line is there (an orphaned line is a window nobody can see). A window
/// opened since — another end, or no limit — is left as it is.
///
/// # Errors
///
/// What [`Window::close`] refuses.
pub fn expire(window: &Window, until_unix_s: i64) -> Result<(), Refusal> {
    match window.opened() {
        Some(opened) if opened.until_unix_s == Some(until_unix_s) => window.close(),
        Some(_) => Ok(()),
        None if window.has_line() => window.close(),
        None => Ok(()),
    }
}

/// What the boot does with the window, at `now_unix_s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtBoot {
    /// Nothing was open.
    Nothing,
    /// It had ended, or its line or its state was missing: closed.
    Closed,
    /// It is still open, until then: its timer armed again.
    Rearmed(i64),
    /// It has no limit: left open.
    Kept,
}

/// The boot's look at the window (`ronoctua-sudo-window`): an expired one,
/// a line with no state and a state with no line are closed; a live timed
/// one gets its timer back — a transient timer does not survive a reboot —
/// and one with no limit stays as it is.
///
/// # Errors
///
/// What [`Window::close`] refuses; [`Why::Unstartable`] or [`Why::Failed`]
/// when the timer cannot be armed again (the line's own `NOTAFTER` still
/// ends it in sudo).
pub fn boot(
    window: &Window,
    tools: &Tools,
    helper: &Path,
    now_unix_s: i64,
) -> Result<AtBoot, Refusal> {
    let opened = window.opened();
    let line = window.has_line();
    match opened {
        None if !line && std::fs::symlink_metadata(&window.state).is_err() => Ok(AtBoot::Nothing),
        Some(opened) if line && opened.is_open(now_unix_s) => match opened.until_unix_s {
            None => Ok(AtBoot::Kept),
            Some(until) => {
                disarm(tools);
                let systemd_run = tools::given(tools.systemd_run.as_ref())?;
                arm(systemd_run, until, helper)?;
                Ok(AtBoot::Rearmed(until))
            }
        },
        _ => window.close().map(|()| AtBoot::Closed),
    }
}

/// Arms the timer that ends the window at `until_unix_s`.
fn arm(systemd_run: &Path, until_unix_s: i64, helper: &Path) -> Result<(), Refusal> {
    let arguments = arm_arguments(until_unix_s, helper);
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    tools::run(systemd_run, &arguments, None).map(|_| ())
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
    fn the_line_closes_itself_at_the_end_or_never() {
        assert_eq!(
            sudoers_line("someone", Some(1_790_000_000)).unwrap(),
            "someone ALL=(ALL:ALL) NOTAFTER=20260921141320Z NOPASSWD:SETENV: ALL"
        );
        assert_eq!(
            sudoers_line("someone", None).unwrap(),
            "someone ALL=(ALL:ALL) NOPASSWD:SETENV: ALL",
            "no limit is no NOTAFTER"
        );
        assert_eq!(notafter(0), "19700101000000Z");
        assert_eq!(notafter(951_782_400), "20000229000000Z", "a leap day");
        assert_eq!(notafter(4_102_444_799), "20991231235959Z");
        assert_eq!(notafter(253_402_300_799), "99991231235959Z");
        assert_eq!(calendar(1_790_000_000), "2026-09-21 14:13:20 UTC");
        assert_eq!(calendar(951_782_400), "2000-02-29 00:00:00 UTC");
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
            for until in [Some(0), None] {
                assert_eq!(
                    sudoers_line(bad, until).unwrap_err().why,
                    Why::Invalid,
                    "{bad:?}"
                );
            }
        }
        for bad_time in [-1, 253_402_300_800, i64::MAX] {
            assert_eq!(
                sudoers_line("someone", Some(bad_time)).unwrap_err().why,
                Why::Invalid
            );
        }
    }

    #[test]
    fn the_timer_runs_this_helper_at_the_end_by_the_wall_clock() {
        assert_eq!(
            arm_arguments(
                1_790_000_000,
                Path::new("/nix/store/x/bin/ronoctua-security-root")
            ),
            [
                "--unit=ronoctua-sudo-expire-1790000000",
                "--on-calendar=2026-09-21 14:13:20 UTC",
                "--timer-property=AccuracySec=1s",
                "--description=Ends the passwordless sudo window",
                "--collect",
                "--quiet",
                "--",
                "/nix/store/x/bin/ronoctua-security-root",
                "relax-expire",
                "1790000000",
            ]
        );
        assert_eq!(disarm_arguments(), ["stop", "ronoctua-sudo-expire-*.timer"]);
    }

    /// FAILS IF: a span opens a window of another length, the longest spans
    /// are not there, no limit gets an end, the state does not read back, a
    /// state whose end and span disagree is taken, the countdown goes below
    /// zero, or "start again" opens anything but the same span.
    #[test]
    fn a_window_says_its_end_its_count_and_how_to_start_it_again() {
        let now = 1_790_000_000;
        let ends: Vec<Option<i64>> = RelaxSpan::ALL
            .iter()
            .map(|&span| Opened::of(span, now).until_unix_s)
            .collect();
        assert_eq!(
            ends,
            [
                Some(now + 300),
                Some(now + 900),
                Some(now + 1800),
                Some(now + 3600),
                Some(now + 86_400),
                Some(now + 172_800),
                None
            ]
        );
        for span in RelaxSpan::ALL {
            let opened = Opened::of(span, now);
            assert_eq!(Opened::of_bytes(&opened.to_bytes()), Some(opened));
            assert!(opened.is_open(now));
            match span {
                RelaxSpan::NoLimit => {
                    assert_eq!(opened.again(), None);
                    assert_eq!(opened.left_s(now), None);
                    assert!(opened.is_open(i64::MAX), "no limit never ends by itself");
                }
                _ => {
                    assert_eq!(opened.again(), Some(RootVerb::Relax { span }));
                    let until = opened.until_unix_s.unwrap();
                    assert_eq!(opened.left_s(now), Some(until - now));
                    assert_eq!(opened.left_s(until + 99), Some(0));
                    assert!(!opened.is_open(until));
                }
            }
        }
        for wrong in [
            &br#"{"format":1,"until-unix-s":null,"span":"fifteen"}"#[..],
            br#"{"format":1,"until-unix-s":1790000000,"span":"no-limit"}"#,
            br#"{"format":2,"until-unix-s":1790000000,"span":"fifteen"}"#,
            br#"{"format":1,"until-unix-s":253402300800,"span":"day"}"#,
            br#"{"format":1,"until-unix-s":1790000000,"span":"fifteen","more":1}"#,
            br#"{"format":1,"until-unix-s":1790000000,"span":"seven"}"#,
            b"1790000000\n",
            b"",
        ] {
            assert_eq!(
                Opened::of_bytes(wrong),
                None,
                "{}",
                String::from_utf8_lossy(wrong)
            );
        }
    }

    /// A `visudo` that passes what the real one passes for these lines and
    /// refuses a draft with anything else in it.
    fn fake_visudo(dir: &Scratch) -> PathBuf {
        dir.program(
            "visudo",
            "#!/bin/sh\n\
             [ \"$1 $2\" = \"-c -f\" ] || exit 2\n\
             IFS= read -r line < \"$3\"\n\
             case \"$line\" in\n\
             *' ALL=(ALL:ALL) NOTAFTER='*'Z NOPASSWD:SETENV: ALL') exit 0 ;;\n\
             *' ALL=(ALL:ALL) NOPASSWD:SETENV: ALL') exit 0 ;;\n\
             esac\n\
             exit 1\n",
        )
    }

    fn window(dir: &Scratch) -> Window {
        let window = Window {
            dir: dir.0.join("sudo"),
            state: dir.0.join("sudo-window"),
        };
        std::fs::create_dir(&window.dir).unwrap();
        window
    }

    /// Tools that record what was armed and stopped, a line each.
    fn recording(dir: &Scratch) -> (Tools, PathBuf) {
        let said = dir.0.join("said");
        let record = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\n", said.display());
        let tools = Tools {
            visudo: Some(fake_visudo(dir)),
            systemd_run: Some(dir.program("systemd-run", &record)),
            systemctl: Some(dir.program("systemctl", &record)),
            nft: None,
        };
        (tools, said)
    }

    #[test]
    fn a_window_opens_whole_and_closes_whole() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = Scratch::new("relax");
        let visudo = fake_visudo(&dir);
        let window = window(&dir);
        assert_eq!(window.opened(), None);
        let opened = Opened {
            format: STATE_FORMAT,
            until_unix_s: Some(1_790_000_000),
            span: RelaxSpan::Fifteen,
        };
        window.open("someone", &opened, &visudo).unwrap();
        let file = window.dir.join(FILE);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "someone ALL=(ALL:ALL) NOTAFTER=20260921141320Z NOPASSWD:SETENV: ALL\n"
        );
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o440);
        assert_eq!(mode(&window.state), 0o644, "the person reads what is open");
        assert!(!window.dir.join(DRAFT).exists(), "the draft was renamed");
        assert_eq!(window.opened(), Some(opened));
        let names: Vec<_> = std::fs::read_dir(&window.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [FILE], "nothing else in the folder sudo reads");
        window.close().unwrap();
        assert!(!file.exists());
        assert_eq!(window.opened(), None);
        window.close().unwrap();
    }

    #[test]
    fn a_line_visudo_refuses_never_goes_live() {
        let dir = Scratch::new("relax-refused");
        let refusing = dir.program("visudo", "#!/bin/sh\nexit 1\n");
        let window = window(&dir);
        assert_eq!(
            window
                .open(
                    "someone",
                    &Opened::of(RelaxSpan::NoLimit, 1_790_000_000),
                    &refusing
                )
                .unwrap_err()
                .why,
            Why::Failed
        );
        let left: Vec<_> = std::fs::read_dir(&window.dir).unwrap().collect();
        assert!(left.is_empty(), "nothing is left in the included folder");
        assert!(!window.state.exists());
    }

    #[test]
    fn a_refused_reopening_leaves_the_open_window_and_its_timer() {
        let dir = Scratch::new("relax-reopen");
        let window = window(&dir);
        let (tools, said) = recording(&dir);
        let opened = open(
            &window,
            &tools,
            "someone",
            RelaxSpan::Five,
            Path::new("/helper"),
        )
        .unwrap();
        let line = std::fs::read_to_string(window.dir.join(FILE)).unwrap();
        std::fs::remove_file(&said).unwrap();

        // visudo refuses the new line: the open window, its state and its
        // timer are all as they were.
        let refusing = Tools {
            visudo: Some(dir.program("visudo-no", "#!/bin/sh\nexit 1\n")),
            ..tools.clone()
        };
        assert_eq!(
            open(
                &window,
                &refusing,
                "someone",
                RelaxSpan::Day,
                Path::new("/helper")
            )
            .unwrap_err()
            .why,
            Why::Failed
        );
        assert_eq!(
            std::fs::read_to_string(window.dir.join(FILE)).unwrap(),
            line
        );
        assert_eq!(window.opened(), Some(opened));
        assert!(!said.exists(), "no timer was stopped");

        // Accepted, the old timers stop — after the new line is live — and
        // the new one is armed for the new end.
        let day = open(
            &window,
            &tools,
            "someone",
            RelaxSpan::Day,
            Path::new("/helper"),
        )
        .unwrap();
        let until = day.until_unix_s.unwrap();
        assert_eq!(
            std::fs::read_to_string(&said).unwrap(),
            format!(
                "stop ronoctua-sudo-expire-*.timer\n{}\n",
                arm_arguments(until, Path::new("/helper")).join(" ")
            )
        );
        std::fs::remove_file(&said).unwrap();

        // NO LIMIT: the old timer stops, and none is armed.
        let forever = open(
            &window,
            &tools,
            "someone",
            RelaxSpan::NoLimit,
            Path::new("/helper"),
        )
        .unwrap();
        assert_eq!(forever.until_unix_s, None);
        assert_eq!(
            std::fs::read_to_string(&said).unwrap(),
            "stop ronoctua-sudo-expire-*.timer\n"
        );
        assert_eq!(
            std::fs::read_to_string(window.dir.join(FILE)).unwrap(),
            "someone ALL=(ALL:ALL) NOPASSWD:SETENV: ALL\n"
        );
    }

    #[test]
    fn a_window_whose_timer_cannot_be_armed_is_closed_again() {
        let dir = Scratch::new("relax-timer");
        let window = window(&dir);
        let tools = Tools {
            visudo: Some(fake_visudo(&dir)),
            systemd_run: Some(dir.program("systemd-run", "#!/bin/sh\nexit 1\n")),
            systemctl: Some(dir.program("systemctl", "#!/bin/sh\nexit 0\n")),
            nft: None,
        };
        assert_eq!(
            open(
                &window,
                &tools,
                "someone",
                RelaxSpan::Five,
                Path::new("/helper")
            )
            .unwrap_err()
            .why,
            Why::Failed
        );
        assert!(!window.dir.join(FILE).exists());
        assert!(!window.state.exists());
        // NO `systemd-run` AT ALL: a timed window is never opened, and no
        // line went live on the way.
        let without = Tools {
            systemd_run: None,
            ..tools
        };
        assert_eq!(
            open(
                &window,
                &without,
                "someone",
                RelaxSpan::Sixty,
                Path::new("/helper")
            )
            .unwrap_err()
            .why,
            Why::Unstartable
        );
        assert!(!window.dir.join(FILE).exists());
        // …and one with no limit needs no timer.
        open(
            &window,
            &without,
            "someone",
            RelaxSpan::NoLimit,
            Path::new("/helper"),
        )
        .unwrap();
        assert!(window.dir.join(FILE).exists());
        close(&window, &without).unwrap();
        assert!(!window.dir.join(FILE).exists());
    }

    /// FAILS IF: a timer closes a window opened after it was armed (another
    /// end, or no limit), leaves the window it was armed for open, or leaves
    /// a line nobody can see.
    #[test]
    fn a_timer_closes_its_own_window_and_no_other() {
        let dir = Scratch::new("relax-expire");
        let window = window(&dir);
        let (tools, _) = recording(&dir);
        let first = open(&window, &tools, "someone", RelaxSpan::Five, Path::new("/h")).unwrap();
        let first_end = first.until_unix_s.unwrap();
        let later = Opened {
            until_unix_s: Some(first_end + 600),
            ..first
        };
        window
            .open("someone", &later, tools.visudo.as_ref().unwrap())
            .unwrap();
        expire(&window, first_end).unwrap();
        assert_eq!(window.opened(), Some(later), "a newer window was closed");
        window
            .open(
                "someone",
                &Opened::of(RelaxSpan::NoLimit, 0),
                tools.visudo.as_ref().unwrap(),
            )
            .unwrap();
        expire(&window, first_end + 600).unwrap();
        assert!(window.opened().is_some(), "no limit was closed by a timer");
        window
            .open("someone", &later, tools.visudo.as_ref().unwrap())
            .unwrap();
        expire(&window, first_end + 600).unwrap();
        assert_eq!(window.opened(), None);
        assert!(!window.dir.join(FILE).exists());
        // AN ORPHANED LINE: no state says it is open, so nobody sees it.
        std::fs::write(
            window.dir.join(FILE),
            "someone ALL=(ALL:ALL) NOPASSWD:SETENV: ALL\n",
        )
        .unwrap();
        expire(&window, 1).unwrap();
        assert!(!window.dir.join(FILE).exists());
        expire(&window, 1).unwrap();
    }

    /// FAILS IF: the boot leaves an expired window, a line with no state or
    /// a state with no line; closes a live one; does not arm a live timed
    /// one again (its transient timer died with the machine); or arms one
    /// for a window with no limit.
    #[test]
    fn the_boot_ends_what_expired_and_rearms_what_lives() {
        let dir = Scratch::new("relax-boot");
        let window = window(&dir);
        let (tools, said) = recording(&dir);
        let visudo = tools.visudo.clone().unwrap();
        let helper = Path::new("/h");
        let now = 1_790_000_000;
        assert_eq!(boot(&window, &tools, helper, now), Ok(AtBoot::Nothing));

        let live = Opened {
            format: STATE_FORMAT,
            until_unix_s: Some(now + 60),
            span: RelaxSpan::Five,
        };
        window.open("someone", &live, &visudo).unwrap();
        assert_eq!(
            boot(&window, &tools, helper, now),
            Ok(AtBoot::Rearmed(now + 60))
        );
        assert_eq!(
            std::fs::read_to_string(&said).unwrap(),
            format!(
                "stop ronoctua-sudo-expire-*.timer\n{}\n",
                arm_arguments(now + 60, helper).join(" ")
            )
        );
        assert_eq!(window.opened(), Some(live));
        std::fs::remove_file(&said).unwrap();

        assert_eq!(boot(&window, &tools, helper, now + 60), Ok(AtBoot::Closed));
        assert!(!window.dir.join(FILE).exists() && !window.state.exists());

        window
            .open("someone", &Opened::of(RelaxSpan::NoLimit, now), &visudo)
            .unwrap();
        assert_eq!(boot(&window, &tools, helper, i64::MAX), Ok(AtBoot::Kept));
        assert!(!said.exists(), "a timer for a window with no limit");
        assert!(window.dir.join(FILE).exists());

        // A LINE WITH NO STATE, and a state with no line (or one that says
        // nothing this reads): closed whole.
        std::fs::remove_file(&window.state).unwrap();
        assert_eq!(boot(&window, &tools, helper, now), Ok(AtBoot::Closed));
        assert!(!window.dir.join(FILE).exists());
        std::fs::write(&window.state, live.to_bytes()).unwrap();
        assert_eq!(boot(&window, &tools, helper, now), Ok(AtBoot::Closed));
        assert!(!window.state.exists());
        std::fs::write(&window.state, b"garbage").unwrap();
        assert_eq!(boot(&window, &tools, helper, now), Ok(AtBoot::Closed));
        assert!(!window.state.exists());
    }
}
