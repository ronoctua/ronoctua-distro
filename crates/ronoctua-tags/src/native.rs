//! lofty, held by the concrete tag types.
//!
//! # Why not `TaggedFile`
//!
//! lofty's generic file converts every tag into its generic `Tag` on the way
//! in, and for Vorbis comments that conversion DROPS every key it has no
//! `ItemKey` for — `SOURCE`, `RIPPER`, whatever a tagger invented. Saving the
//! generic file would erase them. So a file is opened as its concrete type,
//! each tag is kept concrete, and a field is edited by splitting the tag
//! (`split_tag` keeps the unmapped part as the remainder), editing the
//! generic half and merging the two back — the remainder rides through
//! untouched.
//!
//! # FLAC's pictures
//!
//! A FLAC file keeps its pictures in PICTURE blocks beside the Vorbis
//! comment block, and lofty writes both from the comment tag: saving a
//! comment tag with no pictures DELETES every PICTURE block. So the file's
//! pictures are moved into its comment tag when it is opened, exactly as
//! lofty's own `From<FlacFile> for TaggedFile` does.
//!
//! # Memory
//!
//! lofty rewrites a file by reading the rest of it into memory (the MP4
//! writer reads the WHOLE file). [`LOFTY_MOST_BYTES`] is the ceiling on
//! what a write may be asked to hold, and a bigger file says so on its sheet
//! instead of being tried.

use std::fs::File;
use std::io::Seek as _;
use std::path::Path;

use lofty::ape::{ApeFile, ApeItem, ApeTag};
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile as _, FileType};
use lofty::flac::FlacFile;
use lofty::id3::v1::Id3v1Tag;
use lofty::id3::v2::{Frame, Id3v2Tag, Id3v2Version};
use lofty::iff::aiff::{AiffFile, AiffTextChunks};
use lofty::iff::wav::{RiffInfoList, WavFile};
use lofty::mp4::{Atom, AtomData, AtomIdent, Ilst, Mp4File};
use lofty::mpeg::MpegFile;
use lofty::musepack::MpcFile;
use lofty::ogg::tag::VorbisComments;
use lofty::ogg::{OggPictureStorage as _, OpusFile, SpeexFile, VorbisFile};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{
    ItemKey, ItemValue, MergeTag, SplitTag, Tag as Generic, TagExt as _, TagItem, TagType,
};
use lofty::wavpack::WavPackFile;

use crate::picture::{self, ImageKind};
use crate::sheet::{
    Can, Container, Cover, CoverAt, CoverRole, Field, Tag, TagFormat, TagValue, Why, Writer,
};

/// The most a file may be for lofty to rewrite it.
///
/// Five hundred and twelve mebibytes: lofty holds the rest of the file in
/// memory while it rewrites (all of it, for MP4), and the player is a daemon
/// that must not be the one `systemd-oomd` picks. Nearly every song is under
/// fifty; a film in MP4 is the file this refuses, and its sheet says so
/// before anyone tries.
pub(crate) const LOFTY_MOST_BYTES: u64 = 512 * 1024 * 1024;

/// How much of a tag's value the Every-tag page carries — the schema's
/// number, which a stream's sheet cuts by too.
const OTHER_VALUE_BYTES: usize = crate::sheet::TAG_VALUE_BYTES;

/// The two MP4 atoms for a season and an episode, which lofty has no
/// `ItemKey` for.
const SEASON: AtomIdent<'static> = AtomIdent::Fourcc(*b"tvsn");
const EPISODE: AtomIdent<'static> = AtomIdent::Fourcc(*b"tves");

/// One tag of a file, as its own type.
#[derive(Debug, Clone)]
pub(crate) enum Native {
    Id3v2(Id3v2Tag),
    Id3v1(Id3v1Tag),
    Ape(ApeTag),
    Vorbis(VorbisComments),
    Mp4(Ilst),
    RiffInfo(RiffInfoList),
    AiffText(AiffTextChunks),
}

/// A file's tags, as its own types.
#[derive(Debug)]
pub(crate) struct Opened {
    /// What lofty says the file is.
    pub file_type: FileType,
    /// Every tag it has.
    pub tags: Vec<Native>,
}

/// What lofty makes of a file, and whether it found it at the very start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Guess {
    pub file_type: FileType,
    /// The file begins with its own magic, an ID3v2 tag or a frame sync.
    pub at_start: bool,
}

/// What lofty makes of the first bytes, or nothing.
///
/// # Why its answer is not taken as it comes
///
/// When no magic matches, lofty SEARCHES the first kilobyte for an MPEG or
/// AAC frame sync and calls the file whatever it finds. Measured: a
/// Matroska file ffmpeg wrote came back as AAC, and an Ogg Theora film as
/// MPEG — and a writer that believed it would put an ID3v2 tag in front of
/// a Matroska header. So an MPEG or AAC answer that did not start the file
/// is marked, and [`trusted`] asks ffprobe before believing it.
pub(crate) fn guess(path: &Path) -> Option<Guess> {
    use std::io::Read as _;
    let file_type = Probe::open(path)
        .ok()?
        .guess_file_type()
        .ok()?
        .file_type()?;
    let at_start = if matches!(file_type, FileType::Mpeg | FileType::Aac) {
        let mut head = [0_u8; 3];
        File::open(path).ok()?.read_exact(&mut head).ok()?;
        &head == b"ID3" || (head[0] == 0xFF && head[1] & 0xE0 == 0xE0)
    } else {
        true
    };
    Some(Guess {
        file_type,
        at_start,
    })
}

/// lofty's guess, when it can be believed: at the start of the file, or
/// found further in and confirmed by ffprobe.
pub(crate) fn trusted(
    guess: Option<Guess>,
    probed: Option<&crate::probe::Probed>,
) -> Option<FileType> {
    let guess = guess?;
    if guess.at_start {
        return Some(guess.file_type);
    }
    let name = probed?.format_name.as_deref()?;
    let wanted = if guess.file_type == FileType::Mpeg {
        "mp3"
    } else {
        "aac"
    };
    name.split(',')
        .any(|n| n == wanted)
        .then_some(guess.file_type)
}

/// Whether lofty's guess needs ffprobe's word before it is believed.
pub(crate) fn needs_confirming(guess: Option<Guess>) -> bool {
    guess.is_none_or(|g| !g.at_start)
}

/// The container lofty's answer names.
pub(crate) const fn container_of(file_type: FileType) -> Container {
    match file_type {
        FileType::Aac => Container::Aac,
        FileType::Aiff => Container::Aiff,
        FileType::Ape => Container::Ape,
        FileType::Flac => Container::Flac,
        FileType::Mpeg => Container::Mpeg,
        FileType::Mp4 => Container::Mp4,
        FileType::Mpc => Container::Mpc,
        FileType::Opus => Container::Opus,
        FileType::Vorbis => Container::OggVorbis,
        FileType::Speex => Container::Speex,
        FileType::Wav => Container::Wav,
        FileType::WavPack => Container::WavPack,
        _ => Container::Other,
    }
}

/// How every read here parses: tags only.
///
/// `read_properties(false)` is not an optimisation: an MP4 with no sound
/// track fails to read its properties ("file contains no audio tracks"),
/// and a film's tags would be unreachable for a reason that has nothing to
/// do with them.
fn parse_options() -> ParseOptions {
    ParseOptions::new().read_properties(false)
}

