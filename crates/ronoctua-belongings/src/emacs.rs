//! Which place this machine's Emacs reads its configuration from (feature
//! `prefs`) — what the belongings page SAYS when it is not `~/.config/emacs`,
//! the one place the target `emacs` carries. It never acts: nothing is held,
//! moved or deleted because of it, and a sync does not ask it anything.
//!
//! # Emacs 30's choice (`startup.el`)
//!
//! Its own XDG folder is `$XDG_CONFIG_HOME/emacs` — the variable's value as
//! it is, joined by `/emacs` (empty, it is `/emacs`; relative, it is
//! relative to where Emacs starts: the daemon's unit starts in the home) —
//! and `~/.config/emacs` only without the variable. The init FOLDER is
//! `~/.emacs.d` when it exists or `~/.emacs` does (`file-exists-p`, which
//! follows a link: a dangling one is not there); else the XDG folder when
//! that exists; else `~/.emacs.d`. From the XDG folder the init file is its
//! `init` (`init.elc`, else `init.el`), and `~/.emacs.el` is not read at
//! all. From the home it is `~/.emacs` as `load` finds it — `~/.emacs.elc`,
//! else `~/.emacs.el`, else `~/.emacs`, each only a FILE (`load` passes a
//! folder by) —, else `~/.emacs.d/init` (`init.elc`, else `init.el`); the
//! early init file is the folder's `early-init`, `.elc` first too. Measured
//! with Emacs 30.2 in throwaway homes (`docs/ARCHITECTURE.md`). So
//! `~/.emacs` or `~/.emacs.d` hide the XDG folder — `~/.emacs` even as a
//! folder, which is no init file —, `~/.emacs.el` does not once the XDG
//! folder is there, a `~/.emacs.d` that is a link to `~/.config/emacs` is
//! the same place, and a `~/.emacs.d` that holds only an `init.elc` holds
//! the person's configuration, compiled.
//!
//! # What the page says
//!
//! [`Reads::Elsewhere`]: Emacs reads another place here — which one, that the
//! belongings carry `~/.config/emacs`, and that moving the configuration
//! there carries it. [`Reads::Hidden`]: a `~/.emacs.d` that holds nothing of
//! the person's — only what the target would skip in `~/.config/emacs`, a
//! folder Emacs made before the XDG one existed — is read in its place.
//! [`Reads::NoFile`]: a `~/.emacs` that is no file (a folder) makes Emacs
//! read `~/.emacs.d` in its place. [`Reads::Xdg`]: `$XDG_CONFIG_HOME` puts
//! Emacs's own folder elsewhere, so Emacs here never reads `~/.config/emacs`
//! (the target carries the home's `~/.config/emacs`, whatever the variable
//! says) — and moving the configuration there would not change that.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::kept::{self, Inside};
use crate::places::Places;

/// The target that carries the configuration.
pub const TARGET: &str = "emacs";
/// The belongings' `~/.config/emacs`: the target's one path, in the home
/// whatever `$XDG_CONFIG_HOME` says.
pub const XDG: &str = "home/.config/emacs";
/// `~/.emacs.d`: the home's init folder.
pub const EMACS_D: &str = "home/.emacs.d";
/// The home's init files, in the order `load` tries them.
pub const HOME_FILES: [&str; 3] = ["home/.emacs.elc", "home/.emacs.el", "home/.emacs"];
/// `~/.emacs`: the one of them that also makes `~/.emacs.d` the folder.
pub const EMACS: &str = "home/.emacs";
/// The init files Emacs loads COMPILED from the root of its folder (`load`
/// tries `.elc` first): the person's, though the target skips `*.elc`.
const COMPILED_INIT: [&str; 2] = ["init.elc", "early-init.elc"];

/// The most entries [`holds_nothing`] reads before it gives up and takes
/// the folder for the person's: a `~/.emacs.d` of only what Emacs made is a
/// few folders, and the folders the target skips are not entered.
const LOOKED_MOST: usize = 4096;
/// How deep it goes.
const DEEPEST: usize = 16;

