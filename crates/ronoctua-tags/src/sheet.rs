//! What a file says about itself, as the player's Metadata section shows it.
//!
//! # Schema only, and on purpose
//!
//! This module has no lofty, no program and no filesystem call beyond a
//! `Metadata` somebody else already took. It is what crosses the player's
//! socket (`Say::Sheet`) and what a door draws, so the shell can depend on
//! this crate with `default-features = false` and learn the SHAPE of a sheet
//! without linking the machinery that reads one.
//!
//! # Nothing here is a sentence
//!
//! Every name that would be text on screen is a variant — [`Field`],
//! [`TechKey`], [`Why`], [`CoverRole`] — and the door turns each into a text
//! key (invariant 8). What IS text here is data the FILE wrote: a title, a
//! codec's name, a tag's name as the container spells it. Those are shown as
//! they were written, the rule `ronoctua_inspect::facts::Label::Named` keeps.
//!
//! # Units in the names
//!
//! A quantity that crosses a boundary carries its unit (`AGENTS.md`):
//! `mtime_ns`, `start_ms`, `bytes`, and the technical keys say theirs
//! ([`TechKey::DurationMs`], [`TechKey::SampleRateHz`]). A technical VALUE is
//! the bare number, so the door can put it through a Fluent `NUMBER` and the
//! decimal mark is the language's.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How big a value a person may write into one field.
///
/// One mebibyte. Lyrics are the long ones, and a whole album's worth of them
/// is a few dozen kilobytes; past this it is not a tag somebody typed, it is a
/// file pasted by accident, and ID3v2 would carry it into every copy.
pub const VALUE_MOST_BYTES: usize = 1024 * 1024;

/// How big a cover may be, as a file on disk or as bytes inside a tag.
///
/// Sixteen mebibytes, the design's number: a 3000 px JPEG is two or three.
/// A cover is copied into the tag of every file of an album, and a player
/// holds it in memory to draw it.
pub const COVER_MOST_BYTES: u64 = 16 * 1024 * 1024;

/// The file as it was when a sheet was read — the guard every write checks.
///
/// # Why these three
///
/// `len` and `mtime_ns` change when anything rewrites the bytes; `ino`
/// changes when something REPLACES the file, which is exactly what our own
/// writer does (`rename` over it) and what every other tagger that writes a
/// copy does too. Two sheets with one stamp describe one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stamp {
    /// Its size, in bytes.
    pub len: u64,
    /// When its bytes last changed, in nanoseconds since the epoch.
    pub mtime_ns: i64,
    /// Its inode.
    pub ino: u64,
}

impl Stamp {
    /// What an ADDRESS is stamped with: nothing on this machine has its
    /// bytes, so no write can name it — and every write refuses one before
    /// it looks (`Why::NotAFile`).
    pub const NONE: Self = Self {
        len: 0,
        mtime_ns: 0,
        ino: 0,
    };

    /// The stamp of what a `stat` answered.
    #[must_use]
    pub fn of(meta: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;
        Self {
            len: meta.len(),
            mtime_ns: meta
                .mtime()
                .saturating_mul(1_000_000_000)
                .saturating_add(meta.mtime_nsec()),
            ino: meta.ino(),
        }
    }
}

/// What kind of medium a file is, which decides the rows always shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Sound only, or sound with a cover.
    Music,
    /// A moving picture.
    Video,
}

/// A field of the Metadata section.
///
/// The music and video lists share `Title`, `Date`, `Genre`, `Artist` and
/// `Comment`; the rest belong to one of them. A field of the other kind is
/// still shown when the file carries it — see [`Field::common`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Field {
    /// The work's own name.
    Title,
    /// Who performs it.
    Artist,
    /// The album it is on.
    Album,
    /// Who the album is by, when it is not the track's artist.
    AlbumArtist,
    /// `n` or `n/total`.
    Track,
    /// `n` or `n/total`.
    Disc,
    /// `YYYY`, `YYYY-MM`, `YYYY-MM-DD` or with a time (ISO 8601).
    Date,
    /// Its genre, as the file writes it.
    Genre,
    /// Who wrote the music.
    Composer,
    /// Who wrote the words.
    Lyricist,
    /// A free note. Several lines.
    Comment,
    /// The words, unsynchronised. Several lines.
    Lyrics,
    /// The label or publisher.
    Publisher,
    /// The recording's ISRC.
    Isrc,
    /// What a film or episode is about. Several lines.
    Description,
    /// The series an episode belongs to.
    Show,
    /// The season's number.
    Season,
    /// The episode's number.
    Episode,
    /// Who directed it.
    Director,
}

/// How many lines a field's value may have, which decides how it is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lines {
    /// One line: a typed question in the minibuffer.
    One,
    /// Several: a draft in the editor (the plan's Emacs route).
    Many,
}

impl Field {
    /// Every field, in the order a sheet lists them.
    pub const ALL: [Self; 19] = [
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::AlbumArtist,
        Self::Track,
        Self::Disc,
        Self::Date,
        Self::Genre,
        Self::Composer,
        Self::Lyricist,
        Self::Comment,
        Self::Lyrics,
        Self::Publisher,
        Self::Isrc,
        Self::Description,
        Self::Show,
        Self::Season,
        Self::Episode,
        Self::Director,
    ];

