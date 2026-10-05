//! Matroska, through its own tools: `mkvextract` reads the tags, `mkvmerge
//! -J` names the tracks and attachments, `mkvpropedit` edits in place.
//!
//! # Why the tags are read before every write
//!
//! `mkvpropedit --tags global:<file>` REPLACES every global tag with the
//! file's. Writing one field means reading all of them, changing one, and
//! handing all of them back — otherwise an edited title would erase the
//! artist, the ReplayGain and everything a ripper wrote.
//!
//! # The names
//!
//! Matroska's tag names are the specification's (`ARTIST`, `DATE_RELEASED`)
//! where it has one, and ffmpeg's upper-cased metadata keys where it does
//! not (`SEASON_NUMBER`, `EPISODE_SORT`), because ffmpeg is what wrote most
//! Matroska files that exist and what reads them back — a key it maps is a
//! key mpv shows.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ronoctua_xml::roxmltree;

use crate::sheet::{Field, Origin, Tag, Target};

/// One `<Simple>`, with its children.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Simple {
    pub name: String,
    pub string: Option<String>,
    /// Base64, kept as the XML wrote it.
    pub binary: Option<String>,
    pub language: Option<String>,
    pub language_ietf: Option<String>,
    pub default: Option<String>,
    pub children: Vec<Simple>,
}

impl Simple {
    fn text(name: &str, value: &str) -> Self {
        Self {
            name: name.to_owned(),
            string: Some(value.to_owned()),
            ..Self::default()
        }
    }
}

/// What a `<Tag>` targets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Targets {
    pub type_value: Option<u32>,
    pub type_name: Option<String>,
    /// `(element name, uid)`: `TrackUID`, `EditionUID`, `ChapterUID`,
    /// `AttachmentUID`.
    pub uids: Vec<(String, u64)>,
}

impl Targets {
    /// Whether it targets the whole file.
    pub(crate) fn global(&self) -> bool {
        self.uids.is_empty()
    }

    /// Whether it is the block a field lives in: the whole file, at the
    /// default level (50) or none named.
    fn main(&self) -> bool {
        self.global() && self.type_value.is_none_or(|v| v == 50)
    }

    /// The track it targets ALONE — its one UID a `TrackUID` — or `None`.
    pub(crate) fn only_track(&self) -> Option<u64> {
        match self.uids.as_slice() {
            [(kind, uid)] if kind == "TrackUID" => Some(*uid),
            _ => None,
        }
    }

    /// Whether it names `uid`'s track among what it targets.
    fn names_track(&self, uid: u64) -> bool {
        self.uids.iter().any(|(k, u)| k == "TrackUID" && *u == uid)
    }

    fn target(&self) -> Target {
        if self.global() {
            Target::Global
        } else if self.uids.iter().any(|(k, _)| k == "TrackUID") {
            Target::Track
        } else {
            Target::Other
        }
    }
}

/// One `<Tag>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Block {
    pub targets: Targets,
    pub simples: Vec<Simple>,
}

/// Every `<Tag>` of a file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Tags {
    pub blocks: Vec<Block>,
}

/// `mkvextract`'s tags XML, read. An empty answer is a file with no tags.
///
/// # Errors
///
/// [`crate::sheet::Why::Unreadable`] when it is not the XML `mkvextract`
/// writes.
pub(crate) fn parse_tags(xml: &str) -> Result<Tags, crate::sheet::Why> {
    if xml.trim().is_empty() {
        return Ok(Tags::default());
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    // Through the depth guard: the file `mkvextract` read may have come from
    // anywhere, its SimpleTags nest, and both the reader and `simple` below
    // recurse once per level.
    let doc = ronoctua_xml::parse(xml, options).map_err(|_| crate::sheet::Why::Unreadable)?;
    let root = doc.root_element();
    if root.tag_name().name() != "Tags" {
        return Err(crate::sheet::Why::Unreadable);
    }
    let mut out = Tags::default();
    for tag in root.children().filter(|n| n.has_tag_name("Tag")) {
        let mut block = Block::default();
        for child in tag.children().filter(roxmltree::Node::is_element) {
            match child.tag_name().name() {
                "Targets" => block.targets = targets(child),
                "Simple" => block.simples.push(simple(child)),
                _ => {}
            }
        }
        out.blocks.push(block);
    }
    Ok(out)
}

fn text_of(node: roxmltree::Node<'_, '_>) -> Option<String> {
    node.text().map(ToOwned::to_owned)
}

fn targets(node: roxmltree::Node<'_, '_>) -> Targets {
    let mut t = Targets::default();
    for c in node.children().filter(roxmltree::Node::is_element) {
        let name = c.tag_name().name();
        match name {
            "TargetTypeValue" => t.type_value = c.text().and_then(|s| s.trim().parse().ok()),
            "TargetType" => t.type_name = text_of(c),
            _ if name.ends_with("UID") => {
                if let Some(uid) = c.text().and_then(|s| s.trim().parse().ok()) {
                    t.uids.push((name.to_owned(), uid));
                }
            }
            _ => {}
        }
    }
    t
}

fn simple(node: roxmltree::Node<'_, '_>) -> Simple {
    let mut s = Simple::default();
    for c in node.children().filter(roxmltree::Node::is_element) {
        match c.tag_name().name() {
            "Name" => s.name = text_of(c).unwrap_or_default(),
            "String" => s.string = Some(text_of(c).unwrap_or_default()),
            "Binary" => s.binary = Some(text_of(c).unwrap_or_default()),
            "TagLanguage" => s.language = text_of(c),
            "TagLanguageIETF" => s.language_ietf = text_of(c),
            "DefaultLanguage" => s.default = text_of(c),
            "Simple" => s.children.push(simple(c)),
            _ => {}
        }
    }
    s
}

/// The global blocks as the XML `--tags global:` takes.
pub(crate) fn global_xml(tags: &Tags) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE Tags SYSTEM \"matroskatags.dtd\">\n<Tags>\n",
    );
    for block in tags.blocks.iter().filter(|b| b.targets.global()) {
        out.push_str("  <Tag>\n    <Targets>\n");
        if let Some(v) = block.targets.type_value {
            let _ = writeln!(out, "      <TargetTypeValue>{v}</TargetTypeValue>");
        }
        if let Some(n) = &block.targets.type_name {
            let _ = writeln!(out, "      <TargetType>{}</TargetType>", escape(n));
        }
        out.push_str("    </Targets>\n");
        for s in &block.simples {
            write_simple(&mut out, s, 2);
        }
        out.push_str("  </Tag>\n");
    }
    out.push_str("</Tags>\n");
    out
}

