# Icons on the system — sections 4.2 and 6.6.
#
# READ BEFORE TOUCHING: this module installs the icon superpackage, which
# exists EXCLUSIVELY as a pixmap fallback (step 2 of section 4.2's chain) for
# foreign apps that bring their own `IconName`.
#
# The INTERFACE's icons come from the font, as glyphs (section 6.3, invariant
# 10). No design system component consults this theme. If one ever does, the
# design has been violated.
{
  config,
  lib,
  options,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua;
  parts = options.ronoctua ? parts;
in
{
  options.ronoctua.icons.superpack = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = ''
      Installs the merged icon superpack.

      MEASURED COST: ~989 MB. It is 335,002 entries and 26,799 distinct name_lists,
      merged from Zafiro, Adwaita, MoreWaita, Papirus and breeze. Papirus
      alone accounts for 87% of the entries — and it is precisely its width
      that serves the `IconName` fallback.

      There is no obvious fat to cut: the distribution by size is flat
      (capping at <= 64 px would save 23 MB out of 918). See docs/ARCHITECTURE.md.

      Switching this off makes step 2 of section 4.2's chain fall straight through to the
      generic glyph (step 3) for every app not in the table.
    '';
  };

  config = lib.mkMerge [
    # ── anchor: L25 ──
    # THE SUPERPACK IS A PART (`nix/modules/parts.nix`), of data: 0,63 GiB
    # the full profile carries, realized after the installation and out of
    # the system's closure. The session sees its theme through the parts'
    # view (`share/icons`), and the launcher reads it by the part's stable
    # path on every look-up — until it arrived, the chain starts at
    # `hicolor`, the degradation `ronoctua_shell_door::launch::superpack`
    # describes. Declared outside the switch, which is its `wanted`.
    (lib.optionalAttrs parts {
      ronoctua.parts.icon-superpack = {
        wanted = cfg.icons.superpack;
        kind = "data";
        name = "noctua-icon-theme";
        package = ronoctua.iconSuperpack;
      };
    })
    (lib.mkIf (cfg.icons.superpack && parts) (
      let
        iconsRoot = "${config.ronoctua.parts.icon-superpack.path}/share/icons/noctua";
      in
      {
        environment.systemPackages = [ pkgs.hicolor-icon-theme ];
        environment.sessionVariables.RONOCTUA_ICONS = iconsRoot;
        systemd.user.services.ronoctua-footer.environment.RONOCTUA_ICONS = iconsRoot;
        environment.sessionVariables.XDG_ICON_THEME = "noctua";
      }
    ))
  ];
}
