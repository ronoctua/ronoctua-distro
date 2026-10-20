//! Everything that can be asked of this crate, as ONE enum.
//!
//! The screen builds one, the queue carries it to the CLI as JSON on stdin,
//! and the CLI runs it. One type for the three is what keeps "what the button
//! meant" and "what ran" from drifting apart: a variant the CLI does not
//! handle does not compile.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{AppKey, Source};

/// One thing to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "order"
)]
pub enum Order {
    /// Install a program for this person.
    Install {
        /// Which.
        key: AppKey,
        /// What the person already said yes to.
        #[serde(default)]
        consent: Consent,
    },
    /// Install a file the person already has: an `.AppImage`, an `.apk` or a
    /// `.flatpakref`.
    InstallFile {
        /// The file.
        path: PathBuf,
        /// What the person already said yes to (a file from nowhere is never
        /// verified, and saying so is part of the question).
        #[serde(default)]
        consent: Consent,
    },
    /// Remove a program.
    Remove {
        /// Which.
        key: AppKey,
        /// Keep what the program saved (`~/.var/app/…`, the web app's login in
        /// Chrome). The default is to KEEP: removing a person's data is the
        /// second, danger-coloured answer, never the first.
        #[serde(default = "yes")]
        keep_data: bool,
    },
    /// Bring programs up to date.
    Upgrade {
        /// Which ones.
        scope: Scope,
    },
    /// Free what nothing uses any more: Flatpak runtimes no app needs, old
    /// generations of the Nix profile, AppImage extraction caches.
    Tidy {
        /// Whose leftovers.
        source: Source,
    },
    /// Run a Nix package WITHOUT installing it.
    Try {
        /// The nixpkgs attribute.
        attr: String,
        /// What the trial may leave behind.
        #[serde(default)]
        traces: Traces,
        /// Whether the sandboxed trial reaches the network.
        #[serde(default = "yes")]
        network: bool,
        /// What the person already said yes to.
        #[serde(default)]
        consent: Consent,
    },
    /// Stop a running trial.
    StopTrial {
        /// The trial's key (the attribute, folded).
        trial: String,
    },
    /// Keep a trial: install what was tried, without downloading it again.
    Keep {
        /// The trial's key.
        trial: String,
    },
    /// Make a source ready: the Flathub remote and its catalogue, or Android.
    Prepare {
        /// Which source.
        source: Source,
        /// For Android: which system image.
        #[serde(default)]
        image: Option<AndroidImage>,
    },
    /// Fetch a source's catalogue again (and learn what can be updated).
    Refresh {
        /// Which source.
        source: Source,
    },
    /// Go back to an earlier generation of the Nix profile.
    Rollback {
        /// The generation number, as `nix profile history` numbers it.
        generation: u64,
    },
    /// Create a web app.
    WebCreate {
        /// What it is.
        spec: WebSpec,
    },
    /// Change a web app.
    WebEdit {
        /// Which one.
        slug: String,
        /// What it becomes.
        spec: WebSpec,
    },
    /// Start or stop the Android session.
    AndroidSession {
        /// `true` to start.
        on: bool,
    },
    /// Erase Android: the images and everything installed inside it.
    AndroidErase,
    /// Change one thing a Flatpak app may reach (`flatpak override --user`).
    Permission {
        /// The app's id.
        id: String,
        /// What changes.
        grant: Grant,
    },
    /// Each Android app in its own window, or all of Android in one. It takes
    /// effect when the session starts again.
    AndroidWindows {
        /// `true` for one window per app.
        multi: bool,
    },
    /// Install for this person AND declare it: installed now by today's path,
    /// pinned, merged into the person's `declared.json`, applied, and pushed
    /// with the plain part (the software package).
    Declare {
        /// Which.
        key: AppKey,
        /// What the person already said yes to.
        #[serde(default)]
        consent: Consent,
        /// `None` for every machine of the person, or the hostnames.
        #[serde(default)]
        machines: Option<Vec<String>>,
    },
    /// Remove for good: here and from the declaration, so the person's other
    /// machines remove it at their next update.
    Forget {
        /// Which.
        key: AppKey,
        /// Keep what the program saved (the default, as `Remove`'s).
        #[serde(default = "yes")]
        keep_data: bool,
    },
    /// Keep one of the distro's programs uninstalled, here or everywhere.
    Decline {
        /// The distro registry's id.
        id: String,
        /// `None` for every machine of the person, or the hostnames.
        #[serde(default)]
        machines: Option<Vec<String>>,
    },
    /// Bring a declined program of the distro back on this machine: it
    /// leaves the decline's list of machines (the others keep it out), and
    /// a decline of every machine goes whole. With `everywhere`, the
    /// decline goes whole whatever machines it names — one of computers
    /// that no longer exist, asked first.
    Undecline {
        /// The distro registry's id.
        id: String,
        /// On every computer of the person's, not this one alone.
        #[serde(default)]
        everywhere: bool,
    },
    /// Make this machine match the declaration — only within its own ledger
    /// (`declared-applied.json`).
    Reconcile {
        /// One source, or all of them.
        #[serde(default)]
        source: Option<Source>,
    },
    /// Look for newer pins of the declared downloads (AppImages, apks).
    RefreshPins,
    // ── anchor: L18 ──
}

