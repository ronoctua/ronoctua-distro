//! Turning the compositor's JSON into the contract's vocabulary.
//!
//! Everything here is one-directional: JSON in, `ronoctua_compositor` types
//! out. An unknown field is ignored, because a compositor adds them and a
//! desktop that broke over one would be a desktop held hostage by a field
//! nobody reads.

use std::collections::BTreeMap;

use ronoctua_compositor::{
    AccelProfile, ButtonMap, ClickMethod, Device, DeviceKind, DragLock, FingerDrag, Logical, Mode,
    Output, PointerSettings, ScrollMethod, Seat, SendEvents, Transform, Window, Workspace,
};
use serde_json::Value;

/// Every window in a tree, flattened.
///
/// The tree has outputs, workspaces and containers in it; what we want are
/// the leaves that are somebody's window, and the test for that is a `pid` —
/// a container the compositor made up to hold two others does not have one.
#[must_use]
pub fn windows_in(tree: &Value) -> Vec<Window> {
    let mut found = Vec::new();
    walk(tree, &mut found);
    found
}

fn walk(node: &Value, into: &mut Vec<Window>) {
    if let Some(w) = window_of(node) {
        into.push(w);
    }
    for kind in ["nodes", "floating_nodes"] {
        if let Some(kids) = node.get(kind).and_then(Value::as_array) {
            for kid in kids {
                walk(kid, into);
            }
        }
    }
}

/// How much of window `id`'s container is NOT its content: `(width,
/// height)` of the border and the title bar together, from the tree.
///
/// Three rectangles, because the tree moves the title bar OUT of `rect` and
/// into `deco_rect`; `window_rect` is the content, inside `rect`. `None` for
/// a window not found, or not yet laid out — its content still `0 × 0`.
pub fn decoration_in(tree: &Value, id: u64) -> Option<(i64, i64)> {
    fn find(node: &Value, id: u64) -> Option<&Value> {
        if node.get("id").and_then(Value::as_u64) == Some(id) {
            return Some(node);
        }
        ["nodes", "floating_nodes"]
            .iter()
            .filter_map(|kind| node.get(kind).and_then(Value::as_array))
            .flatten()
            .find_map(|kid| find(kid, id))
    }
    let node = find(tree, id)?;
    let size = |key: &str| {
        let r = node.get(key)?;
        Some((
            r.get("width").and_then(Value::as_i64)?,
            r.get("height").and_then(Value::as_i64)?,
        ))
    };
    let (outer_w, outer_h) = size("rect")?;
    let (inner_w, inner_h) = size("window_rect")?;
    let title = size("deco_rect").map_or(0, |(_, h)| h);
    (inner_w > 0 && inner_h > 0).then_some((outer_w - inner_w, outer_h - inner_h + title))
}

