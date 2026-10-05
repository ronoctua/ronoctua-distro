//! The video catalogue of video-metrics (feature `write`).
//!
//! Read through `ronoctua_video_metrics::read` alone — the catalogue's SQL
//! never leaves that crate (`one-sqlite-boundary`). The crawler asks
//! [`Videos::changed_since`] the generation it last took (kept in the commit's
//! state) and gets the videos changed since and the generation now; a hidden
//! video leaves the index. A deleted one is never answered again (the
//! catalogue keeps a tombstone), so the unit starts with a whole resync —
//! every video deleted, generation zero asked — and, while it runs, compares
//! after each change how many videos the index holds with how many the
//! catalogue shows ([`Videos::shown`], one `COUNT`): more in the index is a
//! video deleted, and the videos are taken again whole.
//!
//! [`Videos`] is a trait so the crawler is tested against a fake; the real
//! one is [`Catalogue`].

use std::path::{Path, PathBuf};

use ronoctua_video_metrics::read::{Catalog, Filter, Video};

use crate::Kind;
use crate::write::Document;

/// One video, as the index takes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// YouTube's id.
    pub id: String,
    /// Its title — data.
    pub title: String,
    /// Its channel's title — data.
    pub channel: String,
    /// Its description, tags and the person's second fields, as one text.
    pub text: String,
    /// When it was published, seconds since the epoch (0: unknown).
    pub published_at_s: i64,
    /// Whether the person hid it (it leaves the index).
    pub hidden: bool,
}

impl Found {
    /// A video of the catalogue.
    #[must_use]
    pub fn of(video: &Video) -> Self {
        let mut text = video.description.clone();
        for more in [&video.second_description]
            .into_iter()
            .chain(video.tags.iter())
            .chain(video.second_tags.iter())
            .chain(video.categories.iter())
        {
            if !more.is_empty() {
                text.push('\n');
                text.push_str(more);
            }
        }
        Self {
            id: video.row.id.0.clone(),
            title: video.row.title.clone(),
            channel: video.row.channel.clone(),
            text,
            published_at_s: video.row.published_at_s.unwrap_or(0),
            hidden: video.row.hidden,
        }
    }

    /// Its id in the index.
    #[must_use]
    pub fn index_id(&self) -> String {
        Self::index_id_of(&self.id)
    }

    /// The index's id of YouTube's id `id`.
    #[must_use]
    pub fn index_id_of(id: &str) -> String {
        format!("yt:{id}")
    }

    /// As a document, the text cut at `body_bytes`.
    #[must_use]
    pub fn document(&self, body_bytes: usize) -> Document {
        let mut body = self.text.clone();
        if body.len() > body_bytes {
            let mut at = body_bytes;
            while at > 0 && !body.is_char_boundary(at) {
                at -= 1;
            }
            body.truncate(at);
        }
        Document {
            id: self.index_id(),
            kind: Kind::Video,
            name: self.title.clone(),
            also: if self.channel.is_empty() {
                Vec::new()
            } else {
                vec![self.channel.clone()]
            },
            path_terms: Vec::new(),
            body: (!body.is_empty()).then_some(body),
            note: (!self.channel.is_empty()).then(|| self.channel.clone()),
            mime: None,
            mtime_s: self.published_at_s,
            size_bytes: 0,
        }
    }
}

/// Where videos come from (`Send`: the unit may run on a thread of its own).
pub trait Videos: Send {
    /// The videos changed since `generation`, and the generation now; `Err`
    /// with a word for the log when the source cannot answer.
    ///
    /// # Errors
    ///
    /// The source is absent or refused.
    fn changed_since(&mut self, generation: u64) -> Result<(Vec<Found>, u64), String>;

    /// How many videos the source shows — every one that is not hidden,
    /// which is what the index holds of it.
    ///
    /// # Errors
    ///
    /// As [`Videos::changed_since`].
    fn shown(&mut self) -> Result<u64, String>;

    /// The file whose change means new videos (watched by its folder), when
    /// there is one.
    fn watched(&self) -> Option<PathBuf>;

    /// YouTube's ids of the videos taken OUT since `generation` (deleted,
    /// unavailable) — none, for a source that keeps no record of them.
    ///
    /// # Errors
    ///
    /// As [`Videos::changed_since`].
    fn gone_since(&mut self, generation: u64) -> Result<Vec<String>, String> {
        let _ = generation;
        Ok(Vec::new())
    }
}

/// The catalogue on disk, opened read-only at each question (a door's rule:
/// nothing held open between changes).
#[derive(Debug, Clone)]
pub struct Catalogue {
    path: PathBuf,
}

impl Catalogue {
    /// The catalogue at `path` (`ronoctua_video_metrics::default_path`).
    #[must_use]
    pub fn at(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }
}

impl Videos for Catalogue {
    fn changed_since(&mut self, generation: u64) -> Result<(Vec<Found>, u64), String> {
        let catalog = Catalog::open_ro(&self.path).map_err(|r| format!("{:?}", r.why))?;
        let (videos, now) = catalog
            .all_since(generation)
            .map_err(|r| format!("{:?}", r.why))?;
        Ok((videos.iter().map(Found::of).collect(), now))
    }

    fn shown(&mut self) -> Result<u64, String> {
        let catalog = Catalog::open_ro(&self.path).map_err(|r| format!("{:?}", r.why))?;
        // The default filter: every channel, Shorts and long videos, not the
        // hidden ones — what `changed_since` leaves in the index.
        catalog
            .count(&Filter::default())
            .map_err(|r| format!("{:?}", r.why))
    }

    fn watched(&self) -> Option<PathBuf> {
        Some(self.path.clone())
    }

    fn gone_since(&mut self, generation: u64) -> Result<Vec<String>, String> {
        let catalog = Catalog::open_ro(&self.path).map_err(|r| format!("{:?}", r.why))?;
        catalog
            .gone_since(generation)
            .map(|ids| ids.into_iter().map(|id| id.0).collect())
            .map_err(|r| format!("{:?}", r.why))
    }
}
