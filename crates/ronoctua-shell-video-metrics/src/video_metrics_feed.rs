//! The video-metrics doors' reader and writer: ONE thread per door, which
//! does everything that touches the catalogue or another process — and the
//! loop that draws never waits on any of it.
//!
//! # What it does
//!
//! It reads what the page on screen draws ([`video_metrics::Look`]) through
//! the catalogue opened READ-ONLY (`Catalog::open_ro`), and writes what the
//! person changes through the one writer a door has (`Person`) — the two
//! openers this shell may use (`video-metrics-readers-are-read-only`). It
//! decodes the thumbnails a page shows, reads a list of subscriptions to
//! import and writes the channels as one. And it carries the three errands
//! that leave the view for another process: a scan asked of the brain (or
//! of systemd, with no brain), a video put in the player's queue, a download
//! handed to the transfers' queue.
//!
//! # Event, not clock
//!
//! The catalogue's writers commit into `catalog.sqlite-wal`, which is WATCHED
//! (`Notice::on_log`, inotify): a commit stirs the thread, which asks
//! SQLite's `data_version` whether another connection committed, and reads
//! the page again only then. Nothing here wakes on a timer. Every WRITE into
//! it is heard, and not only the writer letting go: the scanner holds its
//! connection, and the `-wal` with it, open for the whole scan.
//!
//! # What it hands back
//!
//! [`Arrived`] values, taken by the door when the waker rings: a
//! [`video_metrics::Reading`] for what it read, a [`video_metrics::Done`] for
//! what an errand came to. Sentences are the view's (`View::took`): this
//! thread has no language.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use ronoctua_tokens as tokens;
use ronoctua_video_metrics::person::{NotAdded, Person, Registered};
use ronoctua_video_metrics::read::{Catalog, Page as ListPage};
use ronoctua_video_metrics::{Refusal, Why, takeout};

use crate::video_metrics::{
    Done, Edit, Imported, Look, Page, Picture, Queued, Reading, Scanned, ToPlay, Unadded,
    Unexported, Unimported,
};

/// A mark the player's ranking puts on a video ([`mark`]) — the player's
/// word, under the view that carries it.
pub use crate::player_seam::ranking::Mark;

/// What a door asks of its feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Want {
    /// Read what this look draws — and again whenever the catalogue moves.
    Look(Look),
    /// Write this, then read the look again.
    Edit(Edit),
    /// A scan, now.
    Scan,
    /// A video in the player's queue.
    Play(ToPlay),
    /// A video downloaded.
    Download {
        /// Its address.
        url: String,
        /// Its title.
        title: String,
    },
    /// The registered channels written as a list, in Google's shape, here.
    Export(PathBuf),
}

/// What the feed hands back.
#[derive(Debug, Clone)]
pub enum Arrived {
    /// What it read.
    Reading(Box<Reading>),
    /// What an errand came to.
    Done(Done),
}

/// AN ERRAND THAT LEAVES THE VIEW FOR ANOTHER PROGRAM, which only a door
/// carries out ([`carry_out`]'s `Err`): the footer's region makes room for it
/// first, and a window asks it down the footer's socket, or starts it
/// itself. ITS OWN TYPE, and not the view's [`crate::video_metrics::Errand`]:
/// each door matches every one of these by name, with no arm for the rest,
/// so an errand added here that a door does not carry out does not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leaving {
    /// The transfers' queue (`LEADER t t`), where a download handed over is
    /// seen going — the footer's.
    Transfers,
    /// A page of YouTube in Chrome, as a kiosk — the footer's `kiosk-open`.
    Kiosk(String),
    /// A page of YouTube in Nyxt, started by its entry in the launcher.
    Nyxt(String),
    /// The brain's page on the catalogue's scan — the footer's
    /// `brain-open <module>`.
    Brain,
    /// A window of the player's on the page this word opens — the player's
    /// `player-window <word>`.
    Player(&'static str),
}

/// CARRIES OUT what the view asked of a door — the ONE handler of the two
/// doors, the footer's region (`crate::video_metrics_region`) and the window
/// (`crate::video_metrics_window`): what is said, asked, pasted and closed is
/// done on `way`, and what the door's feed must do is handed back as a
/// [`Want`]. What only a door can do is handed back as `Err`, a [`Leaving`]:
/// the queue, Chrome, Nyxt, the brain's page and the player's window — the
/// footer's region makes room for each, and a window asks the footer or
/// starts it itself.
///
/// # Errors
///
/// The errand is the door's own.
pub fn carry_out(
    errand: crate::video_metrics::Errand,
    way: &mut dyn crate::region::Doorway,
) -> Result<Option<Want>, Leaving> {
    use crate::video_metrics::Errand;
    Ok(match errand {
        Errand::Nothing => None,
        Errand::Close => {
            way.close();
            None
        }
        // A NEW LOOK starts the door's page at the top — the footer's region
        // pages; a window's list keeps its own (`Doorway::first_page`).
        Errand::Look(look) => {
            way.first_page();
            Some(Want::Look(look))
        }
        Errand::Edit(edit) => Some(Want::Edit(edit)),
        // ASKED ON THE DOOR'S OWN LINE — the footer's minibuffer, a window's
        // header — from what the row holds now; the answer comes back to the
        // same `View::typed` in both.
        Errand::Ask { ask, initial } => {
            way.ask(crate::line::Line::new(
                ask.command(),
                false,
                initial.as_deref(),
            ));
            None
        }
        // THE LIVE SELECTION, off the loop, back to `View::text_pasted`.
        Errand::Paste => {
            way.paste();
            None
        }
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, crate::echo::Echo::Normal);
            None
        }
        Errand::Scan => Some(Want::Scan),
        Errand::Play(play) => Some(Want::Play(play)),
        Errand::Download { url, title } => Some(Want::Download { url, title }),
        Errand::Say(text) => {
            way.say(text, crate::echo::Echo::Long);
            None
        }
        // THE FILE PICKER'S SENTENCE, in the door's language.
        Errand::Explain(e) => {
            let said = way.texts().t_with(e.key, &e.borrowed());
            way.say(said, crate::echo::Echo::Long);
            None
        }
        // A FILE WRITTEN, on the feed's thread: never on the loop.
        Errand::Export(path) => Some(Want::Export(path)),
        Errand::Transfers => return Err(Leaving::Transfers),
        Errand::Kiosk(url) => return Err(Leaving::Kiosk(url)),
        Errand::Nyxt(url) => return Err(Leaving::Nyxt(url)),
        Errand::Brain => return Err(Leaving::Brain),
        Errand::Player(word) => return Err(Leaving::Player(word)),
    })
}

