//! THE WINDOWS THAT OPEN FLOATING, and how big — one table.
//!
//! # Why a table, and why here
//!
//! It was a list inside `terminal::floating_rules`, which was right while the
//! only readers were the rule export and the three windows it floats. Then the
//! calculator came with a control on its header that floats it again after it
//! was tiled, at the SAME size the rule opens it at — and a size written in
//! the rule and again in the toggle is two numbers for one window, free to
//! disagree. So the size is a row of [`FLOATING`], and both read the row: the
//! rule export ([`rules`], printed by `export-window-rules`) and the window
//! that floats itself again ([`portion_of`], through
//! `Compositor::float_centred`).
//!
//! The shell does not place windows (invariant 4): a row says WHAT a window is
//! and how much of the room it wants, and the compositor decides the rest.
//!
//! # Why the numbers are not tokens
//!
//! They are the compositor's geometry, like `gaps` and the border's width —
//! invariant 9's tokens are the design system's, and no component of it draws
//! these sizes.

use ronoctua_compositor::Portion;

/// One window that opens floating: its `app_id`, and its size in hundredths
/// of the room windows have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Floating {
    /// The `app_id` the window sets.
    pub app_id: &'static str,
    /// How much of the room it takes.
    pub portion: Portion,
}

/// A row of the table.
const fn row(app_id: &'static str, width_ppt: u8, height_ppt: u8) -> Floating {
    Floating {
        app_id,
        portion: Portion {
            width_ppt,
            height_ppt,
        },
    }
}

/// Every window that opens floating.
///
/// - THE MONITOR at 70%: a list one reads over the work and puts away.
/// - THE RESCUE CONSOLE is WIDER, and that is the only difference: it is read
///   while something is wrong, and a list with the log cut off is a diagnosis
///   one has to scroll for.
/// - THE CAMERA APP at 60%: a picture of a face the owner asked to see in a
///   window of its own over the work, not a tile in the strip that would push
///   every other window aside.
/// - THE CALCULATOR, compact: narrower than it is tall, the shape of a keypad
///   with its answer over it — a sum is done over the work and the window put
///   away, and its header's control tiles it when it is to stay.
/// - THE SEARCH OVER EVERYTHING (`LEADER SPC`) at the whole room: the search
///   window under its own `app_id`, a window over the work and not chrome
///   (invariant 14) — the plain search (`LEADER d w`) tiles.
pub const FLOATING: [Floating; 5] = [
    row(crate::terminal::APP_ID_MONITOR, 70, 70),
    row(crate::terminal::APP_ID_RESCUE, 90, 90),
    row(crate::camera::APP_ID, 60, 60),
    row(crate::calc_window::APP_ID, 34, 72),
    row(crate::fast_access::APP_ID_FLOAT, 100, 100),
];

/// How much of the room the window with `app_id` floats at — `None` for a
/// window that is not in the table.
#[must_use]
pub fn portion_of(app_id: &str) -> Option<Portion> {
    FLOATING
        .iter()
        .find(|f| f.app_id == app_id)
        .map(|f| f.portion)
}