/// What the page says of where Emacs reads here: nothing when it reads
/// `~/.config/emacs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reads {
    /// It reads this other place (logical): `~/.emacs.d`, or one of the
    /// home's init files.
    Elsewhere(&'static str),
    /// It reads `~/.emacs.d`, which holds nothing of the person's (only
    /// what the target skips, or nothing at all), in the place of
    /// `~/.config/emacs`.
    Hidden,
    /// `~/.emacs` is there but is no file `load` takes (a folder): while it
    /// is, Emacs reads `~/.emacs.d` in the place of `~/.config/emacs`.
    NoFile,
    /// `$XDG_CONFIG_HOME` puts Emacs's own folder at this path, which is not
    /// the carried `~/.config/emacs`: Emacs here never reads the carried
    /// place (but through a `~/.emacs.d` that is it).
    Xdg(PathBuf),
}

/// What is at a logical path, as Emacs's tests see it: through links, a
/// dangling one missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum There {
    /// Nothing, or a dangling link.
    Missing,
    /// A regular file: what `load` takes.
    File,
    /// Anything else — a folder, mostly.
    Other,
}

/// What the disk under `places` makes Emacs read, as the page says it: a
/// few `stat`s, and — only for a `~/.emacs.d` read in the place of the XDG
/// folder — a walk of it that stops at the first file of the person's. Off
/// the event loop: the count's thread calls it.
///
/// `$XDG_CONFIG_HOME` is this process's (the footer's, a user unit like the
/// daemon, so the same manager's environment), and only when `places` are
/// this process's own home: another home's (a test's) takes Emacs's default.
#[must_use]
pub fn reads(places: &Places) -> Option<Reads> {
    let own = std::env::var_os("HOME").is_some_and(|h| Path::new(&h) == places.home);
    let value = own.then(|| std::env::var_os("XDG_CONFIG_HOME")).flatten();
    reads_with(places, value.as_deref())
}

/// [`reads`], with `$XDG_CONFIG_HOME` given (`None`: unset).
#[must_use]
pub fn reads_with(places: &Places, xdg_config_home: Option<&OsStr>) -> Option<Reads> {
    let at = |logical: &str| places.resolve(logical);
    let there = |logical: &str| at(logical).map_or(There::Missing, |p| there_at(&p));
    let carried = at(XDG)?;
    let own = own_folder(&places.home, xdg_config_home);
    let xdg = (!same_dir(&own, &carried)).then(|| (own.as_path(), own.exists()));
    decide(
        &there,
        xdg,
        &|| same_place(at(EMACS_D).as_deref(), Some(&carried)),
        &|| at(EMACS_D).is_some_and(|p| holds_nothing(&p)),
    )
}

/// Emacs's own XDG folder for `home` and `$XDG_CONFIG_HOME`'s value, as
/// `startup.el` makes it: `value` joined by `/emacs` as text — `~` the home,
/// a relative one under the home (where the daemon's unit starts) — and
/// `~/.config/emacs` without the variable. Pure.
#[must_use]
pub fn own_folder(home: &Path, xdg_config_home: Option<&OsStr>) -> PathBuf {
    let Some(value) = xdg_config_home else {
        return home.join(".config/emacs");
    };
    let mut joined = value.to_os_string();
    joined.push("/emacs");
    let joined = PathBuf::from(joined);
    if let Ok(rest) = joined.strip_prefix("~") {
        home.join(rest)
    } else if joined.is_absolute() {
        joined
    } else {
        home.join(joined)
    }
}

/// What is at `path`, through links.
fn there_at(path: &Path) -> There {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => There::File,
        Ok(_) => There::Other,
        Err(_) => There::Missing,
    }
}

/// Whether two folders are the same: the same path, or the same folder
/// through links.
fn same_dir(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(x), Ok(y)) if x == y
        )
}

