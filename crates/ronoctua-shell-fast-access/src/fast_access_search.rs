//! FAST-ACCESS'S THREADS — the index on one thread where the last question
//! wins, and the network after a pause, one thread per source, each stopped
//! by the next key.
//!
//! # One feed per door
//!
//! A door (the footer's region, the window, the column) makes a [`Feed`] when
//! the view opens and drops it when the view closes: dropping it ends every
//! thread it started and stops every request in flight — nothing of the
//! index stays mapped behind a closed view (the footer lives for months), and
//! nothing keeps talking to a host for a box that is gone. What a thread
//! answers comes back through the door's own channel ([`Back`]), on its loop.
//!
//! # The index: the last question wins
//!
//! One thread holds the reader ([`ronoctua_index::Index`], opened read-only
//! with no clock of its own) and ONE slot for the question. A key replaces
//! what is in the slot; the thread takes whatever is there when it is free,
//! so ten keys typed while a query ran cost one more query, not ten. The
//! writer's commit is an event (`on_commit`, inotify on `meta.json`): the
//! reader reloads and the question on screen is asked again — what arrives
//! slots into its section without moving the page.
//!
//! # The network: after a pause, and stopped by the next key
//!
//! The scopes that ask a host (`Scope::online`) are asked [`ONLINE_PAUSE`]
//! after the last key — a clock that exists only while there is a question
//! waiting for its pause. Each source runs on a thread of its own with a stop
//! ([`Stops`]): the next key, the view closing or the feed dropped pull every
//! one — the HTTP sockets are shut (`web::Abort`) and yt-dlp is killed
//! (`ronoctua_ytdlp::Stop`). Nothing is asked of a host the person has not
//! let: the section draws the consent row instead (`consent_rows`).

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Instant;

use crate::fast_access::{
    Answer, Card, Filled, Hit, ONLINE_PAUSE, Opened, Query, Said, Scope, SectionId, Settings,
    Video, What,
};

/// What the index and the network are asked for one generation.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// Of which generation of what is typed.
    pub generation: u64,
    /// What it is narrowed to.
    pub scope: Scope,
    /// The words.
    pub words: String,
    /// A translation's language, when `/t` named one.
    pub tongue: Option<String>,
    /// The reader's language subtag (`en`, `pt`): Wikipedia's, and the
    /// words' language.
    pub subtag: String,
    /// Shorts kept among YouTube's results.
    pub shorts: bool,
    /// The person's SearXNG, when they named one.
    pub searxng: Option<String>,
    /// What was opened from here, as the index weighs it.
    pub history: Vec<(String, f32)>,
}

/// The registry's commands — those that need no argument a sequence would
/// carry — with the keys that run each, from the default set
/// (`bindings::taught_keys`).
#[must_use]
pub fn commands_of(
    trees: &crate::bindings::Trees,
    registry: &ronoctua_cmd::Registry,
) -> Vec<(&'static str, &'static str, Option<String>)> {
    registry
        .all_ones()
        .filter(|c| crate::actions::with_argument(c.id).is_none())
        .map(|c| {
            (
                c.id,
                c.label,
                crate::bindings::taught_keys(trees, c.id, None),
            )
        })
        .collect()
}

impl Ask {
    /// An ask for nothing: what an empty box sends — every thread's answer
    /// for an older generation is dropped, and every request stopped.
    #[must_use]
    pub fn nothing(generation: u64) -> Self {
        Self {
            generation,
            scope: Scope::All,
            words: String::new(),
            tongue: None,
            subtag: String::new(),
            shorts: false,
            searxng: None,
            history: Vec::new(),
        }
    }

    /// The ask for `query`, with what the settings add.
    #[must_use]
    pub fn of(
        generation: u64,
        query: &Query,
        settings: &Settings,
        subtag: &str,
        recent: &[Opened],
    ) -> Self {
        let now = ronoctua_index::now_s();
        Self {
            generation,
            scope: query.scope,
            words: query.words.clone(),
            tongue: query.tongue.clone(),
            subtag: subtag.to_owned(),
            shorts: settings.shorts,
            searxng: settings.searxng.clone(),
            history: recent
                .iter()
                .filter_map(|o| {
                    index_id_of(&o.id).map(|id| (id, crate::fast_access::frecency(o, now)))
                })
                .collect(),
        }
    }

    /// Whether the index (or the local catalogue, or the zones) answers it.
    #[must_use]
    pub fn wants_local(&self) -> bool {
        !self.words.trim().is_empty()
            && matches!(
                self.scope,
                Scope::All
                    | Scope::Audio
                    | Scope::Video
                    | Scope::Images
                    | Scope::Documents
                    | Scope::Names
                    | Scope::Contents
                    | Scope::Manuals
                    | Scope::Programs
                    | Scope::YouTube
                    | Scope::Zones
            )
    }

    /// Whether a host is asked, after the pause: every scope that goes
    /// online, and "everything" — which asks only the hosts the person
    /// already let (`fast_access_online::ask`).
    #[must_use]
    pub fn wants_online(&self) -> bool {
        !self.words.trim().is_empty() && (self.scope.online() || self.scope == Scope::All)
    }
}

