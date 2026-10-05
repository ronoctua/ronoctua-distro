//! THE LOOKUPS, as a door sees them (plan P8): the daemon's desks held by
//! the view, and the two pages that search the network for the file that
//! plays — subtitles for a film, lyrics for a song. The suggestions for its
//! tags are `crate::player_suggest`'s, on the same desks.
//!
//! # The daemon asks, the door shows
//!
//! Every search is the daemon's (`ronoctua_player::find`): one limiter per
//! host for the whole machine, and a page that outlives the door that asked
//! — a search pressed in the footer is the page the window finds. The state
//! carries each desk's summary (`State::desks`); a door that shows a desk's
//! page asks for it when its generation moves, OFF ITS LOOP, and is woken
//! when it is there ([`Found::look`], the shape of `player_metadata::Meta`).
//! A door with no loop — a photograph, a test — is handed its pages
//! ([`Found::given`]) and asks nobody.
//!
//! A desk's page is about the file a search was asked for: a page shows it
//! only while that is the file it is about ([`Found::page_for`]) — the
//! results for the last song are not this one's.
//!
//! # A press on a row that names the host is the consent
//!
//! The row that searches says whom it asks (`player-consent-ask`), and
//! pressing it lets the player ask them — written into `player.json`, merged
//! with what is on disk — and then asks ([`consented`], `Errand::Let`). A
//! search the daemon refused because a host is not let says so, and its row
//! lets that host and asks again ([`Target::Let`]).
//!
//! # The limits are said
//!
//! Subtitles are searched only with the person's key — there is no source
//! without one today — and lyrics have one source, LRCLIB, with no language
//! and no translation: each page says its limit before its first row.

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::player::{Errand, NAME_IN_A_SENTENCE, PageId, Target, name_of, trouble_sentence};
use crate::player_rows::Builder;
use ronoctua_i18n::{Arg, key};
use ronoctua_lookup::Service;
use ronoctua_lookup::keys::Key;
use ronoctua_player::find::{
    CandidateId, Desk, LyricsCandidate, LyricsTo, Page, Query, SUBTITLE_SERVICES,
    SubtitleCandidate, SuggestedCover, TakeStatus,
};
use ronoctua_player::queue::Name;
use ronoctua_player::wire::{Ask, DeskKind, DeskSummary, State, Trouble};
use ronoctua_render::image::Rgba;
use ronoctua_tokens as tokens;

/// Where a fetch of a desk's page leaves it: the page, or `None` for a fetch
/// that came back with none — no player, no answer in time.
type Slot = Arc<Mutex<Option<Option<Desk>>>>;

/// Where a decode of a suggested cover leaves it: the picture and the key
/// its texture goes up under, or `None` for a file that is no picture.
type PictureSlot = Arc<Mutex<Option<Option<(String, Rgba)>>>>;

/// Where the asks for suggested covers leave how each ended: refused, and
/// why — or `None`, not asked at all (no player, no answer in time), to be
/// asked again.
type CoverAnswers = Arc<Mutex<Vec<(CandidateId, Option<Trouble>)>>>;

/// A picture a door draws, by the key its texture goes up under.
pub type Picture = (String, Arc<Rgba>);

/// THE DESKS OF THE LOOKUPS as this door last heard them, the fetches under
/// way, and the suggested covers decoded to be drawn.
#[derive(Debug, Default)]
pub struct Found {
    /// The page last heard of each desk.
    held: Vec<Desk>,
    /// The fetches under way: which desk, for which generation, and where
    /// the thread leaves the page.
    coming: Vec<(DeskKind, u64, Slot)>,
    /// The suggested covers decoded, by the file the daemon kept each in —
    /// `None` for one that is no picture.
    pictures: Vec<(String, Option<Picture>)>,
    /// The decodes under way.
    pictures_coming: Vec<(String, PictureSlot)>,
    /// The candidates whose cover this door asked the daemon to fetch, so it
    /// is asked once.
    asked_covers: Vec<CandidateId>,
    /// How those asks ended, from their threads.
    cover_answers: CoverAnswers,
    /// The covers the player refused to fetch, and why — said on the
    /// candidate's page in place of "fetching".
    covers_refused: Vec<(CandidateId, Trouble)>,
    /// THE DIRECTORIES' PAGES this door heard, by the list each answers —
    /// only the lists its stack shows ([`Found::keep_listings`]). A desk is
    /// every door's: another that asked for another list took it, and a page
    /// of this door still shows its own ([`Found::listing`]).
    listings: Vec<Desk>,
}

impl Found {
    /// The page last heard of `kind`'s desk.
    #[must_use]
    pub fn desk(&self, kind: DeskKind) -> Option<&Desk> {
        self.held.iter().find(|d| d.kind == kind)
    }

    /// `kind`'s page, while it is about `location` — the file a page shows:
    /// the results of a search for another file are not this one's. A
    /// directory's page, which is about no file, is never one of these.
    #[must_use]
    pub fn page_for(&self, kind: DeskKind, location: &str) -> Option<&Desk> {
        self.desk(kind).filter(|d| {
            d.query
                .as_ref()
                .is_some_and(|q| same_file(q.location(), location))
        })
    }

    /// A page HANDED to the door — a photograph's, a test's: as a door's
    /// thread would have left it, its suggested covers decoded here.
    pub fn given(&mut self, desk: Desk) {
        self.coming.retain(|(k, _, _)| *k != desk.kind);
        for path in kept_covers(&desk) {
            if !self.pictures.iter().any(|(p, _)| *p == path) {
                let picture = decoded(Path::new(&path)).map(|(key, rgba)| (key, Arc::new(rgba)));
                self.pictures.push((path, picture));
            }
        }
        self.keep(desk);
    }

    /// A suggested cover the player REFUSED to fetch, handed to the door — a
    /// photograph's, a test's — as a door's thread would have left it.
    pub fn cover_refused_given(&mut self, id: CandidateId, why: Trouble) {
        self.covers_refused.retain(|(c, _)| *c != id);
        self.covers_refused.push((id, why));
    }

    /// Why the player would not fetch the cover `id` suggests, when it
    /// refused.
    #[must_use]
    pub fn cover_refused(&self, id: CandidateId) -> Option<&Trouble> {
        self.covers_refused
            .iter()
            .find(|(c, _)| *c == id)
            .map(|(_, t)| t)
    }

    /// THE PLAYER IS GONE: every page held of it, every ask under way and
    /// every cover asked for, forgotten — the one that comes back counts its
    /// generations and its candidates from zero again.
    pub fn forget(&mut self) {
        self.held.clear();
        self.coming.clear();
        self.listings.clear();
        self.asked_covers.clear();
        self.covers_refused.clear();
        self.pictures.clear();
        self.pictures_coming.clear();
        if let Ok(mut answers) = self.cover_answers.lock() {
            answers.clear();
        }
    }

    /// A page, kept in place of the older one of its desk — and a
    /// directory's, by the list it answers, too.
    fn keep(&mut self, desk: Desk) {
        if let Some(q) = desk.query.as_ref().filter(|q| q.directory().is_some()) {
            self.listings.retain(|d| d.query.as_ref() != Some(q));
            self.listings.push(desk.clone());
        }
        match self.held.iter_mut().find(|d| d.kind == desk.kind) {
            Some(d) => *d = desk,
            None => self.held.push(desk),
        }
    }

    /// What a directory's desk said of `query`: the page the desk holds when
    /// it is that list, else the one this door kept of it.
    #[must_use]
    pub fn listing(&self, query: &Query) -> Option<&Desk> {
        self.desk(query.desk())
            .filter(|d| d.query.as_ref() == Some(query))
            .or_else(|| {
                self.listings
                    .iter()
                    .find(|d| d.query.as_ref() == Some(query))
            })
    }

    /// Only the directories' pages of `lists` kept — the ones this door's
    /// stack shows: a page left is a list nobody reads, and a stack is a few
    /// pages deep, which is the bound.
    pub fn keep_listings(&mut self, lists: &[Query]) {
        self.listings
            .retain(|d| d.query.as_ref().is_some_and(|q| lists.contains(q)));
    }

