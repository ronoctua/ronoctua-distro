//! `options.json` — the person's opt-ins that travel to every machine.
//!
//! ```json
//! {"format": 1, "ssh-signing": null,
//!  "binary-cache": {"name": "someone", "public-key": "someone.cachix.org-1:…=",
//!                   "visibility": "public"},
//!  "distro-apps-keep-tested-pins": true,
//!  "download-mirrors": ["https://tarballs.example.org"]}
//! ```
//!
//! The belongings' targets offer them after a restore ("no" first); the Nix
//! side reads them through `ronoctua.person.*`.
//!
//! WHETHER A SECURITY KEY ANSWERS SUDO IS NOT HERE (owner, 2026-10-11): it is
//! root's list of the person's keys (`ronoctua_security::u2f::AUTHFILE`),
//! written through the sudo door and holding at once, with no rebuild — a
//! choice that changes how root authenticates never travels as data.
//!
//! # The binary cache is the person's
//!
//! `binary-cache` names the person's OWN Cachix cache — never the distro's,
//! which has none: its name, its public signing key and who may read it. It
//! travels in the plain part of their belongings like every opt-in, and
//! `nix/modules/binary-cache.nix` reads root's validated copy of it and adds
//! the substituter BEFORE `cache.nixos.org`. The token that pushes (and that
//! reads a private cache) is a secret and is never here: it lives in the
//! sealed part (`ronoctua_cachix::token`).
//!
//! The two validators — this one and the Nix module's — are held to ONE
//! table of cases (`fixtures/binary-cache-cases.json`): a value one of them
//! takes and the other refuses would be a cache the screen accepts and the
//! machine ignores, or the reverse.
//!
//! # The distro's apps follow their releases, unless the person said not
//!
//! ON BY DEFAULT (owner's decision, 2026-11-01; 2026-10-20 it was off unless
//! turned on): the routine update follows each distro AppImage whose table
//! entry follows a GitHub repository (the ArtCraft Launcher, SolidCraft) past
//! the pin the distro tested, into the person's own
//! `software/distro-pins.json` ([`crate::distro_pins`]). The person may turn
//! it off on the update's page, and then that file goes at the next update
//! and the machine builds the distro's pins at once — [`Options::follows_releases`]
//! is the one answer.
//!
//! WHY A NEW FIELD SAYS "OFF". The switch of 2026-10-20,
//! `distro-apps-follow-releases`, was written only while on, so a file
//! without it is both "never chosen" and "turned off": nothing tells them
//! apart, and under the new default both would read as on. So the person's
//! "off" is a field of its own, `distro-apps-keep-tested-pins`, which no
//! file before it carries: absent is the default (on), and `true` is an off
//! that stays off. An older file's `true` in the old field is the new
//! default anyway. Each is written only while `true`: an older machine,
//! whose reader refuses a field it does not know, meets the new one only
//! once the person turns following off. The old field is kept as it was
//! read and set by a turn of the switch, so an older machine of theirs,
//! which reads only it, hears the same choice.
//!
//! # Mirrors for the machine's downloads
//!
//! `download-mirrors` (the parts, 2026-10-24): servers the person names that
//! hold files by their hash (`<mirror>/<algo>/<hex>`, the layout of
//! `tarballs.nixos.org`) or by their name (`<mirror>/<name>`, a folder on a
//! machine of the LAN). When a part's own download fails, root's realizer
//! asks them — a source the person named is their consent — and a file is
//! taken only when Nix finds the hash the flake already pins, so a mirror can
//! only bring the very bytes the distro declared. `http://` is allowed for
//! that reason; a user name, a query or a fragment is not (a secret has no
//! place in a file that travels in the plain). Written only while some are
//! named, for the older machine's reason above.

use serde::{Deserialize, Serialize};

use crate::{Document, Refusal, Why, check};

