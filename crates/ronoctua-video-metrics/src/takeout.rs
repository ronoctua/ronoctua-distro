//! Google Takeout's list of an account's YouTube subscriptions: read, and
//! written in the same shape. Pure — bytes in, channels out; who opens the
//! file and what the channels become are the caller's
//! ([`crate::person::Person::stage`]).
//!
//! # The shape, as Google writes it
//!
//! ```text
//! Channel ID,Channel URL,Channel title
//! UC…,http://www.youtube.com/channel/UC…,A title
//! UC…,http://www.youtube.com/channel/UC…,"A title, with a comma"
//! ```
//!
//! CSV by RFC 4180: commas between fields, a field in quotes may hold commas
//! and line breaks, `""` is a quote; records end at LF, CRLF or a lone CR.
//! The file Takeout gives ends in blank lines, and may open with a BOM.
//!
//! # The id is the truth
//!
//! The header is told by its SHAPE, never its words — Takeout writes it in
//! the account's language: a first record that names no channel and does not
//! look like a row that tried to (its first field begun `UC`, or a web
//! address in its second) is the header. Of every other record only the id
//! (`ronoctua_ytdlp::youtube::is_channel_id`) and the title are read; the URL
//! is the id again, and a record without an id is left out by its line, for
//! the person to be told. A channel named twice is taken once.

use std::collections::HashSet;

use ronoctua_ytdlp::youtube::is_channel_id;

use crate::ChannelId;

/// The header Google writes, and the one [`write`] writes.
pub const HEADER: &str = "Channel ID,Channel URL,Channel title";

/// Where the person exports the list: Google Takeout.
pub const TAKEOUT_PAGE: &str = "https://takeout.google.com/";

/// The name Google gives the file (the picker adds `.csv`) — a file name, not
/// words of ours.
pub const FILE_NAME: &str = "subscriptions";

/// The most bytes a list is read from: an account's subscriptions are some
/// tens of kilobytes, and a file past this is something else.
pub const MOST_BYTES: u64 = 8 << 20;

/// One channel of a list.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subscription {
    /// Its id.
    pub id: ChannelId,
    /// Its title — data; the id where the list had none.
    pub title: String,
}

/// A list, read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct List {
    /// The channels, in the list's order, each once.
    pub subscriptions: Vec<Subscription>,
    /// The lines (from 1) of the records whose first field is not a channel
    /// id — where each record starts.
    pub left_out: Vec<u32>,
}

/// Why bytes are not a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unread {
    /// Not UTF-8 text.
    NotText,
    /// A quote opened and never closed, in the record that starts on this
    /// line (from 1).
    Unclosed {
        /// The line.
        line: u32,
    },
    /// Not one record names a channel.
    NotAList,
}

/// The records of a CSV text, each with the line (from 1) it starts on. A
/// blank line — a record of one empty field — is no record.
///
/// # Errors
///
/// [`Unread::Unclosed`] for a quote that never closes.
pub fn records(text: &str) -> Result<Vec<(u32, Vec<String>)>, Unread> {
    let mut all = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut field = String::new();
    // Whether the field began with a quote, and whether that quote is open.
    let (mut quoted, mut open) = (false, false);
    let (mut line, mut start) = (1_u32, 1_u32);
    let mut chars = text.chars().peekable();
    let mut ended = |fields: &mut Vec<String>, field: &mut String, start: u32| {
        fields.push(std::mem::take(field));
        let record = std::mem::take(fields);
        let blank = record.len() == 1 && record[0].is_empty();
        if !blank {
            all.push((start, record));
        }
    };
    while let Some(c) = chars.next() {
        if open {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => open = false,
                '\r' => {
                    // A BREAK INSIDE QUOTES IS THE FIELD'S, and still a line.
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                        field.push('\r');
                        field.push('\n');
                    } else {
                        field.push('\r');
                    }
                    line += 1;
                }
                '\n' => {
                    field.push('\n');
                    line += 1;
                }
                other => field.push(other),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !quoted => {
                quoted = true;
                open = true;
            }
            ',' => {
                fields.push(std::mem::take(&mut field));
                quoted = false;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                ended(&mut fields, &mut field, start);
                quoted = false;
                line += 1;
                start = line;
            }
            // WHAT FOLLOWS A CLOSING QUOTE, and a quote inside a bare field,
            // are kept as they are: RFC 4180 allows neither, and a list a
            // spreadsheet wrote that way still names its channels.
            other => field.push(other),
        }
    }
    if open {
        return Err(Unread::Unclosed { line: start });
    }
    if !field.is_empty() || !fields.is_empty() || quoted {
        ended(&mut fields, &mut field, start);
    }
    Ok(all)
}

