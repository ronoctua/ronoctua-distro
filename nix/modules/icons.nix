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
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua;
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

  config = lib.mkIf cfg.icons.superpack (
    let
      iconsRoot = "${ronoctua.iconSuperpack}/share/icons/noctua";
    in
    {
      environment.systemPackages = [
        ronoctua.iconSuperpack
        # The mandatory terminal of the inheritance chain per the freedesktop spec.
        # The merged theme declares `Inherits=hicolor`, so it has to exist.
        pkgs.hicolor-icon-theme
      ];

      # WHERE THE FOOTER FINDS THE SUPERPACK, at run time.
      #
      # It used to be compiled into the binary with `option_env!`, which made this
      # very option a switch that turned nothing off: the store path was a
      # reference the closure carried whether it was on or not, all 0,61 GiB of it.
      #
      # Through the session's environment instead, and inside this `mkIf`: a
      # machine that asked for no superpack leaves the variable unset, the shell
      # skips the pixmap step, and the application list keeps its glyphs — which
      # is invariant 10's arrangement anyway, since no component of the design
      # system consults this package.
      # BOTH PLACES, because they answer different questions. `sessionVariables`
      # reaches a login shell; the footer is a systemd USER UNIT, and a unit
      # inherits the manager's environment rather than the shell's. Setting only
      # the first is how this would look configured and reach nothing.
      environment.sessionVariables.RONOCTUA_ICONS = iconsRoot;
      systemd.user.services.ronoctua-footer.environment.RONOCTUA_ICONS = iconsRoot;

      # The system's default theme for foreign apps. It does not touch our UI.
      environment.sessionVariables.XDG_ICON_THEME = "noctua";

      # GTK looks the icon theme up by name in `settings.ini`; that file is
      # written ONCE, by `nix/modules/foreign.nix`, which reads this switch —
      # two modules writing one INI would be two `[Settings]` headers.
    }
  );
}
