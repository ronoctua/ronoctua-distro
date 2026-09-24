//! The player's window opened for a STAGE — the whole screen, or the small
//! window in the corner — from a door that has no window of its own to put
//! there: the footer and the right column.
//!
//! # Why a file
//!
//! A click on the column's full-screen glyph started `ronoctua-shell player
//! maximized` every time, and every click was one window more — the owner
//! counted them. A button that says "whole screen" is a SWITCH: pressed
//! again, the window it opened goes. So the window writes its pid here
//! (`player-stage.pid`, `<pid> <how>`) and the button asks first.
//!
//! # A living pid is not enough
//!
//! The same reasoning as [`crate::right_column`]: a pid is recycled, so the
//! command line is read too, through the same [`crate::right_column::started_as`].

/// The runtime file.
pub const FILE: &str = "player-stage.pid";

/// The word after `player` that opens each stage.
#[must_use]
pub const fn word_of(show: crate::player::Show) -> &'static str {
    match show {
        crate::player::Show::Only => "only",
        crate::player::Show::Maximized => "maximized",
        crate::player::Show::Small => "pip",
    }
}

/// `<pid> <how>`.
fn parse(text: &str) -> Option<(i32, String)> {
    let mut words = text.split_whitespace();
    let pid = words.next()?.parse().ok()?;
    Some((pid, words.next()?.to_owned()))
}

/// The window holding a stage, and how it was opened — only when it is
/// alive and is this program started that way.
fn holder() -> Option<(i32, String)> {
    let path = ronoctua_xdg::runtime_file(FILE)?;
    let (pid, how) = parse(&std::fs::read_to_string(path).ok()?)?;
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    crate::right_column::started_as(&line, &["player", &how]).then_some((pid, how))
}

/// Written by the window opened for a stage.
pub fn hold(show: crate::player::Show) {
    if let Some(path) = ronoctua_xdg::runtime_file(FILE) {
        let _ = std::fs::write(path, format!("{} {}", std::process::id(), word_of(show)));
    }
}

/// Given up by that window as it goes — only if the file is still its own.
pub fn leave() {
    if let Some(path) = ronoctua_xdg::runtime_file(FILE)
        && std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| parse(&t))
            .is_some_and(|(pid, _)| u32::try_from(pid).ok() == Some(std::process::id()))
    {
        let _ = std::fs::remove_file(path);
    }
}

/// What a door without a window of its own does with a stage: the window
/// that shows it goes if it is up, and is opened if it is not. A window up
/// for ANOTHER stage is replaced — one window for the player's stage at a
/// time.
pub fn toggle(show: crate::player::Show) -> std::io::Result<()> {
    let word = word_of(show);
    if let Some((pid, how)) = holder() {
        if let Some(p) = rustix::process::Pid::from_raw(pid) {
            let _ = rustix::process::kill_process(p, rustix::process::Signal::TERM);
        }
        if how == word {
            return Ok(());
        }
    }
    let me = std::env::current_exe()?;
    std::process::Command::new(me)
        .args(["player", word])
        .stdin(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the file stops saying how the window was opened — the
    /// toggle could not tell "this one again" (close it) from "the other"
    /// (replace it).
    #[test]
    fn the_file_says_how() {
        assert_eq!(parse("42 maximized"), Some((42, "maximized".into())));
        assert_eq!(parse("42"), None);
        assert_eq!(parse(""), None);
    }

    /// FAILS IF: a recycled pid is taken for the stage's window — the
    /// toggle would kill somebody else's process.
    #[test]
    fn only_the_window_is_the_window() {
        let at = |c: &[u8]| crate::right_column::started_as(c, &["player", "pip"]);
        assert!(at(b"/nix/store/x/bin/ronoctua-shell\0player\0pip\0"));
        assert!(!at(b"/nix/store/x/bin/ronoctua-shell\0player\0maximized\0"));
        assert!(!at(b"/nix/store/x/bin/ronoctua-shell\0player\0"));
        assert!(!at(b"/usr/bin/zsh\0player\0pip\0"));
    }
}
