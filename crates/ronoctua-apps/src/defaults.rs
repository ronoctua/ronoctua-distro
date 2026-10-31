//! Which application a type opens with by CHOICE — the `[Default Applications]`
//! of every `mimeapps.list`, in the order the specification reads them.
//!
//! # Why the catalogue alone was not enough
//!
//! The catalogue answers "who DECLARES this type", and for most types that is
//! the whole question. It is not for a type that descends from another: a
//! `.step` was `model/step`, which the database then made a child of
//! `text/plain` (shared-mime-info 2.4, where the type was ours; upstream's own
//! since 2.5.1 has no parent), and OrcaSlicer's entry did not name
//! `model/step` — so the file went to the first editor that declares
//! `text/plain`. A `.obj` (`model/obj`) is a `text/plain` still, and reaches
//! OrcaSlicer by its declaration only because the opener reads the alias its
//! entry names it by (`application/prs.wavefront-obj`) as the type. The
//! distribution's answer is a default for every model type OrcaSlicer opens
//! (`nix/modules/workshop.nix` writes `/etc/xdg/mimeapps.list`), and a
//! person's answer is the same file under their own `~/.config`. Neither was
//! read.
//!
//! # The order is the specification's, and it is the preference
//!
//! The mime-apps specification lists the files most important first:
//! `$XDG_CONFIG_HOME`, then each of `$XDG_CONFIG_DIRS` (the distribution's
//! `/etc/xdg`), then `$XDG_DATA_HOME/applications` and each of
//! `$XDG_DATA_DIRS/applications` (where a package keeps its own defaults —
//! the player's). In each directory the desktop-specific
//! `<desktop>-mimeapps.list` comes before the plain one. A person who chose
//! mpv for a film in their own file beats the player's default in the store,
//! which is what "default" means.
//!
//! Within that order the FIRST listed entry that is installed wins; an id
//! naming a program that is not in the catalogue is skipped, as the
//! specification says, and the next one is tried — in the same file and then
//! in the next. That is why [`Defaults::for_type`] hands out every candidate
//! in order and the caller keeps the first it can open.
//!
//! # Only the type itself, never its parents
//!
//! A default for `text/plain` does not answer `model/obj`: that inheritance
//! is exactly how a model reached an editor. The parents are the catalogue's
//! business (`MimeType=`), after the defaults had nothing to say.
//!
//! # Read when asked, never cached
//!
//! The files are small and read only when a file is opened. A default the
//! person changes counts at the next open, with nothing to watch.

use std::path::PathBuf;

/// The group the defaults live in. `[Added Associations]` and `[Removed
/// Associations]` shape "open with", which is the catalogue's; only this one
/// says what opens a type by itself.
const GROUP: &str = "[Default Applications]";

/// The file's name, with and without a desktop's prefix.
const FILE: &str = "mimeapps.list";

/// Where the lists are looked for — the environment, gathered, so the order
/// can be proved without touching the machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Places {
    /// `$XDG_CONFIG_HOME` (the person's own file).
    pub config_home: Option<PathBuf>,
    /// `$XDG_CONFIG_DIRS`, most important first (`/etc/xdg` by default).
    pub config_dirs: Vec<PathBuf>,
    /// `$XDG_DATA_HOME` — its `applications` folder is the deprecated place,
    /// still read.
    pub data_home: Option<PathBuf>,
    /// `$XDG_DATA_DIRS`, most important first.
    pub data_dirs: Vec<PathBuf>,
    /// `$XDG_CURRENT_DESKTOP`, lower-cased, most important first.
    pub desktops: Vec<String>,
}

impl Places {
    /// The places this process's environment names, with the specification's
    /// fallbacks for a variable that is not set.
    #[must_use]
    pub fn of_environment() -> Self {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        let home = var("HOME").map(PathBuf::from);
        let split = |value: Option<std::ffi::OsString>, fallback: &str| -> Vec<PathBuf> {
            let text =
                value.map_or_else(|| fallback.to_owned(), |v| v.to_string_lossy().into_owned());
            text.split(':')
                .filter(|d| !d.is_empty())
                .map(PathBuf::from)
                .collect()
        };
        Self {
            config_home: var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| home.as_ref().map(|h| h.join(".config"))),
            config_dirs: split(var("XDG_CONFIG_DIRS"), "/etc/xdg"),
            data_home: var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| home.as_ref().map(|h| h.join(".local/share"))),
            // The same fallback `directories()` uses, so the lists and the
            // entries are looked for under the same roots.
            data_dirs: split(var("XDG_DATA_DIRS"), "/usr/local/share:/usr/share"),
            desktops: var("XDG_CURRENT_DESKTOP")
                .map(|v| {
                    v.to_string_lossy()
                        .split(':')
                        .filter(|d| !d.is_empty())
                        .map(str::to_ascii_lowercase)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Every list, most important first, as the specification orders them.
    /// A path that does not exist is in the answer anyway: reading it is
    /// where that is found out.
    #[must_use]
    pub fn lists(&self) -> Vec<PathBuf> {
        let mut folders: Vec<PathBuf> = Vec::new();
        folders.extend(self.config_home.iter().cloned());
        folders.extend(self.config_dirs.iter().cloned());
        folders.extend(self.data_home.iter().map(|d| d.join("applications")));
        folders.extend(self.data_dirs.iter().map(|d| d.join("applications")));
        let mut out = Vec::new();
        for folder in folders {
            for desktop in &self.desktops {
                out.push(folder.join(format!("{desktop}-{FILE}")));
            }
            out.push(folder.join(FILE));
        }
        out
    }
}

/// The defaults every list names, most important first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Defaults {
    /// `(type, desktop ids)` in the order the lists were read; an id is the
    /// catalogue's (`OrcaSlicer`, not `OrcaSlicer.desktop`).
    chosen: Vec<(String, Vec<String>)>,
}

impl Defaults {
    /// The defaults written in `lists`, most important first. A list that
    /// cannot be read is skipped: most of them do not exist on any machine.
    #[must_use]
    pub fn read_from(lists: &[PathBuf]) -> Self {
        let mut chosen = Vec::new();
        for list in lists {
            if let Ok(text) = std::fs::read_to_string(list) {
                chosen.extend(parse(&text));
            }
        }
        Self { chosen }
    }

