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
//! # And an API key: whether it is there, never what it is
//!
//! A Claude Code account on an API key ([`Kind::ClaudeApi`]) has no
//! credentials file. Its key comes one of three ways, as Claude Code 2.1.314
//! looks for it:
//!
//! - **The Console login** keeps it as `primaryApiKey` in Claude Code's
//!   GLOBAL configuration — `.claude.json` in the folder `CLAUDE_CONFIG_DIR`
//!   names, `~/.claude.json` for `~/.claude` (opened with the variable
//!   unset), and the old `<folder>/.config.json` instead when that one has
//!   anything in it (`global_config`). On Linux it is never in a keychain:
//!   the build writes it to that file.
//! - **A key helper**: the account's `settings.json` names a program that
//!   prints the key (`apiKeyHelper`), which Claude Code runs itself.
//! - **The account's own environment**: `ANTHROPIC_API_KEY` in the `env` of
//!   its `settings.json`, which Claude Code puts in its environment when it
//!   starts in that folder — the folder's key, then, as much as a login is.
//!
//! Each file is read into wiped memory and parsed for its one field into a
//! `Present` — a yes or a no, made where the text lies in that memory —
//! so the key is never copied out, never returned and never printed. (A key
//! is letters, digits, `-` and `_`, which Claude Code checks before it keeps
//! one: there is no escape in it for the parser to unescape into a buffer of
//! its own.) A key the SHELL exports (`ANTHROPIC_API_KEY` in a terminal's
//! environment) is the shell's, not the folder's, and is not looked for.
//!
//! Only the usage request ever reads Anthropic's access token, in
//! [`crate::usage`] (`llm-tokens-never-linger`).

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Deserialize;
use zeroize::Zeroizing;

use super::{Account, Kind, Refusal, Why};

/// Claude Code's credentials file, in the account's folder.
pub const CLAUDE_FILE: &str = ".credentials.json";

/// Codex's credentials file, in the account's folder.
pub const CODEX_FILE: &str = "auth.json";

/// The most a credentials file may weigh: a few tokens and their dates.
pub(crate) const MOST_BYTES: u64 = 64 * 1024;

/// Claude Code's global configuration, beside the folder its variable names.
const GLOBAL_CONFIG: &str = ".claude.json";

/// The global configuration's old place, inside the account's folder — read
/// INSTEAD when it has anything in it, as Claude Code does.
const OLD_GLOBAL_CONFIG: &str = ".config.json";

/// The most the global configuration may weigh: it keeps every project the
/// person opened, and grows with them (tens of kilobytes is usual).
const MOST_CONFIG_BYTES: u64 = 16 * 1024 * 1024;

/// The most the account's `settings.json` may weigh, as the status line
/// reads it.
const MOST_SETTINGS_BYTES: u64 = 1024 * 1024;

/// The facts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts {
    /// The subscription (`max`, `pro`; Codex's `plus`, `pro`), as written.
    pub subscription: Option<String>,
    /// The rate-limit tier, as written (Claude's only).
    pub tier: Option<String>,
    /// When the token expires, in seconds since the epoch (Claude's only).
    pub expires_at_s: Option<i64>,
    /// Where an API key's account gets its key — that it HAS one, and
    /// nothing of the key ([`facts_of`]); `None` for every other kind.
    pub api_key: Option<ApiKey>,
}

/// Where an API key's account gets its key from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiKey {
    /// The Console login kept it in Claude Code's global configuration
    /// (`primaryApiKey`).
    Login,
    /// The account's settings name a program Claude Code runs to print it
    /// (`apiKeyHelper`).
    Helper,
    /// The account's settings give it in their `env` (`ANTHROPIC_API_KEY`).
    Settings,
}

/// The facts of `account`, by its kind: a subscription's and Codex's from
/// their credentials ([`facts`]); an API key's from whether a key is there
/// ([`Facts::api_key`]), under the machine's home.
///
/// # Errors
///
/// [`Why::NoCredentials`] when there is nothing to log in with (never
/// logged in); [`Why::Unreadable`] when a file is there and does not read
/// as the agent writes it.
pub fn facts_of(account: &Account) -> Result<Facts, Refusal> {
    facts_in(account, crate::Roots::of_env().home.as_deref())
}

/// [`facts_of`], under a given home.
pub(crate) fn facts_in(account: &Account, home: Option<&Path>) -> Result<Facts, Refusal> {
    match account.kind {
        Kind::Claude | Kind::Codex => facts(&account.dir),
        Kind::ClaudeApi => api_key(&account.dir, home).map(|from| Facts {
            api_key: Some(from),
            ..Facts::default()
        }),
    }
}

/// Where Claude Code finds its global configuration for the folder `dir`,
/// as 2.1.314 does: the old `<dir>/.config.json` when it is a file with
/// something in it, and otherwise `.claude.json` beside what the variable
/// names — `<dir>/.claude.json` when `CLAUDE_CONFIG_DIR` points at `dir`,
/// `~/.claude.json` for `~/.claude`, which is opened with the variable unset
/// (`launch_arguments`).
fn global_config(dir: &Path, home: Option<&Path>) -> PathBuf {
    let old = dir.join(OLD_GLOBAL_CONFIG);
    if std::fs::metadata(&old).is_ok_and(|m| m.is_file() && m.len() > 0) {
        return old;
    }
    match home {
        Some(home) if dir == Kind::ClaudeApi.native_dir(home) => home.join(GLOBAL_CONFIG),
        _ => dir.join(GLOBAL_CONFIG),
    }
}

