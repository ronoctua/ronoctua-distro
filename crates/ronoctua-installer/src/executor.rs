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
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result, bail};

use crate::model::Answers;
use crate::plan::{
    ATTRIBUTE, BINARY_CACHE_NETRC, BINARY_CACHE_OPTIONS, BINARY_CACHE_TOKEN, Capture,
    HOST_NIX_PATH, HOST_NIX_WORK_PATH, INSTALL_ACCESS, LET_HOST, LUKS_KEY_PATH, Origin,
    PASSWORD_HASH_PATH, PERSON_UID, Runner, SCRATCH, SEAL, SWAPFILE_PATH, Scratch, Step,
    TARGET_PASSWD, WIFI_PROFILE_PATH, WORK_DIRECTORY, WRITE, aimed_flake, belongings_path,
    host_nix, person_passphrase, plain_passphrase, steps, target_netrc, target_user, wifi_profile,
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
    /// The step had nothing to do (what it waits on is not there) and did
    /// not run.
    Skipped {
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
    /// Whether the folders the write makes are `0700` — a runtime folder
    /// holding a passphrase — rather than `0755`.
    pub private: bool,
}

/// The file a step writes, if it is a writing step.
///
/// A **pure** function, and it is what the tests exercise: the difference between
/// writing the passphrase in 0600 and in 0644 shows up on no screen, only
/// here. Two contents need a program or a file and are filled in by
/// [`run_command`]: the password's hash, and the aimed flake.
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
    let root_public = |path: String, content: String| Write {
        path,
        content,
        secrecy: Secrecy::Public,
        owner: None,
        private: false,
    };
    let e = match path.as_str() {
        LUKS_KEY_PATH => Write {
            path,
            // No trailing `\n`: `cryptsetup` treats the whole file as the
            // key, byte by byte. A line break becomes part of the
            // passphrase, and the person never unlocks again by typing what
            // they chose — a silent, irreversible failure.
            content: r.passphrase.clone(),
            secrecy: Secrecy::Secret,
            owner: None,
            private: false,
        },
        PASSWORD_HASH_PATH => Write {
            path,
            // Filled in by `run_command`, which is what has `mkpasswd`. Here only
            // the format: one line, with a break, as shadow expects.
            content: String::new(),
            secrecy: Secrecy::Secret,
            owner: None,
            private: false,
        },
        // THE NETWORK PROFILE. Secret because NetworkManager requires
        // `0600` anyway: a more permissive profile is REFUSED by it.
        WIFI_PROFILE_PATH => Write {
            path,
            content: wifi_profile(r),
            secrecy: Secrecy::Secret,
            owner: None,
            private: false,
        },
        // BOTH: the working directory's (which `disko` reads, before there
        // is a disk) and the target's. The content is the same; what changes between
        // them is the `resume_offset`, which only exists after the swap.
        HOST_NIX_PATH | HOST_NIX_WORK_PATH => root_public(path, host_nix(r)),
        // THE AIMED FLAKE: the template's text, aimed by `run_command` (it
        // reads the template and asks `nix`).
        aimed if aimed == aimed_flake() => root_public(path, String::new()),
        // THE BELONGINGS' SETTINGS, in the person's home and THEIRS: the view
        // writes this file as them, and a root-owned one would refuse every
        // change. Public: an address and how it is reached, never a
        // credential.
        belongings if belongings == belongings_path() => {
            let mut prefs = ronoctua_belongings::prefs::Belongings::with_repository(&r.belongings);
            prefs.auth = crate::access::auth_of(&r.belongings);
            Write {
                path,
                content: prefs.to_file(),
                secrecy: Secrecy::Public,
                owner: Some(target_user().to_owned()),
                private: false,
            }
        }
        // THE PASSPHRASE THE RESTORE READS, in the person's runtime folder:
        // theirs (the restore runs as them), `0600` in a `0700` folder, and
        // deleted by the restore however it ends — and by the installer's
        // last step anyway. No line break: the belongings read the file
        // whole, as `cryptsetup` does.
        passphrase if passphrase == person_passphrase() => Write {
            path,
            content: r.belongings_passphrase.clone(),
            secrecy: Secrecy::Secret,
            owner: Some(target_user().to_owned()),
            private: true,
        },
        // THE PASSPHRASE THE COPY OF THE PLAIN PART READS, root's (the copy
        // runs as root, before the person exists), `0600` in a `0700`
        // folder, deleted by the copy however it ends — and by the
        // installer's end anyway.
        passphrase if passphrase == plain_passphrase() => Write {
            path,
            content: r.belongings_passphrase.clone(),
            secrecy: Secrecy::Secret,
            owner: None,
            private: true,
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
    let ids = match &e.owner {
        Some(user) => Some(ids_in(passwd, user)?),
        None => None,
    };
    let p = Path::new(&e.path);
    if let Some(parent) = p.parent() {
        // THE FOLDERS THIS WRITE MAKES are the owner's too: a
        // `~/.config/ronoctua` born root's would refuse the person's own
        // preferences.
        make_folders(parent, ids, if e.private { 0o700 } else { 0o755 })?;
    }
    let mode = if e.secrecy == Secrecy::Secret {
        0o600
    } else {
        0o644
    };
    write_whole(p, e.content.as_bytes(), mode, ids)
}

/// The uid and gid `user` has in the `/etc/passwd` at `passwd`.
fn ids_in(passwd: &Path, user: &str) -> Result<(u32, u32)> {
    let text =
        std::fs::read_to_string(passwd).with_context(|| format!("read {}", passwd.display()))?;
    ids_of(&text, user).with_context(|| format!("the user `{user}` is not in {}", passwd.display()))
}

/// Makes the folders of `dir` that are missing, each with `mode` and given
/// to `ids` when there are some.
fn make_folders(dir: &Path, ids: Option<(u32, u32)>, mode: u32) -> Result<()> {
    let mut missing: Vec<&Path> = dir.ancestors().take_while(|a| !a.exists()).collect();
    missing.reverse();
    for d in missing {
        std::fs::DirBuilder::new()
            .mode(mode)
            .create(d)
            .with_context(|| format!("create {}", d.display()))?;
        // The mode again: `mkdir` is masked by the umask, and a private
        // folder born `0755` under a `022` umask would not be private.
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(mode))
            .with_context(|| format!("give {} its mode", d.display()))?;
        if let Some((uid, gid)) = ids {
            std::os::unix::fs::chown(d, Some(uid), Some(gid))
                .with_context(|| format!("give {} to its owner", d.display()))?;
        }
    }
    Ok(())
}

/// Writes `bytes` to `p` whole, born `mode` and given to `ids`, through a
/// draft beside it renamed over.
fn write_whole(p: &Path, bytes: &[u8], mode: u32, ids: Option<(u32, u32)>) -> Result<()> {
    let name = p
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let draft = p.with_file_name(format!(".{name}.install-draft"));
    let _ = std::fs::remove_file(&draft);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&draft)
        .with_context(|| format!("create {}", draft.display()))?;
    f.write_all(bytes)
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

/// A child of the installer: `program`, with NONE of the answers'
/// variables. The installer sheds them from its own environment before it
/// does anything (`ronoctua-install` starts itself again without them), so
/// this is the second wall, not the first: every child the installer starts
/// itself is made here, `mkpasswd` and `btrfs` and `lsblk` included.
#[must_use]
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(program);
    for name in crate::model::answer_variables() {
        c.env_remove(name);
    }
    c
}

