//! WHAT A DOOR'S ERRAND CAME TO — the step a key or a click produced, what
//! is to be carried out, and the report a job, a mount, an archive, a seal
//! or a git reading comes back with.
//!
//! # Why a module of its own
//!
//! These are the file manager's words for its work, and they are not only
//! its: the queue's door turns every job that ends into one of these
//! reports, the security centre reads them, the inspection panels step the
//! manager with them, and the archive, the seal and the git line report
//! through them. Under every one of those, so that none of them names the
//! file manager to say what happened.
//!
//! # Why words only, and a crate of their own
//!
//! Everything that reports through here stands on it — the inspection panels
//! (and through them the player and the locale), the git line, the work the
//! file manager and the security centre share — so whatever else lived beside
//! these types would be recompiled under all of them. So this holds the
//! vocabulary and nothing that does anything: the seal's own words for what it
//! hands the queue and what comes back (`SealDoing`, `SealReport`) are here,
//! and the sealing is `ronoctua_shell_work::seal`'s, above. An edit to the seal, the vault,
//! a job's words or the software centre's touches none of the panels.

use std::path::{Path, PathBuf};

use ronoctua_archive::{Member, Why};
use ronoctua_consent::keys::Secret;
use ronoctua_files::archive::Stamp;
use ronoctua_files::mode::Explanation;
use ronoctua_pq::Kind;
use ronoctua_transfers::Work;

