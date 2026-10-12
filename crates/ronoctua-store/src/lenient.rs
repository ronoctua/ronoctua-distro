//! Reading a file that does not read whole: keep the rest of it.
//!
//! # Why
//!
//! A store's file is written by more than one machine (the belongings carry it
//! between the person's machines) and by the person's own hand. One value this
//! machine cannot take — a theme answer a newer distro added, a typo in one
//! filter — used to cost EVERYTHING in the file: the read fell back to the
//! default, and the next write made the loss permanent. Now it costs that one
//! value, and the value is not lost even then.
//!
//! # How
//!
//! The file is parsed into an ordered document ([`Node`]) and read as `T`
//! through `serde_path_to_error`, which says WHERE a read failed. The value
//! there is changed for one `T` can take, and the read tried again — at most
//! [`MOST_TRIES`] times:
//!
//! - an ELEMENT of a list that will not read is dropped;
//! - a FIELD that will not read takes `T::default()`'s value at the same
//!   pointer, or is taken out when the default has none there (so the type's
//!   own `#[serde(default)]` answers);
//! - a field the type needs and the file lacks is filled from the default;
//! - the same place failing twice moves the change to its PARENT — the
//!   coarse paths of an internally tagged or untagged enum land there;
//! - only when all of that fails is the whole file the default.
//!
//! Each change is a [`Repair`]. Keys the type passed over (`serde_ignored`) —
//! a newer machine's fields — are noted, so a write can put them back. A
//! domain may also REFUSE a value serde accepted (an id that names no theme):
//! its [`Refused`] answers are repaired the same way.
//!
//! # And a write keeps what it did not mean to change
//!
//! [`kept_over`]: the value written, with the unknown fields put back where it
//! lacks them, every repair undone where the value still holds what the read
//! substituted, and the old file's key order.

use std::collections::BTreeMap;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::json::{self, Node};
use crate::repair::{Repair, Why};

/// How many reads a file gets before it is the default — every one repairs a
/// place, so this is also the most repairs one file can have.
pub const MOST_TRIES: usize = 64;

/// A value a domain's own check refuses after serde accepted it: an id that
/// names nothing this machine knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// Where, as a pointer into the document.
    pub pointer: String,
    /// Why — most often [`Why::UnknownVariant`].
    pub why: Why,
    /// What to read instead; `None` is `T::default()`'s value there (or the
    /// field taken out when the default has none).
    pub kept: Option<Node>,
}

/// What a lenient read made of a document.
#[derive(Debug, Clone)]
pub struct Lenient<T> {
    /// The value.
    pub value: T,
    /// The repairs, in the order they were made.
    pub repairs: Vec<Repair>,
    /// The keys `T` passed over, by pointer into [`Lenient::doc`], with what
    /// they held.
    pub unknown: Vec<(String, Node)>,
    /// The document as it was finally read: the file, repaired.
    pub doc: Node,
}

impl<T> Lenient<T> {
    /// Whether nothing of the file could be kept: the whole document was
    /// replaced by the default.
    #[must_use]
    pub fn lost_everything(&self) -> bool {
        self.repairs.iter().any(|r| r.pointer.is_empty())
    }
}

/// One failure, located.
enum Fault {
    /// A value that will not read.
    Bad {
        /// Where.
        pointer: String,
        /// Why.
        why: Why,
    },
    /// A key a type that refuses unknown fields (`deny_unknown_fields`) met:
    /// taken out and kept aside, like any key a type passes over.
    Unknown {
        /// The key's pointer.
        pointer: String,
    },
}

