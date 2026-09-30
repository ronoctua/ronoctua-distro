//! Writing: a change goes into a draft beside the file, and one `rename`
//! puts the draft where the file was.
//!
//! # The draft
//!
//! `.<name>.ronoctua-<pid>.part`, in the SAME folder — a `rename` is atomic
//! only inside one filesystem, and the folder is where the room is. It is
//! made by `ioctl(FICLONE)` where the filesystem clones (btrfs, XFS): no
//! byte is copied until the writer changes one. Elsewhere it is a copy, by
//! `copy_file_range`, after `statvfs` says there is room for it. A draft that
//! fails for any reason is removed on the way out; one left by a process that
//! was killed is swept the next time the same file is written.
//!
//! # What the rename buys
//!
//! mpv may be playing the file. It keeps reading the old inode, which the
//! rename unlinks but does not free until mpv lets go — so it plays on, and
//! the engine's `refresh` moves it to the new one when it is ready. A write
//! in place would have moved the sound under it.
//!
//! # Refusals before anything is written
//!
//! The stamp is checked twice: when the write starts ([`Why::ChangedSince`]
//! without touching anything) and again just before the rename (something
//! else wrote it meanwhile; the draft goes, the file stays). A file with
//! other hard links, a file somebody else owns, a file marked read-only and
//! a folder that cannot be written are all refused by name, because the
//! rename would quietly break or change each of them.
//!
//! Extended attributes are not carried to the new inode.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::matroska;
use crate::native::{self, CoverChange, LOFTY_MOST_BYTES, NativeEdit};
use crate::picture;
use crate::read::{self, located};
use crate::run::{Limits, Scratch, Tools, run};
use crate::sheet::{
    Container, CoverAt, CoverEdit, Edit, Field, Origin, Program, Stamp, StreamChange, StreamKind,
    Why, Writer,
};

/// Who is watching a write: a flag to stop it, and a place for progress.
#[derive(Default, Clone, Copy)]
pub struct Watch<'a> {
    /// Raised, the write stops at the next step and leaves the file as it
    /// was.
    pub stop: Option<&'a AtomicBool>,
    /// Told how far it is, in thousandths.
    pub progress: Option<&'a dyn Fn(u16)>,
}

impl Watch<'_> {
    fn stopped(&self) -> Result<(), Why> {
        if self.stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
            Err(Why::Cancelled)
        } else {
            Ok(())
        }
    }

    fn at(&self, permille: u16) {
        if let Some(f) = self.progress {
            f(permille.min(1000));
        }
    }
}

/// Writes a person's changes into a file.
///
/// Returns the file's new stamp, which is what the next write must name.
/// No change at all writes nothing and returns the stamp it was given.
///
/// One write per file at a time, per process: the draft's name is the
/// file's and the pid's, so a second write of the same file started while
/// the first runs would take the first one's draft away. The engine's one
/// retag thread is what keeps that true.
///
/// # Errors
///
/// Every [`Why`] a write can meet; the file is as it was after each.
pub fn apply(path: &Path, stamp: Stamp, edits: &[Edit], tools: &Tools) -> Result<Stamp, Why> {
    apply_watched(path, stamp, edits, tools, &Watch::default())
}