/// Whether an API key's account in `dir` has a key, and where it comes
/// from — the Console login's first, then the account's settings: a helper,
/// then a key in their `env`.
fn api_key(dir: &Path, home: Option<&Path>) -> Result<ApiKey, Refusal> {
    let config: Option<GlobalConfig> = read_one(&global_config(dir, home), MOST_CONFIG_BYTES)?;
    if config.and_then(|c| c.key).is_some_and(|p| p.0) {
        return Ok(ApiKey::Login);
    }
    let settings: Option<Settings> = read_one(
        &dir.join(crate::statusline::SETTINGS_FILE),
        MOST_SETTINGS_BYTES,
    )?;
    let Some(settings) = settings else {
        return Err(Refusal::of(Why::NoCredentials));
    };
    if settings.helper.is_some_and(|p| p.0) {
        return Ok(ApiKey::Helper);
    }
    if settings.env.and_then(|e| e.key).is_some_and(|p| p.0) {
        return Ok(ApiKey::Settings);
    }
    Err(Refusal::of(Why::NoCredentials))
}

/// A file that may hold a secret, read into wiped memory and parsed for
/// the one field `T` names; `None` when it is not there.
fn read_one<T: for<'de> Deserialize<'de>>(path: &Path, most: u64) -> Result<Option<T>, Refusal> {
    let bytes = match crate::files::read_secret(path, most) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Refusal::of(Why::Unreadable)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| Refusal::of(Why::Unreadable))
}

/// The global configuration: the Console login's key, and nothing else.
#[derive(Deserialize)]
struct GlobalConfig {
    #[serde(rename = "primaryApiKey", default)]
    key: Option<Present>,
}

/// An account's settings: its key helper and the key its `env` gives, and
/// nothing else — every other variable of `env` skipped unread.
#[derive(Deserialize)]
struct Settings {
    #[serde(rename = "apiKeyHelper", default)]
    helper: Option<Present>,
    #[serde(default)]
    env: Option<SettingsEnv>,
}

/// The `env` of an account's settings: the key, as a yes or a no.
#[derive(Deserialize)]
struct SettingsEnv {
    #[serde(rename = "ANTHROPIC_API_KEY", default)]
    key: Option<Present>,
}

/// Whether a text that may be a secret is there and not blank — and
/// NOTHING of it: the visitor is handed the text where it lies in the wiped
/// buffer and keeps one bit.
struct Present(bool);

