//! The level of a microphone, while somebody is looking at it.
//!
//! # Why a child spitting raw samples
//!
//! It is the film player's shape (`ronoctua-shell/src/motion.rs`), and for the
//! same reason: a live child writing raw frames down a pipe is a clock the
//! reading thread does not have to keep. Measured here — `pw-cat` at 8 kHz,
//! one channel, signed 16-bit delivers **16 KB/s** and the first sample lands
//! **189 ms** after the ask.
//!
//! # Why it is a VALUE and not a switch
//!
//! Because dropping it is what closes the microphone. A flag would leave the
//! decision to whoever remembered to look at it; a value ties "somebody is
//! looking at this row" to "this device is open" with the compiler holding the
//! knot, and the guard on [`Meter`]'s `Drop` is what a closed view, a moved
//! cursor and a killed footer all reach.
//!
//! # What it costs, and who agreed to it
//!
//! It opens a microphone. That is why the view sends [`crate::Order::Listen`]
//! only for the row under the cursor, why leaving the row sends `None`, and
//! why the row itself says on screen that it is listening. Nothing here starts
//! on its own.

use anyhow::Result;

/// How many bytes of audio one reading covers.
///
/// 1 600 bytes is 800 samples at 8 kHz: a tenth of a second, which is about
/// as often as an eye can read a bar. Smaller windows would report noise;
/// larger ones would lag behind the voice they are meant to follow.
const WINDOW: usize = 1_600;

/// The largest a signed 16-bit sample gets.
const LOUDEST: f64 = 32_768.0;

/// A child that is killed when this is dropped.
///
/// The same guard the film player grew after four hand-written `kill(); wait()`
/// sites disagreed about which paths reached them.
#[derive(Debug)]
struct Reaped(std::process::Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// One open microphone, reporting its level.
#[derive(Debug)]
pub struct Meter {
    /// Which source is open. Read back so the view can prove the meter
    /// follows the cursor rather than trusting that it does.
    target: u32,
    /// Dropping this closes the device.
    _child: Reaped,
}

impl Meter {
    /// Open one source and report its level until this is dropped.
    ///
    /// # Errors
    ///
    /// When `pw-cat` is missing or will not start. A machine with no
    /// microphone is not an error here: the view never offers a row for a
    /// source that does not exist.
    pub fn listen_to<F>(target: u32, on_level: F) -> Result<Self>
    where
        F: Fn(u8) + Send + 'static,
    {
        let mut child = crate::pipewire::record(target)?;
        let Some(out) = child.stdout.take() else {
            let _ = child.kill();
            anyhow::bail!("the meter came up with no output to read");
        };
        std::thread::Builder::new()
            .name("audio-meter".to_owned())
            .spawn(move || {
                let mut out = out;
                let mut block = vec![0_u8; WINDOW];
                let mut peak = Peak::fresh();
                loop {
                    // READ WHAT ARRIVED, not a whole window. A short read at
                    // the end of a device's buffer is normal, and waiting for
                    // the rest with `read_exact` is what once left a thread
                    // wedged for the life of a session. The window is closed
                    // by `Peak`, which counts across reads instead.
                    let got = match std::io::Read::read(&mut out, &mut block) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) => {
                            tracing::debug!(error = %e, "the meter stopped reading");
                            break;
                        }
                    };
                    if let Some(level) = peak.took(&block[..got]) {
                        on_level(level);
                    }
                }
            })
            .map_err(|e| anyhow::anyhow!("could not start the meter thread: {e}"))?;
        Ok(Self {
            target,
            _child: Reaped(child),
        })
    }

    /// Which source this has open.
    #[must_use]
    pub const fn watching(&self) -> u32 {
        self.target
    }
}

/// The loudest sample of a window, built up across however many reads it takes.
///
/// # Why the window is closed HERE and not by the read
///
/// Because a read gives back whatever arrived. Measured against the real
/// `pw-cat`: reporting one level per read fired **forty times a second**, and
/// every one of them is a repaint of the footer. A tenth of a second is as
/// often as an eye can read a bar, so the bytes are counted across reads and
/// the level comes out when a window's worth has been seen.
///
/// # Why it carries a spare byte
///
/// A read can end in the middle of a sample. Dropping that byte would put
/// every later sample in the block one byte out — the high half read as the
/// low one — and the meter would report a whisper as a shout, or the reverse,
/// for the rest of that read.
#[derive(Debug, Default)]
pub struct Peak {
    /// The loudest sample seen since the last reading came out.
    loudest: i32,
    /// How many bytes have gone into it.
    seen: usize,
    /// The odd byte a read ended on, waiting for its other half.
    spare: Option<u8>,
}

impl Peak {
    /// One that has heard nothing.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Take some bytes, and answer with a level when a window is full.
    pub fn took(&mut self, block: &[u8]) -> Option<u8> {
        let mut rest = block;
        if let Some(half) = self.spare.take()
            && let Some((&other, tail)) = rest.split_first()
        {
            self.heard(i16::from_le_bytes([half, other]));
            self.seen += 2;
            rest = tail;
        }
        for pair in rest.chunks_exact(2) {
            // LITTLE-ENDIAN, which is what `pw-cat --format s16` writes and
            // what every machine this runs on is.
            self.heard(i16::from_le_bytes([pair[0], pair[1]]));
        }
        if rest.len() % 2 == 1 {
            self.spare = rest.last().copied();
        }
        self.seen += rest.len() / 2 * 2;
        if self.seen < WINDOW {
            return None;
        }
        let level = crate::perceived(f64::from(self.loudest) / LOUDEST).min(100);
        self.loudest = 0;
        self.seen = 0;
        Some(level)
    }

