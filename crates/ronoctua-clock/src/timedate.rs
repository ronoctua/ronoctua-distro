//! `systemd-timedated`'s D-Bus proxy — and nothing else talks to it.
//!
//! # `interactive` is always `false`
//!
//! Every method here takes a flag that tells polkit whether it may ask an
//! agent. There is no agent in this session — `nix/modules/disks.nix` says why
//! at length, and the reason is the owner's rather than the design's: a
//! fingerprint prompt nobody sees becomes a refusal in a report about
//! something nobody watched. With `false` a refusal comes back at once and has
//! a name.
//!
//! What makes the calls go through at all is the narrow polkit rule in
//! `nix/modules/base.nix`, which grants exactly the three actions below to
//! exactly one user. `timedate-polkit-is-narrow` reads that rule and the
//! constants here in both directions.

// THE PROXY MACRO WRITES A STRUCT WE DO NOT NAME, and `missing_docs` asks for
// a doc comment on it. `ronoctua-power`'s three proxies carry the same
// exemption for the same reason.
#![allow(missing_docs)]

/// The service, on the system bus.
pub const SERVICE: &str = "org.freedesktop.timedate1";

/// Its object.
pub const PATH: &str = "/org/freedesktop/timedate1";

/// Its interface, which is also where the properties live.
pub const INTERFACE: &str = "org.freedesktop.timedate1";

/// The polkit actions this crate can reach, and the whole of them.
///
/// # Why they are written down here
///
/// Because the rule that grants them is written in Nix, and a rule and its
/// caller drifting apart is a door that is wider than anybody believes.
/// `timedate-polkit-is-narrow` compares these three against the installed rule
/// in BOTH directions — an action granted and never called is as much a
/// finding as one called and never granted.
///
/// `set-local-rtc` is deliberately NOT here: nothing in this tree asks the
/// machine to keep its hardware clock in local time, and a fourth action would
/// be a door held open for nobody.
pub const ACTIONS: &[&str] = &[
    "org.freedesktop.timedate1.set-ntp",
    "org.freedesktop.timedate1.set-time",
    "org.freedesktop.timedate1.set-timezone",
];

#[zbus::proxy(
    interface = "org.freedesktop.timedate1",
    default_service = "org.freedesktop.timedate1",
    default_path = "/org/freedesktop/timedate1"
)]
pub trait Timedate {
    /// Turns the network time client on or off, which on this distro is
    /// `systemd-timesyncd`. The second flag is `interactive`.
    fn set_ntp(&self, use_ntp: bool, interactive: bool) -> zbus::Result<()>;

    /// Sets the clock. `usec` is MICROSECONDS since the epoch when `relative`
    /// is false, and an offset from now when it is true.
    ///
    /// The daemon REFUSES this while the network time client is on, and that
    /// refusal is the truthful one: something that corrects the clock every
    /// half hour would put it back. The screen says so before the key is
    /// pressed rather than letting the refusal be the explanation.
    fn set_time(&self, usec: i64, relative: bool, interactive: bool) -> zbus::Result<()>;

    /// Sets the machine's zone by its IANA name.
    ///
    /// It is refused when the zone is declared in the flake: NixOS then starts
    /// timedated with `NIXOS_STATIC_TIMEZONE=1` and `/etc/localtime` is the
    /// store's. See `ronoctua.time.zone`, and the row that says so.
    fn set_timezone(&self, zone: &str, interactive: bool) -> zbus::Result<()>;
}
