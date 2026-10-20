//! The merge engine for JSON (feature `crypt`) — pure: three versions of a
//! document in, one out, and the places where both sides changed the same
//! thing differently.
//!
//! # The rules
//!
//! | base → ours | base → theirs | result |
//! |---|---|---|
//! | same | changed | theirs |
//! | changed | same | ours |
//! | changed | changed, alike | either |
//! | changed | changed, differently | joined below, or a CLASH |
//!
//! Objects are joined KEY BY KEY (our keys in our order, then the keys only
//! theirs has, in theirs'). Arrays whose elements all carry an `id` are
//! joined ELEMENT BY ID (ours first, then theirs' new ones) — and the
//! pointer of such an element names its id (`/apps/nix:firefox`), never its
//! index, so it stays when the list moves. Any other array is ONE value. A
//! clash keeps OUR side unless a pick says otherwise ([`Pick`]): nothing is
//! ever guessed, and the caller decides whether a clash is a conflict.
//!
//! # The person's software lists
//!
//! `state/person/software/declared.json` and `declined.json` obey the merge
//! by id of `ronoctua_person::merge` instead: two edits that differ only in
//! their stamps and their pin are settled by the LATER `edited-at`, and a
//! deletion beats a pin-only change — two machines refreshing a pin on the
//! same day never ask the person anything. A clash there is the whole entry
//! (`/apps/<id>`). A file that does not read as that document (a newer
//! format, a broken one) is joined by the general rules.
//!
//! `software/distro-pins.json` — the distro's AppImages the person follows
//! past its pins, written by the update alone — never clashes: the later
//! pin of an id wins, and a removal (the distro caught up) beats a pin that
//! moved (`ronoctua_person::merge::merge_distro_pins`).
//!
//! # VS Code's list of extensions
//!
//! `home/.config/Code/User/ronoctua-extensions.json` is a SET of ids
//! (`ronoctua_vscode::merge`): the base, plus what either side added, minus
//! what either side took out — so an uninstall on one machine and an install
//! on the other both hold, and nobody is asked.
//!
//! # What it is not
//!
//! It never reads a file nor writes one: the engine hands the bytes in
//! (`engine/merging.rs`, the base from the sealed cache or from git).

use ronoctua_person::declared::Declared;
use ronoctua_person::declined::Declined;
use ronoctua_person::distro_pins::DistroPins;
use ronoctua_store::json::{Node, pointer_of};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::plan::FieldConflict;

// ── anchor: L09 ──
/// Which side a place both changed takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// This machine's.
    Ours,
    /// The repository's.
    Theirs,
}

/// A place both sides changed differently: its pointer and the three values
/// (`None`: absent on that side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// Where.
    pub pointer: String,
    /// What both came from.
    pub base: Option<Node>,
    /// This machine's.
    pub ours: Option<Node>,
    /// The repository's.
    pub theirs: Option<Node>,
}

impl Clash {
    /// The clash as a plan writes it: the values as compact JSON.
    #[must_use]
    pub fn conflict(&self) -> FieldConflict {
        let text = |n: &Option<Node>| n.as_ref().map(ToString::to_string);
        FieldConflict {
            pointer: self.pointer.clone(),
            base: text(&self.base),
            ours: text(&self.ours),
            theirs: text(&self.theirs),
        }
    }
}

/// What a merge of three documents came to: the joined bytes (each clash
/// taking the side its pick said, ours when none did) and the clashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    /// The document.
    pub bytes: Vec<u8>,
    /// Where both changed differently.
    pub clashes: Vec<Clash>,
}

/// Which of the person's lists a logical path is, if one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    /// `software/declared.json`.
    Declared,
    /// `software/declined.json`.
    Declined,
    /// `software/distro-pins.json`.
    DistroPins,
    /// VS Code's list of the person's extensions, a set
    /// (`ronoctua_vscode::merge`) — the `vscode` target's.
    Extensions,
}

/// The person's list `logical` is (`state/person/software/declared.json`…).
/// Pure.
#[must_use]
pub fn list_of(logical: &str) -> Option<List> {
    use ronoctua_person::names::{DECLARED, DECLINED, DISTRO_PINS, MIRROR};
    if logical == crate::offers::EXTENSIONS_LIST {
        return Some(List::Extensions);
    }
    let rest = logical.strip_prefix("state/")?.strip_prefix(MIRROR)?;
    match rest.strip_prefix('/')? {
        r if r == DECLARED => Some(List::Declared),
        r if r == DECLINED => Some(List::Declined),
        r if r == DISTRO_PINS => Some(List::DistroPins),
        _ => None,
    }
}

