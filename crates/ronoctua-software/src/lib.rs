//! The person's own programs — search, install, try, update and remove them.
//!
//! # What this crate is for
//!
//! Invariant 6 says nothing exists that is not in the flake, and its dated
//! exception (`AGENTS.md`, 2026-09-26) carves out exactly one thing: the
//! programs a person installs for THEMSELVES. They live under `$HOME` (the
//! user's Nix profile, the `--user` Flatpak installation, AppImages, web apps)
//! and inside Android, never in the system and never in the repository. This
//! crate is the one border that does that, and nothing else in the tree names
//! the programs it drives (`one-software-boundary`).
//!
//! # The five sources
//!
//! | source | what does the work | where it lands |
//! |---|---|---|
//! | Nix | `nix profile`, and `nix build` for a trial | the user's profile |
//! | Flatpak | `flatpak --user` | `~/.local/share/flatpak` |
//! | AppImage | the file itself, run by `appimage-run` | `~/Applications` |
//! | Android | `waydroid app` | inside the container |
//! | web app | Chrome with `--app=` | a `.desktop` in `~/.local/share/applications` |
//!
//! # How it is split
//!
//! Every argument vector is a PURE function (`nix::add_argv`, …) so a test can
//! read it without running anything; the functions that run take a [`Tools`]
//! and a [`Roots`] as parameters and never reach for the environment
//! themselves, so a test hands them fake programs and a scratch tree. The CLI
//! (`src/bin/cli.rs`) is the only place that reads the real environment.
//!
//! The screen never runs these programs on its own thread: an order goes to the
//! queue (`ronoctua-transfers`), which runs this crate's CLI as a child and reads
//! the progress lines it prints ([`progress`]). The screen links the library
//! for what is quick and read-only — the installed list, the search, a preview.
//!
//! # The person's declaration
//!
//! A program the person keeps is DECLARED: an entry of their
//! `declared.json`, which travels with their belongings to every machine of
//! theirs and which the update builds into each ([`declaration`]). Declaring
//! installs it here at once by the source's own path and pins its download;
//! the mirror is then staged for root's applier ([`stage`]) and pushed. The
//! distro's own programs can be declined, here or everywhere, the same way.
//! What Nix cannot do — Android's apps, and the copies by hand a declared one
//! replaces — the reconciler does, within its ledger ([`reconcile`]).

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod declaration;
pub mod follow;
pub mod found;
pub mod journal;
pub mod lock;
pub mod order;
pub mod pins;
pub mod prefs;
pub mod progress;
pub mod reconcile;
pub mod refusal;
pub mod run;
pub mod stage;
pub mod state;

pub mod android;
pub mod appimage;
pub mod appstream;
pub mod dashboard;
pub mod details;
pub mod fdroid;
pub mod flatpak;
pub mod html;
pub mod nix;
pub mod nix_index;
pub mod nix_log;
pub mod search;
pub mod trial;
pub mod web;

#[cfg(feature = "net")]
pub mod appimagehub;
#[cfg(all(feature = "net", feature = "draw"))]
pub mod cli;
#[cfg(all(feature = "net", feature = "draw"))]
pub mod declare;
#[cfg(feature = "net")]
pub mod flathub;
#[cfg(feature = "net")]
pub mod github;
#[cfg(feature = "draw")]
pub mod icons;
#[cfg(feature = "net")]
pub mod net;

pub use order::Order;
pub use refusal::Refusal;
/// The person's declared data, as its one schema says it — re-exported, so
/// the screen reads a declaration in the types this crate writes it in.
pub use ronoctua_person as person;

/// Where a program comes from.
///
/// The order of [`Source::ALL`] is the order the screen lists them in, and it
/// is also the default preference: Nix first (the system's own revision, no
/// runtime to download), then Flatpak, AppImage and Android. A person can flip
/// the first two in the view's advanced level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// A package from the system's nixpkgs revision, in the user's profile.
    Nix,
    /// An app from Flathub, in the `--user` installation.
    Flatpak,
    /// A single-file program, kept in `~/Applications`.
    #[serde(rename = "appimage")]
    AppImage,
    /// An Android app, inside Waydroid.
    Android,
    /// A site opened by Chrome in a window with nothing of Chrome's.
    Web,
}

