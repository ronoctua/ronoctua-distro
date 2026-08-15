# Fonts on the system — sections 6.2 and 6.4.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  f = ronoctua.fonts;
  fc = ronoctua.fontconfigConf;
in
{
  options.ronoctua.fonts.full = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = ''
      Instala a familia inteira: 540 faces, ~4,1 GB.

      A secao 6.2 manda que nada seja descartado, e esse e o padrao. Um host
      apertado de disco pode desligar isto e ficar so com a TTF patcheada,
      que e a unica usada nas interfaces. O ISO nunca usa este caminho: ele
      leva noctua-iosevka-core (~62 MB). Ver docs/ARCHITECTURE.md.
    '';
  };

  options.ronoctua.fonts.notoExtra = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = ''
      Instala a coverage Noto ampla. E o maior item isolado do closure de
      fontes (passa de 1 GB sozinho). Removivel sem tocar no desenho.

      O package e `noto-fonts`: no nixpkgs 26.05 o `noto-fonts-extra` foi
      absorvido por ele, e o alias RENOMEADO derruba a avaliacao em vez de
      avisar — a mesma classe das others renomeacoes ja anotadas no
      CLAUDE.md.
    '';
  };

  config = {
    # Section 6.4: no NixOS default fonts sneaking in from behind.
    fonts.enableDefaultPackages = false;
    fonts.fontDir.enable = true;

    fonts.packages = [
      # Always: the system font, patched. Section 6.2 — it is the default in
      # modeline, minibuffer, buffers, terminal, installer and TTY.
      f.noctua-iosevka-ttf-nerdfonts
    ]
    ++ lib.optionals config.ronoctua.fonts.full [
      f.noctua-iosevka-ttf
      f.noctua-iosevka-woff2
      f.noctua-iosevka-woff2-nerdfonts
    ]
    ++ (with pkgs; [
      # --- the complete Noto, enumerated (section 6.4) --------------
      #
      # `noto-fonts` IS NOT HERE, and its absence is the point: it is listed at
      # the bottom, under `ronoctua.fonts.notoExtra`. It used to be in both
      # places, which made the switch a dead one — `vm.nix` turned it off
      # believing it saved a gigabyte, and the package came in through this list
      # anyway. A switch that does nothing is worse than no switch: it is a
      # measurement everybody trusts and nobody takes.
      noto-fonts-cjk-sans
      noto-fonts-cjk-serif
      # Section 6.4 lists "noto-fonts-emoji"; in nixpkgs 26.05 that name
      # became an alias for noto-fonts-color-emoji. We use the canonical one so as
      # not to depend on an alias, which may vanish in a future release.
      noto-fonts-color-emoji

      # --- extras, with justification (section 6.4) ----------------
      # Arial/Times/Courier metrics, for legacy apps that hard-code
      # those names.
      liberation_ttf
      # The most assumed fallback target on Linux; it covers basic symbols
      # where Noto falls short.
      dejavu_fonts
      # Real mathematics, with a MATH table. Noto Math is a symbol
      # repertoire, not a maths typesetting font.
      stix-two
    ])
    ++ lib.optional config.ronoctua.fonts.notoExtra pkgs.noto-fonts;

    fonts.fontconfig = {
      enable = true;
      # Section 6.4: the patched noctua-iosevka FIRST in all three generics.
      # The list comes from nix/fontconfig.nix, the same source that feeds the
      # renderer's chain — the two cannot diverge.
      defaultFonts = {
        inherit (fc.chain)
          monospace
          sansSerif
          serif
          emoji
          ;
      };
      # Hinting and antialiasing come from our .conf, not from the defaults.
      antialias = true;
      hinting.enable = true;
      hinting.style = "slight";

      # The generated snippet, with the legacy aliases and the spacing rules.
      #
      # It goes through `localConf`, NOT through `environment.etc`. The NixOS
      # fontconfig module manages the whole of /etc/fonts/conf.d as a read-only
      # link into the store; trying to slip a file in there via environment.etc
      # fails the build with "ln: ... Permission denied", and the message does not
      # say why.
      localConf = fc.conf;
    };

    # The chain as data, for the renderer to read at runtime when it is not
    # embedded in the binary (the installer and minimal environments use the
    # embedded one).
    environment.etc."ronoctua/fallback-chain.json".source = fc.chainJson;

    # -----------------------------------------------------------------
    # TTY — the Linux console does not consume TTF.
    #
    # Section 6.2 says TTF-nerdfonts is the default "everywhere... TTY",
    # but the console wants PSF/PSFU with at most 512 glyphs. We generate a PSF2
    # from the real file, which is what preserves the intent of the rule.
    # -----------------------------------------------------------------
    console = {
      earlySetup = true;
      font = "${f.noctua-console-psf}/share/consolefonts/noctua-iosevka-32.psf";
      # From the OPTION, for the same reason as the compositor's: the console
      # answering in a different layout from the session is worse than both
      # being wrong, because only one of them gets noticed.
      keyMap = lib.mkDefault config.ronoctua.keyboard.console;
    };
  };
}
