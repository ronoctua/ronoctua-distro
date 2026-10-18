# Generated glyph layer — sections 6.3 and 6.5.
#
# WHY fetchurl AND NOT A FLAKE INPUT
# ----------------------------------
# The ryanoasis/nerd-fonts repository is several GB: it versions every
# patched font. We need ONE file, `glyphnames.json`, at
# 524 KB. Pinning the whole repository as an input would cost minutes and gigabytes
# on every lock resolution, and the cost would repeat on every machine that
# built the distro.
#
# THE TWO AUTHORITIES
# -------------------
#   glyphnames.json  -> authority on the NAMES  (fa-folder, md-cog)
#   the .ttf file    -> authority on what EXISTS (the real codepoints)
#
# They have to come from the SAME version of Nerd Fonts, otherwise the PUA
# codepoints shift silently on an update — the trap section 6.3 names.
{
  lib,
  stdenvNoCC,
  python3,
  fetchurl,
  writeText,
  fonts,
  # Explicit files, NOT directories: `import "${dir}/x.nix"` interpolates the
  # path, copies the whole directory into the store and fails with "path is not
  # valid" when the store is collected. A file path is direct and stable.
  iconTokensFile,
  # The base and icon sizes: the chrome size of every token is MEASURED here,
  # against the real face — see `chrome_size` in the generator.
  typographyFile,
  trayTableFile,
  lexiconFile,
  # The languages, from `assets/i18n/languages.nix`. The tags are built
  # for every one of them, keyed on the subtag.
  languages,
  overlayFile,
  # The face holding the marks this repository DRAWS, and the table naming
  # them. A token whose glyph starts with `ours-` resolves against these two
  # instead of against `glyphnames.json` and the Nerd Font — and is checked
  # just as hard, against the real cmap of the real face.
  marksdb,
}:

# THE BRAIN MODULES' GLYPHS ARE NOT HERE. A module's mark is data the footer
# reads at run time (`nix/brain-catalogue.nix`), so its glyph is measured by
# `measure` below — the same code, over the module's tokens alone — and never
# joins this table: the table is compiled into every crate that draws
# (`nix/tokens.nix`), and a person's module would rebuild them all.