/// The index's id of a hit's id, when the index holds such a hit.
fn index_id_of(hit: &str) -> Option<String> {
    let (kind, rest) = hit.split_once(':')?;
    match kind {
        "file" | "folder" | "content" => Some(rest.to_owned()),
        "yt" | "man" | "pkg" => Some(hit.to_owned()),
        _ => None,
    }
}

/// How many lines of a manual page its detail shows.
pub const MAN_LINES: usize = 40;

/// What a thread hands back to the door's loop.
#[derive(Debug)]
pub enum Back {
    /// A section's answer.
    Answer(Answer),
    /// The icon tables, read — or not.
    Icons(Result<Arc<ronoctua_iconset::Set>, ()>),
    /// A picture's pixels, decoded, under its key; the video it is of.
    Picture {
        /// The video's id.
        video: String,
        /// Its key on the GPU.
        key: String,
        /// Its pixels.
        rgba: Arc<ronoctua_render::image::Rgba>,
    },
    /// The exchange rates arrived (or were already on disk).
    Rates(ronoctua_calc::rates::Rates),
    /// The exchange rates did not come, for a generation — and the older
    /// ones on disk, when there are any.
    RatesFailed {
        /// Of which generation of what is typed.
        generation: u64,
        /// Why.
        why: ronoctua_calc::rates::Why,
        /// The rates on disk, however old.
        stale: Option<ronoctua_calc::rates::Rates>,
    },
    /// The launcher's applications, scanned.
    Apps(crate::launch::Catalogue),
    /// A manual page's first lines.
    Man {
        /// `name.section`.
        page: String,
        /// Its first lines, its roff taken off.
        lines: Vec<String>,
    },
    /// A change to a file was written — or why not.
    Amended(crate::report::Amended),
    /// What a program to install is (`ronoctua_software::details`), and
    /// what installing it would ask.
    Program {
        /// Its attribute.
        attr: String,
        /// What was read.
        program: Box<Program>,
    },
    /// A folder's size, walked.
    Measured(PathBuf, u64),
}

/// A program to install, as the detail shows it and the install asks it.
#[derive(Debug, Clone)]
pub struct Program {
    /// Its details, from the programs' own engine — or why there are none.
    pub details: Result<ronoctua_software::details::Details, ronoctua_software::Refusal>,
    /// Whether the person already let this one's licence
    /// (`ronoctua_software::prefs`, the programs' centre's own list).
    pub unfree_let: bool,
    /// The download, in megabytes, past which the person is asked first —
    /// nought: never (the centre's own threshold).
    pub ask_above_mb: u64,
}

/// How a feed reaches its door: whatever calls it is on a thread.
pub type Sink = Arc<dyn Fn(Back) + Send + Sync>;

/// Every stop of ONE generation's requests in flight, pulled together.
///
/// Its clones are ONE set: the generation's thread, the threads it starts and
/// the feed that pulls them hold the same list and the same flag. A stop made
/// after the set was pulled is pulled as it is made — so a request started a
/// moment after the next key, by a thread that had already taken the old
/// question, still ends with it.
#[derive(Debug, Default, Clone)]
pub struct Stops {
    inner: Arc<Mutex<Pulls>>,
}

/// What a [`Stops`] holds.
#[derive(Debug, Default)]
struct Pulls {
    web: Vec<ronoctua_http::web::Abort>,
    ytdlp: Vec<ronoctua_ytdlp::Stop>,
    pulled: bool,
}

impl Stops {
    /// A web request's stop, kept to be pulled — pulled already when the set
    /// was.
    #[must_use]
    pub fn web(&self) -> ronoctua_http::web::Abort {
        let a = ronoctua_http::web::Abort::new();
        let mut p = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if p.pulled {
            a.abort();
        } else {
            p.web.push(a.clone());
        }
        a
    }

    /// A yt-dlp run's stop, kept to be pulled — pulled already when the set
    /// was.
    #[must_use]
    pub fn ytdlp(&self) -> ronoctua_ytdlp::Stop {
        let s = ronoctua_ytdlp::Stop::new();
        let mut p = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if p.pulled {
            s.stop();
        } else {
            p.ytdlp.push(s.clone());
        }
        s
    }

    /// Whether these were pulled.
    #[must_use]
    pub fn pulled(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pulled
    }

    /// Pulls every one: sockets shut, children killed, and the threads told.
    pub fn pull(&self) {
        let mut p = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        p.pulled = true;
        for a in p.web.drain(..) {
            a.abort();
        }
        for s in p.ytdlp.drain(..) {
            s.stop();
        }
    }
}

/// The slot the index thread takes its question from: the last one wins.
#[derive(Default)]
struct Slot {
    ask: Option<Ask>,
    /// The writer committed: reload, and ask again.
    reload: bool,
    /// The feed is gone: the thread ends.
    done: bool,
}

/// The online scheduler's slot: a question waiting for its pause, with the
/// stops of ITS generation — taken together, under one lock, so a thread can
/// never start an old question with a newer key's stops.
#[derive(Default)]
struct Waiting {
    ask: Option<(Ask, Instant, Stops)>,
    done: bool,
}