/// One thing a Flatpak app may reach, in the reduced form a person decides
/// on. There is DELIBERATELY no way to set an environment variable: the only
/// variable it touches is `GTK_THEME`, and only to take it away (it replaces
/// an app's own theme with whatever it names — `docs/ARCHITECTURE.md`, "o modo
/// escuro").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "grant"
)]
pub enum Grant {
    /// The person's files: the home folder read-write, read-only, or not at all.
    Files {
        /// How far.
        reach: Reach,
    },
    /// The network.
    Network {
        /// Allowed.
        on: bool,
    },
    /// Playing and recording sound.
    Sound {
        /// Allowed.
        on: bool,
    },
    /// Cameras, game controllers and every other device.
    Devices {
        /// Allowed.
        on: bool,
    },
    /// Which display protocol it draws through.
    Display {
        /// Which.
        through: Display,
    },
    /// Take away a `GTK_THEME` the app was given.
    UnsetGtkTheme,
    /// Back to what the app itself asks for.
    Reset,
}

impl Grant {
    /// Every word, for `software-words-are-total`.
    pub const WORDS: [&'static str; 7] = [
        "files",
        "network",
        "sound",
        "devices",
        "display",
        "unset-gtk-theme",
        "reset",
    ];

    /// The stem of its text key (`software-grant-<word>`).
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Files { .. } => "files",
            Self::Network { .. } => "network",
            Self::Sound { .. } => "sound",
            Self::Devices { .. } => "devices",
            Self::Display { .. } => "display",
            Self::UnsetGtkTheme => "unset-gtk-theme",
            Self::Reset => "reset",
        }
    }
}

/// How far into the person's files an app may reach.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reach {
    /// Read and write the home folder.
    ReadWrite,
    /// Read it only.
    ReadOnly,
    /// Not at all (it keeps what the portal hands it, a file at a time).
    #[default]
    Nothing,
}

impl Reach {
    /// Every word, for `software-words-are-total`.
    pub const WORDS: [&'static str; 3] = ["read-write", "read-only", "nothing"];

    /// The stem of its text key (`software-reach-<word>`).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::ReadWrite => "read-write",
            Self::ReadOnly => "read-only",
            Self::Nothing => "nothing",
        }
    }
}

/// The display protocol an app draws through.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Display {
    /// Wayland, the session's own.
    #[default]
    Wayland,
    /// X11, through Xwayland.
    X11,
}

impl Display {
    /// Every word, for `software-words-are-total`.
    pub const WORDS: [&'static str; 2] = ["wayland", "x11"];

    /// The stem of its text key (`software-display-<word>`).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Wayland => "wayland",
            Self::X11 => "x11",
        }
    }
}

fn yes() -> bool {
    true
}

impl Order {
    /// Every word, for `software-words-are-total`.
    pub const WORDS: [&'static str; 23] = [
        "install",
        "install-file",
        "remove",
        "upgrade",
        "tidy",
        "try",
        "stop-trial",
        "keep",
        "prepare",
        "refresh",
        "rollback",
        "web-create",
        "web-edit",
        "android-session",
        "android-erase",
        "permission",
        "android-windows",
        "declare",
        "forget",
        "decline",
        "undecline",
        "reconcile",
        "refresh-pins",
    ];

