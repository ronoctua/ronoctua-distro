//! The player's words: what a client asks and what the daemon answers.
//!
//! # Two messages, each one whole
//!
//! The shape of the studio's and the transfer queue's: one JSON object a line,
//! and never a delta — a client that missed a line never has to catch up. What
//! changed in version 2 is that the daemon sends TWO things, each whole:
//!
//! - [`State`], small, on every event (a position is said at most four times a
//!   second): what plays, where, how loud, and the fields of the doors;
//! - the queue's [`Outline`], its shape and names, only when that SHAPE
//!   changes ([`Say::Queue`]).
//!
//! The queue used to ride inside every state. A tree of thousands of leaves
//! would then have been megabytes of JSON a second to every door, for a
//! position that moved a quarter of a second. The owner approved the split
//! (2026-09-28). A door that holds an outline whose `revision` is not the
//! state's knows it missed one, and asks ([`Ask::Queue`]).
//!
//! # Versions
//!
//! [`State::protocol`] is [`PROTOCOL`]. A door that reads it absent (zero) or
//! older is talking to a daemon from before an upgrade, and says so; an ask a
//! daemon cannot read is answered [`Trouble::NotUnderstood`], never silence.
//!
//! # What is asked for, and never broadcast
//!
//! A file's whole [`Sheet`] — every tag, stream and chapter — is kilobytes,
//! and most doors are not showing it. So it is never in the state: the state
//! carries [`State::sheet_generation`], which moves when what is known of a
//! playing file's tags changes, and a door that shows them asks
//! ([`Ask::Sheet`] → [`Say::Sheet`]) when it moves.
//!
//! The same for what the daemon looked up on the network ([`crate::find`]):
//! the state carries each desk's summary ([`State::desks`]), and a door asks
//! for the page ([`Ask::Desk`] → [`Say::Desk`]) when its generation moves.

use std::io::{BufRead, Write};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::find::{CandidateId, Desk, LyricsTo, Query};
use crate::limit::TooMany;
use crate::own::{Arrangement, Lane, LaneId, LaneKind, Screens, Set, Stack, Stream};
use crate::queue::{EntryId, Name, Outline};
use ronoctua_tags::{CoverAt, Edit, Field, Sheet, Stamp};

/// The socket's name under the runtime directory.
pub const SOCKET: &str = "player.sock";

/// How many several-line fields may be open in the editor at once
/// ([`Ask::Draft`]).
///
/// Eight: a person edits one or two; past eight it is drafts nobody came
/// back to, each a file in memory (the runtime directory) and a name the
/// daemon watches.
pub const DRAFTS_MOST: usize = 8;

/// How many changes to files may wait behind the one being written
/// ([`Ask::Retag`], [`Ask::SaveCover`], [`Ask::WriteDraft`]).
///
/// Writes go one at a time — two writes of one file at once would share a
/// draft (`ronoctua_tags::write::apply`) — and a door sends one per answer a
/// person gives. Sixteen waiting is a door gone wrong, not a person typing.
pub const WRITES_WAITING_MOST: usize = 16;

/// How many takes of one desk may wait behind the one being fetched
/// ([`Ask::Take`]).
///
/// A take is one press on a candidate; a person pressing three subtitles in
/// a row wants the three. Past four waiting it is a door gone wrong, and
/// each is a download.
pub const TAKES_WAITING_MOST: usize = 4;

/// This protocol's version, in every [`State`].
///
/// 1 was the flat queue inside every state; 2 is the queue as our own tree,
/// sent apart ([`Say::Queue`]), and what plays as lanes, each on a voice of
/// its own; 3 says where the queue is kept ([`State::queue_file`]), refuses
/// a thing already queued ([`Ask::Put`]'s `once`), carries what mpv's
/// programs said of a refusal ([`Trouble::Unplayable`]), the lyrics'
/// translation and the local catalogue's summary. Every field 3 added is
/// read as absent from a daemon of 2 (`serde(default)`), and the door's
/// "the player is older" offer covers the rest.
pub const PROTOCOL: u32 = 3;

/// Where the daemon listens — one per instance (`.#dev` has its own).
pub fn socket_path() -> Result<std::path::PathBuf> {
    ronoctua_xdg::runtime_file(SOCKET).context("XDG_RUNTIME_DIR is not set")
}

/// What repeats when the queue ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Repeat {
    /// Stop at the end of the queue.
    #[default]
    Nothing,
    /// Play the queue again.
    Queue,
    /// Play the same item again.
    One,
}

impl Repeat {
    /// The next one of the ring.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Nothing => Self::Queue,
            Self::Queue => Self::One,
            Self::One => Self::Nothing,
        }
    }
}

/// Something to put in the queue.
///
/// On the wire a bare string is a [`Thing::Location`], so the common case reads
/// as it is written: `"what": ["/music/a.flac"]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Thing {
    /// A path or an address. The DAEMON decides what it is: a file, a folder
    /// (a playlist named after it), a playlist file (a playlist named after
    /// its title), one of our `.rset`/`.rqueue`, a radio's list (a stream
    /// with its relays), a YouTube list, or one medium on the network.
    Location(String),
    /// A stream a door already knows the whole of — a station of a directory,
    /// with the headers it must be asked with and its relays.
    Stream {
        /// The stream.
        stream: Stream,
    },
    /// Media played together.
    Set {
        /// The set.
        set: Set,
    },
    /// A path or an address that arrives already TITLED — a row of the
    /// catalogue, a ranking's entry: the daemon resolves `location` as a
    /// [`Thing::Location`] and the leaf is called `title` from the start,
    /// never asked for one on the network.
    Named {
        /// The path or address.
        location: String,
        /// What it is called.
        title: String,
    },
}

/// The address of a source of the catalogue, as a person pasted it.
///
/// **A SECRET BY DEFAULT**: an IPTV list's address often carries its account
/// (`?username=…&password=…`), so this is never printed whole — its `Debug`
/// is [`SourceUrl::masked`], the host and an ellipsis, which is also what a
/// door shows. It crosses the socket whole (a local socket, never an argv).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceUrl(pub String);

impl SourceUrl {
    /// The address as a door shows it: its host and `/…`, never the rest —
    /// and only `…` when the host cannot be told from the account.
    ///
    /// # Why not the first `/`, `?` or `#`
    ///
    /// A password pasted by hand holds them unescaped
    /// (`user:pa/ss@host/…`), and the authority cut at the first of them is
    /// `user:pa` — the account, shown as the host. So when an `@` comes
    /// AFTER that first delimiter, the host is looked for after each `@`,
    /// the last first, and only where it can be one: nothing before that `@`
    /// reads as a query (`=`, `&`), and what follows it is a run of a host's
    /// own characters with a dot, a port or a bracket in it. An `@` of the
    /// query (`?mail=a@b&password=…`, `password=se@cret`) is none of that,
    /// and the answer is `…` — a wrong host at worst, never a secret.
    #[must_use]
    pub fn masked(&self) -> String {
        const CUT: [char; 3] = ['/', '?', '#'];
        let rest = self
            .0
            .split_once("://")
            .map_or(self.0.as_str(), |(_, rest)| rest);
        let segment = |from: &str| from.split(CUT).next().unwrap_or_default().to_owned();
        let authority = segment(rest);
        let host = if rest[authority.len()..].contains('@') {
            rest.match_indices('@')
                .rev()
                .filter(|&(i, _)| !rest[..i].contains(['=', '&']))
                .map(|(i, _)| segment(&rest[i + 1..]))
                .find(|h| plausible_host(h) && h.contains(['.', ':', '[']))
        } else {
            authority
                .rsplit('@')
                .next()
                .map(ToOwned::to_owned)
                .filter(|h| plausible_host(h))
        };
        host.map_or_else(|| "…".to_owned(), |h| format!("{h}/…"))
    }
}

/// Whether `text` could be a host with its port: letters, digits and the
/// few marks a name or an address is written with — never `:` alone, never
/// `&`, `=`, `@` or a space, which only an account or a query holds.
fn plausible_host(text: &str) -> bool {
    !text.is_empty()
        && text.chars().any(char::is_alphanumeric)
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '[' | ']'))
}

impl std::fmt::Debug for SourceUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.masked())
    }
}

/// A source of the local catalogue, by its id in it: a default source's
/// (`nasa`, `iptv`, as `assets/channels.nix` declares them) or a person's
/// (`p…`, `ronoctua_broadcast::sources::Book::next_id`) — the id the
/// catalogue's rows carry (`ronoctua_broadcast::catalog::SourceRow::id`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(pub String);

/// An item of the local catalogue — a channel, a film, an episode — by its
/// number in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub i64);

