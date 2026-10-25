//! `ronoctua-console-font` — the console's font, only where the console can
//! take it. Run by `systemd-vconsole-setup.service`, in the initrd and in the
//! system (`nix/modules/console.nix`):
//!
//! ```text
//! ronoctua-console-font setup <systemd-vconsole-setup>
//! ronoctua-console-font load --setfont <setfont> --font <psf>
//! ```
//!
//! `setup` is the setup's main process: on a console that takes a font it
//! becomes `systemd-vconsole-setup` (`exec`), and on the dummy one — where
//! every `setfont` is an `ERROR` in the journal — it sets nothing up and
//! succeeds; the console that binds after it starts the setup again
//! (systemd's own `vtconsole` rule). NOT an `ExecCondition`: udev restarts
//! the setup in bursts (a console and each terminal it allocates), and a
//! restart that lands while the condition answers "skip" stops the unit
//! first — and a control process that exits 1 in a stopping unit is a
//! failure, not a skip (measured: one boot in many under load, in the
//! journal). The main process can only exit 0 or be stopped by the
//! `SIGTERM` the unit counts as success. `load` is its
//! `ExecStartPost`: on the framebuffer console it loads the 15 × 32 on every
//! allocated terminal in text mode, over the 8 × 16 the setup loaded. A
//! terminal in graphics mode (the compositor's) refuses a font with
//! `EINVAL`, so it is passed over. See `ronoctua_vt::console`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use ronoctua_vt::console::{self, Bound};

/// `KDGETMODE`, `_IO('K', 0x3B)`: the terminal's display mode into an `int`.
const KDGETMODE: rustix::ioctl::Opcode = rustix::ioctl::opcode::none(b'K', 0x3B);

/// `KD_TEXT`, the mode a font is loaded in.
const KD_TEXT: rustix::ffi::c_int = 0;

const USAGE: &str = "usage: ronoctua-console-font setup <systemd-vconsole-setup>\n       \
                     ronoctua-console-font load --setfont <setfont> --font <psf>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new("/");
    match args.first().map(String::as_str) {
        Some("setup") if args.len() == 2 => setup(&console::bound(root), Path::new(&args[1])),
        Some("load") => match options(&args[1..]) {
            Some((setfont, font)) => load(root, &console::bound(root), &setfont, &font),
            None => refuse(),
        },
        _ => refuse(),
    }
}

/// The usage, and the exit a wrong command line earns.
fn refuse() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// `--setfont <path> --font <path>`, both, once each.
fn options(args: &[String]) -> Option<(PathBuf, PathBuf)> {
    let (mut setfont, mut font) = (None, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = PathBuf::from(it.next()?);
        let slot = match flag.as_str() {
            "--setfont" => &mut setfont,
            "--font" => &mut font,
            _ => return None,
        };
        if slot.replace(value).is_some() {
            return None;
        }
    }
    Some((setfont?, font?))
}

/// Whether the console bound now takes a font — said, when it does not.
fn takes(bound: &Bound) -> bool {
    if bound.takes_a_font() {
        return true;
    }
    match bound {
        Bound::Other(name) => println!(
            "the console is bound to `{name}`, which this setup does not know: no font is loaded"
        ),
        _ => println!(
            "the console is the dummy one, which takes no font: the console that binds \
             starts the setup again"
        ),
    }
    false
}

/// `systemd-vconsole-setup` at `program`, in this process, on a console that
/// takes a font; success, with nothing done, on any other.
fn setup(bound: &Bound, program: &Path) -> ExitCode {
    use std::os::unix::process::CommandExt as _;
    if !takes(bound) {
        return ExitCode::SUCCESS;
    }
    let e = Command::new(program).exec();
    eprintln!("{}: {e}", program.display());
    ExitCode::FAILURE
}

/// On the framebuffer console, `font` on every allocated terminal in text
/// mode; on any other, nothing (the VGA text console keeps the 8 × 16).
fn load(root: &Path, bound: &Bound, setfont: &Path, font: &Path) -> ExitCode {
    if *bound != Bound::Framebuffer {
        return ExitCode::SUCCESS;
    }
    let mut failed = false;
    for n in console::allocated(root) {
        let tty = root.join(format!("dev/tty{n}"));
        match in_text_mode(&tty) {
            Ok(false) => continue,
            Ok(true) => {}
            Err(e) => {
                eprintln!("{}: its display mode cannot be read: {e}", tty.display());
                failed = true;
                continue;
            }
        }
        let ran = Command::new(setfont).arg("-C").arg(&tty).arg(font).output();
        match ran {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                eprintln!(
                    "{}: setfont {}: {}",
                    tty.display(),
                    out.status,
                    String::from_utf8_lossy(&out.stderr).trim()
                );
                failed = true;
            }
            Err(e) => {
                eprintln!("{}: {}: {e}", tty.display(), setfont.display());
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Whether the terminal `tty` is in text mode — the one a font is loaded in.
#[allow(unsafe_code)]
fn in_text_mode(tty: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOCTTY.bits().cast_signed())
        .open(tty)?;
    // SAFETY: KDGETMODE is the VT driver's `_IO('K', 0x3B)`, and it writes
    // the terminal's mode into the `int` the argument points at — `Getter`
    // owns that `int` and hands it back. On a file that is not a terminal the
    // kernel answers ENOTTY, which arrives as the error.
    let mode = unsafe {
        rustix::ioctl::ioctl(
            &file,
            rustix::ioctl::Getter::<KDGETMODE, rustix::ffi::c_int>::new(),
        )
    }?;
    Ok(mode == KD_TEXT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a flag is taken twice, an unknown one passes, or one of the
    /// two is left out — the unit's line is the one thing this reads.
    #[test]
    fn the_command_line_is_both_paths_once() {
        let words = |w: &[&str]| w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            options(&words(&["--setfont", "/s", "--font", "/f"])),
            Some((PathBuf::from("/s"), PathBuf::from("/f")))
        );
        assert_eq!(options(&words(&["--font", "/f"])), None);
        assert_eq!(
            options(&words(&["--setfont", "/s", "--font", "/f", "--font", "/g"])),
            None
        );
        assert_eq!(
            options(&words(&["--setfont", "/s", "--colour", "/f"])),
            None
        );
        assert_eq!(options(&words(&["--setfont"])), None);
    }

    /// FAILS IF: a font is loaded where the console takes none, or the setup
    /// runs on the dummy console — where its `setfont` would be an error in
    /// the journal — or fails there instead of succeeding with nothing done.
    #[test]
    fn only_a_console_that_takes_a_font_is_set_up() {
        assert!(takes(&Bound::Framebuffer));
        assert!(takes(&Bound::VgaText));
        assert!(!takes(&Bound::Dummy));
        assert!(!takes(&Bound::Other("x".into())));
        // THE DUMMY CONSOLE: success, and the setup never run.
        let nowhere = Path::new("/nonexistent-setup");
        assert_eq!(setup(&Bound::Dummy, nowhere), ExitCode::SUCCESS);
        assert_eq!(setup(&Bound::Other("x".into()), nowhere), ExitCode::SUCCESS);
        // A console that takes one runs it — here, one that is not there.
        assert_eq!(setup(&Bound::Framebuffer, nowhere), ExitCode::FAILURE);
        // Nothing to load, and nothing run, off the framebuffer.
        let nowhere = Path::new("/nonexistent-setfont");
        for b in [Bound::VgaText, Bound::Dummy, Bound::Other("x".into())] {
            assert_eq!(
                load(Path::new("/"), &b, nowhere, nowhere),
                ExitCode::SUCCESS,
                "{b:?}"
            );
        }
    }
}
