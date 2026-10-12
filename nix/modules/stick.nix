# The installer on a USB stick — the one unit that writes a whole drive, and
# the folder the person stages what it writes.
#
# WHO STARTS IT, AND HOW NARROW IT IS
# -----------------------------------
# The sudo door (`ronoctua_security::authority`, verb `stick-write`): the
# root helper runs `systemctl start` on exactly this unit, after `sudo -n` or
# the password the person typed into the stick view. There is no polkit rule
# for it, as there is none for the file manager's root unit
# (`nix/modules/files.nix` says why). `stick-root-is-narrow` holds the three
# names together (the crate's `WRITE_UNIT`, the helper's, this module's) and
# refuses a polkit rule that names it.
#
# The unit is started BEFORE the download, the moment the person said yes, so
# the password is asked once and up front; it then waits for the queue's
# `ready` (`ronoctua_stick::staging`) — on inotify, with a ceiling — and
# validates everything it reads itself: the request (the person's, `0600`,
# no link), the image (a regular file the person owns, so root never copies
# another file of the machine onto a stick the person can read), and the
# drive as the KERNEL sees it right before the write (`ronoctua_stick::device`).
#
# AND WHAT THE KERNEL LETS IT TOUCH
# ---------------------------------
# `DevicePolicy=closed` with `block-sd` and `block-mmc` alone: whatever the
# binary decided, the device controller refuses it the machine's NVMe or
# virtio disk. One capability, `CAP_DAC_READ_SEARCH`, to read the person's
# staged files and image through their `0700` folders; no network; the
# system read-only but for its own folder in `/run`.
{ stickWrite }:
{
  config,
  lib,
  ...
}:

let
  user = config.ronoctua.user;

  # THE UNIT'S NAME IS FIXED — a free instance string would be a free
  # argument to a root helper. `ronoctua_stick::WRITE_UNIT` says the same.
  unit = "ronoctua-stick-write.service";

  # Where the person stages the write (theirs, `0700`), inside the folder the
  # unit answers in (root's, `0755`): a file root writes into a folder the
  # person owns is one symlink away from any file of the machine.
  folder = "/run/ronoctua-stick";
  staging = "${folder}/staged";
in
{
  config = {
    systemd.tmpfiles.rules = [
      "d ${folder} 0755 root root -"
      "d ${staging} 0700 ${user} ${config.users.users.${user}.group} -"
    ];

    systemd.services.${lib.removeSuffix ".service" unit} = {
      description = "Writes the distro's installer to the USB stick the person chose";
      # Started by the sudo door only: nothing wants it, nothing orders it.
      # NO START LIMIT: each start is one gesture of the person, and the unit
      # never restarts by itself — systemd's default of five starts in ten
      # seconds refused a sixth gesture as `start-limit-hit`.
      startLimitIntervalSec = 0;
      serviceConfig = {
        # `exec`: the door's `systemctl start` returns once the writer runs,
        # never after an hour of download and a write.
        Type = "exec";
        TimeoutStartSec = "infinity";
        # The writer gives up waiting for `ready` after twelve hours on its
        # own; this is the floor under that, with the write's time on top.
        RuntimeMaxSec = "14h";
        ExecStart = "${stickWrite}/bin/ronoctua-stick-write ${staging}";
        # HOWEVER IT ENDED — stopped, killed at `RuntimeMaxSec` — its last
        # answer is made final: a `progress.json` left saying "writing" would
        # have every view refuse a new write until the machine restarts.
        ExecStopPost = "${stickWrite}/bin/ronoctua-stick-write --stopped ${staging}";
        DevicePolicy = "closed";
        DeviceAllow = [
          "block-sd rw"
          "block-mmc rw"
        ];
        CapabilityBoundingSet = [ "CAP_DAC_READ_SEARCH" ];
        AmbientCapabilities = [ ];
        NoNewPrivileges = true;
        PrivateNetwork = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
        ProtectSystem = "strict";
        # `-`: a missing folder is the writer's refusal, said in its journal,
        # not a unit that dies in its namespace before it runs.
        ReadWritePaths = [ "-${folder}" ];
        ProtectHome = "read-only";
        PrivateTmp = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectHostname = true;
        ProtectClock = true;
        RestrictRealtime = true;
        RestrictNamespaces = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        UMask = "0022";
      };
    };
  };
}
