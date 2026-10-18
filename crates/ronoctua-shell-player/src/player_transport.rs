//! The player's TRANSPORT: the buttons a window, the column and the footer's
//! media panel draw, the bar's remote control, and the footer's glyph on the
//! "Now" title — which glyph each one wears, whether it is shown and lit, and
//! the ONE mapping from a button to what it does ([`button_press`], then
//! [`button_errand`]), which every door reads.

use crate::player::{Errand, Press, Show, VOLUME_MOST, VOLUME_STEP};
use crate::player_page::TrackKind;
use crate::player_window::{Door, Stage};
use ronoctua_player::wire::{Ask, Repeat, State};
use ronoctua_tokens as tokens;

/// The transport's buttons, in the order they are drawn: the command each one
/// runs, which is also what the header says under the pointer.
///
/// ELEVEN, the swap LAST: at the small column eleven buttons of three cells
/// fill it to the last cell, with no clock and no room for a twelfth
/// (`player_window::fits`), and the swap is the one a narrow door gives up
/// first. The media in the file is drawn only while the file offers a
/// choice ([`shown`]), and the ones after it close up.
pub const BUTTONS: [&str; 11] = [
    "player-previous",
    "player-toggle",
    "player-next",
    "player-stop",
    "player-shuffle",
    "player-repeat",
    "player-tracks",
    "player-only",
    "player-pip",
    "player-maximize",
    "player-swap",
];

/// Whether a button is drawn in `s`: the media in the file only while there
/// is a choice among them that can be made here (`TrackKind::with_a_choice`),
/// and every other always.
#[must_use]
pub fn shown(id: &str, s: Option<&State>) -> bool {
    match id {
        "player-tracks" => s.is_some_and(|s| TrackKind::with_a_choice(s).next().is_some()),
        _ => true,
    }
}

/// The bar's buttons, one per line under the control line's air — previous,
/// play or pause and next; a line of air; louder, the mute and quieter; a
/// line of air; and the swap: the glyphs every player has, which is why the
/// bar says nothing about them. The ids are the transport's own, so a click
/// does what the transport's click does ([`button_errand`]).
pub const BAR_BUTTONS: [Option<&str>; 9] = [
    Some("player-previous"),
    Some("player-toggle"),
    Some("player-next"),
    None,
    Some("player-louder"),
    Some("player-mute"),
    Some("player-quieter"),
    None,
    Some("player-swap"),
];

/// Where each of the bar's buttons goes, in a tenant area whose first line is
/// `top`: the first a line under it, and one per line.
pub fn bar_buttons(top: u16) -> impl Iterator<Item = (u16, &'static str)> {
    (top + 1..)
        .zip(BAR_BUTTONS)
        .filter_map(|(line, id)| id.map(|id| (line, id)))
}

