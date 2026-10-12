//! The drive as the KERNEL says it is, right before the write — the root
//! unit's own reading, never the screen's.
//!
//! # Why a second reading
//!
//! The screen judged the drive off udisks ([`crate::drives`]) when the person
//! chose it, and the image may have taken an hour to download since. A stick
//! pulled and another plugged in gets the same `/dev/sdb`; a stick mounted
//! meanwhile is in use. So the unit reads, as root, from the kernel and
//! udev's database:
//!
//! - the name pinned when the person chose ([`crate::Target::by_id`], under
//!   `/dev/disk/by-id/`) still leads to a WHOLE disk;
//! - its size is the size chosen, and its serial (udev's `ID_SERIAL_SHORT`
//!   or `ID_SERIAL`) the serial chosen — or it [`Why::Changed`];
//! - it is on USB or a memory card, and not read-only — and an `mmcblk` is
//!   a card only when the kernel calls it one (`device/type` is `SD`, or the
//!   disk is removable): the eMMC soldered to the board is an `mmcblk` too,
//!   and `DeviceAllow=block-mmc` lets the unit reach it;
//! - nothing on it is mounted (PID 1's mount table, the host's and not the
//!   unit's namespace), swapped on, or held (an open LUKS, LVM);
//! - and it is not the disk the firmware loaded this system from
//!   (`LoaderDevicePartUUID`, which systemd-boot sets) — or it is
//!   [`Why::NotWritable`].
//!
//! Then `O_EXCL` on the open is the kernel's own "nobody holds it", and the
//! opened descriptor's device number is compared with the one read here.
//!
//! The reading ([`probe`]) is separate from the judgement ([`judge`]), and
//! every root it reads under is a parameter ([`Roots`]): the tests judge
//! planted readings, and the VM test the real thing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{Target, Why};

/// Where a stable name of a whole drive lives.
pub const BY_ID: &str = "/dev/disk/by-id/";

/// systemd-boot's variable naming the partition the system was loaded from.
pub const LOADER_PART_UUID: &str = "LoaderDevicePartUUID-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f";

/// Whether `path` is a name under [`BY_ID`]: one component, no `..`.
#[must_use]
pub fn is_by_id(path: &Path) -> bool {
    path.to_str()
        .and_then(|p| p.strip_prefix(BY_ID))
        .is_some_and(|name| {
            !name.is_empty()
                && !name.contains('/')
                && name != "."
                && name != ".."
                && !name.contains('\0')
        })
}

/// What the unit reads, and from where. [`Roots::system`] in the unit.
#[derive(Debug, Clone)]
pub struct Roots {
    /// `/sys`.
    pub sys: PathBuf,
    /// `/run/udev/data`.
    pub udev: PathBuf,
    /// PID 1's mount table (`/proc/1/mountinfo`): the host's, whatever
    /// namespace the unit runs in.
    pub mountinfo: PathBuf,
    /// `/proc/swaps`.
    pub swaps: PathBuf,
    /// `/sys/firmware/efi/efivars`.
    pub efivars: PathBuf,
    /// `/dev/disk/by-partuuid`.
    pub by_partuuid: PathBuf,
}

impl Roots {
    /// The machine's own.
    #[must_use]
    pub fn system() -> Self {
        Self {
            sys: "/sys".into(),
            udev: "/run/udev/data".into(),
            mountinfo: "/proc/1/mountinfo".into(),
            swaps: "/proc/swaps".into(),
            efivars: "/sys/firmware/efi/efivars".into(),
            by_partuuid: "/dev/disk/by-partuuid".into(),
        }
    }
}

/// A block device as the kernel and udev describe it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Probe {
    /// The kernel's name: `sdb`, `mmcblk0`.
    pub name: String,
    /// `major:minor`.
    pub dev: (u32, u32),
    /// A whole disk (it is under `/sys/block`), not a partition.
    pub whole: bool,
    /// Its size, in bytes.
    pub size_bytes: u64,
    /// The medium is read-only.
    pub read_only: bool,
    /// On USB, or a memory card.
    pub usb_or_card: bool,
    /// udev's serials for it: `ID_SERIAL_SHORT`, `ID_SERIAL`.
    pub serials: Vec<String>,
    /// It, or a partition of it, is mounted.
    pub mounted: bool,
    /// A partition of it is swap.
    pub swap: bool,
    /// Something holds it or a partition of it (an open LUKS, LVM, RAID).
    pub held: bool,
    /// The firmware loaded this system from a partition of it.
    pub boot: bool,
}

