//! The distro the installer carries, and the machine's template in it.
//!
//! # Why nothing asks the release any more
//!
//! A machine follows the release: its flake's `ronoctua` input names the
//! `release` branch (`templates/machine/flake.nix`), and every update moves
//! it there. The installer used to ask that branch which revision it was at
//! and start the release's installer when it was newer. An installation
//! never uses the network now (the owner's plan): the ISO carries every
//! path it needs, so the machine is ALWAYS installed from the ISO's own tree
//! and keeps it: its flake aimed at that tree, the flake and its lock
//! agreeing (`crate::plan::Origin`).
//!
//! What is left here are the names of that tree's parts — where the ISO
//! keeps it, where the template is — and [`aimed`], which points the
//! template's distro input at the tree. Who asks `nix` which reference the
//! template names is `crate::reach` — the distro's flake reference is the
//! template's, never spelled here (invariant 16).

/// The variable naming where the ISO keeps the distro's tree it was built
/// from (`nix/installer/iso.nix`).
pub const FLAKE_DIR: &str = "RONOCTUA_FLAKE_DIR";

/// Where the machine's template is, under a distro's tree.
pub const TEMPLATE: &str = "templates/machine";

/// The template's flake file, under [`TEMPLATE`].
pub const TEMPLATE_FLAKE: &str = "flake.nix";

/// The flake attribute a machine's template reads its distro from — the
/// input the update moves.
pub const DISTRO_INPUT: &str = "ronoctua";

/// The template's flake with its distro input aimed at `tree` instead of
/// the release `release_url` names: the one string literal that is exactly
/// the release's reference, replaced. `None` when the template does not
/// hold that literal exactly once — the template changed under this
/// installer, and a guess would install the wrong thing. Pure.
#[must_use]
pub fn aimed(template: &str, release_url: &str, tree: &str) -> Option<String> {
    let literal = format!("\"{release_url}\"");
    if release_url.is_empty() || template.matches(&literal).count() != 1 {
        return None;
    }
    Some(template.replacen(&literal, &format!("\"path:{tree}\""), 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: aiming touches anything but the release's one literal, or
    /// aims a template that holds it twice or not at all. (The template the
    /// distro ships is not in the Rust build's source; a template that stops
    /// holding the literal once makes the AIM step refuse, before the disk
    /// is touched.)
    #[test]
    fn a_flake_is_aimed_at_the_installers_tree_by_its_one_literal() {
        let url = "github:someone/distro/release";
        // The template's shape: a comment that names the branch in prose,
        // the input, the outputs.
        let template = "{\n  # follows the `release` branch\n  inputs.ronoctua.url = \"github:someone/distro/release\";\n  outputs = { ronoctua, ... }: ronoctua.lib.machineFlake { dir = ./.; };\n}\n";
        let aimed_text = aimed(template, url, "/nix/store/zzzz-source").expect("aimed");
        assert!(aimed_text.contains("\"path:/nix/store/zzzz-source\""));
        assert!(!aimed_text.contains(&format!("\"{url}\"")));
        // Everything else byte for byte.
        assert_eq!(
            aimed_text.replace("\"path:/nix/store/zzzz-source\"", &format!("\"{url}\"")),
            template
        );
        assert_eq!(aimed("x = \"u\"; y = \"u\";", "u", "/t"), None, "twice");
        assert_eq!(aimed("x = \"v\";", "u", "/t"), None, "absent");
        assert_eq!(aimed("x = \"\";", "", "/t"), None, "nothing to aim");
    }
}
