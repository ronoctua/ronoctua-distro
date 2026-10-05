//! CHANGING what a file says about itself — the ONE place exiftool writes.
//!
//! # Why one writer
//!
//! Because a tag written two ways is two answers to "what did this change do
//! to my photograph". The info panel's editable rows ([`set`]) and the menu's
//! "remove location" ([`remove_location`]) both land here; nothing else in the
//! tree hands exiftool an assignment, `-o` or `-overwrite_original*`
//! (`one-inspector` holds that). Reading stays in [`crate::facts`], which asks
//! for everything; this module reads back only the few tags it can write
//! ([`read`]), so the row a person edits and the value it shows are one tag.
//!
//! # A whitelist, and why it is short
//!
//! Six things a person means when they say "the photograph's details": its
//! title, who made it, what it shows, its keywords, its copyright and when it
//! was taken ([`Meta`]). Each is ONE named tag per kind of file ([`Holder`]),
//! chosen so every program that reads the file finds it where it looks: the
//! EXIF fields where EXIF has one, XMP's Dublin Core where it does not (a PNG
//! has no IPTC, and writing one draws exiftool's own "non-standard" warning),
//! and a PDF's own Info dictionary. An arbitrary tag name typed by a person is
//! not offered: a name exiftool accepts and writes somewhere unexpected is a
//! change nobody can find again.
//!
//! # In place, keeping the inode
//!
//! `-overwrite_original_in_place`: exiftool writes a temporary copy and then
//! copies it BACK over the file, so hard links, extended attributes and the
//! owner stay what they were. The modification time moves, and that is wanted —
//! it is what tells the manager's panel the file changed and has to be read
//! again.
//!
//! # Removing the place, and the question before it
//!
//! [`remove_location`] never decides between the file and a copy: the caller
//! asks the person first (Copy / Overwrite / No, "no" first), and says which
//! it was. A coordinate taken out of the only copy does not come back.
//!
//! # No thread, no language
//!
//! Synchronous, like the rest of this crate: the shell calls it off the loop
//! that draws. Nothing here is a sentence — [`Refused`] is a variant the door
//! turns into one (invariant 8).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::Refusal;
use crate::tools::{self, Tool};

/// The most a written value may be, in bytes.
///
/// Four kibibytes: a caption, a list of keywords, a copyright line. Past that
/// it is not a detail somebody typed but a text pasted by accident, and it
/// would travel inside every copy of the picture.
pub const VALUE_MOST_BYTES: usize = 4096;

/// What kind of file, which decides the tag each [`Meta`] is written to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Holder {
    /// A still picture exiftool writes: JPEG, PNG, TIFF, WebP, HEIC/HEIF,
    /// AVIF and the camera RAWs it knows.
    Picture,
    /// A PDF, whose own Info dictionary holds the first five.
    Document,
}

impl Holder {
    /// The holder of a file of this type, or `None` when this module writes
    /// nothing into it.
    ///
    /// A LIST and not "any image": a GIF, a BMP or an icon has nowhere most of
    /// these could go, and offering a row that exiftool then refuses would be
    /// a promise the panel breaks.
    #[must_use]
    pub fn of(mime: &str) -> Option<Self> {
        let mime = ronoctua_files::mime::canonical(mime);
        match mime {
            "application/pdf" => Some(Self::Document),
            "image/jpeg"
            | "image/png"
            | "image/tiff"
            | "image/webp"
            | "image/heic"
            | "image/heif"
            | "image/avif"
            | "image/x-adobe-dng"
            | "image/x-canon-cr2"
            | "image/x-canon-cr3"
            | "image/x-nikon-nef"
            | "image/x-sony-arw"
            | "image/x-fuji-raf"
            | "image/x-olympus-orf"
            | "image/x-panasonic-rw2" => Some(Self::Picture),
            _ => None,
        }
    }
}