/// [`apply`], watched.
///
/// # Errors
///
/// As [`apply`], and [`Why::Cancelled`].
pub fn apply_watched(
    path: &Path,
    stamp: Stamp,
    edits: &[Edit],
    tools: &Tools,
    watch: &Watch<'_>,
) -> Result<Stamp, Why> {
    let (location, meta) = located(path)?;
    if Stamp::of(&meta) != stamp {
        return Err(Why::ChangedSince);
    }
    let folder = guard(&location, &meta)?;
    let asked = asked(edits)?;
    if asked.is_empty() {
        return Ok(stamp);
    }

    let guess = native::guess(&location);
    let probed = if native::needs_confirming(guess) {
        read::probed(&location, tools).ok()
    } else {
        None
    };
    let file_type = native::trusted(guess, probed.as_ref());
    let container = read::container(&location, file_type, probed.as_ref());
    let writer = writer_for(container, &asked)?;
    if let Some(program) = writer.programs().iter().find(|p| !tools.has(**p)) {
        return Err(Why::Missing { program: *program });
    }
    watch.stopped()?;
    watch.at(0);

    let draft = match writer {
        Writer::Lofty => {
            if meta.len() > LOFTY_MOST_BYTES {
                return Err(Why::TooBig {
                    bytes: meta.len(),
                    most_bytes: LOFTY_MOST_BYTES,
                });
            }
            let file_type = file_type.ok_or(Why::Unreadable)?;
            let plan = lofty_plan(&asked)?;
            let mut draft = Draft::copy_of(&location, &folder, meta.len(), watch)?;
            watch.stopped()?;
            let file = draft.file()?;
            let opened = native::open(file, file_type)?;
            native::apply(file, opened, &plan)?;
            draft
        }
        Writer::Matroska => {
            let edit_scratch = Scratch::new()?;
            let edit = matroska_edit(&location, &asked, tools, &edit_scratch)?;
            if edit.is_empty() {
                // Every value was already what was asked: nothing to write.
                return Ok(stamp);
            }
            let mut draft = Draft::copy_of(&location, &folder, meta.len(), watch)?;
            watch.stopped()?;
            let args = matroska::propedit_args(&draft.path, &edit);
            // The file is closed while another program edits it.
            draft.file = None;
            let limits = Limits {
                patience: patience_for(meta.len()),
                ceiling: Limits::READ.ceiling,
            };
            run(tools, Program::Mkvpropedit, &args, limits, watch.stop, None)?
                .ok(Program::Mkvpropedit, &[0, 1])?;
            draft
        }
        Writer::Remux => remux(
            &location,
            &folder,
            meta.len(),
            container,
            &asked,
            tools,
            watch,
            probed.as_ref(),
        )?,
    };
    finish(draft, &location, &folder, &meta, stamp, watch)
}

/// The refusals that need no reading: other names, another owner, a
/// read-only mark, a folder that cannot be written. Gives the folder.
fn guard(location: &Path, meta: &std::fs::Metadata) -> Result<PathBuf, Why> {
    if meta.nlink() > 1 {
        return Err(Why::HardLinked);
    }
    if meta.uid() != rustix::process::geteuid().as_raw() {
        return Err(Why::NotYours);
    }
    if meta.mode() & 0o200 == 0 {
        return Err(Why::ReadOnlyFile);
    }
    let folder = location.parent().ok_or(Why::NotAFile)?.to_path_buf();
    writable_folder(&folder)?;
    Ok(folder)
}

fn writable_folder(folder: &Path) -> Result<(), Why> {
    rustix::fs::access(
        folder,
        rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
    )
    .map_err(|_| Why::ReadOnlyFolder)
}

/// The edits, checked and folded: the last word on a field wins.
#[derive(Debug, Default)]
struct Asked {
    fields: Vec<(Field, Option<String>)>,
    cover: Option<CoverChange>,
    native: Vec<(Origin, String, Option<String>)>,
    streams: Vec<(u32, StreamChange)>,
}

impl Asked {
    fn is_empty(&self) -> bool {
        self.fields.is_empty()
            && self.cover.is_none()
            && self.native.is_empty()
            && self.streams.is_empty()
    }
}

fn asked(edits: &[Edit]) -> Result<Asked, Why> {
    fn field(out: &mut Asked, f: Field, v: Option<String>) {
        out.fields.retain(|(g, _)| *g != f);
        out.fields.push((f, v));
    }
    // The last word on a name wins too.
    fn native(out: &mut Asked, origin: &Origin, name: String, v: Option<String>) {
        out.native.retain(|(o, n, _)| !(o == origin && *n == name));
        out.native.push((origin.clone(), name, v));
    }
    let mut out = Asked::default();
    for edit in edits {
        match edit {
            Edit::Set { field: f, value } => {
                let v = f.check(value)?;
                field(&mut out, *f, (!v.is_empty()).then_some(v));
            }
            Edit::Clear { field: f } => field(&mut out, *f, None),
            // A NAME the tag's grammar takes, made its own (`tag_name`): a
            // row's comes back as it went, a new one becomes the format's.
            Edit::SetTag {
                origin,
                name,
                value,
            } => {
                let name = crate::sheet::tag_name(origin, name)?;
                let v = (!value.is_empty()).then(|| value.clone());
                if let Some(v) = &v
                    && v.len() > crate::sheet::VALUE_MOST_BYTES
                {
                    return Err(Why::TooBig {
                        bytes: v.len() as u64,
                        most_bytes: crate::sheet::VALUE_MOST_BYTES as u64,
                    });
                }
                native(&mut out, origin, name, v);
            }
            Edit::RemoveTag { origin, name } => {
                let name = crate::sheet::tag_name(origin, name)?;
                native(&mut out, origin, name, None);
            }
            Edit::Cover(CoverEdit::Replace { from }) => {
                let (bytes, kind) = picture::for_tags(from)?;
                out.cover = Some(CoverChange::Replace { bytes, kind });
            }
            Edit::Cover(CoverEdit::Remove) => out.cover = Some(CoverChange::Remove),
            Edit::Stream { index, change } => {
                if let StreamChange::Language(l) = change {
                    crate::sheet::language(l)?;
                }
                out.streams.push((*index, change.clone()));
            }
        }
    }
    Ok(out)
}

