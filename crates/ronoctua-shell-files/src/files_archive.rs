//! The file manager's ARCHIVES and ENCRYPTED DISKS: what the view asks the
//! door for, what comes back, and the sheet `compress` opens.
//!
//! # One view, two doors, and nothing blocking either
//!
//! Everything here is carried out OFF the loop that draws: listing an archive
//! and taking a member out of one run a program (`ronoctua-archive`, by store
//! path) on a named thread, and what comes back rides the view's own
//! [`crate::report::Report`] channel as an [`ArchiveReport`] — the path every
//! other slow answer of the manager takes. Packing, extracting, testing,
//! editing and converting go to the transfer queue, which owns them.
//!
//! # A password is asked on the door's line, and lives as a `Secret`
//!
//! The view never types one: it answers [`crate::report::Step::Secret`] with the
//! id of the question ([`PASSWORD`], [`UNLOCK`]…), and the door asks on its
//! own line — the footer's minibuffer, a window's `Ui::ask`, both masked and
//! both kept out of the log. The answer comes back to
//! [`crate::files::View::secret_answered`] as text and is held from then on as
//! a [`Secret`]: printed as nothing, wiped when dropped. It reaches a program
//! as the `0600` file `ronoctua_transfers::secrets` makes (created new, with
//! its mode on the creation), which the queue's job — or this module, after
//! an in-process run — removes. A passphrase for a disk goes into the call to
//! udisks and nowhere else.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ronoctua_archive::{Format, Level, Why};
use ronoctua_consent::keys::Secret;
use ronoctua_files::archive::{Listing, Stamp};
use ronoctua_files::mode::Explanation;
use ronoctua_transfers::Work;

use crate::report::{ArchiveDoing, ArchiveReport, name_of, subject_of};
use crate::{
    files::Errand,
    report::{Choice, Report},
};

/// The question for an archive's password — `ask-files-archive-password`.
pub const PASSWORD: &str = "files-archive-password";
/// The same, after the one given did not open it.
pub const PASSWORD_WRONG: &str = "files-archive-password-wrong";
/// The question for a NEW archive's password, on the sheet.
pub const PASSWORD_NEW: &str = "files-archive-password-new";
/// And the same password again, to be sure of it.
pub const PASSWORD_AGAIN: &str = "files-archive-password-again";
/// The question for the passphrase that unlocks a disk.
pub const UNLOCK: &str = "files-unlock";

/// Every question this module asks — what the doors route back to the view.
pub const QUESTIONS: [&str; 5] = [
    PASSWORD,
    PASSWORD_WRONG,
    PASSWORD_NEW,
    PASSWORD_AGAIN,
    UNLOCK,
];

/// The volumes the sheet offers, in bytes: none, then what a person names —
/// a mail's attachment, a CD, a gibibyte, and what a FAT32 stick takes in one
/// file (4 GiB less a byte, rounded down to MiB).
pub const SPLITS: [Option<u64>; 5] = [
    None,
    Some(100 * 1024 * 1024),
    Some(700 * 1024 * 1024),
    Some(1024 * 1024 * 1024),
    Some(4095 * 1024 * 1024),
];

/// Where the view stands inside an archive: which one, as it was listed, what
/// it holds, and the password that opened it (when it took one).
#[derive(Debug, Clone)]
pub struct Inside {
    /// The archive file.
    pub archive: PathBuf,
    /// What it was when it was listed.
    pub stamp: Option<Stamp>,
    /// What it holds.
    pub listing: Arc<Listing>,
    /// The password it was listed with, kept for taking members out.
    pub secret: Option<Secret>,
}

/// What a password the door is asking for will be used for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanting {
    /// Walking into this archive: its names are behind the password.
    Listing(PathBuf),
    /// Opening this member of this archive.
    Member {
        /// The archive.
        archive: PathBuf,
        /// The member, by its path inside.
        member: String,
    },
    /// A job of the queue that was refused for want of one, to be sent again
    /// with it — and the NEW archive's password it was sent with, which its
    /// end removed with the rest.
    Job(Box<Work>, Option<Secret>),
    /// The sheet's password, asked the first time.
    SheetNew,
    /// The sheet's password, asked again, with the first answer.
    SheetAgain(Secret),
    /// Unlocking this encrypted disk (its id), called this.
    Unlock {
        /// The container's id, as the disk border gave it.
        device: String,
        /// What the row calls it.
        name: String,
    },
}

