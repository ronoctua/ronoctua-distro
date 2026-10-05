//! The super index behind fast-access.
//!
//! # One index, one writer, many readers
//!
//! File names, file contents (text, office, EPUB — `ronoctua-inspect`'s
//! plain text), manual pages, the packages of the Nix index and the video
//! catalogue of video-metrics, in ONE tantivy index under
//! `$XDG_CACHE_HOME/ronoctua[-dev]/index/`. The `ronoctua-index` unit is the
//! one writer (feature `write`); the doors open it read-only ([`Index`]) with
//! no server between them — tantivy's segments are immutable and mmapped, and
//! a reader keeps the old ones mapped while the writer unlinks them.
//!
//! # A reader has no clock
//!
//! tantivy's own reload poller is a timer; it is not used. A door calls
//! [`Index::reload`] when its `Notice::on_file` on `meta.json` fires — the
//! writer's commit is the event.
//!
//! # Held while shown
//!
//! A door opens the reader when its view opens and drops it when the view
//! closes (or its window is parked): the footer lives for months, and nothing
//! of the index stays resident behind a closed view.
//!
//! # The schema
//!
//! `id` (a path, `pkg:<attr>`, `man:<name>.<section>`, `yt:<id>`), `kind`,
//! `name` (folded by [`ronoctua_apps::fold`] and split at punctuation,
//! camelCase and digit edges), `path_terms`, `body` (not stored), `mime`,
//! `mtime`, `size`. The last word of a query is a prefix (at most 64
//! expansions), the others terms plus a Levenshtein-1 fuzzy match from five
//! letters; name weighs 4, path and body 1; recency and frecency adjust.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(feature = "write")]
pub mod crawl;
#[cfg(all(test, feature = "write"))]
mod end_to_end;
#[cfg(feature = "engine")]
mod engine;
#[cfg(feature = "man")]
pub mod man;
#[cfg(feature = "write")]
pub mod packages;
#[cfg(feature = "write")]
pub mod serve;
pub mod settings;
pub mod status;
#[cfg(feature = "write")]
pub mod videos;
pub mod words;
#[cfg(feature = "write")]
pub mod write;

#[cfg(feature = "engine")]
pub use engine::{EXPANSIONS, FUZZY_FROM, HALF_LIFE_DAYS, recency};
pub use settings::Settings;
pub use status::{Phase, Status};

/// The index's folder name, under the cache root.
pub const DIR: &str = "index";

/// The file every commit rewrites last, by rename — what a reader watches.
pub const META: &str = "meta.json";

/// `$XDG_CACHE_HOME/ronoctua[-dev]/index`, or `None` with no home.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    ronoctua_xdg::cache_dir().map(|d| d.join(DIR))
}

/// What a hit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A file, found by its name.
    File,
    /// A folder, found by its name.
    Folder,
    /// A file, found by what is written in it.
    Content,
    /// A manual page.
    Man,
    /// A package of the Nix index, to install.
    Package,
    /// An installed application (the catalogue, in process — never indexed).
    App,
    /// A command of the registry (in process — never indexed).
    Command,
    /// A video of the video-metrics catalogue.
    Video,
}

impl Kind {
    /// Every kind, in the order fast-access's sections come.
    pub const ALL: [Self; 8] = [
        Self::App,
        Self::Command,
        Self::File,
        Self::Folder,
        Self::Content,
        Self::Video,
        Self::Man,
        Self::Package,
    ];

    /// Its kebab-case name, the suffix of its key (`fast-access-kind-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Folder => "folder",
            Self::Content => "content",
            Self::Man => "man",
            Self::Package => "package",
            Self::App => "app",
            Self::Command => "command",
            Self::Video => "video",
        }
    }

    /// Its value in the index's `kind` fast field. Stable: changing one is a
    /// new index format.
    #[must_use]
    pub const fn code(self) -> u64 {
        match self {
            Self::File => 1,
            Self::Folder => 2,
            Self::Content => 3,
            Self::Man => 4,
            Self::Package => 5,
            Self::App => 6,
            Self::Command => 7,
            Self::Video => 8,
        }
    }

    /// The kind of a code, or `None` for one this build does not know.
    #[must_use]
    pub fn of_code(code: u64) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.code() == code)
    }
}

/// A question to the index.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Query {
    /// The words, as typed; the last one is a prefix.
    pub words: Vec<String>,
    /// Only these kinds; empty is every kind the index holds.
    pub kinds: Vec<Kind>,
    /// Only these MIME types, by prefix (`audio/`, `video/`, `image/`); empty
    /// is every type. For `/a`, `/v`, `/i`, `/o`.
    pub mime_prefixes: Vec<String>,
    /// Whether contents are searched (`/c`, and the default scope), or names
    /// only (`/f`).
    pub contents: bool,
    /// The most hits answered OF EACH KIND: fast-access shows a section per
    /// kind, and one crowded kind must not empty the others.
    pub most: usize,
}

