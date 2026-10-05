# The three icon tables in ONE directory — what `RONOCTUA_ICONSET` names.
#
# `ronoctua-iconset` reads `glyphs.json`, `lucide.json` and `unicode.json` at
# RUN time (`atRunTime` in `nix/rust.nix`), never at compile time: ~9 MB that a
# new Nerd Fonts release or a lexicon edit re-bakes, and a store path in
# `generated` would rebuild the whole workspace for data no compilation reads.
# `iconset-is-at-run-time` holds that.
#
# Symlinks and not copies: the three generators already own the files, and a
# copy would be a second 9 MB in every closure that carries both.
{
  runCommand,
  glyphdb,
  lucidedb,
  unicodedb,
}:

runCommand "ronoctua-iconset" { } ''
  mkdir -p $out
  ln -s ${glyphdb}/glyphs.json $out/glyphs.json
  ln -s ${lucidedb}/lucide.json $out/lucide.json
  ln -s ${unicodedb}/unicode.json $out/unicode.json
''