/// The text key naming a format — `files-archive-<id>`.
#[must_use]
pub const fn format_key(format: Format) -> &'static str {
    use crate::key;
    match format {
        Format::Zip => key::FILES_ARCHIVE_ZIP,
        Format::SevenZ => key::FILES_ARCHIVE_7Z,
        Format::Tar => key::FILES_ARCHIVE_TAR,
        Format::TarGz => key::FILES_ARCHIVE_TAR_GZ,
        Format::TarXz => key::FILES_ARCHIVE_TAR_XZ,
        Format::TarZst => key::FILES_ARCHIVE_TAR_ZST,
        Format::TarBz2 => key::FILES_ARCHIVE_TAR_BZ2,
        Format::TarLz4 => key::FILES_ARCHIVE_TAR_LZ4,
        Format::Rar => key::FILES_ARCHIVE_RAR,
        Format::Iso => key::FILES_ARCHIVE_ISO,
        Format::Other => key::FILES_ARCHIVE_OTHER,
    }
}

/// The text key naming a level — `files-archive-level-<id>`.
#[must_use]
pub const fn level_key(level: Level) -> &'static str {
    use crate::key;
    match level {
        Level::Store => key::FILES_ARCHIVE_LEVEL_STORE,
        Level::Fastest => key::FILES_ARCHIVE_LEVEL_FASTEST,
        Level::Normal => key::FILES_ARCHIVE_LEVEL_NORMAL,
        Level::Maximum => key::FILES_ARCHIVE_LEVEL_MAXIMUM,
        Level::Ultra => key::FILES_ARCHIVE_LEVEL_ULTRA,
    }
}

/// A name nothing in `dir` uses for an archive: `stem.ext`, or `stem (2).ext`
/// — and, for one split into volumes, one whose first volume (`.001`) is free
/// too.
#[must_use]
pub fn free_target(dir: &Path, stem: &str, format: Format, split: bool) -> PathBuf {
    let taken = |p: &Path| {
        p.symlink_metadata().is_ok() || {
            let mut first = p.as_os_str().to_os_string();
            first.push(".001");
            split && Path::new(&first).symlink_metadata().is_ok()
        }
    };
    let ext = format.extension();
    let wanted = dir.join(format!("{stem}.{ext}"));
    if !taken(&wanted) {
        return wanted;
    }
    (2..10_000u32)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| !taken(p))
        .unwrap_or_else(|| dir.join(format!("{stem} ({}).{ext}", std::process::id())))
}

/// A folder name nothing in `dir` uses, for an archive to be extracted into:
/// `stem`, or `stem (2)`.
///
/// NEVER ONE THAT IS THERE: a FILE of that name (`report.pdf` beside
/// `report.pdf.zip`) cannot be a folder, and the work failed with nothing
/// better to say than that it did not finish; and a FOLDER of that name is
/// somebody's folder, which an extraction would mix itself into.
#[must_use]
pub fn free_folder(dir: &Path, stem: &str) -> PathBuf {
    let wanted = dir.join(stem);
    if wanted.symlink_metadata().is_err() {
        return wanted;
    }
    (2..10_000u32)
        .map(|n| dir.join(format!("{stem} ({n})")))
        .find(|p| p.symlink_metadata().is_err())
        .unwrap_or_else(|| dir.join(format!("{stem} ({})", std::process::id())))
}

/// What a sheet is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// Packing these, all from one folder.
    Compress(Vec<PathBuf>),
    /// Writing this archive again in another format.
    Convert(PathBuf),
}

/// The sheet `compress` and `convert` open: four answers and the row that
/// goes.
///
/// It is the manager's own question list (`files::Picker`) with rows that
/// change in place — `RET` or a second click on a row turns it to its next
/// value — so it is drawn, walked, pointed at and let go of exactly like every
/// other question of the manager, in both doors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// What it is for.
    pub purpose: Purpose,
    /// The format.
    pub format: Format,
    /// How hard.
    pub level: Level,
    /// The names behind the password too (7z).
    pub hide: bool,
    /// Which of [`SPLITS`].
    pub split: usize,
    /// The new archive's password.
    pub secret: Option<Secret>,
    /// For a conversion: the password the archive as it is was opened with,
    /// when the view has one.
    pub from_secret: Option<Secret>,
}

