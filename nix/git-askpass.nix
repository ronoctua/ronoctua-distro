# `ronoctua-git-askpass`, BUILT ON ITS OWN: what git runs as `GIT_ASKPASS`
# for a remote over HTTPS with a token (see `crates/ronoctua-git`).
#
# WHY NOT EXTRACTED FROM THE WORKSPACE, like the queue's daemon. Every program
# that links `ronoctua-git` has to find the helper, and the extracted ones
# (`rust.transfers`, the brain's, whoever runs the belongings) have nothing
# beside them — a lookup next to `current_exe()` finds it in the workspace's
# `bin/` and nowhere else. So the path is COMPILED IN (`RONOCTUA_GIT_ASKPASS`
# in `generated`), and a path the workspace compiles in cannot come out of
# the workspace: it would be the workspace referencing itself.
#
# It can come out of this instead, because the crate has no dependency at all:
# `rustc` on its sources, the library then the binary — seconds, and only
# `crates/ronoctua-git/src` is an input, so nothing else rebuilds it.
{ pkgs, rustToolchain }:

let
  src = builtins.path {
    name = "ronoctua-git-src";
    path = ../crates/ronoctua-git/src;
  };
in
# `runCommandCC`: rustc links through `cc`.
pkgs.runCommandCC "ronoctua-git-askpass" { nativeBuildInputs = [ rustToolchain ]; } ''
  rustc --edition 2024 --crate-type rlib --crate-name ronoctua_git \
    -C opt-level=2 --cap-lints warn \
    ${src}/lib.rs -o libronoctua_git.rlib
  mkdir -p $out/bin
  rustc --edition 2024 --crate-type bin --crate-name ronoctua_git_askpass \
    -C opt-level=2 -C strip=symbols --cap-lints warn \
    --extern ronoctua_git=libronoctua_git.rlib \
    ${src}/bin/askpass.rs -o $out/bin/ronoctua-git-askpass
''
