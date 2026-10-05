//! Runs the [`plan`](crate::plan)'s steps — section 7.1, milestone 8.
//!
//! # What this module does NOT do
//!
//! It decides nothing. The sequence comes from [`plan::steps`], the files'
//! contents come from pure functions down here, and what is left is calling
//! processes and passing the output along. Keeping the decisions out of here is what allows
//! testing the part that matters without a disk, without root and without a VM.
//!
//! # A secret never goes through `argv`
//!
//! `/proc/<pid>/cmdline` is readable by any process on the machine. The
//! passphrase and the password go in through a FILE (mode 0600) or through `stdin`, never
//! as an argument — and that is why the three writing steps are internal
//! ([`plan::WRITE`]) instead of an `echo … > file`.
//!
//! # Why stop at the first error
//!
//! An installer that carries on after `disko` fails builds a system
//! on a disk in an unknown state. Stopping leaves the machine with no system,
//! which is bad; carrying on leaves the machine with a system that looks fine and is not,
//! which is worse. Whoever stops can start over — the ISO is still on the USB stick.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result, bail};

use crate::model::Answers;
use crate::plan::SEAL;
use crate::plan::{
    HOST_NIX_PATH, HOST_NIX_WORK_PATH, LUKS_KEY_PATH, PASSWORD_HASH_PATH, SWAPFILE_PATH, Step,
    TARGET_PASSWD, WIFI_PROFILE_PATH, WRITE, belongings_path, host_nix, steps, target_user,
    wifi_profile,
};

/// What happens during execution.
///
/// The TUI draws this; the terminal-less mode prints it. Neither of them knows
/// how to run anything, and the executor does not know how to draw.
#[derive(Debug, Clone)]
pub enum Event {
    /// A step has started. `label` is a text **key** (invariant 8).
    Started {
        /// The index, from zero.
        index: usize,
        /// Total number of steps.
        total: usize,
        /// The step's text key.
        label: &'static str,
    },
    /// One line of the command's output.
    Line(String),
    /// The step finished successfully.
    Finished {
        /// The index, from zero.
        index: usize,
    },
}

/// The write mode of a file produced by the installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secrecy {
    /// Public content: it may appear in a log and in a screenshot.
    Public,
    /// Secret content: mode 0600 and **never** passed on as an event.
    Secret,
}

/// A file the installer writes on its own.
#[derive(Debug, Clone)]
pub struct Write {
    /// Where.
    pub path: String,
    /// What.
    pub content: String,
    /// Whether it may be shown.
    pub secrecy: Secrecy,
    /// Whose it is on the target, when not root's: the file and every folder
    /// the write makes are given to this user (read from the target's
    /// `/etc/passwd`).
    pub owner: Option<String>,
}