/// A door's feed: the thread, what it has for the door, and the watch on the
/// catalogue's commits. Dropped with the door's view, and the thread with it.
pub struct Feed {
    wants: mpsc::Sender<Message>,
    arrived: Arc<Mutex<Vec<Arrived>>>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

/// What the thread hears.
enum Message {
    Want(Want),
    /// Something committed into the catalogue.
    Stirred,
}

impl Feed {
    /// The feed of the catalogue at `path` (`None`: there is no home, and
    /// every look is refused), waking the door with `wake` whenever it has
    /// something.
    #[must_use]
    pub fn start(path: Option<PathBuf>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (wants, heard) = mpsc::channel::<Message>();
        let arrived: Arc<Mutex<Vec<Arrived>>> = Arc::default();
        let watch = path.as_deref().and_then(|p| watched(p, wants.clone()));
        let out = Arc::clone(&arrived);
        let spawned = std::thread::Builder::new()
            .name("video-metrics".to_owned())
            .spawn(move || {
                let mut hand = Hand::new(path);
                while let Ok(first) = heard.recv() {
                    // EVERYTHING WAITING, AT ONCE: two looks in a row read
                    // once, the last; every write and errand is carried out.
                    let mut batch = vec![first];
                    batch.extend(heard.try_iter());
                    let said = hand.handle(batch);
                    if said.is_empty() {
                        continue;
                    }
                    if let Ok(mut held) = out.lock() {
                        held.extend(said);
                    }
                    wake();
                }
            });
        if let Err(e) = spawned {
            tracing::error!(error = %e, "could not start the video-metrics feed; the view reads nothing");
        }
        Self {
            wants,
            arrived,
            _watch: watch,
        }
    }

    /// Asks the thread for something.
    pub fn want(&self, want: Want) {
        let _ = self.wants.send(Message::Want(want));
    }

    /// Everything the thread has for the door since the last call.
    #[must_use]
    pub fn take(&self) -> Vec<Arrived> {
        self.arrived
            .lock()
            .map(|mut held| std::mem::take(&mut *held))
            .unwrap_or_default()
    }
}

/// A feed with NO THREAD: each want answered on the spot, in the caller's
/// turn — what a photograph draws with (the window's,
/// `crate::video_metrics_window`, and the footer's region, whose door runs
/// no loop a thread could wake), so a frame is never taken before the
/// reading it shows has arrived. It reads and writes the catalogue it is
/// given, READS this instance's consent to ask YouTube and keeps a change of
/// it in memory (never in this machine's file), and carries NO errand to
/// another process: a scan, a video for the player, a download and adding a
/// channel (which asks yt-dlp) are answered with nothing, and what would
/// have been done is printed — for both doors alike. Nor does it write a
/// list of channels anywhere: an export is answered with nothing too.
pub struct Direct {
    hand: Hand,
}

impl Direct {
    /// The catalogue at `path`, read and written on the spot.
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        Self::of(Some(path))
    }

    /// The catalogue at `path` — `None`: there is no home, and every look is
    /// refused, as a feed's is.
    #[must_use]
    pub fn of(path: Option<PathBuf>) -> Self {
        let mut hand = Hand::new(path);
        hand.pretended = Some(hand.consented());
        Self { hand }
    }

    /// One want, answered now. A scan, a play, a download, a channel to
    /// add and an export answer nothing.
    pub fn want(&mut self, want: Want) -> Vec<Arrived> {
        // WHAT A PHOTOGRAPH WOULD HAVE DONE past the catalogue is printed,
        // never done: its door reaches nothing.
        match &want {
            Want::Scan => println!("would have asked the brain for a scan"),
            Want::Play(p) => println!("asked the player: put {} ({})", p.url, p.title),
            Want::Download { url, .. } => println!("would have downloaded {url}"),
            Want::Edit(Edit::AddChannel(input)) => {
                println!("would have asked yt-dlp who {input} is, and added it");
            }
            Want::Edit(Edit::Consent(on)) => {
                println!("kept in memory, not in this machine's file: ask YouTube = {on}");
            }
            Want::Export(path) => {
                println!(
                    "would have saved the list of channels to {}",
                    path.display()
                );
            }
            Want::Look(_) | Want::Edit(_) => {}
        }
        match want {
            Want::Scan
            | Want::Play(_)
            | Want::Download { .. }
            | Want::Export(_)
            | Want::Edit(Edit::AddChannel(_)) => Vec::new(),
            other => self.hand.handle(vec![Message::Want(other)]),
        }
    }
}

/// The watch on the catalogue's commits: `<catalogue>-wal`, in the folder
/// that holds it — MADE FIRST, because a watch on a folder that does not
/// exist never fires, and the scanner's first commit is what the door is
/// waiting for.
///
/// A LOG'S WATCH (`Notice::on_log`) and not a file's: the scanner writes
/// into a `-wal` it never closes while it runs, so a watch that waited for
/// the close heard the scan begin only once it was over. Coalesced, because
/// a scan is a stream of writes: a few stirs a second at most, each a
/// `data_version` asked (`Hand::moved`).
fn watched(
    catalogue: &Path,
    wants: mpsc::Sender<Message>,
) -> Option<ronoctua_files::notice::Notice> {
    let dir = catalogue.parent()?;
    let name = format!("{}-wal", catalogue.file_name()?.to_string_lossy());
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no folder for the video catalogue; its commits are not watched");
        return None;
    }
    let wants = Mutex::new(wants);
    ronoctua_files::notice::Notice::on_log(
        dir,
        &name,
        ronoctua_files::notice::Pace::Coalesced,
        move || wants.lock().is_ok_and(|w| w.send(Message::Stirred).is_ok()),
    )
    .map_err(|e| tracing::warn!(error = %e, "the video catalogue's commits are not watched"))
    .ok()
}

/// What the thread holds between two messages.
struct Hand {
    path: Option<PathBuf>,
    /// The catalogue, read-only, once it opened.
    catalog: Option<Catalog>,
    /// The writer, once something was written.
    person: Option<Person>,
    /// What the door last asked to see.
    look: Option<Look>,
    /// `data_version` when the look was last read.
    version: Option<u64>,
    /// The thumbnails decoded, by file — only the ones of the page on screen
    /// are kept past a reading.
    pictures: HashMap<PathBuf, Picture>,
    /// The consent a photograph keeps in memory, so a key that changes it
    /// changes no file; `None` is this instance's `consent.json`.
    pretended: Option<bool>,
}

