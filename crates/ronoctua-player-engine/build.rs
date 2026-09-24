//! Where libmpv is.
//!
//! The path comes from the environment (`RONOCTUA_LIBMPV_DIR`, set by
//! `nix/rust.nix` and the devshell) and goes into the binary's RUNPATH, so
//! the daemon finds the SAME libmpv it was linked against and never a
//! system one.

fn main() {
    println!("cargo::rerun-if-env-changed=RONOCTUA_LIBMPV_DIR");
    if let Ok(dir) = std::env::var("RONOCTUA_LIBMPV_DIR") {
        println!("cargo::rustc-link-search=native={dir}");
        // EVERY artefact, the tests included: a test binary that cannot find
        // libmpv at run time fails before it tests anything.
        println!("cargo::rustc-link-arg=-Wl,-rpath,{dir}");
    }
}
