//! One member of a JSON object, edited in the TEXT — the rest left byte for
//! byte as the person wrote it.
//!
//! # Why not parse and print
//!
//! `settings.json` is the person's file. Parsed into a `serde_json::Value`
//! and printed back, every key would come out sorted and every indent
//! re-made: a field-wise edit that rewrites the whole file is not one. So the
//! file is first proved to be JSON (by `serde_json`, which is the judge), and
//! then only the span of `"statusLine": …` is replaced, inserted or taken
//! out. Giving it back restores the person's original value as the very text
//! it was.

use std::ops::Range;

/// One member of the top-level object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Member {
    /// Its key, unescaped.
    pub(crate) key: String,
    /// From the key's opening quote to the end of its value.
    pub(crate) whole: Range<usize>,
    /// The value's text.
    pub(crate) value: Range<usize>,
}

/// The top-level object of `text`: its members in order, and where its
/// closing brace is. `None` when the text is not JSON, or not an object.
pub(crate) fn object(text: &str) -> Option<(Vec<Member>, usize)> {
    let parsed: serde_json::Value = serde_json::from_str(text).ok()?;
    if !parsed.is_object() {
        return None;
    }
    let b = text.as_bytes();
    let mut i = skip_space(b, 0);
    if b.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    let mut members = Vec::new();
    loop {
        i = skip_space(b, i);
        match b.get(i)? {
            b'}' => return Some((members, i)),
            b',' => {
                i += 1;
                continue;
            }
            b'"' => {}
            _ => return None,
        }
        let key_start = i;
        let key_end = skip_string(b, i)?;
        let key: String = serde_json::from_str(&text[key_start..key_end]).ok()?;
        i = skip_space(b, key_end);
        if b.get(i) != Some(&b':') {
            return None;
        }
        let value_start = skip_space(b, i + 1);
        let value_end = skip_value(b, value_start)?;
        members.push(Member {
            key,
            whole: key_start..value_end,
            value: value_start..value_end,
        });
        i = value_end;
    }
}

/// Past JSON whitespace (and a byte-order mark at the very start).
fn skip_space(b: &[u8], mut i: usize) -> usize {
    if i == 0 && b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }
    while matches!(b.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        i += 1;
    }
    i
}

/// Past a string starting at `i` (its opening quote).
fn skip_string(b: &[u8], i: usize) -> Option<usize> {
    let mut k = i + 1;
    loop {
        match b.get(k)? {
            b'\\' => k += 2,
            b'"' => return Some(k + 1),
            _ => k += 1,
        }
    }
}

/// Past one value starting at `i`.
fn skip_value(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i)? {
        b'"' => skip_string(b, i),
        b'{' | b'[' => {
            let mut depth = 0_usize;
            let mut k = i;
            loop {
                match b.get(k)? {
                    b'"' => {
                        k = skip_string(b, k)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(k + 1);
                        }
                    }
                    _ => {}
                }
                k += 1;
            }
        }
        _ => {
            let mut k = i;
            while let Some(c) = b.get(k) {
                if matches!(c, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r') {
                    break;
                }
                k += 1;
            }
            (k > i).then_some(k)
        }
    }
}

/// The indent of the line `at` is on: the spaces and tabs before it, when
/// nothing else is.
fn indent_at(text: &str, at: usize) -> Option<&str> {
    let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
    let before = &text[line_start..at];
    before
        .bytes()
        .all(|c| c == b' ' || c == b'\t')
        .then_some(before)
}

/// `value` printed pretty, every line after the first indented by `indent`
/// — so a nested object sits where a person's editor would have put it.
fn pretty_at(value: &serde_json::Value, indent: &str) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    let mut out = String::with_capacity(text.len());
    for (n, line) in text.lines().enumerate() {
        if n > 0 {
            out.push('\n');
            out.push_str(indent);
        }
        out.push_str(line);
    }
    out
}

/// `text` with the LAST member named `key` set to `value` (replaced where it
/// is, or appended after the last member), everything else untouched.
/// `None` when the text is not a JSON object.
pub(crate) fn set(text: &str, key: &str, value: &serde_json::Value) -> Option<String> {
    let (members, close) = object(text)?;
    let quoted = serde_json::to_string(key).ok()?;
    if let Some(member) = members.iter().rev().find(|m| m.key == key) {
        let indent = indent_at(text, member.whole.start).unwrap_or("");
        let printed = pretty_at(value, indent);
        return Some(format!(
            "{}{printed}{}",
            &text[..member.value.start],
            &text[member.value.end..]
        ));
    }
    let Some(last) = members.last() else {
        // `{}` (or `{ }`): the member is the object's only content.
        let printed = pretty_at(value, "  ");
        let open = text[..close].rfind('{')?;
        return Some(format!(
            "{}{{\n  {quoted}: {printed}\n}}{}",
            &text[..open],
            &text[close + 1..]
        ));
    };
    let indent = indent_at(text, last.whole.start).unwrap_or("  ");
    let printed = pretty_at(value, indent);
    let line = if indent_at(text, last.whole.start).is_some() {
        format!(",\n{indent}{quoted}: {printed}")
    } else {
        format!(", {quoted}: {printed}")
    };
    Some(format!(
        "{}{line}{}",
        &text[..last.whole.end],
        &text[last.whole.end..]
    ))
}

