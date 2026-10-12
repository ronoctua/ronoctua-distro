//! The sudo door — the ONE code in the tree that spawns `sudo`
//! (feature `authority`, turned on by the shell alone).
//!
//! # How
//!
//! First `sudo -n -- <helper> <verb>`: it succeeds while the passwordless
//! window is open or sudo's own timestamp is fresh, and asks nothing. Only
//! when it fails for want of a password does the person answer, in one of two
//! ways.
//!
//! - **With the password only** ([`run`], and [`converse`] once the person
//!   chose "use the password"): `sudo -S -k -p "" -- <helper> <verb>`, the
//!   password and a line break written to stdin, stdin closed. It runs in a
//!   session of its own (`setsid`) with no controlling terminal, so sudo
//!   gives PAM no tty and the terminal gates jump over the security key and
//!   the finger: the password alone answers. A wrong password makes sudo read
//!   EOF and fail — [`crate::Why::WrongPassword`].
//! - **In a conversation** ([`converse`]): sudo on a pseudo-terminal the door
//!   owns, made its controlling terminal (`setsid --ctty`), with `-k -p
//!   <`[`PASSWORD_PROMPT`]`>`. PAM gets a tty, so a security key
//!   (`pam_u2f`) and a finger (`pam_fprintd`) may answer before the
//!   password. The door yields what is being asked ([`Asking`]) and takes the
//!   person's [`Answer`]: the password, "use the password" (the terminal is
//!   left and the password-only run takes over), or cancel.
//!
//! The password never goes in argv or in the environment (which is emptied
//! but for `LC_ALL=C`, so what sudo says is the English this module reads).
//!
//! # The door reads only its own sentinels
//!
//! What comes back on the terminal is sudo's and PAM's text — a lecture, a
//! module's messages, translated or not — and none of it is read. The door
//! listens for exactly three sentences it wrote itself
//! ([`Hearing`]): sudo's `-p` prompt ([`PASSWORD_PROMPT`]), `pam_u2f`'s
//! `cue_prompt` ([`TOUCH_CUE`], `nix/modules/yubikey.nix`) and the
//! `pam_echo` line directly before `pam_fprintd` ([`FINGER_CUE`],
//! `nix/modules/security.nix`). The password is written to the terminal only
//! right after the password sentinel, and sudo had turned the echo off before
//! it printed it.
//!
//! # Leaving a conversation
//!
//! sudo authenticates as root, so the person cannot kill it. The door
//! leaves through the terminal instead, with two characters the kernel and
//! sudo act on whatever text is around them:
//!
//! - **the interrupt** (`^C`), while a key or a finger is awaited: the
//!   kernel sends `SIGINT` to sudo, and the module gives up when it notices
//!   (or at its own timeout);
//! - **the end of input** (`^D`), at a password question — only ever after
//!   its sentinel, so after sudo flushed the terminal's input and turned the
//!   echo off: the read returns nothing, and sudo ends the authentication
//!   there and asks nothing more. A `SIGINT` that came between sudo's prompt
//!   and its read would be lost, and sudo would wait; the end of input waits
//!   in the terminal for the read, so there is no such moment.
//!
//! Every sentinel heard after the door left is answered the same way. The
//! door waits for that sudo to end before it says how it ended — a touch
//! given anyway still ran the verb, and the door says so rather than
//! "cancelled" — and before a password-only run starts, so a verb never runs
//! twice. A wrong password ends the same way: the password question again,
//! after one was written, gets the end of input, and the person is told,
//! never tried again in their name.
//!
//! # Telling sudo from the helper
//!
//! The helper's last line is its verdict ([`crate::verb::Said`]). A run with
//! no verdict line ended in sudo, before the helper: that is a password
//! wanted, refused, or not allowed at all, read from what sudo said on its
//! standard error.
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
//! Only a person's gesture: the shell's views. The brain and the daemons
//! never link this feature (`sudo-door-is-one`), and nothing automatic ever
//! reaches a password.
//!
//! # Where
//!
//! sudo is the setuid wrapper [`SUDO`] (a store path cannot be setuid); the
//! helper is the extracted `securityRoot`, whose path the session carries in
//! [`HELPER_VARIABLE`] (`nix/modules/security.nix`).

use std::io::{BufRead as _, Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};

use ronoctua_consent::keys::Secret;
use zeroize::Zeroizing;

pub use crate::verb::RootVerb;
use crate::verb::Said;
use crate::{Refusal, Why, refused};

