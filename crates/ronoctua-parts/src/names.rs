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

/// The downloads of each part that is here, rooted so a wrapper's change
/// never fetches them again and a stick can carry them (`export`):
/// `<KEPT>/<id>/<name>`.
pub const KEPT: &str = "/nix/var/nix/gcroots/ronoctua-parts/kept";

/// Root's folder of what the person hands over: `0755`, root's.
pub const RUN: &str = "/run/ronoctua-parts";

/// Where the person hands a file in: `<HANDIN>/<part>/<download's name>`.
/// The person's, `0700`, inside root's [`RUN`], so it can be filled but never
/// swapped for a link.
pub const HANDIN: &str = "/run/ronoctua-parts/handin";

/// Where root copies a handed file before Nix reads it: root's, `0700`.
pub const ACCEPTING: &str = "/run/ronoctua-parts/accepting";

/// The file the person writes the stick's folder into, for an import.
pub const IMPORT_FROM: &str = "/run/ronoctua-parts/import/from";

/// The folder on a stick that holds the downloads (`export`, `import`): a
/// binary cache Nix reads by `file://`.
pub const STICK_FOLDER: &str = "ronoctua-parts";

/// Where the person says yes to ONE try of a part on a metered link: an empty
/// `yes/<id>`, which the realizer takes away as the try begins. A folder only
/// they and root reach (`0700`, theirs).
pub const YES: &str = "/run/ronoctua-parts/yes";

/// Where the person names the folder `export` writes to — one line, the
/// folder's path —, in a folder only they and root reach (`0700`, theirs).
pub const EXPORT_TO: &str = "/run/ronoctua-parts/export/to";

/// What the last import or export did ([`crate::stick::Said`]), root's,
/// `0644`: what the page says after the person's gesture.
pub const STICK_SAID: &str = "/var/lib/ronoctua-parts/stick.json";

/// The unit that takes what was handed in.
pub const ACCEPT_UNIT: &str = "ronoctua-parts-accept.service";

/// The unit that imports from a stick.
pub const IMPORT_UNIT: &str = "ronoctua-parts-import.service";

/// The unit that writes this machine's kept downloads on a stick.
pub const EXPORT_UNIT: &str = "ronoctua-parts-export.service";

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
