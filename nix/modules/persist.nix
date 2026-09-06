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
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua;
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
        # The user's app id -> glyph table (section 4.2): an override read at
        # runtime, with hot reload. Losing this would mean losing customisation.
        "/home/${cfg.user}/.config/ronoctua"
        # THE NOTIFICATION HISTORY, which is fifteen days of it (section 4.2).
        #
        # `$XDG_STATE_HOME`, which is the directory for exactly this: machine
        # local, machine written, and it outlives a reboot. Without the line the
        # wipe takes it, and the symptom is a manager that is empty every
        # morning with nothing saying why.
        "/home/${cfg.user}/.local/state/ronoctua"
        # SYNCTHING'S STATE — index, device key and the list of folders the
        # person added.
        #
        # Losing this does not lose files, but it loses the IDENTITY: the
        # device is reborn with a new id, the others stop recognising it,
        # and every folder has to be re-introduced on both ends. On top of
        # that the index of hundreds of gigabytes would be rebuilt from scratch.
        "/home/${cfg.user}/.local/state/syncthing"
        # THE WI-FI PASSWORD sealed in the TPM needs no line here: it already
        # lives in `/persist/etc/ronoctua/wifi`, and the wipe only renames `@root`.
        # A bind mount here would create an empty `/etc/ronoctua` that nobody reads.
        # The directory is created by `nix/modules/network.nix`.
      ];
    };

    files = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      description = "Individual files that survive the wipe.";
      default = [
        "/etc/machine-id"
        "/etc/ssh/ssh_host_ed25519_key"
        "/etc/ssh/ssh_host_ed25519_key.pub"
        "/etc/ssh/ssh_host_rsa_key"
        "/etc/ssh/ssh_host_rsa_key.pub"
      ];
    };
  };

  config = lib.mkIf cfg.persist.enable {
    # Routing by bind mount. Simple and readable; it does not depend on the
    # third-party impermanence module, which keeps the input graph small.
    systemd.tmpfiles.rules = map (d: "d /persist${d} 0755 root root -") cfg.persist.directories ++ [
      "d /persist/etc/shadow 0700 root root -"
    ];

    # A single fileSystems definition: /persist (which has to be up before
    # anything else) plus the bind mounts.
    fileSystems = {
      "/persist".neededForBoot = true;
    }
    // lib.listToAttrs (
      map (d: {
        name = d;
        value = {
          device = "/persist${d}";
          fsType = "none";
          options = [
            "bind"
            "x-systemd.requires-mounts-for=/persist"
          ];
          depends = [ "/persist" ];
        };
      }) cfg.persist.directories
    );

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
    # The distro uses the systemd initrd (required by lanzaboote and by the TPM2
    # unlock). `boot.initrd.postDeviceCommands` belongs to the shell initrd and
    # is IGNORED there — silently. The wipe simply would not happen, and the
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
        message = "ronoctua.persist.wipeRoot exige ronoctua.persist.enable.";
      }
    ];
  };
}
