//! The sudo door — the ONE code in the tree that spawns `sudo`
//! (feature `authority`, turned on by the shell alone).
//!
//! # How
//!
//! First `sudo -n -- <helper> <verb>`: it succeeds while the passwordless
//! window is open or sudo's own timestamp is fresh, and asks nothing. Only
//! when it fails for want of a password does a [`Proof::Typed`] password go
//! in: `sudo -S -k -p "" -- <helper> <verb>` with `LC_ALL=C`, the password
//! and a line break written to stdin, stdin closed. Never in argv, never in
//! the environment. A wrong password makes sudo read EOF and fail —
//! [`crate::Why::WrongPassword`].
//!
//! Both run in a session of their own (`setsid`), with no controlling
//! terminal: sudo then gives PAM no tty, and the fingerprint gate — which
//! answers only in terminals — never makes this door wait for a finger. The
//! environment is emptied but for `LC_ALL=C`, so what sudo says is the
//! English this module reads.
//!
//! # Telling sudo from the helper
//!
//! The helper's last line is its verdict ([`crate::verb::Said`]). A run with
//! no verdict line ended in sudo, before the helper: that is a password
//! wanted, refused, or not allowed at all, read from what sudo said.
//!
//! # Where the details go
//!
//! A refusal on screen is a sentence ([`crate::Why`]); what the helper said
//! of it in detail (`visudo`'s words, `nft`'s, fprintd's) is written to this
//! process's standard error, one line each with the journal's priority
//! prefix and the helper's name ([`journal_lines`]). The footer is a user
//! unit and the windows inherit the compositor's descriptors, so it lands in
//! the journal — which is what `security-refusal-failed` points at.
//!
//! # Who may call it
//!
//! Only a person's gesture: the shell's views, after a secret question. The
//! brain and the daemons never link this feature (`sudo-door-is-one`), and
//! nothing automatic ever reaches a password.
//!
//! # Where
//!
//! sudo is the setuid wrapper [`SUDO`] (a store path cannot be setuid); the
//! helper is the extracted `securityRoot`, whose path the session carries in
//! [`HELPER_VARIABLE`] (`nix/modules/security.nix`).

use std::io::{BufRead as _, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ronoctua_consent::keys::Secret;

pub use crate::verb::RootVerb;
use crate::verb::Said;
use crate::{Refusal, Why, refused};

/// sudo: NixOS's setuid wrapper.
pub const SUDO: &str = "/run/wrappers/bin/sudo";
/// The variable naming the root helper's store path.
pub const HELPER_VARIABLE: &str = "RONOCTUA_SECURITY_ROOT";
/// The most the door keeps of what the helper and sudo print.
const SAID_MOST_BYTES: usize = 64 * 1024;

/// What proves the person may do it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    /// The password the person just typed.
    Typed(Secret),
    /// Nothing typed: only an open window or sudo's fresh timestamp
    /// (`sudo -n`), e.g. "end the window now".
    Cached,
}

/// What the helper printed (its report, for the view) — data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Output {
    /// The helper's standard output.
    pub stdout: String,
}

/// The programs a door runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Door {
    sudo: PathBuf,
    setsid: Option<PathBuf>,
    helper: PathBuf,
}

impl Door {
    /// This session's: [`SUDO`], `setsid` by store path, and the helper the
    /// session was given.
    fn here() -> Result<Self, Refusal> {
        let helper = std::env::var_os(HELPER_VARIABLE)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(refused(Why::NoHelper))?;
        Ok(Self {
            sudo: PathBuf::from(SUDO),
            setsid: option_env!("RONOCTUA_SETSID").map(PathBuf::from),
            helper,
        })
    }

