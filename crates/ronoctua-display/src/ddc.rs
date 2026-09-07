//! DDC/CI — asking an EXTERNAL monitor to change its own brightness.
//!
//! A laptop panel has a backlight in `/sys` ([`crate::backlight`]). A monitor
//! on a cable has nothing there: the only way to move its brightness from the
//! machine is to talk to the monitor itself, over the two I2C wires the video
//! cable has always carried for reading the EDID.
//!
//! # The one `unsafe` in this crate, and why it cannot be avoided
//!
//! Addressing a device on an I2C bus is an `ioctl` (`I2C_SLAVE`). `read` and
//! `write` on `/dev/i2c-N` go to whatever address was set that way, and there
//! is no other interface — the alternative is `I2C_RDWR`, which is more
//! `unsafe`, not less. So the module allows `unsafe_code` and the workspace
//! denies it everywhere else, which is the discipline made mechanical rather
//! than remembered.
//!
//! # Three traps, all of them silent
//!
//! 1. **The checksum covers an address that is not in the buffer.** For a
//!    request it starts at `0x6E` (the display), for a reply at `0x50` (the
//!    host) — neither byte is written or read, because the I2C layer carries
//!    the address itself. Leaving it out gives a checksum that is wrong by a
//!    constant, and a monitor that ignores every packet without saying why.
//! 2. **A monitor needs time between transactions.** The specification asks
//!    for 40 ms; below it a monitor answers the PREVIOUS question, so the
//!    brightness read back is the one from before the write, and the screen
//!    shows a number that lags one gesture behind.
//! 3. **A monitor that does not speak DDC/CI is not a defect.** It refuses
//!    the address with `ENXIO`, or answers zeros. That is an ordinary monitor
//!    and an ordinary answer: it means "this one has buttons on the front",
//!    and the row says so.
//!
//! What is tested here without hardware is everything that is arithmetic: the
//! checksum against the packets the specification writes out, the request
//! bytes, and the parsing of a reply. Talking to a real monitor is proved on
//! the owner's machine with one plugged in, and nowhere else — this repository
//! has no monitor in a VM.

// THE ONE PLACE. See the module's header: `I2C_SLAVE` has no safe wrapper,
// and the workspace's `unsafe_code = "warn"` plus the Nix battery's
// `-D warnings` mean this line is what allows it — in one module, visible in
// a diff, and nowhere else in the crate.
#![allow(unsafe_code)]

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

/// The address every DDC/CI display answers on.
pub const ADDRESS: u16 = 0x37;

/// The byte a REQUEST's checksum starts from: the display's address, doubled.
///
/// `0x37 << 1`. It is not in the buffer — the I2C layer sends the address —
/// and leaving it out of the sum is trap 1.
const TO_DISPLAY: u8 = 0x6E;

/// The byte a REPLY's checksum starts from: the host's own address.
const TO_HOST: u8 = 0x50;

/// The host's source address, which IS the first byte of a request.
const FROM_HOST: u8 = 0x51;

/// VCP code 0x10 — luminance. The one control this reads and writes.
pub const LUMINANCE: u8 = 0x10;

/// How long a monitor is given between two transactions.
///
/// The specification says 40 ms. Below it a monitor answers the previous
/// question — trap 2 — and the number on screen lags a gesture behind.
pub const BREATH: Duration = Duration::from_millis(50);

/// How many times a question is asked before the monitor is called silent.
///
/// DDC/CI has no retransmission of its own and a byte lost on a long cable is
/// ordinary. Three is what a hand notices as an answer that took a moment,
/// and it is what the tools that came before this one settle on.
pub const TRIES: u8 = 3;

/// What a monitor answered about one of its own controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// Where it is now.
    pub now: u16,
    /// The largest it goes. Monitors report 100 far more often than not, but
    /// they are entitled to any ceiling and several use 255.
    pub max: u16,
}

impl Reading {
    /// The brightness as a person reads it.
    #[must_use]
    pub fn percent(self) -> u8 {
        if self.max == 0 {
            return 0;
        }
        let scaled = (u32::from(self.now) * 100 + u32::from(self.max) / 2) / u32::from(self.max);
        u8::try_from(scaled.min(100)).unwrap_or(100)
    }
}

/// The checksum of a DDC/CI packet.
///
/// `first` is the address byte the sum starts from and which is NOT in
/// `bytes`: [`TO_DISPLAY`] for something being sent, [`TO_HOST`] for
/// something received.
#[must_use]
pub fn checksum(first: u8, bytes: &[u8]) -> u8 {
    bytes.iter().fold(first, |sum, b| sum ^ b)
}

