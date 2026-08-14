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
      Instala o superpacote de icones fundido.

      CUSTO MEDIDO: ~989 MB. Sao 335.002 entradas e 26.799 name_list distintos,
      fundidos de Zafiro, Adwaita, MoreWaita, Papirus e breeze. O Papirus
      sozinho responde por 87% das entradas — e e justamente a width dele
      que serve ao fallback por `IconName`.

      Nao ha gordura obvia a cortar: a distribuicao por tamanho e plana
      (limitar a <= 64 px economizaria 23 MB de 918). Ver docs/ARCHITECTURE.md.

      Desligar isto faz o passo 2 da cadeia da 4.2 cair direto no glyph
      generico (passo 3) para todo app que nao esteja na tabela.
    '';
  };

  config = lib.mkIf cfg.icons.superpack {
    environment.systemPackages = [
      ronoctua.iconSuperpack
      # The mandatory terminal of the inheritance chain per the freedesktop spec.
      # The merged theme declares `Inherits=hicolor`, so it has to exist.
      pkgs.hicolor-icon-theme
    ];

    # The system's default theme for foreign apps. It does not touch our UI.
    environment.sessionVariables.XDG_ICON_THEME = "noctua";

    # GTK and Qt look the theme up by name; without this each would pick its own.
    environment.etc."xdg/gtk-3.0/settings.ini".text = ''
      [Settings]
      gtk-icon-theme-name=noctua
    '';
    environment.etc."xdg/gtk-4.0/settings.ini".text = ''
      [Settings]
      gtk-icon-theme-name=noctua
    '';
  };
}
