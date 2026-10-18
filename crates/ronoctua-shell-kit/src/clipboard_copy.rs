//! A TEXT OR AN IMAGE PUT ON THE SYSTEM CLIPBOARD, through `wl-copy`.
//!
//! # Why a module of its own
//!
//! It was the buffer host's, which needed it first; then the chrome, the
//! file manager, the git history and the notifications all copied through
//! it. Copying is a door's verb, and it lives under every door.

use anyhow::{Context as _, Result};
// THE BRIDGE MOVED OUT, and this is the only copy: see `paint`. The
// terminal already drew display lists through it, and the offscreen tool
// that writes a buffer to a PNG needs the same conversion — a second one
// would prove things about itself.

/// Puts a text on the system clipboard.
///
/// # Why a process, and not a `wl_data_device` of our own
///
/// Because whoever **owns** the selection has to stay alive to serve it
/// when someone pastes. The buffer's window dies when the browser is closed
/// — and copying a glyph **in order to** close the browser and paste is exactly the
/// use case. An owner that dies with it would hand over an empty clipboard, which
/// is worse than not copying: it looks as if it worked.
///
/// `wl-copy` leaves a minimal process serving the selection until someone
/// replaces it. It is the same choice as `scrollmsg`: using the client that
/// already exists instead of reimplementing the other side's protocol.
///
/// # Errors
///
/// The binary was not put in the build, did not run, or exited with an error.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    copy_bytes(text.as_bytes(), None)
}

/// Puts an IMAGE FILE on the system clipboard, as `image/png` — a capture,
/// pasted straight into a chat or a document.
///
/// The same `wl-copy` as [`copy_to_clipboard`], told the type: without it the
/// bytes would be offered as text, and a paste would be a page of garbage.
///
/// # Errors
///
/// The file does not read, or `wl-copy` failed as [`copy_to_clipboard`] can.
pub fn copy_image_to_clipboard(file: &std::path::Path) -> Result<()> {
    let bytes =
        std::fs::read(file).with_context(|| format!("could not read {}", file.display()))?;
    copy_bytes(&bytes, Some("image/png"))
}

// UNDER EVERY CRATE'S TESTS, and not only this one's: `cfg(test)` is true in
// this crate's own test build alone, and the footer's and every feature's
// tests copy through here too — so the guard holds under `test-support`,
// which every crate above names in its `[dev-dependencies]`.
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// WHAT THE TESTS "COPIED", in place of the system clipboard: under
    /// `cargo test` nothing may reach the clipboard of the session the tests
    /// run in — a test that copied left a `wl-copy` serving over the person's
    /// own clipboard, and held the pipe of whoever ran the suite open. Each
    /// entry is the type it was offered as, if any, and the bytes.
    pub static COPIED: std::cell::RefCell<Vec<(Option<String>, Vec<u8>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The one way bytes reach `wl-copy`, with or without a type.
fn copy_bytes(bytes: &[u8], mime: Option<&str>) -> Result<()> {
    #[cfg(any(test, feature = "test-support"))]
    {
        COPIED.with(|c| {
            c.borrow_mut()
                .push((mime.map(ToOwned::to_owned), bytes.to_vec()))
        });
        Ok(())
    }
    #[cfg(not(any(test, feature = "test-support")))]
    copy_bytes_for_real(bytes, mime)
}

/// [`copy_bytes`], outside the tests.
#[cfg(not(any(test, feature = "test-support")))]
fn copy_bytes_for_real(bytes: &[u8], mime: Option<&str>) -> Result<()> {
    use std::io::Write as _;

    // A STORE PATH, never a name: the `PATH` of a systemd scope is
    // minimal, and the failure would show up far from the cause.
    let path = option_env!("RONOCTUA_WL_COPY").context(
        "RONOCTUA_WL_COPY was not set at build time.\n\
             It comes from `nix/rust.nix`; without it there is no way to copy.",
    )?;

    let mut cmd = std::process::Command::new(path);
    if let Some(mime) = mime {
        cmd.args(["--type", mime]);
    }
    // ITS PATH, and not ours. See `RONOCTUA_WL_COPY_PATH` in
    // `nix/rust.nix`: `wl-copy` does `exec cat`, and under a systemd
    // unit PATH is minimal. Without this the copy fails with
    // `exec cat: Node such file or directory` — and failing silently here
    // would be going back to the defect this whole path fixes.
    if let Some(p) = option_env!("RONOCTUA_WL_COPY_PATH") {
        cmd.env("PATH", p);
    }
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        // INHERITED, and not a pipe of ours.
        //
        // Why this matters: `wl-copy` FORKS a child that stays serving
        // the selection, and that child inherits the pipes. `wait_with_output()` reads the
        // pipes to EOF — and the EOF only comes when the selection's server dies,
        // that is, never. The buffer's event loop stayed stuck there, and
        // on screen that appeared as "the buffer stopped responding after
        // copying". By inheriting, `wl-copy`'s error message lands in OUR
        // journal, which is where it is any use.
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .with_context(|| format!("could not run {path}"))?;

    child
        .stdin
        .take()
        .context("wl-copy did not open stdin")?
        .write_all(bytes)
        .context("could not write to wl-copy")?;

    // WAIT FOR THE PARENT, and only it. `wl-copy` forks: the parent exits as soon as the
    // selection is set, and the child stays serving. Without waiting, an error
    // (a compositor with no `wl_data_device`, for example) would come out as success, and
    // we would be back to the defect this fixes.
    //
    // `wait()`, and NOT `wait_with_output()`: see the `stderr` comment.
    let status = child.wait().context("wl-copy did not finish")?;
    if !status.success() {
        anyhow::bail!("wl-copy exited with {status}; the reason is in the log above");
    }
    Ok(())
}
