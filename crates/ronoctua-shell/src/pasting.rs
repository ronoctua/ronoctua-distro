//! THE CLIPBOARD, READ WITH A DEADLINE — what either door pastes.
//!
//! # Why a deadline, and why off the loop
//!
//! Because the other end is somebody else's program. `wl-paste` asks the
//! compositor for the selection and the program that copied writes it, when
//! it likes: a slow or hung owner held `wl-paste` open, and `wl-paste` held
//! the window that ran it on its loop thread — frozen, keyboard and paint,
//! until the other program answered. AGENTS.md says nothing blocks the
//! loop's thread; so the reading is a thread's ([`text_then`],
//! [`image_then`]), and the reading itself gives up after [`PATIENCE`] and
//! past a ceiling of bytes, so not even that thread waits for ever.
//!
//! THE FOOTER COMES HERE TOO. It holds the machine's clipboard history
//! (`crate::clipboard`), and it pasted the newest entry of it — but the
//! history never keeps a copy marked secret, so a password copied from a
//! password manager pasted the copy BEFORE it into a masked line, where
//! nobody could see it was the wrong one. What is pasted, in both doors, is
//! what is copied NOW: the live selection, read here.

use anyhow::{Context as _, Result};
use std::io::Read as _;

/// How long the program that owns the selection has to hand it over.
pub const PATIENCE: std::time::Duration = std::time::Duration::from_secs(2);

/// The most TEXT a paste takes. A line on a header or a search is a line; a
/// megabyte of it is somebody's log copied by mistake.
pub const TEXT_CEILING: usize = 1 << 20;

/// The most bytes an IMAGE may be — the icon's own checks come after.
pub const IMAGE_CEILING: usize = 32 << 20;

/// `wl-paste`, by store path — `nix/rust.nix` sets it.
const WL_PASTE: Option<&str> = option_env!("RONOCTUA_WL_PASTE");

/// The clipboard's text, read on a thread of its own, and `done` called with
/// it there. An empty clipboard — or one holding no text — is `Ok("")`.
pub fn text_then(done: impl FnOnce(Result<String>) + Send + 'static) {
    let born = std::thread::Builder::new()
        .name("paste-text".to_owned())
        .spawn(move || done(text()));
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that reads the clipboard");
    }
}

/// The clipboard's image, read on a thread of its own: `Ok(None)` when what
/// is copied is not an image.
pub fn image_then(done: impl FnOnce(Result<Option<Vec<u8>>>) + Send + 'static) {
    let born = std::thread::Builder::new()
        .name("paste-image".to_owned())
        .spawn(move || done(image()));
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that reads the clipboard");
    }
}

/// The clipboard's text, now — BLOCKING, for [`PATIENCE`] at most. Call it
/// from a thread that is not a loop's.
///
/// `--type text` asks `wl-paste` for a TEXT type and no other: without it, an
/// image on the clipboard was read in full and then refused as not UTF-8.
///
/// # Errors
///
/// `wl-paste` was not put in the build, could not run, took too long, handed
/// over more than [`TEXT_CEILING`], or what it handed over is not UTF-8.
pub fn text() -> Result<String> {
    let bytes = read_within(
        wl_paste()?,
        &["--no-newline", "--type", "text"],
        PATIENCE,
        TEXT_CEILING,
    )?;
    let Some(bytes) = bytes else {
        return Ok(String::new());
    };
    String::from_utf8(bytes).context("what was on the clipboard is not UTF-8 text")
}

/// The clipboard's image, now — BLOCKING, for twice [`PATIENCE`] at most:
/// the list of types, then the image in the type the clipboard's own history
/// prefers (`ronoctua_clipboard::image::preferred_type`).
///
/// # Errors
///
/// `wl-paste` is not there, or took too long, or handed over more than
/// [`IMAGE_CEILING`].
pub fn image() -> Result<Option<Vec<u8>>> {
    let program = wl_paste()?;
    let Some(listed) = read_within(program, &["--list-types"], PATIENCE, TEXT_CEILING)? else {
        return Ok(None);
    };
    let kinds: Vec<String> = String::from_utf8_lossy(&listed)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let Some(kind) = ronoctua_clipboard::image::preferred_type(&kinds) else {
        return Ok(None);
    };
    read_within(program, &["--type", kind], PATIENCE, IMAGE_CEILING)
}

