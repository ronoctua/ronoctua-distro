//! The person's repository, as the installer asks for it: the address, the
//! access, the passphrase — and what the belongings' CLI answers about them.
//!
//! # Why the repository is mandatory now
//!
//! It is what makes the new machine the person's: their declarations
//! (`plain/person/`) are applied to the machine's flake BEFORE it is built,
//! so the programs they declared are in the first boot, and their belongings
//! are restored into the new home before anybody logs in. A machine installed
//! without it would be somebody else's until the person found the view.
//!
//! # The access is made here, and proved here
//!
//! An SSH address gets a deploy key made for this machine, in RAM
//! (`ronoctua-belongings keygen` under the installer's scratch home), shown as
//! text, as a QR code and with the host's "add a key" page when the host is
//! one whose page is known ([`add_key_page`]). The installer then PROBES the
//! repository with it until it both reads and writes (`ronoctua-belongings
//! probe --json`): a read-only key would let the restore run and fail at the
//! machine's first sync. An HTTPS address takes a token the person types,
//! probed the same way. The key never leaves the machine and never enters the
//! repository (`config/belongings-keys` is never kept).
//!
//! # The passphrase is proved too
//!
//! On a repository with something in it, the passphrase must open its
//! `key.age` (`verify-passphrase --json`) before the installer goes on — a
//! typo found after the disk was erased is a restore that fails with the
//! machine half made. On an EMPTY repository nothing can be opened, so the
//! passphrase is typed twice instead, and the installation makes the
//! repository (`init`) where it would have restored it.
//!
//! Everything here is pure: what the CLI's lines mean, which sentence says
//! a refusal, where a host's page is, the QR's modules. Who runs the CLI is
//! `crate::reach`.

use crate::key;
use ronoctua_belongings::Why;
use ronoctua_belongings::prefs::{Auth, Invalid};
use serde::Deserialize;

/// How a repository is reached — the address's kind decides, as the
/// belongings' own engine decides (`Named::prefs`).
#[must_use]
pub fn auth_of(address: &str) -> Auth {
    if address.trim().starts_with("https://") {
        Auth::Token
    } else {
        Auth::Ssh
    }
}

/// The host and the repository's path (`owner/name`, without `.git`) of an
/// address the belongings take; `None` for one they do not. Pure.
#[must_use]
pub fn host_and_path(address: &str) -> Option<(String, String)> {
    let address = address.trim();
    let host = ronoctua_belongings::prefs::host_of(address)?;
    let path = if let Some(rest) = address
        .strip_prefix("https://")
        .or_else(|| address.strip_prefix("ssh://"))
    {
        rest.split_once('/')?.1
    } else {
        // `git@host:owner/name.git`
        address.split_once('@')?.1.split_once(':')?.1
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    (!path.is_empty()).then(|| (host, path.to_owned()))
}

/// A forge whose pages are known.
struct Forge {
    /// Its host, as [`host_and_path`] names it.
    host: &'static str,
    /// Where a deploy key is added (`{path}`: the repository's).
    key_page: &'static str,
    /// Where a token is made.
    token_page: &'static str,
    /// The sentence naming exactly what a token there needs: read and write
    /// on this repository, through git, and nothing else.
    token_rights: &'static str,
}

/// The hosts whose pages are known: where a deploy key is added, where a
/// token is made, and what it needs. Public pages of the three forges
/// people keep a private repository in; any other host is told in words.
const FORGES: [Forge; 3] = [
    // A FINE-GRAINED token: "Contents" is git's read and write, and GitHub
    // adds "Metadata" read-only by itself to any token that names a
    // repository.
    Forge {
        host: "github.com",
        key_page: "https://github.com/{path}/settings/keys/new",
        token_page: "https://github.com/settings/personal-access-tokens/new",
        token_rights: key::INST_BELONGINGS_TOKEN_RIGHTS_GITHUB,
    },
    // `write_repository` is git's pull AND push (GitLab's own words); a
    // token with only `read_repository` would let the restore run and fail
    // at the first sync.
    Forge {
        host: "gitlab.com",
        key_page: "https://gitlab.com/{path}/-/settings/repository",
        token_page: "https://gitlab.com/-/user_settings/personal_access_tokens",
        token_rights: key::INST_BELONGINGS_TOKEN_RIGHTS_GITLAB,
    },
    // Forgejo (Codeberg runs 16): the access narrowed to "Specific
    // repositories", and the `repository` scope read and write.
    Forge {
        host: "codeberg.org",
        key_page: "https://codeberg.org/{path}/settings/keys",
        token_page: "https://codeberg.org/user/settings/applications",
        token_rights: key::INST_BELONGINGS_TOKEN_RIGHTS_CODEBERG,
    },
];

/// The known forge `address` is on.
fn forge_of(address: &str) -> Option<(&'static Forge, String)> {
    let (host, path) = host_and_path(address)?;
    FORGES.iter().find(|f| f.host == host).map(|f| (f, path))
}

/// Where the person adds this machine's key, for a host whose page is
/// known. Pure.
#[must_use]
pub fn add_key_page(address: &str) -> Option<String> {
    forge_of(address).map(|(f, path)| f.key_page.replace("{path}", &path))
}

/// Where the person makes a token, for a host whose page is known. Pure.
#[must_use]
pub fn token_page(address: &str) -> Option<String> {
    forge_of(address).map(|(f, _)| f.token_page.to_owned())
}

/// The sentence saying what a token needs on the host `address` is on,
/// for a host whose pages are known. Pure.
#[must_use]
pub fn token_rights(address: &str) -> Option<&'static str> {
    forge_of(address).map(|(f, _)| f.token_rights)
}

