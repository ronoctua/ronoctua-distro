//! The lookups' conversations: what the daemon asks each service, in which
//! order, and what it makes of the answers.
//!
//! Every function here BLOCKS — on each host's pace and on the network — and
//! runs on a desk's thread (`desk.rs`), never under the engine's lock. Each
//! talks to a [`Net`] and reads the person's keys through [`Keys`], so every
//! conversation is tested with answers written by hand and keys that are not
//! anybody's: no test here reaches a host.
//!
//! # What a search is asked with
//!
//! The person's words when they typed some; otherwise what the file says of
//! itself — its tags (a MusicBrainz id first, when it carries one), then what
//! its name reads as (`ronoctua_lookup::name::guess`). What went out is said
//! back ([`Asked`]), so a door can show what was searched.
//!
//! # Consent and keys, checked where they are used
//!
//! Each service is asked only while the person lets it
//! ([`Preferences::allows`]) and, for the keyed ones, only with the key they
//! kept. A subtitle search asks every subtitle service it may, and says of
//! each one it could not ask why ([`Trouble::Off`], a missing key).

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

use ronoctua_lookup::{
    Net, Refusal, Secret, Service, Why,
    found::Found,
    keys::{self, Account, Key},
    lrclib, musicbrainz, name, opensubtitles, query, subdl,
    suggestion::Suggestion,
    tmdb, wikidata,
};
use ronoctua_player::{
    find::Asked,
    find::Suggester,
    prefs::{self, Preferences},
    subtitles::{self, charset, sniff},
    tongues::{self, Spelling, Tongue},
    wire::Trouble,
};
use ronoctua_tags::{Field, Sheet, TagValue, TechKey};

/// How many candidates of a search have their details read: MusicBrainz's
/// recording and release (two requests a second apart each), TMDB's film or
/// episode. The first few are what a person looks at; reading all ten would
/// be twenty seconds of MusicBrainz for rows nobody opens.
pub(super) const DETAILED: usize = 3;

/// The width a poster is asked at: a cover in a tag, not a wall print.
pub(super) const POSTER_PX: u32 = 600;

/// The names a MusicBrainz RECORDING id goes by in a file's tags, lowercase
/// with the separators taken out: Picard's Vorbis `MUSICBRAINZ_TRACKID` (its
/// "track id" IS the recording's) and its MP4 `MusicBrainz Track Id`, and
/// the plainer spelling. NOT `MUSICBRAINZ_RELEASETRACKID`: that names the
/// track on one release, and asked as a recording it would be nobody's.
const RECORDING_ID_TAGS: [&str; 2] = ["musicbrainztrackid", "musicbrainzrecordingid"];

/// The names a MusicBrainz release id goes by.
const RELEASE_ID_TAGS: [&str; 2] = ["musicbrainzalbumid", "musicbrainzreleaseid"];

/// The person's keys, as a conversation reads them.
pub(super) trait Keys: Sync {
    /// The key a service needs, or the refusal a lookup gives without it.
    ///
    /// # Errors
    ///
    /// No key kept ([`Why::NoKey`]), or one that cannot be read.
    fn key(&self, key: Key) -> Result<Secret, Refusal>;
    /// The OpenSubtitles account, when one is kept and can be read.
    fn account(&self) -> Option<Account>;
}

/// The keys the person kept, read when they are needed (never a copy from
/// the daemon's start: a key pasted a moment ago is the one used).
pub(super) struct Kept;

impl Keys for Kept {
    fn key(&self, key: Key) -> Result<Secret, Refusal> {
        keys::needed(key)
    }

    fn account(&self) -> Option<Account> {
        keys::account().ok().flatten()
    }
}

/// A lookup's refusal, as the wire says it. A host the person has not let
/// in `consent.json` ([`Why::NotLet`]: the lookup's pass was refused, and
/// nothing was sent) is said as the service being off — the consent row,
/// whose press lets the hosts its row names and asks again — never as a
/// failure the person cannot act on.
pub(super) fn lookup(r: Refusal) -> Trouble {
    if r.why == Why::NotLet {
        return off(r.service);
    }
    Trouble::Lookup(r)
}

/// A service the person has not let the player ask, as the wire says it:
/// the host its row names first.
pub(super) fn off(service: Service) -> Trouble {
    Trouble::Off {
        host: prefs::host_of(service).to_owned(),
    }
}

/// Every service of `services` the player may ask, or the refusal naming the
/// first it may not.
///
/// # Errors
///
/// [`Trouble::Off`] for the first one not let.
pub(super) fn let_all(prefs: &Preferences, services: &[Service]) -> Result<(), Trouble> {
    prefs.first_off(services).map_or(Ok(()), |s| Err(off(s)))
}

/// What the daemon knows of what a search is about.
#[derive(Debug, Clone, Default)]
pub(super) struct About {
    /// The location as the door named it.
    pub location: String,
    /// The file, canonical, when it is a file on this machine.
    pub file: Option<PathBuf>,
    /// What it says of itself, when it could be read.
    pub sheet: Option<Sheet>,
    /// Its length, from the state when it plays.
    pub duration_seconds: Option<f64>,
}

