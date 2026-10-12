//! The file manager's sidebar — where one starts from.
//!
//! # What is here, and what is deliberately not
//!
//! This module models what a person CONFIGURED: the folders the distro ships
//! with, whatever they added, what they hid, and whether the sidebar is on
//! screen at all. It is pure, every root is a parameter, and its tests run
//! against a temporary tree.
//!
//! **Devices are not here**, and that is a decision rather than an omission. A
//! disk is discovered and not configured, and modelling it would mean this
//! crate depending on `ronoctua-disks` — which would put zbus inside
//! `ronoctua-files-root`, a binary that runs as root, and would give this crate
//! a second border. The host owns both and hands the device rows in through
//! [`Sidebar::devices_are`]; assembling and ordering them stays here, so there
//! is still one place that answers "what does the sidebar look like".
//!
//! # Hiding is not removing, and the difference is visible
//!
//! A row the distro ships can be hidden and cannot be removed: it comes from
//! Nix and would be back on the next start, and a removal that silently undoes
//! itself is the class of quiet failure this project refuses everywhere else.
//! A row the person added is removed outright. [`Gone`] says which happened, so
//! the sentence on screen can differ.
//!
//! # A default that is not on the machine is dropped in silence
//!
//! The same silence `listing::start_at` already keeps: a row that leads
//! nowhere is worse than a row that is missing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a row points at.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Target {
    /// A directory — the ordinary case. Entered, like any other.
    Place(PathBuf),
    /// A file. Opened by whatever opens its type; never entered.
    File(PathBuf),
    /// An application, by its freedesktop id.
    ///
    /// The ID and not the command: a store path would name a build, and the day
    /// the system is rebuilt the row would point at a binary that has been
    /// garbage-collected. An id survives every rebuild.
    Application(String),
    /// The freedesktop trash. It carries no path because the trash's root is a
    /// PARAMETER of whoever assembles the sidebar — writing one in would name
    /// a machine (invariant 13).
    Trash,
    /// A disk, a partition or a stick, by the id the host's disk border gave
    /// it. Never configured, never written to the file: it is here so that a
    /// device and a folder are one kind of row on screen and in the keyboard.
    Device(String),
    // ── anchor: L12 ──
    /// The `/private` vault — the person's encrypted folder. Never
    /// configured: the host hands its row in with the devices, open or
    /// locked as the vault's status says, and `m` and `l` open and lock it.
    Private,
    // ── anchor: L17 ──
    /// A whole REMOVABLE drive, by the id the host's disk border gave it: the
    /// heading the host draws above a stick's volumes — and the one row a
    /// BLANK stick has, since nothing on it is a volume to be a row. It is
    /// where writing the installer is offered (`sidebar_menu::Applies::Drive`).
    /// Never configured, never written to the file.
    Drive(String),
}

/// Which block of the sidebar a row belongs to.
///
/// The blocks are separated by ONE BLANK LINE and nothing else — no heading, no
/// rule, no separator (invariants 14 and 15). A heading would cost a line, a
/// text key and a `Hit` for something nobody can click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    /// What somebody chose: the distro's folders, then their own.
    Pinned,
    /// What is plugged in.
    Devices,
    /// The trash, always last.
    Trash,
}

/// A row as the person's FILE holds it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    /// Stable, and it is what an adjustment keys on: `place:main/temp`,
    /// `file:/etc/hosts`, `app:firefox`, `trash`, `device:<id>`.
    pub id: String,
    /// Where it goes.
    pub target: Target,
    /// What to call it.
    ///
    /// `None` means "the target's own name", which is what one wants: a folder
    /// renamed on disk renames itself here. A name is only stored when it
    /// cannot be derived — an application's, which lives in its `.desktop`.
    #[serde(default)]
    pub label: Option<String>,
}

/// What the person did to a row the DISTRO ships.
///
/// The same "use the default, or override only the fields you name" shape as
/// `ronoctua_notify::store::Preferences`: an id that is not in the map uses the
/// default, and one that is overrides only what it mentions.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Adjustment {
    /// Off the screen, but still in the file, so it can come back.
    pub hidden: bool,
    /// A name of their own.
    pub label: Option<String>,
}

