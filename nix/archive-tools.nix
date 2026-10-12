# WHAT `ronoctua-archive` RUNS, by store path — the three programs a person's
# archive goes through (`crates/ronoctua-archive`). `nix/rust.nix` bakes 7zz
# and bsdtar into the workspace (`generated`, read with `option_env!`) and
# hands all three to the tests at run time; `bsdtar` is `RONOCTUA_BSDTAR`,
# already there for the file manager. unrar is set at run time only
# (`atRunTime`): unfree, a baked path would keep the workspace out of the
# public binary cache.
#
# `7zz` and not `p7zip`: `p7zip` is a fork frozen at 16.02; `7zz` is Igor
# Pavlov's own, current, with AES-256 zip, hidden names in 7z and split
# volumes. The free build has no RAR codec, so RAR stays with `unrar`.
#
# `unrar` IS UNFREE, and it is let in BY NAME and alone: a nixpkgs of its own
# whose predicate admits `unrar` and nothing else, the way
# `nix/player-tools.nix` admits `uade`. The system already names it in
# `nix/modules/nixpkgs-config.nix` for the same reason — it is the only thing
# that opens a `.rar` properly — so the store path is the one the system
# installs, and nothing new arrives.
{ pkgs }:

let
  unrar =
    (import pkgs.path {
      inherit (pkgs.stdenv.hostPlatform) system;
      config.allowUnfreePredicate = pkg: (pkgs.lib.getName pkg) == "unrar";
    }).unrar;

  # `bsdtar` WITH liblz4 LINKED IN. nixpkgs builds libarchive without it, and
  # then `--lz4` is not a codec but a PIPE to a program named `lz4`, found on
  # the `PATH` — a name, not a store path, and absent from the tests' sandbox
  # (MEASURED, 3.8.8: "Can't write to program: lz4", and reading an `.tar.lz4`
  # runs `lz4 -d` the same way). Without `lz4hc.h` it also refuses every
  # `compression-level` above 2 ("High compression not included in this
  # build"), so four of the five levels were the same archive. liblz4 is
  # already in the system (systemd links it); this libarchive is a second
  # copy of a 1 MB library, the price of one program that does it all.
  libarchive = pkgs.libarchive.overrideAttrs (old: {
    buildInputs = old.buildInputs ++ [ pkgs.lz4 ];
  });
in
{
  RONOCTUA_BSDTAR = "${libarchive}/bin/bsdtar";
  RONOCTUA_7ZZ = "${pkgs._7zz}/bin/7zz";
  RONOCTUA_UNRAR = "${unrar}/bin/unrar";
}
