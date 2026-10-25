# A DECLARED FLATPAK — `{ source = "flatpak"; app-id; origin; }`.
#
# The builder's contract is `nix/declared-apps/nix.nix`'s. A Flatpak is not
# built: it goes on nix-flatpak's Home Manager list for the person's own
# `--user` installation (`nix/modules/flatpak.nix` imports the module), which
# installs it at activation and ADOPTS a copy the person installed by hand
# instead of installing it twice. `uninstallUnmanaged` stays false there, so
# nothing the person installed and never declared is touched.
#
# The remote has to be one this machine declares: nix-flatpak adds remotes
# from its own list only, and an app from a remote it does not have would be
# an install that fails at every activation. Such an entry is skipped and said.
{ pkgs, ... }:

let
  skip = reason: detail: {
    ok = false;
    inherit reason detail;
  };
  done =
    what:
    {
      ok = true;
      packages = [ ];
      flatpaks = [ ];
      desktopEntries = { };
      apks = [ ];
    }
    // what;

  # Reverse DNS with at least three parts, as Flatpak requires.
  isAppId =
    id:
    builtins.isString id
    && builtins.stringLength id <= 255
    && builtins.match "[A-Za-z_][A-Za-z0-9_-]*([.][A-Za-z_][A-Za-z0-9_-]*){2,}" id != null;
  isRemote =
    name:
    builtins.isString name
    && builtins.stringLength name <= 64
    && builtins.match "[A-Za-z0-9_-][A-Za-z0-9._-]*" name != null;

  # What `nix-flatpak` takes.
  item = entry: {
    appId = entry."app-id";
    inherit (entry) origin;
  };
in
{
  shape =
    entry:
    if !isAppId (entry."app-id" or null) then
      "`app-id` is not a Flatpak application id"
    else if !isRemote (entry.origin or null) then
      "`origin` is not a remote's name"
    else
      null;

  build =
    {
      entry,
      # The remotes this machine declares (`services.flatpak.remotes`).
      remotes ? [ ],
      ...
    }:
    if !builtins.elem entry.origin remotes then
      skip "source-off" "the remote `${entry.origin}` is not declared on this machine"
    else
      done { flatpaks = [ (item entry) ]; };
}