/// Each button's glyph, from the state, how the picture is shown and which
/// door it is drawn in.
#[must_use]
pub fn glyph_of(id: &str, s: Option<&State>, stage: Stage, door: Door) -> char {
    // PLAY OR PAUSE says what a press does — for the ambient sounds while
    // they are all that plays, whose toggle it is then (`player::toggle_ask`).
    let playing = s.is_some_and(|s| {
        if crate::player::only_ambience(s) {
            !s.ambience.paused
        } else {
            s.playing
        }
    });
    match id {
        "player-only" => tokens::icon::PLAYER_ONLY,
        "player-pip" => tokens::icon::PLAYER_PIP,
        "player-mute" if s.is_some_and(|s| s.muted) => tokens::icon::VOLUME_OFF,
        "player-mute" => tokens::icon::VOLUME,
        "player-louder" => tokens::icon::PLAYER_LOUDER,
        "player-quieter" => tokens::icon::PLAYER_QUIETER,
        // WHERE IT GOES: the column from a window, the window from the column
        // and from the footer.
        "player-swap" => match door {
            Door::Window => tokens::icon::PLAYER_COLUMN,
            Door::Column | Door::Footer => tokens::icon::PLAYER_WINDOW,
        },
        "player-tracks" => tokens::icon::PLAYER_TRACKS,
        // WHERE IT GOES, as the file manager's pair: out when on the screen.
        "player-maximize" if stage == Stage::Maximized => tokens::icon::THEATRE_OFF,
        "player-maximize" => tokens::icon::THEATRE,
        "player-previous" => tokens::icon::PLAYER_PREVIOUS,
        "player-toggle" if playing => tokens::icon::PAUSE,
        "player-toggle" => tokens::icon::PLAY,
        "player-next" => tokens::icon::PLAYER_NEXT,
        "player-stop" => tokens::icon::PLAYER_STOP,
        "player-shuffle" if s.is_some_and(|s| s.shuffle) => tokens::icon::PLAYER_SHUFFLE,
        "player-shuffle" => tokens::icon::PLAYER_SHUFFLE_OFF,
        "player-repeat" => match s.map_or(Repeat::Nothing, |s| s.repeat) {
            Repeat::Nothing => tokens::icon::PLAYER_REPEAT_OFF,
            Repeat::Queue => tokens::icon::PLAYER_REPEAT,
            Repeat::One => tokens::icon::PLAYER_REPEAT_ONE,
        },
        // THE SEEKS AROUND THE CLOCK (`player_media::clock_group`), never
        // among the eleven: ten seconds back and ahead.
        "player-rewind" => tokens::icon::PLAYER_REWIND,
        "player-forward" => tokens::icon::PLAYER_FORWARD,
        _ => ' ',
    }
}

/// Whether a button's glyph is drawn lit: a switch that is on.
#[must_use]
pub fn lit(id: &str, s: Option<&State>, stage: Stage) -> bool {
    match id {
        "player-only" => stage == Stage::Only,
        "player-pip" => stage == Stage::Small,
        "player-shuffle" => s.is_some_and(|s| s.shuffle),
        "player-repeat" => s.is_some_and(|s| s.repeat != Repeat::Nothing),
        _ => false,
    }
}

/// The volume one step up, or down: [`VOLUME_STEP`], and never past
/// [`VOLUME_MOST`] or under nothing — the ONE arithmetic of the view's `-`
/// and `=`, the bar's two buttons and a volume row's arrows.
#[must_use]
pub const fn stepped_volume(now: u8, up: bool) -> u8 {
    if up {
        let louder = now.saturating_add(VOLUME_STEP);
        if louder > VOLUME_MOST {
            VOLUME_MOST
        } else {
            louder
        }
    } else {
        now.saturating_sub(VOLUME_STEP)
    }
}

/// A step of the volume from what the player said it is — and with NO
/// STATE, no step: a volume guessed from nothing was sent as the new one and
/// the player kept it, so a person at 20% who pressed "quieter" before the
/// first state arrived played next at 95%. The door says the player is not
/// answering instead, which is what having no state means.
#[must_use]
pub fn volume_step(s: Option<&State>, up: bool, texts: &ronoctua_i18n::Texts) -> Errand {
    s.map_or_else(
        || Errand::Said(texts.t(crate::key::PLAYER_NOT_RUNNING)),
        |s| {
            Errand::Tell(Ask::Volume {
                percent: stepped_volume(s.volume, up),
            })
        },
    )
}

/// The button that is a KEY OF THE VIEW, by its command: the media in the
/// file opens a page (`Press::Tracks`), which only the view can do — so a
/// door hands it to its view's `key`, and every other button to
/// [`button_errand`]. Asked first, by every door.
#[must_use]
pub fn button_press(id: &str) -> Option<Press> {
    match id {
        "player-tracks" => Some(Press::Tracks),
        _ => None,
    }
}

