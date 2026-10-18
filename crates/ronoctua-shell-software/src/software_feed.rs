//! The software center's readings: the threads that fetch them, and what the
//! doors keep of them.
//!
//! # Nothing here runs on the event loop
//!
//! Reading what the person has runs `nix profile list` and `flatpak list`; a
//! search reads a 144 000-line index and asks Flathub; a sheet's details may
//! reach the network; a web app's page is somebody else's server. Every one of
//! them is a thread here, and what comes back is an [`Arrival`] put in a queue
//! and announced by the WAKER the door handed over — a `calloop` ping or
//! channel, which is how every other feed of this shell reaches its loop.
//!
//! # Two halves, and the door holds both
//!
//! [`Feed`] is the half with threads: it is asked, and later it answers.
//! [`Readings`] is the half with no threads at all: the latest of every answer,
//! which the view reads on every frame. They are apart so the second can be
//! built from fixtures in a test and the first never has to be.
//!
//! # A search has a generation, and a late answer is dropped
//!
//! Typing `g`, `gi`, `gim`, `gimp` is four searches, and the network does not
//! answer them in order. Each is numbered, every [`Batch`] carries its number,
//! and a batch whose number is not the latest is thrown away — in the thread
//! before it is queued, and again in [`Readings::took`], because the door may
//! have asked again between the two.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ronoctua_software::details::Details;
use ronoctua_software::found::{Badge, Batch, Found, Status};
use ronoctua_software::icons::Candidate;
use ronoctua_software::prefs::Preferences;
use ronoctua_software::state::Snapshot;
use ronoctua_software::web::{ChromeProfile, WebApp};
use ronoctua_software::{AppKey, Context, Refusal, Source, journal};

use crate::software::Shelf;

/// How long the NETWORK half of a search waits for the next letter.
///
/// The index on this machine answers every letter — it is a scan of a local
/// file — and the network is asked only once the typing pauses: a request per
/// keystroke would be four questions to Flathub for one word, three of them
/// answered after they stopped mattering. A clock, and armed only while a
/// query is pending (the rule for clocks here).
const QUIET: std::time::Duration = std::time::Duration::from_millis(250);

/// How many icon candidates one shelf offers.
const CANDIDATES: usize = 24;

/// One answer, from one thread.
#[derive(Debug, Clone)]
pub enum Arrival {
    /// What the person has, read from the machine.
    Snapshot(Box<Snapshot>),
    /// What was done, oldest first (the file's order).
    History(Vec<journal::Entry>),
    /// Chrome's profiles, last used first.
    Profiles(Vec<ChromeProfile>),
    /// One source's answer to one search.
    Batch(Batch),
    /// One program's details — boxed, being the one large answer.
    Details {
        /// Which program.
        key: AppKey,
        /// The details, or why there are none.
        answer: Box<Result<Details, Refusal>>,
    },
    /// A web app's page, read for its name and its icons.
    Site {
        /// The address that was read.
        url: String,
        /// The page's name and its icons, best first.
        answer: Result<(Option<String>, Vec<Candidate>), Refusal>,
    },
    /// One shelf of icon candidates.
    Candidates {
        /// Which ask this answers.
        generation: u64,
        /// The candidates, or why there are none.
        answer: Result<Vec<Candidate>, Refusal>,
    },
    /// One Flatpak's permissions.
    Permissions {
        /// The app id.
        id: String,
        /// What it reaches, or `None` when it could not be read.
        found: Option<Permits>,
    },
    /// The dark theme's diagnosis: who forces `GTK_THEME`, and the two
    /// declared parts.
    Dark(DarkReading),
    /// One web app as its `.desktop` says, for the editor.
    WebApp {
        /// The slug asked for.
        slug: String,
        /// What the file says, or `None` when there is no such file.
        app: Option<WebApp>,
    },
}

/// What the view asks the feed for. The door hands it to [`Readings::want`],
/// which asks the [`Feed`] and remembers what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Want {
    /// Search these sources for this.
    Search {
        /// What was typed.
        query: String,
        /// Where to look.
        sources: Vec<Source>,
    },
    /// One program's sheet.
    Details(AppKey),
    /// A web app's page: its name and its icons.
    Site(String),
    /// One shelf of icons, for this query.
    Icons {
        /// Which shelf.
        shelf: Shelf,
        /// What to look for.
        query: String,
    },
    /// One Flatpak's permissions.
    Permissions(String),
    /// One web app, for editing.
    WebApp(String),
    /// Which Flatpaks force `GTK_THEME` (the advanced page's diagnosis).
    Dark,
    /// Read what the person has again (a job ended, the view opened).
    Reread,
}

/// The half with threads.
pub struct Feed {
    ctx: Context,
    language: String,
    wake: Arc<dyn Fn() + Send + Sync>,
    arrived: Arc<Mutex<Vec<Arrival>>>,
    /// The latest search's number, read by every search thread before it
    /// queues a batch.
    searching: Arc<AtomicU64>,
    /// The latest shelf's number.
    shelving: Arc<AtomicU64>,
    /// A reread is running; `again` says one more was asked meanwhile.
    reading: Arc<AtomicBool>,
    again: Arc<AtomicBool>,
    /// The network half of the searches, debounced.
    online: Option<std::sync::mpsc::Sender<Pending>>,
    /// The watches on what the declaration's reading depends on: alive as
    /// long as the feed, which is as long as the view is open.
    _watches: Vec<ronoctua_files::notice::Notice>,
}

impl std::fmt::Debug for Feed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feed")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// A search waiting for the typing to pause.
struct Pending {
    generation: u64,
    query: String,
    sources: Vec<Source>,
}

impl Feed {
    /// The session's feed, with its context and language handed in. `wake`
    /// is called, from a thread, every time an arrival is queued; the door
    /// answers it by calling [`Feed::take`].
    #[must_use]
    pub fn with(ctx: Context, language: String, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let mut me = Self {
            ctx,
            language,
            wake: Arc::new(wake),
            arrived: Arc::new(Mutex::new(Vec::new())),
            searching: Arc::new(AtomicU64::new(0)),
            shelving: Arc::new(AtomicU64::new(0)),
            reading: Arc::new(AtomicBool::new(false)),
            again: Arc::new(AtomicBool::new(false)),
            online: None,
            _watches: Vec::new(),
        };
        me.online = me.dispatcher();
        me._watches = watch_the_declaration(&me.ctx.roots, &me.reader());
        me
    }

