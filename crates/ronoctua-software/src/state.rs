//! What the person has, read from REALITY every time.
//!
//! There is no ledger of "what we installed" to drift from the machine: the
//! Nix profile's manifest, `flatpak list --user`, the AppImages directory,
//! `waydroid app list` and the web apps' `.desktop` files ARE the list. A
//! program installed from a terminal shows up here too, and one removed by
//! hand disappears — which is the only way the screen can be believed.
//!
//! Reading runs programs (`nix profile list`, `flatpak list`), so the screen
//! calls [`read`] on a THREAD, never on its event loop.

use serde::{Deserialize, Serialize};

use crate::found::Picture;
use crate::order::{AndroidImage, Traces};
use crate::{AppKey, Context, Refusal, Source};

/// One program the person has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Installed {
    /// Its identity.
    pub key: AppKey,
    /// Its name (data).
    pub name: String,
    /// The version installed, as its source says it (never parsed: inv. 5).
    #[serde(default)]
    pub version: String,
    /// The `.desktop` ids it put in the launcher, so "open" can be offered.
    #[serde(default)]
    pub desktop_ids: Vec<String>,
    /// What it takes on disk, when known.
    #[serde(default)]
    pub size_bytes: Option<u64>,
    /// The version an update would bring, when one is known.
    #[serde(default)]
    pub update: Option<String>,
    /// Its licence is not free.
    #[serde(default)]
    pub unfree: bool,
    /// Who put it there.
    #[serde(default)]
    pub origin: Origin,
    /// Its picture.
    #[serde(default)]
    pub icon: Option<Picture>,
    /// Something is wrong with it (an AppImage whose file vanished, an
    /// Android app while the session is down).
    #[serde(default)]
    pub trouble: Option<Refusal>,
}

/// Who put a program there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", rename_all_fields = "kebab-case")]
pub enum Origin {
    /// The person, through this crate or by hand, on this machine only: no
    /// declaration names it, and the update neither keeps nor brings it.
    #[default]
    Mine,
    /// The person's declaration names it (`declared.json`): it follows them
    /// to every machine the entry names, and removing it here alone would be
    /// undone — it is removed FOR GOOD, from the declaration
    /// ([`crate::Order::Forget`]). Also what the declaration named until the
    /// system this machine runs was built, and leaves at its next apply: its
    /// trouble is [`Refusal::LeavesAtNextApply`], and removing it for good
    /// asks again for the rebuild that takes it.
    MineDeclared,
    /// One of the distro's programs (`ronoctua.apps`, `distro-apps.json`). A
    /// declinable one can be kept uninstalled, here or on every machine of
    /// the person ([`crate::Order::Decline`]); an essential one says why not.
    Distro {
        /// Whether it may be declined.
        declinable: bool,
    },
    /// The flake installed it for this machine otherwise (a `host.nix` line,
    /// through nix-flatpak's user installation): removing it here would be
    /// undone at the next rebuild, so it is refused with a sentence.
    Declared,
}

impl Origin {
    /// Every kind, a distro program once of each, for the words' census.
    pub const ALL: [Self; 5] = [
        Self::Mine,
        Self::MineDeclared,
        Self::Distro { declinable: true },
        Self::Distro { declinable: false },
        Self::Declared,
    ];
}

/// A trial: a Nix package running without being installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Trial {
    /// Its key (the attribute, folded) — the unit is `ronoctua-try-<key>`.
    pub key: String,
    /// The nixpkgs attribute.
    pub attr: String,
    /// The name to show.
    pub name: String,
    /// Whether its unit is alive.
    pub running: bool,
    /// When it started (seconds since the epoch), when known.
    #[serde(default)]
    pub since: Option<u64>,
    /// What it may leave behind.
    pub traces: Traces,
    /// The command a person types, for a command-line trial (it opens a
    /// shell with the package on `PATH`, so the screen says what to type).
    #[serde(default)]
    pub program: Option<String>,
    /// It runs inside a terminal.
    #[serde(default)]
    pub terminal: bool,
}

/// Android, as it stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Android {
    /// This machine has Waydroid at all (the full profile).
    pub support: bool,
    /// The image has been downloaded (`waydroid.cfg` exists).
    pub prepared: bool,
    /// The preparing unit is running right now.
    pub preparing: bool,
    /// Which image it has, when known.
    #[serde(default)]
    pub image: Option<AndroidImage>,
    /// A session is running (it costs memory; the modeline shows a mark).
    pub session: bool,
    /// Each app in its own window.
    #[serde(default)]
    pub multi_window: Option<bool>,
    /// The id to register at Google's uncertified-device page (GAPPS only).
    #[serde(default)]
    pub android_id: Option<String>,
}