/// Something the host has to carry out — a process, a thread, or the disk.
///
/// The view decides WHAT; it never spawns, never blocks and never writes. It is
/// the same split the memory panel uses, and the reason is the trap this
/// project already paid for: `systemd-run --scope` is synchronous, and waiting
/// inside the loop that draws freezes the footer for the life of the child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doing {
    /// Hand it to whatever opens this type.
    Open(PathBuf),
    /// Open it with the application of this id.
    OpenWith(PathBuf, String),
    /// A terminal in this directory.
    Terminal(PathBuf),
    /// A root terminal in this directory, through polkit.
    RootTerminal(PathBuf),
    /// Put this text on the clipboard.
    Copy(String),
    /// Hand this address to the browser.
    ///
    /// A `String` and not a `PathBuf`, because a URL is text with a grammar
    /// and the one thing that must not happen to it is being treated as a
    /// path. It is the only thing this manager does that leaves the machine,
    /// and the row that raises it says so before the key is pressed.
    Web(String),
    /// Move these to the trash, in the bin at this root.
    Trash(PathBuf, Vec<PathBuf>),
    /// Put these back where they came from, by their name in the trash.
    ///
    /// The bin's root travels along: the door that carries this out has no
    /// business asking the environment where the trash is a second time and
    /// possibly getting a different answer. SEVERAL bins, because the trash's
    /// listing gathers the home's and every mounted volume's, and a set
    /// chosen there can span them — each goes back through its own.
    Restore(Vec<(PathBuf, Vec<String>)>),
    /// Remove these outright.
    Delete(Vec<PathBuf>),
    /// Overwrite and remove these.
    Shred(Vec<PathBuf>),
    /// Copy or move what was cut or copied into this directory.
    Paste(PathBuf),
    /// Copy or move what was DROPPED into this directory.
    ///
    /// # Why it is not `Paste`
    ///
    /// Because `Paste` spends `View::clip`, and a drop has nothing to do with
    /// what somebody copied: routing it through the clipboard would empty it
    /// as a side effect of a gesture that never mentioned it. The two end in
    /// the same `Work::Paste` on the queue — same three arguments, same
    /// engine — and differ only in where the paths came from.
    Drop {
        /// What was carried.
        paths: Vec<PathBuf>,
        /// The directory it landed in.
        into: PathBuf,
        /// Whether the source's files go away, which is what the compositor
        /// negotiated with the modifier as a preference.
        moves: bool,
    },
    /// Something about an archive: list it to walk in, take a member out to
    /// open it, or hand the queue a job on it — `ronoctua_shell_files::files_archive`.
    Archive(crate::report::ArchiveDoing),
    /// Open this encrypted disk with this passphrase, and mount what comes
    /// out of it — then go in, as mounting does.
    Unlock {
        /// The container, by the id the disk border gave it.
        device: String,
        /// The passphrase: it goes into the call to udisks and nowhere else.
        passphrase: ronoctua_consent::keys::Secret,
        /// What the row calls it, for what is said.
        name: String,
    },
    /// Unmount what came out of this encrypted disk and lock it again,
    /// leaving the power on.
    Lock {
        /// The volume, by the id the disk border gave it.
        device: String,
        /// What the row calls it, for what is said when it is locked.
        name: String,
    },
    /// Set the permission bits, on every one of them.
    Mode(Vec<PathBuf>, u32),
    /// Set the group, on every one of them.
    Group(Vec<PathBuf>, u32),
    /// Set the owner — the one that needs the privileged unit.
    Owner(Vec<PathBuf>, u32),
    /// Set when it was modified, in seconds since the epoch. One syscall.
    Modified(PathBuf, i64),
    /// Write a song's or a film's tags, through the `ronoctua-tags` program,
    /// naming the stamp the sheet was read with.
    Tag(PathBuf, ronoctua_tags::Stamp, Vec<ronoctua_tags::Edit>),
    /// Write one of a picture's or a PDF's details.
    Meta(
        PathBuf,
        ronoctua_inspect::amend::Holder,
        ronoctua_inspect::amend::Meta,
        String,
    ),
    /// Take the place out of these files — into a copy beside each when the
    /// flag says so, over each otherwise. Asked first, always.
    RemoveLocation(Vec<PathBuf>, bool),
    /// Walk a directory and report how big it is.
    Measure(PathBuf),
    /// Check this file with VirusTotal, in the security centre's window —
    /// the door's business, like the other ones that start a process.
    VirusTotal(PathBuf),
    /// Mount this volume, by the id the disk border gave it, and go in when it
    /// comes up.
    Mount(String),
    /// Unmount it.
    ///
    /// The ID and not the mount point: the point is the thing that disappears,
    /// and a request naming it would name what is no longer there.
    ///
    /// AND WHAT ITS OWN TRASH STILL HOLDS, counted when the row was pressed:
    /// a stick unmounted with files in its trash leaves with that space still
    /// taken, and the person is told so (`files-trash-left-on-volume`) — the
    /// volume's name and the count, or `None` when the trash is empty.
    Unmount(String, Option<(String, usize)>),
    /// Run this application, by its freedesktop id.
    Launch(String),
    /// Unmount this volume and then cut the power to its drive.
    ///
    /// TWO STEPS AND IN THIS ORDER, and the host does the second only when the
    /// first came back: ejecting with a write still in flight is how a stick
    /// leaves with half a file on it. It is the same reason `Unmount` never
    /// passes `force`. It carries what `Unmount` carries, for the same reason.
    Eject(String, Option<(String, usize)>),
    /// Hand this volume to the formatting buffer.
    ///
    /// The BUFFER asks the questions and the HOST does the work — and the host
    /// re-checks that the volume is one the running system does not need, so
    /// the guard holds even if the buffer names something else.
    Format(String),
    /// Open the history of the repository at this root (`ronoctua_shell_git::git_browser`)
    /// — in the footer's region from the footer's door, in a window of its
    /// own from the window's.
    History(PathBuf),
    /// Open the `/private` vault (`true`) or lock it (`false`) — the door's
    /// business: its two leader commands (`private-open`, `private-lock`).
    Vault(bool),
    /// Open the installer stick's window on this drive (the disk border's
    /// id) — the region is the manager's, so a window, from either door; the
    /// window asks its own question, `no` first (`ronoctua_shell_stick::stick`).
    WriteInstaller(String),
    /// Seal or open with age — the queue's (`ronoctua_shell_work::seal`).
    Seal(SealDoing),
    /// Read the header of this `.age` on a thread: what it says picks the
    /// question that opens it (`ronoctua_shell_work::seal::sniffed`).
    SealSniff(PathBuf),
}