impl Hand {
    fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            catalog: None,
            person: None,
            look: None,
            version: None,
            pictures: HashMap::new(),
            pretended: None,
        }
    }

    /// Whether every host a scan asks is let.
    fn consented(&self) -> bool {
        self.pretended.unwrap_or_else(|| {
            ronoctua_video_metrics::HOSTS
                .iter()
                .all(|h| ronoctua_consent::allows(h))
        })
    }

    /// Lets the hosts a scan asks, or takes them back — in memory for a
    /// photograph.
    fn let_hosts(&mut self, on: bool) -> Result<(), ronoctua_consent::Unkept> {
        if self.pretended.is_some() {
            self.pretended = Some(on);
            return Ok(());
        }
        ronoctua_consent::let_hosts(&ronoctua_video_metrics::HOSTS, on)
    }

    /// A batch of messages, in order; what it came to.
    fn handle(&mut self, batch: Vec<Message>) -> Vec<Arrived> {
        let mut out = Vec::new();
        let mut read = false;
        let mut stirred = false;
        for message in batch {
            match message {
                Message::Stirred => stirred = true,
                Message::Want(Want::Look(look)) => {
                    self.look = Some(look);
                    read = true;
                }
                Message::Want(Want::Edit(edit)) => {
                    out.push(Arrived::Done(self.edit(edit)));
                    read = true;
                }
                // AND THE PAGE READ AGAIN, here and not by the door: a scan
                // already writing when the answer came back is on it now, and
                // one that begins later is a commit the watch hears. A new
                // look from the door would page it back to the top
                // (`Doorway::first_page`); with no look — the region closed —
                // nothing is read.
                Message::Want(Want::Scan) => {
                    out.push(Arrived::Done(Done::Scan(scan())));
                    read = true;
                }
                Message::Want(Want::Play(play)) => {
                    let said = queue(&play);
                    out.push(Arrived::Done(Done::Queued(play, said)));
                }
                Message::Want(Want::Download { url, title }) => {
                    let taken = download(&url);
                    out.push(Arrived::Done(Done::Downloading(title, taken)));
                }
                // READ, NOT WRITTEN: the catalogue is as it was.
                Message::Want(Want::Export(path)) => {
                    out.push(Arrived::Done(Done::Exported(self.export(&path))));
                }
            }
        }
        // A COMMIT OF ANOTHER CONNECTION'S moves `data_version`; ours, and a
        // checkpoint, do not — and then there is nothing new to read.
        if stirred && !read {
            read = self.moved();
        }
        if read && let Some(look) = self.look.clone() {
            out.push(Arrived::Reading(Box::new(self.read(&look))));
        }
        out
    }

    /// Whether another connection committed since the look was last read.
    fn moved(&mut self) -> bool {
        match self.catalog.as_ref().map(Catalog::data_version) {
            Some(Ok(v)) => self.version != Some(v),
            // NOT OPEN YET, or no longer readable: reading again is how the
            // door hears that it opened, or why not.
            _ => true,
        }
    }

    /// The catalogue, opened read-only when it is not yet.
    fn catalog(&mut self) -> Result<&Catalog, Why> {
        if self.catalog.is_none() {
            let Some(path) = self.path.as_deref() else {
                return Err(Why::NotThere);
            };
            self.catalog = Some(Catalog::open_ro(path).map_err(|r| r.why)?);
        }
        self.catalog.as_ref().ok_or(Why::NotThere)
    }

    /// The writer, opened when it is not yet — it makes the catalogue, with
    /// its schema, when there is none.
    fn person(&mut self) -> Result<&Person, Why> {
        if self.person.is_none() {
            let Some(path) = self.path.as_deref() else {
                return Err(Why::NotThere);
            };
            self.person = Some(Person::open(path).map_err(|r| r.why)?);
        }
        self.person.as_ref().ok_or(Why::NotThere)
    }

    /// Everything `look` draws.
    fn read(&mut self, look: &Look) -> Reading {
        let mut r = Reading::default();
        // WHETHER YOUTUBE MAY BE ASKED is the consent file's, and it is
        // answered with or without a catalogue.
        if look.page == Page::Home {
            r.consent = Some(self.consented());
        }
        let version = match self.catalog() {
            Ok(c) => c.data_version().ok(),
            Err(why) => {
                // A REFUSAL IS AN ANSWER: the page says it, and nothing it
                // would have listed is shown as empty data.
                r.catalogue = Some(Err(why));
                if why != Why::NotThere {
                    self.catalog = None;
                }
                return r;
            }
        };
        self.version = version;
        let Ok(c) = self.catalog() else {
            return r;
        };
        let mut failed: Option<Why> = None;
        let mut take = |x: Result<(), Refusal>| {
            if let Err(e) = x {
                failed.get_or_insert(e.why);
            }
        };
        match &look.page {
            Page::Home => {
                take(c.last_scan().map(|s| r.last_scan = Some(s)));
                // AND WHETHER A SCAN HOLDS IT NOW: a last record with no end
                // is a scan running only then.
                r.scanning = Some(c.scanning());
                take(
                    c.count(&ronoctua_video_metrics::read::Filter::default())
                        .map(|n| r.videos_count = Some(n)),
                );
                take(c.skipped().map(|s| r.skipped = Some(s)));
                take(c.channels().map(|x| r.channels = Some(x)));
                // AND WHAT WAS SET ASIDE, hidden or not: the home page's way
                // to them says how many.
                let aside = ronoctua_video_metrics::read::Filter {
                    set_aside: ronoctua_video_metrics::read::SetAside::Only,
                    hidden: ronoctua_video_metrics::read::Hidden::Any,
                    ..ronoctua_video_metrics::read::Filter::default()
                };
                take(c.count(&aside).map(|n| r.set_aside_count = Some(n)));
            }
            // A CHANNEL'S KEEP SAYS, under it, what the next scan takes out
            // (`Catalog::expiring`) — as the rules' page says it under the
            // keep everywhere. AND THE CHANNELS STAGED, wherever a channel's
            // page may be a staged one's (its own, its rules') and where the
            // list of channels says how many wait.
            Page::Channel(_) => {
                take(c.channels().map(|x| r.channels = Some(x)));
                take(c.staged().map(|x| r.staged = Some(x)));
                take(c.expiring().map(|n| r.expiring = Some(n)));
            }
            Page::Channels | Page::Rule(Some(_), _) => {
                take(c.channels().map(|x| r.channels = Some(x)));
                take(c.staged().map(|x| r.staged = Some(x)));
            }
            // THE CHANNELS OF A LIST IMPORTED, each with its decision.
            Page::Import => take(c.staged().map(|x| r.staged = Some(x))),
            // THE SWITCHES, what a video and the list of them follow "as
            // everywhere" (`video_metrics::Follows`) — by the reader open
            // already: a page that only lists videos holds no writer.
            Page::Videos => {
                take(c.channels().map(|x| r.channels = Some(x)));
                take(c.settings().map(|s| r.settings = Some(s)));
            }
            // AND ITS CHANNEL, whose days of its own say when it goes
            // (`video_metrics::lasting`).
            Page::Video(id) => {
                let id = id.clone();
                take(c.video(&id).map(|v| r.video = Some((id, v))));
                take(c.settings().map(|s| r.settings = Some(s)));
                take(c.channels().map(|x| r.channels = Some(x)));
            }
            Page::Rules => take(c.expiring().map(|n| r.expiring = Some(n))),
            Page::Rule(None, _) | Page::Weights => {}
        }
        if let Some(q) = &look.query {
            let page = ListPage {
                number: q.page,
                per_page: crate::video_metrics::PER_PAGE,
            };
            match (c.ranked(&q.filter, page), c.count(&q.filter)) {
                (Ok(rows), Ok(total)) => {
                    r.listed = Some(crate::video_metrics::Listed {
                        query: q.clone(),
                        rows,
                        total,
                    });
                }
                (Err(e), _) | (_, Err(e)) => take(Err(e)),
            }
        }
        // THE RULES AND THE SWITCHES are the writer's to read (`Person`),
        // opened only for a page that shows them.
        let scope = match &look.page {
            Page::Rules | Page::Rule(None, _) => Some(None),
            Page::Channel(id) | Page::Rule(Some(id), _) => Some(Some(id.clone())),
            _ => None,
        };
        let wants_weights = look.page == Page::Weights;
        // THE SWITCHES, wherever a row follows them "as everywhere" and
        // says what that is now (`video_metrics::Follows`): a channel's
        // Shorts, a rule's page in a channel's scope — and the rules' page,
        // which turns them. Read by the writer these pages open for their
        // rules anyway, which is only reached once the catalogue answered:
        // nothing is made to read a default. A video and the list of them
        // read theirs by the reader, above.
        let wants_settings = matches!(
            look.page,
            Page::Rules | Page::Channel(_) | Page::Rule(Some(_), _)
        );
        if scope.is_some() || wants_weights || wants_settings {
            match self.person() {
                Ok(p) => {
                    if let Some(scope) = scope {
                        match p.rules(scope.as_ref()) {
                            Ok(rules) => r.rules = Some((scope, rules)),
                            Err(e) => {
                                failed.get_or_insert(e.why);
                            }
                        }
                    }
                    if wants_weights {
                        match p.weights() {
                            Ok(w) => r.weights = Some(w),
                            Err(e) => {
                                failed.get_or_insert(e.why);
                            }
                        }
                    }
                    if wants_settings {
                        match p.settings() {
                            Ok(s) => r.settings = Some(s),
                            Err(e) => {
                                failed.get_or_insert(e.why);
                            }
                        }
                    }
                }
                Err(why) => {
                    failed.get_or_insert(why);
                }
            }
        }
        r.pictures = Some(self.pictures_of(&r));
        r.catalogue = Some(failed.map_or(Ok(()), Err));
        r
    }

    /// The thumbnails the reading draws: the grid's, or the video's own —
    /// decoded here, off the loop, and only the ones not decoded already.
    fn pictures_of(&mut self, r: &Reading) -> HashMap<PathBuf, Picture> {
        let wanted: Vec<PathBuf> = r
            .listed
            .as_ref()
            .filter(|l| l.query.pictures)
            .map(|l| l.rows.iter().filter_map(|v| v.thumb_file.clone()).collect())
            .or_else(|| {
                r.video
                    .as_ref()
                    .and_then(|(_, v)| v.as_ref()?.row.thumb_file.clone())
                    .map(|f| vec![f])
            })
            .unwrap_or_default();
        let mut kept: HashMap<PathBuf, Picture> = HashMap::new();
        for file in wanted
            .into_iter()
            .take(crate::video_metrics::PICTURES_MOST as usize)
        {
            let picture = self.pictures.remove(&file).or_else(|| decoded(&file));
            if let Some(p) = picture {
                kept.insert(file, p);
            }
        }
        self.pictures.clone_from(&kept);
        kept
    }

    /// One write, and what the catalogue answered.
    fn edit(&mut self, edit: Edit) -> Done {
        if let Edit::AddChannel(input) = &edit {
            return Done::Added(self.add(input));
        }
        // A LIST IMPORTED, AND WHAT WAS DECIDED REGISTERED: each says what it
        // did, in its own words — never only "written".
        if let Edit::Import(file) = &edit {
            return Done::Imported(self.import(file));
        }
        if edit == Edit::Register {
            return Done::Registered(self.register());
        }
        // THE CONSENT IS NOT THE CATALOGUE: a switch that could not be kept
        // says why in the consent's own words (no home, a file it will not
        // write over, a folder it cannot write), never "the catalogue could
        // not be written" — which sent the person looking at the wrong file.
        if let Edit::Consent(on) = &edit {
            return match self.let_hosts(*on) {
                Ok(()) => Done::Edited(edit, Ok(())),
                Err(unkept) => Done::NotLet(crate::video_metrics_words::unkept_key(unkept)),
            };
        }
        // HIDE AND DELETE are the two writes a reader may make, through the
        // catalogue's own short writer; everything else is the person's.
        let result = match &edit {
            Edit::Delete(id) => self.catalog().and_then(|c| c.delete(id).map_err(|r| r.why)),
            _ => self
                .person()
                .and_then(|p| write(p, &edit).map_err(|r| r.why)),
        };
        Done::Edited(edit, result)
    }

    /// A channel added from what the person typed — resolved by yt-dlp, its
    /// hosts let — or why not.
    fn add(&mut self, input: &str) -> Result<String, Unadded> {
        let person = self.person().map_err(Unadded::Refused)?;
        match person.add_from(input) {
            Ok(channel) => Ok(channel.title),
            Err(NotAdded::Unresolved { trouble, said, .. }) => Err(Unadded::Unresolved(
                crate::video_metrics_words::unresolved_key(trouble, said),
            )),
            Err(NotAdded::Refused(r)) => Err(Unadded::Refused(r.why)),
            Err(NotAdded::NotLet(unkept)) => Err(Unadded::NotLet(
                crate::video_metrics_words::unkept_key(unkept),
            )),
        }
    }

    /// THE LIST IN `file`, STAGED — read whole, but never past
    /// `takeout::MOST_BYTES` (one byte more says it is something else), read
    /// as Google Takeout's subscriptions, and each of its channels left
    /// waiting for a decision; a channel registered already is answered as
    /// such and left as it is.
    fn import(&mut self, file: &Path) -> Result<Imported, Unimported> {
        use std::io::Read as _;
        let mut bytes = Vec::new();
        std::fs::File::open(file)
            .and_then(|f| {
                f.take(takeout::MOST_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map(|_| ())
            })
            .map_err(|e| {
                tracing::info!(file = %file.display(), error = %e, "a list of subscriptions that could not be read");
                Unimported::Unreadable
            })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > takeout::MOST_BYTES {
            return Err(Unimported::TooLarge);
        }
        let list = takeout::read(&bytes).map_err(Unimported::Unread)?;
        let staging = self
            .person()
            .map_err(Unimported::Refused)?
            .stage(&list.subscriptions)
            .map_err(|r| Unimported::Refused(r.why))?;
        Ok(Imported {
            staged: staging.staged,
            already: staging.already,
            left_out: u32::try_from(list.left_out.len()).unwrap_or(u32::MAX),
        })
    }

    /// WHAT WAS DECIDED, REGISTERED — and, once one channel joined, the
    /// hosts a scan asks let, ONCE: REGISTERING IS THE CONSENT TO SCAN, as
    /// adding a channel is (`Person::register_staged`). A photograph's
    /// consent is kept in memory ([`Hand::let_hosts`]), never this
    /// machine's file.
    ///
    /// A CONSENT NOT KEPT IS SAID BESIDE WHAT WAS REGISTERED, never in its
    /// place: the catalogue committed before the consent was asked, so the
    /// channels are in, and how many went in and how many were dropped is
    /// still the answer — with the key of why a scan will not ask them.
    fn register(&mut self) -> Result<(Registered, Option<&'static str>), Why> {
        let registered = self.person()?.register_staged().map_err(|r| r.why)?;
        let unkept = if registered.kept > 0 && !self.consented() {
            self.let_hosts(true)
                .err()
                .map(crate::video_metrics_words::unkept_alone_key)
        } else {
            None
        };
        Ok((registered, unkept))
    }

    /// THE REGISTERED CHANNELS, WRITTEN TO `file` as a list in Google's
    /// shape — by id, so two exports of one catalogue are one file — whole,
    /// and renamed over what was there (`ronoctua_xdg::write_atomically`).
    /// The staged ones are not channels of the catalogue yet, and stay out.
    fn export(&mut self, file: &Path) -> Result<(u32, PathBuf), Unexported> {
        let channels = self
            .catalog()
            .map_err(Unexported::Refused)?
            .channels()
            .map_err(|r| Unexported::Refused(r.why))?;
        let mut list: Vec<takeout::Subscription> = channels
            .into_iter()
            .map(|c| takeout::Subscription {
                id: c.id,
                title: c.title,
            })
            .collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        let (Some(dir), Some(name)) = (file.parent(), file.file_name().and_then(|n| n.to_str()))
        else {
            return Err(Unexported::NotWritten);
        };
        ronoctua_xdg::write_atomically(dir, name, takeout::write(&list).as_bytes()).map_err(
            |e| {
                tracing::warn!(file = %file.display(), error = %e, "the list of channels was not written");
                Unexported::NotWritten
            },
        )?;
        Ok((
            u32::try_from(list.len()).unwrap_or(u32::MAX),
            file.to_owned(),
        ))
    }
}

