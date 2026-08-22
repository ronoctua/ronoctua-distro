//! udisks2's D-Bus proxies — the only part of this crate that knows D-Bus
//! exists.
//!
//! # Why the boundary is here
//!
//! The same split as `ronoctua_network::nm` and `ronoctua_notify::dbus`: this
//! file translates the bus and the rest of the crate does not import it. What
//! that buys is that the rules — what counts as a volume, which rows are quiet,
//! how a filesystem finds the disk it sits on — are tested against nine rows
//! copied off a real machine, with no bus, no device and no root.
//!
//! # Blocking, on a thread
//!
//! zbus has both APIs. The blocking one, for the reason the workspace's
//! `Cargo.toml` already gives: the shell is a `calloop` loop with one thread per
//! event source, and an async executor inside it would be a second concurrency
//! model buying nothing.
//!
//! # What is deliberately NOT proxied
//!
//! `Format`, `Check`, `Repair`, `Resize`, `SetLabel` and `TakeOwnership`.
//!
//! Two reasons, and the second is the binding one. A sidebar is for reaching a
//! disk, not for administering one. And `nix/modules/disks.nix` builds udisks
//! with `xfsprogs` removed — 212 MiB of closure, almost all of it `xfs_scrub`'s
//! Python — which is free precisely because MOUNTING is a kernel operation and
//! none of those verbs exists here. `disks-tools-are-pinned` compares the two
//! sides, so the day one of them is added the check names it.
//!
//! # `Serial` and `Id` are not read
//!
//! A drive's serial number is on the bus and this repository is public
//! (invariant 13). The object path already embeds it, which is why a path is a
//! KEY here and never something drawn or written down.

// `#[zbus::proxy]` GENERATES a struct per trait, and the workspace's
// `missing_docs` flags them. There is nowhere to write their documentation: the
// macro creates them after expansion. What can be documented — the trait and
// each method — is. The `allow` stays on the MODULE, not on the crate, so the
// rest goes on requiring docs.
#![allow(missing_docs)]

use std::collections::HashMap;

use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

/// The service, on the **system** bus.
///
/// System and not session: the disks belong to the machine, not to the person
/// logged in. Asking on the session bus answers "name has no owner", which is
/// the same error as an absent service and leads to the wrong diagnosis — the
/// trap `ronoctua_network::nm` already records.
pub const SERVICE: &str = "org.freedesktop.UDisks2";

/// Where the object manager lives.
pub const ROOT: &str = "/org/freedesktop/UDisks2";

/// The interface names, as `GetManagedObjects` keys them.
///
/// Their PRESENCE is a fact in its own right, and half the filter is built on
/// that: a block with no `Filesystem` has nothing to mount and nowhere to go,
/// and one with a `PartitionTable` is a drive's heading rather than a place.
pub const BLOCK: &str = "org.freedesktop.UDisks2.Block";
pub const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
pub const PARTITION_TABLE: &str = "org.freedesktop.UDisks2.PartitionTable";
pub const PARTITION: &str = "org.freedesktop.UDisks2.Partition";
pub const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
pub const SWAPSPACE: &str = "org.freedesktop.UDisks2.Swapspace";
pub const LOOP: &str = "org.freedesktop.UDisks2.Loop";
pub const DRIVE: &str = "org.freedesktop.UDisks2.Drive";

/// What udisks answers for "no object": a path of one slash.
///
/// It is NOT the empty string, and reading it as one is how `Block.Drive`
/// quietly becomes `Some("/")` and a volume ends up filed under a disk that
/// does not exist.
pub const NOTHING: &str = "/";

/// One object's interfaces, each with its properties.
///
/// A name and not the type written out, because the type written out is what
/// `GetManagedObjects` returns nested three deep and clippy is right that
/// nobody can read it there.
pub type Faces = HashMap<String, HashMap<String, OwnedValue>>;

/// Everything udisks knows, keyed by object path.
pub type Managed = HashMap<OwnedObjectPath, Faces>;

