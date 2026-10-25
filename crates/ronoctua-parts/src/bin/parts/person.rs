//! A part of the person's: the entry they declared, evaluated alone.
//!
//! The plan names the entry and its fingerprint, never a derivation
//! (`ronoctua_parts::plan::Entry`): the system never evaluates what the
//! person declared. Its realizer asks the machine flake for that one entry
//! (`personParts."<id>"`, `nix/modules/person.nix`) in this process, and an
//! entry that aborts, or names a package nixpkgs dropped, fails this part
//! alone. The answer is the entry's own build: its derivation and output, or
//! why its build refuses it — a licence the person did not accept, a package
//! marked broken.

use ronoctua_parts::plan::{Entry, is_store_path};
use ronoctua_parts::record::Target;

use crate::disk;

/// The machine directory, baked by the build (`nix/machine.nix`'s one name).
const MACHINE_DIR: Option<&str> = option_env!("RONOCTUA_MACHINE_DIR");

/// `entry` evaluated by `nix` into the part's target at `interface` — or
/// what was said instead, for the record: Nix's own last words, or the
/// build's refusal.
pub fn evaluate(nix: &str, entry: &Entry, interface: u32) -> Result<Target, String> {
    let Some(dir) = MACHINE_DIR else {
        return Err("this build does not know the machine directory".to_owned());
    };
    let said = disk::run(&argv(nix, dir, &entry.id));
    if !said.ok {
        return Err(last_words(&said.err));
    }
    target_of(&said.out, interface)
}

/// `nix eval --json` of the one entry, its lock never moved.
fn argv(nix: &str, dir: &str, id: &str) -> Vec<String> {
    vec![
        nix.to_owned(),
        "eval".to_owned(),
        "--extra-experimental-features".to_owned(),
        "nix-command flakes".to_owned(),
        "--no-update-lock-file".to_owned(),
        "--json".to_owned(),
        format!("path:{dir}#personParts.\"{id}\""),
    ]
}

/// The target `json` names — `{"drv", "out", "fingerprint"}` — or the
/// refusal it says (`{"refused", "detail"}`). Pure.
fn target_of(json: &str, interface: u32) -> Result<Target, String> {
    let said: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "the entry answered nothing readable".to_owned())?;
    let text = |k: &str| said.get(k).and_then(serde_json::Value::as_str);
    if let Some(refused) = text("refused") {
        return Err(format!("{refused}: {}", text("detail").unwrap_or_default()));
    }
    match (text("drv"), text("out")) {
        (Some(drv), Some(out))
            if is_store_path(drv) && drv.ends_with(".drv") && is_store_path(out) =>
        {
            Ok(Target {
                drv: drv.to_owned(),
                out: out.to_owned(),
                interface,
            })
        }
        _ => Err("the entry named no derivation and output".to_owned()),
    }
}

/// The last line of what Nix said that says something.
fn last_words(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty() && !l.starts_with("warning:"))
        .unwrap_or("nix said nothing")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRV: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-hello-2.12.drv";
    const OUT: &str = "/nix/store/zyxwvsrqpnmlkjihgfdcba9876543210-hello-2.12";

    /// FAILS IF: an entry's answer is taken for a target when it names no
    /// store path, or its build's refusal is not said as one.
    #[test]
    fn an_entry_answers_a_target_or_why_not() {
        let fp = "a".repeat(64);
        let ok = format!(r#"{{"drv":"{DRV}","out":"{OUT}","fingerprint":"{fp}"}}"#);
        assert_eq!(
            target_of(&ok, 1),
            Ok(Target {
                drv: DRV.to_owned(),
                out: OUT.to_owned(),
                interface: 1
            })
        );
        let refused = r#"{"refused":"refused-by-meta","detail":"`discord` is unfree, and its declaration carries no consent","fingerprint":"x"}"#;
        assert_eq!(
            target_of(refused, 1),
            Err(
                "refused-by-meta: `discord` is unfree, and its declaration carries no consent"
                    .to_owned()
            )
        );
        for bad in [
            format!(r#"{{"drv":"/tmp/x.drv","out":"{OUT}"}}"#),
            format!(r#"{{"drv":"{OUT}","out":"{OUT}"}}"#),
            "[]".to_owned(),
            "nonsense".to_owned(),
        ] {
            assert!(target_of(&bad, 1).is_err(), "{bad}");
        }
        assert_eq!(
            last_words(
                "warning: Git tree is dirty\nerror: attribute 'nope' missing\n\nwarning: x\n"
            ),
            "error: attribute 'nope' missing"
        );
        let words = argv("/n/nix", "/etc/nixos", "nix:python3Packages.requests");
        assert_eq!(
            words.last().map(String::as_str),
            Some("path:/etc/nixos#personParts.\"nix:python3Packages.requests\"")
        );
    }
}
