# A NAME A SCREEN DRAWS — the characters that refuse one, in one place.
#
# ONE RULE with `ronoctua_person::reports::is_name`: no control character
# (C0, DEL and C1: Unicode's `Cc`) and not white space alone (Unicode's
# `White_Space`, what Rust's `trim` takes off). Compared as CHARACTERS, never
# with a regex's classes, which see bytes in the C locale: `[[:cntrl:]]`
# misses U+0080–U+009F and `[[:space:]]` misses U+00A0. A Nix string has no
# `\u`, so each character is a JSON escape.
#
# The registry's names (`nix/modules/distro-apps.nix`) take `isName`; the
# installer's offered rows (`nix/offered.nix`) take it and two more of these
# sets — a row is drawn on one line, and two names that read alike cannot be
# told apart — so a line break past ASCII (U+0085, U+2028) and a white space
# at an edge (U+00A0) are refused there by the same characters.
{ lib }:

let
  charOf = hex: builtins.fromJSON "\"\\u${hex}\"";
  hex4 = n: lib.fixedWidthString 4 "0" (lib.toHexString n);
  has = set: s: builtins.any (c: lib.hasInfix c s) set;
in
rec {
  # U+0000 cannot be in a Nix string at all.
  controlChars = map (n: charOf (hex4 n)) (lib.range 1 31 ++ lib.range 127 159);
  spaceChars = map charOf (
    [
      "0009"
      "000A"
      "000B"
      "000C"
      "000D"
      "0020"
      "0085"
      "00A0"
      "1680"
    ]
    ++ map hex4 (lib.range 8192 8202)
    ++ [
      "2028"
      "2029"
      "202F"
      "205F"
      "3000"
    ]
  );
  # Unicode's line terminators: what makes one name two lines.
  lineBreaks = map charOf [
    "000A"
    "000B"
    "000C"
    "000D"
    "0085"
    "2028"
    "2029"
  ];

  hasControl = has controlChars;
  spansLines = has lineBreaks;
  isWhiteAlone = s: builtins.replaceStrings spaceChars (map (_: "") spaceChars) s == "";
  hasWhiteAtAnEdge = s: builtins.any (c: lib.hasPrefix c s || lib.hasSuffix c s) spaceChars;

  isName = s: builtins.isString s && !(hasControl s) && !(isWhiteAlone s);
}
