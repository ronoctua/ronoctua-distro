# Design system tokens, from Nix to Rust — sections 7.6 and 7.8.
#
# "Tokens defined ONCE in Nix; the build generates (a) the shell's Rust table
# and (b) a WIT enum of valid names."
#
# BOTH come from here, from the same data: `tokens.rs` for the host and
# `tokens.wit` for the guest. Two lists would diverge, and the divergence
# would show up as the wrong colour on screen — not as an error.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  glyphdb,
  # THE COLOURS, ALREADY SOLVED: `nix/themes.nix` runs the generator over
  # every theme and this only reads `themes.json` out of it — the ordered list
  # of `{ id, colors }`.
  themed,
  # WHICH OF THEM A FRESH MACHINE SHOWS, by name. It is not the first any
  # more: see `assets/themes/themes.nix`.
  defaultTheme,
  typographyFile,
  spaceFile,
  trayTableFile,
  categoryTableFile,
  # THE BRAIN MODULES' GLYPH TOKENS (brain.md §3). They are in the icon table
  # already — `nix/glyphgen.nix` merges and measures them — and this is only
  # which of the table's names are a module's, for `icon::brain_mark`: the
  # modeline draws a mark from a name that crossed the brain's socket, and
  # only a module's own glyph may answer it. The flake's one loader's `glyphs`.
  brainGlyphs,
}:

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

  brainMarksJson = writeText "brain-marks.json" (builtins.toJSON (builtins.attrNames brainGlyphs));
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
      --themes ${themed}/themes.json \
      --default-theme ${defaultTheme} \
      --typography ${toJson "typography.json" typographyFile} \
      --space ${toJson "space.json" spaceFile} \
      --icons ${glyphdb.db}/icons.json \
      --tray ${trayJson} \
      --categories ${categoriesJson} \
      --brain-marks ${brainMarksJson} \
      --out $out/tokens.rs \
      --out-wit $out/tokens.wit
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "Tokens in Rust and in WIT, generated from assets/tokens/*.nix";
    platforms = lib.platforms.all;
  };
}