/// One detail this module writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Meta {
    /// Its title.
    Title,
    /// Who made it: the photographer, the document's author.
    Author,
    /// What it shows or is about.
    Description,
    /// Words to find it by, separated by commas.
    Keywords,
    /// Whose it is.
    Copyright,
    /// When it was taken, or the document created.
    Taken,
}

impl Meta {
    /// Every one, in the order the panel lists them.
    pub const ALL: [Self; 6] = [
        Self::Title,
        Self::Author,
        Self::Description,
        Self::Keywords,
        Self::Copyright,
        Self::Taken,
    ];

    /// The tag it is written to and read from, group included.
    #[must_use]
    pub const fn tag(self, holder: Holder) -> &'static str {
        match (holder, self) {
            (Holder::Picture, Self::Title) => "XMP-dc:Title",
            (Holder::Picture, Self::Author) => "EXIF:Artist",
            (Holder::Picture, Self::Description) => "EXIF:ImageDescription",
            (Holder::Picture, Self::Keywords) => "XMP-dc:Subject",
            (Holder::Picture, Self::Copyright) => "EXIF:Copyright",
            (Holder::Picture, Self::Taken) => "EXIF:DateTimeOriginal",
            (Holder::Document, Self::Title) => "PDF:Title",
            (Holder::Document, Self::Author) => "PDF:Author",
            (Holder::Document, Self::Description) => "PDF:Subject",
            (Holder::Document, Self::Keywords) => "PDF:Keywords",
            (Holder::Document, Self::Copyright) => "XMP-dc:Rights",
            (Holder::Document, Self::Taken) => "PDF:CreateDate",
        }
    }

    /// The name exiftool answers it under — the tag without its group.
    #[must_use]
    pub fn answered_as(self, holder: Holder) -> &'static str {
        let tag = self.tag(holder);
        tag.split_once(':').map_or(tag, |(_, name)| name)
    }

    /// The names the READ-ONLY sheet shows this same detail under, so the
    /// panel can drop that row when it shows the editable one: exiftool's for
    /// a picture ([`crate::facts::from_exif`] lists every tag by its own
    /// name), pdfinfo's for a document ([`crate::facts::from_pdfinfo`]).
    ///
    /// When it was TAKEN is promoted to a phrase of ours on a picture
    /// (`files-fact-taken`), which the caller compares by key instead.
    #[must_use]
    pub const fn also_shown_as(self, holder: Holder) -> &'static [&'static str] {
        match (holder, self) {
            (Holder::Picture, Self::Title) => &["Title"],
            (Holder::Picture, Self::Author) => &["Artist"],
            (Holder::Picture, Self::Description) => &["ImageDescription"],
            (Holder::Picture, Self::Keywords) => &["Subject"],
            (Holder::Picture, Self::Copyright) => &["Copyright"],
            (Holder::Picture, Self::Taken) => &[],
            (Holder::Document, Self::Title) => &["Title"],
            (Holder::Document, Self::Author) => &["Author"],
            (Holder::Document, Self::Description) => &["Subject"],
            (Holder::Document, Self::Keywords) => &["Keywords"],
            (Holder::Document, Self::Copyright) => &["Rights"],
            (Holder::Document, Self::Taken) => &["CreationDate"],
        }
    }

    /// A value as it will be written, or why it cannot be.
    ///
    /// One line, trimmed. A date is typed the way the panel shows it —
    /// `2026-09-30 14:05`, seconds optional, or exiftool's own `2026:09:30
    /// 14:05:00` — and written in exiftool's shape. An empty answer is
    /// `Ok("")`, which [`set`] reads as "take it out".
    ///
    /// # A zone, and only where the tag holds one
    ///
    /// A PDF's `CreationDate` carries its offset (`+02:00`, or `Z`), and
    /// exiftool reads it back that way — so the panel offers it back that way,
    /// and it is written back WITH it: dropping it would move the moment by
    /// hours. EXIF's `DateTimeOriginal` has no room for one (the offset is a
    /// tag of its own), and exiftool drops a typed one without a word, so on
    /// a picture a zone is refused rather than silently lost.
    ///
    /// # Errors
    ///
    /// [`Refused::TooLong`] past [`VALUE_MOST_BYTES`], [`Refused::BadDate`] for
    /// a date that is not one.
    pub fn written(self, holder: Holder, raw: &str) -> Result<String, Refused> {
        if raw.len() > VALUE_MOST_BYTES {
            return Err(Refused::TooLong);
        }
        let line: String = raw
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let line = line.trim();
        if line.is_empty() || self != Self::Taken {
            return Ok(line.to_owned());
        }
        match exif_date(line).ok_or(Refused::BadDate)? {
            (date, None) => Ok(date),
            (date, Some(zone)) if holder == Holder::Document => Ok(format!("{date}{zone}")),
            (_, Some(_)) => Err(Refused::BadDate),
        }
    }
}

