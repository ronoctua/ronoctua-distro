//! The containers' view's threads: what podman says, asked off the loop.
//!
//! # Event, not clock
//!
//! Nothing here polls. The listing is asked when the view opens, after an
//! action of the view's own, and when podman says something happened —
//! `podman events` is a child that lives exactly as long as the feed, which
//! lives exactly as long as a door shows the view. An `inspect` is asked when
//! the row shown changes, or when an event made the last one old. A log and a
//! round of stats are children that live exactly while their tab is shown
//! ([`crate::containers::View::wants`], [`Feed::follow`]): dropping the held
//! half kills the child (`ronoctua_containers::Held`).
//!
//! # One ping for everything
//!
//! Every thread pushes what it has onto one list and rings the door's bell;
//! the door takes the whole list at once ([`Feed::take`]) and hands each
//! arrival to [`Readings::took`]. A burst of log lines is one frame.
//!
//! # The firewall, watched while the view is open
//!
//! The Ports tab judges each port by the firewall's two files, and the
//! security centre — where the tab's row leads — rewrites them while this
//! view stays open. So they are WATCHED (`ronoctua_files::notice`, one
//! inotify watch on each file's folder, narrowed to its name) for as long as
//! the feed lives, and a change is an [`Arrival::Firewall`]; read once with a
//! listing, a rule just added would have gone on reading "refused".
//!
//! # Detached, for a photograph
//!
//! [`Feed::detached`] asks everything on the spot, on the caller's thread,
//! and reads a stream to its end — a photograph's stand-in podman prints and
//! leaves — so the frame drawn is the frame after the answer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ronoctua_consent::keys::Secret;
use ronoctua_containers::{Action, Held, Listing, Podman, Reader, Refusal, Usage, Why};

use crate::containers::{Firewall, Inspected, LOG_TAIL, Log, Readings, STATS_SECONDS, Wants};

/// Something a thread brought.
#[derive(Debug, Clone)]
pub enum Arrival {
    /// A listing, or why not.
    Listed(Result<Listing, Refusal>),
    /// What the firewall's files say.
    Firewall(Firewall),
    /// An inspect, answered.
    Inspected(Inspected),
    /// A line of the log followed.
    LogLine {
        /// Whose.
        id: String,
        /// What it wrote — data.
        line: String,
    },
    /// The log followed ended.
    LogEnded {
        /// Whose.
        id: String,
    },
    /// The log could not be followed.
    LogRefused {
        /// Whose.
        id: String,
        /// Why.
        refusal: Refusal,
    },
    /// A round of stats, or why not.
    Stats(Result<Vec<Usage>, Refusal>),
    /// An action of the view's, done or refused.
    Acted {
        /// What was asked.
        action: Action,
        /// Its thing's name, for the sentence.
        name: String,
        /// How it went.
        result: Result<(), Refusal>,
    },
    /// A registry's login, done or refused.
    LoggedIn {
        /// The registry and the user.
        login: crate::containers::Login,
        /// How it went.
        result: Result<(), Refusal>,
    },
}

/// What a thread shares with the door.
#[derive(Clone)]
struct Shared {
    podman: Option<Podman>,
    arrived: Arc<Mutex<Vec<Arrival>>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    /// A listing thread is running.
    listing: Arc<AtomicBool>,
    /// And somebody asked for another since it began.
    again: Arc<AtomicBool>,
    /// Something happened since the last inspect: ask it again.
    stale: Arc<AtomicBool>,
    /// The firewall a photograph gives, instead of this machine's.
    firewall: Option<Firewall>,
}

impl Shared {
    fn arrive(&self, a: Arrival) {
        if let Ok(mut list) = self.arrived.lock() {
            list.push(a);
        }
        (self.wake)();
    }

