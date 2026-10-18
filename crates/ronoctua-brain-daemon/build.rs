//! Loads every brain module's code — the distro's and the person's — into the
//! daemon.
//!
//! This build script does NOT generate anything — it only locates the
//! finished files. Generating here would be reimplementing Nix logic
//! (invariant 5) and would leave the source of truth. A module's own tables
//! (the update's sequence, the disk's cache policy) arrive beside its
//! manifest (`tables.nix`).
//!
//! The same shape as `ronoctua-notify`'s build script, down to the reason the
//! destination is deleted first.
//!
//! # The modules (brain.md §3)
//!
//! A module is a folder, `crates/ronoctua-brain/modules/<id>/`, beside the
//! library that compiles its manifest (and its shared half, when it has one).
//! What THIS crate compiles is its code, `module.rs`. Nix generates the
//! registry of all of them — `RONOCTUA_BRAIN_REGISTRY_DIR/all.rs` and
//! `ids.txt`, the daemon's part (`nix/brain-modules-rs.nix`). What this script
//! adds is the one thing Nix cannot know: where the folders are on disk. It
//! writes `modules.rs` — one shim per module: a distro module's re-exports
//! the library's (`ronoctua_brain::compiled::<snake>`) and loads its
//! `module.rs` by an absolute `#[path]`; then the registry — which
//! `src/compiled.rs` includes, and before that it COMPARES the folders it sees
//! with the ids Nix listed. A folder added or removed since the environment
//! was entered is a list the compiler would otherwise build wrong in silence
//! — a module missing from the daemon, or a shim pointing at a folder that is
//! gone.
//!
//! It also compares the shared halves on disk with the library's table
//! (`RONOCTUA_BRAIN_MODULES_DIR/shared.txt`): the library's script watches
//! nothing in the folders (so a `module.rs` edit never recompiles the footer),
//! and this one, which watches them all, is where a `shared.rs` added without
//! re-entering the environment is refused with what fixes it.
//!
//! # The person's modules
//!
//! A machine may also compile the PERSON's modules (`my-…`), which live
//! outside this repository: `ids.txt` says whose each module is, and a
//! person's manifest is the registry's own and its `module.rs` is loaded from
//! `RONOCTUA_BRAIN_PERSON_MODULES_DIR`, compared with the table the same way.
//! They reach no other crate: the library, and so the footer, is the same
//! with or without them.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let library = library_dir();
    // THE MODULES, one shim each, after the folders and the table agree.
    modules(&out, &library);
    // AND THE `#[path]` PILOT, the shape every module is loaded in, on the
    // sample that is never registered.
    pilot(&out, &library);
}

/// The variable `nix/rust.nix` sets to the daemon's part: the registry.
const REGISTRY: &str = "RONOCTUA_BRAIN_REGISTRY_DIR";

/// The variable `nix/rust.nix` sets to the library's part, read here only for
/// which distro modules carry a shared half (`shared.txt`).
const MODULES: &str = "RONOCTUA_BRAIN_MODULES_DIR";

/// The variable `nix/rust.nix` sets to the PERSON's modules (`my-…`), when
/// the machine has any: a copy in the store of the folders the person wrote
/// (`$XDG_CONFIG_HOME/ronoctua/brain-modules/`), which the machine's flake
/// hands the build. Unset — the distro's own build, the devshell, a test — is
/// a build with none, and then the table lists none either.
const PERSON_MODULES: &str = "RONOCTUA_BRAIN_PERSON_MODULES_DIR";

/// Whose a module is — the third column of `ids.txt`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// The distro's: a folder under the library's `modules/`.
    Distro,
    /// The person's: a folder under [`PERSON_MODULES`].
    Person,
}

impl Origin {
    fn of(word: &str) -> Option<Self> {
        match word {
            "distro" => Some(Self::Distro),
            "person" => Some(Self::Person),
            _ => None,
        }
    }
}

/// The library's crate, where the distro's module folders and the samples
/// live: this crate's sibling, `crates/ronoctua-brain`.
///
/// THE ONE PLACE THIS CRATE READS OUTSIDE ITS OWN FOLDER, and only two
/// folders of it: `modules/` (every `module.rs` and the files beside it, never
/// a `shared.rs`, which is the library's) and `tests/modules/` (the pilot). A
/// module is ONE folder — its manifest, its code, its NixOS half, its texts —
/// and that folder sits beside the library that compiles its manifest and its
/// shared half; the daemon compiles the rest of it from there. A build that
/// hands this crate its own folder alone (a source cut to
/// `crates/ronoctua-brain-daemon/`) has to hand these two as well.
fn library_dir() -> PathBuf {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let sibling = crate_dir.join("..").join("ronoctua-brain");
    // CANONICAL, so every `#[path]` and every sentence below names the folder
    // and not `ronoctua-brain-daemon/../ronoctua-brain`.
    fs::canonicalize(&sibling)
        .unwrap_or_else(|e| not_handed(&format!("the brain's library at {sibling:?} ({e})")))
}

