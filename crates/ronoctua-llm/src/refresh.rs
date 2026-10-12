//! `ronoctua-llm refresh` — what the usage unit runs.
//!
//! For every account (or one): its transcripts counted from where the last
//! count stopped, the count kept; and for a Claude Code account, the usage
//! endpoint asked when the person let its host, the answer — or why there is
//! none — kept beside the last numbers it gave ([`usage::keep`]).
//!
//! It asks nothing of anybody. The pass is minted from the person's consent
//! at the moment of asking, so a host taken back stops the next request, and
//! one never let sends nothing: the refusal is kept, and the view says it.
//!
//! The pass and the request come in as functions — the machine's
//! (`ronoctua_consent::pass`, [`usage::fetch`]) from the CLI, a test's own
//! consent file and a canned answer from a test — so the whole run is tested
//! the way it goes, roots and all.

use std::path::Path;

use ronoctua_consent::Pass;

use super::{Account, Accounts, Kind, Refusal, Roots, Why, tokens, usage};

/// What a refresh did for one account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refreshed {
    /// The account.
    pub id: String,
    /// The count, kept — or why not.
    pub counted: Result<(), Refusal>,
    /// The usage endpoint's answer — or why there is none; `None` for a
    /// Codex account, which has no endpoint.
    pub asked: Option<Result<(), Refusal>>,
    /// Whether the answer (or its refusal) was kept.
    pub kept: Result<(), Refusal>,
}

/// The pass for [`usage::HOST`], when the person let it.
pub type Minting<'a> = &'a dyn Fn() -> Option<Pass>;

/// The usage request, for an account's folder ([`usage::fetch`]).
pub type Asking<'a> = &'a mut dyn FnMut(&Pass, &Path) -> Result<usage::Usage, Refusal>;

/// Refreshes every account, or `only`, under `roots`.
///
/// Blocks on files and on the network: the CLI's, or a thread's.
///
/// # Errors
///
/// [`Why::Unreadable`] when `llm-accounts.json` does not parse;
/// [`Why::Unknown`] when `only` names no account. Each account's own
/// refusals are in its [`Refreshed`].
pub fn run(
    roots: &Roots,
    only: Option<&str>,
    pass: Minting<'_>,
    ask: Asking<'_>,
) -> Result<Vec<Refreshed>, Refusal> {
    run_at(roots, only, pass, ask, crate::time::now_s())
}

/// [`run`], at an instant.
pub(crate) fn run_at(
    roots: &Roots,
    only: Option<&str>,
    pass: Minting<'_>,
    ask: Asking<'_>,
    now_s: i64,
) -> Result<Vec<Refreshed>, Refusal> {
    let accounts = Accounts::store_in(roots.config.clone())
        .parsed()
        .ok_or(Refusal::of(Why::Unreadable))?;
    let chosen: Vec<&Account> = accounts
        .accounts
        .iter()
        .filter(|a| only.is_none_or(|id| a.id == id))
        .collect();
    if only.is_some() && chosen.is_empty() {
        return Err(Refusal::of(Why::Unknown));
    }
    Ok(chosen
        .into_iter()
        .map(|account| one(roots, account, pass, ask, now_s))
        .collect())
}

/// One account, counted and (a Claude Code one) asked.
fn one(
    roots: &Roots,
    account: &Account,
    pass: Minting<'_>,
    ask: Asking<'_>,
    now_s: i64,
) -> Refreshed {
    let counted = count(roots, account, now_s);
    if account.kind != Kind::Claude {
        return Refreshed {
            id: account.id.clone(),
            counted,
            asked: None,
            kept: Ok(()),
        };
    }
    // The person's consent, read now: a host taken back stops this request,
    // and one never let sends nothing.
    let answer = match pass() {
        Some(pass) => ask(&pass, &account.dir),
        None => Err(Refusal::of(Why::NotLet)),
    };
    let asked = answer.map(|_| ());
    let kept = match &roots.state {
        Some(state) => usage::keep_in(state, account, answer, now_s),
        None => Err(Refusal::of(Why::Failed)),
    };
    Refreshed {
        id: account.id.clone(),
        counted,
        asked: Some(asked),
        kept,
    }
}

