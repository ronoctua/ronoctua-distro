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
//! Ours do not all originate here. The screenshot's is sent by the compositor,
//! in English, with the file's path in the icon field — so the entry says how
//! to RECOGNISE it and what to make of it, and the same machinery then serves a
//! notification we raise ourselves and one that arrives from outside. Two
//! paths, one table.

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
            adjustments: std::collections::BTreeMap::new(),
        }
    }

    /// What is decided about this notification.
    #[must_use]
    pub fn decide(&self, n: &Notification) -> Presentation {
        let mut p = Presentation::for_arrival(n);

        if let Some(e) = self.entries.iter().find(|e| e.recognises(n, &p)) {
            e.apply.over(&mut p);
            p.catalogue = Some(e.id.clone());
        }

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

    fn shot() -> Notification {
        Notification::fresh_one(
            7,
            "niri".into(),
            Some("file:///home/x/Imagens/a.png".into()),
            "Screenshot captured".into(),
            "You can paste the image from the clipboard.".into(),
            Urgency::Normal,
        )
    }

    #[test]
    fn the_generated_table_is_not_empty() {
        // An empty generation would make every notification a stranger, and
        // nothing would fail — the screen would simply stop knowing what
        // anything is.
        let b = Book::distro();
        assert!(!b.entries.is_empty(), "catalogo vazio");
        for e in &b.entries {
            assert!(!e.when.is_empty(), "`{}` reconheceria tudo", e.id);
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
        assert!(!p.loud, "a captura nao grita");
        // AND NO DISTRO RULE TAKES THE ROOM BACK. At level 20 the distro's own
        // `small-things-are-quick` caught it and forced one line: the entry
        // said `full` and the screen showed `brief`, with only the manager able
        // to say why.
        assert!(
            p.by.is_none(),
            "uma regra da distro pegou a captura: {:?}",
            p.by
        );
        // And its glyph came from the font by NAME — nothing in the design
        // system says "screen capture".
        assert!(p.glyph.is_some());
    }

    #[test]
    fn a_stranger_gets_the_defaults_and_says_so() {
        let n = Notification::fresh_one(
            1,
            "algum-app".into(),
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
            id: "sem-captura".into(),
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
        assert_eq!(p.by.as_deref(), Some("sem-captura"));
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
            id: "tudo".into(),
            when: vec![],
            apply: Verdict::default(),
        });
        assert_eq!(b.decide(&shot()).by.as_deref(), Some("tudo"));
    }
}
