//! The system's application entries — the base of the `LEADER a` launcher.
//!
//! # Why a crate, and not a module of the shell
//!
//! Because this is specification logic over `&str` and `Path`: it knows neither
//! Wayland, nor GPU, nor compositor, and that is why `cargo test` proves it in
//! milliseconds. It is the same reason as `ronoctua-clipboard` (queue and policy,
//! without whatever watches the selection) and as `ronoctua-term` (the VT machine, without the PTY).
//!
//! And because the footer's `surface.rs` already had more than four thousand lines.
//!
//! # What it does NOT do
//!
//! **It does not resolve icons.** It returns the raw `Icon=` and the `Categories=`; what
//! calls the resolution chain is the host, which already depends on `ronoctua-notify`.
//! The reverse would invert the crate graph and spread invariant 10 across
//! two places.
//!
//! **It launches nothing.** It returns `(program, arguments)`, which is exactly
//! what `open_foreign` receives.

pub mod defaults;
pub mod desktop;
pub mod exec;
pub mod fold;

use std::path::{Path, PathBuf};

/// An application the system offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    /// The freedesktop id: the path relative to `applications/` with `/` →
    /// `-`, without the extension. `kde4/foo.desktop` → `kde4-foo`.
    ///
    /// It is what deduplicates, and it is what the search matches along with the name — the id
    /// is usually the binary's name, and typing `nvim` to find "Neovim" is
    /// the gesture of whoever knows the system.
    pub id: String,
    /// The name, already in the session's language.
    pub name: String,
    /// The description, already in the session's language. Empty when there is none.
    pub description: String,
    /// The raw `Icon=` — a theme name **or** an absolute path; the spec allows
    /// both, and the resolution chain already handles both.
    pub icon: String,
    /// The `Categories=`, for the icon-by-category step.
    pub categories: Vec<String>,
    /// The `MimeType=` — which types this application says it opens.
    ///
    /// It was not parsed at all until the file manager needed it: an
    /// application launcher only ever asks "what is there", never "who opens
    /// this". It is what answers `open with` without spawning `xdg-open` to
    /// walk the same directories we already walked.
    pub mime_types: Vec<String>,
    /// The program and the arguments, ready for launching.
    ///
    /// The program is the **resolved path** when it exists in the `PATH` of
    /// whoever scanned — see [`resolve_program`]. It only stays as a raw name when
    /// it does not resolve.
    pub command: (String, Vec<String>),
    /// Does it ask for a terminal? These go through OUR terminal.
    pub terminal: bool,
    /// The `.desktop`'s `Path=`: the directory the application expects to start
    /// in. `None` when the entry does not say.
    ///
    /// It used to be parsed and dropped. What suffers is the application that
    /// reads a file next to its own binary — a Wine wrapper, a game with data
    /// alongside — and the symptom is the worst kind: it opens and dies, with
    /// nothing on screen.
    pub working_directory: Option<String>,
    /// The `StartupWMClass=`: the window class — on Wayland, the `app_id` —
    /// the entry promises its window will carry. `None` when it says nothing.
    ///
    /// It is what turns a WINDOW back into the entry that opened it, which
    /// the `app_id` alone cannot do for every program. A web app is the case
    /// that made it necessary: Chrome names an `--app=` window
    /// `chrome-web.whatsapp.com__-Default`, and the modeline's rule for a
    /// foreign `app_id` — keep the last dotted component — would draw
    /// `com__-Default`. The entry that launched it declares that string here,
    /// next to the `Name=` a person reads.
    pub startup_wm_class: Option<String>,
    /// The folded form of `name` — the key the search matches on.
    ///
    /// Computed ONCE during the scan. Folding per keystroke would redo the
    /// work of hundreds of entries on every letter typed.
    pub key: String,
    /// Whether opening a file may fall to it by the catalogue's order: `false`
    /// when the entry says `X-Ronoctua-Fallback=false` ([`FALLBACK_KEY`]).
    ///
    /// What opens a type no list chose is the first entry that declares it,
    /// and the catalogue is sorted by name — so a design tool that declares
    /// every image type, and whose name sorts before the browser's, took every
    /// photograph from the browser. An entry that says
    /// `false` keeps its `MimeType=` (it is still offered where a person
    /// chooses a program, `open with`) and a default that names it still
    /// wins; only the name order passes it by. The distro writes the key on
    /// the launchers it makes (`appimageOf`, `nix/modules/distro-apps.nix`).
    pub fallback: bool,
}