/// One node, if it is a window.
fn window_of(node: &Value) -> Option<Window> {
    // THE PID IS WHAT MAKES IT A WINDOW, and it is also the field the whole
    // buffer correlation hangs on: the control socket of a buffer is named
    // after it. A node without one is furniture.
    let pid = node.get("pid").and_then(Value::as_i64)?;
    let id = node.get("id").and_then(Value::as_u64)?;
    // An X11 window has no `app_id`; what it has is a class, two levels down
    // in `window_properties`. Reading it means an X11 buffer of ours is
    // recognised as ours, which is the whole of the fallback.
    let app_id = node
        .get("app_id")
        .and_then(Value::as_str)
        .or_else(|| {
            node.get("window_properties")
                .and_then(|p| p.get("class"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned);
    Some(Window {
        id,
        title: node
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        app_id,
        pid: i32::try_from(pid).ok(),
        is_focused: node
            .get("focused")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Every output, keyed by connector.
#[must_use]
pub fn outputs_in(said: &Value) -> BTreeMap<String, Output> {
    let mut all = BTreeMap::new();
    let Some(list) = said.as_array() else {
        return all;
    };
    for one in list {
        if let Some(o) = output_of(one) {
            all.insert(o.name.clone(), o);
        }
    }
    all
}

/// The connector of whichever output has the keyboard.
#[must_use]
pub fn focused_in(said: &Value) -> Option<String> {
    said.as_array()?
        .iter()
        .find(|o| o.get("focused").and_then(Value::as_bool) == Some(true))
        .and_then(|o| o.get("name").and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

/// Every workspace, from the answer to `get_workspaces`.
///
/// OCCUPIED IS `focus` NOT BEING EMPTY: the reply does not describe the
/// tiled children (`nodes` is always `[]` there), but `focus` lists the id of
/// every direct child, tiled and floating, and a workspace with nothing open
/// has none. The floating list is asked too, so the answer does not rest on
/// one field alone.
#[must_use]
pub fn workspaces_in(said: &Value) -> Vec<Workspace> {
    let Some(all) = said.as_array() else {
        return Vec::new();
    };
    let non_empty = |w: &Value, k: &str| {
        w.get(k)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
    };
    all.iter()
        .map(|w| Workspace {
            // `-1` is how the compositor says the name is not a number.
            number: w
                .get("num")
                .and_then(Value::as_i64)
                .and_then(|n| u32::try_from(n).ok()),
            focused: w.get("focused").and_then(Value::as_bool) == Some(true),
            occupied: non_empty(w, "focus") || non_empty(w, "floating_nodes"),
        })
        .collect()
}

/// The room windows have on the focused workspace, from the answer to
/// `get_workspaces`.
pub fn room_in(said: &Value) -> Option<crate::wire::Room> {
    let focused = said
        .as_array()?
        .iter()
        .find(|w| w.get("focused").and_then(Value::as_bool) == Some(true))?;
    let rect = focused.get("rect")?;
    let n = |k: &str| rect.get(k).and_then(Value::as_i64);
    Some(crate::wire::Room {
        x: n("x")?,
        y: n("y")?,
        width: n("width")?,
        height: n("height")?,
    })
}

fn output_of(o: &Value) -> Option<Output> {
    let name = o.get("name").and_then(Value::as_str)?.to_owned();
    let said = |k: &str| {
        o.get(k)
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_owned()
    };
    let modes: Vec<Mode> = o
        .get("modes")
        .and_then(Value::as_array)
        .map(|ms| ms.iter().filter_map(mode_of).collect())
        .unwrap_or_default();
    // A DISABLED OUTPUT CARRIES ALMOST NOTHING: no scale, no transform, no
    // current mode, and a rect of zeroes. That is why `logical` is an option
    // rather than a struct full of defaults — "off" is not "at the origin,
    // unturned, at scale one".
    let on = o.get("active").and_then(Value::as_bool).unwrap_or(false);
    let logical = on.then(|| {
        let rect = o.get("rect");
        let at = |k: &str| {
            rect.and_then(|r| r.get(k))
                .and_then(Value::as_i64)
                .unwrap_or(0)
        };
        Logical {
            x: i32::try_from(at("x")).unwrap_or(0),
            y: i32::try_from(at("y")).unwrap_or(0),
            width: u32::try_from(at("width")).unwrap_or(0),
            height: u32::try_from(at("height")).unwrap_or(0),
            scale: o.get("scale").and_then(Value::as_f64).unwrap_or(1.0),
            transform: o
                .get("transform")
                .and_then(Value::as_str)
                .and_then(turn_of)
                .unwrap_or_default(),
        }
    });
    // The mode that is running, as an INDEX into the list — and `None` when
    // it is not in it, which happens when the compositor synthesised the
    // current mode from the size it ended up with.
    let current = o.get("current_mode").and_then(mode_of);
    let current_mode = current.and_then(|c| modes.iter().position(|m| *m == c));
    Some(Output {
        name,
        make: said("make"),
        model: said("model"),
        serial: o
            .get("serial")
            .and_then(Value::as_str)
            .filter(|s| *s != "Unknown")
            .map(ToOwned::to_owned),
        modes,
        current_mode,
        is_custom_mode: current.is_some() && current_mode.is_none(),
        vrr_supported: o
            .get("features")
            .and_then(|f| f.get("adaptive_sync"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        vrr_enabled: o.get("adaptive_sync_status").and_then(Value::as_str) == Some("enabled"),
        logical,
    })
}

fn mode_of(m: &Value) -> Option<Mode> {
    Some(Mode {
        width: u16::try_from(m.get("width")?.as_u64()?).ok()?,
        height: u16::try_from(m.get("height")?.as_u64()?).ok()?,
        // The rate is millihertz here and hertz when asked for. The
        // asymmetry is the compositor's; `Mode::asked` is where it is paid.
        refresh_rate: u32::try_from(m.get("refresh")?.as_u64()?).ok()?,
    })
}

/// The one spelling of a transform, read back.
fn turn_of(word: &str) -> Option<Transform> {
    Some(match word {
        "normal" => Transform::Normal,
        "90" => Transform::R90,
        "180" => Transform::R180,
        "270" => Transform::R270,
        "flipped" => Transform::Flipped,
        "flipped-90" => Transform::Flipped90,
        "flipped-180" => Transform::Flipped180,
        "flipped-270" => Transform::Flipped270,
        _ => return None,
    })
}

/// Every input device in a `get_inputs` reply.
///
/// A device the compositor calls anything but a pointer, a touchpad or a
/// keyboard is [`DeviceKind::Other`]: it is listed, and nothing configures it.
#[must_use]
pub fn devices_in(reply: &Value) -> Vec<Device> {
    reply
        .as_array()
        .map(|all| all.iter().filter_map(device_of).collect())
        .unwrap_or_default()
}

fn device_of(v: &Value) -> Option<Device> {
    let id = v.get("identifier").and_then(Value::as_str)?.to_owned();
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&id)
        .to_owned();
    let kind = match v.get("type").and_then(Value::as_str) {
        Some("pointer") => DeviceKind::Mouse,
        Some("touchpad") => DeviceKind::Touchpad,
        Some("keyboard") => DeviceKind::Keyboard,
        _ => DeviceKind::Other,
    };
    let mut settings = v.get("libinput").map(settings_of).unwrap_or_default();
    // THE SCROLL FACTOR IS THE COMPOSITOR'S, not libinput's: it sits beside
    // the `libinput` object rather than in it, and every pointer has one.
    settings.scroll_factor = v.get("scroll_factor").and_then(Value::as_f64);
    Some(Device {
        id,
        name,
        kind,
        settings,
    })
}

/// A field that is present only when the device supports it, and says
/// `unknown` when libinput could not tell — which is read as absent.
fn word<'a>(li: &'a Value, key: &str) -> Option<&'a str> {
    li.get(key)
        .and_then(Value::as_str)
        .filter(|w| *w != "unknown")
}

fn flag(li: &Value, key: &str) -> Option<bool> {
    match word(li, key)? {
        "enabled" => Some(true),
        "disabled" => Some(false),
        _ => None,
    }
}

fn map_of(li: &Value, key: &str) -> Option<ButtonMap> {
    match word(li, key)? {
        "lrm" => Some(ButtonMap::Lrm),
        "lmr" => Some(ButtonMap::Lmr),
        _ => None,
    }
}

fn settings_of(li: &Value) -> PointerSettings {
    PointerSettings {
        send_events: match word(li, "send_events") {
            Some("enabled") => Some(SendEvents::Enabled),
            Some("disabled") => Some(SendEvents::Disabled),
            Some("disabled_on_external_mouse") => Some(SendEvents::DisabledOnExternalMouse),
            _ => None,
        },
        accel_speed: li.get("accel_speed").and_then(Value::as_f64),
        accel_profile: match word(li, "accel_profile") {
            Some("flat") => Some(AccelProfile::Flat),
            Some("adaptive") => Some(AccelProfile::Adaptive),
            _ => None,
        },
        natural_scroll: flag(li, "natural_scroll"),
        left_handed: flag(li, "left_handed"),
        tap: flag(li, "tap"),
        tap_button_map: map_of(li, "tap_button_map"),
        tap_drag: flag(li, "tap_drag"),
        tap_drag_lock: match word(li, "tap_drag_lock") {
            Some("enabled") => Some(DragLock::Enabled),
            Some("enabled_sticky") => Some(DragLock::Sticky),
            Some("disabled") => Some(DragLock::Disabled),
            _ => None,
        },
        finger_drag: match word(li, "three_finger_drag") {
            Some("three_finger") => Some(FingerDrag::ThreeFinger),
            Some("four_finger") => Some(FingerDrag::FourFinger),
            Some("disabled") => Some(FingerDrag::Disabled),
            _ => None,
        },
        click_method: match word(li, "click_method") {
            Some("none") => Some(ClickMethod::None),
            Some("button_areas") => Some(ClickMethod::ButtonAreas),
            Some("clickfinger") => Some(ClickMethod::Clickfinger),
            _ => None,
        },
        clickfinger_button_map: map_of(li, "clickfinger_button_map"),
        middle_emulation: flag(li, "middle_emulation"),
        scroll_method: match word(li, "scroll_method") {
            Some("none") => Some(ScrollMethod::None),
            Some("two_finger") => Some(ScrollMethod::TwoFinger),
            Some("edge") => Some(ScrollMethod::Edge),
            Some("on_button_down") => Some(ScrollMethod::OnButtonDown),
            _ => None,
        },
        scroll_button: li
            .get("scroll_button")
            .and_then(Value::as_u64)
            .and_then(|b| u32::try_from(b).ok()),
        scroll_button_lock: flag(li, "scroll_button_lock"),
        disable_while_typing: flag(li, "dwt"),
        disable_while_trackpointing: flag(li, "dwtp"),
        scroll_factor: None,
    }
}

/// Every seat in a `get_seats` reply.
#[must_use]
pub fn seats_in(reply: &Value) -> Vec<Seat> {
    let Some(all) = reply.as_array() else {
        return Vec::new();
    };
    all.iter()
        .filter_map(|s| {
            let name = s.get("name").and_then(Value::as_str)?.to_owned();
            let devices = s
                .get("devices")
                .and_then(Value::as_array)
                .map(|ds| {
                    ds.iter()
                        .filter_map(|d| d.get("identifier").and_then(Value::as_str))
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            // `focus` is a container id, and 0 is the compositor's "nothing".
            let focus = s.get("focus").and_then(Value::as_u64).filter(|f| *f != 0);
            Some(Seat {
                name,
                devices,
                focus,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_device_says_only_what_it_supports() {
        // The shape measured from the pinned compositor's `ipc-json.c`: the
        // `libinput` object carries a key only when the device has it, and
        // `unknown` when libinput could not answer.
        let reply: serde_json::Value = serde_json::from_str(
            r#"[
              {"identifier":"1133:50498:Logitech_Wireless_Receiver_Mouse","name":"Logitech Wireless Receiver Mouse",
               "type":"pointer","scroll_factor":1.0,
               "libinput":{"send_events":"enabled","accel_speed":0.0,"accel_profile":"adaptive",
                           "natural_scroll":"disabled","left_handed":"disabled","middle_emulation":"disabled",
                           "scroll_method":"on_button_down","scroll_button":274,"scroll_button_lock":"disabled"}},
              {"identifier":"11311:39:SNSL0027:00_2C2F:0027_Touchpad","name":"SNSL0027:00 2C2F:0027 Touchpad",
               "type":"touchpad","scroll_factor":1.0,
               "libinput":{"send_events":"enabled","tap":"enabled","tap_button_map":"lrm","tap_drag":"enabled",
                           "tap_drag_lock":"enabled_sticky","three_finger_drag":"unknown","accel_speed":0.2,
                           "click_method":"clickfinger","dwt":"enabled","scroll_method":"two_finger"}},
              {"identifier":"1:1:AT_Translated_Set_2_keyboard","name":"AT Translated Set 2 keyboard","type":"keyboard"}
            ]"#,
        )
        .unwrap();
        let ds = super::devices_in(&reply);
        assert_eq!(ds.len(), 3);
        let mouse = &ds[0];
        assert_eq!(mouse.kind, super::DeviceKind::Mouse);
        assert_eq!(mouse.settings.scroll_button, Some(274));
        assert_eq!(mouse.settings.tap, None, "a mouse has no tapping");
        let pad = &ds[1];
        assert_eq!(pad.kind, super::DeviceKind::Touchpad);
        assert_eq!(pad.settings.tap, Some(true));
        assert_eq!(pad.settings.tap_drag_lock, Some(super::DragLock::Sticky));
        assert_eq!(pad.settings.finger_drag, None, "`unknown` is absent");
        assert_eq!(ds[2].kind, super::DeviceKind::Keyboard);
    }

    #[test]
    fn a_seat_lists_its_devices_and_nothing_focused_is_none() {
        let reply: serde_json::Value = serde_json::from_str(
            r#"[{"name":"seat0","capabilities":3,"focus":0,
                 "devices":[{"identifier":"1:1:kb"},{"identifier":"2:2:mouse"}]},
                {"name":"ronoctua-cursor-2","capabilities":1,"focus":42,
                 "devices":[{"identifier":"3:3:pebble"}]}]"#,
        )
        .unwrap();
        let ss = super::seats_in(&reply);
        assert_eq!(ss[0].devices, vec!["1:1:kb", "2:2:mouse"]);
        assert_eq!(ss[0].focus, None);
        assert_eq!(ss[1].focus, Some(42));
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn a_window_is_a_node_with_a_pid() {
        // The tree is outputs, then workspaces, then containers. Only the
        // leaves with a pid are somebody's window; the rest is furniture,
        // and counting it would put phantom entries in the buffer list.
        let tree = json!({
            "type": "root", "name": "root", "nodes": [{
                "type": "output", "name": "eDP-1", "nodes": [{
                    "type": "workspace", "name": "1", "nodes": [
                        { "type": "con", "id": 7, "name": "a terminal",
                          "app_id": "br.ronoctua.term", "pid": 4242, "focused": true },
                        { "type": "con", "id": 8, "name": null, "nodes": [
                            { "type": "con", "id": 9, "name": "nested",
                              "app_id": "x", "pid": 4243, "focused": false }
                        ]}
                    ]
                }]
            }]
        });
        let ws = windows_in(&tree);
        assert_eq!(ws.len(), 2, "two windows, and no containers");
        assert_eq!(ws[0].id, 7);
        assert_eq!(ws[0].pid, Some(4242));
        assert_eq!(ws[0].app_id.as_deref(), Some("br.ronoctua.term"));
        assert!(ws[0].is_focused);
        assert_eq!(ws[1].id, 9, "and it walks all the way down");
    }

    #[test]
    fn an_x11_window_is_named_by_its_class() {
        // Without this an X11 buffer of ours is not recognised as ours, and
        // the modeline shows a foreign window where there is one of ours.
        let tree = json!({ "type": "root", "nodes": [{
            "type": "con", "id": 3, "name": "xterm", "pid": 99,
            "window_properties": { "class": "br.ronoctua.legacy" }
        }]});
        let ws = windows_in(&tree);
        assert_eq!(ws[0].app_id.as_deref(), Some("br.ronoctua.legacy"));
    }

    #[test]
    fn an_output_that_is_off_has_no_place_and_no_mode() {
        let said = json!([{
            "name": "HDMI-A-1", "make": "Maker", "model": "M2", "serial": "S9",
            "active": false, "rect": { "x": 0, "y": 0, "width": 0, "height": 0 },
            "modes": [{ "width": 1920, "height": 1080, "refresh": 60000 }]
        }]);
        let all = outputs_in(&said);
        let o = &all["HDMI-A-1"];
        assert!(o.logical.is_none(), "off is not `at the origin, unturned`");
        assert_eq!(o.modes.len(), 1, "but it still says what it could do");
        assert_eq!(o.naming().name(), "Maker M2 S9");
    }

    #[test]
    fn a_monitor_that_says_nothing_still_reads() {
        // The compositor writes the word `Unknown` rather than leaving the
        // field out; a serial of `Unknown` is no serial at all.
        let said = json!([{
            "name": "eDP-1", "make": "Unknown", "model": "Unknown", "serial": "Unknown",
            "active": true, "scale": 1.0, "transform": "normal",
            "rect": { "x": 0, "y": 0, "width": 1920, "height": 1200 },
            "modes": [], "adaptive_sync_status": "disabled"
        }]);
        let all = outputs_in(&said);
        assert_eq!(all["eDP-1"].serial, None);
        assert_eq!(all["eDP-1"].identity(), "Unknown Unknown Unknown");
    }

    #[test]
    fn the_running_mode_is_an_index_and_may_not_be_in_the_list() {
        let with = json!([{
            "name": "eDP-1", "active": true, "scale": 1.0, "transform": "90",
            "rect": { "x": 0, "y": 0, "width": 1200, "height": 1920 },
            "modes": [
                { "width": 1920, "height": 1200, "refresh": 59951 },
                { "width": 1920, "height": 1200, "refresh": 48000 }
            ],
            "current_mode": { "width": 1920, "height": 1200, "refresh": 48000 },
            "features": { "adaptive_sync": true }, "adaptive_sync_status": "enabled"
        }]);
        let all = outputs_in(&with);
        let o = &all["eDP-1"];
        assert_eq!(o.current_mode, Some(1), "the second one, not the first");
        assert!(!o.is_custom_mode);
        assert!(o.vrr_supported && o.vrr_enabled);
        assert_eq!(o.logical.unwrap().transform, Transform::R90);

        // A mode the list does not have: no index, and it is marked custom.
        let without = json!([{
            "name": "eDP-1", "active": true, "scale": 1.0,
            "rect": { "x": 0, "y": 0, "width": 800, "height": 600 },
            "modes": [{ "width": 1920, "height": 1200, "refresh": 59951 }],
            "current_mode": { "width": 800, "height": 600, "refresh": 60000 }
        }]);
        let odd = outputs_in(&without);
        assert_eq!(odd["eDP-1"].current_mode, None);
        assert!(odd["eDP-1"].is_custom_mode);
    }

    #[test]
    fn a_field_from_a_newer_compositor_is_not_an_error() {
        let said = json!([{
            "name": "eDP-1", "active": true, "scale": 2.0,
            "rect": { "x": 0, "y": 0, "width": 960, "height": 600 },
            "modes": [], "something_new": { "nested": [1, 2, 3] }
        }]);
        assert_eq!(outputs_in(&said).len(), 1);
    }

    #[test]
    fn the_focused_output_is_the_one_that_says_so() {
        let said = json!([
            { "name": "eDP-1", "active": true, "focused": false, "modes": [] },
            { "name": "DP-2", "active": true, "focused": true, "modes": [] }
        ]);
        assert_eq!(focused_in(&said).as_deref(), Some("DP-2"));
        // And nothing focused is a legitimate answer.
        assert_eq!(focused_in(&json!([])), None);
    }

    /// FAILS IF: a workspace holding only a floating window reads as empty,
    /// a named one invents a number, or the focused empty one reads as
    /// occupied — the shape is the pinned compositor's `get_workspaces`,
    /// where `nodes` is always empty and `focus` is what lists the children.
    #[test]
    fn a_workspace_is_occupied_when_it_lists_a_child() {
        let said = json!([
            {"num": 1, "name": "1", "focused": false, "nodes": [], "focus": [11, 12], "floating_nodes": []},
            {"num": 2, "name": "2", "focused": true, "nodes": [], "focus": [], "floating_nodes": []},
            {"num": -1, "name": "mail", "focused": false, "nodes": [], "focus": [30], "floating_nodes": []},
            {"num": 4, "name": "4", "focused": false, "nodes": [], "focus": [],
             "floating_nodes": [{"id": 40}]}
        ]);
        let all = workspaces_in(&said);
        let seen: Vec<(Option<u32>, bool, bool)> = all
            .iter()
            .map(|w| (w.number, w.focused, w.occupied))
            .collect();
        assert_eq!(
            seen,
            [
                (Some(1), false, true),
                (Some(2), true, false),
                (None, false, true),
                (Some(4), false, true),
            ]
        );
        assert!(workspaces_in(&json!({})).is_empty());
    }

    /// FAILS IF: the title bar stops being counted — the tree keeps it in
    /// `deco_rect`, out of `rect` — or a window not yet laid out gives a
    /// decoration made of its whole size.
    #[test]
    fn the_decoration_is_border_and_title_bar() {
        let tree = serde_json::json!({
            "id": 1,
            "nodes": [{
                "id": 7,
                "rect": {"x": 0, "y": 19, "width": 512, "height": 749},
                "window_rect": {"x": 2, "y": 0, "width": 508, "height": 747},
                "deco_rect": {"x": 0, "y": 0, "width": 512, "height": 19},
                "nodes": []
            }, {
                "id": 8,
                "rect": {"x": 0, "y": 0, "width": 512, "height": 768},
                "window_rect": {"x": 0, "y": 0, "width": 0, "height": 0},
                "nodes": []
            }]
        });
        assert_eq!(decoration_in(&tree, 7), Some((4, 21)));
        assert_eq!(decoration_in(&tree, 8), None);
        assert_eq!(decoration_in(&tree, 9), None);
    }
}