impl Source {
    /// Every one, for the checks that demand a text and a glyph per source.
    pub const ALL: [Self; 5] = [
        Self::Nix,
        Self::Flatpak,
        Self::AppImage,
        Self::Android,
        Self::Web,
    ];

    /// The stem of its text key (`software-source-<word>`) and of its glyph.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Nix => "nix",
            Self::Flatpak => "flatpak",
            Self::AppImage => "appimage",
            Self::Android => "android",
            Self::Web => "web",
        }
    }

    /// The source a word names, for the CLI and for a typed `nix:` prefix.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.word() == word)
    }
}

/// One program's identity, whatever its source.
///
/// It is what the cursor keeps (never a row index: a list that reorders would
/// move the cursor onto a neighbour), what an order names, and what a result
/// row and an installed row share when they are the same program.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "source"
)]
pub enum AppKey {
    /// A nixpkgs attribute path, `ripgrep` or `python3Packages.requests`.
    Nix {
        /// The attribute, without `nixpkgs#`.
        attr: String,
    },
    /// A Flathub application id, `org.gnome.Calculator`.
    Flatpak {
        /// The reverse-DNS id.
        id: String,
    },
    /// An AppImage this crate integrated, by the key it gave it.
    #[serde(rename = "appimage")]
    AppImage {
        /// The folded, file-safe name (`obsidian`).
        key: String,
    },
    /// An Android package, `org.fdroid.fdroid`.
    Android {
        /// The package name.
        package: String,
    },
    /// A web app, by its slug (`whatsapp`).
    Web {
        /// The slug the `.desktop` file is named after.
        slug: String,
    },
    /// A program the FLAKE installed. It is listed so the person can see and
    /// open it, and every order that would change it is refused
    /// ([`Refusal::DeclaredInTheFlake`]).
    System {
        /// Its desktop-file id.
        desktop_id: String,
    },
}

impl AppKey {
    /// Which source it belongs to, when it belongs to one.
    #[must_use]
    pub const fn source(&self) -> Option<Source> {
        match self {
            Self::Nix { .. } => Some(Source::Nix),
            Self::Flatpak { .. } => Some(Source::Flatpak),
            Self::AppImage { .. } => Some(Source::AppImage),
            Self::Android { .. } => Some(Source::Android),
            Self::Web { .. } => Some(Source::Web),
            Self::System { .. } => None,
        }
    }

    /// The identifier inside its source, as a person would type it.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Nix { attr } => attr,
            Self::Flatpak { id } => id,
            Self::AppImage { key } => key,
            Self::Android { package } => package,
            Self::Web { slug } => slug,
            Self::System { desktop_id } => desktop_id,
        }
    }
}