/// A request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "ask")]
pub enum Ask {
    /// Keep this connection: the state and the outline now, and each again
    /// when it changes.
    Watch,
    /// The state, once.
    Look,
    /// The queue's outline, once ([`Say::Queue`]).
    Queue,
    /// Frames of `width × height` on this connection (a memfd ring follows
    /// the answer). The width is rounded down to a multiple of 16.
    Preview {
        /// Pixels across.
        width: u32,
        /// Pixels down.
        height: u32,
        /// WHOSE picture: `None` is the first picture shown (the first
        /// picture lane, else a sound's cover), `Some(lane)` that lane's — a
        /// picture lane, or a sound lane whose cover is shown. A lane with
        /// no picture yet sends its first frame when it has one.
        #[serde(default)]
        lane: Option<LaneId>,
    },
    /// Put things in the queue.
    ///
    /// # Where they go
    ///
    /// Right after `after` when it is given; else at the end of the playlist
    /// `into`; else — with `play` — right after the one playing, which is
    /// what opening a file from outside the player does (the file manager,
    /// `ronoctua-shell play`): it plays now and the queue is kept; else at the
    /// end of the queue. `replace` empties the queue first — the player's own
    /// "open queue", and nothing else.
    Put {
        /// What to put.
        what: Vec<Thing>,
        /// Right after this entry.
        #[serde(default)]
        after: Option<EntryId>,
        /// At the end of this playlist.
        #[serde(default)]
        into: Option<EntryId>,
        /// Play the first of them now.
        #[serde(default)]
        play: bool,
        /// Start the queue over with them (and play the first).
        #[serde(default)]
        replace: bool,
        /// ONLY IF NOT THERE: refused under the daemon's lock with
        /// [`Trouble::AlreadyQueued`] when a leaf of the queue is already
        /// at the same place (`queue::same_place` — one file however its
        /// path is written, one YouTube video however its address is). A
        /// door that asked the person sends it again without it.
        #[serde(default)]
        once: bool,
    },
    /// Play or pause.
    Toggle,
    /// Play.
    Play,
    /// Pause.
    Pause,
    /// Stop and forget the position.
    Stop,
    /// The next thing to play.
    Next,
    /// The one before.
    Previous,
    /// Play this entry: a leaf, or a playlist from its first leaf.
    Jump {
        /// Its id.
        id: EntryId,
    },
    /// Take an entry off the queue, and everything in it.
    Remove {
        /// Its id.
        id: EntryId,
    },
    /// Move an entry, keeping its id: right after `after`, else FIRST in the
    /// playlist `into`, else first in the queue.
    Move {
        /// Its id.
        id: EntryId,
        /// Right after this one.
        #[serde(default)]
        after: Option<EntryId>,
        /// First in this playlist.
        #[serde(default)]
        into: Option<EntryId>,
    },
    /// Call an entry something else; an empty name gives a file, a set or a
    /// stream back its own (a playlist always has one).
    Rename {
        /// Its id.
        id: EntryId,
        /// The new name.
        name: String,
    },
    /// Write the queue as a `.rqueue` file at `to`, which the door already
    /// asked about (a file that is there is overwritten).
    SaveQueue {
        /// The file's path.
        to: String,
        /// What the person called it.
        #[serde(default)]
        name: Option<String>,
    },
    /// Write the queue again into the `.rqueue` it was opened from or last
    /// saved as ([`State::queue_file`]). Refused with
    /// [`Trouble::NoOrigin`] when it has none, and with
    /// [`Trouble::ChangedOnDisk`] when that file changed since — unless
    /// `force`, which the person said after a question ("no" first).
    SaveQueueAgain {
        /// Write over a file that changed since.
        #[serde(default)]
        force: bool,
    },
    /// Empty the queue.
    Clear,
    /// Go to `seconds`, or move by them when `relative`.
    Seek {
        /// Where, or by how much.
        seconds: f64,
        /// By how much, rather than where.
        relative: bool,
    },
    /// The volume, in percent (up to 130).
    Volume {
        /// Percent.
        percent: u8,
    },
    /// Mute or unmute.
    Mute,
    /// The speed, 0.25 to 4.
    Speed {
        /// A factor.
        factor: f64,
    },
    /// The next repeat mode.
    Repeat,
    /// Shuffle on or off.
    Shuffle,
    /// Which of its source's tracks a lane plays — a film's second language,
    /// another subtitle track — and the lane is on after it. The file that
    /// plays alone keeps being one: its lanes are its own tracks.
    Track {
        /// The lane.
        lane: LaneId,
        /// The source's track of the lane's kind (mpv's id); `None` is the
        /// source's own choice.
        #[serde(default)]
        track: Option<i64>,
    },
    /// Play something together with what plays: another picture, a sound to
    /// other headphones, a commentary, subtitles. A file that played alone
    /// becomes a set, and keeps its place in the queue and its id.
    ///
    /// A PICTURE LANE ADDED WITHOUT A CHOSEN TRACK BRINGS ITS SOUND: the
    /// film's own sound comes as a lane beside it, on the same timing — a
    /// second film played together is heard too. With `track` it is that
    /// one track and nothing else.
    AddLane {
        /// The file or address.
        source: String,
        /// What it contributes.
        kind: LaneKind,
        /// Which of its tracks of that kind; `None` is its own choice (and,
        /// for subtitles, the file itself).
        #[serde(default)]
        track: Option<i64>,
        /// When it begins on the set's clock, in seconds.
        #[serde(default)]
        start_seconds: f64,
        /// For subtitles: the picture they go over.
        #[serde(default)]
        over: Option<LaneId>,
    },
    /// Change one lane of what plays. A file that played alone becomes a
    /// set, keeping its id.
    Lane {
        /// The lane.
        lane: LaneId,
        /// What changes.
        change: LaneChange,
    },
    /// How what plays shows its pictures. A file that played alone becomes
    /// a set, keeping its id.
    Arrange {
        /// The new arrangement.
        arrangement: Arrangement,
    },
    /// Write what plays as a `.rset` file at `to`, which the door already
    /// asked about (a file that is there is overwritten).
    SaveSet {
        /// The file's path.
        to: String,
        /// What the person called it.
        #[serde(default)]
        name: Option<String>,
    },
    /// Keep THIS connection open while a door shows the sound outputs: the
    /// player reads them ([`State::sinks`]) for as long as one such
    /// connection, or a lane sent to an output, is there — and not a moment
    /// longer. The answer is [`Say::Done`], and nothing follows it.
    Sinks,
    /// Read the settings again.
    Reload,
    /// Show a pairing code for two minutes, so another machine can pair.
    Invite,
    /// Pair with another machine: its tailnet name (or address) and the code
    /// it shows.
    PairWith {
        /// A name on the tailnet, or an address.
        to: String,
        /// The six digits.
        code: String,
    },
    /// Play in step with a paired machine, as `role`.
    Join {
        /// The paired machine, by name.
        host: String,
        /// Picture, sound or both.
        role: crate::remote::Role,
    },
    /// Stop playing in step with another machine.
    Leave,
    /// Forget a paired machine, in both directions.
    Forget {
        /// Its name.
        name: String,
    },
    /// Turn an ambient sound on — it plays at once, at the volume it was
    /// left at, and goes round until it is turned off — or off. Nothing the
    /// queue is told reaches it: not stop, not next, not the media keys.
    Ambient {
        /// Its id.
        id: String,
        /// On, or off.
        on: bool,
    },
    /// An ambient sound's own volume, in percent (up to 130); kept for the
    /// next time it is on.
    AmbientVolume {
        /// Its id.
        id: String,
        /// Percent.
        percent: u8,
    },
    /// Every ambient sound off; each keeps its volume.
    AmbientSilence,
    /// Hold the ambient sounds that are on, or let them go on — the
    /// modeline's toggle while nothing of the queue is current
    /// ([`State::only_ambience`]). Choosing a sound lets them go on too.
    AmbientToggle,
    /// Everything a file says about itself: [`Say::Sheet`] to this
    /// connection, never to the watchers. A file is read (lofty, ffprobe,
    /// exiftool, Matroska's own tools — seconds, at worst) unless the one
    /// read last is still the file; an ADDRESS that plays is what it says
    /// while it plays (a radio's ICY words), read only. A door asks when
    /// [`State::sheet_generation`] moves.
    Sheet {
        /// The file's path, or the address that plays.
        location: String,
    },
    /// Write a person's changes into a file. `stamp` is the one its sheet
    /// was read with: a file changed since is refused, never overwritten.
    ///
    /// Taken at once ([`Say::Done`]) and written on the player's one
    /// writing thread, into a draft beside the file that is renamed over it
    /// — what plays goes on playing. How it goes is [`State::retag`]; when
    /// it is written, the queue's names for the file, the state's title,
    /// artist and album, MPRIS and [`State::sheet_generation`] follow.
    Retag {
        /// The file's path.
        location: String,
        /// The file as its sheet was read.
        stamp: Stamp,
        /// What changes.
        edits: Vec<Edit>,
    },
    /// Take a cover out of a file into `cover.<ext>` beside it (or
    /// `<stem>.cover.<ext>`, never over a file that is there). On the writing
    /// thread; the new file's path is in [`State::retag`].
    SaveCover {
        /// The file's path.
        location: String,
        /// Which picture; `None` is the cover — the front, else the first.
        #[serde(default)]
        at: Option<CoverAt>,
    },
    /// Read a file's tags again, because something else may have changed
    /// them: the queue's names, the state and MPRIS follow, and
    /// [`State::sheet_generation`] moves.
    Refresh {
        /// The file's path, or the address that plays.
        location: String,
    },
    /// A several-line field (lyrics, a comment, a description) in the
    /// editor: the daemon writes its value into a file of its own in the
    /// runtime directory and answers where ([`Say::Draft`]); the door opens
    /// it in the editor. The daemon watches it, and when it is saved with a
    /// new value [`State::drafts`] says so, and a door asks whether to write
    /// it. Asked again for the same file and field, it is the same draft.
    Draft {
        /// The file's path.
        location: String,
        /// Which field.
        field: Field,
    },
    /// Write a saved draft into its file — the person said yes. Refused
    /// when the field changed in the file since the draft was made.
    WriteDraft {
        /// Which.
        id: DraftId,
    },
    /// Let a draft go, unwritten: its file goes, and so does the question.
    DropDraft {
        /// Which.
        id: DraftId,
    },
    /// NOT NOW: the draft stays open in the editor, and the question goes
    /// until it is saved again — what "no" answers. Saving part of a text
    /// while still writing it is how an editor is used, and "not yet" must
    /// not throw the draft away ([`Ask::DropDraft`] does that).
    KeepDraft {
        /// Which.
        id: DraftId,
    },
    /// Look something up on the network: tags for a file, subtitles for a
    /// film, lyrics for a song — or list a directory: the world's radio and
    /// TV, a theme's films on the Internet Archive, what is live on YouTube
    /// ([`crate::find`]). It REPLACES the search of its desk that is under
    /// way. Taken at once ([`Say::Done`]) — or refused at once, naming the
    /// host, when the person did not let the player ask it
    /// ([`Trouble::Off`]), when a key it needs is missing, or when this
    /// installation has no yt-dlp for the lives ([`Trouble::Listing`]) — and
    /// asked on a thread of the daemon's; how it goes is the desk's summary
    /// in [`State::desks`].
    Find {
        /// What to look up.
        query: Query,
    },
    /// A desk's page ([`Say::Desk`]) to this connection, never to the
    /// watchers: asked when its generation in [`State::desks`] moves.
    Desk {
        /// Which desk.
        kind: DeskKind,
    },
    /// Take a candidate of a desk.
    ///
    /// - Suggestions: the cover (when `cover`) is fetched, then `fields` are
    ///   written into the file on the writing thread ([`State::retag`]).
    /// - Subtitles: the file is fetched and saved as `<film>.<language>.<ext>`
    ///   beside the film — in the player's own data folder when the film's
    ///   folder cannot be written — and becomes a lane over the film when
    ///   the film plays.
    /// - Lyrics: saved as `<song>.lrc` beside the song, or `to` its own tag.
    ///   A `.lrc` that is there is replaced only with `replace`: without it
    ///   the answer is [`Trouble::Exists`], for the door to ask the person
    ///   ("no" drawn first) and send the take again.
    ///
    /// Answered at once; how it goes is the desk's [`Desk::taken`].
    Take {
        /// Which desk.
        desk: DeskKind,
        /// Which of its candidates.
        candidate: CandidateId,
        /// Suggestions: the fields to write.
        #[serde(default)]
        fields: Vec<Field>,
        /// Suggestions: the cover too.
        #[serde(default)]
        cover: bool,
        /// Lyrics: where they go.
        #[serde(default)]
        to: LyricsTo,
        /// Lyrics: replace the `.lrc` that is there — the person said yes.
        #[serde(default)]
        replace: bool,
    },
    /// Fetch the cover a suggestion names, to be SEEN beside the file's own
    /// ([`crate::find::SuggestedCover`]) — what a door asks when a person opens the
    /// candidate. Answered at once; the picture lands on the candidate, on
    /// the suggestions desk. Refused at once when the candidate is no longer
    /// on the page ([`Trouble::NoSuchCandidate`]), names no cover
    /// ([`Trouble::Lookup`], nothing found), or its host was not let
    /// ([`Trouble::Off`]).
    SuggestedCover {
        /// Which candidate of the suggestions desk.
        candidate: CandidateId,
    },
    /// Try the key the person kept for `service` (a door writes it with
    /// `ronoctua_lookup::keys`, then asks): [`Say::Done`] when the service
    /// took it, [`Say::Refused`] with [`Trouble::Lookup`] saying why when
    /// not — refused, the wrong kind of key, or no answer. OpenSubtitles
    /// logs in too when an account is kept, and says the downloads left
    /// today on the subtitles desk.
    KeyCheck {
        /// Whose key.
        service: ronoctua_lookup::Service,
    },
    /// Keep a station, a channel, an archive film or a live among the
    /// person's favourites (`on`), or let it go. The DAEMON writes the file
    /// (`ronoctua_broadcast::picks`), so two doors pressing a star at once
    /// are two changes applied one after the other, never one door's copy
    /// written over the other's. What is kept is taken from a directory's
    /// page, the recently played or the favourites themselves: a thing none
    /// of them has is refused ([`Trouble::NoSuchPick`]). Doors read the file,
    /// and watch it.
    Favourite {
        /// What it is.
        kind: ronoctua_broadcast::picks::Kind,
        /// Its id in its directory.
        id: String,
        /// Kept, or let go.
        on: bool,
    },
    /// Translate the lyrics shown, line by line (`on`), into `to` — `None`
    /// is the language of the person's doors — or stop. How it goes is
    /// [`State::lyrics_translation`]; the lines carry
    /// [`LyricLine::translated`].
    LyricsTranslation {
        /// Translate, or stop.
        on: bool,
        /// Into which language, by its code.
        #[serde(default)]
        to: Option<String>,
    },
    /// Keep a new source of the local catalogue — an IPTV list, a feed, a
    /// channel, a collection — by its address. The daemon writes the
    /// sources' file; an `adult` one goes to the catalogue kept apart.
    AddSource {
        /// Its address: a secret ([`SourceUrl`]).
        url: SourceUrl,
        /// It goes to the adult catalogue.
        #[serde(default)]
        adult: bool,
    },
    /// Forget a source of the catalogue, and every item it brought — one of
    /// the person's: a default source is never removed.
    RemoveSource {
        /// Which.
        id: SourceId,
    },
    /// Turn a source's switches: whether its posters are fetched (`posters`
    /// — for a default source, turning it on lets the hosts its pictures
    /// come from) and whether it is kept in the adult catalogue (`adult`,
    /// a person's source only: its rows move there with a new ingest). A
    /// switch left `None` stays as it is.
    SetSource {
        /// Which.
        id: SourceId,
        /// Its posters fetched, or not.
        #[serde(default)]
        posters: Option<bool>,
        /// Kept apart, or not.
        #[serde(default)]
        adult: Option<bool>,
    },
    /// Read a source again into the catalogue — what a door asks when it
    /// shows one that is stale: an event, never a clock. `category` narrows
    /// it to one of the source's categories.
    Ingest {
        /// Which source.
        source: SourceId,
        /// Only this category of it.
        #[serde(default)]
        category: Option<String>,
    },
    /// Fetch the posters of these items — the ones a door shows now — into
    /// the catalogue's posters; [`CatalogSummary::posters_generation`] moves
    /// when some arrived.
    Posters {
        /// Which items.
        items: Vec<ItemId>,
    },
    /// Close the player EVERYWHERE: what plays stops, the queue is
    /// remembered, every watcher is told [`Say::Farewell`] — each door closes
    /// on it — and the daemon leaves, with success, so its unit does not
    /// bring it back. The next thing played starts it again.
    Quit,
}

