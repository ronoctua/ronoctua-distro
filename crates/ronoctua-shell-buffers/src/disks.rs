//! The disks, as the formatting buffer receives them.
//!
//! # Why the translation happens HERE and not over there
//!
//! A wasip2 component gets no bundle and no locale (invariant 11), so every
//! word it draws either arrives as text or is drawn by the host. `Device::needed`
//! is the one field of the record that is a SENTENCE rather than a fact, and
//! this module is where the fact becomes one.
//!
//! It is the mirror of `ronoctua_shell_work::jobs::queue_of`, for the same reason: the
//! shape the wire wants is not the shape udisks gives, and the conversion is
//! worth reading in one place instead of inside the event loop.

use crate::key as k;

/// The rack as the buffer receives it.
///
/// # What `needed` being non-empty means downstream
///
/// It is the only thing standing between the buffer and a disk that must not be
/// touched — on the BUFFER's side. On this side the same judgement is made
/// again, in [`crate::buffer`], against the volume rather than against the
/// string. Two checks and not one because the buffer is a separate binary: a
/// guard that lives only inside the thing being guarded holds exactly until
/// somebody writes a second buffer.
#[must_use]
pub fn rack_of(
    rack: &ronoctua_disks::Rack,
    uid: u32,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::Rack {
    ronoctua_protocol::Rack {
        devices: ronoctua_disks::candidates(&rack.volumes, uid)
            .into_iter()
            .map(|v| ronoctua_protocol::Device {
                id: v.at.clone(),
                // The label or the device node — NEVER the object path, which
                // carries the drive's serial number (invariant 13). The id goes
                // over as well, and it is the one field the buffer never draws.
                name: v.name().to_owned(),
                node: v.node.clone(),
                kind: v.id_type.clone(),
                bytes: v.bytes,
                mounted: v.mounted(),
                // FROM THE DRIVE and not from the volume: "can this be pulled
                // out" is a property of the thing it sits on. A volume whose
                // drive is gone — a device-mapper node, a loop — is not
                // removable, which is the right answer for both.
                removable: ronoctua_disks::attached_to(v, &rack.volumes)
                    .and_then(|at| rack.drives.iter().find(|d| d.at == at))
                    .is_some_and(|d| d.removable),
                needed: why_not(v, texts),
                whole: v.whole(),
            })
            .collect(),
    }
}

/// Why this one must not be written, as a sentence — empty when it is spare.
fn why_not(v: &ronoctua_disks::Volume, texts: &ronoctua_i18n::Texts) -> String {
    match ronoctua_disks::needed(v) {
        None => String::new(),
        // THE MOUNT POINT IS IN THE SENTENCE, and that is the whole point of
        // `Needed::At` carrying it: "the system needs this" is an assertion a
        // person can only take on faith, and "`/boot` is mounted here" is one
        // they can check against what they already know.
        Some(ronoctua_disks::Needed::At(at)) => texts.t_with(k::FORMAT_NEEDED_AT, &[("at", &at)]),
        Some(ronoctua_disks::Needed::InFstab) => texts.t(k::FORMAT_NEEDED_FSTAB),
        Some(ronoctua_disks::Needed::Swap) => texts.t(k::FORMAT_NEEDED_SWAP),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(node: &str, kind: &str) -> ronoctua_disks::Volume {
        ronoctua_disks::Volume {
            at: format!("/block/{node}"),
            node: format!("/dev/{node}"),
            id_type: kind.to_owned(),
            bytes: 1024,
            holds: ronoctua_disks::Holding {
                filesystem: true,
                partition: true,
                ..ronoctua_disks::Holding::default()
            },
            ..ronoctua_disks::Volume::default()
        }
    }

    #[test]
    fn a_spare_partition_crosses_with_nothing_in_the_way() {
        let rack = ronoctua_disks::Rack {
            volumes: vec![volume("sda1", "vfat")],
            drives: Vec::new(),
        };
        let over = rack_of(&rack, 1000, &ronoctua_i18n::Texts::from_env());
        assert_eq!(over.devices.len(), 1);
        assert_eq!(over.devices[0].node, "/dev/sda1");
        assert_eq!(over.devices[0].kind, "vfat");
        // Spare, so nothing stands in the way — and `whole` is false, because
        // it carries the `Partition` interface.
        assert!(over.devices[0].needed.is_empty());
        assert!(!over.devices[0].whole);
    }

    #[test]
    fn an_empty_partition_is_a_candidate_and_not_a_holder() {
        // THE ONE CASE `HOLDERS` WOULD HAVE EATEN. That list calls an empty
        // `id_type` a holder, because a block with nothing on it is nothing to
        // MOUNT. To formatting it is the most ordinary target there is, and
        // reusing the mount-side list here would have hidden exactly the row
        // somebody opened this screen to find.
        let mut blank = volume("sdb1", "");
        blank.holds.filesystem = false;
        let rack = ronoctua_disks::Rack {
            volumes: vec![blank],
            drives: Vec::new(),
        };
        let over = rack_of(&rack, 1000, &ronoctua_i18n::Texts::from_env());
        assert_eq!(over.devices.len(), 1);
        assert!(over.devices[0].kind.is_empty());
    }

    #[test]
    fn a_whole_disk_says_so_and_a_container_does_not_cross() {
        let mut disk = volume("sdb", "");
        disk.holds.partition = false;
        disk.holds.partition_table = true;
        // AN OPEN CONTAINER — something lives inside it right now — and the
        // system's own: neither is a target.
        let mut open = volume("sdc1", "crypto_LUKS");
        open.holds.encrypted = true;
        open.cleartext = Some("/block/dm-4".to_owned());
        let mut root = volume("sdd1", "crypto_LUKS");
        root.holds.encrypted = true;
        root.system = true;

        let rack = ronoctua_disks::Rack {
            volumes: vec![disk, open, root],
            drives: Vec::new(),
        };
        let over = rack_of(&rack, 1000, &ronoctua_i18n::Texts::from_env());
        assert_eq!(over.devices.len(), 1, "LUKS should not cross over");
        assert!(over.devices[0].whole);
    }

    #[test]
    fn a_locked_stick_crosses_to_be_formatted_again() {
        // LOCKED, NOBODY'S AND NOTHING INSIDE IT ALIVE: an encrypted stick
        // somebody wants back as a plain one is the ordinary case, and its
        // one partition is the target.
        let mut locked = volume("sde1", "crypto_LUKS");
        locked.holds.encrypted = true;
        locked.holds.filesystem = false;
        let rack = ronoctua_disks::Rack {
            volumes: vec![locked],
            drives: Vec::new(),
        };
        let over = rack_of(&rack, 1000, &ronoctua_i18n::Texts::from_env());
        assert_eq!(
            over.devices.len(),
            1,
            "a locked stick cannot be formatted again"
        );
        assert_eq!(over.devices[0].kind, "crypto_LUKS");
    }

    #[test]
    fn the_root_crosses_and_arrives_carrying_its_own_refusal() {
        // SHOWN, NOT OMITTED — the decision this screen was designed around.
        // Somebody looking for their disk has to find it and then read why it
        // is not on offer.
        let mut root = volume("dm-0", "btrfs");
        root.mounted_at = vec!["/".to_owned(), "/home".to_owned()];
        let rack = ronoctua_disks::Rack {
            volumes: vec![root],
            drives: Vec::new(),
        };
        let over = rack_of(&rack, 1000, &ronoctua_i18n::Texts::from_env());
        assert_eq!(over.devices.len(), 1);
        assert!(over.devices[0].mounted);
        // AND THE SENTENCE NAMES THE MOUNT POINT. Without it the buffer draws
        // an assertion nobody can check; with it, a fact they already know.
        assert!(
            over.devices[0].needed.contains('/'),
            "the refusal did not name where: {:?}",
            over.devices[0].needed
        );
    }

    #[test]
    fn somebody_elses_loop_does_not_cross() {
        let mut theirs = volume("loop7", "squashfs");
        theirs.holds.loop_back = true;
        theirs.set_up_by = Some(0);
        let rack = ronoctua_disks::Rack {
            volumes: vec![theirs],
            drives: Vec::new(),
        };
        let over = rack_of(&rack, 1000, &ronoctua_i18n::Texts::from_env());
        assert!(over.devices.is_empty());
    }
}
