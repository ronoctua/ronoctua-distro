//! The filter language — section 4.2.
//!
//! A rule looks at a notification and, if every one of its terms matches, says
//! what to do with it: show it or not, for how long, how loudly, at what level,
//! with which glyph.
//!
//! # Why there is no regular expression
//!
//! Two reasons, and neither is asceticism. It is a dependency in a crate the
//! footer links, and it is a footgun in a file the owner edits by hand: a regex
//! that matches too much is the kind of mistake that shows up as "a
//! notification I never saw", which is the hardest kind to notice. Three tests
//! — equals, contains, begins with — cover what a person actually writes, and
//! each of them is obviously right or obviously wrong at a glance.
//!
//! # Why the FIRST match wins
//!
//! Because the manager has to be able to say *which* rule caught something.
//! With every matching rule laying its verdict over the last, the answer to
//! "why is this quiet?" is a list, and the person has to simulate the whole
//! book in their head. One rule, one answer, one thing to edit.

use crate::presentation::{Presentation, Verdict};
use crate::{Notification, Urgency};

/// What a term looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Field {
    /// The name the sender gave itself.
    App,
    /// The short title.
    Summary,
    /// The long text.
    Body,
    /// One of the tags. It matches if ANY tag does.
    Tag,
    /// Which catalogue entry recognised it.
    Catalogue,
    /// The protocol's urgency (`low`, `normal`, `critical`).
    Urgency,
    /// Our level, `0..=100`.
    Level,
}

impl Field {
    /// The name the tables use.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Summary => "summary",
            Self::Body => "body",
            Self::Tag => "tag",
            Self::Catalogue => "catalogue",
            Self::Urgency => "urgency",
            Self::Level => "level",
        }
    }

    /// From the name in the table.
    #[must_use]
    pub fn by_id(s: &str) -> Option<Self> {
        Some(match s {
            "app" => Self::App,
            "summary" => Self::Summary,
            "body" => Self::Body,
            "tag" => Self::Tag,
            "catalogue" => Self::Catalogue,
            "urgency" => Self::Urgency,
            "level" => Self::Level,
            _ => return None,
        })
    }

    /// Every one of them, for the manager's list and for the generator.
    pub const ALL: [Self; 7] = [
        Self::App,
        Self::Summary,
        Self::Body,
        Self::Tag,
        Self::Catalogue,
        Self::Urgency,
        Self::Level,
    ];
}

/// How a field is compared.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Test {
    /// Exactly, ignoring case.
    Is(String),
    /// Somewhere inside, ignoring case.
    Contains(String),
    /// At the beginning, ignoring case.
    StartsWith(String),
    /// The level is at least this. Only [`Field::Level`] answers it.
    AtLeast(u8),
    /// The level is at most this. Only [`Field::Level`] answers it.
    AtMost(u8),
}

impl Test {
    /// The name the tables use for the kind of test.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Is(_) => "is",
            Self::Contains(_) => "contains",
            Self::StartsWith(_) => "starts-with",
            Self::AtLeast(_) => "at-least",
            Self::AtMost(_) => "at-most",
        }
    }

    /// What it is comparing against, as the manager draws it.
    #[must_use]
    pub fn against(&self) -> String {
        match self {
            Self::Is(s) | Self::Contains(s) | Self::StartsWith(s) => s.clone(),
            Self::AtLeast(n) | Self::AtMost(n) => n.to_string(),
        }
    }
}

/// One condition.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Term {
    /// What it looks at.
    pub field: Field,
    /// How it compares.
    pub test: Test,
}

impl Term {
    /// Does the notification satisfy it?
    ///
    /// `level` comes in from the outside rather than off the notification
    /// because the level is a DECISION, not data: by the time a rule is asked,
    /// the catalogue may already have raised it. A rule that tested a level
    /// nobody had computed yet would be testing the urgency in disguise.
    #[must_use]
    pub fn holds(&self, n: &Notification, p: &Presentation) -> bool {
        match self.field {
            Field::Level => match self.test {
                Test::AtLeast(v) => p.level >= v,
                Test::AtMost(v) => p.level <= v,
                // A text test against a number compares the digits, which is
                // the honest reading of `level is 90`.
                _ => self.text_holds(&p.level.to_string()),
            },
            Field::Urgency => self.text_holds(urgency_name(n.urgency)),
            Field::App => self.text_holds(&n.app),
            Field::Summary => self.text_holds(&n.summary),
            Field::Body => self.text_holds(&n.body),
            Field::Catalogue => self.text_holds(p.catalogue.as_deref().unwrap_or_default()),
            // ANY tag. A rule that had to name them all in order would be a
            // rule about the order the sender happened to use.
            Field::Tag => n.tags.iter().any(|t| self.text_holds(t)),
        }
    }

    /// The three text comparisons, all case-insensitive.
    ///
    /// Case-insensitive because the sender chooses it and nobody writing a
    /// filter thinks about it: `app is Niri` failing because the process says
    /// `niri` is a whole evening.
    fn text_holds(&self, value: &str) -> bool {
        let value = value.to_lowercase();
        match &self.test {
            Test::Is(s) => value == s.to_lowercase(),
            Test::Contains(s) => value.contains(&s.to_lowercase()),
            Test::StartsWith(s) => value.starts_with(&s.to_lowercase()),
            // A numeric test on a text field never holds. It is not an error at
            // this point — the generator already refused it, with the rule's id
            // in the message.
            Test::AtLeast(_) | Test::AtMost(_) => false,
        }
    }
}