/// The launcher key that takes an entry out of the opener's name order
/// ([`DesktopEntry::fallback`]). Written by the distro's own launchers; an
/// entry without it is a fallback like any other.
pub const FALLBACK_KEY: &str = "X-Ronoctua-Fallback";

/// The `applications` directories, in priority order.
///
/// # `$XDG_DATA_HOME` FIRST
///
/// Without it in front, the user's override is invisible — and the override is the
/// whole freedesktop customisation mechanism. Whoever puts a
/// `.desktop` in `~/.local/share/applications` is saying precisely "mine
/// beats the system's".
#[must_use]
pub fn directories() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(c) = home {
        out.push(c.join("applications"));
    }
    // The spec's default when the variable does not exist. Without it, a session that
    // does not export it would see no application at all.
    let dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
    for d in dirs.split(':').filter(|d| !d.is_empty()) {
        out.push(Path::new(d).join("applications"));
    }
    out
}

/// How many directory levels the scan reads — the `applications` directory
/// itself and two below it.
///
/// Distribution themes put files in `applications/kde4/`; three levels
/// cover what exists. The ceiling is here so the loop terminates, not to
/// limit anyone.
///
/// Public because a second reader has to agree with it: the launcher's
/// staleness stamp (`apps::stamp` in the shell) looks at exactly the
/// directories this scan reads. One level fewer there and a `.desktop` added
/// two folders down would never trigger a rescan; one more and it would stat
/// directories whose files nobody reads.
pub const DEPTH: usize = 3;

