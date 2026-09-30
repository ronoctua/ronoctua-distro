//! MPRIS: the player on the session bus, as `org.mpris.MediaPlayer2.ronoctua`.
//!
//! What this buys is everything that already speaks MPRIS: the keyboard's
//! media keys (the compositor binds them to `playerctl`, `actions::media`),
//! a headset's buttons through the Bluetooth daemon, `playerctl` in a
//! terminal. Without it those reach every player on the machine but this one.
//!
//! # The whole specification, and the one optional part left out
//!
//! The root interface, `Player` and `TrackList` are all here: the queue is
//! the track list, so a listener can read it, add to it, take from it and
//! jump in it. `Playlists` is not — it is for a player with a library of SAVED
//! lists, and this one has a queue and the files on disk, which is what
//! `LEADER p l` browses.
//!
//! THE TRACK LIST IS THE QUEUE'S LEAVES, in the tree's order, and a
//! track's id is its ENTRY's (`/br/ronoctua/player/track/<entry id>`): it
//! names the same track after the queue is reordered, and is never handed to
//! another. Any change to the queue's shape is still announced as
//! `TrackListReplaced`, so a listener's order is never stale either.
//!
//! Every property is read from the engine when asked — nothing is copied —
//! and the ones a listener watches (status, metadata, volume, the two
//! switches) are announced when they CHANGE: the engine tells this thread on
//! every broadcast, four times a second while something plays, and a
//! position alone is not news (the specification says so: `Position` is
//! polled, and a jump is the `Seeked` signal).

use std::collections::HashMap;

use ronoctua_player::{
    own::LaneKind,
    queue::{Body, EntryId, Place, Queue, Step},
    wire::{Ask, Repeat, State, Thing},
};
use zbus::zvariant::{ObjectPath, Value};

use crate::engine::{Refused, Shared};

/// The object every MPRIS player lives at.
const PATH: &str = "/org/mpris/MediaPlayer2";

/// The bus name (`ronoctua_player::mpris_name`): the one a door that closes
/// an older player calls too.
fn bus_name() -> String {
    ronoctua_player::mpris_name()
}

/// `org.mpris.MediaPlayer2`.
struct Root {
    shared: Shared,
}

/// One order to the engine, as the socket would give it.
fn tell(shared: &Shared, ask: Ask) {
    let (lock, bell) = &**shared;
    if let Ok(mut e) = lock.lock() {
        let _ = e.ask(ask, shared);
        e.broadcast();
    }
    bell.notify_all();
}

/// The state, read from the engine.
fn read(shared: &Shared) -> State {
    let (lock, _) = &**shared;
    lock.lock().map(|e| e.state()).unwrap_or_default()
}

/// Puts what a listener named — a URI — at `place`, playing it when asked;
/// the ids it was given. Read from the disk BEFORE the engine's lock, as the
/// socket's `put` is.
fn put(shared: &Shared, uri: &str, place: Place, play: bool) -> Vec<EntryId> {
    let resolved =
        crate::engine::resolve_all(vec![Thing::Location(crate::engine::path_of_uri(uri))]);
    let (lock, bell) = &**shared;
    let ids = lock.lock().map_or_else(
        |_| Vec::new(),
        |mut e| {
            let ids = match e.put_at(resolved, place, play, false, shared) {
                Ok(ids) => ids,
                Err(Refused(t)) => {
                    e.say_trouble(t);
                    Vec::new()
                }
            };
            e.broadcast();
            ids
        },
    );
    bell.notify_all();
    ids
}

/// A leaf, as a listener is told of it.
#[derive(Debug, Clone, PartialEq)]
struct TrackInfo {
    id: EntryId,
    /// Its name: the queue's, else its file's — a set's is every source
    /// together, never one file's.
    title: String,
    /// What it plays, as an address.
    url: Option<String>,
    /// Its file, when it is one — for the cover beside it.
    file: Option<String>,
}