/// What a key or a click produced.
///
/// There is no `Ask`: a question the view needs answered is answered INSIDE
/// it, by [`Picker`]. It used to leave through here and be handled by the
/// footer's locator, and that was one implementation too many — the window has
/// no locator, so the same gesture would have needed a second one, and two
/// pickers is how "change the owner" comes to behave differently depending on
/// which door you opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing happened; do not redraw.
    Nothing,
    /// The state changed; redraw.
    Changed,
    /// The view closed.
    Closed,
    /// Carry this out.
    Do(Doing),
    /// Put this on the message line.
    Say(Explanation),
    /// The opened line was asked something. The DOOR answers.
    Fold(crate::fold::Ask),
    /// The clipboard's text is wanted — for the question's query when one is
    /// up, else for the filter. The DOOR reads it, off its loop
    /// (`ronoctua_shell_door::pasting`), and hands it to [`View::pasted`].
    Paste,
    /// A SECRET is to be asked, on the door's own line and masked: the
    /// footer's minibuffer, a window's `Ui::ask`. The id is the question's
    /// (`ask-<id>`, one of `files_archive::QUESTIONS`), and the answer comes
    /// back to [`View::secret_answered`].
    Secret(&'static str),
}

/// One row of the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What the answer is: a uid, a gid, an octal, an extension.
    pub id: String,
    /// What is shown.
    pub label: String,
}

/// What a background thread came back with.
///
/// Three cases and no progress: an operation on a handful of files finishes
/// before a progress bar would be legible, and one on ten thousand needs
/// cancellation rather than a number. The walk that DOES take minutes reports
/// its result, and `ronoctua_memory::scan` is where a cancellable version with
/// progress already lives if the day comes.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// It worked; read the directory again.
    Done,
    /// How big that directory turned out to be.
    Measured(String, u64),
    /// It did not work, and this is what to say.
    Failed(String),
    /// It mounted, and this is where. The path comes back because going in is
    /// what the person meant by pressing the row.
    Mounted(PathBuf),
    /// udisks said no, and this is WHICH no — plus its own words, for the one
    /// case that has nothing better to say.
    Refused(ronoctua_disks::Refusal, String),
    /// The sudo door refused, for this reason (`security-refusal-<id>`) —
    /// the bar cancelled, the password wrong, the footer not there.
    Door(ronoctua_security::Why),
    /// A disk was plugged in, unplugged, mounted or unmounted — and this is the
    /// machine as it is now.
    ///
    /// The whole reading and not a nudge: plugging in ONE stick emits four
    /// signals in a few milliseconds, and a nudge would make the loop that
    /// draws do four round trips of its own. Reading on the watching thread
    /// means the bus is never touched while a frame is being painted, and the
    /// last message wins.
    ///
    /// `Box`ed because it is by far the largest variant and a `Report` is
    /// cloned per message.
    Racked(Box<ronoctua_disks::Rack>),
    /// Somebody else touched the directory on screen, or the sidebar's file.
    ///
    /// # Why this one carries nothing, when `Racked` carries everything
    ///
    /// Because the answer lives on the other side. `Racked` is the whole
    /// reading since re-reading udisks2 costs a bus round trip; here the answer
    /// is [`View::reread`], which only the door can run — it is the one place
    /// that knows the sort, the filter and which NAME the cursor is on. So this
    /// is a nudge, and being a nudge it is idempotent: an event misattributed
    /// to a directory just left costs one wasted re-read, never a wrong screen.
    ///
    /// **It must not be answered with [`View::stale`].** That asks the
    /// directory's mtime, and a file rewritten in place does not move it — which
    /// is one of the two holes `notice` exists to close. Consulting the poll
    /// after the kernel has spoken is asking the worse oracle to overrule the
    /// better one, and it would drop exactly the news this variant carries.
    ///
    /// It rides this channel and gains no event source of its own, for the
    /// reason `ARCHITECTURE.md` already gives about `Racked`: both doors were
    /// carrying a `Report` before this existed.
    Stirred,
    /// A writer of the facts' editable rows came back — written, or refused
    /// and why (`inspection_edit::sentence` words it, for both doors). The
    /// directory is read again either way: a write moves the mtime, and the
    /// panel asks about the file afresh.
    Amended(crate::report::Amended),
    /// Something to say once a thread's work is done — the volume's trash an
    /// unmount left behind (`unmounted`). The key and its arguments; the door
    /// that holds the bundle words it.
    Said(Explanation),
    /// An archive was listed, a member taken out, or a job on one ended —
    /// answered by [`View::archive_reported`] in both doors.
    Archive(Box<crate::report::ArchiveReport>),
    /// THE GIT LINE'S READING, from the door's `git_line::Keeper` — the
    /// repository the folder on screen is in, read off the loop.
    Git(Box<crate::report::Line>),
    /// A `.age`'s header was read, or the queue refused a seal — answered
    /// by [`View::seal_reported`] in both doors.
    Seal(Box<SealReport>),
}