    /// The directories this feed reads, for the door's own writes.
    #[must_use]
    pub const fn roots(&self) -> &ronoctua_software::Roots {
        &self.ctx.roots
    }

    /// The tools this feed found, for the door's own launches.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.ctx
    }

    /// The person changed language: searches and details ask in the new one.
    pub fn set_language(&mut self, language: &str) {
        language.clone_into(&mut self.language);
    }

    /// The person's preferences changed on disk (online search, sources).
    pub fn took_preferences(&mut self, prefs: Preferences) {
        self.ctx.prefs = prefs;
    }

    /// Everything that arrived since the last call.
    #[must_use]
    pub fn take(&self) -> Vec<Arrival> {
        self.arrived
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default()
    }

    /// Read what the person has, their history and Chrome's profiles.
    ///
    /// ONE READ AT A TIME: a second ask while one runs marks it to run once
    /// more when it ends, so a burst of finished jobs is two reads, never ten.
    pub fn reread(&self) {
        self.reader().go();
    }

    /// What a reread needs, for whoever asks one: the door, or a watch whose
    /// file moved.
    fn reader(&self) -> Reader {
        Reader {
            ctx: self.ctx.clone(),
            post: Arc::new(self.poster()),
            reading: Arc::clone(&self.reading),
            again: Arc::clone(&self.again),
        }
    }

    /// Search. Answers the search's number, which the door keeps.
    ///
    /// The index on this machine is asked at once; every other source once
    /// the typing pauses. An empty query asks nobody, and its number still
    /// makes every earlier search stale.
    pub fn search(&self, query: &str, sources: &[Source]) -> u64 {
        let generation = self.searching.fetch_add(1, Ordering::SeqCst) + 1;
        let query = query.trim();
        if query.is_empty() {
            return generation;
        }
        let (here, away): (Vec<Source>, Vec<Source>) = sources
            .iter()
            .copied()
            .filter(|s| *s != Source::Web)
            .partition(|s| *s == Source::Nix);
        if !here.is_empty() {
            self.run_search(generation, query.to_owned(), here);
        }
        if !away.is_empty()
            && let Some(q) = &self.online
        {
            let _ = q.send(Pending {
                generation,
                query: query.to_owned(),
                sources: away,
            });
        }
        generation
    }

    /// One program's details.
    pub fn details(&self, key: &AppKey) {
        let ctx = self.ctx.clone();
        let key = key.clone();
        let language = self.language.clone();
        let post = self.poster();
        spawn("ronoctua-software-details", move || {
            let answer = Box::new(ronoctua_software::details::of(&ctx, &key, &language));
            post(Arrival::Details { key, answer });
        });
    }

    /// A web app's page: its name and its icons.
    pub fn site(&self, url: &str) {
        let url = url.to_owned();
        let post = self.poster();
        spawn("ronoctua-software-site", move || {
            // THE PERSON TYPED IT: that is the consent to ask its host,
            // recorded before anything is asked, and the page is read on it.
            let answer = ronoctua_software::icons::typed_pass(&url)
                .and_then(|pass| ronoctua_software::icons::from_site(&pass, &url));
            post(Arrival::Site { url, answer });
        });
    }

    /// One shelf of icons. Answers the shelf's number.
    pub fn icons(&self, shelf: Shelf, query: &str) -> u64 {
        let generation = self.shelving.fetch_add(1, Ordering::SeqCst) + 1;
        let ctx = self.ctx.clone();
        let query = query.to_owned();
        let dark = crate::software_pictures::dark_theme();
        let shelving = Arc::clone(&self.shelving);
        let post = self.poster();
        spawn("ronoctua-software-icons", move || {
            let answer =
                match shelf {
                    // THE SITE'S SHELF IS THE PAGE'S OWN READ (`Want::Site`), and
                    // the file shelf has no candidates — it is where a person
                    // brings one. Neither is fetched here.
                    Shelf::Site | Shelf::File => return,
                    Shelf::Dashboard => ronoctua_software::icons::from_dashboard_in(
                        &ctx.roots.ours_cache(),
                        &query,
                        CANDIDATES,
                        Some(dark),
                    ),
                    Shelf::Collection => ctx.tools.icon_names.as_deref().map_or(
                        Err(Refusal::SourceAbsent),
                        |names| {
                            Ok(ronoctua_software::icons::from_collection(
                                names, &query, CANDIDATES,
                            ))
                        },
                    ),
                };
            if shelving.load(Ordering::SeqCst) == generation {
                post(Arrival::Candidates { generation, answer });
            }
        });
        generation
    }

    /// One Flatpak's permissions.
    pub fn permissions(&self, id: &str) {
        let ctx = self.ctx.clone();
        let id = id.to_owned();
        let post = self.poster();
        spawn("ronoctua-software-permissions", move || {
            let found = ronoctua_software::flatpak::permissions(&ctx, &id).map(|keyfile| Permits {
                lines: ronoctua_software::flatpak::permission_lines(&keyfile),
                grants: ronoctua_software::flatpak::grants_of(&keyfile),
            });
            post(Arrival::Permissions { id, found });
        });
    }

    /// Which installed Flatpaks carry `GTK_THEME` — the one setting that
    /// takes libadwaita's style away — and whether the two declared parts of
    /// the dark theme are there. Reads every app's permissions and two
    /// installations' files: a thread, and only when the advanced page asks.
    pub fn dark(&self) {
        let ctx = self.ctx.clone();
        let post = self.poster();
        spawn("ronoctua-software-dark", move || {
            let forcing: Vec<(String, String)> = ronoctua_software::flatpak::installed(&ctx)
                .into_iter()
                .filter_map(|i| match i.key {
                    AppKey::Flatpak { id } => {
                        let lines = ronoctua_software::flatpak::permissions(&ctx, &id)
                            .map(|k| ronoctua_software::flatpak::permission_lines(&k))
                            .unwrap_or_default();
                        ronoctua_software::flatpak::forces_gtk_theme(&lines).then_some((id, i.name))
                    }
                    _ => None,
                })
                .collect();
            // THE USER'S INSTALLATION FIRST: the global override that keeps
            // the session's Qt settings out is the person's, and
            // `dark_facts` reads it from the first one.
            let facts = ronoctua_software::flatpak::dark_facts(
                &ronoctua_software::flatpak::installations(&ctx.roots),
            );
            post(Arrival::Dark(DarkReading { forcing, facts }));
        });
    }

    /// One web app, for the editor.
    pub fn web_app(&self, slug: &str) {
        let roots = self.ctx.roots.clone();
        let slug = slug.to_owned();
        let post = self.poster();
        spawn("ronoctua-software-web-app", move || {
            // A DECLARED ONE's entry is the system's once the reconciler took
            // the person's away, and it names its picture only as a file:
            // that file is where the editor starts.
            let app = ronoctua_software::web::read_anywhere(&roots, &slug).map(|mut a| {
                if a.icon.is_none() {
                    a.icon = a
                        .icon_file
                        .clone()
                        .map(|path| ronoctua_software::order::IconChoice::File { path });
                }
                a
            });
            post(Arrival::WebApp { slug, app });
        });
    }

    /// Queue an arrival and wake the door.
    fn poster(&self) -> impl Fn(Arrival) + Clone + Send + Sync + 'static {
        let arrived = Arc::clone(&self.arrived);
        let wake = Arc::clone(&self.wake);
        move |a| {
            if let Ok(mut q) = arrived.lock() {
                q.push(a);
            }
            wake();
        }
    }

    /// One search's thread.
    fn run_search(&self, generation: u64, query: String, sources: Vec<Source>) {
        let ctx = self.ctx.clone();
        let language = self.language.clone();
        let searching = Arc::clone(&self.searching);
        let post = self.poster();
        spawn("ronoctua-software-search", move || {
            let sink = |b: Batch| {
                // STALE IN THE THREAD, before it costs the loop a wake.
                if searching.load(Ordering::SeqCst) == b.generation {
                    post(Arrival::Batch(b));
                }
            };
            ronoctua_software::search::run(&ctx, &query, &sources, &language, generation, &sink);
        });
    }

    /// The thread that waits for the typing to pause before asking the
    /// network. It holds the NEWEST pending search only.
    fn dispatcher(&self) -> Option<std::sync::mpsc::Sender<Pending>> {
        let (queue, pending) = std::sync::mpsc::channel::<Pending>();
        let ctx = self.ctx.clone();
        let language = self.language.clone();
        let searching = Arc::clone(&self.searching);
        let post = self.poster();
        let born = std::thread::Builder::new()
            .name("ronoctua-software-online".to_owned())
            .spawn(move || {
                while let Ok(mut next) = pending.recv() {
                    // THE QUIET: every newer query replaces this one, and the
                    // clock starts again from it.
                    loop {
                        match pending.recv_timeout(QUIET) {
                            Ok(newer) => next = newer,
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    if searching.load(Ordering::SeqCst) != next.generation {
                        continue;
                    }
                    let ctx = ctx.clone();
                    let language = language.clone();
                    let searching = Arc::clone(&searching);
                    let post = post.clone();
                    spawn("ronoctua-software-search", move || {
                        let sink = |b: Batch| {
                            if searching.load(Ordering::SeqCst) == b.generation {
                                post(Arrival::Batch(b));
                            }
                        };
                        ronoctua_software::search::run(
                            &ctx,
                            &next.query,
                            &next.sources,
                            &language,
                            next.generation,
                            &sink,
                        );
                    });
                }
            });
        match born {
            Ok(_) => Some(queue),
            Err(e) => {
                tracing::error!(error = %e, "could not start the software search thread");
                None
            }
        }
    }
}

/// A named, detached thread; a failure to start one is logged, and the
/// answer simply never comes — which the screen shows as still asking.
/// A reread of the machine, which a door and a watch's thread both start:
/// one at a time, and one more when asked meanwhile.
#[derive(Clone)]
struct Reader {
    ctx: Context,
    post: Arc<dyn Fn(Arrival) + Send + Sync>,
    reading: Arc<AtomicBool>,
    again: Arc<AtomicBool>,
}

impl Reader {
    /// Read the machine again, on a thread.
    fn go(&self) {
        if self.reading.swap(true, Ordering::SeqCst) {
            self.again.store(true, Ordering::SeqCst);
            return;
        }
        let ctx = self.ctx.clone();
        let post = Arc::clone(&self.post);
        let reading = Arc::clone(&self.reading);
        let again = Arc::clone(&self.again);
        spawn("ronoctua-software-read", move || {
            loop {
                again.store(false, Ordering::SeqCst);
                post(Arrival::Snapshot(Box::new(ronoctua_software::state::read(
                    &ctx,
                ))));
                post(Arrival::History(journal::read(&ctx.roots.ours_state())));
                post(Arrival::Profiles(ronoctua_software::web::chrome_profiles(
                    &ctx.roots,
                )));
                if !again.load(Ordering::SeqCst) {
                    reading.store(false, Ordering::SeqCst);
                    // A late ask between the check and the store is not lost:
                    // it saw `reading` still true and set `again`.
                    if !again.swap(false, Ordering::SeqCst) {
                        return;
                    }
                    reading.store(true, Ordering::SeqCst);
                }
            }
        });
    }
}

/// The watches on what the declaration's reading depends on
/// (`declaration::watched`): the mirror (an order, a belongings sync), the
/// system's reports and the link a switch replaces (the update's apply), and
/// the reconciler's ledger. A file that moves reads the machine again — in
/// the window and in the footer alike, because both hold a feed while the
/// view is open, and only then: an event, never a clock (the file is the
/// broadcast).
///
/// One watch per folder, narrowed to its one file where the folder is busy
/// with others (`/run`, the crate's state); the mirror's folder is made
/// when it is missing, so a first sync is heard.
fn watch_the_declaration(
    roots: &ronoctua_software::Roots,
    reader: &Reader,
) -> Vec<ronoctua_files::notice::Notice> {
    use ronoctua_files::notice::{Notice, Pace};
    let mut by_folder: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for file in ronoctua_software::declaration::watched(roots) {
        let (Some(folder), Some(name)) = (file.parent(), file.file_name()) else {
            continue;
        };
        let name = name.to_string_lossy().into_owned();
        match by_folder.iter_mut().find(|(f, _)| f == folder) {
            Some((_, names)) => names.push(name),
            None => by_folder.push((folder.to_path_buf(), vec![name])),
        }
    }
    let mirror = ronoctua_software::declaration::mirror(roots);
    let mut out = Vec::new();
    for (folder, names) in by_folder {
        if folder.starts_with(&mirror)
            && let Err(e) = std::fs::create_dir_all(&folder)
        {
            tracing::warn!(error = %e, folder = %folder.display(), "the declaration's folder could not be made; it is not watched");
            continue;
        }
        if !folder.is_dir() {
            continue;
        }
        let stirred = {
            let reader = reader.clone();
            move || {
                reader.go();
                true
            }
        };
        let watch = if let [name] = names.as_slice() {
            Notice::on_file(&folder, name, Pace::Coalesced, stirred)
        } else {
            Notice::on_dirs(std::slice::from_ref(&folder), &[], Pace::Coalesced, stirred)
        };
        match watch {
            Ok(w) => out.push(w),
            Err(e) => {
                tracing::warn!(error = %e, folder = %folder.display(), "a file of the declaration is not watched");
            }
        }
    }
    out
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(work)
    {
        tracing::error!(error = %e, thread = name, "could not start a software thread");
    }
}

/// The key sequences some sentences teach, as the door's keymap has them.
///
/// Built by the DOOR from the constants beside the leader's bindings and
/// `bindings::label()` — a key is never text in a `.ftl` and never a literal
/// here (`keys-are-never-text`) — and handed in, so the view can write
/// "the launcher — LEADER a" without knowing which leader this instance has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Taught {
    /// The launcher (`LEADER a`).
    pub launcher: String,
    /// The queue (`LEADER t`).
    pub queue: String,
    /// The brain's page, where the automatic updates are (`LEADER b`).
    pub brain: String,
}

