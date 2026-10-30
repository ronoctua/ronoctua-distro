//! THE GIT BROWSER — `C-h` in the file manager, `h` for history.
//!
//! # What it shows
//!
//! Three columns, each the one to its left drilled into:
//!
//! 1. the ITEMS — Changes · History (paged) · Branches and tags · Stashes ·
//!    Remotes, one kind at a time, the kinds as clickable pieces on the
//!    column's first line; or one file's history (`C-f`);
//! 2. the DETAIL of the item under the cursor — a commit's facts, its whole
//!    message and the files it changed; a change's state; a remote's
//!    addresses;
//! 3. the DIFF of the file under the detail's cursor — the diff's roles
//!    (added, removed, the hunk's heading) as BANDS behind the lines and the
//!    file's own language as the ink (`ronoctua_syntax`) — or, on demand
//!    (`C-a`), who wrote each line of it.
//!
//! From [`THREE_FROM`](ronoctua_tokens::space::GIT_THREECOLUMNSFROM) cells on
//! the three sit side by side; narrower, and always in the footer, two of them
//! do, and the pair moves as the keyboard drills in and comes back.
//!
//! # Read-only, and what it does do
//!
//! Nothing here changes a repository. The safe actions are: fetch (`r`,
//! which never asks for anything — a remote that wants credentials is a
//! refusal that says so), copy a commit's id (`C-y`), open the version of a
//! file a commit had as a READ-ONLY copy in a bounded cache (`C-o`), and a
//! terminal at the repository's root (`C-t`).
//!
//! # Two doors, one view
//!
//! This module decides everything — what each column holds, which key does
//! what, what a point on the screen is, where each bar and band is drawn —
//! and does nothing: it never runs git, never blocks, never writes. A door
//! asks [`View::wanted`] after every key, click and answer, reads each
//! [`Need`] on a thread of its own ([`read`]) and hands the answer back
//! ([`View::heard`]). The footer's region and the window
//! (`crate::git_browser_window`) draw the same [`Drawn`] with the same pixel
//! arithmetic ([`Px`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ronoctua_git::{
    BlameLine, Change, ChangedFile, Commit, CommitDetail, Entry, EntryState, FileCommit, Oid, Ref,
    RefKind, Remote, Stash, Status, Why,
};
use ronoctua_i18n::Arg;
use ronoctua_render::layout::{cells_of, elide_end, width};
use ronoctua_tokens as tokens;

use crate::fields::{Field, Hit, Ink};

/// How many commits a page of history is.
pub const PAGE: usize = 200;

/// How close to the end of what is loaded the cursor may come before the
/// next page is asked for — so a held arrow never meets the end.
const PAGE_AHEAD: usize = 40;

/// How many commits' details are kept, how many diffs, how many blames:
/// each is a view's worth, and walking a history must not grow the process.
const KEEP_DETAILS: usize = 64;
const KEEP_TEXTS: usize = 12;
const KEEP_BLAMES: usize = 3;

/// The most of an untracked file read to show it whole.
pub const UNTRACKED_MOST_BYTES: u64 = 4 * 1024 * 1024;

/// The most of an old version copied out to be opened.
pub const VERSION_MOST_BYTES: usize = 256 * 1024 * 1024;

/// How far `C-[` and `C-]` move the diff sideways, as a share of its width.
const ACROSS_STEP_DIVISOR: u16 = 3;

/// Which kind of thing the items column lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// What the work tree has that HEAD does not.
    Changes,
    /// The commits, from HEAD.
    History,
    /// The branches, the remote branches and the tags.
    Refs,
    /// The stash.
    Stashes,
    /// The remotes.
    Remotes,
}

impl Kind {
    /// Every kind, in the order of the column's first line.
    pub const ALL: [Self; 5] = [
        Self::Changes,
        Self::History,
        Self::Refs,
        Self::Stashes,
        Self::Remotes,
    ];

    /// Its name on the column's first line.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Changes => crate::key::FILES_GIT_KIND_CHANGES,
            Self::History => crate::key::FILES_GIT_KIND_HISTORY,
            Self::Refs => crate::key::FILES_GIT_KIND_REFS,
            Self::Stashes => crate::key::FILES_GIT_KIND_STASHES,
            Self::Remotes => crate::key::FILES_GIT_KIND_REMOTES,
        }
    }

    /// What an empty list of it says.
    const fn empty(self) -> &'static str {
        match self {
            Self::Changes => crate::key::FILES_GIT_EMPTY_CHANGES,
            Self::History => crate::key::FILES_GIT_EMPTY_HISTORY,
            Self::Refs => crate::key::FILES_GIT_EMPTY_REFS,
            Self::Stashes => crate::key::FILES_GIT_EMPTY_STASHES,
            Self::Remotes => crate::key::FILES_GIT_EMPTY_REMOTES,
        }
    }
}

/// What the items column lists: a kind, or one file's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    /// One of the kinds.
    Kind(Kind),
    /// The commits that touched this path, renames followed.
    File(PathBuf),
}

/// The three columns, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Col {
    /// The items.
    Items,
    /// The detail of the item under the cursor.
    Detail,
    /// The diff, or the blame, of the file under the detail's cursor.
    Diff,
}

impl Col {
    /// Its place among the three — the offsets', and a bar's grip's.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Items => 0,
            Self::Detail => 1,
            Self::Diff => 2,
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Items => Self::Detail,
            Self::Detail | Self::Diff => Self::Diff,
        }
    }

    const fn previous(self) -> Self {
        match self {
            Self::Items | Self::Detail => Self::Items,
            Self::Diff => Self::Detail,
        }
    }
}

/// Something the view needs read from the repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Need {
    /// The work tree's state.
    Status,
    /// A page of history.
    History {
        /// Commits already listed.
        skip: usize,
        /// How many: a page — or, when the repository moved, every commit
        /// listed, so what the cursor is on is read again with the rest.
        count: usize,
    },
    /// The refs.
    Refs,
    /// The stash.
    Stashes,
    /// The remotes.
    Remotes,
    /// A page of one path's history.
    FileHistory {
        /// The path.
        path: PathBuf,
        /// Commits already listed.
        skip: usize,
        /// How many, as [`Need::History`] counts them.
        count: usize,
    },
    /// A commit, whole.
    Commit(Oid),
    /// What a commit changed in a path.
    Diff {
        /// The commit.
        commit: Oid,
        /// The path.
        path: PathBuf,
    },
    /// What the work tree changed in a path, staged and not.
    Work(PathBuf),
    /// A file git does not know, whole.
    Untracked(PathBuf),
    /// Who wrote each line of a path as the work tree has it.
    Blame(PathBuf),
}

impl Need {
    /// Whether its answer can change while the view is open. A commit and a
    /// commit's diff never do, and are never asked twice.
    const fn moves(&self) -> bool {
        !matches!(self, Self::Commit(_) | Self::Diff { .. })
    }
}

/// What reading a [`Need`] answered.
#[derive(Debug, Clone)]
pub enum Answer {
    /// The work tree's state.
    Status(Result<Status, Why>),
    /// A page of commits.
    Commits(Result<Vec<Commit>, Why>),
    /// A page of one path's history: each commit, and the path's name in it.
    FileCommits(Result<Vec<FileCommit>, Why>),
    /// The refs.
    Refs(Result<Vec<Ref>, Why>),
    /// The stash.
    Stashes(Result<Vec<Stash>, Why>),
    /// The remotes.
    Remotes(Result<Vec<Remote>, Why>),
    /// A commit, whole.
    Commit(Result<CommitDetail, Why>),
    /// A diff, read into lines.
    Text(Result<Text, Why>),
    /// A blame.
    Blame(Result<Vec<BlameLine>, Why>),
}

/// An answer, and what it answers.
#[derive(Debug, Clone)]
pub struct Heard {
    /// The view's generation when it was asked.
    pub generation: u64,
    /// What was asked.
    pub need: Need,
    /// What came back.
    pub answer: Answer,
}

/// The answer to any need outside a repository: the refusal that says so.
#[must_use]
pub fn refused_answer(need: &Need) -> Answer {
    let no = Why::NotARepo;
    match need {
        Need::Status => Answer::Status(Err(no)),
        Need::History { .. } => Answer::Commits(Err(no)),
        Need::FileHistory { .. } => Answer::FileCommits(Err(no)),
        Need::Refs => Answer::Refs(Err(no)),
        Need::Stashes => Answer::Stashes(Err(no)),
        Need::Remotes => Answer::Remotes(Err(no)),
        Need::Commit(_) => Answer::Commit(Err(no)),
        Need::Diff { .. } | Need::Work(_) | Need::Untracked(_) => Answer::Text(Err(no)),
        Need::Blame(_) => Answer::Blame(Err(no)),
    }
}

/// What one line of a diff is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// A heading of ours: what follows is what is staged.
    Staged,
    /// A heading of ours: what follows is not staged.
    Unstaged,
    /// A heading of ours: the file is not in the repository yet.
    NotTracked,
    /// A heading of ours: the file is not text.
    Binary,
    /// A heading of ours: a stat changed and the content did not.
    NoContent,
    /// A heading of ours: a symbolic link, whose content is where it points.
    Link,
    /// A heading of ours: not a file — a pipe, a socket, a device — and
    /// nothing in it is read.
    Special,
    /// git's own lines about the file: `diff --git`, `index`, `---`, `+++`.
    Meta,
    /// A hunk's heading, `@@ … @@`.
    Hunk,
    /// A line both sides have.
    Context,
    /// A line added.
    Added,
    /// A line removed.
    Removed,
    /// `\ No newline at end of file`, and anything else git says in passing.
    Note,
}

/// One line of a diff, highlighted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// What it is.
    pub kind: LineKind,
    /// Its text, the sign included — data. Tabs are spaces.
    pub text: String,
    /// The ink of its content: byte ranges of `text` and their role.
    pub ink: Vec<ronoctua_syntax::Range>,
}

/// A diff, read into lines on the thread that read it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Text {
    /// The lines.
    pub lines: Vec<DiffLine>,
    /// The widest, in cells.
    pub widest: u16,
}

/// How many spaces a tab is, in a diff — git's own.
const TAB: usize = 8;

/// A line with its tabs made spaces, to the next stop of eight.
fn untabbed(line: &str) -> String {
    if !line.contains('\t') {
        return line.to_owned();
    }
    let mut out = String::with_capacity(line.len() + TAB);
    let mut at = 0usize;
    for cluster in cells_of(line) {
        if cluster == "\t" {
            let pad = TAB - at % TAB;
            out.extend(std::iter::repeat_n(' ', pad));
            at += pad;
        } else {
            out.push_str(cluster);
            at += usize::from(width(cluster));
        }
    }
    out
}

/// The language a path's content is written in, when the table knows it.
#[must_use]
pub fn language_of(path: &Path) -> Option<ronoctua_syntax::Language> {
    ronoctua_syntax::of_mime(ronoctua_files::mime::of_path(
        path,
        ronoctua_files::Kind::File,
        false,
    ))
}

/// A unified diff, read into lines: each line's kind, and its content's
/// ink in the file's language — the old side's state for a removed line,
/// the new side's for an added one, both for a line they share, and both
/// fresh at every hunk (a hunk is not the line before it).
#[must_use]
pub fn diff_lines(diff: &str, language: Option<ronoctua_syntax::Language>) -> Vec<DiffLine> {
    let grammar = language.map(ronoctua_syntax::Language::grammar);
    let (mut old, mut new) = (
        ronoctua_syntax::State::fresh(),
        ronoctua_syntax::State::fresh(),
    );
    let mut scratch = Vec::new();
    let mut out = Vec::new();
    let mut in_hunk = false;
    for raw in diff.lines() {
        let text = untabbed(raw);
        let kind = if raw.starts_with("@@") {
            in_hunk = true;
            old = ronoctua_syntax::State::fresh();
            new = ronoctua_syntax::State::fresh();
            LineKind::Hunk
        } else if raw.starts_with("diff ") {
            in_hunk = false;
            LineKind::Meta
        } else if !in_hunk {
            LineKind::Meta
        } else if raw.starts_with('+') {
            LineKind::Added
        } else if raw.starts_with('-') {
            LineKind::Removed
        } else if raw.starts_with(' ') || raw.is_empty() {
            LineKind::Context
        } else {
            LineKind::Note
        };
        let mut ink = Vec::new();
        if let Some(g) = grammar
            && matches!(
                kind,
                LineKind::Added | LineKind::Removed | LineKind::Context
            )
            && !text.is_empty()
        {
            let body = &text[1..];
            let carried = match kind {
                LineKind::Removed => &mut old,
                _ => &mut new,
            };
            *carried = ronoctua_syntax::line_into(body, g, *carried, &mut scratch);
            ink.extend(scratch.iter().map(|r| ronoctua_syntax::Range {
                start: r.start + 1,
                end: r.end + 1,
                role: r.role,
            }));
            if kind == LineKind::Context {
                old = ronoctua_syntax::line_into(body, g, old, &mut scratch);
            }
        }
        out.push(DiffLine { kind, text, ink });
    }
    out
}

/// The lines of a text, whole, as all added — an untracked file.
fn added_whole(content: &str, language: Option<ronoctua_syntax::Language>) -> Vec<DiffLine> {
    let mut diff = String::from("@@ @@\n");
    for line in content.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    let mut lines = diff_lines(&diff, language);
    // THE HEADING WAS ONLY FOR THE READER: an untracked file has no hunk.
    lines.remove(0);
    lines
}

/// A text of lines, measured.
fn text_of(lines: Vec<DiffLine>) -> Text {
    let widest = lines.iter().map(|l| width(&l.text)).max().unwrap_or(0);
    Text { lines, widest }
}

/// A heading line of ours.
fn heading(kind: LineKind) -> DiffLine {
    DiffLine {
        kind,
        text: String::new(),
        ink: Vec::new(),
    }
}

/// Reads one need. Blocks until git ends: a thread's, never the loop's.
#[must_use]
pub fn read(repo: &ronoctua_git::Repo, need: &Need) -> Answer {
    let why = |r: ronoctua_git::Refusal| r.why;
    match need {
        Need::Status => Answer::Status(repo.status().map_err(why)),
        Need::History { skip, count } => Answer::Commits(repo.log(*skip, *count).map_err(why)),
        Need::FileHistory { path, skip, count } => {
            Answer::FileCommits(repo.file_history_named(path, *skip, *count).map_err(why))
        }
        Need::Refs => Answer::Refs(repo.refs().map_err(why)),
        Need::Stashes => Answer::Stashes(repo.stashes().map_err(why)),
        Need::Remotes => Answer::Remotes(repo.remotes().map_err(why)),
        Need::Commit(id) => Answer::Commit(repo.show(id).map_err(why)),
        Need::Diff { commit, path } => Answer::Text(
            repo.diff(commit, path)
                .map(|d| text_of(diff_lines(&d, language_of(path))))
                .map_err(why),
        ),
        Need::Work(path) => Answer::Text(work_text(repo, path)),
        Need::Untracked(path) => Answer::Text(untracked_text(&repo.root.join(path), path)),
        Need::Blame(path) => Answer::Blame(repo.blame(path).map_err(why)),
    }
}

/// What is staged and what is not, each under its heading — and a stat
/// that moved over an unchanged content says so rather than nothing.
fn work_text(repo: &ronoctua_git::Repo, path: &Path) -> Result<Text, Why> {
    let language = language_of(path);
    let staged = repo.work_diff(path, true).map_err(|r| r.why)?;
    let unstaged = repo.work_diff(path, false).map_err(|r| r.why)?;
    let mut lines = Vec::new();
    for (diff, kind) in [(staged, LineKind::Staged), (unstaged, LineKind::Unstaged)] {
        if diff.trim().is_empty() {
            continue;
        }
        lines.push(heading(kind));
        if diff.contains("\n@@") || diff.starts_with("@@") || diff.contains("Binary files") {
            lines.extend(diff_lines(&diff, language));
        } else {
            lines.push(heading(LineKind::NoContent));
        }
    }
    Ok(text_of(lines))
}

/// A file git does not know, read whole — up to a bound — as all added.
///
/// NEVER THROUGH A LINK, AND NEVER WAITING: a link is shown as git keeps
/// one — where it points, as its content — and not as whatever it points at
/// (a tree someone downloaded can carry `x -> /etc/shadow` as easily as
/// `x -> README`); what is not a file — a pipe, a socket, a device — is said
/// to be one and not read, because opening a pipe for reading waits for a
/// writer that may never come, and this column would say `Reading…` for good.
fn untracked_text(file: &Path, path: &Path) -> Result<Text, Why> {
    use std::io::Read as _;
    use std::os::unix::ffi::OsStrExt as _;
    let kind = std::fs::symlink_metadata(file)
        .map_err(|_| Why::Failed)?
        .file_type();
    let mut lines = vec![heading(LineKind::NotTracked)];
    if kind.is_symlink() {
        let to = std::fs::read_link(file).map_err(|_| Why::Failed)?;
        lines.push(heading(LineKind::Link));
        lines.extend(added_whole(
            &String::from_utf8_lossy(to.as_os_str().as_bytes()),
            None,
        ));
        return Ok(text_of(lines));
    }
    if !kind.is_file() {
        lines.push(heading(LineKind::Special));
        return Ok(text_of(lines));
    }
    // AND THE LOOK ABOVE IS NOT TRUSTED to still be true: a link swapped in
    // since is refused by `NOFOLLOW`, and a pipe opens at once (`NONBLOCK`)
    // and is then refused by what the descriptor itself is.
    let fd = rustix::fs::open(
        file,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| Why::Failed)?;
    let f = std::fs::File::from(fd);
    if !f.metadata().map_err(|_| Why::Failed)?.is_file() {
        lines.push(heading(LineKind::Special));
        return Ok(text_of(lines));
    }
    let mut bytes = Vec::new();
    f.take(UNTRACKED_MOST_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| Why::Failed)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > UNTRACKED_MOST_BYTES {
        return Err(Why::TooLarge);
    }
    if bytes.iter().take(8192).any(|b| *b == 0) {
        lines.push(heading(LineKind::Binary));
    } else {
        lines.extend(added_whole(
            &String::from_utf8_lossy(&bytes),
            language_of(path),
        ));
    }
    Ok(text_of(lines))
}

