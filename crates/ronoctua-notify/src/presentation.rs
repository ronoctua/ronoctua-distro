//! What was DECIDED about a notification — section 4.2.
//!
//! # Why this is not part of [`crate::Notification`]
//!
//! Because they answer different questions and change for different reasons.
//! What arrived is data: an application said a sentence at an instant, and that
//! is a fact nothing can revise. How it appears is policy: the person's filters
//! decide it, they can change it after the fact, and the same notification
//! shown twice may be shown differently.
//!
//! Keeping them in one struct made "the app asked for 5 seconds" and "we give
//! it 6" indistinguishable, and it is the second one the manager has to be able
//! to edit.
//!
//! It is the same split `dbus.rs` already declares at the top: that module
//! translates the bus and decides nothing.

/// Where — and whether — a notification shows up.
///
/// # There is no toast, and it is not the modeline either
///
/// The band is a line (or three) that the footer's own surface grows ABOVE the
/// modeline, exactly as the expansion grows below it. One surface: no popup, no
/// card, no second layer (invariants 14 and 15).
///
/// The two alternatives were tried on paper and both lose. A floating toast is
/// the popup the invariant names. The modeline itself has one line and it is
/// the line that says *where you are* — a notification landing on it takes away
/// the only thing that never should move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Show {
    /// It does not reach the screen, and it does not reach the bell's count.
    ///
    /// It is still recorded: the manager has a column for these, and a filter
    /// nobody can audit is a filter nobody will trust. What it does not do is
    /// interrupt.
    Hidden,
    /// ONE line, elided if it does not fit.
    ///
    /// For what one wants to have been told and does not want to stop for — a
    /// screenshot saved, a copy finished.
    #[default]
    Brief,
    /// UP TO three lines: the summary, the body, and the row of actions.
    ///
    /// Three and not more: past that the band stops being a band and starts
    /// being the content, and there is a whole manager one key away for
    /// reading at length.
    Full,
}

impl Show {
    /// How many lines the band takes at most for this one.
    #[must_use]
    pub const fn ceiling(self) -> u16 {
        match self {
            Self::Hidden => 0,
            Self::Brief => 1,
            Self::Full => 3,
        }
    }

    /// The name the tables and the manager use.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Brief => "brief",
            Self::Full => "full",
        }
    }

    /// From the name a generated table or a saved override carries.
    #[must_use]
    pub fn by_id(s: &str) -> Option<Self> {
        Some(match s {
            "hidden" => Self::Hidden,
            "brief" => Self::Brief,
            "full" => Self::Full,
            _ => return None,
        })
    }

    /// Every one of them, for the manager's list and for the tests.
    pub const ALL: [Self; 3] = [Self::Hidden, Self::Brief, Self::Full];
}

/// How long it stays on screen.
///
/// # Why three names and not a number of seconds
///
/// Because a filter is written by a person, and "3200 ms" is not a decision
/// anybody makes — "quick" is. Three is also the whole useful range: something
/// one glances at, something one reads, and the rare thing one must not miss.
///
/// # And why there is no "forever"
///
/// There was, and it was the critical one: "a critical warning that disappears
/// on its own is a warning that was not given". The owner overruled it, and the
/// reasoning is that nothing may hold the screen: the band pushes the content
/// up while it is there, so a notification that never leaves is a permanent
/// tax on every window. What a critical one gets instead is [`Life::Long`] plus
/// staying unread — it leaves the screen and does not leave the count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Life {
    /// Three seconds. Long enough to notice, short enough not to interrupt.
    Quick,
    /// Six seconds. The default.
    #[default]
    Normal,
    /// Twenty seconds, for the rare exception.
    Long,
}

/// The three durations, in seconds.
///
/// They are not visual tokens — invariant 9 speaks of colour, font, size,
/// spacing and codepoint. They are behaviour policy, and they live here next to
/// the rule that uses them.
const SECONDS_QUICK: u64 = 3;
const SECONDS_NORMAL: u64 = 6;
const SECONDS_LONG: u64 = 20;

