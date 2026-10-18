//! THE CAMERA'S DOORS — `ronoctua-shell photograph`.
//!
//! The walk is the player's (`player_photograph`, in the player's crate: its
//! window and its column, every step of a person's walk, the frame drawn into
//! a PNG or printed). This file hands it the doors that are NOT the player's
//! ([`OTHER_DOORS`]): each door with a camera of its own, which takes the
//! rest of the words (`--door files`) and is written in its crate's table
//! ([`CAMERAS`]), and the three native doors that RIDE the player's walk —
//! keys, the pointer, the frames — drawn in its window's place ([`RIDERS`]):
//! the update's window, the footer's settings and the calculator, with their
//! own words (`--modeline`, `--unwritable`, `--report`, `--journal`). So a
//! camera added to a crate recompiles that crate and the binary, and nothing
//! of the player.
//!
//! Usage: `ronoctua-shell photograph [options]`, `--help` for the list.

use anyhow::{Context as _, Result, ensure};

use ronoctua_shell_door::door::{Detached, Ui};
use ronoctua_shell_door::photo::Camera;
use ronoctua_shell_player::player_photograph::{
    Asked, OtherDoors, OwnCamera, Rider, shoot, shoot_saying,
};
use ronoctua_shell_player::player_window::Stage;

/// THE HEAD OF `--help`: the doors and the riders' words — the player's
/// camera prints its own after it (`player_photograph::USAGE`).
const USAGE: &str = "\
usage: ronoctua-shell photograph [options] [steps]

  --door <door>         window, column, footer-settings, update,
                        update-scene, files, git, icons, calc,
                        video-metrics, security, containers, belongings,
                        stick, software, incus, agents, fast-access,
                        network, bluetooth, locale, camera-settings,
                        display, clipboard, curtain, lifeline or footer
                        (default: window);
                        footer-settings is the footer's own settings in
                        their window (`LEADER s r w`), no player's door;
                        update is the update's window (`LEADER s u w`), over
                        `--report` and `--journal`; calc is the calculator's
                        window over an empty history of its own, which keys
                        and clicks fill; every other one reads the rest of
                        the line itself (see `photograph --door <door>
                        --help`)
  --modeline <file>     the footer's preferences as `modeline.json` keeps
                        them, for footer-settings — never this machine's
                        (default: the defaults); a copy is what a key
                        changes, and the file given is left as it was. A
                        file that does not read as written (not JSON, a
                        value repaired) is drawn as the window opens over
                        it, with the sentence it is owed
  --unwritable          footer-settings over a directory that will not take
                        the file, so a key that changes a setting draws the
                        refusal
  --report <file>       for update: one half's report, as its runner writes
                        it (`<step> TAB <verdict> TAB <seconds>` a line) —
                        repeatable, and never this machine's
                                     (default: none, every step still to come)
  --journal <file>      for update: what the two journals said, a line each,
                        as `journalctl -o cat` prints them — never this
                        machine's                    (default: nothing said)
";

/// Every door's word, as a door that is none of them is refused with.
const NAMED: &str = "window, column, footer-settings, update, update-scene, files, git, icons, calc, video-metrics, security, containers, belongings, stick, software, incus, agents, fast-access, network, bluetooth, locale, camera-settings, display, clipboard, curtain, lifeline and footer";

/// THE DOORS THAT ARE NOT THE PLAYER'S, as the player's camera reads them.
const OTHER_DOORS: OtherDoors = OtherDoors {
    usage: USAGE,
    named: NAMED,
    camera,
    riders: RIDERS,
    words: WORDS,
    read,
    drawing,
};

/// THE NATIVE DOORS THAT RIDE THE PLAYER'S WALK, drawn in its window's place.
const RIDERS: &[Rider] = &[
    // THE UPDATE'S WINDOW, over the reports and the journal given.
    Rider {
        door: "update",
        shoot: update,
    },
    // THE FOOTER'S SETTINGS, over the preferences given.
    Rider {
        door: "footer-settings",
        shoot: footer_settings,
    },
    // THE CALCULATOR'S WINDOW, over a history of its own.
    Rider {
        door: "calc",
        shoot: calc,
    },
];