/// The external programs, by STORE PATH, and never by name.
///
/// A unit's `PATH` is minimal and a bare name fails far from its cause, so
/// every one arrives from the environment the Nix module writes
/// (`nix/modules/software.nix`, `makeWrapper --set-default`). A `None` means
/// the machine does not have that support (the minimal profile has no
/// Waydroid and no `appimage-run`), and the order that needs it is refused
/// with a sentence instead of failing on `exec`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tools {
    /// `RONOCTUA_NIX` — the Nix the person's shell uses.
    pub nix: Option<PathBuf>,
    /// `RONOCTUA_FLATPAK`.
    pub flatpak: Option<PathBuf>,
    /// `RONOCTUA_APPIMAGE_RUN`.
    pub appimage_run: Option<PathBuf>,
    /// `RONOCTUA_UNSQUASHFS`.
    pub unsquashfs: Option<PathBuf>,
    /// `RONOCTUA_WAYDROID`.
    pub waydroid: Option<PathBuf>,
    /// `RONOCTUA_BWRAP`, for the sandboxed trial.
    pub bwrap: Option<PathBuf>,
    /// `RONOCTUA_SYSTEMD_RUN`.
    pub systemd_run: Option<PathBuf>,
    /// `RONOCTUA_SYSTEMCTL`.
    pub systemctl: Option<PathBuf>,
    /// `RONOCTUA_CHROME` — a STABLE path (`/run/current-system/sw/bin/…`),
    /// never a store path: a `.desktop` written today has to open the Chrome
    /// of the next generation too.
    pub chrome: Option<PathBuf>,
    /// `RONOCTUA_SHELL` — our own binary, for "open this in a terminal".
    pub shell: Option<PathBuf>,
    /// `RONOCTUA_NIX_INDEX` — the search index built from the pinned nixpkgs.
    pub nix_index: Option<PathBuf>,
    /// `RONOCTUA_NIXPKGS_REV` — the revision that index was built from.
    pub nixpkgs_rev: Option<String>,
    /// `RONOCTUA_ICONS` — the superpack's theme root (the distro's collection).
    pub icons: Option<PathBuf>,
    /// `RONOCTUA_ICON_NAMES` — every name the superpack answers to, one per line.
    pub icon_names: Option<PathBuf>,
    /// `RONOCTUA_SOFTWARE` — this crate's own CLI, which the queue runs.
    pub software: Option<PathBuf>,
    /// `RONOCTUA_BELONGINGS` — the belongings' CLI, which carries a
    /// declaration to the person's repository (`plain push`). The queue's
    /// unit has it (`nix/modules/belongings.nix`), and the order the queue
    /// runs inherits it; without it a declaration travels at the next sync.
    pub belongings: Option<PathBuf>,
}

impl Tools {
    /// What the environment says. The ONLY reader of `RONOCTUA_*` in the crate.
    #[must_use]
    pub fn from_env() -> Self {
        let path = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            nix: path("RONOCTUA_NIX"),
            flatpak: path("RONOCTUA_FLATPAK"),
            appimage_run: path("RONOCTUA_APPIMAGE_RUN"),
            unsquashfs: path("RONOCTUA_UNSQUASHFS"),
            waydroid: path("RONOCTUA_WAYDROID"),
            bwrap: path("RONOCTUA_BWRAP"),
            systemd_run: path("RONOCTUA_SYSTEMD_RUN"),
            systemctl: path("RONOCTUA_SYSTEMCTL"),
            chrome: path("RONOCTUA_CHROME"),
            shell: path("RONOCTUA_SHELL"),
            nix_index: path("RONOCTUA_NIX_INDEX"),
            nixpkgs_rev: std::env::var("RONOCTUA_NIXPKGS_REV")
                .ok()
                .filter(|v| !v.is_empty()),
            icons: path("RONOCTUA_ICONS"),
            icon_names: path("RONOCTUA_ICON_NAMES"),
            software: path("RONOCTUA_SOFTWARE"),
            belongings: path("RONOCTUA_BELONGINGS"),
        }
    }
}

/// The machine's side of the person's declaration: where root's applier
/// reads the staging and says what it did, what Nix wrote back about the
/// system, and the machine's own record. Constants on a machine; a folder of
/// the test's (or a photograph's room) everywhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct System {
    /// `/run/ronoctua-person` — root's folder of the person's staging
    /// (`staged/`, `verdict.json`; `ronoctua_person::root`).
    pub person_run: PathBuf,
    /// `/etc/ronoctua` — `person-applied.json`, `distro-apps.json` and
    /// `declared-android.json`, written by Nix (`ronoctua_person::reports`).
    pub reports: PathBuf,
    /// `/var/lib/ronoctua-machine` — the machine's record (`state.json`).
    pub machine: PathBuf,
    /// `/run/current-system` — the system the machine runs. A switch
    /// replaces this link, and that is how a watcher learns the reports
    /// changed: their own links under `/etc` never move (`/etc/static`
    /// does).
    pub current: PathBuf,
}