    /// One listing and the firewall, as one step.
    fn list_now(&self) {
        let listed = match &self.podman {
            Some(p) => p.list(),
            None => Err(Refusal { why: Why::NoPodman }),
        };
        self.arrive(Arrival::Listed(listed));
        self.arrive(Arrival::Firewall(
            self.firewall.clone().unwrap_or_else(read_firewall),
        ));
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
            .name("containers-list".into())
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
            tracing::warn!(error = %e, "could not start the containers' listing");
            self.listing.store(false, Ordering::SeqCst);
        }
    }

    fn inspect_now(&self, key: &str) {
        let answer = match &self.podman {
            Some(p) => p.inspect(key),
            None => Err(Refusal { why: Why::NoPodman }),
        };
        let details = answer
            .as_ref()
            .ok()
            .and_then(|a| ronoctua_containers::details_of(a));
        self.arrive(Arrival::Inspected(Inspected {
            key: key.to_owned(),
            answer,
            details,
        }));
    }

    /// `podman login`, the password on its stdin (`Podman::login`).
    fn login_now(&self, login: crate::containers::Login, password: &Secret) {
        let result = match &self.podman {
            Some(p) => p.login(&login.registry, &login.user, password),
            None => Err(Refusal { why: Why::NoPodman }),
        };
        self.arrive(Arrival::LoggedIn { login, result });
    }

    fn act_now(&self, action: &Action, name: String) {
        let result = match &self.podman {
            Some(p) => p.act(action),
            None => Err(Refusal { why: Why::NoPodman }),
        };
        self.arrive(Arrival::Acted {
            action: action.clone(),
            name,
            result,
        });
    }
}

/// The firewall's two files, as the person reads them.
fn read_firewall() -> Firewall {
    let (settings, unreadable) = match ronoctua_security::firewall::read_settings() {
        Ok(s) => (s, false),
        Err(_) => (None, true),
    };
    Firewall {
        settings,
        status: ronoctua_security::firewall::read_status(),
        unreadable,
    }
}

/// Reads a log to its end, line by line.
fn pump_log(shared: &Shared, id: &str, mut reader: Reader) {
    while let Some(line) = reader.next_line() {
        shared.arrive(Arrival::LogLine {
            id: id.to_owned(),
            line,
        });
    }
    shared.arrive(Arrival::LogEnded { id: id.to_owned() });
}

/// Reads rounds of stats to the stream's end.
fn pump_stats(shared: &Shared, mut reader: Reader, once: bool) {
    while let Some(doc) = reader.next_document() {
        if let Some(round) = ronoctua_containers::parse_stats(&doc) {
            shared.arrive(Arrival::Stats(Ok(round)));
            if once {
                return;
            }
        }
    }
}

/// The feed: its threads and children, alive while the door shows the view.
pub struct Feed {
    shared: Shared,
    /// Everything asked on the spot — a photograph's.
    detached: bool,
    /// `podman events`, while the feed lives.
    events: Option<Held>,
    /// The log followed, and whose.
    logs: Option<(String, Held)>,
    /// The stats followed.
    stats: Option<Held>,
    /// The firewall's two files, watched while the feed lives.
    firewall_watch: Vec<ronoctua_files::notice::Notice>,
    /// What the last inspect was asked of.
    inspected: Option<String>,
}

impl std::fmt::Debug for Feed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feed")
            .field("podman", &self.shared.podman)
            .field("detached", &self.detached)
            .field("logs", &self.logs.as_ref().map(|(id, _)| id))
            .field("stats", &self.stats.is_some())
            .field("inspected", &self.inspected)
            .finish_non_exhaustive()
    }
}

