//! Writes, end to end: a file made here, a change, the sheet read back.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::{Watch, apply, apply_watched, save_cover};
use crate::fixture::{self, Dir};
use crate::read::sheet;
use crate::run::Tools;
use crate::sheet::{
    Can, Container, CoverAt, CoverEdit, CoverRole, Edit, Field, Origin, Program, Source, Stamp,
    TagFormat, TagValue, Why, Writer,
};

fn stamp_of(path: &Path) -> Stamp {
    Stamp::of(&std::fs::metadata(path).expect("the file is there"))
}

/// A value each field accepts, so every writable row can be filled.
fn sample(field: Field) -> &'static str {
    match field {
        Field::Title => "Título ☃ & <b>",
        Field::Artist => "Someone",
        Field::Album => "An Album",
        Field::AlbumArtist => "Various",
        Field::Track => "3/12",
        Field::Disc => "1/2",
        Field::Date => "2024-05-01",
        Field::Genre => "Samba",
        Field::Composer => "Composer",
        Field::Lyricist => "Lyricist",
        Field::Comment => "line one\nline two",
        Field::Lyrics => "la la\nla",
        Field::Publisher => "A Label",
        Field::Isrc => "BRABC2400001",
        Field::Description => "What it is about",
        Field::Show => "A Show",
        Field::Season => "2",
        Field::Episode => "5",
        Field::Director => "Director",
    }
}

fn png(dir: &Dir) -> std::path::PathBuf {
    dir.file(
        "front.png",
        &crate::picture::fixture::image(4, 4, image::ImageFormat::Png),
    )
}

/// Every writable row filled, a cover put in, the sheet read back.
fn round_trip(name: &str, bytes: &[u8], container: Container) {
    let dir = Dir::new(name);
    let file = dir.file(name, bytes);
    let cover = png(&dir);
    let tools = Tools::none();
    let before = sheet(&file, &tools).unwrap_or_else(|w| panic!("{name}: {w:?}"));
    assert_eq!(before.container, container, "{name}");
    assert!(
        before
            .unread
            .iter()
            .all(|u| u.source != crate::sheet::Source::Lofty),
        "{name}: {:?}",
        before.unread
    );
    let writable: Vec<Field> = before
        .fields
        .iter()
        .filter(|r| r.can == Can::Yes(Writer::Lofty))
        .map(|r| r.field)
        .collect();
    assert!(
        writable.contains(&Field::Title),
        "{name}: {:?}",
        before.fields
    );
    assert!(writable.contains(&Field::Track), "{name}");
    let mut edits: Vec<Edit> = writable
        .iter()
        .map(|f| Edit::Set {
            field: *f,
            value: sample(*f).to_owned(),
        })
        .collect();
    assert_eq!(before.cover_can, Can::Yes(Writer::Lofty), "{name}");
    edits.push(Edit::Cover(CoverEdit::Replace { from: cover }));

    let stamp =
        apply(&file, before.stamp, &edits, &tools).unwrap_or_else(|w| panic!("{name}: {w:?}"));
    assert_ne!(stamp, before.stamp, "{name}: a write is a new stamp");
    assert_eq!(
        stamp,
        stamp_of(&file),
        "{name}: the stamp returned is the file's"
    );

    let after = sheet(&file, &tools).expect("it reads back");
    for f in &writable {
        let expected = f.check(sample(*f)).expect("the samples are valid");
        assert_eq!(
            after.field(*f).and_then(|r| r.value.clone()).as_deref(),
            Some(expected.as_str()),
            "{name}: {f:?}"
        );
    }
    let fronts: Vec<_> = after
        .covers
        .iter()
        .filter(|c| c.role == CoverRole::Front)
        .collect();
    assert_eq!(fronts.len(), 1, "{name}: {:?}", after.covers);
    assert_eq!(fronts[0].mime, "image/png", "{name}");
    assert_eq!(
        (fronts[0].width, fronts[0].height),
        (Some(4), Some(4)),
        "{name}"
    );
    // Nothing is left beside the file but what the test put there.
    assert_eq!(
        dir.names(),
        ["front.png".to_owned(), name.to_owned()],
        "{name}"
    );
}

#[test]
fn an_mp3_round_trips() {
    round_trip("song.mp3", &fixture::mp3(), Container::Mpeg);
}

#[test]
fn a_flac_round_trips() {
    round_trip("song.flac", &fixture::flac(), Container::Flac);
}

#[test]
fn an_ogg_vorbis_round_trips() {
    round_trip("song.ogg", &fixture::vorbis(), Container::OggVorbis);
}

#[test]
fn an_opus_round_trips() {
    round_trip("song.opus", &fixture::opus(), Container::Opus);
}

#[test]
fn an_m4a_round_trips() {
    round_trip("song.m4a", &fixture::m4a(), Container::Mp4);
}

#[test]
fn a_wav_round_trips() {
    round_trip("song.wav", &fixture::wav(), Container::Wav);
}

#[test]
fn a_vorbis_comment_lofty_has_no_name_for_survives_an_edit() {
    let dir = Dir::new("ripper");
    let file = dir.file("song.flac", &fixture::flac());
    let tools = Tools::none();
    let vorbis = Origin::Lofty {
        format: TagFormat::Vorbis,
    };
    let stamp = apply(
        &file,
        stamp_of(&file),
        &[Edit::SetTag {
            origin: vorbis.clone(),
            name: "RIPPER".to_owned(),
            value: "someone's rig".to_owned(),
        }],
        &tools,
    )
    .expect("a Vorbis key is any key");
    let stamp = apply(
        &file,
        stamp,
        &[Edit::Set {
            field: Field::Title,
            value: "New".to_owned(),
        }],
        &tools,
    )
    .expect("a title");
    let s = sheet(&file, &tools).expect("read");
    let ripper = s
        .other
        .iter()
        .find(|t| t.origin == vorbis && t.name == "RIPPER")
        .expect("the key nobody named is still there");
    assert_eq!(ripper.value, TagValue::Text("someone's rig".to_owned()));
    assert_eq!(ripper.can, Can::Yes(Writer::Lofty));
    apply(
        &file,
        stamp,
        &[Edit::RemoveTag {
            origin: vorbis.clone(),
            name: "RIPPER".to_owned(),
        }],
        &tools,
    )
    .expect("removed");
    let s = sheet(&file, &tools).expect("read");
    assert!(!s.other.iter().any(|t| t.name == "RIPPER"));
    assert_eq!(
        s.field(Field::Title)
            .and_then(|r| r.value.clone())
            .as_deref(),
        Some("New")
    );
}