    /// What the view takes on each settle: pages and pictures that arrived
    /// and — a desk of `kinds` whose generation moved in `state` — its page
    /// asked for again, on a thread that wakes the door. `candidate`: the
    /// suggestion whose page is on screen — its cover asked to be fetched
    /// once, and decoded when it is. With no `wake` (a photograph, a test)
    /// nothing is asked of anybody.
    pub fn look(
        &mut self,
        kinds: &[DeskKind],
        candidate: Option<CandidateId>,
        state: Option<&State>,
        wake: Option<&crate::player_subtitles::Wake>,
    ) {
        // WHAT CAME BACK: a page is kept; a fetch that came back with none
        // is forgotten, and the page asked again on a later settle — never
        // held as asked for good.
        let mut done: Vec<DeskKind> = Vec::new();
        let mut arrived: Vec<Desk> = Vec::new();
        for (kind, _, slot) in &self.coming {
            if let Some(got) = slot.lock().ok().and_then(|mut s| s.take()) {
                done.push(*kind);
                arrived.extend(got);
            }
        }
        self.coming.retain(|(k, _, _)| !done.contains(k));
        for desk in arrived {
            self.keep(desk);
        }
        let decoded_now: Vec<(String, Option<(String, Rgba)>)> = self
            .pictures_coming
            .iter()
            .filter_map(|(path, slot)| Some((path.clone(), slot.lock().ok()?.take()?)))
            .collect();
        for (path, got) in decoded_now {
            self.pictures_coming.retain(|(p, _)| *p != path);
            let picture = got.map(|(key, rgba)| (key, Arc::new(rgba)));
            self.pictures.push((path, picture));
        }
        // THE PICTURES OF PAGES NO LONGER HELD go: a cover a newer search
        // replaced is a texture nobody draws.
        let still: Vec<String> = self.held.iter().flat_map(kept_covers).collect();
        self.pictures.retain(|(p, _)| still.contains(p));
        // AND THE COVERS ASKED FOR, of candidates no longer on the page.
        let on_page: Vec<CandidateId> = match self.desk(DeskKind::Suggestions).map(|d| &d.page) {
            Some(Page::Suggestions { candidates, .. }) => candidates.iter().map(|c| c.id).collect(),
            _ => Vec::new(),
        };
        self.asked_covers.retain(|id| on_page.contains(id));
        self.covers_refused.retain(|(id, _)| on_page.contains(id));
        // HOW THE ASKS FOR COVERS ENDED: a refusal is said on the page; no
        // answer at all is asked again.
        let answers: Vec<(CandidateId, Option<Trouble>)> = self
            .cover_answers
            .lock()
            .map(|mut a| std::mem::take(&mut *a))
            .unwrap_or_default();
        for (id, refused) in answers {
            match refused {
                Some(why) => self.cover_refused_given(id, why),
                None => self.asked_covers.retain(|c| *c != id),
            }
        }
        let Some(s) = state else {
            return;
        };
        let Some(wake) = wake else {
            return;
        };
        for kind in kinds {
            let Some(summary) = summary(s, *kind) else {
                continue;
            };
            let held = self.desk(*kind).map(|d| d.generation);
            let asked = self
                .coming
                .iter()
                .any(|(k, g, _)| k == kind && *g == summary.generation);
            if held == Some(summary.generation) || asked {
                continue;
            }
            self.coming.retain(|(k, _, _)| k != kind);
            let slot: Slot = Arc::default();
            self.coming.push((*kind, summary.generation, slot.clone()));
            let (kind, wake) = (*kind, wake.clone());
            let born = std::thread::Builder::new()
                .name("player-desk-door".to_owned())
                .spawn(move || {
                    // A PAGE WAKES THE DOOR; none — no player, no answer in
                    // time — is left for the next settle to forget and ask
                    // again, WITHOUT a wake: one would ask again at once, and
                    // a player that is not there would be asked in a loop.
                    let got = match ronoctua_player::client::desk(kind) {
                        Ok(ronoctua_player::wire::Say::Desk(desk)) => Some(*desk),
                        Ok(other) => {
                            tracing::warn!(?other, "a desk answered with something else");
                            None
                        }
                        Err(e) => {
                            tracing::debug!(error = %e, "no desk: the player did not answer");
                            None
                        }
                    };
                    let wakes = got.is_some();
                    if let Ok(mut s) = slot.lock() {
                        *s = Some(got);
                    }
                    if wakes {
                        wake.call();
                    }
                });
            if let Err(e) = born {
                tracing::warn!(error = %e, "could not start the thread that asks for a desk");
                self.coming.retain(|(k, _, _)| *k != kind);
            }
        }
        // THE COVER OF THE SUGGESTION ON SCREEN: fetched by the daemon once
        // — a person opened it to see it — and decoded here once it is kept.
        let Some(id) = candidate else {
            return;
        };
        let Some((seen, suggests_one)) =
            self.desk(DeskKind::Suggestions)
                .and_then(|d| match &d.page {
                    Page::Suggestions { candidates, .. } => candidates
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| (c.seen.clone(), c.suggestion.cover.is_some())),
                    _ => None,
                })
        else {
            return;
        };
        match &seen {
            None if suggests_one
                && !self.asked_covers.contains(&id)
                && self.cover_refused(id).is_none() =>
            {
                self.asked_covers.push(id);
                let (wake, answers) = (wake.clone(), self.cover_answers.clone());
                let born = std::thread::Builder::new()
                    .name("player-cover-door".to_owned())
                    .spawn(move || {
                        // A REFUSAL is the page's to say; no answer at all is
                        // asked again on a later settle — with no wake, or a
                        // player that is not there would be asked in a loop.
                        let (answer, wakes) = match ronoctua_player::client::suggested_cover(id) {
                            Ok(ronoctua_player::wire::Say::Refused(t)) => (Some(Some(t)), true),
                            Ok(_) => (None, false),
                            Err(e) => {
                                tracing::debug!(error = %e, "the suggested cover was not asked");
                                (Some(None), false)
                            }
                        };
                        if let Some(answer) = answer
                            && let Ok(mut a) = answers.lock()
                        {
                            a.push((id, answer));
                        }
                        if wakes {
                            wake.call();
                        }
                    });
                if let Err(e) = born {
                    tracing::warn!(error = %e, "could not start the thread that asks for a cover");
                }
            }
            Some(SuggestedCover::Kept { path })
                if !self.pictures.iter().any(|(p, _)| p == path)
                    && !self.pictures_coming.iter().any(|(p, _)| p == path) =>
            {
                let slot: PictureSlot = Arc::default();
                self.pictures_coming.push((path.clone(), slot.clone()));
                let (path, wake) = (path.clone(), wake.clone());
                let born = std::thread::Builder::new()
                    .name("player-cover-decode".to_owned())
                    .spawn(move || {
                        let got = decoded(Path::new(&path));
                        if let Ok(mut s) = slot.lock() {
                            *s = Some(got);
                        }
                        wake.call();
                    });
                if let Err(e) = born {
                    tracing::warn!(error = %e, "could not start the thread that decodes a cover");
                }
            }
            _ => {}
        }
    }

    /// The suggested cover kept in `path`, decoded: its texture's key and its
    /// pixels.
    #[must_use]
    pub fn picture(&self, path: &str) -> Option<&Picture> {
        self.pictures
            .iter()
            .find(|(p, _)| p == path)
            .and_then(|(_, picture)| picture.as_ref())
    }

    /// A picture this view holds under `key`, when it is a suggested cover.
    #[must_use]
    pub fn picture_by_key(&self, key: &str) -> Option<&Picture> {
        self.pictures
            .iter()
            .filter_map(|(_, picture)| picture.as_ref())
            .find(|(k, _)| k == key)
    }
}