/// The file: `$XDG_CONFIG_HOME/ronoctua/files-sidebar.json`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// What the person put there, in the order they put it.
    pub added: Vec<Item>,
    /// What they did to what the distro ships, keyed by id.
    pub adjustments: BTreeMap<String, Adjustment>,
    /// Is the sidebar on screen?
    ///
    /// ITS OWN `default`, and not the struct's. `#[serde(default)]` on a `bool`
    /// gives `false`, so a file that PARSES but predates this field — or one
    /// somebody hand-edited down to `{}` — would come up with the whole sidebar
    /// gone and nothing on screen saying why. `Preferences::fresh()` does not
    /// help there: it is only reached when the file is absent or broken.
    /// Caught by `a_missing_file_and_a_broken_one_both_come_up_showing`.
    #[serde(default = "yes")]
    pub showing: bool,
}

/// The default of [`Preferences::showing`]. serde wants a function.
const fn yes() -> bool {
    true
}

impl Default for Preferences {
    /// What somebody has before they have ever changed anything — the
    /// sidebar SHOWING. Written out, not derived: a derived `bool` is `false`,
    /// and this is what a store reads for a file that is not there, and what
    /// a repair puts in place of a value that will not read.
    fn default() -> Self {
        Self {
            added: Vec::new(),
            adjustments: BTreeMap::new(),
            showing: true,
        }
    }
}

impl Preferences {
    /// What somebody has before they have ever changed anything.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }
}

/// What happened when a row was taken off the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gone {
    /// It came from the distro: hidden, and it can come back.
    Hidden,
    /// It was the person's: removed, and it cannot.
    Removed,
    /// There was no such row.
    Missing,
}

/// Whether an encrypted device is open.
///
/// Carried as a fact, like [`Row::needed`]: this crate does not know what a
/// LUKS container is — the disk border does, and it hands the answer in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seal {
    /// Locked: what is inside cannot be reached until its passphrase opens
    /// it. Mounting asks for it first.
    Locked,
    /// Open: this row is what came out of the container, and ejecting locks
    /// it again before the power goes.
    Unlocked,
}

/// One row, resolved and ready to be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The id, for adjustments and for the keyboard.
    pub id: String,
    /// Where it goes.
    pub target: Target,
    /// Already a NAME and never a text key.
    ///
    /// `None` for [`Target::Trash`] alone, whose word belongs to the language
    /// bundle the host holds. A raw key written into a label and drawn verbatim
    /// is the exact defect the manager's picker was reported for.
    pub label: Option<String>,
    /// Which block it is in.
    pub section: Section,
    /// For a device: is it mounted? `None` for everything else.
    pub mounted: Option<bool>,
    /// Where a mounted device is, if it is anywhere.
    pub at: Option<PathBuf>,
    /// udisks said not to put this in front of somebody (`HintIgnore`: the
    /// ESP, a stick's little EFI helper). It is LISTED anyway — last in its
    /// block, in the quiet register — because a row nobody can bring back is a
    /// row somebody needed once; there is no key that hides it.
    pub quiet: bool,
    /// For a device: does udisks say it belongs to the machine?
    ///
    /// It is the SAME property polkit splits on — mounting one of these is
    /// `auth_admin_keep`, and there is no authentication agent in this session
    /// — so it is what decides whether the row offers to mount at all.
    pub system: bool,
    /// For a device: does the running system need this one?
    ///
    /// Already RESOLVED, and carried as a sentence's worth of fact rather than
    /// as the volume itself, because this crate does not know what a volume is
    /// — the disk border does, and it hands the answer in. `Some` is the mount
    /// point or the word that decided; it is what the refusal names, and a
    /// refusal that names nothing is a rule nobody can check.
    pub needed: Option<String>,
    /// For a device: can the drive it sits on be pulled out?
    ///
    /// It is what decides the GLYPH, and it is a different fact from
    /// [`Row::mounted`], which decides the colour. Reading "unmounted" as
    /// "removable" — which is what a `mounted.is_none()` test would do — draws
    /// an internal partition as a stick the moment somebody unmounts it.
    pub removable: bool,
    /// It comes from the distro: it can be hidden, never removed.
    pub built_in: bool,
    /// For an encrypted device: locked, or open. `None` for everything that
    /// is not encrypted.
    pub seal: Option<Seal>,
}

