//! The machine's disks — the only crate that speaks to udisks2.
//!
//! # Why a crate of its own
//!
//! The same reason `ronoctua-scroll` is the only one that speaks the compositor's IPC,
//! `ronoctua-network` the only one that speaks NetworkManager, `ronoctua-memory`
//! the only one that reads `/proc` and `ronoctua-files` the only one that
//! touches the filesystem: **one border, one crate**. When two parts of the
//! system ask "is this stick plugged in?" of different places, the day comes
//! when they disagree and nobody knows which to believe.
//!
//! # The split that makes it testable
//!
//! [`udisks`] is the only module that knows D-Bus exists. Everything here —
//! what counts as a volume and what is only a holder, which rows are quiet, how
//! a filesystem finds the disk it sits on — is a pure function over [`Volume`],
//! and its tests run against nine rows copied off a real machine with a pendrive
//! in it. No bus, no device, no root, no virtual machine.
//!
//! # What it does NOT do
//!
//! It does not draw, does not translate, does not decide what a person sees. It
//! answers with a [`Rack`]; what turns that into a row, a colour and a sentence
//! is the file manager's sidebar, which does not know D-Bus exists. And it
//! reads no filesystem: how full a mounted volume is comes from
//! `ronoctua_memory::machine::filesystem`, which already does that `statvfs`
//! for the memory panel.
//!
//! # It never runs a program
//!
//! Not one `Command::new`. Everything is the bus, which is why there is no
//! `option_env!` here and no `disks-runs-by-path` check to go with it — that
//! absence is a decision, and `nix/modules/disks.nix` says so.
//!
//! # An encrypted disk that is not the system's
//!
//! A LUKS container udisks does not mark `HintSystem` — a stick, an external
//! drive — is a row of its own while it is LOCKED ([`Encrypted::Locked`]):
//! the place one presses to be asked the passphrase. While it is open the
//! container is not a row (`Skipped::Unlocked`) and what came out of it takes
//! its place. The system's own (the encrypted root) stays what it was, a
//! holder nobody opens from here. [`Manager::unlock`], [`Manager::lock`] and
//! [`Manager::put_away`] are the three calls; a passphrase is a
//! [`Secret`](ronoctua_consent::keys::Secret) and crosses only the bus.

#![forbid(unsafe_code)]

pub mod udisks;

use anyhow::{Context, Result};
use std::collections::HashMap;

/// How a drive is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bus {
    /// A stick, an external enclosure.
    Usb,
    /// An SD card in a reader.
    Sdio,
    /// FireWire, and it is here because udisks reports it.
    Ieee1394,
    /// Soldered or screwed in: SATA, NVMe.
    #[default]
    Internal,
    /// udisks said something this does not know. It is not an error: a bus
    /// nobody wrote down is still a disk one can open.
    Unknown,
}

impl Bus {
    /// From `Drive.ConnectionBus`, which is empty for anything internal.
    #[must_use]
    pub fn of(name: &str) -> Self {
        match name {
            "usb" => Self::Usb,
            "sdio" => Self::Sdio,
            "ieee1394" => Self::Ieee1394,
            "" => Self::Internal,
            _ => Self::Unknown,
        }
    }
}

/// The physical thing volumes sit on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Drive {
    /// The object path. It is the KEY and nothing else: udisks embeds the
    /// drive's SERIAL NUMBER in it, and this repository is public
    /// (invariant 13). It is never drawn and never written down.
    pub at: String,
    /// `SanDisk`.
    pub vendor: String,
    /// `Cruzer Blade`.
    pub model: String,
    /// How big, in bytes.
    pub bytes: u64,
    /// How it is attached.
    pub bus: Bus,
    /// The drive itself can be unplugged.
    pub removable: bool,
    /// It has a tray or a slot to spit something out of.
    pub ejectable: bool,
    /// Its power can be cut, which is what makes a stick safe to pull.
    pub can_power_off: bool,
    /// udisks' own ordering key — `00coldplug/00fixed/nvme0` before
    /// `01hotplug/…`. Taken verbatim rather than re-derived: udisks already
    /// sorted this, and a second ordering is a second truth.
    pub sort_key: String,
    /// `Drive.Serial`, empty when the drive reports none. Like [`Drive::at`]
    /// it is never drawn and never written down (invariant 13): it is what
    /// the installer stick's root writer compares, right before it writes,
    /// to know the drive is still the one the person chose
    /// (`ronoctua-stick`).
    pub serial: String,
}

impl Drive {
    /// What to call it, for a person.
    ///
    /// The two fields joined, and neither on its own: `SanDisk` alone does not
    /// say which of two sticks, and `Cruzer Blade` alone reads like a product
    /// nobody recognises. An empty pair falls back to nothing at all, and the
    /// caller draws the volumes without a heading — a heading that says
    /// `""` is worse than no heading.
    #[must_use]
    pub fn name(&self) -> String {
        match (self.vendor.trim(), self.model.trim()) {
            ("", "") => String::new(),
            ("", m) => m.to_owned(),
            (v, "") => v.to_owned(),
            (v, m) => format!("{v} {m}"),
        }
    }
}

/// Which of udisks' interfaces a block carried.
///
/// Presence is a FACT, and half the filter is built on it: a block with no
/// `Filesystem` has nothing to mount and nowhere to go, whatever its other
/// properties say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Holding {
    /// It has a filesystem — mounted or not.
    pub filesystem: bool,
    /// It has a partition table, so it is a drive's heading and not a place.
    pub partition_table: bool,
    /// It IS a partition, which is the only honest way to tell a partition
    /// from a whole disk.
    ///
    /// The absence of `partition_table` is not the same question: a stick
    /// formatted straight onto the disk with no table has neither interface,
    /// and it is a whole disk. Guessing from the device node's trailing digit
    /// works until `/dev/nvme0n1`, which ends in one and is a disk.
    pub partition: bool,
    /// It is a LUKS container. What one opens is what comes OUT of it.
    pub encrypted: bool,
    /// It is swap.
    pub swap: bool,
    /// It is a loop device: an image somebody attached.
    pub loop_back: bool,
}

/// One block device, as udisks describes it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Volume {
    /// The object path — the key, never drawn.
    pub at: String,
    /// `/dev/sda2`.
    pub node: String,
    /// `MISO_EFI`. Often empty, and that is ordinary.
    pub label: String,
    /// `vfat`, `iso9660`, `squashfs`, `crypto_LUKS`, or empty.
    pub id_type: String,
    /// How big, in bytes.
    pub bytes: u64,
    /// It cannot be written to.
    pub read_only: bool,
    /// `Block.HintSystem` — udisks says this belongs to the machine and not to
    /// the person. It is the SAME property polkit splits on: mounting one of
    /// these is `auth_admin_keep` and mounting anything else is free.
    pub system: bool,
    /// `Block.HintIgnore` — udisks says do not put this in front of a person.
    ///
    /// A FLAG here and not a filter, and that is the whole design: it marks the
    /// ESP, the swap and a stick's little EFI helper, and the owner asked for
    /// `/boot` and both of a pendrive's partitions by name. So it rides the key
    /// that already governs dotfiles instead of deciding on its own —
    /// see [`listed`].
    pub quiet: bool,
    /// Where it is mounted. Empty means it is not.
    pub mounted_at: Vec<String>,
    /// The drive it sits on, when it sits on one directly.
    pub drive: Option<String>,
    /// The LUKS container it came out of, when it did.
    pub behind: Option<String>,
    /// Who attached the loop, when it is one.
    pub set_up_by: Option<u32>,
    /// For a LUKS container: the block that came out of it, while it is open.
    ///
    /// `Encrypted.CleartextDevice`, which udisks answers `/` for a container
    /// that is locked — read here as `None`.
    pub cleartext: Option<String>,
    /// `/etc/fstab` names this device.
    ///
    /// From `Block.Configuration`, which udisks fills with the fstab and
    /// crypttab entries that mention the device. Measured on a real machine it
    /// is a SHARP signal where `system` is a blunt one: it marks the root and
    /// the ESP and nothing else, while `HintSystem` is also true of every snap
    /// image and every internal data partition.
    pub in_fstab: bool,
    /// Which of udisks' interfaces it carried.
    pub holds: Holding,
    /// `Block.Symlinks`: every name udev gave the device under `/dev`
    /// (`/dev/disk/by-id/usb-…`, `/dev/disk/by-path/…`). The stable one is
    /// what a write to a whole stick is pinned to, so a drive renamed between
    /// the question and the write is not written (`ronoctua-stick`).
    pub symlinks: Vec<String>,
}

