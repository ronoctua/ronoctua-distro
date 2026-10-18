//! THE GIT LINE — one row under the file manager's path, when the folder on
//! screen is inside a repository: the branch and how far it is from its
//! upstream, how many paths are changed, staged and untracked, and the last
//! commit's subject and age. `C-h`, or a click on the line, opens the history
//! of the repository.
//!
//! # Where git is asked, and when
//!
//! Never on the loop that draws: a status walks the whole work tree and a
//! large one takes a second. The door owns a [`Keeper`], which finds the
//! repository IN RUST (`ronoctua_git::Repo::discover`, a walk up for `.git`)
//! — so git is started only inside a repository — and reads it on a thread
//! of its own. The reading comes back to the door as a [`Line`], and the view
//! keeps it (`files::View::git_heard`).
//!
//! # Event, not clock
//!
//! The reading is asked again when git's own files move: the git directory
//! (`HEAD`, the index, `packed-refs`, `FETCH_HEAD`) and every folder of its
//! `refs`, the ones made later included — watched by inotify
//! ([`ronoctua_files::notice`], [`watched`]) — and when the folder on screen
//! stirs (the view's own watch). A burst of them is ONE reading more: a
//! reading under way marks itself owed again and runs once more when it
//! ends, never twice at once — and the one owed reads the repository
//! followed THEN, not the one the reading under way was asked about.
//!
//! Our own reads write nothing there — `ronoctua-git` runs with optional
//! locks off and never through the porcelain `diff` — so the watch does not
//! hear itself.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ronoctua_files::mode::Explanation;
use ronoctua_i18n::Arg;
use ronoctua_tokens as tokens;

use crate::fields::{Field, Hit, Ink, Row, width_of};
use crate::report::{Head, Line, State};

/// How many hexadecimal digits a short commit id shows — git's own default.
pub const SHORT_ID: usize = 7;

/// The order the file manager's region gives the footer to open the history
/// of the repository whose root is its argument, in the footer's region —
/// what `C-h` becomes there (in a window, the history opens in a window of
/// its own). Not a command of the registry: it carries a folder, which no key
/// can, and the history's half of the footer answers it.
pub const HISTORY: &str = "git-history";

/// A commit id, short.
#[must_use]
pub fn short(id: &str) -> &str {
    id.get(..SHORT_ID).unwrap_or(id)
}

/// Reads a repository: its status and its last commit. Blocks until git
/// ends — a thread's, never the loop's.
#[must_use]
pub fn read(repo: &ronoctua_git::Repo) -> Line {
    let read = repo.status().and_then(|s| {
        let last = repo.head_commit()?.map(|c| (c.subject, c.at_s));
        Ok(state_of(&s, last))
    });
    Line {
        root: repo.root.clone(),
        read: read.map_err(|r| r.why),
    }
}

/// A status and the last commit, as the line's state. Pure.
#[must_use]
pub fn state_of(s: &ronoctua_git::Status, last: Option<(String, i64)>) -> State {
    let head = match (&s.branch, &s.head) {
        (Some(b), Some(_)) => Head::Branch(b.clone()),
        (Some(b), None) => Head::Unborn(b.clone()),
        (None, Some(id)) => Head::Detached(short(&id.0).to_owned()),
        (None, None) => Head::Detached(String::new()),
    };
    State {
        head,
        upstream: s.upstream.as_ref().map(|_| (s.ahead, s.behind)),
        changed: s.changed,
        staged: s.staged,
        untracked: s.untracked,
        conflicted: s.conflicted,
        last,
    }
}

