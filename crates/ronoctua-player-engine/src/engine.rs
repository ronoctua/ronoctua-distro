//! The engine: the voices that play, the clock they keep, and whoever is
//! watching.
//!
//! # One decode, many pictures
//!
//! A picture is decoded ONCE, by the voice that plays it, and rendered by the
//! software renderer at every size a viewer asked for — the column, the
//! footer and a window each get their own ring, and each ring gets the same
//! frame. Two doors showing one picture are never out of step: nothing could
//! be.
//!
//! # Voices and lanes
//!
//! What plays is lanes — a picture, a sound, subtitles — and a voice is one
//! mpv playing one source's picture and sound from one start (`voice.rs`).
//! A file alone is one voice; a set is as many as it needs, on one clock
//! (`session.rs`): the first sound keeps the time and every other voice is
//! measured against it every `sync::TICK` and sent, or nudged — a picture
//! five percent, a sound two — until it is in step.
//!
//! # The queue is ours
//!
//! The queue is a tree the daemon owns (`ronoctua_player::queue`), and what
//! plays next is ITS answer — repeat and shuffle included. mpv is handed a
//! window of two entries: the leaf that plays and the one after, because
//! audio without a gap between two files only exists inside mpv's own
//! playlist. When mpv moves on by itself the window moves with it
//! (`queue.rs`, `settle`); everything else is loaded by us.
//!
//! # What a file says about itself
//!
//! A file's tags are read and written by `ronoctua-tags`, on two threads of
//! their own and never under the lock (`tags.rs`): its sheet goes only to the
//! door that asks for it, a write goes into a draft beside the file that is
//! renamed over it, and what the player says of the file follows the new one.
//! A several-line field is edited in the editor, in a draft the daemon keeps
//! and watches (`drafts.rs`), so its question survives the door that opened it.
//!
//! # What is looked up on the network
//!
//! Every lookup is the daemon's (`desk.rs`, `lookups.rs`): tags suggested for a
//! file, subtitles for a film, lyrics for a song — one thread per desk, one
//! pace per host, the page kept for every door. The two lookups that ask on
//! their own when the person switched them on are `fetch.rs`'s.
//!
//! And so is every directory (`directory.rs`): the world's radio and TV, a
//! theme's films and lives — listed on desks of their own, kept in the
//! player's cache, placed on the globe here and only drawn by a door.
//!
//! # At rest
//!
//! No mpv exists until something is asked to play: the daemon at rest is a
//! socket and a sleeping thread. The queue it remembers is only a file.

use std::{
    io::{BufReader, Read as _, Write as _},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, LazyLock, Mutex, Weak},
    time::{Duration, Instant},
};

use anyhow::Result;

use ronoctua_lookup::sponsorblock::{self, Segment};
use ronoctua_player::{
    decoders,
    own::LaneId,
    prefs::{self, Preferences},
    queue::{EntryId, Queue},
    wire::{self, Ask, Repeat, Say, State, Trouble},
};

use crate::mpv::{self, Event, Mpv, Value};

mod ambience;
mod catalog;
mod decode;
mod desk;
mod directory;
mod drafts;
mod fetch;
mod lookups;
mod queue;
mod resolve;
mod session;
mod sinks;
mod subtitles;
mod tags;
mod translate;
mod voice;

pub use decode::forget_old_decodes;
pub(crate) use fetch::{Fetch, spawn_fetch};
pub(crate) use queue::name_text;
pub use resolve::{Resolved, path_of_uri, places_of, resolve_all};

/// Why an ask was not carried out: the trouble the asker is answered with.
///
/// A newtype so `?` works on the three kinds of failure an ask meets — the
/// engine's own (`anyhow`), the queue's refusals, and a trouble said as one.
#[derive(Debug)]
pub struct Refused(pub Trouble);

impl From<anyhow::Error> for Refused {
    fn from(e: anyhow::Error) -> Self {
        Self(Trouble::NoEngine {
            why: format!("{e:#}"),
        })
    }
}

impl From<Trouble> for Refused {
    fn from(t: Trouble) -> Self {
        Self(t)
    }
}

impl From<ronoctua_player::queue::Refusal> for Refused {
    fn from(r: ronoctua_player::queue::Refusal) -> Self {
        Self(Trouble::from(r))
    }
}

/// The shared engine: a lock and the bell the render and sync threads sleep on.
pub type Shared = Arc<(Mutex<Engine>, Condvar)>;

/// A viewer: a connection that asked for frames, and its ring.
struct Viewer {
    stream: UnixStream,
    ring: ronoctua_frames::Ring,
    /// Whose picture: `None` the first one shown, `Some(lane)` that lane's.
    lane: Option<LaneId>,
    /// Owed the frame on screen NOW. A paused video and an mp3's cover send
    /// no new frame, so a view opened on one waited for something to change
    /// — the owner's cover stayed black until a click paused the song.
    owed: bool,
}

/// Property ids for `observe`.
mod id {
    pub const PAUSE: u64 = 1;
    pub const TIME: u64 = 2;
    pub const DURATION: u64 = 3;
    pub const VOLUME: u64 = 4;
    pub const MUTE: u64 = 5;
    pub const SPEED: u64 = 6;
    pub const POS: u64 = 7;
    pub const PLAYLIST: u64 = 8;
    pub const TRACKS: u64 = 9;
    pub const METADATA: u64 = 10;
    pub const TITLE: u64 = 11;
    pub const WIDTH: u64 = 12;
    pub const HEIGHT: u64 = 13;
    pub const IDLE: u64 = 14;
    pub const EOF: u64 = 15;
}

/// The engine's state behind the lock.
pub struct Engine {
    /// The voices: the first plays the queue's window when a file plays
    /// alone, and the first part of a set when a set plays; the others only
    /// exist while a set needs them.
    voices: Vec<voice::Voice>,
    /// The last voice's serial handed out.
    serials: u64,
    /// What plays together, while a set (or another machine's lanes) plays.
    session: Option<session::Session>,
    /// The last session's number.
    generations: u64,
    /// Which voice draws which picture: `(lane, voice serial)`, `None` for
    /// the first picture shown.
    drawn: Vec<(Option<LaneId>, u64)>,
    /// The sound outputs, while somebody needs them.
    sinks: sinks::Sinks,
    /// The ambient sounds: apart from the voices, and from everything the
    /// queue is told.
    ambience: ambience::Ambience,
    /// The subtitle lanes over each picture: read and merged off the lock,
    /// one script per picture voice (`subtitles.rs`).
    subtitles: subtitles::Subtitles,
    /// Files' sheets read, changes written and the cover of what plays kept,
    /// off the lock (`tags.rs`).
    tags: tags::Tags,
    /// Several-line fields open in the editor (`drafts.rs`).
    drafts: drafts::Drafts,
    /// What the lookups found, for every door (`desk.rs`).
    desks: desk::Desks,
    /// What the directories listed, for every door (`directory.rs`).
    directories: directory::Directories,
    /// The local catalogue: its writer, its sources' ingests, the person's
    /// sources and the posters (`catalog.rs`).
    catalog: catalog::Catalog,
    /// The engine's own place in the process, for the threads it starts.
    me: Option<Weak<(Mutex<Self>, Condvar)>>,
    state: State,
    /// The queue: OUR tree, and the one answer to what plays next.
    queue: Queue,
    /// The shuffled order of the leaves, while shuffle is on.
    order: Option<Vec<EntryId>>,
    /// What mpv's playlist holds, as (mpv's entry id, ours), in its order:
    /// the leaf that plays and the one after it.
    window: Vec<(i64, EntryId)>,
    /// The outline's revision the watchers were last sent.
    told: Option<u64>,
    /// What was last written to the disk: the tree's revision and the lanes
    /// changed in place ([`Engine::lanes_moved`]) — a lane's start or volume
    /// is kept like any other change, or a restart would lose it.
    kept_revision: Option<(u64, u64)>,
    /// When the queue was last written to the disk ([`queue::keep_due`]).
    queue_kept_at: Option<Instant>,
    /// Whether a lane's change waits for its write ([`Engine::keep_after`]).
    keep_armed: bool,
    /// Entries still being read from the network.
    resolving: Vec<EntryId>,
    /// The entry to play once its reading is done.
    play_when_resolved: Option<EntryId>,
    /// Where the remembered leaf was, for the first time it plays again.
    resume: Option<(EntryId, f64)>,
    /// A person stopped it: the idle that follows is not the queue ending.
    stopping: bool,
    /// The file that ended last ended with an error.
    failed: bool,
    /// How many leaves in a row failed, so a queue of broken files stops
    /// instead of spinning.
    failures: usize,
    /// Which address of the current stream is being tried.
    address: usize,
    /// mpv's own title for what plays (its tags, else its file name).
    mpv_title: Option<String>,
    /// What mpv loaded last, as the queue writes locations.
    loaded: Option<String>,
    watchers: Vec<UnixStream>,
    viewers: Vec<Viewer>,
    /// Something changed that watchers have not been told.
    dirty: bool,
    /// When watchers were last told of a mere POSITION change: those come
    /// dozens of times a second and are said four times.
    last_tick: Instant,
    idle: bool,
    /// What the person chose (`player.json`), reread on `Ask::Reload`.
    prefs: Preferences,
    /// The sponsored stretches of the video playing, and the ones already
    /// jumped.
    segments: Vec<Segment>,
    jumped: Vec<usize>,
    /// Requests to make, taken by the event thread and made OUTSIDE the lock:
    /// a host on the internet is given seconds, and the engine is not.
    fetches: Vec<Fetch>,
    /// The item lyrics were last asked for, so a tag arriving twice is not
    /// two requests.
    lyrics_asked: Option<String>,
    /// The pairing code on show, and when it was made.
    invite: Option<(String, Instant)>,
    /// The tailnet, as last read.
    tailnet: Option<crate::remote::Tailnet>,
    /// The machine this one follows, when it does.
    following: Option<crate::remote::Following>,
    /// Poked on every broadcast, so MPRIS announces what changed.
    mpris: Option<std::sync::mpsc::Sender<()>>,
    /// The player is closing ([`quit`]): the queue was remembered as it
    /// stood, and nothing after writes it again — the stop that ends the
    /// sound moves mpv's place in the queue, and the file would wake with
    /// none.
    leaving: bool,
    /// Where the queue was opened from or last saved as (`queue.rs`).
    origin: Option<queue::Origin>,
    /// Where the files a `.rqueue` or an older queue writes otherwise ARE:
    /// written → canonical, found off the lock (`resolve::aliases_of`), for
    /// "is it in the queue already?" (`queue::not_queued_in`).
    aliases: std::collections::HashMap<String, String>,
    /// Moves every time a set's lanes change in place (`Queue::set_mut`) —
    /// a volume, a start — which the tree's revision does not: with it, the
    /// revision says when the queue's fingerprint is to be computed again.
    lanes_moved: u64,
    /// The queue's fingerprint, as last computed, and the (revision, lanes
    /// moved) it was computed at: computed again only when one moved.
    fingerprinted: std::cell::RefCell<Option<((u64, u64), String)>>,
    /// The lyrics' translation under way or answered (`translate.rs`).
    translating: translate::Translating,
}

