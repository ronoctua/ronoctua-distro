//! The `/private` vault — the one crate that runs `cryptsetup`, `losetup`
//! and `systemd-cryptenroll` for it.
//!
//! # The layout
//!
//! ```text
//! @private (btrfs subvolume, NOCOW)  → /var/lib/ronoctua-private/private.img   sparse, grows and shrinks
//!   loop (--direct-io=on) → LUKS2 aes-xts-plain64 512, argon2id → btrfs (discard=async) → /private
//! /private                            0000, root, chattr +i while locked: nothing lands there in clear
//! /run/ronoctua-private/status.json   0644: what the doors read (never a stat of /private on a loop)
//! ```
//!
//! No fstab or crypttab entry, ever: it opens only when the person asks.
//!
//! # It grows and it shrinks
//!
//! The image is a SPARSE file as long as the cap: it takes on the disk what
//! the vault holds, not what it may hold. The inner btrfs is mounted
//! `discard=async`, the mapping lets discards through (a persistent LUKS2
//! flag) and the loop device turns a discard into a hole punched in the image
//! — so deleting inside `/private` gives the space back to the machine.
//!
//! # Root, through the sudo door
//!
//! The person's gesture stages a [`Request`] (and its secrets, `0600`
//! files, never argv) in a `0700` folder they own; `RootVerb::PrivateRoot`
//! starts the fixed unit [`ROOT_UNIT`], which runs `ronoctua-private-root`:
//! it validates the request and does that one [`Op`] ([`root`]). Secrets
//! reach cryptsetup on stdin (`--key-file=-`), or as a `0600` file of root's
//! own for the two programs that read only files. Whatever it decided goes
//! into [`VERDICT_FILE`], which the door reads to say WHY (a wrong
//! passphrase, no key that answered, something holding `/private`). At
//! suspend, [`LOCK_UNIT`] closes it — or `luksSuspend`s it while it is in
//! use, the key wiped and its I/O frozen until the passphrase resumes it.
//!
//! # Never on a loop
//!
//! After a `luksSuspend` anything that touches `/private` waits in the
//! kernel until the vault is resumed. A door's event loop that did would
//! freeze, with the passphrase question it needs on the same loop: so a
//! listing asks [`may_touch`] — where the path lands once its links are
//! followed ([`inside`], which never looks inside the vault itself), and one
//! read of the status file, which root rewrites BEFORE it suspends — and
//! leaves `/private` alone unless the vault is open. What it then reads
//! inside, it reads on a thread it waits for a moment only
//! (`ronoctua_files::listing`).

#![forbid(unsafe_code)]

pub mod root;

use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The root unit `RootVerb::PrivateRoot` starts.
pub const ROOT_UNIT: &str = "ronoctua-private-root.service";

/// The unit that locks the vault before sleep.
pub const LOCK_UNIT: &str = "ronoctua-private-lock.service";

/// The unit that makes `@private` on a machine installed before it existed.
pub const SUBVOLUME_UNIT: &str = "ronoctua-private-subvolume.service";

/// The unit that writes the first status at boot (and again after every
/// switch): which state the vault is in and whether a key is enrolled —
/// the one fact the person cannot read, since the header is root's.
pub const STATUS_UNIT: &str = "ronoctua-private-status.service";

/// Where the plaintext appears while open.
pub const MOUNT_POINT: &str = "/private";

/// Where the image lives (the `@private` subvolume).
pub const IMAGE_DIR: &str = "/var/lib/ronoctua-private";

/// The status the doors read.
pub const STATUS_FILE: &str = "/run/ronoctua-private/status.json";

/// What the root unit decided last ([`Verdict`]).
pub const VERDICT_FILE: &str = "/run/ronoctua-private/verdict.json";

/// The folder the person stages a request in (`0700`, theirs).
pub const STAGING: &str = "/run/ronoctua-private/staged";

/// The request's file in [`STAGING`].
pub const REQUEST_FILE: &str = "request.json";

/// The device-mapper name of the open vault (`/dev/mapper/ronoctua-private`).
pub const MAPPER: &str = "ronoctua-private";

/// The LUKS2 header's label.
pub const LABEL: &str = "ronoctua-private";

/// The smallest cap: the header, btrfs' own floor and room for anything.
pub const MIN_CAP_BYTES: u64 = 1_000_000_000;

/// The largest cap a request may name — a sanity bound, far beyond a disk.
pub const MOST_CAP_BYTES: u64 = 1 << 50;

