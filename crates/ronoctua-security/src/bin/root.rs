//! `ronoctua-security-root` — the one program the security centre runs as
//! root.
//!
//! # Who runs it
//!
//! - **The sudo door** (`ronoctua_security::authority`), for a person's
//!   gesture: `sudo … -- ronoctua-security-root <RootVerb>`. sudo names the
//!   person in `SUDO_UID`; uid 0 is refused.
//! - **Units and NetworkManager's dispatcher**, with no `SUDO_UID`:
//!   `firewall-apply` (the boot's table), `relax-end` (the expiry timer, the
//!   sleep hook), `shields-up` and `firewall-apply --link …`. A unit verb
//!   that arrives through sudo is refused: the door's set stays the door's.
//!
//! It refuses to run as anybody but root, and does nothing a verb does not
//! name. Its last line on standard output is the verdict (`done` or
//! `refused <why>`), after any progress (`step <word>`); what went wrong in
//! detail goes to standard error, with the journal's `err` prefix.
//!
//! # And one thing it does for anybody
//!
//! `print relax-line <user> <until>` prints the window's sudoers line and
//! touches nothing — so `relax-line-parses` can put the formatter's real
//! output through `visudo` at build time.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use ronoctua_security::firewall::{self, Known, Places};
use ronoctua_security::fprint;
use ronoctua_security::relax::{self, Window};
use ronoctua_security::tools::{self, Tools};
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
        RootVerb::Relax { minutes } => {
            let helper = std::env::current_exe().map_err(|_| refused(Why::Failed))?;
            relax::open(
                &Window::system(),
                tools,
                &named()?.name,
                minutes.minutes(),
                &helper,
            )
            .map(|_| ())
        }
        RootVerb::RelaxEnd => relax::close(&Window::system(), tools),
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
    }
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

/// `print relax-line <user> <until>`: the line, and nothing touched.
fn print(args: &[String]) -> ExitCode {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let line = match words.as_slice() {
        ["relax-line", user, until] => until
            .parse::<i64>()
            .ok()
            .and_then(|until| relax::sudoers_line(user, until).ok()),
        _ => None,
    };
    match line {
        Some(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("usage: ronoctua-security-root print relax-line <user> <until-unix-seconds>");
            ExitCode::FAILURE
        }
    }
}
