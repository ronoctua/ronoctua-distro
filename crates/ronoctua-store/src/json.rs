//! An ordered JSON value — the file's own key order, kept.
//!
//! # Why not `serde_json::Value`
//!
//! Because this workspace builds `serde_json` without `preserve_order`, so a
//! `Value`'s object is a sorted map: a file read into one and written back
//! comes out with its keys in alphabetical order. A person who wrote their
//! preferences by hand — `theme` first, the rest under it — would find every
//! write of the machine's reordering them, and a diff of the belongings would
//! be a diff of the order. [`Node`] keeps the order the file had (its own
//! visitor reads the keys as they come), and a value serialised through it
//! keeps the order its type's fields have.
//!
//! # Pointers
//!
//! A place inside a document is an RFC 6901 JSON pointer: `""` is the whole
//! document, `/screens/0/scale` a field of the first screen, and `~1` / `~0`
//! stand for a `/` and a `~` inside a key. A token names an array's element by
//! its index and an object's field by its key — which of the two is decided by
//! what the document holds there, never by the token.

use std::collections::HashMap;
use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// A JSON value whose objects keep their keys in the order they came.
///
/// Equality is JSON's: two objects with the same keys and values are equal
/// whatever the order — what "the new value still equals the default" asks.
#[derive(Debug, Clone)]
pub enum Node {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, as `serde_json` keeps one.
    Number(serde_json::Number),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Self>),
    /// An object, its keys in their order; a key appears once.
    Object(Vec<(String, Self)>),
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.iter().any(|(bk, bv)| bk == k && bv == v))
            }
            _ => false,
        }
    }
}

impl Eq for Node {}

