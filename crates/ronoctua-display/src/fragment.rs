//! What the compositor remembers — `scroll-display.conf`.
//!
//! # Why a file at all, when the IPC already changes things
//!
//! Because an `output` command sent over the IPC lives in the RUNNING
//! configuration and reaches no file: the next re-read frees every one of them
//! (`load_main_config` rebuilds `config->output_configs` from scratch,
//! `sway/config.c`) and applies the file instead. So the IPC is the preview
//! and this file is the memory.
//!
//! AND WRITING IT IS NOT THE SAME AS APPLYING IT — for the opposite reason it
//! used to be. This compositor watches nothing: there is no inotify in it, so
//! a file written here has no effect at all until somebody asks for a re-read.
//! [`crate::Eyes`] is what asks, straight after writing. This file is the
//! memory; that call is the voice.
//!
//! That is also the whole of UNDO. Under the previous compositor the reload
//! PRESERVED a transient change whenever the configuration it re-read was
//! unchanged from the last disk load, so cancelling by rewriting the same
//! bytes preserved exactly what it meant to discard — and undo had to be six
//! explicit knobs per screen. Here one re-read discards every preview there
//! was.
//!
//! The system's config `include`s this file, exactly as it already does for
//! the theme's fragment, and a missing one is not fatal by construction —
//! `cmd_include` returns success when the path does not resolve. Absent, the
//! compositor drives every monitor its own way, which is the right default for
//! a machine that has never been told anything.
//!
//! # A monitor is named by what it says about itself
//!
//! `output "Maker Model-1 Unknown"`, not `output "eDP-1"`. See
//! [`ronoctua_compositor::Output::identity`]: the description follows the monitor
//! to another port and the connector does not, and it is the difference
//! between settings that survive plugging into the other side of a dock and
//! settings that follow whichever cable is in socket one.
//!
//! It is byte for byte what the compositor builds for the same monitor —
//! `"%s %s %s"` over make, model and serial, with `Unknown` for a part the
//! EDID does not carry (`output_get_identifier`, `sway/config/output.c`). Two
//! spellings of one identity would be settings that apply to nothing.
//!
//! # There is no ordering rule, and that is a property of the compositor
//!
//! Output blocks are MERGED rather than raced: `store_output_config` files
//! each one by name and `get_output_config` applies them `*` first, then the
//! connector, then the identifier, field by field with the later one winning
//! (`sway/config/output.c`). So a block of ours is the most specific there is
//! and wins by construction, whatever else is in the file and in whatever
//! order.
//!
//! The consequence is that `enable` has to be written EXPLICITLY. Under a
//! first-match compositor, silence in the most specific block meant the
//! defaults; here silence means whatever a less specific block said, and the
//! system config says `output * bg …`. A monitor turned off and then turned
//! back on is the case that proves it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ronoctua_compositor::Transform;
use serde::{Deserialize, Serialize};

/// The file's name, under `$XDG_CONFIG_HOME/ronoctua/`.
pub const FILE: &str = "scroll-display.conf";

/// The smallest scale this writer will put in the file.
///
/// IT IS OURS, and not the compositor's: `output_cmd_scale` is a bare
/// `strtof` with no range at all, and the only test applied afterwards is
/// `oc->scale > 0` (`sway/config/output.c`). A monitor at scale 0.01 is a
/// desktop of 100 by 100 logical pixels, and nothing below would refuse it.
pub const SCALE_FLOOR: f64 = 0.1;

/// The largest, and ours for the same reason. Above it a logical desktop is
/// smaller than a window.
pub const SCALE_CEILING: f64 = 10.0;

/// A scale worth writing down, or nothing.
///
/// TWO WAYS TO FAIL AND ONLY ONE OF THEM IS A RANGE. `clamp` would tame the
/// infinities (`+inf` becomes 10) and hand NaN straight back, because that is
/// what `f64::clamp` does with it — so the non-finite ones are dropped BEFORE
/// the clamp, and the file simply says nothing about that screen.
///
/// Dropping is right because `strtof` ACCEPTS both spellings. `scale nan`
/// parses, leaves nothing after the number, and returns success — and then
/// `oc->scale > 0` is false, so the line is silently ignored: a setting in the
/// file that does nothing, with nothing anywhere saying why. `scale inf`
/// parses too and passes that test, and `verify_good_scale` hands the infinity
/// through to wlroots. Neither is a parse error, which is what makes this
/// worse than it was: a refusal at least comes back in `reload`'s reply and
/// reaches the screen.
///
/// It used to be unreachable by accident: the JSON half was written first and
/// `serde_json` refuses a non-finite float, so the fragment never got written.
/// Then the order of the two writes changed for a different reason and the
/// hole opened. Guarding here means it cannot depend on which file goes
/// first.
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
    /// [`ronoctua_compositor::Mode::rate`] and never `Mode::hertz`.
    ///
    /// The compositor rounds it to millihertz and takes the NEAREST mode of
    /// that size (`set_mode`, `sway/config/output.c`: the smallest
    /// `abs(mode->refresh - mhz)`), so a rounded 60 would still find a 59.951
    /// panel — which is a kinder rule than the equality the previous one used,
    /// where the rounded rate named no mode at all and the preferred one came
    /// up instead while the answer said success.
    ///
    /// Exact anyway, and the field is still called `rate` for it: nearest is
    /// only unambiguous while one size offers one rate. A panel that offers
    /// 59.951 and 60.000 is decided by the decimals, and `hertz` here would
    /// make `Mode::hertz` the obvious thing to put in it.
    ///
    /// `None` lets the compositor pick among the rates this size offers —
    /// which is what somebody who changed only the resolution meant.
    pub rate: Option<f64>,
}