impl About {
    /// A field of its sheet.
    fn value(&self, field: Field) -> Option<String> {
        self.sheet
            .as_ref()
            .and_then(|s| s.value(field))
            .map(str::to_owned)
    }

    /// A tag of its sheet by any of `names` (lowercase, no separators),
    /// whole text only.
    fn tag(&self, names: &[&str]) -> Option<String> {
        let sheet = self.sheet.as_ref()?;
        sheet.other.iter().find_map(|t| {
            let bare: String = t
                .name
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_ascii_lowercase();
            let bare = bare.trim_start_matches("txxx");
            match &t.value {
                TagValue::Text(v) if names.contains(&bare) => Some(v.trim().to_ascii_lowercase()),
                _ => None,
            }
        })
    }

    /// Its name without the folder and the extension — a file's, or an
    /// address's last part.
    pub(super) fn stem(&self) -> String {
        let path = self
            .location
            .split(['?', '#'])
            .next()
            .unwrap_or(&self.location);
        Path::new(path.trim_end_matches('/'))
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned()
    }

    /// What its name reads as.
    fn guess(&self) -> name::Guess {
        name::guess(&self.stem())
    }

    /// Its length, in seconds: the state's, else the sheet's.
    pub(super) fn duration(&self) -> Option<f64> {
        self.duration_seconds.filter(|d| *d > 0.0).or_else(|| {
            let sheet = self.sheet.as_ref()?;
            let ms: f64 = sheet
                .technical
                .iter()
                .find(|f| f.key == TechKey::DurationMs)?
                .value
                .parse()
                .ok()?;
            (ms > 0.0).then_some(ms / 1000.0)
        })
    }

    /// A number field of its sheet (`3`, or `3/12`), else what its name says.
    fn number(&self, field: Field, guessed: Option<u16>) -> Option<u16> {
        self.value(field)
            .and_then(|v| v.split('/').next().and_then(|n| n.trim().parse().ok()))
            .or(guessed)
    }

    /// Its year: the date's first four digits, else its name's.
    fn year(&self, guessed: Option<u16>) -> Option<u16> {
        self.value(Field::Date)
            .and_then(|d| d.get(..4).and_then(|y| y.parse().ok()))
            .or(guessed)
    }
}

/// The languages names are asked in: the door's, then English, each once and
/// lowercase — Wikidata's label service takes them in that order.
fn label_languages(tongues: &[&Tongue]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for code in tongues
        .iter()
        .map(|t| t.code.to_ascii_lowercase())
        .chain(std::iter::once(tongues::ENGLISH.to_owned()))
    {
        if !out.contains(&code) {
            out.push(code);
        }
    }
    out
}

/// What a suggestion search tells as it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Step {
    /// The candidates, as the search found them; `complete` when their
    /// details are all there already.
    List(Vec<Suggestion>, bool),
    /// One candidate's details were read: its full suggestion.
    Detail(usize, Suggestion),
}

/// Tags for `about` from `source`: the candidates first, then the details of
/// the first few, one at a time — each told through `step` as it comes.
/// `still` is asked between requests: `false` when a newer search replaced
/// this one, which then stops.
///
/// # Errors
///
/// The trouble that gave nothing: a missing key, the service's refusal,
/// nothing found, nothing to ask with.
#[allow(clippy::too_many_arguments)]
pub(super) fn suggest(
    net: &dyn Net,
    keys: &dyn Keys,
    source: Suggester,
    words: Option<&str>,
    tongues: &[&Tongue],
    about: &About,
    still: &dyn Fn() -> bool,
    step: &mut dyn FnMut(Step),
) -> Result<Asked, Trouble> {
    let words = words.map(str::trim).filter(|w| !w.is_empty());
    match source {
        Suggester::MusicBrainz => from_musicbrainz(net, words, about, still, step),
        Suggester::Wikidata => from_wikidata(net, words, tongues, about, step),
        Suggester::Tmdb => from_tmdb(net, keys, words, tongues, about, still, step),
    }
}

