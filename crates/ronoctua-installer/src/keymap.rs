//! The console's keyboard: the map of the keyboard answered, loaded the
//! moment it is answered.
//!
//! # Why the installer loads it, and what with
//!
//! The installer runs on the kernel's console, and the console does not read
//! xkb: it holds a `kbd` keymap, the distro's default (`us(euro)`) until
//! someone loads another. The questions after the keyboard's are typed — the
//! machine's name, the user's password, the disk's passphrase, the Wi-Fi's —
//! and for as long as nothing loaded the answer they were typed on the
//! default's map: a password with `@ # " \ | ~` typed on a UK keyboard was
//! stored with other characters than the session types, and the disk's
//! passphrase could not be typed again at the initrd's prompt.
//!
//! The ISO cannot compile a map (no xkb compiler on board), so it carries the
//! map of every answer it offers, compiled from the same xkeyboard-config the
//! list is read from (`nix/console-keymaps.nix`, `RONOCTUA_CONSOLE_KEYMAPS`),
//! one file per answer named as xkb writes it ([`name`]): `gb.map`,
//! `us(euro).map`. The installed machine compiles the same answer for its own
//! console and initrd (`nix/modules/console.nix`), from the line `host.nix`
//! carries (`ronoctua.keyboard.console`).
//!
//! # No map is a refusal, never another map
//!
//! An answer with no map (xkeyboard-config's `custom`, whose symbols file
//! does not exist) is refused on its screen ([`map_of`],
//! [`Invalid::NoConsoleKeymap`]): loading the default instead would be the
//! defect again, in silence. The terminal-less mode refuses it too, before
//! anything is touched: the machine's console would have no map to build.

use std::path::{Path, PathBuf};

use crate::key;
use crate::model::Invalid;

/// The variable naming the directory of maps — at run time first, then as
/// it was baked at compile time (the ISO has no environment to set it).
const MAPS: &str = "RONOCTUA_CONSOLE_KEYMAPS";

/// What a map file of the directory ends with.
const SUFFIX: &str = ".map";

/// A keyboard answer as xkb writes it, and as its map is named: the layout
/// with its variant in parentheses (`us(euro)`), or the layout alone when
/// the answer has no variant (`gb`). The same notation as
/// `nix/keyboard-xkb.nix`'s `notation`, which builds the machine's map from
/// the line `host.nix` carries. Pure.
#[must_use]
pub fn name(layout: &str, variant: &str) -> String {
    if variant.is_empty() {
        layout.to_owned()
    } else {
        format!("{layout}({variant})")
    }
}

/// The map of an answer in `dir`: the file named after it ([`name`]) — or
/// the refusal, when the directory has none, or there is no directory.
///
/// # Errors
///
/// [`Invalid::NoConsoleKeymap`]: no map of this answer, which is never
/// stood in for by another.
pub fn map_in(dir: Option<&Path>, layout: &str, variant: &str) -> Result<PathBuf, Invalid> {
    let Some(dir) = dir else {
        return Err(Invalid::NoConsoleKeymap);
    };
    // A NAME THAT IS NOT ONE PATH COMPONENT names no map: the answer comes
    // from the environment in the terminal-less mode, where anything can
    // arrive.
    let file = format!("{}{SUFFIX}", name(layout, variant));
    if layout.is_empty() || file.contains('/') || file.starts_with('.') {
        return Err(Invalid::NoConsoleKeymap);
    }
    let path = dir.join(file);
    if path.is_file() {
        Ok(path)
    } else {
        Err(Invalid::NoConsoleKeymap)
    }
}

/// The directory of maps: the variable as it is now, or as it was baked.
fn maps() -> Option<PathBuf> {
    std::env::var_os(MAPS)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| option_env!("RONOCTUA_CONSOLE_KEYMAPS").map(PathBuf::from))
}

/// The map of an answer on this installer ([`map_in`] over the maps it
/// carries).
///
/// # Errors
///
/// [`Invalid::NoConsoleKeymap`], as [`map_in`].
pub fn map_of(layout: &str, variant: &str) -> Result<PathBuf, Invalid> {
    map_in(maps().as_deref(), layout, variant)
}

/// The console's keyboard, following the answer.
///
/// It exists only on the kernel's console ([`Console::of_term`]): in a
/// terminal of a session, or over SSH, the keys come from that terminal's
/// keyboard and the console's map types nothing here.
#[derive(Debug)]
pub struct Console {
    /// The maps this installer carries.
    maps: Option<PathBuf>,
    /// `loadkeys`, by its store path.
    loadkeys: Option<&'static str>,
    /// The answer last loaded — or tried: a load that failed is said once,
    /// and not tried again until the answer changes.
    held: Option<String>,
}

impl Console {
    /// The console, when the terminal is the kernel's.
    #[must_use]
    pub fn of_term() -> Option<Self> {
        ronoctua_vt::here().then(|| Self {
            maps: maps(),
            loadkeys: option_env!("RONOCTUA_LOADKEYS"),
            held: None,
        })
    }

