//! `ronoctua-person` — root's half of the person's declared data.
//!
//! ```text
//! ronoctua-person apply --staged DIR --machine DIR --owner USER [--verdict FILE] [--lock FILE]
//! ronoctua-person approve --request DIR --pin FILE --machine DIR --owner USER [--verdict FILE] [--lock FILE]
//! ronoctua-person pin --pin FILE [--owner USER] [--machine DIR] [--lock FILE]
//! ronoctua-person hold [--lock FILE] [--patient] -- PROGRAM [ARG…]
//! ronoctua-person request-approval ID HASH [--into DIR]
//! ronoctua-person read KIND FILE
//! ronoctua-person record [--state DIR] [--into DIR] [--host NAME] [--rehearse]
//! ```
//!
//! - `apply` (the unit `ronoctua-person-apply.service`, and the installer):
//!   the staged tree, validated and held to the person's signature under
//!   the key the machine pinned, swapped into the machine directory's
//!   `person/` whole — or refused, and the machine's data stays as it was.
//! - `approve` (the unit the sudo door's `person-module-approve` starts):
//!   the staged request and root's pin of what the door was given consumed,
//!   held to each other, and approved only at the hash
//!   `nix/person-hash.nix` (`RONOCTUA_PERSON_HASH`) computes now.
//! - `pin` (the unit the sudo door's `person-signer-pin` starts, and the
//!   installer): the person's signing key, read from the FILE given (root's
//!   own, or the `--owner`'s, consumed as it is read), pinned in
//!   `local/signer.json`.
//! - `hold` (the update's root half): root's lock held while PROGRAM runs,
//!   and handed down to it as its standard input — so the rebuild it runs
//!   holds it too, and an apply waits until the switch is
//!   done. With `--patient` it waits for the lock until its holder lets go
//!   (said to the journal), instead of refusing as `busy`.
//! - `request-approval` (the person): stages that request.
//! - `read`: one file read, validated and printed in its canonical form, or
//!   refused with its reason — what `person-schema-golden` runs.
//! - `record` (the person, the update's last steps): the machine's record
//!   (`--state`, by default `RONOCTUA_MACHINE_STATE`) copied, validated and
//!   canonical, into the person's mirror (`--into`, by default
//!   `$XDG_STATE_HOME/ronoctua/person`) as `machines/<hostname>.json` — and
//!   left alone when it says the same; `--rehearse` says what it would do
//!   (`would-record`) and writes nothing.
//!
//! `apply`, `approve` and `pin` take root's lock (`--lock`, by default
//! `ronoctua_person::root::LOCK`) before anything else, waiting at most
//! `LOCK_WAIT_S`, and refuse as `busy` after that. `apply` and `approve`
//! write their verdict (`--verdict`) on every way out, while the lock is
//! still held.
//!
//! `--machine` defaults to `RONOCTUA_MACHINE_DIR`. The last line on standard
//! output is the verdict; what went wrong in detail goes to standard error,
//! with the journal's priority prefix. Exit 0 is done (and the verdict
//! written, when one was asked for), 1 refused, 2 a command line that is not
//! one of the above; `hold` exits as PROGRAM did.

#![forbid(unsafe_code)]

mod apply;
mod approve;
mod disk;
mod nix;
mod pin;
mod record;
#[cfg(test)]
mod testing;
mod walk;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use ronoctua_person::approve::Denied;
use ronoctua_person::root::{self, Lock, LockRefused};
use ronoctua_person::tree::{Outcome, Refused, TreeWhy, Verdict};
use ronoctua_person::{Document, Stamp};

/// The variable that names the machine directory (`nix/rust.nix`, the units).
const MACHINE_DIR: &str = "RONOCTUA_MACHINE_DIR";
/// The machine's own `nix`, by store path.
const NIX: &str = "RONOCTUA_NIX";
/// The folder of the machine's record (`nix/modules/machine.nix`).
const MACHINE_STATE: &str = "RONOCTUA_MACHINE_STATE";
/// `nix/person-hash.nix`, by store path.
const PERSON_HASH: &str = "RONOCTUA_PERSON_HASH";
/// Where the person stages a request when `--into` is not given.
const APPROVE_DIR: &str = root::APPROVE;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(Usage) => {
            eprintln!("{}", usage());
            ExitCode::from(2)
        }
    }
}

