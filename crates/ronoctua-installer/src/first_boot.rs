//! What the first boot does that the installation no longer does: the
//! person's belongings, which need the network.
//!
//! # Why it moved
//!
//! An installation reaches no network (the ISO carries everything it
//! builds, `nix/installer/offline.nix`), and the belongings are a repository
//! somewhere else: copying their plain part out, pinning the person's key,
//! applying their declarations and restoring their home all ask a server.
//! The installer keeps what it can do alone — the access in the person's
//! home, the consent for the repository's host, `belongings.json`, the
//! programs left out applied as root's staged declines — and leaves THIS
//! record, with the passphrase sealed in the TPM beside it, for the machine
//! to finish once it is up and reaches the network.
//!
//! # The sequence
//!
//! The installation's own, against the running machine instead of `/mnt`
//! (`ronoctua-first-boot`, a root unit that tries again ever later until it
//! is done):
//!
//! 1. the repository probed AS the person, with their access and consent:
//!    unreachable waits, and what it says decides restore or `init`;
//! 2. a repository with something in it: its plain part copied out as the
//!    person, the machine's choice signed in (`plain --decline`), the key
//!    the passphrase opened pinned by root, the copy applied by root's
//!    applier, and the home restored as the person;
//! 3. an empty one: the declines in the person's mirror, the repository
//!    made (`init`), and the key it made pinned.
//!
//! Done, the record and the sealed passphrase go; the passphrase existed
//! nowhere else on disk.

use serde::{Deserialize, Serialize};

use crate::model::Answers;

/// Where the installation leaves the record and the sealed passphrase, on
/// the machine: `/persist`, out of the root's wipe (`nix/modules/persist.nix`),
/// root's alone.
pub const DIRECTORY: &str = "/persist/etc/ronoctua/first-boot";

/// The record's file, under [`DIRECTORY`].
pub const RECORD: &str = "record.json";

/// The name the passphrase is sealed under: `systemd-creds` authenticates
/// it with the blob, so a blob sealed under another name never opens as
/// this one.
pub const PASSPHRASE_NAME: &str = "belongings-passphrase";

/// The record's format; a newer one is refused, never guessed.
pub const FORMAT: u32 = 1;

/// What the first boot needs to finish the belongings. No secret: an
/// address, the machine's name and its choice of programs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Record {
    /// [`FORMAT`].
    pub format: u32,
    /// The repository's address.
    pub address: String,
    /// The machine's name, which the repository's index remembers.
    pub machine: String,
    /// Whether the repository was found empty while installing; `None` when
    /// the installation could not ask.
    pub empty: Option<bool>,
    /// Whether the access and the passphrase were proved while installing.
    pub checked: bool,
    /// The machine's choice of the offered programs, which the copy of the
    /// plain part signs in (`plain --decline`); `None` unless `full`.
    pub choice: Option<ronoctua_person::choice::Choice>,
    /// The programs left out, as the declines an `init` signs from the
    /// person's mirror; `None` when nothing is left out.
    pub declined: Option<ronoctua_person::declined::Declined>,
}

impl Record {
    /// The record of an installation: the answers, at the moment the
    /// programs were left out.
    #[must_use]
    pub fn of(r: &Answers, at: &ronoctua_person::Stamp) -> Self {
        Self {
            format: FORMAT,
            address: r.belongings.clone(),
            machine: r.hostname.clone(),
            empty: r.belongings_empty,
            checked: r.belongings_checked,
            choice: crate::plan::machine_choice(r, at),
            declined: crate::plan::left_out_declined(r, at),
        }
    }