impl<'de> Deserialize<'de> for Present {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Seen;
        impl serde::de::Visitor<'_> for Seen {
            type Value = Present;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a text")
            }
            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Present, E> {
                Ok(Present(!text.trim().is_empty()))
            }
        }
        d.deserialize_str(Seen)
    }
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
        api_key: None,
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
        api_key: None,
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
                api_key: None,
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
                api_key: None,
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

    /// A Console login's key, made up — and what must never come back out.
    /// Not in the shape of a real one: the tree is swept for those
    /// (`secrets-out-of-repo`), and the check here is presence, not shape.
    const KEY: &str = "made-up-console-key_0123456789";

    fn api_account(dir: &Path) -> Account {
        Account {
            id: "claude-api-1".to_owned(),
            kind: Kind::ClaudeApi,
            name: "Console".to_owned(),
            dir: dir.to_path_buf(),
            default: true,
        }
    }

    /// FAILS IF: an API key's account with a key from the Console login (in
    /// the global configuration Claude Code reads for ITS folder), from a
    /// key helper or from its settings' `env` does not read as logged in;
    /// one without reads as logged in (a blank key, a key in another
    /// account's place, another variable of `env`); or anything the check
    /// answers carries the key.
    #[test]
    fn an_api_key_is_found_where_claude_code_keeps_it_and_never_said() {
        let root = scratch("api");
        let home = root.join("home");
        let folder = home.join(".claude-api");
        std::fs::create_dir_all(&folder).unwrap();
        let account = api_account(&folder);
        let facts = |a: &Account| facts_in(a, Some(&home));
        assert_eq!(facts(&account).unwrap_err().why, Why::NoCredentials);

        // THE CONSOLE LOGIN, beside the folder the variable names — amid
        // the rest of a global configuration, a secret of an MCP server's
        // included.
        let config = format!(
            r#"{{"numStartups":3,"mcpServers":{{"x":{{"env":{{"TOKEN":"sk-ant-other"}}}}}},"primaryApiKey":"{KEY}","customApiKeyResponses":{{"approved":["0123456789"],"rejected":[]}}}}"#
        );
        std::fs::write(folder.join(GLOBAL_CONFIG), &config).unwrap();
        let found = facts(&account).unwrap();
        assert_eq!(
            found,
            Facts {
                api_key: Some(ApiKey::Login),
                ..Facts::default()
            }
        );
        let said = format!("{found:?} {:?}", facts(&account));
        assert!(
            !said.contains("sk-ant") && !said.contains("0123456789"),
            "{said}"
        );

        // A BLANK KEY, or none, is no login.
        for none in [r#"{"primaryApiKey":""}"#, r#"{"primaryApiKey":null}"#, "{}"] {
            std::fs::write(folder.join(GLOBAL_CONFIG), none).unwrap();
            assert_eq!(
                facts(&account).unwrap_err().why,
                Why::NoCredentials,
                "{none}"
            );
        }

        // CLAUDE CODE'S OWN FOLDER is opened with the variable unset: its
        // global configuration is `~/.claude.json`, not one inside it.
        let native = api_account(&home.join(".claude"));
        std::fs::create_dir_all(&native.dir).unwrap();
        std::fs::write(native.dir.join(GLOBAL_CONFIG), &config).unwrap();
        assert_eq!(facts(&native).unwrap_err().why, Why::NoCredentials);
        std::fs::write(home.join(GLOBAL_CONFIG), &config).unwrap();
        assert_eq!(facts(&native).unwrap().api_key, Some(ApiKey::Login));

        // THE OLD PLACE, when it has anything in it, is read INSTEAD.
        std::fs::write(folder.join(GLOBAL_CONFIG), &config).unwrap();
        std::fs::write(folder.join(OLD_GLOBAL_CONFIG), r#"{"numStartups":1}"#).unwrap();
        assert_eq!(facts(&account).unwrap_err().why, Why::NoCredentials);
        std::fs::write(folder.join(OLD_GLOBAL_CONFIG), "").unwrap();
        assert_eq!(facts(&account).unwrap().api_key, Some(ApiKey::Login));
        std::fs::remove_file(folder.join(GLOBAL_CONFIG)).unwrap();

        // A KEY HELPER in the account's settings is a login too, and so is
        // a key in their `env` — the folder's own environment — but not a
        // blank one, nor another variable.
        let settings = folder.join(crate::statusline::SETTINGS_FILE);
        for none in [
            r#"{"env":{"ANTHROPIC_API_KEY":""}}"#.to_owned(),
            format!(r#"{{"env":{{"ANTHROPIC_AUTH_TOKEN":"{KEY}"}}}}"#),
            r#"{"model":"opus"}"#.to_owned(),
        ] {
            std::fs::write(&settings, &none).unwrap();
            assert_eq!(
                facts(&account).unwrap_err().why,
                Why::NoCredentials,
                "{none}"
            );
        }
        std::fs::write(
            &settings,
            format!(r#"{{"env":{{"DISABLE_TELEMETRY":"1","ANTHROPIC_API_KEY":"{KEY}"}}}}"#),
        )
        .unwrap();
        let in_env = facts(&account).unwrap();
        assert_eq!(in_env.api_key, Some(ApiKey::Settings));
        assert!(!format!("{in_env:?}").contains("sk-ant"));
        std::fs::write(
            &settings,
            format!(r#"{{"env":{{"ANTHROPIC_API_KEY":"{KEY}"}},"apiKeyHelper":"~/bin/key.sh"}}"#),
        )
        .unwrap();
        let helped = facts(&account).unwrap();
        assert_eq!(helped.api_key, Some(ApiKey::Helper));
        assert!(!format!("{helped:?}").contains("sk-ant"));
        std::fs::write(&settings, r#"{"env":{"ANTHROPIC_API_KEY":42}}"#).unwrap();
        assert_eq!(
            facts(&account).unwrap_err().why,
            Why::Unreadable,
            "a key that is not text is not read past"
        );
        std::fs::remove_file(&settings).unwrap();

        // A FILE THAT IS NOT WHAT CLAUDE CODE WRITES is not read past.
        for strange in ["{ broken", r#"{"primaryApiKey":42}"#] {
            std::fs::write(folder.join(GLOBAL_CONFIG), strange).unwrap();
            assert_eq!(
                facts(&account).unwrap_err().why,
                Why::Unreadable,
                "{strange}"
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: an account of another kind is judged by the API key's
    /// rules — a subscription logged in by its credentials file would read
    /// as not logged in, or one with only a Console key as logged in.
    #[test]
    fn each_kind_is_judged_by_its_own_login() {
        let dir = scratch("kinds");
        std::fs::write(
            dir.join(GLOBAL_CONFIG),
            format!(r#"{{"primaryApiKey":"{KEY}"}}"#),
        )
        .unwrap();
        let mut account = api_account(&dir);
        account.kind = Kind::Claude;
        assert_eq!(
            facts_in(&account, None).unwrap_err().why,
            Why::NoCredentials
        );
        std::fs::write(dir.join(CLAUDE_FILE), CLAUDE).unwrap();
        assert_eq!(
            facts_in(&account, None).unwrap().subscription.as_deref(),
            Some("max")
        );
        account.kind = Kind::ClaudeApi;
        assert_eq!(
            facts_in(&account, None).unwrap().api_key,
            Some(ApiKey::Login)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
