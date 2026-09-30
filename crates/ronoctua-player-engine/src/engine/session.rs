//! What plays together: the lanes, the voices that play them, and the one
//! clock they keep.
//!
//! # Three ways to play
//!
//! - A file that plays ALONE plays in the first voice, through the queue's
//!   window of two (`queue.rs`): gapless, and mpv's own choice of tracks. Its
//!   lanes are read off its tracks ([`implicit_set`]), and the first change
//!   to one of them makes it a set — the same entry, the same id, the same
//!   file still playing in the same voice.
//! - A SET plays in as many voices as [`session::voices`] groups it into,
//!   one mpv each, on the set's clock ([`Clock`]).
//! - Another machine's lanes, while this one plays in step with it: the same
//!   voices, on that machine's clock. This machine's queue is not touched.
//!
//! # The clock
//!
//! The first sound that plays keeps the time ([`session::master`]); with no
//! sound playing — before the first starts, in a silent gap, after the last
//! — the engine's own clock does. A voice that should be somewhere is seeked
//! there while it settles (a load or a seek just happened) and nudged after
//! that: a picture by five percent, a sound by two (`sync::correct_as`). A
//! lane before its start is held paused at its first frame; a lane past its
//! end is held paused at its last, and the set ends with its last picture or
//! sound.
//!
//! # Which voice takes which part
//!
//! [`Engine::cast`] gives each part the voice that already holds its source —
//! a film that becomes a set keeps playing where it was, and moving a lane in
//! time never reloads it — then the first voice, then any other, and only
//! then a new one. A voice nobody needs is retired, off the lock.
//!
//! # Ticks, only while they are needed
//!
//! A clock thread runs while two voices play, a lane waits for its start, or
//! the engine's own clock keeps the time ([`Engine::needs_ticks`]). One voice
//! alone keeping the time needs none: its own position events move the set.

use std::time::{Duration, Instant};

use ronoctua_player::{
    arrange,
    limit::{Limit, TooMany},
    own::{Arrangement, Item, Lane, LaneId, LaneKind, Set},
    queue::{Body, EntryId, Step},
    remote::{self, Moment},
    session::{self, Master, When},
    sync::{self, Correction, Follower},
    wire::{LaneChange, LaneState, LaneStatus, Source, Track, TrackKind, Trouble},
};

use super::{Engine, Refused, voice};
use crate::mpv::Mpv;

/// How close to the engine's own clock a sound must be before it takes the
/// time over: a sound that has not landed where it was sent would pull every
/// other voice back to where it WAS.
const TAKES_OVER_SECONDS: f64 = sync::SETTLE_MS / 1000.0;

/// Whose lanes play.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Whose {
    /// A set of this machine's queue.
    Here(EntryId),
    /// Another machine's lanes, as this one plays them.
    There {
        /// The other machine's leaf.
        item: Option<u64>,
        /// Its lanes, each opened here.
        lanes: Vec<Lane>,
        /// The sources that are the other machine's files: only those are
        /// asked with the key.
        keyed: Vec<String>,
        /// The header the key travels in.
        header: String,
    },
}

/// Who keeps the time.
#[derive(Debug, Clone)]
pub(super) enum Clock {
    /// This lane's voice: the set is at `start + (position − from)`.
    Lane(LaneId),
    /// The engine's own: `at`, and the time since `since` while it plays.
    Virtual {
        /// Where the set was at `since`.
        at: f64,
        /// When.
        since: Instant,
    },
    /// Another machine's, measured against this one's.
    There {
        /// What it last said.
        moment: Box<Moment>,
        /// Its clock's offset from this one's (`sync::offset`).
        offset_ms: f64,
    },
}

/// What plays together now.
#[derive(Debug)]
pub(super) struct Session {
    /// Whose lanes.
    pub whose: Whose,
    /// Who keeps the time.
    pub clock: Clock,
    /// It plays.
    pub playing: bool,
    /// At what speed.
    pub speed: f64,
    /// A clock thread runs for it.
    pub ticking: bool,
    /// Its number: a clock thread of an older session leaves.
    pub generation: u64,
    /// When it was last looked at.
    pub ticked: Instant,
    /// The set's time, as last read.
    pub at: f64,
}

impl Session {
    fn new(
        whose: Whose,
        clock: Clock,
        playing: bool,
        speed: f64,
        generation: u64,
        at: f64,
    ) -> Self {
        Self {
            whose,
            clock,
            playing,
            speed,
            ticking: false,
            generation,
            ticked: Instant::now(),
            at,
        }
    }
}

/// A file that plays alone, as the set it would become: its picture, its
/// sound and its subtitles, each the track mpv chose, and its cover shown.
///
/// Lanes 1, 2 and 3, always those: a door that asked about lane 2 of a file
/// is asking about its sound whichever file it is.
#[must_use]
pub(super) fn implicit_set(location: &str, title: Option<&str>, tracks: &[Track]) -> Set {
    let mut set = Set::default();
    let selected = |kind: TrackKind| {
        tracks
            .iter()
            .find(|t| t.kind == kind && t.selected && !t.albumart)
            .map(|t| t.id)
    };
    let lane = |id: u32, kind: LaneKind, track: Option<i64>, titled: bool| {
        let mut lane = Lane::new(LaneId(id), kind, location);
        lane.track = track;
        lane.enabled = track.is_some();
        if titled {
            lane.title = title.map(str::to_owned);
        }
        lane
    };
    if tracks
        .iter()
        .any(|t| t.kind == TrackKind::Video && !t.albumart)
    {
        set.lanes
            .push(lane(1, LaneKind::Video, selected(TrackKind::Video), true));
    }
    if tracks.iter().any(|t| t.kind == TrackKind::Audio) {
        set.lanes
            .push(lane(2, LaneKind::Audio, selected(TrackKind::Audio), true));
        if tracks
            .iter()
            .any(|t| t.kind == TrackKind::Video && t.albumart)
        {
            set.arrangement.covers.push(LaneId(2));
        }
    }
    if tracks.iter().any(|t| t.kind == TrackKind::Sub) {
        set.lanes
            .push(lane(3, LaneKind::Subtitle, selected(TrackKind::Sub), false));
    }
    set
}

/// What a voice is told for its part of a set — every property but the
/// subtitles, which are its picture's (`subtitles.rs`).
///
/// `device` is the lane's output as mpv names it, when it is there, else
/// `auto` (the machine's default).
#[must_use]
pub(super) fn dress_of(
    part: &session::Voice,
    lanes: &[Lane],
    covers: &[LaneId],
    (volume, muted): (u8, bool),
    device: Option<String>,
) -> Vec<(&'static str, String)> {
    let lane = |id: Option<LaneId>| id.and_then(|id| lanes.iter().find(|l| l.id == id));
    let track = |l: &Lane| l.track.map_or_else(|| "auto".to_owned(), |t| t.to_string());
    let sound = lane(part.audio);
    let vid = match (lane(part.video), sound) {
        (Some(picture), _) => track(picture),
        // A SOUND SHOWS ITS COVER only when the person chose it.
        (None, Some(s)) if covers.contains(&s.id) => "auto".to_owned(),
        _ => "no".to_owned(),
    };
    let aid = sound.map_or_else(|| "no".to_owned(), track);
    let own = sound.map_or(100, |s| s.volume_percent);
    let loud = (u16::from(volume) * u16::from(own) / 100)
        .min(u16::from(ronoctua_player::own::VOLUME_MOST_PERCENT));
    let mute = muted || sound.is_some_and(|s| s.muted);
    vec![
        // KEPT OPEN at its end: a lane that ended shows its last frame, and
        // the set, not mpv, says what comes next.
        ("keep-open", "yes".to_owned()),
        // THE SET REPEATS, never one of its files by itself.
        ("loop-file", "no".to_owned()),
        ("vid", vid),
        ("aid", aid),
        (
            "audio-device",
            device.map_or_else(|| "auto".to_owned(), |d| format!("pipewire/{d}")),
        ),
        ("volume", loud.to_string()),
        ("mute", if mute { "yes" } else { "no" }.to_owned()),
    ]
}