#[test]
fn a_described_comment_is_neither_the_comment_nor_lost_with_it() {
    use lofty::config::WriteOptions;
    use lofty::id3::v2::{CommentFrame, Frame, Id3v2Tag};
    use lofty::tag::TagExt as _;
    let dir = Dir::new("itunes");
    let file = dir.file("song.mp3", &fixture::mp3());
    // What iTunes leaves: its loudness data in a DESCRIBED comment, and a
    // person's comment beside it.
    let mut id3 = Id3v2Tag::new();
    let latin1 = lofty::TextEncoding::Latin1;
    id3.insert(Frame::Comment(CommentFrame::new(
        latin1,
        *b"eng",
        "iTunNORM",
        " 000001A2 00000180",
    )));
    id3.insert(Frame::Comment(CommentFrame::new(
        latin1, *b"eng", "", "mine",
    )));
    id3.save_to_path(&file, WriteOptions::default())
        .expect("a tag");

    let tools = Tools::none();
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(
        s.field(Field::Comment)
            .and_then(|r| r.value.clone())
            .as_deref(),
        Some("mine")
    );
    apply(
        &file,
        s.stamp,
        &[Edit::Set {
            field: Field::Comment,
            value: "changed".to_owned(),
        }],
        &tools,
    )
    .expect("written");
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(
        s.field(Field::Comment)
            .and_then(|r| r.value.clone())
            .as_deref(),
        Some("changed")
    );
    // THE DESCRIBED ONE IS A ROW OF ITS OWN, named by its description, and
    // written apart from the field.
    let norm = s
        .other
        .iter()
        .find(|t| t.name == "COMM:iTunNORM")
        .unwrap_or_else(|| panic!("iTunes' comment is still there: {:?}", s.other));
    assert_eq!(norm.value, TagValue::Text(" 000001A2 00000180".to_owned()));
    assert_eq!(norm.can, Can::Yes(Writer::Lofty));
    let id3 = Origin::Lofty {
        format: TagFormat::Id3v2,
    };
    let stamp = apply(
        &file,
        s.stamp,
        &[Edit::SetTag {
            origin: id3.clone(),
            name: "COMM:iTunNORM".to_owned(),
            value: " 00000001".to_owned(),
        }],
        &tools,
    )
    .expect("the described one written");
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(
        row_value(&s, &id3, "COMM:iTunNORM"),
        Some(TagValue::Text(" 00000001".to_owned()))
    );
    assert_eq!(
        row_value(&s, &id3, "COMM"),
        Some(TagValue::Text("changed".to_owned()))
    );
    apply(
        &file,
        stamp,
        &[Edit::RemoveTag {
            origin: id3,
            name: "COMM:iTunNORM".to_owned(),
        }],
        &tools,
    )
    .expect("the described one taken out");
    let s = sheet(&file, &tools).expect("read");
    assert!(!s.other.iter().any(|t| t.name == "COMM:iTunNORM"));
    assert_eq!(
        s.field(Field::Comment)
            .and_then(|r| r.value.clone())
            .as_deref(),
        Some("changed"),
        "the plain comment stays"
    );
}

#[test]
fn a_flac_keeps_its_pictures_when_only_a_field_changes() {
    let dir = Dir::new("flac-pictures");
    let file = dir.file("song.flac", &fixture::flac());
    let cover = png(&dir);
    let tools = Tools::none();
    let stamp = apply(
        &file,
        stamp_of(&file),
        &[Edit::Cover(CoverEdit::Replace { from: cover })],
        &tools,
    )
    .expect("a cover");
    apply(
        &file,
        stamp,
        &[Edit::Set {
            field: Field::Genre,
            value: "Jazz".to_owned(),
        }],
        &tools,
    )
    .expect("a genre");
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(s.covers.len(), 1, "the PICTURE block was carried through");
}

#[test]
fn a_draft_lofty_broke_is_refused_and_the_file_stays() {
    // lofty 0.25.4 truncates a file whose tag ends it when the tag shrinks
    // (see `native::verify`). The draft is read back, found broken, and
    // thrown away.
    let dir = Dir::new("lofty-shrink");
    let file = dir.file("song.flac", &fixture::flac_without_frames());
    let tools = Tools::none();
    let vorbis = Origin::Lofty {
        format: TagFormat::Vorbis,
    };
    let stamp = apply(
        &file,
        stamp_of(&file),
        &[Edit::SetTag {
            origin: vorbis.clone(),
            name: "RIPPER".to_owned(),
            value: "a long value that the next write takes away".to_owned(),
        }],
        &tools,
    )
    .expect("growing is fine");
    let bytes = std::fs::read(&file).expect("read");
    let shrink = [Edit::RemoveTag {
        origin: vorbis,
        name: "RIPPER".to_owned(),
    }];
    assert_eq!(apply(&file, stamp, &shrink, &tools), Err(Why::Unwritable));
    assert_eq!(
        std::fs::read(&file).expect("read"),
        bytes,
        "the file is as it was"
    );
    assert_eq!(dir.names(), ["song.flac"]);
}

/// The value of the row named `name` in `origin`, as read back.
fn row_value(s: &crate::sheet::Sheet, origin: &Origin, name: &str) -> Option<TagValue> {
    s.other
        .iter()
        .find(|t| &t.origin == origin && t.name == name)
        .map(|t| t.value.clone())
}

/// FAILS IF: a tag ADDED by its name from the Every-tag page stops going
/// into the file's main tag, stops reading back as a row that can be
/// edited and taken out again, or a FLAC's sheet starts asking Matroska's
/// tools (or saying they are missing) — the photograph that showed
/// "mkvextract is not installed" under a FLAC was a fixture's, and this is
/// the reader's own answer, with the tools there and without them.
#[test]
fn a_flac_takes_a_tag_by_its_own_name_and_never_asks_matroska() {
    let dir = Dir::new("flac-named");
    let file = dir.file("song.flac", &fixture::flac());
    let tools_dir = Dir::new("flac-named-tools");
    let asked = tools_dir.path.join("asked");
    let spy = |name: &str| {
        script(
            &tools_dir,
            name,
            &format!("echo {name} >> '{}'; exit 1", asked.display()),
        )
    };
    let with_matroska = Tools::none()
        .with(Program::Mkvextract, spy("mkvextract"))
        .with(Program::Mkvmerge, spy("mkvmerge"))
        .with(Program::Mkvpropedit, spy("mkvpropedit"));
    let tools = Tools::none();
    for t in [&tools, &with_matroska] {
        let s = sheet(&file, t).expect("read");
        assert!(
            s.unread.iter().all(|u| u.source != Source::Matroska),
            "{:?}",
            s.unread
        );
    }
    assert!(!asked.exists(), "a FLAC ran Matroska's tools");
    let s = sheet(&file, &tools).expect("read");
    let vorbis = Origin::Lofty {
        format: TagFormat::Vorbis,
    };
    assert_eq!(
        s.adding.as_ref().map(|a| (a.origin.clone(), a.can.clone())),
        Some((Some(vorbis.clone()), Can::Yes(Writer::Lofty)))
    );
    // A TITLE BESIDE IT: lofty 0.25.4 cannot write a FLAC's comments EMPTY
    // (the draft reads back short and is refused, the file as it was —
    // PENDING), so the tag taken out below is not the last one.
    let stamp = apply(
        &file,
        s.stamp,
        &[
            Edit::Set {
                field: Field::Title,
                value: "Harbour".to_owned(),
            },
            Edit::SetTag {
                origin: vorbis.clone(),
                name: " MOOD ".to_owned(),
                value: "calm".to_owned(),
            },
        ],
        &with_matroska,
    )
    .expect("added");
    assert!(!asked.exists(), "a FLAC's write ran Matroska's tools");
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(
        row_value(&s, &vorbis, "MOOD"),
        Some(TagValue::Text("calm".to_owned()))
    );
    let stamp = apply(
        &file,
        stamp,
        &[Edit::SetTag {
            origin: vorbis.clone(),
            name: "MOOD".to_owned(),
            value: "restless".to_owned(),
        }],
        &tools,
    )
    .expect("changed");
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(
        s.other.iter().filter(|t| t.name == "MOOD").count(),
        1,
        "a name is written whole"
    );
    assert_eq!(
        row_value(&s, &vorbis, "MOOD"),
        Some(TagValue::Text("restless".to_owned()))
    );
    // A NAME THE FORMAT REFUSES is refused before a draft, the file as it was.
    let bytes = std::fs::read(&file).expect("read");
    let bad = [Edit::SetTag {
        origin: vorbis.clone(),
        name: "A=B".to_owned(),
        value: "x".to_owned(),
    }];
    assert_eq!(apply(&file, stamp, &bad, &tools), Err(Why::BadName));
    assert_eq!(std::fs::read(&file).expect("read"), bytes);
    apply(
        &file,
        stamp,
        &[Edit::RemoveTag {
            origin: vorbis.clone(),
            name: "MOOD".to_owned(),
        }],
        &tools,
    )
    .expect("taken out");
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(row_value(&s, &vorbis, "MOOD"), None);
    assert_eq!(dir.names(), ["song.flac"]);
}