/// One search, as far as the screen goes.
#[derive(Debug, Clone, Default)]
pub struct Search {
    /// What was typed.
    pub query: String,
    /// Its number: a batch of any other number is stale.
    pub generation: u64,
    /// Every source asked, and how it stands.
    pub asked: Vec<(Source, Status)>,
    /// Every batch of this search, one per source.
    pub batches: Vec<Batch>,
    /// What the batches come to, merged and ranked.
    pub groups: Vec<Group>,
}

/// One program as the search found it — every source that has it, together.
///
/// Rows are merged only when their names FOLD to the same letters: a
/// confident identity or none. Merging by homepage or by guessing that an
/// appstream id is a Nix package would put two different programs under one
/// name, which is worse than one program on two rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The folded name, which is the group's identity for the cursor.
    pub id: String,
    /// The name to draw.
    pub name: String,
    /// Every source's row, `Badge::Installed` marked from the snapshot.
    pub members: Vec<Found>,
    /// Which member is recommended, when one may be.
    pub recommended: Option<usize>,
    /// The letters of the name the query matched, by cluster.
    pub lit: Vec<usize>,
    /// Found by what it does, not by its name.
    pub by_description: bool,
}

impl Group {
    /// The member a row stands for: the recommended one, or the first.
    #[must_use]
    pub fn best(&self) -> &Found {
        &self.members[self.recommended.unwrap_or(0).min(self.members.len() - 1)]
    }

