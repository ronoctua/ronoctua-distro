//! What the compositor remembers — `niri-display.kdl`.
//!
//! # Why a file at all, when the IPC already changes things
//!
//! Because niri's IPC change is TRANSIENT by its own design: the next time it
//! reads its configuration it drops every change made that way
//! (`niri.rs`, "transient changes dropped"). So the IPC is the preview and
//! this file is the memory.
//!
//! AND WRITING IT IS NOT THE SAME AS APPLYING IT. niri watches the files it
//! includes and reloads on its own, but it PRESERVES a transient output
//! change whenever the configuration it re-reads is unchanged from the last
//! disk load (`src/niri.rs`). So the reload cannot be relied on to undo a
//! preview — [`crate::Eyes`] says what this file means over the IPC, knob by
//! knob, straight after writing it. This file is the memory; that is the
//! voice.
//!
//! The system's `config.kdl` ends with
//! `include ".../niri-display.kdl" optional=true`, exactly as it already does
//! for the theme's fragment. Absent, niri drives every monitor its own way,
//! which is the right default for a machine that has never been told anything.
//!
//! # A monitor is named by what it says about itself
//!
//! `output "Maker Model-1 Unknown"`, not `output "eDP-1"`. See
//! [`ronoctua_niri::Output::identity`]: the description follows the monitor
//! to another port and the connector does not, and it is the difference
//! between settings that survive plugging into the other side of a dock and
//! settings that follow whichever cable is in socket one.
//!
//! # The one ordering rule
//!
//! niri's `Outputs::find` takes the FIRST block whose name matches, so a
//! second block for the same monitor earlier in the configuration would win
//! over this one. The system's static configuration has no `output` node at
//! all, and a check refuses one appearing.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ronoctua_niri::Transform;
use serde::{Deserialize, Serialize};

/// The file's name, under `$XDG_CONFIG_HOME/ronoctua/`.
pub const FILE: &str = "niri-display.kdl";

/// The smallest scale niri will read. Its own field is `FloatOrInt<0, 10>`,
/// and it clamps to `0.1` when it applies one.
pub const SCALE_FLOOR: f64 = 0.1;

/// The largest. Above it the file does not parse at all.
pub const SCALE_CEILING: f64 = 10.0;

/// A scale niri will read, or nothing.
///
/// TWO WAYS TO FAIL AND ONLY ONE OF THEM IS A RANGE. `clamp` tames the
/// infinities (`+inf` becomes 10, `-inf` becomes 0.1) and hands NaN straight
/// back, because that is what `f64::clamp` does with it; `format!` then writes
/// `scale NaN`, which niri's `FloatOrInt` does not recognise as a number at
/// all and which fails the WHOLE load — every other monitor's settings gone
/// with it.
///
/// It used to be unreachable by accident: the JSON half was written first and
/// `serde_json` refuses a non-finite float, so the KDL never got written. Then
/// the order of the two writes changed for a different reason and the hole
/// opened. Guarding here means it cannot depend on which file goes first.
#[must_use]
pub fn readable_scale(scale: Option<f64>) -> Option<f64> {
    scale
        .filter(|s| s.is_finite())
        .map(|s| s.clamp(SCALE_FLOOR, SCALE_CEILING))
}

/// A mode somebody chose.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Chosen {
    /// Width in physical pixels.
    pub width: u16,
    /// Height in physical pixels.
    pub height: u16,
    /// The rate in hertz — the EXACT one, which is
    /// [`ronoctua_niri::Mode::rate`] and never `Mode::hertz`. niri matches a
    /// rate by turning it back into an integer of millihertz and comparing
    /// for equality, so the rounded one names no mode at all and niri falls
    /// back to the preferred one while answering that it applied the change.
    /// The field is called `rate` for that reason: `hertz` here would make
    /// `Mode::hertz` the obvious thing to put in it.
    ///
    /// `None` lets niri pick among the rates this size offers — which is what
    /// somebody who changed only the resolution meant.
    pub rate: Option<f64>,
}

impl From<ronoctua_niri::Mode> for Chosen {
    /// The safe way to build one, and the only one that needs no thought.
    fn from(m: ronoctua_niri::Mode) -> Self {
        let asked = m.asked();
        Self {
            width: asked.width,
            height: asked.height,
            rate: asked.refresh,
        }
    }
}

