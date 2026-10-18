# THE TOKENS' NAMES, read at EVALUATION — and only the names.
#
# The Rust is compiled against names: `ronoctua_tokens::color::TEXT_DIM` is a
# ROLE, an index, and which bytes it is depends on the theme the process wears
# when it draws. The buffers are compiled against names too: the WIT enums
# `color` and `icon`. So nothing compiled needs a value — and a derivation
# that read one would be rebuilt, with every crate behind it, each time a seed,
# a recipe or the colour generator moved. The values are data instead: the
# palettes the generator writes (`palettes.bin` in `nix/themes.nix`'s output),
# read by the binaries at run time through `RONOCTUA_THEMES`.
#
# What this file reads, it reads by ATTRIBUTE NAME:
#
# - the roles: `assets/tokens/colors.nix` (the recipes) and
#   `assets/tokens/shaped.nix` (the groups the generator lays out by rule),
#   flattened to dotted names the way `gen-theme.py`'s `flatten_recipes` does
#   — a set holding `of` is a recipe, any other set is a group;
# - the icons: `assets/tokens/icons.nix`, the chrome's own tokens. NOT a brain
#   module's glyph: those reach the host's table (`nix/glyphgen.nix`) and
#   never the WIT, so a person's module changes no buffer.
#
# A recipe's VALUE is never looked at, so editing one changes nothing this
# file returns — which is what `names-are-extracted-at-evaluation` proves.
{
  lib,
  # `assets/tokens/colors.nix`: the recipes; their names are roles.
  recipesFile,
  # `assets/tokens/shaped.nix`: the shaped roles' names.
  shapedFile,
  # `assets/tokens/icons.nix`: the chrome's icon tokens.
  iconTokensFile,
}:

let
  # Nested sets to dotted names. A set with `of` in it is a recipe, not a
  # group — `surface.mesh = { of = …; chroma = …; }` is ONE role.
  flatten =
    prefix: set:
    lib.concatLists (
      lib.mapAttrsToList (
        key: value:
        let
          name = if prefix == "" then key else "${prefix}.${key}";
        in
        if builtins.isAttrs value && !(value ? of) then flatten name value else [ name ]
      ) set
    );

  recipes = flatten "" (import recipesFile);
  shaped = flatten "" (import shapedFile);
  # A NAME IN BOTH FILES would be one role with two answers: the recipe's and
  # the generator's rule. The generator would refuse it too, after a build;
  # here it is refused at once, by name.
  twice = lib.intersectLists recipes shaped;

  # Sorted: the index of a role is its place in this list, on both sides of
  # every boundary (`color::NAMES`, the palettes' order, the WIT enum). Byte
  # order, which for these ASCII names is Python's `sorted` order too.
  sorted = builtins.sort builtins.lessThan;
  roles =
    if twice != [ ] then
      throw "a colour role is named by both assets/tokens/colors.nix and assets/tokens/shaped.nix: ${lib.concatStringsSep ", " twice}"
    else
      sorted (recipes ++ shaped);
  icons = sorted (builtins.attrNames (import iconTokensFile));

  # WIT's reserved words: an identifier that collides needs the `%` prefix,
  # or the parser stops at the enum's line with "found keyword `list`".
  reserved = [
    "bool"
    "borrow"
    "char"
    "constructor"
    "enum"
    "error"
    "export"
    "f32"
    "f64"
    "flags"
    "from"
    "func"
    "future"
    "import"
    "include"
    "interface"
    "list"
    "option"
    "own"
    "package"
    "record"
    "resource"
    "result"
    "s16"
    "s32"
    "s64"
    "s8"
    "static"
    "stream"
    "string"
    "tuple"
    "type"
    "u16"
    "u32"
    "u64"
    "u8"
    "use"
    "variant"
    "with"
    "world"
    "as"
  ];

  # `surface.base` -> `surface-base`; `list` -> `%list`. The host's half of the
  # same spelling is `gen-tokens.py`'s `wit_ident` (`color_by_wit_name`), and
  # `invalid-token-fails-build` holds the two to one answer.
  #
  # REFUSED HERE, with the token's name, what WIT would refuse three
  # derivations later inside a buffer's `generate!`: every hyphen-separated
  # piece starts with a letter.
  witIdent =
    name:
    let
      pieces = builtins.filter (p: builtins.isString p && p != "") (
        builtins.split "[^a-z0-9]+" (lib.toLower name)
      );
      id = lib.concatStringsSep "-" pieces;
    in
    if builtins.match "[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*" id == null then
      throw "the token `${name}` becomes `${id}`, which is not a WIT identifier: in WIT every hyphen-separated piece starts with a LETTER — `wifi-0` will not do, `wifi-weak` will"
    else if builtins.elem id reserved then
      "%${id}"
    else
      id;

  witText = lib.concatStringsSep "\n" (
    [
      "// GENERATED at evaluation by nix/token-names.nix. DO NOT EDIT BY HAND."
      "//"
      "// Section 7.6's (b): 'a WIT enum of valid names; a non-existent token"
      "// becomes a compile error in the guest'. From the NAMES alone: the roles"
      "// of assets/tokens/colors.nix and shaped.nix, the icons of"
      "// assets/tokens/icons.nix. No value crosses to a buffer (invariant 11)."
      ""
      "package ronoctua:tokens@0.1.0;"
      ""
      "interface tokens {"
      "    /// Design system colours, by NAME. The host resolves the value."
      "    ///"
      "    /// A buffer cannot name a colour that does not exist: a name"
      "    /// that is not here does not compile."
      "    enum color {"
    ]
    ++ lib.concatMap (role: [
      "        // color.${role}"
      "        ${witIdent role},"
    ]) roles
    ++ [
      "    }"
      ""
      "    /// UI icons — glyphs from the font (invariant 10)."
      "    enum icon {"
    ]
    ++ map (icon: "        ${witIdent icon},") icons
    ++ [
      "    }"
      "}"
      ""
    ]
  );
in
{
  # The role names, sorted: `color::NAMES` and the palettes' order — for the
  # tokens' Rust (`nix/tokens.nix`) and the palettes (`nix/themes.nix`), which
  # refuses to solve a role this list does not name, or to leave one out. And
  # the chrome's icon names, the WIT's.
  inherit roles icons;
  # The WIT package `ronoctua:tokens`, as `nix/wit.nix` puts it in `deps/`.
  wit = builtins.toFile "tokens.wit" witText;
}