impl From<ronoctua_compositor::Mode> for Chosen {
    /// The safe way to build one, and the only one that needs no thought.
    fn from(m: ronoctua_compositor::Mode) -> Self {
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

/// Everything somebody chose for one monitor.
///
/// Every field's default is "let the compositor decide", so a monitor nobody has
/// touched writes no block at all — and a preference file is a list of
/// decisions rather than a photograph of the machine.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Wanted {
    /// Stop driving this monitor.
    pub off: bool,
    /// The mode, or `None` for the compositor's choice.
    pub mode: Option<Chosen>,
    /// The scale, or `None` for the compositor's choice.
    pub scale: Option<f64>,
    /// How the picture is turned.
    pub transform: Transform,
    /// Where it sits, or `None` for the compositor's arrangement.
    pub position: Option<Spot>,
    /// Whether the refresh rate varies.
    pub vary: bool,
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
/// The key is [`ronoctua_compositor::Output::identity`].
#[must_use]
pub fn text_of(screens: &BTreeMap<String, Wanted>) -> String {
    // `#` AND NOT `//`, which is not a comment to this compositor at all.
    //
    // It answers every one of these lines with
    // `Unknown/invalid command '//'` and then `Error(s) loading config!`, so
    // the WHOLE fragment is refused — every monitor's settings, from a header
    // nobody reads. And nothing says so: `reload` answers `success` whatever
    // parsed, and `scrollnag_command -` refuses the window it would otherwise
    // raise. Measured in `display-vm`, where the compositor went on running
    // the transient preview while the file it had just been handed was on the
    // floor.
    let mut out = String::from(
        "# GENERATED by ronoctua-display. Do not edit — the display view rewrites it.\n\
         # Included by the system's compositor config. It is what survives a\n\
         # restart; what makes the running compositor agree with it is the\n\
         # display view asking for a re-read right after writing this — the\n\
         # compositor watches no file of its own.\n",
    );
    // ORDER DOES NOT MATTER HERE, and that is worth saying because it used to
    // be load-bearing.
    //
    // Output blocks are MERGED, not raced: `store_output_config` files each by
    // name and `get_output_config` applies `*`, then the connector, then the
    // identifier, field by field with the later one winning
    // (`sway/config/output.c`). Our keys are connectors and identifiers, and
    // both are more specific than anything the system config says.
    //
    // Two of OUR OWN blocks can still name one monitor — that is what `Naming`
    // is: a monitor whose EDID carries a serial is filed under its
    // description, one that does not is filed under its connector — and
    // merging makes that harmless. Under a first-match compositor it was not:
    // a monitor with no serial on `DP-1`, filed as "DP-1" and turned off, was
    // unplugged, a monitor that DOES give a serial went into the same port,
    // and "D" sorting before "Z" meant the new one came up DARK while the
    // view's row for it said it was on. Nothing in the interface could reach
    // the stale block either, because forgetting a key only works for a
    // monitor present under it.
    //
    // SORTED BY KEY, for a readable diff and nothing else.
    //
    // There is no specificity to arrange here, and that is the compositor's
    // doing rather than luck: it merges the blocks that match one monitor in
    // a fixed order — the wildcard, then the connector, then the identifier —
    // so a description always lands on top of a connector whatever order they
    // are written in. What that buys is the defect it makes impossible: a
    // stale connector block for a port a different monitor was later plugged
    // into cannot beat that monitor's own description.
    //
    // What it costs is one line below: `enable` has to be written out, or a
    // stale `disable` merges first and nothing overrides it.
    let mut order: Vec<(&String, &Wanted)> = screens.iter().collect();
    order.sort_by_key(|(a, _)| *a);
    for (identity, w) in order {
        if w.says_nothing() {
            continue;
        }
        let _ = writeln!(out, "output \"{}\" {{", escaped(identity));
        if w.off {
            // AND NOTHING ELSE MATTERS. A monitor that is off has no mode, no
            // place and no scale; writing them would be describing a picture
            // nobody is drawing.
            out.push_str("    disable\n}\n");
            continue;
        }
        // WRITTEN OUT ALTHOUGH IT IS THE DEFAULT, and it is load-bearing.
        // Blocks are merged field by field, so a stale `disable` left under a
        // connector this monitor is now plugged into would survive every
        // field the description block sets — and the monitor would come up
        // dark with the view saying it is on.
        out.push_str("    enable\n");
        if let Some(m) = w.mode {
            let _ = match m.rate {
                // The rate is written with three decimals because a mode's
                // real rate is 59.951 and not 60: the string is parsed back
                // into a float and matched against the monitor's list, so a
                // rounded number lands on a neighbouring mode. The `Hz` is
                // not optional.
                Some(hz) => writeln!(out, "    mode {}x{}@{hz:.3}Hz", m.width, m.height),
                None => writeln!(out, "    mode {}x{}", m.width, m.height),
            };
        }
        // CLAMPED AND SIEVED. A NaN would be written as the literal `NaN`,
        // which fails the number parse and takes the WHOLE load down with it,
        // leaving every other monitor's settings unapplied. The range is ours
        // rather than a parser's — the compositor snaps a scale it dislikes
        // instead of refusing it — but a number nobody could have meant is
        // still not a number to write. Same reason the name is escaped.
        if let Some(s) = readable_scale(w.scale) {
            let _ = writeln!(out, "    scale {s}");
        }
        if w.transform != Transform::Normal {
            // ONE SPELLING, everywhere: the same word goes in the file, over
            // the wire and into a command.
            let _ = writeln!(
                out,
                "    transform {}",
                ronoctua_scroll::wire::word_of(w.transform)
            );
        }
        if let Some(p) = w.position {
            let _ = writeln!(out, "    position {},{}", p.x, p.y);
        }
        if w.vary {
            out.push_str("    adaptive_sync on\n");
        }
        out.push_str("}\n");
    }
    out
}

/// A monitor's name, safe inside a quoted string.
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
        text_of(&BTreeMap::from([("Maker Model-1 Unknown".to_owned(), w)]))
    }

