//! The protocol: framing, and the words a command is made of.
//!
//! This is the only file in the repository that knows the compositor's
//! spelling. Everything above it speaks `ronoctua_compositor`'s vocabulary.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use anyhow::{Context as _, Result, bail};
use ronoctua_compositor::{
    AccelProfile, Arity, Ask, ButtonMap, ClickMethod, Corner, Deed, DragLock, FingerDrag,
    FocusFollows, KeyboardSetting, Portion, ScrollMethod, SeatAsk, SendEvents, Setting, Transform,
};

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
    /// Every input device.
    Inputs = 100,
    /// Every seat.
    Seats = 101,
    /// How the focused workspace is laid out: its scale, its overview.
    Scroller = 120,
    /// The trails: how many, which one is in use, how many windows it holds.
    Trails = 121,
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
/// A device was added, removed or reconfigured.
pub const EVENT_INPUT: u32 = EVENT_BIT | 21;
/// A key mode was entered or left.
pub const EVENT_MODE: u32 = EVENT_BIT | 2;
/// A workspace appeared, went, took the focus, moved or was renamed.
pub const EVENT_WORKSPACE: u32 = EVENT_BIT;
/// Something about how a workspace is laid out changed — its overview among
/// them.
pub const EVENT_SCROLLER: u32 = EVENT_BIT | 30;
/// A trail was made, chosen, emptied or deleted, or a window was put on one or
/// taken off.
pub const EVENT_TRAILS: u32 = EVENT_BIT | 31;

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

/// The room windows have on a screen: a workspace's `rect`, which is the
/// output less every exclusive zone — the footer and the right column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    /// Its left edge, in the layout's logical pixels.
    pub x: i64,
    /// Its top edge.
    pub y: i64,
    /// Its width.
    pub width: i64,
    /// Its height.
    pub height: i64,
}

/// The commands that float a window in the bottom-right corner of `room`,
/// its CONTENT `corner`'s size: `decoration` is what border and title bar
/// add to it (`read::decoration_in`), because `resize set` sizes the
/// container — with the title bar the VM draws, the picture came out 19
/// pixels short.
///
/// ONE LINE, AND COMMAS: a criteria holds for every command after it up to
/// a `;`, so the four act on the one window. `sticky` keeps it on every
/// workspace, which is what a picture-in-picture is for. `absolute`, because
/// the room is in the layout's coordinates and a plain `move position` is
/// counted from the workspace's corner.
#[must_use]
pub fn corner_command(id: u64, corner: Corner, room: Room, decoration: (i64, i64)) -> String {
    let (w, h, air) = (
        i64::from(corner.width) + decoration.0,
        i64::from(corner.height) + decoration.1,
        i64::from(corner.air),
    );
    let x = room.x + (room.width - w - air).max(0);
    let y = room.y + (room.height - h - air).max(0);
    format!(
        "[con_id={id}] floating enable, sticky enable, \
         resize set width {w} px height {h} px, move absolute position {x} px {y} px"
    )
}

/// The commands that float a window in the MIDDLE of the room windows have,
/// `portion` of it in size — the SAME words the generated rule that opens a
/// floating window speaks (`floating::rules` in the shell): `ppt` of the
/// workspace, which is the output less every exclusive zone (the footer, the
/// right column), and `move position center` in it — the centring
/// [`fitted_command`] relies on too. One unit for the window that opens
/// floating and for the window floated again, so the two land on one size
/// and one place. Not sticky: it stays on its workspace, unlike the corner's
/// picture.
#[must_use]
pub fn centred_command(id: u64, portion: Portion) -> String {
    let (w, h) = (portion.width_ppt.min(100), portion.height_ppt.min(100));
    format!(
        "[con_id={id}] floating enable, \
         resize set width {w} ppt height {h} ppt, move position center"
    )
}

/// The commands that put a window floated by [`corner_command`] back.
#[must_use]
pub fn laid_command(id: u64) -> String {
    format!("[con_id={id}] sticky disable, floating disable")
}