/// Why something does not work — each a sentence on screen, one per variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "trouble")]
pub enum Trouble {
    /// The file could not be played.
    Unplayable {
        /// What mpv said.
        why: String,
        /// What the program behind mpv said of it — yt-dlp's last `ERROR:`
        /// line: DATA, the program's own words, which a door frames with a
        /// key of its own and never takes for a sentence of ours.
        #[serde(default)]
        said: Option<String>,
    },
    /// It is already in the queue ([`Ask::Put`]'s `once`): the entry that
    /// holds it, and what it is called — the door asks whether to add it
    /// again, "no" first.
    AlreadyQueued {
        /// The leaf at the same place.
        entry: EntryId,
        /// Its name.
        name: Name,
    },
    /// The queue's file changed on the disk since it was opened or saved
    /// ([`Ask::SaveQueueAgain`]): writing over it is the person's to say.
    ChangedOnDisk {
        /// Its path.
        path: String,
    },
    /// The queue was never opened from a file nor saved as one: there is
    /// nowhere to save it again ([`Ask::SaveQueueAgain`]).
    NoOrigin,
    /// This player knows the ask and does not do it yet: an honest refusal,
    /// never an ask that seems to work. `ask` is its kebab-case name, for
    /// the log: a door says the sentence, never the name.
    NotYet {
        /// Which ask.
        ask: String,
    },
    /// A sound lane whose source has no sound to play.
    NoSound {
        /// The lane.
        lane: LaneId,
    },
    /// The lyrics could not be translated: why, as the translating crate
    /// says it (`ronoctua_words::Why`, one sentence each), and the host it
    /// concerns.
    Translation(ronoctua_words::Refusal),
    /// A decoder this format needs is not installed.
    NoDecoder {
        /// Its program.
        program: String,
    },
    /// This format is known and nothing here can play it.
    Unsupported {
        /// The format's name.
        format: String,
    },
    /// The engine could not start.
    NoEngine {
        /// Why.
        why: String,
    },
    /// Another machine could not be reached, or said no.
    Remote {
        /// What happened, in the transport's words.
        why: String,
    },
    /// The ask was not one this player knows — a door newer or older than
    /// it. Never silence: the door says the two do not match.
    NotUnderstood {
        /// What the reader said, in its own words.
        why: String,
    },
    /// It would pass a ceiling; the number is in it.
    TooMany(TooMany),
    /// No entry has this id (any more): the door's outline is older than the
    /// queue.
    NoSuchEntry {
        /// The id asked about.
        id: EntryId,
    },
    /// Only a playlist holds entries.
    NotAPlaylist {
        /// The entry that is not one.
        id: EntryId,
    },
    /// A playlist cannot go inside itself.
    IntoItself,
    /// A playlist always has a name.
    Nameless,
    /// A file could not be opened or read.
    Unreadable {
        /// Its path.
        path: String,
    },
    /// A file could not be written.
    Unwritable {
        /// Its path.
        path: String,
        /// What the system said — the system's words, never a sentence of
        /// ours: those are variants of their own, each with its key.
        why: String,
    },
    /// No folder took a file fetched for `path` — beside it, and the
    /// player's own folder under the person's data both refused.
    NowhereToSave {
        /// The file it was fetched for.
        path: String,
    },
    /// The session has no runtime directory (`XDG_RUNTIME_DIR`), where the
    /// player keeps its drafts and what it fetched to be seen.
    NoRuntimeDir,
    /// One of our files is larger than any we write.
    TooLarge {
        /// Its path.
        path: String,
    },
    /// It is not one of our files, whatever its name says.
    NotOurs {
        /// Its path.
        path: String,
    },
    /// A newer player wrote it.
    Newer {
        /// Its path.
        path: String,
        /// The version it says it is.
        found: u32,
    },
    /// It is ours, and something in it is not what the format says.
    Damaged {
        /// Its path.
        path: String,
    },
    /// A list of things to play names nothing that plays.
    Empty {
        /// Its path or address.
        path: String,
    },
    /// A program this needs is not on this machine.
    Missing {
        /// Its name.
        program: String,
    },
    /// A sound output a lane was sent to is gone; it plays on the default.
    SinkGone {
        /// The output's name.
        device: String,
    },
    /// The translator's server address kept in `player.json` is no
    /// `https://` address with a host: nothing was asked.
    NotAnAddress {
        /// The address as it is kept — DATA, framed by a door's sentence.
        address: String,
    },
    /// An address on the network did not answer, or answered no.
    Unreachable {
        /// The address.
        what: String,
        /// What happened, in the transport's words.
        why: String,
    },
    /// Every address of a stream failed.
    StreamDown {
        /// The stream's title, else its first address.
        title: String,
        /// How many addresses were tried.
        tried: usize,
    },
    /// yt-dlp could not list a playlist.
    Listing {
        /// The address.
        what: String,
        /// Why, as a closed set.
        why: ronoctua_ytdlp::Trouble,
    },
    /// A subtitle file could not be read.
    Subtitle {
        /// Its path.
        path: String,
        /// Why.
        why: crate::subtitles::Refusal,
    },
    /// A subtitle file was opened with nothing to go over: no film in the
    /// queue has its name, nothing with a picture plays, and no film beside
    /// it has its name.
    NoPicture {
        /// Its path.
        path: String,
    },
    /// A lane was asked about, and nothing plays.
    NothingPlaying,
    /// No lane of what plays has this id (any more).
    NoSuchLane {
        /// The id asked about.
        lane: LaneId,
    },
    /// This lane is not of the kind that change is for — a sound output for
    /// a picture, a band for a sound, a cover of a picture.
    WrongKind {
        /// The lane.
        lane: LaneId,
    },
    /// A value out of its range for this lane: a start before the set's,
    /// a trim that ends before it begins, a volume past the most, subtitles
    /// laid over what is not a picture.
    BadValue {
        /// The lane.
        lane: LaneId,
    },
    /// A set keeps at least one lane.
    LastLane,
    /// This machine plays in step with another, and what plays is that
    /// machine's: its lanes are changed there.
    Following {
        /// The other machine's name.
        host: String,
    },
    /// A stream from the network plays alone: its relays and the headers it
    /// is asked with are not a lane's.
    StreamPlaysAlone,
    /// This machine has no ambient sounds: none were installed with the
    /// player.
    NoAmbience,
    /// No ambient sound has this id.
    NoSuchSound {
        /// The id asked about.
        id: String,
    },
    /// An ambient sound would not play, and is off.
    SoundUnplayable {
        /// Its id.
        id: String,
        /// What mpv said.
        why: String,
    },
    /// A file's tags could not be read or written; the file is as it was.
    Tags {
        /// The file's path (or the address).
        path: String,
        /// Why, as the tags crate says it — one sentence per reason.
        why: ronoctua_tags::Why,
    },
    /// No draft has this id (any more): it was written or let go.
    NoSuchDraft {
        /// The id asked about.
        id: DraftId,
    },
    /// A lookup gave nothing: which service, and why — one sentence per
    /// reason (`ronoctua_lookup::Why`).
    Lookup(ronoctua_lookup::Refusal),
    /// The person has not let the player ask this host: a press on a row
    /// that names it is the consent ([`crate::prefs::Preferences::consent`]).
    Off {
        /// The host, as the row names it.
        host: String,
    },
    /// A file is there, and replacing it is the person's to say: the door
    /// asks ("no" first) and asks again with `replace`.
    Exists {
        /// Its path.
        path: String,
    },
    /// No candidate of that desk has this number (any more): a newer search
    /// replaced the page.
    NoSuchCandidate {
        /// The number asked about.
        candidate: CandidateId,
    },
    /// A directory gave nothing: which one, and why — one sentence per
    /// reason (`ronoctua_broadcast::Why`).
    Directory(ronoctua_broadcast::Refusal),
    /// No category of "channels by theme" has this id, or it has nothing of
    /// what was asked (no film collection, no live words).
    NoSuchCategory {
        /// The id asked about.
        category: String,
    },
    /// No directory's page, the recently played nor the favourites have
    /// this: there is nothing to keep of it.
    NoSuchPick {
        /// What it was said to be.
        kind: ronoctua_broadcast::picks::Kind,
        /// Its id.
        id: String,
    },
    /// The favourites or the recently played could not be changed.
    Picks {
        /// Why.
        why: ronoctua_broadcast::picks::Trouble,
    },
    /// A source of the catalogue gave nothing — a pasted link that is none
    /// of the shapes read, a host that did not answer, a host not let — one
    /// sentence per reason (`ronoctua_broadcast::sources::Refused`).
    Source {
        /// Why.
        why: ronoctua_broadcast::sources::Refused,
    },
    /// The catalogue's files did not do what was asked — one sentence per
    /// reason (`ronoctua_broadcast::catalog::Trouble`).
    Catalog {
        /// Why.
        why: ronoctua_broadcast::catalog::Trouble,
    },
    /// The person's sources' file did not do what was asked.
    Sources {
        /// Why.
        why: SourcesTrouble,
    },
    /// No source of the catalogue has this id (any more) — or it is a
    /// default source, which is never removed nor marked adult.
    NoSuchSource {
        /// The id asked about.
        id: SourceId,
    },
}

