//! Software (`LEADER i`) — the person's own programs: search, install, try,
//! update and remove them, for this user only. One view, two doors.
//!
//! # What is here
//!
//! Everything the screen DECIDES: which rows each level has, what a key does
//! to them, and what has to be done as a result. Nothing here runs a program,
//! opens a socket or knows what a window is. `ronoctua-software` is the
//! boundary with the package managers; the queue (`ronoctua-transfers`) runs
//! every order that changes the machine; `software_feed.rs` fetches on threads;
//! the footer's region (`software_region.rs`) and the window
//! (`software_window.rs`) are the two doors, and what both do for the view is
//! `software_reading.rs`. The shape
//! `syncthing.rs` gave levels to, and `geo.rs` gave a question.
//!
//! # The view holds no readings
//!
//! What the person has, what a search found, a sheet's details, the queue's
//! jobs — all of it arrives in a [`Readings`] the door owns and hands in on
//! every call. What the view keeps is where it is: the level, the cursor (by
//! IDENTITY, a [`Spot`], never an index), the query, a web app being drafted
//! and the question being asked. So data arriving from a thread never moves
//! the cursor: the row it stood on is still the row with that spot.
//!
//! # Every order is a question to the queue, never a program run here
//!
//! A key that changes the machine becomes an [`Errand::Queue`] with the crate's
//! own [`Order`]; the door sends it as `Work::Software` and the queue shows its
//! progress, its refusal and its end. The only things a door does directly
//! are the harmless ones: launching what is installed, opening a web app once,
//! copying a line, opening a page.
//!
//! # A refusal is a value, and a question opens on "no"
//!
//! A row that cannot act is drawn, refusing, and SAYS WHY — the crate's
//! [`Refusal`] and its sentence (`software_words`). Anything that cannot be
//! undone, or that downloads more than the person allowed, is asked first, and
//! every question's first answer is "no", with the cursor born on it.

use std::path::{Path, PathBuf};

use ronoctua_render::layout;
use ronoctua_software::declaration::{
    Declarations, DeclaredRow, DeclineState, DeclinedRow, DistroRow, Standing,
};
use ronoctua_software::details::Details;
use ronoctua_software::found::{Badge, Found, Prefer, Status};
use ronoctua_software::order::{
    AndroidImage, Consent, Display, Grant, IconChoice, Reach, Scope, Traces, WebSpec,
};
use ronoctua_software::prefs::Preferences;
use ronoctua_software::state::{Installed, Origin, Snapshot, Trial};
use ronoctua_software::{AppKey, Order, Refusal, Source};
use ronoctua_tokens as tokens;

use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::launch::same_desktop;
use crate::software_feed::{Group, Readings, Want};
use crate::software_pictures::Portrait;
use crate::software_words as words;

/// The keys this view teaches, and where each one applies.
///
/// EVERY COMMAND IS `C-`, because in both doors a bare letter is TEXT: the
/// view opens typing, the way the launcher does, and `i` for install would be
/// the first letter nobody could search for. `M-` is the compositor's family
/// and never reaches a window.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "software-down", Where::Anywhere),
    ("C-n", "software-down", Where::Anywhere),
    ("<up>", "software-up", Where::Anywhere),
    ("C-p", "software-up", Where::Anywhere),
    // `RET` REVEALS, and on a fact it COPIES — the rule of every view here.
    ("RET", "software-enter", Where::Anywhere),
    ("TAB", "software-next-source", Where::WithSources),
    ("S-TAB", "software-previous-source", Where::WithSources),
    ("S-<left>", "software-less", Where::OnAValue),
    ("S-<right>", "software-more", Where::OnAValue),
    // THE VERBS, straight from a list: whoever knows what they want skips the
    // sheet. Each is taught only on a row it works on.
    ("C-RET", "software-install", Where::OnInstallable),
    ("C-t", "software-try-it", Where::OnTryable),
    ("C-o", "software-launch", Where::OnLaunchable),
    ("C-u", "software-update", Where::OnUpdatable),
    ("C-e", "software-edit", Where::OnAWebApp),
    ("C-d", "software-remove", Where::OnRemovable),
    ("C-k", "software-stop", Where::OnARunning),
    ("C-v", "software-paste", Where::WhileTyping),
    ("C-y", "software-paste", Where::WhileTyping),
];

/// Which rows a key means anything on. The hint line teaches the keys that
/// WORK where the cursor is, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a level with a ring of sources or shelves to turn.
    WithSources,
    /// On a row whose value turns.
    OnAValue,
    /// On something that can be installed, or a trial that can be kept.
    OnInstallable,
    /// On a Nix package that can run without being installed.
    OnTryable,
    /// On something installed that opens.
    OnLaunchable,
    /// On something with an update waiting.
    OnUpdatable,
    /// On a web app.
    OnAWebApp,
    /// On something that can be removed, or a consent that can be taken back.
    OnRemovable,
    /// On a trial, or the Android session, while it runs.
    OnARunning,
    /// While letters are being typed into the search, in either door.
    WhileTyping,
}

impl Where {
    /// Whether a key is worth teaching on this row, in this view.
    fn shown(self, row: Option<&Row>, view: &View) -> bool {
        let can = row.map(|r| &r.can);
        match self {
            Self::Anywhere => true,
            Self::WithSources => {
                view.asking.is_none()
                    && matches!(view.level, Level::Home | Level::Results | Level::Icons(_))
            }
            Self::OnAValue => {
                row.is_some_and(|r| matches!(r.target, Target::Ring(_)) && !r.disabled)
            }
            Self::OnInstallable => can.is_some_and(|c| c.install.is_some()),
            Self::OnTryable => can.is_some_and(|c| c.try_it.is_some()),
            Self::OnLaunchable => can.is_some_and(|c| c.launch.is_some()),
            Self::OnUpdatable => can.is_some_and(|c| c.update.is_some()),
            Self::OnAWebApp => can.is_some_and(|c| c.edit.is_some()),
            Self::OnRemovable => can.is_some_and(|c| c.remove.is_some()),
            Self::OnARunning => can.is_some_and(|c| c.stop.is_some()),
            Self::WhileTyping => view.takes_letters(),
        }
    }
}

/// A key, once the door has read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Do what this row does.
    Enter,
    /// The next source (or shelf) in the level's ring.
    NextSource,
    /// The previous one.
    PreviousSource,
    /// The previous value on this row.
    Less,
    /// The next value on this row.
    More,
    /// Install it (or keep the trial).
    Install,
    /// Run it without installing.
    Try,
    /// Open it.
    Launch,
    /// Update it.
    Update,
    /// Edit the web app.
    Edit,
    /// Remove it, after a question.
    Remove,
    /// Stop the trial, or the Android session.
    Stop,
    /// Paste into the search.
    Paste,
    /// Back one level, out of a question, and out.
    Escape,
}

/// Which key a command id is — the ONE map both doors reach.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "software-up" => Press::Up,
        "software-down" => Press::Down,
        "software-enter" => Press::Enter,
        "software-next-source" => Press::NextSource,
        "software-previous-source" => Press::PreviousSource,
        "software-less" => Press::Less,
        "software-more" => Press::More,
        "software-install" => Press::Install,
        "software-try-it" => Press::Try,
        "software-launch" => Press::Launch,
        "software-update" => Press::Update,
        "software-edit" => Press::Edit,
        "software-remove" => Press::Remove,
        "software-stop" => Press::Stop,
        "software-paste" => Press::Paste,
        _ => return None,
    })
}

/// Which key a notation is, through [`OF_THE_VIEW`]. `ESC` is the way back,
/// which every view has and no table teaches (`chrome::with_the_way_back`).
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    if notation == "ESC" {
        return Some(Press::Escape);
    }
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

/// What a key is to this view, once read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    /// A key of the view's own.
    Press(Press),
    /// A letter for the search.
    Letter(char),
    /// One letter off the search.
    Erase,
}

/// A key, by its WHOLE notation, as this view reads it — the one reader both
/// doors call, so they answer one key the same way.
///
/// The view's table first, by notation — `C-RET`, `S-<left>`, `TAB`, the
/// arrows, `ESC` — and only then the minibuffer's gestures, where a letter is
/// typing and `C-g` is `Esc`. `None` for a key with no role here.
#[must_use]
pub fn stroke_of(key: ronoctua_keymap::Key) -> Option<Stroke> {
    use crate::minibuffer::Gesture;
    if let Some(press) = pressed(&key.notation()) {
        return Some(Stroke::Press(press));
    }
    match crate::minibuffer::gesture_of(key) {
        // `C-g` IS `Esc`, as in every list with a search field.
        Gesture::Cancel => Some(Stroke::Press(Press::Escape)),
        Gesture::Delete => Some(Stroke::Erase),
        Gesture::Insert(c) => Some(Stroke::Letter(c)),
        Gesture::Confirm | Gesture::Down | Gesture::Up | Gesture::Nothing => None,
    }
}

/// The notation a command id is taught under, from the table — for the
/// sentences that name a key of this view (`keys-are-never-text`).
#[must_use]
pub fn notation_of(id: &str) -> Option<&'static str> {
    OF_THE_VIEW
        .iter()
        .find(|(_, i, _)| *i == id)
        .map(|(k, _, _)| *k)
}

/// Where a web app's icon is looked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shelf {
    /// The site's own: its manifest, its touch icon, its favicon.
    Site,
    /// Dashboard Icons, by name.
    Dashboard,
    /// The distro's collection (the icon superpack).
    Collection,
    /// A file the person brings.
    File,
}

impl Shelf {
    /// Every one, in the ring's order.
    pub const ALL: [Self; 4] = [Self::Site, Self::Dashboard, Self::Collection, Self::File];

    /// The next or the previous one.
    #[must_use]
    fn turned(self, forward: bool) -> Self {
        turned(&Self::ALL, &self, forward)
    }

    /// Which shelf an icon came from.
    #[must_use]
    pub const fn of(choice: &IconChoice) -> Self {
        match choice {
            IconChoice::Site { .. } => Self::Site,
            IconChoice::Dashboard { .. } => Self::Dashboard,
            IconChoice::Collection { .. } => Self::Collection,
            IconChoice::File { .. } => Self::File,
        }
    }
}

/// Where the view opens. Each leader key, the launcher and a file opened from
/// the manager land on a different one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// The first page (`i i`, `i w`).
    Home,
    /// A search, already typed (the launcher's "search to install").
    Results(String),
    /// What is being tried (`i t`).
    Trials,
    /// Updates, space and going back (`i u`).
    Maintenance,
    /// Android (`i a`).
    Android,
    /// The advanced page (`software advanced` in either door: the window's
    /// command line, and `software-open advanced` in the footer).
    Advanced,
    /// What the person declared (`software declared`).
    Declared,
    /// What the person declined of the distro's, and the distro's programs
    /// (`software declined`).
    Declined,
    /// A new web app from this address (`i b`, once it is typed).
    WebNew(String),
    /// One program's sheet (the launcher's `C-i`).
    Sheet {
        /// Which.
        key: AppKey,
        /// Its name, as the launcher has it.
        name: String,
    },
    /// One program's sheet with the remove question already up, on "no" (the
    /// launcher's `C-d`).
    Remove {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
    },
    /// A file to install (`.AppImage`, `.apk`, `.flatpakref`) — the window's
    /// opening for the file manager's double click (`ronoctua-shell
    /// software-file`). A dropped file goes through `View::dropped`.
    File(PathBuf),
}

/// Which page of the view is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Level {
    /// The first page: search, what is installed, what is being tried, doors.
    Home,
    /// What a search found.
    Results,
    /// One program's sheet.
    Sheet(AppKey),
    /// What is being tried.
    Trials,
    /// A web app being made or changed (the draft is the view's).
    WebEditor,
    /// Choosing the draft's icon, from one shelf.
    Icons(Shelf),
    /// Android.
    Android,
    /// Updates, space, going back.
    Maintenance,
    /// Sources, preferences, the rarely-touched.
    Advanced,
    /// One Flatpak's permissions.
    Permissions(String),
    /// The non-free packages allowed.
    Consents,
    /// What was done.
    History,
    /// What the person declared, and how this machine stands to each.
    Declared,
    /// One declared entry, by its id (`nix:ripgrep`): its pin, its standing
    /// and the fixes for it.
    Declaration(String),
    /// What the person declined, and the distro's programs.
    Declined,
    /// One of the distro's programs, by its registry id.
    Distro(String),
}

impl Level {
    /// The spot's own name for a door onto this level.
    const fn door_id(&self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Results => "results",
            Self::Sheet(_) => "sheet",
            Self::Trials => "trials",
            Self::WebEditor => "web-editor",
            Self::Icons(_) => "icons",
            Self::Android => "android",
            Self::Maintenance => "maintenance",
            Self::Advanced => "advanced",
            Self::Permissions(_) => "permissions",
            Self::Consents => "consents",
            Self::History => "history",
            Self::Declared => "declared",
            Self::Declaration(_) => "declaration",
            Self::Declined => "declined",
            Self::Distro(_) => "distro",
        }
    }
}

/// Why the person is searching: to have it, or to try it. A sheet opened with
/// the second one is born with the cursor on "try".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Intent {
    /// To look, and maybe install.
    #[default]
    Browse,
    /// To try.
    Try,
}

/// A web app as the editor holds it, before it is saved.
///
/// Every field the person has not touched stays EMPTY and is shown from what
/// arrived — the page's own name, its best icon, Chrome's last profile — so a
/// late answer fills the screen without overwriting what somebody typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebDraft {
    /// The web app being changed, or `None` for a new one.
    pub slug: Option<String>,
    /// The name typed.
    pub name: String,
    /// The address, canonical.
    pub url: String,
    /// The Chrome profile chosen.
    pub profile: Option<String>,
    /// The icon chosen.
    pub icon: Option<IconChoice>,
    /// The category chosen.
    pub category: Option<String>,
    /// Something was changed since it opened.
    pub changed: bool,
    /// The existing web app's file was read into it.
    pub adopted: bool,
}

/// Something typed on a door's line — the footer's minibuffer, or a
/// window's header (`door::Ui::ask`); both are a `crate::line::Line` — and
/// which command carries it.
///
/// THE COMMAND THAT ASKS IS THE COMMAND THAT ANSWERS (`syncthing.rs`'s rule):
/// the question is that command's own `ask-<id>` key, and the answer arrives
/// as its argument, through [`View::typed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// A new web app's address.
    WebNew,
    /// The draft's address.
    WebAddress,
    /// The draft's name.
    WebName,
    /// The path of an image for the icon.
    IconPath,
    /// The path of a file to install.
    FilePath,
    /// The belongings' passphrase, to sign a change of the declaration
    /// before it goes (the plain-signature package): asked at the gesture
    /// when the session keeps no identity — the person is there.
    Sign,
}

impl Ask {
    /// Which command asks, and answers.
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::WebNew => "software-web-new",
            Self::WebAddress => "software-web-address",
            Self::WebName => "software-web-name",
            Self::IconPath => "software-icon-path",
            Self::FilePath => "software-file-path",
            Self::Sign => "software-sign",
        }
    }

    /// Whether the line hides what is typed: the passphrase, and only it.
    #[must_use]
    pub const fn secret(self) -> bool {
        matches!(self, Self::Sign)
    }

    /// The command an id is, when it is one of these.
    #[must_use]
    pub fn of_command(id: &str) -> Option<Self> {
        [
            Self::WebNew,
            Self::WebAddress,
            Self::WebName,
            Self::IconPath,
            Self::FilePath,
            Self::Sign,
        ]
        .into_iter()
        .find(|a| a.command() == id)
    }
}

/// What the file picker is up for, in this view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picking {
    /// An image for the web app's icon.
    Icon,
    /// A package file to install: an `.AppImage`, an `.apk`, a
    /// `.flatpakref` (`order::file_source`).
    Package,
}

impl Picking {
    /// The request the picker opens with.
    fn request(self) -> crate::file_picker::Request {
        use crate::file_picker::{Mode, PICTURES, Request, Start, Xdg};
        let (purpose, filters, place) = match self {
            Self::Icon => ("software-icon", PICTURES, Xdg::PICTURES),
            Self::Package => ("software-package", PACKAGE_FILES, Xdg::DOWNLOAD),
        };
        Request {
            purpose,
            mode: Mode::Open { many: false },
            filters,
            start: Start::Place(place),
            addresses: false,
            extras: &[],
            alternate: None,
        }
    }

    /// The key of the picker's title.
    const fn title(self) -> &'static str {
        match self {
            Self::Icon => crate::key::SOFTWARE_ICON_TYPED,
            Self::Package => crate::key::SOFTWARE_PICK_PACKAGE,
        }
    }
}

/// The files a source installs, as the file picker shows them.
const PACKAGE: crate::file_picker::Filter = crate::file_picker::Filter {
    label: crate::key::FILE_PICKER_FILTER_PACKAGES,
    keeps: |path, _| {
        ronoctua_software::order::file_source(path).map(|_| ronoctua_tokens::icon::FILE)
    },
};

/// The ring of a package file.
const PACKAGE_FILES: &[crate::file_picker::Filter] = &[PACKAGE, crate::file_picker::Filter::ANY];

/// Which consent a question asks for — the crate's [`Consent`] fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentKind {
    /// A licence that is not free.
    Unfree,
    /// A download nothing vouches for.
    Unverified,
    /// More than the person's threshold.
    Big,
    /// An Android app built only for ARM.
    ArmOnly,
}

impl ConsentKind {
    /// The consent with this one said yes to.
    const fn granted(self, mut c: Consent) -> Consent {
        match self {
            Self::Unfree => c.unfree = true,
            Self::Unverified => c.unverified = true,
            Self::Big => c.big = true,
            Self::ArmOnly => c.arm_only = true,
        }
        c
    }

    /// Whether this one is already said yes to.
    const fn given(self, c: Consent) -> bool {
        match self {
            Self::Unfree => c.unfree,
            Self::Unverified => c.unverified,
            Self::Big => c.big,
            Self::ArmOnly => c.arm_only,
        }
    }

    /// The consent a refusal of the CLI asks for, when it asks for one.
    const fn of_refusal(r: Refusal) -> Option<Self> {
        match r {
            Refusal::UnfreeNeedsConsent => Some(Self::Unfree),
            Refusal::UnverifiedNeedsConsent => Some(Self::Unverified),
            Refusal::BigNeedsConsent => Some(Self::Big),
            Refusal::ArmOnly => Some(Self::ArmOnly),
            _ => None,
        }
    }
}

/// A question the region asks instead of its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// Say yes to something before a deed goes to the queue.
    Consent {
        /// Which yes.
        kind: ConsentKind,
        /// What is done once it is given.
        deed: Box<Deed>,
        /// The program's name.
        name: String,
        /// How much it downloads, when that is what is asked.
        bytes: Option<u64>,
    },
    /// Remove a program; the second yes erases what it saved too.
    Remove {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
    },
    /// Remove a web app.
    RemoveWeb {
        /// Which.
        slug: String,
        /// Its name.
        name: String,
    },
    /// Go back to an earlier version of the Nix list.
    Rollback(u64),
    /// Free what nothing uses, of one source or of all.
    CleanUp(Option<Source>),
    /// Download Android's image.
    PrepareAndroid(AndroidImage),
    /// Erase Android and everything in it.
    EraseAndroid,
    /// Install a file nobody vouched for.
    Foreign(PathBuf),
    /// Leave a web app draft unsaved.
    LeaveDraft,
    /// Change a Flatpak's permissions in a way the page cannot turn back:
    /// every change undone at once, or `GTK_THEME` taken away.
    Permission {
        /// The app id.
        id: String,
        /// Its name.
        name: String,
        /// What changes.
        grant: Grant,
    },
    /// Remove a declared program for good: the explanation screen — here
    /// and from the declaration, the other computers at their next update,
    /// and what it saved kept unless the second yes erases it.
    Forget {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
    },
    /// Keep one of the distro's programs uninstalled: this computer is
    /// rebuilt without it.
    Decline {
        /// The registry's id.
        id: String,
        /// Its name.
        name: String,
        /// On every computer of the person's, or this one.
        everywhere: bool,
    },
}

/// One answer to a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// No — always the first, and the cursor is born on it.
    No,
    /// Yes.
    Yes,
    /// Yes, and the danger-coloured more of it (erase the data too).
    YesAndMore,
}

/// Something done rather than changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deed {
    /// Install for this person.
    Install {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
        /// What was already said yes to.
        consent: Consent,
    },
    /// Install a file.
    InstallFile {
        /// The file.
        path: PathBuf,
        /// What was already said yes to.
        consent: Consent,
    },
    /// Run a Nix package without installing it.
    Try {
        /// The attribute.
        attr: String,
        /// Its name.
        name: String,
        /// What was already said yes to.
        consent: Consent,
    },
    /// Open an installed program, by its desktop id.
    Launch(String),
    /// Update one program.
    Update {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
    },
    /// Update every program of one source.
    UpdateSource(Source),
    /// Update everything.
    UpdateEverything,
    /// Fetch a source's catalogue again.
    Refresh(Source),
    /// Remove, after a question.
    Remove {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
    },
    /// Keep a trial: install what was tried.
    Keep {
        /// The trial's key.
        trial: String,
        /// Its name.
        name: String,
    },
    /// Stop a trial.
    Stop {
        /// The trial's key.
        trial: String,
        /// Its name.
        name: String,
    },
    /// Start or stop the Android session.
    Session(bool),
    /// Make a source ready (Flathub's remote and catalogue).
    Prepare(Source),
    /// Download Android's image, after a question.
    PrepareAndroid,
    /// Erase Android, after a question.
    EraseAndroid,
    /// Free space, after a question: one source, or all.
    Tidy(Option<Source>),
    /// Go back to a version of the Nix list, after a question.
    Rollback(u64),
    /// Install for this person and KEEP it: declared, on every computer of
    /// theirs (or the ones named) — "install only for you", one click, the
    /// consents asked first as an install asks them.
    DeclareForMe {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
        /// What was already said yes to.
        consent: Consent,
        /// `None` for every computer of the person, or the hostnames.
        machines: Option<Vec<String>>,
    },
    /// Remove for good — here and from the declaration — after the
    /// explanation.
    ForgetForever {
        /// Which.
        key: AppKey,
        /// Its name.
        name: String,
    },
    /// Keep one of the distro's programs uninstalled, after a question.
    KeepUninstalled {
        /// The registry's id.
        id: String,
        /// Its name.
        name: String,
        /// On every computer of the person's, or this one.
        everywhere: bool,
    },
    /// Bring a declined program of the distro back.
    BringBack {
        /// The registry's id.
        id: String,
        /// Its name.
        name: String,
    },
    /// Look for newer downloads of the declared programs.
    RefreshPins,
    /// Search for something to take the place of a program this computer
    /// leaves out.
    FindReplacement(String),
    /// Change a web app.
    Edit(String),
    /// Make a web app of this address.
    WebNew(String),
    /// Open this address once, as a web app would.
    OpenOnce(String),
    /// Save the draft.
    WebSave,
    /// Remove a web app, after a question.
    WebRemove {
        /// Which.
        slug: String,
        /// Its name.
        name: String,
    },
    /// Take back a non-free consent.
    Revoke(String),
    /// Run a command of the registry (the file manager, the brain's page).
    Run(&'static str),
    /// Use the copied image as the icon.
    PasteImage,
    /// Change a Flatpak's permissions, after a question.
    Permission {
        /// The app id.
        id: String,
        /// Its name.
        name: String,
        /// What changes.
        grant: Grant,
    },
}

impl Deed {
    /// Its own name, for the row's identity.
    const fn id(&self) -> &'static str {
        match self {
            Self::Install { .. } => "install",
            Self::InstallFile { .. } => "install-file",
            Self::Try { .. } => "try",
            Self::Launch(_) => "launch",
            Self::Update { .. } => "update",
            Self::UpdateSource(_) => "update-source",
            Self::UpdateEverything => "update-everything",
            Self::Refresh(_) => "refresh",
            Self::Remove { .. } => "remove",
            Self::Keep { .. } => "keep",
            Self::Stop { .. } => "stop",
            Self::Session(_) => "session",
            Self::Prepare(_) => "prepare",
            Self::PrepareAndroid => "prepare-android",
            Self::EraseAndroid => "erase-android",
            Self::Tidy(_) => "tidy",
            Self::Rollback(_) => "rollback",
            Self::DeclareForMe { .. } => "declare-for-me",
            Self::ForgetForever { .. } => "forget-forever",
            Self::KeepUninstalled {
                everywhere: false, ..
            } => "keep-uninstalled-here",
            Self::KeepUninstalled {
                everywhere: true, ..
            } => "keep-uninstalled-everywhere",
            Self::BringBack { .. } => "bring-back",
            Self::RefreshPins => "refresh-pins",
            Self::FindReplacement(_) => "find-replacement",
            Self::Edit(_) => "edit",
            Self::WebNew(_) => "web-new",
            Self::OpenOnce(_) => "open-once",
            Self::WebSave => "web-save",
            Self::WebRemove { .. } => "web-remove",
            Self::Revoke(_) => "revoke",
            Self::Run(id) => id,
            Self::PasteImage => "paste-image",
            Self::Permission { grant, .. } => match grant {
                Grant::Reset => "reset-permissions",
                Grant::UnsetGtkTheme => "unset-gtk-theme",
                Grant::Files { .. }
                | Grant::Network { .. }
                | Grant::Sound { .. }
                | Grant::Devices { .. }
                | Grant::Display { .. } => "permission",
            },
        }
    }
}

/// A value a row turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    /// Which source the lists show.
    Filter,
    /// Which source a sheet's program comes from.
    Member,
    /// What a trial leaves behind (this trial).
    Traces,
    /// Whether a sandboxed trial reaches the network (this trial).
    Network,
    /// Which icon shelf.
    Shelf,
    /// The draft's Chrome profile.
    Profile,
    /// The draft's category.
    Category,
    /// Which Android image to prepare.
    Image,
    /// Which source is recommended first (a preference).
    Prefer,
    /// Whether a search asks the network (a preference).
    Online,
    /// One network source, on or off (a preference).
    Switch(OnlineSource),
    /// Ask before downloading more than… (a preference).
    AskAbove,
    /// What trials leave behind by default (a preference).
    DefaultTraces,
    /// Whether a sandboxed trial reaches the network by default.
    DefaultNetwork,
    /// The Chrome profile new web apps use (a preference).
    WebProfile,
    /// One thing a Flatpak may reach.
    Grant(GrantRing),
    /// Android's windows: one per app, or one for all.
    Windows,
}