fn from_musicbrainz(
    net: &dyn Net,
    words: Option<&str>,
    about: &About,
    still: &dyn Fn() -> bool,
    step: &mut dyn FnMut(Step),
) -> Result<Asked, Trouble> {
    let album_hint = about.value(Field::Album);
    let release_id = about.tag(&RELEASE_ID_TAGS).filter(|i| query::is_mbid(i));
    // THE FILE SAYS WHICH RECORDING IT IS: asked by that id, not searched —
    // unless MusicBrainz no longer knows it (merged, or never theirs), and
    // then the search goes on as for a file that says nothing.
    if words.is_none()
        && let Some(id) = about.tag(&RECORDING_ID_TAGS).filter(|i| query::is_mbid(i))
    {
        let recording = match musicbrainz::recording(net, &id) {
            Ok(r) => Some(r),
            Err(r) if r.why == Why::NothingFound => None,
            Err(r) => return Err(lookup(r)),
        };
        if let Some(recording) = recording {
            return Ok(by_id(
                net,
                &recording,
                release_id.as_deref(),
                album_hint.as_deref(),
                step,
            ));
        }
    }
    let song = match words {
        Some(w) => musicbrainz::Song {
            title: w.to_owned(),
            artist: None,
            album: None,
            duration_seconds: None,
        },
        None => musicbrainz::Song {
            title: about
                .value(Field::Title)
                .unwrap_or_else(|| about.guess().title),
            artist: about
                .value(Field::Artist)
                .or_else(|| about.value(Field::AlbumArtist)),
            album: album_hint.clone(),
            duration_seconds: about.duration(),
        },
    };
    let asked = Asked {
        words: song.title.clone(),
        artist: song.artist.clone(),
        album: song.album.clone(),
        ..Asked::default()
    };
    let found = musicbrainz::search(net, &song).map_err(lookup)?;
    if found.is_empty() {
        return Err(lookup(Refusal::new(
            Service::MusicBrainz,
            Why::NothingFound,
        )));
    }
    step(Step::List(
        found
            .iter()
            .map(|r| musicbrainz::suggestion(r, None))
            .collect(),
        false,
    ));
    for (n, r) in found.iter().take(DETAILED).enumerate() {
        if !still() {
            break;
        }
        // THE RECORDING'S OWN PAGE has its writers, ISRCs and genres; the
        // search's has none of them. A recording that will not come is shown
        // as the search found it.
        let full = musicbrainz::recording(net, &r.id).unwrap_or_else(|_| r.clone());
        if !still() {
            break;
        }
        let release = release_of(net, &full, release_id.as_deref(), album_hint.as_deref());
        step(Step::Detail(
            n,
            musicbrainz::suggestion(&full, release.as_ref()),
        ));
    }
    Ok(asked)
}

/// The one candidate of a recording the file named by its id: complete, its
/// release read.
fn by_id(
    net: &dyn Net,
    recording: &musicbrainz::Recording,
    release_id: Option<&str>,
    album: Option<&str>,
    step: &mut dyn FnMut(Step),
) -> Asked {
    let release = release_of(net, recording, release_id, album);
    step(Step::List(
        vec![musicbrainz::suggestion(recording, release.as_ref())],
        true,
    ));
    Asked {
        words: recording.title.clone(),
        artist: Some(recording.artist.clone()).filter(|a| !a.is_empty()),
        by_id: true,
        ..Asked::default()
    }
}

/// The release a recording is suggested from: the one the file names, else
/// the one titled like the file's album, else the first official one, else
/// the first — read whole, or `None` when it would not come.
fn release_of(
    net: &dyn Net,
    recording: &musicbrainz::Recording,
    named: Option<&str>,
    album: Option<&str>,
) -> Option<musicbrainz::Release> {
    let releases = &recording.releases;
    let chosen = named
        .and_then(|id| releases.iter().find(|r| r.id == id))
        .or_else(|| {
            album.and_then(|a| {
                releases
                    .iter()
                    .find(|r| r.title.trim().eq_ignore_ascii_case(a.trim()))
            })
        })
        .or_else(|| {
            releases
                .iter()
                .find(|r| r.status.as_deref() == Some("Official"))
        })
        .or_else(|| releases.first())?;
    musicbrainz::release(net, &chosen.id).ok()
}

fn from_wikidata(
    net: &dyn Net,
    words: Option<&str>,
    tongues: &[&Tongue],
    about: &About,
    step: &mut dyn FnMut(Step),
) -> Result<Asked, Trouble> {
    let guess = about.guess();
    let words = words.map_or_else(
        || {
            about
                .value(Field::Show)
                .or_else(|| about.value(Field::Title))
                .unwrap_or_else(|| guess.title.clone())
        },
        str::to_owned,
    );
    let films = wikidata::find(net, &words, &label_languages(tongues)).map_err(lookup)?;
    if films.is_empty() {
        return Err(lookup(Refusal::new(Service::Wikidata, Why::NothingFound)));
    }
    step(Step::List(
        films
            .iter()
            .map(|f| wikidata::suggestion(f, POSTER_PX))
            .collect(),
        true,
    ));
    Ok(Asked {
        words,
        ..Asked::default()
    })
}