/// Every object and every interface, in one round trip.
///
/// One call and not a property get per object: on an ordinary machine this
/// answers nineteen objects with everything on them, and nineteen `GetAll`s
/// would be nineteen round trips for the same bytes.
#[zbus::proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "org.freedesktop.UDisks2",
    default_path = "/org/freedesktop/UDisks2"
)]
pub trait Objects {
    /// Everything udisks knows, keyed by object path then by interface.
    fn get_managed_objects(&self) -> zbus::Result<Managed>;
}

/// A filesystem that can be mounted.
///
/// The interface exists on a block that HAS one, mounted or not — which is the
/// whole reason a pendrive nobody has mounted yet can be a row.
#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Filesystem",
    default_service = "org.freedesktop.UDisks2"
)]
pub trait Filesystem {
    /// Mounts it and answers where.
    ///
    /// No `fstype` and no `options` are passed: letting udisks choose from its
    /// own `mount_options.conf` is most of the reason to go through udisks at
    /// all. What IS passed is `auth.no_user_interaction` — see [`no_asking`].
    fn mount(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<String>;

    /// Unmounts it.
    ///
    /// `force` is never passed. Forcing a stick out from under a write in
    /// progress is data loss behind a keystroke, and the honest answer to a
    /// busy device is to say which one is busy.
    fn unmount(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

/// A block device, which every row here is one of.
///
/// Read through `GetManagedObjects` for the listing; this proxy exists for the
/// two things that are CALLS and not properties.
#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Block",
    default_service = "org.freedesktop.UDisks2"
)]
pub trait Block {
    /// Writes a filesystem, or a partition table, over whatever is there.
    ///
    /// `kind` is a filesystem name (`vfat`, `ext4`) or a table name (`dos`,
    /// `gpt`) — udisks takes both through the same door.
    ///
    /// # `tear-down` is deliberately never passed
    ///
    /// It would unmount the device first, which sounds like a convenience, and
    /// it ALSO removes the device's `fstab` entries. Formatting a stick must
    /// not edit the machine's boot configuration as a side effect. Whatever is
    /// mounted is unmounted by name, before this is called, or udisks refuses
    /// and the refusal is the honest answer.
    fn format(&self, kind: &str, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;

    /// How big it is, asked directly.
    ///
    /// The listing already carries this, but the caller that needs it here is
    /// choosing a partition table, and reading it a second time costs one round
    /// trip and removes a parameter that could arrive stale.
    #[zbus(property)]
    fn size(&self) -> zbus::Result<u64>;
}

/// A disk that has a partition table on it.
#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.PartitionTable",
    default_service = "org.freedesktop.UDisks2"
)]
pub trait PartitionTable {
    /// Lays down one partition and formats it, in a single call.
    ///
    /// `offset` and `size` are both 0, which is udisks' way of saying "the
    /// first aligned place, and all of it". Two separate calls would leave a
    /// window where the partition exists with no filesystem on it, and a
    /// listing refreshed inside that window shows a row nobody can open.
    #[allow(clippy::too_many_arguments)]
    fn create_partition_and_format(
        &self,
        offset: u64,
        size: u64,
        kind: &str,
        name: &str,
        options: HashMap<&str, Value<'_>>,
        format_kind: &str,
        format_options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

/// The physical thing a volume sits on.
#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Drive",
    default_service = "org.freedesktop.UDisks2"
)]
pub trait Drive {
    /// Spits the medium out — an optical tray, a card reader.
    fn eject(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;

    /// Cuts the power, which is what makes a stick safe to pull.
    fn power_off(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

/// The options every call carries, and the reason they are not optional.
///
/// **There is no polkit authentication agent in this session.** The only one in
/// the distro is `pkttyagent`, on the rescue console. So an action that resolves
/// to `auth_admin_keep` — mounting a partition udisks marks `HintSystem` — has
/// nobody to ask, and without this flag polkit WAITS for an agent that is never
/// going to register: the row is pressed and nothing happens, for ever.
///
/// With it, the refusal comes back at once and it has a NAME
/// (`org.freedesktop.PolicyKit1.Error.NotAuthorized`), which is what turns a
/// hang into a sentence on the message line.
#[must_use]
pub fn no_asking() -> HashMap<&'static str, Value<'static>> {
    HashMap::from([("auth.no_user_interaction", Value::Bool(true))])
}

/// A `Vec<Vec<u8>>` of NUL-terminated byte strings, as udisks writes paths.
///
/// It is how `Device`, `Symlinks` and `MountPoints` all arrive: `ay` and not
/// `s`, because a mount point is whatever bytes the kernel holds and need not
/// be UTF-8. The trailing NUL is udisks' own and is not part of the name —
/// left in, every comparison against a real path fails and nothing says why.
#[must_use]
pub fn paths_of(v: Option<&OwnedValue>) -> Vec<String> {
    let Some(v) = v else { return Vec::new() };
    let Ok(rows) = <Vec<Vec<u8>>>::try_from(v.clone()) else {
        return Vec::new();
    };
    rows.iter().map(|row| path_of_bytes(row)).collect()
}

/// One such byte string.
#[must_use]
pub fn path_of(v: Option<&OwnedValue>) -> String {
    let Some(v) = v else { return String::new() };
    <Vec<u8>>::try_from(v.clone()).map_or_else(|_| String::new(), |b| path_of_bytes(&b))
}

/// The bytes, without udisks' trailing NUL, read lossily.
///
/// Lossily and not fallibly: a stick formatted on another system can carry a
/// name in any encoding, and a row that vanishes because its label is not UTF-8
/// is a disk that cannot be reached and gives no reason.
fn path_of_bytes(row: &[u8]) -> String {
    let end = row.iter().position(|b| *b == 0).unwrap_or(row.len());
    String::from_utf8_lossy(&row[..end]).into_owned()
}

/// A `s` property, or the empty string.
#[must_use]
pub fn text_of(v: Option<&OwnedValue>) -> String {
    v.and_then(|v| <String>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// A `b` property, or `false`.
#[must_use]
pub fn flag_of(v: Option<&OwnedValue>) -> bool {
    v.and_then(|v| <bool>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// A `t` property, or zero.
#[must_use]
pub fn number_of(v: Option<&OwnedValue>) -> u64 {
    v.and_then(|v| <u64>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// A `u` property, or `None`.
///
/// `None` and not zero, because zero is a real uid — root's. `Loop.SetupByUID`
/// is the one place the difference decides something.
#[must_use]
pub fn small_number_of(v: Option<&OwnedValue>) -> Option<u32> {
    v.and_then(|v| <u32>::try_from(v.clone()).ok())
}

/// Does `Block.Configuration` carry an `fstab` entry?
///
/// The property is `a(sa{sv})` — an array of `(kind, details)`, where `kind` is
/// `"fstab"` or `"crypttab"`. Only the KIND is read: the details carry the
/// mount point and the options, and nothing here needs them. What is being
/// asked is one thing — did the administrator declare this device mounted at
/// boot — and reading more would be reading a configuration file's contents
/// out of a bus for no reason.
///
/// It costs no extra round trip: the property already arrives in the same
/// `GetManagedObjects` answer as everything else.
#[must_use]
pub fn in_fstab(v: Option<&OwnedValue>) -> bool {
    let Some(v) = v else { return false };
    <Vec<(String, HashMap<String, OwnedValue>)>>::try_from(v.clone())
        .is_ok_and(|rows| rows.iter().any(|(kind, _)| kind == "fstab"))
}

/// An `o` property, with udisks' "nothing" mapped to `None`.
#[must_use]
pub fn object_of(v: Option<&OwnedValue>) -> Option<String> {
    let path = v
        .and_then(|v| <OwnedObjectPath>::try_from(v.clone()).ok())
        .map(|p| p.as_str().to_owned())?;
    if path == NOTHING { None } else { Some(path) }
}