/// The longest passphrase a staged file may hold, in bytes.
pub const MOST_SECRET_BYTES: usize = 1024;

/// The longest staged request, in bytes.
pub const MOST_REQUEST_BYTES: usize = 4096;

/// The one format of [`Request`], [`Status`] and [`Verdict`].
pub const FORMAT: u32 = 1;

/// Why the vault did not do what was asked, as a closed set
/// (`private-refused-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// There is no vault yet.
    Absent,
    /// There is one already.
    Exists,
    /// The passphrase does not open it.
    WrongPassphrase,
    /// No security key answered.
    NoKey,
    /// Something holds `/private` open.
    Busy,
    /// The disk has not the room the cap asks for.
    NoRoom,
    /// The staged request is not what it must be.
    Invalid,
    /// A program failed.
    Failed,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 8] = [
        Self::Absent,
        Self::Exists,
        Self::WrongPassphrase,
        Self::NoKey,
        Self::Busy,
        Self::NoRoom,
        Self::Invalid,
        Self::Failed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Exists => "exists",
            Self::WrongPassphrase => "wrong-passphrase",
            Self::NoKey => "no-key",
            Self::Busy => "busy",
            Self::NoRoom => "no-room",
            Self::Invalid => "invalid",
            Self::Failed => "failed",
        }
    }

    /// The one of this name.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// A refusal for this reason.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self { why }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.why.id())
    }
}

impl std::error::Error for Refusal {}

/// One operation the root unit does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "op"
)]
pub enum Op {
    /// Make the vault (the passphrase is a staged secret).
    Create {
        /// Its ceiling, in bytes.
        cap_bytes: u64,
    },
    /// Open and mount it (passphrase staged, or a security key).
    Open,
    /// Unmount and close it.
    Close,
    /// Enrol a FIDO2 key (the passphrase staged unlocks the enrolment).
    EnrollFido2,
    /// Forget the enrolled key.
    ForgetFido2,
    /// A new passphrase (old and new staged).
    Passphrase,
    /// A new ceiling.
    Grow {
        /// The new ceiling, in bytes.
        cap_bytes: u64,
    },
}

/// Whether an operation takes a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Need {
    /// It takes none, and one staged is refused.
    No,
    /// It may take one (opening: a passphrase, or the enrolled key).
    May,
    /// It cannot run without one.
    Must,
}

impl Op {
    /// Its word, as the verdict and the journal say it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Create { .. } => "create",
            Self::Open => "open",
            Self::Close => "close",
            Self::EnrollFido2 => "enroll-fido2",
            Self::ForgetFido2 => "forget-fido2",
            Self::Passphrase => "passphrase",
            Self::Grow { .. } => "grow",
        }
    }

    /// Whether it takes the passphrase ([`Slot::Passphrase`]): the vault's
    /// new one for `Create`, the one in force for everything else. A grow
    /// MAY take it: an open vault's mapping is grown with it, a locked one
    /// grows into its new length at its next opening.
    #[must_use]
    pub const fn passphrase(self) -> Need {
        match self {
            Self::Create { .. } | Self::EnrollFido2 | Self::Passphrase => Need::Must,
            Self::Open | Self::Grow { .. } => Need::May,
            Self::Close | Self::ForgetFido2 => Need::No,
        }
    }

    /// Whether it takes a new passphrase ([`Slot::NewPassphrase`]).
    #[must_use]
    pub const fn new_passphrase(self) -> Need {
        match self {
            Self::Passphrase => Need::Must,
            _ => Need::No,
        }
    }

    /// What a slot needs for this operation.
    #[must_use]
    pub const fn needs(self, slot: Slot) -> Need {
        match slot {
            Slot::Passphrase => self.passphrase(),
            Slot::NewPassphrase => self.new_passphrase(),
        }
    }
}

/// What the person stages for the root unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Request {
    /// Always [`FORMAT`].
    pub format: u32,
    /// What to do.
    pub op: Op,
}

impl Request {
    /// A request of this format for `op`.
    #[must_use]
    pub const fn of(op: Op) -> Self {
        Self { format: FORMAT, op }
    }