/// Which of a Flatpak's permissions a ring turns — the reduced form the crate's
/// [`Grant`] changes, one ring each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantRing {
    /// The person's files: read and write, read only, or none.
    Files,
    /// The network.
    Network,
    /// Sound.
    Sound,
    /// Cameras and every other device.
    Devices,
    /// Wayland or X11.
    Display,
}

impl GrantRing {
    /// Every one, in the page's order.
    const ALL: [Self; 5] = [
        Self::Files,
        Self::Network,
        Self::Sound,
        Self::Devices,
        Self::Display,
    ];

    /// The ring's own name, for its spot.
    const fn id(self) -> &'static str {
        match self {
            Self::Files => "grant-files",
            Self::Network => "grant-network",
            Self::Sound => "grant-sound",
            Self::Devices => "grant-devices",
            Self::Display => "grant-display",
        }
    }

    /// The grant a turn of this ring asks for, from what the app reaches now.
    /// `None` for a display ring on a program with no window.
    fn turned(self, now: &ronoctua_software::flatpak::Grants, forward: bool) -> Option<Grant> {
        Some(match self {
            Self::Files => Grant::Files {
                reach: turned(
                    &[Reach::ReadWrite, Reach::ReadOnly, Reach::Nothing],
                    &now.files,
                    forward,
                ),
            },
            Self::Network => Grant::Network { on: !now.network },
            Self::Sound => Grant::Sound { on: !now.sound },
            Self::Devices => Grant::Devices { on: !now.devices },
            Self::Display => Grant::Display {
                through: turned(&[Display::Wayland, Display::X11], &now.display?, forward),
            },
        })
    }
}

/// A service on the network a search or an icon may ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineSource {
    /// flathub.org's search.
    Flathub,
    /// f-droid.org's index.
    Fdroid,
    /// The AppImage catalogue and GitHub's releases.
    AppImageHub,
    /// dashboardicons.com.
    Dashboard,
}

impl OnlineSource {
    /// Every one, in the page's order.
    const ALL: [Self; 4] = [
        Self::Flathub,
        Self::Fdroid,
        Self::AppImageHub,
        Self::Dashboard,
    ];

    const fn is_on(self, p: &Preferences) -> bool {
        match self {
            Self::Flathub => p.sources.flathub,
            Self::Fdroid => p.sources.fdroid,
            Self::AppImageHub => p.sources.appimagehub,
            Self::Dashboard => p.sources.dashboard,
        }
    }

    const fn label(self) -> &'static str {
        use crate::key;
        match self {
            Self::Flathub => key::SOFTWARE_SWITCH_FLATHUB,
            Self::Fdroid => key::SOFTWARE_SWITCH_FDROID,
            Self::AppImageHub => key::SOFTWARE_SWITCH_APPIMAGEHUB,
            Self::Dashboard => key::SOFTWARE_SWITCH_DASHBOARD,
        }
    }

    const fn id(self) -> &'static str {
        match self {
            Self::Flathub => "flathub",
            Self::Fdroid => "fdroid",
            Self::AppImageHub => "appimagehub",
            Self::Dashboard => "dashboard",
        }
    }

    /// The hosts it asks, as its consent row lets them — the catalogue's
    /// own list (`ronoctua_software::<catalogue>::HOSTS`), which the
    /// requests of `ronoctua_software::net` are asked on a pass for.
    #[must_use]
    pub const fn hosts(self) -> &'static [&'static str] {
        match self {
            Self::Flathub => ronoctua_software::flathub::HOSTS,
            Self::Fdroid => ronoctua_software::fdroid::HOSTS,
            Self::AppImageHub => ronoctua_software::appimagehub::HOSTS,
            Self::Dashboard => ronoctua_software::dashboard::HOSTS,
        }
    }

    /// What its consent row says it is for.
    const fn what(self) -> &'static str {
        use crate::key;
        match self {
            Self::Flathub => key::SOFTWARE_CONSENT_FLATHUB,
            Self::Fdroid => key::SOFTWARE_CONSENT_FDROID,
            Self::AppImageHub => key::SOFTWARE_CONSENT_APPIMAGEHUB,
            Self::Dashboard => key::SOFTWARE_CONSENT_DASHBOARD,
        }
    }

    /// Whether every host it asks was let.
    #[must_use]
    pub fn is_let(self, consents: &ronoctua_consent::Consents) -> bool {
        self.hosts().iter().all(|h| consents.allows(h))
    }
}

/// One change to the person's preferences, applied by the DOOR through
/// `ronoctua_software::prefs::change` — the locked read-modify-write — so the
/// change is made to the file as it is NOW and never overwrites what the CLI
/// wrote meanwhile (an unfree consent it recorded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tweak {
    /// Which source is recommended first.
    Prefer(Prefer),
    /// Whether a search asks the network.
    Online(bool),
    /// One network source.
    Switch(OnlineSource, bool),
    /// Ask above this many megabytes; 0 never asks.
    AskAboveMb(u64),
    /// What trials leave behind by default.
    Traces(Traces),
    /// Whether a sandboxed trial reaches the network by default.
    TrialNetwork(bool),
    /// The Chrome profile for new web apps; `None` is Chrome's last used.
    WebProfile(Option<String>),
    /// A non-free package allowed, or taken back.
    Unfree {
        /// The attribute.
        attr: String,
        /// Allowed or not.
        allowed: bool,
    },
}

impl Tweak {
    /// Apply it to the preferences as they are on disk.
    pub fn apply(&self, p: &mut Preferences) {
        match self {
            Self::Prefer(x) => p.prefer = *x,
            Self::Online(on) => p.online = *on,
            Self::Switch(s, on) => match s {
                OnlineSource::Flathub => p.sources.flathub = *on,
                OnlineSource::Fdroid => p.sources.fdroid = *on,
                OnlineSource::AppImageHub => p.sources.appimagehub = *on,
                OnlineSource::Dashboard => p.sources.dashboard = *on,
            },
            Self::AskAboveMb(mb) => p.ask_above_mb = *mb,
            Self::Traces(t) => p.traces = *t,
            Self::TrialNetwork(on) => p.trial_network = *on,
            Self::WebProfile(x) => p.web_profile.clone_from(x),
            Self::Unfree { attr, allowed } => {
                if *allowed {
                    p.unfree.insert(attr.clone());
                } else {
                    p.unfree.remove(attr);
                }
            }
        }
    }
}

/// The steps of the "ask before downloading more than" ring, in megabytes.
/// `0` is "never ask".
const ASK_ABOVE: [u64; 4] = [100, 500, 1_000, 0];

/// How many versions of the Nix list are shown before the rest go into a
/// drawer: ten is a fortnight of installing something a day.
const GENERATIONS_SHOWN: usize = 10;

/// The package the "Google Play without an account" row installs: Aurora
/// Store, from F-Droid. Data — an Android package name — and not a word.
const AURORA: &str = "com.aurora.store";

/// What the keys do on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing — a fact. `RET` copies its value.
    Nothing,
    /// The search: what is typed, in either door.
    Search,
    /// An installed program: `RET` opens its sheet.
    Program(AppKey),
    /// A search result, by its identity: `RET` opens its sheet.
    Group(String),
    /// A trial, by its key: `RET` opens the sheet of what is tried.
    Trial(String),
    /// A door to another level.
    Door(Level),
    /// Something done.
    Deed(Deed),
    /// A value that turns.
    Ring(Ring),
    /// A drawer, which `RET` opens and closes.
    Drawer(&'static str),
    /// An address `RET` opens in the browser.
    Link(String),
    /// Something `RET` asks for on the door's line.
    Ask(Ask),
    /// A file `RET` opens the file picker for (`crate::file_picker`).
    Choose(Picking),
    /// A row or a cell of the file picker, while it is up.
    Pick(crate::file_picker::Target),
    /// An icon candidate: `RET` makes it the draft's.
    Candidate(IconChoice),
    /// One answer to the question.
    Answer(Answer),
    /// A line of the history: `RET` opens the program's sheet.
    Entry(Option<AppKey>),
    /// A catalogue's consent row: `RET` lets its hosts (`consent_rows`).
    Let(OnlineSource),
}

