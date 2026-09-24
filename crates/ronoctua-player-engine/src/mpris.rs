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
//! A TRACK'S ID IS ITS PLACE in the queue (`/br/ronoctua/player/track/<n>`),
//! which moves when the queue does. The specification allows that, on one
//! condition this module keeps: any change to the queue is announced as
//! `TrackListReplaced`, so a listener never holds an id that now names
//! another track.
//!
//! Every property is read from the engine when asked — nothing is copied —
//! and the ones a listener watches (status, metadata, volume, the two
//! switches) are announced when they CHANGE: the engine tells this thread on
//! every broadcast, four times a second while something plays, and a
//! position alone is not news (the specification says so: `Position` is
//! polled, and a jump is the `Seeked` signal).

use std::collections::HashMap;

use ronoctua_player::wire::{Ask, Repeat, State};
use zbus::zvariant::{ObjectPath, Value};

use crate::engine::Shared;

/// The object every MPRIS player lives at.
const PATH: &str = "/org/mpris/MediaPlayer2";

/// The bus name: `org.mpris.MediaPlayer2.<instance>`, so the nested loop's
/// player and the installed one are two players on the bus and not a fight
/// over one name.
fn bus_name() -> String {
    format!(
        "org.mpris.MediaPlayer2.{}",
        ronoctua_xdg::name().replace('-', "_")
    )
}

/// `org.mpris.MediaPlayer2`.
struct Root {
    shared: Shared,
}

impl Root {
    fn ask(&self, ask: Ask) {
        tell(&self.shared, ask);
    }
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
    /// Stops and goes. The next thing played starts the player again
    /// (`client::ensure` asks the unit back), so quitting loses nothing but
    /// what was playing.
    fn quit(&self) {
        self.ask(Ask::Stop);
        // AFTER THE ANSWER: exiting inside the call would leave the caller
        // waiting for a reply that never comes.
        let _ = std::thread::Builder::new()
            .name("player-quit".into())
            .spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(200));
                std::process::exit(0);
            });
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

/// The id a track is known by on the bus: its place in the queue.
fn track_id(i: usize) -> Option<ObjectPath<'static>> {
    ObjectPath::try_from(format!("/br/ronoctua/player/track/{i}")).ok()
}

/// The place an id names, when it names one.
fn place_of(id: &ObjectPath<'_>) -> Option<usize> {
    id.as_str()
        .strip_prefix("/br/ronoctua/player/track/")?
        .parse()
        .ok()
}

/// "No track": where `AddTrack` puts a track FIRST.
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";

