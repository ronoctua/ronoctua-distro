//! The queue: what is on screen, what is waiting, and what happened — 4.2.
//!
//! **One at a time.** The band above the modeline has one message region, not a
//! stack of cards: invariant 14 allows no floating popup and invariant 15 no
//! card. So either the notification is on the band, or it is in the queue.
//!
//! # Why time comes in as a parameter
//!
//! [`Queue::tick`] receives the instant instead of calling `Instant::now()`.
//! That makes the whole expiry policy testable without sleeping: a test that
//! waits 6 seconds to prove something expires is a test nobody runs.
//!
//! # Two clocks, and they are not interchangeable
//!
//! `Instant` decides when something leaves the screen — it is monotonic, and a
//! clock that jumps must not make a notification vanish. `SystemTime` decides
//! when something leaves the HISTORY, fifteen days later — an `Instant` does
//! not survive a reboot, and the history does.

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime};

use crate::action::Action;
use crate::presentation::{Presentation, Show};

/// Urgency, as `org.freedesktop.Notifications` defines it.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum Urgency {
    /// It gives way to anything more urgent.
    Low,
    /// The default, and the protocol's own when a sender says nothing.
    #[default]
    Normal,
    /// The sender says it matters. It becomes a high level and a long life —
    /// see [`Presentation::from_urgency`].
    Critical,
}

impl Urgency {
    /// Converts the protocol's integer. An unknown value becomes `Normal`.
    ///
    /// It is not an error: the specification allows future values, and refusing
    /// the notification over that would lose information the user wants.
    #[must_use]
    pub const fn from_protocol(v: u8) -> Self {
        match v {
            0 => Self::Low,
            2 => Self::Critical,
            _ => Self::Normal,
        }
    }
}

/// A notification, as it ARRIVED. What is decided about it is a
/// [`Presentation`], and it lives apart on purpose.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Notification {
    /// The protocol's id. Stable for as long as the notification exists.
    pub id: u32,
    /// The name of the application that sent it.
    pub app: String,
    /// The `IconName` requested, if any. It enters the icon chain.
    ///
    /// It may also be a `file://` URL: that is what the compositor puts there
    /// for a screenshot, and it is where an action gets its file.
    pub icon: Option<String>,
    /// A short title.
    pub summary: String,
    /// Long text. It may be empty.
    pub body: String,
    /// Urgency, as the sender declared it.
    pub urgency: Urgency,
    /// How long the sender ASKED for. `None` = it did not say.
    ///
    /// It is a request and not a decision: what decides is the filter book.
    /// Recorded because with nothing else having an opinion it picks the
    /// nearest of the three classes, and throwing it away would make
    /// `notify-send -t 1000` mean nothing at all.
    pub asked: Option<Duration>,
    /// Tags, for a filter to test. Ours come from the catalogue.
    pub tags: Vec<String>,
    /// What it lets you do.
    pub actions: Vec<Action>,
    /// When it arrived, on the wall clock. The fifteen days count from here.
    pub at: SystemTime,
}

/// A hand-written `Default`, because `SystemTime` has none.
///
/// It exists for the file format and not for the code: `serde(default)` is what
/// lets a record written before a field existed still load, instead of turning
/// the whole history into a parse error. `UNIX_EPOCH` is the honest stand-in —
/// a record with no date is older than any cutoff, which is what pruning it
/// should conclude.
impl Default for Notification {
    fn default() -> Self {
        Self {
            id: 0,
            app: String::new(),
            icon: None,
            summary: String::new(),
            body: String::new(),
            urgency: Urgency::Normal,
            asked: None,
            tags: Vec::new(),
            actions: Vec::new(),
            at: SystemTime::UNIX_EPOCH,
        }
    }
}

impl Notification {
    /// Builds one from the protocol's fields.
    ///
    /// `expira_ms` does not enter here: it is [`Notification::asked`], and it
    /// is set by whoever read the message off the bus.
    #[must_use]
    pub fn fresh_one(
        id: u32,
        app: String,
        icon: Option<String>,
        summary: String,
        body: String,
        urgency: Urgency,
    ) -> Self {
        Self {
            id,
            app,
            icon,
            summary,
            body,
            urgency,
            asked: None,
            tags: Vec::new(),
            actions: Vec::new(),
            at: SystemTime::now(),
        }
    }

    /// Reads the protocol's `expire_timeout`, in ms.
    ///
    /// `-1` = "let the daemon decide"; `0` = "never expires", which this system
    /// does not have — see [`crate::presentation::Life`]. Both become `None`, meaning "no request".
    #[must_use]
    pub fn asking(mut self, expira_ms: i32) -> Self {
        self.asked = u64::try_from(expira_ms)
            .ok()
            .filter(|ms| *ms > 0)
            .map(Duration::from_millis);
        self
    }