/// What a listener is shown, read from the engine in one go.
#[derive(Debug, Clone, Default)]
struct Look {
    state: State,
    tracks: Vec<TrackInfo>,
    able: Able,
    /// The cover of what plays, taken out of the file into the player's
    /// cache (`engine/tags.rs`).
    art: Option<String>,
}

fn look(shared: &Shared) -> Look {
    let (lock, _) = &**shared;
    let Ok(e) = lock.lock() else {
        return Look::default();
    };
    let state = e.state();
    let q = e.queue();
    Look {
        tracks: q
            .leaves()
            .into_iter()
            .filter_map(|id| track_info(q, id))
            .collect(),
        able: able(&state, q, e.order()),
        art: e.art_url(),
        state,
    }
}

/// A leaf of `q`, as a listener is told of it.
fn track_info(q: &Queue, id: EntryId) -> Option<TrackInfo> {
    let entry = q.find(id)?;
    let location = match &entry.body {
        Body::Single { location, .. } => Some(location.clone()),
        Body::Stream(stream) => Some(stream.url.clone()),
        Body::Set { set, .. } => set
            .lanes
            .iter()
            .find(|l| l.kind != LaneKind::Subtitle)
            .or_else(|| set.lanes.first())
            .map(|l| l.source.clone()),
        Body::Playlist { .. } => return None,
    };
    let file = location.clone().filter(|l| !l.contains("://"));
    Some(TrackInfo {
        id,
        title: crate::engine::name_text(&entry.body.name()),
        url: location.map(|l| {
            if l.contains("://") {
                l
            } else {
                format!("file://{l}")
            }
        }),
        file,
    })
}
/// The shell, to raise the column with: the unit names it
/// (`nix/modules/player.nix`), and the nested loop's shell hands its own path
/// to the daemon it starts (`client::ensure`).
fn the_shell() -> Option<std::path::PathBuf> {
    std::env::var_os("RONOCTUA_SHELL").map(std::path::PathBuf::from)
}

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    /// The column on the right, raised: it is where the player lives. A
    /// column that starts ends the one already there, so this also shows one
    /// that is up and out of sight.
    fn raise(&self) {
        if let Some(shell) = the_shell() {
            let _ = std::process::Command::new(shell)
                .arg("column")
                .stdin(std::process::Stdio::null())
                .spawn();
        }
    }
    /// Closes the player everywhere — the SAME path as the socket's `quit`
    /// (`engine::quit`): the queue remembered, the farewell to every door,
    /// and the process gone a moment AFTER the answer, since exiting inside
    /// the call would leave the caller waiting for a reply that never comes.
    /// The next thing played starts the player again (`client::ensure` asks
    /// the unit back), so quitting loses nothing but what was playing.
    fn quit(&self) {
        crate::engine::quit(&self.shared);
    }
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        the_shell().is_some()
    }
    #[zbus(property)]
    fn fullscreen(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn can_set_fullscreen(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn identity(&self) -> String {
        "ronoctua".to_owned()
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> String {
        "br.ronoctua.player".to_owned()
    }
    /// What libmpv, over the ffmpeg this build pins, opens by address.
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        URI_SCHEMES.map(str::to_owned).to_vec()
    }
    /// The same list the desktop entry carries, read from the file the
    /// module writes beside it: the specification wants each type NAMED, and a
    /// wildcard is not one.
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        supported_types()
    }
}

/// The addresses a listener may hand `OpenUri`.
const URI_SCHEMES: [&str; 7] = ["file", "http", "https", "ftp", "rtmp", "rtsp", "mms"];