/// The riders' own words, each with whether a value follows it.
const WORDS: &[(&str, bool)] = &[
    ("--modeline", true),
    ("--unwritable", false),
    ("--report", true),
    ("--journal", true),
];

/// `ronoctua-shell photograph …`: draws the door the words ask for into a
/// PNG — or prints its frame.
///
/// # Errors
///
/// A word not understood, a file that is not what it says, no graphics
/// adapter, or a picture larger than the GPU takes.
pub fn run(words: &[String]) -> Result<()> {
    ronoctua_shell_player::player_photograph::run(words, &OTHER_DOORS)
}

/// EVERY CRATE'S CAMERAS, by the word `--door` names each with — one table
/// per crate, sorted, each written beside the doors it photographs
/// (`photo::Camera`) — and the two that need what two crates make, the
/// binary's own ([`OWN`]).
const CAMERAS: &[&[Camera]] = &[
    OWN,
    ronoctua_shell_agents::CAMERAS,
    ronoctua_shell_belongings::CAMERAS,
    ronoctua_shell_bluetooth::CAMERAS,
    ronoctua_shell_camera::CAMERAS,
    ronoctua_shell_clipboard::CAMERAS,
    ronoctua_shell_containers::CAMERAS,
    ronoctua_shell_display::CAMERAS,
    ronoctua_shell_fast_access::CAMERAS,
    ronoctua_shell_git::CAMERAS,
    ronoctua_shell_icons::CAMERAS,
    ronoctua_shell_incus::CAMERAS,
    ronoctua_shell_lifeline::CAMERAS,
    ronoctua_shell_locale::CAMERAS,
    ronoctua_shell_network::CAMERAS,
    ronoctua_shell_overlays::CAMERAS,
    ronoctua_shell_security::CAMERAS,
    ronoctua_shell_software::CAMERAS,
    ronoctua_shell_stick::CAMERAS,
    ronoctua_shell_update::CAMERAS,
    ronoctua_shell_video_metrics::CAMERAS,
];

/// THE BINARY'S OWN CAMERAS: the file manager's window, handed the flow of
/// the vault the security centre writes (`vault_flow`), and the footer
/// itself, over a stand-in compositor, handed the regions it hosts and what
/// its camera plants of a feature (`COMPOSITION`).
const OWN: &[Camera] = &[
    Camera {
        door: "files",
        take: files_taken,
    },
    Camera {
        door: "footer",
        take: footer_taken,
    },
];

/// THE CAMERA OF ITS OWN a door's word names — it takes the rest of the
/// words and ends the process.
fn camera(door: &str) -> Option<OwnCamera> {
    ronoctua_shell_door::photo::camera_named(CAMERAS, door).map(|c| c.take)
}

/// The file manager's camera, handed the flow of the vault the binary
/// knows (`crate::vault_flow`), which the files crate cannot name.
fn files_taken(words: &[String]) -> ! {
    ronoctua_shell_files::files_photograph::taken(words, crate::vault_flow::flow())
}

/// The footer's camera, handed what the binary composes (`COMPOSITION`).
fn footer_taken(words: &[String]) -> ! {
    ronoctua_shell_footer::footer_photograph::taken(words, &crate::COMPOSITION)
}

/// What the riders' words refuse once every word is read, `door` the rider
/// asked (`None`: the player's own door).
fn read(door: Option<&'static str>, asked: &Asked) -> Result<()> {
    // THE UPDATE WINDOW'S FIXTURES, on a door that reads none of them.
    ensure!(
        door == Some("update") || (!asked.given("--report") && !asked.given("--journal")),
        "`--report` and `--journal` are the update window's (`--door update`)"
    );
    Ok(())
}

/// What the riders' words refuse when the player's own door is drawn.
fn drawing(asked: &Asked) -> Result<()> {
    ensure!(
        !asked.given("--modeline") && !asked.given("--unwritable"),
        "`--modeline` and `--unwritable` are the footer's settings' (`--door footer-settings`)"
    );
    Ok(())
}

