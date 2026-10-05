//! The socket's vocabulary.
//!
//! # Why JSON, one object per line
//!
//! The same answer the transfer queue's wire gives, and for the same two
//! reasons. What crosses here carries FREE TEXT — a step's trouble is whatever
//! the machine said, and a machine says things with spaces and newlines in them
//! — and a whitespace grammar loses half of one without telling anybody. And
//! `serde_json` is already in the workspace, so one object per line keeps the
//! stream readable with `socat` while it is being built.
//!
//! **Invariant 11 is untouched.** The JSON dies at the host: a wasm component
//! receives typed records through the WIT and has no parser, exactly as before.
//!
//! # Why every answer is the whole mind
//!
//! Because a delta needs both sides to agree on what they last saw, and the
//! thing being agreed about here is *when will my machine rebuild itself*. A
//! client that dropped one update would draw a wrong answer for the rest of the
//! session with nothing to notice it by. The whole thing is a few short
//! records — see [`crate::mind`].

use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::mind::Mind;
use crate::prefs::{Gate, PartOfDay};

/// What a client asks the brain for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "ask")]
pub enum Ask {
    /// The whole mind, once, and then the connection closes.
    Look,
    /// The whole mind, and then again on every change until the connection
    /// drops.
    ///
    /// It is how a screen stays honest without polling: the footer holds one of
    /// these open on a thread and pushes what arrives into `calloop`, exactly
    /// as it already does with the transfer queue and with the compositor's events.
    Watch,
    /// The daemon's own log: every line it keeps, and then every new one as it
    /// is written, until the connection drops. See [`crate::tail`].
    ///
    /// A stream of its own and not a field of the mind, because the mind's
    /// every change wakes every screen that watches the brain — and a tick is
    /// a line every half minute that only the log's page wants.
    Tail,
    /// Put this task in the queue now, whatever its schedule says.
    Run {
        /// Which.
        task: String,
        /// A dry run: it starts the task's rehearsal unit and is NOT written
        /// down as a run. Asking what an update would do must not count as
        /// having done it.
        rehearsal: bool,
    },
    /// Stop it, or take it out of the queue if it has not started.
    Abort {
        /// Which.
        task: String,
    },
    /// Do not wait out the countdown — start now.
    Proceed,
    /// Not now. It leaves the queue and comes back at its next moment.
    Postpone,
    /// Switch one of a task's steps off, or on again.
    Toggle {
        /// Whose.
        task: String,
        /// Which step.
        step: String,
    },
    /// Put a task's steps back to what the distro ships.
    Reset {
        /// Which.
        task: String,
    },
    /// Change ONE preference of one task, resolved over the module's own
    /// defaults.
    ///
    /// # Why one knob and not the whole record
    ///
    /// Because the whole record (the old `Tune`, gone once every door moved
    /// here) is what let three places write a task's preferences from the
    /// SHARED default (`Prefs::mine`): a screen that
    /// switched a watch off and on again sent back a weekly cadence with a
    /// minute of countdown the watch never had. One knob is applied by the
    /// daemon, which is the one place that knows the module's defaults.
    ///
    /// A knob a module does not have — a cadence for a module that keeps no
    /// calendar, a step it does not declare — is refused with a sentence.
    Set {
        /// Which task, by id.
        task: String,
        /// Which preference, and its value.
        knob: Knob,
    },
    /// Replace the general gate — the other half of what the old `Tune` did.
    Gate {
        /// The whole gate: three numbers, one policy.
        gate: Gate,
    },
    /// Do one of a module's actions (`<module>-<action>`). Refused with a
    /// sentence when the module or the action is unknown, a row action has no
    /// row or an action that asks has no answer; declined by the MODULE with
    /// the text key of its reason (`<module>-why-<word>`, `brain-why-<word>`)
    /// in [`Say::Declined`].
    Act {
        /// Whose, by id.
        module: String,
        /// Which action, by its id inside the module.
        action: String,
        /// The row of the module's screen it was asked on, by the row's id.
        row: Option<String>,
        /// What the person typed, for an action that asks first.
        answer: Option<String>,
    },
    /// Put a notice away until it is news again — only one its declaration
    /// lets be dismissed, and only while it is up.
    Dismiss {
        /// Whose, by id.
        module: String,
        /// Which notice, by its id inside the module.
        notice: String,
    },
    /// Put a notice off for a while — one of the snoozes it offers, only
    /// while it is up.
    Snooze {
        /// Whose, by id.
        module: String,
        /// Which notice.
        notice: String,
        /// For how long — one of the snoozes the notice offers.
        minutes: u16,
    },
    /// A module's own page, now: answered with [`Say::Screen`]. A page asks
    /// when the task's `screen-generation` moved, never on a clock.
    Screen {
        /// Whose, by id.
        module: String,
    },
}

