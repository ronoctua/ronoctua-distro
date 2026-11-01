# The reference machine's own file (`../machine/host.nix`, exactly what the
# installer writes), for a machine whose person's one brain module waits for
# an approval (`machine-compiles-approved-person-modules`).
{
  imports = [ ../machine/host.nix ];
}