    /// The record as its file holds it.
    #[must_use]
    pub fn to_file(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// A record read back; one of another format is refused.
    ///
    /// # Errors
    ///
    /// The text is not a record, or names a format this program does not
    /// know.
    pub fn from_file(text: &str) -> anyhow::Result<Self> {
        let r: Self = serde_json::from_str(text)?;
        anyhow::ensure!(
            r.format == FORMAT,
            "the first boot's record is format {}, and this program reads {FORMAT}",
            r.format
        );
        Ok(r)
    }
}

/// How `ronoctua-first-boot` ends, for its unit: done, waiting for the
/// network (tried again later), or needing the person (the passphrase did
/// not open; the belongings' view asks it).
pub mod exit {
    /// Done: the record and the sealed passphrase are gone.
    pub const DONE: u8 = 0;
    /// A step failed; tried again later.
    pub const FAILED: u8 = 1;
    /// The repository did not answer; tried again later.
    pub const OFFLINE: u8 = 10;
    /// The TPM did not give the passphrase back: nothing is tried again,
    /// and the person restores from the belongings' view.
    pub const NEEDS_YOU: u8 = 12;
}

/// The person's runtime folder for this run: theirs, `0700`, in RAM — the
/// passphrase's files never touch a disk, and the folder goes however the
/// run ends.
pub const RUN: &str = "/run/ronoctua-first-boot";

/// Why a run stopped short.
#[derive(Debug)]
enum Stop {
    /// The repository did not answer.
    Offline,
    /// Nothing tried again would end differently: the person decides.
    NeedsYou(String),
    /// A step failed; tried again later.
    Failed(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for Stop {
    fn from(e: E) -> Self {
        Self::Failed(e.into())
    }
}

/// The first boot's work, once — what `ronoctua-first-boot` runs. `say`
/// hears every line worth the journal. Returns the [`exit`] code.
pub fn run(say: &mut dyn FnMut(String)) -> u8 {
    let dir = std::path::Path::new(DIRECTORY);
    let text = match std::fs::read_to_string(dir.join(RECORD)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return exit::DONE,
        Err(e) => {
            say(format!("read the record: {e}"));
            return exit::FAILED;
        }
    };
    let record = match Record::from_file(&text) {
        Ok(record) => record,
        Err(e) => {
            say(format!("the record does not read: {e:#}"));
            return exit::NEEDS_YOU;
        }
    };
    let done = finish(&record, say);
    let _ = std::fs::remove_dir_all(RUN);
    match done {
        Ok(()) => {
            let _ = std::fs::remove_file(dir.join(RECORD));
            let _ = std::fs::remove_file(ronoctua_network::secret::path_at(
                DIRECTORY,
                PASSPHRASE_NAME,
            ));
            say("the belongings are done".to_owned());
            exit::DONE
        }
        Err(Stop::Offline) => {
            say(format!(
                "{} did not answer; tried again later",
                record.address
            ));
            exit::OFFLINE
        }
        Err(Stop::NeedsYou(why)) => {
            say(why);
            exit::NEEDS_YOU
        }
        Err(Stop::Failed(e)) => {
            say(format!("{e:#}"));
            exit::FAILED
        }
    }
}

/// Where a first boot's files are: the machine's, or a test's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The person's name.
    pub user: String,
    /// The person's uid, as `ronoctua-person pin --owner` takes it.
    pub owner: String,
    /// The person's home.
    pub home: String,
    /// The run's folder, the person's ([`RUN`] on the machine).
    pub run: String,
    /// The machine directory.
    pub machine: String,
    /// Root's lock of the person's data on the machine.
    pub lock: String,
}

/// The mark the printed plan shows for the person's uid when this machine
/// has no such person (a dry run away from the machine).
pub const OWNER_MARK: &str = "{person-uid}";

impl Places {
    /// This machine's: the person of `/etc/passwd` (the mark for the uid
    /// when there is none — a printed plan away from the machine).
    #[must_use]
    pub fn of_the_machine() -> Self {
        let user = crate::plan::target_user().to_owned();
        let owner = crate::executor::ids_in(std::path::Path::new("/etc/passwd"), &user)
            .map_or_else(|_| OWNER_MARK.to_owned(), |(uid, _)| uid.to_string());
        Self {
            home: format!("/home/{user}"),
            user,
            owner,
            run: RUN.to_owned(),
            machine: env!("RONOCTUA_MACHINE_DIR").to_owned(),
            lock: ronoctua_person::root::LOCK.to_owned(),
        }
    }

    /// A file of the run's folder.
    fn file(&self, name: &str) -> String {
        format!("{}/{name}", self.run)
    }

    /// The person's access folder.
    fn keys(&self) -> String {
        format!(
            "{}/.config/{}/{}",
            self.home,
            ronoctua_xdg::NAME,
            ronoctua_belongings::places::KEYS
        )
    }
}

/// What a write of the sequence holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holds {
    /// The belongings' passphrase (`0600`, no line break).
    Passphrase,
    /// The machine's choice of programs.
    Choice,
    /// The programs left out, as declines.
    Declined,
}

/// One act of the first boot's sequence — DATA, so it is printed before it
/// runs (`ronoctua-first-boot --plan`) and a test replays it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// A file written, the person's.
    Write {
        /// Where.
        path: String,
        /// What.
        holds: Holds,
    },
    /// A command run.
    Run {
        /// As the person (their home's roots in the environment), or root.
        as_person: bool,
        /// The command.
        argv: Vec<String>,
        /// It runs only when this path exists.
        only_if: Option<String>,
    },
}

