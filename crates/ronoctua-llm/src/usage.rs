//! Anthropic's usage endpoint — unofficial, consented, never renewing.
//!
//! # What it is, and what the screen must say
//!
//! `GET https://api.anthropic.com/api/oauth/usage`, with the account's OAuth
//! token as a bearer and `anthropic-beta: oauth-2025-04-20`, answers how much
//! of each window is used: `{"five_hour": {"utilization": 35.0,
//! "resets_at": "2026-02-06T22:00:00+00:00"}, "seven_day": …,
//! "seven_day_opus": …|null, "seven_day_sonnet": …|null, …}`. It is the one
//! Claude Code's own `/usage` asks, and it is NOT documented: it may change
//! or go without notice, which the view says beside the numbers.
//!
//! # The token
//!
//! Read at the moment of asking, out of `.credentials.json`, into memory that
//! is zeroed when it is dropped; borrowed from that memory (never copied into
//! a `String` of its own) into a bearer header that is itself zeroed; both
//! gone when [`fetch`] returns. The header is borrowed again by
//! `ronoctua-http`, which writes the whole request into a buffer it sized
//! first and zeroes (`request_wiped`); past it, rustls seals each record in
//! place. Nothing else in the tree names the token
//! (`llm-tokens-never-linger`).
//!
//! **Never renewed.** A token past its `expiresAt` is not sent at all, and a
//! 401 (or a 403, an old token without the scope) is [`Why::Expired`]: Claude
//! Code renews its own token the next time it runs, or the person logs in
//! again. Renewing it here would be a second program writing the agent's
//! credentials, behind its back.
//!
//! # Consent
//!
//! [`fetch`] takes a [`Pass`] for [`HOST`] and refuses one for any other
//! host before the token is read (`llm-usage-asks-with-a-pass`).

use std::path::{Path, PathBuf};

#[cfg(feature = "fetch")]
use ronoctua_consent::Pass;
use serde::{Deserialize, Serialize};
#[cfg(feature = "fetch")]
use zeroize::Zeroizing;

use super::{Account, Refusal, Why, Window};

/// The host the usage endpoint lives on — the consent row's.
pub const HOST: &str = "api.anthropic.com";

#[cfg(feature = "fetch")]
/// The endpoint.
const ADDRESS: &str = "https://api.anthropic.com/api/oauth/usage";

#[cfg(feature = "fetch")]
/// The beta header the endpoint wants for an OAuth token.
const BETA: &str = "oauth-2025-04-20";

#[cfg(feature = "fetch")]
/// Who is asking. Our name and nobody else's.
const USER_AGENT: &str = "ronoctua-llm";

/// The most an answer may weigh: a few windows.
const MOST_BYTES: u64 = 64 * 1024;

/// What it said.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Usage {
    /// The five-hour window.
    pub five_hour: Option<Window>,
    /// The weekly window.
    pub seven_day: Option<Window>,
    /// The weekly window of Opus.
    pub seven_day_opus: Option<Window>,
    /// The weekly window of Sonnet.
    pub seven_day_sonnet: Option<Window>,
}

#[cfg(feature = "fetch")]
/// What a request answered, before it is read: the status and the body.
pub(crate) struct Answer {
    /// The HTTP status.
    pub(crate) status: u16,
    /// The body.
    pub(crate) body: Vec<u8>,
}