/// sudo: NixOS's setuid wrapper.
pub const SUDO: &str = "/run/wrappers/bin/sudo";
/// The variable naming the root helper's store path.
pub const HELPER_VARIABLE: &str = "RONOCTUA_SECURITY_ROOT";
/// The most the door keeps of what the helper and sudo print.
const SAID_MOST_BYTES: usize = 64 * 1024;

/// The prompt sudo is given for the password on the door's terminal (`-p`)
/// — the sentinel the door hears the password question by. sudo uses it in
/// place of PAM's `Password: `; it never reaches a person's own terminal.
/// No `%`: sudo would expand it.
pub const PASSWORD_PROMPT: &str = "Password (ronoctua-sudo:password): ";
/// What `pam_u2f` says before it waits for a touch: its `cue_prompt`
/// (`nix/modules/yubikey.nix`). In a person's own terminal it is the cue
/// they read.
pub const TOUCH_CUE: &str = "Touch your security key (ronoctua-sudo:touch)";
/// What the `pam_echo` line directly before `pam_fprintd` says
/// (`nix/modules/security.nix`): the finger is about to be asked.
pub const FINGER_CUE: &str = "A fingerprint may answer for the password (ronoctua-sudo:finger)";

/// The terminal's interrupt character (`VINTR`, `^C`): written to the
/// door's terminal, the kernel sends `SIGINT` to sudo.
const INTERRUPT: u8 = 0x03;
/// The terminal's end-of-input character (`VEOF`, `^D`): at the start of a
/// line, sudo's read of the password returns nothing.
const END_OF_INPUT: u8 = 0x04;

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

/// What a conversation is waiting for, said the moment it starts waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Asking {
    /// The password.
    Password,
    /// A touch of a security key (the password still answers).
    Touch,
    /// A finger on the reader (the password still answers).
    Finger,
}

/// The person's answer to a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The password. While a touch or a finger is asked it is also "use
    /// the password", already typed.
    Password(Secret),
    /// Leave the key or the finger, and ask the password.
    UsePassword,
    /// Do nothing.
    Cancel,
}

/// What a conversation says as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// It is waiting for this.
    Asking(Asking),
    /// The helper's progress (an enrolment's `enroll-stage-passed`, …).
    Step(String),
}

/// What the door's threads, and the person, tell a conversation.
#[derive(Debug)]
enum Heard {
    /// A sentinel came on the terminal.
    Asks(Asking),
    /// A line of the helper's.
    Said(Said),
    /// The standard output closed: sudo and the helper are done. What they
    /// printed, kept.
    Out(String),
    /// The person answered.
    Answer(Answer),
}

/// The person's end of a conversation: hand it to whoever answers (the sudo
/// bar). Clones answer the same conversation.
#[derive(Debug, Clone)]
pub struct Answerer(Sender<Heard>);

impl Answerer {
    /// Answers. Once the conversation has ended this does nothing.
    pub fn answer(&self, answer: Answer) {
        let _ = self.0.send(Heard::Answer(answer));
    }
}

/// The conversation's end of [`answers`], given to [`converse`].
#[derive(Debug)]
pub struct Answers {
    tx: Sender<Heard>,
    rx: Receiver<Heard>,
}

/// A pair for one conversation: the [`Answerer`] the person's side keeps,
/// the [`Answers`] [`converse`] takes.
#[must_use]
pub fn answers() -> (Answerer, Answers) {
    let (tx, rx) = mpsc::channel();
    (Answerer(tx.clone()), Answers { tx, rx })
}

/// What the door hears on its terminal: its three sentinels, in the order
/// they come, and nothing else. Whatever does not end one of them is passed
/// over unread; only a tail that could still begin one is kept between
/// reads, and wiped.
#[derive(Debug, Default)]
struct Hearing {
    kept: Zeroizing<Vec<u8>>,
}

impl Hearing {
    /// The three, with what each asks.
    const SENTINELS: [(&'static str, Asking); 3] = [
        (PASSWORD_PROMPT, Asking::Password),
        (TOUCH_CUE, Asking::Touch),
        (FINGER_CUE, Asking::Finger),
    ];

    /// What `bytes`, after what came before, asked — in order. Pure.
    fn heard(&mut self, bytes: &[u8]) -> Vec<Asking> {
        self.kept.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some((at, len, asking)) = Self::SENTINELS
            .iter()
            .filter_map(|(sentinel, asking)| {
                find(&self.kept, sentinel.as_bytes()).map(|at| (at, sentinel.len(), *asking))
            })
            .min_by_key(|(at, ..)| *at)
        {
            out.push(asking);
            self.kept.drain(..at + len);
        }
        let longest = Self::SENTINELS
            .iter()
            .map(|(s, _)| s.len())
            .max()
            .unwrap_or(0);
        let keep = longest.saturating_sub(1);
        if self.kept.len() > keep {
            let cut = self.kept.len() - keep;
            self.kept[..cut].fill(0);
            self.kept.drain(..cut);
        }
        out
    }
}

/// Where `needle` first is in `haystack`. Pure.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
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

