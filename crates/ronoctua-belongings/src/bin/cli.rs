//! `ronoctua-belongings <order> [--progress]` (`order::Op`'s words) —
//! what the transfer queue runs as a child for `Work::Belongings` — and the
//! verbs other packages run with flags:
//!
//! ```text
//! restore --home <dir> --runtime <dir> --machine <host> [--repository <url> [--user <name>]] [--progress]
//! init [--home <dir> --runtime <dir> --machine <host> [--repository <url> [--user <name>]]] [--progress]
//! verify-passphrase [--home <dir> --runtime <dir> [--repository <url> [--user <name>]]] [--json]
//! probe --address <url> --credential <file> [--user <name>] [--known-hosts <file>] [--json]
//! plain pull|push [--json]                              the plain part alone (the update, the machine)
//! plain --repository <url> --keys <dir> --out <dir> [--user <name>] [--passphrase <file> --signer <file> [--decline <file>]] [--json]
//! take --home <dir> --runtime <dir> --path <logical> --out <file> [--repository <url> [--user <name>]] [--json]
//! sign [--json]                                         this machine's plain part signed here, no network
//! forget [--everywhere] [--progress]                    this instance's kept identity; every one's
//! ```
//!
//! `plain … --decline <file>` says the installer's choice for the machine it
//! installs (`ronoctua_person::choice::Choice`: the programs left out, and
//! those kept) in the repository's `declined.json`, signs it with the key the
//! passphrase opens and pushes it before the copy — only with `--passphrase`:
//! a decline is signed, or not made.
//!
//! `sign` with `--progress` is the queue's order (`Op::Sign`); without it, its
//! last line is `signed <counter>`, `unchanged` or `refused <why>` — what the
//! update's step reads (a locked identity, `refused no-passphrase`, is news
//! there, not a failure).
//!
//! Everything a sync needs is where both sides know to look
//! (`ronoctua_belongings::places`): the settings in `belongings.json`, the
//! passphrase in the `0600` runtime file a door (or the installer) wrote, the
//! decisions beside the plan.
//!
//! A HOME BEING MADE (`--home`, the installer's) has no settings yet, and the
//! installer is not the machine it installs: there the repository is named
//! with `--repository` (its access is the address's kind, from that home's
//! `belongings-keys/`) — without it `belongings.json` of that home must
//! already exist — and `restore` and `init` take the new machine's name
//! (`--machine`, the hostname of its `host.nix`), which the index and
//! `fleet.json` remember instead of the live system's.
//!
//! Nothing secret is an argument: a flag names a FILE (`--credential`,
//! `--keys`), never its content. With `--progress` it prints the queue's
//! lines (`ronoctua_belongings::order::Beat`) on stdout; with `--json` one
//! line of JSON, the answer — `{"refused":"<why>"}` when there is none. It
//! exits `0` when it did what it was asked (a sync waiting for decisions
//! included), and `2` for a command line it does not take.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ronoctua_belongings::Why;
use ronoctua_belongings::engine::{self, Initialized, Named};
use ronoctua_belongings::order::{Beat, Op};
use ronoctua_belongings::places::Places;

/// The usage, in English (invariant 18: a command line's help).
const USAGE: &str = "usage: ronoctua-belongings <prepare|apply|abandon|keygen|rekey|compact|sync-plain|forget|restore|probe|init|verify-passphrase|sign|vscode-extensions> [--progress]
       ronoctua-belongings forget [--everywhere] [--progress]
       ronoctua-belongings restore --home <dir> --runtime <dir> --machine <host> [--repository <url> [--user <name>]] [--progress]
       ronoctua-belongings init [--home <dir> --runtime <dir> --machine <host> [--repository <url> [--user <name>]]] [--progress]
       ronoctua-belongings verify-passphrase [--home <dir> --runtime <dir> [--repository <url> [--user <name>]]] [--json]
       ronoctua-belongings probe --address <url> --credential <file> [--user <name>] [--known-hosts <file>] [--json]
       ronoctua-belongings plain pull|push [--json]
       ronoctua-belongings plain --repository <url> --keys <dir> --out <dir> [--user <name>] [--passphrase <file> --signer <file> [--decline <file>]] [--json]
       ronoctua-belongings take --home <dir> --runtime <dir> --path <logical> --out <file> [--repository <url> [--user <name>]] [--json]
       ronoctua-belongings sign [--json]