/// The types the desktop entry names, from `$RONOCTUA_PLAYER_TYPES` (one per
/// line); the common ones when the file is not there — the nested loop.
fn supported_types() -> Vec<String> {
    if let Some(list) =
        std::env::var_os("RONOCTUA_PLAYER_TYPES").and_then(|p| std::fs::read_to_string(p).ok())
    {
        let named: Vec<String> = list
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        if !named.is_empty() {
            return named;
        }
    }
    [
        "audio/mpeg",
        "audio/flac",
        "audio/ogg",
        "audio/x-wav",
        "video/mp4",
        "video/x-matroska",
        "video/webm",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// `org.mpris.MediaPlayer2.Player`.
struct Player {
    shared: Shared,
}

impl Player {
    fn state(&self) -> State {
        read(&self.shared)
    }

    fn ask(&self, ask: Ask) {
        tell(&self.shared, ask);
    }
}

/// The id a track is known by on the bus: its entry's.
fn track_id(id: EntryId) -> Option<ObjectPath<'static>> {
    ObjectPath::try_from(format!("/br/ronoctua/player/track/{}", id.0)).ok()
}

/// The entry an id names, when it names one.
fn entry_of(id: &ObjectPath<'_>) -> Option<EntryId> {
    id.as_str()
        .strip_prefix("/br/ronoctua/player/track/")?
        .parse()
        .ok()
        .map(EntryId)
}

/// "No track": where `AddTrack` puts a track FIRST.
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";

/// The picture beside a file, for `mpris:artUrl`: the names a ripped album
/// carries its cover under — for a leaf that does not play. What plays is
/// given its OWN cover, taken out of the file into the player's cache
/// ([`metadata_of`]'s `art`): a picture inside a FLAC is not an address a
/// listener can open, and a copy of it is.
fn cover_beside(location: &str) -> Option<String> {
    if location.contains("://") {
        return None;
    }
    let folder = std::path::Path::new(location).parent()?;
    let names = std::fs::read_dir(folder).ok()?;
    let mut found: Option<std::path::PathBuf> = None;
    for entry in names.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let Some((stem, ext)) = name.rsplit_once('.') else {
            continue;
        };
        if ["jpg", "jpeg", "png", "webp"].contains(&ext)
            && ["cover", "folder", "front", "album", "albumart"].contains(&stem)
        {
            found = Some(entry.path());
            break;
        }
    }
    found.map(|p| format!("file://{}", p.display()))
}

/// What a listener may do now — each one false when the queue gives it
/// nothing to act on.
#[derive(Debug, Clone, Copy, Default)]
struct Able {
    next: bool,
    previous: bool,
    play: bool,
    pause: bool,
    seek: bool,
}

/// The queue's own answers — repeat and shuffle included — as what a
/// listener may do.
fn able(s: &State, q: &Queue, order: Option<&[EntryId]>) -> Able {
    Able {
        next: s.current.is_some() && q.next(s.current, s.repeat, Step::Asked, order).is_some(),
        previous: s.current.is_some() && q.previous(s.current, s.repeat, order).is_some(),
        play: q.leaf_count() > 0,
        pause: s.current.is_some(),
        seek: s.current.is_some() && s.duration.is_some(),
    }
}

/// Where a relative `Seek` lands, or `None` for "past the end" — which the
/// specification answers with the next track.
fn seek_to(s: &State, offset_us: i64) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    let to = (s.position + offset_us as f64 / 1_000_000.0).max(0.0);
    match s.duration {
        Some(d) if to > d => None,
        _ => Some(to),
    }
}

/// Seconds to MPRIS's microseconds.
#[allow(clippy::cast_possible_truncation)]
fn micros(seconds: f64) -> i64 {
    (seconds * 1_000_000.0).round() as i64
}

/// `Playing`, `Paused` or `Stopped`.
fn status_of(s: &State) -> &'static str {
    match (s.current.is_some(), s.playing) {
        (false, _) => "Stopped",
        (true, true) => "Playing",
        (true, false) => "Paused",
    }
}

/// `None`, `Track` or `Playlist`.
const fn loop_of(r: Repeat) -> &'static str {
    match r {
        Repeat::Nothing => "None",
        Repeat::One => "Track",
        Repeat::Queue => "Playlist",
    }
}

