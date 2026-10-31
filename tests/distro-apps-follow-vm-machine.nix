# THE MACHINE'S SYSTEM, AS FAR AS FOLLOWING GOES: the distro's own registry
# (`nix/modules/distro-apps.nix`) and parts (`nix/modules/parts.nix`) over
# the table `tests/distro-apps-follow-vm.nix` plants, evaluated ALIKE on the
# host and inside that test's VM.
#
# WHY TWICE
# ---------
# Inside the VM it is the machine's build: it reads root's copies of the
# person's options and followed pins from the machine directory, decides
# which pin the AppImage's part is built at, and the plan it writes names
# that part's derivation — which the VM's realizer builds. A `.drv` cannot
# be handed to a VM's store without every output of its graph (the
# `exportReferencesGraph` trap, AGENTS.md), so the VM instantiates its own;
# and the host evaluates THE SAME FILE with the same nixpkgs for a twin of
# the part (the registry's builder over a local image), whose inputs the VM
# is given. A derivation of the VM's whose inputs are the twin's builds
# there, offline; one that needs anything else fails saying what.
#
# The whole distro is not evaluated: a NixOS system with these two modules
# and the three options they read of others is what decides a followed pin.
{
  # The nixpkgs source, as a string: the VM names it by its store path.
  nixpkgs,
  # The distro's files the two modules read, by their place in the tree.
  distro,
  # `ronoctua-parts` — named only by the parts' units, never built here.
  partsTool,
  # The registry's table and the distro's pins, as the test plants them.
  table,
  pins,
  # Root's copies of the person's options and followed pins
  # (`nix/machine.nix`'s `person/options.json` and
  # `person/software/distro-pins.json`), or `null` when there is none.
  options ? null,
  distroPins ? null,
}:
import "${nixpkgs}/nixos/lib/eval-config.nix" {
  system = "x86_64-linux";
  modules = [
    "${distro}/nix/modules/distro-apps.nix"
    (import "${distro}/nix/modules/parts.nix" { tool = partsTool; })
    (
      { lib, ... }:
      {
        # What the two modules read of `base.nix` and `person.nix`, declared
        # bare (the `person-vm` precedent): those modules carry the whole
        # machine with them.
        options.ronoctua.user = lib.mkOption { type = lib.types.str; };
        options.ronoctua.person = {
          options = lib.mkOption {
            type = lib.types.nullOr lib.types.path;
            default = null;
          };
          distroPins = lib.mkOption {
            type = lib.types.nullOr lib.types.path;
            default = null;
          };
        };
        config = {
          ronoctua.user = "noctua";
          networking.hostName = "machine";
          users.users.noctua = {
            isNormalUser = true;
            uid = 1000;
          };
          ronoctua.distroApps = { inherit table pins; };
          ronoctua.person = { inherit options distroPins; };
          system.stateVersion = lib.trivial.release;
        };
      }
    )
  ];
}