/// The bytes that ask for one control's value.
#[must_use]
pub fn ask_for(vcp: u8) -> [u8; 5] {
    // 0x82: the high bit marks a length, and the length is 2 (the opcode and
    // the VCP code). 0x01 is "Get VCP Feature".
    let body = [FROM_HOST, 0x82, 0x01, vcp];
    [
        body[0],
        body[1],
        body[2],
        body[3],
        checksum(TO_DISPLAY, &body),
    ]
}

/// The bytes that set one control's value.
#[must_use]
pub fn set_to(vcp: u8, value: u16) -> [u8; 7] {
    // 0x84: a length of 4. 0x03 is "Set VCP Feature". The value is
    // big-endian, which is the one place this protocol disagrees with every
    // other byte order on the machine.
    let body = [
        FROM_HOST,
        0x84,
        0x03,
        vcp,
        u8::try_from(value >> 8).unwrap_or(0),
        u8::try_from(value & 0xFF).unwrap_or(0),
    ];
    [
        body[0],
        body[1],
        body[2],
        body[3],
        body[4],
        body[5],
        checksum(TO_DISPLAY, &body),
    ]
}

/// Reads a reply to a "get" and says what the monitor answered.
///
/// # Errors
///
/// The reply is not shaped like one, its checksum does not hold, it is about
/// another control, or the monitor said the control is not supported.
pub fn understand(vcp: u8, reply: &[u8; 11]) -> Result<Reading> {
    // A MONITOR THAT IS NOT THERE ANSWERS ZEROS, and so does a bus with
    // nothing on it. Saying "checksum" about that would send somebody looking
    // for a cable fault.
    if reply.iter().all(|b| *b == 0) {
        bail!("the monitor did not answer");
    }
    let (body, sum) = reply.split_at(reply.len() - 1);
    let expected = checksum(TO_HOST, body);
    if sum[0] != expected {
        bail!(
            "the reply's checksum is {:#04x} and should be {expected:#04x}",
            sum[0]
        );
    }
    if reply[0] != TO_DISPLAY || reply[1] != 0x88 || reply[2] != 0x02 {
        bail!("the reply is not a VCP answer: {reply:02x?}");
    }
    // Byte 3 is the result code: 0 is fine, 1 is "I do not have that one".
    if reply[3] != 0x00 {
        bail!("the monitor does not offer the control {vcp:#04x}");
    }
    if reply[4] != vcp {
        bail!(
            "the monitor answered about {:#04x} and was asked about {vcp:#04x}",
            reply[4]
        );
    }
    // AND A MAXIMUM OF ZERO IS NOT A RANGE.
    //
    // Everything above this line rejects a reply that is malformed; this one
    // rejects a reply that is well formed and says something impossible. It
    // matters because the number is arithmetic downstream: `backlight::raw_of`
    // ends in `clamp(floor_raw(max), max)`, and `floor_raw` is `.max(1)`, so a
    // zero maximum is `clamp(1, 0)` — which panics, on the thread, taking the
    // whole display reader down for the session with the only symptom being
    // that every screen goes stale.
    //
    // A monitor gets here by answering a correctly-checksummed frame with a
    // zero maximum, which a firmware may simply do for a control it half
    // supports; a 1-in-256 checksum collision on a corrupted reply reaches it
    // too. The panel side has refused this since it was written
    // (`backlight::read_in`), and the reason is the same on a cable.
    let max = u16::from(reply[6]) << 8 | u16::from(reply[7]);
    if max == 0 {
        bail!("the monitor says the control {vcp:#04x} has a maximum of zero");
    }
    Ok(Reading {
        max,
        now: u16::from(reply[8]) << 8 | u16::from(reply[9]),
    })
}

/// A bus with a display addressed on it.
pub struct Bus {
    file: std::fs::File,
}