    /// The rows ALWAYS shown for this kind of file, blank ones included.
    ///
    /// The owner's rule: a blank field is a row that can be filled, not an
    /// absent one. A field outside this list is shown only when the file
    /// carries it.
    #[must_use]
    pub const fn common(kind: Kind) -> &'static [Self] {
        match kind {
            Kind::Music => &[
                Self::Title,
                Self::Artist,
                Self::Album,
                Self::AlbumArtist,
                Self::Track,
                Self::Disc,
                Self::Date,
                Self::Genre,
                Self::Composer,
                Self::Lyricist,
                Self::Comment,
                Self::Lyrics,
                Self::Publisher,
                Self::Isrc,
            ],
            Kind::Video => &[
                Self::Title,
                Self::Description,
                Self::Date,
                Self::Genre,
                Self::Show,
                Self::Season,
                Self::Episode,
                Self::Director,
                Self::Artist,
                Self::Comment,
            ],
        }
    }

    /// The keys ffmpeg (and so mpv, and ffprobe) gives this field, the
    /// first being the one a remux writes.
    #[must_use]
    pub const fn ffmpeg_keys(self) -> &'static [&'static str] {
        match self {
            Self::Title => &["title"],
            Self::Artist => &["artist"],
            Self::Album => &["album"],
            Self::AlbumArtist => &["album_artist"],
            Self::Track => &["track"],
            Self::Disc => &["disc"],
            Self::Date => &["date"],
            Self::Genre => &["genre"],
            Self::Composer => &["composer"],
            Self::Lyricist => &["lyricist"],
            Self::Comment => &["comment"],
            Self::Lyrics => &["lyrics"],
            Self::Publisher => &["publisher", "label"],
            Self::Isrc => &["isrc", "tsrc"],
            Self::Description => &["description", "synopsis"],
            Self::Show => &["show"],
            Self::Season => &["season_number"],
            Self::Episode => &["episode_sort"],
            Self::Director => &["director"],
        }
    }

    /// One line or several.
    #[must_use]
    pub const fn lines(self) -> Lines {
        match self {
            Self::Comment | Self::Lyrics | Self::Description => Lines::Many,
            _ => Lines::One,
        }
    }

    /// A value as it will be written, or why it cannot be.
    ///
    /// The DOOR asks this before it sends anything, so a bad date is refused
    /// under the cursor; the writer asks again, because a door is not the
    /// only thing that can send an edit. An empty answer is `Ok("")`, which
    /// the writer reads as "clear the field".
    ///
    /// # Errors
    ///
    /// [`Why::BadNumber`] for a track, disc, season or episode that is not
    /// one; [`Why::BadDate`]; [`Why::BadIsrc`]; [`Why::TooBig`] past
    /// [`VALUE_MOST_BYTES`].
    pub fn check(self, raw: &str) -> Result<String, Why> {
        if raw.len() > VALUE_MOST_BYTES {
            return Err(Why::TooBig {
                bytes: raw.len() as u64,
                most_bytes: VALUE_MOST_BYTES as u64,
            });
        }
        let text = match self.lines() {
            Lines::One => one_line(raw),
            Lines::Many => many_lines(raw),
        };
        if text.is_empty() {
            return Ok(text);
        }
        match self {
            Self::Track | Self::Disc => numbered(&text).map(|n| n.to_string()),
            Self::Season | Self::Episode => count(&text).map(|n| n.to_string()),
            Self::Date => date(&text),
            Self::Isrc => isrc(&text),
            _ => Ok(text),
        }
    }
}

/// A number, and the total it is out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Numbered {
    /// Which one: at least 1.
    pub number: u32,
    /// Out of how many, when said: at least `number`.
    pub total: Option<u32>,
}

impl std::fmt::Display for Numbered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.total {
            Some(total) => write!(f, "{}/{total}", self.number),
            None => write!(f, "{}", self.number),
        }
    }
}

/// The most a track, disc, season or episode number may be.
///
/// Four digits. It is not a limit anybody meets; it is what keeps "12345678"
/// — a pasted barcode — from being taken for a track number.
const NUMBER_MOST: u32 = 9999;

/// `n` or `n/total`, spaces around the slash allowed.
///
/// # Errors
///
/// [`Why::BadNumber`] when either part is not digits, `n` is 0, or the total
/// is below `n`.
pub fn numbered(text: &str) -> Result<Numbered, Why> {
    let (number, total) = match text.split_once('/') {
        Some((n, t)) => (n.trim(), Some(t.trim())),
        None => (text.trim(), None),
    };
    let number = count(number)?;
    if number == 0 {
        return Err(Why::BadNumber);
    }
    let total = match total {
        None => None,
        Some(t) => {
            let t = count(t)?;
            if t < number {
                return Err(Why::BadNumber);
            }
            Some(t)
        }
    };
    Ok(Numbered { number, total })
}

/// A plain count, 0 allowed (season 0 is where the specials live).
///
/// # Errors
///
/// [`Why::BadNumber`] for anything but one to four ASCII digits.
pub fn count(text: &str) -> Result<u32, Why> {
    let text = text.trim();
    if text.is_empty() || text.len() > 4 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Why::BadNumber);
    }
    let n: u32 = text.parse().map_err(|_| Why::BadNumber)?;
    if n > NUMBER_MOST {
        return Err(Why::BadNumber);
    }
    Ok(n)
}

/// An ISO 8601 date as every tag format that has one accepts it.
///
/// `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, then optionally `THH:MM` or `THH:MM:SS`
/// (a space is accepted for the `T` and written as `T`). That is the ID3v2.4
/// `TDRC` grammar, which is the strictest of the formats this crate writes;
/// Vorbis comments, MP4 `©day` and Matroska's `DATE_*` all take it.
///
/// # Errors
///
/// [`Why::BadDate`] for any other shape, and for a day that month does not
/// have — `2023-02-29` is refused, `2024-02-29` is not.
pub fn date(text: &str) -> Result<String, Why> {
    let text = text.trim();
    let (day, time) = match text.find(['T', ' ']) {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    };
    let parts: Vec<&str> = day.split('-').collect();
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    let ok_shape = match parts.as_slice() {
        [y] => digits(y, 4),
        [y, m] => digits(y, 4) && digits(m, 2),
        [y, m, d] => digits(y, 4) && digits(m, 2) && digits(d, 2),
        _ => false,
    };
    if !ok_shape {
        return Err(Why::BadDate);
    }
    let year: u32 = parts[0].parse().map_err(|_| Why::BadDate)?;
    if year == 0 {
        return Err(Why::BadDate);
    }
    if let Some(m) = parts.get(1) {
        let month: u32 = m.parse().map_err(|_| Why::BadDate)?;
        if !(1..=12).contains(&month) {
            return Err(Why::BadDate);
        }
        if let Some(d) = parts.get(2) {
            let day_n: u32 = d.parse().map_err(|_| Why::BadDate)?;
            if day_n == 0 || day_n > days_in(year, month) {
                return Err(Why::BadDate);
            }
        }
    }
    let Some(time) = time else {
        return Ok(day.to_owned());
    };
    // A time needs the whole day before it: `2024T10:00` is not a date.
    if parts.len() != 3 {
        return Err(Why::BadDate);
    }
    let pieces: Vec<&str> = time.split(':').collect();
    if !(2..=3).contains(&pieces.len()) || !pieces.iter().all(|p| digits(p, 2)) {
        return Err(Why::BadDate);
    }
    let most = [23_u32, 59, 59];
    for (piece, most) in pieces.iter().zip(most) {
        let n: u32 = piece.parse().map_err(|_| Why::BadDate)?;
        if n > most {
            return Err(Why::BadDate);
        }
    }
    Ok(format!("{day}T{time}"))
}