/// A kept answer, or where it stands.
#[derive(Debug, Clone, Default)]
enum Slot<T> {
    /// Not asked yet.
    #[default]
    Empty,
    /// Answered.
    Got(Result<T, Why>),
}

impl<T> Slot<T> {
    const fn got(&self) -> Option<&Result<T, Why>> {
        match self {
            Self::Empty => None,
            Self::Got(r) => Some(r),
        }
    }

    fn ok(&self) -> Option<&T> {
        self.got().and_then(|r| r.as_ref().ok())
    }
}

/// Pages of commits.
#[derive(Debug, Clone, Default)]
struct Pages {
    commits: Vec<Commit>,
    /// For a file's history, the path's name in each commit — as long as
    /// `commits`; empty for the history of HEAD.
    names: Vec<Option<PathBuf>>,
    /// Whether the last page came back short: there is nothing after it.
    done: bool,
    /// Why the first page was refused, when it was.
    refused: Option<Why>,
    /// Whether any page has come back.
    heard: bool,
    /// Whether the first page of THIS generation has come back. Until it
    /// has, the list on screen is the one from before the repository moved,
    /// kept whole, and no page follows it.
    fresh: bool,
}

impl Pages {
    /// A page arrived: the first replaces everything (the history may have
    /// moved), the next ones append — and one that does not follow on from
    /// what is listed is dropped. `count` is how many were asked for.
    fn take(
        &mut self,
        skip: usize,
        count: usize,
        page: Result<Vec<(Commit, Option<PathBuf>)>, Why>,
    ) {
        match page {
            Err(why) => {
                if skip == 0 {
                    self.refused = Some(why);
                    self.commits.clear();
                    self.names.clear();
                    self.done = true;
                    self.fresh = true;
                }
            }
            Ok(page) => {
                if skip == 0 {
                    self.commits.clear();
                    self.names.clear();
                    self.refused = None;
                    self.fresh = true;
                } else if skip != self.commits.len() || !self.fresh {
                    return;
                }
                self.done = page.len() < count;
                for (commit, name) in page {
                    self.commits.push(commit);
                    self.names.push(name);
                }
            }
        }
        self.heard = true;
    }

    /// The repository moved: the next first page reads everything listed.
    const fn stale(&mut self) {
        self.fresh = false;
    }
}

/// A map that keeps at most `most` entries, forgetting the oldest.
#[derive(Debug, Clone)]
struct Kept<K, V> {
    most: usize,
    order: Vec<K>,
    map: HashMap<K, V>,
}

impl<K: std::hash::Hash + Eq + Clone, V> Kept<K, V> {
    fn new(most: usize) -> Self {
        Self {
            most,
            order: Vec::new(),
            map: HashMap::new(),
        }
    }

    fn get(&self, k: &K) -> Option<&V> {
        self.map.get(k)
    }

    fn put(&mut self, k: K, v: V) {
        self.order.retain(|o| o != &k);
        self.order.push(k.clone());
        self.map.insert(k, v);
        while self.order.len() > self.most {
            let gone = self.order.remove(0);
            self.map.remove(&gone);
        }
    }
}

/// The item under the items column's cursor.
#[derive(Debug, Clone, Copy)]
enum Item<'a> {
    Change(&'a Entry),
    Commit(&'a Commit),
    Ref(&'a Ref),
    Stash(&'a Stash),
    Remote(&'a Remote),
}

impl<'a> Item<'a> {
    /// The commit it is, or points at.
    fn commit(self) -> Option<&'a Oid> {
        match self {
            Self::Commit(c) => Some(&c.id),
            Self::Ref(r) => Some(&r.target),
            Self::Stash(s) => Some(&s.id),
            Self::Change(_) | Self::Remote(_) => None,
        }
    }

    /// What identifies it across a re-read, so the cursor stays on it.
    fn id(self) -> String {
        match self {
            Self::Change(e) => e.path.to_string_lossy().into_owned(),
            Self::Commit(c) => c.id.0.clone(),
            Self::Ref(r) => format!("{:?}{}", r.kind, r.name),
            Self::Stash(s) => s.id.0.clone(),
            Self::Remote(r) => r.name.clone(),
        }
    }
}

/// A file the detail lists: its path, how it changed, and the commit —
/// none for a change of the work tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAt {
    /// The path, from the repository's root.
    pub path: PathBuf,
    /// How it changed.
    pub change: FileChange,
    /// The commit, and its first parent.
    pub commit: Option<(Oid, Option<Oid>)>,
}

/// How a listed file changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    /// In a commit.
    In(Change),
    /// In the work tree.
    Work(EntryState),
}

/// What a key, a click or an answer asks the door to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing changed.
    Nothing,
    /// Something changed: draw again.
    Paint,
    /// The view closed.
    Close,
    /// Fetch every remote, off the loop, and say how it went.
    Fetch,
    /// Put this text on the clipboard: a commit's id.
    Copy(String),
    /// Copy `path` as `commit` has it into the cache, read-only, and open it.
    Keep {
        /// The commit.
        commit: Oid,
        /// The path, from the repository's root.
        path: PathBuf,
    },
    /// A terminal at the repository's root.
    Terminal(PathBuf),
    /// Say this sentence (a text key).
    Say(&'static str),
}

/// What a point of the region is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A kind on the items column's first line.
    Kind(Kind),
    /// Row `n` of the items.
    Item(usize),
    /// Row `n` of the detail's files.
    File(usize),
    /// Somewhere in a column that is not a row.
    In(Col),
}

/// One line of a column, placed in the column's own cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The pieces.
    pub fields: Vec<Field>,
    /// What a click on it is — a row, a kind, or the column itself.
    pub target: Option<Target>,
    /// The band behind it, when it is one of a diff's roles.
    pub band: Option<tokens::Color>,
}

impl Line {
    fn of(fields: Vec<Field>) -> Self {
        Self {
            fields,
            target: None,
            band: None,
        }
    }

    fn blank() -> Self {
        Self::of(Vec::new())
    }
}

/// A column as drawn: where it is, its fixed lines, the visible part of its
/// body, and its scroll.
#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    /// Which column.
    pub col: Col,
    /// The rule to its left — the cell it sits in — for every column but
    /// the first.
    pub left: Option<u16>,
    /// Its first cell of text: `files.gutter` past the rule on its left, or
    /// past the region's edge.
    pub from: u16,
    /// One past its last cell of text — the bar's cell is the last of them,
    /// while it is drawn.
    pub to: u16,
    /// Its right-hand boundary: the cell the rule after it sits in,
    /// `files.edgeRight` past its text. For the last column no rule is
    /// drawn there, and it is the region's edge.
    pub edge: u16,
    /// Its lines that do not scroll, at its top.
    pub head: Vec<Line>,
    /// The body's lines on screen, from `scroll.offset`.
    pub body: Vec<Line>,
    /// The body's scroll: `top` is how many head lines are above it.
    pub scroll: ronoctua_design::Scroll,
    /// The body's sideways scroll, in cells — the diff's, when its widest
    /// line is wider than the column.
    pub across: Option<ronoctua_design::Scroll>,
    /// Whether the keyboard is in it.
    pub focused: bool,
}

impl Pane {
    /// Whether the body's vertical bar is drawn — and takes the last cell.
    #[must_use]
    pub const fn barred(&self) -> bool {
        self.scroll.cut()
    }

    /// The cells the text may use: the bar's cell is the bar's, and the
    /// air before it ([`BAR_CELLS`]).
    #[must_use]
    pub const fn text_to(&self) -> u16 {
        if self.barred() {
            self.to.saturating_sub(BAR_CELLS)
        } else {
            self.to
        }
    }

    /// Every line of it, top to bottom, as drawn.
    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.head.iter().chain(&self.body)
    }
}

/// The region as drawn: the columns on screen, left to right.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawn {
    /// The columns.
    pub panes: Vec<Pane>,
    /// How many lines the region has.
    pub room: usize,
}

impl Drawn {
    /// Line `line` of the region, every column's fields on it with their
    /// cells made the region's — what a door that draws LINES draws (the
    /// footer's region).
    #[must_use]
    pub fn line(&self, line: usize) -> Vec<Field> {
        let mut out = Vec::new();
        for p in &self.panes {
            if let Some(l) = p.lines().nth(line) {
                for f in &l.fields {
                    let mut f = f.clone();
                    f.at = f.at.saturating_add(p.from);
                    out.push(f);
                }
            }
        }
        out
    }

    /// Every line of the region.
    #[must_use]
    pub fn lines(&self) -> Vec<Vec<Field>> {
        (0..self.room).map(|n| self.line(n)).collect()
    }

    /// The column a cell is in: from the rule on its left (exclusive) to the
    /// rule after it — which is that column's — the gutter and the air
    /// included, as its band is.
    #[must_use]
    pub fn pane_at(&self, cell: u16) -> Option<&Pane> {
        self.panes
            .iter()
            .find(|p| cell <= p.edge)
            .or_else(|| self.panes.last())
    }

    /// What a point of the region is: a row, a kind, or a column.
    #[must_use]
    pub fn target_at(&self, line: usize, cell: u16) -> Option<Target> {
        let pane = self.pane_at(cell)?;
        // THE BAR'S CELL IS THE BAR'S, and the air past it is no row's: the
        // band stops where the bar begins, and a row answers only in it.
        if pane.barred() && cell + 1 >= pane.to {
            return Some(Target::In(pane.col));
        }
        let l = pane.lines().nth(line);
        // A KIND is a piece of its line, and only the piece answers.
        if let Some(l) = l
            && matches!(l.target, Some(Target::Kind(_)))
        {
            let inside = cell.saturating_sub(pane.from);
            return Some(
                crate::fields::lit_field(&l.fields, inside)
                    .and_then(|i| kind_of_hit(&l.fields[i].hit))
                    .map_or(Target::In(pane.col), Target::Kind),
            );
        }
        Some(l.and_then(|l| l.target).unwrap_or(Target::In(pane.col)))
    }

    /// Whether a click takes something somewhere along line `line` — a row,
    /// a kind, or a column the keyboard is not in — the row's answer the
    /// footer gives it (`Region`'s `Row::takes`); WHERE along it is
    /// [`hover_band`]'s, the click's own.
    #[must_use]
    pub fn takes_line(&self, line: usize) -> bool {
        self.panes
            .iter()
            .any(|p| !p.focused || p.lines().nth(line).is_some_and(|l| l.target.is_some()))
    }

    /// The pane of a column, when it is on screen.
    #[must_use]
    pub fn pane(&self, col: Col) -> Option<&Pane> {
        self.panes.iter().find(|p| p.col == col)
    }
}

/// The kind a field on the kinds' line names — carried in its `Hit::Name`.
fn kind_of_hit(hit: &Hit) -> Option<Kind> {
    match hit {
        Hit::Name(n) => Kind::ALL.get(*n).copied(),
        _ => None,
    }
}

/// Where the region is on the door's surface, in pixels — what turns the
/// view's cells and lines into the rectangles both doors paint and hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Px {
    /// Cells before the region's cell 0 (the door's margin, or the
    /// footer's lead).
    pub lead: u16,
    /// Where the region's line 0 begins.
    pub top: f32,
    /// Where the surface ends — the leftover under the last line is the
    /// last band's.
    pub bottom: f32,
    /// The surface's right edge.
    pub edge: f32,
    /// A cell's width.
    pub cell: f32,
    /// A line's height.
    pub line: f32,
}

impl Px {
    /// The left pixel of a cell of the region.
    #[must_use]
    pub fn x(&self, cell: u16) -> f32 {
        f32::from(self.lead.saturating_add(cell)) * self.cell
    }

    /// The top pixel of a line of the region.
    #[must_use]
    pub fn y(&self, line: usize) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let l = line as f32;
        self.top + l * self.line
    }

    /// A column's band, rule to rule: where its hover and its diff's bands
    /// run, stopping where its bar begins.
    #[must_use]
    pub fn band(&self, drawn: &Drawn, pane: &Pane) -> (f32, f32) {
        let last = drawn.panes.last().is_some_and(|p| p.col == pane.col);
        let left = pane.left.map_or(0.0, |r| {
            crate::fields::rule_x(r, self.lead, self.cell) + 1.0
        });
        let right = if pane.barred() {
            self.x(pane.to.saturating_sub(1))
        } else if last {
            self.edge
        } else {
            crate::fields::rule_x(pane.edge, self.lead, self.cell)
        };
        (left, (right - left).max(0.0))
    }
}

/// A bar of a column: which axis, its track (where it is grabbed) and its
/// thumb (what is painted), from one scroll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarBox {
    /// The column.
    pub col: Col,
    /// Down the column, or across its foot.
    pub across: bool,
    /// Where it is grabbed.
    pub track: crate::chrome::Rect,
    /// What is painted.
    pub thumb: Option<crate::chrome::Rect>,
    /// The scroll it moves.
    pub scroll: ronoctua_design::Scroll,
}

impl BarBox {
    /// Whether a point is on its track.
    #[must_use]
    pub fn holds(&self, x: f32, y: f32) -> bool {
        let (tx, ty, tw, th) = self.track;
        x >= tx && x < tx + tw && y >= ty && y < ty + th
    }

    /// The offset a point on the track means — the thumb's inverse.
    #[must_use]
    pub fn offset_at(&self, x: f32, y: f32, px: &Px) -> usize {
        let (tx, ty, tw, th) = self.track;
        if self.across {
            self.scroll.offset_at(x - tx, tw, px.cell)
        } else {
            self.scroll.offset_at(y - ty, th, px.line)
        }
    }
}

/// Every bar the region draws, placed — painted and grabbed from here.
#[must_use]
pub fn bars(drawn: &Drawn, px: &Px) -> Vec<BarBox> {
    let mut out = Vec::new();
    for p in &drawn.panes {
        let top = px.y(p.scroll.top);
        let rows = p.scroll.lines;
        // THE BOTTOM OF THE BODY: the surface's, unless the sideways bar
        // takes the last line.
        let body_bottom = if p.across.is_some_and(|a| a.cut()) {
            px.y(p.scroll.top + rows)
        } else {
            px.bottom
        };
        if p.barred() {
            let right = px.x(p.to);
            let (track, thumb) =
                crate::chrome::band_bar(p.scroll, right, top, body_bottom, px.cell, px.line);
            out.push(BarBox {
                col: p.col,
                across: false,
                track,
                thumb,
                scroll: p.scroll,
            });
        }
        if let Some(a) = p.across.filter(ronoctua_design::Scroll::cut) {
            let left = px.x(p.from);
            let wide = px.x(p.text_to()) - left;
            let foot = px.y(p.scroll.top + rows + 1);
            out.push(BarBox {
                col: p.col,
                across: true,
                track: (left, foot - px.cell, wide, px.cell),
                thumb: a.bar_across(left, foot, wide, px.cell),
                scroll: a,
            });
        }
    }
    out
}

/// The rules between the columns — the focused column's lit.
#[must_use]
pub fn dividers(drawn: &Drawn, px: &Px) -> Vec<ronoctua_render::quad::Quad> {
    let mut out = Vec::new();
    for (i, p) in drawn.panes.iter().enumerate() {
        if i + 1 == drawn.panes.len() {
            break;
        }
        let x = crate::fields::rule_x(p.edge, px.lead, px.cell);
        let lit = drawn.panes.get(i + 1).is_some_and(|n| n.focused);
        out.push(if lit {
            crate::chrome::divider_lit(x, px.top, px.bottom)
        } else {
            crate::chrome::divider(x, px.top, px.bottom)
        });
    }
    out
}

/// The diff's bands — an added line's and a removed line's ground — under
/// the lines that are one, in their column's band.
#[must_use]
pub fn bands(drawn: &Drawn, px: &Px) -> Vec<ronoctua_render::quad::Quad> {
    let mut out = Vec::new();
    for p in &drawn.panes {
        let (x, wide) = px.band(drawn, p);
        for (n, l) in p.lines().enumerate() {
            if let Some(color) = l.band {
                out.push(ronoctua_render::quad::Quad {
                    x,
                    y: px.y(n),
                    width: wide,
                    height: px.line,
                    color,
                });
            }
        }
    }
    out
}

/// A hover's band: what a click at the point takes — the row it is on, in
/// its column's band; a kind, as its piece; a line of a column the keyboard
/// is not in, which a click there gives the keyboard — or none where a click
/// does nothing: the column the keyboard is in, past its rows, and a bar's
/// cell, which the bar takes.
///
/// THE LIGHT IS THE CLICK ([`View::clicked`] reads the same
/// [`Drawn::target_at`]). A kind took the click and lit nothing — the door
/// lights no field here, its accent being the cursor — and so did a click
/// that moved the keyboard to another column.
#[must_use]
pub fn hover_band(
    drawn: &Drawn,
    px: &Px,
    line: usize,
    cell: u16,
    color: tokens::Color,
) -> Option<ronoctua_render::quad::Quad> {
    let pane = drawn.pane_at(cell)?;
    let (x, wide) = match drawn.target_at(line, cell)? {
        Target::Item(_) | Target::File(_) => px.band(drawn, pane),
        // A KIND LIGHTS AS ITS PIECE: the name a click there picks, by the
        // walk the click makes (`fields::lit_field`).
        Target::Kind(_) => {
            let l = pane.lines().nth(line)?;
            let f = &l.fields[crate::fields::lit_field(&l.fields, cell.saturating_sub(pane.from))?];
            let from = pane.from.saturating_add(f.at);
            let cells = ronoctua_render::layout::width(&f.text);
            (px.x(from), f32::from(cells) * px.cell)
        }
        Target::In(_) if !pane.focused && !(pane.barred() && cell + 1 >= pane.to) => {
            px.band(drawn, pane)
        }
        Target::In(_) => return None,
    };
    Some(ronoctua_render::quad::Quad {
        x,
        y: px.y(line),
        width: wide,
        height: px.line,
        color,
    })
}

