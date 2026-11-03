//! The answers two engines owe a program, in the form they are compared in.

/// An answer two engines are expected to give identically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// CPR, the answer to `CSI 6 n`: where the cursor is, counted from 1.
    Cursor {
        /// Row.
        row: u32,
        /// Column.
        column: u32,
    },
    /// DSR, the answer to `CSI 5 n`.
    Status(u32),
    /// DECRPM, the answer to DECRQM: a mode and its state (1 set, 2 reset…).
    Mode {
        /// A DEC private mode (`?`) rather than an ANSI one.
        private: bool,
        /// The mode's number.
        mode: u32,
        /// The state as DECRPM numbers it.
        state: u32,
    },
    /// The answer to an OSC 4/10/11/12 query, in 8 bits a channel.
    Colour {
        /// `4;<index>` for a palette entry, `10`, `11` or `12` for the
        /// foreground, background and cursor.
        slot: String,
        /// The colour.
        rgb: (u8, u8, u8),
    },
}

impl std::fmt::Display for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cursor { row, column } => write!(f, "cursor {row};{column}"),
            Self::Status(s) => write!(f, "status {s}"),
            Self::Mode {
                private,
                mode,
                state,
            } => write!(f, "mode {}{mode}={state}", if *private { "?" } else { "" }),
            Self::Colour {
                slot,
                rgb: (r, g, b),
            } => write!(f, "colour {slot}=#{r:02x}{g:02x}{b:02x}"),
        }
    }
}

/// The answers in `bytes` that two engines must agree on, in order.
///
/// Everything else is dropped ON PURPOSE: DA1, DA2 and XTVERSION say who the
/// terminal is, and two engines saying the same thing there would be a bug.
/// The colour answers are normalised to 8 bits a channel, because
/// `rgb:ff/80/00` and `rgb:ffff/8080/0000` are the same colour and an engine
/// may spell either. A sequence this does not recognise is skipped, never
/// guessed at.
#[must_use]
pub fn comparable_replies(bytes: &[u8]) -> Vec<Reply> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0x1b || i + 1 >= bytes.len() {
            i += 1;
            continue;
        }
        match bytes[i + 1] {
            b'[' => {
                // CSI: parameters and intermediates up to a final byte in
                // 0x40..=0x7e.
                let start = i + 2;
                let Some(end) = bytes[start..]
                    .iter()
                    .position(|b| (0x40..=0x7e).contains(b))
                    .map(|p| start + p)
                else {
                    break;
                };
                if let Some(reply) = csi_reply(&bytes[start..end], bytes[end]) {
                    out.push(reply);
                }
                i = end + 1;
            }
            b']' => {
                // OSC: up to BEL or ST.
                let start = i + 2;
                let mut end = start;
                let mut after = None;
                while end < bytes.len() {
                    if bytes[end] == 0x07 {
                        after = Some(end + 1);
                        break;
                    }
                    if bytes[end] == 0x1b && bytes.get(end + 1) == Some(&b'\\') {
                        after = Some(end + 2);
                        break;
                    }
                    end += 1;
                }
                let Some(after) = after else { break };
                if let Some(reply) = osc_reply(&bytes[start..end]) {
                    out.push(reply);
                }
                i = after;
            }
            _ => i += 2,
        }
    }
    out
}

/// A CSI answer, from its parameters and final byte.
fn csi_reply(params: &[u8], last: u8) -> Option<Reply> {
    let text = std::str::from_utf8(params).ok()?;
    let numbers = |s: &str| -> Option<Vec<u32>> { s.split(';').map(|n| n.parse().ok()).collect() };
    match last {
        b'R' if !text.starts_with('?') => {
            let n = numbers(text)?;
            let [row, column] = n.as_slice() else {
                return None;
            };
            Some(Reply::Cursor {
                row: *row,
                column: *column,
            })
        }
        b'n' => {
            let n = numbers(text)?;
            let [status] = n.as_slice() else { return None };
            Some(Reply::Status(*status))
        }
        b'y' => {
            let body = text.strip_suffix('$')?;
            let (private, body) = match body.strip_prefix('?') {
                Some(rest) => (true, rest),
                None => (false, body),
            };
            let n = numbers(body)?;
            let [mode, state] = n.as_slice() else {
                return None;
            };
            Some(Reply::Mode {
                private,
                mode: *mode,
                state: *state,
            })
        }
        _ => None,
    }
}

