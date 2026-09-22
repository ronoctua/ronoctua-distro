//! The protocol: framing, and the words a command is made of.
//!
//! This is the only file in the repository that knows the compositor's
//! spelling. Everything above it speaks `ronoctua_compositor`'s vocabulary.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use anyhow::{Context as _, Result, bail};
use ronoctua_compositor::{Arity, Ask, Deed, Transform};

/// The six bytes that start every message, in both directions.
///
/// It is `i3-ipc` and not the compositor's own name, because this is i3's
/// protocol wearing a fork's socket — which is also why a stock `swaymsg`
/// talks to it unmodified.
const MAGIC: &[u8; 6] = b"i3-ipc";

/// A message's kind.
///
/// The numbers are the protocol's. The ones above 100 are the fork's own, and
/// they are the half of this surface with no compatibility promise — see the
/// drift check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    /// Run commands, given as free text.
    Command = 0,
    /// Every workspace.
    Workspaces = 1,
    /// Ask to be told about things.
    Subscribe = 2,
    /// Every output.
    Outputs = 3,
    /// The whole tree of containers.
    Tree = 4,
    /// What the compositor calls itself.
    Version = 7,
}

/// An event's kind, as it comes back.
///
/// Events carry the high bit, which is what tells a reply from an event on a
/// socket that carries both — and it is why a reader never has to remember
/// what it asked for.
pub const EVENT_BIT: u32 = 0x8000_0000;
/// Something happened to a window.
pub const EVENT_WINDOW: u32 = EVENT_BIT | 3;
/// Something happened to the outputs.
pub const EVENT_OUTPUT: u32 = EVENT_BIT | 1;
/// The compositor is going away.
pub const EVENT_SHUTDOWN: u32 = EVENT_BIT | 6;

/// Writes one message.
pub fn send(to: &mut UnixStream, kind: Kind, payload: &str) -> Result<()> {
    let body = payload.as_bytes();
    let mut head = Vec::with_capacity(14 + body.len());
    head.extend_from_slice(MAGIC);
    // NATIVE ENDIAN, and that is the protocol's own choice rather than ours:
    // it was designed for a socket that never leaves the machine.
    head.extend_from_slice(&u32::try_from(body.len())?.to_ne_bytes());
    head.extend_from_slice(&(kind as u32).to_ne_bytes());
    head.extend_from_slice(body);
    to.write_all(&head).context("writing to the compositor")?;
    to.flush().context("flushing to the compositor")?;
    Ok(())
}

/// Reads one message: its kind, and its body.
pub fn receive(from: &mut UnixStream) -> Result<(u32, String)> {
    let mut head = [0_u8; 14];
    from.read_exact(&mut head).context("reading a header")?;
    if &head[..6] != MAGIC {
        bail!("the reply did not start with the protocol's magic");
    }
    let length = u32::from_ne_bytes(head[6..10].try_into()?) as usize;
    let kind = u32::from_ne_bytes(head[10..14].try_into()?);
    let mut body = vec![0_u8; length];
    from.read_exact(&mut body).context("reading a body")?;
    Ok((
        kind,
        String::from_utf8(body).context("a body that is not UTF-8")?,
    ))
}

