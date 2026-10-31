//! THE TIME WITHOUT USE — the one file that names the compositor's idle
//! notifications, what the automatic lock is armed by.
//!
//! # Why the compositor says it, and not a clock of ours
//!
//! Only the compositor sees every key and every movement of every seat, the
//! virtual keyboard's and the hands' included; a timer here would have to be
//! told of each, and would know nothing of what the person does in a window.
//! `ext-idle-notify-v1` is the compositor counting for us: a notification per
//! seat, with the wait the person chose, says `idled` when that seat has been
//! still that long and `resumed` when it moves again — evented, never polled.
//! It is the owner's approved exception to invariant 3 (2026-10-31), a
//! STANDARD Wayland protocol, never a patch; `one-idle-notify` keeps every
//! other file from naming it.
//!
//! # A video keeps the screen unlocked, and so does music
//!
//! `get_idle_notification` honours the idle INHIBITORS: a video player or a
//! browser showing a video holds one, and while it does the seat never idles.
//! Sound alone holds none — a song in a player with no window — so when every
//! seat has idled the footer asks, off its loop, whether a stream is playing
//! now (`ronoctua_audio::playing`); if one is, nothing locks and the waits
//! start again from now: the next idle is a whole wait later, asked again.
//!
//! # Every seat, or none
//!
//! Independent cursors are seats of their own, and a hand on one is somebody
//! at the machine: the lock waits until EVERY seat has idled. A seat with
//! nothing on it idles once and stays so, which is what it is.

use std::time::Duration;

use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, QueueHandle, globals::GlobalList, protocol::wl_seat::WlSeat,
};
use smithay_client_toolkit::reexports::protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::{self, ExtIdleNotifierV1},
};

/// The notifier's user data: it sends no event.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quiet;

impl<S> Dispatch2<ExtIdleNotifierV1, S> for Quiet {
    fn event(
        &self,
        _: &mut S,
        _: &ExtIdleNotifierV1,
        _: ext_idle_notifier_v1::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
    }
}

/// A notification's user data: the seat it watches.
#[derive(Debug, Clone)]
pub struct OnSeat(WlSeat);

/// What holds the watch, and hears when every seat is still — the footer.
pub trait Idler: Sized + 'static {
    /// The watch.
    fn idle(&mut self) -> &mut Idle;
    /// Every seat has been still for the whole wait.
    fn idle_everywhere(&mut self);
}

impl<S: Idler> Dispatch2<ExtIdleNotificationV1, S> for OnSeat {
    fn event(
        &self,
        state: &mut S,
        _: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
        let still = match event {
            ext_idle_notification_v1::Event::Idled => true,
            ext_idle_notification_v1::Event::Resumed => false,
            _ => return,
        };
        if state.idle().heard(&self.0, still) {
            state.idle_everywhere();
        }
    }
}

/// One seat, watched.
#[derive(Debug)]
struct Watched {
    seat: WlSeat,
    /// Its notification, while the lock is armed.
    notification: Option<ExtIdleNotificationV1>,
    /// Whether it has said `idled` and not `resumed` since.
    still: bool,
}

/// THE WATCH: the notifier, every seat, and the wait armed.
#[derive(Debug, Default)]
pub struct Idle {
    /// `None` where the compositor offers no idle notifications — the
    /// automatic lock then never locks, and says so once.
    notifier: Option<ExtIdleNotifierV1>,
    seats: Vec<Watched>,
    /// The wait, while the automatic lock is on.
    after: Option<Duration>,
}

impl Idle {
    /// Binds the notifier. Its absence is not an error: nothing locks by
    /// itself, and the log says why.
    pub fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Self
    where
        D: Dispatch<ExtIdleNotifierV1, Quiet> + 'static,
    {
        let notifier = globals
            .bind::<ExtIdleNotifierV1, _, _>(qh, 1..=1, Quiet)
            .map_err(|e| tracing::warn!(error = %e, "no idle notifications: the screen never locks by itself"))
            .ok();
        Self {
            notifier,
            seats: Vec::new(),
            after: None,
        }
    }

    /// The wait armed now, if any.
    #[must_use]
    pub const fn armed(&self) -> Option<Duration> {
        self.after
    }

    /// A seat came: watched, when the lock is armed.
    pub fn seat_came<D>(&mut self, seat: &WlSeat, qh: &QueueHandle<D>)
    where
        D: Dispatch<ExtIdleNotificationV1, OnSeat> + 'static,
    {
        self.seat_went(seat);
        let notification = self.notification_for(seat, qh);
        self.seats.push(Watched {
            seat: seat.clone(),
            notification,
            still: false,
        });
    }

    /// A seat went.
    pub fn seat_went(&mut self, seat: &WlSeat) {
        self.seats.retain(|w| {
            let keep = &w.seat != seat;
            if !keep && let Some(n) = &w.notification {
                n.destroy();
            }
            keep
        });
    }

    /// ARMS THE WAIT — or, `None`, disarms it: every seat's notification made
    /// again, counting from now.
    pub fn arm<D>(&mut self, after: Option<Duration>, qh: &QueueHandle<D>)
    where
        D: Dispatch<ExtIdleNotificationV1, OnSeat> + 'static,
    {
        self.after = after;
        self.again(qh);
    }