/// Asks it for the account in `dir`.
///
/// Blocks: call it on a thread (or from the CLI's `refresh`).
///
/// # Errors
///
/// [`Why::NotLet`] when `pass` is not for [`HOST`] — nothing is read or
/// sent; [`Why::NoCredentials`] (never logged in), [`Why::Unreadable`];
/// [`Why::Expired`] when the token is past its date (nothing sent) or the
/// endpoint refused it; [`Why::Offline`]; [`Why::Failed`] for any other
/// answer.
#[cfg(feature = "fetch")]
pub fn fetch(pass: &Pass, dir: &Path) -> Result<Usage, Refusal> {
    fetch_by(pass, dir, crate::time::now_s(), &mut |bearer| {
        let headers = [("Authorization", bearer), ("anthropic-beta", BETA)];
        let ask = ronoctua_http::web::Ask::get(ADDRESS, USER_AGENT)
            .with_headers(&headers)
            .accepting("application/json")
            .most(MOST_BYTES)
            // The answer is the account's, and so is the header: no other
            // host gets it (the walk carries headers to the pass's host only).
            .hops(0);
        match ronoctua_http::web::request_let(pass, &ask) {
            Ok(got) => Ok(Answer {
                status: got.reply.status,
                body: got.reply.body,
            }),
            Err(e) if e.kind == ronoctua_http::web::Kind::Offline => Err(Why::Offline),
            Err(_) => Err(Why::Failed),
        }
    })
}

#[cfg(feature = "fetch")]
/// The credentials' token and its date, borrowed out of the wiped buffer.
///
/// No `Debug`, no `Clone`, no `Serialize`, and a `&str` rather than a
/// `String`: the token is never copied out of the memory that is zeroed.
#[derive(Deserialize)]
struct Bearer<'a> {
    #[serde(rename = "claudeAiOauth", borrow)]
    oauth: Option<Token<'a>>,
}

#[cfg(feature = "fetch")]
#[derive(Deserialize)]
struct Token<'a> {
    #[serde(rename = "accessToken", borrow)]
    token: Option<&'a str>,
    #[serde(rename = "expiresAt", default)]
    expires_at_ms: Option<i64>,
}

#[cfg(feature = "fetch")]
/// [`fetch`] over any transport: `send` gets the bearer header's value and
/// answers the endpoint's status and body (HTTP for the machine, a canned
/// answer for a test).
pub(crate) fn fetch_by(
    pass: &Pass,
    dir: &Path,
    now_s: i64,
    send: &mut dyn FnMut(&str) -> Result<Answer, Why>,
) -> Result<Usage, Refusal> {
    if !pass.covers(HOST) {
        return Err(Refusal::of(Why::NotLet));
    }
    let answer = {
        let path = dir.join(crate::credentials::CLAUDE_FILE);
        let bytes =
            crate::files::read_secret(&path, crate::credentials::MOST_BYTES).map_err(|e| {
                Refusal::of(if e.kind() == std::io::ErrorKind::NotFound {
                    Why::NoCredentials
                } else {
                    Why::Unreadable
                })
            })?;
        let file: Bearer<'_> =
            serde_json::from_slice(&bytes).map_err(|_| Refusal::of(Why::Unreadable))?;
        let oauth = file.oauth.ok_or(Refusal::of(Why::NoCredentials))?;
        let token = oauth
            .token
            .filter(|t| !t.is_empty())
            .ok_or(Refusal::of(Why::NoCredentials))?;
        if oauth
            .expires_at_ms
            .is_some_and(|ms| ms.div_euclid(1000) <= now_s)
        {
            return Err(Refusal::of(Why::Expired));
        }
        // Sized first: a `String` that grows leaves its smaller copy behind.
        let mut bearer = Zeroizing::new(String::with_capacity("Bearer ".len() + token.len()));
        bearer.push_str("Bearer ");
        bearer.push_str(token);
        send(&bearer).map_err(Refusal::of)?
        // `bearer` and `bytes` are zeroed here, before the answer is read.
    };
    read(&answer)
}

