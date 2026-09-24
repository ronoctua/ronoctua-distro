//! Downloading as the person, instead of as nobody.
//!
//! # What is actually possible, measured against the pinned yt-dlp
//!
//! Four things need an account: age-restricted video, private video, a members'
//! club, and 1080p Premium. **OAuth2 is not one of the ways to get there any
//! more.** yt-dlp 2026.07.04 refuses it in its own source —
//! `yt_dlp/extractor/youtube/_base.py:655`:
//!
//! ```text
//! if username.startswith('oauth'):
//!     raise ExtractorError(
//!         f'Login with OAuth is no longer supported. {self._youtube_login_hint}', ...)
//! ```
//!
//! and the hint it points at says cookies. Password login is refused on the
//! next line. So the account half of this file is **cookies**, because that is
//! what is left.
//!
//! The other half — proving the request comes from a real device, which is the
//! job OAuth used to do — is now the **PO token**. yt-dlp carries the whole
//! framework for it (`extractor/youtube/pot/`) and **no minter**: a token is
//! supplied by hand or by a plugin. This file reads one from a file if it is
//! there, and says nothing when it is not.
//!
//! # Anonymous is the default, and it stays the default
//!
//! Nothing here happens unless a request said [`Identity::Mine`]. The cost of
//! the other one is anonymity, and a cost that is paid by a setting somewhere
//! else is a cost nobody notices paying.
//!
//! # Where the cookies come from is a MACHINE decision
//!
//! Which browser, or which file, is a fact about this installation — so it is
//! declared in `nix/modules/transfers.nix` and arrives as environment, exactly
//! like yt-dlp's own path. Nothing about it is in this repository: a cookie jar
//! is a session, and invariant 13 puts a session in the same sentence as a
//! password.

use crate::job::Identity;
use std::path::PathBuf;

/// Where the person's session lives, when a download asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cookies {
    /// Nowhere. Every download is anonymous, and asking for the other one
    /// says so instead of quietly doing nothing.
    Nowhere,
    /// Out of a browser's own store, by yt-dlp's `BROWSER[+KEYRING]` spelling.
    Browser(String),
    /// A Netscape-format file, exported once.
    File(PathBuf),
}

impl Cookies {
    /// What this installation is set to, from the environment.
    ///
    /// **The file wins**, and that is not arbitrary: reading a browser's live
    /// store is the convenient answer and the risky one — YouTube rotates the
    /// cookies it hands out, and a download that uses the browser's current
    /// ones can invalidate them, which shows up as the browser logging itself
    /// out for no visible reason. Somebody who went to the trouble of
    /// exporting a file meant it.
    #[must_use]
    pub fn here() -> Self {
        Self::of(
            std::env::var("RONOCTUA_COOKIES_FILE").ok().as_deref(),
            std::env::var("RONOCTUA_COOKIES_BROWSER").ok().as_deref(),
        )
    }

    /// The same decision, over values a test can hand it.
    fn of(file: Option<&str>, browser: Option<&str>) -> Self {
        if let Some(path) = file.map(str::trim).filter(|v| !v.is_empty()) {
            return Self::File(PathBuf::from(path));
        }
        if let Some(spec) = browser.map(str::trim).filter(|v| !v.is_empty()) {
            return Self::Browser(spec.to_owned());
        }
        Self::Nowhere
    }

    /// The yt-dlp flags for it, or nothing.
    fn flags(&self) -> Vec<String> {
        match self {
            Self::Nowhere => Vec::new(),
            Self::Browser(spec) => vec!["--cookies-from-browser".to_owned(), spec.clone()],
            Self::File(path) => vec!["--cookies".to_owned(), path.to_string_lossy().into_owned()],
        }
    }

    /// The file a program that is not yt-dlp can read.
    ///
    /// aria2 takes `--load-cookies` and knows the same Netscape format, but it
    /// cannot open a browser's store — so an authenticated plain download works
    /// with a file and not with a browser. Saying which is why this returns an
    /// `Option` rather than a path that might be a lie.
    #[must_use]
    pub fn as_file(&self) -> Option<&std::path::Path> {
        match self {
            Self::File(path) => Some(path),
            Self::Nowhere | Self::Browser(_) => None,
        }
    }
}

/// Where a hand-supplied PO token is kept.
///
/// Under `XDG_STATE_HOME` because it is machine-local and outlives a reboot,
/// and NEVER in the repository: it identifies a session (invariant 13).
#[must_use]
pub fn token_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
        })?;
    Some(base.join("ronoctua").join("po-token"))
}

