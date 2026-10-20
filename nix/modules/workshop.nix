# The workshop — Blender, and OrcaSlicer as what opens a 3D model.
#
# WHAT IS DECLARED
# ----------------
# - Blender and OrcaSlicer, from nixpkgs. Programs one installs to USE, so
#   `profile.nix` turns this on in the full profile only.
# - OrcaSlicer as the DEFAULT for `model/stl`, `model/3mf` and `model/step`,
#   in `/etc/xdg/mimeapps.list` (`xdg.mime.defaultApplications`, which merges
#   with the queue's magnet default in `transfers.nix`). The file manager
#   reads the defaults before the declarations (`ronoctua_apps::defaults`),
#   and that is what a `.step` needs: the database makes `model/step` a
#   `text/plain`, OrcaSlicer's entry does not name it, and without a default
#   the first editor that declares `text/plain` opened it.
#
# `/etc/xdg` sits above `$XDG_DATA_DIRS/applications`, where the player keeps
# its own defaults, and below the person's `~/.config/mimeapps.list`: a
# choice the person makes still wins.
#
# WHY THE ENTRY'S NAME IS READ FROM THE PACKAGE, AT EVALUATION
# --------------------------------------------------------------
# The default has to be a string at evaluation, and the only place the name
# really lives is the package's `share/applications`. Writing it here by hand
# was a second place deciding it, true until the day upstream renames the
# file. So the folder is READ while evaluating (`builtins.readDir`), which is
# import-from-derivation: the evaluation of a system with OrcaSlicer
# installed waits for it to be in the store — substituted once, kept by the
# system that installs it. A system without it (the workshop off, or the
# person declined it) never asks. And the package's entries are counted: one
# entry is THE entry, and none, or two, refuses the evaluation by name rather
# than picking one.
# `system.checks` still reads the entry at build: it must be an application
# that claims the model types, and `models-open-in-orca` proves the whole way
# from a file to the entry.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.workshop;
  orca = pkgs.orca-slicer;
  # THE PACKAGE'S OWN ENTRY, read from its folder (see above): one `.desktop`,
  # or the evaluation stops naming what it found.
  orcaEntries = lib.filter (lib.hasSuffix ".desktop") (
    lib.attrNames (builtins.readDir "${orca}/share/applications")
  );
  orcaEntry =
    assert lib.assertMsg (lib.length orcaEntries == 1)
      "workshop.nix: ${orca.name} has ${toString (lib.length orcaEntries)} desktop entries (${toString orcaEntries}), and the model types need exactly one to open in";
    lib.head orcaEntries;
  models = [
    "model/stl"
    "model/3mf"
    "model/step"
  ];

  # THE NAME, READ FROM THE PACKAGE. It must exist, and it must be an
  # application entry: a default naming a file that is not there is skipped
  # by every reader, and the model would fall back to the editor in silence.
  entryIsThere =
    pkgs.runCommand "workshop-orca-entry"
      {
        entry = "${orca}/share/applications/${orcaEntry}";
      }
      ''
        if [ ! -f "$entry" ]; then
          echo "ERROR: ${orcaEntry} is not in ${orca.name}; these are:" >&2
          ls ${orca}/share/applications >&2 || true
          exit 1
        fi
        grep -q '^Type=Application' "$entry" || {
          echo "ERROR: ${orcaEntry} is not an application entry" >&2; exit 1; }
        grep -q '^MimeType=.*model/stl' "$entry" || {
          echo "ERROR: ${orcaEntry} no longer opens model/stl" >&2; exit 1; }
        touch $out
      '';
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
      ronoctua.apps.blender = {
        package = pkgs.blender;
        name = "Blender";
        group = "workshop";
        desktopIds = [ "blender.desktop" ];
      };
      # ITS ENTRY'S NAME ONLY WHERE IT IS INSTALLED: reading it is the
      # import-from-derivation argued above, and a machine without
      # OrcaSlicer — the workshop off, or the person declined it — must
      # never be made to fetch it to say it has none. `installed` reads each
      # entry's `when` and the declines, never a `desktopIds`, so this does
      # not go round.
      ronoctua.apps.orca-slicer = {
        package = orca;
        name = "OrcaSlicer";
        group = "workshop";
        desktopIds = lib.optional config.ronoctua.distroApps.installed.orca-slicer orcaEntry;
      };
    }

    (lib.mkIf cfg.enable { xdg.mime.enable = true; })

    # THE MODEL TYPES FOLLOW ORCASLICER, and not only the switch: a person
    # who declined it would otherwise keep a default naming an entry that is
    # not there, and every reader skips such a default in silence.
    (lib.mkIf config.ronoctua.distroApps.installed.orca-slicer {
      xdg.mime.defaultApplications = lib.genAttrs models (_: orcaEntry);
      system.checks = [ entryIsThere ];
    })
  ];
}