/// Whether `logical` is a JSON document by its name (`.json`). Pure.
#[must_use]
pub fn is_json_name(logical: &str) -> bool {
    std::path::Path::new(logical)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// The three versions of `logical` merged, each clash taking `pick` of its
/// pointer (ours when it says nothing). `None` when the name is not a JSON
/// document's or one of the three does not parse — the caller then merges it
/// as text, or not at all. Pure.
#[must_use]
pub fn merge(
    logical: &str,
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
    pick: &dyn Fn(&str) -> Option<Pick>,
) -> Option<Joined> {
    if !is_json_name(logical) {
        return None;
    }
    if let Some(list) = list_of(logical) {
        let joined = match list {
            List::Declared => person::<Declared>(base, ours, theirs, pick),
            List::Declined => person::<Declined>(base, ours, theirs, pick),
            List::DistroPins => person::<DistroPins>(base, ours, theirs, pick),
            // A SET, joined as one: what either side added or took out,
            // never a clash.
            List::Extensions => ronoctua_vscode::merge(base, ours, theirs).map(|bytes| Joined {
                bytes,
                clashes: Vec::new(),
            }),
        };
        if joined.is_some() {
            return joined;
        }
    }
    let (b, o, t) = (
        Node::parse(base).ok()?,
        Node::parse(ours).ok()?,
        Node::parse(theirs).ok()?,
    );
    let (node, clashes) = join(&b, &o, &t, pick);
    let bytes = if node == o && node.to_string() == o.to_string() {
        ours.to_vec()
    } else if node == t && node.to_string() == t.to_string() {
        theirs.to_vec()
    } else {
        let mut out = node.pretty();
        if ours.ends_with(b"\n") {
            out.push(b'\n');
        }
        out
    };
    Some(Joined { bytes, clashes })
}

/// Three documents joined by the rules at the top: the result (each clash
/// taking its pick, ours by default) and the clashes. Pure.
#[must_use]
pub fn join(
    base: &Node,
    ours: &Node,
    theirs: &Node,
    pick: &dyn Fn(&str) -> Option<Pick>,
) -> (Node, Vec<Clash>) {
    let mut clashes = Vec::new();
    let mut at = Vec::new();
    let node = three(
        &mut at,
        Some(base),
        Some(ours),
        Some(theirs),
        pick,
        &mut clashes,
    )
    // THE WHOLE DOCUMENT, decided theirs and absent there: never — a file
    // is not deleted by a merge of its content.
    .unwrap_or_else(|| ours.clone());
    (node, clashes)
}

/// One place, three ways: `None` is absent.
fn three(
    at: &mut Vec<String>,
    base: Option<&Node>,
    ours: Option<&Node>,
    theirs: Option<&Node>,
    pick: &dyn Fn(&str) -> Option<Pick>,
    clashes: &mut Vec<Clash>,
) -> Option<Node> {
    if ours == theirs || base == theirs {
        return ours.cloned();
    }
    if base == ours {
        return theirs.cloned();
    }
    match (base, ours, theirs) {
        (None | Some(Node::Object(_)), Some(Node::Object(o)), Some(Node::Object(t))) => {
            let b = match base {
                Some(Node::Object(b)) => Some(b.as_slice()),
                _ => None,
            };
            let mut keys: Vec<&str> = o.iter().map(|(k, _)| k.as_str()).collect();
            for (k, _) in t {
                if !keys.contains(&k.as_str()) {
                    keys.push(k);
                }
            }
            let mut out = Vec::with_capacity(keys.len());
            for k in keys {
                at.push(k.to_owned());
                let joined = three(
                    at,
                    b.and_then(|b| field(b, k)),
                    field(o, k),
                    field(t, k),
                    pick,
                    clashes,
                );
                at.pop();
                if let Some(v) = joined {
                    out.push((k.to_owned(), v));
                }
            }
            Some(Node::Object(out))
        }
        (None | Some(Node::Array(_)), Some(Node::Array(o)), Some(Node::Array(t)))
            if by_id(base, o, t) =>
        {
            let b: &[Node] = match base {
                Some(Node::Array(b)) => b,
                _ => &[],
            };
            let mut ids: Vec<String> = o.iter().filter_map(Node::id).collect();
            for id in t.iter().filter_map(Node::id) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                at.push(id.clone());
                let joined = three(
                    at,
                    element(b, &id),
                    element(o, &id),
                    element(t, &id),
                    pick,
                    clashes,
                );
                at.pop();
                if let Some(v) = joined {
                    out.push(v);
                }
            }
            Some(Node::Array(out))
        }
        _ => {
            let pointer = pointer_of(at);
            let side = pick(&pointer).unwrap_or(Pick::Ours);
            clashes.push(Clash {
                pointer,
                base: base.cloned(),
                ours: ours.cloned(),
                theirs: theirs.cloned(),
            });
            match side {
                Pick::Ours => ours.cloned(),
                Pick::Theirs => theirs.cloned(),
            }
        }
    }
}

