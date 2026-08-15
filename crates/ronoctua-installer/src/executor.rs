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
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result, bail};

use crate::model::Answers;
use crate::plan::SEAL;
use crate::plan::{
    HOST_NIX_PATH, HOST_NIX_WORK_PATH, LUKS_KEY_PATH, PASSWORD_HASH_PATH, SWAPFILE_PATH, Step,
    WIFI_PROFILE_PATH, WRITE, host_nix, steps, wifi_profile,
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
        .context("passo de escrita sem caminho")?
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
        },
        PASSWORD_HASH_PATH => Write {
            path,
            // Filled in by `run_command`, which is what has `mkpasswd`. Here only
            // the format: one line, with a break, as shadow expects.
            content: String::new(),
            secrecy: Secrecy::Secret,
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
        },
        // BOTH: the working directory's (which `disko` reads, before there
        // is a disk) and the target's. The content is the same; what changes between
        // them is the `resume_offset`, which only exists after the swap.
        HOST_NIX_PATH | HOST_NIX_WORK_PATH => Write {
            path,
            content: host_nix(r),
            secrecy: Secrecy::Public,
        },
        otherwise => bail!(
            "passo de escrita para `{otherwise}`, que este modulo nao conhece.\n\
             \n\
             Um passo novo em `plan::passos` precisa de um caso aqui —\n\
             senao ele vira um `escrever` que nao escreve nada."
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

/// Writes a file, creating the parent directory.
///
/// Mode 0600 when secret, and **before** writing the content: creating it in
/// 0644 and fixing it afterwards leaves a window in which the secret is readable.
fn write_out(e: &Write) -> Result<()> {
    let p = Path::new(&e.path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("criar {}", parent.display()))?;
    }
    let mut f = std::fs::File::create(p).with_context(|| format!("criar {}", p.display()))?;
    if e.secrecy == Secrecy::Secret {
        f.set_permissions(std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("modo 0600 em {}", p.display()))?;
    }
    f.write_all(e.content.as_bytes())
        .with_context(|| format!("escrever {}", p.display()))?;
    Ok(())
}

/// Runs a command and passes each line of output along.
fn run(command: &[String], ver: &mut dyn FnMut(Event)) -> Result<()> {
    let (prog, args) = command.split_first().context("comando vazio")?;
    let mut child = Command::new(prog)
        .args(args)
        .stdout(Stdio::piped())
        // It joins the two: `nixos-install` speaks through stderr, and an
        // installation that looks stuck for half an hour because the output is
        // on another channel is worse than having no output at all.
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("nao consegui rodar `{prog}`"))?;

    if let Some(output) = child.stdout.take() {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            ver(Event::Line(line));
        }
    }
    if let Some(err) = child.stderr.take() {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            ver(Event::Line(line));
        }
    }
    let st = child.wait().context("esperar o comando")?;
    if !st.success() {
        bail!("`{prog}` saiu com {st}");
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
        .context("nao consegui rodar `mkpasswd`")?;
    child
        .stdin
        .take()
        .context("mkpasswd sem stdin")?
        .write_all(password.as_bytes())
        .context("passar a senha ao mkpasswd")?;
    let output = child.wait_with_output().context("esperar o mkpasswd")?;
    if !output.status.success() {
        bail!("`mkpasswd` saiu com {}", output.status);
    }
    let h = String::from_utf8(output.stdout)
        .context("mkpasswd devolveu algo que nao e UTF-8")?
        .trim()
        .to_owned();
    if h.is_empty() {
        bail!("`mkpasswd` devolveu vazio");
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
            let diretorio = step.command.get(1).context("selagem sem diretorio")?;
            let ssid = step.command.get(2).context("selagem sem SSID")?;
            ronoctua_network::secret::seal_at(diretorio, ssid, &r.wifi_password)
                .with_context(|| format!("passo `{}`", step.label))?;
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
            write_out(&e).with_context(|| format!("passo `{}`", step.label))?;
        } else {
            run(&step.command, ver).with_context(|| format!("passo `{}`", step.label))?;
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
                .context("mapear o arquivo de swap")?;
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
    use crate::model::Question;

    fn answers() -> Answers {
        let mut r = Answers::default();
        r.set(Question::Disk, "/dev/nvme0n1".into());
        r.set(Question::Hostname, "noctua-teste".into());
        r.set(Question::Password, "Zq7-senha-de-teste".into());
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
            write_of(&p, &r).unwrap_or_else(|e| panic!("passo `{}`: {e}", p.label));
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
            .expect("o plano tem de escrever a chave do LUKS");
        let e = write_of(key, &r).unwrap().unwrap();
        assert_eq!(e.content, r.passphrase);
        assert!(!e.content.ends_with('\n'), "a chave saiu com quebra");
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
        assert!(!s.contains(&r.password), "a senha vazou para o host.nix");
        assert!(
            !s.contains(&r.passphrase),
            "a passphrase vazou para o host.nix"
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
                assert!(!arg.contains(&r.password), "senha em argv: {:?}", p.command);
                assert!(
                    !arg.contains(&r.passphrase),
                    "passphrase em argv: {:?}",
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
        assert_eq!(offset_from("nao ha numero aqui"), None);
    }

    #[test]
    fn the_key_is_written_before_the_disk_is_touched() {
        let r = answers();
        let p = steps(&r, "f");
        let key = p
            .iter()
            .position(|p| p.command.get(1).map(String::as_str) == Some(LUKS_KEY_PATH))
            .expect("falta o passo da chave");
        let primeiro_destrutivo = p
            .iter()
            .position(|p| p.destructive)
            .expect("algum passo tem de ser destrutivo");
        assert!(
            key < primeiro_destrutivo,
            "a chave e escrita depois de o disco ja ter sido tocado"
        );
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
            .expect("falta o passo do hash");
        let instalar = p
            .iter()
            .position(|p| p.command.first().map(String::as_str) == Some("nixos-install"))
            .expect("falta o nixos-install");
        assert!(hash < instalar, "o hash e escrito depois da instalacao");
    }
}