impl Feed {
    /// A feed on threads, ringing `wake` when something arrived. `podman`
    /// is `Podman::from_env()` in production.
    #[must_use]
    pub fn new(podman: Option<Podman>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            shared: Shared {
                podman,
                arrived: Arc::new(Mutex::new(Vec::new())),
                wake: Arc::new(wake),
                listing: Arc::new(AtomicBool::new(false)),
                again: Arc::new(AtomicBool::new(false)),
                stale: Arc::new(AtomicBool::new(false)),
                firewall: None,
            },
            detached: false,
            events: None,
            logs: None,
            stats: None,
            firewall_watch: Vec::new(),
            inspected: None,
        }
    }

    /// A feed that asks everything on the spot, for a photograph, over the
    /// firewall it is given.
    #[must_use]
    pub fn detached(podman: Option<Podman>, firewall: Firewall) -> Self {
        let mut me = Self::new(podman, || {});
        me.shared.firewall = Some(firewall);
        me.detached = true;
        me
    }

    /// The first listing, podman's events and the firewall's files for as
    /// long as the feed lives.
    pub fn start(&mut self) {
        self.relist();
        if self.detached || self.events.is_some() {
            return;
        }
        self.watch_the_firewall();
        let Some(podman) = self.shared.podman.clone() else {
            return;
        };
        match podman.events() {
            Ok(lines) => {
                let (mut reader, held) = lines.split();
                let shared = self.shared.clone();
                let spawned = std::thread::Builder::new()
                    .name("containers-events".into())
                    .spawn(move || {
                        while let Some(line) = reader.next_line() {
                            if ronoctua_containers::parse_event(&line).is_some() {
                                shared.stale.store(true, Ordering::SeqCst);
                                shared.relist();
                            }
                        }
                    });
                match spawned {
                    Ok(_) => self.events = Some(held),
                    Err(e) => tracing::warn!(error = %e, "could not follow podman's events"),
                }
            }
            Err(r) => tracing::warn!(why = ?r.why, "podman's events could not be followed"),
        }
    }

    /// A watch on each of the firewall's files: a change is read and
    /// arrives. A folder that is not there (no security centre on this
    /// machine) is no watch, and the files are read with each listing still.
    fn watch_the_firewall(&mut self) {
        use ronoctua_files::notice::{Notice, Pace};
        for file in [
            ronoctua_security::firewall::FILE,
            ronoctua_security::firewall::STATUS,
        ] {
            let path = std::path::Path::new(file);
            let (Some(dir), Some(name)) =
                (path.parent(), path.file_name().and_then(|n| n.to_str()))
            else {
                continue;
            };
            let shared = self.shared.clone();
            match Notice::on_file(dir, name, Pace::Coalesced, move || {
                shared.arrive(Arrival::Firewall(read_firewall()));
                true
            }) {
                Ok(watch) => self.firewall_watch.push(watch),
                Err(e) => tracing::debug!(error = %e, file, "the firewall's file is not watched"),
            }
        }
    }

    /// Asks for the listing again.
    pub fn relist(&self) {
        if self.detached {
            self.shared.list_now();
        } else {
            self.shared.relist();
        }
    }

    /// Everything that arrived since the last call.
    #[must_use]
    pub fn take(&self) -> Vec<Arrival> {
        self.shared
            .arrived
            .lock()
            .map(|mut a| std::mem::take(&mut *a))
            .unwrap_or_default()
    }

    /// Does an action off the loop; the listing follows it.
    pub fn act(&self, action: Action, name: String) {
        if self.detached {
            self.shared.act_now(&action, name);
            self.shared.list_now();
            return;
        }
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new()
            .name("containers-act".into())
            .spawn(move || {
                shared.act_now(&action, name);
                shared.stale.store(true, Ordering::SeqCst);
                shared.relist();
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not start the containers' action");
        }
    }

    /// Logs in to a registry off the loop — the password goes to podman's
    /// stdin from this thread and is zeroed with it, never an argument and
    /// never logged.
    pub fn login(&self, login: crate::containers::Login, password: Secret) {
        if self.detached {
            self.shared.login_now(login, &password);
            return;
        }
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new()
            .name("containers-login".into())
            .spawn(move || shared.login_now(login, &password));
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not start the registry's login");
        }
    }

    /// What a terminal runs for a shell inside `id`.
    #[must_use]
    pub fn exec_line(&self, id: &str) -> Option<(String, Vec<String>)> {
        self.shared.podman.as_ref().map(|p| p.exec_line(id))
    }

    /// Makes the streams what the view wants: a log started or stopped, the
    /// stats started or stopped, an inspect asked — and the readings made
    /// ready for what starts (an empty log for a new one, no stats once they
    /// stop).
    pub fn follow(&mut self, wants: &Wants, r: &mut Readings) {
        // THE INSPECT: asked when the row changed, or something happened.
        let stale = self.shared.stale.swap(false, Ordering::SeqCst);
        match &wants.inspect {
            Some(key) if stale || self.inspected.as_ref() != Some(key) => {
                self.inspected = Some(key.clone());
                if self.detached {
                    self.shared.inspect_now(key);
                } else {
                    let (shared, key) = (self.shared.clone(), key.clone());
                    let spawned = std::thread::Builder::new()
                        .name("containers-inspect".into())
                        .spawn(move || shared.inspect_now(&key));
                    if let Err(e) = spawned {
                        tracing::warn!(error = %e, "could not start the containers' inspect");
                    }
                }
            }
            Some(_) => {}
            None => self.inspected = None,
        }
        // THE LOG: a child only while its tab is shown.
        let following = self.logs.as_ref().map(|(id, _)| id.clone());
        if following != wants.logs {
            // DROPPED FIRST: the old child dies before the new one starts.
            self.logs = None;
            r.log = wants.logs.as_ref().map(|id| Log {
                id: id.clone(),
                ..Log::default()
            });
            if let Some(id) = &wants.logs {
                self.start_log(id);
            }
        }
        // THE STATS, likewise.
        if wants.stats != self.stats.is_some() {
            self.stats = None;
            r.stats = None;
            if wants.stats {
                self.start_stats();
            }
        }
    }

    fn start_log(&mut self, id: &str) {
        let Some(podman) = self.shared.podman.clone() else {
            self.shared.arrive(Arrival::LogRefused {
                id: id.to_owned(),
                refusal: Refusal { why: Why::NoPodman },
            });
            return;
        };
        match podman.logs(id, LOG_TAIL) {
            Ok(lines) => {
                let (reader, held) = lines.split();
                if self.detached {
                    pump_log(&self.shared, id, reader);
                    self.logs = Some((id.to_owned(), held));
                    return;
                }
                let (shared, who) = (self.shared.clone(), id.to_owned());
                match std::thread::Builder::new()
                    .name("containers-logs".into())
                    .spawn(move || pump_log(&shared, &who, reader))
                {
                    Ok(_) => self.logs = Some((id.to_owned(), held)),
                    Err(e) => tracing::warn!(error = %e, "could not follow the log"),
                }
            }
            Err(refusal) => self.shared.arrive(Arrival::LogRefused {
                id: id.to_owned(),
                refusal,
            }),
        }
    }

    fn start_stats(&mut self) {
        let Some(podman) = self.shared.podman.clone() else {
            self.shared
                .arrive(Arrival::Stats(Err(Refusal { why: Why::NoPodman })));
            return;
        };
        match podman.stats(STATS_SECONDS) {
            Ok(lines) => {
                let (reader, held) = lines.split();
                if self.detached {
                    pump_stats(&self.shared, reader, true);
                    self.stats = Some(held);
                    return;
                }
                let shared = self.shared.clone();
                match std::thread::Builder::new()
                    .name("containers-stats".into())
                    .spawn(move || pump_stats(&shared, reader, false))
                {
                    Ok(_) => self.stats = Some(held),
                    Err(e) => tracing::warn!(error = %e, "could not follow the stats"),
                }
            }
            Err(refusal) => self.shared.arrive(Arrival::Stats(Err(refusal))),
        }
    }

    /// Whether a log is being followed now — for the tests and the
    /// photograph's report.
    #[must_use]
    pub fn following_logs(&self) -> Option<&str> {
        self.logs.as_ref().map(|(id, _)| id.as_str())
    }

    /// Whether the stats are being followed now.
    #[must_use]
    pub const fn following_stats(&self) -> bool {
        self.stats.is_some()
    }
}