/// The list in `bytes`: the channels, and the lines left out.
///
/// # Errors
///
/// [`Unread::NotText`] for bytes that are not UTF-8, [`Unread::Unclosed`],
/// and [`Unread::NotAList`] when no record names a channel.
pub fn read(bytes: &[u8]) -> Result<List, Unread> {
    let text = std::str::from_utf8(bytes).map_err(|_| Unread::NotText)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let all = records(text)?;
    let id_of = |record: &[String]| record.first().map(|f| f.trim().to_owned());
    // THE HEADER BY ITS SHAPE: Takeout writes it in the account's language.
    // A first record that names no channel AND does not look like a row that
    // tried to — an id begun as YouTube's, or an address where the URL goes —
    // is the header; a malformed first row of a list without one is left out
    // by its line like any other, never swallowed as the header.
    let looks_like_a_row = |record: &[String]| {
        let id = record.first().map_or("", |f| f.trim());
        let address = record
            .get(1)
            .map(|f| f.trim().to_ascii_lowercase())
            .unwrap_or_default();
        id.starts_with("UC") || address.starts_with("http://") || address.starts_with("https://")
    };
    let header = all.first().is_some_and(|(_, record)| {
        id_of(record).is_some_and(|first| !is_channel_id(&first)) && !looks_like_a_row(record)
    });
    let mut list = List::default();
    let mut seen = HashSet::new();
    for (line, record) in all.iter().skip(usize::from(header)) {
        let id = id_of(record).unwrap_or_default();
        if !is_channel_id(&id) {
            list.left_out.push(*line);
            continue;
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        // A TITLE IS ONE LINE: a break inside its quotes is ONE space, a
        // CRLF as much as a lone LF or CR.
        let title = record
            .get(2)
            .map(|t| {
                t.replace("\r\n", " ")
                    .replace(['\r', '\n'], " ")
                    .trim()
                    .to_owned()
            })
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| id.clone());
        list.subscriptions.push(Subscription {
            id: ChannelId(id),
            title,
        });
    }
    if list.subscriptions.is_empty() {
        return Err(Unread::NotAList);
    }
    Ok(list)
}

/// A channel's address as Google writes it in the list.
#[must_use]
pub fn channel_url(id: &ChannelId) -> String {
    format!("http://www.youtube.com/channel/{}", id.0)
}