/// The sidebar's whole state.
#[derive(Debug, Clone, Default)]
pub struct Sidebar {
    /// WHERE IT PERSISTS, and `None` means it does not.
    ///
    /// A PARAMETER and never the environment, which is the rule this crate
    /// states everywhere and which this field exists because it was broken:
    /// persistence read `$XDG_CONFIG_HOME` on its own, so the unit tests —
    /// which press the key that hides the sidebar and the key that pins a
    /// file — wrote into the REAL configuration of whoever ran `cargo test`.
    /// They left a `/tmp` path pinned and the sidebar switched off, and the
    /// symptom was a file manager with no sidebar and nothing anywhere saying
    /// why.
    ///
    /// It is the same reason `trash::Bin` takes its root: "two tests setting
    /// the same environment variable race, and the one that loses puts a file
    /// in the other's trash".
    store: Option<PathBuf>,
    /// The distro's rows, already resolved against the machine.
    defaults: Vec<Row>,
    /// The device rows the host handed in.
    devices: Vec<Row>,
    /// The trash's row, when there is a trash.
    bin: Option<Row>,
    /// What the person changed.
    prefs: Preferences,
    /// Everything, in order, rebuilt whenever one of the above moves.
    ///
    /// Cached and not computed per call: `rows_from` asks for this on every
    /// frame, and the manager has already paid once for a listing that rebuilt
    /// itself several times per pointer move.
    rows: Vec<Row>,
}

impl Sidebar {
    /// Builds one.
    ///
    /// EVERY ROOT IS A PARAMETER — the arrangement `ronoctua_memory::Places`
    /// uses and this crate follows everywhere: `defaults` are names relative to
    /// home, `home` and `trash` are where those two are. Nothing here reads the
    /// environment, so a test builds a whole sidebar over a temporary tree.
    #[must_use]
    pub fn of(
        defaults: &[String],
        home: Option<&Path>,
        trash: Option<&Path>,
        prefs: Preferences,
        store: Option<PathBuf>,
    ) -> Self {
        let mut side = Self {
            store,
            defaults: Vec::new(),
            devices: Vec::new(),
            bin: None,
            prefs,
            rows: Vec::new(),
        };
        if let Some(home) = home {
            for name in defaults {
                let path = home.join(name);
                // SILENTLY, and it is the same silence `listing::start_at`
                // keeps: a machine without `main/prj` gets one row fewer, not a
                // row that goes nowhere.
                if !path.is_dir() {
                    continue;
                }
                side.defaults.push(Row {
                    id: format!("place:{name}"),
                    label: Some(leaf_of(&path)),
                    target: Target::Place(path),
                    section: Section::Pinned,
                    mounted: None,
                    at: None,
                    quiet: false,
                    system: false,
                    needed: None,
                    removable: false,
                    built_in: true,
                    seal: None,
                });
            }
        }
        if let Some(trash) = trash {
            side.bin = Some(Row {
                id: "trash".to_owned(),
                target: Target::Trash,
                // The one row whose word is the LANGUAGE's, so the host names it.
                label: None,
                section: Section::Trash,
                mounted: None,
                at: Some(trash.to_path_buf()),
                quiet: false,
                system: false,
                needed: None,
                removable: false,
                built_in: true,
                seal: None,
            });
        }
        side.assemble();
        side
    }

    /// Replaces the device rows.
    ///
    /// The host builds them, because a device comes from the disk border and
    /// this crate deliberately does not know that border exists — see the
    /// module's header.
    pub fn devices_are(&mut self, rows: Vec<Row>) {
        self.devices = rows;
        self.assemble();
    }

    /// The device rows, so a re-read of the FILE does not throw them away.
    ///
    /// They came from the bus and not from the file, and the two change for
    /// different reasons: pinning an application rewrites the file, and a stick
    /// plugged in does not.
    #[must_use]
    pub fn device_rows(&self) -> Vec<Row> {
        self.devices.clone()
    }

    /// Every row, in order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The lines on screen right now, WITH the gaps between blocks.
    ///
    /// `None` is a blank line, and it is the only thing separating the places
    /// from the devices from the trash: no heading, no rule, no box
    /// (invariants 14 and 15). A heading would cost a line AND a word AND a
    /// text key AND a `Hit` for something nobody can click; a blank line costs
    /// one line and says the same thing.
    ///
    /// Never leading and never trailing — a gap at the top would just be the
    /// sidebar starting one line lower than the listing, which reads as a bug.
    ///
    /// EVERY row, the quiet ones included: they come last in their block
    /// ([`Sidebar::assemble`]) and the host draws them in the quiet register.
    /// Filtering them out took away rows somebody asked for, and the key that
    /// brought them back is gone with the one that hid dotfiles.
    #[must_use]
    pub fn shown(&self) -> Vec<Option<&Row>> {
        let mut out: Vec<Option<&Row>> = Vec::new();
        let mut last: Option<Section> = None;
        for row in &self.rows {
            if last.is_some_and(|s| s != row.section) {
                out.push(None);
            }
            last = Some(row.section);
            out.push(Some(row));
        }
        out
    }