/// A command line that is not one of the seven.
struct Usage;

fn usage() -> &'static str {
    "usage: ronoctua-person apply --staged DIR --machine DIR --owner USER [--verdict FILE] [--lock FILE]\n\
     \x20      ronoctua-person approve --request DIR --pin FILE --machine DIR --owner USER [--verdict FILE] [--lock FILE]\n\
     \x20      ronoctua-person pin --pin FILE [--owner USER] [--machine DIR] [--lock FILE]\n\
     \x20      ronoctua-person hold [--lock FILE] [--patient] -- PROGRAM [ARG...]\n\
     \x20      ronoctua-person request-approval ID HASH [--into DIR]\n\
     \x20      ronoctua-person read KIND FILE\n\
     \x20      ronoctua-person record [--state DIR] [--into DIR] [--host NAME] [--rehearse]\n\
     KIND: declared declined approved options person-applied distro-apps\n\
     \x20     declared-android state verdict request approve-verdict signer choice\n\
     \x20     distro-pins"
}

/// The value of `--name`, the positional words, and whether `--rehearse`
/// was given; anything unknown is a usage error.
struct Line {
    flags: std::collections::BTreeMap<String, String>,
    words: Vec<String>,
    rehearse: bool,
}

fn line(args: &[String], takes: &[&str]) -> Result<Line, Usage> {
    let mut flags = std::collections::BTreeMap::new();
    let mut words = Vec::new();
    let mut rehearse = false;
    let mut at = args.iter();
    while let Some(arg) = at.next() {
        if arg == "--rehearse" && takes.contains(&"--rehearse") {
            rehearse = true;
        } else if let Some(name) = arg.strip_prefix("--") {
            if !takes.contains(&arg.as_str()) || flags.contains_key(name) {
                return Err(Usage);
            }
            flags.insert(name.to_owned(), at.next().ok_or(Usage)?.clone());
        } else {
            words.push(arg.clone());
        }
    }
    Ok(Line {
        flags,
        words,
        rehearse,
    })
}

impl Line {
    fn get(&self, name: &str) -> Result<&str, Usage> {
        self.flags.get(name).map(String::as_str).ok_or(Usage)
    }

    fn machine(&self) -> Result<String, Usage> {
        self.flags
            .get("machine")
            .cloned()
            .or_else(|| std::env::var(MACHINE_DIR).ok())
            .ok_or(Usage)
    }

    /// Root's lock: `--lock`, or the machine's.
    fn lock(&self) -> &Path {
        Path::new(self.flags.get("lock").map_or(root::LOCK, String::as_str))
    }
}

/// Root's lock at `path`, waited for at most [`root::LOCK_WAIT_S`].
fn take_lock(path: &Path) -> Result<Lock, LockRefused> {
    Lock::take(path, Duration::from_secs(root::LOCK_WAIT_S))
}

/// Root's lock for `hold`: waited for until its holder lets go (`--patient`,
/// the update's root half), or for at most [`root::LOCK_WAIT_S`].
fn lock_for_hold(path: &Path, patient: bool) -> Result<Lock, LockRefused> {
    lock_waiting(path, patient, Duration::from_secs(root::LOCK_WAIT_S))
}

/// [`lock_for_hold`], with the impatient wait a parameter — so a test tells
/// the two apart without waiting half a minute.
fn lock_waiting(path: &Path, patient: bool, impatience: Duration) -> Result<Lock, LockRefused> {
    if patient {
        patiently(path)
    } else {
        Lock::take(path, impatience)
    }
}