impl From<crate::queue::Refusal> for Trouble {
    fn from(refusal: crate::queue::Refusal) -> Self {
        use crate::queue::Refusal;
        match refusal {
            Refusal::TooMany(t) => Self::TooMany(t),
            Refusal::NoSuchEntry { id } => Self::NoSuchEntry { id },
            Refusal::NotAPlaylist { id } => Self::NotAPlaylist { id },
            Refusal::IntoItself => Self::IntoItself,
            Refusal::Nameless => Self::Nameless,
        }
    }
}

impl Trouble {
    /// One of our files refused, as the trouble about `path`.
    #[must_use]
    pub fn of_own(refusal: crate::own::Refusal, path: &str) -> Self {
        use crate::own::Refusal;
        let path = path.to_owned();
        match refusal {
            Refusal::Unreadable => Self::Unreadable { path },
            Refusal::TooLarge => Self::TooLarge { path },
            Refusal::NotOurs => Self::NotOurs { path },
            Refusal::Newer { found } => Self::Newer { path, found },
            Refusal::Damaged => Self::Damaged { path },
        }
    }
}

/// What one lane changes into ([`Ask::Lane`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "what")]
pub enum LaneChange {
    /// When it begins on the set's clock.
    Start {
        /// Seconds, never below zero.
        seconds: f64,
        /// ITS VOICE MOVES WITH IT: the lane that shares its voice — a
        /// picture's own sound, brought with it ([`Ask::AddLane`]) — begins
        /// there too, so the two stay one voice and in step. What a picture
        /// dragged along the timeline sends; `false` moves this lane alone,
        /// which is how a sound is put back in step with its picture
        /// (lip-sync), typed on the sound's row.
        #[serde(default)]
        voice: bool,
    },
    /// Which stretch of its source plays.
    Trim {
        /// Where in the source it begins, seconds.
        from_seconds: f64,
        /// Where it stops; `None` is the source's end.
        #[serde(default)]
        until_seconds: Option<f64>,
        /// ITS VOICE IS TRIMMED WITH IT, as [`LaneChange::Start`]'s `voice`.
        #[serde(default)]
        voice: bool,
    },
    /// On or off; off, it is kept and costs nothing.
    Enabled {
        /// On.
        on: bool,
    },
    /// Its own volume, under the player's.
    Volume {
        /// Percent, up to 130.
        percent: u8,
    },
    /// Its own mute.
    Mute {
        /// Muted.
        muted: bool,
    },
    /// The screens its picture goes to.
    Screens {
        /// Which.
        screens: Screens,
    },
    /// The sound output it goes to: a sink's `device` from
    /// [`State::sinks`], or `None` for the machine's default. Sounds only.
    Sink {
        /// The output.
        #[serde(default)]
        device: Option<String>,
    },
    /// Where its lines sit. Subtitles only.
    Stack {
        /// The band.
        stack: Stack,
    },
    /// The picture it goes over; `None` is the set's first. Subtitles only.
    Over {
        /// A picture lane.
        #[serde(default)]
        over: Option<LaneId>,
    },
    /// What it is called; `None` gives it back its source's own.
    Title {
        /// The name.
        #[serde(default)]
        title: Option<String>,
    },
    /// The language it keeps ([`Lane::lang`](crate::own::Lane)) — which of a
    /// subtitle file of several languages it shows (a SAMI's class): one of
    /// its [`LaneState::languages`]; `None` gives it back the first that
    /// says something. Subtitles only.
    Language {
        /// The language's code, as the file declares it.
        #[serde(default)]
        lang: Option<String>,
    },
    /// Take it out of the set.
    Remove,
}

/// What kind of track a source holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackKind {
    /// A picture.
    Video,
    /// A sound.
    Audio,
    /// Subtitles.
    Sub,
}

/// A track of a source that plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// mpv's id.
    pub id: i64,
    /// What it is.
    pub kind: TrackKind,
    /// Its title, when it has one.
    pub title: Option<String>,
    /// Its language, when declared.
    pub lang: Option<String>,
    /// Its codec.
    pub codec: Option<String>,
    /// A lane plays it now.
    pub selected: bool,
    /// A picture that is a cover, not a film.
    #[serde(default)]
    pub albumart: bool,
    /// The file says to play it when nothing is chosen.
    #[serde(default)]
    pub default: bool,
    /// The file says to show it always (subtitles for foreign words).
    #[serde(default)]
    pub forced: bool,
    /// It comes from another file, not from the source itself.
    #[serde(default)]
    pub external: bool,
}

/// A source of the lanes that play, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// The file or address.
    pub location: String,
    /// Its tracks, once read.
    pub tracks: Vec<Track>,
}

/// A line of the lyrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// When it is sung, seconds; `None` for unsynchronised lyrics.
    pub at: Option<f64>,
    /// The words.
    pub text: String,
    /// The same line in the language asked ([`Ask::LyricsTranslation`]),
    /// when it is translated.
    #[serde(default)]
    pub translated: Option<String>,
}

/// The lyrics' translation, as the state says it
/// ([`State::lyrics_translation`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Translation {
    /// Into which language, by its code.
    pub to: String,
    /// From which, when the lyrics or the translator said.
    #[serde(default)]
    pub from: Option<String>,
    /// How it goes.
    pub status: TranslationStatus,
    /// THE TRANSLATION AS ONE BLOCK, when it did not come back a line for
    /// every line: under each verse it would be the wrong verse, so no line
    /// carries [`LyricLine::translated`] and a door shows this apart, not
    /// in step with the song.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsynced: Option<String>,
}

/// How a translation goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum TranslationStatus {
    /// Being asked.
    Translating,
    /// The lines carry it; `by` is who translated them, as a door credits it.
    Translated {
        /// The translator's name.
        by: String,
    },
    /// Not translated, and why.
    Refused(Trouble),
}

/// The local catalogue, as the state says it: a door reads the catalogue's
/// file itself (read-only) and reads it again when a generation moves.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogSummary {
    /// Moves every time an ingest is written into the catalogue.
    pub generation: u64,
    /// Moves every time posters arrived ([`Ask::Posters`]).
    pub posters_generation: u64,
    /// Every source: the defaults (`assets/channels.nix`), then the
    /// person's in the order they were added.
    pub sources: Vec<SourceStatus>,
    /// Why the person's sources could not be read ([`Trouble::Sources`]):
    /// none is added over a file that is there and did not read.
    pub trouble: Option<Trouble>,
}

/// One source of the catalogue, as the state says it: every default source
/// (`assets/channels.nix`), then the person's in the order they were added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceStatus {
    /// Its id.
    pub id: SourceId,
    /// What kind of source it is.
    #[serde(default = "a_playlist")]
    pub kind: ronoctua_broadcast::sources::Kind,
    /// What it calls itself, once read.
    #[serde(default)]
    pub title: Option<String>,
    /// Its address, MASKED ([`SourceUrl::masked`]): the whole one never
    /// leaves the daemon's file.
    pub shown: String,
    /// It is kept in the adult catalogue.
    #[serde(default)]
    pub adult: bool,
    /// Its posters are fetched.
    #[serde(default)]
    pub posters: bool,
    /// Its address is `http`: what it answers crosses the network NOT
    /// ENCRYPTED, and a door says so beside it — a person's source only (a
    /// default source is always `https`).
    #[serde(default)]
    pub plain: bool,
    /// How many items it brought, in the two catalogues together.
    #[serde(default)]
    pub items: u64,
    /// An ingest of it is under way.
    #[serde(default)]
    pub busy: bool,
    /// Why its last ingest failed.
    #[serde(default)]
    pub trouble: Option<Trouble>,
}

/// A source whose kind a daemon of an earlier 3 did not say: the kind every
/// person's source had then.
const fn a_playlist() -> ronoctua_broadcast::sources::Kind {
    ronoctua_broadcast::sources::Kind::Playlist
}

/// Why the person's sources' file (`sources.json`) did not do what was asked
/// — each a sentence on screen (`player-trouble-sources-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourcesTrouble {
    /// There is no config root to keep it in.
    NoHome,
    /// It is there and could not be read.
    Unreadable,
    /// It is there and is not the sources' file (another version, damaged):
    /// it is kept as it is, and nothing is written over it.
    Unparseable,
    /// It could not be written.
    Unwritable,
}

impl SourcesTrouble {
    /// Every one, for a test that walks the words.
    pub const ALL: [Self; 4] = [
        Self::NoHome,
        Self::Unreadable,
        Self::Unparseable,
        Self::Unwritable,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoHome => "no-home",
            Self::Unreadable => "unreadable",
            Self::Unparseable => "unparseable",
            Self::Unwritable => "unwritable",
        }
    }
}

/// Playing in step with another machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joined {
    /// Its name.
    pub host: String,
    /// What this machine plays of it.
    pub role: crate::remote::Role,
    /// How far this machine is from it, in milliseconds.
    pub drift_ms: i64,
}

/// What a lane that is on is doing now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneState {
    /// Which lane.
    pub lane: LaneId,
    /// Its length, once known, in seconds.
    pub duration_seconds: Option<f64>,
    /// What it is doing.
    pub status: LaneStatus,
    /// It has a picture to show now.
    pub picture: bool,
    /// How far from the session's clock it is, milliseconds: ahead is
    /// positive.
    pub drift_ms: i64,
    /// It keeps the session's time.
    pub master: bool,
    /// Its picture's size, when it has one.
    pub size: Option<(u32, u32)>,
    /// Its picture is a cover, not a film.
    pub cover: bool,
    /// THE LANGUAGES ITS FILE CARRIES, when a subtitle file holds several and
    /// shows one (a SAMI's classes), by their codes in the file's order:
    /// what [`LaneChange::Language`] chooses among. None otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
}

/// How long a lane that is off runs ([`State::lengths_off`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LaneLength {
    /// Which lane.
    pub lane: LaneId,
    /// Its source's length, in seconds.
    pub seconds: f64,
}