    /// How many ROWS are on screen — the gaps do not count.
    #[must_use]
    pub fn how_many(&self) -> usize {
        self.rows.len()
    }

    /// Is the sidebar on screen at all?
    #[must_use]
    pub const fn showing(&self) -> bool {
        self.prefs.showing
    }

    /// Shows it, or hides it, and answers the new state.
    pub const fn toggle(&mut self) -> bool {
        self.prefs.showing = !self.prefs.showing;
        self.prefs.showing
    }

    /// What to write to disk.
    #[must_use]
    pub const fn preferences(&self) -> &Preferences {
        &self.prefs
    }

    /// Writes it where this sidebar was told to, and NOWHERE otherwise.
    ///
    /// A sidebar built with no store — every one a test builds — writes
    /// nothing and says so by succeeding: there is no failure to report, and
    /// an error here would make every test that presses a key have to handle
    /// one.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn save(&self) -> std::io::Result<()> {
        match &self.store {
            Some(dir) => store::write_to(dir, &self.prefs),
            None => Ok(()),
        }
    }

    /// When the file it persists to last changed, or `None` when it does not
    /// persist.
    #[must_use]
    pub fn stamp(&self) -> Option<std::time::SystemTime> {
        store::stamp_of(self.store.as_deref()?)
    }

    /// Where it persists, so a re-read can be built with the same store.
    #[must_use]
    pub fn store_dir(&self) -> Option<&Path> {
        self.store.as_deref()
    }

    /// Puts something on the sidebar. `false` when it was already there.
    ///
    /// Already there and not a duplicate: two rows leading to one folder is a
    /// sidebar one has to read twice to use once.
    pub fn add(&mut self, item: Item) -> bool {
        // HIDDEN COMES FIRST, and getting that order wrong is what the test
        // `putting_back_something_hidden_shows_it_rather_than_refusing` was
        // written for. A hidden row is in neither `rows` (assemble drops it)
        // nor `added` (a default never was), so a check on those two says "new"
        // and pushes a second copy — which `assemble` then hides again under
        // the same adjustment. The row would be added, twice over, and still
        // not on screen.
        //
        // And showing it again is what somebody means: the row is not on screen
        // for them to have seen it, so they did not ask for a duplicate.
        if let Some(a) = self.prefs.adjustments.get_mut(&item.id)
            && a.hidden
        {
            a.hidden = false;
            self.assemble();
            return true;
        }
        if self.rows.iter().any(|r| r.id == item.id)
            || self.prefs.added.iter().any(|i| i.id == item.id)
        {
            return false;
        }
        self.prefs.added.push(item);
        self.assemble();
        true
    }

    /// Takes a row off the sidebar — hiding the distro's, removing the
    /// person's.
    pub fn drop_row(&mut self, id: &str) -> Gone {
        if let Some(n) = self.prefs.added.iter().position(|i| i.id == id) {
            self.prefs.added.remove(n);
            self.assemble();
            return Gone::Removed;
        }
        if self.rows.iter().any(|r| r.id == id) {
            self.prefs
                .adjustments
                .entry(id.to_owned())
                .or_default()
                .hidden = true;
            self.assemble();
            return Gone::Hidden;
        }
        Gone::Missing
    }

    /// The rows that are hidden right now, so one can be brought back.
    ///
    /// It answers ids and labels rather than `Row`s, because a hidden row is
    /// not assembled: what a person picks from is a list of names.
    #[must_use]
    pub fn hidden(&self) -> Vec<(String, String)> {
        self.prefs
            .adjustments
            .iter()
            .filter(|(_, a)| a.hidden)
            .map(|(id, a)| {
                let label = a
                    .label
                    .clone()
                    .or_else(|| self.name_of(id))
                    .unwrap_or_else(|| id.clone());
                (id.clone(), label)
            })
            .collect()
    }

    /// Brings a hidden row back.
    pub fn unhide(&mut self, id: &str) {
        if let Some(a) = self.prefs.adjustments.get_mut(id) {
            a.hidden = false;
        }
        self.assemble();
    }

    /// What a hidden row would have been called, from what we know of it.
    fn name_of(&self, id: &str) -> Option<String> {
        self.defaults
            .iter()
            .chain(self.devices.iter())
            .chain(self.bin.iter())
            .find(|r| r.id == id)
            .and_then(|r| r.label.clone())
            .or_else(|| id.split_once(':').map(|(_, rest)| rest.to_owned()))
    }

    /// Rebuilds the ordered list: the distro's rows, then the person's, then
    /// the devices, then the trash.
    fn assemble(&mut self) {
        let mut rows: Vec<Row> = Vec::new();
        for r in &self.defaults {
            rows.push(r.clone());
        }
        for item in &self.prefs.added {
            rows.push(Row {
                id: item.id.clone(),
                label: item.label.clone().or_else(|| match &item.target {
                    Target::Place(p) | Target::File(p) => Some(leaf_of(p)),
                    Target::Application(id) => Some(id.clone()),
                    Target::Device(_) | Target::Drive(_) | Target::Trash | Target::Private => None,
                }),
                target: item.target.clone(),
                section: Section::Pinned,
                mounted: None,
                at: None,
                quiet: false,
                system: false,
                needed: None,
                removable: false,
                built_in: false,
                seal: None,
            });
        }
        // THE QUIET DEVICES LAST, in the order the disk border gave them
        // otherwise: a stick is what one reaches for, the ESP beside it is what
        // one reaches for once a year.
        rows.extend(self.devices.iter().filter(|r| !r.quiet).cloned());
        rows.extend(self.devices.iter().filter(|r| r.quiet).cloned());
        rows.extend(self.bin.iter().cloned());

        // The adjustments go on LAST, over everything — a device can be hidden
        // as readily as a folder, and it is keyed the same way.
        for row in &mut rows {
            if let Some(label) = self
                .prefs
                .adjustments
                .get(&row.id)
                .and_then(|a| a.label.as_ref())
            {
                row.label = Some(label.clone());
            }
        }
        rows.retain(|r| !self.prefs.adjustments.get(&r.id).is_some_and(|a| a.hidden));
        self.rows = rows;
    }
}

