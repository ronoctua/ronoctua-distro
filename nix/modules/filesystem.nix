# The filesystem looks after itself: what btrfs can do on its own, and what a
# snapshot may never hold.
#
# # Why a module of its own
#
# Because every step here is a question about the filesystem and not about the
# store: `nix/modules/store.nix` decides what Nix keeps, this decides where the
# bytes may hide once Nix has let go of them. A snapshot of a root that holds
# `/nix` keeps every store path a collection frees, so collecting garbage
# returns almost nothing — and a machine can lose most of its disk that way
# without anything saying so.
#
# # The four steps
#
# 1. **`~/.cache` is a subvolume of its own**, for every normal user, made at
#    boot before anyone logs in. A btrfs snapshot never descends into a nested
#    subvolume, so snapshots of `/home` stop carrying what the cache sweep
#    deletes — and the sweep's root is exactly that subvolume, so it crosses
#    nothing (`crates/ronoctua-cache-sweep/src/lib.rs`). A cache that has to
#    be MOVED is moved only while the machine is booting, at most once per
#    `keepDays` when it fails, and never for longer than the unit allows.
#    Other modules add folders of the same kind (`ownSubvolumes`): where
#    containers are on, `~/.local/share/containers` — every image layer the
#    person pulled, which a snapshot of `/home` must not pin either.
# 2. **The kernel reclaims unallocated space by itself.** From 6.11 btrfs can
#    move nearly empty data block groups back to unallocated space as it runs
#    short (`dynamic_reclaim`, `periodic_reclaim`), which is what a manual
#    `btrfs balance` did. Off upstream; switched on here. An older kernel simply
#    has no such file, and the tmpfiles line writes nothing.
# 3. **A monthly scrub**, nixpkgs' own, on the one filesystem however many
#    subvolumes it has. The only clock here, and on purpose: bit rot has no
#    event.
# 4. **No snapshot may hold the store or a cache.** Any snapper or btrbk target
#    at or under `/nix`, holding `/nix` (a snapshot of `/` on a machine where
#    `/nix` is not its own mount), at a `.cache` or at the containers'
#    storage, is refused at evaluation.
#
# Machines without btrfs get none of the first three and are told nothing:
# the predicate below is false and the steps are not there. The snapshot rule
# holds everywhere.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.filesystem;

  # EVERY NORMAL USER, from the configuration and never a literal (inv. 16).
  people = lib.filterAttrs (_: u: u.isNormalUser) config.users.users;
  cacheUnit = name: "ronoctua-cache-subvolume-${name}";
  # THE UNIT FOR EVERY FOLDER THAT IS A SUBVOLUME OF ITS OWN: the cache's
  # keeps the name it always had, so a machine's journal and the checks read
  # the same unit before and after the list grew.
  subvolumeUnit =
    key: name: if key == "cache" then cacheUnit name else "ronoctua-${key}-subvolume-${name}";
  # WHERE A FAILED CONVERSION LEAVES ITS NOTE, one file per person (and per
  # folder, past the cache's).
  noteDir = "ronoctua-cache-subvolume";
  noteOf = key: name: "/var/lib/${noteDir}/${name}${lib.optionalString (key != "cache") ".${key}"}";
  # `keepDays` — how long a failed conversion waits (see the script).
  declared = import ../../assets/cache.nix;

  # THE CONVERSION, AND EVERY STATE IT CAN BE LEFT IN.
  #
  # A btrfs subvolume's root directory is always inode 256; that inode on a
  # btrfs path is the whole test for "already done", so the unit keeps no state
  # file of its own and a machine that was converted by hand is left alone.
  #
  # The swap is `mv --exchange` (renameat2 with RENAME_EXCHANGE): one atomic
  # step, so a crash leaves either the old directory in place with a half copy
  # beside it, or the new subvolume in place with the old directory beside it.
  # The next boot tells the two apart by which of them is the subvolume, and
  # finishes or undoes.
  #
  # `--reflink=always` and never `auto`: a reflink copy writes almost no data,
  # and a copy that could not be one would write the whole cache again, at
  # boot, on a disk that may be the reason this exists. A file that cannot be
  # reflinked (a no-copy-on-write one) makes the copy fail; then the half copy
  # goes and the cache stays a directory, said in the journal.
  #
  # AND A FAILURE IS NOT RETRIED AT EVERY BOOT. The two likely ones — a
  # no-copy-on-write file, a disk too full for the metadata of a copy — are
  # still there at the next boot, and every boot would pay the copy again
  # before anyone could log in. So the attempt leaves a note BEFORE it starts
  # (`/var/lib/ronoctua-cache-subvolume/<user>`), taken away only by success:
  # an attempt that failed, or that was cut short by the unit's time limit or
  # a power cut, is not tried again for `keepDays` — the sweep's own
  # retention, after which the cache that refused has turned over.
  #
  # ONLY WHILE BOOTING. A switch that brings this unit in finds people logged
  # in, or about to be, and a copy of a whole cache inside the switch is a
  # switch that hangs; `systemctl is-system-running` says `starting` only
  # during boot. Making a missing `~/.cache` a subvolume moves nothing, and
  # happens at any time.
  cacheSubvolume = pkgs.writeShellApplication {
    name = "ronoctua-cache-subvolume";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.btrfs-progs
      pkgs.systemd
      pkgs.util-linux
      pkgs.findutils
      pkgs.gawk
    ];
    text = ''
      if [ $# -ne 6 ]; then
        echo "ERROR: usage: ronoctua-cache-subvolume <home> <folder> <user> <group> <note> <retry-days>" >&2
        exit 2
      fi
      home=$1 folder=$2 user=$3 group=$4 note=$5 retry_days=$6
      if ! [[ $retry_days =~ ^[1-9][0-9]*$ ]]; then
        echo "ERROR: <retry-days> must be a whole number of days, not '$retry_days'" >&2
        exit 2
      fi
      # A folder under the home, named relative to it and going nowhere else.
      if ! [[ $folder =~ ^[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*$ ]] || [[ /$folder/ == */../* ]] || [[ /$folder/ == */./* ]]; then
        echo "ERROR: <folder> must be a plain path under the home, not '$folder'" >&2
        exit 2
      fi
      cache=$home/$folder
      moving=$cache.subvolume-in-progress

      on_btrfs() { [ "$(stat --file-system --format=%T -- "$1")" = btrfs ]; }
      subvolume() { [ -d "$1" ] && [ ! -L "$1" ] && on_btrfs "$1" && [ "$(stat --format=%i -- "$1")" = 256 ]; }

      if [ ! -d "$home" ] || [ -L "$home" ]; then
        echo "$home is not a directory; nothing to do"
        exit 0
      fi
      if ! on_btrfs "$home"; then
        echo "$cache stays a directory: $home is not on btrfs"
        exit 0
      fi

      # AN INTERRUPTED RUN, FIRST.
      if [ -e "$moving" ] || [ -L "$moving" ]; then
        if [ -L "$moving" ] || [ ! -d "$moving" ]; then
          echo "$moving is not what an interrupted run leaves; left as it is"
          exit 0
        elif subvolume "$cache" && ! subvolume "$moving"; then
          rm -rf --one-file-system -- "$moving"
          echo "finished an interrupted conversion: the old copy of $cache is gone"
        elif ! subvolume "$cache" && subvolume "$moving"; then
          btrfs subvolume delete -- "$moving"
          echo "undid an interrupted conversion: the half copy of $cache is gone"
        else
          echo "$moving is not what an interrupted run leaves; left as it is"
          exit 0
        fi
      fi

      if [ -L "$cache" ]; then
        echo "$cache is a link; left as it is"
        exit 0
      fi
      if [ -e "$cache" ] && [ ! -d "$cache" ]; then
        echo "$cache is not a directory; left as it is"
        exit 0
      fi
      if subvolume "$cache"; then
        exit 0
      fi

      # NOTHING THERE YET: the subvolume is simply made, logged in or not —
      # there is nothing of anybody's to move. Every folder missing on the
      # way is made the PERSON's (a `~/.local` made by root would be a home
      # its owner cannot write), and a link on the way is somebody's
      # arrangement: left as it is.
      if [ ! -e "$cache" ]; then
        at=$home
        IFS=/ read -r -a parts <<< "$(dirname -- "$folder")"
        for part in "''${parts[@]}"; do
          if [ "$part" = . ]; then
            continue
          fi
          at=$at/$part
          if [ -L "$at" ]; then
            echo "$at is a link; $cache is left as it is"
            exit 0
          fi
          if [ ! -e "$at" ]; then
            mkdir -- "$at"
            chown -- "$user:$group" "$at"
          fi
        done
        if btrfs subvolume create -- "$cache"; then
          chown -- "$user:$group" "$cache"
          chmod 0700 -- "$cache"
          echo "$cache is a subvolume of its own"
        else
          echo "$cache could not be made a subvolume; the next boot tries again"
        fi
        exit 0
      fi

      # ONLY WHILE BOOTING — see above.
      case "$(systemctl is-system-running 2> /dev/null || true)" in
        initializing | starting) ;;
        *)
          echo "$cache becomes a subvolume at the next boot: the machine is not booting"
          exit 0
          ;;
      esac
      # A FAILED ATTEMPT WAITS, and says since when.
      if [ -e "$note" ] && [ -z "$(find "$note" -maxdepth 0 -mtime +"$retry_days" 2> /dev/null)" ]; then
        echo "$cache stays a directory: $(cat -- "$note" 2> /dev/null || true); tried again $retry_days days after that"
        exit 0
      fi

      # NEVER UNDER A RUNNING SESSION. At boot nobody is; a lingering user's
      # manager may be.
      uid=$(id -u -- "$user")
      if systemctl is-active --quiet "user@$uid.service"; then
        echo "$user is logged in; $cache becomes a subvolume at the next boot"
        exit 0
      fi
      if mountpoint -q -- "$cache"; then
        echo "$cache is a mount point; left as it is"
        exit 0
      fi
      # A MOUNT OR A SUBVOLUME INSIDE is somebody else's arrangement, and a
      # copy would either cross it or flatten it.
      if [ -n "$(findmnt --raw --noheadings --output TARGET | awk -v p="$cache/" 'index($0, p) == 1' | head -n 1)" ]; then
        echo "$cache holds a mount point; left as it is"
        exit 0
      fi
      if [ -n "$(find "$cache" -xdev -mindepth 1 -type d -inum 256 -print -quit)" ]; then
        echo "$cache holds a subvolume of its own; left as it is"
        exit 0
      fi

      # THE NOTE FIRST: an attempt cut short leaves it behind as much as one
      # that failed.
      mkdir -p -- "$(dirname -- "$note")"
      echo "the conversion that began on $(date --iso-8601=seconds) did not finish" > "$note"
      btrfs subvolume create -- "$moving"
      if ! cp -a --reflink=always -- "$cache/." "$moving/"; then
        btrfs subvolume delete -- "$moving"
        echo "the copy on $(date --iso-8601=seconds) could not be made without writing the data again (a file that cannot be reflinked, or no room)" > "$note"
        echo "$cache could not be copied without writing its data again (a file that cannot be reflinked); it stays a directory"
        exit 0
      fi
      chown --reference="$cache" -- "$moving"
      chmod --reference="$cache" -- "$moving"
      touch --reference="$cache" -- "$moving"
      if ! mv --exchange --no-target-directory -- "$moving" "$cache"; then
        btrfs subvolume delete -- "$moving"
        echo "the swap on $(date --iso-8601=seconds) failed" > "$note"
        echo "$cache could not be swapped for its copy; it stays a directory"
        exit 0
      fi
      rm -rf --one-file-system -- "$moving"
      rm -f -- "$note"
      echo "$cache is a subvolume of its own now; snapshots of $home no longer carry it"
    '';
  };

  # ONE UNIT PER PERSON AND FOLDER.
  unitFor = key: folder: name: u: {
    description = "Makes ${u.home}/${folder} a btrfs subvolume of its own";
    # BEFORE ANY LOGIN: `systemd-user-sessions.service` is what lets
    # logins in (the getty, a display manager and every user manager
    # wait for it), so nothing of the person's has a file open in there.
    # With the default dependencies and not before `sysinit.target`: a
    # home mounted late — over the network, by a device that comes up
    # late — would otherwise be an ordering cycle at boot, and the rest
    # of the boot does not wait for a copy it has no stake in.
    wantedBy = [ "multi-user.target" ];
    before = [ "systemd-user-sessions.service" ];
    unitConfig.RequiresMountsFor = [ u.home ];
    # A switch does not re-run it under a live session; the boot does.
    restartIfChanged = false;
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${cacheSubvolume}/bin/ronoctua-cache-subvolume ${
        lib.escapeShellArgs [
          u.home
          folder
          name
          u.group
          (noteOf key name)
          (toString declared.keepDays)
        ]
      }";
      # NEVER LONGER THAN THIS before the logins it holds back: a copy cut
      # short is undone at the next boot and not tried again for
      # `keepDays` (the note above).
      TimeoutStartSec = "10min";
      StateDirectory = noteDir;
      # As root, because every file keeps its owner; and the home is the
      # only thing it may write, beside its note.
      ProtectSystem = "strict";
      ReadWritePaths = [ "-${u.home}" ];
      PrivateNetwork = true;
      NoNewPrivileges = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
    };
  };

  # -----------------------------------------------------------------
  # What a snapshot is OF
  # -----------------------------------------------------------------
  # A path without doubled or trailing slashes, so `/nix/`, `//nix` and `/nix`
  # are one answer.
  tidy =
    path:
    "/"
    + lib.concatStringsSep "/" (
      lib.filter (part: part != "" && part != ".") (lib.splitString "/" path)
    );

  # WHICH MOUNT A SUBVOLUME NAME IS, from `subvol=` in `fileSystems`: btrbk
  # names subvolumes as the top level sees them (`@nix`), and that is only a
  # path once it is found among the mounts.
  subvolOf =
    fs:
    let
      named = lib.filter (o: lib.hasPrefix "subvol=" o) fs.options;
    in
    if named == [ ] then null else lib.removePrefix "/" (lib.removePrefix "subvol=" (lib.head named));
  mountOf =
    name:
    let
      found = lib.filter (fs: subvolOf fs == lib.removePrefix "/" name) (
        lib.attrValues config.fileSystems
      );
    in
    if found == [ ] then null else (lib.head found).mountPoint;
  resolve =
    volume: name:
    let
      mounted = mountOf name;
    in
    if mounted != null then
      tidy mounted
    else if lib.hasPrefix "/" name then
      tidy name
    else
      tidy "${volume}/${name}";
  namesIn =
    sub:
    if lib.isString sub then
      [ sub ]
    else if lib.isAttrs sub then
      lib.attrNames sub
    else
      [ ];

  snapperTargets = lib.mapAttrsToList (name: c: {
    target = tidy c.SUBVOLUME;
    by = "services.snapper.configs.${name}";
  }) config.services.snapper.configs;

  btrbkTargets = lib.concatLists (
    lib.mapAttrsToList (
      instance: i:
      let
        settings = i.settings;
        volumes = if lib.isAttrs (settings.volume or null) then settings.volume else { };
      in
      lib.concatLists (
        lib.mapAttrsToList (
          volume: v:
          map (name: {
            target = resolve volume name;
            by = "services.btrbk.instances.${instance}: volume ${volume}, subvolume ${name}";
          }) (namesIn (if lib.isAttrs v then v.subvolume or null else null))
        ) volumes
      )
      ++ map (name: {
        target = resolve "/" name;
        by = "services.btrbk.instances.${instance}: subvolume ${name}";
      }) (namesIn (settings.subvolume or null))
    ) config.services.btrbk.instances
  );

  # WHY A TARGET IS REFUSED, or null. The store because a snapshot of it pins
  # every path a collection frees; a cache because it pins every file the sweep
  # deletes. `/` is refused only where `/nix` is inside it — on the distro's
  # own layout `@nix` is a top-level subvolume, and a snapshot of `@root` never
  # contains it.
  refusal =
    target:
    if target == "/nix" || lib.hasPrefix "/nix/" target then
      "it is the Nix store"
    else if target == "/" && !(config.fileSystems ? "/nix") then
      "/nix is not a mount of its own here, so a snapshot of / holds the Nix store"
    else if lib.elem ".cache" (lib.splitString "/" target) then
      "it is a cache"
    else if lib.hasInfix "/.local/share/containers/" "${target}/" then
      "it is the containers' storage"
    else
      null;

  targets = snapperTargets ++ btrbkTargets;
  refused = lib.filter (t: t.why != null) (map (t: t // { why = refusal t.target; }) targets);
in
{
  options.ronoctua.filesystem = {
    btrfs = lib.mkOption {
      type = lib.types.bool;
      default = lib.any (fs: fs.fsType == "btrfs") (lib.attrValues config.fileSystems);
      defaultText = lib.literalMD "whether any of `fileSystems` is btrfs";
      description = ''
        Whether this machine has btrfs, and so the steps only btrfs can take: the
        cache subvolumes, the kernel's own reclaim of unallocated space and the
        scrub. The one predicate every one of them reads.
      '';
    };

    ownSubvolumes = lib.mkOption {
      type = lib.types.attrsOf (lib.types.strMatching "[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*");
      default = { };
      example = {
        containers = ".local/share/containers";
      };
      description = ''
        Folders under every person's home that are btrfs subvolumes of their
        own, by a short name (the unit is `ronoctua-<name>-subvolume-<user>`):
        what a snapshot of `/home` must not hold. `cache` (`.cache`) is always
        there; a module adds its own (`containers.nix`).
      '';
    };

    # THE ANSWERS, so a check reads what the assertion reads instead of forcing
    # every message of the whole system to find ours.
    snapshotTargets = lib.mkOption {
      type = lib.types.listOf lib.types.attrs;
      readOnly = true;
      internal = true;
      default = targets;
      description = "Every snapshot target this machine declares, resolved to a path.";
    };

    refusedSnapshots = lib.mkOption {
      type = lib.types.listOf lib.types.attrs;
      readOnly = true;
      internal = true;
      default = refused;
      description = "The snapshot targets this machine may not have, and why.";
    };
  };

  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion = refused == [ ];
          message =
            "ronoctua.filesystem: a snapshot may not hold the Nix store or a cache — it would pin "
            + "every path a collection frees and every file the sweep deletes. Refused: "
            + lib.concatMapStringsSep "; " (t: "${t.target} (${t.by}): ${t.why}") refused;
        }
      ];

      # One scrub per filesystem (nixpkgs dedups by device), monthly, at idle.
      services.btrfs.autoScrub.enable = lib.mkDefault cfg.btrfs;

      # The cache's, always.
      ronoctua.filesystem.ownSubvolumes.cache = ".cache";
    }

    (lib.mkIf cfg.btrfs {
      # DATA ONLY; metadata keeps the kernel's default. A glob that matches
      # nothing (a kernel before 6.11) is no write and no error.
      systemd.tmpfiles.rules = [
        "w /sys/fs/btrfs/*/allocation/data/dynamic_reclaim - - - - 1"
        "w /sys/fs/btrfs/*/allocation/data/periodic_reclaim - - - - 1"
      ];

      systemd.services = lib.listToAttrs (
        lib.concatLists (
          lib.mapAttrsToList (
            key: folder:
            lib.mapAttrsToList (name: u: {
              name = subvolumeUnit key name;
              value = unitFor key folder name u;
            }) people
          ) cfg.ownSubvolumes
        )
      );
    })
  ];
}
