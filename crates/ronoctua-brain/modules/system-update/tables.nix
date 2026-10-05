# THE UPDATE SEQUENCE'S TABLE, as the Rust this module reads: every step's id,
# its class, whether it runs as root, whether it ships on, what it needs, the
# command a person reads, and its two text keys.
#
# THE SAME DERIVATION `module.nix` and `checks.nix` build (`sequence.nix`,
# called with nothing but its own declaration beside it), so the table the
# brain draws and the table the runners were generated from are one table.
# It carries no store path: the commands cross as basenames (`sequence.nix`
# says why), so editing a step rebuilds nothing but this table.
{ pkgs, ... }:
let
  sequence = pkgs.callPackage ./sequence.nix { };
in
{
  "updates.rs" = "${sequence.table}/updates.rs";
}