/// The command that performs an [`Ask`] on one output.
///
/// One function, so the spelling is written once and a test can read it
/// without a compositor.
///
/// [`Ask::Position`] with `None` is deliberately absent: there is no command
/// for "place it automatically" — the sentinel that means it cannot be
/// reached from the command line at all. It goes through the file and a
/// re-read, which is the same path everything persistent takes anyway.
#[must_use]
pub fn command_of(at: &str, ask: Ask) -> Option<String> {
    let said = match ask {
        Ask::On => format!("output \"{at}\" enable"),
        Ask::Off => format!("output \"{at}\" disable"),
        // `0x0` is how the parser is told to fall back to the preferred mode:
        // a width that is not positive stops it looking for a match.
        Ask::Mode(None) => format!("output \"{at}\" mode 0x0"),
        Ask::Mode(Some(m)) => match m.refresh {
            // THE `Hz` SUFFIX IS NOT OPTIONAL, and three decimals are not
            // decoration: a panel sold as 60 Hz runs at 59,951, and a rate
            // rounded to the whole number lands on a neighbouring mode.
            Some(r) => format!("output \"{at}\" mode {}x{}@{r:.3}Hz", m.width, m.height),
            None => format!("output \"{at}\" mode {}x{}", m.width, m.height),
        },
        // A scale of zero is the parser's "choose one yourself".
        Ask::Scale(None) => format!("output \"{at}\" scale 0"),
        Ask::Scale(Some(f)) => format!("output \"{at}\" scale {f}"),
        Ask::Transform(t) => format!("output \"{at}\" transform {}", word_of(t)),
        // A SPACE AND NOT A COMMA, and that is the difference between a
        // command and two.
        //
        // `output_cmd_position` takes both spellings — `1234,4321` and the
        // legacy `1234 4321` — and in a CONFIG FILE either works. Over the
        // IPC only one does: `,` is a command SEPARATOR there, the same as
        // `;`, so `position -1920,120` arrives as `output "X" position -1920`
        // followed by `120`. The first half then answers
        // `Missing position argument (y).` and the second is not a command at
        // all.
        //
        // Measured against a live compositor by `compositor-contract`, which
        // is the only thing that could have found it: the string is
        // well-formed, the config file accepts it, and `--validate` accepts
        // it. Only the wire splits.
        Ask::Position(Some((x, y))) => format!("output \"{at}\" position {x} {y}"),
        Ask::Position(None) => return None,
        Ask::Vary(on) => {
            let how = if on { "on" } else { "off" };
            format!("output \"{at}\" adaptive_sync {how}")
        }
    };
    Some(said)
}

/// The one word a transform is written with.
#[must_use]
pub const fn word_of(t: Transform) -> &'static str {
    match t {
        Transform::Normal => "normal",
        Transform::R90 => "90",
        Transform::R180 => "180",
        Transform::R270 => "270",
        Transform::Flipped => "flipped",
        Transform::Flipped90 => "flipped-90",
        Transform::Flipped180 => "flipped-180",
        Transform::Flipped270 => "flipped-270",
    }
}