/// Every suggested cover a page holds kept in a file.
fn kept_covers(desk: &Desk) -> Vec<String> {
    match &desk.page {
        Page::Suggestions { candidates, .. } => candidates
            .iter()
            .filter_map(|c| match &c.seen {
                Some(SuggestedCover::Kept { path }) => Some(path.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The key a suggested cover's texture goes up under: the file the daemon
/// kept it in AND ITS STAMP — its time and its length. The file is named by
/// the candidate's number (`preview-<n>`), and a player started again numbers
/// from one again: a door left open would show the last session's cover
/// beside a new suggestion, since a texture is never taken down by its key.
fn picture_key(file: &Path) -> String {
    let stamp = std::fs::metadata(file).ok().map_or((0, 0), |m| {
        let nanos = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        (nanos, m.len())
    });
    format!(
        "player:suggested:{}:{}:{}",
        file.display(),
        stamp.0,
        stamp.1
    )
}

/// A picture file, decoded and made no larger than `space.player.coverPx`,
/// with the key its texture goes up under ([`picture_key`]). Reads the disk:
/// a thread's, or a door's with no loop.
fn decoded(file: &Path) -> Option<(String, Rgba)> {
    let key = picture_key(file);
    decoded_pixels(file).map(|rgba| (key, rgba))
}

/// A picture file's pixels, made no larger than `space.player.coverPx`.
fn decoded_pixels(file: &Path) -> Option<Rgba> {
    let picture = image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .map_err(|e| {
            tracing::info!(file = %file.display(), error = %e, "a suggested cover that is no picture");
        })
        .ok()?;
    let most = u32::from(tokens::space::PLAYER_COVERPX);
    let picture = if picture.width() > most || picture.height() > most {
        picture.thumbnail(most, most)
    } else {
        picture
    };
    let rgba = picture.to_rgba8();
    let (w, h) = rgba.dimensions();
    Rgba::new(w, h, rgba.into_raw())
}

/// Whether two locations name the same file: equal, or the same path
/// written two ways. Pure: nothing on disk is read.
fn same_file(a: &str, b: &str) -> bool {
    a == b || (!a.contains("://") && Path::new(a) == Path::new(b))
}

/// `kind`'s summary in the state.
#[must_use]
pub fn summary(s: &State, kind: DeskKind) -> Option<&DeskSummary> {
    s.desks.iter().find(|d| d.kind == kind)
}

/// The hosts `services` reach first, in one line — what a row that asks
/// them names — joined as the language joins a list.
#[must_use]
pub fn hosts_of(services: &[Service], texts: &ronoctua_i18n::Texts) -> String {
    services
        .iter()
        .flat_map(|s| s.hosts().iter().map(|h| h.name()))
        .collect::<Vec<_>>()
        .join(&texts.t(key::PLAYER_LIST_JOINER))
}

/// The languages a search is in, when the person chose none: the
/// interface's, then English (`tongues::default_for`).
#[must_use]
pub fn default_tongues(texts: &ronoctua_i18n::Texts) -> Vec<String> {
    ronoctua_player::tongues::default_for(texts.language())
        .iter()
        .map(|t| t.code.to_owned())
        .collect()
}

/// A language, by its name in the person's language — never a raw code;
/// one this table does not know is said as the code the service gave (data).
#[must_use]
pub fn tongue_name(code: &str, texts: &ronoctua_i18n::Texts) -> String {
    ronoctua_player::tongues::name_key(code).map_or_else(|| code.to_owned(), |key| texts.t(key))
}

/// What the view chose for its searches: the source of the suggestions, the
/// languages of the subtitles, the film they are for, where lyrics go, and
/// which of a suggestion's fields are to be written.
#[derive(Debug, Default)]
pub struct Choices {
    /// Who suggests the tags — `None`: the file's kind says (a song asks
    /// MusicBrainz, a film Wikidata).
    pub source: Option<ronoctua_player::find::Suggester>,
    /// The languages subtitles are searched in — `None`: the interface's,
    /// then English.
    pub tongues: Option<Vec<String>>,
    /// Which film of what plays the subtitles are for: its place among
    /// [`films`].
    pub film: usize,
    /// Where lyrics taken go.
    pub lyrics_to: LyricsTo,
    /// A suggestion's fields the person turned, by candidate: whether each is
    /// to be written. A field not here is as it started — on for a blank.
    pub imports: std::collections::HashMap<(CandidateId, ImportOf), bool>,
}

/// What of a suggestion is taken: one of its fields, or its cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportOf {
    /// A field.
    Field(ronoctua_tags::Field),
    /// The cover.
    Cover,
}

impl Choices {
    /// The languages subtitles are searched in.
    #[must_use]
    pub fn tongues(&self, texts: &ronoctua_i18n::Texts) -> Vec<String> {
        self.tongues
            .clone()
            .unwrap_or_else(|| default_tongues(texts))
    }
}

/// The films of what plays, each once, in the player's order — what the
/// subtitles' file ring walks: the sources of its picture lanes.
#[must_use]
pub fn films(s: &State) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for lane in &s.lanes {
        if lane.kind == ronoctua_player::own::LaneKind::Video && !out.contains(&lane.source) {
            out.push(lane.source.clone());
        }
    }
    out
}

/// The film the subtitles are searched for, in `s`.
#[must_use]
pub fn film(choices: &Choices, s: &State) -> Option<String> {
    let all = films(s);
    if all.is_empty() {
        return None;
    }
    all.get(choices.film.min(all.len() - 1)).cloned()
}

/// A file's name, as a sentence says it.
fn named(location: &str, texts: &ronoctua_i18n::Texts) -> String {
    name_of(
        &Name::Location(location.to_owned()),
        texts,
        NAME_IN_A_SENTENCE,
    )
}

/// A sentence already in words, wrapped in the note's register.
pub(crate) fn note_said(said: &str, b: &mut Builder<'_>) {
    for line in crate::chrome::note_lines(said, b.room) {
        b.inert(vec![Builder::piece(b.left, line, tokens::color::TEXT_DIM)]);
    }
}

/// Whether the person let the player ask every one of `services` — the
/// view's copy of `player.json`.
#[must_use]
pub fn allowed(prefs: &ronoctua_player::prefs::Preferences, services: &[Service]) -> bool {
    services.iter().all(|s| prefs.allows(*s))
}

/// What a press on a row that names `services` does: `ask`, after the
/// services are let — the press IS the consent — written merged with what
/// is on disk, and the view's copy of the preferences kept in step.
pub(crate) fn consented(
    prefs: &mut ronoctua_player::prefs::Preferences,
    services: &[Service],
    ask: Errand,
) -> Errand {
    if allowed(prefs, services) {
        return ask;
    }
    for s in services {
        prefs.consent(*s);
    }
    Errand::Let {
        services: services.to_vec(),
        directories: Vec::new(),
        on: true,
        then: Box::new(ask),
    }
}

/// The desk's STATUS, under the rows that ask it: what the daemon is doing,
/// why the last search gave nothing — a host not let is a row that lets it
/// and asks again — each service that gave nothing while another answered,
/// and what the search went out with.
pub(crate) fn status(
    s: &State,
    kind: DeskKind,
    desk: Option<&Desk>,
    hosts: &str,
    b: &mut Builder<'_>,
) {
    let now = summary(s, kind);
    let busy = now.is_some_and(|d| d.busy);
    if busy {
        let said = b.texts.t_with(key::PLAYER_DESK_ASKING, &[("hosts", hosts)]);
        note_said(&said, b);
    }
    let Some(desk) = desk else {
        return;
    };
    // WHAT THE LAST SEARCH WAS REFUSED, while a new one is under way, is the
    // last search's: "Allow {host} and ask again" beside "Asking…" was the
    // question already answered.
    if !busy {
        if let Some(t) = &desk.trouble {
            trouble_row(t, desk, b);
        }
        for t in &desk.refused {
            trouble_row(t, desk, b);
        }
    }
    if let Some(asked) = &desk.asked
        && !now.is_some_and(|d| d.busy)
    {
        if !asked.words.trim().is_empty() {
            let said = b.texts.t_with(
                key::PLAYER_DESK_SEARCHED_FOR,
                &[("words", asked.words.trim())],
            );
            note_said(&said, b);
        }
        if asked.fingerprint {
            b.note(key::PLAYER_DESK_BY_FINGERPRINT);
        }
        if asked.by_id {
            b.note(key::PLAYER_DESK_BY_ID);
        }
    }
}

/// A trouble of a desk, in its sentence — and, for a host not let, the row
/// that lets it and asks the desk's search again.
fn trouble_row(t: &Trouble, desk: &Desk, b: &mut Builder<'_>) {
    b.warning(&trouble_sentence(t, b.texts));
    if let (Trouble::Off { host }, Some(_)) = (t, desk.query.as_ref()) {
        b.pair(
            b.texts.t_with(key::PLAYER_CONSENT_LET, &[("host", host)]),
            String::new(),
            Target::Let(host.clone()),
            &format!("let-{host}"),
        );
    }
}

/// The row that asks: its label names the hosts, and pressing it is the
/// consent — its value says whether they are let already.
pub(crate) fn ask_row(
    prefs: &ronoctua_player::prefs::Preferences,
    services: &[Service],
    kind: DeskKind,
    b: &mut Builder<'_>,
) {
    // THE HOSTS IN THE LABEL, which has the row to itself; and, under it,
    // that pressing it lets them be asked — while it does.
    let under = if allowed(prefs, services) {
        String::new()
    } else {
        b.t(key::PLAYER_CONSENT_FIRST)
    };
    b.pair_under(
        b.texts.t_with(
            key::PLAYER_CONSENT_ASK,
            &[("hosts", &hosts_of(services, b.texts))],
        ),
        String::new(),
        &under,
        Target::Search(kind),
        &format!("search-{}", kind_word(kind)),
    );
    b.pair(
        b.t(key::PLAYER_SUGGEST_OTHER_WORDS),
        String::new(),
        Target::Words(kind),
        &format!("words-{}", kind_word(kind)),
    );
}

/// A desk's word in a row's spot.
const fn kind_word(kind: DeskKind) -> &'static str {
    match kind {
        DeskKind::Suggestions => "suggestions",
        DeskKind::Subtitles => "subtitles",
        DeskKind::Lyrics => "lyrics",
        DeskKind::Radio => "radio",
        DeskKind::Television => "television",
    }
}

/// How the last take of `id` goes, on its page: under way, handed to the
/// writer, saved — named — or refused in its sentence. Nothing when the
/// desk's last take is another candidate's.
fn taken_line(desk: &Desk, id: CandidateId, b: &mut Builder<'_>) {
    let Some(taken) = desk.taken.as_ref().filter(|t| t.candidate == id) else {
        return;
    };
    match &taken.status {
        TakeStatus::Taking => b.note(key::PLAYER_TAKE_TAKING),
        TakeStatus::Handed => b.note(key::PLAYER_TAKE_HANDED),
        TakeStatus::Saved { path } => {
            let said = b
                .texts
                .t_with(key::PLAYER_TAKE_SAVED, &[("name", &named(path, b.texts))]);
            note_said(&said, b);
        }
        TakeStatus::Refused(t) => b.warning(&trouble_sentence(t, b.texts)),
    }
}

/// The subtitle services the person kept a key for — the ones a search can
/// ask.
#[must_use]
pub fn keyed_subtitle_services(kept: &crate::player_internet::Kept) -> Vec<Service> {
    SUBTITLE_SERVICES
        .iter()
        .copied()
        .filter(|s| s.key().is_some_and(|k| kept.has(k)))
        .collect()
}

/// THE FIND-SUBTITLES PAGE: its limit, the film (a ring while several play),
/// the languages — each by its name, a glyph at its end taking it out — and
/// a row to add one, the row that asks the services the person kept a key
/// for, and what they found: by language, the ones timed for this very file
/// first.
pub(crate) fn subtitles_page(v: &crate::player::View, s: &State, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_FIND_SUBTITLES);
    // NO KEY, NO SEARCH — said first, with the row to where a key goes.
    let services = keyed_subtitle_services(&v.kept);
    if services.is_empty() {
        b.warning(&b.t(key::PLAYER_FIND_NEEDS_A_KEY));
        let internet = PageId::Internet;
        b.pair(
            b.t(internet.key()),
            String::new(),
            Target::Go(internet),
            internet.spot(),
        );
    }
    let Some(film) = film(&v.finding, s) else {
        return;
    };
    if films(s).len() > 1 {
        b.pair(
            b.t(key::PLAYER_META_FILE),
            named(&film, b.texts),
            Target::FindFilm,
            "find-film",
        );
    }
    b.heading(key::PLAYER_FIND_LANGUAGES);
    // EACH LANGUAGE BY ITS NAME, and at its end the glyph that takes it out
    // — never on the last, which stays: a search is in some language. The
    // row itself only says so; `d` takes it out, as the glyph does.
    let tongues = v.finding.tongues(b.texts);
    let last = tongues.len() < 2;
    for code in tongues {
        b.pair_with_control(
            tongue_name(&code, b.texts),
            String::new(),
            Target::Language(code.clone()),
            &format!("language-{code}"),
            (!last).then(|| {
                (
                    tokens::icon::PLAYER_LANGUAGE_REMOVE,
                    Target::LanguageOut(code.clone()),
                )
            }),
        );
    }
    b.pair(
        b.t(PageId::Languages.key()),
        String::new(),
        Target::Go(PageId::Languages),
        PageId::Languages.spot(),
    );
    if services.is_empty() {
        return;
    }
    b.heading(key::PLAYER_HEADING_RESULTS);
    ask_row(&v.prefs, &services, DeskKind::Subtitles, b);
    let desk = v.found.page_for(DeskKind::Subtitles, &film);
    status(
        s,
        DeskKind::Subtitles,
        desk,
        &hosts_of(&services, b.texts),
        b,
    );
    let Some(desk) = desk else {
        return;
    };
    let Page::Subtitles {
        candidates,
        remaining_downloads,
        ..
    } = &desk.page
    else {
        return;
    };
    if let Some(left) = remaining_downloads {
        b.pair(
            b.t(key::PLAYER_FIND_DOWNLOADS_LEFT),
            b.texts
                .t_args(key::PLAYER_META_COUNT, &[("count", Arg::Number(*left))]),
            Target::Nothing,
            "downloads-left",
        );
    }
    if candidates.is_empty()
        && desk.trouble.is_none()
        && !summary(s, DeskKind::Subtitles).is_some_and(|d| d.busy)
    {
        b.note(key::PLAYER_DESK_NOTHING_FOUND);
    }
    // BY LANGUAGE, the ones timed for this very file first, then the most
    // downloaded — the daemon's order within each.
    let mut tongues: Vec<Option<&str>> = Vec::new();
    for c in candidates {
        let t = c.tongue.as_deref();
        if !tongues.contains(&t) {
            tongues.push(t);
        }
    }
    for tongue in tongues {
        let heading = tongue.map_or_else(
            || {
                candidates
                    .iter()
                    .find(|c| c.tongue.is_none())
                    .map(|c| c.found.language.clone())
                    .unwrap_or_default()
            },
            |t| tongue_name(t, b.texts),
        );
        b.heading_within(heading);
        let mut these: Vec<&SubtitleCandidate> = candidates
            .iter()
            .filter(|c| c.tongue.as_deref() == tongue)
            .collect();
        these.sort_by_key(|c| std::cmp::Reverse(c.found.hash_match));
        for c in these {
            subtitle_row(c, b);
            b.nested(|b| taken_line(desk, c.id, b));
        }
    }
}

/// One subtitle found: its release's name, the glyph of one timed for this
/// file, and what the service says of it.
fn subtitle_row(c: &SubtitleCandidate, b: &mut Builder<'_>) {
    let f = &c.found;
    let name = if f.release.trim().is_empty() {
        f.file_name.clone().unwrap_or_default()
    } else {
        f.release.trim().to_owned()
    };
    let mut said: Vec<String> = Vec::new();
    if f.hash_match {
        said.push(b.t(key::PLAYER_FIND_HASH_MATCH));
    }
    if let Some(n) = f.downloads {
        said.push(b.texts.t_args(
            key::PLAYER_FIND_DOWNLOADS,
            &[("count", Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)))],
        ));
    }
    if f.hearing_impaired {
        said.push(b.t(key::PLAYER_FIND_HEARING));
    }
    if f.machine_translated {
        said.push(b.t(key::PLAYER_FIND_MACHINE));
    }
    said.push(b.t(crate::player::service_key(f.source)));
    let said = said.join(&b.t(key::PLAYER_META_JOINER));
    let spot = format!("subtitle-{}", c.id.0);
    // THE RELEASE'S NAME has the row to itself — one word as long as it is —
    // and what the service says of it goes under it.
    if f.hash_match {
        b.marked(tokens::icon::PLAYER_HASH_MATCH, |b| {
            b.pair_under(
                name,
                String::new(),
                &said,
                Target::TakeSubtitle(c.id),
                &spot,
            );
        });
    } else {
        b.pair_under(
            name,
            String::new(),
            &said,
            Target::TakeSubtitle(c.id),
            &spot,
        );
    }
}

