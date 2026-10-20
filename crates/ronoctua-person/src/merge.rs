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
//!
//! # The declines meet machine by machine
//!
//! `declined.json` keeps the order and the ids, and each decline meets as a
//! SET of machines ([`Declining::merged`], the one rule of
//! `crate::declined`): what either side added holds, and what either side
//! took away holds. Two machines bringing a program back at once, or two
//! declining it at once, are two edits that both hold — never a clash.

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

/// `declined.json` three ways, each decline by its machines
/// ([`Declining::merged`]): it never clashes, and the list of clashes is
/// always empty — it is the shape every list of the person's merges into.
#[must_use]
pub fn merge_declined(
    base: &Declined,
    ours: &Declined,
    theirs: &Declined,
) -> (Declined, Vec<Clash<Declining>>) {
    let find = |list: &'_ [Declining], id: &str| list.iter().find(|d| d.id == id).cloned();
    let mut ids: Vec<&str> = ours.apps.iter().map(|d| d.id.as_str()).collect();
    for d in &theirs.apps {
        if !ids.contains(&d.id.as_str()) {
            ids.push(d.id.as_str());
        }
    }
    // An id only the base has was taken away on both sides: nothing to place.
    let apps = ids
        .into_iter()
        .filter_map(|id| {
            Declining::merged(
                find(&base.apps, id).as_ref(),
                find(&ours.apps, id).as_ref(),
                find(&theirs.apps, id).as_ref(),
            )
        })
        .collect();
    (
        Declined {
            format: crate::FORMAT,
            apps,
        },
        Vec::new(),
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

    fn hosts(names: &[&str]) -> Option<Vec<String>> {
        Some(names.iter().map(|&h| h.to_owned()).collect())
    }

    fn machines_of(doc: &Declined, id: &str) -> Option<Option<Vec<String>>> {
        doc.apps
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.machines.clone())
    }

    /// FAILS IF: two machines that each bring a program back at once leave
    /// a clash, or the other's decline behind — instead of both holding,
    /// which is the decline gone.
    #[test]
    fn two_bring_backs_at_once_both_hold() {
        let mut base = Declined::default();
        base.decline("libreoffice", hosts(&["alpha", "beta"]), &at(0), "alpha");
        base.decline("gimp", hosts(&["alpha", "beta", "gamma"]), &at(0), "alpha");
        let mut ours = base.clone();
        assert!(ours.undecline("libreoffice", "alpha", &at(5)));
        assert!(ours.undecline("gimp", "alpha", &at(5)));
        let mut theirs = base.clone();
        assert!(theirs.undecline("libreoffice", "beta", &at(6)));
        assert!(theirs.undecline("gimp", "beta", &at(6)));
        let (merged, clashes) = merge_declined(&base, &ours, &theirs);
        assert!(clashes.is_empty(), "{clashes:?}");
        assert_eq!(machines_of(&merged, "libreoffice"), None, "{merged:?}");
        assert_eq!(machines_of(&merged, "gimp"), Some(hosts(&["gamma"])));
        let gimp = merged.apps.iter().find(|d| d.id == "gimp").unwrap();
        assert_eq!(
            (gimp.edited_at.clone(), gimp.edited_on.as_str()),
            (at(6), "beta")
        );
    }

    /// FAILS IF: two machines that each decline a program at once — one by
    /// hand, one the installer's — clash or lose one of the two machines.
    #[test]
    fn two_declines_at_once_both_hold() {
        use crate::Document as _;
        let base = Declined::default();
        let mut ours = base.clone();
        ours.decline("libreoffice", hosts(&["alpha"]), &at(5), "alpha");
        let mut theirs = base.clone();
        theirs.decline("libreoffice", hosts(&["new-host"]), &at(6), "new-host");
        let (merged, clashes) = merge_declined(&base, &ours, &theirs);
        assert!(clashes.is_empty(), "{clashes:?}");
        assert_eq!(
            machines_of(&merged, "libreoffice"),
            Some(hosts(&["alpha", "new-host"]))
        );
        let one = &merged.apps[0];
        assert_eq!(
            (one.edited_at.clone(), one.edited_on.as_str()),
            (at(6), "new-host")
        );
        assert!(merged.validate().is_ok());
    }

    /// FAILS IF: a decline of every machine a side made does not absorb the
    /// other side's list, one the base had survives a side bringing it back
    /// whole, a side that changed nothing undoes the other's edit, or a
    /// decline only one side has is lost.
    #[test]
    fn every_machine_absorbs_and_one_sided_edits_hold() {
        let mut base = Declined::default();
        base.decline("vlc", hosts(&["alpha"]), &at(0), "alpha");
        base.decline("htop", None, &at(0), "alpha");
        base.decline("btop", hosts(&["alpha"]), &at(0), "alpha");
        let mut ours = base.clone();
        ours.decline("vlc", None, &at(5), "alpha");
        assert!(ours.undecline("htop", "alpha", &at(5)));
        ours.decline("mpv", hosts(&["alpha"]), &at(5), "alpha");
        let mut theirs = base.clone();
        theirs.undecline("vlc", "alpha", &at(6));
        theirs.decline("btop", hosts(&["beta"]), &at(6), "beta");
        let (merged, clashes) = merge_declined(&base, &ours, &theirs);
        assert!(clashes.is_empty(), "{clashes:?}");
        assert_eq!(
            machines_of(&merged, "vlc"),
            Some(None),
            "null did not absorb"
        );
        assert_eq!(
            machines_of(&merged, "htop"),
            None,
            "brought back whole, and back"
        );
        assert_eq!(
            machines_of(&merged, "btop"),
            Some(hosts(&["alpha", "beta"]))
        );
        assert_eq!(machines_of(&merged, "mpv"), Some(hosts(&["alpha"])));
        let ids: Vec<&str> = merged.apps.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["vlc", "btop", "mpv"], "our order, then theirs");
        // NOTHING CHANGED on either side: the base, stamps and all.
        assert_eq!(merge_declined(&base, &base, &base).0, base);
    }
}