impl Life {
    /// How long, as a duration.
    #[must_use]
    pub const fn how_long(self) -> std::time::Duration {
        std::time::Duration::from_secs(match self {
            Self::Quick => SECONDS_QUICK,
            Self::Normal => SECONDS_NORMAL,
            Self::Long => SECONDS_LONG,
        })
    }

    /// The name the tables and the manager use.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Normal => "normal",
            Self::Long => "long",
        }
    }

    /// From the name a generated table or a saved override carries.
    #[must_use]
    pub fn by_id(s: &str) -> Option<Self> {
        Some(match s {
            "quick" => Self::Quick,
            "normal" => Self::Normal,
            "long" => Self::Long,
            _ => return None,
        })
    }

    /// The class nearest to a duration somebody asked for.
    ///
    /// Nearest by the midpoints between the three, so `4 s` reads as normal and
    /// `3,4 s` as quick. A sender that asks for a minute gets `Long` and not a
    /// minute: the band pushes the content up while it is there, and how long
    /// that lasts is not the sender's decision to make.
    #[must_use]
    pub fn nearest(d: std::time::Duration) -> Self {
        let ms = d.as_millis();
        let mid = |a: Self, b: Self| (a.how_long().as_millis() + b.how_long().as_millis()) / 2;
        if ms <= mid(Self::Quick, Self::Normal) {
            Self::Quick
        } else if ms <= mid(Self::Normal, Self::Long) {
            Self::Normal
        } else {
            Self::Long
        }
    }

    /// Every one of them, for the manager's list and for the tests.
    pub const ALL: [Self; 3] = [Self::Quick, Self::Normal, Self::Long];
}

/// The highest level there is. A level is a number from 0 to this.
///
/// # Why a level, when the protocol already has an urgency
///
/// Because the protocol has THREE steps and they come from the application,
/// which is exactly the party with an interest in shouting. The level is ours:
/// it orders the queue, it can be tested by a filter (`level at-least 80`), and
/// it is what an external service will set when it posts through the API the
/// owner intends to put online. The urgency becomes the DEFAULT level, and
/// nothing more.
pub const TOP_LEVEL: u8 = 100;

/// What is done about the notification, once the filters have spoken.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Presentation {
    /// Whether it shows, and in how many lines.
    pub show: Show,
    /// How long it stays.
    pub life: Life,
    /// The loud formatting, instead of the quiet one.
    pub loud: bool,
    /// `0..=TOP_LEVEL`. It orders the queue and it can be filtered on.
    pub level: u8,
    /// A glyph of the font, when something asked for one by name.
    ///
    /// `None` = the icon chain decides (section 4.2), which is the usual case.
    pub glyph: Option<char>,
    /// The text alternates between its tone and the loud one, once a second.
    ///
    /// It rides the clock the footer already beats to expire things, so it
    /// costs nothing.
    pub blink: bool,
    /// Asked to vibrate. **Nothing does it**, and the manager says so.
    ///
    /// A machine like this has no haptic device. The field exists because the
    /// external API the owner is going to put online will set it, and dropping
    /// it at the door would make the same notification mean different things
    /// depending on where it entered. What it must not be is a silent switch —
    /// see `crate::presentation::Effect`.
    pub vibrate: bool,
    /// Asked to play a sound. **Nothing does it**, and the manager says so.
    ///
    /// Same shape as `vibrate`, different reason: this one is not missing a
    /// device, it is missing an implementation, and it is in `docs/PENDING.md`.
    pub audio: bool,
    /// Which catalogue entry recognised it. `None` = it came from outside and
    /// matched nothing.
    pub catalogue: Option<String>,
    /// Which rule had the last word. `None` = no rule matched.
    ///
    /// The manager DRAWS this. A filter that does not say why it caught
    /// something is a filter nobody can correct.
    pub by: Option<String>,
}

