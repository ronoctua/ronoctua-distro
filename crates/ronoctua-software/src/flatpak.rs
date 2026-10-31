//! The `--user` Flatpak installation — the person's Flatpak apps, and nothing
//! of the system's.
//!
//! # `--user`, on every argument vector
//!
//! Invariant 6's exception covers what a person installs for THEMSELVES, under
//! `$HOME`: here that is `$XDG_DATA_HOME/flatpak`. The system installation
//! (`/var/lib/flatpak`) belongs to the flake — nix-flatpak's NixOS module and
//! the update run's own `flatpak-system` step — so every builder below carries
//! `--user` and none carries `--system`, `sudo` or `pkexec`
//! (`software-stays-in-the-home`, and a unit test that reads every builder).
//!
//! # `-y`, and NOT `--noninteractive`
//!
//! Read in the pinned flatpak's source (1.16.6, `app/`): `--noninteractive`
//! does more than refuse to ask — it swaps the whole transaction for the QUIET
//! one (`flatpak-quiet-transaction.c`), which prints one `Installing <ref>`
//! line per operation and NO percentage at all. A job installing a 400 MB
//! runtime would draw a phase and no bar for minutes. `-y` keeps the CLI
//! transaction (`flatpak-cli-transaction.c`) with interaction disabled, and
//! with standard output a pipe it prints in PLAIN mode: the operations as a
//! tab-separated table, then `Installing 2/3…` and one line per progress tick
//! ending in ` 45%` — which is what [`Watch`] reads. Nothing can wait for a
//! person either way: standard input is `/dev/null`, and every flatpak prompt
//! answers itself "n" when standard input is not a terminal
//! (`flatpak-tty-utils.c`); `-y` makes the transaction choose the first remote,
//! add a `.flatpakref`'s remote and rebase an end-of-life app on its own, which
//! is what `--noninteractive` would have done. `FLATPAK_FANCY_OUTPUT=0` is set
//! too, so the plain format does not depend on how the job was started.
//!
//! # The dark theme: never `GTK_THEME`
//!
//! Nothing here, and nothing this crate writes, ever sets `GTK_THEME` — not in
//! a child's environment, not in an override. libadwaita reads the variable as
//! "the theme is being forced" and then skips its OWN stylesheet
//! (`adw-style-manager.c`: the providers are only added when `GTK_THEME` is
//! unset — even an empty value counts), so every GTK4/libadwaita app renders
//! unstyled. That is the breakage the owner remembers, and old tutorials paired
//! it with the dark extension. The dark mode reaches Flatpak apps through the
//! settings portal (`color-scheme`, which libadwaita reads) and through dconf's
//! `gtk-theme` (which GTK3 reads). `org.gtk.Gtk3theme.Adwaita-dark` only
//! affects GTK3 apps, and flatpak installs it BY ITSELF as a related ref of any
//! app whenever the host's `gtk-theme` is `Adwaita-dark` (`download-if =
//! active-gtk-theme`, read through GSettings — which needs the dconf GIO module
//! in the environment the CLI runs in, and that is the Nix module's job). The
//! advanced sheet can only REMOVE a `GTK_THEME` it finds ([`forces_gtk_theme`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::found::Picture;
use crate::order::{Consent, Display, Grant, Reach, Scope};
use crate::progress::{Phase, Report};
use crate::run::{self, Call, Ran};
use crate::state::{Installed, Origin};
use crate::{AppKey, Context, Done, Failure, Order, Outcome, Refusal, Source};

/// The one remote this crate installs from.
pub const REMOTE: &str = "flathub";

/// Where the remote's definition is fetched from when it has to be added.
pub const REMOTE_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// The cache of "what can be updated", under [`crate::Roots::ours_cache`].
pub const UPDATES_FILE: &str = "flatpak-updates.json";

/// The columns [`installed`] asks for, in this order. `active` is the
/// deployed commit (twelve characters), which is what tells a stale update
/// cache from a real update.
pub const LIST_COLUMNS: &str = "application,name,version,branch,origin,installation,size,active";

/// The columns the update query asks for, in this order. `commit` is the
/// remote's commit, cut to the same twelve characters as `active`.
pub const UPDATE_COLUMNS: &str = "application,commit,version";

/// Where nix-flatpak's Home Manager module keeps the state of the USER
/// installation, under `$XDG_STATE_HOME` (nix-flatpak v0.7.0,
/// `modules/flatpak/install.nix`: `${XDG_STATE_HOME:-$HOME/.local/state}/home-manager/gcroots`,
/// file `flatpak-state.json`). The file lists what the flake DECLARED, and
/// nothing else: the unmanaged apps are only merged into the old state in
/// memory, when `uninstallUnmanaged` is on.
pub const DECLARED_STATE: &str = "home-manager/gcroots/flatpak-state.json";

/// The version of [`Updates`]' on-disk shape.
pub const UPDATES_FORMAT: u32 = 1;

/// This machine's architecture in flatpak's words (`x86_64`, `aarch64`,
/// `i386`, `arm`) — the directory an appstream catalogue lives under and what
/// a Flathub listing's `arches` is compared with.
#[must_use]
pub fn arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86" => "i386",
        other => other,
    }
}

// ─── argument vectors ────────────────────────────────────────────────────────
//
// Every one is pure, and every one that takes an id expects one that passed
// [`valid_id`] — which starts with a letter or `_`, so it can never be read as
// an option. The `--` goes BEFORE all the positional arguments: GLib strips a
// `--` only when nothing unparsed precedes it, and a `--` left in place would
// be read as a ref.

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

/// `flatpak list` of the user installation's applications.
#[must_use]
pub fn list_argv() -> Vec<String> {
    argv(&[
        "list",
        "--user",
        "--app",
        &format!("--columns={LIST_COLUMNS}"),
    ])
}

/// `flatpak list` of EVERY ref of the user installation, with its size — the
/// two readings around a tidy.
#[must_use]
pub fn refs_argv() -> Vec<String> {
    argv(&["list", "--user", "--all", "--columns=ref,size"])
}

/// `flatpak remotes` of the user installation, by name.
#[must_use]
pub fn remotes_argv() -> Vec<String> {
    argv(&["remotes", "--user", "--columns=name"])
}

/// Install one app from [`REMOTE`].
#[must_use]
pub fn install_argv(id: &str) -> Vec<String> {
    argv(&["install", "--user", "-y", "--", REMOTE, id])
}

/// Install what a `.flatpakref` describes.
#[must_use]
pub fn install_file_argv(file: &Path) -> Vec<String> {
    let mut out = argv(&["install", "--user", "-y", "--from", "--"]);
    out.push(file.display().to_string());
    out
}

/// Uninstall one app, and its data too when `keep_data` is false.
#[must_use]
pub fn uninstall_argv(id: &str, keep_data: bool) -> Vec<String> {
    let mut out = argv(&["uninstall", "--user", "-y"]);
    if !keep_data {
        out.push("--delete-data".into());
    }
    out.extend(argv(&["--", id]));
    out
}

/// Update one app, or everything in the user installation.
#[must_use]
pub fn update_argv(id: Option<&str>) -> Vec<String> {
    let mut out = argv(&["update", "--user", "-y"]);
    if let Some(id) = id {
        out.extend(argv(&["--", id]));
    }
    out
}

/// Uninstall the runtimes and extensions no app of the user installation uses.
#[must_use]
pub fn tidy_argv() -> Vec<String> {
    argv(&["uninstall", "--user", "-y", "--unused"])
}

/// Add [`REMOTE`] to the user installation, unless it is there.
#[must_use]
pub fn remote_add_argv() -> Vec<String> {
    argv(&[
        "remote-add",
        "--user",
        "--if-not-exists",
        "--",
        REMOTE,
        REMOTE_URL,
    ])
}

/// Fetch [`REMOTE`]'s catalogue (the appstream data the offline search reads).
/// With a remote named, flatpak ignores its one-day TTL.
#[must_use]
pub fn appstream_argv() -> Vec<String> {
    argv(&["update", "--user", "--appstream", "--", REMOTE])
}

/// What can be updated, across every remote of the user installation.
#[must_use]
pub fn updates_argv() -> Vec<String> {
    argv(&[
        "remote-ls",
        "--user",
        "--updates",
        "--app",
        &format!("--columns={UPDATE_COLUMNS}"),
    ])
}

/// An installed app's effective permissions (its metadata with every override
/// applied — `flatpak_context_load_for_deploy`).
#[must_use]
pub fn permissions_argv(id: &str) -> Vec<String> {
    argv(&["info", "--user", "--show-permissions", "--", id])
}

/// `flatpak override --user` of one thing an app may reach. Every flag is
/// one `flatpak override` documents; the only environment flag is
/// `--unset-env=GTK_THEME` — nothing here can GIVE an app a variable.
#[must_use]
pub fn override_argv(id: &str, grant: Grant) -> Vec<String> {
    let flags: &[&str] = match grant {
        Grant::Files {
            reach: Reach::ReadWrite,
        } => &["--filesystem=home"],
        Grant::Files {
            reach: Reach::ReadOnly,
        } => &["--filesystem=home:ro"],
        // BOTH: an app granted `host` reaches the home through it.
        Grant::Files {
            reach: Reach::Nothing,
        } => &["--nofilesystem=home", "--nofilesystem=host"],
        Grant::Network { on: true } => &["--share=network"],
        Grant::Network { on: false } => &["--unshare=network"],
        Grant::Sound { on: true } => &["--socket=pulseaudio"],
        Grant::Sound { on: false } => &["--nosocket=pulseaudio"],
        Grant::Devices { on: true } => &["--device=all"],
        Grant::Devices { on: false } => &["--nodevice=all"],
        Grant::Display {
            through: Display::Wayland,
        } => &[
            "--socket=wayland",
            "--nosocket=x11",
            "--nosocket=fallback-x11",
        ],
        Grant::Display {
            through: Display::X11,
        } => &["--socket=x11", "--nosocket=wayland"],
        Grant::UnsetGtkTheme => &["--unset-env=GTK_THEME"],
        Grant::Reset => &["--reset"],
    };
    let mut out = argv(&["override", "--user"]);
    out.extend(flags.iter().map(|f| (*f).to_owned()));
    out.push("--".into());
    out.push(id.to_owned());
    out
}

