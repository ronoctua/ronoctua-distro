//! `ronoctua-security-root` — the one program the security centre runs as
//! root.
//!
//! # Who runs it
//!
//! - **The sudo door** (`ronoctua_security::authority`), for a person's
//!   gesture: `sudo … -- ronoctua-security-root <RootVerb>`. sudo names the
//!   person in `SUDO_UID`; uid 0 is refused.
//! - **Units and NetworkManager's dispatcher**, with no `SUDO_UID`:
//!   `firewall-apply` (the boot's table), `relax-expire <until>` (the
//!   window's timer), `relax-boot` (the boot's look at the window),
//!   `shields-up` and `firewall-apply --link …`. A unit verb that arrives
//!   through sudo is refused: the door's set stays the door's.
//!
//! It refuses to run as anybody but root, and does nothing a verb does not
//! name. Its last line on standard output is the verdict (`done` or
//! `refused <why>`), after any progress (`step <word>`); what went wrong in
//! detail goes to standard error, with the journal's `err` prefix.
//!
//! # And one thing it does for anybody
//!
//! `print relax-line <user> <until|no-limit>` prints the window's sudoers
//! line and touches nothing — so `relax-line-parses` can put the formatter's
//! real output through `visudo` at build time.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use ronoctua_security::firewall::{self, Known, Places};
use ronoctua_security::fprint;
use ronoctua_security::relax::{self, Window};
use ronoctua_security::tools::{self, Tools};
use ronoctua_security::u2f;
use ronoctua_security::users::{self, Person};
use ronoctua_security::verb::{RootVerb, Said, UnitVerb};
use ronoctua_security::{Refusal, Why};

/// The file manager's privileged unit, which keeps its own hardening.
const FILES_ROOT_UNIT: &str = "ronoctua-files-root.service";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("print") {
        return print(&args[1..]);
    }
    let verdict = match run(&args) {
        Ok(()) => Said::Done,
        Err(refusal) => Said::Refused(refusal.why),
    };
    say(&verdict);
    if verdict == Said::Done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// One line on standard output, now (the door streams them).
fn say(said: &Said) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", said.line());
    let _ = out.flush();
}

const fn refused(why: Why) -> Refusal {
    Refusal { why }
}

fn run(args: &[String]) -> Result<(), Refusal> {
    if !rustix::process::geteuid().is_root() {
        eprintln!("<3>ronoctua-security-root runs as root, through sudo or a unit");
        return Err(refused(Why::NotAllowed));
    }
    let person = users::invoking()?;
    let tools = Tools::baked();
    if let Some(verb) = RootVerb::parse(args) {
        return root_verb(verb, person.as_ref(), &tools);
    }
    if let Some(verb) = UnitVerb::parse(args) {
        if person.is_some() {
            eprintln!("<3>a unit's verb, asked through sudo");
            return Err(refused(Why::NotAllowed));
        }
        return unit_verb(verb, &tools);
    }
    eprintln!("<3>not a verb: {args:?}");
    Err(refused(Why::Invalid))
}

