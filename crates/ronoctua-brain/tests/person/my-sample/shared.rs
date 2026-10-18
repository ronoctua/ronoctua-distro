//! A FILE BESIDE THE PERSON'S SAMPLE, named as a distro module's shared half
//! is, on purpose. In a person's folder `shared.rs` is a file like any other:
//! its `module.rs` declares it (`mod shared;`), and the daemon compiles it
//! with the rest of the module. Only a DISTRO module's `shared.rs` is compiled
//! into the library the footer links, which never reads a person's folder —
//! `brain-library-links-no-module` poisons this file to prove it, and
//! `brain-shared-half-is-the-distros` that the folder loads.

/// The person's word for the third seat: the label its mark is drawn with.
pub const DESK: &str = "desk";