/// Root's lock at `path`, waited for until its holder lets go — and the wait
/// SAID, to the journal of whoever waits: a unit that sits still for the
/// length of somebody else's build has to say what it is waiting for.
fn patiently(path: &Path) -> Result<Lock, LockRefused> {
    match Lock::take(path, Duration::ZERO) {
        Err(LockRefused::Busy) => {
            eprintln!(
                "<5>another of root's programs holds the lock of the person's data \
                 (an apply, a declaration): waiting until it is done"
            );
            Lock::wait_for(path)
        }
        other => other,
    }
}

/// Now, as the files write it.
fn now() -> Stamp {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Stamp::from_unix_s(s)
}

/// The uid an `--owner` names.
fn owner(name: &str) -> Result<u32, String> {
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    disk::uid_of(name, &passwd).ok_or_else(|| format!("{name:?} is not a user of this machine"))
}

/// A program the environment names by store path, or a sentence.
fn program(variable: &str) -> Result<String, String> {
    std::env::var(variable)
        .ok()
        .filter(|p| p.starts_with('/'))
        .ok_or_else(|| format!("{variable} does not name a program by its path"))
}

/// Says a refusal: the journal's detail, and the verdict line.
fn refused(word: &str, detail: &str) -> ExitCode {
    eprintln!("<3>{detail}");
    println!("refused {word}");
    ExitCode::FAILURE
}

/// The refusal of a tree whose lock was not taken.
fn lock_refused(e: &LockRefused, path: &Path) -> Refused {
    let why = match e {
        LockRefused::Busy => TreeWhy::Busy,
        LockRefused::NotOurs(_) | LockRefused::Unopenable(_) => TreeWhy::Unwritable,
    };
    Refused::new(why, path.display().to_string(), e.to_string())
}

