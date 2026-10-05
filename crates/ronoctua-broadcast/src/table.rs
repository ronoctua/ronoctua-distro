//! The categories of "channels by theme", and the numbers of the radio and TV
//! lists — generated from `assets/channels.nix` by `nix/broadcast.nix`, which
//! refuses an iptv-org category that does not exist.
//!
//! And the catalogue's: every theme with its aliases ([`THEMES`]), the default
//! sources with the categories each asks for ([`DEFAULT_SOURCES`]), the words
//! that send a group to the adult store ([`ADULT_WORDS`]) and the bounds of an
//! ingest.
//!
//! The TYPE is here and the VALUES are generated: the table is declared once,
//! in Nix, and read here as constants (`include!`), never rebuilt in Rust
//! (invariant 5).

use crate::sources::Kind as SourceKind;

/// One category: where its programmes come from.
///
/// Any of the three may be empty — the generator refuses a category where all
/// three are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Category {
    /// Its kebab-case id.
    pub id: &'static str,
    /// The text key its name is drawn from, `player-channels-<id>`.
    pub key: &'static str,
    /// iptv-org categories: live TV channels.
    pub iptv: &'static [&'static str],
    /// Internet Archive collections, asked with `mediatype:(movies)`.
    pub archive: &'static [&'static str],
    /// Subjects that narrow those collections (all of them, when empty).
    pub subjects: &'static [&'static str],
    /// Words for YouTube's search with the Live filter.
    pub lives: &'static [&'static str],
}

impl Category {
    /// The category with this id.
    #[must_use]
    pub fn of(id: &str) -> Option<&'static Self> {
        CATEGORIES.iter().find(|c| c.id == id)
    }
}

/// One theme of the catalogue: a row of "channels by theme".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Its kebab-case id.
    pub id: &'static str,
    /// The text key its name is drawn from, `player-channels-<id>`.
    pub key: &'static str,
    /// The words that file a person's group under it
    /// ([`crate::sources::theme_of`]).
    pub aliases: &'static [&'static str],
}

impl Theme {
    /// The theme with this id.
    #[must_use]
    pub fn of(id: &str) -> Option<&'static Self> {
        THEMES.iter().find(|t| t.id == id)
    }
}

/// One category a default source asks for, under one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Native {
    /// Its key in the source's categories (unique in the source).
    pub key: &'static str,
    /// The theme it is filed under.
    pub theme: &'static str,
    /// What is asked: an iptv-org category, an archive collection, the words
    /// of a live or NASA search, a Commons category's name, a PeerTube
    /// category's number.
    pub ask: &'static str,
    /// The archive subjects that narrow a collection; empty for all of it.
    pub narrow: &'static [&'static str],
    /// An archive collection of sound (`mediatype:(audio)`).
    pub audio: bool,
}

/// One of the catalogue's default sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultSource {
    /// Its id in the catalogue.
    pub id: &'static str,
    /// What kind it is.
    pub kind: SourceKind,
    /// The text key its name is drawn from, `player-channels-source-<id>`.
    pub key: &'static str,
    /// Every host it asks: its consent row lets them all.
    pub hosts: &'static [&'static str],
    /// Where its pictures come from; empty is any site.
    pub poster_hosts: &'static [&'static str],
    /// Its posters are shown from the start: every poster host is one of its
    /// own `hosts`.
    pub posters_on: bool,
    /// How long an ingest of it stays fresh, in minutes.
    pub fresh_minutes: u64,
    /// The categories it asks for.
    pub natives: &'static [Native],
}

impl DefaultSource {
    /// The default source with this id.
    #[must_use]
    pub fn of(id: &str) -> Option<&'static Self> {
        DEFAULT_SOURCES.iter().find(|s| s.id == id)
    }
}

include!(concat!(env!("OUT_DIR"), "/broadcast.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_declared_category_has_a_source_and_its_own_key() {
        assert!(!CATEGORIES.is_empty());
        for c in CATEGORIES {
            assert!(
                !c.iptv.is_empty() || !c.archive.is_empty() || !c.lives.is_empty(),
                "{} has no source",
                c.id
            );
            assert_eq!(c.key, format!("player-channels-{}", c.id));
            assert_eq!(Category::of(c.id), Some(c));
            for i in c.iptv {
                assert!(!EXCLUDED.contains(i), "{} asks for an excluded {i}", c.id);
            }
        }
        let mut ids: Vec<&str> = CATEGORIES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATEGORIES.len(), "an id repeats");
    }

    /// FAILS IF: a theme or a default source's category stops having a name
    /// and a place, a poster switch says "on" for a host the person never
    /// let, or two themes fold one alias the same way (the generator
    /// compares them without taking accents off; this does).
    #[test]
    fn the_catalogue_s_table_holds_together() {
        assert!(THEMES.len() >= CATEGORIES.len());
        for c in CATEGORIES {
            assert!(
                Theme::of(c.id).is_some(),
                "the desk's `{}` is a theme",
                c.id
            );
        }
        for t in THEMES {
            assert_eq!(t.key, format!("player-channels-{}", t.id));
        }
        let mut claimed: Vec<(Vec<String>, &str)> = Vec::new();
        for t in THEMES {
            for a in t.aliases {
                let words = crate::catalog::words_of(a);
                assert!(!words.is_empty(), "`{a}` of `{}` has no words", t.id);
                if let Some((_, other)) = claimed.iter().find(|(w, _)| *w == words) {
                    assert_eq!(*other, t.id, "`{a}` folds like an alias of `{other}`");
                }
                claimed.push((words, t.id));
            }
        }
        for s in DEFAULT_SOURCES {
            assert_eq!(s.key, format!("player-channels-source-{}", s.id));
            assert!(s.kind.is_default(), "{}", s.id);
            assert!(!s.hosts.is_empty() && !s.natives.is_empty(), "{}", s.id);
            let own =
                !s.poster_hosts.is_empty() && s.poster_hosts.iter().all(|h| s.hosts.contains(h));
            assert_eq!(
                s.posters_on, own,
                "{}: posters on only from its own hosts",
                s.id
            );
            for n in s.natives {
                assert!(Theme::of(n.theme).is_some(), "{}: `{}`", s.id, n.theme);
            }
            assert_eq!(DefaultSource::of(s.id), Some(s));
        }
        // THE KINDS ARE ONE EACH: an ingest is per kind.
        for kind in crate::sources::Kind::ALL.iter().filter(|k| k.is_default()) {
            assert_eq!(
                DEFAULT_SOURCES.iter().filter(|s| s.kind == *kind).count(),
                1,
                "{}",
                kind.id()
            );
        }
        assert!(ADULT_WORDS.contains(&"xxx"));
        const { assert!(SOURCE_ITEMS_MOST >= NATIVE_ITEMS_MOST) };
    }

    #[test]
    fn nothing_adult_is_asked_of_a_default_source() {
        for s in DEFAULT_SOURCES {
            for n in s.natives {
                assert!(
                    !crate::sources::is_adult(n.ask) && !crate::sources::is_adult(n.key),
                    "{}: `{}`",
                    s.id,
                    n.ask
                );
            }
        }
    }

    #[test]
    fn the_owners_examples_are_there() {
        // Action films were the owner's own example, and anime with them.
        for id in ["action", "anime", "films", "news", "music", "relax"] {
            assert!(Category::of(id).is_some(), "no `{id}`");
        }
        assert!(EXCLUDED.contains(&"xxx"));
    }
}