    /// One line for the band. `app: summary`.
    ///
    /// The body does not go in: it may have paragraphs, and this is one line.
    /// `Show::Full` draws the body on its own line.
    #[must_use]
    pub fn line(&self) -> String {
        if self.app.is_empty() {
            self.summary.clone()
        } else {
            format!("{}: {}", self.app, self.summary)
        }
    }

    /// What NAMES the record this notification makes.
    ///
    /// The pair, and not the id — see [`RecordKey`].
    #[must_use]
    pub const fn key(&self) -> RecordKey {
        RecordKey {
            id: self.id,
            at: self.at,
        }
    }

    /// The file this notification is about, from a `file://` icon.
    ///
    /// It is how the compositor hands over a screenshot's path, and it is what
    /// an action opens. Percent escapes are undone: the URL is built by
    /// `g_filename_to_uri`, and a path with a space arrives as `%20`.
    #[must_use]
    pub fn file(&self) -> Option<std::path::PathBuf> {
        let url = self.icon.as_deref()?.strip_prefix("file://")?;
        Some(std::path::PathBuf::from(unescape(url)))
    }
}

/// Undoes `%XX` in a file URL.
///
/// Small and here rather than a dependency: this is the only URL this system
/// ever parses, and it is always one somebody else built from a local path.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let raw = s.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%'
            && i + 2 < raw.len()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b as char);
            i += 3;
            continue;
        }
        out.push(raw[i] as char);
        i += 1;
    }
    out
}

/// What one beat of the clock did.
///
/// # Why the id comes back, and not just "something happened"
///
/// Because the specification wants a `NotificationClosed` with a REASON, and
/// clients act on it: one that sees `Expired` will send another, one that sees
/// `ByHand` will stop. Without the id there is nobody to tell, and the sender
/// waits for a signal that never comes — which is how a progress notification
/// ends up stuck on somebody's screen for ever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tick {
    /// Which one left the band because its time ran out.
    pub expired: Option<u32>,
    /// Whether anything on screen changed at all.
    pub changed: bool,
}

/// What is on the band right now.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// What arrived.
    pub notification: Notification,
    /// What was decided about it.
    pub presentation: Presentation,
    /// Since when it has been on screen.
    pub since: Instant,
}

/// One line of the history.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Record {
    /// What arrived.
    pub notification: Notification,
    /// What was decided about it.
    pub presentation: Presentation,
    /// Has the person seen it?
    ///
    /// A notification is read when it is dismissed by hand, or when the manager
    /// is opened on it. Merely EXPIRING does not mark it read — that is the
    /// whole point of the count surviving the band.
    pub read: bool,
}

impl Record {
    /// Does it count towards the bell?
    ///
    /// A suppressed one never does. A badge that counts what the person asked
    /// never to see is a badge that cannot be brought to zero.
    #[must_use]
    pub const fn counts(&self) -> bool {
        !self.read && !matches!(self.presentation.show, Show::Hidden)
    }

    /// What NAMES this record.
    #[must_use]
    pub const fn key(&self) -> RecordKey {
        self.notification.key()
    }
}

/// What names ONE record of the history.
///
/// # Why the protocol's id is not enough
///
/// Because it is not unique, and nothing ever promised it would be. The bus's
/// counter starts at 1 on every start of this process ([`crate::dbus`]), the
/// ones we raise ourselves count DOWN from `u32::MAX`, and the history outlives
/// both by fifteen days — so one restart is enough for the same id to name
/// several records. Inside a single session a progress bar does it on its own:
/// `replaces_id` reuses the id BY DESIGN, and every arrival is remembered.
///
/// The consequence was on screen, and it is what the owner reported: marking
/// read and forgetting walked the history with `id ==` and acted on every
/// match, and because the list is drawn newest first the extra ones were always
/// BELOW the row the person was looking at.
///
/// The pair costs nothing — both halves are already written to the file, so
/// both processes already agree on them — and `at` is the wall clock at
/// arrival, which two records of one id never share.
///
/// The id STAYS the protocol's: it is what `replaces_id`, `dismiss` and
/// `NotificationClosed` name, and none of those is about the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordKey {
    /// The protocol's id.
    pub id: u32,
    /// When it arrived, on the wall clock.
    pub at: SystemTime,
}

/// How many records the history keeps in memory.
///
/// A ceiling for MEMORY, not for taste: an application in a loop can generate
/// thousands per minute. The fifteen days are the other limit, and the smaller
/// of the two wins — see [`crate::store`].
pub const HISTORY_CEILING: usize = 400;