/// The one writer for everything asked, or why there is none.
fn writer_for(container: Container, asked: &Asked) -> Result<Writer, Why> {
    let mut writer = None;
    let mut need = |w: Result<Writer, Why>| -> Result<(), Why> {
        writer = Some(w?);
        Ok(())
    };
    if !asked.fields.is_empty() || !asked.native.is_empty() {
        need(container.fields_writer())?;
    }
    if asked.cover.is_some() {
        need(container.cover_writer())?;
    }
    if !asked.streams.is_empty() {
        need(container.streams_writer())?;
    }
    let writer = writer.ok_or(Why::NoPlace)?;
    for (origin, _, _) in &asked.native {
        match (writer, origin) {
            (Writer::Lofty, Origin::Lofty { .. })
            | (Writer::Matroska, Origin::Matroska { .. })
            | (Writer::Remux, Origin::Ffprobe { .. }) => {}
            // ffprobe's row of a file lofty or Matroska's tools write MIRRORS
            // their tag, which is where it changes.
            (Writer::Lofty | Writer::Matroska, Origin::Ffprobe { .. }) => {
                return Err(Why::ReadOnlyView);
            }
            _ => return Err(Why::NoPlace),
        }
    }
    Ok(writer)
}

fn lofty_plan(asked: &Asked) -> Result<native::Plan, Why> {
    let mut native_edits = Vec::new();
    for (origin, name, value) in &asked.native {
        let Origin::Lofty { format } = origin else {
            return Err(Why::NoPlace);
        };
        let edit = NativeEdit {
            format: *format,
            name: name.clone(),
            value: value.clone(),
        };
        native::check_native(&edit)?;
        native_edits.push(edit);
    }
    Ok(native::Plan {
        fields: asked.fields.clone(),
        cover: asked.cover.clone(),
        native: native_edits,
    })
}

