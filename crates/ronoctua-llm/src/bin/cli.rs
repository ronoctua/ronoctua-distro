//! `ronoctua-llm` — the LLM accounts' command line.
//!
//! - `statusline --profile <id>`: what Claude Code runs as the account's
//!   status line (ours, wrapped in front of the person's). Keeps the limits
//!   its stdin says, then chains the person's own line with the same stdin,
//!   passing its stdout and exit code through; with nothing to chain, says
//!   the account's windows itself. It never fails the person's line for a
//!   reason of ours.
//! - `refresh [--profile <id>]`: what the usage unit runs. Counts each
//!   account's transcripts (from where the last count stopped) and, for a
//!   Claude account, asks the usage endpoint when the person let its host —
//!   keeping both in the account's state. It asks nothing of anybody.
//! - `wrap --profile <id>` / `unwrap --profile <id>`: our status line in front
//!   of the account's, or the person's given back — what the view does, from
//!   a terminal.
//! - `remove --profile <id>`: the account forgotten (its folder stays), its
//!   own status line given back first.
//! - `hook`: what Claude Code runs on a session's events, by the distro's
//!   managed settings. Keeps the session's record; never prints, and always
//!   answers 0 — whatever its arguments, whatever its stdin.

use std::io::Read as _;

use ronoctua_llm::{Accounts, Roots, hook, refresh, statusline, usage};

const HELP: &str = "\
ronoctua-llm — the person's LLM accounts (Claude Code, Codex)

USAGE:
    ronoctua-llm statusline --profile <id>   run as Claude Code's status line:
                                             keep the limits, chain the
                                             person's own line
    ronoctua-llm refresh [--profile <id>]    count the transcripts and ask the
                                             usage endpoint (when consented)
    ronoctua-llm wrap --profile <id>         put our status line in front of
                                             the account's
    ronoctua-llm unwrap --profile <id>       give the account's own back
    ronoctua-llm remove --profile <id>       forget the account (its folder
                                             stays), giving its own status
                                             line back
    ronoctua-llm hook                        run as Claude Code's managed hook:
                                             keep the session the event on
                                             stdin is of; prints nothing,
                                             answers 0

The accounts are in $XDG_CONFIG_HOME/ronoctua/llm-accounts.json; the hook's
sessions in $XDG_RUNTIME_DIR/ronoctua-agents/.";

/// The value after `--profile`, when the arguments are exactly that (or
/// nothing, when `optional`).
fn profile(rest: &[String], optional: bool) -> Option<Option<String>> {
    match rest {
        [] if optional => Some(None),
        [flag, id] if flag == "--profile" => Some(Some(id.clone())),
        _ => None,
    }
}

fn main() {
    // THE HOOK FIRST, before anything that could fail aloud: its arguments
    // are not read (an argument it does not know must not answer 2, which
    // Claude Code takes for a blocking error), and an argument that is not
    // text must not panic in `env::args`.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|verb| verb == "hook")
    {
        std::process::exit(hook::serve());
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.split_first() {
        Some((verb, rest)) if verb == "statusline" => match profile(rest, false) {
            Some(Some(id)) => run_statusline(&id),
            _ => usage_error(),
        },
        Some((verb, rest)) if verb == "refresh" => match profile(rest, true) {
            Some(only) => run_refresh(only.as_deref()),
            None => usage_error(),
        },
        Some((verb, rest)) if verb == "wrap" || verb == "unwrap" => match profile(rest, false) {
            Some(Some(id)) => wrapping(&id, verb == "wrap"),
            _ => usage_error(),
        },
        Some((verb, rest)) if verb == "remove" => match profile(rest, false) {
            Some(Some(id)) => remove(&id),
            _ => usage_error(),
        },
        Some((verb, _)) if verb == "--help" || verb == "-h" || verb == "help" => {
            println!("{HELP}");
            0
        }
        _ => usage_error(),
    };
    std::process::exit(code);
}

fn usage_error() -> i32 {
    eprintln!("{HELP}");
    2
}

/// Claude Code's status line: never a failure of ours on the person's line.
fn run_statusline(id: &str) -> i32 {
    let mut input = Vec::new();
    if let Err(e) = std::io::stdin()
        .lock()
        .take(statusline::MOST_INPUT)
        .read_to_end(&mut input)
    {
        eprintln!("ronoctua-llm: the status line's input did not read: {e}");
    }
    let chain = statusline::Chain::of_env();
    let code = statusline::run(id, &input, &chain, std::process::Stdio::inherit());
    // NOBODY'S LINE TO CHAIN: ours says the windows (`statusline::said`) —
    // in the person's language, or nothing when the texts cannot be had:
    // the line is never failed for a reason of ours.
    if !chain.chained && !statusline::chains(&chain, id) {
        let keys = ronoctua_i18n::Keys::of(ronoctua_llm::key::sets);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let language = statusline::language(chain.roots.config.as_deref());
        if let Some(line) = ronoctua_i18n::Texts::checked(&language, &keys)
            .ok()
            .flatten()
            .and_then(|texts| statusline::said(&input, now, &texts))
        {
            println!("{line}");
        }
    }
    code
}

/// The usage unit's run: every account (or one), counted and asked — the
/// pass minted from the person's consent at the moment of asking.
fn run_refresh(only: Option<&str>) -> i32 {
    let done = refresh::run(
        &Roots::of_env(),
        only,
        &|| ronoctua_consent::pass(usage::HOST).ok(),
        &mut |pass, dir| usage::fetch(pass, dir),
    );
    let all = match done {
        Ok(all) => all,
        Err(r) => {
            eprintln!("<3>ronoctua-llm: nothing refreshed: {r}");
            return 1;
        }
    };
    for one in all {
        let id = &one.id;
        if let Err(r) = one.counted {
            eprintln!("<4>ronoctua-llm: {id}: not counted: {r}");
        }
        if let Some(Err(r)) = one.asked {
            eprintln!("<6>ronoctua-llm: {id}: usage not read: {r}");
        }
        if let Err(r) = one.kept {
            eprintln!("<4>ronoctua-llm: {id}: the usage was not kept: {r}");
        }
    }
    0
}

/// Forgets one account, giving its status line back first.
fn remove(id: &str) -> i32 {
    match Accounts::keep(|list| list.remove(id)) {
        Ok(_) => 0,
        Err(r) => {
            eprintln!("ronoctua-llm: {id}: refused: {r}");
            1
        }
    }
}

/// `wrap` or `unwrap` one account's status line.
fn wrapping(id: &str, wrap: bool) -> i32 {
    let accounts = Accounts::store().read();
    let Some(account) = accounts.find(id) else {
        eprintln!("ronoctua-llm: no account {id}");
        return 1;
    };
    let done = if wrap {
        statusline::wrap(account)
    } else {
        statusline::unwrap(account)
    };
    match done {
        Ok(()) => 0,
        Err(r) => {
            eprintln!("ronoctua-llm: {id}: refused: {r}");
            1
        }
    }
}