/// Opens a file's tags.
///
/// # Errors
///
/// [`Why::Unreadable`] when lofty does not know the file or cannot read
/// its tags.
pub(crate) fn open(file: &mut File, file_type: FileType) -> Result<Opened, Why> {
    let opts = parse_options();
    let unreadable = |e: lofty::error::FileParseError| {
        tracing::debug!(error = %e, "lofty could not read the tags");
        Why::Unreadable
    };
    file.rewind().map_err(|_| Why::Unreadable)?;
    let mut tags = Vec::new();
    let mut push = |t: Option<Native>| tags.extend(t);
    match file_type {
        FileType::Mpeg => {
            let mut f = MpegFile::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_id3v2().map(Native::Id3v2));
            push(f.remove_ape().map(Native::Ape));
            push(f.remove_id3v1().map(Native::Id3v1));
        }
        FileType::Aac => {
            let mut f = lofty::aac::AacFile::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_id3v2().map(Native::Id3v2));
            push(f.remove_id3v1().map(Native::Id3v1));
        }
        FileType::Flac => {
            let mut f = FlacFile::read_from(file, opts).map_err(unreadable)?;
            let pictures: Vec<_> = f.pictures().to_vec();
            let mut comments = f.remove_vorbis_comments();
            if comments.is_none() && !pictures.is_empty() {
                comments = Some(VorbisComments::default());
            }
            if let Some(c) = comments.as_mut() {
                for (p, info) in pictures {
                    // Cannot fail with the information already in hand.
                    let _ = c.insert_picture(p, Some(info));
                }
            }
            push(comments.map(Native::Vorbis));
            push(f.remove_id3v2().map(Native::Id3v2));
        }
        FileType::Opus => {
            let mut f = OpusFile::read_from(file, opts).map_err(unreadable)?;
            push(Some(Native::Vorbis(f.remove_vorbis_comments())));
        }
        FileType::Vorbis => {
            let mut f = VorbisFile::read_from(file, opts).map_err(unreadable)?;
            push(Some(Native::Vorbis(f.remove_vorbis_comments())));
        }
        FileType::Speex => {
            let mut f = SpeexFile::read_from(file, opts).map_err(unreadable)?;
            push(Some(Native::Vorbis(f.remove_vorbis_comments())));
        }
        FileType::Mp4 => {
            let mut f = Mp4File::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_ilst().map(Native::Mp4));
        }
        FileType::Wav => {
            let mut f = WavFile::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_id3v2().map(Native::Id3v2));
            push(f.remove_riff_info().map(Native::RiffInfo));
        }
        FileType::Aiff => {
            let mut f = AiffFile::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_id3v2().map(Native::Id3v2));
            push(f.remove_text_chunks().map(Native::AiffText));
        }
        FileType::Ape => {
            let mut f = ApeFile::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_ape().map(Native::Ape));
            push(f.remove_id3v2().map(Native::Id3v2));
            push(f.remove_id3v1().map(Native::Id3v1));
        }
        FileType::Mpc => {
            let mut f = MpcFile::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_ape().map(Native::Ape));
            push(f.remove_id3v2().map(Native::Id3v2));
            push(f.remove_id3v1().map(Native::Id3v1));
        }
        FileType::WavPack => {
            let mut f = WavPackFile::read_from(file, opts).map_err(unreadable)?;
            push(f.remove_ape().map(Native::Ape));
            push(f.remove_id3v1().map(Native::Id3v1));
        }
        _ => return Err(Why::Unreadable),
    }
    Ok(Opened { file_type, tags })
}

/// Splits a tag, lets `edit` change the generic half, and merges it back.
///
/// The remainder — whatever the tag had that lofty's generic form cannot
/// name — is carried through untouched.
fn through<T>(tag: T, edit: impl FnOnce(&mut Generic)) -> T
where
    T: SplitTag,
    T::Remainder: MergeTag<Merged = T>,
{
    let (rest, mut generic) = tag.split_tag();
    edit(&mut generic);
    rest.merge_tag(generic)
}

/// The generic half of a tag, for reading.
fn generic_of<T>(tag: &T) -> Generic
where
    T: SplitTag + Clone,
{
    tag.clone().split_tag().1
}

impl Native {
    /// Which format it is.
    pub(crate) const fn format(&self) -> TagFormat {
        match self {
            Self::Id3v2(_) => TagFormat::Id3v2,
            Self::Id3v1(_) => TagFormat::Id3v1,
            Self::Ape(_) => TagFormat::Ape,
            Self::Vorbis(_) => TagFormat::Vorbis,
            Self::Mp4(_) => TagFormat::Mp4,
            Self::RiffInfo(_) => TagFormat::RiffInfo,
            Self::AiffText(_) => TagFormat::AiffText,
        }
    }

    /// lofty's name for its format.
    pub(crate) const fn tag_type(&self) -> TagType {
        tag_type_of(self.format())
    }

    /// An empty tag of a format.
    fn empty(format: TagFormat) -> Self {
        match format {
            TagFormat::Id3v2 => Self::Id3v2(Id3v2Tag::new()),
            TagFormat::Id3v1 => Self::Id3v1(Id3v1Tag::new()),
            TagFormat::Ape => Self::Ape(ApeTag::new()),
            TagFormat::Vorbis => Self::Vorbis(VorbisComments::new()),
            TagFormat::Mp4 => Self::Mp4(Ilst::new()),
            TagFormat::RiffInfo => Self::RiffInfo(RiffInfoList::default()),
            TagFormat::AiffText => Self::AiffText(AiffTextChunks::default()),
        }
    }

    /// Its generic half, for reading fields and pictures.
    pub(crate) fn generic(&self) -> Generic {
        match self {
            Self::Id3v2(t) => generic_of(t),
            Self::Id3v1(t) => generic_of(t),
            Self::Ape(t) => generic_of(t),
            Self::Vorbis(t) => generic_of(t),
            Self::Mp4(t) => generic_of(t),
            Self::RiffInfo(t) => generic_of(t),
            Self::AiffText(t) => generic_of(t),
        }
    }

    /// The same tag with its generic half edited.
    fn through(self, edit: impl FnOnce(&mut Generic)) -> Self {
        match self {
            Self::Id3v2(t) => Self::Id3v2(through(t, edit)),
            Self::Id3v1(t) => Self::Id3v1(through(t, edit)),
            Self::Ape(t) => Self::Ape(through(t, edit)),
            Self::Vorbis(t) => Self::Vorbis(through(t, edit)),
            Self::Mp4(t) => Self::Mp4(through(t, edit)),
            Self::RiffInfo(t) => Self::RiffInfo(through(t, edit)),
            Self::AiffText(t) => Self::AiffText(through(t, edit)),
        }
    }

    /// Writes this tag into a file lofty already knows, replacing the one
    /// of its format that is there.
    fn save(&self, file: &mut File) -> Result<(), Why> {
        let mut options = WriteOptions::new().parse_options(parse_options());
        file.rewind().map_err(|_| Why::Unwritable)?;
        let done = match self {
            Self::Id3v2(t) => {
                // THE VERSION THE FILE HAD: lofty writes 2.4 unless told, and
                // a 2.3 tag rewritten as 2.4 stops being read by the car
                // stereos and old players that were reading it.
                if t.original_version() == Id3v2Version::V3 {
                    options = options.use_id3v23(true);
                }
                t.save_to(file, options)
            }
            Self::Id3v1(t) => t.save_to(file, options),
            Self::Ape(t) => t.save_to(file, options),
            Self::Vorbis(t) => t.save_to(file, options),
            Self::Mp4(t) => t.save_to(file, options),
            Self::RiffInfo(t) => t.save_to(file, options),
            Self::AiffText(t) => t.save_to(file, options),
        };
        done.map_err(|e| {
            tracing::warn!(error = %e, "lofty could not write the tag");
            Why::Unwritable
        })
    }
}

/// lofty's name for a tag format.
pub(crate) const fn tag_type_of(format: TagFormat) -> TagType {
    match format {
        TagFormat::Id3v2 => TagType::Id3v2,
        TagFormat::Id3v1 => TagType::Id3v1,
        TagFormat::Ape => TagType::Ape,
        TagFormat::Vorbis => TagType::VorbisComments,
        TagFormat::Mp4 => TagType::Mp4Ilst,
        TagFormat::RiffInfo => TagType::RiffInfo,
        TagFormat::AiffText => TagType::AiffText,
    }
}

/// The format lofty's name names.
pub(crate) const fn format_of(tag_type: TagType) -> Option<TagFormat> {
    match tag_type {
        TagType::Id3v2 => Some(TagFormat::Id3v2),
        TagType::Id3v1 => Some(TagFormat::Id3v1),
        TagType::Ape => Some(TagFormat::Ape),
        TagType::VorbisComments => Some(TagFormat::Vorbis),
        TagType::Mp4Ilst => Some(TagFormat::Mp4),
        TagType::RiffInfo => Some(TagFormat::RiffInfo),
        TagType::AiffText => Some(TagFormat::AiffText),
        _ => None,
    }
}