impl Volume {
    /// Is it mounted right now?
    #[must_use]
    pub fn mounted(&self) -> bool {
        !self.mounted_at.is_empty()
    }

    /// Where to go when somebody opens it, if it is anywhere.
    #[must_use]
    pub fn place(&self) -> Option<&str> {
        self.mounted_at.first().map(String::as_str)
    }

    /// For a LUKS container this system opens — not the system's own, and of
    /// type `crypto_LUKS` —: whether it is locked. `None` for everything else.
    #[must_use]
    pub fn encrypted(&self) -> Option<Encrypted> {
        if !self.holds.encrypted || self.system || self.id_type != "crypto_LUKS" {
            return None;
        }
        Some(if self.cleartext.is_some() {
            Encrypted::Unlocked
        } else {
            Encrypted::Locked
        })
    }

    /// Is this the whole disk rather than one partition of it?
    ///
    /// It decides which questions are worth asking: a whole disk can be given a
    /// fresh partition table, and a partition cannot.
    #[must_use]
    pub const fn whole(&self) -> bool {
        !self.holds.partition
    }

    /// What to call it, for a person.
    ///
    /// The label when it has one, the device node otherwise. Never the object
    /// path, which carries a serial number (invariant 13).
    #[must_use]
    pub fn name(&self) -> &str {
        if self.label.is_empty() {
            &self.node
        } else {
            &self.label
        }
    }
}

/// Whether a LUKS container is open — see [`Volume::encrypted`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encrypted {
    /// Locked: its row is the container, and mounting asks the passphrase.
    Locked,
    /// Open: its row is what came out of it.
    Unlocked,
}

/// The container `v` came out of, when it came out of one this system opens
/// — the answer that marks a row as the open side of an encrypted disk, and
/// the container its ejection locks.
#[must_use]
pub fn container_of<'a>(v: &Volume, all: &'a [Volume]) -> Option<&'a Volume> {
    let behind = v.behind.as_deref()?;
    all.iter()
        .find(|c| c.at == behind)
        .filter(|c| c.encrypted().is_some())
}

/// The seal a ROW shows: a locked container's, or `Unlocked` for what came out
/// of one this system opens. `None` for everything else, the encrypted root's
/// cleartext included.
#[must_use]
pub fn seal_of(v: &Volume, all: &[Volume]) -> Option<Encrypted> {
    v.encrypted()
        .or_else(|| container_of(v, all).map(|_| Encrypted::Unlocked))
}

/// One whole reading of the machine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rack {
    /// Every drive udisks knows.
    pub drives: Vec<Drive>,
    /// Every block device udisks knows, unfiltered. [`listed`] is what decides
    /// which of them is a row; keeping the whole reading means the decision can
    /// change without another trip to the bus.
    pub volumes: Vec<Volume>,
}

/// Types that name a HOLDER or an image, never a place a person keeps files.
///
/// A DENY list and not an allow list, and the bias is deliberate: an unknown
/// filesystem shows up as a row somebody ignores, while an unknown filesystem
/// hidden is a disk that cannot be reached and gives no reason. It is the same
/// call `ronoctua_files::Kind::of_mode` makes when it turns an unrecognised
/// inode into a file rather than dropping the row.
pub const HOLDERS: [&str; 11] = [
    "squashfs",
    "erofs",
    "swap",
    "crypto_LUKS",
    "crypto_TCRYPT",
    "BitLocker",
    "LVM2_member",
    "linux_raid_member",
    "zfs_member",
    "DM_snapshot_cow",
    "",
];

/// Where the running system lives.
///
/// # Why a fixed list is enough here, and why it is not the only signal
///
/// The disko layout this distro installs puts `/`, `/home`, `/nix`,
/// `/persist`, `/var/log` and `/.snapshots` on **one block device** — btrfs
/// subvolumes inside a single LUKS container. So one volume's `MountPoints`
/// carries all six, and one predicate over that vector catches every one of
/// them at once. On a machine laid out some other way the list still catches
/// the paths that matter, and `Volume::in_fstab` catches what it does not.
///
/// COMPARED BY COMPONENT and never by string prefix: `starts_with("/home")`
/// also matches `/home-backup`, which is somebody's data disk.
pub const ESSENTIAL: [&str; 8] = [
    "/", "/boot", "/nix", "/home", "/persist", "/var", "/etc", "/usr",
];

/// Why a volume is one the running system needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Needed {
    /// It is mounted where the system lives — and this is which one.
    At(String),
    /// `/etc/fstab` names it: the administrator declared it mounted at boot.
    InFstab,
    /// It is swap.
    Swap,
}

/// Is this a volume the running system needs? `None` means it is not.
///
/// # Why the answer is a REASON and not a bool
///
/// Because the refusal has to name what decided. "This cannot be unmounted" is
/// a rule nobody can check; "`/boot` is mounted here" is a fact somebody can
/// look at. It is the same reason `mode::explain` returns a key and arguments
/// rather than a sentence.
///
/// # And why it is not a confirmation
///
/// `menu::Action::confirms` exists for what cannot be undone, and its own note
/// says a confirmation on the wrong thing "teaches people to press twice
/// without reading". Unmounting `/` is not a thing to confirm. It is a thing
/// not to offer.
#[must_use]
pub fn needed(v: &Volume) -> Option<Needed> {
    // THE MOST SPECIFIC REASON FIRST, and that is what the order is for. The
    // root is both mounted at `/` and named in fstab; answering `InFstab`
    // would be true and useless, because the sentence it produces names no
    // place. `At("/")` names the one fact somebody can go and look at.
    if let Some(at) = v
        .mounted_at
        .iter()
        .find(|at| ESSENTIAL.iter().any(|e| same_place(at, e)))
    {
        return Some(Needed::At(at.clone()));
    }
    if v.in_fstab {
        return Some(Needed::InFstab);
    }
    v.holds.swap.then_some(Needed::Swap)
}

/// Are these the same place, component by component?
///
/// `/home` is not `/home-backup`, and a string prefix says it is. The only
/// pair that needs care is the root, which is a prefix of everything and equal
/// to nothing but itself.
fn same_place(mounted: &str, essential: &str) -> bool {
    if essential == "/" {
        return mounted == "/";
    }
    let mut a = std::path::Path::new(mounted).components();
    let b = std::path::Path::new(essential).components();
    b.into_iter().all(|c| a.next() == Some(c))
}

/// Why a block is not a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skipped {
    /// It carries a partition table, so it is a drive's heading.
    WholeDisk,
    /// It has no filesystem to mount.
    NoFilesystem,
    /// It holds something else, or it is an image.
    Holder,
    /// It is a loop somebody else attached.
    ForeignLoop,
    /// It is an encrypted container that is open: what came out of it is the
    /// row, in its place.
    Unlocked,
    /// It is what came out of the `/private` vault, mounted there: the vault
    /// has its own row, and a second one as a disk would put formatting one
    /// key away from it.
    Vault,
}