/// Stops the build where a folder this crate compiles from is not in the
/// source it was handed, saying which two folders it reads beside itself —
/// never the "re-enter the environment" a stale table would get.
fn not_handed(what: &str) -> ! {
    panic!(
        "{what} is not beside the daemon.\n\
         \n\
         The daemon compiles the distro's modules from the library's folder\n\
         (`crates/ronoctua-brain/modules/`, and the pilot under\n\
         `crates/ronoctua-brain/tests/modules/`): a source that holds this\n\
         crate's folder alone cannot build it. Hand those two folders with it."
    )
}

/// A table's directory, from its variable — or the sentence that says how
/// the environment gives it.
fn generated_at(variable: &str) -> PathBuf {
    println!("cargo:rerun-if-env-changed={variable}");
    PathBuf::from(env::var(variable).unwrap_or_else(|_| {
        panic!(
            "{variable} is not set.\n\
             \n\
             The brain's modules are generated by Nix out of their folders\n\
             (`nix/brain-modules.nix`). Enter the environment with `direnv allow`\n\
             (or `nix develop`), which sets the variable, or build through the\n\
             flake with `nix build`.\n\
             \n\
             Generating it here would reimplement Nix logic (invariant 5)."
        )
    }))
}

/// Writes `modules.rs`: one shim per module, then the generated registry
/// (`IDS`, `all` and the rest), for `src/compiled.rs` to include.
///
/// The ids, their Rust names and whose they are come from `ids.txt`, which the
/// generator wrote out of the same folders; nothing here derives a name. A
/// mismatch with the folders on disk — the distro's under the library's
/// crate, the person's under [`PERSON_MODULES`] — stops the build, naming each
/// folder and what fixes it.
fn modules(out: &Path, library: &Path) {
    println!("cargo:rerun-if-env-changed={PERSON_MODULES}");
    let generated = generated_at(REGISTRY);
    let listed_at = generated.join("ids.txt");
    println!("cargo:rerun-if-changed={}", listed_at.display());
    // THE REGISTRY IS THE DAEMON'S PART: it makes the modules (`all.rs`); the
    // library's part declares them and is no registry.
    if !generated.join("all.rs").is_file() {
        panic!(
            "{REGISTRY} is {generated:?}, which is not the daemon's part (no `all.rs`):\n\
             an environment entered before the brain was two crates, or the\n\
             library's part handed to the daemon. Re-enter `nix develop` (or\n\
             `direnv reload`)."
        );
    }
    let listed = fs::read_to_string(&listed_at)
        .unwrap_or_else(|e| panic!("could not read {listed_at:?}: {e}"));
    // `<id> <snake> <origin>` per line, in the generator's order.
    let named: Vec<(&str, &str, Origin)> = listed
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut words = l.split(' ');
            match (words.next(), words.next(), words.next().and_then(Origin::of), words.next()) {
                (Some(id), Some(snake), Some(origin), None) => (id, snake, origin),
                _ => panic!(
                    "{listed_at:?}: `{l}` is not `<id> <snake> <origin>`, the origin `distro` or `person`"
                ),
            }
        })
        .collect();
    let of = |origin: Origin| -> Vec<&str> {
        named
            .iter()
            .filter(|(_, _, o)| *o == origin)
            .map(|(id, _, _)| *id)
            .collect()
    };

    let folder = library.join("modules");
    // WATCH THE FOLDER, OR THE CRATE WHILE THERE IS NO FOLDER: a path that
    // does not exist is one cargo re-runs the script for on every build, and
    // the first module's folder appearing has to be noticed.
    let watched = if folder.is_dir() {
        folder.clone()
    } else {
        library.to_path_buf()
    };
    println!("cargo:rerun-if-changed={}", watched.display());
    let distro = of(Origin::Distro);
    // THE TABLE LISTS THE DISTRO'S MODULES AND THERE IS NO FOLDER: a source cut
    // without it, not a module gone since the environment was entered.
    if !distro.is_empty() && !folder.is_dir() {
        not_handed(&format!("{folder:?}, the distro's module folders,"));
    }
    let mut why = disagreement(&folder, &distro, &listed_at, Origin::Distro);
    why.push_str(&library_disagrees(&folder, &distro));

    // THE PERSON'S, where the variable says — and a table that lists one
    // with nowhere to find it is refused, never compiled without it.
    let person = env::var_os(PERSON_MODULES)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let mine = of(Origin::Person);
    match &person {
        Some(dir) => {
            println!("cargo:rerun-if-changed={}", dir.display());
            if !dir.is_dir() {
                panic!("{PERSON_MODULES} is {dir:?}, which is not a directory.");
            }
            why.push_str(&disagreement(dir, &mine, &listed_at, Origin::Person));
        }
        None if !mine.is_empty() => why.push_str(&format!(
            "\n\
             the table lists the person's modules ({}), and {PERSON_MODULES} is not\n\
             set: there is no folder to load them from. The machine's flake sets it\n\
             with the table (`withPersonModules`); a table and a variable from two\n\
             different builds are a stale environment.\n",
            mine.join(", ")
        )),
        None => {}
    }
    if !why.is_empty() {
        panic!("the brain's modules and the generated table disagree.\n{why}");
    }

    // `{:?}` OF A PATH IS A RUST STRING LITERAL, quotes and escapes included.
    let mut shims = String::new();
    for (id, snake, origin) in &named {
        match (origin, &person) {
            (Origin::Person, Some(dir)) => {
                let manifest = generated.join(id).join("manifest.rs");
                let module = dir.join(id).join("module.rs");
                shims.push_str(&format!(
                    "/// The brain module `{id}`, from the person's folder: the\n\
                     /// manifest Nix generated, and its own `module.rs`.\n\
                     pub mod {snake} {{\n    \
                         include!({manifest:?});\n    \
                         #[path = {module:?}]\n    \
                         mod module;\n    \
                         pub use module::*;\n\
                     }}\n\n"
                ));
            }
            _ => {
                let module = folder.join(id).join("module.rs");
                shims.push_str(&format!(
                    "/// The brain module `{id}`, from its folder `modules/`: the\n\
                     /// library's manifest (and shared half), and its own `module.rs`.\n\
                     pub mod {snake} {{\n    \
                         pub use ronoctua_brain::compiled::{snake}::*;\n    \
                         #[path = {module:?}]\n    \
                         mod module;\n    \
                         pub use module::*;\n\
                     }}\n\n"
                ));
            }
        }
    }
    let registry = generated.join("all.rs");
    println!("cargo:rerun-if-changed={}", registry.display());
    shims.push_str(&format!("include!({registry:?});\n"));

    let destination = out.join("modules.rs");
    let _ = fs::remove_file(&destination);
    fs::write(&destination, shims)
        .unwrap_or_else(|e| panic!("could not write {destination:?}: {e}"));
}