/// Days in a month of the proleptic Gregorian calendar.
const fn days_in(year: u32, month: u32) -> u32 {
    match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// An ISRC, written the way every format stores it: twelve characters, no
/// hyphens, upper case.
///
/// # Errors
///
/// [`Why::BadIsrc`] unless it is two letters (the country), three letters or
/// digits (the registrant), two digits (the year) and five digits.
pub fn isrc(text: &str) -> Result<String, Why> {
    let code: String = text
        .chars()
        .filter(|c| !matches!(c, '-' | ' '))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let b = code.as_bytes();
    let ok = b.len() == 12
        && b[..2].iter().all(u8::is_ascii_uppercase)
        && b[2..5].iter().all(u8::is_ascii_alphanumeric)
        && b[5..].iter().all(u8::is_ascii_digit);
    if ok { Ok(code) } else { Err(Why::BadIsrc) }
}

/// A stream's language as Matroska takes it: ISO 639-2 (`por`), ISO 639-1
/// (`pt`) or a BCP 47 tag built on one (`pt-BR`).
///
/// # Errors
///
/// [`Why::BadLanguage`] for anything else — `mkvpropedit` would refuse it
/// too, but after the draft was made and with a message in English.
pub fn language(text: &str) -> Result<String, Why> {
    let text = text.trim();
    let mut parts = text.split('-');
    let primary = parts.next().unwrap_or_default();
    let primary_ok =
        (2..=3).contains(&primary.len()) && primary.bytes().all(|b| b.is_ascii_lowercase());
    let rest_ok =
        parts.all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()));
    if primary_ok && rest_ok {
        Ok(text.to_owned())
    } else {
        Err(Why::BadLanguage)
    }
}

/// How long a tag's NAME may be: a kibibyte. A name is a word or a short
/// description (`TXXX:REPLAYGAIN_TRACK_GAIN`); past this it is a value typed
/// on the wrong line.
pub const NAME_MOST_BYTES: usize = 1024;

/// The name a tag is written by in the tag `origin` names, or why that tag
/// takes no such name — the ONE grammar, which the Every-tag page asks
/// before it sends a new tag and the writer asks again (a door is not the
/// only thing that sends an edit).
///
/// Every name the readers list comes back as it went (the rule a row's
/// edit relies on); a name typed for a NEW tag is made the format's own:
///
/// - Vorbis comments: printable ASCII without `=` (the specification's
///   0x20–0x7D), as typed.
/// - APEv2: 2 to 255 printable ASCII characters, none of its four reserved
///   names.
/// - RIFF INFO: four letters or digits (`INAM`, `ISFT`).
/// - AIFF text chunks: its four — `NAME`, `AUTH`, `ANNO` (or `COMT`), and
///   `(c) ` (typed `(c)`).
/// - ID3v2: a frame's id where it is one (`TMOO`, `WOAR`, `COMM`), a
///   described frame as `TXXX:<description>`, `COMM:<description>`,
///   `USLT:<description>`; any other word is a user text frame's
///   description, and becomes `TXXX:<word>`.
/// - MP4: a four-character atom that starts as Apple's do, with `©` or a
///   small letter (`©nam`, `desc`, `aART`), a freeform one as
///   `----:<mean>:<name>`; any other word becomes
///   `----:com.apple.iTunes:<word>`, the freeform iTunes and every tagger
///   read.
/// - Matroska: any name, in the file's own tags (`Target::Global`); a
///   track's, chapter's or attachment's tag is [`Why::NotGlobal`].
/// - ffmpeg's (a remuxed container's): anything without `=`, which ends the
///   name in `-metadata name=value`.
///
/// # Errors
///
/// [`Why::BadName`] for a name the format's grammar refuses (and for an
/// empty one, one with a control character, one past [`NAME_MOST_BYTES`]);
/// [`Why::NoPlace`] for ID3v1, which has no names; [`Why::NotGlobal`];
/// [`Why::ReadOnlyView`] for exiftool's rows and [`Why::NotAFile`] for a
/// stream's, which nothing here writes.
pub fn tag_name(origin: &Origin, raw: &str) -> Result<String, Why> {
    let name = raw.trim();
    match origin {
        Origin::Exiftool { .. } => return Err(Why::ReadOnlyView),
        Origin::Stream => return Err(Why::NotAFile),
        Origin::Matroska { target } if *target != Target::Global => {
            return Err(Why::NotGlobal);
        }
        Origin::Lofty {
            format: TagFormat::Id3v1,
        } => return Err(Why::NoPlace),
        _ => {}
    }
    if name.is_empty() || name.len() > NAME_MOST_BYTES || name.chars().any(char::is_control) {
        return Err(Why::BadName);
    }
    let fits = |ok: bool| {
        if ok {
            Ok(name.to_owned())
        } else {
            Err(Why::BadName)
        }
    };
    match origin {
        Origin::Lofty { format } => match format {
            TagFormat::Vorbis => fits(
                name.bytes()
                    .all(|b| (0x20..=0x7D).contains(&b) && b != b'='),
            ),
            TagFormat::Ape => {
                let reserved = ["ID3", "TAG", "OggS", "MP+"];
                fits(
                    (2..=255).contains(&name.len())
                        && name.bytes().all(|b| (0x20..=0x7E).contains(&b))
                        && !reserved.iter().any(|r| r.eq_ignore_ascii_case(name)),
                )
            }
            TagFormat::RiffInfo => {
                fits(name.len() == 4 && name.bytes().all(|b| b.is_ascii_alphanumeric()))
            }
            // `(c) ` ends in a space, which the trim took: it is the one name
            // of the four a person cannot type as it is written.
            TagFormat::AiffText => match name {
                "NAME" | "AUTH" | "ANNO" | "COMT" => Ok(name.to_owned()),
                "(c)" => Ok("(c) ".to_owned()),
                _ => Err(Why::BadName),
            },
            TagFormat::Id3v2 => Ok(id3_name(name)),
            TagFormat::Mp4 => mp4_name(name),
            TagFormat::Id3v1 => Err(Why::NoPlace),
        },
        Origin::Matroska { .. } => Ok(name.to_owned()),
        Origin::Ffprobe { .. } => fits(!name.contains('=')),
        Origin::Exiftool { .. } => Err(Why::ReadOnlyView),
        Origin::Stream => Err(Why::NotAFile),
    }
}

