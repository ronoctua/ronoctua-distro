//! `ronoctua-vscode record|install` — VS Code's extensions and the person's
//! list of them (`ronoctua_vscode`).
//!
//! `record` is what the user path unit runs when VS Code's manifest (or the
//! list) changes; its line on standard output is `nothing here`,
//! `unchanged` or `recorded <n>`. `install` is the person's, from a
//! terminal (the belongings' offer runs the same function through the
//! queue): a line per id, `entered <id>` or `failed <id>: <what code said>`,
//! then `entered <n>, failed <m>`. Both say `refused <why>` on standard
//! error when they cannot start, and exit `1` — `install` naming the hosts
//! to let when consent is why; `install` exits `0` only when every missing
//! extension entered; a command line it does not take is `2`.

use std::path::PathBuf;
use std::process::ExitCode;

use ronoctua_vscode::install::{self, Event};
use ronoctua_vscode::{Recorded, Refusal, Why};

/// The usage, in English (invariant 18: a command line's help).
const USAGE: &str = "usage: ronoctua-vscode record
       ronoctua-vscode install
record   writes what changed in VS Code's Marketplace extensions here
         (~/.vscode/extensions/extensions.json, the default profile) into your
         list (~/.config/Code/User/ronoctua-extensions.json), which travels with
         your belongings: one installed here since the last look enters it, one
         uninstalled here leaves it, and nothing else moves it — another
         machine's ids stay, and an id another machine took out stays out.
         With nothing installed here, nothing is written.
install  installs the extensions of your list that are not installed here, one at
         a time, with VS Code's own `code --install-extension` (from the Visual
         Studio Marketplace), and says which entered and which failed. It asks
         nothing unless every Marketplace host it reaches is let in your
         consent.json — the belongings page lets them when you install from
         there — and it exits 0 only when every one entered.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["record"] => record(),
        ["install"] => install(),
        ["-h" | "--help"] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The person's home.
fn home() -> Result<PathBuf, Refusal> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or(Refusal::of(Why::NoHome))
}

fn refused(refusal: &Refusal) -> ExitCode {
    eprintln!("ronoctua-vscode: refused {refusal}");
    if refusal.why == Why::NotLet {
        eprintln!(
            "ronoctua-vscode: these hosts are not let in your consent.json, and nothing was asked:"
        );
        for host in &refusal.hosts {
            eprintln!("  {host}");
        }
        eprintln!(
            "ronoctua-vscode: install from the belongings page (it lets them), or let them there first"
        );
    }
    ExitCode::FAILURE
}

fn record() -> ExitCode {
    let done =
        home().and_then(|home| ronoctua_vscode::record(&home, &ronoctua_vscode::seen_store()));
    match done {
        Ok(Recorded::NothingHere) => println!("nothing here"),
        Ok(Recorded::Unchanged) => println!("unchanged"),
        Ok(Recorded::Written(n)) => println!("recorded {n}"),
        Err(r) => return refused(&r),
    }
    ExitCode::SUCCESS
}

fn install() -> ExitCode {
    let Some(code) = ronoctua_vscode::code() else {
        return refused(&Refusal::of(Why::NoCode));
    };
    let home = match home() {
        Ok(home) => home,
        Err(r) => return refused(&r),
    };
    let done = install::install(
        &code,
        &home,
        &|host| ronoctua_consent::pass(host).ok(),
        &mut |e| match e {
            Event::Trying { .. } => {}
            Event::Entered(id) => println!("entered {id}"),
            Event::Failed(id, said) => println!("failed {id}: {said}"),
        },
    );
    match done {
        Ok(done) => {
            println!(
                "entered {}, failed {}",
                done.entered.len(),
                done.failed.len()
            );
            if done.whole() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(r) => refused(&r),
    }
}