/// FAILS IF: a word typed for an MP3 stops becoming a user text frame, a
/// track number's frame or atom is offered as text (a typed value would not
/// fit `TRCK` or `trkn`), or an MP4 name stops becoming the iTunes freeform
/// atom — or either stops coming back to be edited and taken out.
#[test]
fn an_mp3_s_word_is_a_user_text_frame_and_an_m4a_s_a_freeform_atom() {
    let tools = Tools::none();
    for (name, bytes, format, typed, written) in [
        (
            "song.mp3",
            fixture::mp3(),
            TagFormat::Id3v2,
            "MOOD",
            "TXXX:MOOD",
        ),
        (
            "song.m4a",
            fixture::m4a(),
            TagFormat::Mp4,
            // A name lofty has no key for: one it has (`MOOD`) comes back
            // spelled as lofty spells it, and its row is edited by that.
            "Occasion",
            "----:com.apple.iTunes:Occasion",
        ),
    ] {
        let dir = Dir::new("named");
        let file = dir.file(name, &bytes);
        let origin = Origin::Lofty { format };
        let s = sheet(&file, &tools).expect("read");
        assert_eq!(
            s.adding.as_ref().and_then(|a| a.origin.clone()),
            Some(origin.clone()),
            "{name}"
        );
        let stamp = apply(
            &file,
            s.stamp,
            &[
                Edit::SetTag {
                    origin: origin.clone(),
                    name: typed.to_owned(),
                    value: "calm".to_owned(),
                },
                Edit::Set {
                    field: Field::Track,
                    value: "3/12".to_owned(),
                },
            ],
            &tools,
        )
        .unwrap_or_else(|w| panic!("{name}: {w:?}"));
        let s = sheet(&file, &tools).expect("read");
        assert_eq!(
            row_value(&s, &origin, written),
            Some(TagValue::Text("calm".to_owned())),
            "{name}: {:?}",
            s.other
        );
        let track = s
            .other
            .iter()
            .find(|t| t.origin == origin && matches!(t.name.as_str(), "TRCK" | "trkn"))
            .unwrap_or_else(|| panic!("{name}: the track's own row: {:?}", s.other));
        assert_eq!(track.can, Can::No(Why::ReadOnlyView), "{name}");
        apply(
            &file,
            stamp,
            &[Edit::RemoveTag {
                origin: origin.clone(),
                name: written.to_owned(),
            }],
            &tools,
        )
        .unwrap_or_else(|w| panic!("{name}: {w:?}"));
        let s = sheet(&file, &tools).expect("read");
        assert_eq!(row_value(&s, &origin, written), None, "{name}");
        assert_eq!(
            s.field(Field::Track)
                .and_then(|r| r.value.clone())
                .as_deref(),
            Some("3/12"),
            "{name}"
        );
    }
}

/// FAILS IF: a tag is sent to a writer that cannot place it — ffprobe's
/// mirror of an MP3's frame, a Matroska name into an MP3, a track's
/// Matroska tag, a stream's word — or the refusal stops naming the reason.
#[test]
fn a_tag_goes_only_where_its_row_came_from() {
    let dir = Dir::new("misplaced");
    let file = dir.file("song.mp3", &fixture::mp3());
    let tools = Tools::none();
    let stamp = stamp_of(&file);
    let set = |origin: Origin| {
        [Edit::SetTag {
            origin,
            name: "title".to_owned(),
            value: "x".to_owned(),
        }]
    };
    let global = Origin::Matroska {
        target: crate::sheet::Target::Global,
    };
    let track = Origin::Matroska {
        target: crate::sheet::Target::Track,
    };
    for (origin, why) in [
        (Origin::Ffprobe { stream: None }, Why::ReadOnlyView),
        (global, Why::NoPlace),
        (track, Why::NotGlobal),
        (Origin::Stream, Why::NotAFile),
    ] {
        assert_eq!(
            apply(&file, stamp, &set(origin.clone()), &tools),
            Err(why),
            "{origin:?}"
        );
    }
    assert_eq!(stamp_of(&file), stamp);
}

/// FAILS IF: the remux stops writing a tag by its own name where ffprobe
/// read it — the container's in both places, as a field; a stream's in that
/// stream alone — or taking one out stops being ffmpeg's empty value.
#[test]
fn a_remux_writes_a_tag_by_its_name_where_ffprobe_read_it() {
    if !Path::new("/bin/sh").exists() {
        return;
    }
    let dir = Dir::new("ogv-named");
    let tools_dir = Dir::new("ogv-named-tools");
    let record = tools_dir.path.join("args");
    // `ffmpeg … -i <file> … <draft>`: its arguments, and the file copied
    // into the draft.
    let ffmpeg = script(
        &tools_dir,
        "ffmpeg",
        &format!(
            r#"printf '%s\n' "$@" > '{rec}'
for a in "$@"; do last="$a"; done
cp "$6" "$last""#,
            rec = record.display()
        ),
    );
    let tools = Tools::none().with(Program::Ffmpeg, ffmpeg);
    // No magic lofty could name: the name says Ogg, and an Ogg that is not
    // a sound is the remux's.
    let film = dir.file("film.ogv", &[0_u8; 64]);
    let edits = [
        Edit::SetTag {
            origin: Origin::Ffprobe { stream: None },
            name: "comment".to_owned(),
            value: "made here".to_owned(),
        },
        Edit::RemoveTag {
            origin: Origin::Ffprobe { stream: Some(1) },
            name: "language".to_owned(),
        },
    ];
    apply(&film, stamp_of(&film), &edits, &tools).expect("remuxed");
    let args = std::fs::read_to_string(&record).expect("ffmpeg ran");
    let args: Vec<&str> = args.lines().collect();
    let pairs: Vec<(&str, &str)> = args.windows(2).map(|w| (w[0], w[1])).collect();
    for want in [
        ("-metadata", "comment=made here"),
        ("-metadata:s", "comment=made here"),
        ("-metadata:s:1", "language="),
    ] {
        assert!(pairs.contains(&want), "{want:?} in {args:?}");
    }
    assert!(
        !pairs.contains(&("-metadata", "language=")),
        "a stream's tag stays in its stream: {args:?}"
    );
    assert_eq!(dir.names(), ["film.ogv"]);
}