/// Whether `id` has the shape of an ID3v2.3/2.4 frame id: four upper-case
/// letters or digits.
#[must_use]
pub fn frame_like(id: &str) -> bool {
    id.len() == 4
        && id
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// An ID3v2 name: a frame's id, a described frame, or a user text frame's
/// description.
fn id3_name(name: &str) -> String {
    let a_frame = |id: &str| {
        frame_like(id)
            && (id.starts_with('T') || id.starts_with('W') || id == "COMM" || id == "USLT")
    };
    if a_frame(name) {
        return name.to_owned();
    }
    if let Some((id, _)) = name.split_once(':')
        && matches!(id, "TXXX" | "WXXX" | "COMM" | "USLT")
    {
        return name.to_owned();
    }
    format!("TXXX:{name}")
}

/// An MP4 name: a four-character atom, a freeform one, or a word made a
/// freeform iTunes atom.
fn mp4_name(name: &str) -> Result<String, Why> {
    // A FOURCC is four BYTES, Latin-1 (`©` is 0xA9), and Apple's begin
    // with `©` or a small letter (`©nam`, `desc`, `aART`): a word like
    // `Mood` or `MOOD` is a person's name for a freeform atom.
    let mut chars = name.chars();
    let atom_start = chars
        .next()
        .is_some_and(|c| c == '\u{a9}' || c.is_ascii_lowercase());
    if atom_start && name.chars().count() == 4 && name.chars().all(|c| u32::from(c) <= 0xFF) {
        return Ok(name.to_owned());
    }
    if let Some(rest) = name.strip_prefix("----:") {
        return match rest.split_once(':') {
            Some((mean, n)) if !mean.is_empty() && !n.is_empty() => Ok(name.to_owned()),
            _ => Err(Why::BadName),
        };
    }
    Ok(format!("----:com.apple.iTunes:{name}"))
}

/// A tag's value typed on one line: every control character and run of
/// whitespace becomes one space.
///
/// # Errors
///
/// [`Why::TooBig`] past [`VALUE_MOST_BYTES`].
pub fn tag_text(raw: &str) -> Result<String, Why> {
    if raw.len() > VALUE_MOST_BYTES {
        return Err(Why::TooBig {
            bytes: raw.len() as u64,
            most_bytes: VALUE_MOST_BYTES as u64,
        });
    }
    Ok(one_line(raw))
}

/// One line: every control character and run of whitespace becomes one
/// space, and the ends are trimmed.
///
/// A newline in a title is not a title with two lines; it is a paste that
/// brought one along, and ID3v1, RIFF INFO and every one-line register on
/// screen would show it as garbage.
fn one_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut space = false;
    for c in raw.chars() {
        if c.is_control() || c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    out
}

/// Several lines: `\r\n` and `\r` become `\n`, other control characters go,
/// and blank lines at either end are trimmed.
fn many_lines(raw: &str) -> String {
    let unified = raw.replace("\r\n", "\n").replace('\r', "\n");
    let kept: String = unified
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect();
    kept.trim_matches(|c: char| c == '\n' || c.is_whitespace())
        .to_owned()
}

/// One row of the Metadata section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldRow {
    /// Which field.
    pub field: Field,
    /// What the file says, or nothing — a blank row is still a row.
    pub value: Option<String>,
    /// Whether it can be changed, and how.
    pub can: Can,
}

/// Whether something can be written, and by what — or why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Can {
    /// Yes, by this writer.
    Yes(Writer),
    /// No, and the reason, which the door says.
    No(Why),
}

impl Can {
    /// Whether it can.
    #[must_use]
    pub const fn can(&self) -> bool {
        matches!(self, Self::Yes(_))
    }
}

/// What writes a file's tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Writer {
    /// lofty, in process: ID3, APE, Vorbis comments, MP4 `ilst`, RIFF INFO.
    Lofty,
    /// `mkvpropedit`, after `mkvextract` read what is there.
    Matroska,
    /// `ffmpeg -c copy` into a new file, where the muxer keeps every stream.
    Remux,
}

/// A program this crate runs, by the variable that names its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Program {
    /// Streams, chapters, programs and the container's own tags.
    Ffprobe,
    /// The remux, and a cover out of an attached picture.
    Ffmpeg,
    /// Every tag of every group, read only.
    Exiftool,
    /// Matroska's tags, as XML.
    Mkvextract,
    /// Matroska's tracks and attachments, as JSON.
    Mkvmerge,
    /// Matroska, edited in place.
    Mkvpropedit,
}

impl Program {
    /// Every program, so a check can walk them.
    pub const ALL: [Self; 6] = [
        Self::Ffprobe,
        Self::Ffmpeg,
        Self::Exiftool,
        Self::Mkvextract,
        Self::Mkvmerge,
        Self::Mkvpropedit,
    ];

    /// Its own name, as a sentence says which program failed — a program's
    /// name is data, the same in every language (its serde name).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ffprobe => "ffprobe",
            Self::Ffmpeg => "ffmpeg",
            Self::Exiftool => "exiftool",
            Self::Mkvextract => "mkvextract",
            Self::Mkvmerge => "mkvmerge",
            Self::Mkvpropedit => "mkvpropedit",
        }
    }

    /// The environment variable holding its store path.
    ///
    /// The same names `ronoctua_inspect::tools::Tool` reads for the three
    /// both crates run — one variable per program in the whole tree — but
    /// NOT that enum: adding the Matroska three to it would make
    /// `document-formats-are-total` demand them in the footer's unit, and
    /// they belong to the player's.
    #[must_use]
    pub const fn variable(self) -> &'static str {
        match self {
            Self::Ffprobe => "RONOCTUA_FFPROBE",
            Self::Ffmpeg => "RONOCTUA_FFMPEG",
            Self::Exiftool => "RONOCTUA_EXIFTOOL",
            Self::Mkvextract => "RONOCTUA_MKVEXTRACT",
            Self::Mkvmerge => "RONOCTUA_MKVMERGE",
            Self::Mkvpropedit => "RONOCTUA_MKVPROPEDIT",
        }
    }
}

/// The container, which decides the writer (see [`crate::can`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Container {
    /// MPEG audio (MP3, MP2).
    Mpeg,
    /// Raw AAC (ADTS).
    Aac,
    /// FLAC.
    Flac,
    /// Ogg holding Vorbis.
    OggVorbis,
    /// Ogg holding Opus.
    Opus,
    /// Ogg holding Speex.
    Speex,
    /// Ogg holding anything else — Theora, most often.
    OggOther,
    /// ISO media: MP4, M4A, M4V, and QuickTime with an `ftyp`.
    Mp4,
    /// RIFF WAVE.
    Wav,
    /// AIFF and AIFF-C.
    Aiff,
    /// Monkey's Audio.
    Ape,
    /// Musepack.
    Mpc,
    /// WavPack.
    WavPack,
    /// Matroska (`.mkv`, `.mka`, `.mks`).
    Matroska,
    /// WebM, which is Matroska with fewer codecs.
    WebM,
    /// AVI.
    Avi,
    /// MPEG transport stream.
    MpegTs,
    /// Flash video.
    Flv,
    /// ASF: WMA and WMV.
    Asf,
    /// Something ffprobe opened and this table does not name.
    Other,
}

