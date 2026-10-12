//! Two copies of a list meet by `id` — the three-way merge every list of the
//! person's data obeys (the software centre's staging, the belongings' plain
//! part, the installer's first pull).
//!
//! # The rules, per id (base = what both copies came from)
//!
//! | base | ours | theirs | result |
//! |---|---|---|---|
//! | – | x | – | x (added here) |
//! | – | – | y | y (added there) |
//! | – | x | y | x if equal; the LATER edit if they differ only in stamps and pin; else a [`Clash`] |
//! | b | b | y | y (only theirs changed; `None` = deleted there) |
//! | b | x | b | x (only ours changed; `None` = deleted here) |
//! | b | x | – | deleted when x is b or differs from b only in stamps and pin — **a deletion beats a pin-only change**; else a [`Clash`] |
//! | b | – | y | the same, mirrored |
//! | b | x | y | x if equal; the LATER edit if x and y differ only in stamps and pin; else a [`Clash`] |
//! | b | – | – | gone |
//!
//! "Later" is `edited-at`; a tie keeps ours. A clash never blocks: the
//! merged list keeps OUR side of it (the local file stays as the person left
//! it) and the clash is reported for a decision.
//!
//! # Order
//!
//! Our order first; what only theirs has is appended in their order.

use crate::Stamp;
use crate::declared::{Declared, Entry};
use crate::declined::{Declined, Declining};

/// An element of a list merged by id.
pub trait Keyed: Clone + PartialEq {
    /// Its id.
    fn id(&self) -> &str;
    /// When it was last edited.
    fn edited_at(&self) -> &Stamp;
    /// Whether `self` and `other` differ only in what a later edit may settle
    /// alone: the two stamps and, for an entry with one, the pin.
    fn minor_change(&self, other: &Self) -> bool;
}

/// What a merge came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged<T> {
    /// The merged list (our side of every clash).
    pub items: Vec<T>,
    /// What needs a decision.
    pub clashes: Vec<Clash<T>>,
}

/// One id both sides changed differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash<T> {
    /// The id.
    pub id: String,
    /// What both came from (`None`: both added it).
    pub base: Option<T>,
    /// Ours (`None`: deleted here).
    pub ours: Option<T>,
    /// Theirs (`None`: deleted there).
    pub theirs: Option<T>,
}

fn find<'a, T: Keyed>(list: &'a [T], id: &str) -> Option<&'a T> {
    list.iter().find(|t| t.id() == id)
}

/// What one id becomes, or the clash.
enum Fate<T> {
    Keep(T),
    Gone,
    Clash(Clash<T>),
}

fn fate<T: Keyed>(id: &str, base: Option<&T>, ours: Option<&T>, theirs: Option<&T>) -> Fate<T> {
    let clash = || {
        Fate::Clash(Clash {
            id: id.to_owned(),
            base: base.cloned(),
            ours: ours.cloned(),
            theirs: theirs.cloned(),
        })
    };
    let later = |o: &T, t: &T| {
        if t.edited_at() > o.edited_at() {
            Fate::Keep(t.clone())
        } else {
            Fate::Keep(o.clone())
        }
    };
    match (base, ours, theirs) {
        (_, None, None) => Fate::Gone,
        (None, Some(o), None) => Fate::Keep(o.clone()),
        (None, None, Some(t)) => Fate::Keep(t.clone()),
        (None, Some(o), Some(t)) => {
            if o == t {
                Fate::Keep(o.clone())
            } else if o.minor_change(t) {
                later(o, t)
            } else {
                clash()
            }
        }
        (Some(b), Some(o), None) => {
            if o == b || b.minor_change(o) {
                Fate::Gone
            } else {
                clash()
            }
        }
        (Some(b), None, Some(t)) => {
            if t == b || b.minor_change(t) {
                Fate::Gone
            } else {
                clash()
            }
        }
        (Some(b), Some(o), Some(t)) => {
            if o == t || t == b {
                Fate::Keep(o.clone())
            } else if o == b {
                Fate::Keep(t.clone())
            } else if o.minor_change(t) {
                later(o, t)
            } else {
                clash()
            }
        }
    }
}

/// The three-way merge by id.
#[must_use]
pub fn merge_by_id<T: Keyed>(base: &[T], ours: &[T], theirs: &[T]) -> Merged<T> {
    let mut ids: Vec<&str> = ours.iter().map(Keyed::id).collect();
    for t in theirs {
        if !ids.contains(&t.id()) {
            ids.push(t.id());
        }
    }
    // An id only the base has was deleted on both sides: nothing to place.
    let mut items = Vec::new();
    let mut clashes = Vec::new();
    for id in ids {
        match fate(id, find(base, id), find(ours, id), find(theirs, id)) {
            Fate::Keep(t) => items.push(t),
            Fate::Gone => {}
            Fate::Clash(c) => {
                if let Some(o) = &c.ours {
                    items.push(o.clone());
                }
                clashes.push(c);
            }
        }
    }
    Merged { items, clashes }
}

impl Keyed for Entry {
    fn id(&self) -> &str {
        Self::id(self)
    }

    fn edited_at(&self) -> &Stamp {
        Self::edited_at(self)
    }