/// One row of the sheet, as it is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetRow {
    /// What pressing it turns — [`Sheet::turn`]'s id.
    pub id: &'static str,
    /// What it is.
    pub label: String,
    /// What it is set to; `None` for the row that goes.
    pub value: Option<String>,
}

/// What turning a row of the sheet asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Turned {
    /// It changed; draw it again.
    Changed,
    /// The password is to be asked, twice.
    AskPassword,
    /// The work, ready for the queue.
    Go(Box<ArchiveDoing>),
    /// The row that goes was pressed with nothing to make.
    Nothing,
}

impl Sheet {
    /// A sheet to pack `paths`.
    #[must_use]
    pub const fn compress(paths: Vec<PathBuf>) -> Self {
        Self {
            purpose: Purpose::Compress(paths),
            format: Format::Zip,
            level: Level::Normal,
            hide: false,
            split: 0,
            secret: None,
            from_secret: None,
        }
    }

    /// A sheet to write `archive` again — in 7z, or in zip when it already is
    /// one.
    #[must_use]
    pub fn convert(archive: PathBuf, from_secret: Option<Secret>) -> Self {
        let format = if Format::of_path(&archive) == Format::SevenZ {
            Format::Zip
        } else {
            Format::SevenZ
        };
        Self {
            purpose: Purpose::Convert(archive),
            format,
            level: Level::Normal,
            hide: false,
            split: 0,
            secret: None,
            from_secret,
        }
    }

    /// The archive it would make.
    #[must_use]
    pub fn target(&self) -> Option<PathBuf> {
        let split = self.volume().is_some();
        match &self.purpose {
            Purpose::Compress(paths) => {
                let first = paths.first()?;
                let dir = first.parent()?;
                let stem = if let [only] = paths.as_slice() {
                    let name = name_of(only);
                    if only.is_dir() {
                        name
                    } else {
                        ronoctua_files::archive::stem_of(&name)
                    }
                } else {
                    dir.file_name()
                        .map_or_else(|| "files".to_owned(), |n| n.to_string_lossy().into_owned())
                };
                Some(free_target(dir, &stem, self.format, split))
            }
            Purpose::Convert(archive) => {
                let dir = archive.parent()?;
                let stem = ronoctua_files::archive::stem_of(&name_of(archive));
                Some(free_target(dir, &stem, self.format, split))
            }
        }
    }

    /// The size of a volume, when the format splits and one was chosen.
    #[must_use]
    pub fn volume(&self) -> Option<u64> {
        if self.format.splits() {
            SPLITS.get(self.split).copied().flatten()
        } else {
            None
        }
    }

    /// Whether the password is on, for this format.
    fn sealed(&self) -> bool {
        self.secret.is_some() && self.format.encryptable()
    }

