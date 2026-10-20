# THE GLOBAL FLAKE REGISTRY, pinned: what a machine's Nix reads instead of
# downloading it (`nix/modules/nix-registries.nix`).
#
# Nix resolves an indirect flake reference (`nixpkgs#…`, `<nixpkgs>` through
# `NIX_PATH=nixpkgs=flake:nixpkgs`) by reading EVERY registry first, the
# global one included — downloaded from `channels.nixos.org` and kept in the
# person's `~/.cache/nix` for an hour — even when the machine's own registry
# answers. Measured: with no network and no copy in the cache the download
# fails, and so does the command (`tests/vscode-vm.nix`, rust-analyzer's own
# NixOS patch, a `nix-build` over `<nixpkgs>`, before this pin); with a copy,
# fresh or expired, Nix says "using cached version" and goes on (a registry
# served and then taken away). A person's first offline `nixpkgs#…` failed on
# that alone, as `tests/declare-vm.nix` had found.
#
# A COMMIT OF THE REGISTRY'S OWN REPOSITORY, by its hash: the same bytes
# forever (`downloads-are-pinned`), held by the flake's root
# (`nix/kept-sources.nix`) and by every system that names it. What it says
# moves only when somebody moves `rev`; the system's own `nixpkgs` comes first
# anyway (the machine's registry, `nix/modules/nix-registries.nix`).
{ fetchurl }:

let
  rev = "46ea0f17cf6777e156a2be6ae802d9f95d93dd5e";
in
fetchurl {
  url = "https://raw.githubusercontent.com/NixOS/flake-registry/${rev}/flake-registry.json";
  hash = "sha256-9+siRwR7Rt7ZffF8iQYlTN4oSTM29daQJNUF0YT8YUw=";
}