/// The file a step writes, if it is a writing step.
///
/// A **pure** function, and it is what the tests exercise: the difference between
/// writing the passphrase in 0600 and in 0644 shows up on no screen, only
/// here.
///
/// # Errors
///
/// The step is a writing one but the path is not one of the known ones — a sign that
/// `plan::steps` gained a step and this module did not hear about it.
pub fn write_of(step: &Step, r: &Answers) -> Result<Option<Write>> {
    if step.command.first().map(String::as_str) != Some(WRITE) {
        return Ok(None);
    }
    let path = step
        .command
        .get(1)
        .context("a write step with no path")?
        .clone();
    let e = match path.as_str() {
        LUKS_KEY_PATH => Write {
            path,
            // Node trailing `\n`: `cryptsetup` treats the whole file as the
            // key, byte by byte. A line break becomes part of the
            // passphrase, and the person never unlocks again by typing what
            // they chose — a silent, irreversible failure.
            content: r.passphrase.clone(),
            secrecy: Secrecy::Secret,
            owner: None,
        },
        PASSWORD_HASH_PATH => Write {
            path,
            // Filled in by `run_command`, which is what has `mkpasswd`. Here only
            // the format: one line, with a break, as shadow expects.
            content: String::new(),
            secrecy: Secrecy::Secret,
            owner: None,
        },
        // THE NETWORK PROFILE. Secret because the password is inside it — and
        // NetworkManager requires `0600` anyway: a more permissive profile
        // is REFUSED by it, which turns a permission error
        // into a network that does not connect instead of a secret
        // readable by everyone.
        WIFI_PROFILE_PATH => Write {
            path,
            content: wifi_profile(r),
            secrecy: Secrecy::Secret,
            owner: None,
        },
        // BOTH: the working directory's (which `disko` reads, before there
        // is a disk) and the target's. The content is the same; what changes between
        // them is the `resume_offset`, which only exists after the swap.
        HOST_NIX_PATH | HOST_NIX_WORK_PATH => Write {
            path,
            content: host_nix(r),
            secrecy: Secrecy::Public,
            owner: None,
        },
        // THE BELONGINGS' SETTINGS, in the person's home and THEIRS: the view
        // writes this file as them, and a root-owned one would refuse every
        // change. Public: an address, never a credential.
        belongings if belongings == belongings_path() => Write {
            path,
            content: ronoctua_belongings::prefs::Belongings::with_repository(&r.belongings)
                .to_file(),
            secrecy: Secrecy::Public,
            owner: Some(target_user().to_owned()),
        },
        otherwise => bail!(
            "a write step for `{otherwise}`, which this module does not know.\n\
             \n\
             A new step in `plan::steps` needs a case here —\n\
             otherwise it becomes a `write` that writes nothing."
        ),
    };
    Ok(Some(e))
}

/// Reads the swap's offset from `btrfs inspect-internal map-swapfile`'s output.
///
/// The output is a single line, like `123456`, but `btrfs` has changed the
/// format between versions and some builds print a header. Taking the first
/// number in the output is what survives both forms.
#[must_use]
pub fn offset_from(output: &str) -> Option<u64> {
    output
        .split_whitespace()
        .find_map(|t| t.trim().parse::<u64>().ok())
}

/// A user's uid and gid, from the text of an `/etc/passwd`. Pure.
#[must_use]
pub fn ids_of(passwd: &str, user: &str) -> Option<(u32, u32)> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != user {
            return None;
        }
        let _password = fields.next()?;
        let uid = fields.next()?.parse().ok()?;
        let gid = fields.next()?.parse().ok()?;
        Some((uid, gid))
    })
}

/// Writes a file, creating the missing folders above it.
///
/// BORN WITH ITS MODE (`create_new` + mode, 0600 when secret, 0644 when not):
/// creating it and fixing the mode afterwards leaves a window in which the
/// secret is readable. Through a draft beside it, renamed over: a path the
/// plan writes twice (the working `host.nix`) is replaced whole, never
/// truncated in place.
fn write_out(e: &Write) -> Result<()> {
    write_out_with(e, Path::new(TARGET_PASSWD))
}

