# The machine's flake registries: what `nixpkgs#…` and `<nixpkgs>` mean on
# it, and where the global registry comes from. Imported by `software.nix`,
# the module that needs them, and by the VM tests that measure a `nix`
# command the way a machine runs it (`tests/vscode-vm.nix`) — never copied.
{
  lib,
  pkgs,
  inputs,
  ...
}:

{
  # `nixpkgs#…` IS THE SYSTEM'S OWN nixpkgs, LOCKED.
  #
  # `nixosSystem` already points the registry at the system's source, but by
  # PATH alone — an entry with no `narHash` is not a locked one, so every
  # `nix profile add nixpkgs#x` re-hashes the source tree and nothing about it
  # can be cached. Given the flake input itself, the entry carries its `rev`
  # and `narHash`: what a person installs for themselves (invariant 6's dated
  # exception) comes from EXACTLY the revision the system was built from, and
  # the search index (`nix/software-index.nix`) was built from that same one.
  # It moves when the system's `flake update` moves it, and never on its own.
  #
  # Not in `base.nix`: the VM tests that import the base alone have no flake
  # inputs to give it.
  nix.registry.nixpkgs.flake = inputs.nixpkgs;

  # AND THE GLOBAL REGISTRY FROM THE STORE, never from `channels.nixos.org`:
  # Nix reads it before any entry of the machine's, and with no network and
  # no copy in the person's cache the command failed (`nix/flake-registry.nix`
  # says what was measured). A default: a VM test that wants no global
  # registry at all (`flake-registry = ""`) says so over it.
  nix.settings.flake-registry = lib.mkDefault "${import ../flake-registry.nix {
    inherit (pkgs) fetchurl;
  }}";
}
