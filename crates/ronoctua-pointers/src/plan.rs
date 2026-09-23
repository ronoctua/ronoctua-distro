//! Which cursor every pointing device drives — pure, and the whole of the
//! logic.
//!
//! # One seat per cursor
//!
//! The compositor draws one cursor per SEAT, and a seat is whatever devices
//! are attached to it. So "this mouse has its own cursor" is "this mouse is
//! attached to a seat nothing else is on". Cursor [`FIRST`] is the
//! compositor's own `seat0`, which keeps the keyboards; every other cursor
//! is a seat of ours, `ronoctua-cursor-N`.
//!
//! # What is kept for a device that is not plugged in
//!
//! Its place. A mouse unplugged and plugged back comes back to the same
//! cursor with the same colour, because [`Preferences::devices`] remembers it
//! and a number held by a remembered device is not handed to a new one.
//!
//! # Two units of the same model are one device here
//!
//! The compositor's identifier is `vendor:product:name`, so two identical
//! mice answer to the same one — and `seat … attach` takes an identifier, so
//! the two cannot be put on different seats by anything this side of the
//! compositor. They are listed once and drive one cursor together. The screen
//! says so rather than offering a choice that could not be honoured.

use std::collections::{BTreeMap, BTreeSet};

use ronoctua_compositor::{Device, DeviceKind, PointerSettings, SendEvents};

use crate::prefs::{CursorPrefs, DevicePrefs, FIRST, Preferences};

/// What every cursor and every present device should be.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    /// Every cursor with a device on it, and the first one always.
    pub cursors: Vec<CursorPlan>,
    /// Every pointing device that is present.
    pub devices: Vec<DevicePlan>,
    /// Devices this plan placed for the first time — what has to be written
    /// back so that the next plan places them the same way.
    pub placed: Vec<(String, u16)>,
}

/// One cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorPlan {
    /// Its number, from [`FIRST`].
    pub number: u16,
    /// The compositor's name for its seat.
    pub seat: String,
    /// Which colour its inside is, from `0`.
    pub colour: u8,
    /// Whether it is on.
    pub enabled: bool,
    /// Whether it is DRAWN: on, and with a present device on it that is on.
    /// A seat shows its cursor for as long as any pointer is attached to it,
    /// switched off or not — so a cursor nobody can move is hidden on
    /// purpose (`fragment::HIDDEN_MS`), or it stands still on the screen.
    pub shown: bool,
    /// The identifiers of the present devices driving it.
    pub devices: Vec<String>,
}

/// One device.
#[derive(Debug, Clone, PartialEq)]
pub struct DevicePlan {
    /// The compositor's identifier.
    pub id: String,
    /// The name it gives itself.
    pub name: String,
    /// What it is.
    pub kind: DeviceKind,
    /// Which cursor it drives.
    pub cursor: u16,
    /// Whether it is switched on by its own choice.
    pub enabled: bool,
    /// Whether it is a virtual machine's own pointer (see [`EMULATED`]).
    pub emulated: bool,
    /// Whether it moves anything: its own switch, its cursor's, and — for an
    /// emulated one — [`Preferences::emulated`].
    pub on: bool,
    /// The settings it should have, only the ones it supports.
    pub settings: PointerSettings,
    /// The settings it has right now.
    pub now: PointerSettings,
}

impl DevicePlan {
    /// The settings that differ from what the device has now — what a live
    /// change has to ask for, so a re-plan asks nothing it does not need.
    #[must_use]
    pub fn to_ask(&self) -> Vec<ronoctua_compositor::Setting> {
        let now = self.now.present();
        self.settings
            .present()
            .into_iter()
            .filter(|s| !now.contains(s))
            .collect()
    }
}

/// The vendor keyd's virtual devices carry (`0x0FAC`), as the compositor
/// spells an identifier: in decimal, before the first colon.
const KEYBOARD_REMAPPER: &str = "4012:";

