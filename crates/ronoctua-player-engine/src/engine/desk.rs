//! The desks: what the lookups found, kept for every door, and taken from.
//!
//! # One thread per desk, and the newest search wins
//!
//! Each desk — suggestions, subtitles, lyrics — has a thread of its own
//! (`player-lookup-<desk>`), started the first time it is asked and asleep on
//! its mailbox after that. A search is numbered when it is asked; what a
//! search finds is kept only while its number is still the desk's, so a new
//! search REPLACES the one under way: the old one stops between requests and
//! what it already had in flight is dropped. Takes wait behind each other on
//! the same thread (a download each), at most
//! [`wire::TAKES_WAITING_MOST`].
//!
//! Nothing here holds the engine's lock across the network: a thread takes
//! the lock only to post what it found.
//!
//! # What survives a door
//!
//! The desk is the daemon's, so a search asked in the footer is the page the
//! window finds. The state carries each desk's summary; a door asks for the
//! page when its generation moves ([`wire::Ask::Desk`]).
//!
//! # Consent, keys, and what is checked at once
//!
//! The consents are read from the preferences at the moment of asking, never
//! from a copy the daemon kept. A search or a take the person did not let is
//! refused AT ONCE, naming the host; so is one whose key is missing, a take
//! of a candidate a newer search replaced, and lyrics beside a song where a
//! `.lrc` is already there ([`Trouble::Exists`] — the door asks the person).

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

use ronoctua_lookup::{
    Service, Why,
    found::Found,
    internet::Internet,
    keys::Key,
    lrclib::Record,
    picture,
    suggestion::{Cover, Suggestion},
};
use ronoctua_player::{
    find::{
        CandidateId, Desk, LyricsCandidate, LyricsTo, Page, Query, SubtitleCandidate,
        SuggestedCover, SuggestionCandidate, TakeStatus, Taken,
    },
    limit::{Limit, TooMany},
    own::LaneKind,
    prefs::{self, Preferences},
    tongues,
    wire::{self, DeskKind, DeskSummary, Say, Trouble},
};
use ronoctua_tags::{CoverEdit, Edit, Field};

use super::lookups::{self, About, Kept, Keys as _, LoggedIn, Step};
use super::{Engine, Refused, Shared};

/// The desks the lookups keep, in the order the state lists them.
pub(super) const KINDS: [DeskKind; 3] =
    [DeskKind::Suggestions, DeskKind::Subtitles, DeskKind::Lyrics];

/// The folder a fetched cover waits in until it is written into its file:
/// `$XDG_RUNTIME_DIR/<name>-player-fetched`, `0700`, emptied when the player
/// starts and when it closes.
fn fetched_dir() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file("player-fetched")
}

/// One desk, and the work it has.
struct Slot {
    desk: Desk,
    /// The number of the search whose findings count.
    search: u64,
    /// That search is under way.
    searching: bool,
    /// Takes asked and not ended, the one under way included.
    takes: usize,
    /// Its thread's mailbox, once it runs.
    worker: Option<mpsc::Sender<Job>>,
}

impl Slot {
    fn new(kind: DeskKind) -> Self {
        Self {
            desk: Desk::empty(kind),
            search: 0,
            searching: false,
            takes: 0,
            worker: None,
        }
    }

    /// Something changed: the page's generation moves, and whether it is
    /// busy is said again.
    fn moved(&mut self) {
        self.desk.generation += 1;
        self.desk.busy = self.searching || self.takes > 0;
    }
}

/// The desks of the lookups.
pub(super) struct Desks {
    slots: Vec<Slot>,
    /// The last candidate number given — never given twice.
    candidates: u64,
    /// The OpenSubtitles login, in memory only.
    session: Arc<Mutex<Option<LoggedIn>>>,
    /// OpenSubtitles' downloads left today, as it last said.
    remaining: Option<i64>,
    /// Raised when the player closes: nothing found after it is posted.
    stop: Arc<AtomicBool>,
}