/// How long ago, in the person's words — `3 days ago`. Pure but for the
/// words.
#[must_use]
pub fn age(at_s: i64, now_s: i64, texts: &ronoctua_i18n::Texts) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;
    let gone = now_s.saturating_sub(at_s).max(0);
    let (key, n) = if gone < MINUTE {
        return texts.t(crate::key::FILES_GIT_AGE_NOW);
    } else if gone < HOUR {
        (crate::key::FILES_GIT_AGE_MINUTES, gone / MINUTE)
    } else if gone < DAY {
        (crate::key::FILES_GIT_AGE_HOURS, gone / HOUR)
    } else if gone < MONTH {
        (crate::key::FILES_GIT_AGE_DAYS, gone / DAY)
    } else if gone < YEAR {
        (crate::key::FILES_GIT_AGE_MONTHS, gone / MONTH)
    } else {
        (crate::key::FILES_GIT_AGE_YEARS, gone / YEAR)
    };
    texts.t_args(key, &[("n", Arg::Number(n))])
}

/// The sentence a refusal of git is — `files-git-why-<id>`.
#[must_use]
pub fn why_said(why: ronoctua_git::Why, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(&format!("files-git-why-{}", why.id()))
}

/// What the line says about the head: the branch's name, `detached at …`,
/// or `no commits yet on …`.
#[must_use]
pub fn head_said(head: &Head, texts: &ronoctua_i18n::Texts) -> String {
    match head {
        Head::Branch(b) => b.clone(),
        Head::Detached(id) => texts.t_with(crate::key::FILES_GIT_DETACHED, &[("id", id)]),
        Head::Unborn(b) => texts.t_with(crate::key::FILES_GIT_UNBORN, &[("branch", b)]),
    }
}

/// The counts, as the words the line draws, each with its tone — none at
/// all for a clean tree, which says so instead.
#[must_use]
pub fn counts(state: &State, texts: &ronoctua_i18n::Texts) -> Vec<(String, tokens::Color)> {
    let mut out = Vec::new();
    for (n, key, tone) in [
        (
            state.conflicted,
            crate::key::FILES_GIT_CONFLICTED,
            tokens::color::STATE_ERROR,
        ),
        (
            state.changed,
            crate::key::FILES_GIT_CHANGED,
            tokens::color::STATE_WARNING,
        ),
        (
            state.staged,
            crate::key::FILES_GIT_STAGED,
            tokens::color::STATE_SUCCESS,
        ),
        (
            state.untracked,
            crate::key::FILES_GIT_UNTRACKED,
            tokens::color::TEXT_SECONDARY,
        ),
    ] {
        if n > 0 {
            out.push((texts.t_args(key, &[("n", Arg::Number(i64::from(n)))]), tone));
        }
    }
    if out.is_empty() {
        out.push((
            texts.t(crate::key::FILES_GIT_CLEAN),
            tokens::color::TEXT_DIM,
        ));
    }
    out
}

