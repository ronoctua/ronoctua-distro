# The reference machine's own file (`../machine/host.nix`, exactly what the
# installer writes), for a machine whose `person/` carries brain modules.
{
  imports = [ ../machine/host.nix ];
}