/// Scans the given directories and returns the entries, sorted.
///
/// # The order is alphabetical, and not `read_dir`'s
///
/// `read_dir` returns in the filesystem's order: the list would change between
/// machines and on every installation, and the item under the cursor would swap on its own. It is the
/// same reason `icon::look_up` sorts the size directories.
///
/// The sort key is the **folded** one, otherwise `Ágil` would land after
/// `Zebra`.
#[must_use]
pub fn scan_in(dirs: &[PathBuf], language: &str) -> Vec<DesktopEntry> {
    let mut by_id: std::collections::BTreeMap<String, DesktopEntry> =
        std::collections::BTreeMap::new();
    for dir in dirs {
        let mut found = Vec::new();
        collect_from(dir, 0, &mut found);
        // Within a directory, a stable order before deduplicating: two
        // files with the same id in the same place do not exist, but the reading
        // order must not decide anything.
        found.sort();
        for path in found {
            let Some((id, input_of)) = read(&path, dir, language, Listing::Listed) else {
                continue;
            };
            // The FIRST wins: the directories already come in priority order.
            by_id.entry(id).or_insert(input_of);
        }
    }
    let mut out: Vec<DesktopEntry> = by_id.into_values().collect();
    out.sort_by(|a, b| a.key.cmp(&b.key).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Scans the environment's directories.
#[must_use]
pub fn scan(language: &str) -> Vec<DesktopEntry> {
    scan_in(&directories(), language)
}

/// The entry with this id, in the environment's directories — LISTED OR NOT.
/// See [`named_in`].
#[must_use]
pub fn named(id: &str, language: &str) -> Option<DesktopEntry> {
    named_in(&directories(), id, language)
}

/// The entry with this id, `NoDisplay=true` included: what a DEFAULT may
/// name.
///
/// # Why a default reaches what the launcher does not list
///
/// `NoDisplay` says "do not show me in a menu", not "do not open files with
/// me": the specification's own example is a program that only exists to
/// handle a type. The player's entry is one (`nix/modules/player.nix`): it
/// opens every audio and video type, the player's `mimeapps.list` names it
/// for each, and it is no program to launch by itself — so it is not in the
/// catalogue the launcher lists. The opener asked the catalogue alone, found
/// no such entry, skipped the default as "not installed" and handed films to
/// whatever declared them first.
///
/// The first directory that has the id answers, as in [`scan_in`]; a
/// `Hidden=true` file there is the entry deleted, and answers nothing.
/// `TryExec` and the rest of the reading are the catalogue's.
#[must_use]
pub fn named_in(dirs: &[PathBuf], id: &str, language: &str) -> Option<DesktopEntry> {
    for dir in dirs {
        let mut found = Vec::new();
        collect_from(dir, 0, &mut found);
        found.sort();
        if let Some(path) = found.iter().find(|p| id_of(p, dir).as_deref() == Some(id)) {
            return read(path, dir, language, Listing::Any).map(|(_, entry)| entry);
        }
    }
    None
}

/// The defaults the environment's `mimeapps.list` files choose, most
/// important first — see [`defaults::Defaults`] for the order and what it is
/// for. Read now, from the files; nothing is kept between calls.
#[must_use]
pub fn defaults() -> defaults::Defaults {
    defaults::Defaults::read_from(&defaults::Places::of_environment().lists())
}

/// The entry that declares it opens a window with this `app_id`.
///
/// Matched against `StartupWMClass=` — see
/// [`DesktopEntry::startup_wm_class`] for the window that needs it.
///
/// # Exact first, and only then ignoring case
///
/// The key predates Wayland: it was written for X11's `WM_CLASS`, whose
/// class half is capitalised by convention (`StartupWMClass=Gimp`), while the
/// same program's Wayland `app_id` is usually the lower-case one. Refusing the
/// second form would leave half the entries that bother to declare a class
/// unmatched; accepting it only after an exact pass keeps an exact match from
/// ever losing to a looser one.
///
/// When two entries declare the same class the first in the catalogue wins,
/// which is the catalogue's own rule: it is deduplicated with
/// `$XDG_DATA_HOME` first, so the person's override is the one found.
///
/// A linear walk, and on purpose: it runs when the modeline is drawn, over a
/// few hundred entries of which a handful declare a class — an index would be
/// a second structure to keep in step with every rescan for microseconds.
#[must_use]
pub fn declared_window<'a>(
    catalogue: &'a [DesktopEntry],
    app_id: &str,
) -> Option<&'a DesktopEntry> {
    if app_id.is_empty() {
        return None;
    }
    catalogue
        .iter()
        .find(|e| e.startup_wm_class.as_deref() == Some(app_id))
        .or_else(|| {
            catalogue.iter().find(|e| {
                e.startup_wm_class
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(app_id))
            })
        })
}

/// Gathers `dir`'s `.desktop` files, descending to [`DEPTH`].
fn collect_from(dir: &Path, level: usize, out: &mut Vec<PathBuf>) {
    let Ok(items) = std::fs::read_dir(dir) else {
        // A directory that does not exist is the common case, not an error: `XDG_DATA_DIRS`
        // lists places not every system has.
        return;
    };
    for item in items.filter_map(Result::ok) {
        let p = item.path();
        if p.is_dir() {
            if level + 1 < DEPTH {
                collect_from(&p, level + 1, out);
            }
        } else if p.extension().is_some_and(|e| e == "desktop") {
            out.push(p);
        }
    }
}

/// Which entries a reading takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Listing {
    /// What a menu lists: `NoDisplay=true` stays out.
    Listed,
    /// What a default may name: `NoDisplay=true` too ([`named_in`]).
    Any,
}

