//! Text out of the bits of HTML the providers answer: Wiktionary's
//! definitions are HTML fragments, and MyMemory escapes an apostrophe as an
//! entity. Pure.
//!
//! Not an HTML parser, and it does not need to be one: the input is a
//! fragment a host rendered from wikitext, and what is wanted is its words.
//! Tags go, the five named entities and numeric ones are read, and runs of
//! white space become one space.

/// The words of an HTML fragment: tags removed, entities read, white space
/// folded, trimmed.
#[must_use]
pub fn text_of(fragment: &str) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut rest = fragment;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        rest = match rest[open..].find('>') {
            Some(close) => &rest[open + close + 1..],
            None => "",
        };
    }
    out.push_str(rest);
    folded(&entities(&out))
}

/// The fragment up to its first nested list: Wiktionary folds a sense's
/// sub-senses into the definition as `<ol><li>…`, and those come again as
/// definitions of their own.
#[must_use]
pub fn before_a_list(fragment: &str) -> &str {
    ["<ol", "<ul", "<dl"]
        .iter()
        .filter_map(|tag| fragment.find(tag))
        .min()
        .map_or(fragment, |cut| &fragment[..cut])
}

/// Entities read: `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;` and
/// numeric ones, decimal or hexadecimal. An entity it does not know is kept
/// as written.
#[must_use]
pub fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        // An entity is short: a `;` further than twelve bytes on is prose.
        let Some(semi) = tail
            .char_indices()
            .take_while(|(i, _)| *i < 12)
            .find_map(|(i, c)| (c == ';').then_some(i))
        else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let name = &tail[1..semi];
        let read = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match read {
            Some(c) => {
                out.push(c);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Runs of white space as one space, trimmed.
fn folded(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wiktionary_definition_is_its_words() {
        let d = r#"<a rel="mw:WikiLink" href="/wiki/home" title="home">home</a> <span class="mention-gloss-paren">(</span><span class="mention-gloss">one’s own dwelling place</span><span class="mention-gloss-paren">)</span>"#;
        assert_eq!(text_of(d), "home (one’s own dwelling place)");
        let nested = "<a>house</a>\n<ol><li>structure serving as an abode";
        assert_eq!(text_of(before_a_list(nested)), "house");
    }

    #[test]
    fn entities_are_read_and_an_unknown_one_is_kept() {
        assert_eq!(
            entities("it&#39;s &amp; &#x263A; &lt;b&gt;"),
            "it's & ☺ <b>"
        );
        assert_eq!(entities("AT&T &bogus; &"), "AT&T &bogus; &");
    }
}