/// What a command runs as and with: a step's runner (the person's ids), its
/// variables, and NONE of the answers' variables — in the terminal-less mode
/// the passwords came in through the environment, and every child would
/// otherwise inherit them (`/proc/<pid>/environ`, for as long as
/// `nixos-install` runs).
fn command_of(step: &Step, args: &[String], person: Option<(u32, u32)>) -> Result<Command> {
    let (prog, rest) = args.split_first().context("an empty command")?;
    let mut c = match (step.runs_as, person) {
        (Runner::Root, _) => {
            let mut c = command(prog);
            c.args(rest);
            c
        }
        (Runner::Person, Some((uid, gid))) => {
            let mut c = command("setpriv");
            c.arg(format!("--reuid={uid}"))
                .arg(format!("--regid={gid}"))
                .arg("--clear-groups")
                .arg("--")
                .arg(prog)
                .args(rest);
            c
        }
        (Runner::Person, None) => bail!("a step of the person's before the person exists"),
    };
    for (name, value) in &step.env {
        c.env(name, value);
    }
    Ok(c)
}

/// Runs a command and passes each line of output along.
///
/// ONE PIPE FOR BOTH STREAMS, read to the end. Two pipes read one after the
/// other deadlock: `nixos-install` writes its build log to stderr, and once
/// that pipe's buffer (64 KiB) is full the child blocks on it while this end
/// is still waiting for stdout to close — an install that stops forever with
/// the processor idle. Joined, the lines also arrive in the order they were
/// written.
fn run(mut command: Command, ver: &mut dyn FnMut(Event)) -> Result<()> {
    let prog = command.get_program().to_string_lossy().into_owned();
    let (output, writer) = std::io::pipe().context("a pipe for the output")?;
    // The `Command` is dropped right after the spawn, and with it this
    // process's write ends: the read below ends when the child's do.
    let mut child = command
        .stdout(writer.try_clone().context("a pipe for the output")?)
        .stderr(writer)
        .stdin(Stdio::null())
        .spawn()
        .with_context(|| format!("could not run `{prog}`"))?;
    drop(command);

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

/// Runs a command whose standard output is the ANSWER: kept whole and given
/// back; its standard error passed along as lines. Both read to the end in
/// one go (`output`), so neither pipe fills while the other is waited on.
fn answer(mut command: Command, ver: &mut dyn FnMut(Event)) -> Result<String> {
    let prog = command.get_program().to_string_lossy().into_owned();
    let out = command
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("could not run `{prog}`"))?;
    for line in String::from_utf8_lossy(&out.stderr).lines() {
        ver(Event::Line(line.to_owned()));
    }
    if !out.status.success() {
        bail!("`{prog}` exited with {}", out.status);
    }
    String::from_utf8(out.stdout)
        .with_context(|| format!("`{prog}` answered something that is not text"))
}

/// The one system the machine's flake builds, out of `nix eval --json
/// …#nixosConfigurations --apply builtins.attrNames`. Pure.
///
/// # Errors
///
/// The answer is not a list of names, or it names none or several: the
/// flake is not a machine's, and installing a guess would be worse.
pub fn attribute_of(answer: &str) -> Result<String> {
    let names: Vec<String> = serde_json::from_str(answer.trim())
        .with_context(|| format!("the machine's flake answered `{}`", answer.trim()))?;
    match names.as_slice() {
        [one] if !one.is_empty() => Ok(one.clone()),
        _ => bail!(
            "the machine's flake builds {} systems ({names:?}); a machine builds exactly one",
            names.len()
        ),
    }
}

/// The command with the system's name and the person's uid where the plan
/// marked them; `person` is asked only when a mark needs it.
fn filled(
    command: &[String],
    attribute: Option<&str>,
    person: &dyn Fn() -> Result<(u32, u32)>,
) -> Result<Vec<String>> {
    command
        .iter()
        .map(|a| {
            if a.contains(ATTRIBUTE) {
                let name = attribute.context("the system's name was needed before it was asked")?;
                Ok(a.replace(ATTRIBUTE, name))
            } else if a == PERSON_UID {
                Ok(person()?.0.to_string())
            } else {
                Ok(a.clone())
            }
        })
        .collect()
}