    /// The command for one attempt: the program first (`setsid --wait` when
    /// there is one, then sudo), sudo's own flags, `--`, the helper and the
    /// verb.
    fn command(&self, flags: &[&str], verb: &RootVerb) -> Command {
        let mut command = match &self.setsid {
            Some(setsid) => {
                let mut c = Command::new(setsid);
                c.arg("--wait").arg(&self.sudo);
                c
            }
            None => Command::new(&self.sudo),
        };
        command
            .args(flags)
            .arg("--")
            .arg(&self.helper)
            .args(verb.arguments())
            .env_clear()
            .env("LC_ALL", "C")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

/// How one attempt ended.
#[derive(Debug, PartialEq, Eq)]
enum Ended {
    /// The helper answered; beside it, what it said on standard error.
    Helper(Result<Output, Refusal>, String),
    /// sudo stopped it before the helper ran, and said this.
    Sudo(String),
}

/// The name the helper's words carry in the journal.
const HELPER_NAME: &str = "ronoctua-security-root";

/// What `said` (the helper's standard error, or sudo's) becomes in the
/// journal: a line each, with its own priority prefix kept (`<3>` when it
/// carried none) and the helper's name after it. Pure.
fn journal_lines(said: &str) -> Vec<String> {
    said.lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let bytes = line.as_bytes();
            let prefixed = bytes.len() >= 3
                && bytes[0] == b'<'
                && bytes[1].is_ascii_digit()
                && bytes[2] == b'>';
            let (priority, words) = if prefixed {
                line.split_at(3)
            } else {
                ("<3>", line)
            };
            format!("{priority}{HELPER_NAME}: {words}")
        })
        .collect()
}

/// Hands `said` to the journal (see the module's "Where the details go").
fn to_the_journal(said: &str) {
    for line in journal_lines(said) {
        eprintln!("{line}");
    }
}

/// The helper's answer; when it is a refusal, what the helper said of it
/// goes to the journal first.
fn told(answer: Result<Output, Refusal>, said: &str) -> Result<Output, Refusal> {
    if answer.is_err() {
        to_the_journal(said);
    }
    answer
}

/// One attempt: `flags` for sudo, `password` on stdin when there is one,
/// every `step` handed to `on_step` as it comes.
fn attempt(
    door: &Door,
    flags: &[&str],
    verb: &RootVerb,
    password: Option<&Secret>,
    on_step: &mut dyn FnMut(&str),
) -> Result<Ended, Refusal> {
    let mut command = door.command(flags, verb);
    command.stdin(if password.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = crate::tools::spawn(&mut command).map_err(|_| refused(Why::Unstartable))?;
    if let (Some(password), Some(mut stdin)) = (password, child.stdin.take()) {
        // The password and its line break, then EOF: a wrong one makes sudo
        // read nothing more and give up. A sudo that wants no password (the
        // window opened in between) never reads it, and the broken pipe is
        // not an error — the helper never reads its stdin.
        let mut line = Vec::with_capacity(password.expose().len() + 1);
        line.extend_from_slice(password.expose().as_bytes());
        line.push(b'\n');
        let _ = stdin.write_all(&line);
        line.fill(0);
    }
    let stderr = child.stderr.take();
    let stderr = std::thread::spawn(move || {
        let mut said = Vec::new();
        if let Some(stderr) = stderr {
            let _ = stderr.take(SAID_MOST_BYTES as u64).read_to_end(&mut said);
        }
        String::from_utf8_lossy(&said).into_owned()
    });
    let mut stdout = String::new();
    let mut verdict = None;
    if let Some(out) = child.stdout.take() {
        for line in std::io::BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            match Said::parse(&line) {
                Some(Said::Step(word)) => on_step(&word),
                Some(said) => verdict = Some(said),
                None => {}
            }
            if stdout.len() + line.len() < SAID_MOST_BYTES {
                stdout.push_str(&line);
                stdout.push('\n');
            }
        }
    }
    let status = child.wait().map_err(|_| refused(Why::Failed))?;
    let said = stderr.join().unwrap_or_default();
    Ok(match verdict {
        Some(Said::Done) if status.success() => Ended::Helper(Ok(Output { stdout }), said),
        Some(Said::Refused(why)) => Ended::Helper(Err(refused(why)), said),
        Some(_) => Ended::Helper(Err(refused(Why::Failed)), said),
        None => Ended::Sudo(said),
    })
}

/// What sudo's words (with `LC_ALL=C`) say went wrong.
fn sudo_said(said: &str) -> Why {
    if said.contains("a password is required") {
        Why::NeedsPassword
    } else if said.contains("incorrect password")
        || said.contains("Sorry, try again")
        || said.contains("no password was provided")
    {
        Why::WrongPassword
    } else if said.contains("not in the sudoers file")
        || said.contains("is not allowed to")
        || said.contains("may not run sudo")
    {
        Why::NotAllowed
    } else {
        Why::Failed
    }
}