    /// The command for one attempt: the program first (`setsid --wait`
    /// when there is one — and `--ctty` on the door's terminal, which is
    /// then the command's stdin — then sudo), sudo's own flags, `--`, the
    /// helper and the verb.
    fn command(&self, flags: &[&str], verb: &RootVerb, on_a_terminal: bool) -> Command {
        let mut command = match &self.setsid {
            Some(setsid) => {
                let mut c = Command::new(setsid);
                c.arg("--wait");
                if on_a_terminal {
                    c.arg("--ctty");
                }
                c.arg(&self.sudo);
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

/// The password and its line break — the one place the secret is read, for
/// the two places it is written: sudo's stdin, or the door's terminal right
/// after the password sentinel. Wiped when dropped.
fn typed(password: &Secret) -> Zeroizing<Vec<u8>> {
    let mut line = Zeroizing::new(Vec::with_capacity(password.expose().len() + 1));
    line.extend_from_slice(password.expose().as_bytes());
    line.push(b'\n');
    line
}

/// Reads `stderr` to its end (or the cap) on a thread of its own.
fn collect(stderr: Option<std::process::ChildStderr>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut said = Vec::new();
        if let Some(stderr) = stderr {
            let _ = stderr.take(SAID_MOST_BYTES as u64).read_to_end(&mut said);
        }
        String::from_utf8_lossy(&said).into_owned()
    })
}

/// How a run ended, from its verdict (or none), its status and what sudo
/// said.
fn ended(verdict: Option<Said>, success: bool, stdout: String, said: String) -> Ended {
    match verdict {
        Some(Said::Done) if success => Ended::Helper(Ok(Output { stdout }), said),
        Some(Said::Refused(why)) => Ended::Helper(Err(refused(why)), said),
        Some(_) => Ended::Helper(Err(refused(Why::Failed)), said),
        None => Ended::Sudo(said),
    }
}

/// One attempt with no terminal: `flags` for sudo, `password` on stdin when
/// there is one, every `step` handed to `on_step` as it comes.
fn attempt(
    door: &Door,
    flags: &[&str],
    verb: &RootVerb,
    password: Option<&Secret>,
    on_step: &mut dyn FnMut(&str),
) -> Result<Ended, Refusal> {
    let mut command = door.command(flags, verb, false);
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
        let _ = stdin.write_all(&typed(password));
    }
    let stderr = collect(child.stderr.take());
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
    Ok(ended(verdict, status.success(), stdout, said))
}

/// What sudo's words (with `LC_ALL=C`) say went wrong.
fn sudo_said(said: &str) -> Why {
    if said.contains("a password is required") || said.contains("a terminal is required") {
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

/// The password-only run, with `password` typed.
fn with_password(
    door: &Door,
    verb: &RootVerb,
    password: &Secret,
    on_step: &mut dyn FnMut(&str),
) -> Result<Output, Refusal> {
    match attempt(door, &["-S", "-k", "-p", ""], verb, Some(password), on_step)? {
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
    match (why, proof) {
        (Why::NeedsPassword, Proof::Typed(password)) => {
            with_password(door, &verb, password, on_step)
        }
        (why, _) => Err(refused(why)),
    }
}

/// Runs `verb` as root through sudo, with the password only. Blocks (sudo
/// sleeps on a wrong password, an enrolment waits for a finger): call it on
/// a thread.
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

/// sudo running on the door's terminal.
struct Talk {
    child: Child,
    /// The terminal's master, to write the password and the interrupt.
    master: std::fs::File,
    stderr: std::thread::JoinHandle<String>,
}

/// A terminal for one conversation: its master (close-on-exec, so sudo
/// never holds it) and the side sudo gets as its controlling terminal.
fn terminal() -> std::io::Result<(OwnedFd, OwnedFd)> {
    use rustix::fs::{Mode, OFlags};
    use rustix::pty::OpenptFlags;
    let master =
        rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC)?;
    rustix::pty::grantpt(&master)?;
    rustix::pty::unlockpt(&master)?;
    let name = rustix::pty::ptsname(&master, Vec::new())?;
    // NOT THIS PROCESS'S terminal (`NOCTTY`): it is sudo's, through
    // `setsid --ctty`.
    let side = rustix::fs::open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    Ok((master, side))
}

/// Starts sudo on a terminal of the door's: every sentinel it hears, every
/// line of the helper's and the end of its output go to `heard`.
fn talk(door: &Door, verb: &RootVerb, heard: &Sender<Heard>) -> Result<Talk, Refusal> {
    let (master, side) = terminal().map_err(|e| {
        eprintln!("<3>no terminal for the sudo door: {e}");
        refused(Why::Unstartable)
    })?;
    let mut command = door.command(&["-k", "-p", PASSWORD_PROMPT], verb, true);
    command.stdin(Stdio::from(side));
    let mut child = crate::tools::spawn(&mut command).map_err(|_| refused(Why::Unstartable))?;
    // The command took its copy of the terminal's side; ours went with the
    // command, so the master reads EOF once sudo is gone.
    drop(command);
    let master = std::fs::File::from(master);
    let reader = master.try_clone().map_err(|_| refused(Why::Unstartable))?;
    let stderr = collect(child.stderr.take());

    // THE TERMINAL: only the sentinels leave this thread.
    let to = heard.clone();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut hearing = Hearing::default();
        let mut bytes = Zeroizing::new([0u8; 1024]);
        loop {
            match reader.read(&mut bytes[..]) {
                Ok(0) => break,
                Ok(n) => {
                    for asking in hearing.heard(&bytes[..n]) {
                        if to.send(Heard::Asks(asking)).is_err() {
                            return;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                // EIO: nobody holds the terminal's other side any more.
                Err(_) => break,
            }
        }
    });

    // THE HELPER'S LINES, and the end of its output.
    let to = heard.clone();
    let stdout = child.stdout.take();
    std::thread::spawn(move || {
        let mut kept = String::new();
        if let Some(out) = stdout {
            for line in std::io::BufReader::new(out).lines() {
                let Ok(line) = line else { break };
                if let Some(said) = Said::parse(&line) {
                    let _ = to.send(Heard::Said(said));
                }
                if kept.len() + line.len() < SAID_MOST_BYTES {
                    kept.push_str(&line);
                    kept.push('\n');
                }
            }
        }
        let _ = to.send(Heard::Out(kept));
    });
    Ok(Talk {
        child,
        master,
        stderr,
    })
}

/// Where a conversation is.
#[derive(Debug)]
enum State {
    /// Nothing asked yet.
    Waiting,
    /// A touch or a finger is asked.
    Asked,
    /// The password is asked, and nothing written.
    AtPassword,
    /// A password was written after the sentinel.
    Written,
    /// The sentinel came again after a password: it was wrong.
    Wrong,
    /// Leaving the terminal for the password-only run — with the password,
    /// once the person typed it.
    ToPassword(Option<Secret>),
    /// The person cancelled.
    Cancelling,
}

/// Writes the interrupt to the terminal, for a key's or a finger's wait
/// (see the module's "Leaving a conversation"). A terminal sudo has left is
/// no failure.
fn interrupt(master: &mut std::fs::File) {
    let _ = master.write_all(&[INTERRUPT]);
}

/// Writes the end of input to the terminal, for a password question whose
/// sentinel was heard. A terminal sudo has left is no failure.
fn end_of_input(master: &mut std::fs::File) {
    let _ = master.write_all(&[END_OF_INPUT]);
}

/// Runs `verb` as root through sudo, letting a security key or a finger
/// answer before the password (see the module's "In a conversation").
/// What is asked goes to `on_event` as it starts; the person answers
/// through the [`Answerer`] paired with `answers`. Blocks until it ends:
/// call it on a thread.
///
/// # Errors
///
/// [`Why::Cancelled`] when the person cancelled and nothing ran,
/// [`Why::WrongPassword`], and everything [`run`] refuses.
pub fn converse(
    verb: RootVerb,
    answers: Answers,
    on_event: &mut dyn FnMut(Event),
) -> Result<Output, Refusal> {
    converse_with(&Door::here()?, &verb, answers, on_event)
}

fn converse_with(
    door: &Door,
    verb: &RootVerb,
    answers: Answers,
    on_event: &mut dyn FnMut(Event),
) -> Result<Output, Refusal> {
    let Answers { tx, rx } = answers;
    let first = attempt(door, &["-n"], verb, None, &mut |word| {
        on_event(Event::Step(word.to_owned()));
    })?;
    match first {
        Ended::Helper(answer, said) => return told(answer, &said),
        Ended::Sudo(said) => match sudo_said(&said) {
            Why::NeedsPassword => {}
            why => return Err(refused(why)),
        },
    }
    let mut talk = talk(door, verb, &tx)?;
    // Only the answerers and the terminal's two threads may hold the line
    // now: when all are gone, nothing can answer any more.
    drop(tx);
    let mut state = State::Waiting;
    let mut verdict = None;
    let mut stdout = String::new();
    while let Ok(heard) = rx.recv() {
        match heard {
            Heard::Asks(Asking::Password) => match state {
                State::Waiting | State::Asked => {
                    state = State::AtPassword;
                    on_event(Event::Asking(Asking::Password));
                }
                State::AtPassword => {}
                State::Written => {
                    end_of_input(&mut talk.master);
                    state = State::Wrong;
                }
                State::Wrong | State::ToPassword(_) | State::Cancelling => {
                    end_of_input(&mut talk.master);
                }
            },
            Heard::Asks(asking) => match state {
                State::Waiting | State::Asked => {
                    state = State::Asked;
                    on_event(Event::Asking(asking));
                }
                State::Wrong | State::ToPassword(_) | State::Cancelling => {
                    interrupt(&mut talk.master);
                }
                State::AtPassword | State::Written => {}
            },
            Heard::Said(Said::Step(word)) => on_event(Event::Step(word)),
            Heard::Said(said) => verdict = Some(said),
            Heard::Out(kept) => {
                stdout = kept;
                break;
            }
            Heard::Answer(Answer::Password(password)) => match state {
                State::AtPassword => {
                    // THE ONE PLACE THE TERMINAL GETS THE PASSWORD: right
                    // after the sentinel, sudo's echo already off.
                    let _ = talk.master.write_all(&typed(&password));
                    state = State::Written;
                }
                State::Waiting | State::Asked => {
                    interrupt(&mut talk.master);
                    state = State::ToPassword(Some(password));
                }
                State::ToPassword(None) => state = State::ToPassword(Some(password)),
                State::Written | State::Wrong | State::ToPassword(Some(_)) | State::Cancelling => {}
            },
            Heard::Answer(Answer::UsePassword) => {
                if matches!(state, State::Waiting | State::Asked) {
                    interrupt(&mut talk.master);
                    state = State::ToPassword(None);
                    on_event(Event::Asking(Asking::Password));
                }
            }
            Heard::Answer(Answer::Cancel) => match state {
                // The question is on the terminal: end its read.
                State::AtPassword => {
                    end_of_input(&mut talk.master);
                    state = State::Cancelling;
                }
                State::Waiting | State::Asked | State::ToPassword(_) => {
                    interrupt(&mut talk.master);
                    state = State::Cancelling;
                }
                // Already ending (a password written, or wrong): whatever it
                // came to is said.
                State::Written | State::Wrong => state = State::Cancelling,
                State::Cancelling => {}
            },
        }
    }
    let status = talk.child.wait().map_err(|_| refused(Why::Failed))?;
    let said = talk.stderr.join().unwrap_or_default();
    drop(talk.master);
    match ended(verdict, status.success(), stdout, said) {
        // THE HELPER RAN — a touch given anyway included: its verdict is
        // what happened.
        Ended::Helper(answer, said) => told(answer, &said),
        Ended::Sudo(said) => {
            let password = match state {
                State::Cancelling => return Err(refused(Why::Cancelled)),
                State::Wrong => return Err(refused(Why::WrongPassword)),
                State::ToPassword(Some(password)) => password,
                State::ToPassword(None) => the_password(&rx).ok_or(refused(Why::Cancelled))?,
                State::Written => {
                    return Err(refused(match sudo_said(&said) {
                        Why::NeedsPassword => Why::WrongPassword,
                        Why::Failed => {
                            to_the_journal(&said);
                            Why::Failed
                        }
                        why => why,
                    }));
                }
                State::Waiting | State::Asked | State::AtPassword => match sudo_said(&said) {
                    // NO TERMINAL AFTER ALL (a build without `setsid`): the
                    // password-only run, asked now.
                    Why::NeedsPassword => {
                        on_event(Event::Asking(Asking::Password));
                        the_password(&rx).ok_or(refused(Why::Cancelled))?
                    }
                    // Words this module does not know: the journal gets them.
                    Why::Failed => {
                        to_the_journal(&said);
                        return Err(refused(Why::Failed));
                    }
                    why => return Err(refused(why)),
                },
            };
            with_password(door, verb, &password, &mut |word| {
                on_event(Event::Step(word.to_owned()));
            })
        }
    }
}

/// The password the person types next; `None` when they cancel, or nobody
/// is left to answer.
fn the_password(rx: &Receiver<Heard>) -> Option<Secret> {
    loop {
        match rx.recv() {
            Ok(Heard::Answer(Answer::Password(password))) => return Some(password),
            Ok(Heard::Answer(Answer::Cancel)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
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
    use crate::verb::RelaxSpan;

    const PASSWORD: &str = "correct horse";

    /// A sudo that records what it was given and behaves like sudo: `-n`
    /// wants a password unless `open` exists (the window), `-S` takes one
    /// line and runs the helper only when it is the right one.
    fn fake_sudo(dir: &Scratch) -> Door {
        let root = dir.0.display();
        let helper = dir.program(
            "helper",
            &format!(
                "#!/bin/sh\n\
                 : > {root}/ran\n\
                 case \"$1\" in\n\
                 finger-enroll) printf 'step enroll-stage-passed\\nstep enroll-completed\\ndone\\n' ;;\n\
                 firewall-apply) printf 'refused invalid\\n'; printf '<3>the wish is not valid\\nno prefix here\\n' >&2; exit 1 ;;\n\
                 *) printf 'done\\n' ;;\n\
                 esac\n"
            ),
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
                 -k)\n\
                   shift 4\n\
                   exec 3<>/dev/tty\n\
                   trap 'printf \"%s\\n\" interrupted >> {root}/heard; exit 1' INT\n\
                   wait_for_nothing() {{ while :; do IFS= read -r -t 1 got <&3; done; }}\n\
                   printf 'Password: \\r\\nPlease touch the device.\\r\\nPlace your finger on the fingerprint reader\\r\\n' >&3\n\
                   while IFS= read -r step; do\n\
                     case \"$step\" in\n\
                     touch-ok) printf '%s\\r\\n' '{TOUCH_CUE}' >&3; exec \"$@\" ;;\n\
                     touch-wait) printf '%s\\r\\n' '{TOUCH_CUE}' >&3; wait_for_nothing ;;\n\
                     finger-wait) printf '%s\\r\\n' '{FINGER_CUE}' >&3; wait_for_nothing ;;\n\
                     password)\n\
                       printf '%s' '{PASSWORD_PROMPT}' >&3\n\
                       if ! IFS= read -r got <&3; then printf '%s\\n' eof >> {root}/heard; exit 1; fi\n\
                       printf '%s\\n' \"$got\" >> {root}/heard\n\
                       if [ \"$got\" = '{PASSWORD}' ]; then exec \"$@\"; fi\n\
                       printf 'Sorry, try again.\\r\\n' >&3 ;;\n\
                     esac\n\
                   done < {root}/steps\n\
                   printf 'sudo: 3 incorrect password attempts\\n' >&2; exit 1 ;;\n\
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

    /// The fake on a real terminal: `setsid --ctty`, by store path, so the
    /// interrupt reaches it as `SIGINT` the way it reaches sudo.
    fn talking_sudo(dir: &Scratch, steps: &[&str]) -> Door {
        let setsid = std::env::var_os("RONOCTUA_SETSID")
            .map(PathBuf::from)
            .or_else(|| option_env!("RONOCTUA_SETSID").map(PathBuf::from))
            .expect("no setsid for this build: enter the environment or run through the flake");
        std::fs::write(dir.0.join("steps"), steps.join("\n") + "\n").unwrap();
        Door {
            setsid: Some(setsid),
            ..fake_sudo(dir)
        }
    }

    fn lines(path: PathBuf) -> Vec<String> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// A conversation where each thing asked gets the answer `reply` gives,
    /// and what was asked, in order.
    fn talk_through(
        door: &Door,
        verb: &RootVerb,
        reply: impl Fn(Asking) -> Option<Answer>,
    ) -> (Result<Output, Refusal>, Vec<Event>) {
        let (answerer, answers) = answers();
        let mut events = Vec::new();
        let result = converse_with(door, verb, answers, &mut |event| {
            if let Event::Asking(asking) = &event
                && let Some(answer) = reply(*asking)
            {
                answerer.answer(answer);
            }
            events.push(event);
        });
        (result, events)
    }

    fn typed_password() -> Answer {
        Answer::Password(Secret::new(PASSWORD))
    }

    #[test]
    fn the_password_goes_on_stdin_and_never_in_argv() {
        let dir = Scratch::new("door");
        let door = fake_sudo(&dir);
        let verb = RootVerb::Relax {
            span: RelaxSpan::Fifteen,
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
        // NOR DOES A CONVERSATION: nothing is asked, and no terminal opened.
        let (result, events) = talk_through(&door, &RootVerb::RelaxEnd, |_| None);
        result.unwrap();
        assert!(events.is_empty(), "{events:?}");
        assert_eq!(lines(dir.0.join("argv"))[0], "-n");
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
            sudo_said(
                "sudo: a terminal is required to read the password; either use the -S option to read from standard input or configure an askpass helper\n"
            ),
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

    /// FAILS IF: anything but the door's own three sentences is heard — a
    /// PAM module's "Password: ", "Please touch the device.", fprintd's
    /// words, a sentence that only begins like one, the password echoed —
    /// or one of them is missed when it comes split across reads, twice in
    /// one read, or after a long text.
    #[test]
    fn the_door_hears_its_own_sentinels_and_nothing_else() {
        let mut hearing = Hearing::default();
        for noise in [
            &b"Password: "[..],
            b"[sudo] password for someone: ",
            b"Please touch the device.\r\n",
            b"Place your finger on the fingerprint reader\r\n",
            b"Password (ronoctua-sudo:passwor",
            b"d) \r\n",
            b"Touch your security key\r\n",
            b"correct horse\r\n",
            b"(ronoctua-sudo:touch)\r\n",
        ] {
            assert!(
                hearing.heard(noise).is_empty(),
                "{}",
                String::from_utf8_lossy(noise)
            );
        }
        let mut hearing = Hearing::default();
        let touch = TOUCH_CUE.as_bytes();
        assert!(hearing.heard(&touch[..10]).is_empty());
        assert_eq!(hearing.heard(&touch[10..]), [Asking::Touch]);
        let long: Vec<u8> = std::iter::repeat_n(b'x', 10_000)
            .chain(FINGER_CUE.bytes())
            .chain(b"\r\nmore text\r\n".iter().copied())
            .chain(PASSWORD_PROMPT.bytes())
            .collect();
        assert_eq!(hearing.heard(&long), [Asking::Finger, Asking::Password]);
        assert!(
            hearing.kept.len() < PASSWORD_PROMPT.len().max(FINGER_CUE.len()),
            "only a tail that could begin a sentinel is kept"
        );
        assert!(hearing.heard(b"\r\n").is_empty(), "a sentinel heard twice");
    }

    /// FAILS IF: the conversation is not on the door's terminal with the
    /// password sentinel as sudo's prompt, a third party's words ask
    /// anything, the password is asked more than once or goes anywhere but
    /// the terminal after the sentinel, or the verb does not run.
    #[test]
    fn a_conversation_asks_what_its_sentinels_say() {
        let dir = Scratch::new("door-talk");
        let door = talking_sudo(&dir, &["password"]);
        let verb = RootVerb::Relax {
            span: RelaxSpan::Day,
        };
        let (result, events) = talk_through(&door, &verb, |asking| {
            (asking == Asking::Password).then(typed_password)
        });
        result.unwrap();
        assert_eq!(events, [Event::Asking(Asking::Password)]);
        let helper = door.helper.display().to_string();
        assert_eq!(
            lines(dir.0.join("argv")),
            [
                "-k",
                "-p",
                PASSWORD_PROMPT,
                "--",
                helper.as_str(),
                "relax",
                "1440"
            ]
        );
        assert_eq!(
            lines(dir.0.join("heard")),
            [PASSWORD],
            "read on the terminal"
        );
        assert!(!dir.0.join("stdin").exists(), "never on stdin");
        let env = std::fs::read_to_string(dir.0.join("env")).unwrap();
        assert!(!env.contains(PASSWORD) && !env.contains("HOME="), "{env}");
        assert!(dir.0.join("ran").exists());
    }

    /// FAILS IF: a touch that answers asks the password too, or does not
    /// run the verb.
    #[test]
    fn a_touch_answers_for_the_password() {
        let dir = Scratch::new("door-touch");
        let door = talking_sudo(&dir, &["touch-ok"]);
        let (result, events) = talk_through(&door, &RootVerb::StickWrite, |_| None);
        result.unwrap();
        // THE STAND-IN IS TOUCHED AT ONCE: the verb may end before the cue
        // is read off the terminal, and a cue after the end is no question.
        // That the cue is heard is `use_the_password_…`'s to prove.
        assert!(
            events.iter().all(|e| *e == Event::Asking(Asking::Touch)),
            "{events:?}"
        );
        assert!(!dir.0.join("heard").exists(), "nothing was typed");
        assert!(dir.0.join("ran").exists());
    }

    /// FAILS IF: "use the password" does not leave the terminal (sudo there
    /// is not interrupted), the password is not asked at once, or the
    /// password-only run does not take over — and run the verb once.
    #[test]
    fn use_the_password_leaves_the_key_for_the_password() {
        let dir = Scratch::new("door-use-password");
        let door = talking_sudo(&dir, &["touch-wait"]);
        let (result, events) = talk_through(&door, &RootVerb::FilesRoot, |asking| match asking {
            Asking::Touch => Some(Answer::UsePassword),
            Asking::Password => Some(typed_password()),
            Asking::Finger => None,
        });
        result.unwrap();
        assert_eq!(
            events,
            [
                Event::Asking(Asking::Touch),
                Event::Asking(Asking::Password)
            ]
        );
        assert_eq!(lines(dir.0.join("heard")), ["interrupted"]);
        assert_eq!(
            lines(dir.0.join("stdin")),
            [PASSWORD],
            "the password-only run"
        );
        assert_eq!(lines(dir.0.join("argv"))[..4], ["-S", "-k", "-p", ""]);
    }

    /// FAILS IF: a password typed while a finger is asked waits for another
    /// question, or is written to the terminal sudo is leaving.
    #[test]
    fn a_password_typed_while_a_finger_is_asked_is_used_at_once() {
        let dir = Scratch::new("door-typed-early");
        let door = talking_sudo(&dir, &["finger-wait"]);
        let (result, events) = talk_through(&door, &RootVerb::FilesRoot, |asking| {
            (asking == Asking::Finger).then(typed_password)
        });
        result.unwrap();
        assert_eq!(events, [Event::Asking(Asking::Finger)]);
        assert_eq!(lines(dir.0.join("heard")), ["interrupted"]);
        assert_eq!(lines(dir.0.join("stdin")), [PASSWORD]);
    }

    /// FAILS IF: a cancel runs the verb, is said as anything but a cancel,
    /// or leaves sudo waiting.
    #[test]
    fn a_cancel_runs_nothing() {
        let dir = Scratch::new("door-cancel");
        let door = talking_sudo(&dir, &["finger-wait"]);
        let (result, events) = talk_through(&door, &RootVerb::FilesRoot, |_| Some(Answer::Cancel));
        assert_eq!(result.unwrap_err().why, Why::Cancelled);
        assert_eq!(events, [Event::Asking(Asking::Finger)]);
        assert_eq!(lines(dir.0.join("heard")), ["interrupted"]);
        assert!(!dir.0.join("ran").exists(), "a cancelled verb ran");
        // AND AT THE PASSWORD: its read ends with nothing in it, and nothing
        // runs.
        let dir = Scratch::new("door-cancel-password");
        let door = talking_sudo(&dir, &["password"]);
        let (result, _) = talk_through(&door, &RootVerb::FilesRoot, |_| Some(Answer::Cancel));
        assert_eq!(result.unwrap_err().why, Why::Cancelled);
        assert_eq!(lines(dir.0.join("heard")), ["eof"]);
        assert!(!dir.0.join("ran").exists());
    }

    /// FAILS IF: a wrong password on the terminal is tried again in the
    /// person's name, or said as anything but a wrong password.
    #[test]
    fn a_wrong_password_on_the_terminal_is_said_once() {
        let dir = Scratch::new("door-talk-wrong");
        let door = talking_sudo(&dir, &["password", "password", "password"]);
        let (result, events) = talk_through(&door, &RootVerb::FilesRoot, |asking| {
            (asking == Asking::Password).then(|| Answer::Password(Secret::new("wrong")))
        });
        assert_eq!(result.unwrap_err().why, Why::WrongPassword);
        assert_eq!(events, [Event::Asking(Asking::Password)]);
        assert_eq!(lines(dir.0.join("heard")), ["wrong", "eof"]);
        assert!(!dir.0.join("ran").exists());
    }

    /// FAILS IF: a conversation whose answerer is gone waits forever after
    /// sudo left the terminal for the password.
    #[test]
    fn nobody_left_to_answer_is_a_cancel() {
        let dir = Scratch::new("door-nobody");
        let door = talking_sudo(&dir, &["touch-wait"]);
        let (answerer, answers) = answers();
        let mut answerer = Some(answerer);
        let result = converse_with(&door, &RootVerb::FilesRoot, answers, &mut |event| {
            if event == Event::Asking(Asking::Touch)
                && let Some(answerer) = answerer.take()
            {
                answerer.answer(Answer::UsePassword);
            }
        });
        assert_eq!(result.unwrap_err().why, Why::Cancelled);
        assert!(!dir.0.join("ran").exists());
    }
}