/// FAILS IF: a Matroska tag added or taken out by its name stops going
/// through the read-then-replace path, which hands every other global tag
/// back.
#[test]
fn a_matroska_tag_is_added_and_taken_out_by_its_name() {
    if !Path::new("/bin/sh").exists() {
        return;
    }
    let dir = Dir::new("mkv-named");
    let tools_dir = Dir::new("mkv-named-tools");
    let extract = script(
        &tools_dir,
        "mkvextract",
        r#"cat > "$3" <<'XML'
<?xml version="1.0"?>
<Tags>
  <Tag><Targets/>
    <Simple><Name>ARTIST</Name><String>Someone</String></Simple>
    <Simple><Name>RIPPER</Name><String>gone soon</String></Simple>
  </Tag>
</Tags>
XML"#,
    );
    let record = tools_dir.path.join("args");
    let propedit = script(
        &tools_dir,
        "mkvpropedit",
        &format!(
            r#"prev=''
for a in "$@"; do
  if [ "$prev" = '--tags' ]; then cp "${{a#global:}}" '{rec}.xml'; fi
  prev="$a"
done"#,
            rec = record.display()
        ),
    );
    let tools = Tools::none()
        .with(Program::Mkvextract, extract)
        .with(Program::Mkvpropedit, propedit);
    let mut ebml = vec![0x1A, 0x45, 0xDF, 0xA3];
    ebml.extend(std::iter::repeat_n(0_u8, 60));
    let film = dir.file("a.mkv", &ebml);
    let global = Origin::Matroska {
        target: crate::sheet::Target::Global,
    };
    let edits = [
        Edit::SetTag {
            origin: global.clone(),
            name: "MOOD".to_owned(),
            value: "calm & slow".to_owned(),
        },
        Edit::RemoveTag {
            origin: global,
            name: "RIPPER".to_owned(),
        },
    ];
    apply(&film, stamp_of(&film), &edits, &tools).expect("written");
    let xml = std::fs::read_to_string(format!("{}.xml", record.display())).expect("the tags file");
    let tags = crate::matroska::parse_tags(&xml).expect("our XML");
    let names: Vec<(&str, Option<&str>)> = tags.blocks[0]
        .simples
        .iter()
        .map(|s| (s.name.as_str(), s.string.as_deref()))
        .collect();
    assert_eq!(
        names,
        [("ARTIST", Some("Someone")), ("MOOD", Some("calm & slow"))],
        "{xml}"
    );
}

#[test]
fn a_stale_stamp_changes_nothing() {
    let dir = Dir::new("stale");
    let file = dir.file("song.mp3", &fixture::mp3());
    let mut stamp = stamp_of(&file);
    stamp.mtime_ns -= 1;
    let bytes = std::fs::read(&file).expect("read");
    let edit = [Edit::Set {
        field: Field::Title,
        value: "x".to_owned(),
    }];
    assert_eq!(
        apply(&file, stamp, &edit, &Tools::none()),
        Err(Why::ChangedSince)
    );
    assert_eq!(std::fs::read(&file).expect("read"), bytes);
    assert_eq!(dir.names(), ["song.mp3"]);
}

#[test]
fn a_write_that_fails_halfway_leaves_no_draft() {
    let dir = Dir::new("broken");
    let file = dir.file("broken.flac", &fixture::broken_flac());
    let bytes = std::fs::read(&file).expect("read");
    let edit = [Edit::Set {
        field: Field::Title,
        value: "x".to_owned(),
    }];
    // The draft is made, lofty cannot read it, and it goes.
    assert_eq!(
        apply(&file, stamp_of(&file), &edit, &Tools::none()),
        Err(Why::Unreadable)
    );
    assert_eq!(dir.names(), ["broken.flac"]);
    assert_eq!(std::fs::read(&file).expect("read"), bytes);
}

#[test]
fn a_raised_stop_leaves_the_file_and_no_draft() {
    let dir = Dir::new("stop");
    let file = dir.file("song.mp3", &fixture::mp3());
    let bytes = std::fs::read(&file).expect("read");
    let stop = AtomicBool::new(true);
    let watch = Watch {
        stop: Some(&stop),
        progress: None,
    };
    let edit = [Edit::Set {
        field: Field::Title,
        value: "x".to_owned(),
    }];
    assert_eq!(
        apply_watched(&file, stamp_of(&file), &edit, &Tools::none(), &watch),
        Err(Why::Cancelled)
    );
    assert_eq!(dir.names(), ["song.mp3"]);
    assert_eq!(std::fs::read(&file).expect("read"), bytes);
}

#[test]
fn progress_ends_at_a_thousand() {
    let dir = Dir::new("progress");
    let file = dir.file("song.mp3", &fixture::mp3());
    let seen = std::cell::RefCell::new(Vec::new());
    let tell = |p: u16| seen.borrow_mut().push(p);
    let watch = Watch {
        stop: None,
        progress: Some(&tell),
    };
    let edit = [Edit::Set {
        field: Field::Title,
        value: "x".to_owned(),
    }];
    apply_watched(&file, stamp_of(&file), &edit, &Tools::none(), &watch).expect("written");
    let seen = seen.into_inner();
    assert_eq!(seen.last(), Some(&1000));
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "{seen:?}");
}

#[test]
fn a_file_with_other_names_or_marked_read_only_is_refused() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = Dir::new("guards");
    let edit = [Edit::Set {
        field: Field::Title,
        value: "x".to_owned(),
    }];
    let linked = dir.file("a.mp3", &fixture::mp3());
    std::fs::hard_link(&linked, dir.path.join("b.mp3")).expect("a hard link");
    assert_eq!(
        apply(&linked, stamp_of(&linked), &edit, &Tools::none()),
        Err(Why::HardLinked)
    );

    let locked = dir.file("c.mp3", &fixture::mp3());
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).expect("chmod");
    assert_eq!(
        apply(&locked, stamp_of(&locked), &edit, &Tools::none()),
        Err(Why::ReadOnlyFile)
    );
    assert_eq!(dir.names(), ["a.mp3", "b.mp3", "c.mp3"]);
}

#[test]
fn a_draft_left_by_a_dead_process_is_swept() {
    let dir = Dir::new("sweep");
    let file = dir.file("song.mp3", &fixture::mp3());
    // No process has this pid: it is past the kernel's ceiling.
    dir.file(".song.mp3.ronoctua-4194305.part", b"old");
    dir.file(".other.mp3.ronoctua-4194305.part", b"not this file's");
    let edit = [Edit::Set {
        field: Field::Title,
        value: "x".to_owned(),
    }];
    apply(&file, stamp_of(&file), &edit, &Tools::none()).expect("written");
    assert_eq!(
        dir.names(),
        [".other.mp3.ronoctua-4194305.part", "song.mp3"]
    );
}

