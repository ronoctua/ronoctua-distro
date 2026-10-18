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
//! decodes the thumbnails a page shows. And it carries the three errands
//! that leave the view for another process: a scan asked of the brain (or
//! of systemd, with no brain), a video put in the player's queue, a download
//! handed to the transfers' queue.
//!
//! # Event, not clock
//!
//! The catalogue's writers commit into `catalog.sqlite-wal`, which is WATCHED
//! (`Notice::on_file`, inotify): a commit stirs the thread, which asks
//! SQLite's `data_version` whether another connection committed, and reads
//! the page again only then. Nothing here wakes on a timer.
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
use ronoctua_video_metrics::person::{NotAdded, Person};
use ronoctua_video_metrics::read::{Catalog, Page as ListPage};
use ronoctua_video_metrics::{Refusal, Why};

use crate::video_metrics::{
    Done, Edit, Look, Page, Picture, Queued, Reading, Scanned, ToPlay, Unadded,
};

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
}

/// What the feed hands back.
#[derive(Debug, Clone)]
pub enum Arrived {
    /// What it read.
    Reading(Box<Reading>),
    /// What an errand came to.
    Done(Done),
}

/// CARRIES OUT what the view asked of a door — the ONE handler of the two
/// doors, the footer's region (`crate::video_metrics_region`) and the window
/// (`crate::video_metrics_window`): what is said, asked, pasted and closed is
/// done on `way`, and what the door's feed must do is handed back as a
/// [`Want`]. What only a door can do is handed back as `Err`: the queue's
/// (`Errand::Transfers`) and the kiosk's (`Errand::Kiosk`) — the footer's,
/// which a window asks down its socket and the footer's region makes room
/// for.
///
/// # Errors
///
/// The errand is the door's own.
// THE ERRAND HANDED BACK is the door's own, moved once: boxing it would cost
// an allocation on every key, of every errand, for the two a door keeps.
#[allow(clippy::result_large_err)]
pub fn carry_out(
    errand: crate::video_metrics::Errand,
    way: &mut dyn crate::region::Doorway,
) -> Result<Option<Want>, crate::video_metrics::Errand> {
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
        errand @ (Errand::Transfers | Errand::Kiosk(_)) => return Err(errand),
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
/// turn — what a photograph draws with (`crate::video_metrics_window`), so a
/// frame is never taken before the reading it shows has arrived. It reads and
/// writes the catalogue it is given, READS this instance's consent to ask
/// YouTube and keeps a change of it in memory (never in this machine's file),
/// and carries NO errand to another process: a scan, a video for the player,
/// a download and adding a channel (which asks yt-dlp) are answered with
/// nothing, and the caller says what would have been done.
pub struct Direct {
    hand: Hand,
}

impl Direct {
    /// The catalogue at `path`, read and written on the spot.
    #[must_use]
    pub fn at(path: PathBuf) -> Self {
        let mut hand = Hand::new(Some(path));
        hand.pretended = Some(hand.consented());
        Self { hand }
    }

    /// One want, answered now. A scan, a play, a download and a channel to
    /// add answer nothing.
    pub fn want(&mut self, want: Want) -> Vec<Arrived> {
        match want {
            Want::Scan
            | Want::Play(_)
            | Want::Download { .. }
            | Want::Edit(Edit::AddChannel(_)) => Vec::new(),
            other => self.hand.handle(vec![Message::Want(other)]),
        }
    }
}

/// The watch on the catalogue's commits: `<catalogue>-wal`, in the folder
/// that holds it — MADE FIRST, because a watch on a folder that does not
/// exist never fires, and the scanner's first commit is what the door is
/// waiting for.
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
    ronoctua_files::notice::Notice::on_file(
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
                Message::Want(Want::Scan) => out.push(Arrived::Done(Done::Scan(scan()))),
                Message::Want(Want::Play(play)) => {
                    let said = queue(&play);
                    out.push(Arrived::Done(Done::Queued(play, said)));
                }
                Message::Want(Want::Download { url, title }) => {
                    let taken = download(&url);
                    out.push(Arrived::Done(Done::Downloading(title, taken)));
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
            }
            Page::Channels | Page::Channel(_) | Page::Rule(Some(_), _) => {
                take(c.channels().map(|x| r.channels = Some(x)));
            }
            Page::Videos => take(c.channels().map(|x| r.channels = Some(x))),
            Page::Video(id) => {
                let id = id.clone();
                take(c.video(&id).map(|v| r.video = Some((id, v))));
            }
            Page::Rules | Page::Rule(None, _) | Page::Weights => {}
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
        let wants_settings = matches!(look.page, Page::Rules);
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
        if let Edit::Consent(on) = &edit {
            let kept = self.let_hosts(*on).map_err(|_| Why::Failed);
            return Done::Edited(edit, kept);
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
        // Answered by the caller, never here.
        Edit::AddChannel(_) | Edit::Delete(_) | Edit::Consent(_) => Err(Refusal::of(Why::Invalid)),
    }
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
        .and_then(|m| m.read().profiles)
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
}
