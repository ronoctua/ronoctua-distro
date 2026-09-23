//! The kernel's two keyboard interfaces: a device's events, and a device made.
//!
//! # The one `unsafe` in this crate, and why it cannot be avoided
//!
//! Taking a keyboard for ourselves (`EVIOCGRAB`) and making a virtual one
//! (`UI_SET_EVBIT`, `UI_SET_KEYBIT`, `UI_DEV_SETUP`, `UI_DEV_CREATE`) are
//! ioctls, and `rustix` has no typed wrapper for any of them. So this module
//! allows `unsafe_code` and the workspace warns everywhere else — the
//! `ronoctua-display::ddc` arrangement, and the same reasoning.
//!
//! # Three things that are silent when wrong
//!
//! 1. **An event is 24 bytes on a 64-bit kernel** — a `timeval` of two `long`s,
//!    then `u16` type, `u16` code, `i32` value. Read as anything else, the
//!    codes come out shifted and every key is some other key.
//! 2. **A key the virtual device did not declare is dropped by the kernel**,
//!    with no error. So every key code up to `KEY_MAX` is declared — a media
//!    key on some keyboard nobody tested is still a media key — EXCEPT the
//!    buttons (`codes::is_a_button`): a device with `BTN_LEFT` is a MOUSE to
//!    libinput, and measured in the VM every twin came up twice, a keyboard
//!    and a pointer, and took a cursor of its own.
//! 3. **A grab is released by closing the descriptor**, and by nothing else.
//!    That is the safety of this whole mode: a daemon that dies — any way at
//!    all — gives every keyboard back to the compositor, because the kernel
//!    closes its descriptors.

// THE ONE PLACE. See the module's header.
#![allow(unsafe_code)]

use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;

use anyhow::{Context as _, Result};
use rustix::ioctl::{self, IntegerSetter, NoArg, Opcode, Setter, opcode};

/// `EV_SYN`, `EV_KEY`.
const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;

/// The size of one `struct input_event` on a 64-bit kernel.
const EVENT: usize = 24;

/// `struct uinput_setup` — `input_id` (four `u16`), `name[80]`,
/// `ff_effects_max`.
#[repr(C)]
struct UinputSetup {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
    name: [u8; 80],
    ff_effects_max: u32,
}

const EVIOCGRAB: Opcode = opcode::write::<rustix::ffi::c_int>(b'E', 0x90);
const UI_SET_EVBIT: Opcode = opcode::write::<rustix::ffi::c_int>(b'U', 100);
const UI_SET_KEYBIT: Opcode = opcode::write::<rustix::ffi::c_int>(b'U', 101);
const UI_DEV_SETUP: Opcode = opcode::write::<UinputSetup>(b'U', 3);
const UI_DEV_CREATE: Opcode = opcode::none(b'U', 1);

/// `BUS_VIRTUAL`.
const BUS_VIRTUAL: u16 = 0x06;

/// A physical keyboard, taken for ourselves.
#[derive(Debug)]
pub struct Grabbed {
    file: File,
    left: Vec<u8>,
}

impl Grabbed {
    /// Opens `/dev/input/<event>` and grabs it: from here on only this reads
    /// it, and the compositor stops seeing it.
    ///
    /// # Errors
    ///
    /// The node could not be opened, or somebody else has it grabbed.
    pub fn take(event: &str) -> Result<Self> {
        let path = format!("/dev/input/{event}");
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open(&path)
            .with_context(|| format!("opening {path}"))?;
        // SAFETY: EVIOCGRAB is `_IOW('E', 0x90, int)` and the evdev driver
        // takes the argument as the integer itself (1 grabs, 0 releases); it
        // reads and writes no memory of ours. On a node that is not evdev the
        // kernel answers ENOTTY, which arrives as the error below.
        unsafe { ioctl::ioctl(&file, IntegerSetter::<EVIOCGRAB>::new_usize(1)) }
            .with_context(|| format!("grabbing {path} — is another remapper holding it?"))?;
        Ok(Self {
            file,
            left: Vec::new(),
        })
    }

