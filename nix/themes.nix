# One palette per theme, SOLVED from seeds — section 7.6, one wall earlier.
#
# `assets/tokens/colors.nix` holds RECIPES: every token names a primitive of a
# family. The values come out of `nix/scripts/gen-theme.py`, which runs ONCE
# here for every theme in `assets/themes/themes.nix`, gates every relationship
# the palette promises (contrast, distance, ladders, hue separation), and
# regresses `owl` against the hand-tuned palette it was derived from. A gate
# that fails stops the build with the pair and the number on screen.
#
# Nothing downstream reads the output at EVALUATION time: the compositor's
# config is assembled by a build that `cat`s the three compositor values,
# Ghostty is pointed at a file, the extension's generator reads the flat map,
# and the screenshot driver loads the JSON when it runs. No
# import-from-derivation — the same rule `nix/compositor-binds.nix` argues for.
#
# # The palettes are DATA, and no compilation reads them
#
# `palettes.bin` is every theme's every page's every role, as RGBA, behind a
# header: the theme ids in the chooser's order, which one is the default,
# which way each climbs (`mode`), the pages, and a hash of the role names in
# `color::NAMES` order. The binaries read it at run time from
# `RONOCTUA_THEMES` (this output's path) and refuse, naming the variable, a
# file whose roles or pages are not the ones they were compiled with
# (`ronoctua_tokens::palettes`). Before 2026-10-12 the same bytes were a table
# compiled into `ronoctua-tokens` — so a seed's hue rebuilt the workspace.
# Now the Rust is compiled against the role NAMES alone, read at evaluation
# (`nix/token-names.nix`), and this derivation is handed the same list
# (`--roles`): the generator refuses to solve a role it was not handed, or
# to leave one out, so the palettes' order IS the compiled order.
#
# The two scripts are copied one by one, and not the directory: a derivation
# references what its bytes cite, and `${./scripts}` would make every other
# generator in there a reason to solve every palette again
# (`generators-cite-their-scripts`).
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  # The list, from `assets/themes/themes.nix`. Passed in rather than imported
  # here so there is exactly one `import` of that file in the flake.
  themes,
  # WHICH OF THEM A FRESH MACHINE SHOWS, by name — the palettes' header
  # carries it, since the binaries know no theme until they read one.
  defaultTheme,
  # EVERY COLOUR ROLE, by name and in `color::NAMES` order:
  # `nix/token-names.nix`'s `roles`.
  roleNames,
  # `./assets/themes`: the seeds, and the frozen reference of a theme that has
  # one.
  themesDir,
  # `./assets/tokens/colors.nix`: the recipes.
  recipesFile,
  # A SEED EDITED, by theme id: fields laid over that seed as it was read
  # (`lib.recursiveUpdate`). Empty in every build of the flake; what
  # `colour-seeds-rebuild-no-rust` hands in to move a hue and see what is
  # built again (`tests/build-granularity.nix`).
  seedEdits ? { },
}:

let
  # The Nix -> Python bridge is JSON, as in `nix/tokens.nix`.
  toJson = name: value: writeText name (builtins.toJSON value);

  # THE FROZEN FIXTURE, for whichever theme carries one. Only the report reads
  # it; it is not a source of colour (its own header says so).
  withReference = builtins.filter (
    id: builtins.pathExists (themesDir + "/${id}.reference.nix")
  ) themes;

  # WHAT THE GENERATOR IS HANDED, by value: Nix's JSON of the recipes, of every
  # seed and of every reference. Named once here, for the arguments below and
  # for `passthru.handed`.
  handed = {
    recipes = builtins.toJSON (import recipesFile);
    seeds = lib.genAttrs themes (
      id:
      builtins.toJSON (lib.recursiveUpdate (import (themesDir + "/${id}.nix")) (seedEdits.${id} or { }))
    );
    references = lib.genAttrs withReference (
      id: builtins.toJSON (import (themesDir + "/${id}.reference.nix"))
    );
  };

  seedArgs = lib.concatMapStringsSep " " (
    id: "--seed ${id}=${writeText "${id}.seed.json" handed.seeds.${id}}"
  ) themes;
  referenceArgs = lib.concatMapStringsSep " " (
    id: "--reference ${id}=${writeText "${id}.reference.json" handed.references.${id}}"
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
      --recipes ${writeText "colors.json" handed.recipes} \
      --roles ${toJson "token-roles.json" roleNames} \
      --default ${defaultTheme} \
      ${referenceArgs} \
      --out $out
    runHook postBuild
  '';

  dontInstall = true;

  # THE SAME TEXTS, for `themes-are-data`: it looks for them in the strings of
  # every compiled derivation, so a colour's value that reached one by any
  # route but this derivation is still found. Not part of the derivation.
  passthru = { inherit handed; };

  meta = {
    description = "One palette per theme, solved from assets/themes/*.nix through assets/tokens/colors.nix";
    platforms = lib.platforms.all;
  };
}