/// Why this block is not a row — `None` means it IS one.
///
/// Four clauses, in order, and each of them earns its place against a real
/// machine's nineteen block devices:
///
/// 1. **A partition table is a heading, not a place.** A hybrid ISO on a stick
///    carries `iso9660` on the WHOLE disk *and* a table, so without this the
///    stick appears twice — once as itself and once as its first partition,
///    same label, different sizes.
/// 2. **No `Filesystem` interface, nothing to mount.** This is what removes the
///    swap and the LUKS container. It is *not* "no mount point": an unmounted
///    vfat is exactly what a sidebar exists for.
/// 3. **A holder or an image.** This is what removes the eleven squashfs loops
///    a snap-using machine carries, which DO have a filesystem and DO have a
///    mount point.
/// 4. **Somebody else's loop.** One the person attached themselves is an image
///    they went looking for, and hiding it would be hiding their own work.
///
/// And one exception, before the second clause: **a LUKS container that is
/// not the system's** has no filesystem and is still a row while it is locked
/// — it is where one asks to open it — and gives its place to what came out of
/// it while it is open. The system's own stays a holder.
///
/// And before every clause, **what came out of the `/private` vault**, mounted
/// there: the vault is a row of its own (the file manager's), never a disk.
#[must_use]
pub fn skipped(v: &Volume, uid: u32) -> Option<Skipped> {
    if v.mounted_at
        .iter()
        .any(|at| std::path::Path::new(at) == std::path::Path::new(ronoctua_private::MOUNT_POINT))
    {
        return Some(Skipped::Vault);
    }
    if v.holds.partition_table {
        return Some(Skipped::WholeDisk);
    }
    match v.encrypted() {
        Some(Encrypted::Locked) => return None,
        Some(Encrypted::Unlocked) => return Some(Skipped::Unlocked),
        None => {}
    }
    if !v.holds.filesystem {
        return Some(Skipped::NoFilesystem);
    }
    if HOLDERS.contains(&v.id_type.as_str()) {
        return Some(Skipped::Holder);
    }
    if v.holds.loop_back && v.set_up_by != Some(uid) {
        return Some(Skipped::ForeignLoop);
    }
    None
}

/// The blocks that are rows, in the order they were read.
///
/// `show_quiet` is the file manager's own "show hidden" — the key that already
/// reveals dotfiles. Default off, the ESP and a stick's EFI helper stay out of
/// the way; on, everything udisks marked `HintIgnore` comes back. Neither
/// answer is silent, which is the point: filtering on `HintIgnore` alone would
/// take away `/boot` with no way to ask for it.
#[must_use]
pub fn listed(all: &[Volume], uid: u32, show_quiet: bool) -> Vec<&Volume> {
    all.iter()
        .filter(|v| skipped(v, uid).is_none())
        .filter(|v| show_quiet || !v.quiet)
        .collect()
}

/// Things that HOLD another device, and are never a place to write.
///
/// [`HOLDERS`] cannot be reused here, and the difference is one entry: it lists
/// the empty type as a holder, because a block with nothing on it is nothing to
/// MOUNT. To formatting, a block with nothing on it is the most ordinary
/// target there is — a partition waiting for a filesystem.
pub const CONTAINERS: [&str; 9] = [
    "squashfs",
    "erofs",
    "crypto_LUKS",
    "crypto_TCRYPT",
    "BitLocker",
    "LVM2_member",
    "linux_raid_member",
    "zfs_member",
    "DM_snapshot_cow",
];

/// Every device the formatting screen shows.
///
/// # Including the ones it will refuse
///
/// A volume the running system [`needed`] answers for is IN this list. It has
/// to be: somebody looking for their disk has to find it, and then read why it
/// is not on offer. A list that quietly omitted it would send them looking for
/// a row that is not there, and the next thing they reach for is `mkfs` in a
/// terminal, which asks nothing.
///
/// What is cut is what cannot be a target at all: a container with a live
/// device inside it, somebody else's loop, and read-only media — none of which
/// is a decision anybody can change from that screen.
#[must_use]
pub fn candidates(all: &[Volume], uid: u32) -> Vec<&Volume> {
    // A LOCKED CONTAINER THIS SYSTEM OPENS IS A TARGET: a stick encrypted
    // yesterday is formatted again today, and nothing lives inside it while
    // it is locked. One that is open — or the system's own — is not.
    let locked = |v: &Volume| v.encrypted() == Some(Encrypted::Locked);
    all.iter()
        .filter(|v| !(v.holds.loop_back && v.set_up_by != Some(uid)))
        .filter(|v| (!v.holds.encrypted || locked(v)) && !v.holds.swap)
        .filter(|v| locked(v) || !CONTAINERS.contains(&v.id_type.as_str()))
        .filter(|v| !v.read_only && !v.quiet)
        // AND NOTHING OF ZERO BYTES. Measured on a booted machine: udisks
        // publishes a floppy node and EIGHT unattached loop devices, all of
        // size 0, and every one of them came out as a row on the formatting
        // screen — nine lines of noise in front of the two disks somebody
        // actually has.
        //
        // The rule is not cosmetic: a device of zero bytes has nowhere to put a
        // filesystem, so offering it is offering a refusal. And it is `bytes`
        // and not a list of node names, because the next machine has a card
        // reader with four empty slots and they are not called `loop`.
        .filter(|v| v.bytes > 0)
        .collect()
}

/// Which drive a volume belongs under.
///
/// One hop when it sits on a drive, two when it came out of a LUKS container —
/// and the second is the case this exists for. An encrypted root arrives as a
/// device-mapper node with no drive of its own, and reaches one only through
/// the container it was unlocked from. A rule of "no drive, drop it" would take
/// the loops away and the root filesystem with them.
#[must_use]
pub fn attached_to<'a>(v: &'a Volume, all: &'a [Volume]) -> Option<&'a str> {
    if let Some(direct) = v.drive.as_deref() {
        return Some(direct);
    }
    // ONE more hop, and only one. Two would mean a container inside a
    // container, which udisks already flattens: what comes out of a LUKS has
    // the container as its `CryptoBackingDevice`, and the container sits on a
    // drive. A loop over `behind` would be a loop over a chain that is always
    // one long, and it would hang on a cycle nobody can rule out from here.
    let behind = v.behind.as_deref()?;
    all.iter()
        .find(|b| b.at == behind)
        .and_then(|b| b.drive.as_deref())
}

/// Does this drive earn a heading of its own?
///
/// Yes when something under it is a row — and yes when NOTHING is but the drive
/// can be unplugged. A blank stick with no table and no signature is removed by
/// the second clause of [`skipped`], and without this it would vanish entirely:
/// "I plugged it in and nothing happened" is the worst answer a sidebar can
/// give.
#[must_use]
pub fn worth_a_heading(drive: &Drive, rows: &[&Volume], all: &[Volume]) -> bool {
    drive.removable || rows.iter().any(|v| attached_to(v, all) == Some(&drive.at))
}

/// What MBR can address with 512-byte sectors. Past it, GPT.
const MBR_CEILING: u64 = 2 * 1024 * 1024 * 1024 * 1024;

/// How many times to wait for a freshly written partition table to show up.
const SETTLE_TRIES: u32 = 20;

/// How long each of those waits is — two seconds in total, which is longer
/// than a uevent takes and shorter than a person decides something is broken.
const SETTLE_WAIT: std::time::Duration = std::time::Duration::from_millis(100);

/// The conversation with udisks2.
pub struct Manager {
    connection: zbus::blocking::Connection,
}

impl Manager {
    /// Opens the connection to the system bus.
    ///
    /// # Errors
    ///
    /// No system bus, or no udisks2 on it.
    pub fn fresh() -> Result<Self> {
        let connection =
            zbus::blocking::Connection::system().context("no system bus to talk to udisks2")?;
        Ok(Self { connection })
    }

