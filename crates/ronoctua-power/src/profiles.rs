//! power-profiles-daemon's bus shape — names only.
//!
//! The daemon's properties arrive through one `GetAll` (`upower::get_all`) and
//! `ActiveProfile` is written through one `Set` (`upower::set_one`): there is
//! no `#[zbus::proxy]` here because there is nothing to call. What this file
//! holds is where the daemon lives and what its rows are called.
//!
//! # Two names, one daemon
//!
//! The daemon has owned `org.freedesktop.UPower.PowerProfiles` since 0.20 and
//! still answers to the older `net.hadess.PowerProfiles`; nixpkgs ships 0.30.
//! Only the current name is used — a second name would be a second thing to
//! keep in step, for a daemon that is not going back.

/// The service, on the system bus.
pub const SERVICE: &str = "org.freedesktop.UPower.PowerProfiles";

/// Its one object — under UPower's path namespace, which is what lets one
/// signal subscription cover the batteries and the profile at once.
pub const PATH: &str = "/org/freedesktop/UPower/PowerProfiles";

/// The interface.
pub const INTERFACE: &str = "org.freedesktop.UPower.PowerProfiles";

/// The property that is written.
pub const ACTIVE: &str = "ActiveProfile";

/// The switch that lets the daemon lower the profile on its own when the
/// battery runs low (0.23 and later).
pub const BATTERY_AWARE: &str = "BatteryAware";