/// How long a record survives, at most.
///
/// The owner's number. It is long enough that "what was that thing on Tuesday?"
/// is answerable, and short enough that the file stays a file.
pub const HISTORY_DAYS: u64 = 15;

/// The queue: one on the band, the rest waiting, everything recorded.
#[derive(Debug, Default)]
pub struct Queue {
    showing: Option<Shown>,
    pending: VecDeque<(Notification, Presentation)>,
    history: Vec<Record>,
}

impl Queue {
    /// An empty queue.
    #[must_use]
    pub fn fresh_one() -> Self {
        Self::default()
    }

    /// Starts from a history that was read off disk.
    #[must_use]
    pub fn with_history(history: Vec<Record>) -> Self {
        Self {
            history,
            ..Self::default()
        }
    }

    /// Receives a notification with what was decided about it.
    ///
    /// A **suppressed** one is recorded and never reaches the band: that is the
    /// filter doing its job, and the record is what lets the manager show what
    /// was suppressed.
    ///
    /// Otherwise it takes the band if it is free. If it is not, a notification
    /// of a STRICTLY higher level takes the place of the one that is there, and
    /// the one that leaves goes back to the FRONT of the queue — jumping the
    /// queue and losing are different things. Equal levels do not unseat each
    /// other, or two of them would erase one another in turn.
    pub fn enqueue(&mut self, n: Notification, p: Presentation, now: Instant) {
        self.remember(n.clone(), p.clone());
        if matches!(p.show, Show::Hidden) {
            return;
        }
        match self.showing.take() {
            None => {
                self.showing = Some(Shown {
                    notification: n,
                    presentation: p,
                    since: now,
                });
            }
            Some(current) => {
                if p.level > current.presentation.level {
                    self.pending
                        .push_front((current.notification, current.presentation));
                    self.showing = Some(Shown {
                        notification: n,
                        presentation: p,
                        since: now,
                    });
                } else {
                    self.showing = Some(current);
                    self.insert_by_level(n, p);
                }
            }
        }
    }

    /// Puts it in the queue after everything of at least its level.
    ///
    /// Stable within a level: two notifications of the same importance keep the
    /// order they arrived in, which is the order the person expects to read.
    fn insert_by_level(&mut self, n: Notification, p: Presentation) {
        let at = self
            .pending
            .iter()
            .position(|(_, q)| q.level < p.level)
            .unwrap_or(self.pending.len());
        self.pending.insert(at, (n, p));
    }

    fn remember(&mut self, n: Notification, p: Presentation) {
        // AND SOME ARRIVE READ. It is news you do not answer — a device
        // connecting, a device going — and the bell is for what still wants
        // something from you. See `Presentation::auto_read`; it does not touch
        // whether the band shows it, which is `show`'s question.
        let read = p.auto_read;
        self.history.push(Record {
            notification: n,
            presentation: p,
            read,
        });
        if self.history.len() > HISTORY_CEILING {
            self.history.remove(0);
        }
    }

    /// Expires the one on the band if its time is up and pulls the next.
    pub fn tick(&mut self, now: Instant) -> Tick {
        let Some(current) = self.showing.as_ref() else {
            return Tick {
                expired: None,
                changed: self.pull(now),
            };
        };
        if now.duration_since(current.since) < self.how_long(current) {
            return Tick {
                expired: None,
                changed: false,
            };
        }
        // EXPIRING DOES NOT MARK IT READ. The band is where one is told; the
        // count is where one is reminded, and the two are not the same event.
        let id = current.notification.id;
        self.showing = None;
        self.pull(now);
        Tick {
            expired: Some(id),
            changed: true,
        }
    }

    /// How long the one on the band gets.
    ///
    /// It comes off the presentation and nowhere else. What the sender asked
    /// for was already folded in when the presentation was built
    /// (`Presentation::for_arrival`), and a rule may have overruled it since —
    /// which is the point of the person having filters at all.
    ///
    /// A method and not a field so the whole policy stays in one expression:
    /// the "critical never expires" special case used to live in the
    /// constructor, and finding it meant reading two files.
    #[allow(clippy::unused_self)]
    fn how_long(&self, s: &Shown) -> Duration {
        s.presentation.life.how_long()
    }

    /// Marks the one on the band read and takes it off at once.
    ///
    /// It is what the clickable circle does, and what `notif-dismiss` does.
    pub fn dismiss_current(&mut self, now: Instant) -> bool {
        match self.showing.take() {
            Some(s) => {
                self.mark_read(s.notification.key());
                self.pull(now);
                true
            }
            None => false,
        }
    }

