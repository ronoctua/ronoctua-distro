//! What the questions are checked against: NetworkManager and the person's
//! repository — the jobs of [`crate::job`], run.
//!
//! # Nothing here decides
//!
//! The network's reading is [`crate::gate::gate`]'s, the repository's lines
//! [`crate::access`]'s. This runs the programs those decide about —
//! `ronoctua-network` (the one boundary with NetworkManager), `nix` (which
//! names the template's distro input), and the belongings' own CLI (the one
//! that speaks git and age) — in the installer's scratch home, so nothing
//! the person typed lands on a disk.
//!
//! # With no network, only what is local
//!
//! The installation never uses the network, and the network question goes
//! on without one. Then the repository is not asked at all: the scratch
//! home is made and a deploy key generated (both local), a token is kept
//! where the access is read, and the first boot checks them
//! ([`Reach::checks`]).
//!
//! # The scratch home
//!
//! The repository is asked from a home of the installer's own, in RAM
//! ([`Scratch`]): its `belongings.json` names the repository, its
//! `consent.json` lets the repository's host (typing the address was the
//! consent), and its `belongings-keys/` holds the access made for this
//! machine. Every belongings' child runs with that home and its roots, and
//! with none of the answers' variables.

use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ronoctua_belongings::Why;
use ronoctua_belongings::places::{DEPLOY_KEY, PASSPHRASE_FILE, TOKEN_FILE};
use ronoctua_belongings::prefs::{Auth, Belongings};
use ronoctua_network::{JOIN_MOST_S, Joined, Joining};

use crate::access::{Probed, auth_of, probed_of, verified_of};
use crate::gate::{Gate, SETTLE_MOST_S, gate, unsettled};
use crate::job::{Done, Job};
use crate::model::{Answers, MINIMUM_PASSPHRASE};
use crate::plan::{BELONGINGS, Scratch};

/// How long, at most, a joined link waits for NetworkManager to measure the
/// way out before the gate takes "not measured" for an answer.
pub const MEASURE_MOST_S: u64 = 5;

/// What a job is run against.
#[derive(Debug, Clone)]
pub struct Reach {
    /// The belongings' CLI, by path.
    pub belongings: PathBuf,
    /// `nix`, by path.
    pub nix: PathBuf,
    /// The scratch folder.
    pub scratch: Scratch,
    /// The distro's tree, by its real path; `None`: the ISO said nothing.
    pub distro: Option<String>,
}

/// A program on `PATH`, by its path — what a child is started with, so the
/// one that runs is the one that was looked at.
#[must_use]
pub fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(|d| Path::new(d).join(name))
        .find(|p| {
            std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// The release's flake reference, as the template names it — asked of
/// `nix`, never read out of the file by hand (invariant 5).
///
/// # Errors
///
/// `nix` could not be run, or the template names no distro input.
pub fn release_url(nix: &Path, template_flake: &str) -> Result<String, String> {
    let out = bare(nix)
        .args([
            "eval",
            "--raw",
            "--file",
            template_flake,
            &format!("inputs.{}.url", crate::release::DISTRO_INPUT),
        ])
        .output()
        .map_err(|e| format!("{}: {e}", nix.display()))?;
    let url = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() || url.is_empty() {
        return Err(format!(
            "the template names no distro input: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(url)
}

/// Why the terminal-less mode refuses to start. No network is not a reason:
/// the installation needs none, and the first boot checks the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The access does not both read and write the repository — or, with no
    /// network, the file named cannot be kept where it is read.
    Access(Result<Probed, Why>),
    /// The passphrase does not open the repository.
    Passphrase(Why),
    /// An answer is no good for the repository found (a passphrase too
    /// short to make one with).
    Answer(crate::model::Invalid),
}

/// The exit code of an installer stopped by the repository: the access or
/// the passphrase.
pub const EXIT_REPOSITORY: u8 = 4;

impl Refused {
    /// The exit code it ends the installer with.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Access(_) | Self::Passphrase(_) | Self::Answer(_) => EXIT_REPOSITORY,
        }
    }

    /// The sentence the person reads about it.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Access(Ok(p)) => p.status_key(),
            Self::Access(Err(why)) | Self::Passphrase(why) => crate::access::refusal_key(*why),
            Self::Answer(i) => i.key(),
        }
    }

    /// Its id, for the English line of the terminal-less mode.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::Access(Ok(p)) => format!("access read={} write={}", p.read, p.write),
            Self::Access(Err(why)) => format!("access {}", why.id()),
            Self::Passphrase(why) => format!("passphrase {}", why.id()),
            Self::Answer(i) => format!("answer {i}"),
        }
    }
}

