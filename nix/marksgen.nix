# The face that holds the marks this repository DRAWS — section 6.3.
#
# # Why a fourth face exists
#
# Because a mark the interface has to draw existed in none of the five fonts,
# and the two invariants that meet there leave one way out. Invariant 10: a UI
# icon is a glyph of a font, and no design system component consults a pixmap.
# Invariant 9: a codepoint is a token, resolved at build time against a real
# cmap. So the mark is drawn INTO a face of ours, declared in
# `assets/icons/marks.nix`, and from there it is an icon like every other.
#
# # Why it is a face of its own and not a glyph added to the Iosevka
#
# Because the Iosevka here is 540 faces, 4.1 GB of Release asset, each with its
# own metrics. Adding a glyph would mean rewriting every one of them to change
# one drawing — and the next mark would do it again. A separate face is what
# `nix/lucidegen.nix` already does for the same class of reason, and the
# renderer already carries three.
#
# # The range, and the defect behind it
#
# `U+F3000`: plane 15, past Nerd Fonts (which stops at `U+F1AF0`) and past
# Lucide (remapped to `U+F2000` by `nix/lucidegen.nix`). The face asked for by
# NAME wins the fallback chain, so a mark sharing a codepoint with a Nerd Fonts
# glyph would be drawn as ANOTHER symbol — not tofu, which one sees, but a
# pretty and wrong icon. The generator CHECKS the range is free in all three
# other faces before writing anything, and the negative fixture below proves
# the check by handing it a range that is not.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  fonts,
  lucidedb,
  noto-fonts-color-emoji,
  # Explicit file, NOT a directory — see `nix/glyphgen.nix` for why.
  marksFile,
}:

let
  py = python3.withPackages (ps: [ ps.fonttools ]);

  # The marks live in Nix (source of truth, section 7.6) and the generator is
  # Python; the bridge is JSON, as it is for the tokens.
  marksJson = writeText "marks.json" (builtins.toJSON (import marksFile));

  base = "F3000";

  nerdFont = "${fonts.corrected}/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf";
  emojiFont = "${noto-fonts-color-emoji}/share/fonts/noto/NotoColorEmoji.ttf";

  # THE GLYPH A MARK IS MEASURED AGAINST: `md-dots_grid`, which is the glyph
  # the Tailscale mark replaced.
  #
  # Measured over the 6896 `md-` glyphs of the built face: the ADVANCE is 600
  # for every one of them, and it is what decides how many cells a glyph takes.
  # The ink box is not constant — 747 distinct boxes, the commonest
  # `(0, 40, 600, 640)` on 1992 of them, and `md-battery` at
  # `(28, -114, 572, 794)`. So the generator requires the advance to MATCH and
  # the ink to FIT INSIDE this glyph's box: a mark may be smaller than its
  # neighbours, never wider than the cell.
  reference = "F15FC";
  referenceIs = "md-dots_grid";

  mkGen =
    {
      name,
      declaredBase ? base,
      expectFailure ? false,
    }:
    stdenvNoCC.mkDerivation {
      inherit name;
      nativeBuildInputs = [ py ];
      dontUnpack = true;
      dontConfigure = true;

      buildPhase =
        let
          # A shell FUNCTION and not a command string, for the reason written
          # in `nix/glyphgen.nix`: interpolating a multi-line string and then
          # appending `> file` puts the redirection on a line of its own.
          cmd = ''
            gen() {
              python3 ${./scripts}/gen-marksdb.py \
                --marks ${marksJson} \
                --like ${nerdFont} \
                --reference ${reference} \
                --reference-is ${referenceIs} \
                --other ${nerdFont} \
                --other ${emojiFont} \
                --other ${lucidedb.db}/NoctuaLucide.ttf \
                --base ${declaredBase} \
                --out "$1"
            }
          '';
        in
        if expectFailure then
          ''
            runHook preBuild
            mkdir -p $out
            echo "### negative fixture: the generator MUST fail ###"

            # NO pipe: in `cmd | tee` the exit code is `tee`'s, which is always
            # 0 — the test would report success with the generator refusing
            # correctly. It has already happened in `glyphgen.nix`.
            ${cmd}
            set +e
            gen "$out" > $out/out_text.txt 2>&1
            rc=$?
            set -e
            cat $out/out_text.txt

            if [ "$rc" -eq 0 ]; then
              echo "ERROR: the generator ACCEPTED a range that collides." >&2
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
        description = "The face holding the marks this repository draws";
        platforms = lib.platforms.all;
      };
    };
in
{
  inherit base reference referenceIs;

  db = mkGen { name = "ronoctua-marksdb"; };

  # THE NEGATIVE FIXTURE: a range inside Nerd Fonts' own, which is exactly the
  # collision this face exists to avoid. Without the guard the mark would draw
  # somebody else's icon and nothing on screen would give it away.
  rangeTakenFails = mkGen {
    name = "ronoctua-marksdb-negative-fixture";
    declaredBase = "F0317";
    expectFailure = true;
  };
}