/// Reads `doc` as `T`, repairing what will not read — see the module's header.
pub fn read<T>(doc: Node, check: &dyn Fn(&T) -> Vec<Refused>) -> Lenient<T>
where
    T: DeserializeOwned + Serialize + Default,
{
    let fallback = Node::of(&T::default()).unwrap_or(Node::Null);
    let mut doc = doc;
    let mut mender = Mender {
        fallback: &fallback,
        tries: BTreeMap::new(),
        repairs: Vec::new(),
    };
    // Keys taken out of a type that refuses them: kept aside like the ones a
    // type passes over.
    let mut set_aside: Vec<(String, Node)> = Vec::new();
    // A DOCUMENT OF ANOTHER KIND than the type's — a list where a record
    // belongs — is nothing of it, whole. Read element by element it would be
    // taken apart position by position (serde reads a record from a list),
    // which keeps nothing anybody wrote and says it kept something.
    let kind = |n: &Node| std::mem::discriminant(n);
    if matches!(fallback, Node::Object(_) | Node::Array(_)) && kind(&doc) != kind(&fallback) {
        mender.mend(&mut doc, String::new(), Why::WrongType, None);
    }
    for _ in 0..MOST_TRIES {
        let mut ignored: Vec<Vec<String>> = Vec::new();
        // READ FROM A `serde_json::Value`, not from text: the text reader says
        // "expected value" of a number where a word belongs, and the `Value`
        // says "invalid type" — which is the `why` a person is shown. Its
        // objects are sorted, so faults come in the keys' alphabetical order;
        // the ledger is a sequence, and that order is what it records.
        let attempt: Result<T, _> = {
            let mut note = |path: serde_ignored::Path<'_>| ignored.push(ignored_tokens(&path));
            serde_path_to_error::deserialize(serde_ignored::Deserializer::new(
                doc.to_value(),
                &mut note,
            ))
        };
        match attempt {
            Ok(value) => {
                let refused = check(&value);
                if refused.is_empty() {
                    let mut unknown: Vec<(String, Node)> = ignored
                        .iter()
                        .map(|t| json::pointer_of(t))
                        .filter_map(|p| doc.at(&p).cloned().map(|n| (p, n)))
                        .collect();
                    unknown.extend(set_aside);
                    return Lenient {
                        value,
                        repairs: mender.repairs,
                        unknown,
                        doc,
                    };
                }
                let mut moved = false;
                for r in refused {
                    moved |= mender.mend(&mut doc, r.pointer, r.why, r.kept);
                }
                if !moved {
                    break;
                }
            }
            Err(e) => {
                let moved = match fault_of(&e) {
                    Fault::Unknown { pointer } => match doc.take(&pointer) {
                        Some(node) => {
                            set_aside.push((pointer, node));
                            true
                        }
                        // NOT THERE: the message named a key the document does
                        // not hold where the path says — read it as a bad
                        // value of the object instead, so the loop moves.
                        None => mender.mend(&mut doc, json::parent(&pointer), Why::Invalid, None),
                    },
                    Fault::Bad { pointer, why } => mender.mend(&mut doc, pointer, why, None),
                };
                if !moved {
                    break;
                }
            }
        }
    }
    // NOTHING LEFT TO TRY: the whole file is the default, and that is a
    // repair too — the ledger says what was lost.
    tracing::error!(
        tries = MOST_TRIES,
        "a file would not read even repaired; reading the default"
    );
    let mut repairs = mender.repairs;
    if !repairs.iter().any(|r| r.pointer.is_empty()) {
        let found = std::mem::replace(&mut doc, fallback.clone());
        repairs.push(Repair {
            pointer: String::new(),
            found,
            kept: Some(fallback),
            why: Why::Invalid,
        });
    }
    Lenient {
        value: T::default(),
        repairs,
        unknown: Vec::new(),
        doc,
    }
}

/// What a write puts on disk: `new` (the value written), with the old file's
/// unknown keys put back where `new` lacks them, every repair of the old
/// file put back where the writer left the value as the read gave it (last
/// first; a repair inside a place the writer changed stays undone), and the
/// old file's key order.
///
/// # "Left as the read gave it" is judged against the value READ
///
/// Not against what the repair says it substituted. A value taken out of a
/// record (the type's default has nothing at that place: a field of an
/// element of a list, of a value of a map, of an `Option` that defaults to
/// `null`) is read as the type's own field default — and the type WRITES
/// that default back, `"normal"` or `null`. Judged by absence, every such
/// repair looked like the writer's choice, and the file's own value — a
/// newer machine's — was erased by the first save of anything else. So the
/// value read is serialised beside the one written (`base`), the repairs are
/// put back into both, in step, and a place is the writer's only when the
/// two differ there. A place in a list is found again in the value written by
/// its element's `id` (or by its index when the lists are as long); one that
/// cannot be found — its element deleted by the writer — is the writer's.
#[must_use]
pub fn kept_over<T: Serialize>(new: Node, old: &Node, was: &Lenient<T>) -> Node {
    let mut out = new;
    // WHAT A WRITER THAT CHANGED NOTHING WOULD WRITE.
    let mut base = Node::of(&was.value).unwrap_or_else(|_| was.doc.clone());
    for (pointer, value) in &was.unknown {
        put_unknown(&mut out, pointer, value, &was.doc);
        put_unknown(&mut base, pointer, value, &was.doc);
    }
    let mut changed: Vec<&str> = Vec::new();
    for repair in was.repairs.iter().rev() {
        if changed.iter().any(|c| json::covers(c, &repair.pointer)) {
            continue;
        }
        if !put_back(repair, &mut base, &mut out) {
            changed.push(&repair.pointer);
        }
    }
    out.ordered_like(old)
}