take writes ONE file of the sealed part at <file>, 0600 (the installer's, before nixos-install).
plain --decline <file> (the installer's choice for one machine: the programs left out, and those kept) says it in the repository's declined.json, signs it with the key the passphrase opens and pushes it before copying.
With --home and no --repository, <home>/.config/ronoctua/belongings.json must name the repository.
forget takes this instance's kept identity; --everywhere, every instance's (the end of a session, a sleep).
vscode-extensions installs the VS Code extensions of your list that are missing here (the yes to the offer).";

/// The flags that take a value.
const VALUED: [&str; 14] = [
    "--home",
    "--runtime",
    "--address",
    "--credential",
    "--user",
    "--known-hosts",
    "--repository",
    "--keys",
    "--out",
    "--machine",
    "--path",
    "--passphrase",
    "--signer",
    "--decline",
];

/// A command line, read: its words, its switches and its flags' values.
#[derive(Debug, Default, PartialEq, Eq)]
struct Line {
    words: Vec<String>,
    progress: bool,
    json: bool,
    /// `forget --everywhere`: every instance's identity, not this one's.
    everywhere: bool,
    values: Vec<(String, String)>,
}

impl Line {
    /// The command line, or `None` when it is not one this program takes (a
    /// flag twice, a flag without its value, one it does not know).
    fn read(args: &[String]) -> Option<Self> {
        let mut line = Self::default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--progress" => line.progress = true,
                "--json" => line.json = true,
                "--everywhere" => line.everywhere = true,
                flag if VALUED.contains(&flag) => {
                    let value = it.next()?;
                    if value.is_empty() || line.value(flag).is_some() {
                        return None;
                    }
                    line.values.push((flag.to_owned(), value.clone()));
                }
                flag if flag.starts_with("--") => return None,
                word => line.words.push(word.to_owned()),
            }
        }
        Some(line)
    }

    fn value(&self, flag: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(f, _)| f == flag)
            .map(|(_, v)| v.as_str())
    }

    fn path(&self, flag: &str) -> Option<PathBuf> {
        self.value(flag).map(PathBuf::from)
    }

    /// Whether every flag given is one of `allowed`.
    fn only(&self, allowed: &[&str]) -> bool {
        self.values
            .iter()
            .all(|(f, _)| allowed.contains(&f.as_str()))
    }
}

/// The flags an order takes: the installer's verbs take a home and what it
/// names; every other order, none.
fn flags_of(op: Op) -> &'static [&'static str] {
    match op {
        Op::Restore | Op::Init => &["--home", "--runtime", "--repository", "--user", "--machine"],
        Op::VerifyPassphrase => &["--home", "--runtime", "--repository", "--user"],
        _ => &[],
    }
}

/// What the installer names for `op`, or `None` when the line breaks a rule:
/// what it names is for a home being made, so only beside `--home`; a user
/// only beside the repository it is for; and where the index or
/// `fleet.json` remembers the machine (`restore`, `init`) its name is
/// required — else the live system's would be — and must be a hostname.
fn named_of(op: Op, line: &Line) -> Option<Named> {
    let named = Named {
        repository: line.value("--repository").map(str::to_owned),
        user: line.value("--user").map(str::to_owned),
        machine: line.value("--machine").map(str::to_owned),
    };
    let home = line.value("--home").is_some();
    let names_any = named.repository.is_some() || named.user.is_some() || named.machine.is_some();
    if !line.only(flags_of(op))
        || (!home && names_any)
        || (named.user.is_some() && named.repository.is_none())
        || (home && matches!(op, Op::Restore | Op::Init) && named.machine.is_none())
        || named
            .machine
            .as_deref()
            .is_some_and(|m| !ronoctua_person::is_hostname(m))
    {
        return None;
    }
    Some(named)
}