fn from_tmdb(
    net: &dyn Net,
    keys: &dyn Keys,
    words: Option<&str>,
    tongues: &[&Tongue],
    about: &About,
    still: &dyn Fn() -> bool,
    step: &mut dyn FnMut(Step),
) -> Result<Asked, Trouble> {
    let token = keys.key(Key::Tmdb).map_err(lookup)?;
    let guess = about.guess();
    let season = about.number(Field::Season, guess.season);
    let episode = about.number(Field::Episode, guess.episode);
    let series = episode.is_some() || about.value(Field::Show).is_some();
    let kind = if series {
        tmdb::Kind::Tv
    } else {
        tmdb::Kind::Movie
    };
    let words = words.map_or_else(
        || {
            about
                .value(Field::Show)
                .filter(|_| series)
                .or_else(|| about.value(Field::Title))
                .unwrap_or_else(|| guess.title.clone())
        },
        str::to_owned,
    );
    let year = about.year(guess.year);
    let language = tongues.first().map_or(tongues::ENGLISH, |t| t.code);
    let hits = tmdb::search(net, &token, kind, &words, year, language).map_err(lookup)?;
    if hits.is_empty() {
        return Err(lookup(Refusal::new(Service::Tmdb, Why::NothingFound)));
    }
    step(Step::List(
        hits.iter()
            .map(|h| tmdb::suggestion(&details_of_hit(h), POSTER_PX))
            .collect(),
        false,
    ));
    for (n, hit) in hits.iter().take(DETAILED).enumerate() {
        if !still() {
            break;
        }
        let details = match (kind, season, episode) {
            (tmdb::Kind::Movie, ..) => tmdb::movie(net, &token, hit.id, language).ok(),
            (tmdb::Kind::Tv, Some(s), Some(e)) => {
                tmdb::episode(net, &token, hit.id, s, e, language).ok()
            }
            // A SERIES WITH NO EPISODE NAMED has no more to read than its
            // search already said.
            (tmdb::Kind::Tv, ..) => None,
        };
        if let Some(d) = details {
            step(Step::Detail(n, tmdb::suggestion(&d, POSTER_PX)));
        }
    }
    Ok(Asked {
        words,
        year,
        season,
        episode,
        ..Asked::default()
    })
}

/// A search hit as the details a suggestion is made of, with what the hit
/// alone says.
fn details_of_hit(hit: &tmdb::Hit) -> tmdb::Details {
    tmdb::Details {
        id: hit.id,
        kind: hit.kind,
        title: hit.title.clone(),
        show: (hit.kind == tmdb::Kind::Tv).then(|| hit.title.clone()),
        season: None,
        episode: None,
        date: hit.date.clone(),
        overview: hit.overview.clone(),
        genres: Vec::new(),
        directors: Vec::new(),
        language: None,
        poster: hit.poster.clone(),
    }
}

/// An OpenSubtitles login the daemon keeps in memory — never on disk — for
/// the account it was made with.
#[derive(Debug, Clone)]
pub(super) struct LoggedIn {
    /// The account's user name.
    user: String,
    /// The token and the host it goes to.
    session: opensubtitles::Session,
}

/// The session to ask OpenSubtitles with: the one kept when the account is
/// the same, else a new login (`fresh` forces one) — and the downloads left
/// today when a login was made. `None` with no account, or a login refused
/// (said in the trouble; a key alone still searches).
pub(super) fn session_for(
    net: &dyn Net,
    key: &Secret,
    keys: &dyn Keys,
    slot: &Mutex<Option<LoggedIn>>,
    fresh: bool,
) -> (Option<opensubtitles::Session>, Option<Trouble>, Option<i64>) {
    let mut kept = slot.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(account) = keys.account() else {
        *kept = None;
        return (None, None, None);
    };
    if !fresh && let Some(l) = kept.as_ref().filter(|l| l.user == account.user) {
        return (Some(l.session.clone()), None, None);
    }
    // ONE LOGIN AT A TIME: the slot is held across it, so two searches at
    // once never log in twice.
    match opensubtitles::login(net, key, &account) {
        Ok(session) => {
            let left = opensubtitles::user(net, key, &session)
                .ok()
                .and_then(|a| a.remaining_downloads);
            *kept = Some(LoggedIn {
                user: account.user,
                session: session.clone(),
            });
            (Some(session), None, left)
        }
        Err(r) => {
            *kept = None;
            (None, Some(lookup(r)), None)
        }
    }
}

/// What a subtitle search found.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SubtitlesFound {
    /// What it was asked with.
    pub asked: Asked,
    /// The subtitles, ranked.
    pub found: Vec<Found>,
    /// The services that gave nothing while another answered, and why.
    pub refused: Vec<Trouble>,
    /// OpenSubtitles' downloads left today, when a login said.
    pub remaining: Option<i64>,
    /// Why nothing is on the page, when nothing is.
    pub trouble: Option<Trouble>,
}