impl Engine {
    /// An engine with no mpv, and the queue it last had.
    #[must_use]
    pub fn new() -> Self {
        let mut engine = Self {
            voices: Vec::new(),
            serials: 0,
            session: None,
            generations: 0,
            drawn: Vec::new(),
            sinks: sinks::Sinks::default(),
            ambience: ambience::Ambience::open(),
            subtitles: subtitles::Subtitles::open(),
            tags: tags::Tags::open(),
            drafts: drafts::Drafts::open(),
            desks: desk::Desks::open(),
            directories: directory::Directories::open(),
            catalog: catalog::Catalog::open(),
            me: None,
            state: State {
                protocol: wire::PROTOCOL,
                volume: 100,
                speed: 1.0,
                ..State::default()
            },
            queue: Queue::new(),
            order: None,
            window: Vec::new(),
            told: None,
            kept_revision: None,
            queue_kept_at: None,
            keep_armed: false,
            resolving: Vec::new(),
            play_when_resolved: None,
            resume: None,
            stopping: false,
            failed: false,
            failures: 0,
            address: 0,
            mpv_title: None,
            loaded: None,
            watchers: Vec::new(),
            viewers: Vec::new(),
            dirty: false,
            last_tick: Instant::now(),
            idle: true,
            prefs: Self::reread_prefs(),
            segments: Vec::new(),
            jumped: Vec::new(),
            fetches: Vec::new(),
            lyrics_asked: None,
            invite: None,
            tailnet: None,
            following: None,
            mpris: None,
            leaving: false,
            origin: None,
            aliases: std::collections::HashMap::new(),
            lanes_moved: 0,
            fingerprinted: std::cell::RefCell::new(None),
            translating: translate::Translating::default(),
        };
        engine.restore();
        engine
    }

    /// The preferences read again — the player's older list of services
    /// let merged into the consent file first, once
    /// (`ronoctua_player::prefs::merge_consent`) — with the hosts the
    /// consent file lets.
    fn reread_prefs() -> Preferences {
        match prefs::merge_consent() {
            Ok(true) => {
                tracing::info!("the player's own consents were merged into the consent file")
            }
            Ok(false) => {}
            Err(why) => tracing::warn!(
                ?why,
                "the player's own consents could not be merged; they still count"
            ),
        }
        prefs::read()
    }

    /// Where the engine lives: the threads it starts — a voice's, the
    /// clock's, the outputs' reading — reach it through this.
    pub fn set_shared(&mut self, shared: &Shared) {
        self.me = Some(Arc::downgrade(shared));
        // THE QUEUE IT WOKE WITH: where its files are, asked off the lock.
        self.find_aliases(shared);
    }

    /// The engine's place, for a thread it starts.
    fn shared(&self) -> Result<Shared, Refused> {
        self.me.as_ref().and_then(Weak::upgrade).ok_or_else(|| {
            Refused(Trouble::NoEngine {
                why: "the engine is not placed yet".to_owned(),
            })
        })
    }

    /// Whether the voice `serial` draws the picture `lane` asks for.
    fn draws(&self, serial: u64, lane: Option<LaneId>) -> bool {
        self.drawn.contains(&(lane, serial))
    }