/// Where a voice should be sent when the set is at `when` of `lane`, whose
/// source runs `length` (for a lane already past its end: its last moment).
fn target_of(when: When, lane: &Lane, length: Option<f64>) -> f64 {
    match when {
        When::Before => lane.from_seconds,
        When::At { seconds } => seconds,
        When::After => lane
            .until_seconds
            .or(length)
            .map_or(lane.from_seconds, |end| (end - 0.05).max(lane.from_seconds)),
    }
}

/// How long a voice's source runs, as far as the set is concerned: its
/// length, or — once it is at its end, kept open on its last frame — where
/// it stopped, which is a frame short of it. A sound that stopped keeps no
/// time, and a set whose last lane stopped is over.
fn voice_length(v: &voice::Voice) -> Option<f64> {
    if v.eof {
        return v.mpv.float("time-pos").or(v.duration);
    }
    v.duration
}

/// `mpv seek <to> absolute+exact`.
fn seek(mpv: &Mpv, to: f64) {
    let _ = mpv.command(&["seek", &format!("{to:.3}"), "absolute+exact"]);
}

/// A change to one lane, as the set says it — refused with the reason when
/// the lane is not one it applies to, or the value is out of its range.
fn apply_change(set: &mut Set, lane: LaneId, change: LaneChange) -> Result<(), Trouble> {
    let count = set.lanes.len();
    let l = set.lane_mut(lane).ok_or(Trouble::NoSuchLane { lane })?;
    let bad = Trouble::BadValue { lane };
    let wrong = Trouble::WrongKind { lane };
    let seconds = |s: f64| s.is_finite() && s >= 0.0;
    match change {
        LaneChange::Start { seconds: s } => {
            if !seconds(s) {
                return Err(bad);
            }
            l.start_seconds = s;
        }
        LaneChange::Trim {
            from_seconds,
            until_seconds,
        } => {
            if !seconds(from_seconds)
                || until_seconds.is_some_and(|u| !u.is_finite() || u <= from_seconds)
            {
                return Err(bad);
            }
            l.from_seconds = from_seconds;
            l.until_seconds = until_seconds;
        }
        LaneChange::Enabled { on } => l.enabled = on,
        LaneChange::Volume { percent } => {
            if percent > ronoctua_player::own::VOLUME_MOST_PERCENT {
                return Err(bad);
            }
            l.volume_percent = percent;
        }
        LaneChange::Mute { muted } => l.muted = muted,
        LaneChange::Screens { screens } => {
            if l.kind == LaneKind::Subtitle {
                return Err(wrong);
            }
            l.screens = screens;
        }
        LaneChange::Sink { device } => {
            if l.kind != LaneKind::Audio {
                return Err(wrong);
            }
            l.sink = device.filter(|d| !d.trim().is_empty());
        }
        LaneChange::Stack { stack } => {
            if l.kind != LaneKind::Subtitle {
                return Err(wrong);
            }
            l.stack = stack;
        }
        LaneChange::Over { over } => {
            if l.kind != LaneKind::Subtitle {
                return Err(wrong);
            }
            l.over = over;
        }
        LaneChange::Title { title } => {
            l.title = title.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        }
        LaneChange::Remove => {
            if count == 1 {
                return Err(Trouble::LastLane);
            }
            set.lanes.retain(|l| l.id != lane);
            // WORDS OVER A PICTURE THAT WENT go over the first one left.
            for l in &mut set.lanes {
                if l.over == Some(lane) {
                    l.over = None;
                }
            }
        }
    }
    Ok(())
}

/// A new arrangement for `set`, refused when it names a lane that is not
/// there or one that cannot be used that way.
fn apply_arrangement(set: &mut Set, arrangement: Arrangement) -> Result<(), Trouble> {
    let kind_of = |id: LaneId| {
        set.lane(id)
            .map(|l| l.kind)
            .ok_or(Trouble::NoSuchLane { lane: id })
    };
    for &id in &arrangement.order {
        kind_of(id)?;
    }
    for &id in &arrangement.hidden {
        if kind_of(id)? == LaneKind::Subtitle {
            return Err(Trouble::WrongKind { lane: id });
        }
    }
    for &id in &arrangement.covers {
        if kind_of(id)? != LaneKind::Audio {
            return Err(Trouble::WrongKind { lane: id });
        }
    }
    set.arrangement = arrangement;
    Ok(())
}

impl Engine {
    /// The lanes of what plays, and how its pictures share a screen.
    pub(super) fn lanes_now(&self) -> (Vec<Lane>, Option<Arrangement>) {
        match self.session.as_ref().map(|s| &s.whose) {
            Some(Whose::There { lanes, .. }) => (lanes.clone(), None),
            Some(Whose::Here(id)) => match self.queue.find(*id).map(|e| &e.body) {
                Some(Body::Set { set, .. }) => (set.lanes.clone(), Some(set.arrangement.clone())),
                _ => (Vec::new(), None),
            },
            None => self
                .plain_set()
                .map_or_else(|| (Vec::new(), None), |s| (s.lanes, Some(s.arrangement))),
        }
    }

    /// The file that plays alone in the first voice, as the set it would
    /// become ([`implicit_set`]); `None` with nothing of this machine's queue
    /// loaded.
    pub(super) fn plain_set(&self) -> Option<Set> {
        if self.following.is_some() || self.session.is_some() {
            return None;
        }
        let current = self.state.current?;
        let (location, title) = match &self.queue.find(current)?.body {
            Body::Single {
                location, title, ..
            } => (location.clone(), title.clone()),
            Body::Stream(stream) => (self.location_of_leaf(current)?, stream.title.clone()),
            Body::Set { .. } | Body::Playlist { .. } => return None,
        };
        let voice = self.voices.first()?;
        if voice.loaded.as_deref() != Some(location.as_str()) {
            return None;
        }
        Some(implicit_set(&location, title.as_deref(), &voice.tracks))
    }

    /// The voice that plays `lane`.
    fn voice_of(&self, lane: LaneId) -> Option<&voice::Voice> {
        self.voices.iter().find(|v| v.plays(lane))
    }

    /// Where the set is now, by its clock.
    pub(super) fn clock_now(&self) -> f64 {
        let Some(s) = &self.session else {
            return self.state.position;
        };
        match &s.clock {
            Clock::Lane(id) => {
                let (lanes, _) = self.lanes_now();
                lanes
                    .iter()
                    .find(|l| l.id == *id)
                    .zip(self.voice_of(*id))
                    .and_then(|(lane, v)| {
                        let at = v.mpv.float("time-pos")?;
                        Some(lane.start_seconds + (at - lane.from_seconds))
                    })
                    .unwrap_or(s.at)
            }
            Clock::Virtual { at, since } => {
                if s.playing {
                    at + since.elapsed().as_secs_f64() * s.speed
                } else {
                    *at
                }
            }
            Clock::There { moment, offset_ms } => {
                remote::master_now(moment, remote::clock_ms(), *offset_ms)
            }
        }
    }