/// The line's row, from cell `from`, `room` cells wide: every piece is the
/// line's one target (`Hit::Git`), and the pointer resting on it says what
/// the line is and which key opens the history.
///
/// THE PIECES GIVE WAY FROM THE END: the subject first — it is the longest
/// and the one a person least needs to find the branch — then its age, then
/// the counts; the branch is cut last, and only when nothing else is left.
#[must_use]
pub fn row(
    line: &Line,
    from: u16,
    room: u16,
    now_s: i64,
    keys: &str,
    texts: &ronoctua_i18n::Texts,
) -> Row {
    let explain = Some(explained(line, keys, texts));
    let mut pieces: Vec<(String, tokens::Color)> = vec![(
        tokens::icon::GIT_BRANCH.to_string(),
        tokens::color::TEXT_SECONDARY,
    )];
    match &line.read {
        Err(why) => pieces.push((why_said(*why, texts), tokens::color::TEXT_DIM)),
        Ok(state) => {
            pieces.push((head_said(&state.head, texts), tokens::color::TEXT_PRIMARY));
            if let Some((ahead, behind)) = state.upstream {
                pieces.push((
                    format!(
                        "{}{ahead} {}{behind}",
                        tokens::mark::ARROW_UP,
                        tokens::mark::ARROW_DOWN
                    ),
                    if behind > 0 {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                ));
            }
            pieces.push((tokens::mark::SEPARATOR.to_string(), tokens::color::TEXT_DIM));
            let said = counts(state, texts);
            let last = said.len();
            for (n, (text, tone)) in said.into_iter().enumerate() {
                let text = if n + 1 < last {
                    texts.t_with(crate::key::FILES_GIT_LIST_ITEM, &[("item", &text)])
                } else {
                    text
                };
                pieces.push((text, tone));
            }
            if let Some((subject, at_s)) = &state.last {
                pieces.push((tokens::mark::SEPARATOR.to_string(), tokens::color::TEXT_DIM));
                pieces.push((
                    texts.t_with(crate::key::FILES_GIT_SUBJECT, &[("subject", subject)]),
                    tokens::color::TEXT_SECONDARY,
                ));
                pieces.push((age(*at_s, now_s, texts), tokens::color::TEXT_DIM));
            }
        }
    }
    Row {
        fields: placed(pieces, from, room, explain.as_ref()),
        marked: false,
        foot: false,
        focused: false,
    }
}

/// Pieces laid out from `from`, one cell of air between each, inside `room`:
/// whole pieces from the start. The glyph and the head are cut to what is
/// left when they do not fit; any other piece that does not fit is dropped
/// with everything after it — three letters of a subject say nothing.
fn placed(
    pieces: Vec<(String, tokens::Color)>,
    from: u16,
    room: u16,
    explain: Option<&Explanation>,
) -> Vec<Field> {
    /// The glyph and the head: what the line is about.
    const CUT_ONLY: usize = 2;
    let end = from.saturating_add(room);
    let mut at = from;
    let mut out = Vec::new();
    let widths: Vec<u16> = pieces.iter().map(|(t, _)| width_of(t)).collect();
    let separator = tokens::mark::SEPARATOR.to_string();
    for (n, (text, color)) in pieces.into_iter().enumerate() {
        if at >= end {
            break;
        }
        // A SEPARATOR IS ONLY DRAWN BEFORE WHAT IT SEPARATES: with nothing
        // whole after it, it would be a dot hanging at the end of the line.
        if text == separator
            && widths
                .get(n + 1)
                .is_none_or(|next| at + width_of(&text) + 1 + next > end)
        {
            break;
        }
        let left = end - at;
        let text = if width_of(&text) > left {
            if n >= CUT_ONLY {
                break;
            }
            ronoctua_render::layout::elide_end(&text, left)
        } else {
            text
        };
        if text.is_empty() {
            break;
        }
        let wide = width_of(&text);
        out.push(Field {
            at,
            text,
            color,
            hit: Hit::Git,
            explain: explain.cloned(),
            ink: Ink::Grid,
        });
        at = at.saturating_add(wide).saturating_add(1);
    }
    out
}

/// What the pointer resting on the line says.
fn explained(line: &Line, keys: &str, texts: &ronoctua_i18n::Texts) -> Explanation {
    match &line.read {
        Ok(state) => Explanation {
            key: crate::key::FILES_GIT_LINE_EXPLAIN,
            args: vec![
                ("branch", head_said(&state.head, texts)),
                ("changed", state.changed.to_string()),
                ("staged", state.staged.to_string()),
                ("untracked", state.untracked.to_string()),
                ("keys", keys.to_owned()),
            ],
        },
        Err(_) => Explanation {
            key: crate::key::FILES_GIT_LINE_REFUSED,
            args: vec![("keys", keys.to_owned())],
        },
    }
}

/// How a reading gets back to the door that asked: `false` when the door
/// has gone, which ends the thread.
type Send = dyn Fn(Line) -> bool + std::marker::Send + Sync;

/// Whether a reading is under way, whether another is owed after it — and
/// the repository followed NOW, which is what every reading reads.
///
/// THE REPOSITORY LIVES HERE, not in the reading's thread: a folder left for
/// another repository while a reading of the first is under way owes a
/// reading of the SECOND, and a thread that kept the repository it was
/// started with would read the first again and leave the second without its
/// line.
#[derive(Debug, Default)]
struct Pace {
    running: bool,
    owed: bool,
    repo: Option<ronoctua_git::Repo>,
}

/// The door's hold on the line: the repository the folder on screen is in,
/// the watch on its git files, and the one reading at a time.
pub struct Keeper {
    /// The repository followed, and how git is run in it.
    repo: Option<ronoctua_git::Repo>,
    /// Its git files, reporting themselves.
    watch: Option<ronoctua_files::notice::Notice>,
    /// How a reading gets back.
    send: Arc<Send>,
    /// One reading at a time, one more when asked during it, and the
    /// repository it reads.
    pace: Arc<Mutex<Pace>>,
}

impl std::fmt::Debug for Keeper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keeper")
            .field("repo", &self.repo.as_ref().map(|r| &r.root))
            .finish_non_exhaustive()
    }
}

