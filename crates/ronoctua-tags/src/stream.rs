//! The sheet of something that plays from the NETWORK: a radio, a TV
//! channel, a file behind an address.
//!
//! # Read only, and from what it says while it plays
//!
//! Nothing on this machine has its bytes, so nothing here can be written
//! and no program can read it again: what is known is what the stream says
//! as it plays — a radio's ICY words (`icy-title` is the song on the air,
//! `icy-name` the station), or the tags a file behind an address carries,
//! as the player read them. The daemon hands those over as pairs; this
//! turns them into the same [`Sheet`] a file has, so a door draws one page
//! for both. Every row says it cannot be changed, and why
//! ([`Why::NotAFile`]).
//!
//! Pure: no program, no filesystem, no clock.

use std::path::PathBuf;

use crate::sheet::{
    Adding, Can, Container, Fact, Field, FieldRow, Kind, Origin, Sheet, Stamp, Tag, TagValue, Why,
};

/// The ICY words a field is also read from, after ffmpeg's own keys: a
/// radio says the song on the air as `icy-title` ("Artist - Title", as the
/// station writes it), its genre and its description.
const fn icy_keys(field: Field) -> &'static [&'static str] {
    match field {
        Field::Title => &["icy-title"],
        Field::Genre => &["icy-genre"],
        Field::Description => &["icy-description"],
        _ => &[],
    }
}

/// The value of `field` among `tags`, by ffmpeg's keys first and then the
/// ICY ones; any case, as mpv passes them on as the stream wrote them.
fn value_of(tags: &[(String, String)], field: Field) -> Option<String> {
    field
        .ffmpeg_keys()
        .iter()
        .chain(icy_keys(field))
        .find_map(|key| {
            tags.iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .map(|(_, value)| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

/// The read-only sheet of what plays at `location`.
///
/// `kind` is what the player measured (a picture that is not a cover makes
/// it a video), `tags` what the stream said, in the order it said them, and
/// `technical` what the player measured of it (codec, rate, size).
///
/// The rows are those of a file: the common fields of its kind, blank ones
/// included, then any other field the stream carries — and every tag it
/// said, by its own name, on the Every-tag page.
#[must_use]
pub fn sheet(location: &str, kind: Kind, tags: &[(String, String)], technical: Vec<Fact>) -> Sheet {
    let no = || Can::No(Why::NotAFile);
    let mut fields: Vec<FieldRow> = Field::common(kind)
        .iter()
        .map(|&field| FieldRow {
            field,
            value: value_of(tags, field),
            can: no(),
        })
        .collect();
    for field in Field::ALL {
        if fields.iter().any(|row| row.field == field) {
            continue;
        }
        if let Some(value) = value_of(tags, field) {
            fields.push(FieldRow {
                field,
                value: Some(value),
                can: no(),
            });
        }
    }
    let other = tags
        .iter()
        .map(|(name, value)| Tag {
            origin: Origin::Stream,
            name: name.clone(),
            value: TagValue::of(value),
            // What a stream SAYS: nothing on this machine has its bytes.
            can: no(),
        })
        .collect();
    Sheet {
        location: PathBuf::from(location),
        stamp: Stamp::NONE,
        kind,
        container: Container::Other,
        fields,
        covers: Vec::new(),
        cover_can: no(),
        technical,
        streams: Vec::new(),
        chapters: Vec::new(),
        other,
        unread: Vec::new(),
        cover_file: None,
        adding: Some(Adding {
            origin: None,
            can: no(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::sheet;
    use crate::sheet::{Can, Fact, Field, Kind, Origin, Stamp, TagValue, TechKey, Why};

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// FAILS IF: a radio's song on the air stops being its title, a
    /// station's own words are lost instead of kept as its tags, or a row
    /// of a stream says it can be written.
    #[test]
    fn a_radio_is_titled_by_the_song_on_the_air_and_nothing_can_be_written() {
        let tags = pairs(&[
            ("icy-name", "Rádio Exemplo"),
            ("icy-genre", "Jazz"),
            ("icy-title", "Someone - Something"),
            ("icy-br", "128"),
        ]);
        let technical = vec![Fact {
            key: TechKey::AudioCodec,
            value: "mp3".into(),
        }];
        let s = sheet("https://radio.example/live", Kind::Music, &tags, technical);
        assert_eq!(s.value(Field::Title), Some("Someone - Something"));
        assert_eq!(s.value(Field::Genre), Some("Jazz"));
        assert_eq!(s.stamp, Stamp::NONE);
        // THE COMMON ROWS, blanks included: a blank is a row, not an absence.
        assert_eq!(s.fields.len(), Field::common(Kind::Music).len());
        assert!(s.field(Field::Album).is_some_and(|r| r.value.is_none()));
        assert!(
            s.fields.iter().all(|r| r.can == Can::No(Why::NotAFile)),
            "{:?}",
            s.fields
        );
        assert_eq!(s.cover_can, Can::No(Why::NotAFile));
        // EVERY WORD IT SAID, under its own name.
        let names: Vec<&str> = s.other.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["icy-name", "icy-genre", "icy-title", "icy-br"]);
        assert!(s.other.iter().all(|t| t.origin == Origin::Stream));
        // NONE OF THEM WRITTEN, and a new one added nowhere — each saying
        // why in the stream's own words.
        assert!(s.other.iter().all(|t| t.can == Can::No(Why::NotAFile)));
        assert_eq!(
            s.adding.as_ref().map(|a| (a.origin.clone(), a.can.clone())),
            Some((None, Can::No(Why::NotAFile)))
        );
        assert_eq!(s.technical.len(), 1);
    }

    /// FAILS IF: a file's own tag loses to the station's words, a field
    /// outside the common rows is dropped when the stream carries it, or a
    /// long value reaches a door whole.
    #[test]
    fn a_file_behind_an_address_keeps_its_own_tags() {
        let long = "x".repeat(crate::sheet::TAG_VALUE_BYTES + 10);
        let tags = pairs(&[
            ("TITLE", "Its own"),
            ("icy-title", "The station's"),
            ("show", "A series"),
            ("comment", &long),
        ]);
        let s = sheet(
            "https://files.example/a.mkv",
            Kind::Video,
            &tags,
            Vec::new(),
        );
        assert_eq!(s.value(Field::Title), Some("Its own"));
        assert_eq!(s.value(Field::Show), Some("A series"));
        assert!(matches!(
            s.other.iter().find(|t| t.name == "comment").map(|t| &t.value),
            Some(TagValue::Cut { bytes, .. }) if *bytes == long.len() as u64
        ));
    }
}