    /// Closes one by id, wherever it is. The protocol's `CloseNotification`.
    ///
    /// BY ID, because that is the only name the sender has: `CloseNotification`
    /// carries the number the bus handed out. What it marks read is named the
    /// other way — by the record ([`RecordKey`]) of the very notification it
    /// took off, so closing one does not touch a record from a past session
    /// that happens to have been given the same number.
    pub fn dismiss(&mut self, id: u32, now: Instant) -> bool {
        if let Some(s) = self.showing.as_ref().filter(|s| s.notification.id == id) {
            let key = s.notification.key();
            self.showing = None;
            self.mark_read(key);
            self.pull(now);
            return true;
        }
        let gone: Vec<RecordKey> = self
            .pending
            .iter()
            .filter(|(n, _)| n.id == id)
            .map(|(n, _)| n.key())
            .collect();
        self.pending.retain(|(n, _)| n.id != id);
        if gone.is_empty() {
            return false;
        }
        for key in gone {
            self.mark_read(key);
        }
        true
    }

    /// Marks a record read. It is what takes it off the bell's count.
    pub fn mark_read(&mut self, key: RecordKey) -> bool {
        self.set_read(key, true)
    }

    /// Marks a record UNREAD, and puts it back on the bell's count.
    ///
    /// The gesture is the same key and the same target as marking read — one
    /// control, and the envelope on the row says which way it goes. Without it
    /// the manager was a place where a decision could not be undone: a row
    /// marked read by a slip of the hand was read for fifteen days.
    pub fn mark_unread(&mut self, key: RecordKey) -> bool {
        self.set_read(key, false)
    }

    /// Both of the above. It returns whether anything actually moved.
    fn set_read(&mut self, key: RecordKey, read: bool) -> bool {
        let mut touched = false;
        for r in self.history.iter_mut().filter(|r| r.key() == key) {
            touched |= r.read != read;
            r.read = read;
        }
        touched
    }

    /// Forgets a record: it leaves the history for good.
    ///
    /// # Why this exists at all
    ///
    /// The history is the record of what happened, and for everything else in
    /// this system that is the end of the argument — dismissing removes from
    /// the QUEUE and never from the record. But fifteen days is a long time to
    /// live with one line somebody does not want, and the alternative was
    /// waiting for it to age out. It is the person's own record.
    ///
    /// It does not touch the band: what is on screen is on screen until it is
    /// dismissed or expires, and pulling it out from under the eye would be the
    /// list acting on something the person is reading.
    pub fn forget(&mut self, key: RecordKey) -> bool {
        let before = self.history.len();
        self.history.retain(|r| r.key() != key);
        before != self.history.len()
    }

    /// Marks everything read.
    pub fn mark_all_read(&mut self) -> usize {
        let mut n = 0;
        for r in self.history.iter_mut().filter(|r| !r.read) {
            r.read = true;
            n += 1;
        }
        n
    }

    fn pull(&mut self, now: Instant) -> bool {
        if self.showing.is_some() {
            return false;
        }
        match self.pending.pop_front() {
            Some((n, p)) => {
                self.showing = Some(Shown {
                    notification: n,
                    presentation: p,
                    since: now,
                });
                true
            }
            None => false,
        }
    }

    /// What is on the band right now.
    #[must_use]
    pub const fn showing(&self) -> Option<&Shown> {
        self.showing.as_ref()
    }

    /// `(which one is on the band, how many there are in all)`.
    ///
    /// `None` when there is at most ONE: a counter that says `1/1` tells the
    /// person nothing and costs three columns of a line that elides. It is what
    /// the owner asked for, literally — "shown only when there is more than
    /// one".
    #[must_use]
    pub fn position(&self) -> Option<(usize, usize)> {
        let total = usize::from(self.showing.is_some()) + self.pending.len();
        (total > 1).then_some((1, total))
    }

    /// How much of the band's time is left, from 1.0 to 0.0.
    ///
    /// It is what the eighths pie draws. `None` when nothing is showing.
    #[must_use]
    pub fn fraction_left(&self, now: Instant) -> Option<f32> {
        let s = self.showing.as_ref()?;
        let whole = self.how_long(s).as_secs_f32();
        if whole <= 0.0 {
            return Some(0.0);
        }
        let gone = now.duration_since(s.since).as_secs_f32();
        Some((1.0 - gone / whole).clamp(0.0, 1.0))
    }

    /// How many are waiting their turn.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// How many the bell counts: unread, and not suppressed.
    #[must_use]
    pub fn unread(&self) -> usize {
        self.history.iter().filter(|r| r.counts()).count()
    }