impl System {
    /// The machine's: the three folders where the system puts them — or, when
    /// `RONOCTUA_SOFTWARE_SYSROOT` names a folder, the same three under it
    /// (the photograph's room, a test). Read once, by the session's roots.
    #[must_use]
    pub fn of_machine(sysroot: Option<&Path>) -> Self {
        let at = |path: &str| {
            sysroot.map_or_else(
                || PathBuf::from(path),
                |root| root.join(path.trim_start_matches('/')),
            )
        };
        Self {
            person_run: at(ronoctua_person::root::RUN),
            reports: at(REPORTS),
            machine: at(ronoctua_machine::STATE_DIR),
            current: at(CURRENT_SYSTEM),
        }
    }

    /// `staged/`, where the person's tree is laid for the applier.
    #[must_use]
    pub fn staged(&self) -> PathBuf {
        self.person_run.join("staged")
    }

    /// `verdict.json`, what the applier last did.
    #[must_use]
    pub fn verdict(&self) -> PathBuf {
        self.person_run.join("verdict.json")
    }
}

/// Where Nix writes its reports about the person's declaration.
pub const REPORTS: &str = "/etc/ronoctua";

/// The link to the system the machine runs.
pub const CURRENT_SYSTEM: &str = "/run/current-system";

/// Where the person's things are. Every function that reads or writes takes
/// one of these as a PARAMETER (`persistence-takes-its-root-as-a-parameter`),
/// so a test hands it a scratch tree instead of a booted machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
    /// `$HOME`.
    pub home: PathBuf,
    /// `$XDG_DATA_HOME` (`~/.local/share`).
    pub data: PathBuf,
    /// `$XDG_CONFIG_HOME` (`~/.config`).
    pub config: PathBuf,
    /// `$XDG_STATE_HOME` (`~/.local/state`).
    pub state: PathBuf,
    /// `$XDG_CACHE_HOME` (`~/.cache`).
    pub cache: PathBuf,
    /// `$XDG_RUNTIME_DIR`, when there is one.
    pub runtime: Option<PathBuf>,
    /// `$XDG_DATA_DIRS`, in its order: where a launcher entry the system or
    /// the person's profile installed lives (a declared web app's, a
    /// declared AppImage's, the distro's programs').
    pub data_dirs: Vec<PathBuf>,
    /// The machine's side of the declaration.
    pub system: System,
}

impl Roots {
    /// The session's, read once.
    ///
    /// # Errors
    ///
    /// When there is no `$HOME` — nothing here can be answered without one.
    pub fn of_session() -> anyhow::Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
        let xdg = |name: &str, fallback: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map_or_else(|| home.join(fallback), PathBuf::from)
        };
        Ok(Self {
            data: xdg("XDG_DATA_HOME", ".local/share"),
            config: xdg("XDG_CONFIG_HOME", ".config"),
            state: xdg("XDG_STATE_HOME", ".local/state"),
            cache: xdg("XDG_CACHE_HOME", ".cache"),
            runtime: std::env::var_os("XDG_RUNTIME_DIR")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            data_dirs: data_dirs_of(std::env::var("XDG_DATA_DIRS").ok().as_deref()),
            system: System::of_machine(
                std::env::var_os("RONOCTUA_SOFTWARE_SYSROOT")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
                    .as_deref(),
            ),
            home,
        })
    }

    /// A tree under one directory, for tests and for a photograph's room:
    /// the person's under `home/`, the machine's under `sys/` (as
    /// [`System::of_machine`] lays it) and the system's launcher entries
    /// under `share/`.
    #[must_use]
    pub fn under(root: &Path) -> Self {
        Self {
            home: root.join("home"),
            data: root.join("home/.local/share"),
            config: root.join("home/.config"),
            state: root.join("home/.local/state"),
            cache: root.join("home/.cache"),
            runtime: Some(root.join("run")),
            data_dirs: vec![root.join("share")],
            system: System::of_machine(Some(&root.join("sys"))),
        }
    }

    /// `$XDG_DATA_HOME/applications`, where every `.desktop` we write lives —
    /// at the TOP level, because the launcher's stamp reads that directory's
    /// modification time.
    #[must_use]
    pub fn applications(&self) -> PathBuf {
        self.data.join("applications")
    }

    /// `~/Applications`, the AppImages' home (the convention AppImageLauncher
    /// set, and the one a person already looks in).
    #[must_use]
    pub fn appimages(&self) -> PathBuf {
        self.home.join("Applications")
    }

    /// Our own data: `$XDG_DATA_HOME/ronoctua[-dev]/software`.
    #[must_use]
    pub fn ours(&self) -> PathBuf {
        self.data.join(ronoctua_xdg::name()).join("software")
    }

    /// Our own state: `$XDG_STATE_HOME/ronoctua[-dev]/software`.
    #[must_use]
    pub fn ours_state(&self) -> PathBuf {
        self.state.join(ronoctua_xdg::name()).join("software")
    }

    /// Our own cache: `$XDG_CACHE_HOME/ronoctua[-dev]/software` — what can be
    /// thrown away and fetched again (search indices, icon candidates).
    #[must_use]
    pub fn ours_cache(&self) -> PathBuf {
        self.cache.join(ronoctua_xdg::name()).join("software")
    }
}