/// THE FIND-LYRICS PAGE: its limit, the file (the Metadata's ring), where
/// what is taken goes, the row that asks LRCLIB, and what it found — synced
/// or words only or instrumental, and how far each is from the song's
/// length.
pub(crate) fn lyrics_page(v: &crate::player::View, s: &State, b: &mut Builder<'_>) {
    b.note(key::PLAYER_LYRICS_ONE_SOURCE);
    let Some(song) = v.meta.location(s) else {
        return;
    };
    if crate::player_metadata::locations(s).len() > 1 {
        b.pair(
            b.t(key::PLAYER_META_FILE),
            named(&song, b.texts),
            Target::MetaFile,
            "meta-file",
        );
    }
    b.pair_whole(
        b.t(key::PLAYER_LYRICS_TO),
        b.t(match v.finding.lyrics_to {
            LyricsTo::Beside => key::PLAYER_LYRICS_TO_BESIDE,
            LyricsTo::Tag => key::PLAYER_LYRICS_TO_TAG,
        }),
        Target::LyricsTo,
        "lyrics-to",
    );
    b.heading(key::PLAYER_HEADING_RESULTS);
    let services = [Service::Lrclib];
    ask_row(&v.prefs, &services, DeskKind::Lyrics, b);
    let desk = v.found.page_for(DeskKind::Lyrics, &song);
    status(s, DeskKind::Lyrics, desk, &hosts_of(&services, b.texts), b);
    let Some(desk) = desk else {
        return;
    };
    let Page::Lyrics { candidates, .. } = &desk.page else {
        return;
    };
    if candidates.is_empty()
        && desk.trouble.is_none()
        && !summary(s, DeskKind::Lyrics).is_some_and(|d| d.busy)
    {
        b.note(key::PLAYER_DESK_NOTHING_FOUND);
    }
    for c in candidates {
        lyrics_row(c, b);
        b.nested(|b| taken_line(desk, c.id, b));
    }
}