    /// Starts playing the set `id` from `at` on its clock — in the voices
    /// that already hold its sources where they can.
    pub(super) fn start_set(
        &mut self,
        mpv: &Mpv,
        id: EntryId,
        at: f64,
        paused: bool,
    ) -> Result<(), Refused> {
        let Some(Body::Set { set, .. }) = self.queue.find(id).map(|e| &e.body) else {
            return Err(Trouble::NoSuchEntry { id }.into());
        };
        let set = set.clone();
        // A SET HAS NO NEXT FILE IN mpv: the window goes, and what plays in
        // the first voice stays until a part is given to it.
        self.window.clear();
        let _ = mpv.command(&["playlist-clear"]);
        self.stopping = false;
        self.generations += 1;
        self.session = Some(Session::new(
            Whose::Here(id),
            Clock::Virtual {
                at,
                since: Instant::now(),
            },
            !paused,
            self.state.speed,
            self.generations,
            at,
        ));
        self.set_current(Some(id));
        self.state.position = at;
        self.state.playing = !paused;
        self.cast(&set.lanes, &set.arrangement.covers, at)?;
        self.tick();
        self.remember_where();
        Ok(())
    }

    /// What plays together ends: every voice but the first leaves, and the
    /// first goes back to mpv's own choices, for the queue's window.
    pub(super) fn end_session(&mut self) {
        if self.session.take().is_none() {
            return;
        }
        // THE LANES' SUBTITLES go with what played together: the first
        // voice shows the file's own choice again.
        self.undress_subtitles();
        while self.voices.len() > 1 {
            if let Some(v) = self.voices.pop() {
                voice::retire(v);
            }
        }
        let (speed, volume, muted, repeat) = (
            self.state.speed,
            self.state.volume,
            self.state.muted,
            self.state.repeat,
        );
        if let Some(v) = self.voices.first_mut() {
            v.part = None;
            v.drift_ms = 0;
            v.forget_applied();
            for (name, value) in [
                ("keep-open", "no".to_owned()),
                ("vid", "auto".to_owned()),
                ("aid", "auto".to_owned()),
                ("sid", "auto".to_owned()),
                ("secondary-sid", "no".to_owned()),
                ("sub-delay", "0".to_owned()),
                ("secondary-sub-delay", "0".to_owned()),
                ("audio-device", "auto".to_owned()),
                ("http-header-fields", String::new()),
                ("speed", speed.to_string()),
                ("volume", volume.to_string()),
                ("mute", if muted { "yes" } else { "no" }.to_owned()),
            ] {
                let _ = v.mpv.set(name, &value);
            }
            super::apply_repeat(&v.mpv, repeat);
        }
        self.map_pictures();
    }

    /// Makes the voices play `lanes` from the set's time `t`: each part to
    /// the voice that already holds its source, else the first voice, else
    /// any other, else a new one; a voice nobody needs leaves.
    pub(super) fn cast(
        &mut self,
        lanes: &[Lane],
        covers: &[LaneId],
        t: f64,
    ) -> Result<(), Refused> {
        let wanted = session::voices(lanes);
        if wanted.len() > session::VOICES_MOST {
            return Err(Trouble::TooMany(TooMany {
                what: Limit::Voices,
                most: session::VOICES_MOST,
            })
            .into());
        }
        let shared = self.shared()?;
        self.main(&shared)?;
        let mut free: Vec<u64> = self.voices.iter().map(|v| v.serial).collect();
        let mut given: Vec<Option<u64>> = vec![None; wanted.len()];
        let loaded = |e: &Self, serial: u64| {
            e.voices
                .iter()
                .find(|v| v.serial == serial)
                .and_then(|v| v.loaded.clone())
        };
        let same_part = |e: &Self, serial: u64, part: &session::Voice| {
            e.voices.iter().any(|v| {
                v.serial == serial
                    && v.part
                        .as_ref()
                        .is_some_and(|p| p.video == part.video && p.audio == part.audio)
            })
        };
        // THE SAME SOURCE AND THE SAME PART, then the same source.
        for exact in [true, false] {
            for (w, part) in wanted.iter().enumerate() {
                if given[w].is_some() {
                    continue;
                }
                if let Some(k) = free.iter().position(|&s| {
                    loaded(self, s).as_deref() == Some(part.source.as_str())
                        && (!exact || same_part(self, s, part))
                }) {
                    given[w] = Some(free.remove(k));
                }
            }
        }
        // THE FIRST VOICE, then any other, then a new one.
        let first = self.voices.first().map(|v| v.serial);
        for slot in &mut given {
            if slot.is_some() {
                continue;
            }
            let pick = free
                .iter()
                .position(|s| Some(*s) == first)
                .or_else(|| (!free.is_empty()).then_some(0));
            *slot = match pick {
                Some(k) => Some(free.remove(k)),
                None => {
                    self.serials += 1;
                    let v = voice::Voice::open(self.serials, &shared)?;
                    self.voices.push(v);
                    Some(self.serials)
                }
            };
        }
        // NOBODY'S: the first voice stops, the others leave.
        for serial in free {
            if Some(serial) == first {
                if let Some(v) = self.voices.first_mut() {
                    let _ = v.mpv.command(&["stop"]);
                    v.part = None;
                    v.loaded = None;
                }
                continue;
            }
            if let Some(k) = self.voices.iter().position(|v| v.serial == serial) {
                voice::retire(self.voices.remove(k));
            }
        }
        for (part, serial) in wanted.iter().zip(given) {
            if let Some(serial) = serial {
                self.play_part(serial, part, lanes, covers, t);
            }
        }
        self.dress_subtitles();
        self.map_pictures();
        self.mind_sinks();
        Ok(())
    }

    /// One voice takes one part: told its tracks, its output and its volume
    /// first, and loaded — from where the set is — only when it does not
    /// hold that source already.
    fn play_part(
        &mut self,
        serial: u64,
        part: &session::Voice,
        lanes: &[Lane],
        covers: &[LaneId],
        t: f64,
    ) {
        let device = part.sink.clone().filter(|d| {
            let ok = self.sink_is_there(d);
            if !ok {
                self.sink_gone(d);
            }
            ok
        });
        let dress = dress_of(
            part,
            lanes,
            covers,
            (self.state.volume, self.state.muted),
            device,
        );
        let header = match self.session.as_ref().map(|s| &s.whose) {
            Some(Whose::There { keyed, header, .. }) if keyed.contains(&part.source) => {
                header.clone()
            }
            _ => String::new(),
        };
        let lead = part
            .audio
            .or(part.video)
            .and_then(|id| lanes.iter().find(|l| l.id == id));
        let Some(v) = self.voices.iter_mut().find(|v| v.serial == serial) else {
            return;
        };
        v.part = Some(part.clone());
        for (name, value) in &dress {
            v.set(name, value);
        }
        v.set("http-header-fields", &header);
        if v.loaded.as_deref() == Some(part.source.as_str()) {
            return;
        }
        let at = lead.map_or(0.0, |lane| {
            target_of(session::when(t, lane, None), lane, None)
        });
        // PAUSED until the clock says: the next look decides.
        let _ = v.mpv.set("pause", "yes");
        if let Err(e) = super::load(
            &v.mpv,
            &part.source,
            "replace",
            &[("start", format!("{at:.3}"))],
        ) {
            v.failed = Some(format!("{e:#}"));
            return;
        }
        v.loading(&part.source);
    }

