//! The kernel's side of a monitor — `/sys/class/drm`.
//!
//! niri names an output by its connector (`eDP-1`, `DP-3`). The kernel names
//! the same thing `cardN-eDP-1`, and inside that directory it puts the two
//! things a compositor cannot answer:
//!
//! - the **backlight** of the panel this connector drives, as a subdirectory
//!   that is also an entry of `/sys/class/backlight`;
//! - the **I2C adapter** that reaches the monitor's own controls, as
//!   `ddc/i2c-dev/i2c-N`.
//!
//! That is why nothing here probes: asking every bus on the machine which
//! monitor is on it takes seconds and answers wrong on a laptop with a
//! docking station. The connector already knows.
//!
//! AND THE BUS NUMBERS ARE IN NO ORDER WORTH GUESSING, which is the point:
//! the panel's bus is not the lowest, the connectors are not numbered in the
//! order they are listed, and a machine with two GPUs starts at `card2`. The
//! connector's own directory says which one is its, and that is the only
//! thing here that ever asks.
//!
//! NO REAL NUMBERS ARE WRITTEN DOWN, and that is not squeamishness: a card
//! index, a backlight device name and two bus numbers together name the
//! hardware they were read off, and this repository is public (invariants 2
//! and 13). The argument above needs none of them to be true.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What the kernel offers for one connector.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Wiring {
    /// The backlight's directory, when this connector drives a panel that has
    /// one. It is the only way to change the brightness of a laptop screen.
    pub backlight: Option<PathBuf>,
    /// The I2C device node that reaches the monitor's own controls. It is how
    /// an EXTERNAL monitor's brightness is changed, over DDC/CI.
    pub bus: Option<PathBuf>,
    /// Whether the connector is built into the machine.
    pub internal: bool,
}

/// Everything the kernel knows, by the name niri uses.
#[must_use]
pub fn read(places: &ronoctua_memory::Places) -> BTreeMap<String, Wiring> {
    read_in(&places.sysfs, Path::new("/dev"))
}

/// The same, under roots a test can build.
///
/// `dev` is separate from `sys` because an I2C adapter is announced in `/sys`
/// and opened in `/dev`, and a test cannot make a device node.
#[must_use]
pub fn read_in(sys: &Path, dev: &Path) -> BTreeMap<String, Wiring> {
    let drm = sys.join("class/drm");
    let Ok(entries) = std::fs::read_dir(&drm) else {
        // NOT AN ERROR. A machine with no DRM at all is a machine running
        // under Xvfb, and the screens still come from the compositor.
        tracing::debug!(at = %drm.display(), "no DRM connectors to read");
        return BTreeMap::new();
    };

    let mut out = BTreeMap::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(connector) = connector_of(name) else {
            continue;
        };
        let dir = drm.join(name);
        out.insert(
            connector.to_owned(),
            Wiring {
                backlight: backlight_under(&dir, sys),
                bus: bus_under(&dir, dev),
                internal: is_internal(connector),
            },
        );
    }

    lend_the_lone_backlight(sys, &mut out);
    // WHAT WAS FOUND, SAID OUT LOUD. Every claim a machine without a panel and
    // without a bus can make about this feature was an ABSENCE — no brightness
    // sentence in a journal — and an absence is satisfied by a build where the
    // whole path was deleted. This is the positive half: the connectors the
    // kernel offers, and for each of them whether there is a light to move and
    // a bus to speak DDC/CI over. `tests/display-vm.nix` reads it.
    for (at, wiring) in &out {
        tracing::debug!(
            at,
            light = wiring.backlight.is_some(),
            bus = wiring.bus.is_some(),
            internal = wiring.internal,
            "what is wired to this connector"
        );
    }
    out
}

/// `card1-eDP-1` is niri's `eDP-1`.
///
/// It strips `card` and the digits that follow it, and then one hyphen. The
/// connector's own name may contain hyphens (`DP-1`), so this cannot be a
/// split on the last one, and the card's number is not a fixed width — a
/// machine with a second GPU has `card2-`.
fn connector_of(entry: &str) -> Option<&str> {
    let rest = entry.strip_prefix("card")?;
    let digits = rest.find(|c: char| !c.is_ascii_digit())?;
    if digits == 0 {
        return None;
    }
    let name = rest[digits..].strip_prefix('-')?;
    (!name.is_empty()).then_some(name)
}