/// The urgency's name, as a filter spells it.
#[must_use]
pub const fn urgency_name(u: Urgency) -> &'static str {
    match u {
        Urgency::Low => "low",
        Urgency::Normal => "normal",
        Urgency::Critical => "critical",
    }
}

/// A filter: conditions, and what to do when they all hold.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Rule {
    /// Stable id. The manager shows it, and an override is keyed by it.
    pub id: String,
    /// Every one of them has to hold. An empty list matches EVERYTHING, and
    /// that is deliberate: it is how one writes "and anything else".
    pub when: Vec<Term>,
    /// What it decides. `apply`, as `assets/notifications.nix` spells it.
    pub apply: Verdict,
}

impl Rule {
    /// Does it catch this notification?
    #[must_use]
    pub fn catches(&self, n: &Notification, p: &Presentation) -> bool {
        self.when.iter().all(|t| t.holds(n, p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::Show;

    fn n(app: &str, summary: &str, body: &str) -> Notification {
        let mut x = Notification::fresh_one(
            1,
            app.into(),
            None,
            summary.into(),
            body.into(),
            Urgency::Normal,
        );
        x.tags = vec!["screenshot".into(), "media".into()];
        x
    }

    fn term(field: Field, test: Test) -> Term {
        Term { field, test }
    }

    #[test]
    fn a_case_does_not_decide_a_filter() {
        // The sender chooses the case and nobody writing a filter thinks about
        // it. `app is Niri` failing because the process says `niri` is an
        // evening spent on nothing.
        let x = n("niri", "Screenshot captured", "");
        let p = Presentation::from_urgency(Urgency::Normal);
        assert!(term(Field::App, Test::Is("Niri".into())).holds(&x, &p));
        assert!(term(Field::Summary, Test::Contains("SCREENSHOT".into())).holds(&x, &p));
        assert!(term(Field::Summary, Test::StartsWith("screen".into())).holds(&x, &p));
    }

    #[test]
    fn every_term_has_to_hold() {
        let x = n("niri", "Screenshot captured", "");
        let p = Presentation::from_urgency(Urgency::Normal);
        let r = Rule {
            id: "shot".into(),
            when: vec![
                term(Field::App, Test::Is("niri".into())),
                term(Field::Summary, Test::Is("Screenshot captured".into())),
            ],
            apply: Verdict {
                show: Some(Show::Brief),
                ..Verdict::default()
            },
        };
        assert!(r.catches(&x, &p));
        // One term wrong is the whole rule wrong. `any` here would make every
        // two-term rule mean something the person did not write.
        let other = n("firefox", "Screenshot captured", "");
        assert!(!r.catches(&other, &p));
    }

    #[test]
    fn an_empty_rule_catches_everything() {
        // It is how one writes "and anything else", which every filter list
        // ends up wanting.
        let r = Rule {
            id: "rest".into(),
            when: vec![],
            apply: Verdict::default(),
        };
        let p = Presentation::from_urgency(Urgency::Low);
        assert!(r.catches(&n("a", "b", "c"), &p));
    }

    #[test]
    fn any_tag_answers_for_the_tag_field() {
        let x = n("nosso", "algo", "");
        let p = Presentation::from_urgency(Urgency::Normal);
        assert!(term(Field::Tag, Test::Is("media".into())).holds(&x, &p));
        assert!(!term(Field::Tag, Test::Is("rede".into())).holds(&x, &p));
    }

    #[test]
    fn the_level_is_compared_as_a_number() {
        let x = n("a", "b", "");
        let mut p = Presentation::from_urgency(Urgency::Normal);
        p.level = 50;
        assert!(term(Field::Level, Test::AtLeast(50)).holds(&x, &p));
        assert!(!term(Field::Level, Test::AtLeast(51)).holds(&x, &p));
        assert!(term(Field::Level, Test::AtMost(50)).holds(&x, &p));
        // And a numeric test on a TEXT field never holds — the generator
        // refuses it before it gets here, with the rule's id in the message.
        assert!(!term(Field::App, Test::AtLeast(1)).holds(&x, &p));
    }

    #[test]
    fn the_level_a_rule_sees_is_the_one_already_decided() {
        // Not the urgency in disguise: the catalogue may have raised it before
        // any rule was asked, and a rule that tested a level nobody had
        // computed would be testing something else.
        let x = n("a", "b", "");
        let mut p = Presentation::from_urgency(Urgency::Low);
        assert!(term(Field::Level, Test::AtMost(25)).holds(&x, &p));
        p.level = 90;
        assert!(term(Field::Level, Test::AtLeast(90)).holds(&x, &p));
    }

    #[test]
    fn the_names_go_and_come_back() {
        for f in Field::ALL {
            assert_eq!(Field::by_id(f.id()), Some(f));
        }
        assert_eq!(Field::by_id("no-such-field"), None);
    }
}
