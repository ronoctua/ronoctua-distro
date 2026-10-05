//! EVERYTHING THAT CAN BE EDITED, IS — the info panel's rows that change the
//! file they describe.
//!
//! # What is editable, and who writes it
//!
//! | Row | Writer |
//! |---|---|
//! | the name | `ronoctua_files::act::rename` (the rename question the menu asks) |
//! | the permission, the owner | the menu's own questions (`Asking::Mode`, `Asking::Owner`) |
//! | when it was modified | `ronoctua_files::act::set_modified` (`utimensat`, the access time kept) |
//! | a song's or a film's tags | the `ronoctua-tags` PROGRAM, as a child ([`Tags`]) |
//! | a picture's or a PDF's details | `ronoctua_inspect::amend::set` |
//! | where a photograph was taken | `ronoctua_inspect::amend::remove_location`, after a question |
//!
//! [`crate::inspection_sheet::Doable`] names each, and the row that offers one
//! is drawn as a SETTING (`chrome::Kind::Setting`); a row nothing can change is
//! a FACT. Pressing the row — anywhere on it — or `RET` on it with the facts
//! focused asks the question, in the manager's own picker, in either door.
//!
//! # Why lofty is a child here
//!
//! A song's tags are parsed by lofty, out of whatever bytes the file holds. In
//! the player's daemon that is a daemon's risk; in the footer it would be the
//! desktop's — a panic or a runaway parse in the process that draws the
//! modeline. So the footer never links it: it runs `ronoctua-tags sheet` and
//! `ronoctua-tags apply` and reads one JSON answer (`ronoctua_tags::Answer`),
//! with a deadline, and the worst a hostile file can do is end one child
//! (`the-footer-never-links-lofty`).
//!
//! # One exiftool writer
//!
//! The menu's "remove the place" and the panel's place row reach the SAME
//! question and the same function, `amend::remove_location`, run on a thread
//! of this process — the reconciliation of two designs that each had a writer
//! (a queue job and this module): exiftool is already a child, so the queue
//! would add a socket and a second process in front of one short write and
//! nothing else. `one-inspector` refuses a second writer anywhere.
//!
//! # After a write
//!
//! Nothing is patched in place. Every writer moves the file's mtime, the
//! sheet's key (`SheetFor`) carries the mtime, and the panel is asked about
//! the file again — so what is drawn after a change is what the file now
//! says, read the same way it was read before.

use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use ronoctua_files::mode::Explanation;
use ronoctua_inspect::amend::{self, Holder, Meta};
use ronoctua_tags::{Answer, Edit, Field, Lines, Stamp, Why};

use crate::inspection_sheet::{Doable, Fact};

/// How long `ronoctua-tags sheet` may take: ffprobe, exiftool and lofty on
/// one file, measured in tens of milliseconds, with a margin for a slow disk.
pub const SHEET_PATIENCE: Duration = Duration::from_secs(15);

/// How long `ronoctua-tags apply` may take. A REMUX rewrites the whole film
/// into a draft beside it, so this is a copy's time and not a parse's; the
/// panel waits on a thread and is never blocked by it.
pub const APPLY_PATIENCE: Duration = Duration::from_secs(600);

/// The most the child may write to stdout — a sheet with every tag of a file
/// that carries hundreds, many times over.
const ANSWER_MOST_BYTES: u64 = 16 * 1024 * 1024;

/// What the tag program said about one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tags {
    /// Its tags, and whether each can be written.
    Sheet(Box<ronoctua_tags::Sheet>),
    /// Why it has none to show.
    Refused(Why),
    /// The program is not on this machine.
    Absent,
    /// It ran and did not answer — killed, too slow, or said nothing readable.
    Stopped,
}

/// What exiftool said about a picture's or a PDF's six details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Details {
    /// Each detail and its value, blanks included.
    Read(Holder, Vec<(Meta, String)>),
    /// Why they could not be read.
    Refused(ronoctua_inspect::Refusal),
}

/// Everything the panel can change beyond what `lstat` answers, read on the
/// inspector's thread beside the facts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Amendable {
    /// A song's or a film's tags.
    pub tags: Option<Tags>,
    /// A picture's or a PDF's details.
    pub details: Option<Details>,
}

