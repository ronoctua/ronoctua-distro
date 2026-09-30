# WHERE CARGO WRITES — the one place these names live (invariant 16).
#
# # The defect
#
# A local `cargo` puts its build next to the manifest, in `./target`, and a
# day of `cargo test` there is tens of gigabytes. Inside a flake's tree that
# is the 793 GB of `lean-tree`; outside it, in a directory named by hand in
# three files (`.envrc`, the devshell, the loop), it became hundreds of
# gigabytes that no sweep understood and no compiler upgrade ever released —
# the objects of every toolchain the project had used, piled up in one
# directory.
#
# # The layout, on every machine
#
#   <cache>/cargo-target/rustc-<version>/target                        final artifacts
#   <cache>/cargo-target/rustc-<version>/build/{workspace-path-hash}   intermediates
#
# - `build.build-dir` (stable since 1.91) is where the INTERMEDIATES go — the
#   `deps/`, the fingerprints, the incremental state: nearly all of the bytes.
#   `{workspace-path-hash}` is cargo's own template, expanded per workspace
#   into two directory levels (`ab/cdef…`), so two projects never share one.
# - `build.target-dir` keeps only what cargo UPLIFTS: the binaries a person
#   runs. It has no template, so it is one directory for everything that sets
#   it — which is why only this repository sets it (below).
# - `rustc-<version>` is the toolchain of the evaluation that wrote the path.
#   A new compiler starts a fresh directory instead of rebuilding over the old
#   one, and the old one is a unit a sweep can take whole once nothing writes
#   into it. It labels the SESSION's toolchain, not whichever compiler ran.
# - `cargo-target` and not `cargo`: some dotfiles point `CARGO_HOME` at
#   `$XDG_CACHE_HOME/cargo`, and a sweep of that name would take installed
#   binaries with it.
# - The same subvolume as the rest of the cache, so uplifting stays a hard link
#   and never a copy.
#
# # Who reads it
#
# - The SESSION, on every machine (`nix/modules/cache.nix`): only
#   `CARGO_BUILD_BUILD_DIR`. The intermediates of every project on the machine
#   leave every tree, and each project keeps its own `./target` for the few
#   files a person runs. Setting `CARGO_TARGET_DIR` there too would override
#   every project's own `build.target-dir`, make `cargo clean` anywhere empty
#   everybody's binaries, and let two projects with a binary of the same name
#   overwrite each other.
# - THIS REPOSITORY (`nix/devshell.nix`, `nix/dev.nix`): both variables. Its
#   `./target` would be inside the flake's tree, which is `lean-tree`'s whole
#   subject, and the loop looks for its footer under the target directory.
# - The sweep (`ronoctua-cache-sweep`): `parent`, `compilerPrefix` and
#   `compiler` are the names it walks — only a directory named with the prefix
#   is ever a compiler's, and the session's own is never taken — and nothing
#   else about the layout is a contract: what is below `build/` is cargo's,
#   and unstable.
{ version }:

let
  compilerPrefix = "rustc-";
  compiler = "${compilerPrefix}${version}";
  under = root: {
    target = "${root}/cargo-target/${compiler}/target";
    build = "${root}/cargo-target/${compiler}/build/{workspace-path-hash}";
  };
  # THE SHELL'S ROOT honours `XDG_CACHE_HOME`. The session's cannot: pam_env
  # has no `:-`, and NixOS rewrites only `$HOME` and `$USER` into pam_env's
  # own syntax. A person who moves `XDG_CACHE_HOME` gets the devshell's build
  # there and every other project's under `~/.cache` — two directories,
  # neither in a tree, and only the one under `XDG_CACHE_HOME` (`%C` of the
  # units) is walked by the sweep and the relief.
  shell = under "\${XDG_CACHE_HOME:-$HOME/.cache}";
in
{
  parent = "cargo-target";
  inherit compiler compilerPrefix;

  # For `environment.sessionVariables` and `environment.d`, which both expand
  # `$HOME` and leave `{…}` alone.
  session = under "$HOME/.cache";

  inherit shell;

  # THE DEVSHELL'S TWO LINES. It always exports: inside `nix develop` the
  # toolchain is the flake's, and so is the directory that names it.
  exports = ''
    export CARGO_TARGET_DIR="${shell.target}"
    export CARGO_BUILD_BUILD_DIR="${shell.build}"
    mkdir -p "$CARGO_TARGET_DIR"
  '';

  # THE LOOP'S, which keeps whatever its caller already exported — the
  # devshell's, or the session's build directory — and fills in only what is
  # missing. `mkdir` on the target and never on the build directory: that one
  # is a template until cargo expands it.
  #
  # `if` AND NOT `''${VAR:-default}`: the template's own `}` closes the
  # expansion, so a variable that WAS set came out with a stray `}` on the end
  # — measured in bash, and silent: cargo would have made that directory.
  fallback = ''
    if [ -z "''${CARGO_TARGET_DIR:-}" ]; then
      export CARGO_TARGET_DIR="${shell.target}"
    fi
    if [ -z "''${CARGO_BUILD_BUILD_DIR:-}" ]; then
      export CARGO_BUILD_BUILD_DIR="${shell.build}"
    fi
    export CARGO_TARGET_DIR CARGO_BUILD_BUILD_DIR
    mkdir -p "$CARGO_TARGET_DIR"
  '';
}