/// The metadata of what plays; `art`, its own cover kept in the cache, wins
/// over a picture beside it.
fn metadata_of(
    s: &State,
    tracks: &[TrackInfo],
    art: Option<&str>,
) -> HashMap<String, Value<'static>> {
    let Some(current) = s.current else {
        return HashMap::new();
    };
    let mut m = tracks
        .iter()
        .find(|t| t.id == current)
        .map(item_metadata)
        .unwrap_or_default();
    // THE STATE'S TITLE: the queue's name for it, else its own tag — and a
    // set's is every source together (`Engine::given_title`).
    if let Some(title) = s.title.clone() {
        m.insert("xesam:title".into(), Value::from(title));
    }
    if let Some(a) = &s.artist {
        m.insert("xesam:artist".into(), Value::from(vec![a.clone()]));
    }
    if let Some(a) = &s.album {
        m.insert("xesam:album".into(), Value::from(a.clone()));
    }
    if let Some(d) = s.duration {
        m.insert("mpris:length".into(), Value::from(micros(d)));
    }
    if let Some(art) = art {
        m.insert("mpris:artUrl".into(), Value::from(art.to_owned()));
    }
    m
}

/// What is known of ANY leaf of the queue — the track list asks this of
/// leaves that are not playing, whose tags the engine has not read.
fn item_metadata(t: &TrackInfo) -> HashMap<String, Value<'static>> {
    let mut m: HashMap<String, Value<'static>> = HashMap::new();
    if let Some(p) = track_id(t.id) {
        m.insert("mpris:trackid".into(), Value::from(p));
    }
    // THE NAME WHEN NOTHING ELSE: a listener draws the title, and a track list
    // of blank rows says less than a list of file names.
    m.insert("xesam:title".into(), Value::from(t.title.clone()));
    if let Some(url) = &t.url {
        m.insert("xesam:url".into(), Value::from(url.clone()));
    }
    if let Some(art) = t.file.as_deref().and_then(cover_beside) {
        m.insert("mpris:artUrl".into(), Value::from(art));
    }
    m
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) {
        self.ask(Ask::Next);
    }
    fn previous(&self) {
        self.ask(Ask::Previous);
    }
    fn pause(&self) {
        self.ask(Ask::Pause);
    }
    fn play_pause(&self) {
        self.ask(Ask::Toggle);
    }
    fn stop(&self) {
        self.ask(Ask::Stop);
    }
    fn play(&self) {
        self.ask(Ask::Play);
    }
    /// Before the start is the start, and past the end is the next track —
    /// the specification's two rules.
    fn seek(&self, offset: i64) {
        let Look { state: s, able, .. } = look(&self.shared);
        if !able.seek {
            return;
        }
        match seek_to(&s, offset) {
            Some(seconds) => self.ask(Ask::Seek {
                seconds,
                relative: false,
            }),
            None => self.ask(Ask::Next),
        }
    }
    /// Ignored unless it names the track that plays and a place inside it:
    /// a listener that raced a change of track must not move the new one.
    fn set_position(&self, track: ObjectPath<'_>, position: i64) {
        let s = self.state();
        if position < 0 || entry_of(&track) != s.current || s.current.is_none() {
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        let seconds = position as f64 / 1_000_000.0;
        if s.duration.is_some_and(|d| seconds > d) {
            return;
        }
        self.ask(Ask::Seek {
            seconds,
            relative: false,
        });
    }
    /// Plays now, right after what played, and the queue is kept — what
    /// opening from outside the player does everywhere.
    fn open_uri(&self, uri: &str) {
        put(&self.shared, uri, Place::End(None), true);
    }

    #[zbus(property)]
    fn playback_status(&self) -> String {
        status_of(&self.state()).to_owned()
    }
    #[zbus(property)]
    fn loop_status(&self) -> String {
        loop_of(self.state().repeat).to_owned()
    }
    #[zbus(property)]
    fn set_loop_status(&self, value: String) {
        // THE RING IS ONE STEP AT A TIME: turn it until it says what was asked.
        for _ in 0..3 {
            if loop_of(self.state().repeat) == value {
                return;
            }
            self.ask(Ask::Repeat);
        }
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.state().speed
    }
    /// Zero is a pause, as the specification allows; anything else is held
    /// between the two rates this player says it has.
    #[zbus(property)]
    fn set_rate(&self, value: f64) {
        if value <= 0.0 {
            self.ask(Ask::Pause);
        } else {
            self.ask(Ask::Speed {
                factor: value.clamp(MINIMUM_RATE, MAXIMUM_RATE),
            });
        }
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.state().shuffle
    }
    #[zbus(property)]
    fn set_shuffle(&self, value: bool) {
        if self.state().shuffle != value {
            self.ask(Ask::Shuffle);
        }
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        let l = look(&self.shared);
        metadata_of(&l.state, &l.tracks, l.art.as_deref())
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        f64::from(self.state().volume) / 100.0
    }
    #[zbus(property)]
    fn set_volume(&self, value: f64) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let percent = (value * 100.0)
            .round()
            .clamp(0.0, f64::from(ronoctua_player_max())) as u8;
        self.ask(Ask::Volume { percent });
    }
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        micros(self.state().position)
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        MINIMUM_RATE
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        MAXIMUM_RATE
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        look(&self.shared).able.next
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        look(&self.shared).able.previous
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        look(&self.shared).able.play
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        look(&self.shared).able.pause
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        look(&self.shared).able.seek
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn can_control(&self) -> bool {
        true
    }
}

