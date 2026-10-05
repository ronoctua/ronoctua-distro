# The super index's limits, as the arguments of `ronoctua-index serve`.
#
# ONE LIST, TWO READERS: `nix/modules/index.nix` starts the unit with it, and
# the checks (`index-is-bounded`, `the-reader-sees-the-commit`) run the same
# binary with the same words — so what a check proves is what the unit is
# started with. The numbers are `assets/cache.nix`'s; the binary has no
# default for any of them and refuses to start without one.
let
  declared = import ../assets/cache.nix;

  positive =
    name:
    let
      value = declared.${name};
    in
    if builtins.isInt value && value > 0 then
      value
    else
      throw "assets/cache.nix: `${name}` must be a positive whole number, not ${builtins.toJSON value}";

  # The writer's arena is an eighth of the unit's memory: the rest is the
  # walk, a document being read and the merges. tantivy refuses less than
  # fifteen megabytes a thread, so the floor is sixteen MiB.
  arena =
    let
      eighth = positive "searchMemoryMiB" / 8;
    in
    if eighth < 16 then 16 else eighth;

  skips =
    if builtins.isList declared.searchSkip && builtins.all builtins.isString declared.searchSkip then
      declared.searchSkip
    else
      throw "assets/cache.nix: `searchSkip` must be a list of names and absolute paths";
in
{
  # The unit's `MemoryMax`, in MiB.
  memoryMiB = positive "searchMemoryMiB";

  # Every limit, as words for the command line.
  arguments = [
    "--ceiling-mib"
    (toString (positive "searchIndexMiB"))
    "--content-file-mib"
    (toString (positive "searchContentFileMiB"))
    "--content-kib"
    (toString (positive "searchContentKiB"))
    "--memory-mib"
    (toString arena)
  ]
  ++ builtins.concatMap (skip: [
    "--skip"
    skip
  ]) skips;
}
