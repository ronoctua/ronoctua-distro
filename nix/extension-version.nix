# THE EXTENSION'S VERSION — one place, and it has to go UP.
#
# Chrome replaces a force-installed extension only with a HIGHER version: the
# same number with new files is ignored, silently, for ever. It was "1.0.0"
# and frozen, so no change to `extension/` ever reached a browser that had
# installed it once.
#
# TWO HALVES, and neither needs a person to remember anything:
#
#   - `base`, here, is the version the manifest is BUILT with
#     (`nix/extension.nix`). Raised by hand only to say "this is a new
#     release" — a change of what the extension asks for, say;
#   - the fourth number is the MACHINE's: the unit that packs the `.crx`
#     (`nix/modules/browser-extension.nix`) keeps, beside the signing key, the
#     store path it last packed and a counter, and every time the extension it
#     is given is another build, the counter goes up by one and is written into
#     the packed manifest and `update.xml` alike. So any change to
#     `extension/`, to its texts or to its colours — each is another store
#     path — arrives as `<base>.<n + 1>`, which Chrome takes.
#
# Why not a hash of the sources: a hash is not ORDERED, and half of all edits
# would come out "older". Why not the commit's time: it moves with every
# commit that never touched the extension, and it is not known for an edited
# tree. The counter dies with the key, and a new key is a new extension
# anyway (a new id), so it never has to go backwards.
{
  base = "1.1.0";
}