/// The person's binary cache, as the working flake's validated
/// `person/options.json` names it — `None` with no file, no cache, or a file
/// this installer cannot read (nothing is guessed).
fn cache_in(work: &Path) -> Option<ronoctua_person::options::BinaryCache> {
    let at = work
        .join(ronoctua_person::names::MACHINE_PERSON)
        .join(ronoctua_person::names::OPTIONS);
    let bytes = std::fs::read(at).ok()?;
    ronoctua_person::parse::<ronoctua_person::options::Options>(&bytes)
        .ok()?
        .binary_cache
}

/// What `nixos-install` is told of the person's binary cache: the cache as
/// an extra substituter (BEFORE nothing it replaces: `extra-`), its key as
/// an extra trusted one, and — for a private cache whose token was read —
/// the installer's netrc. Nothing with no cache. Pure.
#[must_use]
pub fn cache_options(
    cache: Option<&ronoctua_person::options::BinaryCache>,
    netrc: Option<&str>,
) -> Vec<String> {
    let Some(cache) = cache else {
        return Vec::new();
    };
    let mut out = vec![
        "--option".to_owned(),
        "extra-substituters".to_owned(),
        cache.substituter(),
        "--option".to_owned(),
        "extra-trusted-public-keys".to_owned(),
        cache.public_key.clone(),
    ];
    if let Some(netrc) = netrc.filter(|_| cache.is_private()) {
        out.extend([
            "--option".to_owned(),
            "netrc-file".to_owned(),
            netrc.to_owned(),
        ]);
    }
    out
}

/// The command with the person's binary cache where the plan marked it
/// ([`BINARY_CACHE_OPTIONS`]): read from the working flake, and the
/// installer's netrc when the token step made one.
fn with_cache(args: Vec<String>) -> Vec<String> {
    with_cache_in(
        args,
        Path::new(WORK_DIRECTORY),
        &Scratch::installer().binary_cache_netrc(),
    )
}

/// [`with_cache`], over the working flake `work` and the netrc at `netrc`
/// (when it is a file) — a test's.
fn with_cache_in(args: Vec<String>, work: &Path, netrc: &str) -> Vec<String> {
    if !args.iter().any(|a| a == BINARY_CACHE_OPTIONS) {
        return args;
    }
    let cache = cache_in(work);
    let netrc = Path::new(netrc).is_file().then_some(netrc);
    let options = cache_options(cache.as_ref(), netrc);
    args.into_iter()
        .flat_map(|a| {
            if a == BINARY_CACHE_OPTIONS {
                options.clone()
            } else {
                vec![a]
            }
        })
        .collect()
}

/// `binary-cache-token`: for a PRIVATE cache, the push token read out of the
/// sealed part with the passphrase (`ronoctua-belongings take`, as root in
/// the scratch home, the passphrase in a `0600` file of a folder of its
/// own) and made into the installer's netrc, `0600`; the token's file and
/// the passphrase's go at once. A token the repository does not keep, or a
/// `take` that refused, is said and is no failure: the installation goes on
/// without the cache, as the ISO's seed lets it.
fn binary_cache_token(
    scratch: &Scratch,
    cache: &ronoctua_person::options::BinaryCache,
    passphrase: &str,
    belongings: &str,
    ver: &mut dyn FnMut(Event),
) -> Result<()> {
    let folder = scratch.binary_cache();
    let runtime = scratch.binary_cache_runtime();
    make_folders(Path::new(&folder), None, 0o700)?;
    make_folders(Path::new(&runtime), None, 0o700)?;
    let token_at = scratch.binary_cache_token();
    let _ = std::fs::remove_file(&token_at);
    write_whole(
        &Path::new(&runtime).join(ronoctua_belongings::places::PASSPHRASE_FILE),
        passphrase.as_bytes(),
        0o600,
        None,
    )?;
    let home = scratch.home();
    let mut take = command(belongings);
    take.args([
        "take",
        "--home",
        &home,
        "--runtime",
        &runtime,
        "--path",
        &ronoctua_cachix::token::logical(),
        "--out",
        &token_at,
        "--json",
    ]);
    for (name, value) in crate::plan::home_env_of(&home) {
        take.env(name, value);
    }
    let out = take
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("could not run `{belongings}`"));
    let _ = std::fs::remove_dir_all(&runtime);
    let out = out?;
    for line in String::from_utf8_lossy(&out.stderr).lines() {
        ver(Event::Line(line.to_owned()));
    }
    let said: serde_json::Value =
        serde_json::from_slice(out.stdout.trim_ascii()).unwrap_or(serde_json::Value::Null);
    if said.get("taken").and_then(serde_json::Value::as_bool) != Some(true) {
        let _ = std::fs::remove_file(&token_at);
        let why = said
            .get("refused")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("not-kept");
        ver(Event::Line(format!(
            "{}: {why}",
            ronoctua_cachix::token::logical()
        )));
        return Ok(());
    }
    let read = std::fs::read_to_string(&token_at);
    let _ = std::fs::remove_file(&token_at);
    let token = ronoctua_consent::keys::Secret::new(read.context("read the token")?.trim());
    let line = ronoctua_cachix::netrc::text(&cache.name, &token)
        .map_err(|why| anyhow::anyhow!("the netrc of {}: {why}", cache.host()))?;
    write_whole(
        Path::new(&scratch.binary_cache_netrc()),
        line.as_bytes(),
        0o600,
        None,
    )?;
    ver(Event::Line(scratch.binary_cache_netrc()));
    Ok(())
}