/// Whether a pointing device is the keyboards' OWN output — keyd's virtual
/// pointer, which moves only when a key is remapped to the mouse.
///
/// It is not a hand on a mouse, so it never takes a cursor: it stays on the
/// keyboards' seat, where the compositor puts every device by default. Taken
/// as "the first pointer there ever was" it held `seat0`'s cursor still in a
/// corner for the whole session, and the mouse that was actually there got
/// cursor three — measured in the VM.
#[must_use]
pub fn follows_the_keyboards(id: &str) -> bool {
    id.starts_with(KEYBOARD_REMAPPER)
}

/// The `vendor:product` of an identifier, when it names a real one — what
/// two interfaces of ONE physical device share.
///
/// A touchpad reaches the compositor as TWO devices: `…_Touchpad`, which
/// moves, and `…_Mouse`, a compatibility interface that sits still. Measured
/// on a laptop's `SNSL0027:00 2C2F:0027`: both are libinput pointers, and one
/// cursor each left a cursor that never moved beside the touchpad's own.
/// Vendor `0` is "unknown", and two unknowns are not one device.
fn one_physical(id: &str) -> Option<&str> {
    let mut at = id.match_indices(':').map(|(i, _)| i);
    at.next()?;
    let end = at.next()?;
    let vp = &id[..end];
    (!vp.starts_with("0:")).then_some(vp)
}

/// Where the names of a virtual machine's own pointers come from, `|`
/// between them: set by the module that knows the machine is one.
pub const EMULATED: &str = "RONOCTUA_POINTERS_EMULATED";