/// `text` with the member named `key` put back as the text `raw` was — the
/// exact span a person wrote, restored. `None` when the text is not a JSON
/// object, or `raw` is not one JSON value.
pub(crate) fn set_raw(text: &str, key: &str, raw: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw).ok()?;
    let (members, _) = object(text)?;
    if let Some(member) = members.iter().rev().find(|m| m.key == key) {
        return Some(format!(
            "{}{raw}{}",
            &text[..member.value.start],
            &text[member.value.end..]
        ));
    }
    // Not there any more: appended as a value, then its text put in place.
    let placed = set(text, key, &serde_json::Value::Null)?;
    let (members, _) = object(&placed)?;
    let member = members.iter().rev().find(|m| m.key == key)?;
    Some(format!(
        "{}{raw}{}",
        &placed[..member.value.start],
        &placed[member.value.end..]
    ))
}

/// `text` without any member named `key`, the commas and the line it stood
/// on taken with it. `None` when the text is not a JSON object.
pub(crate) fn remove(text: &str, key: &str) -> Option<String> {
    let mut out = text.to_owned();
    while let Some((members, close)) = object(&out) {
        let Some(at) = members.iter().position(|m| m.key == key) else {
            return Some(out);
        };
        let member = &members[at];
        let cut = if at > 0 {
            // From the end of the member before (its comma and the line
            // break included) to the end of this one.
            members[at - 1].whole.end..member.whole.end
        } else if let Some(next) = members.get(1) {
            // The first of several: up to the next one's key, so the next
            // one takes this one's place and indent.
            member.whole.start..next.whole.start
        } else {
            // The only one: the object becomes empty, `{}`.
            let open = out[..member.whole.start].rfind('{')?;
            open + 1..close
        };
        out.replace_range(cut, "");
    }
    None
}

/// The text of the LAST member named `key`, when there is one.
pub(crate) fn raw<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let (members, _) = object(text)?;
    members
        .iter()
        .rev()
        .find(|m| m.key == key)
        .map(|m| &text[m.value.clone()])
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERSON: &str = "{\n  \"model\": \"opus\",\n  \"statusLine\": {\n    \"type\": \"command\",\n    \"command\": \"~/bin/line \\\"x\\\"\",\n    \"padding\": 2\n  },\n  \"env\": { \"A\": \"b,}\" }\n}\n";

    #[test]
    fn a_member_is_found_by_its_span() {
        let (members, close) = object(PERSON).unwrap();
        let keys: Vec<&str> = members.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, ["model", "statusLine", "env"]);
        assert_eq!(&PERSON[close..=close], "}");
        assert_eq!(
            raw(PERSON, "env"),
            Some("{ \"A\": \"b,}\" }"),
            "a brace inside a string is not the end"
        );
        assert_eq!(object("[1, 2]"), None, "not an object");
        assert_eq!(object("{\"a\": }"), None, "not JSON");
    }

    #[test]
    fn replacing_and_restoring_gives_the_same_bytes() {
        let original = raw(PERSON, "statusLine").unwrap().to_owned();
        let ours = serde_json::json!({"type": "command", "command": "ours"});
        let wrapped = set(PERSON, "statusLine", &ours).unwrap();
        assert!(wrapped.starts_with("{\n  \"model\": \"opus\",\n  \"statusLine\": {\n    \"command\": \"ours\",\n    \"type\": \"command\"\n  },"));
        assert!(
            wrapped.ends_with("  \"env\": { \"A\": \"b,}\" }\n}\n"),
            "{wrapped}"
        );
        let back = set_raw(&wrapped, "statusLine", &original).unwrap();
        assert_eq!(back, PERSON);
    }

    #[test]
    fn a_member_that_was_not_there_is_appended_and_taken_back_out() {
        for text in [
            "{\n  \"model\": \"opus\"\n}\n",
            "{\n\t\"model\": \"opus\",\n\t\"x\": [1, {\"y\": 2}]\n}",
            "{\"model\":\"opus\"}",
            "{}\n",
            "{ }",
        ] {
            let ours = serde_json::json!({"type": "command", "command": "ours"});
            let wrapped = set(text, "statusLine", &ours).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&wrapped).unwrap();
            assert_eq!(parsed["statusLine"], ours, "{wrapped}");
            let back = remove(&wrapped, "statusLine").unwrap();
            let before: serde_json::Value = serde_json::from_str(text).unwrap();
            let after: serde_json::Value = serde_json::from_str(&back).unwrap();
            assert_eq!(after, before, "{text:?} → {wrapped:?} → {back:?}");
            if text.contains('\n') && text.len() > 5 {
                assert_eq!(back, text, "a pretty file comes back byte for byte");
            }
        }
    }

    #[test]
    fn removing_the_first_or_the_only_member_leaves_json() {
        let first = "{\n  \"statusLine\": 1,\n  \"b\": 2\n}";
        assert_eq!(remove(first, "statusLine").unwrap(), "{\n  \"b\": 2\n}");
        let only = "{\n  \"statusLine\": {\"a\": 1}\n}";
        assert_eq!(remove(only, "statusLine").unwrap(), "{}");
        let twice = "{\"statusLine\": 1, \"a\": 2, \"statusLine\": 3}";
        let gone = remove(twice, "statusLine").unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&gone).unwrap(),
            serde_json::json!({"a": 2})
        );
        assert_eq!(remove("not json", "a"), None);
    }
}
