//! WHAT THE FILE MANAGER KEEPS IN THE BELONGINGS (`C-m`, `k`) — each path
//! added to what is kept, or taken out, and the sentence it comes to.
//!
//! # Why a module of its own
//!
//! The file manager asks it of the belongings, and naming the belongings'
//! view to do so made the manager recompile with every line of it. What the
//! gesture writes is one file of the person's (`belongings.json`), through
//! the belongings crate's own types; this is that writing, under both.

use crate::key;
use ronoctua_belongings::kept::{self};
use ronoctua_belongings::places::{Places, covers};
use ronoctua_belongings::prefs::{self, Belongings};
use ronoctua_belongings::status::Last;
use ronoctua_files::mode::Explanation;
use std::path::{Path, PathBuf};

/// `belongings.json`'s store, under the configuration root of `places`.
#[must_use]
pub fn store_of(places: &Places) -> ronoctua_store::Store<Belongings> {
    ronoctua_store::Store::at(Some(places.config.clone()), prefs::FILE)
}

/// A logical path as a person reads it: under their home, `~/…`; a folder
/// the table keeps whole ends in `/`.
#[must_use]
pub fn shown_path(logical: &str, places: Option<&Places>, folder: bool) -> String {
    let resolved = places.and_then(|p| p.resolve(logical).map(|at| (p, at)));
    let mut text = match resolved {
        Some((p, at)) => match at.strip_prefix(&p.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => at.display().to_string(),
        },
        None => logical.to_owned(),
    };
    if folder && !text.ends_with('/') {
        text.push('/');
    }
    text
}

