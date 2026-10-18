//! THE NOTIFICATIONS' TAGS the shell raises and recognises — each the SAME
//! string as its entry in `assets/notifications.nix`.
//!
//! # Why a module of its own
//!
//! They were the footer's, beside the code that raised them; but what
//! raises a Bluetooth device's arrival, a battery's alarm or a program's
//! install is the view that hears it, and what recognises them is the
//! footer's events. A tag both sides compare is a word both sides reach.

/// The tag that makes the catalogue recognise the out-of-memory notification.
///
/// It is a string that has to be the SAME here and in
/// `assets/notifications.nix`, and there is no compiler between them — so it
/// lives in one place on this side and the `notification-tags-are-known` check
/// compares the two.
pub const MEMORY_KILLED: &str = "memory-killed";

/// The pairing question's tag, for the times the view cannot be opened to
/// ask it — the file manager holding the footer. The two actions are the
/// catalogue's, and they run the same two commands `s b y`/`s b n` do.
pub const BLUETOOTH_PAIRING: &str = "bluetooth-pairing";

/// A device came back. Brief and quick: it confirms something that was asked
/// for, and the mark on the modeline says it too.
pub const BLUETOOTH_CONNECTED: &str = "bluetooth-connected";

/// A device DROPPED, which is the one worth a sentence.
///
/// The owner's keyboard is Bluetooth: when it goes, the machine stops
/// answering keys, and what says so on the modeline is a glyph that has just
/// DISAPPEARED. Reading an absence is not something a person can do.
pub const BLUETOOTH_DISCONNECTED: &str = "bluetooth-disconnected";

/// A device's own battery crossed the number the modeline turns red at.
///
/// The machine's battery had a second voice (`BATTERY_LOW`) and the devices
/// had none: a mark that changes colour is not something one notices while
/// typing.
pub const BLUETOOTH_BATTERY_LOW: &str = "bluetooth-battery-low";

/// The battery's three alarms — the same strings `ronoctua_power::Alarm::id`
/// answers, written here so the tag check can read them off this file, and
/// tied to the crate's by a test so the two cannot drift.
pub const BATTERY_LOW: &str = "battery-low";

/// See [`BATTERY_LOW`].
pub const BATTERY_CRITICAL: &str = "battery-critical";

/// See [`BATTERY_LOW`].
pub const BATTERY_CHARGED: &str = "battery-charged";

/// The capture's tag, and the whole recognition of it.
///
/// The picture used to be taken by the compositor, which sent a notification of
/// its own with an English summary; the entry in `assets/notifications.nix`
/// matched the pair (app, summary) and a check read the sender's source so that
/// pair could not move in silence. Now `grim` takes the picture and the footer
/// raises the notification, so the entry matches a word WE write — which is why
/// the check is gone and nothing replaced it.
pub const SCREEN_CAPTURED: &str = "screenshot";

/// The software centre's notifications — one tag each, raised when a job of
/// the queue's software engine ends (`software::ended_of`), and when a trial
/// closes (`software_feed::Running`).
pub const SOFTWARE_INSTALLED: &str = "software-installed";

/// See [`SOFTWARE_INSTALLED`].
pub const SOFTWARE_WEB_CREATED: &str = "software-web-created";

/// See [`SOFTWARE_INSTALLED`].
pub const SOFTWARE_REMOVED: &str = "software-removed";

/// See [`SOFTWARE_INSTALLED`].
pub const SOFTWARE_UPDATED: &str = "software-updated";

/// See [`SOFTWARE_INSTALLED`].
pub const SOFTWARE_FAILED: &str = "software-failed";

/// See [`SOFTWARE_INSTALLED`].
pub const SOFTWARE_TRIAL_ENDED: &str = "software-trial-ended";

/// See [`SOFTWARE_INSTALLED`].
pub const SOFTWARE_ANDROID_READY: &str = "software-android-ready";