/// What a BUTTON of the player does, by its command — the ONE mapping every
/// door reads: the window's and the column's transport, the column's bar,
/// the footer's media panel and its glyph on the "Now" title. What the
/// player is asked is `Tell`; a stage and the swap are the door's (`Show`,
/// `Swap`); a key of the view is [`button_press`]'s, and an id that is no
/// button does nothing.
#[must_use]
pub fn button_errand(id: &str, s: Option<&State>, texts: &ronoctua_i18n::Texts) -> Errand {
    match id {
        "player-previous" => Errand::Tell(Ask::Previous),
        "player-toggle" => Errand::Tell(crate::player::toggle_ask(s)),
        "player-next" => Errand::Tell(Ask::Next),
        "player-stop" => Errand::Tell(Ask::Stop),
        "player-shuffle" => Errand::Tell(Ask::Shuffle),
        "player-repeat" => Errand::Tell(Ask::Repeat),
        "player-mute" => Errand::Tell(Ask::Mute),
        "player-louder" => volume_step(s, true, texts),
        "player-quieter" => volume_step(s, false, texts),
        "player-only" => Errand::Show(Show::Only),
        "player-maximize" => Errand::Show(Show::Maximized),
        "player-pip" => Errand::Show(Show::Small),
        "player-swap" => Errand::Swap,
        "player-rewind" => seek_errand(false, texts),
        "player-forward" => seek_errand(true, texts),
        _ => Errand::Nothing,
    }
}

/// TEN SECONDS BACK OR AHEAD — the seeks beside the clock, `j` and `l`, and
/// a double click on a side of the picture — and the sentence that says it,
/// which goes again after the footer's seconds (`Ui::say`).
#[must_use]
pub fn seek_errand(ahead: bool, texts: &ronoctua_i18n::Texts) -> Errand {
    let step = crate::player::SEEK_STEP;
    Errand::TellSaying(
        Ask::Seek {
            seconds: if ahead { step } else { -step },
            relative: true,
        },
        seek_said(ahead, texts),
    )
}