    /// The stem of its text key (`software-order-<word>`).
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Install { .. } => "install",
            Self::InstallFile { .. } => "install-file",
            Self::Remove { .. } => "remove",
            Self::Upgrade { .. } => "upgrade",
            Self::Tidy { .. } => "tidy",
            Self::Try { .. } => "try",
            Self::StopTrial { .. } => "stop-trial",
            Self::Keep { .. } => "keep",
            Self::Prepare { .. } => "prepare",
            Self::Refresh { .. } => "refresh",
            Self::Rollback { .. } => "rollback",
            Self::WebCreate { .. } => "web-create",
            Self::WebEdit { .. } => "web-edit",
            Self::AndroidSession { .. } => "android-session",
            Self::Permission { .. } => "permission",
            Self::AndroidWindows { .. } => "android-windows",
            Self::AndroidErase => "android-erase",
            Self::Declare { .. } => "declare",
            Self::Forget { .. } => "forget",
            Self::Decline { .. } => "decline",
            Self::Undecline { .. } => "undecline",
            Self::Reconcile { .. } => "reconcile",
            Self::RefreshPins => "refresh-pins",
        }
    }

    /// The source whose LANE this order takes (`crate::lock`): two orders on
    /// the same source never run at once, from the screen or from the update
    /// run. `None` for an upgrade of everything, which takes every lane in
    /// turn.
    #[must_use]
    pub fn lane(&self) -> Option<Source> {
        match self {
            Self::Install { key, .. } | Self::Remove { key, .. } => key.source(),
            Self::InstallFile { path, .. } => file_source(path),
            Self::Upgrade { scope } => match scope {
                Scope::One { key } => key.source(),
                Scope::Source { source } => Some(*source),
                Scope::Everything => None,
            },
            Self::Tidy { source } | Self::Prepare { source, .. } | Self::Refresh { source } => {
                Some(*source)
            }
            Self::Try { .. }
            | Self::StopTrial { .. }
            | Self::Keep { .. }
            | Self::Rollback { .. } => Some(Source::Nix),
            Self::WebCreate { .. } | Self::WebEdit { .. } => Some(Source::Web),
            Self::AndroidSession { .. } | Self::AndroidErase | Self::AndroidWindows { .. } => {
                Some(Source::Android)
            }
            Self::Permission { .. } => Some(Source::Flatpak),
            Self::Declare { key, .. } | Self::Forget { key, .. } => key.source(),
            Self::Reconcile { source } => *source,
            // THE DISTRO'S PROGRAMS AND THE PINS are no source's lane: they
            // change the person's declaration, and the apply that follows
            // takes the lanes it needs.
            Self::Decline { .. } | Self::Undecline { .. } | Self::RefreshPins => None,
        }
    }

    /// Does this order CHANGE anything? A rehearsal (`.#dev`) runs only the
    /// ones that do not.
    #[must_use]
    pub const fn changes_something(&self) -> bool {
        !matches!(self, Self::Refresh { .. })
    }
}

/// Which source a file belongs to, by its magic and not only by its name —
/// but the name is what decides the lane before the file is read.
#[must_use]
pub fn file_source(path: &std::path::Path) -> Option<Source> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.ends_with(".appimage") {
        Some(Source::AppImage)
    } else if name.ends_with(".apk") {
        Some(Source::Android)
    } else if name.ends_with(".flatpakref") {
        Some(Source::Flatpak)
    } else {
        None
    }
}

/// Which programs an upgrade moves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "scope"
)]
pub enum Scope {
    /// One program.
    One {
        /// Which.
        key: AppKey,
    },
    /// Every program of one source.
    Source {
        /// Which source.
        source: Source,
    },
    /// Everything this person installed.
    Everything,
}

/// What the person has already said yes to, carried IN the order so the CLI
/// never asks (nothing that runs may ask) and never assumes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Consent {
    /// A package whose licence is not free (Nix `meta.unfree`).
    #[serde(default)]
    pub unfree: bool,
    /// A download nothing vouches for (an AppImage release with no digest, a
    /// file from the disk).
    #[serde(default)]
    pub unverified: bool,
    /// An Android app built only for ARM (it will not run without a
    /// translation layer; the person was told).
    #[serde(default)]
    pub arm_only: bool,
    /// A download above the person's threshold.
    #[serde(default)]
    pub big: bool,
}

/// What a trial may leave behind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Traces {
    /// A throwaway home: `HOME` and every `XDG_*_HOME` point into the trial's
    /// own directory, deleted when it ends. The default — trying a program
    /// should not leave its settings in the person's folders.
    #[default]
    Scratch,
    /// The person's own home and settings.
    Home,
    /// A bubblewrap sandbox: the store and `/etc` read-only, a home in tmpfs,
    /// the Wayland and PipeWire sockets, and the network only if allowed.
    Sandbox,
}

impl Traces {
    /// Every one, in the ring's order.
    pub const ALL: [Self; 3] = [Self::Scratch, Self::Home, Self::Sandbox];

    /// The stem of its text key.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Scratch => "scratch",
            Self::Home => "home",
            Self::Sandbox => "sandbox",
        }
    }
}