impl Desks {
    /// Empty desks; a cover left fetched by a player that died is gone.
    pub(super) fn open() -> Self {
        if let Some(dir) = fetched_dir() {
            let _ = std::fs::remove_dir_all(dir);
        }
        Self {
            slots: KINDS.iter().map(|k| Slot::new(*k)).collect(),
            candidates: 0,
            session: Arc::new(Mutex::new(None)),
            remaining: None,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The player is closing: nothing more is posted, and the covers waiting
    /// to be written go.
    pub(super) fn end(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(dir) = fetched_dir() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Each desk's summary, for the state.
    pub(super) fn summaries(&self) -> Vec<DeskSummary> {
        self.slots
            .iter()
            .map(|s| DeskSummary {
                kind: s.desk.kind,
                generation: s.desk.generation,
                busy: s.desk.busy,
                trouble: s.desk.trouble.clone(),
            })
            .collect()
    }

    /// A desk's page; an empty one for a desk the lookups do not keep.
    pub(super) fn page(&self, kind: DeskKind) -> Desk {
        self.slots
            .iter()
            .find(|s| s.desk.kind == kind)
            .map_or_else(|| Desk::empty(kind), |s| s.desk.clone())
    }

    fn slot(&mut self, kind: DeskKind) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|s| s.desk.kind == kind)
    }

    fn next_candidate(&mut self) -> CandidateId {
        self.candidates += 1;
        CandidateId(self.candidates)
    }

    /// A suggested cover asked to be SEEN ([`wire::Ask::SuggestedCover`]): the
    /// candidate marked as fetching, and what the desk's thread fetches —
    /// the search it belongs to and the cover. `Ok(None)` when it is already
    /// fetched or being fetched: nothing is asked twice.
    fn preview_asked(
        &mut self,
        candidate: CandidateId,
        prefs: &Preferences,
    ) -> Result<Option<(u64, Cover)>, Refused> {
        let gone = || Refused(Trouble::NoSuchCandidate { candidate });
        let slot = self.slot(DeskKind::Suggestions).ok_or_else(gone)?;
        let serial = slot.search;
        let Page::Suggestions { candidates, .. } = &mut slot.desk.page else {
            return Err(gone());
        };
        let c = candidates
            .iter_mut()
            .find(|c| c.id == candidate)
            .ok_or_else(gone)?;
        if matches!(
            c.seen,
            Some(SuggestedCover::Fetching | SuggestedCover::Kept { .. })
        ) {
            return Ok(None);
        }
        let Some(cover) = c.suggestion.cover.clone() else {
            return Err(lookups::lookup(ronoctua_lookup::Refusal::new(
                c.suggestion.source,
                Why::NothingFound,
            ))
            .into());
        };
        let service = cover_service(&cover);
        if !prefs.allows(service) {
            return Err(lookups::off(service).into());
        }
        c.seen = Some(SuggestedCover::Fetching);
        slot.moved();
        Ok(Some((serial, cover)))
    }

    /// A suggested cover fetched, or not: onto its candidate while the page
    /// is still search `serial`'s — else the picture is let go, since no
    /// page shows it. `false` when it was let go.
    fn preview_arrived(
        &mut self,
        serial: u64,
        candidate: CandidateId,
        preview: SuggestedCover,
    ) -> bool {
        let landed = self
            .slot(DeskKind::Suggestions)
            .filter(|slot| slot.search == serial)
            .and_then(|slot| {
                let Page::Suggestions { candidates, .. } = &mut slot.desk.page else {
                    return None;
                };
                let c = candidates.iter_mut().find(|c| c.id == candidate)?;
                c.seen = Some(preview.clone());
                slot.moved();
                Some(())
            })
            .is_some();
        if !landed && let SuggestedCover::Kept { path } = &preview {
            let _ = std::fs::remove_file(path);
        }
        landed
    }

    /// The suggested covers fetched to be seen on `kind`'s page, let go — a
    /// newer search replaces the page they were on.
    fn forget_previews(&mut self, kind: DeskKind) {
        let Some(slot) = self.slot(kind) else {
            return;
        };
        if let Page::Suggestions { candidates, .. } = &slot.desk.page {
            for c in candidates {
                if let Some(SuggestedCover::Kept { path }) = &c.seen {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    /// The downloads left today, said on the subtitles desk.
    fn set_remaining(&mut self, left: i64) {
        self.remaining = Some(left);
        if let Some(slot) = self.slot(DeskKind::Subtitles) {
            if let Page::Subtitles {
                remaining_downloads,
                ..
            } = &mut slot.desk.page
            {
                *remaining_downloads = Some(left);
            }
            slot.moved();
        }
    }
}

/// What the state says of what the search is about, when it plays: the
/// length a lyrics record is measured against, and the title and artist of
/// a stream whose sheet may say nothing.
#[derive(Debug, Clone, Default)]
struct Plays {
    duration_seconds: Option<f64>,
    title: Option<String>,
    artist: Option<String>,
}

/// Work for a desk's thread.
enum Job {
    Find {
        serial: u64,
        query: Query,
        prefs: Preferences,
        plays: Plays,
    },
    Take {
        candidate: CandidateId,
        what: TakeJob,
    },
    /// A suggested cover to fetch, to be seen.
    Preview {
        serial: u64,
        candidate: CandidateId,
        cover: Cover,
    },
}

/// A take that needs the network or the disk.
enum TakeJob {
    /// A suggestion's cover to fetch, then its fields and the cover to write.
    Cover {
        location: String,
        stamp: ronoctua_tags::Stamp,
        edits: Vec<Edit>,
        cover: Cover,
    },
    /// A subtitle to fetch, save and put over its film.
    Subtitle {
        found: Box<Found>,
        location: String,
        prefs: Preferences,
    },
    /// Lyrics to write beside a song.
    Lyrics {
        path: PathBuf,
        text: String,
        replace: bool,
        song: PathBuf,
    },
}

impl Engine {
    /// A search asked ([`wire::Ask::Find`]): checked now — the consent, the
    /// key —, then asked on the desk's thread, replacing the one under way.
    pub(super) fn find(&mut self, query: Query) -> Result<(), Refused> {
        let prefs = prefs::read();
        let query = with_absolute_location(query);
        if query.location().trim().is_empty() {
            return Err(lookups::lookup(ronoctua_lookup::Refusal::new(
                query.services()[0],
                Why::NothingToAsk,
            ))
            .into());
        }
        allowed(&prefs, &query)?;
        let plays = self.plays(query.location());
        let shared = self.shared()?;
        let kind = query.desk();
        // THE COVERS FETCHED TO BE SEEN go with the page they were on.
        self.desks.forget_previews(kind);
        let slot = self
            .desks
            .slot(kind)
            .ok_or(Refused(Trouble::NothingPlaying))?;
        slot.search += 1;
        slot.searching = true;
        let serial = slot.search;
        slot.desk.trouble = None;
        slot.desk.query = Some(query.clone());
        slot.desk.asked = None;
        slot.desk.refused.clear();
        slot.desk.page = Page::Nothing;
        slot.moved();
        send(
            slot,
            kind,
            Job::Find {
                serial,
                query,
                prefs,
                plays,
            },
            &shared,
        );
        self.dirty = true;
        Ok(())
    }

    /// What the state says of `location` when it is what plays.
    fn plays(&self, location: &str) -> Plays {
        let here = self
            .state
            .current
            .and_then(|c| self.location_of_leaf(c))
            .is_some_and(|l| same_place(&l, location));
        if !here {
            return Plays::default();
        }
        Plays {
            duration_seconds: self.state.duration,
            title: self.mpv_title.clone(),
            artist: self.state.artist.clone(),
        }
    }

    /// A suggested cover asked to be SEEN ([`wire::Ask::SuggestedCover`]): checked
    /// now — the candidate, its cover, the consent of its host —, then
    /// fetched on the suggestions desk's thread, behind whatever it does.
    pub(super) fn suggested_cover(&mut self, candidate: CandidateId) -> Result<(), Refused> {
        let prefs = prefs::read();
        let shared = self.shared()?;
        let Some((serial, cover)) = self.desks.preview_asked(candidate, &prefs)? else {
            return Ok(());
        };
        if let Some(slot) = self.desks.slot(DeskKind::Suggestions) {
            send(
                slot,
                DeskKind::Suggestions,
                Job::Preview {
                    serial,
                    candidate,
                    cover,
                },
                &shared,
            );
        }
        self.dirty = true;
        Ok(())
    }

    /// A desk's page ([`wire::Ask::Desk`]): a directory's, or a lookup's.
    pub(super) fn desk_page(&self, kind: DeskKind) -> Desk {
        self.directories
            .page(kind)
            .unwrap_or_else(|| self.desks.page(kind))
    }

    /// A take asked ([`wire::Ask::Take`]): checked now, then carried out —
    /// at once when it is only a write of what the page already has, on the
    /// desk's thread when something must be fetched or saved.
    pub(super) fn take(
        &mut self,
        kind: DeskKind,
        candidate: CandidateId,
        fields: &[Field],
        cover: bool,
        to: LyricsTo,
        replace: bool,
    ) -> Result<(), Refused> {
        let gone = || Refused(Trouble::NoSuchCandidate { candidate });
        let slot = self.desks.slot(kind).ok_or_else(gone)?;
        if slot.takes > wire::TAKES_WAITING_MOST {
            return Err(Trouble::TooMany(TooMany {
                what: Limit::Takes,
                most: wire::TAKES_WAITING_MOST,
            })
            .into());
        }
        let prefs = prefs::read();
        let job = match &slot.desk.page {
            Page::Suggestions {
                location,
                candidates,
            } => {
                let c = candidates
                    .iter()
                    .find(|c| c.id == candidate)
                    .ok_or_else(gone)?;
                let location = location.clone();
                let suggestion = c.suggestion.clone();
                return self.take_suggestion(
                    kind,
                    candidate,
                    &location,
                    &suggestion,
                    fields,
                    cover,
                    prefs,
                );
            }
            Page::Subtitles {
                location,
                candidates,
                ..
            } => {
                let c = candidates
                    .iter()
                    .find(|c| c.id == candidate)
                    .ok_or_else(gone)?;
                if !prefs.allows(c.found.source) {
                    return Err(lookups::off(c.found.source).into());
                }
                TakeJob::Subtitle {
                    found: Box::new(c.found.clone()),
                    location: location.clone(),
                    prefs,
                }
            }
            Page::Lyrics {
                location,
                candidates,
                ..
            } => {
                let c = candidates
                    .iter()
                    .find(|c| c.id == candidate)
                    .ok_or_else(gone)?;
                let record = c.record.clone();
                let location = location.clone();
                match self.take_lyrics(kind, candidate, &location, &record, to, replace)? {
                    Some(job) => job,
                    None => return Ok(()),
                }
            }
            // NOTHING TO TAKE: no page, or a directory's (its rows are put in
            // the queue by the door, never taken).
            Page::Nothing
            | Page::RadioCountries { .. }
            | Page::Stations { .. }
            | Page::Genres { .. }
            | Page::Languages { .. }
            | Page::TvCountries { .. }
            | Page::Channels { .. } => return Err(gone()),
        };
        self.queue_take(kind, candidate, job)
    }

    /// A take for the desk's thread.
    fn queue_take(
        &mut self,
        kind: DeskKind,
        candidate: CandidateId,
        what: TakeJob,
    ) -> Result<(), Refused> {
        let shared = self.shared()?;
        let Some(slot) = self.desks.slot(kind) else {
            return Err(Trouble::NoSuchCandidate { candidate }.into());
        };
        slot.takes += 1;
        slot.desk.taken = Some(Taken {
            candidate,
            status: TakeStatus::Taking,
        });
        slot.moved();
        send(slot, kind, Job::Take { candidate, what }, &shared);
        self.dirty = true;
        Ok(())
    }

    /// Tags suggested, taken: the fields written at once, or the cover
    /// fetched first on the desk's thread.
    #[allow(clippy::too_many_arguments)]
    fn take_suggestion(
        &mut self,
        kind: DeskKind,
        candidate: CandidateId,
        location: &str,
        suggestion: &Suggestion,
        fields: &[Field],
        cover: bool,
        prefs: Preferences,
    ) -> Result<(), Refused> {
        let (path, stamp) = local_file(location)?;
        let location = path.to_string_lossy().into_owned();
        let mut edits = Vec::new();
        for field in fields {
            // ONLY WHAT THE PAGE SHOWED: a field the candidate does not have
            // is nothing to write.
            if let Some(value) = suggestion.value(*field) {
                let value = field
                    .check(value)
                    .map_err(|why| super::tags::trouble(&location, why))?;
                edits.push(Edit::Set {
                    field: *field,
                    value,
                });
            }
        }
        let cover = suggestion.cover.clone().filter(|_| cover);
        if let Some(cover) = cover {
            let service = cover_service(&cover);
            if !prefs.allows(service) {
                return Err(lookups::off(service).into());
            }
            return self.queue_take(
                kind,
                candidate,
                TakeJob::Cover {
                    location,
                    stamp,
                    edits,
                    cover,
                },
            );
        }
        if edits.is_empty() {
            return Ok(());
        }
        self.retag_taken(location, stamp, edits, None)?;
        if let Some(slot) = self.desks.slot(kind) {
            slot.desk.taken = Some(Taken {
                candidate,
                status: TakeStatus::Handed,
            });
            slot.moved();
        }
        Ok(())
    }

    /// Lyrics taken: into the song's tag at once, or beside it on the
    /// desk's thread — refused now when a `.lrc` is there and the person
    /// was not asked.
    fn take_lyrics(
        &mut self,
        kind: DeskKind,
        candidate: CandidateId,
        location: &str,
        record: &Record,
        to: LyricsTo,
        replace: bool,
    ) -> Result<Option<TakeJob>, Refused> {
        let (path, stamp) = local_file(location)?;
        let Some(text) = lookups::lrc_of(record) else {
            return Err(lookups::lookup(ronoctua_lookup::Refusal::new(
                Service::Lrclib,
                Why::NothingFound,
            ))
            .into());
        };
        match to {
            LyricsTo::Tag => {
                let words = record.best().map_or(text.as_str(), |(_, t)| t).to_owned();
                let value = Field::Lyrics
                    .check(&words)
                    .map_err(|why| super::tags::trouble(location, why))?;
                let file = path.to_string_lossy().into_owned();
                self.retag_taken(
                    file,
                    stamp,
                    vec![Edit::Set {
                        field: Field::Lyrics,
                        value,
                    }],
                    None,
                )?;
                if let Some(slot) = self.desks.slot(kind) {
                    slot.desk.taken = Some(Taken {
                        candidate,
                        status: TakeStatus::Handed,
                    });
                    slot.moved();
                }
                Ok(None)
            }
            LyricsTo::Beside => {
                let lrc = path.with_extension("lrc");
                // THE PERSON'S TO SAY: a `.lrc` that is there is replaced
                // only when the door asked ("no" first) and sent `replace`.
                if !replace && lrc.exists() {
                    return Err(Trouble::Exists {
                        path: lrc.to_string_lossy().into_owned(),
                    }
                    .into());
                }
                Ok(Some(TakeJob::Lyrics {
                    path: lrc,
                    text,
                    replace,
                    song: path,
                }))
            }
        }
    }

    /// A search's findings, posted by its thread — kept only while it is
    /// still the desk's search.
    fn post(&mut self, kind: DeskKind, serial: u64, change: impl FnOnce(&mut Desks, DeskKind)) {
        let current = self.desks.slot(kind).is_some_and(|s| s.search == serial);
        if !current || self.desks.stop.load(Ordering::SeqCst) {
            return;
        }
        change(&mut self.desks, kind);
        if let Some(slot) = self.desks.slot(kind) {
            slot.moved();
        }
        self.dirty = true;
    }

    /// A search ended: the page stays as it was posted; the trouble, when
    /// there is one, is said.
    fn search_ended(&mut self, kind: DeskKind, serial: u64, trouble: Option<Trouble>) {
        self.post(kind, serial, |desks, kind| {
            if let Some(slot) = desks.slot(kind) {
                slot.searching = false;
                slot.desk.trouble = trouble;
            }
        });
    }

    /// A take ended, and how.
    fn take_ended(&mut self, kind: DeskKind, candidate: CandidateId, status: TakeStatus) {
        if let TakeStatus::Refused(t) = &status {
            // A REFUSAL NOBODY HEARS is the silence the protocol promises not
            // to leave: said with the others too.
            self.state.troubles = vec![t.clone()];
        }
        if let Some(slot) = self.desks.slot(kind) {
            slot.takes = slot.takes.saturating_sub(1);
            slot.desk.taken = Some(Taken { candidate, status });
            slot.moved();
        }
        self.dirty = true;
    }

    /// A subtitle saved for `location` becomes a lane over it: over the
    /// picture of what plays that is that film, else over the film of the
    /// queue named like it; a film that is neither keeps it for later (the
    /// file is beside it, and the Subtitles page lists it).
    fn put_over(&mut self, saved: &Path, location: &str) -> Result<(), Refused> {
        let saved = saved.to_string_lossy().into_owned();
        let (lanes, _) = self.lanes_now();
        let picture = lanes
            .iter()
            .find(|l| l.kind == LaneKind::Video && same_place(&l.source, location))
            .map(|l| l.id);
        if let Some(over) = picture {
            return match self.add_lane(&saved, LaneKind::Subtitle, None, 0.0, Some(over)) {
                // STILL LOADING: the lane goes on once its tracks are read.
                Err(Refused(Trouble::NothingPlaying)) => {
                    if let Some(current) = self.state.current {
                        self.subtitles.attach.push((current, saved));
                    }
                    Ok(())
                }
                other => other,
            };
        }
        if self.named_after(&saved).is_some() {
            let shared = self.shared()?;
            return self.open_subtitle(
                &saved,
                ronoctua_player::queue::Place::End(None),
                false,
                &shared,
            );
        }
        Ok(())
    }
}

/// A file on this machine, canonical, and its stamp — what a write into a
/// file (its tags, lyrics beside it) needs. An address is none.
fn local_file(location: &str) -> Result<(PathBuf, ronoctua_tags::Stamp), Refused> {
    if location.contains("://") {
        return Err(super::tags::trouble(location, ronoctua_tags::Why::NotAFile).into());
    }
    super::tags::canonical(location).map_err(|why| super::tags::trouble(location, why).into())
}

/// The query with its location absolute, as the queue writes locations.
fn with_absolute_location(query: Query) -> Query {
    match query {
        Query::Suggest {
            location,
            source,
            words,
            tongues,
        } => Query::Suggest {
            location: super::absolute(location.trim()),
            source,
            words,
            tongues,
        },
        Query::Subtitles {
            location,
            tongues,
            words,
        } => Query::Subtitles {
            location: super::absolute(location.trim()),
            tongues,
            words,
        },
        Query::Lyrics { location, words } => Query::Lyrics {
            location: super::absolute(location.trim()),
            words,
        },
        // A DIRECTORY'S LISTING is about no file (`directory.rs`).
        other @ (Query::Radio(_) | Query::Tv(_)) => other,
    }
}

/// Whether `a` and `b` name the same file (or the same address).
fn same_place(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if a.contains("://") || b.contains("://") {
        return false;
    }
    matches!(
        (std::fs::canonicalize(a), std::fs::canonicalize(b)),
        (Ok(x), Ok(y)) if x == y
    )
}

/// The service a suggested cover is fetched from.
const fn cover_service(cover: &Cover) -> Service {
    match cover {
        Cover::CoverArt { .. } => Service::CoverArtArchive,
        Cover::Address { host, .. } => host.service(),
    }
}

/// Whether the person lets this search be asked, and a key it needs is
/// kept — refused at once, naming what is missing.
fn allowed(prefs: &Preferences, query: &Query) -> Result<(), Refused> {
    match query {
        Query::Subtitles { .. } => {
            // ANY subtitle service the person let and kept a key for; the
            // others are said on the page.
            let services = query.services();
            let let_ones: Vec<Service> = services
                .iter()
                .copied()
                .filter(|s| prefs.allows(*s))
                .collect();
            let Some(first) = let_ones.first() else {
                return Err(lookups::off(services[0]).into());
            };
            let keyed = let_ones
                .iter()
                .any(|s| s.key().is_some_and(|k| Kept.key(k).is_ok()));
            if keyed {
                return Ok(());
            }
            let key = first.key().unwrap_or(Key::OpenSubtitles);
            Kept.key(key)
                .map(drop)
                .map_err(|r| lookups::lookup(r).into())
        }
        _ => {
            lookups::let_all(prefs, query.services())?;
            for key in query.services().iter().filter_map(|s| s.key()) {
                Kept.key(key).map_err(lookups::lookup)?;
            }
            Ok(())
        }
    }
}

/// Hands a job to a desk's thread, starting it when it does not run.
fn send(slot: &mut Slot, kind: DeskKind, job: Job, shared: &Shared) {
    let worker = slot
        .worker
        .get_or_insert_with(|| spawn_worker(kind, shared.clone()));
    if let Err(mpsc::SendError(job)) = worker.send(job) {
        // A THREAD THAT DIED is started again, once.
        let worker = spawn_worker(kind, shared.clone());
        let _ = worker.send(job);
        slot.worker = Some(worker);
    }
}

/// A desk's thread: its jobs one at a time, in the order asked.
fn spawn_worker(kind: DeskKind, shared: Shared) -> mpsc::Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    let name = match kind {
        DeskKind::Suggestions => "player-lookup-suggestions",
        DeskKind::Subtitles => "player-lookup-subtitles",
        DeskKind::Lyrics => "player-lookup-lyrics",
        _ => "player-lookup",
    };
    let _ = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                match job {
                    Job::Find {
                        serial,
                        query,
                        prefs,
                        plays,
                    } => run_find(kind, serial, &query, &prefs, &plays, &shared),
                    Job::Take { candidate, what } => run_take(kind, candidate, what, &shared),
                    Job::Preview {
                        serial,
                        candidate,
                        cover,
                    } => run_preview(serial, candidate, &cover, &shared),
                }
            }
        });
    tx
}

/// With the engine, when it is still there and not closing: a search or a
/// take that ends after the farewell changes nothing and tells nobody.
pub(super) fn with_engine(shared: &Shared, f: impl FnOnce(&mut Engine)) {
    let (lock, bell) = &**shared;
    {
        let Ok(mut e) = lock.lock() else { return };
        if e.leaving {
            return;
        }
        f(&mut e);
        if e.dirty {
            e.broadcast();
        }
    }
    bell.notify_all();
}

/// Whether search `serial` is still the desk's, and the player not closing.
fn still(shared: &Shared, kind: DeskKind, serial: u64) -> bool {
    shared.0.lock().is_ok_and(|mut e| {
        !e.desks.stop.load(Ordering::SeqCst)
            && e.desks.slot(kind).is_some_and(|s| s.search == serial)
    })
}

/// What the daemon knows of `location`: the file, its sheet (read on the
/// sheet thread, or what an address says while it plays), its length.
fn about_of(location: &str, plays: &Plays, shared: &Shared) -> About {
    let file = (!location.contains("://"))
        .then(|| std::fs::canonicalize(location).ok())
        .flatten()
        .filter(|p| p.is_file());
    About {
        location: location.to_owned(),
        file,
        sheet: super::tags::sheet_of(location, shared).ok(),
        duration_seconds: plays.duration_seconds,
    }
}

/// One search, on its desk's thread.
fn run_find(
    kind: DeskKind,
    serial: u64,
    query: &Query,
    prefs: &Preferences,
    plays: &Plays,
    shared: &Shared,
) {
    if !still(shared, kind, serial) {
        return;
    }
    let about = about_of(query.location(), plays, shared);
    let net = Internet;
    let trouble = match query {
        Query::Suggest {
            source,
            words,
            tongues: asked_in,
            ..
        } => {
            let asked_in = tongues::all_of(asked_in);
            let alive = || still(shared, kind, serial);
            let mut step = |s: Step| {
                with_engine(shared, |e| {
                    e.post(kind, serial, |desks, kind| {
                        post_step(desks, kind, &about.location, s)
                    });
                });
            };
            match lookups::suggest(
                &net,
                &Kept,
                *source,
                words.as_deref(),
                &asked_in,
                &about,
                &alive,
                &mut step,
            ) {
                Ok(asked) => {
                    with_engine(shared, |e| {
                        e.post(kind, serial, |desks, kind| {
                            if let Some(slot) = desks.slot(kind) {
                                slot.desk.asked = Some(asked);
                                if let Page::Suggestions { candidates, .. } = &mut slot.desk.page {
                                    // PAST THE FEW READ, a candidate is what
                                    // the search said: complete as it is.
                                    for c in candidates.iter_mut().skip(lookups::DETAILED) {
                                        c.complete = true;
                                    }
                                }
                            }
                        });
                    });
                    None
                }
                Err(t) => Some(t),
            }
        }
        Query::Subtitles {
            tongues: asked_in,
            words,
            ..
        } => {
            let mut asked_in = tongues::all_of(asked_in);
            if asked_in.is_empty() {
                asked_in = tongues::default_for(tongues::ENGLISH);
            }
            let session = shared
                .0
                .lock()
                .map(|e| e.desks.session.clone())
                .unwrap_or_default();
            match lookups::subtitles(
                &net,
                &Kept,
                prefs,
                &session,
                &asked_in,
                words.as_deref(),
                &about,
            ) {
                Ok(found) => {
                    let codes: Vec<String> = asked_in.iter().map(|t| t.code.to_owned()).collect();
                    let trouble = found.trouble.clone();
                    with_engine(shared, |e| {
                        e.post(kind, serial, |desks, kind| {
                            if let Some(left) = found.remaining {
                                desks.remaining = Some(left);
                            }
                            let remaining = desks.remaining;
                            let candidates: Vec<SubtitleCandidate> = found
                                .found
                                .into_iter()
                                .map(|f| {
                                    let tongue =
                                        tongues::of(&f.language).map(|t| t.code.to_owned());
                                    SubtitleCandidate {
                                        id: desks.next_candidate(),
                                        found: f,
                                        tongue,
                                    }
                                })
                                .collect();
                            if let Some(slot) = desks.slot(kind) {
                                slot.desk.asked = Some(found.asked);
                                slot.desk.refused = found.refused;
                                slot.desk.page = Page::Subtitles {
                                    location: about.location.clone(),
                                    tongues: codes,
                                    candidates,
                                    remaining_downloads: remaining,
                                };
                            }
                        });
                    });
                    trouble
                }
                Err(t) => Some(t),
            }
        }
        Query::Lyrics { words, .. } => {
            match lookups::lyrics(
                &net,
                words.as_deref(),
                plays.title.as_deref(),
                plays.artist.as_deref(),
                &about,
            ) {
                Ok((asked, records)) => {
                    let duration = about.duration();
                    with_engine(shared, |e| {
                        e.post(kind, serial, |desks, kind| {
                            let candidates: Vec<LyricsCandidate> = records
                                .into_iter()
                                .map(|record| LyricsCandidate {
                                    id: desks.next_candidate(),
                                    off_by_seconds: record.off_by_seconds(duration),
                                    record,
                                })
                                .collect();
                            if let Some(slot) = desks.slot(kind) {
                                slot.desk.asked = Some(asked);
                                slot.desk.page = Page::Lyrics {
                                    location: about.location.clone(),
                                    duration_seconds: duration,
                                    candidates,
                                };
                            }
                        });
                    });
                    None
                }
                Err(t) => Some(t),
            }
        }
        // A DIRECTORY'S LISTING is `directory.rs`'s, on a desk of its own;
        // it never reaches a lookup's thread.
        Query::Radio(_) | Query::Tv(_) => None,
    };
    with_engine(shared, |e| e.search_ended(kind, serial, trouble));
}

/// A suggestion search's step, onto its page.
fn post_step(desks: &mut Desks, kind: DeskKind, location: &str, step: Step) {
    match step {
        Step::List(found, complete) => {
            let candidates: Vec<SuggestionCandidate> = found
                .into_iter()
                .map(|suggestion| SuggestionCandidate {
                    id: desks.next_candidate(),
                    suggestion,
                    complete,
                    seen: None,
                })
                .collect();
            if let Some(slot) = desks.slot(kind) {
                slot.desk.page = Page::Suggestions {
                    location: location.to_owned(),
                    candidates,
                };
            }
        }
        Step::Detail(n, suggestion) => {
            if let Some(slot) = desks.slot(kind)
                && let Page::Suggestions { candidates, .. } = &mut slot.desk.page
                && let Some(c) = candidates.get_mut(n)
            {
                c.suggestion = suggestion;
                c.complete = true;
            }
        }
    }
}

/// One take, on its desk's thread.
fn run_take(kind: DeskKind, candidate: CandidateId, what: TakeJob, shared: &Shared) {
    let net = Internet;
    match what {
        TakeJob::Cover {
            location,
            stamp,
            mut edits,
            cover,
        } => {
            let fetched = picture::cover(&net, &cover)
                .map_err(lookups::lookup)
                .and_then(|p| keep_fetched("cover", candidate, &p));
            with_engine(shared, |e| match fetched {
                Ok(scratch) => {
                    edits.push(Edit::Cover(CoverEdit::Replace {
                        from: scratch.clone(),
                    }));
                    let status = match e.retag_taken(location, stamp, edits, Some(scratch.clone()))
                    {
                        Ok(()) => TakeStatus::Handed,
                        Err(Refused(t)) => {
                            let _ = std::fs::remove_file(&scratch);
                            TakeStatus::Refused(t)
                        }
                    };
                    e.take_ended(kind, candidate, status);
                }
                Err(t) => e.take_ended(kind, candidate, TakeStatus::Refused(t)),
            });
        }
        TakeJob::Subtitle {
            found,
            location,
            prefs,
        } => {
            let session = shared
                .0
                .lock()
                .map(|e| e.desks.session.clone())
                .unwrap_or_default();
            let about = about_of(&location, &Plays::default(), shared);
            let got = lookups::fetch_subtitle(&net, &Kept, &prefs, &session, &found, &about);
            let saved = got.and_then(|(fetched, left)| {
                lookups::save_subtitle(
                    &about,
                    &found.language,
                    &fetched,
                    lookups::kept_subtitles_dir(&location),
                )
                .map(|path| (path, left))
            });
            with_engine(shared, |e| match saved {
                Ok((path, left)) => {
                    if let Some(left) = left {
                        e.desks.set_remaining(left);
                    }
                    let status = match e.put_over(&path, &location) {
                        Ok(()) => TakeStatus::Saved {
                            path: path.to_string_lossy().into_owned(),
                        },
                        // SAVED, and not put over the film: the file is there
                        // all the same, and the refusal says why.
                        Err(Refused(t)) => TakeStatus::Refused(t),
                    };
                    e.take_ended(kind, candidate, status);
                }
                Err(t) => e.take_ended(kind, candidate, TakeStatus::Refused(t)),
            });
        }
        TakeJob::Lyrics {
            path,
            text,
            replace,
            song,
        } => {
            let saved = lookups::save_lyrics(&path, &text, replace);
            with_engine(shared, |e| match saved {
                Ok(()) => {
                    e.lyrics_saved(&song, &text);
                    e.take_ended(
                        kind,
                        candidate,
                        TakeStatus::Saved {
                            path: path.to_string_lossy().into_owned(),
                        },
                    );
                }
                Err(t) => e.take_ended(kind, candidate, TakeStatus::Refused(t)),
            });
        }
    }
}

/// A suggested cover fetched to be seen, on its desk's thread: kept as
/// `preview-<candidate>.<ext>` and posted onto its candidate — or, the page
/// replaced meanwhile, let go.
fn run_preview(serial: u64, candidate: CandidateId, cover: &Cover, shared: &Shared) {
    if !still(shared, DeskKind::Suggestions, serial) {
        return;
    }
    let preview = match picture::preview(&Internet, cover)
        .map_err(lookups::lookup)
        .and_then(|p| keep_fetched("preview", candidate, &p))
    {
        Ok(path) => SuggestedCover::Kept {
            path: path.to_string_lossy().into_owned(),
        },
        Err(t) => SuggestedCover::Refused(t),
    };
    with_engine(shared, |e| {
        if !e.desks.stop.load(Ordering::SeqCst)
            && e.desks.preview_arrived(serial, candidate, preview)
        {
            e.dirty = true;
        }
    });
}

/// A fetched cover in the folder it waits in, `<what>-<candidate>.<ext>`:
/// `cover` for one to be written, `preview` for one to be seen.
fn keep_fetched(
    what: &str,
    candidate: CandidateId,
    picture: &picture::Picture,
) -> Result<PathBuf, Trouble> {
    let unwritable = |path: &Path, e: &std::io::Error| Trouble::Unwritable {
        path: path.to_string_lossy().into_owned(),
        why: e.to_string(),
    };
    let dir = fetched_dir().ok_or(Trouble::NoRuntimeDir)?;
    {
        use std::os::unix::fs::DirBuilderExt as _;
        let mut b = std::fs::DirBuilder::new();
        b.recursive(true).mode(0o700);
        b.create(&dir).map_err(|e| unwritable(&dir, &e))?;
    }
    let path = dir.join(format!(
        "{what}-{}.{}",
        candidate.0,
        picture.kind.extension()
    ));
    std::fs::write(&path, &picture.bytes).map_err(|e| unwritable(&path, &e))?;
    Ok(path)
}

impl Engine {
    /// Lyrics were written beside `song`: when it is what plays, they are the
    /// lyrics shown now, from the file.
    fn lyrics_saved(&mut self, song: &Path, text: &str) {
        let playing = self
            .voices
            .first()
            .and_then(|v| v.loaded.clone())
            .or_else(|| self.loaded.clone());
        if playing.is_some_and(|l| same_place(&l, &song.to_string_lossy())) {
            self.state.lyrics = ronoctua_player::lyrics::parse(text);
            self.state.lyrics_from = Some("file".to_owned());
            self.dirty = true;
        }
    }
}

/// Tries the key the person kept for `service` ([`wire::Ask::KeyCheck`]) —
/// on the asking connection's thread, never under the lock — and says the
/// downloads left today when a login said.
pub(super) fn key_check(service: Service, shared: &Shared) -> Say {
    let prefs = prefs::read();
    let session = shared
        .0
        .lock()
        .map(|e| e.desks.session.clone())
        .unwrap_or_default();
    match lookups::key_check(&Internet, &Kept, &prefs, &session, service) {
        Ok(left) => {
            if let Some(left) = left {
                with_engine(shared, |e| {
                    e.desks.set_remaining(left);
                    e.dirty = true;
                });
            }
            Say::Done
        }
        Err(t) => Say::Refused(t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_lookup::suggestion::Suggestion;

    fn suggestion(title: &str) -> Suggestion {
        let mut s = Suggestion::new(Service::MusicBrainz, title.into());
        s.put(Field::Title, Some(title.into()));
        s
    }

    /// FAILS IF: the state stops listing the three desks of the lookups —
    /// a door would not know there is a page to ask for.
    #[test]
    fn the_state_lists_the_three_desks_from_the_start() {
        let desks = Desks {
            slots: KINDS.iter().map(|k| Slot::new(*k)).collect(),
            candidates: 0,
            session: Arc::new(Mutex::new(None)),
            remaining: None,
            stop: Arc::new(AtomicBool::new(false)),
        };
        let kinds: Vec<DeskKind> = desks.summaries().iter().map(|s| s.kind).collect();
        assert_eq!(kinds, KINDS);
        assert!(
            desks
                .summaries()
                .iter()
                .all(|s| s.generation == 0 && !s.busy)
        );
        assert_eq!(desks.page(DeskKind::Radio), Desk::empty(DeskKind::Radio));
    }

    /// FAILS IF: a candidate's number is ever given twice — a take aimed at
    /// a page a newer search replaced would be carried out on whatever sits
    /// in its place — or a detail lands on the wrong candidate.
    #[test]
    fn a_candidate_is_numbered_once_and_its_details_land_on_it() {
        let mut desks = Desks {
            slots: KINDS.iter().map(|k| Slot::new(*k)).collect(),
            candidates: 0,
            session: Arc::new(Mutex::new(None)),
            remaining: None,
            stop: Arc::new(AtomicBool::new(false)),
        };
        let kind = DeskKind::Suggestions;
        post_step(
            &mut desks,
            kind,
            "/m/a.flac",
            Step::List(vec![suggestion("a"), suggestion("b")], false),
        );
        let first: Vec<CandidateId> = match &desks.page(kind).page {
            Page::Suggestions { candidates, .. } => candidates.iter().map(|c| c.id).collect(),
            other => panic!("{other:?}"),
        };
        post_step(
            &mut desks,
            kind,
            "/m/a.flac",
            Step::Detail(1, suggestion("B")),
        );
        match &desks.page(kind).page {
            Page::Suggestions { candidates, .. } => {
                assert!(!candidates[0].complete);
                assert!(candidates[1].complete);
                assert_eq!(candidates[1].suggestion.value(Field::Title), Some("B"));
            }
            other => panic!("{other:?}"),
        }
        // A NEW SEARCH, new numbers.
        post_step(
            &mut desks,
            kind,
            "/m/a.flac",
            Step::List(vec![suggestion("c")], true),
        );
        let second: Vec<CandidateId> = match &desks.page(kind).page {
            Page::Suggestions { candidates, .. } => candidates.iter().map(|c| c.id).collect(),
            other => panic!("{other:?}"),
        };
        assert!(
            second.iter().all(|id| !first.contains(id)),
            "{first:?} {second:?}"
        );
    }

    /// FAILS IF: a suggested cover asked to be seen is fetched without the
    /// person having let its host, asked twice, or lands on a page a newer
    /// search replaced — or one that landed is not let go with its page.
    #[test]
    fn a_preview_is_asked_once_and_lands_only_on_its_page() {
        let mut desks = Desks {
            slots: KINDS.iter().map(|k| Slot::new(*k)).collect(),
            candidates: 0,
            session: Arc::new(Mutex::new(None)),
            remaining: None,
            stop: Arc::new(AtomicBool::new(false)),
        };
        let kind = DeskKind::Suggestions;
        let mut with_cover = suggestion("a");
        with_cover.cover = Some(Cover::CoverArt {
            release: "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d".into(),
        });
        post_step(
            &mut desks,
            kind,
            "/m/a.flac",
            Step::List(vec![with_cover, suggestion("b")], true),
        );
        let ids: Vec<CandidateId> = match &desks.page(kind).page {
            Page::Suggestions { candidates, .. } => candidates.iter().map(|c| c.id).collect(),
            other => panic!("{other:?}"),
        };
        // NOT LET: refused, naming the covers' host, and nothing marked.
        let mut p = Preferences::default();
        let Err(Refused(Trouble::Off { host })) = desks.preview_asked(ids[0], &p) else {
            panic!("a cover whose host was not let was asked");
        };
        assert_eq!(host, "coverartarchive.org");
        // NO COVER to see.
        p.consent(Service::CoverArtArchive);
        assert!(matches!(
            desks.preview_asked(ids[1], &p),
            Err(Refused(Trouble::Lookup(_)))
        ));
        // LET: marked as fetching once, and the second ask asks nothing.
        let before = desks.page(kind).generation;
        let Ok(Some((serial, _))) = desks.preview_asked(ids[0], &p) else {
            panic!("a let cover was not asked");
        };
        assert!(desks.page(kind).generation > before);
        assert!(matches!(desks.preview_asked(ids[0], &p), Ok(None)));
        // LANDED on its candidate.
        let dir = std::env::temp_dir().join(format!("ronoctua-preview-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let kept = dir.join("preview-1.jpg");
        std::fs::write(&kept, b"x").expect("a picture");
        let landed = SuggestedCover::Kept {
            path: kept.to_string_lossy().into_owned(),
        };
        assert!(desks.preview_arrived(serial, ids[0], landed.clone()));
        match &desks.page(kind).page {
            Page::Suggestions { candidates, .. } => {
                assert_eq!(candidates[0].seen, Some(landed));
            }
            other => panic!("{other:?}"),
        }
        // A NEWER SEARCH lets it go, and one fetched for the old page after
        // it lands nowhere and is let go too.
        desks.forget_previews(kind);
        assert!(!kept.exists(), "the picture outlived its page");
        let late = dir.join("preview-2.jpg");
        std::fs::write(&late, b"x").expect("a picture");
        assert!(!desks.preview_arrived(
            serial + 1,
            ids[0],
            SuggestedCover::Kept {
                path: late.to_string_lossy().into_owned()
            }
        ));
        assert!(!late.exists(), "a picture for no page was kept");
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: the downloads OpenSubtitles says are left stop reaching the
    /// subtitles desk — or reach it without its generation moving.
    #[test]
    fn the_downloads_left_are_said_on_the_subtitles_desk() {
        let mut desks = Desks {
            slots: KINDS.iter().map(|k| Slot::new(*k)).collect(),
            candidates: 0,
            session: Arc::new(Mutex::new(None)),
            remaining: None,
            stop: Arc::new(AtomicBool::new(false)),
        };
        if let Some(slot) = desks.slot(DeskKind::Subtitles) {
            slot.desk.page = Page::Subtitles {
                location: "/v/f.mkv".into(),
                tongues: vec!["en".into()],
                candidates: Vec::new(),
                remaining_downloads: None,
            };
        }
        desks.set_remaining(4);
        let page = desks.page(DeskKind::Subtitles);
        assert_eq!(page.generation, 1);
        assert!(matches!(
            page.page,
            Page::Subtitles {
                remaining_downloads: Some(4),
                ..
            }
        ));
    }

    /// FAILS IF: a search of a service the person did not let reaches the
    /// keys, or anything else, before it is refused naming the host.
    #[test]
    fn a_search_not_let_is_refused_before_anything_is_read() {
        let p = Preferences::default();
        for query in [
            Query::Suggest {
                location: "/m/a.flac".into(),
                source: ronoctua_player::find::Suggester::MusicBrainz,
                words: None,
                tongues: Vec::new(),
            },
            Query::Lyrics {
                location: "/m/a.flac".into(),
                words: None,
            },
            Query::Subtitles {
                location: "/v/f.mkv".into(),
                tongues: Vec::new(),
                words: None,
            },
        ] {
            let Err(Refused(t)) = allowed(&p, &query) else {
                panic!("{query:?} was let");
            };
            assert!(matches!(t, Trouble::Off { .. }), "{t:?}");
        }
        let mut p = Preferences::default();
        p.consent(Service::MusicBrainz);
        let Err(Refused(Trouble::Off { host })) = allowed(
            &p,
            &Query::Suggest {
                location: "/m/a.flac".into(),
                source: ronoctua_player::find::Suggester::MusicBrainz,
                words: None,
                tongues: Vec::new(),
            },
        ) else {
            panic!("the covers' host was not let");
        };
        assert_eq!(host, "coverartarchive.org");
    }

    /// FAILS IF: a location a door gives is not made absolute, as the queue
    /// writes it — the page's film would not be the lane's.
    #[test]
    fn a_query_names_its_file_as_the_queue_does() {
        let q = with_absolute_location(Query::Lyrics {
            location: " https://radio.example/live ".into(),
            words: None,
        });
        assert_eq!(q.location(), "https://radio.example/live");
        assert!(same_place("/a/../a/b", "/a/b") || !Path::new("/a/b").exists());
        assert!(!same_place("https://x/a", "/x/a"));
    }
}
