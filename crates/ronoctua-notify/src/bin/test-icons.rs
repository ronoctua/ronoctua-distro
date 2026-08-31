//! Exercises the icon chain against the REAL superpackage.
//!
//! # Why a binary, and not a `#[test]`
//!
//! Because the superpackage is a derivation: its path only exists at Nix
//! build time, and a unit test would have to guess it or skip it. A
//! test that skips itself is the façade this project spent months
//! undoing — it goes green exactly when it stops holding.
//!
//! # What it asserts, and which defect each thing catches
//!
//! - **The actual names resolve.** `firefox`, `code`, `org.gnome.Nautilus`: if
//!   the context lookup breaks, the pixmap step returns `None` for everything
//!   and the list is left with only the category glyphs — which **look
//!   deliberate**. It is the most expensive defect here, because it has no symptom.
//! - **A pixel comes out.** Finding the file is not drawing: an SVG `usvg`
//!   rejects gives `None` in the same place, and one that rasterises white gives an
//!   invisible smear over the dark background. That is why the probe counts OPAQUE
//!   pixels, and not just bytes.
//! - **What does not exist returns `None`.** Without this, "everything resolved" could
//!   mean "the function always returns something".
//! - **The size is the one asked for.** It is what the layout reserves; a 512 px icon
//!   delivered as such would come out on top of the name.

use ronoctua_notify::icon;

/// The side the footer asks for — the font's body. See `apps::pixmap_side`.
const SIDE: u32 = 18;

/// `Icon=` names that appear in actual `.desktop` files.
///
/// All three forms coexist on the same system, and the chain has to serve all three:
/// a short binary name, a dotted vendor name, and a generic freedesktop icon
/// name.
const NAMES: &[&str] = &[
    "firefox",
    "code",
    "org.gnome.Nautilus",
    "utilities-terminal",
    "text-editor",
    "system-file-manager",
];

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(root) = args.next() else {
        eprintln!("usage: ronoctua-test-icons <superpack-root>");
        return std::process::ExitCode::FAILURE;
    };
    let root = std::path::PathBuf::from(root);
    if !root.is_dir() {
        eprintln!("ERROR: {} is not a directory", root.display());
        return std::process::ExitCode::FAILURE;
    }

    let mut failed = false;
    for name in NAMES {
        let Some(rgba) = icon::rasterize(Some(&root), name, SIDE) else {
            eprintln!("ERROR: `{name}` did not resolve in the superpack.");
            eprintln!("       The pixmap stage would give back `None` for everything, and the");
            eprintln!("       application list would be left with only the glyphs of the");
            eprintln!("       category — which look deliberate.");
            failed = true;
            continue;
        };
        if (rgba.width, rgba.height) != (SIDE, SIDE) {
            eprintln!(
                "ERROR: `{name}` came out {}x{}, and the layout reserves {SIDE}x{SIDE}",
                rgba.width, rgba.height
            );
            failed = true;
            continue;
        }
        // OPAQUE PIXELS, and not bytes: an SVG that rasterises empty returns the
        // whole buffer with zero alpha — invisible over the footer's background, and
        // indistinguishable from "the icon has not arrived yet".
        let opaque = rgba.pixels.chunks_exact(4).filter(|p| p[3] > 0).count();
        let total = (SIDE * SIDE) as usize;
        if opaque * 100 < total * 5 {
            eprintln!("ERROR: `{name}` rasterised with {opaque}/{total} pixels visible");
            failed = true;
            continue;
        }
        println!("OK: {name} — {opaque}/{total} pixels visible");
    }

    // THE OTHER SIDE: without this, "everything resolved" could only mean that the
    // function always returns something.
    let missing = "zzz-there-is-no-icon-with-this-name";
    if icon::rasterize(Some(&root), missing, SIDE).is_some() {
        eprintln!("ERROR: a made-up name resolved; the chain answers anything");
        failed = true;
    }
    if icon::rasterize(Some(&root), "firefox", 0).is_some() {
        eprintln!("ERROR: a side of zero gave back an image");
        failed = true;
    }

    if failed {
        return std::process::ExitCode::FAILURE;
    }
    println!(
        "OK: {} real names resolve and draw at {SIDE}px",
        NAMES.len()
    );
    std::process::ExitCode::SUCCESS
}