/// Subtitles for `about`, in `tongues`, from every subtitle service the
/// person let the player ask and kept a key for.
///
/// # Errors
///
/// No service could be asked: the first reason (not let, no key).
pub(super) fn subtitles(
    net: &dyn Net,
    keys: &dyn Keys,
    prefs: &Preferences,
    session: &Mutex<Option<LoggedIn>>,
    tongues: &[&Tongue],
    words: Option<&str>,
    about: &About,
) -> Result<SubtitlesFound, Trouble> {
    let guess = about.guess();
    let words = words.map(str::trim).filter(|w| !w.is_empty());
    let season = about.number(Field::Season, guess.season);
    let episode = about.number(Field::Episode, guess.episode);
    let title = words.map_or_else(
        || {
            about
                .value(Field::Show)
                .or_else(|| {
                    about
                        .value(Field::Title)
                        .filter(|_| guess.kind != name::Kind::Episode)
                })
                .unwrap_or_else(|| guess.title.clone())
        },
        str::to_owned,
    );
    let year = about.year(guess.year).filter(|_| episode.is_none());
    // THE FILE'S FINGERPRINT, when it is a file here: a match is timed for
    // exactly this file. A file too small or unreadable is asked by its
    // name alone.
    let hash = about
        .file
        .as_deref()
        .and_then(|f| opensubtitles::moviehash(f).ok());
    let mut asked = Asked {
        words: title.clone(),
        year,
        season,
        episode,
        fingerprint: false,
        ..Asked::default()
    };
    let mut out = SubtitlesFound {
        asked: Asked::default(),
        found: Vec::new(),
        refused: Vec::new(),
        remaining: None,
        trouble: None,
    };
    let mut answered: Vec<Service> = Vec::new();

    // OPENSUBTITLES: the key, and the account when one is kept.
    if !prefs.allows(Service::OpenSubtitles) {
        out.refused.push(off(Service::OpenSubtitles));
    } else {
        match keys.key(Key::OpenSubtitles) {
            Err(r) => out.refused.push(lookup(r)),
            Ok(key) => {
                let search = opensubtitles::Search {
                    moviehash: hash,
                    query: Some(title.clone()),
                    year,
                    season,
                    episode,
                    imdb: None,
                    languages: tongues::spelled(tongues, Spelling::OpenSubtitles),
                };
                asked.fingerprint = hash.is_some();
                let (mut logged, trouble, left) = session_for(net, &key, keys, session, false);
                out.refused.extend(trouble);
                out.remaining = left.or(out.remaining);
                let mut result = opensubtitles::search(net, &key, logged.as_ref(), &search);
                // A TOKEN THE SERVICE NO LONGER TAKES: logged in once more.
                if logged.is_some() && result.as_ref().is_err_and(|r| r.why == Why::KeyRefused) {
                    let (again, _, left) = session_for(net, &key, keys, session, true);
                    logged = again;
                    out.remaining = left.or(out.remaining);
                    result = opensubtitles::search(net, &key, logged.as_ref(), &search);
                }
                match result {
                    Ok(found) => {
                        answered.push(Service::OpenSubtitles);
                        out.found.extend(found);
                    }
                    Err(r) => out.refused.push(lookup(r)),
                }
            }
        }
    }

    // SUBDL: the key, in the query string — its only way.
    if !prefs.allows(Service::SubDl) {
        out.refused.push(off(Service::SubDl));
    } else {
        match keys.key(Key::SubDl) {
            Err(r) => out.refused.push(lookup(r)),
            Ok(key) => {
                let search = subdl::Search {
                    name: Some(title),
                    imdb: None,
                    tmdb: None,
                    kind: Some(if episode.is_some() {
                        subdl::Kind::Tv
                    } else {
                        subdl::Kind::Movie
                    }),
                    year,
                    season,
                    episode,
                    languages: tongues::spelled(tongues, Spelling::SubDl),
                };
                match subdl::search(net, &key, &search) {
                    Ok(found) => {
                        answered.push(Service::SubDl);
                        out.found.extend(found);
                    }
                    Err(r) => out.refused.push(lookup(r)),
                }
            }
        }
    }

    if answered.is_empty() {
        // WHY NOTHING CAME: a service the person let, and what stopped it,
        // before one they did not let.
        return Err(out
            .refused
            .iter()
            .find(|t| !matches!(t, Trouble::Off { .. }))
            .or_else(|| out.refused.first())
            .cloned()
            .unwrap_or_else(|| lookup(Refusal::new(Service::OpenSubtitles, Why::NothingToAsk))));
    }
    ronoctua_lookup::found::rank(&mut out.found);
    if out.found.is_empty() {
        out.trouble = Some(lookup(Refusal::new(answered[0], Why::NothingFound)));
    }
    out.asked = asked;
    Ok(out)
}

/// A subtitle fetched: its file's name and bytes, and the `.sub` a VobSub
/// `.idx` needs beside it.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Fetched {
    /// The file's name, as the service gave it.
    pub name: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
    /// A VobSub's pictures, for an `.idx`.
    pub companion: Option<(String, Vec<u8>)>,
}