/// The emulated pointers' names, from [`EMULATED`].
#[must_use]
pub fn emulated_names() -> Vec<String> {
    std::env::var(EMULATED)
        .unwrap_or_default()
        .split('|')
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The compositor's name for cursor `n`'s seat.
#[must_use]
pub fn seat_of(n: u16) -> String {
    if n == FIRST {
        "seat0".to_owned()
    } else {
        format!("ronoctua-cursor-{n}")
    }
}

/// The plan for these devices under these choices.
#[must_use]
pub fn plan(devices: &[Device], prefs: &Preferences) -> Plan {
    plan_with(devices, prefs, &emulated_names())
}

/// [`plan`], with the emulated pointers' names given.
///
/// # An emulated pointer is OFF while a real hand is here
///
/// A virtual machine's tablet carries the host's pointer in; once the host's
/// own mice are handed to the guest one by one, it is a cursor that only the
/// pointers the host kept can move. So it is off unless
/// [`Preferences::emulated`] says otherwise — and ON when it is the only
/// pointer there is, because a machine with no pointer at all is not a
/// default anyone chose.
#[must_use]
pub fn plan_with(devices: &[Device], prefs: &Preferences, emulated: &[String]) -> Plan {
    let mut out = Plan::default();
    let is_emulated = |d: &Device| emulated.contains(&d.name);
    let a_real_hand = devices.iter().any(|d| {
        d.kind.points()
            && !is_emulated(d)
            && !follows_the_keyboards(&d.id)
            && ronoctua_keys::wire::unit_of_twin(&d.id).is_none()
    });
    // THE NUMBERS ALREADY TAKEN, by any device ever remembered — present or
    // not — so a number is never handed to a newcomer while its owner is in
    // a drawer.
    let mut taken: BTreeSet<u16> = prefs
        .devices
        .iter()
        .filter(|(id, _)| !follows_the_keyboards(id))
        .filter_map(|(_, d)| d.cursor)
        .collect();
    // THE VIRTUAL MACHINE'S POINTERS ARE ONE HAND: its tablet and its two
    // mice are fed by the same host pointer, and only one of them gets the
    // motion at a time — apart, the others are cursors that stand still.
    let mut emulated_cursor: Option<u16> = devices
        .iter()
        .filter(|d| d.kind.points() && is_emulated(d))
        .filter_map(|d| prefs.devices.get(&d.id).and_then(|p| p.cursor))
        .min();
    let mut seen = BTreeSet::new();
    let mut by_cursor: BTreeMap<u16, Vec<String>> = BTreeMap::new();
    // WHERE EACH PHYSICAL DEVICE ALREADY IS, so its other interfaces join it
    // instead of taking cursors of their own (see `one_physical`).
    let mut physical: BTreeMap<String, u16> = BTreeMap::new();
    for d in devices.iter().filter(|d| d.kind.points()) {
        if let (Some(vp), Some(n)) = (
            one_physical(&d.id),
            prefs.devices.get(&d.id).and_then(|p| p.cursor),
        ) {
            physical.entry(vp.to_owned()).or_insert(n);
        }
    }

    for d in devices.iter().filter(|d| {
        d.kind.points()
                && !follows_the_keyboards(&d.id)
                // A KEYBOARD'S TWIN IS NEVER A HAND, whatever libinput
                // decided it is: its seat is its keyboard's (`keyboards`).
                && ronoctua_keys::wire::unit_of_twin(&d.id).is_none()
    }) {
        if !seen.insert(d.id.clone()) {
            continue;
        }
        let own = prefs.devices.get(&d.id).cloned().unwrap_or_default();
        let emulated = is_emulated(d);
        let cursor = match emulated_cursor.filter(|_| emulated).or(own.cursor) {
            Some(n) => n,
            None => {
                let sibling = one_physical(&d.id).and_then(|vp| physical.get(vp).copied());
                let n = if let Some(n) = sibling {
                    n
                } else if prefs.detect {
                    // THE FIRST POINTER THERE EVER WAS takes the first seat,
                    // which is the one the keyboards are on; every later one
                    // gets a cursor of its own.
                    if taken.contains(&FIRST) {
                        (FIRST + 1..).find(|n| !taken.contains(n)).unwrap_or(FIRST)
                    } else {
                        FIRST
                    }
                } else {
                    FIRST
                };
                taken.insert(n);
                out.placed.push((d.id.clone(), n));
                n
            }
        };
        if let Some(vp) = one_physical(&d.id) {
            physical.entry(vp.to_owned()).or_insert(cursor);
        }
        if emulated {
            emulated_cursor.get_or_insert(cursor);
            if own.cursor.is_some_and(|n| n != cursor) {
                out.placed.push((d.id.clone(), cursor));
            }
        }
        // ONE CURSOR FOR EVERY HAND: the place just found is REMEMBERED (it is
        // in `placed` already) and not used, so turning `together` off gives
        // every device back the cursor it had.
        let cursor = if prefs.together { FIRST } else { cursor };
        let kind_default = match d.kind {
            DeviceKind::Touchpad => &prefs.touchpads,
            _ => &prefs.mice,
        };
        let mut settings = kind_default
            .only_where(&d.settings)
            .under(&own.settings.only_where(&d.settings));
        let cursor_on = prefs.cursor(cursor).enabled;
        let on = own.enabled && cursor_on && (!emulated || prefs.emulated || !a_real_hand);
        if !on && d.settings.send_events.is_some() {
            settings.send_events = Some(SendEvents::Disabled);
        } else if settings.send_events == Some(SendEvents::Disabled) {
            // A DISABLED DEVICE IS SAID BY `enabled`, never by a leftover
            // `send_events` in its settings: two switches for one thing is a
            // device the screen shows as on and the compositor keeps off.
            settings.send_events = Some(SendEvents::Enabled);
        } else if settings.send_events.is_none() && d.settings.send_events.is_some() {
            settings.send_events = Some(SendEvents::Enabled);
        }
        by_cursor.entry(cursor).or_default().push(d.id.clone());
        out.devices.push(DevicePlan {
            id: d.id.clone(),
            name: d.name.clone(),
            kind: d.kind,
            cursor,
            enabled: own.enabled,
            emulated,
            on,
            settings,
            now: d.settings,
        });
    }

    by_cursor.entry(FIRST).or_default();
    for (number, ids) in by_cursor {
        let c = prefs.cursor(number);
        let shown = c.enabled && out.devices.iter().any(|d| d.cursor == number && d.on);
        out.cursors.push(CursorPlan {
            number,
            seat: seat_of(number),
            colour: c.colour,
            enabled: c.enabled,
            shown,
            devices: ids,
        });
    }
    out
}

/// The numbers the screens DRAW, which are not the numbers kept.
///
/// A cursor's number is memory: a device that goes away keeps its own, so it
/// comes back to the same cursor. Drawn as it is, that leaves holes — cursor 1,
/// then cursor 3 — every time a device is away or two cursors become one. So
/// every screen draws the cursors that exist as 1, 2, 3…, in the order of
/// their numbers, and turns a drawn number back into a kept one. The first
/// cursor is always 1: it is the first seat's, and it is always there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The kept numbers of the cursors that exist, in order.
    numbers: Vec<u16>,
    /// The kept number a NEW cursor would take — past every number held,
    /// present or not.
    next: u16,
}

