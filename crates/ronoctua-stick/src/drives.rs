//! Which drives the installer may be written to — read off the disk border's
//! rack, never off a second reading of udisks.
//!
//! # A drive a person calls a stick is LISTED, and refused with a reason
//!
//! A stick that is plugged in and does not appear is the worst answer a
//! screen can give ("I plugged it in and nothing happened"), and a stick that
//! appears and silently cannot be chosen is the second worst. So every drive
//! that is removable, or on USB, is a [`Judged`]: a [`Target`] and, when it
//! may not be written, the [`Unfit`] that says why — mounted, open, the
//! system's, read-only, nameless. Soldered-in disks are not listed at all:
//! the screen is about sticks, and the refusal for an internal disk would be
//! a line of noise per disk.
//!
//! # A memory card, never the eMMC beside it
//!
//! udisks calls EVERY `mmcblk` drive's bus `sdio`: the card in the slot and
//! the eMMC soldered to the board — a laptop's internal disk, with a factory
//! system on it. What tells them apart is udisks' `Removable`, which it sets
//! for an SD card and not for an eMMC. So the bus alone says "stick" only for
//! USB; on `sdio` the drive must also be removable ([`is_a_stick`]). The root
//! writer asks the kernel the same question (`crate::device`).
//!
//! # Size is the release's question
//!
//! Whether the image FITS depends on the release, which this does not know;
//! [`Target::fits`] answers it once the release is read.
//!
//! # The root writer asks again
//!
//! This is what the SCREEN offers. The root unit does not trust it: it reads
//! the kernel's own answers right before it writes (`crate::device`), and a
//! drive that changed in between is refused there.

use ronoctua_disks::{Bus, Drive, Rack, Volume};

use crate::Target;

/// Why a drive that is listed may not be written, as a closed set
/// (`stick-unfit-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unfit {
    /// Not on USB nor a memory card: removable, but on a bus this does not
    /// write (FireWire, a bay).
    Bus,
    /// The running system needs a volume on it (mounted where the system
    /// lives, in `fstab`, swap).
    System,
    /// A filesystem on it is mounted: unmount it first.
    Mounted,
    /// An encrypted volume on it is open: lock it first.
    Open,
    /// The medium is read-only (a switch on the card, a write-protected
    /// stick).
    ReadOnly,
    /// It has no stable name under `/dev/disk/by-id/`, so a write could not
    /// be pinned to it.
    Unnamed,
}

impl Unfit {
    /// Every one.
    pub const ALL: [Self; 6] = [
        Self::Bus,
        Self::System,
        Self::Mounted,
        Self::Open,
        Self::ReadOnly,
        Self::Unnamed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Bus => "bus",
            Self::System => "system",
            Self::Mounted => "mounted",
            Self::Open => "open",
            Self::ReadOnly => "read-only",
            Self::Unnamed => "unnamed",
        }
    }
}

/// One drive as the screen offers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    /// The drive, as a write would name it.
    pub target: Target,
    /// Why it may not be written; `None` when it may.
    pub unfit: Option<Unfit>,
}

/// Whether a person would call this drive a stick: something one can pull
/// out (a memory card is that, an eMMC on the same bus is not), or anything
/// on USB. Zero bytes is a card reader with no card. The file manager's
/// sidebar gives a heading to the same drives.
#[must_use]
pub fn is_a_stick(d: &Drive) -> bool {
    d.bytes > 0 && (d.removable || matches!(d.bus, Bus::Usb))
}

/// The block of the WHOLE drive — the one udisks always publishes, a blank
/// stick's included.
fn whole_of<'a>(d: &Drive, all: &'a [Volume]) -> Option<&'a Volume> {
    all.iter()
        .find(|v| v.drive.as_deref() == Some(d.at.as_str()) && v.whole())
}

/// The stable name of a whole drive: a `/dev/disk/by-id/` link that is not a
/// partition's, the bus's own (`usb-`, `mmc-`) first.
#[must_use]
pub fn by_id_of(v: &Volume) -> Option<&str> {
    let mut ids: Vec<&str> = v
        .symlinks
        .iter()
        .map(String::as_str)
        .filter(|l| {
            l.strip_prefix(crate::device::BY_ID).is_some_and(|name| {
                !name.is_empty() && !name.contains('/') && !name.contains("-part")
            })
        })
        .collect();
    ids.sort_by_key(|l| {
        let name = l.trim_start_matches(crate::device::BY_ID);
        (!(name.starts_with("usb-") || name.starts_with("mmc-")), *l)
    });
    ids.first().copied()
}

/// Every drive a person would call a stick, judged — in udisks' order.
#[must_use]
pub fn judge(rack: &Rack) -> Vec<Judged> {
    rack.drives
        .iter()
        .filter(|d| is_a_stick(d))
        .map(|d| judged(d, rack))
        .collect()
}