#[test]
fn a_growing_moov_moves_the_chunk_offsets_with_the_sound() {
    let dir = Dir::new("moov");
    let file = dir.file("song.m4a", &fixture::m4a());
    let before = std::fs::read(&file).expect("read");
    let at = fixture::first_chunk_offset(&before).expect("an stco") as usize;
    assert_eq!(
        &before[at..at + fixture::SAMPLE_MARK.len()],
        fixture::SAMPLE_MARK
    );

    let cover = png(&dir);
    let long = "la ".repeat(40_000);
    let edits = [
        Edit::Set {
            field: Field::Lyrics,
            value: long,
        },
        Edit::Cover(CoverEdit::Replace { from: cover }),
    ];
    apply(&file, stamp_of(&file), &edits, &Tools::none()).expect("written");
    let after = std::fs::read(&file).expect("read");
    let moved = fixture::first_chunk_offset(&after).expect("an stco") as usize;
    assert!(
        moved > at + 100_000,
        "the moov grew in front of the sound: {at} -> {moved}"
    );
    assert_eq!(
        &after[moved..moved + fixture::SAMPLE_MARK.len()],
        fixture::SAMPLE_MARK
    );
}

#[test]
fn season_and_episode_live_in_their_own_mp4_atoms() {
    let dir = Dir::new("tv");
    let file = dir.file("episode.m4a", &fixture::m4a());
    let tools = Tools::none();
    let edits = [
        Edit::Set {
            field: Field::Season,
            value: "02".to_owned(),
        },
        Edit::Set {
            field: Field::Episode,
            value: "5".to_owned(),
        },
        Edit::Set {
            field: Field::Show,
            value: "A Show".to_owned(),
        },
    ];
    apply(&file, stamp_of(&file), &edits, &tools).expect("written");
    let s = sheet(&file, &tools).expect("read");
    let value = |f: Field| s.field(f).and_then(|r| r.value.clone());
    assert_eq!(value(Field::Season).as_deref(), Some("2"));
    assert_eq!(value(Field::Episode).as_deref(), Some("5"));
    assert_eq!(value(Field::Show).as_deref(), Some("A Show"));
    assert!(
        s.other
            .iter()
            .any(|t| t.name == "tvsn" && t.value == TagValue::Text("2".to_owned()))
    );
}

#[test]
fn a_field_the_tag_has_no_place_for_is_refused_before_a_draft() {
    let dir = Dir::new("noplace");
    let file = dir.file("song.mp3", &fixture::mp3());
    let s = sheet(&file, &Tools::none()).expect("read");
    let edit = [Edit::Set {
        field: Field::Season,
        value: "2".to_owned(),
    }];
    assert_eq!(
        apply(&file, s.stamp, &edit, &Tools::none()),
        Err(Why::NoPlace)
    );
    assert_eq!(dir.names(), ["song.mp3"]);
    let bad = [Edit::Set {
        field: Field::Date,
        value: "2023-02-29".to_owned(),
    }];
    assert_eq!(
        apply(&file, s.stamp, &bad, &Tools::none()),
        Err(Why::BadDate)
    );
}

#[test]
fn nothing_asked_writes_nothing() {
    let dir = Dir::new("nothing");
    let file = dir.file("song.mp3", &fixture::mp3());
    let stamp = stamp_of(&file);
    assert_eq!(apply(&file, stamp, &[], &Tools::none()), Ok(stamp));
    assert_eq!(stamp_of(&file), stamp);
}

#[test]
fn a_row_read_to_be_shown_is_not_written() {
    let dir = Dir::new("view");
    let file = dir.file("song.mp3", &fixture::mp3());
    let edit = [Edit::SetTag {
        origin: Origin::Exiftool {
            group: "ID3v2_4".to_owned(),
        },
        name: "Title".to_owned(),
        value: "x".to_owned(),
    }];
    assert_eq!(
        apply(&file, stamp_of(&file), &edit, &Tools::none()),
        Err(Why::ReadOnlyView)
    );
}

#[test]
fn a_cover_comes_out_beside_the_file_and_never_over_another() {
    let dir = Dir::new("save-cover");
    let file = dir.file("song.mp3", &fixture::mp3());
    let cover = png(&dir);
    let tools = Tools::none();
    let stamp = apply(
        &file,
        stamp_of(&file),
        &[Edit::Cover(CoverEdit::Replace { from: cover })],
        &tools,
    )
    .expect("a cover");
    let s = sheet(&file, &tools).expect("read");
    let at = s.covers[0].at.clone();
    assert!(matches!(
        at,
        CoverAt::Tag {
            format: TagFormat::Id3v2,
            index: 0
        }
    ));
    let first = save_cover(&file, &at, &tools).expect("saved");
    assert_eq!(first, dir.path.join("cover.png"));
    let second = save_cover(&file, &at, &tools).expect("saved again");
    assert_eq!(second, dir.path.join("song.cover.png"));
    assert_eq!(save_cover(&file, &at, &tools), Err(Why::Taken));
    let saved = std::fs::read(&first).expect("read");
    assert_eq!(crate::picture::dimensions(&saved), Some((4, 4)));

    // And away.
    apply(&file, stamp, &[Edit::Cover(CoverEdit::Remove)], &tools).expect("removed");
    let s = sheet(&file, &tools).expect("read");
    assert!(s.covers.is_empty(), "{:?}", s.covers);
    let gone = save_cover(&file, &at, &tools);
    assert_eq!(gone, Err(Why::NoSuchThing));
}

#[test]
fn a_folder_and_nothing_are_not_files() {
    let dir = Dir::new("not-files");
    assert_eq!(sheet(&dir.path, &Tools::none()), Err(Why::NotAFile));
    assert_eq!(
        sheet(&dir.path.join("absent.mp3"), &Tools::none()),
        Err(Why::Gone)
    );
}

#[test]
fn a_sheet_without_its_programs_says_which_were_not_read() {
    let dir = Dir::new("unread");
    let file = dir.file("song.mp3", &fixture::mp3());
    let s = sheet(&file, &Tools::none()).expect("read");
    let unread: Vec<_> = s.unread.iter().map(|u| (u.source, u.why.clone())).collect();
    assert!(unread.contains(&(
        crate::sheet::Source::Ffprobe,
        Why::Missing {
            program: Program::Ffprobe
        }
    )));
    assert!(unread.contains(&(
        crate::sheet::Source::Exiftool,
        Why::Missing {
            program: Program::Exiftool
        }
    )));
    // The blank rows are still rows.
    assert_eq!(
        s.fields.len(),
        Field::common(crate::sheet::Kind::Music).len()
    );
    assert!(s.fields.iter().all(|r| r.value.is_none()));
}

#[test]
fn a_sheet_crosses_the_socket_whole() {
    let dir = Dir::new("json");
    let file = dir.file("song.flac", &fixture::flac());
    let s = sheet(&file, &Tools::none()).expect("read");
    let text = serde_json::to_string(&s).expect("it serialises");
    let back: crate::sheet::Sheet = serde_json::from_str(&text).expect("it reads back");
    assert_eq!(back, s);
}

// ---- with the real programs --------------------------------------------

fn skipped(what: &str) {
    eprintln!("skipped: RONOCTUA_FFMPEG is not set, so {what} was not tried");
}