/// Puts a key the type passed over (at `pointer` in `from`) back into
/// `into`, where `into` lacks it and the record that held it is there.
fn put_unknown(into: &mut Node, pointer: &str, value: &Node, from: &Node) {
    if let Some(there) = translated(pointer, from, into)
        && into.at(&there).is_none()
        && json::split(&there)
            .is_some_and(|(parent, _)| matches!(into.at(&parent), Some(Node::Object(_))))
    {
        into.put(&there, value.clone());
    }
}

/// Puts back, in `out` (the value being written), what the file said at
/// `repair`'s place — when the writer left that place as `base` (the value
/// read, serialised, with the later repairs already put back) has it; and in
/// `base` too, so the earlier repairs' places, which are counted in the
/// document as it was at their step, are found in both. `false` when the
/// writer changed the place or it cannot be found in `out`: then the
/// writer's value stands.
fn put_back(repair: &Repair, base: &mut Node, out: &mut Node) -> bool {
    let found = &repair.found;
    let Some((parent, last)) = json::split(&repair.pointer) else {
        // THE WHOLE DOCUMENT was replaced: only an untouched default gives
        // the file back.
        if *out != *base {
            return false;
        }
        *out = found.clone();
        *base = found.clone();
        return true;
    };
    // AN ELEMENT DROPPED from a list goes back beside the element it
    // followed: the writer never saw it, so it cannot have meant to remove it.
    if repair.kept.is_none()
        && let Some(Node::Array(was)) = base.at(&parent)
    {
        let Ok(i) = last.parse::<usize>() else {
            return false;
        };
        let Some(list) = translated(&parent, base, out) else {
            return false;
        };
        let Some(Node::Array(now)) = out.at(&list) else {
            return false;
        };
        // AFTER THE ONE IT FOLLOWED, found by its `id`; without one, at the
        // same index (the end, when the list became shorter than that).
        let at = if i == 0 {
            0
        } else {
            was.get(i - 1)
                .and_then(Node::id)
                .and_then(|id| now.iter().position(|n| n.id().as_deref() == Some(&id)))
                .map_or(i.min(now.len()), |j| j + 1)
        };
        if !out.insert(&format!("{list}/{at}"), found.clone()) {
            return false;
        }
        base.insert(&repair.pointer, found.clone());
        return true;
    }
    let Some(there) = translated(&repair.pointer, base, out) else {
        return false;
    };
    if base.at(&repair.pointer) != out.at(&there) {
        return false;
    }
    match (&repair.kept, repair.why) {
        // A FIELD FILLED IN goes away again.
        (Some(_), Why::Missing) => {
            out.take(&there);
            base.take(&repair.pointer);
        }
        // A VALUE REPLACED, OR TAKEN OUT, comes back.
        _ => {
            if !out.put(&there, found.clone()) {
                return false;
            }
            base.put(&repair.pointer, found.clone());
        }
    }
    true
}

/// `pointer`, a place in `from`, as the same place in `to`: an element of a
/// list found again by its `id`, or by its index when the two lists are as
/// long. `None` when it cannot be placed.
fn translated(pointer: &str, from: &Node, to: &Node) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    let (mut a, mut b) = (Some(from), Some(to));
    for token in json::tokens(pointer)? {
        match (a, b) {
            (Some(Node::Array(xs)), Some(Node::Array(ys))) => {
                let i: usize = token.parse().ok()?;
                let x = xs.get(i)?;
                let j = match x.id() {
                    Some(id) => ys.iter().position(|y| y.id().as_deref() == Some(&id))?,
                    None if xs.len() == ys.len() => i,
                    None => return None,
                };
                out.push(j.to_string());
                a = Some(x);
                b = ys.get(j);
            }
            (Some(Node::Object(_)), Some(Node::Object(_))) => {
                a = a.and_then(|n| n.at(&json::pointer_of(&[&token])));
                b = b.and_then(|n| n.at(&json::pointer_of(&[&token])));
                out.push(token);
            }
            // THE LAST TOKEN may name a key `to` does not have yet — that
            // is the point of putting it back.
            (_, Some(Node::Object(_))) => {
                a = None;
                b = None;
                out.push(token);
            }
            _ => return None,
        }
    }
    Some(json::pointer_of(&out))
}