impl Node {
    /// The document `bytes` hold.
    ///
    /// # Errors
    ///
    /// They are not JSON.
    pub fn parse(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    /// `value` as a document, its fields in its type's order.
    ///
    /// Through text, on purpose: `serde_json::to_value` would sort the keys
    /// (see this module's header), and a serialiser of our own would be a
    /// second copy of `serde_json`'s rules for maps, enums and options.
    ///
    /// # Errors
    ///
    /// The value does not serialise (a map whose keys are not strings).
    pub fn of<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<Self> {
        Self::parse(&serde_json::to_vec(value)?)
    }

    /// The same document as a `serde_json::Value` — what a type is read from.
    /// The order is lost there, which is harmless: reading never asks it.
    #[must_use]
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// The document as a person reads it: pretty, two spaces, in order.
    #[must_use]
    pub fn pretty(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    /// The value at `pointer`, if the document has one there.
    #[must_use]
    pub fn at(&self, pointer: &str) -> Option<&Self> {
        let mut here = self;
        for token in tokens(pointer)? {
            here = here.child(&token)?;
        }
        Some(here)
    }

    /// The child a token names here: an element by index, a field by key.
    fn child(&self, token: &str) -> Option<&Self> {
        match self {
            Self::Array(items) => items.get(index_of(token)?),
            Self::Object(pairs) => pairs.iter().find(|(k, _)| k == token).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The same, to change.
    fn child_mut(&mut self, token: &str) -> Option<&mut Self> {
        match self {
            Self::Array(items) => items.get_mut(index_of(token)?),
            Self::Object(pairs) => pairs.iter_mut().find(|(k, _)| k == token).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The value at `pointer`, to change.
    pub fn at_mut(&mut self, pointer: &str) -> Option<&mut Self> {
        let mut here = self;
        for token in tokens(pointer)? {
            here = here.child_mut(&token)?;
        }
        Some(here)
    }

    /// Puts `value` at `pointer`: the whole document for `""`; a field set
    /// (added at the end when it was not there); an element REPLACED in an
    /// array, or added when the index is the array's length. `false` when the
    /// place does not exist and cannot be made (no parent, an index past the
    /// end, a parent that is not a container).
    pub fn put(&mut self, pointer: &str, value: Self) -> bool {
        let Some((parent, last)) = split(pointer) else {
            *self = value;
            return true;
        };
        match self.at_mut(&parent) {
            Some(Self::Object(pairs)) => {
                if let Some((_, v)) = pairs.iter_mut().find(|(k, _)| *k == last) {
                    *v = value;
                } else {
                    pairs.push((last, value));
                }
                true
            }
            Some(Self::Array(items)) => match index_of(&last) {
                Some(i) if i < items.len() => {
                    items[i] = value;
                    true
                }
                Some(i) if i == items.len() => {
                    items.push(value);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// Like [`Node::put`], but into an array it INSERTS — at the index, or at
    /// the end when the array has become shorter than that. What puts back an
    /// element a reading dropped.
    pub fn insert(&mut self, pointer: &str, value: Self) -> bool {
        let Some((parent, last)) = split(pointer) else {
            *self = value;
            return true;
        };
        match self.at_mut(&parent) {
            Some(Self::Array(items)) => match index_of(&last) {
                Some(i) => {
                    items.insert(i.min(items.len()), value);
                    true
                }
                None => false,
            },
            Some(Self::Object(_)) => self.put(pointer, value),
            _ => false,
        }
    }

    /// Takes the value at `pointer` out: a field removed, an element removed
    /// (the ones after it move up). `None` when there is nothing there; the
    /// whole document is never taken.
    pub fn take(&mut self, pointer: &str) -> Option<Self> {
        let (parent, last) = split(pointer)?;
        match self.at_mut(&parent)? {
            Self::Object(pairs) => {
                let i = pairs.iter().position(|(k, _)| *k == last)?;
                Some(pairs.remove(i).1)
            }
            Self::Array(items) => {
                let i = index_of(&last)?;
                (i < items.len()).then(|| items.remove(i))
            }
            _ => None,
        }
    }

    /// The `id` of an object that carries one (a string or a number) — what
    /// lets an element of a list be found again after the list moved.
    #[must_use]
    pub fn id(&self) -> Option<String> {
        match self.child("id")? {
            Self::String(s) => Some(s.clone()),
            Self::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    /// `self`, with every object's keys in the order `like` has them — the
    /// keys `like` does not have after, in their own order. Elements of
    /// arrays are matched by `id` when they carry one, or by position when
    /// the two arrays are as long.
    ///
    /// What a write does with the file it replaces: a person's order survives
    /// the machine's rewrite.
    #[must_use]
    pub fn ordered_like(self, like: &Self) -> Self {
        match (self, like) {
            (Self::Object(pairs), Self::Object(model)) => {
                let mut by_key: HashMap<String, Self> = HashMap::with_capacity(pairs.len());
                let mut order: Vec<String> = Vec::with_capacity(pairs.len());
                for (k, v) in pairs {
                    order.push(k.clone());
                    by_key.insert(k, v);
                }
                let mut out = Vec::with_capacity(order.len());
                for (k, m) in model {
                    if let Some(v) = by_key.remove(k) {
                        out.push((k.clone(), v.ordered_like(m)));
                    }
                }
                for k in order {
                    if let Some(v) = by_key.remove(&k) {
                        out.push((k, v));
                    }
                }
                Self::Object(out)
            }
            (Self::Array(items), Self::Array(model)) => {
                let by_id: HashMap<String, &Self> =
                    model.iter().filter_map(|m| Some((m.id()?, m))).collect();
                let same_length = items.len() == model.len();
                Self::Array(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let twin = item
                                .id()
                                .and_then(|id| by_id.get(&id).copied())
                                .or_else(|| same_length.then(|| &model[i]));
                            match twin {
                                Some(m) => item.ordered_like(m),
                                None => item,
                            }
                        })
                        .collect(),
                )
            }
            (node, _) => node,
        }
    }
}

impl fmt::Display for Node {
    /// Compact JSON — what a ledger's reader shows of a value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&text)
    }
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(b) => serializer.serialize_bool(*b),
            Self::Number(n) => n.serialize(serializer),
            Self::String(s) => serializer.serialize_str(s),
            Self::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Object(pairs) => {
                let mut map = serializer.serialize_map(Some(pairs.len()))?;
                for (k, v) in pairs {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

/// The visitor that keeps the keys in the order they come.
struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Node, E> {
        Ok(Node::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Node, E> {
        Ok(Node::Number(v.into()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Node, E> {
        Ok(Node::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Node, E> {
        Ok(serde_json::Number::from_f64(v).map_or(Node::Null, Node::Number))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Node, E> {
        Ok(Node::String(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Node, E> {
        Ok(Node::String(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Node, D::Error> {
        Node::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(4096));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Node::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut pairs: Vec<(String, Node)> =
            Vec::with_capacity(map.size_hint().unwrap_or(0).min(4096));
        while let Some((k, v)) = map.next_entry::<String, Node>()? {
            // A KEY TWICE is JSON that `serde_json` reads with the last value
            // winning; the first place is kept, so the order is still the
            // file's.
            if let Some((_, was)) = pairs.iter_mut().find(|(pk, _)| *pk == k) {
                *was = v;
            } else {
                pairs.push((k, v));
            }
        }
        Ok(Node::Object(pairs))
    }
}

/// The tokens of a pointer, unescaped; `None` for text that is not one (it
/// must be empty or begin with `/`).
#[must_use]
pub fn tokens(pointer: &str) -> Option<Vec<String>> {
    if pointer.is_empty() {
        return Some(Vec::new());
    }
    let rest = pointer.strip_prefix('/')?;
    Some(
        rest.split('/')
            .map(|t| t.replace("~1", "/").replace("~0", "~"))
            .collect(),
    )
}

/// The pointer of `tokens`, escaped.
#[must_use]
pub fn pointer_of<S: AsRef<str>>(tokens: &[S]) -> String {
    let mut out = String::new();
    for t in tokens {
        out.push('/');
        out.push_str(&t.as_ref().replace('~', "~0").replace('/', "~1"));
    }
    out
}

/// The pointer's parent and its last token, unescaped; `None` for the whole
/// document, which has no parent.
#[must_use]
pub fn split(pointer: &str) -> Option<(String, String)> {
    let mut all = tokens(pointer)?;
    let last = all.pop()?;
    Some((pointer_of(&all), last))
}

/// The parent of a pointer — the whole document's parent is itself.
#[must_use]
pub fn parent(pointer: &str) -> String {
    split(pointer).map_or_else(String::new, |(p, _)| p)
}

/// Whether `pointer` is `under` or lies inside it, token by token.
#[must_use]
pub fn covers(under: &str, pointer: &str) -> bool {
    pointer == under
        || pointer
            .strip_prefix(under)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// An array index as RFC 6901 writes one: digits, and no leading zero.
fn index_of(token: &str) -> Option<usize> {
    if token.is_empty() || (token.len() > 1 && token.starts_with('0')) {
        return None;
    }
    if !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    token.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(text: &str) -> Node {
        Node::parse(text.as_bytes()).unwrap()
    }

    #[test]
    fn a_document_keeps_the_order_it_was_written_in() {
        // THE REASON THIS TYPE EXISTS: `serde_json::Value` would sort these.
        let doc = node(r#"{"zebra": 1, "apple": {"y": true, "b": null}, "mid": [3, "x"]}"#);
        assert_eq!(
            doc.to_string(),
            r#"{"zebra":1,"apple":{"y":true,"b":null},"mid":[3,"x"]}"#
        );
        let pretty = String::from_utf8(doc.pretty()).unwrap();
        assert!(pretty.find("zebra").unwrap() < pretty.find("apple").unwrap());
        // A key twice: the last value, in the first place.
        assert_eq!(
            node(r#"{"a":1,"b":2,"a":3}"#).to_string(),
            r#"{"a":3,"b":2}"#
        );
    }

    #[derive(serde::Serialize)]
    struct Fields {
        zulu: u8,
        alpha: &'static str,
    }

    #[test]
    fn a_value_keeps_its_types_field_order() {
        let n = Node::of(&Fields {
            zulu: 1,
            alpha: "a",
        })
        .unwrap();
        assert_eq!(n.to_string(), r#"{"zulu":1,"alpha":"a"}"#);
    }

    #[test]
    fn equality_is_jsons_and_not_the_orders() {
        assert_eq!(node(r#"{"a":1,"b":2}"#), node(r#"{"b":2,"a":1}"#));
        assert_ne!(node(r#"{"a":1}"#), node(r#"{"a":1,"b":2}"#));
        assert_ne!(node("[1,2]"), node("[2,1]"));
    }

    #[test]
    fn pointers_reach_fields_and_elements_and_escape_their_slashes() {
        let mut doc = node(r#"{"a/b": {"~k": [10, 20, 30]}}"#);
        let p = pointer_of(&["a/b", "~k", "1"]);
        assert_eq!(p, "/a~1b/~0k/1");
        assert_eq!(tokens(&p).unwrap(), ["a/b", "~k", "1"]);
        assert_eq!(doc.at(&p), Some(&node("20")));
        assert_eq!(doc.at(""), Some(&doc.clone()));
        assert_eq!(doc.at("/nope"), None);
        assert_eq!(
            doc.at("/a~1b/~0k/01"),
            None,
            "a leading zero is not an index"
        );
        // Take an element: the ones after move up.
        assert_eq!(doc.take(&p), Some(node("20")));
        assert_eq!(doc.at("/a~1b/~0k").unwrap().to_string(), "[10,30]");
        // Insert puts it back where it was; put REPLACES.
        assert!(doc.insert(&p, node("20")));
        assert_eq!(doc.at("/a~1b/~0k").unwrap().to_string(), "[10,20,30]");
        assert!(doc.put(&p, node("21")));
        assert_eq!(doc.at("/a~1b/~0k").unwrap().to_string(), "[10,21,30]");
        // Past the end, insert appends; put refuses.
        assert!(!doc.put("/a~1b/~0k/9", node("0")));
        assert!(doc.insert("/a~1b/~0k/9", node("40")));
        assert_eq!(doc.at("/a~1b/~0k").unwrap().to_string(), "[10,21,30,40]");
        // A new field goes at the end; no parent, no place.
        assert!(doc.put("/new", node("true")));
        assert!(!doc.put("/missing/child", node("1")));
        assert_eq!(doc.take(""), None, "the whole document is never taken");
        assert_eq!(parent("/a/b/c"), "/a/b");
        assert_eq!(parent("/a"), "");
        assert!(covers("/a", "/a/b") && covers("/a", "/a") && !covers("/a", "/ab"));
        assert!(covers("", "/anything"));
    }

    #[test]
    fn order_follows_the_old_document_and_lists_follow_their_ids() {
        let old = node(r#"{"b": 1, "a": {"y": 1, "x": 2}, "list": [{"id": "p", "k": 1, "j": 2}]}"#);
        let new = node(
            r#"{"a": {"x": 3, "y": 4}, "b": 5, "c": 6, "list": [{"j": 7, "id": "q"}, {"j": 8, "k": 9, "id": "p"}]}"#,
        );
        let out = new.ordered_like(&old);
        assert_eq!(
            out.to_string(),
            r#"{"b":5,"a":{"y":4,"x":3},"list":[{"j":7,"id":"q"},{"id":"p","k":9,"j":8}],"c":6}"#
        );
    }
}