/// What a verb answers with `--json`.
fn print_json<T: serde::Serialize>(value: &T) {
    let mut out = std::io::stdout().lock();
    if let Ok(text) = serde_json::to_string(value) {
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }
}

/// A refusal, said: on stderr always, as JSON on stdout with `--json`.
fn refused(verb: &str, why: Why, json: bool) -> ExitCode {
    eprintln!("ronoctua-belongings {verb}: {}", why.id());
    if json {
        print_json(&serde_json::json!({ "refused": why.id() }));
    }
    ExitCode::FAILURE
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(line) = Line::read(&args) else {
        return usage();
    };
    let words: Vec<&str> = line.words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["plain", "pull" | "push"] if line.only(&[]) => plain(&line, words[1] == "push"),
        ["plain"]
            if line.only(&[
                "--repository",
                "--keys",
                "--out",
                "--user",
                "--passphrase",
                "--signer",
                "--decline",
            ]) =>
        {
            plain_out(&line)
        }
        ["sign"] if !line.progress && line.only(&[]) => sign(&line),
        ["probe"] if line.value("--address").is_some() => probe(&line),
        ["take"] => take(&line),
        [word] => match Op::of_word(word) {
            Some(op) => order(op, &line),
            None => usage(),
        },
        _ => usage(),
    }
}

/// The places of this process — or of the home `--home` names, with the
/// runtime folder `--runtime` names (the installer's).
fn places_of(line: &Line) -> Result<Places, ExitCode> {
    match (line.path("--home"), line.path("--runtime")) {
        (Some(home), Some(runtime)) => Ok(Places::for_home(&home, &runtime)),
        (None, None) => Places::from_env().ok_or_else(|| {
            eprintln!(
                "ronoctua-belongings: no home, no configuration root or no runtime directory"
            );
            ExitCode::FAILURE
        }),
        _ => Err(usage()),
    }
}

/// An order of the queue's, with a home and what the installer names where
/// it takes them.
fn order(op: Op, line: &Line) -> ExitCode {
    let Some(named) = named_of(op, line) else {
        return usage();
    };
    if line.everywhere && op != Op::Forget {
        return usage();
    }
    // FORGETTING NEEDS NO HOME, no settings and no runtime variable: the
    // system unit that runs it as the person before a sleep has none of
    // them. This instance's identity goes, and its marker — the queue's
    // order, which the development loop's copy runs too; `--everywhere`
    // (the end of a session, a sleep) takes every instance's.
    if op == Op::Forget {
        let _ = if line.everywhere {
            engine::forget_everywhere()
        } else {
            engine::forget_here()
        };
        if line.progress {
            println!("{}", Beat::Done { conflicts: 0 }.line());
        }
        return ExitCode::SUCCESS;
    }
    let places = match places_of(line) {
        Ok(places) => places,
        Err(code) => {
            if line.progress {
                println!("{}", Beat::Failed(Why::Failed).line());
            }
            return code;
        }
    };
    if op == Op::VerifyPassphrase && line.json {
        return match engine::format_opened(&places, &named, &mut |_| {}) {
            Ok(format) => {
                print_json(&serde_json::json!({ "format": format }));
                ExitCode::SUCCESS
            }
            Err(r) => refused(op.word(), r.why, true),
        };
    }
    let mut out = std::io::stdout().lock();
    let progress = line.progress;
    let mut beat = |b: Beat| {
        if progress {
            let _ = writeln!(out, "{}", b.line());
            let _ = out.flush();
        }
    };
    // EVERYTHING ELSE IS THE LIBRARY'S (`engine::order` and the installer's
    // verbs), where a test reaches it: which refusal deletes what, and under
    // which lock.
    let done = match op {
        Op::Restore => engine::restore_named(&places, &named, &mut beat),
        Op::Init => engine::init_named(&places, &named, &mut beat).map(|i| match i {
            Initialized::Made(engine::Done::Waiting(plan)) => {
                u32::try_from(plan.conflicts.len()).unwrap_or(u32::MAX)
            }
            Initialized::Made(engine::Done::Synced(_)) | Initialized::Verified { .. } => 0,
        }),
        Op::VerifyPassphrase => engine::format_opened(&places, &named, &mut beat).map(|_| 0),
        _ => engine::order(op, &places, &mut beat),
    };
    match done {
        Ok(conflicts) => {
            beat(Beat::Done { conflicts });
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            eprintln!("ronoctua-belongings {}: {refusal}", op.word());
            beat(Beat::Failed(refusal.why));
            ExitCode::FAILURE
        }
    }
}