/// The state of one read's repairs.
struct Mender<'a> {
    /// `T::default()` as a document — where a substitute comes from.
    fallback: &'a Node,
    /// How often each place was repaired.
    tries: BTreeMap<String, u32>,
    /// What was done, in order.
    repairs: Vec<Repair>,
}

impl Mender<'_> {
    /// Repairs the place `pointer` (or, if it was repaired before, its
    /// parent — and so on up). `false` when there is nothing left to do: the
    /// whole document was already the default.
    fn mend(&mut self, doc: &mut Node, pointer: String, why: Why, kept: Option<Node>) -> bool {
        let mut at = pointer;
        let mut kept = kept;
        // THE SAME PLACE TWICE climbs: what was put there did not read
        // either, so the fault is above it.
        while !at.is_empty() && self.tries.contains_key(&at) {
            at = json::parent(&at);
            kept = None;
        }
        loop {
            if at.is_empty() {
                if self.tries.contains_key(&at) {
                    return false;
                }
                self.tries.insert(String::new(), 1);
                let found = std::mem::replace(doc, self.fallback.clone());
                let (found, why) = self.unwind("", found, why);
                self.repairs.push(Repair {
                    pointer: at,
                    found,
                    kept: Some(self.fallback.clone()),
                    why,
                });
                return true;
            }
            *self.tries.entry(at.clone()).or_default() += 1;
            if self.mend_at(doc, &at, why, kept.take()) {
                return true;
            }
            // NOTHING TO DO HERE (a missing field the default has no value
            // for, a parent that is not a container): one level up.
            at = json::parent(&at);
        }
    }

    /// One repair at `at`; `false` when this place cannot be repaired.
    fn mend_at(&mut self, doc: &mut Node, at: &str, why: Why, kept: Option<Node>) -> bool {
        let Some((parent, _)) = json::split(at) else {
            return false;
        };
        let (found, kept) = match doc.at(&parent) {
            // AN ELEMENT OF A LIST is dropped: the list without it is a list.
            Some(Node::Array(_)) => match doc.take(at) {
                Some(found) => {
                    // THE ELEMENTS AFTER IT MOVED UP: what was tried at
                    // their old indices was tried on somebody else.
                    let from: usize = json::split(at)
                        .and_then(|(_, i)| i.parse().ok())
                        .unwrap_or(0);
                    self.tries.retain(|k, _| {
                        k.strip_prefix(parent.as_str())
                            .and_then(|rest| rest.strip_prefix('/'))
                            .and_then(|rest| rest.split('/').next())
                            .and_then(|i| i.parse::<usize>().ok())
                            .is_none_or(|i| i < from)
                    });
                    (found, None)
                }
                None => return false,
            },
            Some(Node::Object(_)) => {
                let substitute = kept.or_else(|| self.fallback.at(at).cloned());
                match (doc.at(at).cloned(), substitute) {
                    (Some(found), Some(substitute)) => {
                        doc.put(at, substitute.clone());
                        (found, Some(substitute))
                    }
                    (Some(found), None) => {
                        doc.take(at);
                        (found, None)
                    }
                    (None, Some(substitute)) => {
                        doc.put(at, substitute.clone());
                        self.repairs.push(Repair {
                            pointer: at.to_owned(),
                            found: Node::Null,
                            kept: Some(substitute),
                            why: Why::Missing,
                        });
                        return true;
                    }
                    (None, None) => return false,
                }
            }
            _ => return false,
        };
        let (found, why) = self.unwind(at, found, why);
        self.repairs.push(Repair {
            pointer: at.to_owned(),
            found,
            kept,
            why,
        });
        true
    }

    /// Folds the repairs already made INSIDE `at` into the one being made
    /// there: `found` becomes what the file said at `at` before any of them,
    /// and the why the first of them had — one fault, one line in the ledger,
    /// however many steps it took to climb to where it could be mended.
    fn unwind(&mut self, at: &str, mut found: Node, why: Why) -> (Node, Why) {
        let mut why = why;
        let mut i = self.repairs.len();
        while i > 0 {
            i -= 1;
            let inner = &self.repairs[i].pointer;
            if inner == at || !json::covers(at, inner) {
                continue;
            }
            let inner = self.repairs.remove(i);
            why = inner.why;
            let rebased = Repair {
                pointer: inner.pointer[at.len()..].to_owned(),
                ..inner
            };
            rebased.undo(&mut found);
        }
        (found, why)
    }
}

