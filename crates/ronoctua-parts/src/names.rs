//! The names the parts answer to: where their profiles, records and roots
//! live, and the units that realize them. `nix/modules/parts.nix` declares the
//! same, and `parts-rule-is-narrow` holds the two together.

use std::path::PathBuf;

/// The root of the parts' profiles. A part is a profile under it, never a
/// path inside the system.
pub const PARTS_ROOT: &str = "/nix/var/nix/profiles/ronoctua/parts";

/// Root's state: the records and the session view. Persisted.
pub const STATE_DIR: &str = "/var/lib/ronoctua-parts";

/// The folder of the records, one `<id>.json` each.
pub const RECORDS: &str = "/var/lib/ronoctua-parts/parts";

/// The session's view of the parts that are here ([`crate::view`]).
pub const VIEW: &str = "/var/lib/ronoctua-parts/view";

/// Root's lock of the records: the planner and a realizer never interleave
/// a write of the same record.
pub const LOCK: &str = "/var/lib/ronoctua-parts/records.lock";

/// The roots of the plan's derivations, so a garbage collection between a
/// switch and a part's realization never takes what is still to be built.
pub const PLAN_ROOTS: &str = "/nix/var/nix/gcroots/ronoctua-parts/plan";

/// The plan of the running system, written by Nix (`nix/modules/parts.nix`).
pub const PLAN: &str = "/etc/ronoctua/parts-plan.json";

/// The unit that plans: reads [`PLAN`], writes the records, starts the
/// realizers. Started at every boot and switch, on a network coming up, and
/// by the person.
pub const PLANNER_UNIT: &str = "ronoctua-parts.service";

/// The unit that realizes one part, a template whose instance is the part's
/// id ([`crate::id`]).
#[must_use]
pub fn realizer_unit(id: &str) -> String {
    format!("ronoctua-part@{id}.service")
}

/// A part's profile at its interface: the ONE path the system names it by.
#[must_use]
pub fn profile(id: &str, interface: u32) -> PathBuf {
    PathBuf::from(format!("{PARTS_ROOT}/{id}/v{interface}"))
}

/// A part's record.
#[must_use]
pub fn record(id: &str) -> PathBuf {
    PathBuf::from(format!("{RECORDS}/{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the path a part is reached by drifts from what
    /// `nix/modules/parts.nix` writes into the system — every reader of a
    /// part would ask after a path nothing ever creates.
    #[test]
    fn a_part_is_reached_at_its_interface() {
        assert_eq!(
            profile("pot-provider", 1),
            PathBuf::from("/nix/var/nix/profiles/ronoctua/parts/pot-provider/v1")
        );
        assert_eq!(realizer_unit("yt-dlp"), "ronoctua-part@yt-dlp.service");
        assert_eq!(
            record("yt-dlp"),
            PathBuf::from("/var/lib/ronoctua-parts/parts/yt-dlp.json")
        );
    }
}