/// Reads what the panel can change about one file. Synchronous and it spawns
/// children: the inspector's thread only.
#[must_use]
pub fn read(path: &Path, mime: &str) -> Amendable {
    let canonical = ronoctua_files::mime::canonical(mime);
    let media = canonical.split_once('/').map_or(canonical, |(m, _)| m);
    let tagged =
        matches!(media, "audio" | "video") && !ronoctua_inspect::playlist::is_a_playlist(canonical);
    Amendable {
        tags: tagged.then(|| tags_of(path)),
        details: Holder::of(mime).map(|holder| match amend::read(path, holder) {
            Ok(read) => Details::Read(holder, read),
            Err(why) => Details::Refused(why),
        }),
    }
}

/// Where the tag program is: `RONOCTUA_TAGS` when the environment names it,
/// else beside this binary — both come out of the same build, and a variable
/// holding a path of the workspace the shell is itself built in would be the
/// workspace referencing itself (the reason the queue's daemon is found the
/// same way, `ronoctua_transfers::client`).
#[must_use]
pub fn program() -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("RONOCTUA_TAGS").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(named));
    }
    let me = std::env::current_exe().ok()?;
    let beside = me.parent()?.join("ronoctua-tags");
    beside.is_file().then_some(beside)
}

/// `ronoctua-tags sheet <path>`, answered.
#[must_use]
pub fn tags_of(path: &Path) -> Tags {
    sheet_by(program(), path)
}

/// [`tags_of`], through the program at `program` — a PARAMETER, so a test can
/// hand it a stand-in and drive every road the child's answer can take.
fn sheet_by(program: Option<PathBuf>, path: &Path) -> Tags {
    match asked(
        program,
        &[OsString::from("sheet"), path.as_os_str().to_owned()],
        None,
        SHEET_PATIENCE,
    ) {
        Ok(Answer::Sheet { sheet }) => Tags::Sheet(sheet),
        Ok(Answer::Refused { why }) => Tags::Refused(why),
        Ok(Answer::Written { .. }) => Tags::Stopped,
        Err(trouble) => trouble,
    }
}

/// Runs the tag program once: `words`, what goes on its stdin, and how long it
/// may take. One JSON [`Answer`] comes back, or [`Tags::Absent`] /
/// [`Tags::Stopped`] saying why none did.
///
/// # Events, not a clock
///
/// The child's stdin and stdout are each drained on a thread of their own, so
/// neither pipe can fill and stop it while the other is being waited on. The
/// child answers with ONE line and exits, so the END of its stdout is the
/// event this thread waits for — on a channel, with the patience as the
/// channel's timeout. Nothing wakes in between: a remux that takes ten minutes
/// is ten minutes of one blocked `recv`, not thirty thousand polls. Past the
/// patience the child is killed, which closes its stdout and lets the reader
/// go. A reader that stops at [`ANSWER_MOST_BYTES`] drops the pipe, so a child
/// still writing gets `EPIPE` and ends rather than blocking the `wait`.
///
/// Every caller is already off the loop that draws (the inspector's thread, a
/// writer's).
fn asked(
    program: Option<PathBuf>,
    words: &[OsString],
    input: Option<Vec<u8>>,
    patience: Duration,
) -> Result<Answer, Tags> {
    let program = program.ok_or(Tags::Absent)?;
    let mut child = std::process::Command::new(&program)
        .args(words)
        .stdin(if input.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            tracing::warn!(program = %program.display(), error = %e, "the tag program did not start");
            Tags::Absent
        })?;
    if let (Some(bytes), Some(mut pipe)) = (input, child.stdin.take()) {
        std::thread::Builder::new()
            .name("files-tags-in".to_owned())
            .spawn(move || {
                let _ = pipe.write_all(&bytes);
            })
            .map_err(|_| Tags::Stopped)?;
    }
    let Some(pipe) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Tags::Stopped);
    };
    let (sent, heard) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("files-tags-out".to_owned())
        .spawn(move || {
            let mut out = Vec::new();
            let _ = pipe.take(ANSWER_MOST_BYTES).read_to_end(&mut out);
            let _ = sent.send(out);
        })
        .map_err(|_| Tags::Stopped)?;
    let Ok(out) = heard.recv_timeout(patience) else {
        tracing::warn!(program = %program.display(), "the tag program went past its patience");
        let _ = child.kill();
        let _ = child.wait();
        return Err(Tags::Stopped);
    };
    let status = child.wait().map_err(|_| Tags::Stopped)?;
    if !status.success() {
        return Err(Tags::Stopped);
    }
    serde_json::from_slice(&out).map_err(|e| {
        tracing::warn!(error = %e, "the tag program's answer is not one");
        Tags::Stopped
    })
}