    /// THE SAME WAIT, from now: what a seat that idled while sound played
    /// gets — the next idle is a whole wait later.
    pub fn again<D>(&mut self, qh: &QueueHandle<D>)
    where
        D: Dispatch<ExtIdleNotificationV1, OnSeat> + 'static,
    {
        let fresh: Vec<Option<ExtIdleNotificationV1>> = self
            .seats
            .iter()
            .map(|w| {
                if let Some(n) = &w.notification {
                    n.destroy();
                }
                self.notification_for(&w.seat, qh)
            })
            .collect();
        for (w, n) in self.seats.iter_mut().zip(fresh) {
            w.notification = n;
            w.still = false;
        }
    }

    /// A notification of `seat` for the wait armed — none while disarmed.
    fn notification_for<D>(
        &self,
        seat: &WlSeat,
        qh: &QueueHandle<D>,
    ) -> Option<ExtIdleNotificationV1>
    where
        D: Dispatch<ExtIdleNotificationV1, OnSeat> + 'static,
    {
        let notifier = self.notifier.as_ref()?;
        let ms = u32::try_from(self.after?.as_millis()).unwrap_or(u32::MAX);
        Some(notifier.get_idle_notification(ms, seat, qh, OnSeat(seat.clone())))
    }

    /// A seat said it went still (`idled`) or moved (`resumed`); answers
    /// whether EVERY seat is still now — and only on the word that made it
    /// so, so one stillness is heard once.
    fn heard(&mut self, seat: &WlSeat, still: bool) -> bool {
        let mut states: Vec<(bool, bool)> = self
            .seats
            .iter()
            .map(|w| (&w.seat == seat, w.still))
            .collect();
        let made_it = heard_in(&mut states, still, self.after.is_some());
        for (w, (_, s)) in self.seats.iter_mut().zip(states) {
            w.still = s;
        }
        made_it
    }

    /// Whether the wait is armed and every seat is still.
    #[must_use]
    pub fn everywhere(&self) -> bool {
        let states: Vec<bool> = self.seats.iter().map(|w| w.still).collect();
        all_still(&states, self.after.is_some())
    }
}

/// The word of the seat marked `true` in `states` (`(is it, still)`), taken:
/// whether EVERY seat is still now, on the word that made it so. Pure.
fn heard_in(states: &mut [(bool, bool)], still: bool, armed: bool) -> bool {
    let Some(at) = states.iter().position(|(is, _)| *is) else {
        return false;
    };
    states[at].1 = still;
    let all: Vec<bool> = states.iter().map(|(_, s)| *s).collect();
    still && all_still(&all, armed)
}

/// Whether the wait is armed, there is a seat, and every one is still. Pure.
fn all_still(states: &[bool], armed: bool) -> bool {
    armed && !states.is_empty() && states.iter().all(|s| *s)
}

/// WHAT AN IDLE COMES TO, once the sound is known — the rule, pure, so it is
/// tested with no compositor and no PipeWire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Somebody came back while the sound was asked: nothing.
    Nothing,
    /// Sound is playing: no lock, and the wait starts again from now.
    WaitAgain,
    /// The screen locks.
    Lock,
}

/// The verdict for an idle: `still` is whether every seat is still NOW,
/// `playing` what PipeWire answered (`None`: it could not be asked).
///
/// UNKNOWN SOUND LOCKS. The two asks pull apart only there: the automatic lock
/// is the person's guard, and a PipeWire that cannot be asked is not a reason
/// to leave the machine open; the log says the lock did not know.
#[must_use]
pub const fn verdict(still: bool, playing: Option<bool>) -> Verdict {
    match (still, playing) {
        (false, _) => Verdict::Nothing,
        (true, Some(true)) => Verdict::WaitAgain,
        (true, Some(false) | None) => Verdict::Lock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: music playing locks the screen, somebody who came back is
    /// locked out, or a PipeWire that cannot be asked keeps it unlocked.
    #[test]
    fn music_waits_and_silence_locks() {
        assert_eq!(verdict(true, Some(true)), Verdict::WaitAgain);
        assert_eq!(verdict(true, Some(false)), Verdict::Lock);
        assert_eq!(verdict(true, None), Verdict::Lock);
        assert_eq!(verdict(false, Some(false)), Verdict::Nothing);
        assert_eq!(verdict(false, None), Verdict::Nothing);
    }

    /// FAILS IF: a watch with nothing armed, or with no seat, says everybody
    /// is still.
    #[test]
    fn nothing_armed_is_nobody_still() {
        let mut idle = Idle::default();
        assert!(!idle.everywhere(), "no seat");
        idle.after = Some(Duration::from_secs(60));
        assert!(!idle.everywhere(), "armed, and still no seat");
        assert!(!all_still(&[true, true], false), "still, and nothing armed");
    }

    /// FAILS IF: one seat's stillness locks while another seat is in use —
    /// independent cursors are somebody at the machine — or one stillness is
    /// heard twice.
    #[test]
    fn every_seat_or_none() {
        // TWO SEATS; the first goes still, the second is in use.
        let mut seats = [(true, false), (false, false)];
        assert!(
            !heard_in(&mut seats, true, true),
            "the other seat is in use"
        );
        // THE SECOND goes still: now everybody is.
        let mut seats = [(false, seats[0].1), (true, seats[1].1)];
        assert!(heard_in(&mut seats, true, true), "every seat is still");
        // A RESUME is never the word that makes it so.
        assert!(!heard_in(&mut seats, false, true));
        // AND A SEAT NOT WATCHED says nothing.
        let mut none = [(false, true)];
        assert!(!heard_in(&mut none, true, true));
    }
}
