# One palette per theme, SOLVED from seeds — section 7.6, one wall earlier.
#
# `assets/tokens/colors.nix` holds RECIPES: every token names a primitive of a
# family. The values come out of `nix/scripts/gen-theme.py`, which runs ONCE
# here for every theme in `assets/themes/themes.nix`, gates every relationship
# the palette promises (contrast, distance, ladders, hue separation), and
# regresses `owl` against the hand-tuned palette it was derived from. A gate
# that fails stops the build with the pair and the number on screen.
#
# Nothing downstream reads the output at EVALUATION time: `nix/tokens.nix`
# takes `themes.json` as a file, the compositor's config is assembled by a
# build that `cat`s the three compositor values, Ghostty is pointed at a file, the
# extension's generator reads the flat map, and the screenshot driver loads
# the JSON when it runs. No import-from-derivation — the same rule
# `nix/compositor-binds.nix` argues for.
#
# The two scripts are copied one by one, and not the directory: a derivation
# references what its bytes cite, and `${./scripts}` would make every other
# generator in there a reason to rebuild the tokens — and, behind them, the
# whole workspace.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  # The list, from `assets/themes/themes.nix`. Passed in rather than imported
  # here so there is exactly one `import` of that file in the flake.
  themes,
  # `./assets/themes`: the seeds, and the frozen reference of a theme that has
  # one.
  themesDir,
  # `./assets/tokens/colors.nix`: the recipes.
  recipesFile,
}:

let
  # The Nix -> Python bridge is JSON, as in `nix/tokens.nix`.
  toJson = name: value: writeText name (builtins.toJSON value);

  seedArgs = lib.concatMapStringsSep " " (
    id: "--seed ${id}=${toJson "${id}.seed.json" (import (themesDir + "/${id}.nix"))}"
  ) themes;

  # THE FROZEN FIXTURE, for whichever theme carries one. Only the report reads
  # it; it is not a source of colour (its own header says so).
  withReference = builtins.filter (
    id: builtins.pathExists (themesDir + "/${id}.reference.nix")
  ) themes;
  referenceArgs = lib.concatMapStringsSep " " (
    id:
    "--reference ${id}=${toJson "${id}.reference.json" (import (themesDir + "/${id}.reference.nix"))}"
  ) withReference;
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-themes";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out generator
    cp ${./scripts/colorspace.py} generator/colorspace.py
    cp ${./scripts/gen-theme.py} generator/gen-theme.py
    python3 generator/gen-theme.py \
      --themes ${toJson "themes.json" themes} \
      ${seedArgs} \
      --recipes ${toJson "colors.json" (import recipesFile)} \
      ${referenceArgs} \
      --out $out
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "One palette per theme, solved from assets/themes/*.nix through assets/tokens/colors.nix";
    platforms = lib.platforms.all;
  };
}