    /// Which voice draws which picture: each picture lane its voice's, and
    /// the first picture shown — the arrangement's first — as `None`.
    pub(super) fn map_pictures(&mut self) {
        let mut drawn: Vec<(Option<LaneId>, u64)> = Vec::new();
        if self.session.is_none() {
            // ALONE, the first voice draws everything: its picture, its
            // cover, and "the picture".
            if let Some(v) = self.voices.first() {
                drawn.push((None, v.serial));
                drawn.push((Some(LaneId(1)), v.serial));
                drawn.push((Some(LaneId(2)), v.serial));
            }
        } else {
            let (lanes, arrangement) = self.lanes_now();
            let arrangement = arrangement.unwrap_or_default();
            for lane in lanes.iter().filter(|l| l.enabled) {
                let picture = lane.kind == LaneKind::Video
                    || (lane.kind == LaneKind::Audio && arrangement.covers.contains(&lane.id));
                if let Some(v) = self.voice_of(lane.id).filter(|_| picture) {
                    drawn.push((Some(lane.id), v.serial));
                }
            }
            let shown = arrange::tiles(&lanes, &arrangement, None, |id| {
                let v = self.voice_of(id)?;
                let is_sound = lanes
                    .iter()
                    .any(|l| l.id == id && l.kind == LaneKind::Audio);
                (!is_sound || v.has_cover()).then_some(arrange::DEFAULT_ASPECT)
            });
            let first = shown
                .first()
                .and_then(|t| drawn.iter().find(|(l, _)| *l == Some(t.lane)))
                .map(|(_, s)| *s)
                .or_else(|| {
                    self.voices
                        .iter()
                        .find(|v| v.part.is_some())
                        .map(|v| v.serial)
                });
            if let Some(serial) = first {
                drawn.push((None, serial));
            }
        }
        if drawn != self.drawn {
            self.drawn = drawn;
            // EVERY VIEW IS OWED the frame of whoever draws it now.
            for v in &mut self.viewers {
                v.owed = true;
            }
            for v in &self.voices {
                v.poke.ring();
            }
        }
    }

    /// One look at every voice: where the set's clock is, who keeps it, and
    /// each voice held, sent or nudged towards where it should be. Ends the
    /// set when its last lane has.
    pub(super) fn tick(&mut self) {
        let Some(s) = &self.session else { return };
        let (playing, speed, here) = (s.playing, s.speed, matches!(s.whose, Whose::Here(_)));
        let mut clock = s.clock.clone();
        let (lanes, _) = self.lanes_now();
        let mut t = self.clock_now();
        let length_of = |e: &Self, id: LaneId| e.voice_of(id).and_then(voice_length);
        // WHO KEEPS THE TIME, for a set of this machine.
        if here {
            let master = session::master(&lanes, |id| length_of(self, id), t);
            clock = match (clock, master) {
                (Clock::Lane(a), Master::Lane(b)) if a == b => Clock::Lane(a),
                (old, Master::Lane(b)) => {
                    let heard = lanes
                        .iter()
                        .find(|l| l.id == b)
                        .zip(self.voice_of(b))
                        .and_then(|(lane, v)| {
                            Some(
                                lane.start_seconds + (v.mpv.float("time-pos")? - lane.from_seconds),
                            )
                        });
                    match (old, heard) {
                        // A SOUND THAT LANDED takes the time over; one still
                        // on its way is sent where the time is.
                        (Clock::Lane(_), Some(h)) => {
                            t = h;
                            Clock::Lane(b)
                        }
                        (_, Some(h)) if (h - t).abs() <= TAKES_OVER_SECONDS => {
                            t = h;
                            Clock::Lane(b)
                        }
                        (Clock::Virtual { at, since }, _) => Clock::Virtual { at, since },
                        _ => Clock::Virtual {
                            at: t,
                            since: Instant::now(),
                        },
                    }
                }
                (Clock::Virtual { at, since }, Master::Virtual) => Clock::Virtual { at, since },
                (_, Master::Virtual) => Clock::Virtual {
                    at: t,
                    since: Instant::now(),
                },
            };
            // THE ENGINE'S OWN CLOCK WAITS for a voice still loading: it
            // would otherwise run ahead of what nobody hears yet.
            let loading = self.voices.iter().any(|v| {
                v.part.is_some()
                    && v.failed.is_none()
                    && v.loaded.is_some()
                    && v.mpv.float("time-pos").is_none()
            });
            if loading && let Clock::Virtual { .. } = clock {
                clock = Clock::Virtual {
                    at: t,
                    since: Instant::now(),
                };
            }
        }
        let master_lane = match clock {
            Clock::Lane(l) => Some(l),
            _ => None,
        };
        for v in &mut self.voices {
            let Some(part) = v.part.clone() else { continue };
            let Some(lead) = part
                .audio
                .or(part.video)
                .and_then(|id| lanes.iter().find(|l| l.id == id))
            else {
                continue;
            };
            let at = v.mpv.float("time-pos");
            match session::when(t, lead, v.duration) {
                When::Before => {
                    let _ = v.mpv.set("pause", "yes");
                    v.drift_ms = 0;
                    if let Some(p) = at
                        && (p - lead.from_seconds).abs() > 0.1
                        && !v.settling()
                    {
                        seek(&v.mpv, lead.from_seconds);
                    }
                    // WAITING TO START is settling: the first seconds after
                    // its start send it, and do not stretch it.
                    v.settle();
                }
                When::At { seconds } => {
                    let _ = v.mpv.set("pause", if playing { "no" } else { "yes" });
                    if master_lane.is_some_and(|m| v.plays(m)) {
                        v.drift_ms = 0;
                        let _ = v.mpv.set("speed", &speed.to_string());
                        continue;
                    }
                    let Some(p) = at else { continue };
                    #[allow(clippy::cast_possible_truncation)]
                    let drift = ((p - seconds) * 1000.0).round() as i64;
                    v.drift_ms = drift;
                    if !playing {
                        // HELD WHERE IT BELONGS while nothing moves.
                        if (p - seconds).abs() > 0.05 && !v.settling() {
                            seek(&v.mpv, seconds);
                            v.settle();
                        }
                        continue;
                    }
                    if v.settling() {
                        match sync::settle(seconds, p) {
                            Some(to) => seek(&v.mpv, to),
                            None => {
                                let _ = v.mpv.set("speed", &speed.to_string());
                            }
                        }
                        continue;
                    }
                    let who = if part.audio.is_some() {
                        Follower::Sound
                    } else {
                        Follower::Picture
                    };
                    match sync::correct_as(seconds, p, speed, who) {
                        Correction::Keep => {
                            let _ = v.mpv.set("speed", &speed.to_string());
                        }
                        Correction::Speed(f) => {
                            let _ = v.mpv.set("speed", &f.to_string());
                        }
                        Correction::Seek(to) => {
                            seek(&v.mpv, to);
                            v.settle();
                        }
                    }
                }
                When::After => {
                    let _ = v.mpv.set("pause", "yes");
                    v.drift_ms = 0;
                }
            }
        }
        let length = session::length(&lanes, |id| length_of(self, id));
        if let Some(s) = self.session.as_mut() {
            s.clock = clock;
            s.at = t;
            s.ticked = Instant::now();
        }
        self.state.position = t;
        self.state.playing = playing;
        self.state.duration = length;
        if here {
            // DONE when the clock passed the last lane's end, or when every
            // voice is at its end (a live lane has none), failed, or past
            // its lane's stretch.
            let done = |v: &voice::Voice| {
                v.eof
                    || v.failed.is_some()
                    || v.part
                        .as_ref()
                        .and_then(|p| p.audio.or(p.video))
                        .and_then(|id| lanes.iter().find(|l| l.id == id))
                        .is_some_and(|lane| {
                            matches!(session::when(t, lane, voice_length(v)), When::After)
                        })
            };
            let voiced: Vec<&voice::Voice> =
                self.voices.iter().filter(|v| v.part.is_some()).collect();
            let over = length.is_some_and(|end| t >= end - 0.02)
                || (!voiced.is_empty() && voiced.iter().all(|v| done(v)));
            if over && playing {
                self.set_ended();
                return;
            }
        } else if let Some(f) = self.following.as_mut() {
            f.drift_ms = self
                .voices
                .iter()
                .filter(|v| v.part.is_some())
                .map(|v| v.drift_ms)
                .max_by_key(|d| d.abs())
                .unwrap_or(0);
        }
        self.dirty = true;
        self.mind_the_clock();
    }

