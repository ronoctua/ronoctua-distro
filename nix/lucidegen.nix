# The Lucide icon browser's database.
#
# # The two sources, and why there are two
#
#     lucide-icons-<version>.zip  the SET: name, tags, categories,
#                                 aliases and each icon's drawing
#     pkgs.lucide                 the FONT, which is the only way to put the
#                                 icons on screen (the renderer draws glyphs)
#
# The zip is pinned by hash and does not move; `pkgs.lucide` moves along with
# nixpkgs. That is why the version guard points at IT — it is the half that moves.
#
# The real guard, however, is not the `assert`: it is the generator requiring that
# the font's glyph names and the zip's icon names be the SAME set. An
# `assert` on a string checks what someone typed; comparing sets
# checks what the two things ARE.
#
# # Why the font is remapped
#
# Lucide lives at `U+E038..U+E6BB` and the project's Iosevka is patched with
# Nerd Fonts, which occupies 634 of those codepoints. The primary face wins the
# fallback chain, so 38% of the icons would come out drawn as ANOTHER symbol —
# silent fallback at its worst, because there is no tofu to give it away. See
# `gen-lucidedb.py`.
{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
  python3,
  lucide,
  fonts,
  noto-fonts-color-emoji,
  lexiconFile,
  # The languages, from `assets/i18n/languages.nix`. The tags are built
  # for every one of them, keyed on the subtag.
  languages,
}:

let
  # THE EXPECTED VERSION, WRITTEN BY HAND — same reason as `unicodegen.nix`: taking
  # it from `lib.getVersion lucide` would be a guard that follows the package instead
  # of checking it, and a `nix flake update` would swap the 1,668 icons in silence.
  upstreamVersion = "0.563.0";

  fromPackage = lib.getVersion lucide;

  # WHERE THE ICONS COME TO LIVE — plan 15, out of reach of Nerd Fonts
  # (which goes up to `U+F1AF0`) and of NotoColorEmoji. The generator CHECKS that the
  # whole range is free in both before writing anything at all.
  base = "F2000";

  icons = fetchurl {
    url = "https://github.com/lucide-icons/lucide/releases/download/${upstreamVersion}/lucide-icons-${upstreamVersion}.zip";
    hash = "sha256-iLuxXrVPjDw43+4px1uZQwrVlyzTwKMI3SxdgcJHElU=";
  };

  py = python3.withPackages (ps: [ ps.fonttools ]);

  mkGen =
    {
      name,
      declaredBase ? base,
      expectFailure ? false,
    }:
    stdenvNoCC.mkDerivation {
      inherit name;
      nativeBuildInputs = [
        py
        unzip
      ];
      dontUnpack = true;
      dontConfigure = true;

      buildPhase =
        let
          # The scripts' DIRECTORY, and not the file: the generator imports
          # `lexicon.py`, the module the three generators share.
          cmd = ''
            unzip -q ${icons} -d set_of
            gen() {
              python3 ${./scripts}/gen-lucidedb.py \
                --icons set_of/icons \
                --font ${lucide}/share/fonts/truetype/Lucide.ttf \
                --lexicon ${lexiconFile} \
                --languages ${lib.escapeShellArgs languages} \
                --other ${fonts.noctua-iosevka-core}/share/fonts/truetype/noctua-iosevka/NoctuaIosevkaNerdFontMono-Regular.ttf \
                --other ${noto-fonts-color-emoji}/share/fonts/noto/NotoColorEmoji.ttf \
                --base ${declaredBase} \
                --min-coverage 90.0 \
                --cap-no-pt 0 \
                --out "$1"
            }
          '';
        in
        if expectFailure then
          ''
            runHook preBuild
            mkdir -p $out
            echo "### negative fixture: the generator MUST fail ###"

            # NO pipe: in `cmd | tee` the exit code is `tee`'s, which
            # is always 0 — the test would report success with the generator failing
            # correctly. It has already happened in `glyphgen.nix`.
            ${cmd}
            set +e
            gen "$out" > $out/out_text.txt 2>&1
            rc=$?
            set -e
            cat $out/out_text.txt

            if [ "$rc" -eq 0 ]; then
              echo "ERROR: the generator ACCEPTED the range that collides with Nerd Fonts." >&2
              exit 1
            fi
            echo "OK: the generator refused, as it should (code $rc)"
            runHook postBuild
          ''
        else
          ''
            runHook preBuild
            ${cmd}
            gen $out
            runHook postBuild
          '';

      dontInstall = true;

      meta = {
        description = "Lucide icon browser database";
        platforms = lib.platforms.all;
      };
    };
in
assert lib.assertMsg (fromPackage == upstreamVersion) ''
  nixpkgs' lucide is at ${fromPackage}, and nix/lucidegen.nix declares ${upstreamVersion}.

  The font and the icon set have to come from the SAME version: raising one without the
  other makes the generator stop at the name comparison, which is where it has to stop.
  Raise both deliberately — the zip's URL also carries the version, and the hash with it.
'';
{
  inherit upstreamVersion base;

  db = mkGen { name = "ronoctua-lucidedb"; };

  # THE NEGATIVE FIXTURE: Lucide's SOURCE range, the one that collides.
  #
  # It is not hypothetical — it is the measured defect. Without the guard, 634 of the
  # 1,668 icons would draw the Nerd Fonts glyph occupying the same codepoint, and the
  # screen would have no way to give it away: out would come a pretty, wrong icon.
  rangeTakenFails = mkGen {
    name = "ronoctua-lucidedb-negative-fixture";
    declaredBase = "E038";
    expectFailure = true;
  };
}