/// The label of a tag's row: the player's own key for the field, so a song's
/// title is called the same in both places it can be changed.
fn tag_label(field: Field, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(crate::player_metadata::field_key(field))
}

/// The label of a detail's row.
#[must_use]
pub const fn meta_key(meta: Meta) -> &'static str {
    use ronoctua_i18n::key;
    match meta {
        Meta::Title => key::FILES_META_TITLE,
        Meta::Author => key::FILES_META_AUTHOR,
        Meta::Description => key::FILES_META_DESCRIPTION,
        Meta::Keywords => key::FILES_META_KEYWORDS,
        Meta::Copyright => key::FILES_META_COPYRIGHT,
        Meta::Taken => key::FILES_META_TAKEN,
    }
}

/// The fact key the READ-ONLY sheet shows a tag under
/// (`ronoctua_inspect::facts::from_probe`), so the editable row replaces it
/// instead of repeating it.
const fn probe_key(field: Field) -> Option<&'static str> {
    use ronoctua_inspect::key;
    Some(match field {
        Field::Title => key::FACT_TITLE,
        Field::Artist => key::FACT_ARTIST,
        Field::Album => key::FACT_ALBUM,
        Field::AlbumArtist => key::FACT_ALBUM_ARTIST,
        Field::Track => key::FACT_TRACK,
        Field::Disc => key::FACT_DISC,
        Field::Date => key::FACT_YEAR,
        Field::Genre => key::FACT_GENRE,
        Field::Composer => key::FACT_COMPOSER,
        Field::Publisher => key::FACT_PUBLISHER,
        Field::Isrc => key::FACT_ISRC,
        _ => return None,
    })
}

/// Whether a read-only fact says the same thing an editable row of this file
/// says — and so gives its place to that row. Two rows about one title, one
/// that can be pressed and one that cannot, would be two answers to "can I
/// change this".
#[must_use]
pub fn shadowed(amendable: &Amendable, fact: &ronoctua_inspect::facts::Fact) -> bool {
    use ronoctua_inspect::facts::Label;
    if let Some(Tags::Sheet(sheet)) = &amendable.tags {
        for row in &sheet.fields {
            let hit = match &fact.label {
                Label::Key(k) => probe_key(row.field) == Some(*k),
                Label::Named(n) => row
                    .field
                    .ffmpeg_keys()
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(n)),
            };
            if hit {
                return true;
            }
        }
    }
    if let Some(Details::Read(holder, _)) = &amendable.details {
        return Meta::ALL.iter().any(|meta| match &fact.label {
            Label::Key(k) => {
                *meta == Meta::Taken
                    && *holder == Holder::Picture
                    && *k == ronoctua_inspect::key::FACT_TAKEN
            }
            Label::Named(n) => meta
                .also_shown_as(*holder)
                .iter()
                .any(|name| name.eq_ignore_ascii_case(n)),
        });
    }
    false
}

/// Whether a tag's value can be edited on ONE LINE here.
///
/// A comment, a description or a song's words can hold several lines; the
/// panel's question is one. A blank or one-line value of such a field is
/// edited like any other — what is refused is flattening several lines into
/// one without saying so, and those rows stay facts whose sentence says where
/// they are edited (the player's editor).
fn on_one_line(field: Field, value: Option<&str>) -> bool {
    field.lines() == Lines::One || !value.is_some_and(|v| v.contains('\n'))
}