    /// Everything the machine has, in one round trip.
    ///
    /// # Errors
    ///
    /// udisks2 is not on the bus, or did not answer.
    pub fn read(&self) -> Result<Rack> {
        let objects = udisks::ObjectsProxyBlocking::new(&self.connection)
            .context("udisks2 is off the bus")?;
        let managed = objects
            .get_managed_objects()
            .context("udisks2 did not answer GetManagedObjects")?;

        let mut rack = Rack::default();
        for (path, faces) in &managed {
            let at = path.as_str().to_owned();
            if let Some(props) = faces.get(udisks::DRIVE) {
                rack.drives.push(Drive {
                    at: at.clone(),
                    vendor: udisks::text_of(props.get("Vendor")),
                    model: udisks::text_of(props.get("Model")),
                    bytes: udisks::number_of(props.get("Size")),
                    bus: Bus::of(&udisks::text_of(props.get("ConnectionBus"))),
                    removable: udisks::flag_of(props.get("Removable")),
                    ejectable: udisks::flag_of(props.get("Ejectable")),
                    can_power_off: udisks::flag_of(props.get("CanPowerOff")),
                    sort_key: udisks::text_of(props.get("SortKey")),
                    serial: udisks::text_of(props.get("Serial")),
                });
            }
            let Some(block) = faces.get(udisks::BLOCK) else {
                continue;
            };
            let filesystem = faces.get(udisks::FILESYSTEM);
            rack.volumes.push(Volume {
                at,
                node: udisks::path_of(block.get("Device")),
                label: udisks::text_of(block.get("IdLabel")),
                id_type: udisks::text_of(block.get("IdType")),
                bytes: udisks::number_of(block.get("Size")),
                read_only: udisks::flag_of(block.get("ReadOnly")),
                system: udisks::flag_of(block.get("HintSystem")),
                quiet: udisks::flag_of(block.get("HintIgnore")),
                mounted_at: filesystem
                    .map(|f| udisks::paths_of(f.get("MountPoints")))
                    .unwrap_or_default(),
                drive: udisks::object_of(block.get("Drive")),
                behind: udisks::object_of(block.get("CryptoBackingDevice")),
                cleartext: faces
                    .get(udisks::ENCRYPTED)
                    .and_then(|e| udisks::object_of(e.get("CleartextDevice"))),
                set_up_by: faces
                    .get(udisks::LOOP)
                    .and_then(|l| udisks::small_number_of(l.get("SetupByUID"))),
                in_fstab: udisks::in_fstab(block.get("Configuration")),
                holds: Holding {
                    filesystem: filesystem.is_some(),
                    partition_table: faces.contains_key(udisks::PARTITION_TABLE),
                    partition: faces.contains_key(udisks::PARTITION),
                    encrypted: faces.contains_key(udisks::ENCRYPTED),
                    swap: faces.contains_key(udisks::SWAPSPACE),
                    loop_back: faces.contains_key(udisks::LOOP),
                },
                symlinks: udisks::paths_of(block.get("Symlinks")),
            });
        }
        // udisks' own key, and the device node to break ties. `GetManagedObjects`
        // answers a map, so without this the order changes between readings and
        // the sidebar reshuffles under the cursor.
        rack.drives
            .sort_by(|a, b| a.sort_key.cmp(&b.sort_key).then(a.at.cmp(&b.at)));
        rack.volumes.sort_by(|a, b| a.node.cmp(&b.node));
        Ok(rack)
    }

    /// Mounts a volume and answers where it landed.
    ///
    /// # Errors
    ///
    /// udisks refused. The error carries its NAME, which is what the caller
    /// turns into a sentence — see [`refusal`].
    pub fn mount(&self, at: &str) -> Result<String> {
        let fs = self.filesystem(at)?;
        Ok(fs.mount(udisks::no_asking())?)
    }

    /// Unmounts a volume.
    ///
    /// # Errors
    ///
    /// udisks refused — most often because something still has a file open on
    /// it, which is a refusal worth repeating rather than overriding.
    pub fn unmount(&self, at: &str) -> Result<()> {
        let fs = self.filesystem(at)?;
        fs.unmount(udisks::no_asking())?;
        Ok(())
    }

    /// Cuts a drive's power, which is what makes a stick safe to pull.
    ///
    /// # Errors
    ///
    /// udisks refused, or the drive cannot do it.
    pub fn power_off(&self, at: &str) -> Result<()> {
        let drive = udisks::DriveProxyBlocking::builder(&self.connection)
            .path(at.to_owned())?
            .build()?;
        drive.power_off(udisks::no_asking())?;
        Ok(())
    }

    /// Spits the medium out of a drive that has a tray or a slot.
    ///
    /// # Errors
    ///
    /// udisks refused, or the drive cannot do it.
    pub fn eject(&self, at: &str) -> Result<()> {
        let drive = udisks::DriveProxyBlocking::builder(&self.connection)
            .path(at.to_owned())?
            .build()?;
        drive.eject(udisks::no_asking())?;
        Ok(())
    }

