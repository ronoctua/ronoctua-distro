//! A TAG OF A SONG OR A FILM, IN WORDS — a field's label, and why a file's
//! tags cannot be read or written.
//!
//! # Why a module of its own
//!
//! The player says them, and so do the file manager's facts and the panel
//! that edits a tag: three doors, one sentence each. They were the player's,
//! so the file manager's panels named the player to say a tag.

use crate::key;
use ronoctua_tags::Field;

/// Why a file's tags cannot be read or written, in a sentence — a refusal's
/// second half, and the sentence under a row of the Metadata section that
/// cannot be changed (`ronoctua_shell_player::player_metadata`). One for each reason.
#[must_use]
pub fn tags_refusal(why: &ronoctua_tags::Why, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    use ronoctua_tags::Why;
    let size = |bytes: u64| crate::sizes::size_text(bytes, texts.decimal_mark());
    match why {
        Why::NotAFile => texts.t(key::PLAYER_TROUBLE_TAG_NOT_A_FILE),
        Why::Gone => texts.t(key::PLAYER_TROUBLE_TAG_GONE),
        Why::ChangedSince => texts.t(key::PLAYER_TROUBLE_TAG_CHANGED_SINCE),
        Why::HardLinked => texts.t(key::PLAYER_TROUBLE_TAG_HARD_LINKED),
        Why::ReadOnlyFolder => texts.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_FOLDER),
        Why::ReadOnlyFile => texts.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_FILE),
        Why::NotYours => texts.t(key::PLAYER_TROUBLE_TAG_NOT_YOURS),
        Why::NoRoom {
            need_bytes,
            free_bytes,
        } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_ROOM,
            &[("need", &size(*need_bytes)), ("free", &size(*free_bytes))],
        ),
        Why::Unreadable => texts.t(key::PLAYER_TROUBLE_TAG_UNREADABLE),
        Why::Unwritable => texts.t(key::PLAYER_TROUBLE_TAG_UNWRITABLE),
        Why::NoWriter { container } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_WRITER,
            &[("container", &texts.t(container_key(*container)))],
        ),
        Why::NoFieldIn { field, format } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_FIELD_IN,
            &[
                ("format", &texts.t(tag_format_key(*format))),
                ("field", &texts.t(crate::tag_words::field_key(*field))),
            ],
        ),
        Why::NoPicturesIn { format } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_NO_PICTURES_IN,
            &[("format", &texts.t(tag_format_key(*format)))],
        ),
        Why::StreamsOnlyInMatroska { container } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_STREAMS_ONLY_MATROSKA,
            &[("container", &texts.t(container_key(*container)))],
        ),
        Why::ChaptersOnlyInMatroska { container } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_CHAPTERS_ONLY_MATROSKA,
            &[("container", &texts.t(container_key(*container)))],
        ),
        Why::AttachmentIsNotAStream => texts.t(key::PLAYER_TROUBLE_TAG_ATTACHMENT),
        Why::RemuxWouldDrop => texts.t(key::PLAYER_TROUBLE_TAG_REMUX_WOULD_DROP),
        Why::TagReadOnly { format } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_READ_ONLY,
            &[("format", &texts.t(tag_format_key(*format)))],
        ),
        Why::OnlyTheFrontCover => texts.t(key::PLAYER_TROUBLE_TAG_ONLY_THE_FRONT_COVER),
        Why::Missing { program } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_MISSING,
            &[("program", program.name())],
        ),
        Why::ProgramFailed { program } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_PROGRAM_FAILED,
            &[("program", program.name())],
        ),
        Why::TimedOut { program } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_TIMED_OUT,
            &[("program", program.name())],
        ),
        Why::TooBig { bytes, most_bytes } => texts.t_with(
            key::PLAYER_TROUBLE_TAG_TOO_BIG,
            &[("bytes", &size(*bytes)), ("most", &size(*most_bytes))],
        ),
        Why::NotAnImage => texts.t(key::PLAYER_TROUBLE_TAG_NOT_AN_IMAGE),
        Why::BadNumber => texts.t(key::PLAYER_TROUBLE_TAG_BAD_NUMBER),
        Why::BadDate => texts.t(key::PLAYER_TROUBLE_TAG_BAD_DATE),
        Why::BadIsrc => texts.t(key::PLAYER_TROUBLE_TAG_BAD_ISRC),
        Why::BadLanguage => texts.t(key::PLAYER_TROUBLE_TAG_BAD_LANGUAGE),
        Why::ReadOnlyView => texts.t(key::PLAYER_TROUBLE_TAG_READ_ONLY_VIEW),
        Why::BadName => texts.t(key::PLAYER_TROUBLE_TAG_BAD_NAME),
        Why::NotText => texts.t(key::PLAYER_TROUBLE_TAG_NOT_TEXT),
        Why::NotGlobal => texts.t(key::PLAYER_TROUBLE_TAG_NOT_GLOBAL),
        Why::NoSuchThing => texts.t(key::PLAYER_TROUBLE_TAG_NO_SUCH_THING),
        Why::Taken => texts.t(key::PLAYER_TROUBLE_TAG_TAKEN),
        Why::Cancelled => texts.t(key::PLAYER_TROUBLE_TAG_CANCELLED),
    }
}

