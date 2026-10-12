//! The programs the root helper runs, by store path.
//!
//! sudo resets the environment and a unit's `PATH` is minimal, so a name
//! would fail far from the cause. Every path is baked by `nix/rust.nix`
//! (`generated`, read with `option_env!`); a build outside Nix has none, and
//! the helper then refuses with [`crate::Why::Unstartable`] instead of
//! guessing a program by name.

use std::path::{Path, PathBuf};

use crate::{Refusal, Why, refused};

/// Where each program is — or `None` when this build was not given it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tools {
    /// sudo's `visudo`, which checks the window's line before it is live.
    pub visudo: Option<PathBuf>,
    /// nftables' `nft`.
    pub nft: Option<PathBuf>,
    /// `systemctl`, to stop the expiry timers and start the file manager's
    /// unit.
    pub systemctl: Option<PathBuf>,
    /// `systemd-run`, to arm the window's expiry timer.
    pub systemd_run: Option<PathBuf>,
}

impl Tools {
    /// The paths this build was given.
    #[must_use]
    pub fn baked() -> Self {
        let of = |path: Option<&str>| path.filter(|p| !p.is_empty()).map(PathBuf::from);
        Self {
            visudo: of(option_env!("RONOCTUA_VISUDO")),
            nft: of(option_env!("RONOCTUA_NFT")),
            systemctl: of(option_env!("RONOCTUA_SYSTEMCTL")),
            systemd_run: of(option_env!("RONOCTUA_SYSTEMD_RUN")),
        }
    }
}

/// The program, or the refusal that says it was not given.
///
/// # Errors
///
/// [`Why::Unstartable`] when it is `None`.
pub fn given(path: Option<&PathBuf>) -> Result<&Path, Refusal> {
    path.map(PathBuf::as_path).ok_or(refused(Why::Unstartable))
}

/// Runs `program` with `args` and an empty environment (`LC_ALL=C`, so what
/// it says is the English the log reader expects), feeding it `stdin`.
///
/// What it printed on failure goes to this process's standard error with
/// the journal's `err` priority prefix: from a unit it lands in the journal
/// where the brain looks, through the door it is read by nobody.
///
/// # Errors
///
/// [`Why::Unstartable`] when it cannot be started, [`Why::Failed`] when it
/// ends with a failure.
pub(crate) fn run(
    program: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<std::process::Output, Refusal> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn(&mut command).map_err(|e| {
        eprintln!("<3>could not start {}: {e}", program.display());
        refused(Why::Unstartable)
    })?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // A program that exits before reading all of it is judged by its
        // status below, not by the broken pipe.
        let _ = pipe.write_all(bytes);
    }
    let output = child.wait_with_output().map_err(|e| {
        eprintln!("<3>lost {}: {e}", program.display());
        refused(Why::Failed)
    })?;
    if output.status.success() {
        return Ok(output);
    }
    let said = String::from_utf8_lossy(&output.stderr);
    eprintln!(
        "<3>{} {} failed ({}): {}",
        program.display(),
        args.join(" "),
        output.status,
        said.trim()
    );
    Err(refused(Why::Failed))
}

/// Starts `command`, again when the program was busy being written.
///
/// `ETXTBSY` is a race between threads, not a property of the program: a
/// file another thread just wrote may still be open in a child forked in
/// between, until that child's `exec`. Tests write their stand-ins and run
/// them in parallel, and a machine may write a store path while a unit
/// starts; a few tries, milliseconds apart, outlast it.
///
/// The tries cover what this process runs, and not what its child runs in
/// turn (`setsid` running sudo): so in the tests no child is forked while a
/// stand-in is being written ([`FORKS`]).
pub(crate) fn spawn(command: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    #[cfg(test)]
    let _no_stand_in_open = FORKS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut tries = 0;
    loop {
        match command.spawn() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            spawned => return spawned,
        }
    }
}

/// The tests' stand-ins are written alone (the write lock) and children are
/// forked together (the read lock), so no child ever holds a stand-in open
/// for writing — a program it runs in turn would find it busy.
#[cfg(test)]
pub(crate) static FORKS: std::sync::RwLock<()> = std::sync::RwLock::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn a_program_is_run_with_nothing_of_ours_in_its_environment() {
        let dir = Scratch::new("tools");
        let out = dir.0.join("out");
        let program = dir.program(
            "fake",
            &format!(
                "#!/bin/sh\nexport -p > {out}\nIFS= read -r line\nprintf '%s|%s\\n' \"$1\" \"$line\"\n",
                out = out.display()
            ),
        );
        let said = run(&program, &["one"], Some(b"fed\n")).unwrap();
        assert_eq!(String::from_utf8_lossy(&said.stdout), "one|fed\n");
        let env = std::fs::read_to_string(&out).unwrap();
        assert!(env.contains("LC_ALL"), "{env}");
        assert!(!env.contains("HOME="), "the environment was cleared: {env}");
        let failing = dir.program("failing", "#!/bin/sh\nexit 3\n");
        assert_eq!(run(&failing, &[], None).unwrap_err().why, Why::Failed);
        assert_eq!(
            run(&dir.0.join("absent"), &[], None).unwrap_err().why,
            Why::Unstartable
        );
        assert_eq!(given(None).unwrap_err().why, Why::Unstartable);
    }
}
