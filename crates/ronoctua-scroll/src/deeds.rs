//! The generated command table.
//!
//! What is in it comes from `assets/compositor-deeds.nix`; see
//! `nix/scripts/gen-deeds.py` for the three things that file becomes.

use std::sync::LazyLock;

use ronoctua_compositor::Deed;

// The generated file brings its own imports, so it is included AFTER ours.
include!(concat!(env!("OUT_DIR"), "/deeds.rs"));

/// Every deed, as the contract's vocabulary.
///
/// Derived from [`SPOKEN`] rather than emitted beside it: two lists that must
/// agree are two lists that will one day not.
pub static DEEDS: LazyLock<Vec<Deed>> = LazyLock::new(|| SPOKEN.iter().map(|s| s.deed).collect());

/// The deed with this id, and what it says.
#[must_use]
pub fn spoken_of(id: &str) -> Option<&'static Spoken> {
    SPOKEN.iter().find(|s| s.deed.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_arrived() {
        assert!(SPOKEN.len() > 40, "the generated table is there");
        assert_eq!(DEEDS.len(), SPOKEN.len(), "and the two views agree");
    }

    #[test]
    fn every_deed_is_found_by_its_id() {
        for s in SPOKEN {
            assert_eq!(spoken_of(s.deed.id).map(|f| f.text), Some(s.text));
        }
        assert!(spoken_of("wm-there-is-no-such-deed").is_none());
    }

    #[test]
    fn an_id_and_its_key_agree() {
        // The registry demands it, and the i18n check reads the key out of
        // the generated file — so a divergence here is a key nobody writes.
        for s in SPOKEN {
            assert_eq!(s.deed.key, format!("cmd-{}", s.deed.id), "{}", s.deed.id);
            assert!(s.deed.id.starts_with("wm-"), "{}", s.deed.id);
        }
    }

    #[test]
    fn a_placeholder_and_an_argument_come_in_pairs() {
        for s in SPOKEN {
            assert_eq!(
                s.text.contains("{n}"),
                s.deed.argument.is_some(),
                "{} says {:?}",
                s.deed.id,
                s.text
            );
        }
    }
}