/// The last component of a path, as a name.
///
/// The whole path for `/`, which has no last component — a row reading `""`
/// would be a row nobody can identify.
fn leaf_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// What survives a restart, and where.
///
/// The same two directories, and the same reasoning, as
/// `ronoctua_notify::store`: this is CONFIGURATION — what the person chose —
/// so it lives under `$XDG_CONFIG_HOME`, next to the notification preferences
/// and the app-id table that are already there. That directory is already in
/// `nix/modules/persist.nix`, so an impermanent root does not take it.
pub mod store {
    use std::path::Path;

    use super::Preferences;

    // THERE IS NO `read()` OR `write()` HERE, and their absence is the fix for
    // a defect that reached the owner's machine.
    //
    // They existed, they found `$XDG_CONFIG_HOME` on their own, and the view's
    // unit tests — which press the key that hides the sidebar and the key that
    // pins a file — wrote into the REAL configuration of whoever ran
    // `cargo test`. They left a `/tmp` path pinned and `showing: false`, and
    // the symptom was a file manager with no sidebar at all, on a build that
    // had one compiled in.
    //
    // Everything here takes the directory as a PARAMETER, which is the rule
    // this crate states everywhere else — `trash::Bin`, `who::Roster`,
    // `when::Zone` — and `config_dir` is what the APPLICATION calls, once,
    // before handing the answer in.

    /// The file, under `$XDG_CONFIG_HOME/ronoctua`.
    pub const FILE: &str = "files-sidebar.json";

    /// `$XDG_CONFIG_HOME/ronoctua`, from the one crate that owns the roots
    /// (`ronoctua-xdg`, which says why it exists).
    pub use ronoctua_xdg::config_dir;

    /// The file as a [`ronoctua_store::Store`] in `dir`.
    fn store_at(dir: &Path) -> ronoctua_store::Store<Preferences> {
        ronoctua_store::Store::at(Some(dir.to_path_buf()), FILE)
    }

    /// Reads the preferences from an explicit directory — the form a test uses.
    ///
    /// A missing file is a fresh sidebar. A BROKEN one keeps what still
    /// reads: a row the person added that will not parse costs that row, and
    /// the store says so — loudly, and in the repair ledger the belongings'
    /// page shows. A file that is not JSON at all is a fresh sidebar, said
    /// the same way.
    #[must_use]
    pub fn read_from(dir: &Path) -> Preferences {
        store_at(dir).read()
    }