/// `YYYY-MM-DD HH:MM[:SS]` (or with `:` in the date, exiftool's own) as
/// `YYYY:MM:DD HH:MM:SS`, and the zone that followed it (`+02:00`, `-0300`,
/// `Z`) apart, as `+02:00` or `Z`. Checked against the calendar: a day the
/// month does not have is not a date.
fn exif_date(text: &str) -> Option<(String, Option<String>)> {
    let (date, time) = text.split_once([' ', 'T']).unwrap_or((text, "00:00:00"));
    let (time, zone) = zone_of(time.trim())?;
    let mut d = date.split(['-', ':']);
    let year: i64 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=9999).contains(&year) {
        return None;
    }
    // THE CALENDAR'S OWN INVERSE, the manager's: `2026-02-30` is refused
    // rather than rolled into March.
    let day_is_there = ronoctua_files::when::instant_of(
        &format!("{year:04}-{month:02}-{day:02}"),
        &ronoctua_files::when::Zone::utc(),
    )
    .is_some();
    if !day_is_there {
        return None;
    }
    let mut t = time.trim().split(':');
    let hour: u32 = t.next()?.parse().ok()?;
    let minute: u32 = t.next()?.parse().ok()?;
    let second: u32 = t.next().map_or(Some(0), |s| s.parse().ok())?;
    if t.next().is_some() || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some((
        format!("{year:04}:{month:02}:{day:02} {hour:02}:{minute:02}:{second:02}"),
        zone,
    ))
}

/// A time of day and the zone written after it, apart: `10:00:00+02:00` is
/// `("10:00:00", Some("+02:00"))`, `10:00Z` is `("10:00", Some("Z"))`, and
/// a time with nothing after it has no zone. `None` for a zone that is not
/// one (`+25:00`, `+2`).
fn zone_of(time: &str) -> Option<(&str, Option<String>)> {
    if let Some(bare) = time.strip_suffix(['Z', 'z']) {
        return Some((bare.trim_end(), Some("Z".to_owned())));
    }
    let Some(at) = time.rfind(['+', '-']) else {
        return Some((time, None));
    };
    let (bare, offset) = time.split_at(at);
    let sign = &offset[..1];
    let digits: String = offset[1..].chars().filter(|c| *c != ':').collect();
    if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: u32 = digits[..2].parse().ok()?;
    let minutes: u32 = digits[2..].parse().ok()?;
    if hours > 14 || minutes > 59 {
        return None;
    }
    Some((
        bare.trim_end(),
        Some(format!("{sign}{hours:02}:{minutes:02}")),
    ))
}

/// Why a change was not made. The file is as it was after every one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Longer than [`VALUE_MOST_BYTES`].
    TooLong,
    /// Not a date.
    BadDate,
    /// exiftool ran and wrote nothing: a format it cannot write, a file it
    /// cannot open, a read-only folder.
    NotWritten,
    /// A copy was asked for and the name is taken.
    Taken,
    /// exiftool could not be run — not installed, or too slow.
    Program(Refusal),
}

impl From<Refusal> for Refused {
    fn from(why: Refusal) -> Self {
        // A PROGRAM THAT RAN AND FAILED wrote nothing: that is the one
        // sentence a person can act on ("it cannot be written here"), where
        // `Failed`'s own says the file may be damaged — which it is not.
        match why {
            Refusal::Failed => Self::NotWritten,
            other => Self::Program(other),
        }
    }
}