    /// The question's title.
    #[must_use]
    pub fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        match &self.purpose {
            Purpose::Compress(paths) => {
                let what = if let [only] = paths.as_slice() {
                    name_of(only)
                } else {
                    texts.t_with(crate::key::FILES_ITEMS, &[("n", &paths.len().to_string())])
                };
                texts.t_with(crate::key::FILES_COMPRESS_TITLE, &[("what", &what)])
            }
            Purpose::Convert(archive) => texts.t_with(
                crate::key::FILES_CONVERT_TITLE,
                &[("name", &name_of(archive))],
            ),
        }
    }

    /// Its rows: one per answer this format has — a label and its value —
    /// then the row that goes, which has no value.
    ///
    /// THE LABEL AND THE VALUE ARE TWO STRINGS, drawn by `chrome::field_pair`
    /// in the tones of a setting: glued into one sentence they were one
    /// colour, and every value started wherever its label happened to end.
    #[must_use]
    pub fn rows(&self, texts: &ronoctua_i18n::Texts) -> Vec<SheetRow> {
        use crate::key;
        let row = |id: &'static str, label: &str, value: String| SheetRow {
            id,
            label: texts.t(label),
            value: Some(value),
        };
        let mut out = vec![
            row(
                "format",
                key::FILES_SHEET_FORMAT,
                texts.t(format_key(self.format)),
            ),
            row(
                "level",
                key::FILES_SHEET_LEVEL,
                texts.t(level_key(self.level)),
            ),
        ];
        if self.format.encryptable() {
            out.push(row(
                "password",
                key::FILES_SHEET_PASSWORD,
                texts.t(if self.secret.is_some() {
                    key::FILES_SHEET_PASSWORD_SET
                } else {
                    key::FILES_SHEET_PASSWORD_NONE
                }),
            ));
        }
        if self.format.hides_names() && self.secret.is_some() {
            out.push(row(
                "hide",
                key::FILES_SHEET_HIDE,
                texts.t(if self.hide {
                    key::FILES_SHEET_YES
                } else {
                    key::FILES_SHEET_NO
                }),
            ));
        }
        if self.format.splits() {
            let value = self.volume().map_or_else(
                || texts.t(key::FILES_SHEET_SPLIT_NONE),
                |bytes| crate::sizes::size_text(bytes, texts.decimal_mark()),
            );
            out.push(row("split", key::FILES_SHEET_SPLIT, value));
        }
        let target = self.target().as_deref().map(name_of).unwrap_or_default();
        out.push(SheetRow {
            id: "go",
            label: texts.t_with(
                match self.purpose {
                    Purpose::Compress(_) => key::FILES_SHEET_GO,
                    Purpose::Convert(_) => key::FILES_SHEET_CONVERT_GO,
                },
                &[("name", &target)],
            ),
            value: None,
        });
        out
    }

    /// Its rows as the question list's choices: what typing narrows, by the
    /// label and the value together.
    #[must_use]
    pub fn choices(&self, texts: &ronoctua_i18n::Texts) -> Vec<Choice> {
        self.rows(texts)
            .into_iter()
            .map(|r| Choice {
                id: r.id.to_owned(),
                label: match r.value {
                    Some(v) => format!("{} {v}", r.label),
                    None => r.label,
                },
            })
            .collect()
    }

    /// Turns the row `id` to its next value — or goes.
    pub fn turn(&mut self, id: &str) -> Turned {
        match id {
            "format" => {
                let all = Format::creatable_ones();
                let at = all.iter().position(|f| *f == self.format).unwrap_or(0);
                self.format = all[(at + 1) % all.len()];
                Turned::Changed
            }
            "level" => {
                let at = Level::ALL
                    .iter()
                    .position(|l| *l == self.level)
                    .unwrap_or(0);
                self.level = Level::ALL[(at + 1) % Level::ALL.len()];
                Turned::Changed
            }
            // ON, IT IS ASKED; SET, IT GOES OFF. Twice, the second to be sure:
            // an archive encrypted under a password mistyped once is an
            // archive nobody opens again.
            "password" if self.secret.is_some() => {
                self.secret = None;
                self.hide = false;
                Turned::Changed
            }
            "password" => Turned::AskPassword,
            "hide" => {
                self.hide = !self.hide;
                Turned::Changed
            }
            "split" => {
                self.split = (self.split + 1) % SPLITS.len();
                Turned::Changed
            }
            "go" => self
                .work()
                .map_or(Turned::Nothing, |w| Turned::Go(Box::new(w))),
            _ => Turned::Nothing,
        }
    }

    /// The work the sheet asks for, with its passwords beside it.
    #[must_use]
    pub fn work(&self) -> Option<ArchiveDoing> {
        let target = self.target()?;
        let secret = self.sealed().then(|| self.secret.clone()).flatten();
        let hide_names = self.hide && self.format.hides_names() && secret.is_some();
        let work = match &self.purpose {
            Purpose::Compress(paths) => {
                let directory = paths.first()?.parent()?.to_path_buf();
                let names = paths.iter().map(|p| name_of(p)).collect();
                Work::Compress {
                    directory,
                    names,
                    archive: target,
                    format: Some(self.format),
                    level: self.level,
                    volume_bytes: self.volume(),
                    hide_names,
                    secret: None,
                }
            }
            Purpose::Convert(from) => Work::ArchiveConvert {
                from: from.clone(),
                from_secret: None,
                to: target,
                format: Some(self.format),
                level: self.level,
                volume_bytes: self.volume(),
                hide_names,
                secret: None,
            },
        };
        Some(ArchiveDoing::Queue {
            work: Box::new(work),
            secret,
            from_secret: self.from_secret.clone(),
        })
    }
}

