//! Independent keyboards, from the footer's side: the leader over a socket.
//!
//! # Why the footer does not take the Wayland keyboard in this mode
//!
//! The compositor hands an exclusive layer surface EVERY seat's keyboard at
//! once (`layer_shell.c:130` in the pinned source) and has no way to focus a
//! layer for one seat. So in Independent mode the footer asks for no keyboard
//! at all: `ronoctua-keys` says which keyboard tapped its leader, and while the
//! footer's region is open that ONE keyboard is diverted to it over the
//! socket. Every other keyboard goes on typing into its own window.
//!
//! # The keys arrive as the kernel's codes
//!
//! After that keyboard's remaps, and before any layout: the daemon never
//! translates. So each diverted keyboard is translated here with ITS layout —
//! its own `xkb::State`, the same library SCTK compiles every Wayland keymap
//! with — and handed to the footer as the `KeyEvent` a Wayland keyboard would
//! have given it. The footer's key path does not know the difference.

use smithay_client_toolkit::reexports::calloop::channel;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use xkbcommon::xkb;

use ronoctua_keys::client::Link;
use ronoctua_keys::wire::FromDaemon;

/// What the socket's thread tells the footer.
pub enum Said {
    /// Connected: the half that asks.
    Connected(Link),
    /// A message.
    Daemon(FromDaemon),
    /// The daemon went away — Together mode, or it died.
    Gone,
}

/// Keeps a connection to the daemon, reconnecting every two seconds.
///
/// A connection that fails costs one `connect` on a path that does not exist
/// — Together mode, which is most machines, pays that and nothing else.
pub fn listen(to: channel::Sender<Said>) {
    let spawned = std::thread::Builder::new()
        .name("keys-link".to_owned())
        .spawn(move || {
            loop {
                if let Ok((link, messages)) = Link::open() {
                    if to.send(Said::Connected(link)).is_err() {
                        return;
                    }
                    for m in messages {
                        if to.send(Said::Daemon(m)).is_err() {
                            return;
                        }
                    }
                    if to.send(Said::Gone).is_err() {
                        return;
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "no thread for the keys daemon; Independent mode has no leader");
    }
}

/// The keyboard holding the footer's region, and its layout's state.
pub struct Owner {
    /// Its identity.
    pub unit: String,
    state: xkb::State,
}

impl std::fmt::Debug for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Owner")
            .field("unit", &self.unit)
            .finish_non_exhaustive()
    }
}

impl Owner {
    /// A unit about to be diverted, with its layout — its own, or the
    /// machine's (`RONOCTUA_XKB_*`).
    #[must_use]
    pub fn of(unit: &str) -> Option<Self> {
        let prefs = ronoctua_keyboards::store().read();
        let own = prefs.units.get(unit);
        let machine = ronoctua_pointers::keyboards::Machine::from_env();
        let layout = own
            .and_then(|u| u.layout.clone())
            .filter(|l| !l.is_empty())
            .unwrap_or(machine.layout.clone());
        let variant = if own.and_then(|u| u.layout.as_ref()).is_some() {
            own.and_then(|u| u.variant.clone()).unwrap_or_default()
        } else {
            machine.variant.clone()
        };
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            &layout,
            &variant,
            Some(machine.options),
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )?;
        Some(Self {
            unit: unit.to_owned(),
            state: xkb::State::new(&keymap),
        })
    }

    /// One diverted key: the event the footer's key path takes, whether it
    /// went down (a repeat counts), and the modifiers after it.
    pub fn key(&mut self, code: u16, value: i32) -> (KeyEvent, bool, Modifiers) {
        // THE KERNEL'S CODE PLUS EIGHT, which is what an xkb keycode is.
        let keycode = xkb::Keycode::new(u32::from(code) + 8);
        let down = value != 0;
        let keysym = self.state.key_get_one_sym(keycode);
        let text = self.state.key_get_utf8(keycode);
        if value != 2 {
            self.state.update_key(
                keycode,
                if down {
                    xkb::KeyDirection::Down
                } else {
                    xkb::KeyDirection::Up
                },
            );
        }
        let on = |name: &str| {
            self.state
                .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
        };
        let mods = Modifiers {
            ctrl: on(xkb::MOD_NAME_CTRL),
            alt: on(xkb::MOD_NAME_ALT),
            shift: on(xkb::MOD_NAME_SHIFT),
            caps_lock: on(xkb::MOD_NAME_CAPS),
            logo: on(xkb::MOD_NAME_LOGO),
            num_lock: on(xkb::MOD_NAME_NUM),
        };
        let event = KeyEvent {
            time: 0,
            raw_code: u32::from(code) + 8,
            keysym,
            utf8: (down && !text.is_empty()).then_some(text),
        };
        (event, down, mods)
    }
}

/// The footer's side of Independent mode: whether it is on, the connection,
/// and the keyboard holding the region.
#[derive(Debug, Default)]
pub struct Keys {
    /// Independent mode — the footer then never asks for a Wayland keyboard.
    pub independent: bool,
    /// The half that asks the daemon.
    pub link: Option<Link>,
    /// The keyboard the open region belongs to.
    pub owner: Option<Owner>,
}

impl Keys {
    /// Asks the daemon one thing; a dead link is dropped, and the thread
    /// reconnects.
    pub fn ask(&mut self, ask: &ronoctua_keys::wire::ToDaemon) {
        if let Some(l) = self.link.as_mut()
            && let Err(e) = l.ask(ask)
        {
            tracing::warn!(error = %e, "the keys daemon did not take the ask");
            self.link = None;
        }
    }

    /// Gives the owning keyboard back to its twin.
    pub fn release(&mut self) {
        if let Some(o) = self.owner.take() {
            self.ask(&ronoctua_keys::wire::ToDaemon::Release { unit: o.unit });
        }
    }
}

/// Is Independent mode on — read from the keyboards' own file, which is the
/// broadcast every door reads.
#[must_use]
pub fn independent() -> bool {
    ronoctua_keyboards::store().read().mode == ronoctua_keyboards::Mode::Independent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diverted_key_is_what_its_layout_says() {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let Some(keymap) = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            // No xkb data in this sandbox: the translation is the library's.
            return;
        };
        let mut o = Owner {
            unit: "x".into(),
            state: xkb::State::new(&keymap),
        };
        // KEY_A is 30.
        let (e, down, _) = o.key(30, 1);
        assert!(down);
        assert_eq!(e.utf8.as_deref(), Some("a"));
        // Shift (42) held makes it capital.
        let _ = o.key(42, 1);
        let (e, _, mods) = o.key(30, 1);
        assert!(mods.shift);
        assert_eq!(e.utf8.as_deref(), Some("A"));
        let (e, down, _) = o.key(30, 0);
        assert!(!down);
        assert!(e.utf8.is_none(), "a release carries no text");
    }
}
