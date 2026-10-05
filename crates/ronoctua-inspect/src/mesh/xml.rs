//! The two questions every XML reader here asks, and why neither is
//! `roxmltree`'s own.
//!
//! Three of the formats in this module are XML — 3MF, COLLADA and AMF — and
//! all three are written by tools that declare namespaces. Asking `roxmltree`
//! about a name directly means asking about a name in NO namespace, which
//! matches none of them, in every file, silently.
//!
//! And the third, which every one of them shares: the text is parsed through
//! the depth guard (`ronoctua_xml`), at the format's own ceiling.

use ronoctua_xml::roxmltree;

use crate::Refusal;

/// A model's XML, parsed once its nesting is measured under `most`.
///
/// Each reader passes its format's ceiling, measured: `roxmltree` recurses once
/// per level and has no limit of its own, so a document made for it ran the
/// inspector's thread out of stack — which aborts the footer, not a preview.
///
/// # Errors
///
/// [`Refusal::Failed`] for a document deeper than `most` and for one that is
/// not XML: the panel says the same thing for both.
pub fn parse(text: &str, most: usize) -> Result<roxmltree::Document<'_>, Refusal> {
    ronoctua_xml::parse_within(text, roxmltree::ParsingOptions::default(), most)
        .map_err(|_| Refusal::Failed)
}

/// An element by LOCAL name, whatever namespace it is in.
///
/// `roxmltree`'s own `has_tag_name("model")` asks for an element in NO
/// namespace, and every 3MF ever written declares a default one on `<model>` —
/// so that spelling matches nothing at all, in every file, silently. The
/// namespace is not information here: a `<vertex>` is a vertex whichever URI
/// the writer chose, and the extensions are told apart by their attributes.
pub fn tagged(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.tag_name().name() == name
}

/// An attribute by LOCAL name, whatever namespace prefix it carries.
///
/// `p:path` and `path` are the same attribute to this reader, and which prefix
/// a writer chose for the production extension is its own business.
pub fn attr<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == name)
        .map(|a| a.value())
}
