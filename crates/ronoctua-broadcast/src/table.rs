//! The categories of "channels by theme", and the numbers of the radio and TV
//! lists — generated from `assets/channels.nix` by `nix/broadcast.nix`, which
//! refuses an iptv-org category that does not exist.
//!
//! The TYPE is here and the VALUES are generated: the table is declared once,
//! in Nix, and read here as constants (`include!`), never rebuilt in Rust
//! (invariant 5).

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

    #[test]
    fn the_owners_examples_are_there() {
        // Action films were the owner's own example, and anime with them.
        for id in ["action", "anime", "films", "news", "music", "relax"] {
            assert!(Category::of(id).is_some(), "no `{id}`");
        }
        assert!(EXCLUDED.contains(&"xxx"));
    }
}