    /// Keep the louder of what is held and one sample.
    ///
    /// The PEAK and not the average: a meter is there to show that sound is
    /// arriving at all, and the mean of a tenth of a second of speech sits
    /// near zero between syllables — a bar that fell to nothing between words
    /// would read as a microphone cutting out.
    ///
    /// The peak goes through the same rule as the volume
    /// ([`crate::perceived`]), so the bar in this row and the bar in the
    /// volume row mean the same thing about the same amount of sound. A
    /// linear meter would sit in the bottom tenth for every voice that is not
    /// shouting.
    fn heard(&mut self, sample: i16) {
        self.loudest = self.loudest.max(i32::from(sample).abs());
    }
}

/// The level of one block of signed 16-bit samples, 0..=100, on its own.
///
/// [`Peak`] is what the thread uses, because it has a window to fill. This is
/// the same arithmetic for a block that is already whole, and it is what the
/// tests below reason about.
#[must_use]
pub fn level_of(block: &[u8]) -> u8 {
    let mut loudest = 0_i32;
    for pair in block.chunks_exact(2) {
        loudest = loudest.max(i32::from(i16::from_le_bytes([pair[0], pair[1]])).abs());
    }
    crate::perceived(f64::from(loudest) / LOUDEST).min(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_of(samples: &[i16]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    #[test]
    fn silence_is_nothing() {
        assert_eq!(level_of(&block_of(&[0; 64])), 0);
        assert_eq!(level_of(&[]), 0);
    }

    #[test]
    fn the_loudest_sample_is_the_reading() {
        // One loud sample in a block of silence is a click, and a peak meter
        // is supposed to show it.
        let mut samples = vec![0_i16; 64];
        samples[13] = i16::MAX;
        assert_eq!(level_of(&block_of(&samples)), 100);
    }

    #[test]
    fn a_negative_swing_counts_as_much_as_a_positive_one() {
        // Sound is symmetrical about zero, and a meter that only looked at
        // the positive half would read half the level for every waveform
        // whose peak happens to fall below the line.
        assert_eq!(
            level_of(&block_of(&[-20_000, 0, 0])),
            level_of(&block_of(&[20_000, 0, 0]))
        );
    }

    #[test]
    fn a_quiet_voice_is_not_pinned_to_the_bottom() {
        // Roughly where speech at a normal distance sits. A linear meter
        // would answer 6 here, which draws as an empty bar.
        let speech = level_of(&block_of(&[2_000, -1_800, 900]));
        assert!(
            (30..=50).contains(&speech),
            "speech read {speech}, which is not a readable bar"
        );
    }

    #[test]
    fn a_window_is_filled_before_anything_comes_out() {
        // THE MEASUREMENT THIS PINS: reporting one level per read fired forty
        // times a second against the real `pw-cat`, and every one of those is
        // a repaint of the whole footer.
        let mut peak = Peak::fresh();
        let quarter = block_of(&vec![1_000_i16; WINDOW / 8]);
        assert_eq!(
            peak.took(&quarter),
            None,
            "a quarter window is not a reading"
        );
        assert_eq!(peak.took(&quarter), None);
        assert_eq!(peak.took(&quarter), None);
        assert!(
            peak.took(&quarter).is_some(),
            "the fourth quarter closes it"
        );
        // And it starts again from nothing.
        assert_eq!(peak.took(&quarter), None);
    }

    #[test]
    fn the_loudest_of_a_window_survives_the_reads_it_arrived_in() {
        let mut peak = Peak::fresh();
        let mut half = vec![0_i16; WINDOW / 4];
        half[7] = i16::MAX;
        assert_eq!(peak.took(&block_of(&half)), None);
        let level = peak
            .took(&block_of(&vec![0_i16; WINDOW / 4]))
            .expect("the window closed");
        assert_eq!(level, 100, "the click in the first read is still the peak");
    }

    #[test]
    fn a_sample_split_across_two_reads_is_not_misread() {
        // The failure this pins: dropping the odd byte puts every later
        // sample one byte out — the high half read as the low one — and the
        // meter reports a whisper as a shout for the rest of the read.
        let whole = block_of(&vec![i16::MAX; WINDOW / 2]);
        let (head, tail) = whole.split_at(WINDOW / 2 + 1);

        let mut split = Peak::fresh();
        assert_eq!(split.took(head), None);
        let by_halves = split.took(tail).expect("the window closed");

        let mut once = Peak::fresh();
        let at_once = once.took(&whole).expect("the window closed");

        assert_eq!(by_halves, at_once);
        assert_eq!(at_once, 100);
    }

    #[test]
    fn an_odd_tail_does_not_panic() {
        // A short read can end mid-sample; `chunks_exact` leaves the stray
        // byte alone and the reading is of what did arrive.
        let mut bytes = block_of(&[i16::MAX]);
        bytes.push(0x7f);
        assert_eq!(level_of(&bytes), 100);
    }

    #[test]
    fn the_bytes_are_read_little_endian() {
        // 0x0100 read the wrong way round is 0x0001 — the difference between
        // a whisper and a shout, and it would be inaudible in the code.
        let quiet = level_of(&[0x01, 0x00]);
        let loud = level_of(&[0x00, 0x01]);
        assert!(quiet < loud, "quiet {quiet} should be under loud {loud}");
    }
}