/// What the online thread does with a question whose pause is over: every
/// source of its scope started, each stop made from the generation's
/// [`Stops`] (`fast_access_online::ask`; a test hands its own).
type Starter = Arc<dyn Fn(&Ask, &Stops, &Sink) + Send + Sync>;

/// The threads of one door's search.
pub struct Feed {
    sink: Sink,
    local: Arc<(Mutex<Slot>, Condvar)>,
    online: Arc<(Mutex<Waiting>, Condvar)>,
    /// The stops of the generation asked last.
    stops: Arc<Mutex<Stops>>,
    /// How long the typing pauses before a host is asked.
    pause: std::time::Duration,
    /// The commit watch, while the index thread runs.
    _commits: Option<ronoctua_files::notice::Notice>,
    /// Whether to answer on the caller's thread (a photograph, a test).
    inline: bool,
    /// The index an inline feed reads — a photograph's.
    inline_dir: Option<PathBuf>,
}

impl std::fmt::Debug for Feed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feed")
            .field("inline", &self.inline)
            .finish_non_exhaustive()
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        // EVERY THREAD ENDS AND EVERY REQUEST STOPS with the view.
        self.stops
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pull();
        let (slot, wake) = &*self.local;
        slot.lock().unwrap_or_else(PoisonError::into_inner).done = true;
        wake.notify_all();
        let (waiting, wake) = &*self.online;
        waiting.lock().unwrap_or_else(PoisonError::into_inner).done = true;
        wake.notify_all();
    }
}

impl Feed {
    /// The threads, started: the icon tables read, the index's reader on its
    /// thread with its commit watch, and the online scheduler waiting.
    #[must_use]
    pub fn start(sink: Sink) -> Self {
        let local = Arc::new((Mutex::new(Slot::default()), Condvar::new()));
        let online = Arc::new((Mutex::new(Waiting::default()), Condvar::new()));
        let stops = Arc::new(Mutex::new(Stops::default()));
        // THE TABLES, ~9 MB, on a thread of their own.
        let icons = Arc::clone(&sink);
        let _ = std::thread::Builder::new()
            .name("fast-access-icons".into())
            .spawn(move || icons(Back::Icons(ronoctua_iconset::shared().map_err(|_| ()))));
        // THE WRITER'S COMMIT, heard: reload and ask again.
        let commits = ronoctua_index::default_dir().and_then(|dir| {
            let local = Arc::clone(&local);
            ronoctua_index::on_commit(&dir, move || {
                let (slot, wake) = &*local;
                let mut s = slot.lock().unwrap_or_else(PoisonError::into_inner);
                s.reload = true;
                wake.notify_all();
                !s.done
            })
            .map_err(|r| {
                tracing::info!(
                    why = r.why.id(),
                    "fast-access: the index's commits are not watched"
                )
            })
            .ok()
        });
        {
            let local = Arc::clone(&local);
            let sink = Arc::clone(&sink);
            let _ = std::thread::Builder::new()
                .name("fast-access-index".into())
                .spawn(move || local_thread(&local, &sink));
        }
        spawn_online(
            &online,
            &sink,
            Arc::new(|ask: &Ask, stops: &Stops, sink: &Sink| {
                crate::fast_access_online::ask(ask, stops, sink);
            }),
        );
        Self {
            sink,
            local,
            online,
            stops,
            pause: ONLINE_PAUSE,
            _commits: commits,
            inline: false,
            inline_dir: None,
        }
    }

    /// A feed that answers on the CALLER's thread, at once, and asks no host
    /// — what a photograph draws (`crate::fast_access_photograph`), whose
    /// frame is taken after the answers are in — reading the index in `dir`
    /// (one built for it), or its own place when `None`.
    #[must_use]
    pub fn inline_at(sink: Sink, dir: Option<PathBuf>) -> Self {
        sink(Back::Icons(ronoctua_iconset::shared().map_err(|_| ())));
        Self {
            sink,
            local: Arc::new((Mutex::new(Slot::default()), Condvar::new())),
            online: Arc::new((Mutex::new(Waiting::default()), Condvar::new())),
            stops: Arc::new(Mutex::new(Stops::default())),
            pause: ONLINE_PAUSE,
            _commits: None,
            inline: true,
            inline_dir: dir,
        }
    }

    /// A new generation: whatever was in flight for the last one is stopped,
    /// the index is asked (the slot replaced), and the network after the
    /// pause.
    pub fn ask(&self, ask: &Ask) {
        // THE NEXT KEY STOPS what the last one asked — and this one's stops
        // travel WITH its question to the online thread.
        let fresh = Stops::default();
        {
            let mut stops = self.stops.lock().unwrap_or_else(PoisonError::into_inner);
            stops.pull();
            *stops = fresh.clone();
        }
        if self.inline {
            if ask.wants_local() {
                let mut reader = Reader {
                    dir: self.inline_dir.clone(),
                    ..Reader::default()
                };
                answer_local(&mut reader, ask, &self.sink);
            }
            return;
        }
        {
            let (slot, wake) = &*self.local;
            let mut s = slot.lock().unwrap_or_else(PoisonError::into_inner);
            s.ask = ask.wants_local().then(|| ask.clone());
            wake.notify_all();
        }
        let (waiting, wake) = &*self.online;
        let mut w = waiting.lock().unwrap_or_else(PoisonError::into_inner);
        w.ask = ask
            .wants_online()
            .then(|| (ask.clone(), Instant::now() + self.pause, fresh));
        wake.notify_all();
    }