/// `path`, its links resolved — or as given when it does not resolve, which
/// the step that reads it then says.
///
/// THE ISO'S TREE IS A LINK into the store (`environment.etc`), and the
/// unstable Nix refuses a path with a link in it: the lock step's
/// `--reference-lock-file /etc/ronoctua/distro/flake.lock` failed with "path
/// '/etc/ronoctua/distro' is a symlink". Every step names the tree by the
/// store path it is.
fn real_path(path: &str) -> String {
    std::fs::canonicalize(path).map_or_else(
        |_| path.to_owned(),
        |real| real.to_string_lossy().into_owned(),
    )
}

impl Reach {
    /// The world of this installer: the programs on `PATH`, the scratch
    /// folder at its place, and what the ISO said of the distro. A program
    /// that is not on `PATH` keeps its bare name, and the job that needs it
    /// says so ([`Why::NoTools`]) — the network's reading needs none, and it
    /// is asked first.
    #[must_use]
    pub fn from_env() -> Self {
        let find = |name: &str| on_path(name).unwrap_or_else(|| PathBuf::from(name));
        Self {
            belongings: find(BELONGINGS),
            nix: find("nix"),
            scratch: Scratch::installer(),
            distro: std::env::var(crate::release::FLAKE_DIR)
                .ok()
                .filter(|v| !v.is_empty())
                .map(|v| real_path(&v)),
        }
    }

    /// Runs one job, to the end. A thread's call in the TUI; the
    /// terminal-less mode's, one after the other.
    #[must_use]
    pub fn run(&self, job: &Job) -> Done {
        match job {
            Job::Network { ssid, password } => Done::Network(self.network(ssid, password)),
            Job::Prepare { address, online } => {
                Done::Prepared(self.prepare(address, None, *online))
            }
            Job::Probe {
                address,
                token,
                wait_s,
            } => {
                if *wait_s > 0 {
                    std::thread::sleep(Duration::from_secs(*wait_s));
                }
                Done::Probed(self.probe(address, token.as_deref()))
            }
            Job::Keep { address, token } => Done::Kept(self.keep(address, token)),
            Job::Verify {
                address,
                passphrase,
            } => Done::Verified(self.verify(address, passphrase).map(|_| ())),
        }
    }