/// What `ronoctua-belongings probe --json` prints, one line — the shape of
/// `ronoctua_belongings::engine::Probed`, which this crate does not link
/// (it is the `crypt` engine's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Probed {
    /// The access reads the repository.
    pub read: bool,
    /// The access may push to it.
    pub write: bool,
    /// The repository has nothing yet.
    pub empty: bool,
    /// Why one of the two failed.
    pub why: Option<Why>,
}

impl Probed {
    /// Whether the access is the one the installation needs: it reads AND
    /// writes.
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.read && self.write
    }

    /// The sentence the access screen shows for it.
    #[must_use]
    pub const fn status_key(&self) -> &'static str {
        match (self.read, self.write) {
            (true, true) => key::INST_ACCESS_READY,
            (true, false) => key::INST_ACCESS_READS,
            _ => key::INST_ACCESS_WAITING,
        }
    }
}

/// One answer of the CLI with `--json`: what it found, or `{"refused":
/// "<why>"}`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Answer<T> {
    Refused { refused: Why },
    Found(T),
}

/// The probe's answer, out of the CLI's last line on stdout. Pure.
///
/// # Errors
///
/// The refusal the CLI printed, or [`Why::Failed`] for a line that is not
/// an answer.
pub fn probed_of(stdout: &str) -> Result<Probed, Why> {
    match last_answer::<Probed>(stdout) {
        Some(Answer::Found(p)) => Ok(p),
        Some(Answer::Refused { refused }) => Err(refused),
        None => Err(Why::Failed),
    }
}

/// What `verify-passphrase --json` found: the repository's format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct Opened {
    format: u32,
}

/// The passphrase's verification, out of the CLI's last line on stdout:
/// the repository's format when the passphrase opens it. Pure.
///
/// # Errors
///
/// The refusal the CLI printed (`wrong-passphrase` first among them), or
/// [`Why::Failed`] for a line that is not an answer.
pub fn verified_of(stdout: &str) -> Result<u32, Why> {
    match last_answer::<Opened>(stdout) {
        Some(Answer::Found(o)) => Ok(o.format),
        Some(Answer::Refused { refused }) => Err(refused),
        None => Err(Why::Failed),
    }
}

/// The last line of `stdout` that is an answer.
fn last_answer<T: serde::de::DeserializeOwned>(stdout: &str) -> Option<Answer<T>> {
    stdout
        .lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Answer<T>>(l.trim()).ok())
}