/// What the view hands the door for an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveDoing {
    /// List this archive, to walk into it.
    List {
        /// The archive.
        archive: PathBuf,
        /// Its password, for one whose names are behind it.
        secret: Option<Secret>,
    },
    /// Take this member out into the cache and open it.
    Open {
        /// The archive.
        archive: PathBuf,
        /// The member, by its path inside.
        member: String,
        /// Its password.
        secret: Option<Secret>,
    },
    /// Hand this work to the queue, each password written into the job's
    /// `0600` file on the way.
    Queue {
        /// The work.
        work: Box<Work>,
        /// The password its `secret` field is to carry.
        secret: Option<Secret>,
        /// The password a conversion's `from_secret` is to carry.
        from_secret: Option<Secret>,
    },
}

/// What came back from an archive's thread, or from the queue.
#[derive(Debug, Clone, PartialEq)]
pub enum ArchiveReport {
    /// The archive was listed — or why not. The flag is whether a password
    /// was given to list it.
    Listed {
        /// The archive.
        archive: PathBuf,
        /// What it was when it was listed.
        stamp: Option<Stamp>,
        /// Its members, or the refusal.
        listed: Result<(Vec<Member>, bool), Why>,
    },
    /// A member is out, at this path, and is to be opened.
    Opened(PathBuf),
    /// A member could not be taken out.
    NotOpened {
        /// The archive.
        archive: PathBuf,
        /// The member.
        member: String,
        /// Why.
        why: Why,
    },
    /// A job of the queue about an archive was refused.
    JobRefused {
        /// The work, as it was sent.
        work: Box<Work>,
        /// Why.
        why: Why,
    },
    /// The queue changed this archive (members added or taken out).
    Changed(PathBuf),
    /// Any other end of a job on an archive — done, or given up — so the view
    /// that sent it lets go of what it kept to ask again with.
    Ended(Box<Work>),
}

/// The archive a work is ABOUT — the one a password opens, and the one the
/// view that sent it recognises it by.
#[must_use]
pub fn subject_of(work: &Work) -> Option<&Path> {
    match work {
        Work::Compress { archive, .. }
        | Work::Extract { archive, .. }
        | Work::ArchiveEdit { archive, .. }
        | Work::ArchiveTest { archive, .. } => Some(archive),
        Work::ArchiveConvert { from, .. } => Some(from),
        _ => None,
    }
}

/// The sentence for an archive's refusal, naming it.
#[must_use]
pub fn refusal(why: Why, archive: &Path) -> Explanation {
    Explanation {
        key: why_key(why),
        args: vec![("name", name_of(archive))],
    }
}