#[test]
fn files_ffmpeg_makes_round_trip_and_still_decode() {
    let Some(ffmpeg) = fixture::ffmpeg() else {
        return skipped("a write into a real encoder's file");
    };
    let dir = Dir::new("ffmpeg-made");
    let cover = png(&dir);
    let tools = Tools::from_env();
    for (name, args) in [
        ("a.mp3", &["-c:a", "libmp3lame"][..]),
        ("a.flac", &["-c:a", "flac"][..]),
        ("a.ogg", &["-c:a", "libvorbis"][..]),
        ("a.opus", &["-c:a", "libopus"][..]),
        ("a.m4a", &["-c:a", "aac"][..]),
    ] {
        let file = fixture::made_by_ffmpeg(&ffmpeg, &dir, name, args);
        let edits = [
            Edit::Set {
                field: Field::Title,
                value: "Written here".to_owned(),
            },
            Edit::Set {
                field: Field::Track,
                value: "7/9".to_owned(),
            },
            Edit::Cover(CoverEdit::Replace {
                from: cover.clone(),
            }),
        ];
        apply(&file, stamp_of(&file), &edits, &tools).unwrap_or_else(|w| panic!("{name}: {w:?}"));
        let s = sheet(&file, &tools).expect("read");
        assert_eq!(
            s.field(Field::Title)
                .and_then(|r| r.value.clone())
                .as_deref(),
            Some("Written here"),
            "{name}"
        );
        // THE REAL EXIFTOOL, when the variable names one (the Nix `tests`
        // derivation sets it): its JSON is read, not merely asked for. Every
        // parse test above is a recorded answer; this is the one place a
        // changed exiftool would show — as rows gone missing, which the sheet
        // would otherwise swallow into `unread` without a word.
        if tools.has(Program::Exiftool) {
            assert!(
                !s.unread.iter().any(|u| u.source == Source::Exiftool),
                "{name}: exiftool was not read: {:?}",
                s.unread
            );
            assert!(
                s.other
                    .iter()
                    .any(|t| matches!(t.origin, Origin::Exiftool { .. })),
                "{name}: exiftool gave no row"
            );
        }
        let target = file.to_string_lossy().into_owned();
        let (ok, said) =
            fixture::ffmpeg_run(&ffmpeg, &["-i", &target, "-map", "0:a", "-f", "null", "-"]);
        assert!(
            ok && said.trim().is_empty(),
            "{name} does not decode cleanly: {said}"
        );
    }
}

/// The MD5 of every decoded stream, as ffmpeg's `md5` muxer writes it.
fn decoded_md5(ffmpeg: &Path, file: &Path) -> String {
    let out = std::process::Command::new(ffmpeg)
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(file)
        // `V`: the film's pictures and not the cover, which ffmpeg shows
        // as a picture stream of its own once there is one.
        .args(["-map", "0:V", "-map", "0:a", "-f", "md5", "-"])
        .output()
        .expect("ffmpeg starts");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn a_faststart_mp4_still_decodes_the_same_after_its_moov_grows() {
    let Some(ffmpeg) = fixture::ffmpeg() else {
        return skipped("a faststart MP4 whose moov grows");
    };
    let dir = Dir::new("faststart");
    let film = dir.path.join("film.mp4");
    let target = film.to_string_lossy().into_owned();
    let (ok, said) = fixture::ffmpeg_run(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=160x90:rate=25:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=330:duration=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            &target,
        ],
    );
    assert!(ok, "{said}");
    let bytes = std::fs::read(&film).expect("read");
    let before_offset = fixture::first_chunk_offset(&bytes).expect("an stco");
    let before = decoded_md5(&ffmpeg, &film);

    let cover = dir.file(
        "big.png",
        &crate::picture::fixture::image(600, 600, image::ImageFormat::Png),
    );
    let edits = [
        Edit::Set {
            field: Field::Description,
            value: "a film ".repeat(30_000),
        },
        Edit::Cover(CoverEdit::Replace { from: cover }),
    ];
    apply(&film, stamp_of(&film), &edits, &Tools::from_env()).expect("written");
    let bytes = std::fs::read(&film).expect("read");
    let after_offset = fixture::first_chunk_offset(&bytes).expect("an stco");
    assert!(
        after_offset > before_offset,
        "{before_offset} -> {after_offset}"
    );
    assert_eq!(
        decoded_md5(&ffmpeg, &film),
        before,
        "every frame and sample is the same"
    );
}

#[test]
fn an_ogg_film_is_remuxed_with_its_new_title() {
    let Some(ffmpeg) = fixture::ffmpeg() else {
        return skipped("the Ogg remux");
    };
    let dir = Dir::new("ogv");
    let film = dir.path.join("film.ogv");
    let target = film.to_string_lossy().into_owned();
    let (ok, said) = fixture::ffmpeg_run(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x48:rate=10:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=1",
            "-c:v",
            "libtheora",
            "-c:a",
            "libvorbis",
            "-metadata",
            "title=Old",
            &target,
        ],
    );
    assert!(ok, "{said}");
    let tools = Tools::from_env();
    let edit = [Edit::Set {
        field: Field::Title,
        value: "New".to_owned(),
    }];
    apply(&film, stamp_of(&film), &edit, &tools).expect("remuxed");
    assert_eq!(dir.names(), ["film.ogv"]);
    if !tools.has(Program::Ffprobe) {
        eprintln!("skipped: RONOCTUA_FFPROBE is not set, so the remuxed title was not read back");
        return;
    }
    let s = sheet(&film, &tools).expect("read");
    assert_eq!(s.container, Container::OggOther);
    assert_eq!(
        s.field(Field::Title)
            .and_then(|r| r.value.clone())
            .as_deref(),
        Some("New")
    );
    assert_eq!(s.streams.len(), 2, "both streams are still there");
    assert_eq!(
        s.field(Field::Title).map(|r| r.can.clone()),
        Some(Can::Yes(Writer::Remux))
    );
}

/// FAILS IF: a real Ogg film stops taking a tag by its own name through the
/// remux — added in the container, read back by ffprobe as a row that can
/// be written, and taken out again — or loses a stream on the way.
#[test]
fn an_ogg_film_takes_a_tag_by_its_name_and_gives_it_back() {
    let Some(ffmpeg) = fixture::ffmpeg() else {
        return skipped("a tag by name through the Ogg remux");
    };
    let tools = Tools::from_env();
    if !tools.has(Program::Ffprobe) {
        eprintln!("skipped: RONOCTUA_FFPROBE is not set, so a remuxed tag was not read back");
        return;
    }
    let dir = Dir::new("ogv-tag");
    let film = dir.path.join("film.ogv");
    let target = film.to_string_lossy().into_owned();
    let (ok, said) = fixture::ffmpeg_run(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x48:rate=10:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=1",
            "-c:v",
            "libtheora",
            "-c:a",
            "libvorbis",
            &target,
        ],
    );
    assert!(ok, "{said}");
    let s = sheet(&film, &tools).expect("read");
    let adding = s.adding.clone().expect("where a tag goes");
    assert_eq!(adding.can, Can::Yes(Writer::Remux));
    let origin = adding.origin.expect("the container's");
    let stamp = apply(
        &film,
        s.stamp,
        &[Edit::SetTag {
            origin,
            name: "OCCASION".to_owned(),
            value: "made here".to_owned(),
        }],
        &tools,
    )
    .expect("remuxed");
    let s = sheet(&film, &tools).expect("read");
    let rows: Vec<_> = s
        .other
        .iter()
        .filter(|t| {
            t.name.eq_ignore_ascii_case("OCCASION") && matches!(t.origin, Origin::Ffprobe { .. })
        })
        .collect();
    assert!(!rows.is_empty(), "read back: {:?}", s.other);
    assert!(
        rows.iter()
            .all(|t| t.value == TagValue::Text("made here".to_owned()))
    );
    assert!(rows.iter().all(|t| t.can == Can::Yes(Writer::Remux)));
    assert_eq!(s.streams.len(), 2, "both streams are still there");
    let edits: Vec<Edit> = rows
        .iter()
        .map(|t| Edit::RemoveTag {
            origin: t.origin.clone(),
            name: t.name.clone(),
        })
        .collect();
    apply(&film, stamp, &edits, &tools).expect("taken out");
    let s = sheet(&film, &tools).expect("read");
    assert!(
        !s.other
            .iter()
            .any(|t| t.name.eq_ignore_ascii_case("OCCASION")
                && matches!(t.origin, Origin::Ffprobe { .. })),
        "{:?}",
        s.other
    );
    assert_eq!(dir.names(), ["film.ogv"]);
}