/// A command made to act on one window, by CRITERIA and not by focus.
///
/// # Why not focus it first
///
/// That was the first form (`[con_id=N] focus; fullscreen`), and it put a
/// window into fullscreen at the place it had been drawn: focusing a column
/// ANIMATES the layout towards it, a fullscreen window is placed by its
/// animation (`transaction.c`, `fs->animation.xt`), and the second command
/// landed in the middle of the first one's. The owner photographed a
/// fullscreen window hanging off the right edge. Every command the window menu
/// sends reads its target from the criteria (`config->handler_context`), so
/// nothing needs the focus to move — except a key MODE, whose keys go to
/// whatever is focused, which is the one case focused first.
///
/// A criteria binds the first command of a `;` list; the second of the one
/// deed with two (move and follow) is `workspace number N`, which needs no
/// window.
#[must_use]
pub fn on_the_window(window: u64, command: &str) -> String {
    if command.starts_with("mode ") {
        format!("[con_id={window}] focus; {command}")
    } else {
        format!("[con_id={window}] {command}")
    }
}

/// The commands that keep a window taken out of the drawer inside `room`:
/// shrunk to it when it is bigger, then centred in it.
#[must_use]
pub fn fitted_command(id: u64, room: Room, size: Option<(i64, i64)>) -> String {
    match size {
        Some((w, h)) if w > room.width || h > room.height => format!(
            "[con_id={id}] resize set width {} px height {} px, move position center",
            w.min(room.width),
            h.min(room.height)
        ),
        _ => format!("[con_id={id}] move position center"),
    }
}

/// The command that takes one window out of the drawer.
///
/// `scratchpad show` with criteria shows the windows that MATCH, and a
/// `con_id` matches one — the one the pointer chose.
#[must_use]
pub fn taken_out_command(id: u64) -> String {
    format!("[con_id={id}] scratchpad show")
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

/// A word that can stand in a command without quotes: what an identifier or
/// a seat name must be before it is put between them.
///
/// An identifier is `vendor:product:name` with the name's spaces already
/// turned into `_` by the compositor, so a legitimate one never needs
/// anything outside this set — and a quote, `;` or `,` inside one would end
/// the command early.
fn safe_word(w: &str) -> Result<&str> {
    if !w.is_empty()
        && w.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._:+-/".contains(c))
    {
        Ok(w)
    } else {
        bail!("`{w}` is not a name a command can carry")
    }
}

const fn on_off(b: bool) -> &'static str {
    if b { "enabled" } else { "disabled" }
}

const fn map_word(m: ButtonMap) -> &'static str {
    match m {
        ButtonMap::Lrm => "lrm",
        ButtonMap::Lmr => "lmr",
    }
}