/// The sentence of one of the belongings' refusals — the same sentences
/// the belongings' view says them with.
#[must_use]
pub const fn refusal_key(why: Why) -> &'static str {
    match why {
        Why::NoRepository => key::BELONGINGS_REFUSED_NO_REPOSITORY,
        Why::NoPassphrase => key::BELONGINGS_REFUSED_NO_PASSPHRASE,
        Why::WrongPassphrase => key::BELONGINGS_REFUSED_WRONG_PASSPHRASE,
        Why::NeedsAccess => key::BELONGINGS_REFUSED_NEEDS_ACCESS,
        Why::Refused => key::BELONGINGS_REFUSED_REFUSED,
        Why::Unreachable => key::BELONGINGS_REFUSED_UNREACHABLE,
        Why::Raced => key::BELONGINGS_REFUSED_RACED,
        Why::NotOurs => key::BELONGINGS_REFUSED_NOT_OURS,
        Why::Empty => key::BELONGINGS_REFUSED_EMPTY,
        Why::Corrupt => key::BELONGINGS_REFUSED_CORRUPT,
        Why::Undecided => key::BELONGINGS_REFUSED_UNDECIDED,
        Why::NoSession => key::BELONGINGS_REFUSED_NO_SESSION,
        Why::NoTools => key::BELONGINGS_REFUSED_NO_TOOLS,
        Why::Failed => key::BELONGINGS_REFUSED_FAILED,
        Why::Busy => key::BELONGINGS_REFUSED_BUSY,
        Why::NotLet => key::BELONGINGS_REFUSED_NOT_LET,
        Why::Newer => key::BELONGINGS_REFUSED_NEWER,
        Why::PlainTooLarge => key::BELONGINGS_REFUSED_PLAIN_TOO_LARGE,
        Why::NotPostQuantum => key::BELONGINGS_REFUSED_NOT_POST_QUANTUM,
        Why::ChoiceUnread => key::BELONGINGS_REFUSED_CHOICE_UNREAD,
        Why::CopyUnwritten => key::BELONGINGS_REFUSED_COPY_UNWRITTEN,
    }
}

/// The sentence of an address the belongings refuse — the view's.
#[must_use]
pub const fn invalid_key(why: Invalid) -> &'static str {
    match why {
        Invalid::Empty => key::BELONGINGS_INVALID_EMPTY,
        Invalid::CredentialsInAddress => key::BELONGINGS_INVALID_CREDENTIALS_IN_ADDRESS,
        Invalid::PlainHttp => key::BELONGINGS_INVALID_PLAIN_HTTP,
        Invalid::NotRemote => key::BELONGINGS_INVALID_NOT_REMOTE,
        Invalid::UnnamedHost => key::BELONGINGS_INVALID_UNNAMED_HOST,
        Invalid::Unknown => key::BELONGINGS_INVALID_UNKNOWN,
    }
}

/// A QR code's modules: `side × side`, `true` = dark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qr {
    /// Modules per edge.
    pub side: usize,
    /// The modules, in reading order.
    pub dark: Vec<bool>,
}

impl Qr {
    /// The QR of `text`; `None` when it does not fit one. Pure.
    ///
    /// Correction `L`: it is read off a screen, a hand's length away, and
    /// a higher level would spend modules — a larger code for the same key
    /// — on damage a screen never has.
    #[must_use]
    pub fn of(text: &str) -> Option<Self> {
        let code = qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::L).ok()?;
        let side = code.width();
        let dark = code
            .to_colors()
            .into_iter()
            .map(|c| c == qrcode::Color::Dark)
            .collect();
        Some(Self { side, dark })
    }

    /// Whether the module at `(x, y)` is dark; outside the code (the quiet
    /// zone) it is light.
    #[must_use]
    pub fn is_dark(&self, x: isize, y: isize) -> bool {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return false;
        };
        x < self.side && y < self.side && self.dark[y * self.side + x]
    }
}