/// The cells a column's vertical bar costs its text, while it is drawn: the
/// bar's own cell, and `files.edgeRight` of air before it — the air every
/// column keeps before a rule. Text that ran up to the bar read as text the
/// bar cut: a forty-digit commit id ending against it looked like one with
/// digits missing.
pub const BAR_CELLS: u16 = 1 + tokens::space::FILES_EDGERIGHT;

/// The view's own keys, as data: the hint line is generated from it, and
/// every key it answers to is here (`OF_THE_VIEW` is the table both doors
/// dispatch through).
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "files-git-down"),
    ("C-n", "files-git-down"),
    ("<up>", "files-git-up"),
    ("C-p", "files-git-up"),
    ("RET", "files-git-enter"),
    ("TAB", "files-git-next"),
    ("<right>", "files-git-next"),
    ("S-TAB", "files-git-previous"),
    ("<left>", "files-git-previous"),
    ("<next>", "files-git-page-down"),
    ("<prior>", "files-git-page-up"),
    ("C-1", "files-git-changes"),
    ("C-2", "files-git-history"),
    ("C-3", "files-git-refs"),
    ("C-4", "files-git-stashes"),
    ("C-5", "files-git-remotes"),
    ("C-f", "files-git-file-history"),
    ("C-a", "files-git-blame"),
    ("C-y", "files-git-copy"),
    ("C-o", "files-git-open-version"),
    ("r", "files-git-fetch"),
    ("C-t", "files-git-terminal"),
    ("C-[", "files-git-left"),
    ("C-]", "files-git-right"),
];

/// The id a key runs, when the table has one for it.
#[must_use]
pub fn of_the_view(key: ronoctua_keymap::Key) -> Option<&'static str> {
    let notation = ronoctua_keymap::notation(&[key]);
    OF_THE_VIEW
        .iter()
        .find(|(n, _)| *n == notation)
        .map(|(_, id)| *id)
}

/// The browser's whole state.
#[derive(Debug)]
pub struct View {
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    zone: ronoctua_files::when::Zone,
    root: PathBuf,
    /// Whether this door may lay three columns side by side (the window) or
    /// only two (the footer).
    wide: bool,
    listing: Listing,
    /// The kind a file's history returns to.
    back: Kind,
    focus: Col,
    /// The items' cursor and the detail's, as indices.
    cursor: [usize; 2],
    /// What the items' cursor is on, by identity — kept across a re-read.
    on: Option<String>,
    /// Each column's first visible body line.
    offset: [usize; 3],
    /// The diff's first visible cell.
    across: usize,
    /// Whether the diff column shows who wrote each line instead.
    blame: bool,
    /// Bumped when the repository moved: every moving answer is asked again.
    generation: u64,
    /// What was asked, and in which generation.
    asked: HashMap<Need, u64>,
    status: Slot<Status>,
    history: Pages,
    refs: Slot<Vec<Ref>>,
    stashes: Slot<Vec<Stash>>,
    remotes: Slot<Vec<Remote>>,
    file: Pages,
    details: Kept<Oid, Result<CommitDetail, Why>>,
    diffs: Kept<Need, Result<Text, Why>>,
    blames: Kept<PathBuf, Result<Vec<BlameLine>, Why>>,
    /// A fetch is under way.
    fetching: bool,
    /// The width and the room the view was last drawn at: what a key and a
    /// click arriving without them are measured against.
    drawn_at: std::cell::Cell<(u16, usize)>,
}

impl View {
    /// The browser over the repository at `root`, opened on its history.
    #[must_use]
    pub fn at(
        root: &Path,
        wide: bool,
        texts: std::rc::Rc<ronoctua_i18n::Texts>,
        zone: ronoctua_files::when::Zone,
    ) -> Self {
        Self {
            texts,
            zone,
            root: root.to_path_buf(),
            wide,
            listing: Listing::Kind(Kind::History),
            back: Kind::History,
            focus: Col::Items,
            cursor: [0; 2],
            on: None,
            offset: [0; 3],
            across: 0,
            blame: false,
            generation: 0,
            asked: HashMap::new(),
            status: Slot::Empty,
            history: Pages::default(),
            refs: Slot::Empty,
            stashes: Slot::Empty,
            remotes: Slot::Empty,
            file: Pages::default(),
            details: Kept::new(KEEP_DETAILS),
            diffs: Kept::new(KEEP_TEXTS),
            blames: Kept::new(KEEP_BLAMES),
            fetching: false,
            drawn_at: std::cell::Cell::new((80, 20)),
        }
    }

    /// The repository's root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The generation answers are stamped with.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// The language changed.
    pub fn speak(&mut self, texts: std::rc::Rc<ronoctua_i18n::Texts>) {
        self.texts = texts;
    }

    /// Which column has the keyboard.
    #[cfg(test)]
    #[must_use]
    pub const fn focus(&self) -> Col {
        self.focus
    }

    /// What the items column lists.
    #[cfg(test)]
    #[must_use]
    pub const fn listing(&self) -> &Listing {
        &self.listing
    }

    /// The repository moved on disk: every answer that can change is asked
    /// again, and what is on screen stays until its new answer arrives.
    pub fn stale(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.history.stale();
        self.file.stale();
    }

    /// Everything the view needs and has not asked for in this generation,
    /// marked asked — the door reads each on a thread.
    pub fn wanted(&mut self) -> Vec<Need> {
        let mut out = Vec::new();
        // A MOVING NEED is asked once a generation; one that never moves —
        // a commit, a commit's diff — once until it is answered, and again
        // only if the bounded cache let it go.
        let mut want = |need: Need, asked: &mut HashMap<Need, u64>, generation: u64| {
            let fresh = match asked.get(&need) {
                None => true,
                Some(g) => need.moves() && *g != generation,
            };
            if fresh {
                asked.insert(need.clone(), generation);
                out.push(need);
            }
        };
        let generation = self.generation;
        // THE CHANGES' COUNT is on the kinds' line whatever is listed, and
        // it is the cheapest reading there is.
        want(Need::Status, &mut self.asked, generation);
        match &self.listing {
            Listing::Kind(Kind::Changes) => {}
            Listing::Kind(Kind::History) => {
                for (skip, count) in next_pages(&self.history, self.cursor[0]) {
                    want(Need::History { skip, count }, &mut self.asked, generation);
                }
            }
            Listing::Kind(Kind::Refs) => want(Need::Refs, &mut self.asked, generation),
            Listing::Kind(Kind::Stashes) => want(Need::Stashes, &mut self.asked, generation),
            Listing::Kind(Kind::Remotes) => {
                want(Need::Remotes, &mut self.asked, generation);
                want(Need::Refs, &mut self.asked, generation);
            }
            Listing::File(path) => {
                for (skip, count) in next_pages(&self.file, self.cursor[0]) {
                    want(
                        Need::FileHistory {
                            path: path.clone(),
                            skip,
                            count,
                        },
                        &mut self.asked,
                        generation,
                    );
                }
            }
        }
        if let Some(id) = self.item().and_then(Item::commit).cloned()
            && self.details.get(&id).is_none()
        {
            want(Need::Commit(id), &mut self.asked, generation);
        }
        if let Some(need) = self.text_need() {
            let kept = match &need {
                Need::Blame(path) => self.blames.get(path).is_some(),
                other => self.diffs.get(other).is_some(),
            };
            if need.moves() || !kept {
                want(need, &mut self.asked, generation);
            }
        }
        out
    }

    /// What the diff column needs for the file under the detail's cursor.
    fn text_need(&self) -> Option<Need> {
        let file = self.file_at()?;
        if self.blame {
            return Some(Need::Blame(file.path));
        }
        Some(match (&file.change, &file.commit) {
            (FileChange::Work(EntryState::Untracked), _) => Need::Untracked(file.path),
            (FileChange::Work(_), _) => Need::Work(file.path),
            (FileChange::In(_), Some((commit, _))) => Need::Diff {
                commit: commit.clone(),
                path: file.path,
            },
            (FileChange::In(_), None) => return None,
        })
    }

    /// An answer arrived. `true` when what is drawn changed.
    pub fn heard(&mut self, h: Heard) -> bool {
        // A MOVING ANSWER FROM AN OLDER GENERATION is about a repository
        // that has moved since: the newer one is on its way.
        if h.need.moves() && h.generation != self.generation {
            return false;
        }
        // AN ANSWER THAT NEVER MOVES is no longer in flight: if the cache
        // lets it go, it is asked again.
        if !h.need.moves() {
            self.asked.remove(&h.need);
        }
        let was = self.item().map(Item::id);
        match (h.need, h.answer) {
            (Need::Status, Answer::Status(s)) => self.status = Slot::Got(s),
            (Need::History { skip, count }, Answer::Commits(c)) => self.history.take(
                skip,
                count,
                c.map(|page| page.into_iter().map(|c| (c, None)).collect()),
            ),
            (Need::FileHistory { path, skip, count }, Answer::FileCommits(c)) => {
                if self.listing == Listing::File(path) {
                    self.file.take(
                        skip,
                        count,
                        c.map(|page| page.into_iter().map(|f| (f.commit, f.name)).collect()),
                    );
                }
            }
            (Need::Refs, Answer::Refs(r)) => self.refs = Slot::Got(r),
            (Need::Stashes, Answer::Stashes(s)) => self.stashes = Slot::Got(s),
            (Need::Remotes, Answer::Remotes(r)) => self.remotes = Slot::Got(r),
            (Need::Commit(id), Answer::Commit(d)) => self.details.put(id, d),
            (Need::Blame(path), Answer::Blame(b)) => self.blames.put(path, b),
            (need @ (Need::Diff { .. } | Need::Work(_) | Need::Untracked(_)), Answer::Text(t)) => {
                self.diffs.put(need, t);
            }
            _ => return false,
        }
        // THE CURSOR STAYS ON WHAT IT WAS ON, by identity: a list that
        // re-read and moved under it must not move the person. What the
        // person put it on first (`on`), and only then what it happened to be
        // on — a cursor a shorter list once pushed back is not a choice.
        self.reselect(self.on.clone().or(was));
        self.clamp();
        true
    }

    /// The fetch ended: what to say.
    pub fn fetched(&mut self, done: Result<(), Why>) -> String {
        self.fetching = false;
        self.stale();
        match done {
            Ok(()) => self.texts.t(crate::key::FILES_GIT_FETCHED),
            Err(why) => self.texts.t_with(
                crate::key::FILES_GIT_FETCH_REFUSED,
                &[("why", &crate::git_line::why_said(why, &self.texts))],
            ),
        }
    }

    /// Puts the items' cursor back on the item called `id`, when the list
    /// still has it — and keeps asking for it by that name when it does not
    /// (yet): a history read again comes back whole before the cursor could
    /// have lost its commit, and a list that lost the item for good leaves
    /// the cursor where it is.
    fn reselect(&mut self, id: Option<String>) {
        let Some(id) = id else {
            return;
        };
        let at = (0..self.items_len()).find(|n| self.item_at(*n).is_some_and(|i| i.id() == id));
        if let Some(n) = at
            && n != self.cursor[0]
        {
            self.cursor[0] = n;
        }
        self.on = Some(id);
    }

    // ---- what is listed ------------------------------------------------

    /// How many items the column lists.
    fn items_len(&self) -> usize {
        match &self.listing {
            Listing::Kind(Kind::Changes) => self.status.ok().map_or(0, |s| s.entries.len()),
            Listing::Kind(Kind::History) => self.history.commits.len(),
            Listing::Kind(Kind::Refs) => self.refs.ok().map_or(0, Vec::len),
            Listing::Kind(Kind::Stashes) => self.stashes.ok().map_or(0, Vec::len),
            Listing::Kind(Kind::Remotes) => self.remotes.ok().map_or(0, Vec::len),
            Listing::File(_) => self.file.commits.len(),
        }
    }