    /// Whether it is one the root unit takes: this format, and a cap inside
    /// [`MIN_CAP_BYTES`]..=[`MOST_CAP_BYTES`].
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`].
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.format != FORMAT {
            return Err(Refusal::of(Why::Invalid));
        }
        match self.op {
            Op::Create { cap_bytes } | Op::Grow { cap_bytes }
                if !(MIN_CAP_BYTES..=MOST_CAP_BYTES).contains(&cap_bytes) =>
            {
                Err(Refusal::of(Why::Invalid))
            }
            _ => Ok(()),
        }
    }
}

/// A secret the person stages beside the request, one `0600` file each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The passphrase: the one in force, or a new vault's.
    Passphrase,
    /// The passphrase that replaces it.
    NewPassphrase,
}

impl Slot {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Passphrase, Self::NewPassphrase];

    /// Its file in the staging folder.
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::Passphrase => "passphrase",
            Self::NewPassphrase => "new-passphrase",
        }
    }
}

/// Where the vault is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// There is none.
    Absent,
    /// Closed.
    Locked,
    /// Open and mounted at `/private`.
    Open,
    /// Suspended while in use at sleep: its key is wiped and its I/O frozen.
    SuspendedInUse,
}

impl State {
    /// All four.
    pub const ALL: [Self; 4] = [Self::Absent, Self::Locked, Self::Open, Self::SuspendedInUse];
}

/// What `status.json` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Status {
    /// Always [`FORMAT`].
    pub format: u32,
    /// Where it is.
    pub state: State,
    /// Its ceiling, in bytes.
    pub cap_bytes: u64,
    /// What the image looks like it weighs (`du --apparent-size`): its
    /// length, which is the cap.
    pub apparent_bytes: u64,
    /// What it really takes on the disk (its blocks): what the vault holds,
    /// give or take btrfs' own.
    pub real_bytes: u64,
    /// Whether a security key is enrolled.
    pub fido2: bool,
}

impl Status {
    /// The status of a machine with no vault.
    #[must_use]
    pub const fn absent() -> Self {
        Self {
            format: FORMAT,
            state: State::Absent,
            cap_bytes: 0,
            apparent_bytes: 0,
            real_bytes: 0,
            fido2: false,
        }
    }
}

/// What the root unit decided last: written whole after every request, so
/// the door that started it can say why it was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Verdict {
    /// Always [`FORMAT`].
    pub format: u32,
    /// The operation's word ([`Op::word`]).
    pub op: String,
    /// `None` when it was done.
    pub why: Option<Why>,
    /// When, in seconds since the epoch.
    pub at_s: i64,
}

// ─── the person's side: staging ─────────────────────────────────────────────

/// Writes `bytes` as `staging/<name>`, `0600` from its creation and never
/// through a link, after taking away whatever was there.
fn write_fresh(staging: &Path, name: &str, bytes: &[u8]) -> Result<(), Refusal> {
    let path = staging.join(name);
    wipe(&path);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(&path)
        .map_err(|_| Refusal::of(Why::Failed))?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        wipe(&path);
        return Err(Refusal::of(Why::Failed));
    }
    Ok(())
}

/// Overwrites a file with zeros and removes it — never through a link, and a
/// file that is not there is no error. The staging folder is a `tmpfs`: what
/// was written there stays in memory until it is overwritten.
///
/// ONLY A FILE WITH ONE NAME is overwritten: root wipes what a person
/// staged, and a second name planted for a file of somebody else's would
/// otherwise have root zero that file. Its name is removed either way.
pub fn wipe(path: &Path) {
    let opened = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK)
                .bits()
                .cast_signed(),
        )
        .open(path);
    if let Ok(mut file) = opened
        && let Ok(meta) = file.metadata()
        && meta.is_file()
        && meta.nlink() == 1
    {
        let zeros = vec![0u8; usize::try_from(meta.len()).unwrap_or(0).min(1 << 20)];
        let _ = file.write_all(&zeros).and_then(|()| file.sync_all());
    }
    let _ = std::fs::remove_file(path);
}

/// Stages a request in `staging` (`0600` files in the person's `0700`
/// folder) for the root unit. A request staged before it — one sudo refused
/// — is replaced.
///
/// # Errors
///
/// [`Why::Invalid`] for a request the root unit would refuse;
/// [`Why::Failed`] when the folder cannot be written (a machine without the
/// vault's units has no such folder).
pub fn stage(staging: &Path, request: &Request) -> Result<(), Refusal> {
    request.validate()?;
    let text = serde_json::to_vec(request).map_err(|_| Refusal::of(Why::Failed))?;
    write_fresh(staging, REQUEST_FILE, &text)
}

/// Stages a secret beside the request.
///
/// # Errors
///
/// [`Why::Invalid`] for an empty one or one longer than
/// [`MOST_SECRET_BYTES`]; [`Why::Failed`] when it cannot be written.
pub fn stage_secret(staging: &Path, slot: Slot, secret: &[u8]) -> Result<(), Refusal> {
    if secret.is_empty() || secret.len() > MOST_SECRET_BYTES {
        return Err(Refusal::of(Why::Invalid));
    }
    write_fresh(staging, slot.file(), secret)
}

/// Takes everything staged away, the secrets wiped first — what a door does
/// after the root unit ran, or after sudo refused and it never did.
pub fn unstage(staging: &Path) {
    for slot in Slot::ALL {
        wipe(&staging.join(slot.file()));
    }
    wipe(&staging.join(REQUEST_FILE));
}

// ─── the person's side: reading ─────────────────────────────────────────────

/// Reads at most `most` bytes of a file, never through a link.
fn read_small(path: &Path, most: usize) -> std::io::Result<Vec<u8>> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(path)?;
    let mut out = Vec::new();
    file.take(most as u64 + 1).read_to_end(&mut out)?;
    if out.len() > most {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(out)
}

/// Reads the status file. `Ok(None)` when there is none: the vault's units
/// are not on this machine (the nested loop, a machine without the module).
///
/// One small read on `/run` — what a door may do on its loop, and the ONE
/// thing it may do about the vault there.
///
/// # Errors
///
/// [`Why::Invalid`] for a file that is there and is not a status.
pub fn status(file: &Path) -> Result<Option<Status>, Refusal> {
    match read_small(file, MOST_REQUEST_BYTES) {
        Ok(bytes) => serde_json::from_slice::<Status>(&bytes)
            .ok()
            .filter(|s| s.format == FORMAT)
            .map(Some)
            .ok_or(Refusal::of(Why::Invalid)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Refusal::of(Why::Invalid)),
    }
}

/// The status with what the image takes NOW: its length and its blocks, read
/// with a `stat` of the image — which lives on the machine's own disk, never
/// inside `/private`, so it cannot wait on a suspended vault. The image's
/// folder is `0711`: its names cannot be listed, the known one can be
/// stat'ed. Without the image the status is as it was.
#[must_use]
pub fn weighed(mut status: Status, image: &Path) -> Status {
    if let Ok(meta) = std::fs::symlink_metadata(image)
        && meta.is_file()
    {
        status.apparent_bytes = meta.len();
        status.real_bytes = meta.blocks().saturating_mul(512);
    }
    status
}

/// The last verdict, when there is one that reads.
#[must_use]
pub fn verdict(file: &Path) -> Option<Verdict> {
    let bytes = read_small(file, MOST_REQUEST_BYTES).ok()?;
    serde_json::from_slice::<Verdict>(&bytes)
        .ok()
        .filter(|v| v.format == FORMAT)
}

/// Why the root unit refused `op` started at or after `since_s` — what the
/// door says instead of "it did not work". `None` when the verdict is
/// another request's, older, or says it was done.
#[must_use]
pub fn refused_since(file: &Path, op: Op, since_s: i64) -> Option<Why> {
    let v = verdict(file)?;
    (v.op == op.word() && v.at_s >= since_s).then_some(v.why)?
}

/// Whether a door may touch `path` — list it, stat it, watch it — now.
///
/// Anything outside `/private` may be touched. Inside — once its links are
/// followed ([`inside`]) — only while the vault is OPEN: a locked vault is an
/// empty immutable folder, and a suspended one would freeze whoever touched
/// it until the passphrase resumes it.
#[must_use]
pub fn may_touch(path: &Path) -> bool {
    may_touch_with(path, Path::new(MOUNT_POINT), Path::new(STATUS_FILE))
}

/// [`may_touch`] with its two roots handed in. Reads one status file, and
/// `lstat`s the components of `path` that are outside the vault.
#[must_use]
pub fn may_touch_with(path: &Path, mount_point: &Path, status_file: &Path) -> bool {
    !inside_with(path, mount_point) || is_open(status_file)
}

/// Whether the status file says the vault is open — the one state in which
/// what is under `/private` may be read.
#[must_use]
pub fn is_open(status_file: &Path) -> bool {
    matches!(status(status_file), Ok(Some(s)) if s.state == State::Open)
}

/// [`is_open`], of this machine's status file.
#[must_use]
pub fn open_now() -> bool {
    is_open(Path::new(STATUS_FILE))
}

/// The most links one resolution follows — the kernel's own bound (`ELOOP`).
const MOST_LINKS: usize = 40;

/// Whether `path` lands inside `/private` once its links are followed:
/// `~/Private -> /private`, `/home/p/../../private/notes` and a relative
/// path from a working folder in it all do.
///
/// FOUND WITHOUT TOUCHING THE VAULT. The path is walked a component at a
/// time, from `/`, each link read and spliced in where it stands; the walk
/// stops — inside — the moment the next name would BE the mount point, so
/// nothing at or under `/private` is ever `lstat`ed or read: a suspended
/// vault cannot freeze the walk that asks about it. What is outside costs
/// one `lstat` per component (and a `readlink` per link), as a listing
/// does.
///
/// CAREFUL BOTH WAYS: a walk that passes through the vault counts as inside
/// even when a later `..` would leave it, and a link that cannot be read is
/// taken for one into the vault. A loop of links is outside: the kernel
/// refuses it (`ELOOP`) before reading anything through it.
#[must_use]
pub fn inside(path: &Path) -> bool {
    inside_with(path, Path::new(MOUNT_POINT))
}

/// [`inside`], with the mount point handed in.
#[must_use]
pub fn inside_with(path: &Path, mount_point: &Path) -> bool {
    use std::path::Component;

    /// One step of the walk.
    enum Part {
        Root,
        Up,
        Name(std::ffi::OsString),
    }

    /// `p`'s steps, the first one LAST: the walk pops them.
    fn parts(p: &Path) -> Vec<Part> {
        p.components()
            .rev()
            .filter_map(|c| match c {
                Component::RootDir => Some(Part::Root),
                Component::ParentDir => Some(Part::Up),
                Component::Normal(n) => Some(Part::Name(n.to_os_string())),
                Component::CurDir | Component::Prefix(_) => None,
            })
            .collect()
    }

    let mut todo = parts(path);
    if !path.has_root() {
        // A RELATIVE PATH starts where this process stands — which may be
        // inside the vault, and the walk then says so.
        match std::env::current_dir() {
            Ok(here) => todo.extend(parts(&here)),
            Err(_) => return true,
        }
    }
    let mut here = PathBuf::from("/");
    let mut links = 0;
    while let Some(part) = todo.pop() {
        match part {
            Part::Root => here = PathBuf::from("/"),
            // `here` holds no link (each was resolved on the way), so going
            // up it is taking its last name off.
            Part::Up => {
                here.pop();
            }
            Part::Name(name) => {
                let next = here.join(&name);
                if next.starts_with(mount_point) {
                    return true;
                }
                match std::fs::symlink_metadata(&next) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        links += 1;
                        if links > MOST_LINKS {
                            return false;
                        }
                        match std::fs::read_link(&next) {
                            // Relative, it goes on from the link's folder;
                            // absolute, its `Root` starts over.
                            Ok(target) => todo.extend(parts(&target)),
                            Err(_) => return true,
                        }
                    }
                    // A FOLDER, A FILE OR NOTHING AT ALL: walked on by name.
                    _ => here = next,
                }
            }
        }
    }
    here.starts_with(mount_point)
}

/// The progress word the security helper says once sudo let the person
/// through, right before it starts [`ROOT_UNIT`]: from that moment an
/// opening by the enrolled key waits for a touch, and the door says so —
/// never before the password question that may come first.
pub const STARTED: &str = "private-started";

/// How long an opening by the enrolled key waits for its touch
/// (`token-timeout` in [`root::FIDO2_OPTIONS`]) — what a door holds "touch
/// your key" for, until the work's own answer takes its place.
pub const TOUCH_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// The root side: validates and runs the staged request with the programs
/// the unit's environment names, keeping its verdict and the new status
/// ([`root::Machine::run_staged`]).
///
/// # Errors
///
/// Why it was refused.
pub fn run_staged(staging: &Path) -> Result<(), Refusal> {
    root::Machine::system(root::Tools::from_env()).run_staged(staging)
}

/// The image's path under the image folder.
#[must_use]
pub fn image_of(dir: &Path) -> PathBuf {
    dir.join("private.img")
}

// ─── sizes ──────────────────────────────────────────────────────────────────

/// One decimal gigabyte: the unit a size is shown in (`sizes::readable_bytes`
/// writes `64.4 GB`), and so the one a size is typed in.
pub const GB: u64 = 1_000_000_000;

/// A size as a person types it: `64`, `64G`, `64 GB`, `1.5T`, `1,5 TB`,
/// `512M` — DECIMAL units, as every size on screen is shown, gigabytes when
/// none is named; `GiB`, `TiB` and `MiB` when the person says binary. `None`
/// for anything else, zero, and what does not fit in 64 bits. Pure.
#[must_use]
pub fn parse_size(text: &str) -> Option<u64> {
    let squeezed: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let split = squeezed
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(squeezed.len());
    let (number, unit) = squeezed.split_at(split);
    let unit_bytes: u128 = match unit {
        "" | "g" | "gb" => 1_000_000_000,
        "t" | "tb" => 1_000_000_000_000,
        "m" | "mb" => 1_000_000,
        "gib" => 1 << 30,
        "tib" => 1 << 40,
        "mib" => 1 << 20,
        _ => return None,
    };
    let (whole, fraction) = match number.split_once(['.', ',']) {
        Some((w, f)) => (w, f),
        None => (number, ""),
    };
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction = fraction.get(..fraction.len().min(9))?;
    let scale = 10u128.pow(u32::try_from(fraction.len()).ok()?);
    let part: u128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().ok()?
    };
    let bytes = whole
        .checked_mul(unit_bytes)?
        .checked_add(part.checked_mul(unit_bytes)? / scale)?;
    u64::try_from(bytes).ok().filter(|b| *b > 0)
}

/// The cap a new vault is offered: 64 GB, or half of what is free when that
/// is less — whole gigabytes, never under [`MIN_CAP_BYTES`]. Pure.
#[must_use]
pub fn suggested_cap(free_bytes: u64) -> u64 {
    const OFFERED: u64 = 64 * GB;
    let half = free_bytes / 2 / GB * GB;
    OFFERED.min(half).max(MIN_CAP_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-private-{name}-{}-{n}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// FAILS IF: a request stops crossing to the root side and back, or a
    /// cap outside the bounds, another format or an unknown field is taken.
    #[test]
    fn a_request_crosses_and_a_bad_one_is_refused() {
        for op in [
            Op::Create {
                cap_bytes: 64 << 30,
            },
            Op::Open,
            Op::Close,
            Op::EnrollFido2,
            Op::ForgetFido2,
            Op::Passphrase,
            Op::Grow {
                cap_bytes: 128 << 30,
            },
        ] {
            let request = Request::of(op);
            let text = serde_json::to_string(&request).unwrap();
            assert_eq!(serde_json::from_str::<Request>(&text).unwrap(), request);
            assert_eq!(request.validate(), Ok(()), "{text}");
        }
        assert_eq!(
            serde_json::to_string(&Request::of(Op::Create { cap_bytes: 1 << 30 })).unwrap(),
            r#"{"format":1,"op":{"op":"create","cap-bytes":1073741824}}"#
        );
        for cap in [0, MIN_CAP_BYTES - 1, MOST_CAP_BYTES + 1] {
            assert_eq!(
                Request::of(Op::Create { cap_bytes: cap }).validate(),
                Err(Refusal::of(Why::Invalid))
            );
            assert_eq!(
                Request::of(Op::Grow { cap_bytes: cap }).validate(),
                Err(Refusal::of(Why::Invalid))
            );
        }
        let newer = Request {
            format: FORMAT + 1,
            op: Op::Open,
        };
        assert_eq!(newer.validate(), Err(Refusal::of(Why::Invalid)));
        assert!(
            serde_json::from_str::<Request>(r#"{"format":1,"op":{"op":"open"},"then":"rm"}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Request>(r#"{"format":1,"op":{"op":"format"}}"#).is_err());
        assert_eq!(
            image_of(Path::new(IMAGE_DIR)),
            Path::new("/var/lib/ronoctua-private/private.img")
        );
    }

    /// FAILS IF: what each operation needs drifts — opening stops taking a
    /// passphrase OR the key, a change of passphrase stops needing both.
    #[test]
    fn each_operation_says_what_it_needs() {
        assert_eq!(Op::Open.passphrase(), Need::May);
        assert_eq!(Op::Create { cap_bytes: 0 }.passphrase(), Need::Must);
        assert_eq!(Op::EnrollFido2.passphrase(), Need::Must);
        assert_eq!(Op::Passphrase.needs(Slot::Passphrase), Need::Must);
        assert_eq!(Op::Passphrase.needs(Slot::NewPassphrase), Need::Must);
        assert_eq!(Op::Grow { cap_bytes: 0 }.passphrase(), Need::May);
        for op in [Op::Close, Op::ForgetFido2, Op::Grow { cap_bytes: 0 }] {
            assert_eq!(op.new_passphrase(), Need::No, "{op:?}");
        }
        for op in [Op::Close, Op::ForgetFido2] {
            assert_eq!(op.passphrase(), Need::No, "{op:?}");
        }
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
        }
    }

    /// FAILS IF: a staged file stops being `0600` from its first byte, a
    /// second staging fails on the first one's leftovers, a link is followed,
    /// or unstaging leaves a secret behind.
    #[test]
    fn staging_is_private_replaces_and_is_taken_away() {
        let s = Scratch::new("stage");
        let request = Request::of(Op::Open);
        stage(&s.0, &request).unwrap();
        stage_secret(&s.0, Slot::Passphrase, b"correct horse").unwrap();
        // AGAIN, over the leftovers of a run sudo refused.
        stage(&s.0, &request).unwrap();
        stage_secret(&s.0, Slot::Passphrase, b"battery staple").unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().mode() & 0o777;
        assert_eq!(mode(&s.0.join(REQUEST_FILE)), 0o600);
        assert_eq!(mode(&s.0.join("passphrase")), 0o600);
        assert_eq!(
            std::fs::read(s.0.join("passphrase")).unwrap(),
            b"battery staple"
        );
        assert_eq!(
            stage_secret(&s.0, Slot::NewPassphrase, b""),
            Err(Refusal::of(Why::Invalid))
        );
        assert_eq!(
            stage_secret(&s.0, Slot::NewPassphrase, &[b'x'; MOST_SECRET_BYTES + 1]),
            Err(Refusal::of(Why::Invalid))
        );
        assert_eq!(
            stage(&s.0, &Request::of(Op::Grow { cap_bytes: 1 })),
            Err(Refusal::of(Why::Invalid))
        );
        unstage(&s.0);
        assert_eq!(std::fs::read_dir(&s.0).unwrap().count(), 0, "left behind");
        // A LINK planted where a secret goes is taken away, never written
        // through.
        let target = s.0.join("elsewhere");
        std::fs::write(&target, b"keep").unwrap();
        std::os::unix::fs::symlink(&target, s.0.join("passphrase")).unwrap();
        stage_secret(&s.0, Slot::Passphrase, b"secret").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
        assert!(
            !std::fs::symlink_metadata(s.0.join("passphrase"))
                .unwrap()
                .is_symlink()
        );
    }

    /// FAILS IF: a missing status stops being "not on this machine", a broken
    /// one is taken for a vault, or a listing may touch `/private` while the
    /// vault is anything but open.
    #[test]
    fn the_status_is_read_and_guards_the_mount_point() {
        let s = Scratch::new("status");
        let file = s.0.join("status.json");
        assert_eq!(status(&file), Ok(None));
        std::fs::write(&file, b"{\"format\":1}").unwrap();
        assert_eq!(status(&file), Err(Refusal::of(Why::Invalid)));
        let mount = Path::new("/private");
        let inside = Path::new("/private/notes/a.txt");
        assert!(may_touch_with(Path::new("/home/p"), mount, &file));
        assert!(!may_touch_with(inside, mount, &file), "a broken status");
        assert!(
            may_touch_with(Path::new("/privateer"), mount, &file),
            "a sibling with the same prefix"
        );
        for state in State::ALL {
            let written = Status {
                state,
                ..Status::absent()
            };
            std::fs::write(&file, serde_json::to_vec(&written).unwrap()).unwrap();
            assert_eq!(status(&file), Ok(Some(written)));
            assert_eq!(
                may_touch_with(inside, mount, &file),
                state == State::Open,
                "{state:?}"
            );
            assert_eq!(may_touch_with(mount, mount, &file), state == State::Open);
        }
        assert_eq!(
            serde_json::to_string(&Status::absent()).unwrap(),
            r#"{"format":1,"state":"absent","cap-bytes":0,"apparent-bytes":0,"real-bytes":0,"fido2":false}"#
        );
    }

    /// FAILS IF: a link, a `..` or a relative path reaches the vault without
    /// being taken for it — or the walk that asks looks inside the vault
    /// (a link planted under the mount point would then send it elsewhere).
    #[test]
    fn links_and_dots_into_the_vault_are_inside() {
        let s = Scratch::new("inside");
        let mount = s.0.join("private");
        let home = s.0.join("home");
        std::fs::create_dir_all(&mount).unwrap();
        std::fs::create_dir_all(home.join("notes")).unwrap();
        let link = |at: &Path, to: &Path| std::os::unix::fs::symlink(to, at).unwrap();
        link(&home.join("Private"), &mount);
        link(&home.join("relative"), Path::new("../private"));
        link(&home.join("chain"), &home.join("Private"));
        link(&home.join("loop"), &home.join("loop"));
        link(&home.join("elsewhere"), &home.join("notes"));
        // UNDER THE MOUNT POINT, a way out: walked into, it would say
        // "outside". Never walked into, it changes nothing.
        link(&mount.join("out"), &home);
        let inside = |p: &Path| inside_with(p, &mount);
        for p in [
            mount.clone(),
            mount.join("a/b.txt"),
            home.join("Private"),
            home.join("Private/notes"),
            home.join("relative/x"),
            home.join("chain"),
            home.join("../private"),
            home.join("notes/../../private/x"),
            mount.join("out"),
            mount.join("out/notes"),
        ] {
            assert!(inside(&p), "{} is the vault", p.display());
        }
        for p in [
            home.clone(),
            home.join("notes"),
            home.join("elsewhere"),
            home.join("loop"),
            home.join("missing/../notes"),
            s.0.join("privateer"),
            Path::new("/").to_path_buf(),
        ] {
            assert!(!inside(&p), "{} is not the vault", p.display());
        }
        // AND THE GUARD READS IT SO: a link into a locked vault is refused.
        let status_file = s.0.join("status.json");
        let write = |state| {
            let st = Status {
                state,
                ..Status::absent()
            };
            std::fs::write(&status_file, serde_json::to_vec(&st).unwrap()).unwrap();
        };
        write(State::SuspendedInUse);
        assert!(!may_touch_with(&home.join("Private"), &mount, &status_file));
        assert!(may_touch_with(&home.join("notes"), &mount, &status_file));
        write(State::Open);
        assert!(may_touch_with(&home.join("Private"), &mount, &status_file));
        assert!(is_open(&status_file));
    }

    /// FAILS IF: an image's weight stops being its length and its blocks — a
    /// sparse file is not as heavy as it is long.
    #[test]
    fn a_sparse_image_weighs_what_it_holds() {
        let s = Scratch::new("weigh");
        let image = image_of(&s.0);
        let file = std::fs::File::create(&image).unwrap();
        file.set_len(64 << 20).unwrap();
        drop(file);
        let w = weighed(Status::absent(), &image);
        assert_eq!(w.apparent_bytes, 64 << 20);
        assert!(w.real_bytes < 64 << 20, "{w:?}");
        assert_eq!(
            weighed(Status::absent(), &s.0.join("none")),
            Status::absent()
        );
    }

    /// FAILS IF: a verdict is said for another operation or an older run.
    #[test]
    fn a_verdict_is_said_only_for_its_own_run() {
        let s = Scratch::new("verdict");
        let file = s.0.join("verdict.json");
        assert_eq!(refused_since(&file, Op::Open, 0), None);
        let v = Verdict {
            format: FORMAT,
            op: "open".into(),
            why: Some(Why::WrongPassphrase),
            at_s: 100,
        };
        std::fs::write(&file, serde_json::to_vec(&v).unwrap()).unwrap();
        assert_eq!(
            refused_since(&file, Op::Open, 99),
            Some(Why::WrongPassphrase)
        );
        assert_eq!(refused_since(&file, Op::Open, 101), None, "older");
        assert_eq!(refused_since(&file, Op::Close, 0), None, "another op");
        assert_eq!(verdict(&file), Some(v));
    }

    /// FAILS IF: a typed size stops reading as a person writes it, or a
    /// nonsense one is taken for a size.
    #[test]
    fn sizes_read_as_people_type_them() {
        const G: u64 = GB;
        assert_eq!(parse_size("64"), Some(64 * G));
        assert_eq!(parse_size("64G"), Some(64 * G));
        assert_eq!(parse_size(" 64 GB "), Some(64 * G));
        assert_eq!(parse_size("64 GiB"), Some(64 << 30));
        assert_eq!(parse_size("1.5T"), Some(1500 * G));
        assert_eq!(parse_size("1,5 tb"), Some(1500 * G));
        assert_eq!(parse_size("1TiB"), Some(1 << 40));
        assert_eq!(parse_size("512M"), Some(512_000_000));
        assert_eq!(parse_size(".5"), Some(G / 2));
        for bad in [
            "",
            "G",
            "0",
            "0G",
            "-1",
            "64X",
            "1.2.3",
            "1e9",
            "99999999999T",
        ] {
            assert_eq!(parse_size(bad), None, "{bad:?}");
        }
        assert_eq!(suggested_cap(1 << 40), 64 * G);
        assert_eq!(suggested_cap(21 * G + 7), 10 * G);
        assert_eq!(suggested_cap(0), MIN_CAP_BYTES);
    }
}