/// The quiet zone around the code, in modules: two, not the standard's four
/// — the code is drawn on a terminal whose own background frames it, and
/// four would cost eight rows of a console that has thirty.
pub const QUIET: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_names_its_host_and_its_repository() {
        for (address, host, path) in [
            (
                "git@github.com:me/belongings.git",
                "github.com",
                "me/belongings",
            ),
            (
                "ssh://git@gitlab.com/group/sub/belongings.git",
                "gitlab.com",
                "group/sub/belongings",
            ),
            (
                "https://codeberg.org/me/belongings",
                "codeberg.org",
                "me/belongings",
            ),
        ] {
            assert_eq!(
                host_and_path(address),
                Some((host.to_owned(), path.to_owned())),
                "{address}"
            );
        }
        assert_eq!(host_and_path("git@github.com:"), None);
        assert_eq!(host_and_path("/srv/b.git"), None);
    }

    /// FAILS IF: the key's page points anywhere but the repository's own
    /// deploy-key settings on the three known forges, or a page is guessed
    /// for a host nobody knows.
    #[test]
    fn the_add_key_page_is_the_repositorys_own() {
        assert_eq!(
            add_key_page("git@github.com:me/belongings.git").as_deref(),
            Some("https://github.com/me/belongings/settings/keys/new")
        );
        assert_eq!(
            add_key_page("ssh://git@gitlab.com/me/belongings.git").as_deref(),
            Some("https://gitlab.com/me/belongings/-/settings/repository")
        );
        assert_eq!(
            add_key_page("git@codeberg.org:me/b.git").as_deref(),
            Some("https://codeberg.org/me/b/settings/keys")
        );
        assert_eq!(add_key_page("git@git.example.org:me/b.git"), None);
        assert!(
            token_page("https://github.com/me/b.git")
                .is_some_and(|p| p.starts_with("https://github.com/"))
        );
        assert_eq!(token_page("https://git.example.org/me/b.git"), None);
        // WHAT THE TOKEN NEEDS is said for each known forge, its own.
        assert_eq!(
            token_rights("https://github.com/me/b.git"),
            Some(key::INST_BELONGINGS_TOKEN_RIGHTS_GITHUB)
        );
        assert_eq!(
            token_rights("https://gitlab.com/me/b.git"),
            Some(key::INST_BELONGINGS_TOKEN_RIGHTS_GITLAB)
        );
        assert_eq!(
            token_rights("https://codeberg.org/me/b.git"),
            Some(key::INST_BELONGINGS_TOKEN_RIGHTS_CODEBERG)
        );
        assert_eq!(token_rights("https://git.example.org/me/b.git"), None);
        assert_eq!(auth_of(" https://github.com/me/b.git"), Auth::Token);
        assert_eq!(auth_of("git@github.com:me/b.git"), Auth::Ssh);
    }

    /// FAILS IF: the CLI's lines are misread — a probe that reads and
    /// writes taken for one that does not, an empty repository missed, a
    /// refusal taken for an answer, or the last line not the one read.
    #[test]
    fn the_belongings_answers_are_read_from_their_last_line() {
        // The line the belongings' own test pins (`a_probe_crosses_one_line_whole`).
        let p = probed_of(r#"{"read":true,"write":false,"empty":true,"why":"refused"}"#).unwrap();
        assert!(p.read && !p.write && p.empty && !p.ready());
        assert_eq!(p.why, Some(Why::Refused));
        assert_eq!(p.status_key(), key::INST_ACCESS_READS);
        let ready =
            probed_of("noise\n{\"read\":true,\"write\":true,\"empty\":false,\"why\":null}\n")
                .unwrap();
        assert!(ready.ready());
        assert_eq!(ready.status_key(), key::INST_ACCESS_READY);
        assert_eq!(
            Probed::default().status_key(),
            key::INST_ACCESS_WAITING,
            "nothing read"
        );
        assert_eq!(probed_of(r#"{"refused":"no-tools"}"#), Err(Why::NoTools));
        assert_eq!(probed_of(""), Err(Why::Failed));
        assert_eq!(verified_of(r#"{"format":2}"#), Ok(2));
        assert_eq!(
            verified_of(r#"{"refused":"wrong-passphrase"}"#),
            Err(Why::WrongPassphrase)
        );
        assert_eq!(verified_of("garbage"), Err(Why::Failed));
    }

    #[test]
    fn a_deploy_key_fits_a_qr_with_its_three_eyes() {
        let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOv6S1n3Wq0pXqk6Q7a1e3zB0b4yT2Yx9cJ9mVhK0aQ1 ronoctua-belongings";
        let qr = Qr::of(key).expect("a deploy key fits");
        assert!(qr.side >= 21 && qr.side % 2 == 1, "{}", qr.side);
        assert_eq!(qr.dark.len(), qr.side * qr.side);
        let side = isize::try_from(qr.side).unwrap();
        let eye = |ox: isize, oy: isize| {
            qr.is_dark(ox, oy) && qr.is_dark(ox + 6, oy) && !qr.is_dark(ox + 1, oy + 1)
        };
        assert!(eye(0, 0) && eye(side - 7, 0) && eye(0, side - 7));
        assert!(
            !qr.is_dark(-1, 0) && !qr.is_dark(0, side),
            "the quiet zone is light"
        );
    }
}