    /// The state, for an answer.
    #[must_use]
    pub fn state(&self) -> State {
        let mut s = self.state.clone();
        s.protocol = wire::PROTOCOL;
        s.revision = self.queue.revision();
        s.resolving.clone_from(&self.resolving);
        if self.following.is_some() {
            // ANOTHER MACHINE'S: nothing of this queue plays while following,
            // and what plays is where its first lane is opened from.
            s.current = None;
            s.next = None;
            s.title.clone_from(&self.mpv_title);
        } else {
            s.location = s.current.and_then(|c| self.location_of_leaf(c));
            s.title = s
                .current
                .and_then(|c| self.given_title(c))
                .or_else(|| self.mpv_title.clone());
            s.next = self
                .queue
                .next(
                    s.current,
                    s.repeat,
                    ronoctua_player::queue::Step::Asked,
                    self.order.as_deref(),
                )
                .and_then(|n| self.queue.find(n))
                .map(|n| n.body.name());
        }
        s.queue_file = self.queue_kept();
        let (lanes, arrangement) = self.lanes_now();
        s.lane_states = self.lane_states(&lanes, arrangement.as_ref());
        s.lengths_off = self.lengths_off(&lanes);
        s.sources = self.sources_of(&lanes);
        s.lanes = lanes;
        s.arrangement = arrangement;
        s.sinks.clone_from(&self.sinks.list);
        s.ambience = self.ambience.state();
        s.drafts = self.drafts.list();
        s.desks = self.desks.summaries();
        s.desks.extend(self.directories.summaries());
        s.catalog = self.catalog.summary();
        s.invite = self
            .invite
            .as_ref()
            .filter(|(_, at)| at.elapsed() < ronoctua_player::remote::CODE_LIFE)
            .map(|(c, _)| c.clone());
        s.tailnet = self.tailnet.as_ref().map(|t| t.name.clone());
        s.nearby = self
            .tailnet
            .as_ref()
            .map(|t| t.peers.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default();
        let peers = ronoctua_player::remote::peers().read();
        s.hosts = peers.hosts.iter().map(|p| p.name.clone()).collect();
        s.guests = peers.guests.iter().map(|p| p.name.clone()).collect();
        s.joined = self.following.as_ref().map(|f| wire::Joined {
            host: f.host.clone(),
            role: f.role,
            drift_ms: f.drift_ms,
        });
        s
    }

    /// The tailnet, as the listener last read it.
    pub fn set_tailnet(&mut self, t: Option<crate::remote::Tailnet>) {
        self.tailnet = t;
    }

    /// The tailnet, when this machine is on one.
    #[must_use]
    pub const fn tailnet(&self) -> Option<&crate::remote::Tailnet> {
        self.tailnet.as_ref()
    }

    /// What this machine calls itself to another: its tailnet name, or its
    /// host name.
    #[must_use]
    pub fn machine_name(&self) -> String {
        self.tailnet
            .as_ref()
            .map(|t| t.name.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| {
                std::fs::read_to_string("/etc/hostname")
                    .ok()
                    .map(|h| h.trim().to_owned())
            })
            .unwrap_or_default()
    }

    /// The key for a guest that read the right code, while it is good; the
    /// code is used up.
    pub fn take_invite(&mut self, code: &str) -> Option<String> {
        let (shown, at) = self.invite.as_ref()?;
        if at.elapsed() >= ronoctua_player::remote::CODE_LIFE
            || !ronoctua_player::remote::same(shown, code)
        {
            return None;
        }
        self.invite = None;
        let bytes: [u8; 32] = ronoctua_player::remote::random().ok()?;
        Some(ronoctua_player::remote::key_of(&bytes))
    }

    /// Where this machine is, for a machine that follows it: its leaf, the
    /// set's time NOW (read from the voice that keeps it, not the last
    /// broadcast), and its lanes, each with where to open it.
    #[must_use]
    pub fn moment(&self) -> ronoctua_player::remote::Moment {
        let item = self.state.current.filter(|_| self.following.is_none());
        let speed = if self.state.speed > 0.0 {
            self.state.speed
        } else {
            1.0
        };
        let position = if self.session.is_some() {
            self.clock_now()
        } else {
            self.voices
                .first()
                .and_then(|v| v.mpv.float("time-pos"))
                .unwrap_or(self.state.position)
        };
        let (lanes, _) = self.lanes_now();
        ronoctua_player::remote::Moment {
            clock_ms: ronoctua_player::remote::clock_ms(),
            position,
            playing: self.state.playing,
            speed,
            title: item
                .and_then(|c| self.given_title(c))
                .or_else(|| self.mpv_title.clone()),
            lanes: item
                .map(|i| ronoctua_player::remote::lanes_of(i.0, &lanes))
                .unwrap_or_default(),
            item: item.map(|i| i.0),
        }
    }

    /// A lane's local file of the leaf that plays, for a machine that
    /// follows this one — never an address, which is handed over as itself,
    /// and never a leaf that does not play: only what plays is shared.
    #[must_use]
    pub fn media_location(&self, leaf: u64, lane: LaneId) -> Option<String> {
        if self.following.is_some() || self.state.current != Some(EntryId(leaf)) {
            return None;
        }
        let (lanes, _) = self.lanes_now();
        lanes
            .into_iter()
            .find(|l| l.id == lane)
            .map(|l| l.source)
            .filter(|s| !s.contains("://"))
    }

    /// This machine stops playing in step with another: what that machine
    /// played leaves, and this machine's own leaf is loaded again, paused
    /// where it was. Its queue was never touched.
    fn leave_following(&mut self) {
        let Some(f) = self.following.take() else {
            return;
        };
        f.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.end_session();
        self.window.clear();
        self.loaded = None;
        self.state.location = None;
        let Some(mpv) = self.voices.first().map(|v| v.mpv.clone()) else {
            return;
        };
        let (current, position) = f.resume;
        match current.filter(|c| self.queue.find(*c).is_some()) {
            // LOADED OVER what the other machine played: a stop in between
            // would leave an idle behind it that reads as the queue ending.
            Some(leaf) => {
                self.resume = Some((leaf, position));
                if let Err(Refused(t)) = self.load_leaf(&mpv, leaf, true) {
                    self.state.troubles = vec![t];
                }
            }
            None => {
                self.stopping = true;
                let _ = mpv.command(&["stop"]);
                self.set_current(None);
            }
        }
        self.dirty = true;
    }

    /// Says a trouble in the state, for every door.
    pub fn say_trouble(&mut self, t: Trouble) {
        self.state.troubles = vec![t];
        self.dirty = true;
    }

    /// Following ended on its own: why, in the troubles — when it is still
    /// the following that ended, and not one that replaced it.
    pub fn left(&mut self, follow: &Arc<std::sync::atomic::AtomicBool>, why: &str) {
        if !self
            .following
            .as_ref()
            .is_some_and(|f| Arc::ptr_eq(&f.stop, follow))
        {
            return;
        }
        self.leave_following();
        self.state.troubles = vec![Trouble::Remote {
            why: why.to_owned(),
        }];
    }

    /// Tells every watcher, dropping the ones gone: the outline first when
    /// the queue's shape changed since they were last told (and the queue
    /// is written to the disk then too — a change is an event), then the
    /// state.
    pub fn broadcast(&mut self) {
        // THE LYRICS, looked at before they are said: new ones are asked to
        // be translated while the translation is on (`translate.rs`).
        self.mind_translation();
        let revision = self.queue.revision();
        // EACH LINE MADE ONCE, and written whole to every watcher.
        if self.told != Some(revision) {
            if let Ok(line) = wire::line_of(&Say::Queue(Box::new(self.queue.outline()))) {
                self.watchers.retain_mut(|w| w.write_all(&line).is_ok());
            }
            self.told = Some(revision);
        }
        match queue::keep_due(
            self.kept_revision,
            (revision, self.lanes_moved),
            self.queue_kept_at,
            Instant::now(),
        ) {
            queue::Keep::Now => self.keep_queue(),
            queue::Keep::After(wait) => self.keep_after(wait),
            queue::Keep::Nothing => {}
        }
        if let Ok(line) = wire::line_of(&Say::State(Box::new(self.state()))) {
            self.watchers.retain_mut(|w| w.write_all(&line).is_ok());
        }
        self.dirty = false;
        self.last_tick = Instant::now();
        if let Some(m) = &self.mpris {
            let _ = m.send(());
        }
    }

    /// Where to poke MPRIS.
    pub fn set_mpris(&mut self, poke: Option<std::sync::mpsc::Sender<()>>) {
        self.mpris = poke;
    }

    /// Everything the player does on its way out, but leaving: the queue
    /// remembered — this machine's own, when it was following another —,
    /// the sound stopped, every watcher told [`Say::Farewell`] and let go,
    /// every picture's connection shut, and the socket taken off the disk,
    /// so nothing asks a player that is going and the next ask starts a new
    /// one. See [`quit`].
    fn farewell(&mut self) {
        // ONCE: a second quit while the first is leaving has nothing to add.
        if self.leaving {
            return;
        }
        self.keep_queue();
        self.remember_where();
        self.leaving = true;
        if let Some(f) = self.following.take() {
            f.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        self.stopping = true;
        for v in &self.voices {
            let _ = v.mpv.command(&["stop"]);
        }
        // THE OUTPUTS' MONITOR ENDS with the player, and so does every
        // ambient sound: closing is the one thing the queue is told that
        // reaches them.
        self.sinks = sinks::Sinks::default();
        self.ambience.end();
        self.subtitles.end();
        // A WRITE UNDER WAY stops, leaving the file as it was; the drafts go
        // with the player.
        self.tags.end();
        self.drafts.end();
        // THE LOOKUPS post nothing more, and a fetched cover waiting to be
        // written goes; nor do the directories.
        self.desks.end();
        self.directories.end();
        self.catalog.end();
        farewell_to(std::mem::take(&mut self.watchers));
        for v in self.viewers.drain(..) {
            let _ = v.stream.shutdown(std::net::Shutdown::Both);
        }
        if let Ok(path) = wire::socket_path() {
            let _ = std::fs::remove_file(path);
        }
        tracing::info!("the player is closing, as asked");
    }
}

/// Every watcher told [`Say::Farewell`] — the last line it reads — and let
/// go: its stream ends right after, so a door closes on the word and never
/// waits on a silence. A watcher that stopped reading is given up on after
/// [`WATCHER_PATIENCE`], never waited for.
fn farewell_to(watchers: Vec<UnixStream>) {
    for mut w in watchers {
        let _ = w.set_write_timeout(Some(WATCHER_PATIENCE));
        let _ = wire::write_line(&mut w, &Say::Farewell);
        let _ = w.shutdown(std::net::Shutdown::Both);
    }
}

/// How long a write to a watcher may wait for it to read — the state four
/// times a second, the farewell once. Every one is written under the
/// engine's lock, so a door that stopped reading (stopped, or stuck) would
/// otherwise hold the lock, every ask and every MPRIS call behind it, for as
/// long as it stayed stopped. Past this it is let go, and its watch connects
/// again when it wakes.
const WATCHER_PATIENCE: Duration = Duration::from_secs(1);

/// How long the process lives on after its farewell: the answer to whoever
/// asked has to leave first — MPRIS's caller waits for a reply.
const LEAVING: Duration = Duration::from_millis(200);

/// How long the farewell may take before the process leaves without it — a
/// lock held by something that never lets go must not keep a player that
/// was asked to close alive.
const FAREWELL_AT_MOST: Duration = Duration::from_secs(5);

/// The answer to whoever asked, out of what was carried: done, or the
/// refusal — which is ALSO said in `troubles`, every door's (most doors tell
/// and do not wait for the answer, and a refusal nobody hears is the silence
/// the protocol promises not to leave), except "it is in the queue already":
/// that is the asker's question to put to the person, not a trouble for
/// every door.
fn answer_of(carried: Result<(), Refused>, troubles: &mut Vec<Trouble>) -> Say {
    match carried {
        Ok(()) => Say::Done,
        Err(Refused(t @ Trouble::AlreadyQueued { .. })) => Say::Refused(t),
        Err(Refused(t)) => {
            *troubles = vec![t.clone()];
            Say::Refused(t)
        }
    }
}

/// CLOSES THE PLAYER, everywhere — [`Ask::Quit`] on the socket, MPRIS's
/// `Quit`: the farewell ([`Engine::farewell`]), and the process leaves
/// [`LEAVING`] later WITH SUCCESS, which its unit's `Restart=on-failure` does
/// not bring back. The next thing played starts it again (`client::ensure`).
///
/// THE WAY OUT IS ARMED FIRST, and it waits for the farewell only
/// [`FAREWELL_AT_MOST`]; and it leaves by `_exit` ([`crate::mpv::leave_now`]),
/// never through the C library's exit handlers with libmpv still running.
pub fn quit(shared: &Shared) {
    let (said, farewell_said) = std::sync::mpsc::channel::<()>();
    let _ = std::thread::Builder::new()
        .name("player-quit".into())
        .spawn(move || {
            let _ = farewell_said.recv_timeout(FAREWELL_AT_MOST);
            std::thread::sleep(LEAVING);
            crate::mpv::leave_now(0);
        });
    let (lock, bell) = &**shared;
    if let Ok(mut e) = lock.lock() {
        e.farewell();
    }
    bell.notify_all();
    let _ = said.send(());
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// What every mpv here is started with — every one the same, with sound:
/// a voice that plays no sound is told `aid=no`.
fn options() -> Vec<(&'static str, String)> {
    let mut o = vec![
        ("vo", "libmpv".to_owned()),
        // NO TERMINAL: measured, mpv touching a terminal it does not own
        // (a process group in the background) is what a daemon must never do.
        ("terminal", "no".to_owned()),
        ("input-default-bindings", "no".to_owned()),
        ("input-vo-keyboard", "no".to_owned()),
        // NOT THE PERSON'S mpv.conf: this is a system component, and an
        // option set for the standalone mpv (a vo, a profile) must not change
        // what the column shows.
        ("config", "no".to_owned()),
        ("idle", "yes".to_owned()),
        ("osc", "no".to_owned()),
        // NO OSD: the software renderer draws it INTO the frame, and every
        // door draws its own controls.
        ("osd-level", "0".to_owned()),
        // COVER ART IS THE PICTURE of a song: it arrives as a video track and
        // every door shows it like one.
        ("audio-display", "embedded-first".to_owned()),
        // NO SUBTITLE OR SOUND FILE IS PICKED UP BY NAME: each is a lane the
        // person sees and chose, never one mpv found beside the film.
        // Measured: `fuzzy` added `long1.flac` beside `long.mp4` and CHOSE it
        // as the film's sound — and a machine following this one, opening
        // the film by its address, had no such track and played silence.
        ("sub-auto", "no".to_owned()),
        ("audio-file-auto", "no".to_owned()),
        // THE WINDOW OF TWO: the next leaf is opened while this one plays, and
        // the sound goes on across the join when both can share an output —
        // `weak`, mpv's own default, said here because the window exists for
        // it (`queue.rs`). URLs yt-dlp resolves are never prefetched.
        ("gapless-audio", "weak".to_owned()),
        ("prefetch-playlist", "yes".to_owned()),
        ("hwdec", "auto-copy".to_owned()),
        ("audio-client-name", "ronoctua".to_owned()),
        ("keep-open", "no".to_owned()),
        // A MACHINE WITH NO SOUND still plays: the output falls to `null`
        // instead of refusing the file. Without it every file on a machine
        // whose sound server is down is "audio output initialization failed"
        // — measured — and a video would not move either.
        ("audio-fallback-to-null", "yes".to_owned()),
        ("ytdl", "yes".to_owned()),
    ];
    // YT-DLP BY PATH, where the unit names it (`ronoctua_ytdlp::named`):
    // a unit's PATH does not have it.
    if let Some(path) = ronoctua_ytdlp::named() {
        o.push((
            "script-opts",
            format!("ytdl_hook-ytdl_path={}", path.display()),
        ));
    }
    // ONE VIDEO, NEVER A LIST: a list is OURS to open — a playlist of the
    // queue, named, listed flat by `ronoctua_ytdlp::flat` (`resolve.rs`) —
    // and mpv expanding one on its own would put entries in its playlist
    // that the queue does not have.
    let mut raw = vec!["no-playlist=".to_owned()];
    // AND THE PO TOKEN, the queue's own flags (`ronoctua_ytdlp`): what makes
    // an anonymous request work is the same for playing as for downloading.
    let pot = ronoctua_ytdlp::pot_flags();
    for pair in pot.as_chunks::<2>().0 {
        raw.push(format!("{}={}", pair[0].trim_start_matches("--"), pair[1]));
    }
    o.push(("ytdl-raw-options", raw.join(",")));
    o
}

/// What mpv opens a `ronoctua-decode://` URI with — ONE for the process,
/// however many voices register it: mpv keeps the pointer for the handle's
/// life, and one made per voice would be one leaked per voice.
static DECODED: LazyLock<mpv::Opener> = LazyLock::new(|| Box::new(open_decoded));

fn open_mpv() -> Result<Arc<Mpv>> {
    let owned = options();
    let pairs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    // WHAT yt-dlp IS HANDED, where a play that does not start is looked
    // into: the PO token's plugin among it, or not.
    if let Some((_, raw)) = pairs.iter().find(|(k, _)| *k == "ytdl-raw-options") {
        tracing::debug!(raw, "a voice's yt-dlp options");
    }
    let mpv = Arc::new(Mpv::new(&pairs)?);
    mpv.streams(decoders::PROTOCOL, &DECODED)?;
    Ok(mpv)
}

/// What mpv opens for a `ronoctua-decode://` URI: the decoder's output.
fn open_decoded(uri: &str) -> Option<Box<dyn mpv::Stream>> {
    let path = decoders::path_of(uri)?;
    let decoder = decoders::for_path(Path::new(path))?;
    match decode::decode(decoder, Path::new(path)) {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::warn!(error = %e, decoder = decoder.id, "a decoder would not start");
            None
        }
    }
}

/// The names libgme reads: the music of consoles and 8-bit computers, one
/// chip's register writes.
///
/// # Why they are named at all
///
/// Because ffmpeg recognizes them at a probe SCORE of 25 and libmpv asks for
/// 26 (`demuxer-lavf-probescore`), so every one of them was answered
/// "unrecognized file format" — measured on a VGM that `ffprobe` read
/// without a word. The plan counted them as free with `ffmpeg-full`; nothing
/// had played one. Lowering the threshold for every file would let a weak
/// guess win elsewhere, so the demuxer is named for these names only.
const GAME_MUSIC: [&str; 11] = [
    "vgm", "vgz", "nsf", "nsfe", "spc", "gbs", "hes", "kss", "ay", "sap", "gym",
];

/// The options a file is loaded with: libgme named for the names it reads.
fn load_options(location: &str) -> Option<&'static str> {
    let path = location.split(['?', '#']).next().unwrap_or(location);
    let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    GAME_MUSIC.contains(&extension.as_str()).then_some("libgme")
}

/// Loads one location — through its decoder when one of ours reads it, and
/// with the demuxer named when libmpv would not find it on its own — with
/// `options` set for this file only.
pub(crate) fn load(mpv: &Mpv, location: &str, how: &str, options: &[(&str, String)]) -> Result<()> {
    let target = loadable(location);
    let mut all: Vec<(&str, String)> = options.to_vec();
    if let Some(demuxer) = load_options(location) {
        all.push(("demuxer-lavf-format", demuxer.to_owned()));
    }
    loadfile(mpv, &target, how, &all)
}

/// `loadfile <target> <how> -1 <options>`: `-1` is "no index", and the
/// options last as long as this one file does — the sound outputs mpv may
/// try ([`outputs`]) among them.
pub(crate) fn loadfile(
    mpv: &Mpv,
    target: &str,
    how: &str,
    options: &[(&str, String)],
) -> Result<()> {
    let mut options = options.to_vec();
    if let Some(list) = outputs() {
        options.push(("ao", list.to_owned()));
    }
    if options.is_empty() {
        return mpv.command(&["loadfile", target, how]);
    }
    mpv.command(&["loadfile", target, how, "-1", &options_text(&options)])
}

/// The sound outputs mpv tries when no PipeWire server can be listening: the
/// two others the machine may have, and then `null`
/// (`audio-fallback-to-null`).
pub(crate) const WITHOUT_PIPEWIRE: &str = "pulse,alsa";

/// The sound outputs a file may try: mpv's own list, unless no PipeWire
/// server can be listening ([`ronoctua_audio::pipewire::server_may_listen`]),
/// and then [`WITHOUT_PIPEWIRE`].
///
/// ASKED AT EVERY FILE, because mpv opens its output again at every file and
/// the server may have come or gone. Trying PipeWire's output with no server
/// is not a refusal but a throw of a die: libpipewire's thread loop, stopped
/// the instant it started, can leave mpv's core joining a thread that never
/// wakes — measured in `player-vm`, the core in `pw_thread_loop_stop`, the
/// loop's thread in `epoll_wait` — and the daemon stops answering with it.
/// OpenAL's output goes too: it asks PipeWire on its own.
pub(crate) fn outputs() -> Option<&'static str> {
    (!ronoctua_audio::pipewire::server_may_listen()).then_some(WITHOUT_PIPEWIRE)
}

