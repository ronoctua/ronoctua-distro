//! A whole sheet out of every reader there is.
//!
//! # Every reader, and each one optional
//!
//! lofty reads the tags of the containers it knows, ffprobe the streams,
//! chapters and technical facts of every container, exiftool everything as
//! a read-only dump, and `mkvextract`/`mkvmerge` Matroska's own tags. A
//! missing program makes a sheet with fewer rows and an [`Unread`] that
//! says which and why — never an error for the whole sheet. What IS an
//! error is a file that is not there or not a file.
//!
//! # Blocking
//!
//! Up to four programs run one after the other, a few hundred milliseconds
//! in all on a local disk. The engine calls this on its own worker thread.

use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::matroska;
use crate::native::{self, LOFTY_MOST_BYTES, Opened};
use crate::picture::{self, ImageKind};
use crate::probe::{self, Probed};
use crate::run::{Limits, Tools, run};
use crate::sheet::{
    Adding, Can, Container, Cover, CoverAt, CoverRole, Field, FieldRow, Kind, Origin, Program,
    Sheet, Source, Stamp, StreamKind, Tag, Unread, Why, Writer,
};

/// The file, canonical, and what `stat` says about it.
///
/// # Errors
///
/// [`Why::Gone`] when it is not there, [`Why::NotAFile`] when it is not a
/// regular file.
pub(crate) fn located(path: &Path) -> Result<(PathBuf, std::fs::Metadata), Why> {
    let location = std::fs::canonicalize(path).map_err(|_| Why::Gone)?;
    let meta = std::fs::metadata(&location).map_err(|_| Why::Gone)?;
    if !meta.is_file() {
        return Err(Why::NotAFile);
    }
    Ok((location, meta))
}

/// ffprobe's answer about a file.
pub(crate) fn probed(location: &Path, tools: &Tools) -> Result<Probed, Why> {
    let out = run(
        tools,
        Program::Ffprobe,
        &probe::probe_args(location),
        Limits::READ,
        None,
        None,
    )?
    .ok(Program::Ffprobe, &[0])?;
    probe::parse(&out)
}

/// Which container a file is, from lofty's guess, then ffprobe's, then its
/// name.
pub(crate) fn container(
    location: &Path,
    lofty: Option<lofty::file::FileType>,
    probed: Option<&Probed>,
) -> Container {
    if let Some(ft) = lofty {
        let c = native::container_of(ft);
        if c != Container::Other {
            return c;
        }
    }
    let extension = location
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if let Some(name) = probed.and_then(|p| p.format_name.as_deref()) {
        let has = |n: &str| name.split(',').any(|part| part == n);
        return if has("matroska") || has("webm") {
            if extension == "webm" {
                Container::WebM
            } else {
                Container::Matroska
            }
        } else if has("avi") {
            Container::Avi
        } else if has("mpegts") {
            Container::MpegTs
        } else if has("flv") {
            Container::Flv
        } else if has("asf") {
            Container::Asf
        } else if has("ogg") {
            Container::OggOther
        } else if has("mov") || has("mp4") {
            // lofty did not read it; the sheet will say it cannot.
            Container::Mp4
        } else if has("mp3") {
            Container::Mpeg
        } else if has("flac") {
            Container::Flac
        } else if has("wav") {
            Container::Wav
        } else if has("aiff") {
            Container::Aiff
        } else if has("ape") {
            Container::Ape
        } else if has("wv") {
            Container::WavPack
        } else if has("aac") {
            Container::Aac
        } else if has("mpc") || has("mpc8") {
            Container::Mpc
        } else {
            Container::Other
        };
    }
    match extension.as_str() {
        "mkv" | "mka" | "mks" | "mk3d" => Container::Matroska,
        "webm" => Container::WebM,
        "avi" => Container::Avi,
        "ts" | "m2ts" | "mts" => Container::MpegTs,
        "flv" => Container::Flv,
        "wmv" | "wma" | "asf" => Container::Asf,
        "ogv" | "ogg" | "oga" => Container::OggOther,
        _ => Container::Other,
    }
}

/// Music or video, when ffprobe could not be asked.
fn kind_by_name(location: &Path, container: Container) -> Kind {
    let extension = location
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let film_name = matches!(
        extension.as_str(),
        "mp4" | "m4v" | "mov" | "3gp" | "mkv" | "webm"
    );
    if (container.usually_video() && extension != "mka") || film_name {
        Kind::Video
    } else {
        Kind::Music
    }
}