    /// Joins `ssid` (none: a cable, or no network at all) and reads the way
    /// out — waiting while a link comes up, at most [`SETTLE_MOST_S`] (a
    /// network joined, until NetworkManager's verdict, at most
    /// [`JOIN_MOST_S`]), and while a joined link is not measured yet, at
    /// most [`MEASURE_MOST_S`] more.
    pub fn network(&self, ssid: &str, password: &str) -> Gate {
        let Ok(manager) = ronoctua_network::Manager::fresh() else {
            return Gate::NoDaemon;
        };
        let join = if ssid.is_empty() {
            None
        } else {
            let password = (!password.is_empty()).then_some(password);
            // FOR THIS BOOT, the password kept by NetworkManager in RAM: the
            // ISO runs no secret agent, and a password left to one is never
            // used (`join_for_this_boot`). The installed machine gets its
            // own profile and its own sealing.
            //
            // A JOIN NOT EVEN STARTED is no refused key: NetworkManager turned
            // the request down, or the network went out of range.
            match manager.join_for_this_boot(ssid, password) {
                Ok(join) => Some(join),
                Err(_) => return Gate::Unjoined,
            }
        };
        let over = |join: &Joining, said: Gate| {
            // Its profile goes with it, so the daemon does not try a refused
            // key again on its own.
            let _ = manager.abandon(join);
            said
        };
        let most = Duration::from_secs(if join.is_some() {
            JOIN_MOST_S
        } else {
            SETTLE_MOST_S
        });
        let start = Instant::now();
        let mut linked_at: Option<Instant> = None;
        loop {
            // THE JOIN'S OWN VERDICT, and not "a link is up": a cable plugged
            // in would let a refused password through. The verdict is the
            // answer the moment it comes — a refused key at once, any other
            // ending after the join tried again (`JOIN_MOST_S`): there is
            // nothing left to wait for.
            //
            // AND ONLY A REFUSED KEY SAYS "check its password": every other
            // ending says the network was not joined, and nothing of the
            // password (a right one read "check it", 2026-10-25).
            let joined = join.as_ref().map(|j| manager.joining(j));
            match (join.as_ref(), joined) {
                (Some(j), Some(Joined::Refused)) => return over(j, Gate::Unconnected),
                (Some(j), Some(Joined::Failed)) => return over(j, Gate::Unjoined),
                _ => {}
            }
            let look = manager.look();
            let up = joined.map_or_else(|| look.linked(), |j| j == Joined::Up);
            if up {
                linked_at.get_or_insert_with(Instant::now);
            }
            let coming = unsettled(&look) || joined == Some(Joined::Coming);
            let too_long = start.elapsed() >= most
                || linked_at.is_some_and(|t| t.elapsed() >= Duration::from_secs(MEASURE_MOST_S));
            if !coming || too_long {
                if let Some(j) = join.as_ref().filter(|_| !up) {
                    return over(j, Gate::Unjoined);
                }
                return gate(&look);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    /// A child with the scratch home's roots and none of the answers'
    /// variables.
    fn child(&self, program: &Path, flags: &[&str], args: &[&str]) -> Command {
        let mut c = crate::executor::command(program);
        c.args(args).args(flags).stdin(Stdio::null());
        let home = self.scratch.home();
        c.env("HOME", &home)
            .env("XDG_CONFIG_HOME", format!("{home}/.config"))
            .env("XDG_STATE_HOME", format!("{home}/.local/state"))
            .env("XDG_CACHE_HOME", format!("{home}/.cache"))
            .env("XDG_RUNTIME_DIR", self.scratch.runtime());
        c
    }

    /// Makes the scratch home for `address`: its folders (the root `0711`,
    /// everything under it root's alone), its settings, the consent for its
    /// host — and the access: the file the terminal-less mode named
    /// (`credential`), or, for an SSH address, a deploy key made here. The
    /// public half of a key made here, to show. All of it local but an
    /// HTTPS address's transport, asked of its server only `online`.
    ///
    /// # Errors
    ///
    /// [`Why::NoRepository`] for an address with no host, [`Why::NeedsAccess`]
    /// for a credential that cannot be read, [`Why::NoTools`] when the CLI
    /// cannot be run, [`Why::Failed`] when the scratch home cannot be made,
    /// [`Why::NotPostQuantum`] for an HTTPS server that cannot be keyed
    /// post-quantum (online only).
    pub fn prepare(
        &self,
        address: &str,
        credential: Option<&Path>,
        online: bool,
    ) -> Result<Option<String>, Why> {
        let host = ronoctua_belongings::prefs::host_of(address).ok_or(Why::NoRepository)?;
        let failed = |_| Why::Failed;
        made(&self.scratch.root, 0o711).map_err(failed)?;
        made(&self.scratch.home(), 0o700).map_err(failed)?;
        made(&self.scratch.runtime(), 0o700).map_err(failed)?;
        let config = self.scratch.config();
        std::fs::create_dir_all(&config).map_err(failed)?;
        let mut prefs = Belongings::with_repository(address);
        prefs.auth = auth_of(address);
        write_private(
            &Path::new(&config).join(ronoctua_belongings::prefs::FILE),
            prefs.to_file().as_bytes(),
        )
        .map_err(failed)?;
        ronoctua_consent::let_hosts_under(Path::new(&config), &[host.as_str()], true)
            .map_err(|_| Why::Failed)?;
        let keys = self.scratch.keys();
        made(&keys, 0o700).map_err(failed)?;
        let kept = Path::new(&keys).join(match prefs.auth {
            Auth::Ssh => DEPLOY_KEY,
            Auth::Token => TOKEN_FILE,
        });
        if let Some(file) = credential {
            let bytes = std::fs::read(file).map_err(|_| Why::NeedsAccess)?;
            write_private(&kept, &bytes).map_err(failed)?;
            return Ok(None);
        }
        // WITH NO NETWORK THE ADDRESS IS TAKEN AS TYPED: its server is not
        // asked, and the first boot finds out what it agrees.
        if prefs.auth == Auth::Token {
            if !online {
                return Ok(None);
            }
            return self.keys_post_quantum(address, &kept).map(|()| None);
        }
        let ran = self
            .child(&self.belongings, &[], &["keygen"])
            .output()
            .map_err(|_| Why::NoTools)?;
        if !ran.status.success() {
            return Err(Why::NoTools);
        }
        std::fs::read_to_string(Path::new(&keys).join(format!("{DEPLOY_KEY}.pub")))
            .map(|k| Some(k.trim().to_owned()))
            .map_err(|_| Why::Failed)
    }

    /// Whether an HTTPS address's server agrees a post-quantum key, asked
    /// BEFORE the token is: a host whose HTTPS is classical (GitHub's,
    /// measured) refuses every token, and a token is typed by hand. Asked by
    /// the probe itself, with an empty token in `kept` — the session is the
    /// belongings' own, so the answer is the one the token would get; and
    /// the askpass never answers an empty token, so no credential is sent.
    /// The empty file goes with the answer.
    ///
    /// # Errors
    ///
    /// [`Why::NotPostQuantum`] when the server offered no post-quantum key
    /// exchange. Any other answer (a token refused, an unreachable host) is
    /// the token's probe to say, once there is one.
    fn keys_post_quantum(&self, address: &str, kept: &Path) -> Result<(), Why> {
        let probed = self.probe(address, Some(""));
        let _ = std::fs::remove_file(kept);
        match probed {
            Ok(Probed {
                why: Some(Why::NotPostQuantum),
                ..
            })
            | Err(Why::NotPostQuantum) => Err(Why::NotPostQuantum),
            _ => Ok(()),
        }
    }

    /// Probes the access: the deploy key, or `token` written where the CLI
    /// reads it.
    ///
    /// # Errors
    ///
    /// The refusal the CLI printed.
    pub fn probe(&self, address: &str, token: Option<&str>) -> Result<Probed, Why> {
        let credential = self.credential(address);
        if let Some(token) = token {
            write_private(&credential, token.trim().as_bytes()).map_err(|_| Why::Failed)?;
        }
        let credential = credential.to_string_lossy().into_owned();
        let out = self
            .child(
                &self.belongings,
                &["--json"],
                &[
                    "probe",
                    "--address",
                    address.trim(),
                    "--credential",
                    &credential,
                ],
            )
            .output()
            .map_err(|_| Why::NoTools)?;
        // THE ANSWER IS THE LINE, whatever the status: a probe that does
        // not both read and write exits 1 and still says what it found.
        probed_of(&String::from_utf8_lossy(&out.stdout))
    }

    /// Where the CLI reads the access to `address`: the deploy key, or the
    /// token's file (HTTPS), in the scratch home.
    #[must_use]
    pub fn credential(&self, address: &str) -> PathBuf {
        Path::new(&self.scratch.keys()).join(match auth_of(address) {
            Auth::Ssh => DEPLOY_KEY,
            Auth::Token => TOKEN_FILE,
        })
    }

    /// Keeps `token` where the access to `address` is read, `0600`, without
    /// asking the repository: with no network, the first boot tries it.
    ///
    /// # Errors
    ///
    /// [`Why::NeedsAccess`] for a blank token, [`Why::Failed`] when it
    /// cannot be written (the scratch home is not made).
    pub fn keep(&self, address: &str, token: &str) -> Result<(), Why> {
        let token = token.trim();
        if token.is_empty() {
            return Err(Why::NeedsAccess);
        }
        write_private(&self.credential(address), token.as_bytes()).map_err(|_| Why::Failed)
    }

    /// Whether `passphrase` opens the repository: the CLI reads it from a
    /// `0600` file in a runtime folder of THIS check's own, under the
    /// scratch one, which goes however it ends.
    ///
    /// # Why a folder per check
    ///
    /// A check the person left (Esc) keeps running on its thread. With one
    /// file for every check, a second one typed meanwhile would write it, the
    /// first would end and delete it, and the second's CLI would read
    /// nothing — a correct passphrase refused as "no passphrase".
    ///
    /// # Errors
    ///
    /// The refusal the CLI printed — `wrong-passphrase` among them.
    pub fn verify(&self, address: &str, passphrase: &str) -> Result<u32, Why> {
        let _ = address;
        let runtime = Path::new(&self.scratch.runtime()).join(format!(
            "check-{}-{}",
            std::process::id(),
            CHECKS.fetch_add(1, Ordering::Relaxed)
        ));
        let runtime_text = runtime.to_string_lossy().into_owned();
        made(&runtime_text, 0o700).map_err(|_| Why::Failed)?;
        let said = write_private(&runtime.join(PASSPHRASE_FILE), passphrase.as_bytes())
            .map_err(|_| Why::Failed)
            .and_then(|()| {
                self.child(
                    &self.belongings,
                    &["--json"],
                    &[
                        "verify-passphrase",
                        "--home",
                        &self.scratch.home(),
                        "--runtime",
                        &runtime_text,
                    ],
                )
                .output()
                .map_err(|_| Why::NoTools)
            });
        let _ = std::fs::remove_dir_all(&runtime);
        verified_of(&String::from_utf8_lossy(&said?.stdout))
    }

    /// What the terminal-less mode checks of the repository, given the
    /// network's reading `way`: the access named kept where the CLI reads
    /// it, and — with a way out — probed reading and writing, and the
    /// passphrase opening the repository (or, an empty one, as long as a
    /// disk's). With no way out nothing is asked: the passphrase is held to
    /// a disk's length, the answers are recorded unchecked, and the first
    /// boot checks them. Fills in what was found.
    ///
    /// # Errors
    ///
    /// What stops the installation, before anything is touched.
    pub fn checks(&self, r: &mut Answers, way: Gate) -> Result<(), Refused> {
        let online = way.open();
        r.online = Some(online);
        r.belongings_empty = None;
        r.belongings_checked = false;
        self.prepare(&r.belongings, Some(Path::new(&r.belongings_access)), online)
            .map_err(|why| Refused::Access(Err(why)))?;
        if online {
            let probed = self
                .probe(&r.belongings, None)
                .map_err(|why| Refused::Access(Err(why)))?;
            if !probed.ready() {
                return Err(Refused::Access(Ok(probed)));
            }
            r.belongings_empty = Some(probed.empty);
        }
        if r.passphrase_proved() {
            self.verify(&r.belongings, &r.belongings_passphrase)
                .map_err(Refused::Passphrase)?;
        } else if r.belongings_passphrase.chars().count() < MINIMUM_PASSPHRASE {
            // An empty repository is MADE with this passphrase, and one not
            // asked may be: as long as a disk's, for the reason a disk's is.
            return Err(Refused::Answer(crate::model::Invalid::PassphraseTooShort));
        }
        r.belongings_checked = online;
        r.belongings_access = self
            .credential(&r.belongings)
            .to_string_lossy()
            .into_owned();
        Ok(())
    }
}

/// A child with none of the answers' variables — `nix`, which keeps the
/// installer's own home and caches.
fn bare(program: &Path) -> Command {
    let mut c = crate::executor::command(program);
    c.stdin(Stdio::null());
    c
}

/// The number of the passphrase's checks this process ran: each one's
/// runtime folder is its own ([`Reach::verify`]).
static CHECKS: AtomicU64 = AtomicU64::new(0);

/// A folder made with `mode`, or given it when it was there.
fn made(dir: &str, mode: u32) -> std::io::Result<()> {
    match std::fs::DirBuilder::new().mode(mode).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode))
}

/// A file written whole and born `0600` (`create_new` after the old one is
/// gone): never readable by anybody else, not even for an instant.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the distro's tree is named through the link the ISO puts in
    /// `/etc` — the unstable Nix refuses a path with a link in it, and the
    /// lock step failed on it —, or a tree that does not resolve is lost
    /// instead of said by the step that reads it.
    #[test]
    fn the_tree_is_named_by_its_real_path() {
        let dir = std::env::temp_dir().join(format!("ronoctua-reach-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("store/tree")).unwrap();
        std::os::unix::fs::symlink(dir.join("store/tree"), dir.join("link")).unwrap();
        let real = std::fs::canonicalize(dir.join("store/tree")).unwrap();
        assert_eq!(
            real_path(&dir.join("link").to_string_lossy()),
            real.to_string_lossy()
        );
        let gone = dir.join("gone").to_string_lossy().into_owned();
        assert_eq!(real_path(&gone), gone);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A scratch folder of the test's own, and a fake CLI that answers like
    /// the belongings' one: it records its command line and environment and
    /// prints what the test planted.
    struct Fake {
        dir: PathBuf,
        reach: Reach,
    }

    impl Fake {
        fn new(name: &str, script: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("ronoctua-reach-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let cli = dir.join("belongings");
            // WRITTEN BY A CHILD `/bin/sh`, never by this process: a file
            // this process holds open for writing while another test's
            // thread forks is inherited by that fork until it execs, and
            // running it in that window fails with `ETXTBSY`.
            let body = format!(
                "#!/bin/sh\necho \"$*\" >> {log}\nenv >> {log}.env\n{script}\n",
                log = dir.join("log").display()
            );
            let status = std::process::Command::new("/bin/sh")
                .args([
                    "-c",
                    "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
                    "sh",
                ])
                .arg(&cli)
                .arg(&body)
                .status()
                .unwrap();
            assert!(status.success(), "{}", cli.display());
            let reach = Reach {
                belongings: cli,
                nix: PathBuf::from("/nonexistent/nix"),
                scratch: Scratch {
                    root: dir.join("scratch").to_string_lossy().into_owned(),
                },
                distro: None,
            };
            Self { dir, reach }
        }

        fn log(&self) -> String {
            std::fs::read_to_string(self.dir.join("log")).unwrap_or_default()
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// FAILS IF: the scratch home is not private, its settings do not name
    /// the repository (the passphrase's check reads them), the host is not
    /// let, the access named is not where the CLI reads it, or a deploy
    /// key's public half is not handed back to show.
    #[test]
    fn the_scratch_home_is_made_private_with_the_repository_and_its_consent() {
        let f = Fake::new(
            "prepare",
            &format!(
                "mkdir -p \"$XDG_CONFIG_HOME/{name}/belongings-keys\"; \
                 echo PRIVATE > \"$XDG_CONFIG_HOME/{name}/belongings-keys/deploy\"; \
                 echo 'ssh-ed25519 AAAA ronoctua-belongings' > \"$XDG_CONFIG_HOME/{name}/belongings-keys/deploy.pub\"",
                name = ronoctua_xdg::NAME
            ),
        );
        let address = "git@example.org:someone/belongings.git";
        let public = f.reach.prepare(address, None, true).unwrap();
        assert_eq!(
            public.as_deref(),
            Some("ssh-ed25519 AAAA ronoctua-belongings")
        );
        assert!(f.log().starts_with("keygen"), "{}", f.log());
        let mode = |p: &str| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&f.reach.scratch.root), 0o711);
        assert_eq!(mode(&f.reach.scratch.home()), 0o700);
        assert_eq!(mode(&f.reach.scratch.runtime()), 0o700);
        assert_eq!(mode(&f.reach.scratch.keys()), 0o700);
        let settings = std::fs::read_to_string(
            Path::new(&f.reach.scratch.config()).join(ronoctua_belongings::prefs::FILE),
        )
        .unwrap();
        assert_eq!(
            Belongings::from_file(&settings).unwrap().repository,
            address
        );
        assert!(ronoctua_consent::allows_under(
            Path::new(&f.reach.scratch.config()),
            "example.org"
        ));
        // THE CHILD RAN IN THE SCRATCH HOME, and inherited no answer.
        let env = std::fs::read_to_string(f.dir.join("log.env")).unwrap();
        assert!(env.contains(&format!("HOME={}", f.reach.scratch.home())));
        assert!(
            !env.contains("RONOCTUA_INST_"),
            "an answer reached the child"
        );

        // A FILE NAMED (the terminal-less mode): copied where the CLI reads
        // it, `0600`, and no key made.
        let token = f.dir.join("token-file");
        std::fs::write(&token, "tok").unwrap();
        let https = "https://example.org/someone/belongings.git";
        assert_eq!(f.reach.prepare(https, Some(&token), true), Ok(None));
        let kept = Path::new(&f.reach.scratch.keys()).join(TOKEN_FILE);
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "tok");
        assert_eq!(mode(&kept.to_string_lossy()), 0o600);
        assert_eq!(
            f.reach.prepare(https, Some(&f.dir.join("missing")), true),
            Err(Why::NeedsAccess)
        );
        assert_eq!(
            f.reach.prepare("/srv/b.git", None, true),
            Err(Why::NoRepository)
        );
    }

    /// FAILS IF: an HTTPS address is taken without its server's transport
    /// asked first, a server that cannot be keyed post-quantum is not said
    /// before any token, another answer (the empty token refused) stops the
    /// address, or the empty token is left where the CLI reads the real one.
    #[test]
    fn an_https_address_asks_the_transport_before_the_token() {
        let https = "https://example.org/someone/belongings.git";
        for (answer, want) in [
            (
                r#"echo '{"read":false,"write":false,"empty":false,"why":"not-post-quantum"}'; exit 1"#,
                Err(Why::NotPostQuantum),
            ),
            (
                r#"echo '{"refused":"not-post-quantum"}'; exit 1"#,
                Err(Why::NotPostQuantum),
            ),
            (
                r#"echo '{"read":false,"write":false,"empty":false,"why":"needs-access"}'; exit 1"#,
                Ok(None),
            ),
        ] {
            let f = Fake::new("transport", answer);
            assert_eq!(f.reach.prepare(https, None, true), want, "{answer}");
            assert!(
                f.log().starts_with(&format!("probe --address {https} ")),
                "{}",
                f.log()
            );
            assert!(
                !Path::new(&f.reach.scratch.keys()).join(TOKEN_FILE).exists(),
                "the empty token stayed"
            );
        }
    }

    /// FAILS IF: the probe's answer is not read when the CLI exits 1 (a
    /// key that does not write yet), a token is not where the CLI reads
    /// it, or the access is named by its content instead of its file.
    #[test]
    fn a_probe_reads_the_answer_and_names_the_access_by_its_file() {
        let f = Fake::new(
            "probe",
            r#"echo '{"read":true,"write":false,"empty":false,"why":"refused"}'; exit 1"#,
        );
        f.reach
            .prepare(
                "https://example.org/b.git",
                Some(&{
                    let t = f.dir.join("t");
                    std::fs::write(&t, "old").unwrap();
                    t
                }),
                true,
            )
            .unwrap();
        let p = f
            .reach
            .probe("https://example.org/b.git", Some("Zq7-token\n"))
            .unwrap();
        assert!(p.read && !p.write);
        let line = f.log();
        assert!(line.contains("probe --address https://example.org/b.git --credential "));
        assert!(line.contains("--json"));
        assert!(
            !line.contains("Zq7-token"),
            "the token reached argv: {line}"
        );
        let token = Path::new(&f.reach.scratch.keys()).join(TOKEN_FILE);
        assert_eq!(std::fs::read_to_string(token).unwrap(), "Zq7-token");
    }

    /// The fake CLI's way to the runtime folder it was named (`--runtime`).
    const RUNTIME_OF: &str = r#"rt=""; prev=""; for a in "$@"; do if [ "$prev" = --runtime ]; then rt=$a; fi; prev=$a; done"#;

    /// FAILS IF: the passphrase reaches argv, outlives the check in its
    /// file, or a refusal is taken for an opening.
    #[test]
    fn the_passphrase_is_checked_through_a_file_that_goes() {
        let f = Fake::new(
            "verify",
            &format!(
                r#"{RUNTIME_OF}; cat "$rt/passphrase" > /dev/null && echo '{{"refused":"wrong-passphrase"}}'"#
            ),
        );
        f.reach
            .prepare(
                "git@example.org:b.git",
                Some(&{
                    let k = f.dir.join("k");
                    std::fs::write(&k, "key").unwrap();
                    k
                }),
                true,
            )
            .unwrap();
        assert_eq!(
            f.reach.verify("git@example.org:b.git", "Zq7-belongings"),
            Err(Why::WrongPassphrase)
        );
        assert!(!f.log().contains("Zq7"), "{}", f.log());
        let line = f.log();
        assert!(
            line.contains(&format!(
                "verify-passphrase --home {} --runtime {}/check-",
                f.reach.scratch.home(),
                f.reach.scratch.runtime()
            )),
            "{line}"
        );
        // THE CHECK'S FOLDER GOES with the passphrase in it.
        let left: Vec<_> = std::fs::read_dir(f.reach.scratch.runtime())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(
            left.is_empty(),
            "the passphrase outlived its check: {left:?}"
        );
    }

    /// FAILS IF: a check the person left (Esc), still running on its
    /// thread, deletes the passphrase of the check typed after it — the
    /// second refused for a passphrase its CLI never read.
    #[test]
    fn a_check_left_behind_never_takes_the_next_ones_passphrase() {
        let f = Fake::new(
            "verify-twice",
            &format!(
                r#"{RUNTIME_OF}; p=$(cat "$rt/passphrase"); if [ "$p" = slow ]; then sleep 1; fi; q=$(cat "$rt/passphrase" 2> /dev/null); if [ "$q" = "$p" ]; then echo '{{"format":2}}'; else echo '{{"refused":"no-passphrase"}}'; fi"#
            ),
        );
        f.reach
            .prepare(
                "git@example.org:b.git",
                Some(&{
                    let k = f.dir.join("k");
                    std::fs::write(&k, "key").unwrap();
                    k
                }),
                true,
            )
            .unwrap();
        let slow = {
            let reach = f.reach.clone();
            std::thread::spawn(move || reach.verify("git@example.org:b.git", "slow"))
        };
        std::thread::sleep(Duration::from_millis(300));
        let fast = f.reach.verify("git@example.org:b.git", "fast");
        assert_eq!(fast, Ok(2), "the check typed second was refused");
        assert_eq!(
            slow.join().unwrap(),
            Ok(2),
            "the check left behind lost its own passphrase"
        );
    }

    /// FAILS IF: with no network an HTTPS address's server is asked — the
    /// transport's probe is the CLI's, and it must not run —, the scratch
    /// home is not made all the same, or an SSH address gets no deploy key
    /// (made here: nothing about it needs the network).
    #[test]
    fn with_no_network_the_address_is_taken_as_typed() {
        let f = Fake::new(
            "offline-address",
            &format!(
                "mkdir -p \"$XDG_CONFIG_HOME/{name}/belongings-keys\"; \
                 echo PRIVATE > \"$XDG_CONFIG_HOME/{name}/belongings-keys/deploy\"; \
                 echo 'ssh-ed25519 AAAA ronoctua-belongings' > \"$XDG_CONFIG_HOME/{name}/belongings-keys/deploy.pub\"",
                name = ronoctua_xdg::NAME
            ),
        );
        let https = "https://github.com/someone/belongings.git";
        assert_eq!(f.reach.prepare(https, None, false), Ok(None));
        assert_eq!(f.log(), "", "the CLI ran with no network");
        let settings = std::fs::read_to_string(
            Path::new(&f.reach.scratch.config()).join(ronoctua_belongings::prefs::FILE),
        )
        .unwrap();
        assert_eq!(Belongings::from_file(&settings).unwrap().repository, https);
        let ssh = "git@github.com:someone/belongings.git";
        assert_eq!(
            f.reach.prepare(ssh, None, false).unwrap().as_deref(),
            Some("ssh-ed25519 AAAA ronoctua-belongings")
        );
        assert!(f.log().starts_with("keygen"), "{}", f.log());
        assert!(!f.log().contains("probe"), "{}", f.log());
    }

    /// FAILS IF: a token kept with no network is not where the CLI reads it,
    /// `0600` and trimmed, the repository is asked to keep it, or a blank
    /// one is kept.
    #[test]
    fn a_token_is_kept_without_asking_the_repository() {
        let f = Fake::new("keep", "exit 1");
        let https = "https://example.org/someone/belongings.git";
        f.reach.prepare(https, None, false).unwrap();
        assert_eq!(f.reach.keep(https, "Zq7-token\n"), Ok(()));
        let kept = Path::new(&f.reach.scratch.keys()).join(TOKEN_FILE);
        assert_eq!(kept, f.reach.credential(https));
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "Zq7-token");
        let mode = std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(f.log(), "", "the CLI ran to keep a token");
        assert_eq!(f.reach.keep(https, "  \n"), Err(Why::NeedsAccess));
    }

    /// A fake CLI that answers a probe with `probed` and a passphrase's
    /// check with `verified`.
    fn answering(probed: &str, verified: &str) -> String {
        format!(
            "case \"$1\" in probe) echo '{probed}';; verify-passphrase) echo '{verified}';; esac"
        )
    }

    /// The terminal-less mode's answers, the access named by a file of the
    /// test's own.
    fn named(f: &Fake, passphrase: &str) -> Answers {
        let access = f.dir.join("deploy");
        std::fs::write(&access, "key").unwrap();
        Answers {
            belongings: "git@example.org:someone/belongings.git".into(),
            belongings_access: access.to_string_lossy().into_owned(),
            belongings_passphrase: passphrase.into(),
            ..Answers::default()
        }
    }

    /// FAILS IF: without a terminal and with no network the repository is
    /// asked, the run is refused for having no network, the answers are not
    /// recorded unchecked (no network, nothing found, the access kept where
    /// the first boot reads it), or a passphrase shorter than a disk's is
    /// taken where nothing proves it; or, with a way out, a repository that
    /// reads and writes is not recorded checked — empty, or opened by the
    /// passphrase.
    #[test]
    fn the_terminal_less_checks_ask_the_repository_only_online() {
        let ready = r#"{"read":true,"write":true,"empty":false,"why":null}"#;
        let empty = r#"{"read":true,"write":true,"empty":true,"why":null}"#;
        let opened = r#"{"format":2}"#;
        let f = Fake::new("checks-offline", &answering(ready, opened));
        let mut r = named(&f, "Qw4-a-long-passphrase");
        assert_eq!(f.reach.checks(&mut r, Gate::NoLink), Ok(()));
        assert_eq!(f.log(), "", "the repository was asked with no network");
        assert_eq!(
            (r.online, r.belongings_empty, r.belongings_checked),
            (Some(false), None, false)
        );
        assert_eq!(
            r.belongings_access,
            f.reach.credential(&r.belongings).to_string_lossy()
        );
        let mut short = named(&f, "short");
        assert_eq!(
            f.reach.checks(&mut short, Gate::Unconnected),
            Err(Refused::Answer(crate::model::Invalid::PassphraseTooShort))
        );
        assert_eq!(f.log(), "");
        // ONLINE: probed, and opened by the passphrase — which a repository
        // that opens with it does not hold to a disk's length.
        let mut r = named(&f, "short");
        assert_eq!(f.reach.checks(&mut r, Gate::Open), Ok(()));
        assert!(f.log().contains("probe --address"), "{}", f.log());
        assert!(f.log().contains("verify-passphrase"), "{}", f.log());
        assert_eq!(
            (r.online, r.belongings_empty, r.belongings_checked),
            (Some(true), Some(false), true)
        );
        // ONLINE AND EMPTY: nothing to open, as long as a disk's.
        let f = Fake::new("checks-empty", &answering(empty, opened));
        let mut r = named(&f, "Qw4-a-long-passphrase");
        assert_eq!(f.reach.checks(&mut r, Gate::Open), Ok(()));
        assert!(!f.log().contains("verify-passphrase"), "{}", f.log());
        assert_eq!(
            (r.online, r.belongings_empty, r.belongings_checked),
            (Some(true), Some(true), true)
        );
    }

    #[test]
    fn a_program_is_found_on_path_by_its_path() {
        let found = on_path("sh").expect("sh is on PATH");
        assert!(found.is_absolute() && found.ends_with("sh"));
        assert_eq!(on_path("ronoctua-no-such-program"), None);
    }
}