/// What differs between the library's table and the distro's folders, as
/// this script sees them on every edit — empty when they agree.
///
/// The library's own script watches nothing in the folders, so this is where
/// a `shared.rs` added or removed since the environment was entered is said;
/// and the two tables are one evaluation's, so a library listing other
/// modules than the registry is an environment made of two builds.
fn library_disagrees(folder: &Path, distro: &[&str]) -> String {
    let generated = generated_at(MODULES);
    let listed_at = generated.join("ids.txt");
    let shared_at = generated.join("shared.txt");
    if !shared_at.is_file() {
        panic!(
            "{MODULES} is {generated:?}, which is not the library's part (no\n\
             `shared.txt`): an environment entered before the brain was two crates.\n\
             Re-enter `nix develop` (or `direnv reload`)."
        );
    }
    for watched in [&listed_at, &shared_at] {
        println!("cargo:rerun-if-changed={}", watched.display());
    }
    let read = |path: &Path| {
        fs::read_to_string(path).unwrap_or_else(|e| panic!("could not read {path:?}: {e}"))
    };
    let mut why = String::new();
    let library_ids: Vec<String> = read(&listed_at)
        .lines()
        .filter_map(|l| l.split(' ').next())
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if library_ids
        .iter()
        .map(String::as_str)
        .ne(distro.iter().copied())
    {
        why.push_str(&format!(
            "\n\
             the library's table ({listed_at:?}) lists {library_ids:?}, and the\n\
             registry's distro modules are {distro:?}: the two are made by ONE\n\
             evaluation, and an environment that hands two builds is stale.\n\
             Re-enter `nix develop` (or `direnv reload`).\n"
        ));
    }
    let shared: Vec<String> = read(&shared_at)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let on_disk: Vec<&str> = distro
        .iter()
        .copied()
        .filter(|id| folder.join(id).join("shared.rs").is_file())
        .collect();
    let unlisted: Vec<&str> = on_disk
        .iter()
        .copied()
        .filter(|id| !shared.iter().any(|s| s == id))
        .collect();
    let gone: Vec<&str> = shared
        .iter()
        .map(String::as_str)
        .filter(|id| !on_disk.contains(id))
        .collect();
    if !unlisted.is_empty() {
        why.push_str(&format!(
            "\n\
             modules whose folder has a `shared.rs` that {shared_at:?} does not list: {}\n\
             \n\
             The library compiles a shared half only where Nix says one is, from the\n\
             files the flake sees: `git add -N` the new file and re-enter\n\
             `nix develop` (or `direnv reload`).\n",
            unlisted.join(", "),
        ));
    }
    if !gone.is_empty() {
        why.push_str(&format!(
            "\n\
             modules {shared_at:?} lists as carrying a `shared.rs` that has none: {}\n\
             \n\
             The table was generated when the environment was entered, and the\n\
             file is gone since. Re-enter `nix develop` (or `direnv reload`).\n",
            gone.join(", "),
        ));
    }
    why
}

