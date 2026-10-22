//! What the kernel's console is bound to, and which terminals it holds —
//! read from sysfs, for `ronoctua-console-font` (`nix/modules/console.nix`).
//!
//! # Why a font is loaded only where the console can take it
//!
//! `systemd-vconsole-setup` runs `setfont`, and `setfont` writes `ERROR` to
//! the journal whenever the console refuses the font — even where systemd
//! itself calls the refusal benign and only says `ignoring`. And three
//! consoles answer a font three ways:
//!
//! - the framebuffer console takes any font: the 15 × 32;
//! - the VGA text console (a machine booted from BIOS, before its graphics
//!   driver) takes only a font eight pixels wide: the 8 × 16 of the same face;
//! - the dummy console takes none. It is bound while the firmware's frame
//!   buffer is handed to the graphics driver, and while `fbcon` defers its
//!   takeover — and the console that binds after it starts the setup again.
//!
//! So the setup does not run on the dummy console at all, the font it loads
//! is the 8 × 16 both others take, and on the framebuffer the 15 × 32 is
//! loaded after it ([`Bound`]).

use std::path::Path;

/// The sysfs folder of the console drivers (`vtcon0`, `vtcon1`…).
pub const VTCONSOLES: &str = "sys/class/vtconsole";

/// The sysfs folder of the allocated terminals (`vcs1`, `vcsu1`, `vcsa1`…).
pub const TERMINALS: &str = "sys/class/vc";

/// The console driver the terminals are bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    /// `fbcon`: any font, the 15 × 32.
    Framebuffer,
    /// `vgacon`: a font eight pixels wide, the 8 × 16.
    VgaText,
    /// The dummy console: no font at all.
    Dummy,
    /// A driver of another name, said by it.
    Other(String),
}

impl Bound {
    /// Whether a font can be loaded on it.
    #[must_use]
    pub fn takes_a_font(&self) -> bool {
        matches!(self, Self::Framebuffer | Self::VgaText)
    }
}

/// The driver a sysfs `name` names: `(M) frame buffer device`, `(S) VGA+`,
/// `(S) dummy device`.
#[must_use]
pub fn driver_of(name: &str) -> Bound {
    let name = name.trim();
    if name.contains("frame buffer") {
        Bound::Framebuffer
    } else if name.contains("VGA") {
        Bound::VgaText
    } else if name.contains("dummy") {
        Bound::Dummy
    } else {
        Bound::Other(name.to_owned())
    }
}

/// The driver bound under `root` (`/` on a machine): the first `vtcon*`
/// whose `bind` is `1`; the dummy console when none is.
#[must_use]
pub fn bound(root: &Path) -> Bound {
    let Ok(read) = std::fs::read_dir(root.join(VTCONSOLES)) else {
        return Bound::Dummy;
    };
    let mut consoles: Vec<_> = read.filter_map(Result::ok).map(|e| e.path()).collect();
    consoles.sort();
    consoles
        .iter()
        .find(|c| std::fs::read_to_string(c.join("bind")).is_ok_and(|b| b.trim() == "1"))
        .and_then(|c| std::fs::read_to_string(c.join("name")).ok())
        .map_or(Bound::Dummy, |name| driver_of(&name))
}

/// The allocated terminals under `root`, ascending: each has a `vcs<N>`
/// (`N` ≥ 1; `vcs` alone is the foreground one, and `vcsu`/`vcsa` are the
/// same terminal's other views).
#[must_use]
pub fn allocated(root: &Path) -> Vec<u32> {
    let Ok(read) = std::fs::read_dir(root.join(TERMINALS)) else {
        return Vec::new();
    };
    let mut terminals: Vec<u32> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_prefix("vcs"))
                .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                .and_then(|n| n.parse().ok())
        })
        .filter(|n| *n >= 1)
        .collect();
    terminals.sort_unstable();
    terminals.dedup();
    terminals
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sysfs of consoles and terminals, under a folder of its own.
    fn sysfs(consoles: &[(&str, &str)], terminals: &[&str]) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "ronoctua-vt-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        for (i, (name, bind)) in consoles.iter().enumerate() {
            let c = root.join(VTCONSOLES).join(format!("vtcon{i}"));
            std::fs::create_dir_all(&c).unwrap();
            std::fs::write(c.join("name"), format!("{name}\n")).unwrap();
            std::fs::write(c.join("bind"), format!("{bind}\n")).unwrap();
        }
        let t = root.join(TERMINALS);
        std::fs::create_dir_all(&t).unwrap();
        for name in terminals {
            std::fs::create_dir_all(t.join(name)).unwrap();
        }
        root
    }

    /// FAILS IF: the driver is read from a console that is not bound, the
    /// three the kernel has are told apart wrongly, or a console of another
    /// name reads as one that takes a font.
    #[test]
    fn the_bound_console_is_the_one_bound() {
        let at = |c: &[(&str, &str)]| bound(&sysfs(c, &[]));
        assert_eq!(
            at(&[("(S) dummy device", "0"), ("(M) frame buffer device", "1")]),
            Bound::Framebuffer
        );
        assert_eq!(
            at(&[("(S) VGA+", "1"), ("(M) frame buffer device", "0")]),
            Bound::VgaText
        );
        assert_eq!(at(&[("(S) dummy device", "1")]), Bound::Dummy);
        assert_eq!(at(&[("(S) dummy device", "0")]), Bound::Dummy, "none bound");
        assert_eq!(at(&[]), Bound::Dummy, "no console driver at all");
        let other = at(&[("(S) MDA-2", "1")]);
        assert_eq!(other, Bound::Other("(S) MDA-2".into()));
        assert!(!other.takes_a_font());
        assert!(Bound::Framebuffer.takes_a_font() && Bound::VgaText.takes_a_font());
        assert!(!Bound::Dummy.takes_a_font());
    }

    /// FAILS IF: a terminal is counted twice for its other views, the
    /// foreground's `vcs` reads as a terminal, or the list is out of order.
    #[test]
    fn the_allocated_terminals_are_the_numbered_ones() {
        let root = sysfs(
            &[],
            &[
                "vcs", "vcsa", "vcsu", "vcs8", "vcsa8", "vcsu8", "vcs1", "vcsa1", "vcsu1", "vcs12",
            ],
        );
        assert_eq!(allocated(&root), vec![1, 8, 12]);
        assert!(allocated(Path::new("/nonexistent-root")).is_empty());
    }
}
