//! Finds the shim, or says the engine is not available.
//!
//! # Why the absence is not an error
//!
//! `RONOCTUA_TORRENT_SHIM` comes from `nix/rust.nix`. Outside that build — a
//! bare `cargo check` on someone's machine — it is absent, and the right answer
//! is to compile WITHOUT the engine rather than to fail: the queue still has
//! four other engines, and it already knows how to say that one of them is not
//! there.
//!
//! The `cfg` it emits is what makes that honest. Without it the functions would
//! be declared and their symbols missing, and the failure would land at link
//! time in whatever binary happened to pull the crate in. It is the same shape
//! `crates/ronoctua-term/build.rs` uses for `libghostty-vt`.

fn main() {
    println!("cargo::rerun-if-env-changed=RONOCTUA_TORRENT_SHIM");
    println!("cargo::rustc-check-cfg=cfg(torrent)");

    let Ok(prefix) = std::env::var("RONOCTUA_TORRENT_SHIM") else {
        return;
    };
    if prefix.is_empty() {
        return;
    }

    println!("cargo::rustc-link-search=native={prefix}/lib");
    // The shared library, and the name without the `lib` prefix or the suffix.
    println!("cargo::rustc-link-lib=dylib=ronoctua-torrent");
    println!("cargo::rustc-cfg=torrent");
}
