//! Sound: several inputs into one, and how loud each one is.
//!
//! Signed 16-bit, interleaved stereo, 48 kHz — what `pw-cat` records and what
//! the encoder reads. Pure over slices, so the rules are tested with no
//! PipeWire running.

/// One input's knobs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Knobs {
    /// Linear gain, `0.0` to `2.0` (up to +6 dB).
    pub gain: f32,
    /// Silent without being removed.
    pub muted: bool,
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            gain: 1.0,
            muted: false,
        }
    }
}

/// Mixes `inputs` into `out`, which is overwritten. A shorter input is
/// silence past its end; the sum is CLAMPED, never wrapped — a wrapped sample
/// is a click loud enough to hurt.
pub fn mix(out: &mut [i16], inputs: &[(&[i16], Knobs)]) {
    for (i, o) in out.iter_mut().enumerate() {
        let mut sum = 0.0f32;
        for (samples, knobs) in inputs {
            if knobs.muted {
                continue;
            }
            if let Some(s) = samples.get(i) {
                sum += f32::from(*s) * knobs.gain.clamp(0.0, 2.0);
            }
        }
        *o = sum.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
    }
}

/// How loud a block is, `0` to `100` — the peak on a decibel scale from
/// −60 dBFS (0) to full scale (100), which is how a meter is read: a
/// linear bar spends nine tenths of its length on sounds too loud to talk at.
pub fn level(samples: &[i16]) -> u8 {
    let peak = samples
        .iter()
        .map(|s| i32::from(*s).unsigned_abs())
        .max()
        .unwrap_or(0);
    if peak == 0 {
        return 0;
    }
    let db = 20.0 * (peak as f32 / 32768.0).log10();
    (((db + 60.0) / 60.0) * 100.0).round().clamp(0.0, 100.0) as u8
}

/// Bytes to samples, little-endian, a trailing odd byte ignored.
pub fn samples(bytes: &[u8], out: &mut Vec<i16>) {
    out.clear();
    out.extend(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b)),
    );
}

/// Samples to bytes, little-endian.
pub fn bytes(samples: &[i16], out: &mut Vec<u8>) {
    out.clear();
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_inputs_add_and_clamp() {
        let a = [10_000i16, 30_000, -30_000];
        let b = [5_000i16, 10_000, -10_000];
        let mut out = [0i16; 3];
        mix(&mut out, &[(&a, Knobs::default()), (&b, Knobs::default())]);
        assert_eq!(out, [15_000, i16::MAX, i16::MIN]);
    }

    #[test]
    fn a_muted_input_adds_nothing_and_gain_scales() {
        let a = [1_000i16; 2];
        let b = [7_000i16; 2];
        let mut out = [0i16; 2];
        mix(
            &mut out,
            &[
                (
                    &a,
                    Knobs {
                        gain: 2.0,
                        muted: false,
                    },
                ),
                (
                    &b,
                    Knobs {
                        gain: 1.0,
                        muted: true,
                    },
                ),
            ],
        );
        assert_eq!(out, [2_000; 2]);
    }

    #[test]
    fn a_short_input_is_silence_past_its_end() {
        let a = [100i16];
        let mut out = [9i16; 3];
        mix(&mut out, &[(&a, Knobs::default())]);
        assert_eq!(out, [100, 0, 0]);
    }

    #[test]
    fn the_meter_is_in_decibels() {
        assert_eq!(level(&[0, 0]), 0);
        assert_eq!(level(&[i16::MAX]), 100);
        // −6 dBFS is nine tenths of the bar, not half of it.
        assert_eq!(level(&[16_384]), 90);
    }

    #[test]
    fn samples_round_trip_through_bytes() {
        let mut b = Vec::new();
        bytes(&[1, -2, 300], &mut b);
        let mut s = Vec::new();
        samples(&b, &mut s);
        assert_eq!(s, vec![1, -2, 300]);
    }
}
