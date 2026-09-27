//! Opening a kiosk window: the argument vector, the nonce, and where our
//! extension's id is written.
//!
//! # One path, Chrome open or not
//!
//! The window is born from a command line — the person's profile, a new
//! window, and our extension's `kiosk.html` with a nonce in its fragment. With
//! Chrome already running, its process singleton hands the line to the
//! running browser and the spawned process exits; with Chrome closed, that
//! same line starts it. Either way the page puts the nonce in its title (the
//! compositor sees it) and tells the extension (the footer hears it), and the
//! pair is made from the two halves — never from timing.
//!
//! None of Chromium's `--kiosk`, `--start-fullscreen`, `--user-data-dir` or
//! `--enable-features` is ever passed: the first two lock the whole browser,
//! the third loses the person's logins, the fourth replaces the wrapper's
//! own (`web-apps-are-never-kiosk`).

use anyhow::{Context as _, Result, bail};
use std::ffi::OsString;
use std::io::Read as _;
use std::path::Path;

/// The environment variable that names our native-messaging host manifest,
/// from which the extension's id is read.
pub const MANIFEST_ENV: &str = "RONOCTUA_BROWSER_MANIFEST";

/// The kiosk page inside our extension.
pub const PAGE: &str = "kiosk.html";

/// What a kiosk window is opened with.
#[derive(Debug, Clone, Copy)]
pub struct Launch<'a> {
    /// Chrome's program.
    pub chrome: &'a Path,
    /// The profile directory (`Default`, `Profile 1`…).
    pub profile: &'a str,
    /// Our extension's id.
    pub extension_id: &'a str,
    /// The nonce that pairs the window.
    pub nonce: &'a str,
    /// Where the kiosk goes once paired; `None` for Chrome's new-tab page.
    pub url: Option<&'a str>,
}

/// The whole argument vector, program first. The address is ONE element: no
/// shell ever reads it.
#[must_use]
pub fn argv(l: &Launch<'_>) -> Vec<OsString> {
    let mut page = format!(
        "chrome-extension://{}/{PAGE}#n={}&p={}",
        l.extension_id,
        percent_encoded(l.nonce),
        percent_encoded(l.profile)
    );
    if let Some(url) = l.url {
        page.push_str("&u=");
        page.push_str(&percent_encoded(url));
    }
    vec![
        l.chrome.as_os_str().to_owned(),
        OsString::from(format!("--profile-directory={}", l.profile)),
        OsString::from("--new-window"),
        OsString::from(page),
    ]
}

/// A fresh nonce: 128 random bits in hex, from the kernel.
///
/// # Errors
///
/// When `/dev/urandom` cannot be read.
pub fn nonce() -> std::io::Result<String> {
    let mut bytes = [0_u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Our extension's id, read from the native-messaging host manifest the
/// system wrote (`allowed_origins[0]`, `chrome-extension://<id>/`).
///
/// # Errors
///
/// When the file cannot be read or does not name exactly one extension.
pub fn extension_id(manifest: &Path) -> Result<String> {
    let text = std::fs::read_to_string(manifest)
        .with_context(|| format!("reading {}", manifest.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).context("the manifest is not JSON")?;
    let origin = value
        .get("allowed_origins")
        .and_then(|o| o.as_array())
        .and_then(|a| a.first())
        .and_then(|o| o.as_str())
        .context("the manifest names no allowed origin")?;
    let id = origin
        .strip_prefix("chrome-extension://")
        .and_then(|rest| rest.strip_suffix('/'))
        .unwrap_or_default();
    if !crate::url::is_extension_id(id) {
        bail!("the manifest's origin is not an extension: {origin}");
    }
    Ok(id.to_owned())
}

/// Whether a name is a Chrome profile DIRECTORY this system opens a kiosk
/// in: `Default` or `Profile <n>`. Anything else is refused rather than
/// passed to Chrome, which would create a new, empty profile by that name.
#[must_use]
pub fn is_profile_dir(name: &str) -> bool {
    name == "Default"
        || name
            .strip_prefix("Profile ")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Percent-encodes everything but the unreserved characters (RFC 3986), so a
/// value survives inside a fragment's `key=value&` grammar.
#[must_use]
pub fn percent_encoded(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kiosk_argv_has_no_flag_that_locks_chrome() {
        let args = argv(&Launch {
            chrome: Path::new("/run/current-system/sw/bin/google-chrome-stable"),
            profile: "Profile 1",
            extension_id: "abcdefghijklmnopabcdefghijklmnop",
            nonce: "00ff",
            url: Some("https://a.example/x?y=1&z=2"),
        });
        let text: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(text[1], "--profile-directory=Profile 1");
        assert_eq!(text[2], "--new-window");
        assert_eq!(
            text[3],
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop/kiosk.html#n=00ff&p=Profile%201&u=https%3A%2F%2Fa.example%2Fx%3Fy%3D1%26z%3D2"
        );
        for flag in [
            "--kiosk",
            "--start-fullscreen",
            "--user-data-dir",
            "--enable-features",
        ] {
            assert!(text.iter().all(|a| !a.starts_with(flag)), "{flag}");
        }
    }

    #[test]
    fn a_nonce_is_128_bits_of_hex() {
        let n = nonce().expect("urandom");
        assert_eq!(n.len(), 32);
        assert!(n.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn only_profile_directories_are_profiles() {
        assert!(is_profile_dir("Default"));
        assert!(is_profile_dir("Profile 12"));
        assert!(!is_profile_dir("Profile "));
        assert!(!is_profile_dir("../Default"));
        assert!(!is_profile_dir("System Profile"));
    }

    #[test]
    fn the_id_comes_from_the_manifest() {
        let dir = std::env::temp_dir().join(format!("ronoctua-browser-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("m.json");
        std::fs::write(
            &file,
            r#"{"name":"br.ronoctua.browser","allowed_origins":["chrome-extension://abcdefghijklmnopabcdefghijklmnop/"]}"#,
        )
        .expect("write");
        assert_eq!(
            extension_id(&file).expect("id"),
            "abcdefghijklmnopabcdefghijklmnop"
        );
        std::fs::write(&file, r#"{"allowed_origins":["https://evil/"]}"#).expect("write");
        assert!(extension_id(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
