//! Copies the generated command table into `OUT_DIR`.
//!
//! The table is built by Nix (`nix/compositor-deeds.nix`) because that is
//! where the compositor's source is, and the generator checks every verb
//! against it. `RONOCTUA_DEEDS` is how the path arrives.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=RONOCTUA_DEEDS");

    let Some(generated) = std::env::var_os("RONOCTUA_DEEDS") else {
        panic!(
            "RONOCTUA_DEEDS is not set.\n\
             \n\
             It names the generated command table, which comes out of\n\
             `assets/compositor-deeds.nix` by way of `nix/compositor-deeds.nix`.\n\
             Building this crate outside the flake's environment cannot work,\n\
             because the generator also checks every command against the\n\
             compositor's own source.\n\
             \n\
             Use `nix develop` (or `direnv allow`), or build through `nix build`."
        );
    };
    let generated = PathBuf::from(generated);
    println!("cargo:rerun-if-changed={}", generated.display());

    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let landing = out.join("deeds.rs");
    // REMOVED FIRST, because a store file is read-only and `fs::copy` copies
    // the mode along with the bytes — so the second build of the same tree
    // fails on a file it cannot open for writing.
    let _ = std::fs::remove_file(&landing);
    std::fs::copy(&generated, &landing).expect("copying the command table");
}
