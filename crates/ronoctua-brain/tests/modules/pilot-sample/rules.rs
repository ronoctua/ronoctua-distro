//! The sample's arithmetic: a sibling of `module.rs`, declared there with
//! `mod rules;` and found beside it.

/// Where rustc found this file. The pilot test reads it: a `rules.rs` looked
/// for anywhere else would be a missing file, and this is the proof of where it
/// was found instead.
pub(super) const FILE: &str = file!();

/// Below this much free space, in per mille, the disk is nearly full.
///
/// A sample's constant. The real threshold is policy and lives in
/// `assets/cache.nix`, where the relief's floor already is.
const NEARLY_FULL_PER_MILLE: u16 = 100;

/// Whether a reading says the disk is nearly full. No reading is NOT full: a
/// mark about a disk nobody measured would be a mark that lies.
pub(super) fn nearly_full(free_per_mille: Option<u16>) -> bool {
    free_per_mille.is_some_and(|free| free < NEARLY_FULL_PER_MILLE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unmeasured_disk_is_not_nearly_full() {
        assert!(!nearly_full(None));
        assert!(nearly_full(Some(NEARLY_FULL_PER_MILLE - 1)));
        assert!(!nearly_full(Some(NEARLY_FULL_PER_MILLE)));
    }
}