/// The program, or why there is none.
fn wl_paste() -> Result<&'static str> {
    WL_PASTE.filter(|p| !p.is_empty()).context(
        "RONOCTUA_WL_PASTE was not set at build time.\n\
         It comes from `nix/rust.nix`; without it there is no way to paste.",
    )
}

/// Runs `program args`, and takes what it writes — within `patience` and
/// `ceiling` bytes, or not at all.
///
/// `Ok(None)` when the program ended in failure, which for `wl-paste` is an
/// empty clipboard or no content of the type asked: a normal answer, not an
/// error — treating it as one filled the log every time somebody pasted
/// having copied nothing.
///
/// THE WAIT IS ON A CHANNEL, the reading on a thread: a child that writes
/// nothing and never exits would otherwise hold `read_to_end` for ever. At
/// the deadline the child is killed, which ends its pipe, which ends the
/// reading thread.
fn read_within(
    program: &str,
    args: &[&str],
    patience: std::time::Duration,
    ceiling: usize,
) -> Result<Option<Vec<u8>>> {
    let started = std::time::Instant::now();
    let mut cmd = std::process::Command::new(program);
    // THE SAME PATH as `wl-copy`'s (`buffer::copy_to_clipboard`): under a
    // systemd unit it is minimal, and these tools spawn by NAME.
    if let Some(p) = option_env!("RONOCTUA_WL_COPY_PATH") {
        cmd.env("PATH", p);
    }
    let mut child = cmd
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("could not run {program}"))?;
    let mut out = child.stdout.take().context("the child has no stdout")?;
    let limit = u64::try_from(ceiling).unwrap_or(u64::MAX).saturating_add(1);
    let (sender, arrived) = std::sync::mpsc::channel();
    let reader = std::thread::Builder::new()
        .name("paste-read".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let read = (&mut out).take(limit).read_to_end(&mut bytes);
            let _ = sender.send(read.map(|_| bytes));
        });
    if let Err(e) = reader {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e).context("could not start the thread that reads the clipboard");
    }
    let bytes = match arrived.recv_timeout(patience.saturating_sub(started.elapsed())) {
        Ok(Ok(bytes)) if bytes.len() <= ceiling => bytes,
        Ok(Ok(_)) => {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("what was copied is over {ceiling} bytes");
        }
        Ok(Err(e)) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e).context("could not read what was copied");
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "{program} gave nothing in {} ms; the program that copied is not answering",
                patience.as_millis()
            );
        }
    };
    // THE PIPE IS CLOSED, which for `wl-paste` is its exit — but a program
    // may close its output and live on, so the exit is waited for inside the
    // same deadline, and not a moment past it.
    let status = loop {
        if let Some(status) = child.try_wait().context("the child did not finish")? {
            break status;
        }
        if started.elapsed() >= patience {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("{program} handed its output over and did not exit");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    if !status.success() {
        tracing::debug!(%status, program, "the clipboard gave back nothing of that type");
        return Ok(None);
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const SH: &str = "/bin/sh";

    #[test]
    fn what_is_written_is_what_is_read() {
        let got = read_within(SH, &["-c", "printf 'a b\\nc'"], PATIENCE, 64).expect("read");
        assert_eq!(got.as_deref(), Some(&b"a b\nc"[..]));
    }

    /// AN EMPTY CLIPBOARD is `wl-paste` exiting 1, and that is no text — not
    /// an error.
    #[test]
    fn a_failure_is_nothing_copied() {
        let got = read_within(SH, &["-c", "exit 1"], PATIENCE, 64).expect("an answer");
        assert_eq!(got, None);
    }

    /// THE DEFECT: a program that never answers held the window's loop. Here
    /// it holds nothing past the deadline.
    #[test]
    fn a_program_that_never_answers_is_given_up_on() {
        let started = std::time::Instant::now();
        let got = read_within(
            SH,
            &["-c", "while :; do :; done"],
            Duration::from_millis(150),
            64,
        );
        assert!(got.is_err(), "{got:?}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "it waited {:?}",
            started.elapsed()
        );
    }

    /// AND PAST THE CEILING it is refused, not read to the end.
    #[test]
    fn more_than_the_ceiling_is_refused() {
        let got = read_within(
            SH,
            &["-c", "while :; do printf xxxxxxxxxxxxxxxx; done"],
            PATIENCE,
            1024,
        );
        assert!(got.is_err(), "{got:?}");
    }
}