/// The rule, over what is at each logical path (`there`), where Emacs's own
/// XDG folder is when it is not the carried one (`xdg`: the path, and
/// whether it exists), whether `~/.emacs.d` is the carried folder itself
/// (`same`), and whether it holds nothing of the person's (`empty`) — the
/// last two asked only when they decide. Pure.
#[must_use]
pub fn decide(
    there: &dyn Fn(&str) -> There,
    xdg: Option<(&Path, bool)>,
    same: &dyn Fn() -> bool,
    empty: &dyn Fn() -> bool,
) -> Option<Reads> {
    let exists = |logical: &str| there(logical) != There::Missing;
    let home = exists(EMACS_D) || exists(EMACS);
    let file = HOME_FILES.iter().copied().find(|f| there(f) == There::File);
    let linked = || exists(EMACS_D) && same();
    if let Some((at, at_exists)) = xdg {
        // EMACS'S OWN FOLDER IS ELSEWHERE: it reads the carried one only
        // through a `~/.emacs.d` that is it — and then the home's init file
        // first, as below; where nothing is anywhere, there is nothing to
        // say.
        if home && linked() {
            return file.map(Reads::Elsewhere);
        }
        if !home && file.is_none() && !at_exists && !exists(XDG) {
            return None;
        }
        return Some(Reads::Xdg(at.to_path_buf()));
    }
    if !home {
        // THE XDG FOLDER, when it is there, is read whole — a `~/.emacs.el`
        // beside it is not read at all. Without it, a `~/.emacs.el` alone is
        // the init file; with nothing, there is nothing to say.
        return if exists(XDG) {
            None
        } else {
            file.map(Reads::Elsewhere)
        };
    }
    // FROM THE HOME, its init file first, wherever `~/.emacs.d` points.
    if let Some(file) = file {
        return Some(Reads::Elsewhere(file));
    }
    if linked() {
        None
    } else if exists(EMACS) {
        // A `~/.emacs` THAT IS NO FILE still makes the folder `~/.emacs.d`.
        Some(Reads::NoFile)
    } else if empty() {
        Some(Reads::Hidden)
    } else {
        Some(Reads::Elsewhere(EMACS_D))
    }
}