/// `plain pull` / `plain push`.
fn plain(line: &Line, push: bool) -> ExitCode {
    let verb = if push { "plain push" } else { "plain pull" };
    let places = match places_of(line) {
        Ok(places) => places,
        Err(code) => return code,
    };
    match engine::plain_verb(&places, push) {
        Ok(report) => {
            if line.json {
                print_json(&report);
            }
            ExitCode::SUCCESS
        }
        Err(r) => refused(verb, r.why, line.json),
    }
}

/// `plain --repository <url> --keys <dir> --out <dir>`.
fn plain_out(line: &Line) -> ExitCode {
    let (Some(address), Some(keys), Some(out)) = (
        line.value("--repository"),
        line.path("--keys"),
        line.path("--out"),
    ) else {
        return usage();
    };
    let Some(opening) = opening_of(line) else {
        return usage();
    };
    let opened = engine::Opened {
        passphrase: opening.passphrase.as_deref(),
        signer: opening.signer.as_deref(),
        choice: opening.choice.as_deref(),
    };
    match engine::plain_out(address, &keys, &out, line.value("--user"), opened) {
        Ok(report) => {
            if line.json {
                print_json(&report);
            }
            ExitCode::SUCCESS
        }
        Err(r) => refused("plain", r.why, line.json),
    }
}

/// What `plain` opens, as files.
#[derive(Debug, PartialEq, Eq)]
struct Opening {
    passphrase: Option<PathBuf>,
    signer: Option<PathBuf>,
    /// `--decline`: the installer's choice.
    choice: Option<PathBuf>,
}

/// What `plain` opens, or `None` for a line that breaks a rule: the
/// passphrase and where its key goes come together or not at all (a key
/// opened and left nowhere would be a check the installer cannot pin), and
/// a choice only with the passphrase — a decline is signed with the key the
/// passphrase opens, or not made.
fn opening_of(line: &Line) -> Option<Opening> {
    let opening = Opening {
        passphrase: line.path("--passphrase"),
        signer: line.path("--signer"),
        choice: line.path("--decline"),
    };
    if opening.passphrase.is_some() != opening.signer.is_some()
        || (opening.choice.is_some() && opening.passphrase.is_none())
    {
        return None;
    }
    Some(opening)
}

/// `take --home <dir> --runtime <dir> --path <logical> --out <file>`: one
/// file of the sealed part, for the installer. Exits `0` when it was taken,
/// and when the repository keeps no such file (`{"taken": false}`).
fn take(line: &Line) -> ExitCode {
    if !line.only(&[
        "--home",
        "--runtime",
        "--repository",
        "--user",
        "--path",
        "--out",
    ]) || line.value("--home").is_none()
        || (line.value("--user").is_some() && line.value("--repository").is_none())
    {
        return usage();
    }
    let (Some(logical), Some(out)) = (line.value("--path"), line.path("--out")) else {
        return usage();
    };
    let places = match places_of(line) {
        Ok(places) => places,
        Err(code) => return code,
    };
    let named = Named {
        repository: line.value("--repository").map(str::to_owned),
        user: line.value("--user").map(str::to_owned),
        machine: None,
    };
    match engine::take_named(&places, &named, logical, &out, &mut |_| {}) {
        Ok(taken) => {
            if line.json {
                print_json(&serde_json::json!({ "taken": taken }));
            }
            ExitCode::SUCCESS
        }
        Err(r) => refused("take", r.why, line.json),
    }
}