/// The keys a field is read from, the first being the one it is written as
/// where the format has it.
const fn keys_of(field: Field) -> &'static [ItemKey] {
    match field {
        Field::Title => &[ItemKey::TrackTitle],
        Field::Artist => &[ItemKey::TrackArtist],
        Field::Album => &[ItemKey::AlbumTitle],
        Field::AlbumArtist => &[ItemKey::AlbumArtist],
        Field::Track => &[ItemKey::TrackNumber],
        Field::Disc => &[ItemKey::DiscNumber],
        // YEAR is ID3v1-era and Vorbis's second spelling; RELEASEDATE is
        // read when nothing else says, and never overwritten — a release
        // date is not a recording date.
        Field::Date => &[ItemKey::RecordingDate, ItemKey::Year, ItemKey::ReleaseDate],
        Field::Genre => &[ItemKey::Genre],
        Field::Composer => &[ItemKey::Composer],
        Field::Lyricist => &[ItemKey::Lyricist],
        Field::Comment => &[ItemKey::Comment],
        // ID3v2 has no plain `Lyrics` (its synchronised frame is SYLT); the
        // USLT frame is `UnsyncLyrics`.
        Field::Lyrics => &[ItemKey::Lyrics, ItemKey::UnsyncLyrics],
        // One frame in ID3v2 (TPUB) answers both; Vorbis and MP4 call it a
        // label.
        Field::Publisher => &[ItemKey::Publisher, ItemKey::Label],
        Field::Isrc => &[ItemKey::Isrc],
        Field::Description => &[ItemKey::Description],
        Field::Show => &[ItemKey::ShowName],
        Field::Director => &[ItemKey::Director],
        Field::Season | Field::Episode => &[],
    }
}

/// The keys a write REPLACES: every spelling but the release date.
fn replaced_keys(field: Field) -> &'static [ItemKey] {
    match field {
        Field::Date => &[ItemKey::RecordingDate, ItemKey::Year],
        other => keys_of(other),
    }
}

/// Whether a tag format has a place for a field.
pub(crate) fn has_place(format: TagFormat, field: Field) -> bool {
    let tag_type = tag_type_of(format);
    match field {
        Field::Season | Field::Episode => format == TagFormat::Mp4,
        // ID3v2's DESCRIPTION is a user text frame (`described_in_id3`).
        Field::Description if format == TagFormat::Id3v2 => true,
        other => keys_of(other).iter().any(|k| k.map_key(tag_type).is_some()),
    }
}

/// The description a user text frame of ID3v2 is kept under: `TXXX:DESCRIPTION`,
/// what the taggers that write one (Mp3tag, foobar2000, Kid3) call it.
///
/// # Why a user text frame
///
/// ID3v2 has no frame of its own for what a film or an episode is about —
/// `TDES` is iTunes' PODCAST description, and lofty's generic half has no
/// key for a description in ID3v2 at all — so an MP3 had no place for the
/// field, and the row said so. A user text frame is the place every other
/// tagger uses, and lofty keeps it in the tag's remainder, untouched by the
/// generic half: it is read and written here on the concrete tag.
const ID3_DESCRIPTION: &str = "DESCRIPTION";

/// The ID3v2 tag's description, read from its user text frame (any case).
fn id3_description(id3: &Id3v2Tag) -> Option<String> {
    id3.into_iter().find_map(|frame| match frame {
        Frame::UserText(f) if f.description.eq_ignore_ascii_case(ID3_DESCRIPTION) => {
            Some(f.content.trim().to_owned()).filter(|c| !c.is_empty())
        }
        _ => None,
    })
}

/// The description written into an ID3v2 tag's user text frame — every
/// spelling of the frame taken out first, so one is left — or taken out.
fn described_in_id3(tag: Native, fields: &[(Field, Option<String>)]) -> Native {
    let Native::Id3v2(mut id3) = tag else {
        return tag;
    };
    for (_, value) in fields.iter().filter(|(f, _)| *f == Field::Description) {
        id3.retain(|frame| {
            !matches!(frame, Frame::UserText(f) if f.description.eq_ignore_ascii_case(ID3_DESCRIPTION))
        });
        if let Some(value) = value {
            let _ = id3.insert_user_text(ID3_DESCRIPTION.to_owned(), value.clone());
        }
    }
    Native::Id3v2(id3)
}

/// A field's value in one tag.
pub(crate) fn field_value(native: &Native, generic: &Generic, field: Field) -> Option<String> {
    match field {
        Field::Description if matches!(native, Native::Id3v2(_)) => match native {
            Native::Id3v2(id3) => id3_description(id3),
            _ => None,
        },
        Field::Season | Field::Episode => {
            let Native::Mp4(ilst) = native else {
                return None;
            };
            let ident = if field == Field::Season {
                &SEASON
            } else {
                &EPISODE
            };
            ilst.get(ident)?.data().find_map(|d| match d {
                AtomData::SignedInteger(n) => Some(n.to_string()),
                AtomData::UnsignedInteger(n) => Some(n.to_string()),
                AtomData::UTF8(s) | AtomData::UTF16(s) => Some(s.clone()),
                _ => None,
            })
        }
        Field::Track | Field::Disc => {
            let (n, t) = if field == Field::Track {
                (ItemKey::TrackNumber, ItemKey::TrackTotal)
            } else {
                (ItemKey::DiscNumber, ItemKey::DiscTotal)
            };
            let number = plain(generic, n)?;
            match plain(generic, t) {
                Some(total) => Some(format!("{number}/{total}")),
                None => Some(number),
            }
        }
        other => keys_of(other).iter().find_map(|k| plain(generic, *k)),
    }
}

/// The PLAIN values of a key — those with no description — joined.
///
/// # Why the description matters
///
/// ID3v2 keeps many `COMM` frames and tells them apart by description:
/// iTunes writes its loudness and gapless data as `COMM:iTunNORM` and
/// `COMM:iTunSMPB`, and lofty's generic tag holds them all as `Comment`. The
/// Comment FIELD is the one without a description; the others are data
/// another program needs, and are neither shown as the comment nor removed
/// when it is written. The same holds for `USLT` in several languages.
///
/// # Why joined
///
/// A Vorbis comment, an MP4 atom and an ID3v2.4 frame may each hold several
/// values (two genres, three artists). The row shows all of them, `; `
/// between, rather than the first as if it were the whole; a person who
/// writes the row writes one value, which is what they typed.
fn plain(generic: &Generic, key: ItemKey) -> Option<String> {
    let values: Vec<&str> = generic
        .get_items(key)
        .filter(|i| i.description().is_empty())
        .filter_map(|i| i.value().text())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();
    (!values.is_empty()).then(|| values.join("; "))
}

/// Removes a key's plain items, and leaves its described ones.
fn remove_plain(g: &mut Generic, key: ItemKey) {
    g.retain(|i| !(i.key() == key && i.description().is_empty()));
}

/// What a field edit does to one tag's generic half.
///
/// `strict` is for the tag the person is writing TO: a field it has no
/// place for is [`Why::NoFieldIn`]. The others (ID3v1 beside ID3v2, APE
/// beside it) are kept in step where they can be and skipped where they
/// cannot.
fn set_generic(
    g: &mut Generic,
    field: Field,
    value: Option<&str>,
    strict: bool,
) -> Result<(), Why> {
    let tag_type = g.tag_type();
    let no_place =
        || format_of(tag_type).map_or(Why::Unwritable, |format| Why::NoFieldIn { field, format });
    match field {
        Field::Season | Field::Episode => Ok(()),
        // ON THE CONCRETE TAG, not here: `described_in_id3`.
        Field::Description if tag_type == TagType::Id3v2 => Ok(()),
        Field::Track | Field::Disc => {
            let (n, t) = if field == Field::Track {
                (ItemKey::TrackNumber, ItemKey::TrackTotal)
            } else {
                (ItemKey::DiscNumber, ItemKey::DiscTotal)
            };
            if n.map_key(tag_type).is_none() {
                return if strict { Err(no_place()) } else { Ok(()) };
            }
            g.remove_key(n);
            g.remove_key(t);
            if let Some(value) = value {
                let parsed = crate::sheet::numbered(value)?;
                g.insert_text(n, parsed.number.to_string());
                if let Some(total) = parsed.total {
                    g.insert_text(t, total.to_string());
                }
            }
            Ok(())
        }
        other => {
            let keys = keys_of(other);
            let Some(write_as) = keys.iter().find(|k| k.map_key(tag_type).is_some()) else {
                return if strict { Err(no_place()) } else { Ok(()) };
            };
            for k in replaced_keys(other) {
                remove_plain(g, *k);
            }
            // `push` and not `insert`: `insert` drops EVERY item of the key,
            // the described ones included (see [`plain`]).
            if let Some(value) = value
                && !g.push(TagItem::new(*write_as, ItemValue::Text(value.to_owned())))
                && strict
            {
                return Err(no_place());
            }
            Ok(())
        }
    }
}