/// One generation of the Nix profile, for "go back".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Generation {
    /// Its number, as `nix profile history` gives it.
    pub number: u64,
    /// When it was made (seconds since the epoch).
    pub when: u64,
    /// It is the one in use.
    pub current: bool,
    /// What changed, as data (`+gimp -vlc`).
    pub summary: String,
}

/// Which sources this machine supports at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Supports {
    /// `nix profile` (always, on this distro).
    pub nix: bool,
    /// Flatpak.
    pub flatpak: bool,
    /// AppImage (`appimage-run`).
    pub appimage: bool,
    /// Android (Waydroid).
    pub android: bool,
    /// Web apps (Chrome).
    pub web: bool,
}

impl Supports {
    /// What the tools say.
    #[must_use]
    pub fn of(tools: &crate::Tools) -> Self {
        Self {
            nix: tools.nix.is_some(),
            flatpak: tools.flatpak.is_some(),
            appimage: tools.appimage_run.is_some() && tools.unsquashfs.is_some(),
            android: tools.waydroid.is_some(),
            web: tools.chrome.is_some(),
        }
    }

    /// Whether one source is supported.
    #[must_use]
    pub const fn has(self, source: Source) -> bool {
        match source {
            Source::Nix => self.nix,
            Source::Flatpak => self.flatpak,
            Source::AppImage => self.appimage,
            Source::Android => self.android,
            Source::Web => self.web,
        }
    }
}

/// Everything, at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Snapshot {
    /// What the person has, sorted by source and then by folded name.
    pub installed: Vec<Installed>,
    /// Trials, running or ended this session.
    pub trials: Vec<Trial>,
    /// Android's state.
    pub android: Android,
    /// The Nix profile's generations, newest first.
    pub generations: Vec<Generation>,
    /// Which sources this machine has.
    pub supports: Supports,
    /// The nixpkgs revision the search index comes from.
    #[serde(default)]
    pub nix_index_rev: Option<String>,
    /// The person's declaration and how this machine stands to it: what is
    /// declared and declined, what was applied, skipped or waits, and the
    /// distro's programs ([`crate::declaration`]).
    #[serde(default)]
    pub declarations: crate::declaration::Declarations,
}

/// Read everything. Runs programs: call it on a thread.
#[must_use]
pub fn read(ctx: &Context) -> Snapshot {
    read_with(ctx, &crate::declaration::this_host())
}

/// [`read`], on a machine called `host` — a test's, or a photograph's.
#[must_use]
pub fn read_with(ctx: &Context, host: &str) -> Snapshot {
    let supports = Supports::of(&ctx.tools);
    let android = if supports.android {
        crate::android::status(ctx)
    } else {
        Android::default()
    };
    let mut installed = Vec::new();
    if supports.nix {
        installed.extend(crate::nix::installed(ctx));
    }
    if supports.flatpak {
        installed.extend(crate::flatpak::installed(ctx));
    }
    if supports.appimage {
        installed.extend(crate::appimage::installed(ctx));
    }
    if supports.android {
        installed.extend(crate::android::installed(ctx, &android));
    }
    installed.extend(crate::web::installed(&ctx.roots));
    // WHO PUT EACH THERE, and the declared copies the lists above cannot
    // see: a declared Nix package is in the person's `home.packages`, not in
    // their profile; a declared AppImage or web app is the system's entry,
    // not a file of `~/Applications` or `~/.local/share/applications`.
    let declarations = crate::declaration::read(&ctx.roots, host);
    crate::declaration::shape(&mut installed, &declarations, &ctx.roots);
    installed.sort_by(|a, b| {
        (a.key.source(), ronoctua_apps::fold::fold(&a.name))
            .cmp(&(b.key.source(), ronoctua_apps::fold::fold(&b.name)))
    });
    Snapshot {
        installed,
        trials: if supports.nix {
            crate::trial::trials(ctx)
        } else {
            Vec::new()
        },
        generations: if supports.nix {
            crate::nix::generations(ctx)
        } else {
            Vec::new()
        },
        android: Android {
            support: supports.android,
            ..android
        },
        supports,
        nix_index_rev: ctx.tools.nixpkgs_rev.clone(),
        declarations,
    }
}