/// A switch of the presentation that has no effect on this machine.
///
/// # Why a list, and why the manager reads it
///
/// The `CLAUDE.md` records the trap by name: a switch that switches nothing off —
/// a switch that switches nothing off, and the day one finds out is the day
/// something did not happen. Two of these fields are in that state on purpose
/// (see [`Presentation::vibrate`] and [`Presentation::audio`]).
///
/// So they are not silent: the manager draws the reason next to the switch, and
/// a check makes sure a field with neither an effect nor a reason cannot exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inert {
    /// The field's name, as the manager and the tables spell it.
    pub field: &'static str,
    /// The text key that says WHY it does nothing.
    pub why: &'static str,
}

/// Every switch that is declared and does nothing.
///
/// Two, today. The list is what the manager reads and what the
/// `switches-that-do-nothing-say-so` check counts against the struct.
pub const INERT: &[Inert] = &[
    Inert {
        field: "vibrate",
        why: "notif-inert-vibrate",
    },
    Inert {
        field: "audio",
        why: "notif-inert-audio",
    },
];

impl Presentation {
    /// What an ARRIVING notification gets before any table is consulted.
    ///
    /// The urgency decides the level and the loudness; the duration the sender
    /// asked for, when it asked for one, picks the nearest of the three
    /// classes. Nobody's filter has spoken yet, so honouring the request here
    /// is the only place it can be honoured without overruling a person.
    #[must_use]
    pub fn for_arrival(n: &crate::Notification) -> Self {
        let mut p = Self::from_urgency(n.urgency);
        if let Some(d) = n.asked {
            p.life = Life::nearest(d);
        }
        p
    }

    /// What a notification gets when nothing has an opinion about it.
    ///
    /// The urgency is the only input, because it is the only thing the protocol
    /// gives that says anything about importance.
    #[must_use]
    pub fn from_urgency(u: crate::Urgency) -> Self {
        let (level, life, loud) = match u {
            crate::Urgency::Low => (25, Life::Quick, false),
            crate::Urgency::Normal => (50, Life::Normal, false),
            crate::Urgency::Critical => (90, Life::Long, true),
        };
        Self {
            show: Show::Brief,
            life,
            loud,
            level,
            glyph: None,
            blink: false,
            vibrate: false,
            audio: false,
            catalogue: None,
            by: None,
        }
    }
}

/// A partial presentation — what a catalogue entry or a rule has to say.
///
/// # Why every field is optional
///
/// So a rule can say one thing. "Everything from the backup goes quiet" should
/// not have to restate the level, the glyph and the duration; and if it did,
/// the day the default level changes, every rule would keep the old one without
/// anybody noticing.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Verdict {
    /// See [`Presentation::show`].
    pub show: Option<Show>,
    /// See [`Presentation::life`].
    pub life: Option<Life>,
    /// See [`Presentation::loud`].
    pub loud: Option<bool>,
    /// See [`Presentation::level`].
    pub level: Option<u8>,
    /// See [`Presentation::glyph`].
    pub glyph: Option<char>,
    /// See [`Presentation::blink`].
    pub blink: Option<bool>,
    /// See [`Presentation::vibrate`].
    pub vibrate: Option<bool>,
    /// See [`Presentation::audio`].
    pub audio: Option<bool>,
}

impl Verdict {
    /// Lays this verdict over a presentation. What it does not name, it leaves.
    pub fn over(&self, p: &mut Presentation) {
        if let Some(v) = self.show {
            p.show = v;
        }
        if let Some(v) = self.life {
            p.life = v;
        }
        if let Some(v) = self.loud {
            p.loud = v;
        }
        if let Some(v) = self.level {
            p.level = v.min(TOP_LEVEL);
        }
        if let Some(v) = self.glyph {
            p.glyph = Some(v);
        }
        if let Some(v) = self.blink {
            p.blink = v;
        }
        if let Some(v) = self.vibrate {
            p.vibrate = v;
        }
        if let Some(v) = self.audio {
            p.audio = v;
        }
    }

