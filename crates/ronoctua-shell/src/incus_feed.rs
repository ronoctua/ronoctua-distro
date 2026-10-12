//! The Incus view's threads: what the daemon says, asked off the loop.
//!
//! # Events, and a clock only for what no event says
//!
//! The listing is asked when the view opens, after a deed of the view's own,
//! and when the daemon says something changed — `/1.0/events`, a WebSocket
//! over its socket (`ronoctua_incus::Events`). THE WHOLE STREAM IS ITS
//! THREAD'S, the handshake included: opening it is a connect, a request and
//! the daemon's `101`, which a daemon still starting answers late, and a
//! door's loop never waits for it. The feed keeps the stream's
//! [`ronoctua_incus::Stopper`] once it opened, under one lock with a flag
//! that says the feed is gone: dropping the feed shuts the socket (the
//! blocked read answers at once), and a stream that opens after the feed
//! went is shut on the spot.
//!
//! A stream the DAEMON ends (it restarted: a switch, an update) is said
//! ([`Arrival::Events`]: the view says what it shows may be old) and opened
//! again — with the next listing, or on the clock's next tick, and what
//! changed meanwhile is listed once it is open.
//!
//! Some things change with no event at all: a container's address arrives
//! with its DHCP lease, after the start the event announced, and processor
//! time, memory and processes move all the time. While the view SHOWS one
//! of them ([`crate::incus::Wants::states`]) a clock reads that state again
//! every [`crate::incus::STATE_SECONDS`]; it is armed only then (and while a
//! lost stream waits to be opened again), and its thread ends as soon as
//! nothing needs it.
//!
//! A remote's catalogue and a pool's volumes are asked when their detail is
//! shown ([`crate::incus::View::wants`], [`Feed::follow`]); an answer for one
//! no longer shown is dropped, so a slow remote cannot overwrite the one on
//! screen.
//!
//! # The person's consent, read here and watched
//!
//! Which of the remotes' hosts are let is read from `consent.json` by the
//! feed (never on the loop), and the file is WATCHED for as long as the feed
//! lives: a host let in another door, or the consent row pressed here, is
//! the catalogue asked for now.
//!
//! # One ping for everything
//!
//! Every thread pushes what it has onto one list and rings the door's bell;
//! the door takes the whole list at once ([`Feed::take`]) and hands each
//! arrival to [`Readings::took`].
//!
//! # A deed outlives its door
//!
//! A deed (a stop, a snapshot) runs on its own thread and is counted until
//! it answered ([`Feed::deeds_in_flight`]). A door that closes while one is
//! out QUIETS its feed ([`Feed::quiet`]: no events, no clock, no watch) and
//! keeps it until the answer came, so a refusal is still said on its line.
//!
//! # A scene, for a photograph
//!
//! [`Feed::detached`] answers on the spot, on the caller's thread — and over
//! a [`Scene`], the daemon's answers handed in whole, when a photograph gives
//! one: then nothing reaches this machine's daemon, its queue or the person's
//! consent, and what would have been queued or let is kept for the report.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use ronoctua_incus::{
    Image, Incus, Instance, Network, Offered, Pool, Profile, Project, Refusal, Remote, State,
    Status, Volume, Why,
};
use serde::Deserialize;

use crate::incus::{
    Catalogue, Deed, Following, Listing, Readings, STATE_SECONDS, Use, Volumes, Wants,
};

/// The variable that names a photograph's scene for the FOOTER's camera
/// (`photograph --door footer --command incus-open`). The footer's camera has
/// no flag of its own for one view: it runs itself again in a room that
/// inherits the environment, as the containers' stand-in podman reaches it.
/// Read only by a footer that is a photograph's (`Screen::photographing`) —
/// a footer on a screen never reads it.
pub const SCENE_VARIABLE: &str = "RONOCTUA_PHOTOGRAPH_INCUS_SCENE";

/// The daemon's answers, handed in whole — a photograph's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scene {
    /// Every answer is this refusal (Incus not here, a group not yet in).
    pub refused: Option<Why>,
    /// The instances.
    pub instances: Vec<Instance>,
    /// The images here.
    pub images: Vec<Image>,
    /// The profiles.
    pub profiles: Vec<Profile>,
    /// The networks.
    pub networks: Vec<Network>,
    /// The storage pools.
    pub pools: Vec<Pool>,
    /// The projects.
    pub projects: Vec<Project>,
    /// Each pool's volumes, by its name.
    pub volumes: BTreeMap<String, Vec<Volume>>,
    /// The client's remotes.
    pub remotes: Vec<Remote>,
    /// Each remote's catalogue, by its name.
    pub catalogues: BTreeMap<String, Vec<Offered>>,
    /// The hosts the person has let.
    pub consented: Vec<String>,
    /// The CLI's path, for what a console would have started.
    pub cli: Option<String>,
    /// The daemon's stream of events ended (it restarted): the view says
    /// that what it shows may be old.
    pub events_ended: bool,
    /// Every deed is refused with this (a stop the daemon is busy with), and
    /// its sentence said; without it, a deed is a yes that changes nothing.
    pub deeds_refused: Option<Why>,
}