    fn item_at(&self, n: usize) -> Option<Item<'_>> {
        Some(match &self.listing {
            Listing::Kind(Kind::Changes) => Item::Change(self.status.ok()?.entries.get(n)?),
            Listing::Kind(Kind::History) => Item::Commit(self.history.commits.get(n)?),
            Listing::Kind(Kind::Refs) => Item::Ref(self.refs.ok()?.get(n)?),
            Listing::Kind(Kind::Stashes) => Item::Stash(self.stashes.ok()?.get(n)?),
            Listing::Kind(Kind::Remotes) => Item::Remote(self.remotes.ok()?.get(n)?),
            Listing::File(_) => Item::Commit(self.file.commits.get(n)?),
        })
    }

    fn item(&self) -> Option<Item<'_>> {
        self.item_at(self.cursor[0])
    }

    /// The files the detail lists for the item under the cursor.
    fn files(&self) -> Vec<FileAt> {
        match self.item() {
            Some(Item::Change(e)) => vec![FileAt {
                path: e.path.clone(),
                change: FileChange::Work(e.state.clone()),
                commit: None,
            }],
            Some(item) => {
                let Some(id) = item.commit() else {
                    return Vec::new();
                };
                let Some(Ok(d)) = self.details.get(id) else {
                    return Vec::new();
                };
                let parent = d.commit.parents.first().cloned();
                let mut files: Vec<FileAt> = d
                    .files
                    .iter()
                    .map(|f: &ChangedFile| FileAt {
                        path: f.path.clone(),
                        change: FileChange::In(f.change.clone()),
                        commit: Some((d.commit.id.clone(), parent.clone())),
                    })
                    .collect();
                // A FILE'S HISTORY PUTS THE FILE FIRST, under whichever of its
                // names the commit knew it by — git's own answer for this
                // commit (`file_history_named`), and the followed name only
                // where git gave none: it is the diff that was asked.
                if let Listing::File(followed) = &self.listing {
                    let named = self.file.names.get(self.cursor[0]).cloned().flatten();
                    let at = named
                        .and_then(|name| files.iter().position(|f| f.path == name))
                        .or_else(|| {
                            files.iter().position(|f| {
                                &f.path == followed
                                    || matches!(&f.change, FileChange::In(Change::Renamed { from } | Change::Copied { from }) if from == followed)
                            })
                        });
                    if let Some(n) = at {
                        let f = files.remove(n);
                        files.insert(0, f);
                    }
                }
                files
            }
            None => Vec::new(),
        }
    }

    /// The file under the detail's cursor.
    fn file_at(&self) -> Option<FileAt> {
        self.files().into_iter().nth(self.cursor[1])
    }

    /// The file a key about ONE file means, wherever the keyboard is: the
    /// change under the items' cursor, or the detail's file.
    fn file_in_hand(&self) -> Option<FileAt> {
        if let Listing::File(path) = &self.listing
            && self.focus == Col::Items
        {
            return Some(FileAt {
                path: path.clone(),
                change: FileChange::In(Change::Modified),
                commit: None,
            });
        }
        self.file_at()
    }

    /// The commit a key about ONE commit means: the item's in the items
    /// column, the detail's elsewhere.
    fn commit_in_hand(&self) -> Option<Oid> {
        self.item().and_then(Item::commit).cloned()
    }

    fn clamp(&mut self) {
        let n = self.items_len();
        self.cursor[0] = self.cursor[0].min(n.saturating_sub(1));
        let f = self.files().len();
        self.cursor[1] = self.cursor[1].min(f.saturating_sub(1));
    }

    // ---- keys ------------------------------------------------------------

    /// One key. `columns` and `room` are the region's, as last drawn.
    pub fn key(&mut self, key: ronoctua_keymap::Key) -> Errand {
        if matches!(key.sym, ronoctua_keymap::Sym::Esc) {
            return self.back();
        }
        let Some(id) = of_the_view(key) else {
            return Errand::Nothing;
        };
        self.run(id)
    }

    /// `Esc`: back one level — the blame, the column, the file's history —
    /// and only then closed.
    fn back(&mut self) -> Errand {
        if self.blame {
            self.blame = false;
            return Errand::Paint;
        }
        if self.focus != Col::Items {
            self.focus = self.focus.previous();
            return Errand::Paint;
        }
        if matches!(self.listing, Listing::File(_)) {
            self.list(Listing::Kind(self.back));
            return Errand::Paint;
        }
        Errand::Close
    }

    /// Runs one id of the table.
    #[allow(clippy::too_many_lines)]
    pub fn run(&mut self, id: &str) -> Errand {
        let (columns, room) = self.drawn_at.get();
        match id {
            "files-git-down" => self.step(1, columns, room),
            "files-git-up" => self.step(-1, columns, room),
            "files-git-page-down" | "files-git-page-up" => {
                let page = i64::try_from(room.saturating_sub(2).max(1)).unwrap_or(1);
                self.step(
                    if id == "files-git-page-down" {
                        page
                    } else {
                        -page
                    },
                    columns,
                    room,
                )
            }
            "files-git-enter" => self.enter(),
            "files-git-next" => {
                self.focus = self.focus.next();
                Errand::Paint
            }
            "files-git-previous" => {
                self.focus = self.focus.previous();
                Errand::Paint
            }
            "files-git-changes" => self.pick(Kind::Changes),
            "files-git-history" => self.pick(Kind::History),
            "files-git-refs" => self.pick(Kind::Refs),
            "files-git-stashes" => self.pick(Kind::Stashes),
            "files-git-remotes" => self.pick(Kind::Remotes),
            "files-git-file-history" => match self.file_in_hand() {
                Some(f) => {
                    if let Listing::Kind(k) = self.listing {
                        self.back = k;
                    }
                    self.list(Listing::File(f.path));
                    Errand::Paint
                }
                None => Errand::Say(crate::key::FILES_GIT_NO_FILE),
            },
            "files-git-blame" => {
                if self.blame {
                    self.blame = false;
                    return Errand::Paint;
                }
                match self.file_at() {
                    Some(f) if self.root.join(&f.path).is_file() => {
                        self.blame = true;
                        self.offset[2] = 0;
                        self.across = 0;
                        Errand::Paint
                    }
                    Some(_) => Errand::Say(crate::key::FILES_GIT_BLAME_GONE),
                    None => Errand::Say(crate::key::FILES_GIT_NO_FILE),
                }
            }
            "files-git-copy" => match self.commit_in_hand() {
                Some(id) => Errand::Copy(id.0),
                None => Errand::Say(crate::key::FILES_GIT_NOTHING_TO_COPY),
            },
            "files-git-open-version" => self.keep(),
            "files-git-fetch" => {
                if self.fetching {
                    return Errand::Say(crate::key::FILES_GIT_FETCHING);
                }
                self.fetching = true;
                Errand::Fetch
            }
            "files-git-terminal" => Errand::Terminal(self.root.clone()),
            "files-git-left" | "files-git-right" => {
                let step = usize::from((columns / ACROSS_STEP_DIVISOR).max(1));
                self.across = if id == "files-git-left" {
                    self.across.saturating_sub(step)
                } else {
                    self.across.saturating_add(step)
                };
                self.clamp_across(columns, room);
                Errand::Paint
            }
            _ => Errand::Nothing,
        }
    }

    /// Lists something else: the cursors go home.
    fn list(&mut self, listing: Listing) {
        if self.listing != listing {
            self.file = Pages::default();
        }
        self.listing = listing;
        self.focus = Col::Items;
        self.cursor = [0; 2];
        self.offset = [0; 3];
        self.on = None;
        self.across = 0;
        self.blame = false;
    }

    fn pick(&mut self, kind: Kind) -> Errand {
        self.back = kind;
        self.list(Listing::Kind(kind));
        Errand::Paint
    }

    /// `RET`: one column deeper.
    fn enter(&mut self) -> Errand {
        match self.focus {
            Col::Items if self.item().is_some() => {
                self.focus = Col::Detail;
                Errand::Paint
            }
            Col::Detail if self.file_at().is_some() => {
                self.focus = Col::Diff;
                Errand::Paint
            }
            _ => Errand::Nothing,
        }
    }

    /// The version of the file in hand the commit in hand has, copied out
    /// and opened — a deleted file's as its parent had it; a change of the
    /// work tree's as HEAD has it.
    fn keep(&self) -> Errand {
        let Some(file) = self.file_at() else {
            return Errand::Say(crate::key::FILES_GIT_NO_FILE);
        };
        let commit = match (&file.change, file.commit) {
            (FileChange::In(Change::Deleted), Some((_, parent))) => parent,
            (FileChange::In(_), Some((id, _))) => Some(id),
            (FileChange::Work(EntryState::Untracked), _) => None,
            (FileChange::Work(_), _) => self.status.ok().and_then(|s| s.head.clone()),
            (FileChange::In(_), None) => None,
        };
        match commit {
            Some(commit) => Errand::Keep {
                commit,
                path: file.path,
            },
            None => Errand::Say(crate::key::FILES_GIT_NO_VERSION),
        }
    }

    /// Moves the focused column's cursor — or, in the diff, its window.
    fn step(&mut self, by: i64, columns: u16, room: usize) -> Errand {
        match self.focus {
            Col::Items | Col::Detail => {
                let i = self.focus.index();
                let len = if i == 0 {
                    self.items_len()
                } else {
                    self.files().len()
                };
                if len == 0 {
                    return Errand::Nothing;
                }
                let now = crate::fields::slide(self.cursor[i], by, len, 1);
                let now = now.min(len - 1);
                if now == self.cursor[i] {
                    return Errand::Nothing;
                }
                self.cursor[i] = now;
                if i == 0 {
                    // A NEW ITEM IS A NEW DETAIL: its files start at the top.
                    self.cursor[1] = 0;
                    self.offset[1] = 0;
                    self.on = self.item().map(Item::id);
                }
                self.offset[2] = 0;
                self.across = 0;
                self.follow(columns, room);
                Errand::Paint
            }
            Col::Diff => {
                let total = self.diff_len();
                let visible = self.visible_of(Col::Diff, columns, room);
                let now = crate::fields::slide(self.offset[2], by, total, visible);
                if now == self.offset[2] {
                    return Errand::Nothing;
                }
                self.offset[2] = now;
                Errand::Paint
            }
        }
    }

    /// Keeps the cursors on screen, a line of context past each.
    fn follow(&mut self, columns: u16, room: usize) {
        let visible = self.visible_of(Col::Items, columns, room);
        self.offset[0] = follow_within(self.offset[0], self.cursor[0], self.items_len(), visible);
        let lines = self.detail_drawn(self.layout(columns).width_of(Col::Detail), room);
        if let Some(line) = lines
            .iter()
            .position(|l| l.target == Some(Target::File(self.cursor[1])))
        {
            let visible = self.visible_of(Col::Detail, columns, room);
            self.offset[1] = follow_within(self.offset[1], line, lines.len(), visible);
        }
    }

    // ---- the pointer ---------------------------------------------------

    /// A click on a point of the region: the first selects, the second on
    /// what is already selected does what `RET` does (invariant 19).
    pub fn clicked(&mut self, line: usize, cell: u16) -> Errand {
        let (columns, room) = self.drawn_at.get();
        let drawn = self.drawn(columns, room);
        match drawn.target_at(line, cell) {
            Some(Target::Kind(k)) => self.pick(k),
            Some(Target::Item(n)) => {
                let again = self.focus == Col::Items && self.cursor[0] == n;
                self.focus = Col::Items;
                if again {
                    return self.enter();
                }
                self.cursor[0] = n;
                self.cursor[1] = 0;
                self.offset[1] = 0;
                self.offset[2] = 0;
                self.on = self.item().map(Item::id);
                Errand::Paint
            }
            Some(Target::File(n)) => {
                let again = self.focus == Col::Detail && self.cursor[1] == n;
                self.focus = Col::Detail;
                if again {
                    return self.enter();
                }
                self.cursor[1] = n;
                self.offset[2] = 0;
                self.across = 0;
                Errand::Paint
            }
            Some(Target::In(col)) => {
                if self.focus == col {
                    return Errand::Nothing;
                }
                self.focus = col;
                Errand::Paint
            }
            None => Errand::Nothing,
        }
    }

    /// The wheel over a column: its window moves, never its cursor.
    pub fn wheel(&mut self, cell: u16, by: i64) -> bool {
        let (columns, room) = self.drawn_at.get();
        let drawn = self.drawn(columns, room);
        let Some(pane) = drawn.pane_at(cell) else {
            return false;
        };
        self.scroll_to(pane.col, pane.scroll.offset, by, &drawn)
    }

    /// A column's window put at `to` — the drag of its bar.
    pub fn scroll_bar(&mut self, bar: &BarBox, to: usize) -> bool {
        if bar.across {
            let was = self.across;
            self.across = to;
            let (columns, room) = self.drawn_at.get();
            self.clamp_across(columns, room);
            return was != self.across;
        }
        let i = bar.col.index();
        let last = bar.scroll.total.saturating_sub(bar.scroll.lines);
        let now = to.min(last);
        let moved = now != self.offset[i];
        self.offset[i] = now;
        moved
    }

    fn scroll_to(&mut self, col: Col, from: usize, by: i64, drawn: &Drawn) -> bool {
        let Some(pane) = drawn.pane(col) else {
            return false;
        };
        let i = col.index();
        let now = crate::fields::slide(from, by, pane.scroll.total, pane.scroll.lines);
        let moved = now != self.offset[i];
        self.offset[i] = now;
        moved
    }

    fn clamp_across(&mut self, columns: u16, room: usize) {
        let drawn = self.drawn(columns, room);
        let most = drawn
            .pane(Col::Diff)
            .and_then(|p| p.across)
            .map_or(0, |a| a.total.saturating_sub(a.lines));
        self.across = self.across.min(most);
    }

    // ---- the layout ------------------------------------------------------

    /// Where each column is, at this width.
    fn layout(&self, columns: u16) -> Layout {
        Layout::of(columns, self.wide, self.focus)
    }

    fn visible_of(&self, col: Col, columns: u16, room: usize) -> usize {
        self.drawn(columns, room)
            .pane(col)
            .map_or(room, |p| p.scroll.lines)
    }

    fn diff_len(&self) -> usize {
        if self.blame {
            return self
                .file_at()
                .and_then(|f| self.blames.get(&f.path))
                .and_then(|b| b.as_ref().ok())
                .map_or(0, Vec::len);
        }
        self.text_need()
            .and_then(|n| self.diffs.get(&n))
            .and_then(|t| t.as_ref().ok())
            .map_or(0, |t| t.lines.len())
    }

    /// The hint line: every key of the table, `Esc` first.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::with_the_way_back(crate::chrome::hint_of(OF_THE_VIEW, texts), texts)
    }

    /// The whole region, `columns` wide and `room` lines tall: what both
    /// doors draw and what every point is resolved against.
    #[must_use]
    pub fn drawn(&self, columns: u16, room: usize) -> Drawn {
        self.drawn_at.set((columns, room));
        let layout = self.layout(columns);
        let mut panes = Vec::new();
        for place in layout.places.iter().copied() {
            panes.push(self.pane(place, room));
        }
        Drawn { panes, room }
    }

    /// One column, laid out.
    fn pane(&self, place: Place, room: usize) -> Pane {
        let Place {
            col,
            left,
            from,
            to,
            edge,
        } = place;
        let focused = self.focus == col;
        let wide = to.saturating_sub(from);
        // THE DETAIL AS DRAWN, wrapped at the width it is drawn at — counted
        // and shown from the same lines.
        let detail = (col == Col::Detail).then(|| self.detail_drawn(wide, room));
        let (head, total, across_total) = match col {
            Col::Items => (self.items_head(wide), self.items_len().max(1), 0),
            Col::Detail => (Vec::new(), detail.as_ref().map_or(0, Vec::len), 0),
            Col::Diff => {
                let (head, total, widest) = self.diff_shape();
                (head, total, widest)
            }
        };
        let rows = room.saturating_sub(head.len());
        // THE SIDEWAYS BAR TAKES THE LAST LINE, and only while it is drawn.
        let cut_across = col == Col::Diff && usize::from(across_total) > usize::from(wide);
        let lines = if cut_across {
            rows.saturating_sub(1)
        } else {
            rows
        };
        let i = col.index();
        let offset = self.offset[i].min(total.saturating_sub(lines));
        let scroll = ronoctua_design::Scroll {
            total,
            lines,
            offset,
            top: head.len(),
        };
        // THE BAR'S CELL IS THE BAR'S, and the air before it: the text is
        // laid out `BAR_CELLS` short.
        let text_wide = if scroll.cut() {
            wide.saturating_sub(BAR_CELLS)
        } else {
            wide
        };
        let across = cut_across.then(|| ronoctua_design::Scroll {
            total: usize::from(across_total),
            lines: usize::from(text_wide),
            offset: self
                .across
                .min(usize::from(across_total.saturating_sub(text_wide))),
            top: 0,
        });
        let body = match col {
            Col::Items => self.items_body(text_wide, offset, lines, focused),
            Col::Detail => detail
                .unwrap_or_default()
                .into_iter()
                .skip(offset)
                .take(lines)
                .collect(),
            Col::Diff => self.diff_body(text_wide, offset, lines, across.map_or(0, |a| a.offset)),
        };
        Pane {
            col,
            left,
            from,
            to,
            edge,
            head,
            body,
            scroll,
            across,
            focused,
        }
    }

    // ---- the items column ----------------------------------------------

    /// The items column's fixed lines: the kinds, and — for a file's
    /// history — the title that names the file under them.
    fn items_head(&self, wide: u16) -> Vec<Line> {
        let mut lines = self.kinds_lines(wide);
        lines.extend(self.file_title(wide));
        lines
    }

    /// The kinds, as pieces that wrap onto a second line when the column is
    /// narrow: CONTROLS, each a click on its kind, the one listed in the
    /// accent.
    fn kinds_lines(&self, wide: u16) -> Vec<Line> {
        let t = &self.texts;
        let mut lines: Vec<Line> = Vec::new();
        let mut fields: Vec<Field> = Vec::new();
        let mut at = 0u16;
        for (n, kind) in Kind::ALL.iter().copied().enumerate() {
            let count = match kind {
                Kind::Changes => self.status.ok().map(|s| s.entries.len()),
                Kind::History => None,
                Kind::Refs => self.refs.ok().map(Vec::len),
                Kind::Stashes => self.stashes.ok().map(Vec::len),
                Kind::Remotes => self.remotes.ok().map(Vec::len),
            };
            let name = t.t(kind.key());
            let text = match count {
                Some(c) => t.t_args(
                    crate::key::FILES_GIT_KIND_COUNT,
                    &[
                        ("kind", Arg::Text(&name)),
                        ("n", Arg::Number(i64::try_from(c).unwrap_or(i64::MAX))),
                    ],
                ),
                None => name,
            };
            let text = elide_end(&text, wide);
            let w = width(&text);
            if at > 0 && at + w > wide {
                lines.push(kinds_line(std::mem::take(&mut fields)));
                at = 0;
            }
            let here = self.listing == Listing::Kind(kind);
            fields.push(Field {
                at,
                text,
                color: if here {
                    tokens::color::STATE_ACCENT
                } else {
                    tokens::color::TEXT_SECONDARY
                },
                hit: Hit::Name(n),
                explain: None,
                ink: Ink::Grid,
            });
            at = at
                .saturating_add(w)
                .saturating_add(tokens::space::GIT_KINDSGAP);
        }
        if !fields.is_empty() {
            lines.push(kinds_line(fields));
        }
        lines
    }

    /// A file's history's title: which file — wrapped, never cut.
    fn file_title(&self, wide: u16) -> Vec<Line> {
        let Listing::File(path) = &self.listing else {
            return Vec::new();
        };
        let title = self.texts.t_with(
            crate::key::FILES_GIT_FILE_HISTORY_TITLE,
            &[("path", &path.to_string_lossy())],
        );
        note(&title, wide, tokens::color::TEXT_HEADING)
    }

    /// The items on screen, or what the column says instead of them.
    fn items_body(&self, wide: u16, offset: usize, lines: usize, focused: bool) -> Vec<Line> {
        let t = &self.texts;
        let refused = match &self.listing {
            Listing::Kind(Kind::Changes) => self.status.got().map(Result::is_err),
            Listing::Kind(Kind::History) => self.history.heard.then_some(false),
            Listing::Kind(Kind::Refs) => self.refs.got().map(Result::is_err),
            Listing::Kind(Kind::Stashes) => self.stashes.got().map(Result::is_err),
            Listing::Kind(Kind::Remotes) => self.remotes.got().map(Result::is_err),
            Listing::File(_) => self.file.heard.then_some(false),
        };
        let why = match &self.listing {
            Listing::Kind(Kind::Changes) => {
                self.status.got().and_then(|r| r.as_ref().err().copied())
            }
            Listing::Kind(Kind::History) => self.history.refused,
            Listing::Kind(Kind::Refs) => self.refs.got().and_then(|r| r.as_ref().err().copied()),
            Listing::Kind(Kind::Stashes) => {
                self.stashes.got().and_then(|r| r.as_ref().err().copied())
            }
            Listing::Kind(Kind::Remotes) => {
                self.remotes.got().and_then(|r| r.as_ref().err().copied())
            }
            Listing::File(_) => self.file.refused,
        };
        if let Some(why) = why {
            return note(
                &crate::git_line::why_said(why, t),
                wide,
                tokens::color::STATE_ERROR,
            );
        }
        if refused.is_none() {
            return note(
                &t.t(crate::key::FILES_GIT_READING),
                wide,
                tokens::color::TEXT_DIM,
            );
        }
        if self.items_len() == 0 {
            let key = match &self.listing {
                Listing::Kind(k) => k.empty(),
                Listing::File(_) => crate::key::FILES_GIT_EMPTY_FILE,
            };
            return note(&t.t(key), wide, tokens::color::TEXT_DIM);
        }
        let now = now_s();
        (offset..self.items_len())
            .take(lines)
            .filter_map(|n| {
                let item = self.item_at(n)?;
                let here = n == self.cursor[0];
                let fields = item_fields(item, wide, here, focused, now, t);
                Some(Line {
                    fields,
                    target: Some(Target::Item(n)),
                    band: None,
                })
            })
            .collect()
    }

    // ---- the detail column ---------------------------------------------

    /// The detail's lines as a column `wide` cells by `rows` lines draws
    /// them: wrapped [`BAR_CELLS`] short when they are more than it holds,
    /// because then its bar and the air before it take those cells — and a
    /// message that wraps one line more at the narrower width, counted at the
    /// wider one, would leave its last line where no scroll reaches.
    fn detail_drawn(&self, wide: u16, rows: usize) -> Vec<Line> {
        let all = self.detail_body(wide);
        if all.len() > rows && rows > 0 {
            // Narrower is never shorter, so it is still more than the column
            // holds, and the bar is still drawn.
            self.detail_body(wide.saturating_sub(BAR_CELLS))
        } else {
            all
        }
    }

    /// Everything the detail says about the item under the cursor.
    #[allow(clippy::too_many_lines)]
    fn detail_body(&self, wide: u16) -> Vec<Line> {
        let t = &self.texts;
        let Some(item) = self.item() else {
            return note(
                &t.t(crate::key::FILES_GIT_PICK_ITEM),
                wide,
                tokens::color::TEXT_DIM,
            );
        };
        // THE FACTS ARE ONE SECTION, laid out whole: every value after the
        // widest label (`chrome::section_column`), so a commit's forty-digit
        // id is read whole beside four short labels.
        let mut facts: Vec<(String, String)> = Vec::new();
        let fact = |facts: &mut Vec<(String, String)>, label: &str, value: String| {
            facts.push((t.t(label), value));
        };
        let mut out: Vec<Line> = Vec::new();
        match item {
            Item::Change(e) => {
                fact(
                    &mut facts,
                    crate::key::FILES_GIT_FACT_PATH,
                    e.path.to_string_lossy().into_owned(),
                );
                fact(
                    &mut facts,
                    crate::key::FILES_GIT_FACT_STATE,
                    state_said(&e.state, t),
                );
                if let Some(from) = renamed_from(&e.state) {
                    fact(
                        &mut facts,
                        crate::key::FILES_GIT_FACT_FROM,
                        from.to_string_lossy().into_owned(),
                    );
                }
            }
            Item::Remote(r) => {
                fact(&mut facts, crate::key::FILES_GIT_FACT_NAME, r.name.clone());
                fact(
                    &mut facts,
                    crate::key::FILES_GIT_FACT_FETCH,
                    r.fetch_url.clone(),
                );
                if let Some(push) = &r.push_url {
                    fact(&mut facts, crate::key::FILES_GIT_FACT_PUSH, push.clone());
                }
                out.extend(section(&facts, wide));
                out.push(Line::blank());
                out.push(Line::of(vec![plain(
                    0,
                    elide_end(&t.t(crate::key::FILES_GIT_REMOTE_BRANCHES), wide),
                    tokens::color::TEXT_HEADING,
                )]));
                let prefix = format!("{}/", r.name);
                let branches: Vec<&Ref> = self.refs.ok().map_or_else(Vec::new, |all| {
                    all.iter()
                        .filter(|x| x.kind == RefKind::Remote && x.name.starts_with(&prefix))
                        .collect()
                });
                if branches.is_empty() {
                    out.extend(note(
                        &t.t(crate::key::FILES_GIT_NO_REMOTE_BRANCHES),
                        wide,
                        tokens::color::TEXT_DIM,
                    ));
                }
                for b in branches {
                    out.push(Line::of(vec![
                        plain(0, short_of(&b.target.0), tokens::color::TEXT_DIM),
                        plain(
                            id_cells(),
                            elide_end(&b.name, wide.saturating_sub(id_cells())),
                            tokens::color::TEXT_SECONDARY,
                        ),
                    ]));
                }
                return out;
            }
            Item::Ref(r) => {
                fact(&mut facts, crate::key::FILES_GIT_FACT_NAME, r.name.clone());
                fact(
                    &mut facts,
                    crate::key::FILES_GIT_FACT_KIND,
                    t.t(match r.kind {
                        RefKind::Branch => crate::key::FILES_GIT_REF_BRANCH,
                        RefKind::Remote => crate::key::FILES_GIT_REF_REMOTE,
                        RefKind::Tag => crate::key::FILES_GIT_REF_TAG,
                    }),
                );
                if let Some(up) = &r.upstream {
                    fact(&mut facts, crate::key::FILES_GIT_FACT_UPSTREAM, up.clone());
                }
            }
            Item::Stash(s) => {
                fact(
                    &mut facts,
                    crate::key::FILES_GIT_FACT_STASH,
                    format!("stash@{{{}}}", s.index),
                );
            }
            Item::Commit(_) => {}
        }
        // A COMMIT, WHOLE — for everything that is one or points at one.
        let mut message: Vec<Line> = Vec::new();
        if let Some(id) = item.commit() {
            match self.details.get(id) {
                None => message.extend(note(
                    &t.t(crate::key::FILES_GIT_READING),
                    wide,
                    tokens::color::TEXT_DIM,
                )),
                Some(Err(why)) => message.extend(note(
                    &crate::git_line::why_said(*why, t),
                    wide,
                    tokens::color::STATE_ERROR,
                )),
                Some(Ok(d)) => {
                    // THE WHOLE ID WHERE IT FITS, the short one where it does
                    // not — never an id cut in the middle (`C-y` copies the
                    // whole one either way).
                    let whole = d.commit.id.0.clone();
                    let room = wide.saturating_sub(crate::chrome::value_column(0, wide));
                    fact(
                        &mut facts,
                        crate::key::FILES_GIT_FACT_COMMIT,
                        if width(&whole) <= room {
                            whole
                        } else {
                            short_of(&d.commit.id.0)
                        },
                    );
                    fact(
                        &mut facts,
                        crate::key::FILES_GIT_FACT_AUTHOR,
                        d.commit.author.clone(),
                    );
                    fact(
                        &mut facts,
                        crate::key::FILES_GIT_FACT_DATE,
                        crate::chrome::when_text(d.commit.at_s, &self.zone, true, t),
                    );
                    if !d.commit.parents.is_empty() {
                        let parents: Vec<&str> = d
                            .commit
                            .parents
                            .iter()
                            .map(|p| short_of_str(&p.0))
                            .collect();
                        fact(
                            &mut facts,
                            crate::key::FILES_GIT_FACT_PARENTS,
                            parents.join(" "),
                        );
                    }
                    // THE MESSAGE IS PROSE, and prose wraps; it is never cut.
                    message.push(Line::blank());
                    for (n, l) in d.message.lines().enumerate() {
                        for piece in crate::chrome::note_lines(l, wide.max(1)) {
                            message.push(Line::of(vec![plain(
                                0,
                                piece,
                                if n == 0 {
                                    tokens::color::TEXT_PRIMARY
                                } else {
                                    tokens::color::TEXT_SECONDARY
                                },
                            )]));
                        }
                    }
                }
            }
        }
        out.extend(section(&facts, wide));
        out.extend(message);
        // THE FILES, the one list of this column: a change's is itself.
        let files = self.files();
        if !files.is_empty() {
            out.push(Line::blank());
            out.push(Line::of(vec![plain(
                0,
                elide_end(
                    &t.t_args(
                        crate::key::FILES_GIT_FILES,
                        &[("n", Arg::Number(i64::try_from(files.len()).unwrap_or(0)))],
                    ),
                    wide,
                ),
                tokens::color::TEXT_HEADING,
            )]));
            let focused = self.focus == Col::Detail;
            for (n, f) in files.iter().enumerate() {
                let here = n == self.cursor[1];
                out.push(Line {
                    fields: file_fields(f, wide, here, focused),
                    target: Some(Target::File(n)),
                    band: None,
                });
            }
        }
        out
    }

    // ---- the diff column -------------------------------------------------

    /// The diff column's head, how many body lines it has and its widest.
    fn diff_shape(&self) -> (Vec<Line>, usize, u16) {
        let t = &self.texts;
        let Some(file) = self.file_at() else {
            return (Vec::new(), 1, 0);
        };
        let title = if self.blame {
            t.t_with(
                crate::key::FILES_GIT_BLAME_TITLE,
                &[("path", &file.path.to_string_lossy())],
            )
        } else {
            file.path.to_string_lossy().into_owned()
        };
        let head = vec![Line::of(vec![plain(0, title, tokens::color::TEXT_HEADING)])];
        if self.blame {
            let b = self.blames.get(&file.path);
            let (n, widest) = match b {
                Some(Ok(lines)) => (
                    lines.len().max(1),
                    lines
                        .iter()
                        .map(|l| blame_gutter() + width(&untabbed(&l.text)))
                        .max()
                        .unwrap_or(0),
                ),
                _ => (1, 0),
            };
            return (head, n, widest);
        }
        let text = self.text_need().and_then(|n| self.diffs.get(&n));
        let (n, widest) = match text {
            Some(Ok(text)) => (text.lines.len().max(1), text.widest),
            _ => (1, 0),
        };
        (head, n, widest)
    }

    /// The diff's lines on screen, from cell `across`.
    fn diff_body(&self, wide: u16, offset: usize, lines: usize, across: usize) -> Vec<Line> {
        let t = &self.texts;
        let Some(file) = self.file_at() else {
            let key = match self.item() {
                Some(Item::Remote(_)) => crate::key::FILES_GIT_DIFF_NO_FILES,
                _ => crate::key::FILES_GIT_PICK_FILE,
            };
            return note(&t.t(key), wide, tokens::color::TEXT_DIM);
        };
        if self.blame {
            return match self.blames.get(&file.path) {
                None => note(
                    &t.t(crate::key::FILES_GIT_READING),
                    wide,
                    tokens::color::TEXT_DIM,
                ),
                Some(Err(why)) => note(
                    &crate::git_line::why_said(*why, t),
                    wide,
                    tokens::color::STATE_ERROR,
                ),
                Some(Ok(blame)) => blame_lines(blame, &file.path, wide, offset, lines, across),
            };
        }
        match self.text_need().and_then(|n| self.diffs.get(&n)) {
            None => note(
                &t.t(crate::key::FILES_GIT_READING),
                wide,
                tokens::color::TEXT_DIM,
            ),
            Some(Err(why)) => note(
                &crate::git_line::why_said(*why, t),
                wide,
                tokens::color::STATE_ERROR,
            ),
            Some(Ok(text)) if text.lines.is_empty() => note(
                &t.t(crate::key::FILES_GIT_DIFF_NOTHING),
                wide,
                tokens::color::TEXT_DIM,
            ),
            Some(Ok(text)) => text
                .lines
                .iter()
                .skip(offset)
                .take(lines)
                .map(|l| diff_line(l, wide, across, t))
                .collect(),
        }
    }
}

