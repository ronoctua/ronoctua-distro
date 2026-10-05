//! The distro's own notifications and filters — section 4.2.
//!
//! # One place, and it is not Rust
//!
//! Everything this system raises is declared in `assets/notifications.nix` and
//! generated from there, in the same shape as the design tokens (section 7.6):
//! the table, the list of text keys both languages have to carry, and the
//! refusal of an invalid field — all from one file, at build time.
//!
//! What that buys is the thing the owner asked for: a single list to manage.
//! Written as Rust, "every notification the distro sends" would be a `match`
//! somewhere plus a `format!` somewhere else, and the honest answer to "which
//! ones are there?" would be a grep.
//!
//! # An entry is a recognition, not a constructor
//!
//! Ours do not all originate here: the radio's, the power's and the capture's
//! are raised by the footer with a TAG, and others arrive from outside — so an
//! entry says how to RECOGNISE a notification and what to make of it, and the
//! same machinery then serves a notification we raise ourselves and one that
//! arrives from outside. Two paths, one table.

use crate::Notification;
use crate::action::{Action, Arg, Doing, Label, Program};
use crate::presentation::{Life, Presentation, Show, Verdict};
use crate::rule::{Field, Rule, Term, Test};

/// One of the distro's notifications.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Stable, kebab, English (invariant 18). The text keys derive from it.
    pub id: String,
    /// How an arriving notification is recognised as being this one.
    ///
    /// Every term has to hold, as in a rule. An empty list would recognise
    /// everything, and the generator refuses that here — an entry is a
    /// particular notification, not a catch-all.
    pub when: Vec<Term>,
    /// The distro's default presentation for it.
    pub apply: Verdict,
    /// Tags it carries, for a filter to test.
    pub tags: Vec<String>,
    /// What it lets you do.
    pub actions: Vec<Action>,
    /// Whether its summary and body are REWRITTEN from our text keys.
    ///
    /// True for the ones that arrive in a foreign language — the compositor's
    /// screenshot says "Screenshot captured", and invariant 8 wants both
    /// languages. False for the ones we raise, which already carry our text.
    pub rewrite: bool,
}

impl Entry {
    /// Does it recognise this notification?
    #[must_use]
    pub fn recognises(&self, n: &Notification, p: &Presentation) -> bool {
        !self.when.is_empty() && self.when.iter().all(|t| t.holds(n, p))
    }

    /// The text key of its summary.
    #[must_use]
    pub fn summary_key(&self) -> String {
        format!("notif-{}-summary", self.id)
    }

    /// The text key of its body.
    #[must_use]
    pub fn body_key(&self) -> String {
        format!("notif-{}-body", self.id)
    }
}

// The generated table. See `nix/scripts/gen-notifications.py`.
//
// It defines `fn generated_entries() -> Vec<Entry>` and
// `fn generated_rules() -> Vec<Rule>`, from `assets/notifications.nix`.
include!(concat!(env!("OUT_DIR"), "/notifications.rs"));

/// The whole decision: catalogue, the distro's filters, and the person's.
///
/// # The order, and why it is this one
///
/// The distro speaks first, from general to specific; then the person does, in
/// the same direction. Nothing the distro ships can overrule an adjustment
/// somebody made on purpose.
///
/// 1. the defaults, from the urgency — the only thing the protocol says about
///    importance;
/// 2. the catalogue entry that recognises it, if any — where identity comes
///    from, and the distro's opinion about ITS OWN notification;
/// 3. the first filter that catches it, the person's own filters before the
///    distro's;
/// 4. the person's adjustment to that FILTER — a class of notifications;
/// 5. the person's adjustment to the catalogue ENTRY — one notification, and
///    therefore the last word.
///
/// Steps 4 and 5 were once folded into 2 and 3, and it was wrong in a way a
/// test caught: adjusting the screenshot entry to be loud did nothing, because
/// a distro rule about low-level notifications ran afterwards and set it quiet
/// again. "Use the distro's default, or customise" has to mean the second half
/// actually wins.
#[derive(Debug, Clone, Default)]
pub struct Book {
    /// The distro's, generated from Nix.
    pub entries: Vec<Entry>,
    /// The distro's filters, generated from Nix, in declared order.
    pub rules: Vec<Rule>,
    /// The person's own filters. They are tried BEFORE the distro's.
    pub mine: Vec<Rule>,
    /// Which of `mine` have been sent to the repository and not come back yet.
    ///
    /// The screen reads it to say "on its way to everybody" instead of
    /// "yours"; nothing else does. When a build brings one back it appears in
    /// `rules` as well, and the copy in `mine` retires — see `Book::settled`.
    pub shared: std::collections::BTreeSet<String>,
    /// The person's adjustments, keyed by the id of an entry or of a rule.
    pub adjustments: std::collections::BTreeMap<String, Verdict>,
}