/// What a row is, so the cursor can be found again after a rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spot {
    /// A heading, a sentence: the cursor never stands here.
    Nothing,
    /// The first row that is a PROGRAM, whatever it is: where typing, and
    /// most levels as they open, put the cursor.
    Top,
    /// The search row.
    Search,
    /// An installed program.
    Program(AppKey),
    /// A search result.
    Group(String),
    /// A trial.
    Trial(String),
    /// A door, by its level.
    Door(&'static str),
    /// A deed, by its id.
    Deed(&'static str),
    /// The sheet's first verb: install, open, or keep.
    Primary,
    /// A ring, by its id.
    Ring(&'static str),
    /// A fact, by its id.
    Fact(&'static str),
    /// A drawer's line.
    Drawer(&'static str),
    /// An icon candidate.
    Candidate(IconChoice),
    /// A catalogue's consent row, by the catalogue's id.
    Let(&'static str),
    /// A row of the file picker, by its own spot.
    Pick(String),
    /// An answer.
    Answer(Answer),
    /// A consent, by its attribute.
    Consent(String),
    /// A version of the Nix list.
    Generation(u64),
    /// A line of the history, counted from the newest.
    Entry(usize),
    /// A permission line, counted from the first.
    Permission(usize),
    /// A source's status in a search.
    Status(Source),
    /// A Flatpak in the permissions list.
    Permissions(String),
    /// A declared entry, by its id.
    Declared(String),
    /// A decline, by the registry's id.
    Declined(String),
    /// One of the distro's programs, by the registry's id.
    Distro(String),
}

/// What the `C-` keys do on a row: each is `Some` where it works, and the hint
/// line teaches exactly those.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Can {
    /// `C-RET`.
    pub install: Option<Deed>,
    /// `C-t`.
    pub try_it: Option<Deed>,
    /// `C-o`.
    pub launch: Option<Deed>,
    /// `C-u`.
    pub update: Option<Deed>,
    /// `C-e`.
    pub edit: Option<Deed>,
    /// `C-d`.
    pub remove: Option<Deed>,
    /// `C-k`.
    pub stop: Option<Deed>,
}

/// One drawn line.
///
/// A row with a [`Row::picture`] starts with its GLYPH as `fields[0]`, one
/// cell and its air: a door that has the picture draws it at `fields[0].at`
/// and leaves the glyph out; a door without it draws the glyph. The two never
/// draw in the same place.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// What is drawn, already placed and cut.
    pub fields: Vec<Field>,
    /// Whether the cursor is on it.
    pub focused: bool,
    /// Whether the cursor skips it.
    pub heading: bool,
    /// What `RET` does here.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: Spot,
    /// Reachable and refusing — drawn dim, and `RET` says why.
    pub disabled: bool,
    /// Why it refuses, as the sentence `RET` says.
    pub said: Option<String>,
    /// What the `C-` keys do here.
    pub can: Can,
    /// The program's picture, or an icon candidate's.
    pub picture: Option<Portrait>,
}

impl Row {
    /// Whether a click takes it — a row the cursor stands on, or a later
    /// line of a catalogue's consent row, which is the row's press as its
    /// first line is ([`View::clicked`] lands it there) — and so whether a
    /// resting pointer lights it: ONE ANSWER for both doors' light and click
    /// (the region's `Row::takes`, the window's `Content::takes`,
    /// [`View::clicked`]).
    #[must_use]
    pub const fn takes(&self) -> bool {
        !self.heading || matches!(self.target, Target::Let(_))
    }
}

/// An order for the queue, with the name its row shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queued {
    /// What to do.
    pub order: Order,
    /// The program's name (data, not a key) — `Work::Software::label`.
    pub label: String,
}

/// What the door has to do about a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Shut the view.
    Close,
    /// Ask the feed: `readings.want(&feed, want)`.
    Feed(Want),
    /// Send each to the queue as `Work::Software { order, label }`, and say
    /// [`queued_said`] (or the queue's own refusal when it is not there).
    Queue(Vec<Queued>),
    /// Ask on the door's own line — the footer's minibuffer, a window's
    /// header: `ask.command()`, `ask.secret()`, and the text it starts from.
    /// Both doors answer through [`View::typed`].
    Ask {
        /// What is asked.
        ask: Ask,
        /// What the line starts with.
        initial: Option<String>,
    },
    /// Open this installed program (its desktop id), through the launcher's
    /// own path.
    Launch(String),
    /// Open this address once, as a web app would, without making one
    /// (`ronoctua_software::web::once_argv`).
    OpenOnce {
        /// The address.
        url: String,
        /// Chrome's profile directory.
        profile: String,
    },
    /// Put this on the clipboard (`chrome::copied`).
    Copy(String),
    /// Open this address in the browser.
    Browse(String),
    /// Say this on the message line; it is already a sentence.
    Say(String),
    /// Change one preference (`ronoctua_software::prefs::change`).
    Tweak(Tweak),
    /// Run this command of the registry.
    Run(&'static str),
    /// Hand the clipboard's newest text to [`View::pasted`].
    Paste,
    /// Write the clipboard's newest IMAGE to a file and hand its path to
    /// [`View::dropped`].
    PasteImage,
    /// Let this catalogue's hosts in the person's `consent.json`
    /// (`consent_rows::allow`), and read the consents again.
    Let(OnlineSource),
}

impl Errand {
    /// Whether doing it takes the person ELSEWHERE — an installed program
    /// started, the brain's page opened in a window — so a door that holds
    /// the keyboard (the footer's region) gives it back first, and leaves
    /// nothing open over what just opened. A window keeps its own keyboard,
    /// and asks nothing of this.
    #[must_use]
    pub fn opens_elsewhere(&self) -> bool {
        match self {
            Self::Launch(_) => true,
            Self::Run(id) => *id == "brain-open",
            _ => false,
        }
    }
}

/// Where the rows start and how wide they are — `chrome::extent`, the one
/// arithmetic of this shape.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    crate::chrome::extent(columns)
}

/// The width the tests build rows at.
#[cfg(test)]
const DECIDING_WIDTH: u16 = 100;

/// How far back `Esc` can go. A path deeper than this is a person lost, and
/// the bottom of it is the first page anyway.
const DEPTH: usize = 8;

/// The view.
#[derive(Debug, Clone)]
pub struct View {
    level: Level,
    /// Where `Esc` goes, with where the cursor was there.
    back: Vec<(Level, Spot)>,
    /// The cursor, by identity.
    at: Spot,
    /// The cursor's index last time, for when its row is gone.
    last: usize,
    /// What was typed into the search.
    query: String,
    /// Which source the lists show; `None` is every one.
    filter: Option<Source>,
    intent: Intent,
    /// Which drawers are open.
    open: Vec<&'static str>,
    asking: Option<Question>,
    /// Where the cursor was when the question opened.
    before: Option<Spot>,
    /// This trial's traces and network, born from the preferences.
    traces: Traces,
    network: bool,
    /// Which Android image the preparing row offers.
    image: AndroidImage,
    /// The web app being made or changed.
    draft: Option<WebDraft>,
    /// What the icon shelves look for.
    icon_query: String,
    /// Names handed in with a key from outside (the launcher).
    outside: Vec<(AppKey, String)>,
    /// The file picker, while it is up over the view's rows — an icon, a
    /// package file (`crate::file_picker::Hosted`).
    picking: Option<crate::file_picker::Hosted<Picking>>,
    /// The door's wake, for the file picker ([`View::watch_with`]).
    wake: Option<crate::wake::Wake>,
    /// The change of the declaration waiting for the belongings' passphrase
    /// ([`Ask::Sign`]): queued once it is typed, dropped by an empty answer.
    signing: Option<Queued>,
}

impl View {
    /// A view opened at `opening`, and the first thing the door has to do for
    /// it — besides reading the machine (`Want::Reread`), which every opening
    /// needs and the door asks for itself.
    ///
    /// IT DOES NOT KNOW ITS DOOR: the footer's region and the window ask it
    /// the same questions and get the same answers, and what a door does with
    /// an answer is the door's (the owner's rule: the two do the same things).
    #[must_use]
    pub fn fresh(opening: Opening, r: &Readings, texts: &ronoctua_i18n::Texts) -> (Self, Errand) {
        let mut v = Self {
            level: Level::Home,
            back: Vec::new(),
            // BORN ON THE SEARCH in both doors: both type into it.
            at: Spot::Search,
            last: 0,
            query: String::new(),
            filter: None,
            intent: Intent::Browse,
            open: Vec::new(),
            asking: None,
            before: None,
            traces: r.prefs.traces,
            network: r.prefs.trial_network,
            image: AndroidImage::default(),
            draft: None,
            icon_query: String::new(),
            outside: Vec::new(),
            picking: None,
            wake: None,
            signing: None,
        };
        let errand = v.reopen(opening, r, texts);
        (v, errand)
    }

    /// Go to `opening` from wherever the view is — a second leader key while
    /// it is open, or an opening that had to wait for the first reading of
    /// the machine (the launcher's `C-i` before anything was read). The level
    /// it leaves is the way back.
    pub fn reopen(
        &mut self,
        opening: Opening,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if self.asking.is_some() {
            self.close_question();
        }
        match opening {
            Opening::Home => {
                while !self.back.is_empty() {
                    self.pop();
                }
                self.enter_level(Level::Home, r)
            }
            Opening::Results(q) => {
                self.query = q;
                self.show_results(r)
            }
            Opening::Trials => self.go(Level::Trials, r),
            Opening::Maintenance => self.go(Level::Maintenance, r),
            Opening::Android => self.go(Level::Android, r),
            Opening::Advanced => self.go(Level::Advanced, r),
            Opening::Declared => self.go(Level::Declared, r),
            Opening::Declined => self.go(Level::Declined, r),
            Opening::WebNew(url) => self.web_new(&url, r, texts),
            Opening::Sheet { key, name } => {
                self.outside.push((key.clone(), name));
                self.go(Level::Sheet(key), r)
            }
            // ONE OF THE DISTRO'S PROGRAMS is kept uninstalled, not removed:
            // the sheet opens with that question, on "no". One the distro
            // needs, and what the flake installed otherwise, say why not
            // instead of a question whose yes the CLI would refuse anyway.
            Opening::Remove {
                key: key @ AppKey::System { .. },
                name,
            } => {
                self.outside.push((key.clone(), name.clone()));
                self.go(Level::Sheet(key.clone()), r);
                match declarations(r).and_then(|d| d.distro_of(&key, &[])) {
                    Some(d) if d.app.essential.is_none() => {
                        self.ask_it(Question::Decline {
                            id: d.app.id.clone(),
                            name,
                            everywhere: false,
                        });
                        Errand::Nothing
                    }
                    Some(d) => Errand::Say(essential_said(d, texts)),
                    None => Errand::Say(texts.t(words::refusal_key(Refusal::DeclaredInTheFlake))),
                }
            }
            Opening::Remove { key, name } => {
                self.outside.push((key.clone(), name.clone()));
                let e = self.go(Level::Sheet(key.clone()), r);
                // A DECLARED PROGRAM goes for good, after the explanation:
                // removed here alone, the next update would bring it back.
                if origin_of(r, &key) == Origin::MineDeclared {
                    self.ask_it(Question::Forget { key, name });
                } else {
                    self.ask_it(Question::Remove { key, name });
                }
                e
            }
            Opening::File(path) => {
                self.ask_it(Question::Foreign(path));
                Errand::Nothing
            }
        }
    }

    /// Which page is showing — what the footer compares before and after a
    /// key, so a page it opens starts at its top.
    #[must_use]
    pub const fn level(&self) -> &Level {
        &self.level
    }

    /// What was typed into the search — both doors draw it on the search's
    /// row, and the footer on the minibuffer's line too, as the launcher's
    /// query is.
    #[must_use]
    pub fn query(&self) -> &str {
        if matches!(self.level, Level::Icons(_)) {
            &self.icon_query
        } else {
            &self.query
        }
    }

    /// What the door asks the feed for once an order of this view ENDED: the
    /// machine again, always, and the page's own reading when the page is
    /// one a job changes — a Flatpak's permissions after a ring turned them,
    /// the dark diagnosis after one was taken away.
    #[must_use]
    pub fn rereads(&self) -> Vec<Want> {
        let mut out = vec![Want::Reread];
        match &self.level {
            Level::Permissions(id) => out.push(Want::Permissions(id.clone())),
            Level::Advanced => out.push(Want::Dark),
            _ => {}
        }
        out
    }

    /// Whether a letter typed now is search text, in either door.
    ///
    /// Not on the web app's editor, whose rows are fields a letter would
    /// half-fill, and not while a question is asked: a letter there is a key
    /// somebody meant for something else.
    #[must_use]
    pub fn takes_letters(&self) -> bool {
        // THE PICKER, while it is up, takes every letter.
        if self.picking.is_some() {
            return true;
        }
        self.asking.is_none()
            && match self.level {
                Level::WebEditor | Level::Icons(Shelf::Site | Shelf::File) => false,
                Level::Home
                | Level::Results
                | Level::Sheet(_)
                | Level::Trials
                | Level::Icons(Shelf::Dashboard | Shelf::Collection)
                | Level::Android
                | Level::Maintenance
                | Level::Advanced
                | Level::Permissions(_)
                | Level::Consents
                | Level::History
                | Level::Declared
                | Level::Declaration(_)
                | Level::Declined
                | Level::Distro(_) => true,
            }
    }

    /// Which row the cursor is on, in rows this view built.
    #[must_use]
    pub fn cursor(rows: &[Row]) -> usize {
        rows.iter().position(|r| r.focused).unwrap_or(0)
    }

    /// Every row, for this level.
    #[must_use]
    pub fn rows(&self, r: &Readings, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        // THE PICKER'S ROWS while it is up, as this view's own.
        if let Some(h) = self.picking.as_ref() {
            return h.rows(columns, texts, |p| Row {
                fields: p.fields,
                focused: p.focused,
                heading: p.heading,
                target: Target::Pick(p.target),
                spot: if p.heading {
                    Spot::Nothing
                } else {
                    Spot::Pick(p.spot)
                },
                disabled: false,
                said: None,
                can: Can::default(),
                picture: None,
            });
        }
        let first = self.build(r, columns, texts, &self.at);
        if first.iter().any(|x| x.focused) {
            return first;
        }
        // THE SPOT IS NOT HERE — the program was removed, a batch changed the
        // list, or it is `Top` and asks for the first program. The cursor goes
        // to the row nearest to where it was, and the spot follows at the next
        // key (`settle`).
        match self.fallback(&first) {
            Some(spot) => self.build(r, columns, texts, &spot),
            None => first,
        }
    }

    /// Put the cursor's identity back in step with what is drawn. The door
    /// calls it after readings arrive; it never moves the page.
    pub fn settle(&mut self, r: &Readings, columns: u16, texts: &ronoctua_i18n::Texts) {
        let rows = self.rows(r, columns, texts);
        if let Some(i) = rows.iter().position(|x| x.focused) {
            self.at = rows[i].spot.clone();
            self.last = i;
        }
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        r: &Readings,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        if let Some(h) = self.picking.as_ref() {
            return h.picker.hint(texts);
        }
        let rows = self.rows(r, columns, texts);
        let row = rows.iter().find(|x| x.focused);
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(row, self))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// The row a click on row `n` of `rows` lands on — `n` itself, a row the
    /// cursor stands on, or the first line of the consent row a later line
    /// belongs to — and `None` where the click takes nothing ([`Row::takes`]):
    /// a heading, a sentence.
    fn landing(n: usize, rows: &[Row]) -> Option<usize> {
        let row = rows.get(n).filter(|x| x.takes())?;
        // A LATER LINE OF THE CONSENT ROW is the row: its first line is
        // where the cursor stands.
        let n = match row.target {
            Target::Let(source) if row.heading => rows[..n]
                .iter()
                .rposition(|p| !p.heading && p.target == Target::Let(source))?,
            _ => n,
        };
        Some(n)
    }

    /// The row whose control row `n` is: itself, or the first line of the
    /// consent row a later line belongs to — the lines of one control light
    /// together, and a click on any takes the same thing
    /// (`Region::control_of`).
    #[must_use]
    pub fn control_of(n: usize, rows: &[Row]) -> usize {
        Self::landing(n, rows).unwrap_or(n)
    }

    /// Put the cursor where a click landed. Answers whether it was ALREADY
    /// there — the second click, which the door turns into `RET`. A click
    /// on any line of a consent row is a click on its first
    /// ([`View::control_of`]).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let Some(n) = Self::landing(n, rows) else {
            return false;
        };
        let Some(row) = rows.get(n) else {
            return false;
        };
        // A ROW OF THE PICKER is chosen there: its cursor, and the second
        // click its `RET`.
        if let (Some(h), Target::Pick(t)) = (self.picking.as_mut(), &row.target) {
            return h.clicked(t.clone());
        }
        let again = row.focused;
        self.at = row.spot.clone();
        self.last = n;
        again
    }

    /// A key.
    ///
    /// THE WIDTH IS THE DOOR'S: a note wraps, so the rows a key acts on are
    /// the rows that were DRAWN at this width.
    pub fn key(
        &mut self,
        press: Press,
        r: &Readings,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // THE PICKER takes the view's keys that walk, answer and go back — a
        // second click's `RET`, a command's arrows, the pointer's way back.
        if self.picking.is_some() {
            use ronoctua_keymap::{Key, Sym};
            let key = match press {
                Press::Up => Key::simple(Sym::Up),
                Press::Down => Key::simple(Sym::Down),
                Press::Less => Key::simple(Sym::Left),
                Press::More => Key::simple(Sym::Right),
                Press::Enter => Key::simple(Sym::Enter),
                Press::Escape => Key::simple(Sym::Esc),
                Press::NextSource => Key::simple(Sym::Tab),
                Press::Paste => return Errand::Paste,
                _ => return Errand::Nothing,
            };
            return self.pick_key(key, r, texts).unwrap_or(Errand::Nothing);
        }
        let rows = self.rows(r, columns, texts);
        let at = rows.iter().position(|x| x.focused);
        if let Some(i) = at {
            self.at = rows[i].spot.clone();
            self.last = i;
        }
        let row = at.map(|i| rows[i].clone());
        match press {
            Press::Escape => self.escape(),
            Press::Up | Press::Down => {
                self.walk(&rows, press == Press::Down);
                Errand::Nothing
            }
            Press::Paste => {
                if self.takes_letters() {
                    Errand::Paste
                } else {
                    Errand::Nothing
                }
            }
            Press::NextSource | Press::PreviousSource => {
                if self.asking.is_some() {
                    return Errand::Nothing;
                }
                let forward = press == Press::NextSource;
                match self.level {
                    Level::Home | Level::Results => self.turn(Ring::Filter, forward, r, texts),
                    Level::Icons(_) => self.turn(Ring::Shelf, forward, r, texts),
                    _ => Errand::Nothing,
                }
            }
            Press::Enter | Press::Less | Press::More => {
                let Some(row) = row else {
                    return Errand::Nothing;
                };
                if row.disabled {
                    return row.said.clone().map_or(Errand::Nothing, Errand::Say);
                }
                match (press, &row.target) {
                    (Press::Enter, _) => self.enter(&row, r, texts),
                    (_, Target::Ring(ring)) => self.turn(*ring, press == Press::More, r, texts),
                    _ => Errand::Nothing,
                }
            }
            Press::Install
            | Press::Try
            | Press::Launch
            | Press::Update
            | Press::Edit
            | Press::Remove
            | Press::Stop => {
                if self.asking.is_some() {
                    return Errand::Nothing;
                }
                let deed = row.and_then(|x| {
                    let c = x.can;
                    match press {
                        Press::Install => c.install,
                        Press::Try => c.try_it,
                        Press::Launch => c.launch,
                        Press::Update => c.update,
                        Press::Edit => c.edit,
                        Press::Remove => c.remove,
                        _ => c.stop,
                    }
                });
                deed.map_or(Errand::Nothing, |d| self.perform(d, r, texts))
            }
        }
    }

    /// A key the door read, for the file picker while it is up — `None` when
    /// none is, or it does not take the key, which is then the view's.
    pub fn pick_key(
        &mut self,
        key: ronoctua_keymap::Key,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        let h = self.picking.as_mut().filter(|h| h.picker.claims(key))?;
        let hosting = h.key(key);
        Some(self.hosting(hosting, r, texts))
    }

    /// What the picker's step is to this view: its answer taken — the icon,
    /// the package asked about — and the view's rows back.
    fn hosting(
        &mut self,
        hosting: crate::file_picker::Hosting<Picking>,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::file_picker::{Hosting, Picked};
        match hosting {
            Hosting::Nothing => Errand::Nothing,
            Hosting::Back => {
                self.picking = None;
                Errand::Nothing
            }
            Hosting::Say(e) => Errand::Say(texts.t_with(e.key, &e.borrowed())),
            Hosting::Paste => Errand::Paste,
            Hosting::Picked(asked, Picked::Paths { paths, .. }) => {
                let Some(path) = paths.into_iter().next() else {
                    return Errand::Nothing;
                };
                match asked {
                    Picking::Icon => {
                        self.picking = None;
                        self.took_icon(IconChoice::File { path }, r)
                    }
                    // A FILE NO SOURCE INSTALLS is said, and the picker
                    // stays for another.
                    Picking::Package => {
                        if ronoctua_software::order::file_source(&path).is_none() {
                            return Errand::Say(texts.t(crate::key::SOFTWARE_NOT_A_PACKAGE_FILE));
                        }
                        self.picking = None;
                        self.ask_it(Question::Foreign(path));
                        Errand::Nothing
                    }
                }
            }
            Hosting::Picked(..) => Errand::Nothing,
        }
    }

    /// The file picker, opened for `asked` — what the rows that choose a
    /// file run, and the commands that ask for one with no argument.
    pub fn choose(&mut self, asked: Picking) -> Errand {
        self.picking = Some(crate::file_picker::Hosted::open(
            asked.request(),
            asked,
            asked.title(),
            self.wake.as_ref(),
        ));
        Errand::Nothing
    }

    /// The door's wake, for the file picker's folder and devices: a door
    /// with a loop hands it once, and every picker this view opens after is
    /// watched through it (`file_picker::FilePicker::watch_with`).
    pub fn watch_with(&mut self, wake: crate::wake::Wake) {
        self.wake = Some(wake);
    }

    /// What the wake brought the file picker, taken — `true` when its rows
    /// changed and the door draws again.
    pub fn look(&mut self) -> bool {
        self.picking
            .as_mut()
            .is_some_and(crate::file_picker::Hosted::look)
    }

    /// How many lines of rows the door shows, for the file picker's `PgUp`
    /// and `PgDn` — told before a key is handed over.
    pub fn page_is(&self, lines: usize) {
        if let Some(h) = self.picking.as_ref() {
            h.picker.page_is(u16::try_from(lines).unwrap_or(u16::MAX));
        }
    }

    /// Whether the file picker is up.
    #[must_use]
    pub const fn picking(&self) -> bool {
        self.picking.is_some()
    }

    /// A letter typed, in either door.
    pub fn letter(&mut self, c: char, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        if self.picking.is_some() {
            let key = ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Char(c));
            return self.pick_key(key, r, texts).unwrap_or(Errand::Nothing);
        }
        // A CONTROL CHARACTER IS NOT A LETTER: `Delete` arrives as U+007F,
        // and typed it is a search for a character nobody can see.
        if c.is_control() {
            return Errand::Nothing;
        }
        if !self.takes_letters() {
            // A FIELD OF THE DRAFT is typed on its own row: `RET` there opens
            // the door's line on it — in either door.
            return Errand::Say(texts.t_with(
                crate::key::SOFTWARE_TYPE_ON_A_ROW,
                &[("keys", notation_of("software-enter").unwrap_or_default())],
            ));
        }
        if let Level::Icons(shelf) = self.level {
            self.icon_query.push(c);
            return Errand::Feed(Want::Icons {
                shelf,
                query: self.icon_query.clone(),
            });
        }
        if self.level == Level::Trials {
            self.intent = Intent::Try;
        }
        self.query.push(c);
        self.show_results(r)
    }

    /// `DEL`, in either door: one letter off the search, and an empty search
    /// goes back where it came from.
    pub fn erase(&mut self, r: &Readings) -> Errand {
        if let Some(h) = self.picking.as_mut() {
            let key = ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Backspace);
            return match h.key(key) {
                crate::file_picker::Hosting::Back => {
                    self.picking = None;
                    Errand::Nothing
                }
                _ => Errand::Nothing,
            };
        }
        if !self.takes_letters() {
            return Errand::Nothing;
        }
        // ONE CLUSTER, as every line of the system takes it
        // (`line::pop_cluster`): an accent or an emoji is one thing typed.
        if let Level::Icons(shelf) = self.level {
            if !crate::line::pop_cluster(&mut self.icon_query) {
                return Errand::Nothing;
            }
            return Errand::Feed(Want::Icons {
                shelf,
                query: self.icon_query.clone(),
            });
        }
        if !crate::line::pop_cluster(&mut self.query) {
            return if self.level == Level::Results {
                self.escape()
            } else {
                Errand::Nothing
            };
        }
        if self.query.is_empty() && self.level == Level::Results {
            self.escape();
            return Errand::Feed(Want::Search {
                query: String::new(),
                sources: Vec::new(),
            });
        }
        self.show_results(r)
    }

    /// What the clipboard held, pasted into the search. A pasted ADDRESS
    /// offers a web app; a pasted PATH to an `.AppImage`, `.apk` or
    /// `.flatpakref` offers to install that file — both as rows on top of what
    /// the search finds.
    pub fn pasted(&mut self, text: &str, r: &Readings) -> Errand {
        if let Some(h) = self.picking.as_mut() {
            let _ = h.pasted(text);
            return Errand::Nothing;
        }
        let line = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or_default();
        if line.is_empty() || !self.takes_letters() {
            return Errand::Nothing;
        }
        if let Level::Icons(shelf) = self.level {
            line.clone_into(&mut self.icon_query);
            return Errand::Feed(Want::Icons {
                shelf,
                query: self.icon_query.clone(),
            });
        }
        line.clone_into(&mut self.query);
        self.show_results(r)
    }

    /// Files dropped on a door. An image on the web app's editor or its icons
    /// becomes the icon; a file a source installs is asked about.
    pub fn dropped(
        &mut self,
        paths: &[PathBuf],
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(path) = paths.first() else {
            return Errand::Nothing;
        };
        if ronoctua_software::order::file_source(path).is_some() {
            self.ask_it(Question::Foreign(path.clone()));
            return Errand::Nothing;
        }
        if matches!(self.level, Level::WebEditor | Level::Icons(_)) {
            return self.took_icon(IconChoice::File { path: path.clone() }, r);
        }
        Errand::Say(texts.t(words::refusal_key(Refusal::UnknownItem)))
    }

    /// What the door's line answered, for the command that asked — the
    /// footer's minibuffer or a window's header, the same call.
    pub fn typed(
        &mut self,
        id: &str,
        said: &str,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(ask) = Ask::of_command(id) else {
            return Errand::Nothing;
        };
        // A PASSPHRASE IS TAKEN AS TYPED — spaces are part of one.
        if ask == Ask::Sign {
            return self.signed_with(said, r, texts);
        }
        let said = said.trim();
        // AN EMPTY ANSWER IS `Esc`, not a mistake.
        if said.is_empty() {
            return Errand::Nothing;
        }
        match ask {
            Ask::WebNew => self.web_new(said, r, texts),
            Ask::WebAddress => match ronoctua_software::web::canonical_url(said) {
                Ok(url) => {
                    let draft = self.draft_mut(r);
                    draft.url.clone_from(&url);
                    draft.changed = true;
                    Errand::Feed(Want::Site(url))
                }
                Err(why) => Errand::Say(texts.t(words::refusal_key(why))),
            },
            Ask::WebName => {
                let draft = self.draft_mut(r);
                said.clone_into(&mut draft.name);
                draft.changed = true;
                Errand::Nothing
            }
            Ask::IconPath => {
                let path = expanded(said, r.home.as_deref());
                self.took_icon(IconChoice::File { path }, r)
            }
            Ask::FilePath => {
                let path = expanded(said, r.home.as_deref());
                if ronoctua_software::order::file_source(&path).is_none() {
                    return Errand::Say(texts.t(crate::key::SOFTWARE_NOT_A_PACKAGE_FILE));
                }
                self.ask_it(Question::Foreign(path));
                Errand::Nothing
            }
            // Answered above, as typed.
            Ask::Sign => Errand::Nothing,
        }
    }

    /// The belongings' passphrase, typed to sign the change waiting for it:
    /// written where `sign` reads it, and the change queued — `apply` signs
    /// the mirror before it stages it (`ronoctua_software::stage`). An empty
    /// answer is `Esc`: the change is dropped, and nothing was done.
    fn signed_with(&mut self, typed: &str, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(waiting) = self.signing.take() else {
            return Errand::Nothing;
        };
        if typed.is_empty() {
            return Errand::Nothing;
        }
        let Some(places) = r.belongings.as_ref() else {
            return Errand::Queue(vec![waiting]);
        };
        match crate::keep::write_passphrase_to_sign(places, typed) {
            Ok(()) => Errand::Queue(vec![waiting]),
            Err(said) => Errand::Say(texts.t(said)),
        }
    }

    /// A change of the person's declaration, for the queue — or, when the
    /// session keeps no identity to sign it with
    /// ([`crate::keep::signing_asks`]), the belongings' passphrase
    /// asked first, at the gesture: signed, the change reaches this machine's
    /// root and the person's other machines; unsigned, it would wait for
    /// their next sync.
    fn queue_signed(&mut self, order: Order, label: String, r: &Readings) -> Errand {
        let asks = r.belongings.as_ref().is_some_and(|places| {
            crate::keep::signing_asks(places, i64::try_from(r.now).unwrap_or(i64::MAX))
        });
        if !asks {
            return queue(order, label);
        }
        self.signing = Some(Queued { order, label });
        Errand::Ask {
            ask: Ask::Sign,
            initial: None,
        }
    }

    // ─── moving ─────────────────────────────────────────────────────────────

    /// Open a level, remembering where the cursor was, and answer what has to
    /// be fetched for it.
    fn go(&mut self, level: Level, r: &Readings) -> Errand {
        if self.back.len() >= DEPTH {
            self.back.remove(0);
        }
        self.back.push((self.level.clone(), self.at.clone()));
        self.enter_level(level, r)
    }

    /// Show a level, without touching the way back.
    fn enter_level(&mut self, level: Level, r: &Readings) -> Errand {
        self.at = match &level {
            Level::Sheet(_) if self.intent == Intent::Try => Spot::Deed("try"),
            Level::Sheet(_) | Level::WebEditor => Spot::Primary,
            Level::Home | Level::Trials => Spot::Search,
            _ => Spot::Top,
        };
        self.last = 0;
        let errand = match &level {
            Level::Sheet(key) => match key {
                AppKey::System { .. } => Errand::Nothing,
                _ => Errand::Feed(Want::Details(key.clone())),
            },
            Level::Permissions(id) => Errand::Feed(Want::Permissions(id.clone())),
            Level::Advanced => Errand::Feed(Want::Dark),
            Level::Icons(shelf) => {
                if self.icon_query.is_empty() {
                    self.icon_query = self.draft_query(r);
                }
                match shelf {
                    Shelf::Dashboard | Shelf::Collection => Errand::Feed(Want::Icons {
                        shelf: *shelf,
                        query: self.icon_query.clone(),
                    }),
                    Shelf::Site | Shelf::File => Errand::Nothing,
                }
            }
            _ => Errand::Nothing,
        };
        self.level = level;
        errand
    }

    /// `Esc`: out of a question, up one level, and out.
    fn escape(&mut self) -> Errand {
        if self.asking.is_some() {
            self.close_question();
            return Errand::Nothing;
        }
        if self.level == Level::WebEditor && self.draft.as_ref().is_some_and(|d| d.changed) {
            self.ask_it(Question::LeaveDraft);
            return Errand::Nothing;
        }
        self.pop()
    }

    /// Up one level.
    fn pop(&mut self) -> Errand {
        let Some((level, spot)) = self.back.pop() else {
            return Errand::Close;
        };
        if self.level == Level::Results {
            self.query.clear();
            self.intent = Intent::Browse;
        }
        if self.level == Level::WebEditor {
            self.draft = None;
        }
        if matches!(self.level, Level::Icons(_)) {
            self.icon_query.clear();
        }
        self.level = level;
        self.at = spot;
        Errand::Nothing
    }

    /// One row up or down, past headings.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let from = rows.iter().position(|x| x.focused).unwrap_or(self.last);
        let next = if down {
            (from + 1..rows.len()).find(|&i| !rows[i].heading)
        } else {
            (0..from).rev().find(|&i| !rows[i].heading)
        };
        if let Some(i) = next {
            self.at = rows[i].spot.clone();
            self.last = i;
        }
    }

    /// The row the cursor goes to when its spot is not drawn.
    fn fallback(&self, rows: &[Row]) -> Option<Spot> {
        if self.at == Spot::Top
            && let Some(r) = rows.iter().find(|x| {
                !x.heading
                    && matches!(
                        x.target,
                        Target::Program(_)
                            | Target::Group(_)
                            | Target::Trial(_)
                            | Target::Candidate(_)
                    )
            })
        {
            return Some(r.spot.clone());
        }
        let from = self.last.min(rows.len().saturating_sub(1));
        (from..rows.len())
            .chain((0..from).rev())
            .find(|&i| !rows[i].heading)
            .map(|i| rows[i].spot.clone())
    }

    /// The search, with what was typed.
    fn show_results(&mut self, r: &Readings) -> Errand {
        // A TYPED PREFIX PICKS THE SOURCE: `nix:ripgrep`, `flatpak:gimp`.
        if let Some((word, rest)) = self.query.split_once(':')
            && let Some(source) = Source::of_word(word.trim())
        {
            self.filter = Some(source);
            self.query = rest.trim_start().to_owned();
        }
        if self.level != Level::Results {
            if self.back.len() >= DEPTH {
                self.back.remove(0);
            }
            self.back.push((self.level.clone(), self.at.clone()));
            self.level = Level::Results;
        }
        self.at = Spot::Top;
        self.last = 0;
        Errand::Feed(Want::Search {
            query: self.query.clone(),
            sources: self.sources(r),
        })
    }

    /// Where a search looks: the filter's source, or every source this machine
    /// has (web apps are made, not searched).
    fn sources(&self, r: &Readings) -> Vec<Source> {
        if let Some(s) = self.filter {
            return vec![s];
        }
        Source::ALL
            .into_iter()
            .filter(|s| *s != Source::Web)
            .filter(|s| r.snapshot.as_ref().is_none_or(|snap| snap.supports.has(*s)))
            .collect()
    }

    // ─── questions ──────────────────────────────────────────────────────────

    /// Ask, with the cursor on "no".
    fn ask_it(&mut self, q: Question) {
        if self.asking.is_none() {
            self.before = Some(self.at.clone());
        }
        self.asking = Some(q);
        self.at = Spot::Answer(Answer::No);
    }

    fn close_question(&mut self) {
        self.asking = None;
        self.at = self.before.take().unwrap_or(Spot::Top);
    }

    /// An answer.
    fn answered(&mut self, answer: Answer, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(q) = self.asking.clone() else {
            return Errand::Nothing;
        };
        self.close_question();
        if answer == Answer::No {
            return Errand::Nothing;
        }
        match q {
            Question::Consent { kind, deed, .. } => {
                let deed = match *deed {
                    Deed::Install { key, name, consent } => Deed::Install {
                        key,
                        name,
                        consent: kind.granted(consent),
                    },
                    Deed::Try {
                        attr,
                        name,
                        consent,
                    } => Deed::Try {
                        attr,
                        name,
                        consent: kind.granted(consent),
                    },
                    Deed::InstallFile { path, consent } => Deed::InstallFile {
                        path,
                        consent: kind.granted(consent),
                    },
                    Deed::DeclareForMe {
                        key,
                        name,
                        consent,
                        machines,
                    } => Deed::DeclareForMe {
                        key,
                        name,
                        consent: kind.granted(consent),
                        machines,
                    },
                    other => other,
                };
                // THE NEXT CONSENT MAY BE ASKED NOW: a package can be both
                // not free and big, and each is its own question.
                self.perform(deed, r, texts)
            }
            Question::Remove { key, name } => queue(
                Order::Remove {
                    key,
                    keep_data: answer != Answer::YesAndMore,
                },
                name,
            ),
            Question::RemoveWeb { slug, name } => {
                self.draft = None;
                if self.level == Level::WebEditor {
                    self.pop();
                }
                queue(
                    Order::Remove {
                        key: AppKey::Web { slug },
                        keep_data: true,
                    },
                    name,
                )
            }
            Question::Rollback(generation) => queue(
                Order::Rollback { generation },
                texts.t(words::source_key(Source::Nix)),
            ),
            Question::CleanUp(source) => {
                let all: Vec<Source> = match source {
                    Some(s) => vec![s],
                    None => [Source::Nix, Source::Flatpak, Source::AppImage]
                        .into_iter()
                        .filter(|s| r.snapshot.as_ref().is_none_or(|snap| snap.supports.has(*s)))
                        .collect(),
                };
                Errand::Queue(
                    all.into_iter()
                        .map(|s| Queued {
                            order: Order::Tidy { source: s },
                            label: texts.t(words::source_key(s)),
                        })
                        .collect(),
                )
            }
            Question::PrepareAndroid(image) => queue(
                Order::Prepare {
                    source: Source::Android,
                    image: Some(image),
                },
                texts.t(words::source_key(Source::Android)),
            ),
            Question::EraseAndroid => queue(
                Order::AndroidErase,
                texts.t(words::source_key(Source::Android)),
            ),
            Question::Foreign(path) => self.perform(
                Deed::InstallFile {
                    path,
                    consent: Consent {
                        unverified: true,
                        ..Consent::default()
                    },
                },
                r,
                texts,
            ),
            Question::LeaveDraft => {
                self.draft = None;
                self.pop()
            }
            Question::Permission { id, name, grant } => {
                queue(Order::Permission { id, grant }, name)
            }
            Question::Forget { key, name } => self.queue_signed(
                Order::Forget {
                    key,
                    keep_data: answer != Answer::YesAndMore,
                },
                name,
                r,
            ),
            Question::Decline {
                id,
                name,
                everywhere,
            } => self.queue_signed(
                Order::Decline {
                    id,
                    machines: if everywhere {
                        None
                    } else {
                        declarations(r)
                            .map(|d| d.host.clone())
                            .filter(|h| !h.is_empty())
                            .map(|h| vec![h])
                    },
                },
                name,
                r,
            ),
        }
    }

    // ─── deeds ──────────────────────────────────────────────────────────────

    /// What `RET` does on a row.
    fn enter(&mut self, row: &Row, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        match &row.target {
            // A CONSENT COPIES ITS PACKAGE, the thing one would paste; the
            // word beside it is the same on every row.
            Target::Nothing if matches!(row.spot, Spot::Consent(_)) => match &row.spot {
                Spot::Consent(attr) => Errand::Copy(attr.clone()),
                _ => Errand::Nothing,
            },
            // A FACT COPIES ITSELF — `chrome::information_of`.
            Target::Nothing => {
                crate::chrome::information_of(&row.fields).map_or(Errand::Nothing, Errand::Copy)
            }
            // TYPING IS ALREADY ON, in either door: `RET` says so, or runs
            // the search again.
            Target::Search => {
                if matches!(self.level, Level::Icons(_)) {
                    return Errand::Say(texts.t(crate::key::SOFTWARE_TYPE_TO_SEARCH));
                }
                if self.level == Level::Trials {
                    self.intent = Intent::Try;
                }
                if self.query.is_empty() {
                    return Errand::Say(texts.t(crate::key::SOFTWARE_TYPE_TO_SEARCH));
                }
                self.show_results(r)
            }
            Target::Program(key) => self.go(Level::Sheet(key.clone()), r),
            Target::Group(id) => match r.group(id) {
                Some(g) => {
                    let key = g.best().key.clone();
                    self.go(Level::Sheet(key), r)
                }
                None => Errand::Nothing,
            },
            Target::Trial(key) => {
                let attr = r
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.trials.iter().find(|t| t.key == *key))
                    .map(|t| t.attr.clone());
                match attr {
                    Some(attr) => {
                        self.intent = Intent::Try;
                        self.go(Level::Sheet(AppKey::Nix { attr }), r)
                    }
                    None => Errand::Nothing,
                }
            }
            Target::Door(level) => self.go(level.clone(), r),
            Target::Deed(deed) => self.perform(deed.clone(), r, texts),
            Target::Ring(ring) => self.turn(*ring, true, r, texts),
            Target::Let(source) => Errand::Let(*source),
            Target::Drawer(id) => {
                if let Some(at) = self.open.iter().position(|o| o == id) {
                    self.open.remove(at);
                } else {
                    self.open.push(id);
                }
                Errand::Nothing
            }
            Target::Link(url) => Errand::Browse(url.clone()),
            Target::Ask(ask) => {
                let initial = match ask {
                    Ask::WebAddress => self
                        .draft
                        .as_ref()
                        .map(|d| d.url.clone())
                        .filter(|u| !u.is_empty()),
                    Ask::WebName => Some(self.web_name(r)).filter(|n| !n.is_empty()),
                    _ => None,
                };
                Errand::Ask { ask: *ask, initial }
            }
            Target::Candidate(choice) => self.took_icon(choice.clone(), r),
            Target::Choose(asked) => self.choose(*asked),
            // A PICKER'S ROW reaches here only when none is up.
            Target::Pick(_) => Errand::Nothing,
            Target::Answer(a) => self.answered(*a, r, texts),
            Target::Entry(key) => match key {
                Some(k) => self.go(Level::Sheet(k.clone()), r),
                None => Errand::Nothing,
            },
        }
    }

    /// Do a deed: an order for the queue, a question first, or a level.
    fn perform(&mut self, deed: Deed, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        match deed {
            Deed::Install { key, name, consent } => {
                self.install(key, name, consent, None, r, texts)
            }
            Deed::DeclareForMe {
                key,
                name,
                consent,
                machines,
            } => self.install(key, name, consent, Some(machines), r, texts),
            Deed::ForgetForever { key, name } => {
                self.ask_it(Question::Forget { key, name });
                Errand::Nothing
            }
            Deed::KeepUninstalled {
                id,
                name,
                everywhere,
            } => {
                self.ask_it(Question::Decline {
                    id,
                    name,
                    everywhere,
                });
                Errand::Nothing
            }
            Deed::BringBack { id, name } => self.queue_signed(Order::Undecline { id }, name, r),
            Deed::RefreshPins => self.queue_signed(
                Order::RefreshPins,
                texts.t(crate::key::SOFTWARE_DECLARED_DOOR),
                r,
            ),
            Deed::FindReplacement(name) => {
                self.filter = None;
                self.query = name;
                self.show_results(r)
            }
            Deed::InstallFile { path, consent } => {
                if consent.unverified {
                    let label = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    queue(Order::InstallFile { path, consent }, label)
                } else {
                    self.ask_it(Question::Foreign(path));
                    Errand::Nothing
                }
            }
            Deed::Try {
                attr,
                name,
                consent,
            } => {
                let consent = remembered(consent, &attr, &r.prefs);
                let unfree =
                    badges_for(r, &AppKey::Nix { attr: attr.clone() }).contains(&Badge::Unfree);
                if unfree && !consent.unfree {
                    self.ask_it(Question::Consent {
                        kind: ConsentKind::Unfree,
                        deed: Box::new(Deed::Try {
                            attr,
                            name: name.clone(),
                            consent,
                        }),
                        name,
                        bytes: None,
                    });
                    return Errand::Nothing;
                }
                queue(
                    Order::Try {
                        attr,
                        traces: self.traces,
                        network: self.network,
                        consent,
                    },
                    name,
                )
            }
            Deed::Launch(id) => Errand::Launch(id),
            Deed::Update { key, name } => queue(
                Order::Upgrade {
                    scope: Scope::One { key },
                },
                name,
            ),
            Deed::UpdateSource(source) => queue(
                Order::Upgrade {
                    scope: Scope::Source { source },
                },
                texts.t(words::source_key(source)),
            ),
            Deed::UpdateEverything => queue(
                Order::Upgrade {
                    scope: Scope::Everything,
                },
                texts.t(crate::key::SOFTWARE_UPDATES_ALL),
            ),
            Deed::Refresh(source) => queue(
                Order::Refresh { source },
                texts.t(words::source_key(source)),
            ),
            Deed::Remove { key, name } => {
                self.ask_it(Question::Remove { key, name });
                Errand::Nothing
            }
            Deed::Keep { trial, name } => queue(Order::Keep { trial }, name),
            Deed::Stop { trial, name } => queue(Order::StopTrial { trial }, name),
            Deed::Session(on) => queue(
                Order::AndroidSession { on },
                texts.t(words::source_key(Source::Android)),
            ),
            Deed::Prepare(source) => queue(
                Order::Prepare {
                    source,
                    image: None,
                },
                texts.t(words::source_key(source)),
            ),
            Deed::PrepareAndroid => {
                self.ask_it(Question::PrepareAndroid(self.image));
                Errand::Nothing
            }
            Deed::EraseAndroid => {
                self.ask_it(Question::EraseAndroid);
                Errand::Nothing
            }
            Deed::Tidy(source) => {
                self.ask_it(Question::CleanUp(source));
                Errand::Nothing
            }
            Deed::Rollback(n) => {
                self.ask_it(Question::Rollback(n));
                Errand::Nothing
            }
            Deed::Edit(slug) => {
                self.draft = Some(WebDraft {
                    slug: Some(slug.clone()),
                    ..WebDraft::default()
                });
                self.go(Level::WebEditor, r);
                Errand::Feed(Want::WebApp(slug))
            }
            Deed::WebNew(url) => self.web_new(&url, r, texts),
            Deed::OpenOnce(url) => Errand::OpenOnce {
                url,
                profile: self.web_profile(r),
            },
            Deed::WebSave => self.web_save(r, texts),
            Deed::WebRemove { slug, name } => {
                self.ask_it(Question::RemoveWeb { slug, name });
                Errand::Nothing
            }
            Deed::Revoke(attr) => Errand::Tweak(Tweak::Unfree {
                attr,
                allowed: false,
            }),
            Deed::Run(id) => Errand::Run(id),
            Deed::PasteImage => Errand::PasteImage,
            Deed::Permission { id, name, grant } => {
                self.ask_it(Question::Permission { id, name, grant });
                Errand::Nothing
            }
        }
    }

    /// Install — or install and declare (`declare`: the machines, `None` for
    /// every one) — asking first for whatever consent is missing.
    fn install(
        &mut self,
        key: AppKey,
        name: String,
        consent: Consent,
        declare: Option<Option<Vec<String>>>,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let badges = badges_for(r, &key);
        if let Some(why) = badges.iter().find_map(|b| b.refuses()) {
            return Errand::Say(texts.t(words::refusal_key(why)));
        }
        let consent = match &key {
            AppKey::Nix { attr } => remembered(consent, attr, &r.prefs),
            _ => consent,
        };
        let bytes = download_of(r, &key);
        let big = r.prefs.ask_above_mb > 0
            && bytes.is_some_and(|b| b > r.prefs.ask_above_mb.saturating_mul(1_000_000));
        // IN THIS ORDER: what the licence is, what the chip is, how big it is,
        // and what the CLI itself said the last time it refused.
        let wanted = [
            (ConsentKind::Unfree, badges.contains(&Badge::Unfree)),
            (ConsentKind::ArmOnly, badges.contains(&Badge::ArmOnly)),
            (ConsentKind::Big, big),
        ]
        .into_iter()
        .chain(
            last_failure(r, &key)
                .and_then(|j| j.refusal)
                .and_then(ConsentKind::of_refusal)
                .map(|k| (k, true)),
        )
        .find(|(kind, needed)| *needed && !kind.given(consent));
        if let Some((kind, _)) = wanted {
            let deed = match declare {
                Some(machines) => Deed::DeclareForMe {
                    key,
                    name: name.clone(),
                    consent,
                    machines,
                },
                None => Deed::Install {
                    key,
                    name: name.clone(),
                    consent,
                },
            };
            self.ask_it(Question::Consent {
                kind,
                deed: Box::new(deed),
                name,
                bytes,
            });
            return Errand::Nothing;
        }
        match declare {
            Some(machines) => self.queue_signed(
                Order::Declare {
                    key,
                    consent,
                    machines,
                },
                name,
                r,
            ),
            None => queue(Order::Install { key, consent }, name),
        }
    }

    /// Turn a ring.
    fn turn(
        &mut self,
        ring: Ring,
        forward: bool,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match ring {
            Ring::Filter => {
                let mut all: Vec<Option<Source>> = vec![None];
                all.extend(
                    Source::ALL
                        .into_iter()
                        .filter(|s| r.snapshot.as_ref().is_none_or(|snap| snap.supports.has(*s)))
                        .map(Some),
                );
                if self.level == Level::Results {
                    all.retain(|s| *s != Some(Source::Web));
                }
                self.filter = turned(&all, &self.filter, forward);
                if self.level == Level::Results {
                    Errand::Feed(Want::Search {
                        query: self.query.clone(),
                        sources: self.sources(r),
                    })
                } else {
                    Errand::Nothing
                }
            }
            Ring::Member => {
                let Level::Sheet(key) = &self.level else {
                    return Errand::Nothing;
                };
                let Some(g) = r.group_of(key) else {
                    return Errand::Nothing;
                };
                let keys: Vec<AppKey> = g.members.iter().map(|m| m.key.clone()).collect();
                let next = turned(&keys, key, forward);
                self.level = Level::Sheet(next.clone());
                Errand::Feed(Want::Details(next))
            }
            Ring::Traces => {
                self.traces = turned(&Traces::ALL, &self.traces, forward);
                Errand::Nothing
            }
            Ring::Network => {
                self.network = !self.network;
                Errand::Nothing
            }
            Ring::Shelf => {
                let Level::Icons(shelf) = self.level else {
                    return Errand::Nothing;
                };
                let next = shelf.turned(forward);
                self.level = Level::Icons(next);
                self.at = Spot::Ring("shelf");
                match next {
                    Shelf::Dashboard | Shelf::Collection => Errand::Feed(Want::Icons {
                        shelf: next,
                        query: self.icon_query.clone(),
                    }),
                    Shelf::Site | Shelf::File => Errand::Nothing,
                }
            }
            Ring::Profile => {
                let all: Vec<String> = r.profiles.iter().map(|p| p.directory.clone()).collect();
                if all.is_empty() {
                    return Errand::Nothing;
                }
                let now = self.web_profile(r);
                let next = turned(&all, &now, forward);
                let draft = self.draft_mut(r);
                draft.profile = Some(next);
                draft.changed = true;
                Errand::Nothing
            }
            Ring::Category => {
                let all: Vec<String> = ronoctua_software::web::CATEGORIES
                    .iter()
                    .map(|c| (*c).to_owned())
                    .collect();
                let now = self.web_category(r);
                let next = turned(&all, &now, forward);
                let draft = self.draft_mut(r);
                draft.category = Some(next);
                draft.changed = true;
                Errand::Nothing
            }
            Ring::Image => {
                self.image = turned(&AndroidImage::ALL, &self.image, forward);
                Errand::Nothing
            }
            Ring::Prefer => Errand::Tweak(Tweak::Prefer(turned(
                &Prefer::ALL,
                &r.prefs.prefer,
                forward,
            ))),
            Ring::Online => Errand::Tweak(Tweak::Online(!r.prefs.online)),
            Ring::Switch(s) => Errand::Tweak(Tweak::Switch(s, !s.is_on(&r.prefs))),
            Ring::AskAbove => Errand::Tweak(Tweak::AskAboveMb(turned(
                &ASK_ABOVE,
                &r.prefs.ask_above_mb,
                forward,
            ))),
            Ring::DefaultTraces => Errand::Tweak(Tweak::Traces(turned(
                &Traces::ALL,
                &r.prefs.traces,
                forward,
            ))),
            Ring::DefaultNetwork => Errand::Tweak(Tweak::TrialNetwork(!r.prefs.trial_network)),
            Ring::WebProfile => {
                let mut all: Vec<Option<String>> = vec![None];
                all.extend(r.profiles.iter().map(|p| Some(p.directory.clone())));
                Errand::Tweak(Tweak::WebProfile(turned(
                    &all,
                    &r.prefs.web_profile,
                    forward,
                )))
            }
            // A PERMISSION IS AN ORDER, never a preference: `flatpak override`
            // is the file of record, and the ring shows what it says once the
            // queue is done and the page reads it again (`View::rereads`).
            Ring::Grant(which) => {
                let Level::Permissions(id) = &self.level else {
                    return Errand::Nothing;
                };
                let Some((_, Some(Some(found)))) = r.permissions.as_ref().filter(|(a, _)| a == id)
                else {
                    return Errand::Nothing;
                };
                match which.turned(&found.grants, forward) {
                    Some(grant) => queue(
                        Order::Permission {
                            id: id.clone(),
                            grant,
                        },
                        flatpak_name(r, id),
                    ),
                    None => Errand::Nothing,
                }
            }
            Ring::Windows => {
                let multi = r
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.android.multi_window == Some(true));
                queue(
                    Order::AndroidWindows { multi: !multi },
                    texts.t(words::source_key(Source::Android)),
                )
            }
        }
    }

    // ─── the web app's draft ────────────────────────────────────────────────

    /// A new web app from an address.
    fn web_new(&mut self, typed: &str, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        match ronoctua_software::web::canonical_url(typed) {
            Ok(url) => {
                self.draft = Some(WebDraft {
                    url: url.clone(),
                    adopted: true,
                    ..WebDraft::default()
                });
                self.go(Level::WebEditor, r);
                Errand::Feed(Want::Site(url))
            }
            Err(why) => Errand::Say(texts.t(words::refusal_key(why))),
        }
    }

    /// The draft, with the existing web app's file read into it the first
    /// time it is touched.
    fn draft_mut(&mut self, r: &Readings) -> &mut WebDraft {
        let draft = self.draft.get_or_insert_with(WebDraft::default);
        if !draft.adopted
            && let Some(slug) = &draft.slug
            && let Some((asked, Some(Some(app)))) = &r.web_app
            && asked == slug
        {
            draft.name.clone_from(&app.name);
            draft.url.clone_from(&app.url);
            draft.profile = Some(app.profile.clone());
            draft.icon.clone_from(&app.icon);
            draft.category.clone_from(&app.category);
            draft.adopted = true;
        }
        draft
    }

    /// The draft as it would be saved: what was typed, else what arrived.
    fn effective_draft(&self, r: &Readings) -> WebDraft {
        let mut d = self.draft.clone().unwrap_or_default();
        if !d.adopted
            && let Some(slug) = &d.slug
            && let Some((asked, Some(Some(app)))) = &r.web_app
            && asked == slug
        {
            d = WebDraft {
                slug: d.slug.clone(),
                name: if d.name.is_empty() {
                    app.name.clone()
                } else {
                    d.name
                },
                url: if d.url.is_empty() {
                    app.url.clone()
                } else {
                    d.url
                },
                profile: d.profile.or_else(|| Some(app.profile.clone())),
                icon: d.icon.or_else(|| app.icon.clone()),
                category: d.category.or_else(|| app.category.clone()),
                changed: d.changed,
                adopted: true,
            };
        }
        d
    }

    /// The draft's name: typed, else the page's own, else its host.
    fn web_name(&self, r: &Readings) -> String {
        let d = self.effective_draft(r);
        if !d.name.is_empty() {
            return d.name;
        }
        if let Some((url, Some(Ok((Some(title), _))))) = &r.site
            && *url == d.url
            && !title.trim().is_empty()
        {
            return title.trim().to_owned();
        }
        host_of(&d.url)
    }

    /// The draft's icon: chosen, else the page's best.
    fn web_icon(&self, r: &Readings) -> Option<IconChoice> {
        let d = self.effective_draft(r);
        d.icon.or_else(|| match &r.site {
            Some((url, Some(Ok((_, found))))) if *url == d.url => {
                found.first().map(|c| c.choice.clone())
            }
            _ => None,
        })
    }

    /// The draft's Chrome profile: chosen, else the preference, else Chrome's
    /// last used, else the one Chrome makes first.
    fn web_profile(&self, r: &Readings) -> String {
        self.effective_draft(r)
            .profile
            .or_else(|| r.prefs.web_profile.clone())
            .or_else(|| {
                r.profiles
                    .iter()
                    .find(|p| p.last_used)
                    .map(|p| p.directory.clone())
            })
            .unwrap_or_else(|| ronoctua_software::web::DEFAULT_PROFILE.to_owned())
    }

    /// The draft's category: chosen, else the one a web app falls back to.
    fn web_category(&self, r: &Readings) -> String {
        self.effective_draft(r)
            .category
            .unwrap_or_else(|| ronoctua_software::web::FALLBACK_CATEGORY.to_owned())
    }

    /// What the icon shelves look for first: the name, else the host's own
    /// name — so the first screen of Dashboard Icons already has matches.
    fn draft_query(&self, r: &Readings) -> String {
        let name = self.web_name(r);
        if name.is_empty() {
            ronoctua_software::dashboard::query_of_host(&host_of(&self.effective_draft(r).url))
        } else {
            name
        }
    }

    /// An icon was chosen: it is the draft's, and the editor comes back.
    fn took_icon(&mut self, choice: IconChoice, r: &Readings) -> Errand {
        let draft = self.draft_mut(r);
        draft.icon = Some(choice);
        draft.changed = true;
        while matches!(self.level, Level::Icons(_)) {
            self.pop();
        }
        if self.level == Level::WebEditor {
            self.at = Spot::Ring("icon");
        }
        Errand::Nothing
    }

    /// Save the draft: a new web app, or the changed one.
    fn web_save(&mut self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let d = self.effective_draft(r);
        let url = match ronoctua_software::web::canonical_url(&d.url) {
            Ok(u) => u,
            Err(why) => return Errand::Say(texts.t(words::refusal_key(why))),
        };
        let Some(icon) = self.web_icon(r) else {
            return Errand::Say(texts.t(crate::key::SOFTWARE_WEB_NO_ICON));
        };
        let name = self.web_name(r);
        let spec = WebSpec {
            name: name.clone(),
            url,
            profile: self.web_profile(r),
            icon,
            prepared: None,
            category: Some(self.web_category(r)),
        };
        let order = match d.slug {
            Some(slug) => Order::WebEdit { slug, spec },
            None => Order::WebCreate { spec },
        };
        self.draft = None;
        self.pop();
        queue(order, name)
    }

    // ─── the rows ───────────────────────────────────────────────────────────

    /// The rows, with the cursor on `focus`.
    fn build(
        &self,
        r: &Readings,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
        focus: &Spot,
    ) -> Vec<Row> {
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            focus,
            open: &self.open,
            texts,
        };
        if let Some(q) = &self.asking {
            question(&mut b, q);
            return b.out;
        }
        match &self.level {
            Level::Home => self.home(&mut b, r),
            Level::Results => self.results(&mut b, r),
            Level::Sheet(key) => self.sheet(&mut b, r, key),
            Level::Trials => self.trials(&mut b, r),
            Level::WebEditor => self.web_editor(&mut b, r),
            Level::Icons(shelf) => self.icons(&mut b, r, *shelf),
            Level::Android => self.android(&mut b, r),
            Level::Maintenance => maintenance(&mut b, r),
            Level::Advanced => self.advanced(&mut b, r),
            Level::Permissions(id) => permissions(&mut b, r, id),
            Level::Consents => consents(&mut b, r),
            Level::History => history(&mut b, r),
            Level::Declared => declared_level(&mut b, r),
            Level::Declaration(id) => declaration_sheet(&mut b, r, id),
            Level::Declined => declined_level(&mut b, r),
            Level::Distro(id) => {
                // THE DISTRO'S PROGRAM, or a decline of one it no longer
                // carries — which can still be brought back.
                let named = declarations(r).and_then(|d| {
                    d.distro(id)
                        .map(|x| x.name.clone())
                        .or_else(|| d.decline_of(id).map(|x| x.name.clone()))
                });
                if let Some(name) = named {
                    b.title(name);
                    distro_rows(&mut b, r, id);
                } else {
                    b.heading(crate::key::SOFTWARE_DECLARED_HEADING_DISTRO);
                    b.said(
                        b.texts.t(words::refusal_key(Refusal::UnknownItem)),
                        tokens::color::TEXT_DIM,
                    );
                }
            }
        }
        b.out
    }

    /// The first page.
    fn home(&self, b: &mut Builder<'_>, r: &Readings) {
        use crate::key;
        b.heading(key::SOFTWARE_TITLE);
        self.search_row(b, key::SOFTWARE_SEARCH);
        let Some(s) = &r.snapshot else {
            b.note(key::SOFTWARE_READING);
            return;
        };
        b.heading(key::SOFTWARE_HEADING_INSTALLED);
        b.note(key::SOFTWARE_NOTE_YOURS);
        if s.installed.is_empty() {
            let t = b.texts.t_with(
                key::SOFTWARE_NOTE_NONE,
                &[("keys", r.taught.launcher.as_str())],
            );
            b.said(t, tokens::color::TEXT_DIM);
        } else {
            b.ring(
                key::SOFTWARE_FILTER,
                filter_said(self.filter, b.texts),
                Ring::Filter,
                "filter",
            );
            let mine: Vec<&Installed> = s
                .installed
                .iter()
                .filter(|i| self.filter.is_none() || i.key.source() == self.filter)
                .collect();
            if mine.is_empty() {
                b.note(key::SOFTWARE_NOTE_NONE_HERE);
            }
            for i in mine {
                installed_row(b, i, r);
            }
        }
        if !s.trials.is_empty() {
            b.heading(key::SOFTWARE_HEADING_TRYING);
            for t in &s.trials {
                trial_row(b, t, r);
            }
        }
        b.heading(key::SOFTWARE_HEADING_LOOKING_AFTER);
        let updates = s.installed.iter().filter(|i| i.update.is_some()).count();
        b.door(
            key::SOFTWARE_UPDATES,
            updates_said(updates, b.texts),
            Level::Maintenance,
        );
        // THE DECLARATION: what follows the person, and what of the distro's
        // they keep away.
        b.door(
            key::SOFTWARE_DECLARED_DOOR,
            declared_said(&s.declarations, b.texts),
            Level::Declared,
        );
        b.door(
            key::SOFTWARE_DECLARED_DOOR_DECLINED,
            declined_said(&s.declarations, b.texts),
            Level::Declined,
        );
        b.heading(key::SOFTWARE_HEADING_MORE);
        let web = b.texts.t(key::SOFTWARE_WEB_NEW_VALUE);
        let refused = (!s.supports.web).then(|| b.texts.t(words::refusal_key(Refusal::NoBrowser)));
        b.pair(
            b.texts.t(key::SOFTWARE_WEB_NEW),
            web,
            Target::Ask(Ask::WebNew),
            Spot::Deed("web-new"),
            Can::default(),
            refused,
        );
        let running = s.trials.iter().filter(|t| t.running).count();
        b.door(
            key::SOFTWARE_TRIALS,
            trials_said(running, b.texts),
            Level::Trials,
        );
        b.door(
            key::SOFTWARE_ANDROID_STATE,
            b.texts.t(android_state_key(&s.android)),
            Level::Android,
        );
        b.door(
            key::SOFTWARE_ADVANCED,
            b.texts.t(key::SOFTWARE_ADVANCED_VALUE),
            Level::Advanced,
        );
    }

    /// The search's own row: what was typed, or how to start.
    fn search_row(&self, b: &mut Builder<'_>, label: &str) {
        let value = if self.query.is_empty() {
            b.texts.t(crate::key::SOFTWARE_SEARCH_EMPTY)
        } else {
            self.query.clone()
        };
        b.pair(
            b.texts.t(label),
            value,
            Target::Search,
            Spot::Search,
            Can::default(),
            None,
        );
    }

    /// What a search found.
    fn results(&self, b: &mut Builder<'_>, r: &Readings) {
        use crate::key;
        b.heading(key::SOFTWARE_HEADING_SEARCH);
        self.search_row(b, key::SOFTWARE_SEARCH);
        b.ring(
            key::SOFTWARE_FILTER,
            filter_said(self.filter, b.texts),
            Ring::Filter,
            "filter",
        );
        let query = self.query.trim();
        // WHAT THE QUERY ITSELF IS, before what it finds: an address makes a
        // web app, a path installs a file.
        if let Some(url) = address_in(query) {
            let refused = r
                .snapshot
                .as_ref()
                .is_some_and(|s| !s.supports.web)
                .then(|| b.texts.t(words::refusal_key(Refusal::NoBrowser)));
            b.deed(
                key::SOFTWARE_MAKE_WEB_APP,
                host_of(&url),
                Deed::WebNew(url.clone()),
                refused.clone(),
            );
            b.deed(
                key::SOFTWARE_OPEN_ONCE,
                String::new(),
                Deed::OpenOnce(url),
                refused,
            );
        }
        if let Some(path) = package_file_in(query, r.home.as_deref()) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            b.deed(
                key::SOFTWARE_INSTALL_FILE,
                name,
                Deed::InstallFile {
                    path,
                    consent: Consent::default(),
                },
                None,
            );
        }
        if query.is_empty() {
            b.note(key::SOFTWARE_NOTE_TYPE);
            return;
        }
        let groups = &r.search.groups;
        let (named, described): (Vec<&Group>, Vec<&Group>) =
            groups.iter().partition(|g| !g.by_description);
        if !named.is_empty() {
            b.heading(key::SOFTWARE_HEADING_RESULTS);
            for g in named {
                group_row(b, g, r);
            }
        }
        if !described.is_empty() {
            b.heading(key::SOFTWARE_HEADING_BY_DESCRIPTION);
            for g in described {
                group_row(b, g, r);
            }
        }
        let asking = r.search.asked.iter().any(|(_, s)| *s == Status::Asking);
        if groups.is_empty() && !asking {
            b.note(key::SOFTWARE_NOTE_NOTHING_FOUND);
        }
        // HOW EACH SOURCE STANDS: asking, off, refused, or what its answer is
        // based on. Nothing hides silently — a source that did not answer is a
        // row that says so.
        let said: Vec<(Source, &Status, String)> = r
            .search
            .asked
            .iter()
            .filter_map(|(s, st)| words::status_said(st, b.texts).map(|t| (*s, st, t)))
            .collect();
        if !said.is_empty() {
            b.heading(key::SOFTWARE_HEADING_ASKED);
            for (source, status, text) in said {
                // A SOURCE THAT IS NOT READY IS ONE KEY AWAY FROM READY: F-Droid's
                // catalogue is fetched, Flathub's remote is set up — through
                // the queue, and the search never does either by itself.
                let ready = match (source, status) {
                    (
                        Source::Android,
                        Status::Refused {
                            refusal: Refusal::SourceUnprepared,
                        },
                    ) => Some(Deed::Refresh(Source::Android)),
                    (
                        Source::Flatpak,
                        Status::Refused {
                            refusal: Refusal::SourceUnprepared,
                        },
                    ) => Some(Deed::Prepare(Source::Flatpak)),
                    _ => None,
                };
                let label = b.texts.t(words::source_key(source));
                match ready {
                    Some(deed) => {
                        b.pair(
                            label,
                            text,
                            Target::Deed(deed),
                            Spot::Status(source),
                            Can::default(),
                            None,
                        );
                    }
                    None => b.fact_at(label, text, Spot::Status(source)),
                }
            }
        }
        if r.search.asked.iter().any(|(_, s)| {
            *s == Status::Refused {
                refusal: Refusal::Offline,
            }
        }) {
            b.note(key::SOFTWARE_NOTE_OFFLINE);
        }
    }

    /// One program's sheet.
    fn sheet(&self, b: &mut Builder<'_>, r: &Readings, app: &AppKey) {
        use crate::key;
        let installed = r
            .snapshot
            .as_ref()
            .and_then(|s| s.installed.iter().find(|i| i.key == *app));
        let found = found_of(r, app);
        let details = details_of(r, app);
        let name = installed
            .map(|i| i.name.clone())
            .or_else(|| found.map(|f| f.name.clone()))
            .or_else(|| {
                details
                    .and_then(|d| d.ok())
                    .map(|d| d.name.clone())
                    .filter(|n| !n.is_empty())
            })
            .or_else(|| {
                self.outside
                    .iter()
                    .find(|(o, _)| o == app)
                    .map(|(_, n)| n.clone())
            })
            .unwrap_or_else(|| app.id().to_owned());
        let picture = installed
            .and_then(|i| i.icon.clone())
            .or_else(|| found.and_then(|f| f.icon.clone()))
            .or_else(|| details.and_then(|d| d.ok()).and_then(|d| d.icon.clone()));
        b.title_with(
            words::key_glyph(app),
            picture.map(Portrait::Program),
            name.clone(),
        );

        // WHAT THE QUEUE IS DOING TO IT, or what it said last time it failed.
        if let Some(job) = job_of(&r.jobs, app) {
            if job.stage.afoot() {
                b.fact_at(
                    b.texts.t(key::SOFTWARE_ROW_WORKING),
                    working_said(job, b.texts),
                    Spot::Fact("working"),
                );
            } else if job.stage == ronoctua_transfers::Stage::Failed {
                let said = job.refusal.map_or_else(
                    || job.trouble.clone().unwrap_or_default(),
                    |why| b.texts.t(words::refusal_key(why)),
                );
                b.said(said, tokens::color::STATE_ERROR);
            }
        }

        if let AppKey::System { desktop_id } = app {
            b.deed_at(
                key::SOFTWARE_ROW_LAUNCH,
                String::new(),
                Deed::Launch(desktop_id.clone()),
                Spot::Primary,
                None,
            );
            // ONE OF THE DISTRO'S PROGRAMS: what it is, and keeping it
            // uninstalled — or why the distro needs it.
            if let Some(d) = declarations(r).and_then(|d| d.distro_of(app, &[])) {
                distro_rows(b, r, &d.app.id);
                return;
            }
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_SOURCE),
                b.texts.t(key::SOFTWARE_SOURCE_SYSTEM),
                Spot::Fact("source"),
            );
            let refused = b.texts.t(words::refusal_key(Refusal::DeclaredInTheFlake));
            b.deed_at(
                key::SOFTWARE_ROW_REMOVE,
                String::new(),
                Deed::Remove {
                    key: app.clone(),
                    name,
                },
                Spot::Deed("remove"),
                Some(refused),
            );
            return;
        }

        let trial = match app {
            AppKey::Nix { attr } => r
                .snapshot
                .as_ref()
                .and_then(|s| s.trials.iter().find(|t| t.attr == *attr)),
            _ => None,
        };
        match installed {
            Some(i) => installed_sheet(b, i, r),
            None => self.found_sheet(b, r, app, &name, trial),
        }
        about(b, found, details, app, installed);
    }

    /// The sheet of something not installed.
    fn found_sheet(
        &self,
        b: &mut Builder<'_>,
        r: &Readings,
        app: &AppKey,
        name: &str,
        trial: Option<&Trial>,
    ) {
        use crate::key;
        let snap = r.snapshot.as_ref();
        let source = app.source();
        let badges = badges_for(r, app);
        // A TRIAL OF IT IS RUNNING OR ENDED: keeping it is the first verb.
        if let Some(t) = trial {
            b.deed_at(
                key::SOFTWARE_ROW_KEEP,
                b.texts.t(key::SOFTWARE_KEEP_VALUE),
                Deed::Keep {
                    trial: t.key.clone(),
                    name: name.to_owned(),
                },
                Spot::Primary,
                None,
            );
            if t.running {
                b.deed(
                    key::SOFTWARE_ROW_STOP,
                    String::new(),
                    Deed::Stop {
                        trial: t.key.clone(),
                        name: name.to_owned(),
                    },
                    None,
                );
            }
        }
        // INSTALL, with how and how much — or why not.
        let refused = source
            .and_then(|s| refusal_of_source(s, snap))
            .or_else(|| badges.iter().find_map(|x| x.refuses()))
            .map(|why| b.texts.t(words::refusal_key(why)));
        let recommended = r
            .group_of(app)
            .is_some_and(|g| g.best().key == *app && g.members.len() > 1);
        let how = source.map(|s| {
            let with = b.texts.t_with(
                key::SOFTWARE_INSTALL_WITH,
                &[("source", b.texts.t(words::source_key(s)).as_str())],
            );
            if recommended {
                joined(&with, &b.texts.t(key::SOFTWARE_RECOMMENDED), b.texts)
            } else {
                with
            }
        });
        let size = download_of(r, app).map(|bytes| {
            b.texts.t_with(
                key::SOFTWARE_SIZE_DOWNLOAD,
                &[("size", readable(bytes, b.texts).as_str())],
            )
        });
        let value = match (how, size) {
            (Some(h), Some(s)) => joined(&h, &s, b.texts),
            (Some(h), None) => h,
            (None, Some(s)) => s,
            (None, None) => String::new(),
        };
        // INSTALL ONLY FOR YOU, one click: here now, and declared — so the
        // person's other computers get it, and every update keeps it.
        b.deed_at(
            key::SOFTWARE_DECLARED_ROW_FOR_ME,
            joined(
                &b.texts.t(key::SOFTWARE_DECLARED_EVERYWHERE),
                &value,
                b.texts,
            ),
            Deed::DeclareForMe {
                key: app.clone(),
                name: name.to_owned(),
                consent: Consent::default(),
                machines: None,
            },
            if trial.is_some() {
                Spot::Deed("declare-for-me")
            } else {
                Spot::Primary
            },
            refused.clone(),
        );
        // OR ON THIS COMPUTER ONLY, as before: undeclared, nothing keeps it.
        b.deed(
            key::SOFTWARE_DECLARED_ROW_HERE,
            b.texts.t(key::SOFTWARE_DECLARED_HERE_VALUE),
            Deed::Install {
                key: app.clone(),
                name: name.to_owned(),
                consent: Consent::default(),
            },
            refused,
        );
        // TRY — only a Nix package runs without being installed, and the
        // others say so rather than vanish.
        match app {
            AppKey::Nix { attr } => {
                let program = program_for(r, app);
                let value = if badges.contains(&Badge::CommandLine) {
                    b.texts.t_with(
                        key::SOFTWARE_TRY_TERMINAL,
                        &[("program", program.as_deref().unwrap_or(attr))],
                    )
                } else {
                    b.texts.t(key::SOFTWARE_TRY_WINDOW)
                };
                b.deed_at(
                    key::SOFTWARE_ROW_TRY,
                    value,
                    Deed::Try {
                        attr: attr.clone(),
                        name: name.to_owned(),
                        consent: Consent::default(),
                    },
                    Spot::Deed("try"),
                    snap.is_some_and(|s| !s.supports.nix)
                        .then(|| b.texts.t(words::refusal_key(Refusal::SourceAbsent))),
                );
                b.ring(
                    key::SOFTWARE_ROW_TRACES,
                    b.texts.t(words::traces_key(self.traces)),
                    Ring::Traces,
                    "traces",
                );
                if self.traces == Traces::Sandbox {
                    b.ring(
                        key::SOFTWARE_ROW_NETWORK,
                        on_off(self.network, b.texts),
                        Ring::Network,
                        "network",
                    );
                }
            }
            _ => b.deed_at(
                key::SOFTWARE_ROW_TRY,
                String::new(),
                Deed::Try {
                    attr: String::new(),
                    name: name.to_owned(),
                    consent: Consent::default(),
                },
                Spot::Deed("try"),
                Some(b.texts.t(words::refusal_key(Refusal::CannotTry))),
            ),
        }
        // WHICH SOURCE, when it is in several.
        if let Some(g) = r.group_of(app)
            && g.members.len() > 1
            && let Some(s) = source
        {
            let others: Vec<String> = g
                .members
                .iter()
                .filter(|m| m.key != *app)
                .filter_map(|m| m.key.source())
                .map(|s| b.texts.t(words::source_key(s)))
                .collect();
            let value = joined(
                &b.texts.t(words::source_key(s)),
                &b.texts.t_with(
                    key::SOFTWARE_ALSO_IN,
                    &[("sources", others.join(", ").as_str())],
                ),
                b.texts,
            );
            b.ring(key::SOFTWARE_ROW_SOURCE, value, Ring::Member, "member");
        }
        let version = offered_version(r, app);
        if !version.is_empty() {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_VERSION),
                version,
                Spot::Fact("version"),
            );
        }
        if let Some(Ok(d)) = details_of(r, app)
            && let Some(bytes) = d.installed_bytes
        {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_SIZE),
                readable(bytes, b.texts),
                Spot::Fact("size"),
            );
        }
        if let Some(s) = source {
            b.note(words::why_key(s));
        }
        if app.source() == Some(Source::Nix) {
            b.note(key::SOFTWARE_NOTE_TRIAL);
        }
    }

    /// What is being tried.
    fn trials(&self, b: &mut Builder<'_>, r: &Readings) {
        use crate::key;
        b.heading(key::SOFTWARE_HEADING_TRIALS);
        let value = if self.query.is_empty() {
            b.texts.t(key::SOFTWARE_TRY_NEW_VALUE)
        } else {
            self.query.clone()
        };
        b.pair(
            b.texts.t(key::SOFTWARE_TRY_NEW),
            value,
            Target::Search,
            Spot::Search,
            Can::default(),
            None,
        );
        b.note(key::SOFTWARE_NOTE_TRIAL);
        let Some(s) = &r.snapshot else {
            b.note(key::SOFTWARE_READING);
            return;
        };
        if s.trials.is_empty() {
            b.note(key::SOFTWARE_NOTE_NO_TRIALS);
        }
        for t in &s.trials {
            trial_row(b, t, r);
        }
    }

    /// The web app's editor.
    fn web_editor(&self, b: &mut Builder<'_>, r: &Readings) {
        use crate::key;
        let d = self.effective_draft(r);
        if d.slug.is_some() {
            b.title(self.web_name(r));
        } else {
            b.heading(key::SOFTWARE_HEADING_WEB);
        }
        // A DECLARED ONE is changed in the declaration: saving declares it
        // again, and the person's other computers take the change.
        if let Some(slug) = &d.slug
            && declarations(r)
                .is_some_and(|x| x.declared_as(&AppKey::Web { slug: slug.clone() }).is_some())
        {
            b.note(key::SOFTWARE_DECLARED_WEB_EDIT);
        }
        if d.slug.is_some() && !d.adopted {
            match &r.web_app {
                Some((_, Some(None))) => {
                    b.said(
                        b.texts.t(words::refusal_key(Refusal::UnknownItem)),
                        tokens::color::STATE_ERROR,
                    );
                }
                _ => b.note(key::SOFTWARE_READING),
            }
        }
        let url_refused = match ronoctua_software::web::canonical_url(&d.url) {
            Ok(_) => None,
            Err(why) => Some(b.texts.t(words::refusal_key(why))),
        };
        b.pair(
            b.texts.t(key::SOFTWARE_WEB_ADDRESS),
            if d.url.is_empty() {
                b.texts.t(key::SOFTWARE_WEB_NO_ADDRESS)
            } else {
                d.url.clone()
            },
            Target::Ask(Ask::WebAddress),
            Spot::Ring("address"),
            Can::default(),
            None,
        );
        b.pair(
            b.texts.t(key::SOFTWARE_WEB_NAME),
            self.web_name(r),
            Target::Ask(Ask::WebName),
            Spot::Ring("name"),
            Can::default(),
            None,
        );
        let icon = self.web_icon(r);
        let icon_said = icon.as_ref().map_or_else(
            || b.texts.t(key::SOFTWARE_WEB_NO_ICON_YET),
            |c| b.texts.t(crate::software::shelf_key(Shelf::of(c))),
        );
        let shelf = icon.as_ref().map_or(Shelf::Site, Shelf::of);
        b.picture_pair(
            b.texts.t(key::SOFTWARE_WEB_ICON),
            icon_said,
            Target::Door(Level::Icons(shelf)),
            Spot::Ring("icon"),
            icon.map(Portrait::Candidate),
        );
        let profile = self.web_profile(r);
        let profile_said = r
            .profiles
            .iter()
            .find(|p| p.directory == profile)
            .map_or(profile.clone(), |p| p.name.clone());
        b.ring(
            key::SOFTWARE_WEB_PROFILE,
            profile_said,
            Ring::Profile,
            "profile",
        );
        let category = self.web_category(r);
        b.ring(
            key::SOFTWARE_WEB_CATEGORY,
            words::category_key(&category).map_or(category.clone(), |c| b.texts.t(c)),
            Ring::Category,
            "category",
        );
        // THE PAGE, BEING READ: said, never guessed at. A page that could not
        // be read leaves the name as its host, and says so.
        match &r.site {
            Some((url, None)) if *url == d.url => b.note(key::SOFTWARE_WEB_FETCHING),
            Some((url, Some(Err(why)))) if *url == d.url => {
                let t = joined(
                    &b.texts.t(words::refusal_key(*why)),
                    &b.texts.t(key::SOFTWARE_WEB_HOST_NAME),
                    b.texts,
                );
                b.said(t, tokens::color::TEXT_DIM);
            }
            _ => {}
        }
        let supports = r.snapshot.as_ref().is_none_or(|s| s.supports.web);
        let save_refused = if supports {
            url_refused.clone().or_else(|| {
                self.web_icon(r)
                    .is_none()
                    .then(|| b.texts.t(key::SOFTWARE_WEB_NO_ICON))
            })
        } else {
            Some(b.texts.t(words::refusal_key(Refusal::NoBrowser)))
        };
        b.deed_at(
            if d.slug.is_some() {
                key::SOFTWARE_WEB_SAVE
            } else {
                key::SOFTWARE_WEB_CREATE
            },
            String::new(),
            Deed::WebSave,
            Spot::Primary,
            save_refused,
        );
        b.deed(
            key::SOFTWARE_WEB_ONCE,
            String::new(),
            Deed::OpenOnce(d.url.clone()),
            if supports {
                url_refused
            } else {
                Some(b.texts.t(words::refusal_key(Refusal::NoBrowser)))
            },
        );
        if let Some(slug) = &d.slug {
            b.deed(
                key::SOFTWARE_WEB_REMOVE,
                String::new(),
                Deed::WebRemove {
                    slug: slug.clone(),
                    name: self.web_name(r),
                },
                None,
            );
        }
        b.note(key::SOFTWARE_NOTE_WEB);
    }

    /// The draft's icon, from one shelf.
    fn icons(&self, b: &mut Builder<'_>, r: &Readings, shelf: Shelf) {
        use crate::key;
        b.heading(key::SOFTWARE_HEADING_ICON);
        b.ring(
            key::SOFTWARE_SHELF,
            b.texts.t(crate::software::shelf_key(shelf)),
            Ring::Shelf,
            "shelf",
        );
        let chosen = self.web_icon(r);
        let d = self.effective_draft(r);
        let answer: Option<Result<&Vec<ronoctua_software::icons::Candidate>, Refusal>> = match shelf
        {
            Shelf::Site => match &r.site {
                Some((url, Some(a))) if *url == d.url => {
                    Some(a.as_ref().map(|(_, c)| c).map_err(|e| *e))
                }
                _ => None,
            },
            Shelf::Dashboard | Shelf::Collection => {
                let value = if self.icon_query.is_empty() {
                    b.texts.t(key::SOFTWARE_SEARCH_EMPTY)
                } else {
                    self.icon_query.clone()
                };
                b.pair(
                    b.texts.t(key::SOFTWARE_ICON_LOOKING_FOR),
                    value,
                    Target::Search,
                    Spot::Search,
                    Can::default(),
                    None,
                );
                match &r.candidates {
                    Some(c) if c.shelf == shelf && c.query == self.icon_query => {
                        c.answer.as_ref().map(|a| a.as_ref().map_err(|e| *e))
                    }
                    _ => None,
                }
            }
            Shelf::File => {
                b.pair(
                    b.texts.t(key::SOFTWARE_ICON_TYPED),
                    String::new(),
                    Target::Choose(Picking::Icon),
                    Spot::Deed("icon-path"),
                    Can::default(),
                    None,
                );
                b.deed(
                    key::SOFTWARE_ICON_CLIPBOARD,
                    String::new(),
                    Deed::PasteImage,
                    None,
                );
                b.deed(
                    key::SOFTWARE_ICON_FILES,
                    String::new(),
                    Deed::Run("files-window"),
                    None,
                );
                b.note(key::SOFTWARE_NOTE_DROP);
                if let Some(IconChoice::File { path }) = &chosen {
                    b.picture_pair(
                        b.texts.t(key::SOFTWARE_ICON_NOW),
                        path.display().to_string(),
                        Target::Nothing,
                        Spot::Fact("icon-now"),
                        Some(Portrait::Candidate(IconChoice::File { path: path.clone() })),
                    );
                }
                return;
            }
        };
        match answer {
            None if shelf == Shelf::Site && d.url.is_empty() => {
                b.note(key::SOFTWARE_WEB_NO_ADDRESS)
            }
            None => b.note(key::SOFTWARE_ASKING),
            Some(Err(why)) => b.said(b.texts.t(words::refusal_key(why)), tokens::color::TEXT_DIM),
            Some(Ok(list)) if list.is_empty() => b.note(key::SOFTWARE_NOTE_NO_ICONS),
            Some(Ok(list)) => {
                for c in list {
                    let label = match &c.choice {
                        IconChoice::Dashboard { name, .. } | IconChoice::Collection { name } => {
                            name.clone()
                        }
                        IconChoice::Site { url, .. } => host_of(url),
                        IconChoice::File { path } => path.display().to_string(),
                    };
                    let value = if chosen.as_ref() == Some(&c.choice) {
                        joined(&c.about, &b.texts.t(key::SOFTWARE_ICON_CHOSEN), b.texts)
                    } else {
                        c.about.clone()
                    };
                    b.picture_pair(
                        label,
                        value,
                        Target::Candidate(c.choice.clone()),
                        Spot::Candidate(c.choice.clone()),
                        Some(Portrait::Candidate(c.choice.clone())),
                    );
                }
            }
        }
    }

    /// Android.
    fn android(&self, b: &mut Builder<'_>, r: &Readings) {
        use crate::key;
        b.heading(key::SOFTWARE_HEADING_ANDROID);
        let Some(s) = &r.snapshot else {
            b.note(key::SOFTWARE_READING);
            return;
        };
        let a = &s.android;
        if !a.support {
            b.said(
                b.texts.t(words::refusal_key(Refusal::SourceAbsent)),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        b.note(key::SOFTWARE_NOTE_ANDROID);
        let state = b.texts.t(android_state_key(a));
        if a.prepared {
            let mut can = Can::default();
            if a.session {
                can.stop = Some(Deed::Session(false));
            }
            b.pair(
                b.texts.t(key::SOFTWARE_ANDROID_STATE),
                state,
                Target::Deed(Deed::Session(!a.session)),
                Spot::Primary,
                can,
                None,
            );
        } else {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ANDROID_STATE),
                state,
                Spot::Fact("state"),
            );
        }
        if !a.prepared && !a.preparing {
            b.ring(
                key::SOFTWARE_ANDROID_IMAGE,
                b.texts.t(words::image_key(self.image)),
                Ring::Image,
                "image",
            );
            if self.image == AndroidImage::Gapps {
                b.note(key::SOFTWARE_NOTE_GAPPS);
            }
            b.deed_at(
                key::SOFTWARE_ANDROID_PREPARE,
                String::new(),
                Deed::PrepareAndroid,
                Spot::Primary,
                None,
            );
            return;
        }
        if a.preparing {
            b.note(key::SOFTWARE_NOTE_PREPARING);
            return;
        }
        // THE PLAY STORE ASKS FOR THE DEVICE TO BE REGISTERED, once, as an
        // uncertified one: the id to paste, and the page to paste it in.
        if a.image == Some(AndroidImage::Gapps) {
            b.heading(key::SOFTWARE_HEADING_REGISTER);
            b.note(key::SOFTWARE_NOTE_UNCERTIFIED);
            b.fact_at(
                b.texts.t(key::SOFTWARE_ANDROID_ID),
                a.android_id
                    .clone()
                    .unwrap_or_else(|| b.texts.t(key::SOFTWARE_ANDROID_ID_UNKNOWN)),
                Spot::Fact("android-id"),
            );
            b.pair(
                b.texts.t(key::SOFTWARE_ANDROID_REGISTER),
                ronoctua_software::android::UNCERTIFIED.to_owned(),
                Target::Link(ronoctua_software::android::UNCERTIFIED.to_owned()),
                Spot::Deed("register"),
                Can::default(),
                None,
            );
        }
        // THE WINDOWS, a ring: `persist.waydroid.multi_windows`, whose own
        // default is off — one window for all of Android — and which is
        // read by the session when it starts, so the note says when a turn
        // takes effect.
        b.ring(
            key::SOFTWARE_ANDROID_WINDOWS,
            b.texts.t(if a.multi_window == Some(true) {
                key::SOFTWARE_ANDROID_ONE_PER_APP
            } else {
                key::SOFTWARE_ANDROID_ONE_FOR_ALL
            }),
            Ring::Windows,
            "windows",
        );
        b.note(key::SOFTWARE_NOTE_WINDOWS_NEXT_SESSION);
        b.heading(key::SOFTWARE_HEADING_ANDROID_APPS);
        let apps: Vec<&Installed> = s
            .installed
            .iter()
            .filter(|i| i.key.source() == Some(Source::Android))
            .collect();
        if apps.is_empty() {
            b.note(key::SOFTWARE_NOTE_NO_ANDROID_APPS);
        }
        for i in &apps {
            installed_row(b, i, r);
        }
        b.pair(
            b.texts.t(key::SOFTWARE_ANDROID_APK),
            String::new(),
            Target::Choose(Picking::Package),
            Spot::Deed("apk"),
            Can::default(),
            None,
        );
        let aurora = AppKey::Android {
            package: AURORA.to_owned(),
        };
        if !apps.iter().any(|i| i.key == aurora) {
            let name = b.texts.t(key::SOFTWARE_ANDROID_AURORA);
            b.deed(
                key::SOFTWARE_ANDROID_AURORA,
                b.texts.t(key::SOFTWARE_ANDROID_AURORA_VALUE),
                Deed::Install {
                    key: aurora,
                    name,
                    consent: Consent::default(),
                },
                None,
            );
        }
        b.danger(key::SOFTWARE_ANDROID_ERASE, Deed::EraseAndroid);
    }

    /// Sources, preferences and what is rarely touched.
    fn advanced(&self, b: &mut Builder<'_>, r: &Readings) {
        use crate::key;
        let p = &r.prefs;
        b.heading(key::SOFTWARE_HEADING_SOURCES);
        b.ring(
            key::SOFTWARE_ONLINE,
            on_off(p.online, b.texts),
            Ring::Online,
            "online",
        );
        for s in OnlineSource::ALL {
            let refused = (!p.online).then(|| b.texts.t(key::SOFTWARE_ONLINE_OFF));
            b.pair(
                b.texts.t(s.label()),
                on_off(s.is_on(p), b.texts),
                Target::Ring(Ring::Switch(s)),
                Spot::Ring(s.id()),
                Can::default(),
                refused,
            );
            // ON AND NOT LET: nothing is asked until its row is pressed
            // (`ronoctua_software::net::pass_for`), and the row says to whom.
            if p.online && s.is_on(p) && !s.is_let(&r.consents) {
                b.consent(s);
            }
        }
        let rev = r.snapshot.as_ref().and_then(|s| s.nix_index_rev.clone());
        b.fact_at(
            b.texts.t(key::SOFTWARE_NIX_INDEX),
            rev.unwrap_or_else(|| b.texts.t(words::refusal_key(Refusal::SourceAbsent))),
            Spot::Fact("nix-index"),
        );
        b.ring(
            key::SOFTWARE_PREFER,
            b.texts.t(words::prefer_key(p.prefer)),
            Ring::Prefer,
            "prefer",
        );
        b.ring(
            key::SOFTWARE_ASK_ABOVE,
            ask_above_said(p.ask_above_mb, b.texts),
            Ring::AskAbove,
            "ask-above",
        );

        b.heading(key::SOFTWARE_HEADING_TRIAL_DEFAULTS);
        b.ring(
            key::SOFTWARE_DEFAULT_TRACES,
            b.texts.t(words::traces_key(p.traces)),
            Ring::DefaultTraces,
            "default-traces",
        );
        b.ring(
            key::SOFTWARE_DEFAULT_NETWORK,
            on_off(p.trial_network, b.texts),
            Ring::DefaultNetwork,
            "default-network",
        );

        b.heading(key::SOFTWARE_HEADING_WEB_DEFAULTS);
        let profile = p.web_profile.as_ref().map_or_else(
            || b.texts.t(key::SOFTWARE_PROFILE_LAST_USED),
            |d| {
                r.profiles
                    .iter()
                    .find(|x| x.directory == *d)
                    .map_or(d.clone(), |x| x.name.clone())
            },
        );
        b.ring(
            key::SOFTWARE_WEB_PROFILE_DEFAULT,
            profile,
            Ring::WebProfile,
            "web-profile",
        );

        b.heading(key::SOFTWARE_HEADING_KEPT);
        b.door(
            key::SOFTWARE_CONSENTS,
            b.texts.t_args(
                key::SOFTWARE_CONSENTS_COUNT,
                &[("count", ronoctua_i18n::Arg::Number(count(p.unfree.len())))],
            ),
            Level::Consents,
        );
        b.door(
            key::SOFTWARE_HISTORY,
            b.texts.t(key::SOFTWARE_HISTORY_VALUE),
            Level::History,
        );

        // THE FLATPAKS' PERMISSIONS, one door per app.
        let flatpaks: Vec<&Installed> = r
            .snapshot
            .as_ref()
            .map(|s| {
                s.installed
                    .iter()
                    .filter(|i| matches!(i.key, AppKey::Flatpak { .. }))
                    .collect()
            })
            .unwrap_or_default();
        if !flatpaks.is_empty() {
            b.heading(key::SOFTWARE_HEADING_PERMISSIONS_LIST);
            for i in flatpaks {
                if let AppKey::Flatpak { id } = &i.key {
                    b.pair(
                        i.name.clone(),
                        b.texts.t(key::SOFTWARE_PERMISSIONS_VALUE),
                        Target::Door(Level::Permissions(id.clone())),
                        Spot::Permissions(id.clone()),
                        Can::default(),
                        None,
                    );
                }
            }
        }

        // DARK FLATPAKS: what the system does for each toolkit, as facts, and
        // the one thing that can undo it — an app carrying `GTK_THEME`, which
        // takes libadwaita's style away. Nothing here ever sets it.
        b.heading(key::SOFTWARE_HEADING_DARK);
        b.fact_at(
            b.texts.t(key::SOFTWARE_DARK_GTK4),
            b.texts.t(key::SOFTWARE_DARK_FOLLOWS),
            Spot::Fact("dark-gtk4"),
        );
        // THE TWO DECLARED PARTS, as the installations have them — read, and
        // not assumed from the flake: a person may have removed the
        // extension, and the page says what IS.
        let facts = match &r.dark {
            Some(Some(d)) => Some(d.facts),
            _ => None,
        };
        let t = b.texts;
        let asking = || t.t(key::SOFTWARE_ASKING);
        b.fact_at(
            t.t(key::SOFTWARE_DARK_GTK3),
            facts.map_or_else(asking, |f| {
                t.t(if f.extension {
                    key::SOFTWARE_DARK_GTK3_VALUE
                } else {
                    key::SOFTWARE_DARK_GTK3_MISSING
                })
            }),
            Spot::Fact("dark-gtk3"),
        );
        b.fact_at(
            t.t(key::SOFTWARE_DARK_QT),
            facts.map_or_else(asking, |f| {
                t.t(if f.qt_unset {
                    key::SOFTWARE_DARK_QT_VALUE
                } else {
                    key::SOFTWARE_DARK_QT_LEAKS
                })
            }),
            Spot::Fact("dark-qt"),
        );
        match r.dark.as_ref().map(|d| d.as_ref().map(|x| &x.forcing)) {
            Some(Some(forcing)) if forcing.is_empty() => {
                b.fact_at(
                    b.texts.t(key::SOFTWARE_DARK_FORCED),
                    b.texts.t(key::SOFTWARE_DARK_NONE_FORCED),
                    Spot::Fact("dark-forced"),
                );
            }
            Some(Some(forcing)) => {
                for (id, name) in forcing {
                    b.pair(
                        name.clone(),
                        b.texts.t(key::SOFTWARE_DARK_FORCES),
                        Target::Door(Level::Permissions(id.clone())),
                        Spot::Permissions(id.clone()),
                        Can::default(),
                        None,
                    );
                    if let Some(row) = b.out.last_mut() {
                        row.fields = crate::chrome::tinted(
                            std::mem::take(&mut row.fields),
                            Some(tokens::color::STATE_ERROR),
                            row.focused,
                        );
                    }
                }
                b.said(
                    b.texts.t(key::SOFTWARE_NOTE_GTK_THEME),
                    tokens::color::STATE_ERROR,
                );
            }
            _ => b.fact_at(
                b.texts.t(key::SOFTWARE_DARK_FORCED),
                b.texts.t(key::SOFTWARE_ASKING),
                Spot::Fact("dark-forced"),
            ),
        }
    }
}