/// One record of lyrics: the track and its artist (data), and what it is.
fn lyrics_row(c: &LyricsCandidate, b: &mut Builder<'_>) {
    let r = &c.record;
    let name = [r.track.trim(), r.artist.trim()]
        .into_iter()
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(&b.t(key::PLAYER_META_JOINER));
    let mut said: Vec<String> = vec![b.t(if r.instrumental {
        key::PLAYER_FIND_INSTRUMENTAL
    } else if r.synced.as_deref().is_some_and(|t| !t.trim().is_empty()) {
        key::PLAYER_FIND_SYNCED
    } else {
        key::PLAYER_FIND_PLAIN
    })];
    if let Some(off) = c.off_by_seconds {
        #[allow(clippy::cast_possible_truncation)]
        let seconds = off.abs().round() as i64;
        said.push(if seconds == 0 {
            b.t(key::PLAYER_FIND_SAME_LENGTH)
        } else {
            b.texts.t_args(
                key::PLAYER_FIND_OFF_BY,
                &[("seconds", Arg::Number(seconds))],
            )
        });
    }
    if let Some(album) = r.album.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        said.push(album.to_owned());
    }
    b.pair_under(
        name,
        String::new(),
        &said.join(&b.t(key::PLAYER_META_JOINER)),
        Target::TakeLyrics(c.id),
        &format!("lyrics-{}", c.id.0),
    );
}

/// What a press on a row of the lookups does — `None` for a target that is
/// none of theirs.
#[allow(clippy::too_many_lines)]
pub(crate) fn turn(
    v: &mut crate::player::View,
    target: &Target,
    press: crate::player::Press,
    s: &State,
    texts: &ronoctua_i18n::Texts,
) -> Option<crate::player_metadata::Turned> {
    use crate::player::Press;
    use crate::player_metadata::Turned;
    let forward = !matches!(press, Press::Less);
    match (target, press) {
        // THE SEARCH: the consent, then the ask.
        (Target::Search(kind), Press::Enter) => {
            let query = v.query_of(*kind, None, s, texts)?;
            Some(Turned::Errand(v.asking(query, texts)))
        }
        (Target::Words(kind), Press::Enter) => {
            let held = v
                .found
                .desk(*kind)
                .and_then(|d| d.asked.as_ref())
                .map(|a| a.words.trim().to_owned())
                .filter(|w| !w.is_empty());
            let question = held.map_or_else(
                || {
                    crate::player_question::Question::plain(
                        crate::player_question::FIND_WORDS,
                        false,
                    )
                },
                |w| {
                    crate::player_question::Question::holding(crate::player_question::FIND_WORDS, w)
                },
            );
            Some(Turned::Asked(crate::player::Typing::Words(*kind), question))
        }
        // A HOST NOT LET, said under a search: let, and asked again.
        (Target::Let(host), Press::Enter) => {
            let query = [DeskKind::Suggestions, DeskKind::Subtitles, DeskKind::Lyrics]
                .into_iter()
                .filter_map(|k| v.found.desk(k))
                .find(|d| {
                    d.trouble
                        .iter()
                        .chain(&d.refused)
                        .any(|t| matches!(t, Trouble::Off { host: h } if h == host))
                })
                .and_then(|d| d.query.clone())?;
            let service = ronoctua_player::prefs::service_of_host(host)?;
            v.prefs.consent(service);
            let hosts = hosts_of(&v.services_of(&query), texts);
            Some(Turned::Errand(Errand::Let {
                services: vec![service],
                directories: Vec::new(),
                on: true,
                then: Box::new(Errand::TellSaying(
                    Ask::Find { query },
                    texts.t_with(key::PLAYER_DESK_ASKING, &[("hosts", &hosts)]),
                )),
            }))
        }
        (Target::FindFilm, Press::Enter | Press::Less | Press::More) => {
            let n = films(s).len().max(1);
            let now = v.finding.film.min(n - 1);
            v.finding.film = if forward {
                (now + 1) % n
            } else {
                (now + n - 1) % n
            };
            Some(Turned::Errand(Errand::Nothing))
        }
        (Target::LyricsTo, Press::Enter | Press::Less | Press::More) => {
            v.finding.lyrics_to = match v.finding.lyrics_to {
                LyricsTo::Beside => LyricsTo::Tag,
                LyricsTo::Tag => LyricsTo::Beside,
            };
            Some(Turned::Errand(Errand::Nothing))
        }
        // A LANGUAGE'S ROW says what it is and how it goes: a press is no
        // way to take one out by accident.
        (Target::Language(code), Press::Enter) => Some(Turned::Errand(Errand::Said(texts.t_with(
            key::PLAYER_FIND_LANGUAGE_SEARCHED,
            &[("language", &tongue_name(code, texts))],
        )))),
        // A LANGUAGE taken out of the search — its glyph, or `d` on its row —
        // never the last.
        (Target::Language(code), Press::Remove) | (Target::LanguageOut(code), Press::Enter) => {
            Some(Turned::Errand(language_out(&mut v.finding, code, texts)))
        }
        // THE LANGUAGES' PAGE: a row a language, each on or off.
        (Target::LanguagePick(code), Press::Enter | Press::Less | Press::More) => {
            let on = v.finding.tongues(texts).iter().any(|c| c == code);
            Some(Turned::Errand(if on {
                language_out(&mut v.finding, code, texts)
            } else {
                language_in(&mut v.finding, code, texts)
            }))
        }
        (Target::AddLanguage, Press::Enter) => Some(Turned::Asked(
            crate::player::Typing::Language,
            crate::player_question::Question::plain(crate::player_question::FIND_LANGUAGE, false),
        )),
        // A SUBTITLE CHOSEN: downloaded beside the film, and laid over it.
        (Target::TakeSubtitle(id), Press::Enter) => {
            let desk = v.found.desk(DeskKind::Subtitles)?;
            let Page::Subtitles { candidates, .. } = &desk.page else {
                return None;
            };
            let c = candidates.iter().find(|c| c.id == *id)?;
            let name = if c.found.release.trim().is_empty() {
                c.found.file_name.clone().unwrap_or_default()
            } else {
                c.found.release.trim().to_owned()
            };
            Some(Turned::Errand(Errand::TellSaying(
                take(DeskKind::Subtitles, *id),
                texts.t_with(key::PLAYER_FIND_TAKING, &[("name", &name)]),
            )))
        }
        // LYRICS CHOSEN: where the ring says — beside the song, asked first
        // when a `.lrc` is there already ("no" first), or into its tag.
        (Target::TakeLyrics(id), Press::Enter) => {
            let song = v.meta.location(s)?;
            let name = named(&song, texts);
            let to = v.finding.lyrics_to;
            let ask = Ask::Take {
                desk: DeskKind::Lyrics,
                candidate: *id,
                fields: Vec::new(),
                cover: false,
                to,
                replace: false,
            };
            match to {
                LyricsTo::Tag => Some(Turned::Errand(Errand::TellSaying(
                    ask,
                    texts.t_with(key::PLAYER_LYRICS_INTO_TAG, &[("name", &name)]),
                ))),
                LyricsTo::Beside => {
                    let lrc = (!song.contains("://"))
                        .then(|| Path::new(&song).with_extension("lrc"))
                        .filter(|p| p.exists());
                    let saving = texts.t_with(key::PLAYER_LYRICS_TAKING, &[("name", &name)]);
                    match lrc {
                        Some(lrc) => {
                            let file = named(&lrc.to_string_lossy(), texts);
                            Some(Turned::Question(
                                texts.t_with(key::PLAYER_ASK_REPLACE_LRC, &[("name", &file)]),
                                Errand::TellSaying(
                                    Ask::Take {
                                        desk: DeskKind::Lyrics,
                                        candidate: *id,
                                        fields: Vec::new(),
                                        cover: false,
                                        to,
                                        replace: true,
                                    },
                                    saving,
                                ),
                            ))
                        }
                        None => Some(Turned::Errand(Errand::TellSaying(ask, saving))),
                    }
                }
            }
        }
        _ => None,
    }
}

