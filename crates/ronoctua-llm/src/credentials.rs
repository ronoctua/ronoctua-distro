//! What an account's credentials say about it — and nothing else.
//!
//! # Three facts, never the token
//!
//! Claude Code keeps `<dir>/.credentials.json`:
//! `{"claudeAiOauth": {"accessToken", "refreshToken", "expiresAt" (ms),
//! "scopes", "subscriptionType", "rateLimitTier"}}`. The view wants which
//! subscription, which tier and when the token expires; the tokens are never
//! deserialized here — `serde` skips a field nobody asked for without
//! copying it — and the file itself is read into memory that is zeroed when
//! it is dropped ([`crate::files::read_secret`]).
//!
//! Codex keeps `<dir>/auth.json`, whose plan (`plus`, `pro`) sits in the
//! claims of its `id_token`, a JWT. Its middle part is decoded into wiped
//! memory too, and only the plan is kept. Codex's token expiry is not one of
//! the facts: Codex renews its own tokens, and nothing here asks OpenAI
//! anything.
//!
//! Only the usage request ever reads Anthropic's access token, in
//! [`crate::usage`] (`llm-tokens-never-linger`).

use std::borrow::Cow;
use std::path::Path;

use base64::Engine as _;
use serde::Deserialize;
use zeroize::Zeroizing;

use super::{Refusal, Why};

/// Claude Code's credentials file, in the account's folder.
pub const CLAUDE_FILE: &str = ".credentials.json";

/// Codex's credentials file, in the account's folder.
pub const CODEX_FILE: &str = "auth.json";

/// The most a credentials file may weigh: a few tokens and their dates.
pub(crate) const MOST_BYTES: u64 = 64 * 1024;

/// The three facts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts {
    /// The subscription (`max`, `pro`; Codex's `plus`, `pro`), as written.
    pub subscription: Option<String>,
    /// The rate-limit tier, as written (Claude's only).
    pub tier: Option<String>,
    /// When the token expires, in seconds since the epoch (Claude's only).
    pub expires_at_s: Option<i64>,
}

/// The facts of the account in `dir` — Claude Code's when its credentials
/// file is there, Codex's when its is.
///
/// # Errors
///
/// [`Why::NoCredentials`] when neither file is there (never logged in);
/// [`Why::Unreadable`] when one is there and does not read as the agent
/// writes it.
pub fn facts(dir: &Path) -> Result<Facts, Refusal> {
    let claude = dir.join(CLAUDE_FILE);
    if claude.exists() {
        let bytes = secret(&claude)?;
        return claude_facts(&bytes).ok_or(Refusal::of(Why::Unreadable));
    }
    let codex = dir.join(CODEX_FILE);
    if codex.exists() {
        let bytes = secret(&codex)?;
        return codex_facts(&bytes).ok_or(Refusal::of(Why::Unreadable));
    }
    Err(Refusal::of(Why::NoCredentials))
}

/// A credentials file, in wiped memory.
fn secret(path: &Path) -> Result<Zeroizing<Vec<u8>>, Refusal> {
    crate::files::read_secret(path, MOST_BYTES).map_err(|e| {
        Refusal::of(if e.kind() == std::io::ErrorKind::NotFound {
            Why::NoCredentials
        } else {
            Why::Unreadable
        })
    })
}

/// Claude Code's file: only the three fields are named, so the tokens are
/// skipped by the parser and never copied out of the wiped buffer.
#[derive(Deserialize)]
struct ClaudeFile<'a> {
    #[serde(rename = "claudeAiOauth", borrow)]
    oauth: Option<ClaudeFacts<'a>>,
}

#[derive(Deserialize)]
struct ClaudeFacts<'a> {
    #[serde(rename = "subscriptionType", borrow, default)]
    subscription: Option<Cow<'a, str>>,
    #[serde(rename = "rateLimitTier", borrow, default)]
    tier: Option<Cow<'a, str>>,
    #[serde(rename = "expiresAt", default)]
    expires_at_ms: Option<i64>,
}

/// The facts out of Claude Code's file; `None` when it is not that file.
fn claude_facts(bytes: &[u8]) -> Option<Facts> {
    let file: ClaudeFile<'_> = serde_json::from_slice(bytes).ok()?;
    let oauth = file.oauth?;
    Some(Facts {
        subscription: oauth.subscription.map(Cow::into_owned),
        tier: oauth.tier.map(Cow::into_owned),
        expires_at_s: oauth.expires_at_ms.map(|ms| ms.div_euclid(1000)),
    })
}

/// Codex's file: its `id_token` is borrowed out of the wiped buffer, never
/// copied; the other tokens are not named.
#[derive(Deserialize)]
struct CodexFile<'a> {
    #[serde(borrow, default)]
    tokens: Option<CodexTokens<'a>>,
}

