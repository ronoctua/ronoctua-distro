//! What a sync that brought a target's files here offers to turn on — and
//! saying yes to it (feature `offers`).
//!
//! # Offered once, when the files that make it possible arrive
//!
//! A target's files can make something possible that the machine does not
//! do by itself: with the person's SSH key here, git can sign their commits
//! with it; with their registered security keys here
//! (`~/.config/Yubico/u2f_keys`), a key can answer for the password. A sync
//! (or the installer's restore) that BROUGHT those very files — a half of a
//! key pair git may sign with ([`signing_keys`]), the security keys' file
//! ([`U2F_KEYS`]) — says so in its outcome (`Outcome::Synced.offers`), and
//! the door draws each offer with "no" first. Another file of the same
//! target (a new `known_hosts` line, a git alias) offers nothing. So a "no"
//! needs no memory: the offer comes back only when the key itself changes
//! on another machine — and never for what the person already turned on
//! (`options.json`), nor SSH signing where git signs already, with SSH or
//! with OpenPGP: a yes would replace their setup ([`Signing::set_up`]).
//!
//! # Saying yes
//!
//! - **SSH signing** ([`accept`]): the settings go into a git file of their
//!   own INSIDE `~/.ssh` ([`SIGNING_INCLUDE`]: `gpg.format ssh`,
//!   `user.signingkey ~/.ssh/<key>.pub`, `commit.gpgsign`, `tag.gpgsign`, and
//!   the signers the person trusts), and the global `~/.gitconfig` only
//!   INCLUDES it (`include.path`). The include file is the `ssh` target's
//!   and travels with the key; `.gitconfig` is the `git` target's and
//!   travels on its own. On a machine where the key is not — `~/.ssh` "not
//!   here", or dropped everywhere — the include file is not either, and git
//!   silently reads nothing from an include that is missing: commits there
//!   go unsigned, instead of every `git commit` failing on a key that is not
//!   there. When git knows the person's address, their key joins the
//!   signers they trust (`~/.config/git/allowed_signers`, the `git`
//!   target's, a public key), so their own commits show as verified.
//! - **Security keys**: the keys the person registered on their other
//!   machines (their `u2f_keys`, [`U2F_KEYS`]) that this machine's root list
//!   lacks ([`keys_to_bring`]) go into that list — `pam_u2f` trusts it
//!   alone, for sudo, always `sufficient`, and at once (`nix/modules/
//!   yubikey.nix`). Writing it takes root, so the yes is the sudo door's
//!   (`u2f-keys`, the shell's, in the same gesture), never this module's:
//!   [`accept`] refuses it. Nothing is recorded: the offer is over when root's
//!   list has the keys, on each machine for itself.
//!
//! - **VS Code's extensions** ([`EXTENSIONS_LIST`], the `vscode` target's
//!   list of the person's extensions, `ronoctua_vscode`): a sync that brought
//!   the list, with ids this machine has not installed, offers to install
//!   them — where VS Code is ([`vscode_here`]), naming the Marketplace hosts
//!   the install reaches ([`extension_hosts`]). The yes lets exactly those
//!   hosts in the person's `consent.json` and is the queue's — the install
//!   takes minutes of network — so the door puts `order::Op::VscodeExtensions`
//!   on the queue, whose child runs [`install_extensions`]: a `Pass` for
//!   every host, then `code --install-extension`, one id at a time, under
//!   its own lock ([`EXTENSIONS_LOCK`]), writing that it began and what
//!   entered, what failed and why ([`ExtensionsDone`], the file is the
//!   broadcast). [`accept`] refuses it. Nothing is recorded: the offer is
//!   over when the extensions are here, and while some are missing the page
//!   keeps a row that installs them.
//!
//! SSH signing is recorded in `options.json` — the person's data, in the
//! plain part — so no other machine offers it again.

use std::path::{Path, PathBuf};

use ronoctua_person::options::{Options, SshSigning};

use crate::kept::{self, Offering};
use crate::order::{Beat, Phase};
use crate::places::Places;
use crate::status::Offer;
use crate::{Refusal, Why};

/// The keys a person signs with, the preferred first: what `ssh-keygen`
/// makes by default, the hardware ones beside them.
pub const SIGNING_KEYS: [&str; 5] = [
    "id_ed25519",
    "id_ed25519_sk",
    "id_ecdsa_sk",
    "id_ecdsa",
    "id_rsa",
];

/// The folder of the person's SSH, logical.
pub const SSH_DIR: &str = "home/.ssh";

/// The person's registered security keys, logical — the list the keys come
/// from, never what `pam_u2f` trusts.
pub const U2F_KEYS: &str = "home/.config/Yubico/u2f_keys";

/// Root's list of the keys `pam_u2f` trusts for sudo, read by the person
/// (`0644`): `ronoctua_security::u2f::AUTHFILE`, which only the sudo door
/// writes (`pam-u2f-is-only-sufficient` holds the two equal).
pub const ROOT_U2F_KEYS: &str = "/var/lib/ronoctua-security/u2f_keys";

/// The signers git trusts, logical (`gpg.ssh.allowedSignersFile`).
pub const ALLOWED_SIGNERS: &str = "home/.config/git/allowed_signers";

/// The person's list of VS Code extensions, logical — inside the `vscode`
/// target, so it travels (`ronoctua_vscode::LIST_IN_HOME`, under the home):
/// the crate's own literal, never a second spelling of VS Code's folder
/// (`one-vscode-boundary`).
pub const EXTENSIONS_LIST: &str = concat!("home/", ronoctua_vscode::list_in_home!());

/// What the last install of the person's extensions did, under the
/// belongings' own folder ([`ExtensionsDone`]).
pub const EXTENSIONS_FILE: &str = "offer-extensions.json";

/// git's settings for signing with SSH, logical: a file of the `ssh`
/// target, so it travels with the key it names, and the global
/// configuration includes it.
pub const SIGNING_INCLUDE: &str = "home/.ssh/git-signing.inc";

/// Where this machine mirrors the person's options.
#[must_use]
pub fn options_path(places: &Places) -> Option<PathBuf> {
    places.resolve(&format!(
        "state/{}/{}",
        ronoctua_person::names::MIRROR,
        ronoctua_person::names::OPTIONS
    ))
}

/// The person's options here — none turned on when there are none, or they
/// cannot be read (nothing is turned on by a file this machine cannot
/// read).
#[must_use]
pub fn options(places: &Places) -> Options {
    options_path(places)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| ronoctua_person::parse::<Options>(&bytes).ok())
        .unwrap_or_default()
}