/// What an app reaches today, read from its effective permissions
/// ([`permissions`]), in the reduced form [`Grant`] changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Grants {
    /// The person's files.
    pub files: Reach,
    /// The network.
    pub network: bool,
    /// Sound.
    pub sound: bool,
    /// Every device.
    pub devices: bool,
    /// The display protocol (`None`: neither socket, a program with no window).
    pub display: Option<Display>,
    /// `GTK_THEME` is forced into it.
    pub gtk_theme: bool,
}

/// Read the reduced grants from a `--show-permissions` key file. `home` and
/// `host` both reach the person's files, read-write unless marked `:ro`.
#[must_use]
pub fn grants_of(keyfile: &str) -> Grants {
    let mut group = "";
    let mut g = Grants::default();
    let mut sockets: Vec<&str> = Vec::new();
    for line in keyfile.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            group = name;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let items = || value.split(';').filter(|v| !v.is_empty());
        match (group, key) {
            ("Context", "filesystems") => {
                for fs in items() {
                    match fs {
                        "home" | "host" => g.files = Reach::ReadWrite,
                        "home:ro" | "host:ro" if g.files == Reach::Nothing => {
                            g.files = Reach::ReadOnly;
                        }
                        _ => {}
                    }
                }
            }
            ("Context", "shared") => g.network = items().any(|v| v == "network"),
            ("Context", "devices") => g.devices = items().any(|v| v == "all"),
            ("Context", "sockets") => sockets = items().collect(),
            _ => {}
        }
    }
    // ONE detector for a forced theme, the one the sheet already uses.
    g.gtk_theme = forces_gtk_theme(&permission_lines(keyfile));
    g.sound = sockets.contains(&"pulseaudio");
    g.display = if sockets.contains(&"wayland") {
        Some(Display::Wayland)
    } else if sockets.iter().any(|s| *s == "x11" || *s == "fallback-x11") {
        Some(Display::X11)
    } else {
        None
    };
    g
}

/// The dark theme's two declared parts, as this person's installation has
/// them (`nix/modules/flatpak.nix`): the GTK 3 extension and the override that
/// keeps the session's Qt variables out of every sandbox.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DarkFacts {
    /// `org.gtk.Gtk3theme.Adwaita-dark` is installed, in either installation.
    pub extension: bool,
    /// The global override unsets `QT_QPA_PLATFORMTHEME` and `QT_STYLE_OVERRIDE`.
    pub qt_unset: bool,
}

/// The two installations' directories, the person's first: the user
/// installation under `$XDG_DATA_HOME`, then the system's. Reading only —
/// nothing in this crate writes into the second.
#[must_use]
pub fn installations(roots: &crate::Roots) -> [PathBuf; 2] {
    [
        roots.data.join("flatpak"),
        PathBuf::from(crate::appstream::SYSTEM_ROOT),
    ]
}

/// The id of the GTK 3 dark theme extension.
pub const DARK_EXTENSION: &str = "org.gtk.Gtk3theme.Adwaita-dark";

/// The Qt variables the session exports and a sandbox must not see.
pub const QT_LEAKS: [&str; 2] = ["QT_QPA_PLATFORMTHEME", "QT_STYLE_OVERRIDE"];

/// Read the dark theme's facts from the FILES, without running flatpak: a
/// runtime is a directory under `runtime/`, and the global override is a key
/// file. `installations` are the user's and the system's directories.
#[must_use]
pub fn dark_facts(installations: &[PathBuf]) -> DarkFacts {
    let extension = installations
        .iter()
        .any(|i| i.join("runtime").join(DARK_EXTENSION).is_dir());
    let qt_unset = installations.first().is_some_and(|user| {
        std::fs::read_to_string(user.join("overrides").join("global")).is_ok_and(|text| {
            let unset: Vec<&str> = text
                .lines()
                .map(str::trim)
                .filter_map(|l| l.strip_prefix("unset-environment="))
                .flat_map(|v| v.split(';'))
                .collect();
            QT_LEAKS.iter().all(|q| unset.contains(q))
        })
    });
    DarkFacts {
        extension,
        qt_unset,
    }
}

/// Every builder with a sample input, for the test that reads them all.
#[cfg(test)]
fn every_argv() -> Vec<Vec<String>> {
    vec![
        list_argv(),
        refs_argv(),
        remotes_argv(),
        install_argv("org.gnome.Calculator"),
        install_file_argv(Path::new("/home/p/Downloads/x.flatpakref")),
        uninstall_argv("org.gnome.Calculator", true),
        uninstall_argv("org.gnome.Calculator", false),
        update_argv(None),
        update_argv(Some("org.gnome.Calculator")),
        tidy_argv(),
        remote_add_argv(),
        appstream_argv(),
        updates_argv(),
        permissions_argv("org.gnome.Calculator"),
        override_argv(
            "org.gnome.Calculator",
            Grant::Files {
                reach: Reach::Nothing,
            },
        ),
        override_argv("org.gnome.Calculator", Grant::UnsetGtkTheme),
        override_argv("org.gnome.Calculator", Grant::Reset),
    ]
}

/// What a Flatpak saved (`~/.var/app/<id>`), once the app itself is gone —
/// the second, danger-coloured answer of "remove for good".
pub(crate) fn erase_data(ctx: &Context, id: &str) {
    if !valid_id(id)
        || installed(ctx)
            .iter()
            .any(|i| i.key == AppKey::Flatpak { id: id.to_owned() })
    {
        return;
    }
    let dir = ctx.roots.home.join(".var/app").join(id);
    if let Err(e) = std::fs::remove_dir_all(&dir)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, dir = %dir.display(), "what the app saved was not erased");
    }
}

/// Whether `id` is an application id this crate will put on a command line.
///
/// Three or more dot-separated parts of ASCII letters, digits, `_` and `-`,
/// at most 255 bytes, and the FIRST character a letter or `_`: an id that
/// could start with `-` would be an option, and one with `/` a path.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 255
        && parts.len() >= 3
        && id
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

// ─── what is installed ───────────────────────────────────────────────────────

/// One row of [`list_argv`]'s answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    /// The application id.
    pub id: String,
    /// Its name (from its appstream data, untranslated: `LC_ALL=C.UTF-8`).
    pub name: String,
    /// Its version, often empty.
    pub version: String,
    /// Its branch (`stable`, `beta`).
    pub branch: String,
    /// The remote it came from.
    pub origin: String,
    /// The installation's name — `user`, always, when asked with `--user`.
    pub installation: String,
    /// The installed size, as flatpak formats it (`5.0 MB`).
    pub size: String,
    /// The deployed commit, first twelve characters.
    pub active: String,
}

/// The rows of [`list_argv`]'s plain output: one per line, tab-separated, no
/// title (a title is only printed on a terminal — `flatpak-table-printer.c`).
/// A line with the wrong number of fields is skipped.
#[must_use]
pub fn parse_list(stdout: &str) -> Vec<ListRow> {
    stdout
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let [
                id,
                name,
                version,
                branch,
                origin,
                installation,
                size,
                active,
            ] = f.as_slice()
            else {
                return None;
            };
            valid_id(id).then(|| ListRow {
                id: (*id).to_owned(),
                name: (*name).to_owned(),
                version: (*version).to_owned(),
                branch: (*branch).to_owned(),
                origin: (*origin).to_owned(),
                installation: (*installation).to_owned(),
                size: (*size).to_owned(),
                active: (*active).to_owned(),
            })
        })
        .collect()
}

/// Bytes, from a size as flatpak prints it (`g_format_size`: SI units, one
/// decimal, and a NO-BREAK SPACE before the unit — `812 bytes`, `5.0 MB`,
/// `1.1 GB`; a `< ` in front and a `(partial)` behind in a transaction's
/// table).
///
/// It is ROUNDED to what was printed — three or four significant digits — and
/// that is the precision the screen shows a size in anyway. The exact number
/// is one `flatpak info --show-size` per app away, and a list of thirty apps
/// is not worth thirty processes.
#[must_use]
pub fn bytes_of(size: &str) -> Option<u64> {
    let t = size.trim();
    let t = t.strip_prefix('<').map_or(t, str::trim_start);
    let mut words = t.split_whitespace();
    let number: f64 = words.next()?.replace(',', ".").parse().ok()?;
    let unit: f64 = match words.next()? {
        "byte" | "bytes" => 1.0,
        "kB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        "PB" => 1e15,
        _ => return None,
    };
    (number >= 0.0).then(|| (number * unit).round() as u64)
}

/// The ids nix-flatpak's USER installation declared, from its state file
/// ([`DECLARED_STATE`] under `$XDG_STATE_HOME`). Empty when there is none.
#[must_use]
pub fn declared(roots: &crate::Roots) -> BTreeSet<String> {
    std::fs::read(roots.state.join(DECLARED_STATE))
        .map_or_else(|_| BTreeSet::new(), |b| declared_of(&b))
}

/// The ids in a nix-flatpak state file, in both of its shapes: the current one
/// (`"packages": [{"appId": …}]`, format 1) and the legacy one
/// (`"packages": ["id", …]`), which its own `parse_statefile.jq` still reads.
#[must_use]
pub fn declared_of(bytes: &[u8]) -> BTreeSet<String> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return BTreeSet::new();
    };
    value
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            p.as_str()
                .or_else(|| p.get("appId").and_then(serde_json::Value::as_str))
        })
        .map(str::to_owned)
        .collect()
}

/// What an app exported to the launcher.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exported {
    /// Its desktop ids, `<id>.desktop` first.
    pub desktop_ids: Vec<String>,
    /// The icon its main desktop file names.
    pub icon: Option<Picture>,
}

/// What every app of the user installation exported, by app id.
///
/// An export is a SYMLINK into the app's own deploy directory
/// (`exports/share/applications/X.desktop → …/app/<id>/current/active/export/…`),
/// and the link is what says whose it is: guessing by name would give
/// `org.x.Y.Plugin.desktop` to `org.x.Y`.
#[must_use]
pub fn exports_of(data: &Path) -> BTreeMap<String, Exported> {
    let dir = data.join("flatpak/exports/share/applications");
    let mut out: BTreeMap<String, Exported> = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    let mut files: Vec<(String, String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            let target = std::fs::read_link(e.path()).ok()?;
            let owner = owner_of_export(&target)?;
            name.ends_with(".desktop").then(|| (owner, name, e.path()))
        })
        .collect();
    files.sort();
    for (owner, name, path) in files {
        let main = name == format!("{owner}.desktop");
        let entry = out.entry(owner).or_default();
        if main {
            entry.desktop_ids.insert(0, name);
        } else {
            entry.desktop_ids.push(name);
        }
        if (main || entry.icon.is_none())
            && let Some(icon) = icon_of_desktop(&path)
        {
            entry.icon = Some(icon);
        }
    }
    out
}