/// [`write_out`], with the `/etc/passwd` an owner is read from — a test's.
fn write_out_with(e: &Write, passwd: &Path) -> Result<()> {
    let p = Path::new(&e.path);
    let ids = match &e.owner {
        Some(user) => {
            let text = std::fs::read_to_string(passwd)
                .with_context(|| format!("read {}", passwd.display()))?;
            Some(
                ids_of(&text, user)
                    .with_context(|| format!("the user `{user}` is not in {}", passwd.display()))?,
            )
        }
        None => None,
    };
    if let Some(parent) = p.parent() {
        // THE FOLDERS THIS WRITE MAKES are the owner's too: a
        // `~/.config/ronoctua` born root's would refuse the person's own
        // preferences.
        let mut missing: Vec<&Path> = parent.ancestors().take_while(|a| !a.exists()).collect();
        missing.reverse();
        for dir in missing {
            std::fs::create_dir(dir).with_context(|| format!("create {}", dir.display()))?;
            if let Some((uid, gid)) = ids {
                std::os::unix::fs::chown(dir, Some(uid), Some(gid))
                    .with_context(|| format!("give {} to its owner", dir.display()))?;
            }
        }
    }
    let name = p
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let draft = p.with_file_name(format!(".{name}.install-draft"));
    let _ = std::fs::remove_file(&draft);
    let mode = if e.secrecy == Secrecy::Secret {
        0o600
    } else {
        0o644
    };
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&draft)
        .with_context(|| format!("create {}", draft.display()))?;
    f.write_all(e.content.as_bytes())
        .and_then(|()| f.sync_all())
        .with_context(|| format!("write {}", draft.display()))?;
    drop(f);
    if let Some((uid, gid)) = ids {
        std::os::unix::fs::chown(&draft, Some(uid), Some(gid))
            .with_context(|| format!("give {} to its owner", p.display()))?;
    }
    std::fs::rename(&draft, p).with_context(|| format!("write {}", p.display()))?;
    Ok(())
}

/// Runs a command and passes each line of output along.
///
/// ONE PIPE FOR BOTH STREAMS, read to the end. Two pipes read one after the
/// other deadlock: `nixos-install` writes its build log to stderr, and once
/// that pipe's buffer (64 KiB) is full the child blocks on it while this end
/// is still waiting for stdout to close — an install that stops forever with
/// the processor idle. Joined, the lines also arrive in the order they were
/// written.
fn run(command: &[String], ver: &mut dyn FnMut(Event)) -> Result<()> {
    let (prog, args) = command.split_first().context("an empty command")?;
    let (output, writer) = std::io::pipe().context("a pipe for the output")?;
    // The `Command` is a temporary, dropped at the end of this statement and
    // with it this process's write ends: the read below ends when the child's
    // do.
    let mut child = Command::new(prog)
        .args(args)
        .stdout(writer.try_clone().context("a pipe for the output")?)
        // It joins the two: `nixos-install` speaks through stderr, and an
        // installation that looks stuck for half an hour because the output is
        // on another channel is worse than having no output at all.
        .stderr(writer)
        .spawn()
        .with_context(|| format!("could not run `{prog}`"))?;

    // BY BYTES, NOT `lines()`: a line that is not UTF-8 would end that
    // iterator and leave the pipe undrained — the same deadlock.
    for line in BufReader::new(output).split(b'\n') {
        let Ok(line) = line else { break };
        ver(Event::Line(String::from_utf8_lossy(&line).into_owned()));
    }
    let st = child.wait().context("wait for the command")?;
    if !st.success() {
        bail!("`{prog}` exited with {st}");
    }
    Ok(())
}