/// Per-file options as mpv reads them: `key=value,…`, every value quoted by
/// its LENGTH (`%5%a,b c`) — a `User-Agent` with a comma in it is ordinary
/// (741 of iptv-org's), and a bare comma would end the value there.
#[must_use]
pub(crate) fn options_text(options: &[(&str, String)]) -> String {
    options
        .iter()
        .map(|(k, v)| format!("{k}=%{}%{v}", v.len()))
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn loadable(location: &str) -> String {
    let p = Path::new(location);
    if !location.contains("://") && decoders::for_path(p).is_some() {
        decoders::uri_of(p)
    } else {
        location.to_owned()
    }
}

/// The queue's location back from what mpv holds.
fn location_of(loaded: &str) -> String {
    decoders::path_of(loaded).map_or_else(|| loaded.to_owned(), str::to_owned)
}

impl Engine {
    /// The first voice's mpv, made on first need. Nothing is loaded into it
    /// until something is asked to play: the queue is ours, and mpv holds
    /// only the window of it that plays.
    pub(crate) fn main(&mut self, shared: &Shared) -> Result<Arc<Mpv>> {
        if let Some(v) = self.voices.first() {
            return Ok(v.mpv.clone());
        }
        self.serials += 1;
        let v = voice::Voice::open(self.serials, shared)?;
        v.mpv.set("volume", &self.state.volume.to_string())?;
        apply_repeat(&v.mpv, self.state.repeat);
        let mpv = v.mpv.clone();
        self.voices.push(v);
        self.map_pictures();
        Ok(mpv)
    }

    /// Carries out an ask; the answer for the one who asked.
    ///
    /// A refusal is also said in the state's troubles: most doors TELL and do
    /// not wait for the answer, and a refusal nobody hears is the silence the
    /// protocol promises not to leave.
    pub fn ask(&mut self, ask: Ask, shared: &Shared) -> Say {
        self.dirty = true;
        let carried = self.carry(ask, shared);
        answer_of(carried, &mut self.state.troubles)
    }

    fn carry(&mut self, ask: Ask, shared: &Shared) -> Result<(), Refused> {
        match ask {
            Ask::Watch | Ask::Look | Ask::Queue | Ask::Preview { .. } => {}
            Ask::Put {
                what,
                after,
                into,
                play,
                replace,
                once,
            } => {
                // RESOLVED HERE only when the asker did not do it first
                // (`serve_one` does, outside the lock): a folder is a walk of
                // the disk, and the engine's lock is every door's.
                if once {
                    self.not_queued(&places_of(&what))?;
                }
                let resolved = resolve_all(what);
                self.put_resolved(resolved, after, into, play, replace, shared)?;
            }
            Ask::Toggle => {
                self.not_following()?;
                let mpv = self.main(shared)?;
                if let Some(s) = &self.session {
                    let on = !s.playing;
                    self.session_play(on);
                } else if self.idle {
                    self.play(&mpv)?;
                } else {
                    mpv.command(&["cycle", "pause"])?;
                    self.remember_where();
                }
            }
            Ask::Play => {
                self.not_following()?;
                let mpv = self.main(shared)?;
                if self.session.is_some() {
                    self.session_play(true);
                } else if self.idle {
                    self.play(&mpv)?;
                } else {
                    mpv.set("pause", "no")?;
                }
            }
            Ask::Pause => {
                self.not_following()?;
                if self.session.is_some() {
                    self.session_play(false);
                } else if let Some(m) = self.first_mpv() {
                    m.set("pause", "yes")?;
                    self.remember_where();
                }
            }
            Ask::Stop => {
                if let Some(m) = self.first_mpv() {
                    self.stop(&m)?;
                }
            }
            Ask::Next => {
                self.not_following()?;
                let mpv = self.main(shared)?;
                self.next(&mpv)?;
            }
            Ask::Previous => {
                self.not_following()?;
                let mpv = self.main(shared)?;
                self.previous(&mpv)?;
            }
            Ask::Jump { id } => {
                self.not_following()?;
                let mpv = self.main(shared)?;
                self.jump(&mpv, id)?;
            }
            Ask::Remove { id } => {
                let mpv = self.main(shared)?;
                self.remove(&mpv, id)?;
            }
            Ask::Move { id, after, into } => {
                let place = after.map_or(
                    ronoctua_player::queue::Place::Start(into),
                    ronoctua_player::queue::Place::After,
                );
                self.queue.move_to(id, place)?;
                if let Some(m) = self.first_mpv() {
                    self.refresh_lookahead(&m);
                }
            }
            Ask::Rename { id, name } => self.queue.rename(id, &name)?,
            Ask::SaveQueue { to, name } => self.save_queue(&to, name)?,
            Ask::Clear => {
                if let Some(m) = self.first_mpv() {
                    self.stop(&m)?;
                }
                self.clear();
            }
            Ask::Seek { seconds, relative } => {
                self.not_following()?;
                if self.session.is_some() {
                    let at = if relative {
                        self.clock_now() + seconds
                    } else {
                        seconds
                    };
                    self.session_seek(at);
                } else if let Some(m) = self.first_mpv() {
                    m.command(&[
                        "seek",
                        &seconds.to_string(),
                        if relative { "relative" } else { "absolute" },
                        "exact",
                    ])?;
                }
            }
            Ask::Volume { percent } => {
                let p = percent.min(130);
                self.state.volume = p;
                if self.session.is_some() {
                    self.redress()?;
                } else if let Some(m) = self.first_mpv() {
                    m.set("volume", &p.to_string())?;
                }
                self.remember_where();
            }
            Ask::Mute => {
                if self.session.is_some() {
                    self.state.muted = !self.state.muted;
                    self.redress()?;
                } else if let Some(m) = self.first_mpv() {
                    m.command(&["cycle", "mute"])?;
                }
            }
            Ask::Speed { factor } => {
                self.not_following()?;
                let factor = factor.clamp(0.25, 4.0);
                let t = self.clock_now();
                if let Some(s) = self.session.as_mut() {
                    s.speed = factor;
                    if let session::Clock::Virtual { .. } = s.clock {
                        s.clock = session::Clock::Virtual {
                            at: t,
                            since: Instant::now(),
                        };
                    }
                    self.state.speed = factor;
                    self.tick();
                } else if let Some(m) = self.first_mpv() {
                    m.set("speed", &factor.to_string())?;
                }
            }
            Ask::Repeat => {
                self.state.repeat = self.state.repeat.next();
                if self.session.is_none()
                    && let Some(m) = self.first_mpv()
                {
                    apply_repeat(&m, self.state.repeat);
                    self.refresh_lookahead(&m);
                }
                self.remember_where();
            }
            Ask::Shuffle => {
                self.shuffle(!self.state.shuffle);
                if let Some(m) = self.first_mpv() {
                    self.refresh_lookahead(&m);
                }
                self.remember_where();
            }
            Ask::Track { lane, track } => self.choose_track(lane, track)?,
            Ask::AddLane {
                source,
                kind,
                track,
                start_seconds,
                over,
            } => self.add_lane(&source, kind, track, start_seconds, over)?,
            Ask::Lane { lane, change } => self.change_lane(lane, change)?,
            Ask::Arrange { arrangement } => self.arrange(arrangement)?,
            Ask::SaveSet { to, name } => self.save_set(&to, name)?,
            // THE AMBIENT SOUNDS, apart: nothing above this reaches them.
            Ask::Ambient { id, on } => self.ambient(&id, on, shared)?,
            Ask::AmbientVolume { id, percent } => self.ambient_volume(&id, percent, shared)?,
            Ask::AmbientSilence => self.ambient_silence(shared)?,
            Ask::AmbientToggle => self.ambient_toggle(shared)?,
            // HELD ON ITS OWN CONNECTION by `serve_one`; it never reaches here.
            Ask::Sinks => {}
            // ANSWERED BY `serve_one` outside the lock — a sheet may be
            // read, and the lock is every door's; they never reach here.
            Ask::Sheet { .. } | Ask::Draft { .. } => {}
            Ask::Retag {
                location,
                stamp,
                edits,
            } => self.retag(location, stamp, edits)?,
            Ask::SaveCover { location, at } => self.save_cover(location, at)?,
            Ask::Refresh { location } => self.refresh(&location)?,
            Ask::WriteDraft { id } => self.write_draft(id)?,
            Ask::DropDraft { id } => self.drop_draft(id)?,
            Ask::KeepDraft { id } => self.keep_draft(id)?,
            // THE LOOKUPS AND THE DIRECTORIES: checked here, asked on a
            // desk's thread.
            Ask::Find { query } if query.directory().is_some() => self.look(query)?,
            Ask::Find { query } => self.find(query)?,
            Ask::Take {
                desk,
                candidate,
                fields,
                cover,
                to,
                replace,
            } => self.take(desk, candidate, &fields, cover, to, replace)?,
            Ask::SuggestedCover { candidate } => self.suggested_cover(candidate)?,
            // ANSWERED BY `serve_one`: a page to the one who asked, a key
            // tried on the network and a favourite's file written, outside
            // the lock; they never reach here.
            Ask::Desk { .. } | Ask::KeyCheck { .. } | Ask::Favourite { .. } => {}
            Ask::Reload => {
                self.prefs = Self::reread_prefs();
                self.retry_a_refused_translation();
            }
            Ask::Invite => {
                let code = ronoctua_player::remote::code_of(
                    ronoctua_player::remote::random().map_err(anyhow::Error::from)?,
                );
                self.invite = Some((code, Instant::now()));
            }
            // PAIRING TALKS TO ANOTHER MACHINE, so it is `serve_one`'s, outside
            // the lock; it never reaches here.
            Ask::PairWith { .. } => {}
            Ask::SaveQueueAgain { force } => self.save_queue_again(force)?,
            Ask::LyricsTranslation { on, to } => self.translate_lyrics(on, to)?,
            // ASKS THIS PLAYER KNOWS AND DOES NOT DO YET: refused in a
            // sentence, never taken as if they worked.
            // THE CATALOGUE: an ingest a door asked (it showed the source
            // stale) and the posters of what it shows, each on a thread of
            // its own (`catalog.rs`). Adding, removing and switching a
            // source write the person's sources' file, on the asking
            // connection's thread (`serve_one`); they never reach here.
            Ask::Ingest { source, category } => self.ingest(&source, category.as_deref())?,
            Ask::Posters { items } => self.posters(&items)?,
            Ask::AddSource { .. } | Ask::RemoveSource { .. } | Ask::SetSource { .. } => {}
            // CLOSING IS `quit`'s, taken before the lock — it ends the
            // process. Reaching here is a defect, and says so.
            Ask::Quit => {
                return Err(anyhow::anyhow!(
                    "closing the player is `engine::quit`'s, not an ask's"
                )
                .into());
            }
            Ask::Join { host, role } => {
                let peers = ronoctua_player::remote::peers().read();
                let Some(peer) = peers.hosts.into_iter().find(|h| h.name == host) else {
                    return Err(anyhow::anyhow!("{host} is not paired with this machine").into());
                };
                // WHERE THIS MACHINE WAS, once: joining a second machine while
                // following a first keeps the place this machine had.
                let resume = match self.following.take() {
                    Some(f) => {
                        f.stop.store(true, std::sync::atomic::Ordering::SeqCst);
                        f.resume
                    }
                    None => {
                        let here = (self.state.current, self.state.position);
                        self.remember_where();
                        here
                    }
                };
                // WHAT THIS MACHINE PLAYED stops, and the window is not ours
                // while another machine's lanes play: nothing of the queue
                // follows mpv until this machine leaves.
                self.end_session();
                self.window.clear();
                if let Some(m) = self.first_mpv() {
                    let _ = m.set("pause", "yes");
                }
                let stop = crate::remote::follow(peer, role, shared.clone());
                self.following = Some(crate::remote::Following {
                    host,
                    role,
                    drift_ms: 0,
                    stop,
                    resume,
                });
            }
            Ask::Leave => self.leave_following(),
            Ask::Forget { name } => {
                let mut p = ronoctua_player::remote::peers().read();
                p.hosts.retain(|h| h.name != name);
                p.guests.retain(|g| g.name != name);
                ronoctua_player::remote::keep(&p)?;
            }
        }
        Ok(())
    }

    /// The first voice's mpv, when there is one.
    fn first_mpv(&self) -> Option<Arc<Mpv>> {
        self.voices.first().map(|v| v.mpv.clone())
    }

    /// Refused while this machine plays another's lanes: what plays is that
    /// machine's to pause, seek and change.
    fn not_following(&self) -> Result<(), Refused> {
        match &self.following {
            Some(f) => Err(Trouble::Following {
                host: f.host.clone(),
            }
            .into()),
            None => Ok(()),
        }
    }

    /// Every voice of what plays told its volume, its output and its tracks
    /// again — the player's volume or mute changed.
    fn redress(&mut self) -> Result<(), Refused> {
        let t = self.clock_now();
        let (lanes, arrangement) = self.lanes_now();
        let covers = arrangement.unwrap_or_default().covers;
        self.cast(&lanes, &covers, t)
    }

    /// A new viewer on `stream`, at `width × height` of `lane`'s picture —
    /// or of the first picture shown. A lane with no picture yet is waited
    /// for: its frames come when a voice draws it.
    pub fn add_viewer(
        &mut self,
        stream: UnixStream,
        width: u32,
        height: u32,
        lane: Option<LaneId>,
    ) -> Result<()> {
        let width = (width / 16).max(1) * 16;
        let height = height.max(1);
        let ring = ronoctua_frames::Ring::new("ronoctua-player-view", width, height)?;
        let mut line = serde_json::to_vec(&Say::Preview {
            width,
            height,
            slots: ronoctua_frames::SLOTS,
        })?;
        line.push(b'\n');
        ring.hand_over(&stream, &line)?;
        stream.set_nonblocking(true)?;
        self.viewers.push(Viewer {
            stream,
            ring,
            lane,
            owed: true,
        });
        for v in &self.voices {
            if self.drawn.contains(&(lane, v.serial)) {
                v.poke.ring();
            }
        }
        Ok(())
    }

    /// An event of the voice `serial`: what it measures is kept for its
    /// lanes, and — for the first voice while a file plays alone — the
    /// queue's window follows it as before.
    fn on_voice_event(&mut self, serial: u64, mpv: &Mpv, event: Event) {
        let Some(k) = self.voices.iter().position(|v| v.serial == serial) else {
            return;
        };
        let mut look_now = false;
        let mut redraw = false;
        {
            let v = &mut self.voices[k];
            match &event {
                Event::Property { id, value } => match (*id, value) {
                    (id::DURATION, Value::Float(d)) => v.duration = length_of_mpv(mpv, Some(*d)),
                    (id::DURATION, _) => v.duration = None,
                    (id::WIDTH | id::HEIGHT, _) => {
                        let w = mpv.text("dwidth").and_then(|x| x.parse().ok());
                        let h = mpv.text("dheight").and_then(|x| x.parse().ok());
                        v.size = w.zip(h);
                        redraw = true;
                    }
                    (id::TRACKS, _) => {
                        v.tracks = voice::tracks_of(mpv);
                        redraw = true;
                    }
                    (id::EOF, Value::Flag(done)) => {
                        v.eof = *done;
                        look_now = *done;
                    }
                    _ => {}
                },
                Event::EndFile {
                    error: Some(why), ..
                } => {
                    v.failed = Some(why.clone());
                    // WHAT THE PROGRAM BEHIND mpv SAID, from its log: the one
                    // place yt-dlp's reason is (`voice::said_of`).
                    v.said = voice::said_of(v.heard.iter());
                }
                Event::FileLoaded => {
                    v.failed = None;
                    v.eof = false;
                    v.file_loaded = true;
                    // ITS TRACKS AS THEY ARE NOW: the list's own change may
                    // come after this event, and a sound lane would be said
                    // to have no sound until it did.
                    v.tracks = voice::tracks_of(mpv);
                    if self.session.is_none() && k == 0 {
                        v.loaded = mpv.text("path").map(|p| location_of(&p));
                    }
                }
                Event::StartFile => v.starting(),
                Event::Log {
                    prefix,
                    level,
                    text,
                } => v.hear(prefix, level, text),
                _ => {}
            }
        }
        // A LINE OF mpv'S LOG is kept by its voice and nothing else.
        if matches!(event, Event::Log { .. }) {
            return;
        }
        if let Event::Property {
            id: id::IDLE,
            value: Value::Flag(idle),
        } = &event
            && k == 0
        {
            self.idle = *idle;
        }
        // A FILE THAT STARTS PLAYING is read before any door asks: its sheet
        // kept, its cover taken out for MPRIS (`tags.rs`).
        if matches!(event, Event::FileLoaded)
            && let Some(location) = mpv.text("path").map(|p| location_of(&p))
        {
            self.read_tags_of(&location);
        }
        if self.session.is_none() {
            let read = matches!(
                event,
                Event::FileLoaded | Event::Property { id: id::TRACKS, .. }
            );
            if k == 0 {
                self.on_event(mpv, event);
                // A SUBTITLE FILE opened for this leaf becomes a lane of it
                // once its own lanes — its tracks — are read.
                if read {
                    self.attach_waiting();
                }
            }
            if redraw {
                self.dirty = true;
            }
            return;
        }
        // PLAYING TOGETHER: the first voice's tags still name what plays.
        if k == 0 {
            match &event {
                Event::Property {
                    id: id::METADATA | id::TITLE,
                    ..
                } => self.on_event(mpv, event.clone()),
                Event::FileLoaded => {
                    self.loaded = mpv.text("path").map(|p| location_of(&p));
                    self.state.lyrics.clear();
                    self.state.lyrics_from = None;
                    self.lyrics_asked = None;
                }
                _ => {}
            }
        }
        match &event {
            Event::FileLoaded => {
                // WHAT WAS ADDED went with the file before: the voice's
                // subtitles are given again, once it has one loaded.
                self.subtitles.reloaded(serial);
                self.dress_subtitles();
                self.dirty = true;
            }
            Event::EndFile { error: Some(_), .. } => self.dirty = true,
            Event::Property { id: id::TIME, .. } => self.tick_soon(false),
            _ => {}
        }
        if redraw {
            self.map_pictures();
            // ITS TRACKS OR ITS SIZE: what the subtitles over it are read
            // from and drawn at.
            self.dress_subtitles();
            self.dirty = true;
        }
        if look_now {
            self.tick_soon(true);
        }
    }

    fn on_event(&mut self, mpv: &Mpv, event: Event) {
        match event {
            Event::Property { id, value } => {
                let tick = id == id::TIME;
                match (id, value) {
                    (id::PAUSE, Value::Flag(p)) => self.state.playing = !p && !self.idle,
                    (id::IDLE, Value::Flag(i)) => {
                        self.idle = i;
                        if i {
                            self.state.playing = false;
                            self.state.position = 0.0;
                            self.loaded = None;
                            if let Some(v) = self.voices.first_mut() {
                                v.loaded = None;
                            }
                            // THE WINDOW RAN OUT and nobody stopped it: the
                            // queue decides what comes next, not mpv. Asked of
                            // mpv as it is NOW — the event is older than any
                            // load made since, and a stop followed by a load
                            // leaves one behind.
                            if !self.window.is_empty()
                                && !self.stopping
                                && self.following.is_none()
                                && mpv.text("idle-active").as_deref() == Some("yes")
                            {
                                self.ended(mpv);
                            }
                        } else {
                            self.state.playing = mpv.text("pause").as_deref() == Some("no");
                        }
                    }
                    (id::TIME, Value::Float(t)) => {
                        self.state.position = t;
                        // A SPONSORED STRETCH IS JUMPED at its first frame.
                        if let Some(to) = sponsorblock::skip(t, &self.segments, &mut self.jumped) {
                            let _ = mpv.command(&["seek", &format!("{to:.3}"), "absolute+exact"]);
                        }
                    }
                    (id::TIME, _) => self.state.position = 0.0,
                    (id::DURATION, Value::Float(d)) => {
                        self.state.duration = length_of_mpv(mpv, Some(d));
                    }
                    (id::DURATION, _) => self.state.duration = None,
                    (id::VOLUME, Value::Float(v)) => {
                        self.state.volume = v.round().clamp(0.0, 130.0) as u8;
                    }
                    (id::MUTE, Value::Flag(m)) => self.state.muted = m,
                    (id::SPEED, Value::Float(s)) => self.state.speed = s,
                    // WHERE mpv IS IN ITS WINDOW, read again from mpv itself:
                    // the event may be older than a load made since.
                    (id::POS | id::PLAYLIST, _) => self.settle(mpv),
                    (id::METADATA, _) => {
                        let tags = metadata_of(mpv);
                        let pick = |keys: &[&str]| {
                            keys.iter().find_map(|k| {
                                tags.iter()
                                    .find(|(t, _)| t.eq_ignore_ascii_case(k))
                                    .map(|(_, v)| v.clone())
                            })
                        };
                        self.state.artist = pick(&["artist", "album_artist"]);
                        self.state.album = pick(&["album"]);
                        // A STREAM SAYS NEW THINGS as it plays — the song on
                        // the air — and its sheet is those words: every door
                        // showing it asks again.
                        if self.loaded.as_deref().is_some_and(|l| l.contains("://")) {
                            self.state.sheet_generation += 1;
                        }
                        self.find_lyrics(mpv, &tags);
                        self.ask_for_lyrics(&tags);
                    }
                    (id::TITLE, Value::Text(t)) => self.mpv_title = Some(t),
                    (id::TITLE, _) => self.mpv_title = None,
                    _ => {}
                }
                // A POSITION alone is said four times a second; anything else
                // at once.
                if !tick || self.last_tick.elapsed() >= Duration::from_millis(250) {
                    self.dirty = true;
                }
            }
            Event::EndFile {
                error: Some(why), ..
            } => {
                // SAID NOW, and acted on when mpv goes idle (`queue.rs`,
                // `ended`): a stream tries its next address, anything else
                // gives way to the next leaf. With what yt-dlp said of it,
                // when it said something (`voice::said_of`).
                self.failed = true;
                let said = self.voices.first().and_then(|v| v.said.clone());
                self.state.troubles = vec![Trouble::Unplayable { why, said }];
                self.dirty = true;
            }
            Event::EndFile { .. } => self.failed = false,
            Event::FileLoaded => {
                self.failed = false;
                self.failures = 0;
                self.loaded = mpv.text("path").map(|p| location_of(&p));
                // A STREAM OF A DIRECTORY STARTED — its sound, not its row:
                // the click it is owed and its place among the recent.
                self.stream_started();
                self.state.troubles.clear();
                self.state.lyrics.clear();
                self.state.lyrics_from = None;
                self.lyrics_asked = None;
                self.segments.clear();
                self.jumped.clear();
                self.adopt_media_title(mpv);
                if self.prefs.sponsorblock
                    && let Some(item) = mpv.text("path").map(|p| location_of(&p))
                    && let Some(id) = sponsorblock::video_id(&item)
                {
                    self.fetches.push(Fetch::Segments {
                        item,
                        id,
                        categories: self.prefs.skipped.clone(),
                    });
                }
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// The location of the item playing.
    fn playing(&self) -> Option<String> {
        self.loaded.clone()
    }

    /// A leaf that arrived as an address, and nobody named, takes mpv's
    /// own title for it at FILE_LOADED — when the ytdl hook opened it (mpv
    /// plays another address than the one it was given, the video's stream)
    /// and so the title is the video's, not the file name of an address nor
    /// the song a radio's stream is playing now.
    fn adopt_media_title(&mut self, mpv: &Mpv) {
        let Some(current) = self.state.current else {
            return;
        };
        let Some(path) = mpv.text("path") else { return };
        let opened_by_the_hook = mpv
            .text("stream-open-filename")
            .is_some_and(|opened| opened != path);
        if !path.contains("://") || !opened_by_the_hook {
            return;
        }
        if let Some(title) = mpv.text("media-title") {
            self.adopt_title(current, &path, &title);
        }
    }

    /// The fourth source of lyrics, when the other three had none and the
    /// person allowed it: `lrclib.net`, asked once per item.
    fn ask_for_lyrics(&mut self, tags: &[(String, String)]) {
        if !self.prefs.lyrics_online || !self.state.lyrics.is_empty() {
            return;
        }
        let Some(item) = self.playing() else { return };
        if self.lyrics_asked.as_deref() == Some(item.as_str()) {
            return;
        }
        let tag = |k: &str| {
            tags.iter()
                .find(|(t, _)| t.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.clone())
        };
        // AN ARTIST AND A TITLE, or nothing is asked: a film's file name is
        // not a song, and a request made of it is a guess sent to a stranger.
        let (Some(artist), Some(title)) = (self.state.artist.clone(), tag("title")) else {
            return;
        };
        self.lyrics_asked = Some(item.clone());
        self.fetches.push(Fetch::Lyrics {
            item,
            artist,
            title,
            album: self.state.album.clone(),
            duration_seconds: self.state.duration,
        });
    }

    /// The lyrics of what just loaded: a `.lrc` beside it, or its own tag.
    fn find_lyrics(&mut self, mpv: &Mpv, tags: &[(String, String)]) {
        let path = mpv
            .text("path")
            .map(|p| location_of(&p))
            .filter(|p| !p.contains("://"));
        if let Some(lrc) = path.as_deref().map(|p| Path::new(p).with_extension("lrc"))
            && let Ok(text) = std::fs::read_to_string(&lrc)
        {
            self.state.lyrics = ronoctua_player::lyrics::parse(&text);
            self.state.lyrics_from = Some("file".to_owned());
            return;
        }
        if let Some((_, text)) = tags.iter().find(|(k, _)| {
            ["lyrics", "unsyncedlyrics", "lyrics-eng", "uslt"]
                .iter()
                .any(|w| k.eq_ignore_ascii_case(w) || k.to_ascii_lowercase().starts_with("lyrics"))
        }) {
            self.state.lyrics = ronoctua_player::lyrics::parse(text);
            self.state.lyrics_from = Some("tag".to_owned());
        }
    }
}

/// Repeat, as far as mpv takes part in it: ONE file looped by mpv itself
/// (no reload, no gap); the queue's going round is ours (`queue.rs`), so
/// mpv's own playlist never loops.
fn apply_repeat(mpv: &Mpv, repeat: Repeat) {
    let _ = mpv.set("loop-playlist", "no");
    let _ = mpv.set(
        "loop-file",
        if repeat == Repeat::One { "inf" } else { "no" },
    );
}

/// The length of what `mpv` plays, from the `duration` it just said — or
/// none, for a LIVE stream.
///
/// mpv says a `duration` for a stream from the network that cannot be sought
/// and whose size the server never said: what has ARRIVED so far, growing
/// with it. Measured on mpv 0.41 against an endless MP3 over HTTP/1.0 with no
/// length, the way every Icecast radio sends one: `duration` 0, 3, 6 s as it
/// played, `file-size` unavailable, `demuxer-via-network` yes, `seekable` and
/// `partially-seekable` no. Taken as a length, the door drew a bar that was
/// always full and a clock that said the station ended where it was (the
/// final battery's `footer-media`: "the stream has a length: 2.53").
fn length_said(
    duration: Option<f64>,
    via_network: Option<&str>,
    seekable: Option<&str>,
) -> Option<f64> {
    let live = via_network == Some("yes") && seekable == Some("no");
    duration.filter(|_| !live)
}

/// [`length_said`] of `mpv`, asked now.
fn length_of_mpv(mpv: &Mpv, duration: Option<f64>) -> Option<f64> {
    length_said(
        duration,
        mpv.text("demuxer-via-network").as_deref(),
        mpv.text("seekable").as_deref(),
    )
}

fn metadata_of(mpv: &Mpv) -> Vec<(String, String)> {
    let Some(json) = mpv.text("metadata") else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&json) else {
        return Vec::new();
    };
    map.into_iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
        .collect()
}

/// One connection.
pub fn serve_one(stream: UnixStream, shared: &Shared) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut input = BufReader::new(read_half);
    let Ok(Some(line)) = wire::read_raw(&mut input) else {
        return;
    };
    // AN ASK THIS PLAYER DOES NOT KNOW is answered, never dropped: a door
    // newer or older than the daemon hears that the two do not match.
    let ask: Ask = match serde_json::from_str(&line) {
        Ok(a) => a,
        Err(err) => {
            let mut out = stream;
            let _ = wire::write_line(
                &mut out,
                &Say::Refused(Trouble::NotUnderstood {
                    why: err.to_string(),
                }),
            );
            return;
        }
    };
    // CLOSING, before the lock, which `quit` takes itself: the farewell goes
    // to every watcher, and then the answer to the one who asked — which a
    // door does not wait for (`client::tell_if_running`), so it may find the
    // stream already gone.
    if ask == Ask::Quit {
        quit(shared);
        let mut out = stream;
        let _ = wire::write_line(&mut out, &Say::Done);
        return;
    }
    // THE OUTPUTS ARE READ while this connection stays open: the door that
    // shows them holds it, and closing it — or the door going away — lets
    // go. Nothing is read from it; its end is what is waited for.
    if ask == Ask::Sinks {
        hold_sinks(stream, shared);
        return;
    }
    // A SHEET MAY BE READ — programs, seconds at worst — and this connection
    // waits for it; the lock does not (`tags.rs`). A draft needs the sheet
    // first, for the value it starts from.
    if let Ask::Sheet { location } = &ask {
        let say = tags::sheet_for(location, shared);
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        return;
    }
    if let Ask::Draft { location, field } = &ask {
        let say = drafts::open_draft(location, *field, shared);
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        return;
    }
    // A STAR IS A FILE WRITTEN under a lock of its own: not the engine's
    // (`directory.rs`).
    if let Ask::Favourite { kind, id, on } = &ask {
        let say = directory::favourite(*kind, id, *on, shared);
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        let (lock, bell) = &**shared;
        if let Ok(mut e) = lock.lock()
            && e.dirty
        {
            e.broadcast();
        }
        bell.notify_all();
        return;
    }
    // A KEY IS TRIED ON THE NETWORK, and this connection waits for the
    // service's answer; the lock does not (`desk.rs`).
    if let Ask::KeyCheck { service } = &ask {
        let say = desk::key_check(*service, shared);
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        return;
    }
    // THE CATALOGUE'S SOURCES: a pasted link is fetched to be recognised,
    // and the book is written — the connection waits for it; the lock is
    // held only for the book (`catalog.rs`).
    let catalogued = match &ask {
        Ask::AddSource { url, adult } => Some(catalog::add_source(url, *adult, shared)),
        Ask::RemoveSource { id } => Some(catalog::remove_source(id, shared)),
        Ask::SetSource { id, posters, adult } => {
            Some(catalog::set_source(id, *posters, *adult, shared))
        }
        _ => None,
    };
    if let Some(say) = catalogued {
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        return;
    }
    // PAIRING TALKS TO ANOTHER MACHINE, and a host on the tailnet is given
    // seconds: the engine's lock is not held across it.
    if let Ask::PairWith { to, code } = &ask {
        let say = match crate::remote::pair_with(to, code, shared) {
            Ok(_) => Say::Done,
            Err(err) => Say::Refused(Trouble::Remote {
                why: format!("{err:#}"),
            }),
        };
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        let (lock, bell) = &**shared;
        if let Ok(mut e) = lock.lock() {
            e.broadcast();
        }
        bell.notify_all();
        return;
    }
    // WHAT IS PUT IS READ FIRST, outside the lock: a folder is a walk of the
    // disk and a playlist a file to read, and the lock is every door's.
    // AND WHERE IT IS, for `once`: a file's canonical path is a question
    // for the disk, asked here too; under the lock only text is compared.
    let (ask, resolved) = match ask {
        Ask::Put {
            what,
            after,
            into,
            play,
            replace,
            once,
        } => {
            let places = if once { places_of(&what) } else { Vec::new() };
            (
                None,
                Some((resolve_all(what), places, after, into, play, replace)),
            )
        }
        other => (Some(other), None),
    };
    let (lock, bell) = &**shared;
    let Ok(mut e) = lock.lock() else { return };
    // A PLAYER ON ITS WAY OUT answers whoever connected before the socket
    // left the disk with its farewell: a watcher that arrived then would
    // otherwise hear the stream end, and say "not running" where it should
    // close.
    if e.leaving {
        let mut out = stream;
        let _ = wire::write_line(&mut out, &Say::Farewell);
        return;
    }
    if let Some((resolved, places, after, into, play, replace)) = resolved {
        e.dirty = true;
        // "IS IT IN THE QUEUE ALREADY?" under the lock, where nothing can be
        // put between the answer and the put.
        let put = e
            .not_queued(&places)
            .and_then(|()| e.put_resolved(resolved, after, into, play, replace, shared))
            .map(|_| ());
        let say = answer_of(put, &mut e.state.troubles);
        e.broadcast();
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        drop(e);
        bell.notify_all();
        return;
    }
    let Some(ask) = ask else { return };
    match ask {
        Ask::Watch => {
            let mut s = stream;
            let _ = s.set_write_timeout(Some(WATCHER_PATIENCE));
            // THE OUTLINE, THEN THE STATE: a door knows the queue's shape
            // before it reads which leaf of it is current.
            let outline = Say::Queue(Box::new(e.queue.outline()));
            if wire::write_line(&mut s, &outline).is_ok()
                && wire::write_line(&mut s, &Say::State(Box::new(e.state()))).is_ok()
            {
                e.watchers.push(s);
            }
        }
        Ask::Queue => {
            let mut out = stream;
            let _ = wire::write_line(&mut out, &Say::Queue(Box::new(e.queue.outline())));
        }
        // A DESK'S PAGE, to the one who asked and never to the watchers.
        Ask::Desk { kind } => {
            let mut out = stream;
            let _ = wire::write_line(&mut out, &Say::Desk(Box::new(e.desk_page(kind))));
        }
        Ask::Look => {
            let mut out = stream;
            let _ = wire::write_line(&mut out, &Say::State(Box::new(e.state())));
        }
        Ask::Preview {
            width,
            height,
            lane,
        } => {
            // A PREVIEW WANTS A PICTURE, so it wants the engine up.
            if let Err(err) = e.main(shared) {
                let mut out = stream;
                let _ = wire::write_line(
                    &mut out,
                    &Say::Refused(Trouble::NoEngine {
                        why: format!("{err:#}"),
                    }),
                );
                return;
            }
            if let Err(err) = e.add_viewer(stream, width, height, lane) {
                tracing::warn!(error = %err, "a preview was refused");
            }
        }
        other => {
            let say = e.ask(other, shared);
            e.broadcast();
            let mut out = stream;
            let _ = wire::write_line(&mut out, &say);
        }
    }
    drop(e);
    bell.notify_all();
}

/// Holds the outputs read for as long as `stream` stays open ([`Ask::Sinks`]):
/// counted in, answered, waited on until the door closes it, counted out.
fn hold_sinks(stream: UnixStream, shared: &Shared) {
    let (lock, bell) = &**shared;
    {
        let Ok(mut e) = lock.lock() else { return };
        if e.leaving {
            let mut out = stream;
            let _ = wire::write_line(&mut out, &Say::Farewell);
            return;
        }
        e.sinks.holders += 1;
        e.mind_sinks();
        e.broadcast();
    }
    bell.notify_all();
    let mut out = stream;
    if wire::write_line(&mut out, &Say::Done).is_ok() {
        let mut rest = [0_u8; 64];
        // UNTIL IT ENDS: a door says nothing more on it.
        while matches!(out.read(&mut rest), Ok(n) if n > 0) {}
    }
    if let Ok(mut e) = lock.lock() {
        e.sinks.holders = e.sinks.holders.saturating_sub(1);
        e.mind_sinks();
        e.broadcast();
    }
    bell.notify_all();
}

/// Where a path the person gave is played from: absolute, so the queue means
/// the same file whoever reads it.
#[must_use]
pub fn absolute(item: &str) -> String {
    if item.contains("://") {
        return item.to_owned();
    }
    let p = PathBuf::from(item);
    std::fs::canonicalize(&p)
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

/// A test run AGAIN, alone, in a process of its own ([`alone::run`]).
#[cfg(test)]
pub(crate) mod alone {
    /// The mark of a test run again in a process of its own.
    const ALONE: &str = "RONOCTUA_TEST_ALONE";

    /// Whether this is the run [`run`] started: the half of a test that
    /// returns at once otherwise.
    #[must_use]
    pub(crate) fn is_alone() -> bool {
        std::env::var_os(ALONE).is_some()
    }

    /// Runs the test `name` of `module` (its `module_path!()`) again, ALONE,
    /// in a process of its own: a home of its own (every `XDG_*` root in a
    /// scratch folder, so the engine it makes reads and writes nothing of
    /// the person's), and `env` set — what a test of a program's variable
    /// needs, since the tests of one binary share one process and one
    /// environment. Panics when it failed, or did not run.
    pub(crate) fn run(module: &str, name: &str, env: &[(&str, &std::ffi::OsStr)]) {
        let home = std::env::temp_dir().join(format!(
            "ronoctua-engine-alone-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&home);
        let module = module.split_once("::").map_or(module, |(_, m)| m);
        let test = format!("{module}::{name}");
        let mut run = std::process::Command::new(std::env::current_exe().expect("this binary"));
        run.args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"])
            .env(ALONE, "1")
            .env("HOME", &home)
            .env_remove("RONOCTUA_INSTANCE");
        for (variable, leaf) in [
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_RUNTIME_DIR", "run"),
        ] {
            let dir = home.join(leaf);
            std::fs::create_dir_all(&dir).expect("a root");
            run.env(variable, dir);
        }
        for (variable, value) in env {
            run.env(variable, value);
        }
        let out = run.output().expect("the test, again");
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{test} failed alone:\n{said}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            said.contains("1 passed"),
            "{test} did not run alone:\n{said}"
        );
        let _ = std::fs::remove_dir_all(&home);
    }
}

#[cfg(test)]
mod tests {

    /// FAILS IF: a watcher of a player asked to close is not told so, or
    /// its stream stays open after the word — a door would wait for a state
    /// that never comes instead of closing.
    #[test]
    fn every_watcher_hears_the_farewell_and_then_nothing() {
        use std::io::BufReader;
        let (ours, theirs) = std::os::unix::net::UnixStream::pair().expect("a pair");
        super::farewell_to(vec![ours]);
        let mut input = BufReader::new(theirs);
        let said: Option<super::Say> = super::wire::read_line(&mut input).expect("reads");
        assert_eq!(said, Some(super::Say::Farewell));
        let after: Option<super::Say> = super::wire::read_line(&mut input).expect("reads");
        assert_eq!(after, None, "the stream ends after the farewell");
    }

    /// FAILS IF: a watcher that stopped reading holds the farewell — and the
    /// engine's lock it is said under — for as long as it stays stopped: the
    /// player asked to close would never leave, and every ask behind the
    /// lock would hang.
    #[test]
    fn a_watcher_that_stopped_reading_does_not_hold_the_farewell() {
        use std::io::Write as _;
        let (mut ours, theirs) = std::os::unix::net::UnixStream::pair().expect("a pair");
        // ITS BUFFER FULL, as a door stopped for a while leaves it.
        ours.set_nonblocking(true).expect("nonblocking");
        let chunk = [b'x'; 4096];
        while ours.write(&chunk).is_ok() {}
        ours.set_nonblocking(false).expect("blocking again");
        let began = std::time::Instant::now();
        super::farewell_to(vec![ours]);
        assert!(
            began.elapsed() < super::WATCHER_PATIENCE * 3,
            "the farewell waited {:?} for a watcher that does not read",
            began.elapsed()
        );
        drop(theirs);
    }

    /// FAILS IF: a name only libgme reads reaches libmpv without the demuxer
    /// named, and is refused as an unrecognized format — or the option
    /// leaks onto a file that libmpv finds on its own.
    #[test]
    fn the_music_of_consoles_names_its_demuxer() {
        let gme = Some("libgme");
        assert_eq!(super::load_options("/m/tune.vgm"), gme);
        assert_eq!(super::load_options("/m/Zelda.SPC"), gme);
        assert_eq!(
            super::load_options("http://100.1.2.3:45517/m/a.nsf?k=1"),
            gme
        );
        assert_eq!(super::load_options("/m/song.flac"), None);
        assert_eq!(super::load_options("/m/tune.sid"), None);
    }

    /// FAILS IF: a stream from the network that cannot be sought is given
    /// the length of what has arrived (mpv's growing `duration`), or a file
    /// or a seekable stream loses its length — or an unknown answer from mpv
    /// is taken for "live".
    #[test]
    fn a_live_stream_has_no_length() {
        use super::length_said;
        assert_eq!(length_said(Some(2.53), Some("yes"), Some("no")), None);
        assert_eq!(length_said(Some(30.0), Some("no"), Some("yes")), Some(30.0));
        assert_eq!(
            length_said(Some(600.0), Some("yes"), Some("yes")),
            Some(600.0)
        );
        assert_eq!(length_said(Some(30.0), None, None), Some(30.0));
        assert_eq!(length_said(Some(30.0), Some("no"), Some("no")), Some(30.0));
        assert_eq!(length_said(None, Some("no"), Some("yes")), None);
    }
}
