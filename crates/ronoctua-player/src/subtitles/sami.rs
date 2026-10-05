//! Microsoft SAMI (`.smi`): HTML with a `<SYNC Start=ms>` before every
//! change of text.
//!
//! ```text
//! <SAMI><BODY>
//! <SYNC Start=1000><P Class=ENCC>Hello<br>world
//! <SYNC Start=3000><P Class=ENCC>&nbsp;
//! </BODY></SAMI>
//! ```
//!
//! A cue lasts until the next `SYNC`; one that holds only a space clears the
//! screen.
//!
//! # Several languages
//!
//! A file can carry several languages, one `Class` each, declared in its
//! `<STYLE>` (`.ENCC {Name: English; lang: en-US;}`). A lane shows ONE: the
//! one its [`Lane::lang`](crate::own::Lane) names — matched against each
//! class's `lang`, then its name ([`chosen`]) — and with none named, the
//! first class that says something. [`languages`] are what a door offers to
//! choose among, by the codes a lane keeps.

use crate::subtitles::cue::{self, Cue, Dialect};

/// One class a SAMI declares in its `<STYLE>`: a language of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Its name, lowercase, as a `<P Class=…>` names it (`encc`).
    pub name: String,
    /// The language it declares (`en-US`), when it does.
    pub lang: Option<String>,
}

impl Class {
    /// The code a lane keeps for it: its language, or its name when it
    /// declares none.
    #[must_use]
    pub fn code(&self) -> &str {
        self.lang.as_deref().unwrap_or(&self.name)
    }
}

/// The classes a SAMI's `<STYLE>` declares, in its order: every `.NAME {…}`
/// rule, with the `lang:` inside it. A class used by a paragraph and never
/// declared is not one a person can name, and is left out; a `P` rule or a
/// `#id` rule is no class.
#[must_use]
pub fn classes(text: &str) -> Vec<Class> {
    let lower = text.to_ascii_lowercase();
    let Some(start) = lower.find("<style") else {
        return Vec::new();
    };
    let end = lower[start..]
        .find("</style")
        .map_or(lower.len(), |e| start + e);
    let style = &text[start..end];
    // RULE BY RULE, `selector { body }`: a dot inside a body (`1.5em`) is no
    // class.
    let mut out: Vec<Class> = Vec::new();
    for rule in style.split('}') {
        let Some((selector, body)) = rule.split_once('{') else {
            continue;
        };
        let selector = selector
            .rsplit(['>', '\n', ';'])
            .next()
            .unwrap_or(selector)
            .trim()
            .trim_start_matches("<!--")
            .trim();
        let Some(name) = selector.strip_prefix('.') else {
            continue;
        };
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        let lang = body.split(';').find_map(|part| {
            let (key, value) = part.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case("lang")
                .then(|| value.trim().to_owned())
                .filter(|v| !v.is_empty())
        });
        let name = name.to_ascii_lowercase();
        if !out.iter().any(|c| c.name == name) {
            out.push(Class { name, lang });
        }
    }
    out
}

/// The codes of a SAMI's languages a lane can choose among ([`Class::code`]),
/// in the file's order — none when it has fewer than two: one language is no
/// choice.
#[must_use]
pub fn languages(text: &str) -> Vec<String> {
    let all = classes(text);
    if all.len() < 2 {
        return Vec::new();
    }
    all.iter().map(|c| c.code().to_owned()).collect()
}

/// The class a lane that keeps `wanted` (a language code, or a class's name)
/// shows of `declared`: the one whose language is `wanted` — `en` takes
/// `en-US` and `en-US` takes `en`, by the primary subtag, ignoring case — or
/// whose name is; `None` when no class answers it, and the first class that
/// says something is shown instead.
#[must_use]
pub fn chosen(declared: &[Class], wanted: &str) -> Option<String> {
    let wanted = wanted.trim().replace('_', "-").to_ascii_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let primary = |code: &str| code.split('-').next().unwrap_or(code).to_owned();
    let lang_of = |c: &Class| {
        c.lang
            .as_deref()
            .map(|l| l.replace('_', "-").to_ascii_lowercase())
    };
    declared
        .iter()
        .find(|c| lang_of(c).as_deref() == Some(wanted.as_str()))
        .or_else(|| declared.iter().find(|c| c.name == wanted))
        .or_else(|| {
            declared
                .iter()
                .find(|c| lang_of(c).is_some_and(|l| primary(&l) == primary(&wanted)))
        })
        .map(|c| c.name.clone())
}