impl std::fmt::Debug for Fetched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fetched")
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// The subtitle `found` names, fetched — and the downloads left today when
/// the service said (OpenSubtitles counts each).
///
/// # Errors
///
/// Not let, no key, the day's downloads spent, the service's refusal.
pub(super) fn fetch_subtitle(
    net: &dyn Net,
    keys: &dyn Keys,
    prefs: &Preferences,
    session: &Mutex<Option<LoggedIn>>,
    found: &Found,
    about: &About,
) -> Result<(Fetched, Option<i64>), Trouble> {
    let service = found.source;
    if !prefs.allows(service) {
        return Err(off(service));
    }
    match &found.take {
        ronoctua_lookup::found::Take::OpenSubtitles { file_id } => {
            let key = keys.key(Key::OpenSubtitles).map_err(lookup)?;
            let (mut logged, _, mut left) = session_for(net, &key, keys, session, false);
            let mut got = opensubtitles::download(net, &key, logged.as_ref(), *file_id);
            if logged.is_some() && got.as_ref().is_err_and(|r| r.why == Why::KeyRefused) {
                let (again, _, l) = session_for(net, &key, keys, session, true);
                logged = again;
                left = l.or(left);
                got = opensubtitles::download(net, &key, logged.as_ref(), *file_id);
            }
            let download = got.map_err(lookup)?;
            let bytes = opensubtitles::fetch(net, &download).map_err(lookup)?;
            let name = download
                .file_name
                .clone()
                .or_else(|| found.file_name.clone())
                .unwrap_or_else(|| "subtitle.srt".to_owned());
            Ok((
                Fetched {
                    name,
                    bytes,
                    companion: None,
                },
                download.remaining.or(left),
            ))
        }
        ronoctua_lookup::found::Take::SubDl { path } => {
            let members = subdl::fetch(net, path).map_err(lookup)?;
            let guess = about.guess();
            let season = found
                .season
                .or_else(|| about.number(Field::Season, guess.season));
            let episode = found
                .episode
                .or_else(|| about.number(Field::Episode, guess.episode));
            chosen(members, season, episode)
                .map(|f| (f, None))
                .ok_or_else(|| lookup(Refusal::new(Service::SubDl, Why::NothingFound)))
        }
        ronoctua_lookup::found::Take::Podnapisi { .. } => {
            Err(lookup(Refusal::new(Service::Podnapisi, Why::Dormant)))
        }
    }
}

/// The subtitle of a zip that is this episode's — a season pack holds every
/// episode — else the first that is words, else the first; and a VobSub
/// `.idx` with its `.sub`. Pure.
fn chosen(
    members: Vec<subdl::Member>,
    season: Option<u16>,
    episode: Option<u16>,
) -> Option<Fetched> {
    let extension = |m: &subdl::Member| {
        Path::new(&m.name)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default()
    };
    let this_episode = |m: &subdl::Member| {
        let g = name::guess(
            Path::new(&m.name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default(),
        );
        episode.is_some() && g.episode == episode && (season.is_none() || g.season == season)
    };
    let pictures = |m: &subdl::Member| matches!(extension(m).as_str(), "sub" | "sup");
    let at = members
        .iter()
        .position(|m| this_episode(m) && !pictures(m))
        .or_else(|| members.iter().position(this_episode))
        .or_else(|| members.iter().position(|m| !pictures(m)))
        .or_else(|| (!members.is_empty()).then_some(0))?;
    let pick = members[at].clone();
    let companion = (extension(&pick) == "idx")
        .then(|| {
            let stem = Path::new(&pick.name).file_stem().map(ToOwned::to_owned);
            members.iter().find(|m| {
                extension(m) == "sub"
                    && Path::new(&m.name).file_stem().map(ToOwned::to_owned) == stem
            })
        })
        .flatten()
        .map(|m| (m.name.clone(), m.bytes.clone()));
    Some(Fetched {
        name: pick.name,
        bytes: pick.bytes,
        companion,
    })
}

/// How a subtitle file is kept: its extension, and its bytes as written —
/// words made UTF-8 (a film's subtitles are the last place old code pages
/// live), pictures and binary formats as they came.
///
/// # Errors
///
/// No subtitle format this player knows.
pub(super) fn kept_as(name: &str, bytes: &[u8]) -> Result<(String, Vec<u8>), Trouble> {
    let own = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let unknown = || Trouble::Subtitle {
        path: name.to_owned(),
        why: subtitles::Refusal::Unknown,
    };
    // A VOBSUB INDEX is words that say where the pictures are: it goes as it
    // came, beside its `.sub`.
    if own.as_deref() == Some("idx") {
        return Ok(("idx".to_owned(), bytes.to_vec()));
    }
    let kind = sniff::sniff(bytes, own.as_deref()).ok_or_else(unknown)?;
    let pick = |list: &[&str]| {
        own.clone()
            .filter(|o| list.contains(&o.as_str()))
            .or_else(|| list.first().map(|e| (*e).to_owned()))
            .ok_or_else(unknown)
    };
    let words = || charset::decode(bytes).text.into_bytes();
    Ok(match kind {
        sniff::Kind::Ours(sniff::Format::EbuStl) => ("stl".to_owned(), bytes.to_vec()),
        sniff::Kind::Ours(f) => (pick(f.extensions())?, words()),
        sniff::Kind::Ffmpeg(t) => (pick(t.extensions())?, words()),
        sniff::Kind::Image(sniff::Picture::Pgs) => ("sup".to_owned(), bytes.to_vec()),
        sniff::Kind::Image(sniff::Picture::VobSub) => ("sub".to_owned(), bytes.to_vec()),
    })
}

/// The language part of a saved subtitle's name: this system's code for
/// the language the service tagged it with, else the tag itself made safe
/// for a name, else `und` (ISO's "undetermined").
pub(super) fn tongue_part(language: &str) -> String {
    if let Some(t) = tongues::of(language) {
        return t.code.to_owned();
    }
    let safe: String = language
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(16)
        .collect();
    if safe.is_empty() {
        "und".to_owned()
    } else {
        safe
    }
}

/// How many names a saved subtitle tries before it gives up: `film.en.srt`,
/// then `film.en.2.srt` to `film.en.9.srt`.
const NAMES_TRIED: u32 = 9;

/// Where subtitles for a film that is not a file here — or whose folder
/// cannot be written — are kept: `<data>/player/subtitles/<hash>/`, the hash
/// of the film's location. DATA, not cache: a saved set names them.
pub(super) fn kept_subtitles_dir(location: &str) -> Option<PathBuf> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in location.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(
        ronoctua_xdg::data_dir()?
            .join("player")
            .join("subtitles")
            .join(format!("{h:016x}")),
    )
}