impl Query {
    /// A query for every kind, over names and contents, from what was typed.
    #[must_use]
    pub fn of_text(text: &str, most: usize) -> Self {
        Self {
            words: text.split_whitespace().map(str::to_owned).collect(),
            contents: true,
            most,
            ..Self::default()
        }
    }
}

/// One answer of the index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    /// Its id: a path, `pkg:<attr>`, `man:<name>.<section>` or `yt:<id>` —
    /// what fast-access's cursor keeps, so arriving hits never move it.
    pub id: String,
    /// What it is.
    pub kind: Kind,
    /// What a row shows as its name (the file's name, the page's, the
    /// package's attribute, the video's title).
    pub name: String,
    /// The dim detail beside the name, when the kind has one: a manual's
    /// one-line description, a package's, a video's channel. Data, shown as
    /// it came.
    pub note: Option<String>,
    /// How well it matched, after recency and frecency; higher is better.
    pub score: f32,
    /// The file, for a file, folder or content hit.
    pub path: Option<PathBuf>,
    /// Its MIME type, when the index knows it.
    pub mime: Option<String>,
    /// Its modification time, in seconds since the epoch.
    pub mtime_s: Option<i64>,
    /// Its size, in bytes.
    pub size_bytes: Option<u64>,
}

/// Why the index answered nothing, as a closed set (`fast-access-index-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// This build has no engine (feature `engine` off).
    NoEngine,
    /// There is no index yet: the unit has not built one.
    NotBuilt,
    /// The index is there and cannot be read.
    Unreadable,
    /// The index is of another format; the unit rebuilds it.
    OtherFormat,
    /// Another writer holds the index (writer side).
    Busy,
    /// The index reached its ceiling (`searchIndexMiB`).
    Full,
    /// A query the engine could not run.
    Failed,
}

impl Why {
    /// Every reason, for the checks that want a sentence for each.
    pub const ALL: [Self; 7] = [
        Self::NoEngine,
        Self::NotBuilt,
        Self::Unreadable,
        Self::OtherFormat,
        Self::Busy,
        Self::Full,
        Self::Failed,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoEngine => "no-engine",
            Self::NotBuilt => "not-built",
            Self::Unreadable => "unreadable",
            Self::OtherFormat => "other-format",
            Self::Busy => "busy",
            Self::Full => "full",
            Self::Failed => "failed",
        }
    }
}

/// A refusal of the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

/// The app's history, as the index weighs it: an id and how much it was used
/// lately (frecency), from 0 to 1.
///
/// The app keeps the history (`/r`); the index only multiplies: a hit it
/// weighs `w` scores `1 + w` times, and its ids are asked for in a second,
/// narrow query so a frecent hit outside the first window is answered too.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    weights: HashMap<String, f32>,
}

impl History {
    /// An empty history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// With `id` weighed `weight`, clamped to 0–1 (a non-number is 0).
    #[must_use]
    pub fn with(mut self, id: impl Into<String>, weight: f32) -> Self {
        let weight = if weight.is_nan() {
            0.0
        } else {
            weight.clamp(0.0, 1.0)
        };
        self.weights.insert(id.into(), weight);
        self
    }

    /// How much `id` weighs, when the history knows it.
    #[must_use]
    pub fn weight(&self, id: &str) -> Option<f32> {
        self.weights.get(id).copied()
    }

    /// Whether it knows nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }

    /// Every id it knows.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.weights.keys().map(String::as_str)
    }
}

/// A read-only handle on the index.
///
/// Opened when a view opens and dropped when it closes: a reader maps the
/// segments of the commit it sees, and the footer lives for months.
#[derive(Debug)]
pub struct Index {
    dir: PathBuf,
    #[cfg(feature = "engine")]
    reader: engine::Reader,
}

impl Index {
    /// Opens the index in `dir` read-only, with no reload clock.
    ///
    /// # Errors
    ///
    /// [`Why::NoEngine`] in a build without `engine`; [`Why::NotBuilt`] when
    /// the unit has not written one, [`Why::Unreadable`], and
    /// [`Why::OtherFormat`] for an index this build did not write.
    pub fn open_ro(dir: &Path) -> Result<Self, Refusal> {
        #[cfg(feature = "engine")]
        {
            Ok(Self {
                dir: dir.to_path_buf(),
                reader: engine::Reader::open(dir)?,
            })
        }
        #[cfg(not(feature = "engine"))]
        {
            let _ = dir;
            Err(Refusal { why: Why::NoEngine })
        }
    }

