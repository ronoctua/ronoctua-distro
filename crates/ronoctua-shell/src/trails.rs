//! The compositor's trails, as numbers on the modeline.
//!
//! A trail is a list of windows somebody marked to walk between, on any
//! workspace; there may be several, and one is in use. The owner asked to see
//! them: the modeline draws a glyph with how many windows the one in use holds,
//! and — from two trails on, as the workspaces from two — one number per trail,
//! the one in use brighter (`workspaces::colour_of`, the same kind of thing).
//! A click on a number puts that trail in use; on the glyph, it opens the
//! window menu at the trails.
//!
//! PEOPLE COUNT FROM ONE and the compositor from ZERO (`trail number 0` is the
//! first), so the conversion lives here and nowhere else.

/// The nine marks the modeline can carry, one per trail — nine, like the
/// workspaces, and `&'static str` because a segment's id is one.
pub const POOL: [&str; 9] = [
    "trail-1", "trail-2", "trail-3", "trail-4", "trail-5", "trail-6", "trail-7", "trail-8",
    "trail-9",
];

/// The segment id of trail `number`, counted from one.
#[must_use]
pub fn id_of(number: u32) -> Option<&'static str> {
    POOL.get(usize::try_from(number).ok()?.checked_sub(1)?)
        .copied()
}

/// The trail a segment id names, counted from one.
#[must_use]
pub fn number_of(id: &str) -> Option<u32> {
    let at = POOL.iter().position(|p| *p == id)?;
    u32::try_from(at + 1).ok()
}

/// What the compositor calls trail `number` (counted from one): its index.
#[must_use]
pub fn index_of(number: u32) -> u32 {
    number.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: an id and a number stop being each other's inverse, or the
    /// first trail a person sees is not the compositor's zeroth.
    #[test]
    fn a_trail_is_named_from_one_and_asked_from_zero() {
        for n in 1..=9 {
            assert_eq!(number_of(id_of(n).unwrap()), Some(n));
        }
        assert_eq!(id_of(0), None);
        assert_eq!(id_of(10), None);
        assert_eq!(number_of("ws-1"), None);
        assert_eq!(index_of(1), 0);
    }
}