/// One `mkvpropedit` run, planned; the tags file and the cover are written
/// into `scratch`, which must outlive the run.
fn matroska_edit(
    location: &Path,
    asked: &Asked,
    tools: &Tools,
    scratch: &Scratch,
) -> Result<matroska::Edit, Why> {
    let mut edit = matroska::Edit::default();
    let touches_tags =
        asked.fields.iter().any(|(f, _)| *f != Field::Title) || !asked.native.is_empty();
    let needs_title_tag = asked.fields.iter().any(|(f, _)| *f == Field::Title);
    if touches_tags || needs_title_tag {
        // THE TAGS FIRST, all of them: `--tags global:` replaces them.
        let mut tags = read::matroska_tags(location, tools)?;
        let before = tags.clone();
        let title_tag = matroska::has_title_tag(&tags);
        for (field, value) in &asked.fields {
            if *field == Field::Title {
                edit.title = Some(value.clone());
                if title_tag {
                    matroska::set_named(&mut tags, "TITLE", value.as_deref());
                }
            } else {
                matroska::set_field(&mut tags, *field, value.as_deref());
            }
        }
        for (_, name, value) in &asked.native {
            if name.is_empty() || name.chars().any(char::is_control) {
                return Err(Why::NoPlace);
            }
            matroska::set_named(&mut tags, name, value.as_deref());
        }
        if tags != before {
            edit.tags = Some(if matroska::any_global(&tags) {
                let xml = scratch.path("tags.xml");
                std::fs::write(&xml, matroska::global_xml(&tags)).map_err(|_| Why::Unwritable)?;
                Some(xml)
            } else {
                None
            });
        }
    }
    if !asked.streams.is_empty() || asked.cover.is_some() {
        let probed = read::probed(location, tools)?;
        // `track:n` is the n-th TRACK; ffprobe lists attachments (and the
        // covers ffmpeg makes of them) after the tracks, so they are left
        // out of the count.
        let tracks: Vec<u32> = probed
            .streams
            .iter()
            .filter(|s| s.kind != Some(StreamKind::Attachment) && !s.attached)
            .map(|s| s.index)
            .collect();
        for (index, change) in &asked.streams {
            let n = tracks
                .iter()
                .position(|i| i == index)
                .ok_or(Why::NoSuchThing)?
                + 1;
            let n = u32::try_from(n).map_err(|_| Why::NoSuchThing)?;
            let property = match change {
                StreamChange::Language(l) => matroska::Property::Language(l.trim().to_owned()),
                StreamChange::Title(t) => {
                    let t = t.trim();
                    matroska::Property::Name((!t.is_empty()).then(|| t.to_owned()))
                }
                StreamChange::Default(b) => matroska::Property::Default(*b),
                StreamChange::Forced(b) => matroska::Property::Forced(*b),
            };
            match edit.tracks.iter_mut().find(|(k, _)| *k == n) {
                Some((_, props)) => props.push(property),
                None => edit.tracks.push((n, vec![property])),
            }
        }
        if let Some(change) = &asked.cover {
            let mut fronts: Vec<String> = probed
                .streams
                .iter()
                .filter_map(|s| s.tag("filename"))
                .filter(|n| matroska::is_front_cover(n))
                .map(ToOwned::to_owned)
                .collect();
            fronts.dedup();
            if matches!(change, CoverChange::Remove) && fronts.is_empty() {
                return Err(Why::NoSuchThing);
            }
            edit.delete_attachments = fronts;
            if let CoverChange::Replace { bytes, kind } = change {
                let name = format!("cover.{}", kind.extension());
                let path = scratch.path(&name);
                std::fs::write(&path, bytes).map_err(|_| Why::Unwritable)?;
                edit.add_attachment = Some((path, name, kind.mime().to_owned()));
            }
        }
    }
    Ok(edit)
}

/// How long a program that rewrites a file may take: a minute, and a
/// second more for every sixteen mebibytes — a slow USB disk's pace, twice
/// over.
fn patience_for(len: u64) -> Duration {
    Duration::from_secs(60 + len / (16 * 1024 * 1024))
}