/// The account's transcripts counted from where the kept count stopped, and
/// the count kept.
fn count(roots: &Roots, account: &Account, now_s: i64) -> Result<(), Refusal> {
    let root = roots.cache.as_deref().ok_or(Refusal::of(Why::Failed))?;
    let mut cache = tokens::kept_in(root, account);
    tokens::count(&account.dir, account.kind, &mut cache, now_s)?;
    tokens::keep_in(root, account, &cache)
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;

    use super::*;
    use crate::time::{DAY_S, HOUR_S, days_from_civil};

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-llm-refresh-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Noon on 2026-10-05, UTC.
    const fn noon() -> i64 {
        days_from_civil(2026, 10, 5) * DAY_S + 12 * HOUR_S
    }

    /// The endpoint's answer.
    const ANSWER: &str = r#"{"five_hour":{"utilization":35.0,"resets_at":"2026-10-05T15:00:00+00:00"},"seven_day":{"utilization":14.0,"resets_at":"2026-10-09T20:00:00+00:00"},"seven_day_opus":{"utilization":9.0,"resets_at":"2026-10-09T20:00:00+00:00"},"seven_day_sonnet":null}"#;

    /// Two accounts on disk, as the agents leave them, and the list.
    fn machine(dir: &Path) -> (Roots, Account, Account) {
        let roots = Roots {
            config: Some(dir.join("config")),
            state: Some(dir.join("state")),
            cache: Some(dir.join("cache")),
            home: Some(dir.join("home")),
        };
        let claude = Account {
            id: "claude-1".to_owned(),
            kind: Kind::Claude,
            name: "Work".to_owned(),
            dir: dir.join("home").join(".claude"),
            default: true,
        };
        let codex = Account {
            id: "codex-1".to_owned(),
            kind: Kind::Codex,
            name: "Home".to_owned(),
            dir: dir.join("home").join(".codex"),
            default: true,
        };
        let project = claude.dir.join("projects").join("-home-p-work");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("s1.jsonl"),
            concat!(
                r#"{"type":"assistant","timestamp":"2026-10-05T11:00:00Z","requestId":"req_a","message":{"id":"msg_a","model":"claude-opus-4-1","usage":{"input_tokens":10,"output_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#,
                "\n"
            ),
        )
        .unwrap();
        std::fs::write(
            claude.dir.join(crate::credentials::CLAUDE_FILE),
            r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-made-up","expiresAt":4102444800000}}"#,
        )
        .unwrap();
        let day = codex
            .dir
            .join("sessions")
            .join("2026")
            .join("10")
            .join("05");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(
            day.join("rollout-2026-10-05T11-00-00-0199aaaa-bbbb-cccc-dddd-eeeeffff0000.jsonl"),
            concat!(
                r#"{"timestamp":"2026-10-05T11:00:00Z","type":"turn_context","payload":{"model":"gpt-5"}}"#,
                "\n",
                r#"{"timestamp":"2026-10-05T11:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":700},"last_token_usage":{"total_tokens":700}},"rate_limits":null}}"#,
                "\n"
            ),
        )
        .unwrap();
        crate::Accounts::store_in(roots.config.clone())
            .write(&crate::Accounts {
                format: 1,
                accounts: vec![claude.clone(), codex.clone()],
            })
            .unwrap();
        (roots, claude, codex)
    }

    /// FAILS IF: a refresh does not keep each account's count where it is
    /// read back, asks the endpoint for a Codex account or without the
    /// person's pass, mints the pass for another host, or loses the last
    /// numbers when a later ask is refused.
    #[test]
    fn a_refresh_counts_asks_with_the_persons_pass_and_keeps_both() {
        let dir = scratch("run");
        let (roots, claude, codex) = machine(&dir);
        let consent = ronoctua_consent::store_at(Some(dir.join("consent")));
        ronoctua_consent::let_hosts_in(&consent, &[usage::HOST], true).unwrap();
        let minted = Cell::new(0);
        let mint = || {
            minted.set(minted.get() + 1);
            ronoctua_consent::pass_in(&consent, usage::HOST).ok()
        };
        let asked_for = RefCell::new(Vec::new());
        let mut ask = |pass: &Pass, at: &Path| {
            asked_for.borrow_mut().push(at.to_path_buf());
            usage::fetch_by(pass, at, noon(), &mut |bearer| {
                assert_eq!(bearer, "Bearer sk-ant-oat01-made-up");
                Ok(usage::Answer {
                    status: 200,
                    body: ANSWER.as_bytes().to_vec(),
                })
            })
        };
        let done = run_at(&roots, None, &mint, &mut ask, noon()).unwrap();
        assert_eq!(
            done,
            [
                Refreshed {
                    id: "claude-1".to_owned(),
                    counted: Ok(()),
                    asked: Some(Ok(())),
                    kept: Ok(()),
                },
                Refreshed {
                    id: "codex-1".to_owned(),
                    counted: Ok(()),
                    asked: None,
                    kept: Ok(()),
                },
            ]
        );
        assert_eq!(minted.get(), 1, "one pass, for the one Claude account");
        assert_eq!(
            *asked_for.borrow(),
            std::slice::from_ref(&claude.dir),
            "Codex has no endpoint"
        );
        let cache = roots.cache.as_deref().unwrap();
        let tally = |a: &Account| tokens::kept_in(cache, a).tally_in(&|_| 0).by_model;
        assert_eq!(tally(&claude)["claude-opus-4-1"], 30);
        assert_eq!(tally(&codex)["gpt-5"], 700);
        let state = roots.state.as_deref().unwrap();
        let first = usage::kept_in(state, &claude).unwrap();
        assert_eq!(
            (first.refused, first.usage_at_s, first.at_s),
            (None, Some(noon()), noon())
        );
        let opus = first.usage.unwrap().seven_day_opus.unwrap();
        assert_eq!(opus.used_percent, 9.0);
        assert!(usage::kept_in(state, &codex).is_none());

        // The host taken back: nothing is asked, the refusal is kept, and
        // the numbers the endpoint gave last stay beside it.
        ronoctua_consent::let_hosts_in(&consent, &[usage::HOST], false).unwrap();
        let later = noon() + 60;
        let done = run_at(&roots, Some("claude-1"), &mint, &mut ask, later).unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].asked, Some(Err(Refusal::of(Why::NotLet))));
        assert_eq!(asked_for.borrow().len(), 1, "no pass, no request");
        let second = usage::kept_in(state, &claude).unwrap();
        assert_eq!(second.refused, Some(Why::NotLet));
        assert_eq!(second.at_s, later);
        assert_eq!(
            (second.usage, second.usage_at_s),
            (first.usage, Some(noon())),
            "the last numbers are not lost to a refusal"
        );

        assert_eq!(
            run_at(&roots, Some("nobody"), &mint, &mut ask, later).unwrap_err(),
            Refusal::of(Why::Unknown)
        );
        let only = run_at(&roots, Some("codex-1"), &mint, &mut ask, later).unwrap();
        assert_eq!(
            only.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["codex-1"]
        );
        std::fs::write(
            roots.config.as_deref().unwrap().join(Accounts::FILE),
            "{ broken",
        )
        .unwrap();
        assert_eq!(
            run_at(&roots, None, &mint, &mut ask, later).unwrap_err(),
            Refusal::of(Why::Unreadable)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