impl Readings {
    /// Takes one arrival. Answers what the message line should say, for an
    /// action's end: done, or refused with its reason.
    pub fn took(&mut self, a: Arrival, texts: &ronoctua_i18n::Texts) -> Option<String> {
        match a {
            Arrival::Listed(l) => self.listing = Some(l),
            Arrival::Firewall(f) => self.firewall = f,
            Arrival::Inspected(i) => self.inspected = Some(i),
            Arrival::LogLine { id, line } => {
                if let Some(log) = self.log.as_mut().filter(|l| l.id == id) {
                    log.push(line);
                }
            }
            Arrival::LogEnded { id } => {
                if let Some(log) = self.log.as_mut().filter(|l| l.id == id) {
                    log.ended = true;
                }
            }
            Arrival::LogRefused { id, refusal } => {
                if let Some(log) = self.log.as_mut().filter(|l| l.id == id) {
                    log.refused = Some(refusal);
                }
            }
            Arrival::Stats(s) => self.stats = Some(s),
            Arrival::Acted {
                action,
                name,
                result,
            } => {
                return Some(match result {
                    Ok(()) => texts.t_with(
                        crate::key::CONTAINERS_ECHO_DONE,
                        &[("what", &texts.t(action_key(&action))), ("name", &name)],
                    ),
                    Err(refusal) => texts.t_with(
                        crate::key::CONTAINERS_ECHO_REFUSED,
                        &[
                            ("what", &texts.t(action_key(&action))),
                            ("name", &name),
                            ("why", &crate::containers_words::refused(refusal, texts)),
                        ],
                    ),
                });
            }
            Arrival::LoggedIn { login, result } => {
                return Some(match result {
                    Ok(()) => texts.t_with(
                        crate::key::CONTAINERS_ECHO_LOGGED_IN,
                        &[("registry", &login.registry), ("user", &login.user)],
                    ),
                    Err(refusal) => texts.t_with(
                        crate::key::CONTAINERS_ECHO_LOGIN_REFUSED,
                        &[
                            ("registry", &login.registry),
                            ("why", &crate::containers_words::refused(refusal, texts)),
                        ],
                    ),
                });
            }
        }
        None
    }
}

