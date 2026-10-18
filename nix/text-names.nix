# THE NAMES OF A `.ftl`'s KEYS, read at EVALUATION — and nothing of their
# values.
#
# The shape of `gen-i18n.py`'s `KEY`: a message key in column 0,
# `^([a-z][a-z0-9-]*)\s*=`. One reader for every place that needs a fragment's
# names before anything is built: each crate's `key` module
# (`nix/text-keys.nix`), the merged texts' census (`nix/i18n.nix`) and the
# brain's manifests (`nix/brain-modules.nix`).
{ lib }:
let
  # A message key in column 0, or nothing.
  keyOf =
    line:
    let
      m = builtins.match "([a-z][a-z0-9-]*)[[:space:]]*=.*" line;
    in
    if m == null then [ ] else m;
in
{
  # Every key of a `.ftl`'s text, in file order.
  #
  # THE CONTEXT GOES FIRST. These are NAMES, and the text of a file read out
  # of the store carries that path as its context: whatever is written from
  # it would depend on the file's every byte — the values this function exists
  # to keep out.
  namesIn =
    text: lib.concatMap keyOf (lib.splitString "\n" (builtins.unsafeDiscardStringContext text));
}