/// What a lane is doing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum LaneStatus {
    /// Its start is still ahead.
    Pending,
    /// It plays.
    Playing,
    /// It reached its end.
    Ended,
    /// It is being made ready (a subtitle converted, a file fetched).
    Preparing,
    /// It could not play, and why.
    Refused(Trouble),
}

/// A sound output a lane can be sent to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sink {
    /// Its name for the sound server, which is what a lane keeps.
    pub device: String,
    /// What the sound server calls it for people.
    pub description: String,
    /// What kind of output it is.
    pub kind: SinkKind,
}

/// What kind of output a sink is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SinkKind {
    /// Headphones or a headset.
    Headphones,
    /// Speakers.
    Speakers,
    /// Something over Bluetooth.
    Bluetooth,
    /// Anything else the sound server offers.
    Other,
}

/// The ambient sounds of this machine, as the state says them
/// ([`crate::ambience`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ambience {
    /// Every sound this machine has, on or off, in the order they were
    /// declared; none when it has none, and `trouble` says why.
    pub sounds: Vec<Sounding>,
    /// How many may sound at once.
    pub at_once: u8,
    /// The ones that are on are held ([`Ask::AmbientToggle`]).
    pub paused: bool,
    /// Why this machine has no ambient sounds: none installed
    /// ([`Trouble::NoAmbience`]), or a catalogue it could not read.
    pub trouble: Option<Trouble>,
}

/// One ambient sound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sounding {
    /// Its id: its name is the text key `player-ambience-<id>`.
    pub id: String,
    /// What kind of sound it is: `player-ambience-group-<group>`.
    pub group: String,
    /// It is on: playing, or held with the others.
    pub on: bool,
    /// Its own volume, percent — kept while it is off, for the next time.
    pub volume_percent: u8,
    /// How many times it went round since it was turned on: the proof that a
    /// loop plays on past its own length.
    pub rounds: u64,
    /// Who recorded it, and under what licence.
    pub credit: Credit,
}

/// Who a recording is owed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credit {
    /// The person who recorded it, by the name they published under.
    pub author: String,
    /// Where it was published.
    pub page: String,
    /// Under what terms.
    pub licence: Licence,
}

/// The terms an ambient sound is under — only these two are taken
/// (`nix/ambience.nix` refuses any other).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Licence {
    /// Creative Commons Zero: given to the public domain.
    #[serde(rename = "CC0-1.0")]
    Cc0,
    /// In the public domain.
    #[serde(rename = "public-domain")]
    PublicDomain,
}

/// A page of results the daemon keeps for the doors ([`crate::find::Desk`]),
/// as the state names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeskKind {
    /// Tags suggested for what plays.
    Suggestions,
    /// Subtitles found on the network.
    Subtitles,
    /// Lyrics found on the network.
    Lyrics,
    /// The world's radio: countries, genres, languages, stations
    /// ([`crate::find::RadioList`]).
    Radio,
    /// The world's TV: countries, a country's channels, a search
    /// ([`crate::find::TvList`]).
    Television,
}

/// A desk, as the state names it: a door asks for its page when the
/// generation moves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeskSummary {
    /// Which desk.
    pub kind: DeskKind,
    /// Moves every time its page changes.
    pub generation: u64,
    /// A search or a take is under way.
    pub busy: bool,
    /// Why the last search gave nothing, or the last take failed.
    pub trouble: Option<Trouble>,
}

/// A several-line field open in the editor ([`Ask::Draft`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DraftId(pub u64);

/// A draft the daemon keeps and watches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drafted {
    /// Its id.
    pub id: DraftId,
    /// The file it goes into.
    pub location: String,
    /// The field.
    pub field: Field,
    /// Where it is, for a door that opens it in the editor again.
    pub path: String,
    /// It was saved with a value that is not the file's: a door asks
    /// whether to write it ("no" drawn first), and the answer is
    /// [`Ask::WriteDraft`], or [`Ask::KeepDraft`] for not now — the draft
    /// stays, and is asked about when it is saved again; [`Ask::DropDraft`]
    /// throws it away. The question survives the
    /// door that opened the editor — any door asks it.
    pub saved: bool,
}

/// The writing of a file's tags, as the state says it ([`State::retag`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retagging {
    /// The file.
    pub location: String,
    /// How it goes.
    pub status: RetagStatus,
    /// How many more wait behind it.
    pub waiting: usize,
}

/// How a write goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum RetagStatus {
    /// Being written.
    Writing {
        /// How far, in thousandths.
        permille: u16,
    },
    /// Written: the file is the new one.
    Written,
    /// A cover was taken out into this file ([`Ask::SaveCover`]).
    Saved {
        /// The new file.
        path: String,
    },
    /// Not written, and why; the file is as it was.
    Refused(Trouble),
}

/// The state: small, and sent whole on every change.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// [`PROTOCOL`]; zero is a daemon from before there was one.
    pub protocol: u32,
    /// The revision of the queue's outline this state goes with.
    pub revision: u64,
    /// Which leaf of the queue is the current one.
    pub current: Option<EntryId>,
    /// Where what plays is: the leaf's file or address, or another machine's
    /// while this one plays in step with it.
    pub location: Option<String>,
    /// It is playing (not paused, not stopped).
    pub playing: bool,
    /// Seconds into the item.
    pub position: f64,
    /// Its length, when known.
    pub duration: Option<f64>,
    /// Volume, percent.
    pub volume: u8,
    /// Muted.
    pub muted: bool,
    /// Speed factor.
    pub speed: f64,
    /// Repeat mode.
    pub repeat: Repeat,
    /// Shuffled.
    pub shuffle: bool,
    /// The item's title: the name the queue gives it, else its own tag.
    pub title: Option<String>,
    /// Its artist.
    pub artist: Option<String>,
    /// Its album.
    pub album: Option<String>,
    /// What plays after it, in the order it will play (shuffled or not).
    pub next: Option<Name>,
    /// Entries still being read from the network: shown, not yet playable.
    pub resolving: Vec<EntryId>,
    /// The lanes of what plays: a set's own, or — for a file that plays
    /// alone — its picture, its sound and its subtitles, which a change
    /// makes a set's.
    pub lanes: Vec<Lane>,
    /// What each lane that is on is doing.
    pub lane_states: Vec<LaneState>,
    /// THE LENGTHS OF THE LANES THAT ARE OFF — a picture or a sound switched
    /// off has no voice to say how long it runs, and its band would run to
    /// the track's end like a live one: as the file said it while it played,
    /// or as its sheet reads it. A lane whose length is not known yet is not
    /// here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lengths_off: Vec<LaneLength>,
    /// How the pictures of what plays share a screen.
    pub arrangement: Option<Arrangement>,
    /// The sources of the lanes, with their tracks.
    pub sources: Vec<Source>,
    /// The sound outputs a lane can be sent to, while a door looks at them
    /// ([`Ask::Sinks`]) or a lane is sent to one.
    pub sinks: Vec<Sink>,
    /// The ambient sounds, alongside the queue and apart from it.
    pub ambience: Ambience,
    /// Its lyrics, when found.
    pub lyrics: Vec<LyricLine>,
    /// Where the lyrics came from: `file` (a `.lrc` beside it), `tag` (its
    /// own tags), or the host they were asked of (`prefs::LRCLIB_HOST`).
    pub lyrics_from: Option<String>,
    /// The lyrics' translation, while one is asked for
    /// ([`Ask::LyricsTranslation`]).
    pub lyrics_translation: Option<Translation>,
    /// Where the queue is kept: nowhere yet, a file it is the same as, or
    /// one it changed from — what the Queue's title says.
    pub queue_file: crate::queue::QueueKept,
    /// The local catalogue's summary.
    pub catalog: CatalogSummary,
    /// What is wrong now.
    pub troubles: Vec<Trouble>,
    /// The pages of results the daemon keeps — ALWAYS the five, in
    /// [`DeskKind`]'s order: the lookups' three (suggestions, subtitles,
    /// lyrics) and the directories' two (radio, television), each with its
    /// generation. Channels by theme are the local catalogue's
    /// ([`State::catalog`]), read by the doors themselves.
    pub desks: Vec<DeskSummary>,
    /// Moves when what is known of a playing file's tags changes — it was
    /// read, written, refreshed, or a stream said something new. A door
    /// that shows a sheet asks for it again ([`Ask::Sheet`]).
    pub sheet_generation: u64,
    /// The last write of a file's tags: under way, written, or refused.
    pub retag: Option<Retagging>,
    /// The several-line fields open in the editor.
    pub drafts: Vec<Drafted>,
    /// Addresses that did not answer this session.
    pub unanswered: Vec<String>,
    /// The pairing code this machine is showing, while it lasts.
    pub invite: Option<String>,
    /// This machine's name on the tailnet, when it is on one.
    pub tailnet: Option<String>,
    /// The other machines on the tailnet that are online — who can be paired.
    pub nearby: Vec<String>,
    /// Paired machines this one can join.
    pub hosts: Vec<String>,
    /// Paired machines that can join this one.
    pub guests: Vec<String>,
    /// The machine this one plays in step with, when it does.
    pub joined: Option<Joined>,
}

impl State {
    /// Nothing of the queue is current, and an ambient sound is on: the
    /// modeline's pair stands for the ambience — its toggle is
    /// [`Ask::AmbientToggle`], its glyph says [`Ambience::paused`], and
    /// "next" has nothing to go to. With a leaf current, even stopped, the
    /// pair is the queue's.
    #[must_use]
    pub fn only_ambience(&self) -> bool {
        self.current.is_none() && self.ambience.sounds.iter().any(|s| s.on)
    }
}

/// An answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "say")]
pub enum Say {
    /// The whole state.
    State(Box<State>),
    /// The whole outline of the queue: to a watcher when its shape changes,
    /// and to [`Ask::Queue`].
    Queue(Box<Outline>),
    /// A preview's geometry; the memfd came with this line.
    Preview {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
        /// Slots in the ring.
        slots: u32,
    },
    /// Taken.
    Done,
    /// Not taken, and why.
    Refused(Trouble),
    /// Everything a file says about itself ([`Ask::Sheet`]): only ever to
    /// the connection that asked.
    Sheet(Box<Sheet>),
    /// A desk's page ([`Ask::Desk`]): only ever to the connection that
    /// asked.
    Desk(Box<Desk>),
    /// Where a draft is ([`Ask::Draft`]).
    Draft {
        /// Its id.
        id: DraftId,
        /// The file to open in the editor.
        path: String,
    },
    /// The player is closing because somebody asked ([`Ask::Quit`]): the
    /// last line a watcher reads. A door closes on it — and on a stream that
    /// merely ENDS it does not: a player that crashed is a player a door
    /// says is not running.
    Farewell,
}

/// One JSON object and a newline, in ONE write.
///
/// Made whole first and written at once: serialised straight into a socket,
/// every token was a write of its own, and a Unix socket counts each write's
/// overhead against its buffer — a state of two kilobytes took two hundred
/// of them and filled the buffer by itself. A door that paused reading for a
/// moment then had a line cut in the middle by the daemon's write deadline,
/// and read half a state (measured: a watcher dropped with its last line
/// ending after a colon).
pub fn write_line<T: Serialize>(out: &mut impl Write, value: &T) -> Result<()> {
    out.write_all(&line_of(value)?)?;
    out.flush()?;
    Ok(())
}