/// Reads a SAMI's cues, in the language a lane keeps (`wanted`, see
/// [`chosen`]) — the first that says something when it names none, or one the
/// file does not have.
#[must_use]
pub fn read_in(text: &str, wanted: Option<&str>) -> Vec<Cue> {
    let class = wanted.and_then(|w| chosen(&classes(text), w));
    cues(text, class)
}

/// Reads a SAMI's cues: the first language that says something.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    cues(text, None)
}

/// The cues of `class`, or of the first class that says something.
fn cues(text: &str, mut class: Option<String>) -> Vec<Cue> {
    let lower = text.to_ascii_lowercase();
    // Every `<sync`, where its tag ends, and its start.
    let mut syncs: Vec<(usize, usize, u64)> = Vec::new();
    let mut from = 0;
    while let Some(found) = lower[from..].find("<sync") {
        let at = from + found;
        let Some(close) = lower[at..].find('>') else {
            break;
        };
        let tag_end = at + close + 1;
        from = tag_end;
        let start = cue::attribute(&text[at + 5..tag_end - 1], "start")
            .and_then(|v| v.trim().parse::<u64>().ok());
        if let Some(start) = start {
            syncs.push((at, tag_end, start));
        }
    }
    let body_end = lower.find("</body>").unwrap_or(text.len());
    let mut cues = Vec::new();
    for (n, &(_, tag_end, start_ms)) in syncs.iter().enumerate() {
        let next = syncs.get(n + 1);
        let content_end = next.map_or(body_end, |s| s.0).max(tag_end);
        let content = &text[tag_end..content_end];
        let Some(body) = paragraph(content, &mut class) else {
            continue;
        };
        let (lines, band) = cue::markup(&body, Dialect::Html);
        // Until the next SYNC, however far: SAMI clears the screen itself.
        let end_ms = next.map_or(start_ms + cue::UNTIMED_MOST_MS, |s| s.2);
        cues.push(Cue {
            start_ms,
            end_ms,
            lines,
            band,
        });
    }
    cues
}

/// The text of the paragraph of `class` in one SYNC's content — the first
/// class with words decides it — or the whole content when it has no `<P>`.
fn paragraph(content: &str, class: &mut Option<String>) -> Option<String> {
    let lower = content.to_ascii_lowercase();
    let mut paragraphs: Vec<(Option<String>, &str)> = Vec::new();
    let mut from = 0;
    while let Some(found) = find_p(&lower[from..]) {
        let at = from + found;
        let close = at + lower[at..].find('>')?;
        let next = find_p(&lower[close..]).map_or(content.len(), |n| close + n);
        let end = lower[close..next].find("</p").map_or(next, |e| close + e);
        let this_class =
            cue::attribute(&content[at + 2..close], "class").map(|c| c.to_ascii_lowercase());
        paragraphs.push((this_class, &content[close + 1..end]));
        from = next;
    }
    if paragraphs.is_empty() {
        return Some(content.to_owned());
    }
    if class.is_none() {
        *class = paragraphs
            .iter()
            .find(|(_, text)| !is_blank(text))
            .map(|(c, _)| c.clone().unwrap_or_default());
    }
    let wanted = class.clone().unwrap_or_default();
    paragraphs
        .into_iter()
        .find(|(c, _)| c.clone().unwrap_or_default() == wanted)
        .map(|(_, text)| text.to_owned())
}

/// `<p` followed by a space or `>`: a paragraph, not `<param`.
fn find_p(lower: &str) -> Option<usize> {
    lower.match_indices("<p").map(|(at, _)| at).find(|&at| {
        lower[at + 2..]
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace() || c == '>')
    })
}

