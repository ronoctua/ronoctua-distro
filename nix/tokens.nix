# Design system tokens, from Nix to Rust — sections 7.6 and 7.8.
#
# "Tokens defined ONCE in Nix; the build generates (a) the shell's Rust table
# and (b) a WIT enum of valid names."
#
# (a) is this derivation, `tokens.rs`; (b) is written at evaluation by
# `nix/token-names.nix` from the SAME names this one is handed. Two lists
# would diverge, and the divergence would show up as the wrong colour on
# screen — not as an error; `invalid-token-fails-build` holds them to one.
#
# NO COLOUR VALUE COMES IN. The roles are names read at evaluation, and the
# bytes are the palettes `nix/themes.nix` writes (`palettes.bin`), read by the
# binaries at run time (`RONOCTUA_THEMES`): so a seed, a recipe's value, the
# theme list or the colour generator itself can change without this
# derivation — and every crate compiled against it — changing at all
# (`themes-are-data`, `names-are-extracted-at-evaluation`).
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  glyphdb,
  # THE COLOUR ROLES, by name: `nix/token-names.nix`'s `roles`, the attribute
  # names of `assets/tokens/colors.nix` and `assets/tokens/shaped.nix`.
  roleNames,
  typographyFile,
  spaceFile,
  trayTableFile,
  categoryTableFile,
}:

# NO BRAIN MODULE'S GLYPH COMES IN: a module's mark is data the footer reads at
# run time (`nix/brain-catalogue.nix`), so a module's glyph — a person's above
# all — never moves this table, nor any of the crates that include it.

let
  # The Nix -> Python bridge is JSON. There is no Nix parser in Python, and
  # writing one would be reimplementing Nix logic (invariant 5).
  toJson = name: file: writeText name (builtins.toJSON (import file));

  # The SAME table `glyphgen` validates against the real `cmap`. It arrives here
  # by the second route because what knows token -> codepoint is this generator;
  # over there it is checked, here it is resolved.
  trayJson = toJson "tray-glyphs.json" trayTableFile;

  # A LIST, and not an attrset: `builtins.toJSON` of a list preserves the file's
  # order, and the order is what decides which category wins. See the header of
  # `assets/icons/category-glyphs.nix`.
  categoriesJson = toJson "category-glyphs.json" categoryTableFile;

  rolesJson = writeText "token-roles.json" (builtins.toJSON roleNames);
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-tokens-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-tokens.py} \
      --roles ${rolesJson} \
      --typography ${toJson "typography.json" typographyFile} \
      --space ${toJson "space.json" spaceFile} \
      --icons ${glyphdb.db}/icons.json \
      --tray ${trayJson} \
      --categories ${categoriesJson} \
      --out $out/tokens.rs
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "The tokens' Rust table, generated from assets/tokens/*.nix: names and layout constants, no colour value";
    platforms = lib.platforms.all;
  };
}