/// Where a read failed, and why, out of `serde_path_to_error`'s error.
fn fault_of(e: &serde_path_to_error::Error<serde_json::Error>) -> Fault {
    let mut tokens: Vec<String> = Vec::new();
    for segment in e.path().iter() {
        match segment {
            serde_path_to_error::Segment::Seq { index } => tokens.push(index.to_string()),
            serde_path_to_error::Segment::Map { key } => tokens.push(key.clone()),
            serde_path_to_error::Segment::Enum { variant } => tokens.push(variant.clone()),
            // THE PATH STOPS BEING KNOWN here (a key that is not a string):
            // the repair lands on what is known, which is coarser and still
            // right.
            serde_path_to_error::Segment::Unknown => break,
        }
    }
    let message = e.inner().to_string();
    // THE KEY may already end the path (an unknown key is captured as it is
    // read) or not (a missing one never was).
    let mut ending_with = |field: &str| {
        if tokens.last().map(String::as_str) != Some(field) {
            tokens.push(field.to_owned());
        }
        json::pointer_of(&tokens)
    };
    if let Some(field) = named(&message, "missing field") {
        return Fault::Bad {
            pointer: ending_with(field),
            why: Why::Missing,
        };
    }
    if let Some(field) = named(&message, "unknown field") {
        return Fault::Unknown {
            pointer: ending_with(field),
        };
    }
    let why = if message.starts_with("unknown variant") {
        Why::UnknownVariant
    } else if message.starts_with("invalid type") {
        Why::WrongType
    } else {
        Why::Invalid
    };
    Fault::Bad {
        pointer: json::pointer_of(&tokens),
        why,
    }
}

/// The name between the first pair of backticks after `prefix`, when the
/// message begins with it — serde's own wording (`missing field `x``).
fn named<'a>(message: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = message.strip_prefix(prefix)?.trim_start();
    let rest = rest.strip_prefix('`')?;
    let end = rest.find('`')?;
    Some(&rest[..end])
}