/// Which Android system image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AndroidImage {
    /// LineageOS with Google's apps — the Play Store works after the device is
    /// registered as uncertified. The owner's choice for the default.
    #[default]
    Gapps,
    /// LineageOS without Google: F-Droid and Aurora Store.
    Vanilla,
}

impl AndroidImage {
    /// Every one, in the ring's order.
    pub const ALL: [Self; 2] = [Self::Gapps, Self::Vanilla];

    /// The stem of its text key, and the instance of the init unit
    /// (`ronoctua-android-init@<UNIT>.service`).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Gapps => "gapps",
            Self::Vanilla => "vanilla",
        }
    }

    /// What `waydroid init -s` calls it, which is also the unit's instance.
    #[must_use]
    pub const fn system_type(self) -> &'static str {
        match self {
            Self::Gapps => "GAPPS",
            Self::Vanilla => "VANILLA",
        }
    }
}

/// A web app, as the editor holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebSpec {
    /// The name in the launcher.
    pub name: String,
    /// The address it opens (http or https only).
    pub url: String,
    /// Chrome's profile DIRECTORY (`Default`, `Profile 1`) — the logins it uses.
    pub profile: String,
    /// Where the icon came from, kept so editing starts from it.
    pub icon: IconChoice,
    /// The icon already normalised to a square PNG by the preview, when the
    /// screen has one; otherwise the CLI resolves [`WebSpec::icon`] itself.
    #[serde(default)]
    pub prepared: Option<PathBuf>,
    /// A freedesktop main category (`Network`, `Office`, …): it picks the
    /// launcher's fallback glyph.
    #[serde(default)]
    pub category: Option<String>,
}

/// Where a web app's icon comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "from"
)]
pub enum IconChoice {
    /// The site's own icon, by the address it was found at.
    Site {
        /// The icon's URL.
        url: String,
        /// The page that NAMED it, when it was found on one — the address
        /// the person typed, on the host they let. An icon on another host
        /// is asked bare on that page's pass (`net::get_named`); without a
        /// page it is asked on a pass of its own host.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
    },
    /// Dashboard Icons (dashboardicons.com): one DRAWING of one icon.
    Dashboard {
        /// The drawing's FILE name, without its extension — what the CDN
        /// serves as `svg/<name>.svg` and `png/<name>.png`
        /// (`dashboard::url_of`): the icon's own name for its base drawing
        /// (`github`), or the file the metadata names for a variant
        /// (`github-light`), read from the metadata and never built.
        name: String,
        /// Which drawing of the metadata's entry it is — the metadata's own
        /// key (`dashboard::Icon::drawings`): `""` for the base, `"light"`
        /// for `colors.light` (the LIGHT-coloured drawing, made for a DARK
        /// background) or `"dark"` for `colors.dark` (for a light one).
        #[serde(default)]
        variant: String,
    },
    /// The distro's collection (the icon superpack), by icon name.
    Collection {
        /// The icon name.
        name: String,
    },
    /// A file the person gave: dropped, typed, pasted or picked.
    File {
        /// Where it was.
        path: PathBuf,
    },
}

impl IconChoice {
    /// The stem of its text key (`software-shelf-<word>`).
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Site { .. } => "site",
            Self::Dashboard { .. } => "dashboard",
            Self::Collection { .. } => "collection",
            Self::File { .. } => "file",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_order() -> Vec<Order> {
        let key = AppKey::Nix {
            attr: "hello".into(),
        };
        let spec = WebSpec {
            name: "Mail".into(),
            url: "https://mail.example.org/".into(),
            profile: "Default".into(),
            icon: IconChoice::Site {
                url: "https://mail.example.org/icon.png".into(),
                page: None,
            },
            prepared: None,
            category: None,
        };
        vec![
            Order::Install {
                key: key.clone(),
                consent: Consent::default(),
            },
            Order::InstallFile {
                path: "/tmp/x.AppImage".into(),
                consent: Consent::default(),
            },
            Order::Remove {
                key,
                keep_data: true,
            },
            Order::Upgrade {
                scope: Scope::Everything,
            },
            Order::Tidy {
                source: Source::Flatpak,
            },
            Order::Try {
                attr: "hello".into(),
                traces: Traces::Scratch,
                network: true,
                consent: Consent::default(),
            },
            Order::StopTrial {
                trial: "hello".into(),
            },
            Order::Keep {
                trial: "hello".into(),
            },
            Order::Prepare {
                source: Source::Android,
                image: Some(AndroidImage::Gapps),
            },
            Order::Refresh {
                source: Source::Flatpak,
            },
            Order::Rollback { generation: 3 },
            Order::WebCreate { spec: spec.clone() },
            Order::WebEdit {
                slug: "mail".into(),
                spec,
            },
            Order::AndroidSession { on: true },
            Order::AndroidErase,
            Order::Permission {
                id: "org.gnome.gedit".into(),
                grant: Grant::Files {
                    reach: Reach::ReadOnly,
                },
            },
            Order::AndroidWindows { multi: false },
            Order::Declare {
                key: AppKey::Flatpak {
                    id: "org.gnome.Calculator".into(),
                },
                consent: Consent::default(),
                machines: Some(vec!["noctua".into()]),
            },
            Order::Forget {
                key: AppKey::Nix {
                    attr: "hello".into(),
                },
                keep_data: true,
            },
            Order::Decline {
                id: "gimp".into(),
                machines: None,
            },
            Order::Undecline {
                id: "gimp".into(),
                everywhere: true,
            },
            Order::Reconcile { source: None },
            Order::RefreshPins,
            // ── anchor: L18 ──
        ]
    }