    /// How many a filter kept off the screen.
    ///
    /// The other half of what the pointer says over the bell: "three unread,
    /// one filtered out". It is the same partition the manager's right-hand
    /// column draws, counted here so the modeline does not build the lists to
    /// learn a number.
    #[must_use]
    pub fn suppressed(&self) -> usize {
        self.history
            .iter()
            .filter(|r| matches!(r.presentation.show, Show::Hidden))
            .count()
    }

    /// Takes what ANOTHER process wrote to the history file.
    ///
    /// # Why this exists
    ///
    /// The manager has two doors, and the window is a process of its own: it
    /// marks records read and forgets them by writing the file. The footer
    /// read that file ONCE, at start-up, and wrote its whole vector over it
    /// on every change — so a record forgotten in the window came back the
    /// moment any notification arrived, and the window's next reread showed
    /// it returned. One store, two writers, and the last one won.
    ///
    /// The reconciliation is BY THE RECORD'S NAME ([`RecordKey`]) and in one
    /// direction only: what the file says about a record the queue HAS is
    /// taken (read or unread, and gone), and what the queue has and the file
    /// does not is taken as forgotten. Records the file has and the queue does
    /// not are NOT added back — the queue is what is live, and a record it has
    /// already pruned is not news.
    ///
    /// # Why `read` is taken in BOTH directions
    ///
    /// It used to be copied only when the file said `true`, which was right
    /// while `true` was the only thing that could be said. Now a row can be
    /// marked UNREAD by hand, and a one-way reconciliation would undo that on
    /// the other door's next write: the file is the broadcast, so for a record
    /// the file holds, the file's answer is the answer.
    ///
    /// It returns whether anything moved, so the caller can decide to paint.
    pub fn took_from_disk(&mut self, disk: &[Record]) -> bool {
        use std::collections::HashMap;
        let theirs: HashMap<RecordKey, bool> = disk.iter().map(|r| (r.key(), r.read)).collect();
        // ONLY WHAT ARRIVED BEFORE THE FILE WAS WRITTEN can be judged missing
        // from it: anything newer simply had not been written yet, and
        // dropping it would delete a notification nobody dismissed.
        let newest = disk.iter().map(|r| r.notification.at).max();
        let mut moved = false;
        let before = self.history.len();
        self.history.retain(|r| {
            let known = theirs.contains_key(&r.key());
            known || newest.is_none_or(|t| r.notification.at > t)
        });
        moved |= self.history.len() != before;
        for r in &mut self.history {
            if let Some(read) = theirs.get(&r.key())
                && r.read != *read
            {
                r.read = *read;
                moved = true;
            }
        }
        moved
    }

    /// The whole history, oldest first.
    #[must_use]
    pub fn history(&self) -> &[Record] {
        &self.history
    }

    /// The history, for a store to write.
    #[must_use]
    pub fn history_mut(&mut self) -> &mut Vec<Record> {
        &mut self.history
    }

    /// Is there anything loud waiting or showing?
    ///
    /// The bell reads it: something shouting in the queue must not look like an
    /// ordinary notification waiting.
    #[must_use]
    pub fn has_loud(&self) -> bool {
        self.showing.as_ref().is_some_and(|s| s.presentation.loud)
            || self.pending.iter().any(|(_, p)| p.loud)
    }
}

