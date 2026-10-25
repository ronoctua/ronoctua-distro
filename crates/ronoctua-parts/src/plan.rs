//! The plan: the parts THIS system wants, as Nix wrote them.
//!
//! `nix/modules/parts.nix` writes it into the system
//! (`/etc/ronoctua/parts-plan.json`) with every store path's context
//! DISCARDED: the system names a part's derivation and output as text and
//! never depends on them, so building the system builds no part. Evaluating
//! the system instantiates the derivations, which is why a part's `.drv` is in
//! the store after a switch without anybody evaluating again; the planner
//! roots them at once ([`crate::names::PLAN_ROOTS`]).
//!
//! The plan is the running system's, so the parts can never run ahead of the
//! base: a switch brings the next plan, and nothing else does.

use std::collections::BTreeMap;

use ronoctua_person::{Document, FORMAT, Refusal, Why};
use serde::{Deserialize, Serialize};

/// The whole plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Plan {
    /// [`FORMAT`].
    pub format: u32,
    /// Every wanted part, by id. A part the system does not want is not here.
    pub parts: BTreeMap<String, Wanted>,
}

/// One wanted part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Wanted {
    /// Its derivation, a store path ending in `.drv`.
    pub drv: String,
    /// The output that becomes the profile's next generation.
    pub out: String,
    /// What the session sees of it.
    pub kind: Kind,
    /// The shape the base speaks to it in; part of its path.
    pub interface: u32,
    /// Realized last and alone: a part whose build alone fills the machine.
    pub heavy: bool,
    /// Not free: never on the published ISO, never in a public cache.
    pub unfree: bool,
}

/// What the session sees of a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Programs and their launchers: `bin/` and `share/…` in the session's
    /// view ([`crate::view`]).
    Program,
    /// Data a program reads by the part's path; nothing in the view.
    Data,
}

impl Plan {
    /// The parts in the order they are realized: the light ones first, then
    /// the heavy ones, each group by id — so a part that fills the machine
    /// never starves the small ones queued behind it.
    #[must_use]
    pub fn order(&self) -> Vec<(&str, &Wanted)> {
        let mut parts: Vec<(&str, &Wanted)> =
            self.parts.iter().map(|(id, w)| (id.as_str(), w)).collect();
        parts.sort_by_key(|(id, w)| (w.heavy, *id));
        parts
    }
}

/// Whether `path` is a store path of the form Nix writes:
/// `/nix/store/<32 base-32 characters>-<name>`.
#[must_use]
pub fn is_store_path(path: &str) -> bool {
    const BASE32: &[u8] = b"0123456789abcdfghijklmnpqrsvwxyz";
    let Some(rest) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    let bytes = rest.as_bytes();
    bytes.len() > 33
        && bytes[..32].iter().all(|b| BASE32.contains(b))
        && bytes[32] == b'-'
        && !rest.contains('/')
}

impl Document for Plan {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        for (id, wanted) in &self.parts {
            let at = |field: &str| format!("/parts/{id}/{field}");
            if !crate::id::is_id(id) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("/parts/{id}"),
                    "not a part id",
                ));
            }
            if !is_store_path(&wanted.drv) || !wanted.drv.ends_with(".drv") {
                return Err(Refusal::new(Why::Invalid, at("drv"), wanted.drv.clone()));
            }
            if !is_store_path(&wanted.out) || wanted.out.ends_with(".drv") {
                return Err(Refusal::new(Why::Invalid, at("out"), wanted.out.clone()));
            }
            if wanted.interface == 0 {
                return Err(Refusal::new(Why::Invalid, at("interface"), "0"));
            }
        }
        Ok(())
    }
}

/// An empty plan of the current format: a system that wants no part.
#[must_use]
pub fn empty() -> Plan {
    Plan {
        format: FORMAT,
        parts: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const DRV: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-yt-dlp.drv";
    pub(crate) const OUT: &str = "/nix/store/zyxwvsrqpnmlkjihgfdcba9876543210-yt-dlp";

    fn plan_with(id: &str, drv: &str, out: &str, heavy: bool) -> String {
        format!(
            r#"{{"format":1,"parts":{{"{id}":{{"drv":"{drv}","out":"{out}","kind":"program","interface":1,"heavy":{heavy},"unfree":false}}}}}}"#
        )
    }

    /// FAILS IF: a plan that names a part by a bad id or a path that is not
    /// a store path is taken — the realizer would hand it to `nix` and to a
    /// unit instance as it is.
    #[test]
    fn a_plan_names_store_paths_and_ids() {
        let good: Plan = ronoctua_person::parse(plan_with("yt-dlp", DRV, OUT, false).as_bytes())
            .expect("a good plan");
        assert_eq!(good.parts["yt-dlp"].kind, Kind::Program);

        for (id, drv, out) in [
            ("Yt", DRV, OUT),
            ("yt-dlp", "/tmp/x.drv", OUT),
            ("yt-dlp", OUT, OUT),
            ("yt-dlp", DRV, DRV),
            ("yt-dlp", DRV, "/nix/store/short-x"),
        ] {
            let refused = ronoctua_person::parse::<Plan>(plan_with(id, drv, out, false).as_bytes());
            assert_eq!(
                refused.map(|_| ()).map_err(|r| r.why),
                Err(Why::Invalid),
                "{id} {drv} {out}"
            );
        }

        let newer = r#"{"format":2,"parts":{}}"#;
        assert_eq!(
            ronoctua_person::parse::<Plan>(newer.as_bytes()).map_err(|r| r.why),
            Err(Why::Newer)
        );
    }

    /// FAILS IF: a heavy part is realized before a light one — a gigabyte of
    /// Android Studio would hold back a forty-kilobyte script for hours.
    #[test]
    fn the_heavy_parts_come_last() {
        let mut plan = empty();
        for (id, heavy) in [
            ("android-studio", true),
            ("yt-dlp", false),
            ("chrome", false),
        ] {
            plan.parts.insert(
                id.to_owned(),
                Wanted {
                    drv: DRV.to_owned(),
                    out: OUT.to_owned(),
                    kind: Kind::Program,
                    interface: 1,
                    heavy,
                    unfree: false,
                },
            );
        }
        let order: Vec<&str> = plan.order().iter().map(|(id, _)| *id).collect();
        assert_eq!(order, ["chrome", "yt-dlp", "android-studio"]);
    }
}
