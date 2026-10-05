# The video catalogue's scanner: the user unit the brain starts every few
# hours (the `video-metrics` module), and the command line a person or a test
# runs by hand.
#
# WHY THIS EXISTS
# ---------------
# Video metrics keeps a catalogue of the person's YouTube channels, scored
# (`crates/ronoctua-video-metrics`, `LEADER v`). New videos arrive by a SCAN:
# each channel's uploads feed over plain HTTP, a flat tab through yt-dlp when
# a feed hides a backlog, details and thumbnails for a few videos at a time.
# A scan is minutes of slow asking, so it is a unit of its own — never the
# door's loop, never the brain's thread — and the brain decides WHEN.
#
# WHAT IT RUNS AS
# ---------------
# The person, at idle priority (`Nice`, idle I/O): it asks slowly on purpose
# and yields to everything they do. Its report goes where the brain reads
# (`%t/ronoctua-brain/<module id>.json`), which is how the brain watches it
# whoever started it — the brain on its cadence, or the door's "Scan now"
# through systemd when no brain answers.
#
# WHAT IT NEEDS
# -------------
# yt-dlp by store path (a unit's `PATH` is minimal), inside the `mkIf` that
# decides whether the package exists at all — the reason `RONOCTUA_YT_DLP` is
# never baked into a binary (`ronoctua_ytdlp::program`). And the PO token
# minter, the same three variables the queue and the player take, when
# `ronoctua.transfers.poToken` asks for it: details are YouTube's video pages,
# which want the device proof the default clients are asked for. The three
# are the minter's own (`potProvider.environment`, `nix/pot-provider.nix`):
# where its plugin is is a fact of its layout, and a unit that spelled it
# again once spelled it wrong.
#
# A SCAN KILLED BEFORE IT ENDED — the brain giving up, `systemctl stop`, the
# memory killer — never writes the end of its report, and the brain would
# wait out its patience for it with the modeline saying "scanning". The
# unit's `ExecStopPost` writes that end (`scan-stopped`), with systemd's own
# word for what happened; a report the scan ended itself is left as it is.
{ videoMetrics }:
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.videoMetrics;
  media = config.ronoctua.media.enable;
  onDemand = (config.ronoctua.transfers.poToken or "none") == "on-demand";

  # THE MODULE'S ID IS ITS FOLDER'S NAME, and so is the report's: the brain
  # watches `<id>.json` (`runner::report_in`), and the name is written here
  # once, from the folder.
  moduleId = baseNameOf ../../crates/ronoctua-brain/modules/video-metrics;
  manifest = import ../../crates/ronoctua-brain/modules/video-metrics/manifest.nix;
  unit = lib.removeSuffix ".service" manifest.units.run;
  # THE REPORT THE BRAIN WATCHES — written by the scan, and ended by the
  # unit when the scan could not end it.
  report = "%t/ronoctua-brain/${moduleId}.json";

  # WHAT A SCAN READS FROM ITS ENVIRONMENT — the unit's, and the command
  # line's, which is the same program by hand.
  scanEnvironment =
    lib.optionalAttrs media {
      RONOCTUA_YT_DLP = "${pkgs.yt-dlp}/bin/yt-dlp";
    }
    # ALL OF THEM OR NONE, AND THE MINTER'S OWN: the minter is TypeScript and
    # needs the runtime that reads it, and the plugin that asks it is loaded
    # from the provider's root — named once, beside the layout.
    // lib.optionalAttrs (media && onDemand) ronoctua.potProvider.environment;

  # THE COMMAND LINE, with the scan's environment set unless the caller set
  # one: `add`, `scan`, `hide` and `delete` for a terminal and for a test.
  cli = pkgs.runCommand "ronoctua-video-metrics-cli" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
    mkdir -p $out/bin
    makeWrapper ${videoMetrics}/bin/ronoctua-video-metrics $out/bin/ronoctua-video-metrics ${
      lib.concatStringsSep " " (
        lib.mapAttrsToList (k: v: "--set-default ${k} ${lib.escapeShellArg v}") scanEnvironment
      )
    }
  '';
in
{
  options.ronoctua.videoMetrics.enable =
    lib.mkEnableOption "the video catalogue's scanner: the unit the brain starts, and its command line"
    // {
      # WHERE THE MEDIA IS: a machine without yt-dlp has nothing to ask a
      # backlog or a video's details with.
      default = media;
      defaultText = lib.literalExpression "config.ronoctua.media.enable";
    };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cli ];

    # ADDING A CHANNEL FROM THE DOOR asks yt-dlp who it is: the footer and
    # the windows it opens need the same program by store path.
    systemd.user.services.ronoctua-footer.environment = lib.optionalAttrs media {
      RONOCTUA_YT_DLP = "${pkgs.yt-dlp}/bin/yt-dlp";
    };

    systemd.user.services.${unit} = {
      description = "ronoctua — scan the video channels for new videos";
      environment = scanEnvironment;
      serviceConfig = {
        Type = "oneshot";
        ExecStart = lib.concatStringsSep " " [
          "${videoMetrics}/bin/ronoctua-video-metrics"
          "scan"
          # THE REPORT THE BRAIN WATCHES — the module's check reads the unit
          # this line makes.
          "--report ${report}"
        ];
        # AND ITS END when the scan was killed before writing it: the brain
        # waits for an end, and a mark stays up until one comes. `-`: a
        # report that cannot be ended is no reason to call the unit failed.
        ExecStopPost = "-${videoMetrics}/bin/ronoctua-video-metrics scan-stopped --report ${report}";
        # MADE BEFORE THE RUN: the report's folder is the brain's, and a
        # scan started before the brain ever ran still has one.
        RuntimeDirectory = "ronoctua-brain";
        RuntimeDirectoryPreserve = "yes";
        # IDLE: it asks slowly on purpose, and yields to everything the
        # person does.
        Nice = 10;
        IOSchedulingClass = "idle";
        Slice = "background.slice";
        # A SCAN THAT STOPPED EARLY (a bot check, a 429, no network: 3) OR
        # FOUND EVERY CHANNEL FAILING (4) is said in its report, where the
        # brain reads it; the unit ended as it should.
        SuccessExitStatus = "3 4";

        NoNewPrivileges = true;
        RestrictSUIDSGID = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        RestrictNamespaces = true;
        SystemCallFilter = [ "@system-service" ];
        RestrictAddressFamilies = [
          "AF_UNIX"
          "AF_INET"
          "AF_INET6"
        ];
      };
    };
  };
}