    /// Opens a LUKS container with `passphrase`, and answers the object path
    /// of what came out of it.
    ///
    /// # Errors
    ///
    /// udisks refused. A passphrase that does not open it is refused as
    /// [`Refusal::WrongPassphrase`] (see [`refusal`]) — udisks names that one
    /// no differently from any other failure, so its words are read here.
    pub fn unlock(&self, at: &str, passphrase: &ronoctua_consent::keys::Secret) -> Result<String> {
        let container = udisks::EncryptedProxyBlocking::builder(&self.connection)
            .path(at.to_owned())?
            .build()?;
        match container.unlock(passphrase.expose(), udisks::no_asking()) {
            Ok(path) => Ok(path.as_str().to_owned()),
            Err(zbus::Error::MethodError(name, said, _))
                if name.as_str() == "org.freedesktop.UDisks2.Error.Failed"
                    && said.as_deref().is_some_and(wrong_passphrase) =>
            {
                Err(anyhow::Error::new(WrongPassphrase))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Closes a LUKS container. What came out of it must not be mounted —
    /// [`Manager::put_away`] unmounts first.
    ///
    /// # Errors
    ///
    /// udisks refused.
    pub fn lock(&self, at: &str) -> Result<()> {
        let container = udisks::EncryptedProxyBlocking::builder(&self.connection)
            .path(at.to_owned())?
            .build()?;
        container.lock(udisks::no_asking())?;
        Ok(())
    }

    /// Puts a volume away: unmounts it when it is mounted, locks the
    /// container it came out of when it came out of one this system opens
    /// (`lock_only`: and stops there), and — for an ejection — cuts the
    /// power to its drive when the drive can be.
    ///
    /// THE STEPS IN THIS ORDER AND EACH ONLY AFTER THE ONE BEFORE CAME BACK:
    /// cutting the power under a mounted filesystem is how a stick leaves with
    /// a write half done, and locking under one is refused by udisks anyway.
    /// The drive is found through the container for an open one — the
    /// cleartext block has no drive of its own — and never by the volume's
    /// own path, which is a block and not a drive.
    ///
    /// # Errors
    ///
    /// The first refusal, which stops the rest.
    pub fn put_away(&self, at: &str, lock_only: bool) -> Result<()> {
        let rack = self.read()?;
        // GONE ALREADY is put away already: what came out of a container
        // disappears when the container is locked, and a caller that named
        // both is not refused for the second.
        let Some(volume) = rack.volumes.iter().find(|v| v.at == at) else {
            return Ok(());
        };
        if volume.mounted() {
            self.unmount(at)?;
        }
        // THE CONTAINER: the one this came out of, or this one itself when
        // what was pressed is the container, open.
        let container = container_of(volume, &rack.volumes)
            .or_else(|| (volume.encrypted() == Some(Encrypted::Unlocked)).then_some(volume));
        if let Some(c) = container {
            if let Some(clear) = c.cleartext.as_deref()
                && clear != at
                && rack.volumes.iter().any(|v| v.at == clear && v.mounted())
            {
                self.unmount(clear)?;
            }
            self.lock(&c.at)?;
        }
        if lock_only {
            return Ok(());
        }
        let drive =
            attached_to(volume, &rack.volumes).and_then(|d| rack.drives.iter().find(|x| x.at == d));
        if let Some(d) = drive
            && d.can_power_off
        {
            self.power_off(&d.at)?;
        }
        Ok(())
    }

    /// Writes a filesystem over a volume, or over a whole disk.
    ///
    /// # What this does NOT decide
    ///
    /// Whether it is allowed. Two answers stand in front of it and neither is
    /// here: udisks' own polkit split (`modify-device` is free for an active
    /// session, `modify-device-system` asks for the administrator nobody can
    /// answer), and the caller's check that the target is not a volume
    /// [`needed`] answers for. This function is the CALL, and a function that
    /// also judged would be a second copy of a judgement that has to be one.
    ///
    /// # What `whole` costs
    ///
    /// A partition table, then one partition filling it. Anything already on
    /// the disk is gone — which is the point of the gesture, and the reason the
    /// buffer asks twice before it gets here.
    ///
    /// # And `encrypt`
    ///
    /// With a passphrase the filesystem is written inside a LUKS2 container
    /// (`encrypt.passphrase`, `encrypt.type = luks2`) — on the partition, or on
    /// the one partition a whole disk gets. The passphrase goes in the call to
    /// udisksd and nowhere else.
    ///
    /// # Errors
    ///
    /// udisks refused: something is still mounted on it, or the session is not
    /// allowed to touch this device. The error carries its NAME — see
    /// [`refusal`].
    pub fn format(
        &self,
        at: &str,
        filesystem: &str,
        label: &str,
        whole: bool,
        encrypt: Option<&ronoctua_consent::keys::Secret>,
    ) -> Result<()> {
        let block = self.block(at)?;

        if !whole {
            block.format(filesystem, self.recipe_options(label, encrypt))?;
            return Ok(());
        }

        // THE TABLE, CHOSEN BY SIZE AND NOT BY TASTE. MBR addresses 2 TiB with
        // 512-byte sectors and silently loses everything past it, so a disk
        // over the line gets GPT. Under it, MBR — because "opens on every
        // computer there is" includes the camera and the car stereo, and those
        // read MBR.
        let bytes = block.size().unwrap_or(0);
        let table = if bytes > MBR_CEILING { "gpt" } else { "dos" };
        block.format(table, udisks::no_asking())?;

        // AND THE INTERFACE HAS TO ARRIVE BEFORE IT CAN BE USED. `Format`
        // returns when the table is written, but `PartitionTable` appears on
        // the object through a uevent, and asking for it in the same
        // millisecond gets `UnknownInterface` — a refusal that reads like a
        // disk that cannot be partitioned. Bounded, so a real failure still
        // fails instead of spinning.
        let mut last = None;
        for _ in 0..SETTLE_TRIES {
            let table = udisks::PartitionTableProxyBlocking::builder(&self.connection)
                .path(at.to_owned())?
                .build()?;
            match table.create_partition_and_format(
                0,
                0,
                "",
                "",
                udisks::no_asking(),
                filesystem,
                self.recipe_options(label, encrypt),
            ) {
                Ok(_) => return Ok(()),
                Err(zbus::Error::MethodError(name, _, _))
                    if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod"
                        || name.as_str() == "org.freedesktop.DBus.Error.UnknownInterface" =>
                {
                    std::thread::sleep(SETTLE_WAIT);
                    last = Some(name);
                }
                Err(e) => return Err(e.into()),
            }
        }
        anyhow::bail!(
            "udisks2 wrote the table at {at} but did not offer the partition interface: {last:?}"
        )
    }

    /// The options a format call carries, on top of [`udisks::no_asking`].
    fn recipe_options(
        &self,
        label: &str,
        encrypt: Option<&ronoctua_consent::keys::Secret>,
    ) -> HashMap<&'static str, zbus::zvariant::Value<'static>> {
        let mut options = udisks::no_asking();
        if !label.is_empty() {
            options.insert("label", zbus::zvariant::Value::new(label.to_owned()));
        }
        if let Some(passphrase) = encrypt {
            options.insert(
                "encrypt.passphrase",
                zbus::zvariant::Value::new(passphrase.expose().to_owned()),
            );
            options.insert("encrypt.type", zbus::zvariant::Value::new("luks2"));
        }
        // WITHOUT THIS, AN MBR PARTITION HOLDING vfat IS TYPED `0x83`, Linux.
        // Windows then does not offer to open the stick and nothing anywhere
        // says why — the filesystem is right and the byte beside it is wrong.
        options.insert("update-partition-type", zbus::zvariant::Value::Bool(true));
        options
    }

    fn block(&self, at: &str) -> Result<udisks::BlockProxyBlocking<'_>> {
        Ok(udisks::BlockProxyBlocking::builder(&self.connection)
            .path(at.to_owned())?
            .build()?)
    }

    fn filesystem(&self, at: &str) -> Result<udisks::FilesystemProxyBlocking<'_>> {
        Ok(udisks::FilesystemProxyBlocking::builder(&self.connection)
            .path(at.to_owned())?
            .build()?)
    }

    /// A blocking stream of everything udisks2 says.
    ///
    /// ONE iterator and not three streams, because the blocking zbus API has no
    /// `select`: `InterfacesAdded`, `InterfacesRemoved` and the
    /// `PropertiesChanged` that carries a new `MountPoints` all matter, and a
    /// match rule on the sender catches the three on one thread.
    ///
    /// # Errors
    ///
    /// The match rule could not be installed.
    pub fn signals(&self) -> Result<zbus::blocking::MessageIterator> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(udisks::SERVICE)?
            .build();
        Ok(zbus::blocking::MessageIterator::for_match_rule(
            rule,
            &self.connection,
            None,
        )?)
    }
}

/// What udisks refused with, as a name rather than a sentence.
///
/// The NAME is the thing worth reading: `NotSupported` and `NotAuthorized` are
/// two different answers a person can act on, and udisks' own English prose is
/// upstream's to reword whenever it likes. Whoever holds the language bundle
/// turns this into a sentence (invariant 8); the prose travels alongside as an
/// ARGUMENT, which is the concession the transfer queue already makes — a
/// failure carries its own words, and "could not mount" alone does not say
/// "unknown filesystem type".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No driver, or the filesystem is one this build cannot mount.
    NoDriver,
    /// It is already mounted. Not an error to a person: go there.
    AlreadyMounted,
    /// polkit said no. With no authentication agent in this session, this is
    /// what a `HintSystem` volume answers.
    NeedsAdmin,
    /// Something still has it open.
    Busy,
    /// The passphrase does not open this container.
    WrongPassphrase,
    /// Anything else. The caller carries udisks' own words alongside.
    Other,
}

/// A passphrase that did not open a container — carried through `anyhow` from
/// [`Manager::unlock`] to [`refusal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongPassphrase;

impl std::fmt::Display for WrongPassphrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the passphrase does not open this container")
    }
}

impl std::error::Error for WrongPassphrase {}

/// Whether udisks' words about a failed unlock say the passphrase was wrong.
///
/// udisks answers a wrong passphrase with its generic `Error.Failed`
/// (`Error unlocking /dev/sdb1: …`), and the words after it are
/// libblockdev's: for the `EPERM` cryptsetup returns when no key slot opens,
/// the pinned libblockdev (`src/plugins/crypto.c`, read from its source) says
/// `Failed to activate device: Incorrect passphrase.`; older ones printed the
/// errno (`Operation not permitted`), and cryptsetup's own is `No key
/// available with this passphrase`. Read ONLY after an unlock: the same words
/// after a mount mean something else.
fn wrong_passphrase(said: &str) -> bool {
    let said = said.to_ascii_lowercase();
    [
        "no key available",
        "operation not permitted",
        "incorrect passphrase",
        "wrong passphrase",
        "invalid passphrase",
    ]
    .iter()
    .any(|w| said.contains(w))
}

/// Reads the refusal out of an error, whatever it is wrapped in.
///
/// It walks the chain because the error arrives through `anyhow` and the D-Bus
/// name is on the `zbus::Error` underneath.
#[must_use]
pub fn refusal(e: &anyhow::Error) -> Refusal {
    if e.chain()
        .any(|c| c.downcast_ref::<WrongPassphrase>().is_some())
    {
        return Refusal::WrongPassphrase;
    }
    for cause in e.chain() {
        if let Some(zbus::Error::MethodError(name, _, _)) = cause.downcast_ref::<zbus::Error>() {
            return match name.as_str() {
                "org.freedesktop.UDisks2.Error.NotSupported" => Refusal::NoDriver,
                "org.freedesktop.UDisks2.Error.AlreadyMounted" => Refusal::AlreadyMounted,
                "org.freedesktop.UDisks2.Error.DeviceBusy" => Refusal::Busy,
                "org.freedesktop.PolicyKit1.Error.NotAuthorized"
                | "org.freedesktop.UDisks2.Error.NotAuthorized"
                | "org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain"
                | "org.freedesktop.UDisks2.Error.NotAuthorizedDismissed" => Refusal::NeedsAdmin,
                _ => Refusal::Other,
            };
        }
    }
    Refusal::Other
}

