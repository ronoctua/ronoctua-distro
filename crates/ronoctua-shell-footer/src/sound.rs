//! The line the volume — and a screen's brightness — flashes on.
//!
//! # Where it goes
//!
//! ON THE MINIBUFFER'S LINE when that line is free (no view, no echo, no
//! leader, no drawer), which costs the footer no height; on the row above the
//! modeline otherwise. Decided once, when the key is pressed
//! (`Footer::flash_the_sound`), so a held key never moves the modeline.
//!
//! # Why it is the glance's row and not a new one
//!
//! Because there is one row there. It sits between the notification band and
//! the modeline — above the modeline for the band's own reason (the modeline
//! is the one line that says where you are), and below the band because a
//! notification arrives on its own schedule and must not push a line somebody
//! is reading.
//!
//! A second row of the same kind would be a second term in six arithmetic
//! functions of `surface`, a second thing that is not in the expansion's
//! budget, and a second countdown for a person to tell apart. So the row has
//! two TENANTS and one arithmetic, and the last thing asked for holds it.
//!
//! # Why the countdown is literally the glance's
//!
//! [`crate::world::Showing`] carries it — when it went up, how long it has,
//! and whether a hand is on it — and the pie is `band::countdown_glyph`, the
//! notification band's own. Three countdowns on one footer that behaved
//! differently would be three things to learn about a machine that has one.
//!
//! # What it draws
//!
//! The device, a bar, and the number. On a screen too narrow for all three,
//! `layout::resolve` drops the device first and the bar second: the number is
//! what was asked for, and half a bar is worse than no bar.

use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// How many cells the bar gets on this line.
///
/// Wider than the view's, because here it is the whole drawing rather than one
/// column of a row.
const BAR_CELLS: u16 = 14;

/// What the line is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The device, in its own words.
    pub device: String,
    /// How loud it is, as a person reads it.
    pub percent: u8,
    /// Whether it is silent, in which case the number says nothing.
    pub muted: bool,
    /// What the number is about.
    pub what: Source,
}

/// What a flashed number is about.
///
/// A SCREEN rides the same line: the brightness keys sit beside the volume
/// keys, are pressed the same way, and answered nowhere until this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The output, a speaker or a headset.
    Speaker,
    /// The input.
    Microphone,
    /// A screen's light.
    Screen,
}