/// Where a technical fact came from is ffprobe; what it IS is one of these.
///
/// The value beside it is the bare datum — a number without its unit, or a
/// name as ffmpeg spells it (`h264`, `stereo`). The unit is in the key's name
/// and the sentence is the door's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TechKey {
    /// ffmpeg's long name for the container (`QuickTime / MOV`).
    Container,
    /// How long it runs.
    DurationMs,
    /// The whole file's bit rate.
    BitRateBitsPerSecond,
    /// The first picture's codec (`h264`).
    VideoCodec,
    /// Its width.
    WidthPx,
    /// Its height.
    HeightPx,
    /// Its frames per second, with up to three decimals (`23.976`).
    FramesPerSecond,
    /// The first sound's codec (`aac`).
    AudioCodec,
    /// Its sample rate.
    SampleRateHz,
    /// Its channel count.
    Channels,
    /// Its layout as ffmpeg names it (`5.1(side)`).
    ChannelLayout,
    /// Bits per sample, when the codec says.
    BitsPerSample,
    /// How many programs a transport stream carries.
    Programs,
}

/// One technical row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    /// What it is.
    pub key: TechKey,
    /// The datum.
    pub value: String,
}

/// What a stream is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamKind {
    /// Moving picture — or a cover, when `attached` says so.
    Video,
    /// Sound.
    Audio,
    /// Subtitles.
    Subtitle,
    /// Timed data (timecode, GPS, chapters as a track).
    Data,
    /// A file carried inside (Matroska fonts and covers).
    Attachment,
}

/// One stream of the container, for the Streams page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stream {
    /// ffprobe's index, which is also `-map 0:<index>`.
    pub index: u32,
    /// What it is.
    pub kind: StreamKind,
    /// Its codec as ffmpeg names it.
    pub codec: Option<String>,
    /// Its declared language.
    pub lang: Option<String>,
    /// Its own title.
    pub title: Option<String>,
    /// Marked default.
    pub default: bool,
    /// Marked forced.
    pub forced: bool,
    /// Marked for the hard of hearing.
    pub hearing: bool,
    /// A still picture carried as a stream (a cover), not a film.
    pub attached: bool,
    /// Whether language, title and the two marks can be changed.
    pub can: Can,
}

/// A chapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chapter {
    /// Where it starts.
    pub start_ms: u64,
    /// Where it ends, when the container says.
    pub end_ms: Option<u64>,
    /// Its title.
    pub title: Option<String>,
}

/// What a picture inside a file is for.
///
/// Fewer than ID3's twenty-one: the rest (`BrightFish`, `DuringRecording`…)
/// are read as [`CoverRole::Other`], which is what nearly every player does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverRole {
    /// The front cover — what "the cover" means.
    Front,
    /// The back cover.
    Back,
    /// A booklet page.
    Leaflet,
    /// The disc itself.
    Media,
    /// The artist, the band or the conductor.
    Artist,
    /// A small icon.
    Icon,
    /// Anything else.
    Other,
}

/// Where a cover lives inside the file, so it can be taken out again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "at")]
pub enum CoverAt {
    /// The `index`-th picture of a lofty tag.
    Tag {
        /// Which tag format holds it.
        format: TagFormat,
        /// Its place among that tag's pictures.
        index: u32,
    },
    /// An attached picture ffmpeg exposes as a stream (Matroska attachments
    /// with an image type, MP4 `covr` lofty could not read, ASF).
    Stream {
        /// ffprobe's index.
        index: u32,
        /// The Matroska attachment's file name, which is how `mkvpropedit`
        /// finds it again.
        name: Option<String>,
    },
}

/// A picture inside the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cover {
    /// What it is for.
    pub role: CoverRole,
    /// Its MIME type (`image/jpeg`).
    pub mime: String,
    /// Its width, when its header could be read.
    pub width: Option<u32>,
    /// Its height, when its header could be read.
    pub height: Option<u32>,
    /// How many bytes it takes, when the reader knows without taking it
    /// out (ffprobe does not).
    pub bytes: Option<u64>,
    /// Where it is.
    pub at: CoverAt,
    /// Whether it can be replaced or removed.
    pub can: Can,
}

/// Which lofty tag a native name belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TagFormat {
    /// ID3v2 (`TIT2`).
    Id3v2,
    /// ID3v1's fixed fields.
    Id3v1,
    /// APEv2 (`Title`).
    Ape,
    /// Vorbis comments (`TITLE`).
    Vorbis,
    /// MP4 `ilst` (`©nam`).
    Mp4,
    /// RIFF INFO (`INAM`).
    RiffInfo,
    /// AIFF text chunks (`NAME`).
    AiffText,
}

/// Which place in a Matroska file a tag targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    /// The whole file (no UID in the tag's targets).
    Global,
    /// One track.
    Track,
    /// One edition, chapter or attachment.
    Other,
}

/// Who read a tag, which decides whether it can be edited here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "origin")]
pub enum Origin {
    /// lofty, from this tag format; the name is the format's own.
    Lofty {
        /// Which tag.
        format: TagFormat,
    },
    /// mkvextract, from a Matroska `Tag`.
    Matroska {
        /// What it targets.
        target: Target,
    },
    /// ffprobe: the container's (`stream: None`) or a stream's tags.
    Ffprobe {
        /// Which stream, if any.
        stream: Option<u32>,
    },
    /// exiftool, with the group `-G1` gives (`ID3v2_4`, `QuickTime`).
    Exiftool {
        /// The group.
        group: String,
    },
    /// What a STREAM says of itself while it plays — a radio's ICY words
    /// (`icy-title`, `icy-name`), or the tags a file on the network carries
    /// as the player read them. Read only: nothing here has its bytes.
    Stream,
}

/// How much of a tag's value a sheet carries: four kibibytes, a row's worth
/// and then some. Past it the value is [`TagValue::Cut`] and says how long it
/// really was.
pub const TAG_VALUE_BYTES: usize = 4096;