/// Matroska's own view: tags and identification.
struct MatroskaView {
    tags: Result<matroska::Tags, Why>,
    identified: Option<matroska::Identified>,
}

/// `mkvextract`'s tags of a file.
pub(crate) fn matroska_tags(location: &Path, tools: &Tools) -> Result<matroska::Tags, Why> {
    tools.path(Program::Mkvextract)?;
    let scratch = crate::run::Scratch::new()?;
    let xml = scratch.path("tags.xml");
    let mut args: Vec<OsString> = vec![location.as_os_str().to_owned(), OsString::from("tags")];
    args.push(xml.as_os_str().to_owned());
    run(tools, Program::Mkvextract, &args, Limits::READ, None, None)?
        .ok(Program::Mkvextract, &[0, 1])?;
    // No tags, no file: `mkvextract` writes nothing for a file with none.
    let text = match std::fs::read(&xml) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| Why::Unreadable)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err(Why::Unreadable),
    };
    matroska::parse_tags(&text)
}

fn matroska_view(location: &Path, tools: &Tools) -> MatroskaView {
    let identified = run(
        tools,
        Program::Mkvmerge,
        &[OsString::from("-J"), location.as_os_str().to_owned()],
        Limits::READ,
        None,
        None,
    )
    .and_then(|o| o.ok(Program::Mkvmerge, &[0, 1]))
    .and_then(|bytes| matroska::parse_identified(&bytes))
    .ok();
    MatroskaView {
        tags: matroska_tags(location, tools),
        identified,
    }
}

/// The keys ffmpeg gives each field, for the containers only ffprobe reads
/// — the schema's table ([`Field::ffmpeg_keys`]), which a stream's sheet
/// reads mpv's words by too.
pub(crate) const fn ffmpeg_keys(field: Field) -> &'static [&'static str] {
    field.ffmpeg_keys()
}

/// A field out of ffprobe's tags: the container's, then — for Ogg, where
/// comments belong to a stream — the first stream's.
fn ffprobe_value(p: &Probed, field: Field) -> Option<String> {
    ffmpeg_keys(field).iter().find_map(|k| {
        probe::tag_in(&p.tags, k)
            .or_else(|| p.streams.iter().find_map(|s| s.tag(k)))
            .map(ToOwned::to_owned)
    })
}

/// Whether a writer can run here, and why not when it cannot.
pub(crate) fn writer_can(writer: Result<Writer, Why>, tools: &Tools) -> Can {
    match writer {
        Err(why) => Can::No(why),
        Ok(w) => match w.programs().iter().find(|p| !tools.has(**p)) {
            Some(program) => Can::No(Why::Missing { program: *program }),
            None => Can::Yes(w),
        },
    }
}