    /// The member with this key.
    #[must_use]
    pub fn member(&self, key: &AppKey) -> Option<&Found> {
        self.members.iter().find(|f| &f.key == key)
    }
}

/// What a web app's page answered: its name and its icons, best first.
pub type SiteAnswer = Result<(Option<String>, Vec<Candidate>), Refusal>;

/// One Flatpak's permissions: the lines as it has them, and what they come to
/// in the reduced form a ring changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permits {
    /// Every permission, one line each (`share=network`).
    pub lines: Vec<String>,
    /// What it reaches today.
    pub grants: ronoctua_software::flatpak::Grants,
}

/// The advanced page's dark diagnosis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DarkReading {
    /// The Flatpaks forcing `GTK_THEME`, by id and name.
    pub forcing: Vec<(String, String)>,
    /// Whether the GTK 3 extension and the Qt override are there.
    pub facts: ronoctua_software::flatpak::DarkFacts,
}

/// One shelf of icons, as asked and as answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shelved {
    /// The ask's number: an answer of any other is stale.
    pub generation: u64,
    /// Which shelf.
    pub shelf: Shelf,
    /// What was looked for.
    pub query: String,
    /// The candidates, once they came.
    pub answer: Option<Result<Vec<Candidate>, Refusal>>,
}

/// A copied image written where the view can take it as a dropped file:
/// `<cache>/pasted/<hash>`, named by its bytes, so the same image pasted twice
/// is one file. The icon's own checks (magic bytes, size) are the crate's, when
/// the view takes it. `None` when it cannot be written.
#[must_use]
pub fn keep_pasted(roots: &ronoctua_software::Roots, bytes: &[u8]) -> Option<PathBuf> {
    use std::hash::{Hash as _, Hasher as _};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    let dir = roots.ours_cache().join("pasted");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{:016x}", h.finish()));
    if !path.exists() {
        std::fs::write(&path, bytes).ok()?;
    }
    Some(path)
}

/// The copied image, as the thread that read it hands it back — to either
/// door: the footer and the window read the LIVE selection the same way
/// (`crate::pasting::image`), keep it the same way ([`keep_pasted`]) and say
/// the same sentence when there is nothing to keep.
#[derive(Debug)]
pub enum Kept {
    /// Written where the view takes it as a dropped file.
    File(PathBuf),
    /// What is copied is not an image.
    NoImage,
    /// It could not be written to a file.
    Unwritten,
    /// The clipboard could not be read.
    Unread,
}