/// What a door that rides the walk refuses of the player's own words.
fn no_player_words(asked: &Asked) -> Result<()> {
    ensure!(
        asked.size().is_none() && asked.stage() == Stage::Off,
        "`--size` and `--stage` are the player's; this door takes neither"
    );
    Ok(())
}

/// THE UPDATE'S WINDOW over what the units would have written and
/// the journals said — the files given, never this machine's — and
/// a key that would start or answer something kept, not done
/// (`update_window::photographed`).
fn update(texts: ronoctua_i18n::Texts, scale: i32, asked: &Asked) -> Result<()> {
    no_player_words(asked)?;
    ensure!(
        !asked.given("--modeline") && !asked.given("--unwritable"),
        "`--modeline` and `--unwritable` are the footer's settings' (`--door footer-settings`)"
    );
    let halves = asked
        .values_of("--report")
        .into_iter()
        .map(|f| std::fs::read_to_string(f).with_context(|| format!("could not read `{f}`")))
        .collect::<Result<Vec<_>>>()?;
    let said = match asked.value_of("--journal") {
        Some(f) => std::fs::read_to_string(f).with_context(|| format!("could not read `{f}`"))?,
        None => String::new(),
    };
    let ui = Ui::detached(texts, (1, 1), scale)?;
    let content = ronoctua_shell_update::update_window::photographed(&halves, &said);
    shoot_saying(
        Detached::new(ui, content),
        asked,
        ronoctua_shell_update::update_window::UpdateWindow::would_have,
    )
}

/// The footer's settings, in their window, over the preferences
/// `--modeline` gave — never this machine's.
fn footer_settings(texts: ronoctua_i18n::Texts, scale: i32, asked: &Asked) -> Result<()> {
    no_player_words(asked)?;
    // A COPY IN A DIRECTORY OF ITS OWN, which is what a key
    // changes: the window writes the file it reads, and this
    // machine's `modeline.json` is the bars of the session the
    // photograph runs in.
    let dir = std::env::temp_dir().join(format!(
        "ronoctua-photograph-modeline-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("could not make `{}`", dir.display()))?;
    // AS IT IS, broken or not: a file that does not read as
    // written is a state of the window too — it opens over it and
    // says the sentence the file is owed (`adapt::at_start`).
    if let Some(file) = asked.value_of("--modeline") {
        let raw = std::fs::read(file).with_context(|| format!("could not read `{file}`"))?;
        std::fs::write(dir.join(ronoctua_shell_footer::adapt::FILE), raw)
            .with_context(|| format!("could not copy `{file}`"))?;
    }
    let lock = |mode: u32| {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode))
    };
    if asked.given("--unwritable") {
        // THE OWNER'S BUG, made here: a directory the person can
        // read and not write, as `/persist` made `~/.config/ronoctua`.
        lock(0o555).context("could not make the copy's directory read-only")?;
    }
    let store = ronoctua_store::Store::at(Some(dir.clone()), ronoctua_shell_footer::adapt::FILE);
    let ui = Ui::detached(texts, (1, 1), scale)?;
    let content = ronoctua_shell_footer::footer_settings_window::FooterSettingsWindow::over(store);
    let shot = shoot(Detached::new(ui, content), asked);
    let _ = lock(0o755);
    let _ = std::fs::remove_dir_all(&dir);
    shot
}