fn write_simple(out: &mut String, s: &Simple, depth: usize) {
    let pad = "  ".repeat(depth);
    let _ = writeln!(out, "{pad}<Simple>");
    let _ = writeln!(out, "{pad}  <Name>{}</Name>", escape(&s.name));
    if let Some(v) = &s.string {
        let _ = writeln!(out, "{pad}  <String>{}</String>", escape(v));
    }
    if let Some(v) = &s.binary {
        let _ = writeln!(out, "{pad}  <Binary>{}</Binary>", escape(v));
    }
    if let Some(v) = &s.language {
        let _ = writeln!(out, "{pad}  <TagLanguage>{}</TagLanguage>", escape(v));
    }
    if let Some(v) = &s.language_ietf {
        let _ = writeln!(
            out,
            "{pad}  <TagLanguageIETF>{}</TagLanguageIETF>",
            escape(v)
        );
    }
    if let Some(v) = &s.default {
        let _ = writeln!(
            out,
            "{pad}  <DefaultLanguage>{}</DefaultLanguage>",
            escape(v)
        );
    }
    for c in &s.children {
        write_simple(out, c, depth + 1);
    }
    let _ = writeln!(out, "{pad}</Simple>");
}

/// XML's five, and the control characters XML 1.0 cannot carry at all.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' | '\t' | '\r' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// The names a field is read from; the first is the one written.
const fn names_of(field: Field) -> &'static [&'static str] {
    match field {
        // The segment's own title is what players show; a TITLE tag is read
        // when the segment has none, and kept in step when it exists.
        Field::Title => &["TITLE"],
        Field::Artist => &["ARTIST"],
        Field::Album => &["ALBUM"],
        Field::AlbumArtist => &["ALBUM_ARTIST", "ALBUMARTIST"],
        Field::Track => &["PART_NUMBER", "TRACK"],
        Field::Disc => &["DISC", "DISCNUMBER"],
        Field::Date => &["DATE_RELEASED", "DATE", "DATE_RECORDED", "YEAR"],
        Field::Genre => &["GENRE"],
        Field::Composer => &["COMPOSER"],
        Field::Lyricist => &["LYRICIST"],
        Field::Comment => &["COMMENT"],
        Field::Lyrics => &["LYRICS"],
        Field::Publisher => &["PUBLISHER", "LABEL"],
        Field::Isrc => &["ISRC"],
        Field::Description => &["DESCRIPTION", "SUMMARY", "SYNOPSIS"],
        Field::Show => &["SHOW"],
        Field::Season => &["SEASON_NUMBER"],
        Field::Episode => &["EPISODE_SORT"],
        Field::Director => &["DIRECTOR"],
    }
}

/// The names a write replaces: every spelling but the recording date.
fn replaced_of(field: Field) -> &'static [&'static str] {
    match field {
        Field::Date => &["DATE_RELEASED", "DATE", "YEAR"],
        other => names_of(other),
    }
}

/// A field's value in the file's global tags (the segment title aside).
pub(crate) fn field_value(tags: &Tags, field: Field) -> Option<String> {
    let find = |name: &str| {
        let main = tags.blocks.iter().filter(|b| b.targets.main());
        let other = tags
            .blocks
            .iter()
            .filter(|b| b.targets.global() && !b.targets.main());
        main.chain(other).find_map(|b| {
            b.simples
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .and_then(|s| s.string.clone())
                .filter(|v| !v.trim().is_empty())
        })
    };
    let value = names_of(field).iter().find_map(|n| find(n))?;
    if field == Field::Track
        && !value.contains('/')
        && let Some(total) = find("TOTAL_PARTS")
    {
        return Some(format!("{value}/{total}"));
    }
    Some(value)
}

/// Writes a field into the global tags: every old spelling out of every
/// global block, the new value into the main one (made if missing).
pub(crate) fn set_field(tags: &mut Tags, field: Field, value: Option<&str>) {
    let mut replaced: Vec<&str> = replaced_of(field).to_vec();
    if field == Field::Track {
        replaced.push("TOTAL_PARTS");
    }
    for block in tags.blocks.iter_mut().filter(|b| b.targets.global()) {
        block
            .simples
            .retain(|s| !replaced.iter().any(|n| s.name.eq_ignore_ascii_case(n)));
    }
    drop_empty(tags);
    let Some(value) = value else {
        return;
    };
    let block = main_block(tags);
    if field == Field::Track {
        let (n, t) = value.split_once('/').unwrap_or((value, ""));
        block.simples.push(Simple::text("PART_NUMBER", n));
        if !t.is_empty() {
            block.simples.push(Simple::text("TOTAL_PARTS", t));
        }
    } else {
        block.simples.push(Simple::text(names_of(field)[0], value));
    }
}