/// Where git writes what moves a repository's line and its history.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Watched {
    /// Watched whole, and only themselves: the git directory (`HEAD`, the
    /// index, `packed-refs`, `FETCH_HEAD`) and, for a linked work tree, the
    /// common directory its `commondir` names.
    pub dirs: Vec<PathBuf>,
    /// Watched with every folder under them, the ones made later included:
    /// each of those directories' `refs` — a branch called `feature/x` is a
    /// folder `feature` that did not exist before it was made, and a stash is
    /// `refs/stash`.
    pub trees: Vec<PathBuf>,
}

/// The places whose changes move the line and the history, for the git
/// directory `git_dir` — only those that are there.
#[must_use]
pub fn watched(git_dir: &Path) -> Watched {
    let mut roots = vec![git_dir.to_path_buf()];
    if let Ok(common) = std::fs::read_to_string(git_dir.join("commondir")) {
        let common = git_dir.join(common.trim());
        if common != git_dir {
            roots.push(common);
        }
    }
    let trees = roots
        .iter()
        .map(|r| r.join("refs"))
        .filter(|d| d.is_dir())
        .collect();
    roots.retain(|d| d.is_dir());
    Watched { dirs: roots, trees }
}

/// A watch on the places [`watched`] names, `on_stir` told of every change —
/// coalesced: an index rewritten, a ref moved and a log appended are one
/// commit. `extra` are more directories watched whole (the browser's: the
/// work tree's top folder).
///
/// # Errors
///
/// As [`ronoctua_files::notice::Notice::on_dirs`].
pub fn watch(
    git_dir: &Path,
    extra: &[PathBuf],
    on_stir: impl Fn() -> bool + std::marker::Send + 'static,
) -> anyhow::Result<ronoctua_files::notice::Notice> {
    let mut w = watched(git_dir);
    w.dirs.extend(extra.iter().cloned());
    ronoctua_files::notice::Notice::on_dirs(
        &w.dirs,
        &w.trees,
        ronoctua_files::notice::Pace::Coalesced,
        on_stir,
    )
}

impl Keeper {
    /// A keeper that hands every reading to `send`.
    pub fn new(send: impl Fn(Line) -> bool + std::marker::Send + Sync + 'static) -> Self {
        Self {
            repo: None,
            watch: None,
            send: Arc::new(send),
            pace: Arc::new(Mutex::new(Pace::default())),
        }
    }