/// A take of a candidate that needs nothing but itself.
fn take(desk: DeskKind, candidate: CandidateId) -> Ask {
    Ask::Take {
        desk,
        candidate,
        fields: Vec::new(),
        cover: false,
        to: LyricsTo::Beside,
        replace: false,
    }
}

/// `code` taken out of the languages subtitles are searched in — never the
/// last: a search is in some language.
fn language_out(choices: &mut Choices, code: &str, texts: &ronoctua_i18n::Texts) -> Errand {
    let mut all = choices.tongues(texts);
    if all.len() < 2 {
        return Errand::Said(texts.t(key::PLAYER_FIND_LANGUAGE_LAST));
    }
    all.retain(|c| c != code);
    choices.tongues = Some(all);
    Errand::Said(texts.t_with(
        key::PLAYER_FIND_LANGUAGE_REMOVED,
        &[("language", &tongue_name(code, texts))],
    ))
}

/// `code` — one of the table's — added to the languages subtitles are
/// searched in, named, never by its code.
fn language_in(choices: &mut Choices, code: &str, texts: &ronoctua_i18n::Texts) -> Errand {
    let Some(tongue) = ronoctua_player::tongues::of(code) else {
        return Errand::Said(texts.t_with(key::PLAYER_FIND_UNKNOWN_LANGUAGE, &[("code", code)]));
    };
    let name = texts.t(tongue.key);
    let mut all = choices.tongues(texts);
    if all.iter().any(|c| c == tongue.code) {
        return Errand::Said(texts.t_with(key::PLAYER_FIND_LANGUAGE_THERE, &[("language", &name)]));
    }
    all.push(tongue.code.to_owned());
    choices.tongues = Some(all);
    Errand::Said(texts.t_with(key::PLAYER_FIND_LANGUAGE_ADDED, &[("language", &name)]))
}

/// A language typed to search subtitles in, on the Languages' page: its code
/// (`pt-BR`, `ja`) or its name in the person's language (`japonês`, any
/// case) — added when this system knows it; else a sentence says why.
/// Nothing typed is never mind.
pub(crate) fn language_typed(
    choices: &mut Choices,
    said: &str,
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    let said = said.trim();
    if said.is_empty() {
        return Errand::Nothing;
    }
    let by_name = ronoctua_player::tongues::TONGUES
        .iter()
        .find(|t| texts.t(t.key).to_lowercase() == said.to_lowercase());
    match by_name.or_else(|| ronoctua_player::tongues::of(said)) {
        Some(tongue) => language_in(choices, tongue.code, texts),
        None => Errand::Said(texts.t_with(key::PLAYER_FIND_UNKNOWN_LANGUAGE, &[("code", said)])),
    }
}

/// THE LANGUAGES' PAGE: every language subtitles can be searched in — the
/// table's, each by its name in the person's language, in that order — a
/// row each, on or off; and a row to type one, by its code or its name.
pub(crate) fn languages_page(v: &crate::player::View, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_LANGUAGES);
    b.pair(
        b.t(key::PLAYER_FIND_TYPE_LANGUAGE),
        String::new(),
        Target::AddLanguage,
        "type-language",
    );
    let chosen = v.finding.tongues(b.texts);
    let mut all: Vec<(String, &'static str)> = ronoctua_player::tongues::TONGUES
        .iter()
        .map(|t| (b.texts.t(t.key), t.code))
        .collect();
    all.sort_by_key(|(name, _)| name.to_lowercase());
    for (name, code) in all {
        let on = chosen.iter().any(|c| c == code);
        b.pair(
            name,
            crate::player_rows::on_off(on, b.texts),
            Target::LanguagePick(code.to_owned()),
            &format!("pick-language-{code}"),
        );
    }
}

/// Whether `id` is one of the lookups' questions on a door's line — the
/// footer hands their answers to the view.
#[must_use]
pub fn asks(id: &str) -> bool {
    use crate::player_question as q;
    [
        q::FIND_WORDS,
        q::FIND_LANGUAGE,
        q::KEY_OPENSUBTITLES,
        q::KEY_TMDB,
        q::KEY_SUBDL,
        q::ACCOUNT_USER,
        q::ACCOUNT_PASSWORD,
    ]
    .contains(&id)
}

