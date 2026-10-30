# The workshop — Blender, and OrcaSlicer as what opens a 3D model.
#
# WHAT IS DECLARED
# ----------------
# - Blender and OrcaSlicer, from nixpkgs. Programs one installs to USE, so
#   `profile.nix` turns this on in the full profile only.
# - OrcaSlicer as the DEFAULT for `model/stl`, `model/3mf` and `model/step`,
#   in `/etc/xdg/mimeapps.list` (`xdg.mime.defaultApplications`, which merges
#   with the queue's magnet default in `transfers.nix`). The file manager
#   reads the defaults before the declarations (`ronoctua_apps::defaults`).
#   OrcaSlicer's own entry now claims all three upstream (orca-slicer 2.4.2 on
#   nixos-unstable gained `model/step`), so the default no longer rescues a
#   `.step` from a `text/plain` editor — it makes OrcaSlicer win
#   deterministically over any other claimant.
#
# `/etc/xdg` sits above `$XDG_DATA_DIRS/applications`, where the player keeps
# its own defaults, and below the person's `~/.config/mimeapps.list`: a
# choice the person makes still wins.
#
# WHY THE ENTRY'S NAME IS WRITTEN HERE, AND WATCHED
# --------------------------------------------------
# The default has to be a string at evaluation, and the only place the name
# really lives is the package's `share/applications`. It was READ there while
# evaluating (`builtins.readDir`, import-from-derivation), and a
# `system.checks` read it again at build — both of which made evaluating or
# building a system wait for OrcaSlicer itself. Both are a PART now
# (`nix/modules/parts.nix`): realized after the installation, never needed
# to evaluate one, which an installation without a network has to do. So the
# name is written here, and `models-open-in-orca` reads the package's own
# entry independently and refuses a default that names any other file — the
# day upstream renames it, that check says so.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.workshop;
  orca = pkgs.orca-slicer;
  # THE PACKAGE'S OWN ENTRY (see above), watched by `models-open-in-orca`.
  # nixos-unstable renamed it to the reverse-DNS id (was `OrcaSlicer.desktop`).
  orcaEntry = "com.orcaslicer.OrcaSlicer.desktop";
  models = [
    "model/stl"
    "model/3mf"
    "model/step"
  ];

in
{
  # The registry this module writes its two programs into (see `browser.nix`
  # for why it is imported here too).
  imports = [ ./distro-apps.nix ];

  options.ronoctua.workshop.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Blender and OrcaSlicer, with OrcaSlicer as the default for
      `model/stl`, `model/3mf` and `model/step`. On in the full profile.
    '';
  };

  config = lib.mkMerge [
    {
      # THE TWO ARE PROGRAMS OF THE REGISTRY (`nix/modules/distro-apps.nix`),
      # in the `workshop` group: this module's switch says the machine has
      # them, and a person may decline either.
      #
      # BOTH ARE PARTS (`nix/modules/parts.nix`): gigabytes the full profile
      # carries, realized after the installation and out of the system's
      # closure — the ISO need not hold them, and their downloads hold back
      # nothing. Heavy: the small parts arrive first.
      ronoctua.apps.blender = {
        package = pkgs.blender;
        part = true;
        heavy = true;
        name = "Blender";
        group = "workshop";
        desktopIds = [ "blender.desktop" ];
      };
      ronoctua.apps.orca-slicer = {
        package = orca;
        part = true;
        heavy = true;
        name = "OrcaSlicer";
        group = "workshop";
        desktopIds = [ orcaEntry ];
      };
    }

    (lib.mkIf cfg.enable { xdg.mime.enable = true; })

    # THE MODEL TYPES FOLLOW ORCASLICER, and not only the switch: a person
    # who declined it would otherwise keep a default naming an entry that is
    # not there, and every reader skips such a default in silence.
    (lib.mkIf config.ronoctua.distroApps.installed.orca-slicer {
      xdg.mime.defaultApplications = lib.genAttrs models (_: orcaEntry);
    })
  ];
}
