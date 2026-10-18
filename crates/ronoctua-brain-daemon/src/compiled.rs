//! The modules the daemon compiled: the distro's, from
//! `crates/ronoctua-brain/modules/<id>/`, and the person's, from the folder
//! the machine hands in.
//!
//! # The seam
//!
//! The generator (`nix/brain-modules-rs.nix`, `gen-brain-modules.py`, the
//! daemon's part) writes a `manifest.rs` per PERSON's module and the registry
//! `all.rs`, and `build.rs` a shim per module; `build.rs` puts them together
//! in `$OUT_DIR/modules.rs`, which is this file's body. A distro module's
//! shim re-exports the library's (`ronoctua_brain::compiled::<snake>`: its
//! manifest and its shared half) beside its own `module.rs`; a person's
//! includes its manifest from the registry. It defines what the rest of the
//! crate names: `IDS`, every id the build found, and `all`, one of each module
//! made for a birth — and, beside them, the manifests and the registry's own
//! test.
//!
//! `ronoctua-brain modules` prints what `all` makes, one id per line, and the
//! sealed check compares it with the folders Nix sees.

include!(concat!(env!("OUT_DIR"), "/modules.rs"));

#[cfg(test)]
mod the_catalogue {
    use ronoctua_brain::catalogue::{Declared, catalogue};

    /// FAILS IF: the catalogue the build wrote for the footer — data it reads
    /// at run time — is not what this daemon compiled: a module on one side
    /// only, in another order, or a mark, a notice, an action, an event or a
    /// reason the footer would draw and the daemon never declared. Of every
    /// module it made, the person's included, from the build's own file
    /// (`RONOCTUA_BRAIN_CATALOGUE`, in the tests' environment). Whose each
    /// module is, the library's test asks (the distro's are the ones it
    /// compiled).
    #[test]
    fn the_builds_catalogue_is_what_the_daemon_compiled() {
        let c = catalogue();
        let ids: Vec<&str> = c.modules.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            super::IDS,
            "the catalogue and the daemon hold other modules"
        );
        for m in super::all(&crate::Birth::detached()) {
            let manifest = m.manifest();
            let read = c.module(manifest.id).expect("a module of the catalogue");
            assert_eq!(
                read.declared(),
                Declared::of(manifest),
                "`{}` differs",
                manifest.id
            );
        }
    }
}