/// Whether the connector is built into the machine.
///
/// It decides which screen a lone backlight belongs to, and it is what a
/// screen says about itself. It does NOT decide whether DDC/CI is tried:
/// what decides that is whether a backlight was found, because a built-in
/// panel that has one never needs to be asked — and one that has none is
/// worth asking anyway. A built-in panel CAN have a `ddc` directory — the
/// machine this was written on has one — so gating on "internal" would have
/// been gating on the wrong fact.
#[must_use]
pub fn is_internal(connector: &str) -> bool {
    let kind = connector
        .split_once('-')
        .map_or(connector, |(k, _)| k)
        .to_ascii_lowercase();
    matches!(kind.as_str(), "edp" | "lvds" | "dsi")
}

/// The backlight this connector owns, if it announces one.
fn backlight_under(dir: &Path, sys: &Path) -> Option<PathBuf> {
    let lights = sys.join("class/backlight");
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let name = e.file_name();
        let candidate = lights.join(&name);
        // BOTH SIDES, because a connector's directory holds many
        // subdirectories and only one of them is also a backlight.
        candidate.is_dir().then_some(candidate)
    })
}

/// The I2C node behind `ddc/i2c-dev/i2c-N`.
fn bus_under(dir: &Path, dev: &Path) -> Option<PathBuf> {
    let holder = dir.join("ddc/i2c-dev");
    let entry = std::fs::read_dir(holder).ok()?.flatten().next()?;
    let node = dev.join(entry.file_name());
    // The adapter exists in `/sys` whether or not `i2c-dev` was loaded; the
    // NODE is what a read needs, and only the module creates it.
    node.exists().then_some(node)
}