/// The file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Options {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// Commits signed by an SSH key, if the person said yes.
    pub ssh_signing: Option<SshSigning>,
    /// The person's own binary cache, if they named one. Absent in a file
    /// written before it existed, which reads as none.
    #[serde(default)]
    pub binary_cache: Option<BinaryCache>,
    /// The switch of 2026-10-20, when following was off unless turned on —
    /// what an older machine reads: `true` is the default now. Kept as read,
    /// set by [`Options::set_following`], and never written while off. Ask
    /// [`Options::follows_releases`], never this.
    #[serde(default, skip_serializing_if = "is_off")]
    pub distro_apps_follow_releases: bool,
    /// The person turned following off: the distro's AppImages stay on the
    /// pins the distro tested. Absent — following, the default — in a file
    /// written before it existed, and never written while following.
    #[serde(default, skip_serializing_if = "is_off")]
    pub distro_apps_keep_tested_pins: bool,
    /// The mirrors the person named for the machine's downloads, in the
    /// order they are asked. Absent — none — in a file written before it
    /// existed, and never written while empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub download_mirrors: Vec<String>,
}

/// How many mirrors a file may name.
pub const MIRRORS_MOST: usize = 8;

/// How long a mirror's address may be.
const MIRROR_LONGEST: usize = 512;

/// Whether `url` may be a mirror: `https://` or `http://`, a host, printable
/// ASCII with no space, and no user name, query or fragment. Pure.
#[must_use]
pub fn is_mirror(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    url.len() <= MIRROR_LONGEST
        && url.chars().all(|c| c.is_ascii_graphic())
        && !url.contains(['@', '?', '#', '\\'])
        && rest.split('/').next().is_some_and(|host| !host.is_empty())
}

/// The host a mirror's address names — what the person's consent records.
#[must_use]
pub fn mirror_host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?;
    let host = host.rsplit_once(':').map_or(host, |(h, port)| {
        if port.chars().all(|c| c.is_ascii_digit()) {
            h
        } else {
            host
        }
    });
    (!host.is_empty()).then_some(host)
}

/// Whether a switch is off — what is never written.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_off(on: &bool) -> bool {
    !*on
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            ssh_signing: None,
            binary_cache: None,
            distro_apps_follow_releases: false,
            distro_apps_keep_tested_pins: false,
            download_mirrors: Vec::new(),
        }
    }
}

impl Options {
    /// Whether the distro's AppImages that follow their releases follow them
    /// with the routine update, past the distro's pins: yes, unless the
    /// person turned it off. Pure.
    #[must_use]
    pub const fn follows_releases(&self) -> bool {
        !self.distro_apps_keep_tested_pins
    }

    /// Following turned `on` or off — the person's gesture: the field that
    /// says it, and the older switch, so an older machine of theirs hears
    /// the same choice.
    pub const fn set_following(&mut self, on: bool) {
        self.distro_apps_keep_tested_pins = !on;
        self.distro_apps_follow_releases = on;
    }
}

/// Signing commits with an SSH key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SshSigning {
    /// The public key's path, relative to home (`.ssh/id_ed25519.pub`).
    pub key: String,
}

/// The person's Cachix cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BinaryCache {
    /// The cache's name: its address is `https://<name>.cachix.org`.
    pub name: String,
    /// Its signing key, exactly as cachix.org shows it:
    /// `<name>.cachix.org-1:<44 characters of base64>`.
    pub public_key: String,
    /// Who may read it.
    pub visibility: Visibility,
}

impl BinaryCache {
    /// The cache's address, as `nix.conf` names a substituter.
    #[must_use]
    pub fn substituter(&self) -> String {
        format!("https://{}", self.host())
    }

    /// The host the cache answers on — and the one a netrc names.
    #[must_use]
    pub fn host(&self) -> String {
        format!("{}.cachix.org", self.name)
    }

    /// Whether the cache is private: reading it needs the token.
    #[must_use]
    pub const fn is_private(&self) -> bool {
        matches!(self.visibility, Visibility::Private)
    }

    /// Why the cache is refused, as the JSON pointer of the value at fault —
    /// `None` when it is one. The Nix module asks the same three questions
    /// (`nix/modules/binary-cache.nix`).
    #[must_use]
    pub fn fault(&self) -> Option<&'static str> {
        if !is_cache_name(&self.name) {
            Some("/binary-cache/name")
        } else if !is_key_of(&self.name, &self.public_key) {
            Some("/binary-cache/public-key")
        } else {
            None
        }
    }
}

/// Who may read the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Anybody: Cachix's free plan. It receives only the distro's own
    /// outputs — never a path that carries the person's data, never an
    /// unfree program (`ronoctua_cachix::push`).
    Public,
    /// Only who holds the token: the machine's Nix reads it through a netrc
    /// root writes, and it may receive the system's whole closure.
    Private,
}

