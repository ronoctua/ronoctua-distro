//! What the parts' records say of a program — the person's declared ones
//! and the distro's AppImages (`ronoctua_parts`): whether it is here, and
//! whether its last try could not fetch its download.
//!
//! A declared package or AppImage is a part of the person's
//! (`nix/modules/person.nix`): `person-applied.json` says which part each
//! applied entry became, and the part's record says how it stands. Root
//! writes the records; reading them is everyone's.

use ronoctua_parts::record::{Phase, Record};
use ronoctua_person::reports::PersonApplied;

use crate::Roots;

/// The part `id`'s record, when there is one and it reads.
fn record(roots: &Roots, id: &str) -> Option<Record> {
    let bytes = std::fs::read(roots.system.parts.join(format!("{id}.json"))).ok()?;
    ronoctua_person::parse::<Record>(&bytes).ok()
}

/// The part an applied entry became, as the report says.
#[must_use]
pub fn of_entry<'a>(applied: Option<&'a PersonApplied>, entry: &str) -> Option<&'a str> {
    applied?.parts.get(entry).map(String::as_str)
}

/// Whether the part `id` is here: what it wants is its profile's.
#[must_use]
pub fn here(roots: &Roots, id: &str) -> bool {
    record(roots, id).is_some_and(|r| r.phase == Phase::Here)
}

/// Whether the part `id`'s last try could not fetch the download at `url`
/// — gone from that address, or changed under it: a pin that stayed there
/// would never build again.
#[must_use]
pub fn misses(roots: &Roots, id: &str, url: &str) -> bool {
    record(roots, id).is_some_and(|r| r.missing.iter().any(|d| d.urls.iter().any(|u| u == url)))
}

/// What the part `id`'s last try said, when it failed.
#[must_use]
pub fn failure(roots: &Roots, id: &str) -> Option<String> {
    record(roots, id)?.last_failure.map(|f| f.detail)
}
