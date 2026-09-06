//! logind's D-Bus proxy — sleeping, shutting down, and holding the machine
//! awake.
//!
//! # `interactive` is always `false`
//!
//! Every action here takes an `interactive` flag that tells polkit whether it
//! may ask an agent. There is no agent in this session (`nix/modules/disks.nix`
//! says why at length), so `true` would be a call that waits for an answer
//! nobody can give. With `false`, a refusal comes back at once and has a name
//! — the `auth.no_user_interaction` udisks gets, spelled logind's way.
//!
//! # The properties are not proxied
//!
//! `HandleLidSwitch`, `IdleAction`, `LidClosed` and the rest arrive together
//! through one `GetAll` (see `upower::get_all`), which is what `Sleep::from_props`
//! is fed and what the fixture test feeds it.

#![allow(missing_docs)]

/// The service, on the system bus.
pub const SERVICE: &str = "org.freedesktop.login1";

/// The manager's object.
pub const PATH: &str = "/org/freedesktop/login1";

/// The manager's interface.
pub const MANAGER: &str = "org.freedesktop.login1.Manager";

/// The `who` an inhibitor of ours carries, and how `ListInhibitors` is read
/// back to find them again. A NAME and not the pid: the footer restarts, and
/// the inhibitor a dead footer held is gone with its fd anyway.
pub const WHO: &str = "ronoctua-shell";

/// Why we hold the machine awake — a unit-description-class string
/// (invariant 18: one language, and it is English), never shown as UI.
pub const WHY: &str = "kept awake from the power view";

/// One inhibitor, as `ListInhibitors` returns it.
pub type Row = (String, String, String, String, u32, u32);

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait Login {
    /// Takes an inhibitor lock; the fd IS the lock, and closing it releases it.
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;

    /// Every lock anybody holds: `(what, who, why, mode, uid, pid)`.
    fn list_inhibitors(&self) -> zbus::Result<Vec<Row>>;

    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    fn hibernate(&self, interactive: bool) -> zbus::Result<()>;
    fn hybrid_sleep(&self, interactive: bool) -> zbus::Result<()>;
    fn suspend_then_hibernate(&self, interactive: bool) -> zbus::Result<()>;
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;

    /// `yes`, `no`, `na` (the hardware cannot) or `challenge` (polkit would
    /// ask — which here means "no", there being nobody to ask).
    fn can_suspend(&self) -> zbus::Result<String>;
    fn can_hibernate(&self) -> zbus::Result<String>;
    fn can_hybrid_sleep(&self) -> zbus::Result<String>;
    fn can_suspend_then_hibernate(&self) -> zbus::Result<String>;
    fn can_power_off(&self) -> zbus::Result<String>;
    fn can_reboot(&self) -> zbus::Result<String>;
}