/// Whether `emacs_d` is the carried folder itself — a link to it — and the
/// carried folder a folder (a link there carries only the link, not what it
/// points at).
fn same_place(emacs_d: Option<&Path>, carried: Option<&Path>) -> bool {
    let (Some(emacs_d), Some(carried)) = (emacs_d, carried) else {
        return false;
    };
    if !std::fs::symlink_metadata(carried).is_ok_and(|m| m.is_dir()) {
        return false;
    }
    match (
        std::fs::canonicalize(emacs_d),
        std::fs::canonicalize(carried),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Whether the folder `dir` holds nothing of the person's: every name in it
/// one the target would skip below `~/.config/emacs` (`kept::Target::
/// inside_below`), at any depth — but an `init.elc` or an `early-init.elc`
/// at its root, which Emacs loads — and a folder it cannot read, or one too
/// large to look through, holds something. Links in it are not followed.
#[must_use]
pub fn holds_nothing(dir: &Path) -> bool {
    let Some(target) = kept::target_by_id(TARGET) else {
        return false;
    };
    let mut looked = 0usize;
    let mut todo: Vec<(PathBuf, Vec<String>)> = vec![(dir.to_path_buf(), Vec::new())];
    while let Some((at, below)) = todo.pop() {
        if below.len() >= DEEPEST {
            return false;
        }
        let Ok(each) = std::fs::read_dir(&at) else {
            return false;
        };
        for entry in each {
            looked += 1;
            if looked > LOOKED_MOST {
                return false;
            }
            let Ok(entry) = entry else {
                return false;
            };
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return false;
            };
            // WHAT EMACS LOADS COMPILED is the person's configuration, though
            // the target skips what was compiled.
            if below.is_empty() && COMPILED_INIT.contains(&name.as_str()) {
                return false;
            }
            let mut path = below.clone();
            path.push(name);
            let names: Vec<&str> = path.iter().map(String::as_str).collect();
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            match target.inside_below(&names) {
                Inside::Skipped(_) => {}
                Inside::Passage(_) | Inside::Carried if is_dir => todo.push((entry.path(), path)),
                Inside::Passage(_) => {}
                Inside::Carried => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the target that carries the configuration is not the one
    /// place this module says it is.
    #[test]
    fn the_target_carries_the_xdg_folder_alone() {
        let target = kept::target_by_id(TARGET).expect("the emacs target");
        assert_eq!(target.paths, [XDG]);
    }

    /// FAILS IF: what this says Emacs reads is not what `startup.el` makes
    /// it read (measured against Emacs 30.2 in throwaway homes): the home's
    /// places before the XDG folder, `~/.emacs` before `~/.emacs.d/init`,
    /// a `~/.emacs` that is a folder no init file but still the home's sign,
    /// and an XDG folder elsewhere never the carried one.
    #[test]
    fn emacs_reads_what_startup_el_reads() {
        let elsewhere = Path::new("/elsewhere/emacs");
        let on = |files: &[&str], others: &[&str], xdg: Option<bool>, same: bool, empty: bool| {
            let there = |p: &str| {
                if files.contains(&p) {
                    There::File
                } else if others.contains(&p) {
                    There::Other
                } else {
                    There::Missing
                }
            };
            decide(&there, xdg.map(|e| (elsewhere, e)), &|| same, &|| empty)
        };
        let el = "home/.emacs.el";
        let elc = "home/.emacs.elc";
        let n = |files: &[&str], others: &[&str]| on(files, others, None, false, false);
        assert_eq!(n(&[], &[]), None, "nothing: nothing to say");
        assert_eq!(n(&[], &[XDG]), None);
        assert_eq!(
            n(&[EMACS], &[XDG]),
            Some(Reads::Elsewhere(EMACS)),
            "~/.emacs hides the XDG folder"
        );
        assert_eq!(
            n(&[], &[XDG, EMACS_D]),
            Some(Reads::Elsewhere(EMACS_D)),
            "so does ~/.emacs.d"
        );
        assert_eq!(
            on(&[], &[XDG, EMACS_D], None, false, true),
            Some(Reads::Hidden),
            "a ~/.emacs.d with nothing of the person's"
        );
        assert_eq!(
            on(&[], &[XDG, EMACS_D], None, true, true),
            None,
            "~/.emacs.d a link to the XDG folder is the same place"
        );
        assert_eq!(
            on(&[EMACS], &[XDG, EMACS_D], None, true, false),
            Some(Reads::Elsewhere(EMACS)),
            "linked, but the home's init file is read first"
        );
        assert_eq!(n(&[el], &[XDG]), None, "~/.emacs.el does not");
        assert_eq!(n(&[el], &[]), Some(Reads::Elsewhere(el)));
        assert_eq!(n(&[el, EMACS], &[EMACS_D]), Some(Reads::Elsewhere(el)));
        assert_eq!(n(&[elc, el], &[EMACS_D]), Some(Reads::Elsewhere(elc)));
        assert_eq!(
            on(&[EMACS], &[EMACS_D], None, false, true),
            Some(Reads::Elsewhere(EMACS))
        );
        // `~/.emacs` A FOLDER: no init file — Emacs reads `~/.emacs.d`,
        // whatever is in it, or there is none.
        for (emacs_d, empty) in [(true, false), (true, true), (false, true)] {
            let others: Vec<&str> = [XDG, EMACS]
                .into_iter()
                .chain(emacs_d.then_some(EMACS_D))
                .collect();
            assert_eq!(
                on(&[], &others, None, false, empty),
                Some(Reads::NoFile),
                "~/.emacs a folder, ~/.emacs.d {emacs_d} empty {empty}"
            );
        }
        assert_eq!(
            on(&[el], &[XDG, EMACS], None, false, true),
            Some(Reads::Elsewhere(el)),
            "a folder ~/.emacs, a file ~/.emacs.el: the file"
        );
        assert_eq!(
            on(&[], &[XDG, EMACS, EMACS_D], None, true, false),
            None,
            "a folder ~/.emacs, ~/.emacs.d linked to the XDG folder"
        );
        // `$XDG_CONFIG_HOME` ELSEWHERE: the carried folder is never read —
        // but through a `~/.emacs.d` that is it.
        let x = |files: &[&str], others: &[&str], exists: bool, same: bool| {
            on(files, others, Some(exists), same, false)
        };
        let said = Some(Reads::Xdg(elsewhere.to_path_buf()));
        assert_eq!(x(&[], &[], true, false), said, "Emacs reads it there");
        assert_eq!(x(&[], &[XDG], false, false), said, "and nothing here");
        assert_eq!(x(&[], &[XDG], true, false), said);
        assert_eq!(x(&[el], &[XDG], false, false), said);
        assert_eq!(x(&[], &[XDG, EMACS_D], true, false), said);
        assert_eq!(x(&[], &[XDG, EMACS_D], false, true), None, "linked");
        assert_eq!(
            x(&[EMACS], &[XDG, EMACS_D], false, true),
            Some(Reads::Elsewhere(EMACS)),
            "linked, the home's init file first"
        );
        assert_eq!(x(&[], &[], false, false), None, "nothing anywhere");
    }

    /// FAILS IF: Emacs's own folder is not where `startup.el` puts it for a
    /// `$XDG_CONFIG_HOME` — unset, absolute, empty (`/emacs`), a `~`, or
    /// relative (under the home, where the daemon starts).
    #[test]
    fn the_own_folder_is_where_startup_el_puts_it() {
        let home = Path::new("/h");
        let of = |v: Option<&str>| own_folder(home, v.map(OsStr::new));
        assert_eq!(of(None), Path::new("/h/.config/emacs"));
        assert_eq!(of(Some("/x/conf")), Path::new("/x/conf/emacs"));
        assert_eq!(of(Some("/x/conf/")), Path::new("/x/conf/emacs"));
        assert_eq!(of(Some("")), Path::new("/emacs"));
        assert_eq!(of(Some("~")), Path::new("/h/emacs"));
        assert_eq!(of(Some("~/.config")), Path::new("/h/.config/emacs"));
        assert_eq!(of(Some("conf")), Path::new("/h/conf/emacs"));
    }

    fn scratch(name: &str) -> (std::path::PathBuf, Places) {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-emacs-reads-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let places = Places::under(&root);
        std::fs::create_dir_all(&places.home).unwrap();
        (root, places)
    }

    fn put(places: &Places, logical: &str) {
        let at = places.resolve(logical).unwrap();
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, b";;").unwrap();
    }

    /// FAILS IF: on a real disk, a `~/.emacs.d` of only what Emacs made is
    /// not said as hiding the XDG folder, one with a file of the person's
    /// is not said as read, a link to the XDG folder is said at all, or the
    /// XDG folder alone is.
    #[test]
    fn what_the_disk_makes_emacs_read_is_said() {
        let (root, places) = scratch("disk");
        put(&places, "home/.config/emacs/init.el");
        assert_eq!(reads(&places), None);
        // WHAT EMACS MADE: its compiled cache, its packages, its session.
        put(&places, "home/.emacs.d/eln-cache/30.2-72dc/x.eln");
        put(&places, "home/.emacs.d/elpa/magit-4.1/magit.el");
        put(&places, "home/.emacs.d/auto-save-list/.saves-1-x~");
        put(&places, "home/.emacs.d/transient/history.el");
        put(&places, "home/.emacs.d/straight/repos/x/x.el");
        std::fs::create_dir_all(places.resolve("home/.emacs.d/empty").unwrap()).unwrap();
        assert_eq!(reads(&places), Some(Reads::Hidden));
        // A LOCKFILE THE TARGET CARRIES is the person's.
        put(&places, "home/.emacs.d/straight/versions/default.el");
        assert_eq!(reads(&places), Some(Reads::Elsewhere(EMACS_D)));
        std::fs::remove_dir_all(places.resolve(EMACS_D).unwrap()).unwrap();
        put(&places, "home/.emacs.d/init.el");
        assert_eq!(reads(&places), Some(Reads::Elsewhere(EMACS_D)));
        // A LINK TO THE XDG FOLDER is the same place.
        std::fs::remove_dir_all(places.resolve(EMACS_D).unwrap()).unwrap();
        std::os::unix::fs::symlink(".config/emacs", places.resolve(EMACS_D).unwrap()).unwrap();
        assert_eq!(reads(&places), None);
        // AND THE HOME'S INIT FILE beats it.
        put(&places, "home/.emacs");
        assert_eq!(reads(&places), Some(Reads::Elsewhere(EMACS)));
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: on a real disk, a `~/.emacs.d` holding only a compiled init
    /// file is said to hold nothing of the person's, a `~/.emacs` FOLDER is
    /// said as the init file read, a dangling link at `~/.emacs` or
    /// `~/.emacs.d` hides the carried folder, or a `$XDG_CONFIG_HOME`
    /// elsewhere is not said — while one that is the carried folder's,
    /// written another way or through a link, is.
    #[test]
    fn what_emacs_loads_and_where_its_folder_is_are_said() {
        let (root, places) = scratch("loads");
        put(&places, "home/.config/emacs/init.el");
        for compiled in ["init.elc", "early-init.elc"] {
            put(&places, &format!("home/.emacs.d/{compiled}"));
            put(&places, "home/.emacs.d/eln-cache/30.2-72dc/x.eln");
            assert_eq!(
                reads(&places),
                Some(Reads::Elsewhere(EMACS_D)),
                "{compiled}"
            );
            std::fs::remove_dir_all(places.resolve(EMACS_D).unwrap()).unwrap();
        }
        // A `.elc` DEEPER is no init file.
        put(&places, "home/.emacs.d/lisp/x.elc");
        assert_eq!(reads(&places), Some(Reads::Hidden));
        std::fs::remove_dir_all(places.resolve(EMACS_D).unwrap()).unwrap();
        // `~/.emacs` A FOLDER: Emacs reads `~/.emacs.d`, here none.
        std::fs::create_dir_all(places.resolve(EMACS).unwrap()).unwrap();
        assert_eq!(reads(&places), Some(Reads::NoFile));
        put(&places, "home/.emacs.d/init.el");
        assert_eq!(reads(&places), Some(Reads::NoFile));
        std::fs::remove_dir_all(places.resolve(EMACS).unwrap()).unwrap();
        std::fs::remove_dir_all(places.resolve(EMACS_D).unwrap()).unwrap();
        // DANGLING LINKS are not there for Emacs.
        for name in [EMACS, EMACS_D] {
            std::os::unix::fs::symlink("nowhere", places.resolve(name).unwrap()).unwrap();
            assert_eq!(reads(&places), None, "{name}");
            std::fs::remove_file(places.resolve(name).unwrap()).unwrap();
        }
        // `$XDG_CONFIG_HOME`.
        let other = root.join("other");
        let said = Some(Reads::Xdg(other.join("emacs")));
        assert_eq!(reads_with(&places, Some(other.as_os_str())), said);
        put(&places, "home/.emacs.el");
        assert_eq!(reads_with(&places, Some(other.as_os_str())), said);
        std::fs::remove_file(places.resolve("home/.emacs.el").unwrap()).unwrap();
        let config = places.home.join(".config");
        assert_eq!(reads_with(&places, Some(config.as_os_str())), None);
        assert_eq!(reads_with(&places, Some(OsStr::new("~/.config"))), None);
        assert_eq!(reads_with(&places, Some(OsStr::new(".config"))), None);
        std::os::unix::fs::symlink(&config, &other).unwrap();
        assert_eq!(reads_with(&places, Some(other.as_os_str())), None, "linked");
        std::fs::remove_file(&other).unwrap();
        assert_eq!(
            reads_with(&places, Some(OsStr::new(""))),
            Some(Reads::Xdg(PathBuf::from("/emacs")))
        );
        // ELSEWHERE, BUT `~/.emacs.d` IS THE CARRIED FOLDER: it is read.
        std::os::unix::fs::symlink(".config/emacs", places.resolve(EMACS_D).unwrap()).unwrap();
        assert_eq!(reads_with(&places, Some(other.as_os_str())), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