fn is_blank(html: &str) -> bool {
    cue::entities(html)
        .replace("<br>", "")
        .chars()
        .all(|c| c.is_whitespace() || c == '\u{a0}')
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "<SAMI><HEAD><STYLE TYPE=\"text/css\"><!--\n\
P { font-family: Arial; font-size: 1.5em; }\n\
.ENCC { Name: English; lang: en-US; SAMIType: CC; }\n\
.PTCC { Name: Portugu\u{ea}s; lang: pt-BR; }\n\
#Source { color: white; }\n\
--></STYLE></HEAD><BODY>\n\
<SYNC Start=1000><P Class=ENCC>Hello<P Class=PTCC>Ol\u{e1}\n\
<SYNC Start=3000><P Class=ENCC>Goodbye<P Class=PTCC>Tchau\n\
</BODY></SAMI>";

    /// FAILS IF: a SAMI of several languages can only ever show its first,
    /// whatever the lane keeps — or a lane naming a language the file lacks
    /// shows nothing instead of the first that speaks.
    #[test]
    fn a_sami_of_several_languages_shows_the_one_the_lane_keeps() {
        let declared = classes(TWO);
        assert_eq!(
            declared,
            vec![
                Class {
                    name: "encc".into(),
                    lang: Some("en-US".into())
                },
                Class {
                    name: "ptcc".into(),
                    lang: Some("pt-BR".into())
                },
            ],
            "the P rule and the #id rule are no classes"
        );
        assert_eq!(languages(TWO), vec!["en-US", "pt-BR"]);
        let text = |wanted: Option<&str>| {
            let mut c = read_in(TWO, wanted);
            cue::tidy(&mut c);
            c.iter().map(Cue::text).collect::<Vec<_>>()
        };
        assert_eq!(text(None), vec!["Hello", "Goodbye"]);
        assert_eq!(text(Some("pt-BR")), vec!["Ol\u{e1}", "Tchau"]);
        assert_eq!(
            text(Some("pt")),
            vec!["Ol\u{e1}", "Tchau"],
            "by the primary subtag"
        );
        assert_eq!(text(Some("PTCC")), vec!["Ol\u{e1}", "Tchau"], "by its name");
        assert_eq!(text(Some("en")), vec!["Hello", "Goodbye"]);
        assert_eq!(text(Some("ja")), vec!["Hello", "Goodbye"], "one it lacks");
        // ONE LANGUAGE is no choice.
        assert!(languages("<SAMI><STYLE>.ENCC {lang: en;}</STYLE></SAMI>").is_empty());
        // A CLASS WITH NO LANG is named by itself.
        let unnamed = classes("<STYLE>.KRCC {Name: Korean;} .ENCC {lang: en;}</STYLE>");
        assert_eq!(unnamed[0].code(), "krcc");
    }

    #[test]
    fn a_sami_lasts_until_the_next_sync_and_keeps_one_language() {
        let mut cues = read(
            "<SAMI><HEAD><STYLE><!-- .ENCC {Name: English; lang: en-US;} --></STYLE></HEAD><BODY>\n\
<SYNC Start=1000><P Class=ENCC>Hello<br>\n  <i>world</i> &amp; you\n<P Class=FRCC>Bonjour\n\
<SYNC Start=3000><P Class=ENCC>&nbsp;\n\
<SYNC Start=4000><P Class=ENCC><font color=\"#00ff00\">Green</font>\n\
</BODY></SAMI>",
        );
        cue::tidy(&mut cues);
        assert_eq!(cues.len(), 2, "the blank one only clears");
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3000));
        assert_eq!(cues[0].text(), "Hello\nworld & you", "English only");
        assert!(cues[0].lines[1][0].italic);
        // The last one has no next SYNC: it stays at most the ceiling.
        assert_eq!(cues[1].start_ms, 4000);
        assert_eq!(cues[1].end_ms, 4000 + cue::UNTIMED_MOST_MS);
        assert_eq!(
            cues[1].lines[0][0].colour,
            Some(crate::subtitles::Rgb::new(0, 255, 0))
        );
    }
}