/// The rows this module adds to a sheet: the tags, the details, or the
/// sentence that says why there are none. In the panel's reading order, after
/// what the filesystem knows.
#[must_use]
pub fn rows(amendable: &Amendable, texts: &ronoctua_i18n::Texts) -> Vec<Fact> {
    let mut out = Vec::new();
    match &amendable.tags {
        None => {}
        Some(Tags::Sheet(sheet)) => {
            for row in &sheet.fields {
                let label = tag_label(row.field, texts);
                let value = row.value.clone().unwrap_or_default();
                let (does, says) = match &row.can {
                    ronoctua_tags::Can::Yes(_) if on_one_line(row.field, row.value.as_deref()) => {
                        (Doable::Tag(row.field), None)
                    }
                    ronoctua_tags::Can::Yes(_) => (
                        Doable::Nothing,
                        Some(Explanation {
                            key: ronoctua_i18n::key::FILES_TAG_SEVERAL_LINES,
                            args: vec![("label", label.clone())],
                        }),
                    ),
                    ronoctua_tags::Can::No(why) => (
                        Doable::Nothing,
                        Some(Explanation {
                            key: ronoctua_i18n::key::FILES_TAG_CANNOT,
                            args: vec![
                                ("label", label.clone()),
                                ("why", crate::player::tags_refusal(why, texts)),
                            ],
                        }),
                    ),
                };
                out.push(Fact {
                    label,
                    value: value.lines().next().unwrap_or_default().to_owned(),
                    does,
                    says,
                });
            }
        }
        Some(Tags::Refused(why)) => out.push(Fact {
            label: texts.t(ronoctua_i18n::key::FILES_FACT_TAGS),
            value: crate::player::tags_refusal(why, texts),
            does: Doable::Nothing,
            says: None,
        }),
        Some(Tags::Absent) => out.push(Fact {
            label: texts.t(ronoctua_i18n::key::FILES_FACT_TAGS),
            value: texts.t(ronoctua_i18n::key::FILES_TAGS_ABSENT),
            does: Doable::Nothing,
            says: None,
        }),
        Some(Tags::Stopped) => out.push(Fact {
            label: texts.t(ronoctua_i18n::key::FILES_FACT_TAGS),
            value: texts.t(ronoctua_i18n::key::FILES_TAGS_STOPPED),
            does: Doable::Nothing,
            says: None,
        }),
    }
    match &amendable.details {
        None => {}
        Some(Details::Read(_, read)) => {
            for (meta, value) in read {
                out.push(Fact {
                    label: texts.t(meta_key(*meta)),
                    value: value.clone(),
                    does: Doable::Meta(*meta),
                    says: None,
                });
            }
        }
        Some(Details::Refused(why)) => out.push(Fact {
            label: texts.t(ronoctua_i18n::key::FILES_FACT_DETAILS),
            value: texts.t_with(why.key(), &borrowed(&why.args())),
            does: Doable::Nothing,
            says: None,
        }),
    }
    out
}