/// An action's word, for the sentence after it.
#[must_use]
pub const fn action_key(action: &Action) -> &'static str {
    use crate::key;
    match action {
        Action::Start { .. } | Action::StartPod { .. } => key::CONTAINERS_DID_START,
        Action::Stop { .. } | Action::StopPod { .. } => key::CONTAINERS_DID_STOP,
        Action::Restart { .. } | Action::RestartPod { .. } => key::CONTAINERS_DID_RESTART,
        Action::Pause { .. } => key::CONTAINERS_DID_PAUSE,
        Action::Unpause { .. } => key::CONTAINERS_DID_UNPAUSE,
        Action::Remove { .. }
        | Action::RemovePod { .. }
        | Action::RemoveImage { .. }
        | Action::RemoveVolume { .. }
        | Action::RemoveNetwork { .. } => key::CONTAINERS_DID_REMOVE,
        Action::Prune { .. } => key::CONTAINERS_DID_PRUNE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};

    /// ONE TEST AT A TIME STARTS A STAND-IN: a script written and run, which a
    /// `fork` in another test's thread could hold open for writing.
    static FORKS: Mutex<()> = Mutex::new(());

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-containers-feed-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A stand-in podman: `logs` prints two lines and waits, `stats` one
    /// round and waits, anything else prints an empty list.
    fn stand_in(dir: &Path) -> Podman {
        let program = dir.join("podman");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\necho \"$1\" >> '{}/asked'\ncase \"$1\" in\n  logs) echo one; echo two; exec sleep 30 ;;\n  stats) echo '[{{\"id\":\"web\",\"cpu_percent\":\"1%\"}}]'; exec sleep 30 ;;\n  *) echo '[]' ;;\nesac\n",
                dir.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        Podman { program }
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    /// Takes what arrived until `done` says so, or ten seconds.
    fn until(feed: &Feed, r: &mut Readings, done: impl Fn(&Readings) -> bool) {
        let t = texts();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !done(r) {
            for a in feed.take() {
                r.took(a, &t);
            }
            assert!(std::time::Instant::now() < deadline, "never arrived");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn a_log_and_the_stats_live_only_while_wanted() {
        let _one = FORKS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = scratch("follow");
        let mut feed = Feed::new(Some(stand_in(&dir)), || {});
        let mut r = Readings::default();
        feed.follow(
            &Wants {
                inspect: None,
                logs: Some("web".to_owned()),
                stats: true,
            },
            &mut r,
        );
        assert_eq!(feed.following_logs(), Some("web"));
        assert!(feed.following_stats());
        until(&feed, &mut r, |r| {
            r.log.as_ref().is_some_and(|l| l.lines.len() == 2) && r.stats.is_some()
        });
        // NOTHING WANTED: both children go, and the readings forget them.
        feed.follow(&Wants::default(), &mut r);
        assert_eq!(feed.following_logs(), None);
        assert!(!feed.following_stats());
        assert!(r.log.is_none() && r.stats.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_podman_everything_is_a_refusal_with_its_reason() {
        let t = texts();
        let mut feed = Feed::detached(None, Firewall::default());
        let mut r = Readings::default();
        feed.start();
        for a in feed.take() {
            r.took(a, &t);
        }
        assert!(matches!(
            r.listing,
            Some(Err(Refusal { why: Why::NoPodman }))
        ));
        feed.follow(
            &Wants {
                inspect: None,
                logs: Some("web".to_owned()),
                stats: false,
            },
            &mut r,
        );
        for a in feed.take() {
            r.took(a, &t);
        }
        assert_eq!(
            r.log.as_ref().and_then(|l| l.refused).map(|x| x.why),
            Some(Why::NoPodman)
        );
        feed.act(
            Action::Stop {
                id: "web".to_owned(),
            },
            "web".to_owned(),
        );
        let said: Vec<String> = feed
            .take()
            .into_iter()
            .filter_map(|a| r.took(a, &t))
            .collect();
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(said[0].contains("web"), "{said:?}");
    }
}
