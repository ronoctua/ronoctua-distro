# THIS MACHINE.
#
# The ronoctua distro, as an input that follows its `release` branch, and
# beside this file what is this machine's own: `host.nix` and `facter.json`
# (written by the installer), the person's declarations under `person/`, and
# root's `local/`. The update moves the `ronoctua` and `nixpkgs` inputs
# forward (`nix flake update ronoctua nixpkgs`) and rebuilds, for the next
# boot; nothing else here changes on its own.
#
# THE DISTRO'S OWN PINS ARE USED AS THEY ARE, EXCEPT nixpkgs, WHICH THIS
# MACHINE ROLLS (owner amendment 2026-10-30). The whole system follows
# `nixos-unstable`: this flake carries its own `nixpkgs` input and points the
# distro's at it, and the weekly update moves it (`nix flake update nixpkgs`).
# Every other input stays the distro's tested pin (`machine-uses-the-distro-pins`).
{
  description = "This machine: the ronoctua distro, its host.nix and the person's declarations";

  # ROLLING: nixos-unstable, moved weekly by the update.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  # `github:` and not `git+https:`, decided: the tarball carries no history,
  # and Nix fetches it over HTTP by itself. A `git+https:` input needs a `git`
  # program on the PATH of whoever updates — and the update runs in a unit,
  # whose PATH is minimal. The GitHub API answers 60 unauthenticated requests
  # an hour per address; a machine asks twice per update (the distro and
  # nixpkgs).
  inputs.ronoctua.url = "github:ronoctua/ronoctua-distro/release";
  # The distro builds against this machine's rolling nixpkgs.
  inputs.ronoctua.inputs.nixpkgs.follows = "nixpkgs";

  outputs =
    { ronoctua, ... }:
    ronoctua.lib.machineFlake {
      dir = ./.;
      # The contract this file was written for: a distro that speaks
      # another refuses it with a sentence instead of half-building it.
      contract = 1;
    };
}