    /// Does it say anything at all?
    ///
    /// The manager uses it to tell "the distro's default" from "your
    /// adjustment": an empty override is not an adjustment, it is a leftover.
    #[must_use]
    pub const fn says_nothing(&self) -> bool {
        self.show.is_none()
            && self.life.is_none()
            && self.loud.is_none()
            && self.level.is_none()
            && self.glyph.is_none()
            && self.blink.is_none()
            && self.vibrate.is_none()
            && self.audio.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Urgency;

    #[test]
    fn nothing_stays_on_screen_for_ever() {
        // The owner's decision, and it overturned the rule that a critical one
        // never expires. The band pushes the content up while it is there, so
        // one that never leaves is a permanent tax on every window.
        for life in Life::ALL {
            assert!(life.how_long().as_secs() > 0);
        }
        let p = Presentation::from_urgency(Urgency::Critical);
        assert_eq!(
            p.life,
            Life::Long,
            "the critical one stays, but not forever"
        );
        assert!(p.loud, "the critical one is loud by default");
    }

    #[test]
    fn what_a_sender_asks_for_becomes_the_nearest_class() {
        use std::time::Duration;
        // Honoured HERE and nowhere else: at this point no filter has spoken,
        // so nothing is being overruled.
        assert_eq!(Life::nearest(Duration::from_millis(500)), Life::Quick);
        assert_eq!(Life::nearest(Duration::from_secs(4)), Life::Quick);
        assert_eq!(Life::nearest(Duration::from_secs(6)), Life::Normal);
        // A minute is not a minute: how long the band holds the screen is not
        // the sender's decision to make.
        assert_eq!(Life::nearest(Duration::from_secs(60)), Life::Long);
    }

    #[test]
    fn the_names_go_and_come_back() {
        // The tables in Nix and the saved overrides carry the NAME. A name that
        // does not come back is a filter that silently stops applying.
        for s in Show::ALL {
            assert_eq!(Show::by_id(s.id()), Some(s));
        }
        for l in Life::ALL {
            assert_eq!(Life::by_id(l.id()), Some(l));
        }
        assert_eq!(Show::by_id("toast"), None);
    }

    #[test]
    fn a_verdict_only_changes_what_it_names() {
        // The whole reason the fields are optional: a rule that says "quiet"
        // must not also freeze the level at whatever the default was the day it
        // was written.
        let mut p = Presentation::from_urgency(Urgency::Normal);
        let before = p.level;
        Verdict {
            show: Some(Show::Hidden),
            ..Verdict::default()
        }
        .over(&mut p);
        assert_eq!(p.show, Show::Hidden);
        assert_eq!(p.level, before);
    }

    #[test]
    fn a_level_past_the_top_is_brought_back() {
        let mut p = Presentation::from_urgency(Urgency::Low);
        Verdict {
            level: Some(255),
            ..Verdict::default()
        }
        .over(&mut p);
        assert_eq!(p.level, TOP_LEVEL);
    }

    #[test]
    fn a_switch_that_does_nothing_says_why() {
        // `vibrate` and `audio` are declared and inert. The trap this repo has
        // already paid for is the switch that switches nothing and says
        // nothing.
        let named: Vec<&str> = INERT.iter().map(|i| i.field).collect();
        assert!(named.contains(&"vibrate"));
        assert!(named.contains(&"audio"));
        for i in INERT {
            assert!(
                i.why.starts_with("notif-inert-"),
                "{} with no reason",
                i.field
            );
        }
    }

    #[test]
    fn an_empty_override_is_not_an_adjustment() {
        assert!(Verdict::default().says_nothing());
        assert!(
            !Verdict {
                loud: Some(false),
                ..Verdict::default()
            }
            .says_nothing()
        );
    }
}
