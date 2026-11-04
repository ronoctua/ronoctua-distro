# Nyxt, the keyboard-driven browser, in every installation.
#
# THE OWNER'S DECISION (2026-11-04): every new installation comes with Nyxt,
# in both profiles — so its entry is in the `base` group, which no switch of
# `profile.nix` turns off. A person still declines it through the registry
# (`software/declined.json`), as any program that is not essential.
#
# FROM NIXPKGS, as it is: free (BSD-3-Clause) and on the public cache, so it
# is no part (`nix/modules/parts.nix`) and the ISO's seed carries it.
#
# WHAT IT WEIGHS, measured on 2026-11-04 (`nix path-info -S`): the program is
# one SBCL image of 2,4 GiB unpacked, nearly all of it zeros — the download is
# 37 MB, and the store's zstd (`nix/disko/default.nix`) keeps it near that on
# disk. Against a built full profile its closure adds that image, the SBCL it
# runs on (141 MiB; not the Common Lisp of `dev.nix`) and two small tools. A
# minimal machine also gains its WebKitGTK and GStreamer (not measured).
#
# IT TAKES NOTHING FROM CHROME. Its entry claims the web (`text/html`, `http`,
# `https`) and a few picture types: where Chrome is installed, `browser.nix`
# names it the default for the web, and its name sorts first in the file
# manager's opener; where it is not — the minimal profile, a decline — Nyxt is
# the browser.
{ pkgs, ... }:

{
  # The registry this module writes Nyxt's entry into (see `browser.nix` for
  # why it is imported here too).
  imports = [ ./distro-apps.nix ];

  ronoctua.apps.nyxt = {
    package = pkgs.nyxt;
    name = "Nyxt";
    desktopIds = [ "nyxt.desktop" ];
  };
}