/// A tag's value as the Every-tag page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TagValue {
    /// Text, whole.
    Text(String),
    /// Text too long for a row: its beginning, and how long it really is.
    Cut {
        /// The first part, cut on a character boundary.
        head: String,
        /// The whole value's size.
        bytes: u64,
    },
    /// Bytes that are not text (a picture, a fingerprint).
    Binary {
        /// How many.
        bytes: u64,
    },
}

impl TagValue {
    /// Text as a sheet carries it: whole, or its first [`TAG_VALUE_BYTES`]
    /// cut on a character boundary.
    #[must_use]
    pub fn of(text: &str) -> Self {
        if text.len() <= TAG_VALUE_BYTES {
            return Self::Text(text.to_owned());
        }
        let mut end = TAG_VALUE_BYTES;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        Self::Cut {
            head: text[..end].to_owned(),
            bytes: text.len() as u64,
        }
    }
}

/// One tag, by the name its container gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    /// Who read it.
    pub origin: Origin,
    /// Its name, as written in the file — data, never a key.
    pub name: String,
    /// Its value.
    pub value: TagValue,
    /// Whether it can be edited from the Every-tag page.
    pub can: Can,
}

/// A source of the sheet that could not be read, and why.
///
/// A sheet is still made when exiftool is missing — with fewer rows. This is
/// how the door says "fewer, and here is why" instead of drawing a shorter
/// page as if it were the whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unread {
    /// What was not read.
    pub source: Source,
    /// Why.
    pub why: Why,
}

/// A source of a sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// lofty's tags and pictures.
    Lofty,
    /// ffprobe's streams, chapters and facts.
    Ffprobe,
    /// exiftool's dump.
    Exiftool,
    /// Matroska's own tags and attachments.
    Matroska,
}

/// Where a tag ADDED by its own name goes — the Every-tag page's "Add a
/// tag" — and whether one can be.
///
/// The file's main tag: lofty's primary format for its type (Vorbis
/// comments in a FLAC, ID3v2 in an MP3, `ilst` in an MP4), Matroska's own
/// global tags, the container's for a remux. A tag added anywhere else
/// would be one no player reads first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adding {
    /// The tag it goes into, when the file has one this crate writes.
    pub origin: Option<Origin>,
    /// Whether it can be, and by what — or why not.
    pub can: Can,
}

/// Everything a file says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    /// The file, canonical.
    pub location: PathBuf,
    /// The file as it was read — every write is checked against it.
    pub stamp: Stamp,
    /// Music or video.
    pub kind: Kind,
    /// The container.
    pub container: Container,
    /// The common rows, blanks included, then any other field it carries.
    pub fields: Vec<FieldRow>,
    /// The pictures inside it.
    pub covers: Vec<Cover>,
    /// Whether a front cover can be SET (added or replaced) here.
    pub cover_can: Can,
    /// Technical facts.
    pub technical: Vec<Fact>,
    /// The streams.
    pub streams: Vec<Stream>,
    /// The chapters.
    pub chapters: Vec<Chapter>,
    /// Every tag, from every reader, by its own name.
    pub other: Vec<Tag>,
    /// What could not be read.
    pub unread: Vec<Unread>,
    /// Where the player KEPT its cover as a file of its own — the player's
    /// cache (`$XDG_CACHE_HOME/ronoctua/player`), written by the daemon
    /// alone — so a door can draw the picture a file carries without
    /// reading its tags itself. `None` with no cover, or none kept: a
    /// reader never sets it; the daemon does, when it answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_file: Option<PathBuf>,
    /// Where a tag added by its own name goes, and whether one can be.
    /// `None` from a daemon that predates it: the door offers no row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adding: Option<Adding>,
}

impl Sheet {
    /// A field's row.
    #[must_use]
    pub fn field(&self, field: Field) -> Option<&FieldRow> {
        self.fields.iter().find(|row| row.field == field)
    }

    /// A field's value, when the file says one (a blank is no value).
    #[must_use]
    pub fn value(&self, field: Field) -> Option<&str> {
        self.field(field)
            .and_then(|row| row.value.as_deref())
            .map(str::trim)
            .filter(|v| !v.is_empty())
    }

    /// "The cover": the picture marked front, else the first picture —
    /// what a player shows and what MPRIS is handed.
    #[must_use]
    pub fn front_cover(&self) -> Option<&Cover> {
        self.covers
            .iter()
            .find(|c| c.role == CoverRole::Front)
            .or_else(|| self.covers.first())
    }
}

/// One change a person asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "edit")]
pub enum Edit {
    /// Write a field. An empty value clears it.
    Set {
        /// Which.
        field: Field,
        /// The new value, as typed ([`Field::check`] runs again).
        value: String,
    },
    /// Remove a field.
    Clear {
        /// Which.
        field: Field,
    },
    /// Write a tag by its own name, from the Every-tag page — a row whose
    /// `can` said yes, or one added ([`Sheet::adding`]).
    ///
    /// A NAME IS WRITTEN WHOLE: every value it had in that tag (a Vorbis
    /// comment may say ARTIST twice) is replaced by this one, which is what
    /// the person typed.
    SetTag {
        /// Where the row came from; only a row whose `can` said yes.
        origin: Origin,
        /// Its name ([`tag_name`] checks it again).
        name: String,
        /// The value. Empty removes it.
        value: String,
    },
    /// Remove a tag by its own name — every value it has in that tag.
    RemoveTag {
        /// Where the row came from.
        origin: Origin,
        /// Its name.
        name: String,
    },
    /// Set or remove the front cover.
    Cover(CoverEdit),
    /// Change a stream (Matroska only).
    Stream {
        /// ffprobe's index.
        index: u32,
        /// What.
        change: StreamChange,
    },
}

/// A change to the front cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "cover")]
pub enum CoverEdit {
    /// Put this image in, in place of the front cover (or as it, when there
    /// is none). JPEG and PNG go in as they are; anything else the `image`
    /// crate reads goes in as PNG.
    Replace {
        /// The image.
        from: PathBuf,
    },
    /// Take the front cover out.
    Remove,
}

/// A change to one stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "change", content = "to")]
pub enum StreamChange {
    /// Its language ([`language`] checks it).
    Language(String),
    /// Its title; empty removes it.
    Title(String),
    /// The default mark.
    Default(bool),
    /// The forced mark.
    Forced(bool),
}