/// The drives that may be written (size aside — [`Target::fits`]).
#[must_use]
pub fn targets(rack: &Rack) -> Vec<Target> {
    judge(rack)
        .into_iter()
        .filter(|j| j.unfit.is_none())
        .map(|j| j.target)
        .collect()
}

fn judged(d: &Drive, rack: &Rack) -> Judged {
    let all = &rack.volumes;
    let whole = whole_of(d, all);
    let on_it: Vec<&Volume> = all
        .iter()
        .filter(|v| ronoctua_disks::attached_to(v, all) == Some(d.at.as_str()))
        .collect();
    let by_id = whole.and_then(by_id_of);
    let label = match d.name() {
        name if !name.is_empty() => name,
        _ => whole.map_or_else(String::new, |w| w.node.clone()),
    };
    let target = Target {
        drive: d.at.clone(),
        by_id: by_id.unwrap_or_default().into(),
        serial: (!d.serial.trim().is_empty()).then(|| d.serial.trim().to_owned()),
        size_bytes: whole.map_or(d.bytes, |w| w.bytes.max(d.bytes)),
        label,
    };
    // THE ORDER IS THE ORDER OF WHAT A PERSON CAN DO ABOUT IT: nothing about
    // the system's disk, then what a key fixes (unmount, lock), then what a
    // hand fixes (the switch), then what nobody can.
    let unfit = if on_it.iter().any(|v| ronoctua_disks::needed(v).is_some()) {
        Some(Unfit::System)
    } else if !matches!(d.bus, Bus::Usb | Bus::Sdio) {
        Some(Unfit::Bus)
    } else if on_it.iter().any(|v| v.mounted()) {
        Some(Unfit::Mounted)
    } else if on_it
        .iter()
        .any(|v| v.cleartext.is_some() || ronoctua_disks::container_of(v, all).is_some())
    {
        Some(Unfit::Open)
    } else if whole.is_none_or(|w| w.read_only) {
        Some(Unfit::ReadOnly)
    } else if by_id.is_none() {
        Some(Unfit::Unnamed)
    } else {
        None
    };
    Judged { target, unfit }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_disks::Holding;

    /// A drive and its whole block, as udisks publishes them.
    fn stick(rack: &mut Rack, n: &str, bus: Bus, removable: bool, bytes: u64) {
        rack.drives.push(Drive {
            at: format!("/drive/{n}"),
            vendor: "Vendor".to_owned(),
            model: format!("Model {n}"),
            bytes,
            bus,
            removable,
            serial: format!("SERIAL{n}"),
            sort_key: format!("01hotplug/{n}"),
            ..Drive::default()
        });
        rack.volumes.push(Volume {
            at: format!("/block/sd{n}"),
            node: format!("/dev/sd{n}"),
            bytes,
            drive: Some(format!("/drive/{n}")),
            symlinks: vec![
                format!("/dev/disk/by-path/pci-0000:00:14.0-usb-0:{n}:1.0-scsi-0:0:0:0"),
                format!("/dev/disk/by-id/usb-Vendor_Model_{n}_SERIAL{n}-0:0"),
            ],
            ..Volume::default()
        });
    }

    fn partition(rack: &mut Rack, n: &str, mounted_at: Option<&str>) {
        rack.volumes.push(Volume {
            at: format!("/block/sd{n}1"),
            node: format!("/dev/sd{n}1"),
            bytes: 1,
            id_type: "vfat".to_owned(),
            drive: Some(format!("/drive/{n}")),
            mounted_at: mounted_at.map(ToOwned::to_owned).into_iter().collect(),
            holds: Holding {
                filesystem: true,
                partition: true,
                ..Holding::default()
            },
            symlinks: vec![format!(
                "/dev/disk/by-id/usb-Vendor_Model_{n}_SERIAL{n}-0:0-part1"
            )],
            ..Volume::default()
        });
    }

    /// THE FIXTURE RACK: every case the screen must tell apart.
    fn fixture() -> Rack {
        let mut rack = Rack::default();
        // The machine's own disk: soldered in and the system's.
        rack.drives.push(Drive {
            at: "/drive/nvme".to_owned(),
            model: "an internal disk".to_owned(),
            bytes: 2_000_000_000_000,
            bus: Bus::Internal,
            sort_key: "00coldplug/00fixed/nvme0".to_owned(),
            ..Drive::default()
        });
        rack.volumes.push(Volume {
            at: "/block/nvme0n1p1".to_owned(),
            node: "/dev/nvme0n1p1".to_owned(),
            mounted_at: vec!["/boot".to_owned()],
            drive: Some("/drive/nvme".to_owned()),
            holds: Holding {
                filesystem: true,
                partition: true,
                ..Holding::default()
            },
            ..Volume::default()
        });
        stick(&mut rack, "b", Bus::Usb, true, 32_000_000_000); // blank: the one
        stick(&mut rack, "c", Bus::Usb, true, 16_000_000_000);
        partition(&mut rack, "c", Some("/run/media/p/STICK")); // mounted
        stick(&mut rack, "d", Bus::Usb, false, 1_000_000_000); // small, fixed-flag USB
        stick(&mut rack, "e", Bus::Usb, true, 64_000_000_000);
        partition(&mut rack, "e", Some("/")); // a stick the system runs from
        stick(&mut rack, "f", Bus::Ieee1394, true, 64_000_000_000); // FireWire
        stick(&mut rack, "g", Bus::Sdio, true, 8_000_000_000);
        if let Some(v) = rack.volumes.iter_mut().find(|v| v.node == "/dev/sdg") {
            v.read_only = true; // the card's switch
        }
        stick(&mut rack, "h", Bus::Usb, true, 8_000_000_000);
        if let Some(v) = rack.volumes.iter_mut().find(|v| v.node == "/dev/sdh") {
            v.symlinks.retain(|l| !l.contains("by-id")); // no stable name
        }
        stick(&mut rack, "i", Bus::Usb, true, 0); // a reader with no card
        // AN eMMC: udisks says `sdio` for it as for a card, and not removable.
        stick(&mut rack, "k", Bus::Sdio, false, 32_000_000_000);
        if let Some(v) = rack.volumes.iter_mut().find(|v| v.node == "/dev/sdk") {
            v.symlinks = vec!["/dev/disk/by-id/mmc-H8G4a_0x1234".to_owned()];
        }
        rack
    }

    fn unfit_of(judged: &[Judged], n: &str) -> Option<Option<Unfit>> {
        judged
            .iter()
            .find(|j| j.target.drive == format!("/drive/{n}"))
            .map(|j| j.unfit)
    }

    /// THE QUESTION `stick-writes-only-removable` ASKS: of a rack with the
    /// system's disk, a mounted stick, a stick the system runs from, a
    /// FireWire drive, a read-only card, a nameless stick, an empty reader
    /// and a soldered eMMC on the cards' bus, only the blank and the small
    /// sticks are offered — and the small one only for an image that fits.
    #[test]
    fn writes_only_removable_targets_from_a_rack() {
        let rack = fixture();
        let judged = judge(&rack);
        assert_eq!(
            unfit_of(&judged, "nvme"),
            None,
            "a soldered disk is not listed"
        );
        assert_eq!(
            unfit_of(&judged, "i"),
            None,
            "an empty reader is not listed"
        );
        assert_eq!(
            unfit_of(&judged, "k"),
            None,
            "an eMMC is not a memory card, whatever its bus"
        );
        assert_eq!(unfit_of(&judged, "b"), Some(None));
        assert_eq!(unfit_of(&judged, "c"), Some(Some(Unfit::Mounted)));
        assert_eq!(unfit_of(&judged, "d"), Some(None), "USB, whatever its flag");
        assert_eq!(unfit_of(&judged, "e"), Some(Some(Unfit::System)));
        assert_eq!(unfit_of(&judged, "f"), Some(Some(Unfit::Bus)));
        assert_eq!(unfit_of(&judged, "g"), Some(Some(Unfit::ReadOnly)));
        assert_eq!(unfit_of(&judged, "h"), Some(Some(Unfit::Unnamed)));

        let offered = targets(&rack);
        let names: Vec<&str> = offered.iter().map(|t| t.drive.as_str()).collect();
        assert_eq!(names, ["/drive/b", "/drive/d"]);
        let blank = &offered[0];
        assert_eq!(
            blank.by_id.to_str(),
            Some("/dev/disk/by-id/usb-Vendor_Model_b_SERIALb-0:0"),
            "the bus's own link, never a partition's or a path's"
        );
        assert_eq!(blank.serial.as_deref(), Some("SERIALb"));
        assert_eq!(blank.label, "Vendor Model b");
        // TOO SMALL is the release's question.
        let image = 3_000_000_000;
        let fitting: Vec<&str> = offered
            .iter()
            .filter(|t| t.fits(image))
            .map(|t| t.drive.as_str())
            .collect();
        assert_eq!(fitting, ["/drive/b"]);
    }

    #[test]
    fn an_open_encrypted_stick_is_locked_first() {
        let mut rack = Rack::default();
        stick(&mut rack, "j", Bus::Usb, true, 8_000_000_000);
        rack.volumes.push(Volume {
            at: "/block/sdj1".to_owned(),
            node: "/dev/sdj1".to_owned(),
            id_type: "crypto_LUKS".to_owned(),
            drive: Some("/drive/j".to_owned()),
            cleartext: Some("/block/dm-4".to_owned()),
            holds: Holding {
                encrypted: true,
                partition: true,
                ..Holding::default()
            },
            ..Volume::default()
        });
        assert_eq!(judge(&rack)[0].unfit, Some(Unfit::Open));
        assert!(targets(&rack).is_empty());
    }
}
