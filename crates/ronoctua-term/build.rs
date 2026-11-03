//! Finds `libghostty-vt`, or refuses to build.
//!
//! # Why the absence is an error now
//!
//! It used to be a degraded mode: without `RONOCTUA_GHOSTTY_VT` the crate
//! compiled with the alacritty engine alone, and the differential said it was
//! comparing one engine against a golden file. That was right while Ghostty was
//! the second opinion. It is the PRODUCT engine now — the terminal a holder
//! keeps, the mirror a window draws — and a build that quietly left it out
//! would ship a terminal with no engine, found only when a window opened.
//!
//! `RONOCTUA_GHOSTTY_VT` comes from `nix/rust.nix`, and the devshell exports
//! it; a bare `cargo` outside both is told where to get it.

fn main() {
    println!("cargo::rerun-if-env-changed=RONOCTUA_GHOSTTY_VT");

    let prefix = std::env::var("RONOCTUA_GHOSTTY_VT").unwrap_or_default();
    assert!(
        !prefix.is_empty(),
        "RONOCTUA_GHOSTTY_VT is not set: ronoctua-term links libghostty-vt, the terminal's \
         engine. Build inside `nix develop` (which exports it) or through the flake."
    );

    // THE RELEASE, off the derivation's name (`…-libghostty-vt-1.4.0`), for
    // XTVERSION: the library itself only knows its ABI's own number.
    let release = std::path::Path::new(&prefix)
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.rsplit_once("libghostty-vt-"))
        .map(|(_, v)| v.to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            panic!("RONOCTUA_GHOSTTY_VT is {prefix}, not a `libghostty-vt-<version>` store path")
        });
    println!("cargo::rustc-env=GHOSTTY_VT_RELEASE={release}");
    // THE BUILD, by its store path's name (`<hash>-libghostty-vt-1.4.0`): a
    // window attaching to a holder names the library it decodes snapshots
    // with, and a holder of another build — one left running across a
    // switch — answers in the portable form instead (`holder.rs`). The hash
    // is the point: two builds of one release are not promised one format.
    let engine = std::path::Path::new(&prefix)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    println!("cargo::rustc-env=GHOSTTY_VT_ENGINE={engine}");

    println!("cargo::rustc-link-search=native={prefix}/lib");
    // The SHARED library, not the static one: the static `.a` carries Zig's
    // runtime, and mixing it with Rust's is a class of problem this project has
    // no reason to take on — and `nix/ghostty-vt.nix` deletes it anyway.
    println!("cargo::rustc-link-lib=dylib=ghostty-vt");
}
