//! HOW BUSY THE PROCESSORS ARE, at the right of the FREE minibuffer — only
//! while they are very busy (the owner, 2026-11-08).
//!
//! The hours' place and the hours' manners (`footer_settings`, `Setting::Cpu`):
//! the quiet ink, nothing at rest, and gone the moment the line is anybody's.
//! Unlike the memory's mark it is not a modeline mark with a colour: it is a
//! glance at a number while the machine labours, and it stands beside the
//! hours, to their left, when both are up.
//!
//! # Read on the footer's own second
//!
//! The clock that already beats once a second to expire things reads
//! `/proc/stat` (`ronoctua_memory::machine::cpu_times`, the one border with
//! `/proc`) — microseconds, no clock of its own — and only while the setting
//! is on. What is drawn is the average of the last [`WINDOW`] seconds, so a
//! compile's first second does not flash it up and a pause of one second does
//! not take it down: up at [`SHOWN_AT`], down below [`HIDDEN_BELOW`].

use std::collections::VecDeque;

use ronoctua_memory::machine::CpuTimes;

/// The seconds averaged.
pub const WINDOW: usize = 5;

/// The busy share, in percent, at which it comes up.
pub const SHOWN_AT: u8 = 90;

/// And below which it goes again.
pub const HIDDEN_BELOW: u8 = 80;

/// What the readings came to.
#[derive(Debug, Default)]
pub struct Busy {
    /// The last reading, which the next one is measured against.
    last: Option<CpuTimes>,
    /// The busy share of each of the last seconds, oldest first.
    recent: VecDeque<u8>,
    /// Whether it is up.
    shown: bool,
}

impl Busy {
    /// One second's reading — `None` when `/proc/stat` could not be read,
    /// which forgets what was measured. Says whether what is drawn changed.
    pub fn read(&mut self, now: Option<CpuTimes>) -> bool {
        let was = self.percent();
        match (now, self.last) {
            (Some(now), Some(before)) => {
                if let Some(p) = now.busy_percent_since(before) {
                    if self.recent.len() == WINDOW {
                        self.recent.pop_front();
                    }
                    self.recent.push_back(p);
                }
            }
            (None, _) => self.recent.clear(),
            (Some(_), None) => {}
        }
        self.last = now;
        if let Some(mean) = self.mean() {
            if mean >= SHOWN_AT && self.recent.len() == WINDOW {
                self.shown = true;
            } else if mean < HIDDEN_BELOW {
                self.shown = false;
            }
        } else {
            self.shown = false;
        }
        self.percent() != was
    }

    /// Stops reading: nothing is drawn, and the next reading starts over.
    pub fn stop(&mut self) -> bool {
        let was = self.shown;
        *self = Self::default();
        was
    }

    /// The share to draw, while it is up.
    #[must_use]
    pub fn percent(&self) -> Option<u8> {
        self.mean().filter(|_| self.shown)
    }

    fn mean(&self) -> Option<u8> {
        let n = u32::try_from(self.recent.len()).ok().filter(|n| *n > 0)?;
        let sum: u32 = self.recent.iter().map(|p| u32::from(*p)).sum();
        u8::try_from(sum / n).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(busy: u64, total: u64) -> Option<CpuTimes> {
        Some(CpuTimes { busy, total })
    }

    /// FAILS IF: one busy second brings it up, a moment's pause takes it
    /// down, it does not come up after [`WINDOW`] seconds above
    /// [`SHOWN_AT`], or it stays up once the average falls under
    /// [`HIDDEN_BELOW`].
    #[test]
    fn it_comes_up_only_when_the_machine_labours_and_goes_when_it_rests() {
        let mut b = Busy::default();
        let (mut busy, mut total) = (0, 0);
        let mut second = |b: &mut Busy, share: u64| {
            busy += share;
            total += 100;
            b.read(times(busy, total))
        };
        b.read(times(0, 0));
        assert!(!second(&mut b, 100), "one busy second");
        for _ in 0..3 {
            second(&mut b, 100);
        }
        assert_eq!(b.percent(), None, "four seconds are not the window");
        assert!(second(&mut b, 95), "the window is full and busy");
        assert_eq!(b.percent(), Some(99));
        second(&mut b, 60);
        assert!(b.percent().is_some(), "a pause of one second took it down");
        for _ in 0..4 {
            second(&mut b, 10);
        }
        assert_eq!(b.percent(), None, "it stayed up at rest");
        assert!(b.read(None) || b.percent().is_none());
        b.stop();
        assert_eq!(b.percent(), None);
    }
}
