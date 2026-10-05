# The disk makes room for itself when it is short: the two user units the
# brain starts.
#
# WHY THIS EXISTS
# ---------------
# The owner's words (2026-09-28): the distro handles the disk "sem precisar de
# intervenções minhas", and the answer has to hold on every machine that
# installs it. Nix's own floor (`nix/modules/store.nix`, `min-free`) makes
# room only while something builds, and only in the store; a disk filled by
# anything else stays full until somebody notices. The brain notices: its
# heartbeat already reads the filesystems under `/nix/store` and the home,
# and below the floor it starts the unit below, at once — without waiting for
# the night or for a quiet machine (`module.rs`, and `why.md`).
#
# WHAT IT DOES
# ------------
# Two steps, both `ronoctua-disk-relief` (`crates/ronoctua-cache-sweep`):
#
#   cargo-builds    only while the CACHE's filesystem is short: whole
#                   directories of a compiler nobody uses, then, while every
#                   cargo tree under the cache weighs more than
#                   `cargoCeilingPercent` of its disk, what nothing reads again
#                   and the least recently built units — only under cargo's own
#                   lock, never a link, never a GC root;
#   store-garbage   `nix-store --gc --max-freed <goal − free>`: GARBAGE and
#                   nothing else, through the daemon, until `diskGoalPercent`
#                   is free (at most `diskGoalCapGiB`). No generation of
#                   anybody's, no rooted path.
#
# WHAT IT MAY NOT DO
# ------------------
# It runs as the PERSON, asks nobody for anything, and writes only under the
# cache and the brain's report directory — the sweep's sandbox, copied below.
# No polkit rule: the store's collection goes through the daemon, which lets
# any user collect garbage and nobody collect what is live.
#
# EVERY NUMBER IS THE CATALOGUE'S (`assets/cache.nix`), passed as an argument:
# the brain's floor is compiled from the same table (`tables.nix`), so a floor
# here and a floor there cannot disagree. There is no option for them on
# purpose — and no switch for the relief here either: the brain's own page
# switches this module off, and `ronoctua.brain.modules.<id>.enable = false`
# takes these units away with it (the gate `nix/brain-modules.nix` puts around
# this file).
#
# THE NAMES ARE THE MANIFEST'S: the two units are the ones it declares, and
# the reports are this folder's name — `report_path` in the brain builds
# `<id>.json` and `<id>-rehearsal.json`, so the file the unit writes and the
# file the runner watches are one name, written once.
#
# The one thing it needs besides is a graphical session: the brain runs in
# one. A machine nobody logs into graphically has Nix's own floor during
# builds and the trim at every activation, and nothing that walks a cache.
{
  config,
  lib,
  ronoctua,
  ...
}:

let
  declared = import ../../../../assets/cache.nix;
  id = baseNameOf ./.;
  manifest = import ./manifest.nix;
  service = unit: lib.removeSuffix ".service" unit;

  # ONE UNIT, WRITTEN ONCE — the sweep's reason: the rehearsal is the one
  # nobody reads, and it has to behave exactly like the real one to be worth
  # anything.
  reliefUnit =
    {
      what,
      report,
      execute,
    }:
    {
      description = "ronoctua — ${what}";

      serviceConfig = {
        Type = "oneshot";
        ExecStart = lib.concatStringsSep " " (
          [
            "${ronoctua.diskRelief}/bin/ronoctua-disk-relief"
            # THE CACHE, the same `%C` the sandbox opens — the sweep's reason.
            "--root %C"
            # WHERE CARGO WRITES, from the one file that names it: one
            # directory per compiler, and the session's own is never taken.
            "--cargo-root %C/${ronoctua.cargoDirs.parent}"
            "--compiler-prefix ${ronoctua.cargoDirs.compilerPrefix}"
            "--keep-compiler ${ronoctua.cargoDirs.compiler}"
            # NIX BY STORE PATH: asked which links are roots, and asked to
            # collect.
            "--nix-store ${config.nix.package}/bin/nix-store"
            "--store ${builtins.storeDir}"
            # THE REPORT THE BRAIN WATCHES — this module's own check reads
            # the unit this line makes.
            "--report %t/ronoctua-brain/${report}"
            "--goal-percent ${toString declared.diskGoalPercent}"
            "--goal-cap-gib ${toString declared.diskGoalCapGiB}"
            "--cargo-ceiling-percent ${toString declared.cargoCeilingPercent}"
            "--old-compiler-days ${toString declared.oldCompilerDays}"
          ]
          # AND ONLY THE REAL ONE ACTS.
          ++ lib.optional execute "--execute"
        );

        # IDLE, like the sweep and the store's own collection: it runs while
        # the person works — that is the point of not waiting for the night —
        # so it yields to everything they do.
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";
        Slice = "session.slice";
        OOMPolicy = "stop";

        # THE SWEEP'S SANDBOX, line for line (`nix/modules/cache.nix`): the
        # only writable paths are the cache and the brain's reports. The
        # store's collection happens in the daemon, as root, on its own terms.
        ProtectSystem = "strict";
        ProtectHome = "read-only";
        ReadWritePaths = [
          # `-`: a person with no `~/.cache` yet still gets the store's
          # collection, instead of a unit that fails before it starts.
          "-%C"
          "%t/ronoctua-brain"
        ];
        # MADE BEFORE THE SANDBOX, or a missing `%t/ronoctua-brain` fails the
        # unit with 226/NAMESPACE — the sweep's reason, in `cache.nix`.
        RuntimeDirectory = "ronoctua-brain";
        RuntimeDirectoryPreserve = "yes";
        PrivateDevices = true;
        NoNewPrivileges = true;
        RestrictSUIDSGID = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictNamespaces = true;
        SystemCallFilter = [ "@system-service" ];
        # NO NETWORK: the daemon's socket is a Unix socket on the filesystem.
        PrivateNetwork = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
      };
    };
in
{
  # TWO UNITS AND NOT A FLAG, for the sweep's reason: `m e` on the brain's
  # page rehearses, and a flag crossing from the brain would be a flag
  # somebody else could cross with. ONE attrset: two names computed into two
  # separate `systemd.user.services.<name>` lines is an evaluation error.
  systemd.user.services = {
    ${service manifest.units.rehearse} = reliefUnit {
      what = "rehearse making room on the disk — deletes nothing";
      report = "${id}-rehearsal.json";
      execute = false;
    };
    ${service manifest.units.run} = reliefUnit {
      what = "make room on the disk: old cargo builds, then the store's garbage";
      report = "${id}.json";
      execute = true;
    };
  };
}