impl Shown {
    /// The cursors of `plan`, plus the ones in `also` (a keyboard's seat has
    /// a cursor with no pointer on it), against every number `prefs` holds.
    #[must_use]
    pub fn of(plan: &Plan, prefs: &Preferences, also: &[u16]) -> Self {
        let numbers: BTreeSet<u16> = plan
            .cursors
            .iter()
            .map(|c| c.number)
            .chain(also.iter().copied())
            .chain([FIRST])
            .collect();
        let held = prefs
            .devices
            .values()
            .filter_map(|d| d.cursor)
            .chain(prefs.cursors.keys().copied())
            .chain(numbers.iter().copied())
            .max()
            .unwrap_or(FIRST);
        Self {
            numbers: numbers.into_iter().collect(),
            next: held + 1,
        }
    }

    /// What cursor `n` is drawn as; `None` for one that does not exist now.
    #[must_use]
    pub fn label(&self, n: u16) -> Option<u16> {
        let at = self.numbers.iter().position(|x| *x == n)?;
        u16::try_from(at + 1).ok()
    }

    /// The kept number behind a drawn one — one past the last is a new cursor.
    #[must_use]
    pub fn number(&self, label: u16) -> Option<u16> {
        let at = usize::from(label.checked_sub(1)?);
        match at.cmp(&self.numbers.len()) {
            std::cmp::Ordering::Less => Some(self.numbers[at]),
            std::cmp::Ordering::Equal => Some(self.next),
            std::cmp::Ordering::Greater => None,
        }
    }

    /// Every drawn number, and the one a new cursor would be.
    #[must_use]
    pub fn labels(&self) -> Vec<u16> {
        (1..=u16::try_from(self.numbers.len() + 1).unwrap_or(u16::MAX)).collect()
    }
}