/// `binary-cache-netrc`: the installer's netrc to the machine's root netrc
/// at `to` — its folder `0755`, the file `0600`, root's — and the
/// installer's copy gone.
fn install_netrc(from: &Path, to: &Path) -> Result<()> {
    let bytes = zeroize::Zeroizing::new(
        std::fs::read(from).with_context(|| format!("read {}", from.display()))?,
    );
    if let Some(parent) = to.parent() {
        make_folders(parent, None, 0o755)?;
    }
    write_whole(to, &bytes, 0o600, None)?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

/// The password's hash, through `mkpasswd`.
///
/// The password goes in through `stdin`, never through `argv`.
fn password_hash(password: &str) -> Result<String> {
    let mut child = command("mkpasswd")
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

/// The aimed flake: the template's text, its distro input aimed at the
/// distro's tree.
fn aimed_text(origin: &Origin) -> Result<String> {
    let template = std::fs::read_to_string(origin.template_flake())
        .with_context(|| format!("read {}", origin.template_flake()))?;
    // THE TREE BY ITS STORE PATH: the distro's folder on the ISO is a link,
    // and a `path:` input through a link would be the link.
    let tree = std::fs::canonicalize(&origin.distro)
        .with_context(|| format!("resolve {}", origin.distro))?;
    let url = crate::reach::release_url(Path::new(&origin.nix), &origin.template_flake())
        .map_err(anyhow::Error::msg)?;
    crate::release::aimed(&template, &url, &tree.to_string_lossy()).with_context(|| {
        format!(
            "the template does not name `{url}` exactly once; it changed under this installer, \
             and a flake aimed by guess would install the wrong thing"
        )
    })
}

/// `install-access <from> <to>`: the access files the scratch home holds,
/// given to the person in their own access folder (`0700`), each `0600`
/// (the public half `0644`) and theirs. The files written, by path.
fn install_access(from: &str, to: &str, ids: (u32, u32)) -> Result<Vec<String>> {
    use ronoctua_belongings::places::{DEPLOY_KEY, KNOWN_HOSTS, TOKEN_FILE};
    let to = Path::new(to);
    if let Some(parent) = to.parent() {
        make_folders(parent, Some(ids), 0o755)?;
    }
    make_folders(to, Some(ids), 0o700)?;
    let mut copied = Vec::new();
    let public = format!("{DEPLOY_KEY}.pub");
    for name in [DEPLOY_KEY, public.as_str(), KNOWN_HOSTS, TOKEN_FILE] {
        let source = Path::new(from).join(name);
        let Ok(bytes) = std::fs::read(&source) else {
            continue;
        };
        let mode = if name == public { 0o644 } else { 0o600 };
        let into = to.join(name);
        write_whole(&into, &bytes, mode, Some(ids))?;
        copied.push(into.to_string_lossy().into_owned());
    }
    if copied.is_empty() {
        bail!("{from} holds no access to give the person");
    }
    Ok(copied)
}

/// `let-host <config>`: the repository's host let in the `consent.json`
/// under `config` (the person's configuration root), which stays theirs.
fn let_host(config: &str, address: &str, ids: (u32, u32)) -> Result<()> {
    let host = ronoctua_belongings::prefs::host_of(address)
        .context("the repository's address names no host")?;
    let config = Path::new(config);
    if let Some(parent) = config.parent() {
        make_folders(parent, Some(ids), 0o755)?;
    }
    make_folders(config, Some(ids), 0o755)?;
    ronoctua_consent::let_hosts_under(config, &[host.as_str()], true)
        .map_err(|e| anyhow::anyhow!("the consent was not kept: {e:?}"))?;
    let file = config.join(CONSENT_FILE);
    std::os::unix::fs::chown(&file, Some(ids.0), Some(ids.1))
        .with_context(|| format!("give {} to its owner", file.display()))?;
    Ok(())
}

/// The consent's file name, under a configuration root
/// (`ronoctua-consent`'s).
const CONSENT_FILE: &str = "consent.json";

/// Runs the whole installation.
///
/// It stops at the first error, and says which step it stopped on. However
/// it ends, the disk's key and the passphrase's file are deleted; the
/// scratch folder goes when it ends well (when it does not, the access made
/// for this machine stays in RAM for another try).
///
/// # Errors
///
/// Any step that fails. The message carries the step and the reason.
pub fn run_command(r: &Answers, origin: &Origin, ver: &mut dyn FnMut(Event)) -> Result<()> {
    let mut r = r.clone();
    // AN AIMED MACHINE KEEPS ITS TREE: `host.nix` says so (see `Origin`).
    r.aimed = origin.aimed;
    if r.swap.is_none() {
        r.swap = crate::swap::measure(&r.disk);
    }
    let done = run_steps(&mut r, origin, ver);
    // THE TWO SECRETS ON DISK go, however it ended: the disk's passphrase
    // exists only in the person's head, and the belongings' passphrase in
    // their head and their repository's key, nowhere else. And the private
    // cache's token, in the installer's netrc: the machine's root copy is
    // the one that stays.
    let _ = std::fs::remove_file(LUKS_KEY_PATH);
    let _ = std::fs::remove_file(person_passphrase());
    let _ = std::fs::remove_file(plain_passphrase());
    let _ = std::fs::remove_dir_all(Scratch::installer().binary_cache());
    if done.is_ok() {
        let _ = std::fs::remove_dir_all(SCRATCH);
    }
    done
}

/// [`run_command`]'s loop.
fn run_steps(r: &mut Answers, origin: &Origin, ver: &mut dyn FnMut(Event)) -> Result<()> {
    let list = steps(r, origin);
    let total = list.len();
    let mut attribute: Option<String> = None;
    let person = || ids_in(Path::new(TARGET_PASSWD), target_user());

    for (index, step) in list.iter().enumerate() {
        ver(Event::Started {
            index,
            total,
            label: step.label,
        });
        let context = || format!("step `{}`", step.label);

        if let Some(path) = &step.only_if
            && !Path::new(path).exists()
        {
            ver(Event::Skipped { index });
            continue;
        }

        match step.command.first().map(String::as_str) {
            // SEALING IS NOT A WRITE: it runs `systemd-creds` against the
            // TPM and stores what it returns.
            Some(SEAL) => {
                let directory = step.command.get(1).context("sealing with no directory")?;
                let ssid = step.command.get(2).context("sealing with no SSID")?;
                ronoctua_network::secret::seal_at(directory, ssid, &r.wifi_password)
                    .with_context(context)?;
            }
            Some(INSTALL_ACCESS) => {
                let (from, to) = (
                    step.command.get(1).context("an access with no source")?,
                    step.command.get(2).context("an access with no place")?,
                );
                let ids = person().with_context(context)?;
                // WHAT WAS WRITTEN, one path a line — the data, as `cp -v`
                // would print it, and no sentence of the installer's own.
                for written in install_access(from, to, ids).with_context(context)? {
                    ver(Event::Line(written));
                }
            }
            Some(LET_HOST) => {
                let config = step.command.get(1).context("a consent with no place")?;
                let ids = person().with_context(context)?;
                let_host(config, &r.belongings, ids).with_context(context)?;
            }
            // THE PERSON'S BINARY CACHE: its address said, and for a private
            // one the token made into the installer's netrc.
            Some(BINARY_CACHE_TOKEN) => {
                if let Some(cache) = cache_in(Path::new(WORK_DIRECTORY)) {
                    ver(Event::Line(cache.substituter()));
                    if cache.is_private() {
                        binary_cache_token(
                            &Scratch::installer(),
                            &cache,
                            &r.belongings_passphrase,
                            crate::plan::BELONGINGS,
                            ver,
                        )
                        .with_context(context)?;
                    }
                }
            }
            Some(BINARY_CACHE_NETRC) => {
                let from = Scratch::installer().binary_cache_netrc();
                install_netrc(Path::new(&from), Path::new(&target_netrc()))
                    .with_context(context)?;
                ver(Event::Line(target_netrc()));
            }
            Some(WRITE) => {
                let mut e = write_of(step, r)?.context("a write step that writes nothing")?;
                // The hash can only be computed now: it is the only content
                // that depends on an external process.
                if e.path == PASSWORD_HASH_PATH {
                    e.content = password_hash(&r.password)?;
                }
                // The `host.nix` is rebuilt here, because by this point the
                // `resume_offset` may have been measured.
                if e.path == HOST_NIX_PATH || e.path == HOST_NIX_WORK_PATH {
                    e.content = host_nix(r);
                }
                if e.path == aimed_flake() {
                    e.content = aimed_text(origin).with_context(context)?;
                }
                write_out(&e).with_context(context)?;
            }
            _ => {
                let args = with_cache(
                    filled(&step.command, attribute.as_deref(), &person).with_context(context)?,
                );
                let ids = match step.runs_as {
                    Runner::Person => Some(person().with_context(context)?),
                    Runner::Root => None,
                };
                let command = command_of(step, &args, ids)?;
                match step.captures {
                    Some(Capture::Attribute) => {
                        let said = answer(command, ver).with_context(context)?;
                        let name = attribute_of(&said).with_context(context)?;
                        ver(Event::Line(name.clone()));
                        attribute = Some(name);
                    }
                    None => run(command, ver).with_context(context)?,
                }
            }
        }

        // RIGHT AFTER `disko`, which is what creates the swapfile, and before
        // the `host.nix` that carries the offset is written. The file is
        // `disko`'s own — the one `swapDevices` names — and a swap capped
        // below the RAM holds no hibernation image, so it gets no offset.
        if step.command.first().map(String::as_str) == Some("disko")
            && r.swap.is_none_or(|s| s.hibernates)
        {
            let output = command("btrfs")
                .args(["inspect-internal", "map-swapfile", "-r", SWAPFILE_PATH])
                .output()
                .context("map the swap file")?;
            r.resume_offset = offset_from(&String::from_utf8_lossy(&output.stdout));
        }

        ver(Event::Finished { index });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Question;
    use crate::plan::scratch_keys;
    use std::os::unix::fs::MetadataExt as _;

    fn root_step(command: &[&str]) -> Step {
        Step {
            label: "test",
            command: command.iter().map(|&s| s.to_owned()).collect(),
            destructive: false,
            runs_as: Runner::Root,
            env: Vec::new(),
            only_if: None,
            captures: None,
        }
    }

    const CACHE_KEY: &str = "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=";
    const CACHE_TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJqdGkiOiJpbnN0YWxsIn0.c2ln";

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-installer-cache-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn cache(private: bool) -> ronoctua_person::options::BinaryCache {
        ronoctua_person::options::BinaryCache {
            name: "someone".to_owned(),
            public_key: CACHE_KEY.to_owned(),
            visibility: if private {
                ronoctua_person::options::Visibility::Private
            } else {
                ronoctua_person::options::Visibility::Public
            },
        }
    }

    /// FAILS IF: `nixos-install` is told of a cache nobody named, is not
    /// told of the one the options name (its address and its key), reads a
    /// public cache through a netrc, or a private one without the netrc
    /// the token step made — or the mark is left in the command.
    #[test]
    fn nixos_install_is_told_of_the_persons_cache_and_nothing_else() {
        assert!(cache_options(None, Some("/n")).is_empty());
        assert_eq!(
            cache_options(Some(&cache(false)), Some("/n")),
            [
                "--option",
                "extra-substituters",
                "https://someone.cachix.org",
                "--option",
                "extra-trusted-public-keys",
                CACHE_KEY
            ]
        );
        let private = cache_options(Some(&cache(true)), Some("/n"));
        assert_eq!(&private[6..], ["--option", "netrc-file", "/n"]);
        assert_eq!(cache_options(Some(&cache(true)), None).len(), 6);

        let root = scratch_dir("mark");
        let work = root.join("work");
        let person = work.join("person");
        std::fs::create_dir_all(&person).unwrap();
        let args = || vec!["nixos-install".to_owned(), BINARY_CACHE_OPTIONS.to_owned()];
        let netrc = root.join("netrc");
        let netrc_text = netrc.to_string_lossy().into_owned();
        assert_eq!(with_cache_in(args(), &work, &netrc_text), ["nixos-install"]);
        std::fs::write(
            person.join("options.json"),
            format!(
                r#"{{"format": 1, "ssh-signing": null,
                   "binary-cache": {{"name": "someone", "public-key": "{CACHE_KEY}", "visibility": "private"}}}}"#
            ),
        )
        .unwrap();
        std::fs::write(&netrc, "x").unwrap();
        let told = with_cache_in(args(), &work, &netrc_text);
        assert_eq!(told.len(), 10);
        assert!(!told.iter().any(|a| a == BINARY_CACHE_OPTIONS));
        assert_eq!(told.last(), Some(&netrc_text));
        // A FILE THAT DOES NOT READ names no cache: nothing is guessed.
        std::fs::write(person.join("options.json"), "{\"format\": 9}").unwrap();
        assert_eq!(with_cache_in(args(), &work, &netrc_text), ["nixos-install"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the private cache's token is asked of the belongings by
    /// anything but a file — the passphrase in a `0600` file, the token
    /// written to one — or outlives the netrc it became, the netrc is born
    /// readable, or a token the repository does not keep stops the
    /// installation.
    #[test]
    fn the_private_caches_token_becomes_the_installers_netrc_and_nothing_else() {
        let root = scratch_dir("token");
        let scratch = Scratch {
            root: root.join("run").to_string_lossy().into_owned(),
        };
        let stand_in = root.join("belongings");
        let log = root.join("argv");
        std::fs::write(
            &stand_in,
            format!(
                "#!/bin/sh\necho \"$@\" >{log}\nout=\"\"; run=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in --out) out=$2; shift ;; --runtime) run=$2; shift ;; esac; shift; done\n[ \"$(cat \"$run/passphrase\")\" = pass-phrase ] || {{ echo '{{\"refused\":\"no-passphrase\"}}'; exit 1; }}\n[ -n \"$KEEP\" ] || {{ echo '{{\"taken\":false}}'; exit 0; }}\nprintf %s {CACHE_TOKEN} >\"$out\"\necho '{{\"taken\":true}}'\n",
                log = log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&stand_in, std::fs::Permissions::from_mode(0o755)).unwrap();
        let program = stand_in.to_string_lossy().into_owned();
        let mut lines = Vec::new();
        // NOT KEPT: said, and no netrc.
        binary_cache_token(&scratch, &cache(true), "pass-phrase", &program, &mut |e| {
            if let Event::Line(l) = e {
                lines.push(l);
            }
        })
        .unwrap();
        assert!(!Path::new(&scratch.binary_cache_netrc()).exists());
        assert!(lines.iter().any(|l| l.ends_with("not-kept")), "{lines:?}");
        // KEPT: the netrc, private, and nothing else left.
        let wrapped = root.join("keeping");
        std::fs::write(
            &wrapped,
            format!("#!/bin/sh\nKEEP=1 exec {program} \"$@\"\n"),
        )
        .unwrap();
        std::fs::set_permissions(&wrapped, std::fs::Permissions::from_mode(0o755)).unwrap();
        binary_cache_token(
            &scratch,
            &cache(true),
            "pass-phrase",
            &wrapped.to_string_lossy(),
            &mut |_| {},
        )
        .unwrap();
        let netrc = scratch.binary_cache_netrc();
        assert_eq!(
            std::fs::read_to_string(&netrc).unwrap(),
            format!("machine someone.cachix.org password {CACHE_TOKEN}\n")
        );
        assert_eq!(std::fs::metadata(&netrc).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(scratch.binary_cache()).unwrap().mode() & 0o777,
            0o700
        );
        assert!(!Path::new(&scratch.binary_cache_token()).exists());
        assert!(!Path::new(&scratch.binary_cache_runtime()).exists());
        let argv = std::fs::read_to_string(&log).unwrap();
        assert!(argv.contains("take --home"), "{argv}");
        assert!(
            !argv.contains("pass-phrase") && !argv.contains(CACHE_TOKEN),
            "{argv}"
        );
        // AND TO THE TARGET: root's, `0600`, the installer's copy gone.
        let target = root.join("mnt/var/lib/ronoctua-binary-cache/netrc");
        install_netrc(Path::new(&netrc), &target).unwrap();
        assert_eq!(std::fs::metadata(&target).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(target.parent().unwrap()).unwrap().mode() & 0o777,
            0o755
        );
        assert!(!Path::new(&netrc).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF (by hanging): stderr is read only after stdout closes, so a
    /// child that writes more than a pipe holds to stderr first never ends.
    #[test]
    fn a_command_that_talks_on_stderr_is_drained_whole() {
        let mut lines = Vec::new();
        let args: Vec<String> = [
            "sh",
            "-c",
            "i=0; while [ $i -lt 20000 ]; do echo err-line-$i >&2; i=$((i+1)); done; \
             printf 'bad-\\377\\n'; echo out",
        ]
        .iter()
        .map(|&s| s.to_owned())
        .collect();
        let c = command_of(&root_step(&[]), &args, None).unwrap();
        run(c, &mut |e| {
            if let Event::Line(l) = e {
                lines.push(l);
            }
        })
        .unwrap();
        assert_eq!(lines.len(), 20_002, "{:?}", lines.last());
        assert_eq!(lines.last().map(String::as_str), Some("out"));
        assert!(lines[20_000].starts_with("bad-"));
    }

    /// FAILS IF: a child inherits an answer from the environment (the
    /// terminal-less mode's passwords), or loses a variable its step gives.
    #[test]
    fn no_child_inherits_an_answer() {
        let mut step = root_step(&[]);
        step.env = vec![("HOME".into(), "/mnt/home/noctua".into())];
        let args: Vec<String> = ["sh", "-c", "env"].iter().map(|&s| s.to_owned()).collect();
        let c = command_of(&step, &args, None).unwrap();
        let removed: Vec<String> = c
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        for q in Question::ALL_KEYS {
            assert!(removed.contains(&q.variable()), "{q:?} is inherited");
        }
        assert!(
            c.get_envs()
                .any(|(k, v)| k == "HOME" && v == Some("/mnt/home/noctua".as_ref()))
        );
        // AS THE PERSON: `setpriv` with their ids and no supplementary
        // group, and no person without ids.
        let mut theirs = step;
        theirs.runs_as = Runner::Person;
        let c = command_of(&theirs, &args, Some((1000, 100))).unwrap();
        assert_eq!(c.get_program(), "setpriv");
        let given: Vec<String> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            given[..5],
            ["--reuid=1000", "--regid=100", "--clear-groups", "--", "sh"]
        );
        assert!(command_of(&theirs, &args, None).is_err());
    }

    fn answers() -> Answers {
        let mut r = Answers::default();
        r.set(Question::Disk, "/dev/nvme0n1".into());
        r.set(Question::Hostname, "noctua-teste".into());
        r.set(Question::Password, "Zq7-test-password".into());
        r.set(Question::Passphrase, "Kx9-passphrase-de-teste".into());
        r.set(Question::Language, "pt-BR".into());
        r.set(
            Question::BelongingsAddress,
            "git@example.org:someone/belongings.git".into(),
        );
        r.set(
            Question::BelongingsAccess,
            format!("{}/deploy", scratch_keys()),
        );
        r.set(Question::BelongingsPassphrase, "Qw4-belongings".into());
        r
    }

    fn origin(aimed: bool) -> Origin {
        Origin {
            distro: "/etc/ronoctua/distro".into(),
            aimed,
            nix: "/run/current-system/sw/bin/nix".into(),
        }
    }

    #[test]
    fn every_step_either_writes_or_runs() {
        // The failure mode this test catches: someone adds a step in
        // `plan::steps` with the internal `write` command and forgets the
        // case here — a silent no-op.
        let r = answers();
        for aimed in [false, true] {
            for p in steps(&r, &origin(aimed)) {
                write_of(&p, &r).unwrap_or_else(|e| panic!("step `{}`: {e}", p.label));
            }
        }
    }

    #[test]
    fn the_passphrases_go_with_no_line_break() {
        // `cryptsetup` uses the whole file as the key, and the belongings
        // read theirs whole too. A trailing `\n` becomes part of the
        // passphrase — no error, no warning: it just does not open.
        let r = answers();
        let p = steps(&r, &origin(false));
        for (path, want) in [
            (LUKS_KEY_PATH.to_owned(), &r.passphrase),
            (person_passphrase(), &r.belongings_passphrase),
            (plain_passphrase(), &r.belongings_passphrase),
        ] {
            let step = p
                .iter()
                .find(|s| s.command.get(1) == Some(&path))
                .unwrap_or_else(|| panic!("the plan has to write {path}"));
            let e = write_of(step, &r).unwrap().unwrap();
            assert_eq!(&e.content, want);
            assert!(
                !e.content.ends_with('\n'),
                "{path} came out with a line break"
            );
        }
    }

    /// FAILS IF: a secret is written readable, a public file secret, or
    /// the belongings' passphrase not the person's, in a private folder.
    #[test]
    fn every_secret_is_marked_as_secret() {
        let r = answers();
        for p in steps(&r, &origin(true)) {
            let Some(e) = write_of(&p, &r).unwrap() else {
                continue;
            };
            let secret = [
                LUKS_KEY_PATH.to_owned(),
                PASSWORD_HASH_PATH.to_owned(),
                WIFI_PROFILE_PATH.to_owned(),
                person_passphrase(),
                plain_passphrase(),
            ]
            .contains(&e.path);
            assert_eq!(
                e.secrecy,
                if secret {
                    Secrecy::Secret
                } else {
                    Secrecy::Public
                },
                "{}",
                e.path
            );
        }
        let step = steps(&r, &origin(false))
            .into_iter()
            .find(|s| s.command.get(1) == Some(&person_passphrase()))
            .unwrap();
        let e = write_of(&step, &r).unwrap().unwrap();
        assert_eq!(e.owner.as_deref(), Some(target_user()));
        assert!(e.private, "the passphrase's folder would be readable");
        // THE COPY'S PASSPHRASE is root's (the person does not exist yet),
        // private all the same.
        let step = steps(&r, &origin(false))
            .into_iter()
            .find(|s| s.command.get(1) == Some(&plain_passphrase()))
            .unwrap();
        let e = write_of(&step, &r).unwrap().unwrap();
        assert_eq!(e.owner, None);
        assert!(e.private);
    }

    #[test]
    fn the_host_nix_carries_no_secret() {
        let r = answers();
        let s = host_nix(&r);
        for secret in [&r.password, &r.passphrase, &r.belongings_passphrase] {
            assert!(!s.contains(secret.as_str()), "{s}");
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

    /// FAILS IF: the system's name is taken from a flake that builds none
    /// or several, or the mark is left in a command, or filled before it
    /// was asked.
    #[test]
    fn the_system_is_the_one_the_flake_names() {
        assert_eq!(attribute_of("[\"noctua\"]\n").unwrap(), "noctua");
        assert!(attribute_of("[]").is_err());
        assert!(attribute_of("[\"a\",\"b\"]").is_err());
        assert!(attribute_of("error: …").is_err());
        let cmd: Vec<String> = ["disko", &format!("/w#{ATTRIBUTE}")]
            .iter()
            .map(|&s| s.to_owned())
            .collect();
        let nobody = || -> Result<(u32, u32)> { anyhow::bail!("no person yet") };
        assert_eq!(
            filled(&cmd, Some("noctua"), &nobody).unwrap()[1],
            "/w#noctua"
        );
        assert!(filled(&cmd, None, &nobody).is_err());
        assert_eq!(filled(&cmd[..1], None, &nobody).unwrap(), ["disko"]);
    }

    /// FAILS IF: the person's uid is not put where the plan marked it, is
    /// asked of the target before a command needs it, or a command that
    /// needs it goes on without it.
    #[test]
    fn the_persons_uid_is_filled_only_where_marked() {
        let cmd: Vec<String> = ["ronoctua-person", "pin", "--owner", PERSON_UID]
            .iter()
            .map(|&s| s.to_owned())
            .collect();
        let asked = std::cell::Cell::new(0);
        let person = || -> Result<(u32, u32)> {
            asked.set(asked.get() + 1);
            Ok((1000, 100))
        };
        assert_eq!(filled(&cmd, None, &person).unwrap()[3], "1000");
        assert_eq!(asked.get(), 1);
        assert_eq!(filled(&cmd[..2], None, &person).unwrap().len(), 2);
        assert_eq!(asked.get(), 1, "asked of the target with no mark");
        let nobody = || -> Result<(u32, u32)> { anyhow::bail!("no person yet") };
        assert!(filled(&cmd, None, &nobody).is_err());
    }

    #[test]
    fn the_key_is_written_before_the_disk_is_touched() {
        let r = answers();
        let p = steps(&r, &origin(false));
        let key = p
            .iter()
            .position(|p| p.command.get(1).map(String::as_str) == Some(LUKS_KEY_PATH))
            .expect("the key's step is missing");
        let first_destructive = p
            .iter()
            .position(|p| p.destructive)
            .expect("some step has to be destructive");
        assert!(key < first_destructive);
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-installer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// FAILS IF: a file is born with a mode it should not have for an
    /// instant, a second write to the same path fails or truncates in place,
    /// a private folder is born readable, or a file the person owns is left
    /// root's.
    #[test]
    fn a_file_is_born_with_its_mode_and_given_to_its_owner() {
        let dir = scratch("write");
        let secret = Write {
            path: dir.join("key").to_string_lossy().into_owned(),
            content: "k".into(),
            secrecy: Secrecy::Secret,
            owner: None,
            private: false,
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
            private: false,
        };
        write_out_with(&theirs, &passwd).unwrap();
        let file = std::fs::metadata(&theirs.path).unwrap();
        assert_eq!(file.permissions().mode() & 0o777, 0o644);
        assert_eq!((file.uid(), file.gid()), (me.uid(), me.gid()));
        let made = std::fs::metadata(dir.join("home/someone/.config")).unwrap();
        assert_eq!(made.uid(), me.uid());
        assert_eq!(made.permissions().mode() & 0o777, 0o755);
        // A PRIVATE FOLDER is born `0700`, whatever the umask says.
        let hidden = Write {
            path: dir
                .join("run/person/passphrase")
                .to_string_lossy()
                .into_owned(),
            content: "p".into(),
            secrecy: Secrecy::Secret,
            owner: Some("someone".into()),
            private: true,
        };
        write_out_with(&hidden, &passwd).unwrap();
        for folder in ["run", "run/person"] {
            let m = std::fs::metadata(dir.join(folder)).unwrap();
            assert_eq!(m.permissions().mode() & 0o777, 0o700, "{folder}");
        }
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

    /// FAILS IF: the access given to the person is not theirs, its private
    /// half readable by others, its folder open, or a scratch home with no
    /// access passes for one that has.
    #[test]
    fn the_access_goes_to_the_person_private() {
        let dir = scratch("access");
        let me = std::fs::metadata(&dir).unwrap();
        let ids = (me.uid(), me.gid());
        let from = dir.join("scratch/keys");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::write(from.join("deploy"), "PRIVATE").unwrap();
        std::fs::write(from.join("deploy.pub"), "ssh-ed25519 AAAA").unwrap();
        std::fs::write(from.join("known_hosts"), "example.org ssh-ed25519 AAAA").unwrap();
        let to = dir.join("home/.config/ronoctua/belongings-keys");
        let written = install_access(&from.to_string_lossy(), &to.to_string_lossy(), ids).unwrap();
        assert_eq!(written.len(), 3);
        assert!(
            written
                .iter()
                .all(|w| w.starts_with(&*to.to_string_lossy()))
        );
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&to), 0o700);
        assert_eq!(mode(&to.join("deploy")), 0o600);
        assert_eq!(mode(&to.join("known_hosts")), 0o600);
        assert_eq!(mode(&to.join("deploy.pub")), 0o644);
        assert_eq!(
            std::fs::read_to_string(to.join("deploy")).unwrap(),
            "PRIVATE"
        );
        assert!(!to.join("token").exists());
        let empty = dir.join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(
            install_access(&empty.to_string_lossy(), &to.to_string_lossy(), ids).is_err(),
            "a home with no access was given one"
        );
        // AND THE CONSENT: the repository's host let, in the person's file.
        let config = dir.join("home/.config/ronoctua");
        let_host(
            &config.to_string_lossy(),
            "git@Example.org:someone/b.git",
            ids,
        )
        .unwrap();
        assert!(ronoctua_consent::allows_under(&config, "example.org"));
        assert!(!ronoctua_consent::allows_under(&config, "elsewhere.org"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_belongings_file_is_the_address_and_its_way_and_the_persons() {
        let r = answers();
        let step = steps(&r, &origin(false))
            .into_iter()
            .find(|s| s.command.get(1) == Some(&belongings_path()))
            .unwrap();
        let e = write_of(&step, &r).unwrap().unwrap();
        assert_eq!(e.secrecy, Secrecy::Public);
        assert_eq!(e.owner.as_deref(), Some(target_user()));
        let kept = ronoctua_belongings::prefs::Belongings::from_file(&e.content).unwrap();
        assert_eq!(kept.repository, r.belongings);
        assert_eq!(kept.auth, ronoctua_belongings::prefs::Auth::Ssh);
        for secret in [&r.password, &r.passphrase, &r.belongings_passphrase] {
            assert!(!e.content.contains(secret.as_str()));
        }
    }

    #[test]
    fn the_password_hash_is_written_before_nixos_install() {
        // With `mutableUsers = false`, the activator reads the
        // `hashedPasswordFile` during the installation.
        let r = answers();
        let p = steps(&r, &origin(false));
        let hash = p
            .iter()
            .position(|p| p.command.get(1).map(String::as_str) == Some(PASSWORD_HASH_PATH))
            .expect("the hash's step is missing");
        let install_it = p
            .iter()
            .position(|p| p.command.first().map(String::as_str) == Some("nixos-install"))
            .expect("nixos-install is missing");
        assert!(hash < install_it);
    }
}