/// WHICH preference, and its VALUE — the brain's side of the WIT's `knob`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Knob {
    /// Days between runs.
    IntervalDays(u16),
    /// Which stretch of the day.
    PartOfDay(PartOfDay),
    /// Whether it runs on its own at all.
    Enabled(bool),
    /// One step, on or off.
    Step {
        /// Which step, by id.
        id: String,
        /// Whether it is to run.
        on: bool,
    },
    /// Every step back to what the distro ships.
    ResetSteps,
}

/// What the brain says back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "say")]
pub enum Say {
    /// Everything there is to know. Whole, never a delta.
    ///
    /// `Box` because it is much larger than the other case, and an enum is as
    /// big as its biggest arm: every refusal would otherwise carry the size of
    /// a whole mind on the stack.
    Mind {
        /// The state.
        mind: Box<Mind>,
    },
    /// Lines of the daemon's own log, oldest first: the whole ring on the
    /// first answer to [`Ask::Tail`], and only the new ones after it.
    Lines {
        /// In order.
        lines: Vec<crate::tail::Line>,
    },
    /// The ask made no sense, and this is why.
    Refused {
        /// In the daemon's own words.
        why: String,
    },
    /// A module declined an [`Ask::Act`], and this key says why — a case of
    /// its own, and not the words of [`Say::Refused`], so the door knows to
    /// TRANSLATE it (invariant 8) and can say it in the door that asked.
    Declined {
        /// The text key of the reason: `<module>-why-<word>`, or
        /// `brain-why-<word>` for a word of the core's.
        key: String,
    },
    /// A module's own page, as asked by [`Ask::Screen`] — rows of DATA, never
    /// carried in the mind.
    Screen {
        /// Whose, by id.
        module: String,
        /// The generation it is, so a page that asked twice keeps the newer.
        generation: u64,
        /// The rows.
        screen: crate::module::Screen,
    },
}

/// Where the socket lives.
///
/// # Why one per USER and not one per display
///
/// The footer's control socket carries the display in its name so a nested
/// `nix run .#dev` does not send keys to the real session. This one is the
/// opposite case, exactly as the transfer queue's is: the machine has one
/// system to update and one memory to be busy, whichever compositor happens to
/// be on screen. Two brains would be two answers to "when will this machine
/// update itself", and the second one would be invisible from the first.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent. There is no fallback to `/tmp`, for the reason
/// `leader::socket_path` already gives: a control socket in a shared directory
/// is attack surface.
///
/// Named per instance (`ronoctua_xdg::INSTANCE`): the development loop's brain
/// is not the installed session's, and it only ever rehearses.
pub fn socket_path() -> Result<PathBuf> {
    ronoctua_xdg::runtime_file("brain.sock")
        .context("XDG_RUNTIME_DIR is not set; without it there is nowhere to put the socket")
}

/// Writes one object and the newline that ends it.
///
/// # Errors
///
/// When the value cannot be encoded, or the other side has gone.
pub fn write_line<T: Serialize>(to: &mut impl Write, value: &T) -> Result<()> {
    let mut line = serde_json::to_string(value).context("encode the line")?;
    line.push('\n');
    to.write_all(line.as_bytes())
        .context("write to the brain's socket")?;
    to.flush().context("drain the brain's socket")
}