impl Book {
    /// The distro's, with nothing of the person's yet.
    #[must_use]
    pub fn distro() -> Self {
        Self {
            entries: generated_entries(),
            rules: generated_rules(),
            mine: Vec::new(),
            shared: std::collections::BTreeSet::new(),
            adjustments: std::collections::BTreeMap::new(),
        }
    }

    /// Takes the person's half, retiring what has come back as the distro's.
    ///
    /// # Why the retirement is here
    ///
    /// Because a filter somebody shared travels as TWO copies for a while:
    /// theirs, which keeps working, and the one in the repository, which does
    /// nothing until a build brings it into this table. The day it arrives,
    /// both exist and the catalogue would list it twice.
    ///
    /// So the answer to "has it landed" is asked here, once, against the table
    /// this binary was built with — no socket, no git, no clock. If the id is
    /// among the distro's rules, the local copy has done its job and goes.
    pub fn took(&mut self, mine: Vec<Rule>, shared: std::collections::BTreeSet<String>) {
        let landed: Vec<String> = shared
            .iter()
            .filter(|id| self.rules.iter().any(|r| r.id == **id))
            .cloned()
            .collect();
        self.mine = mine
            .into_iter()
            .filter(|r| !landed.contains(&r.id))
            .collect();
        self.shared = shared
            .into_iter()
            .filter(|id| !landed.contains(id))
            .collect();
        for id in landed {
            tracing::info!(id = %id, "a shared filter came back as the distro's");
        }
    }

    /// What is decided about this notification.
    ///
    /// # A filter tests the tags of the entry that recognised it
    ///
    /// [`Entry::tags`] are "for a filter to test", and a filter is tried here,
    /// before anybody else could add them to the notification — so they are
    /// added for the test. It is also what lets an entry keep an OLD tag: a
    /// notification renamed (a brain module's event, `<module>-<event>`, that
    /// was a tag of the footer's before) carries its old name among its tags,
    /// and a filter somebody saved under it still catches it.
    #[must_use]
    pub fn decide(&self, n: &Notification) -> Presentation {
        let mut p = Presentation::for_arrival(n);

        let recognised = self.entries.iter().find(|e| e.recognises(n, &p));
        if let Some(e) = recognised {
            e.apply.over(&mut p);
            p.catalogue = Some(e.id.clone());
        }
        let tagged;
        let n = match recognised {
            Some(e) if e.tags.iter().any(|t| !n.tags.contains(t)) => {
                let mut with = n.clone();
                with.tags
                    .extend(e.tags.iter().filter(|t| !n.tags.contains(t)).cloned());
                tagged = with;
                &tagged
            }
            _ => n,
        };

        // THE PERSON'S FILTERS FIRST. One of one's own that could not overrule
        // the distro's would not be a filter, it would be a suggestion.
        if let Some(r) = self
            .mine
            .iter()
            .chain(&self.rules)
            .find(|r| r.catches(n, &p))
        {
            r.apply.over(&mut p);
            p.by = Some(r.id.clone());
        }

        // AND NOW THE ADJUSTMENTS, general before specific. See the header.
        for id in [p.by.clone(), p.catalogue.clone()].into_iter().flatten() {
            if let Some(a) = self.adjustments.get(&id) {
                a.over(&mut p);
            }
        }
        p
    }