/// Reads everything a file says about itself.
///
/// # Errors
///
/// [`Why::Gone`] and [`Why::NotAFile`]. Every other failure is an
/// [`Unread`] on the sheet.
pub fn sheet(path: &Path, tools: &Tools) -> Result<Sheet, Why> {
    let (location, meta) = located(path)?;
    let stamp = Stamp::of(&meta);
    let mut unread = Vec::new();

    let probed = match probed(&location, tools) {
        Ok(p) => Some(p),
        Err(why) => {
            unread.push(Unread {
                source: Source::Ffprobe,
                why,
            });
            None
        }
    };
    let file_type = native::trusted(native::guess(&location), probed.as_ref());
    let container = container(&location, file_type, probed.as_ref());
    let kind = probed
        .as_ref()
        .and_then(probe::kind_of)
        .unwrap_or_else(|| kind_by_name(&location, container));

    let opened: Option<Result<Opened, Why>> = container.lofty().then(|| {
        let ft = file_type.ok_or(Why::Unreadable)?;
        let mut file = File::open(&location).map_err(|_| Why::Gone)?;
        native::open(&mut file, ft)
    });
    if let Some(Err(why)) = &opened {
        unread.push(Unread {
            source: Source::Lofty,
            why: why.clone(),
        });
    }
    let opened = opened.and_then(Result::ok);

    let mkv = container
        .matroska()
        .then(|| matroska_view(&location, tools));
    if let Some(MatroskaView { tags: Err(why), .. }) = &mkv {
        unread.push(Unread {
            source: Source::Matroska,
            why: why.clone(),
        });
    }

    let exif = run(
        tools,
        Program::Exiftool,
        &probe::exif_args(&location),
        Limits::READ,
        None,
        None,
    )
    .and_then(|o| o.ok(Program::Exiftool, &[0]))
    .and_then(|bytes| probe::parse_exif(&bytes));
    let exif = match exif {
        Ok(rows) => rows,
        Err(why) => {
            unread.push(Unread {
                source: Source::Exiftool,
                why,
            });
            Vec::new()
        }
    };

    // ---- who may write what -------------------------------------------
    let mut fields_can = writer_can(container.fields_writer(), tools);
    let mut cover_can = writer_can(container.cover_writer(), tools);
    if container.lofty() {
        let refusal = if opened.is_none() {
            Some(Why::Unreadable)
        } else if meta.len() > LOFTY_MOST_BYTES {
            Some(Why::TooBig {
                bytes: meta.len(),
                most_bytes: LOFTY_MOST_BYTES,
            })
        } else {
            None
        };
        if let Some(why) = refusal {
            fields_can = Can::No(why.clone());
            cover_can = Can::No(why);
        }
    }
    if container.matroska() {
        // A write re-reads the tags; ffprobe finds the covers and the tracks.
        if let Some(MatroskaView { tags: Err(why), .. }) = &mkv
            && fields_can.can()
        {
            fields_can = Can::No(why.clone());
        }
        if !tools.has(Program::Ffprobe) && cover_can.can() {
            cover_can = Can::No(Why::Missing {
                program: Program::Ffprobe,
            });
        }
    }
    let primary = opened
        .as_ref()
        .and_then(|o| native::format_of(o.file_type.primary_tag_type()));
    if let (Some(format), Can::Yes(_)) = (primary, &cover_can)
        && !matches!(
            format,
            crate::sheet::TagFormat::Id3v2
                | crate::sheet::TagFormat::Ape
                | crate::sheet::TagFormat::Vorbis
                | crate::sheet::TagFormat::Mp4
        )
    {
        cover_can = Can::No(Why::NoPlace);
    }

    // ---- fields ------------------------------------------------------------
    let generics: Vec<(&native::Native, lofty::tag::Tag)> = opened
        .as_ref()
        .map(|o| {
            let mut tags: Vec<&native::Native> = o.tags.iter().collect();
            // The main tag first: it is the one a field is written to, so it
            // is the one a field is read from.
            tags.sort_by_key(|t| Some(t.format()) != primary);
            tags.into_iter().map(|t| (t, t.generic())).collect()
        })
        .unwrap_or_default();
    let value_of = |field: Field| -> Option<String> {
        if let Some(v) = generics
            .iter()
            .find_map(|(n, g)| native::field_value(n, g, field))
        {
            return Some(v);
        }
        if let Some(MatroskaView {
            tags: Ok(tags),
            identified,
        }) = &mkv
        {
            if field == Field::Title
                && let Some(t) = identified.as_ref().and_then(|i| i.title.clone())
            {
                return Some(t);
            }
            if let Some(v) = matroska::field_value(tags, field) {
                return Some(v);
            }
        }
        probed.as_ref().and_then(|p| ffprobe_value(p, field))
    };
    let can_of = |field: Field| -> Can {
        match (&fields_can, primary) {
            (Can::Yes(Writer::Lofty), Some(format)) if !native::has_place(format, field) => {
                Can::No(Why::NoPlace)
            }
            (can, _) => can.clone(),
        }
    };
    let common = Field::common(kind);
    let mut fields = Vec::new();
    for field in Field::ALL {
        let value = value_of(field);
        if common.contains(&field) || value.is_some() {
            fields.push(FieldRow {
                field,
                value,
                can: can_of(field),
            });
        }
    }
    // The common rows in their own order, then the others.
    fields.sort_by_key(|row| {
        common
            .iter()
            .position(|f| *f == row.field)
            .unwrap_or(usize::MAX)
    });

    // ---- covers ------------------------------------------------------------
    let mut covers: Vec<Cover> = Vec::new();
    if let Some(o) = &opened {
        for tag in &o.tags {
            let writable = o.file_type.tag_support(tag.tag_type()).is_writable();
            let can = if writable {
                cover_can.clone()
            } else {
                Can::No(Why::NoPlace)
            };
            covers.extend(native::covers(tag, &can));
        }
    } else if let Some(p) = &probed {
        let sizes = mkv
            .as_ref()
            .and_then(|m| m.identified.as_ref())
            .map(|i| i.attachments.clone())
            .unwrap_or_default();
        for s in p.streams.iter().filter(|s| s.attached) {
            let name = s.tag("filename").map(ToOwned::to_owned);
            let front = name.as_deref().is_some_and(matroska::is_front_cover)
                || (!container.matroska()
                    && s.tag("comment")
                        .is_none_or(|c| c.eq_ignore_ascii_case("cover (front)")));
            let mime = s
                .tag("mimetype")
                .map(ToOwned::to_owned)
                .or_else(|| {
                    match s.codec.as_deref() {
                        Some("mjpeg") => Some(ImageKind::Jpeg),
                        Some("png") => Some(ImageKind::Png),
                        Some("bmp") => Some(ImageKind::Bmp),
                        Some("gif") => Some(ImageKind::Gif),
                        Some("webp") => Some(ImageKind::WebP),
                        _ => None,
                    }
                    .map(|k| k.mime().to_owned())
                })
                .unwrap_or_default();
            let bytes = name
                .as_ref()
                .and_then(|n| sizes.iter().find(|a| &a.file_name == n))
                .map(|a| a.size);
            covers.push(Cover {
                role: if front {
                    CoverRole::Front
                } else {
                    CoverRole::Other
                },
                mime,
                width: s.width,
                height: s.height,
                bytes,
                at: CoverAt::Stream {
                    index: s.index,
                    name,
                },
                // Only the FRONT cover is written, and only Matroska's is
                // written here; elsewhere the sheet's own refusal says why
                // (lofty could not read it, or nothing writes the container).
                can: if front || !container.matroska() {
                    cover_can.clone()
                } else {
                    Can::No(Why::NoPlace)
                },
            });
        }
    }

    // ---- the rest ----------------------------------------------------------
    let technical = probed.as_ref().map(probe::technical).unwrap_or_default();
    let streams_can = writer_can(container.streams_writer(), tools);
    let streams = probed
        .as_ref()
        .map(|p| {
            probe::streams(p, |s| {
                if s.kind == Some(StreamKind::Attachment) || s.attached {
                    Can::No(Why::NoPlace)
                } else {
                    streams_can.clone()
                }
            })
        })
        .unwrap_or_default();
    let chapters = probed
        .as_ref()
        .map(|p| p.chapters.clone())
        .unwrap_or_default();

    let mut other: Vec<Tag> = Vec::new();
    if let Some(o) = &opened {
        let mut tags: Vec<&native::Native> = o.tags.iter().collect();
        tags.sort_by_key(|t| Some(t.format()) != primary);
        for t in tags {
            let writable = o.file_type.tag_support(t.tag_type()).is_writable();
            let mut rows = native::listing(t);
            if !writable || !fields_can.can() {
                for r in &mut rows {
                    if r.can.can() {
                        r.can = if writable {
                            fields_can.clone()
                        } else {
                            Can::No(Why::NoPlace)
                        };
                    }
                }
            }
            other.extend(rows);
        }
    }
    if let Some(MatroskaView { tags: Ok(tags), .. }) = &mkv {
        other.extend(matroska::listing(tags, &fields_can));
    }
    // FFPROBE'S AND EXIFTOOL'S ROWS say why they are not written, in the
    // file's own terms: where a remux writes the container, ffprobe's rows
    // ARE its tags and are written by name; where lofty or Matroska's tools
    // write it, they mirror a tag listed above and change through it
    // (`ReadOnlyView`); where nothing writes it, that is the reason.
    let mirrors = |can: &mut Can| {
        if let Err(why) = container.fields_writer() {
            *can = Can::No(why);
        }
    };
    if let Some(p) = &probed {
        let mut rows = probe::listing(p);
        for r in &mut rows {
            if container.fields_writer() == Ok(Writer::Remux) {
                r.can = match crate::sheet::tag_name(&r.origin, &r.name) {
                    Ok(_) => fields_can.clone(),
                    Err(why) => Can::No(why),
                };
            } else {
                mirrors(&mut r.can);
            }
        }
        other.extend(rows);
    }
    let mut exif = exif;
    for r in &mut exif {
        mirrors(&mut r.can);
    }
    other.extend(exif);

    // WHERE A TAG ADDED BY NAME GOES: the file's main tag, and whether the
    // file can take one at all.
    let adding = match container.fields_writer() {
        Ok(Writer::Lofty) => Adding {
            origin: primary.map(|format| Origin::Lofty { format }),
            can: if primary.is_some() {
                fields_can.clone()
            } else {
                Can::No(fields_can_or(&fields_can, Why::Unreadable))
            },
        },
        Ok(Writer::Matroska) => Adding {
            origin: Some(Origin::Matroska {
                target: crate::sheet::Target::Global,
            }),
            can: fields_can.clone(),
        },
        Ok(Writer::Remux) => Adding {
            origin: Some(Origin::Ffprobe { stream: None }),
            can: fields_can.clone(),
        },
        Err(why) => Adding {
            origin: None,
            can: Can::No(why),
        },
    };

    Ok(Sheet {
        location,
        stamp,
        kind,
        container,
        fields,
        covers,
        cover_can,
        technical,
        streams,
        chapters,
        other,
        unread,
        cover_file: None,
        adding: Some(adding),
    })
}