/// Writes a tag by its own name into the main global block.
pub(crate) fn set_named(tags: &mut Tags, name: &str, value: Option<&str>) {
    for block in tags.blocks.iter_mut().filter(|b| b.targets.global()) {
        block.simples.retain(|s| !s.name.eq_ignore_ascii_case(name));
    }
    drop_empty(tags);
    if let Some(v) = value {
        main_block(tags).simples.push(Simple::text(name, v));
    }
}

/// Writes a tag by its own name into track `uid`'s tags: out of every
/// block that targets that track alone, the new value into the first of
/// them (made at the default level when the track has none).
pub(crate) fn set_named_track(tags: &mut Tags, uid: u64, name: &str, value: Option<&str>) {
    for block in tags
        .blocks
        .iter_mut()
        .filter(|b| b.targets.only_track() == Some(uid))
    {
        block.simples.retain(|s| !s.name.eq_ignore_ascii_case(name));
    }
    tags.blocks
        .retain(|b| !(b.targets.only_track() == Some(uid) && b.simples.is_empty()));
    let Some(v) = value else { return };
    let at = match tags
        .blocks
        .iter()
        .position(|b| b.targets.only_track() == Some(uid))
    {
        Some(at) => at,
        None => {
            tags.blocks.push(Block {
                targets: Targets {
                    type_value: Some(50),
                    type_name: None,
                    uids: vec![("TrackUID".to_owned(), uid)],
                },
                simples: Vec::new(),
            });
            tags.blocks.len() - 1
        }
    };
    tags.blocks[at].simples.push(Simple::text(name, v));
}

/// Track `uid`'s blocks as the XML `--tags track:<n>:` takes — their
/// targets without the UID, which `mkvpropedit` gives the track it is told
/// — or `None` when the track has no tag left.
pub(crate) fn track_xml(tags: &Tags, uid: u64) -> Option<String> {
    let blocks: Vec<&Block> = tags
        .blocks
        .iter()
        .filter(|b| b.targets.only_track() == Some(uid) && !b.simples.is_empty())
        .collect();
    if blocks.is_empty() {
        return None;
    }
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE Tags SYSTEM \"matroskatags.dtd\">\n<Tags>\n",
    );
    for block in blocks {
        out.push_str("  <Tag>\n    <Targets>\n");
        if let Some(v) = block.targets.type_value {
            let _ = writeln!(out, "      <TargetTypeValue>{v}</TargetTypeValue>");
        }
        if let Some(n) = &block.targets.type_name {
            let _ = writeln!(out, "      <TargetType>{}</TargetType>", escape(n));
        }
        out.push_str("    </Targets>\n");
        for s in &block.simples {
            write_simple(&mut out, s, 2);
        }
        out.push_str("  </Tag>\n");
    }
    out.push_str("</Tags>\n");
    Some(out)
}

/// A chapter's start as Matroska's XML writes it (`HH:MM:SS.nnnnnnnnn`),
/// in milliseconds, rounded.
fn chapter_ms(text: &str) -> Option<u64> {
    let mut parts = text.trim().split(':');
    let h: u64 = parts.next()?.parse().ok()?;
    let m: u64 = parts.next()?.parse().ok()?;
    let rest = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let (s, frac) = rest.split_once('.').unwrap_or((rest, "0"));
    let s: u64 = s.parse().ok()?;
    let mut nanos = frac.chars().take(9).collect::<String>();
    while nanos.len() < 9 {
        nanos.push('0');
    }
    let ns: u64 = nanos.parse().ok()?;
    Some((h * 3600 + m * 60 + s) * 1000 + (ns + 500_000) / 1_000_000)
}

/// `mkvextract`'s chapters XML with every atom that starts at `start_ms`
/// (one in each edition) retitled — its first display's string replaced,
/// or a display added where it has none — and every other byte as it was.
///
/// # Errors
///
/// [`crate::sheet::Why::Unreadable`] for XML that is not the chapters'
/// and [`crate::sheet::Why::NoSuchThing`] when no chapter starts there.
pub(crate) fn retitle_chapters(
    xml: &str,
    start_ms: u64,
    title: &str,
) -> Result<String, crate::sheet::Why> {
    use crate::sheet::Why;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = ronoctua_xml::parse(xml, options).map_err(|_| Why::Unreadable)?;
    if doc.root_element().tag_name().name() != "Chapters" {
        return Err(Why::Unreadable);
    }
    fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
        n.children()
            .find(|c| c.is_element() && c.has_tag_name(name))
    }
    // (where, how much of it goes, what comes in), back to front.
    let mut splices: Vec<(usize, usize, String)> = Vec::new();
    let written = escape(title);
    for atom in doc.descendants().filter(|n| n.has_tag_name("ChapterAtom")) {
        let starts = child(atom, "ChapterTimeStart")
            .and_then(|s| s.text())
            .and_then(chapter_ms);
        if starts.is_none_or(|ms| ms.abs_diff(start_ms) > 1) {
            continue;
        }
        match child(atom, "ChapterDisplay").and_then(|d| child(d, "ChapterString")) {
            Some(string) => {
                let r = string.range();
                splices.push((
                    r.start,
                    r.len(),
                    format!("<ChapterString>{written}</ChapterString>"),
                ));
            }
            None => {
                let r = atom.range();
                let close = xml[r.clone()].rfind("</").ok_or(Why::Unreadable)?;
                splices.push((
                    r.start + close,
                    0,
                    format!(
                        "<ChapterDisplay><ChapterString>{written}</ChapterString></ChapterDisplay>"
                    ),
                ));
            }
        }
    }
    if splices.is_empty() {
        return Err(Why::NoSuchThing);
    }
    splices.sort_by_key(|(at, ..)| std::cmp::Reverse(*at));
    let mut out = xml.to_owned();
    for (at, len, with) in splices {
        out.replace_range(at..at + len, &with);
    }
    Ok(out)
}

