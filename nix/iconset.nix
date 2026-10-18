# The three icon tables in ONE directory — what `RONOCTUA_ICONSET` names.
#
# `ronoctua-iconset` reads `glyphs.json`, `lucide.json` and `unicode.json` at
# RUN time (`atRunTime` in `nix/rust.nix`), never at compile time: ~9 MB that a
# new Nerd Fonts release or a lexicon edit re-bakes, and a store path in
# `generated` would rebuild every unit that reads it, and every one above, for
# data no compilation reads.
# `iconset-is-at-run-time` holds that.
#
# THE GLYPH INDEX IS WRITTEN HERE, the other two are symlinks. The glyph
# database names each glyph by the chrome's token that draws it, and a brain
# module's glyph token is measured into the brain's catalogue instead
# (`nix/brain-catalogue.nix`) — so this joins the modules' measured tokens to
# the database's index (`gen-iconset.py`), by the database's own rule, and the
# browser names a module's glyph by its token as it did when the database
# measured them: the `token` row, the `token:` filter, copying it as
# `icon.<token>` (`the-glyph-browser-names-the-modules-glyphs`). Data joined to
# data: a person's module moves this directory (`nix/person-brain.nix`) and
# compiles nothing. The database itself no longer travels — nothing else a
# program carries refers to it — so the written index is not a second copy.
#
# Symlinks for the other two: their generators already own the files, and a
# copy would be a second copy in every closure that carries both.
{
  runCommand,
  python3,
  glyphdb,
  lucidedb,
  unicodedb,
  # THE BRAIN MODULES' GLYPH TOKENS, measured (`brainCatalogue.measured`,
  # `icons.json`): the distro's, or a machine's with the person's beside them.
  brainGlyphs,
}:

runCommand "ronoctua-iconset" { nativeBuildInputs = [ python3 ]; } ''
  mkdir -p $out
  python3 ${./scripts/gen-iconset.py} \
    --glyphs ${glyphdb}/glyphs.json \
    --icons ${glyphdb}/icons.json \
    --modules ${brainGlyphs}/icons.json \
    --out $out/glyphs.json
  ln -s ${lucidedb}/lucide.json $out/lucide.json
  ln -s ${unicodedb}/unicode.json $out/unicode.json
''