/// Whether the unit may write `target` onto the device `probe` describes.
///
/// # Errors
///
/// [`Why::Changed`] when it is not the drive chosen (size, serial);
/// [`Why::NotWritable`] when it is no longer one that may be written.
pub fn judge(probe: &Probe, target: &Target) -> Result<(), Why> {
    if !probe.whole {
        return Err(Why::NotWritable);
    }
    if probe.size_bytes != target.size_bytes {
        return Err(Why::Changed);
    }
    if let Some(serial) = target.serial.as_deref()
        && !probe.serials.iter().any(|s| s == serial)
    {
        return Err(Why::Changed);
    }
    if !probe.usb_or_card
        || probe.read_only
        || probe.mounted
        || probe.swap
        || probe.held
        || probe.boot
    {
        return Err(Why::NotWritable);
    }
    Ok(())
}

/// The kernel's name of the disk a `/dev/disk/by-id/` link leads to.
///
/// # Errors
///
/// Not a name under [`BY_ID`], a link that leads nowhere, or somewhere that
/// is not a plain device name under `/dev`.
pub fn resolve(by_id: &Path) -> std::io::Result<String> {
    let refused = |why: &str| std::io::Error::new(std::io::ErrorKind::InvalidInput, why.to_owned());
    if !is_by_id(by_id) {
        return Err(refused("not a /dev/disk/by-id/ name"));
    }
    let real = std::fs::canonicalize(by_id)?;
    let name = real
        .strip_prefix("/dev")
        .ok()
        .and_then(|rest| rest.to_str())
        .filter(|n| is_kernel_name(n))
        .ok_or_else(|| refused("the link does not lead to a device of /dev"))?;
    Ok(name.to_owned())
}

/// A kernel block name: lower-case letters and digits, nothing else.
#[must_use]
pub fn is_kernel_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|t| t.trim().to_owned())
}