/// Watches udisks2 and hands over a fresh reading each time something moves.
///
/// # Why a signal and not the footer's clock
///
/// Plugging in a stick is an EVENT, with somebody's hand behind it. The
/// manager's `stale()` polls because a directory's mtime is a *state* anything
/// may change; this is not that. And two of the reasons decide on their own:
/// the footer's tick only fires the re-read while the manager is OPEN, so a
/// stick plugged before opening it would go unnoticed — which is the common
/// case, not the edge one — and the window door has no clock at all.
///
/// # Why the whole reading and not a nudge
///
/// Because plugging in ONE stick emits four `InterfacesAdded` in a few
/// milliseconds — the drive and its three blocks — and a nudge would make the
/// drawing loop do four round trips of its own. Sending the reading means the
/// bus is only ever touched on this thread, and the last message wins.
///
/// # Errors
///
/// No system bus, no udisks2 on it, or the thread could not be raised.
pub fn watch<F>(on_change: F) -> Result<()>
where
    F: Fn(Rack) + Send + 'static,
{
    watch_while(move |rack| {
        on_change(rack);
        true
    })
}

/// [`watch`], for as long as `on_change` answers `true`: a reading answered
/// with `false` ends the thread — for a screen that watches the disks only
/// while it is open (the file picker), whose thread then goes at the next
/// signal instead of living as long as the process.
///
/// # Errors
///
/// As [`watch`].
pub fn watch_while<F>(on_change: F) -> Result<()>
where
    F: Fn(Rack) -> bool + Send + 'static,
{
    watch_readings(move |reading| reading.map_or(true, &on_change))
}