    /// The descriptor, for `poll`.
    #[must_use]
    pub fn fd(&self) -> std::os::fd::BorrowedFd<'_> {
        use std::os::fd::AsFd as _;
        self.file.as_fd()
    }

    /// Every key event waiting, as `(code, value)`. `Err` is a device that
    /// went away — unplugged.
    ///
    /// # Errors
    ///
    /// The device is gone.
    pub fn keys(&mut self) -> Result<Vec<(u16, i32)>> {
        let mut buf = [0u8; EVENT * 64];
        loop {
            match self.file.read(&mut buf) {
                Ok(0) => anyhow::bail!("the device closed"),
                Ok(n) => self.left.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e).context("reading a keyboard"),
            }
        }
        let whole = self.left.len() / EVENT * EVENT;
        let out = self.left[..whole]
            .chunks_exact(EVENT)
            .filter_map(|e| {
                let kind = u16::from_ne_bytes([e[16], e[17]]);
                let code = u16::from_ne_bytes([e[18], e[19]]);
                let value = i32::from_ne_bytes([e[20], e[21], e[22], e[23]]);
                (kind == EV_KEY).then_some((code, value))
            })
            .collect();
        self.left.drain(..whole);
        Ok(out)
    }
}

/// A keyboard we made — a unit's virtual twin.
#[derive(Debug)]
pub struct Twin {
    file: File,
}

impl Twin {
    /// Makes a virtual keyboard called `name`, able to press every key.
    ///
    /// # Errors
    ///
    /// `/dev/uinput` is not there or not ours — `hardware.uinput.enable`.
    pub fn make(name: &str) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .context("opening /dev/uinput")?;
        let mut setup = UinputSetup {
            bustype: BUS_VIRTUAL,
            // ZERO VENDOR AND PRODUCT: what keeps a twin out of the list of
            // keyboards (`ronoctua_keyboards::seen` skips vendor zero), so the
            // daemon never grabs its own output.
            vendor: 0,
            product: 0,
            version: 1,
            name: [0; 80],
            ff_effects_max: 0,
        };
        let bytes = name.as_bytes();
        let n = bytes.len().min(79);
        setup.name[..n].copy_from_slice(&bytes[..n]);
        // SAFETY: the four opcodes are uinput's, with the argument types its
        // header declares: an integer for the two SET_* bits (passed as the
        // value itself), a pointer to `struct uinput_setup` — whose layout is
        // `UinputSetup`, `repr(C)` — for SETUP, and nothing for CREATE. The
        // kernel reads `setup` and writes nothing of ours.
        unsafe {
            ioctl::ioctl(
                &file,
                IntegerSetter::<UI_SET_EVBIT>::new_usize(usize::from(EV_KEY)),
            )?;
            for code in (1..=crate::codes::MAX).filter(|c| !crate::codes::is_a_button(*c)) {
                ioctl::ioctl(
                    &file,
                    IntegerSetter::<UI_SET_KEYBIT>::new_usize(usize::from(code)),
                )?;
            }
            ioctl::ioctl(&file, Setter::<UI_DEV_SETUP, UinputSetup>::new(setup))?;
            ioctl::ioctl(&file, NoArg::<UI_DEV_CREATE>::new())?;
        }
        Ok(Self { file })
    }

    /// Emits one key event, and the report that closes it.
    ///
    /// # Errors
    ///
    /// The device could not be written.
    pub fn key(&mut self, code: u16, value: i32) -> Result<()> {
        let mut out = [0u8; EVENT * 2];
        write_event(&mut out[..EVENT], EV_KEY, code, value);
        write_event(&mut out[EVENT..], EV_SYN, 0, 0);
        self.file.write_all(&out).context("writing a key")
    }
}

/// One `struct input_event`, the time left zero — the kernel stamps it.
fn write_event(into: &mut [u8], kind: u16, code: u16, value: i32) {
    into[..16].fill(0);
    into[16..18].copy_from_slice(&kind.to_ne_bytes());
    into[18..20].copy_from_slice(&code.to_ne_bytes());
    into[20..24].copy_from_slice(&value.to_ne_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_is_the_kernels_layout() {
        let mut e = [0xffu8; EVENT];
        write_event(&mut e, EV_KEY, 30, 1);
        assert_eq!(&e[..16], &[0; 16]);
        assert_eq!(u16::from_ne_bytes([e[16], e[17]]), EV_KEY);
        assert_eq!(u16::from_ne_bytes([e[18], e[19]]), 30);
        assert_eq!(i32::from_ne_bytes([e[20], e[21], e[22], e[23]]), 1);
        assert_eq!(std::mem::size_of::<UinputSetup>(), 92, "uinput.h's size");
    }
}