/// Season and episode, which live beside the generic half in MP4.
fn set_mp4_number(ilst: &mut Ilst, field: Field, value: Option<&str>) -> Result<(), Why> {
    let ident = if field == Field::Season {
        SEASON
    } else {
        EPISODE
    };
    let _ = ilst.remove(&ident).count();
    if let Some(value) = value {
        let n = crate::sheet::count(value)?;
        let n = i32::try_from(n).map_err(|_| Why::BadNumber)?;
        ilst.replace_atom(Atom::new(ident, AtomData::SignedInteger(n)));
    }
    Ok(())
}

/// The index of the front cover among a generic tag's pictures.
///
/// MP4's `covr` has no picture type (lofty reads every one as `Other`), so
/// there the front is the first.
fn front_of(g: &Generic) -> Option<usize> {
    g.pictures()
        .iter()
        .position(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| (g.tag_type() == TagType::Mp4Ilst && !g.pictures().is_empty()).then_some(0))
}

/// A cover change, ready to go in.
#[derive(Debug, Clone)]
pub(crate) enum CoverChange {
    Replace { bytes: Vec<u8>, kind: ImageKind },
    Remove,
}

fn set_cover(g: &mut Generic, change: &CoverChange) {
    match change {
        CoverChange::Replace { bytes, kind } => {
            let mime = match kind {
                ImageKind::Png => MimeType::Png,
                _ => MimeType::Jpeg,
            };
            let picture = Picture::unchecked(bytes.clone())
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build();
            match front_of(g) {
                Some(i) => g.set_picture(i, picture),
                None => g.push_picture(picture),
            }
        }
        CoverChange::Remove => {
            while let Some(i) = front_of(g) {
                g.remove_picture(i);
            }
        }
    }
}

/// Whether a native name can be written in a format, and as what.
enum NativeName {
    /// Written on the concrete tag, by this very name (Vorbis, APE, RIFF).
    Raw,
    /// Written through the generic half, as this key.
    Key(ItemKey),
    /// An ID3v2 user text frame, `TXXX:<description>`.
    UserText(String),
    /// An ID3v2 comment or unsynchronised text with a description
    /// (`COMM:iTunNORM`), told apart from the plain one by it.
    Described {
        /// `COMM` or `USLT`.
        frame: Described,
        /// Its description.
        description: String,
    },
    /// An MP4 freeform atom, `----:<mean>:<name>`.
    Freeform {
        /// Its reverse-DNS mean.
        mean: String,
        /// Its name.
        name: String,
    },
}

/// The two ID3v2 frames told apart by a description and a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Described {
    Comment,
    Lyrics,
}

/// The keys a FIELD writes whole: the track and the disc are a number and
/// a total in one frame or atom (`TRCK`, `trkn`), which a value typed as
/// text would not fit — they change through their field.
const FIELD_ONLY_KEYS: [ItemKey; 4] = [
    ItemKey::TrackNumber,
    ItemKey::TrackTotal,
    ItemKey::DiscNumber,
    ItemKey::DiscTotal,
];

/// How a native name is written in a format — or why it is not.
fn native_name(format: TagFormat, name: &str) -> Result<NativeName, Why> {
    let own = crate::sheet::tag_name(&crate::sheet::Origin::Lofty { format }, name)?;
    // A NAME IS WRITTEN AS ITSELF: the writer hands this function names
    // `tag_name` already made the format's own, so one that would come out
    // different is a name a READER listed that the grammar would write as
    // another tag (an atom `XID ` as a freeform) — not this row's to change.
    if own != name {
        return Err(Why::BadName);
    }
    let name = own;
    let key = |tag_type: TagType, key: &str| match ItemKey::from_key(tag_type, key) {
        Some(k) if FIELD_ONLY_KEYS.contains(&k) => Err(Why::ReadOnlyView),
        Some(k) => Ok(NativeName::Key(k)),
        // A NAME THIS TAG'S KIND HAS NO ITEM FOR — a frame lofty does not
        // know — is not one it takes as text.
        None => Err(Why::BadName),
    };
    match format {
        TagFormat::Vorbis | TagFormat::Ape | TagFormat::RiffInfo => Ok(NativeName::Raw),
        TagFormat::AiffText => key(TagType::AiffText, &name),
        TagFormat::Id3v2 => {
            if let Some((frame, description)) = name.split_once(':') {
                return match frame {
                    "TXXX" => Ok(NativeName::UserText(description.to_owned())),
                    "COMM" => Ok(NativeName::Described {
                        frame: Described::Comment,
                        description: description.to_owned(),
                    }),
                    "USLT" => Ok(NativeName::Described {
                        frame: Described::Lyrics,
                        description: description.to_owned(),
                    }),
                    // WXXX: an address with a description, which no reader
                    // of ours lists as text to change.
                    _ => Err(Why::NotText),
                };
            }
            key(TagType::Id3v2, &name)
        }
        TagFormat::Mp4 => {
            if let Some(rest) = name.strip_prefix("----:")
                && let Some((mean, n)) = rest.split_once(':')
            {
                return Ok(NativeName::Freeform {
                    mean: mean.to_owned(),
                    name: n.to_owned(),
                });
            }
            // A season's and an episode's atoms are numbers their fields
            // write (`set_mp4_number`).
            if name == "tvsn" || name == "tves" {
                return Err(Why::ReadOnlyView);
            }
            key(TagType::Mp4Ilst, &name)
        }
        TagFormat::Id3v1 => Err(Why::TagReadOnly { format }),
    }
}

/// Whether a native tag of this name is one the Every-tag page may edit —
/// and why not, in a sentence of its own.
fn native_can(format: TagFormat, name: &str) -> Can {
    match native_name(format, name) {
        Ok(_) => Can::Yes(Writer::Lofty),
        Err(why) => Can::No(why),
    }
}

/// A native change, by name.
#[derive(Debug, Clone)]
pub(crate) struct NativeEdit {
    pub format: TagFormat,
    pub name: String,
    /// `None` removes it.
    pub value: Option<String>,
}

/// Refuses a native edit before anything is written.
pub(crate) fn check_native(edit: &NativeEdit) -> Result<(), Why> {
    native_name(edit.format, &edit.name).map(|_| ())
}

/// Everything a write does to a file's lofty tags.
#[derive(Debug, Clone, Default)]
pub(crate) struct Plan {
    /// Field → value (`None` clears).
    pub fields: Vec<(Field, Option<String>)>,
    /// The front cover.
    pub cover: Option<CoverChange>,
    /// Tags by native name.
    pub native: Vec<NativeEdit>,
}

/// The tags a plan makes of a file's: the ones it changes, as they will be
/// saved, and the ones it leaves alone. Pure — nothing is written.
pub(crate) struct Planned {
    /// The file's main tag format.
    primary: TagFormat,
    /// The tags as they will be saved.
    changed: Vec<Native>,
    /// The tags nobody touches, as they were read.
    untouched: Vec<Native>,
}