// ─── rows shared by several levels ──────────────────────────────────────────

/// One installed program.
fn installed_row(b: &mut Builder<'_>, i: &Installed, r: &Readings) {
    let value = job_of(&r.jobs, &i.key)
        .filter(|j| j.stage.afoot())
        .map(|j| working_said(j, b.texts))
        .or_else(|| i.trouble.map(|why| b.texts.t(words::refusal_key(why))))
        .or_else(|| {
            i.update.as_ref().map(|v| {
                b.texts
                    .t_with(crate::key::SOFTWARE_UPDATE_TO, &[("version", v.as_str())])
            })
        })
        // WHAT THE FLAKE DECLARED SAYS SO, before a person reaches for `C-d`
        // — and so does what the PERSON declared, which `C-d` removes for
        // good and not from here alone.
        .or_else(|| match i.origin {
            Origin::Declared => Some(b.texts.t(words::origin_key(i.origin))),
            Origin::MineDeclared => Some(joined(
                &i.version,
                &b.texts.t(words::origin_key(i.origin)),
                b.texts,
            )),
            Origin::Mine | Origin::Distro { .. } => None,
        })
        .unwrap_or_else(|| i.version.clone());
    b.program(
        words::key_glyph(&i.key),
        i.icon.clone().map(Portrait::Program),
        i.name.clone(),
        value,
        Target::Program(i.key.clone()),
        Spot::Program(i.key.clone()),
        can_of_installed(i),
        &[],
    );
}

