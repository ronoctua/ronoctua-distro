//! Several-line fields in the editor: the drafts the daemon writes, watches
//! and, when a person says yes, writes into the file.
//!
//! # Why the daemon keeps them, and not a door
//!
//! Lyrics, a comment, a film's description are too long for the minibuffer:
//! they are edited in the editor, in a file. The door that opened it may be
//! gone by the time the file is saved — the person swapped the column for a
//! window, or closed the footer's view — so the draft is the DAEMON's: it
//! writes the file in its runtime directory
//! (`$XDG_RUNTIME_DIR/ronoctua-player-drafts`, memory, the dev instance's
//! own), watches it, and when it is saved with a value that is not the
//! file's, `State.drafts` says so and ANY door asks "write this?", with the
//! "no" drawn first. Yes is `write-draft`; no is `keep-draft` — not now: the
//! draft stays, and is asked about when it is saved again — and
//! `drop-draft` throws it away (a door's `d` on the field while it is open).
//!
//! # An event, not a clock
//!
//! The folder is watched with inotify for a file written and closed, or
//! renamed into place — an editor saves one way or the other (Emacs, by
//! default, renames the old file to its backup and writes a new one). One
//! thread reads the events, and only while a draft is open: taking the last
//! draft away removes the watch, which wakes the thread with `IN_IGNORED`,
//! and it leaves.

use std::{
    fs,
    io::Read as _,
    mem::MaybeUninit,
    os::{
        fd::OwnedFd,
        unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    },
    path::PathBuf,
    sync::Arc,
};

use ronoctua_player::{
    limit::{Limit, TooMany},
    wire::{self, DraftId, Drafted, Say, Trouble},
};
use ronoctua_tags::{Field, VALUE_MOST_BYTES, Why};
use rustix::fs::inotify;

use super::{Engine, Refused, Shared, tags};

/// The runtime directory's leaf the drafts are written under.
const DIRECTORY: &str = "player-drafts";

/// Where the drafts of this instance are.
fn directory() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file(DIRECTORY)
}

/// One draft.
#[derive(Debug, Clone)]
struct Open {
    id: DraftId,
    /// The file it goes into, canonical.
    location: String,
    field: Field,
    /// The draft itself.
    path: PathBuf,
    /// The field as the file had it when the draft was made, as a write
    /// would put it: a write is refused when the file's is no longer this.
    original: String,
    /// Saved with a value that is not `original`: the question is asked.
    saved: bool,
}

/// The watch on the drafts' folder, while a draft is open.
#[derive(Debug)]
struct Watch {
    fd: Arc<OwnedFd>,
    wd: i32,
    /// Which watch: a thread that outlived its watch finds another number.
    serial: u64,
}

/// The drafts open, and the watch on them.
#[derive(Debug, Default)]
pub(super) struct Drafts {
    open: Vec<Open>,
    /// The last id handed out; never reused.
    ids: u64,
    watch: Option<Watch>,
    watches: u64,
}

impl Drafts {
    /// A fresh start: what an earlier daemon left goes — its drafts were
    /// its own, and nobody is asked about them any more.
    pub(super) fn open() -> Self {
        if let Some(dir) = directory() {
            let _ = fs::remove_dir_all(dir);
        }
        Self::default()
    }

    /// The drafts, as the state says them.
    pub(super) fn list(&self) -> Vec<Drafted> {
        self.open
            .iter()
            .map(|d| Drafted {
                id: d.id,
                location: d.location.clone(),
                field: d.field,
                path: d.path.to_string_lossy().into_owned(),
                saved: d.saved,
            })
            .collect()
    }