impl Planned {
    /// Whether the plan leaves a FLAC's Vorbis comment with NOTHING in it —
    /// no field, no named item, no picture: what lofty 0.25.4 cannot write
    /// (the draft reads back with the old comment, and is refused). The
    /// writer goes another way for it (`write::flac_by_blocks`).
    pub(crate) fn empties_the_comment(&self, file_type: FileType) -> bool {
        file_type == FileType::Flac
            && self.changed.iter().any(|t| {
                // THE VENDOR STRING is no item: lofty's generic tag carries it,
                // and no row lists it.
                t.format() == TagFormat::Vorbis
                    && listing(t).is_empty()
                    && t.generic().pictures().is_empty()
            })
    }
}

/// What `plan` makes of `opened`'s tags ([`Planned`]).
///
/// # Errors
///
/// A field the main tag has no place for, a name it refuses.
pub(crate) fn planned(opened: Opened, plan: &Plan) -> Result<Planned, Why> {
    let file_type = opened.file_type;
    let primary = file_type.primary_tag_type();
    let primary_format = format_of(primary).ok_or(Why::Unwritable)?;
    let mut tags = opened.tags;
    if !tags.iter().any(|t| t.tag_type() == primary) {
        tags.insert(0, Native::empty(primary_format));
    }
    let writable = |t: &Native| file_type.tag_support(t.tag_type()).is_writable();

    let mut changed: Vec<Native> = Vec::new();
    let mut untouched: Vec<Native> = Vec::new();
    for tag in tags {
        if !writable(&tag) {
            untouched.push(tag);
            continue;
        }
        let is_primary = tag.tag_type() == primary;
        let format = tag.format();
        let natives: Vec<&NativeEdit> = plan.native.iter().filter(|n| n.format == format).collect();
        // A SECOND TAG IS REWRITTEN ONLY FOR WHAT IT HOLDS: an ID3v1 beside
        // the ID3v2 follows a title, and is left alone for a composer it has
        // no place for.
        let touches_fields = plan
            .fields
            .iter()
            .any(|(f, _)| is_primary || has_place(format, *f));
        let touches_cover = plan.cover.as_ref().is_some_and(|c| {
            is_primary || (matches!(c, CoverChange::Remove) && tag_holds_pictures(format))
        });
        if !touches_fields && !touches_cover && natives.is_empty() {
            untouched.push(tag);
            continue;
        }
        let tag = described_in_id3(edit_raw(tag, &natives)?, &plan.fields);
        let mut failed: Option<Why> = None;
        let mut tag = tag.through(|g| {
            for (field, value) in &plan.fields {
                if let Err(why) = set_generic(g, *field, value.as_deref(), is_primary) {
                    failed.get_or_insert(why);
                }
            }
            for n in &natives {
                if let Ok(NativeName::Key(key)) = native_name(n.format, &n.name) {
                    remove_plain(g, key);
                    if let Some(v) = &n.value {
                        g.push(TagItem::new(key, ItemValue::Text(v.clone())));
                    }
                }
            }
            if touches_cover && let Some(change) = &plan.cover {
                set_cover(g, change);
            }
        });
        if let Some(why) = failed {
            return Err(why);
        }
        for (field, value) in &plan.fields {
            if !matches!(field, Field::Season | Field::Episode) {
                continue;
            }
            match &mut tag {
                Native::Mp4(ilst) => set_mp4_number(ilst, *field, value.as_deref())?,
                _ if is_primary => {
                    return Err(Why::NoFieldIn {
                        field: *field,
                        format,
                    });
                }
                _ => {}
            }
        }
        changed.push(tag);
    }
    Ok(Planned {
        primary: primary_format,
        changed,
        untouched,
    })
}

/// The planned tags saved into `file`, one after the other.
///
/// # Errors
///
/// [`Why::Unwritable`] when lofty refuses one.
pub(crate) fn save_planned(file: &mut File, planned: &Planned) -> Result<(), Why> {
    for tag in &planned.changed {
        tag.save(file)?;
    }
    Ok(())
}

/// A file ANOTHER WRITER (ffmpeg's remux) wrote a plan into, read back as a
/// lofty write is ([`verify_planned`]) — and more: every item of every tag
/// it rewrote is what was planned, name for name and value for value, in any
/// order. A remux keeps what its muxer knows, and an atom or a comment it
/// dropped would be lost without a word.
///
/// # Errors
///
/// [`Why::Unwritable`] when it does not read back as planned.
pub(crate) fn verify_remuxed(
    file: &mut File,
    file_type: FileType,
    plan: &Plan,
    planned: &Planned,
    tail: Option<u64>,
) -> Result<(), Why> {
    verify_planned(file, file_type, plan, planned, tail)?;
    let reread = open(file, file_type).map_err(|_| Why::Unwritable)?;
    let sorted = |t: &Native| {
        let mut rows: Vec<(String, String)> = listing(t)
            .into_iter()
            .map(|r| (r.name.to_ascii_uppercase(), format!("{:?}", r.value)))
            .collect();
        rows.sort();
        rows
    };
    for want in &planned.changed {
        let got = reread
            .tags
            .iter()
            .find(|t| t.format() == want.format())
            .map_or_else(Vec::new, &sorted);
        if got != sorted(want) {
            tracing::warn!(
                format = ?want.format(),
                "the remux did not keep every item of the tag as planned"
            );
            return Err(Why::Unwritable);
        }
    }
    Ok(())
}

/// A file a plan was written into — by lofty, or another writer — read back
/// against what was planned ([`verify`]), its FLAC frames against `tail`, the
/// bytes after the original's metadata.
///
/// # Errors
///
/// [`Why::Unwritable`] when it does not read back as planned.
pub(crate) fn verify_planned(
    file: &mut File,
    file_type: FileType,
    plan: &Plan,
    planned: &Planned,
    tail: Option<u64>,
) -> Result<(), Why> {
    let expected = Expected {
        primary: planned.primary,
        changed: &planned.changed,
        untouched: &planned.untouched,
        tail,
    };
    verify(file, file_type, plan, &expected)
}

/// The bytes after a FLAC's metadata — the frames — or `None` for another
/// file: what a write that only touches tags must leave as they were.
///
/// # Errors
///
/// [`Why::Unreadable`] for a FLAC whose blocks cannot be walked.
pub(crate) fn frames_of(file: &mut File, file_type: FileType) -> Result<Option<u64>, Why> {
    flac_tail(file, file_type)
}

/// What a draft must read back as.
struct Expected<'a> {
    primary: TagFormat,
    /// The tags as they were saved.
    changed: &'a [Native],
    /// The tags nobody touched, as they were read.
    untouched: &'a [Native],
    /// A FLAC's bytes after its metadata.
    tail: Option<u64>,
}