impl Scene {
    /// The scene a file holds — both cameras read it here.
    ///
    /// # Errors
    ///
    /// The file cannot be read, or is not a scene (a word that is not one of
    /// its fields included).
    pub fn read(file: &std::path::Path) -> anyhow::Result<Self> {
        use anyhow::Context as _;
        let raw = std::fs::read_to_string(file)
            .with_context(|| format!("could not read `{}`", file.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("`{}` is not an Incus scene", file.display()))
    }

    /// A photograph's footer's scene: the file [`SCENE_VARIABLE`] names, an
    /// empty scene when it names none. A file that does not read is a
    /// refusal ON THE PICTURE, and the reason is in the log — never an empty
    /// scene that would pass for one.
    #[must_use]
    pub fn of_the_footer_camera(named: Option<&std::path::Path>) -> Self {
        let Some(file) = named else {
            return Self::default();
        };
        Self::read(file).unwrap_or_else(|e| {
            tracing::error!(error = %format!("{e:#}"), "the footer camera's Incus scene");
            Self {
                refused: Some(Why::Invalid),
                ..Self::default()
            }
        })
    }
}

/// Something a thread brought.
#[derive(Debug, Clone)]
pub enum Arrival {
    /// A listing, or why not.
    Listed(Result<Listing, Refusal>),
    /// The client's remotes, or why not.
    Remotes(Result<Vec<Remote>, Refusal>),
    /// The hosts the person has let, read again.
    Consented(BTreeSet<String>),
    /// A remote's catalogue.
    Catalogue(Catalogue),
    /// A pool's volumes.
    Volumes(Volumes),
    /// An instance's state, read again on the clock.
    State {
        /// Whose.
        name: String,
        /// Its state, or why not.
        answer: Result<State, Refusal>,
    },
    /// The daemon's events: followed now (`true`), or the stream ended or
    /// would not open (`false`).
    Events(bool),
    /// A deed of the view's, done or refused.
    Done {
        /// What was asked.
        deed: Deed,
        /// Its thing's name, for the sentence.
        name: String,
        /// How it went.
        result: Result<(), Refusal>,
    },
}

/// Where the answers come from.
#[derive(Debug, Clone)]
enum Daemon {
    /// This machine's daemon.
    Live(Incus),
    /// A photograph's scene, which changes only by a consent let.
    Scene(Arc<Mutex<Scene>>),
    /// Incus is not here — every answer is this.
    Absent(Refusal),
}

/// The daemon's stream of events, as the feed's threads share it.
#[derive(Debug, Default)]
enum Stream {
    /// Not opened (not yet, or not a live daemon's).
    #[default]
    Closed,
    /// A thread is opening it — the handshake, which may wait.
    Opening,
    /// Open: its stopper, held for its drop, which shuts it from here.
    Open {
        /// The stream's stopper.
        _stopper: ronoctua_incus::Stopper,
    },
    /// The daemon ended it, or it would not open: opened again with the next
    /// listing, or on the clock's next tick.
    Lost,
}

/// What the feed shares with its own threads, under one lock.
#[derive(Debug, Default)]
struct Watch {
    /// The feed went (dropped, or quieted): a stream that opens now is shut
    /// at once, nothing opens again, and the clock ends.
    gone: bool,
    /// The events' stream.
    stream: Stream,
    /// That the stream is lost was said, and not said again on every try.
    said_lost: bool,
    /// The instances whose state the clock reads again.
    states: Vec<String>,
    /// The clock's thread runs.
    ticking: bool,
}

impl Watch {
    /// Whether the clock has work: a state to read again, or a stream to
    /// open again.
    fn armed(&self) -> bool {
        !self.gone && (!self.states.is_empty() || matches!(self.stream, Stream::Lost))
    }
}

/// What a thread shares with the door.
#[derive(Clone)]
struct Shared {
    daemon: Daemon,
    /// Everything is asked on the caller's thread — a photograph's.
    detached: bool,
    arrived: Arc<Mutex<Vec<Arrival>>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    /// A listing thread is running.
    listing: Arc<AtomicBool>,
    /// And somebody asked for another since it began.
    again: Arc<AtomicBool>,
    /// The client's remotes, as read last: whose hosts' consent is read,
    /// and whose address a catalogue's pass is for.
    remotes: Arc<Mutex<Vec<Remote>>>,
    /// The events' stream and the clock, and whether the feed is gone; the
    /// bell wakes the clock early only to end it.
    watch: Arc<(Mutex<Watch>, Condvar)>,
    /// The volumes are old: something about storage changed since.
    volumes_stale: Arc<AtomicBool>,
    /// Deeds told and not answered yet.
    deeds: Arc<AtomicUsize>,
}

impl Shared {
    fn arrive(&self, a: Arrival) {
        if let Ok(mut list) = self.arrived.lock() {
            list.push(a);
        }
        (self.wake)();
    }

    /// The watch, a poisoned lock taken as it is: what it holds is flags.
    fn watch(&self) -> MutexGuard<'_, Watch> {
        self.watch.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One listing: everything the kinds column counts, as one answer — the
    /// daemon answers all or none. A stream of events that was lost is
    /// opened again with it.
    fn list_now(&self) {
        let lost = matches!(self.watch().stream, Stream::Lost);
        if lost {
            self.follow_events(false);
        }
        let listed = match &self.daemon {
            Daemon::Live(incus) => (|| -> Result<Listing, Refusal> {
                Ok(Listing {
                    instances: incus.instances()?,
                    images: incus.images()?,
                    profiles: incus.profiles()?,
                    networks: incus.networks()?,
                    pools: incus.pools()?,
                    projects: incus.projects()?,
                })
            })(),
            Daemon::Scene(scene) => scene_of(scene, |s| {
                Ok(Listing {
                    instances: s.instances.clone(),
                    images: s.images.clone(),
                    profiles: s.profiles.clone(),
                    networks: s.networks.clone(),
                    pools: s.pools.clone(),
                    projects: s.projects.clone(),
                })
            }),
            Daemon::Absent(refusal) => Err(*refusal),
        };
        self.arrive(Arrival::Listed(listed));
    }

    /// The client's remotes, and then the consent of their hosts.
    fn remotes_now(&self) {
        let remotes = match &self.daemon {
            Daemon::Live(incus) => incus.remotes(),
            Daemon::Scene(scene) => scene_of(scene, |s| Ok(s.remotes.clone())),
            Daemon::Absent(refusal) => Err(*refusal),
        };
        if let (Ok(r), Ok(mut kept)) = (&remotes, self.remotes.lock()) {
            kept.clone_from(r);
        }
        self.arrive(Arrival::Remotes(remotes));
        self.consent_now();
    }