/// The pages still to ask for, each `(skip, count)`: the first — every
/// commit listed, when the repository moved under a list already read, so
/// the list on screen is replaced by one at least as long and the cursor's
/// commit is in it — or, once it has come back, the next when the cursor is
/// near the end of what is listed and there may be more.
fn next_pages(pages: &Pages, cursor: usize) -> Vec<(usize, usize)> {
    let len = pages.commits.len();
    if !pages.fresh {
        return vec![(0, len.max(PAGE))];
    }
    if !pages.done && len > 0 && cursor + PAGE_AHEAD >= len {
        return vec![(len, PAGE)];
    }
    Vec::new()
}

/// Where a list has to start so `cursor` stays on screen, a line of
/// context past it — `files::follow_within`'s arithmetic, for this view.
fn follow_within(offset: usize, cursor: usize, total: usize, room: usize) -> usize {
    const MARGIN: usize = 1;
    if room == 0 {
        return 0;
    }
    let last = total.saturating_sub(room);
    let cursor = cursor.min(total.saturating_sub(1));
    let moved = if cursor < offset.saturating_add(MARGIN) {
        cursor.saturating_sub(MARGIN)
    } else if cursor + MARGIN >= offset.saturating_add(room) {
        cursor + MARGIN + 1 - room
    } else {
        offset
    };
    moved.min(last)
}

/// Where one column is, in the region's cells — see [`Pane`] for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Place {
    col: Col,
    left: Option<u16>,
    from: u16,
    to: u16,
    edge: u16,
}

/// The columns' places at a width.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Layout {
    /// Each column on screen, left to right.
    places: Vec<Place>,
}

impl Layout {
    /// THREE COLUMNS FROM `space.git.threeColumnsFrom` cells in a door that
    /// may have them; TWO otherwise — the focused one and its neighbour, the
    /// pair moving as the keyboard drills in — and ONE where two would each
    /// be narrower than `space.git.columnFloor`.
    ///
    /// THE FILE MANAGER'S TABLE AIR, because this is a table (the theme
    /// chooser's arithmetic, `theme_chooser::Layout::with`): a column's text
    /// begins `files.gutter` after its left boundary — the region's edge for
    /// the first, a rule for the others — and ends `files.edgeRight` before
    /// its right one. So every rule has air on both sides, and no age, path
    /// or subject touches it: a glyph flush against a rule reads as a glyph
    /// that was cut.
    fn of(columns: u16, wide: bool, focus: Col) -> Self {
        const RULE: u16 = 1;
        let gutter = tokens::space::FILES_GUTTER;
        let air = tokens::space::FILES_EDGERIGHT;
        let between = air + RULE + gutter;
        let share = |room: u16, part: u16| {
            u16::try_from(u32::from(room) * u32::from(part) / 100).unwrap_or(room)
        };
        // Columns of `widths` text cells, laid left to right from the edge.
        let laid = |cols: &[Col], widths: &[u16]| {
            let mut places = Vec::new();
            let mut left = None;
            let mut at = gutter;
            for (col, wide) in cols.iter().copied().zip(widths.iter().copied()) {
                let to = at + wide;
                let edge = to + air;
                places.push(Place {
                    col,
                    left,
                    from: at,
                    to,
                    edge,
                });
                left = Some(edge);
                at = edge + RULE + gutter;
            }
            Self { places }
        };
        let floor = tokens::space::GIT_COLUMNFLOOR;
        let room = |n: u16| columns.saturating_sub(gutter + between * n.saturating_sub(1) + air);
        let three = room(3);
        if wide && columns >= tokens::space::GIT_THREECOLUMNSFROM && three >= 3 * floor {
            let a = share(three, tokens::space::GIT_ITEMSPERCENT);
            let b = share(three, tokens::space::GIT_DETAILPERCENT);
            return laid(
                &[Col::Items, Col::Detail, Col::Diff],
                &[a, b, three - a - b],
            );
        }
        let two = room(2);
        if two < 2 * floor {
            return laid(&[focus], &[room(1)]);
        }
        let (left, right) = match focus {
            Col::Diff => (Col::Detail, Col::Diff),
            _ => (Col::Items, Col::Detail),
        };
        let a = share(two, tokens::space::GIT_PAIRPERCENT).max(floor);
        laid(&[left, right], &[a, two - a])
    }

    fn width_of(&self, col: Col) -> u16 {
        self.places
            .iter()
            .find(|p| p.col == col)
            .map_or(0, |p| p.to.saturating_sub(p.from))
    }
}

/// The cells the footer's region gives the browser, from the screen's:
/// NET OF BOTH MARGINS, as a window's `Ui::columns` is — the region draws a
/// field at `chrome::FIELDS_LEAD + at`, so a view laid out on the whole
/// width put its last column, and that column's bar, a cell past the
/// surface's right edge.
#[must_use]
pub const fn footer_columns(screen_columns: u16) -> u16 {
    screen_columns.saturating_sub(2 * crate::chrome::FIELDS_LEAD)
}

/// Now, in seconds since the epoch.
fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn plain(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        at,
        text,
        color,
        hit: Hit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

fn kinds_line(fields: Vec<Field>) -> Line {
    Line {
        fields,
        target: Some(Target::Kind(Kind::History)),
        band: None,
    }
}

/// A section of facts, laid out whole: every value in one column after the
/// widest label, a label that does not fit wrapped under itself.
///
/// A VALUE LONGER THAN ITS ROOM WRAPS under itself, at the value column, and
/// is never cut: an address or a path cut in the middle is one nobody can
/// read or copy.
fn section(facts: &[(String, String)], wide: u16) -> Vec<Line> {
    let pairs: Vec<(u16, u16, u16)> = facts.iter().map(|(l, v)| (0, width(l), width(v))).collect();
    let column = crate::chrome::section_column(0, wide, &pairs);
    let room = wide.saturating_sub(column).max(1);
    let (_, value_colour) = crate::chrome::Kind::Fact.tones(false);
    let mut out = Vec::new();
    for (l, v) in facts {
        let pieces = if width(v) > room {
            ronoctua_render::layout::wrap(v, room, usize::MAX)
        } else {
            vec![v.clone()]
        };
        let first = pieces.first().cloned().unwrap_or_default();
        out.extend(
            crate::chrome::aligned_pair(
                0,
                column,
                wide,
                l,
                &first,
                crate::chrome::Kind::Fact,
                false,
            )
            .into_iter()
            .map(Line::of),
        );
        for piece in pieces.into_iter().skip(1) {
            out.push(Line::of(vec![plain(column, piece, value_colour)]));
        }
    }
    out
}

/// A sentence in place of a list: wrapped, never cut.
fn note(text: &str, wide: u16, color: tokens::Color) -> Vec<Line> {
    crate::chrome::note_lines(text, wide.max(1))
        .into_iter()
        .map(|l| Line::of(vec![plain(0, l, color)]))
        .collect()
}

fn short_of(id: &str) -> String {
    crate::git_line::short(id).to_owned()
}

fn short_of_str(id: &str) -> &str {
    crate::git_line::short(id)
}

/// The cells a short id takes, and its air.
fn id_cells() -> u16 {
    u16::try_from(crate::git_line::SHORT_ID + 1).unwrap_or(8)
}

/// One item's fields, `wide` cells: what identifies it, what it says, and
/// — at the column's right edge — how long ago.
fn item_fields(
    item: Item<'_>,
    wide: u16,
    here: bool,
    focused: bool,
    now: i64,
    t: &ronoctua_i18n::Texts,
) -> Vec<Field> {
    let main = if here && focused {
        tokens::color::STATE_ACCENT
    } else if here {
        tokens::color::TEXT_HERE_READ
    } else {
        tokens::color::TEXT_PRIMARY
    };
    let mut out = Vec::new();
    let (lead, text, age): (Vec<(String, tokens::Color)>, String, Option<i64>) = match item {
        Item::Change(e) => (
            vec![(state_letters(&e.state), state_tone(&e.state))],
            e.path.to_string_lossy().into_owned(),
            None,
        ),
        Item::Commit(c) => (
            vec![(short_of(&c.id.0), tokens::color::TEXT_DIM)],
            c.subject.clone(),
            Some(c.at_s),
        ),
        Item::Ref(r) => (
            vec![(
                t.t(match r.kind {
                    RefKind::Branch => crate::key::FILES_GIT_MARK_BRANCH,
                    RefKind::Remote => crate::key::FILES_GIT_MARK_REMOTE,
                    RefKind::Tag => crate::key::FILES_GIT_MARK_TAG,
                }),
                tokens::color::TEXT_DIM,
            )],
            r.name.clone(),
            Some(r.at_s),
        ),
        Item::Stash(s) => (
            vec![(format!("{{{}}}", s.index), tokens::color::TEXT_DIM)],
            s.subject.clone(),
            None,
        ),
        Item::Remote(r) => (Vec::new(), format!("{}  {}", r.name, r.fetch_url), None),
    };
    // THE LEAD IS A COLUMN: an id's cells, or the widest of the three kinds
    // of ref, so the names under it start in one place.
    let lead_cells = match item {
        Item::Commit(_) => id_cells().saturating_sub(1),
        Item::Ref(_) => [
            crate::key::FILES_GIT_MARK_BRANCH,
            crate::key::FILES_GIT_MARK_REMOTE,
            crate::key::FILES_GIT_MARK_TAG,
        ]
        .iter()
        .map(|k| width(&t.t(k)))
        .max()
        .unwrap_or(0),
        _ => 0,
    };
    let mut at = 0u16;
    for (text, color) in lead {
        let text = elide_end(&text, wide.saturating_sub(at));
        let w = width(&text).max(lead_cells);
        out.push(plain(at, text, color));
        at = at.saturating_add(w).saturating_add(1);
    }
    let age = age.map(|a| crate::git_line::age(a, now, t));
    let age_w = age.as_deref().map_or(0, width);
    // THE AGE IS COUNTED FROM THE RIGHT EDGE, and gives way first: kept
    // while what it says still leaves the subject `space.git.subjectFloor`.
    let room = wide.saturating_sub(at);
    let (room, age) = if age_w > 0 && room > age_w + tokens::space::GIT_SUBJECTFLOOR {
        (room - age_w - 1, age)
    } else {
        (room, None)
    };
    out.push(plain(at, elide_end(&text, room), main));
    if let Some(age) = age {
        out.push(plain(
            wide.saturating_sub(age_w),
            age,
            tokens::color::TEXT_DIM,
        ));
    }
    out
}

/// A file of the detail: its letter, and its path — `new ← old` for a
/// rename.
fn file_fields(f: &FileAt, wide: u16, here: bool, focused: bool) -> Vec<Field> {
    let (letter, tone) = match &f.change {
        FileChange::In(c) => (change_letter(c).to_owned(), change_tone(c)),
        FileChange::Work(s) => (state_letters(s), state_tone(s)),
    };
    let path = match &f.change {
        FileChange::In(Change::Renamed { from } | Change::Copied { from }) => format!(
            "{} {} {}",
            f.path.to_string_lossy(),
            tokens::mark::ARROW_LEFT,
            from.to_string_lossy()
        ),
        _ => f.path.to_string_lossy().into_owned(),
    };
    let main = if here && focused {
        tokens::color::STATE_ACCENT
    } else if here {
        tokens::color::TEXT_HERE_READ
    } else {
        tokens::color::TEXT_PRIMARY
    };
    let lw = width(&letter).max(2);
    vec![
        plain(0, letter, tone),
        plain(lw + 1, elide_end(&path, wide.saturating_sub(lw + 1)), main),
    ]
}

/// A change in a commit, as git's letter.
const fn change_letter(c: &Change) -> &'static str {
    match c {
        Change::Added => "A",
        Change::Modified => "M",
        Change::Deleted => "D",
        Change::Renamed { .. } => "R",
        Change::Copied { .. } => "C",
        Change::TypeChanged => "T",
        Change::Unmerged => "U",
    }
}