/// Reads the draft back and checks that it is what was written.
///
/// # Why a write is read back
///
/// Because lofty has been seen to break a file. Its `replace_range` has a
/// fast path for a tag at the very end of the file, and when the new tag is
/// SMALLER it truncates the file to the difference instead of by it — a
/// FLAC whose metadata ends the file lost everything past its first
/// 24 bytes (lofty 0.25.4, `util/io.rs`). The truncated file still OPENS,
/// so reading is not enough: the checks are that every field and every
/// named tag reads back as written, that every picture is still there, that
/// the tags nobody touched are unchanged, and that a FLAC still has every
/// byte it had after its metadata. A draft that fails any of them is
/// refused as [`Why::Unwritable`], and the file is left as it was.
fn verify(
    file: &mut File,
    file_type: FileType,
    plan: &Plan,
    expected: &Expected<'_>,
) -> Result<(), Why> {
    let broken = |what: &str| {
        tracing::warn!(what, "the draft lofty wrote does not read back as written");
        Why::Unwritable
    };
    if flac_tail(file, file_type).map_err(|_| broken("the FLAC blocks"))? != expected.tail {
        return Err(broken("the FLAC frames"));
    }
    let reread = open(file, file_type).map_err(|_| broken("the tags"))?;
    let find = |format: TagFormat| reread.tags.iter().find(|t| t.format() == format);

    let empty = |t: &Native| t.generic().is_empty() && listing(t).is_empty();
    for want in expected.untouched {
        match find(want.format()) {
            Some(got)
                if listing(got) == listing(want) && picture_data(got) == picture_data(want) => {}
            // The main tag this write added and then had nothing to put in.
            None if empty(want) => {}
            _ => return Err(broken("a tag nobody touched")),
        }
    }
    for want in expected.changed {
        match find(want.format()) {
            Some(got) => {
                if picture_data(got) != picture_data(want) {
                    return Err(broken("the pictures"));
                }
                // Where a name is only a string, every item must come back
                // as it went — the keys lofty has no name for included.
                let raw = matches!(want.format(), TagFormat::Vorbis | TagFormat::Ape);
                if raw && listing(got) != listing(want) {
                    return Err(broken("the items of a tag"));
                }
            }
            // A tag emptied by the write is written as no tag at all.
            None if empty(want) => {}
            None => return Err(broken("a tag that was written")),
        }
    }
    if let Some(tag) = find(expected.primary) {
        let generic = tag.generic();
        for (field, value) in &plan.fields {
            if has_place(expected.primary, *field)
                && field_value(tag, &generic, *field).as_deref() != value.as_deref()
            {
                return Err(broken("a field"));
            }
        }
    } else if plan.fields.iter().any(|(_, v)| v.is_some()) {
        return Err(broken("the main tag"));
    }
    for n in &plan.native {
        let got = find(n.format).map(|tag| match native_name(n.format, &n.name) {
            Ok(NativeName::Key(key)) => plain(&tag.generic(), key),
            _ => {
                // THE NAME AS WRITTEN FIRST, and only then another spelling
                // of it: an ID3v2 user text frame's description is kept as it
                // is spelled, and a `TXXX:description` beside a
                // `TXXX:DESCRIPTION` is two frames.
                let rows = listing(tag);
                let value_of = |exact: bool| {
                    rows.iter().find_map(|t| {
                        let named = if exact {
                            t.name == n.name
                        } else {
                            t.name.eq_ignore_ascii_case(&n.name)
                        };
                        match &t.value {
                            TagValue::Text(v) if named => Some(v.clone()),
                            TagValue::Cut { head, .. } if named => Some(head.clone()),
                            _ => None,
                        }
                    })
                };
                value_of(true).or_else(|| value_of(false))
            }
        });
        let got = got.flatten();
        let fits = match (&n.value, &got) {
            (None, None) => true,
            (Some(v), Some(g)) => {
                v == g || (v.len() > OTHER_VALUE_BYTES && v.starts_with(g.as_str()))
            }
            _ => false,
        };
        if !fits {
            return Err(broken("a named tag"));
        }
    }
    Ok(())
}

/// A tag's pictures, as bytes, sorted — the same pictures in any order.
fn picture_data(tag: &Native) -> Vec<Vec<u8>> {
    let mut data: Vec<Vec<u8>> = tag
        .generic()
        .pictures()
        .iter()
        .map(|p| p.data().to_vec())
        .collect();
    data.sort();
    data
}

/// How many bytes a FLAC file has after its metadata — its frames — or
/// `None` for any other file.
///
/// The block chain is walked here, and a block that claims more bytes than
/// the file has is [`Why::Unreadable`]: that is exactly what lofty's
/// shrinking write leaves.
fn flac_tail(file: &mut File, file_type: FileType) -> Result<Option<u64>, Why> {
    use std::io::{Read as _, SeekFrom};
    if file_type != FileType::Flac {
        return Ok(None);
    }
    let len = file.metadata().map_err(|_| Why::Unreadable)?.len();
    let mut at: u64 = 0;
    let mut head = [0_u8; 10];
    file.seek(SeekFrom::Start(0)).map_err(|_| Why::Unreadable)?;
    file.read_exact(&mut head).map_err(|_| Why::Unreadable)?;
    // An ID3v2 tag in front: ten bytes of header and a syncsafe size.
    if &head[..3] == b"ID3" {
        let size = head[6..10]
            .iter()
            .fold(0_u64, |acc, b| (acc << 7) | u64::from(b & 0x7F));
        let footer = if head[5] & 0x10 != 0 { 10 } else { 0 };
        at = 10 + size + footer;
    }
    let mut magic = [0_u8; 4];
    file.seek(SeekFrom::Start(at))
        .map_err(|_| Why::Unreadable)?;
    file.read_exact(&mut magic).map_err(|_| Why::Unreadable)?;
    if &magic != b"fLaC" {
        return Err(Why::Unreadable);
    }
    at += 4;
    loop {
        let mut header = [0_u8; 4];
        file.seek(SeekFrom::Start(at))
            .map_err(|_| Why::Unreadable)?;
        file.read_exact(&mut header).map_err(|_| Why::Unreadable)?;
        let size = u64::from(u32::from_be_bytes([0, header[1], header[2], header[3]]));
        at += 4 + size;
        if at > len {
            return Err(Why::Unreadable);
        }
        if header[0] & 0x80 != 0 {
            return Ok(Some(len - at));
        }
    }
}

/// The atoms of an MP4's `ilst` that ffmpeg's mov muxer writes back as it
/// read them (`-map_metadata 0`): the fields of `write::MP4_REMUX_FIELDS`
/// and the cover. `©too` is written back by name (`encoding_tool`, see
/// [`mp4_encoding_tool`]); any other atom — a freeform `----`, `gnre`, a
/// store's ids — is one the remux would lose.
const MP4_REMUX_ATOMS: [[u8; 4]; 15] = [
    *b"\xa9nam",
    *b"\xa9ART",
    *b"\xa9alb",
    *b"aART",
    *b"\xa9wrt",
    *b"\xa9day",
    *b"\xa9gen",
    *b"\xa9cmt",
    *b"\xa9lyr",
    *b"desc",
    *b"tvsh",
    *b"trkn",
    *b"disk",
    *b"covr",
    *b"\xa9too",
];

/// Whether a remux of this MP4 would lose an item of its `ilst` — one
/// [`MP4_REMUX_ATOMS`] does not list. What lofty does not list (a `uuid`
/// box beside the `moov`) is not seen here, and is the read-back's to
/// catch.
pub(crate) fn mp4_remux_drops(opened: &Opened) -> bool {
    opened.tags.iter().any(|t| match t {
        Native::Mp4(ilst) => ilst.into_iter().any(|atom| match atom.ident() {
            AtomIdent::Fourcc(b) => !MP4_REMUX_ATOMS.contains(b),
            AtomIdent::Freeform { .. } => true,
        }),
        _ => false,
    })
}

/// An MP4's `©too` — the program that made it — as text: ffmpeg reads it
/// as `encoder`, and with `+bitexact` writes it back only when told it as
/// `encoding_tool`.
pub(crate) fn mp4_encoding_tool(opened: &Opened) -> Option<String> {
    opened.tags.iter().find_map(|t| match t {
        Native::Mp4(ilst) => {
            ilst.get(&AtomIdent::Fourcc(*b"\xa9too"))?
                .data()
                .find_map(|d| match d {
                    AtomData::UTF8(s) | AtomData::UTF16(s) => Some(s.clone()),
                    _ => None,
                })
        }
        _ => None,
    })
}

/// A FLAC's front, read whole: the bytes before its `fLaC` (an ID3v2 tag),
/// each metadata block as its kind and body, and where its frames begin.
pub(crate) struct FlacFront {
    /// What comes before `fLaC`, as it was.
    pub(crate) before: Vec<u8>,
    /// Every metadata block, in order: its kind (the header's low seven
    /// bits) and its body.
    pub(crate) blocks: Vec<(u8, Vec<u8>)>,
    /// The offset of the first frame.
    pub(crate) frames_at: u64,
}

/// A FLAC's metadata block that is its Vorbis comment.
pub(crate) const FLAC_VORBIS_COMMENT: u8 = 4;
/// A FLAC's metadata block that is a picture.
pub(crate) const FLAC_PICTURE: u8 = 6;
/// The longest body a FLAC metadata block's 24-bit length can say.
const FLAC_BLOCK_MOST: usize = (1 << 24) - 1;

