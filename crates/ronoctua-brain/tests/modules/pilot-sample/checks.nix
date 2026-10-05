# The sample's own checks, in the shape every module's `checks.nix` has: the
# arguments it asks for by name (`nix/brain-modules.nix`, `checkArguments`),
# and what it answers — its checks, each named after its id or one of its key
# prefixes, and which of them only the gate runs.
{ mkCheck, pkgs, ... }:
{
  checks = {
    pilot-sample-is-a-sample = mkCheck "pilot-sample-is-a-sample" [ pkgs.coreutils ] ''
      echo "OK: a sample module's check runs"
    '';
  };
  slow = [ "pilot-sample-is-a-sample" ];
}
