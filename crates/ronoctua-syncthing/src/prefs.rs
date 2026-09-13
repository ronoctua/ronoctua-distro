//! The one thing about Syncthing that is ours and not its.
//!
//! Syncthing persists every folder, every device and all hundred and thirty-one
//! options itself. A second copy here would be a second truth, and the first
//! disagreement between them would be a bug nobody could see. So this file
//! holds exactly one decision, and it is one Syncthing has no opinion about:
//! whether the modeline draws a mark, and when.

use serde::{Deserialize, Serialize};

/// Where the person's word lives.
pub const FILE: &str = "syncthing.json";

/// When the modeline says something about Syncthing.
///
/// Three, and the owner chose both the three and which is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mark {
    /// Never. The modeline says nothing about this, ever.
    Never,
    /// Only when nothing is travelling — paused, or the service is down.
    ///
    /// The default, and it is the one that keeps the modeline silent at rest:
    /// a machine whose sync is working draws nothing at all, which is this
    /// system's rule for that line.
    #[default]
    WhenOff,
    /// Only when it IS syncing.
    ///
    /// The deliberate exception, asked for by the owner. It puts a mark on a
    /// modeline that is otherwise silent while everything is well — which is
    /// a trade about reassurance, and theirs to make.
    WhenOn,
}

impl Mark {
    /// The id, as written in the file and in the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::WhenOff => "when-off",
            Self::WhenOn => "when-on",
        }
    }

    /// The three, in the order the ring turns.
    pub fn all() -> impl Iterator<Item = Self> {
        [Self::Never, Self::WhenOff, Self::WhenOn].into_iter()
    }

    /// The one this id names, if any.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::all().find(|m| m.id() == id)
    }

    /// Whether the mark is drawn, given whether anything is travelling.
    #[must_use]
    pub const fn shows(self, syncing: bool) -> bool {
        match self {
            Self::Never => false,
            Self::WhenOff => !syncing,
            Self::WhenOn => syncing,
        }
    }
}

/// What this interface keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// When the modeline draws a mark.
    pub mark: Mark,
}

/// Where the file lives, for whoever wants to WATCH it.
///
/// # Why the shell asks this and not `config_dir()`
///
/// Because a function that reads or writes does not get to decide where —
/// `persistence-takes-its-root-as-a-parameter` is the check that says so, and
/// the reason is that `cargo test` otherwise writes into the configuration of
/// whoever ran it.
#[must_use]
pub fn home() -> Option<std::path::PathBuf> {
    ronoctua_store::config_dir()
}

/// The store the person's word lives in.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(home(), FILE)
}

/// Writes the whole record back, and REFUSES over a file that will not parse.
///
/// # Why this is a function here and not a `store().write()` at the call site
///
/// Because getting it wrong is silent and permanent, and it has already
/// happened once in this repository. `Store::read` falls back to the default,
/// which is the right answer for a reader and destruction for a writer: read
/// the fallback, lay one keystroke over it, write, and a file that merely
/// would not parse is now a file that is gone. `Store::parsed` says "there is
/// a file and it does not parse" and nothing else, which is exactly the case a
/// writer must stop on.
///
/// The sound view wrote its preferences straight from a `read()`, and one
/// press of a key over an unparseable `audio.json` erased it. Putting the rule
/// in the crate means the two doors cannot disagree about it and a third door
/// cannot get it wrong.
///
/// # Errors
///
/// There is a file and it will not parse — in which case NOTHING is written,
/// and the caller is expected to say so on screen. Or the write itself failed.
pub fn keep(store: &ronoctua_store::Store<Preferences>, prefs: Preferences) -> anyhow::Result<()> {
    if store.path().is_some() && store.parsed().is_none() {
        anyhow::bail!("{FILE} will not parse, and writing would replace it");
    }
    store.write(&prefs)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_keeps_the_modeline_silent_while_all_is_well() {
        // The owner's choice, and the house rule for that line: at rest it
        // says nothing.
        assert_eq!(Preferences::default().mark, Mark::WhenOff);
        assert!(!Mark::WhenOff.shows(true), "a working sync draws nothing");
        assert!(Mark::WhenOff.shows(false), "a stopped one does");
    }

    #[test]
    fn the_ring_has_three_and_every_one_answers_to_its_id() {
        let all: Vec<Mark> = Mark::all().collect();
        assert_eq!(all.len(), 3);
        for m in all {
            assert_eq!(
                Mark::by_id(m.id()),
                Some(m),
                "{} does not round trip",
                m.id()
            );
        }
        assert_eq!(Mark::by_id("something-else"), None);
    }

    #[test]
    fn never_means_never_and_when_on_is_the_mirror() {
        assert!(!Mark::Never.shows(true));
        assert!(!Mark::Never.shows(false));
        assert!(Mark::WhenOn.shows(true));
        assert!(!Mark::WhenOn.shows(false));
    }

    #[test]
    fn the_id_is_what_serde_writes() {
        // The file and the text key are the same word, so a rename that
        // touched only one of them would be a preference that silently reset.
        for m in Mark::all() {
            let written = serde_json::to_string(&m).unwrap();
            assert_eq!(written, format!("\"{}\"", m.id()), "{m:?}");
        }
    }

    #[test]
    fn a_writer_refuses_over_a_file_that_will_not_parse() {
        // THE DEFECT THIS PREVENTS, which this repository has already shipped
        // once: `read()` hands a writer the DEFAULT for an unparseable file,
        // and the write then replaces the file with it. One keystroke, and
        // what was there is gone.
        let dir = std::env::temp_dir().join(format!("ronoctua-syncthing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ronoctua_store::Store::<Preferences>::at(Some(dir.clone()), FILE);

        // A file that is not there is not trouble: a fresh machine writes.
        assert!(keep(&store, Preferences { mark: Mark::WhenOn }).is_ok());
        assert_eq!(store.read().mark, Mark::WhenOn);

        // A file that will not parse stops the write dead.
        std::fs::write(dir.join(FILE), b"{ this is not json").unwrap();
        assert!(keep(&store, Preferences { mark: Mark::Never }).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.join(FILE)).unwrap(),
            "{ this is not json",
            "the unparseable file was replaced anyway"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_store_that_keeps_nothing_writes_nothing_and_says_so() {
        let store = ronoctua_store::Store::<Preferences>::at(None, FILE);
        assert!(store.path().is_none());
        assert!(keep(&store, Preferences::default()).is_ok());
    }
}
