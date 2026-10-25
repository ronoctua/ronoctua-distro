# Impermanence — section 7.2.
#
# "Designed, not switched on yet. Create @persist from day one and route state
# there. The root wipe stays off until milestone 7."
#
# This module does the routing NOW. The wipe arrives in milestone 7 by becoming
# an option, not by rewriting the module — which is the point of routing from
# the start.
{
  config,
  options,
  lib,
  pkgs,
  utils,
  ...
}:

let
  cfg = config.ronoctua;

  # WHAT THE HOME USED TO SEND TO /persist, until 2026-09-30.
  #
  # Three directories of the person's were bound from `/persist/home/<user>`
  # over their own place in the home, and it went wrong on the first reboot:
  # `tmpfiles` made each of them again as `0755 root root`, the bind laid the
  # root-owned copy over the person's, and from then on NO preference of the
  # footer could be written — the bars' mode, the theme's answers, the
  # notification history, Syncthing's state — while the footer said nothing
  # but a line in a log. The home is `@home`, which the wipe never touches
  # (section 7.2), so none of them needed persisting at all: they leave the
  # list, an assertion keeps any other line under `/home` out of it, and the
  # unit below gives what was kept in `/persist/home` back to the home, with
  # its owner.
  legacyHome = [
    ".config/ronoctua"
    ".local/state/ronoctua"
    ".local/state/syncthing"
  ];

  # THE RETURN, once: what `/persist/home/<user>` holds of those three is
  # copied into the home — the persisted copy wins, since it is the one the
  # person was writing into while the bind stood — the directories and every
  # root-owned parent on the way (`~/.config`, `~/.local`, `~/.local/state`,
  # which the bind's mount points made as root) given back to the person, and
  # `/persist/home` renamed to `/persist/home-returned-<date>`: never deleted,
  # the recovery window the wipe keeps for the same reason.
  #
  # AT BOOT ONLY. Every switch restarts the active targets, so the first
  # switch to this configuration STARTS the new unit under a live session —
  # the footer writing `modeline.json`, Syncthing its index — and a copy laid
  # over files in use lets the persisted copy win over a newer one. So the
  # program asks the manager where it is: `initializing` or `starting` is the
  # boot, before `systemd-user-sessions`; anything else is a switch, and it
  # returns at once, leaving everything for the next boot.
  #
  # A directory that is STILL a mount point is left alone and the rename
  # waits: copying a bind onto itself is `cp` refusing "the same file". The
  # next boot, with no bind, does it.
  homeReturn = pkgs.writeShellApplication {
    name = "ronoctua-persist-home-return";
    runtimeInputs = with pkgs; [
      coreutils
      util-linux
      config.systemd.package
    ];
    text = ''
      root=/persist/home
      if [ ! -d "$root" ]; then
        exit 0
      fi
      state=$(systemctl is-system-running || true)
      case "$state" in
        initializing | starting) ;;
        *)
          echo "ronoctua: the system is $state, not booting; the next boot returns the home"
          exit 0
          ;;
      esac
      user=${lib.escapeShellArg cfg.user}
      home=/home/$user
      group=$(id -gn "$user")
      waiting=0
      for d in ${lib.escapeShellArgs legacyHome}; do
        kept="$root/$user/$d"
        if [ ! -d "$kept" ]; then
          continue
        fi
        if mountpoint -q "$home/$d"; then
          echo "ronoctua: $home/$d is still a bind; the next boot returns it"
          waiting=1
          continue
        fi
        mkdir -p "$home/$d"
        cp -a --reflink=auto "$kept/." "$home/$d/"
        chown -R "$user:$group" "$home/$d"
        echo "ronoctua: returned $d to $home"
      done
      # THE PARENTS THE MOUNT POINTS MADE, given back when they are root's.
      for parent in .config .local .local/state; do
        if [ -d "$home/$parent" ] && [ "$(stat -c %u "$home/$parent")" = 0 ]; then
          chown "$user:$group" "$home/$parent"
        fi
      done
      if [ "$waiting" = 1 ]; then
        exit 0
      fi
      target="/persist/home-returned-$(date +%Y%m%d%H%M%S)"
      n=0
      while [ -e "$target" ]; do
        n=$((n + 1))
        target="/persist/home-returned-$(date +%Y%m%d%H%M%S)-$n"
      done
      mv "$root" "$target"
      echo "ronoctua: /persist/home kept as $target"
    '';
  };

  # THE SOURCE OF EVERY BIND, before the bind.
  #
  # A bind from a directory that does not exist is a mount that fails, and
  # the first boot after a line is added to `directories` is exactly that:
  # `tmpfiles` makes `/persist<dir>` too late, after `local-fs.target`. And a
  # bind from an EMPTY directory hides what the root already held there — the
  # browser extension's key, made on an earlier boot, would vanish under it
  # and be made again: a new id, another extension to Chrome.
  #
  # THE SWITCH MAKES THE EMPTY ONE. A `nixos-rebuild switch` that adds a line
  # reactivates `sysinit` first, so `systemd-tmpfiles-resetup` makes
  # `/persist<dir>` (the `d` rule below) — empty — before any unit of the new
  # configuration starts. So a `/persist` copy that is missing OR EMPTY is
  # seeded from what the root holds at that path (nothing, on a wiped root);
  # one that holds anything is never touched — it is the one the wipe keeps,
  # and the newer. A path that is already a mount point (a switch, with the
  # binds up) is left as it is.
  #
  # THROUGH A DRAFT: the copy is made beside the target and renamed over the
  # empty directory, so a copy cut short (a full disk) leaves the `/persist`
  # side empty — seeded again next time — and never half a key. The
  # directory's owner and mode go with it, which `tmpfiles`' `-` would not
  # give (`0755 root`).
  #
  # `<root> <directory>...`: the root the system is mounted at — `/` in stage
  # 2, `/sysroot` in the initrd (`initrdDirectories` below) — and the
  # directories to seed, as the running system names them.
  seed = pkgs.writeShellApplication {
    name = "ronoctua-persist-seed";
    runtimeInputs = with pkgs; [
      coreutils
      findutils
      util-linux
    ];
    text = ''
      if [ $# -lt 1 ]; then
        echo "ERROR: usage: ronoctua-persist-seed <root> [<directory>...]" >&2
        exit 2
      fi
      root=''${1%/}
      shift
      for d in "$@"; do
        here="$root$d"
        kept="$root/persist$d"
        if [ -e "$kept" ] && [ -n "$(find "$kept" -mindepth 1 -maxdepth 1 -print -quit)" ]; then
          continue
        fi
        if [ ! -d "$here" ] || mountpoint -q "$here"; then
          mkdir -p "$kept"
          continue
        fi
        if [ -z "$(find "$here" -mindepth 1 -maxdepth 1 -print -quit)" ]; then
          mkdir -p "$kept"
          continue
        fi
        draft="$kept.seeding"
        rm -rf "$draft"
        mkdir -p "$(dirname "$kept")"
        cp -a --reflink=auto "$here" "$draft"
        if [ -e "$kept" ]; then
          rmdir "$kept"
        fi
        mv "$draft" "$kept"
        echo "ronoctua: $d seeded into /persist"
      done
    '';
  };

  # THE BINDS THE INITRD MOUNTS, and why the initrd has a seed of its own.
  #
  # NixOS mounts some paths before it switches root (`utils.fsNeededForBoot`:
  # `/var/lib/nixos`, where the activation keeps every user's and group's id,
  # is one of `directories`), and the initrd reads their lines from the same
  # fstab — `x-systemd.requires=ronoctua-persist-seed.service` included. With
  # no unit of that name in the initrd, the start that `initrd-parse-etc`
  # ("Mountpoints Configured in the Real Root") asks for is refused whole,
  # stage 1 fails, and `boot.panic_on_fail` reboots: the first boot of an
  # installed machine never got past the disk's passphrase. And a seed there
  # is needed anyway: the installer leaves `/var/lib/nixos` on `@root` and
  # nothing in `/persist`, and stage 2 comes after the bind.
  #
  # From `config.fileSystems`, so a test VM, whose `virtualisation.fileSystems`
  # replace them, asks what it actually mounts.
  initrdDirectories = lib.filter (
    d: config.fileSystems ? ${d} && utils.fsNeededForBoot config.fileSystems.${d}
  ) cfg.persist.directories;

  # EVERY BIND ASKS FOR THE SEED, wherever the mount is started from: the
  # initrd (`initrdDirectories`), the boot's `local-fs.target`, a switch's
  # start phase, or a unit that restarts with `RequiresMountsFor=` on the
  # path (the browser extension's) — the last one is a transaction of its
  # own, and without this it mounts the empty copy the switch's `tmpfiles`
  # made. `x-systemd.requires=` is a `Requires=` and an `After=`.
  binds = lib.listToAttrs (
    map (d: {
      name = d;
      value = {
        device = "/persist${d}";
        fsType = "none";
        options = [
          "bind"
          "x-systemd.requires-mounts-for=/persist"
          "x-systemd.requires=ronoctua-persist-seed.service"
        ];
        depends = [ "/persist" ];
      };
    }) cfg.persist.directories
  );
in
{
  options.ronoctua.persist = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Routes state to /persist. On since day one.";
    };

    # Milestone 7's switch. Off until then, by section 7.2's decision.
    wipeRoot = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Erases the @root subvolume on every boot, leaving only what is in
        /persist. OFF until milestone 7 (section 7.2).

        Before switching it on: check that everything in `directories` and `files` covers
        what the system needs, or the next boot loses real state.
      '';
    };

    blockDevice = lib.mkOption {
      type = lib.types.str;
      default = "/dev/mapper/cryptroot";
      description = ''
        The btrfs blockDevice where `@root` lives, already unlocked.

        An option and not a constant because the test VM does not use LUKS: hard-coding
        `/dev/mapper/cryptroot` would make impermanence impossible to test
        without mounting the whole encryption, and what the test has to prove is
        something else.
      '';
    };

    keepDays = lib.mkOption {
      type = lib.types.ints.unsigned;
      default = 30;
      description = ''
        For how many days the old `@root`s stay in `/@old`.

        It is the recovery window for the case of a path missing from
        `directories`. Zero switches the window off and deletes on the spot — not
        recommended before the list is stable.
      '';
    };

    directories = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      description = "Directories that survive the wipe.";
      default = [
        # `/var/log` does NOT belong here: section 7.2's layout already gives it
        # a subvolume of its own (`@log`), and that is why it survives the wipe
        # of `@root`. With both, the bind and the subvolume mount fight over the
        # same point and evaluation fails with "conflicting definition values" —
        # which only showed up on a system using impermanence AND the layout
        # together, that is, on the first system actually installed.
        "/var/lib/nixos"
        "/var/lib/systemd/coredump"
        "/var/lib/bluetooth"
        # UPower's history: what feeds `TimeToEmpty` and the charge graph.
        # Losing it costs accuracy for the first hours, not correctness.
        "/var/lib/upower"
        # THE SECURITY CENTRE'S STATE: the person's firewall rules and
        # profiles, and the list of who may answer sudo with a finger. Lost,
        # the rules the person made are gone and every network is back on the
        # profile it was first given.
        "/var/lib/ronoctua-security"
        # THE PASSWORDLESS SUDO WINDOW'S LINE, the folder sudo includes: the
        # window survives a reboot and ends only at its end or by the
        # person's gesture (owner, 2026-10-11). Lost, an open window closes
        # with the wipe — the safe side — but the state beside it in
        # `/var/lib/ronoctua-security` would say it is open until the boot's
        # look (`ronoctua-sudo-window`) closes it.
        "/var/lib/ronoctua-sudo"
        # THE PARTS' RECORDS AND THE SESSION'S VIEW of them
        # (`nix/modules/parts.nix`). The parts themselves are profiles under
        # `/nix`, which is never wiped; lost, the records are made again from
        # the plan, but each part's count of tries and the view start over.
        "/var/lib/ronoctua-parts"
        # The enrolled fingerprints (fprintd's). Lost, every finger has to be
        # enrolled again, through the password.
        "/var/lib/fprint"
        # TOR'S GUARDS. Without them every start picks three new entry relays,
        # which is exactly what guards exist to prevent — and this is state
        # that is only worth anything by being old.
        "/var/lib/tor"
        # THE TAILNET'S NODE KEY. Losing it makes this machine a new one on
        # the tailnet, and the person is asked to log in again.
        "/var/lib/tailscale"
        "/etc/NetworkManager/system-connections"
        # Section 7.2 + lanzaboote: the Secure Boot keys are local and
        # never enter the repository (section 9).
        "/etc/secureboot"
        # NOTHING OF THE HOME. The footer's preferences, the notification
        # history and Syncthing's identity live in `@home`, which the wipe
        # never touches; persisting them bound a root-owned copy over the
        # person's own (`legacyHome` above, and the assertion below).
        # THE WI-FI PASSWORD sealed in the TPM needs no line here: it already
        # lives in `/persist/etc/ronoctua/wifi`, and the wipe only renames `@root`.
        # A bind mount here would create an empty `/etc/ronoctua` that nobody reads.
        # The directory is created by `nix/modules/network.nix`.
        # THE 2026-10-05 WAVE: one region per package, after its anchor.
        # ── anchor: L03 ──
        # THE MACHINE'S OWN FLAKE: `host.nix`, the hardware report, the lock
        # that pins the distro, the person's declarations and root's `local/`.
        # Wiped, the machine could not rebuild itself at all — it would boot
        # the generation it has and have nothing to describe the next one.
        # By the option and not a path written again (`machine-dir-has-one-name`);
        # `or` because a test machine may import this module alone.
        (config.ronoctua.machine.dir or (import ../machine.nix).machineDir)
        # The machine's life (`ronoctua-machine`): the last good generation,
        # the boot trial, a hold and the old layout a migration set aside.
        "/var/lib/ronoctua-machine"
        # ── anchor: L05 ──
        # ── anchor: L12 ──
        # ── anchor: L13 ──
        # ROOT'S NETRC FOR THE PERSON'S PRIVATE BINARY CACHE (the 2026-10-11
        # wave): written through the sudo door when the person set the token,
        # and by the installer. Lost, the machine reads the private cache no
        # more until the person sets the token again.
        (import ../binary-cache.nix).netrcDir
      ]
      # INCUS'S STATE — its database, its images, every instance and its
      # snapshots — where Incus is on (the full profile). On btrfs the
      # persisted copy is a subvolume of its own (`nix/modules/incus.nix`
      # makes it before this bind), so a snapshot of `/persist` never holds
      # it. Lost, every instance the person made is gone.
      ++ lib.optional (config.ronoctua.incus.enable or false) "/var/lib/incus"
      # THE PERSON'S PROGRAMS THAT LIVE OUTSIDE THE HOME, and only where the
      # machine has the support. Written HERE and not in `nix/modules/flatpak.nix`
      # or `android.nix`: this option has a DEFAULT, and a definition in another
      # module does not add to a default — it replaces it, and the wipe would
      # then take the machine id, the Secure Boot keys and every entry above.
      #
      # The system Flatpak installation: its remote and anything installed
      # `--system`. The person's own lives in `$HOME`, which is never wiped.
      ++ lib.optional (config.ronoctua.software.flatpak or false) "/var/lib/flatpak"
      # The Android image the person asked for — a gigabyte, downloaded once —
      # and everything installed inside it.
      ++ lib.optional (config.ronoctua.software.android or false) "/var/lib/waydroid"
      # THE BROWSER EXTENSION'S KEY, and the counter of its version. Lost, the
      # key is made again at the next boot, and a new key is a new extension
      # id: another extension to Chrome, the old one left force-installed by
      # a policy nothing writes any more, and the person's switch and history
      # with it (`nix/modules/browser-extension.nix`). `or false` because a
      # machine without the browser module has no option to read.
      ++ lib.optional (config.ronoctua.browser.enable or false) "/var/lib/ronoctua-browser";
      defaultText = lib.literalMD ''
        the machine's own state, plus `/var/lib/flatpak` and `/var/lib/waydroid`
        where `ronoctua.software` turns those supports on, and
        `/var/lib/ronoctua-browser` where the browser is
      '';
    };

    binds = lib.mkOption {
      type = lib.types.attrsOf lib.types.anything;
      readOnly = true;
      internal = true;
      description = ''
        The bind of every entry of `directories`, as `fileSystems` takes it.
        Read by a test VM, whose `virtualisation.fileSystems` replace
        `fileSystems`: it carries the module's own binds, seed included.
      '';
    };

    files = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      description = "Individual files that survive the wipe.";
      default = [
        "/etc/machine-id"
        "/etc/ssh/ssh_host_ed25519_key"
        "/etc/ssh/ssh_host_ed25519_key.pub"
        # ── anchor: L02 ──
        # NO RSA HOST KEY (2026-10-05): an sshd of ours has ed25519 alone
        # (`assets/ssh.nix`), and the RSA pair this list used to keep is
        # removed from `/persist` by `nix/modules/ssh.nix` (`no-rsa-host-key`).
        # ── anchor: L03 ──
      ];
    };
  };

  config = lib.mkIf cfg.persist.enable {
    # Routing by bind mount. Simple and readable; it does not depend on the
    # third-party impermanence module, which keeps the input graph small.
    # `-` FOR MODE, USER AND GROUP: a missing directory is made as root,
    # 0755 — tmpfiles' own default — and an existing one is left as it is.
    # With `0755 root root` every boot took back the owner a service had set
    # (`StateDirectory=` for Tor and Tailscale), and it is what made the home's
    # directories root's (`legacyHome`).
    systemd.tmpfiles.rules = map (d: "d /persist${d} - - - -") cfg.persist.directories ++ [
      "d /persist/etc/shadow 0700 root root -"
    ];

    # THE HOME'S DIRECTORIES BACK FROM /persist, once (`homeReturn`). Before
    # any login, as the cache's subvolume unit is (`filesystem.nix`): nothing
    # of the person's has a file open in there yet. A switch starts it too —
    # it is new, and the switch restarts the targets that want it — and the
    # program returns at once there, since only the boot comes before the
    # logins. Only where there IS a person (`base.nix` declares
    # `ronoctua.user`): a test that imports this module alone has no home to
    # return anything to.
    systemd.services = {
      ronoctua-persist-seed = {
        description = "Gives every bind from /persist its source, seeded from the root";
        wantedBy = [ "local-fs.target" ];
        before = [
          "local-fs.target"
        ]
        ++ map (d: "${utils.escapeSystemdPath d}.mount") cfg.persist.directories;
        unitConfig = {
          # BEFORE THE ORDINARY ORDER EXISTS: the binds are part of
          # `local-fs.target`, and the default dependencies would put this
          # after it.
          DefaultDependencies = false;
          RequiresMountsFor = [ "/persist" ];
        };
        # NEVER RESTARTED BY A SWITCH: every bind `Requires=` it, and a restart
        # of a required unit restarts its dependants — every bind, under a
        # live session. Being inactive between runs, a switch has nothing to
        # restart anyway; this says it.
        restartIfChanged = false;
        # NOT `RemainAfterExit`: a seed that stayed active would be a no-op
        # for the next mount that asks for it (a switch's), and it is the
        # mount that has to find its source. It runs before every bind that
        # starts, and does nothing where the copy already holds something.
        serviceConfig = {
          Type = "oneshot";
          ExecStart = "${seed}/bin/ronoctua-persist-seed / ${lib.escapeShellArgs cfg.persist.directories}";
        };
      };
    }
    // lib.optionalAttrs (options.ronoctua ? user) {
      ronoctua-persist-home-return = {
        description = "Returns what /persist kept of the home to the home";
        wantedBy = [ "multi-user.target" ];
        before = [ "systemd-user-sessions.service" ];
        unitConfig = {
          ConditionPathExists = "/persist/home";
          RequiresMountsFor = [
            "/persist"
            "/home/${cfg.user}"
          ];
        };
        # A CHANGED program is not re-run by a switch; a NEW unit is started by
        # one, and the program's own gate lets it do nothing then.
        restartIfChanged = false;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          ExecStart = "${homeReturn}/bin/ronoctua-persist-home-return";
        };
      };
    };

    # THE SEED OF THE BINDS THE INITRD MOUNTS, under the name their fstab line
    # requires (`initrdDirectories` says why), run against `/sysroot` once
    # `/sysroot/persist` is up and before each of those binds. The program goes
    # in by `storePaths`, with the two commands it runs beside the initrd's own
    # coreutils: the initrd holds no store path a unit merely names — the unit
    # failed at EXEC, and `mountpoint` was "command not found".
    boot.initrd.systemd.storePaths = lib.mkIf (initrdDirectories != [ ]) [
      seed
      "${lib.getBin pkgs.findutils}/bin/find"
      "${lib.getBin pkgs.util-linux}/bin/mountpoint"
    ];
    boot.initrd.systemd.services.ronoctua-persist-seed = lib.mkIf (initrdDirectories != [ ]) {
      description = "Gives every bind the initrd mounts from /persist its source";
      before = map (d: "${utils.escapeSystemdPath "/sysroot${d}"}.mount") initrdDirectories;
      unitConfig = {
        DefaultDependencies = false;
        RequiresMountsFor = [ "/sysroot/persist" ];
      };
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${seed}/bin/ronoctua-persist-seed /sysroot ${lib.escapeShellArgs initrdDirectories}";
      };
    };

    # A single fileSystems definition: /persist (which has to be up before
    # anything else) plus the bind mounts.
    fileSystems = {
      "/persist".neededForBoot = true;
    }
    // binds;
    ronoctua.persist.binds = binds;

    # THE WIPE.
    #
    # It runs in the initrd, before `/` is mounted for real — afterwards it
    # could not: you do not delete the subvolume that is mounted.
    #
    # # Why RENAME and not delete
    #
    # The temptation is `btrfs subvolume delete @root`. We do not do that: if a
    # path is missing from `directories`, the state disappears with no recourse,
    # and the person only finds out at the next boot. Renaming to
    # `@root-<date>` leaves a recovery window — the forgotten file is still
    # there, mountable by hand. The old ones are collected after `keepDays`.
    #
    # # `boot.initrd.systemd`, not `postDeviceCommands`
    #
    # The distro uses the systemd initrd (`boot.initrd.systemd.enable`, in
    # `nix/modules/host.nix`). `boot.initrd.postDeviceCommands` belongs to the
    # shell initrd and is IGNORED there — silently. The wipe simply would not happen, and the
    # only symptom would be state surviving when it should not.
    boot.initrd.systemd.services.ronoctua-wipe = lib.mkIf cfg.persist.wipeRoot {
      description = "Renews @root (impermanence)";
      wantedBy = [ "initrd.target" ];
      # After the LUKS unlock and BEFORE mounting the sysroot: in between is the
      # only point where the subvolume exists and is not in use.
      after = [ "cryptsetup.target" ];
      before = [ "sysroot.mount" ];
      unitConfig.DefaultDependencies = "no";
      serviceConfig.Type = "oneshot";
      script = ''
        set -eu
        blockDevice=${lib.escapeShellArg cfg.persist.blockDevice}
        mkdir -p /btrfs_tmp
        mount -t btrfs -o subvol=/ "$blockDevice" /btrfs_tmp

        if [ -e /btrfs_tmp/@root ]; then
          # The date of the SUBVOLUME ITSELF, not of right now: the clock in the
          # initrd may not be set, and a name with the wrong date gets in the way
          # of exactly the person looking for the lost file.
          stamp=$(stat -c %y /btrfs_tmp/@root | cut -d' ' -f1)
          mkdir -p /btrfs_tmp/@old

          # A UNIQUE NAME, checked.
          #
          # Date plus epoch is not enough: two wipes in the same second produce
          # the same name, and `mv source target` with the target existing as a
          # directory moves the source INSIDE it — with no error. The result
          # is one `@root` nested inside another, and the count of old ones
          # stops adding up with nothing to flag it.
          target_path="/btrfs_tmp/@old/@root-$stamp-$(date +%s)"
          n=0
          while [ -e "$target_path" ]; do
            n=$((n + 1))
            target_path="/btrfs_tmp/@old/@root-$stamp-$(date +%s)-$n"
          done

          echo "ronoctua: keeping @root as $(basename "$target_path")"
          mv /btrfs_tmp/@root "$target_path"
        fi

        # Collects the old ones. `-mtime` counts whole days; one wipe per boot
        # on a machine left on all day would produce several with the same date,
        # and that is why the name carries the epoch alongside.
        if [ -d /btrfs_tmp/@old ]; then
          for old in $(find /btrfs_tmp/@old -maxdepth 1 -mindepth 1 \
                            -type d -mtime +${toString cfg.persist.keepDays}); do
            echo "ronoctua: collecting $old"
            # Nested subvolume first: `delete` on the parent fails if there is a
            # child, and the message talks about "directory not empty".
            btrfs subvolume list -o "$old" \
              | cut -f9 -d' ' \
              | while read -r child; do btrfs subvolume delete "/btrfs_tmp/$child"; done
            btrfs subvolume delete "$old"
          done
        fi

        btrfs subvolume create /btrfs_tmp/@root
        umount /btrfs_tmp
      '';
    };

    assertions = [
      {
        assertion = !cfg.persist.wipeRoot || cfg.persist.enable;
        message = "ronoctua.persist.wipeRoot requires ronoctua.persist.enable.";
      }
      {
        assertion = !lib.any (d: d == "/home" || lib.hasPrefix "/home/" d) cfg.persist.directories;
        message = ''
          ronoctua.persist: no entry under /home — the home is `@home`, which the
          wipe never touches, and a bind over it hides the home's own copy while
          tmpfiles makes the persisted one root's.
        '';
      }
    ];
  };
}
