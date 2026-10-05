# Nothing cached stays for ever — the catalogue's options, and where every
# build lands.
#
# WHY THIS EXISTS
# ---------------
# Because nothing swept anything. The owner asked how this distro's caches
# worked and the honest answer was that neither of the two on disk was ever
# cleaned: no timer, no tmpfiles rule, no walk. His rule came back in one
# sentence — nothing cached may stay for ever.
#
# WHAT IS HERE, AND WHAT IS NOT
# -----------------------------
# The UNITS that sweep the cache and empty the trash are the brain's modules
# now, each in its folder with its own `module.nix`
# (`crates/ronoctua-brain/modules/`): the brain decides when, against a gate
# the person configured, and each can be switched off alone
# (`ronoctua.brain.modules.<id>.enable`). What stays here is what is not a
# module's: the catalogue's OPTIONS — whether the machine throws cached
# things away at all (the default of those two modules' switches), and after
# how long — which those units read, and the one rule about builds that holds
# on every machine whether anything sweeps or not.
{
  config,
  lib,
  ronoctua,
  ...
}:

let
  # THE POLICY IS THE CATALOGUE'S, and this reads the same file the Rust table
  # is generated from. Two spellings of twenty days is one of them going stale.
  declared = import ../../assets/cache.nix;
in
{
  options.ronoctua.cache = {
    enable = lib.mkEnableOption "sweeping cached files nothing has used for a while" // {
      default = true;
    };

    keepDays = lib.mkOption {
      type = lib.types.ints.positive;
      default = declared.keepDays;
      description = ''
        How long a cached file may go unused before the sweep takes it, in days.

        The default comes from `assets/cache.nix`, which is also where the Rust
        side reads it — the option exists so a machine can differ, not so the
        number can be written twice.
      '';
    };

    trashDays = lib.mkOption {
      type = lib.types.ints.positive;
      default = declared.trashDays;
      description = ''
        How long something stays in the trash before it is thrown away for good,
        in days.

        Counted from the `DeletionDate` the freedesktop spec writes into every
        note, never from the file's own timestamps: a document written a year
        ago and thrown away this morning has been in the trash for a morning.

        This is the one number in this module that decides something
        irreversible, which is why it is four days and not forty.
      '';
    };
  };

  config = {
    # WHETHER THE MACHINE THROWS CACHED THINGS AWAY AT ALL is the default of
    # the two modules' own switches, and not a second switch around their
    # units. It was a second switch: `enable = false` took the units away and
    # told the brain nothing, so the brain went on scheduling both, started
    # units that were no longer there and wrote a failure every cadence. Now
    # the module's switch is the only one the units answer to (the loader's
    # gate), and it is the one `RONOCTUA_BRAIN_OFF` is built from — so the
    # rows are drawn refusing with the system's reason. A default and not a
    # value: a machine may still switch one of the two back on by its own id.
    ronoctua.brain.modules = lib.mkIf (!config.ronoctua.cache.enable) {
      cache-sweep.enable = lib.mkDefault false;
      trash-expire.enable = lib.mkDefault false;
    };

    # EVERY PROJECT'S INTERMEDIATES LEAVE EVERY TREE — on every machine, and
    # whether the sweep runs or not: this is about where a build lands, not
    # about who cleans it.
    #
    # `cargo` in any directory puts `deps/`, fingerprints and incremental state
    # — nearly all of a build's bytes — under `./target`, beside the source,
    # where no cache rule sees them and a backup or a snapshot copies them.
    # `CARGO_BUILD_BUILD_DIR` sends them under the cache, one directory per
    # workspace by cargo's own `{workspace-path-hash}`, named after the
    # compiler (nix/cargo-dirs.nix). Each project keeps its `./target` for
    # what a person runs; `CARGO_TARGET_DIR` stays unset here on purpose, and
    # the reason is written in that file.
    #
    # TWICE, because two things start programs. `sessionVariables` reaches
    # login shells, the console and ssh (pam_env and `/etc/profile`);
    # `environment.d` reaches the user manager, and so every unit and every
    # scope the footer opens — a terminal, an editor, rust-analyzer. It is the
    # route NixOS itself takes for `PATH` (`50-systemd-path.conf`).
    environment.sessionVariables.CARGO_BUILD_BUILD_DIR = ronoctua.cargoDirs.session.build;
    environment.etc."environment.d/60-ronoctua-cargo.conf".text = ''
      CARGO_BUILD_BUILD_DIR="${ronoctua.cargoDirs.session.build}"
    '';
  };
}
