//! What the parts' records say of a program — the person's declared ones
//! and the distro's AppImages and apks (`ronoctua_parts`): whether it is
//! here, and whether its last try could not fetch its download.
//!
//! A declared package, AppImage or Android app is a part of the person's
//! (`nix/modules/person.nix`): `person-applied.json` says which part each
//! applied entry became, and the part's record says how it stands. Root
//! writes the records; reading them is everyone's.

use std::path::PathBuf;

use ronoctua_parts::Absent;
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

/// The file `relative` of the part `id` — a declared apk — at the
/// interface the running system's plan wants, when the part is here: asked
/// on EVERY use, since a part arrives while the session runs.
///
/// # Errors
///
/// [`Absent`] when the plan does not want the part, or it is not here yet.
pub fn file(roots: &Roots, id: &str, relative: &str) -> Result<PathBuf, Absent> {
    ronoctua_parts::file_of_in(
        &roots.system.parts_plan,
        &roots.system.part_profiles,
        id,
        relative,
    )
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