fn root_verb(verb: RootVerb, person: Option<&Person>, tools: &Tools) -> Result<(), Refusal> {
    let person = match person {
        Some(person) => Some(person),
        None if verb.needs_a_person() => {
            eprintln!("<3>{verb:?} acts for a person, and sudo named none");
            return Err(refused(Why::NotAllowed));
        }
        None => None,
    };
    let named = || person.ok_or(refused(Why::NotAllowed));
    match verb {
        RootVerb::Relax { span } => {
            let helper = std::env::current_exe().map_err(|_| refused(Why::Failed))?;
            relax::open(&Window::system(), tools, &named()?.name, span, &helper).map(|_| ())
        }
        RootVerb::RelaxEnd => {
            named()?;
            relax::close(&Window::system(), tools)
        }
        RootVerb::FirewallApply => match person {
            Some(person) => {
                firewall::apply_staged(&Places::system(), tools, person.uid, known().as_ref())
            }
            None => firewall::apply_stored(&Places::system(), tools),
        },
        RootVerb::FingerEnroll { finger } => fprint::enroll(&named()?.name, finger, &mut |step| {
            say(&Said::Step(step.word().to_owned()));
        }),
        RootVerb::FingerDelete { which } => fprint::delete(&named()?.name, which),
        RootVerb::FingerSudo { on } => {
            fprint::set_sudo(Path::new(fprint::SUDO_LIST), &named()?.name, on)
        }
        RootVerb::FilesRoot => {
            named()?;
            let systemctl = tools::given(tools.systemctl.as_ref())?;
            run_program(systemctl, &["start", FILES_ROOT_UNIT])
        }
        // THE 2026-10-05 WAVE'S VERBS: each will start one fixed unit that
        // reads a request the person staged. Until its package lands, each
        // refuses — and says so — without touching anything.
        // ── anchor: L06 ──
        // THE PERSON'S BRAIN MODULE, APPROVED: the module and the hash the
        // door was given (its own words, checked by `RootVerb::parse`) are
        // pinned in ROOT'S folder first — the person's approval folder is
        // any of their processes', and a request swapped there while sudo
        // read the password would be approved in place of the one shown.
        // Then the one unit starts: it holds the staged request to the pin,
        // recomputes the hash with `nix/person-hash.nix` and records the
        // approval only at the pinned one (`ronoctua_person::approve`). No
        // polkit rule names it. Its failure is said here as `failed`; WHY
        // (the module changed, no request, busy…) is in the unit's verdict,
        // `ronoctua_person::root::APPROVE_VERDICT`, which the view reads.
        RootVerb::PersonModuleApprove { id, hash } => {
            named()?;
            pin(Path::new(ronoctua_person::root::APPROVE_PIN), &id, &hash)?;
            let systemctl = tools::given(tools.systemctl.as_ref())?;
            run_program(systemctl, &["start", ronoctua_person::root::APPROVE_UNIT])
        }
        // THE KEYS THAT MAY ANSWER FOR THE PASSWORD: root's own list, the
        // invoking person's line only (`ronoctua_security::u2f`). Changing
        // how root authenticates asks the password; the person's `u2f_keys`
        // is never what PAM reads.
        RootVerb::U2fKeys { credentials } => {
            u2f::set_keys(Path::new(u2f::AUTHFILE), &named()?.name, &credentials)
        }
        // THE PERSON'S SIGNING KEY, PINNED: the key the door was given (its
        // own word, checked by `RootVerb::parse`) goes to ROOT'S folder first
        // — the person's folders are any of their processes' — and the one
        // unit that writes `local/signer.json` (`ronoctua-person pin`)
        // consumes it. No polkit rule names it; only `ronoctua-person`
        // writes the machine directory's `local/` (`one-person-boundary`).
        RootVerb::PersonSignerPin { key } => {
            named()?;
            pin_text(
                Path::new(ronoctua_person::root::SIGNER_PIN),
                &format!("{key}\n"),
            )?;
            let systemctl = tools::given(tools.systemctl.as_ref())?;
            run_program(systemctl, &["start", ronoctua_person::root::PIN_UNIT])
        }
        // ── anchor: L12 ──
        // THE VAULT'S UNIT, which reads the one request the person staged
        // and keeps its own verdict (why it refused) for the door to say.
        // SAID FIRST, once sudo let the person through: the unit starts now,
        // and a key opening waits for a touch from here on — the door says
        // "touch your key" at this word, never before a password question.
        RootVerb::PrivateRoot => {
            named()?;
            let systemctl = tools::given(tools.systemctl.as_ref())?;
            say(&Said::Step(ronoctua_private::STARTED.to_owned()));
            run_program(systemctl, &["start", ronoctua_private::ROOT_UNIT])
        }
        // ── anchor: L17 ──
        // THE INSTALLER STICK: start the writer's fixed unit and nothing else.
        // It waits for the person's staged `ready` and validates everything
        // it reads; `Type=exec`, so this returns once it runs, never after
        // the write (`nix/modules/stick.nix`).
        RootVerb::StickWrite => {
            const STICK_WRITE_UNIT: &str = "ronoctua-stick-write.service";
            named()?;
            let systemctl = tools::given(tools.systemctl.as_ref())?;
            run_program(systemctl, &["start", STICK_WRITE_UNIT])
        }
        // THE 2026-10-11 WAVE: root's netrc for the person's PRIVATE binary
        // cache — the token taken from the copy the gesture staged in the
        // person's own runtime folder (never a word of the verb), the copy
        // deleted, the netrc written `0600` from its first byte.
        RootVerb::BinaryCacheNetrc { name } => {
            let person = named()?;
            let runtime = ronoctua_cachix::token::runtime_of(person.uid);
            let token = ronoctua_cachix::netrc::take_staged(&runtime, person.uid)
                .map_err(|w| refused(cachix_why(w)))?;
            ronoctua_cachix::netrc::write(Path::new(ronoctua_cachix::NETRC), &name, &token)
                .map_err(|w| refused(cachix_why(w)))
        }
        RootVerb::BinaryCacheNetrcRemove => {
            named()?;
            ronoctua_cachix::netrc::remove(Path::new(ronoctua_cachix::NETRC))
                .map_err(|w| refused(cachix_why(w)))
        }
    }
}