    /// Which of the remotes' hosts the person let, read now.
    fn consent_now(&self) {
        let hosts: Vec<String> = self
            .remotes
            .lock()
            .map(|r| {
                r.iter()
                    .flat_map(|x| ronoctua_incus::hosts_of(&x.address, x.protocol))
                    .filter(|h| !h.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let let_ = match &self.daemon {
            Daemon::Scene(scene) => scene
                .lock()
                .map(|s| s.consented.iter().cloned().collect())
                .unwrap_or_default(),
            Daemon::Live(_) | Daemon::Absent(_) => hosts
                .into_iter()
                .filter(|h| ronoctua_consent::allows(h))
                .collect(),
        };
        self.arrive(Arrival::Consented(let_));
    }

    /// A listing on a thread; asked while one runs, one more after it — a
    /// burst of events is two listings, never one per event.
    fn relist(&self) {
        self.again.store(true, Ordering::SeqCst);
        if self.listing.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("incus-list".into())
            .spawn(move || {
                loop {
                    me.again.store(false, Ordering::SeqCst);
                    me.list_now();
                    if me.again.load(Ordering::SeqCst) {
                        continue;
                    }
                    me.listing.store(false, Ordering::SeqCst);
                    // A REQUEST BETWEEN THE LOOK AND THE STORE would find the
                    // flag still up and leave: take it here.
                    if me.again.load(Ordering::SeqCst) && !me.listing.swap(true, Ordering::SeqCst) {
                        continue;
                    }
                    break;
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not start Incus's listing");
            self.listing.store(false, Ordering::SeqCst);
        }
    }

    /// The daemon's events on a thread of their own — THE HANDSHAKE
    /// INCLUDED, which waits on the daemon and so never runs on a door's
    /// loop. Nothing when they are open or opening, once the feed is gone, or
    /// without a live daemon. A change is a listing again; the stream's end
    /// is said and it is opened again later. `again`: what changed while it
    /// was lost is listed once it is open.
    fn follow_events(&self, again: bool) {
        let Daemon::Live(incus) = &self.daemon else {
            return;
        };
        if self.detached {
            return;
        }
        {
            let mut w = self.watch();
            if w.gone || matches!(w.stream, Stream::Opening | Stream::Open { .. }) {
                return;
            }
            w.stream = Stream::Opening;
        }
        let (me, incus) = (self.clone(), incus.clone());
        let spawned = std::thread::Builder::new()
            .name("incus-events".into())
            .spawn(move || {
                let mut events = match incus.events() {
                    Ok(events) => events,
                    Err(refusal) => {
                        tracing::debug!(why = ?refusal.why, "Incus's events would not open");
                        me.events_ended();
                        return;
                    }
                };
                let Some(stopper) = events.stopper() else {
                    me.events_ended();
                    return;
                };
                {
                    let mut w = me.watch();
                    // GONE WHILE IT OPENED: the stream is shut right here,
                    // stopper and all, and nothing is read.
                    if w.gone {
                        w.stream = Stream::Closed;
                        return;
                    }
                    w.stream = Stream::Open { _stopper: stopper };
                    w.said_lost = false;
                }
                me.arrive(Arrival::Events(true));
                if again {
                    me.relist();
                }
                for event in events {
                    me.heard(&event);
                }
                me.events_ended();
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not follow Incus's events");
            self.events_ended();
        }
    }

    /// One event: what it is about, read again.
    fn heard(&self, event: &ronoctua_incus::Event) {
        if let ronoctua_incus::Event::Changed { topic, .. } = event {
            if *topic == ronoctua_incus::Topic::Storage {
                self.volumes_stale.store(true, Ordering::SeqCst);
            }
            if *topic != ronoctua_incus::Topic::Other {
                self.relist();
            }
        }
    }

    /// The stream ended — the daemon closed it (a restart), or it would not
    /// open: said once, and opened again later, unless the feed is gone.
    fn events_ended(&self) {
        let say = {
            let mut w = self.watch();
            if w.gone {
                w.stream = Stream::Closed;
                return;
            }
            w.stream = Stream::Lost;
            !std::mem::replace(&mut w.said_lost, true)
        };
        if say {
            self.arrive(Arrival::Events(false));
        }
        self.arm_clock();
    }

    /// The states the screen needs read again, as the view said them last:
    /// the clock armed when there are some, and woken to end at once when
    /// there are none (a wake never moves its next tick: it is a deadline).
    fn want_states(&self, states: &[String]) {
        {
            let mut w = self.watch();
            if w.gone || w.states == states {
                return;
            }
            w.states = states.to_vec();
        }
        self.watch.1.notify_all();
        self.arm_clock();
    }

    /// Starts the clock's thread, when it has work and is not running — a
    /// live daemon's only: a photograph's scene never moves.
    fn arm_clock(&self) {
        if self.detached || !matches!(self.daemon, Daemon::Live(_)) {
            return;
        }
        {
            let mut w = self.watch();
            if w.ticking || !w.armed() {
                return;
            }
            w.ticking = true;
        }
        let me = self.clone();
        let spawned = std::thread::Builder::new()
            .name("incus-clock".into())
            .spawn(move || me.tick());
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not start Incus's clock");
            self.watch().ticking = false;
        }
    }

    /// The clock: every [`STATE_SECONDS`] while it has work, the states read
    /// again and a lost stream opened again. It ends as soon as it has none,
    /// or the feed is gone — which wakes it at once.
    fn tick(&self) {
        let period = Duration::from_secs(STATE_SECONDS);
        loop {
            let deadline = Instant::now() + period;
            let (states, lost) = {
                let mut w = self.watch();
                loop {
                    if !w.armed() {
                        w.ticking = false;
                        return;
                    }
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    w = self
                        .watch
                        .1
                        .wait_timeout(w, deadline - now)
                        .map_or_else(|e| e.into_inner().0, |(w, _)| w);
                }
                (w.states.clone(), matches!(w.stream, Stream::Lost))
            };
            if lost {
                self.follow_events(true);
            }
            for name in states {
                self.state_now(&name);
            }
        }
    }

    /// One instance's state, read now.
    fn state_now(&self, name: &str) {
        let answer = match &self.daemon {
            Daemon::Live(incus) => incus.state(name),
            Daemon::Scene(scene) => scene_of(scene, |s| {
                s.instances
                    .iter()
                    .find(|i| i.name == name)
                    .and_then(|i| i.state.clone())
                    .ok_or(Refusal::of(Why::Gone))
            }),
            Daemon::Absent(refusal) => Err(*refusal),
        };
        self.arrive(Arrival::State {
            name: name.to_owned(),
            answer,
        });
    }

    /// The feed went: the stream shut (its stopper dropped), nothing opened
    /// again, the clock woken to end.
    fn shut(&self) {
        let stream = {
            let mut w = self.watch();
            w.gone = true;
            w.states.clear();
            std::mem::take(&mut w.stream)
        };
        self.watch.1.notify_all();
        drop(stream);
    }

    /// A remote's catalogue, under the pass of its address (every other host
    /// its client asks is read from `consent.json` by the crate).
    fn catalogue_now(&self, remote: &str) {
        let answer = match &self.daemon {
            Daemon::Live(incus) => {
                // THE DOOR'S PASS IS THE ADDRESS'S; the crate reads the
                // other hosts' from `consent.json` before the CLI starts.
                let address = self
                    .remotes
                    .lock()
                    .ok()
                    .and_then(|r| {
                        r.iter()
                            .find(|x| x.name == remote)
                            .and_then(|x| ronoctua_incus::host_of(&x.address))
                    })
                    .unwrap_or_default();
                match ronoctua_consent::pass(&address) {
                    Ok(pass) => incus.catalogue(&pass, remote),
                    Err(_) => Err(Refusal::of(Why::NotLet)),
                }
            }
            Daemon::Scene(scene) => scene_of(scene, |s| {
                let found = s
                    .remotes
                    .iter()
                    .find(|r| r.name == remote)
                    .ok_or(Refusal::of(Why::Gone))?;
                let hosts = ronoctua_incus::hosts_of(&found.address, found.protocol);
                if !hosts.iter().all(|h| s.consented.contains(h)) {
                    return Err(Refusal::of(Why::NotLet));
                }
                Ok(s.catalogues.get(remote).cloned().unwrap_or_default())
            }),
            Daemon::Absent(refusal) => Err(*refusal),
        };
        self.arrive(Arrival::Catalogue(Catalogue {
            remote: remote.to_owned(),
            answer,
        }));
    }

    fn volumes_now(&self, pool: &str) {
        let answer = match &self.daemon {
            Daemon::Live(incus) => incus.volumes(pool),
            Daemon::Scene(scene) => scene_of(scene, |s| {
                Ok(s.volumes.get(pool).cloned().unwrap_or_default())
            }),
            Daemon::Absent(refusal) => Err(*refusal),
        };
        self.arrive(Arrival::Volumes(Volumes {
            pool: pool.to_owned(),
            answer,
        }));
    }

    /// A deed told to the daemon, and its answer.
    fn done(&self, deed: &Deed, name: String) -> Arrival {
        let result = match &self.daemon {
            Daemon::Live(incus) => match deed {
                Deed::Act { name, action } => incus.act(name, *action),
                Deed::Limit { name, change } => incus.set_limit(name, *change),
                Deed::Snapshot { name, snapshot } => incus.snapshot(name, snapshot),
                Deed::Restore { name, snapshot } => incus.restore(name, snapshot),
                Deed::DeleteSnapshot { name, snapshot } => incus.delete_snapshot(name, snapshot),
                Deed::DeleteImage { fingerprint } => incus.delete_image(fingerprint),
            },
            // A PHOTOGRAPH'S DAEMON says yes and changes nothing — or says
            // the refusal its scene gives every deed.
            Daemon::Scene(scene) => scene_of(scene, |s| {
                s.deeds_refused.map_or(Ok(()), |why| Err(Refusal::of(why)))
            }),
            Daemon::Absent(refusal) => Err(*refusal),
        };
        Arrival::Done {
            deed: deed.clone(),
            name,
            result,
        }
    }
}

/// A scene's answer — or its refusal, when it gives one.
fn scene_of<T>(
    scene: &Arc<Mutex<Scene>>,
    answer: impl FnOnce(&Scene) -> Result<T, Refusal>,
) -> Result<T, Refusal> {
    let s = scene.lock().map_err(|_| Refusal::of(Why::Failed))?;
    match s.refused {
        Some(why) => Err(Refusal::of(why)),
        None => answer(&s),
    }
}

/// The feed: its threads, alive while the door shows the view.
pub struct Feed {
    shared: Shared,
    /// `consent.json`, watched while the feed lives.
    consent_watch: Option<ronoctua_files::notice::Notice>,
    /// The catalogue asked last, and the consent as it was then: a refusal
    /// for want of it is asked again only once the consent changed — never
    /// on every frame. An answer for another remote is dropped.
    catalogue_asked: Option<(String, BTreeSet<String>)>,
    /// The pool whose volumes were asked last; an answer for another pool
    /// is dropped.
    volumes_asked: Option<String>,
    /// What a photograph's door would have queued, in order.
    queued: Vec<ronoctua_transfers::Work>,
}

impl std::fmt::Debug for Feed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feed")
            .field("detached", &self.shared.detached)
            .field("watch", &*self.shared.watch())
            .field("catalogue_asked", &self.catalogue_asked)
            .field("volumes_asked", &self.volumes_asked)
            .finish_non_exhaustive()
    }
}

/// DROPPED, the feed takes its threads with it: the events' socket shut, the
/// clock ended, nothing opened again.
impl Drop for Feed {
    fn drop(&mut self) {
        self.shared.shut();
    }
}

impl Feed {
    /// A feed on threads, over this machine's daemon (`Incus::from_env`),
    /// ringing `wake` when something arrived.
    #[must_use]
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let daemon = match Incus::from_env() {
            Ok(incus) => Daemon::Live(incus),
            Err(refusal) => Daemon::Absent(refusal),
        };
        Self::over(daemon, false, wake)
    }

    fn over(daemon: Daemon, detached: bool, wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            shared: Shared {
                daemon,
                detached,
                arrived: Arc::new(Mutex::new(Vec::new())),
                wake: Arc::new(wake),
                listing: Arc::new(AtomicBool::new(false)),
                again: Arc::new(AtomicBool::new(false)),
                remotes: Arc::new(Mutex::new(Vec::new())),
                watch: Arc::new((Mutex::new(Watch::default()), Condvar::new())),
                volumes_stale: Arc::new(AtomicBool::new(false)),
                deeds: Arc::new(AtomicUsize::new(0)),
            },
            consent_watch: None,
            catalogue_asked: None,
            volumes_asked: None,
            queued: Vec::new(),
        }
    }

    /// A feed that asks everything on the spot — a photograph's, over its
    /// scene, or (without one) this machine's daemon.
    #[must_use]
    pub fn detached(scene: Option<Scene>) -> Self {
        let daemon = match scene {
            Some(s) => Daemon::Scene(Arc::new(Mutex::new(s))),
            None => match Incus::from_env() {
                Ok(incus) => Daemon::Live(incus),
                Err(refusal) => Daemon::Absent(refusal),
            },
        };
        Self::over(daemon, true, || {})
    }

    /// The first listing and the remotes; the daemon's events and the
    /// consent's file for as long as the feed lives. NOTHING HERE WAITS on
    /// the daemon: the listing, the remotes and the events' handshake are
    /// their threads'.
    pub fn start(&mut self) {
        if self.shared.detached {
            self.shared.list_now();
            self.shared.remotes_now();
            // A SCENE'S DAEMON THAT RESTARTED: its stream ended, as a live
            // one's would say.
            let ended = match &self.shared.daemon {
                Daemon::Scene(scene) => scene.lock().is_ok_and(|s| s.events_ended),
                Daemon::Live(_) | Daemon::Absent(_) => false,
            };
            if ended {
                self.shared.arrive(Arrival::Events(false));
            }
            return;
        }
        self.shared.relist();
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new()
            .name("incus-remotes".into())
            .spawn(move || shared.remotes_now());
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not read Incus's remotes");
        }
        self.watch_the_consent();
        self.shared.follow_events(false);
    }