/// The six details as the file holds them now, blanks included: a blank is a
/// row that can be filled, not an absent one.
///
/// # Errors
///
/// When exiftool cannot be run or answers nothing readable.
pub fn read(path: &Path, holder: Holder) -> Result<Vec<(Meta, String)>, Refusal> {
    let mut args: Vec<OsString> = vec![
        "-json".into(),
        "-n".into(),
        // A LIST (keywords) as one line, in the same shape a person types it
        // back in.
        "-sep".into(),
        ", ".into(),
    ];
    args.extend(
        Meta::ALL
            .iter()
            .map(|m| OsString::from(format!("-{}", m.tag(holder)))),
    );
    args.push("--".into());
    args.push(path.as_os_str().to_owned());
    let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let out = tools::run(Tool::Exiftool, &borrowed)?;
    let doc: serde_json::Value = serde_json::from_slice(&out).map_err(|_| Refusal::Failed)?;
    let one = doc
        .as_array()
        .and_then(|a| a.first())
        .ok_or(Refusal::Failed)?;
    Ok(Meta::ALL
        .iter()
        .map(|m| {
            let value = match one.get(m.answered_as(holder)) {
                Some(serde_json::Value::String(s)) => s.trim().to_owned(),
                Some(serde_json::Value::Number(n)) => n.to_string(),
                Some(serde_json::Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(s.trim().to_owned()),
                        serde_json::Value::Number(n) => Some(n.to_string()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            (*m, shown_date(*m, value))
        })
        .collect())
}

/// A date as the panel shows it and the question offers it back: the
/// DATE's colons turned into dashes (`2026-09-30 14:05:00`), which is the
/// shape [`Meta::written`] reads.
fn shown_date(meta: Meta, value: String) -> String {
    if meta != Meta::Taken || value.len() < 10 {
        return value;
    }
    let (date, rest) = value.split_at(10);
    if date.bytes().filter(|b| *b == b':').count() == 2 {
        format!("{}{rest}", date.replace(':', "-"))
    } else {
        value
    }
}

/// Writes one detail into the file, in place. An empty `value` takes the tag
/// out.
///
/// # Errors
///
/// Every [`Refused`]; the file is as it was after each.
pub fn set(path: &Path, holder: Holder, meta: Meta, value: &str) -> Result<(), Refused> {
    let value = meta.written(holder, value)?;
    let mut args: Vec<OsString> = vec!["-overwrite_original_in_place".into()];
    if meta == Meta::Keywords {
        // A LIST, split where a person separated it — and only for the tag
        // that is one.
        args.push("-sep".into());
        args.push(", ".into());
    }
    args.push(format!("-{}={value}", meta.tag(holder)).into());
    written(path, args)
}

/// Where [`remove_location`] writes: over the file, or into a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Where {
    /// Over the file itself. It cannot be undone.
    InPlace,
    /// Into a new file at this path, which must not exist.
    Copy(PathBuf),
}

/// Takes the coordinate out of a file — every place a phone or a camera puts
/// one: EXIF's GPS block, XMP's `GPS*`, and the three QuickTime spellings a
/// film carries.
///
/// Returns the file that no longer says where it was made: the same one, or
/// the copy.
///
/// # Errors
///
/// [`Refused::Taken`] when the copy's name exists; the rest of [`Refused`].
///
/// # The copy is made first, and then stripped in place
///
/// Not exiftool's `-o`: that expands `%f`, `%d`, `%e` and `%c` in the name it
/// is given, so a file called `sun%fset.jpg` had its copy land under a name
/// nobody asked for — and not at the path this returned and the sentence
/// named. `%%` is no escape either (the pinned exiftool turned
/// `sun%%fset` into `sun%sun%fsetset`). So the copy is written here, by name,
/// with `create_new` — which is also the check that the name is free, with no
/// moment between the check and the write — and then treated exactly like
/// [`Where::InPlace`]. A write that fails takes its half-made copy with it.
pub fn remove_location(path: &Path, at: &Where) -> Result<PathBuf, Refused> {
    let mut args: Vec<OsString> = LOCATION_TAGS.iter().map(OsString::from).collect();
    args.push("-overwrite_original_in_place".into());
    match at {
        Where::InPlace => {
            written(path, args)?;
            Ok(path.to_path_buf())
        }
        Where::Copy(to) => {
            copy_new(path, to)?;
            match written(to, args) {
                Ok(()) => Ok(to.clone()),
                Err(why) => {
                    let _ = std::fs::remove_file(to);
                    Err(why)
                }
            }
        }
    }
}

/// `from`'s bytes and permission bits into a NEW file at `to`.
///
/// # Errors
///
/// [`Refused::Taken`] when `to` exists — a copy never overwrites —, and
/// [`Refused::NotWritten`] when the copy cannot be made (a read-only folder, a
/// full disk); a half-written copy is removed.
fn copy_new(from: &Path, to: &Path) -> Result<(), Refused> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    let mut source = std::fs::File::open(from).map_err(|_| Refused::NotWritten)?;
    let mode = source
        .metadata()
        .map_or(0o644, |m| m.permissions().mode() & 0o7777);
    let mut target = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(to)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(Refused::Taken),
        Err(_) => return Err(Refused::NotWritten),
    };
    if std::io::copy(&mut source, &mut target).is_err() || target.sync_all().is_err() {
        drop(target);
        let _ = std::fs::remove_file(to);
        return Err(Refused::NotWritten);
    }
    Ok(())
}