/// A path with `.` and `..` resolved by its words, never climbing past the
/// root — the file need not exist yet.
pub fn lexically_clean(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::RootDir => out.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(w) => out.push(w),
            Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

/// Why a change to the settings was not written.
#[derive(Debug)]
pub enum NotChanged {
    /// No places.
    Nowhere,
    /// The file does not parse, and nothing was written over it.
    Unparsed,
    /// It could not be written.
    NotWritten(std::io::Error),
}

impl NotChanged {
    /// Its sentence, as a key and its arguments — what the file manager's
    /// line takes.
    #[must_use]
    pub fn explanation(&self) -> Explanation {
        match self {
            Self::Nowhere => said(key::BELONGINGS_NO_PLACES),
            Self::Unparsed => said(key::BELONGINGS_PREFS_REFUSED),
            Self::NotWritten(why) => Explanation {
                key: key::ECHO_SETTING_NOT_WRITTEN,
                args: vec![("file", prefs::FILE.to_owned()), ("why", why.to_string())],
            },
        }
    }

    /// Its sentence.
    #[must_use]
    pub fn sentence(&self, texts: &ronoctua_i18n::Texts) -> String {
        let e = self.explanation();
        texts.t_with(e.key, &e.borrowed())
    }
}

/// A sentence with no arguments.
pub(crate) const fn said(key: &'static str) -> Explanation {
    Explanation {
        key,
        args: Vec::new(),
    }
}

/// One change to `belongings.json`, read with `whole` — never `read`: a
/// file that does not read whole is refused, not replaced, nor written with
/// a repair the engine would still refuse (it reads the file strictly) —
/// laid over the file as it is NOW, and written whole.
///
/// # Errors
///
/// A [`NotChanged`].
pub fn change_the_file(
    places: Option<&Places>,
    change: impl FnOnce(&mut Belongings),
) -> Result<(), NotChanged> {
    let places = places.ok_or(NotChanged::Nowhere)?;
    let store = store_of(places);
    let mut now = store.whole().ok_or(NotChanged::Unparsed)?;
    change(&mut now);
    store.write(&now).map_err(NotChanged::NotWritten)
}

/// The file manager's "keep in the belongings" (`C-m`, `k`): each path added
/// to what is kept — or, when all of them are kept already, taken out. The
/// sentence it comes to, for the manager's line.
#[must_use]
pub fn keep_from_the_files(paths: &[PathBuf]) -> Explanation {
    match Places::from_env() {
        Some(places) => keep_under(&places, paths),
        None => said(key::BELONGINGS_NO_PLACES),
    }
}

/// [`keep_from_the_files`], under `places` — what a test hands in.
#[must_use]
pub fn keep_under(places: &Places, paths: &[PathBuf]) -> Explanation {
    let logical: Vec<String> = paths
        .iter()
        .filter_map(|p| lexically_clean(p).and_then(|c| places.logical_of(&c)))
        .collect();
    if logical.is_empty() || logical.len() != paths.len() {
        return said(key::BELONGINGS_OUTSIDE_HOME);
    }
    // THE PLAIN PART travels already, in clear, and is never sealed too.
    if logical.iter().any(|l| kept::plain_root(l).is_some()) {
        return said(key::BELONGINGS_PLAIN_ALREADY);
    }
    if logical.iter().any(|l| !kept::may_add(l)) {
        return said(key::BELONGINGS_NEVER_KEPT);
    }
    let Some(prefs) = store_of(places).whole() else {
        return said(key::BELONGINGS_PREFS_REFUSED);
    };
    // WHAT ANOTHER MACHINE ADDED, followed here, is kept here too.
    let followed: Vec<String> = Last::read(&places.last())
        .filter(|l| l.repository == prefs.repository)
        .map(|l| l.roots.into_iter().collect())
        .unwrap_or_default();
    // ALL KEPT ALREADY: the same gesture takes them out — one row of the
    // menu, and what it did is said.
    let all_kept = logical
        .iter()
        .all(|l| kept::kept_here(l, &prefs, followed.iter().any(|f| covers(f, l))));
    let changed = change_the_file(Some(places), |p| {
        for l in &logical {
            if all_kept {
                p.stop_keeping(l);
            } else {
                p.keep(l);
            }
        }
    });
    if let Err(why) = changed {
        return why.explanation();
    }
    let name = if logical.len() == 1 {
        shown_path(&logical[0], Some(places), false)
    } else {
        logical.len().to_string()
    };
    Explanation {
        key: if all_kept {
            key::BELONGINGS_STOPPED_KEEPING
        } else {
            key::BELONGINGS_NOW_KEPT
        },
        args: vec![("name", name)],
    }
}

// ---------------------------------------------------------------------------
// SIGNING A CHANGE ANOTHER VIEW MAKES — the software centre's declarations
// ---------------------------------------------------------------------------
//
// The software centre asks these of the belongings before it queues a change
// of the person's signed declarations; they lived in the belongings' view,
// which the centre then had to stand on — so a line of that view recompiled
// the centre. They are the belongings as another view touches them, and
// that is this crate.

/// Whether a gesture of another view that changes the person's signed
/// declarations (the software centre's "install only for you", a program
/// declined or forgotten for good) asks the belongings' passphrase FIRST:
/// a repository is configured, this machine synced with it (its clone has
/// `key.age`, so there is a key to open), and the session keeps no identity.
/// The person is there, at the gesture: with the passphrase, `sign` signs the
/// change before it is staged and pushed, and it arrives on every machine;
/// without it, the change waits for the next sync that opens the identity.
#[must_use]
pub fn signing_asks(places: &Places, now_s: i64) -> bool {
    let Some(prefs) = store_of(places).whole() else {
        return false;
    };
    if !prefs.has_repository() || !places.key_age().is_file() {
        return false;
    }
    !ronoctua_belongings::unlock::Marker::read(places)
        .is_some_and(|m| prefs.keep_unlocked && m.holds(&prefs.repository, now_s))
}

/// Writes the passphrase another view asked to sign with where `sign`
/// reads it (and deletes it, once it opened `key.age` or refused): a `0600`
/// file of the belongings' runtime folder, as the belongings' own questions
/// write it.
///
/// # Errors
///
/// The key of the sentence that says the file could not be written.
pub fn write_passphrase_to_sign(places: &Places, typed: &str) -> Result<(), &'static str> {
    if typed.is_empty() {
        return Err(key::BELONGINGS_EMPTY_SECRET);
    }
    ronoctua_belongings::secret::write_private(&places.passphrase(), typed.as_bytes()).map_err(
        |e| {
            tracing::warn!(error = %e, "the passphrase to sign could not be written");
            key::BELONGINGS_NOT_WRITTEN
        },
    )
}
