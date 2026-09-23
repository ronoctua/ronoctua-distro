//! The keyboards' seats and settings, in Independent mode — pure.
//!
//! # A cursor is a seat, and so is a keyboard's
//!
//! A seat is one cursor, one keyboard focus: a keyboard on cursor 2's seat
//! types into the window cursor 2 chose. In Together mode keyd's one keyboard
//! stays on `seat0` and nothing here says a word. In Independent mode every
//! unit's twin (`ronoctua-keys`) is attached to the seat of its cursor, and
//! carries its own layout and repeat.
//!
//! # Which cursor, when nobody said
//!
//! The cursor of a pointing device BUILT INTO THE SAME UNIT — the Bluetooth
//! keyboard and its touchpad share `vendor:product`, which is
//! [`crate::plan`]'s rule for one physical device. Anything else types
//! through the first cursor, which is where every keyboard was before.
//!
//! # The whole xkb block, always
//!
//! A twin's block carries layout, variant AND options, even where it only
//! changed one: the compositor merges `type:keyboard` into a device's block
//! only when the device already exists as the config is read
//! (`store_input_config` in the pinned source), and a twin made after it would
//! come up with none of the machine's settings.

use ronoctua_compositor::{Device, KeyboardSetting};

use crate::plan::Plan;
use crate::prefs::FIRST;

/// The machine's own keyboard, which a unit without a layout of its own
/// keeps — `RONOCTUA_XKB_*`, set by the footer's unit from
/// `config.ronoctua.keyboard`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    /// The xkb layout.
    pub layout: String,
    /// The xkb variant.
    pub variant: String,
    /// The xkb options.
    pub options: String,
}

impl Machine {
    /// From the environment the footer's unit sets.
    #[must_use]
    pub fn from_env() -> Self {
        let var = |n: &str| std::env::var(n).unwrap_or_default();
        Self {
            layout: var("RONOCTUA_XKB_LAYOUT"),
            variant: var("RONOCTUA_XKB_VARIANT"),
            options: var("RONOCTUA_XKB_OPTIONS"),
        }
    }
}

/// One twin's place and settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardPlan {
    /// The unit's identity.
    pub identity: String,
    /// What the kernel calls the physical unit.
    pub name: String,
    /// The twin's compositor identifier.
    pub twin: String,
    /// The cursor it types through.
    pub cursor: u16,
    /// Its settings, whole.
    pub settings: Vec<KeyboardSetting>,
}

/// The cursor a unit types through.
#[must_use]
pub fn cursor_of(
    keyboards: &ronoctua_keyboards::Preferences,
    unit: &ronoctua_keyboards::Unit,
    pointers: &Plan,
) -> u16 {
    if let Some(n) = keyboards.units.get(&unit.identity()).and_then(|u| u.cursor) {
        return n;
    }
    paired(
        unit,
        pointers.devices.iter().map(|d| (d.id.as_str(), d.cursor)),
    )
    .unwrap_or(FIRST)
}

/// The cursor of a pointing device built into the same unit, among
/// `(compositor identifier, cursor)` pairs — the plan's, or the ones
/// `pointers.json` remembers.
pub fn paired<'a>(
    unit: &ronoctua_keyboards::Unit,
    mut pointers: impl Iterator<Item = (&'a str, u16)>,
) -> Option<u16> {
    let prefix = decimal_model(&unit.id)?;
    pointers
        .find(|(id, _)| id.starts_with(&prefix))
        .map(|(_, c)| c)
}

/// `vendor:product` in hex (the kernel's, keyd's) as the compositor's
/// identifier spells it: decimal, with the colon after.
fn decimal_model(model: &str) -> Option<String> {
    let (v, p) = model.split_once(':')?;
    let v = u32::from_str_radix(v, 16).ok()?;
    let p = u32::from_str_radix(p, 16).ok()?;
    Some(format!("{v}:{p}:"))
}