impl Kept {
    /// What a thread makes of the clipboard's reading: the image kept, or
    /// why it was not. Called ON THE THREAD that read it — the write is a
    /// file, and a file is not the loop's to wait for.
    #[must_use]
    pub fn of(read: anyhow::Result<Option<Vec<u8>>>, roots: &ronoctua_software::Roots) -> Self {
        match read {
            Ok(Some(bytes)) => keep_pasted(roots, &bytes).map_or(Self::Unwritten, Self::File),
            Ok(None) => Self::NoImage,
            Err(e) => {
                tracing::warn!(error = %e, "the copied image could not be read");
                Self::Unread
            }
        }
    }

    /// The file to take as dropped — or the key of the sentence that says
    /// why there is none.
    ///
    /// # Errors
    ///
    /// The sentence's key, when nothing was kept.
    pub fn file(self) -> Result<PathBuf, &'static str> {
        use crate::key;
        match self {
            Self::File(path) => Ok(path),
            Self::NoImage => Err(key::SOFTWARE_NO_COPIED_IMAGE),
            Self::Unwritten => Err(key::SOFTWARE_IMAGE_NOT_KEPT),
            Self::Unread => Err(key::DOOR_PASTE_FAILED),
        }
    }
}

impl Taught {
    /// The sequences of THIS instance's leader: what `bindings::taught` finds
    /// for each command, after the leader's label — a key is never text
    /// (`keys-are-never-text`).
    ///
    /// FROM THE LEADER'S DEFAULT SET, and not from the constants beside the
    /// bindings: those are the full tree's, and `t t` — the queue's there —
    /// typed where every sequence starts is the queue and then a key that
    /// leads nowhere. The queue is `t` now.
    #[must_use]
    pub fn of_this_leader() -> Self {
        let of = |command: &str| crate::bindings::taught(command, None).unwrap_or_default();
        Self {
            launcher: of("apps-open"),
            queue: of("transfers-open"),
            brain: of("brain-open"),
        }
    }
}

/// The latest of every answer. The door owns it and hands it to the view.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// What the person has; `None` until the first read.
    pub snapshot: Option<Snapshot>,
    /// The person's choices, as the file says NOW.
    pub prefs: Preferences,
    /// The search.
    pub search: Search,
    /// The latest details, and whose.
    pub details: Option<(AppKey, Result<Details, Refusal>)>,
    /// The latest page read for a web app, and its answer when it came.
    pub site: Option<(String, Option<SiteAnswer>)>,
    /// The latest shelf of icons asked.
    pub candidates: Option<Shelved>,
    /// Chrome's profiles.
    pub profiles: Vec<ChromeProfile>,
    /// What was done, newest LAST.
    pub history: Vec<journal::Entry>,
    /// One Flatpak's permissions, when asked: its id and what came back.
    pub permissions: Option<(String, Option<Option<Permits>>)>,
    /// One web app, read for the editor: its slug and what came back.
    pub web_app: Option<(String, Option<Option<WebApp>>)>,
    /// The dark diagnosis: `Some(None)` while asking.
    pub dark: Option<Option<DarkReading>>,
    /// The queue's software jobs.
    pub jobs: Vec<ronoctua_transfers::Job>,
    /// Key sequences for sentences.
    pub taught: Taught,
    /// The machine's time zone, for dates.
    pub zone: ronoctua_files::when::Zone,
    /// Seconds since the epoch, as of the door's last look at the clock.
    pub now: u64,
    /// `$HOME`, so a typed `~/…` is a path.
    pub home: Option<PathBuf>,
    /// The person's consents (`consent.json`), as read at the view's
    /// opening, after a consent row was pressed, and when the preferences
    /// change: a catalogue that is on and not let draws its consent row.
    pub consents: ronoctua_consent::Consents,
    /// The person's belongings, where a door has them: a change of the
    /// declaration asks their passphrase first when the session keeps no
    /// identity (`keep::signing_asks`), so the change goes signed.
    /// `None` (a test, a photograph with no belongings) never asks.
    pub belongings: Option<ronoctua_belongings::places::Places>,
}

impl Readings {
    /// Nothing read yet.
    #[must_use]
    pub fn fresh(prefs: Preferences, taught: Taught) -> Self {
        Self {
            prefs,
            taught,
            consents: ronoctua_consent::store().read(),
            ..Self::default()
        }
    }

    /// Ask the feed, and remember what was asked.
    pub fn want(&mut self, feed: &Feed, want: Want) {
        match want {
            Want::Search { query, sources } => {
                let generation = feed.search(&query, &sources);
                self.asked(query, &sources, generation);
            }
            Want::Details(key) => {
                // THE OLD DETAILS GO, so a sheet never draws another
                // program's description while its own is on the way.
                if self.details.as_ref().is_none_or(|(k, _)| *k != key) {
                    self.details = None;
                }
                feed.details(&key);
            }
            Want::Site(url) => {
                if self
                    .site
                    .as_ref()
                    .is_some_and(|(u, a)| *u == url && a.is_some())
                {
                    return;
                }
                self.site = Some((url.clone(), None));
                feed.site(&url);
            }
            Want::Icons { shelf, query } => {
                let generation = feed.icons(shelf, &query);
                self.candidates = Some(Shelved {
                    generation,
                    shelf,
                    query,
                    answer: None,
                });
            }
            Want::Permissions(id) => {
                self.permissions = Some((id.clone(), None));
                feed.permissions(&id);
            }
            Want::WebApp(slug) => {
                self.web_app = Some((slug.clone(), None));
                feed.web_app(&slug);
            }
            Want::Dark => {
                if self.dark.is_none() {
                    self.dark = Some(None);
                }
                feed.dark();
            }
            Want::Reread => feed.reread(),
        }
    }

