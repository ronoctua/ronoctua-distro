//! The two families read a line at a time: markdown and diff.
//!
//! Both are line-oriented in a way code is not — what a line MEANS is decided
//! by how it starts — and both would be wrong if they went through the code
//! scanner, which would colour `+` and `#` as punctuation and prose as prose.

use alloc::vec::Vec;

use crate::{Grammar, Open, Range, Role, State, of_tag, push, scan};

/// One line of markdown.
///
/// # A fence is highlighted by the language it names
///
/// Which is what a fence is FOR: it says "this part is not prose". So the line
/// inside one is handed to that language's own grammar, carrying its own open
/// state in [`State::inner`] — one level, because a fence cannot contain a
/// fence.
///
/// And a fence whose info string names nothing this crate knows gets **no
/// highlighting at all**. Falling back to a default would colour a shell
/// session as if it were Rust, which is worse than leaving it alone — the
/// prose rule, one level down.
pub(crate) fn markdown(text: &str, grammar: &Grammar, state: State, out: &mut Vec<Range>) -> State {
    // INSIDE A FENCE, and the close is tested BEFORE the content: a line that
    // closes the fence is the fence's line, not the inner language's.
    if let Open::Fence {
        marker,
        length,
        inner,
    } = state.outer
    {
        let opener = grammar
            .fences
            .get(usize::from(marker))
            .copied()
            .unwrap_or("```");
        if closes_fence(text, opener, length) {
            push(out, 0, text.len(), Role::Attribute);
            return State {
                outer: Open::Nothing,
                inner: Open::Nothing,
            };
        }
        let Some(language) = inner else {
            return state;
        };
        let carried = scan::code(
            text,
            language.grammar(),
            State {
                outer: state.inner,
                inner: Open::Nothing,
            },
            out,
        );
        return State {
            outer: state.outer,
            inner: carried.outer,
        };
    }

    // A COMMENT ACROSS LINES, which markdown borrows from HTML.
    if let Open::Comment { .. } = state.outer {
        return scan::markup(text, grammar, state, out);
    }

    let trimmed = text.trim_start();
    let lead = text.len() - trimmed.len();

    // A FENCE OPENS, and its own line carries the marker AND the info string:
    // both are about the block rather than in it.
    if let Some((n, opener)) = grammar
        .fences
        .iter()
        .enumerate()
        .find(|(_, f)| trimmed.starts_with(**f))
    {
        let ch = opener.chars().next().unwrap_or('`');
        let run = trimmed.chars().take_while(|c| *c == ch).count();
        let info = trimmed[run..].trim();
        // THE FIRST WORD ONLY: ```rust,ignore and ```js {highlight} both name
        // one language and then say something else about the block.
        let named = info
            .split(|c: char| c == ',' || c.is_whitespace())
            .next()
            .filter(|s| !s.is_empty())
            .and_then(of_tag);
        push(out, 0, text.len(), Role::Attribute);
        return State {
            outer: Open::Fence {
                marker: u8::try_from(n).unwrap_or(0),
                length: u8::try_from(run).unwrap_or(3),
                inner: named,
            },
            inner: Open::Nothing,
        };
    }

    // A HEADING. Setext headings — the `====` underline — are deliberately
    // absent: they need the PREVIOUS line's content, and the whole design here
    // is "one line plus eight bytes".
    if trimmed.starts_with('#') {
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
            push(out, 0, text.len(), Role::Heading);
            return state;
        }
    }

    // A QUOTE IS NOT THE AUTHOR'S VOICE, which is what `Comment` means here.
    if trimmed.starts_with('>') {
        push(out, lead, text.len(), Role::Comment);
        return state;
    }

    // A REFERENCE DEFINITION: `[name]: https://…` — a key and its value.
    if trimmed.starts_with('[')
        && let Some(close) = trimmed.find("]:")
    {
        push(out, lead, lead + close + 2, Role::Property);
        push(out, lead + close + 2, text.len(), Role::String);
        return state;
    }

    inline(text, out);
    state
}

/// The marks that live inside a line of prose.
///
/// Only inline code, and only that: `**bold**` already carries its own visible
/// markers, and `ronoctua_render::Span` has no weight to draw emphasis with —
/// so "emphasis" could only be a hue, and half a mechanism is not a mechanism.
fn inline(text: &str, out: &mut Vec<Range>) {
    let bytes = text.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] == b'`' {
            let rest = &text[at + 1..];
            if let Some(end) = rest.find('`') {
                push(out, at, at + 1 + end + 1, Role::String);
                at = at + 1 + end + 1;
                continue;
            }
        }
        at += 1;
        while at < bytes.len() && (bytes[at] & 0xC0) == 0x80 {
            at += 1;
        }
    }
}

/// Does this line close a fence opened with `run` of `opener`'s character?
fn closes_fence(text: &str, opener: &str, run: u8) -> bool {
    let Some(ch) = opener.chars().next() else {
        return false;
    };
    let trimmed = text.trim();
    !trimmed.is_empty()
        && trimmed.chars().all(|c| c == ch)
        && trimmed.chars().count() >= usize::from(run)
}

/// One line of a diff or a patch.
///
/// # The order of these tests is the whole of it
///
/// `--- a/x` and `+++ b/x` are file headers, and they start with the same
/// characters a removed and an added line do. Testing `+` and `-` first paints
/// the header of every hunk as if it were content, in a file whose entire
/// purpose is telling those two apart.
pub(crate) fn diff(text: &str, out: &mut Vec<Range>) {
    const STRUCTURE: [&str; 10] = [
        "diff ",
        "index ",
        "--- ",
        "+++ ",
        "@@",
        "old mode",
        "new mode",
        "similarity",
        "rename ",
        "Binary files",
    ];
    if STRUCTURE.iter().any(|m| text.starts_with(m)) {
        push(out, 0, text.len(), Role::Heading);
        return;
    }
    match text.as_bytes().first() {
        Some(b'+') => push(out, 0, text.len(), Role::Added),
        Some(b'-') => push(out, 0, text.len(), Role::Removed),
        // `\ No newline at end of file` is the diff format talking about
        // itself, which is what a comment is.
        Some(b'\\') => push(out, 0, text.len(), Role::Comment),
        _ => {}
    }
}