    #[test]
    fn a_monitor_nobody_touched_writes_no_block() {
        // The file is a list of DECISIONS, not a photograph of the machine.
        // Writing every monitor out would mean a fragment that changes on
        // every hotplug, and a configuration reload each time.
        let quiet = text_of(&BTreeMap::from([
            ("Maker Model-1 Unknown".to_owned(), Wanted::default()),
            ("Other Model-2 ABC123".to_owned(), Wanted::default()),
        ]));
        assert!(!quiet.contains("output"), "{quiet}");
    }

    #[test]
    fn a_rate_keeps_its_decimals() {
        // A mode's real rate is 59.951. Rounded to 60 it is a mode the
        // monitor does not have. The compositor takes the nearest one, which
        // on a panel offering both 59.951 and 60.000 is the other mode — and
        // the answer still says success.
        let out = one(Wanted {
            mode: Some(Chosen {
                width: 1920,
                height: 1200,
                rate: Some(59.951),
            }),
            ..Wanted::default()
        });
        assert!(out.contains("mode 1920x1200@59.951Hz"), "{out}");
    }

    #[test]
    fn a_resolution_with_no_rate_lets_the_compositor_choose_one() {
        // Which is what somebody who changed only the resolution meant.
        let out = one(Wanted {
            mode: Some(Chosen {
                width: 1280,
                height: 720,
                rate: None,
            }),
            ..Wanted::default()
        });
        assert!(out.contains("mode 1280x720"), "{out}");
        assert!(!out.contains('@'), "{out}");
    }