/// Every tag a coordinate lives in, as exiftool deletes it.
pub const LOCATION_TAGS: [&str; 5] = [
    "-gps:all=",
    "-xmp:GPS*=",
    "-Keys:GPSCoordinates=",
    "-UserData:GPSCoordinates=",
    "-ItemList:GPSCoordinates=",
];

/// Where a copy without the place goes, beside the file: `<stem> (no
/// location).<ext>`, numbered when that is taken.
///
/// A name on the person's disk, in the same English the rest of what this
/// system writes to disk is in.
#[must_use]
pub fn copy_beside(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let named = |n: u32| {
        if n == 1 {
            path.with_file_name(format!("{stem} (no location){extension}"))
        } else {
            path.with_file_name(format!("{stem} (no location {n}){extension}"))
        }
    };
    (1..10_000)
        .map(named)
        .find(|p| p.symlink_metadata().is_err())
        .unwrap_or_else(|| named(std::process::id()))
}

/// Runs one write and says whether it happened: exiftool answers `0 image
/// files updated` with exit status 0 when it changed nothing — a tag this
/// format has no place for — so the COUNT is read, not only the status.
fn written(path: &Path, mut args: Vec<OsString>) -> Result<(), Refused> {
    args.push("--".into());
    args.push(path.as_os_str().to_owned());
    let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let out = tools::run(Tool::Exiftool, &borrowed)?;
    let said = String::from_utf8_lossy(&out);
    let changed = said.lines().any(|l| {
        let l = l.trim();
        (l.ends_with("files updated") || l.ends_with("files created")) && !l.starts_with("0 ")
    });
    // "UNCHANGED" IS A WRITE THAT HAD NOTHING TO DO — taking the place out of
    // a file that carries none — and it is not a refusal.
    let unchanged = said.lines().any(|l| l.trim().ends_with("unchanged"));
    if changed || unchanged {
        Ok(())
    } else {
        Err(Refused::NotWritten)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pinned exiftool, when the environment names it (the tests
    /// derivation does); `None` makes a test say so and stop.
    fn exiftool() -> bool {
        Tool::Exiftool.there()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-amend-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    /// A real JPEG, 2×2, made by the decoder crate this one already links —
    /// exiftool refuses bytes that are not a picture, which is a test of its
    /// own below.
    fn a_jpeg(path: &Path) {
        image::RgbImage::from_pixel(2, 2, image::Rgb([200, 30, 30]))
            .save_with_format(path, image::ImageFormat::Jpeg)
            .expect("a JPEG is written");
    }

    /// The smallest PDF with a valid cross-reference table, which exiftool
    /// needs before it will add to one.
    fn a_pdf(path: &Path) {
        let objects: [&[u8]; 3] = [
            b"<</Type/Catalog/Pages 2 0 R>>",
            b"<</Type/Pages/Kids[3 0 R]/Count 1>>",
            b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 10 10]>>",
        ];
        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(o);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for o in offsets {
            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        std::fs::write(path, out).expect("a PDF is written");
    }

    /// A photograph that says where it was taken, the way a phone writes it.
    fn a_photo_with_a_place(dir: &Path) -> PathBuf {
        let photo = dir.join("praia.jpg");
        a_jpeg(&photo);
        let args: Vec<&OsStr> = [
            "-overwrite_original",
            "-GPSLatitude=51.5",
            "-GPSLatitudeRef=N",
            "-GPSLongitude=0.1",
            "-GPSLongitudeRef=W",
            "-XMP:GPSLatitude=51.5",
            "--",
        ]
        .iter()
        .map(OsStr::new)
        .chain(std::iter::once(photo.as_os_str()))
        .collect();
        tools::run(Tool::Exiftool, &args).expect("the fixture takes a place");
        photo
    }

    fn place_of(path: &Path) -> Option<(f32, f32)> {
        let args = crate::facts::exif_args(path);
        let borrowed: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        let out = tools::run(Tool::Exiftool, &borrowed).ok()?;
        crate::facts::coordinate(&crate::facts::from_exif(&String::from_utf8_lossy(&out)))
    }

    #[test]
    fn the_holder_is_decided_by_type_and_a_gif_has_none() {
        assert_eq!(Holder::of("image/jpeg"), Some(Holder::Picture));
        assert_eq!(Holder::of("application/pdf"), Some(Holder::Document));
        assert_eq!(Holder::of("image/gif"), None);
        assert_eq!(Holder::of("audio/mpeg"), None);
    }

    #[test]
    fn a_value_is_one_line_and_a_date_is_a_date() {
        assert_eq!(
            Meta::Title.written(Holder::Picture, "  a\nb  "),
            Ok("a b".to_owned())
        );
        assert_eq!(Meta::Title.written(Holder::Picture, ""), Ok(String::new()));
        assert_eq!(
            Meta::Taken.written(Holder::Picture, "2026-09-30 14:05"),
            Ok("2026:09:30 14:05:00".to_owned())
        );
        assert_eq!(
            Meta::Taken.written(Holder::Picture, "2026:09:30 14:05:09"),
            Ok("2026:09:30 14:05:09".to_owned())
        );
        assert_eq!(
            Meta::Taken.written(Holder::Picture, "2026-02-30 10:00"),
            Err(Refused::BadDate)
        );
        assert_eq!(
            Meta::Taken.written(Holder::Picture, "tomorrow"),
            Err(Refused::BadDate)
        );
        assert_eq!(
            Meta::Title.written(Holder::Picture, &"x".repeat(VALUE_MOST_BYTES + 1)),
            Err(Refused::TooLong)
        );
        // THE SHAPE SHOWN IS THE SHAPE READ: a date that comes back from the
        // file is offered back in a form `written` accepts.
        let shown = shown_date(Meta::Taken, "2020:01:02 03:04:05".to_owned());
        assert_eq!(shown, "2020-01-02 03:04:05");
        assert!(Meta::Taken.written(Holder::Picture, &shown).is_ok());
    }

    #[test]
    fn a_documents_date_keeps_its_zone_and_a_pictures_refuses_one() {
        // THE SHAPE exiftool 13 answers a PDF's CreationDate in: with its
        // offset. Offered back as it is shown, it is accepted, and written
        // back with the offset — not an hour moved.
        let shown = shown_date(Meta::Taken, "2023:05:01 10:00:00+02:00".to_owned());
        assert_eq!(shown, "2023-05-01 10:00:00+02:00");
        assert_eq!(
            Meta::Taken.written(Holder::Document, &shown),
            Ok("2023:05:01 10:00:00+02:00".to_owned())
        );
        assert_eq!(
            Meta::Taken.written(Holder::Document, "2023-05-01 10:00Z"),
            Ok("2023:05:01 10:00:00Z".to_owned())
        );
        assert_eq!(
            Meta::Taken.written(Holder::Document, "2023-05-01 10:00-0300"),
            Ok("2023:05:01 10:00:00-03:00".to_owned())
        );
        assert_eq!(
            Meta::Taken.written(Holder::Document, "2023-05-01 10:00"),
            Ok("2023:05:01 10:00:00".to_owned())
        );
        assert_eq!(
            Meta::Taken.written(Holder::Document, "2023-05-01 10:00+25:00"),
            Err(Refused::BadDate)
        );
        // EXIF's field has no room for one, and exiftool drops it in silence.
        assert_eq!(
            Meta::Taken.written(Holder::Picture, &shown),
            Err(Refused::BadDate)
        );
    }

    #[test]
    fn a_copy_beside_never_takes_a_name_that_is_there() {
        let dir = scratch("beside");
        let photo = dir.join("praia.jpg");
        assert_eq!(copy_beside(&photo), dir.join("praia (no location).jpg"));
        std::fs::write(dir.join("praia (no location).jpg"), b"x").unwrap();
        assert_eq!(copy_beside(&photo), dir.join("praia (no location 2).jpg"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_detail_written_is_the_detail_read_and_a_blank_takes_it_out() {
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("details");
        let photo = dir.join("retrato.jpg");
        a_jpeg(&photo);
        for meta in Meta::ALL {
            let value = match meta {
                Meta::Taken => "2020-01-02 03:04:05",
                Meta::Keywords => "mar, areia",
                _ => "Ana Silva",
            };
            set(&photo, Holder::Picture, meta, value).expect("written");
        }
        let read = read(&photo, Holder::Picture).expect("read back");
        assert_eq!(read.len(), Meta::ALL.len());
        for (meta, value) in &read {
            let wanted = match meta {
                Meta::Taken => "2020-01-02 03:04:05",
                Meta::Keywords => "mar, areia",
                _ => "Ana Silva",
            };
            assert_eq!(value, wanted, "{meta:?}");
        }
        set(&photo, Holder::Picture, Meta::Title, "").expect("taken out");
        let read = super::read(&photo, Holder::Picture).unwrap();
        assert_eq!(
            read[0],
            (Meta::Title, String::new()),
            "a blank is a row, not a gap"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_place_leaves_the_copy_and_stays_in_the_original() {
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("copy");
        let photo = a_photo_with_a_place(&dir);
        assert!(
            place_of(&photo).is_some(),
            "the fixture says where it was taken"
        );
        let copy = copy_beside(&photo);
        let landed = remove_location(&photo, &Where::Copy(copy.clone())).expect("a copy");
        assert_eq!(landed, copy);
        assert_eq!(place_of(&copy), None, "the copy says nowhere");
        assert!(place_of(&photo).is_some(), "the original is untouched");
        // AND A COPY NEVER OVERWRITES.
        assert_eq!(
            remove_location(&photo, &Where::Copy(copy)),
            Err(Refused::Taken)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_place_leaves_the_file_in_place_and_keeps_its_inode() {
        use std::os::unix::fs::MetadataExt as _;
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("in-place");
        let photo = a_photo_with_a_place(&dir);
        let before = std::fs::metadata(&photo).unwrap().ino();
        assert_eq!(remove_location(&photo, &Where::InPlace), Ok(photo.clone()));
        assert_eq!(place_of(&photo), None);
        assert_eq!(std::fs::metadata(&photo).unwrap().ino(), before);
        // A SECOND TIME there is nothing to take, and that is not a refusal.
        assert_eq!(remove_location(&photo, &Where::InPlace), Ok(photo));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_document_keeps_its_details_in_its_own_dictionary() {
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("document");
        let pdf = dir.join("relatorio.pdf");
        a_pdf(&pdf);
        set(&pdf, Holder::Document, Meta::Author, "Ana").expect("written");
        set(&pdf, Holder::Document, Meta::Keywords, "um, dois").expect("written");
        let read = read(&pdf, Holder::Document).expect("read back");
        assert!(read.contains(&(Meta::Author, "Ana".to_owned())), "{read:?}");
        assert!(
            read.contains(&(Meta::Keywords, "um, dois".to_owned())),
            "{read:?}"
        );
        assert!(read.contains(&(Meta::Title, String::new())), "{read:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_documents_zoned_date_goes_read_prefilled_and_written() {
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("zoned");
        let pdf = dir.join("relatorio.pdf");
        a_pdf(&pdf);
        // A ZONED CreationDate, the way most PDFs carry one, put there by
        // exiftool itself rather than by this module.
        let args: Vec<&OsStr> = [
            "-overwrite_original",
            "-PDF:CreateDate=2023:05:01 10:00:00+02:00",
            "--",
        ]
        .iter()
        .map(OsStr::new)
        .chain(std::iter::once(pdf.as_os_str()))
        .collect();
        tools::run(Tool::Exiftool, &args).expect("the fixture takes a date");
        let taken = |pdf: &Path| {
            read(pdf, Holder::Document)
                .expect("read")
                .into_iter()
                .find(|(m, _)| *m == Meta::Taken)
                .map(|(_, v)| v)
                .unwrap_or_default()
        };
        let prefilled = taken(&pdf);
        assert_eq!(prefilled, "2023-05-01 10:00:00+02:00");
        // ACCEPTING THE PREFILL writes the same moment back.
        set(&pdf, Holder::Document, Meta::Taken, &prefilled).expect("the prefill is a date");
        assert_eq!(taken(&pdf), prefilled);
        set(
            &pdf,
            Holder::Document,
            Meta::Taken,
            "2024-12-31 23:59:00-03:00",
        )
        .expect("written");
        assert_eq!(taken(&pdf), "2024-12-31 23:59:00-03:00");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_lands_at_the_name_given_even_with_a_percent_in_it() {
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("percent");
        let photo = a_photo_with_a_place(&dir);
        let odd = dir.join("sun%fset.jpg");
        std::fs::rename(&photo, &odd).unwrap();
        let copy = copy_beside(&odd);
        assert_eq!(copy, dir.join("sun%fset (no location).jpg"));
        let landed = remove_location(&odd, &Where::Copy(copy.clone())).expect("a copy");
        assert_eq!(landed, copy);
        assert!(copy.exists(), "the copy is where it was said to be");
        assert_eq!(place_of(&copy), None, "and it says nowhere");
        assert!(place_of(&odd).is_some(), "the original is untouched");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 2, "nothing landed anywhere else: {names:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copy_that_cannot_be_stripped_is_not_left_behind() {
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("half");
        let text = dir.join("notes.jpg");
        std::fs::write(&text, b"not a picture at all").unwrap();
        let copy = copy_beside(&text);
        assert_eq!(
            remove_location(&text, &Where::Copy(copy.clone())),
            Err(Refused::NotWritten)
        );
        assert!(!copy.exists(), "the half-made copy went with the refusal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_exiftool_cannot_write_is_refused_and_left_alone() {
        if !exiftool() {
            eprintln!("RONOCTUA_EXIFTOOL is not set; this test proves nothing here");
            return;
        }
        let dir = scratch("unwritable");
        let text = dir.join("notes.jpg");
        std::fs::write(&text, b"not a picture at all").unwrap();
        assert_eq!(
            set(&text, Holder::Picture, Meta::Title, "x"),
            Err(Refused::NotWritten)
        );
        assert_eq!(std::fs::read(&text).unwrap(), b"not a picture at all");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
