//! The person's edits: what the video-metrics door writes.
//!
//! A [`Person`] holds a read-write connection. Every edit is one
//! transaction; a rule, weight or setting that changes the static points
//! recomputes them in the same transaction ([`Person::rescore`] runs it on
//! its own), and a door calls these on a thread, never on the loop.
//!
//! Without the `db` feature a [`Person`] cannot exist: [`Person::open`]
//! refuses with [`crate::Why::NoDatabase`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::read::{Decision, Staged};
use crate::{ChannelId, Keep, Rating, Refusal, Rule, RuleId, Settings, VideoId, Weights};

/// A channel to add: resolved from what the person pasted (a URL or an
/// `@handle`) by `ronoctua_ytdlp::channel_of`, on a thread, before this.
///
/// Adding a channel is the person's consent to scan it: the caller lets
/// [`crate::HOSTS`] (`ronoctua_consent::let_hosts`) with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewChannel {
    /// Its id.
    pub id: ChannelId,
    /// Its handle, when known.
    pub handle: Option<String>,
    /// Its title.
    pub title: String,
    /// Its address.
    pub url: String,
    /// Its picture's address, when known.
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// What staging an imported list did ([`Person::stage`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Staging {
    /// Channels now waiting for a decision (new ones, and the ones the list
    /// named again).
    pub staged: u32,
    /// Channels already in the catalogue, never staged: their ids and the
    /// catalogue's titles.
    pub already: Vec<(ChannelId, String)>,
}

/// What registering an imported list did ([`Person::register_staged`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Registered {
    /// Channels that joined the catalogue.
    pub kept: u32,
    /// Channels dropped.
    pub discarded: u32,
    /// Channels still to decide.
    pub left: u32,
}

/// Why a channel the person typed was not added ([`Person::add_from`]).
#[cfg(feature = "net")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAdded {
    /// yt-dlp could not say which channel it is: its trouble, what its last
    /// line meant, and the line itself — data, for a log.
    Unresolved {
        /// Which of the closed set.
        trouble: ronoctua_ytdlp::Trouble,
        /// What the line meant (a bot check, a 429, a channel that is gone).
        said: ronoctua_ytdlp::youtube::Said,
        /// yt-dlp's own words.
        line: Option<String>,
    },
    /// The catalogue refused it.
    Refused(Refusal),
    /// The channel went in, and the consent to ask its hosts could not be
    /// kept: a scan will not ask YouTube until it is.
    NotLet(ronoctua_consent::Unkept),
}

/// The catalogue, as the person edits it.
#[derive(Debug)]
pub struct Person {
    path: PathBuf,
    #[cfg(feature = "db")]
    conn: rusqlite::Connection,
    #[cfg(not(feature = "db"))]
    never: std::convert::Infallible,
}

impl Person {
    /// Opens (or creates, with the schema) the catalogue at `path` for
    /// writing.
    ///
    /// # Errors
    ///
    /// [`crate::Why::NoDatabase`] without `db`; [`crate::Why::Unreadable`],
    /// [`crate::Why::OtherSchema`], [`crate::Why::Busy`].
    pub fn open(path: &Path) -> Result<Self, Refusal> {
        #[cfg(feature = "db")]
        return Ok(Self {
            path: path.to_owned(),
            conn: crate::db::open_rw(path)?,
        });
        #[cfg(not(feature = "db"))]
        return {
            let _ = path;
            Err(Refusal::no_database())
        };
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Rates a video (`None` clears the rating) and rescores it.
    ///
    /// # Errors
    ///
    /// [`crate::Why::NotFound`], [`crate::Why::Busy`],
    /// [`crate::Why::Failed`].
    pub fn set_rating(&self, id: &VideoId, rating: Option<Rating>) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_rating(&self.conn, id, rating, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never((id, rating));
    }

    /// Replaces a video's second description (the person's own text) and
    /// rescores it (a term rule may read it).
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_second_description(&self, id: &VideoId, text: &str) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_second_description(&self.conn, id, text, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never((id, text));
    }

    /// Replaces a video's second tags (trimmed; empty ones and repeats
    /// dropped) and rescores it.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_second_tags(&self, id: &VideoId, tags: &[String]) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_second_tags(&self.conn, id, tags, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never((id, tags));
    }

    /// Hides a video (`true`) or shows it again (`false`).
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_hidden(&self, id: &VideoId, hidden: bool) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_hidden(&self.conn, id, hidden, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never((id, hidden));
    }