    #[test]
    fn a_transform_has_one_spelling_everywhere() {
        // Not the way the wire spells it. `Flipped90` in the config is a
        // parse error, and a parse error in an included file takes the whole
        // reload down — so every OTHER monitor loses its settings too.
        let out = one(Wanted {
            transform: Transform::Flipped90,
            ..Wanted::default()
        });
        assert!(out.contains("transform flipped-90"), "{out}");
        assert!(!out.contains("Flipped90"), "{out}");
        // And the identity transform is not written: it is the default, and
        // a file that spells out defaults is a file nobody can read.
        let plain = one(Wanted {
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
        assert!(out.contains("    disable\n"), "{out}");
        assert!(!out.contains("enable"), "{out}");
        assert!(!out.contains("mode"), "{out}");
        assert!(!out.contains("scale"), "{out}");
        assert!(!out.contains("position"), "{out}");
    }

    #[test]
    fn a_quote_in_a_model_name_cannot_break_the_file() {
        // A parse error in an included fragment takes the reload down and
        // leaves every monitor unapplied — so this is not about tidiness.
        let out = text_of(&BTreeMap::from([(
            r#"Acme 27" Unknown"#.to_owned(),
            Wanted {
                scale: Some(1.0),
                ..Wanted::default()
            },
        )]));
        assert!(out.contains(r#"output "Acme 27\" Unknown""#), "{out}");
    }

    #[test]
    fn the_whole_grammar_comes_out_at_once() {
        // EVERY NODE THIS WRITER CAN EMIT, in one place a person can read
        // beside the compositor's own grammar — and it is not only for
        // reading: the `display-fragment-validates` check lifts this very text
        // out of the source and puts it in front of a real compositor, so what
        // is pinned here is what it is asked to accept.
        //
        // TWO MONITORS, because `off` is the one node that excludes the
        // others and would otherwise never appear in the pinned text.
        let both = text_of(&BTreeMap::from([
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
                    vary: true,
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
    enable
    mode 3840x2160@59.997Hz
    scale 1.5
    transform 270
    position -1920,120
    adaptive_sync on
}
output "Other Model-2 ABC123" {
    disable
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
            vary: true,
        });
        // The header is prose and its length is not the subject here, so the
        // body is taken from the first node rather than by counting lines.
        let body = out[out.find("output ").unwrap()..].trim_end().to_owned();
        assert_eq!(
            body,
            r#"output "Maker Model-1 Unknown" {
    enable
    mode 3840x2160@59.997Hz
    scale 1.5
    transform 270
    position -1920,120
    adaptive_sync on
}"#
        );
    }

    #[test]
    fn a_scale_outside_our_range_is_brought_into_it() {
        // The range is OURS — `output_cmd_scale` has none — so nothing here is
        // about a parse error. It is about what a person can still use: below
        // 0.1 the desktop is a hundred logical pixels across, and above 10 a
        // window is bigger than it.
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
        // and hides that something upstream produced nonsense. And the
        // compositor would not catch either: `strtof` accepts both spellings,
        // so `scale nan` is a line that parses and then does nothing, and
        // `scale inf` passes `> 0` and reaches wlroots.
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
        let m = ronoctua_compositor::Mode {
            width: 1920,
            height: 1200,
            refresh_rate: 59_951,
        };
        let chosen = Chosen::from(m);
        assert_eq!(chosen.rate, Some(59.951));
        let out = one(Wanted {
            mode: Some(chosen),
            ..Wanted::default()
        });
        assert!(out.contains("mode 1920x1200@59.951Hz"), "{out}");
    }

    #[test]
    fn varying_the_rate_is_a_plain_toggle() {
        let out = one(Wanted {
            vary: true,
            ..Wanted::default()
        });
        assert!(out.contains("    adaptive_sync on\n"), "{out}");
        // There is no third state to spell: the reading answers on or off.
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
            vary: true,
        };
        let text = serde_json::to_string(&w).unwrap();
        assert_eq!(serde_json::from_str::<Wanted>(&text).unwrap(), w);
        // And a file written by an older version, missing every field, reads
        // as "nothing was chosen" rather than failing.
        assert!(serde_json::from_str::<Wanted>("{}").unwrap().says_nothing());
    }

    #[test]
    fn a_description_block_beats_a_connector_block_whatever_the_order() {
        // THE ORDERING RULE DISSOLVED, and what replaced it is `enable`.
        //
        // Blocks that match one monitor are merged in a fixed order — the
        // wildcard, then the connector, then the identifier — so a
        // description always lands on top of a connector however they are
        // written. What that leaves is the field-level hazard: a stale
        // `disable` under a port a different monitor was later plugged into
        // survives every field the description block does NOT set.
        //
        // So every block that is not turning a monitor off says `enable`,
        // and this is the test that keeps it there. Without it the monitor
        // comes up dark while the view says it is on — and no gesture in the
        // interface can reach the stale block, because forgetting a key only
        // works for a monitor that is present under it.
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
                    scale: Some(1.5),
                    ..Wanted::default()
                },
            ),
        ]);
        let said = text_of(&screens);
        assert!(
            said.contains("    enable\n"),
            "the live block says so:\n{said}"
        );
        assert!(
            said.contains("    disable\n"),
            "and the dark one says so:\n{said}"
        );
    }
}