    #[test]
    fn every_grant_has_its_word_and_travels_as_json() {
        let all = [
            Grant::Files {
                reach: Reach::Nothing,
            },
            Grant::Network { on: false },
            Grant::Sound { on: true },
            Grant::Devices { on: false },
            Grant::Display {
                through: Display::X11,
            },
            Grant::UnsetGtkTheme,
            Grant::Reset,
        ];
        assert_eq!(all.len(), Grant::WORDS.len());
        for g in all {
            assert!(Grant::WORDS.contains(&g.word()));
            let text = serde_json::to_string(&g).unwrap();
            assert_eq!(serde_json::from_str::<Grant>(&text).unwrap(), g);
        }
        let read_only = serde_json::to_string(&Grant::Files {
            reach: Reach::ReadOnly,
        })
        .unwrap();
        assert_eq!(read_only, r#"{"grant":"files","reach":"read-only"}"#);
        for r in [Reach::ReadWrite, Reach::ReadOnly, Reach::Nothing] {
            assert!(Reach::WORDS.contains(&r.word()));
        }
        for d in [Display::Wayland, Display::X11] {
            assert!(Display::WORDS.contains(&d.word()));
        }
    }

    #[test]
    fn every_order_has_its_word_in_the_list_and_travels_as_json() {
        let all = every_order();
        assert_eq!(all.len(), Order::WORDS.len());
        for o in all {
            assert!(
                Order::WORDS.contains(&o.word()),
                "{} is not in WORDS",
                o.word()
            );
            let text = serde_json::to_string(&o).unwrap();
            assert_eq!(serde_json::from_str::<Order>(&text).unwrap(), o);
        }
    }

    /// FAILS IF: a bring-back queued before `everywhere` existed reads as
    /// one on every computer — or at all differently from "here".
    #[test]
    fn bringing_back_is_here_unless_asked_everywhere() {
        let o: Order = serde_json::from_str(r#"{"order":"undecline","id":"gimp"}"#).unwrap();
        assert_eq!(
            o,
            Order::Undecline {
                id: "gimp".into(),
                everywhere: false
            }
        );
    }

    #[test]
    fn removing_keeps_the_data_unless_asked_not_to() {
        let o: Order =
            serde_json::from_str(r#"{"order":"remove","key":{"source":"nix","attr":"x"}}"#)
                .unwrap();
        assert_eq!(
            o,
            Order::Remove {
                key: AppKey::Nix { attr: "x".into() },
                keep_data: true
            }
        );
        // The fields of a variant are kebab-case on the wire, like every other
        // name this crate writes: one spelling, so nobody writing JSON by hand
        // has to know which half of it is snake.
        let text = serde_json::to_string(&Order::Remove {
            key: AppKey::System {
                desktop_id: "a.desktop".into(),
            },
            keep_data: false,
        })
        .unwrap();
        assert!(
            text.contains("\"keep-data\":false") && text.contains("\"desktop-id\""),
            "{text}"
        );
    }

    #[test]
    fn a_file_takes_the_lane_of_what_it_is() {
        assert_eq!(
            file_source("/d/Obsidian-1.6.AppImage".as_ref()),
            Some(Source::AppImage)
        );
        assert_eq!(file_source("/d/fdroid.APK".as_ref()), Some(Source::Android));
        assert_eq!(
            file_source("/d/x.flatpakref".as_ref()),
            Some(Source::Flatpak)
        );
        assert_eq!(file_source("/d/notes.txt".as_ref()), None);
    }
}