    /// A look from an event, at most as often as the clock thread would:
    /// the time moved in the voice that keeps it, a lane reached its end.
    pub(super) fn tick_soon(&mut self, now: bool) {
        let Some(s) = &self.session else { return };
        if !matches!(s.whose, Whose::Here(_)) || s.ticking {
            return;
        }
        if now || s.ticked.elapsed() >= sync::TICK {
            self.tick();
        }
    }

    /// Whether the set needs looking at on a clock: two voices to keep
    /// together, the engine's own clock to move, or a lane waiting for its
    /// start.
    pub(super) fn needs_ticks(&self) -> bool {
        let Some(s) = &self.session else { return false };
        if !matches!(s.whose, Whose::Here(_)) {
            return false;
        }
        if self.voices.iter().filter(|v| v.part.is_some()).count() >= 2 {
            return true;
        }
        if matches!(s.clock, Clock::Virtual { .. }) && s.playing {
            return true;
        }
        let (lanes, _) = self.lanes_now();
        lanes.iter().any(|l| {
            l.enabled
                && l.kind != LaneKind::Subtitle
                && matches!(session::when(s.at, l, None), When::Before)
        })
    }

    /// Starts the clock thread when the set needs one and has none; it
    /// leaves when the set no longer needs it, or is another set.
    pub(super) fn mind_the_clock(&mut self) {
        if !self.needs_ticks() {
            return;
        }
        let Ok(shared) = self.shared() else { return };
        let Some(s) = self.session.as_mut() else {
            return;
        };
        if s.ticking {
            return;
        }
        s.ticking = true;
        let generation = s.generation;
        let _ = std::thread::Builder::new()
            .name("player-clock".into())
            .spawn(move || {
                loop {
                    let wait = {
                        let (lock, _) = &*shared;
                        let Ok(e) = lock.lock() else { return };
                        e.until_next_boundary()
                    };
                    std::thread::sleep(wait);
                    let (lock, bell) = &*shared;
                    let Ok(mut e) = lock.lock() else { return };
                    let mine = e
                        .session
                        .as_ref()
                        .is_some_and(|s| s.generation == generation);
                    if !mine {
                        return;
                    }
                    if !e.needs_ticks() {
                        if let Some(s) = e.session.as_mut() {
                            s.ticking = false;
                        }
                        return;
                    }
                    e.tick();
                    if e.dirty {
                        e.broadcast();
                    }
                    drop(e);
                    bell.notify_all();
                }
            });
    }

    /// How long until the next look: a tick, or sooner when a lane starts
    /// before it — so a lane begins ON its start, not up to a tick late.
    fn until_next_boundary(&self) -> Duration {
        let Some(s) = &self.session else {
            return sync::TICK;
        };
        let (lanes, _) = self.lanes_now();
        let t = self.clock_now();
        let speed = if s.speed > 0.0 { s.speed } else { 1.0 };
        lanes
            .iter()
            .filter(|l| l.enabled && l.start_seconds > t)
            .map(|l| (l.start_seconds - t) / speed)
            .filter(|w| w.is_finite() && *w >= 0.0)
            .map(Duration::from_secs_f64)
            .chain(std::iter::once(sync::TICK))
            .min()
            .unwrap_or(sync::TICK)
            .max(Duration::from_millis(5))
    }

    /// Plays or pauses the set.
    pub(super) fn session_play(&mut self, on: bool) {
        let t = self.clock_now();
        let Some(s) = self.session.as_mut() else {
            return;
        };
        if s.playing == on {
            return;
        }
        s.playing = on;
        if let Clock::Virtual { .. } = s.clock {
            s.clock = Clock::Virtual {
                at: t,
                since: Instant::now(),
            };
        }
        s.at = t;
        self.tick();
        self.remember_where();
    }

    /// Goes to `t` on the set's clock: every voice sent where it belongs
    /// then, and the engine's clock keeping the time until the sound that
    /// keeps it has landed.
    pub(super) fn session_seek(&mut self, t: f64) {
        let (lanes, _) = self.lanes_now();
        let length = session::length(&lanes, |id| self.voice_of(id).and_then(|v| v.duration));
        // BACK FROM THE END: nothing is at its end any more.
        for v in &mut self.voices {
            v.eof = false;
        }
        let t = length.map_or(t, |l| t.min(l)).max(0.0);
        for v in &mut self.voices {
            let Some(part) = &v.part else { continue };
            let Some(lead) = part
                .audio
                .or(part.video)
                .and_then(|id| lanes.iter().find(|l| l.id == id))
            else {
                continue;
            };
            let when = session::when(t, lead, v.duration);
            seek(&v.mpv, target_of(when, lead, v.duration));
            v.eof = false;
            v.settle();
        }
        if let Some(s) = self.session.as_mut() {
            s.clock = Clock::Virtual {
                at: t,
                since: Instant::now(),
            };
            s.at = t;
        }
        self.state.position = t;
        self.tick();
    }

    /// The set reached its end: the queue says what comes next — the same
    /// set from its start, with repeat one.
    fn set_ended(&mut self) {
        let Some(Whose::Here(id)) = self.session.as_ref().map(|s| s.whose.clone()) else {
            return;
        };
        let next = self.queue.next(
            Some(id),
            self.state.repeat,
            Step::Ended,
            self.order.as_deref(),
        );
        let Some(first) = self.voices.first().map(|v| v.mpv.clone()) else {
            return;
        };
        match next {
            Some(n) if n == id => self.session_seek(0.0),
            Some(n) => {
                if let Err(Refused(t)) = self.load_leaf(&first, n, false) {
                    self.state.troubles = vec![t];
                }
            }
            None => {
                let _ = self.stop(&first);
                self.set_current(None);
                self.remember_where();
            }
        }
        self.dirty = true;
    }

    /// What plays, as a set: the current one, or the file playing alone
    /// made one — the same entry and id, still playing where it was.
    fn promoted(&mut self) -> Result<EntryId, Refused> {
        if let Some(f) = &self.following {
            return Err(Trouble::Following {
                host: f.host.clone(),
            }
            .into());
        }
        let id = self.state.current.ok_or(Trouble::NothingPlaying)?;
        match self.queue.find(id).map(|e| &e.body) {
            Some(Body::Set { .. }) => return Ok(id),
            Some(Body::Stream(_)) => return Err(Trouble::StreamPlaysAlone.into()),
            Some(Body::Single { .. }) => {}
            _ => return Err(Trouble::NothingPlaying.into()),
        }
        let set = self.plain_set().ok_or(Trouble::NothingPlaying)?;
        self.queue.set_body(id, Item::Set { set, from: None })?;
        let at = self
            .voices
            .first()
            .and_then(|v| v.mpv.float("time-pos"))
            .unwrap_or(self.state.position);
        let paused = !self.state.playing;
        let shared = self.shared()?;
        let mpv = self.main(&shared)?;
        self.start_set(&mpv, id, at, paused)?;
        Ok(id)
    }