/// A FLAC's [`FlacFront`].
///
/// # Errors
///
/// [`Why::Unreadable`] for what is not a FLAC, or a block chain that runs
/// past the file.
pub(crate) fn flac_front(file: &mut File) -> Result<FlacFront, Why> {
    use std::io::{Read as _, SeekFrom};
    let len = file.metadata().map_err(|_| Why::Unreadable)?.len();
    let mut head = [0_u8; 10];
    file.seek(SeekFrom::Start(0)).map_err(|_| Why::Unreadable)?;
    file.read_exact(&mut head).map_err(|_| Why::Unreadable)?;
    let mut at: u64 = 0;
    if &head[..3] == b"ID3" {
        let size = head[6..10]
            .iter()
            .fold(0_u64, |acc, b| (acc << 7) | u64::from(b & 0x7F));
        let footer = if head[5] & 0x10 != 0 { 10 } else { 0 };
        at = 10 + size + footer;
    }
    if at + 4 > len {
        return Err(Why::Unreadable);
    }
    let mut before = vec![0_u8; usize::try_from(at).map_err(|_| Why::Unreadable)?];
    file.seek(SeekFrom::Start(0)).map_err(|_| Why::Unreadable)?;
    file.read_exact(&mut before).map_err(|_| Why::Unreadable)?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic).map_err(|_| Why::Unreadable)?;
    if &magic != b"fLaC" {
        return Err(Why::Unreadable);
    }
    at += 4;
    let mut blocks = Vec::new();
    loop {
        let mut header = [0_u8; 4];
        file.seek(SeekFrom::Start(at))
            .map_err(|_| Why::Unreadable)?;
        file.read_exact(&mut header).map_err(|_| Why::Unreadable)?;
        let size = u32::from_be_bytes([0, header[1], header[2], header[3]]);
        at += 4 + u64::from(size);
        if at > len {
            return Err(Why::Unreadable);
        }
        let mut body = vec![0_u8; size as usize];
        file.read_exact(&mut body).map_err(|_| Why::Unreadable)?;
        blocks.push((header[0] & 0x7F, body));
        if header[0] & 0x80 != 0 {
            return Ok(FlacFront {
                before,
                blocks,
                frames_at: at,
            });
        }
    }
}

/// `front`'s metadata as a FLAC whose Vorbis comment has NOTHING in it:
/// the comment keeps its vendor string and holds no item, every picture
/// block goes (the plan has none), and every other block — stream info,
/// seek table, cue sheet, application, padding — is copied byte for byte,
/// in its place. What lofty 0.25.4 cannot write, and what a remux would
/// lose (ffmpeg's FLAC muxer keeps no cue sheet, seek table or application
/// block).
///
/// # Errors
///
/// [`Why::Unreadable`] for a Vorbis comment whose vendor string runs past
/// its block.
pub(crate) fn flac_emptied(front: &FlacFront) -> Result<Vec<u8>, Why> {
    let mut kept: Vec<(u8, Vec<u8>)> = Vec::with_capacity(front.blocks.len());
    for (kind, body) in &front.blocks {
        match *kind {
            FLAC_PICTURE => {}
            FLAC_VORBIS_COMMENT => {
                let vendor_len = body
                    .get(..4)
                    .and_then(|b| <[u8; 4]>::try_from(b).ok())
                    .map(u32::from_le_bytes)
                    .ok_or(Why::Unreadable)? as usize;
                let vendor = body.get(4..4 + vendor_len).ok_or(Why::Unreadable)?;
                let mut emptied = Vec::with_capacity(8 + vendor_len);
                emptied.extend_from_slice(&body[..4]);
                emptied.extend_from_slice(vendor);
                emptied.extend_from_slice(&0_u32.to_le_bytes());
                kept.push((*kind, emptied));
            }
            _ => kept.push((*kind, body.clone())),
        }
    }
    let mut out = front.before.clone();
    out.extend_from_slice(b"fLaC");
    let last = kept.len().saturating_sub(1);
    for (i, (kind, body)) in kept.iter().enumerate() {
        if body.len() > FLAC_BLOCK_MOST {
            return Err(Why::Unreadable);
        }
        let flag = if i == last { 0x80 } else { 0 };
        let size = u32::try_from(body.len()).map_err(|_| Why::Unreadable)?;
        let [_, a, b, c] = size.to_be_bytes();
        out.extend_from_slice(&[flag | kind, a, b, c]);
        out.extend_from_slice(body);
    }
    Ok(out)
}

/// The tags of `planned` other than the FLAC's Vorbis comment, saved into
/// `file` by lofty — an ID3v2 tag in front of a FLAC whose comment was
/// emptied by [`flac_emptied`].
///
/// # Errors
///
/// [`Why::Unwritable`] when lofty refuses one.
pub(crate) fn save_besides_the_comment(file: &mut File, planned: &Planned) -> Result<(), Why> {
    for tag in planned
        .changed
        .iter()
        .filter(|t| t.format() != TagFormat::Vorbis)
    {
        tag.save(file)?;
    }
    Ok(())
}

/// Whether a format can hold pictures at all.
const fn tag_holds_pictures(format: TagFormat) -> bool {
    matches!(
        format,
        TagFormat::Id3v2 | TagFormat::Ape | TagFormat::Vorbis | TagFormat::Mp4
    )
}

/// The native edits that are written on the concrete tag, by raw name —
/// every name the generic half has no key for: Vorbis, APE and RIFF names,
/// ID3v2's described frames, MP4's freeform atoms.
fn edit_raw(tag: Native, edits: &[&NativeEdit]) -> Result<Native, Why> {
    let raw: Vec<(&NativeEdit, NativeName)> = edits
        .iter()
        .filter_map(|e| match native_name(e.format, &e.name) {
            Ok(NativeName::Key(_)) | Err(_) => None,
            Ok(how) => Some((*e, how)),
        })
        .collect();
    if raw.is_empty() {
        return Ok(tag);
    }
    Ok(match tag {
        Native::Vorbis(mut vc) => {
            for (e, _) in raw {
                let _ = vc.remove(&e.name).count();
                if let Some(v) = &e.value {
                    vc.push(e.name.clone(), v.clone());
                }
            }
            Native::Vorbis(vc)
        }
        Native::Ape(mut ape) => {
            for (e, _) in raw {
                ape.remove(&e.name);
                if let Some(v) = &e.value {
                    let item = ApeItem::new(e.name.clone(), ItemValue::Text(v.clone()))
                        .map_err(|_| Why::BadName)?;
                    ape.insert(item);
                }
            }
            Native::Ape(ape)
        }
        Native::RiffInfo(mut riff) => {
            for (e, _) in raw {
                let _ = riff.remove(&e.name);
                if let Some(v) = &e.value {
                    riff.insert(e.name.clone(), v.clone());
                }
            }
            Native::RiffInfo(riff)
        }
        Native::Id3v2(mut id3) => {
            for (e, how) in raw {
                match how {
                    NativeName::UserText(description) => {
                        // `insert` replaces the frame of the same description.
                        while id3.remove_user_text(&description).is_some() {}
                        if let Some(v) = &e.value {
                            let _ = id3.insert_user_text(description, v.clone());
                        }
                    }
                    NativeName::Described { frame, description } => {
                        set_described(&mut id3, frame, &description, e.value.as_deref());
                    }
                    _ => {}
                }
            }
            Native::Id3v2(id3)
        }
        Native::Mp4(mut ilst) => {
            for (e, how) in raw {
                if let NativeName::Freeform { mean, name } = how {
                    let ident = AtomIdent::Freeform {
                        mean: mean.into(),
                        name: name.into(),
                    };
                    match &e.value {
                        Some(v) => ilst.replace_atom(Atom::new(ident, AtomData::UTF8(v.clone()))),
                        None => {
                            let _ = ilst.remove(&ident).count();
                        }
                    }
                }
            }
            Native::Mp4(ilst)
        }
        other => other,
    })
}