    /// Writes them — whole and renamed over, so a full disk or a kill
    /// mid-write leaves the previous file rather than half of this one, and
    /// keeping what the file holds that this machine does not know.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn write_to(dir: &Path, p: &Preferences) -> std::io::Result<()> {
        store_at(dir).write(p)
    }

    /// When the file last changed, for "has somebody edited it".
    #[must_use]
    pub fn stamp_of(dir: &Path) -> Option<std::time::SystemTime> {
        std::fs::metadata(dir.join(FILE)).ok()?.modified().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree {
        root: PathBuf,
    }

    impl Tree {
        fn fresh(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("ronoctua-sidebar-{name}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn dir(&self, name: &str) -> PathBuf {
            let p = self.root.join(name);
            std::fs::create_dir_all(&p).unwrap();
            p
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn names(side: &Sidebar) -> Vec<String> {
        side.rows()
            .iter()
            .map(|r| r.label.clone().unwrap_or_else(|| r.id.clone()))
            .collect()
    }

    fn a_device(id: &str, mounted: bool, quiet: bool) -> Row {
        Row {
            id: format!("device:{id}"),
            target: Target::Device(id.to_owned()),
            label: Some(id.to_owned()),
            section: Section::Devices,
            mounted: Some(mounted),
            at: mounted.then(|| PathBuf::from("/run/media/somebody").join(id)),
            quiet,
            system: false,
            needed: None,
            removable: true,
            built_in: true,
            seal: None,
        }
    }

    #[test]
    fn a_default_whose_directory_is_not_there_is_dropped_in_silence() {
        let tree = Tree::fresh("missing");
        tree.dir("main");
        tree.dir("main/temp");
        // `main/prj` is deliberately NOT created — which is the state of the
        // machine this was written on, and the reason the silence is the right
        // answer: a row that leads nowhere is worse than a row that is missing.
        let side = Sidebar::of(
            &["main".into(), "main/temp".into(), "main/prj".into()],
            Some(&tree.root),
            None,
            Preferences::fresh(),
            None,
        );
        assert_eq!(names(&side), ["main", "temp"]);
    }

    #[test]
    fn the_trash_is_last_whatever_else_is_there() {
        let tree = Tree::fresh("trash-last");
        tree.dir("main");
        let mut side = Sidebar::of(
            &["main".into()],
            Some(&tree.root),
            Some(&tree.root.join("bin")),
            Preferences::fresh(),
            None,
        );
        side.devices_are(vec![a_device("stick", true, false)]);
        side.add(Item {
            id: "place:/etc".to_owned(),
            target: Target::Place(PathBuf::from("/etc")),
            label: None,
        });
        let sections: Vec<Section> = side.rows().iter().map(|r| r.section).collect();
        assert_eq!(
            sections,
            [
                Section::Pinned,
                Section::Pinned,
                Section::Devices,
                Section::Trash
            ]
        );
        assert_eq!(side.rows().last().unwrap().target, Target::Trash);
    }

    #[test]
    fn the_gap_sits_between_blocks_and_never_at_an_end() {
        let tree = Tree::fresh("gaps");
        tree.dir("main");
        let mut side = Sidebar::of(
            &["main".into()],
            Some(&tree.root),
            Some(&tree.root.join("bin")),
            Preferences::fresh(),
            None,
        );
        side.devices_are(vec![a_device("stick", true, false)]);
        // Three blocks, three rows, two gaps: place · gap · device · gap · trash.
        let lines = side.shown();
        let shape: Vec<bool> = lines.iter().map(Option::is_some).collect();
        assert_eq!(shape, [true, false, true, false, true]);
        assert_eq!(side.how_many(), 3, "the gaps are not content lines");

        // ONE BLOCK, NO GAP. A leading or trailing blank would just be the
        // sidebar starting a line lower or ending a line later than the
        // listing, which reads as a defect and not as separation.
        let alone = Sidebar::of(
            &["main".into()],
            Some(&tree.root),
            None,
            Preferences::fresh(),
            None,
        );
        assert_eq!(alone.shown().len(), 1);
        assert!(alone.shown()[0].is_some());

        // AND NOTHING AT ALL IS NOTHING, not a stray blank.
        let empty = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        assert!(empty.shown().is_empty());
    }

    #[test]
    fn the_trash_carries_no_label_because_its_word_belongs_to_the_language() {
        let tree = Tree::fresh("trash-label");
        let side = Sidebar::of(
            &[],
            Some(&tree.root),
            Some(&tree.root.join("bin")),
            Preferences::fresh(),
            None,
        );
        assert_eq!(side.rows().len(), 1);
        assert_eq!(side.rows()[0].label, None);
    }

    #[test]
    fn hiding_a_default_is_not_removing_it() {
        let tree = Tree::fresh("hide-default");
        tree.dir("main");
        let mut side = Sidebar::of(
            &["main".into()],
            Some(&tree.root),
            None,
            Preferences::fresh(),
            None,
        );
        assert_eq!(side.drop_row("place:main"), Gone::Hidden);
        assert!(names(&side).is_empty());
        // Still in the file, so it can come back — which is the whole reason a
        // default is hidden rather than removed: it comes from Nix and would be
        // back on the next start anyway.
        assert_eq!(side.hidden().len(), 1);
        side.unhide("place:main");
        assert_eq!(names(&side), ["main"]);
    }

    #[test]
    fn removing_what_somebody_added_is_removing_it() {
        let tree = Tree::fresh("remove-added");
        let mut side = Sidebar::of(&[], Some(&tree.root), None, Preferences::fresh(), None);
        side.add(Item {
            id: "app:firefox".to_owned(),
            target: Target::Application("firefox".to_owned()),
            label: Some("Firefox".to_owned()),
        });
        assert_eq!(names(&side), ["Firefox"]);
        assert_eq!(side.drop_row("app:firefox"), Gone::Removed);
        assert!(names(&side).is_empty());
        // And it does NOT linger as a hidden row: there is nothing to bring
        // back, and offering it would be offering something that is gone.
        assert!(side.hidden().is_empty());
    }

    #[test]
    fn the_two_answers_differ_because_the_sentence_differs() {
        let tree = Tree::fresh("two-answers");
        tree.dir("main");
        let mut side = Sidebar::of(
            &["main".into()],
            Some(&tree.root),
            None,
            Preferences::fresh(),
            None,
        );
        side.add(Item {
            id: "place:/etc".to_owned(),
            target: Target::Place(PathBuf::from("/etc")),
            label: None,
        });
        assert_ne!(side.drop_row("place:main"), side.drop_row("place:/etc"));
        assert_eq!(side.drop_row("place:nowhere"), Gone::Missing);
    }

    #[test]
    fn putting_back_something_hidden_shows_it_rather_than_refusing() {
        let tree = Tree::fresh("re-add");
        tree.dir("main");
        let mut side = Sidebar::of(
            &["main".into()],
            Some(&tree.root),
            None,
            Preferences::fresh(),
            None,
        );
        side.drop_row("place:main");
        assert!(names(&side).is_empty());
        // Somebody who navigates back to `main` and asks for it on the sidebar
        // means "show it", not "you already have it" — and the row is not on
        // screen for them to see either.
        assert!(side.add(Item {
            id: "place:main".to_owned(),
            target: Target::Place(tree.root.join("main")),
            label: None,
        }));
        assert_eq!(names(&side), ["main"]);
    }

    #[test]
    fn adding_the_same_thing_twice_is_refused() {
        let tree = Tree::fresh("twice");
        let mut side = Sidebar::of(&[], Some(&tree.root), None, Preferences::fresh(), None);
        let item = Item {
            id: "place:/etc".to_owned(),
            target: Target::Place(PathBuf::from("/etc")),
            label: None,
        };
        assert!(side.add(item.clone()));
        assert!(!side.add(item));
        assert_eq!(side.rows().len(), 1);
    }

    #[test]
    fn quiet_devices_come_last() {
        let tree = Tree::fresh("quiet");
        let mut side = Sidebar::of(&[], Some(&tree.root), None, Preferences::fresh(), None);
        // THE QUIET ONE FIRST, as udisks may well list it.
        side.devices_are(vec![
            a_device("efi-helper", false, true),
            a_device("stick", true, false),
            a_device("backup", false, false),
        ]);
        // LISTED, every one: there is no key that hides a row any more.
        assert_eq!(side.how_many(), 3);
        assert_eq!(side.shown().len(), 3, "one block, no gap");
        // And the quiet one is LAST in its block, the others in udisks' order.
        let order: Vec<&str> = side
            .shown()
            .into_iter()
            .flatten()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(
            order,
            ["device:stick", "device:backup", "device:efi-helper"]
        );
    }

    #[test]
    fn a_device_can_be_hidden_like_anything_else() {
        let tree = Tree::fresh("hide-device");
        let mut side = Sidebar::of(&[], Some(&tree.root), None, Preferences::fresh(), None);
        side.devices_are(vec![a_device("stick", true, false)]);
        assert_eq!(side.drop_row("device:stick"), Gone::Hidden);
        assert!(side.rows().is_empty());
        // And it stays hidden when the disks are read again, which is what
        // makes hiding one worth doing at all.
        side.devices_are(vec![a_device("stick", true, false)]);
        assert!(side.rows().is_empty());
    }

    #[test]
    fn the_ids_are_the_same_across_two_assemblies() {
        let tree = Tree::fresh("stable-ids");
        tree.dir("main");
        tree.dir("main/temp");
        let ids = |s: &Sidebar| -> Vec<String> { s.rows().iter().map(|r| r.id.clone()).collect() };
        let once = Sidebar::of(
            &["main".into(), "main/temp".into()],
            Some(&tree.root),
            Some(&tree.root.join("bin")),
            Preferences::fresh(),
            None,
        );
        let twice = Sidebar::of(
            &["main".into(), "main/temp".into()],
            Some(&tree.root),
            Some(&tree.root.join("bin")),
            Preferences::fresh(),
            None,
        );
        assert_eq!(ids(&once), ids(&twice));
        assert_eq!(ids(&once), ["place:main", "place:main/temp", "trash"]);
    }

    #[test]
    fn a_sidebar_with_no_store_writes_absolutely_nothing() {
        // THE DEFECT THIS EXISTS FOR, and it reached the owner's machine.
        //
        // Persistence used to find `$XDG_CONFIG_HOME` on its own. The view's
        // unit tests press the key that hides the sidebar and the key that
        // pins a file, so `cargo test` wrote into the REAL configuration of
        // whoever ran it — and left a `/tmp` path pinned and `showing: false`.
        // The symptom was a file manager with no sidebar and nothing anywhere
        // saying why, on a build that had the feature compiled in.
        //
        // It is the same rule `trash::Bin` follows and states: every root is a
        // PARAMETER. A test that does not name a store must not be able to
        // find one.
        let tree = Tree::fresh("no-store");
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        assert!(side.store_dir().is_none());

        side.toggle();
        side.add(Item {
            id: "file:/tmp/anything.zip".to_owned(),
            target: Target::File(PathBuf::from("/tmp/anything.zip")),
            label: None,
        });
        side.save()
            .expect("saving with no destination is not an error");
        assert!(
            side.stamp().is_none(),
            "a bar with no destination has no file"
        );

        // Nothing appeared anywhere it could have appeared.
        assert!(!tree.root.join(store::FILE).exists());
        assert!(std::fs::read_dir(&tree.root).unwrap().next().is_none());
    }

    #[test]
    fn a_sidebar_with_a_store_writes_there_and_only_there() {
        let tree = Tree::fresh("with-store");
        let mut side = Sidebar::of(
            &[],
            None,
            None,
            Preferences::fresh(),
            Some(tree.root.clone()),
        );
        side.toggle();
        side.save().expect("save");
        assert!(tree.root.join(store::FILE).exists());
        assert!(side.stamp().is_some());
        assert!(!store::read_from(&tree.root).showing);
    }

    #[test]
    fn a_round_trip_through_the_file_keeps_what_was_changed() {
        let tree = Tree::fresh("round-trip");
        let mut prefs = Preferences::fresh();
        prefs.added.push(Item {
            id: "app:firefox".to_owned(),
            target: Target::Application("firefox".to_owned()),
            label: Some("Firefox".to_owned()),
        });
        prefs.adjustments.insert(
            "place:main".to_owned(),
            Adjustment {
                hidden: true,
                label: None,
            },
        );
        prefs.showing = false;
        store::write_to(&tree.root, &prefs).unwrap();
        assert_eq!(store::read_from(&tree.root), prefs);
    }

    #[test]
    fn a_missing_file_and_a_broken_one_both_come_up_showing() {
        let tree = Tree::fresh("broken");
        // Missing.
        assert!(store::read_from(&tree.root).showing);
        // Broken. The sidebar must NOT come up hidden — a `bool` defaults to
        // `false`, and somebody whose file got mangled would find the whole
        // sidebar gone with nothing on screen saying why.
        std::fs::write(tree.root.join(store::FILE), "{ not json").unwrap();
        assert!(store::read_from(&tree.root).showing);
        // A file that parses but predates the field gets the same answer.
        std::fs::write(tree.root.join(store::FILE), "{}").unwrap();
        assert!(store::read_from(&tree.root).showing);
    }

    #[test]
    fn the_root_directory_is_named_by_itself_and_not_by_nothing() {
        // `/` has no last component, and a row reading `""` is a row nobody can
        // identify.
        assert_eq!(leaf_of(Path::new("/")), "/");
        assert_eq!(leaf_of(Path::new("/home/x/main")), "main");
    }
}
