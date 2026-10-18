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
//! # Every window, a file
//!
//! The plain window has no stage and no slot to toggle, but closing the
//! player everywhere has to find it when no daemon is there to say farewell:
//! every window of the player holds `player-window-<pid>.pid` while it is
//! open ([`WindowSlot`], [`end_every_window`]).
//!
//! # A living pid is not enough
//!
//! The same reasoning as [`crate::right_column`]: a pid is recycled, so the
//! command line is read too, through the same [`crate::right_column::started_as`].
//!
//! # Every screen, a window
//!
//! "Show on every screen" (`M`, `player-every-screen`) opens one window per
//! screen something of what plays is routed to — `ronoctua-shell player
//! maximized --screen <connector>` — each made whole-screen ON ITS OWN OUTPUT
//! (`xdg_toplevel.set_fullscreen`, the xdg-shell's own argument: invariant 3)
//! and showing the pictures routed there. Each holds a slot of its own,
//! `player-stage@<connector>.pid`, so pressing again finds every one of them
//! and closes them all ([`everywhere`]); with one screen there is nothing to
//! spread, and that is said.

/// The runtime file.
pub const FILE: &str = "player-stage.pid";

/// The word after `player` that opens each stage.
#[must_use]
pub const fn word_of(show: crate::player::Show) -> &'static str {
    match show {
        crate::player::Show::Only => "only",
        crate::player::Show::Maximized => "maximized",
        crate::player::Show::Small => "pip",
        crate::player::Show::Listing => "listing",
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

/// Ends the window holding a stage, whichever it is; whether one went. What
/// closing the player everywhere does to it when no daemon is there to say
/// farewell. Never this process: a stage's window closing itself quits.
pub fn end() -> bool {
    holder()
        .filter(|(pid, _)| u32::try_from(*pid).ok() != Some(std::process::id()))
        .and_then(|(pid, _)| rustix::process::Pid::from_raw(pid))
        .is_some_and(|p| rustix::process::kill_process(p, rustix::process::Signal::TERM).is_ok())
}

/// EVERY WINDOW OF THE PLAYER, the plain one too — which has no stage slot —
/// holds a file of its own, `player-window-<pid>.pid`, for as long as it is
/// open: closing the player everywhere with no daemon to say farewell (after
/// a crash, a unit given up) still finds every one ([`end_every_window`]).
/// One file per window, because windows are as many as a person opens.
const WINDOW_PREFIX: &str = "player-window-";

/// What a window's file is named after the prefix: its pid, and `.pid`.
const WINDOW_SUFFIX: &str = ".pid";

/// A player window's file, held for as long as the window is open — made by
/// `player_window::host_window`, around the door, and never by a detached
/// player (a photograph), which is no window of the machine.
pub struct WindowSlot(Option<std::path::PathBuf>);

impl WindowSlot {
    /// Writes this process's file.
    #[must_use]
    pub fn hold() -> Self {
        let path = ronoctua_xdg::runtime_file(&format!(
            "{WINDOW_PREFIX}{}{WINDOW_SUFFIX}",
            std::process::id()
        ));
        if let Some(p) = &path
            && let Err(e) = std::fs::write(p, b"")
        {
            tracing::warn!(error = %e, "the player window's file could not be written");
        }
        Self(path)
    }
}

impl Drop for WindowSlot {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// The pid a window's file NAME carries, when `name` is one — `prefix` is the
/// whole leaf before the pid, the instance's name included.
fn window_pid(name: &str, prefix: &str) -> Option<i32> {
    name.strip_prefix(prefix)?
        .strip_suffix(WINDOW_SUFFIX)?
        .parse()
        .ok()
        .filter(|pid| *pid > 0)
}

/// Ends every window of the player but this process's own; how many went.
/// Each is believed only when its `/proc/<pid>/cmdline` is this program
/// started as `player` (a pid is recycled); its file goes either way — one
/// ended by `TERM` never ran its own `Drop`, and one whose pid is somebody
/// else's now names nothing.
pub fn end_every_window() -> usize {
    let Some(probe) = ronoctua_xdg::runtime_file(WINDOW_PREFIX) else {
        return 0;
    };
    let (Some(dir), Some(prefix)) = (
        probe.parent(),
        probe.file_name().map(|n| n.to_string_lossy().into_owned()),
    ) else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let me = i32::try_from(std::process::id()).ok();
    let mut ended = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = window_pid(&name, &prefix) else {
            continue;
        };
        if Some(pid) == me {
            continue;
        }
        let ours = std::fs::read(format!("/proc/{pid}/cmdline"))
            .is_ok_and(|line| crate::right_column::started_as(&line, &["player"]));
        if ours
            && rustix::process::Pid::from_raw(pid).is_some_and(|p| {
                rustix::process::kill_process(p, rustix::process::Signal::TERM).is_ok()
            })
        {
            ended += 1;
        }
        let _ = std::fs::remove_file(entry.path());
    }
    ended
}

/// The word after `--screen`: a stage window for one screen.
pub const SCREEN: &str = "--screen";

/// What a slot's file for screen `connector` is called after the runtime
/// prefix — `None` for a connector that is no safe file name (a compositor
/// names them `eDP-1`, `DP-2`, `HDMI-A-1`; anything else is refused rather
/// than written somewhere else).
fn screen_leaf(connector: &str) -> Option<String> {
    let safe = !connector.is_empty()
        && !connector.starts_with('.')
        && connector
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    safe.then(|| format!("{SCREEN_PREFIX}{connector}{WINDOW_SUFFIX}"))
}

/// The leaf a screen's slot starts with.
const SCREEN_PREFIX: &str = "player-stage@";

/// The connector a screen slot's NAME carries, when `name` is one — `prefix`
/// is the whole leaf before the connector, the instance's name included.
fn screen_of(name: &str, prefix: &str) -> Option<String> {
    let connector = name.strip_prefix(prefix)?.strip_suffix(WINDOW_SUFFIX)?;
    screen_leaf(connector).map(|_| connector.to_owned())
}

/// Written by the window opened for screen `connector`'s stage: its pid.
/// The slot it wrote, to give up as it goes ([`ScreenSlot`]).
pub struct ScreenSlot(Option<std::path::PathBuf>);

impl ScreenSlot {
    /// This process's slot for `connector`.
    #[must_use]
    pub fn hold(connector: &str) -> Self {
        let path = screen_leaf(connector).and_then(|leaf| ronoctua_xdg::runtime_file(&leaf));
        if let Some(p) = &path
            && let Err(e) = std::fs::write(p, std::process::id().to_string())
        {
            tracing::warn!(error = %e, "the screen's stage slot could not be written");
        }
        Self(path)
    }
}

impl Drop for ScreenSlot {
    fn drop(&mut self) {
        // ONLY IF STILL ITS OWN: a window started for the same screen since
        // wrote its own pid there.
        if let Some(p) = &self.0
            && std::fs::read_to_string(p)
                .ok()
                .and_then(|t| t.trim().parse::<u32>().ok())
                == Some(std::process::id())
        {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Every window holding a screen's stage, by its connector and pid — each
/// believed only when its command line is this program opened for THAT
/// screen; the file of one that is not goes.
fn screen_holders() -> Vec<(String, i32)> {
    let Some(probe) = ronoctua_xdg::runtime_file(SCREEN_PREFIX) else {
        return Vec::new();
    };
    let (Some(dir), Some(prefix)) = (
        probe.parent(),
        probe.file_name().map(|n| n.to_string_lossy().into_owned()),
    ) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut up = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(connector) = screen_of(&name, &prefix) else {
            continue;
        };
        let pid = std::fs::read_to_string(entry.path())
            .ok()
            .and_then(|t| t.trim().parse::<i32>().ok())
            .filter(|pid| *pid > 0);
        let alive = pid.filter(|pid| {
            std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|line| {
                crate::right_column::started_as(
                    &line,
                    &[
                        "player",
                        word_of(crate::player::Show::Maximized),
                        SCREEN,
                        &connector,
                    ],
                )
            })
        });
        match alive {
            Some(pid) => up.push((connector, pid)),
            None => {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    up
}

/// What "show on every screen" does, from what is up and what is there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Everywhere {
    /// The windows that were up are closed: this many.
    Closed(usize),
    /// A window opened for each of these screens.
    Opened(Vec<String>),
    /// One screen is all there is: nothing to spread.
    OneScreen,
    /// The door knows no screen by its name yet — none can be asked for,
    /// and "there is one screen" would be a guess.
    Unknown,
    /// Nothing of what plays has a picture routed to a screen.
    NothingRouted,
}

/// The decision, pure: `up` windows for screens are open, `connected`
/// screens are known, and `routed` are the screens something goes to.
#[must_use]
pub fn everywhere_plan(up: usize, connected: usize, routed: &[String]) -> Everywhere {
    if up > 0 {
        Everywhere::Closed(up)
    } else if connected == 0 {
        Everywhere::Unknown
    } else if connected < 2 {
        Everywhere::OneScreen
    } else if routed.is_empty() {
        Everywhere::NothingRouted
    } else {
        Everywhere::Opened(routed.to_vec())
    }
}

/// SHOW ON EVERY SCREEN, or close what it opened: the windows up for screens
/// go; else a window is opened for each of `routed` — when there are two
/// screens or more (`connected`) and something goes to them.
///
/// # Errors
///
/// A window could not be started.
pub fn everywhere(connected: usize, routed: &[String]) -> std::io::Result<Everywhere> {
    crate::daemon::about_to_act("the player's windows on every screen, opened or closed");
    let up = screen_holders();
    let plan = everywhere_plan(up.len(), connected, routed);
    match &plan {
        Everywhere::Closed(_) => {
            for (_, pid) in up {
                if let Some(p) = rustix::process::Pid::from_raw(pid) {
                    let _ = rustix::process::kill_process(p, rustix::process::Signal::TERM);
                }
            }
        }
        Everywhere::Opened(screens) => {
            let me = std::env::current_exe()?;
            for screen in screens {
                let Some(slot) = screen_leaf(screen).and_then(|l| ronoctua_xdg::runtime_file(&l))
                else {
                    tracing::warn!(screen, "a screen whose name is no file name: not opened");
                    continue;
                };
                let child = std::process::Command::new(&me)
                    .args([
                        "player",
                        word_of(crate::player::Show::Maximized),
                        SCREEN,
                        screen,
                    ])
                    .stdin(std::process::Stdio::null())
                    .spawn()?;
                // ITS SLOT, WRITTEN NOW by the one that opened it: the window
                // writes the same pid once it is up, and a second press
                // before then — a key held — found no slot and opened every
                // window again.
                if let Err(e) = std::fs::write(&slot, child.id().to_string()) {
                    tracing::warn!(error = %e, "the screen's stage slot could not be written");
                }
                reaped(child);
            }
        }
        Everywhere::OneScreen | Everywhere::Unknown | Everywhere::NothingRouted => {}
    }
    Ok(plan)
}

/// A window this door started, WAITED FOR on a thread of its own until it
/// ends — else it would stay a zombie of the door that started it, the
/// footer lives for the whole session — and never on the loop.
///
/// NO SCOPE, as the stage's window and the column have none: its pid is the
/// slot it is found by, known the moment it starts, and `systemd-run
/// --scope` becomes the program only after a round trip of its own.
fn reaped(child: std::process::Child) {
    let _ = crate::foreign::waited(child);
}

/// What a door says of `e`.
#[must_use]
pub fn everywhere_said(e: &Everywhere, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    match e {
        Everywhere::Closed(_) => texts.t(key::PLAYER_EVERY_SCREEN_CLOSED),
        Everywhere::Opened(screens) => texts.t_args(
            key::PLAYER_EVERY_SCREEN_OPENED,
            &[(
                "count",
                ronoctua_i18n::Arg::Number(i64::try_from(screens.len()).unwrap_or(i64::MAX)),
            )],
        ),
        Everywhere::OneScreen => texts.t(key::PLAYER_EVERY_SCREEN_ONE),
        Everywhere::Unknown => texts.t(key::PLAYER_SCREENS_UNKNOWN),
        Everywhere::NothingRouted => texts.t(key::PLAYER_EVERY_SCREEN_NOTHING),
    }
}

/// What a door without a window of its own does with a stage: the window
/// that shows it goes if it is up, and is opened if it is not. A window up
/// for ANOTHER stage is replaced — one window for the player's stage at a
/// time.
pub fn toggle(show: crate::player::Show) -> std::io::Result<()> {
    crate::daemon::about_to_act("the player's stage window, started or ended");
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
        .map(reaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the file stops saying how the window was opened — the
    /// toggle could not tell "this one again" (close it) from "the other"
    /// (replace it).
    /// FAILS IF: a window's file is not read back as its pid — closing the
    /// player everywhere would miss that window — or a file of anything else
    /// (another instance's, the stage's, a half-written name) is read as one.
    #[test]
    fn a_window_file_names_its_pid_and_nothing_else_does() {
        let prefix = "ronoctua-player-window-";
        assert_eq!(
            window_pid("ronoctua-player-window-4242.pid", prefix),
            Some(4242)
        );
        assert_eq!(
            window_pid("ronoctua-dev-player-window-4242.pid", prefix),
            None
        );
        assert_eq!(window_pid("ronoctua-player-stage.pid", prefix), None);
        assert_eq!(window_pid("ronoctua-player-window-.pid", prefix), None);
        assert_eq!(window_pid("ronoctua-player-window-0.pid", prefix), None);
        assert_eq!(
            window_pid("ronoctua-player-window-12.pid.tmp", prefix),
            None
        );
    }

    #[test]
    fn the_file_says_how() {
        assert_eq!(parse("42 maximized"), Some((42, "maximized".into())));
        assert_eq!(parse("42"), None);
        assert_eq!(parse(""), None);
    }

    /// FAILS IF: two screens share a slot — pressing again would close one
    /// screen's window and leave the other up — a slot's name is not read
    /// back as its screen, another file (the one stage's slot, a window's,
    /// another instance's) is read as one, or a connector that is no safe
    /// file name gets a slot at all.
    #[test]
    fn each_screen_has_its_own_slot() {
        let a = screen_leaf("eDP-1").expect("a slot");
        let b = screen_leaf("DP-2").expect("a slot");
        assert_ne!(a, b);
        let prefix = "ronoctua-player-stage@";
        assert_eq!(
            screen_of(&format!("ronoctua-{a}"), prefix).as_deref(),
            Some("eDP-1")
        );
        assert_eq!(
            screen_of("ronoctua-player-stage@HDMI-A-1.pid", prefix).as_deref(),
            Some("HDMI-A-1")
        );
        assert_eq!(screen_of("ronoctua-player-stage.pid", prefix), None);
        assert_eq!(screen_of("ronoctua-player-window-42.pid", prefix), None);
        assert_eq!(
            screen_of("ronoctua-dev-player-stage@DP-2.pid", prefix),
            None
        );
        assert_eq!(screen_of("ronoctua-player-stage@.pid", prefix), None);
        for bad in ["", "../x", ".hidden", "DP 2", "a/b"] {
            assert_eq!(screen_leaf(bad), None, "{bad:?}");
        }
    }

    /// FAILS IF: pressing again stops closing what is up, one screen stops
    /// being said instead of opening a window on it, a door that knows no
    /// screen by name says there is one, a screen nothing goes to gets a
    /// window, or the windows opened are not one per routed screen.
    #[test]
    fn every_screen_opens_one_a_screen_and_closes_them_again() {
        let routed = vec!["eDP-1".to_owned(), "DP-2".to_owned()];
        assert_eq!(everywhere_plan(2, 2, &routed), Everywhere::Closed(2));
        assert_eq!(everywhere_plan(0, 1, &routed), Everywhere::OneScreen);
        assert_eq!(everywhere_plan(0, 0, &routed), Everywhere::Unknown);
        assert_eq!(everywhere_plan(0, 3, &[]), Everywhere::NothingRouted);
        assert_eq!(
            everywhere_plan(0, 3, &routed),
            Everywhere::Opened(routed.clone())
        );
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        for e in [
            Everywhere::Closed(2),
            Everywhere::Opened(routed),
            Everywhere::OneScreen,
            Everywhere::Unknown,
            Everywhere::NothingRouted,
        ] {
            assert!(!everywhere_said(&e, &t).is_empty(), "{e:?} says nothing");
        }
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