fn through(
    door: &Door,
    verb: RootVerb,
    proof: &Proof,
    on_step: &mut dyn FnMut(&str),
) -> Result<Output, Refusal> {
    let why = match attempt(door, &["-n"], &verb, None, on_step)? {
        Ended::Helper(answer, said) => return told(answer, &said),
        Ended::Sudo(said) => sudo_said(&said),
    };
    let password = match (why, proof) {
        (Why::NeedsPassword, Proof::Typed(password)) => password,
        (why, _) => return Err(refused(why)),
    };
    match attempt(
        door,
        &["-S", "-k", "-p", ""],
        &verb,
        Some(password),
        on_step,
    )? {
        Ended::Helper(answer, said) => told(answer, &said),
        Ended::Sudo(said) => Err(refused(match sudo_said(&said) {
            Why::NeedsPassword => Why::WrongPassword,
            // Words this module does not know: they are the only account of
            // what happened, so the journal gets them.
            Why::Failed => {
                to_the_journal(&said);
                Why::Failed
            }
            why => why,
        })),
    }
}

/// Runs `verb` as root through sudo. Blocks (sudo sleeps on a wrong
/// password, an enrolment waits for a finger): call it on a thread.
///
/// # Errors
///
/// [`Why::NoHelper`], [`Why::NeedsPassword`] (a `Cached` proof and no open
/// window), [`Why::WrongPassword`], [`Why::NotAllowed`], [`Why::Unstartable`],
/// [`Why::Failed`] — and whatever the helper refused.
pub fn run(verb: RootVerb, proof: Proof) -> Result<Output, Refusal> {
    run_with(verb, &proof, &mut |_| {})
}

/// [`run`], handing every progress word the helper says (an enrolment's
/// `enroll-stage-passed`, …) to `on_step` as it comes.
///
/// # Errors
///
/// As [`run`].
pub fn run_with(
    verb: RootVerb,
    proof: &Proof,
    on_step: &mut dyn FnMut(&str),
) -> Result<Output, Refusal> {
    through(&Door::here()?, verb, proof, on_step)
}

