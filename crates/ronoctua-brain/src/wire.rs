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
use crate::prefs::Prefs;

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
    /// as it already does with the transfer queue and with niri's events.
    Watch,
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
    /// Replace the preferences with these, and write them down.
    ///
    /// The WHOLE record, and not one field: the screen has the mind in front of
    /// it and sends back what it wants, so there is one shape to validate
    /// instead of one message per preference. The daemon is what writes the
    /// file — the component touches no disk (invariant 11).
    Tune {
        /// All of them.
        prefs: Box<Prefs>,
    },
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
    /// The ask made no sense, and this is why.
    Refused {
        /// In the daemon's own words.
        why: String,
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
pub fn socket_path() -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .context("XDG_RUNTIME_DIR is not set; without it there is nowhere to put the socket")?;
    Ok(PathBuf::from(dir).join("ronoctua-brain.sock"))
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
    use crate::mind::{Announcement, Standing, StepState, TaskState};
    use crate::prefs::PartOfDay;
    use crate::task::{Hold, OutcomeKind, RunRecord, StepOutcome};

    fn a_mind() -> Mind {
        Mind {
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
            },
            tasks: vec![TaskState {
                id: "system-update".to_owned(),
                standing: Standing::Queued,
                enabled: true,
                interval_days: 7,
                part_of_day: PartOfDay::Afternoon,
                next: 1_700_003_600,
                hold: Some(Hold::Ram),
                why_not: String::new(),
                last: Some(RunRecord {
                    task: "system-update".to_owned(),
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
                }),
                steps: vec![StepState {
                    id: "build".to_owned(),
                    command: "nixos-rebuild boot --flake /etc/nixos".to_owned(),
                    needs_root: true,
                    on: true,
                    changed: false,
                }],
            }],
            queue: vec!["system-update".to_owned()],
            running: String::new(),
            announcement: Some(Announcement {
                task: "system-update".to_owned(),
                total_seconds: 60,
                deadline: 1_700_000_060,
                why: "due".to_owned(),
            }),
            prefs: Prefs::default(),
            rehearsing: None,
            postponed: None,
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
            .expect("ler")
            .expect("nothing on the line");
        assert_eq!(&back, value);
    }

    #[test]
    fn every_ask_crosses_the_wire_and_comes_back_the_same() {
        for ask in [
            Ask::Look,
            Ask::Watch,
            Ask::Run {
                task: "system-update".to_owned(),
                rehearsal: false,
            },
            Ask::Run {
                task: "system-update".to_owned(),
                rehearsal: true,
            },
            Ask::Abort {
                task: "system-update".to_owned(),
            },
            Ask::Proceed,
            Ask::Postpone,
            Ask::Toggle {
                task: "system-update".to_owned(),
                step: "collect-garbage".to_owned(),
            },
            Ask::Reset {
                task: "system-update".to_owned(),
            },
            Ask::Tune {
                prefs: Box::new(Prefs::default()),
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
        let Some(Say::Mind { mind: back }) = read_line(&mut reading).expect("ler") else {
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