/// Whether `name` may name a key in git's settings as it is: letters,
/// digits, `.`, `_` and `-`, never a leading `.`. A key by another name is
/// never offered. Pure.
fn plain_key_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// The SSH public keys a person may sign with here, by logical path, the
/// preferred first: a `.pub` of `~/.ssh` whose private half is there too
/// (never a certificate), by a plain name.
#[must_use]
pub fn signing_keys(places: &Places) -> Vec<String> {
    let Some(dir) = places.resolve(SSH_DIR) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter_map(|n| n.strip_suffix(".pub").map(str::to_owned))
        .filter(|n| plain_key_name(n) && !n.ends_with("-cert") && dir.join(n).is_file())
        .collect();
    names.sort_by_key(|n| {
        (
            SIGNING_KEYS
                .iter()
                .position(|k| k == n)
                .unwrap_or(SIGNING_KEYS.len()),
            n.clone(),
        )
    });
    names
        .into_iter()
        .map(|n| format!("{SSH_DIR}/{n}.pub"))
        .collect()
}

/// Whether `path` is a file of a target that offers `offering`. Pure.
fn of_a_target_offering(path: &str, offering: Offering) -> bool {
    kept::target_of(path).is_some_and(|t| t.offers(offering))
}

/// Whether `landed` holds a half of a key pair git may sign with here — its
/// `.pub`, or the private half beside it.
fn brought_a_signing_key(places: &Places, landed: &[String]) -> bool {
    let keys = signing_keys(places);
    landed
        .iter()
        .filter(|p| of_a_target_offering(p, Offering::SshSigning))
        .any(|p| {
            keys.iter()
                .any(|k| k == p || k.strip_suffix(".pub") == Some(p.as_str()))
        })
}

/// Whether `landed` holds the person's list of VS Code extensions. Pure.
fn brought_the_extensions(landed: &[String]) -> bool {
    landed
        .iter()
        .any(|p| p == EXTENSIONS_LIST && of_a_target_offering(p, Offering::VscodeExtensions))
}

/// How many extensions of the person's list are missing here, when it
/// reads — none offered when it does not (the yes would refuse it).
fn extensions_missing(places: &Places) -> usize {
    extensions_missing_here(places).ids.len()
}

/// Whether VS Code is on this machine — `code` named by its store path
/// (`ronoctua_vscode::VARIABLE`, set where it is installed): the door draws
/// the extensions' offer only where its yes can work.
#[must_use]
pub fn vscode_here() -> bool {
    ronoctua_vscode::code().is_some()
}

/// Whether `landed` holds the person's registered security keys. Pure.
fn brought_security_keys(landed: &[String]) -> bool {
    landed
        .iter()
        .any(|p| p == U2F_KEYS && of_a_target_offering(p, Offering::PamU2f))
}

/// Whether `credential` is one as `pamu2fcfg` writes it — base64 and hex,
/// the algorithm and the options, between commas — the grammar the sudo
/// door's `u2f-keys` verb takes (`ronoctua_security::u2f::credential_is_plain`,
/// which is the guard; this only keeps an offer from proposing what the
/// verb would refuse). Pure.
fn plain_credential(credential: &str) -> bool {
    !credential.is_empty()
        && credential.len() <= 1024
        && credential.contains(',')
        && credential
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"+/=,-_".contains(&c))
}

/// The key a credential is of: its key handle, the first of its parts.
fn key_of(credential: &str) -> &str {
    credential.split(',').next().unwrap_or_default()
}

/// Every credential a mapping file lists (`name:credential[:…]`, a line per
/// user), in order.
fn credentials_in(mapping: &str) -> impl Iterator<Item = &str> {
    mapping
        .lines()
        .filter_map(|line| line.trim_end().split_once(':'))
        .flat_map(|(_, credentials)| credentials.split(':'))
}

/// The keys the person's own list (`person`, their `u2f_keys`) has and
/// root's list (`root`) lacks — what a yes brings, each once, in the
/// person's order. Every line of the person's own file is theirs (it is
/// written on a machine where their name may differ); every line of root's
/// list counts, as this machine is one person's. A key is its handle: the
/// same key with other options is not brought twice. Pure.
#[must_use]
pub fn keys_to_bring(person: &str, root: &str) -> Vec<String> {
    let mut known: Vec<&str> = credentials_in(root).map(key_of).collect();
    let mut out = Vec::new();
    for credential in credentials_in(person).filter(|c| plain_credential(c)) {
        if !known.contains(&key_of(credential)) {
            known.push(key_of(credential));
            out.push(credential.to_owned());
        }
    }
    out
}

/// [`keys_to_bring`] on this machine: the person's `u2f_keys` here against
/// root's list at `root_list` (none there is none in it).
fn security_keys_to_bring(places: &Places, root_list: &Path) -> Vec<String> {
    let person = places
        .resolve(U2F_KEYS)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let root = std::fs::read_to_string(root_list).unwrap_or_default();
    keys_to_bring(&person, &root)
}

/// Whether a sync that brought `landed` here may offer SSH signing at all —
/// what tells the engine to ask git how it signs ([`Signing::of`]).
#[must_use]
pub fn may_offer_signing(places: &Places, landed: &[String]) -> bool {
    brought_a_signing_key(places, landed)
}

/// What git says of signing here, before SSH signing is offered or turned
/// on — what the person's global configuration says, with what it includes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Signing {
    /// `gpg.format`, when it is set.
    pub format: Option<String>,
    /// `user.signingkey`, when it is set.
    pub key: Option<String>,
    /// Whether `commit.gpgsign` is on.
    pub commits: bool,
}

impl Signing {
    /// What git says here; `None` when it could not be asked (nothing is
    /// offered whose yes would fail).
    #[must_use]
    pub fn of(tools: &ronoctua_git::Tools, home: &Path) -> Option<Self> {
        Self::read(tools, home).ok()
    }

    /// What git says here.
    fn read(tools: &ronoctua_git::Tools, home: &Path) -> Result<Self, Refusal> {
        let get = |key: &str| ronoctua_git::get_global(tools, home, key).map_err(refused);
        Ok(Self {
            format: get("gpg.format")?,
            key: get("user.signingkey")?,
            commits: get("commit.gpgsign")?.is_some_and(|v| {
                ["true", "yes", "on", "1"]
                    .iter()
                    .any(|t| v.eq_ignore_ascii_case(t))
            }),
        })
    }