/// Whether the helper exists where the session says, for a door that should
/// say so before asking for a password.
#[must_use]
pub fn has_helper() -> bool {
    Door::here().is_ok_and(|door| Path::new(&door.helper).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use crate::verb::RelaxMinutes;

    const PASSWORD: &str = "correct horse";

    /// A sudo that records what it was given and behaves like sudo: `-n`
    /// wants a password unless `open` exists (the window), `-S` takes one
    /// line and runs the helper only when it is the right one.
    fn fake_sudo(dir: &Scratch) -> Door {
        let root = dir.0.display();
        let helper = dir.program(
            "helper",
            "#!/bin/sh\n\
             case \"$1\" in\n\
             finger-enroll) printf 'step enroll-stage-passed\\nstep enroll-completed\\ndone\\n' ;;\n\
             firewall-apply) printf 'refused invalid\\n'; printf '<3>the wish is not valid\\nno prefix here\\n' >&2; exit 1 ;;\n\
             *) printf 'done\\n' ;;\n\
             esac\n",
        );
        let sudo = dir.program(
            "sudo",
            &format!(
                "#!/bin/sh\n\
                 : > {root}/argv\n\
                 for a in \"$@\"; do printf '%s\\n' \"$a\" >> {root}/argv; done\n\
                 export -p > {root}/env\n\
                 case \"$1\" in\n\
                 -n)\n\
                   if [ -e {root}/open ]; then shift 2; exec \"$@\"; fi\n\
                   printf 'sudo: a password is required\\n' >&2; exit 1 ;;\n\
                 -S)\n\
                   IFS= read -r password || password=\n\
                   printf '%s\\n' \"$password\" > {root}/stdin\n\
                   if IFS= read -r more; then printf 'more\\n' >> {root}/stdin; fi\n\
                   if [ \"$password\" = '{PASSWORD}' ]; then shift 5; exec \"$@\"; fi\n\
                   printf 'sudo: 1 incorrect password attempt\\n' >&2; exit 1 ;;\n\
                 esac\n\
                 exit 1\n"
            ),
        );
        Door {
            sudo,
            setsid: None,
            helper,
        }
    }

    fn lines(path: PathBuf) -> Vec<String> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn the_password_goes_on_stdin_and_never_in_argv() {
        let dir = Scratch::new("door");
        let door = fake_sudo(&dir);
        let verb = RootVerb::Relax {
            minutes: RelaxMinutes::Fifteen,
        };
        let proof = Proof::Typed(Secret::new(PASSWORD));
        through(&door, verb, &proof, &mut |_| {}).unwrap();
        let helper = door.helper.display().to_string();
        assert_eq!(
            lines(dir.0.join("argv")),
            ["-S", "-k", "-p", "", "--", helper.as_str(), "relax", "15"]
        );
        assert_eq!(lines(dir.0.join("stdin")), [PASSWORD], "one line, then EOF");
        let env = std::fs::read_to_string(dir.0.join("env")).unwrap();
        assert!(env.contains("LC_ALL"), "{env}");
        assert!(!env.contains("HOME="), "nothing else of ours: {env}");
        assert!(!env.contains(PASSWORD), "not in the environment either");
    }

    #[test]
    fn an_open_window_asks_nothing() {
        let dir = Scratch::new("door-open");
        let door = fake_sudo(&dir);
        std::fs::write(dir.0.join("open"), b"").unwrap();
        through(&door, RootVerb::RelaxEnd, &Proof::Cached, &mut |_| {}).unwrap();
        let helper = door.helper.display().to_string();
        assert_eq!(
            lines(dir.0.join("argv")),
            ["-n", "--", helper.as_str(), "relax-end"]
        );
        assert!(!dir.0.join("stdin").exists(), "no password was read");
    }

    #[test]
    fn what_goes_wrong_is_said_by_whom_it_went_wrong_with() {
        let dir = Scratch::new("door-wrong");
        let door = fake_sudo(&dir);
        let wrong = Proof::Typed(Secret::new("wrong"));
        assert_eq!(
            through(&door, RootVerb::RelaxEnd, &wrong, &mut |_| {})
                .unwrap_err()
                .why,
            Why::WrongPassword
        );
        assert_eq!(
            through(&door, RootVerb::RelaxEnd, &Proof::Cached, &mut |_| {})
                .unwrap_err()
                .why,
            Why::NeedsPassword,
            "nothing typed and no window"
        );
        let right = Proof::Typed(Secret::new(PASSWORD));
        assert_eq!(
            through(&door, RootVerb::FirewallApply, &right, &mut |_| {})
                .unwrap_err()
                .why,
            Why::Invalid,
            "the helper's own refusal"
        );
        let absent = Door {
            sudo: dir.0.join("absent"),
            ..door
        };
        assert_eq!(
            through(&absent, RootVerb::RelaxEnd, &right, &mut |_| {})
                .unwrap_err()
                .why,
            Why::Unstartable
        );
    }

    #[test]
    fn steps_arrive_as_they_are_said() {
        let dir = Scratch::new("door-steps");
        let door = fake_sudo(&dir);
        let mut steps = Vec::new();
        let out = through(
            &door,
            RootVerb::FingerEnroll {
                finger: crate::fprint::Finger::RightIndex,
            },
            &Proof::Typed(Secret::new(PASSWORD)),
            &mut |word| steps.push(word.to_owned()),
        )
        .unwrap();
        assert_eq!(steps, ["enroll-stage-passed", "enroll-completed"]);
        assert!(out.stdout.ends_with("done\n"), "{}", out.stdout);
    }

    #[test]
    fn what_the_helper_said_of_a_refusal_is_kept_for_the_journal() {
        let dir = Scratch::new("door-said");
        let door = fake_sudo(&dir);
        std::fs::write(dir.0.join("open"), b"").unwrap();
        let ended = attempt(&door, &["-n"], &RootVerb::FirewallApply, None, &mut |_| {}).unwrap();
        let Ended::Helper(Err(refusal), said) = ended else {
            panic!("{ended:?}");
        };
        assert_eq!(refusal.why, Why::Invalid);
        assert_eq!(
            journal_lines(&said),
            [
                "<3>ronoctua-security-root: the wish is not valid",
                "<3>ronoctua-security-root: no prefix here",
            ]
        );
        assert_eq!(
            journal_lines("<6>fine\n\n  \n<x>odd\n"),
            [
                "<6>ronoctua-security-root: fine",
                "<3>ronoctua-security-root: <x>odd",
            ]
        );
    }

    #[test]
    fn sudo_is_read_in_its_own_words() {
        assert_eq!(
            sudo_said("sudo: a password is required\n"),
            Why::NeedsPassword
        );
        assert_eq!(
            sudo_said("sudo: 1 incorrect password attempt\n"),
            Why::WrongPassword
        );
        assert_eq!(
            sudo_said("someone is not in the sudoers file.\n"),
            Why::NotAllowed
        );
        assert_eq!(
            sudo_said("Sorry, user someone may not run sudo on host.\n"),
            Why::NotAllowed
        );
        assert_eq!(sudo_said("sudo: unable to resolve host\n"), Why::Failed);
    }
}