    /// Changes what plays as a set, and plays the change: refused whole,
    /// with the reason, when the change or the set it makes is not one.
    fn change_set(
        &mut self,
        change: impl FnOnce(&mut Set) -> Result<(), Trouble>,
    ) -> Result<(), Refused> {
        let id = self.promoted()?;
        let Some(Body::Set { set, .. }) = self.queue.find(id).map(|e| &e.body) else {
            return Err(Trouble::NothingPlaying.into());
        };
        let mut set = set.clone();
        let shape = (set.lanes.len(), set.name.clone());
        change(&mut set)?;
        set.tidy();
        if set.check().is_err() {
            // THE CHANGE BROKE A RULE a lane of the set keeps (a subtitle
            // over something that is not a picture): the lane is named.
            let blamed = set
                .lanes
                .iter()
                .find(|l| {
                    let mut alone = set.clone();
                    alone.lanes.retain(|o| o.id != l.id);
                    alone.tidy();
                    alone.check().is_ok()
                })
                .map_or(LaneId(0), |l| l.id);
            return Err(Trouble::BadValue { lane: blamed }.into());
        }
        if let Some(t) = session::refusal(&set) {
            return Err(Trouble::TooMany(t).into());
        }
        let reshaped = (set.lanes.len(), set.name.clone()) != shape;
        if let Some(slot) = self.queue.set_mut(id) {
            *slot = set;
        }
        if reshaped {
            self.queue.touch();
        }
        self.recast()
    }

    /// Plays the set again as it now says, from where it is.
    fn recast(&mut self) -> Result<(), Refused> {
        let t = self.clock_now();
        let (lanes, arrangement) = self.lanes_now();
        let covers = arrangement.unwrap_or_default().covers;
        self.cast(&lanes, &covers, t)?;
        self.tick();
        Ok(())
    }

    /// `track`: which of its source's tracks a lane plays; the lane is on
    /// after it. A file that plays alone keeps playing alone: mpv is told.
    pub(super) fn choose_track(&mut self, lane: LaneId, track: Option<i64>) -> Result<(), Refused> {
        if let Some(f) = &self.following {
            return Err(Trouble::Following {
                host: f.host.clone(),
            }
            .into());
        }
        if let Some(set) = self.plain_set() {
            let kind = set.lane(lane).ok_or(Trouble::NoSuchLane { lane })?.kind;
            let name = match kind {
                LaneKind::Video => "vid",
                LaneKind::Audio => "aid",
                LaneKind::Subtitle => "sid",
            };
            let value = track.map_or_else(|| "auto".to_owned(), |t| t.to_string());
            if let Some(v) = self.voices.first() {
                v.mpv.set(name, &value)?;
            }
            return Ok(());
        }
        self.change_set(|set| {
            let l = set.lane_mut(lane).ok_or(Trouble::NoSuchLane { lane })?;
            l.track = track;
            l.enabled = true;
            Ok(())
        })
    }

    /// `add-lane`: something more plays together with what plays.
    pub(super) fn add_lane(
        &mut self,
        source: &str,
        kind: LaneKind,
        track: Option<i64>,
        start_seconds: f64,
        over: Option<LaneId>,
    ) -> Result<(), Refused> {
        let source = super::absolute(source.trim());
        self.change_set(|set| {
            let id = set.next_lane_id();
            if source.is_empty() || !start_seconds.is_finite() || start_seconds < 0.0 {
                return Err(Trouble::BadValue { lane: id });
            }
            if over.is_some() && kind != LaneKind::Subtitle {
                return Err(Trouble::WrongKind { lane: id });
            }
            let mut lane = Lane::new(id, kind, source);
            lane.track = track;
            lane.start_seconds = start_seconds;
            lane.over = over;
            set.lanes.push(lane);
            Ok(())
        })
    }

    /// `lane`: one lane changes.
    ///
    /// A file that plays alone and has a lane switched on or off keeps
    /// playing alone: which of its tracks mpv plays, or none, is mpv's own
    /// choice to make. Every other change makes it a set.
    pub(super) fn change_lane(&mut self, lane: LaneId, change: LaneChange) -> Result<(), Refused> {
        if let (LaneChange::Enabled { on }, Some(set)) = (&change, self.plain_set()) {
            let l = set.lane(lane).ok_or(Trouble::NoSuchLane { lane })?;
            let (name, kind) = match l.kind {
                LaneKind::Video => ("vid", TrackKind::Video),
                LaneKind::Audio => ("aid", TrackKind::Audio),
                LaneKind::Subtitle => ("sid", TrackKind::Sub),
            };
            let Some(v) = self.voices.first() else {
                return Err(Trouble::NothingPlaying.into());
            };
            // ON with nothing chosen is the file's first track of the kind:
            // mpv's own choice may be none (subtitles nobody flagged).
            let first = v
                .tracks
                .iter()
                .find(|t| t.kind == kind && !t.albumart)
                .map(|t| t.id);
            let value = match (on, l.track.or(first)) {
                (false, _) => "no".to_owned(),
                (true, Some(t)) => t.to_string(),
                (true, None) => "auto".to_owned(),
            };
            v.mpv.set(name, &value)?;
            return Ok(());
        }
        self.change_set(|set| apply_change(set, lane, change))
    }

    /// `arrange`: how the pictures share a screen.
    pub(super) fn arrange(&mut self, arrangement: Arrangement) -> Result<(), Refused> {
        self.change_set(|set| apply_arrangement(set, arrangement))
    }