/// `$XDG_DATA_DIRS` as a list: absolute folders only, in order, and the
/// specification's default (`/usr/local/share:/usr/share`) when it is unset
/// or empty.
#[must_use]
pub fn data_dirs_of(value: Option<&str>) -> Vec<PathBuf> {
    let value = value
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("/usr/local/share:/usr/share");
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in value.split(':').map(PathBuf::from) {
        if dir.is_absolute() && !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// What a finished order leaves behind, for the notification and the screen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Outcome {
    /// The program it was about, when there was one.
    pub key: Option<AppKey>,
    /// The desktop ids it created, so "open" can be offered.
    #[serde(default)]
    pub desktop_ids: Vec<String>,
    /// A number the order produced (bytes freed by a tidy, items upgraded).
    #[serde(default)]
    pub amount: Option<u64>,
    /// `true` when this instance only REHEARSED (`RONOCTUA_INSTANCE=dev`): it
    /// printed what it would run and ran nothing.
    #[serde(default)]
    pub rehearsed: bool,
}

/// How an order failed.
///
/// A [`Refusal`] is a VALUE the screen draws with a sentence; `trouble` is the
/// program's own words (English, inv. 18), shown as the last lines of the job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Failure {
    /// The reason, when it is one of the closed set.
    pub refusal: Option<Refusal>,
    /// What the program said.
    pub trouble: String,
}

impl Failure {
    /// A refusal with no program behind it.
    #[must_use]
    pub fn refused(refusal: Refusal) -> Self {
        Self {
            refusal: Some(refusal),
            trouble: String::new(),
        }
    }

    /// A failure in a program's own words.
    #[must_use]
    pub fn trouble(said: impl Into<String>) -> Self {
        Self {
            refusal: None,
            trouble: said.into(),
        }
    }
}

impl From<anyhow::Error> for Failure {
    fn from(e: anyhow::Error) -> Self {
        Self::trouble(format!("{e:#}"))
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.refusal, self.trouble.is_empty()) {
            (Some(r), true) => write!(f, "{}", r.word()),
            (Some(r), false) => write!(f, "{}: {}", r.word(), self.trouble),
            (None, _) => write!(f, "{}", self.trouble),
        }
    }
}

/// The answer to an order.
pub type Done = Result<Outcome, Failure>;

/// Everything an order needs to run, passed down and never looked up.
#[derive(Debug, Clone)]
pub struct Context {
    /// The programs, by path.
    pub tools: Tools,
    /// The person's directories.
    pub roots: Roots,
    /// The person's choices (unfree consents, the trial's traces, …).
    pub prefs: prefs::Preferences,
    /// `true` in the `.#dev` instance: print what would run, run nothing.
    pub rehearse: bool,
}

impl Context {
    /// The session's, as the CLI builds it.
    ///
    /// # Errors
    ///
    /// When there is no `$HOME`.
    pub fn of_session() -> anyhow::Result<Self> {
        let roots = Roots::of_session()?;
        Ok(Self {
            tools: Tools::from_env(),
            prefs: prefs::read(&roots.config),
            rehearse: std::env::var_os("RONOCTUA_SOFTWARE_REHEARSE").is_some_and(|v| v == "1")
                || ronoctua_xdg::instance().is_some(),
            roots,
        })
    }
}