#[derive(Deserialize)]
struct CodexTokens<'a> {
    #[serde(borrow, default)]
    id_token: Option<&'a str>,
}

/// The claims of Codex's `id_token` that say the plan.
#[derive(Deserialize)]
struct CodexClaims<'a> {
    #[serde(rename = "https://api.openai.com/auth", borrow, default)]
    auth: Option<CodexAuth<'a>>,
}

#[derive(Deserialize)]
struct CodexAuth<'a> {
    #[serde(borrow, default)]
    chatgpt_plan_type: Option<Cow<'a, str>>,
}

/// The facts out of Codex's file; `None` when it is not that file. A file
/// with an API key and no tokens says no plan, and that is a fact too.
fn codex_facts(bytes: &[u8]) -> Option<Facts> {
    let file: CodexFile<'_> = serde_json::from_slice(bytes).ok()?;
    let Some(jwt) = file.tokens.and_then(|t| t.id_token) else {
        return Some(Facts::default());
    };
    let claims = jwt.split('.').nth(1)?;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    // Sized before it is filled, so decoding never grows (and copies) it.
    let mut decoded = Zeroizing::new(vec![0_u8; base64::decoded_len_estimate(claims.len())]);
    let n = engine
        .decode_slice(claims.trim_end_matches('='), &mut decoded)
        .ok()?;
    decoded.truncate(n);
    let claims: CodexClaims<'_> = serde_json::from_slice(&decoded).ok()?;
    Some(Facts {
        subscription: claims
            .auth
            .and_then(|a| a.chatgpt_plan_type)
            .map(Cow::into_owned),
        tier: None,
        expires_at_s: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-llm-credentials-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A Claude Code credentials file, its tokens made up.
    const CLAUDE: &str = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-made-up","refreshToken":"sk-ant-ort01-made-up","expiresAt":1770412938485,"scopes":["user:inference","user:profile"],"subscriptionType":"max","rateLimitTier":"default_claude_max_20x"}}"#;

    #[test]
    fn claude_says_three_facts_and_no_token() {
        let dir = scratch("claude");
        std::fs::write(dir.join(CLAUDE_FILE), CLAUDE).unwrap();
        let facts = facts(&dir).unwrap();
        assert_eq!(
            facts,
            Facts {
                subscription: Some("max".to_owned()),
                tier: Some("default_claude_max_20x".to_owned()),
                expires_at_s: Some(1_770_412_938),
            }
        );
        let printed = format!("{facts:?}");
        assert!(!printed.contains("sk-ant"), "{printed}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn codex_says_its_plan_from_the_id_token_claims() {
        let dir = scratch("codex");
        let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let claims = engine.encode(
            r#"{"exp":1770000000,"https://api.openai.com/auth":{"chatgpt_plan_type":"plus","chatgpt_account_id":"made-up"}}"#,
        );
        let jwt = format!("{}.{claims}.signature", engine.encode(r#"{"alg":"RS256"}"#));
        let file = format!(
            r#"{{"OPENAI_API_KEY":null,"tokens":{{"id_token":"{jwt}","access_token":"made-up","refresh_token":"made-up","account_id":"made-up"}},"last_refresh":"2026-10-05T12:00:00Z"}}"#
        );
        std::fs::write(dir.join(CODEX_FILE), file).unwrap();
        assert_eq!(
            facts(&dir).unwrap(),
            Facts {
                subscription: Some("plus".to_owned()),
                tier: None,
                expires_at_s: None,
            }
        );
        // An API key and no tokens: no plan, and not an error.
        std::fs::write(dir.join(CODEX_FILE), r#"{"OPENAI_API_KEY":"sk-made-up"}"#).unwrap();
        assert_eq!(facts(&dir).unwrap(), Facts::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn no_file_is_never_logged_in_and_a_strange_one_is_unreadable() {
        let dir = scratch("none");
        assert_eq!(facts(&dir).unwrap_err().why, Why::NoCredentials);
        std::fs::write(dir.join(CLAUDE_FILE), "{ not json").unwrap();
        assert_eq!(facts(&dir).unwrap_err().why, Why::Unreadable);
        std::fs::write(dir.join(CLAUDE_FILE), r#"{"other":1}"#).unwrap();
        assert_eq!(facts(&dir).unwrap_err().why, Why::Unreadable);
        std::fs::write(dir.join(CLAUDE_FILE), vec![b' '; 70_000]).unwrap();
        assert_eq!(
            facts(&dir).unwrap_err().why,
            Why::Unreadable,
            "past the bound"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