/// One JSON object and its newline, as the bytes [`write_line`] writes — for
/// a daemon that says the same line to many.
pub fn line_of<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    Ok(line)
}

/// One line, parsed; `None` at the end of the stream.
pub fn read_line<T: DeserializeOwned>(input: &mut impl BufRead) -> Result<Option<T>> {
    match read_raw(input)? {
        None => Ok(None),
        Some(line) => Ok(Some(serde_json::from_str(&line)?)),
    }
}

/// One line, unparsed; `None` at the end of the stream — for a reader that
/// tells "nothing came" from "something came that is not an ask".
pub fn read_raw(input: &mut impl BufRead) -> Result<Option<String>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim_end().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: an ask or an answer stops crossing the socket as the words
    /// the other side reads.
    #[test]
    fn an_ask_and_an_answer_round_trip() {
        let a = Ask::Put {
            what: vec![
                Thing::Location("/music/a.flac".into()),
                Thing::Stream {
                    stream: Stream::typed("https://radio.example/live"),
                },
            ],
            after: Some(EntryId(4)),
            into: None,
            play: true,
            replace: false,
            once: true,
        };
        let s = serde_json::to_string(&a).expect("serialises");
        assert!(s.contains("\"ask\":\"put\""), "{s}");
        assert!(s.contains("\"what\":[\"/music/a.flac\",{\"stream\""), "{s}");
        assert_eq!(serde_json::from_str::<Ask>(&s).expect("parses"), a);
        let state = Say::State(Box::default());
        let s = serde_json::to_string(&state).expect("serialises");
        assert_eq!(serde_json::from_str::<Say>(&s).expect("parses"), state);
    }

    /// FAILS IF: the short spelling a door and a test write stops being read
    /// — a bare path, and every place field left out.
    #[test]
    fn a_put_is_read_as_people_write_it() {
        let a: Ask = serde_json::from_str(r#"{"ask":"put","what":["/m/a.flac"],"play":true}"#)
            .expect("a put");
        assert_eq!(
            a,
            Ask::Put {
                what: vec![Thing::Location("/m/a.flac".into())],
                after: None,
                into: None,
                play: true,
                replace: false,
                once: false,
            }
        );
        let m: Ask = serde_json::from_str(r#"{"ask":"move","id":3}"#).expect("a move");
        assert_eq!(
            m,
            Ask::Move {
                id: EntryId(3),
                after: None,
                into: None
            }
        );
        let j: Ask = serde_json::from_str(r#"{"ask":"jump","id":7}"#).expect("a jump");
        assert_eq!(j, Ask::Jump { id: EntryId(7) });
    }

    /// FAILS IF: the outline stops crossing the socket whole, or the state
    /// stops saying which protocol it speaks and which outline it goes with.
    #[test]
    fn the_outline_and_the_state_cross_whole() {
        let mut queue = crate::queue::Queue::new();
        queue
            .insert(
                crate::queue::Place::End(None),
                vec![crate::own::Item::Playlist {
                    name: "list".into(),
                    from: None,
                    entries: vec![crate::own::Item::Single {
                        location: "/m/a.flac".into(),
                        title: None,
                        seconds: None,
                    }],
                }],
            )
            .expect("inserted");
        let say = Say::Queue(Box::new(queue.outline()));
        let s = serde_json::to_string(&say).expect("serialises");
        assert!(s.starts_with(r#"{"say":"queue","revision":"#), "{s}");
        assert_eq!(serde_json::from_str::<Say>(&s).expect("parses"), say);

        let state = State {
            protocol: PROTOCOL,
            revision: queue.revision(),
            current: Some(EntryId(2)),
            next: Some(Name::Location("/m/b.flac".into())),
            ..State::default()
        };
        let s = serde_json::to_string(&state).expect("serialises");
        assert!(
            s.contains(r#""protocol":3"#) && s.contains(r#""current":2"#),
            "{s}"
        );
        // A DAEMON FROM BEFORE THERE WAS A PROTOCOL says nothing of it, and
        // reads as zero: the door knows it is old.
        let old: State = serde_json::from_str(r#"{"playing":true}"#).expect("an old state");
        assert_eq!(old.protocol, 0);
    }

    /// FAILS IF: a trouble loses the facts its sentence is made of — the
    /// ceiling's number, the entry, the version a file says it is.
    #[test]
    fn a_trouble_carries_its_facts() {
        let t = Trouble::from(crate::queue::Refusal::TooMany(TooMany {
            what: crate::limit::Limit::Leaves,
            most: 10_000,
        }));
        let s = serde_json::to_string(&t).expect("serialises");
        assert_eq!(s, r#"{"trouble":"too-many","what":"leaves","most":10000}"#);
        assert_eq!(serde_json::from_str::<Trouble>(&s).expect("parses"), t);
        let n = Trouble::of_own(crate::own::Refusal::Newer { found: 3 }, "/q.rqueue");
        let s = serde_json::to_string(&n).expect("serialises");
        assert_eq!(s, r#"{"trouble":"newer","path":"/q.rqueue","found":3}"#);
        let l = Trouble::Listing {
            what: "https://x".into(),
            why: ronoctua_ytdlp::Trouble::Absent,
        };
        let s = serde_json::to_string(&l).expect("serialises");
        assert_eq!(serde_json::from_str::<Trouble>(&s).expect("parses"), l);
    }

    /// FAILS IF: an ask about lanes stops being read as a door writes it —
    /// the change nested under its own tag, the picture asked by its lane —
    /// or an ask of the participants a set's lanes replaced is still taken.
    #[test]
    fn the_lanes_are_asked_as_doors_write_them() {
        let read = |s: &str| serde_json::from_str::<Ask>(s).expect(s);
        assert_eq!(
            read(r#"{"ask":"lane","lane":2,"change":{"what":"volume","percent":80}}"#),
            Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Volume { percent: 80 }
            }
        );
        assert_eq!(
            read(r#"{"ask":"lane","lane":3,"change":{"what":"remove"}}"#),
            Ask::Lane {
                lane: LaneId(3),
                change: LaneChange::Remove
            }
        );
        assert_eq!(
            read(r#"{"ask":"lane","lane":2,"change":{"what":"sink"}}"#),
            Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Sink { device: None }
            }
        );
        assert_eq!(
            read(r#"{"ask":"add-lane","source":"/m/b.mkv","kind":"video"}"#),
            Ask::AddLane {
                source: "/m/b.mkv".into(),
                kind: LaneKind::Video,
                track: None,
                start_seconds: 0.0,
                over: None,
            }
        );
        assert_eq!(
            read(r#"{"ask":"track","lane":3,"track":2}"#),
            Ask::Track {
                lane: LaneId(3),
                track: Some(2)
            }
        );
        assert_eq!(
            read(r#"{"ask":"preview","width":320,"height":180,"lane":3}"#),
            Ask::Preview {
                width: 320,
                height: 180,
                lane: Some(LaneId(3))
            }
        );
        assert_eq!(read(r#"{"ask":"sinks"}"#), Ask::Sinks);
        for gone in [
            r#"{"ask":"follow","item":"/m/b.mkv"}"#,
            r#"{"ask":"unfollow","index":0}"#,
            r#"{"ask":"audio-track","id":2}"#,
            r#"{"ask":"subtitle-track","id":null}"#,
        ] {
            assert!(
                serde_json::from_str::<Ask>(gone).is_err(),
                "{gone} is still an ask"
            );
        }
        let every = Ask::Lane {
            lane: LaneId(1),
            change: LaneChange::Trim {
                from_seconds: 1.5,
                until_seconds: Some(9.0),
                voice: true,
            },
        };
        let s = serde_json::to_string(&every).expect("serialises");
        assert_eq!(serde_json::from_str::<Ask>(&s).expect("parses"), every);
        // A DOOR OF PROTOCOL 2 moves one lane: no `voice` is `false`.
        let old: Ask = serde_json::from_str(
            r#"{"ask":"lane","lane":2,"change":{"what":"start","seconds":3.5}}"#,
        )
        .expect("an older door's start");
        assert_eq!(
            old,
            Ask::Lane {
                lane: LaneId(2),
                change: LaneChange::Start {
                    seconds: 3.5,
                    voice: false
                }
            }
        );
    }

    /// FAILS IF: a lane's refusal loses the lane it is about, or a state
    /// stops carrying what each lane does.
    #[test]
    fn a_lane_s_trouble_and_state_carry_their_facts() {
        let t = Trouble::WrongKind { lane: LaneId(4) };
        let s = serde_json::to_string(&t).expect("serialises");
        assert_eq!(s, r#"{"trouble":"wrong-kind","lane":4}"#);
        let gone = Trouble::SinkGone {
            device: "bluez_output.x".into(),
        };
        let s = serde_json::to_string(&gone).expect("serialises");
        assert_eq!(s, r#"{"trouble":"sink-gone","device":"bluez_output.x"}"#);
        let state = State {
            lane_states: vec![LaneState {
                lane: LaneId(2),
                duration_seconds: Some(30.0),
                status: LaneStatus::Refused(Trouble::Unplayable {
                    why: "x".into(),
                    said: None,
                }),
                picture: false,
                drift_ms: -12,
                master: true,
                size: None,
                cover: true,
                languages: Vec::new(),
            }],
            ..State::default()
        };
        let s = serde_json::to_string(&state).expect("serialises");
        assert!(
            s.contains(
                r#""status":{"status":"refused","trouble":"unplayable","why":"x","said":null}"#
            ),
            "{s}"
        );
        assert_eq!(serde_json::from_str::<State>(&s).expect("parses"), state);
    }

    /// FAILS IF: the quit or its farewell stop crossing the socket as the
    /// words the other side reads — a door would never hear why the player
    /// went, and would say "not running" where it should have closed.
    #[test]
    fn a_quit_and_its_farewell_round_trip() {
        let s = serde_json::to_string(&Ask::Quit).expect("serialises");
        assert_eq!(s, r#"{"ask":"quit"}"#);
        assert_eq!(serde_json::from_str::<Ask>(&s).expect("parses"), Ask::Quit);
        let s = serde_json::to_string(&Say::Farewell).expect("serialises");
        assert_eq!(s, r#"{"say":"farewell"}"#);
        assert_eq!(
            serde_json::from_str::<Say>(&s).expect("parses"),
            Say::Farewell
        );
    }

    /// FAILS IF: a line goes out in pieces again — a socket counts every
    /// write against its buffer, and a door that paused had its line cut.
    #[test]
    fn a_line_is_one_write() {
        struct Writes(Vec<usize>);
        impl Write for Writes {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.push(buf.len());
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut out = Writes(Vec::new());
        write_line(&mut out, &Say::State(Box::default())).expect("written");
        assert_eq!(out.0.len(), 1, "{:?}", out.0);
    }

    /// FAILS IF: an ambient ask stops being read as a door writes it, the
    /// state stops carrying what a door shows of every sound — on, volume,
    /// credit, the ceiling, whether they are held — or a refusal loses the
    /// sound it is about.
    #[test]
    fn the_ambient_sounds_are_asked_and_said_as_doors_write_them() {
        let read = |s: &str| serde_json::from_str::<Ask>(s).expect(s);
        assert_eq!(
            read(r#"{"ask":"ambient","id":"rain","on":true}"#),
            Ask::Ambient {
                id: "rain".into(),
                on: true
            }
        );
        assert_eq!(
            read(r#"{"ask":"ambient-volume","id":"rain","percent":60}"#),
            Ask::AmbientVolume {
                id: "rain".into(),
                percent: 60
            }
        );
        assert_eq!(read(r#"{"ask":"ambient-silence"}"#), Ask::AmbientSilence);
        assert_eq!(read(r#"{"ask":"ambient-toggle"}"#), Ask::AmbientToggle);
        let state = State {
            ambience: Ambience {
                sounds: vec![Sounding {
                    id: "rain".into(),
                    group: "storm".into(),
                    on: true,
                    volume_percent: 60,
                    rounds: 3,
                    credit: Credit {
                        author: "someone".into(),
                        page: "https://freesound.org/s/1/".into(),
                        licence: Licence::Cc0,
                    },
                }],
                at_once: 4,
                paused: false,
                trouble: None,
            },
            ..State::default()
        };
        let s = serde_json::to_string(&state).expect("serialises");
        assert!(s.contains(r#""licence":"CC0-1.0""#), "{s}");
        assert!(s.contains(r#""at_once":4"#), "{s}");
        assert_eq!(serde_json::from_str::<State>(&s).expect("parses"), state);
        let t = Trouble::NoSuchSound { id: "wind".into() };
        let s = serde_json::to_string(&t).expect("serialises");
        assert_eq!(s, r#"{"trouble":"no-such-sound","id":"wind"}"#);
        let t = Trouble::TooMany(TooMany {
            what: crate::limit::Limit::Ambience,
            most: 4,
        });
        let s = serde_json::to_string(&t).expect("serialises");
        assert_eq!(s, r#"{"trouble":"too-many","what":"ambience","most":4}"#);
    }

    /// FAILS IF: the modeline's pair is handed to the ambience while a leaf
    /// of the queue is current — stopped or paused, a click would then hold
    /// the rain instead of playing the song — or not handed to it when the
    /// ambience is all that is on.
    #[test]
    fn the_pair_is_the_ambience_s_only_when_nothing_is_current() {
        let on = Sounding {
            id: "rain".into(),
            group: "storm".into(),
            on: true,
            volume_percent: 100,
            rounds: 0,
            credit: Credit {
                author: "a".into(),
                page: "https://x".into(),
                licence: Licence::PublicDomain,
            },
        };
        let mut s = State::default();
        assert!(!s.only_ambience(), "nothing at all");
        s.ambience.sounds = vec![Sounding {
            on: false,
            ..on.clone()
        }];
        assert!(!s.only_ambience(), "a sound that is off");
        s.ambience.sounds = vec![on];
        assert!(s.only_ambience());
        s.current = Some(EntryId(3));
        assert!(!s.only_ambience(), "a leaf is current, even stopped");
    }

    /// FAILS IF: an ask about a file's tags stops being read as a door
    /// writes it — the stamp, the edits under their own tag, a cover asked
    /// by no place — or a sheet, a draft's place or a write's outcome stop
    /// crossing whole.
    #[test]
    fn the_tags_are_asked_and_said_as_doors_write_them() {
        let read = |s: &str| serde_json::from_str::<Ask>(s).expect(s);
        assert_eq!(
            read(r#"{"ask":"sheet","location":"/m/a.flac"}"#),
            Ask::Sheet {
                location: "/m/a.flac".into()
            }
        );
        let stamp = Stamp {
            len: 10,
            mtime_ns: 20,
            ino: 30,
        };
        assert_eq!(
            read(
                r#"{"ask":"retag","location":"/m/a.flac","stamp":{"len":10,"mtime_ns":20,"ino":30},
                    "edits":[{"edit":"set","field":"title","value":"New"},{"edit":"clear","field":"genre"}]}"#
            ),
            Ask::Retag {
                location: "/m/a.flac".into(),
                stamp,
                edits: vec![
                    Edit::Set {
                        field: Field::Title,
                        value: "New".into()
                    },
                    Edit::Clear {
                        field: Field::Genre
                    },
                ],
            }
        );
        assert_eq!(
            read(r#"{"ask":"save-cover","location":"/m/a.flac"}"#),
            Ask::SaveCover {
                location: "/m/a.flac".into(),
                at: None
            }
        );
        assert_eq!(
            read(r#"{"ask":"draft","location":"/m/a.flac","field":"lyrics"}"#),
            Ask::Draft {
                location: "/m/a.flac".into(),
                field: Field::Lyrics
            }
        );
        assert_eq!(
            read(r#"{"ask":"write-draft","id":3}"#),
            Ask::WriteDraft { id: DraftId(3) }
        );
        assert_eq!(
            read(r#"{"ask":"keep-draft","id":3}"#),
            Ask::KeepDraft { id: DraftId(3) }
        );
        assert_eq!(
            read(r#"{"ask":"refresh","location":"https://radio.example/live"}"#),
            Ask::Refresh {
                location: "https://radio.example/live".into()
            }
        );
        let sheet = ronoctua_tags::stream::sheet(
            "https://radio.example/live",
            ronoctua_tags::Kind::Music,
            &[("icy-title".to_owned(), "A - B".to_owned())],
            Vec::new(),
        );
        let said = Say::Sheet(Box::new(sheet));
        let s = serde_json::to_string(&said).expect("serialises");
        assert!(s.starts_with(r#"{"say":"sheet","location":"#), "{s}");
        assert_eq!(serde_json::from_str::<Say>(&s).expect("parses"), said);
        let drafted = Say::Draft {
            id: DraftId(1),
            path: "/run/user/1/ronoctua-player-drafts/1-lyrics.txt".into(),
        };
        let s = serde_json::to_string(&drafted).expect("serialises");
        assert_eq!(serde_json::from_str::<Say>(&s).expect("parses"), drafted);
        let state = State {
            retag: Some(Retagging {
                location: "/m/a.flac".into(),
                status: RetagStatus::Refused(Trouble::Tags {
                    path: "/m/a.flac".into(),
                    why: ronoctua_tags::Why::ChangedSince,
                }),
                waiting: 0,
            }),
            drafts: vec![Drafted {
                id: DraftId(1),
                location: "/m/a.flac".into(),
                field: Field::Lyrics,
                path: "/d/1-lyrics.txt".into(),
                saved: true,
            }],
            ..State::default()
        };
        let s = serde_json::to_string(&state).expect("serialises");
        assert!(
            s.contains(
                r#""status":{"status":"refused","trouble":"tags","path":"/m/a.flac","why":{"why":"changed-since"}}"#
            ),
            "{s}"
        );
        assert_eq!(serde_json::from_str::<State>(&s).expect("parses"), state);
        let w = RetagStatus::Writing { permille: 500 };
        let s = serde_json::to_string(&w).expect("serialises");
        assert_eq!(s, r#"{"status":"writing","permille":500}"#);
    }

    /// FAILS IF: a lookup is not asked as a door writes it — the query under
    /// its own tag, a take with only what its desk needs — or a refusal
    /// loses the service, the host or the file it names.
    #[test]
    fn the_lookups_are_asked_and_refused_as_doors_write_them() {
        use crate::find::{Suggester, Taken};
        let read = |s: &str| serde_json::from_str::<Ask>(s).expect(s);
        assert_eq!(
            read(r#"{"ask":"find","query":{"what":"lyrics","location":"/m/a.flac"}}"#),
            Ask::Find {
                query: Query::Lyrics {
                    location: "/m/a.flac".into(),
                    words: None
                }
            }
        );
        assert_eq!(
            read(
                r#"{"ask":"find","query":{"what":"suggest","location":"/m/a.flac","source":"wikidata","words":"x","tongues":["pt-BR"]}}"#
            ),
            Ask::Find {
                query: Query::Suggest {
                    location: "/m/a.flac".into(),
                    source: Suggester::Wikidata,
                    words: Some("x".into()),
                    tongues: vec!["pt-BR".into()],
                }
            }
        );
        assert_eq!(
            read(r#"{"ask":"desk","kind":"subtitles"}"#),
            Ask::Desk {
                kind: DeskKind::Subtitles
            }
        );
        assert_eq!(
            read(r#"{"ask":"take","desk":"subtitles","candidate":4}"#),
            Ask::Take {
                desk: DeskKind::Subtitles,
                candidate: CandidateId(4),
                fields: Vec::new(),
                cover: false,
                to: LyricsTo::Beside,
                replace: false,
            }
        );
        assert_eq!(
            read(
                r#"{"ask":"take","desk":"suggestions","candidate":2,"fields":["title","album"],"cover":true}"#
            ),
            Ask::Take {
                desk: DeskKind::Suggestions,
                candidate: CandidateId(2),
                fields: vec![Field::Title, Field::Album],
                cover: true,
                to: LyricsTo::Beside,
                replace: false,
            }
        );
        assert_eq!(
            read(r#"{"ask":"key-check","service":"opensubtitles"}"#),
            Ask::KeyCheck {
                service: ronoctua_lookup::Service::OpenSubtitles
            }
        );
        let t = Trouble::Lookup(ronoctua_lookup::Refusal::new(
            ronoctua_lookup::Service::Tmdb,
            ronoctua_lookup::Why::WrongKey,
        ));
        let s = serde_json::to_string(&t).expect("serialises");
        assert_eq!(
            s,
            r#"{"trouble":"lookup","service":"tmdb","why":"wrong-key"}"#
        );
        assert_eq!(serde_json::from_str::<Trouble>(&s).expect("parses"), t);
        let off = Trouble::Off {
            host: "musicbrainz.org".into(),
        };
        let s = serde_json::to_string(&off).expect("serialises");
        assert_eq!(s, r#"{"trouble":"off","host":"musicbrainz.org"}"#);
        let there = Trouble::Exists {
            path: "/m/a.lrc".into(),
        };
        let s = serde_json::to_string(&there).expect("serialises");
        assert_eq!(s, r#"{"trouble":"exists","path":"/m/a.lrc"}"#);
        let gone = Trouble::NoSuchCandidate {
            candidate: CandidateId(9),
        };
        let s = serde_json::to_string(&gone).expect("serialises");
        assert_eq!(s, r#"{"trouble":"no-such-candidate","candidate":9}"#);
        let desk = Say::Desk(Box::new(Desk {
            taken: Some(Taken {
                candidate: CandidateId(1),
                status: crate::find::TakeStatus::Handed,
            }),
            ..Desk::empty(DeskKind::Suggestions)
        }));
        let s = serde_json::to_string(&desk).expect("serialises");
        assert!(
            s.starts_with(r#"{"say":"desk","kind":"suggestions""#),
            "{s}"
        );
        assert_eq!(serde_json::from_str::<Say>(&s).expect("parses"), desk);
        let state = State {
            desks: vec![DeskSummary {
                kind: DeskKind::Lyrics,
                generation: 3,
                busy: true,
                trouble: Some(off),
            }],
            ..State::default()
        };
        let s = serde_json::to_string(&state).expect("serialises");
        assert!(
            s.contains(r#""desks":[{"kind":"lyrics","generation":3,"busy":true,"trouble":{"trouble":"off","host":"musicbrainz.org"}}]"#),
            "{s}"
        );
    }

    /// FAILS IF: a door's ask for a suggested cover to be seen stops
    /// crossing the socket as the daemon reads it, or the picture it lands
    /// as stops crossing back on its candidate.
    #[test]
    fn a_suggested_cover_is_asked_and_lands_on_its_candidate() {
        assert_eq!(
            serde_json::from_str::<Ask>(r#"{"ask":"suggested-cover","candidate":7}"#)
                .expect("parses"),
            Ask::SuggestedCover {
                candidate: CandidateId(7)
            }
        );
        let kept = crate::find::SuggestedCover::Kept {
            path: "/run/user/1000/ronoctua-player-fetched/preview-7.jpg".into(),
        };
        let s = serde_json::to_string(&kept).expect("serialises");
        assert_eq!(
            s,
            r#"{"cover":"kept","path":"/run/user/1000/ronoctua-player-fetched/preview-7.jpg"}"#
        );
        let refused = crate::find::SuggestedCover::Refused(Trouble::Off {
            host: "coverartarchive.org".into(),
        });
        let s = serde_json::to_string(&refused).expect("serialises");
        assert_eq!(
            s,
            r#"{"cover":"refused","trouble":"off","host":"coverartarchive.org"}"#
        );
        assert_eq!(
            serde_json::from_str::<crate::find::SuggestedCover>(&s).expect("parses"),
            refused
        );
        // A CANDIDATE FROM BEFORE PREVIEWS reads as one with none.
        let c: crate::find::SuggestionCandidate = serde_json::from_str(
            r#"{"id":3,"suggestion":{"source":"musicbrainz","id":"x","fields":[],"cover":null,"score":null},"complete":true}"#,
        )
        .expect("parses");
        assert_eq!(c.seen, None);
    }

    /// FAILS IF: a star or a directory's refusal stops crossing the socket as
    /// the words the other side reads — the kind and id a favourite names,
    /// the directory and the reason it gave nothing.
    #[test]
    fn the_directories_are_asked_and_refused_as_doors_write_them() {
        let read = |s: &str| serde_json::from_str::<Ask>(s).expect(s);
        assert_eq!(
            read(r#"{"ask":"favourite","kind":"radio","id":"u","on":true}"#),
            Ask::Favourite {
                kind: ronoctua_broadcast::picks::Kind::Radio,
                id: "u".into(),
                on: true
            }
        );
        assert_eq!(
            read(r#"{"ask":"find","query":{"what":"tv","list":"countries"}}"#),
            Ask::Find {
                query: Query::Tv(crate::find::TvList::Countries)
            }
        );
        let t = Trouble::Directory(ronoctua_broadcast::Refusal::new(
            ronoctua_broadcast::Source::Radio,
            ronoctua_broadcast::Why::Unreachable,
        ));
        let s = serde_json::to_string(&t).expect("serialises");
        assert_eq!(
            s,
            r#"{"trouble":"directory","source":"radio","why":"unreachable"}"#
        );
        assert_eq!(serde_json::from_str::<Trouble>(&s).expect("parses"), t);
        for (t, want) in [
            (
                Trouble::NoSuchCategory {
                    category: "x".into(),
                },
                r#"{"trouble":"no-such-category","category":"x"}"#,
            ),
            (
                Trouble::NoSuchPick {
                    kind: ronoctua_broadcast::picks::Kind::Tv,
                    id: "A.uk".into(),
                },
                r#"{"trouble":"no-such-pick","kind":"tv","id":"A.uk"}"#,
            ),
            (
                Trouble::Picks {
                    why: ronoctua_broadcast::picks::Trouble::Unreadable,
                },
                r#"{"trouble":"picks","why":"unreadable"}"#,
            ),
        ] {
            assert_eq!(serde_json::to_string(&t).expect("serialises"), want);
            assert_eq!(serde_json::from_str::<Trouble>(want).expect(want), t);
        }
    }

    /// FAILS IF: what protocol 2 wrote stops being read by protocol 3 — a
    /// put with no `once`, an unplayable with no `said`, a lyric with no
    /// translation, a state with none of the new fields — or a field 3 added
    /// stops crossing whole.
    #[test]
    fn protocol_2_is_read_and_3_round_trips() {
        let put: Ask = serde_json::from_str(r#"{"ask":"put","what":["/m/a.flac"],"play":false}"#)
            .expect("an old put");
        assert!(matches!(put, Ask::Put { once: false, .. }), "{put:?}");
        let t: Trouble =
            serde_json::from_str(r#"{"trouble":"unplayable","why":"x"}"#).expect("an old refusal");
        assert_eq!(
            t,
            Trouble::Unplayable {
                why: "x".into(),
                said: None
            }
        );
        let line: LyricLine = serde_json::from_str(r#"{"at":1.5,"text":"la"}"#).expect("a line");
        assert_eq!(line.translated, None);
        let old: State = serde_json::from_str(
            r#"{"protocol":2,"playing":true,"lyrics":[{"at":null,"text":"a"}]}"#,
        )
        .expect("a state of protocol 2");
        assert_eq!(old.protocol, 2);
        assert_eq!(old.queue_file, crate::queue::QueueKept::Empty);
        assert_eq!(old.lyrics_translation, None);
        assert_eq!(old.catalog, CatalogSummary::default());

        let asks = [
            Ask::SaveQueueAgain { force: true },
            Ask::LyricsTranslation {
                on: true,
                to: Some("pt-BR".into()),
            },
            Ask::AddSource {
                url: SourceUrl("https://iptv.example/get.php?username=u&password=p".into()),
                adult: false,
            },
            Ask::RemoveSource {
                id: SourceId("p3".into()),
            },
            Ask::Ingest {
                source: SourceId("iptv".into()),
                category: Some("news".into()),
            },
            Ask::SetSource {
                id: SourceId("lives".into()),
                posters: Some(true),
                adult: None,
            },
            Ask::Posters {
                items: vec![ItemId(1), ItemId(9)],
            },
            Ask::Put {
                what: vec![Thing::Named {
                    location: "https://youtu.be/dQw4w9WgXcQ".into(),
                    title: "A song".into(),
                }],
                after: None,
                into: None,
                play: false,
                replace: false,
                once: true,
            },
        ];
        for ask in asks {
            let s = serde_json::to_string(&ask).expect("serialises");
            assert_eq!(serde_json::from_str::<Ask>(&s).expect(&s), ask);
        }
        assert_eq!(
            serde_json::from_str::<Ask>(r#"{"ask":"save-queue-again"}"#).expect("parses"),
            Ask::SaveQueueAgain { force: false }
        );
        // A NAMED THING is told apart from a stream and a set by its fields.
        let named: Thing =
            serde_json::from_str(r#"{"location":"/m/a.flac","title":"A"}"#).expect("named");
        assert!(matches!(named, Thing::Named { .. }), "{named:?}");

        for t in [
            Trouble::AlreadyQueued {
                entry: EntryId(4),
                name: Name::Titled("A".into()),
            },
            Trouble::ChangedOnDisk {
                path: "/q.rqueue".into(),
            },
            Trouble::NoOrigin,
            Trouble::NoSound { lane: LaneId(2) },
            Trouble::Unplayable {
                why: "loading failed".into(),
                said: Some("ERROR: [youtube] x: Sign in".into()),
            },
        ] {
            let s = serde_json::to_string(&t).expect("serialises");
            assert_eq!(serde_json::from_str::<Trouble>(&s).expect(&s), t);
        }
        assert_eq!(
            serde_json::to_string(&Trouble::NoOrigin).expect("serialises"),
            r#"{"trouble":"no-origin"}"#
        );

        let state = State {
            protocol: PROTOCOL,
            lyrics: vec![LyricLine {
                at: Some(1.0),
                text: "la".into(),
                translated: Some("lá".into()),
            }],
            lyrics_translation: Some(Translation {
                to: "pt-BR".into(),
                from: Some("en".into()),
                status: TranslationStatus::Translated { by: "x".into() },
                unsynced: Some("one\ntwo".into()),
            }),
            queue_file: crate::queue::QueueKept::Changed {
                path: "/q.rqueue".into(),
                name: "q".into(),
            },
            catalog: CatalogSummary {
                generation: 2,
                posters_generation: 1,
                sources: vec![SourceStatus {
                    id: SourceId("p1".into()),
                    kind: ronoctua_broadcast::sources::Kind::Playlist,
                    title: None,
                    shown: "iptv.example/…".into(),
                    adult: false,
                    posters: false,
                    plain: true,
                    items: 10,
                    busy: true,
                    trouble: Some(Trouble::Source {
                        why: ronoctua_broadcast::sources::Refused::Unreachable,
                    }),
                }],
                trouble: Some(Trouble::Sources {
                    why: SourcesTrouble::Unparseable,
                }),
            },
            ..State::default()
        };
        let s = serde_json::to_string(&state).expect("serialises");
        assert!(
            s.contains(r#""queue_file":{"kept":"changed","path":"/q.rqueue","name":"q"}"#),
            "{s}"
        );
        assert_eq!(serde_json::from_str::<State>(&s).expect("parses"), state);
    }

    /// FAILS IF: a source's address reaches a log or a door whole — its
    /// account rides in the query.
    #[test]
    fn a_source_address_is_never_printed_whole() {
        let url = SourceUrl("http://user:pw@iptv.example:8080/get.php?password=p".into());
        assert_eq!(url.masked(), "iptv.example:8080/…");
        // A PASSWORD WITH A DELIMITER IN IT, unescaped as pasted by hand.
        for (pasted, shown) in [
            (
                "http://user:pa/ss@iptv.example/get.php?x=1",
                "iptv.example/…",
            ),
            ("http://user:p#w@iptv.example/", "iptv.example/…"),
            ("http://user:p?w@iptv.example:81/x", "iptv.example:81/…"),
            ("http://u:p@ss/w@iptv.example/x", "iptv.example/…"),
            // AN `@` OF THE QUERY is no account: nothing after it is a host.
            ("http://h.example/get.php?mail=me@x.y&password=secret", "…"),
            ("http://h.example/get.php?username=me&password=se@cret", "…"),
            ("http://iptv.example/list.m3u", "iptv.example/…"),
            ("iptv.example", "iptv.example/…"),
        ] {
            let masked = SourceUrl(pasted.into()).masked();
            assert_eq!(masked, shown, "{pasted}");
            for secret in ["pa", "ss", "p#w", "p?w", "secret", "se@cret", "user"] {
                assert!(
                    !masked.contains(secret),
                    "{pasted} showed {secret}: {masked}"
                );
            }
        }
        let ask = Ask::AddSource { url, adult: true };
        let printed = format!("{ask:?}");
        assert!(
            !printed.contains("pw") && !printed.contains("password"),
            "{printed}"
        );
    }

    #[test]
    fn the_repeat_ring_comes_back() {
        assert_eq!(Repeat::Nothing.next().next().next(), Repeat::Nothing);
    }
}