/// A `.desktop` becoming `(id, DesktopEntry)`. `None` when it should not appear.
fn read(
    path: &Path,
    root: &Path,
    language: &str,
    listing: Listing,
) -> Option<(String, DesktopEntry)> {
    let text = std::fs::read_to_string(path).ok()?;
    let fields = desktop::fields(&text);

    // The FILTERS, in order of cost. Measured on a actual host: without them, **41 of
    // 108** files get in, and 38% of the list becomes a MIME handler — the first
    // page is useless.
    if desktop::raw_bytes(&fields, "Type") != Some("Application") {
        return None;
    }
    if (listing == Listing::Listed && desktop::boolean(&fields, "NoDisplay"))
        || desktop::boolean(&fields, "Hidden")
    {
        return None;
    }
    // `TryExec` is the spec's answer to "a `.desktop` orphaned by a removed package".
    if let Some(t) = desktop::raw_bytes(&fields, "TryExec")
        && !exists_on_path(t)
    {
        return None;
    }

    let name = desktop::localized(&fields, "Name", language)?
        .trim()
        .to_owned();
    if name.is_empty() {
        return None;
    }
    let icon = desktop::raw_bytes(&fields, "Icon")
        .unwrap_or_default()
        .to_owned();
    let raw = desktop::raw_bytes(&fields, "Exec")?;
    let (program, args) = exec::assemble(
        raw,
        exec::Context {
            icon: &icon,
            name: &name,
            file: &path.to_string_lossy(),
            // NOTHING to open at scan time. What the file manager hands over
            // later goes through `command_for`, which re-assembles the same
            // `Exec=` line with the paths in place.
            open: &[],
        },
    )?;
    // THE PROGRAM BECOMES A PATH HERE, during the scan, and not at launch time:
    // here we are still in the session's `PATH`, and over there inside the systemd scope
    // it is minimal. See [`resolve_program`].
    //
    // If it does not resolve, the raw name stays: the `.desktop` may point at something
    // that only exists under another `PATH`, and making the application disappear over
    // that would be worse than letting it try and fail saying why.
    let program = resolve_program(&program).map_or(program, |p| p.to_string_lossy().into_owned());
    let command = (program, args);

    let id = id_of(path, root)?;
    let description = desktop::localized(&fields, "Comment", language)
        // `GenericName` is the second-best description: "Web Browser" when
        // there is no `Comment`. Measured: 87 have `Comment`, 51 have `GenericName`, and
        // the cascade covers nearly everything.
        .or_else(|| desktop::localized(&fields, "GenericName", language))
        .unwrap_or_default()
        .trim()
        .to_owned();

    Some((
        id.clone(),
        DesktopEntry {
            key: fold::fold(&name),
            id,
            name,
            description,
            icon,
            categories: desktop::list(&fields, "Categories")
                .into_iter()
                .map(str::to_owned)
                .collect(),
            mime_types: desktop::list(&fields, "MimeType")
                .into_iter()
                .map(str::to_owned)
                .collect(),
            command,
            terminal: desktop::boolean(&fields, "Terminal"),
            // `Path=` is a plain value, never localized — the spec says so, and
            // `raw` is what reads the last occurrence like every other key here.
            working_directory: desktop::raw_bytes(&fields, "Path")
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_owned),
            // A plain string, never localized: it names a window, and a
            // window's class is the same whatever language is on screen.
            startup_wm_class: desktop::raw_bytes(&fields, "StartupWMClass")
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_owned),
            // ONLY `false` says no: a key that is absent, or says anything
            // else, leaves the entry a fallback like any other.
            fallback: !desktop::raw_bytes(&fields, FALLBACK_KEY)
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("false")),
        },
    ))
}

/// The freedesktop id: the path relative to the root, `/` → `-`, without the extension.
fn id_of(path: &Path, root: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let s = rel.to_str()?;
    Some(s.trim_end_matches(".desktop").replace('/', "-"))
}

/// Where `program` actually is.
///
/// An absolute path is checked directly; a bare name is looked up in `PATH`, which is what
/// the spec demands for `TryExec`.
///
/// # Why the PATH, and not a boolean
///
/// Because whoever launches needs it. The `.desktop` writes `Exec=firefox`, and
/// that name only resolves against the `PATH` of **whoever scanned** — the application starts
/// inside a `systemd-run --scope`, where `PATH` is minimal. It is the trap
/// `AGENTS.md` records in so many words: *spawning in a systemd unit, use a PATH,
/// never a name* — and the failure shows up far from the cause.
///
/// As a bonus it is what makes the message line always say the same thing: without
/// this, half the applications showed `/nix/store/…/bin/x` and the other
/// half only `x`, depending on what the vendor wrote in the file.
fn resolve_program(program: &str) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(program))
        .find(|c| c.exists())
}