/// Saves a fetched subtitle for `about` as `<film>.<language>.<ext>`: beside
/// the film when it is a file here and its folder takes it, else in
/// [`kept_subtitles_dir`]. Never over a file that is there: the same bytes
/// are that file, other bytes take the next name.
///
/// # Errors
///
/// A format this player does not know, or nowhere could be written.
pub(super) fn save_subtitle(
    about: &About,
    language: &str,
    fetched: &Fetched,
    elsewhere: Option<PathBuf>,
) -> Result<PathBuf, Trouble> {
    let (extension, bytes) = kept_as(&fetched.name, &fetched.bytes)?;
    let stem = about
        .file
        .as_deref()
        .and_then(|f| f.file_stem())
        .and_then(|s| s.to_str())
        .map_or_else(|| about.stem(), str::to_owned);
    let stem = if stem.is_empty() {
        "subtitles".to_owned()
    } else {
        stem
    };
    let base = format!("{stem}.{}", tongue_part(language));
    let beside = about.file.as_deref().and_then(Path::parent);
    for dir in [beside.map(Path::to_path_buf), elsewhere]
        .into_iter()
        .flatten()
    {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::info!(dir = %dir.display(), error = %e, "a folder that does not take the subtitle");
            continue;
        }
        match save_in(&dir, &base, &extension, &bytes) {
            Ok(Some(path)) => {
                // A VOBSUB'S PICTURES go beside their index, under its name.
                if let Some((_, pictures)) = &fetched.companion {
                    let sub = path.with_extension("sub");
                    if let Err(e) = std::fs::write(&sub, pictures) {
                        let _ = std::fs::remove_file(&path);
                        return Err(Trouble::Unwritable {
                            path: sub.to_string_lossy().into_owned(),
                            why: e.to_string(),
                        });
                    }
                }
                return Ok(path);
            }
            // THIS FOLDER DOES NOT TAKE IT (read only, not ours): the next.
            Ok(None) => {
                tracing::info!(dir = %dir.display(), "a folder that does not take the subtitle");
            }
            Err(e) => {
                tracing::info!(dir = %dir.display(), error = %e, "a folder that does not take the subtitle");
            }
        }
    }
    // A SENTENCE OF ITS OWN, and not the system's words: no one folder
    // refused, all of them did.
    Err(Trouble::NowhereToSave {
        path: about.location.clone(),
    })
}