    /// A search was asked: every source is asking, and nothing is found yet.
    pub fn asked(&mut self, query: String, sources: &[Source], generation: u64) {
        let empty = query.trim().is_empty();
        self.search = Search {
            asked: if empty {
                Vec::new()
            } else {
                sources
                    .iter()
                    .filter(|s| **s != Source::Web)
                    .map(|s| (*s, Status::Asking))
                    .collect()
            },
            query,
            generation,
            batches: Vec::new(),
            groups: Vec::new(),
        };
    }

    /// Take one arrival. Answers whether anything the screen draws changed.
    pub fn took(&mut self, a: Arrival) -> bool {
        match a {
            Arrival::Snapshot(s) => {
                if self.snapshot.as_ref() == Some(&*s) {
                    return false;
                }
                self.snapshot = Some(*s);
                // WHAT IS INSTALLED CHANGED, so the badges and the
                // recommendation of every result may have.
                self.merge();
                true
            }
            Arrival::History(h) => {
                let changed = self.history != h;
                self.history = h;
                changed
            }
            Arrival::Profiles(p) => {
                let changed = self.profiles != p;
                self.profiles = p;
                changed
            }
            Arrival::Batch(b) => {
                // STALE AGAIN, here: the door may have asked since the
                // thread looked.
                if b.generation != self.search.generation {
                    return false;
                }
                if let Some((_, status)) =
                    self.search.asked.iter_mut().find(|(s, _)| *s == b.source)
                {
                    status.clone_from(&b.status);
                } else {
                    self.search.asked.push((b.source, b.status.clone()));
                }
                self.search.batches.retain(|x| x.source != b.source);
                self.search.batches.push(b);
                self.merge();
                true
            }
            Arrival::Details { key, answer } => {
                self.details = Some((key, *answer));
                true
            }
            Arrival::Site { url, answer } => match &mut self.site {
                Some((asked, slot)) if *asked == url => {
                    *slot = Some(answer);
                    true
                }
                _ => false,
            },
            Arrival::Candidates { generation, answer } => match &mut self.candidates {
                Some(shelved) if shelved.generation == generation => {
                    shelved.answer = Some(answer);
                    true
                }
                _ => false,
            },
            Arrival::Permissions { id, found } => match &mut self.permissions {
                Some((asked, slot)) if *asked == id => {
                    *slot = Some(found);
                    true
                }
                _ => false,
            },
            Arrival::Dark(reading) => {
                let fresh = Some(Some(reading));
                let changed = self.dark != fresh;
                self.dark = fresh;
                changed
            }
            Arrival::WebApp { slug, app } => match &mut self.web_app {
                Some((asked, slot)) if *asked == slug => {
                    *slot = Some(app);
                    true
                }
                _ => false,
            },
        }
    }

    /// The queue's list, as it arrives. Keeps the software jobs, and answers
    /// whether one of them ENDED since the last list — when the door reads
    /// the machine again, because an order that ended changed it.
    pub fn took_jobs(&mut self, jobs: &[ronoctua_transfers::Job]) -> bool {
        let mine: Vec<ronoctua_transfers::Job> = jobs
            .iter()
            .filter(|j| matches!(j.work, ronoctua_transfers::Work::Software { .. }))
            .cloned()
            .collect();
        let ended = mine.iter().any(|j| {
            !j.stage.afoot()
                && self
                    .jobs
                    .iter()
                    .find(|o| o.id == j.id)
                    .is_none_or(|o| o.stage.afoot())
        });
        self.jobs = mine;
        ended
    }

    /// The preferences file changed.
    pub fn took_preferences(&mut self, prefs: Preferences) -> bool {
        let consents = self.reread_consents();
        if self.prefs == prefs {
            return consents;
        }
        self.prefs = prefs;
        // THE PREFERENCE DECIDES WHICH SOURCE IS RECOMMENDED.
        self.merge();
        true
    }

    /// Reads `consent.json` again; answers whether it changed.
    pub fn reread_consents(&mut self) -> bool {
        let fresh = ronoctua_consent::store().read();
        if fresh == self.consents {
            return false;
        }
        self.consents = fresh;
        true
    }

    /// Every batch of this search, merged into groups and ranked.
    fn merge(&mut self) {
        let installed: Vec<&AppKey> = self
            .snapshot
            .as_ref()
            .map(|s| s.installed.iter().map(|i| &i.key).collect())
            .unwrap_or_default();
        let mut groups: Vec<Group> = Vec::new();
        for batch in &self.search.batches {
            for found in &batch.found {
                let mut found = found.clone();
                if installed.contains(&&found.key) && !found.badges.contains(&Badge::Installed) {
                    found.badges.push(Badge::Installed);
                }
                let id = identity(&found.name);
                match groups.iter_mut().find(|g| g.id == id) {
                    // ONE ROW PER SOURCE inside a group: a source that
                    // answered twice for one name (two Flatpaks called the
                    // same) keeps its first.
                    Some(g)
                        if !g
                            .members
                            .iter()
                            .any(|m| m.key.source() == found.key.source()) =>
                    {
                        g.by_description &= found.by_description;
                        g.members.push(found);
                    }
                    Some(_) => {}
                    None => groups.push(Group {
                        name: found.name.clone(),
                        id,
                        by_description: found.by_description,
                        members: vec![found],
                        recommended: None,
                        lit: Vec::new(),
                    }),
                }
            }
        }
        for g in &mut groups {
            let rows: Vec<&Found> = g.members.iter().collect();
            g.recommended = ronoctua_software::found::recommended(&rows, self.prefs.prefer);
            let name = g.best().name.clone();
            g.name = name;
        }
        // RANKED BY NAME, the finder's own arithmetic: a group whose name,
        // command or id the query matches comes first, in the order `rank`
        // gives; a group the sources found by what it DOES comes after, under
        // a heading of its own, so the lit letters always say why a row is
        // there.
        let query = ronoctua_apps::fold::fold(self.search.query.trim());
        let keys: Vec<[String; 3]> = groups
            .iter()
            .map(|g| {
                let best = g.best();
                [
                    ronoctua_apps::fold::fold(&g.name),
                    best.program.clone().unwrap_or_default(),
                    ronoctua_apps::fold::fold(best.key.id()),
                ]
            })
            .collect();
        let items: Vec<Vec<&str>> = keys
            .iter()
            .map(|k| k.iter().map(String::as_str).collect())
            .collect();
        let slices: Vec<&[&str]> = items.iter().map(Vec::as_slice).collect();
        let order = crate::minibuffer::rank(&query, &slices);
        let mut by_index: Vec<Option<Group>> = groups.into_iter().map(Some).collect();
        let mut ranked: Vec<Group> = Vec::with_capacity(by_index.len());
        for i in order {
            if let Some(mut g) = by_index.get_mut(i).and_then(Option::take) {
                g.by_description = false;
                g.lit = crate::minibuffer::match_up(&query, &ronoctua_apps::fold::fold(&g.name))
                    .map(|(_, at)| at)
                    .unwrap_or_default();
                ranked.push(g);
            }
        }
        // WHAT THE NAMES DID NOT MATCH, the sources found by what it does —
        // in the order they came, which is each source's own ranking.
        ranked.extend(by_index.into_iter().flatten().map(|mut g| {
            g.by_description = true;
            g.lit = Vec::new();
            g
        }));
        self.search.groups = ranked;
    }