/// A folded, file-safe key from a name: lower case, ASCII letters and digits,
/// runs of anything else as one `-`, at most 40 characters.
///
/// It names files (`ronoctua-web-<slug>.desktop`, `~/Applications/<key>…`) and
/// ids, so it may never contain a `/`, a `.` or a space — whatever the person
/// typed as a name.
#[must_use]
pub fn slug_of(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = ronoctua_apps::fold::folded_char(c);
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
        if out.len() >= 40 {
            break;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// An executable file written by a child `/bin/sh` and never by this
/// process, for the tests' fake tools: a file this process holds open for
/// writing while another test's thread forks is inherited by that fork until
/// it execs, and running the file in that window fails with `ETXTBSY` ("Text
/// file busy" — it made the Flatpak and Nix upgrade tests flaky).
#[cfg(test)]
pub(crate) fn write_executable(path: &std::path::Path, body: &str) {
    std::fs::create_dir_all(path.parent().expect("a file in a directory")).unwrap();
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
            "sh",
        ])
        .arg(path)
        .arg(body)
        .status()
        .unwrap();
    assert!(status.success(), "{}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_names_a_file_and_nothing_else() {
        assert_eq!(slug_of("WhatsApp Web"), "whatsapp-web");
        assert_eq!(slug_of("  Café / Notícias.. "), "cafe-noticias");
        assert_eq!(slug_of("../../etc"), "etc");
        assert_eq!(slug_of("日本"), "");
        assert!(slug_of(&"a".repeat(90)).len() <= 40);
    }

    #[test]
    fn the_data_dirs_are_absolute_ordered_and_have_a_default() {
        assert_eq!(
            data_dirs_of(Some(
                "/etc/profiles/per-user/a/share:relative:/run/current-system/sw/share:/etc/profiles/per-user/a/share"
            )),
            [
                PathBuf::from("/etc/profiles/per-user/a/share"),
                PathBuf::from("/run/current-system/sw/share"),
            ]
        );
        assert_eq!(
            data_dirs_of(None),
            [
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share")
            ]
        );
        assert_eq!(data_dirs_of(Some("")), data_dirs_of(None));
    }

    #[test]
    fn the_machines_side_moves_whole_under_a_room() {
        let here = System::of_machine(None);
        assert_eq!(here.person_run, PathBuf::from(ronoctua_person::root::RUN));
        assert_eq!(here.staged(), PathBuf::from(ronoctua_person::root::STAGED));
        assert_eq!(
            here.verdict(),
            PathBuf::from(ronoctua_person::root::VERDICT)
        );
        assert_eq!(here.reports, PathBuf::from(REPORTS));
        assert_eq!(here.machine, PathBuf::from(ronoctua_machine::STATE_DIR));
        assert_eq!(here.current, PathBuf::from(CURRENT_SYSTEM));
        let room = System::of_machine(Some(Path::new("/room/sys")));
        assert_eq!(room.reports, PathBuf::from("/room/sys/etc/ronoctua"));
        assert_eq!(room.current, PathBuf::from("/room/sys/run/current-system"));
        assert_eq!(
            room.staged(),
            PathBuf::from("/room/sys/run/ronoctua-person/staged")
        );
    }

    #[test]
    fn every_source_has_a_word_and_is_found_by_it() {
        for s in Source::ALL {
            assert_eq!(Source::of_word(s.word()), Some(s));
        }
    }

    #[test]
    fn a_key_travels_as_json_and_comes_back_the_same() {
        let a = AppKey::AppImage {
            key: "obsidian".into(),
        };
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            r#"{"source":"appimage","key":"obsidian"}"#
        );
        for s in Source::ALL {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.word())
            );
        }
        let k = AppKey::Flatpak {
            id: "org.gnome.Calculator".into(),
        };
        let text = serde_json::to_string(&k).unwrap();
        assert_eq!(text, r#"{"source":"flatpak","id":"org.gnome.Calculator"}"#);
        assert_eq!(serde_json::from_str::<AppKey>(&text).unwrap(), k);
    }
}