/// `bytes` into `dir` as `<base>.<ext>`, else `<base>.<n>.<ext>`: the path
/// written — or the one that already holds these bytes —, `None` when the
/// folder does not take files.
fn save_in(
    dir: &Path,
    base: &str,
    extension: &str,
    bytes: &[u8],
) -> std::io::Result<Option<PathBuf>> {
    for n in 1..=NAMES_TRIED {
        let name = if n == 1 {
            format!("{base}.{extension}")
        } else {
            format!("{base}.{n}.{extension}")
        };
        let path = dir.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                let done = f.write_all(bytes).and_then(|()| f.sync_all());
                if let Err(e) = done {
                    let _ = std::fs::remove_file(&path);
                    return Err(e);
                }
                return Ok(Some(path));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // THE SAME SUBTITLE, taken twice: it is that file.
                if std::fs::read(&path).is_ok_and(|there| there == bytes) {
                    return Ok(Some(path));
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
                ) =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

/// Lyrics for `about`: by the person's words, else by its title, artist and
/// album — LRCLIB, which has no language field: a record is in whatever
/// language it is sung.
///
/// # Errors
///
/// Nothing to ask with, the service's refusal, nothing found.
pub(super) fn lyrics(
    net: &dyn Net,
    words: Option<&str>,
    title: Option<&str>,
    artist: Option<&str>,
    about: &About,
) -> Result<(Asked, Vec<lrclib::Record>), Trouble> {
    let words = words.map(str::trim).filter(|w| !w.is_empty());
    let (search, asked) = match words {
        Some(w) => (
            lrclib::Search::Words {
                words: w.to_owned(),
            },
            Asked {
                words: w.to_owned(),
                ..Asked::default()
            },
        ),
        None => {
            let track = about
                .value(Field::Title)
                .or_else(|| title.map(str::to_owned))
                .unwrap_or_else(|| about.guess().title);
            let artist = about
                .value(Field::Artist)
                .or_else(|| about.value(Field::AlbumArtist))
                .or_else(|| artist.map(str::to_owned));
            let album = about.value(Field::Album);
            (
                lrclib::Search::Track {
                    track: track.clone(),
                    artist: artist.clone(),
                    album: album.clone(),
                },
                Asked {
                    words: track,
                    artist,
                    album,
                    ..Asked::default()
                },
            )
        }
    };
    let records = lrclib::search(net, &search).map_err(lookup)?;
    if records.is_empty() {
        return Err(lookup(Refusal::new(Service::Lrclib, Why::NothingFound)));
    }
    Ok((asked, records))
}

/// A record's lyrics as a `.lrc` file: the header LRC readers show (artist,
/// title, album, length), then the synced lines — or the plain ones, which
/// a player shows unsynchronised. `None` for an instrumental, or a record
/// with no words.
#[must_use]
pub(super) fn lrc_of(record: &lrclib::Record) -> Option<String> {
    let (_, text) = record.best()?;
    let mut out = String::new();
    let mut tag = |name: &str, value: &str| {
        let value = value.trim().replace(['\n', '\r', ']'], " ");
        if !value.is_empty() {
            out.push_str(&format!("[{name}:{value}]\n"));
        }
    };
    tag("ar", &record.artist);
    tag("ti", &record.track);
    if let Some(album) = &record.album {
        tag("al", album);
    }
    if let Some(d) = record.duration_seconds {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let s = d.round().max(0.0) as u64;
        tag("length", &format!("{}:{:02}", s / 60, s % 60));
    }
    out.push_str(text.trim_end());
    out.push('\n');
    Some(out)
}

/// Writes lyrics beside a song: never over a `.lrc` that is there unless the
/// person said so (`replace`), and then whole or not at all.
///
/// # Errors
///
/// [`Trouble::Exists`] when one is there and `replace` is not said;
/// [`Trouble::Unwritable`] when the folder does not take it.
pub(super) fn save_lyrics(path: &Path, text: &str, replace: bool) -> Result<(), Trouble> {
    let unwritable = |e: std::io::Error| Trouble::Unwritable {
        path: path.to_string_lossy().into_owned(),
        why: e.to_string(),
    };
    if replace {
        let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str()))
        else {
            return Err(unwritable(std::io::ErrorKind::InvalidInput.into()));
        };
        return ronoctua_xdg::write_atomically(dir, file, text.as_bytes()).map_err(unwritable);
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => f.write_all(text.as_bytes()).map_err(|e| {
            let _ = std::fs::remove_file(path);
            unwritable(e)
        }),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Trouble::Exists {
            path: path.to_string_lossy().into_owned(),
        }),
        Err(e) => Err(unwritable(e)),
    }
}

/// Whether the service takes the key the person kept for it
/// ([`ronoctua_player::wire::Ask::KeyCheck`]): OpenSubtitles its key, then
/// the account when one is kept (a login, and the downloads left today);
/// TMDB its token; SubDL its key.
///
/// # Errors
///
/// Not let, no key, the key refused, no answer.
pub(super) fn key_check(
    net: &dyn Net,
    keys: &dyn Keys,
    prefs: &Preferences,
    session: &Mutex<Option<LoggedIn>>,
    service: Service,
) -> Result<Option<i64>, Trouble> {
    let Some(key_of) = service.key() else {
        return Err(lookup(Refusal::new(service, Why::NothingToAsk)));
    };
    if !prefs.allows(service) {
        return Err(off(service));
    }
    let key = keys.key(key_of).map_err(lookup)?;
    match service {
        Service::OpenSubtitles => {
            opensubtitles::check(net, &key).map_err(lookup)?;
            let (_, trouble, left) = session_for(net, &key, keys, session, true);
            trouble.map_or(Ok(left), Err)
        }
        Service::Tmdb => tmdb::check(net, &key).map(|()| None).map_err(lookup),
        Service::SubDl => subdl::check(net, &key).map(|()| None).map_err(lookup),
        _ => Err(lookup(Refusal::new(service, Why::NothingToAsk))),
    }
}

#[cfg(test)]
mod tests;