/// Which app an export's link points into: the path component after `app`.
fn owner_of_export(target: &Path) -> Option<String> {
    let parts: Vec<&str> = target.iter().filter_map(|p| p.to_str()).collect();
    let at = parts.iter().position(|p| *p == "app")?;
    parts
        .get(at + 1)
        .filter(|id| valid_id(id))
        .map(|id| (*id).to_owned())
}

/// The `Icon=` of a desktop file: a theme name, or a file when absolute.
fn icon_of_desktop(path: &Path) -> Option<Picture> {
    let text = std::fs::read_to_string(path).ok()?;
    let fields = ronoctua_apps::desktop::fields(&text);
    let icon = ronoctua_apps::desktop::raw_bytes(&fields, "Icon")?.trim();
    if icon.is_empty() {
        None
    } else if icon.starts_with('/') {
        Some(Picture::File {
            path: PathBuf::from(icon),
        })
    } else {
        Some(Picture::Named {
            name: icon.to_owned(),
        })
    }
}

/// The installed list, from its parts. Pure: [`installed`] reads the parts.
///
/// One row per id — an app installed from two branches (`stable` and `beta`)
/// is one program to a person, and the cursor keeps the id; `stable` wins.
#[must_use]
pub fn installed_of(
    rows: &[ListRow],
    declared: &BTreeSet<String>,
    updates: &Updates,
    exports: &BTreeMap<String, Exported>,
) -> Vec<Installed> {
    let mut chosen: BTreeMap<&str, &ListRow> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.installation == "user") {
        match chosen.get(row.id.as_str()) {
            Some(kept) if kept.branch == "stable" || row.branch != "stable" => {}
            _ => {
                chosen.insert(&row.id, row);
            }
        }
    }
    chosen
        .into_values()
        .map(|row| {
            let exported = exports.get(&row.id).cloned().unwrap_or_default();
            let desktop_ids = if exported.desktop_ids.is_empty() {
                vec![format!("{}.desktop", row.id)]
            } else {
                exported.desktop_ids
            };
            Installed {
                key: AppKey::Flatpak { id: row.id.clone() },
                name: if row.name.is_empty() {
                    row.id.clone()
                } else {
                    row.name.clone()
                },
                version: row.version.clone(),
                desktop_ids,
                size_bytes: bytes_of(&row.size),
                update: updates.for_installed(&row.id, &row.active),
                unfree: false,
                origin: if declared.contains(&row.id) {
                    Origin::Declared
                } else {
                    Origin::Mine
                },
                icon: exported.icon,
                trouble: None,
            }
        })
        .collect()
}

/// The remote an installed app came from (`flathub`), from the user
/// installation's list; `None` when it is not installed or the list fails.
/// Runs `flatpak list`: call it on a thread.
#[must_use]
pub fn origin_of(ctx: &Context, id: &str) -> Option<String> {
    let program = ctx.tools.flatpak.as_deref()?;
    let ran = plain(Call::new(program, list_argv()))
        .keep_stdout()
        .quietly(ctx)
        .ok()
        .filter(|r| r.code == Some(0))?;
    parse_list(&ran.stdout)
        .into_iter()
        .find(|r| r.id == id && r.installation == "user" && !r.origin.is_empty())
        .map(|r| r.origin)
}

/// The applications in the user installation, and whether nix-flatpak's user
/// installation declared them ([`Origin::Declared`]). Runs `flatpak list`:
/// call it on a thread. Empty when there is no Flatpak or the list fails
/// (logged).
#[must_use]
pub fn installed(ctx: &Context) -> Vec<Installed> {
    let Some(program) = ctx.tools.flatpak.as_deref() else {
        return Vec::new();
    };
    let ran = match plain(Call::new(program, list_argv()))
        .keep_stdout()
        .quietly(ctx)
    {
        Ok(ran) if ran.code == Some(0) => ran,
        Ok(ran) => {
            tracing::warn!(tail = %ran.tail.join(" | "), "flatpak list failed");
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!(%e, "flatpak list could not run");
            return Vec::new();
        }
    };
    installed_of(
        &parse_list(&ran.stdout),
        &declared(&ctx.roots),
        &read_updates(&ctx.roots),
        &exports_of(&ctx.roots.data),
    )
}

// ─── what can be updated ─────────────────────────────────────────────────────

/// The cached answer to "what can be updated", written by a refresh and read
/// by [`installed`]. Asking the remote is a network round trip, so the screen
/// never asks: the refresh step and the queue do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Updates {
    /// [`UPDATES_FORMAT`] when written by this crate.
    pub format: u32,
    /// When the remote was asked, seconds since the epoch.
    pub when: u64,
    /// Per app id, what the remote has.
    pub updates: BTreeMap<String, Update>,
}

/// One update the remote offers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Update {
    /// The version it would bring (empty when its appstream says none).
    pub version: String,
    /// The remote's commit, first twelve characters.
    pub commit: String,
}

impl Updates {
    /// The update an installed app has, given the commit it runs.
    ///
    /// A cache is stale the moment someone updates from a terminal; the
    /// deployed commit equal to the cached one is what says so, and then there
    /// is no update. `Some("")` is an update whose version nobody named.
    #[must_use]
    pub fn for_installed(&self, id: &str, active: &str) -> Option<String> {
        let u = self.updates.get(id)?;
        let same = !u.commit.is_empty() && !active.is_empty() && u.commit == active;
        (!same).then(|| u.version.clone())
    }
}

/// The rows of [`updates_argv`]'s plain output.
#[must_use]
pub fn parse_updates(stdout: &str) -> BTreeMap<String, Update> {
    stdout
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let [id, commit, version] = f.as_slice() else {
                return None;
            };
            valid_id(id).then(|| {
                (
                    (*id).to_owned(),
                    Update {
                        version: (*version).to_owned(),
                        commit: (*commit).to_owned(),
                    },
                )
            })
        })
        .collect()
}

/// The cached updates, or none.
#[must_use]
pub fn read_updates(roots: &crate::Roots) -> Updates {
    std::fs::read(roots.ours_cache().join(UPDATES_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Updates>(&b).ok())
        .filter(|u| u.format == UPDATES_FORMAT)
        .unwrap_or_default()
}

fn write_updates(roots: &crate::Roots, updates: &Updates) -> std::io::Result<()> {
    let dir = roots.ours_cache();
    std::fs::create_dir_all(&dir)?;
    let text = serde_json::to_vec_pretty(updates).unwrap_or_default();
    ronoctua_xdg::write_atomically(&dir, UPDATES_FILE, &text)
}

/// Take ids out of the cache (after they were upgraded or removed, when a new
/// answer could not be had); `None` empties it.
fn forget_updates(roots: &crate::Roots, id: Option<&str>) {
    let mut cached = read_updates(roots);
    if cached.updates.is_empty() {
        return;
    }
    match id {
        Some(id) => {
            cached.updates.remove(id);
        }
        None => cached.updates.clear(),
    }
    if let Err(e) = write_updates(roots, &cached) {
        tracing::warn!(%e, "could not rewrite the flatpak update cache");
    }
}

// ─── permissions ─────────────────────────────────────────────────────────────

/// The permissions of an installed app, as `flatpak info --show-permissions`
/// gives them (a key file, overrides applied), for the advanced sheet. Runs
/// a program: call it on a thread.
#[must_use]
pub fn permissions(ctx: &Context, id: &str) -> Option<String> {
    let program = ctx.tools.flatpak.as_deref()?;
    if !valid_id(id) {
        return None;
    }
    let ran = plain(Call::new(program, permissions_argv(id)))
        .keep_stdout()
        .quietly(ctx)
        .ok()?;
    (ran.code == Some(0)).then_some(ran.stdout)
}

/// One permission as the flag `flatpak override` takes for it — the words
/// Flatseal and the documentation use: `share=network`, `socket=wayland`,
/// `filesystem=xdg-download:ro`, and the negations (`!home` in the metadata is
/// `nofilesystem=home`). A key this table does not know stays `key=value`.
#[must_use]
pub fn permission_line(key: &str, value: &str) -> String {
    let (negated, value) = value
        .strip_prefix('!')
        .map_or((false, value), |v| (true, v));
    let flag = match (key, negated) {
        ("shared", false) => "share",
        ("shared", true) => "unshare",
        ("sockets", false) => "socket",
        ("sockets", true) => "nosocket",
        ("devices", false) => "device",
        ("devices", true) => "nodevice",
        ("features", false) => "allow",
        ("features", true) => "disallow",
        ("filesystems", false) => "filesystem",
        ("filesystems", true) => "nofilesystem",
        ("persistent", _) => "persist",
        ("unset-environment", _) => "unset-env",
        (other, _) => return format!("{other}={}{value}", if negated { "!" } else { "" }),
    };
    format!("{flag}={value}")
}

/// A bus name's policy as a line: `talk-name=org.x`, `own-name=…`,
/// `no-talk-name=…` (policy `none`), and `see-name=…` — the one with no flag of
/// its own, written the same way. `system` prefixes `system-`.
#[must_use]
pub fn bus_line(system: bool, name: &str, policy: &str) -> String {
    let what = match policy {
        "own" => "own-name",
        "see" => "see-name",
        "none" => "no-talk-name",
        _ => "talk-name",
    };
    format!("{}{what}={name}", if system { "system-" } else { "" })
}

/// The lines of a `--show-permissions` key file: `[Context]` keys through
/// [`permission_line`], the two bus policies through [`bus_line`], and
/// `[Environment]` as `env=NAME=value`.
#[must_use]
pub fn permission_lines(keyfile: &str) -> Vec<String> {
    let mut group = "";
    let mut out = Vec::new();
    for line in keyfile.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(g) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            group = g;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match group {
            "Context" => out.extend(
                value
                    .split(';')
                    .filter(|v| !v.is_empty())
                    .map(|v| permission_line(key, v)),
            ),
            "Session Bus Policy" => out.push(bus_line(false, key, value)),
            "System Bus Policy" => out.push(bus_line(true, key, value)),
            "Environment" => out.push(format!("env={key}={value}")),
            other => out.push(format!(
                "{}:{key}={value}",
                other.to_ascii_lowercase().replace(' ', "-")
            )),
        }
    }
    out
}