/// The remux: `ffmpeg -map 0 -c copy` into the draft, with the new tags.
#[allow(clippy::too_many_arguments)]
fn remux(
    location: &Path,
    folder: &Path,
    len: u64,
    container: Container,
    asked: &Asked,
    tools: &Tools,
    watch: &Watch<'_>,
    probed: Option<&crate::probe::Probed>,
) -> Result<Draft, Why> {
    let muxer = container.remux_muxer().ok_or(Why::NoWriter { container })?;
    let mut draft = Draft::reserve(location, folder, len)?;
    draft.file = None;
    let mut args: Vec<OsString> = ["-nostdin", "-v", "error", "-y", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(location.as_os_str().to_owned());
    for a in [
        "-map",
        "0",
        "-c",
        "copy",
        "-map_metadata",
        "0",
        "-map_chapters",
        "0",
    ] {
        args.push(OsString::from(a));
    }
    for (field, value) in &asked.fields {
        let key = read::ffmpeg_keys(*field)[0];
        let pair = format!("{key}={}", value.as_deref().unwrap_or_default());
        // BOTH PLACES: an Ogg file's comments belong to each stream, and the
        // muxer does not let the container's replace a stream's own.
        for place in ["-metadata", "-metadata:s"] {
            args.push(OsString::from(place));
            args.push(OsString::from(&pair));
        }
    }
    // A TAG BY ITS OWN NAME, where ffprobe read it: the container's goes in
    // both places, as a field does; a stream's into that stream alone —
    // `-map 0` keeps ffprobe's order, so its index is the output's. An
    // empty value is ffmpeg's way of taking a key out.
    for (origin, name, value) in &asked.native {
        let Origin::Ffprobe { stream } = origin else {
            return Err(Why::NoPlace);
        };
        let pair = format!("{name}={}", value.as_deref().unwrap_or_default());
        let places = match stream {
            None => vec!["-metadata".to_owned(), "-metadata:s".to_owned()],
            Some(index) => vec![format!("-metadata:s:{index}")],
        };
        for place in places {
            args.push(OsString::from(place));
            args.push(OsString::from(&pair));
        }
    }
    for a in ["-progress", "pipe:1", "-f", muxer] {
        args.push(OsString::from(a));
    }
    args.push(draft.path.as_os_str().to_owned());
    let total_us = probed
        .and_then(|p| p.duration_ms)
        .map(|ms| ms.saturating_mul(1000));
    let mut on_line = |line: &str| {
        if let (Some(total), Some(done)) = (
            total_us.filter(|t| *t > 0),
            line.strip_prefix("out_time_us=")
                .and_then(|v| v.trim().parse::<u64>().ok()),
        ) {
            watch.at(u16::try_from(done.saturating_mul(990) / total)
                .unwrap_or(990)
                .min(990));
        }
    };
    let limits = Limits {
        patience: patience_for(len),
        ceiling: Limits::READ.ceiling,
    };
    run(
        tools,
        Program::Ffmpeg,
        &args,
        limits,
        watch.stop,
        Some(&mut on_line),
    )?
    .ok(Program::Ffmpeg, &[0])?;
    Ok(draft)
}

/// Syncs the draft, gives it the file's mode, checks nothing else wrote the
/// file meanwhile, and renames it over.
fn finish(
    mut draft: Draft,
    location: &Path,
    folder: &Path,
    meta: &std::fs::Metadata,
    stamp: Stamp,
    watch: &Watch<'_>,
) -> Result<Stamp, Why> {
    draft.file()?.sync_all().map_err(|_| Why::Unwritable)?;
    std::fs::set_permissions(&draft.path, meta.permissions()).map_err(|_| Why::Unwritable)?;
    watch.stopped()?;
    let now = std::fs::metadata(location).map_err(|_| Why::Gone)?;
    if Stamp::of(&now) != stamp {
        return Err(Why::ChangedSince);
    }
    std::fs::rename(&draft.path, location).map_err(|_| Why::Unwritable)?;
    draft.kept = true;
    // The rename is a change to the FOLDER; without this a crash can bring
    // the old name back.
    if let Ok(dir) = File::open(folder) {
        let _ = dir.sync_all();
    }
    watch.at(1000);
    let after = std::fs::metadata(location).map_err(|_| Why::Gone)?;
    Ok(Stamp::of(&after))
}

/// The draft beside a file, removed unless it was renamed into place.
struct Draft {
    path: PathBuf,
    file: Option<File>,
    kept: bool,
}

/// The room a draft needs beyond the file itself: a tag grows, and a
/// filesystem at its last megabyte is one nobody should fill.
const ROOM_MARGIN_BYTES: u64 = 8 * 1024 * 1024;

impl Draft {
    /// `.<name>.ronoctua-<pid>.part`, beside it.
    fn path_for(location: &Path, pid: u32) -> Option<PathBuf> {
        let name = location.file_name()?;
        let mut draft = OsString::from(".");
        draft.push(name);
        draft.push(format!(".ronoctua-{pid}.part"));
        Some(location.with_file_name(draft))
    }

    /// Drafts of this same file left by processes that are gone.
    fn sweep(location: &Path) {
        let (Some(folder), Some(name)) = (location.parent(), location.file_name()) else {
            return;
        };
        let mut prefix = OsString::from(".");
        prefix.push(name);
        prefix.push(".ronoctua-");
        let Some(prefix) = prefix.to_str().map(ToOwned::to_owned) else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(pid) = file_name
                .to_str()
                .and_then(|n| n.strip_prefix(prefix.as_str()))
                .and_then(|rest| rest.strip_suffix(".part"))
                .and_then(|pid| pid.parse::<u32>().ok())
            else {
                continue;
            };
            let alive = Path::new(&format!("/proc/{pid}")).exists();
            if pid != std::process::id() && !alive {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// An empty draft, made new (never an existing file taken over).
    fn reserve(location: &Path, folder: &Path, need: u64) -> Result<Self, Why> {
        room(folder, need)?;
        Self::sweep(location);
        let path = Self::path_for(location, std::process::id()).ok_or(Why::NotAFile)?;
        // One of ours left by this very process (a panic between two
        // writes): it is ours, by name and pid, and goes.
        let _ = std::fs::remove_file(&path);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem => {
                    Why::ReadOnlyFolder
                }
                std::io::ErrorKind::StorageFull => Why::NoRoom {
                    need_bytes: need,
                    free_bytes: 0,
                },
                _ => Why::Unwritable,
            })?;
        Ok(Self {
            path,
            file: Some(file),
            kept: false,
        })
    }

    /// A draft with the file's bytes: cloned where the filesystem can,
    /// copied where it cannot.
    fn copy_of(location: &Path, folder: &Path, len: u64, watch: &Watch<'_>) -> Result<Self, Why> {
        let mut draft = Self::reserve(location, folder, len)?;
        let source = File::open(location).map_err(|_| Why::Gone)?;
        let target = draft.file()?;
        if rustix::fs::ioctl_ficlone(&*target, &source).is_ok() {
            watch.at(500);
            return Ok(draft);
        }
        let mut copied: u64 = 0;
        let chunk: usize = 64 * 1024 * 1024;
        let mut plain = false;
        while copied < len {
            watch.stopped()?;
            let n = if plain {
                0
            } else {
                match rustix::fs::copy_file_range(&source, None, &*target, None, chunk) {
                    Ok(n) => n,
                    // Not on this kernel or between these two: read and write.
                    Err(_) if copied == 0 => {
                        plain = true;
                        0
                    }
                    Err(_) => return Err(Why::Unwritable),
                }
            };
            if plain {
                let mut from = &source;
                std::io::copy(&mut from, &mut &*target).map_err(|_| Why::Unwritable)?;
                break;
            }
            if n == 0 {
                // The file shrank under us.
                return Err(Why::ChangedSince);
            }
            copied += n as u64;
            watch.at(u16::try_from(copied.saturating_mul(500) / len.max(1)).unwrap_or(500));
        }
        Ok(draft)
    }

    /// The draft, open for reading and writing.
    fn file(&mut self) -> Result<&mut File, Why> {
        if self.file.is_none() {
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.path)
                .map_err(|_| Why::Unwritable)?;
            self.file = Some(f);
        }
        self.file.as_mut().ok_or(Why::Unwritable)
    }
}