/// Whether a file has a global TITLE tag to keep in step with the segment's
/// title.
pub(crate) fn has_title_tag(tags: &Tags) -> bool {
    tags.blocks.iter().filter(|b| b.targets.global()).any(|b| {
        b.simples
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case("TITLE"))
    })
}

fn main_block(tags: &mut Tags) -> &mut Block {
    let at = match tags.blocks.iter().position(|b| b.targets.main()) {
        Some(at) => at,
        None => {
            tags.blocks.push(Block {
                targets: Targets {
                    type_value: Some(50),
                    ..Targets::default()
                },
                simples: Vec::new(),
            });
            tags.blocks.len() - 1
        }
    };
    &mut tags.blocks[at]
}

/// A global block left with nothing in it goes: an empty `<Tag>` is noise
/// every reader skips.
fn drop_empty(tags: &mut Tags) {
    tags.blocks
        .retain(|b| !(b.targets.global() && b.simples.is_empty()));
}

/// Whether any global tag is left.
pub(crate) fn any_global(tags: &Tags) -> bool {
    tags.blocks
        .iter()
        .any(|b| b.targets.global() && !b.simples.is_empty())
}

/// Whether the tags of track `uid` can be handed back whole: every block
/// that names it names it ALONE — `--tags track:<n>:` replaces the track's
/// tags, and one that also targets a chapter would come back as the track's
/// only.
fn track_is_whole(tags: &Tags, uid: u64) -> bool {
    tags.blocks
        .iter()
        .filter(|b| b.targets.names_track(uid))
        .all(|b| b.targets.only_track() == Some(uid))
}

/// Every tag, by its own name, for the Every-tag page — a global one
/// written as `can_global` says, one of a single track as `can_track` says
/// (its writer also needs `mkvmerge` to find the track by its UID).
pub(crate) fn listing(
    tags: &Tags,
    can_global: &crate::sheet::Can,
    can_track: &crate::sheet::Can,
) -> Vec<Tag> {
    use crate::sheet::{Can, TagValue, Why};
    let mut out = Vec::new();
    for block in &tags.blocks {
        let target = block.targets.target();
        // A TAG OF ONE TRACK ALONE is written into that track by its UID.
        let track = block
            .targets
            .only_track()
            .filter(|uid| track_is_whole(tags, *uid));
        let can_here = if track.is_some() {
            can_track
        } else {
            can_global
        };
        let mut stack: Vec<&Simple> = block.simples.iter().rev().collect();
        while let Some(s) = stack.pop() {
            let value = match (&s.string, &s.binary) {
                (Some(text), _) => crate::native::text_value(text),
                // Base64 is four characters for three bytes.
                (None, Some(b64)) => TagValue::Binary {
                    bytes: (b64.trim().len() as u64 / 4) * 3,
                },
                (None, None) => TagValue::Text(String::new()),
            };
            let top = block.simples.iter().any(|t| std::ptr::eq(t, s));
            // A TAG OF THE WHOLE FILE, on its own: `--tags global:` hands
            // every other one back as it was read. One nested in another, or
            // holding others, would lose them in the replace; a track's,
            // chapter's or attachment's is not global at all.
            let origin = Origin::Matroska { target, track };
            let can = if (target != Target::Global && track.is_none())
                || !top
                || !s.children.is_empty()
            {
                Can::No(Why::NotGlobal)
            } else if s.string.is_none() {
                Can::No(Why::NotText)
            } else {
                match crate::sheet::tag_name(&origin, &s.name) {
                    Ok(_) => can_here.clone(),
                    Err(why) => Can::No(why),
                }
            };
            out.push(Tag {
                origin,
                name: s.name.clone(),
                value,
                can,
            });
            stack.extend(s.children.iter().rev());
        }
    }
    out
}

/// `mkvmerge -J`, the parts a sheet uses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Identified {
    /// The segment's title.
    pub title: Option<String>,
    /// The attachments.
    pub attachments: Vec<Attachment>,
    /// Every track's UID, in the order `track:<n>` counts them.
    pub tracks: Vec<u64>,
}

/// Each track of `streams` given its UID, `uids` being mkvmerge's in the
/// order `track:<n>` counts them: ffprobe lists a Matroska file's tracks in
/// that same order, and its attachments (a cover among them) after. When the
/// two do not count the same tracks, no stream is named — a wrong name would
/// put one track's tags under another's.
pub(crate) fn name_tracks(streams: &mut [crate::sheet::Stream], uids: &[u64]) {
    let mut tracks: Vec<&mut crate::sheet::Stream> = streams
        .iter_mut()
        .filter(|s| s.kind != crate::sheet::StreamKind::Attachment && !s.attached)
        .collect();
    if tracks.len() != uids.len() {
        return;
    }
    for (stream, uid) in tracks.iter_mut().zip(uids) {
        stream.uid = Some(*uid);
    }
}