/// What the `C-` keys do on an installed program.
fn can_of_installed(i: &Installed) -> Can {
    Can {
        install: None,
        try_it: None,
        launch: i.desktop_ids.first().map(|id| Deed::Launch(id.clone())),
        update: i.update.as_ref().map(|_| Deed::Update {
            key: i.key.clone(),
            name: i.name.clone(),
        }),
        edit: match &i.key {
            AppKey::Web { slug } => Some(Deed::Edit(slug.clone())),
            _ => None,
        },
        remove: removal_of(i),
        stop: None,
    }
}

/// What `C-d` does to an installed program, by who put it there: removed
/// here; removed for good (here and from the declaration); kept
/// uninstalled (one of the distro's); nothing (what the distro needs, what
/// the flake installed).
fn removal_of(i: &Installed) -> Option<Deed> {
    let key = i.key.clone();
    let name = i.name.clone();
    match i.origin {
        Origin::Mine if !matches!(i.key, AppKey::System { .. }) => Some(Deed::Remove { key, name }),
        Origin::MineDeclared => Some(Deed::ForgetForever { key, name }),
        Origin::Mine | Origin::Distro { .. } | Origin::Declared => None,
    }
}

/// One trial.
fn trial_row(b: &mut Builder<'_>, t: &Trial, r: &Readings) {
    use crate::key;
    let value = if t.running {
        let minutes = t.since.map_or(0, |s| r.now.saturating_sub(s) / 60);
        b.texts.t_args(
            key::SOFTWARE_TRIAL_RUNNING,
            &[("minutes", ronoctua_i18n::Arg::Number(count_u64(minutes)))],
        )
    } else {
        b.texts.t(key::SOFTWARE_TRIAL_CLOSED)
    };
    let can = Can {
        install: Some(Deed::Keep {
            trial: t.key.clone(),
            name: t.name.clone(),
        }),
        launch: Some(Deed::Try {
            attr: t.attr.clone(),
            name: t.name.clone(),
            consent: Consent::default(),
        }),
        stop: t.running.then(|| Deed::Stop {
            trial: t.key.clone(),
            name: t.name.clone(),
        }),
        ..Can::default()
    };
    b.program(
        tokens::icon::SOFTWARE_TRIAL,
        None,
        t.name.clone(),
        value,
        Target::Trial(t.key.clone()),
        Spot::Trial(t.key.clone()),
        can,
        &[],
    );
}