/// One of the person's writes.
fn write(p: &Person, edit: &Edit) -> Result<(), Refusal> {
    match edit {
        Edit::ChannelScore(id, score) => p.set_channel_score(id, *score),
        Edit::ChannelShorts(id, block) => p.set_channel_block_shorts(id, *block),
        Edit::RemoveChannel(id) => p.remove_channel(id),
        Edit::PutRule(rule) => p.put_rule(rule).map(|_| ()),
        Edit::RemoveRule(id) => p.remove_rule(*id),
        Edit::Weights(w) => p.set_weights(w),
        Edit::Settings(s) => p.set_settings(s),
        Edit::Rating(id, rating) => p.set_rating(id, *rating),
        Edit::SecondDescription(id, text) => p.set_second_description(id, text),
        Edit::SecondTags(id, tags) => p.set_second_tags(id, tags),
        Edit::Hidden(id, hidden) => p.set_hidden(id, *hidden),
        // THE MARKS THE RANKING PUTS ON A VIDEO TOO, by the one writer of
        // them both doors and the ranking share.
        Edit::SetAside(id, aside) => mark_with(p, &Mark::SetAside(id.clone(), *aside)),
        Edit::KeepVideo(id, keep) => mark_with(p, &Mark::Keep(id.clone(), *keep)),
        Edit::ChannelKeep(id, keep) => p.set_channel_keep(id, *keep),
        Edit::Decide(id, decision) => p.decide(id, *decision),
        Edit::DecideAll(decision) => p.decide_all(*decision).map(|_| ()),
        // Answered by the caller, never here.
        Edit::AddChannel(_)
        | Edit::Delete(_)
        | Edit::Consent(_)
        | Edit::Import(_)
        | Edit::Register => Err(Refusal::of(Why::Invalid)),
    }
}