#[cfg(feature = "fetch")]
/// The endpoint's answer, read.
fn read(answer: &Answer) -> Result<Usage, Refusal> {
    match answer.status {
        200 => {}
        401 | 403 => return Err(Refusal::of(Why::Expired)),
        _ => return Err(Refusal::of(Why::Failed)),
    }
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        five_hour: Option<RawWindow>,
        #[serde(default)]
        seven_day: Option<RawWindow>,
        #[serde(default)]
        seven_day_opus: Option<RawWindow>,
        #[serde(default)]
        seven_day_sonnet: Option<RawWindow>,
    }
    #[derive(Deserialize)]
    struct RawWindow {
        #[serde(default)]
        utilization: Option<f64>,
        #[serde(default)]
        resets_at: Option<String>,
    }
    let raw: Raw = serde_json::from_slice(&answer.body).map_err(|_| Refusal::of(Why::Failed))?;
    // A window with no reset is one that is not running (nothing used in
    // it yet): there is no bar to draw and no time to say, so it is `None`.
    let window = |w: Option<RawWindow>| {
        let w = w?;
        Some(Window {
            used_percent: w.utilization? as f32,
            resets_at_s: crate::time::parse_rfc3339(w.resets_at.as_deref()?)?,
        })
    };
    Ok(Usage {
        five_hour: window(raw.five_hour),
        seven_day: window(raw.seven_day),
        seven_day_opus: window(raw.seven_day_opus),
        seven_day_sonnet: window(raw.seven_day_sonnet),
    })
}

impl Usage {
    /// The same, without a window whose reset has passed: it started over,
    /// and how much of the new one is used is not known yet.
    #[must_use]
    pub fn at(self, now_s: i64) -> Self {
        let running = |w: Option<Window>| w.filter(|w| w.resets_at_s > now_s);
        Self {
            five_hour: running(self.five_hour),
            seven_day: running(self.seven_day),
            seven_day_opus: running(self.seven_day_opus),
            seven_day_sonnet: running(self.seven_day_sonnet),
        }
    }
}

/// What `ronoctua-llm refresh` kept of the endpoint
/// (`$XDG_STATE_HOME/ronoctua/llm/<id>/usage.json`): the last ask, and the
/// last numbers it gave — which a refusal after them does not erase.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Kept {
    /// Always 1.
    pub format: u32,
    /// When it was last asked, in seconds since the epoch.
    pub at_s: i64,
    /// What it said the last time it answered — kept through the refusals
    /// after it: a laptop offline for one ask still knows the weekly windows
    /// ([`Usage::at`] drops the ones that reset since).
    pub usage: Option<Usage>,
    /// When `usage` was answered, in seconds since the epoch.
    #[serde(default)]
    pub usage_at_s: Option<i64>,
    /// Why the last ask was not answered; `None` when it was.
    pub refused: Option<Why>,
}

/// The file's name, in the account's state folder.
const KEPT_FILE: &str = "usage.json";

/// Where an account's last answer is kept, under a state root.
pub(crate) fn kept_path(state: &Path, account: &Account) -> Option<PathBuf> {
    crate::id_folder(state, &account.id).map(|d| d.join(KEPT_FILE))
}

/// The last answer kept for `account`, when there is one.
#[must_use]
pub fn kept(account: &Account) -> Option<Kept> {
    kept_in(&ronoctua_store::state_dir()?, account)
}

/// [`kept`], under a given state root.
pub(crate) fn kept_in(state: &Path, account: &Account) -> Option<Kept> {
    let bytes = crate::files::read_bounded(&kept_path(state, account)?, MOST_BYTES).ok()?;
    serde_json::from_slice::<Kept>(&bytes)
        .ok()
        .filter(|k| k.format == 1)
}

/// Keeps an answer for `account`, written whole and atomically: an answer
/// replaces the numbers, and a refusal is kept BESIDE the last ones.
///
/// # Errors
///
/// [`Why::Unknown`] for an id that is not a folder's name; [`Why::Failed`]
/// when it could not be written.
pub fn keep(account: &Account, answer: Result<Usage, Refusal>) -> Result<(), Refusal> {
    let state = ronoctua_store::state_dir().ok_or(Refusal::of(Why::Failed))?;
    keep_in(&state, account, answer, crate::time::now_s())
}

