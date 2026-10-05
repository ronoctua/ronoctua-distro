# Android — Waydroid, and the three root units the person may start.
#
# # Nothing is downloaded until the person asks
#
# The Android system image is 1–1,3 GB, and it is NOT in the store: pinned
# there it would cost every full machine about 2,4 GB unpacked whether Android
# is ever used or not, and a fixed URL never moves with `flake update`. So
# `waydroid init` fetches it on the machine, when the person presses Prepare,
# and it checks every zip against the sha256 its own OTA channel publishes.
# That is the ONE write outside the home that invariant 6's exception names
# (`AGENTS.md`, 2026-09-26): `/var/lib/waydroid`, by a unit that runs as root
# and that nothing but this rule may start.
#
# # Why the container SKIPS before the image exists
#
# Waydroid's own `waydroid-container.service` starts at boot and FAILS with no
# image — and a failed unit makes the system report itself `degraded`, which
# is exactly when nix-flatpak's Home Manager activation skips, in silence
# (`nix/modules/flatpak.nix`). A condition turns that failure into a skip, and
# the unit starts on its own the first boot after Prepare.
#
# # The door is one millimetre wide
#
# One rule: the action that manages units, the verb `start` and nothing else,
# four units named exactly, one person. `YES` and not `AUTH_SELF_KEEP`,
# because there is no polkit agent in this session (the update's `module.nix`);
# what buys that width back is that a typo in any of the names is a refusal
# rather than a wider door.
#
# `mkDefault full` in `nix/modules/profile.nix`: Waydroid's closure is 0,72 GB
# on its own, measured, and it asks the kernel for binder.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  user = config.ronoctua.user;
  waydroid = config.virtualisation.waydroid.package;

  # WHERE WAYDROID KEEPS ITS SYSTEM, from its own defaults (`tools/config`):
  # the configuration `init` writes last, and the images it downloads.
  state = "/var/lib/waydroid";
  configured = "${state}/waydroid.cfg";

  # WHERE THE DEVICE ID IS HANDED TO THE PERSON. `/run`, because it is read
  # out of a running Android and means nothing after a reboot that nobody
  # re-asked; `0644`, because it is not a secret — it is what the person types
  # into Google's registration page.
  idDirectory = "ronoctua-android";
  idFile = "/run/${idDirectory}/android-id";

  # THE TWO IMAGES `init` offers, as the instance of one template: `GAPPS`
  # (with Google Play, the owner's default) and `VANILLA` (without).
  images = [
    "GAPPS"
    "VANILLA"
  ];
  initUnit = image: "ronoctua-android-init@${image}.service";
  resetUnit = "ronoctua-android-reset.service";
  idUnit = "ronoctua-android-id.service";
  startable = map initUnit images ++ [
    resetUnit
    idUnit
  ];