/// THE MARKS A VIDEO TAKES FROM THE PLAYER'S RANKING — rated, set aside,
/// kept for ever by its own mark, hidden or shown again — each one of the
/// person's writes.
fn mark_with(p: &Person, mark: &Mark) -> Result<(), Refusal> {
    match mark {
        Mark::Rating(id, rating) => p.set_rating(id, *rating),
        Mark::SetAside(id, aside) => p.set_aside(id, *aside),
        Mark::Keep(id, keep) => p.set_keep(id, *keep),
        Mark::Hidden(id, hidden) => p.set_hidden(id, *hidden),
    }
}

/// A MARK FROM THE PLAYER'S RANKING, written into the catalogue at
/// `catalogue` — the ranking's one way to the person's writer, which only
/// this door may name (`video-metrics-readers-are-read-only`). Called off
/// the loop, by the thread that carries the ranking's act
/// (`player_ranking::carry`): the catalogue is opened for the one write, as
/// a reader's hide opens it, and a scan that holds it is waited for.
///
/// # Errors
///
/// What the catalogue answered: not there, busy, no such video.
pub fn mark(catalogue: &Path, mark: &Mark) -> Result<(), Why> {
    Person::open(catalogue)
        .and_then(|p| mark_with(&p, mark))
        .map_err(|r| r.why)
}

/// A SCRATCH CATALOGUE at `catalogue` holding one video, `id` titled
/// `title`, of a channel of its own — what a test of the ranking's marks
/// reads back (`Person::plant`, under `test-support`). Made by this door, as
/// every write of the person's catalogue is.
#[cfg(test)]
pub(crate) fn holding_one(catalogue: &Path, id: &ronoctua_video_metrics::VideoId, title: &str) {
    let person = Person::open(catalogue).expect("a scratch catalogue");
    let channel = ronoctua_video_metrics::ChannelId(format!("UC{}", "a".repeat(22)));
    person
        .add_channel(&ronoctua_video_metrics::person::NewChannel {
            id: channel.clone(),
            handle: None,
            title: "A channel".to_owned(),
            url: format!("https://www.youtube.com/channel/{}", channel.0),
            avatar_url: None,
        })
        .expect("a channel");
    person.plant(id, &channel, title).expect("a video");
}

/// A thumbnail file, decoded and made no larger than
/// `space.videoMetrics.thumbPx`. `None` for a file that is not a picture.
fn decoded(file: &Path) -> Option<Picture> {
    let picture = image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .map_err(|e| tracing::info!(file = %file.display(), error = %e, "a thumbnail that is no picture"))
        .ok()?;
    let most = u32::from(tokens::space::VIDEOMETRICS_THUMBPX);
    let picture = if picture.width() > most || picture.height() > most {
        picture.thumbnail(most, most)
    } else {
        picture
    };
    let rgba = picture.to_rgba8();
    let size = rgba.dimensions();
    ronoctua_render::image::Rgba::new(size.0, size.1, rgba.into_raw()).map(|rgba| Picture {
        rgba: Arc::new(rgba),
        size,
    })
}

