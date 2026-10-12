# The boot trial's watch has no NixOS half of its own: it runs nothing and
# starts no unit. The record it reads is the machine's (`nix/modules/
# machine.nix` declares its folder, readable by everybody, and the bless that
# writes it). The file exists because every module has one;
# `ronoctua.brain.modules.<id>.enable = false` still switches the watch off,
# through the gate `nix/brain-modules.nix` puts around this file.
{ ... }:
{
}