/// One attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attachment {
    pub file_name: String,
    pub content_type: Option<String>,
    pub size: u64,
}

/// `mkvmerge -J`'s JSON, read.
///
/// # Errors
///
/// [`crate::sheet::Why::Unreadable`] when it is not that JSON.
pub(crate) fn parse_identified(text: &[u8]) -> Result<Identified, crate::sheet::Why> {
    let doc: serde_json::Value =
        serde_json::from_slice(text).map_err(|_| crate::sheet::Why::Unreadable)?;
    let title = doc
        .pointer("/container/properties/title")
        .and_then(serde_json::Value::as_str)
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned);
    let attachments = doc
        .get("attachments")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some(Attachment {
                file_name: a.get("file_name")?.as_str()?.to_owned(),
                content_type: a
                    .get("content_type")
                    .and_then(serde_json::Value::as_str)
                    .map(ToOwned::to_owned),
                size: a
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            })
        })
        .collect();
    let tracks = doc
        .get("tracks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|t| {
            t.pointer("/properties/uid")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        })
        .collect();
    Ok(Identified {
        title,
        attachments,
        tracks,
    })
}

/// Whether an attachment is a FRONT cover, by the names Matroska's own
/// convention gives them (`cover.jpg`, `cover.png`; `small_cover` and
/// `cover_land` are other sizes of it and are not what "the cover" means).
pub(crate) fn is_front_cover(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.rsplit_once('.').map_or(lower.as_str(), |(s, _)| s);
    stem == "cover"
}

/// What one `mkvpropedit` run does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Edit {
    /// The segment title: `Some(None)` deletes it.
    pub title: Option<Option<String>>,
    /// The global tags file; `Some(None)` removes every global tag.
    pub tags: Option<Option<PathBuf>>,
    /// Per track, by its 1-based ORDER (`track:n`), its tags file — `None`
    /// removes every tag of that track.
    pub track_tags: Vec<(u32, Option<PathBuf>)>,
    /// Per track, by its 1-based ORDER (`track:n`), the properties.
    pub tracks: Vec<(u32, Vec<Property>)>,
    /// The chapters file `--chapters` replaces every chapter with.
    pub chapters: Option<PathBuf>,
    /// Attachments to delete, by name.
    pub delete_attachments: Vec<String>,
    /// An attachment to add: path, name, MIME type.
    pub add_attachment: Option<(PathBuf, String, String)>,
}

/// A track property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Property {
    /// `--set language=`.
    Language(String),
    /// `--set name=`, or `--delete name` for `None`.
    Name(Option<String>),
    /// `--set flag-default=`.
    Default(bool),
    /// `--set flag-forced=`.
    Forced(bool),
}

impl Edit {
    /// Whether it does nothing.
    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// `mkvpropedit`'s argument list.
///
/// Every value is its own argument, so nothing is ever read by a shell;
/// `mkvpropedit` splits `title=…` at the FIRST `=`, so a title with `=` in
/// it survives.
pub(crate) fn propedit_args(draft: &Path, edit: &Edit) -> Vec<OsString> {
    fn push(a: &mut Vec<OsString>, s: &str) {
        a.push(OsString::from(s));
    }
    let mut a: Vec<OsString> = vec![draft.as_os_str().to_owned()];
    match &edit.title {
        Some(Some(t)) => {
            for s in ["--edit", "info", "--set", &format!("title={t}")] {
                push(&mut a, s);
            }
        }
        Some(None) => {
            for s in ["--edit", "info", "--delete", "title"] {
                push(&mut a, s);
            }
        }
        None => {}
    }
    for (n, props) in &edit.tracks {
        push(&mut a, "--edit");
        push(&mut a, &format!("track:{n}"));
        for p in props {
            let (verb, what) = match p {
                Property::Language(l) => ("--set", format!("language={l}")),
                Property::Name(Some(t)) => ("--set", format!("name={t}")),
                Property::Name(None) => ("--delete", "name".to_owned()),
                Property::Default(b) => ("--set", format!("flag-default={}", u8::from(*b))),
                Property::Forced(b) => ("--set", format!("flag-forced={}", u8::from(*b))),
            };
            push(&mut a, verb);
            push(&mut a, &what);
        }
    }
    if let Some(path) = &edit.chapters {
        push(&mut a, "--chapters");
        a.push(path.as_os_str().to_owned());
    }
    for name in &edit.delete_attachments {
        push(&mut a, "--delete-attachment");
        push(&mut a, &format!("name:{name}"));
    }
    match &edit.tags {
        Some(Some(path)) => {
            let mut arg = OsString::from("global:");
            arg.push(path.as_os_str());
            push(&mut a, "--tags");
            a.push(arg);
        }
        Some(None) => {
            push(&mut a, "--tags");
            push(&mut a, "global:");
        }
        None => {}
    }
    for (n, file) in &edit.track_tags {
        let mut arg = OsString::from(format!("track:{n}:"));
        if let Some(path) = file {
            arg.push(path.as_os_str());
        }
        push(&mut a, "--tags");
        a.push(arg);
    }
    if let Some((path, name, mime)) = &edit.add_attachment {
        for s in [
            "--attachment-name",
            name,
            "--attachment-mime-type",
            mime,
            "--add-attachment",
        ] {
            push(&mut a, s);
        }
        a.push(path.as_os_str().to_owned());
    }
    a
}

#[cfg(test)]
mod tests {
    use super::{
        Edit, Property, field_value, global_xml, is_front_cover, listing, parse_identified,
        parse_tags, propedit_args, set_field, set_named,
    };
    use crate::sheet::{Can, Field, Origin, Target, Why, Writer};
    use std::path::{Path, PathBuf};