/// Gives an unclaimed backlight to the one internal panel.
///
/// A kernel that registers the backlight under the connector is the modern
/// case and needs none of this. The older one — and the ACPI backlight of
/// several laptops — leaves `acpi_video0` in `/sys/class/backlight` belonging
/// to nothing. Handing it to the internal panel is right whenever there is
/// exactly ONE of each; with two of either it would be a guess, and a guess
/// here dims the wrong screen.
fn lend_the_lone_backlight(sys: &Path, out: &mut BTreeMap<String, Wiring>) {
    if out.values().any(|w| w.backlight.is_some()) {
        return;
    }
    let mut internal = out.iter_mut().filter(|(name, _)| is_internal(name));
    let Some((_, panel)) = internal.next() else {
        return;
    };
    if internal.next().is_some() {
        return;
    }
    let lights = sys.join("class/backlight");
    let Ok(found) = std::fs::read_dir(&lights) else {
        return;
    };
    let mut found = found.flatten().map(|e| lights.join(e.file_name()));
    let Some(only) = found.next() else { return };
    if found.next().is_some() {
        tracing::debug!("more than one unclaimed backlight; none of them is chosen");
        return;
    }
    panel.backlight = Some(only);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a `/sys` and a `/dev` in a directory of the test's own.
    struct Fake {
        root: PathBuf,
    }

    impl Fake {
        fn fresh(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("ronoctua-connector-{name}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("sys/class/drm")).unwrap();
            std::fs::create_dir_all(root.join("sys/class/backlight")).unwrap();
            std::fs::create_dir_all(root.join("dev")).unwrap();
            Self { root }
        }

        fn sys(&self) -> PathBuf {
            self.root.join("sys")
        }

        fn dev(&self) -> PathBuf {
            self.root.join("dev")
        }

        fn connector(&self, entry: &str) -> PathBuf {
            let dir = self.root.join("sys/class/drm").join(entry);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        fn backlight(&self, entry: &str, under: Option<&str>) {
            std::fs::create_dir_all(self.root.join("sys/class/backlight").join(entry)).unwrap();
            if let Some(c) = under {
                std::fs::create_dir_all(self.connector(c).join(entry)).unwrap();
            }
        }

        fn bus(&self, entry: &str, node: &str) {
            std::fs::create_dir_all(self.connector(entry).join("ddc/i2c-dev").join(node)).unwrap();
            std::fs::write(self.root.join("dev").join(node), b"").unwrap();
        }

        fn read(&self) -> BTreeMap<String, Wiring> {
            read_in(&self.sys(), &self.dev())
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn the_card_prefix_comes_off_and_the_hyphens_of_the_name_stay() {
        // The connector's own name has a hyphen in it, so this cannot be a
        // split on the last one — and the card's number is not one digit on a
        // machine with two GPUs.
        assert_eq!(connector_of("card1-eDP-1"), Some("eDP-1"));
        assert_eq!(connector_of("card0-DP-12"), Some("DP-12"));
        assert_eq!(connector_of("card12-HDMI-A-1"), Some("HDMI-A-1"));
        assert_eq!(connector_of("card1-Writeback-1"), Some("Writeback-1"));
        // And what is not a connector is not one.
        assert_eq!(connector_of("card1"), None);
        assert_eq!(connector_of("renderD128"), None);
        assert_eq!(connector_of("card-eDP-1"), None);
        assert_eq!(connector_of("version"), None);
        assert_eq!(connector_of("card1-"), None);
    }

    #[test]
    fn a_built_in_panel_is_told_from_a_socket() {
        for internal in ["eDP-1", "LVDS-1", "DSI-1", "eDP-2"] {
            assert!(is_internal(internal), "{internal}");
        }
        for external in ["DP-1", "HDMI-A-1", "DVI-D-1", "Writeback-1"] {
            assert!(!is_internal(external), "{external}");
        }
    }

    #[test]
    fn the_backlight_comes_from_the_connector_that_owns_it() {
        // THE PAIR IS WHAT IDENTIFIES IT: a backlight is BOTH a
        // subdirectory of the connector and an entry of
        // `/sys/class/backlight`, and one of the two alone is not an answer.
        // The names here are invented; what is real is the shape.
        let f = Fake::fresh("owned");
        f.connector("card1-eDP-1");
        f.connector("card1-DP-1");
        f.backlight("amdgpu_bl1", Some("card1-eDP-1"));
        let out = f.read();
        assert_eq!(
            out["eDP-1"].backlight,
            Some(f.sys().join("class/backlight/amdgpu_bl1"))
        );
        assert_eq!(out["DP-1"].backlight, None);
    }

    #[test]
    fn a_subdirectory_that_is_not_a_backlight_is_not_taken_for_one() {
        // A connector's directory holds `power`, `device`, `ddc` and more. If
        // the pairing were dropped, the first of them would be handed back as
        // a backlight and every write would go to a file that is not one.
        let f = Fake::fresh("decoy");
        let dir = f.connector("card1-eDP-1");
        std::fs::create_dir_all(dir.join("power")).unwrap();
        std::fs::create_dir_all(dir.join("device")).unwrap();
        assert_eq!(f.read()["eDP-1"].backlight, None);
    }

    #[test]
    fn a_lone_unclaimed_backlight_goes_to_the_lone_internal_panel() {
        // `acpi_video0` belongs to no connector. One panel and one backlight
        // is not a guess; anything else is.
        let f = Fake::fresh("lone");
        f.connector("card1-eDP-1");
        f.connector("card1-DP-1");
        f.backlight("acpi_video0", None);
        let out = f.read();
        assert_eq!(
            out["eDP-1"].backlight,
            Some(f.sys().join("class/backlight/acpi_video0"))
        );
        assert_eq!(out["DP-1"].backlight, None);
    }

    #[test]
    fn two_panels_or_two_backlights_get_nothing_lent() {
        // Dimming the wrong screen is worse than not offering to dim.
        let two_lights = Fake::fresh("two-lights");
        two_lights.connector("card1-eDP-1");
        two_lights.backlight("acpi_video0", None);
        two_lights.backlight("intel_backlight", None);
        assert_eq!(two_lights.read()["eDP-1"].backlight, None);

        let two_panels = Fake::fresh("two-panels");
        two_panels.connector("card1-eDP-1");
        two_panels.connector("card1-eDP-2");
        two_panels.backlight("acpi_video0", None);
        let out = two_panels.read();
        assert_eq!(out["eDP-1"].backlight, None);
        assert_eq!(out["eDP-2"].backlight, None);
    }

    #[test]
    fn the_bus_is_the_node_and_not_the_adapter() {
        // The adapter is announced in `/sys` whether or not `i2c-dev` was
        // loaded. What a read needs is the NODE, and only the module makes
        // one — so a machine without it has to come back with `None` rather
        // than a path that cannot be opened.
        let f = Fake::fresh("bus");
        f.bus("card1-DP-1", "i2c-4");
        assert_eq!(f.read()["DP-1"].bus, Some(f.dev().join("i2c-4")));

        let no_module = Fake::fresh("no-module");
        std::fs::create_dir_all(no_module.connector("card1-DP-1").join("ddc/i2c-dev/i2c-9"))
            .unwrap();
        assert_eq!(no_module.read()["DP-1"].bus, None);
    }

    #[test]
    fn a_machine_with_no_drm_at_all_is_empty_and_not_an_error() {
        // Which is every VM this repo tests in, and the Xvfb harness too.
        assert!(read_in(Path::new("/nonexistent/sys"), Path::new("/dev")).is_empty());
    }
}