const fn change_tone(c: &Change) -> tokens::Color {
    match c {
        Change::Added | Change::Copied { .. } => tokens::color::STATE_SUCCESS,
        Change::Deleted => tokens::color::STATE_ERROR,
        Change::Unmerged => tokens::color::STATE_ERROR,
        Change::Modified | Change::Renamed { .. } | Change::TypeChanged => {
            tokens::color::STATE_WARNING
        }
    }
}

/// A work-tree entry as git's two letters — the staged side, then the
/// work tree's — `??` untracked and `UU` in conflict.
fn state_letters(s: &EntryState) -> String {
    match s {
        EntryState::Untracked => "??".to_owned(),
        EntryState::Conflicted => "UU".to_owned(),
        EntryState::Tracked { staged, unstaged } => format!(
            "{}{}",
            staged.as_ref().map_or(" ", change_letter),
            unstaged.as_ref().map_or(" ", change_letter)
        ),
    }
}

const fn state_tone(s: &EntryState) -> tokens::Color {
    match s {
        EntryState::Untracked => tokens::color::TEXT_SECONDARY,
        EntryState::Conflicted => tokens::color::STATE_ERROR,
        EntryState::Tracked {
            staged: Some(_),
            unstaged: None,
        } => tokens::color::STATE_SUCCESS,
        EntryState::Tracked { .. } => tokens::color::STATE_WARNING,
    }
}

/// A change's state, in words: what is staged and what is not.
fn state_said(s: &EntryState, t: &ronoctua_i18n::Texts) -> String {
    let what = |c: &Change| {
        t.t(match c {
            Change::Added => crate::key::FILES_GIT_CHANGE_ADDED,
            Change::Modified => crate::key::FILES_GIT_CHANGE_MODIFIED,
            Change::Deleted => crate::key::FILES_GIT_CHANGE_DELETED,
            Change::Renamed { .. } => crate::key::FILES_GIT_CHANGE_RENAMED,
            Change::Copied { .. } => crate::key::FILES_GIT_CHANGE_COPIED,
            Change::TypeChanged => crate::key::FILES_GIT_CHANGE_TYPE,
            Change::Unmerged => crate::key::FILES_GIT_CHANGE_UNMERGED,
        })
    };
    match s {
        EntryState::Untracked => t.t(crate::key::FILES_GIT_STATE_UNTRACKED),
        EntryState::Conflicted => t.t(crate::key::FILES_GIT_STATE_CONFLICTED),
        EntryState::Tracked { staged, unstaged } => {
            let mut parts = Vec::new();
            if let Some(c) = staged {
                parts.push(t.t_with(crate::key::FILES_GIT_STATE_STAGED, &[("what", &what(c))]));
            }
            if let Some(c) = unstaged {
                parts.push(t.t_with(crate::key::FILES_GIT_STATE_UNSTAGED, &[("what", &what(c))]));
            }
            parts.join(&format!(" {} ", tokens::mark::SEPARATOR))
        }
    }
}

fn renamed_from(s: &EntryState) -> Option<&Path> {
    match s {
        EntryState::Tracked { staged, unstaged } => [staged, unstaged]
            .into_iter()
            .flatten()
            .find_map(|c| match c {
                Change::Renamed { from } | Change::Copied { from } => Some(from.as_path()),
                _ => None,
            }),
        _ => None,
    }
}

/// The band behind a diff's line: an addition's, a removal's — the diff's
/// ROLES, as grounds; the hunk's heading is ink, not ground.
const fn band_of(kind: LineKind) -> Option<tokens::Color> {
    match kind {
        LineKind::Added => Some(tokens::color::SURFACE_DIFF_ADDED),
        LineKind::Removed => Some(tokens::color::SURFACE_DIFF_REMOVED),
        _ => None,
    }
}

/// A slice of `text` by CELLS: what is on screen from cell `from`, `wide`
/// cells of it — with where it starts, in bytes, and how many cells in the
/// first cluster start before `from` (a wide cluster cut at the edge is
/// not drawn in half).
fn window_of(text: &str, from: usize, wide: u16) -> (usize, usize) {
    let (mut at, mut start, mut end) = (0usize, None, text.len());
    let mut byte = 0usize;
    for cluster in cells_of(text) {
        let w = usize::from(width(cluster));
        if start.is_none() && at >= from {
            start = Some(byte);
        }
        if start.is_some() && at + w > from + usize::from(wide) {
            end = byte;
            break;
        }
        at += w;
        byte += cluster.len();
    }
    let start = start.unwrap_or(text.len());
    (start, end.max(start))
}

/// One diff line on screen: its band, and its content inked by role.
fn diff_line(l: &DiffLine, wide: u16, across: usize, t: &ronoctua_i18n::Texts) -> Line {
    // OUR OWN LINES: a section's title — what follows is staged, or not —
    // and a sentence in place of lines there is nothing to show of.
    let heading = |key: &str| {
        Line::of(vec![plain(
            0,
            elide_end(&t.t(key), wide),
            tokens::color::TEXT_HEADING,
        )])
    };
    let said = |key: &str| {
        Line::of(vec![plain(
            0,
            elide_end(&t.t(key), wide),
            tokens::color::TEXT_DIM,
        )])
    };
    match l.kind {
        LineKind::Staged => return heading(crate::key::FILES_GIT_DIFF_STAGED),
        LineKind::Unstaged => return heading(crate::key::FILES_GIT_DIFF_UNSTAGED),
        LineKind::NotTracked => return heading(crate::key::FILES_GIT_DIFF_UNTRACKED),
        LineKind::Binary => return said(crate::key::FILES_GIT_DIFF_BINARY),
        LineKind::NoContent => return said(crate::key::FILES_GIT_DIFF_NO_CONTENT),
        LineKind::Link => return said(crate::key::FILES_GIT_DIFF_LINK),
        LineKind::Special => return said(crate::key::FILES_GIT_DIFF_SPECIAL),
        _ => {}
    }
    let base = match l.kind {
        LineKind::Meta => tokens::color::TEXT_DIM,
        LineKind::Hunk => tokens::color::SYNTAX_ATTRIBUTE,
        LineKind::Note => tokens::color::TEXT_DIM,
        _ => tokens::color::TEXT_PRIMARY,
    };
    let (start, end) = window_of(&l.text, across, wide);
    let mut fields = Vec::new();
    let mut at = 0u16;
    let mut byte = start;
    let push =
        |from: usize, to: usize, color: tokens::Color, at: &mut u16, fields: &mut Vec<Field>| {
            if from >= to {
                return;
            }
            let piece = &l.text[from..to];
            let w = width(piece);
            fields.push(plain(*at, piece.to_owned(), color));
            *at = at.saturating_add(w);
        };
    // THE SIGN, in its role's ink.
    let sign_color = match l.kind {
        LineKind::Added => Some(tokens::color::SYNTAX_ADDED),
        LineKind::Removed => Some(tokens::color::SYNTAX_REMOVED),
        _ => None,
    };
    for r in &l.ink {
        let (from, to) = (r.start.max(start), r.end.min(end));
        if from >= to {
            continue;
        }
        push(byte, from, base, &mut at, &mut fields);
        push(
            from,
            to,
            crate::chrome::syntax_color(r.role),
            &mut at,
            &mut fields,
        );
        byte = to;
    }
    push(byte, end, base, &mut at, &mut fields);
    if let (Some(c), Some(first)) = (sign_color, fields.first_mut())
        && start == 0
    {
        // The sign is the first character of the first piece.
        let sign: String = first.text.chars().take(1).collect();
        let rest: String = first.text.chars().skip(1).collect();
        let at0 = first.at;
        let color = first.color;
        *first = plain(at0, sign, c);
        if !rest.is_empty() {
            fields.insert(1, plain(at0 + 1, rest, color));
        }
    }
    Line {
        fields,
        target: None,
        band: band_of(l.kind),
    }
}

/// The blame's gutter: a short id, and the air after it.
fn blame_gutter() -> u16 {
    id_cells() + 1
}

/// The blame's lines on screen: the commit of each line — shown once for a
/// run of lines from the same commit — and the line, inked in the file's
/// language.
fn blame_lines(
    blame: &[BlameLine],
    path: &Path,
    wide: u16,
    offset: usize,
    lines: usize,
    across: usize,
) -> Vec<Line> {
    let grammar = language_of(path).map(ronoctua_syntax::Language::grammar);
    let gutter = blame_gutter();
    let mut out = Vec::new();
    for (n, b) in blame.iter().enumerate().skip(offset).take(lines) {
        let same = n > 0 && blame[n - 1].commit == b.commit && n != offset;
        let mut fields = Vec::new();
        if !same {
            fields.push(plain(
                0,
                short_of(&b.commit.0),
                if b.commit.0.bytes().all(|c| c == b'0') {
                    tokens::color::STATE_WARNING
                } else {
                    tokens::color::TEXT_DIM
                },
            ));
        }
        let text = untabbed(&b.text);
        let (start, end) = window_of(&text, across, wide.saturating_sub(gutter));
        let mut at = gutter;
        let mut byte = start;
        let ranges = grammar.map_or_else(Vec::new, |g| {
            ronoctua_syntax::line(&text, g, ronoctua_syntax::State::fresh()).0
        });
        let mut push = |from: usize, to: usize, color: tokens::Color, at: &mut u16| {
            if from < to {
                let piece = &text[from..to];
                fields.push(plain(*at, piece.to_owned(), color));
                *at = at.saturating_add(width(piece));
            }
        };
        for r in &ranges {
            let (from, to) = (r.start.max(start), r.end.min(end));
            if from >= to {
                continue;
            }
            push(byte, from, tokens::color::TEXT_PRIMARY, &mut at);
            push(from, to, crate::chrome::syntax_color(r.role), &mut at);
            byte = to;
        }
        push(byte, end, tokens::color::TEXT_PRIMARY, &mut at);
        out.push(Line::of(fields));
    }
    out
}

/// Why a version could not be copied out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepWhy {
    /// What git answered — no such path in that commit, too large, and so on.
    Git(Why),
    /// The disk would not take the copy: full, read-only, not ours.
    Disk,
}

impl KeepWhy {
    /// The sentence it is, in the person's words.
    #[must_use]
    pub fn said(self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Git(why) => crate::git_line::why_said(why, texts),
            Self::Disk => texts.t(crate::key::FILES_GIT_KEEP_WHY_DISK),
        }
    }
}

/// The folder a copy of `path` as `commit` has it lives in: ONE COPY PER
/// FOLDER, named by the commit and the whole path — two files called
/// `mod.rs` in one commit are two copies, never the first one twice — and
/// the file inside keeps its own name, so whatever opens it knows its type.
fn version_dir(cache: &Path, commit: &Oid, path: &Path) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    // FNV-1a, 64 bits: stable across processes and versions of Rust (a
    // `DefaultHasher` is neither promised), and a cache key, not a secret.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in commit
        .0
        .as_bytes()
        .iter()
        .chain(b":")
        .chain(path.as_os_str().as_bytes())
    {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    cache.join(format!("{}-{hash:016x}", crate::git_line::short(&commit.0)))
}

/// Copies `path` as `commit` has it into the versions' cache, read-only,
/// and hands back where — keeping at most `cache.gitVersionsMost` copies
/// (`assets/cache.nix`), the least recently kept going first. Blocks on git
/// and on the disk: a thread's.
///
/// # Errors
///
/// What git refused, or that the disk would not take it.
pub fn keep_version(
    repo: &ronoctua_git::Repo,
    commit: &Oid,
    path: &Path,
    cache: &Path,
) -> Result<PathBuf, KeepWhy> {
    keep_version_within(
        repo,
        commit,
        path,
        cache,
        ronoctua_inspect::cache::policy::GIT_VERSIONS_MOST,
    )
}

/// [`keep_version`], keeping at most `most` copies.
fn keep_version_within(
    repo: &ronoctua_git::Repo,
    commit: &Oid,
    path: &Path,
    cache: &Path,
    most: usize,
) -> Result<PathBuf, KeepWhy> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let dir = version_dir(cache, commit, path);
    let name = path
        .file_name()
        .map_or_else(|| std::ffi::OsString::from("file"), ToOwned::to_owned);
    let file = dir.join(&name);
    // A COPY ALREADY THERE is the same bytes — a commit does not change, and
    // a copy only ever appears whole (renamed into place below) — and it is
    // read-only: it is opened again, never written over. Its folder is
    // touched, so the trim below counts it as just kept.
    if file.is_file() {
        if let Ok(d) = std::fs::File::open(&dir) {
            let _ = d.set_modified(std::time::SystemTime::now());
        }
    } else {
        let bytes = repo
            .blob(commit, path, VERSION_MOST_BYTES)
            .map_err(|r| KeepWhy::Git(r.why))?;
        std::fs::create_dir_all(&dir).map_err(|_| KeepWhy::Disk)?;
        // WRITTEN BESIDE, THEN RENAMED IN: the two doors are two processes,
        // and a copy half written by one must never be opened by the other.
        let mut part = std::ffi::OsString::from(".");
        part.push(&name);
        part.push(format!(".{}.part", std::process::id()));
        let part = dir.join(part);
        let _ = std::fs::remove_file(&part);
        let written = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o444)
            .open(&part)
            .and_then(|mut f| f.write_all(&bytes))
            .and_then(|()| std::fs::rename(&part, &file));
        if written.is_err() {
            let _ = std::fs::remove_file(&part);
            return Err(KeepWhy::Disk);
        }
    }
    trim_versions(cache, most, &dir);
    Ok(file)
}

