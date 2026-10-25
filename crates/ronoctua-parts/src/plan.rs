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
//!
//! A PART OF THE PERSON'S — a program they declared (`nix/modules/person.nix`)
//! — has no derivation in the plan: the system never evaluates what a person
//! declared, since one entry that aborts would stop the system's evaluation.
//! It names the entry instead ([`Entry`]), and its realizer evaluates that
//! entry alone, in its own process, from the machine's flake.

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
    /// Its derivation, a store path ending in `.drv`; `None` for a part of
    /// the person's, whose realizer evaluates its [`Wanted::entry`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drv: Option<String>,
    /// The output that becomes the profile's next generation; `None` with
    /// the derivation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<String>,
    /// The person's declared entry the part is made of; `None` for the
    /// distro's parts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<Entry>,
    /// What the session sees of it.
    pub kind: Kind,
    /// The shape the base speaks to it in; part of its path.
    pub interface: u32,
    /// Realized last and alone: a part whose build alone fills the machine.
    pub heavy: bool,
    /// Not free: never on the published ISO, never in a public cache.
    pub unfree: bool,
    /// What the page calls it: a program's own name, the same in every
    /// language (invariant 8's exception for a proper name).
    pub name: String,
}

/// The person's declared entry a part is made of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Entry {
    /// Its id in the person's declaration (`<source>:<source-id>`), which
    /// the machine flake's `personParts` names it by.
    pub id: String,
    /// What its declaration is, hashed: another one is another target.
    pub fingerprint: String,
}

impl Wanted {
    /// What a part of the distro's wants: its derivation and its output.
    /// `None` for a part of the person's, whose target its realizer finds.
    #[must_use]
    pub fn target(&self) -> Option<crate::record::Target> {
        Some(crate::record::Target {
            drv: self.drv.clone()?,
            out: self.out.clone()?,
            interface: self.interface,
        })
    }
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
            // A DISTRO'S PART names its derivation and output; a person's,
            // its entry — never both, never neither.
            match (&wanted.drv, &wanted.out, &wanted.entry) {
                (Some(drv), Some(out), None) => {
                    if !is_store_path(drv) || !drv.ends_with(".drv") {
                        return Err(Refusal::new(Why::Invalid, at("drv"), drv.clone()));
                    }
                    if !is_store_path(out) || out.ends_with(".drv") {
                        return Err(Refusal::new(Why::Invalid, at("out"), out.clone()));
                    }
                }
                (None, None, Some(entry)) => {
                    if !is_entry_id(&entry.id) {
                        return Err(Refusal::new(Why::Invalid, at("entry/id"), entry.id.clone()));
                    }
                    if !is_fingerprint(&entry.fingerprint) {
                        return Err(Refusal::new(
                            Why::Invalid,
                            at("entry/fingerprint"),
                            entry.fingerprint.clone(),
                        ));
                    }
                }
                _ => {
                    return Err(Refusal::new(
                        Why::Invalid,
                        format!("/parts/{id}"),
                        "neither a derivation and its output nor an entry",
                    ));
                }
            }
            if wanted.interface == 0 {
                return Err(Refusal::new(Why::Invalid, at("interface"), "0"));
            }
        }
        Ok(())
    }
}

/// Whether `id` may name a person's entry in the machine flake's
/// `personParts`: `<source>:<source-id>`, a source of lower-case letters and
/// an id of the characters the person's declarations take — never a quote,
/// a space or anything a shell or Nix's attribute path would read apart.
#[must_use]
pub fn is_entry_id(id: &str) -> bool {
    let Some((source, rest)) = id.split_once(':') else {
        return false;
    };
    !source.is_empty()
        && source.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
        && !rest.is_empty()
        && rest.len() <= 200
        && rest
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
}

/// Whether `text` is a fingerprint as `nix/modules/person.nix` writes one:
/// a sha256 in hex.
#[must_use]
pub fn is_fingerprint(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The plan at `path` ([`crate::names::PLAN`] on a machine), when it is
/// there and reads; what the page and the brain's watch name the parts by.
#[must_use]
pub fn read(path: &std::path::Path) -> Option<Plan> {
    ronoctua_person::parse(&std::fs::read(path).ok()?).ok()
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
            r#"{{"format":1,"parts":{{"{id}":{{"drv":"{drv}","out":"{out}","kind":"program","interface":1,"heavy":{heavy},"unfree":false,"name":"X"}}}}}}"#
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

        // A PART OF THE PERSON'S names its entry, and only that.
        let fp = "0".repeat(64);
        let person = format!(
            r#"{{"format":1,"parts":{{"person-hello":{{"entry":{{"id":"nix:hello","fingerprint":"{fp}"}},"kind":"program","interface":1,"heavy":false,"unfree":false,"name":"hello"}}}}}}"#
        );
        let taken: Plan = ronoctua_person::parse(person.as_bytes()).expect("a person's part");
        assert_eq!(taken.parts["person-hello"].target(), None);
        for bad in [
            person.replace("nix:hello", "nix:hel lo"),
            person.replace("nix:hello", "hello"),
            person.replace(&fp, "abc"),
            person.replace(
                r#""kind""#,
                &format!(r#""drv":"{DRV}","out":"{OUT}","kind""#),
            ),
            person.replace(
                r#"{"id":"nix:hello","fingerprint":"0000"#,
                r#"{"id":"nix:x\"y","fingerprint":"0000"#,
            ),
        ] {
            assert_eq!(
                ronoctua_person::parse::<Plan>(bad.as_bytes())
                    .map(|_| ())
                    .map_err(|r| r.why),
                Err(Why::Invalid),
                "{bad}"
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
                    drv: Some(DRV.to_owned()),
                    out: Some(OUT.to_owned()),
                    entry: None,
                    kind: Kind::Program,
                    interface: 1,
                    heavy,
                    unfree: false,
                    name: id.to_owned(),
                },
            );
        }
        let order: Vec<&str> = plan.order().iter().map(|(id, _)| *id).collect();
        assert_eq!(order, ["chrome", "yt-dlp", "android-studio"]);
    }
}