    /// FAILS IF: a stream is given another track's UID — ffprobe's
    /// attachments and covers counted as tracks — or any stream is named
    /// when mkvmerge counts other tracks than ffprobe lists.
    #[test]
    fn each_track_is_named_by_its_uid_and_no_other() {
        use crate::sheet::{Stream, StreamKind};
        let stream = |index: u32, kind: StreamKind, attached: bool| Stream {
            index,
            kind,
            codec: None,
            lang: None,
            title: None,
            default: false,
            forced: false,
            hearing: false,
            attached,
            can: Can::No(Why::AttachmentIsNotAStream),
            uid: None,
        };
        let mut streams = vec![
            stream(0, StreamKind::Video, false),
            stream(1, StreamKind::Audio, false),
            stream(2, StreamKind::Subtitle, false),
            stream(3, StreamKind::Video, true),
            stream(4, StreamKind::Attachment, false),
        ];
        super::name_tracks(&mut streams, &[11, 22, 33]);
        let uids: Vec<Option<u64>> = streams.iter().map(|s| s.uid).collect();
        assert_eq!(uids, [Some(11), Some(22), Some(33), None, None]);
        let mut streams = vec![
            stream(0, StreamKind::Video, false),
            stream(1, StreamKind::Audio, false),
        ];
        super::name_tracks(&mut streams, &[11, 22, 33]);
        assert!(streams.iter().all(|s| s.uid.is_none()));
    }

    /// What `mkvextract … tags` writes for a file ffmpeg made and a ripper
    /// touched: a global block, a track's block, a nested simple.
    const XML: &str = r#"<?xml version="1.0"?>
<!-- <!DOCTYPE Tags SYSTEM "matroskatags.dtd"> -->
<Tags>
  <Tag>
    <Targets />
    <Simple><Name>ARTIST</Name><String>Someone &amp; Band</String></Simple>
    <Simple><Name>PART_NUMBER</Name><String>3</String></Simple>
    <Simple><Name>TOTAL_PARTS</Name><String>12</String></Simple>
    <Simple><Name>REPLAYGAIN_TRACK_GAIN</Name><String>-6.1 dB</String>
      <Simple><Name>NOTE</Name><String>measured</String></Simple>
    </Simple>
  </Tag>
  <Tag>
    <Targets><TargetTypeValue>70</TargetTypeValue></Targets>
    <Simple><Name>DATE</Name><String>1999</String></Simple>
  </Tag>
  <Tag>
    <Targets><TrackUID>1234567</TrackUID></Targets>
    <Simple><Name>BPS</Name><String>128000</String></Simple>
  </Tag>
</Tags>"#;

    #[test]
    fn the_tags_are_read_with_their_targets() {
        let tags = parse_tags(XML).expect("it is mkvextract's XML");
        assert_eq!(tags.blocks.len(), 3);
        assert!(tags.blocks[0].targets.global());
        assert!(!tags.blocks[2].targets.global());
        assert_eq!(
            field_value(&tags, Field::Artist).as_deref(),
            Some("Someone & Band")
        );
        assert_eq!(field_value(&tags, Field::Track).as_deref(), Some("3/12"));
        assert_eq!(
            field_value(&tags, Field::Date).as_deref(),
            Some("1999"),
            "a collection's date still counts"
        );
        assert_eq!(field_value(&tags, Field::Genre), None);
        assert!(parse_tags("").expect("no tags").blocks.is_empty());
        assert!(parse_tags("<Chapters/>").is_err());
    }

    /// FAILS IF: SimpleTags nested past the depth guard reach the parser or
    /// `simple`, which both recurse once per level — the file `mkvextract`
    /// read may have come from anywhere.
    #[test]
    fn simples_nested_past_the_guard_are_unreadable_and_not_a_crash() {
        let depth = 100_000;
        let mut xml = String::from("<Tags><Tag><Targets />");
        xml.push_str(&"<Simple><Name>X</Name>".repeat(depth));
        xml.push_str(&"</Simple>".repeat(depth));
        xml.push_str("</Tag></Tags>");
        assert_eq!(parse_tags(&xml).err(), Some(Why::Unreadable));
    }

    #[test]
    fn a_write_keeps_every_tag_it_did_not_touch() {
        let mut tags = parse_tags(XML).expect("parsed");
        set_field(&mut tags, Field::Genre, Some("Jazz"));
        set_field(&mut tags, Field::Track, Some("4/10"));
        set_field(&mut tags, Field::Date, Some("2001-05-02"));
        let again = parse_tags(&global_xml(&tags)).expect("our XML reads back");
        assert_eq!(field_value(&again, Field::Genre).as_deref(), Some("Jazz"));
        assert_eq!(field_value(&again, Field::Track).as_deref(), Some("4/10"));
        assert_eq!(
            field_value(&again, Field::Date).as_deref(),
            Some("2001-05-02")
        );
        assert_eq!(
            field_value(&again, Field::Artist).as_deref(),
            Some("Someone & Band")
        );
        let rg = again.blocks[0]
            .simples
            .iter()
            .find(|s| s.name == "REPLAYGAIN_TRACK_GAIN")
            .expect("the ripper's tag is still there");
        assert_eq!(rg.children.len(), 1, "with its child");
        // The collection block lost its DATE (one date per file) and, empty,
        // went; the track's block is not global and is not in the file.
        assert!(
            again
                .blocks
                .iter()
                .all(|b| b.targets.type_value != Some(70))
        );
        assert!(again.blocks.iter().all(|b| b.targets.global()));
    }