/// `major:minor`, as `/sys/…/dev` and the mount table write it.
#[must_use]
pub fn parse_dev(text: &str) -> Option<(u32, u32)> {
    let (major, minor) = text.trim().split_once(':')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// udev's database entry for a device (`E:KEY=VALUE` lines), as a map.
#[must_use]
pub fn parse_udev(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("E:"))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// Every device number the mount table names (its third field).
#[must_use]
pub fn mounted_devices(mountinfo: &str) -> Vec<(u32, u32)> {
    mountinfo
        .lines()
        .filter_map(|l| l.split_whitespace().nth(2))
        .filter_map(parse_dev)
        .collect()
}

/// The kernel names `/proc/swaps` lists (its first column, past the header).
#[must_use]
pub fn swapped_names(swaps: &str) -> Vec<String> {
    swaps
        .lines()
        .skip(1)
        .filter_map(|l| l.split_whitespace().next())
        .filter_map(|p| p.strip_prefix("/dev/"))
        .map(ToOwned::to_owned)
        .collect()
}

/// The partition UUID in systemd-boot's variable: four bytes of attributes,
/// then UTF-16LE text, NUL-terminated — lower-cased, as
/// `/dev/disk/by-partuuid` names it.
#[must_use]
pub fn loader_partuuid(bytes: &[u8]) -> Option<String> {
    let units: Vec<u16> = bytes
        .get(4..)?
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|u| *u != 0)
        .collect();
    let text = String::from_utf16(&units).ok()?.to_ascii_lowercase();
    (text.len() == 36 && text.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')).then_some(text)
}

/// Reads what [`judge`] judges, for the whole disk `name`.
///
/// # Errors
///
/// `name` is not a whole disk the kernel knows (`/sys/block/<name>`), or
/// its numbers could not be read.
pub fn probe(name: &str, roots: &Roots) -> std::io::Result<Probe> {
    let invalid = |why: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, why.to_owned());
    if !is_kernel_name(name) {
        return Err(invalid("not a kernel block name"));
    }
    let block = roots.sys.join("block").join(name);
    if !block.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not a whole disk",
        ));
    }
    let dev = read_trimmed(&block.join("dev"))
        .and_then(|t| parse_dev(&t))
        .ok_or_else(|| invalid("no device number"))?;
    let sectors: u64 = read_trimmed(&block.join("size"))
        .and_then(|t| t.parse().ok())
        .ok_or_else(|| invalid("no size"))?;
    let udev = std::fs::read_to_string(roots.udev.join(format!("b{}:{}", dev.0, dev.1)))
        .map(|t| parse_udev(&t))
        .unwrap_or_default();
    // THE PATH THROUGH THE BUSES, which says USB even when udev's database
    // was not read: `/sys/block/sdb` is a link into `…/usb2/2-1/…`.
    let through_usb = std::fs::canonicalize(&block)
        .ok()
        .is_some_and(|p| p.to_string_lossy().contains("/usb"));
    // A CARD AND NOT AN eMMC: both are `mmcblk`, and the kernel's own word
    // for the medium (`device/type`: `SD`, `MMC`, `SDIO`) or its removable
    // flag is what tells the slot from the board.
    let card = name.starts_with("mmcblk")
        && (read_trimmed(&block.join("device/type")).is_some_and(|t| t == "SD")
            || read_trimmed(&block.join("removable")).is_some_and(|t| t == "1"));
    let usb_or_card = udev.get("ID_BUS").is_some_and(|b| b == "usb") || through_usb || card;
    let serials: Vec<String> = ["ID_SERIAL_SHORT", "ID_SERIAL"]
        .iter()
        .filter_map(|k| udev.get(*k))
        .filter(|s| !s.is_empty())
        .cloned()
        .collect();

    // THE PARTITIONS: the folders under it with a `partition` file.
    let mut parts: Vec<(String, (u32, u32))> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&block) {
        for e in entries.flatten() {
            let p = e.path();
            if p.join("partition").is_file()
                && let Some(d) = read_trimmed(&p.join("dev")).and_then(|t| parse_dev(&t))
            {
                parts.push((e.file_name().to_string_lossy().into_owned(), d));
            }
        }
    }
    let holds =
        |dir: &Path| std::fs::read_dir(dir.join("holders")).is_ok_and(|mut d| d.next().is_some());
    let held = holds(&block) || parts.iter().any(|(n, _)| holds(&block.join(n)));

    let mounts = std::fs::read_to_string(&roots.mountinfo)
        .map(|t| mounted_devices(&t))
        .map_err(|_| invalid("the mount table could not be read"))?;
    let mounted = mounts
        .iter()
        .any(|d| *d == dev || parts.iter().any(|(_, p)| p == d));
    let swapped = std::fs::read_to_string(&roots.swaps)
        .map(|t| swapped_names(&t))
        .unwrap_or_default();
    let swap = swapped
        .iter()
        .any(|s| s == name || parts.iter().any(|(n, _)| n == s));

    // THE DISK THE SYSTEM WAS LOADED FROM, even with its ESP unmounted
    // (an automounted `/boot` that nobody touched yet).
    let boot = std::fs::read(roots.efivars.join(LOADER_PART_UUID))
        .ok()
        .and_then(|b| loader_partuuid(&b))
        .and_then(|uuid| std::fs::canonicalize(roots.by_partuuid.join(uuid)).ok())
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .is_some_and(|part| parts.iter().any(|(n, _)| *n == part));

    Ok(Probe {
        name: name.to_owned(),
        dev,
        whole: true,
        size_bytes: sectors.saturating_mul(512),
        read_only: read_trimmed(&block.join("ro")).is_some_and(|t| t == "1"),
        usb_or_card,
        serials,
        mounted,
        swap,
        held,
        boot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            drive: "/drive/b".to_owned(),
            by_id: "/dev/disk/by-id/usb-Vendor_Model_SERIAL-0:0".into(),
            serial: Some("SERIAL".to_owned()),
            size_bytes: 32 << 30,
            label: "Vendor Model".to_owned(),
        }
    }

    fn good() -> Probe {
        Probe {
            name: "sdb".to_owned(),
            dev: (8, 16),
            whole: true,
            size_bytes: 32 << 30,
            usb_or_card: true,
            serials: vec!["SERIAL".to_owned(), "Vendor_Model_SERIAL".to_owned()],
            ..Probe::default()
        }
    }

    /// THE ROOT UNIT'S HALF of `stick-writes-only-removable`: the stick
    /// chosen is written, and each way a device stops being one that may be
    /// written — or stops being the one chosen — is refused, by name.
    #[test]
    fn writes_only_removable_devices_as_root_reads_them() {
        assert_eq!(judge(&good(), &target()), Ok(()));
        let refused = |change: fn(&mut Probe)| {
            let mut p = good();
            change(&mut p);
            judge(&p, &target())
        };
        assert_eq!(
            refused(|p| p.usb_or_card = false),
            Err(Why::NotWritable),
            "internal"
        );
        assert_eq!(
            refused(|p| p.mounted = true),
            Err(Why::NotWritable),
            "mounted"
        );
        assert_eq!(
            refused(|p| p.boot = true),
            Err(Why::NotWritable),
            "the system's"
        );
        assert_eq!(refused(|p| p.swap = true), Err(Why::NotWritable), "swap");
        assert_eq!(refused(|p| p.held = true), Err(Why::NotWritable), "held");
        assert_eq!(refused(|p| p.read_only = true), Err(Why::NotWritable));
        assert_eq!(refused(|p| p.whole = false), Err(Why::NotWritable));
        assert_eq!(
            refused(|p| p.size_bytes = 8 << 30),
            Err(Why::Changed),
            "another stick"
        );
        assert_eq!(
            refused(|p| p.serials = vec!["OTHER".to_owned()]),
            Err(Why::Changed)
        );
    }

    #[test]
    fn the_readings_parse_as_the_kernel_writes_them() {
        assert_eq!(parse_dev("8:16\n"), Some((8, 16)));
        assert_eq!(parse_dev("8"), None);
        let udev =
            parse_udev("S:disk/by-id/usb-x\nE:ID_BUS=usb\nE:ID_SERIAL_SHORT=ABC\nG:systemd\n");
        assert_eq!(udev.get("ID_BUS").map(String::as_str), Some("usb"));
        assert_eq!(udev.get("ID_SERIAL_SHORT").map(String::as_str), Some("ABC"));
        let info = "22 1 0:21 / /proc rw - proc proc rw\n\
                    30 1 8:17 / /run/media/p/STICK rw - vfat /dev/sdb1 rw\n";
        assert_eq!(mounted_devices(info), vec![(0, 21), (8, 17)]);
        let swaps = "Filename Type Size Used Priority\n/dev/sdb2 partition 1 0 -2\n";
        assert_eq!(swapped_names(swaps), ["sdb2"]);
        let mut var = vec![7, 0, 0, 0];
        for u in "9F1C3E2A-0B7D-4E55-9A51-2C8D7E6F1A0B"
            .encode_utf16()
            .chain([0])
        {
            var.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(
            loader_partuuid(&var).as_deref(),
            Some("9f1c3e2a-0b7d-4e55-9a51-2c8d7e6f1a0b")
        );
        assert_eq!(loader_partuuid(&[0, 0, 0, 0, b'x', 0]), None);
        assert!(is_by_id(Path::new("/dev/disk/by-id/usb-x")));
        assert!(!is_by_id(Path::new("/dev/disk/by-id/../../sda")));
        assert!(!is_by_id(Path::new("/dev/sda")));
        assert!(!is_kernel_name("../sda") && is_kernel_name("mmcblk0"));
    }

    /// THE ROOT UNIT'S HALF FOR CARDS (`stick-writes-only-removable`): an
    /// `mmcblk` the kernel calls an SD card is written; the eMMC soldered to
    /// the board — an `mmcblk` too, reachable through `block-mmc` — is
    /// refused, unmounted and unheld as it may be.
    #[test]
    fn writes_only_removable_cards_never_an_emmc() {
        let root = std::env::temp_dir().join(format!("ronoctua-stick-emmc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let block = root.join("sys/block/mmcblk0");
        std::fs::create_dir_all(block.join("device")).unwrap();
        std::fs::write(block.join("dev"), "179:0\n").unwrap();
        std::fs::write(block.join("size"), "61071360\n").unwrap();
        std::fs::write(block.join("ro"), "0\n").unwrap();
        std::fs::write(block.join("removable"), "0\n").unwrap();
        std::fs::write(block.join("device/type"), "MMC\n").unwrap();
        std::fs::create_dir_all(root.join("udev")).unwrap();
        std::fs::write(
            root.join("mountinfo"),
            "22 1 0:21 / /proc rw - proc proc rw\n",
        )
        .unwrap();
        std::fs::write(root.join("swaps"), "Filename Type Size Used Priority\n").unwrap();
        let roots = Roots {
            sys: root.join("sys"),
            udev: root.join("udev"),
            mountinfo: root.join("mountinfo"),
            swaps: root.join("swaps"),
            efivars: root.join("efivars"),
            by_partuuid: root.join("by-partuuid"),
        };
        let mut t = target();
        t.serial = None;
        let p = probe("mmcblk0", &roots).unwrap();
        t.size_bytes = p.size_bytes;
        assert!(!p.usb_or_card, "an eMMC is not a card");
        assert_eq!(judge(&p, &t), Err(Why::NotWritable), "the board's own disk");
        std::fs::write(block.join("device/type"), "SD\n").unwrap();
        let p = probe("mmcblk0", &roots).unwrap();
        assert!(p.usb_or_card, "an SD card");
        assert_eq!(judge(&p, &t), Ok(()));
        std::fs::write(block.join("device/type"), "MMC\n").unwrap();
        std::fs::write(block.join("removable"), "1\n").unwrap();
        assert!(
            probe("mmcblk0", &roots).unwrap().usb_or_card,
            "an MMC card in a slot the kernel calls removable"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A WHOLE PLANTED MACHINE, read as the unit reads its own: a stick with
    /// a mounted partition, then the same stick unmounted.
    #[test]
    fn a_planted_machine_is_read_whole() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-stick-probe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let block = root.join("sys/block/sdb");
        std::fs::create_dir_all(block.join("sdb1/holders")).unwrap();
        std::fs::create_dir_all(block.join("holders")).unwrap();
        std::fs::write(block.join("dev"), "8:16\n").unwrap();
        std::fs::write(block.join("size"), "62500000\n").unwrap();
        std::fs::write(block.join("ro"), "0\n").unwrap();
        std::fs::write(block.join("sdb1/partition"), "1\n").unwrap();
        std::fs::write(block.join("sdb1/dev"), "8:17\n").unwrap();
        std::fs::create_dir_all(root.join("udev")).unwrap();
        std::fs::write(
            root.join("udev/b8:16"),
            "E:ID_BUS=usb\nE:ID_SERIAL_SHORT=SERIAL\n",
        )
        .unwrap();
        std::fs::write(
            root.join("mountinfo"),
            "30 1 8:17 / /run/media/p/X rw - vfat /dev/sdb1 rw\n",
        )
        .unwrap();
        std::fs::write(root.join("swaps"), "Filename Type Size Used Priority\n").unwrap();
        let roots = Roots {
            sys: root.join("sys"),
            udev: root.join("udev"),
            mountinfo: root.join("mountinfo"),
            swaps: root.join("swaps"),
            efivars: root.join("efivars"),
            by_partuuid: root.join("by-partuuid"),
        };
        let p = probe("sdb", &roots).unwrap();
        assert_eq!(p.size_bytes, 62_500_000 * 512);
        assert!(p.usb_or_card && p.mounted && !p.held && !p.swap && !p.boot);
        assert_eq!(p.serials, ["SERIAL"]);
        let mut t = target();
        t.size_bytes = p.size_bytes;
        assert_eq!(judge(&p, &t), Err(Why::NotWritable));
        std::fs::write(
            root.join("mountinfo"),
            "22 1 0:21 / /proc rw - proc proc rw\n",
        )
        .unwrap();
        let p = probe("sdb", &roots).unwrap();
        assert_eq!(judge(&p, &t), Ok(()));
        // AN OPEN LUKS on the partition holds it.
        std::fs::create_dir_all(block.join("sdb1/holders/dm-3")).unwrap();
        assert!(probe("sdb", &roots).unwrap().held);
        assert!(
            probe("sdb1", &roots).is_err(),
            "a partition is not a whole disk"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