    /// The defaults of `text`, one list's worth.
    #[must_use]
    pub fn of_text(text: &str) -> Self {
        Self {
            chosen: parse(text),
        }
    }

    /// Every entry chosen for a type `names` accepts, most important first.
    ///
    /// `names` rather than a string, because a list may spell a type by an
    /// old alias and the caller is the one that knows the database's aliases
    /// (this crate knows none).
    pub fn for_type<'a>(
        &'a self,
        names: impl Fn(&str) -> bool + 'a,
    ) -> impl Iterator<Item = &'a str> {
        self.chosen
            .iter()
            .filter(move |(mime, _)| names(mime))
            .flat_map(|(_, ids)| ids.iter().map(String::as_str))
    }

    /// Whether no list chose anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chosen.is_empty()
    }
}

/// The `[Default Applications]` of one list: `(type, ids)` in file order.
///
/// A value is a `;`-separated list of desktop file names; each becomes the
/// catalogue's id (the `.desktop` suffix dropped). A line outside the group,
/// a comment, a key with no `=` or a value with no id is ignored — the file
/// is shared with other programs, and a line this does not read is theirs.
fn parse(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            inside = line == GROUP;
            continue;
        }
        if !inside {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let mime = key.trim();
        if mime.is_empty() {
            continue;
        }
        let ids: Vec<String> = value
            .split(';')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| id.strip_suffix(".desktop").unwrap_or(id).to_owned())
            .filter(|id| !id.is_empty())
            .collect();
        if !ids.is_empty() {
            out.push((mime.to_owned(), ids));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Defaults, Places, parse};
    use std::path::PathBuf;

    /// FAILS IF: the order stops being the specification's — the person's
    /// own file first, the distribution's next, a package's last, and each
    /// desktop's list before the plain one in its folder.
    #[test]
    fn the_lists_come_in_the_specifications_order() {
        let places = Places {
            config_home: Some(PathBuf::from("/h/.config")),
            config_dirs: vec![PathBuf::from("/etc/xdg"), PathBuf::from("/x/etc")],
            data_home: Some(PathBuf::from("/h/.local/share")),
            data_dirs: vec![PathBuf::from("/run/sw/share")],
            desktops: vec!["scroll".into()],
        };
        let lists: Vec<String> = places
            .lists()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            lists,
            [
                "/h/.config/scroll-mimeapps.list",
                "/h/.config/mimeapps.list",
                "/etc/xdg/scroll-mimeapps.list",
                "/etc/xdg/mimeapps.list",
                "/x/etc/scroll-mimeapps.list",
                "/x/etc/mimeapps.list",
                "/h/.local/share/applications/scroll-mimeapps.list",
                "/h/.local/share/applications/mimeapps.list",
                "/run/sw/share/applications/scroll-mimeapps.list",
                "/run/sw/share/applications/mimeapps.list",
            ]
        );
    }

    /// FAILS IF: another group's lines are read as defaults — `[Added
    /// Associations]` lists what MAY open a type, not what does — or the
    /// suffix stays on the id, or an empty id survives.
    #[test]
    fn only_the_default_group_is_read_and_ids_lose_the_suffix() {
        let text = "\
# a comment
[Added Associations]
model/stl=editor.desktop;
[Default Applications]
model/stl=OrcaSlicer.desktop;other.desktop;
model/step = OrcaSlicer.desktop ;
broken line
text/plain=;
[Removed Associations]
model/3mf=OrcaSlicer.desktop;
";
        assert_eq!(
            parse(text),
            [
                (
                    "model/stl".to_owned(),
                    vec!["OrcaSlicer".to_owned(), "other".to_owned()]
                ),
                ("model/step".to_owned(), vec!["OrcaSlicer".to_owned()]),
            ]
        );
    }

    /// FAILS IF: a later file's choice comes before an earlier file's, or a
    /// type's alias is not handed to the caller to decide.
    #[test]
    fn the_candidates_keep_the_order_of_the_lists() {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-apps-defaults-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let mine = root.join("mine.list");
        let system = root.join("system.list");
        std::fs::write(&mine, "[Default Applications]\nvideo/mp4=mpv.desktop\n").expect("write");
        std::fs::write(
            &system,
            "[Default Applications]\nvideo/mp4=br.ronoctua.player.desktop\n\
             application/vnd.ms-3mfdocument=OrcaSlicer.desktop\n",
        )
        .expect("write");
        let missing = root.join("not-there.list");
        let d = Defaults::read_from(&[missing, mine, system]);
        std::fs::remove_dir_all(&root).ok();
        let film: Vec<&str> = d.for_type(|m| m == "video/mp4").collect();
        assert_eq!(film, ["mpv", "br.ronoctua.player"]);
        let model: Vec<&str> = d
            .for_type(|m| m == "model/3mf" || m == "application/vnd.ms-3mfdocument")
            .collect();
        assert_eq!(model, ["OrcaSlicer"]);
        assert_eq!(d.for_type(|m| m == "text/plain").count(), 0);
    }

    #[test]
    fn nothing_written_is_nothing_chosen() {
        assert!(Defaults::of_text("").is_empty());
        assert!(Defaults::read_from(&[PathBuf::from("/does/not/exist/mimeapps.list")]).is_empty());
    }
}