impl Act {
    /// The act as the printed plan shows it, in the installer's form:
    /// `[<person>] ` before what the person runs, ` [if <path>]` after a
    /// condition, `write <path>` for a write.
    #[must_use]
    pub fn shown(&self, user: &str) -> String {
        match self {
            Self::Write { path, .. } => format!("{} {path}", crate::plan::WRITE),
            Self::Run {
                as_person,
                argv,
                only_if,
            } => {
                let mut out = if *as_person {
                    format!("[{user}] ")
                } else {
                    String::new()
                };
                out.push_str(&argv.join(" "));
                if let Some(path) = only_if {
                    out.push_str(&format!(" [if {path}]"));
                }
                out
            }
        }
    }
}

/// The sequence (the module's header), once the probe said whether the
/// repository is `empty`. Pure.
#[must_use]
pub fn sequence(record: &Record, empty: bool, p: &Places) -> Vec<Act> {
    use ronoctua_belongings::places::PASSPHRASE_FILE;
    use ronoctua_belongings::plain::SIGNER_FILE;
    let run = |as_person: bool, argv: &[&str], only_if: Option<String>| Act::Run {
        as_person,
        argv: argv.iter().map(|&a| a.to_owned()).collect(),
        only_if,
    };
    let signer = p.file(SIGNER_FILE);
    let mut acts = vec![Act::Write {
        path: p.file(PASSPHRASE_FILE),
        holds: Holds::Passphrase,
    }];
    if empty {
        // AN EMPTY REPOSITORY: the declines in the person's mirror, which
        // `init`'s first sync signs, then the repository made, then its key
        // pinned.
        if record.declined.is_some() {
            let state = format!("{}/.local/state/{}", p.home, ronoctua_xdg::NAME);
            acts.push(Act::Write {
                path: ronoctua_person::mirror_declined(std::path::Path::new(&state))
                    .to_string_lossy()
                    .into_owned(),
                holds: Holds::Declined,
            });
        }
        acts.push(run(
            true,
            &[
                crate::plan::BELONGINGS,
                "init",
                "--home",
                &p.home,
                "--runtime",
                &p.run,
                "--machine",
                &record.machine,
                "--progress",
            ],
            None,
        ));
        acts.push(run(
            false,
            &[
                crate::plan::PERSON,
                "pin",
                "--pin",
                &signer,
                "--owner",
                &p.owner,
                "--machine",
                &p.machine,
                "--lock",
                &p.lock,
            ],
            Some(signer.clone()),
        ));
        return acts;
    }
    // A REPOSITORY WITH SOMETHING IN IT: the plain part copied out (the
    // machine's choice signed in), its key pinned, the copy applied, the
    // home restored.
    let plain_passphrase = p.file("plain-passphrase");
    let out = p.file("plain");
    let choice = p.file("choice.json");
    acts.push(Act::Write {
        path: plain_passphrase.clone(),
        holds: Holds::Passphrase,
    });
    let keys = p.keys();
    let mut plain = vec![
        crate::plan::BELONGINGS,
        "plain",
        "--repository",
        record.address.as_str(),
        "--keys",
        keys.as_str(),
        "--out",
        out.as_str(),
        "--passphrase",
        plain_passphrase.as_str(),
        "--signer",
        signer.as_str(),
    ];
    if record.choice.is_some() {
        acts.push(Act::Write {
            path: choice.clone(),
            holds: Holds::Choice,
        });
        plain.extend(["--decline", choice.as_str()]);
    }
    plain.push("--json");
    acts.push(run(true, &plain, None));
    acts.push(run(
        false,
        &[
            crate::plan::PERSON,
            "pin",
            "--pin",
            &signer,
            "--machine",
            &p.machine,
            "--lock",
            &p.lock,
        ],
        Some(signer.clone()),
    ));
    acts.push(run(
        false,
        &[
            crate::plan::PERSON,
            "apply",
            "--staged",
            &out,
            "--machine",
            &p.machine,
            "--owner",
            "root",
            "--lock",
            &p.lock,
        ],
        Some(format!("{out}/{}", ronoctua_person::tree::PERSON)),
    ));
    acts.push(run(
        true,
        &[
            crate::plan::BELONGINGS,
            "restore",
            "--home",
            &p.home,
            "--runtime",
            &p.run,
            "--machine",
            &record.machine,
            "--progress",
        ],
        None,
    ));
    acts
}