/// The slowest and fastest the bus may ask for.
const MINIMUM_RATE: f64 = 0.25;
const MAXIMUM_RATE: f64 = 4.0;

/// `org.mpris.MediaPlayer2.TrackList`: the queue.
struct TrackList {
    shared: Shared,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl TrackList {
    fn get_tracks_metadata(
        &self,
        track_ids: Vec<ObjectPath<'_>>,
    ) -> Vec<HashMap<String, Value<'static>>> {
        let l = look(&self.shared);
        track_ids
            .iter()
            .filter_map(entry_of)
            .filter_map(|id| l.tracks.iter().find(|t| t.id == id))
            .map(|t| {
                if Some(t.id) == l.state.current {
                    metadata_of(&l.state, &l.tracks, l.art.as_deref())
                } else {
                    item_metadata(t)
                }
            })
            .collect()
    }
    /// Put where it was asked: right after `after_track`, first for "no
    /// track", and at the end for an id that names nothing — the listener's
    /// stale view of the list, and the end is the one place never wrong.
    fn add_track(&self, uri: &str, after_track: ObjectPath<'_>, set_as_current: bool) {
        let place = if after_track.as_str() == NO_TRACK {
            Place::Start(None)
        } else {
            let known = read_queue_has(&self.shared, entry_of(&after_track));
            known.map_or(Place::End(None), Place::After)
        };
        put(&self.shared, uri, place, set_as_current);
    }
    fn remove_track(&self, track_id: ObjectPath<'_>) {
        if let Some(id) = read_queue_has(&self.shared, entry_of(&track_id)) {
            tell(&self.shared, Ask::Remove { id });
        }
    }
    fn go_to(&self, track_id: ObjectPath<'_>) {
        if let Some(id) = read_queue_has(&self.shared, entry_of(&track_id)) {
            tell(&self.shared, Ask::Jump { id });
        }
    }
    /// The ids, announced by `TrackListReplaced` rather than by a property
    /// change — the specification says `invalidates`.
    #[zbus(property(emits_changed_signal = "invalidates"))]
    fn tracks(&self) -> Vec<ObjectPath<'static>> {
        look(&self.shared)
            .tracks
            .iter()
            .filter_map(|t| track_id(t.id))
            .collect()
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn can_edit_tracks(&self) -> bool {
        true
    }
}

