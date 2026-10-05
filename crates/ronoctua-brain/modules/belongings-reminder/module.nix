# The belongings' reminder has no NixOS half of its own: it runs nothing and
# starts no unit. It reads the files the belongings' CLI leaves
# (`nix/modules/belongings.nix` declares that program, for the queue) and
# watches them from the brain's own watch thread. The file exists because
# every module has one; `ronoctua.brain.modules.<id>.enable = false` still
# switches the reminder off, through the gate `nix/brain-modules.nix` puts
# around this file.
{ ... }:
{
}
