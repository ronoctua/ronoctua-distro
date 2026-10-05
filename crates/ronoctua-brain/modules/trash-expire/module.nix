# The trash is emptied of what has been in it long enough: the two user units
# the brain starts.
#
# NOT A CACHE, and the differences from the sweep's units are the point:
# another binary, another number, and `%S` rather than `%C` as the one
# writable path — a sweep that could reach the trash would be a cache cleaner
# deleting the only copy of somebody's file. The two used to live in one file
# because they are the same question asked of two directories — what may the
# machine throw away by itself, and after how long — and each is its own
# brain module now, so each can be switched off alone.
#
# It runs as the PERSON, asks nobody for anything, and writes only under the
# data home (where the trash is), the mounted drives' own trashes
# (`/run/media/<person>`) and the brain's report directory. No polkit
# rule, and there must never be one.
#
# ONE SWITCH: `ronoctua.brain.modules.<id>.enable = false` takes these units
# away with the module (the gate `nix/brain-modules.nix` puts around this
# file) and tells the brain, which draws the row refusing. `trashDays` stays
# the catalogue's option (`nix/modules/cache.nix`), and so does
# `ronoctua.cache.enable` — which is only that switch's DEFAULT, and never a
# second gate here: a second gate took the units away without telling the
# brain, which went on starting them.
#
# THE NAMES ARE THE MANIFEST'S, and the reports are this folder's name — the
# runner watches `<id>.json` and `<id>-rehearsal.json`.
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

  # ONE UNIT, WRITTEN ONCE: the rehearsal is the one nobody reads, and it has
  # to behave exactly like the real one to be worth anything.
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
            # THE DATA HOME, the same `%S` the sandbox below opens — the
            # sweep's reason for naming its root, and the trash lives there.
            "--data-home %S"
            # AND EVERY MOUNTED DRIVE'S OWN TRASH, where the file manager puts
            # what is thrown away on a stick: the same number of days.
            "--media /run/media/%u"
            # THE REPORT THE BRAIN WATCHES, where the runner looks; this
            # module's own check reads the unit as the machine gets it.
            "--report %t/ronoctua-brain/${report}"
            # COUNTED FROM THE `DeletionDate` every note carries, never from
            # the file's own stamps: thrown away this morning is a morning.
            "--days ${toString cfg.trashDays}"
          ]
          # AND ONLY THE REAL ONE ACTS.
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
          # `-`: a machine where no drive was ever mounted has no such folder,
          # and a missing `ReadWritePaths=` stops the unit before it runs
          # (226/NAMESPACE).
          "-/run/media/%u"
        ];
        # MADE BEFORE THE SANDBOX, or a missing `%t/ronoctua-brain` fails the
        # unit with 226/NAMESPACE before the program runs (measured).
        RuntimeDirectory = "ronoctua-brain";
        RuntimeDirectoryPreserve = "yes";
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
  # TWO UNITS AND NOT A FLAG, in ONE attrset: two names computed into two
  # separate `systemd.user.services.<name>` lines is an evaluation error.
  config = {
    systemd.user.services = {
      ${service manifest.units.rehearse} = trashUnit {
        what = "rehearse emptying the trash — deletes nothing";
        report = "${id}-rehearsal.json";
        execute = false;
      };
      ${service manifest.units.run} = trashUnit {
        what = "empty the trash of what has been in it long enough";
        report = "${id}.json";
        execute = true;
      };
    };
  };
}