    fn minor_change(&self, other: &Self) -> bool {
        self.settled() == other.settled()
    }
}

impl Keyed for Declining {
    fn id(&self) -> &str {
        &self.id
    }

    fn edited_at(&self) -> &Stamp {
        &self.edited_at
    }

    fn minor_change(&self, other: &Self) -> bool {
        self.id == other.id && self.machines == other.machines
    }
}

/// `declared.json` three ways.
#[must_use]
pub fn merge_declared(
    base: &Declared,
    ours: &Declared,
    theirs: &Declared,
) -> (Declared, Vec<Clash<Entry>>) {
    let m = merge_by_id(&base.apps, &ours.apps, &theirs.apps);
    (
        Declared {
            format: crate::FORMAT,
            apps: m.items,
        },
        m.clashes,
    )
}

/// `declined.json` three ways.
#[must_use]
pub fn merge_declined(
    base: &Declined,
    ours: &Declined,
    theirs: &Declined,
) -> (Declined, Vec<Clash<Declining>>) {
    let m = merge_by_id(&base.apps, &ours.apps, &theirs.apps);
    (
        Declined {
            format: crate::FORMAT,
            apps: m.items,
        },
        m.clashes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declared::{AppImageApp, NixApp, Pin};

    fn at(s: i64) -> Stamp {
        Stamp::from_unix_s(1_791_201_600 + s)
    }

    fn nix(attr: &str, machines: Option<&[&str]>, edited: i64) -> Entry {
        Entry::Nix(NixApp {
            id: format!("nix:{attr}"),
            machines: machines.map(|m| m.iter().map(|&h| h.to_owned()).collect()),
            edited_at: at(edited),
            edited_on: "noctua".to_owned(),
            attr: attr.to_owned(),
            unfree: false,
        })
    }

    fn appimage(version: &str, edited: i64) -> Entry {
        Entry::AppImage(AppImageApp {
            id: "appimage:obsidian".to_owned(),
            machines: None,
            edited_at: at(edited),
            edited_on: "noctua".to_owned(),
            key: "obsidian".to_owned(),
            name: "Obsidian".to_owned(),
            pin: Pin {
                url: format!("https://example.org/Obsidian-{version}.AppImage"),
                hash: "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=".to_owned(),
                version: version.to_owned(),
                pinned_at: at(edited),
            },
            follows: None,
            icon: None,
        })
    }

    fn ids(items: &[Entry]) -> Vec<&str> {
        items.iter().map(Entry::id).collect()
    }

    #[test]
    fn one_side_changes_are_taken_and_order_is_ours_then_theirs() {
        let base = [nix("a", None, 0), nix("b", None, 0)];
        let ours = [
            nix("b", None, 0),
            nix("a", Some(&["noctua"]), 5),
            nix("c", None, 5),
        ];
        let theirs = [nix("a", None, 0), nix("d", None, 7)];
        let m = merge_by_id(&base, &ours, &theirs);
        assert!(m.clashes.is_empty());
        // `b` deleted there and untouched here: gone.
        assert_eq!(ids(&m.items), ["nix:a", "nix:c", "nix:d"]);
        assert_eq!(m.items[0].machines(), Some(&["noctua".to_owned()][..]));
    }

    #[test]
    fn a_pin_only_difference_goes_to_the_later_edit() {
        let base = [appimage("1.0", 0)];
        let m = merge_by_id(&base, &[appimage("1.1", 10)], &[appimage("1.2", 20)]);
        assert!(m.clashes.is_empty());
        assert_eq!(m.items, [appimage("1.2", 20)]);
        let tie = merge_by_id(&base, &[appimage("1.1", 10)], &[appimage("1.2", 10)]);
        assert_eq!(tie.items, [appimage("1.1", 10)], "a tie keeps ours");
    }

    #[test]
    fn a_deletion_beats_a_pin_only_change_and_nothing_else() {
        let base = [appimage("1.0", 0)];
        let m = merge_by_id(&base, &[appimage("1.1", 10)], &[]);
        assert!(m.items.is_empty() && m.clashes.is_empty());
        let m = merge_by_id(&base, &[], &[appimage("1.1", 10)]);
        assert!(m.items.is_empty() && m.clashes.is_empty());

        // A REAL change against a deletion is a clash, and ours stays.
        let base = [nix("a", None, 0)];
        let ours = [nix("a", Some(&["noctua"]), 5)];
        let m = merge_by_id(&base, &ours, &[]);
        assert_eq!(m.items, ours);
        assert_eq!(m.clashes.len(), 1);
        assert_eq!(m.clashes[0].theirs, None);
    }

    #[test]
    fn real_changes_on_both_sides_clash_and_never_block() {
        let base = [nix("a", None, 0)];
        let ours = [nix("a", Some(&["noctua"]), 5)];
        let theirs = [nix("a", Some(&["vm"]), 9)];
        let m = merge_by_id(&base, &ours, &theirs);
        assert_eq!(m.items, ours, "the local side stays");
        assert_eq!(
            m.clashes,
            [Clash {
                id: "nix:a".to_owned(),
                base: Some(base[0].clone()),
                ours: Some(ours[0].clone()),
                theirs: Some(theirs[0].clone()),
            }]
        );
    }
}