/// The list in Google's shape: [`HEADER`], then one line per channel — its
/// id, [`channel_url`] and its title, quoted only when it must be — each
/// ending in LF, as Google's do.
#[must_use]
pub fn write(list: &[Subscription]) -> String {
    let quoted = |field: &str| {
        if field.contains([',', '"', '\r', '\n']) {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_owned()
        }
    };
    let mut text = format!("{HEADER}\n");
    for s in list {
        text.push_str(&format!(
            "{},{},{}\n",
            s.id.0,
            channel_url(&s.id),
            quoted(&s.title)
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Channel ids shaped as YouTube's (`UC` and 22 more), made up.
    fn id(n: u8) -> String {
        format!("UC{}", char::from(b'a' + n).to_string().repeat(22))
    }

    fn line(n: u8, title: &str) -> String {
        format!("{},http://www.youtube.com/channel/{},{title}", id(n), id(n))
    }

    fn titles(list: &List) -> Vec<&str> {
        list.subscriptions
            .iter()
            .map(|s| s.title.as_str())
            .collect()
    }

    /// FAILS IF: the list as Takeout writes it — a title quoted for its
    /// comma, accents, a title ending in a space, the blank lines at the end —
    /// is not read as its channels, exactly.
    #[test]
    fn the_sample_shape_is_read() {
        // pt-BR: an accented title with a comma, as a channel of the account may be named.
        let accented = "Ação, reação!";
        let text = format!(
            "{HEADER}\n{}\n{}\n{}\n{}\n\n\n\n\n\n",
            line(0, "\"Workshop, a maker community\""),
            line(1, &format!("\"{accented}\"")),
            line(2, "Glass Studio "),
            line(3, "Plain Title"),
        );
        let list = read(text.as_bytes()).unwrap();
        assert_eq!(
            titles(&list),
            [
                "Workshop, a maker community",
                accented,
                "Glass Studio",
                "Plain Title"
            ]
        );
        assert_eq!(list.subscriptions[1].id, ChannelId(id(1)));
        assert!(list.left_out.is_empty());
    }

    /// FAILS IF: a doubled quote, CRLF, a BOM or a line break inside quotes
    /// reads as anything but what the field holds, or a title's CRLF comes
    /// out as more than the one space a break is.
    #[test]
    fn quotes_doubled_crlf_bom_and_a_newline_inside_a_quote() {
        let text = format!(
            "\u{feff}{HEADER}\r\n{}\r\n{}\r\n{}\r",
            line(0, "\"The \"\"Best\"\" Bench\""),
            line(1, "\"Two\r\nLines\""),
            line(2, "Last"),
        );
        let list = read(text.as_bytes()).unwrap();
        assert_eq!(titles(&list), ["The \"Best\" Bench", "Two Lines", "Last"]);
        let all = records(&text[3..]).unwrap();
        assert_eq!(
            all.iter().map(|(l, _)| *l).collect::<Vec<_>>(),
            [1, 2, 3, 5],
            "a record's line is where it starts, the break inside quotes counted"
        );
        assert_eq!(all[2].1[2], "Two\r\nLines");
    }

    /// FAILS IF: a header in the account's language is read as a line left
    /// out, or as a channel.
    #[test]
    fn a_localised_header_is_taken_by_its_shape() {
        // pt-BR: the header as Takeout writes it for an account in Portuguese.
        let header = "ID do canal,URL do canal,Título do canal";
        let text = format!("{header}\n{}\n", line(0, "One"));
        let list = read(text.as_bytes()).unwrap();
        assert_eq!(titles(&list), ["One"]);
        assert!(list.left_out.is_empty());
        // NO HEADER AT ALL is a list too.
        let list = read(format!("{}\n", line(1, "Two")).as_bytes()).unwrap();
        assert_eq!(titles(&list), ["Two"]);
    }

    /// FAILS IF: a record without a channel id is dropped silently — the
    /// first of a list without a header too — or told by another line than
    /// its own.
    #[test]
    fn a_line_without_a_channel_id_is_left_out_by_its_line() {
        let text = format!(
            "{HEADER}\n{}\nUCshort,http://www.youtube.com/channel/UCshort,Short\n{}\n",
            line(0, "One"),
            line(1, "Two"),
        );
        let list = read(text.as_bytes()).unwrap();
        assert_eq!(titles(&list), ["One", "Two"]);
        assert_eq!(list.left_out, [3]);
        // A LIST WITHOUT A HEADER whose first row is malformed: that row is
        // told by its line, never taken for the header — by its id begun as
        // YouTube's, or by the address where the URL goes.
        for first in [
            "UCshort,,Short",
            "not-an-id,https://www.youtube.com/channel/UCshort,Short",
        ] {
            let text = format!("{first}\n{}\n", line(0, "One"));
            let list = read(text.as_bytes()).unwrap();
            assert_eq!(titles(&list), ["One"], "{first}");
            assert_eq!(list.left_out, [1], "{first}");
        }
        // AN EMPTY TITLE is the id, never an empty row.
        let list = read(format!("{},x,\n", id(4)).as_bytes()).unwrap();
        assert_eq!(titles(&list), [id(4).as_str()]);
    }

    /// FAILS IF: a quote that never closes reads as a list, or is told by
    /// another line than the one its record starts on.
    #[test]
    fn an_unclosed_quote_is_refused_with_its_line() {
        let text = format!(
            "{HEADER}\n{}\n{}\n{}\n",
            line(0, "One"),
            line(1, "\"Never closed"),
            line(2, "Three"),
        );
        assert_eq!(read(text.as_bytes()), Err(Unread::Unclosed { line: 3 }));
    }

    /// FAILS IF: something that names no channel — nothing, a header alone,
    /// bytes that are not text — is a list of none.
    #[test]
    fn a_file_that_names_no_channel_is_not_a_list() {
        assert_eq!(read(b""), Err(Unread::NotAList));
        assert_eq!(read(b"\n\n\n"), Err(Unread::NotAList));
        assert_eq!(
            read(format!("{HEADER}\n").as_bytes()),
            Err(Unread::NotAList)
        );
        assert_eq!(read(&[0xff, 0xfe, b',', 0x80]), Err(Unread::NotText));
    }

    /// FAILS IF: a channel named twice is staged twice.
    #[test]
    fn duplicates_are_taken_once() {
        let text = format!(
            "{HEADER}\n{}\n{}\n{}\n",
            line(0, "First"),
            line(1, "Other"),
            line(0, "Again"),
        );
        let list = read(text.as_bytes()).unwrap();
        assert_eq!(titles(&list), ["First", "Other"], "the first naming wins");
    }

    /// FAILS IF: what this writes is not Google's shape — the exact header,
    /// the address as Google writes it, quotes only where a field needs them —
    /// or does not read back as the same list.
    #[test]
    fn what_is_written_reads_back_and_quotes_only_what_needs_it() {
        let list = vec![
            Subscription {
                id: ChannelId(id(0)),
                title: "Plain".to_owned(),
            },
            Subscription {
                id: ChannelId(id(1)),
                title: "A comma, and \"quotes\"".to_owned(),
            },
        ];
        let text = write(&list);
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some(HEADER));
        assert_eq!(
            lines.next(),
            Some(format!("{},http://www.youtube.com/channel/{},Plain", id(0), id(0)).as_str())
        );
        assert_eq!(
            lines.next(),
            Some(
                format!(
                    "{},http://www.youtube.com/channel/{},\"A comma, and \"\"quotes\"\"\"",
                    id(1),
                    id(1)
                )
                .as_str()
            )
        );
        assert!(
            text.ends_with('\n') && !text.contains('\r'),
            "LF, as Google"
        );
        assert_eq!(read(text.as_bytes()).unwrap().subscriptions, list);
    }
}