/// `id`, when it names a leaf of the queue now.
fn read_queue_has(shared: &Shared, id: Option<EntryId>) -> Option<EntryId> {
    let id = id?;
    let (lock, _) = &**shared;
    let e = lock.lock().ok()?;
    e.queue().find(id).filter(|e| e.body.is_leaf()).map(|_| id)
}

/// The loudest the bus may set, the engine's own ceiling.
const fn ronoctua_player_max() -> u8 {
    130
}

/// What names a track to a listener: its entry, title, artist, album,
/// length and cover — a file whose tags were written keeps its entry and
/// changes the rest.
type TrackKey = (
    Option<EntryId>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<String>,
);

/// What a listener is told about when it changes.
#[derive(Debug, Clone, PartialEq)]
struct Watched {
    status: &'static str,
    track: Option<TrackKey>,
    volume: u8,
    shuffle: bool,
    repeat: Repeat,
    speed: f64,
    /// Next, previous, play, pause and seek.
    able: [bool; 5],
}

impl Watched {
    fn of(s: &State, a: Able, art: Option<&str>) -> Self {
        Self {
            status: status_of(s),
            track: Some((
                s.current,
                s.title.clone(),
                s.artist.clone(),
                s.album.clone(),
                s.duration,
                art.map(str::to_owned),
            )),
            volume: s.volume,
            shuffle: s.shuffle,
            repeat: s.repeat,
            speed: s.speed,
            able: [a.next, a.previous, a.play, a.pause, a.seek],
        }
    }
}

