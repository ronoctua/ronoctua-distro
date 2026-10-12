# THIS MACHINE.
#
# The ronoctua distro, as an input that follows its `release` branch, and
# beside this file what is this machine's own: `host.nix` and `facter.json`
# (written by the installer), the person's declarations under `person/`, and
# root's `local/`. The update moves the `ronoctua` input forward
# (`nix flake update ronoctua`) and rebuilds; nothing else here changes on its
# own.
#
# THE DISTRO'S OWN PINS ARE USED AS THEY ARE: no `follows` and no override.
# The distro refuses to build with any input other than the ones its release
# was tested with (`machine-uses-the-distro-pins`).
{
  description = "This machine: the ronoctua distro, its host.nix and the person's declarations";

  # `github:` and not `git+https:`, decided: the tarball carries no history,
  # and Nix fetches it over HTTP by itself. A `git+https:` input needs a `git`
  # program on the PATH of whoever updates — and the update runs in a unit,
  # whose PATH is minimal. The GitHub API answers 60 unauthenticated requests
  # an hour per address; a machine asks once per update.
  inputs.ronoctua.url = "github:ronoctua/ronoctua-distro/release";

  outputs =
    { ronoctua, ... }:
    ronoctua.lib.machineFlake {
      dir = ./.;
      # The contract this file was written for: a distro that speaks
      # another refuses it with a sentence instead of half-building it.
      contract = 1;
    };
}
