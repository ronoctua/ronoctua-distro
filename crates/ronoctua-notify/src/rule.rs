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

    /// Every kind of comparison, in the order the manager cycles them.
    ///
    /// The kinds and not the values: a `Test` carries what it compares
    /// against, and turning "contains" into "at least" has to keep it.
    pub const KINDS: [&'static str; 5] = ["is", "contains", "starts-with", "at-least", "at-most"];

    /// The same comparison of another kind, keeping what it compares against.
    ///
    /// Text becomes a number by reading it, and a number becomes text by
    /// writing it — so turning the knob past the end of the text tests and back
    /// does not silently empty the value the person typed.
    #[must_use]
    pub fn of_kind(&self, kind: &str) -> Self {
        let text = self.against();
        let number = text.trim().parse::<u8>().unwrap_or_default();
        match kind {
            "contains" => Self::Contains(text),
            "starts-with" => Self::StartsWith(text),
            "at-least" => Self::AtLeast(number),
            "at-most" => Self::AtMost(number),
            _ => Self::Is(text),
        }
    }

    /// Does it compare against a NUMBER?
    #[must_use]
    pub const fn is_numeric(&self) -> bool {
        matches!(self, Self::AtLeast(_) | Self::AtMost(_))
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
    /// filter thinks about it: `app is Emacs` failing because the process says
    /// `emacs` is a whole evening.
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

/// Can this filter's name be a name in the REPOSITORY?
///
/// A filter on one machine is called whatever its author typed — it is data.
/// One that goes to everybody becomes a line of the distro, and invariant 18
/// says every name in this repository is English kebab: it is the key an
/// adjustment is stored under, and the id the build refuses if it is malformed.
///
/// So the door checks it and says so, rather than mangling `silenciar cópias`
/// into `silenciar-cpias` behind somebody's back.
#[must_use]
pub fn is_shareable_name(id: &str) -> bool {
    !id.is_empty()
        && id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A filter in the shape the BUILD reads — which is the shape a person reads.
///
/// # Why not serde's own
///
/// Because this goes into the repository, beside `assets/notifications.nix`,
/// and somebody will open it. Serde spells a `Test` as `{"Contains": "backup"}`
/// — the variant's Rust name — and a `Field` as `"Summary"`. The table spells
/// the same condition `{"field": "summary", "contains": "backup"}`, which is
/// what the Nix file says and what `gen-notifications.py` already validates.
/// One shape for the build to read, and it is the readable one.
#[must_use]
pub fn as_table(rule: &Rule) -> serde_json::Value {
    let when: Vec<serde_json::Value> = rule
        .when
        .iter()
        .map(|t| {
            let value = match &t.test {
                Test::Is(s) | Test::Contains(s) | Test::StartsWith(s) => {
                    serde_json::Value::String(s.clone())
                }
                Test::AtLeast(n) | Test::AtMost(n) => serde_json::Value::from(*n),
            };
            serde_json::json!({ "field": t.field.id(), t.test.id(): value })
        })
        .collect();
    let mut apply = serde_json::Map::new();
    let v = &rule.apply;
    if let Some(x) = v.show {
        apply.insert("show".into(), x.id().into());
    }
    if let Some(x) = v.life {
        apply.insert("life".into(), x.id().into());
    }
    if let Some(x) = v.level {
        apply.insert("level".into(), x.into());
    }
    for (name, held) in [
        ("loud", v.loud),
        ("blink", v.blink),
        ("vibrate", v.vibrate),
        ("audio", v.audio),
        ("auto-read", v.auto_read),
    ] {
        if let Some(x) = held {
            apply.insert(name.into(), x.into());
        }
    }
    serde_json::json!({ "id": rule.id, "when": when, "apply": apply })
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
        // it. `app is Emacs` failing because the process says `emacs` is an
        // evening spent on nothing.
        let x = n("emacs", "Compilation finished", "");
        let p = Presentation::from_urgency(Urgency::Normal);
        assert!(term(Field::App, Test::Is("Emacs".into())).holds(&x, &p));
        assert!(term(Field::Summary, Test::Contains("COMPILATION".into())).holds(&x, &p));
        assert!(term(Field::Summary, Test::StartsWith("compil".into())).holds(&x, &p));
    }

    #[test]
    fn every_term_has_to_hold() {
        let x = n("emacs", "Compilation finished", "");
        let p = Presentation::from_urgency(Urgency::Normal);
        let r = Rule {
            id: "shot".into(),
            when: vec![
                term(Field::App, Test::Is("emacs".into())),
                term(Field::Summary, Test::Is("Compilation finished".into())),
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
        let x = n("ours", "something", "");
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