impl Bus {
    /// Opens `/dev/i2c-N` and points it at the display.
    ///
    /// # Errors
    ///
    /// The node is not there, this user is not in the group that may open it,
    /// or the kernel refused the address.
    pub fn open(node: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(node)
            .with_context(|| {
                format!(
                    "could not open {}. Reaching a monitor's own controls needs \
                     `i2c-dev` loaded and this user in the group that owns the node.",
                    node.display()
                )
            })?;

        // `I2C_SLAVE` is `_IO(0x07, 0x03)` — a legacy opcode that takes the
        // address as the integer argument rather than a pointer.
        const I2C_SLAVE: rustix::ioctl::Opcode = rustix::ioctl::opcode::none(0x07, 0x03);

        // SAFETY: `I2C_SLAVE` is a valid opcode of the i2c-dev driver and it
        // expects an integer, not a pointer; `ADDRESS` is 0x37, inside the
        // 7-bit range the driver accepts. The call reads and writes no memory
        // of ours — the argument IS the value.
        //
        // The file is whatever `node` pointed at, and nothing here proves it
        // is an i2c device: it comes from a connector's `ddc/i2c-dev` entry,
        // which is as good as sysfs gets, but the check is the KERNEL's. An
        // ioctl with this opcode on anything else answers `ENOTTY`, which
        // arrives as the error below.
        unsafe {
            let ask = rustix::ioctl::IntegerSetter::<I2C_SLAVE>::new_usize(ADDRESS as usize);
            rustix::ioctl::ioctl(&file, ask)
        }
        .with_context(|| format!("{} refused the display's address", node.display()))?;

        Ok(Self { file })
    }

    /// Asks the monitor for one of its controls.
    ///
    /// # Errors
    ///
    /// The monitor stayed silent through every try, or answered something
    /// that is not an answer to this question.
    pub fn get(&mut self, vcp: u8) -> Result<Reading> {
        let mut last = None;
        for _ in 0..TRIES {
            std::thread::sleep(BREATH);
            match self.once(vcp) {
                Ok(r) => return Ok(r),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("the monitor said nothing")))
    }

    /// Sets one of the monitor's controls.
    ///
    /// It is not confirmed by a read: a monitor takes up to a second to move
    /// its own backlight, and reading straight back gives the value from
    /// before. What confirms it is the NEXT reading, which the snapshot makes
    /// on its own.
    ///
    /// # Errors
    ///
    /// The bus refused the write.
    pub fn set(&mut self, vcp: u8, value: u16) -> Result<()> {
        std::thread::sleep(BREATH);
        self.file
            .write_all(&set_to(vcp, value))
            .context("the monitor refused the change")
    }