/// One search result.
fn group_row(b: &mut Builder<'_>, g: &Group, r: &Readings) {
    let best = g.best();
    let installed = g
        .members
        .iter()
        .find(|m| m.badges.contains(&Badge::Installed));
    let value = job_of(&r.jobs, &best.key)
        .filter(|j| j.stage.afoot())
        .map(|j| working_said(j, b.texts))
        .unwrap_or_else(|| {
            let marks: String = best
                .badges
                .iter()
                .map(|x| words::badge_glyph(*x).to_string())
                .collect::<Vec<_>>()
                .join(" ");
            if marks.is_empty() {
                best.summary.clone()
            } else if best.summary.is_empty() {
                marks
            } else {
                format!("{marks}  {}", best.summary)
            }
        });
    let installed_one = installed.and_then(|m| {
        r.snapshot
            .as_ref()
            .and_then(|s| s.installed.iter().find(|i| i.key == m.key))
    });
    let mut can = installed_one.map(can_of_installed).unwrap_or_default();
    if installed.is_none() && best.badges.iter().all(|x| x.refuses().is_none()) {
        can.install = Some(Deed::DeclareForMe {
            key: best.key.clone(),
            name: g.name.clone(),
            consent: Consent::default(),
            machines: None,
        });
    }
    if installed.is_none()
        && let Some(nix) = g
            .members
            .iter()
            .find(|m| m.key.source() == Some(Source::Nix))
        && let AppKey::Nix { attr } = &nix.key
    {
        can.try_it = Some(Deed::Try {
            attr: attr.clone(),
            name: g.name.clone(),
            consent: Consent::default(),
        });
    }
    b.program(
        words::key_glyph(&best.key),
        best.icon.clone().map(Portrait::Program),
        g.name.clone(),
        value,
        Target::Group(g.id.clone()),
        Spot::Group(g.id.clone()),
        can,
        &g.lit,
    );
}

/// The sheet of something installed.
fn installed_sheet(b: &mut Builder<'_>, i: &Installed, r: &Readings) {
    use crate::key;
    let can = can_of_installed(i);
    match i.desktop_ids.first() {
        Some(id) => b.deed_at(
            key::SOFTWARE_ROW_LAUNCH,
            String::new(),
            Deed::Launch(id.clone()),
            Spot::Primary,
            None,
        ),
        None => b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_LAUNCH),
            b.texts.t(key::SOFTWARE_NO_WINDOW),
            Spot::Primary,
        ),
    }
    match &i.update {
        Some(version) => b.deed(
            key::SOFTWARE_ROW_UPDATE,
            b.texts
                .t_with(key::SOFTWARE_UPDATE_TO, &[("version", version.as_str())]),
            Deed::Update {
                key: i.key.clone(),
                name: i.name.clone(),
            },
            None,
        ),
        None => b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_UPDATE),
            b.texts.t(key::SOFTWARE_UP_TO_DATE),
            Spot::Fact("update"),
        ),
    }
    let source = i.key.source().map_or_else(
        || b.texts.t(key::SOFTWARE_SOURCE_SYSTEM),
        |s| b.texts.t(words::source_key(s)),
    );
    // WHAT IS LEAVING is no longer declared by anybody: its source alone,
    // and why it is still here is said below.
    let leaving = i.trouble == Some(Refusal::LeavesAtNextApply);
    b.fact_at(
        b.texts.t(key::SOFTWARE_ROW_SOURCE),
        if leaving {
            source
        } else {
            joined(&source, &b.texts.t(words::origin_key(i.origin)), b.texts)
        },
        Spot::Fact("source"),
    );
    if !i.version.is_empty() {
        b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_VERSION),
            i.version.clone(),
            Spot::Fact("version"),
        );
    }
    if let Some(bytes) = i.size_bytes {
        b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_SIZE),
            readable(bytes, b.texts),
            Spot::Fact("size"),
        );
    }
    if let AppKey::Flatpak { id } = &i.key {
        b.door(
            key::SOFTWARE_ROW_PERMISSIONS,
            b.texts.t(key::SOFTWARE_PERMISSIONS_VALUE),
            Level::Permissions(id.clone()),
        );
    }
    if let Some(edit) = can.edit {
        b.deed(key::SOFTWARE_ROW_EDIT, String::new(), edit, None);
    }
    // WHO PUT IT THERE decides how it leaves — and whether it stays.
    match i.origin {
        Origin::Mine => {
            // KEEP IT: declared, the update keeps it and the person's other
            // computers get it — the one click "make it permanent" was a
            // line to copy for.
            if i.key.source().is_some() {
                b.deed(
                    key::SOFTWARE_DECLARED_ROW_KEEP,
                    b.texts.t(key::SOFTWARE_DECLARED_KEEP_VALUE),
                    Deed::DeclareForMe {
                        key: i.key.clone(),
                        name: i.name.clone(),
                        consent: Consent::default(),
                        machines: None,
                    },
                    None,
                );
            }
            b.deed_at(
                key::SOFTWARE_ROW_REMOVE,
                String::new(),
                Deed::Remove {
                    key: i.key.clone(),
                    name: i.name.clone(),
                },
                Spot::Deed("remove"),
                None,
            );
        }
        Origin::MineDeclared => {
            if let Some(row) = declarations(r).and_then(|d| d.declared_as(&i.key)) {
                b.pair(
                    b.texts.t(key::SOFTWARE_DECLARED_ROW_DECLARED),
                    standing_said(row, b.texts),
                    Target::Door(Level::Declaration(row.entry.id().to_owned())),
                    Spot::Door("declaration"),
                    Can::default(),
                    None,
                );
            }
            forget_row(b, &i.key, &i.name);
        }
        Origin::Distro { .. } => {
            if let Some(d) = declarations(r).and_then(|d| d.distro_of(&i.key, &i.desktop_ids)) {
                distro_rows(b, r, &d.app.id);
            }
        }
        Origin::Declared => {
            b.deed_at(
                key::SOFTWARE_ROW_REMOVE,
                String::new(),
                Deed::Remove {
                    key: i.key.clone(),
                    name: i.name.clone(),
                },
                Spot::Deed("remove"),
                Some(b.texts.t(words::refusal_key(Refusal::DeclaredInTheFlake))),
            );
        }
    }
    if let Some(why) = i.trouble {
        b.said(
            b.texts.t(words::refusal_key(why)),
            tokens::color::STATE_ERROR,
        );
    }
}

/// "Remove for good", in the danger tone: here, from the declaration, and
/// from the person's other computers at their next update — after the
/// explanation.
fn forget_row(b: &mut Builder<'_>, key: &AppKey, name: &str) {
    use crate::key as k;
    b.deed_at(
        k::SOFTWARE_DECLARED_ROW_FORGET,
        b.texts.t(k::SOFTWARE_DECLARED_FORGET_VALUE),
        Deed::ForgetForever {
            key: key.clone(),
            name: name.to_owned(),
        },
        Spot::Deed("remove"),
        None,
    );
    if let Some(row) = b.out.last_mut()
        && !row.focused
    {
        row.fields = crate::chrome::tinted(
            std::mem::take(&mut row.fields),
            Some(tokens::color::STATE_ERROR),
            false,
        );
    }
}

// ─── the declaration ────────────────────────────────────────────────────────

/// The person's declaration and how this machine stands to it, once the
/// machine was read.
fn declarations(r: &Readings) -> Option<&Declarations> {
    r.snapshot.as_ref().map(|s| &s.declarations)
}

/// Who put a program there, as the reading says: a declared one is the
/// person's declaration even when it is not on this machine yet.
fn origin_of(r: &Readings, key: &AppKey) -> Origin {
    if declarations(r).is_some_and(|d| d.declared_as(key).is_some()) {
        return Origin::MineDeclared;
    }
    r.snapshot
        .as_ref()
        .and_then(|s| s.installed.iter().find(|i| i.key == *key))
        .map_or(Origin::Mine, |i| i.origin)
}

/// The installed program a declared entry is, when it is on this machine.
fn installed_of<'a>(r: &'a Readings, row: &DeclaredRow) -> Option<&'a Installed> {
    let key = ronoctua_software::declaration::key_of(&row.entry);
    r.snapshot
        .as_ref()
        .and_then(|s| s.installed.iter().find(|i| i.key == key))
}

/// A declared entry's name: the installed program's, else the entry's own.
fn entry_name(r: &Readings, row: &DeclaredRow) -> String {
    installed_of(r, row).map_or_else(
        || ronoctua_software::declaration::name_of(&row.entry),
        |i| i.name.clone(),
    )
}

/// How this machine stands to a declared entry, as a sentence — the hold's
/// reason after a held one's.
fn standing_said(row: &DeclaredRow, texts: &ronoctua_i18n::Texts) -> String {
    let said = texts.t(words::standing_key(&row.standing));
    match &row.standing {
        Standing::Skipped {
            reason: ronoctua_software::person::reports::SkipWhy::Held,
            detail,
        } => words::held_key(detail).map_or(said.clone(), |k| joined(&said, &texts.t(k), texts)),
        _ => said,
    }
}

/// Where a declaration says a program goes: every computer, or the named.
fn machines_said(machines: Option<&[String]>, texts: &ronoctua_i18n::Texts) -> String {
    match machines {
        None => texts.t(crate::key::SOFTWARE_DECLARED_ALL_COMPUTERS),
        Some(hosts) => hosts.join(", "),
    }
}

/// The door's value for the declared: how many, and how many this computer
/// left out.
fn declared_said(d: &Declarations, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    use ronoctua_i18n::Arg;
    if d.declared.is_empty() {
        return texts.t(key::SOFTWARE_DECLARED_NONE_YET);
    }
    let out = d
        .declared
        .iter()
        .filter(|r| matches!(r.standing, Standing::Skipped { .. }))
        .count();
    let all = texts.t_args(
        key::SOFTWARE_DECLARED_COUNT,
        &[("count", Arg::Number(count(d.declared.len())))],
    );
    if out == 0 {
        return all;
    }
    joined(
        &all,
        &texts.t_args(
            key::SOFTWARE_DECLARED_COUNT_OUT,
            &[("count", Arg::Number(count(out)))],
        ),
        texts,
    )
}

/// The door's value for the declined.
fn declined_said(d: &Declarations, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    use ronoctua_i18n::Arg;
    if d.declined.is_empty() {
        return texts.t(key::SOFTWARE_DECLARED_NONE_DECLINED_YET);
    }
    texts.t_args(
        key::SOFTWARE_DECLARED_DECLINED_COUNT,
        &[("count", Arg::Number(count(d.declined.len())))],
    )
}

/// What a decline did, as a sentence — with the distro's reason when it
/// needs the program.
fn decline_said(row: &DeclinedRow, texts: &ronoctua_i18n::Texts) -> String {
    let key = words::decline_key(&row.state);
    match &row.state {
        DeclineState::Essential { reason } => {
            joined(&texts.t(key), &words::essential_said(reason, texts), texts)
        }
        DeclineState::Elsewhere => texts.t_with(
            key,
            &[(
                "machines",
                machines_said(row.declining.machines.as_deref(), texts).as_str(),
            )],
        ),
        _ => texts.t(key),
    }
}

/// Why one of the distro's programs cannot be kept uninstalled.
fn essential_said(d: &DistroRow, texts: &ronoctua_i18n::Texts) -> String {
    d.app
        .essential
        .as_deref()
        .map_or_else(String::new, |r| words::essential_said(r, texts))
}

/// The latest job about one of the distro's programs (a decline, a
/// return).
fn distro_job<'a>(r: &'a Readings, id: &str) -> Option<&'a ronoctua_transfers::Job> {
    r.jobs
        .iter()
        .filter(|j| match &j.work {
            ronoctua_transfers::Work::Software { order, .. } => matches!(
                order.as_ref(),
                Order::Decline { id: x, .. } | Order::Undecline { id: x } if x == id
            ),
            _ => false,
        })
        .max_by_key(|j| j.id)
}

/// What the queue is doing to it, or what it said when it last failed.
fn job_rows(b: &mut Builder<'_>, job: Option<&ronoctua_transfers::Job>) {
    use crate::key;
    let Some(job) = job else { return };
    if job.stage.afoot() {
        b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_WORKING),
            working_said(job, b.texts),
            Spot::Fact("working"),
        );
    } else if job.stage == ronoctua_transfers::Stage::Failed {
        b.said(
            crate::programs_words::failure_said(job, b.texts),
            tokens::color::STATE_ERROR,
        );
    }
}

/// What the person declared, and how this machine stands to each.
fn declared_level(b: &mut Builder<'_>, r: &Readings) {
    use crate::key;
    b.heading(key::SOFTWARE_DECLARED_DOOR);
    b.note(key::SOFTWARE_DECLARED_NOTE);
    let Some(d) = declarations(r) else {
        b.note(key::SOFTWARE_READING);
        return;
    };
    unreadable_rows(b, d);
    if !d.reported {
        b.note(key::SOFTWARE_DECLARED_NOT_REPORTED);
    }
    if d.declared.is_empty() {
        let t = b.texts.t(key::SOFTWARE_DECLARED_NONE);
        b.said(t, tokens::color::TEXT_DIM);
    }
    for row in &d.declared {
        let key = ronoctua_software::declaration::key_of(&row.entry);
        let id = row.entry.id().to_owned();
        let name = entry_name(r, row);
        let picture = installed_of(r, row).and_then(|i| i.icon.clone());
        let value = job_of(&r.jobs, &key)
            .filter(|j| j.stage.afoot())
            .map_or_else(|| standing_said(row, b.texts), |j| working_said(j, b.texts));
        b.program(
            words::key_glyph(&key),
            picture.map(Portrait::Program),
            name.clone(),
            value,
            Target::Door(Level::Declaration(id.clone())),
            Spot::Declared(id),
            Can {
                remove: Some(Deed::ForgetForever { key, name }),
                ..Can::default()
            },
            &[],
        );
        // LEFT OUT IS A PROBLEM: the row says so in the danger tone.
        if matches!(row.standing, Standing::Skipped { .. })
            && let Some(last) = b.out.last_mut()
            && !last.focused
        {
            last.fields = crate::chrome::tinted(
                std::mem::take(&mut last.fields),
                Some(tokens::color::STATE_ERROR),
                false,
            );
        }
    }
    if !d.android_pending.is_empty() {
        b.note(key::SOFTWARE_DECLARED_ANDROID_PENDING);
    }
    if d.declared
        .iter()
        .any(|x| ronoctua_software::declaration::pin_of(&x.entry).is_some())
    {
        b.heading(key::SOFTWARE_DECLARED_HEADING_PINS);
        b.deed(
            key::SOFTWARE_DECLARED_REFRESH,
            b.texts.t(key::SOFTWARE_DECLARED_REFRESH_VALUE),
            Deed::RefreshPins,
            None,
        );
    }
}

/// The documents of the declaration that do not read here, said.
fn unreadable_rows(b: &mut Builder<'_>, d: &Declarations) {
    use crate::key;
    for u in &d.unreadable {
        let file = match u.file {
            ronoctua_software::declaration::File::Declared => key::SOFTWARE_DECLARED_FILE_DECLARED,
            ronoctua_software::declaration::File::Declined => key::SOFTWARE_DECLARED_FILE_DECLINED,
        };
        let why = if u.why == ronoctua_software::person::Why::Newer {
            key::SOFTWARE_DECLARED_UNREADABLE_NEWER
        } else {
            key::SOFTWARE_DECLARED_UNREADABLE_BROKEN
        };
        let said = b.texts.t_with(why, &[("file", b.texts.t(file).as_str())]);
        b.said(said, tokens::color::STATE_ERROR);
    }
}

/// One declared entry: what it is, where it goes, how this computer stands
/// to it, and what can be done.
fn declaration_sheet(b: &mut Builder<'_>, r: &Readings, id: &str) {
    use crate::key;
    use ronoctua_software::declaration as decl;
    let Some(d) = declarations(r) else {
        b.heading(key::SOFTWARE_DECLARED_DOOR);
        b.note(key::SOFTWARE_READING);
        return;
    };
    let Some(row) = d.declared.iter().find(|x| x.entry.id() == id) else {
        // FORGOTTEN MEANWHILE, here or by another computer.
        b.heading(key::SOFTWARE_DECLARED_DOOR);
        b.said(
            b.texts.t(key::SOFTWARE_DECLARED_GONE),
            tokens::color::TEXT_DIM,
        );
        return;
    };
    let app = decl::key_of(&row.entry);
    let name = entry_name(r, row);
    let installed = installed_of(r, row);
    b.title_with(
        words::key_glyph(&app),
        installed
            .and_then(|i| i.icon.clone())
            .map(Portrait::Program),
        name.clone(),
    );
    job_rows(b, job_of(&r.jobs, &app));
    // WHERE IT GOES, AND HOW THIS COMPUTER STANDS — the standing, and on a
    // row of its own WHY: a value column cuts what does not fit, and the
    // reason is the part the person came to read.
    b.fact_at(
        b.texts.t(key::SOFTWARE_DECLARED_ROW_HERE_NOW),
        b.texts.t(words::standing_key(&row.standing)),
        Spot::Fact("standing"),
    );
    if let Standing::Skipped { reason, detail } = &row.standing {
        let held = (*reason == ronoctua_software::person::reports::SkipWhy::Held)
            .then(|| words::held_key(detail))
            .flatten();
        match held {
            Some(k) => b.fact_at(
                b.texts.t(key::SOFTWARE_DECLARED_ROW_WHY),
                b.texts.t(k),
                Spot::Fact("why"),
            ),
            // WHAT NIX SAID (data), when it said something.
            None if !detail.trim().is_empty() => b.fact_at(
                b.texts.t(key::SOFTWARE_DECLARED_ROW_SAID),
                detail.clone(),
                Spot::Fact("said"),
            ),
            None => {}
        }
    }
    b.fact_at(
        b.texts.t(key::SOFTWARE_DECLARED_ROW_ON),
        machines_said(row.entry.machines(), b.texts),
        Spot::Fact("machines"),
    );
    b.fact_at(
        b.texts.t(key::SOFTWARE_ROW_SOURCE),
        b.texts.t(words::source_key(decl::source_of(&row.entry))),
        Spot::Fact("source"),
    );
    b.fact_at(
        b.texts.t(key::SOFTWARE_ROW_REFERENCE),
        row.entry.source_id().to_owned(),
        Spot::Fact("reference"),
    );
    if let Some((version, url, pinned)) = decl::pin_of(&row.entry) {
        b.fact_at(
            b.texts.t(key::SOFTWARE_DECLARED_ROW_PIN),
            match pinned {
                Some(at) => joined(
                    &version,
                    &crate::chrome::when_text(at.unix_s(), &r.zone, false, b.texts),
                    b.texts,
                ),
                None => version,
            },
            Spot::Fact("pin"),
        );
        b.fact_at(
            b.texts.t(key::SOFTWARE_DECLARED_ROW_DOWNLOAD),
            url,
            Spot::Fact("download"),
        );
    }
    b.fact_at(
        b.texts.t(key::SOFTWARE_DECLARED_ROW_EDITED),
        b.texts.t_with(
            key::SOFTWARE_DECLARED_EDITED_ON,
            &[
                (
                    "when",
                    crate::chrome::when_text(
                        row.entry.edited_at().unix_s(),
                        &r.zone,
                        false,
                        b.texts,
                    )
                    .as_str(),
                ),
                ("machine", row.entry.edited_on()),
            ],
        ),
        Spot::Fact("edited"),
    );
    // WHAT CAN BE DONE, by why it stands as it does.
    b.heading(key::SOFTWARE_DECLARED_HEADING_FIXES);
    match &row.standing {
        Standing::Applied => {
            if installed.is_some() {
                b.pair(
                    b.texts.t(key::SOFTWARE_DECLARED_ROW_PROGRAM),
                    b.texts.t(key::SOFTWARE_DECLARED_PROGRAM_VALUE),
                    Target::Program(app.clone()),
                    Spot::Door("program"),
                    Can::default(),
                    None,
                );
            }
        }
        Standing::Waiting => b.note(key::SOFTWARE_DECLARED_NOTE_WAITING),
        Standing::Elsewhere => {
            let mut machines: Vec<String> = row
                .entry
                .machines()
                .map(<[String]>::to_vec)
                .unwrap_or_default();
            if !d.host.is_empty() && !machines.contains(&d.host) {
                machines.push(d.host.clone());
            }
            b.deed(
                key::SOFTWARE_DECLARED_ROW_ALSO_HERE,
                String::new(),
                Deed::DeclareForMe {
                    key: app.clone(),
                    name: name.clone(),
                    consent: Consent::default(),
                    machines: Some(machines),
                },
                None,
            );
        }
        Standing::Skipped { reason, detail } => {
            use ronoctua_software::person::reports::SkipWhy;
            let refresh = decl::pin_of(&row.entry).is_some()
                && (*reason == SkipWhy::Held
                    && matches!(detail.as_str(), "fetch-failed" | "upstream-changed"));
            if refresh {
                b.deed(
                    key::SOFTWARE_DECLARED_REFRESH_ONE,
                    b.texts.t(key::SOFTWARE_DECLARED_REFRESH_ONE_VALUE),
                    Deed::RefreshPins,
                    None,
                );
            }
            match reason {
                SkipWhy::SourceOff => b.note(key::SOFTWARE_DECLARED_NOTE_SOURCE_OFF),
                SkipWhy::FormatUnknown => b.note(key::SOFTWARE_DECLARED_NOTE_UPDATE_ME),
                SkipWhy::OtherMachine => {}
                SkipWhy::AttrMissing
                | SkipWhy::NotAPackage
                | SkipWhy::RefusedByMeta
                | SkipWhy::Held => b.deed(
                    key::SOFTWARE_DECLARED_ROW_REPLACE,
                    b.texts.t(key::SOFTWARE_DECLARED_REPLACE_VALUE),
                    Deed::FindReplacement(name.clone()),
                    None,
                ),
            }
        }
    }
    forget_row(b, &app, &name);
}

