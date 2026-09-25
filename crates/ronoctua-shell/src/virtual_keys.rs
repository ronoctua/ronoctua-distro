//! Keys typed into whatever window has the focus — the on-screen keyboard's
//! one door to the compositor.
//!
//! # Why this is the one file that names the protocol
//!
//! Invariant 3 keeps this shell to IPC and layer-shell. `zwp_virtual_keyboard_v1`
//! is the owner's approved exception (2026-09-25), of the same kind as
//! `ext-image-copy-capture-v1` and `wl_data_device`: a STANDARD Wayland
//! protocol, not a patch and not a fork. The pinned compositor creates the
//! manager and attaches every keyboard made through it to a seat, where it is
//! one more keyboard of that seat — its keys reach the focused window, the
//! compositor's own bindings see them, and a window's repeat applies to them.
//! `one-virtual-keyboard` keeps every other file from naming it.
//!
//! # Why the keymap is OURS
//!
//! Because the language is chosen on the keyboard itself. A virtual keyboard
//! carries its own keymap, compiled here from the layout the person picked,
//! and the compositor hands that keymap to the window whenever a key comes
//! from this device. So `br` types `ç` on a machine whose own keyboard is `gb`,
//! and dead keys compose in the receiving window exactly as a physical ABNT2
//! keyboard's would.
//!
//! # Why modifiers go out as KEYS
//!
//! A latched Ctrl is a Ctrl key held down until the next key is released. Sent
//! as a key, the compositor's own xkb state computes every mask — `AltGr` is
//! level three in one layout and a plain Alt in another, and nothing on this
//! side has to know which.

use std::io::Write as _;
use std::os::fd::AsFd as _;

use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, Proxy as _, QueueHandle, globals::GlobalList, protocol::wl_seat::WlSeat,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::{self, ZwpVirtualKeyboardManagerV1},
    zwp_virtual_keyboard_v1::{self, ZwpVirtualKeyboardV1},
};

/// The user data both objects carry. Neither sends any event.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quiet;

impl<S> Dispatch2<ZwpVirtualKeyboardManagerV1, S> for Quiet {
    fn event(
        &self,
        _: &mut S,
        _: &ZwpVirtualKeyboardManagerV1,
        _: zwp_virtual_keyboard_manager_v1::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
    }
}

impl<S> Dispatch2<ZwpVirtualKeyboardV1, S> for Quiet {
    fn event(
        &self,
        _: &mut S,
        _: &ZwpVirtualKeyboardV1,
        _: zwp_virtual_keyboard_v1::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
    }
}

/// `WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1`.
const XKB_V1: u32 = 1;

/// The virtual keyboard of the seat the last key was pressed on.
#[derive(Debug)]
struct Made {
    seat: WlSeat,
    keyboard: ZwpVirtualKeyboardV1,
    /// The keymap it was last given, so a language that did not change is not
    /// uploaded again.
    keymap: Option<String>,
    /// The kernel codes it holds DOWN right now. Moving to another seat, or
    /// changing the keymap, lets every one of them go first — a key held on a
    /// keymap that no longer exists is a key that never comes up.
    held: Vec<u16>,
}

/// The manager, and the one keyboard in use.
#[derive(Debug)]
pub struct Keys {
    manager: ZwpVirtualKeyboardManagerV1,
    made: Option<Made>,
    began: std::time::Instant,
}

impl Keys {
    /// Binds the manager, or answers `None` where the compositor has none.
    ///
    /// ABSENCE IS NOT AN ERROR, the rule `carry::Carrying::bind` follows: the
    /// keyboard is drawn and answers the pointer, and a key says on screen
    /// that it reaches nothing.
    pub fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Option<Self>
    where
        D: Dispatch<ZwpVirtualKeyboardManagerV1, Quiet> + 'static,
    {
        match globals.bind::<ZwpVirtualKeyboardManagerV1, D, Quiet>(qh, 1..=1, Quiet) {
            Ok(manager) => Some(Self {
                manager,
                made: None,
                began: std::time::Instant::now(),
            }),
            Err(e) => {
                tracing::info!(error = %e, "no virtual keyboard manager; the on-screen keyboard types nothing");
                None
            }
        }
    }

    /// Makes sure the keyboard of `seat` exists and carries `keymap`.
    pub fn ready<D>(&mut self, qh: &QueueHandle<D>, seat: &WlSeat, keymap: &str)
    where
        D: Dispatch<ZwpVirtualKeyboardV1, Quiet> + 'static,
    {
        if self.made.as_ref().is_some_and(|m| m.seat.id() != seat.id()) {
            self.let_everything_go();
            if let Some(m) = self.made.take() {
                m.keyboard.destroy();
            }
        }
        if self.made.is_none() {
            let keyboard = self.manager.create_virtual_keyboard(seat, qh, Quiet);
            self.made = Some(Made {
                seat: seat.clone(),
                keyboard,
                keymap: None,
                held: Vec::new(),
            });
        }
        let changed = self
            .made
            .as_ref()
            .is_some_and(|m| m.keymap.as_deref() != Some(keymap));
        if changed {
            self.let_everything_go();
            if let Some(m) = self.made.as_mut() {
                match upload(&m.keyboard, keymap) {
                    Ok(()) => m.keymap = Some(keymap.to_owned()),
                    Err(e) => tracing::warn!(error = %e, "the keymap could not be handed over"),
                }
            }
        }
    }

    /// One key, down or up, by its KERNEL code.
    pub fn key(&mut self, code: u16, down: bool) {
        let time = self.now();
        let Some(m) = self.made.as_mut() else {
            return;
        };
        if m.keymap.is_none() {
            return;
        }
        if down {
            if !m.held.contains(&code) {
                m.held.push(code);
            }
        } else {
            m.held.retain(|c| *c != code);
        }
        m.keyboard.key(time, u32::from(code), u32::from(down));
    }

    /// Lets go of every key held down — before a keymap changes, a seat
    /// changes, or the keyboard is put away.
    pub fn let_everything_go(&mut self) {
        let time = self.now();
        if let Some(m) = self.made.as_mut() {
            for code in std::mem::take(&mut m.held) {
                m.keyboard.key(time, u32::from(code), 0);
            }
        }
    }

    /// Milliseconds, from a clock of our own: the protocol only asks that they
    /// go forward.
    fn now(&self) -> u32 {
        u32::try_from(self.began.elapsed().as_millis() % u128::from(u32::MAX)).unwrap_or(0)
    }
}

/// Hands `keymap` over in a sealed-size memory file, which is what the
/// protocol reads it from.
fn upload(keyboard: &ZwpVirtualKeyboardV1, keymap: &str) -> std::io::Result<()> {
    let fd = rustix::fs::memfd_create("ronoctua-keymap", rustix::fs::MemfdFlags::CLOEXEC)?;
    let mut file = std::fs::File::from(fd);
    // THE TERMINATING NUL, which the size counts: the compositor maps the file
    // and hands it to `xkb_keymap_new_from_string`, which reads a C string.
    file.write_all(keymap.as_bytes())?;
    file.write_all(&[0])?;
    file.flush()?;
    let size = u32::try_from(keymap.len() + 1).unwrap_or(u32::MAX);
    keyboard.keymap(XKB_V1, file.as_fd(), size);
    Ok(())
}