    /// `save-set`: what plays, as a `.rset` at `to`.
    pub(super) fn save_set(&self, to: &str, name: Option<String>) -> Result<(), Refused> {
        let id = self.state.current.ok_or(Trouble::NothingPlaying)?;
        let mut set = match self.queue.find(id).map(|e| &e.body) {
            Some(Body::Set { set, .. }) => set.clone(),
            _ => self.plain_set().ok_or(Trouble::NothingPlaying)?,
        };
        if let Some(name) = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()) {
            set.name = Some(name);
        }
        set.tidy();
        let unwritable = |why: &str| Trouble::Unwritable {
            path: to.to_owned(),
            why: why.to_owned(),
        };
        let path = std::path::Path::new(to);
        let folder = path
            .parent()
            .filter(|p| p.is_absolute())
            .ok_or_else(|| unwritable("not an absolute path"))?;
        let file = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| unwritable("no file name"))?;
        let text = ronoctua_player::own::set_to_text(&set, folder);
        ronoctua_xdg::write_atomically(folder, file, text.as_bytes())
            .map_err(|e| unwritable(&e.to_string()))?;
        Ok(())
    }

    /// What each lane that is on is doing now.
    pub(super) fn lane_states(
        &self,
        lanes: &[Lane],
        arrangement: Option<&Arrangement>,
    ) -> Vec<LaneState> {
        let covers = arrangement.map(|a| a.covers.as_slice()).unwrap_or_default();
        let (t, master) = match &self.session {
            Some(s) => (
                s.at,
                match s.clock {
                    Clock::Lane(l) => Some(l),
                    _ => None,
                },
            ),
            // ALONE, the file's own sound keeps its time.
            None => (
                self.state.position,
                lanes
                    .iter()
                    .find(|l| l.enabled && l.kind == LaneKind::Audio)
                    .map(|l| l.id),
            ),
        };
        lanes
            .iter()
            .filter(|l| l.enabled)
            .map(|lane| {
                let v = if self.session.is_some() {
                    self.voice_of(lane.id)
                } else {
                    self.voices.first()
                };
                let cover = lane.kind == LaneKind::Audio && v.is_some_and(voice::Voice::has_cover);
                let picture = v.is_some_and(|v| v.size.is_some())
                    && match lane.kind {
                        LaneKind::Video => true,
                        LaneKind::Audio => cover && covers.contains(&lane.id),
                        LaneKind::Subtitle => false,
                    };
                let status = if lane.kind == LaneKind::Subtitle {
                    // ITS WORDS BEING READ, or refused with the reason; else
                    // where the set's clock is in its stretch.
                    self.subtitles.status_of(lane.id).unwrap_or_else(|| {
                        match session::when(t, lane, None) {
                            When::Before => LaneStatus::Pending,
                            When::At { .. } => LaneStatus::Playing,
                            When::After => LaneStatus::Ended,
                        }
                    })
                } else {
                    match v {
                        None => LaneStatus::Preparing,
                        Some(v) if v.failed.is_some() => LaneStatus::Refused(Trouble::Unplayable {
                            why: v.failed.clone().unwrap_or_default(),
                        }),
                        Some(_) if self.session.is_none() => LaneStatus::Playing,
                        Some(v) if v.loaded.is_none() => LaneStatus::Preparing,
                        Some(v) => match session::when(t, lane, v.duration) {
                            When::Before => LaneStatus::Pending,
                            When::At { .. } => LaneStatus::Playing,
                            When::After => LaneStatus::Ended,
                        },
                    }
                };
                let sounding = lane.kind != LaneKind::Subtitle;
                LaneState {
                    lane: lane.id,
                    duration_seconds: v.filter(|_| sounding).and_then(|v| v.duration),
                    status,
                    picture,
                    drift_ms: v.filter(|_| sounding).map_or(0, |v| v.drift_ms),
                    master: master == Some(lane.id),
                    size: v.filter(|_| picture).and_then(|v| v.size),
                    cover,
                }
            })
            .collect()
    }

    /// The sources of `lanes`, each with its tracks — what every voice that
    /// holds it reads, a track in use in any of them in use.
    pub(super) fn sources_of(&self, lanes: &[Lane]) -> Vec<Source> {
        let mut out: Vec<Source> = Vec::new();
        for lane in lanes {
            if out.iter().any(|s| s.location == lane.source) {
                continue;
            }
            let mut tracks: Vec<Track> = Vec::new();
            for v in self
                .voices
                .iter()
                .filter(|v| v.loaded.as_deref() == Some(lane.source.as_str()))
            {
                for t in &v.tracks {
                    // OURS ARE NOT THE SOURCE'S: the merged script and the
                    // files added for subtitle lanes are lanes of their own.
                    if t.external
                        && (v.added.iter().any(|(_, id)| *id == t.id)
                            || self.subtitles.merged_of(v.serial) == Some(t.id))
                    {
                        continue;
                    }
                    match tracks.iter_mut().find(|o| o.id == t.id && o.kind == t.kind) {
                        Some(o) => o.selected |= t.selected,
                        None => tracks.push(t.clone()),
                    }
                }
            }
            out.push(Source {
                location: lane.source.clone(),
                tracks,
            });
        }
        out
    }

    /// One look at another machine's moment, while this one plays in step
    /// with it: its lanes cast when they changed, every voice sent or
    /// nudged towards where that machine is.
    ///
    /// `follow` names the following it is a step of: a step of one that was
    /// replaced — joined again as another role — is a step of nothing.
    pub(crate) fn follow_step(
        &mut self,
        follow: &std::sync::Arc<std::sync::atomic::AtomicBool>,
        moment: Moment,
        lanes: Vec<Lane>,
        (offset_ms, keyed, header): (f64, Vec<String>, String),
    ) -> Result<(), Refused> {
        if !self
            .following
            .as_ref()
            .is_some_and(|f| std::sync::Arc::ptr_eq(&f.stop, follow))
        {
            return Ok(());
        }
        let whose = Whose::There {
            item: moment.item,
            lanes: lanes.clone(),
            keyed,
            header,
        };
        let same = self.session.as_ref().is_some_and(|s| s.whose == whose);
        let (playing, speed) = (moment.playing, moment.speed);
        let clock = Clock::There {
            moment: Box::new(moment),
            offset_ms,
        };
        if same {
            if let Some(s) = self.session.as_mut() {
                s.clock = clock;
                s.playing = playing;
                s.speed = speed;
            }
        } else {
            self.window.clear();
            self.generations += 1;
            self.session = Some(Session::new(
                whose,
                clock,
                playing,
                speed,
                self.generations,
                0.0,
            ));
            let t = self.clock_now();
            self.cast(&lanes, &[], t)?;
        }
        self.state.location = lanes.first().map(|l| l.source.clone());
        self.tick();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_player::own::Stack;

    fn track(id: i64, kind: TrackKind, selected: bool, albumart: bool) -> Track {
        Track {
            id,
            kind,
            title: None,
            lang: None,
            codec: None,
            selected,
            albumart,
            default: false,
            forced: false,
            external: false,
        }
    }

    /// FAILS IF: a file that plays alone is not the set it would become —
    /// its picture, its sound and its subtitles as lanes 1, 2 and 3, each
    /// the track mpv chose, the ones nothing was chosen for off, and a
    /// song's cover shown.
    #[test]
    fn a_file_alone_is_the_set_it_would_become() {
        let film = implicit_set(
            "/m/film.mkv",
            Some("Film"),
            &[
                track(1, TrackKind::Video, true, false),
                track(1, TrackKind::Audio, false, false),
                track(2, TrackKind::Audio, true, false),
                track(1, TrackKind::Sub, false, false),
            ],
        );
        let lanes: Vec<(u32, LaneKind, Option<i64>, bool)> = film
            .lanes
            .iter()
            .map(|l| (l.id.0, l.kind, l.track, l.enabled))
            .collect();
        assert_eq!(
            lanes,
            vec![
                (1, LaneKind::Video, Some(1), true),
                (2, LaneKind::Audio, Some(2), true),
                (3, LaneKind::Subtitle, None, false),
            ]
        );
        assert_eq!(film.lanes[0].title.as_deref(), Some("Film"));
        assert_eq!(
            film.lanes[2].title, None,
            "subtitles are not the film's title"
        );
        assert!(film.arrangement.covers.is_empty());
        assert!(film.check().is_ok());
        // A SONG WITH A COVER: a sound, and its cover shown; no picture lane.
        let song = implicit_set(
            "/m/song.flac",
            None,
            &[
                track(1, TrackKind::Video, true, true),
                track(1, TrackKind::Audio, true, false),
            ],
        );
        assert_eq!(song.lanes.len(), 1);
        assert_eq!(song.lanes[0].kind, LaneKind::Audio);
        assert_eq!(song.arrangement.covers, vec![LaneId(2)]);
        assert!(song.check().is_ok());
        assert!(implicit_set("/m/x", None, &[]).lanes.is_empty());
    }

    /// FAILS IF: a voice is told a track, an output or a volume its part
    /// does not say — a picture voice playing a sound, a sound showing a
    /// cover nobody chose, the lane's volume ignored or past the most.
    #[test]
    fn a_voice_is_dressed_for_its_part() {
        let mut picture = Lane::new(LaneId(1), LaneKind::Video, "/m/film.mkv");
        picture.track = Some(1);
        let mut dub = Lane::new(LaneId(2), LaneKind::Audio, "/m/film.mkv");
        dub.track = Some(2);
        dub.volume_percent = 120;
        dub.sink = Some("bluez_output.x".into());
        let song = Lane::new(LaneId(3), LaneKind::Audio, "/m/song.flac");
        let lanes = vec![picture, dub, song];
        let get = |dress: &[(&str, String)], k: &str| {
            dress
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let voices = session::voices(&lanes);
        assert_eq!(voices.len(), 2);
        let film = dress_of(
            &voices[0],
            &lanes,
            &[],
            (120, false),
            Some("bluez_output.x".into()),
        );
        assert_eq!(get(&film, "vid"), "1");
        assert_eq!(get(&film, "aid"), "2");
        assert_eq!(get(&film, "audio-device"), "pipewire/bluez_output.x");
        assert_eq!(get(&film, "volume"), "130", "the most, not 144");
        assert_eq!(get(&film, "keep-open"), "yes");
        assert_eq!(get(&film, "loop-file"), "no");
        let alone = dress_of(&voices[1], &lanes, &[], (50, true), None);
        assert_eq!(
            get(&alone, "vid"),
            "no",
            "a sound shows no cover unless chosen"
        );
        assert_eq!(get(&alone, "aid"), "auto");
        assert_eq!(get(&alone, "audio-device"), "auto");
        assert_eq!(get(&alone, "volume"), "50");
        assert_eq!(get(&alone, "mute"), "yes");
        let covered = dress_of(&voices[1], &lanes, &[LaneId(3)], (100, false), None);
        assert_eq!(get(&covered, "vid"), "auto");
        // A PICTURE ALONE plays no sound.
        let mut only = lanes.clone();
        only.retain(|l| l.id == LaneId(1));
        let silent = dress_of(&session::voices(&only)[0], &only, &[], (100, false), None);
        assert_eq!(get(&silent, "aid"), "no");
    }

    /// FAILS IF: a lane change is taken for a lane it does not fit, a value
    /// out of its range is taken, or the last lane of a set can be removed.
    #[test]
    fn a_lane_change_is_refused_with_its_reason() {
        let mut set = Set::default();
        set.lanes
            .push(Lane::new(LaneId(1), LaneKind::Video, "/m/v.mkv"));
        set.lanes
            .push(Lane::new(LaneId(2), LaneKind::Audio, "/m/a.opus"));
        let mut words = Lane::new(LaneId(3), LaneKind::Subtitle, "/m/w.srt");
        words.over = Some(LaneId(1));
        set.lanes.push(words);
        let refused = |set: &mut Set, lane: u32, change: LaneChange| {
            apply_change(set, LaneId(lane), change).err()
        };
        assert_eq!(
            refused(
                &mut set,
                1,
                LaneChange::Sink {
                    device: Some("x".into())
                }
            ),
            Some(Trouble::WrongKind { lane: LaneId(1) })
        );
        assert_eq!(
            refused(&mut set, 2, LaneChange::Stack { stack: Stack::Top }),
            Some(Trouble::WrongKind { lane: LaneId(2) })
        );
        assert_eq!(
            refused(&mut set, 2, LaneChange::Start { seconds: -1.0 }),
            Some(Trouble::BadValue { lane: LaneId(2) })
        );
        assert_eq!(
            refused(
                &mut set,
                2,
                LaneChange::Trim {
                    from_seconds: 5.0,
                    until_seconds: Some(5.0)
                }
            ),
            Some(Trouble::BadValue { lane: LaneId(2) })
        );
        assert_eq!(
            refused(&mut set, 2, LaneChange::Volume { percent: 131 }),
            Some(Trouble::BadValue { lane: LaneId(2) })
        );
        assert_eq!(
            refused(&mut set, 9, LaneChange::Mute { muted: true }),
            Some(Trouble::NoSuchLane { lane: LaneId(9) })
        );
        // TAKEN: a sound sent to headphones, starting two seconds in.
        assert_eq!(
            refused(
                &mut set,
                2,
                LaneChange::Sink {
                    device: Some("h".into())
                }
            ),
            None
        );
        assert_eq!(
            refused(&mut set, 2, LaneChange::Start { seconds: 2.0 }),
            None
        );
        assert_eq!(set.lanes[1].sink.as_deref(), Some("h"));
        // THE PICTURE GOES, and the words over it fall to the first left.
        assert_eq!(refused(&mut set, 1, LaneChange::Remove), None);
        assert_eq!(set.lane(LaneId(3)).and_then(|l| l.over), None);
        assert_eq!(refused(&mut set, 3, LaneChange::Remove), None);
        assert_eq!(
            refused(&mut set, 2, LaneChange::Remove),
            Some(Trouble::LastLane)
        );
    }

    /// FAILS IF: an arrangement naming a lane that is not there, a cover of
    /// a picture, or a hidden subtitle is taken.
    #[test]
    fn an_arrangement_names_lanes_that_can_be_used_so() {
        let mut set = Set::default();
        set.lanes
            .push(Lane::new(LaneId(1), LaneKind::Video, "/m/v.mkv"));
        set.lanes
            .push(Lane::new(LaneId(2), LaneKind::Audio, "/m/a.flac"));
        set.lanes
            .push(Lane::new(LaneId(3), LaneKind::Subtitle, "/m/w.srt"));
        let with = |order: Vec<u32>, hidden: Vec<u32>, covers: Vec<u32>| Arrangement {
            order: order.into_iter().map(LaneId).collect(),
            hidden: hidden.into_iter().map(LaneId).collect(),
            covers: covers.into_iter().map(LaneId).collect(),
            ..Arrangement::default()
        };
        assert_eq!(
            apply_arrangement(&mut set, with(vec![9], vec![], vec![])),
            Err(Trouble::NoSuchLane { lane: LaneId(9) })
        );
        assert_eq!(
            apply_arrangement(&mut set, with(vec![], vec![], vec![1])),
            Err(Trouble::WrongKind { lane: LaneId(1) })
        );
        assert_eq!(
            apply_arrangement(&mut set, with(vec![], vec![3], vec![])),
            Err(Trouble::WrongKind { lane: LaneId(3) })
        );
        assert_eq!(
            apply_arrangement(&mut set, with(vec![2, 1], vec![1], vec![2])),
            Ok(())
        );
        assert_eq!(set.arrangement.covers, vec![LaneId(2)]);
    }

    /// FAILS IF: a voice before its lane's start is not sent to its first
    /// frame, or one past its end to its first instead of its last.
    #[test]
    fn a_voice_is_sent_to_its_lane_s_first_or_last_frame() {
        let mut lane = Lane::new(LaneId(1), LaneKind::Audio, "/m/a.opus");
        lane.start_seconds = 2.0;
        lane.from_seconds = 10.0;
        assert!((target_of(When::Before, &lane, Some(60.0)) - 10.0).abs() < 1e-9);
        assert!((target_of(When::At { seconds: 13.0 }, &lane, None) - 13.0).abs() < 1e-9);
        assert!((target_of(When::After, &lane, Some(60.0)) - 59.95).abs() < 1e-9);
        lane.until_seconds = Some(20.0);
        assert!((target_of(When::After, &lane, Some(60.0)) - 19.95).abs() < 1e-9);
    }
}