    /// The player is closing: every draft goes with it.
    pub(super) fn end(&mut self) {
        self.open.clear();
        self.unwatch();
        if let Some(dir) = directory() {
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// What a write of draft `id` needs: its file, its field, what it
    /// started from and what it says now.
    pub(super) fn to_write(&self, id: DraftId) -> Result<(String, Field, String, String), Refused> {
        let d = self
            .open
            .iter()
            .find(|d| d.id == id)
            .ok_or(Trouble::NoSuchDraft { id })?;
        let value = read_draft(d).map_err(|why| Trouble::Tags {
            path: d.location.clone(),
            why,
        })?;
        Ok((d.location.clone(), d.field, d.original.clone(), value))
    }

    /// Draft `id` is in its file: it goes.
    pub(super) fn written(&mut self, id: DraftId) {
        self.close(id);
    }

    /// A write of draft `id` was refused: nobody is asked again until it is
    /// saved again.
    pub(super) fn refused(&mut self, id: DraftId) {
        if let Some(d) = self.open.iter_mut().find(|d| d.id == id) {
            d.saved = false;
        }
    }

    /// Draft `id` stays open, and no question is asked about it until it is
    /// saved again — `false` for none open by that id.
    fn not_now(&mut self, id: DraftId) -> bool {
        let Some(d) = self.open.iter_mut().find(|d| d.id == id) else {
            return false;
        };
        d.saved = false;
        true
    }

    /// Draft `id` goes, unwritten: its file, what the editor left beside
    /// it, and — with the last one — the watch.
    fn close(&mut self, id: DraftId) -> bool {
        let Some(at) = self.open.iter().position(|d| d.id == id) else {
            return false;
        };
        let d = self.open.remove(at);
        remove_with_leftovers(&d.path);
        if self.open.is_empty() {
            self.unwatch();
        }
        true
    }

    /// The watch goes: its thread wakes with `IN_IGNORED` and leaves.
    fn unwatch(&mut self) {
        if let Some(w) = self.watch.take() {
            let _ = inotify::remove_watch(&*w.fd, w.wd);
        }
    }
}

/// A draft's file, and what an editor leaves beside a file it edits: a
/// backup (`name~`), an autosave (`#name#`) and a lock (`.#name`).
fn remove_with_leftovers(path: &std::path::Path) {
    let _ = fs::remove_file(path);
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    for other in [format!("{name}~"), format!("#{name}#"), format!(".#{name}")] {
        let _ = fs::remove_file(dir.join(other));
    }
}

/// What a draft says now, as a write would put it.
fn read_draft(d: &Open) -> Result<String, Why> {
    let file = fs::File::open(&d.path).map_err(|_| Why::Gone)?;
    let mut bytes = Vec::new();
    // ONE BYTE PAST THE CEILING tells "too big" from "exactly the most".
    file.take(VALUE_MOST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Why::Unreadable)?;
    let text = String::from_utf8(bytes).map_err(|_| Why::Unreadable)?;
    d.field.check(&text)
}

/// The file name of draft `id` for `field`: `<id>-<field>.txt`, so an editor
/// knows it for text and a person knows it for the field.
fn file_name(id: DraftId, field: Field) -> String {
    let field = serde_json::to_value(field)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("{}-{field}.txt", id.0)
}

/// A draft asked for ([`wire::Ask::Draft`]): the file's sheet read (off the
/// lock, as every sheet is), then the draft written and watched. The same
/// file and field asked again is the same draft.
pub(super) fn open_draft(location: &str, field: Field, shared: &Shared) -> Say {
    let sheet = match tags::sheet_of(location, shared) {
        Ok(s) => s,
        Err(t) => return Say::Refused(t),
    };
    let refuse = |why| {
        Say::Refused(Trouble::Tags {
            path: location.to_owned(),
            why,
        })
    };
    if sheet.stamp == ronoctua_tags::Stamp::NONE {
        return refuse(Why::NotAFile);
    }
    // WHETHER IT CAN BE WRITTEN, before anybody types a word into it: the
    // field's own row, else any field's — every field of a file has one
    // writer.
    let can = sheet
        .field(field)
        .or_else(|| sheet.fields.first())
        .map(|row| row.can.clone());
    match can {
        Some(ronoctua_tags::Can::Yes(_)) => {}
        Some(ronoctua_tags::Can::No(why)) => return refuse(why),
        None => {
            return refuse(Why::NoWriter {
                container: sheet.container,
            });
        }
    }
    let original = tags::checked(field, sheet.value(field));
    let target = sheet.location.to_string_lossy().into_owned();
    let (lock, bell) = &**shared;
    let Ok(mut e) = lock.lock() else {
        return refuse(Why::Unwritable);
    };
    let say = match e.draft(&target, field, original, shared) {
        Ok((id, path)) => Say::Draft {
            id,
            path: path.to_string_lossy().into_owned(),
        },
        Err(Refused(t)) => Say::Refused(t),
    };
    e.dirty = true;
    e.broadcast();
    drop(e);
    bell.notify_all();
    say
}

impl Engine {
    /// The draft of `field` of `location` — the one open, else a new one
    /// holding `original`.
    fn draft(
        &mut self,
        location: &str,
        field: Field,
        original: String,
        shared: &Shared,
    ) -> Result<(DraftId, PathBuf), Refused> {
        if let Some(d) = self
            .drafts
            .open
            .iter()
            .find(|d| d.location == location && d.field == field)
        {
            return Ok((d.id, d.path.clone()));
        }
        if self.drafts.open.len() >= wire::DRAFTS_MOST {
            return Err(Trouble::TooMany(TooMany {
                what: Limit::Drafts,
                most: wire::DRAFTS_MOST,
            })
            .into());
        }
        let unwritable = |why: String| Trouble::Unwritable {
            path: location.to_owned(),
            why,
        };
        let dir = directory().ok_or(Trouble::NoRuntimeDir)?;
        fs::create_dir_all(&dir).map_err(|e| unwritable(e.to_string()))?;
        // THE PERSON'S WORDS, in memory: this user's alone.
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        self.watch_drafts(&dir, shared)?;
        self.drafts.ids += 1;
        let id = DraftId(self.drafts.ids);
        let path = dir.join(file_name(id, field));
        let mut text = original.clone();
        if !text.is_empty() {
            text.push('\n');
        }
        let written = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
        if let Err(e) = written {
            let _ = fs::remove_file(&path);
            return Err(unwritable(e.to_string()).into());
        }
        self.drafts.open.push(Open {
            id,
            location: location.to_owned(),
            field,
            path: path.clone(),
            original,
            saved: false,
        });
        Ok((id, path))
    }

    /// The folder watched, and its thread started, when nothing watches it.
    fn watch_drafts(&mut self, dir: &std::path::Path, shared: &Shared) -> Result<(), Refused> {
        if self.drafts.watch.is_some() {
            return Ok(());
        }
        let refused = |e: rustix::io::Errno| Trouble::Unwritable {
            path: dir.to_string_lossy().into_owned(),
            why: e.to_string(),
        };
        let fd = inotify::init(inotify::CreateFlags::CLOEXEC).map_err(refused)?;
        let wd = inotify::add_watch(
            &fd,
            dir,
            inotify::WatchFlags::CLOSE_WRITE | inotify::WatchFlags::MOVED_TO,
        )
        .map_err(refused)?;
        self.drafts.watches += 1;
        let serial = self.drafts.watches;
        let fd = Arc::new(fd);
        spawn_watch(fd.clone(), serial, shared.clone());
        self.drafts.watch = Some(Watch { fd, wd, serial });
        Ok(())
    }

    /// A file of the drafts' folder was saved: when it is a draft and says
    /// something the file does not, the question is asked.
    fn draft_saved(&mut self, name: &str) {
        let Some(d) = self
            .drafts
            .open
            .iter()
            .find(|d| d.path.file_name().and_then(|n| n.to_str()) == Some(name))
            .cloned()
        else {
            return;
        };
        let saved = match read_draft(&d) {
            Ok(value) => value != d.original,
            Err(why) => {
                self.state.troubles = vec![Trouble::Tags {
                    path: d.location.clone(),
                    why,
                }];
                false
            }
        };
        if let Some(open) = self.drafts.open.iter_mut().find(|o| o.id == d.id) {
            open.saved = saved;
        }
        self.dirty = true;
    }

    /// A draft let go ([`wire::Ask::DropDraft`]).
    pub(super) fn drop_draft(&mut self, id: DraftId) -> Result<(), Refused> {
        if self.drafts.close(id) {
            Ok(())
        } else {
            Err(Trouble::NoSuchDraft { id }.into())
        }
    }

    /// NOT NOW ([`wire::Ask::KeepDraft`]): the draft stays open and
    /// watched, and nobody is asked about it until it is saved again — the
    /// "no" of the question, which must not throw away a text still being
    /// written.
    pub(super) fn keep_draft(&mut self, id: DraftId) -> Result<(), Refused> {
        if !self.drafts.not_now(id) {
            return Err(Trouble::NoSuchDraft { id }.into());
        }
        self.dirty = true;
        Ok(())
    }
}

/// The thread that hears the drafts' folder, for as long as its watch lasts.
fn spawn_watch(fd: Arc<OwnedFd>, serial: u64, shared: Shared) {
    let _ = std::thread::Builder::new()
        .name("player-drafts".into())
        .spawn(move || {
            let mut buf = [MaybeUninit::<u8>::uninit(); 4096];
            let mut events = inotify::Reader::new(&*fd, &mut buf);
            loop {
                let (gone, name) = match events.next() {
                    Ok(event) => (
                        event.events().contains(inotify::ReadFlags::IGNORED),
                        event
                            .file_name()
                            .and_then(|n| n.to_str().ok())
                            .map(str::to_owned),
                    ),
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(_) => (true, None),
                };
                let (lock, bell) = &*shared;
                let Ok(mut e) = lock.lock() else { return };
                if gone {
                    // THE WATCH WENT (the last draft closed, or the folder
                    // did): this thread's, when it is still the one.
                    if e.drafts.watch.as_ref().is_some_and(|w| w.serial == serial) {
                        e.drafts.watch = None;
                    }
                    return;
                }
                if let Some(name) = name {
                    e.draft_saved(&name);
                    if e.dirty {
                        e.broadcast();
                    }
                }
                drop(e);
                bell.notify_all();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a draft's file name stops saying its field, or two drafts
    /// could share one.
    #[test]
    fn a_draft_is_named_by_its_id_and_its_field() {
        assert_eq!(file_name(DraftId(3), Field::Lyrics), "3-lyrics.txt");
        assert_eq!(
            file_name(DraftId(12), Field::Description),
            "12-description.txt"
        );
        assert_ne!(
            file_name(DraftId(1), Field::Comment),
            file_name(DraftId(2), Field::Comment)
        );
    }

    /// FAILS IF: what an editor leaves beside a draft outlives it — a
    /// backup of the person's words, in memory, for as long as the session.
    #[test]
    fn a_draft_goes_with_what_the_editor_left_beside_it() {
        let dir = std::env::temp_dir().join(format!("ronoctua-drafts-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("a folder");
        let draft = dir.join("1-lyrics.txt");
        for name in [
            "1-lyrics.txt",
            "1-lyrics.txt~",
            "#1-lyrics.txt#",
            ".#1-lyrics.txt",
            "2-comment.txt",
        ] {
            fs::write(dir.join(name), "x").expect("written");
        }
        remove_with_leftovers(&draft);
        let left: Vec<String> = fs::read_dir(&dir)
            .expect("listed")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["2-comment.txt"], "another draft stays");
        fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: "not now" throws the draft away — its file, or its place
    /// among the open ones — or leaves it asked about.
    #[test]
    fn not_now_keeps_the_draft_and_its_file() {
        let dir = std::env::temp_dir().join(format!("ronoctua-drafts-keep-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join("4-lyrics.txt");
        fs::write(&path, "half written").expect("written");
        let mut drafts = Drafts::default();
        drafts.open.push(Open {
            id: DraftId(4),
            location: "/m/a.flac".into(),
            field: Field::Lyrics,
            path: path.clone(),
            original: String::new(),
            saved: true,
        });
        assert!(drafts.not_now(DraftId(4)));
        assert!(!drafts.not_now(DraftId(5)), "no such draft");
        let listed = drafts.list();
        assert_eq!(listed.len(), 1, "the draft is still open");
        assert!(
            !listed[0].saved,
            "and nobody is asked until it is saved again"
        );
        assert!(path.exists(), "its file is where the editor has it");
        fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: a draft is read as anything but what a write would put — a
    /// trailing newline the editor added would be a change — or a draft
    /// past the ceiling reaches a write.
    #[test]
    fn a_draft_reads_as_a_write_would_put_it() {
        let dir = std::env::temp_dir().join(format!("ronoctua-drafts-read-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join("1-lyrics.txt");
        fs::write(&path, "first\r\nsecond\n\n").expect("written");
        let d = Open {
            id: DraftId(1),
            location: "/m/a.flac".into(),
            field: Field::Lyrics,
            path: path.clone(),
            original: "first\nsecond".into(),
            saved: false,
        };
        assert_eq!(read_draft(&d).as_deref(), Ok("first\nsecond"));
        fs::write(&path, "x".repeat(VALUE_MOST_BYTES + 1)).expect("written");
        assert!(matches!(read_draft(&d), Err(Why::TooBig { .. })));
        fs::remove_dir_all(&dir).expect("cleaned");
    }
}