    fn once(&mut self, vcp: u8) -> Result<Reading> {
        self.file
            .write_all(&ask_for(vcp))
            .context("the monitor refused the question")?;
        // The specification's own wait between asking and reading. Without
        // it the monitor is still assembling the reply and hands back the
        // previous one.
        std::thread::sleep(BREATH);
        let mut reply = [0u8; 11];
        self.file
            .read_exact(&mut reply)
            .context("the monitor stopped mid-answer")?;
        understand(vcp, &reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_the_packet_the_specification_writes_out() {
        // Worked by hand from the specification, and it is the packet every
        // other implementation of this protocol sends:
        //   0x6E ^ 0x51 ^ 0x82 ^ 0x01 ^ 0x10 = 0xAC
        // The 0x6E is the display's address, which is NOT in the buffer —
        // trap 1 of the module's header. Dropping it gives 0xC2, and a
        // monitor that ignores everything without a word.
        assert_eq!(ask_for(LUMINANCE), [0x51, 0x82, 0x01, 0x10, 0xAC]);
        assert_ne!(checksum(0x00, &[0x51, 0x82, 0x01, 0x10]), 0xAC);
    }

    #[test]
    fn a_value_is_set_big_endian() {
        // The one place this protocol disagrees with every other byte order
        // on the machine. Swapped, setting 50 asks for 12800 and the monitor
        // clamps to its maximum — a key that always jumps to full brightness.
        assert_eq!(
            set_to(LUMINANCE, 50),
            [0x51, 0x84, 0x03, 0x10, 0x00, 0x32, 0x9A]
        );
        // 0x6E^0x51^0x84^0x03^0x10^0x01^0x02 = 0xAB. The first version of
        // this line said 0xA9, and the test is what caught the slip — which
        // is the point of writing the expected packet out by hand rather
        // than calling the function that produced it.
        assert_eq!(
            set_to(LUMINANCE, 0x0102),
            [0x51, 0x84, 0x03, 0x10, 0x01, 0x02, 0xAB]
        );
        // And the checksum really does cover the value: two values that
        // differ must not share a packet.
        assert_ne!(set_to(LUMINANCE, 50)[6], set_to(LUMINANCE, 51)[6]);
    }

    #[test]
    fn a_maximum_of_zero_is_refused() {
        // A WELL-FORMED REPLY THAT SAYS SOMETHING IMPOSSIBLE. The checksum
        // holds, the header is right, the control is the one asked about — and
        // the maximum is zero, which is not a range. Taken at face value it
        // reaches `backlight::raw_of`, whose `clamp(floor_raw(max), max)` is
        // `clamp(1, 0)` and PANICS on the reader thread, leaving every screen
        // stale with nothing on screen saying why.
        let bad = reply_of(LUMINANCE, 0x00, 0, 0x0032);
        let said = understand(LUMINANCE, &bad).unwrap_err().to_string();
        assert!(said.contains("maximum of zero"), "{said}");
        // And the ordinary reply still reads, or the guard would cost the
        // feature instead of the crash.
        let good = understand(LUMINANCE, &reply_of(LUMINANCE, 0x00, 100, 40)).unwrap();
        assert_eq!((good.max, good.now), (100, 40));
    }

    fn reply_of(vcp: u8, result: u8, max: u16, now: u16) -> [u8; 11] {
        let mut r = [
            TO_DISPLAY,
            0x88,
            0x02,
            result,
            vcp,
            0x00,
            u8::try_from(max >> 8).unwrap(),
            u8::try_from(max & 0xFF).unwrap(),
            u8::try_from(now >> 8).unwrap(),
            u8::try_from(now & 0xFF).unwrap(),
            0x00,
        ];
        r[10] = checksum(TO_HOST, &r[..10]);
        r
    }

    #[test]
    fn a_good_reply_is_understood() {
        let r = understand(LUMINANCE, &reply_of(LUMINANCE, 0, 100, 50)).unwrap();
        assert_eq!(r, Reading { now: 50, max: 100 });
        assert_eq!(r.percent(), 50);
        // A monitor is entitled to any ceiling, and several use 255.
        let coarse = understand(LUMINANCE, &reply_of(LUMINANCE, 0, 255, 128)).unwrap();
        assert_eq!(coarse.percent(), 50);
    }

    #[test]
    fn a_replys_checksum_starts_from_the_hosts_address() {
        // A REPLY WRITTEN OUT BY HAND, and it has to be: `reply_of` builds
        // its checksum with the same function `understand` verifies it with,
        // so the SEED cancels out and every test that uses it would pass with
        // `TO_HOST` set to anything at all — while every real monitor was
        // rejected as corrupt, which reads on screen as a broken cable.
        //
        // Worked from the specification for luminance, result 0, max 100,
        // now 50: 0x50^0x6E^0x88^0x02^0x00^0x10^0x00^0x00^0x64^0x00^0x32.
        let by_hand: [u8; 11] = [
            0x6E, 0x88, 0x02, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00, 0x32, 0xF2,
        ];
        assert_eq!(
            understand(LUMINANCE, &by_hand).unwrap(),
            Reading { now: 50, max: 100 }
        );
        assert_eq!(by_hand, reply_of(LUMINANCE, 0, 100, 50));

        // And the request's seed is the OTHER one. Swapping them makes every
        // good reply look corrupt.
        let mut wrong = by_hand;
        wrong[10] = checksum(TO_DISPLAY, &by_hand[..10]);
        assert!(understand(LUMINANCE, &wrong).is_err());
    }

    #[test]
    fn silence_says_silence_and_not_checksum() {
        // A bus with nothing on it reads zeros. Calling that a checksum
        // failure sends somebody looking for a cable fault that is not there.
        let quiet = [0u8; 11];
        let e = understand(LUMINANCE, &quiet).unwrap_err().to_string();
        assert!(e.contains("did not answer"), "{e}");
    }

    #[test]
    fn an_answer_about_another_control_is_refused() {
        // It is what arrives when the monitor is a transaction behind — trap
        // 2. Taking it would show the contrast where the brightness goes.
        let e = understand(LUMINANCE, &reply_of(0x12, 0, 100, 50))
            .unwrap_err()
            .to_string();
        assert!(e.contains("0x12"), "{e}");
    }

    #[test]
    fn a_control_the_monitor_does_not_have_is_refused() {
        let e = understand(LUMINANCE, &reply_of(LUMINANCE, 1, 0, 0))
            .unwrap_err()
            .to_string();
        assert!(e.contains("does not offer"), "{e}");
    }

    #[test]
    fn a_reading_with_no_range_does_not_divide_by_zero() {
        assert_eq!(Reading { now: 0, max: 0 }.percent(), 0);
    }
}