impl Drop for Draft {
    fn drop(&mut self) {
        if !self.kept {
            self.file = None;
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Refuses when the folder's filesystem has less room than `need` and the
/// margin.
fn room(folder: &Path, need: u64) -> Result<(), Why> {
    let Ok(stat) = rustix::fs::statvfs(folder) else {
        // A filesystem that will not say is written to, and a full one
        // fails the write, which removes the draft.
        return Ok(());
    };
    let free = stat.f_bavail.saturating_mul(stat.f_frsize);
    let need = need.saturating_add(ROOM_MARGIN_BYTES);
    if free < need {
        return Err(Why::NoRoom {
            need_bytes: need,
            free_bytes: free,
        });
    }
    Ok(())
}

/// Takes a cover out of a file into `cover.<ext>` beside it — or
/// `<stem>.cover.<ext>` when that name is taken.
///
/// Nothing is overwritten: each name is created new, and when both exist
/// the answer is [`Why::Taken`].
///
/// # Errors
///
/// [`Why::Taken`], [`Why::ReadOnlyFolder`], and whatever taking the cover
/// out refuses with.
pub fn save_cover(path: &Path, at: &CoverAt, tools: &Tools) -> Result<PathBuf, Why> {
    let (location, _) = located(path)?;
    let folder = location.parent().ok_or(Why::NotAFile)?.to_path_buf();
    writable_folder(&folder)?;
    let picture = read::cover(&location, at, tools)?;
    let stem = location.file_stem().unwrap_or_default();
    let mut second = stem.to_owned();
    second.push(format!(".cover.{}", picture.extension));
    let names = [
        OsString::from(format!("cover.{}", picture.extension)),
        second,
    ];
    for name in names {
        let target = folder.join(name);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
                ) =>
            {
                return Err(Why::ReadOnlyFolder);
            }
            Err(_) => return Err(Why::Unwritable),
        };
        let written = file
            .write_all(&picture.bytes)
            .and_then(|()| file.sync_all());
        if written.is_err() {
            drop(file);
            let _ = std::fs::remove_file(&target);
            return Err(Why::Unwritable);
        }
        return Ok(target);
    }
    Err(Why::Taken)
}

#[cfg(test)]
mod tests;