/// Reads one object. `None` when the other side closed cleanly.
///
/// # Errors
///
/// When the read fails or the line is not the expected shape.
pub fn read_line<T: DeserializeOwned>(from: &mut impl BufRead) -> Result<Option<T>> {
    let mut line = String::new();
    if from
        .read_line(&mut line)
        .context("read from the brain's socket")?
        == 0
    {
        return Ok(None);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .with_context(|| format!("a line I did not understand: {trimmed}"))
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mind::{ActionState, Announcement, Posted, Said, Standing, StepState, TaskState};
    use crate::module::{Arg, Cell, Item, Level, Mark, MarkId, Row, Section, Tone};
    use crate::prefs::Prefs;
    use crate::task::{Hold, OutcomeKind, RunRecord, StepOutcome};

    fn a_mind() -> Mind {
        Mind {
            journal: Vec::new(),
            now: 1_700_000_000,
            pulse: crate::pulse::Pulse {
                at: 1_700_000_000,
                ram_used_pct: Some(41),
                memory_stall: Some(3),
                // NOT MEASURED, on purpose: the one thing a round trip has to
                // keep is the difference between "zero" and "nothing was read",
                // and a serialisation that turned this into a number would be
                // a gate with a hole in it.
                cpu_stall: None,
                on_battery: Some(false),
                disk_free_per_mille: Some(87),
                rooms: vec![crate::pulse::Room {
                    total: 256_000_000_000,
                    free: 22_272_000_000,
                }],
            },
            tasks: vec![TaskState {
                kind: crate::task::Kind::Routine,
                id: "made-up".to_owned(),
                standing: Standing::Queued,
                enabled: true,
                interval_days: 7,
                part_of_day: PartOfDay::Afternoon,
                next: 1_700_003_600,
                hold: Some(Hold::Ram),
                why_not: String::new(),
                history: Vec::new(),
                last: Some(RunRecord {
                    task: "made-up".to_owned(),
                    began: 1_699_000_000,
                    ended: 1_699_003_000,
                    kind: OutcomeKind::Failed,
                    at: "build".to_owned(),
                    why: String::new(),
                    trouble: "error: builder for drv failed\n  line 2".to_owned(),
                    steps: vec![StepOutcome {
                        id: "build".to_owned(),
                        ok: false,
                        seconds: 3000,
                        trouble: "it did not work".to_owned(),
                    }],
                    says: String::new(),
                    args: Vec::new(),
                }),
                steps: vec![StepState {
                    id: "build".to_owned(),
                    command: "nixos-rebuild boot --flake /etc/nixos".to_owned(),
                    needs_root: true,
                    on: true,
                    on_by_default: true,
                    name_key: "update-build".to_owned(),
                    does_key: "update-build-does".to_owned(),
                }],
                calendar: true,
                name_key: "brain-task-made-up".to_owned(),
                does_key: "brain-task-made-up-does".to_owned(),
                why_key: String::new(),
                screen_generation: 3,
                actions: vec![ActionState {
                    id: "keep".to_owned(),
                    label: "cmd-made-up-keep".to_owned(),
                    row: false,
                    asks: Some("ask-made-up-keep".to_owned()),
                }],
            }],
            queue: vec!["made-up".to_owned()],
            running: String::new(),
            announcement: Some(Announcement {
                task: "made-up".to_owned(),
                total_seconds: 60,
                deadline: 1_700_000_060,
                why: "due".to_owned(),
            }),
            prefs: Prefs::default(),
            rehearsing: None,
            postponed: None,
            // THE MODULES' HALF, every kind of it: what a module publishes has
            // to come back from a line of JSON EQUAL, borrowed constants and
            // all.
            notices: vec![Posted {
                module: "fixture".to_owned(),
                what: crate::module::Notice {
                    id: "due".into(),
                    says: "fixture-says-due".into(),
                    args: vec![
                        ("freed".into(), Arg::Bytes(12_345)),
                        ("when".into(), Arg::Moment(1_700_000_000)),
                        ("free".into(), Arg::PerMille(87)),
                        ("what".into(), Arg::Key("fixture-what".into())),
                        ("typed".into(), Arg::Text("water the plants".to_owned())),
                        ("count".into(), Arg::Number(-3)),
                    ],
                    level: Level::Alarm,
                    since: 1_699_999_000,
                    controls: vec!["forget".into()],
                    dismiss: true,
                    snooze: vec![5, 10, 60].into(),
                },
            }],
            marks: vec![Posted {
                module: "fixture".to_owned(),
                what: Mark {
                    id: MarkId::of("full", "fixture-full", "fixture-mark-full"),
                    tone: Tone::Danger,
                    count: 2,
                    act: Some("forget".into()),
                },
            }],
            events: vec![Said {
                seq: 7,
                tag: "fixture-emptied".into(),
                args: vec![("freed".into(), Arg::Bytes(1))],
            }],
            items: vec![Posted {
                module: "fixture".to_owned(),
                what: Item {
                    id: "r-1".to_owned(),
                    label: Cell::Data("call the plumber".to_owned()),
                    at: 1_700_086_400,
                    act: None,
                },
            }],
        }
    }

    fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
        let mut wire = Vec::new();
        write_line(&mut wire, value).expect("write");
        assert_eq!(
            wire.last().copied(),
            Some(b'\n'),
            "the line did not end in a newline: the next object would stick to this one"
        );
        let mut reading = std::io::BufReader::new(wire.as_slice());
        let back: T = read_line(&mut reading)
            .expect("read")
            .expect("nothing on the line");
        assert_eq!(&back, value);
    }

    #[test]
    fn every_ask_crosses_the_wire_and_comes_back_the_same() {
        for ask in [
            Ask::Look,
            Ask::Watch,
            Ask::Run {
                task: "made-up".to_owned(),
                rehearsal: false,
            },
            Ask::Run {
                task: "made-up".to_owned(),
                rehearsal: true,
            },
            Ask::Abort {
                task: "made-up".to_owned(),
            },
            Ask::Proceed,
            Ask::Postpone,
            Ask::Toggle {
                task: "made-up".to_owned(),
                step: "collect-garbage".to_owned(),
            },
            Ask::Reset {
                task: "made-up".to_owned(),
            },
            Ask::Set {
                task: "made-up".to_owned(),
                knob: Knob::IntervalDays(14),
            },
            Ask::Set {
                task: "made-up".to_owned(),
                knob: Knob::PartOfDay(PartOfDay::Night),
            },
            Ask::Set {
                task: "made-up".to_owned(),
                knob: Knob::Enabled(false),
            },
            Ask::Set {
                task: "made-up".to_owned(),
                knob: Knob::Step {
                    id: "collect-garbage".to_owned(),
                    on: false,
                },
            },
            Ask::Set {
                task: "made-up".to_owned(),
                knob: Knob::ResetSteps,
            },
            Ask::Gate {
                gate: crate::prefs::Gate::default(),
            },
            Ask::Act {
                module: "fixture".to_owned(),
                action: "forget".to_owned(),
                row: Some("r-1".to_owned()),
                answer: None,
            },
            Ask::Act {
                module: "fixture".to_owned(),
                action: "new".to_owned(),
                row: None,
                answer: Some("tomorrow 9:00 water the plants".to_owned()),
            },
            Ask::Dismiss {
                module: "fixture".to_owned(),
                notice: "due".to_owned(),
            },
            Ask::Snooze {
                module: "fixture".to_owned(),
                notice: "due".to_owned(),
                minutes: 10,
            },
            Ask::Screen {
                module: "fixture".to_owned(),
            },
        ] {
            round_trip(&ask);
        }
    }

    #[test]
    fn a_mind_crosses_whole_including_what_was_never_measured() {
        round_trip(&Say::Mind {
            mind: Box::new(a_mind()),
        });
        round_trip(&Say::Refused {
            why: "I do not know that task".to_owned(),
        });
        round_trip(&Say::Declined {
            key: "fixture-why-when-empty".to_owned(),
        });
        // A CASE OF ITS OWN on the wire, never a `refused` a door would draw
        // untranslated.
        let line = serde_json::to_string(&Say::Declined {
            key: "k".to_owned(),
        })
        .expect("encode");
        assert_eq!(line, r#"{"say":"declined","key":"k"}"#);
        round_trip(&Say::Screen {
            module: "fixture".to_owned(),
            generation: 4,
            screen: crate::module::Screen {
                sections: vec![Section {
                    heading: "fixture-heading".into(),
                    rows: vec![Row {
                        id: "one".to_owned(),
                        label: Cell::Key("fixture-row".into()),
                        value: Cell::Count(3),
                        note: Cell::Empty,
                        tone: Tone::Watching,
                        act: None,
                        also: Some("ignore".into()),
                    }],
                }],
            },
        });
    }

    #[test]
    fn the_whole_record_is_no_longer_an_ask() {
        // THE OLD `Tune` IS GONE: a door that still sent every preference back
        // is refused at the line, and never writes a module's preferences
        // from the shared default again.
        let old = r#"{"ask":"tune","prefs":{}}"#;
        assert!(
            serde_json::from_str::<Ask>(old).is_err(),
            "a tune still reads"
        );
    }

    #[test]
    fn a_mind_from_before_the_modules_still_reads() {
        // A LINE FROM AN OLDER DAEMON (a footer restarted before its brain):
        // a line without any of the modules' fields is still a mind, and a
        // task row without them is still a row — with a calendar, which every
        // task before the modules kept. Its one-line `notice` is ignored.
        let old = r#"{"now":5,"tasks":[{"id":"made-up","kind":"routine","standing":"idle","enabled":true,"interval-days":7,"part-of-day":"afternoon","next":0,"hold":null,"why-not":"","last":null,"steps":[{"id":"build","command":"x","needs-root":true,"on":true,"on-by-default":true}]}],"notice":{"task":"t","says":"k","act":null}}"#;
        let mind: Mind = serde_json::from_str(old).expect("an old mind no longer reads");
        assert!(mind.notices.is_empty() && mind.marks.is_empty());
        assert!(mind.tasks[0].calendar);
        assert!(mind.tasks[0].name_key.is_empty());
        assert!(mind.tasks[0].steps[0].name_key.is_empty());
    }

    #[test]
    fn a_newline_inside_a_failure_does_not_split_the_line() {
        // THE DEFECT THIS CATCHES: a machine's error message with a newline in
        // it — which is most of them — cut in half by a line-based protocol,
        // and the second half read as an object nobody sent. JSON escapes it;
        // a whitespace grammar would not, which is the trap `leader.rs`'s
        // `rsplit_once` already records.
        let mut mind = a_mind();
        if let Some(last) = mind.tasks[0].last.as_mut() {
            last.trouble = "first\nsecond\tthird".to_owned();
        }
        let mut wire = Vec::new();
        write_line(
            &mut wire,
            &Say::Mind {
                mind: Box::new(mind.clone()),
            },
        )
        .expect("write");
        assert_eq!(
            wire.iter().filter(|b| **b == b'\n').count(),
            1,
            "the message became two lines"
        );
        let mut reading = std::io::BufReader::new(wire.as_slice());
        let Some(Say::Mind { mind: back }) = read_line(&mut reading).expect("read") else {
            panic!("it did not give back a mind");
        };
        assert_eq!(*back, mind);
    }

    #[test]
    fn the_socket_is_one_per_user() {
        // SAFETY OF THE TEST, not of the code: `XDG_RUNTIME_DIR` is
        // process-global, so this only reads it.
        if let Ok(path) = socket_path() {
            assert!(path.ends_with("ronoctua-brain.sock"));
        }
    }
}