#[test]
fn a_matroska_file_without_its_tools_says_which_one_is_missing() {
    let Some(ffmpeg) = fixture::ffmpeg() else {
        return skipped("the Matroska refusal");
    };
    let dir = Dir::new("mkv");
    let file = fixture::made_by_ffmpeg(
        &ffmpeg,
        &dir,
        "a.mka",
        &["-c:a", "flac", "-metadata", "artist=Someone"],
    );
    let tools = Tools::from_env()
        .without(Program::Mkvextract)
        .without(Program::Mkvmerge)
        .without(Program::Mkvpropedit);
    let s = sheet(&file, &tools).expect("read");
    assert_eq!(s.container, Container::Matroska);
    let missing = Can::No(Why::Missing {
        program: Program::Mkvextract,
    });
    assert_eq!(s.field(Field::Artist).map(|r| r.can.clone()), Some(missing));
    if tools.has(Program::Ffprobe) {
        assert_eq!(
            s.field(Field::Artist)
                .and_then(|r| r.value.clone())
                .as_deref(),
            Some("Someone"),
            "read through ffprobe while the tools are away"
        );
    }
    let edit = [Edit::Set {
        field: Field::Artist,
        value: "x".to_owned(),
    }];
    assert_eq!(
        apply(&file, s.stamp, &edit, &tools),
        Err(Why::Missing {
            program: Program::Mkvextract
        })
    );
    // WITH NOTHING AT ALL, lofty's search finds an AAC frame sync inside the
    // Matroska header; believed, it would write an ID3v2 tag in front of it.
    // The name decides instead, and the file is untouched.
    let bytes = std::fs::read(&file).expect("read");
    assert_eq!(
        apply(&file, s.stamp, &edit, &Tools::none()),
        Err(Why::Missing {
            program: Program::Mkvextract
        })
    );
    assert_eq!(std::fs::read(&file).expect("read"), bytes);
}

#[test]
fn a_matroska_cover_comes_out_through_ffmpeg() {
    let Some(ffmpeg) = fixture::ffmpeg() else {
        return skipped("a Matroska attachment taken out");
    };
    let tools = Tools::from_env();
    if !tools.has(Program::Ffprobe) {
        eprintln!("skipped: RONOCTUA_FFPROBE is not set, so a Matroska cover was not looked for");
        return;
    }
    let dir = Dir::new("mkv-cover");
    let png_path = png(&dir);
    let png_bytes = std::fs::read(&png_path).expect("read");
    let target = dir.path.join("a.mka");
    let target_text = target.to_string_lossy().into_owned();
    let png_text = png_path.to_string_lossy().into_owned();
    let (ok, said) = fixture::ffmpeg_run(
        &ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=duration=0.5",
            "-c:a",
            "flac",
            "-attach",
            &png_text,
            "-metadata:s:t",
            "mimetype=image/png",
            "-metadata:s:t",
            "filename=cover.png",
            &target_text,
        ],
    );
    assert!(ok, "{said}");
    let s = sheet(&target, &tools).expect("read");
    let front = s
        .covers
        .iter()
        .find(|c| c.role == CoverRole::Front)
        .unwrap_or_else(|| panic!("the attachment is the front cover: {:?}", s.covers));
    assert!(matches!(&front.at, CoverAt::Stream { name: Some(n), .. } if n == "cover.png"));
    let picture = crate::read::cover(&target, &front.at, &tools).expect("taken out");
    assert_eq!(picture.bytes, png_bytes);
    assert_eq!(picture.extension, "png");
    let saved = save_cover(&target, &front.at, &tools).expect("saved");
    assert_eq!(std::fs::read(saved).expect("read"), png_bytes);
}

