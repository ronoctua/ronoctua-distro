# Nothing cached stays for ever.
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
# `Task::run` blocks the brain's own thread: a walk of a quarter of a million
# files there is a daemon that draws nothing and cannot be stopped while it
# lasts. And a unit gets `IOSchedulingClass=idle` for nothing, which is the
# argument `nix/modules/memory.nix` already makes about collecting the store —
# it competes with everything for the disk, so it yields to whatever the person
# is doing. A run takes whole cache directories at once, which is a lot of
# `unlink`; without this it is a machine nobody can type on while it happens.
#
# WHAT IT MAY NOT DO
# ------------------
# It runs as the PERSON and never as root, it asks nobody for anything, and it
# touches exactly one directory tree: `$XDG_CACHE_HOME`. There is no polkit rule
# here and there must never be one — a sweep that could be talked into running
# as root is a sweep that can delete a system.
#
# WHEN IT RUNS
# ------------
# Not on a timer. The brain decides, against a gate the person configured
# (`ronoctua_brain::runner::judge`: RAM, CPU stall, battery) — which is what
# makes "at four in the morning, if the machine is not busy" one rule and not
# two. `systemd.user.timers` would be a second schedule that knows nothing about
# load, and the two would disagree on exactly the night it mattered.
{
  config,
  lib,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.cache;
  # THE POLICY IS THE CATALOGUE'S, and this reads the same file the Rust table
  # is generated from. Two spellings of twenty days is one of them going stale.
  declared = import ../../assets/cache.nix;

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
            # in a way that reads as a bug in the walk.
            "--root %C"
            # THE REPORT THE BRAIN WATCHES. `%t` is the user's runtime
            # directory, which is where `ronoctua_brain::runner::reports_dir`
            # looks — and the two agreeing is not a comment, it is
            # `the-brain-reads-what-the-units-write`, which reads this line and
            # that function. They disagreed once (the daemon looked in
            # `/run/ronoctua-brain`, which nothing ever wrote), and the symptom
            # was every run the brain started ending as `runaway` after its
            # whole patience had gone by.
            "--report %t/ronoctua-brain/${report}"
            "--days ${toString cfg.keepDays}"
          ]
          # AND ONLY THE REAL ONE ACTS. The binary rehearses by default, so the
          # word that deletes files is written once, in a unit, where a person
          # reading `systemctl --user cat` can see it.
          ++ lib.optional execute "--execute"
        );

        # IT COMPETES WITH EVERYTHING FOR THE DISK, so it yields. Same pair, and
        # the same sentence, as the store's own collection in `memory.nix`.
        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";

        # `session.slice` for the brain's reason: `memory.nix` arms
        # `ManagedOOMMemoryPressure=kill` on `app.slice`, and a sweep killed
        # half way through is a cache in a state nobody planned.
        Slice = "session.slice";
        OOMPolicy = "stop";

        # WHAT IT MAY TOUCH, SAID BY SYSTEMD and not only by the code.
        #
        # The walk already refuses to follow a link out of its root and to cross
        # a filesystem. This is the second lock on the same door, and it is the
        # one that still holds if the code is wrong: the only writable path in
        # the whole filesystem is the cache directory itself. `ReadWritePaths`
        # takes precedence over `ProtectHome`, which is what lets the two lines
        # say "the home is read-only EXCEPT this".
        ProtectSystem = "strict";
        ProtectHome = "read-only";
        ReadWritePaths = [
          "%C"
          "%t/ronoctua-brain"
        ];
        PrivateDevices = true;
        NoNewPrivileges = true;
        RestrictSUIDSGID = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictNamespaces = true;
        SystemCallFilter = [ "@system-service" ];
        # It opens no socket at all.
        PrivateNetwork = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
      };
    };
  # THE TRASH'S OWN UNIT, and the differences from the sweep's are the point:
  # another binary, another number, and `%S` rather than `%C` as the one
  # writable path — a sweep that could reach the trash would be a cache cleaner
  # deleting the only copy of somebody's file.
  trashUnit =
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
            "${ronoctua.emptyTrash}/bin/ronoctua-empty-trash"
            "--data-home %S"
            # THE SAME AGREEMENT the sweep above spells out, and the same check
            # reads this line too.
            "--report %t/ronoctua-brain/${report}"
            "--days ${toString cfg.trashDays}"
          ]
          ++ lib.optional execute "--execute"
        );

        CPUSchedulingPolicy = "idle";
        IOSchedulingClass = "idle";
        Slice = "session.slice";
        OOMPolicy = "stop";

        ProtectSystem = "strict";
        ProtectHome = "read-only";
        ReadWritePaths = [
          "%S"
          "%t/ronoctua-brain"
        ];
        PrivateDevices = true;
        NoNewPrivileges = true;
        RestrictSUIDSGID = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictNamespaces = true;
        SystemCallFilter = [ "@system-service" ];
        PrivateNetwork = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
      };
    };
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

  config = lib.mkIf (cfg.enable && config.ronoctua.brain.enable) {
    # TWO UNITS AND NOT A FLAG. The second walks and counts and deletes nothing,
    # which is what `m e` on the brain's page asks for — and it earns its keep
    # here more than it does for the update: *"how much would this take?"* is
    # exactly the question one wants answered before letting something delete a
    # hundred and fifty gibibytes. A flag crossing from the brain would be a
    # flag somebody else could cross with.
    systemd.user.services.ronoctua-cache-sweep-rehearsal = sweepUnit {
      what = "rehearse sweeping cached files — deletes nothing";
      report = "cache-sweep-rehearsal.json";
      execute = false;
    };

    systemd.user.services.ronoctua-cache-sweep = sweepUnit {
      what = "sweep cached files nothing has used";
      report = "cache-sweep.json";
      execute = true;
    };

    # AND THE TRASH, which is NOT a cache — what is in there is the only copy,
    # which is why it has its own binary, its own number (four days, not forty)
    # and its own writable path. The two live in one module because they are the
    # same question asked of two directories: what may the machine throw away by
    # itself, and after how long.
    systemd.user.services.ronoctua-empty-trash-rehearsal = trashUnit {
      what = "rehearse emptying the trash — deletes nothing";
      report = "trash-expire-rehearsal.json";
      execute = false;
    };

    systemd.user.services.ronoctua-empty-trash = trashUnit {
      what = "empty the trash of what has been in it long enough";
      report = "trash-expire.json";
      execute = true;
    };
  };
}