/// A scan, now: asked of the brain — queued, gated, journaled, and the
/// modeline marked while it runs — and, with no brain answering (or one that
/// does not run this module: the person switched it off, which takes the
/// cadence and the mark and never the scan they ask for), of systemd. The
/// brain is a scheduler, not a gatekeeper: a machine whose daemon is down
/// still scans when the person asks.
///
/// TWO THINGS ARE SAID AND NOT ASKED. The power saver on: the module skips
/// every run while it is (the person saying "spend less"), so a run asked of
/// the brain would come to nothing behind a sentence that said it was asked
/// — and one started by systemd would spend what the saver keeps. And a
/// second instance (`RONOCTUA_INSTANCE`, the development loop): its brain
/// only rehearses, and the unit systemd knows is the INSTALLED session's,
/// which would scan the installed catalogue — the loop never touches the
/// real session.
fn scan() -> Scanned {
    use ronoctua_brain::compiled::video_metrics as module;
    if ronoctua_xdg::instance().is_some() {
        return Scanned::OtherInstance;
    }
    if saving() {
        return Scanned::Saver;
    }
    let ask = ronoctua_brain::Ask::Run {
        task: module::ID.to_owned(),
        rehearsal: false,
    };
    match ronoctua_brain::client::ask(&ask) {
        Ok(ronoctua_brain::Say::Refused { why }) => {
            tracing::info!(why = %why, "the brain does not run the scan; asking systemd");
        }
        Ok(ronoctua_brain::Say::Declined { key }) => {
            tracing::info!(key = %key, "the brain declined the scan; asking systemd");
        }
        Ok(_) => return Scanned::Brain,
        Err(e) => tracing::debug!(error = %e, "no brain; asking systemd for the scan"),
    }
    if !ronoctua_brain::systemd::reachable() {
        return Scanned::Nowhere;
    }
    match ronoctua_brain::systemd::start(module::unit::RUN) {
        Ok(()) => Scanned::Systemd,
        Err(e) => {
            tracing::warn!(error = %e, "systemd did not start the scan");
            Scanned::Nowhere
        }
    }
}

/// Whether the power saver is on — through the one crate that speaks
/// power-profiles-daemon, on this thread (a bus round trip).
fn saving() -> bool {
    ronoctua_power::Manager::fresh()
        .ok()
        .and_then(|m| m.profiles())
        .is_some_and(|p| p.active == ronoctua_power::Profile::PowerSaver)
}

/// A video put in the player's queue — `once`: refused when the queue has
/// it — and what the player said.
fn queue(play: &ToPlay) -> Result<(), Queued> {
    use ronoctua_player::wire::{Ask, Say, Thing, Trouble};
    let ask = Ask::Put {
        what: vec![Thing::Named {
            location: play.url.clone(),
            title: play.title.clone(),
        }],
        after: None,
        into: None,
        play: false,
        replace: false,
        once: play.once,
    };
    match ronoctua_player::client::ask(&ask) {
        Ok(Say::Refused(Trouble::AlreadyQueued { name, .. })) => Err(Queued::Already(name)),
        Ok(Say::Refused(t)) => Err(Queued::Refused(t)),
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::warn!(error = %e, "the player did not take the video");
            Err(Queued::Unreachable)
        }
    }
}