/// Whether the permissions force `GTK_THEME` into the sandbox — the one line
/// the advanced sheet offers to REMOVE, never to add (see the module's head).
#[must_use]
pub fn forces_gtk_theme(lines: &[String]) -> bool {
    lines.iter().any(|l| l.starts_with("env=GTK_THEME="))
}

// ─── a transaction's progress ────────────────────────────────────────────────

/// What a transaction does to one ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    /// `i` in the table.
    Install,
    /// `u`.
    Update,
    /// `r` (remove).
    Uninstall,
}

/// One operation of a transaction, from its table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    /// The ref's id (`org.gnome.Platform.Locale`).
    pub id: String,
    /// What happens to it.
    pub kind: OpKind,
}

/// One progress reading, ready for [`Report::step`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What is happening.
    pub phase: Phase,
    /// Thousandths of the whole transaction behind.
    pub done: u64,
    /// Always 1000: flatpak's plain output gives a PERCENTAGE per operation
    /// and never a byte count, so the bar is a fraction and says so.
    pub total: u64,
    /// The ref being processed (empty when the table did not name it).
    pub now: String,
}

/// Reads a transaction's plain output line by line
/// (`flatpak-cli-transaction.c`, not a terminal):
///
/// ```text
///  1.<TAB>   <TAB>org.gnome.Platform<TAB>51<TAB>i<TAB>flathub<TAB>< 402.3 MB
///  2.<TAB>   <TAB>org.gnome.Calculator<TAB>stable<TAB>i<TAB>flathub<TAB>< 1.9 MB
/// Installing 1/2…
/// Installing 1/2… ████▌                22%  3.1 MB/s  01:40
/// Installing 2/2…
/// Installation complete.
/// ```
///
/// The table (printed once, before the operations) names each operation's
/// ref; `Installing|Updating|Uninstalling`, an optional `N/M` and the ellipsis
/// start every operation line, and a progress tick ends in ` NN%`. The first
/// operation of a lone one has no counter (`Installing…`).
#[derive(Debug, Clone, Default)]
pub struct Watch {
    ops: Vec<Op>,
}

impl Watch {
    /// The operations the table listed, in order.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// Read one line; a progress reading when it was one.
    pub fn feed(&mut self, line: &str) -> Option<Step> {
        if let Some((n, op)) = table_row(line) {
            if n == self.ops.len() + 1 {
                self.ops.push(op);
            }
            return None;
        }
        let (kind, op, of, percent) = operation_line(line)?;
        let percent = u64::from(percent.unwrap_or(0));
        let done = ((op as u64 - 1) * 100 + percent) * 1000 / (of as u64 * 100);
        let phase = match (kind, percent) {
            (OpKind::Uninstall, _) => Phase::Removing,
            (_, p) if p > 0 && p < 100 => Phase::Downloading,
            _ => Phase::Installing,
        };
        let now = self
            .ops
            .get(op - 1)
            .map(|o| o.id.clone())
            .unwrap_or_default();
        Some(Step {
            phase,
            done,
            total: 1000,
            now,
        })
    }

    /// How many operations of one kind the table listed.
    #[must_use]
    pub fn count(&self, kind: OpKind) -> u64 {
        self.ops.iter().filter(|o| o.kind == kind).count() as u64
    }
}

/// A row of the operations table: `" 1."`, a blank, the id, then the arch
/// (only when it is not this machine's), the branch, the operation letter and,
/// for what downloads, the remote and the size.
fn table_row(line: &str) -> Option<(usize, Op)> {
    let fields: Vec<&str> = line.split('\t').collect();
    let n: usize = fields.first()?.trim().strip_suffix('.')?.parse().ok()?;
    let id = fields.get(2)?.trim();
    if !valid_id(id) {
        return None;
    }
    let kind = fields.iter().skip(3).find_map(|f| match f.trim() {
        "i" => Some(OpKind::Install),
        "u" => Some(OpKind::Update),
        "r" => Some(OpKind::Uninstall),
        _ => None,
    })?;
    Some((
        n,
        Op {
            id: id.to_owned(),
            kind,
        },
    ))
}

/// `Installing 2/3… ██▌   45%  1.2 MB/s` → (install, 2, 3, Some(45)).
fn operation_line(line: &str) -> Option<(OpKind, usize, usize, Option<u8>)> {
    let (kind, rest) = [
        ("Uninstalling", OpKind::Uninstall),
        ("Installing", OpKind::Install),
        ("Updating", OpKind::Update),
    ]
    .into_iter()
    .find_map(|(verb, kind)| line.strip_prefix(verb).map(|rest| (kind, rest)))?;
    // The ellipsis is U+2026 (untranslated: `LC_ALL=C.UTF-8`). Its absence is
    // what tells `Updating appstream data for user remote flathub` apart.
    let (counter, after) = rest.split_once('…').or_else(|| rest.split_once("..."))?;
    let counter = counter.trim();
    let (op, of) = if counter.is_empty() {
        (1, 1)
    } else {
        let (a, b) = counter.split_once('/')?;
        (a.parse().ok()?, b.parse().ok()?)
    };
    if op == 0 || of == 0 || op > of {
        return None;
    }
    let percent = after.rfind('%').and_then(|at| {
        let digits: String = after[..at]
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        digits.parse::<u8>().ok().map(|p| p.min(100))
    });
    Some((kind, op, of, percent))
}

/// The closed reason a failed flatpak gave, from its last lines.
#[must_use]
pub fn refusal_of(tail: &[String]) -> Refusal {
    let text = tail.join("\n");
    let lower = text.to_ascii_lowercase();
    let offline = [
        "could not resolve",
        "couldn't resolve",
        "could not connect",
        "couldn't connect",
        "network is unreachable",
        "timeout was reached",
        "unable to load summary",
    ];
    if offline.iter().any(|s| lower.contains(s)) {
        Refusal::Offline
    } else if lower.contains("remote \"") && lower.contains("not found") {
        Refusal::SourceUnprepared
    } else if [
        "nothing matches",
        "no remote refs found",
        "no such ref",
        "not installed",
    ]
    .iter()
    .any(|s| lower.contains(s))
    {
        Refusal::UnknownItem
    } else {
        Refusal::ToolFailed
    }
}

fn failure_of(ran: &Ran) -> Failure {
    Failure {
        refusal: Some(refusal_of(&ran.tail)),
        trouble: ran.tail.join("\n"),
    }
}