    /// A picture of the catalogue's, decoded off the loop.
    pub fn picture(&self, video: &str, file: PathBuf) {
        let sink = Arc::clone(&self.sink);
        let video = video.to_owned();
        let decode = move || {
            if let Some((key, rgba)) = crate::fast_access_videos::decoded(&file) {
                sink(Back::Picture { video, key, rgba });
            }
        };
        if self.inline {
            decode();
        } else {
            let _ = std::thread::Builder::new()
                .name("fast-access-picture".into())
                .spawn(decode);
        }
    }

    /// The launcher's applications, scanned on a thread in `language`.
    pub fn apps(&self, language: &str) {
        let sink = Arc::clone(&self.sink);
        // THE DESKTOP ENTRIES' LANGUAGE is the interface's, written as they
        // write it (`pt_BR`).
        let language = language.replace('-', "_");
        let scan = move || sink(Back::Apps(Arc::new(ronoctua_apps::scan(&language))));
        if self.inline {
            scan();
        } else {
            let _ = std::thread::Builder::new()
                .name("fast-access-apps".into())
                .spawn(scan);
        }
    }

    /// A manual page's first lines, read off the loop from the folders
    /// `man` itself would search.
    pub fn man(&self, name: &str, section: &str) {
        let sink = Arc::clone(&self.sink);
        let (name, section) = (name.to_owned(), section.to_owned());
        let read = move || {
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let state = std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .or_else(|| home.as_ref().map(|h| h.join(".local/state")));
            let user = std::env::var("USER").ok();
            let folders =
                ronoctua_index::man::candidates(user.as_deref(), home.as_deref(), state.as_deref());
            let lines: Vec<String> = ronoctua_index::man::text_of(&folders, &name, &section)
                .map(|text| {
                    text.lines()
                        .map(str::trim_end)
                        .filter(|l| !l.trim().is_empty())
                        .take(MAN_LINES)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            sink(Back::Man {
                page: format!("{name}.{section}"),
                lines,
            });
        };
        if self.inline {
            read();
        } else {
            let _ = std::thread::Builder::new()
                .name("fast-access-man".into())
                .spawn(read);
        }
    }

    /// What a program to install is, read off the loop the way the programs'
    /// centre reads it (`ronoctua_software::details::of`) — the index's row,
    /// its licence, its size — with the person's own answers about licences
    /// and big downloads.
    pub fn program(&self, attr: &str, language: &str) {
        let sink = Arc::clone(&self.sink);
        let (attr, language) = (attr.to_owned(), language.to_owned());
        let read = move || {
            let program = match ronoctua_software::Context::of_session() {
                Ok(ctx) => Program {
                    details: ronoctua_software::details::of(
                        &ctx,
                        &ronoctua_software::AppKey::Nix { attr: attr.clone() },
                        &language,
                    ),
                    unfree_let: ctx.prefs.unfree.contains(&attr),
                    ask_above_mb: ctx.prefs.ask_above_mb,
                },
                Err(_) => Program {
                    details: Err(ronoctua_software::Refusal::SourceAbsent),
                    unfree_let: false,
                    ask_above_mb: 0,
                },
            };
            sink(Back::Program {
                attr,
                program: Box::new(program),
            });
        };
        if self.inline {
            read();
        } else {
            let _ = std::thread::Builder::new()
                .name("fast-access-program".into())
                .spawn(read);
        }
    }

    /// A writer of a file's facts, on a thread of its own: the tag program
    /// and exiftool are children, and a remux is a copy's worth of time.
    pub fn amend(&self, work: impl FnOnce() -> crate::report::Amended + Send + 'static) {
        let sink = Arc::clone(&self.sink);
        let _ = std::thread::Builder::new()
            .name("fast-access-amend".into())
            .spawn(move || sink(Back::Amended(work())));
    }

    /// A folder walked for its size, on a thread.
    pub fn measure(&self, path: PathBuf) {
        let sink = Arc::clone(&self.sink);
        let _ = std::thread::Builder::new()
            .name("fast-access-measure".into())
            .spawn(move || {
                let bytes = ronoctua_files::act::weigh(&path);
                sink(Back::Measured(path, bytes));
            });
    }

    /// A feed whose only thread is the online scheduler, starting each
    /// question with `start` after `pause` — a test plays the sources' part.
    #[cfg(test)]
    pub(crate) fn online_only(sink: Sink, start: Starter, pause: std::time::Duration) -> Self {
        let online = Arc::new((Mutex::new(Waiting::default()), Condvar::new()));
        spawn_online(&online, &sink, start);
        Self {
            sink,
            local: Arc::new((Mutex::new(Slot::default()), Condvar::new())),
            online,
            stops: Arc::new(Mutex::new(Stops::default())),
            pause,
            _commits: None,
            inline: false,
            inline_dir: None,
        }
    }
}

/// The index's reader, the catalogue's place and the zones' names, held by
/// the index thread while the view is open.
#[derive(Default)]
pub struct Reader {
    /// Where the index is — its own place when `None` (a photograph names
    /// one built for it).
    dir: Option<PathBuf>,
    index: Option<ronoctua_index::Index>,
    refused: Option<ronoctua_index::Why>,
    zones: Option<Vec<String>>,
}

impl Reader {
    /// The reader, opened the first time it is wanted.
    fn index(&mut self) -> Result<&ronoctua_index::Index, ronoctua_index::Why> {
        if self.index.is_none() {
            let dir = self
                .dir
                .clone()
                .or_else(ronoctua_index::default_dir)
                .ok_or(ronoctua_index::Why::Unreadable)?;
            match ronoctua_index::Index::open_ro(&dir) {
                Ok(i) => {
                    self.index = Some(i);
                    self.refused = None;
                }
                Err(r) => {
                    self.refused = Some(r.why);
                    return Err(r.why);
                }
            }
        }
        self.index.as_ref().ok_or(ronoctua_index::Why::Unreadable)
    }

    /// The writer committed.
    fn reload(&mut self) {
        match self.index.as_mut() {
            Some(i) => {
                if i.reload().is_err() {
                    self.index = None;
                }
            }
            // NOT OPEN YET (the first commit of an index being built): the next
            // question opens it.
            None => self.refused = None,
        }
    }
}

/// The index thread: the slot's question, the last one, whenever there is
/// one — and nothing while there is none.
fn local_thread(local: &Arc<(Mutex<Slot>, Condvar)>, sink: &Sink) {
    let mut reader = Reader::default();
    let mut last: Option<Ask> = None;
    loop {
        let (ask, reload) = {
            let (slot, wake) = &**local;
            let mut s = slot.lock().unwrap_or_else(PoisonError::into_inner);
            while s.ask.is_none() && !s.reload && !s.done {
                s = wake.wait(s).unwrap_or_else(PoisonError::into_inner);
            }
            if s.done {
                return;
            }
            (s.ask.take(), std::mem::take(&mut s.reload))
        };
        if reload {
            reader.reload();
        }
        let ask = match ask {
            Some(a) => {
                last = Some(a.clone());
                a
            }
            // A COMMIT WITH NOTHING NEW TYPED asks the last question again.
            None => match &last {
                Some(a) if reload => a.clone(),
                _ => continue,
            },
        };
        answer_local(&mut reader, &ask, sink);
    }
}

/// The index's kinds and types for a scope.
fn index_query(ask: &Ask) -> Option<ronoctua_index::Query> {
    use ronoctua_index::Kind as K;
    let most = crate::fast_access::most_of(ask.scope, SectionId::Files);
    let words: Vec<String> = ask.words.split_whitespace().map(str::to_owned).collect();
    let (kinds, mimes, contents): (Vec<K>, Vec<&str>, bool) = match ask.scope {
        Scope::All => (
            vec![K::File, K::Folder, K::Content, K::Video, K::Man, K::Package],
            Vec::new(),
            true,
        ),
        Scope::Audio => (vec![K::File], vec!["audio/"], false),
        Scope::Video => (vec![K::File], vec!["video/"], false),
        Scope::Images => (vec![K::File], vec!["image/"], false),
        Scope::Documents => (
            vec![K::File, K::Content],
            vec![
                "application/pdf",
                "application/epub",
                "application/msword",
                "application/vnd.oasis.opendocument",
                "application/vnd.openxmlformats",
                "application/rtf",
                "text/",
            ],
            true,
        ),
        Scope::Names => (vec![K::File, K::Folder], Vec::new(), false),
        Scope::Contents => (vec![K::Content], Vec::new(), true),
        Scope::Manuals => (vec![K::Man], Vec::new(), true),
        Scope::Programs => (vec![K::Package], Vec::new(), true),
        _ => return None,
    };
    Some(ronoctua_index::Query {
        words,
        kinds,
        mime_prefixes: mimes.into_iter().map(str::to_owned).collect(),
        contents,
        most,
    })
}

/// The sections the index answers for a scope — each answered, even with
/// nothing, so none stays "searching".
fn index_sections(scope: Scope) -> Vec<SectionId> {
    crate::fast_access::SectionId::of(scope)
        .iter()
        .copied()
        .filter(|s| {
            matches!(
                s,
                SectionId::Files
                    | SectionId::Folders
                    | SectionId::Contents
                    | SectionId::Videos
                    | SectionId::Manuals
                    | SectionId::Packages
            )
        })
        .collect()
}

/// Asks what is local for `ask` and hands each section's answer back.
pub fn answer_local(reader: &mut Reader, ask: &Ask, sink: &Sink) {
    let answer = |section, filled| {
        sink(Back::Answer(Answer {
            generation: ask.generation,
            section,
            filled,
        }));
    };
    match ask.scope {
        Scope::YouTube => {
            answer(
                SectionId::YouTube,
                crate::fast_access_videos::local(&ask.words, ask.shorts),
            );
            return;
        }
        Scope::Zones => {
            let names = reader.zones.get_or_insert_with(zone_names);
            answer(SectionId::Zones, zones_section(names, &ask.words));
            return;
        }
        _ => {}
    }
    let Some(query) = index_query(ask) else {
        return;
    };
    let sections = index_sections(ask.scope);
    let hits = reader.index().and_then(|index| {
        let mut history = ronoctua_index::History::new();
        for (id, weight) in &ask.history {
            history = history.with(id.clone(), *weight);
        }
        index.query_with(&query, &history).map_err(|r| r.why)
    });
    match hits {
        Err(why) => {
            let said = Said::key(format!("fast-access-index-why-{}", why.id()));
            for s in sections {
                answer(s, Filled::Refused(said.clone()));
            }
        }
        Ok(hits) => {
            let mut by: std::collections::BTreeMap<SectionId, Vec<Hit>> =
                sections.iter().map(|s| (*s, Vec::new())).collect();
            let most = |s: SectionId| crate::fast_access::most_of(ask.scope, s);
            for h in hits {
                let (section, hit) = hit_of(h);
                if let Some(list) = by.get_mut(&section)
                    && list.len() < most(section)
                {
                    list.push(hit);
                }
            }
            for (s, list) in by {
                answer(s, Filled::Hits(list));
            }
        }
    }
}

/// An index hit as fast-access's, and its section.
#[must_use]
pub fn hit_of(h: ronoctua_index::Hit) -> (SectionId, Hit) {
    use ronoctua_index::Kind as K;
    let folder_of = |p: &std::path::Path| {
        p.parent()
            .map(crate::fast_access_rows::tilde)
            .filter(|s| !s.is_empty())
    };
    match h.kind {
        K::File | K::Folder | K::Content => {
            let path = h.path.clone().unwrap_or_else(|| PathBuf::from(&h.id));
            let (section, kind) = match h.kind {
                K::Folder => (SectionId::Folders, "folder"),
                K::Content => (SectionId::Contents, "content"),
                _ => (SectionId::Files, "file"),
            };
            (
                section,
                Hit {
                    id: format!("{kind}:{}", path.display()),
                    title: h.name,
                    note: folder_of(&path),
                    what: What::File {
                        folder: h.kind == K::Folder,
                        content: h.kind == K::Content,
                        mime: h.mime,
                        size_bytes: h.size_bytes,
                        mtime_s: h.mtime_s,
                        path,
                    },
                },
            )
        }
        K::Video => {
            let id = h.id.strip_prefix("yt:").unwrap_or(&h.id).to_owned();
            (
                SectionId::Videos,
                Hit {
                    id: format!("yt:{id}"),
                    title: h.name,
                    note: h.note.clone(),
                    what: What::Video(Box::new(Video {
                        url: format!("https://www.youtube.com/watch?v={id}"),
                        id,
                        channel: h.note,
                        duration_s: None,
                        views: None,
                        short: false,
                        local: true,
                        thumb_file: None,
                        thumb: None,
                    })),
                },
            )
        }
        K::Man => {
            let page = h.id.strip_prefix("man:").unwrap_or(&h.id);
            let (name, section) = page.rsplit_once('.').unwrap_or((page, ""));
            (
                SectionId::Manuals,
                Hit {
                    id: format!("man:{page}"),
                    title: format!("{name}({section})"),
                    note: h.note,
                    what: What::Man {
                        name: name.to_owned(),
                        section: section.to_owned(),
                    },
                },
            )
        }
        K::Package => {
            let attr = h.id.strip_prefix("pkg:").unwrap_or(&h.id).to_owned();
            (
                SectionId::Packages,
                Hit {
                    id: format!("pkg:{attr}"),
                    title: h.name,
                    note: h.note,
                    what: What::Package { attr },
                },
            )
        }
        // NEVER IN THE INDEX (they are answered in-process); a build that
        // put one there is answered as a file would not be — left out.
        K::App | K::Command => (
            SectionId::Files,
            Hit {
                id: format!("other:{}", h.id),
                title: h.name,
                note: None,
                what: What::Kiosk(String::new()),
            },
        ),
    }
}

/// Every canonical zone name tzdata has (`Area/City`, and `UTC`), read once
/// per open view from `RONOCTUA_ZONEINFO`.
fn zone_names() -> Vec<String> {
    let Some(root) = std::env::var_os("RONOCTUA_ZONEINFO").map(PathBuf::from) else {
        return Vec::new();
    };
    // THE ZONES' OWN TABLE, `zone1970.tab`, and not a walk of the folder: its
    // third field is every canonical name, one a line — none of the links
    // and copies (`posix/`, `right/`, the old aliases) a walk would find.
    let Ok(table) = std::fs::read_to_string(root.join("zone1970.tab")) else {
        return Vec::new();
    };
    let mut out: Vec<String> = table
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split('\t').nth(2))
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .collect();
    out.push("UTC".to_owned());
    out.sort();
    out.dedup();
    out
}

/// The zones whose name the words find, each with its offset now.
fn zones_section(names: &[String], words: &str) -> Filled {
    let q = ronoctua_apps::fold::fold(words.trim()).replace(' ', "_");
    let Some(root) = std::env::var_os("RONOCTUA_ZONEINFO").map(PathBuf::from) else {
        return Filled::Refused(Said::key("fast-access-zone-none".to_owned()));
    };
    let now = ronoctua_index::now_s();
    let found: Vec<Hit> = names
        .iter()
        .filter(|n| q.is_empty() || ronoctua_apps::fold::fold(n).contains(&q))
        .take(crate::fast_access::most_of(Scope::Zones, SectionId::Zones).min(40))
        .map(|n| {
            let zone = ronoctua_files::when::Zone::read_from(&root.join(n));
            Hit {
                id: format!("zone:{n}"),
                title: n.replace('_', " "),
                note: None,
                what: What::Zone {
                    name: n.clone(),
                    offset_s: zone.offset_at(now),
                },
            }
        })
        .collect();
    if found.is_empty() {
        return Filled::Refused(Said::key("fast-access-zone-none".to_owned()));
    }
    Filled::Hits(found)
}

/// The online scheduler, on a thread of its own.
fn spawn_online(online: &Arc<(Mutex<Waiting>, Condvar)>, sink: &Sink, start: Starter) {
    let online = Arc::clone(online);
    let sink = Arc::clone(sink);
    let _ = std::thread::Builder::new()
        .name("fast-access-online".into())
        .spawn(move || online_thread(&online, &sink, &start));
}

/// The online scheduler: a question waits for its pause, then every source
/// of its scope runs on a thread of its own, stopped by the next key — the
/// question and its generation's stops taken TOGETHER.
fn online_thread(online: &Arc<(Mutex<Waiting>, Condvar)>, sink: &Sink, start: &Starter) {
    loop {
        let taken = {
            let (waiting, wake) = &**online;
            let mut w = waiting.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if w.done {
                    return;
                }
                match &w.ask {
                    // NO CLOCK while nothing waits.
                    None => w = wake.wait(w).unwrap_or_else(PoisonError::into_inner),
                    Some((_, due, _)) => {
                        let now = Instant::now();
                        if now >= *due {
                            break;
                        }
                        let left = *due - now;
                        w = wake
                            .wait_timeout(w, left)
                            .unwrap_or_else(PoisonError::into_inner)
                            .0;
                    }
                }
            }
            w.ask.take().map(|(a, _, stops)| (a, stops))
        };
        let Some((ask, stops)) = taken else {
            continue;
        };
        // A QUESTION WHOSE KEY WAS ALREADY FOLLOWED BY ANOTHER starts
        // nothing: its stops were pulled.
        if !stops.pulled() {
            start(&ask, &stops, sink);
        }
    }
}