    /// Where it is.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file whose change means a commit: the door's `Notice::on_file`
    /// watches it (by its directory and name, since the writer replaces it by
    /// rename) and calls [`Index::reload`].
    #[must_use]
    pub fn meta_path(&self) -> PathBuf {
        self.dir.join(META)
    }

    /// Sees the writer's last commit. Called when `meta.json` changes.
    ///
    /// # Errors
    ///
    /// [`Why::Unreadable`].
    pub fn reload(&mut self) -> Result<(), Refusal> {
        #[cfg(feature = "engine")]
        {
            self.reader.reload()
        }
        #[cfg(not(feature = "engine"))]
        {
            Err(Refusal { why: Why::NoEngine })
        }
    }

    /// How many documents the commit this handle sees holds.
    #[must_use]
    pub fn documents(&self) -> u64 {
        #[cfg(feature = "engine")]
        {
            self.reader.searcher().num_docs()
        }
        #[cfg(not(feature = "engine"))]
        {
            0
        }
    }

    /// The best hits for `query`, best first. Blocks for the query's length
    /// (the gate: p99 ≤ 8 ms on names, ≤ 25 ms on contents, over a million
    /// documents) — fast-access asks on its "latest wins" thread.
    ///
    /// A `Result` and not a bare list, so a failing index is said on screen
    /// rather than drawn as "nothing found".
    ///
    /// # Errors
    ///
    /// [`Why::Failed`].
    pub fn query(&self, query: &Query) -> Result<Vec<Hit>, Refusal> {
        self.query_with(query, &History::default())
    }

    /// [`Index::query`], with the app's history weighing the hits it knows.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`].
    pub fn query_with(&self, query: &Query, history: &History) -> Result<Vec<Hit>, Refusal> {
        #[cfg(feature = "engine")]
        {
            self.reader.query(query, history, now_s())
        }
        #[cfg(not(feature = "engine"))]
        {
            let _ = (query, history);
            Err(Refusal { why: Why::NoEngine })
        }
    }
}

/// Calls `on_commit` each time the writer commits — `meta.json` renamed into
/// place in `dir` — on a watch thread of its own, with no clock, until it
/// answers `false` or the handle is dropped. The door answers by calling
/// [`Index::reload`] on its own loop (or opening the index, the first time:
/// the watch can be armed before there is one).
///
/// The folder is made when it is not there yet: the watch is on the folder
/// (the writer renames `meta.json` into it), and a watch asked of a folder
/// that does not exist is no watch at all — a door opened before the unit's
/// first run would never hear its first commit. The folder is a cache's; an
/// empty one is what the unit would make.
///
/// # Errors
///
/// [`Why::Unreadable`]: the folder cannot be made or watched (inotify is out
/// of instances).
#[cfg(feature = "engine")]
pub fn on_commit<F>(dir: &Path, on_commit: F) -> Result<ronoctua_files::notice::Notice, Refusal>
where
    F: Fn() -> bool + Send + 'static,
{
    std::fs::create_dir_all(dir).map_err(|e| {
        tracing::warn!(dir = %dir.display(), error = %e, "the index's folder cannot be made");
        Refusal {
            why: Why::Unreadable,
        }
    })?;
    ronoctua_files::notice::Notice::on_file(
        dir,
        META,
        ronoctua_files::notice::Pace::AtOnce,
        on_commit,
    )
    .map_err(|e| {
        tracing::warn!(dir = %dir.display(), error = %e, "the index's commits cannot be watched");
        Refusal {
            why: Why::Unreadable,
        }
    })
}

/// Now, in seconds since the epoch (0 before it).
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_have_stable_codes() {
        for kind in Kind::ALL {
            assert_eq!(Kind::of_code(kind.code()), Some(kind));
        }
        assert_eq!(Kind::of_code(0), None);
        let codes: std::collections::BTreeSet<_> = Kind::ALL.iter().map(|k| k.code()).collect();
        assert_eq!(codes.len(), Kind::ALL.len());
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len());
        assert_eq!(
            Query::of_text("  report  2026 ", 20).words,
            ["report", "2026"]
        );
    }

    #[test]
    fn a_missing_index_is_refused_in_words() {
        let refused = Index::open_ro(Path::new("/nonexistent/ronoctua-index")).unwrap_err();
        let expected = if cfg!(feature = "engine") {
            Why::NotBuilt
        } else {
            Why::NoEngine
        };
        assert_eq!(refused.why, expected);
    }

    #[test]
    fn the_history_clamps_its_weights() {
        let h = History::new()
            .with("a", 3.0)
            .with("b", -1.0)
            .with("c", f32::NAN);
        assert_eq!(h.weight("a"), Some(1.0));
        assert_eq!(h.weight("b"), Some(0.0));
        assert_eq!(h.weight("c"), Some(0.0));
        assert_eq!(h.weight("d"), None);
        assert!(History::new().is_empty());
    }
}
