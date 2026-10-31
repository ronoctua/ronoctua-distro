# THE PARTS' NAMES ON THE NIX SIDE, as `ronoctua_parts::names` writes them on
# the Rust side — one file, as `nix/machine.nix` is for the machine's folder:
# the parts' module (`nix/modules/parts.nix`) builds from these, and a module
# that reaches a part through the session's view (Chrome's `TryExec`, the
# software centre's variable) imports this file and never spells a path
# (`one-parts-boundary`).
rec {
  partsRoot = "/nix/var/nix/profiles/ronoctua/parts";
  stateDir = "/var/lib/ronoctua-parts";
  gcroots = "/nix/var/nix/gcroots/ronoctua-parts";
  # THE ROOTS OF THE NEXT BOOT's plan (`ronoctua_parts::names::NEXT_ROOTS`),
  # which the update's `plan --next` keeps while a system waits for a restart.
  nextRoots = "${gcroots}/next";
  # THE SESSION'S VIEW of the parts that are here: `bin/` and a closed set
  # of `share/` (`ronoctua_parts::view::LINKED`). A program in it exists
  # only once its part arrived, at the same path whatever its interface —
  # what a launcher entry's `TryExec` asks.
  view = "${stateDir}/view";
  viewBin = name: "${view}/bin/${name}";
}