/// What a source of the network answered, as a section's hits.
#[must_use]
pub fn web_hits(hits: Vec<ronoctua_websearch::WebHit>) -> Vec<Hit> {
    hits.into_iter()
        .map(|h| Hit {
            id: format!("web:{}", h.url),
            note: Some(crate::fast_access_rows::host_of(&h.url)),
            title: h.title,
            what: What::Web {
                url: h.url,
                snippet: h.snippet,
                source: h.source.id(),
            },
        })
        .collect()
}

/// A card of words, as the section's one hit.
#[must_use]
pub fn card_hit(card: Card, title: &str) -> Hit {
    Hit {
        id: "words".to_owned(),
        title: title.to_owned(),
        note: None,
        what: What::Words(card),
    }
}

/// The identities of every hit of a section's answer — what a test reads.
#[cfg(test)]
#[must_use]
pub fn ids_of(filled: &Filled) -> Vec<crate::fast_access::HitId> {
    match filled {
        Filled::Hits(h) => h.iter().map(|h| h.id.clone()).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(generation: u64, text: &str) -> Ask {
        Ask::of(
            generation,
            &crate::fast_access::parse(text),
            &crate::fast_access::Settings::default(),
            "en",
            &[],
        )
    }

    fn quiet() -> Sink {
        Arc::new(|_| {})
    }

    /// The sources' part, played: each question the online thread starts is
    /// told (`taken`), waits for the test's word (`go`) — the window between
    /// the thread taking a question and its requests being made — then makes
    /// a web request's stop and a yt-dlp run's and hands them back (`made`).
    struct Played {
        feed: Feed,
        taken: std::sync::mpsc::Receiver<u64>,
        go: std::sync::mpsc::Sender<()>,
        made: std::sync::mpsc::Receiver<(u64, ronoctua_http::web::Abort, ronoctua_ytdlp::Stop)>,
    }

    /// How long a test waits for something that should happen.
    const PATIENCE: std::time::Duration = std::time::Duration::from_secs(5);

    /// The pause a played feed waits: long enough for a second key to land in
    /// it, short enough for a test.
    const PAUSE: std::time::Duration = std::time::Duration::from_millis(60);

    fn played() -> Played {
        let (taken_tx, taken) = std::sync::mpsc::channel();
        let (go, go_rx) = std::sync::mpsc::channel::<()>();
        let (made_tx, made) = std::sync::mpsc::channel();
        let go_rx = Mutex::new(go_rx);
        let taken_tx = Mutex::new(taken_tx);
        let made_tx = Mutex::new(made_tx);
        let start: Starter = Arc::new(move |ask: &Ask, stops: &Stops, _: &Sink| {
            let _ = taken_tx.lock().unwrap().send(ask.generation);
            let _ = go_rx.lock().unwrap().recv_timeout(PATIENCE);
            let made = (ask.generation, stops.web(), stops.ytdlp());
            let _ = made_tx.lock().unwrap().send(made);
        });
        Played {
            feed: Feed::online_only(quiet(), start, PAUSE),
            taken,
            go,
            made,
        }
    }

    /// FAILS IF: what the network was asked for the last key goes on after
    /// the next one — a web request not shut, a yt-dlp not killed — even
    /// when the next key lands while the online thread is starting the old
    /// question's requests; or a question still in its pause is asked after
    /// a newer one replaced it; or an empty box or a scope that asks no host
    /// asks one (`online-stops-on-the-next-key`). The REAL online thread runs
    /// here; only the sources are played.
    #[test]
    fn online_stops_on_the_next_key() {
        let p = played();
        // A KEY, AND THE NEXT ONE WHILE THE THREAD STARTS THE FIRST'S
        // REQUESTS: what it makes for the first is stopped as it is made.
        p.feed.ask(&ask(1, "/w rust"));
        assert_eq!(p.taken.recv_timeout(PATIENCE), Ok(1));
        p.feed.ask(&ask(2, "/w rusty"));
        p.go.send(()).unwrap();
        let (generation, abort, stop) = p.made.recv_timeout(PATIENCE).unwrap();
        assert_eq!(generation, 1);
        assert!(abort.is_aborted(), "a request of an old key lived on");
        assert!(stop.is_stopped(), "a yt-dlp of an old key lived on");
        // THE SECOND KEY'S, after its pause: alive until the next key.
        assert_eq!(p.taken.recv_timeout(PATIENCE), Ok(2));
        p.go.send(()).unwrap();
        let (generation, abort, stop) = p.made.recv_timeout(PATIENCE).unwrap();
        assert_eq!(generation, 2);
        assert!(!abort.is_aborted() && !stop.is_stopped());
        p.feed.ask(&ask(3, "/w rustic"));
        assert!(abort.is_aborted(), "the web request went on");
        assert!(stop.is_stopped(), "yt-dlp went on");
        // A QUESTION IN ITS PAUSE is replaced, never asked.
        p.feed.ask(&ask(4, "/w r"));
        assert_eq!(p.taken.recv_timeout(PATIENCE), Ok(4), "3 was asked");
        p.go.send(()).unwrap();
        let (_, abort, _) = p.made.recv_timeout(PATIENCE).unwrap();
        // AN EMPTY BOX stops it, and asks nothing.
        p.feed.ask(&Ask::nothing(5));
        assert!(abort.is_aborted());
        // A SCOPE THAT ASKS NO HOST (the names) asks none either.
        // "Everything" does ask, the hosts already let only
        // (`fast_access_online`'s own test).
        p.feed.ask(&ask(6, "/f report"));
        assert!(
            p.taken.recv_timeout(PAUSE * 5).is_err(),
            "a host was asked for an empty box or an offline scope"
        );
    }

    /// FAILS IF: closing the view leaves a request of its running.
    #[test]
    fn closing_stops_what_is_in_flight() {
        let p = played();
        p.feed.ask(&ask(1, "/y lofi"));
        assert_eq!(p.taken.recv_timeout(PATIENCE), Ok(1));
        p.go.send(()).unwrap();
        let (_, abort, stop) = p.made.recv_timeout(PATIENCE).unwrap();
        assert!(!abort.is_aborted());
        drop(p.feed);
        assert!(stop.is_stopped() && abort.is_aborted());
    }

    /// FAILS IF: an inline feed answers an older generation, or an answer of
    /// the index's sections is missing for a scope that has them — a section
    /// left "searching" for ever.
    #[test]
    fn every_index_section_is_answered_even_without_an_index() {
        let got: Arc<Mutex<Vec<Back>>> = Arc::default();
        let sink: Sink = {
            let got = Arc::clone(&got);
            Arc::new(move |b| got.lock().unwrap().push(b))
        };
        let nowhere = std::env::temp_dir().join(format!(
            "ronoctua-fast-access-no-index-{}",
            std::process::id()
        ));
        let feed = Feed::inline_at(sink, Some(nowhere));
        feed.ask(&ask(7, "/f report"));
        let answered: Vec<SectionId> = got
            .lock()
            .unwrap()
            .iter()
            .filter_map(|b| match b {
                Back::Answer(a) if a.generation == 7 => Some(a.section),
                _ => None,
            })
            .collect();
        for s in index_sections(Scope::Names) {
            assert!(
                answered.contains(&s),
                "{s:?} was never answered: {answered:?}"
            );
        }
    }
}