/// Where a monitor sits, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spot {
    /// Logical X of the top-left corner.
    pub x: i32,
    /// Logical Y of the top-left corner.
    pub y: i32,
}

/// How the refresh rate is allowed to vary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vary {
    /// Always, which costs battery on a laptop.
    Always,
    /// Only while a window that asked for it is on this monitor.
    OnDemand,
}

/// Everything somebody chose for one monitor.
///
/// Every field's default is "let niri decide", so a monitor nobody has
/// touched writes no block at all — and a preference file is a list of
/// decisions rather than a photograph of the machine.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Wanted {
    /// Stop driving this monitor.
    pub off: bool,
    /// The mode, or `None` for niri's choice.
    pub mode: Option<Chosen>,
    /// The scale, or `None` for niri's choice from the physical size.
    pub scale: Option<f64>,
    /// How the picture is turned.
    pub transform: Transform,
    /// Where it sits, or `None` for niri's arrangement.
    pub position: Option<Spot>,
    /// Whether the refresh rate varies.
    pub vary: Option<Vary>,
    /// Whether the session opens focused here.
    pub first: bool,
}

impl Wanted {
    /// Whether anything was chosen at all.
    #[must_use]
    pub fn says_nothing(&self) -> bool {
        *self == Self::default()
    }
}

/// The whole fragment, for the monitors that have something to say.
///
/// The key is [`ronoctua_niri::Output::identity`].
#[must_use]
pub fn kdl_of(screens: &BTreeMap<String, Wanted>) -> String {
    let mut out = String::from(
        "// GENERATED by ronoctua-display. Do not edit — the display view rewrites it.\n\
         // Included by the system's niri config with `optional=true`. It is what\n\
         // survives a restart; what makes the running compositor agree with it is\n\
         // the display view saying so over the IPC, knob by knob, right after\n\
         // writing this. A reload alone does not: niri keeps a transient change\n\
         // whenever the configuration it re-reads is unchanged from the last one.\n",
    );
    // THE MORE SPECIFIC BLOCK GOES FIRST, AND THAT ORDERING IS LOAD-BEARING.
    //
    // niri's `OutputName::matches` answers true for EITHER the connector or
    // the description of the same output, and `Outputs::find` takes the FIRST
    // block that matches (`niri-config/src/output.rs`; `modify_output_config`
    // uses `find_mut` the same way, so the IPC path shadows identically).
    //
    // Our own file holds BOTH kinds of key, because that is what `Naming` is:
    // a monitor whose EDID carries a serial is filed under its description,
    // one that does not is filed under its connector. So two of our own blocks
    // can match one monitor, and left in `BTreeMap` order the winner is
    // whichever happens to spell earlier.
    //
    // What that cost: a monitor with no serial on `DP-1` is filed as "DP-1"
    // and turned off. It is unplugged, and a monitor that DOES give a serial
    // goes into the same port — filed under, say, "Zenith Y S9". "D" sorts
    // before "Z", niri finds the stale connector block first, and the new
    // monitor comes up DARK, while the view's row for it says it is on
    // (the view reads the description key). Nothing in the interface can
    // reach the stale block either: forgetting a key only works for a monitor
    // that is present under it.
    //
    // A description names ONE monitor; a connector names a PORT. The
    // description is the more specific claim, so it is the one that must win.
    //
    // The rule that separates them is that a description is
    // `"<make> <model> <serial>"` — `Output::identity`, three parts and always
    // two spaces, with the word `Unknown` where the monitor said nothing —
    // while a kernel connector name (`DP-1`, `eDP-1`, `HDMI-A-1`) has no space
    // in it and cannot grow one.
    let mut order: Vec<(&String, &Wanted)> = screens.iter().collect();
    order.sort_by(|(a, _), (b, _)| {
        let specific = |k: &str| !k.contains(' ');
        specific(a).cmp(&specific(b)).then_with(|| a.cmp(b))
    });
    for (identity, w) in order {
        if w.says_nothing() {
            continue;
        }
        let _ = writeln!(out, "output \"{}\" {{", escaped(identity));
        if w.off {
            // AND NOTHING ELSE MATTERS. A monitor that is off has no mode, no
            // place and no scale; writing them would be describing a picture
            // nobody is drawing.
            out.push_str("    off\n}\n");
            continue;
        }
        if let Some(m) = w.mode {
            let _ = match m.rate {
                // The rate is written with three decimals because a mode's
                // real rate is 59.951 and not 60: niri parses the string back
                // into a float and matches it against the monitor's list, so
                // a rounded number is a mode the monitor does not have.
                Some(hz) => writeln!(out, "    mode \"{}x{}@{hz:.3}\"", m.width, m.height),
                None => writeln!(out, "    mode \"{}x{}\"", m.width, m.height),
            };
        }
        // CLAMPED AND SIEVED, because niri's field is `FloatOrInt<0, 10>`:
        // a value outside the range is a decode error and a NaN is not a
        // number at all, and either one fails the whole load and leaves every
        // other monitor's settings unapplied. Same reason the name is escaped.
        if let Some(s) = readable_scale(w.scale) {
            let _ = writeln!(out, "    scale {s}");
        }
        if w.transform != Transform::Normal {
            // `kdl()` and NOT the serde name: the config file and the wire
            // spell these differently.
            let _ = writeln!(out, "    transform \"{}\"", w.transform.kdl());
        }
        if let Some(p) = w.position {
            let _ = writeln!(out, "    position x={} y={}", p.x, p.y);
        }
        match w.vary {
            Some(Vary::Always) => out.push_str("    variable-refresh-rate\n"),
            Some(Vary::OnDemand) => out.push_str("    variable-refresh-rate on-demand=true\n"),
            None => {}
        }
        if w.first {
            out.push_str("    focus-at-startup\n");
        }
        out.push_str("}\n");
    }
    out
}