/// `prefs` with what `plan` placed written in, and every present device's
/// name remembered.
#[must_use]
pub fn remembered(mut prefs: Preferences, plan: &Plan) -> Preferences {
    for (id, n) in &plan.placed {
        prefs.devices.entry(id.clone()).or_default().cursor = Some(*n);
    }
    for d in &plan.devices {
        let entry = prefs
            .devices
            .entry(d.id.clone())
            .or_insert_with(|| DevicePrefs {
                cursor: Some(d.cursor),
                ..DevicePrefs::default()
            });
        if entry.name != d.name {
            entry.name.clone_from(&d.name);
        }
    }
    for c in &plan.cursors {
        prefs.cursors.entry(c.number).or_insert(CursorPrefs {
            enabled: c.enabled,
            colour: c.colour,
        });
    }
    prefs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What these tests are about: a machine where every device gets a
    /// cursor of its own. It is no longer the default.
    fn detecting() -> Preferences {
        Preferences {
            detect: true,
            ..Preferences::default()
        }
    }

    fn device(id: &str, kind: DeviceKind) -> Device {
        Device {
            id: id.to_owned(),
            name: id.to_owned(),
            kind,
            settings: PointerSettings {
                send_events: Some(SendEvents::Enabled),
                tap: (kind == DeviceKind::Touchpad).then_some(false),
                natural_scroll: Some(false),
                ..PointerSettings::default()
            },
        }
    }

    fn laptop() -> Vec<Device> {
        vec![
            device("1:1:keyboard", DeviceKind::Keyboard),
            device("2:2:touchpad", DeviceKind::Touchpad),
            device("3:3:dongle-mouse", DeviceKind::Mouse),
            device("4:4:bluetooth-mouse", DeviceKind::Mouse),
        ]
    }

    #[test]
    fn every_pointing_device_gets_a_cursor_of_its_own() {
        let p = plan(&laptop(), &detecting());
        let places: Vec<_> = p
            .devices
            .iter()
            .map(|d| (d.id.as_str(), d.cursor))
            .collect();
        assert_eq!(
            places,
            vec![
                ("2:2:touchpad", 1),
                ("3:3:dongle-mouse", 2),
                ("4:4:bluetooth-mouse", 3)
            ]
        );
        assert_eq!(p.cursors.len(), 3);
        assert_eq!(p.cursors[0].seat, "seat0");
        assert_eq!(p.cursors[2].seat, "ronoctua-cursor-3");
        assert_eq!(p.cursors[1].colour, 1, "each cursor a colour of its own");
        assert_eq!(p.placed.len(), 3, "and all three are written down");
    }

    #[test]
    fn the_keyboards_own_pointer_takes_no_cursor() {
        // AHEAD OF THE MOUSE, the order the compositor listed them in the VM:
        // taken as the first pointer, it kept `seat0` and the mouse got two.
        let mut devices = vec![device("4012:6878:keyd_virtual_pointer", DeviceKind::Mouse)];
        devices.extend(laptop());
        let mut prefs = detecting();
        // And a stale entry for it does not hold number one either.
        prefs.devices.insert(
            "4012:6878:keyd_virtual_pointer".to_owned(),
            DevicePrefs {
                cursor: Some(FIRST),
                ..DevicePrefs::default()
            },
        );
        let p = plan(&devices, &prefs);
        assert!(p.devices.iter().all(|d| !follows_the_keyboards(&d.id)));
        assert_eq!(p.devices[0].id, "2:2:touchpad");
        assert_eq!(p.devices[0].cursor, FIRST);
        assert!(p.placed.iter().all(|(id, _)| !follows_the_keyboards(id)));
    }

    #[test]
    fn one_touchpad_is_one_cursor_however_many_interfaces_it_has() {
        // The laptop's own, as the compositor names them — the idle `Mouse`
        // listed first, and a real mouse after both.
        let devices = vec![
            device("11311:39:SNSL0027:00_2C2F:0027_Mouse", DeviceKind::Mouse),
            device(
                "11311:39:SNSL0027:00_2C2F:0027_Touchpad",
                DeviceKind::Touchpad,
            ),
            device(
                "1133:50504:Logitech_Wireless_Receiver_Mouse",
                DeviceKind::Mouse,
            ),
        ];
        let p = plan(&devices, &detecting());
        let cursors: Vec<u16> = p.devices.iter().map(|d| d.cursor).collect();
        assert_eq!(cursors, vec![FIRST, FIRST, FIRST + 1]);
        assert_eq!(p.cursors.len(), 2, "two hands, two cursors");
        // And the second visit keeps them together from what was written.
        let again = plan(&devices, &remembered(detecting(), &p));
        assert_eq!(
            again.devices.iter().map(|d| d.cursor).collect::<Vec<_>>(),
            cursors
        );
    }

    #[test]
    fn an_emulated_pointer_is_off_while_a_real_hand_is_here() {
        let tablet = "QEMU QEMU USB Tablet".to_owned();
        let mut devices = vec![Device {
            name: tablet.clone(),
            ..device("1591:1:QEMU_QEMU_USB_Tablet", DeviceKind::Mouse)
        }];
        // ALONE, it is the machine's only pointer, and it moves.
        let alone = plan_with(&devices, &detecting(), std::slice::from_ref(&tablet));
        assert!(alone.devices[0].on && alone.cursors[0].shown);
        // With a real mouse beside it, it is off and its cursor is not drawn.
        devices.push(device("3:3:dongle-mouse", DeviceKind::Mouse));
        let p = plan_with(&devices, &detecting(), std::slice::from_ref(&tablet));
        let t = &p.devices[0];
        assert!(t.emulated && !t.on);
        assert_eq!(t.settings.send_events, Some(SendEvents::Disabled));
        assert!(!p.cursors[0].shown, "a cursor nobody can move is hidden");
        assert!(p.cursors[1].shown);
        // Its two mice join it, even where they were remembered apart.
        let mut two = devices.clone();
        let mouse = "VirtualPS/2 VMware VMMouse".to_owned();
        two.push(Device {
            name: mouse.clone(),
            ..device("2:19:VirtualPS/2_VMware_VMMouse", DeviceKind::Mouse)
        });
        let mut apart = detecting();
        apart.devices.insert(
            "2:19:VirtualPS/2_VMware_VMMouse".into(),
            DevicePrefs {
                cursor: Some(5),
                ..DevicePrefs::default()
            },
        );
        let names = [tablet.clone(), mouse];
        let first = plan_with(&two, &apart, &names);
        let again = plan_with(&two, &remembered(apart, &first), &names);
        let theirs: Vec<u16> = again
            .devices
            .iter()
            .filter(|d| d.emulated)
            .map(|d| d.cursor)
            .collect();
        assert_eq!(theirs.len(), 2);
        assert_eq!(theirs[0], theirs[1], "{theirs:?}");
        // And the switch turns it back on.
        let on = Preferences {
            emulated: true,
            ..detecting()
        };
        let p = plan_with(&devices, &on, std::slice::from_ref(&tablet));
        assert!(p.devices[0].on && p.cursors[0].shown);
    }

    #[test]
    fn together_puts_every_hand_on_one_cursor_and_forgets_nothing() {
        let apart = plan(&laptop(), &detecting());
        let prefs = remembered(detecting(), &apart);
        let one = plan(
            &laptop(),
            &Preferences {
                together: true,
                ..prefs.clone()
            },
        );
        assert!(one.devices.iter().all(|d| d.cursor == FIRST));
        assert_eq!(one.cursors.len(), 1);
        // And off again, each is where it was.
        let again = plan(&laptop(), &prefs);
        assert_eq!(
            again.devices.iter().map(|d| d.cursor).collect::<Vec<_>>(),
            apart.devices.iter().map(|d| d.cursor).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_screen_numbers_the_cursors_there_are_with_no_hole() {
        let mut prefs = detecting();
        // Number 2 held by a mouse that is not plugged in.
        prefs.devices.insert(
            "9:9:away".into(),
            DevicePrefs {
                cursor: Some(2),
                ..DevicePrefs::default()
            },
        );
        let p = plan(&laptop(), &prefs);
        let kept: Vec<u16> = p.cursors.iter().map(|c| c.number).collect();
        assert_eq!(kept, vec![1, 3, 4], "the kept numbers skip the away one");
        let shown = Shown::of(&p, &remembered(prefs, &p), &[]);
        assert_eq!(shown.label(1), Some(1));
        assert_eq!(shown.label(3), Some(2));
        assert_eq!(shown.label(4), Some(3));
        assert_eq!(shown.label(2), None, "the away one is not drawn");
        assert_eq!(shown.number(2), Some(3));
        assert_eq!(shown.number(4), Some(5), "one past the last is a new one");
        assert_eq!(shown.number(5), None);
        assert_eq!(shown.labels(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn a_switched_off_cursor_is_not_drawn() {
        let mut prefs = detecting();
        prefs.cursors.insert(
            FIRST + 1,
            CursorPrefs {
                enabled: false,
                colour: 1,
            },
        );
        let p = plan(&laptop(), &prefs);
        assert!(p.cursors[0].shown);
        assert!(!p.cursors[1].shown);
    }

    #[test]
    fn the_first_seat_with_no_hand_is_not_drawn() {
        // Only the keyboards' own pointer: it stays on the first seat and is
        // no hand, so that seat's cursor would stand still in a corner.
        let devices = vec![device("4012:6878:keyd_virtual_pointer", DeviceKind::Mouse)];
        let p = plan(&devices, &detecting());
        assert_eq!(p.cursors.len(), 1);
        assert!(!p.cursors[0].shown);
    }

    #[test]
    fn two_unknown_vendors_are_not_one_device() {
        let devices = vec![
            device("0:0:virtual_one", DeviceKind::Mouse),
            device("0:0:virtual_two", DeviceKind::Mouse),
        ];
        let p = plan(&devices, &detecting());
        assert_eq!(p.cursors.len(), 2);
    }

    #[test]
    fn with_detection_off_everything_drives_the_first_cursor() {
        let prefs = Preferences {
            detect: false,
            ..detecting()
        };
        let p = plan(&laptop(), &prefs);
        assert!(p.devices.iter().all(|d| d.cursor == FIRST));
        assert_eq!(p.cursors.len(), 1);
    }

    #[test]
    fn a_device_back_from_a_drawer_gets_its_old_cursor() {
        let first = plan(&laptop(), &detecting());
        let prefs = remembered(detecting(), &first);
        // The dongle mouse is unplugged; a NEW mouse arrives. It must not be
        // handed cursor 2, which the dongle still owns.
        let mut now = laptop();
        now.retain(|d| d.id != "3:3:dongle-mouse");
        now.push(device("5:5:new-mouse", DeviceKind::Mouse));
        let p = plan(&now, &prefs);
        let new = p.devices.iter().find(|d| d.id == "5:5:new-mouse").unwrap();
        assert_eq!(new.cursor, 4);
        // And the dongle comes back to 2.
        let back = plan(&laptop(), &remembered(prefs, &p));
        let dongle = back
            .devices
            .iter()
            .find(|d| d.id == "3:3:dongle-mouse")
            .unwrap();
        assert_eq!(dongle.cursor, 2);
    }

    #[test]
    fn two_devices_can_share_a_cursor() {
        let mut prefs = remembered(detecting(), &plan(&laptop(), &detecting()));
        prefs.devices.get_mut("4:4:bluetooth-mouse").unwrap().cursor = Some(1);
        let p = plan(&laptop(), &prefs);
        assert_eq!(
            p.cursors[0].devices,
            vec!["2:2:touchpad", "4:4:bluetooth-mouse"]
        );
    }

    #[test]
    fn a_cursor_switched_off_switches_its_devices_off() {
        let mut prefs = remembered(detecting(), &plan(&laptop(), &detecting()));
        prefs.cursors.insert(
            2,
            CursorPrefs {
                enabled: false,
                colour: 1,
            },
        );
        let p = plan(&laptop(), &prefs);
        let dongle = p.devices.iter().find(|d| d.cursor == 2).unwrap();
        assert_eq!(dongle.settings.send_events, Some(SendEvents::Disabled));
        let pad = p.devices.iter().find(|d| d.cursor == 1).unwrap();
        assert_eq!(pad.settings.send_events, Some(SendEvents::Enabled));
    }

    #[test]
    fn a_default_reaches_only_the_devices_that_have_it() {
        let p = plan(&laptop(), &detecting());
        let pad = p
            .devices
            .iter()
            .find(|d| d.kind == DeviceKind::Touchpad)
            .unwrap();
        assert_eq!(pad.settings.tap, Some(true), "the touchpad default taps");
        let mouse = p
            .devices
            .iter()
            .find(|d| d.kind == DeviceKind::Mouse)
            .unwrap();
        assert_eq!(
            mouse.settings.tap, None,
            "and asks a mouse nothing about it"
        );
        // Only what differs from now is asked live.
        assert!(
            pad.to_ask()
                .contains(&ronoctua_compositor::Setting::Tap(true))
        );
        assert!(
            !pad.to_ask()
                .contains(&ronoctua_compositor::Setting::SendEvents(
                    SendEvents::Enabled
                ))
        );
    }

    #[test]
    fn two_identical_mice_are_one_device() {
        let mut two = laptop();
        two.push(device("3:3:dongle-mouse", DeviceKind::Mouse));
        let p = plan(&two, &detecting());
        assert_eq!(
            p.devices
                .iter()
                .filter(|d| d.id == "3:3:dongle-mouse")
                .count(),
            1
        );
    }
}