/// What the cache's refusal is, in the door's closed set: what was staged
/// is not there or not a token — `invalid`; not the person's own private
/// file — `not-allowed`; the netrc not written — `failed`.
fn cachix_why(why: ronoctua_cachix::Why) -> Why {
    eprintln!("<3>the binary cache's netrc: {why}");
    match why {
        ronoctua_cachix::Why::TokenOpen => Why::NotAllowed,
        ronoctua_cachix::Why::NoToken
        | ronoctua_cachix::Why::NotAToken
        | ronoctua_cachix::Why::NoCache => Why::Invalid,
        _ => Why::Failed,
    }
}

/// Pins what the sudo door was given for an approval in root's folder, in
/// `ronoctua_person`'s one format ([`pin_text`]).
fn pin(path: &Path, id: &str, hash: &str) -> Result<(), Refusal> {
    pin_text(path, &ronoctua_person::approve::pin_text(id, hash))
}

/// Writes `text` as root's pin at `path`: the old pin removed, the new one
/// created `0600` (never through a link: `create_new` refuses one).
fn pin_text(path: &Path, text: &str) -> Result<(), Refusal> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            eprintln!("<3>{}: {e}", path.display());
            return Err(refused(Why::Failed));
        }
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .map_err(|e| {
            eprintln!("<3>{} could not be written: {e}", path.display());
            refused(Why::Failed)
        })
}

fn unit_verb(verb: UnitVerb, tools: &Tools) -> Result<(), Refusal> {
    match verb {
        UnitVerb::Link {
            interface,
            connection,
            up,
        } => firewall::link(
            &Places::system(),
            tools,
            &interface,
            &connection,
            up,
            known().as_ref(),
        ),
        UnitVerb::ShieldsUp => firewall::shields_up(&Places::system(), tools),
        UnitVerb::RelaxExpire { until_unix_s } => relax::expire(&Window::system(), until_unix_s),
        UnitVerb::RelaxBoot => {
            let helper = std::env::current_exe().map_err(|_| refused(Why::Failed))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            let at_boot = relax::boot(&Window::system(), tools, &helper, now)?;
            if at_boot != relax::AtBoot::Nothing {
                eprintln!("<6>the passwordless window at boot: {at_boot:?}");
            }
            Ok(())
        }
    }
}

/// What NetworkManager says of its stored connections — asked through
/// `ronoctua-network`, the one crate that talks to it, and never read from
/// its files. `None`, said in the journal, when it does not answer: then
/// nothing is placed, and no network is taken for one this machine serves.
fn known() -> Option<Known> {
    match ronoctua_network::Manager::fresh().and_then(|nm| nm.stored()) {
        Ok(stored) => Some(Known::of(
            stored
                .into_iter()
                .map(|profile| (profile.uuid, profile.serves)),
        )),
        Err(e) => {
            eprintln!("<4>NetworkManager did not answer: {e:#}");
            None
        }
    }
}

/// Runs a program with no environment of ours; its failure is `Failed`.
fn run_program(program: &Path, args: &[&str]) -> Result<(), Refusal> {
    let status = std::process::Command::new(program)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|_| refused(Why::Unstartable))?;
    if status.success() {
        Ok(())
    } else {
        eprintln!(
            "<3>{} {} failed ({status})",
            program.display(),
            args.join(" ")
        );
        Err(refused(Why::Failed))
    }
}

/// `print relax-line <user> <until|no-limit>`: the line, and nothing
/// touched.
fn print(args: &[String]) -> ExitCode {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let line = match words.as_slice() {
        ["relax-line", user, "no-limit"] => relax::sudoers_line(user, None).ok(),
        ["relax-line", user, until] => until
            .parse::<i64>()
            .ok()
            .and_then(|until| relax::sudoers_line(user, Some(until)).ok()),
        _ => None,
    };
    match line {
        Some(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        None => {
            eprintln!(
                "usage: ronoctua-security-root print relax-line <user> <until-unix-seconds|no-limit>"
            );
            ExitCode::FAILURE
        }
    }
}