/// Writes a verdict, when one was asked for; whether that went well.
fn wrote<T: serde::Serialize>(file: Option<&String>, verdict: &T) -> bool {
    match file.map(|f| write_verdict(f, verdict)) {
        None | Some(Ok(())) => true,
        Some(Err(e)) => {
            eprintln!("<3>the verdict was not written: {e}");
            false
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, Usage> {
    let (command, rest) = args.split_first().ok_or(Usage)?;
    Ok(match command.as_str() {
        "apply" => {
            let l = line(
                rest,
                &["--staged", "--machine", "--owner", "--verdict", "--lock"],
            )?;
            if !l.words.is_empty() {
                return Err(Usage);
            }
            let (staged, machine, who) = (l.get("staged")?, l.machine()?, l.get("owner")?);
            let at = now();
            // THE LOCK FIRST, and kept until the verdict is written: a start
            // that joined this run reads THIS run's verdict, never a later
            // one's.
            let lock = take_lock(l.lock());
            let (nonce, done) = match &lock {
                Err(e) => (None, Err(lock_refused(e, l.lock()))),
                Ok(_) => match owner(who) {
                    Err(e) => (None, Err(Refused::new(TreeWhy::NotOwned, staged, e))),
                    Ok(uid) => apply::apply(staged, &machine, uid, &at),
                },
            };
            let verdict = Verdict::of(at, nonce, done.clone());
            let written = wrote(l.flags.get("verdict"), &verdict);
            drop(lock);
            match done {
                Ok(done) => {
                    for path in &done.ignored {
                        eprintln!("<6>not taken: {path}");
                    }
                    for back in &done.kept_back {
                        eprintln!(
                            "<4>{} was kept as the machine had it ({}): the person's signature does not vouch for what was staged",
                            back.at,
                            back.why.id()
                        );
                    }
                    for out in &done.left_out {
                        eprintln!(
                            "<4>the module {} was left out ({} at '{}': {}); the machine {}",
                            out.module,
                            out.why.id(),
                            out.at,
                            out.detail,
                            if out.kept {
                                "keeps the copy it had"
                            } else {
                                "has none"
                            }
                        );
                    }
                    println!(
                        "{}",
                        match done.outcome {
                            Outcome::Applied => "applied",
                            Outcome::Unchanged => "unchanged",
                            Outcome::Refused => "refused",
                        }
                    );
                    if written {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(r) => refused(r.why.id(), &format!("the staged tree was refused: {r}")),
            }
        }
        "approve" => {
            let l = line(
                rest,
                &[
                    "--request",
                    "--pin",
                    "--machine",
                    "--owner",
                    "--verdict",
                    "--lock",
                ],
            )?;
            if !l.words.is_empty() {
                return Err(Usage);
            }
            let (request_dir, pin, machine, who) = (
                l.get("request")?,
                l.get("pin")?,
                l.machine()?,
                l.get("owner")?,
            );
            let at = now();
            let tools = program(NIX).and_then(|n| program(PERSON_HASH).map(|h| (n, h)));
            // The request and the pin are taken FIRST — both spent whatever
            // comes after — and then the lock, kept until the verdict is
            // written. The pin is root's (the sudo door's helper wrote it);
            // the request is held to it.
            let pinned = approve::take_pin(pin, 0);
            let request = owner(who)
                .map_err(|e| approve::Failed {
                    denied: Denied::NotOwned,
                    computed: None,
                    detail: e,
                })
                .and_then(|uid| approve::take_request(request_dir, uid))
                .and_then(|request| pinned.and_then(|pin| approve::held_to_pin(request, &pin)));
            let lock = request.as_ref().ok().map(|_| take_lock(l.lock()));
            let result = match (&request, &lock, tools) {
                (Err(f), _, _) => Err(f.clone()),
                (Ok(_), Some(Err(e)), _) => Err(approve::Failed {
                    denied: if *e == LockRefused::Busy {
                        Denied::Busy
                    } else {
                        Denied::Unwritable
                    },
                    computed: None,
                    detail: e.to_string(),
                }),
                (Ok(_), _, Err(e)) => Err(approve::Failed {
                    denied: Denied::Unhashable,
                    computed: None,
                    detail: e,
                }),
                (Ok(request), _, Ok((nix, hasher))) => approve::record(
                    request,
                    &machine,
                    &nix::Nix {
                        program: PathBuf::from(nix),
                    },
                    &hasher,
                    at.clone(),
                ),
            };
            let verdict = match &result {
                Ok(computed) => ronoctua_person::approve::Verdict::of(
                    at,
                    request.as_ref().ok(),
                    Some(computed.clone()),
                    None,
                ),
                Err(f) => ronoctua_person::approve::Verdict::of(
                    at,
                    request.as_ref().ok(),
                    f.computed.clone(),
                    Some(f.denied),
                ),
            };
            let written = wrote(l.flags.get("verdict"), &verdict);
            drop(lock);
            match result {
                Ok(_) if written => {
                    eprintln!("<5>approved {}", verdict.id.as_deref().unwrap_or("?"));
                    println!("done");
                    ExitCode::SUCCESS
                }
                Ok(_) => refused(
                    "verdict-unwritten",
                    "the approval was recorded, and its verdict was not written",
                ),
                Err(f) => refused(f.denied.id(), &f.detail),
            }
        }
        "pin" => {
            let l = line(rest, &["--pin", "--owner", "--machine", "--lock"])?;
            if !l.words.is_empty() {
                return Err(Usage);
            }
            let (file, machine) = (l.get("pin")?, l.machine()?);
            // ROOT'S OWN FILE unless the installer names the user whose
            // restore wrote it.
            let uid = match l.flags.get("owner") {
                None => rustix::process::geteuid().as_raw(),
                Some(who) => match owner(who) {
                    Ok(uid) => uid,
                    Err(e) => return Ok(refused(TreeWhy::NotOwned.id(), &e)),
                },
            };
            // THE KEY FIRST — consumed whatever comes after — then the lock.
            let key = match pin::take_key(file, uid) {
                Ok(key) => key,
                Err(r) => return Ok(refused(r.why.id(), &r.to_string())),
            };
            let _lock = match take_lock(l.lock()) {
                Ok(lock) => lock,
                Err(e) => return Ok(refused(lock_refused(&e, l.lock()).why.id(), &e.to_string())),
            };
            match pin::pin(&machine, &key, now()) {
                Ok(pin::Pinned::New(fingerprint)) => {
                    eprintln!("<5>the person's signing key {fingerprint} is pinned");
                    println!("pinned {fingerprint}");
                    ExitCode::SUCCESS
                }
                Ok(pin::Pinned::Again(fingerprint)) => {
                    println!("unchanged {fingerprint}");
                    ExitCode::SUCCESS
                }
                Err(r) => refused(r.why.id(), &format!("the key was not pinned: {r}")),
            }
        }
        "hold" => {
            let split = rest.iter().position(|a| a == "--").ok_or(Usage)?;
            // `--patient`, at most once: wait for the lock for as long as
            // its holder keeps it (`Lock::wait_for`) — the update's root
            // half, which an apply building the system must not fail.
            let patient = rest[..split].iter().filter(|a| *a == "--patient").count();
            if patient > 1 {
                return Err(Usage);
            }
            let flags: Vec<String> = rest[..split]
                .iter()
                .filter(|a| *a != "--patient")
                .cloned()
                .collect();
            let l = line(&flags, &["--lock"])?;
            let (program, args) = rest[split + 1..].split_first().ok_or(Usage)?;
            if !l.words.is_empty() || !program.starts_with('/') {
                return Err(Usage);
            }
            let lock = match lock_for_hold(l.lock(), patient == 1) {
                Ok(lock) => lock,
                Err(e) => return Ok(refused(lock_refused(&e, l.lock()).why.id(), &e.to_string())),
            };
            let stdin = match lock.for_a_child() {
                Ok(stdin) => stdin,
                Err(e) => {
                    return Ok(refused(
                        "unwritable",
                        &format!("the lock could not be handed down: {e}"),
                    ));
                }
            };
            match std::process::Command::new(program)
                .args(args)
                .stdin(stdin)
                .status()
            {
                Ok(status) => {
                    use std::os::unix::process::ExitStatusExt as _;
                    let code = status
                        .code()
                        .or_else(|| status.signal().map(|s| 128 + s))
                        .unwrap_or(1);
                    ExitCode::from(u8::try_from(code).unwrap_or(1))
                }
                Err(e) => refused("unstartable", &format!("{program}: {e}")),
            }
        }
        "request-approval" => {
            let l = line(rest, &["--into"])?;
            let [id, hash] = l.words.as_slice() else {
                return Err(Usage);
            };
            let into = l.flags.get("into").map_or(APPROVE_DIR, String::as_str);
            match approve::request(into, id, hash) {
                Ok(()) => {
                    println!("done");
                    ExitCode::SUCCESS
                }
                Err(e) => refused("invalid", &e),
            }
        }
        "record" => {
            let l = line(rest, &["--state", "--into", "--host", "--rehearse"])?;
            if !l.words.is_empty() {
                return Err(Usage);
            }
            let Some(state) = l
                .flags
                .get("state")
                .cloned()
                .or_else(|| std::env::var(MACHINE_STATE).ok())
            else {
                return Ok(refused(
                    "no-record",
                    &format!("{MACHINE_STATE} does not name the folder of the machine's record"),
                ));
            };
            let Some(into) = l
                .flags
                .get("into")
                .map(PathBuf::from)
                .or_else(record::mirror)
            else {
                return Ok(refused(
                    "no-home",
                    "neither XDG_STATE_HOME nor HOME says where the person's mirror is",
                ));
            };
            let host = l.flags.get("host").cloned().unwrap_or_else(record::host);
            match record::record(Path::new(&state), &into, &host, l.rehearse) {
                Ok(record::Recorded::Nothing) => {
                    println!("nothing");
                    ExitCode::SUCCESS
                }
                Ok(record::Recorded::Unchanged) => {
                    println!("unchanged");
                    ExitCode::SUCCESS
                }
                Ok(record::Recorded::Written(at)) => {
                    eprintln!("<6>the machine's record is in {}", at.display());
                    println!("recorded");
                    ExitCode::SUCCESS
                }
                Ok(record::Recorded::Would(at)) => {
                    eprintln!(
                        "<6>a run would write the machine's record into {}",
                        at.display()
                    );
                    println!("would-record");
                    ExitCode::SUCCESS
                }
                Err(e) => refused("unreadable", &e),
            }
        }
        "read" => {
            let l = line(rest, &[])?;
            let [kind, file] = l.words.as_slice() else {
                return Err(Usage);
            };
            read(kind, file).ok_or(Usage)?
        }
        _ => return Err(Usage),
    })
}

/// Writes a verdict into its file, whole (the folder is root's).
fn write_verdict<T: serde::Serialize>(file: &str, verdict: &T) -> Result<(), String> {
    let path = std::path::Path::new(file);
    let (Some(folder), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str()))
    else {
        return Err(format!("{file} is not a file's path"));
    };
    let me = disk::Whose {
        owner: rustix::process::geteuid().as_raw(),
    };
    let folder = folder.to_str().ok_or("a folder that is not text")?;
    let fd = disk::open_root(folder, me).map_err(|r| r.to_string())?;
    disk::write_whole(
        &fd,
        name,
        ronoctua_person::to_json(verdict).as_bytes(),
        0o644,
    )
    .map_err(|r| r.to_string())
}

/// `read KIND FILE`: the canonical text, or `refused <why> <pointer>`;
/// `None` for a kind that is not one.
fn read(kind: &str, file: &str) -> Option<ExitCode> {
    use ronoctua_person::{
        approve, choice, declared, declined, distro_pins, local, options, reports, state, tree,
    };
    fn one<T: Document>(bytes: &[u8]) -> Result<String, ronoctua_person::Refusal> {
        ronoctua_person::parse::<T>(bytes).map(|d| ronoctua_person::to_json(&d))
    }
    let reader: fn(&[u8]) -> Result<String, ronoctua_person::Refusal> = match kind {
        "declared" => one::<declared::Declared>,
        "declined" => one::<declined::Declined>,
        "approved" => one::<local::Approved>,
        "options" => one::<options::Options>,
        "person-applied" => one::<reports::PersonApplied>,
        "distro-apps" => one::<reports::DistroApps>,
        "declared-android" => one::<reports::DeclaredAndroid>,
        "state" => one::<state::MachineState>,
        "last-apply" => one::<state::Whole>,
        "verdict" => one::<tree::Verdict>,
        "request" => one::<approve::Request>,
        "approve-verdict" => one::<approve::Verdict>,
        "signer" => one::<local::Signer>,
        "choice" => one::<choice::Choice>,
        "distro-pins" => one::<distro_pins::DistroPins>,
        _ => return None,
    };
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => return Some(refused("unreadable", &format!("{file}: {e}"))),
    };
    Some(match reader(&bytes) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(r) => {
            eprintln!("<3>{file}: {r}");
            println!("refused {} {}", r.why.id(), r.at);
            ExitCode::FAILURE
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn a_command_line_takes_only_its_own_flags() {
        let l = line(
            &words(&["--staged", "/s", "--owner", "me"]),
            &["--staged", "--owner"],
        )
        .ok();
        assert!(l.is_some_and(|l| l.flags["staged"] == "/s" && l.flags["owner"] == "me"));
        assert!(line(&words(&["--rehearse"]), &["--machine"]).is_err());
        assert!(
            line(&words(&["--staged"]), &["--staged"]).is_err(),
            "a flag with no value"
        );
        assert!(
            line(&words(&["--staged", "a", "--staged", "b"]), &["--staged"]).is_err(),
            "twice"
        );
        assert!(matches!(run(&words(&["format-the-disk"])), Err(Usage)));
        assert!(matches!(run(&words(&[])), Err(Usage)));
        assert!(matches!(
            run(&words(&["read", "nonsense", "/x"])),
            Err(Usage)
        ));
        for bad in [
            &["hold", "/bin/true"][..],
            &["hold", "--"],
            &["hold", "--", "true"],
            &["hold", "--wait", "1", "--", "/bin/true"],
            &["hold", "--patient", "--patient", "--", "/bin/true"],
        ] {
            assert!(matches!(run(&words(bad)), Err(Usage)), "{bad:?}");
        }
    }

    /// FAILS IF: `hold --patient` refuses while another of root's programs
    /// holds the lock — the update failed as `busy` while the software
    /// centre's apply was building — or does not take it once the holder
    /// lets go. (The lock is asked of, and no program run: a fork here would
    /// race the scripts the verification's tests write and exec.)
    #[test]
    fn a_patient_hold_waits_for_the_holder() {
        let s = crate::testing::Scratch::new("patient-hold");
        let path = s.0.join("lock");
        let holder = Lock::take(&path, Duration::ZERO).unwrap();
        let started = std::time::Instant::now();
        let impatience = Duration::from_millis(100);
        let waiting = |patient: bool| {
            let path = path.clone();
            std::thread::spawn(move || lock_waiting(&path, patient, impatience).map(|l| l.is_own()))
        };
        // WITHOUT `--patient` it refuses as busy once its wait is over…
        assert_eq!(
            waiting(false).join().unwrap(),
            Err(LockRefused::Busy),
            "an impatient hold waited past its wait"
        );
        // …and WITH it, it waits past that, and takes the lock when the
        // holder lets go.
        let patient = waiting(true);
        std::thread::sleep(Duration::from_millis(400));
        assert!(!patient.is_finished(), "it did not wait for the holder");
        drop(holder);
        assert_eq!(patient.join().unwrap(), Ok(true));
        assert!(started.elapsed() >= Duration::from_millis(400));
    }

    /// FAILS IF: an apply that ends before it reads anything — the owner
    /// unknown — leaves the verdict of an earlier run in place, so whoever
    /// started it would read somebody else's answer.
    #[test]
    fn every_way_out_writes_its_verdict() {
        let s = crate::testing::Scratch::new("main-verdict");
        s.dir("staged/person");
        s.dir("machine");
        s.file("verdict.json", b"an earlier run's");
        let at = |rel: &str| s.0.join(rel).display().to_string();
        let code = run(&words(&[
            "apply",
            "--staged",
            &at("staged"),
            "--machine",
            &at("machine"),
            "--owner",
            "nobody-of-this-machine-at-all",
            "--verdict",
            &at("verdict.json"),
            "--lock",
            &at("lock"),
        ]))
        .ok()
        .unwrap();
        assert_eq!(code, ExitCode::FAILURE);
        let verdict: Verdict =
            ronoctua_person::parse(&std::fs::read(s.0.join("verdict.json")).unwrap()).unwrap();
        assert_eq!(verdict.outcome, Outcome::Refused);
        assert_eq!(verdict.refused.unwrap().why, TreeWhy::NotOwned);
        assert_eq!(verdict.nonce, None);

        let me = rustix::process::geteuid().as_raw().to_string();
        s.file("staged/nonce", b"stager-0002\n");
        let code = run(&words(&[
            "apply",
            "--staged",
            &at("staged"),
            "--machine",
            &at("machine"),
            "--owner",
            &me,
            "--verdict",
            &at("verdict.json"),
            "--lock",
            &at("lock"),
        ]))
        .ok()
        .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        let verdict: Verdict =
            ronoctua_person::parse(&std::fs::read(s.0.join("verdict.json")).unwrap()).unwrap();
        assert_eq!(
            verdict.outcome,
            Outcome::Unchanged,
            "an empty person/ on a machine with none"
        );
        assert_eq!(verdict.nonce.as_deref(), Some("stager-0002"));
    }
}