/// A stand-in for a program: a shell script that does what the test says.
///
/// WRITTEN BY A CHILD, never by this process. The tests run in parallel, and
/// a `fork` in another test's thread while this one held the script open for
/// writing hands the child that descriptor until it execs; `execve` of a file
/// somebody holds for writing fails with ETXTBSY ("Text file busy"), and the
/// stand-in "failed" at random — seen once in a whole-workspace run, as
/// `ProgramFailed { program: Mkvextract }`.
fn script(dir: &Dir, name: &str, body: &str) -> std::path::PathBuf {
    use std::io::Write as _;
    let path = dir.path.join(name);
    let mut child = std::process::Command::new("/bin/sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
        .arg(&path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("a shell writes the stand-in");
    child
        .stdin
        .take()
        .expect("its input")
        .write_all(format!("#!/bin/sh\n{body}\n").as_bytes())
        .expect("the stand-in's text");
    assert!(child.wait().expect("the shell ends").success());
    path
}

#[test]
fn the_matroska_writer_hands_mkvpropedit_every_global_tag() {
    if !Path::new("/bin/sh").exists() {
        return;
    }
    let dir = Dir::new("mkv-plan");
    let tools_dir = Dir::new("mkv-tools");
    // `mkvextract <file> tags <out>`: the tags a ripper left.
    let extract = script(
        &tools_dir,
        "mkvextract",
        r#"cat > "$3" <<'XML'
<?xml version="1.0"?>
<Tags>
  <Tag><Targets/>
    <Simple><Name>ARTIST</Name><String>Old</String></Simple>
    <Simple><Name>RIPPER</Name><String>kept</String></Simple>
  </Tag>
  <Tag><Targets><TrackUID>7</TrackUID></Targets>
    <Simple><Name>BPS</Name><String>1</String></Simple>
  </Tag>
</Tags>
XML"#,
    );
    // `mkvpropedit`: its arguments, and the tags file it was handed.
    let record = tools_dir.path.join("args");
    let propedit = script(
        &tools_dir,
        "mkvpropedit",
        &format!(
            r#"printf '%s\n' "$@" > '{rec}'
prev=''
for a in "$@"; do
  if [ "$prev" = '--tags' ]; then cp "${{a#global:}}" '{rec}.xml'; fi
  prev="$a"
done"#,
            rec = record.display()
        ),
    );
    let tools = Tools::none()
        .with(Program::Mkvextract, extract)
        .with(Program::Mkvpropedit, propedit);
    let mut ebml = vec![0x1A, 0x45, 0xDF, 0xA3];
    ebml.extend(std::iter::repeat_n(0_u8, 60));
    let film = dir.file("a.mkv", &ebml);
    let edits = [
        Edit::Set {
            field: Field::Title,
            value: "New = title".to_owned(),
        },
        Edit::Set {
            field: Field::Artist,
            value: "Someone".to_owned(),
        },
    ];
    apply(&film, stamp_of(&film), &edits, &tools).expect("written");
    let args = std::fs::read_to_string(&record).expect("mkvpropedit ran");
    let args: Vec<&str> = args.lines().collect();
    assert!(
        args[0].contains("/.a.mkv.ronoctua-") && args[0].ends_with(".part"),
        "the draft, never the file: {args:?}"
    );
    assert_eq!(args[1..5], ["--edit", "info", "--set", "title=New = title"]);
    assert_eq!(args[5], "--tags");
    assert!(args[6].starts_with("global:"), "{args:?}");
    let xml = std::fs::read_to_string(format!("{}.xml", record.display())).expect("the tags file");
    let tags = crate::matroska::parse_tags(&xml).expect("our XML");
    assert_eq!(
        crate::matroska::field_value(&tags, Field::Artist).as_deref(),
        Some("Someone")
    );
    assert!(
        xml.contains("RIPPER"),
        "a tag nobody touched is handed back: {xml}"
    );
    assert!(!xml.contains("BPS"), "a track's tag is not global: {xml}");
    // The draft was renamed over the file; nothing else is left beside it.
    assert_eq!(dir.names(), ["a.mkv"]);
}

#[test]
fn a_matroska_track_is_named_by_its_order_among_tracks() {
    if !Path::new("/bin/sh").exists() {
        return;
    }
    let dir = Dir::new("mkv-tracks");
    let tools_dir = Dir::new("mkv-tracks-tools");
    // ffprobe's answer: a film, a sound, a font and the cover ffmpeg shows
    // as a picture stream. Tracks are the first two only.
    let probe = script(
        &tools_dir,
        "ffprobe",
        r#"cat <<'JSON'
{"streams":[
 {"index":0,"codec_type":"video","codec_name":"h264"},
 {"index":1,"codec_type":"audio","codec_name":"opus"},
 {"index":2,"codec_type":"attachment","tags":{"filename":"font.ttf"}},
 {"index":3,"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1},
  "tags":{"filename":"cover.jpg","mimetype":"image/jpeg"}}
],"format":{"format_name":"matroska,webm"}}
JSON"#,
    );
    let record = tools_dir.path.join("args");
    let propedit = script(
        &tools_dir,
        "mkvpropedit",
        &format!(r#"printf '%s\n' "$@" > '{}'"#, record.display()),
    );
    let extract = script(&tools_dir, "mkvextract", "exit 0");
    let tools = Tools::none()
        .with(Program::Ffprobe, probe)
        .with(Program::Mkvextract, extract)
        .with(Program::Mkvpropedit, propedit);
    let film = dir.file("b.mkv", &[0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0]);
    let edits = [
        Edit::Stream {
            index: 1,
            change: crate::sheet::StreamChange::Language("pt-BR".to_owned()),
        },
        Edit::Stream {
            index: 1,
            change: crate::sheet::StreamChange::Title(String::new()),
        },
        Edit::Cover(CoverEdit::Remove),
    ];
    apply(&film, stamp_of(&film), &edits, &tools).expect("written");
    let args = std::fs::read_to_string(&record).expect("mkvpropedit ran");
    let args: Vec<&str> = args.lines().skip(1).collect();
    assert_eq!(
        args,
        [
            "--edit",
            "track:2",
            "--set",
            "language=pt-BR",
            "--delete",
            "name",
            "--delete-attachment",
            "name:cover.jpg",
        ]
    );
    let bad = [Edit::Stream {
        index: 1,
        change: crate::sheet::StreamChange::Language("Portuguese".to_owned()),
    }];
    assert_eq!(
        apply(&film, stamp_of(&film), &bad, &tools),
        Err(Why::BadLanguage)
    );
    let attachment = [Edit::Stream {
        index: 2,
        change: crate::sheet::StreamChange::Default(true),
    }];
    assert_eq!(
        apply(&film, stamp_of(&film), &attachment, &tools),
        Err(Why::NoSuchThing),
        "an attachment is not a track"
    );
}

/// FAILS IF: ffprobe's rows stop saying who writes them in the file's own
/// terms — an Ogg film's ARE its tags and are written by the remux, an
/// AVI's say nothing writes an AVI — or "Add a tag" stops going where the
/// file's writer puts it, or saying why it cannot.
#[test]
fn ffprobe_s_rows_are_written_where_a_remux_writes_them_and_say_why_elsewhere() {
    if !Path::new("/bin/sh").exists() {
        return;
    }
    let tools_dir = Dir::new("probe-rows-tools");
    let probe = |format: &str| {
        script(
            &tools_dir,
            &format!("ffprobe-{format}"),
            &format!(
                r#"cat <<'JSON'
{{"streams":[
 {{"index":0,"codec_type":"video","codec_name":"theora","tags":{{"ENCODER":"x"}}}},
 {{"index":1,"codec_type":"audio","codec_name":"vorbis","tags":{{"language":"por"}}}}
],"format":{{"format_name":"{format}","tags":{{"title":"Old"}}}}}}
JSON"#
            ),
        )
    };
    let ffmpeg = script(&tools_dir, "ffmpeg", "exit 0");
    let dir = Dir::new("probe-rows");
    for (name, format, container) in [
        ("film.ogv", "ogg", Container::OggOther),
        ("film.avi", "avi", Container::Avi),
    ] {
        let tools = Tools::none()
            .with(Program::Ffprobe, probe(format))
            .with(Program::Ffmpeg, ffmpeg.clone());
        let file = dir.file(name, &[0_u8; 64]);
        let s = sheet(&file, &tools).expect("read");
        assert_eq!(s.container, container, "{name}");
        let rows: Vec<_> = s
            .other
            .iter()
            .filter(|t| matches!(t.origin, Origin::Ffprobe { .. }))
            .collect();
        assert_eq!(rows.len(), 3, "{name}: {:?}", s.other);
        let adding = s.adding.clone().expect("a sheet says where a tag goes");
        if container == Container::OggOther {
            assert!(
                rows.iter().all(|t| t.can == Can::Yes(Writer::Remux)),
                "{rows:?}"
            );
            assert_eq!(adding.origin, Some(Origin::Ffprobe { stream: None }));
            assert_eq!(adding.can, Can::Yes(Writer::Remux));
            // AND WITHOUT FFMPEG, the program is the reason.
            let without = Tools::none().with(Program::Ffprobe, probe(format));
            let s = sheet(&file, &without).expect("read");
            let missing = Can::No(Why::Missing {
                program: Program::Ffmpeg,
            });
            assert!(
                s.other
                    .iter()
                    .filter(|t| matches!(t.origin, Origin::Ffprobe { .. }))
                    .all(|t| t.can == missing)
            );
        } else {
            let nothing = Can::No(Why::NoWriter { container });
            assert!(rows.iter().all(|t| t.can == nothing), "{rows:?}");
            assert_eq!(adding.origin, None);
            assert_eq!(adding.can, nothing);
        }
    }
}