/// A call in flatpak's plain output format, whatever started the job.
fn plain(call: Call<'_>) -> Call<'_> {
    call.env("FLATPAK_FANCY_OUTPUT", "0")
}

/// What a transaction left: its table, and whether it only rehearsed.
struct Tally {
    watch: Watch,
    rehearsed: bool,
}

/// Run one transaction, reporting its progress, failing with the reason flatpak
/// gave. `fallback` names the job when the table did not.
fn transact(
    ctx: &Context,
    program: &Path,
    args: Vec<String>,
    report: &mut dyn Report,
    fallback: &str,
) -> Result<Tally, Failure> {
    let mut watch = Watch::default();
    // flatpak prints a tick every 300 ms whether or not it moved; only a
    // change becomes a beat, so the queue is not woken for nothing.
    let mut last: Option<Step> = None;
    let ran = plain(Call::new(program, args))
        .changing()
        .run(ctx, &mut |line| {
            if let Some(step) = watch.feed(line).filter(|s| last.as_ref() != Some(s)) {
                let now = if step.now.is_empty() {
                    fallback
                } else {
                    step.now.as_str()
                };
                report.step(step.phase, step.done, step.total, now);
                last = Some(step);
            }
        })?;
    if ran.rehearsed {
        return Ok(Tally {
            watch,
            rehearsed: true,
        });
    }
    if ran.code != Some(0) {
        return Err(failure_of(&ran));
    }
    Ok(Tally {
        watch,
        rehearsed: false,
    })
}

/// Refuse unless the user installation has [`REMOTE`].
fn ensure_remote(ctx: &Context, program: &Path) -> Result<(), Failure> {
    let ran = plain(Call::new(program, remotes_argv()))
        .keep_stdout()
        .quietly(ctx)?;
    if ran.code != Some(0) {
        return Err(failure_of(&ran));
    }
    if ran.stdout.lines().any(|l| l.trim() == REMOTE) {
        Ok(())
    } else {
        Err(Failure::refused(Refusal::SourceUnprepared))
    }
}

/// The app id an order names, or why it is not one this lane can act on.
fn flatpak_id(key: &AppKey) -> Result<&str, Failure> {
    match key {
        AppKey::Flatpak { id } if valid_id(id) => Ok(id),
        AppKey::System { .. } => Err(Failure::refused(Refusal::DeclaredInTheFlake)),
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

fn desktop_ids_after(ctx: &Context, id: &str) -> Vec<String> {
    exports_of(&ctx.roots.data)
        .remove(id)
        .map(|e| e.desktop_ids)
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| vec![format!("{id}.desktop")])
}

// ─── running an order ────────────────────────────────────────────────────────

/// Run one order whose lane is Flatpak: `Install`, `InstallFile`
/// (`.flatpakref`), `Remove`, `Upgrade`, `Tidy`, `Prepare`, `Refresh`.
///
/// The lane itself (`crate::lock`) is the caller's to take. Runs programs and
/// reaches the network: never on a screen's thread.
///
/// # Errors
///
/// [`Refusal::SourceAbsent`] without Flatpak, [`Refusal::SourceUnprepared`]
/// without the Flathub remote, [`Refusal::DeclaredInTheFlake`] for removing
/// what nix-flatpak declared, [`Refusal::UnverifiedNeedsConsent`] for a
/// `.flatpakref` nobody said yes to, [`Refusal::Offline`], and
/// [`Refusal::ToolFailed`] with flatpak's own words.
pub fn run(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    let program = run::tool(ctx.tools.flatpak.as_ref())?;
    match order {
        Order::Install { key, .. } => install(ctx, program, flatpak_id(key)?, report),
        Order::InstallFile { path, consent } => install_file(ctx, program, path, *consent, report),
        Order::Remove { key, keep_data } => remove(ctx, program, key, *keep_data, report),
        Order::Upgrade { scope } => upgrade(ctx, program, scope, report),
        Order::Tidy {
            source: Source::Flatpak,
        } => tidy(ctx, program, report),
        Order::Prepare {
            source: Source::Flatpak,
            ..
        } => prepare(ctx, program, report),
        Order::Refresh {
            source: Source::Flatpak,
        } => {
            let updates = refresh_with(ctx, program, report)?;
            Ok(Outcome {
                amount: Some(updates.updates.len() as u64),
                ..Outcome::default()
            })
        }
        Order::Permission { id, grant } => permit(ctx, program, id, *grant, report),
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

/// Change one thing an INSTALLED app may reach. The override file is the
/// person's (`~/.local/share/flatpak/overrides/<id>`); a declared app's
/// overrides are the person's too, so it is not refused.
fn permit(ctx: &Context, program: &Path, id: &str, grant: Grant, report: &mut dyn Report) -> Done {
    if !valid_id(id) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    report.step(Phase::Integrating, 0, 0, id);
    let ran = plain(Call::new(program, override_argv(id, grant)))
        .changing()
        .quietly(ctx)?;
    if !ran.rehearsed && ran.code != Some(0) {
        return Err(failure_of(&ran));
    }
    Ok(Outcome {
        key: Some(AppKey::Flatpak { id: id.to_owned() }),
        rehearsed: ran.rehearsed,
        ..Outcome::default()
    })
}

fn install(ctx: &Context, program: &Path, id: &str, report: &mut dyn Report) -> Done {
    report.step(Phase::Resolving, 0, 0, id);
    ensure_remote(ctx, program)?;
    let tally = transact(ctx, program, install_argv(id), report, id)?;
    if !tally.rehearsed {
        forget_updates(&ctx.roots, Some(id));
    }
    Ok(Outcome {
        key: Some(AppKey::Flatpak { id: id.to_owned() }),
        desktop_ids: desktop_ids_after(ctx, id),
        amount: None,
        rehearsed: tally.rehearsed,
    })
}

/// What a `.flatpakref` says (the `[Flatpak Ref]` group).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlatpakRef {
    /// `Name=`, the app id.
    pub name: String,
    /// `Branch=`.
    pub branch: String,
    /// `Url=`, the repository.
    pub url: String,
    /// `Title=`.
    pub title: String,
    /// `IsRuntime=true`.
    pub is_runtime: bool,
}

/// The largest `.flatpakref` this crate reads: a real one is a few kilobytes
/// (mostly the base64 GPG key).
pub const FLATPAKREF_MOST: u64 = 64 * 1024;

/// Read a `.flatpakref`'s text. `None` when it is not one: no `[Flatpak Ref]`
/// group, or a `Name` that is not a valid id.
#[must_use]
pub fn flatpakref_of(text: &str) -> Option<FlatpakRef> {
    let mut inside = false;
    let mut seen = false;
    let mut out = FlatpakRef::default();
    for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == "[Flatpak Ref]";
            seen |= inside;
            continue;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| inside) else {
            continue;
        };
        let value = value.trim().to_owned();
        match key.trim() {
            "Name" => out.name = value,
            "Branch" => out.branch = value,
            "Url" => out.url = value,
            "Title" => out.title = value,
            "IsRuntime" => out.is_runtime = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    (seen && valid_id(&out.name)).then_some(out)
}

fn install_file(
    ctx: &Context,
    program: &Path,
    path: &Path,
    consent: Consent,
    report: &mut dyn Report,
) -> Done {
    // Nothing vouches for a file from the disk: it names its own repository and
    // brings its own key. The question already said so, or it was not asked.
    if !consent.unverified {
        return Err(Failure::refused(Refusal::UnverifiedNeedsConsent));
    }
    let is_ref = path.is_absolute()
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.to_ascii_lowercase().ends_with(".flatpakref"));
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(u64::MAX);
    if !is_ref || size > FLATPAKREF_MOST {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let head = run::head_of(path, FLATPAKREF_MOST as usize)
        .map_err(|e| Failure::trouble(e.to_string()))?;
    let reference = flatpakref_of(&String::from_utf8_lossy(&head))
        .filter(|r| !r.is_runtime)
        .ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
    report.step(Phase::Resolving, 0, 0, &reference.name);
    let tally = transact(
        ctx,
        program,
        install_file_argv(path),
        report,
        &reference.name,
    )?;
    Ok(Outcome {
        desktop_ids: desktop_ids_after(ctx, &reference.name),
        key: Some(AppKey::Flatpak { id: reference.name }),
        amount: None,
        rehearsed: tally.rehearsed,
    })
}

fn remove(
    ctx: &Context,
    program: &Path,
    key: &AppKey,
    keep_data: bool,
    report: &mut dyn Report,
) -> Done {
    let id = flatpak_id(key)?;
    // Removed here, nix-flatpak would install it again at the next activation:
    // only the flake can take it away.
    if declared(&ctx.roots).contains(id) {
        return Err(Failure::refused(Refusal::DeclaredInTheFlake));
    }
    report.step(Phase::Removing, 0, 0, id);
    let tally = transact(ctx, program, uninstall_argv(id, keep_data), report, id)?;
    if !tally.rehearsed {
        forget_updates(&ctx.roots, Some(id));
    }
    Ok(Outcome {
        key: Some(key.clone()),
        rehearsed: tally.rehearsed,
        ..Outcome::default()
    })
}

fn upgrade(ctx: &Context, program: &Path, scope: &Scope, report: &mut dyn Report) -> Done {
    let (key, id) = match scope {
        Scope::One { key } => (Some(key.clone()), Some(flatpak_id(key)?)),
        Scope::Source {
            source: Source::Flatpak,
        }
        | Scope::Everything => (None, None),
        Scope::Source { .. } => return Err(Failure::refused(Refusal::UnknownItem)),
    };
    report.step(Phase::Resolving, 0, 0, id.unwrap_or(REMOTE));
    let tally = transact(ctx, program, update_argv(id), report, id.unwrap_or(REMOTE))?;
    if !tally.rehearsed {
        // The catalogue and the update list, fresh; when that cannot be had
        // (offline, no remote), at least the list stops naming what was just
        // upgraded.
        if refresh_with(ctx, program, report).is_err() {
            forget_updates(&ctx.roots, id);
        }
    }
    Ok(Outcome {
        key,
        desktop_ids: Vec::new(),
        amount: (!tally.rehearsed).then(|| tally.watch.count(OpKind::Update)),
        rehearsed: tally.rehearsed,
    })
}

/// Every ref of the user installation and its size, or `None` when the list
/// could not be had.
fn sizes(ctx: &Context, program: &Path) -> Option<BTreeMap<String, u64>> {
    let ran = plain(Call::new(program, refs_argv()))
        .keep_stdout()
        .quietly(ctx)
        .ok()?;
    (ran.code == Some(0)).then(|| parse_refs(&ran.stdout))
}

/// The rows of [`refs_argv`]'s output: partial ref (`org.x.Y/x86_64/stable`,
/// no `app/` or `runtime/`) → bytes.
#[must_use]
pub fn parse_refs(stdout: &str) -> BTreeMap<String, u64> {
    stdout
        .lines()
        .filter_map(|l| {
            let (reference, size) = l.split_once('\t')?;
            Some((reference.to_owned(), bytes_of(size)?))
        })
        .collect()
}

/// What a tidy freed: the sizes of the refs present before and gone after.
/// Refs that arrived meanwhile do not subtract.
#[must_use]
pub fn freed(before: &BTreeMap<String, u64>, after: &BTreeMap<String, u64>) -> u64 {
    before
        .iter()
        .filter(|(r, _)| !after.contains_key(*r))
        .map(|(_, b)| b)
        .sum()
}

fn tidy(ctx: &Context, program: &Path, report: &mut dyn Report) -> Done {
    report.step(Phase::Removing, 0, 0, REMOTE);
    let before = sizes(ctx, program);
    let tally = transact(ctx, program, tidy_argv(), report, REMOTE)?;
    let amount = if tally.rehearsed {
        None
    } else {
        before.zip(sizes(ctx, program)).map(|(b, a)| freed(&b, &a))
    };
    Ok(Outcome {
        amount,
        rehearsed: tally.rehearsed,
        ..Outcome::default()
    })
}

fn prepare(ctx: &Context, program: &Path, report: &mut dyn Report) -> Done {
    report.step(Phase::Preparing, 0, 0, REMOTE);
    let ran = plain(Call::new(program, remote_add_argv()))
        .changing()
        .quietly(ctx)?;
    if ran.rehearsed {
        return Ok(Outcome {
            rehearsed: true,
            ..Outcome::default()
        });
    }
    if ran.code != Some(0) {
        return Err(failure_of(&ran));
    }
    let updates = refresh_with(ctx, program, report)?;
    Ok(Outcome {
        amount: Some(updates.updates.len() as u64),
        ..Outcome::default()
    })
}

/// Fetch the catalogue, ask what can be updated, and write the cache
/// [`installed`] reads — then rebuild the offline search's index while the job
/// is still running, so the first search after it does not pay for it.
///
/// It CHANGES nothing a person has (the catalogue is a cache), so it runs in a
/// rehearsal too. Runs programs and reaches the network: call it on a thread.
///
/// # Errors
///
/// [`Refusal::SourceAbsent`] without Flatpak, [`Refusal::SourceUnprepared`]
/// without the remote, [`Refusal::Offline`], or flatpak's words.
pub fn refresh(ctx: &Context, report: &mut dyn Report) -> Result<Updates, Failure> {
    refresh_with(ctx, run::tool(ctx.tools.flatpak.as_ref())?, report)
}

fn refresh_with(
    ctx: &Context,
    program: &Path,
    report: &mut dyn Report,
) -> Result<Updates, Failure> {
    ensure_remote(ctx, program)?;
    report.step(Phase::Downloading, 0, 0, REMOTE);
    let ran = plain(Call::new(program, appstream_argv())).quietly(ctx)?;
    if ran.code != Some(0) {
        return Err(failure_of(&ran));
    }
    report.step(Phase::Indexing, 0, 0, REMOTE);
    let ran = plain(Call::new(program, updates_argv()))
        .keep_stdout()
        .quietly(ctx)?;
    if ran.code != Some(0) {
        return Err(failure_of(&ran));
    }
    let updates = Updates {
        format: UPDATES_FORMAT,
        when: now(),
        updates: parse_updates(&ran.stdout),
    };
    write_updates(&ctx.roots, &updates)
        .map_err(|e| Failure::trouble(format!("could not write the update cache: {e}")))?;
    if let Err(e) = crate::appstream::rebuild(&ctx.roots) {
        tracing::warn!(%e, "the offline catalogue could not be indexed");
    }
    Ok(updates)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::progress::Beat;

    const LIST: &str = include_str!("../tests/fixtures/flatpak/list.tsv");
    const INSTALL: &str = include_str!("../tests/fixtures/flatpak/install.txt");
    const UPDATE: &str = include_str!("../tests/fixtures/flatpak/update.txt");
    const PERMISSIONS: &str = include_str!("../tests/fixtures/flatpak/permissions.ini");

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-flatpak-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A fake `flatpak`: logs its argument vector (one line) and answers what
    /// `body` says, by its first argument (`$1`).
    fn fake(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("flatpak");
        let log = dir.join("argv.log");
        let script = format!("#!/bin/sh\necho \"$*\" >> '{}'\n{body}\n", log.display());
        crate::write_executable(&path, &script);
        path
    }

    fn ctx_with(dir: &Path, program: Option<PathBuf>, rehearse: bool) -> Context {
        Context {
            tools: crate::Tools {
                flatpak: program,
                ..crate::Tools::default()
            },
            roots: crate::Roots::under(dir),
            prefs: crate::prefs::Preferences::default(),
            rehearse,
        }
    }

    fn logged(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("argv.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn software_stays_in_the_home() {
        for argv in every_argv() {
            let line = argv.join(" ");
            assert!(argv.iter().any(|a| a == "--user"), "no --user: {line}");
            for never in ["--system", "--installation", "sudo", "pkexec"] {
                assert!(
                    !argv.iter().any(|a| a.contains(never)),
                    "{never} in: {line}"
                );
            }
        }
    }

    #[test]
    fn a_transaction_keeps_the_output_that_has_percentages() {
        // `--noninteractive` would swap in the quiet transaction, which prints
        // no percentage at all (see the module's head).
        for argv in every_argv() {
            assert!(
                !argv.iter().any(|a| a == "--noninteractive"),
                "{}",
                argv.join(" ")
            );
        }
        for argv in [
            install_argv("org.x.Y"),
            uninstall_argv("org.x.Y", true),
            update_argv(None),
            tidy_argv(),
        ] {
            assert!(argv.iter().any(|a| a == "-y"), "{}", argv.join(" "));
        }
    }

    #[test]
    fn the_double_dash_comes_before_every_positional_argument() {
        assert_eq!(
            install_argv("org.gnome.Calculator"),
            [
                "install",
                "--user",
                "-y",
                "--",
                "flathub",
                "org.gnome.Calculator"
            ]
        );
        assert_eq!(
            uninstall_argv("org.x.Y", false),
            [
                "uninstall",
                "--user",
                "-y",
                "--delete-data",
                "--",
                "org.x.Y"
            ]
        );
        assert_eq!(
            uninstall_argv("org.x.Y", true),
            ["uninstall", "--user", "-y", "--", "org.x.Y"]
        );
        assert_eq!(update_argv(None), ["update", "--user", "-y"]);
        assert_eq!(
            update_argv(Some("org.x.Y")),
            ["update", "--user", "-y", "--", "org.x.Y"]
        );
        assert_eq!(
            install_file_argv(Path::new("/d/a b.flatpakref")),
            [
                "install",
                "--user",
                "-y",
                "--from",
                "--",
                "/d/a b.flatpakref"
            ]
        );
        assert_eq!(tidy_argv(), ["uninstall", "--user", "-y", "--unused"]);
        assert_eq!(
            remote_add_argv(),
            [
                "remote-add",
                "--user",
                "--if-not-exists",
                "--",
                "flathub",
                REMOTE_URL
            ]
        );
        assert_eq!(
            appstream_argv(),
            ["update", "--user", "--appstream", "--", "flathub"]
        );
        assert_eq!(
            permissions_argv("org.x.Y"),
            ["info", "--user", "--show-permissions", "--", "org.x.Y"]
        );
        assert_eq!(
            list_argv().last().unwrap(),
            &format!("--columns={LIST_COLUMNS}")
        );
        assert_eq!(
            updates_argv().last().unwrap(),
            &format!("--columns={UPDATE_COLUMNS}")
        );
        assert_eq!(
            refs_argv(),
            ["list", "--user", "--all", "--columns=ref,size"]
        );
        assert_eq!(remotes_argv(), ["remotes", "--user", "--columns=name"]);
    }

    #[test]
    fn an_id_is_never_an_option_nor_a_path() {
        for good in [
            "org.gnome.Calculator",
            "com.github.nihui.waifu2x-ncnn-vulkan",
            "net._86box._86Box",
            "io.github.x_y.Z",
        ] {
            assert!(valid_id(good), "{good}");
        }
        for bad in [
            "-org.x.Y",
            "org.x",
            "org..Y",
            "org/x/Y",
            "../x.y.z",
            "org.x.Y ",
            "",
            "org.x.Y//stable",
            "1org.x.Y",
        ] {
            assert!(!valid_id(bad), "{bad:?}");
        }
        assert!(!valid_id(&format!("a.b.{}", "c".repeat(260))));
    }

    #[test]
    fn a_size_comes_back_from_how_flatpak_printed_it() {
        // GLib puts a NO-BREAK SPACE between the number and the unit (seen
        // in the host's own `flatpak list`); `split_whitespace` takes it.
        assert_eq!(bytes_of("3.3\u{a0}MB"), Some(3_300_000));
        assert_eq!(bytes_of("812 bytes"), Some(812));
        assert_eq!(bytes_of("1 byte"), Some(1));
        assert_eq!(bytes_of("5.0 MB"), Some(5_000_000));
        assert_eq!(bytes_of("138.9 MB"), Some(138_900_000));
        assert_eq!(bytes_of("1.1 GB"), Some(1_100_000_000));
        assert_eq!(bytes_of("1.0 kB"), Some(1000));
        assert_eq!(bytes_of("< 386.1 MB (partial)"), Some(386_100_000));
        assert_eq!(bytes_of("big"), None);
        assert_eq!(bytes_of("3 MiB"), None);
    }

    #[test]
    fn the_list_is_read_in_the_order_it_was_asked() {
        let rows = parse_list(LIST);
        assert_eq!(rows.len(), 4, "the malformed line is skipped");
        assert_eq!(rows[0].id, "org.gnome.Calculator");
        assert_eq!(rows[0].name, "Calculator");
        assert_eq!(rows[0].version, "51.0");
        assert_eq!(rows[0].size, "5.0\u{a0}MB");
        assert_eq!(rows[0].active, "3a1b2c3d4e5f");
        assert_eq!(rows[2].version, "", "an empty column stays empty");
    }

    #[test]
    fn declared_apps_are_read_from_both_state_shapes() {
        let v1 = br#"{"version":1,"packages":[{"appId":"org.mozilla.firefox","origin":"flathub","flatpakref":null,"commit":null,"bundle":null,"sha256":null},{"appId":"org.gtk.Gtk3theme.Adwaita-dark","origin":"flathub"}],"overrides":{},"remotes":[{"name":"flathub"}]}"#;
        let legacy = br#"{"packages":["org.gnome.Epiphany"],"overrides":{},"remotes":["flathub"]}"#;
        assert_eq!(
            declared_of(v1),
            BTreeSet::from([
                "org.mozilla.firefox".into(),
                "org.gtk.Gtk3theme.Adwaita-dark".into()
            ])
        );
        assert_eq!(
            declared_of(legacy),
            BTreeSet::from(["org.gnome.Epiphany".into()])
        );
        assert!(declared_of(b"not json").is_empty());
    }

    /// Against the SYSTEM installation of the machine it runs on, read only:
    /// the export links and, when `RONOCTUA_FLATPAK_LIST_SAMPLE` names a file
    /// holding `flatpak list --system --app --columns=<LIST_COLUMNS>`'s output,
    /// that output. `cargo test -p ronoctua-software -- --ignored --nocapture live`.
    #[test]
    #[ignore = "reads /var/lib/flatpak; a measurement, not a check"]
    fn live_system_installation() {
        let exports = exports_of(Path::new("/var/lib"));
        println!(
            "{} apps export desktop files; first: {:?}",
            exports.len(),
            exports.iter().next()
        );
        if let Some(sample) = std::env::var_os("RONOCTUA_FLATPAK_LIST_SAMPLE") {
            let rows = parse_list(&std::fs::read_to_string(sample).unwrap());
            let sizeless = rows.iter().filter(|r| bytes_of(&r.size).is_none()).count();
            println!(
                "{} rows, {sizeless} without a readable size; first: {:?}",
                rows.len(),
                rows.first()
            );
            assert_eq!(sizeless, 0);
            assert!(
                rows.iter().all(|r| exports.contains_key(&r.id)),
                "every app exports its desktop file"
            );
        }
    }

    #[test]
    fn a_stale_update_is_no_update() {
        let updates = Updates {
            format: UPDATES_FORMAT,
            when: 1,
            updates: parse_updates(include_str!("../tests/fixtures/flatpak/updates.tsv")),
        };
        assert_eq!(
            updates.for_installed("org.mozilla.firefox", "0123456789ab"),
            Some("144.0".into())
        );
        // Updated from a terminal since: the deployed commit IS the cached one.
        assert_eq!(
            updates.for_installed("org.gnome.Calculator", "3a1b2c3d4e5f"),
            None
        );
        assert_eq!(
            updates.for_installed("io.github.flattool.Warehouse", "fedcba987654"),
            Some(String::new())
        );
        assert_eq!(updates.for_installed("org.x.Absent", "x"), None);
    }

    #[test]
    fn the_installed_list_is_one_row_per_app_with_what_the_flake_declared() {
        let declared = BTreeSet::from(["org.mozilla.firefox".to_owned()]);
        let updates = Updates {
            format: UPDATES_FORMAT,
            when: 1,
            updates: parse_updates(include_str!("../tests/fixtures/flatpak/updates.tsv")),
        };
        let exports = BTreeMap::from([(
            "org.gnome.Calculator".to_owned(),
            Exported {
                desktop_ids: vec!["org.gnome.Calculator.desktop".into()],
                icon: Some(Picture::Named {
                    name: "org.gnome.Calculator".into(),
                }),
            },
        )]);
        let all = installed_of(&parse_list(LIST), &declared, &updates, &exports);
        assert_eq!(all.len(), 3, "firefox is there twice, from two branches");
        let firefox = all
            .iter()
            .find(|i| i.key.id() == "org.mozilla.firefox")
            .unwrap();
        assert_eq!(firefox.version, "143.0.1", "stable wins over beta");
        assert_eq!(firefox.origin, Origin::Declared);
        assert_eq!(firefox.update.as_deref(), Some("144.0"));
        assert_eq!(firefox.size_bytes, Some(312_400_000));
        assert_eq!(
            firefox.desktop_ids,
            ["org.mozilla.firefox.desktop"],
            "nothing exported: the id's own name"
        );
        let calc = all
            .iter()
            .find(|i| i.key.id() == "org.gnome.Calculator")
            .unwrap();
        assert_eq!(calc.origin, Origin::Mine);
        assert_eq!(calc.update, None);
        assert_eq!(
            calc.icon,
            Some(Picture::Named {
                name: "org.gnome.Calculator".into()
            })
        );
        let warehouse = all
            .iter()
            .find(|i| i.key.id() == "io.github.flattool.Warehouse")
            .unwrap();
        assert_eq!(warehouse.name, "Warehouse");
        assert_eq!(warehouse.size_bytes, Some(812));
    }

    #[test]
    fn an_export_belongs_to_the_app_its_link_points_into() {
        let dir = scratch("exports");
        let data = dir.join("data");
        let apps = data.join("flatpak/exports/share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        let deploy = |id: &str, file: &str, icon: &str| {
            let target = data.join(format!(
                "flatpak/app/{id}/current/active/export/share/applications"
            ));
            std::fs::create_dir_all(&target).unwrap();
            std::fs::write(
                target.join(file),
                format!("[Desktop Entry]\nName=X\nIcon={icon}\nExec=x\n"),
            )
            .unwrap();
            std::os::unix::fs::symlink(
                format!("../../../app/{id}/current/active/export/share/applications/{file}"),
                apps.join(file),
            )
            .unwrap();
        };
        deploy("org.x.Y", "org.x.Y.desktop", "org.x.Y");
        deploy("org.x.Y", "org.x.Y.Second.desktop", "org.x.Y.Second");
        deploy("org.x.Y.Plugin", "org.x.Y.Plugin.desktop", "/opt/icon.png");
        std::fs::write(apps.join("stray.desktop"), "[Desktop Entry]\n").unwrap();
        let exports = exports_of(&data);
        assert_eq!(
            exports["org.x.Y"].desktop_ids,
            ["org.x.Y.desktop", "org.x.Y.Second.desktop"]
        );
        assert_eq!(
            exports["org.x.Y"].icon,
            Some(Picture::Named {
                name: "org.x.Y".into()
            })
        );
        assert_eq!(
            exports["org.x.Y.Plugin"].desktop_ids,
            ["org.x.Y.Plugin.desktop"]
        );
        assert_eq!(
            exports["org.x.Y.Plugin"].icon,
            Some(Picture::File {
                path: "/opt/icon.png".into()
            })
        );
        assert_eq!(exports.len(), 2, "a file that is not a link is nobody's");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_transaction_is_read_as_thousandths_of_the_whole() {
        let mut watch = Watch::default();
        let steps: Vec<Step> = run::lines_of(INSTALL.as_bytes())
            .iter()
            .filter_map(|l| watch.feed(l))
            .collect();
        assert_eq!(
            watch
                .ops()
                .iter()
                .map(|o| o.id.as_str())
                .collect::<Vec<_>>(),
            [
                "org.gnome.Platform.Locale",
                "org.gnome.Platform",
                "org.gnome.Calculator"
            ]
        );
        assert_eq!(watch.count(OpKind::Install), 3);
        // Monotonic, never past the end, and it reaches the end.
        assert!(
            steps.windows(2).all(|w| w[0].done <= w[1].done),
            "{steps:#?}"
        );
        assert!(steps.iter().all(|s| s.total == 1000 && s.done <= 1000));
        assert_eq!(steps.last().unwrap().done, 1000);
        let first_tick = steps.iter().find(|s| s.done > 0).unwrap();
        assert_eq!(first_tick.done, 40, "12% of the first of three");
        assert_eq!(first_tick.phase, Phase::Downloading);
        assert_eq!(first_tick.now, "org.gnome.Platform.Locale");
        let second = steps
            .iter()
            .find(|s| s.now == "org.gnome.Platform" && s.done > 400)
            .unwrap();
        assert_eq!(second.done, 506, "(1 + 52%) of three");
        assert!(
            steps
                .iter()
                .any(|s| s.phase == Phase::Installing && s.now == "org.gnome.Calculator")
        );
    }

    #[test]
    fn a_line_that_only_looks_like_an_operation_is_none() {
        let mut watch = Watch::default();
        for line in [
            "Updating appstream data for user remote flathub",
            "Installing required authenticator for remote flathub",
            "Installation complete.",
            "Looking for matches…",
            "    ipc\tnetwork\tfallback-x11\twayland",
            "Installing 4/3…",
            "Info: org.x.Y was skipped",
        ] {
            assert_eq!(watch.feed(line), None, "{line}");
        }
        let lone = watch.feed("Uninstalling…").unwrap();
        assert_eq!((lone.phase, lone.done), (Phase::Removing, 0));
        let three_dots = watch.feed("Installing 2/2... ██ 50%").unwrap();
        assert_eq!(three_dots.done, 750);
    }

    #[test]
    fn an_update_counts_what_it_updated() {
        let mut watch = Watch::default();
        for l in run::lines_of(UPDATE.as_bytes()) {
            let _ = watch.feed(&l);
        }
        assert_eq!(watch.count(OpKind::Update), 2);
        assert_eq!(
            watch.count(OpKind::Install),
            1,
            "a new runtime is an install inside an update"
        );
    }

    #[test]
    fn flatpaks_words_become_a_closed_reason() {
        let t = |s: &str| refusal_of(&[s.to_owned()]);
        assert_eq!(
            t(
                "error: Unable to load summary from remote flathub: While fetching https://dl.flathub.org/repo/summary.idx: [6] Couldn't resolve host name"
            ),
            Refusal::Offline
        );
        // The next two are what the host's flatpak printed, read-only commands.
        assert_eq!(
            t("error: Remote \"flathub\" not found in the user installation"),
            Refusal::SourceUnprepared
        );
        assert_eq!(
            t("error: org.x.Nope/*unspecified*/*unspecified* not installed"),
            Refusal::UnknownItem
        );
        assert_eq!(
            t("error: Nothing matches org.x.Nope in remote flathub"),
            Refusal::UnknownItem
        );
        assert_eq!(
            t("error: org.x.Y/x86_64/stable not installed"),
            Refusal::UnknownItem
        );
        assert_eq!(
            t("error: Not enough disk space to complete this operation"),
            Refusal::ToolFailed
        );
    }

    #[test]
    fn permissions_read_in_the_words_of_flatpak_override() {
        let lines = permission_lines(PERMISSIONS);
        for want in [
            "share=network",
            "share=ipc",
            "socket=wayland",
            "nosocket=gpg-agent",
            "device=all",
            "filesystem=xdg-config/gtk-3.0:ro",
            "nofilesystem=home",
            "persist=.mozilla",
            "unset-env=QT_STYLE_OVERRIDE",
            "talk-name=org.freedesktop.FileManager1",
            "own-name=org.mozilla.firefox.*",
            "system-talk-name=org.freedesktop.NetworkManager",
            "env=MOZ_ENABLE_WAYLAND=1",
            "env=GTK_THEME=Adwaita:dark",
        ] {
            assert!(
                lines.iter().any(|l| l == want),
                "{want} missing from {lines:#?}"
            );
        }
        assert!(forces_gtk_theme(&lines));
        assert!(!forces_gtk_theme(&permission_lines(
            "[Context]\nshared=network;\n"
        )));
    }

    #[test]
    fn a_flatpakref_names_its_app_or_is_not_one() {
        let text = "[Flatpak Ref]\nTitle=Calculator\nName=org.gnome.Calculator\nBranch=stable\nUrl=https://dl.flathub.org/repo/\nIsRuntime=false\nGPGKey=AAAA\n";
        let r = flatpakref_of(text).unwrap();
        assert_eq!(
            (r.name.as_str(), r.branch.as_str(), r.is_runtime),
            ("org.gnome.Calculator", "stable", false)
        );
        assert_eq!(flatpakref_of("[Desktop Entry]\nName=org.x.Y\n"), None);
        assert_eq!(flatpakref_of("[Flatpak Ref]\nName=--help\n"), None);
    }

    #[test]
    fn a_tidy_counts_only_what_went_away() {
        // The `ref` column is the PARTIAL ref, without `app/` or `runtime/`.
        let before = parse_refs("org.a.Platform/x86_64/1\t1.0 GB\norg.c.D/x86_64/stable\t5.0 MB\n");
        let after = parse_refs("org.c.D/x86_64/stable\t5.0 MB\norg.e.F/x86_64/stable\t9.0 MB\n");
        assert_eq!(freed(&before, &after), 1_000_000_000);
    }

    #[test]
    fn nothing_runs_without_flatpak() {
        let dir = scratch("absent");
        let ctx = ctx_with(&dir, None, false);
        let order = Order::Install {
            key: AppKey::Flatpak {
                id: "org.x.Y".into(),
            },
            consent: Consent::default(),
        };
        let failure = run(&ctx, &order, &mut Vec::new()).unwrap_err();
        assert_eq!(failure.refusal, Some(Refusal::SourceAbsent));
        assert!(installed(&ctx).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_install_reports_its_progress_and_names_the_app() {
        let dir = scratch("install");
        let transcript = dir.join("install.txt");
        std::fs::write(&transcript, INSTALL).unwrap();
        let program = fake(
            &dir,
            &format!(
                "case \"$1\" in remotes) echo flathub ;; install) cat '{}' ;; esac",
                transcript.display()
            ),
        );
        let ctx = ctx_with(&dir, Some(program), false);
        let mut beats: Vec<Beat> = Vec::new();
        let order = Order::Install {
            key: AppKey::Flatpak {
                id: "org.gnome.Calculator".into(),
            },
            consent: Consent::default(),
        };
        let outcome = run(&ctx, &order, &mut beats).unwrap();
        assert_eq!(
            outcome.key,
            Some(AppKey::Flatpak {
                id: "org.gnome.Calculator".into()
            })
        );
        assert_eq!(outcome.desktop_ids, ["org.gnome.Calculator.desktop"]);
        assert!(beats.iter().any(|b| matches!(
            b,
            Beat::Step {
                done: 1000,
                total: 1000,
                ..
            }
        )));
        assert_eq!(
            logged(&dir),
            [
                "remotes --user --columns=name",
                "install --user -y -- flathub org.gnome.Calculator"
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_the_remote_the_source_is_unprepared() {
        let dir = scratch("unprepared");
        let program = fake(&dir, "case \"$1\" in remotes) echo other ;; esac");
        let ctx = ctx_with(&dir, Some(program), false);
        let order = Order::Install {
            key: AppKey::Flatpak {
                id: "org.x.Y".into(),
            },
            consent: Consent::default(),
        };
        assert_eq!(
            run(&ctx, &order, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::SourceUnprepared)
        );
        assert_eq!(
            logged(&dir),
            ["remotes --user --columns=name"],
            "nothing was installed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_the_flake_declared_is_never_removed_here() {
        let dir = scratch("declared");
        let program = fake(&dir, "exit 0");
        let ctx = ctx_with(&dir, Some(program), false);
        let state = ctx.roots.state.join(DECLARED_STATE);
        std::fs::create_dir_all(state.parent().unwrap()).unwrap();
        std::fs::write(
            &state,
            r#"{"version":1,"packages":[{"appId":"org.mozilla.firefox"}]}"#,
        )
        .unwrap();
        let order = Order::Remove {
            key: AppKey::Flatpak {
                id: "org.mozilla.firefox".into(),
            },
            keep_data: false,
        };
        assert_eq!(
            run(&ctx, &order, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::DeclaredInTheFlake)
        );
        assert!(logged(&dir).is_empty(), "flatpak was never called");
        let mine = Order::Remove {
            key: AppKey::Flatpak {
                id: "org.gnome.Calculator".into(),
            },
            keep_data: false,
        };
        run(&ctx, &mine, &mut Vec::new()).unwrap();
        assert_eq!(
            logged(&dir),
            ["uninstall --user -y --delete-data -- org.gnome.Calculator"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failure_carries_flatpaks_words_and_its_reason() {
        let dir = scratch("offline");
        let program = fake(
            &dir,
            "case \"$1\" in remotes) echo flathub ;; *) echo 'error: Unable to load summary from remote flathub: Could not resolve host' >&2; exit 1 ;; esac",
        );
        let ctx = ctx_with(&dir, Some(program), false);
        let order = Order::Install {
            key: AppKey::Flatpak {
                id: "org.x.Y".into(),
            },
            consent: Consent::default(),
        };
        let failure = run(&ctx, &order, &mut Vec::new()).unwrap_err();
        assert_eq!(failure.refusal, Some(Refusal::Offline));
        assert!(failure.trouble.contains("Could not resolve host"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refresh_writes_the_cache_the_list_reads() {
        let dir = scratch("refresh");
        let program = fake(
            &dir,
            "case \"$1\" in remotes) echo flathub ;; remote-ls) printf 'org.mozilla.firefox\\t9f8e7d6c5b4a\\t144.0\\n' ;; list) printf 'org.mozilla.firefox\\tFirefox\\t143.0.1\\tstable\\tflathub\\tuser\\t312.4 MB\\t0123456789ab\\n' ;; esac",
        );
        let ctx = ctx_with(&dir, Some(program), false);
        let outcome = run(
            &ctx,
            &Order::Refresh {
                source: Source::Flatpak,
            },
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(outcome.amount, Some(1));
        let cached = read_updates(&ctx.roots);
        assert_eq!(cached.updates["org.mozilla.firefox"].version, "144.0");
        assert!(cached.when > 0);
        let list = installed(&ctx);
        assert_eq!(list[0].update.as_deref(), Some("144.0"));
        assert_eq!(
            logged(&dir)[..3],
            [
                "remotes --user --columns=name".to_owned(),
                "update --user --appstream -- flathub".to_owned(),
                format!("remote-ls --user --updates --app --columns={UPDATE_COLUMNS}"),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_override_names_one_permission_and_never_gives_a_variable() {
        let id = "org.gnome.gedit";
        assert_eq!(
            override_argv(
                id,
                Grant::Files {
                    reach: Reach::ReadOnly
                }
            ),
            ["override", "--user", "--filesystem=home:ro", "--", id]
        );
        assert_eq!(
            override_argv(
                id,
                Grant::Files {
                    reach: Reach::Nothing
                }
            ),
            [
                "override",
                "--user",
                "--nofilesystem=home",
                "--nofilesystem=host",
                "--",
                id
            ]
        );
        assert_eq!(
            override_argv(id, Grant::UnsetGtkTheme),
            ["override", "--user", "--unset-env=GTK_THEME", "--", id]
        );
        let every = [
            Grant::Files {
                reach: Reach::ReadWrite,
            },
            Grant::Network { on: true },
            Grant::Network { on: false },
            Grant::Sound { on: true },
            Grant::Sound { on: false },
            Grant::Devices { on: true },
            Grant::Devices { on: false },
            Grant::Display {
                through: Display::Wayland,
            },
            Grant::Display {
                through: Display::X11,
            },
            Grant::UnsetGtkTheme,
            Grant::Reset,
        ];
        for g in every {
            let argv = override_argv(id, g);
            assert_eq!(argv.last().map(String::as_str), Some(id));
            assert!(argv.iter().all(|a| !a.starts_with("--env")), "{argv:?}");
        }
    }

    #[test]
    fn the_grants_are_read_from_the_effective_permissions() {
        let g = grants_of(
            "[Application]\nname=org.gnome.gedit\n\n[Context]\nshared=network;ipc;\n\
             sockets=x11;wayland;pulseaudio;\ndevices=dri;\nfilesystems=xdg-run/gvfsd;host:ro;\n\n\
             [Environment]\nGTK_THEME=Adwaita:dark\n",
        );
        assert_eq!(
            g,
            Grants {
                files: Reach::ReadOnly,
                network: true,
                sound: true,
                devices: false,
                display: Some(Display::Wayland),
                gtk_theme: true,
            }
        );
        let g = grants_of(
            "[Context]\nfilesystems=home:ro;host;\nsockets=fallback-x11;\ndevices=all;\n",
        );
        assert_eq!(g.files, Reach::ReadWrite, "the widest one wins");
        assert_eq!(g.display, Some(Display::X11));
        assert!(g.devices && !g.network && !g.sound && !g.gtk_theme);
        assert_eq!(grants_of(""), Grants::default());
    }

    #[test]
    fn the_dark_facts_are_read_from_the_files() {
        let dir = scratch("dark");
        let user = dir.join("user");
        let system = dir.join("system");
        let both = [user.clone(), system.clone()];
        assert_eq!(dark_facts(&both), DarkFacts::default());
        std::fs::create_dir_all(system.join("runtime").join(DARK_EXTENSION).join("x86_64"))
            .unwrap();
        std::fs::create_dir_all(user.join("overrides")).unwrap();
        std::fs::write(
            user.join("overrides").join("global"),
            "[Context]\nunset-environment=QT_QPA_PLATFORMTHEME;\n",
        )
        .unwrap();
        assert_eq!(
            dark_facts(&both),
            DarkFacts {
                extension: true,
                qt_unset: false
            },
            "one of the two Qt variables is not enough"
        );
        std::fs::write(
            user.join("overrides").join("global"),
            "[Context]\nunset-environment=QT_QPA_PLATFORMTHEME;QT_STYLE_OVERRIDE;\n",
        )
        .unwrap();
        assert!(dark_facts(&both).qt_unset);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_permission_is_one_override_and_a_rehearsal_runs_none() {
        let dir = scratch("permit");
        let program = fake(&dir, "exit 0");
        let order = Order::Permission {
            id: "org.gnome.gedit".into(),
            grant: Grant::Network { on: false },
        };
        let done = run(
            &ctx_with(&dir, Some(program.clone()), false),
            &order,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(!done.rehearsed);
        assert_eq!(
            logged(&dir),
            ["override --user --unshare=network -- org.gnome.gedit"]
        );
        let _ = std::fs::remove_file(dir.join("argv.log"));
        assert!(
            run(
                &ctx_with(&dir, Some(program), true),
                &order,
                &mut Vec::new()
            )
            .unwrap()
            .rehearsed
        );
        assert!(logged(&dir).is_empty());
        let bad = Order::Permission {
            id: "--reset".into(),
            grant: Grant::Reset,
        };
        let program = fake(&dir, "exit 0");
        assert_eq!(
            run(&ctx_with(&dir, Some(program), false), &bad, &mut Vec::new())
                .unwrap_err()
                .refusal,
            Some(Refusal::UnknownItem)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rehearsal_changes_nothing_and_says_so() {
        let dir = scratch("rehearse");
        let program = fake(&dir, "case \"$1\" in remotes) echo flathub ;; esac");
        let ctx = ctx_with(&dir, Some(program), true);
        let order = Order::Install {
            key: AppKey::Flatpak {
                id: "org.x.Y".into(),
            },
            consent: Consent::default(),
        };
        assert!(run(&ctx, &order, &mut Vec::new()).unwrap().rehearsed);
        let tidy = run(
            &ctx,
            &Order::Tidy {
                source: Source::Flatpak,
            },
            &mut Vec::new(),
        )
        .unwrap();
        assert!(tidy.rehearsed);
        assert_eq!(tidy.amount, None);
        assert!(
            logged(&dir)
                .iter()
                .all(|l| l.starts_with("remotes ") || l.starts_with("list ")),
            "only reads ran: {:?}",
            logged(&dir)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_flatpakref_needs_consent_and_must_be_one() {
        let dir = scratch("ref");
        let program = fake(&dir, "exit 0");
        let ctx = ctx_with(&dir, Some(program), false);
        let file = dir.join("calc.flatpakref");
        std::fs::write(
            &file,
            "[Flatpak Ref]\nName=org.gnome.Calculator\nUrl=https://dl.flathub.org/repo/\n",
        )
        .unwrap();
        let without = Order::InstallFile {
            path: file.clone(),
            consent: Consent::default(),
        };
        assert_eq!(
            run(&ctx, &without, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::UnverifiedNeedsConsent)
        );
        let consent = Consent {
            unverified: true,
            ..Consent::default()
        };
        let with = Order::InstallFile {
            path: file.clone(),
            consent,
        };
        assert_eq!(
            run(&ctx, &with, &mut Vec::new()).unwrap().key,
            Some(AppKey::Flatpak {
                id: "org.gnome.Calculator".into()
            })
        );
        assert_eq!(
            logged(&dir),
            [format!("install --user -y --from -- {}", file.display())]
        );
        let fake_ref = dir.join("not.flatpakref");
        std::fs::write(&fake_ref, "#!/bin/sh\n").unwrap();
        let bad = Order::InstallFile {
            path: fake_ref,
            consent,
        };
        assert_eq!(
            run(&ctx, &bad, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::UnknownItem)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