/// Keeps the newest `most` copies of the versions' cache — each copy is a
/// folder of one file — removing the oldest folders, and never `keep`: the
/// copy just made or opened again, which is about to be read.
pub fn trim_versions(cache: &Path, most: usize, keep: &Path) {
    let Ok(each) = std::fs::read_dir(cache) else {
        return;
    };
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = each
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if dirs.len() <= most {
        return;
    }
    dirs.sort();
    let over = dirs.len() - most;
    for (_, dir) in dirs.into_iter().filter(|(_, d)| d != keep).take(over) {
        // READ-ONLY FILES in a folder this process owns: the folder's own
        // write bit is what removal asks, and it has it.
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Where the versions' copies live: `$XDG_CACHE_HOME/ronoctua/git-versions`.
#[must_use]
pub fn versions_dir() -> Option<PathBuf> {
    ronoctua_xdg::cache_dir().map(|d| d.join("git-versions"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> std::rc::Rc<ronoctua_i18n::Texts> {
        std::rc::Rc::new(ronoctua_i18n::Texts::fresh(language).unwrap())
    }

    fn id(n: u8) -> Oid {
        Oid(format!("{n:02x}").repeat(20))
    }

    fn commit(n: u8, subject: &str) -> Commit {
        Commit {
            id: id(n),
            parents: if n > 1 { vec![id(n - 1)] } else { Vec::new() },
            author: "Ada".to_owned(),
            at_s: 1_700_000_000 + i64::from(n),
            subject: subject.to_owned(),
        }
    }

    fn a_view(wide: bool) -> View {
        View::at(
            Path::new("/r"),
            wide,
            texts("en-GB"),
            ronoctua_files::when::Zone::utc(),
        )
    }

    fn told(v: &mut View, need: Need, answer: Answer) -> bool {
        let generation = v.generation();
        v.heard(Heard {
            generation,
            need,
            answer,
        })
    }

    fn key(v: &mut View, notation: &str) -> Errand {
        v.key(ronoctua_keymap::Key::parse(notation).unwrap())
    }

    /// A view with three commits listed, the newest changing two files.
    fn with_history() -> View {
        let mut v = a_view(true);
        let _ = v.drawn(180, 30);
        let _ = v.wanted();
        told(
            &mut v,
            Need::History {
                skip: 0,
                count: PAGE,
            },
            Answer::Commits(Ok(vec![
                commit(3, "third"),
                commit(2, "second"),
                commit(1, "first"),
            ])),
        );
        told(
            &mut v,
            Need::Commit(id(3)),
            Answer::Commit(Ok(CommitDetail {
                commit: commit(3, "third"),
                message: "third\n\nA body that says why, at some length.".to_owned(),
                files: vec![
                    ChangedFile {
                        path: PathBuf::from("src/lib.rs"),
                        change: Change::Modified,
                    },
                    ChangedFile {
                        path: PathBuf::from("gone.txt"),
                        change: Change::Deleted,
                    },
                ],
            })),
        );
        v
    }

    #[test]
    fn a_diff_is_read_into_roles_and_the_files_own_ink() {
        let diff = "diff --git a/x.rs b/x.rs\nindex 1..2 100644\n--- a/x.rs\n+++ b/x.rs\n\
                    @@ -1,2 +1,2 @@\n fn main() {\n-    let a = 1;\n+    let a = \"é\";\n \\ No newline\n";
        let lines = diff_lines(diff, ronoctua_syntax::of_tag("rust"));
        let kinds: Vec<LineKind> = lines.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [
                LineKind::Meta,
                LineKind::Meta,
                LineKind::Meta,
                LineKind::Meta,
                LineKind::Hunk,
                LineKind::Context,
                LineKind::Removed,
                LineKind::Added,
                LineKind::Context,
            ]
        );
        // THE INK IS THE FILE'S LANGUAGE, past the sign, on char boundaries.
        let added = &lines[7];
        assert!(
            added
                .ink
                .iter()
                .any(|r| r.role == ronoctua_syntax::Role::String),
            "{added:?}"
        );
        for l in &lines {
            for r in &l.ink {
                assert!(r.start >= 1 && r.start < r.end && r.end <= l.text.len());
                assert!(l.text.is_char_boundary(r.start) && l.text.is_char_boundary(r.end));
            }
        }
        // AND THE BANDS ARE THE ROLES.
        assert_eq!(
            band_of(LineKind::Added),
            Some(tokens::color::SURFACE_DIFF_ADDED)
        );
        assert_eq!(
            band_of(LineKind::Removed),
            Some(tokens::color::SURFACE_DIFF_REMOVED)
        );
        assert_eq!(band_of(LineKind::Context), None);
        // TABS ARE SPACES, to the stops git keeps.
        assert_eq!(untabbed("+\tx"), "+       x");
    }

    #[test]
    fn three_columns_from_the_token_in_a_window_and_two_in_the_footer() {
        let three = tokens::space::GIT_THREECOLUMNSFROM;
        let wide = Layout::of(three, true, Col::Items);
        assert_eq!(wide.places.len(), 3);
        assert_eq!(Layout::of(three - 1, true, Col::Items).places.len(), 2);
        assert_eq!(
            Layout::of(three + 40, false, Col::Items).places.len(),
            2,
            "the footer"
        );
        // THE PAIR FOLLOWS THE KEYBOARD INTO THE DIFF.
        let cols = |focus| -> Vec<Col> {
            Layout::of(120, false, focus)
                .places
                .iter()
                .map(|p| p.col)
                .collect()
        };
        assert_eq!(cols(Col::Items), [Col::Items, Col::Detail]);
        assert_eq!(cols(Col::Detail), [Col::Items, Col::Detail]);
        assert_eq!(cols(Col::Diff), [Col::Detail, Col::Diff]);
        assert_eq!(cols_of_narrow(), [Col::Detail]);
        // NOTHING LEAKS AND NOTHING TOUCHES A RULE, whatever the width: the
        // text of a column begins `files.gutter` past its left boundary and
        // ends `files.edgeRight` before its right one, and the rule is a cell
        // of its own between them.
        let gutter = tokens::space::FILES_GUTTER;
        let air = tokens::space::FILES_EDGERIGHT;
        for columns in [10_u16, 41, 45, 80, 159, 160, 200, 400] {
            for wide in [false, true] {
                let l = Layout::of(columns, wide, Col::Items);
                let mut boundary: Option<u16> = None;
                for p in &l.places {
                    assert_eq!(p.left, boundary, "{columns}: {l:?}");
                    let start = boundary.map_or(0, |b| b + 1);
                    assert!(
                        p.from >= start + gutter,
                        "{columns}: text on the rule {l:?}"
                    );
                    assert!(p.from <= p.to, "{columns}: {l:?}");
                    assert!(
                        p.to + air <= p.edge,
                        "{columns}: no air before the rule {l:?}"
                    );
                    boundary = Some(p.edge);
                }
                assert!(
                    l.places
                        .last()
                        .is_some_and(|p| p.edge <= columns.max(gutter + air)),
                    "{columns}: the last column runs past the region {l:?}"
                );
            }
        }
    }

    fn cols_of_narrow() -> Vec<Col> {
        Layout::of(30, false, Col::Detail)
            .places
            .iter()
            .map(|p| p.col)
            .collect()
    }

    #[test]
    fn what_is_under_the_cursor_is_what_is_asked_for_and_asked_once() {
        let mut v = a_view(true);
        let _ = v.drawn(180, 30);
        let first = v.wanted();
        assert!(first.contains(&Need::Status));
        assert!(first.contains(&Need::History {
            skip: 0,
            count: PAGE
        }));
        assert!(v.wanted().is_empty(), "asked twice in one generation");
        let mut v = with_history();
        let asked = v.wanted();
        // THE DETAIL WAS ANSWERED; THE DIFF OF ITS FIRST FILE IS ASKED.
        assert!(!asked.contains(&Need::Commit(id(3))));
        assert!(asked.contains(&Need::Diff {
            commit: id(3),
            path: PathBuf::from("src/lib.rs")
        }));
        // DOWN: the next commit's detail.
        assert_eq!(key(&mut v, "<down>"), Errand::Paint);
        assert!(v.wanted().contains(&Need::Commit(id(2))));
        // A COMMIT NEVER MOVES; the history does.
        v.stale();
        let again = v.wanted();
        assert!(again.contains(&Need::History {
            skip: 0,
            count: PAGE
        }));
        assert!(again.contains(&Need::Status));
        assert!(!again.contains(&Need::Commit(id(2))), "{again:?}");
    }

    #[test]
    fn an_answer_from_before_the_repository_moved_is_dropped() {
        let mut v = with_history();
        let old = v.generation();
        v.stale();
        assert!(!v.heard(Heard {
            generation: old,
            need: Need::History {
                skip: 0,
                count: PAGE
            },
            answer: Answer::Commits(Ok(Vec::new())),
        }));
        assert_eq!(v.items_len(), 3, "the stale answer emptied the list");
        // A commit is a commit, whenever it was asked.
        assert!(v.heard(Heard {
            generation: old,
            need: Need::Commit(id(1)),
            answer: Answer::Commit(Err(Why::Failed)),
        }));
    }

    #[test]
    fn the_cursor_stays_on_its_commit_when_the_history_moves_under_it() {
        let mut v = with_history();
        key(&mut v, "<down>");
        assert_eq!(v.item().map(Item::id), Some(id(2).0));
        v.stale();
        told(
            &mut v,
            Need::History {
                skip: 0,
                count: PAGE,
            },
            Answer::Commits(Ok(vec![
                commit(4, "fourth"),
                commit(3, "third"),
                commit(2, "second"),
                commit(1, "first"),
            ])),
        );
        assert_eq!(
            v.item().map(Item::id),
            Some(id(2).0),
            "a new commit on top moved the cursor"
        );
    }

    /// `n` commits, newest first, their ids from `from` on.
    fn commits(from: usize, n: usize) -> Vec<Commit> {
        (from..from + n)
            .map(|k| {
                let mut c = commit(1, "x");
                c.id = Oid(format!("{k:040x}"));
                c
            })
            .collect()
    }

    #[test]
    // THE DEFECT THIS CATCHES: a repository that moved threw away every page
    // after the first — the list asked its first page again, dropped the
    // rest, and the cursor on the 380th commit was clamped onto the 200th.
    // The browser's own fetch is enough to move it.
    fn the_cursor_keeps_its_commit_past_the_first_page_when_the_repository_moves() {
        let mut v = a_view(true);
        let _ = v.drawn(180, 30);
        let _ = v.wanted();
        let first = Need::History {
            skip: 0,
            count: PAGE,
        };
        told(&mut v, first, Answer::Commits(Ok(commits(0, PAGE))));
        v.cursor[0] = PAGE - 1;
        let second = Need::History {
            skip: PAGE,
            count: PAGE,
        };
        assert!(v.wanted().contains(&second));
        told(&mut v, second, Answer::Commits(Ok(commits(PAGE, PAGE))));
        // THE PERSON walks to the 380th, as a key does.
        v.cursor[0] = 379;
        v.on = v.item().map(Item::id);
        let on = v.on.clone();
        // A FETCH, or a commit: the repository moved.
        v.stale();
        let again: Vec<Need> = v
            .wanted()
            .into_iter()
            .filter(|n| matches!(n, Need::History { .. }))
            .collect();
        assert_eq!(
            again,
            [Need::History {
                skip: 0,
                count: 2 * PAGE
            }],
            "the whole list is read again, and nothing after it until it is"
        );
        // ONE NEW COMMIT ON TOP: everything moved down by one.
        let mut moved = commits(9_999, 1);
        moved.extend(commits(0, 2 * PAGE - 1));
        told(
            &mut v,
            Need::History {
                skip: 0,
                count: 2 * PAGE,
            },
            Answer::Commits(Ok(moved)),
        );
        assert_eq!(v.items_len(), 2 * PAGE, "the list was cut to a page");
        assert_eq!(v.item().map(Item::id), on, "the cursor lost its commit");
        assert_eq!(v.cursor[0], 380);
        // AND THE PAGE AFTER IT follows on from the whole list.
        v.cursor[0] = 2 * PAGE - 1;
        assert!(v.wanted().contains(&Need::History {
            skip: 2 * PAGE,
            count: PAGE
        }));
    }

    #[test]
    // THE DEFECT THIS CATCHES: a file's history put the followed file first
    // only under its CURRENT name — a commit from before a rename showed the
    // diff of whichever file it listed first.
    fn a_files_history_opens_each_commit_on_the_file_under_the_name_it_had() {
        let mut v = with_history();
        key(&mut v, "RET");
        key(&mut v, "C-f");
        let _ = v.wanted();
        let old = commit(1, "before the rename");
        told(
            &mut v,
            Need::FileHistory {
                path: PathBuf::from("src/lib.rs"),
                skip: 0,
                count: PAGE,
            },
            Answer::FileCommits(Ok(vec![FileCommit {
                commit: old.clone(),
                name: Some(PathBuf::from("src/old.rs")),
            }])),
        );
        told(
            &mut v,
            Need::Commit(old.id.clone()),
            Answer::Commit(Ok(CommitDetail {
                commit: old.clone(),
                message: "before the rename".to_owned(),
                files: vec![
                    ChangedFile {
                        path: PathBuf::from("README.md"),
                        change: Change::Modified,
                    },
                    ChangedFile {
                        path: PathBuf::from("src/old.rs"),
                        change: Change::Modified,
                    },
                ],
            })),
        );
        assert_eq!(
            v.file_at().map(|f| f.path),
            Some(PathBuf::from("src/old.rs")),
            "the diff shown is another file's"
        );
        assert!(v.wanted().contains(&Need::Diff {
            commit: old.id,
            path: PathBuf::from("src/old.rs")
        }));
    }

    #[test]
    // THE DEFECT THIS CATCHES: the detail was COUNTED at its full width and
    // DRAWN one cell short once its bar took that cell — a message that wraps
    // one line more at the narrower width had its last line where no scroll
    // reached.
    fn the_details_last_line_is_reached_at_every_width() {
        let mut v = with_history();
        let words = "a word of seven letters wraps where it wraps ".repeat(12);
        told(
            &mut v,
            Need::Commit(id(3)),
            Answer::Commit(Ok(CommitDetail {
                commit: commit(3, "third"),
                message: format!("third\n\n{words}THE-LAST-WORD"),
                files: Vec::new(),
            })),
        );
        let mut cut = 0;
        for columns in 50_u16..=120 {
            let room = 8;
            // TO THE BOTTOM, as a wheel or a bar takes it: the offset is
            // clamped to the last place the column can start.
            v.offset[1] = usize::MAX / 2;
            let d = v.drawn(columns, room);
            let pane = d.pane(Col::Detail).expect("the detail is on screen");
            if !pane.barred() {
                continue;
            }
            cut += 1;
            let said: String = pane
                .body
                .iter()
                .flat_map(|l| l.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                said.contains("THE-LAST-WORD"),
                "{columns}: the detail's last line is out of reach — {said:?}"
            );
            // AND NOTHING RUNS UP TO THE BAR: `files.edgeRight` of air before
            // its cell, as before a rule — an id against the bar reads as cut.
            let bar = pane.to - 1;
            for l in pane.lines() {
                for f in &l.fields {
                    assert!(
                        pane.from + f.at + width(&f.text) + tokens::space::FILES_EDGERIGHT <= bar,
                        "{columns}: {:?} runs up to the bar",
                        f.text
                    );
                }
            }
        }
        assert!(cut > 20, "the message was never long enough for a bar");
    }

    #[test]
    fn ret_drills_in_and_esc_comes_back_one_level_then_closes() {
        let mut v = with_history();
        assert_eq!(v.focus(), Col::Items);
        assert_eq!(key(&mut v, "RET"), Errand::Paint);
        assert_eq!(v.focus(), Col::Detail);
        assert_eq!(key(&mut v, "RET"), Errand::Paint);
        assert_eq!(v.focus(), Col::Diff);
        assert_eq!(
            key(&mut v, "C-a"),
            Errand::Say(crate::key::FILES_GIT_BLAME_GONE)
        );
        assert_eq!(key(&mut v, "ESC"), Errand::Paint);
        assert_eq!(v.focus(), Col::Detail);
        assert_eq!(key(&mut v, "ESC"), Errand::Paint);
        assert_eq!(key(&mut v, "ESC"), Errand::Close);
    }

    #[test]
    fn a_files_history_is_its_own_listing_and_esc_returns_to_the_kind() {
        let mut v = with_history();
        key(&mut v, "RET");
        assert_eq!(key(&mut v, "C-f"), Errand::Paint);
        assert_eq!(v.listing(), &Listing::File(PathBuf::from("src/lib.rs")));
        assert!(v.wanted().contains(&Need::FileHistory {
            path: PathBuf::from("src/lib.rs"),
            skip: 0,
            count: PAGE,
        }));
        assert_eq!(key(&mut v, "ESC"), Errand::Paint);
        assert_eq!(v.listing(), &Listing::Kind(Kind::History));
    }

    #[test]
    fn the_safe_actions_name_what_is_under_the_cursor() {
        let mut v = with_history();
        assert_eq!(key(&mut v, "C-y"), Errand::Copy(id(3).0));
        assert_eq!(key(&mut v, "r"), Errand::Fetch);
        assert_eq!(
            key(&mut v, "r"),
            Errand::Say(crate::key::FILES_GIT_FETCHING),
            "two fetches at once"
        );
        let _ = v.fetched(Err(Why::NeedsCredentials));
        assert_eq!(key(&mut v, "C-t"), Errand::Terminal(PathBuf::from("/r")));
        assert_eq!(
            key(&mut v, "C-o"),
            Errand::Keep {
                commit: id(3),
                path: PathBuf::from("src/lib.rs")
            }
        );
        // A DELETED FILE opens as its parent had it.
        key(&mut v, "RET");
        key(&mut v, "<down>");
        assert_eq!(
            key(&mut v, "C-o"),
            Errand::Keep {
                commit: id(2),
                path: PathBuf::from("gone.txt")
            }
        );
    }

    #[test]
    fn every_key_of_the_table_is_answered_and_none_is_bound_twice() {
        let mut seen = std::collections::HashSet::new();
        for (notation, id) in OF_THE_VIEW {
            assert!(seen.insert(*notation), "{notation} is bound twice");
            let k =
                ronoctua_keymap::Key::parse(notation).unwrap_or_else(|e| panic!("{notation}: {e}"));
            assert_eq!(of_the_view(k), Some(*id), "{notation}");
            // NOT A COMPOSITOR'S CHORD: `M-` never reaches a door.
            assert!(!notation.starts_with("M-"), "{notation}");
        }
        for (_, id) in OF_THE_VIEW {
            let mut v = with_history();
            // UP needs somewhere to come up from.
            if id.ends_with("-up") {
                let _ = v.run("files-git-down");
            }
            assert_ne!(
                v.run(id),
                Errand::Nothing,
                "{id} does nothing on a history with a commit under the cursor"
            );
        }
    }

    #[test]
    fn a_point_is_a_kind_a_row_or_a_column_and_the_bar_is_the_bars() {
        let v = with_history();
        let d = v.drawn(180, 30);
        let items = d.pane(Col::Items).unwrap();
        // THE KINDS' LINE: each piece its kind.
        let kinds = &items.head[0];
        for f in &kinds.fields {
            let k = kind_of_hit(&f.hit).unwrap();
            assert_eq!(d.target_at(0, items.from + f.at), Some(Target::Kind(k)));
        }
        // THE ROWS.
        let top = items.head.len();
        assert_eq!(d.target_at(top, 2), Some(Target::Item(0)));
        assert_eq!(d.target_at(top + 1, 2), Some(Target::Item(1)));
        // THE DETAIL'S FILES are rows too, after its facts and message.
        let detail = d.pane(Col::Detail).unwrap();
        let line = detail
            .lines()
            .position(|l| l.target == Some(Target::File(1)))
            .unwrap();
        assert_eq!(d.target_at(line, detail.from + 3), Some(Target::File(1)));
        // A RULE CELL BELONGS TO THE COLUMN ON ITS LEFT.
        assert_eq!(d.pane_at(items.to).map(|p| p.col), Some(Col::Items));
    }

    #[test]
    fn a_click_selects_and_a_second_click_opens() {
        let mut v = with_history();
        let d = v.drawn(180, 30);
        let top = d.pane(Col::Items).unwrap().head.len();
        assert_eq!(v.clicked(top + 1, 3), Errand::Paint);
        assert_eq!(v.item().map(Item::id), Some(id(2).0));
        assert_eq!(v.focus(), Col::Items);
        assert_eq!(v.clicked(top + 1, 3), Errand::Paint);
        assert_eq!(v.focus(), Col::Detail, "the second click is RET");
        // A KIND, clicked.
        let d = v.drawn(180, 30);
        let items = d.pane(Col::Items).unwrap();
        let refs = items.head[0]
            .fields
            .iter()
            .find(|f| kind_of_hit(&f.hit) == Some(Kind::Refs))
            .unwrap();
        v.clicked(0, items.from + refs.at);
        assert_eq!(v.listing(), &Listing::Kind(Kind::Refs));
        assert!(v.wanted().contains(&Need::Refs));
    }

    #[test]
    fn a_long_history_asks_its_next_page_before_the_cursor_meets_the_end() {
        let mut v = a_view(true);
        let _ = v.drawn(180, 30);
        let _ = v.wanted();
        let page: Vec<Commit> = (0..PAGE)
            .map(|n| {
                let mut c = commit(1, "x");
                c.id = Oid(format!("{n:040x}"));
                c
            })
            .collect();
        told(
            &mut v,
            Need::History {
                skip: 0,
                count: PAGE,
            },
            Answer::Commits(Ok(page)),
        );
        let next = Need::History {
            skip: PAGE,
            count: PAGE,
        };
        assert!(!v.wanted().contains(&next));
        v.cursor[0] = PAGE - PAGE_AHEAD;
        assert!(v.wanted().contains(&next));
        told(&mut v, next, Answer::Commits(Ok(vec![commit(1, "last")])));
        assert_eq!(v.items_len(), PAGE + 1);
        v.cursor[0] = PAGE;
        assert!(
            !v.wanted()
                .iter()
                .any(|n| matches!(n, Need::History { skip, .. } if *skip > PAGE)),
            "a short page is the end"
        );
    }

    #[test]
    fn every_line_stays_in_its_column_and_the_diff_band_stops_at_the_bar() {
        let mut v = with_history();
        key(&mut v, "RET");
        key(&mut v, "RET");
        let long = format!("@@ -1 +1 @@\n+{}\n-y\n", "z".repeat(400));
        let lines = (0..80).map(|_| long.clone()).collect::<String>();
        told(
            &mut v,
            Need::Diff {
                commit: id(3),
                path: PathBuf::from("src/lib.rs"),
            },
            Answer::Text(Ok(text_of(diff_lines(&lines, None)))),
        );
        for (columns, wide) in [(180_u16, true), (120, true), (90, false)] {
            let mut v2 = with_history();
            v2.focus = Col::Diff;
            v2.diffs = v.diffs.clone();
            let d = v2.drawn(columns, 24);
            for p in &d.panes {
                // THE HEAD sits above the bar, and may use its cell.
                for (l, end) in p
                    .head
                    .iter()
                    .map(|l| (l, p.to))
                    .chain(p.body.iter().map(|l| (l, p.text_to())))
                {
                    for f in &l.fields {
                        assert!(
                            p.from + f.at + width(&f.text) <= end,
                            "{columns}: {:?} leaks out of {:?}",
                            f.text,
                            p.col
                        );
                    }
                }
            }
            let diff = d.pane(Col::Diff).unwrap();
            assert!(diff.barred(), "eighty lines in twenty-four");
            assert!(diff.across.is_some_and(|a| a.cut()), "a line of 400 cells");
            let px = Px {
                lead: 1,
                top: 30.0,
                bottom: 30.0 + 24.0 * 20.0,
                edge: f32::from(columns + 2) * 10.0,
                cell: 10.0,
                line: 20.0,
            };
            let bar = bars(&d, &px)
                .into_iter()
                .find(|b| b.col == Col::Diff && !b.across)
                .unwrap();
            for q in bands(&d, &px) {
                assert!(
                    q.x + q.width <= bar.track.0 + 0.01,
                    "a band ran under the bar"
                );
            }
            let _ = wide;
        }
    }

    #[test]
    fn a_bar_is_grabbed_where_it_is_drawn() {
        let mut v = a_view(true);
        let _ = v.drawn(180, 10);
        let _ = v.wanted();
        let many: Vec<Commit> = (0..50)
            .map(|n| {
                let mut c = commit(1, "x");
                c.id = Oid(format!("{n:040x}"));
                c
            })
            .collect();
        told(
            &mut v,
            Need::History {
                skip: 0,
                count: PAGE,
            },
            Answer::Commits(Ok(many)),
        );
        let d = v.drawn(180, 10);
        let px = Px {
            lead: 1,
            top: 0.0,
            bottom: 200.0,
            edge: 1820.0,
            cell: 10.0,
            line: 20.0,
        };
        let bar = bars(&d, &px)
            .into_iter()
            .find(|b| b.col == Col::Items)
            .unwrap();
        let (x, y, w, h) = bar.track;
        assert!(bar.holds(x + w / 2.0, y + h - 1.0));
        let last = bar.offset_at(x, y + h, &px);
        assert!(v.scroll_bar(&bar, last));
        let d = v.drawn(180, 10);
        let items = d.pane(Col::Items).unwrap();
        assert_eq!(
            items.scroll.offset,
            items.scroll.total - items.scroll.lines,
            "the bottom of the track is the last page"
        );
        // AND NO ROW ANSWERS IN THE BAR'S CELL.
        assert_eq!(
            d.target_at(items.head.len(), items.to - 1),
            Some(Target::In(Col::Items))
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: the footer laid the browser out on the whole
    // screen's cells and drew it a cell in (`chrome::FIELDS_LEAD`), so the
    // right column's last cell and its bar were past the surface's edge —
    // drawn where nobody sees them, grabbed where nothing is.
    fn in_the_footer_every_line_and_every_bar_is_inside_the_surface() {
        let mut v = with_history();
        v.wide = false;
        key(&mut v, "RET");
        key(&mut v, "RET");
        let long = format!("@@ -1 +1 @@\n+{}\n", "z".repeat(400));
        told(
            &mut v,
            Need::Diff {
                commit: id(3),
                path: PathBuf::from("src/lib.rs"),
            },
            Answer::Text(Ok(text_of(diff_lines(&long.repeat(60), None)))),
        );
        let cell = 9.0_f32;
        for surface in [900.0_f32, 1366.0, 1917.0, 2560.0] {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let screen = (surface / cell) as u16;
            let d = v.drawn(footer_columns(screen), 20);
            let px = Px {
                lead: crate::chrome::FIELDS_LEAD,
                top: 100.0,
                bottom: 18.0f32.mul_add(21.0, 100.0),
                edge: surface,
                cell,
                line: 18.0,
            };
            assert!(
                d.panes.last().is_some_and(Pane::barred),
                "{surface}: the diff is long enough for a bar"
            );
            for b in bars(&d, &px) {
                let (x, _, w, _) = b.track;
                assert!(
                    x + w <= surface + 0.01,
                    "{surface}: a bar ends at {} past the surface",
                    x + w
                );
            }
            let margin = f32::from(crate::chrome::FIELDS_LEAD) * cell;
            for p in &d.panes {
                for l in p.lines() {
                    for f in &l.fields {
                        let end = px.x(p.from + f.at + width(&f.text));
                        assert!(
                            end <= surface - margin + 0.01,
                            "{surface}: {:?} ends at {end}, in the right margin or past it",
                            f.text
                        );
                    }
                }
            }
        }
    }

    #[test]
    // THE DEFECT THIS CATCHES: an untracked LINK was read through — a pipe
    // at the end of it blocked its reading thread for good (`Reading…`, for
    // ever), and a link's target was shown as if it were the file.
    fn an_untracked_link_is_where_it_points_and_a_pipe_is_not_read() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-git-untracked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("secret"), "never shown\n").unwrap();
        std::os::unix::fs::symlink(dir.join("secret"), dir.join("link")).unwrap();
        let fifo = dir.join("pipe");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        std::os::unix::fs::symlink(&fifo, dir.join("to-pipe")).unwrap();
        let text = untracked_text(&dir.join("link"), Path::new("link")).unwrap();
        let kinds: Vec<LineKind> = text.lines.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            [LineKind::NotTracked, LineKind::Link, LineKind::Added]
        );
        assert!(
            text.lines[2].text.ends_with("secret"),
            "{:?}",
            text.lines[2]
        );
        assert!(
            text.lines.iter().all(|l| !l.text.contains("never shown")),
            "the link was read through"
        );
        // A PIPE: said, not read — and at once, never waiting for a writer.
        // A link to one is a link: where it points.
        for (name, said) in [
            ("pipe", vec![LineKind::NotTracked, LineKind::Special]),
            (
                "to-pipe",
                vec![LineKind::NotTracked, LineKind::Link, LineKind::Added],
            ),
        ] {
            let (tx, rx) = std::sync::mpsc::channel();
            let file = dir.join(name);
            std::thread::spawn(move || {
                let _ = tx.send(untracked_text(&file, Path::new("x")));
            });
            let text = rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("reading a pipe waited for a writer")
                .unwrap();
            let kinds: Vec<LineKind> = text.lines.iter().map(|l| l.kind).collect();
            assert_eq!(kinds, said, "{name}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_raw_key_reaches_the_screen_in_either_language() {
        for language in ["en-GB", "pt-BR"] {
            let mut v = with_history();
            v.speak(texts(language));
            for kind in Kind::ALL {
                v.pick(kind);
                let d = v.drawn(180, 30);
                for line in d.lines() {
                    for f in line {
                        assert!(
                            !f.text.contains("files-git") && !f.text.contains('['),
                            "{language} {kind:?}: {:?}",
                            f.text
                        );
                    }
                }
            }
            let hint = v.hint(&v.texts.clone());
            assert!(hint.iter().all(|p| !p.text.contains("files-git")));
        }
    }

    #[test]
    fn a_kept_version_is_read_only_and_the_cache_is_bounded() {
        use std::os::unix::fs::PermissionsExt as _;
        let tools = ronoctua_git::Tools::from_env()
            .expect("RONOCTUA_GIT is not set: run inside `nix develop` or the tests derivation");
        let root =
            std::env::temp_dir().join(format!("ronoctua-git-browser-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let work = root.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new(&tools.git)
                .arg("-C")
                .arg(&work)
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Ada")
                .env("GIT_AUTHOR_EMAIL", "ada@example.org")
                .env("GIT_COMMITTER_NAME", "Ada")
                .env("GIT_COMMITTER_EMAIL", "ada@example.org")
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(work.join("a.rs"), "fn a() {}\n").unwrap();
        git(&["add", "a.rs"]);
        git(&["commit", "-qm", "one"]);
        let one = Oid(git(&["rev-parse", "HEAD"]));
        std::fs::write(work.join("a.rs"), "fn a() { let x = 1; }\n").unwrap();
        std::fs::write(work.join("new.txt"), "hello\n").unwrap();
        let repo = ronoctua_git::Repo::discover(&work)
            .unwrap()
            .with_tools(tools.clone());
        // THE FOUR READINGS OF A WORK TREE, through `read`.
        let Answer::Text(Ok(work_text)) = read(&repo, &Need::Work(PathBuf::from("a.rs"))) else {
            panic!("no work diff");
        };
        assert!(work_text.lines.iter().any(|l| l.kind == LineKind::Unstaged));
        assert!(work_text.lines.iter().any(|l| l.kind == LineKind::Added));
        let Answer::Text(Ok(new)) = read(&repo, &Need::Untracked(PathBuf::from("new.txt"))) else {
            panic!("no untracked text");
        };
        assert_eq!(new.lines[0].kind, LineKind::NotTracked);
        assert_eq!(new.lines[1].text, "+hello");
        let Answer::Blame(Ok(b)) = read(&repo, &Need::Blame(PathBuf::from("a.rs"))) else {
            panic!("no blame");
        };
        assert_eq!(b.len(), 1);
        // A VERSION KEPT is the bytes the commit had, and cannot be written.
        let cache = root.join("cache");
        let kept = keep_version(&repo, &one, Path::new("a.rs"), &cache).unwrap();
        assert_eq!(std::fs::read(&kept).unwrap(), b"fn a() {}\n");
        let mode = std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o444);
        // AGAIN is the same copy, not a second one.
        assert_eq!(
            keep_version(&repo, &one, Path::new("a.rs"), &cache).unwrap(),
            kept
        );
        // TWO FILES OF ONE NAME in one commit are two copies, each its own
        // bytes — never the first one opened twice.
        for d in ["x", "y"] {
            std::fs::create_dir_all(work.join(d)).unwrap();
            std::fs::write(work.join(d).join("mod.rs"), format!("// {d}\n")).unwrap();
        }
        git(&["add", "x", "y"]);
        git(&["commit", "-qm", "two of a name"]);
        let two = Oid(git(&["rev-parse", "HEAD"]));
        let x = keep_version(&repo, &two, Path::new("x/mod.rs"), &cache).unwrap();
        let y = keep_version(&repo, &two, Path::new("y/mod.rs"), &cache).unwrap();
        assert_ne!(x, y, "two paths of one name were kept as one copy");
        assert_eq!(std::fs::read(&x).unwrap(), b"// x\n");
        assert_eq!(std::fs::read(&y).unwrap(), b"// y\n");
        assert_eq!(x.file_name(), y.file_name(), "a copy keeps its own name");
        // A PATH THE COMMIT DOES NOT HAVE is git's refusal, as a sentence.
        let refused = keep_version(&repo, &one, Path::new("nowhere.rs"), &cache).unwrap_err();
        assert!(matches!(refused, KeepWhy::Git(_)), "{refused:?}");
        for t in ["en-GB", "pt-BR"] {
            for why in [refused, KeepWhy::Disk] {
                let said = why.said(&texts(t));
                assert!(
                    !said.is_empty() && !said.contains("files-git"),
                    "{t}: {said}"
                );
            }
        }
        // THE BOUND COUNTS COPIES — each a folder of one file — and never
        // takes the one just kept, however old its folder is.
        assert_eq!(
            std::fs::read_dir(&cache).unwrap().count(),
            3,
            "a.rs, x/mod.rs and y/mod.rs"
        );
        let a = kept.parent().unwrap().to_path_buf();
        let long_ago = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1);
        std::fs::File::open(&a)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
        assert_eq!(
            keep_version_within(&repo, &one, Path::new("a.rs"), &cache, 1).unwrap(),
            kept
        );
        assert!(
            kept.is_file(),
            "the copy just opened again was trimmed away"
        );
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the band both doors paint under a resting pointer
    /// (`hover_band`) lights what a click at the same point does not take,
    /// or leaves unlit what it takes — an item, a file, a kind (as its
    /// piece: it took the click and lit nothing), a line of a column the
    /// keyboard is not in (a click there gives it the keyboard) — or lights
    /// anything in the column the keyboard is in past its rows, or a bar's
    /// cell, which the bar takes. THE LIGHT IS THE CLICK: every point clicked
    /// once, on a view of its own.
    #[test]
    fn the_band_is_what_the_click_takes() {
        let (columns, room) = (180, 30);
        let px = Px {
            lead: 1,
            top: 0.0,
            bottom: 600.0,
            edge: 1810.0,
            cell: 10.0,
            line: 20.0,
        };
        let v = with_history();
        let drawn = v.drawn(columns, room);
        let (mut lit_any, mut dark_any, mut a_kind) = (false, false, false);
        for line in 0..drawn.room {
            for cell in (0..columns).step_by(2) {
                let lit =
                    hover_band(&drawn, &px, line, cell, tokens::color::SURFACE_HOVER).is_some();
                if drawn
                    .pane_at(cell)
                    .is_some_and(|p| p.barred() && cell + 1 >= p.to)
                {
                    assert!(!lit, "line {line}: a bar's cell lights");
                    continue;
                }
                let mut fresh = with_history();
                let _ = fresh.drawn(columns, room);
                let did = fresh.clicked(line, cell) != Errand::Nothing;
                assert_eq!(
                    lit, did,
                    "line {line} cell {cell}: lit {lit}, a click did {did}"
                );
                lit_any |= lit;
                dark_any |= !lit;
                a_kind |= lit && matches!(drawn.target_at(line, cell), Some(Target::Kind(_)));
            }
        }
        assert!(
            lit_any && dark_any && a_kind,
            "the sweep reached too little"
        );
    }
}