let
  # It has to match the font's `nameID 5` AND the JSON's own METADATA.version.
  # The generator checks all three and fails if they diverge.
  version = "3.4.0";

  glyphnames = fetchurl {
    url = "https://raw.githubusercontent.com/ryanoasis/nerd-fonts/v${version}/glyphnames.json";
    hash = "sha256-4tENI/W/8L1vBnbpsB2XifzcZW3ntJiilVwncW6kQ5w=";
  };

  py = python3.withPackages (ps: [ ps.fonttools ]);

  # The tokens live in Nix (source of truth, section 7.6) but the generator is
  # Python. The bridge is JSON, generated here — there is no Nix parser in Python, and
  # writing one would be reimplementing Nix logic (invariant 5).
  iconTokensJson = writeText "icon-tokens.json" (builtins.toJSON (import iconTokensFile));
  trayJson = writeText "tray-glyphs.json" (builtins.toJSON (import trayTableFile));
  typographyJson = writeText "typography.json" (builtins.toJSON (import typographyFile));

  mkGen =
    {
      name,
      tokens,
      expectFailure ? false,
      # ONLY THE ICON TABLE (`icons.json`) of `tokens`, measured: the brain
      # modules' glyphs (`measure`). No glyph index and no coverage — those are
      # the glyph browser's, and the chrome's table already pays for them.
      iconsOnly ? false,
    }:
    stdenvNoCC.mkDerivation {
      inherit name;
      nativeBuildInputs = [ py ];
      dontUnpack = true;
      dontConfigure = true;

      buildPhase =
        let
          # A shell FUNCTION, not a command string. Interpolating a
          # multi-line string and then appending `> file` puts the redirection
          # on a SEPARATE line — it does not apply to the command, and `$?` ends up
          # capturing the empty line instead of the python. It cost a build.
          cmd = ''
            # THE SCRIPT AND ITS ONE SIBLING, copied by FILE: the generator
            # imports `lexicon.py`, so the two sit side by side here — and not
            # the whole `nix/scripts`, whose every other generator would then be
            # a reason to build this table again (`generators-cite-their-scripts`).
            mkdir -p generator
            cp ${./scripts/gen-glyphdb.py} generator/gen-glyphdb.py
            cp ${./scripts/lexicon.py} generator/lexicon.py
            gen() {
              python3 generator/gen-glyphdb.py \
                --font ${fonts.corrected}/TTF-nerdfonts/NoctuaIosevkaNerdFontMono-Regular.ttf \
                --font-unpatched ${fonts.corrected}/TTF/NoctuaIosevka-Regular.ttf \
                --glyphnames ${glyphnames} \
                --tokens ${tokens} \
                ${
                  if iconsOnly then
                    "--icons-only"
                  else
                    "--tray ${trayJson} --lexicon ${lexiconFile} --languages ${lib.escapeShellArgs languages} --overlay ${overlayFile}"
                } \
                --typography ${typographyJson} \
                --marks ${marksdb.db}/marks.json \
                --marks-font ${marksdb.db}/NoctuaMarks.ttf \
                --nf-version ${version} \
                --out "$1"
            }
          '';
        in
        if expectFailure then
          ''
            runHook preBuild
            mkdir -p $out
            echo "### negative fixture: the generator MUST fail ###"

            # NO pipe here, on purpose. In `cmd | tee`, the pipeline's exit
            # code is `tee`'s, which is always 0 — so the test
            # reported success even when the generator failed correctly.
            ${cmd}
            set +e
            gen "$out" > $out/out_text.txt 2>&1
            rc=$?
            set -e
            cat $out/out_text.txt

            if [ $rc -eq 0 ]; then
              echo "" >&2
              echo "ERROR: the generator ACCEPTED an icon token that does not exist." >&2
              echo "      Section 6.3's guard is not working, and a" >&2
              echo "      wrong codepoint would go unnoticed." >&2
              exit 1
            fi
            grep -q "icon tokens that do not resolve" $out/out_text.txt \
              || { echo "ERROR: it failed, but not for the expected reason" >&2; exit 1; }
            echo "OK: a token that does not exist takes the build down, as it should"
            runHook postBuild
          ''
        else
          ''
            runHook preBuild
            ${cmd}
            gen "$out"
            runHook postBuild
          '';

      dontInstall = true;

      meta = {
        description =
          if iconsOnly then
            "Glyph tokens measured against the real font file, outside the chrome's table"
          else
            "Icon token table and glyph database, generated from the real font file";
        platforms = lib.platforms.all;
      };
    };

  # A token that exists nowhere — section 6.3's negative fixture.
  badTokens = writeText "icon-tokens-bad.json" (
    builtins.toJSON ((import iconTokensFile) // { negative-fixture = "md-this-glyph-does-not-exist"; })
  );

  # AND THE SAME FOR A MARK THIS REPOSITORY DRAWS.
  #
  # `ours-` resolves down a different branch of `build_icon_table` — against
  # `marks.json` and the marks face, not against `glyphnames.json` and the Nerd
  # Font — and the fixture above exercises only the other one. A whole
  # resolution path with no negative fixture is exactly what that fixture's own
  # comment says must not exist.
  badMark = writeText "icon-tokens-bad-mark.json" (
    builtins.toJSON ((import iconTokensFile) // { negative-fixture = "ours-this-mark-is-not-drawn"; })
  );
in
{
  inherit version glyphnames;

  # The real output: icons.json, glyphs.json, coverage.md.
  db = mkGen {
    name = "ronoctua-glyphdb";
    tokens = iconTokensJson;
  };

  # Proves the guard works. Without this, "the build fails if the codepoint does
  # not exist" is an untested claim.
  negativeFixture = mkGen {
    name = "ronoctua-glyphdb-negative-fixture";
    tokens = badTokens;
    expectFailure = true;
  };

  # The same, down the `ours-` branch.
  negativeMarkFixture = mkGen {
    name = "ronoctua-glyphdb-negative-mark-fixture";
    tokens = badMark;
    expectFailure = true;
  };

  # GLYPH TOKENS MEASURED OUTSIDE THE CHROME'S TABLE: `{ <token> = "<glyph
  # name>"; }` -> `icons.json` holding those tokens alone, each resolved and
  # measured exactly as a chrome token is (codepoint, the size beside the
  # text, the ink across). What the brain's catalogue asks for its modules'
  # marks (`nix/brain-catalogue.nix`); a token that does not resolve stops the
  # build, as one of `assets/tokens/icons.nix` does.
  measure =
    tokens:
    mkGen {
      name = "ronoctua-measured-glyphs";
      tokens = writeText "measured-tokens.json" (builtins.toJSON tokens);
      iconsOnly = true;
    };
}
