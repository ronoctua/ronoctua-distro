//! Keeping a voice in step with the master clock.
//!
//! Pure: given how far a voice is from where the clock says it should be,
//! what to do about it. The daemon asks every [`TICK`] and applies the answer
//! to that voice's mpv — a speed, or a seek.
//!
//! # A picture and a sound are nudged differently
//!
//! mpv keeps a sound's pitch when its speed changes, by stretching it — and a
//! stretch of five percent is heard, a wobble in a voice or a held note. A
//! picture five percent fast is not seen. So a SOUND is nudged by
//! [`NUDGE_SOUND`] and jumps sooner ([`JUMP_MS_SOUND`]): a gap of a few
//! milliseconds once is less than twenty seconds of stretched speech. The
//! numbers are what the tests below measure: how long each takes to catch up.

/// How often a voice is measured.
pub const TICK: std::time::Duration = std::time::Duration::from_millis(250);
/// Below this, nothing is done: a correction audible as a pitch wobble costs
/// more than a drift nobody can see.
pub const STILL_MS: f64 = 40.0;
/// Above this a gentle catch-up would take too long: jump.
pub const JUMP_MS: f64 = 1000.0;
/// How much faster or slower a picture may run while catching up.
pub const NUDGE: f64 = 0.05;
/// How much faster or slower a sound may run while catching up — less than
/// a picture, because a sound's stretch is heard.
pub const NUDGE_SOUND: f64 = 0.02;
/// Above this a sound jumps: at [`NUDGE_SOUND`] it would take twenty seconds
/// of stretched sound to catch up, and a jump is one short gap.
pub const JUMP_MS_SOUND: f64 = 400.0;

/// How long after a load or a seek a voice is SEEKED instead of nudged.
///
/// A load — over the tailnet, or of a second file beside the first — takes a
/// moment the start position cannot know (measured: 0.7 s on a local file),
/// and a nudge pays that back at 50 ms a second: fourteen seconds out of
/// step. Right after one, anything past [`SETTLE_MS`] is a seek
/// ([`settle`]); after that, the nudges of [`correct_as`].
pub const SETTLING: std::time::Duration = std::time::Duration::from_secs(4);

/// What counts as out of step while settling.
pub const SETTLE_MS: f64 = 120.0;

/// How far AHEAD of the master a settling voice is sent: the seek itself
/// takes a moment, and a voice sent exactly where the master was lands
/// behind it.
pub const SEEK_LEAD_SECONDS: f64 = 0.06;

/// What is being kept in step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Follower {
    /// A picture, with or without a sound of its own that keeps no time.
    Picture,
    /// A sound: heard, so nudged gently.
    Sound,
}

/// What to do to a voice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Correction {
    /// It is in step: normal speed.
    Keep,
    /// Run at this factor of the master's speed until caught up.
    Speed(f64),
    /// Jump to this position, seconds.
    Seek(f64),
}

/// `master` and `follower` are positions in seconds at the same instant,
/// `speed` the master's own. Behind means the follower is earlier. A
/// picture's rule ([`correct_as`]).
#[must_use]
pub fn correct(master: f64, follower: f64, speed: f64) -> Correction {
    correct_as(master, follower, speed, Follower::Picture)
}

/// [`correct`], for a picture or a sound.
#[must_use]
pub fn correct_as(master: f64, follower: f64, speed: f64, who: Follower) -> Correction {
    let (nudge, jump_ms) = match who {
        Follower::Picture => (NUDGE, JUMP_MS),
        Follower::Sound => (NUDGE_SOUND, JUMP_MS_SOUND),
    };
    let drift_ms = (follower - master) * 1000.0;
    if drift_ms.abs() < STILL_MS {
        Correction::Keep
    } else if drift_ms.abs() >= jump_ms {
        Correction::Seek(master)
    } else if drift_ms < 0.0 {
        Correction::Speed(speed * (1.0 + nudge))
    } else {
        Correction::Speed(speed * (1.0 - nudge))
    }
}

/// While a voice settles ([`SETTLING`]): where to send it when it is out by
/// more than [`SETTLE_MS`] — a little ahead of the master, by what a seek
/// takes — and `None` when it is close enough to be left alone.
#[must_use]
pub fn settle(master: f64, follower: f64) -> Option<f64> {
    ((follower - master).abs() * 1000.0 > SETTLE_MS).then_some(master + SEEK_LEAD_SECONDS)
}