/// The tokens of a key `serde_ignored` saw passed over.
fn ignored_tokens(path: &serde_ignored::Path<'_>) -> Vec<String> {
    match path {
        serde_ignored::Path::Root => Vec::new(),
        serde_ignored::Path::Seq { parent, index } => {
            let mut t = ignored_tokens(parent);
            t.push(index.to_string());
            t
        }
        serde_ignored::Path::Map { parent, key } => {
            let mut t = ignored_tokens(parent);
            t.push(key.clone());
            t
        }
        serde_ignored::Path::Some { parent }
        | serde_ignored::Path::NewtypeStruct { parent }
        | serde_ignored::Path::NewtypeVariant { parent } => ignored_tokens(parent),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    fn node(text: &str) -> Node {
        Node::parse(text.as_bytes()).unwrap()
    }

    fn nothing<T>(_: &T) -> Vec<Refused> {
        Vec::new()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Mode {
        #[default]
        Calm,
        Busy,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Shape {
        #[default]
        Dot,
        Ring {
            size: u8,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Item {
        id: String,
        mode: Mode,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "kebab-case")]
    enum Tagged {
        Note { text: String },
        Count { n: u32 },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(default)]
    struct Prefs {
        mode: Mode,
        size: u16,
        name: String,
        items: Vec<Item>,
        shape: Shape,
        tagged: Vec<Tagged>,
    }

    impl Default for Prefs {
        fn default() -> Self {
            Self {
                mode: Mode::Calm,
                size: 12,
                name: "me".to_owned(),
                items: Vec::new(),
                shape: Shape::Dot,
                tagged: Vec::new(),
            }
        }
    }

    /// The repairs as `(pointer, why, found, kept)`, for a whole comparison.
    fn said(repairs: &[Repair]) -> Vec<(String, Why, String, Option<String>)> {
        repairs
            .iter()
            .map(|r| {
                (
                    r.pointer.clone(),
                    r.why,
                    r.found.to_string(),
                    r.kept.as_ref().map(Node::to_string),
                )
            })
            .collect()
    }

    fn line(
        pointer: &str,
        why: Why,
        found: &str,
        kept: Option<&str>,
    ) -> (String, Why, String, Option<String>) {
        (
            pointer.to_owned(),
            why,
            found.to_owned(),
            kept.map(str::to_owned),
        )
    }

    /// FAILS IF: a file with one unknown variant, one value of the wrong type
    /// and one element of a list that will not read loses anything else, or
    /// the repairs are not exactly those three — the element's line saying
    /// what the file had, whole. (The same file through a `Store`, and its
    /// LEDGER, is `crate::tests::a_store_keeps_the_rest_of_a_broken_file`.)
    #[test]
    fn a_broken_file_keeps_the_rest_and_says_what_it_repaired() {
        let doc = node(
            r#"{
              "mode": "frantic",
              "size": "large",
              "name": "kept",
              "items": [
                {"id": "a", "mode": "busy"},
                {"id": "b", "mode": 7},
                {"id": "c", "mode": "calm"}
              ]
            }"#,
        );
        let got = super::read::<Prefs>(doc, &nothing);
        assert_eq!(got.value.name, "kept", "a good field was lost");
        assert_eq!(got.value.mode, Mode::Calm);
        assert_eq!(got.value.size, 12);
        assert_eq!(
            got.value.items,
            vec![
                Item {
                    id: "a".to_owned(),
                    mode: Mode::Busy,
                },
                Item {
                    id: "c".to_owned(),
                    mode: Mode::Calm,
                },
            ],
            "a good element was lost"
        );
        // IN THE ORDER THEY WERE MADE, which is the keys' alphabetical one
        // (see `read`): `items` before `mode` before `size`.
        assert_eq!(
            said(&got.repairs),
            vec![
                line("/items/1", Why::WrongType, r#"{"id":"b","mode":7}"#, None),
                line(
                    "/mode",
                    Why::UnknownVariant,
                    "\"frantic\"",
                    Some("\"calm\"")
                ),
                line("/size", Why::WrongType, "\"large\"", Some("12")),
            ]
        );
        assert!(!got.lost_everything());
    }

    #[test]
    fn a_coarse_path_climbs_to_what_holds_it() {
        // AN INTERNALLY TAGGED ENUM is read through a buffer, so serde says
        // only "this element": the element goes, its neighbours stay.
        let got = super::read::<Prefs>(
            node(
                r#"{"tagged": [{"kind": "note", "text": "a"}, {"kind": "count", "n": "x"}, {"kind": "later"}, {"kind": "count", "n": 2}]}"#,
            ),
            &nothing,
        );
        assert_eq!(
            got.value.tagged,
            vec![
                Tagged::Note {
                    text: "a".to_owned()
                },
                Tagged::Count { n: 2 }
            ]
        );
        assert_eq!(got.repairs.len(), 2, "{:?}", said(&got.repairs));
        assert!(got.repairs.iter().all(|r| r.kept.is_none()));

        // A STRUCT VARIANT whose field will not read, with no default at that
        // depth, climbs until something can be put there: the whole enum's
        // default — and ONE line, saying what the file had.
        let got = super::read::<Prefs>(
            node(r#"{"shape": {"ring": {"size": "x"}}, "size": 3}"#),
            &nothing,
        );
        assert_eq!(got.value.shape, Shape::Dot);
        assert_eq!(got.value.size, 3);
        assert_eq!(
            said(&got.repairs),
            vec![line(
                "/shape",
                Why::WrongType,
                r#"{"ring":{"size":"x"}}"#,
                Some("\"dot\"")
            )]
        );
    }

    #[test]
    fn a_document_that_will_not_be_the_type_is_the_default_and_says_what_was_lost() {
        let got = super::read::<Prefs>(node("[1, 2, 3]"), &nothing);
        assert_eq!(got.value, Prefs::default());
        assert!(got.lost_everything());
        assert_eq!(got.repairs.len(), 1);
        assert_eq!(got.repairs[0].found.to_string(), "[1,2,3]");
        // A check that refuses everything cannot loop: it climbs, replaces the
        // whole document once, and stops.
        let never = |_: &Prefs| {
            vec![Refused {
                pointer: "/name".to_owned(),
                why: Why::Invalid,
                kept: None,
            }]
        };
        let got = super::read::<Prefs>(node(r#"{"name": "x"}"#), &never);
        assert!(got.lost_everything());
        assert_eq!(got.value, Prefs::default());
        assert!(got.repairs.len() <= 2, "{:?}", said(&got.repairs));
    }

    #[test]
    fn the_keys_a_type_passes_over_are_noted_where_they_are() {
        let got = super::read::<Prefs>(
            node(
                r#"{"mode": "busy", "future": {"x": 1}, "items": [{"id": "a", "mode": "calm", "colour": "red"}]}"#,
            ),
            &nothing,
        );
        assert!(got.repairs.is_empty());
        let unknown: Vec<(String, String)> = got
            .unknown
            .iter()
            .map(|(p, n)| (p.clone(), n.to_string()))
            .collect();
        assert!(
            unknown.contains(&("/future".to_owned(), r#"{"x":1}"#.to_owned())),
            "{unknown:?}"
        );
        assert!(
            unknown.contains(&("/items/0/colour".to_owned(), "\"red\"".to_owned())),
            "{unknown:?}"
        );
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, default)]
    struct Strict {
        a: u8,
        b: u8,
    }

    #[test]
    fn a_type_that_refuses_unknown_keys_reads_and_the_key_is_kept_aside() {
        let got = super::read::<Strict>(node(r#"{"a": 1, "newer": true, "b": 2}"#), &nothing);
        assert_eq!(got.value, Strict { a: 1, b: 2 });
        assert!(got.repairs.is_empty(), "{:?}", said(&got.repairs));
        assert_eq!(got.unknown.len(), 1);
        assert_eq!(got.unknown[0].0, "/newer");
    }

    #[test]
    fn a_domains_refusal_is_repaired_with_what_it_says() {
        let only_known = |p: &Prefs| {
            if p.name == "ghost" {
                vec![Refused {
                    pointer: "/name".to_owned(),
                    why: Why::UnknownVariant,
                    kept: Some(Node::String("fallback".to_owned())),
                }]
            } else {
                Vec::new()
            }
        };
        let got = super::read::<Prefs>(node(r#"{"name": "ghost", "size": 4}"#), &only_known);
        assert_eq!(got.value.name, "fallback");
        assert_eq!(got.value.size, 4);
        assert_eq!(
            said(&got.repairs),
            vec![line(
                "/name",
                Why::UnknownVariant,
                "\"ghost\"",
                Some("\"fallback\"")
            )]
        );
    }

    #[test]
    fn a_missing_field_the_type_needs_is_filled_from_the_default() {
        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct Needs {
            want: u8,
            have: u8,
        }
        impl Default for Needs {
            fn default() -> Self {
                Self { want: 5, have: 0 }
            }
        }
        let got = super::read::<Needs>(node(r#"{"have": 1}"#), &nothing);
        assert_eq!(got.value, Needs { want: 5, have: 1 });
        assert_eq!(
            said(&got.repairs),
            vec![line("/want", Why::Missing, "null", Some("5"))]
        );
    }

    #[test]
    fn a_write_puts_back_what_it_did_not_mean_to_change() {
        let old = node(
            r#"{"name": "kept", "mode": "frantic", "future": 1, "items": [{"id": "a", "mode": "busy"}, {"id": "b", "mode": 7}], "size": 3}"#,
        );
        let was = read::<Prefs>(old.clone(), &nothing);
        // The writer changed `size` and nothing else.
        let mut value = was.value.clone();
        value.size = 4;
        let out = kept_over(Node::of(&value).unwrap(), &old, &was);
        assert_eq!(
            out.at("/mode"),
            Some(&node("\"frantic\"")),
            "an untouched repair was written"
        );
        assert_eq!(
            out.at("/future"),
            Some(&node("1")),
            "a newer field was lost"
        );
        assert_eq!(out.at("/size"), Some(&node("4")));
        assert_eq!(
            out.at("/items/1"),
            Some(&node(r#"{"id":"b","mode":7}"#)),
            "a dropped element was lost"
        );
        // AND THE OLD ORDER: `name` first, as the person wrote it.
        assert!(
            out.to_string()
                .starts_with(r#"{"name":"kept","mode":"frantic""#),
            "{out}"
        );

        // A WRITER THAT CHANGED THE REPAIRED VALUE wins: that is a choice.
        let mut value = was.value.clone();
        value.mode = Mode::Busy;
        let out = kept_over(Node::of(&value).unwrap(), &old, &was);
        assert_eq!(out.at("/mode"), Some(&node("\"busy\"")));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Transform {
        #[default]
        Normal,
        Flipped,
    }

    /// A value of a map whose fields the type defaults — `display.json`'s
    /// screens.
    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(default)]
    struct Screen {
        transform: Transform,
        scale: u8,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Verdict {
        Quiet,
        Loud,
    }

    /// An `Option` that a missing value reads as `None` and that is written
    /// as `null` — the notifications' `show`.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Apply {
        show: Option<Verdict>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Rule {
        id: String,
        apply: Apply,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(default)]
    struct Nested {
        screens: BTreeMap<String, Screen>,
        rules: Vec<Rule>,
    }

    /// The `show` of the rule `id` in a written document.
    fn show_of<'a>(out: &'a Node, id: &str) -> Option<&'a Node> {
        let Some(Node::Array(rules)) = out.at("/rules") else {
            return None;
        };
        rules
            .iter()
            .find(|r| r.id().as_deref() == Some(id))
            .and_then(|r| r.at("/apply/show"))
    }

    /// FAILS IF: a value repaired INSIDE a record that the type's default
    /// does not have — a field of a map's value, a field of a list's
    /// element — is written over by a save of something else. The read takes
    /// such a value out (the default has nothing there), the type writes its
    /// own field default back (`"normal"`, `null`), and judged by absence the
    /// write took that for the writer's choice: `"flipped-90"` erased by a
    /// change of another screen's scale. Also: the element moved or gone in
    /// the written list, and a writer that DID change the place.
    #[test]
    fn a_repair_inside_a_map_value_or_a_list_element_survives_an_unrelated_write() {
        let old = node(
            r#"{
              "screens": {
                "DEL U2720Q": {"transform": "flipped-90", "scale": 2},
                "LG": {"scale": 1}
              },
              "rules": [
                {"id": "a", "apply": {"show": "later"}},
                {"id": "b", "apply": {"show": "loud"}}
              ]
            }"#,
        );
        let was = read::<Nested>(old.clone(), &nothing);
        assert_eq!(
            said(&was.repairs),
            vec![
                line(
                    "/rules/0/apply/show",
                    Why::UnknownVariant,
                    "\"later\"",
                    None
                ),
                line(
                    "/screens/DEL U2720Q/transform",
                    Why::UnknownVariant,
                    "\"flipped-90\"",
                    None
                ),
            ]
        );
        assert_eq!(was.value.screens["DEL U2720Q"].transform, Transform::Normal);
        assert_eq!(was.value.rules[0].apply.show, None);

        // ANOTHER SCREEN'S SCALE, and nothing else.
        let mut value = was.value.clone();
        value.screens.get_mut("LG").unwrap().scale = 3;
        let out = kept_over(Node::of(&value).unwrap(), &old, &was);
        assert_eq!(
            out.at("/screens/DEL U2720Q/transform"),
            Some(&node("\"flipped-90\"")),
            "a repaired value of a map was written over: {out}"
        );
        assert_eq!(out.at("/screens/LG/scale"), Some(&node("3")));
        assert_eq!(
            show_of(&out, "a"),
            Some(&node("\"later\"")),
            "a repaired value of a list's element was written over: {out}"
        );
        assert_eq!(show_of(&out, "b"), Some(&node("\"loud\"")));

        // A RULE ADDED IN FRONT and another one's answer changed: the
        // repaired element is found again by its `id`.
        let mut value = was.value.clone();
        value.rules.insert(
            0,
            Rule {
                id: "c".to_owned(),
                apply: Apply { show: None },
            },
        );
        value.rules[2].apply.show = Some(Verdict::Quiet);
        let out = kept_over(Node::of(&value).unwrap(), &old, &was);
        assert_eq!(show_of(&out, "a"), Some(&node("\"later\"")), "{out}");
        assert_eq!(show_of(&out, "b"), Some(&node("\"quiet\"")), "{out}");
        assert_eq!(show_of(&out, "c"), Some(&Node::Null), "{out}");

        // THE RULE DELETED by the writer: its repair is moot — and never
        // lands on the rule that took its place.
        let mut value = was.value.clone();
        value.rules.remove(0);
        let out = kept_over(Node::of(&value).unwrap(), &old, &was);
        assert_eq!(show_of(&out, "a"), None);
        assert_eq!(show_of(&out, "b"), Some(&node("\"loud\"")), "{out}");

        // A WRITER THAT CHOSE the repaired place's value: written.
        let mut value = was.value.clone();
        value.screens.get_mut("DEL U2720Q").unwrap().transform = Transform::Flipped;
        value.rules[0].apply.show = Some(Verdict::Loud);
        let out = kept_over(Node::of(&value).unwrap(), &old, &was);
        assert_eq!(
            out.at("/screens/DEL U2720Q/transform"),
            Some(&node("\"flipped\""))
        );
        assert_eq!(show_of(&out, "a"), Some(&node("\"loud\"")));
    }
}