fn borrowed<'a>(args: &'a [(&'static str, String)]) -> Vec<(&'a str, &'a str)> {
    args.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

/// A tag's current value and the stamp a write of it must name — what the
/// question is prefilled with and what the writer is handed.
#[must_use]
pub fn tag_now(amendable: &Amendable, field: Field) -> Option<(String, Stamp)> {
    let Some(Tags::Sheet(sheet)) = &amendable.tags else {
        return None;
    };
    let value = sheet
        .field(field)
        .and_then(|row| row.value.clone())
        .unwrap_or_default();
    Some((value, sheet.stamp))
}

/// A detail's current value and the holder it is written through.
#[must_use]
pub fn detail_now(amendable: &Amendable, meta: Meta) -> Option<(String, Holder)> {
    let Some(Details::Read(holder, read)) = &amendable.details else {
        return None;
    };
    read.iter()
        .find(|(m, _)| *m == meta)
        .map(|(_, v)| (v.clone(), *holder))
}

/// A tag's answer, checked before anything is written: the field's own rule
/// (`Field::check`), so a date that is not one is refused under the cursor.
///
/// # Errors
///
/// The sentence that says why, in the language on screen.
pub fn tag_answer(field: Field, raw: &str, texts: &ronoctua_i18n::Texts) -> Result<Edit, String> {
    field
        .check(raw)
        .map(|value| {
            if value.is_empty() {
                Edit::Clear { field }
            } else {
                Edit::Set { field, value }
            }
        })
        .map_err(|why| crate::player::tags_refusal(&why, texts))
}

/// What a writer on a thread came back with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Amended {
    /// Written; the panel reads the file again.
    Written,
    /// The place came out of these files — or of the COPIES at these paths.
    Located {
        /// Where the files without a place are.
        landed: Vec<PathBuf>,
        /// Whether they are copies.
        copies: bool,
    },
    /// The tag program refused, and why.
    Tags(Why),
    /// The tag program did not answer.
    TagsTrouble(Tags),
    /// exiftool refused, and why.
    Details(amend::Refused),
}

/// Writes one tag edit through the child. On a writer's thread.
#[must_use]
pub fn write_tags(path: &Path, stamp: Stamp, edits: &[Edit]) -> Amended {
    write_tags_by(program(), path, stamp, edits)
}

/// [`write_tags`], through the program at `program` (see [`sheet_by`]).
fn write_tags_by(program: Option<PathBuf>, path: &Path, stamp: Stamp, edits: &[Edit]) -> Amended {
    let (Ok(stamp), Ok(edits)) = (serde_json::to_string(&stamp), serde_json::to_vec(edits)) else {
        return Amended::TagsTrouble(Tags::Stopped);
    };
    match asked(
        program,
        &[
            OsString::from("apply"),
            path.as_os_str().to_owned(),
            OsString::from("--stamp"),
            OsString::from(stamp),
        ],
        Some(edits),
        APPLY_PATIENCE,
    ) {
        Ok(Answer::Written { .. }) => Amended::Written,
        Ok(Answer::Refused { why }) => Amended::Tags(why),
        Ok(Answer::Sheet { .. }) => Amended::TagsTrouble(Tags::Stopped),
        Err(trouble) => Amended::TagsTrouble(trouble),
    }
}

/// Writes one detail. On a writer's thread.
#[must_use]
pub fn write_detail(path: &Path, holder: Holder, meta: Meta, value: &str) -> Amended {
    match amend::set(path, holder, meta, value) {
        Ok(()) => Amended::Written,
        Err(why) => Amended::Details(why),
    }
}

/// Takes the place out of every one of `paths` — over each, or into a copy
/// beside each — and STOPS at the first refusal, for the reason every set
/// operation of the manager gives: a set half changed with one message about
/// one file is a state nobody can act on. On a writer's thread.
#[must_use]
pub fn remove_locations(paths: &[PathBuf], copies: bool) -> Amended {
    let mut landed = Vec::with_capacity(paths.len());
    for path in paths {
        let at = if copies {
            amend::Where::Copy(amend::copy_beside(path))
        } else {
            amend::Where::InPlace
        };
        match amend::remove_location(path, &at) {
            Ok(file) => landed.push(file),
            Err(why) => return Amended::Details(why),
        }
    }
    Amended::Located { landed, copies }
}

/// What the message line says once a writer is done — `None` for a plain
/// success, whose proof is the panel showing the new value. One function, and
/// both doors call it.
#[must_use]
pub fn sentence(amended: &Amended, texts: &ronoctua_i18n::Texts) -> Option<String> {
    use ronoctua_i18n::{Arg, key};
    Some(match amended {
        Amended::Written => return None,
        Amended::Located { landed, copies } => {
            let n = i64::try_from(landed.len()).unwrap_or(i64::MAX);
            let name = landed
                .first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            texts.t_args(
                if *copies {
                    key::FILES_LOCATION_COPIED
                } else {
                    key::FILES_LOCATION_REMOVED
                },
                &[("n", Arg::Number(n)), ("name", Arg::Text(&name))],
            )
        }
        Amended::Tags(why) => crate::player::tags_refusal(why, texts),
        Amended::TagsTrouble(Tags::Absent) => texts.t(key::FILES_TAGS_ABSENT),
        Amended::TagsTrouble(_) => texts.t(key::FILES_TAGS_STOPPED),
        Amended::Details(why) => refused_said(why, texts),
    })
}

/// exiftool's refusal, in words.
#[must_use]
pub fn refused_said(why: &amend::Refused, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    match why {
        amend::Refused::TooLong => texts.t_args(
            key::FILES_AMEND_TOO_LONG,
            &[(
                "most",
                ronoctua_i18n::Arg::Number(
                    i64::try_from(amend::VALUE_MOST_BYTES).unwrap_or(i64::MAX),
                ),
            )],
        ),
        amend::Refused::BadDate => texts.t(key::FILES_AMEND_BAD_DATE),
        amend::Refused::NotWritten => texts.t(key::FILES_AMEND_NOT_WRITTEN),
        amend::Refused::Taken => texts.t(key::FILES_AMEND_TAKEN),
        amend::Refused::Program(r) => texts.t_with(r.key(), &borrowed(&r.args())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB exists")
    }

    fn sheet_with(fields: Vec<ronoctua_tags::FieldRow>) -> ronoctua_tags::Sheet {
        ronoctua_tags::Sheet {
            location: PathBuf::from("/music/song.flac"),
            stamp: Stamp {
                len: 10,
                mtime_ns: 20,
                ino: 30,
            },
            kind: ronoctua_tags::Kind::Music,
            container: ronoctua_tags::Container::Flac,
            fields,
            covers: Vec::new(),
            cover_can: ronoctua_tags::Can::No(Why::NoPicturesIn {
                format: ronoctua_tags::TagFormat::RiffInfo,
            }),
            technical: Vec::new(),
            streams: Vec::new(),
            chapters: Vec::new(),
            chapters_can: None,
            other: Vec::new(),
            unread: Vec::new(),
            cover_file: None,
            adding: None,
        }
    }

    fn row(field: Field, value: Option<&str>, can: bool) -> ronoctua_tags::FieldRow {
        ronoctua_tags::FieldRow {
            field,
            value: value.map(ToOwned::to_owned),
            can: if can {
                ronoctua_tags::Can::Yes(ronoctua_tags::Writer::Lofty)
            } else {
                ronoctua_tags::Can::No(Why::Missing {
                    program: ronoctua_tags::Program::Mkvpropedit,
                })
            },
        }
    }

    #[test]
    fn a_tag_the_file_can_take_is_a_setting_and_one_it_cannot_says_why() {
        let t = texts();
        let amendable = Amendable {
            tags: Some(Tags::Sheet(Box::new(sheet_with(vec![
                row(Field::Title, Some("Aquarela"), true),
                row(Field::Album, None, true),
                row(Field::Artist, Some("Toquinho"), false),
                row(Field::Lyrics, Some("uma\nduas"), true),
            ])))),
            details: None,
        };
        let rows = rows(&amendable, &t);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].does, Doable::Tag(Field::Title));
        assert_eq!(rows[0].value, "Aquarela");
        // A BLANK IS A ROW THAT CAN BE FILLED, and it is a setting.
        assert_eq!(rows[1].does, Doable::Tag(Field::Album));
        assert_eq!(rows[1].value, "");
        // REFUSED BY THE FILE: a fact, and the sentence names the program.
        assert_eq!(rows[2].does, Doable::Nothing);
        let says = rows[2].says.as_ref().expect("a refusal says why");
        assert_eq!(says.key, ronoctua_i18n::key::FILES_TAG_CANNOT);
        assert!(
            says.args.iter().any(|(_, v)| v.contains("mkvpropedit")),
            "{says:?}"
        );
        // AND SEVERAL LINES ARE NOT FLATTENED onto one without a word.
        assert_eq!(rows[3].does, Doable::Nothing);
        assert_eq!(rows[3].value, "uma");
    }

    #[test]
    fn the_editable_row_takes_the_place_of_the_read_only_one() {
        use ronoctua_inspect::facts::{Fact as Told, Label, Value};
        let amendable = Amendable {
            tags: Some(Tags::Sheet(Box::new(sheet_with(vec![row(
                Field::Title,
                Some("Aquarela"),
                true,
            )])))),
            details: Some(Details::Read(
                Holder::Picture,
                vec![(Meta::Copyright, String::new())],
            )),
        };
        let told = |label| Told {
            label,
            value: Value::Data("x".to_owned()),
        };
        assert!(shadowed(
            &amendable,
            &told(Label::Key(ronoctua_inspect::key::FACT_TITLE))
        ));
        assert!(shadowed(
            &amendable,
            &told(Label::Named("COPYRIGHT".to_owned()))
        ));
        assert!(shadowed(
            &amendable,
            &told(Label::Key(ronoctua_inspect::key::FACT_TAKEN))
        ));
        // And a fact nothing here edits stays.
        assert!(!shadowed(
            &amendable,
            &told(Label::Key(ronoctua_inspect::key::FACT_ISO))
        ));
        assert!(!shadowed(
            &amendable,
            &told(Label::Named("LensModel".to_owned()))
        ));
    }

    #[test]
    fn a_tag_answer_is_checked_before_anything_is_written() {
        let t = texts();
        assert_eq!(
            tag_answer(Field::Track, "3/12", &t),
            Ok(Edit::Set {
                field: Field::Track,
                value: "3/12".to_owned()
            })
        );
        assert_eq!(
            tag_answer(Field::Title, "  ", &t),
            Ok(Edit::Clear {
                field: Field::Title
            })
        );
        let refused = tag_answer(Field::Date, "sometime", &t).unwrap_err();
        assert!(
            !refused.is_empty() && !refused.contains("player-"),
            "{refused}"
        );
    }

    #[test]
    fn a_missing_tag_program_is_a_sentence_and_not_a_blank_panel() {
        // THE VARIABLE POINTS AT NOTHING, which is what a machine without
        // the program looks like; the answer is a row that says so.
        let amended = sentence(&Amended::TagsTrouble(Tags::Absent), &texts());
        assert!(amended.is_some_and(|s| !s.is_empty()));
        let rows = rows(
            &Amendable {
                tags: Some(Tags::Absent),
                details: None,
            },
            &texts(),
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].does, Doable::Nothing);
    }

    /// A STAND-IN for the tag program: a script that writes its words (one
    /// per line) and its stdin beside itself, then runs `body`. Written by a
    /// `sh` of its own and not by this process: a file this process has open
    /// for writing while another test thread forks is `ETXTBSY` at the exec.
    struct StandIn {
        dir: PathBuf,
        program: PathBuf,
    }

    impl StandIn {
        fn new(name: &str, body: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("ronoctua-tags-child-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a folder for the stand-in");
            let program = dir.join("ronoctua-tags");
            let script = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/words'\n{body}\n",
                d = dir.display()
            );
            let made = std::process::Command::new("/bin/sh")
                .args(["-c", "printf '%s' \"$1\" > \"$2\" && chmod 755 \"$2\""])
                .args(["sh", &script])
                .arg(&program)
                .status()
                .expect("sh runs");
            assert!(made.success(), "the stand-in is written");
            Self { dir, program }
        }

        /// The answer it will print, as the real program prints it.
        fn answering(self, answer: &Answer) -> Self {
            let mut line = serde_json::to_vec(answer).expect("an answer serialises");
            line.push(b'\n');
            std::fs::write(self.dir.join("answer.json"), line).expect("the answer is written");
            self
        }

        fn words(&self) -> Vec<String> {
            std::fs::read_to_string(self.dir.join("words"))
                .unwrap_or_default()
                .lines()
                .map(ToOwned::to_owned)
                .collect()
        }
    }

    impl Drop for StandIn {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn a_missing_program_is_absent_and_not_a_blank() {
        let song = Path::new("/music/song.flac");
        assert_eq!(sheet_by(None, song), Tags::Absent);
        assert_eq!(
            sheet_by(Some(PathBuf::from("/nowhere/ronoctua-tags")), song),
            Tags::Absent
        );
        assert_eq!(
            write_tags_by(
                None,
                song,
                Stamp {
                    len: 0,
                    mtime_ns: 0,
                    ino: 0
                },
                &[]
            ),
            Amended::TagsTrouble(Tags::Absent)
        );
    }

    #[test]
    fn the_sheet_is_the_childs_one_line_of_json() {
        let sheet = sheet_with(vec![row(Field::Title, Some("Aquarela"), true)]);
        let child = StandIn::new("sheet", "cat \"$(dirname \"$0\")/answer.json\"").answering(
            &Answer::Sheet {
                sheet: Box::new(sheet.clone()),
            },
        );
        let song = Path::new("/music/song with spaces.flac");
        assert_eq!(
            sheet_by(Some(child.program.clone()), song),
            Tags::Sheet(Box::new(sheet))
        );
        assert_eq!(
            child.words(),
            ["sheet", "/music/song with spaces.flac"],
            "the path is ONE word, never split"
        );
    }

    #[test]
    fn a_child_that_fails_or_says_nothing_readable_has_stopped() {
        let garbage = StandIn::new("garbage", "echo this is not json");
        assert_eq!(
            sheet_by(Some(garbage.program.clone()), Path::new("/x.flac")),
            Tags::Stopped
        );
        // AN ANSWER FOLLOWED BY A FAILING STATUS is not trusted.
        let failed = StandIn::new("failed", "cat \"$(dirname \"$0\")/answer.json\"; exit 3")
            .answering(&Answer::Refused { why: Why::Gone });
        assert_eq!(
            sheet_by(Some(failed.program.clone()), Path::new("/x.flac")),
            Tags::Stopped
        );
        // AND A REFUSAL IS AN ANSWER, not a failure.
        let refused = StandIn::new("refused", "cat \"$(dirname \"$0\")/answer.json\"")
            .answering(&Answer::Refused { why: Why::Gone });
        assert_eq!(
            sheet_by(Some(refused.program.clone()), Path::new("/x.flac")),
            Tags::Refused(Why::Gone)
        );
    }

    #[test]
    fn a_child_past_its_patience_is_killed_and_stopped() {
        let slow = StandIn::new("slow", "exec sleep 30");
        let began = std::time::Instant::now();
        let answer = asked(
            Some(slow.program.clone()),
            &[OsString::from("sheet"), OsString::from("/x.flac")],
            None,
            Duration::from_millis(300),
        );
        assert_eq!(answer, Err(Tags::Stopped));
        assert!(
            began.elapsed() < Duration::from_secs(10),
            "the deadline ended it, not the child: {:?}",
            began.elapsed()
        );
    }

    #[test]
    fn a_write_hands_the_edits_on_stdin_and_the_stamp_as_a_word() {
        let stamp = Stamp {
            len: 10,
            mtime_ns: 20,
            ino: 30,
        };
        let edits = vec![
            Edit::Set {
                field: Field::Title,
                value: "Aquarela".to_owned(),
            },
            Edit::Clear {
                field: Field::Album,
            },
        ];
        let body = "cat > \"$(dirname \"$0\")/stdin\"; cat \"$(dirname \"$0\")/answer.json\"";
        let child = StandIn::new("write", body).answering(&Answer::Written { stamp });
        let song = Path::new("/music/song.flac");
        assert_eq!(
            write_tags_by(Some(child.program.clone()), song, stamp, &edits),
            Amended::Written
        );
        let words = child.words();
        assert_eq!(words[..3], ["apply", "/music/song.flac", "--stamp"]);
        assert_eq!(
            serde_json::from_str::<Stamp>(&words[3]).expect("the stamp is JSON"),
            stamp
        );
        let stdin = std::fs::read(child.dir.join("stdin")).expect("stdin was written");
        assert_eq!(
            serde_json::from_slice::<Vec<Edit>>(&stdin).expect("the edits are JSON"),
            edits
        );
        // A STALE STAMP comes back as the program's refusal, in words.
        let stale = StandIn::new("stale", body).answering(&Answer::Refused {
            why: Why::ChangedSince,
        });
        let amended = write_tags_by(Some(stale.program.clone()), song, stamp, &edits);
        assert_eq!(amended, Amended::Tags(Why::ChangedSince));
        assert!(sentence(&amended, &texts()).is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn every_detail_has_a_label_in_both_languages_and_they_differ() {
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a declared language");
            let labels: std::collections::BTreeSet<String> =
                Meta::ALL.iter().map(|m| t.t(meta_key(*m))).collect();
            assert_eq!(labels.len(), Meta::ALL.len(), "{language}");
        }
    }
}