/// Why something cannot be read or written — one sentence each, on screen.
///
/// A VALUE and never an absence (`AGENTS.md`): a panel with no rows and a
/// panel that could not be read look the same, and only one of them says so.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "why")]
pub enum Why {
    /// Not a regular file: a folder, a device, an address.
    NotAFile,
    /// It is not there any more.
    Gone,
    /// It changed since the sheet was read.
    ChangedSince,
    /// It has other names (hard links), which a replaced file would lose.
    HardLinked,
    /// Its folder cannot be written.
    ReadOnlyFolder,
    /// The file itself is marked read only.
    ReadOnlyFile,
    /// Another user owns it; the new copy would be ours.
    NotYours,
    /// Not enough room for the copy the writer needs.
    NoRoom {
        /// How much it needs.
        need_bytes: u64,
        /// How much there is.
        free_bytes: u64,
    },
    /// Its tags could not be read.
    Unreadable,
    /// Writing failed; the file is as it was.
    Unwritable,
    /// Nothing here writes this container's tags.
    NoWriter {
        /// Which container.
        container: Container,
    },
    /// This container, or this tag, has no place for that field or name.
    NoPlace,
    /// A program this needs is not installed.
    Missing {
        /// Which.
        program: Program,
    },
    /// A program failed.
    ProgramFailed {
        /// Which.
        program: Program,
    },
    /// A program took too long and was stopped.
    TimedOut {
        /// Which.
        program: Program,
    },
    /// Too big for this.
    TooBig {
        /// How big it is.
        bytes: u64,
        /// The most allowed.
        most_bytes: u64,
    },
    /// Not an image this crate can read.
    NotAnImage,
    /// Not a number (track, disc, season, episode).
    BadNumber,
    /// Not a date (ISO 8601).
    BadDate,
    /// Not an ISRC.
    BadIsrc,
    /// Not a language code.
    BadLanguage,
    /// A row read to be SHOWN (exiftool's, ffprobe's): it is edited through
    /// the field or the native tag it mirrors.
    ReadOnlyView,
    /// Not a name this kind of tag takes ([`tag_name`]).
    BadName,
    /// The value is data, not text (a picture, a fingerprint): a row shows
    /// its size, and nothing here writes it.
    NotText,
    /// A Matroska tag that belongs to one track, chapter or attachment, or
    /// sits inside another tag: only the file's own tags, at the top, are
    /// written — `mkvpropedit --tags global:` is the one replace that
    /// hands every other tag back untouched.
    NotGlobal,
    /// There is no such cover or stream.
    NoSuchThing,
    /// `cover.<ext>` and `<stem>.cover.<ext>` both exist already.
    Taken,
    /// Somebody said stop.
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::{Field, Kind, Numbered, Why, count, date, isrc, language, numbered};

    #[test]
    fn a_track_is_a_number_or_a_number_out_of_a_total() {
        assert_eq!(
            numbered("3"),
            Ok(Numbered {
                number: 3,
                total: None
            })
        );
        assert_eq!(
            numbered(" 3 / 12 "),
            Ok(Numbered {
                number: 3,
                total: Some(12)
            })
        );
        for bad in [
            "", "0", "3/2", "a", "3/", "/3", "-1", "3.5", "12345", "3/12/1",
        ] {
            assert_eq!(numbered(bad), Err(Why::BadNumber), "{bad:?}");
        }
        assert_eq!(Field::Track.check("3 /12"), Ok("3/12".to_owned()));
        assert_eq!(Field::Disc.check("x"), Err(Why::BadNumber));
    }

    #[test]
    fn a_season_may_be_zero_and_is_never_a_word() {
        assert_eq!(count("0"), Ok(0));
        assert_eq!(Field::Season.check("02"), Ok("2".to_owned()));
        assert_eq!(Field::Episode.check("two"), Err(Why::BadNumber));
    }

    #[test]
    fn a_date_is_the_iso_grammar_and_a_day_the_month_has() {
        for good in [
            "2024",
            "2024-02",
            "2024-02-29",
            "2000-02-29",
            "1999-12-31T23:59:59",
        ] {
            assert_eq!(date(good), Ok(good.to_owned()), "{good}");
        }
        assert_eq!(date("2024-01-02 10:30"), Ok("2024-01-02T10:30".to_owned()));
        for bad in [
            "24",
            "2023-02-29",
            "1900-02-29",
            "2024-13",
            "2024-00",
            "2024-04-31",
            "2024-1-2",
            "2024T10:00",
            "2024-01-01T24:00",
            "2024-01-01T10",
            "0000",
            "yesterday",
        ] {
            assert_eq!(date(bad), Err(Why::BadDate), "{bad}");
        }
    }

    #[test]
    fn an_isrc_is_stored_without_its_hyphens() {
        assert_eq!(isrc("br-abc-24-00001"), Ok("BRABC2400001".to_owned()));
        assert_eq!(isrc("BRABC2400001"), Ok("BRABC2400001".to_owned()));
        for bad in ["BR-ABC-24-0001", "12ABC2400001", "BRABC24X0001", ""] {
            assert_eq!(isrc(bad), Err(Why::BadIsrc), "{bad}");
        }
    }

    #[test]
    fn a_language_is_a_code_and_not_a_name() {
        for good in ["pt", "por", "pt-BR", "zh-Hant-TW"] {
            assert!(language(good).is_ok(), "{good}");
        }
        for bad in ["Portuguese", "PT", "p", "pt_BR", "pt-", ""] {
            assert_eq!(language(bad), Err(Why::BadLanguage), "{bad}");
        }
    }

    #[test]
    fn one_line_fields_lose_their_newlines_and_many_line_fields_keep_them() {
        assert_eq!(Field::Title.check("  a\n b\t\tc  "), Ok("a b c".to_owned()));
        assert_eq!(
            Field::Lyrics.check("\r\nfirst\r\nsecond\u{0}\n\n"),
            Ok("first\nsecond".to_owned())
        );
        assert_eq!(Field::Title.check("   "), Ok(String::new()));
    }

    #[test]
    fn a_pasted_file_is_not_a_field() {
        let big = "x".repeat(super::VALUE_MOST_BYTES + 1);
        assert!(matches!(
            Field::Comment.check(&big),
            Err(Why::TooBig { .. })
        ));
    }

    #[test]
    fn every_common_row_is_a_field_and_none_repeats() {
        for kind in [Kind::Music, Kind::Video] {
            let rows = Field::common(kind);
            for (i, a) in rows.iter().enumerate() {
                assert!(Field::ALL.contains(a));
                assert!(!rows[i + 1..].contains(a), "{a:?} twice in {kind:?}");
            }
        }
        assert_eq!(Field::common(Kind::Music).len(), 14);
        assert_eq!(Field::common(Kind::Video).len(), 10);
    }