/// The calculator's window, over a history of its own.
fn calc(texts: ronoctua_i18n::Texts, scale: i32, asked: &Asked) -> Result<()> {
    no_player_words(asked)?;
    ensure!(
        !asked.given("--modeline") && !asked.given("--unwritable"),
        "`--modeline` and `--unwritable` are the footer's settings'"
    );
    // A HISTORY OF ITS OWN, in a directory of its own: what `RET`
    // keeps is written, and this machine's `calc.json` is the
    // person's.
    let dir = std::env::temp_dir().join(format!("ronoctua-photograph-calc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("could not make `{}`", dir.display()))?;
    let store = ronoctua_store::Store::at(Some(dir.clone()), ronoctua_shell_calc::calc::FILE);
    let ui = Ui::detached(texts, (1, 1), scale)?;
    let content = ronoctua_shell_calc::calc_window::CalcWindow::over(
        store,
        ronoctua_shell_calc::calc_window::Reach::Kept,
    );
    let shot = shoot(Detached::new(ui, content), asked);
    let _ = std::fs::remove_dir_all(&dir);
    shot
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_shell_player::player_photograph::said_past_the_door_with;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    /// `parse`, over the binary's doors.
    fn parse(words: &[String]) -> Result<Asked> {
        ronoctua_shell_player::player_photograph::parse(words, &OTHER_DOORS)
    }

    /// FAILS IF: a door the refusal names is none the camera takes — the
    /// player's two, a rider or a camera of its own — a rider is not named,
    /// or a door named is missing from the help: the list a person reads
    /// would send them to a door that is not there, or hide one that is.
    #[test]
    fn every_door_named_is_one_the_camera_takes() {
        let named: Vec<&str> = NAMED.split(", ").flat_map(|w| w.split(" and ")).collect();
        assert!(named.len() > RIDERS.len() + 2, "{named:?}");
        for door in &named {
            assert!(
                matches!(*door, "window" | "column")
                    || RIDERS.iter().any(|r| r.door == *door)
                    || camera(door).is_some(),
                "`{door}` is named, and no door takes it"
            );
            assert!(USAGE.contains(door), "`{door}` is not in the help");
        }
        for r in RIDERS {
            assert!(
                named.contains(&r.door),
                "`{}` rides and is not named",
                r.door
            );
        }
        assert!(camera("window").is_none() && camera("nowhere").is_none());
    }

    /// FAILS IF: a camera of a table is named nowhere a person reads — the
    /// refusal's list and the help — or two tables claim one door, the
    /// first listed taking the other's words (the binary's own first).
    #[test]
    fn every_camera_is_named_and_one_doors() {
        let named: Vec<&str> = NAMED.split(", ").flat_map(|w| w.split(" and ")).collect();
        let mut seen = std::collections::BTreeSet::new();
        for table in CAMERAS {
            for c in *table {
                assert!(
                    seen.insert(c.door),
                    "`{}` is the door of two cameras",
                    c.door
                );
                assert!(
                    named.contains(&c.door),
                    "`{}` has a camera and is not named",
                    c.door
                );
                assert!(
                    USAGE.contains(c.door),
                    "`{}` has a camera and is not in the help",
                    c.door
                );
            }
        }
        assert!(seen.len() >= 20, "{} cameras in the tables", seen.len());
        assert!(
            seen.contains("files") && seen.contains("footer"),
            "{seen:?}"
        );
    }

    /// FAILS IF: the update's window is not a door of this camera, its two
    /// fixtures are dropped or taken out of order, they are taken on a door
    /// that does not read them, or a key of its walk reaches the machine
    /// instead of being said.
    #[test]
    fn the_updates_window_is_a_door_and_what_it_would_ask_is_said() {
        let a = parse(&words(&[
            "--door",
            "update",
            "--report",
            "root.tsv",
            "--report",
            "person.tsv",
            "--journal",
            "said.txt",
            "--key",
            "u",
        ]))
        .expect("understood");
        assert_eq!(
            (a.door(), a.rider().map(|r| r.door)),
            (
                ronoctua_shell_player::player_window::Door::Window,
                Some("update")
            )
        );
        assert_eq!(a.values_of("--report"), ["root.tsv", "person.tsv"]);
        assert_eq!(a.value_of("--journal"), Some("said.txt"));
        for door in ["window", "footer-settings"] {
            let refused = parse(&words(&["--door", door, "--report", "root.tsv"]))
                .expect_err("a report on a door that reads none");
            assert!(refused.to_string().contains("update window's"), "{refused}");
        }

        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::detached(texts, (800, 600), 1).expect("a detached ui");
        let mut d = Detached::new(
            ui,
            ronoctua_shell_update::update_window::photographed(&[], ""),
        );
        d.press(ronoctua_keymap::Key::parse("u").expect("a key"));
        let said = said_past_the_door_with(
            &mut d,
            ronoctua_shell_update::update_window::UpdateWindow::would_have,
        )
        .expect("said");
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].starts_with("would have asked for a run of "),
            "{said:?}"
        );
    }
}