/// An object's field.
fn field<'n>(pairs: &'n [(String, Node)], key: &str) -> Option<&'n Node> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// An array's element by its id.
fn element<'n>(items: &'n [Node], id: &str) -> Option<&'n Node> {
    items.iter().find(|n| n.id().as_deref() == Some(id))
}

/// Whether the three arrays are lists BY ID: something in them, and every
/// element of each carries an `id` no other element of the same array has.
fn by_id(base: Option<&Node>, ours: &[Node], theirs: &[Node]) -> bool {
    let keyed = |items: &[Node]| {
        let ids: Vec<String> = items.iter().filter_map(Node::id).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        ids.len() == items.len() && unique.len() == ids.len()
    };
    let base_keyed = match base {
        Some(Node::Array(b)) => keyed(b),
        _ => true,
    };
    (!ours.is_empty() || !theirs.is_empty()) && keyed(ours) && keyed(theirs) && base_keyed
}

/// The person's lists, by `ronoctua_person::merge` — `None` when one of
/// the three does not read as `D`.
fn person<D>(
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
    pick: &dyn Fn(&str) -> Option<Pick>,
) -> Option<Joined>
where
    D: PersonList,
{
    let b: D = ronoctua_person::parse(base).ok()?;
    let o: D = ronoctua_person::parse(ours).ok()?;
    let t: D = ronoctua_person::parse(theirs).ok()?;
    let (mut merged, clashes) = D::merged(&b, &o, &t);
    let mut out = Vec::with_capacity(clashes.len());
    for clash in clashes {
        let pointer = pointer_of(&["apps", clash.id.as_str()]);
        if pick(&pointer) == Some(Pick::Theirs) {
            merged.put(&clash.id, clash.theirs.clone());
        }
        let node = |e: &Option<D::Item>| e.as_ref().and_then(|e| Node::of(e).ok());
        out.push(Clash {
            pointer,
            base: node(&clash.base),
            ours: node(&clash.ours),
            theirs: node(&clash.theirs),
        });
    }
    let text = ronoctua_person::to_json(&merged);
    let bytes = if text.as_bytes() == ours {
        ours.to_vec()
    } else {
        text.into_bytes()
    };
    Some(Joined {
        bytes,
        clashes: out,
    })
}

/// One of the person's lists, as the merge by id sees it.
trait PersonList: ronoctua_person::Document + Serialize + DeserializeOwned {
    /// What an element is.
    type Item: Serialize + Clone;

    /// Three ways, by `ronoctua_person::merge`.
    fn merged(
        base: &Self,
        ours: &Self,
        theirs: &Self,
    ) -> (Self, Vec<ronoctua_person::merge::Clash<Self::Item>>)
    where
        Self: Sized;

    /// The element `id` set to `item` (`None`: removed); one it did not
    /// have is appended.
    fn put(&mut self, id: &str, item: Option<Self::Item>);
}

impl PersonList for Declared {
    type Item = ronoctua_person::declared::Entry;

    fn merged(
        base: &Self,
        ours: &Self,
        theirs: &Self,
    ) -> (Self, Vec<ronoctua_person::merge::Clash<Self::Item>>) {
        ronoctua_person::merge::merge_declared(base, ours, theirs)
    }

    fn put(&mut self, id: &str, item: Option<Self::Item>) {
        put_by(&mut self.apps, id, item, |e| e.id());
    }
}

impl PersonList for Declined {
    type Item = ronoctua_person::declined::Declining;