/// Every twin the compositor lists, placed — empty in Together mode.
#[must_use]
pub fn plan(
    keyboards: &ronoctua_keyboards::Preferences,
    units: &[ronoctua_keyboards::Unit],
    devices: &[Device],
    pointers: &Plan,
    machine: &Machine,
) -> Vec<KeyboardPlan> {
    if keyboards.mode != ronoctua_keyboards::Mode::Independent {
        return Vec::new();
    }
    let mut seen = std::collections::BTreeSet::new();
    devices
        .iter()
        // ONE PLAN PER TWIN, however many times the compositor lists it.
        .filter(|d| seen.insert(d.id.clone()))
        .filter_map(|d| {
            let identity = ronoctua_keys::wire::unit_of_twin(&d.id)?;
            let unit = units.iter().find(|u| u.identity() == identity);
            let own = keyboards.units.get(identity);
            let cursor = unit.map_or_else(
                || own.and_then(|u| u.cursor).unwrap_or(FIRST),
                |u| cursor_of(keyboards, u, pointers),
            );
            let text = |mine: Option<&String>, machine: &str| {
                mine.filter(|s| !s.is_empty())
                    .cloned()
                    .unwrap_or_else(|| machine.to_owned())
            };
            let layout_own = own
                .and_then(|u| u.layout.as_ref())
                .filter(|s| !s.is_empty());
            let mut settings = vec![
                KeyboardSetting::Layout(text(layout_own, &machine.layout)),
                // A LAYOUT OF ITS OWN TAKES ITS OWN VARIANT, or none: the
                // machine's variant belongs to the machine's layout.
                KeyboardSetting::Variant(if layout_own.is_some() {
                    own.and_then(|u| u.variant.clone()).unwrap_or_default()
                } else {
                    text(own.and_then(|u| u.variant.as_ref()), &machine.variant)
                }),
                KeyboardSetting::Options(machine.options.clone()),
            ];
            if let Some(r) = own.and_then(|u| u.repeat_rate).filter(|r| *r > 0) {
                settings.push(KeyboardSetting::RepeatRate(r));
            }
            if let Some(d) = own.and_then(|u| u.repeat_delay).filter(|d| *d > 0) {
                settings.push(KeyboardSetting::RepeatDelay(d));
            }
            Some(KeyboardPlan {
                identity: identity.to_owned(),
                name: unit.map_or_else(
                    || own.map(|u| u.name.clone()).unwrap_or_default(),
                    |u| u.name.clone(),
                ),
                twin: d.id.clone(),
                cursor,
                settings,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_compositor::{DeviceKind, PointerSettings};

    fn device(id: &str, kind: DeviceKind) -> Device {
        Device {
            id: id.to_owned(),
            name: id.to_owned(),
            kind,
            settings: PointerSettings::default(),
        }
    }

    fn unit(id: &str, phys: &str) -> ronoctua_keyboards::Unit {
        ronoctua_keyboards::Unit {
            id: id.to_owned(),
            name: "Bluetooth Keyboard".to_owned(),
            built_in: false,
            phys: phys.to_owned(),
            uniq: String::new(),
            event: "event4".to_owned(),
        }
    }

    fn independent() -> ronoctua_keyboards::Preferences {
        ronoctua_keyboards::Preferences {
            mode: ronoctua_keyboards::Mode::Independent,
            ..Default::default()
        }
    }

    #[test]
    fn together_mode_places_no_keyboard() {
        let u = unit("3554:f605", "bt");
        let devices = vec![device(
            &ronoctua_keys::wire::twin_identifier(&u.identity()),
            DeviceKind::Keyboard,
        )];
        let p = plan(
            &ronoctua_keyboards::Preferences::default(),
            &[u],
            &devices,
            &Plan::default(),
            &Machine::default(),
        );
        assert!(p.is_empty());
    }

    #[test]
    fn a_keyboard_goes_where_its_own_touchpad_is() {
        let u = unit("3554:f605", "bt");
        // 0x3554 = 13652, 0xf605 = 62981: the same unit's touchpad.
        let pad = device("13652:62981:Bluetooth_Keyboard_Mouse", DeviceKind::Touchpad);
        let twin = device(
            &ronoctua_keys::wire::twin_identifier(&u.identity()),
            DeviceKind::Keyboard,
        );
        let prefs = crate::prefs::Preferences {
            detect: true,
            ..Default::default()
        };
        let other = device("1133:50504:Logitech_Mouse", DeviceKind::Mouse);
        let pointers = crate::plan::plan(&[other, pad.clone()], &prefs);
        let p = plan(
            &independent(),
            &[u],
            &[pad, twin],
            &pointers,
            &Machine {
                layout: "gb".into(),
                ..Default::default()
            },
        );
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].cursor, 2, "the touchpad's cursor, not the first");
        assert_eq!(p[0].settings[0], KeyboardSetting::Layout("gb".into()));
    }

    #[test]
    fn a_layout_of_its_own_brings_its_own_variant() {
        let u = unit("046d:c31c", "usb-1");
        let mut k = independent();
        k.units.insert(
            u.identity(),
            ronoctua_keyboards::UnitPrefs {
                layout: Some("fr".into()),
                cursor: Some(3),
                repeat_rate: Some(40),
                ..Default::default()
            },
        );
        let twin = device(
            &ronoctua_keys::wire::twin_identifier(&u.identity()),
            DeviceKind::Keyboard,
        );
        let p = plan(
            &k,
            &[u],
            &[twin],
            &Plan::default(),
            &Machine {
                layout: "gb".into(),
                variant: "extd".into(),
                options: "compose:caps".into(),
            },
        );
        assert_eq!(p[0].cursor, 3);
        assert_eq!(
            p[0].settings,
            vec![
                KeyboardSetting::Layout("fr".into()),
                KeyboardSetting::Variant(String::new()),
                KeyboardSetting::Options("compose:caps".into()),
                KeyboardSetting::RepeatRate(40),
            ]
        );
    }
}