/// A monitor's name, safe inside a KDL string.
///
/// A model name with a quote in it is not something anybody has seen, and it
/// is also not something to find out about by writing a configuration file
/// the compositor cannot parse — which would take the reload down and leave
/// every OTHER monitor's settings unapplied.
fn escaped(name: &str) -> String {
    name.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(w: Wanted) -> String {
        kdl_of(&BTreeMap::from([("Maker Model-1 Unknown".to_owned(), w)]))
    }

    #[test]
    fn a_monitor_nobody_touched_writes_no_block() {
        // The file is a list of DECISIONS, not a photograph of the machine.
        // Writing every monitor out would mean a fragment that changes on
        // every hotplug, and a niri reload each time.
        let quiet = kdl_of(&BTreeMap::from([
            ("Maker Model-1 Unknown".to_owned(), Wanted::default()),
            ("Other Model-2 ABC123".to_owned(), Wanted::default()),
        ]));
        assert!(!quiet.contains("output"), "{quiet}");
    }

    #[test]
    fn a_rate_keeps_its_decimals() {
        // A mode's real rate is 59.951. Rounded to 60 it is a mode the
        // monitor does not have, and niri falls back to its own choice with
        // nothing on screen saying why.
        let out = one(Wanted {
            mode: Some(Chosen {
                width: 1920,
                height: 1200,
                rate: Some(59.951),
            }),
            ..Wanted::default()
        });
        assert!(out.contains(r#"mode "1920x1200@59.951""#), "{out}");
    }

    #[test]
    fn a_resolution_with_no_rate_lets_niri_choose_one() {
        // Which is what somebody who changed only the resolution meant.
        let out = one(Wanted {
            mode: Some(Chosen {
                width: 1280,
                height: 720,
                rate: None,
            }),
            ..Wanted::default()
        });
        assert!(out.contains(r#"mode "1280x720""#), "{out}");
        assert!(!out.contains('@'), "{out}");
    }

    #[test]
    fn a_transform_is_written_the_way_the_config_spells_it() {
        // Not the way the wire spells it. `Flipped90` in KDL is a parse
        // error, and a parse error in an included file takes the whole
        // reload down — so every OTHER monitor loses its settings too.
        let out = one(Wanted {
            transform: Transform::Flipped90,
            ..Wanted::default()
        });
        assert!(out.contains(r#"transform "flipped-90""#), "{out}");
        assert!(!out.contains("Flipped90"), "{out}");
        // And the identity transform is not written: it is the default, and
        // a file that spells out defaults is a file nobody can read.
        let plain = one(Wanted {
            first: true,
            ..Wanted::default()
        });
        assert!(!plain.contains("transform"), "{plain}");
    }

    #[test]
    fn a_monitor_that_is_off_says_only_that() {
        // It has no mode, no place and no scale. Writing them would describe
        // a picture nobody is drawing.
        let out = one(Wanted {
            off: true,
            mode: Some(Chosen {
                width: 1920,
                height: 1080,
                rate: None,
            }),
            scale: Some(2.0),
            position: Some(Spot { x: 100, y: 0 }),
            ..Wanted::default()
        });
        assert!(out.contains("    off\n"), "{out}");
        assert!(!out.contains("mode"), "{out}");
        assert!(!out.contains("scale"), "{out}");
        assert!(!out.contains("position"), "{out}");
    }

    #[test]
    fn a_quote_in_a_model_name_cannot_break_the_file() {
        // A parse error in an included fragment takes the reload down and
        // leaves every monitor unapplied — so this is not about tidiness.
        let out = kdl_of(&BTreeMap::from([(
            r#"Acme 27" Unknown"#.to_owned(),
            Wanted {
                first: true,
                ..Wanted::default()
            },
        )]));
        assert!(out.contains(r#"output "Acme 27\" Unknown""#), "{out}");
    }

    #[test]
    fn the_whole_grammar_comes_out_at_once() {
        // EVERY NODE THIS WRITER CAN EMIT, in one place a person can read
        // beside niri's own grammar — and it is not only for reading: the
        // `display-fragment-validates` check lifts this very text out of the
        // source and feeds it to a real `niri validate`, so what is pinned
        // here is what the compositor is asked to accept.
        //
        // TWO MONITORS, because `off` is the one node that excludes the
        // others and would otherwise never appear in the pinned text.
        let both = kdl_of(&BTreeMap::from([
            (
                "Maker Model-1 Unknown".to_owned(),
                Wanted {
                    off: false,
                    mode: Some(Chosen {
                        width: 3840,
                        height: 2160,
                        rate: Some(59.997),
                    }),
                    scale: Some(1.5),
                    transform: Transform::R270,
                    position: Some(Spot { x: -1920, y: 120 }),
                    vary: Some(Vary::OnDemand),
                    first: true,
                },
            ),
            (
                "Other Model-2 ABC123".to_owned(),
                Wanted {
                    off: true,
                    ..Wanted::default()
                },
            ),
        ]));
        let pinned = &both[both.find("output ").unwrap()..];
        assert_eq!(
            pinned.trim_end(),
            r#"output "Maker Model-1 Unknown" {
    mode "3840x2160@59.997"
    scale 1.5
    transform "270"
    position x=-1920 y=120
    variable-refresh-rate on-demand=true
    focus-at-startup
}
output "Other Model-2 ABC123" {
    off
}"#
        );

        // And the single-monitor shape, unchanged.
        let out = one(Wanted {
            off: false,
            mode: Some(Chosen {
                width: 3840,
                height: 2160,
                rate: Some(59.997),
            }),
            scale: Some(1.5),
            transform: Transform::R270,
            position: Some(Spot { x: -1920, y: 120 }),
            vary: Some(Vary::OnDemand),
            first: true,
        });
        // The header is prose and its length is not the subject here, so the
        // body is taken from the first node rather than by counting lines.
        let body = out[out.find("output ").unwrap()..].trim_end().to_owned();
        assert_eq!(
            body,
            r#"output "Maker Model-1 Unknown" {
    mode "3840x2160@59.997"
    scale 1.5
    transform "270"
    position x=-1920 y=120
    variable-refresh-rate on-demand=true
    focus-at-startup
}"#
        );
    }

    #[test]
    fn a_scale_outside_niris_range_does_not_take_the_file_down() {
        // `FloatOrInt<0, 10>` REFUSES rather than clamps, and a refusal in an
        // included fragment fails the whole load — so one bad number would
        // leave every other monitor unapplied.
        for (asked, written) in [(0.0, "0.1"), (-4.0, "0.1"), (99.0, "10"), (1.5, "1.5")] {
            let out = one(Wanted {
                scale: Some(asked),
                ..Wanted::default()
            });
            assert!(
                out.contains(&format!("    scale {written}\n")),
                "{asked} came out as: {out}"
            );
        }
        // AND WHAT IS NOT A NUMBER IS NOT WRITTEN AT ALL — NaN, and the two
        // infinities with it. `clamp` would turn `+inf` into a tidy `10`,
        // which is worse than dropping it: it honours a value nobody meant
        // and hides that something upstream produced nonsense. NaN it hands
        // straight back, `format!` spells it `NaN`, and niri's parser rejects
        // the whole file — every other monitor's settings gone with it.
        for mad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let out = one(Wanted {
                scale: Some(mad),
                ..Wanted::default()
            });
            assert!(!out.contains("scale"), "{mad} wrote: {out}");
            assert_eq!(readable_scale(Some(mad)), None, "{mad}");
        }
        // A finite one still lands, clamped.
        assert_eq!(readable_scale(Some(400.0)), Some(10.0));
        assert_eq!(readable_scale(None), None);
    }

    #[test]
    fn a_mode_is_built_from_the_conversion_that_survives() {
        // `Mode::hertz` rounds and names no mode; `Mode::rate` is exact. The
        // `From` impl is here so that nobody has to remember which.
        let m = ronoctua_niri::Mode {
            width: 1920,
            height: 1200,
            refresh_rate: 59_951,
            is_preferred: true,
        };
        let chosen = Chosen::from(m);
        assert_eq!(chosen.rate, Some(59.951));
        let out = one(Wanted {
            mode: Some(chosen),
            ..Wanted::default()
        });
        assert!(out.contains(r#"mode "1920x1200@59.951""#), "{out}");
    }

    #[test]
    fn always_varying_is_the_bare_node() {
        let out = one(Wanted {
            vary: Some(Vary::Always),
            ..Wanted::default()
        });
        assert!(out.contains("    variable-refresh-rate\n"), "{out}");
        assert!(!out.contains("on-demand"), "{out}");
    }

    #[test]
    fn a_preference_survives_the_disk() {
        // It is stored as JSON and read back; a field that does not
        // round-trip is a setting that quietly resets on the next boot.
        let w = Wanted {
            off: false,
            mode: Some(Chosen {
                width: 1920,
                height: 1200,
                rate: Some(59.951),
            }),
            scale: Some(1.25),
            transform: Transform::Flipped180,
            position: Some(Spot { x: 0, y: -1080 }),
            vary: Some(Vary::Always),
            first: true,
        };
        let text = serde_json::to_string(&w).unwrap();
        assert_eq!(serde_json::from_str::<Wanted>(&text).unwrap(), w);
        // And a file written by an older version, missing every field, reads
        // as "nothing was chosen" rather than failing.
        assert!(serde_json::from_str::<Wanted>("{}").unwrap().says_nothing());
    }

    #[test]
    fn a_description_block_is_written_before_a_connector_block() {
        // TWO OF OUR OWN BLOCKS CAN MATCH ONE MONITOR, because niri matches an
        // output by its connector OR its description and takes the FIRST that
        // matches. A stale connector block that sorts earlier than the current
        // monitor's description block therefore decides — and if it says `off`,
        // the screen is black with nothing in the view able to reach it.
        let screens = BTreeMap::from([
            (
                "DP-1".to_owned(),
                Wanted {
                    off: true,
                    ..Wanted::default()
                },
            ),
            (
                "Zenith Y S9".to_owned(),
                Wanted {
                    scale: Some(2.0),
                    ..Wanted::default()
                },
            ),
        ]);
        let kdl = kdl_of(&screens);
        let described = kdl
            .find("Zenith Y S9")
            .expect("the description block is missing");
        let connector = kdl
            .find("\"DP-1\"")
            .expect("the connector block is missing");
        assert!(
            described < connector,
            "the connector block comes first, so a monitor that has both \
             matches the PORT's stale setting instead of its own:\n{kdl}",
        );
    }
}