/// What the person declined, and the rest of the distro's programs.
fn declined_level(b: &mut Builder<'_>, r: &Readings) {
    use crate::key;
    b.heading(key::SOFTWARE_DECLARED_DOOR_DECLINED);
    b.note(key::SOFTWARE_DECLARED_NOTE_DECLINED);
    let Some(d) = declarations(r) else {
        b.note(key::SOFTWARE_READING);
        return;
    };
    unreadable_rows(b, d);
    if d.declined.is_empty() {
        let t = b.texts.t(key::SOFTWARE_DECLARED_NONE_DECLINED);
        b.said(t, tokens::color::TEXT_DIM);
    }
    for row in &d.declined {
        let id = row.declining.id.clone();
        let value = distro_job(r, &id)
            .filter(|j| j.stage.afoot())
            .map_or_else(|| decline_said(row, b.texts), |j| working_said(j, b.texts));
        b.program(
            tokens::icon::SOFTWARE_SOURCE_SYSTEM,
            None,
            row.name.clone(),
            value,
            Target::Door(Level::Distro(id.clone())),
            Spot::Declined(id),
            Can::default(),
            &[],
        );
        // STILL HERE THROUGH ANOTHER PROGRAM: the decline did not do what it
        // says, and the row is in the danger tone.
        if row.state == DeclineState::StillPresent
            && let Some(last) = b.out.last_mut()
            && !last.focused
        {
            last.fields = crate::chrome::tinted(
                std::mem::take(&mut last.fields),
                Some(tokens::color::STATE_ERROR),
                false,
            );
        }
    }
    // THE REST OF THE DISTRO'S PROGRAMS on this computer: what can be kept
    // uninstalled, and what the distro needs.
    let rest: Vec<&DistroRow> = d
        .distro
        .iter()
        .filter(|x| x.app.on_this_machine && d.decline_of(&x.app.id).is_none())
        .collect();
    if rest.is_empty() {
        return;
    }
    b.heading(key::SOFTWARE_DECLARED_HEADING_DISTRO);
    for x in rest {
        let declinable = x.app.essential.is_none();
        let value = if declinable {
            b.texts.t(key::SOFTWARE_DECLARED_DISTRO_INSTALLED)
        } else {
            essential_said(x, b.texts)
        };
        b.program(
            tokens::icon::SOFTWARE_SOURCE_SYSTEM,
            None,
            x.name.clone(),
            value,
            Target::Door(Level::Distro(x.app.id.clone())),
            Spot::Distro(x.app.id.clone()),
            Can {
                remove: declinable.then(|| Deed::KeepUninstalled {
                    id: x.app.id.clone(),
                    name: x.name.clone(),
                    everywhere: false,
                }),
                ..Can::default()
            },
            &[],
        );
    }
}

/// One of the distro's programs — or the decline of one the distro no
/// longer carries: what it is, why the distro needs it, and keeping it
/// uninstalled (here or on every computer) or bringing it back.
///
/// A decline is always reachable here, whatever came of it: one the system
/// refused because the distro needs the program, one of a program the
/// distro stopped carrying, one of other computers only (and then keeping
/// it uninstalled HERE too is offered, as the launcher's removal asks).
fn distro_rows(b: &mut Builder<'_>, r: &Readings, id: &str) {
    use crate::key;
    let decls = declarations(r);
    let host = decls.map_or("", |x| x.host.as_str());
    let d = decls.and_then(|x| x.distro(id));
    let declined = decls.and_then(|x| x.decline_of(id));
    if let Some(d) = d {
        b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_SOURCE),
            b.texts.t(words::origin_key(Origin::Distro {
                declinable: d.app.essential.is_none(),
            })),
            Spot::Fact("source"),
        );
        b.fact_at(
            b.texts.t(key::SOFTWARE_DECLARED_ROW_GROUP),
            d.app.group.clone(),
            Spot::Fact("group"),
        );
        if !d.app.on_this_machine {
            // LEFT OUT BY THE SYSTEM'S DECLINE, and no longer declined here:
            // the next apply brings it back.
            let returning =
                d.app.declined_here && declined.is_none_or(|row| !row.declining.applies_to(host));
            b.note(if returning {
                key::SOFTWARE_DECLARED_RETURNING
            } else {
                key::SOFTWARE_DECLARED_NOT_ON_THIS_MACHINE
            });
        }
        if d.app.essential.is_some() {
            let said = essential_said(d, b.texts);
            b.said(said, tokens::color::TEXT_DIM);
        }
    }
    job_rows(b, distro_job(r, id));
    let declinable = d.filter(|d| d.app.essential.is_none());
    match declined {
        Some(row) => {
            // THE DISTRO'S REASON is said once: on its own line above, when
            // the program is one the distro needs.
            let said = if d.is_some_and(|d| d.app.essential.is_some()) {
                b.texts.t(words::decline_key(&row.state))
            } else {
                decline_said(row, b.texts)
            };
            b.fact_at(
                b.texts.t(key::SOFTWARE_DECLARED_ROW_DECLINED),
                said,
                Spot::Fact("declined"),
            );
            b.deed(
                key::SOFTWARE_DECLARED_ROW_BRING_BACK,
                b.texts.t(key::SOFTWARE_DECLARED_BRING_BACK_VALUE),
                Deed::BringBack {
                    id: id.to_owned(),
                    name: d.map_or_else(|| row.name.clone(), |d| d.name.clone()),
                },
                None,
            );
            if let Some(d) = declinable {
                if !row.declining.applies_to(host) {
                    keep_uninstalled(b, d, false);
                }
                if row.declining.machines.is_some() {
                    keep_uninstalled(b, d, true);
                }
            }
        }
        None => {
            if let Some(d) = declinable {
                keep_uninstalled(b, d, false);
                keep_uninstalled(b, d, true);
            }
        }
    }
}

/// "Keep uninstalled here", or "on all your computers".
fn keep_uninstalled(b: &mut Builder<'_>, d: &DistroRow, everywhere: bool) {
    use crate::key;
    b.deed(
        if everywhere {
            key::SOFTWARE_DECLARED_ROW_DECLINE_EVERYWHERE
        } else {
            key::SOFTWARE_DECLARED_ROW_DECLINE_HERE
        },
        String::new(),
        Deed::KeepUninstalled {
            id: d.app.id.clone(),
            name: d.name.clone(),
            everywhere,
        },
        None,
    );
}

/// What a sheet says about the program: its summary, its badges in words, its
/// description, and the drawer of details — from the search's row where
/// there is one, and from the details otherwise (a sheet opened from the
/// launcher, or of something installed, has no search row).
fn about(
    b: &mut Builder<'_>,
    found: Option<&Found>,
    details: Option<Result<&Details, &Refusal>>,
    app: &AppKey,
    installed: Option<&Installed>,
) {
    use crate::key;
    let d = details.and_then(Result::ok);
    let summary = found
        .map(|f| f.summary.clone())
        .filter(|x| !x.is_empty())
        .or_else(|| d.map(|d| d.summary.clone()))
        .unwrap_or_default();
    let paragraphs: Vec<String> = d
        .map(|d| {
            d.description
                .iter()
                .filter(|p| **p != summary)
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let badges: Vec<Badge> = found.map_or_else(
        || d.map(|d| d.badges.clone()).unwrap_or_default(),
        |f| f.badges.clone(),
    );
    if !summary.is_empty() || !paragraphs.is_empty() || !badges.is_empty() {
        b.heading(key::SOFTWARE_HEADING_ABOUT);
        if !summary.is_empty() {
            b.said(summary, tokens::color::TEXT_SECONDARY);
        }
        // EACH BADGE IN WORDS: a glyph alone is only readable by whoever
        // chose it. A warning is in the danger tone — the glyph stays.
        for x in badges {
            let colour = if words::badge_warns(x) {
                tokens::color::STATE_ERROR
            } else {
                tokens::color::TEXT_DIM
            };
            b.said(
                format!(
                    "{} {}",
                    words::badge_glyph(x),
                    b.texts.t(words::badge_key(x))
                ),
                colour,
            );
        }
        for p in paragraphs {
            b.said(p, tokens::color::TEXT_DIM);
        }
    }
    if let Some(Err(why)) = details {
        b.said(b.texts.t(words::refusal_key(*why)), tokens::color::TEXT_DIM);
    }
    let open = b.is_open("details");
    b.drawer(key::SOFTWARE_HEADING_DETAILS, "details", open);
    if !open {
        return;
    }
    let reference = d
        .map(|d| d.reference.clone())
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| app.id().to_owned());
    b.fact_at(
        b.texts.t(key::SOFTWARE_ROW_REFERENCE),
        reference,
        Spot::Fact("reference"),
    );
    let publisher = found
        .and_then(|f| f.publisher.clone())
        .or_else(|| d.and_then(|d| d.publisher.clone()));
    if let Some(p) = publisher {
        b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_PUBLISHER),
            p,
            Spot::Fact("publisher"),
        );
    }
    let license = found
        .and_then(|f| f.license.clone())
        .or_else(|| d.and_then(|d| d.license.clone()));
    if let Some(l) = license {
        b.fact_at(
            b.texts.t(key::SOFTWARE_ROW_LICENCE),
            l,
            Spot::Fact("licence"),
        );
    }
    let homepage = found
        .and_then(|f| f.homepage.clone())
        .or_else(|| d.and_then(|d| d.homepage.clone()));
    if let Some(h) = homepage {
        b.pair(
            b.texts.t(key::SOFTWARE_ROW_SITE),
            h.clone(),
            Target::Link(h),
            Spot::Fact("site"),
            Can::default(),
            None,
        );
    }
    if let Some(d) = d {
        if let Some(needs) = &d.needs {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_NEEDS),
                needs.clone(),
                Spot::Fact("needs"),
            );
        }
        if let Some(bytes) = d
            .installed_bytes
            .or_else(|| installed.and_then(|i| i.size_bytes))
        {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_INSTALLED_SIZE),
                readable(bytes, b.texts),
                Spot::Fact("installed-size"),
            );
        }
        for (n, line) in d.permissions.iter().enumerate() {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_PERMISSION),
                line.clone(),
                Spot::Permission(n),
            );
        }
        if !d.anti_features.is_empty() {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_ANTI_FEATURES),
                d.anti_features.join(", "),
                Spot::Fact("anti-features"),
            );
        }
    }
}

/// Updates, space and going back.
fn maintenance(b: &mut Builder<'_>, r: &Readings) {
    use crate::key;
    b.heading(key::SOFTWARE_HEADING_UPDATES);
    let Some(s) = &r.snapshot else {
        b.note(key::SOFTWARE_READING);
        return;
    };
    let total = s.installed.iter().filter(|i| i.update.is_some()).count();
    b.deed_at(
        key::SOFTWARE_UPDATES_ALL,
        updates_said(total, b.texts),
        Deed::UpdateEverything,
        Spot::Primary,
        None,
    );
    for source in Source::ALL.into_iter().filter(|x| s.supports.has(*x)) {
        let name = b.texts.t(words::source_key(source));
        if source == Source::Web {
            b.fact_at(
                name,
                b.texts.t(key::SOFTWARE_WEB_ALWAYS_LATEST),
                Spot::Fact("web-updates"),
            );
            continue;
        }
        let n = s
            .installed
            .iter()
            .filter(|i| i.key.source() == Some(source) && i.update.is_some())
            .count();
        let mut value = updates_said(n, b.texts);
        if source == Source::Nix
            && let Some(rev) = &s.nix_index_rev
        {
            value = joined(
                &value,
                &b.texts
                    .t_with(key::SOFTWARE_FOLLOWS_SYSTEM, &[("rev", rev.as_str())]),
                b.texts,
            );
        }
        b.pair(
            name,
            value,
            Target::Deed(Deed::UpdateSource(source)),
            Spot::Status(source),
            Can::default(),
            None,
        );
    }
    if s.supports.flatpak {
        b.deed(
            key::SOFTWARE_LOOK_AGAIN,
            String::new(),
            Deed::Refresh(Source::Flatpak),
            None,
        );
    }
    b.pair(
        b.texts.t(key::SOFTWARE_UPDATES_AUTO),
        b.texts.t_with(
            key::SOFTWARE_AUTO_WITH,
            &[("keys", r.taught.brain.as_str())],
        ),
        Target::Deed(Deed::Run("brain-open")),
        Spot::Deed("brain"),
        Can::default(),
        None,
    );

    b.heading(key::SOFTWARE_HEADING_SPACE);
    b.deed_at(
        key::SOFTWARE_CLEAN,
        String::new(),
        Deed::Tidy(None),
        Spot::Deed("tidy-all"),
        None,
    );
    if s.supports.nix {
        b.deed_at(
            key::SOFTWARE_CLEAN_GENERATIONS,
            String::new(),
            Deed::Tidy(Some(Source::Nix)),
            Spot::Deed("tidy-nix"),
            None,
        );
    }
    if s.supports.flatpak {
        b.deed_at(
            key::SOFTWARE_CLEAN_FLATPAK,
            String::new(),
            Deed::Tidy(Some(Source::Flatpak)),
            Spot::Deed("tidy-flatpak"),
            None,
        );
    }
    if s.supports.appimage {
        b.deed_at(
            key::SOFTWARE_CLEAN_APPIMAGE,
            String::new(),
            Deed::Tidy(Some(Source::AppImage)),
            Spot::Deed("tidy-appimage"),
            None,
        );
    }
    b.fact_at(
        b.texts.t(key::SOFTWARE_CLEAN_TRIALS),
        b.texts.t(key::SOFTWARE_CLEAN_TRIALS_VALUE),
        Spot::Fact("tidy-trials"),
    );

    if !s.supports.nix {
        return;
    }
    b.heading(key::SOFTWARE_HEADING_GENERATIONS);
    if s.generations.is_empty() {
        b.note(key::SOFTWARE_NOTE_NO_GENERATIONS);
        return;
    }
    let (shown, older) = s
        .generations
        .split_at(s.generations.len().min(GENERATIONS_SHOWN));
    for g in shown {
        generation_row(b, g, r);
    }
    if older.is_empty() {
        return;
    }
    let open = b.is_open("older");
    b.drawer(key::SOFTWARE_OLDER_VERSIONS, "older", open);
    if open {
        for g in older {
            generation_row(b, g, r);
        }
    }
}

/// One version of the Nix list.
fn generation_row(b: &mut Builder<'_>, g: &ronoctua_software::state::Generation, r: &Readings) {
    use crate::key;
    let label = b.texts.t_with(
        key::SOFTWARE_GENERATION,
        &[("n", g.number.to_string().as_str())],
    );
    let when = crate::chrome::when_text(
        i64::try_from(g.when).unwrap_or(i64::MAX),
        &r.zone,
        false,
        b.texts,
    );
    let value = if g.summary.is_empty() {
        when
    } else {
        joined(&when, &g.summary, b.texts)
    };
    if g.current {
        b.fact_at(
            label,
            joined(&value, &b.texts.t(key::SOFTWARE_IN_USE), b.texts),
            Spot::Generation(g.number),
        );
    } else {
        b.pair(
            label,
            value,
            Target::Deed(Deed::Rollback(g.number)),
            Spot::Generation(g.number),
            Can::default(),
            None,
        );
    }
}

/// A Flatpak's name, as the machine has it, else its id.
fn flatpak_name(r: &Readings, id: &str) -> String {
    r.snapshot
        .as_ref()
        .and_then(|s| {
            s.installed
                .iter()
                .find(|i| i.key == AppKey::Flatpak { id: id.to_owned() })
        })
        .map_or_else(|| id.to_owned(), |i| i.name.clone())
}

/// One Flatpak's permissions: a ring per thing it may reach, what cannot be
/// turned back behind a question, and the lines themselves in a drawer.
fn permissions(b: &mut Builder<'_>, r: &Readings, id: &str) {
    use crate::key;
    let name = flatpak_name(r, id);
    b.title(
        b.texts
            .t_with(key::SOFTWARE_HEADING_PERMISSIONS, &[("app", name.as_str())]),
    );
    let found = match &r.permissions {
        Some((asked, Some(Some(found)))) if asked == id => found,
        Some((asked, Some(None))) if asked == id => {
            b.said(
                b.texts.t(words::refusal_key(Refusal::ToolFailed)),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        _ => {
            b.note(key::SOFTWARE_READING);
            return;
        }
    };
    let g = found.grants;
    for ring in GrantRing::ALL {
        let value = match ring {
            GrantRing::Files => b.texts.t(words::reach_key(g.files)),
            GrantRing::Network => on_off(g.network, b.texts),
            GrantRing::Sound => on_off(g.sound, b.texts),
            GrantRing::Devices => on_off(g.devices, b.texts),
            // A PROGRAM WITH NO WINDOW has no display to choose, and a ring
            // that turned nothing would be a row that lies.
            GrantRing::Display => match g.display {
                Some(d) => b.texts.t(words::display_key(d)),
                None => continue,
            },
        };
        let grant = match ring {
            GrantRing::Files => Grant::Files { reach: g.files },
            GrantRing::Network => Grant::Network { on: g.network },
            GrantRing::Sound => Grant::Sound { on: g.sound },
            GrantRing::Devices => Grant::Devices { on: g.devices },
            GrantRing::Display => Grant::Display {
                through: g.display.unwrap_or_default(),
            },
        };
        b.ring(words::grant_key(grant), value, Ring::Grant(ring), ring.id());
    }
    // `GTK_THEME` CAN ONLY BE TAKEN AWAY — nothing here can put it in — and
    // the row that does it is drawn only where it is forced, in the danger
    // tone, because that is the one setting that costs an app its own style.
    if g.gtk_theme {
        b.deed(
            words::grant_key(Grant::UnsetGtkTheme),
            b.texts.t(key::SOFTWARE_GTK_THEME_FORCED),
            Deed::Permission {
                id: id.to_owned(),
                name: name.clone(),
                grant: Grant::UnsetGtkTheme,
            },
            None,
        );
        if let Some(row) = b.out.last_mut() {
            row.fields = crate::chrome::tinted(
                std::mem::take(&mut row.fields),
                Some(tokens::color::STATE_ERROR),
                row.focused,
            );
        }
        b.said(
            b.texts.t(key::SOFTWARE_NOTE_GTK_THEME),
            tokens::color::STATE_ERROR,
        );
    }
    b.deed(
        words::grant_key(Grant::Reset),
        String::new(),
        Deed::Permission {
            id: id.to_owned(),
            name,
            grant: Grant::Reset,
        },
        None,
    );
    b.note(key::SOFTWARE_NOTE_PERMISSIONS_RESTART);
    // THE LINES THEMSELVES, for whoever wants to read what the rings come
    // from — a drawer, because the rings are the page.
    let open = b.is_open("asks");
    b.drawer(key::SOFTWARE_HEADING_ASKS, "asks", open);
    if open {
        if found.lines.is_empty() {
            b.note(key::SOFTWARE_NOTE_NO_PERMISSIONS);
        }
        for (n, line) in found.lines.iter().enumerate() {
            b.fact_at(
                b.texts.t(key::SOFTWARE_ROW_PERMISSION),
                line.clone(),
                Spot::Permission(n),
            );
        }
    }
}

/// The non-free packages allowed.
fn consents(b: &mut Builder<'_>, r: &Readings) {
    use crate::key;
    b.heading(key::SOFTWARE_HEADING_CONSENTS);
    b.note(key::SOFTWARE_NOTE_CONSENTS);
    if r.prefs.unfree.is_empty() {
        b.note(key::SOFTWARE_NOTE_NO_CONSENTS);
    }
    for attr in &r.prefs.unfree {
        b.pair(
            attr.clone(),
            b.texts.t(key::SOFTWARE_ALLOWED),
            Target::Nothing,
            Spot::Consent(attr.clone()),
            Can {
                remove: Some(Deed::Revoke(attr.clone())),
                ..Can::default()
            },
            None,
        );
    }
}

/// What was done, newest first.
fn history(b: &mut Builder<'_>, r: &Readings) {
    use crate::key;
    b.heading(key::SOFTWARE_HEADING_HISTORY);
    if r.history.is_empty() {
        b.note(key::SOFTWARE_NOTE_NO_HISTORY);
    }
    for (n, e) in r.history.iter().rev().enumerate() {
        let did =
            words::order_key_of_word(&e.order).map_or_else(|| e.order.clone(), |k| b.texts.t(k));
        let when = crate::chrome::when_text(
            i64::try_from(e.when).unwrap_or(i64::MAX),
            &r.zone,
            false,
            b.texts,
        );
        let mut value = joined(&when, &did, b.texts);
        if !e.ok {
            let why = e.refusal.map_or_else(
                || b.texts.t(key::SOFTWARE_FAILED),
                |x| b.texts.t(words::refusal_key(x)),
            );
            value = joined(&value, &why, b.texts);
        }
        let label = if e.name.is_empty() {
            e.key
                .as_ref()
                .map(|k| k.id().to_owned())
                .unwrap_or_default()
        } else {
            e.name.clone()
        };
        b.pair(
            label,
            value,
            Target::Entry(e.key.clone()),
            Spot::Entry(n),
            Can::default(),
            None,
        );
    }
}

/// What a question explains before its answers, line by line: the
/// declaration's two say what happens beyond this computer.
fn explanation_of(q: &Question) -> Vec<&'static str> {
    use crate::key;
    match q {
        Question::Forget { .. } => vec![
            key::SOFTWARE_DECLARED_FORGET_HERE,
            key::SOFTWARE_DECLARED_FORGET_ELSEWHERE,
            key::SOFTWARE_DECLARED_FORGET_DATA,
        ],
        Question::Decline {
            everywhere: true, ..
        } => vec![
            key::SOFTWARE_DECLARED_DECLINE_REBUILD,
            key::SOFTWARE_DECLARED_DECLINE_OTHERS,
            key::SOFTWARE_DECLARED_DECLINE_BACK,
        ],
        Question::Decline { .. } => vec![
            key::SOFTWARE_DECLARED_DECLINE_REBUILD,
            key::SOFTWARE_DECLARED_DECLINE_BACK,
        ],
        _ => Vec::new(),
    }
}

/// A question, in place of the rows: the sentence, then "no", then the yeses.
fn question(b: &mut Builder<'_>, q: &Question) {
    use crate::key;
    let t = b.texts;
    let (sentence, yes, more): (String, &str, Option<&str>) = match q {
        Question::Consent {
            kind, name, bytes, ..
        } => match kind {
            ConsentKind::Unfree => (
                t.t_with(key::SOFTWARE_SURE_UNFREE, &[("app", name.as_str())]),
                key::SOFTWARE_YES_UNFREE,
                None,
            ),
            ConsentKind::Unverified => (
                t.t_with(key::SOFTWARE_SURE_UNVERIFIED, &[("app", name.as_str())]),
                key::SOFTWARE_YES_TRUST,
                None,
            ),
            ConsentKind::Big => (
                t.t_with(
                    key::SOFTWARE_SURE_DOWNLOAD,
                    &[
                        ("app", name.as_str()),
                        (
                            "size",
                            bytes.map(|x| readable(x, t)).unwrap_or_default().as_str(),
                        ),
                    ],
                ),
                key::SOFTWARE_YES_DOWNLOAD,
                None,
            ),
            ConsentKind::ArmOnly => (
                t.t_with(key::SOFTWARE_SURE_ARM, &[("app", name.as_str())]),
                key::SOFTWARE_YES_ARM,
                None,
            ),
        },
        Question::Remove { name, .. } => (
            t.t_with(key::SOFTWARE_SURE_REMOVE, &[("app", name.as_str())]),
            key::SOFTWARE_YES_REMOVE,
            Some(key::SOFTWARE_YES_REMOVE_DATA),
        ),
        Question::RemoveWeb { name, .. } => (
            t.t_with(key::SOFTWARE_SURE_WEB_REMOVE, &[("app", name.as_str())]),
            key::SOFTWARE_YES_REMOVE_WEB,
            None,
        ),
        Question::Rollback(n) => (
            t.t_with(
                key::SOFTWARE_SURE_ROLLBACK,
                &[("n", n.to_string().as_str())],
            ),
            key::SOFTWARE_YES_ROLLBACK,
            None,
        ),
        Question::CleanUp(_) => (t.t(key::SOFTWARE_SURE_CLEAN), key::SOFTWARE_YES_CLEAN, None),
        Question::PrepareAndroid(image) => (
            t.t_with(
                key::SOFTWARE_SURE_ANDROID,
                &[("image", t.t(words::image_key(*image)).as_str())],
            ),
            key::SOFTWARE_YES_ANDROID,
            None,
        ),
        Question::EraseAndroid => (
            t.t(key::SOFTWARE_SURE_ERASE_ANDROID),
            key::SOFTWARE_YES_ERASE,
            None,
        ),
        Question::Foreign(path) => (
            t.t_with(
                key::SOFTWARE_SURE_FOREIGN,
                &[(
                    "file",
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                        .as_str(),
                )],
            ),
            key::SOFTWARE_YES_TRUST,
            None,
        ),
        Question::LeaveDraft => (t.t(key::SOFTWARE_SURE_LEAVE), key::SOFTWARE_YES_LEAVE, None),
        Question::Permission { name, grant, .. } => {
            let (sure, yes) = if *grant == Grant::UnsetGtkTheme {
                (
                    key::SOFTWARE_SURE_UNSET_GTK_THEME,
                    key::SOFTWARE_YES_UNSET_GTK_THEME,
                )
            } else {
                (key::SOFTWARE_SURE_RESET, key::SOFTWARE_YES_RESET)
            };
            (t.t_with(sure, &[("app", name.as_str())]), yes, None)
        }
        Question::Forget { name, .. } => (
            t.t_with(
                key::SOFTWARE_DECLARED_SURE_FORGET,
                &[("app", name.as_str())],
            ),
            key::SOFTWARE_DECLARED_YES_FORGET,
            Some(key::SOFTWARE_YES_REMOVE_DATA),
        ),
        Question::Decline {
            name, everywhere, ..
        } => (
            t.t_with(
                if *everywhere {
                    key::SOFTWARE_DECLARED_SURE_DECLINE_EVERYWHERE
                } else {
                    key::SOFTWARE_DECLARED_SURE_DECLINE_HERE
                },
                &[("app", name.as_str())],
            ),
            key::SOFTWARE_DECLARED_YES_DECLINE,
            None,
        ),
    };
    for line in crate::chrome::note_lines(&sentence, b.room) {
        b.push_heading(line, tokens::color::TEXT_HEADING);
    }
    // THE EXPLANATION, before the answers: what the yes does beyond this
    // computer, and what it keeps.
    for k in explanation_of(q) {
        for line in crate::chrome::note_lines(&t.t(k), b.room) {
            b.push_heading(line, tokens::color::TEXT_DIM);
        }
    }
    // NO FIRST, AND THE CURSOR IS BORN ON IT.
    b.answer(t.t(key::SOFTWARE_NO), String::new(), Answer::No, false);
    b.answer(t.t(yes), String::new(), Answer::Yes, false);
    if let Some(more) = more {
        b.answer(t.t(more), String::new(), Answer::YesAndMore, true);
    }
}

// ─── what the rows say ──────────────────────────────────────────────────────

/// The sentence a door says once an order went to the queue.
#[must_use]
pub fn queued_said(label: &str, r: &Readings, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        crate::key::SOFTWARE_ECHO_QUEUED,
        &[("app", label), ("keys", r.taught.queue.as_str())],
    )
}