    #[test]
    fn clearing_the_last_tag_leaves_no_empty_block() {
        let mut tags = parse_tags(XML).expect("parsed");
        for f in [Field::Artist, Field::Track, Field::Date] {
            set_field(&mut tags, f, None);
        }
        set_named(&mut tags, "REPLAYGAIN_TRACK_GAIN", None);
        assert!(!super::any_global(&tags));
    }

    /// FAILS IF: a tag the global replace would lose something through —
    /// a child, a parent with children, a track's — is offered, or one that
    /// is not offered stops saying why in its own words.
    #[test]
    fn only_a_global_top_level_text_tag_is_editable() {
        let tags = parse_tags(XML).expect("parsed");
        let no_track = Can::No(Why::NotGlobal);
        let rows = listing(&tags, &Can::Yes(Writer::Matroska), &no_track);
        let row = |name: &str| rows.iter().find(|r| r.name == name).expect(name);
        assert_eq!(row("ARTIST").can, Can::Yes(Writer::Matroska));
        assert_eq!(
            row("DATE").can,
            Can::Yes(Writer::Matroska),
            "global, level 70"
        );
        assert_eq!(row("NOTE").can, Can::No(Why::NotGlobal), "a child");
        assert_eq!(
            row("REPLAYGAIN_TRACK_GAIN").can,
            Can::No(Why::NotGlobal),
            "a parent: the replace would lose its child"
        );
        assert_eq!(
            row("BPS").can,
            no_track,
            "a track's, as its track's writer says"
        );
        let missing = Can::No(Why::Missing {
            program: crate::sheet::Program::Mkvpropedit,
        });
        let rows = listing(&tags, &missing, &no_track);
        assert_eq!(
            rows.iter()
                .find(|r| r.name == "ARTIST")
                .map(|r| r.can.clone()),
            Some(missing),
            "the file's own refusal, where the tag itself would do"
        );
        assert_eq!(
            row("BPS").origin,
            Origin::Matroska {
                target: Target::Track,
                track: Some(1_234_567),
            }
        );
    }

    /// FAILS IF: a tag of one track says it is not written, where its track
    /// can be found by its UID — or one that also targets something else is
    /// offered, which `--tags track:` would hand back as the track's alone.
    #[test]
    fn a_track_s_own_tag_is_editable_by_its_uid() {
        let tags = parse_tags(XML).expect("parsed");
        let rows = listing(
            &tags,
            &Can::Yes(Writer::Matroska),
            &Can::Yes(Writer::Matroska),
        );
        let bps = rows.iter().find(|r| r.name == "BPS").expect("BPS");
        assert_eq!(bps.can, Can::Yes(Writer::Matroska));
        let shared = XML.replace(
            "<Targets><TrackUID>1234567</TrackUID></Targets>\n    <Simple><Name>BPS",
            "<Targets><TrackUID>1234567</TrackUID></Targets>\n    <Simple><Name>BPS</Name><String>1</String></Simple>\n  </Tag>\n  <Tag>\n    <Targets><TrackUID>1234567</TrackUID><ChapterUID>9</ChapterUID></Targets>\n    <Simple><Name>X",
        );
        let tags = parse_tags(&shared).expect("parsed");
        let rows = listing(
            &tags,
            &Can::Yes(Writer::Matroska),
            &Can::Yes(Writer::Matroska),
        );
        let bps = rows.iter().find(|r| r.name == "BPS").expect("BPS");
        assert_eq!(
            bps.can,
            Can::No(Why::NotGlobal),
            "the track's tags are not whole"
        );
        assert_eq!(
            bps.origin,
            Origin::Matroska {
                target: Target::Track,
                track: None,
            }
        );
    }

    /// FAILS IF: a track's tag is written into another block — the global
    /// ones, another track's — or the XML handed for `track:` keeps a UID,
    /// or a track left with no tag is not said to be emptied.
    #[test]
    fn a_track_s_tags_are_written_apart_from_the_global_ones() {
        let mut tags = parse_tags(XML).expect("parsed");
        let before_global = global_xml(&tags);
        super::set_named_track(&mut tags, 1_234_567, "BPS", Some("256000"));
        super::set_named_track(&mut tags, 1_234_567, "MOOD", Some("calm"));
        assert_eq!(
            global_xml(&tags),
            before_global,
            "the global tags untouched"
        );
        let xml = super::track_xml(&tags, 1_234_567).expect("the track's tags");
        assert!(
            xml.contains("<Name>BPS</Name>") && xml.contains("256000"),
            "{xml}"
        );
        assert!(xml.contains("<Name>MOOD</Name>"), "{xml}");
        assert!(!xml.contains("UID"), "mkvpropedit gives the UID: {xml}");
        // A TRACK WITH NO TAG YET gets a block of its own.
        super::set_named_track(&mut tags, 42, "LANGUAGE_NOTE", Some("dub"));
        assert!(super::track_xml(&tags, 42).is_some());
        // EMPTIED: nothing to hand back.
        super::set_named_track(&mut tags, 42, "LANGUAGE_NOTE", None);
        assert_eq!(super::track_xml(&tags, 42), None);
        let edit = Edit {
            track_tags: vec![(2, Some(PathBuf::from("/run/t2.xml"))), (3, None)],
            ..Edit::default()
        };
        let args: Vec<String> = propedit_args(Path::new("f"), &edit)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args[1..],
            ["--tags", "track:2:/run/t2.xml", "--tags", "track:3:"]
        );
    }