/// What a seek of ten seconds says: `−10 s`, `+10 s`.
#[must_use]
pub fn seek_said(ahead: bool, texts: &ronoctua_i18n::Texts) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let seconds = crate::player::SEEK_STEP.round() as i64;
    texts.t_args(
        if ahead {
            crate::key::PLAYER_SEEKED_AHEAD
        } else {
            crate::key::PLAYER_SEEKED_BACK
        },
        &[("seconds", ronoctua_i18n::Arg::Number(seconds))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a transport button stops doing something — asking the
    /// player, or the door's own stage or swap — has no glyph in either
    /// door, or is not a command of the registry.
    #[test]
    fn every_button_asks_the_player_something_and_has_a_command() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        // THE VOLUME'S GLYPH is a button too, on the time bar's line.
        for id in BUTTONS.into_iter().chain(["player-mute"]) {
            // THE MEDIA IN THE FILE IS A KEY OF THE VIEW: it opens a page,
            // and the door hands it to the view.
            if let Some(press) = button_press(id) {
                assert_eq!(id, "player-tracks");
                assert_eq!(crate::player::press_of(id), Some(press));
                assert_eq!(button_errand(id, None, &texts), Errand::Nothing);
            } else {
                // THE THREE STAGE BUTTONS AND THE SWAP ARE THE DOOR'S, not
                // the player's: they change how the picture is shown, or the
                // door, and nothing is asked.
                let errand = button_errand(id, None, &texts);
                let the_door_s = matches!(id, "player-only" | "player-pip" | "player-maximize");
                match errand {
                    Errand::Tell(_) => assert!(!the_door_s && id != "player-swap", "{id}"),
                    Errand::Show(_) => assert!(the_door_s, "{id}"),
                    Errand::Swap => assert_eq!(id, "player-swap"),
                    other => panic!("{id} does {other:?}"),
                }
            }
            for door in [Door::Window, Door::Column, Door::Footer] {
                assert_ne!(glyph_of(id, None, Stage::Off, door), ' ', "{id}");
            }
            assert!(
                crate::player::KEYS.commands.iter().any(|c| c.id == id),
                "{id} is not a command of the registry"
            );
        }
        // THE SWAP IS DRAWN AS WHERE IT GOES.
        assert_eq!(
            glyph_of("player-swap", None, Stage::Off, Door::Window),
            tokens::icon::PLAYER_COLUMN
        );
        assert_eq!(
            glyph_of("player-swap", None, Stage::Off, Door::Column),
            tokens::icon::PLAYER_WINDOW
        );
        assert_eq!(
            glyph_of("player-swap", None, Stage::Off, Door::Footer),
            tokens::icon::PLAYER_WINDOW
        );
    }

    /// FAILS IF: a seek beside the clock stops asking the player ten
    /// seconds back or ahead — saying so — or has no glyph in a door, or is
    /// no command of the registry; or it becomes one of the eleven, which
    /// fill the small column to its last cell. And the commands' sentences
    /// say TEN seconds: the step moving under them is a sentence lying.
    #[test]
    fn the_seeks_beside_the_clock_ask_ten_seconds() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        assert!((crate::player::SEEK_STEP - 10.0).abs() < f64::EPSILON);
        for (id, seconds) in [("player-rewind", -10.0), ("player-forward", 10.0)] {
            assert!(!BUTTONS.contains(&id), "{id} is one of the eleven");
            match button_errand(id, None, &texts) {
                Errand::TellSaying(
                    Ask::Seek {
                        seconds: s,
                        relative: true,
                    },
                    said,
                ) => {
                    assert!((s - seconds).abs() < f64::EPSILON, "{id}: {s}");
                    assert!(said.contains("10"), "{id}: {said}");
                }
                other => panic!("{id} does {other:?}"),
            }
            for door in [Door::Window, Door::Column, Door::Footer] {
                assert_ne!(glyph_of(id, None, Stage::Off, door), ' ', "{id}");
            }
            assert!(
                crate::player::KEYS.commands.iter().any(|c| c.id == id),
                "{id} is not a command of the registry"
            );
        }
    }

    /// FAILS IF: the media's glyph is drawn while the file offers no choice
    /// — one sound and no picture, or nothing loaded — or is hidden while it
    /// does (a picture always offers the subtitles a file can lay over it),
    /// or another button starts coming and going.
    #[test]
    fn the_media_glyph_is_shown_only_with_a_choice() {
        use crate::player::fixture::{alone, track};
        use ronoctua_player::wire::TrackKind as Kind;
        let of = |kinds: &[(i64, Kind)]| {
            alone(
                "/v/a.mkv",
                kinds
                    .iter()
                    .map(|(id, kind)| track(*id, *kind, None, *id == 1))
                    .collect(),
            )
        };
        assert!(!shown("player-tracks", None));
        assert!(!shown("player-tracks", Some(&of(&[(1, Kind::Audio)]))));
        assert!(shown(
            "player-tracks",
            Some(&of(&[(1, Kind::Video), (1, Kind::Audio)]))
        ));
        assert!(shown(
            "player-tracks",
            Some(&of(&[(1, Kind::Video), (1, Kind::Audio), (2, Kind::Audio)]))
        ));
        assert!(shown(
            "player-tracks",
            Some(&of(&[(1, Kind::Video), (1, Kind::Audio), (1, Kind::Sub)]))
        ));
        for id in BUTTONS.into_iter().filter(|id| *id != "player-tracks") {
            assert!(shown(id, None), "{id}");
        }
    }

    /// FAILS IF: the bar's buttons stop being one per line from the line
    /// under the control line's air — the transport, a line of air, the
    /// volume's three, a line of air and the swap — or one of them is not a
    /// transport button: a glyph, something done and a command.
    #[test]
    fn the_bar_is_one_button_a_line() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let state = State::default();
        let lines: Vec<(u16, &str)> = bar_buttons(1).collect();
        assert_eq!(
            lines,
            [
                (2, "player-previous"),
                (3, "player-toggle"),
                (4, "player-next"),
                (6, "player-louder"),
                (7, "player-mute"),
                (8, "player-quieter"),
                (10, "player-swap"),
            ]
        );
        for (_, id) in lines {
            assert!(
                matches!(
                    button_errand(id, Some(&state), &texts),
                    Errand::Tell(_) | Errand::Swap
                ),
                "{id} does nothing"
            );
            assert_ne!(glyph_of(id, None, Stage::Off, Door::Column), ' ', "{id}");
            assert!(
                crate::player::KEYS.commands.iter().any(|c| c.id == id),
                "{id} is not a command of the registry"
            );
        }
    }
}
