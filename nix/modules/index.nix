# The super index of fast-access: one unit writes it, every door reads it.
#
# A function of the extracted binary, so the unit's closure is the indexer —
# tantivy, the preview's text readers, the Nix index's reader — and never the
# desktop (`flake.nix` imports it with `rust.index`, the way `ronoctua.brain`
# reaches the brain's module).
#
# WHY A UNIT OF ITS OWN
# ---------------------
# It walks the person's home and reads what is written in their documents:
# minutes of disk and CPU on a first run, then inotify and almost nothing. A
# thread of the footer doing that would be the footer competing with the
# person for the disk; here it is a process the kernel schedules last
# (`Nice=19`, `IOSchedulingClass=idle`, `CPUWeight=idle`) inside a memory
# ceiling of its own (`assets/cache.nix`). The index is a cache: killed or
# restarted, the unit reconciles against the disk and loses nothing.
#
# WHEN IT RUNS
# ------------
# Not by itself: the brain starts it (the module that keeps the index), against the gate
# the person configured — a first walk of a home is not something to begin
# on battery in the middle of a call. There is no `wantedBy` here on purpose.
#
# WHAT IT MAY TOUCH
# -----------------
# It reads the home and writes one folder, `%C/ronoctua/index`
# (`CacheDirectory`, which systemd makes and opens for writing; everything
# else is read-only by `ProtectSystem=strict` and `ProtectHome=read-only`).
# No network.
{ index }:
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.index;
  # THE LIMITS, from `assets/cache.nix` through the one list the checks run
  # the same binary with.
  limits = import ../index-args.nix;

  # THE PACKAGES ONE CAN INSTALL, at a path that follows the running system.
  # The unit lives across switches (the brain starts it, nobody restarts it),
  # so a store path in its environment would be the list of the generation it
  # started in; this link is replaced by the switch, and the switch moves
  # `/run/current-system`, which is what makes the unit read it again.
  nixIndex = "ronoctua/nix-index.tsv";
in
{
  options.ronoctua.index.enable =
    lib.mkEnableOption "the super index of fast-access: names, contents, manual pages, packages and videos"
    // {
      default = true;
    };

  config = lib.mkIf cfg.enable {
    environment.etc.${nixIndex}.source = "${ronoctua.softwareIndex}/index.tsv";

    # ONE WATCH PER FOLDER WALKED. The kernel's default scales with memory and
    # is a few tens of thousands on a small machine; a home with its projects
    # is more folders than that, and past the limit the unit says so once and
    # catches the rest at its next reconcile. A watch costs about a kilobyte of
    # kernel memory while it exists, and only a folder that exists has one.
    #
    # NIXPKGS ALREADY RAISES IT (`nixos/modules/config/sysctl.nix`, a
    # `mkDefault` of 524 288), and a second `mkDefault` here — even of the same
    # number — is an option defined twice, which refuses the whole system. So
    # this module ASKS instead of setting: the number it needs, held.
    assertions = [
      {
        assertion = (config.boot.kernel.sysctl."fs.inotify.max_user_watches" or 0) >= 524288;
        message = "ronoctua.index: `fs.inotify.max_user_watches` is below 524288; the super index watches one folder each";
      }
    ];

    systemd.user.services.ronoctua-index = {
      description = "ronoctua — the super index of fast-access";
      environment = {
        RONOCTUA_NIX_INDEX = "/etc/${nixIndex}";
        # A spreadsheet's text is read by the preview's own reader
        # (`ronoctua-inspect`), which asks `xls2csv` for an `.xls` — the
        # program the footer is given in `nix/modules/media.nix`. A unit
        # inherits nothing of the session: without it an `.xls` would be
        # found by its name only.
        RONOCTUA_XLS2CSV = "${pkgs.catdoc}/bin/xls2csv";
      };
      serviceConfig = {
        ExecStart = lib.concatStringsSep " " (
          [
            "${index}/bin/ronoctua-index"
            "serve"
            # THE FOLDER IS NAMED, and it is the one `CacheDirectory` below
            # opens for writing: a walk and a sandbox that disagreed about it
            # would fail in a way that reads as a bug in the walk.
            "--dir %C/ronoctua/index"
          ]
          ++ map lib.escapeShellArg limits.arguments
        );
        CacheDirectory = "ronoctua/index";
        Restart = "on-failure";
        RestartSec = 30;

        # LAST IN LINE for the processor and the disk: it is catching up on
        # what the person did, never in their way while they do it.
        Nice = 19;
        IOSchedulingClass = "idle";
        CPUWeight = "idle";
        MemoryMax = "${toString limits.memoryMiB}M";
        # `background.slice`: the user manager's slice for work nobody waits
        # on, apart from the session and the apps.
        Slice = "background.slice";

        ProtectSystem = "strict";
        ProtectHome = "read-only";
        PrivateDevices = true;
        PrivateNetwork = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
        NoNewPrivileges = true;
        RestrictSUIDSGID = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictNamespaces = true;
        SystemCallFilter = [ "@system-service" ];
      };
    };
  };
}