    fn merged(
        base: &Self,
        ours: &Self,
        theirs: &Self,
    ) -> (Self, Vec<ronoctua_person::merge::Clash<Self::Item>>) {
        ronoctua_person::merge::merge_declined(base, ours, theirs)
    }

    fn put(&mut self, id: &str, item: Option<Self::Item>) {
        put_by(&mut self.apps, id, item, |e| e.id.as_str());
    }
}

impl PersonList for DistroPins {
    type Item = ronoctua_person::distro_pins::FollowedPin;

    fn merged(
        base: &Self,
        ours: &Self,
        theirs: &Self,
    ) -> (Self, Vec<ronoctua_person::merge::Clash<Self::Item>>) {
        ronoctua_person::merge::merge_distro_pins(base, ours, theirs)
    }

    fn put(&mut self, id: &str, item: Option<Self::Item>) {
        put_by(&mut self.pins, id, item, |p| p.id.as_str());
    }
}

/// Sets the element `id` of `items` to `item`, removes it (`None`), or
/// appends it.
fn put_by<T>(items: &mut Vec<T>, id: &str, item: Option<T>, id_of: impl Fn(&T) -> &str) {
    let at = items.iter().position(|e| id_of(e) == id);
    match (at, item) {
        (Some(i), Some(item)) => items[i] = item,
        (Some(i), None) => {
            items.remove(i);
        }
        (None, Some(item)) => items.push(item),
        (None, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<Pick> {
        None
    }

    fn text(joined: &Joined) -> String {
        String::from_utf8(joined.bytes.clone()).unwrap()
    }

    /// FAILS IF: two machines that changed DIFFERENT fields of one document
    /// are asked anything, a list of elements with ids is merged by position,
    /// a field both changed differently is not a clash naming its place, or
    /// a pick does not take the side it names.
    #[test]
    fn belongings_json_merges_by_field() {
        let base = br#"{"theme": "owl", "size": 12, "screens": [{"id": "a", "scale": 1}, {"id": "b", "scale": 1}], "tags": ["x"]}"#;
        let ours = br#"{"theme": "paper", "size": 12, "screens": [{"id": "b", "scale": 2}, {"id": "a", "scale": 1}], "tags": ["x"], "mine": true}"#;
        let theirs = br#"{"theme": "owl", "size": 14, "screens": [{"id": "a", "scale": 1}, {"id": "b", "scale": 1}, {"id": "c", "scale": 3}], "tags": ["x"]}"#;
        let joined = merge("config/display.json", base, ours, theirs, &none).unwrap();
        assert!(joined.clashes.is_empty(), "{:?}", joined.clashes);
        let node = Node::parse(&joined.bytes).unwrap();
        assert_eq!(node.at("/theme"), Some(&Node::String("paper".into())));
        assert_eq!(node.at("/size").unwrap().to_string(), "14");
        assert_eq!(node.at("/mine"), Some(&Node::Bool(true)));
        // BY ID, ours' order first, theirs' new one appended.
        let ids: Vec<String> = match node.at("/screens").unwrap() {
            Node::Array(items) => items.iter().filter_map(Node::id).collect(),
            other => panic!("{other:?}"),
        };
        assert_eq!(ids, ["b", "a", "c"]);
        assert_eq!(node.at("/screens/0/scale").unwrap().to_string(), "2");
        // OUR KEY ORDER is kept.
        let order: Vec<&str> = match &node {
            Node::Object(pairs) => pairs.iter().map(|(k, _)| k.as_str()).collect(),
            _ => unreachable!(),
        };
        assert_eq!(order, ["theme", "size", "screens", "tags", "mine"]);

        // BOTH CHANGED ONE FIELD, AND ONE ELEMENT, DIFFERENTLY.
        let theirs = br#"{"theme": "cyber", "size": 12, "screens": [{"id": "a", "scale": 1}, {"id": "b", "scale": 5}], "tags": ["y"]}"#;
        let ours = br#"{"theme": "paper", "size": 12, "screens": [{"id": "a", "scale": 1}, {"id": "b", "scale": 2}], "tags": ["z"]}"#;
        let joined = merge("config/display.json", base, ours, theirs, &none).unwrap();
        let pointers: Vec<&str> = joined.clashes.iter().map(|c| c.pointer.as_str()).collect();
        assert_eq!(pointers, ["/theme", "/screens/b/scale", "/tags"]);
        let theme = joined.clashes[0].conflict();
        assert_eq!(theme.base.as_deref(), Some("\"owl\""));
        assert_eq!(theme.ours.as_deref(), Some("\"paper\""));
        assert_eq!(theme.theirs.as_deref(), Some("\"cyber\""));
        // NOTHING GUESSED: each clash keeps ours until it is picked.
        let node = Node::parse(&joined.bytes).unwrap();
        assert_eq!(node.at("/theme"), Some(&Node::String("paper".into())));
        // A PICK PER FIELD.
        let picked = merge("config/display.json", base, ours, theirs, &|p: &str| {
            (p == "/screens/b/scale").then_some(Pick::Theirs)
        })
        .unwrap();
        let node = Node::parse(&picked.bytes).unwrap();
        assert_eq!(node.at("/theme"), Some(&Node::String("paper".into())));
        assert_eq!(node.at("/screens/1/scale").unwrap().to_string(), "5");
        // A DELETION against a change clashes; against nothing, it is taken.
        let ours = br#"{"theme": "owl", "screens": [{"id": "a", "scale": 1}], "tags": ["x"]}"#;
        let theirs = br#"{"theme": "owl", "size": 12, "screens": [{"id": "a", "scale": 1}, {"id": "b", "scale": 9}], "tags": ["x"]}"#;
        let joined = merge("config/display.json", base, ours, theirs, &none).unwrap();
        let pointers: Vec<&str> = joined.clashes.iter().map(|c| c.pointer.as_str()).collect();
        assert_eq!(pointers, ["/screens/b"]);
        assert_eq!(joined.clashes[0].ours, None);
        let node = Node::parse(&joined.bytes).unwrap();
        assert_eq!(node.at("/size"), None, "a deletion of an unchanged field");
    }

    #[test]
    fn what_is_not_json_is_not_merged_here() {
        assert!(merge("config/a.json", b"{", b"{}", b"{}", &none).is_none());
        assert!(merge("home/notes.md", b"{}", b"{}", b"{}", &none).is_none());
        assert!(is_json_name("config/THEME.JSON"));
        // A LIST WITH AN ELEMENT WITHOUT AN ID, or two alike, is one value.
        let base = br#"{"l": [{"id": "a"}, {"x": 1}]}"#;
        let ours = br#"{"l": [{"id": "a"}, {"x": 2}]}"#;
        let theirs = br#"{"l": [{"id": "a"}, {"x": 3}]}"#;
        let joined = merge("config/a.json", base, ours, theirs, &none).unwrap();
        assert_eq!(joined.clashes.len(), 1);
        assert_eq!(joined.clashes[0].pointer, "/l");
        // ONE SIDE'S BYTES, when the result is that side: no rewrite.
        let joined = merge(
            "config/a.json",
            b"{\"a\":1}",
            b"{\"a\":2}",
            b"{\"a\":1}",
            &none,
        );
        assert_eq!(joined.unwrap().bytes, b"{\"a\":2}");
    }

    fn entry(id: &str, version: &str, at: &str) -> String {
        format!(
            r#"{{"source": "appimage", "id": "appimage:{id}", "machines": null, "edited-at": "{at}", "edited-on": "laptop", "key": "{id}", "name": "{id}", "pin": {{"url": "https://example.org/{id}-{version}.AppImage", "hash": "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=", "version": "{version}", "pinned-at": "{at}"}}, "follows": null, "icon": null}}"#
        )
    }

    fn declared(entries: &[String]) -> Vec<u8> {
        format!(r#"{{"format": 1, "apps": [{}]}}"#, entries.join(", ")).into_bytes()
    }

    /// FAILS IF: two pin refreshes of one app ask the person, the later edit
    /// does not win, a deletion loses to a pin-only change, or a real change
    /// on both sides is not a clash of the whole entry.
    #[test]
    fn the_persons_lists_settle_pins_by_time_and_deletions_win() {
        const PATH: &str = "state/person/software/declared.json";
        assert_eq!(list_of(PATH), Some(List::Declared));
        assert_eq!(
            list_of("state/person/software/declined.json"),
            Some(List::Declined)
        );
        assert_eq!(list_of("state/person/options.json"), None);
        assert_eq!(
            list_of("state/person/software/distro-pins.json"),
            Some(List::DistroPins)
        );
        // THE FOLLOWED DISTRO PINS: two machines that followed one release
        // to two versions are asked nothing, and the later pin wins.
        let pin = |version: &str, at: &str| {
            format!(
                r#"{{"id": "cadcraft", "url": "https://github.com/storytold/cadcraft/releases/download/v{version}/cadcraft-{version}-linux-x86_64.AppImage", "hash": "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=", "version": "{version}", "pinned-at": "{at}"}}"#
            )
        };
        let pins = |p: &str| format!(r#"{{"format": 1, "pins": [{p}]}}"#).into_bytes();
        let joined = merge(
            "state/person/software/distro-pins.json",
            &pins(&pin("0.8.1", "2026-10-01T10:00:00Z")),
            &pins(&pin("0.9.0", "2026-10-03T10:00:00Z")),
            &pins(&pin("0.9.1", "2026-10-04T10:00:00Z")),
            &none,
        )
        .unwrap();
        assert!(joined.clashes.is_empty(), "{:?}", joined.clashes);
        assert!(
            text(&joined).contains("\"version\": \"0.9.1\""),
            "{}",
            text(&joined)
        );
        let base = declared(&[
            entry("one", "1.0", "2026-10-01T10:00:00Z"),
            entry("two", "1.0", "2026-10-01T10:00:00Z"),
        ]);
        let ours = declared(&[
            entry("one", "1.1", "2026-10-03T10:00:00Z"),
            entry("two", "1.1", "2026-10-03T10:00:00Z"),
        ]);
        let theirs = declared(&[entry("one", "1.2", "2026-10-04T10:00:00Z")]);
        let joined = merge(PATH, &base, &ours, &theirs, &none).unwrap();
        assert!(joined.clashes.is_empty(), "{:?}", joined.clashes);
        let merged: Declared = ronoctua_person::parse(&joined.bytes).unwrap();
        assert_eq!(merged.apps.len(), 1, "the deletion lost to a pin");
        let written = text(&joined);
        assert!(
            written.contains("\"version\": \"1.2\""),
            "the later pin lost: {written}"
        );
        assert!(written.ends_with("}\n"), "not the person's own shape");

        // A REAL CHANGE ON BOTH SIDES is one clash: the whole entry.
        let ours = declared(&[entry("one", "1.0", "2026-10-01T10:00:00Z")
            .replace("\"machines\": null", "\"machines\": [\"laptop\"]")]);
        let theirs = declared(&[entry("one", "1.0", "2026-10-01T10:00:00Z")
            .replace("\"machines\": null", "\"machines\": [\"desk\"]")]);
        let joined = merge(PATH, &base, &ours, &theirs, &none).unwrap();
        let pointers: Vec<&str> = joined.clashes.iter().map(|c| c.pointer.as_str()).collect();
        assert_eq!(pointers, ["/apps/appimage:one"]);
        let picked = merge(PATH, &base, &ours, &theirs, &|_: &str| Some(Pick::Theirs)).unwrap();
        assert!(text(&picked).contains("\"desk\""));
    }

    /// FAILS IF: VS Code's list of extensions is not joined as a SET when
    /// both machines changed it — an install on one side and an uninstall on
    /// the other both kept, never a clash — or the same document under
    /// another name is (its array is ONE value there, and both changed it).
    #[test]
    fn the_vscode_list_merges_as_a_set() {
        let list = |ids: &[&str]| {
            let quoted: Vec<String> = ids.iter().map(|i| format!("\"{i}\"")).collect();
            format!(
                "{{\"format\": 1, \"extensions\": [{}]}}\n",
                quoted.join(", ")
            )
            .into_bytes()
        };
        let base = list(&["a.x", "z.z"]);
        let ours = list(&["a.x", "n.ours", "z.z"]);
        let theirs = list(&["a.x"]);
        let joined = merge(
            crate::offers::EXTENSIONS_LIST,
            &base,
            &ours,
            &theirs,
            &|_| None,
        )
        .unwrap();
        assert!(joined.clashes.is_empty(), "{:?}", joined.clashes);
        let ids = ronoctua_vscode::parse_list(&joined.bytes).unwrap().ids;
        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            ["a.x", "n.ours"],
            "an addition or a removal was lost"
        );
        let elsewhere = merge("config/some-list.json", &base, &ours, &theirs, &|_| None).unwrap();
        assert!(
            !elsewhere.clashes.is_empty(),
            "the general rules joined the set"
        );
    }
}