/// The tokens in that file, one per line.
///
/// Each line is yt-dlp's own `CLIENT.CONTEXT+TOKEN` spelling, so what is
/// written there can be pasted straight from yt-dlp's documentation without a
/// translation step in between — and a translation step is a thing that can be
/// wrong.
fn tokens_in(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        // A line with no `+` is not a token, it is a note somebody left. Taking
        // it would make yt-dlp warn about a format it cannot read, once per
        // download, about a file this program chose to read.
        .filter(|line| line.contains('+'))
        .map(ToOwned::to_owned)
        .collect()
}

/// What `--extractor-args` should carry, or nothing.
fn extractor_args(tokens: &[String]) -> Vec<String> {
    if tokens.is_empty() {
        return Vec::new();
    }
    // ONE argument with `;` between, and not one flag per token: yt-dlp keys
    // `--extractor-args` by extractor, and a second one for `youtube` replaces
    // the first instead of adding to it.
    let joined = tokens
        .iter()
        .map(|t| format!("po_token={t}"))
        .collect::<Vec<_>>()
        .join(";");
    vec!["--extractor-args".to_owned(), format!("youtube:{joined}")]
}

/// The PO token minter's flags — moved to `ronoctua-ytdlp`, because the
/// player's ytdl hook needs the same two and a copy would drift.
pub use ronoctua_ytdlp::pot_flags;

/// Everything yt-dlp needs to be the person, for one request.
///
/// Empty for [`Identity::Anonymous`], which is the default and the whole point.
#[must_use]
pub fn yt_dlp_flags(identity: Identity, cookies: &Cookies) -> Vec<String> {
    if identity == Identity::Anonymous {
        return Vec::new();
    }
    let mut out = cookies.flags();
    let tokens = token_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| tokens_in(&text))
        .unwrap_or_default();
    out.extend(extractor_args(&tokens));
    out
}

/// Why an identified request cannot be made, when it cannot.
///
/// A sentence and not a silent fall back to anonymous: falling back would
/// download the trailer instead of the video, or nothing at all, and blame the
/// site. The text is the queue's, so it goes on the row.
#[must_use]
pub fn refusal(identity: Identity, cookies: &Cookies) -> Option<&'static str> {
    if identity == Identity::Anonymous {
        return None;
    }
    match cookies {
        Cookies::Nowhere => Some(
            "this installation has nowhere to take the cookies from; \
             veja ronoctua.transfers.cookies",
        ),
        Cookies::Browser(_) | Cookies::File(_) => None,
    }
}

// ---------------------------------------------------------------------------
// The cookie jar, for the engine that cannot read a browser.
//
// aria2 knows `--load-cookies` and the same Netscape format, but its RPC takes
// that option only for the whole session and not per download — and a queue
// where ONE authenticated link makes every other one carry the person's
// cookies is a queue that leaks identity into requests nobody asked to
// identify. What it does take per download is a header, so the jar is read
// here and exactly the cookies that belong to THAT address are sent.
// ---------------------------------------------------------------------------

/// The scheme, host and path of an address, as far as a cookie cares.
///
/// A parser of three fields and not a URL crate: what is needed is the host to
/// match a domain, the path to match a prefix, and whether it is https. A
/// dependency for that would be a dependency in a lock file every crate in the
/// workspace shares.
fn parts_of(url: &str) -> Option<(bool, String, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let secure = scheme.eq_ignore_ascii_case("https");
    let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, h)| h)
        .split(':')
        .next()?
        .to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some((secure, host, format!("/{path}")))
}