    /// The catalogue entry with this id.
    #[must_use]
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The actions of whichever entry recognised it. Empty for a stranger.
    #[must_use]
    pub fn actions_of(&self, p: &Presentation) -> Vec<Action> {
        p.catalogue
            .as_deref()
            .and_then(|id| self.entry(id))
            .map(|e| e.actions.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Urgency;
    use crate::presentation::Show;
    use crate::rule::{Field, Test};

    /// What the footer raises after `grim` has written a file.
    ///
    /// The TAG is the whole recognition. It used to be the pair (app, summary),
    /// because the sender was the compositor and an English sentence was all it
    /// gave us; the capture is ours now, so the entry matches a word we write.
    fn shot() -> Notification {
        let mut n = Notification::fresh_one(
            7,
            "screen".into(),
            Some("file:///home/x/Pictures/a.png".into()),
            "Screen captured".into(),
            "The picture was saved to disk.".into(),
            Urgency::Low,
        );
        n.tags = vec!["screenshot".to_owned()];
        n
    }

    #[test]
    fn the_generated_table_is_not_empty() {
        // An empty generation would make every notification a stranger, and
        // nothing would fail — the screen would simply stop knowing what
        // anything is.
        let b = Book::distro();
        assert!(!b.entries.is_empty(), "an empty catalogue");
        for e in &b.entries {
            assert!(!e.when.is_empty(), "`{}` would recognise everything", e.id);
            assert!(!e.id.is_empty());
        }
    }

    #[test]
    fn the_screenshot_is_recognised_discreet_and_has_room() {
        // The owner's ask, and it is two asks: DISCREET is the formatting — no
        // shouting colour — and `full` is the SIZE. It offers to be edited,
        // opened and copied, and one line has no room for the row that carries
        // those.
        let p = Book::distro().decide(&shot());
        assert_eq!(p.catalogue.as_deref(), Some("screenshot"));
        assert_eq!(p.show, Show::Full);
        assert!(!p.loud, "the screenshot does not shout");
        // AND NO DISTRO RULE TAKES THE ROOM BACK. At level 20 the distro's own
        // `small-things-are-quick` caught it and forced one line: the entry
        // said `full` and the screen showed `brief`, with only the manager able
        // to say why.
        assert!(
            p.by.is_none(),
            "a distro rule caught the screenshot: {:?}",
            p.by
        );
        // And its glyph came from the font by NAME — nothing in the design
        // system says "screen capture".
        assert!(p.glyph.is_some());
    }

    #[test]
    fn a_filter_saved_under_an_entrys_old_tag_still_catches_it() {
        // THE ALIAS: an entry renamed keeps its old name among its tags, and a
        // filter somebody saved under the old name — tried BEFORE the tags
        // reach the notification — still catches what the entry recognises.
        let mut book = Book::distro();
        book.entries.push(Entry {
            id: "made-up-renamed".into(),
            when: vec![Term {
                field: Field::Tag,
                test: Test::Is("made-up-renamed".into()),
            }],
            apply: Verdict::default(),
            tags: vec!["made-up-renamed".into(), "made-up-old".into()],
            actions: Vec::new(),
            rewrite: false,
        });
        book.mine.push(Rule {
            id: "mine-quiet".into(),
            when: vec![Term {
                field: Field::Tag,
                test: Test::Is("made-up-old".into()),
            }],
            apply: Verdict {
                show: Some(Show::Hidden),
                ..Verdict::default()
            },
        });
        let mut n = Notification::fresh_one(
            1,
            "an app".into(),
            None,
            "said".into(),
            String::new(),
            Urgency::Normal,
        );
        n.tags = vec!["made-up-renamed".to_owned()];
        let p = book.decide(&n);
        assert_eq!(p.catalogue.as_deref(), Some("made-up-renamed"));
        assert_eq!(
            p.by.as_deref(),
            Some("mine-quiet"),
            "the filter saved under the old tag no longer catches it"
        );
        assert_eq!(p.show, Show::Hidden);
        // AND A NOTIFICATION NO ENTRY RECOGNISES gains no tag.
        n.tags = vec!["nobody-knows".to_owned()];
        assert_ne!(book.decide(&n).by.as_deref(), Some("mine-quiet"));
    }

    #[test]
    fn what_the_bluetooth_says_arrives_already_read() {
        // The owner asked for it off his own screen: a keyboard that
        // reconnects three times had left three things on the bell, and none
        // of them was a thing to do. All four of the radio's notifications
        // arrive read — they still SHOW, which is the moment they are worth
        // anything, and `show` is what decides that.
        let book = Book::distro();
        for tag in [
            "bluetooth-connected",
            "bluetooth-disconnected",
            "bluetooth-pairing",
            "bluetooth-battery-low",
        ] {
            let mut n = Notification::fresh_one(
                1,
                "bluetooth".into(),
                None,
                "a keyboard".into(),
                String::new(),
                Urgency::Normal,
            );
            n.tags = vec![tag.to_owned()];
            let p = book.decide(&n);
            assert_eq!(p.catalogue.as_deref(), Some(tag));
            assert!(p.auto_read, "`{tag}` still counts on the bell");
            // A DEVICE COMING AND GOING IS SAID BY ITS MARK, and the distro
            // keeps those two off the screen (the owner, 2026-09-26). The
            // other two are a question and an alarm, and they show.
            let quiet = matches!(tag, "bluetooth-connected" | "bluetooth-disconnected");
            assert_eq!(
                matches!(p.show, Show::Hidden),
                quiet,
                "`{tag}`: shown when it should not be, or hidden when it should show"
            );
        }
        // AND IT IS THE QUEUE THAT HONOURS IT, not the manager: the record is
        // written read, so nothing has to remember to read it later.
        let mut n = Notification::fresh_one(
            1,
            "bluetooth".into(),
            None,
            "a keyboard".into(),
            String::new(),
            Urgency::Normal,
        );
        n.tags = vec!["bluetooth-battery-low".to_owned()];
        let p = book.decide(&n);
        let mut q = crate::Queue::fresh_one();
        q.enqueue(n, p, std::time::Instant::now());
        assert_eq!(q.history().len(), 1);
        assert!(q.history()[0].read);
        assert_eq!(q.unread(), 0, "the bell counted one that arrives read");
        assert!(q.showing().is_some(), "and it never reached the band");
    }

    #[test]
    fn a_stranger_gets_the_defaults_and_says_so() {
        let n = Notification::fresh_one(
            1,
            "some-app".into(),
            None,
            "oi".into(),
            String::new(),
            Urgency::Normal,
        );
        let p = Book::distro().decide(&n);
        assert_eq!(p.catalogue, None);
        assert_eq!(p.show, Show::Brief);
    }

    #[test]
    fn a_filter_of_mine_beats_the_distros() {
        // A filter of one's own that could not overrule the distro's would be a
        // suggestion, not a filter.
        let mut b = Book::distro();
        b.mine.push(Rule {
            id: "no-screenshot".into(),
            when: vec![Term {
                field: Field::Catalogue,
                test: Test::Is("screenshot".into()),
            }],
            apply: Verdict {
                show: Some(Show::Hidden),
                ..Verdict::default()
            },
        });
        let p = b.decide(&shot());
        assert_eq!(p.show, Show::Hidden);
        assert_eq!(p.by.as_deref(), Some("no-screenshot"));
    }

    #[test]
    fn an_adjustment_reaches_a_catalogue_entry() {
        // "use the distro's default, or customise" — one map, keyed by the id.
        //
        // This one FAILED when the adjustments were applied next to the thing
        // they adjust: the screenshot is level 20, so the distro's
        // `small-things-are-quick` ran afterwards and put it back to quiet. The
        // person's word has to come last.
        let mut b = Book::distro();
        b.adjustments.insert(
            "screenshot".into(),
            Verdict {
                loud: Some(true),
                ..Verdict::default()
            },
        );
        assert!(b.decide(&shot()).loud);
    }

    #[test]
    fn an_entrys_adjustment_beats_a_rules() {
        // Both are the person's, and the more specific one wins: an entry names
        // ONE notification, a rule names a class of them.
        let mut b = Book::distro();
        b.adjustments.insert(
            "small-things-are-quick".into(),
            Verdict {
                level: Some(10),
                ..Verdict::default()
            },
        );
        b.adjustments.insert(
            "screenshot".into(),
            Verdict {
                level: Some(70),
                ..Verdict::default()
            },
        );
        assert_eq!(b.decide(&shot()).level, 70);
    }

    #[test]
    fn the_manager_can_always_say_who_decided() {
        // A filter that does not say why it caught something is a filter
        // nobody can correct.
        let mut b = Book::distro();
        b.mine.push(Rule {
            id: "everything".into(),
            when: vec![],
            apply: Verdict::default(),
        });
        assert_eq!(b.decide(&shot()).by.as_deref(), Some("everything"));
    }
}