/// A `file://` address, back to the path it names — `%20` and its kin
/// decoded, since a listener sends a URI and the engine opens paths.
fn path_of(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("file://") else {
        return uri.to_owned();
    };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(Some(h)), Some(Some(l))) = (
                bytes.get(i + 1).map(|b| hex(*b)),
                bytes.get(i + 2).map(|b| hex(*b)),
            )
        {
            out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The picture beside a file, for `mpris:artUrl`: the names a ripped album
/// carries its cover under. Nothing is extracted from the file itself — a
/// picture inside a FLAC is not an address a listener can open.
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

/// What a listener may do now, from the state — each one false when the
/// queue gives it nothing to act on.
struct Able {
    next: bool,
    previous: bool,
    play: bool,
    pause: bool,
    seek: bool,
}

fn able(s: &State) -> Able {
    let looping = s.repeat == Repeat::Queue && !s.queue.is_empty();
    Able {
        next: s.current.is_some_and(|i| i + 1 < s.queue.len() || looping),
        previous: s.current.is_some_and(|i| i > 0 || looping),
        play: !s.queue.is_empty(),
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

/// The metadata of what plays.
fn metadata_of(s: &State) -> HashMap<String, Value<'static>> {
    let Some(i) = s.current else {
        return HashMap::new();
    };
    let mut m = item_metadata(s, i);
    if let Some(item) = s.queue.get(i)
        && let Some(title) = s.title.clone().or_else(|| item.title.clone())
    {
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
    m
}

/// What is known of ANY item of the queue — the track list asks this of
/// items that are not playing, whose tags the engine has not read.
fn item_metadata(s: &State, i: usize) -> HashMap<String, Value<'static>> {
    let mut m: HashMap<String, Value<'static>> = HashMap::new();
    let Some(item) = s.queue.get(i) else {
        return m;
    };
    if let Some(p) = track_id(i) {
        m.insert("mpris:trackid".into(), Value::from(p));
    }
    // THE NAME WHEN NOTHING ELSE: a listener draws the title, and a track list
    // of blank rows says less than a list of file names.
    let title = item.title.clone().unwrap_or_else(|| {
        std::path::Path::new(&item.location)
            .file_name()
            .map_or_else(
                || item.location.clone(),
                |n| n.to_string_lossy().into_owned(),
            )
    });
    m.insert("xesam:title".into(), Value::from(title));
    let url = if item.location.contains("://") {
        item.location.clone()
    } else {
        format!("file://{}", item.location)
    };
    m.insert("xesam:url".into(), Value::from(url));
    if let Some(art) = cover_beside(&item.location) {
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
        let s = self.state();
        if !able(&s).seek {
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
        if position < 0 || place_of(&track) != s.current || s.current.is_none() {
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
    fn open_uri(&self, uri: &str) {
        self.ask(Ask::Open {
            items: vec![path_of(uri)],
            replace: true,
        });
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
        metadata_of(&self.state())
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
        able(&self.state()).next
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        able(&self.state()).previous
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        able(&self.state()).play
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        able(&self.state()).pause
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        able(&self.state()).seek
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
        let s = read(&self.shared);
        track_ids
            .iter()
            .filter_map(place_of)
            .filter(|&i| i < s.queue.len())
            .map(|i| {
                if Some(i) == s.current {
                    metadata_of(&s)
                } else {
                    item_metadata(&s, i)
                }
            })
            .collect()
    }
    /// Appended, then moved to where it was asked: after `after_track`, or
    /// first for "no track".
    fn add_track(&self, uri: &str, after_track: ObjectPath<'_>, set_as_current: bool) {
        let s = read(&self.shared);
        let appended = s.queue.len();
        let wanted = if after_track.as_str() == NO_TRACK {
            0
        } else {
            match place_of(&after_track).filter(|&i| i < appended) {
                Some(i) => i + 1,
                // AN ID THAT NAMES NOTHING is the listener's stale view of
                // the list; the end is the one place that is never wrong.
                None => appended,
            }
        };
        tell(
            &self.shared,
            Ask::Open {
                items: vec![path_of(uri)],
                replace: false,
            },
        );
        if wanted != appended {
            tell(
                &self.shared,
                Ask::Move {
                    from: appended,
                    to: wanted,
                },
            );
        }
        if set_as_current {
            tell(&self.shared, Ask::Jump { index: wanted });
        }
    }
    fn remove_track(&self, track_id: ObjectPath<'_>) {
        if let Some(index) = place_of(&track_id).filter(|&i| i < read(&self.shared).queue.len()) {
            tell(&self.shared, Ask::Remove { index });
        }
    }
    fn go_to(&self, track_id: ObjectPath<'_>) {
        if let Some(index) = place_of(&track_id).filter(|&i| i < read(&self.shared).queue.len()) {
            tell(&self.shared, Ask::Jump { index });
        }
    }
    /// The ids, announced by `TrackListReplaced` rather than by a property
    /// change — the specification says `invalidates`.
    #[zbus(property(emits_changed_signal = "invalidates"))]
    fn tracks(&self) -> Vec<ObjectPath<'static>> {
        (0..read(&self.shared).queue.len())
            .filter_map(track_id)
            .collect()
    }
    #[zbus(property(emits_changed_signal = "const"))]
    fn can_edit_tracks(&self) -> bool {
        true
    }
}

/// The loudest the bus may set, the engine's own ceiling.
const fn ronoctua_player_max() -> u8 {
    130
}

/// What names a track to a listener: its place, title, artist and length.
type TrackKey = (Option<usize>, Option<String>, Option<String>, Option<f64>);

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
    fn of(s: &State) -> Self {
        let a = able(s);
        Self {
            status: status_of(s),
            track: Some((s.current, s.title.clone(), s.artist.clone(), s.duration)),
            volume: s.volume,
            shuffle: s.shuffle,
            repeat: s.repeat,
            speed: s.speed,
            able: [a.next, a.previous, a.play, a.pause, a.seek],
        }
    }
}

/// The queue as a listener knows it: where each item is, and which plays.
fn queue_of(s: &State) -> Vec<String> {
    s.queue.iter().map(|i| i.location.clone()).collect()
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
            let mut last_queue: Option<Vec<String>> = None;
            let mut last_position = 0.0f64;
            while pokes.recv().is_ok() {
                while pokes.try_recv().is_ok() {}
                let s = {
                    let (lock, _) = &*shared;
                    let Ok(e) = lock.lock() else { return };
                    e.state()
                };
                let now = Watched::of(&s);
                // A JUMP IS `Seeked`: further than a tick's worth of playing.
                let expected = last_position + if s.playing { 1.0 } else { 0.0 } * s.speed;
                let jumped = (s.position - expected).abs() > 1.5 && last.is_some();
                last_position = s.position;
                // THE QUEUE CHANGED: every id may name another track now, so
                // the whole list is announced, with the one playing.
                let queue = queue_of(&s);
                if last_queue.as_ref() != Some(&queue) {
                    let tracks: Vec<ObjectPath<'static>> =
                        (0..queue.len()).filter_map(track_id).collect();
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
    use ronoctua_player::wire::Item;

    #[test]
    fn the_status_and_the_loop_are_the_specifications_words() {
        let mut s = State::default();
        assert_eq!(status_of(&s), "Stopped");
        s.current = Some(0);
        assert_eq!(status_of(&s), "Paused");
        s.playing = true;
        assert_eq!(status_of(&s), "Playing");
        assert_eq!(loop_of(Repeat::Queue), "Playlist");
        assert_eq!(loop_of(Repeat::One), "Track");
    }

    #[test]
    fn metadata_carries_a_track_id_and_microseconds() {
        let s = State {
            queue: vec![Item {
                location: "/m/a.flac".into(),
                title: None,
            }],
            current: Some(0),
            title: Some("A".into()),
            artist: Some("B".into()),
            duration: Some(2.5),
            ..State::default()
        };
        let m = metadata_of(&s);
        assert_eq!(m.get("mpris:length"), Some(&Value::from(2_500_000i64)));
        assert_eq!(m.get("xesam:url"), Some(&Value::from("file:///m/a.flac")));
        assert!(m.contains_key("mpris:trackid"));
        assert!(metadata_of(&State::default()).is_empty());
    }

    fn queue_of_three() -> State {
        State {
            queue: ["/m/a.flac", "/m/b.flac", "/m/c.flac"]
                .map(|l| Item {
                    location: l.into(),
                    title: None,
                })
                .to_vec(),
            current: Some(0),
            duration: Some(100.0),
            position: 95.0,
            ..State::default()
        }
    }

    /// FAILS IF: a listener is told it can go somewhere the queue does not
    /// reach — every one of these was `true` in any state.
    #[test]
    fn what_a_listener_can_do_follows_the_queue() {
        let mut s = queue_of_three();
        let a = able(&s);
        assert!(a.next && !a.previous && a.play && a.pause && a.seek);
        s.current = Some(2);
        assert!(!able(&s).next);
        s.repeat = Repeat::Queue;
        assert!(able(&s).next, "a looping queue always has a next");
        let empty = State::default();
        let a = able(&empty);
        assert!(!a.next && !a.previous && !a.play && !a.pause && !a.seek);
        s.duration = None;
        assert!(!able(&s).seek, "a stream with no length does not seek");
    }

    /// FAILS IF: the two rules of `Seek` stop holding: before the start is
    /// the start, past the end is the next track.
    #[test]
    fn a_seek_stops_at_the_start_and_past_the_end_is_the_next() {
        let s = queue_of_three();
        assert_eq!(seek_to(&s, -200_000_000), Some(0.0));
        assert_eq!(seek_to(&s, 2_000_000), Some(97.0));
        assert_eq!(seek_to(&s, 10_000_000), None);
    }

    /// FAILS IF: a track's id stops naming its place, or an id from some
    /// other player is read as one of ours.
    #[test]
    fn a_track_id_is_its_place_and_nothing_else_is() {
        let id = track_id(7).unwrap();
        assert_eq!(place_of(&id), Some(7));
        let other = ObjectPath::try_from(NO_TRACK).unwrap();
        assert_eq!(place_of(&other), None);
    }

    /// FAILS IF: an address from a listener reaches the engine still
    /// escaped, and names a file that does not exist.
    #[test]
    fn a_file_address_is_the_path_it_names() {
        assert_eq!(path_of("file:///m/a%20b.flac"), "/m/a b.flac");
        assert_eq!(path_of("https://x.org/a%20b"), "https://x.org/a%20b");
    }

    /// FAILS IF: an item that is not playing reaches the track list with no
    /// title, or a cover beside it is not offered.
    #[test]
    fn an_item_not_playing_is_named_and_shows_its_cover() {
        let dir = std::env::temp_dir().join(format!("ronoctua-mpris-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cover.JPG"), b"").unwrap();
        let s = State {
            queue: vec![Item {
                location: dir.join("01 One.flac").to_string_lossy().into_owned(),
                title: None,
            }],
            ..State::default()
        };
        let m = item_metadata(&s, 0);
        assert_eq!(m.get("xesam:title"), Some(&Value::from("01 One.flac")));
        let art = format!("file://{}", dir.join("Cover.JPG").display());
        assert_eq!(m.get("mpris:artUrl"), Some(&Value::from(art)));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