/// The file's own refusal when it has one, else `why`.
fn fields_can_or(can: &Can, why: Why) -> Why {
    match can {
        Can::No(own) => own.clone(),
        Can::Yes(_) => why,
    }
}

/// A cover taken out of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Its bytes, as they are inside the file.
    pub bytes: Vec<u8>,
    /// Its MIME type (`image/jpeg`).
    pub mime: String,
    /// The extension a file of it takes (`jpg`).
    pub extension: String,
}

/// The most bytes a cover stream may come out as.
const COVER_OUT_MOST: u64 = crate::sheet::COVER_MOST_BYTES + 1;

/// A cover's bytes, by where its sheet said it was — for the engine's
/// cache, for MPRIS's `artUrl`, and for [`crate::write::save_cover`].
///
/// # Errors
///
/// [`Why::NoSuchThing`] when it is not there any more, [`Why::NotAnImage`]
/// when the bytes are no image this crate knows, and whatever reading them
/// refuses with.
pub fn cover(path: &Path, at: &CoverAt, tools: &Tools) -> Result<Picture, Why> {
    let (location, _) = located(path)?;
    let (bytes, kind) = match at {
        CoverAt::Tag { format, index } => {
            let guess = native::guess(&location);
            let probed = if native::needs_confirming(guess) {
                probed(&location, tools).ok()
            } else {
                None
            };
            let ft = native::trusted(guess, probed.as_ref()).ok_or(Why::Unreadable)?;
            let mut file = File::open(&location).map_err(|_| Why::Gone)?;
            let opened = native::open(&mut file, ft)?;
            native::picture_bytes(&opened, *format, *index)?
        }
        CoverAt::Stream { index, .. } => {
            let args: Vec<OsString> = ["-nostdin", "-v", "error", "-i"]
                .iter()
                .map(OsString::from)
                .chain([location.as_os_str().to_owned()])
                .chain(
                    [
                        "-map",
                        &format!("0:{index}"),
                        "-c",
                        "copy",
                        "-frames:v",
                        "1",
                        "-f",
                        "image2pipe",
                        "pipe:1",
                    ]
                    .iter()
                    .map(OsString::from),
                )
                .collect();
            let limits = Limits {
                ceiling: COVER_OUT_MOST,
                ..Limits::READ
            };
            let bytes = run(tools, Program::Ffmpeg, &args, limits, None, None)?
                .ok(Program::Ffmpeg, &[0])?;
            if bytes.is_empty() {
                return Err(Why::NoSuchThing);
            }
            let kind = picture::sniff(&bytes);
            (bytes, kind)
        }
    };
    let kind = kind.ok_or(Why::NotAnImage)?;
    Ok(Picture {
        bytes,
        mime: kind.mime().to_owned(),
        extension: kind.extension().to_owned(),
    })
}