/// `work` again, with `password` where the refusal said one was missing —
/// the reading side's: a conversion's source, everything else's archive.
#[must_use]
pub fn with_password(work: Work, password: Secret, new: Option<Secret>) -> ArchiveDoing {
    match work {
        Work::ArchiveConvert { .. } => ArchiveDoing::Queue {
            work: Box::new(work),
            secret: new,
            from_secret: Some(password),
        },
        other => ArchiveDoing::Queue {
            work: Box::new(other),
            secret: Some(password),
            from_secret: None,
        },
    }
}

/// A password written for one run of a program, removed when this is dropped.
struct Written(Option<PathBuf>);

impl Written {
    /// `secret`, written into the queue's private folder.
    fn of(secret: Option<&Secret>) -> Result<Self, Why> {
        match secret {
            None => Ok(Self(None)),
            Some(s) => ronoctua_transfers::secrets::keep(s.expose())
                .map(|p| Self(Some(p)))
                .map_err(|e| {
                    tracing::warn!(error = %e, "a password could not be written for a program");
                    Why::Failed
                }),
        }
    }

    fn path(&self) -> Option<&Path> {
        self.0.as_deref()
    }

    /// The file is somebody else's to remove now — the queue's job.
    fn hand_over(mut self) {
        self.0 = None;
    }
}

impl Drop for Written {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            ronoctua_transfers::secrets::forget(&p);
        }
    }
}

/// Lists `archive` — on the calling thread. What a thread of the door runs,
/// and what the photograph's camera runs in place.
#[must_use]
pub fn list_now(archive: &Path, secret: Option<&Secret>) -> ArchiveReport {
    let stamp = Stamp::of(archive);
    let listed = Written::of(secret).and_then(|written| {
        ronoctua_archive::Tools::from_env()
            .list(archive, written.path())
            .map(|members| (members, secret.is_some()))
            .map_err(|r| r.why)
    });
    ArchiveReport::Listed {
        archive: archive.to_path_buf(),
        stamp,
        listed,
    }
}

/// Takes `member` of `archive` out into the cache, or finds it there already
/// — on the calling thread.
#[must_use]
pub fn open_now(archive: &Path, member: &str, secret: Option<&Secret>) -> ArchiveReport {
    match ronoctua_files::archive::opened_root() {
        Some(root) => open_into(&root, archive, member, secret),
        None => ArchiveReport::NotOpened {
            archive: archive.to_path_buf(),
            member: member.to_owned(),
            why: Why::Failed,
        },
    }
}

/// [`open_now`], under the cache folder `root`.
///
/// EACH MEMBER IS EXTRACTED INTO A FOLDER OF ITS OWN and moved to its path:
/// extracted straight into the archive's folder, the second member of the
/// same top-level folder came out under a copy's name — the archive crate
/// never replaces what is already where it extracts — and was not where it
/// was looked for (`ronoctua_files::archive::taking_dir`).
fn open_into(root: &Path, archive: &Path, member: &str, secret: Option<&Secret>) -> ArchiveReport {
    let refused = |why| ArchiveReport::NotOpened {
        archive: archive.to_path_buf(),
        member: member.to_owned(),
        why,
    };
    let Some(stamp) = Stamp::of(archive) else {
        return refused(Why::Failed);
    };
    let dir = ronoctua_files::archive::opened_dir(root, archive, stamp);
    let Some(target) = ronoctua_files::archive::opened_member(&dir, member) else {
        return refused(Why::Failed);
    };
    // ALREADY OUT, from this archive as it is now: opened again for free.
    if target.symlink_metadata().is_ok() {
        return ArchiveReport::Opened(target);
    }
    let written = match Written::of(secret) {
        Ok(w) => w,
        Err(why) => return refused(why),
    };
    let taking = ronoctua_files::archive::taking_dir(&dir);
    let taken = ronoctua_archive::Tools::from_env().extract(
        archive,
        &taking,
        &[member.to_owned()],
        written.path(),
        &mut |_| {},
    );
    drop(written);
    let settled = taken.map(|()| ronoctua_files::archive::settle_opened(&taking, &dir, member));
    let _ = std::fs::remove_dir_all(&taking);
    match settled {
        Ok(Some(at)) => {
            // THE BOUND, after every extraction: the oldest archive's folder
            // goes first, and never the one just written.
            ronoctua_files::archive::trim_opened(
                root,
                ronoctua_files::archive::opened_most_bytes(),
                &dir,
            );
            ArchiveReport::Opened(at)
        }
        Ok(None) => refused(Why::Failed),
        Err(r) => refused(r.why),
    }
}

