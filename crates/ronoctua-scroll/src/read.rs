//! Turning the compositor's JSON into the contract's vocabulary.
//!
//! Everything here is one-directional: JSON in, `ronoctua_compositor` types
//! out. An unknown field is ignored, because a compositor adds them and a
//! desktop that broke over one would be a desktop held hostage by a field
//! nobody reads.

use std::collections::BTreeMap;

use ronoctua_compositor::{Logical, Mode, Output, Transform, Window};
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

#[cfg(test)]
mod tests {
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
}
