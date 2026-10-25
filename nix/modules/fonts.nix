# Fonts on the system — sections 6.2 and 6.4.
{
  config,
  lib,
  options,
  pkgs,
  ronoctua,
  ...
}:

let
  f = ronoctua.fonts;
  fc = ronoctua.fontconfigConf;
  # THE FULL FAMILY'S VARIANTS, joined: the part's one path.
  fullFamily = pkgs.symlinkJoin {
    name = "noctua-iosevka-full";
    paths = [
      f.noctua-iosevka-ttf
      f.noctua-iosevka-woff2
      f.noctua-iosevka-woff2-nerdfonts
    ];
  };
in
{
  options.ronoctua.fonts.full = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = ''
      Installs the whole family: 540 faces, ~4.1 GB.

      Section 6.2 says nothing is to be discarded, and that is the default. A host
      tight on disk can switch this off and keep only the patched TTF,
      which is the only one used in the interfaces. The ISO never uses this path: it
      takes noctua-iosevka-core (~62 MB). See docs/ARCHITECTURE.md.
    '';
  };

  # ── anchor: L25 ──
  # THE FULL FAMILY IS A PART (`nix/modules/parts.nix`), of data: ~2 GiB of
  # variants the full profile carries, downloaded from the fonts' Release
  # (`fontAssets` in `flake.nix`) after the installation and out of the
  # system's closure. Its faces reach the session through the parts' view,
  # whose folder the system's fontconfig names (`nix/fontconfig.nix`). The
  # system font (`noctua-iosevka-ttf-nerdfonts`) stays in the base.
  config = lib.mkMerge [
    (lib.optionalAttrs (options.ronoctua ? parts) {
      ronoctua.parts.fonts-full = {
        wanted = config.ronoctua.fonts.full;
        kind = "data";
        heavy = true;
        name = "Noctua Iosevka";
        package = fullFamily;
      };
    })
    {
      # Section 6.4: no NixOS default fonts sneaking in from behind.
      fonts.enableDefaultPackages = false;
      fonts.fontDir.enable = true;

      fonts.packages = [
        # Always: the system font, patched. Section 6.2 — it is the default in
        # modeline, minibuffer, buffers, terminal, installer and TTY.
        f.noctua-iosevka-ttf-nerdfonts
      ]
      ++ (with pkgs; [
        # --- the complete Noto, enumerated (section 6.4) --------------
        #
        # `noto-fonts` IS HERE, unconditionally, and it used not to be.
        #
        # It sat behind `ronoctua.fonts.notoExtra`, which the minimal profile and
        # the dev VM turned off, and the option's own text said why: "the single
        # largest item in the font closure (over 1 GB on its own)". MEASURED with
        # `nix path-info -S`, against the pinned nixpkgs: **49,1 MB**. Wrong by
        # twenty times.
        #
        # The number aged when `noto-fonts-extra` was absorbed into `noto-fonts`
        # in nixpkgs 26.05 — the same rename already noted in AGENTS.md, arriving
        # as a stale measurement rather than as an evaluation error. Nobody
        # re-measured, and a profile went without Arabic, Hebrew, Devanagari and
        # Thai to save fifty megabytes it was told were a thousand.
        #
        # Two hundred and twenty-nine files, and what they carry is every writing
        # system a foreign application might put on this screen. The owner's
        # decision, once the real number was on the table: stop avoiding it.
        noto-fonts
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
      ]);

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
      # but the console wants PSF/PSFU of 256 or 512 glyphs (any other count is
      # refused, and the console keeps the kernel's font). We generate a PSF2 from
      # the real file, which is what preserves the intent of the rule; which of
      # its two sizes a console wears, and when, is `nix/modules/console.nix`'s.
      # -----------------------------------------------------------------
      # From the OPTION, for the same reason as the compositor's: the console
      # answering in a different layout from the session is worse than both
      # being wrong, because only one of them gets noticed.
      console.keyMap = lib.mkDefault config.ronoctua.keyboard.console;
    }
  ];
}