/// The command that sets one [`Setting`] on one device.
pub fn input_command(device: &str, setting: Setting) -> Result<String> {
    let at = safe_word(device)?;
    let tail = match setting {
        Setting::SendEvents(e) => format!(
            "events {}",
            match e {
                SendEvents::Enabled => "enabled",
                SendEvents::Disabled => "disabled",
                SendEvents::DisabledOnExternalMouse => "disabled_on_external_mouse",
            }
        ),
        // THE DOT IS THE PARSER'S, never the locale's: `parse_float` is
        // `strtof`, and a comma would end the command besides.
        Setting::AccelSpeed(v) => format!("pointer_accel {:.3}", v.clamp(-1.0, 1.0)),
        Setting::AccelProfile(p) => format!(
            "accel_profile {}",
            match p {
                AccelProfile::Flat => "flat",
                AccelProfile::Adaptive => "adaptive",
            }
        ),
        Setting::NaturalScroll(b) => format!("natural_scroll {}", on_off(b)),
        Setting::LeftHanded(b) => format!("left_handed {}", on_off(b)),
        Setting::Tap(b) => format!("tap {}", on_off(b)),
        Setting::TapButtonMap(m) => format!("tap_button_map {}", map_word(m)),
        Setting::TapDrag(b) => format!("drag {}", on_off(b)),
        Setting::TapDragLock(l) => format!(
            "drag_lock {}",
            match l {
                DragLock::Disabled => "disabled",
                DragLock::Enabled => "enabled",
                DragLock::Sticky => "enabled_sticky",
            }
        ),
        Setting::FingerDrag(d) => format!(
            "three_finger_drag {}",
            match d {
                FingerDrag::Disabled => "disabled",
                FingerDrag::ThreeFinger => "three_finger",
                FingerDrag::FourFinger => "four_finger",
            }
        ),
        Setting::ClickMethod(m) => format!(
            "click_method {}",
            match m {
                ClickMethod::None => "none",
                ClickMethod::ButtonAreas => "button_areas",
                ClickMethod::Clickfinger => "clickfinger",
            }
        ),
        Setting::ClickfingerButtonMap(m) => format!("clickfinger_button_map {}", map_word(m)),
        Setting::MiddleEmulation(b) => format!("middle_emulation {}", on_off(b)),
        Setting::ScrollMethod(m) => format!(
            "scroll_method {}",
            match m {
                ScrollMethod::None => "none",
                ScrollMethod::TwoFinger => "two_finger",
                ScrollMethod::Edge => "edge",
                ScrollMethod::OnButtonDown => "on_button_down",
            }
        ),
        // A LINUX EVENT CODE, which `get_mouse_bindcode` takes as a number;
        // zero is the parser's own word for "none".
        Setting::ScrollButton(0) => "scroll_button disable".to_owned(),
        Setting::ScrollButton(code) => format!("scroll_button {code}"),
        Setting::ScrollButtonLock(b) => format!("scroll_button_lock {}", on_off(b)),
        Setting::DisableWhileTyping(b) => format!("dwt {}", on_off(b)),
        Setting::DisableWhileTrackpointing(b) => format!("dwtp {}", on_off(b)),
        Setting::ScrollFactor(v) => format!("scroll_factor {:.3}", v.clamp(0.1, 10.0)),
    };
    Ok(format!("input \"{at}\" {tail}"))
}

/// An xkb value as a command can carry it: layouts, variants and options are
/// letters, digits and `_,:+-()` — `compose:caps,shift:both_capslock` — and
/// the empty string, which is "the default".
fn xkb_word(w: &str) -> Result<&str> {
    if w.chars()
        .all(|c| c.is_ascii_alphanumeric() || "_,:+-()".contains(c))
    {
        Ok(w)
    } else {
        bail!("`{w}` is not an xkb value a command can carry")
    }
}

/// The command that sets one [`KeyboardSetting`] on one device.
///
/// QUOTED, and an empty value is the quotes alone: `xkb_variant ""` is the
/// parser's own way of saying "none", which a bare word could not say.
pub fn keyboard_command(device: &str, setting: &KeyboardSetting) -> Result<String> {
    let at = safe_word(device)?;
    let tail = match setting {
        KeyboardSetting::Layout(l) => format!("xkb_layout \"{}\"", xkb_word(l)?),
        KeyboardSetting::Variant(v) => format!("xkb_variant \"{}\"", xkb_word(v)?),
        KeyboardSetting::Options(o) => format!("xkb_options \"{}\"", xkb_word(o)?),
        KeyboardSetting::RepeatRate(r) => format!("repeat_rate {r}"),
        KeyboardSetting::RepeatDelay(d) => format!("repeat_delay {d}"),
    };
    Ok(format!("input \"{at}\" {tail}"))
}

/// The command that asks one thing of one seat.
pub fn seat_command(seat: &str, ask: &SeatAsk) -> Result<String> {
    // `*` IS THE COMPOSITOR'S "EVERY SEAT", and it is the one word outside the
    // safe set a seat name may be: it cannot end a command.
    let at = if seat == "*" { seat } else { safe_word(seat)? };
    let tail = match ask {
        SeatAsk::Attach(id) => format!("attach \"{}\"", safe_word(id)?),
        SeatAsk::Cursor { theme, size } => {
            format!("xcursor_theme {} {size}", safe_word(theme)?)
        }
        SeatAsk::HideAfter(ms) => format!("hide_cursor {ms}"),
        SeatAsk::HideWhileTyping(b) => format!(
            "hide_cursor when-typing {}",
            if *b { "enable" } else { "disable" }
        ),
    };
    Ok(format!("seat {at} {tail}"))
}