/// Puts the player on the session bus, and announces what changes. Returns
/// the sender the engine pokes on every broadcast; `None` with no bus, which
/// is not fatal — the player plays, and only MPRIS is missing.
pub fn start(shared: Shared) -> Option<std::sync::mpsc::Sender<()>> {
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.name(bus_name()))
        .and_then(|b| {
            b.serve_at(
                PATH,
                Root {
                    shared: shared.clone(),
                },
            )
        })
        .and_then(|b| {
            b.serve_at(
                PATH,
                TrackList {
                    shared: shared.clone(),
                },
            )
        })
        .and_then(|b| {
            b.serve_at(
                PATH,
                Player {
                    shared: shared.clone(),
                },
            )
        })
        .and_then(zbus::blocking::connection::Builder::build);
    let connection = match connection {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "no MPRIS: the media keys will not reach the player");
            return None;
        }
    };
    tracing::info!(name = %bus_name(), "on the session bus");
    let (poke, pokes) = std::sync::mpsc::channel::<()>();
    let _ = std::thread::Builder::new()
        .name("player-mpris".into())
        .spawn(move || {
            let mut last: Option<Watched> = None;
            let mut last_queue: Option<u64> = None;
            let mut last_position = 0.0f64;
            while pokes.recv().is_ok() {
                while pokes.try_recv().is_ok() {}
                let l = look(&shared);
                let s = &l.state;
                let now = Watched::of(s, l.able, l.art.as_deref());
                // A JUMP IS `Seeked`: further than a tick's worth of playing.
                let expected = last_position + if s.playing { 1.0 } else { 0.0 } * s.speed;
                let jumped = (s.position - expected).abs() > 1.5 && last.is_some();
                last_position = s.position;
                // THE QUEUE'S SHAPE CHANGED: the whole list is announced,
                // with the one playing.
                let queue = s.revision;
                if last_queue.as_ref() != Some(&queue) {
                    let tracks: Vec<ObjectPath<'static>> =
                        l.tracks.iter().filter_map(|t| track_id(t.id)).collect();
                    let current = s
                        .current
                        .and_then(track_id)
                        .or_else(|| ObjectPath::try_from(NO_TRACK).ok());
                    if let Some(current) = current {
                        let _ = connection.emit_signal(
                            None::<&str>,
                            PATH,
                            "org.mpris.MediaPlayer2.TrackList",
                            "TrackListReplaced",
                            &(tracks, current),
                        );
                    }
                    last_queue = Some(queue);
                }
                if last.as_ref() == Some(&now) && !jumped {
                    continue;
                }
                let Ok(iface) = connection.object_server().interface::<_, Player>(PATH) else {
                    continue;
                };
                let emitter = iface.signal_emitter();
                let player = iface.get();
                zbus::block_on(async {
                    let _ = player.playback_status_changed(emitter).await;
                    let _ = player.metadata_changed(emitter).await;
                    let _ = player.volume_changed(emitter).await;
                    let _ = player.shuffle_changed(emitter).await;
                    let _ = player.loop_status_changed(emitter).await;
                    let _ = player.rate_changed(emitter).await;
                    let _ = player.can_go_next_changed(emitter).await;
                    let _ = player.can_go_previous_changed(emitter).await;
                    let _ = player.can_play_changed(emitter).await;
                    let _ = player.can_pause_changed(emitter).await;
                    let _ = player.can_seek_changed(emitter).await;
                });
                if jumped {
                    let _ = connection.emit_signal(
                        None::<&str>,
                        PATH,
                        "org.mpris.MediaPlayer2.Player",
                        "Seeked",
                        &(micros(s.position),),
                    );
                }
                last = Some(now);
            }
        });
    Some(poke)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_player::own::Item;

    fn single(location: &str) -> Item {
        Item::Single {
            location: location.to_owned(),
            title: None,
            seconds: None,
        }
    }

    /// Three files, and a state with the first playing.
    fn three() -> (Queue, Vec<EntryId>, State) {
        let mut q = Queue::new();
        let ids = q
            .insert(
                Place::End(None),
                vec![
                    single("/m/a.flac"),
                    single("/m/b.flac"),
                    single("/m/c.flac"),
                ],
            )
            .expect("inserted");
        let s = State {
            current: Some(ids[0]),
            duration: Some(100.0),
            position: 95.0,
            ..State::default()
        };
        (q, ids, s)
    }

    #[test]
    fn the_status_and_the_loop_are_the_specifications_words() {
        let mut s = State::default();
        assert_eq!(status_of(&s), "Stopped");
        s.current = Some(EntryId(1));
        assert_eq!(status_of(&s), "Paused");
        s.playing = true;
        assert_eq!(status_of(&s), "Playing");
        assert_eq!(loop_of(Repeat::Queue), "Playlist");
        assert_eq!(loop_of(Repeat::One), "Track");
    }

    #[test]
    fn metadata_carries_a_track_id_and_microseconds() {
        let (q, ids, mut s) = three();
        s.title = Some("A".into());
        s.artist = Some("B".into());
        s.duration = Some(2.5);
        let tracks: Vec<TrackInfo> = q
            .leaves()
            .into_iter()
            .filter_map(|i| track_info(&q, i))
            .collect();
        let m = metadata_of(&s, &tracks, None);
        assert_eq!(m.get("mpris:length"), Some(&Value::from(2_500_000i64)));
        assert_eq!(m.get("xesam:url"), Some(&Value::from("file:///m/a.flac")));
        assert_eq!(m.get("xesam:title"), Some(&Value::from("A")));
        assert_eq!(
            m.get("mpris:trackid"),
            Some(&Value::from(track_id(ids[0]).expect("an id")))
        );
        assert!(metadata_of(&State::default(), &tracks, None).is_empty());
        // ITS OWN COVER, kept in the cache, is what a listener is handed.
        let m = metadata_of(&s, &tracks, Some("file:///c/player/cover-1.jpg"));
        assert_eq!(
            m.get("mpris:artUrl"),
            Some(&Value::from("file:///c/player/cover-1.jpg"))
        );
    }

    /// FAILS IF: a listener is told it can go somewhere the queue does not
    /// reach — every one of these was `true` in any state.
    #[test]
    fn what_a_listener_can_do_follows_the_queue() {
        let (q, ids, mut s) = three();
        let a = able(&s, &q, None);
        assert!(a.next && !a.previous && a.play && a.pause && a.seek);
        s.current = Some(ids[2]);
        assert!(!able(&s, &q, None).next);
        s.repeat = Repeat::Queue;
        assert!(able(&s, &q, None).next, "a looping queue always has a next");
        let a = able(&State::default(), &Queue::new(), None);
        assert!(!a.next && !a.previous && !a.play && !a.pause && !a.seek);
        s.duration = None;
        assert!(
            !able(&s, &q, None).seek,
            "a stream with no length does not seek"
        );
        // SHUFFLED, what comes next is the order's: the last of the tree may
        // have one.
        s.repeat = Repeat::Nothing;
        let order = vec![ids[2], ids[0], ids[1]];
        assert!(able(&s, &q, Some(&order)).next);
    }

    /// FAILS IF: the two rules of `Seek` stop holding: before the start is
    /// the start, past the end is the next track.
    #[test]
    fn a_seek_stops_at_the_start_and_past_the_end_is_the_next() {
        let (_, _, s) = three();
        assert_eq!(seek_to(&s, -200_000_000), Some(0.0));
        assert_eq!(seek_to(&s, 2_000_000), Some(97.0));
        assert_eq!(seek_to(&s, 10_000_000), None);
    }

    /// FAILS IF: a track's id stops naming its entry — and so stops naming
    /// the same track once the queue is reordered — or an id from some other
    /// player is read as one of ours.
    #[test]
    fn a_track_id_is_its_entry_and_survives_a_move() {
        let (mut q, ids, _) = three();
        let id = track_id(ids[2]).expect("an id");
        q.move_to(ids[2], Place::Start(None)).expect("moved");
        assert_eq!(entry_of(&id), Some(ids[2]));
        assert_eq!(q.leaves()[0], ids[2]);
        let other = ObjectPath::try_from(NO_TRACK).expect("a path");
        assert_eq!(entry_of(&other), None);
    }

    /// FAILS IF: a set reaches a listener under the name of one of its
    /// files.
    #[test]
    fn a_set_is_never_titled_after_one_file() {
        use ronoctua_player::own::{Lane, LaneId, Set};
        let mut q = Queue::new();
        let mut set = Set::default();
        set.lanes
            .push(Lane::new(LaneId(1), LaneKind::Video, "/f/film.mkv"));
        set.lanes
            .push(Lane::new(LaneId(2), LaneKind::Audio, "/f/commentary.opus"));
        let ids = q
            .insert(Place::End(None), vec![Item::Set { set, from: None }])
            .expect("inserted");
        let t = track_info(&q, ids[0]).expect("a track");
        assert_eq!(t.title, "film.mkv + commentary.opus");
        assert_eq!(t.url.as_deref(), Some("file:///f/film.mkv"));
    }

    /// FAILS IF: an item that is not playing reaches the track list with no
    /// title, or a cover beside it is not offered.
    #[test]
    fn an_item_not_playing_is_named_and_shows_its_cover() {
        let dir = std::env::temp_dir().join(format!("ronoctua-mpris-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cover.JPG"), b"").unwrap();
        let mut q = Queue::new();
        let ids = q
            .insert(
                Place::End(None),
                vec![single(&dir.join("01 One.flac").to_string_lossy())],
            )
            .expect("inserted");
        let m = item_metadata(&track_info(&q, ids[0]).expect("a track"));
        assert_eq!(m.get("xesam:title"), Some(&Value::from("01 One.flac")));
        let art = format!("file://{}", dir.join("Cover.JPG").display());
        assert_eq!(m.get("mpris:artUrl"), Some(&Value::from(art)));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