/// A kind of tag, as a sentence names it — its key.
#[must_use]
pub const fn tag_format_key(format: ronoctua_tags::TagFormat) -> &'static str {
    use crate::key;
    use ronoctua_tags::TagFormat as F;
    match format {
        F::Id3v2 => key::PLAYER_TAG_FORMAT_ID3V2,
        F::Id3v1 => key::PLAYER_TAG_FORMAT_ID3V1,
        F::Ape => key::PLAYER_TAG_FORMAT_APE,
        F::Vorbis => key::PLAYER_TAG_FORMAT_VORBIS,
        F::Mp4 => key::PLAYER_TAG_FORMAT_MP4,
        F::RiffInfo => key::PLAYER_TAG_FORMAT_RIFF_INFO,
        F::AiffText => key::PLAYER_TAG_FORMAT_AIFF_TEXT,
    }
}

/// A container, by the name a person knows it by — its key.
#[must_use]
pub const fn container_key(container: ronoctua_tags::Container) -> &'static str {
    use crate::key;
    use ronoctua_tags::Container as C;
    match container {
        C::Mpeg => key::PLAYER_CONTAINER_MPEG,
        C::Aac => key::PLAYER_CONTAINER_AAC,
        C::Flac => key::PLAYER_CONTAINER_FLAC,
        C::OggVorbis => key::PLAYER_CONTAINER_OGG_VORBIS,
        C::Opus => key::PLAYER_CONTAINER_OPUS,
        C::Speex => key::PLAYER_CONTAINER_SPEEX,
        C::OggOther => key::PLAYER_CONTAINER_OGG_OTHER,
        C::Mp4 => key::PLAYER_CONTAINER_MP4,
        C::Wav => key::PLAYER_CONTAINER_WAV,
        C::Aiff => key::PLAYER_CONTAINER_AIFF,
        C::Ape => key::PLAYER_CONTAINER_APE,
        C::Mpc => key::PLAYER_CONTAINER_MPC,
        C::WavPack => key::PLAYER_CONTAINER_WAVPACK,
        C::Matroska => key::PLAYER_CONTAINER_MATROSKA,
        C::WebM => key::PLAYER_CONTAINER_WEBM,
        C::Avi => key::PLAYER_CONTAINER_AVI,
        C::MpegTs => key::PLAYER_CONTAINER_MPEG_TS,
        C::Flv => key::PLAYER_CONTAINER_FLV,
        C::Asf => key::PLAYER_CONTAINER_ASF,
        C::Other => key::PLAYER_CONTAINER_OTHER,
    }
}

/// A field's label — its own key.
#[must_use]
pub const fn field_key(field: Field) -> &'static str {
    match field {
        Field::Title => key::PLAYER_TAG_TITLE,
        Field::Artist => key::PLAYER_TAG_ARTIST,
        Field::Album => key::PLAYER_TAG_ALBUM,
        Field::AlbumArtist => key::PLAYER_TAG_ALBUM_ARTIST,
        Field::Track => key::PLAYER_TAG_TRACK,
        Field::Disc => key::PLAYER_TAG_DISC,
        Field::Date => key::PLAYER_TAG_DATE,
        Field::Genre => key::PLAYER_TAG_GENRE,
        Field::Composer => key::PLAYER_TAG_COMPOSER,
        Field::Lyricist => key::PLAYER_TAG_LYRICIST,
        Field::Comment => key::PLAYER_TAG_COMMENT,
        Field::Lyrics => key::PLAYER_TAG_LYRICS,
        Field::Publisher => key::PLAYER_TAG_PUBLISHER,
        Field::Isrc => key::PLAYER_TAG_ISRC,
        Field::Description => key::PLAYER_TAG_DESCRIPTION,
        Field::Show => key::PLAYER_TAG_SHOW,
        Field::Season => key::PLAYER_TAG_SEASON,
        Field::Episode => key::PLAYER_TAG_EPISODE,
        Field::Director => key::PLAYER_TAG_DIRECTOR,
    }
}