    /// Whether git signs here already, or is set up to — with SSH or with
    /// OpenPGP, a key named, a format chosen or commits signed. SSH signing
    /// is then not offered: a yes would replace the person's own setup.
    #[must_use]
    pub const fn set_up(&self) -> bool {
        self.format.is_some() || self.key.is_some() || self.commits
    }

    /// Whether it is exactly signing with SSH by `key` (`~/…`) — what a yes
    /// said already. Pure.
    fn signs_with(&self, key: &str) -> bool {
        self.format.as_deref() == Some("ssh") && self.key.as_deref() == Some(key)
    }
}

/// What a sync that brought `landed` here offers, "no" drawn first by the
/// door. `signing`: what git says of signing here ([`Signing::of`], asked
/// when [`may_offer_signing`]); `None` offers no SSH signing.
#[must_use]
pub fn offered(places: &Places, landed: &[String], signing: Option<&Signing>) -> Vec<Offer> {
    offered_against(places, landed, signing, Path::new(ROOT_U2F_KEYS))
}

/// [`offered`], with root's list of keys at `root_list`.
fn offered_against(
    places: &Places,
    landed: &[String],
    signing: Option<&Signing>,
    root_list: &Path,
) -> Vec<Offer> {
    let options = options(places);
    let mut out = Vec::new();
    if options.ssh_signing.is_none()
        && signing.is_some_and(|s| !s.set_up())
        && brought_a_signing_key(places, landed)
        && let Some(key) = signing_keys(places).into_iter().next()
    {
        out.push(Offer::SshSigning { key });
    }
    if brought_security_keys(landed) && !security_keys_to_bring(places, root_list).is_empty() {
        out.push(Offer::PamU2f);
    }
    // THE LIST OF VS CODE'S EXTENSIONS, offered whether or not VS Code is
    // here: the installer's restore cannot tell, and the door that draws it
    // can ([`vscode_here`]).
    if brought_the_extensions(landed) {
        let count = extensions_missing(places);
        if count > 0 {
            out.push(Offer::VscodeExtensions {
                count: u32::try_from(count).unwrap_or(u32::MAX),
            });
        }
    }
    out
}

/// What a git refusal is, for the belongings.
const fn refused(refusal: ronoctua_git::Refusal) -> Refusal {
    match refusal.why {
        ronoctua_git::Why::NoGit | ronoctua_git::Why::Unstartable => Refusal::of(Why::NoTools),
        _ => Refusal::of(Why::Failed),
    }
}

/// Says yes to `offer` on this machine (see the module's header).
///
/// # Errors
///
/// [`Why::Failed`] for a key that is not a public key of `~/.ssh` here, git
/// set up to sign with something else already (nothing of it is replaced),
/// a file that could not be written — or the security keys, which only the
/// sudo door brings into root's list; [`Why::Newer`] for an `options.json`
/// a newer machine wrote (it is never rewritten by an older one);
/// [`Why::NoTools`] without git.
pub fn accept(offer: &Offer, places: &Places, tools: &ronoctua_git::Tools) -> Result<(), Refusal> {
    match offer {
        Offer::SshSigning { key } => {
            let mut options = current_options(places)?;
            let rel = signing_key_of(places, key)?;
            let signing = Signing::read(tools, &places.home)?;
            if signing.set_up() && !signing.signs_with(&tilde(&rel)) {
                return Err(Refusal::of(Why::Failed));
            }
            sign_with(places, tools, &rel)?;
            options.ssh_signing = Some(SshSigning { key: rel });
            write_options(places, &options)
        }
        // ROOT'S LIST, written by the sudo door with the credentials of
        // `security_keys_to_bring`: this process has no root, and the yes
        // that gives it is the door's.
        Offer::PamU2f => Err(Refusal::of(Why::Failed)),
        // THE QUEUE'S (`order::Op::VscodeExtensions`, [`install_extensions`]):
        // minutes of network, never a door's thread.
        Offer::VscodeExtensions { .. } => Err(Refusal::of(Why::Failed)),
    }
}

/// Why the extensions' install tried nothing, as `ronoctua-vscode` says it —
/// for the door that draws [`ExtensionsDone::refused`].
pub use ronoctua_vscode::Why as VscodeWhy;

/// A set of VS Code extensions (the ids, and the pre-releases among them),
/// as `ronoctua-vscode` reads it — for the door that counts what is missing.
pub use ronoctua_vscode::Extensions as ExtensionSet;

/// VS Code's manifest of the extensions installed here, under the home —
/// what a door watches beside the list.
pub use ronoctua_vscode::MANIFEST_IN_HOME;

/// The lock an install of the extensions holds for its whole run, under the
/// belongings' own folder: a second one is refused, and a door SEES one
/// running ([`extensions_running`]) — and that one that was killed or
/// cancelled is not, the kernel letting the lock go however it ends.
pub const EXTENSIONS_LOCK: &str = "extensions.lock";

/// Where the install's lock is.
#[must_use]
pub fn extensions_lock(places: &Places) -> PathBuf {
    places.own().join(EXTENSIONS_LOCK)
}

/// Whether an install of the extensions runs now. Never waits.
#[must_use]
pub fn extensions_running(places: &Places) -> bool {
    crate::running::busy_at(&extensions_lock(places))
}

/// The extensions of the person's list missing here, read now — none when
/// the list or the manifest does not read (the install would refuse it, and
/// says so when it is asked).
#[must_use]
pub fn extensions_missing_here(places: &Places) -> ExtensionSet {
    ronoctua_vscode::missing(&places.home).unwrap_or_default()
}

/// The hosts installing `missing` reaches — what a door lets when the
/// person says yes, and names before ([`ronoctua_vscode::hosts_of`]).
#[must_use]
pub fn extension_hosts(missing: &ExtensionSet) -> Vec<String> {
    ronoctua_vscode::hosts_of(missing.ids.iter().map(String::as_str))
}

/// Where an install of the extensions is, as its record says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallState {
    /// It began, and has not said how it ended — running while its lock is
    /// held ([`extensions_running`]), stopped before its end once it is not.
    Installing,
    /// It ended: what entered and what failed.
    Done,
    /// It tried nothing, and why ([`ExtensionsDone::refused`]).
    Refused,
}