/// A video handed to the transfers' queue, the video at its best, into the
/// person's downloads, as nobody — the queue's own answer to where a page's
/// media goes (`where_to::work_for`). `false` when the queue did not take it.
fn download(url: &str) -> bool {
    use ronoctua_transfers::where_to::{Wish, landing, work_for};
    let work = work_for(url, landing(), Wish::media(ronoctua_transfers::Want::Video));
    match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(error = %e, "the transfer queue did not take the download");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_metrics::Query;
    use ronoctua_video_metrics::read::Filter;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-feed-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    /// FAILS IF: a catalogue that is not there is read as an empty one
    /// rather than said, or a write the person makes is not followed by a
    /// reading of the page — the door would draw what was there before.
    #[test]
    fn a_missing_catalogue_is_said_and_a_write_is_read_back() {
        let dir = scratch("feed");
        let path = dir.join("video-metrics").join("catalog.sqlite");
        let mut hand = Hand::new(Some(path));
        let said = hand.handle(vec![Message::Want(Want::Look(Look {
            page: Page::Channels,
            query: None,
        }))]);
        let Some(Arrived::Reading(r)) = said.last() else {
            panic!("no reading: {said:?}");
        };
        assert_eq!(r.catalogue, Some(Err(Why::NotThere)));
        assert!(r.channels.is_none(), "an absent catalogue listed channels");

        // THE WRITER MAKES THE CATALOGUE, and the page is read after it.
        let said = hand.handle(vec![Message::Want(Want::Edit(Edit::Weights(
            ronoctua_video_metrics::Weights {
                positive: 700,
                ..ronoctua_video_metrics::Weights::default()
            },
        )))]);
        assert!(
            matches!(
                said.first(),
                Some(Arrived::Done(Done::Edited(Edit::Weights(_), Ok(()))))
            ),
            "{said:?}"
        );
        let Some(Arrived::Reading(r)) = said.last() else {
            panic!("the write was not followed by a reading: {said:?}");
        };
        assert_eq!(r.catalogue, Some(Ok(())));
        assert_eq!(r.channels.as_deref().map(<[_]>::len), Some(0));

        // AND THE WEIGHTS PAGE reads what was written.
        let said = hand.handle(vec![Message::Want(Want::Look(Look {
            page: Page::Weights,
            query: None,
        }))]);
        let Some(Arrived::Reading(r)) = said.last() else {
            panic!("no reading: {said:?}");
        };
        assert_eq!(r.weights.map(|w| w.positive), Some(700));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a page with a row "as everywhere" is read without the
    /// switches it follows — the row would say "as everywhere" and not what
    /// that is now —, a page that only lists videos opens the writer to read
    /// them, or a page with none of them reads them for nothing.
    #[test]
    fn the_switches_are_read_where_a_default_is_shown() {
        let dir = scratch("switches");
        let path = dir.join("video-metrics").join("catalog.sqlite");
        drop(Person::open(&path).expect("a catalogue"));
        let mut hand = Hand::new(Some(path));
        let channel = ronoctua_video_metrics::ChannelId(format!("UC{}", "a".repeat(22)));
        let video = ronoctua_video_metrics::VideoId("video000000".to_owned());
        let read = |hand: &mut Hand, page: Page| {
            let said = hand.handle(vec![Message::Want(Want::Look(Look {
                page: page.clone(),
                query: None,
            }))]);
            let Some(Arrived::Reading(r)) = said.last() else {
                panic!("{page:?}: no reading: {said:?}");
            };
            r.settings.is_some()
        };
        // A PAGE THAT ONLY LISTS VIDEOS reads them by the reader: no writer
        // is opened for a default.
        for page in [Page::Videos, Page::Video(video)] {
            assert!(
                read(&mut hand, page.clone()),
                "{page:?}: the switches were not read"
            );
        }
        assert!(
            hand.person.is_none(),
            "a page of videos opened the writer to read the switches"
        );
        for page in [
            Page::Rules,
            Page::Channel(channel.clone()),
            Page::Rule(
                Some(channel),
                crate::video_metrics::Slot::Single(crate::video_metrics::Single::Short),
            ),
        ] {
            assert!(
                read(&mut hand, page.clone()),
                "{page:?}: the switches were not read"
            );
        }
        for page in [Page::Home, Page::Channels, Page::Weights] {
            assert!(
                !read(&mut hand, page.clone()),
                "{page:?}: the switches read for nothing"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a page that says a count is read without it — the home
    /// page's set-aside videos, hidden ones included; what the next scan
    /// takes out under the keep everywhere and a channel's —, a video's page
    /// is read without its channel, whose days say when it goes, or a mark a
    /// door writes does not land on its video.
    #[test]
    fn the_counts_a_page_says_are_read() {
        let dir = scratch("counts");
        let path = dir.join("video-metrics").join("catalog.sqlite");
        let video = ronoctua_video_metrics::VideoId("video000000".to_owned());
        holding_one(&path, &video, "A video");
        let mut hand = Hand::new(Some(path));
        let read = |hand: &mut Hand, page: Page| -> Box<crate::video_metrics::Reading> {
            let said = hand.handle(vec![Message::Want(Want::Look(Look {
                page: page.clone(),
                query: None,
            }))]);
            match said.into_iter().last() {
                Some(Arrived::Reading(r)) => r,
                other => panic!("{page:?}: no reading: {other:?}"),
            }
        };
        assert_eq!(read(&mut hand, Page::Home).set_aside_count, Some(0));
        // SET ASIDE AND HIDDEN, by the door's own writes: still counted.
        for edit in [
            Edit::SetAside(video.clone(), true),
            Edit::Hidden(video.clone(), true),
            Edit::KeepVideo(video.clone(), true),
        ] {
            let said = hand.handle(vec![Message::Want(Want::Edit(edit.clone()))]);
            assert!(
                said.iter()
                    .any(|a| matches!(a, Arrived::Done(Done::Edited(e, Ok(()))) if *e == edit)),
                "{edit:?}: {said:?}"
            );
        }
        assert_eq!(read(&mut hand, Page::Home).set_aside_count, Some(1));
        let channel = read(&mut hand, Page::Channels)
            .channels
            .and_then(|c| c.first().map(|c| c.id.clone()))
            .expect("the channel");
        for page in [Page::Rules, Page::Channel(channel.clone())] {
            assert_eq!(read(&mut hand, page.clone()).expiring, Some(0), "{page:?}");
        }
        let seen = read(&mut hand, Page::Video(video));
        assert!(
            seen.channels.is_some_and(|c| !c.is_empty()),
            "the video's channel unread"
        );
        let row = seen.video.and_then(|(_, v)| v).expect("the video").row;
        assert!(row.set_aside_at_s.is_some(), "the bookmark did not land");
        assert_eq!(row.forever, ronoctua_video_metrics::read::Forever::ByVideo);
        // A CHANNEL'S KEEP, written and read back.
        let keep = Some(ronoctua_video_metrics::Keep::Days(7));
        let said = hand.handle(vec![Message::Want(Want::Edit(Edit::ChannelKeep(
            channel.clone(),
            keep,
        )))]);
        assert!(
            said.iter()
                .any(|a| matches!(a, Arrived::Done(Done::Edited(_, Ok(()))))),
            "{said:?}"
        );
        let channels = read(&mut hand, Page::Channel(channel))
            .channels
            .expect("the channels");
        assert_eq!(channels[0].keep, keep);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: two looks waiting at once are read twice, or a stir with
    /// nothing committed by another connection reads the page again — the
    /// feed would wake the door for nothing on every checkpoint.
    #[test]
    fn a_batch_reads_once_and_a_stir_reads_only_what_moved() {
        let dir = scratch("batch");
        let path = dir.join("video-metrics").join("catalog.sqlite");
        drop(Person::open(&path).expect("a catalogue"));
        let mut hand = Hand::new(Some(path.clone()));
        let look = |page| Message::Want(Want::Look(Look { page, query: None }));
        let said = hand.handle(vec![look(Page::Home), look(Page::Channels)]);
        assert_eq!(said.len(), 1, "two looks read twice: {said:?}");
        // THE LAST LOOK, the channels': the home page's scan was not read.
        assert!(
            matches!(&said[0], Arrived::Reading(r) if r.channels.is_some() && r.last_scan.is_none()),
            "{said:?}"
        );
        assert!(
            hand.handle(vec![Message::Stirred]).is_empty(),
            "a stir with nothing new was read"
        );
        // ANOTHER CONNECTION COMMITS — the scanner, or the other door.
        let other = Person::open(&path).expect("a second writer");
        other
            .set_weights(&ronoctua_video_metrics::Weights::default())
            .expect("a write");
        let said = hand.handle(vec![Message::Stirred]);
        assert_eq!(
            said.len(),
            1,
            "another connection's commit was not read: {said:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a commit wakes the door only when its writer lets go. The
    /// scanner holds its connection — and the `-wal` under it — open for the
    /// whole scan, so a watch that waited for the close heard the scan begin
    /// only once it was over: "Scan now" said nothing until the page was
    /// opened again.
    #[test]
    fn a_commit_into_an_open_wal_wakes_the_feed() {
        // A CEILING, never a sleep: every wait returns the moment the ring
        // does.
        const PATIENCE: std::time::Duration = std::time::Duration::from_secs(5);
        let dir = scratch("open-wal");
        let path = dir.join("video-metrics").join("catalog.sqlite");
        // THE SCANNER: its connection open from before the door to after,
        // so the `-wal` is neither made nor let go of while the door watches.
        let scanner = Person::open(&path).expect("a catalogue");
        let (rang, rings) = mpsc::channel();
        let feed = Feed::start(Some(path), move || {
            let _ = rang.send(());
        });
        feed.want(Want::Look(Look {
            page: Page::Weights,
            query: None,
        }));
        rings
            .recv_timeout(PATIENCE)
            .expect("the look was never read");
        assert!(
            matches!(feed.take().last(), Some(Arrived::Reading(_))),
            "the look came back as no reading"
        );
        // A COMMIT, and again until the door reads one: the watch is armed
        // on a thread of its own, and a commit before it is nobody's to
        // hear. Each is weights no default has, so the reading that shows
        // them is a reading of a commit.
        let deadline = std::time::Instant::now() + PATIENCE;
        let mut positive = 1000;
        let heard = loop {
            positive += 1;
            scanner
                .set_weights(&ronoctua_video_metrics::Weights {
                    positive,
                    ..ronoctua_video_metrics::Weights::default()
                })
                .expect("a commit");
            let _ = rings.recv_timeout(std::time::Duration::from_secs(1));
            let read = feed.take().iter().any(|a| {
                matches!(a, Arrived::Reading(r)
                    if r.weights.is_some_and(|w| w.positive > 1000))
            });
            if read || std::time::Instant::now() >= deadline {
                break read;
            }
        };
        assert!(
            heard,
            "commits into a `-wal` its writer holds open never woke the feed"
        );
        drop(feed);
        drop(scanner);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a page of the list is read without its count, or the
    /// grid's pictures are decoded for a list — or kept past the page.
    #[test]
    fn a_page_of_the_list_comes_with_its_count() {
        let dir = scratch("list");
        let path = dir.join("video-metrics").join("catalog.sqlite");
        drop(Person::open(&path).expect("a catalogue"));
        let mut hand = Hand::new(Some(path));
        let said = hand.handle(vec![Message::Want(Want::Look(Look {
            page: Page::Videos,
            query: Some(Query {
                filter: Filter::default(),
                page: 0,
                pictures: false,
            }),
        }))]);
        let Some(Arrived::Reading(r)) = said.last() else {
            panic!("no reading: {said:?}");
        };
        let listed = r.listed.as_ref().expect("a page of the list");
        assert_eq!((listed.rows.len(), listed.total), (0, 0));
        assert!(r.pictures.as_ref().is_some_and(HashMap::is_empty));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A channel's id shaped as YouTube's — `UC` and twenty-two more —
    /// made up: no account's list is in this tree (invariant 13).
    fn made_up(n: u8) -> ronoctua_video_metrics::ChannelId {
        ronoctua_video_metrics::ChannelId(format!(
            "UC{}",
            char::from(b'k' + n).to_string().repeat(22)
        ))
    }

    /// What one want came to, and the reading after it.
    fn asked(hand: &mut Hand, want: Want) -> Vec<Arrived> {
        hand.handle(vec![Message::Want(want)])
    }

    /// FAILS IF: a list of subscriptions does not travel the whole way
    /// through a door's feed — imported (what it staged, what was here
    /// already, the line it left out), decided one by one and all at once,
    /// registered (the kept join the catalogue, the discarded go, the
    /// undecided wait, and the hosts a scan asks are let — in memory, for a
    /// photograph's door), and exported in Google's exact shape, the
    /// registered channels only, by id —; or a file that is no list, or is
    /// not there, or a place that cannot be written, is not refused as
    /// such.
    #[test]
    fn a_list_is_imported_decided_registered_and_exported() {
        use crate::video_metrics::{Imported, Unexported, Unimported};
        use ronoctua_video_metrics::read::Decision;
        let dir = scratch("takeout");
        let path = dir.join("video-metrics").join("catalog.sqlite");
        // ONE CHANNEL HERE ALREADY, and a list naming it with three more and
        // a line that names none.
        let person = Person::open(&path).expect("a catalogue");
        let here = made_up(0);
        person
            .add_channel(&ronoctua_video_metrics::person::NewChannel {
                id: here.clone(),
                handle: None,
                title: "Here already".to_owned(),
                url: format!("https://www.youtube.com/channel/{}", here.0),
                avatar_url: None,
            })
            .expect("a channel");
        drop(person);
        let (kept, dropped, waiting) = (made_up(1), made_up(2), made_up(3));
        let line = |id: &ronoctua_video_metrics::ChannelId, title: &str| {
            format!("{},{},{title}\n", id.0, takeout::channel_url(id))
        };
        let list = dir.join("subscriptions.csv");
        std::fs::write(
            &list,
            format!(
                "{}\n{}{}{}not-a-channel,x,Nothing\n{}\n\n",
                takeout::HEADER,
                line(&here, "Here already"),
                line(&kept, "\"Kept, with a comma\""),
                line(&dropped, "Dropped"),
                line(&waiting, "Waiting").trim_end(),
            ),
        )
        .expect("a list");
        let mut hand = Hand::new(Some(path));
        hand.pretended = Some(false);
        hand.handle(vec![Message::Want(Want::Look(Look {
            page: Page::Import,
            query: None,
        }))]);
        // IMPORTED.
        let said = asked(&mut hand, Want::Edit(Edit::Import(list)));
        let Some(Arrived::Done(Done::Imported(Ok(imported)))) = said.first() else {
            panic!("not imported: {said:?}");
        };
        assert_eq!(
            *imported,
            Imported {
                staged: 3,
                already: vec![(here.clone(), "Here already".to_owned())],
                left_out: 1,
            }
        );
        let Some(Arrived::Reading(r)) = said.last() else {
            panic!("the import was not read back: {said:?}");
        };
        let staged = r.staged.clone().expect("the staged channels");
        assert_eq!(staged.len(), 3);
        assert!(staged.iter().all(|s| s.decision == Decision::Undecided));
        assert!(
            staged
                .iter()
                .any(|s| s.channel.id == kept && s.channel.title == "Kept, with a comma")
        );
        // DECIDED: all kept, then one dropped and one left to wait.
        for edit in [
            Edit::DecideAll(Decision::Keep),
            Edit::Decide(dropped, Decision::Discard),
            Edit::Decide(waiting.clone(), Decision::Undecided),
        ] {
            let said = asked(&mut hand, Want::Edit(edit.clone()));
            assert!(
                matches!(said.first(), Some(Arrived::Done(Done::Edited(e, Ok(())))) if *e == edit),
                "{edit:?}: {said:?}"
            );
        }
        // REGISTERED — and the hosts let, in memory only.
        let said = asked(&mut hand, Want::Edit(Edit::Register));
        let Some(Arrived::Done(Done::Registered(Ok((done, None))))) = said.first() else {
            panic!("not registered, or the consent said unkept: {said:?}");
        };
        assert_eq!((done.kept, done.discarded, done.left), (1, 1, 1));
        assert_eq!(
            hand.pretended,
            Some(true),
            "registering did not let the hosts"
        );
        let said = asked(
            &mut hand,
            Want::Look(Look {
                page: Page::Channels,
                query: None,
            }),
        );
        let Some(Arrived::Reading(r)) = said.last() else {
            panic!("no reading: {said:?}");
        };
        let mut ids: Vec<String> = r
            .channels
            .clone()
            .expect("the channels")
            .into_iter()
            .map(|c| c.id.0)
            .collect();
        ids.sort();
        assert_eq!(ids, [here.0.clone(), kept.0.clone()]);
        let left: Vec<_> = r
            .staged
            .clone()
            .expect("the staged")
            .into_iter()
            .map(|s| s.channel.id)
            .collect();
        assert_eq!(left, [waiting]);
        // EXPORTED: Google's shape, the registered channels by id.
        let out = dir.join("out").join("subscriptions.csv");
        let said = asked(&mut hand, Want::Export(out.clone()));
        assert_eq!(said.len(), 1, "an export read the page again: {said:?}");
        let Some(Arrived::Done(Done::Exported(Ok((count, to))))) = said.first() else {
            panic!("not exported: {said:?}");
        };
        assert_eq!((*count, to), (2, &out));
        let written = std::fs::read_to_string(&out).expect("the list written");
        assert_eq!(
            written.lines().next(),
            Some(takeout::HEADER),
            "not Google's header"
        );
        let mut expected = vec![
            takeout::Subscription {
                id: here,
                title: "Here already".to_owned(),
            },
            takeout::Subscription {
                id: kept,
                title: "Kept, with a comma".to_owned(),
            },
        ];
        expected.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(written, takeout::write(&expected));
        // REFUSED, each as what it is.
        let refused = |hand: &mut Hand, want: Want| match asked(hand, want).into_iter().next() {
            Some(Arrived::Done(done)) => done,
            other => panic!("no answer: {other:?}"),
        };
        let blank = dir.join("blank.csv");
        std::fs::write(&blank, "Channel ID\n\n").expect("a file");
        assert_eq!(
            refused(&mut hand, Want::Edit(Edit::Import(blank))),
            Done::Imported(Err(Unimported::Unread(takeout::Unread::NotAList)))
        );
        assert_eq!(
            refused(&mut hand, Want::Edit(Edit::Import(dir.join("none.csv")))),
            Done::Imported(Err(Unimported::Unreadable))
        );
        let a_file = dir.join("a-file");
        std::fs::write(&a_file, b"").expect("a file");
        assert_eq!(
            refused(&mut hand, Want::Export(a_file.join("subscriptions.csv"))),
            Done::Exported(Err(Unexported::NotWritten))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
