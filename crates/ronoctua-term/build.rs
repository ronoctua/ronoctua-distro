//! Finds `libghostty-vt`, or says the engine is not available.
//!
//! # Why the absence is not an error
//!
//! `RONOCTUA_GHOSTTY_VT` comes from `nix/rust.nix`. Outside that build — a bare
//! `cargo check` on someone's machine — it is absent, and the right answer is to
//! compile WITHOUT the Ghostty engine rather than to fail: the crate still has
//! the alacritty one, and the differential already knows how to say it is
//! comparing a single engine against a golden file.
//!
//! The `cfg` it emits is what makes that honest. Without it the engine would be
//! declared and its symbols missing, and the failure would land at link time in
//! whatever binary happened to pull the crate in.

fn main() {
    println!("cargo::rerun-if-env-changed=RONOCTUA_GHOSTTY_VT");
    println!("cargo::rustc-check-cfg=cfg(ghostty_vt)");

    let Ok(prefix) = std::env::var("RONOCTUA_GHOSTTY_VT") else {
        return;
    };
    if prefix.is_empty() {
        return;
    }

    println!("cargo::rustc-link-search=native={prefix}/lib");
    // The SHARED library, not the static one: the static `.a` carries Zig's
    // runtime, and mixing it with Rust's is a class of problem this project has
    // no reason to take on for a test harness.
    println!("cargo::rustc-link-lib=dylib=ghostty-vt");
    println!("cargo::rustc-cfg=ghostty_vt");
}
