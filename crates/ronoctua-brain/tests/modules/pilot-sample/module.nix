# The sample's NixOS half: one file in `/etc`, and one more in a sibling file
# it imports — nothing else.
#
# Enough for `brain-modules-match-the-golden` to see the gate `nix/brain-modules.nix`
# puts around every module's `module.nix` open and close, and around what it
# imports: turned off with `ronoctua.brain.modules.pilot-sample.enable = false`,
# neither file is there.
{ ... }:
{
  imports = [ ./etc.nix ];

  environment.etc."ronoctua/pilot-sample".text = "a sample module\n";
}
