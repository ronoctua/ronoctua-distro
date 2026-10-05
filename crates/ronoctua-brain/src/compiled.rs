//! The modules the build compiled from `crates/ronoctua-brain/modules/<id>/`.
//!
//! # The seam
//!
//! The generator (`nix/brain-modules-rs.nix`, `gen-brain-modules.py`) writes a
//! `manifest.rs` per module and the registry `all.rs`, and `build.rs` a shim
//! per module; `build.rs` puts them together in `$OUT_DIR/modules.rs`, which is
//! this file's body. It defines what the rest of the crate names: `IDS`, every
//! id the build found, and `all`, one of each module made for a birth — and,
//! beside them, the manifests and the registry's own test.
//!
//! `ronoctua-brain modules` prints what `all` makes, one id per line, and the
//! sealed check compares it with the folders Nix sees.

include!(concat!(env!("OUT_DIR"), "/modules.rs"));