/// Carries out what the view asked for an archive: a thread for a program, the
/// queue for a job. What is said now is the door's to say.
#[must_use]
pub fn carry_out(doing: ArchiveDoing, sender: &crate::wake::Out<Report>) -> Errand {
    match doing {
        ArchiveDoing::List { archive, secret } => {
            let name = name_of(&archive);
            let sender = sender.clone();
            raise("archive-list", move || {
                let _ = sender.send(Report::Archive(Box::new(list_now(
                    &archive,
                    secret.as_ref(),
                ))));
            });
            Errand::Say(Explanation {
                key: crate::key::FILES_ARCHIVE_READING,
                args: vec![("name", name)],
            })
        }
        ArchiveDoing::Open {
            archive,
            member,
            secret,
        } => {
            let name = member.rsplit('/').next().unwrap_or(&member).to_owned();
            let sender = sender.clone();
            raise("archive-open", move || {
                let _ = sender.send(Report::Archive(Box::new(open_now(
                    &archive,
                    &member,
                    secret.as_ref(),
                ))));
            });
            Errand::Say(Explanation {
                key: crate::key::FILES_ARCHIVE_OPENING,
                args: vec![("name", name)],
            })
        }
        ArchiveDoing::Queue {
            work,
            secret,
            from_secret,
        } => queue(*work, secret.as_ref(), from_secret.as_ref()),
    }
}

/// Writes the passwords into the job's files and hands the work to the queue.
///
/// EACH FILE IS HELD BY A [`Written`] until the queue has the job, and only
/// then handed over — the job removes it when it ends. Every other way out of
/// here (the second file not written, the queue not up) drops the guards, and
/// the files go at once: a password written for a job nobody runs is a
/// password left in a folder until the queue next starts.
fn queue(mut work: Work, secret: Option<&Secret>, from_secret: Option<&Secret>) -> Errand {
    let refused = |work: &Work| {
        Errand::Say(Explanation {
            key: crate::key::FILES_ARCHIVE_WHY_FAILED,
            args: vec![("name", subject_of(work).map(name_of).unwrap_or_default())],
        })
    };
    let Ok(written) = Written::of(secret) else {
        return refused(&work);
    };
    let Ok(from_written) = Written::of(from_secret) else {
        return refused(&work);
    };
    let path = || written.path().map(Path::to_path_buf);
    match &mut work {
        Work::ArchiveConvert {
            secret: s,
            from_secret: f,
            ..
        } => {
            *s = path();
            *f = from_written.path().map(Path::to_path_buf);
        }
        Work::Compress { secret: s, .. }
        | Work::Extract { secret: s, .. }
        | Work::ArchiveEdit { secret: s, .. }
        | Work::ArchiveTest { secret: s, .. } => *s = path(),
        _ => {}
    }
    match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
        Ok(()) => {
            written.hand_over();
            from_written.hand_over();
            Errand::Done
        }
        Err(e) => {
            tracing::warn!(error = %e, "the transfer queue did not take the archive's job");
            Errand::Say(Explanation {
                key: crate::key::TRANSFERS_OFFLINE,
                args: Vec::new(),
            })
        }
    }
}