    /// Loads the map of the keyboard answered, when it is not the one the
    /// console holds. Before any answer the console keeps the ISO's own map,
    /// the default answer's. Synchronous on purpose, and short (tens of
    /// milliseconds): the next key read is typed on the map just loaded.
    ///
    /// # Errors
    ///
    /// The text key that says it: no map of the answer, or the console did
    /// not take it — said once per answer.
    pub fn follow(&mut self, layout: &str, variant: &str) -> Result<(), &'static str> {
        if layout.is_empty() {
            return Ok(());
        }
        let wanted = name(layout, variant);
        if self.held.as_deref() == Some(wanted.as_str()) {
            return Ok(());
        }
        self.held = Some(wanted);
        let map = map_in(self.maps.as_deref(), layout, variant).map_err(|e| e.key())?;
        let Some(loadkeys) = self.loadkeys else {
            return Err(key::INST_CONSOLE_KEYMAP_REFUSED);
        };
        // QUIET, AND UNICODE as the ISO's console is (its locale is UTF-8 —
        // what `systemd-vconsole-setup` loads it with); what it says is kept
        // off the screen the TUI draws.
        let loaded = crate::executor::command(loadkeys)
            .args(["-q", "-u"])
            .arg(&map)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match loaded {
            Ok(s) if s.success() => Ok(()),
            _ => Err(key::INST_CONSOLE_KEYMAP_REFUSED),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of maps as `nix/console-keymaps.nix` writes one, planted
    /// with the answers named, under a name of the test's own.
    fn planted(test: &str, answers: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-installer-keymaps-{test}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for a in answers {
            std::fs::write(dir.join(format!("{a}.map")), "keymaps 0-127\n").unwrap();
        }
        // NOT A MAP: what the build writes beside them.
        std::fs::write(dir.join("left-out.txt"), "custom\tno symbols\n").unwrap();
        dir
    }

    /// FAILS IF: an answer is named otherwise than xkb writes it — the
    /// variant outside parentheses, or empty parentheses for no variant —,
    /// which `nix/keyboard-xkb.nix`'s `notation` would not build.
    #[test]
    fn an_answer_is_named_as_xkb_writes_it() {
        assert_eq!(name("us", "euro"), "us(euro)");
        assert_eq!(name("br", "abnt2"), "br(abnt2)");
        assert_eq!(name("gb", ""), "gb");
        assert_eq!(name("us", "dvorak-intl"), "us(dvorak-intl)");
    }

    /// FAILS IF: the map of an answer is not the file named after it — the
    /// plain layout's taken for a variant's, a variant's for the plain
    /// layout's, or another layout's for one with none.
    #[test]
    fn the_map_is_the_answers_own() {
        let dir = planted("own", &["us", "us(euro)", "gb", "br(abnt2)"]);
        for (layout, variant, file) in [
            ("us", "euro", "us(euro).map"),
            ("us", "", "us.map"),
            ("gb", "", "gb.map"),
            ("br", "abnt2", "br(abnt2).map"),
        ] {
            assert_eq!(
                map_in(Some(&dir), layout, variant),
                Ok(dir.join(file)),
                "{layout}({variant})"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: an answer with no map of its own is given another — the
    /// layout's plain map for a variant it lacks, the default's for a layout
    /// with none — instead of refused; or a name that is not one file of
    /// the directory (a path, the build's own note) is taken for a map; or
    /// an installer with no maps at all finds one.
    #[test]
    fn an_answer_with_no_map_is_refused_and_never_stood_in_for() {
        let dir = planted("none", &["us", "us(euro)", "gb", "br"]);
        for (layout, variant) in [
            // xkeyboard-config's placeholder, with no symbols file.
            ("custom", ""),
            // Variants the plain map must not stand in for.
            ("br", "abnt2"),
            ("gb", "extd"),
            ("", ""),
            ("../us", ""),
            ("left-out", ""),
            (".", "map"),
        ] {
            assert_eq!(
                map_in(Some(&dir), layout, variant),
                Err(Invalid::NoConsoleKeymap),
                "{layout}({variant})"
            );
        }
        assert_eq!(map_in(None, "us", "euro"), Err(Invalid::NoConsoleKeymap));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the console is touched before any keyboard is answered; an
    /// answer with no map loads anything instead of saying so; a console
    /// that does not take the map (its loader does not run, or is not
    /// there) is not said; or a refusal is said again for the same answer
    /// instead of once.
    #[test]
    fn the_console_follows_the_answer_and_says_a_refusal_once() {
        let dir = planted("follow", &["gb"]);
        let mut console = Console {
            maps: Some(dir.clone()),
            loadkeys: Some("/nonexistent/loadkeys"),
            held: None,
        };
        assert_eq!(console.follow("", ""), Ok(()));
        assert!(console.held.is_none(), "touched before any answer");
        assert_eq!(
            console.follow("custom", ""),
            Err(key::INST_INVALID_CONSOLE_KEYMAP)
        );
        assert_eq!(console.follow("custom", ""), Ok(()), "said twice");
        assert_eq!(
            console.follow("gb", ""),
            Err(key::INST_CONSOLE_KEYMAP_REFUSED)
        );
        assert_eq!(console.held.as_deref(), Some("gb"));
        let mut without = Console {
            maps: Some(dir.clone()),
            loadkeys: None,
            held: None,
        };
        assert_eq!(
            without.follow("gb", ""),
            Err(key::INST_CONSOLE_KEYMAP_REFUSED)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