    /// `consent.json` watched: a change is read again and arrives.
    fn watch_the_consent(&mut self) {
        let Some(dir) = ronoctua_store::config_dir() else {
            return;
        };
        let shared = self.shared.clone();
        match ronoctua_files::notice::Notice::on_file(
            &dir,
            ronoctua_consent::FILE,
            ronoctua_files::notice::Pace::AtOnce,
            move || {
                shared.consent_now();
                true
            },
        ) {
            Ok(watch) => self.consent_watch = Some(watch),
            Err(e) => tracing::debug!(error = %e, "the consent is not watched"),
        }
    }

    /// Whether the daemon's events are followed now — the stream open — for
    /// the tests and the photograph's report.
    #[must_use]
    pub fn following_events(&self) -> bool {
        matches!(self.shared.watch().stream, Stream::Open { .. })
    }

    /// The instances whose state the clock reads again now — for the tests
    /// and the photograph's report.
    #[must_use]
    pub fn rereading(&self) -> Vec<String> {
        self.shared.watch().states.clone()
    }

    /// Whether a deed was told and has not answered yet.
    #[must_use]
    pub fn deeds_in_flight(&self) -> bool {
        self.shared.deeds.load(Ordering::SeqCst) > 0
    }

    /// The door closed with a deed out: no events, no clock, no watch any
    /// more — only the deeds' answers still arrive, for the door's line.
    pub fn quiet(&mut self) {
        self.shared.shut();
        self.consent_watch = None;
    }

