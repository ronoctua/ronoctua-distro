//! Keeping a participant in step with the master clock.
//!
//! Pure: given how far a participant is from the master, what to do about it.
//! The daemon asks every [`TICK`] and applies the answer to that participant's
//! mpv — a speed, or a seek.

/// How often a participant is measured.
pub const TICK: std::time::Duration = std::time::Duration::from_millis(250);
/// Below this, nothing is done: a correction audible as a pitch wobble costs
/// more than a drift nobody can see.
pub const STILL_MS: f64 = 40.0;
/// Above this a gentle catch-up would take too long: jump.
pub const JUMP_MS: f64 = 1000.0;
/// How much faster or slower a participant may run while catching up.
pub const NUDGE: f64 = 0.05;

/// What to do to a participant.
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
/// `speed` the master's own. Behind means the follower is earlier.
#[must_use]
pub fn correct(master: f64, follower: f64, speed: f64) -> Correction {
    let drift_ms = (follower - master) * 1000.0;
    if drift_ms.abs() < STILL_MS {
        Correction::Keep
    } else if drift_ms.abs() >= JUMP_MS {
        Correction::Seek(master)
    } else if drift_ms < 0.0 {
        Correction::Speed(speed * (1.0 + NUDGE))
    } else {
        Correction::Speed(speed * (1.0 - NUDGE))
    }
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