    /// The group with this identity.
    #[must_use]
    pub fn group(&self, id: &str) -> Option<&Group> {
        self.search.groups.iter().find(|g| g.id == id)
    }

    /// The group one program belongs to, when a search found it.
    #[must_use]
    pub fn group_of(&self, key: &AppKey) -> Option<&Group> {
        self.search.groups.iter().find(|g| g.member(key).is_some())
    }
}

/// A name's identity for merging: folded, letters and digits only.
#[must_use]
pub fn identity(name: &str) -> String {
    ronoctua_apps::fold::fold(name)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_software::found::Prefer;
    use ronoctua_software::state::Installed;

    /// FAILS IF: a file of the declaration that moves while the view is
    /// open — a belongings sync writing the mirror, the update's apply —
    /// does not read the machine again: the open view would keep saying
    /// what was true before. Or the mirror's folder is not made, so the
    /// first sync on a new machine would not be heard.
    #[test]
    fn a_moved_declaration_reads_the_machine_again() {
        use std::time::{Duration, Instant};
        let at =
            std::env::temp_dir().join(format!("ronoctua-shell-feed-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        let roots = ronoctua_software::Roots::under(&at);
        let ctx = Context {
            tools: ronoctua_software::Tools::default(),
            roots: roots.clone(),
            prefs: Preferences::default(),
            rehearse: false,
        };
        let (woke, wakes) = std::sync::mpsc::channel::<()>();
        let woke = Mutex::new(woke);
        let feed = Feed::with(ctx, "en-GB".into(), move || {
            if let Ok(w) = woke.lock() {
                let _ = w.send(());
            }
        });
        let folder = ronoctua_software::declaration::mirror(&roots).join("software");
        assert!(folder.is_dir(), "the mirror's folder was not made");
        assert!(feed.take().is_empty(), "nothing read before anything moved");
        // WRITTEN AGAIN until it is heard: the watch comes alive on its own
        // thread, and a write before that is no event of anybody's.
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut read = false;
        while !read && Instant::now() < deadline {
            std::fs::write(folder.join("declared.json"), "{\"format\":1,\"apps\":[]}\n").unwrap();
            let _ = wakes.recv_timeout(Duration::from_millis(250));
            read = feed
                .take()
                .iter()
                .any(|a| matches!(a, Arrival::Snapshot(_)));
        }
        assert!(read, "the machine was not read again");
        drop(feed);
        let _ = std::fs::remove_dir_all(&at);
    }

    fn found(key: AppKey, name: &str, badges: &[Badge]) -> Found {
        Found {
            key,
            name: name.to_owned(),
            summary: String::new(),
            version: String::new(),
            badges: badges.to_vec(),
            download_bytes: None,
            icon: None,
            homepage: None,
            license: None,
            publisher: None,
            program: None,
            by_description: false,
        }
    }

    fn batch(source: Source, generation: u64, found: Vec<Found>) -> Batch {
        Batch {
            source,
            generation,
            found,
            status: Status::Answered { from: None },
        }
    }

    #[test]
    fn a_stale_batch_is_dropped_and_a_current_one_is_kept() {
        let mut r = Readings::default();
        r.asked("gimp".into(), &[Source::Nix, Source::Flatpak], 5);
        let old = batch(
            Source::Nix,
            4,
            vec![found(
                AppKey::Nix {
                    attr: "gimp".into(),
                },
                "GIMP",
                &[],
            )],
        );
        assert!(
            !r.took(Arrival::Batch(old)),
            "a batch of an older search changed the screen"
        );
        assert!(r.search.groups.is_empty());
        let now = batch(
            Source::Nix,
            5,
            vec![found(
                AppKey::Nix {
                    attr: "gimp".into(),
                },
                "GIMP",
                &[],
            )],
        );
        assert!(r.took(Arrival::Batch(now)));
        assert_eq!(r.search.groups.len(), 1);
        // THE SOURCE THAT ANSWERED STOPS ASKING; the other still asks.
        assert!(matches!(
            r.search.asked.iter().find(|(s, _)| *s == Source::Nix),
            Some((_, Status::Answered { .. }))
        ));
        assert!(matches!(
            r.search.asked.iter().find(|(s, _)| *s == Source::Flatpak),
            Some((_, Status::Asking))
        ));
    }

    #[test]
    fn the_same_name_from_two_sources_is_one_group_and_nix_is_recommended() {
        let mut r = Readings::default();
        r.asked("gimp".into(), &[Source::Nix, Source::Flatpak], 1);
        r.took(Arrival::Batch(batch(
            Source::Flatpak,
            1,
            vec![found(
                AppKey::Flatpak {
                    id: "org.gimp.GIMP".into(),
                },
                "GIMP",
                &[Badge::Verified],
            )],
        )));
        r.took(Arrival::Batch(batch(
            Source::Nix,
            1,
            vec![found(
                AppKey::Nix {
                    attr: "gimp".into(),
                },
                "gimp",
                &[],
            )],
        )));
        assert_eq!(r.search.groups.len(), 1, "{:?}", r.search.groups);
        let g = &r.search.groups[0];
        assert_eq!(g.members.len(), 2);
        assert_eq!(
            g.best().key,
            AppKey::Nix {
                attr: "gimp".into()
            }
        );
        // AND THE RING IN ADVANCED FLIPS IT.
        r.took_preferences(Preferences {
            prefer: Prefer::FlatpakFirst,
            ..Preferences::default()
        });
        assert_eq!(
            r.search.groups[0].best().key,
            AppKey::Flatpak {
                id: "org.gimp.GIMP".into()
            }
        );
    }

    #[test]
    fn what_is_installed_is_marked_and_recommended() {
        let mut r = Readings::default();
        r.took(Arrival::Snapshot(Box::new(Snapshot {
            installed: vec![Installed {
                key: AppKey::Flatpak {
                    id: "org.gimp.GIMP".into(),
                },
                name: "GIMP".into(),
                version: String::new(),
                desktop_ids: vec!["org.gimp.GIMP.desktop".into()],
                size_bytes: None,
                update: None,
                unfree: false,
                origin: ronoctua_software::state::Origin::Mine,
                icon: None,
                trouble: None,
            }],
            ..Snapshot::default()
        })));
        r.asked("gimp".into(), &[Source::Nix, Source::Flatpak], 2);
        r.took(Arrival::Batch(batch(
            Source::Nix,
            2,
            vec![found(
                AppKey::Nix {
                    attr: "gimp".into(),
                },
                "GIMP",
                &[],
            )],
        )));
        r.took(Arrival::Batch(batch(
            Source::Flatpak,
            2,
            vec![found(
                AppKey::Flatpak {
                    id: "org.gimp.GIMP".into(),
                },
                "GIMP",
                &[],
            )],
        )));
        let g = &r.search.groups[0];
        assert!(g.best().badges.contains(&Badge::Installed));
        assert_eq!(g.best().key.source(), Some(Source::Flatpak));
    }

    #[test]
    fn a_name_match_ranks_before_a_description_match() {
        let mut r = Readings::default();
        r.asked("paint".into(), &[Source::Nix], 1);
        let mut by_what = found(
            AppKey::Nix {
                attr: "krita".into(),
            },
            "Krita",
            &[],
        );
        by_what.by_description = true;
        r.took(Arrival::Batch(batch(
            Source::Nix,
            1,
            vec![
                by_what,
                found(
                    AppKey::Nix {
                        attr: "pinta".into(),
                    },
                    "Paint Shop",
                    &[],
                ),
            ],
        )));
        let names: Vec<&str> = r.search.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Paint Shop", "Krita"]);
        assert!(!r.search.groups[0].by_description);
        assert!(r.search.groups[1].by_description);
        assert_eq!(
            r.search.groups[0].lit,
            vec![0, 1, 2, 3, 4],
            "the letters that matched are lit"
        );
    }

    #[test]
    fn an_empty_query_asks_nobody() {
        let mut r = Readings::default();
        r.asked(String::new(), &[Source::Nix], 3);
        assert!(r.search.asked.is_empty());
    }

    #[test]
    fn an_answer_nobody_asked_for_is_dropped() {
        let mut r = Readings::default();
        assert!(!r.took(Arrival::Site {
            url: "https://a.example".into(),
            answer: Err(Refusal::Offline)
        }));
        r.site = Some(("https://b.example".into(), None));
        assert!(!r.took(Arrival::Site {
            url: "https://a.example".into(),
            answer: Err(Refusal::Offline)
        }));
        assert!(r.took(Arrival::Site {
            url: "https://b.example".into(),
            answer: Err(Refusal::Offline)
        }));
        r.candidates = Some(Shelved {
            generation: 7,
            shelf: Shelf::Dashboard,
            query: "mail".into(),
            answer: None,
        });
        assert!(!r.took(Arrival::Candidates {
            generation: 6,
            answer: Ok(Vec::new())
        }));
        assert!(r.took(Arrival::Candidates {
            generation: 7,
            answer: Ok(Vec::new())
        }));
    }

    #[test]
    fn a_job_that_ended_is_reported_once() {
        use ronoctua_transfers::{Engine, Job, Stage, Work};
        let work = Work::Software {
            order: Box::new(ronoctua_software::Order::Refresh {
                source: Source::Flatpak,
            }),
            label: "Flathub".into(),
        };
        let mut running = Job::fresh_one(1, Engine::Software, work);
        running.stage = Stage::Running;
        let mut r = Readings::default();
        assert!(!r.took_jobs(std::slice::from_ref(&running)));
        let mut done = running.clone();
        done.stage = Stage::Done;
        assert!(
            r.took_jobs(std::slice::from_ref(&done)),
            "the end was not seen"
        );
        assert!(
            !r.took_jobs(std::slice::from_ref(&done)),
            "the same end was seen twice"
        );
    }

    #[test]
    fn a_pasted_image_is_one_file_named_by_its_bytes() {
        let at = std::env::temp_dir().join(format!("ronoctua-pasted-{}", std::process::id()));
        let roots = ronoctua_software::Roots {
            home: at.clone(),
            data: at.join("data"),
            config: at.join("config"),
            state: at.join("state"),
            cache: at.join("cache"),
            runtime: None,
            data_dirs: Vec::new(),
            system: ronoctua_software::System::of_machine(Some(&at.join("sys"))),
        };
        let a = keep_pasted(&roots, b"\x89PNG one").expect("written");
        let b = keep_pasted(&roots, b"\x89PNG one").expect("written");
        let c = keep_pasted(&roots, b"\x89PNG two").expect("written");
        assert_eq!(a, b, "the same image twice is one file");
        assert_ne!(a, c);
        assert_eq!(std::fs::read(&c).unwrap(), b"\x89PNG two");
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn identity_folds_accents_case_and_punctuation() {
        assert_eq!(identity("GIMP"), identity("gimp"));
        assert_eq!(identity("Café Notes!"), "cafenotes");
    }
}