/// An extension that did not enter, and what VS Code's `code` said of it
/// (`ronoctua_vscode::install::what_code_said`; data: VS Code's own words).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExtensionFailed {
    /// Which.
    pub id: String,
    /// What `code` said.
    pub said: String,
}

/// What the last install of the person's VS Code extensions did — written by
/// [`install_extensions`] under the belongings' own folder when it begins and
/// when it ends, read by both doors.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExtensionsDone {
    /// When it began (while [`InstallState::Installing`]) or ended, seconds
    /// since the epoch: a door draws it while it is newer than the last
    /// sync.
    pub at_s: i64,
    /// Where it is.
    pub state: InstallState,
    /// What entered.
    #[serde(default)]
    pub entered: Vec<String>,
    /// What `code` could not install.
    #[serde(default)]
    pub failed: Vec<ExtensionFailed>,
    /// Why nothing was tried, when nothing was.
    #[serde(default)]
    pub refused: Option<ronoctua_vscode::Why>,
    /// The hosts that were not let ([`VscodeWhy::NotLet`]).
    #[serde(default)]
    pub hosts: Vec<String>,
}

impl ExtensionsDone {
    /// The last one under `places`; none when there is no file or it does
    /// not read.
    #[must_use]
    pub fn read(places: &Places) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(places.own().join(EXTENSIONS_FILE)).ok()?).ok()
    }

    /// Writes it — `0600`, like every file of the belongings' own folder.
    fn write(&self, places: &Places) {
        let written = serde_json::to_vec_pretty(self)
            .map_err(std::io::Error::other)
            .and_then(|bytes| {
                crate::secret::write_private(&places.own().join(EXTENSIONS_FILE), &bytes)
            });
        if let Err(e) = written {
            eprintln!(
                "ronoctua-belongings: what the extensions' install did could not be written: {e}"
            );
        }
    }
}

/// Seconds since the epoch, now.
fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// THE YES TO THE EXTENSIONS, as the queue's child runs it
/// (`order::Op::VscodeExtensions`): every extension of the person's list
/// that is missing here, installed one at a time by VS Code's `code`
/// (`ronoctua_vscode::install`) once a consent `Pass` was minted for every
/// host it reaches (`ronoctua_consent::pass`, the person's `consent.json`),
/// each step a beat. It holds [`EXTENSIONS_LOCK`] for its whole run, says it
/// began, and then what entered, what failed or why nothing was tried, where
/// both doors read it ([`ExtensionsDone`]). What `code` said of a failure
/// goes to standard error too, the queue's tail: standard output is the
/// beats'.
///
/// # Errors
///
/// [`Why::Busy`] while another install runs (nothing is written);
/// [`Why::NoTools`] with no `code` here; [`Why::NotLet`] with a host not
/// let; [`Why::Newer`] for a list a newer machine wrote; [`Why::Failed`] for
/// a list that does not read, or when an extension did not enter (the file
/// says which).
pub fn install_extensions(places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    install_extensions_with(
        ronoctua_vscode::code(),
        places,
        &|host| ronoctua_consent::pass(host).ok(),
        beat,
    )
}

/// [`install_extensions`], with `code` where it is (`None`: not here) and
/// the passes `minter` gives.
fn install_extensions_with(
    code: Option<PathBuf>,
    places: &Places,
    minter: &dyn Fn(&str) -> Option<ronoctua_consent::Pass>,
    beat: &mut dyn FnMut(Beat),
) -> Result<u32, Refusal> {
    let _held = crate::running::hold_at(&extensions_lock(places))?;
    let began = |state| ExtensionsDone {
        at_s: now_s(),
        state,
        entered: Vec::new(),
        failed: Vec::new(),
        refused: None,
        hosts: Vec::new(),
    };
    began(InstallState::Installing).write(places);
    let done = match code {
        None => Err(ronoctua_vscode::Refusal::of(ronoctua_vscode::Why::NoCode)),
        Some(code) => {
            ronoctua_vscode::install::install(&code, &places.home, minter, &mut |e| match e {
                ronoctua_vscode::install::Event::Trying { done, total, .. } => beat(Beat::Step {
                    phase: Phase::Installing,
                    done: u64::try_from(done).unwrap_or(u64::MAX),
                    total: u64::try_from(total).unwrap_or(u64::MAX),
                }),
                ronoctua_vscode::install::Event::Entered(_) => {}
                ronoctua_vscode::install::Event::Failed(id, said) => {
                    eprintln!("ronoctua-belongings: {id}: {said}");
                }
            })
        }
    };
    let (record, result) = match done {
        Ok(installed) => {
            let whole = installed.whole();
            (
                ExtensionsDone {
                    entered: installed.entered,
                    failed: installed
                        .failed
                        .into_iter()
                        .map(|f| ExtensionFailed {
                            id: f.id,
                            said: f.said,
                        })
                        .collect(),
                    ..began(InstallState::Done)
                },
                if whole {
                    Ok(0)
                } else {
                    Err(Refusal::of(Why::Failed))
                },
            )
        }
        Err(r) => (
            ExtensionsDone {
                refused: Some(r.why),
                hosts: r.hosts,
                ..began(InstallState::Refused)
            },
            Err(Refusal::of(match r.why {
                ronoctua_vscode::Why::NoCode => Why::NoTools,
                ronoctua_vscode::Why::NotLet => Why::NotLet,
                ronoctua_vscode::Why::Newer => Why::Newer,
                ronoctua_vscode::Why::Busy => Why::Busy,
                _ => Why::Failed,
            })),
        ),
    };
    record.write(places);
    result
}

/// The keys a yes to [`Offer::PamU2f`] brings into this machine's root
/// list — for the door that passes them to the sudo door's `u2f-keys`, with
/// the ones root's list has for the person already. Empty: nothing to offer.
#[must_use]
pub fn security_keys(places: &Places) -> Vec<String> {
    security_keys_to_bring(places, Path::new(ROOT_U2F_KEYS))
}

/// `options.json` as it is, to be changed: the defaults when there is none,
/// a refusal when it is there and cannot be read (never overwritten blind).
fn current_options(places: &Places) -> Result<Options, Refusal> {
    let path = options_path(places).ok_or(Refusal::of(Why::Failed))?;
    match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Options::default()),
        Err(_) => Err(Refusal::of(Why::Failed)),
        Ok(bytes) => ronoctua_person::parse::<Options>(&bytes).map_err(|r| {
            Refusal::of(if r.why == ronoctua_person::Why::Newer {
                Why::Newer
            } else {
                Why::Failed
            })
        }),
    }
}

