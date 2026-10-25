# OLD ISOs LEAVE THE STORE (`nix/scripts/iso-sweep.py` says how and why).
#
# Every app that builds an ISO ends by running this — `.#iso`, `.#release`,
# `.#gate`, `.#most` and `.#sized-build`, which is how a VM test of the
# installer is built by hand — and `.#iso-sweep` runs it alone. An ISO that
# is to stay gets a root (`--out-link`); the newest of each name always
# stays. `iso-sweep-keeps-what-it-must` holds the choice to that.
#
# `nix` and `nix-store` are the caller's, from its `PATH`: the ones that talk
# to the daemon of the store being swept.
{
  writeShellApplication,
  python3,
}:

writeShellApplication {
  name = "ronoctua-iso-sweep";
  runtimeInputs = [ python3 ];
  text = ''
    exec python3 -I ${./scripts/iso-sweep.py} "$@"
  '';
}