/// What differs between the folders in `folder` and the ids the table lists
/// for this origin, said with what fixes each — empty when they agree.
///
/// BOTH DIRECTIONS, EACH NAMED: what fixes a folder Nix did not list is not
/// what fixes an id with no folder.
fn disagreement(folder: &Path, ids: &[&str], listed_at: &Path, origin: Origin) -> String {
    let mut present: Vec<String> = match fs::read_dir(folder) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    };
    present.sort();
    let unlisted: Vec<&str> = present
        .iter()
        .map(String::as_str)
        .filter(|id| !ids.contains(id))
        .collect();
    let gone: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| !present.iter().any(|p| p == id))
        .collect();
    let mut why = String::new();
    if !unlisted.is_empty() {
        match origin {
            // THE USUAL CAUSE IS GIT, NOT THE ENVIRONMENT: the table is made
            // from the folders the FLAKE sees, and the flake sees only what
            // git tracks. Re-entering the environment without `git add -N`
            // reads the same tree again and stops here again.
            Origin::Distro => why.push_str(&format!(
                "\n\
                 folders in {folder:?} that Nix did not list: {}\n\
                 \n\
                 Nix reads the folders the flake sees, and the flake sees only the files\n\
                 git tracks: a new folder is usually one git does not know yet.\n\
                 \n\
                 {}\n\
                 \n\
                 and then re-enter `nix develop` (or `direnv reload`).\n",
                unlisted.join(", "),
                unlisted
                    .iter()
                    .map(|id| format!("    git add -N crates/ronoctua-brain/modules/{id}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
            // THE PERSON'S FOLDER AND ITS TABLE COME FROM ONE COPY: a folder
            // the table does not list is a table made from another copy.
            Origin::Person => why.push_str(&format!(
                "\n\
                 folders in the person's modules ({folder:?}) that Nix did not list: {}\n\
                 \n\
                 The table and the folder are made from ONE copy of the person's\n\
                 modules (`withPersonModules`); two different copies are two builds.\n",
                unlisted.join(", "),
            )),
        }
    }
    if !gone.is_empty() {
        why.push_str(&format!(
            "\n\
             ids in {listed_at:?} with no folder in {folder:?}: {}\n\
             \n\
             The table was generated when the environment was entered, and the\n\
             folder is gone since. Re-enter `nix develop` (or `direnv reload`).\n",
            gone.join(", "),
        ));
    }
    why
}

/// Writes the shim that loads `tests/modules/pilot-sample` — brain.md §3,
/// "Step 1".
///
/// It is the shape [`modules`] writes for every module: an inline module that
/// `include!`s the manifest and loads `module.rs` by an ABSOLUTE `#[path]`.
/// What it proves is where rustc then looks for a `mod rules;` declared inside
/// `module.rs`: beside it, because a file loaded by `#[path]` owns its own
/// directory. The brain's test reads `file!()` from the sibling to say so.
///
/// The sample lives under the library's `tests/modules/`, which the loader
/// does not read, so it is never registered; its `manifest.rs` is the
/// hand-written golden file the generator is compared with
/// (`brain-modules-match-the-golden`). Only written, never generated from
/// anything: the paths are the brain's own, and the one test that includes
/// it is `cfg(test)`.
fn pilot(out: &Path, library: &Path) {
    let dir = library.join("tests").join("modules").join("pilot-sample");
    if !dir.is_dir() {
        not_handed(&format!("{dir:?}, the pilot sample,"));
    }
    let manifest = dir.join("manifest.rs");
    let module = dir.join("module.rs");
    // `{:?}` OF A PATH IS A RUST STRING LITERAL, quotes and escapes included.
    // DEAD CODE IS ALLOWED on the sample alone: its generated constants are
    // declared for the generator to match, and only some are read.
    let shim = format!(
        "#[allow(dead_code)]\n\
         pub mod pilot_sample {{\n    \
             include!({manifest:?});\n    \
             #[path = {module:?}]\n    \
             mod module;\n    \
             pub use module::*;\n\
         }}\n"
    );
    let destination = out.join("pilot.rs");
    let _ = fs::remove_file(&destination);
    fs::write(&destination, shim)
        .unwrap_or_else(|e| panic!("could not write {destination:?}: {e}"));
}