/// The command for focus-follows-mouse.
#[must_use]
pub const fn focus_command(f: FocusFollows) -> &'static str {
    match f {
        FocusFollows::No => "focus_follows_mouse no",
        FocusFollows::Yes => "focus_follows_mouse yes",
        FocusFollows::Always => "focus_follows_mouse always",
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

    #[test]
    fn a_keyboard_setting_is_quoted_and_an_empty_one_is_the_default() {
        let twin = "0:0:ronoctua_keyboard_0a1b2c3d";
        assert_eq!(
            keyboard_command(twin, &KeyboardSetting::Layout("gb".into())).unwrap(),
            "input \"0:0:ronoctua_keyboard_0a1b2c3d\" xkb_layout \"gb\""
        );
        assert_eq!(
            keyboard_command(twin, &KeyboardSetting::Variant(String::new())).unwrap(),
            "input \"0:0:ronoctua_keyboard_0a1b2c3d\" xkb_variant \"\""
        );
        assert!(
            keyboard_command(
                twin,
                &KeyboardSetting::Options("compose:caps,shift:both_capslock".into())
            )
            .is_ok()
        );
        assert!(keyboard_command(twin, &KeyboardSetting::Layout("gb\"; exit".into())).is_err());
    }
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
    fn every_setting_is_one_command_with_no_separator() {
        use ronoctua_compositor::PointerSettings;
        let all = PointerSettings {
            send_events: Some(SendEvents::DisabledOnExternalMouse),
            accel_speed: Some(-0.25),
            accel_profile: Some(AccelProfile::Flat),
            natural_scroll: Some(true),
            left_handed: Some(false),
            tap: Some(true),
            tap_button_map: Some(ButtonMap::Lmr),
            tap_drag: Some(true),
            tap_drag_lock: Some(DragLock::Sticky),
            finger_drag: Some(FingerDrag::ThreeFinger),
            click_method: Some(ClickMethod::Clickfinger),
            clickfinger_button_map: Some(ButtonMap::Lrm),
            middle_emulation: Some(true),
            scroll_method: Some(ScrollMethod::OnButtonDown),
            scroll_button: Some(274),
            scroll_button_lock: Some(true),
            disable_while_typing: Some(true),
            disable_while_trackpointing: Some(false),
            scroll_factor: Some(1.5),
        };
        let id = "1133:50498:Logitech_Wireless_Receiver_Mouse";
        for s in all.present() {
            let said = input_command(id, s).unwrap();
            assert!(said.starts_with(&format!("input \"{id}\" ")), "{said}");
            assert!(!said.contains(',') && !said.contains(';'), "{said}");
        }
        assert_eq!(
            input_command(id, Setting::AccelSpeed(-0.25)).unwrap(),
            format!("input \"{id}\" pointer_accel -0.250")
        );
        assert_eq!(
            input_command(id, Setting::TapDragLock(DragLock::Sticky)).unwrap(),
            format!("input \"{id}\" drag_lock enabled_sticky")
        );
    }

    #[test]
    fn a_name_that_could_end_the_command_is_refused() {
        assert!(input_command("a\";exit", Setting::Tap(true)).is_err());
        assert!(seat_command("seat1; exit", &SeatAsk::HideAfter(0)).is_err());
        assert!(seat_command("seat1", &SeatAsk::Attach("x,y".into())).is_err());
        assert_eq!(
            seat_command("ronoctua-cursor-2", &SeatAsk::Attach("1:2:Mouse".into())).unwrap(),
            "seat ronoctua-cursor-2 attach \"1:2:Mouse\""
        );
        assert_eq!(
            seat_command(
                "seat0",
                &SeatAsk::Cursor {
                    theme: "ronoctua-owl-1".into(),
                    size: 24
                }
            )
            .unwrap(),
            "seat seat0 xcursor_theme ronoctua-owl-1 24"
        );
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

    /// FAILS IF: the picture-in-picture leaves the room windows have — over
    /// the footer or the column —, or the four commands stop acting on the
    /// one window.
    #[test]
    fn the_corner_is_of_the_room_and_one_window() {
        let corner = Corner {
            width: 320,
            height: 180,
            air: 16,
        };
        let room = Room {
            x: 1920,
            y: 0,
            width: 1500,
            height: 1000,
        };
        assert_eq!(
            corner_command(7, corner, room, (0, 0)),
            "[con_id=7] floating enable, sticky enable, resize set width 320 px height 180 px, \
             move absolute position 3084 px 804 px"
        );
        assert!(!corner_command(7, corner, room, (0, 0)).contains(';'));
        // BORDER AND TITLE BAR are added to the content, and the corner is
        // the container's.
        assert!(
            corner_command(7, corner, room, (4, 23))
                .contains("width 324 px height 203 px, move absolute position 3080 px 781 px")
        );
        // A ROOM SMALLER THAN THE PICTURE keeps it at the room's corner, not
        // before it.
        let tiny = Room {
            width: 100,
            height: 100,
            ..room
        };
        assert!(corner_command(7, corner, tiny, (0, 0)).ends_with("position 1920 px 0 px"));
        assert_eq!(
            laid_command(7),
            "[con_id=7] sticky disable, floating disable"
        );
    }

    /// A window floated in the middle takes its portion of the room in the
    /// rule's own words, and is centred in it.
    #[test]
    fn a_centred_window_is_a_portion_of_the_room() {
        let portion = Portion {
            width_ppt: 30,
            height_ppt: 60,
        };
        assert_eq!(
            centred_command(5, portion),
            "[con_id=5] floating enable, resize set width 30 ppt height 60 ppt, \
             move position center"
        );
        // NEVER LARGER THAN THE ROOM, whatever the table says.
        let whole = Portion {
            width_ppt: 250,
            height_ppt: 100,
        };
        assert!(centred_command(5, whole).contains("width 100 ppt height 100 ppt"));
        // AND NOT STICKY: it stays on its workspace.
        assert!(!centred_command(5, portion).contains("sticky"));
    }

    /// One window out of the drawer, the one named — and one command, so
    /// nothing after it rides along.
    #[test]
    fn taking_out_names_the_window() {
        assert_eq!(taken_out_command(9), "[con_id=9] scratchpad show");
        assert!(!taken_out_command(9).contains(';'));
    }

    /// A window out of the drawer is kept in the room windows have: centred,
    /// and shrunk first only when it is bigger.
    #[test]
    fn a_window_out_of_the_drawer_fits_the_room() {
        let room = Room {
            x: 0,
            y: 0,
            width: 1920,
            height: 1000,
        };
        assert_eq!(
            fitted_command(4, room, Some((800, 600))),
            "[con_id=4] move position center"
        );
        assert_eq!(
            fitted_command(4, room, Some((800, 1080))),
            "[con_id=4] resize set width 800 px height 1000 px, move position center"
        );
        assert_eq!(
            fitted_command(4, room, None),
            "[con_id=4] move position center"
        );
    }

    /// A deed on one window names it by criteria and does not move the focus
    /// — focusing animated the layout under a fullscreen — except a key mode,
    /// whose keys go to the focused window.
    #[test]
    fn a_deed_on_a_window_names_it_and_focuses_only_for_a_mode() {
        assert_eq!(on_the_window(7, "fullscreen"), "[con_id=7] fullscreen");
        assert_eq!(
            on_the_window(
                7,
                "move container to workspace number 3; workspace number 3"
            ),
            "[con_id=7] move container to workspace number 3; workspace number 3"
        );
        assert_eq!(
            on_the_window(7, r#"mode "resize""#),
            r#"[con_id=7] focus; mode "resize""#
        );
    }
}