    /// Follows the folder on screen: when it is in another repository — or
    /// in one after none — the watch moves there and a reading is asked.
    /// When it is in the same one, nothing: calling it after every key is
    /// a comparison of two paths.
    pub fn follow(&mut self, dir: &Path) {
        let found = ronoctua_git::Repo::discover(dir);
        let same = match (&self.repo, &found) {
            (Some(a), Some(b)) => a.root == b.root && a.git_dir == b.git_dir,
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.watch = None;
        self.repo = found;
        // WHAT THE NEXT READING READS — the one under way included, when it
        // loops for the reading it owes.
        if let Ok(mut p) = self.pace.lock() {
            p.repo.clone_from(&self.repo);
        }
        let Some(repo) = self.repo.as_ref() else {
            return;
        };
        let pace = Arc::clone(&self.pace);
        let send = Arc::clone(&self.send);
        self.watch = match watch(&repo.git_dir, &[], move || {
            ask(&pace, &send);
            true
        }) {
            Ok(n) => Some(n),
            // NOT SILENT: the line still reads when the folder stirs, and
            // what is lost is the news from the git directory itself.
            Err(e) => {
                tracing::warn!(error = %e, root = %repo.root.display(), "no watch on the git directory");
                None
            }
        };
        self.nudge();
    }

    /// Asks a reading of the repository followed — at once, or after the
    /// one under way.
    pub fn nudge(&self) {
        ask(&self.pace, &self.send);
    }

    /// Stops following anything: the manager closed.
    pub fn rest(&mut self) {
        self.watch = None;
        self.repo = None;
        if let Ok(mut p) = self.pace.lock() {
            p.repo = None;
        }
    }
}

/// One reading of the repository followed now, or one more after the
/// reading under way. The thread reads until nothing is owed — taking the
/// repository afresh each time — so a burst is at most two readings.
fn ask(pace: &Arc<Mutex<Pace>>, send: &Arc<Send>) {
    {
        let Ok(mut p) = pace.lock() else {
            return;
        };
        if p.repo.is_none() {
            return;
        }
        if p.running {
            p.owed = true;
            return;
        }
        p.running = true;
    }
    let (held, send) = (Arc::clone(pace), Arc::clone(send));
    let started = std::thread::Builder::new()
        .name("git-line".to_owned())
        .spawn(move || {
            loop {
                let repo = {
                    let Ok(mut p) = held.lock() else {
                        return;
                    };
                    p.owed = false;
                    let Some(repo) = p.repo.clone() else {
                        p.running = false;
                        return;
                    };
                    repo
                };
                let heard = send(read(&repo));
                let Ok(mut p) = held.lock() else {
                    return;
                };
                if heard && p.owed {
                    continue;
                }
                p.running = false;
                p.owed = false;
                return;
            }
        });
    if let Err(e) = started {
        tracing::warn!(error = %e, "the git line's thread could not start");
        if let Ok(mut p) = pace.lock() {
            p.running = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).unwrap()
    }

    fn a_state() -> State {
        State {
            head: Head::Branch("main".to_owned()),
            upstream: Some((2, 1)),
            changed: 3,
            staged: 1,
            untracked: 0,
            conflicted: 0,
            last: Some(("Make the line say where HEAD is".to_owned(), 1_700_000_000)),
        }
    }

    fn line() -> Line {
        Line {
            root: PathBuf::from("/r"),
            read: Ok(a_state()),
        }
    }

    fn text_of(r: &Row) -> String {
        r.fields
            .iter()
            .map(|f| f.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn the_line_says_the_branch_the_counts_and_the_last_commit() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = row(&line(), 0, 200, 1_700_000_000 + 3 * 86_400, "C-h", &t);
            let said = text_of(&r);
            assert!(said.contains("main"), "{said}");
            assert!(said.contains("↑2 ↓1"), "{said}");
            assert!(said.contains('3') && said.contains('1'), "{said}");
            assert!(said.contains("Make the line say"), "{said}");
            assert!(said.contains('3'), "{said}");
            // EVERY PIECE IS THE LINE'S ONE TARGET, and it explains itself.
            assert!(r.fields.iter().all(|f| f.hit == Hit::Git));
            assert!(r.fields.iter().all(|f| f.explain.is_some()));
            // NO RAW KEY on the screen.
            assert!(!said.contains("files-git"), "{said}");
        }
    }

    #[test]
    fn a_narrow_line_gives_way_from_the_end_and_never_leaks() {
        let t = texts("en-GB");
        let wide = row(&line(), 4, 200, 1_700_000_000, "C-h", &t);
        for room in [0_u16, 1, 5, 12, 20, 40] {
            let r = row(&line(), 4, room, 1_700_000_000, "C-h", &t);
            for f in &r.fields {
                assert!(
                    f.at + width_of(&f.text) <= 4 + room,
                    "{room}: {:?} leaks past the room",
                    f.text
                );
            }
            // THE BRANCH GOES LAST: whatever survives is a prefix of the wide
            // line's pieces.
            for (a, b) in r.fields.iter().zip(&wide.fields) {
                assert_eq!(a.at, b.at);
            }
        }
    }

    #[test]
    fn a_clean_tree_says_so_and_a_refusal_is_a_sentence() {
        let t = texts("en-GB");
        let mut s = a_state();
        (s.changed, s.staged) = (0, 0);
        let said = counts(&s, &t);
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].0, t.t(crate::key::FILES_GIT_CLEAN));
        for why in ronoctua_git::Why::ALL {
            let r = row(
                &Line {
                    root: PathBuf::from("/r"),
                    read: Err(why),
                },
                0,
                120,
                0,
                "C-h",
                &t,
            );
            let said = text_of(&r);
            assert!(!said.contains("files-git-why"), "{why:?}: {said}");
        }
    }

    #[test]
    fn heads_are_named_whatever_state_they_are_in() {
        let mut s = ronoctua_git::Status {
            branch: Some("main".to_owned()),
            head: None,
            ..Default::default()
        };
        assert_eq!(state_of(&s, None).head, Head::Unborn("main".to_owned()));
        s.head = Some(ronoctua_git::Oid(
            "0123456789abcdef0123456789abcdef01234567".to_owned(),
        ));
        assert_eq!(state_of(&s, None).head, Head::Branch("main".to_owned()));
        s.branch = None;
        assert_eq!(
            state_of(&s, None).head,
            Head::Detached("0123456".to_owned())
        );
        assert_eq!(state_of(&s, None).upstream, None, "no upstream, no arrows");
    }

    #[test]
    fn an_age_is_counted_in_the_largest_whole_unit() {
        let t = texts("en-GB");
        let now = 1_700_000_000;
        assert_eq!(age(now, now, &t), t.t(crate::key::FILES_GIT_AGE_NOW));
        let days = age(now - 3 * 86_400, now, &t);
        assert!(days.contains('3'), "{days}");
        let one = age(now - 3_600, now, &t);
        assert!(one.contains('1') || !one.contains('2'), "{one}");
        assert_ne!(age(now - 400 * 86_400, now, &t), days);
        // A CLOCK A LITTLE BEHIND is not a commit from the future.
        assert_eq!(age(now + 30, now, &t), t.t(crate::key::FILES_GIT_AGE_NOW));
    }

    #[test]
    fn the_watch_follows_every_place_git_writes_a_ref_and_skips_the_absent() {
        let root = std::env::temp_dir().join(format!("ronoctua-git-line-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let git = root.join(".git");
        std::fs::create_dir_all(git.join("refs/heads")).unwrap();
        std::fs::create_dir_all(git.join("refs/remotes/origin")).unwrap();
        let w = watched(&git);
        assert_eq!(w.dirs, std::slice::from_ref(&git));
        // THE REFS ARE A TREE: `refs/remotes/origin/feature/x` is a folder
        // deeper than any listed once (`Notice::on_dirs` holds them all).
        assert_eq!(w.trees, [git.join("refs")]);
        // A LINKED WORK TREE: its own directory and the common one.
        let linked = git.join("worktrees/other");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join("commondir"), "../..\n").unwrap();
        let w = watched(&linked);
        assert_eq!(w.dirs, [linked.clone(), linked.join("../..")]);
        assert_eq!(
            w.trees,
            [linked.join("../../refs")],
            "the linked tree has no refs of its own"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A folder with a `.git` that is not a repository: reading it is a
    /// refusal, and that is still one reading about its root — found first
    /// by the walk up, so no repository around the scratch folder is read.
    fn not_a_repository(name: &str) -> ronoctua_git::Repo {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-git-line-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let repo = ronoctua_git::Repo::discover(&dir).unwrap();
        assert_eq!(repo.root, dir);
        repo
    }

    /// A pace following `repo`, and a send held shut until the test lets go,
    /// that says when a reading has reached it and writes down the root of
    /// every reading it is handed.
    struct Held {
        pace: Arc<Mutex<Pace>>,
        gate: Arc<Mutex<()>>,
        reached: Arc<std::sync::atomic::AtomicUsize>,
        roots: Arc<Mutex<Vec<PathBuf>>>,
        send: Arc<Send>,
    }

    fn a_held_pace(repo: &ronoctua_git::Repo) -> Held {
        let pace = Arc::new(Mutex::new(Pace {
            repo: Some(repo.clone()),
            ..Pace::default()
        }));
        let gate = Arc::new(Mutex::new(()));
        let reached = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let roots = Arc::new(Mutex::new(Vec::new()));
        let (r, g, n) = (Arc::clone(&roots), Arc::clone(&gate), Arc::clone(&reached));
        let send: Arc<Send> = Arc::new(move |line: Line| {
            n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Held until the test lets go: what is asked meanwhile lands
            // while this reading is under way.
            drop(g.lock());
            r.lock().unwrap().push(line.root);
            true
        });
        Held {
            pace,
            gate,
            reached,
            roots,
            send,
        }
    }

    /// Waits until `n` readings have reached the send.
    fn reached(h: &Held, n: usize) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while h.reached.load(std::sync::atomic::Ordering::SeqCst) < n
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn quiet(pace: &Arc<Mutex<Pace>>) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while pace.lock().unwrap().running && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn a_burst_of_nudges_is_one_reading_under_way_and_one_after() {
        let repo = not_a_repository("burst");
        let h = a_held_pace(&repo);
        let held = h.gate.lock().unwrap();
        ask(&h.pace, &h.send);
        reached(&h, 1);
        for _ in 0..10 {
            ask(&h.pace, &h.send);
        }
        drop(held);
        quiet(&h.pace);
        assert_eq!(
            h.roots.lock().unwrap().len(),
            2,
            "ten nudges during one reading are one more, not ten"
        );
        let _ = std::fs::remove_dir_all(&repo.root);
    }

    #[test]
    // THE DEFECT THIS CATCHES: the reading owed after the one under way read
    // the repository THAT one was started with. A folder of a second
    // repository entered while the first was being read never got its line.
    fn a_reading_owed_after_another_is_of_the_repository_followed_now() {
        let first = not_a_repository("first");
        let second = not_a_repository("second");
        let h = a_held_pace(&first);
        let held = h.gate.lock().unwrap();
        ask(&h.pace, &h.send);
        reached(&h, 1);
        // THE FOLDER MOVES to the second repository mid-reading, as
        // `Keeper::follow` moves it, and asks.
        h.pace.lock().unwrap().repo = Some(second.clone());
        ask(&h.pace, &h.send);
        drop(held);
        quiet(&h.pace);
        assert_eq!(
            *h.roots.lock().unwrap(),
            [first.root.clone(), second.root.clone()],
            "the reading owed read the repository that was left"
        );
        // AND A KEEPER AT REST reads nothing more.
        h.pace.lock().unwrap().repo = None;
        ask(&h.pace, &h.send);
        quiet(&h.pace);
        assert_eq!(h.roots.lock().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&first.root);
        let _ = std::fs::remove_dir_all(&second.root);
    }
}