/// [`keep`], under a given state root.
pub(crate) fn keep_in(
    state: &Path,
    account: &Account,
    answer: Result<Usage, Refusal>,
    now_s: i64,
) -> Result<(), Refusal> {
    let path = kept_path(state, account).ok_or(Refusal::of(Why::Unknown))?;
    let kept = match answer {
        Ok(usage) => Kept {
            format: 1,
            at_s: now_s,
            usage: Some(usage),
            usage_at_s: Some(now_s),
            refused: None,
        },
        Err(refusal) => {
            // A file that does not read keeps no numbers to carry over: it is
            // ours, and the next answer writes it whole again.
            let before = kept_in(state, account);
            Kept {
                format: 1,
                at_s: now_s,
                usage: before.and_then(|k| k.usage),
                usage_at_s: before.and_then(|k| k.usage_at_s),
                refused: Some(refusal.why),
            }
        }
    };
    let bytes = serde_json::to_vec_pretty(&kept).map_err(|_| Refusal::of(Why::Failed))?;
    crate::files::write_keeping(&path, &bytes, 0o644).map_err(|_| Refusal::of(Why::Failed))
}

// THEY ASK THE ENDPOINT (over a canned transport): only with `fetch`.
#[cfg(feature = "fetch")]
#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-llm-usage-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The answer's real shape, with a window not running and one null.
    const ANSWER: &str = r#"{"five_hour":{"utilization":35.0,"resets_at":"2026-02-06T22:00:00+00:00"},"seven_day":{"utilization":14.0,"resets_at":"2026-02-12T20:00:00.943679+00:00"},"seven_day_oauth_apps":null,"seven_day_sonnet":{"utilization":0.0,"resets_at":null},"seven_day_opus":null,"extra_usage":{"is_enabled":true,"monthly_limit":100000,"used_credits":0.0,"utilization":null}}"#;

    fn credentials(dir: &Path, expires_at_ms: i64) {
        std::fs::write(
            dir.join(crate::credentials::CLAUDE_FILE),
            format!(
                r#"{{"claudeAiOauth":{{"accessToken":"sk-ant-oat01-made-up","refreshToken":"sk-ant-ort01-made-up","expiresAt":{expires_at_ms},"subscriptionType":"max"}}}}"#
            ),
        )
        .unwrap();
    }

    /// FAILS IF: a pass for another host still reads the token or sends a
    /// request — the person let one host, and only that one may be asked.
    #[test]
    fn nothing_leaves_without_a_pass_for_anthropic() {
        let dir = scratch("pass");
        credentials(&dir, 4_102_444_800_000);
        let store = ronoctua_consent::store_at(Some(dir.join("config")));
        ronoctua_consent::let_hosts_in(&store, &["example.org", HOST], true).unwrap();
        let wrong = ronoctua_consent::pass_in(&store, "example.org").unwrap();
        let mut sent = Vec::new();
        let answer = fetch_by(&wrong, &dir, 0, &mut |bearer| {
            sent.push(bearer.to_owned());
            Ok(Answer {
                status: 200,
                body: ANSWER.as_bytes().to_vec(),
            })
        });
        assert_eq!(answer.unwrap_err().why, Why::NotLet);
        assert!(sent.is_empty(), "a pass for another host sent nothing");

        let right = ronoctua_consent::pass_in(&store, HOST).unwrap();
        let usage = fetch_by(&right, &dir, 0, &mut |bearer| {
            sent.push(bearer.to_owned());
            Ok(Answer {
                status: 200,
                body: ANSWER.as_bytes().to_vec(),
            })
        })
        .unwrap();
        assert_eq!(sent, ["Bearer sk-ant-oat01-made-up"]);
        let at = |s: &str| crate::time::parse_rfc3339(s).unwrap();
        assert_eq!(
            usage,
            Usage {
                five_hour: Some(Window {
                    used_percent: 35.0,
                    resets_at_s: at("2026-02-06T22:00:00Z"),
                }),
                seven_day: Some(Window {
                    used_percent: 14.0,
                    resets_at_s: at("2026-02-12T20:00:00Z"),
                }),
                seven_day_opus: None,
                seven_day_sonnet: None,
            }
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a token past its date is sent anyway, or a refused one is
    /// read as anything but "expired" — the endpoint is never asked to renew.
    #[test]
    fn an_expired_token_is_never_sent_and_a_refused_one_is_expired() {
        let dir = scratch("expired");
        let store = ronoctua_consent::store_at(Some(dir.join("config")));
        ronoctua_consent::let_hosts_in(&store, &[HOST], true).unwrap();
        let pass = ronoctua_consent::pass_in(&store, HOST).unwrap();
        credentials(&dir, 1_000_000_000_000);
        let mut sent = 0;
        let mut send = |_: &str| {
            sent += 1;
            Ok(Answer {
                status: 200,
                body: ANSWER.as_bytes().to_vec(),
            })
        };
        assert_eq!(
            fetch_by(&pass, &dir, 1_000_000_000, &mut send)
                .unwrap_err()
                .why,
            Why::Expired
        );
        assert_eq!(sent, 0, "a token past its date is not sent");

        credentials(&dir, 4_102_444_800_000);
        for (status, why) in [
            (401, Why::Expired),
            (403, Why::Expired),
            (429, Why::Failed),
            (500, Why::Failed),
        ] {
            let answer = fetch_by(&pass, &dir, 0, &mut |_| {
                Ok(Answer {
                    status,
                    body: br#"{"error":{"type":"authentication_error"}}"#.to_vec(),
                })
            });
            assert_eq!(answer.unwrap_err().why, why, "{status}");
        }
        let offline = fetch_by(&pass, &dir, 0, &mut |_| Err(Why::Offline));
        assert_eq!(offline.unwrap_err().why, Why::Offline);
        std::fs::remove_file(dir.join(crate::credentials::CLAUDE_FILE)).unwrap();
        let none = fetch_by(&pass, &dir, 0, &mut |_| {
            unreachable!("no token, no request")
        });
        assert_eq!(none.unwrap_err().why, Why::NoCredentials);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a refusal (offline for one ask) erases the numbers the
    /// endpoint gave last — the weekly Opus and Sonnet windows live only
    /// here — or the next answer does not replace them.
    #[test]
    fn an_answer_is_kept_and_read_back() {
        let dir = scratch("kept");
        let account = Account {
            id: "claude-1".to_owned(),
            kind: crate::Kind::Claude,
            name: "Work".to_owned(),
            dir: dir.join("account"),
            default: true,
        };
        let usage = Usage {
            five_hour: Some(Window {
                used_percent: 12.5,
                resets_at_s: 1_800_000_000,
            }),
            ..Usage::default()
        };
        keep_in(&dir, &account, Ok(usage), 1_700_000_000).unwrap();
        assert_eq!(
            kept_in(&dir, &account),
            Some(Kept {
                format: 1,
                at_s: 1_700_000_000,
                usage: Some(usage),
                usage_at_s: Some(1_700_000_000),
                refused: None,
            })
        );
        for why in [Why::Offline, Why::Expired] {
            keep_in(&dir, &account, Err(Refusal::of(why)), 1_700_000_100).unwrap();
            assert_eq!(
                kept_in(&dir, &account),
                Some(Kept {
                    format: 1,
                    at_s: 1_700_000_100,
                    usage: Some(usage),
                    usage_at_s: Some(1_700_000_000),
                    refused: Some(why),
                }),
                "{why:?} keeps the last numbers beside it"
            );
        }
        let newer = Usage {
            seven_day: usage.five_hour,
            ..Usage::default()
        };
        keep_in(&dir, &account, Ok(newer), 1_700_000_200).unwrap();
        let kept = kept_in(&dir, &account).unwrap();
        assert_eq!(
            (kept.usage, kept.usage_at_s, kept.refused),
            (Some(newer), Some(1_700_000_200), None)
        );
        assert_eq!(
            usage.at(1_800_000_000).five_hour,
            None,
            "a window past its reset"
        );
        assert_eq!(usage.at(1_799_999_999), usage);
        let _ = std::fs::remove_dir_all(dir);
    }
}