/// Does the `TryExec` resolve?
fn exists_on_path(program: &str) -> bool {
    resolve_program(program).is_some()
}

#[cfg(test)]
mod tests {
    use super::{DesktopEntry, named_in, scan_in};
    use std::path::PathBuf;

    /// Writes `.desktop` files into a temporary directory and scans.
    fn with_it(files: &[(&str, &str)]) -> Vec<DesktopEntry> {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-apps-teste-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        for (name, content) in files {
            let target = root.join(name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            std::fs::write(&target, content).expect("write");
        }
        let r = scan_in(std::slice::from_ref(&root), "pt_BR");
        std::fs::remove_dir_all(&root).ok();
        r
    }

    /// THE DEFECT: without the filters, 38% of the list becomes a MIME handler.
    #[test]
    fn what_must_not_show_up_does_not_show_up() {
        let r = with_it(&[
            (
                "bom.desktop",
                "[Desktop Entry]\nType=Application\nName=Bom\nExec=bom\n",
            ),
            (
                "escondido.desktop",
                "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n",
            ),
            (
                "oculto.desktop",
                "[Desktop Entry]\nType=Application\nName=Y\nExec=y\nHidden=true\n",
            ),
            (
                "link.desktop",
                "[Desktop Entry]\nType=Link\nName=Z\nURL=http://x\n",
            ),
            (
                "orphan.desktop",
                "[Desktop Entry]\nType=Application\nName=W\nExec=w\nTryExec=/does/not/exist/at-all\n",
            ),
            (
                "no-exec.desktop",
                "[Desktop Entry]\nType=Application\nName=V\n",
            ),
        ]);
        assert_eq!(
            r.len(),
            1,
            "{:?} was left over",
            r.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
        assert_eq!(r[0].name, "Bom");
    }

    /// FAILS IF: a default cannot reach a `NoDisplay` entry (the player's,
    /// which opens every film and is no program to launch), or a `Hidden`
    /// one — the entry deleted — comes back, or a lower directory answers
    /// for an id a higher one has.
    #[test]
    fn a_default_reaches_an_entry_the_menu_does_not_list() {
        let base = std::env::temp_dir().join(format!(
            "ronoctua-apps-named-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let (mine, system) = (base.join("mine"), base.join("system"));
        let write = |dir: &PathBuf, name: &str, text: &str| {
            let target = dir.join(name);
            std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
            std::fs::write(target, text).expect("write");
        };
        write(
            &system,
            "br.ronoctua.player.desktop",
            "[Desktop Entry]\nType=Application\nName=Player\nExec=play %U\nNoDisplay=true\n",
        );
        write(
            &system,
            "gone.desktop",
            "[Desktop Entry]\nType=Application\nName=Gone\nExec=gone\n",
        );
        write(
            &mine,
            "gone.desktop",
            "[Desktop Entry]\nType=Application\nName=Gone\nExec=gone\nHidden=true\n",
        );
        write(
            &system,
            "kde4/deep.desktop",
            "[Desktop Entry]\nType=Application\nName=Deep\nExec=deep\nNoDisplay=true\n",
        );
        let dirs = vec![mine, system];
        let listed = scan_in(&dirs, "pt_BR");
        let player = named_in(&dirs, "br.ronoctua.player", "pt_BR");
        let gone = named_in(&dirs, "gone", "pt_BR");
        let deep = named_in(&dirs, "kde4-deep", "pt_BR");
        let nobody = named_in(&dirs, "nobody", "pt_BR");
        std::fs::remove_dir_all(&base).ok();
        assert!(
            listed.iter().all(|e| e.id != "br.ronoctua.player"),
            "the menu lists a NoDisplay entry"
        );
        assert_eq!(
            player.map(|e| e.command.1),
            Some(vec![]),
            "a default cannot reach the player"
        );
        assert_eq!(gone, None, "a Hidden entry is the entry deleted");
        assert_eq!(deep.map(|e| e.name), Some("Deep".to_owned()));
        assert_eq!(nobody, None);
    }

    /// FAILS IF: the opener's name order cannot be told to pass an entry by
    /// (`X-Ronoctua-Fallback=false`), or anything but `false` does it.
    #[test]
    fn only_false_takes_an_entry_out_of_the_name_order() {
        let r = with_it(&[
            (
                "a.desktop",
                "[Desktop Entry]\nType=Application\nName=A\nExec=a\nX-Ronoctua-Fallback=false\n",
            ),
            (
                "b.desktop",
                "[Desktop Entry]\nType=Application\nName=B\nExec=b\nX-Ronoctua-Fallback= FALSE \n",
            ),
            (
                "c.desktop",
                "[Desktop Entry]\nType=Application\nName=C\nExec=c\nX-Ronoctua-Fallback=no\n",
            ),
            (
                "d.desktop",
                "[Desktop Entry]\nType=Application\nName=D\nExec=d\n",
            ),
        ]);
        let fallback: Vec<(&str, bool)> = r.iter().map(|e| (e.id.as_str(), e.fallback)).collect();
        assert_eq!(
            fallback,
            [("a", false), ("b", false), ("c", true), ("d", true)]
        );
    }

    /// THE DEFECT: `read_dir` is the filesystem's order, and the item under the
    /// cursor swaps on its own between machines.
    #[test]
    fn the_order_is_alphabetical_and_ignores_accents() {
        let r = with_it(&[
            (
                "z.desktop",
                "[Desktop Entry]\nType=Application\nName=Zebra\nExec=z\n",
            ),
            (
                "a.desktop",
                "[Desktop Entry]\nType=Application\nName=Ágil\nExec=a\n",
            ),
            (
                "m.desktop",
                "[Desktop Entry]\nType=Application\nName=Meio\nExec=m\n",
            ),
        ]);
        let names: Vec<&str> = r.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Ágil", "Meio", "Zebra"]);
    }

    /// THE DEFECT: a shallow `read_dir` loses the vendor subdirectories, and their
    /// id has to carry the path.
    #[test]
    fn the_subdirectory_goes_in_and_the_id_carries_the_path() {
        let r = with_it(&[(
            "kde4/foo.desktop",
            "[Desktop Entry]\nType=Application\nName=Foo\nExec=foo\n",
        )]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "kde4-foo");
    }

    #[test]
    fn the_description_falls_back_to_generic_name_when_there_is_no_comment() {
        let r = with_it(&[(
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Firefox\nExec=firefox\n\
             GenericName=Navegador Web\n",
        )]);
        assert_eq!(r[0].description, "Navegador Web");
    }

    /// THE DEFECT: without `XDG_DATA_HOME` in front, the user's override is
    /// invisible — and it is the whole freedesktop mechanism.
    #[test]
    fn the_first_directory_wins_on_the_same_id() {
        let base = std::env::temp_dir().join(format!(
            "ronoctua-apps-prio-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let (mine, system) = (base.join("mine"), base.join("system"));
        for (d, name) in [(&mine, "O Meu"), (&system, "O Do Sistema")] {
            std::fs::create_dir_all(d).expect("mkdir");
            std::fs::write(
                d.join("x.desktop"),
                format!("[Desktop Entry]\nType=Application\nName={name}\nExec=x\n"),
            )
            .expect("write");
        }
        let dirs: Vec<PathBuf> = vec![mine, system];
        let r = scan_in(&dirs, "pt_BR");
        std::fs::remove_dir_all(&base).ok();
        assert_eq!(r.len(), 1, "the same id cannot appear twice");
        assert_eq!(r[0].name, "O Meu");
    }

    /// THE DEFECT, SEEN ON SCREEN: half the applications showed the binary's
    /// path and the other half only the name, depending on what the vendor
    /// wrote in the `Exec=`.
    ///
    /// And the side that hurts more than the inconsistency: the application starts inside
    /// a `systemd-run --scope`, where `PATH` is minimal — a bare name there
    /// fails far from the cause.
    #[test]
    fn the_program_becomes_a_path_when_it_exists() {
        // `sh` is on every system that runs this test — including in the
        // Nix sandbox, where `PATH` is short but has a shell.
        let r = with_it(&[(
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Shell\nExec=sh -c oi\n",
        )]);
        assert_eq!(r.len(), 1);
        let (program, args) = &r[0].command;
        assert!(
            std::path::Path::new(program).is_absolute(),
            "`sh` was left as a raw name: {program}"
        );
        assert!(
            program.ends_with("/sh"),
            "it resolved to something else: {program}"
        );
        assert_eq!(args, &["-c", "oi"], "the arguments cannot change");

        // And what does NOT resolve stays as it is: making the application disappear would be
        // worse than letting it try and fail saying why.
        let r = with_it(&[(
            "b.desktop",
            "[Desktop Entry]\nType=Application\nName=Ghost\nExec=zzz-really-does-not-exist\n",
        )]);
        assert_eq!(r[0].command.0, "zzz-really-does-not-exist");
    }

    /// A web app's window, named back from the entry that opened it. The
    /// `app_id` is the one Chrome gives an `--app=` window; the modeline's
    /// fallback for it would be `com__-Default`.
    #[test]
    fn the_window_class_is_read_and_names_the_window() {
        let r = with_it(&[
            (
                "ronoctua-web-whatsapp.desktop",
                "[Desktop Entry]\nType=Application\nName=WhatsApp\nExec=web-open whatsapp\n\
                 StartupWMClass= chrome-web.whatsapp.com__-Default \n",
            ),
            (
                "gimp.desktop",
                "[Desktop Entry]\nType=Application\nName=GIMP\nExec=gimp\nStartupWMClass=Gimp\n",
            ),
            (
                "plain.desktop",
                "[Desktop Entry]\nType=Application\nName=Plain\nExec=plain\n",
            ),
        ]);
        let web = r.iter().find(|e| e.name == "WhatsApp").expect("scanned");
        // TRIMMED: a stray space is invisible in the file and would make the
        // class match nothing, which on screen is the fallback name again.
        assert_eq!(
            web.startup_wm_class.as_deref(),
            Some("chrome-web.whatsapp.com__-Default")
        );
        let plain = r.iter().find(|e| e.name == "Plain").expect("scanned");
        assert_eq!(plain.startup_wm_class, None, "no key is no class");

        let found = |id: &str| super::declared_window(&r, id).map(|e| e.name.as_str());
        assert_eq!(found("chrome-web.whatsapp.com__-Default"), Some("WhatsApp"));
        // The X11 capital against the Wayland lower case.
        assert_eq!(found("gimp"), Some("GIMP"));
        // Nothing declares these, and nothing is invented for them.
        assert_eq!(found("plain"), None, "a desktop id is not a window class");
        assert_eq!(found("chrome-web.whatsapp.com__-Profile_1"), None);
        assert_eq!(found(""), None);
    }

    /// An exact match is never beaten by a looser one that comes first.
    #[test]
    fn an_exact_class_beats_one_that_only_differs_in_case() {
        let r = with_it(&[
            (
                "a.desktop",
                "[Desktop Entry]\nType=Application\nName=Aaa\nExec=a\nStartupWMClass=Tool\n",
            ),
            (
                "b.desktop",
                "[Desktop Entry]\nType=Application\nName=Bbb\nExec=b\nStartupWMClass=tool\n",
            ),
        ]);
        let found = super::declared_window(&r, "tool").map(|e| e.name.as_str());
        assert_eq!(found, Some("Bbb"));
    }

    #[test]
    fn the_search_key_arrives_already_folded() {
        let r = with_it(&[(
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Cálculo\nExec=galculator\n",
        )]);
        assert_eq!(r[0].key, "calculo");
    }
}