    /// Asks for the listing again.
    pub fn relist(&self) {
        if self.shared.detached {
            self.shared.list_now();
        } else {
            self.shared.relist();
        }
    }

    /// Everything that arrived since the last call — but a catalogue or
    /// volumes for what is no longer asked: a slow answer for a remote the
    /// person left must not take the place of the one shown.
    #[must_use]
    pub fn take(&self) -> Vec<Arrival> {
        let all = self
            .shared
            .arrived
            .lock()
            .map(|mut a| std::mem::take(&mut *a))
            .unwrap_or_default();
        all.into_iter()
            .filter(|a| match a {
                Arrival::Catalogue(c) => self
                    .catalogue_asked
                    .as_ref()
                    .is_some_and(|(asked, _)| *asked == c.remote),
                Arrival::Volumes(v) => self.volumes_asked.as_ref() == Some(&v.pool),
                _ => true,
            })
            .collect()
    }

    /// Tells the daemon a deed off the loop; the listing follows it. The
    /// deed is counted until it answered ([`Feed::deeds_in_flight`]).
    pub fn deed(&self, deed: Deed, name: String) {
        if self.shared.detached {
            let done = self.shared.done(&deed, name);
            self.shared.arrive(done);
            self.shared.list_now();
            return;
        }
        self.shared.deeds.fetch_add(1, Ordering::SeqCst);
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new()
            .name("incus-deed".into())
            .spawn(move || {
                let done = shared.done(&deed, name);
                // COUNTED OFF BEFORE IT ARRIVES: the door the bell wakes
                // finds it answered.
                shared.deeds.fetch_sub(1, Ordering::SeqCst);
                shared.arrive(done);
                let gone = shared.watch().gone;
                if !gone {
                    shared.relist();
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not start Incus's deed");
            self.shared.deeds.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Puts work on the transfer queue — or, for a photograph's scene, keeps
    /// it for the report: a photograph never queues on this machine.
    ///
    /// # Errors
    ///
    /// When the queue did not take it.
    pub fn queue(&mut self, work: ronoctua_transfers::Work) -> Result<(), ()> {
        if matches!(self.shared.daemon, Daemon::Scene(_)) {
            self.queued.push(work);
            return Ok(());
        }
        ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work })
            .map(|_| ())
            .map_err(|e| {
                tracing::warn!(error = %e, "the transfer queue did not take the job");
            })
    }

    /// What a photograph's door would have queued.
    #[must_use]
    pub fn queued(&self) -> &[ronoctua_transfers::Work] {
        &self.queued
    }

    /// Lets these hosts in `consent.json` — or, for a photograph's scene, in
    /// the scene: a photograph never writes the person's consent. The new
    /// consent arrives (and the watch says so too, on a live machine).
    ///
    /// # Errors
    ///
    /// The sentence that says why it could not be kept.
    pub fn allow(&self, hosts: &[String], texts: &ronoctua_i18n::Texts) -> Result<(), String> {
        if let Daemon::Scene(scene) = &self.shared.daemon {
            if let Ok(mut s) = scene.lock() {
                for h in hosts {
                    if !s.consented.contains(h) {
                        s.consented.push(h.clone());
                    }
                }
            }
            self.shared.consent_now();
            return Ok(());
        }
        let named: Vec<&str> = hosts.iter().map(String::as_str).collect();
        crate::consent_rows::allow(&named, texts)?;
        if self.shared.detached {
            self.shared.consent_now();
        } else {
            let shared = self.shared.clone();
            let _ = std::thread::Builder::new()
                .name("incus-consent".into())
                .spawn(move || shared.consent_now());
        }
        Ok(())
    }

    /// The program a door starts for a console, the screen or a shell of
    /// `name`, and its words ([`crate::incus::launched`]): `me` is this
    /// binary.
    ///
    /// # Errors
    ///
    /// The daemon's refusal: no CLI, a name Incus does not take.
    pub fn opening(
        &self,
        what: Use,
        name: &str,
        me: &str,
    ) -> Result<(String, Vec<String>), Refusal> {
        let console = |kind| match &self.shared.daemon {
            Daemon::Live(incus) => incus.console_command(name, kind),
            Daemon::Scene(scene) => scene_of(scene, |s| {
                let cli = s.cli.clone().ok_or(Refusal::of(Why::NoIncus))?;
                let mut line = vec![cli];
                line.extend(ronoctua_incus::console_arguments(name, kind));
                Ok(line)
            }),
            Daemon::Absent(refusal) => Err(*refusal),
        };
        let line = match what {
            Use::Console => console(ronoctua_incus::Console::Text)?,
            Use::Screen => console(ronoctua_incus::Console::Vga)?,
            Use::Shell => match &self.shared.daemon {
                Daemon::Live(incus) => incus.shell_command(name)?,
                Daemon::Scene(scene) => scene_of(scene, |s| {
                    let cli = s.cli.clone().ok_or(Refusal::of(Why::NoIncus))?;
                    let mut line = vec![cli];
                    line.extend(ronoctua_incus::shell_arguments(name));
                    Ok(line)
                })?,
                Daemon::Absent(refusal) => return Err(*refusal),
            },
            Use::Push | Use::Pull | Use::Export | Use::Publish => {
                return Err(Refusal::of(Why::Invalid));
            }
        };
        what.launch()
            .and_then(|how| crate::incus::launched(how, me, &line))
            .ok_or(Refusal::of(Why::Failed))
    }

    /// Makes what is read what the view wants: a catalogue asked when its
    /// remote's detail shows (again when its hosts were let since its
    /// refusal, and again when it is opened again after any other refusal),
    /// a pool's volumes when its detail shows (again when storage changed),
    /// and the clock armed for the states the screen needs read again. What
    /// is asked is put to "reading" in `r` first.
    pub fn follow(&mut self, wants: &Wants, r: &mut Readings) {
        match &wants.catalogue {
            Some(remote) => {
                let refused = matches!(
                    r.catalogue
                        .as_ref()
                        .filter(|c| c.remote == *remote)
                        .map(|c| &c.answer),
                    Some(Err(Refusal { why: Why::NotLet }))
                );
                let again = match &self.catalogue_asked {
                    Some((asked, consent)) if asked == remote => refused && *consent != r.consented,
                    _ => true,
                };
                if again {
                    self.catalogue_asked = Some((remote.clone(), r.consented.clone()));
                    r.catalogue = None;
                    self.ask(
                        |shared, remote| shared.catalogue_now(&remote),
                        remote.clone(),
                    );
                }
            }
            // NOT SHOWN: a catalogue that answered is kept and not fetched
            // again; one that was refused (offline, the CLI failed) or never
            // came is forgotten, and asked again when its remote is opened
            // again.
            None => {
                let answered = self.catalogue_asked.as_ref().is_some_and(|(asked, _)| {
                    r.catalogue
                        .as_ref()
                        .is_some_and(|c| c.remote == *asked && c.answer.is_ok())
                });
                if !answered {
                    self.catalogue_asked = None;
                }
            }
        }
        match &wants.volumes {
            Some(pool) => {
                let stale = self.shared.volumes_stale.swap(false, Ordering::SeqCst);
                if stale || self.volumes_asked.as_ref() != Some(pool) {
                    if r.volumes.as_ref().is_none_or(|v| v.pool != *pool) {
                        r.volumes = None;
                    }
                    self.volumes_asked = Some(pool.clone());
                    self.ask(|shared, pool| shared.volumes_now(&pool), pool.clone());
                }
            }
            None => self.volumes_asked = None,
        }
        self.shared.want_states(&wants.states);
    }

    /// One question of the daemon's, on the spot or on a thread.
    fn ask(&self, what: impl FnOnce(&Shared, String) + Send + 'static, about: String) {
        if self.shared.detached {
            what(&self.shared, about);
            return;
        }
        let shared = self.shared.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("incus-ask".into())
            .spawn(move || what(&shared, about))
        {
            tracing::warn!(error = %e, "could not ask Incus");
        }
    }
}

/// The queue's jobs that are Incus's — what both doors draw above a list.
#[must_use]
pub fn incus_jobs(jobs: &[ronoctua_transfers::Job]) -> Vec<ronoctua_transfers::Job> {
    jobs.iter()
        .filter(|j| matches!(j.work, ronoctua_transfers::Work::Incus { .. }))
        .cloned()
        .collect()
}

/// Whether fewer of `now` run than of `before` — a job ended, and what
/// there is changed: a listing again, in both doors. Not a clock: the
/// queue's list arrives when it changes.
#[must_use]
pub fn fewer_running(now: &[ronoctua_transfers::Job], before: &[ronoctua_transfers::Job]) -> bool {
    let running = |js: &[ronoctua_transfers::Job]| {
        js.iter()
            .filter(|j| {
                matches!(
                    j.stage,
                    ronoctua_transfers::Stage::Running | ronoctua_transfers::Stage::Weighing
                )
            })
            .count()
    };
    running(now) < running(before)
}

impl Readings {
    /// Takes one arrival. Answers what the message line should say, for a
    /// deed's end: done, or refused with its reason.
    pub fn took(&mut self, a: Arrival, texts: &ronoctua_i18n::Texts) -> Option<String> {
        match a {
            Arrival::Listed(l) => self.listing = Some(l),
            Arrival::Remotes(r) => self.remotes = Some(r),
            Arrival::Consented(c) => self.consented = c,
            Arrival::Catalogue(c) => self.catalogue = Some(c),
            Arrival::Volumes(v) => self.volumes = Some(v),
            // A STATE READ AGAIN goes into its instance while it still runs:
            // a stop listed meanwhile wins over a state read before it.
            Arrival::State { name, answer } => {
                if let (Ok(state), Some(Ok(l))) = (answer, self.listing.as_mut())
                    && let Some(i) = l
                        .instances
                        .iter_mut()
                        .find(|i| i.name == name && i.status == Status::Running)
                {
                    i.state = Some(state);
                }
            }
            Arrival::Events(followed) => {
                self.events = if followed {
                    Following::Yes
                } else {
                    Following::Lost
                };
            }
            Arrival::Done { deed, name, result } => {
                let what = texts.t(deed.key());
                return Some(match result {
                    Ok(()) => texts.t_with(
                        ronoctua_i18n::key::INCUS_ECHO_DONE,
                        &[("what", &what), ("name", &name)],
                    ),
                    Err(refusal) => texts.t_with(
                        ronoctua_i18n::key::INCUS_ECHO_REFUSED,
                        &[
                            ("what", &what),
                            ("name", &name),
                            ("why", &crate::incus_words::refused(refusal, texts)),
                        ],
                    ),
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn scene() -> Scene {
        let r = crate::incus::tests::readings();
        let l = r.listed().cloned().unwrap();
        Scene {
            instances: l.instances,
            images: l.images,
            profiles: l.profiles,
            networks: l.networks,
            pools: l.pools,
            projects: l.projects,
            remotes: r.remotes_read().to_vec(),
            cli: Some("/nix/store/x-incus/bin/incus".to_owned()),
            ..Scene::default()
        }
    }

    /// An Incus whose socket is not there: every call answers `no-incus`
    /// at once.
    fn nowhere(tag: &str) -> Incus {
        Incus {
            socket: std::env::temp_dir()
                .join(format!("ronoctua-incus-feed-{tag}-{}", std::process::id())),
            cli: None,
        }
    }

    /// A bell a test waits on: each ring is a message.
    fn bell() -> (
        impl Fn() + Send + Sync + 'static,
        std::sync::mpsc::Receiver<()>,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Mutex::new(tx);
        (
            move || {
                if let Ok(t) = tx.lock() {
                    let _ = t.send(());
                }
            },
            rx,
        )
    }

    /// The arrivals taken until one is `wanted`, or the time is out.
    fn until(
        feed: &Feed,
        rx: &std::sync::mpsc::Receiver<()>,
        wanted: impl Fn(&Arrival) -> bool,
        within: Duration,
    ) -> Vec<Arrival> {
        let end = Instant::now() + within;
        let mut seen = Vec::new();
        loop {
            seen.extend(feed.take());
            let left = end.saturating_duration_since(Instant::now());
            if seen.iter().any(&wanted) || left.is_zero() {
                return seen;
            }
            let _ = rx.recv_timeout(left);
        }
    }

    /// Whether `holds` came true within `within`.
    fn eventually(holds: impl Fn() -> bool, within: Duration) -> bool {
        let end = Instant::now() + within;
        while !holds() {
            if Instant::now() >= end {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }

    fn offered(alias: &str) -> Offered {
        Offered {
            alias: alias.to_owned(),
            description: String::new(),
            kind: ronoctua_incus::Kind::Container,
            fingerprint: "f00d".to_owned(),
            size_bytes: 3 << 20,
        }
    }

    /// FAILS IF: the events' handshake runs on the caller's thread — a
    /// door's loop. A daemon still starting takes the connection and answers
    /// late (here: never); `start` comes back at once all the same.
    #[test]
    fn the_events_handshake_never_waits_on_the_door_s_thread() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-incus-handshake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("unix.socket");
        // A DAEMON THAT TAKES THE CALL AND SAYS NOTHING: the connection
        // waits in its backlog.
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let (wake, _rx) = bell();
        let mut feed = Feed::over(Daemon::Live(Incus { socket, cli: None }), false, wake);
        let began = Instant::now();
        feed.start();
        assert!(
            began.elapsed() < Duration::from_secs(2),
            "`start` waited {:?} on the daemon",
            began.elapsed()
        );
        assert!(!feed.following_events(), "still opening, on its thread");
        drop(feed);
        drop(listener);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a stream of events that ended — or would not open — is not
    /// said, the feed does not try it again on its own, or a feed that went
    /// keeps its clock.
    #[test]
    fn events_that_end_are_said_and_tried_again_until_the_feed_goes() {
        let (wake, rx) = bell();
        let mut feed = Feed::over(Daemon::Live(nowhere("lost")), false, wake);
        feed.start();
        let seen = until(
            &feed,
            &rx,
            |a| matches!(a, Arrival::Events(false)),
            Duration::from_secs(10),
        );
        let (t, mut r) = (texts(), Readings::default());
        for a in seen {
            r.took(a, &t);
        }
        assert_eq!(r.events, Following::Lost);
        assert!(!feed.following_events());
        assert!(
            eventually(|| feed.shared.watch().ticking, Duration::from_secs(5)),
            "nothing will open them again"
        );
        let watch = Arc::clone(&feed.shared.watch);
        drop(feed);
        let ticking = || watch.0.lock().map(|w| w.ticking).unwrap_or(true);
        assert!(
            eventually(|| !ticking(), Duration::from_secs(5)),
            "the clock outlived its feed"
        );
    }

    /// FAILS IF: what no event reports (an address that comes with its
    /// lease, the figures that move) is never read again while it is shown
    /// — or the clock is still armed once nothing on screen needs it.
    #[test]
    fn the_clock_reads_a_state_again_only_while_one_is_wanted() {
        let (wake, rx) = bell();
        let mut feed = Feed::over(Daemon::Live(nowhere("clock")), false, wake);
        let mut r = Readings::default();
        let wants = Wants {
            states: vec!["web".to_owned()],
            ..Wants::default()
        };
        feed.follow(&wants, &mut r);
        assert_eq!(feed.rereading(), ["web".to_owned()]);
        let seen = until(
            &feed,
            &rx,
            |a| matches!(a, Arrival::State { name, .. } if name == "web"),
            Duration::from_secs(STATE_SECONDS * 3),
        );
        assert!(
            seen.iter()
                .any(|a| matches!(a, Arrival::State { name, .. } if name == "web")),
            "{seen:?}"
        );
        feed.follow(&Wants::default(), &mut r);
        assert!(feed.rereading().is_empty());
        assert!(
            eventually(|| !feed.shared.watch().ticking, Duration::from_secs(1)),
            "the clock is still armed"
        );
    }

    /// FAILS IF: a state read again does not reach its instance, reaches one
    /// that stopped meanwhile, or a refusal wipes what is there.
    #[test]
    fn a_state_read_again_reaches_its_running_instance_only() {
        let t = texts();
        let mut r = crate::incus::tests::readings();
        let state = State {
            ipv4: vec!["10.218.42.99".to_owned()],
            ..State::default()
        };
        for name in ["web", "desk"] {
            r.took(
                Arrival::State {
                    name: name.to_owned(),
                    answer: Ok(state.clone()),
                },
                &t,
            );
        }
        assert_eq!(r.instance("web").and_then(|i| i.state.clone()), Some(state));
        assert_eq!(
            r.instance("desk").and_then(|i| i.state.clone()),
            None,
            "desk is stopped"
        );
        r.took(
            Arrival::State {
                name: "web".to_owned(),
                answer: Err(Refusal::of(Why::Gone)),
            },
            &t,
        );
        assert!(r.instance("web").is_some_and(|i| i.state.is_some()));
    }

    /// FAILS IF: a slow catalogue for a remote the person left takes the
    /// place of the one shown, or a catalogue refused for another reason
    /// than consent is never asked again — or an answered one is fetched
    /// again every time its detail opens.
    #[test]
    fn a_late_catalogue_is_dropped_and_a_refused_one_asked_again() {
        let mut s = scene();
        s.consented = [
            "images.linuxcontainers.org",
            "docker.io",
            "registry-1.docker.io",
            "auth.docker.io",
        ]
        .map(str::to_owned)
        .to_vec();
        s.catalogues
            .insert("images".to_owned(), vec![offered("alpine/3.22")]);
        s.catalogues
            .insert("docker".to_owned(), vec![offered("nginx")]);
        let mut feed = Feed::detached(Some(s));
        let mut r = Readings::default();
        feed.start();
        taken(&feed, &mut r);
        let wants = |remote: &str| Wants {
            catalogue: Some(remote.to_owned()),
            ..Wants::default()
        };
        feed.follow(&wants("images"), &mut r);
        taken(&feed, &mut r);
        feed.follow(&wants("docker"), &mut r);
        taken(&feed, &mut r);
        // THE IMAGES' ANSWER, LATE: it comes after the person moved on.
        feed.shared.arrive(Arrival::Catalogue(Catalogue {
            remote: "images".to_owned(),
            answer: Ok(vec![offered("late")]),
        }));
        taken(&feed, &mut r);
        assert_eq!(
            r.catalogue.as_ref().map(|c| c.remote.as_str()),
            Some("docker"),
            "the late answer took the shown one's place"
        );
        // A REFUSAL THAT IS NOT CONSENT'S (the CLI failed offline)…
        r.catalogue = Some(Catalogue {
            remote: "docker".to_owned(),
            answer: Err(Refusal::of(Why::Failed)),
        });
        feed.follow(&wants("docker"), &mut r);
        assert!(feed.take().is_empty(), "not asked on every frame");
        // …is asked again when the remote is opened again.
        feed.follow(&Wants::default(), &mut r);
        feed.follow(&wants("docker"), &mut r);
        taken(&feed, &mut r);
        assert_eq!(
            r.catalogue
                .as_ref()
                .map(|c| c.answer.as_ref().map(Vec::len)),
            Some(Ok(1))
        );
        // AN ANSWERED ONE is kept, and not fetched again.
        feed.follow(&Wants::default(), &mut r);
        feed.follow(&wants("docker"), &mut r);
        assert!(feed.take().is_empty());
    }

    /// FAILS IF: a deed told before its door closed loses its answer — the
    /// feed quieted for it still brings it, and counts it off.
    #[test]
    fn a_quieted_feed_still_brings_its_deed_s_answer() {
        let (wake, rx) = bell();
        let mut feed = Feed::over(Daemon::Live(nowhere("deed")), false, wake);
        feed.deed(
            Deed::Act {
                name: "web".to_owned(),
                action: ronoctua_incus::Action::Stop,
            },
            "web".to_owned(),
        );
        feed.quiet();
        let seen = until(
            &feed,
            &rx,
            |a| matches!(a, Arrival::Done { .. }),
            Duration::from_secs(10),
        );
        assert!(
            seen.iter().any(|a| matches!(
                a,
                Arrival::Done {
                    result: Err(Refusal { why: Why::NoIncus }),
                    ..
                }
            )),
            "{seen:?}"
        );
        assert!(!feed.deeds_in_flight());
        assert!(!feed.following_events());
    }

    /// A SCENE'S DAEMON THAT RESTARTED says so, as a live one's would.
    #[test]
    fn a_scene_s_ended_events_are_lost_events() {
        let t = texts();
        let mut feed = Feed::detached(Some(Scene {
            events_ended: true,
            ..scene()
        }));
        let mut r = Readings::default();
        feed.start();
        for a in feed.take() {
            r.took(a, &t);
        }
        assert_eq!(r.events, Following::Lost);
    }

    fn taken(feed: &Feed, r: &mut Readings) -> Vec<String> {
        let t = texts();
        feed.take()
            .into_iter()
            .filter_map(|a| r.took(a, &t))
            .collect()
    }

    /// FAILS IF: without Incus the view is left with nothing to say — every
    /// answer must be the refusal, a sentence.
    #[test]
    fn without_incus_everything_is_a_refusal_with_its_reason() {
        let mut feed = Feed::over(Daemon::Absent(Refusal::of(Why::NoIncus)), true, || {});
        let mut r = Readings::default();
        feed.start();
        taken(&feed, &mut r);
        assert_eq!(r.listing, Some(Err(Refusal::of(Why::NoIncus))));
        assert_eq!(r.remotes, Some(Err(Refusal::of(Why::NoIncus))));
        feed.deed(
            Deed::Act {
                name: "web".to_owned(),
                action: ronoctua_incus::Action::Start,
            },
            "web".to_owned(),
        );
        let said = taken(&feed, &mut r);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("web"), "{said:?}");
    }

    /// FAILS IF: a photograph's feed reaches the person's consent or the
    /// machine's queue, or a catalogue is answered before its hosts are let.
    #[test]
    fn a_scene_answers_and_keeps_what_would_have_left() {
        let t = texts();
        let mut s = scene();
        s.catalogues.insert(
            "images".to_owned(),
            vec![Offered {
                alias: "alpine/3.22".to_owned(),
                description: "Alpine".to_owned(),
                kind: ronoctua_incus::Kind::Container,
                fingerprint: "f00d".to_owned(),
                size_bytes: 3 << 20,
            }],
        );
        let mut feed = Feed::detached(Some(s));
        let mut r = Readings::default();
        feed.start();
        taken(&feed, &mut r);
        assert_eq!(r.count(crate::incus::Kind::Instances), Some(2));
        assert!(r.consented.is_empty());
        let wants = Wants {
            catalogue: Some("images".to_owned()),
            ..Wants::default()
        };
        feed.follow(&wants, &mut r);
        taken(&feed, &mut r);
        assert_eq!(
            r.catalogue.as_ref().map(|c| c.answer.clone()),
            Some(Err(Refusal::of(Why::NotLet)))
        );
        feed.allow(&["images.linuxcontainers.org".to_owned()], &t)
            .unwrap();
        taken(&feed, &mut r);
        assert!(r.consented.contains("images.linuxcontainers.org"));
        feed.follow(&wants, &mut r);
        taken(&feed, &mut r);
        assert_eq!(
            r.catalogue
                .as_ref()
                .map(|c| c.answer.as_ref().map(Vec::len)),
            Some(Ok(1)),
            "asked again once its hosts were let"
        );
        let work = ronoctua_transfers::Work::Incus {
            job: Box::new(ronoctua_incus::Job::Import {
                from: "/x.tar".into(),
            }),
            label: "x".to_owned(),
        };
        assert_eq!(feed.queue(work.clone()), Ok(()));
        assert_eq!(feed.queued(), [work]);
        let (program, words) = feed.opening(Use::Shell, "web", "/me").unwrap();
        assert_eq!(program, "/me");
        assert_eq!(
            words[..2],
            [
                "terminal".to_owned(),
                "/nix/store/x-incus/bin/incus".to_owned()
            ]
        );
        assert!(!feed.following_events(), "a scene has no events to follow");
    }

    /// FAILS IF: the footer's camera, handed a scene that does not read,
    /// draws an empty scene that passes for one — or takes no scene for the
    /// empty one it is.
    #[test]
    fn a_footer_camera_s_scene_that_does_not_read_is_a_refusal() {
        let dir = std::env::temp_dir().join(format!("ronoctua-incus-scene-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.json");
        std::fs::write(&good, r#"{"cli": "/nix/store/x-incus/bin/incus"}"#).unwrap();
        let bad = dir.join("bad.json");
        std::fs::write(&bad, r#"{"instance": []}"#).unwrap();
        assert_eq!(Scene::of_the_footer_camera(None), Scene::default());
        assert_eq!(
            Scene::of_the_footer_camera(Some(&good)).cli.as_deref(),
            Some("/nix/store/x-incus/bin/incus")
        );
        for unread in [bad, dir.join("absent.json")] {
            assert_eq!(
                Scene::of_the_footer_camera(Some(&unread)).refused,
                Some(Why::Invalid),
                "{}",
                unread.display()
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_pool_s_volumes_are_asked_when_shown_and_again_after_a_change() {
        let mut s = scene();
        s.volumes.insert(
            "default".to_owned(),
            vec![Volume {
                name: "data".to_owned(),
                kind: "custom".to_owned(),
            }],
        );
        let mut feed = Feed::detached(Some(s));
        let mut r = Readings::default();
        let wants = Wants {
            volumes: Some("default".to_owned()),
            ..Wants::default()
        };
        feed.follow(&wants, &mut r);
        taken(&feed, &mut r);
        assert_eq!(
            r.volumes.as_ref().map(|v| v.answer.as_ref().map(Vec::len)),
            Some(Ok(1))
        );
        // ASKED ONCE while shown…
        feed.follow(&wants, &mut r);
        assert!(feed.take().is_empty());
        // …AND AGAIN once storage changed.
        feed.shared.volumes_stale.store(true, Ordering::SeqCst);
        feed.follow(&wants, &mut r);
        assert_eq!(feed.take().len(), 1);
    }
}