impl Visibility {
    /// Both, in the order the screen offers them.
    pub const ALL: [Self; 2] = [Self::Public, Self::Private];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

/// Whether `name` is a Cachix cache's name: lower-case letters, digits and
/// dashes, starting with a letter or a digit, at most 63 — the host label of
/// `<name>.cachix.org`. Pure.
#[must_use]
pub fn is_cache_name(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// Whether `key` is the signing key cachix.org shows for the cache `name`:
/// `<name>.cachix.org-1:` and an ed25519 public key in base64 — 43
/// characters of the alphabet and one `=`. Pure.
#[must_use]
pub fn is_key_of(name: &str, key: &str) -> bool {
    let Some(rest) = key
        .strip_prefix(name)
        .and_then(|r| r.strip_prefix(".cachix.org-1:"))
    else {
        return false;
    };
    let b = rest.as_bytes();
    b.len() == 44
        && b[43] == b'='
        && b[..43]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/')
}

impl Document for Options {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if let Some(s) = &self.ssh_signing {
            let relative = !s.key.starts_with('/') && !s.key.split('/').any(|p| p == "..");
            if !relative || !check::token(&s.key) {
                return Err(Refusal::new(
                    Why::Invalid,
                    "/ssh-signing/key",
                    s.key.as_str(),
                ));
            }
        }
        if let Some(cache) = &self.binary_cache
            && let Some(at) = cache.fault()
        {
            let found = if at.ends_with("/name") {
                cache.name.as_str()
            } else {
                cache.public_key.as_str()
            };
            return Err(Refusal::new(Why::Invalid, at, found));
        }
        if self.download_mirrors.len() > MIRRORS_MOST {
            return Err(Refusal::new(
                Why::Invalid,
                "/download-mirrors",
                format!(
                    "{} mirrors, at most {MIRRORS_MOST}",
                    self.download_mirrors.len()
                ),
            ));
        }
        for (n, url) in self.download_mirrors.iter().enumerate() {
            if !is_mirror(url) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("/download-mirrors/{n}"),
                    url.as_str(),
                ));
            }
            if self.download_mirrors[..n].contains(url) {
                return Err(Refusal::new(
                    Why::Duplicate,
                    format!("/download-mirrors/{n}"),
                    url.as_str(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: following is not the default — in a new file, in one that
    /// never said, in an older one that said on —, the person's "off" does
    /// not read back as off (an older file's silence must not be taken for
    /// it, nor the new field's for "on"), the off is written where nothing
    /// was turned — an older machine would refuse a file it reads today —, a
    /// turn of the switch does not tell an older machine the same, or a
    /// value that is not a switch is taken.
    #[test]
    fn following_is_on_unless_the_person_turned_it_off() {
        let fresh = Options::default();
        assert!(fresh.follows_releases());
        let text = crate::to_json(&fresh);
        assert!(!text.contains("distro-apps-"), "{text}");
        for older in [
            r#"{"format": 1, "ssh-signing": null}"#,
            r#"{"format": 1, "ssh-signing": null, "distro-apps-follow-releases": true}"#,
        ] {
            let o: Options = crate::parse(older.as_bytes()).unwrap();
            assert!(o.follows_releases(), "{older}");
            assert_eq!(
                crate::to_json(&o).contains("follow-releases"),
                older.contains("follow-releases"),
                "{older}: not kept as read"
            );
        }
        let mut off = Options::default();
        off.set_following(false);
        assert!(!off.follows_releases());
        let text = crate::to_json(&off);
        assert!(
            text.contains("\"distro-apps-keep-tested-pins\": true"),
            "{text}"
        );
        assert!(
            !text.contains("follow-releases"),
            "an older machine would follow: {text}"
        );
        assert_eq!(crate::parse::<Options>(text.as_bytes()).unwrap(), off);
        let mut on = off;
        on.set_following(true);
        assert!(on.follows_releases());
        let text = crate::to_json(&on);
        assert!(!text.contains("keep-tested-pins"), "{text}");
        assert!(
            text.contains("\"distro-apps-follow-releases\": true"),
            "an older machine would not follow: {text}"
        );
        // THE NEWER ANSWER DECIDES a file that says both.
        let both: Options = crate::parse(
            br#"{"format": 1, "ssh-signing": null, "distro-apps-follow-releases": true, "distro-apps-keep-tested-pins": true}"#,
        )
        .unwrap();
        assert!(!both.follows_releases());
        for bad in [
            &br#"{"format": 1, "ssh-signing": null, "distro-apps-follow-releases": "yes"}"#[..],
            br#"{"format": 1, "ssh-signing": null, "distro-apps-keep-tested-pins": 1}"#,
        ] {
            assert_eq!(crate::parse::<Options>(bad).unwrap_err().why, Why::Shape);
        }
    }

    /// FAILS IF: the mirrors are written while there are none — an older
    /// machine would refuse the file —, an address that could carry a secret
    /// or is not the web is taken, a duplicate or a ninth is taken, or a
    /// mirror's host is not what the consent records.
    #[test]
    fn mirrors_are_web_addresses_written_only_when_named() {
        assert!(!crate::to_json(&Options::default()).contains("download-mirrors"));
        let named = Options {
            download_mirrors: vec![
                "https://tarballs.example.org".to_owned(),
                "http://192.168.1.4:8080/files".to_owned(),
            ],
            ..Options::default()
        };
        let text = crate::to_json(&named);
        assert_eq!(crate::parse::<Options>(text.as_bytes()).unwrap(), named);
        for bad in [
            "ftp://x.org",
            "https://",
            "https://user:pw@localhost/files",
            "https://x.org/?token=1",
            "https://x.org/#a",
            "https://x .org",
            "file:///etc",
        ] {
            assert!(!is_mirror(bad), "{bad}");
        }
        let twice = Options {
            download_mirrors: vec!["https://a.org".to_owned(), "https://a.org".to_owned()],
            ..Options::default()
        };
        assert_eq!(twice.validate().unwrap_err().why, Why::Duplicate);
        let many = Options {
            download_mirrors: (0..=MIRRORS_MOST)
                .map(|n| format!("https://m{n}.org"))
                .collect(),
            ..Options::default()
        };
        assert!(many.validate().is_err());
        assert_eq!(
            mirror_host("http://192.168.1.4:8080/files"),
            Some("192.168.1.4")
        );
        assert_eq!(
            mirror_host("https://tarballs.example.org"),
            Some("tarballs.example.org")
        );
    }

    /// One case of the table both validators are held to.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        case: String,
        #[serde(rename = "binary-cache")]
        binary_cache: serde_json::Value,
        valid: bool,
    }

    /// FAILS IF: a cache the shared table calls valid is refused, or one it
    /// calls invalid is taken — the Nix module reads the same table
    /// (`binary-cache-declared-once`), so the two never disagree.
    #[test]
    fn the_cache_is_judged_as_the_shared_table_says() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../fixtures/binary-cache-cases.json"))
                .expect("the table");
        assert!(cases.len() >= 10, "the table lost its cases");
        assert!(cases.iter().any(|c| c.valid) && cases.iter().any(|c| !c.valid));
        for c in cases {
            let file = serde_json::json!({
                "format": crate::FORMAT,
                "ssh-signing": null,
                "binary-cache": c.binary_cache,
            });
            let read = crate::parse::<Options>(file.to_string().as_bytes());
            assert_eq!(read.is_ok(), c.valid, "{}: {read:?}", c.case);
            if let Err(r) = read {
                assert!(
                    matches!(r.why, Why::Invalid | Why::Shape),
                    "{}: {r}",
                    c.case
                );
            }
        }
    }

    /// FAILS IF: a file written before the cache existed stops reading, or
    /// the cache's address and host are not the name's.
    #[test]
    fn an_older_file_reads_as_no_cache_and_the_address_is_the_names() {
        let old: Options = crate::parse(br#"{"format": 1, "ssh-signing": null}"#).unwrap();
        assert_eq!(old.binary_cache, None);
        assert!(
            old.follows_releases(),
            "an older file that never said follows, the default"
        );
        let cache = BinaryCache {
            name: "someone".to_owned(),
            public_key: format!("someone.cachix.org-1:{}=", "Q".repeat(43)),
            visibility: Visibility::Private,
        };
        assert_eq!(cache.substituter(), "https://someone.cachix.org");
        assert_eq!(cache.host(), "someone.cachix.org");
        assert!(cache.is_private());
        assert_eq!(cache.fault(), None);
        for v in Visibility::ALL {
            assert_eq!(
                serde_json::to_string(&v).unwrap(),
                format!("\"{}\"", v.id())
            );
        }
    }
}
