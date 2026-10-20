# The camera studio — `LEADER m c` (the app) and `LEADER s c` (its settings).
#
# ONE PROCESS OWNS THE VIDEO: the `ronoctua-studio` unit. A V4L2 capture
# streams to one reader, a recording and the virtual camera must outlive the
# window that started them, and a scene mixes sources no window has — so the
# app and the settings view are clients of the unit's socket, and nothing else
# opens a camera (checked by `the-camera-has-one-door`).
#
# At rest it costs nothing: with no preview open, no recording, no stream and
# no virtual camera, every source is closed — the camera's light is off — and
# the loop sleeps on a condition variable.
#
# EXTRACTED from the workspace (`rust.studio`), for the transfers' reason: a
# unit's closure is what its bytes cite, and this one draws nothing.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.studio;
in
{
  # THE SESSION'S CAMERAS IN PIPEWIRE, and why they are off.
  #
  # WirePlumber's `main` profile requires `hardware.video-capture`, which
  # wants its V4L2 and libcamera monitors: when the person's PipeWire starts —
  # at login, on the session's first audio client — WirePlumber opens every
  # `/dev/videoN` to ask what it is, and again whenever a camera is plugged.
  # On a UVC webcam that open is the light blinking with nobody asking, the
  # defect the footer and the studio stopped committing (`camera-vm` measures
  # both). Nothing the distro runs reads a camera through PipeWire: the
  # studio, the browser and the virtual camera's readers open the V4L2 node
  # themselves. What turning it on gives is the cameras as PipeWire nodes —
  # the camera portal, and a program that reads a camera only through it (a
  # Flatpak without device access, OBS's PipeWire source).
  #
  # HERE AND NOT IN `base.nix`, beside `services.pipewire`: it is a camera's
  # system side, like the loopback below, and it is outside the studio's
  # switch because the light is the person's with or without the studio.
  options.ronoctua.camera.pipewireCameras = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Let WirePlumber publish every camera as a PipeWire node (its V4L2 and libcamera monitors).
      On, the person's WirePlumber opens every camera when the session starts and whenever one is
      plugged in — on a webcam, the light comes on with nobody asking. What it gives: the cameras
      through PipeWire, which the camera portal and programs that only read a camera through it
      (a Flatpak without device access, OBS's PipeWire source) need. Off, those see no camera;
      every program that opens /dev/video* itself (the studio, the browser) is unchanged.
    '';
  };

  options.ronoctua.studio = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "The camera studio: the app, its settings, photos, recordings, scenes and streams.";
    };

    # THE VIRTUAL CAMERA, and why it is a kernel module.
    #
    # The effects reach OTHER programs — a browser call, a meeting app — only
    # through a camera device they can open, and `v4l2loopback` is the one
    # every program on Linux already knows how to read. It is out of tree, so
    # it is built for THIS kernel by `boot.kernelPackages`, and loaded at boot:
    # a device that appears only when the studio asks would be missing from a
    # browser that enumerated cameras first.
    #
    # `exclusive_caps=1` is what browsers need: the node advertises CAPTURE
    # only while something writes to it, and Chromium ignores a node that
    # claims to be both. The card label is how `ronoctua-camera` tells OUR
    # loopback (never a source) from somebody else's (a camera like any other).
    virtualCamera = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Load v4l2loopback so the studio can offer its program as a camera to other programs.";
    };

    # A HIGH NUMBER, so the node never takes `/dev/video0` from a real camera
    # plugged in later — programs that open "the first camera" open that.
    virtualNode = lib.mkOption {
      type = lib.types.ints.between 0 63;
      default = 42;
      description = "The /dev/videoN the virtual camera takes.";
    };
  };

  # `hardware.video-capture` is the virtual feature that wants both monitors,
  # and nothing requires it but the profile: disabling it is the switch the
  # pinned WirePlumber (0.5) documents, and it takes the libcamera monitor
  # with the V4L2 one.
  config = lib.mkMerge [
    {
      services.pipewire.wireplumber.extraConfig."90-ronoctua-cameras" =
        lib.mkIf (!config.ronoctua.camera.pipewireCameras)
          {
            "wireplumber.profiles".main."hardware.video-capture" = "disabled";
          };
    }

    (lib.mkIf cfg.enable {
      boot.extraModulePackages = lib.mkIf cfg.virtualCamera [ config.boot.kernelPackages.v4l2loopback ];
      boot.kernelModules = lib.mkIf cfg.virtualCamera [ "v4l2loopback" ];
      boot.extraModprobeConfig = lib.mkIf cfg.virtualCamera ''
        options v4l2loopback video_nr=${toString cfg.virtualNode} card_label="ronoctua" exclusive_caps=1
      '';

      systemd.user.services.ronoctua-studio = {
        description = "ronoctua — the camera studio";
        partOf = [ "graphical-session.target" ];
        after = [ "graphical-session.target" ];
        wantedBy = [ "graphical-session.target" ];
        environment = {
          # THE PATH AND NOT THE NAME — a unit's `PATH` is minimal. The same
          # ffmpeg `nix/modules/media.nix` installs on every profile.
          RONOCTUA_FFMPEG = "${pkgs.ffmpeg-full}/bin/ffmpeg";
          # The segmentation model; absent, the three background effects are
          # drawn refusing and everything else works.
          RONOCTUA_SEGMENT_MODEL = "${ronoctua.segmentModel}";
          # The hand models; absent, gestures are refused with a sentence.
          RONOCTUA_HAND_MODELS = "${ronoctua.handModels}";
        };
        serviceConfig = {
          ExecStart = "${ronoctua.studio}/bin/ronoctua-studio";
          Restart = "on-failure";
          RestartSec = 1;
          # `session.slice`, beside the footer: a recording holds a camera and
          # an encoder, and oomd choosing it among the applications would end a
          # recording at random.
          Slice = "session.slice";
          # Long enough for ffmpeg to finish a file on logout: a Matroska that
          # was killed keeps its frames, and one that finished keeps its index.
          TimeoutStopSec = 20;
        };
      };
    })
  ];
}