/// The rules that make every window of [`FLOATING`] float.
///
/// # Why it is generated from here, and not written in the two configs
///
/// The `app_id` and the rule's regex have to be the same string, and it would
/// exist in THREE places — the window's file, `nix/modules/scroll.nix` and
/// `nix/dev.nix`. A string that crosses a border needs a check (`AGENTS.md`),
/// and generating it is the check that cannot go stale: `export-window-rules`
/// prints this, and both configs take it in — the same arrangement as the
/// chords in `nix/compositor-binds.nix`.
///
/// `r"…"` and not plain quotes: the criterion's value is a regex with a
/// backslash in it (`^br\.ronoctua\.monitor$`), and writing it in a raw string
/// is what keeps Rust's escapes out of a string the compositor parses.
#[must_use]
pub fn rules() -> String {
    FLOATING
        .iter()
        .map(|f| {
            let regex = f.app_id.replace('.', r"\.");
            let (w, h) = (f.portion.width_ppt, f.portion.height_ppt);
            // `ppt` IS NOT OPTIONAL: a floating container is sized in PIXELS
            // unless the unit says otherwise, so `resize set width 70` would
            // make a seventy-pixel window — which reads as the monitor having
            // failed to open rather than as a unit that was left out.
            //
            // And `move position center`, because a new floating window is
            // not centred on its own. Both are what `float_centred` says too
            // (`ronoctua-scroll`), so the calculator floated again by its
            // control lands where and as big as this rule opened it — the
            // workspace's room, the footer and the right column carved out
            // (`one-floating-table`). There is no equivalent of asking for it
            // to open focused; one does, and that is the compositor's own
            // behaviour rather than something to spell.
            format!(
                "for_window [app_id=\"^{regex}$\"] \
                 floating enable, \
                 resize set width {w} ppt height {h} ppt, \
                 move position center\n"
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_matches_the_app_id_the_window_actually_sets() {
        // THE RULE AND THE WINDOW ARE ONE STRING. Written apart, an `app_id`
        // renamed here would give a rule that matches nothing — and the symptom
        // is the monitor opening TILED, which looks like the shortcut working.
        let rule = rules();
        assert!(
            rule.contains(&crate::terminal::APP_ID_MONITOR.replace('.', r"\.")),
            "the rule does not name the window's app id:\n{rule}"
        );
        // The dots are escaped: `br.ronoctua.monitor` as a regex would also
        // match `brXronoctuaYmonitor`, and — worse — is not what one reads.
        assert!(rule.contains(r"^br\.ronoctua\.monitor$"), "{rule}");
        assert!(rule.contains("floating enable"), "{rule}");
        // `ppt` IS NOT OPTIONAL. A floating container is sized in PIXELS
        // unless the unit says so, and a seventy-pixel window reads as the
        // monitor having failed to open rather than as a missing unit.
        assert!(rule.contains("ppt"), "the size has no unit:\n{rule}");
        // AND IT IS CENTRED, because a new floating window is not.
        assert!(rule.contains("move position center"), "{rule}");
        // One rule per window of the table, one per line.
        assert_eq!(rule.lines().count(), FLOATING.len(), "{rule}");
    }

    /// FAILS IF: the calculator's rule and its header's toggle disagree about
    /// its size — the toggle reads the table the rule is printed from.
    #[test]
    fn the_calculator_floats_at_the_size_its_rule_opens_it_at() {
        let portion = portion_of(crate::calc_window::APP_ID).expect("the calculator floats");
        let rule = rules();
        let line = rule
            .lines()
            .find(|l| l.contains(r"^br\.ronoctua\.calc$"))
            .expect("a rule for the calculator");
        assert!(
            line.contains(&format!(
                "resize set width {} ppt height {} ppt",
                portion.width_ppt, portion.height_ppt
            )),
            "{line}"
        );
        // Every size is a portion of the room, never more than all of it.
        for f in FLOATING {
            assert!(f.portion.width_ppt <= 100 && f.portion.height_ppt <= 100);
        }
        // And a window outside the table floats at nothing of its own.
        assert_eq!(portion_of(crate::terminal::APP_ID), None);
    }

    /// FAILS IF: the search over everything does not float at the whole
    /// room, or the plain search window floats with it — its rule is
    /// anchored on the whole `app_id`, and the float's id only extends it.
    #[test]
    fn the_search_floats_over_everything_and_its_window_tiles() {
        let whole = portion_of(crate::fast_access::APP_ID_FLOAT).expect("the float floats");
        assert_eq!((whole.width_ppt, whole.height_ppt), (100, 100));
        assert_eq!(portion_of(crate::fast_access::APP_ID), None);
        let rule = rules();
        assert!(
            rule.contains(r"^br\.ronoctua\.fast-access-float$"),
            "{rule}"
        );
        assert!(!rule.contains(r"^br\.ronoctua\.fast-access$"), "{rule}");
    }
}
