//! The DISTRO's modules, as data: what the build compiled from
//! `crates/ronoctua-brain/modules/<id>/`.
//!
//! # The seam
//!
//! The generator (`nix/brain-modules-rs.nix`, `gen-brain-modules.py`, the
//! library's part) writes a `manifest.rs` per module and the views
//! `distro.rs`, and `build.rs` a shim per module — its manifest, and its
//! `shared.rs` when its folder carries one; `build.rs` puts them together in
//! `$OUT_DIR/manifests.rs`, which is this file's body. It defines what the
//! rest of the system names: `<snake>::…` for each module (its `ID`, its
//! `MANIFEST`, its keys, units and declarations, and its shared half), `IDS`,
//! `MANIFESTS` and `manifest(id)` — and, beside them, the views' own test.
//!
//! NO LIST OF ACTIONS OR EVENTS. What the footer registers and recognises of
//! a module — its actions, its events, its marks' glyphs, its notifications —
//! it reads at run time from the brain's catalogue ([`crate::catalogue`]),
//! which holds the person's modules too: a list compiled here would be the
//! distro's alone, and a person's module would have to rebuild the footer to
//! join it.
//!
//! What it does NOT define is `all`: the modules are MADE by the daemon
//! (`ronoctua_brain_daemon::compiled::all`), which compiles their code and the
//! person's modules too. This is the distro's alone, so the footer that reads
//! it is the same on every machine.

include!(concat!(env!("OUT_DIR"), "/manifests.rs"));