/// The line, placed and cut.
#[must_use]
pub fn line(
    said: &Said,
    columns: u16,
    showing: Option<crate::world::Showing>,
    texts: &ronoctua_i18n::Texts,
) -> crate::world::Drawn {
    // THE PIE'S CELLS COME OFF THE BUDGET FIRST — the glance's rule, and for
    // the same reason: taken afterwards, a full line would draw its last piece
    // under the glyph.
    let budget = columns.saturating_sub(crate::world::PIE_CELLS);

    let glyph = match said.what {
        Source::Microphone => tokens::icon::MICROPHONE,
        Source::Screen => tokens::icon::BRIGHTNESS,
        Source::Speaker if said.muted => tokens::icon::VOLUME_MUTE,
        Source::Speaker => tokens::icon::VOLUME,
    };
    let device = format!("{glyph} {}", said.device);
    let number = if said.muted {
        texts.t(crate::key::SOUND_MUTED)
    } else {
        format!("{}%", said.percent)
    };
    // A MUTED DEVICE DRAWS NO BAR. An empty bar and a device that is off read
    // the same, and only one of them comes back with a key.
    let bar = if said.muted {
        String::new()
    } else {
        crate::chrome::meter(said.percent, BAR_CELLS)
    };

    let mut segments = vec![
        layout::Segment {
            id: "sound-device",
            text: device,
            // THE ORDER THINGS GO IN, and it is not the order they are
            // written in. The NUMBER is the answer and never goes (0). The
            // DEVICE goes next-to-last, because with two sinks connected it
            // is the difference between the speaker and the headset somebody
            // forgot was on. The BAR goes FIRST, because it is the one piece
            // that says what the number already said.
            priority: 1,
            min_cols: 0,
            side: layout::Side::Left,
            shrink: layout::Shrink::AtTheEnd,
            removable: true,
        },
        layout::Segment {
            id: "sound-number",
            // AS WIDE AS IT IS, and never shrunk. A cut percentage is worse
            // than no percentage: `38%` cut to `3…` is a number that is not
            // true, and this line exists to say a number.
            min_cols: layout::width(&number),
            text: number,
            priority: 0,
            side: layout::Side::Left,
            shrink: layout::Shrink::Cut,
            removable: false,
        },
    ];
    if !bar.is_empty() {
        let width = layout::width(&bar);
        segments.insert(
            1,
            layout::Segment {
                id: "sound-bar",
                text: bar,
                priority: 2,
                // AS WIDE AS IT IS, so `resolve` drops it whole. Half a bar is
                // a lie about how loud something is.
                min_cols: width,
                side: layout::Side::Left,
                shrink: layout::Shrink::AtTheEnd,
                removable: true,
            },
        );
    }

    let mut drawn = crate::world::Drawn::default();
    for one in layout::resolve(budget, &segments) {
        drawn.pieces.push(crate::world::Piece {
            at: one.column,
            text: one.text.clone(),
            color: if one.id == "sound-device" {
                tokens::color::TEXT_SECONDARY
            } else {
                tokens::color::TEXT_PRIMARY
            },
            edge: false,
        });
    }

    // THE PIE, on the right edge, and it is the band's own glyph — the same
    // function the glance calls, so the two lines that share this row cannot
    // come to look different. It becomes a PAUSE while a hand is on the line.
    if let Some(showing) = showing {
        let glyph = if showing.held() {
            tokens::icon::PAUSE
        } else {
            crate::band::countdown_glyph(ronoctua_notify::eighths(
                showing.fraction_left(std::time::Instant::now()),
            ))
        };
        drawn.pieces.push(crate::world::Piece {
            // FROM THE BORDER — the glance's own placement, by the same
            // function, so the two tenants of this row cannot drift apart.
            at: crate::chrome::from_the_edge(columns, crate::world::PIE_GLYPH_CELLS),
            text: glyph.to_string(),
            color: tokens::color::NOTIFICATION_COUNTDOWN,
            edge: true,
        });
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn speaker(percent: u8) -> Said {
        Said {
            device: "Speaker".to_owned(),
            percent,
            muted: false,
            what: Source::Speaker,
        }
    }

    #[test]
    fn the_line_says_the_device_the_bar_and_the_number() {
        let d = line(&speaker(38), 80, None, &texts());
        let said: Vec<&str> = d.pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(said.len(), 3, "{said:?}");
        assert!(said[0].contains("Speaker"));
        assert_eq!(layout::width(said[1]), BAR_CELLS, "the bar is whole");
        assert_eq!(said[2], "38%");
    }

    #[test]
    fn the_number_is_the_last_thing_to_go() {
        // THE RULE THIS PINS is the ORDER things are dropped in. The number
        // is what was asked for; the bar is a second way of saying it; the
        // device is context. A narrow screen that kept the name and dropped
        // the number would answer a question nobody put.
        let t = texts();
        let wide = line(&speaker(38), 80, None, &t);
        assert_eq!(wide.pieces.len(), 3, "{:?}", wide.pieces);

        let middling = line(&speaker(38), 24, None, &t);
        assert!(
            !middling
                .pieces
                .iter()
                .any(|p| p.text.contains(tokens::mark::METER_EMPTY)),
            "the bar goes before the name, because the number already said it: {:?}",
            middling.pieces
        );
        assert!(
            middling.pieces.iter().any(|p| p.text.contains("Speaker")),
            "and the name stayed: {:?}",
            middling.pieces
        );

        let narrow = line(&speaker(38), 12, None, &t);
        assert_eq!(narrow.pieces.len(), 1, "{:?}", narrow.pieces);
        assert_eq!(narrow.pieces[0].text, "38%", "and it survives whole");

        // AND PAST THAT, THE SCREEN WINS — the line draws NOTHING rather than
        // running off the edge. `min_cols` and `removable` are both floors of
        // PREFERENCE; what never gives way is that nothing leaks. Four columns
        // leaves two after the countdown's cells, which is less than `38%`, so
        // there is no honest way to draw it and it is not drawn. That is the
        // modeline's own rule and this line does not get a second one.
        let absurd = line(&speaker(38), 4, None, &t);
        for p in &absurd.pieces {
            assert!(
                u32::from(p.at) + u32::from(layout::width(&p.text)) <= 4,
                "{p:?} runs off a four-column surface"
            );
        }
    }

    #[test]
    fn a_silent_device_draws_no_bar() {
        let d = line(
            &Said {
                muted: true,
                ..speaker(38)
            },
            80,
            None,
            &texts(),
        );
        let said: Vec<&str> = d.pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(said.len(), 2, "{said:?}");
        assert!(!said.iter().any(|t| t.contains('▮') || t.contains('▯')));
        assert_ne!(said[1], "38%", "the number gives way to the word");
    }

    #[test]
    fn the_microphone_says_so_with_its_own_glyph() {
        let speaking = line(&speaker(38), 80, None, &texts());
        let listening = line(
            &Said {
                what: Source::Microphone,
                ..speaker(38)
            },
            80,
            None,
            &texts(),
        );
        assert_ne!(speaking.pieces[0].text, listening.pieces[0].text);
        assert!(
            listening.pieces[0]
                .text
                .starts_with(tokens::icon::MICROPHONE)
        );
    }

    #[test]
    fn nothing_is_drawn_under_the_pie() {
        // The glance paid for this rule once: the budget has to lose the
        // countdown's cells BEFORE the pieces are placed, or the last one is
        // drawn beneath the glyph.
        let showing = crate::world::Showing::new(
            std::time::Instant::now(),
            std::time::Duration::from_secs(3),
        );
        let d = line(&speaker(100), 40, Some(showing), &texts());
        let pie = d.pieces.last().expect("the pie");
        assert_eq!(pie.at, 40 - crate::world::PIE_CELLS);
        for p in &d.pieces[..d.pieces.len() - 1] {
            assert!(
                p.at + layout::width(&p.text) <= pie.at,
                "{p:?} runs under the pie at {}",
                pie.at
            );
        }
    }

    #[test]
    fn a_hand_on_the_line_turns_the_pie_into_a_pause() {
        let now = std::time::Instant::now();
        let mut showing = crate::world::Showing::new(now, std::time::Duration::from_secs(3));
        let running = line(&speaker(50), 40, Some(showing), &texts());
        showing.hold(now);
        let held = line(&speaker(50), 40, Some(showing), &texts());
        assert_eq!(
            held.pieces.last().expect("the pie").text,
            tokens::icon::PAUSE.to_string()
        );
        assert_ne!(
            running.pieces.last().expect("the pie").text,
            held.pieces.last().expect("the pie").text
        );
    }
}