/// [`watch_while`], with every reading that FAILED handed over too: the
/// first one, a later one, and a udisks that gives no stream at all (the
/// thread then ends). For a screen that waits for its first reading to draw
/// anything — the installer stick's drives — a failure that is only logged
/// is a "reading the drives…" drawn for ever over a refusal.
///
/// # Errors
///
/// As [`watch`].
pub fn watch_readings<F>(on_reading: F) -> Result<()>
where
    F: Fn(Result<Rack>) -> bool + Send + 'static,
{
    let manager = Manager::fresh()?;
    std::thread::Builder::new()
        .name("ronoctua-disks".to_owned())
        .spawn(move || {
            // THE MATCH RULE BEFORE THE FIRST READING: a stick plugged in
            // while the first reading is on the wire emits its signal before
            // the rule lands, and the list would miss it until the next
            // unrelated one. The same order `ronoctua-power` records.
            let messages = match manager.signals() {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("no udisks2 stream; the list will not reread on its own");
                    let _ = on_reading(Err(e));
                    return;
                }
            };
            match manager.read() {
                Ok(rack) => {
                    if !on_reading(Ok(rack)) {
                        return;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "the first reading of the disks failed");
                    if !on_reading(Err(e)) {
                        return;
                    }
                }
            }
            for _ in messages {
                let reading = manager.read();
                if let Err(e) = &reading {
                    tracing::warn!(error = %e, "rereading the disks failed");
                }
                if !on_reading(reading) {
                    return;
                }
            }
            // The iterator ending means udisks2 went away. Saying so beats a
            // thread that quietly stops working — the lesson `watch_scan` in
            // `ronoctua-network` already records.
            tracing::info!("the udisks2 stream ended; no automatic reread");
        })
        .context("could not start the disks thread")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The uid the fixture's loops were attached by. Not the person's.
    const SOMEBODY_ELSE: u32 = 0;
    /// The person at the keyboard.
    const ME: u32 = 1000;

    fn block(node: &str, id_type: &str) -> Volume {
        Volume {
            at: format!("/block/{}", node.trim_start_matches("/dev/")),
            node: node.to_owned(),
            id_type: id_type.to_owned(),
            holds: Holding {
                filesystem: true,
                ..Holding::default()
            },
            ..Volume::default()
        }
    }

    /// THE NINE ROWS OF A REAL MACHINE, with a pendrive in it.
    ///
    /// Measured with `udisksctl dump` on the machine this was written on. The
    /// ids are SYNTHETIC — udisks embeds a drive's serial number in its object
    /// path, and this repository is public (invariant 13). Nothing else was
    /// changed: the hints, the types and the mount points are what the machine
    /// actually said.
    fn fixture() -> Rack {
        let internal = Drive {
            at: "/drive/internal".to_owned(),
            vendor: String::new(),
            model: "an internal disk".to_owned(),
            bytes: 2_048_408_248_320,
            bus: Bus::Internal,
            removable: false,
            ejectable: false,
            can_power_off: false,
            sort_key: "00coldplug/00fixed/nvme0".to_owned(),
            serial: String::new(),
        };
        let hotplug = Drive {
            at: "/drive/hotplug".to_owned(),
            vendor: "SanDisk".to_owned(),
            model: "Cruzer Blade".to_owned(),
            bytes: 8_053_063_680,
            bus: Bus::Usb,
            removable: true,
            ejectable: true,
            can_power_off: true,
            sort_key: "01hotplug/1700000000".to_owned(),
            serial: String::new(),
        };

        let mut volumes = Vec::new();

        // The root filesystem: device-mapper, NO drive of its own, reaching one
        // only through the LUKS container it was unlocked from.
        volumes.push(Volume {
            system: true,
            in_fstab: true,
            // ONE DEVICE, SIX PLACES. The disko layout puts the root, the home,
            // the store, the persistent tree and the log on btrfs subvolumes
            // inside one LUKS container, so this single volume is mounted at
            // all of them — which is why one predicate over `mounted_at`
            // protects the lot.
            mounted_at: vec![
                "/".to_owned(),
                "/home".to_owned(),
                "/nix".to_owned(),
                "/persist".to_owned(),
                "/var/log".to_owned(),
            ],
            behind: Some("/block/nvme0n1p2".to_owned()),
            ..block("/dev/dm-0", "btrfs")
        });

        // Eleven squashfs images somebody's snaps left mounted. They HAVE a
        // filesystem and they HAVE a mount point, which is why the type is what
        // removes them.
        for n in 0..11 {
            volumes.push(Volume {
                system: true,
                mounted_at: vec![format!("/var/lib/snapd/snap/thing/{n}")],
                set_up_by: Some(SOMEBODY_ELSE),
                holds: Holding {
                    filesystem: true,
                    loop_back: true,
                    ..Holding::default()
                },
                ..block(&format!("/dev/loop{n}"), "squashfs")
            });
        }

        // The internal disk itself: a partition table, no filesystem.
        volumes.push(Volume {
            system: true,
            drive: Some(internal.at.clone()),
            holds: Holding {
                filesystem: false,
                partition_table: true,
                ..Holding::default()
            },
            ..block("/dev/nvme0n1", "")
        });
        // The ESP. udisks marks it `HintIgnore` by its GPT type.
        volumes.push(Volume {
            system: true,
            quiet: true,
            in_fstab: true,
            mounted_at: vec!["/boot".to_owned()],
            drive: Some(internal.at.clone()),
            ..block("/dev/nvme0n1p1", "vfat")
        });
        // The LUKS container. What one opens is what comes out of it.
        volumes.push(Volume {
            system: true,
            drive: Some(internal.at.clone()),
            holds: Holding {
                filesystem: false,
                encrypted: true,
                ..Holding::default()
            },
            ..block("/dev/nvme0n1p2", "crypto_LUKS")
        });

        // The stick, and it is a HYBRID ISO: a signature on the whole disk AND
        // a partition table.
        volumes.push(Volume {
            label: "MJRO1812".to_owned(),
            drive: Some(hotplug.at.clone()),
            holds: Holding {
                filesystem: true,
                partition_table: true,
                ..Holding::default()
            },
            ..block("/dev/sda", "iso9660")
        });
        volumes.push(Volume {
            label: "MJRO1812".to_owned(),
            mounted_at: vec!["/run/media/somebody/MJRO1812".to_owned()],
            drive: Some(hotplug.at.clone()),
            ..block("/dev/sda1", "iso9660")
        });
        // The stick's 4 MiB EFI helper — `HintIgnore` by its MBR type, and
        // UNMOUNTED, which is what makes it the best thing on this machine to
        // prove mounting with.
        volumes.push(Volume {
            label: "MISO_EFI".to_owned(),
            quiet: true,
            bytes: 4_194_304,
            drive: Some(hotplug.at.clone()),
            ..block("/dev/sda2", "vfat")
        });

        // Swap in RAM.
        volumes.push(Volume {
            system: true,
            quiet: true,
            label: "zram0".to_owned(),
            holds: Holding {
                filesystem: false,
                swap: true,
                ..Holding::default()
            },
            ..block("/dev/zram0", "swap")
        });

        volumes.sort_by(|a, b| a.node.cmp(&b.node));
        Rack {
            drives: vec![internal, hotplug],
            volumes,
        }
    }

    fn nodes(rows: &[&Volume]) -> Vec<String> {
        rows.iter().map(|v| v.node.clone()).collect()
    }

    #[test]
    fn the_images_the_swap_and_the_holder_stay_out_and_the_rest_stays_in() {
        let rack = fixture();
        let seen = nodes(&listed(&rack.volumes, ME, false));

        // The eleven images have a filesystem AND a mount point, so only the
        // TYPE removes them. This is the clause a "no mount point, no row" rule
        // would have got backwards in both directions at once.
        assert!(
            !seen.iter().any(|n| n.starts_with("/dev/loop")),
            "a loop became a line: {seen:?}"
        );
        assert!(!seen.contains(&"/dev/zram0".to_owned()), "{seen:?}");
        assert!(!seen.contains(&"/dev/nvme0n1p2".to_owned()), "{seen:?}");
        // The two whole disks are headings. `sda` is the one that earns the
        // clause: it carries a hybrid `iso9660` signature and a table at once.
        assert!(!seen.contains(&"/dev/sda".to_owned()), "{seen:?}");
        assert!(!seen.contains(&"/dev/nvme0n1".to_owned()), "{seen:?}");
        // And what somebody came for is there.
        assert_eq!(seen, ["/dev/dm-0", "/dev/sda1"]);
    }

    #[test]
    fn hint_ignore_is_quiet_and_not_gone() {
        // The measurement this test exists for: udisks marks BOTH EFI system
        // partitions `HintIgnore` — `/boot` by its GPT type and the stick's
        // helper by its MBR type. Filtering on it would take away two rows the
        // owner asked for by name, and give no reason for either.
        let rack = fixture();
        assert_eq!(
            nodes(&listed(&rack.volumes, ME, true)),
            ["/dev/dm-0", "/dev/nvme0n1p1", "/dev/sda1", "/dev/sda2"]
        );
    }

    #[test]
    fn the_swap_is_gone_under_both_keys() {
        // `zram0` is `HintIgnore` AND has no filesystem. The second is what
        // must decide, because `HintIgnore` alone is a thing a person can ask
        // to see — and nobody should be one key away from their swap device.
        let rack = fixture();
        assert!(!nodes(&listed(&rack.volumes, ME, true)).contains(&"/dev/zram0".to_owned()));
    }

    #[test]
    fn the_root_filesystem_finds_its_disk_through_the_luks_it_is_inside() {
        // The case a "no drive, drop it" rule throws away along with the loops.
        let rack = fixture();
        let root = rack.volumes.iter().find(|v| v.node == "/dev/dm-0").unwrap();
        assert_eq!(attached_to(root, &rack.volumes), Some("/drive/internal"));

        let stick = rack.volumes.iter().find(|v| v.node == "/dev/sda1").unwrap();
        assert_eq!(attached_to(stick, &rack.volumes), Some("/drive/hotplug"));
    }

    #[test]
    fn what_the_running_system_needs_is_named_and_not_merely_refused() {
        let rack = fixture();
        let of = |node: &str| -> Option<Needed> {
            needed(rack.volumes.iter().find(|v| v.node == node).unwrap())
        };
        // THE ROOT, AND THE FIVE PLACES THAT SHARE ITS DEVICE. The answer names
        // the mount point, because "this cannot be unmounted" is a rule nobody
        // can check and "`/` is mounted here" is a fact anybody can look at.
        assert_eq!(of("/dev/dm-0"), Some(Needed::At("/".to_owned())));
        // The ESP, by the same list AND by fstab — either alone would do.
        assert!(of("/dev/nvme0n1p1").is_some());
        // Swap, whatever else is true of it.
        assert_eq!(of("/dev/zram0"), Some(Needed::Swap));
        // AND THE PENDRIVE IS NOT. This is the half that matters: a guard that
        // refused everything would be safe and useless.
        assert_eq!(of("/dev/sda1"), None);
        assert_eq!(of("/dev/sda2"), None);
    }

    #[test]
    fn a_data_disk_at_home_backup_is_not_the_home() {
        // COMPONENT BY COMPONENT, never `starts_with`. `/home-backup` and
        // `/nix-cache` are somebody's own disks, and a string prefix would
        // refuse to unmount both of them for ever, with a message naming a
        // mount point that is not theirs.
        let mine = |at: &str| {
            let mut v = block("/dev/sdb1", "ext4");
            v.mounted_at = vec![at.to_owned()];
            needed(&v)
        };
        assert_eq!(mine("/home-backup"), None);
        assert_eq!(mine("/nix-cache"), None);
        assert_eq!(mine("/var-old"), None);
        // And the real ones still are.
        assert_eq!(mine("/home"), Some(Needed::At("/home".to_owned())));
        assert_eq!(mine("/var/log"), Some(Needed::At("/var/log".to_owned())));
    }

    #[test]
    fn fstab_alone_is_enough_even_where_the_list_says_nothing() {
        // The signal that covers a machine laid out some other way: a data
        // partition the administrator declared at boot is one the system
        // expects, wherever it is mounted.
        let mut v = block("/dev/sdb2", "ext4");
        v.mounted_at = vec!["/srv/data".to_owned()];
        assert_eq!(needed(&v), None);
        v.in_fstab = true;
        assert_eq!(needed(&v), Some(Needed::InFstab));
    }

    #[test]
    fn an_unknown_filesystem_is_shown_and_not_swallowed() {
        // The deny list is a deny list on purpose: a type nobody wrote down is
        // a row one ignores, while a disk hidden for an unstated reason is a
        // disk that cannot be reached.
        let v = block("/dev/sdb1", "bcachefs");
        assert_eq!(skipped(&v, ME), None);
    }

    /// FAILS IF: what came out of the `/private` vault becomes a row of its
    /// own, beside the vault's.
    #[test]
    fn the_vaults_inside_is_not_a_disk() {
        let mut v = block("/dev/dm-4", "btrfs");
        v.mounted_at = vec![ronoctua_private::MOUNT_POINT.to_owned()];
        assert_eq!(skipped(&v, ME), Some(Skipped::Vault));
        v.mounted_at = vec!["/private-stuff".to_owned()];
        assert_eq!(skipped(&v, ME), None);
    }

    #[test]
    fn my_own_loop_is_a_row_and_somebody_elses_is_not() {
        let mut mine = block("/dev/loop20", "ext4");
        mine.holds.loop_back = true;
        mine.set_up_by = Some(ME);
        assert_eq!(skipped(&mine, ME), None);

        let mut theirs = mine;
        theirs.set_up_by = Some(SOMEBODY_ELSE);
        assert_eq!(skipped(&theirs, ME), Some(Skipped::ForeignLoop));
    }

    #[test]
    fn an_unmounted_volume_is_still_a_row() {
        // The whole reason this crate exists: a stick nobody has mounted yet
        // has no mount point, and it is exactly what somebody opened the
        // sidebar to find.
        let rack = fixture();
        let helper = rack.volumes.iter().find(|v| v.node == "/dev/sda2").unwrap();
        assert!(!helper.mounted());
        assert_eq!(skipped(helper, ME), None);
        assert_eq!(helper.place(), None);
    }

    /// An external stick holding ONE LUKS2 partition — locked, or open with
    /// what came out of it — beside the machine's own encrypted root.
    fn with_a_luks_stick(open: bool) -> Rack {
        let mut rack = fixture();
        let stick = Drive {
            at: "/drive/crypt".to_owned(),
            vendor: "Kingston".to_owned(),
            model: "DataTraveler".to_owned(),
            bytes: 16_000_000_000,
            bus: Bus::Usb,
            removable: true,
            ejectable: true,
            can_power_off: true,
            sort_key: "01hotplug/1700000001".to_owned(),
            serial: String::new(),
        };
        rack.volumes.push(Volume {
            drive: Some(stick.at.clone()),
            holds: Holding {
                partition_table: true,
                ..Holding::default()
            },
            ..block("/dev/sdb", "")
        });
        rack.volumes.push(Volume {
            drive: Some(stick.at.clone()),
            cleartext: open.then(|| "/block/dm-3".to_owned()),
            holds: Holding {
                filesystem: false,
                encrypted: true,
                partition: true,
                ..Holding::default()
            },
            bytes: 15_999_000_000,
            ..block("/dev/sdb1", "crypto_LUKS")
        });
        if open {
            rack.volumes.push(Volume {
                label: "COFRE".to_owned(),
                behind: Some("/block/sdb1".to_owned()),
                bytes: 15_980_000_000,
                // SYSTEM, as udisks marks it: a device-mapper block has no
                // drive of its own, and `update_hints` (udiskslinuxblock.c)
                // clears `HintSystem` only for a drive that is removable or
                // on USB, FireWire or a memory card. What came out of a stick
                // is "system" by that rule — and still the stick's row.
                system: true,
                ..block("/dev/dm-3", "ext4")
            });
        }
        rack.drives.push(stick);
        rack.volumes.sort_by(|a, b| a.node.cmp(&b.node));
        rack
    }

    #[test]
    fn a_locked_stick_is_a_row_and_an_open_one_gives_its_place_to_what_came_out() {
        let locked = with_a_luks_stick(false);
        let rows = nodes(&listed(&locked.volumes, ME, false));
        assert!(
            rows.contains(&"/dev/sdb1".to_owned()),
            "the locked container is not a row: {rows:?}"
        );
        let container = locked
            .volumes
            .iter()
            .find(|v| v.node == "/dev/sdb1")
            .unwrap();
        assert_eq!(container.encrypted(), Some(Encrypted::Locked));
        assert_eq!(seal_of(container, &locked.volumes), Some(Encrypted::Locked));
        // THE SYSTEM'S OWN CONTAINER STAYS A HOLDER, locked or open: nobody
        // opens the encrypted root from a sidebar.
        assert!(!rows.contains(&"/dev/nvme0n1p2".to_owned()), "{rows:?}");
        let root_container = locked
            .volumes
            .iter()
            .find(|v| v.node == "/dev/nvme0n1p2")
            .unwrap();
        assert_eq!(root_container.encrypted(), None);
        let root = locked
            .volumes
            .iter()
            .find(|v| v.node == "/dev/dm-0")
            .unwrap();
        assert_eq!(
            seal_of(root, &locked.volumes),
            None,
            "the root is not an open stick"
        );

        let open = with_a_luks_stick(true);
        let rows = nodes(&listed(&open.volumes, ME, false));
        assert!(
            !rows.contains(&"/dev/sdb1".to_owned()),
            "an open container is still a row: {rows:?}"
        );
        assert!(
            rows.contains(&"/dev/dm-3".to_owned()),
            "what came out is not a row: {rows:?}"
        );
        let clear = open.volumes.iter().find(|v| v.node == "/dev/dm-3").unwrap();
        assert_eq!(seal_of(clear, &open.volumes), Some(Encrypted::Unlocked));
        assert_eq!(
            container_of(clear, &open.volumes).map(|c| c.node.as_str()),
            Some("/dev/sdb1")
        );
        // AND IT IS FILED UNDER THE STICK, through the container.
        assert_eq!(attached_to(clear, &open.volumes), Some("/drive/crypt"));
    }

    #[test]
    fn a_locked_stick_can_be_formatted_again_and_an_open_one_cannot() {
        let locked = with_a_luks_stick(false);
        let offered = nodes(&candidates(&locked.volumes, ME));
        assert!(offered.contains(&"/dev/sdb1".to_owned()), "{offered:?}");
        assert!(
            !offered.contains(&"/dev/nvme0n1p2".to_owned()),
            "the root's container: {offered:?}"
        );
        let open = with_a_luks_stick(true);
        let offered = nodes(&candidates(&open.volumes, ME));
        assert!(
            !offered.contains(&"/dev/sdb1".to_owned()),
            "a container with a live device inside: {offered:?}"
        );
    }

    #[test]
    fn a_wrong_passphrase_is_named_by_udisks_words_and_nothing_else_is() {
        // THE PINNED libblockdev's words, which udisks puts after its own.
        assert!(wrong_passphrase(
            "Error unlocking /dev/vdc1: Failed to activate device: Incorrect passphrase."
        ));
        assert!(wrong_passphrase(
            "Error unlocking /dev/sdb1: Failed to activate device: Operation not permitted"
        ));
        assert!(wrong_passphrase("No key available with this passphrase."));
        assert!(!wrong_passphrase(
            "Error unlocking /dev/sdb1: Device /dev/sdb1 is busy"
        ));
        let e = anyhow::Error::new(WrongPassphrase).context("unlock");
        assert_eq!(refusal(&e), Refusal::WrongPassphrase);
        assert_eq!(refusal(&anyhow::anyhow!("something else")), Refusal::Other);
    }

    #[test]
    fn a_blank_stick_still_gets_a_heading() {
        // No table, no signature, no filesystem: every clause removes it. If
        // the drive went too, "I plugged it in and nothing happened" would be
        // the honest description of the sidebar.
        let drive = Drive {
            at: "/drive/blank".to_owned(),
            removable: true,
            ..Drive::default()
        };
        assert!(worth_a_heading(&drive, &[], &[]));

        let soldered = Drive {
            at: "/drive/soldered".to_owned(),
            removable: false,
            ..Drive::default()
        };
        assert!(!worth_a_heading(&soldered, &[], &[]));
    }

    #[test]
    fn a_volume_is_named_by_its_label_and_falls_back_to_its_node() {
        let rack = fixture();
        let labelled = rack.volumes.iter().find(|v| v.node == "/dev/sda2").unwrap();
        assert_eq!(labelled.name(), "MISO_EFI");
        let bare = rack.volumes.iter().find(|v| v.node == "/dev/dm-0").unwrap();
        assert_eq!(bare.name(), "/dev/dm-0");
        // And NEITHER is the object path, which carries a serial number.
        assert!(!labelled.name().contains("/drive/"));
    }

    #[test]
    fn a_drive_with_no_vendor_is_named_by_its_model_alone() {
        let rack = fixture();
        assert_eq!(rack.drives[0].name(), "an internal disk");
        assert_eq!(rack.drives[1].name(), "SanDisk Cruzer Blade");
        assert_eq!(Drive::default().name(), "");
    }

    #[test]
    fn the_bus_of_an_internal_drive_is_not_unknown() {
        // udisks answers the EMPTY STRING for anything soldered in, and reading
        // that as "unknown" would file every internal disk under the bucket
        // meant for buses nobody wrote down.
        assert_eq!(Bus::of(""), Bus::Internal);
        assert_eq!(Bus::of("usb"), Bus::Usb);
        assert_eq!(Bus::of("thunderbolt-ish"), Bus::Unknown);
    }
}