    /// Sets a video aside for later (`true`) or takes it off that list
    /// (`false`); set aside again, it keeps the moment it first was. While it
    /// is set aside, its time never runs out.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_aside(&self, id: &VideoId, aside: bool) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_aside(&self.conn, id, aside, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never((id, aside));
    }

    /// Marks a video to keep for ever (`true`), whatever its channel keeps,
    /// or takes the mark off (`false`).
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_keep(&self, id: &VideoId, keep: bool) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_keep(&self.conn, id, keep);
        #[cfg(not(feature = "db"))]
        return self.never((id, keep));
    }

    /// Sets how long a channel's videos stay (`None`: as everywhere); the
    /// next scan takes out what outlived it.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Invalid`] for no days or more than
    /// [`crate::KEEP_DAYS_MOST`], and as [`Person::set_rating`].
    pub fn set_channel_keep(&self, id: &ChannelId, keep: Option<Keep>) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_channel_keep(&self.conn, id, keep);
        #[cfg(not(feature = "db"))]
        return self.never((id, keep));
    }

    /// Stages the channels of an imported list ([`crate::takeout::read`]):
    /// each waits for a decision, never scanned until it is registered; a
    /// channel already in the catalogue is answered as such and left alone.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Invalid`] for an id that is not a channel's (the whole
    /// list is refused), and as [`Person::set_rating`].
    #[cfg(feature = "db")]
    pub fn stage(&self, list: &[crate::takeout::Subscription]) -> Result<Staging, Refusal> {
        crate::edit::stage(&self.conn, list, crate::db::now_s())
    }

    /// Decides what registering does with one staged channel.
    ///
    /// # Errors
    ///
    /// [`crate::Why::NotFound`] for a channel that is not staged, and as
    /// [`Person::set_rating`].
    pub fn decide(&self, id: &ChannelId, decision: Decision) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::decide(&self.conn, id, decision);
        #[cfg(not(feature = "db"))]
        return self.never((id, decision));
    }

    /// Gives every staged channel the same decision; answers how many.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn decide_all(&self, decision: Decision) -> Result<u32, Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::decide_all(&self.conn, decision);
        #[cfg(not(feature = "db"))]
        return self.never(decision);
    }

    /// Registers what was decided: the kept channels join the catalogue, the
    /// discarded ones go, the undecided stay. REGISTERING IS THE CONSENT TO
    /// SCAN, as adding is: a caller that registered any lets
    /// [`crate::HOSTS`] (`ronoctua_consent::let_hosts`).
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn register_staged(&self) -> Result<Registered, Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::register_staged(&self.conn, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// Every channel of an imported list not registered yet, by title.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn staged(&self) -> Result<Vec<Staged>, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::staged(&self.conn);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// Adds a channel (its first scan follows the safeguard in
    /// [`Settings`]); one already there keeps its score and its reference.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Invalid`] for an id that is not a channel's, and as
    /// [`Person::set_rating`].
    pub fn add_channel(&self, channel: &NewChannel) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::add_channel(&self.conn, channel, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never(channel);
    }

    /// A TEST'S VIDEO: `id`, titled `title`, of `channel` — added before
    /// ([`Person::add_channel`]) — catalogued as a scan that caught it would
    /// leave it: published now, its details pending, its points the
    /// channel's rescore's. A scan is the one writer of videos, and no test
    /// outside this crate may drive one (`video-metrics-readers-are-read-only`);
    /// this is how a door's test reads a mark back off a scratch catalogue
    /// (`test-support`, never a product's).
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`]; [`crate::Why::Failed`] for a channel that
    /// was not added.
    #[cfg(feature = "test-support")]
    pub fn plant(&self, id: &VideoId, channel: &ChannelId, title: &str) -> Result<(), Refusal> {
        use crate::{db, edit};
        let now = db::now_s();
        let url = format!("https://www.youtube.com/watch?v={}", id.0);
        let tx = db::immediate(&self.conn)?;
        let generation = db::bump(&tx)?;
        let video = edit::NewVideo {
            id,
            channel,
            url: &url,
            title,
            description: "",
            published_at_s: Some(now),
            duration_s: None,
            short_evidence: None,
            live_status: None,
            thumb_url: None,
            views: None,
        };
        edit::insert_video(&tx, &video, false, generation, now)?;
        edit::rescore(&tx, edit::Scope::Channel(channel), generation)?;
        tx.commit().map_err(db::sql)
    }

    /// Adds the channel `input` names — an address, an `@handle` or a
    /// channel id — asking yt-dlp who it is, and lets the hosts a scan asks
    /// ([`crate::HOSTS`]): ADDING A CHANNEL IS THE CONSENT TO SCAN IT. Blocks
    /// for as long as yt-dlp takes: call it on a thread.
    ///
    /// # Errors
    ///
    /// [`NotAdded::Unresolved`] with yt-dlp's trouble, [`NotAdded::Refused`]
    /// with the catalogue's, and [`NotAdded::NotLet`] when the channel is in
    /// and the consent could not be written.
    #[cfg(feature = "net")]
    pub fn add_from(&self, input: &str) -> Result<NewChannel, NotAdded> {
        let found = ronoctua_ytdlp::channel_of(input).map_err(|failed| NotAdded::Unresolved {
            trouble: failed.trouble,
            said: failed.said,
            line: failed.line,
        })?;
        let channel = NewChannel {
            id: ChannelId(found.id),
            handle: found.handle,
            title: found.title,
            url: found.url,
            avatar_url: found.avatar_url,
        };
        self.add_channel(&channel).map_err(NotAdded::Refused)?;
        ronoctua_consent::let_hosts(&crate::HOSTS, true).map_err(NotAdded::NotLet)?;
        Ok(channel)
    }

    /// Sets a channel's score, 0–10000 (clamped).
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_channel_score(&self, id: &ChannelId, score: i32) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_channel_score(&self.conn, id, score);
        #[cfg(not(feature = "db"))]
        return self.never((id, score));
    }

    /// Sets whether a channel's Shorts are blocked (`None`: as the global
    /// switch); its Shorts are hidden or brought back to match.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_channel_block_shorts(
        &self,
        id: &ChannelId,
        block: Option<bool>,
    ) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_channel_block_shorts(&self.conn, id, block, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never((id, block));
    }

    /// Removes a channel and its videos; their ids stay as tombstones.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn remove_channel(&self, id: &ChannelId) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::remove_channel(&self.conn, id, crate::db::now_s()).map(|thumbs| {
            // The pictures go once the catalogue no longer names them.
            for thumb in thumbs {
                crate::thumbs::forget(Some(&thumb));
            }
        });
        #[cfg(not(feature = "db"))]
        return self.never(id);
    }

    /// The rules in force for `channel` (its versions over the global ones,
    /// its disabled ones included), or the global ones for `None`.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn rules(&self, channel: Option<&ChannelId>) -> Result<Vec<Rule>, Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::rules_for(&self.conn, channel);
        #[cfg(not(feature = "db"))]
        return self.never(channel);
    }

    /// Keeps a rule (a new one when its id is `None`) and rescores. A new
    /// single rule (see [`crate::RuleKind::is_single`]) replaces the one its
    /// scope has.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Invalid`] for a rule the score cannot use, and as
    /// [`Person::set_rating`].
    pub fn put_rule(&self, rule: &Rule) -> Result<RuleId, Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::put_rule(&self.conn, rule);
        #[cfg(not(feature = "db"))]
        return self.never(rule);
    }

    /// Removes a rule and rescores.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn remove_rule(&self, id: RuleId) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::remove_rule(&self.conn, id);
        #[cfg(not(feature = "db"))]
        return self.never(id);
    }

    /// The rating weights.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn weights(&self) -> Result<Weights, Refusal> {
        #[cfg(feature = "db")]
        return crate::db::weights(&self.conn);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// Sets the rating weights and rescores.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn set_weights(&self, weights: &Weights) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_weights(&self.conn, weights);
        #[cfg(not(feature = "db"))]
        return self.never(weights);
    }

    /// The catalogue's switches.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn settings(&self) -> Result<Settings, Refusal> {
        #[cfg(feature = "db")]
        return crate::db::settings(&self.conn);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// Sets the switches (blocking Shorts hides the ones already in;
    /// unblocking brings back what blocking hid; a new keep takes out at the
    /// next scan what outlived it).
    ///
    /// # Errors
    ///
    /// [`crate::Why::Invalid`] for a keep of no days or more than
    /// [`crate::KEEP_DAYS_MOST`], and as [`Person::set_rating`].
    pub fn set_settings(&self, settings: &Settings) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_settings(&self.conn, settings, crate::db::now_s());
        #[cfg(not(feature = "db"))]
        return self.never(settings);
    }

    /// Recomputes every video's static points in one transaction; answers how
    /// many rows changed.
    ///
    /// # Errors
    ///
    /// As [`Person::set_rating`].
    pub fn rescore(&self) -> Result<u64, Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::rescore_all(&self.conn);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// What a method of a catalogue that cannot exist answers: a build
    /// without `db` never makes one ([`Person::open`] refuses).
    #[cfg(not(feature = "db"))]
    fn never<T, A>(&self, _arguments: A) -> T {
        match self.never {}
    }
}