/// The text key of a refusal — `files-archive-why-<id>`, written out so the
/// compiler sees every one.
#[must_use]
pub const fn why_key(why: Why) -> &'static str {
    use crate::key;
    match why {
        Why::NoTool => key::FILES_ARCHIVE_WHY_NO_TOOL,
        Why::Unsupported => key::FILES_ARCHIVE_WHY_UNSUPPORTED,
        Why::NeedsPassword => key::FILES_ARCHIVE_WHY_NEEDS_PASSWORD,
        Why::WrongPassword => key::FILES_ARCHIVE_WHY_WRONG_PASSWORD,
        Why::Corrupt => key::FILES_ARCHIVE_WHY_CORRUPT,
        Why::Exists => key::FILES_ARCHIVE_WHY_EXISTS,
        Why::Unstartable => key::FILES_ARCHIVE_WHY_UNSTARTABLE,
        Why::Failed => key::FILES_ARCHIVE_WHY_FAILED,
        Why::Stopped => key::FILES_ARCHIVE_WHY_STOPPED,
    }
}

/// A file's name, for a sentence.
pub fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// What the tag program said about one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tags {
    /// Its tags, and whether each can be written.
    Sheet(Box<ronoctua_tags::Sheet>),
    /// Why it has none to show.
    Refused(ronoctua_tags::Why),
    /// The program is not on this machine.
    Absent,
    /// It ran and did not answer — killed, too slow, or said nothing readable.
    Stopped,
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
    Tags(ronoctua_tags::Why),
    /// The tag program did not answer.
    TagsTrouble(Tags),
    /// exiftool refused, and why.
    Details(ronoctua_inspect::amend::Refused),
}

/// What a repository said, read off the loop — or why it said nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The repository's root: what the reading is about.
    pub root: PathBuf,
    /// What it said.
    pub read: Result<State, ronoctua_git::Why>,
}

/// Where HEAD is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Head {
    /// On a branch.
    Branch(String),
    /// On a commit and no branch: its short id.
    Detached(String),
    /// On a branch with no commit yet.
    Unborn(String),
}

/// One reading of a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// Where HEAD is.
    pub head: Head,
    /// Ahead and behind its upstream, when it has one.
    pub upstream: Option<(u32, u32)>,
    /// Paths changed and not staged.
    pub changed: u32,
    /// Paths staged.
    pub staged: u32,
    /// Paths git does not know.
    pub untracked: u32,
    /// Paths in conflict.
    pub conflicted: u32,
    /// The last commit's subject — data — and when it was made.
    pub last: Option<(String, i64)>,
}

/// What opens a `.age`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Opening {
    /// Its passphrase.
    Passphrase,
    /// The belongings' post-quantum key.
    Key,
}

/// How a seal is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum How {
    /// To these recipients (the person's, or someone's).
    To(Vec<String>),
    /// To this passphrase.
    Passphrase(Secret),
}

/// What a `.age` is opened with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum With {
    /// Its passphrase.
    Passphrase(Secret),
    /// The belongings' key, opened with their passphrase.
    Key(Secret),
}

/// What a view hands its door for the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealDoing {
    /// Seal each of these, the same way.
    Seal {
        /// What.
        paths: Vec<PathBuf>,
        /// How.
        how: How,
    },
    /// Open this.
    Open {
        /// What.
        path: PathBuf,
        /// With what.
        with: With,
    },
}

/// What came back to the file manager about a seal, from a thread or from
/// the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealReport {
    /// The header of `path` was read (`None`: it could not be).
    Sniffed {
        /// The `.age`.
        path: PathBuf,
        /// What it says.
        kind: Option<Kind>,
    },
    /// The queue refused a job on `from`.
    Refused {
        /// What was sealed or opened.
        from: PathBuf,
        /// What opened it, for an open.
        opening: Option<Opening>,
        /// Why.
        why: ronoctua_pq::Why,
    },
    /// A job on `from` ended with no refusal — done, stopped, or a program
    /// that died without a verdict (whose failure is said as any job's is):
    /// the view that sent it lets go of it.
    Ended {
        /// What was sealed or opened.
        from: PathBuf,
        /// It was an open.
        opens: bool,
    },
}