/// Whether a jar's domain field covers a host.
fn covers(domain: &str, host: &str) -> bool {
    let domain = domain.trim_start_matches('.').to_ascii_lowercase();
    if domain.is_empty() {
        return false;
    }
    // A LEADING DOT MEANS SUBDOMAINS, and stripping it is not enough on its
    // own: without the dot in the suffix test, `youtube.com` would also match
    // `notyoutube.com`, and the person's cookies would go to somebody else.
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// The `Cookie:` header for one address, out of a Netscape jar.
///
/// `now` is a parameter so the expiry rule can be tested without waiting.
fn header_at(jar: &str, url: &str, now: u64) -> Option<String> {
    let (secure, host, path) = parts_of(url)?;
    let mut pairs = Vec::new();
    for line in jar.lines() {
        // `#HttpOnly_` is a real prefix on a real line, and everything else
        // beginning with `#` is a comment. Treating the first as a comment
        // drops exactly the cookies that carry a session.
        let line = line
            .strip_prefix("#HttpOnly_")
            .unwrap_or_else(|| line.trim_start());
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let field: Vec<&str> = line.split('\t').collect();
        if field.len() < 7 {
            continue;
        }
        let (domain, cookie_path, only_secure, expires, name, value) =
            (field[0], field[2], field[3], field[4], field[5], field[6]);
        if !covers(domain, &host) || !path.starts_with(cookie_path) {
            continue;
        }
        if only_secure.eq_ignore_ascii_case("TRUE") && !secure {
            continue;
        }
        // Zero is a SESSION cookie and never an expired one. Comparing it as a
        // date drops every session cookie there is, which for a logged-in site
        // is most of what matters.
        let when: u64 = expires.parse().unwrap_or(0);
        if when != 0 && when <= now {
            continue;
        }
        pairs.push(format!("{name}={value}"));
    }
    (!pairs.is_empty()).then(|| format!("Cookie: {}", pairs.join("; ")))
}

/// The header aria2 should carry for this address, when it should carry one.
///
/// `None` for an anonymous request, for a jar that is not a file, and for a
/// jar with nothing that belongs to this address — the last of which is not a
/// failure, it is a site the person is not logged in to.
#[must_use]
pub fn cookie_header(identity: Identity, cookies: &Cookies, url: &str) -> Option<String> {
    if identity == Identity::Anonymous {
        return None;
    }
    let jar = std::fs::read_to_string(cookies.as_file()?).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    header_at(&jar, url, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_extractor_keys_never_collide() {
        // yt-dlp keys `--extractor-args` by extractor. The minter writes
        // `youtubepot-bgutilscript:` and the hand-pasted token writes
        // `youtube:`, so both survive; two flags for one key would not.
        let minter = ronoctua_ytdlp::pot_flags_of(Some("/x"), Some("/y"));
        let manual = extractor_args(&tokens_in("web.gvs+ABC\n"));
        let key = |v: &[String]| {
            v.iter()
                .position(|a| a == "--extractor-args")
                .map(|i| v[i + 1].split(':').next().unwrap_or("").to_owned())
        };
        assert_eq!(key(&minter).as_deref(), Some("youtubepot-bgutilscript"));
        assert_eq!(key(&manual).as_deref(), Some("youtube"));
        assert_ne!(key(&minter), key(&manual));
    }

    #[test]
    fn anonymous_carries_nothing_at_all() {
        // THE DEFAULT, and the one that must never grow a flag by accident: a
        // request that did not ask to be identified must not send a cookie,
        // a token or anything else that says who is asking.
        let browser = Cookies::Browser("chrome+basictext".to_owned());
        assert!(yt_dlp_flags(Identity::Anonymous, &browser).is_empty());
        assert!(yt_dlp_flags(Identity::Anonymous, &Cookies::Nowhere).is_empty());
        assert_eq!(refusal(Identity::Anonymous, &Cookies::Nowhere), None);
    }

    #[test]
    fn the_file_wins_over_the_browser() {
        // Reading the browser's live store is the convenient answer and the
        // risky one: YouTube rotates what it hands out, and using the current
        // cookies can invalidate them — which looks like the browser logging
        // itself out. Somebody who exported a file meant it.
        assert_eq!(
            Cookies::of(Some("/home/someone/cookies.txt"), Some("chrome")),
            Cookies::File(PathBuf::from("/home/someone/cookies.txt"))
        );
        assert_eq!(
            Cookies::of(None, Some("chrome+basictext")),
            Cookies::Browser("chrome+basictext".to_owned())
        );
        // An empty variable is not a value. Systemd writes one for an option
        // that resolved to nothing, and treating it as a path would hand
        // yt-dlp `--cookies ""`.
        assert_eq!(Cookies::of(Some(""), Some("")), Cookies::Nowhere);
        assert_eq!(Cookies::of(Some("  "), None), Cookies::Nowhere);
    }

    #[test]
    fn a_browser_and_a_file_produce_the_flags_yt_dlp_documents() {
        let browser = Cookies::Browser("chrome+basictext".to_owned());
        assert_eq!(
            browser.flags(),
            ["--cookies-from-browser", "chrome+basictext"]
        );
        // `basictext` is not a detail: this distro forces
        // `--password-store=basic` on Chrome, and that is exactly the keyring
        // yt-dlp calls `basictext`. Without it, decrypting the store needs a
        // keyring the distro deliberately does not have.
        assert!(browser.as_file().is_none());

        let file = Cookies::File(PathBuf::from("/x/cookies.txt"));
        assert_eq!(file.flags(), ["--cookies", "/x/cookies.txt"]);
        assert_eq!(file.as_file(), Some(std::path::Path::new("/x/cookies.txt")));
    }

    #[test]
    fn the_token_file_is_read_the_way_yt_dlp_spells_it() {
        let text = "\
# a comment, and a note that is not a token
web.gvs+MnQ_ABCDEF
tv.player+ZZZ

a loose line with nothing more
mweb.gvs+QQQ
";
        assert_eq!(
            tokens_in(text),
            ["web.gvs+MnQ_ABCDEF", "tv.player+ZZZ", "mweb.gvs+QQQ"]
        );
        // ONE argument, with `;` between: yt-dlp keys `--extractor-args` by
        // extractor, so a second `youtube:` one REPLACES the first instead of
        // adding to it — and the second token would silently not exist.
        assert_eq!(
            extractor_args(&tokens_in(text)),
            [
                "--extractor-args",
                "youtube:po_token=web.gvs+MnQ_ABCDEF;po_token=tv.player+ZZZ;po_token=mweb.gvs+QQQ"
            ]
        );
        assert!(extractor_args(&[]).is_empty());
        assert!(tokens_in("# so comentario\n").is_empty());
    }

    /// A jar with one cookie per line, in the shape a browser exports.
    const JAR: &str = "\
# Netscape HTTP Cookie File
.youtube.com\tTRUE\t/\tTRUE\t2000000000\tSID\tabc
#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t0\tHSID\tdef
.youtube.com\tTRUE\t/\tTRUE\t1\tOLD\texpired
.othersite.com\tTRUE\t/\tTRUE\t2000000000\tFOREIGN\tno
.youtube.com\tTRUE\t/premium\tTRUE\t2000000000\tPATHONLY\tyes
.youtube.com\tTRUE\t/\tFALSE\t2000000000\tNOTLS\tmaybe
";

    #[test]
    fn the_jar_gives_up_only_what_belongs_to_that_address() {
        let header =
            header_at(JAR, "https://www.youtube.com/watch?v=x", 1_000_000_000).expect("header");
        assert!(header.starts_with("Cookie: "));
        assert!(header.contains("SID=abc"), "{header}");
        // `#HttpOnly_` is a PREFIX on a real line, not a comment. Reading it as
        // one drops exactly the cookies that carry a session.
        assert!(header.contains("HSID=def"), "{header}");
        // Expired, another site's, and a path that does not match.
        assert!(!header.contains("OLD"), "{header}");
        assert!(!header.contains("FOREIGN"), "{header}");
        assert!(!header.contains("PATHONLY"), "{header}");
    }

    #[test]
    fn a_secure_cookie_never_goes_over_plain_http() {
        let over_http = header_at(JAR, "http://www.youtube.com/", 1_000_000_000).expect("header");
        assert!(!over_http.contains("SID=abc"), "{over_http}");
        assert!(over_http.contains("NOTLS"), "{over_http}");
    }

    #[test]
    fn a_domain_is_a_suffix_and_not_a_substring() {
        // Without the dot in the suffix test, `youtube.com` also matches
        // `notyoutube.com` — and the person's session goes to whoever
        // registered it.
        assert!(covers(".youtube.com", "www.youtube.com"));
        assert!(covers(".youtube.com", "youtube.com"));
        assert!(!covers(".youtube.com", "notyoutube.com"));
        assert!(!covers(".youtube.com", "youtube.com.br"));
        assert!(header_at(JAR, "https://notyoutube.com/", 1_000_000_000).is_none());
    }

    #[test]
    fn an_address_is_read_down_to_the_three_things_a_cookie_cares_about() {
        assert_eq!(
            parts_of("https://www.Example.com:8443/a/b?c=d"),
            Some((true, "www.example.com".to_owned(), "/a/b?c=d".to_owned()))
        );
        // Userinfo is stripped, or the host would carry the credentials with it
        // and no cookie would ever match. The host in this fixture has no dot
        // on purpose: `secrets-out-of-repo` reads a name, an at sign and a
        // dotted domain as an address it must refuse, and a test fixture is not
        // worth teaching it an exception.
        assert_eq!(
            parts_of("https://someone:password@localhost:8443/a"),
            Some((true, "localhost".to_owned(), "/a".to_owned()))
        );
        assert_eq!(
            parts_of("http://exemplo"),
            Some((false, "exemplo".to_owned(), "/".to_owned()))
        );
        assert_eq!(parts_of("not-an-address"), None);
    }

    #[test]
    fn asking_to_be_identified_with_nowhere_to_get_it_says_so() {
        // NOT a silent fall back to anonymous: that downloads the trailer
        // instead of the video, or nothing at all, and blames the site.
        assert!(refusal(Identity::Mine, &Cookies::Nowhere).is_some());
        assert_eq!(
            refusal(Identity::Mine, &Cookies::Browser("chrome".to_owned())),
            None
        );
    }
}
