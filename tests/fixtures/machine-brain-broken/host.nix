# The reference machine's own file (`../machine/host.nix`, exactly what the
# installer writes), for a machine whose person's brain modules include some
# that do not evaluate (`a-folder-that-does-not-evaluate-stops-only-itself`).
{
  imports = [ ../machine/host.nix ];
}
