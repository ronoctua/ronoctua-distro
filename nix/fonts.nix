# Font layer — section 6 of the bootstrap document.
#
# Five public derivations, not one. A single 4.1 GB package would enter the
# closure of every machine AND of the ISO, and an ISO does not close with 2.1 GB
# of font. See docs/ARCHITECTURE.md, section "Fontes".
{
  lib,
  stdenvNoCC,
  python3,
  symlinkJoin,
  writeText,
  zstd,
  # Folder name → the `.tar.zst` that carries it, one per Release asset. Each
  # tarball holds a single top-level directory with that same name.
  fontAssets,
  # The SIL Open Font License, redistributed alongside every face. The faces are
  # a derivative of Iosevka (OFL, Reserved Font Name) — this repository's own
  # code is MIT, and the two boundaries have to stay legible.
  oflText ? ../LICENSES/LICENSE-OFL.txt,
}:

let
  # THE TOKEN FILE IS THE TRUTH about every size and family, and this reads it
  # rather than repeating a number (invariant 9). Only `size.terminalPoints`
  # leaves here — see `sizes` at the bottom.
  typography = import ../assets/tokens/typography.nix;

  # fonttools to rewrite name tables; brotli to read/write WOFF2;
  # freetype-py to rasterise the console's PSF.
  py = python3.withPackages (ps: [
    ps.fonttools
    ps.brotli
    ps.zopfli
    ps.freetype-py
  ]);

  familyNF = "NoctuaIosevka Nerd Font Mono";
  familyPlain = "Noctua Iosevka";

  # The four folders. `fix` = normalises the naming; `inventory` = only
  # checks and reports, without touching the bytes.
  #
  # WHY THE WOFF2 FILES ARE NOT FIXED
  # ---------------------------------
  # The 27-face defect (see scripts/fix-font-names.py) gets in the way of
  # MATCHING by family name — fontconfig and fontdb. None of that consumes
  # WOFF2: fontconfig does not index .woff2, and a browser loads through
  # `@font-face { src: url(...) }` with the `font-family` declared by the CSS
  # author. The internal name table is inert there.
  #
  # Rewriting a WOFF2 costs ~49 s, because fontTools recompresses the whole
  # font with brotli (there are 57 thousand glyphs). The 270 files would take 3.7
  # hours to change no behaviour at all. So they pass through untouched, which
  # has the extra advantage of preserving the source bytes.
  variants = {
    "TTF" = {
      family = familyPlain;
      mode = "fix";
    };
    "TTF-nerdfonts" = {
      family = familyNF;
      mode = "fix";
    };
    "WOFF2" = {
      family = familyPlain;
      mode = "inventory";
    };
    "WOFF2-nerdfonts" = {
      family = familyNF;
      mode = "inventory";
    };
  };

  # One asset per variant, and no spare. An asset without a variant would be
  # downloaded and never fixed; a variant without an asset would send
  # `fix-font-names.py` at a directory that does not exist, and the message it
  # prints then talks about faces, not about a missing download.
  assets =
    let
      wanted = lib.attrNames variants;
      given = lib.attrNames fontAssets;
    in
    lib.throwIf (wanted != given) ''
      nix/fonts.nix: `fontAssets` and `variants` diverge.
        variants:   ${lib.concatStringsSep ", " wanted}
        fontAssets: ${lib.concatStringsSep ", " given}
    '' fontAssets;

  # The four normal-width RIBBI faces — the subset that goes on the ISO,
  # in the initrd and in the installer (~62 MB against 2.1 GB).
  coreFaces = [
    "NoctuaIosevkaNerdFontMono-Regular.ttf"
    "NoctuaIosevkaNerdFontMono-Bold.ttf"
    "NoctuaIosevkaNerdFontMono-Italic.ttf"
    "NoctuaIosevkaNerdFontMono-BoldItalic.ttf"
  ];

  meta = {
    description = "ronoctua-distro's customised Iosevka";
    homepage = "https://github.com/be5invis/Iosevka";
    license = lib.licenses.ofl;
    platforms = lib.platforms.all;
  };

  # ---------------------------------------------------------------------
  # Internal derivation: normalises the naming of the 540 faces.
  #
  # The Nerd Fonts patcher throws 27 of the 135 patched faces into nine
  # invented typographic families. See nix/scripts/fix-font-names.py
  # for the full diagnostics and for the second failure that fixing this
  # naively would create.
  # ---------------------------------------------------------------------
  corrected = stdenvNoCC.mkDerivation {
    pname = "noctua-iosevka-corrected";
    version = "34.7.0+nf3.4.0";

    # THE FOUR ASSETS ARE UNPACKED STRAIGHT INTO THE TREE WE FIX.
    #
    # Not into a source derivation that this one would then copy: the faces are
    # 4.1 GB uncompressed, and an intermediate tree would mean carrying that
    # twice in the store to change nothing. Each tarball already holds one
    # top-level directory named after the folder, so extracting all four side by
    # side reassembles `noctua-iosevka/` exactly as the fonts repository had it.
    srcs = lib.attrValues assets;

    nativeBuildInputs = [
      py
      zstd
    ];

    dontConfigure = true;

    # No `sourceRoot`: the `cd` into it lives inside stdenv's DEFAULT
    # `unpackPhase`, which this one replaces. The build stays in its own
    # directory, which is where `noctua-iosevka/` was just created.
    unpackPhase = ''
      runHook preUnpack
      mkdir -p noctua-iosevka
      ${lib.concatStringsSep "\n" (
        lib.mapAttrsToList (dir: asset: ''
          echo "--- extracting ${dir} ---"
          tar --zstd -xf ${asset} -C noctua-iosevka
          test -d "noctua-iosevka/${dir}" || {
            echo "ERROR: ${builtins.baseNameOf asset} did not bring the '${dir}' directory." >&2
            echo "      The Release asset changed shape; rebuild it with" >&2
            echo "      nix run .#fonts-release e atualize o flake.lock." >&2
            exit 1
          }
        '') assets
      )}
      runHook postUnpack
    '';

    buildPhase = ''
      runHook preBuild

      # We fix them IN PLACE, in the tree unpackPhase just extracted. An extra
      # `cp -r` here would cost another 4.1 GB inside the build directory.
      chmod -R u+w noctua-iosevka
      mkdir -p reports

      ${lib.concatStringsSep "\n" (
        lib.mapAttrsToList (dir: v: ''
          echo "--- ${dir} (${v.mode}) ---"
          python3 ${./scripts/fix-font-names.py} "noctua-iosevka/${dir}" \
            --family ${lib.escapeShellArg v.family} \
            --expect-count 135 \
            ${lib.optionalString (v.mode == "inventory") "--inventory"} \
            --report "reports/${dir}.json"
        '') variants
      )}

      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      mkdir -p $out
      # --reflink=auto: with the build dir on the same btrfs as the store, the copy
      # comes out almost free. On another filesystem it degrades to a normal
      # copy without breaking.
      cp -r --reflink=auto noctua-iosevka/* $out/
      cp -r reports $out/reports
      # The OFL text comes from THIS repository, not from the assets.
      #
      # The assets are 4.1 GB of build output: they carry faces and nothing
      # else. A licence that has to travel with the fonts cannot live inside the
      # artefact it licenses — it lives in git, where it is versioned and where
      # `fonts-license` can see it.
      cp ${oflText} $out/LICENSE-OFL.txt
      runHook postInstall
    '';

    inherit meta;
  };

  # ---------------------------------------------------------------------
  # Generic installer for a subset of faces.
  # ---------------------------------------------------------------------
  mkFontPkg =
    {
      pname,
      dir,
      # where fontconfig looks: truetype/ or woff2/
      target,
      # if null, installs the whole folder
      only ? null,
      description,
    }:
    stdenvNoCC.mkDerivation {
      inherit pname;
      version = "34.7.0+nf3.4.0";
      src = corrected;
      dontConfigure = true;
      dontBuild = true;
      installPhase = ''
        runHook preInstall
        install -Dm444 -t $out/share/fonts/${target}/noctua-iosevka \
          ${
            if only == null then "$src/${dir}/*" else lib.concatMapStringsSep " " (f: "$src/${dir}/${f}") only
          }
        install -Dm444 ${oflText} \
          $out/share/doc/noctua-iosevka/LICENSE-OFL.txt
        runHook postInstall
      '';
      meta = meta // {
        inherit description;
      };
    };

  noctua-iosevka-ttf-nerdfonts = mkFontPkg {
    pname = "noctua-iosevka-ttf-nerdfonts";
    dir = "TTF-nerdfonts";
    target = "truetype";
    description = "noctua-iosevka, 135 TTF faces patched with Nerd Fonts — the system font";
  };

  noctua-iosevka-ttf = mkFontPkg {
    pname = "noctua-iosevka-ttf";
    dir = "TTF";
    target = "truetype";
    description = "noctua-iosevka, 135 unpatched TTF faces";
  };

  noctua-iosevka-woff2-nerdfonts = mkFontPkg {
    pname = "noctua-iosevka-woff2-nerdfonts";
    dir = "WOFF2-nerdfonts";
    target = "woff2";
    description = "noctua-iosevka, 135 patched WOFF2 faces — local web content";
  };

  noctua-iosevka-woff2 = mkFontPkg {
    pname = "noctua-iosevka-woff2";
    dir = "WOFF2";
    target = "woff2";
    description = "noctua-iosevka, 135 unpatched WOFF2 faces";
  };

  # The ISO's subset. Section 6.2 says "nothing is discarded", and that holds
  # for the installed system; an installable ISO does not close with 2.1 GB of
  # font. See docs/ARCHITECTURE.md.
  noctua-iosevka-core = mkFontPkg {
    pname = "noctua-iosevka-core";
    dir = "TTF-nerdfonts";
    target = "truetype";
    only = coreFaces;
    description = "noctua-iosevka, 4 RIBBI faces — ISO, initrd, TTY and installer";
  };

  # ---------------------------------------------------------------------
  # Console font. Section 6.2 demands TTF-nerdfonts be the default "everywhere...
  # TTY", but the Linux console does NOT consume TTF: it needs
  # PSF/PSFU of 256 or 512 glyphs. We generate a PSF2 from the real file,
  # which is what preserves the intent of the rule.
  # ---------------------------------------------------------------------
  noctua-console-psf = stdenvNoCC.mkDerivation {
    pname = "noctua-console-psf";
    version = "34.7.0+nf3.4.0";
    src = corrected;
    nativeBuildInputs = [ py ];
    dontConfigure = true;
    buildPhase = ''
      runHook preBuild
      mkdir -p $out/share/consolefonts
      for size in 16 32; do
        python3 ${./scripts/make-console-psf.py} \
          "$src/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf" \
          --height "$size" \
          --output "$out/share/consolefonts/noctua-iosevka-$size.psf"
      done
      runHook postBuild
    '';
    dontInstall = true;
    meta = meta // {
      description = "PSF2 console font generated from the real noctua-iosevka";
    };
  };

  noctua-iosevka = symlinkJoin {
    name = "noctua-iosevka";
    paths = [
      noctua-iosevka-ttf-nerdfonts
      noctua-iosevka-ttf
      noctua-iosevka-woff2-nerdfonts
      noctua-iosevka-woff2
    ];
    meta = meta // {
      description = "noctua-iosevka complete — 540 faces, all four variants";
    };
  };
in
{
  inherit
    corrected
    noctua-iosevka
    noctua-iosevka-core
    noctua-iosevka-ttf
    noctua-iosevka-ttf-nerdfonts
    noctua-iosevka-woff2
    noctua-iosevka-woff2-nerdfonts
    noctua-console-psf
    ;

  # THE CONSOLE'S FONT, the one file every console loads — the ISO's, every
  # machine's (`nix/modules/fonts.nix`) and the VM tests that look at one —,
  # named once.
  consoleFont = "${noctua-console-psf}/share/consolefonts/noctua-iosevka-32.psf";

  # Consumed by nix/fontconfig.nix and by the modules. No crate names the
  # font directly (section 6.2): always through a token.
  families = {
    mono = familyNF;
    ui = familyNF;
    plain = familyPlain;
  };

  # The sizes a MODULE needs, straight out of the token file.
  #
  # Only the foreign terminal's, and only because Ghostty takes a size in its
  # own config: everything this system draws itself asks
  # `ronoctua_tokens::text::size()` at runtime instead. Read from the token so
  # `nix/modules/terminal.nix` and `toolkits::tell_them` cannot drift apart —
  # a terminal at a different size from everything around it is exactly the
  # defect this feature exists to remove.
  sizes = {
    inherit (typography.size) terminalPoints;
  };

  inherit coreFaces;
}