/// Writes `options.json` whole (`ronoctua_person::to_json`), through a draft.
fn write_options(places: &Places, options: &Options) -> Result<(), Refusal> {
    use ronoctua_person::Document as _;
    options.validate().map_err(|_| Refusal::of(Why::Failed))?;
    let path = options_path(places).ok_or(Refusal::of(Why::Failed))?;
    write_beside(&path, ronoctua_person::to_json(options).as_bytes())
}

/// Writes `bytes` at `path` through a draft beside it.
fn write_beside(path: &Path, bytes: &[u8]) -> Result<(), Refusal> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return Err(Refusal::of(Why::Failed));
    };
    ronoctua_xdg::write_atomically(dir, name, bytes).map_err(|_| Refusal::of(Why::Failed))
}

/// The key of an offer, relative to the home (`.ssh/id_ed25519.pub`): a
/// public key of `~/.ssh`, by a plain name, that is here.
fn signing_key_of(places: &Places, key: &str) -> Result<String, Refusal> {
    let rel = key
        .strip_prefix("home/")
        .filter(|_| signing_keys(places).iter().any(|k| k == key))
        .ok_or(Refusal::of(Why::Failed))?;
    Ok(rel.to_owned())
}

/// A path relative to the home, as git reads it on every machine: `~/…`.
fn tilde(rel: &str) -> String {
    format!("~/{rel}")
}

/// A logical path of the home, relative to it.
fn of_home(logical: &str) -> &str {
    logical.strip_prefix("home/").unwrap_or(logical)
}

/// Makes the include file `0600` (`create_new`) when it is not there — git
/// keeps the mode of the file it rewrites, and `~/.ssh` is private.
fn private_include(at: &Path) -> Result<(), Refusal> {
    use std::os::unix::fs::OpenOptionsExt as _;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(at)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(Refusal::of(Why::Failed)),
    }
}

/// git signs with the key at `rel` (relative to the home): the settings in
/// [`SIGNING_INCLUDE`], the key among the signers the person trusts when git
/// knows their address, and — last, so it names a whole file — the include
/// in the global configuration.
fn sign_with(places: &Places, tools: &ronoctua_git::Tools, rel: &str) -> Result<(), Refusal> {
    let include = places
        .resolve(SIGNING_INCLUDE)
        .ok_or(Refusal::of(Why::Failed))?;
    private_include(&include)?;
    for (key, value) in [
        ("gpg.format", "ssh".to_owned()),
        ("user.signingkey", tilde(rel)),
        ("commit.gpgsign", "true".to_owned()),
        ("tag.gpgsign", "true".to_owned()),
    ] {
        ronoctua_git::set_in_file(tools, &include, key, &value).map_err(refused)?;
    }
    trust_own_key(places, tools, rel, &include)?;
    ronoctua_git::add_global(
        tools,
        &places.home,
        "include.path",
        &tilde(of_home(SIGNING_INCLUDE)),
    )
    .map_err(refused)
}

