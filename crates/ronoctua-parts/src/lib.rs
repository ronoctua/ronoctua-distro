//! What reaches a machine after it is installed, part by part.
//!
//! # Why parts
//!
//! A NixOS system is one derivation, built whole or not at all: one download
//! that failed — the npm cache of the PO-token minter, on 2026-10-23 — stopped
//! an installation and would stop every update after it. So whatever needs a
//! download no public cache holds, whatever is not free, and whatever only the
//! full profile carries is a PART: declared in the flake
//! (`ronoctua.parts.<id>`, `nix/modules/parts.nix`), never in the system's
//! closure, realized on its own and kept in a Nix profile of its own.
//!
//! ```text
//! /etc/ronoctua/parts-plan.json                  the wanted parts of THIS system ([`plan`]), written by Nix
//! /nix/var/nix/profiles/ronoctua/parts/<id>/     one profile per part, `v<interface>` its generations
//! /var/lib/ronoctua-parts/parts/<id>.json        what became of each ([`record`]), root's, 0644
//! /var/lib/ronoctua-parts/view/                  the session's view: bin/, share/applications… ([`view`])
//! /nix/var/nix/gcroots/ronoctua-parts/plan/      the plan's derivations, rooted until realized
//! ```
//!
//! # The rules that make a part a part
//!
//! - The system reaches a part only by its stable path ([`names::profile`]),
//!   and every reader asks [`here`] first: a part may be arriving, waiting for
//!   a network, or kept back — and the screen says so.
//! - One part never waits on another: each is realized by its own process
//!   (`ronoctua-part@<id>.service`), whose failure is its own.
//! - A part that fails keeps its previous generation, working. The interface
//!   is in the path, so a system never meets a part's shape it does not
//!   speak, and a rollback finds the old one.
//!
//! # Pure
//!
//! Serde and the file contract of `ronoctua-person`. A few functions ask the
//! disk anything: [`here`], whether a path exists, and the readers the page
//! and the brain call — [`record::read_all`], [`plan::read`] and
//! [`stick::read`].
//! Root's realizer (`ronoctua-parts`, feature `realize`) opens, runs `nix` and
//! writes; what it decides is here, tested.

#![forbid(unsafe_code)]

pub mod argv;
pub mod exit;
pub mod id;
pub mod names;
pub mod plan;
pub mod record;
pub mod stick;
pub mod view;

mod here;

pub use here::{Absent, file_of, file_of_in, here};