/// The password's hash, through `mkpasswd`.
///
/// The password goes in through `stdin`, never through `argv`.
fn password_hash(password: &str) -> Result<String> {
    let mut child = Command::new("mkpasswd")
        .args(["--method=yescrypt", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("could not run `mkpasswd`")?;
    child
        .stdin
        .take()
        .context("mkpasswd with no stdin")?
        .write_all(password.as_bytes())
        .context("pass the password to mkpasswd")?;
    let output = child.wait_with_output().context("wait for mkpasswd")?;
    if !output.status.success() {
        bail!("`mkpasswd` exited with {}", output.status);
    }
    let h = String::from_utf8(output.stdout)
        .context("mkpasswd gave back something that is not UTF-8")?
        .trim()
        .to_owned();
    if h.is_empty() {
        bail!("`mkpasswd` gave back nothing");
    }
    Ok(format!("{h}\n"))
}

/// Runs the whole installation.
///
/// It stops at the first error, and says which step it stopped on.
///
/// # Errors
///
/// Any step that fails. The message carries the step and the reason.
pub fn run_command(r: &Answers, flake: &str, ver: &mut dyn FnMut(Event)) -> Result<()> {
    let mut r = r.clone();
    let list = steps(&r, flake);
    let total = list.len();

    for (index, step) in list.iter().enumerate() {
        ver(Event::Started {
            index,
            total,
            label: step.label,
        });

        // SEALING IS NOT A WRITE.
        //
        // It runs `systemd-creds` against the TPM and stores what it
        // returns. It comes before `write_of` because the latter would refuse the
        // command with an unknown-path `bail!` — which is the right
        // protection for a new step nobody wired up, and the reason
        // this one has a name of its own.
        if step.command.first().map(String::as_str) == Some(SEAL) {
            let directory = step.command.get(1).context("sealing with no directory")?;
            let ssid = step.command.get(2).context("sealing with no SSID")?;
            ronoctua_network::secret::seal_at(directory, ssid, &r.wifi_password)
                .with_context(|| format!("step `{}`", step.label))?;
            ver(Event::Finished { index });
            continue;
        }

        if let Some(mut e) = write_of(step, &r)? {
            // The hash can only be computed now: it is the only content that
            // depends on an external process.
            if e.path == PASSWORD_HASH_PATH {
                e.content = password_hash(&r.password)?;
            }
            // The `host.nix` is rebuilt here, and not in the `write_of` from
            // before the loop, because by this point the `resume_offset` has already been
            // computed — and without it there is no hibernation.
            if e.path == HOST_NIX_PATH || e.path == HOST_NIX_WORK_PATH {
                e.content = host_nix(&r);
            }
            write_out(&e).with_context(|| format!("step `{}`", step.label))?;
        } else {
            run(&step.command, ver).with_context(|| format!("step `{}`", step.label))?;
        }

        // RIGHT AFTER `disko`, which is what creates the swapfile, and before
        // the `host.nix` that carries the offset is written.
        //
        // The file is `disko`'s own — `nix/disko/default.nix` declares
        // `swap.swapfile` under the `@swap` subvolume, and that is the path that
        // reaches `swapDevices`. The installer used to create a SECOND file
        // beside it and measure that one, which is an offset into a file the
        // kernel never resumes from.
        if step.command.first().map(String::as_str) == Some("disko") {
            let output = Command::new("btrfs")
                .args(["inspect-internal", "map-swapfile", "-r", SWAPFILE_PATH])
                .output()
                .context("map the swap file")?;
            r.resume_offset = offset_from(&String::from_utf8_lossy(&output.stdout));
        }

        ver(Event::Finished { index });
    }

    // The passphrase does not survive the installation: from then on it exists in the
    // TPM and in the head of whoever installed, and nowhere else.
    let _ = std::fs::remove_file(LUKS_KEY_PATH);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF (by hanging): stderr is read only after stdout closes, so a
    /// child that writes more than a pipe holds to stderr first never ends.
    #[test]
    fn a_command_that_talks_on_stderr_is_drained_whole() {
        let mut lines = Vec::new();
        run(
            &[
                "sh".into(),
                "-c".into(),
                "i=0; while [ $i -lt 20000 ]; do echo err-line-$i >&2; i=$((i+1)); done; \
                 printf 'bad-\\377\\n'; echo out"
                    .into(),
            ],
            &mut |e| {
                if let Event::Line(l) = e {
                    lines.push(l);
                }
            },
        )
        .unwrap();
        assert_eq!(lines.len(), 20_002, "{:?}", lines.last());
        assert_eq!(lines.last().map(String::as_str), Some("out"));
        assert!(lines[20_000].starts_with("bad-"));
    }
    use crate::model::Question;

    fn answers() -> Answers {
        let mut r = Answers::default();
        r.set(Question::Disk, "/dev/nvme0n1".into());
        r.set(Question::Hostname, "noctua-teste".into());
        r.set(Question::Password, "Zq7-test-password".into());
        r.set(Question::Passphrase, "Kx9-passphrase-de-teste".into());
        r.set(Question::Language, "pt-BR".into());
        r
    }

    #[test]
    fn every_step_either_writes_or_runs() {
        // The failure mode this test catches: someone adds a step in
        // `plan::steps` with the internal `write_bytes` command and forgets the case
        // here. Without this, the step becomes a silent no-op — the file does not
        // appear and the installation carries on as if it had.
        let r = answers();
        for p in steps(&r, "/mnt/etc/nixos") {
            write_of(&p, &r).unwrap_or_else(|e| panic!("step `{}`: {e}", p.label));
        }
    }

    #[test]
    fn the_passphrase_goes_with_no_line_break() {
        // `cryptsetup` uses the whole file as the key. A trailing `\n`
        // becomes part of the passphrase, and the person never unlocks again by typing
        // what they chose. There is no error, no warning: it just does not open.
        let r = answers();
        let p = steps(&r, "f");
        let key = p
            .iter()
            .find(|p| p.command.get(1).map(String::as_str) == Some(LUKS_KEY_PATH))
            .expect("the plan has to write the LUKS key");
        let e = write_of(key, &r).unwrap().unwrap();
        assert_eq!(e.content, r.passphrase);
        assert!(
            !e.content.ends_with('\n'),
            "the key came out with a line break"
        );
    }

    #[test]
    fn both_secrets_are_marked_as_secret() {
        let r = answers();
        for p in steps(&r, "f") {
            let Some(e) = write_of(&p, &r).unwrap() else {
                continue;
            };
            // BOTH `host.nix` files are public: the working one and the target's
            // have the same content, and it carries no secret (section 9).
            let expected = if e.path == HOST_NIX_PATH || e.path == HOST_NIX_WORK_PATH {
                Secrecy::Public
            } else {
                Secrecy::Secret
            };
            assert_eq!(e.secrecy, expected, "{}", e.path);
        }
    }

    #[test]
    fn the_host_nix_carries_no_secret() {
        // It goes into backups: it is the only installer file that survives
        // and can be copied. Section 9 admits neither password nor passphrase there.
        let r = answers();
        let s = host_nix(&r);
        assert!(
            !s.contains(&r.password),
            "the password leaked into host.nix"
        );
        assert!(
            !s.contains(&r.passphrase),
            "the passphrase leaked into host.nix"
        );
    }

    #[test]
    fn no_secret_goes_through_argv() {
        // `/proc/<pid>/cmdline` is readable by any process. If the password
        // is in an argument, it is visible for as long as the command runs — and
        // `nixos-install` runs for a long time.
        let r = answers();
        for p in steps(&r, "/mnt/etc/nixos") {
            for arg in &p.command {
                assert!(
                    !arg.contains(&r.password),
                    "a password in argv: {:?}",
                    p.command
                );
                assert!(
                    !arg.contains(&r.passphrase),
                    "the passphrase reached argv: {:?}",
                    p.command
                );
            }
        }
    }

    #[test]
    fn the_offset_comes_from_btrfs_output() {
        assert_eq!(offset_from("533760\n"), Some(533_760));
        // With a header, which is the other form `btrfs` has printed.
        assert_eq!(offset_from("resume_offset 533760\n"), Some(533_760));
        assert_eq!(offset_from(""), None);
        assert_eq!(offset_from("there is no number here"), None);
    }

    #[test]
    fn the_key_is_written_before_the_disk_is_touched() {
        let r = answers();
        let p = steps(&r, "f");
        let key = p
            .iter()
            .position(|p| p.command.get(1).map(String::as_str) == Some(LUKS_KEY_PATH))
            .expect("the key's step is missing");
        let first_destructive = p
            .iter()
            .position(|p| p.destructive)
            .expect("some step has to be destructive");
        assert!(
            key < first_destructive,
            "the key is written after the disk has already been touched"
        );
    }

    /// FAILS IF: a file is born with a mode it should not have for an
    /// instant, a second write to the same path fails or truncates in place,
    /// or a file the person owns is left root's.
    #[test]
    fn a_file_is_born_with_its_mode_and_given_to_its_owner() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let dir =
            std::env::temp_dir().join(format!("ronoctua-installer-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let secret = Write {
            path: dir.join("key").to_string_lossy().into_owned(),
            content: "k".into(),
            secrecy: Secrecy::Secret,
            owner: None,
        };
        write_out(&secret).unwrap();
        write_out(&secret).unwrap();
        let meta = std::fs::metadata(dir.join("key")).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(dir.join("key")).unwrap(), "k");

        // OWNED: this test's own user, from a passwd written here.
        let me = std::fs::metadata(&dir).unwrap();
        let passwd = dir.join("passwd");
        std::fs::write(
            &passwd,
            format!(
                "root:x:0:0::/root:/bin/sh\nsomeone:x:{}:{}::/home/someone:/bin/sh\n",
                me.uid(),
                me.gid()
            ),
        )
        .unwrap();
        let theirs = Write {
            path: dir
                .join("home/someone/.config/ronoctua/belongings.json")
                .to_string_lossy()
                .into_owned(),
            content: "{}".into(),
            secrecy: Secrecy::Public,
            owner: Some("someone".into()),
        };
        write_out_with(&theirs, &passwd).unwrap();
        let file = std::fs::metadata(&theirs.path).unwrap();
        assert_eq!(file.permissions().mode() & 0o777, 0o644);
        assert_eq!((file.uid(), file.gid()), (me.uid(), me.gid()));
        let made = std::fs::metadata(dir.join("home/someone/.config")).unwrap();
        assert_eq!(made.uid(), me.uid());
        // A USER THE TARGET DOES NOT HAVE is a failure, never root's file.
        let nobody = Write {
            owner: Some("nobody-here".into()),
            ..theirs
        };
        assert!(write_out_with(&nobody, &passwd).is_err());
        assert_eq!(ids_of("a:x:1:2:\nb:x:3:4::", "b"), Some((3, 4)));
        assert_eq!(ids_of("a:x:1:2:", "c"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_belongings_file_is_the_address_alone_and_the_persons() {
        let mut r = answers();
        r.belongings = "git@example.org:someone/belongings.git".into();
        let step = steps(&r, "f")
            .into_iter()
            .find(|s| s.command.get(1) == Some(&belongings_path()))
            .unwrap();
        let e = write_of(&step, &r).unwrap().unwrap();
        assert_eq!(e.secrecy, Secrecy::Public);
        assert_eq!(e.owner.as_deref(), Some(target_user()));
        let kept: ronoctua_belongings::prefs::Belongings = serde_json_like(&e.content);
        assert_eq!(kept.repository, r.belongings);
        assert!(!e.content.contains(&r.password) && !e.content.contains(&r.passphrase));
    }

    /// The file read back the way the view reads it.
    fn serde_json_like(text: &str) -> ronoctua_belongings::prefs::Belongings {
        ronoctua_belongings::prefs::Belongings::from_file(text).expect("the file parses")
    }

    #[test]
    fn the_password_hash_is_written_before_nixos_install() {
        // With `mutableUsers = false`, the activator reads the `hashedPasswordFile`
        // during the installation. Afterwards would be too late: the user would be born with no
        // valid password and only at the first login would anyone find out.
        let r = answers();
        let p = steps(&r, "f");
        let hash = p
            .iter()
            .position(|p| p.command.get(1).map(String::as_str) == Some(PASSWORD_HASH_PATH))
            .expect("the hash's step is missing");
        let install_it = p
            .iter()
            .position(|p| p.command.first().map(String::as_str) == Some("nixos-install"))
            .expect("nixos-install is missing");
        assert!(
            hash < install_it,
            "the hash is written after the installation"
        );
    }
}
