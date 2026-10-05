# Nothing cached stays for ever: the two user units the brain starts.
#
# WHY THIS EXISTS
# ---------------
# Because nothing swept anything. The owner asked how this distro's caches
# worked and the honest answer was that neither of the two on disk was ever
# cleaned: no timer, no tmpfiles rule, no walk. His rule came back in one
# sentence — nothing cached may stay for ever.
#
# WHY IT IS A UNIT AND NOT SOMETHING THE BRAIN DOES ITSELF
# ---------------------------------------------------------
# A run blocks the brain's own thread: a walk of a quarter of a million files
# there is a daemon that draws nothing and cannot be stopped while it lasts.
# And a unit gets `IOSchedulingClass=idle` for nothing, which is the argument
# `nix/modules/memory.nix` already makes about collecting the store — it
# competes with everything for the disk, so it yields to whatever the person
# is doing. A run takes whole cache directories at once, which is a lot of
# `unlink`; without this it is a machine nobody can type on while it happens.
#
# WHAT IT MAY NOT DO
# ------------------
# It runs as the PERSON and never as root, it asks nobody for anything, and it
# touches exactly one directory tree: `$XDG_CACHE_HOME`. There is no polkit
# rule here and there must never be one — a sweep that could be talked into
# running as root is a sweep that can delete a system.
#
# WHEN IT RUNS
# ------------
# Not on a timer. The brain decides, against a gate the person configured
# (RAM, CPU stall, battery) — which is what makes "at four in the morning, if
# the machine is not busy" one rule and not two. `systemd.user.timers` would
# be a second schedule that knows nothing about load, and the two would
# disagree on exactly the night it mattered.
#
# ONE SWITCH: `ronoctua.brain.modules.<id>.enable = false` takes these units
# away with the module (the gate `nix/brain-modules.nix` puts around this
# file) and tells the brain, which draws the row refusing. `keepDays` stays
# the catalogue's option (`nix/modules/cache.nix`), where the file manager's
# side reads it too, and so does `ronoctua.cache.enable` — which is only that
# switch's DEFAULT, and never a second gate here: a second gate took the
# units away without telling the brain, which went on starting them.
#
# THE NAMES ARE THE MANIFEST'S: the two units are the ones it declares, and
# the reports are this folder's name — the runner watches `<id>.json` and
# `<id>-rehearsal.json`, so the file the unit writes and the file the brain
# reads are one name, written once.
{
  config,
  lib,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.cache;
  id = baseNameOf ./.;
  manifest = import ./manifest.nix;
  service = unit: lib.removeSuffix ".service" unit;

  # ONE UNIT, WRITTEN ONCE. The two differ in a description, a report file and
  # one word; spelled twice they would drift, and the one that drifts is the one
  # nobody reads — the rehearsal, which is exactly the one that has to behave
  # identically to be worth anything.
  sweepUnit =
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
            "${ronoctua.cacheSweep}/bin/ronoctua-cache-sweep"
            # THE ROOT IS NAMED, and it is the same `%C` the sandbox below
            # opens. Left to the binary's own default it would read
            # `$XDG_CACHE_HOME` from the environment, and a unit whose sandbox
            # and whose walk disagree about which directory that is would fail
            # in a way that reads as a bug in the walk. On btrfs `%C` is a
            # subvolume of its own (`nix/modules/filesystem.nix`), so the walk's
            # one device is the whole cache.
            "--root %C"
            # THE REPORT THE BRAIN WATCHES. `%t` is the user's runtime
            # directory, which is where the runner looks — and the two agreeing
            # is not a comment, it is this module's own check, which reads the
            # unit as the machine gets it. They disagreed once (the daemon
            # looked in `/run/ronoctua-brain`, which nothing ever wrote), and
            # the symptom was every run the brain started ending as `runaway`
            # after its whole patience had gone by.
            "--report %t/ronoctua-brain/${report}"
            "--days ${toString cfg.keepDays}"
            # THE ROOTS, ASKED OF NIX by store path. An entry holding a GC root
            # — a `nix develop --profile` link under the cache — is kept, and
            # without an answer every directory holding a link is.
            "--nix-store ${config.nix.package}/bin/nix-store"
          ]
          # AND ONLY THE REAL ONE ACTS. The binary rehearses by default, so the
          # word that deletes files is written once, in a unit, where a person
          # reading `systemctl --user cat` can see it.
          ++ lib.optional execute "--execute"
        );

        # IT COMPETES WITH EVERYTHING FOR THE DISK, so it yields. Same pair,
        # and the same sentence, as the store's own collection in `memory.nix`.
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";

        # `session.slice` for the brain's reason: `memory.nix` arms
        # `ManagedOOMMemoryPressure=kill` on `app.slice`, and a sweep killed
        # half way through is a cache in a state nobody planned.
        Slice = "session.slice";
        OOMPolicy = "stop";

        # WHAT IT MAY TOUCH, SAID BY SYSTEMD and not only by the code.
        #
        # The walk already refuses to follow a link out of its root and to
        # cross a filesystem. This is the second lock on the same door, and it
        # is the one that still holds if the code is wrong: the only writable
        # path in the whole filesystem is the cache directory itself.
        # `ReadWritePaths` takes precedence over `ProtectHome`, which is what
        # lets the two lines say "the home is read-only EXCEPT this".
        ProtectSystem = "strict";
        ProtectHome = "read-only";
        ReadWritePaths = [
          # `-`: a person with no `~/.cache` yet has nothing to sweep, and a
          # path that is not there would fail the unit before it starts.
          "-%C"
          "%t/ronoctua-brain"
        ];
        # AND THE REPORT'S DIRECTORY EXISTS BEFORE THE SANDBOX IS BUILT. A
        # `ReadWritePaths` naming a path that is not there fails the unit with
        # 226/NAMESPACE before the program starts (measured with a transient
        # unit), and nothing else made `%t/ronoctua-brain` but the update's
        # script — so on a session where no update had run yet, the sweep
        # never started and the brain waited out its whole patience.
        # `Preserve` because several units and the update share it, and the
        # brain reads it.
        RuntimeDirectory = "ronoctua-brain";
        RuntimeDirectoryPreserve = "yes";
        PrivateDevices = true;
        NoNewPrivileges = true;
        RestrictSUIDSGID = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictNamespaces = true;
        SystemCallFilter = [ "@system-service" ];
        # NO NETWORK. The one socket it reaches is the Nix daemon's, a Unix
        # socket on the filesystem, which `nix-store --gc --print-roots` asks
        # which links are roots — and a network namespace does not hide those.
        PrivateNetwork = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
      };
    };
in
{
  # TWO UNITS AND NOT A FLAG. The second walks and counts and deletes nothing,
  # which is what `m e` on the brain's page asks for — and it earns its keep
  # here more than it does for the update: *"how much would this take?"* is
  # exactly the question one wants answered before letting something delete a
  # hundred and fifty gibibytes. ONE attrset: two names computed into two
  # separate `systemd.user.services.<name>` lines is an evaluation error.
  config = {
    systemd.user.services = {
      ${service manifest.units.rehearse} = sweepUnit {
        what = "rehearse sweeping cached files — deletes nothing";
        report = "${id}-rehearsal.json";
        execute = false;
      };
      ${service manifest.units.run} = sweepUnit {
        what = "sweep cached files nothing has used";
        report = "${id}.json";
        execute = true;
      };
    };
  };
}