/// The key of a program a desktop id belongs to, for the launcher's `C-i` and
/// `C-d`: the person's own when a source put it there, else the system's.
#[must_use]
pub fn key_of_desktop(snapshot: Option<&Snapshot>, desktop_id: &str) -> AppKey {
    snapshot
        .and_then(|s| {
            s.installed
                .iter()
                .find(|i| i.desktop_ids.iter().any(|d| same_desktop(d, desktop_id)))
        })
        .map_or_else(
            || AppKey::System {
                desktop_id: desktop_id.to_owned(),
            },
            |i| i.key.clone(),
        )
}

/// A job's progress, as a value: `42 % · downloading`, or the phase alone
/// when nothing can be counted.
fn working_said(job: &ronoctua_transfers::Job, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    if job.stage == ronoctua_transfers::Stage::Weighing && job.phase.is_none() {
        return texts.t(key::SOFTWARE_QUEUED);
    }
    let phase = job.phase.map_or_else(
        || texts.t(key::SOFTWARE_WORKING_NOW),
        |p| texts.t(words::phase_key(p)),
    );
    if job.total == 0 {
        return phase;
    }
    let percent = job.done.saturating_mul(100) / job.total;
    texts.t_with(
        key::SOFTWARE_WORKING,
        &[
            ("percent", percent.to_string().as_str()),
            ("phase", phase.as_str()),
        ],
    )
}

/// The latest job about one program.
fn job_of<'a>(
    jobs: &'a [ronoctua_transfers::Job],
    key: &AppKey,
) -> Option<&'a ronoctua_transfers::Job> {
    jobs.iter()
        .filter(|j| match &j.work {
            ronoctua_transfers::Work::Software { order, .. } => {
                about_key(order).as_ref() == Some(key)
            }
            _ => false,
        })
        .max_by_key(|j| j.id)
}

/// The latest job about one program, when it failed.
fn last_failure<'a>(r: &'a Readings, key: &AppKey) -> Option<&'a ronoctua_transfers::Job> {
    job_of(&r.jobs, key).filter(|j| j.stage == ronoctua_transfers::Stage::Failed)
}

/// Which program an order is about, when it is about one.
fn about_key(order: &Order) -> Option<AppKey> {
    match order {
        Order::Install { key, .. }
        | Order::Remove { key, .. }
        | Order::Declare { key, .. }
        | Order::Forget { key, .. } => Some(key.clone()),
        Order::Upgrade {
            scope: Scope::One { key },
        } => Some(key.clone()),
        Order::Try { attr, .. } => Some(AppKey::Nix { attr: attr.clone() }),
        Order::WebEdit { slug, .. } => Some(AppKey::Web { slug: slug.clone() }),
        _ => None,
    }
}

/// The row a search found for this key.
fn found_of<'a>(r: &'a Readings, key: &AppKey) -> Option<&'a Found> {
    r.group_of(key).and_then(|g| g.member(key))
}

/// The details that arrived for this key, when they are this key's.
fn details_of<'a>(r: &'a Readings, key: &AppKey) -> Option<Result<&'a Details, &'a Refusal>> {
    r.details
        .as_ref()
        .filter(|(k, _)| k == key)
        .map(|(_, d)| d.as_ref())
}

/// What sets it apart: the search's row, else the details.
fn badges_for(r: &Readings, key: &AppKey) -> Vec<Badge> {
    found_of(r, key).map_or_else(
        || {
            details_of(r, key)
                .and_then(Result::ok)
                .map(|d| d.badges.clone())
                .unwrap_or_default()
        },
        |f| f.badges.clone(),
    )
}

/// The command it runs.
fn program_for(r: &Readings, key: &AppKey) -> Option<String> {
    found_of(r, key)
        .and_then(|f| f.program.clone())
        .or_else(|| {
            details_of(r, key)
                .and_then(Result::ok)
                .and_then(|d| d.program.clone())
        })
}

/// The version its source would install.
fn offered_version(r: &Readings, key: &AppKey) -> String {
    found_of(r, key)
        .map(|f| f.version.clone())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            details_of(r, key)
                .and_then(Result::ok)
                .map(|d| d.version.clone())
        })
        .unwrap_or_default()
}

/// How much installing it downloads, when anybody says.
fn download_of(r: &Readings, key: &AppKey) -> Option<u64> {
    details_of(r, key)
        .and_then(Result::ok)
        .and_then(|d| d.download_bytes)
        .or_else(|| found_of(r, key).and_then(|f| f.download_bytes))
}

/// A consent the person already gave, remembered in the preferences.
fn remembered(mut consent: Consent, attr: &str, prefs: &Preferences) -> Consent {
    if prefs.unfree.contains(attr) {
        consent.unfree = true;
    }
    consent
}

/// Why this machine cannot install from a source, when it cannot.
fn refusal_of_source(source: Source, snap: Option<&Snapshot>) -> Option<Refusal> {
    let s = snap?;
    if !s.supports.has(source) {
        return Some(Refusal::SourceAbsent);
    }
    (source == Source::Android && !s.android.prepared).then_some(Refusal::SourceUnprepared)
}

/// Android's state, as one word.
const fn android_state_key(a: &ronoctua_software::state::Android) -> &'static str {
    use crate::key;
    if !a.support {
        key::SOFTWARE_ANDROID_ABSENT
    } else if a.preparing {
        key::SOFTWARE_ANDROID_PREPARING
    } else if !a.prepared {
        key::SOFTWARE_ANDROID_NOT_PREPARED
    } else if a.session {
        key::SOFTWARE_ANDROID_RUNNING
    } else {
        key::SOFTWARE_ANDROID_STOPPED
    }
}

/// Which source the lists show.
fn filter_said(filter: Option<Source>, texts: &ronoctua_i18n::Texts) -> String {
    filter.map_or_else(
        || texts.t(crate::key::SOFTWARE_SOURCE_ALL),
        |s| texts.t(words::source_key(s)),
    )
}

/// How many updates wait.
fn updates_said(n: usize, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    if n == 0 {
        return texts.t(key::SOFTWARE_UP_TO_DATE);
    }
    texts.t_args(
        key::SOFTWARE_UPDATES_COUNT,
        &[("count", ronoctua_i18n::Arg::Number(count(n)))],
    )
}

/// How many trials run.
fn trials_said(n: usize, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        crate::key::SOFTWARE_TRIALS_COUNT,
        &[("count", ronoctua_i18n::Arg::Number(count(n)))],
    )
}

/// The ask-above threshold, as a value.
fn ask_above_said(mb: u64, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    if mb == 0 {
        return texts.t(key::SOFTWARE_ASK_NEVER);
    }
    texts.t_with(
        key::SOFTWARE_ASK_ABOVE_SIZE,
        &[(
            "size",
            readable(mb.saturating_mul(1_000_000), texts).as_str(),
        )],
    )
}

/// On or off.
fn on_off(on: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(if on {
        crate::key::SOFTWARE_ON
    } else {
        crate::key::SOFTWARE_OFF
    })
}

/// Bytes, in the units a download is sold in, with the language's separator.
fn readable(bytes: u64, texts: &ronoctua_i18n::Texts) -> String {
    crate::sizes::readable_bytes(bytes, texts.decimal_mark())
}

/// Two phrases, one after the other, in the language's own way of joining
/// them — a separator is text too (invariant 8).
fn joined(first: &str, second: &str, texts: &ronoctua_i18n::Texts) -> String {
    if second.is_empty() {
        return first.to_owned();
    }
    if first.is_empty() {
        return second.to_owned();
    }
    texts.t_with(
        crate::key::SOFTWARE_JOINED,
        &[("first", first), ("second", second)],
    )
}

/// A count, as a number Fluent can pick a plural with.
fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The same, from a `u64`.
fn count_u64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// One order for the queue.
fn queue(order: Order, label: String) -> Errand {
    Errand::Queue(vec![Queued { order, label }])
}

/// The next or previous of a list, around.
fn turned<T: Clone + PartialEq>(all: &[T], now: &T, forward: bool) -> T {
    if all.is_empty() {
        return now.clone();
    }
    let at = all.iter().position(|x| x == now).unwrap_or(0);
    let step = if forward { 1 } else { all.len() - 1 };
    all[(at + step) % all.len()].clone()
}

/// An address's host, or the address when it has none. The address is one
/// `canonical_url` made, so it has a scheme, a host and no credentials.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit('@').next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    if host.is_empty() {
        url.to_owned()
    } else {
        host.to_owned()
    }
}

/// A typed query that is an ADDRESS, canonical.
///
/// Stricter than `canonical_url`, which makes an address out of any word
/// (`gimp` becomes `https://gimp/`): here it must say `http(s)://`, or be a
/// host of lower-case labels with a dot and no spaces — `web.whatsapp.com`,
/// `mail.example.org/inbox`. A Flatpak id (`org.gimp.GIMP`) has capitals and
/// is left to the search.
fn address_in(query: &str) -> Option<String> {
    let q = query.trim();
    if q.is_empty() || q.contains(char::is_whitespace) {
        return None;
    }
    let lower = q.to_ascii_lowercase();
    let schemed = lower.starts_with("http://") || lower.starts_with("https://");
    if !schemed {
        let host = q.split(['/', '?', '#']).next().unwrap_or_default();
        let host = host.split(':').next().unwrap_or_default();
        let labels: Vec<&str> = host.split('.').collect();
        let plain = labels.len() >= 2
            && labels.iter().all(|l| {
                !l.is_empty()
                    && l.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            })
            && labels
                .last()
                .is_some_and(|t| t.chars().all(|c| c.is_ascii_lowercase()) && t.len() >= 2);
        if !plain {
            return None;
        }
    }
    ronoctua_software::web::canonical_url(q).ok()
}

/// A typed query that is a PATH to a file a source installs.
fn package_file_in(query: &str, home: Option<&Path>) -> Option<PathBuf> {
    let q = query.trim();
    if !(q.starts_with('/') || q.starts_with("~/")) {
        return None;
    }
    let path = expanded(q, home);
    ronoctua_software::order::file_source(&path).map(|_| path)
}

/// `~/x` under the home; anything else as typed.
fn expanded(typed: &str, home: Option<&Path>) -> PathBuf {
    match (typed.strip_prefix("~/"), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => PathBuf::from(typed),
    }
}

// ─── the builder ────────────────────────────────────────────────────────────

struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    focus: &'a Spot,
    /// Which drawers are open.
    open: &'a [&'static str],
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    /// Whether a drawer is open.
    fn is_open(&self, id: &str) -> bool {
        self.open.contains(&id)
    }

    fn push(&mut self, row: Row) {
        self.out.push(row);
    }

    /// A heading's line, already a string.
    fn push_heading(&mut self, text: String, colour: tokens::Color) {
        self.push(Row {
            fields: vec![Self::piece(self.left, text, colour)],
            focused: false,
            heading: true,
            target: Target::Nothing,
            spot: Spot::Nothing,
            disabled: false,
            said: None,
            can: Can::default(),
            picture: None,
        });
    }

    /// A section title, from a key.
    fn heading(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.title(text);
    }

    /// A section title, from words already the person's (a program's name).
    fn title(&mut self, text: String) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: Spot::Nothing,
                disabled: false,
                said: None,
                can: Can::default(),
                picture: None,
            });
        }
        let cut = layout::elide_end(&text, self.room);
        self.push_heading(cut, tokens::color::TEXT_HEADING);
    }

    /// A title with the program's glyph in front — and its picture, which a
    /// door draws over the glyph once it has it.
    fn title_with(&mut self, glyph: char, picture: Option<Portrait>, text: String) {
        self.title(text);
        let mark = format!("{glyph} ");
        let w = layout::width(&mark);
        if let Some(row) = self.out.last_mut()
            && let Some(name) = row.fields.first_mut()
        {
            name.at += w;
            name.text = layout::elide_end(&name.text, self.room.saturating_sub(w));
            row.fields
                .insert(0, Self::piece(self.left, mark, tokens::color::TEXT_HEADING));
            row.picture = picture;
        }
    }

    /// A sentence from a key: it WRAPS, never cut (`chrome::note_lines`).
    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.said(text, tokens::color::TEXT_DIM);
    }

    /// Prose, in a tone of its own, one row per line.
    fn said(&mut self, text: String, colour: tokens::Color) {
        for line in crate::chrome::note_lines(&text, self.room) {
            self.push_heading(line, colour);
        }
    }

    /// A row a key acts on, or a fact.
    fn pair(
        &mut self,
        label: String,
        value: String,
        target: Target,
        spot: Spot,
        can: Can,
        refused: Option<String>,
    ) {
        let focused = spot == *self.focus;
        let acts = !matches!(target, Target::Nothing) || can != Can::default();
        let kind = if refused.is_some() {
            crate::chrome::Kind::Disabled
        } else {
            crate::chrome::Kind::of(acts)
        };
        // A REFUSING ROW SAYS WHY IN ITS VALUE, and `RET` says it whole.
        let value = refused.clone().unwrap_or(value);
        self.push(Row {
            fields: crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
            focused,
            heading: false,
            target,
            spot,
            disabled: refused.is_some(),
            said: refused,
            can,
            picture: None,
        });
    }

    /// A catalogue's consent row — "Ask {hosts}", what for, and that the
    /// press allows it (`consent_rows::lines`): every line is the press, the
    /// cursor stands on the first.
    fn consent(&mut self, source: OnlineSource) {
        let spot = Spot::Let(source.id());
        let focused = spot == *self.focus;
        let ask = crate::consent_rows::Ask {
            hosts: source.hosts(),
            what: source.what(),
        };
        let lines = crate::consent_rows::lines(ask, self.room, focused, self.texts);
        for (n, (text, colour)) in lines.into_iter().enumerate() {
            self.push(Row {
                fields: vec![Self::piece(self.left, text, colour)],
                focused: focused && n == 0,
                // THE CURSOR SKIPS A LATER LINE, and a click takes it all
                // the same: it lands on the first (`Row::takes`,
                // `View::clicked`).
                heading: n > 0,
                target: Target::Let(source),
                spot: if n == 0 { spot.clone() } else { Spot::Nothing },
                disabled: false,
                said: None,
                can: Can::default(),
                picture: None,
            });
        }
    }

    /// A fact: `RET` copies its value.
    fn fact_at(&mut self, label: String, value: String, spot: Spot) {
        self.pair(label, value, Target::Nothing, spot, Can::default(), None);
    }

    /// A deed, its spot named after it.
    fn deed(&mut self, label: &str, value: String, deed: Deed, refused: Option<String>) {
        let spot = Spot::Deed(deed.id());
        self.deed_at(label, value, deed, spot, refused);
    }

    /// A deed at a given spot.
    fn deed_at(
        &mut self,
        label: &str,
        value: String,
        deed: Deed,
        spot: Spot,
        refused: Option<String>,
    ) {
        let label = self.texts.t(label);
        self.pair(
            label,
            value,
            Target::Deed(deed),
            spot,
            Can::default(),
            refused,
        );
    }

    /// A deed in the danger tone: the value says it cannot be undone.
    fn danger(&mut self, label: &str, deed: Deed) {
        let value = self.texts.t(crate::key::SOFTWARE_CANNOT_UNDO);
        self.deed(label, value, deed, None);
        if let Some(row) = self.out.last_mut() {
            row.fields = crate::chrome::tinted(
                std::mem::take(&mut row.fields),
                Some(tokens::color::STATE_ERROR),
                row.focused,
            );
        }
    }

    /// A door to another level.
    fn door(&mut self, label: &str, value: String, level: Level) {
        let spot = Spot::Door(level.door_id());
        let label = self.texts.t(label);
        self.pair(
            label,
            value,
            Target::Door(level),
            spot,
            Can::default(),
            None,
        );
    }

    /// A value that turns.
    fn ring(&mut self, label: &str, value: String, ring: Ring, id: &'static str) {
        let label = self.texts.t(label);
        self.pair(
            label,
            value,
            Target::Ring(ring),
            Spot::Ring(id),
            Can::default(),
            None,
        );
    }

    /// An answer to a question; the more-than-yes one WHOLLY in the danger
    /// tone, words and all — a warning in a value column is the part a narrow
    /// footer cuts. Under the cursor it says what the cursor says.
    fn answer(&mut self, label: String, value: String, answer: Answer, danger: bool) {
        self.pair(
            label,
            value,
            Target::Answer(answer),
            Spot::Answer(answer),
            Can::default(),
            None,
        );
        if danger
            && let Some(row) = self.out.last_mut()
            && !row.focused
        {
            for f in &mut row.fields {
                f.color = tokens::color::STATE_ERROR;
            }
        }
    }

    /// The drawer's own line: a chevron, and the word — the one place a
    /// chevron is drawn (`syncthing.rs`'s rule, and its reason).
    fn drawer(&mut self, key: &str, id: &'static str, open: bool) {
        if !self.out.is_empty() {
            self.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: Spot::Nothing,
                disabled: false,
                said: None,
                can: Can::default(),
                picture: None,
            });
        }
        let spot = Spot::Drawer(id);
        let focused = spot == *self.focus;
        let glyph = if open {
            tokens::icon::COLLAPSE
        } else {
            tokens::icon::EXPAND
        };
        let colour = if focused {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_HEADING
        };
        let mark = format!("{glyph} ");
        let w = layout::width(&mark);
        self.push(Row {
            fields: vec![
                Self::piece(self.left, mark, colour),
                Self::piece(
                    self.left + w,
                    layout::elide_end(&self.texts.t(key), self.room.saturating_sub(w)),
                    colour,
                ),
            ],
            focused,
            heading: false,
            target: Target::Drawer(id),
            spot,
            disabled: false,
            said: None,
            can: Can::default(),
            picture: None,
        });
    }

    /// A row with a picture in front: its glyph, then the label and value.
    /// A door that has the picture draws it over `fields[0]`.
    fn picture_pair(
        &mut self,
        label: String,
        value: String,
        target: Target,
        spot: Spot,
        picture: Option<Portrait>,
    ) {
        self.program(
            tokens::icon::IMAGE,
            picture,
            label,
            value,
            target,
            spot,
            Can::default(),
            &[],
        );
    }

    /// A program: its glyph (where its picture goes), its name — with the
    /// letters the search matched lit — and its value.
    #[allow(clippy::too_many_arguments)]
    fn program(
        &mut self,
        glyph: char,
        picture: Option<Portrait>,
        label: String,
        value: String,
        target: Target,
        spot: Spot,
        can: Can,
        lit: &[usize],
    ) {
        let focused = spot == *self.focus;
        let kind = crate::chrome::Kind::ListedSetting;
        let (label_colour, _) = kind.tones(focused);
        let mark = format!("{glyph} ");
        let w = layout::width(&mark);
        let mut fields = vec![Self::piece(self.left, mark.clone(), label_colour)];
        // THE GLYPH IS PART OF THE LABEL'S WIDTH, NOT OF THE ROW'S LEFT: laid
        // out from after it, the value column moved two cells right of every
        // other row of the section (`declared by you` under `every source`,
        // photographed). The pair is laid from the row's left with the glyph
        // in front, and the glyph is then taken back out into its own piece.
        let mut rest = crate::chrome::field_pair(
            self.left,
            self.room,
            &format!("{mark}{label}"),
            &value,
            kind,
            focused,
        );
        match rest.first_mut() {
            Some(first) if first.at == self.left && first.text.starts_with(&mark) => {
                first.text = first.text[mark.len()..].to_owned();
                first.at += w;
            }
            // NO ROOM FOR THE GLYPH AND A LABEL: the glyph stays `fields[0]`,
            // where a door draws the picture.
            Some(first) => first.text.clear(),
            None => {}
        }
        rest.retain(|f| !f.text.is_empty());
        if !lit.is_empty()
            && let Some(first) = rest.first().cloned()
        {
            rest.splice(0..1, crate::chrome::lit_letters(&first, lit, focused));
        }
        fields.extend(rest);
        self.push(Row {
            fields,
            focused,
            heading: false,
            target,
            spot,
            disabled: false,
            said: None,
            can,
            picture,
        });
    }
}

/// Where a web app's icon is looked for.
#[must_use]
pub const fn shelf_key(shelf: Shelf) -> &'static str {
    use crate::key;
    match shelf {
        Shelf::Site => key::SOFTWARE_SHELF_SITE,
        Shelf::Dashboard => key::SOFTWARE_SHELF_DASHBOARD,
        Shelf::Collection => key::SOFTWARE_SHELF_COLLECTION,
        Shelf::File => key::SOFTWARE_SHELF_FILE,
    }
}

/// The software view's keys in the footer, BESIDE its binding.
pub const SOFTWARE_KEYS: &str = "i i";

/// The software view's trials, BESIDE its binding — for a sentence that
/// names where the trials are (the modeline's trial mark leads there too).
pub const SOFTWARE_TRIALS_KEYS: &str = "i t";

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "software",
    commands: &[
        // THE PERSON'S OWN PROGRAMS (`LEADER i`): the footer's view and its
        // window, the four levels a key opens directly, the five questions
        // its rows ask on their door's line (the footer's minibuffer or the
        // window's header), what a notification offers, and the view's own
        // keys (`software::OF_THE_VIEW`).
        ronoctua_cmd::Command {
            id: "software-open",
            label: crate::key::CMD_SOFTWARE_OPEN,
        },
        ronoctua_cmd::Command {
            id: "software-window",
            label: crate::key::CMD_SOFTWARE_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "software-trials",
            label: crate::key::CMD_SOFTWARE_TRIALS,
        },
        ronoctua_cmd::Command {
            id: "software-maintenance",
            label: crate::key::CMD_SOFTWARE_MAINTENANCE,
        },
        ronoctua_cmd::Command {
            id: "software-android",
            label: crate::key::CMD_SOFTWARE_ANDROID,
        },
        ronoctua_cmd::Command {
            id: "software-web-new",
            label: crate::key::CMD_SOFTWARE_WEB_NEW,
        },
        ronoctua_cmd::Command {
            id: "software-web-address",
            label: crate::key::CMD_SOFTWARE_WEB_ADDRESS,
        },
        ronoctua_cmd::Command {
            id: "software-web-name",
            label: crate::key::CMD_SOFTWARE_WEB_NAME,
        },
        ronoctua_cmd::Command {
            id: "software-icon-path",
            label: crate::key::CMD_SOFTWARE_ICON_PATH,
        },
        ronoctua_cmd::Command {
            id: "software-file-path",
            label: crate::key::CMD_SOFTWARE_FILE_PATH,
        },
        ronoctua_cmd::Command {
            id: "software-launch-latest",
            label: crate::key::CMD_SOFTWARE_LAUNCH_LATEST,
        },
        ronoctua_cmd::Command {
            id: "software-down",
            label: crate::key::CMD_SOFTWARE_DOWN,
        },
        ronoctua_cmd::Command {
            id: "software-up",
            label: crate::key::CMD_SOFTWARE_UP,
        },
        ronoctua_cmd::Command {
            id: "software-enter",
            label: crate::key::CMD_SOFTWARE_ENTER,
        },
        ronoctua_cmd::Command {
            id: "software-next-source",
            label: crate::key::CMD_SOFTWARE_NEXT_SOURCE,
        },
        ronoctua_cmd::Command {
            id: "software-previous-source",
            label: crate::key::CMD_SOFTWARE_PREVIOUS_SOURCE,
        },
        ronoctua_cmd::Command {
            id: "software-less",
            label: crate::key::CMD_SOFTWARE_LESS,
        },
        ronoctua_cmd::Command {
            id: "software-more",
            label: crate::key::CMD_SOFTWARE_MORE,
        },
        ronoctua_cmd::Command {
            id: "software-install",
            label: crate::key::CMD_SOFTWARE_INSTALL,
        },
        ronoctua_cmd::Command {
            id: "software-try-it",
            label: crate::key::CMD_SOFTWARE_TRY_IT,
        },
        ronoctua_cmd::Command {
            id: "software-launch",
            label: crate::key::CMD_SOFTWARE_LAUNCH,
        },
        ronoctua_cmd::Command {
            id: "software-update",
            label: crate::key::CMD_SOFTWARE_UPDATE,
        },
        ronoctua_cmd::Command {
            id: "software-edit",
            label: crate::key::CMD_SOFTWARE_EDIT,
        },
        ronoctua_cmd::Command {
            id: "software-remove",
            label: crate::key::CMD_SOFTWARE_REMOVE,
        },
        ronoctua_cmd::Command {
            id: "software-stop",
            label: crate::key::CMD_SOFTWARE_STOP,
        },
        ronoctua_cmd::Command {
            id: "software-paste",
            label: crate::key::CMD_SOFTWARE_PASTE,
        },
        // THE SOFTWARE CENTRE'S PASSPHRASE: a change of the declaration made
        // with no identity kept asks it first, and the answer comes back as
        // this command's argument (`software::Ask::Sign`).
        ronoctua_cmd::Command {
            id: "software-sign",
            label: crate::key::CMD_SOFTWARE_SIGN,
        },
    ],
    bindings: &[
        // `LEADER i` — THE PERSON'S OWN PROGRAMS: install, try, update and
        // remove, for this user only. `i` because the verb begins with it in
        // both languages, and it was free; `a` stays the bare launcher. `i i` is
        // the footer (the doubled letter every view here uses) and `i w` the
        // window; the other four open the view on one of its levels, and
        // `i b` — `b` for browser, since `w` is the window's letter — asks
        // for a web app's address first.
        (SOFTWARE_KEYS, "software-open", None),
        ("i w", "software-window", None),
        (SOFTWARE_TRIALS_KEYS, "software-trials", None),
        ("i b", "software-web-new", None),
        ("i u", "software-maintenance", None),
        ("i a", "software-android", None),
    ],
    groups: &[
        // `i` FOR THE PERSON'S OWN PROGRAMS — see the bindings.
        ("i", crate::key::GROUP_SOFTWARE),
    ],
    by_name: &[
        // THE SOFTWARE VIEW'S QUESTIONS: rows of the view run them. The web
        // app's address and name open its door's line asking for its words
        // (`ask-<id>`) — the footer's minibuffer or the window's header — and the
        // answer comes back as the same command's argument; the icon's image and
        // the file to install open the view's file picker instead
        // (`software::Picking`), and a path given as the argument is the answer.
        "software-web-address",
        "software-web-name",
        "software-icon-path",
        "software-file-path",
        // And the belongings' passphrase a change of the declaration asks first
        // when no identity is kept: the answer to the view's own question.
        "software-sign",
        // AND WHAT A NOTIFICATION OFFERS once something was installed: open it.
        // A key of its own would open whatever was installed last, which is a
        // key that means something different every day.
        "software-launch-latest",
    ],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::software::OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .collect()
}

#[cfg(test)]
mod tests;