/// A clock offset from NTP-style exchanges: each is `(sent, remote, received)`
/// in seconds, the remote time taken between our two. The answer is how much
/// to ADD to a remote time to make it ours — the median of the samples, so a
/// single delayed packet cannot move it.
#[must_use]
pub fn offset(samples: &[(f64, f64, f64)]) -> Option<f64> {
    let mut each: Vec<f64> = samples
        .iter()
        .map(|(sent, remote, received)| (sent + received) / 2.0 - remote)
        .collect();
    if each.is_empty() {
        return None;
    }
    each.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(each[each.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_step_is_left_alone_and_far_off_jumps() {
        assert_eq!(correct(10.0, 10.02, 1.0), Correction::Keep);
        assert_eq!(correct(10.0, 12.0, 1.0), Correction::Seek(10.0));
    }

    #[test]
    fn behind_runs_faster_and_ahead_runs_slower() {
        let Correction::Speed(s) = correct(10.0, 9.8, 1.0) else {
            panic!("a nudge")
        };
        assert!(s > 1.0);
        let Correction::Speed(s) = correct(10.0, 10.3, 2.0) else {
            panic!("a nudge")
        };
        assert!(s < 2.0 && s > 1.8);
    }

    #[test]
    fn a_follower_nudged_every_tick_converges() {
        // A follower that starts 400 ms behind, corrected every tick, is in
        // step within a few seconds and never overshoots into a jump.
        let (mut master, mut follower) = (0.0f64, -0.4f64);
        let dt = TICK.as_secs_f64();
        let mut ticks = 0;
        loop {
            let speed = match correct(master, follower, 1.0) {
                Correction::Keep => break,
                Correction::Speed(s) => s,
                Correction::Seek(_) => panic!("a 400 ms drift should not jump"),
            };
            master += dt;
            follower += dt * speed;
            ticks += 1;
            assert!(ticks < 200, "never caught up");
        }
        assert!(f64::from(ticks) * dt < 10.0);
    }

    /// How many ticks `who`, starting `drift_s` behind, takes to be left
    /// alone — `None` when it jumps instead.
    fn ticks_to_catch_up(drift_s: f64, who: Follower) -> Option<u32> {
        let (mut master, mut follower) = (0.0f64, -drift_s);
        let dt = TICK.as_secs_f64();
        let mut ticks = 0;
        loop {
            let speed = match correct_as(master, follower, 1.0, who) {
                Correction::Keep => return Some(ticks),
                Correction::Speed(s) => s,
                Correction::Seek(_) => return None,
            };
            master += dt;
            follower += dt * speed;
            ticks += 1;
            assert!(ticks < 1000, "never caught up");
        }
    }

    /// FAILS IF: a sound is nudged as hard as a picture — mpv keeps its pitch
    /// by stretching it, and five percent of stretch is heard — or so gently,
    /// or so long, that it would be out of step for longer than a jump costs.
    #[test]
    fn a_sound_is_nudged_gently_and_jumps_sooner() {
        let dt = TICK.as_secs_f64();
        let Correction::Speed(s) = correct_as(10.0, 9.8, 1.0, Follower::Sound) else {
            panic!("a nudge")
        };
        assert!((s - 1.02).abs() < 1e-9, "{s}");
        // THE MEASURE: 100 ms behind is caught in a few seconds, and the most
        // a sound is ever nudged — just under its jump — within twenty.
        let small = ticks_to_catch_up(0.1, Follower::Sound).expect("nudged");
        assert!(f64::from(small) * dt < 5.0, "{small} ticks");
        let most = ticks_to_catch_up(0.39, Follower::Sound).expect("nudged");
        assert!(f64::from(most) * dt < 20.0, "{most} ticks");
        // PAST ITS JUMP a sound jumps, where a picture is still nudged.
        assert_eq!(ticks_to_catch_up(0.5, Follower::Sound), None);
        assert!(ticks_to_catch_up(0.5, Follower::Picture).is_some());
    }

    /// FAILS IF: a voice that just loaded is nudged for seconds instead of
    /// sent where it belongs, or sent where the master WAS — the seek takes
    /// a moment, and it would land behind.
    #[test]
    fn a_settling_voice_is_sent_ahead_or_left_alone() {
        assert_eq!(settle(10.0, 10.1), None);
        let to = settle(10.0, 9.3).expect("sent");
        assert!(to > 10.0 && to < 10.2, "{to}");
        assert!(settle(10.0, 10.5).is_some());
    }

    #[test]
    fn the_offset_is_the_median_and_ignores_one_slow_packet() {
        // The remote clock is 5 s behind ours; one exchange took a second.
        let s = [
            (100.0, 95.01, 100.02),
            (101.0, 96.01, 101.02),
            (102.0, 97.51, 104.0),
        ];
        let o = offset(&s).expect("an offset");
        assert!((o - 5.0).abs() < 0.01, "{o}");
    }
}