/// The plan a record makes, as `ronoctua-first-boot --plan` prints it: one
/// line an act, for a repository that is `empty` or not.
#[must_use]
pub fn printed(record: &Record, empty: bool) -> String {
    let p = Places::of_the_machine();
    let mut out = String::new();
    for act in sequence(record, empty, &p) {
        out.push_str(&act.shown(&p.user));
        out.push('\n');
    }
    out
}

/// The sequence (the module's header), against this machine.
fn finish(record: &Record, say: &mut dyn FnMut(String)) -> Result<(), Stop> {
    use ronoctua_belongings::places::{DEPLOY_KEY, TOKEN_FILE};
    use ronoctua_belongings::prefs::Auth;
    use std::path::Path;

    let p = Places::of_the_machine();
    let ids = crate::executor::ids_in(Path::new("/etc/passwd"), &p.user)?;
    // THE RUN'S FOLDER, the person's, before anything of theirs runs: it is
    // their runtime folder too (`XDG_RUNTIME_DIR`), where the belongings
    // keep what a run needs — the probe's scratch repository first.
    let run = Path::new(&p.run);
    let _ = std::fs::remove_dir_all(run);
    crate::executor::make_folders(run, Some(ids), 0o700)?;
    let mut env = crate::plan::home_env_of(&p.home);
    env.push(("XDG_RUNTIME_DIR".to_owned(), p.run.clone()));
    let credential = Path::new(&p.keys()).join(match crate::access::auth_of(&record.address) {
        Auth::Ssh => DEPLOY_KEY,
        Auth::Token => TOKEN_FILE,
    });
    let as_person = |args: &[String]| {
        let mut c = std::process::Command::new("setpriv");
        c.arg(format!("--reuid={}", ids.0))
            .arg(format!("--regid={}", ids.1))
            .arg("--clear-groups")
            .arg("--")
            .args(args)
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        c
    };

    // 1. THE REPOSITORY, asked as the person: unreachable waits.
    let out = as_person(&[
        crate::plan::BELONGINGS.to_owned(),
        "--json".to_owned(),
        "probe".to_owned(),
        "--address".to_owned(),
        record.address.trim().to_owned(),
        "--credential".to_owned(),
        credential.to_string_lossy().into_owned(),
    ])
    .output()
    .map_err(|e| anyhow::anyhow!("run the probe: {e}"))?;
    let probed = match crate::access::probed_of(&String::from_utf8_lossy(&out.stdout)) {
        Ok(p) => p,
        Err(ronoctua_belongings::Why::Unreachable) => return Err(Stop::Offline),
        Err(why) => {
            // WHAT THE BELONGINGS SAID, for the journal: a refusal's id alone
            // does not say which file.
            return Err(Stop::Failed(anyhow::anyhow!(
                "the probe refused: {}: {}",
                why.id(),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
    };
    if probed.why == Some(ronoctua_belongings::Why::Unreachable) {
        return Err(Stop::Offline);
    }
    if !probed.ready() {
        return Err(Stop::Failed(anyhow::anyhow!(
            "the access does not read and write {} yet",
            record.address
        )));
    }

    // 2. THE PASSPHRASE, from the TPM.
    let Ok(passphrase) = ronoctua_network::secret::open_at(DIRECTORY, PASSPHRASE_NAME) else {
        return Err(Stop::NeedsYou(
            "the TPM did not give the passphrase back: restore from the belongings' view"
                .to_owned(),
        ));
    };

    // 3. THE SEQUENCE.
    say(if probed.empty {
        "making the repository".to_owned()
    } else {
        "copying the plain part and restoring the home".to_owned()
    });
    for act in sequence(record, probed.empty, &p) {
        match act {
            Act::Write { path, holds } => {
                let (bytes, mode) = match holds {
                    Holds::Passphrase => (passphrase.clone(), 0o600),
                    Holds::Choice => (
                        record
                            .choice
                            .as_ref()
                            .map(ronoctua_person::to_json)
                            .unwrap_or_default(),
                        0o644,
                    ),
                    Holds::Declined => (
                        record
                            .declined
                            .as_ref()
                            .map(ronoctua_person::to_json)
                            .unwrap_or_default(),
                        0o644,
                    ),
                };
                let path = Path::new(&path);
                if let Some(parent) = path.parent() {
                    crate::executor::make_folders(parent, Some(ids), 0o755)?;
                }
                crate::executor::write_whole(path, bytes.as_bytes(), mode, Some(ids))?;
            }
            Act::Run {
                as_person: person,
                argv,
                only_if,
            } => {
                if only_if.is_some_and(|path| !Path::new(&path).exists()) {
                    continue;
                }
                let mut c = if person {
                    as_person(&argv)
                } else {
                    let mut c = std::process::Command::new(&argv[0]);
                    c.args(&argv[1..]);
                    c
                };
                let what = argv[..2.min(argv.len())].join(" ");
                let status = c.status().map_err(|e| anyhow::anyhow!("run {what}: {e}"))?;
                if !status.success() {
                    return Err(Stop::Failed(anyhow::anyhow!("{what} ended with {status}")));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn places() -> Places {
        Places {
            user: "noctua".to_owned(),
            owner: "1000".to_owned(),
            home: "/home/noctua".to_owned(),
            run: "/run/ronoctua-first-boot".to_owned(),
            machine: "/etc/nixos".to_owned(),
            lock: "/run/ronoctua-person/machine.lock".to_owned(),
        }
    }

    fn record(choice: bool) -> Record {
        let at = ronoctua_person::Stamp::from_unix_s(0);
        Record {
            format: FORMAT,
            address: "git@example.org:p/belongings.git".to_owned(),
            machine: "noctua-elsewhere".to_owned(),
            empty: None,
            checked: false,
            choice: choice.then(|| {
                ronoctua_person::choice::Choice::new(
                    "noctua-elsewhere",
                    at.clone(),
                    vec!["vlc".to_owned()],
                    vec![],
                )
            }),
            declined: None,
        }
    }

    /// FAILS IF: a repository with something in it is not copied out as the
    /// person (the choice signed in when there is one), its key pinned and
    /// the copy applied by root, and the home restored as the person — in
    /// that order — or an empty one is not made as the person and its key
    /// pinned; or the passphrase reaches a command line.
    #[test]
    fn the_first_boot_runs_the_installations_own_sequence() {
        let p = places();
        let shown: Vec<String> = sequence(&record(true), false, &p)
            .iter()
            .map(|a| a.shown(&p.user))
            .collect();
        let at = |needle: &str| {
            shown
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("no `{needle}` in {shown:#?}"))
        };
        let order = [
            "write /run/ronoctua-first-boot/choice.json",
            "[noctua] ronoctua-belongings plain ",
            "ronoctua-person pin ",
            "ronoctua-person apply ",
            "[noctua] ronoctua-belongings restore ",
        ];
        let places: Vec<usize> = order.iter().map(|n| at(n)).collect();
        assert!(places.windows(2).all(|w| w[0] < w[1]), "{shown:#?}");
        assert!(shown[at("plain ")].contains("--decline /run/ronoctua-first-boot/choice.json"));
        assert!(shown[at("person apply")].ends_with("[if /run/ronoctua-first-boot/plain/person]"));
        assert!(!shown[at("person pin")].starts_with('['), "root pins");
        // NO CHOICE, NO DECLINE.
        let none: Vec<String> = sequence(&record(false), false, &p)
            .iter()
            .map(|a| a.shown(&p.user))
            .collect();
        assert!(
            !none
                .iter()
                .any(|l| l.contains("--decline") || l.contains("choice.json"))
        );
        // AN EMPTY ONE: made, then its key pinned for the person.
        let made: Vec<String> = sequence(&record(true), true, &p)
            .iter()
            .map(|a| a.shown(&p.user))
            .collect();
        let init = made
            .iter()
            .position(|l| l.starts_with("[noctua] ronoctua-belongings init "))
            .unwrap();
        let pin = made
            .iter()
            .position(|l| l.contains("ronoctua-person pin "))
            .unwrap();
        assert!(init < pin && made[pin].contains("--owner 1000"));
        assert!(
            !made
                .iter()
                .any(|l| l.contains("plain ") || l.contains("restore "))
        );
        for act in sequence(&record(true), false, &p) {
            if let Act::Run { argv, .. } = act {
                assert!(!argv.iter().any(|a| a.contains("passphrase")
                    && !a.starts_with('/')
                    && !a.starts_with("--")));
            }
        }
    }

    #[test]
    fn a_record_crosses_its_file_and_a_newer_one_is_refused() {
        let r = Record {
            format: FORMAT,
            address: "git@example.org:p/belongings.git".to_owned(),
            machine: "noctua-elsewhere".to_owned(),
            empty: None,
            checked: false,
            choice: None,
            declined: None,
        };
        assert_eq!(Record::from_file(&r.to_file()).unwrap(), r);
        let newer = r.to_file().replace("\"format\": 1", "\"format\": 2");
        assert!(Record::from_file(&newer).is_err());
        let unknown = r
            .to_file()
            .replace("\"checked\"", "\"secret\": \"x\",\n  \"checked\"");
        assert!(
            Record::from_file(&unknown).is_err(),
            "an unknown field is refused"
        );
    }
}