    /// FAILS IF: a chapter is retitled by its place instead of where it
    /// starts, another chapter's title or any other byte moves, a chapter
    /// with no display gets none, or a start no chapter has is taken.
    #[test]
    fn a_chapter_is_retitled_by_where_it_starts() {
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE Chapters SYSTEM "matroskachapters.dtd">
<Chapters>
  <EditionEntry>
    <ChapterAtom>
      <ChapterUID>1</ChapterUID>
      <ChapterTimeStart>00:00:00.000000000</ChapterTimeStart>
      <ChapterDisplay><ChapterString>Opening</ChapterString><ChapterLanguage>eng</ChapterLanguage></ChapterDisplay>
    </ChapterAtom>
    <ChapterAtom>
      <ChapterUID>2</ChapterUID>
      <ChapterTimeStart>00:01:01.500000000</ChapterTimeStart>
    </ChapterAtom>
  </EditionEntry>
</Chapters>"#;
        let out = super::retitle_chapters(xml, 0, "The <Harbour> & sea").expect("retitled");
        assert!(
            out.contains(
                "<ChapterString>The &lt;Harbour&gt; &amp; sea</ChapterString><ChapterLanguage>eng"
            ),
            "{out}"
        );
        assert_eq!(
            out.matches("<ChapterString>").count(),
            1,
            "the other untouched"
        );
        let out = super::retitle_chapters(&out, 61_500, "Second").expect("one with no display");
        assert!(
            out.contains("<ChapterDisplay><ChapterString>Second</ChapterString></ChapterDisplay>"),
            "{out}"
        );
        assert!(
            out.contains("<ChapterUID>2</ChapterUID>"),
            "every other byte kept"
        );
        assert_eq!(
            super::retitle_chapters(xml, 5_000, "x"),
            Err(Why::NoSuchThing)
        );
        assert_eq!(super::chapter_ms("01:02:03.4"), Some(3_723_400));
        let edit = Edit {
            chapters: Some(PathBuf::from("/run/c.xml")),
            ..Edit::default()
        };
        let args: Vec<String> = propedit_args(Path::new("f"), &edit)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[1..], ["--chapters", "/run/c.xml"]);
    }

    #[test]
    fn mkvpropedit_gets_one_argument_per_value() {
        let edit = Edit {
            title: Some(Some("a = b".to_owned())),
            tags: Some(Some(PathBuf::from("/run/t.xml"))),
            track_tags: Vec::new(),
            chapters: None,
            tracks: vec![(
                2,
                vec![
                    Property::Language("pt-BR".to_owned()),
                    Property::Name(None),
                    Property::Forced(true),
                ],
            )],
            delete_attachments: vec!["cover.jpg".to_owned()],
            add_attachment: Some((
                PathBuf::from("/run/c.png"),
                "cover.png".to_owned(),
                "image/png".to_owned(),
            )),
        };
        let args: Vec<String> = propedit_args(Path::new("/m/.x.part"), &edit)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "/m/.x.part",
                "--edit",
                "info",
                "--set",
                "title=a = b",
                "--edit",
                "track:2",
                "--set",
                "language=pt-BR",
                "--delete",
                "name",
                "--set",
                "flag-forced=1",
                "--delete-attachment",
                "name:cover.jpg",
                "--tags",
                "global:/run/t.xml",
                "--attachment-name",
                "cover.png",
                "--attachment-mime-type",
                "image/png",
                "--add-attachment",
                "/run/c.png",
            ]
        );
        let clear = Edit {
            tags: Some(None),
            ..Edit::default()
        };
        let args = propedit_args(Path::new("f"), &clear);
        assert_eq!(args[1..], ["--tags", "global:"]);
        assert!(Edit::default().is_empty());
    }

    #[test]
    fn mkvmerge_names_the_title_and_the_attachments() {
        let json = br#"{"container":{"properties":{"title":"A Film"},"type":"Matroska"},
          "attachments":[{"id":0,"file_name":"cover.jpg","content_type":"image/jpeg","size":51234},
                         {"id":1,"file_name":"font.ttf","content_type":"font/ttf","size":9}],
          "tracks":[{"id":0,"type":"video","properties":{"uid":11}},
                    {"id":1,"type":"audio","properties":{"uid":1234567}}]}"#;
        let id = parse_identified(json).expect("parsed");
        assert_eq!(
            id.tracks,
            vec![11, 1_234_567],
            "in the order track:n counts"
        );
        assert_eq!(id.title.as_deref(), Some("A Film"));
        assert_eq!(id.attachments.len(), 2);
        assert!(is_front_cover(&id.attachments[0].file_name));
        assert!(!is_front_cover("small_cover.jpg"));
        assert!(!is_front_cover("font.ttf"));
        assert!(is_front_cover("COVER.PNG"));
    }

    #[test]
    fn xml_that_would_break_the_file_is_escaped() {
        let mut tags = super::Tags::default();
        set_named(&mut tags, "COMMENT", Some("<b>&\"'\u{1}</b>"));
        let again = parse_tags(&global_xml(&tags)).expect("it still parses");
        assert_eq!(
            field_value(&again, Field::Comment).as_deref(),
            Some("<b>&\"'</b>")
        );
    }
}