in
{
  config = lib.mkIf config.ronoctua.software.android {
    virtualisation.waydroid.enable = true;

    # SKIPPED, NOT FAILED, until Prepare has run — see the header.
    systemd.services.waydroid-container.unitConfig.ConditionPathExists = configured;

    # PREPARE: download and set up the image the person chose.
    #
    # A TEMPLATE, so the image is the instance name and not an argument: the
    # polkit rule names each instance whole, and there is no way to smuggle a
    # third value through a unit name that the rule does not list.
    #
    # `ConditionPathExists=!` makes a second press a no-op instead of a second
    # download over the first; starting over is the reset unit's job, and it
    # asks first on the screen. `--no-block` for the container: its start job
    # must not wait inside this one's.
    systemd.services."ronoctua-android-init@" = {
      description = "Prepares Android (Waydroid) with the %i image";
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];
      unitConfig.ConditionPathExists = "!${configured}";
      serviceConfig = {
        Type = "oneshot";
        # A gigabyte over whatever the network is: the default start timeout
        # would kill the download at ninety seconds.
        TimeoutStartSec = "infinity";
        ExecStart = "${waydroid}/bin/waydroid init -s %i";
        ExecStartPost = "${pkgs.systemd}/bin/systemctl start --no-block waydroid-container.service";
      };
      # A rebuild must never restart a download in the middle.
      restartIfChanged = false;
    };

    # ERASE: the images and everything installed inside Android.
    #
    # The CONTENTS go, and the directory stays: it is a mount point when the
    # root is wiped (`nix/modules/persist.nix`), and `--one-file-system` keeps
    # the removal from following anything still mounted beneath it. The device
    # id goes too — it belonged to the Android that was just erased.
    systemd.services.ronoctua-android-reset = {
      description = "Erases Android (Waydroid): its images and what was installed in it";
      serviceConfig.Type = "oneshot";
      script = ''
        systemctl stop waydroid-container.service
        if [ -d ${state} ]; then
          find ${state} -mindepth 1 -maxdepth 1 -exec rm -rf --one-file-system {} +
        fi
        rm -f ${idFile}
      '';
      restartIfChanged = false;
    };

    # THE DEVICE ID GOOGLE ASKS FOR, read out of a running Android.
    #
    # Play Store refuses to work on a device Google has not certified, and
    # Waydroid is not certified. The way through is Google's own page
    # (`https://www.google.com/android/uncertified`), which asks for the
    # Google Services Framework id — and that id lives in a database INSIDE
    # the container, which only root can reach. The query is the one
    # Waydroid's documentation gives (docs.waydro.id, "Google Play
    # Certification"); `waydroid shell` already sets the Android environment
    # it spells out by hand.
    #
    # EVERY WAY OF NOT FINDING IT IS AN ANSWER, NEVER A FAILURE: no session,
    # Play Services not yet registered, a VANILLA image with no Play Services
    # at all, an image with no `sqlite3`. Each leaves no file, the screen says
    # the id is not there yet, and the unit still exits zero — a failed unit is
    # a `degraded` system, and that has a cost of its own (see the header).
    systemd.services.ronoctua-android-id = {
      description = "Reads Android's device id, for Google's registration page";
      path = [ waydroid ];
      serviceConfig = {
        Type = "oneshot";
        TimeoutStartSec = "5min";
        RuntimeDirectory = idDirectory;
        RuntimeDirectoryMode = "0755";
        # The file must outlive this unit, which is over the moment it wrote it.
        RuntimeDirectoryPreserve = "yes";
      };
      script = ''
        deadline=$((SECONDS + 240))
        while [ "$SECONDS" -lt "$deadline" ]; do
          if waydroid status 2>/dev/null | grep -q '^Session:[[:space:]]*RUNNING'; then
            said=$(timeout 30 waydroid shell -- sqlite3 \
              /data/data/com.google.android.gsf/databases/gservices.db \
              "select value from main where name = 'android_id';" 2>/dev/null || true)
            id=$(printf '%s' "$said" | tr -d '[:space:]')
            case "$id" in
              "" | *[!0-9]*) ;;
              *)
                printf '%s\n' "$id" > ${idFile}.new
                chmod 0644 ${idFile}.new
                mv -f ${idFile}.new ${idFile}
                echo "the device id is in ${idFile}"
                exit 0
                ;;
            esac
          fi
          sleep 5
        done
        echo "no device id: no session is running, Play Services has not registered yet, or the image has no sqlite3"
      '';
      restartIfChanged = false;
    };

    # WHO MAY START THOSE FOUR, AND ONLY THOSE.
    #
    # The alternatives are BRACKETED, and that is not style: `&&` binds tighter
    # than `||`, so without the brackets every unit after the first would be
    # granted for ANY action and ANY subject — `nix/modules/rescue.nix` shipped
    # exactly that for one commit (`polkit-rules-are-the-ones-we-know`).
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (action.id == "org.freedesktop.systemd1.manage-units" &&
            action.lookup("verb") == "start" &&
            (${
              lib.concatMapStringsSep " ||\n       " (u: ''action.lookup("unit") == "${u}"'') startable
            }) &&
            subject.user == "${user}") {
          return polkit.Result.YES;
        }
      });
    '';

    # `/var/lib/waydroid` survives the root wipe through
    # `nix/modules/persist.nix`, which holds the whole list. What the person
    # keeps inside Android lives in `$HOME/.local/share/waydroid`, which is
    # never wiped.
  };
}