/// The key at `rel` among the signers the person trusts, for their address
/// — when git knows it; nothing otherwise.
fn trust_own_key(
    places: &Places,
    tools: &ronoctua_git::Tools,
    rel: &str,
    include: &Path,
) -> Result<(), Refusal> {
    let email = ronoctua_git::get_global(tools, &places.home, "user.email").map_err(refused)?;
    let Some(email) = email.filter(|e| !e.is_empty() && !e.contains(char::is_whitespace)) else {
        return Ok(());
    };
    let public = places
        .resolve(&format!("home/{rel}"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .ok_or(Refusal::of(Why::Failed))?;
    let mut fields = public.split_whitespace();
    let (Some(kind), Some(blob)) = (fields.next(), fields.next()) else {
        return Err(Refusal::of(Why::Failed));
    };
    let signers = places
        .resolve(ALLOWED_SIGNERS)
        .ok_or(Refusal::of(Why::Failed))?;
    let mut text = std::fs::read_to_string(&signers).unwrap_or_default();
    let known = text.lines().any(|l| {
        let mut words = l.split_whitespace().skip_while(|w| *w != kind);
        words.next() == Some(kind) && words.next() == Some(blob)
    });
    if !known {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("{email} namespaces=\"git\" {kind} {blob}\n"));
        if let Some(dir) = signers.parent() {
            std::fs::create_dir_all(dir).map_err(|_| Refusal::of(Why::Failed))?;
        }
        write_beside(&signers, text.as_bytes())?;
    }
    ronoctua_git::set_in_file(
        tools,
        include,
        "gpg.ssh.allowedSignersFile",
        &tilde(of_home(ALLOWED_SIGNERS)),
    )
    .map_err(refused)
}

// ── anchor: L10 ──

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> (PathBuf, Places) {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-belongings-offers-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let p = Places::under(&root);
        std::fs::create_dir_all(p.home.join(".ssh")).unwrap();
        (root, p)
    }

    fn touch(p: &Places, logical: &str, text: &str) {
        let at = p.resolve(logical).unwrap();
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }

    fn landed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|p| (*p).to_owned()).collect()
    }

    /// Nothing about signing in git.
    const NONE: Signing = Signing {
        format: None,
        key: None,
        commits: false,
    };

    /// FAILS IF: a sync that brought no key offers anything — another file
    /// of the same targets (`known_hosts`, `.gitconfig`, the signers)
    /// included, which would bring a "no" back at almost every sync — a
    /// signing key without its private half (or by a name git's settings
    /// would have to quote) is offered, the preferred key is not the one
    /// offered, or what the person turned on already, or a git that signs
    /// already with SSH or with OpenPGP, is offered SSH signing.
    #[test]
    fn only_what_arrived_and_is_not_on_yet_is_offered() {
        let (root, p) = scratch("offered");
        touch(&p, "home/.ssh/id_rsa", "private");
        touch(&p, "home/.ssh/id_rsa.pub", "ssh-rsa AAAA me");
        touch(&p, "home/.ssh/id_ed25519", "private");
        touch(&p, "home/.ssh/id_ed25519.pub", "ssh-ed25519 AAAA me");
        touch(&p, "home/.ssh/lonely.pub", "ssh-ed25519 BBBB lonely");
        touch(&p, "home/.ssh/my key", "private");
        touch(&p, "home/.ssh/my key.pub", "ssh-ed25519 CCCC spaced");
        touch(&p, "home/.ssh/known_hosts", "github.com ssh-ed25519 AAAA\n");
        touch(&p, "home/.gitconfig", "[alias]\n\tst = status\n");
        touch(&p, ALLOWED_SIGNERS, "me ssh-ed25519 AAAA\n");
        touch(&p, U2F_KEYS, "me:handle,key,es256,+presence");
        let root_list = root.join("root-u2f_keys");
        let offered = |p: &Places, landed: &[String], signing: Option<&Signing>| {
            offered_against(p, landed, signing, &root_list)
        };
        assert_eq!(
            signing_keys(&p),
            ["home/.ssh/id_ed25519.pub", "home/.ssh/id_rsa.pub"]
        );
        assert!(offered(&p, &landed(&["config/theme.json"]), Some(&NONE)).is_empty());
        // ANOTHER FILE OF THE SAME TARGETS: no key, no security keys.
        let unrelated = landed(&[
            "home/.ssh/known_hosts",
            "home/.gitconfig",
            ALLOWED_SIGNERS,
            "home/.ssh/lonely.pub",
            "home/.ssh/my key.pub",
        ]);
        assert!(
            offered(&p, &unrelated, Some(&NONE)).is_empty(),
            "a file that makes nothing possible offered it"
        );
        let both = landed(&["home/.ssh/id_ed25519", U2F_KEYS]);
        assert_eq!(
            offered(&p, &both, Some(&NONE)),
            [
                Offer::SshSigning {
                    key: "home/.ssh/id_ed25519.pub".to_owned()
                },
                Offer::PamU2f
            ]
        );
        // A HALF OF ANOTHER PAIR: the preferred key is still the one offered.
        assert_eq!(
            offered(&p, &landed(&["home/.ssh/id_rsa.pub"]), Some(&NONE)),
            [Offer::SshSigning {
                key: "home/.ssh/id_ed25519.pub".to_owned()
            }]
        );
        assert!(may_offer_signing(&p, &both));
        assert!(!may_offer_signing(&p, &unrelated));
        // GIT SIGNS ALREADY, or could not be asked: no SSH signing.
        let openpgp = Signing {
            format: None,
            key: Some("0xA1B2C3D4E5F60718".to_owned()),
            commits: true,
        };
        let ssh = Signing {
            format: Some("ssh".to_owned()),
            key: Some("~/.ssh/id_rsa.pub".to_owned()),
            commits: true,
        };
        let off = Signing {
            format: Some("openpgp".to_owned()),
            ..NONE
        };
        for (signing, what) in [
            (Some(&openpgp), "OpenPGP signing"),
            (Some(&ssh), "SSH signing"),
            (Some(&off), "a format chosen"),
            (None, "a git that could not be asked"),
        ] {
            assert_eq!(
                offered(&p, &both, signing),
                [Offer::PamU2f],
                "SSH signing offered over {what}"
            );
        }
        // TURNED ON already: SSH signing on any machine (the options
        // travel), the keys where root's list has them — under another
        // name, with other options.
        let on = Options {
            ssh_signing: Some(SshSigning {
                key: ".ssh/id_ed25519.pub".to_owned(),
            }),
            ..Options::default()
        };
        write_options(&p, &on).unwrap();
        std::fs::write(&root_list, "other:handle,key,es256,+presence+pin\n").unwrap();
        assert!(offered(&p, &both, Some(&NONE)).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a key root's list has is brought again (by its handle,
    /// whatever its options or the name on its line), one is brought twice,
    /// something that is not a credential is proposed, or the person's
    /// order is lost.
    #[test]
    fn the_keys_brought_are_the_ones_roots_list_lacks() {
        let a = "QUFB,04aa,es256,+presence";
        let b = "QkJC,04bb,eddsa,+presence";
        let c = "Q0ND,04cc,es256,+presence+pin";
        let person = format!("me:{a}:{b}\nme-elsewhere:{c}:{a}\nbroken\nme:x y,z\n");
        assert_eq!(keys_to_bring(&person, ""), [a, b, c]);
        assert_eq!(
            keys_to_bring(
                &person,
                &format!("someone:{}\n", b.replace("+presence", "+presence+pin"))
            ),
            [a, c]
        );
        assert!(keys_to_bring(&person, &format!("someone:{a}:{b}:{c}\n")).is_empty());
        assert!(keys_to_bring("", "").is_empty());
        assert!(keys_to_bring("me:a:b\n", "").is_empty(), "not credentials");
    }

    /// FAILS IF: a yes to the security keys writes anything here (root's
    /// list is the sudo door's), a key outside `~/.ssh` is taken for
    /// signing, or a file a newer machine wrote is rewritten.
    #[test]
    fn yes_writes_only_what_this_process_may() {
        let (root, p) = scratch("accept");
        let tools = ronoctua_git::Tools {
            git: PathBuf::from("/nonexistent/git"),
            ssh: None,
            askpass: None,
            pq_tls: None,
        };
        assert_eq!(
            accept(&Offer::PamU2f, &p, &tools),
            Err(Refusal::of(Why::Failed))
        );
        assert!(
            !options_path(&p).unwrap().exists(),
            "the keys' yes wrote the options"
        );
        assert_eq!(
            accept(
                &Offer::SshSigning {
                    key: "config/theme.json".to_owned()
                },
                &p,
                &tools
            ),
            Err(Refusal::of(Why::Failed))
        );
        let path = options_path(&p).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, br#"{"format": 9, "ssh-signing": null}"#).unwrap();
        touch(&p, "home/.ssh/id_ed25519", "private");
        touch(&p, "home/.ssh/id_ed25519.pub", "ssh-ed25519 AAAA me");
        assert_eq!(
            accept(
                &Offer::SshSigning {
                    key: "home/.ssh/id_ed25519.pub".to_owned()
                },
                &p,
                &tools
            ),
            Err(Refusal::of(Why::Newer))
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            br#"{"format": 9, "ssh-signing": null}"#,
            "a newer machine's options were rewritten"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn real_tools() -> ronoctua_git::Tools {
        ronoctua_git::Tools::from_env()
            .expect("no git for this build: enter the environment or run through the flake")
    }

    /// FAILS IF: saying yes to SSH signing does not make git sign every
    /// commit and tag with the person's key — or writes those settings into
    /// `.gitconfig` itself (which travels without the key) instead of a
    /// private file of `~/.ssh` that `.gitconfig` only includes — does not
    /// trust the key for their address, trusts it or includes the file
    /// twice when asked twice, drops an include the person had, or is not
    /// recorded. The real git of this build, in a home of the test's own.
    #[test]
    fn yes_to_signing_makes_git_sign_with_the_persons_key() {
        use std::os::unix::fs::PermissionsExt as _;
        let tools = real_tools();
        let (root, p) = scratch("signing");
        touch(&p, "home/.ssh/id_ed25519", "private");
        touch(
            &p,
            "home/.ssh/id_ed25519.pub",
            "ssh-ed25519 AAAAC3Nza me@laptop\n",
        );
        ronoctua_git::set_global(&tools, &p.home, "user.email", "me@example.org").unwrap();
        ronoctua_git::add_global(&tools, &p.home, "include.path", "~/.mine.inc").unwrap();
        let offer = Offer::SshSigning {
            key: "home/.ssh/id_ed25519.pub".to_owned(),
        };
        accept(&offer, &p, &tools).unwrap();
        accept(&offer, &p, &tools).unwrap();
        let get = |k: &str| ronoctua_git::get_global(&tools, &p.home, k).unwrap();
        assert_eq!(get("gpg.format").as_deref(), Some("ssh"));
        assert_eq!(
            get("user.signingkey").as_deref(),
            Some("~/.ssh/id_ed25519.pub")
        );
        assert_eq!(get("commit.gpgsign").as_deref(), Some("true"));
        assert_eq!(get("tag.gpgsign").as_deref(), Some("true"));
        assert_eq!(
            get("gpg.ssh.allowedSignersFile").as_deref(),
            Some("~/.config/git/allowed_signers")
        );
        // THE GLOBAL FILE ONLY INCLUDES: what it says alone signs nothing.
        let gitconfig = std::fs::read_to_string(p.home.join(".gitconfig")).unwrap();
        for setting in ["format", "signingkey", "gpgsign", "allowedSignersFile"] {
            assert!(
                !gitconfig.contains(setting),
                "`{setting}` is in .gitconfig, which travels without the key:\n{gitconfig}"
            );
        }
        assert_eq!(
            gitconfig.matches("path = ~/.ssh/git-signing.inc").count(),
            1,
            "{gitconfig}"
        );
        assert!(gitconfig.contains("path = ~/.mine.inc"), "{gitconfig}");
        let include = p.resolve(SIGNING_INCLUDE).unwrap();
        assert_eq!(
            kept::target_of(SIGNING_INCLUDE).map(|t| t.id),
            Some("ssh"),
            "the settings do not travel with the key"
        );
        assert_eq!(
            std::fs::metadata(&include).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let signers = std::fs::read_to_string(p.resolve(ALLOWED_SIGNERS).unwrap()).unwrap();
        assert_eq!(
            signers,
            "me@example.org namespaces=\"git\" ssh-ed25519 AAAAC3Nza\n"
        );
        assert_eq!(
            options(&p).ssh_signing,
            Some(SshSigning {
                key: ".ssh/id_ed25519.pub".to_owned()
            })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a yes to SSH signing replaces a git that signs with
    /// OpenPGP already — its key, its format, the file it lives in — or is
    /// recorded as turned on.
    #[test]
    fn yes_never_replaces_another_signing_setup() {
        let tools = real_tools();
        let (root, p) = scratch("openpgp");
        touch(&p, "home/.ssh/id_ed25519", "private");
        touch(&p, "home/.ssh/id_ed25519.pub", "ssh-ed25519 AAAAC3Nza me\n");
        ronoctua_git::set_global(&tools, &p.home, "user.signingkey", "0xA1B2C3D4E5F60718").unwrap();
        ronoctua_git::set_global(&tools, &p.home, "commit.gpgsign", "true").unwrap();
        let before = std::fs::read(p.home.join(".gitconfig")).unwrap();
        assert!(Signing::of(&tools, &p.home).is_some_and(|s| s.set_up()));
        let offer = Offer::SshSigning {
            key: "home/.ssh/id_ed25519.pub".to_owned(),
        };
        assert_eq!(
            accept(&offer, &p, &tools),
            Err(Refusal::of(Why::Failed)),
            "a yes replaced the person's OpenPGP signing"
        );
        assert_eq!(std::fs::read(p.home.join(".gitconfig")).unwrap(), before);
        assert!(!p.resolve(SIGNING_INCLUDE).unwrap().exists());
        assert_eq!(options(&p).ssh_signing, None);
        let _ = std::fs::remove_dir_all(root);
    }

    /// The person's list, and VS Code's manifest, under `p`'s home.
    fn extensions(p: &Places, list: &[&str], installed: &[&str]) {
        let ids = |ids: &[&str]| {
            ids.iter()
                .map(|i| format!("\"{i}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        touch(
            p,
            EXTENSIONS_LIST,
            &format!("{{\"format\": 1, \"extensions\": [{}]}}", ids(list)),
        );
        let entries: Vec<String> = installed
            .iter()
            .map(|i| {
                format!("{{\"identifier\": {{\"id\": \"{i}\"}}, \"metadata\": {{\"source\": \"gallery\"}}}}")
            })
            .collect();
        touch(
            p,
            &format!("home/{}", ronoctua_vscode::MANIFEST_IN_HOME),
            &format!("[{}]", entries.join(", ")),
        );
    }

    /// FAILS IF: the list's logical path is not the one `ronoctua-vscode`
    /// writes, or not a file of a target that offers the extensions — the
    /// table and the crate would name two files, and nothing would ever be
    /// offered — or the target carries this machine's state or other
    /// people's code (agent plugins, a git folder, `node_modules`, a
    /// profile's manifest), or leaves the person's settings out.
    #[test]
    fn the_extensions_list_is_the_vscode_targets() {
        assert_eq!(
            EXTENSIONS_LIST,
            format!("home/{}", ronoctua_vscode::LIST_IN_HOME)
        );
        let target = kept::target_of(EXTENSIONS_LIST).expect("the list is in no target");
        assert_eq!(target.id, "vscode");
        assert!(target.offers(Offering::VscodeExtensions));
        assert!(!target.skips(EXTENSIONS_LIST), "the target skips the list");
        // WHAT IS THIS MACHINE'S, OR OTHER PEOPLE'S CODE, stays.
        for skipped in [
            "home/.config/Code/User/globalStorage/state.vscdb",
            "home/.config/Code/User/workspaceStorage/abc/state.vscdb",
            "home/.config/Code/User/History/1/entries.json",
            "home/.config/Code/User/sync/settings/lastSyncsettings.json",
            "home/.config/Code/User/profiles/p1/extensions.json",
            "home/.config/Code/User/profiles/p1/globalStorage/x.json",
            "home/.config/Code/User/agent-plugins/installed.json",
            "home/.config/Code/User/agent-plugins/github.com/x/y/.git/HEAD",
            "home/.config/Code/User/prompts/tool/node_modules/a/index.js",
            "home/.config/Code/User/snippets/.git/config",
        ] {
            assert!(target.skips(skipped), "{skipped} travels");
        }
        for kept in [
            "home/.config/Code/User/settings.json",
            "home/.config/Code/User/keybindings.json",
            "home/.config/Code/User/snippets/rust.json",
            "home/.config/Code/User/profiles/p1/settings.json",
            "home/.config/Code/User/mcp.json",
        ] {
            assert!(!target.skips(kept), "{kept} does not travel");
        }
    }

    /// FAILS IF: a sync that brought the list with ids missing here does not
    /// offer them, counted — or one that brought another file of the
    /// target, a list all installed here, or a list that does not read
    /// offers anything.
    #[test]
    fn the_extensions_are_offered_when_the_list_brought_some_missing() {
        let (root, p) = scratch("extensions");
        let root_list = root.join("root-u2f_keys");
        let list = landed(&[EXTENSIONS_LIST]);
        extensions(&p, &["a.x", "b.y", "c.z"], &["b.y"]);
        assert_eq!(
            offered_against(&p, &list, None, &root_list),
            [Offer::VscodeExtensions { count: 2 }]
        );
        let settings = landed(&["home/.config/Code/User/settings.json"]);
        assert!(offered_against(&p, &settings, None, &root_list).is_empty());
        extensions(&p, &["b.y"], &["B.y"]);
        assert!(offered_against(&p, &list, None, &root_list).is_empty());
        touch(
            &p,
            EXTENSIONS_LIST,
            "{\"format\": 1, \"extensions\": [\"-x.y\"]}",
        );
        assert!(offered_against(&p, &list, None, &root_list).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    /// A program of the test's, written by a `/bin/sh` child — never by
    /// this process (a `fork` on another thread would keep it open, and
    /// running it would be `ETXTBSY` at random).
    fn write_executable(at: &Path, script: &str) {
        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"")
            .arg("sh")
            .arg(at)
            .arg(script)
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// FAILS IF: the yes to the extensions asks anything with a host not
    /// let (or does not say which); runs beside another install; does not
    /// install what is missing one at a time with a beat each; does not
    /// write what entered and what failed — with what `code` said — where
    /// both doors read it; calls a run with a failure done; lets its lock
    /// outlive it; or — with no `code` here — tries anything, or does not
    /// say why.
    #[test]
    fn the_extensions_yes_installs_and_says_what_entered() {
        let (root, p) = scratch("extensions-yes");
        extensions(&p, &["a.x", "bad.one", "c.z"], &["c.z"]);
        let code = root.join("code");
        write_executable(
            &code,
            "#!/bin/sh\n[ \"$4\" = bad.one ] && { echo \"Extension 'bad.one' not found.\" >&2; exit 1; }\nexit 0\n",
        );
        // NO HOST LET: nothing is asked, and the record says which hosts.
        let ran = install_extensions_with(Some(code.clone()), &p, &|_| None, &mut |_| {});
        assert_eq!(ran, Err(Refusal::of(Why::NotLet)));
        let refused = ExtensionsDone::read(&p).unwrap();
        assert_eq!(refused.state, InstallState::Refused);
        assert_eq!(refused.refused, Some(ronoctua_vscode::Why::NotLet));
        assert_eq!(
            refused.hosts,
            ronoctua_vscode::hosts_of(["a.x", "bad.one"]),
            "the hosts not let are not the install's"
        );
        let consent = ronoctua_consent::store_at(Some(root.join("consent")));
        let hosts = refused.hosts.iter().map(String::as_str).collect::<Vec<_>>();
        ronoctua_consent::let_hosts_in(&consent, &hosts, true).unwrap();
        let minter = |h: &str| ronoctua_consent::pass_in(&consent, h).ok();
        // ANOTHER RUN HOLDS THE LOCK: refused, and nothing written over.
        {
            let _other = crate::running::hold_at(&extensions_lock(&p)).unwrap();
            assert!(extensions_running(&p));
            assert_eq!(
                install_extensions_with(Some(code.clone()), &p, &minter, &mut |_| {}),
                Err(Refusal::of(Why::Busy))
            );
            assert_eq!(ExtensionsDone::read(&p).unwrap(), refused);
        }
        assert!(!extensions_running(&p), "the lock outlived its run");
        let mut beats = Vec::new();
        let ran = install_extensions_with(Some(code), &p, &minter, &mut |b| beats.push(b));
        assert_eq!(ran, Err(Refusal::of(Why::Failed)));
        assert_eq!(
            beats,
            [
                Beat::Step {
                    phase: Phase::Installing,
                    done: 0,
                    total: 2
                },
                Beat::Step {
                    phase: Phase::Installing,
                    done: 1,
                    total: 2
                }
            ]
        );
        let done = ExtensionsDone::read(&p).expect("nothing written for the doors");
        assert_eq!(done.state, InstallState::Done);
        assert_eq!(done.entered, vec!["a.x".to_owned()]);
        assert_eq!(
            done.failed,
            vec![ExtensionFailed {
                id: "bad.one".to_owned(),
                said: "Extension 'bad.one' not found.".to_owned()
            }]
        );
        assert_eq!(done.refused, None);
        assert!(done.at_s > 0);
        assert_eq!(
            install_extensions_with(None, &p, &minter, &mut |_| {}),
            Err(Refusal::of(Why::NoTools))
        );
        let none = ExtensionsDone::read(&p).unwrap();
        assert_eq!(none.refused, Some(ronoctua_vscode::Why::NoCode));
        assert!(none.entered.is_empty() && none.failed.is_empty());
        assert_eq!(
            accept(
                &Offer::VscodeExtensions { count: 2 },
                &p,
                &ronoctua_git::Tools {
                    git: PathBuf::from("/nonexistent/git"),
                    ssh: None,
                    askpass: None,
                    pq_tls: None,
                }
            ),
            Err(Refusal::of(Why::Failed)),
            "a door's thread installed the extensions"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