/// Runs `work` on a named thread.
fn raise(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(format!("files-{name}"))
        .spawn(work)
    {
        tracing::error!(error = %e, thread = name, "could not start the thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::why_key;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-files-archive-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn bundle() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    #[test]
    fn a_name_nothing_uses_is_found_and_a_first_volume_counts() {
        let dir = scratch("free");
        assert_eq!(
            free_target(&dir, "a", Format::Zip, false),
            dir.join("a.zip")
        );
        std::fs::write(dir.join("a.zip"), b"x").unwrap();
        assert_eq!(
            free_target(&dir, "a", Format::Zip, false),
            dir.join("a (2).zip")
        );
        std::fs::write(dir.join("b.7z.001"), b"x").unwrap();
        assert_eq!(
            free_target(&dir, "b", Format::SevenZ, true),
            dir.join("b (2).7z")
        );
        assert_eq!(
            free_target(&dir, "b", Format::SevenZ, false),
            dir.join("b.7z")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_extraction_goes_into_a_folder_nothing_there_uses() {
        // THE DEFECT: `report.pdf.zip` beside `report.pdf` was extracted into
        // `report.pdf` — a file — and the job failed with "did not finish".
        let dir = scratch("free-folder");
        assert_eq!(free_folder(&dir, "report.pdf"), dir.join("report.pdf"));
        std::fs::write(dir.join("report.pdf"), b"x").unwrap();
        assert_eq!(free_folder(&dir, "report.pdf"), dir.join("report.pdf (2)"));
        // AND SOMEBODY'S FOLDER is not mixed into either.
        std::fs::create_dir(dir.join("photos")).unwrap();
        std::fs::create_dir(dir.join("photos (2)")).unwrap();
        assert_eq!(free_folder(&dir, "photos"), dir.join("photos (3)"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_sheet_offers_only_what_its_format_can_do_and_turns_in_place() {
        let dir = scratch("sheet");
        let file = dir.join("notes.txt");
        std::fs::write(&file, b"x").unwrap();
        let texts = bundle();
        let mut s = Sheet::compress(vec![file.clone()]);
        let ids = |s: &Sheet| {
            s.choices(&texts)
                .into_iter()
                .map(|c| c.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(&s),
            ["format", "level", "password", "split", "go"],
            "zip"
        );
        assert_eq!(s.target(), Some(dir.join("notes.zip")));
        // 7z: a password on, the names can be hidden too.
        assert_eq!(s.turn("format"), Turned::Changed);
        assert_eq!(s.format, Format::SevenZ);
        assert_eq!(s.turn("password"), Turned::AskPassword);
        s.secret = Some(Secret::new("pw"));
        assert_eq!(
            ids(&s),
            ["format", "level", "password", "hide", "split", "go"]
        );
        assert_eq!(s.turn("hide"), Turned::Changed);
        // A tar: no password, no volumes — and a password set on the zip is
        // not carried into a tar that cannot hold one.
        s.turn("format");
        assert_eq!(s.format, Format::TarZst);
        assert_eq!(ids(&s), ["format", "level", "go"]);
        let Some(ArchiveDoing::Queue { work, secret, .. }) = s.work() else {
            panic!("the sheet made no work");
        };
        assert_eq!(secret, None, "a tar took a password");
        let Work::Compress {
            names,
            archive,
            hide_names,
            volume_bytes,
            ..
        } = *work
        else {
            panic!("not a compress");
        };
        assert_eq!(names, ["notes.txt"]);
        assert_eq!(archive, dir.join("notes.tar.zst"));
        assert!(!hide_names && volume_bytes.is_none());
        // Every level is reached, and the ring closes.
        let mut seen = Vec::new();
        for _ in 0..Level::ALL.len() {
            seen.push(s.level);
            s.turn("level");
        }
        assert_eq!(s.level, Level::Normal);
        assert_eq!(seen.len(), 5);
        // THE PASSWORD, SET, GOES OFF with its hidden names.
        let mut s = Sheet::compress(vec![file]);
        s.format = Format::SevenZ;
        s.secret = Some(Secret::new("pw"));
        s.hide = true;
        s.split = 1;
        let Some(ArchiveDoing::Queue { work, secret, .. }) = s.work() else {
            panic!();
        };
        assert!(secret.is_some());
        let Work::Compress {
            hide_names,
            volume_bytes,
            ..
        } = *work
        else {
            panic!();
        };
        assert!(hide_names);
        assert_eq!(volume_bytes, SPLITS[1]);
        assert_eq!(s.turn("password"), Turned::Changed);
        assert!(s.secret.is_none() && !s.hide);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_conversion_goes_beside_the_archive_and_a_refused_job_takes_its_password_where_it_was_missing()
     {
        let dir = scratch("convert");
        let tar = dir.join("photos.tar.gz");
        std::fs::write(&tar, b"x").unwrap();
        let s = Sheet::convert(tar.clone(), None);
        assert_eq!(s.format, Format::SevenZ);
        assert_eq!(s.target(), Some(dir.join("photos.7z")));
        let Some(ArchiveDoing::Queue { work, .. }) = s.work() else {
            panic!();
        };
        // SENT AGAIN WITH THE SOURCE'S PASSWORD, and the new archive's kept.
        let again = with_password(*work, Secret::new("old"), Some(Secret::new("new")));
        let ArchiveDoing::Queue {
            secret,
            from_secret,
            ..
        } = again
        else {
            panic!();
        };
        assert_eq!(
            from_secret.map(|s| s.expose().to_owned()).as_deref(),
            Some("old")
        );
        assert_eq!(
            secret.map(|s| s.expose().to_owned()).as_deref(),
            Some("new")
        );
        let extract = Work::Extract {
            archive: tar.clone(),
            into: dir.clone(),
            members: Vec::new(),
            secret: None,
        };
        assert_eq!(subject_of(&extract), Some(tar.as_path()));
        let ArchiveDoing::Queue {
            secret,
            from_secret,
            ..
        } = with_password(extract, Secret::new("pw"), None)
        else {
            panic!();
        };
        assert!(secret.is_some() && from_secret.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// THE REVIEW'S REPRODUCTION, through the real program: two members of
    /// one top-level folder, opened one after the other, each where the door
    /// looks for it — the second used to land under `2026 (copy 1)`.
    #[test]
    fn two_members_of_one_folder_open_one_after_the_other() {
        let tools = ronoctua_archive::Tools::from_env();
        assert!(
            tools.sevenzz.is_some() && tools.setsid.is_some(),
            "RONOCTUA_7ZZ and RONOCTUA_SETSID are needed: run inside `nix develop` or the tests derivation"
        );
        let dir = scratch("open-two");
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("2026/june")).unwrap();
        std::fs::write(src.join("2026/b.jpg"), b"bee").unwrap();
        std::fs::write(src.join("2026/june/a.jpg"), b"ay").unwrap();
        let zip = dir.join("photos.zip");
        tools
            .create(
                &[src.join("2026")],
                &zip,
                &ronoctua_archive::Options {
                    format: Format::Zip,
                    level: Level::Normal,
                    volume_bytes: None,
                    secret: None,
                    hide_names: false,
                },
                &mut |_| {},
            )
            .unwrap();
        let root = dir.join("cache");
        for (member, bytes) in [("2026/b.jpg", &b"bee"[..]), ("2026/june/a.jpg", &b"ay"[..])] {
            let ArchiveReport::Opened(at) = open_into(&root, &zip, member, None) else {
                panic!("{member} was not opened");
            };
            assert!(at.ends_with(member), "{member} landed at {}", at.display());
            assert_eq!(std::fs::read(&at).unwrap(), bytes);
        }
        let opened = ronoctua_files::archive::opened_dir(&root, &zip, Stamp::of(&zip).unwrap());
        let names: Vec<_> = std::fs::read_dir(&opened)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, ["2026"], "a copy's name, or a folder left behind");
        // AND A THIRD OPEN OF THE FIRST is found where it is, without a program.
        assert_eq!(
            open_into(&root, &zip, "2026/b.jpg", None),
            ArchiveReport::Opened(opened.join("2026/b.jpg"))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_refusal_and_format_has_a_sentence_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for why in Why::ALL {
                let said = texts.t_with(why_key(why), &[("name", "x.zip")]);
                assert!(!said.starts_with('['), "{language}: {why:?}");
            }
            for f in Format::ALL {
                assert!(
                    !texts.t(format_key(f)).starts_with('['),
                    "{language}: {f:?}"
                );
            }
            for l in Level::ALL {
                assert!(!texts.t(level_key(l)).starts_with('['), "{language}: {l:?}");
            }
            for id in QUESTIONS {
                assert!(
                    !texts.t(&format!("ask-{id}")).starts_with('['),
                    "{language}: {id}"
                );
            }
        }
    }
}