/// `sign`: this machine's plain part, signed here — `signed <counter>` or
/// `unchanged` as the last line (or one line of JSON with `--json`), and
/// `refused <why>` when it was not.
fn sign(line: &Line) -> ExitCode {
    let places = match places_of(line) {
        Ok(places) => places,
        Err(code) => return code,
    };
    match engine::sign_verb(&places) {
        Ok(signed) => {
            if line.json {
                print_json(&signed);
            } else {
                match signed {
                    engine::SignedHere::Signed { counter } => println!("signed {counter}"),
                    engine::SignedHere::Unchanged => println!("unchanged"),
                }
            }
            ExitCode::SUCCESS
        }
        Err(r) => {
            if !line.json {
                println!("refused {}", r.why.id());
            }
            refused("sign", r.why, line.json)
        }
    }
}

/// `probe --address <url> --credential <file>`: exits `0` only when the
/// access both reads and writes.
fn probe(line: &Line) -> ExitCode {
    if !line.only(&["--address", "--credential", "--user", "--known-hosts"]) {
        return usage();
    }
    let (Some(address), Some(credential)) = (line.value("--address"), line.path("--credential"))
    else {
        return usage();
    };
    let places = match places_of(line) {
        Ok(places) => places,
        Err(code) => return code,
    };
    let known_hosts = line.path("--known-hosts");
    match engine::probe_address(
        &places,
        address,
        &credential,
        line.value("--user"),
        known_hosts.as_deref().map(Path::new),
    ) {
        Ok(probed) => {
            if line.json {
                print_json(&probed);
            }
            if probed.read && probed.write {
                ExitCode::SUCCESS
            } else {
                eprintln!(
                    "ronoctua-belongings probe: {}",
                    probed.why.map_or("refused", Why::id)
                );
                ExitCode::FAILURE
            }
        }
        Err(r) => refused("probe", r.why, line.json),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(args: &[&str]) -> Option<Line> {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        Line::read(&args)
    }

    /// FAILS IF: the command line takes a flag twice, a flag with no value
    /// or an empty one, or a flag it does not know — or loses a word, a
    /// switch or a value.
    #[test]
    fn a_command_line_is_read_whole_or_refused() {
        let read = line(&[
            "restore",
            "--home",
            "/mnt/home/someone",
            "--progress",
            "--runtime",
            "/run/i",
            "--json",
        ])
        .unwrap();
        assert_eq!(read.words, ["restore"]);
        assert!(read.progress && read.json);
        assert_eq!(read.value("--home"), Some("/mnt/home/someone"));
        assert_eq!(read.path("--runtime"), Some(PathBuf::from("/run/i")));
        assert_eq!(read.value("--out"), None);
        assert!(read.only(&["--home", "--runtime"]));
        assert!(!read.only(&["--home"]));
        for bad in [
            &["restore", "--home", "/a", "--home", "/b"][..],
            &["restore", "--home"][..],
            &["restore", "--home", ""][..],
            &["restore", "--secret", "x"][..],
            &["restore", "--force"][..],
        ] {
            assert_eq!(line(bad), None, "{bad:?}");
        }
        // `--passphrase` names a FILE, and only the installer's copy of the
        // plain part takes one: no order of the queue's does.
        let with_file = line(&["restore", "--passphrase", "/run/i/p"]).unwrap();
        assert_eq!(named_of(Op::Restore, &with_file), None);
        assert_eq!(
            line(&["plain", "pull", "--json"]).unwrap().words,
            ["plain", "pull"]
        );
        // `forget` takes this instance's; `--everywhere`, every one's.
        assert!(!line(&["forget"]).unwrap().everywhere);
        assert!(line(&["forget", "--everywhere"]).unwrap().everywhere);
    }

    /// FAILS IF: what the installer names reaches a run where it does not
    /// belong — without a home, a user without its repository, another
    /// order's flag — or `restore`/`init` into a home go without the new
    /// machine's name (the index would remember the live system's), or a
    /// name that is not a hostname is taken.
    #[test]
    fn what_the_installer_names_follows_the_rules() {
        let named = |op: Op, args: &[&str]| line(args).and_then(|l| named_of(op, &l));
        let home = ["x", "--home", "/mnt/home/s", "--runtime", "/run/i"];
        let with = |extra: &[&'static str]| -> Vec<&'static str> {
            home.iter().copied().chain(extra.iter().copied()).collect()
        };
        assert_eq!(
            named(
                Op::Restore,
                &with(&[
                    "--machine",
                    "new-laptop",
                    "--repository",
                    "git@example.org:s/b.git"
                ])
            ),
            Some(Named {
                repository: Some("git@example.org:s/b.git".to_owned()),
                user: None,
                machine: Some("new-laptop".to_owned()),
            })
        );
        assert_eq!(named(Op::Restore, &home), None, "restore with no machine");
        assert_eq!(named(Op::Init, &home), None, "init with no machine");
        assert_eq!(
            named(Op::Init, &with(&["--machine", "Not_A_Host"])),
            None,
            "a machine that is not a hostname"
        );
        assert_eq!(
            named(Op::Restore, &with(&["--machine", "m", "--user", "me"])),
            None,
            "a user without its repository"
        );
        assert_eq!(
            named(Op::VerifyPassphrase, &with(&["--machine", "m"])),
            None,
            "verify remembers no machine"
        );
        assert_eq!(
            named(Op::VerifyPassphrase, &home),
            Some(Named::default()),
            "verify reads the home's settings"
        );
        assert_eq!(
            named(
                Op::Init,
                &["init", "--repository", "git@example.org:s/b.git"]
            ),
            None,
            "a repository named without a home"
        );
        assert_eq!(named(Op::Init, &["init"]), Some(Named::default()));
        assert_eq!(named(Op::Prepare, &with(&[])), None, "a sync takes no home");
    }

    /// FAILS IF: `plain` takes declines without the passphrase whose key
    /// signs them, a passphrase without where its key goes (or the other way
    /// round), or loses one of the three files it names.
    #[test]
    fn a_decline_comes_only_with_the_passphrase() {
        let opening = |args: &[&str]| {
            let base = ["plain", "--repository", "git@example.org:s/b.git"];
            line(&base.iter().chain(args).copied().collect::<Vec<_>>()).and_then(|l| opening_of(&l))
        };
        assert_eq!(
            opening(&["--decline", "/run/i/declines.json"]),
            None,
            "a decline with no passphrase"
        );
        assert_eq!(
            opening(&["--passphrase", "/run/i/p", "--decline", "/run/i/d"]),
            None,
            "a passphrase whose key goes nowhere"
        );
        assert_eq!(opening(&["--signer", "/run/i/s"]), None);
        assert_eq!(
            opening(&[
                "--passphrase",
                "/run/i/p",
                "--signer",
                "/run/i/s",
                "--decline",
                "/run/i/d"
            ]),
            Some(Opening {
                passphrase: Some(PathBuf::from("/run/i/p")),
                signer: Some(PathBuf::from("/run/i/s")),
                choice: Some(PathBuf::from("/run/i/d")),
            })
        );
        assert_eq!(
            opening(&[]),
            Some(Opening {
                passphrase: None,
                signer: None,
                choice: None,
            })
        );
        assert_eq!(
            opening(&["--decline", "/a", "--decline", "/b"]),
            None,
            "declines named twice"
        );
    }
}