/// An OSC colour answer: `4;<i>;rgb:…`, `10;rgb:…`, `11;…`, `12;…`.
fn osc_reply(body: &[u8]) -> Option<Reply> {
    let text = std::str::from_utf8(body).ok()?;
    let (slot, spec) = match text.split_once(';')? {
        ("4", rest) => {
            let (index, spec) = rest.split_once(';')?;
            (format!("4;{index}"), spec)
        }
        (number @ ("10" | "11" | "12"), spec) => (number.to_owned(), spec),
        _ => return None,
    };
    let channels: Vec<u8> = spec
        .strip_prefix("rgb:")?
        .split('/')
        .map(eight_bits)
        .collect::<Option<_>>()?;
    let [r, g, b] = channels.as_slice() else {
        return None;
    };
    Some(Reply::Colour {
        slot,
        rgb: (*r, *g, *b),
    })
}

/// One channel of an X11 colour spec, scaled to 8 bits: `h`, `hh`, `hhh` and
/// `hhhh` are 4, 8, 12 and 16 bits, and the high byte is what both engines
/// were set to.
fn eight_bits(hex: &str) -> Option<u8> {
    let value = u32::from_str_radix(hex, 16).ok()?;
    let bits = u32::try_from(hex.len()).ok()?.checked_mul(4)?;
    match bits {
        4 => u8::try_from(value * 0x11).ok(),
        8 => u8::try_from(value).ok(),
        12 | 16 => u8::try_from(value >> (bits - 8)).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_answers_are_left_out_of_the_comparison() {
        // FAILS IF DA1, DA2 or XTVERSION reach the cross-engine comparison:
        // they differ between engines by design, and comparing them would make
        // every case that asks fail forever.
        let bytes = b"\x1b[?62;4;22;52c\x1b[>1;10;0c\x1bP>|ronoctua 1\x1b\\\x1b[3;7R\x1b[0n";
        assert_eq!(
            comparable_replies(bytes),
            [Reply::Cursor { row: 3, column: 7 }, Reply::Status(0)]
        );
    }

    #[test]
    fn mode_answers_keep_their_kind() {
        // FAILS IF a DEC private mode and an ANSI mode of the same number are
        // read as one: `?4` (smooth scroll) and `4` (insert) are different modes.
        assert_eq!(
            comparable_replies(b"\x1b[?4;2$y\x1b[4;1$y"),
            [
                Reply::Mode {
                    private: true,
                    mode: 4,
                    state: 2
                },
                Reply::Mode {
                    private: false,
                    mode: 4,
                    state: 1
                },
            ]
        );
    }

    #[test]
    fn colour_answers_are_compared_in_eight_bits() {
        // FAILS IF two spellings of the same colour count as two colours:
        // alacritty writes 16 bits a channel, and an engine writing 8 is
        // answering the same thing.
        let long = comparable_replies(b"\x1b]11;rgb:1010/1010/1010\x1b\\");
        let short = comparable_replies(b"\x1b]11;rgb:10/10/10\x07");
        assert_eq!(long, short);
        assert_eq!(
            comparable_replies(b"\x1b]4;1;rgb:cdcd/0000/0000\x07"),
            [Reply::Colour {
                slot: "4;1".to_owned(),
                rgb: (0xcd, 0, 0)
            }]
        );
    }

    #[test]
    fn a_cut_answer_is_not_guessed_at() {
        // FAILS IF a half-written sequence at the end of the bytes is read as
        // something: a reply the engine never finished is no reply.
        assert!(comparable_replies(b"\x1b[3;").is_empty());
        assert!(comparable_replies(b"\x1b]10;rgb:ff/ff").is_empty());
    }
}