/// The command a [`Deed`] performs, with its argument put in.
///
/// # Why this VALIDATES instead of interpolating
///
/// Commands are free text, and the separator between two of them is `;` or
/// `,`. So an argument is not a value in a structured message — it is text
/// that lands in the middle of a sentence the compositor will parse. Typing
/// `1; exit` into the minibuffer would end the session.
///
/// A protocol of enumerated actions could not be attacked this way, and it is
/// the one thing that got harder in exchange for everything that got simpler.
/// The answer is that an argument must BE the shape the deed says it is, and
/// anything else is refused before it reaches the socket.
pub fn spoken(deed: &Deed, text: &str, argument: Option<&str>) -> Result<String> {
    match (deed.argument, argument) {
        (None, None) => Ok(text.to_owned()),
        (None, Some(_)) => bail!("`{}` takes nothing after it", deed.id),
        (Some(_), None) => bail!("`{}` needs something after it", deed.id),
        (Some(arity), Some(given)) => {
            let ok = match arity {
                Arity::Number => given.parse::<f64>().is_ok(),
                Arity::Word => {
                    !given.is_empty()
                        && given
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || "._:+-".contains(c))
                }
            };
            if !ok {
                bail!("`{given}` is not something `{}` can take", deed.id);
            }
            Ok(text.replace("{n}", given))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_compositor::ModeAsked;

    #[test]
    fn every_ask_has_the_shape_the_compositor_documents() {
        let cases: Vec<(Ask, &str)> = vec![
            (Ask::On, "output \"eDP-1\" enable"),
            (Ask::Off, "output \"eDP-1\" disable"),
            (Ask::Mode(None), "output \"eDP-1\" mode 0x0"),
            (
                Ask::Mode(Some(ModeAsked {
                    width: 1920,
                    height: 1200,
                    refresh: Some(59.951),
                })),
                "output \"eDP-1\" mode 1920x1200@59.951Hz",
            ),
            (Ask::Scale(None), "output \"eDP-1\" scale 0"),
            (Ask::Scale(Some(1.5)), "output \"eDP-1\" scale 1.5"),
            (
                Ask::Transform(Transform::Flipped90),
                "output \"eDP-1\" transform flipped-90",
            ),
            // A SPACE. `,` is a command separator on this wire, so the comma
            // spelling — which a config FILE takes — arrives as two commands
            // and the first answers `Missing position argument (y).` See
            // `command_of`; a live compositor is what found it.
            (
                Ask::Position(Some((-1920, 120))),
                "output \"eDP-1\" position -1920 120",
            ),
            (Ask::Vary(true), "output \"eDP-1\" adaptive_sync on"),
            (Ask::Vary(false), "output \"eDP-1\" adaptive_sync off"),
        ];
        for (ask, said) in cases {
            assert_eq!(command_of("eDP-1", ask).as_deref(), Some(said), "{ask:?}");
        }
        // The one that has no command at all, and says so rather than
        // sending something that would be wrong.
        assert_eq!(command_of("eDP-1", Ask::Position(None)), None);
    }

    #[test]
    fn a_rate_keeps_its_three_decimals() {
        // Rounded to 60 this asks for a mode the monitor does not have.
        let m = ModeAsked {
            width: 2560,
            height: 1440,
            refresh: Some(143.998),
        };
        assert_eq!(
            command_of("DP-1", Ask::Mode(Some(m))).unwrap(),
            "output \"DP-1\" mode 2560x1440@143.998Hz"
        );
    }

    #[test]
    fn no_command_carries_a_separator_of_its_own() {
        // `;` AND `,` BOTH END A COMMAND on this wire, so a command that
        // contains either is two commands and the second is whatever was
        // after it.
        //
        // This has been paid for once: `position -1920,120` is the spelling a
        // config file takes and `output_cmd_position` documents, and over the
        // IPC it arrived as `output "X" position -1920` plus `120`. The first
        // answered `Missing position argument (y).` and nothing said where the
        // rest went. Only a live compositor could tell — the string is
        // well-formed and `--validate` accepts the file.
        //
        // So no command this module builds may carry one, for any input.
        let at = "eDP-1";
        let every = [
            Ask::On,
            Ask::Off,
            Ask::Mode(None),
            Ask::Scale(None),
            Ask::Scale(Some(1.5)),
            Ask::Transform(Transform::Flipped90),
            Ask::Position(Some((-1920, 120))),
            Ask::Position(Some((0, 0))),
            Ask::Vary(true),
            Ask::Vary(false),
        ];
        for ask in every {
            let Some(said) = command_of(at, ask) else {
                continue;
            };
            assert!(
                !said.contains(',') && !said.contains(';'),
                "`{said}` carries a separator: the compositor would read it as two commands"
            );
        }
    }

    #[test]
    fn an_argument_cannot_smuggle_a_second_command() {
        // THE ATTACK THIS EXISTS TO STOP: commands are separated by `;` and
        // `,`, and `LEADER :` prompts for free text.
        let go = Deed {
            id: "wm-workspace-go",
            key: "cmd-wm-workspace-go",
            argument: Some(Arity::Number),
        };
        assert!(spoken(&go, "workspace number {n}", Some("3; exit")).is_err());
        assert!(spoken(&go, "workspace number {n}", Some("3, kill")).is_err());
        assert!(spoken(&go, "workspace number {n}", Some("../../etc")).is_err());
        assert_eq!(
            spoken(&go, "workspace number {n}", Some("3")).unwrap(),
            "workspace number 3"
        );

        let mark = Deed {
            id: "wm-mark",
            key: "cmd-wm-mark",
            argument: Some(Arity::Word),
        };
        assert!(
            spoken(&mark, "mark {n}", Some("a b")).is_err(),
            "a space is two words"
        );
        assert!(spoken(&mark, "mark {n}", Some("a;b")).is_err());
        assert_eq!(
            spoken(&mark, "mark {n}", Some("home")).unwrap(),
            "mark home"
        );
    }

    #[test]
    fn an_argument_is_required_exactly_where_it_is_declared() {
        let plain = Deed {
            id: "wm-focus-left",
            key: "cmd-wm-focus-left",
            argument: None,
        };
        assert_eq!(spoken(&plain, "focus left", None).unwrap(), "focus left");
        assert!(spoken(&plain, "focus left", Some("3")).is_err());
        let takes = Deed {
            id: "wm-workspace-go",
            key: "k",
            argument: Some(Arity::Number),
        };
        assert!(spoken(&takes, "workspace number {n}", None).is_err());
    }

    #[test]
    fn an_argument_goes_in_everywhere_it_is_named() {
        // Moving a window to a workspace and following it names the number
        // TWICE, which is why substitution is a placeholder and not an
        // append.
        let move_to = Deed {
            id: "wm-workspace-move",
            key: "k",
            argument: Some(Arity::Number),
        };
        assert_eq!(
            spoken(
                &move_to,
                "move container to workspace number {n}; workspace number {n}",
                Some("4")
            )
            .unwrap(),
            "move container to workspace number 4; workspace number 4"
        );
    }
}