/// How many eighths of the pie are left, from a fraction.
///
/// Eight steps and never zero while something is still on screen: a pie that
/// has already emptied says "gone" about something that is still there.
#[must_use]
pub fn eighths(fraction: f32) -> u8 {
    let n = (fraction * 8.0).ceil();
    if n.is_nan() {
        return 1;
    }
    (n as u8).clamp(1, 8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::{Life, Verdict};

    fn p(level: u8, life: Life) -> Presentation {
        let mut x = Presentation::from_urgency(Urgency::Normal);
        x.level = level;
        x.life = life;
        x
    }

    fn n(id: u32) -> Notification {
        at(id, u64::from(id))
    }

    /// One with its ARRIVAL named — the other half of a record's name, and the
    /// half two records can differ in while sharing an id.
    fn at(id: u32, secs: u64) -> Notification {
        let mut one = Notification::fresh_one(
            id,
            "app".into(),
            None,
            format!("summary {id}"),
            String::new(),
            Urgency::Normal,
        );
        one.at = std::time::UNIX_EPOCH + Duration::from_secs(secs);
        one
    }

    /// The name of the record `n(id)` makes.
    fn named(id: u32) -> RecordKey {
        n(id).key()
    }

    #[test]
    fn one_at_a_time() {
        // The band has ONE message region. Two at once would need stacking, and
        // stacking would need a card (invariant 15).
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        for i in 1..=3 {
            f.enqueue(n(i), p(50, Life::Normal), t0);
        }
        assert_eq!(f.showing().map(|s| s.notification.id), Some(1));
        assert_eq!(f.pending(), 2);
    }

    #[test]
    fn nothing_holds_the_band_for_ever() {
        // The rule this replaced said a critical one never expires. The band
        // pushes the content up while it is there, so one that never leaves is
        // a permanent tax on every window — the owner's decision.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        let mut critical = Presentation::from_urgency(Urgency::Critical);
        critical.level = 90;
        f.enqueue(n(1), critical, t0);
        assert!(f.showing().is_some());
        assert!(
            f.tick(t0 + Life::Long.how_long() + Duration::from_secs(1))
                .changed
        );
        assert!(
            f.showing().is_none(),
            "the critical one stayed on screen forever"
        );
    }

    #[test]
    fn the_clock_says_which_one_expired() {
        // The specification wants a `NotificationClosed` with a reason, and
        // clients act on it. Without the id there is nobody to tell, and the
        // sender waits for a signal that never comes.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(7), p(50, Life::Quick), t0);
        assert_eq!(f.tick(t0).expired, None);
        let beat = f.tick(t0 + Life::Quick.how_long() + Duration::from_secs(1));
        assert_eq!(beat.expired, Some(7));
        assert!(beat.changed);
    }

    #[test]
    fn what_expired_is_still_unread() {
        // Leaving the band and being seen are not the same event. The count is
        // what reminds you afterwards.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(50, Life::Quick), t0);
        f.tick(t0 + Duration::from_secs(10));
        assert_eq!(f.unread(), 1);
    }

    #[test]
    fn what_the_hover_counts_is_the_column_the_manager_draws() {
        // `suppressed()` is what the bell's hover reads, and it has to be the
        // same number as the manager's right-hand column: one is the
        // pointer's answer to "how many were filtered out", the other is the
        // list of them. It is deliberately NOT the shape of `unread()` —
        // that one empties as things are read, and this one counts what a
        // filter kept out over the whole history, read or not.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        assert_eq!(f.suppressed(), 0);
        let mut hidden = p(50, Life::Normal);
        hidden.show = Show::Hidden;
        f.enqueue(n(1), hidden.clone(), t0);
        f.enqueue(n(2), p(50, Life::Normal), t0);
        f.enqueue(n(3), hidden, t0);
        assert_eq!(f.suppressed(), 2, "two were filtered out");
        assert_eq!(f.unread(), 1, "and only the one that was shown counts");
        // AND READING DOES NOT MOVE IT: a suppressed one was never shown, so
        // there is nothing to read, and the number the hover says is about
        // the filter and not about attention.
        f.mark_read(named(2));
        assert_eq!(f.suppressed(), 2);
        assert_eq!(f.unread(), 0);
    }

    #[test]
    fn what_another_process_wrote_is_taken_and_not_overwritten() {
        // The window forgets a record and marks another read by writing the
        // file. The footer used to read that file once, at start-up, and
        // write its whole vector over it on the next change — so the
        // window's `d` came undone the moment anything arrived.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        for id in 1..=3_u32 {
            let mut one = n(id);
            one.at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(u64::from(id));
            f.enqueue(one, p(50, Life::Normal), t0);
        }
        assert_eq!(f.history().len(), 3);

        // WHAT THE OTHER DOOR WROTE: 2 is gone, 1 is read.
        let mut disk = f.history().to_vec();
        disk.retain(|r| r.notification.id != 2);
        for r in &mut disk {
            if r.notification.id == 1 {
                r.read = true;
            }
        }
        assert!(f.took_from_disk(&disk));
        let ids: Vec<u32> = f.history().iter().map(|r| r.notification.id).collect();
        assert_eq!(ids, vec![1, 3]);
        assert!(f.history()[0].read);
        // AND TWICE IS NOT TWICE AS MUCH: nothing moved the second time, so
        // the door does not paint and does not write back — which is what
        // keeps two processes from answering each other's inotify for ever.
        assert!(!f.took_from_disk(&disk));

        // AND WHAT ARRIVED AFTER THE FILE WAS WRITTEN SURVIVES IT. Without
        // this, a notification landing while the window had the file open
        // would be deleted by the window's next save.
        let mut later = n(9);
        later.at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(99);
        f.enqueue(later, p(50, Life::Normal), t0);
        assert!(!f.took_from_disk(&disk));
        assert!(f.history().iter().any(|r| r.notification.id == 9));
    }

    #[test]
    fn what_the_file_says_about_read_is_taken_both_ways() {
        // A row can be put BACK to unread now, and that decision has to
        // survive the other door writing the file. The reconciliation used to
        // copy `read` only when the file said `true` — right while `true` was
        // the only thing that could be said, and a silent undo the day it was
        // not: the window would put a row back to unread, the footer would
        // keep its own `true`, and the next write from the footer would put
        // the file back the way it was.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(50, Life::Normal), t0);
        assert!(f.mark_read(named(1)));
        assert!(f.history()[0].read);

        let mut disk = f.history().to_vec();
        disk[0].read = false;
        assert!(f.took_from_disk(&disk), "the file's `unread` was ignored");
        assert!(!f.history()[0].read);
        assert_eq!(f.unread(), 1, "and it counts on the bell again");
    }

    #[test]
    fn one_id_can_name_several_records_and_only_one_is_acted_on() {
        // THE DEFECT THE OWNER REPORTED, at its root. The bus's counter starts
        // at 1 on every start of this process and the ones we raise ourselves
        // count down from `u32::MAX`, while the history lives fifteen days —
        // so one restart is enough for an id to name several records. Inside
        // one session `replaces_id` does the same by design: a progress bar
        // keeps its id and every arrival is remembered.
        //
        // Marking read and forgetting walked the history with `id ==` and hit
        // every match. The list is drawn newest first, so the extra ones were
        // always the rows BELOW the one under the cursor — which is exactly
        // what "it applies to all the notifications listed below" describes.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(at(7, 10), p(50, Life::Normal), t0);
        f.enqueue(at(7, 20), p(50, Life::Normal), t0);
        f.enqueue(at(7, 30), p(50, Life::Normal), t0);
        assert_eq!(f.history().len(), 3, "three records, one id");

        let middle = RecordKey {
            id: 7,
            at: SystemTime::UNIX_EPOCH + Duration::from_secs(20),
        };
        assert!(f.mark_read(middle));
        let read: Vec<bool> = f.history().iter().map(|r| r.read).collect();
        assert_eq!(read, vec![false, true, false], "it read more than one");

        assert!(f.forget(middle));
        let left: Vec<SystemTime> = f.history().iter().map(|r| r.notification.at).collect();
        assert_eq!(
            left,
            vec![
                SystemTime::UNIX_EPOCH + Duration::from_secs(10),
                SystemTime::UNIX_EPOCH + Duration::from_secs(30),
            ],
            "it forgot more than one"
        );
    }

    #[test]
    fn a_suppressed_one_never_reaches_the_band_nor_the_count() {
        // The owner's requirement, in both halves: not shown AND not counted.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        let mut hidden = p(50, Life::Normal);
        hidden.show = Show::Hidden;
        f.enqueue(n(1), hidden, t0);
        assert!(f.showing().is_none());
        assert_eq!(f.unread(), 0);
        // And it is RECORDED — a filter nobody can audit is a filter nobody
        // will trust.
        assert_eq!(f.history().len(), 1);
    }

    #[test]
    fn a_higher_level_takes_the_band_without_losing_the_other() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(50, Life::Normal), t0);
        f.enqueue(n(2), p(50, Life::Normal), t0);
        f.enqueue(n(9), p(90, Life::Long), t0);
        assert_eq!(f.showing().map(|s| s.notification.id), Some(9));
        f.dismiss_current(t0);
        assert_eq!(
            f.showing().map(|s| s.notification.id),
            Some(1),
            "the one that left was lost"
        );
    }

    #[test]
    fn an_equal_level_does_not_unseat() {
        // Otherwise two of the same importance would erase one another in turn.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(90, Life::Long), t0);
        f.enqueue(n(2), p(90, Life::Long), t0);
        assert_eq!(f.showing().map(|s| s.notification.id), Some(1));
        assert_eq!(f.pending(), 1);
    }

    #[test]
    fn the_queue_is_ordered_by_level_and_then_by_arrival() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(50, Life::Normal), t0); // takes the band
        f.enqueue(n(2), p(10, Life::Quick), t0);
        // 80 beats the 50 that is on the band, so it takes its place and the
        // one that leaves goes to the FRONT of the queue.
        f.enqueue(n(3), p(80, Life::Normal), t0);
        // 80 does not beat 80, so this one queues — after its equal, which is
        // the order the person expects to read.
        f.enqueue(n(4), p(80, Life::Normal), t0);

        let mut seen = vec![f.showing().map(|s| s.notification.id).unwrap()];
        while f.dismiss_current(t0) {
            if let Some(s) = f.showing() {
                seen.push(s.notification.id);
            }
        }
        assert_eq!(seen, vec![3, 4, 1, 2]);
    }

    #[test]
    fn the_counter_only_exists_with_more_than_one() {
        // `1/1` tells nobody anything and costs three columns of a line that
        // elides.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        assert_eq!(f.position(), None);
        f.enqueue(n(1), p(50, Life::Normal), t0);
        assert_eq!(f.position(), None);
        f.enqueue(n(2), p(50, Life::Normal), t0);
        assert_eq!(f.position(), Some((1, 2)));
    }

    #[test]
    fn the_pie_empties_but_not_before_it_goes() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(50, Life::Normal), t0);
        assert_eq!(eighths(f.fraction_left(t0).unwrap()), 8);
        let half = t0 + Life::Normal.how_long() / 2;
        assert_eq!(eighths(f.fraction_left(half).unwrap()), 4);
        // Still on screen at the very end: a pie that already emptied says
        // "gone" about something that is still there.
        let end = t0 + Life::Normal.how_long();
        assert_eq!(eighths(f.fraction_left(end).unwrap()), 1);
    }

    #[test]
    fn forgetting_takes_it_out_of_the_record_for_good() {
        // Dismissing removes from the QUEUE and never from the record; this is
        // the other thing, and it is the person's own record.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(50, Life::Normal), t0);
        f.enqueue(n(2), p(50, Life::Normal), t0);
        assert!(f.forget(named(1)));
        assert_eq!(f.history().len(), 1);
        assert_eq!(f.history()[0].notification.id, 2);
        // And forgetting what is not there is not an error, it is nothing.
        assert!(!f.forget(named(9)));
    }

    #[test]
    fn dismissing_marks_it_read_and_the_history_keeps_it() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), p(50, Life::Normal), t0);
        f.enqueue(n(2), p(50, Life::Normal), t0);
        f.dismiss_current(t0);
        assert_eq!(f.unread(), 1);
        // The history is the record of what happened; deleting from it would be
        // losing information.
        assert_eq!(f.history().len(), 2);
    }

    #[test]
    fn the_history_has_a_ceiling() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        for i in 0..(HISTORY_CEILING as u32 + 50) {
            let mut hidden = p(10, Life::Quick);
            hidden.show = Show::Hidden;
            f.enqueue(n(i), hidden, t0);
        }
        assert_eq!(f.history().len(), HISTORY_CEILING);
        // It drops the OLD ones, not the recent ones.
        assert_eq!(
            f.history().last().map(|r| r.notification.id),
            Some(HISTORY_CEILING as u32 + 49)
        );
    }

    #[test]
    fn an_unknown_urgency_becomes_normal() {
        assert_eq!(Urgency::from_protocol(7), Urgency::Normal);
        assert_eq!(Urgency::from_protocol(0), Urgency::Low);
        assert_eq!(Urgency::from_protocol(2), Urgency::Critical);
    }

    #[test]
    fn the_bands_line_does_not_carry_the_body() {
        let x = Notification::fresh_one(
            1,
            "git".into(),
            None,
            "push ok".into(),
            "line 1\nline 2".into(),
            Urgency::Normal,
        );
        assert_eq!(x.line(), "git: push ok");
    }

    #[test]
    fn the_file_comes_out_of_the_icon_url() {
        // It is how the compositor hands over a screenshot's path, and it is
        // what the `edit` action opens.
        let x = Notification::fresh_one(
            1,
            "niri".into(),
            Some("file:///home/x/Pictures/Screenshot.png".into()),
            "Screenshot captured".into(),
            String::new(),
            Urgency::Normal,
        );
        assert_eq!(
            x.file().unwrap().to_str().unwrap(),
            "/home/x/Pictures/Screenshot.png"
        );
        // A plain icon NAME is not a file, and must not be treated as one.
        let y = Notification::fresh_one(
            1,
            "a".into(),
            Some("dialog-information".into()),
            "b".into(),
            String::new(),
            Urgency::Normal,
        );
        assert_eq!(y.file(), None);
    }

    #[test]
    fn a_zero_timeout_is_not_a_request_for_ever() {
        // The specification's `0` means "never expires". This system has no
        // such thing, so it is read as "no preference".
        assert_eq!(n(1).asking(0).asked, None);
        assert_eq!(n(1).asking(-1).asked, None);
        assert_eq!(n(1).asking(1500).asked, Some(Duration::from_millis(1500)));
    }

    #[test]
    fn a_verdict_reaches_the_queue_through_the_presentation() {
        // The queue never reads a rule; it is handed the decision. It is the
        // same split `dbus.rs` declares: that module decides nothing either.
        let mut x = Presentation::from_urgency(Urgency::Low);
        Verdict {
            life: Some(Life::Long),
            ..Verdict::default()
        }
        .over(&mut x);
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1), x, t0);
        assert!(
            !f.tick(t0 + Life::Normal.how_long() + Duration::from_secs(1))
                .changed
        );
        assert!(
            f.tick(t0 + Life::Long.how_long() + Duration::from_secs(1))
                .changed
        );
    }
}