/// An ID3v2 comment or unsynchronised text of one description, set or
/// taken out — in the language the frame had (`XXX`, "unknown", for a new
/// one), every other description left as it was.
fn set_described(id3: &mut Id3v2Tag, frame: Described, description: &str, value: Option<&str>) {
    use lofty::TextEncoding;
    use lofty::id3::v2::{CommentFrame, UnsynchronizedTextFrame};
    let mut language: Option<[u8; 3]> = None;
    id3.retain(|f| {
        let (lang, desc) = match (frame, f) {
            (Described::Comment, Frame::Comment(c)) => (c.language, c.description.as_ref()),
            (Described::Lyrics, Frame::UnsynchronizedText(u)) => {
                (u.language, u.description.as_ref())
            }
            _ => return true,
        };
        if desc == description {
            language.get_or_insert(lang);
            false
        } else {
            true
        }
    });
    let Some(value) = value else {
        return;
    };
    let language = language.unwrap_or(*b"XXX");
    let _ = id3.insert(match frame {
        Described::Comment => Frame::Comment(CommentFrame::new(
            TextEncoding::UTF8,
            language,
            description.to_owned(),
            value.to_owned(),
        )),
        Described::Lyrics => Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
            TextEncoding::UTF8,
            language,
            description.to_owned(),
            value.to_owned(),
        )),
    });
}

/// An ID3v2 frame's name on its row: its id, and its description when it
/// has one.
fn described(id: &str, description: &str) -> String {
    if description.is_empty() {
        id.to_owned()
    } else {
        format!("{id}:{description}")
    }
}

/// Every native tag, by its own name, for the Every-tag page.
pub(crate) fn listing(native: &Native) -> Vec<Tag> {
    let format = native.format();
    let origin = crate::sheet::Origin::Lofty { format };
    let row = |name: String, value: TagValue| {
        let can = match value {
            TagValue::Binary { .. } => Can::No(Why::NotText),
            _ => native_can(format, &name),
        };
        Tag {
            origin: origin.clone(),
            name,
            value,
            can,
        }
    };
    let mut out = Vec::new();
    match native {
        Native::Vorbis(vc) => {
            for (k, v) in vc.items() {
                out.push(row(k.to_owned(), text_value(v)));
            }
        }
        Native::Ape(ape) => {
            for item in ape {
                let value = match item.value() {
                    ItemValue::Text(s) | ItemValue::Locator(s) => text_value(s),
                    ItemValue::Binary(b) => TagValue::Binary {
                        bytes: b.len() as u64,
                    },
                };
                out.push(row(item.key().to_owned(), value));
            }
        }
        Native::RiffInfo(riff) => {
            for (k, v) in riff {
                out.push(row(k.clone(), text_value(v)));
            }
        }
        Native::Id3v2(id3) => {
            for frame in id3 {
                let id = frame.id_str();
                let entry = match frame {
                    Frame::Text(f) => Some((id.to_owned(), f.value.replace('\0', "; "))),
                    Frame::UserText(f) => {
                        Some((format!("TXXX:{}", f.description), f.content.to_string()))
                    }
                    // A DESCRIBED comment or text is its own row, named by
                    // its description (`COMM:iTunNORM`): the plain one is the
                    // field's, and the two are written apart.
                    Frame::Comment(f) => {
                        Some((described(id, &f.description), f.content.to_string()))
                    }
                    Frame::UnsynchronizedText(f) => {
                        Some((described(id, &f.description), f.content.to_string()))
                    }
                    Frame::Url(f) => Some((id.to_owned(), f.url().to_owned())),
                    Frame::UserUrl(f) => {
                        Some((format!("WXXX:{}", f.description), f.content.to_string()))
                    }
                    Frame::Timestamp(f) => Some((id.to_owned(), f.timestamp.to_string())),
                    // Pictures are covers; the rest (POPM, PRIV, GEOB, SYLT…)
                    // are exiftool's rows, which name them better.
                    _ => None,
                };
                if let Some((name, value)) = entry {
                    out.push(row(name, text_value(&value)));
                }
            }
        }
        Native::Mp4(ilst) => {
            for atom in ilst {
                let name = match atom.ident() {
                    AtomIdent::Fourcc(b) => b.iter().map(|&c| char::from(c)).collect(),
                    AtomIdent::Freeform { mean, name } => format!("----:{mean}:{name}"),
                };
                let value = match atom.data().next() {
                    Some(AtomData::UTF8(s) | AtomData::UTF16(s)) => text_value(s),
                    Some(AtomData::SignedInteger(n)) => TagValue::Text(n.to_string()),
                    Some(AtomData::UnsignedInteger(n)) => TagValue::Text(n.to_string()),
                    Some(AtomData::Bool(b)) => TagValue::Text(u8::from(*b).to_string()),
                    Some(AtomData::Picture(_)) | None => continue,
                    Some(AtomData::Unknown { data, .. }) => match name.as_str() {
                        // `trkn` and `disk`: eight bytes, the number and the
                        // total big-endian at 2 and 4.
                        "trkn" | "disk" if data.len() >= 6 => {
                            let n = u16::from_be_bytes([data[2], data[3]]);
                            let t = u16::from_be_bytes([data[4], data[5]]);
                            TagValue::Text(if t > 0 {
                                format!("{n}/{t}")
                            } else {
                                n.to_string()
                            })
                        }
                        _ => TagValue::Binary {
                            bytes: data.len() as u64,
                        },
                    },
                };
                out.push(row(name, value));
            }
        }
        Native::AiffText(_) => {
            let generic = native.generic();
            for item in generic.items() {
                if let (Some(name), Some(text)) =
                    (item.key().map_key(TagType::AiffText), item.value().text())
                {
                    out.push(row(name.to_owned(), text_value(text)));
                }
            }
        }
        // ID3v1 has positions and no names; its fields are kept in step
        // with the main tag, and exiftool's rows show it.
        Native::Id3v1(_) => {}
    }
    out
}

/// A value as the Every-tag page carries it: whole, or its beginning.
pub(crate) fn text_value(text: &str) -> TagValue {
    TagValue::of(text)
}

/// The pictures of a tag, as covers.
pub(crate) fn covers(native: &Native, can: &Can) -> Vec<Cover> {
    let generic = native.generic();
    let front = front_of(&generic);
    generic
        .pictures()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let data = p.data();
            let kind = picture::sniff(data)
                .or_else(|| p.mime_type().and_then(|m| ImageKind::of_mime(m.as_str())));
            let mime = kind.map_or_else(
                || {
                    p.mime_type()
                        .map_or_else(String::new, |m| m.as_str().to_owned())
                },
                |k| k.mime().to_owned(),
            );
            let (width, height) =
                picture::dimensions(data).map_or((None, None), |(w, h)| (Some(w), Some(h)));
            Cover {
                role: if Some(i) == front {
                    CoverRole::Front
                } else {
                    role_of(p.pic_type())
                },
                mime,
                width,
                height,
                bytes: Some(data.len() as u64),
                at: CoverAt::Tag {
                    format: native.format(),
                    index: u32::try_from(i).unwrap_or(u32::MAX),
                },
                can: can.clone(),
            }
        })
        .collect()
}

/// A picture's bytes and kind, by where the sheet said it was.
pub(crate) fn picture_bytes(
    opened: &Opened,
    format: TagFormat,
    index: u32,
) -> Result<(Vec<u8>, Option<ImageKind>), Why> {
    let tag = opened
        .tags
        .iter()
        .find(|t| t.format() == format)
        .ok_or(Why::NoSuchThing)?;
    let generic = tag.generic();
    let p = generic
        .pictures()
        .get(index as usize)
        .ok_or(Why::NoSuchThing)?;
    let kind = picture::sniff(p.data())
        .or_else(|| p.mime_type().and_then(|m| ImageKind::of_mime(m.as_str())));
    Ok((p.data().to_vec(), kind))
}

const fn role_of(t: PictureType) -> CoverRole {
    match t {
        PictureType::CoverFront => CoverRole::Front,
        PictureType::CoverBack => CoverRole::Back,
        PictureType::Leaflet => CoverRole::Leaflet,
        PictureType::Media => CoverRole::Media,
        PictureType::LeadArtist
        | PictureType::Artist
        | PictureType::Band
        | PictureType::Conductor => CoverRole::Artist,
        PictureType::Icon | PictureType::OtherIcon => CoverRole::Icon,
        _ => CoverRole::Other,
    }
}