/// The question a key is asked with, masked.
#[must_use]
pub const fn key_question(key: Key) -> &'static str {
    use crate::player_question as q;
    match key {
        Key::OpenSubtitles => q::KEY_OPENSUBTITLES,
        Key::OpenSubtitlesAccount => q::ACCOUNT_USER,
        Key::Tmdb => q::KEY_TMDB,
        Key::SubDl => q::KEY_SUBDL,
        // THE SECURITY CENTRE'S KEY is asked on its own page, never here:
        // the player lists only its own keys (`Key::scope`).
        Key::VirusTotal => crate::security::VT_KEY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::fixture::{alone, track};
    use crate::player::{Press, View};
    use ronoctua_lookup::found::{Found as FoundOne, Take};
    use ronoctua_lookup::lrclib::Record;
    use ronoctua_player::find::{Asked, Query, Taken};
    use ronoctua_player::wire::TrackKind as Kind;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect(language)
    }

    /// Every word on the page, a wrapped sentence whole again.
    fn said(rows: &[crate::player_rows::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn row_of(rows: &[crate::player_rows::Row], target: &Target) -> Option<usize> {
        rows.iter().position(|r| &r.target == target)
    }

    /// A film that plays.
    fn a_film(location: &str) -> State {
        alone(
            location,
            vec![
                track(1, Kind::Video, None, true),
                track(1, Kind::Audio, None, true),
            ],
        )
    }

    fn found(
        id: u64,
        language: &str,
        hash: bool,
        downloads: u64,
        release: &str,
    ) -> SubtitleCandidate {
        SubtitleCandidate {
            id: CandidateId(id),
            found: FoundOne {
                source: Service::OpenSubtitles,
                take: Take::OpenSubtitles { file_id: id },
                language: language.into(),
                release: release.into(),
                file_name: None,
                downloads: Some(downloads),
                hearing_impaired: id == 3,
                fps: None,
                hash_match: hash,
                machine_translated: false,
                title: None,
                year: None,
                season: None,
                episode: None,
            },
            tongue: ronoctua_player::tongues::of(language).map(|t| t.code.to_owned()),
        }
    }

    /// The subtitles desk as the daemon answers it for `film`: two in
    /// Portuguese of Brazil — the one timed for this file second in the
    /// daemon's list — and one in English.
    pub(crate) fn subtitles_desk(film: &str) -> Desk {
        Desk {
            generation: 3,
            query: Some(Query::Subtitles {
                location: film.into(),
                tongues: vec!["pt-BR".into(), "en".into()],
                words: None,
            }),
            asked: Some(Asked {
                words: "harbour".into(),
                fingerprint: true,
                ..Asked::default()
            }),
            page: Page::Subtitles {
                location: film.into(),
                tongues: vec!["pt-BR".into(), "en".into()],
                candidates: vec![
                    found(1, "pt-br", false, 900, "Harbour.2024.WEB"),
                    found(2, "pt-br", true, 12, "Harbour.2024.1080p.BluRay"),
                    found(3, "en", false, 4000, "Harbour.2024.SDH"),
                ],
                remaining_downloads: Some(18),
            },
            ..Desk::empty(DeskKind::Subtitles)
        }
    }

    /// FAILS IF: a desk's fetch that came back with no page — no player, no
    /// answer in time — is held as asked for good (its generation never asked
    /// again), or a suggested cover's ask that got no answer is never asked
    /// again, or one refused is not kept to be said; or the player gone
    /// leaves a page of its held.
    #[test]
    fn what_came_back_with_nothing_is_asked_again() {
        let mut f = Found::default();
        let slot: Slot = Arc::default();
        f.coming.push((DeskKind::Subtitles, 3, slot.clone()));
        *slot.lock().expect("a slot") = Some(None);
        f.look(&[], None, None, None);
        assert!(f.coming.is_empty(), "a fetch that failed is held as asked");
        let slot: Slot = Arc::default();
        f.coming.push((DeskKind::Subtitles, 3, slot.clone()));
        *slot.lock().expect("a slot") = Some(Some(subtitles_desk("/f.mkv")));
        f.look(&[], None, None, None);
        assert!(f.coming.is_empty());
        assert_eq!(f.desk(DeskKind::Subtitles).map(|d| d.generation), Some(3));
        // THE COVERS: none answered is asked again; one refused is said.
        f.asked_covers = vec![CandidateId(1), CandidateId(2)];
        f.cover_answers.lock().expect("answers").extend([
            (CandidateId(1), None),
            (
                CandidateId(2),
                Some(Trouble::NoSuchCandidate {
                    candidate: CandidateId(2),
                }),
            ),
        ]);
        f.look(&[], None, None, None);
        assert!(
            !f.asked_covers.contains(&CandidateId(1)),
            "never asked again"
        );
        assert!(
            f.cover_refused(CandidateId(2)).is_some(),
            "the refusal is lost"
        );
        // THE PLAYER GONE: nothing of it held.
        f.forget();
        assert!(f.desk(DeskKind::Subtitles).is_none());
        assert!(f.cover_refused(CandidateId(2)).is_none());
    }

    /// FAILS IF: subtitles are searched without the person's key — or the
    /// page stops saying there is no source without one and leading to where
    /// it goes — or the row that asks names a service with no key, or the
    /// languages stop being the interface's then English, named, added by a
    /// code the table knows and taken out down to the last one.
    #[test]
    fn subtitles_are_searched_in_named_languages_with_a_key() {
        let t = texts("pt-BR");
        let film = "/films/harbour.mkv";
        let s = a_film(film);
        let mut v = View::blank(None);
        v.key(Press::Go(PageId::FindSubtitles), Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::FindSubtitles);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(said(&rows).contains(&t.t(key::PLAYER_FIND_NEEDS_A_KEY)));
        assert!(row_of(&rows, &Target::Search(DeskKind::Subtitles)).is_none());
        assert!(row_of(&rows, &Target::Go(PageId::Internet)).is_some());
        // THE LANGUAGES, BY NAME: the interface's, then English.
        let names: Vec<String> = rows
            .iter()
            .filter(|r| matches!(r.target, Target::Language(_)))
            .map(|r| r.fields[0].text.clone())
            .collect();
        assert_eq!(names, [t.t("tongue-pt-br"), t.t("tongue-en")]);
        // A KEY KEPT: its service, and only it, is asked.
        v.kept = crate::player_internet::Kept::given(vec![Key::OpenSubtitles]);
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Search(DeskKind::Subtitles)).expect("the ask");
        let label = &rows[at].fields[0].text;
        assert!(
            label.contains("api.opensubtitles.com") && !label.contains("subdl"),
            "{label}"
        );
        v.grabbed(at);
        match v.key(Press::Enter, Some(&s), 80, &t) {
            Errand::Let { services, then, .. } => {
                assert_eq!(services, [Service::OpenSubtitles]);
                assert!(matches!(
                    *then,
                    Errand::TellSaying(
                        Ask::Find {
                            query: Query::Subtitles { ref tongues, .. }
                        },
                        _
                    ) if tongues == &["pt-BR".to_owned(), "en".to_owned()]
                ));
            }
            other => panic!("the first press asked {other:?}"),
        }
        // A LANGUAGE ADDED BY ITS CODE OR ITS NAME, typed on the Languages'
        // page — named when said — and refused when the table does not know
        // it or it is there already.
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Go(PageId::Languages)).expect("the languages' page");
        v.grabbed(at);
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Languages);
        let mut pairing = crate::player_machines::Pairing::default();
        let mut add = |v: &mut View, code: &str| {
            let rows = v.rows(Some(&s), 80, &t);
            let at = row_of(&rows, &Target::AddLanguage).expect("type a language");
            v.grabbed(at);
            let Errand::Ask(q) = v.key(Press::Enter, Some(&s), 80, &t) else {
                panic!("adding asked nothing");
            };
            v.answered(q.id, code, &mut pairing, Some(&s), 80, &t)
        };
        assert_eq!(
            add(&mut v, "es"),
            Errand::Said(t.t_with(
                key::PLAYER_FIND_LANGUAGE_ADDED,
                &[("language", &t.t("tongue-es"))]
            ))
        );
        assert_eq!(
            add(&mut v, "xx"),
            Errand::Said(t.t_with(key::PLAYER_FIND_UNKNOWN_LANGUAGE, &[("code", "xx")]))
        );
        assert!(matches!(add(&mut v, "EN"), Errand::Said(_)));
        assert_eq!(v.finding.tongues(&t), ["pt-BR", "en", "es"]);
        // BY ITS NAME in the person's language, any case.
        let japanese = t.t("tongue-ja").to_uppercase();
        assert!(matches!(add(&mut v, &japanese), Errand::Said(_)));
        assert_eq!(v.finding.tongues(&t), ["pt-BR", "en", "es", "ja"]);
        // AND BY THE POINTER: every language a row, on or off.
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::LanguagePick("ja".into())).expect("japanese's row");
        v.grabbed(at);
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(v.finding.tongues(&t), ["pt-BR", "en", "es"]);
        assert!(
            ronoctua_player::tongues::TONGUES
                .iter()
                .all(
                    |tongue| row_of(&rows, &Target::LanguagePick(tongue.code.to_owned())).is_some()
                ),
            "a language the services search in has no row"
        );
        v.key(Press::Escape, Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::FindSubtitles);
        // RET ON A LANGUAGE'S NAME takes nothing out: it says so.
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Language("en".into())).expect("english");
        v.grabbed(at);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Said(_)
        ));
        assert_eq!(v.finding.tongues(&t), ["pt-BR", "en", "es"]);
        // TAKEN OUT by the glyph at its end — a click acts at once.
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Language("en".into())).expect("english");
        let glyph = rows[at]
            .cells
            .iter()
            .find(|c| c.control)
            .map(|c| c.from)
            .expect("the glyph");
        assert!(v.clicked_at(at, glyph, &rows));
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Said(_)
        ));
        assert_eq!(v.finding.tongues(&t), ["pt-BR", "es"]);
        // NEVER THE LAST — and the last carries no glyph that would.
        v.finding.tongues = Some(vec!["es".into()]);
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Language("es".into())).expect("spanish");
        assert!(
            !rows[at].cells.iter().any(|c| c.control),
            "the last language offers a glyph that takes it out"
        );
        v.grabbed(at);
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            Errand::Said(t.t(key::PLAYER_FIND_LANGUAGE_LAST))
        );
    }

    /// FAILS IF: what was found stops being grouped by language — each
    /// named, the one timed for this very file first and marked — or a
    /// choice stops downloading it, or how that went stops being said under
    /// it, or the downloads left stop being said.
    #[test]
    fn subtitles_found_are_grouped_by_language_timed_first() {
        let t = texts("en-GB");
        let film = "/films/harbour.mkv";
        let s = a_film(film);
        let mut v = View::blank(None);
        v.kept = crate::player_internet::Kept::given(vec![Key::OpenSubtitles]);
        let mut desk = subtitles_desk(film);
        desk.taken = Some(Taken {
            candidate: CandidateId(2),
            status: TakeStatus::Saved {
                path: "/films/harbour.pt-BR.srt".into(),
            },
        });
        v.found.given(desk);
        v.push(PageId::FindSubtitles, None);
        let rows = v.rows(Some(&s), 80, &t);
        let order: Vec<&Target> = rows
            .iter()
            .filter(|r| r.stops())
            .map(|r| &r.target)
            .filter(|t| matches!(t, Target::TakeSubtitle(_)))
            .collect();
        assert_eq!(
            order,
            [
                &Target::TakeSubtitle(CandidateId(2)),
                &Target::TakeSubtitle(CandidateId(1)),
                &Target::TakeSubtitle(CandidateId(3)),
            ]
        );
        let text = said(&rows);
        let brazil = text.find("Portuguese (Brazil)").expect("its heading");
        let english = text.rfind("English").expect("its heading");
        assert!(brazil < english, "{text}");
        let timed = row_of(&rows, &Target::TakeSubtitle(CandidateId(2))).expect("timed");
        assert_eq!(
            rows[timed].fields[0].text,
            tokens::icon::PLAYER_HASH_MATCH.to_string()
        );
        assert!(text.contains("for the hearing impaired"), "{text}");
        assert!(
            text.contains(&t.t_with(key::PLAYER_TAKE_SAVED, &[("name", "harbour.pt-BR.srt")])),
            "{text}"
        );
        assert!(text.contains(&t.t(key::PLAYER_FIND_DOWNLOADS_LEFT)));
        v.grabbed(timed);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(
                take(DeskKind::Subtitles, CandidateId(2)),
                t.t_with(
                    key::PLAYER_FIND_TAKING,
                    &[("name", "Harbour.2024.1080p.BluRay")]
                )
            )
        );
    }

    /// FAILS IF: a search refused for a host not let stops being a row that
    /// lets that host and asks the same search again.
    #[test]
    fn a_host_not_let_is_a_row_that_lets_it_and_asks_again() {
        let t = texts("en-GB");
        let film = "/films/harbour.mkv";
        let s = a_film(film);
        let mut v = View::blank(None);
        v.kept = crate::player_internet::Kept::given(vec![Key::OpenSubtitles, Key::SubDl]);
        let mut desk = subtitles_desk(film);
        desk.refused = vec![Trouble::Off {
            host: "api.subdl.com".into(),
        }];
        let asked = desk.query.clone().expect("a query");
        v.found.given(desk);
        v.push(PageId::FindSubtitles, None);
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::Let("api.subdl.com".into())).expect("the row");
        assert_eq!(
            rows[at].fields[0].text,
            t.t_with(key::PLAYER_CONSENT_LET, &[("host", "api.subdl.com")])
        );
        v.grabbed(at);
        match v.key(Press::Enter, Some(&s), 80, &t) {
            Errand::Let {
                services,
                on: true,
                then,
                ..
            } => {
                assert_eq!(services, [Service::SubDl]);
                assert!(
                    matches!(*then, Errand::TellSaying(Ask::Find { ref query }, _) if *query == asked)
                );
            }
            other => panic!("the row asked {other:?}"),
        }
        assert!(v.prefs.allows(Service::SubDl));
    }

    fn record(id: u64, track: &str, synced: bool, instrumental: bool) -> Record {
        Record {
            id,
            track: track.into(),
            artist: "Marta Quintela".into(),
            album: Some("Tidewater".into()),
            duration_seconds: Some(215.0),
            instrumental,
            plain: (!instrumental).then(|| "words".into()),
            synced: synced.then(|| "[00:01.00] words".into()),
        }
    }

    /// FAILS IF: lyrics stop saying what each record is — synced, words
    /// only, instrumental — and how far its length is from the song's, or a
    /// take stops going where the ring says, or a `.lrc` already beside the
    /// song is replaced without a question born on its "no".
    #[test]
    fn lyrics_go_where_the_ring_says_and_a_replacement_is_asked() {
        let t = texts("en-GB");
        let dir = std::env::temp_dir().join(format!("ronoctua-lyrics-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let song = dir.join("harbour.flac").to_string_lossy().into_owned();
        std::fs::write(dir.join("harbour.lrc"), "[00:01.00] old").expect("a .lrc");
        let s = alone(&song, vec![track(1, Kind::Audio, None, true)]);
        let mut v = View::blank(None);
        v.found.given(Desk {
            query: Some(Query::Lyrics {
                location: song.clone(),
                words: None,
            }),
            page: Page::Lyrics {
                location: song,
                duration_seconds: Some(215.0),
                candidates: vec![
                    LyricsCandidate {
                        id: CandidateId(7),
                        record: record(1, "Harbour Lights", true, false),
                        off_by_seconds: Some(0.2),
                    },
                    LyricsCandidate {
                        id: CandidateId(8),
                        record: record(2, "Harbour Lights (live)", false, false),
                        off_by_seconds: Some(65.0),
                    },
                    LyricsCandidate {
                        id: CandidateId(9),
                        record: record(3, "Harbour Lights (instrumental)", false, true),
                        off_by_seconds: None,
                    },
                ],
            },
            ..Desk::empty(DeskKind::Lyrics)
        });
        v.key(Press::Go(PageId::FindLyrics), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let text = said(&rows);
        for k in [
            key::PLAYER_LYRICS_ONE_SOURCE,
            key::PLAYER_FIND_SYNCED,
            key::PLAYER_FIND_PLAIN,
            key::PLAYER_FIND_INSTRUMENTAL,
            key::PLAYER_FIND_SAME_LENGTH,
        ] {
            assert!(text.contains(&t.t(k)), "{k}:\n{text}");
        }
        assert!(
            text.contains(&t.t_args(key::PLAYER_FIND_OFF_BY, &[("seconds", Arg::Number(65))])),
            "{text}"
        );
        // BESIDE THE SONG, where a `.lrc` is: asked first, born on its "no".
        let at = row_of(&rows, &Target::TakeLyrics(CandidateId(7))).expect("the first");
        v.grabbed(at);
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Answer(false));
        v.key(Press::Down, Some(&s), 80, &t);
        match v.key(Press::Enter, Some(&s), 80, &t) {
            Errand::TellSaying(
                Ask::Take {
                    candidate: CandidateId(7),
                    to: LyricsTo::Beside,
                    replace: true,
                    ..
                },
                _,
            ) => {}
            other => panic!("yes asked {other:?}"),
        }
        // INTO THE SONG, the ring turned: no question.
        let rows = v.rows(Some(&s), 80, &t);
        let ring = row_of(&rows, &Target::LyricsTo).expect("the ring");
        v.grabbed(ring);
        v.key(Press::More, Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let at = row_of(&rows, &Target::TakeLyrics(CandidateId(8))).expect("the second");
        v.grabbed(at);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(
                Ask::Take {
                    to: LyricsTo::Tag,
                    replace: false,
                    ..
                },
                _
            )
        ));
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: a door with no loop asks anybody for a desk, or a page
    /// handed to it is not the one it shows.
    #[test]
    fn a_door_with_no_loop_asks_nobody_and_shows_what_it_is_handed() {
        let film = "/films/harbour.mkv";
        let mut s = a_film(film);
        s.desks = vec![ronoctua_player::wire::DeskSummary {
            kind: DeskKind::Subtitles,
            generation: 9,
            busy: true,
            trouble: None,
        }];
        let mut f = Found::default();
        f.look(&[DeskKind::Subtitles], None, Some(&s), None);
        assert!(f.coming.is_empty(), "a desk was asked with no loop to wake");
        f.given(subtitles_desk(film));
        assert!(f.page_for(DeskKind::Subtitles, film).is_some());
        assert!(
            f.page_for(DeskKind::Subtitles, "/films/other.mkv")
                .is_none()
        );
    }
}