    /// FAILS IF: "the cover" stops being the picture marked front — a
    /// player would show the back of the booklet — or a file whose only
    /// picture is not marked has none.
    #[test]
    fn the_cover_is_the_front_else_the_first() {
        use super::{Can, Container, Cover, CoverAt, CoverRole, Sheet, Stamp, TagFormat};
        let picture = |role, index| Cover {
            role,
            mime: "image/png".into(),
            width: None,
            height: None,
            bytes: None,
            at: CoverAt::Tag {
                format: TagFormat::Id3v2,
                index,
            },
            can: Can::No(Why::NotAFile),
        };
        let mut sheet = Sheet {
            location: "/m/a.mp3".into(),
            stamp: Stamp::NONE,
            kind: Kind::Music,
            container: Container::Mpeg,
            fields: Vec::new(),
            covers: vec![picture(CoverRole::Back, 0), picture(CoverRole::Front, 1)],
            cover_can: Can::No(Why::NotAFile),
            technical: Vec::new(),
            streams: Vec::new(),
            chapters: Vec::new(),
            other: Vec::new(),
            unread: Vec::new(),
            cover_file: None,
            adding: None,
        };
        assert_eq!(sheet.front_cover().map(|c| c.role), Some(CoverRole::Front));
        sheet.covers.remove(1);
        assert_eq!(sheet.front_cover().map(|c| c.role), Some(CoverRole::Back));
        sheet.covers.clear();
        assert!(sheet.front_cover().is_none());
    }

    /// FAILS IF: a name a format refuses is sent to the writer, a word
    /// typed for ID3v2 or MP4 stops becoming the frame every tagger reads
    /// (TXXX, the iTunes freeform), a name a reader LISTED comes back
    /// different (its row would edit another tag), or a row nothing writes
    /// stops saying why.
    #[test]
    fn a_tag_s_name_is_its_format_s_own_and_a_listed_one_comes_back_as_it_went() {
        use super::{Origin, TagFormat, Target, tag_name};
        let lofty = |format| Origin::Lofty { format };
        let vorbis = lofty(TagFormat::Vorbis);
        assert_eq!(tag_name(&vorbis, " MOOD "), Ok("MOOD".to_owned()));
        for bad in ["", "A=B", "caf\u{e9}", "a\nb"] {
            assert_eq!(tag_name(&vorbis, bad), Err(Why::BadName), "{bad:?}");
        }
        let ape = lofty(TagFormat::Ape);
        assert_eq!(tag_name(&ape, "Mood"), Ok("Mood".to_owned()));
        assert_eq!(tag_name(&ape, "M"), Err(Why::BadName));
        assert_eq!(tag_name(&ape, "tag"), Err(Why::BadName));
        let riff = lofty(TagFormat::RiffInfo);
        assert_eq!(tag_name(&riff, "ISFT"), Ok("ISFT".to_owned()));
        assert_eq!(tag_name(&riff, "SOFTWARE"), Err(Why::BadName));
        let aiff = lofty(TagFormat::AiffText);
        assert_eq!(tag_name(&aiff, "(c)"), Ok("(c) ".to_owned()));
        assert_eq!(tag_name(&aiff, "(c) "), Ok("(c) ".to_owned()));
        assert_eq!(tag_name(&aiff, "MOOD"), Err(Why::BadName));
        let id3 = lofty(TagFormat::Id3v2);
        for (typed, written) in [
            ("TMOO", "TMOO"),
            ("COMM", "COMM"),
            ("TXXX:REPLAYGAIN_TRACK_GAIN", "TXXX:REPLAYGAIN_TRACK_GAIN"),
            ("COMM:iTunNORM", "COMM:iTunNORM"),
            ("MOOD", "TXXX:MOOD"),
            ("my tag", "TXXX:my tag"),
        ] {
            assert_eq!(tag_name(&id3, typed), Ok(written.to_owned()), "{typed}");
            assert_eq!(tag_name(&id3, written), Ok(written.to_owned()), "{written}");
        }
        let mp4 = lofty(TagFormat::Mp4);
        for (typed, written) in [
            ("\u{a9}nam", "\u{a9}nam"),
            ("----:com.apple.iTunes:MOOD", "----:com.apple.iTunes:MOOD"),
            ("Mood", "----:com.apple.iTunes:Mood"),
            ("MOOD", "----:com.apple.iTunes:MOOD"),
            ("aART", "aART"),
        ] {
            assert_eq!(tag_name(&mp4, typed), Ok(written.to_owned()), "{typed}");
            assert_eq!(tag_name(&mp4, written), Ok(written.to_owned()), "{written}");
        }
        assert_eq!(tag_name(&mp4, "----:only-a-mean"), Err(Why::BadName));
        assert_eq!(
            tag_name(&lofty(TagFormat::Id3v1), "TITLE"),
            Err(Why::NoPlace)
        );
        let global = Origin::Matroska {
            target: Target::Global,
        };
        assert_eq!(
            tag_name(&global, "Mood = high"),
            Ok("Mood = high".to_owned())
        );
        let track = Origin::Matroska {
            target: Target::Track,
        };
        assert_eq!(tag_name(&track, "BPS"), Err(Why::NotGlobal));
        let container = Origin::Ffprobe { stream: None };
        assert_eq!(tag_name(&container, "comment"), Ok("comment".to_owned()));
        assert_eq!(tag_name(&container, "a=b"), Err(Why::BadName));
        let exif = Origin::Exiftool {
            group: "Vorbis".to_owned(),
        };
        assert_eq!(tag_name(&exif, "Vendor"), Err(Why::ReadOnlyView));
        assert_eq!(tag_name(&Origin::Stream, "icy-name"), Err(Why::NotAFile));
        let long = "N".repeat(super::NAME_MOST_BYTES + 1);
        assert_eq!(tag_name(&global, &long), Err(Why::BadName));
    }

    #[test]
    fn a_tag_typed_on_a_line_is_one_line_and_not_a_pasted_file() {
        assert_eq!(super::tag_text("  a\n b  "), Ok("a b".to_owned()));
        let big = "x".repeat(super::VALUE_MOST_BYTES + 1);
        assert!(matches!(super::tag_text(&big), Err(Why::TooBig { .. })));
    }

    #[test]
    fn a_refusal_crosses_the_socket_as_a_tagged_value() {
        let why = Why::NoRoom {
            need_bytes: 10,
            free_bytes: 2,
        };
        let text = serde_json::to_string(&why).expect("it serialises");
        assert_eq!(text, r#"{"why":"no-room","need_bytes":10,"free_bytes":2}"#);
        let back: Why = serde_json::from_str(&text).expect("it reads back");
        assert_eq!(back, why);
    }
}
