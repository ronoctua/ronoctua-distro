# The player — `LEADER p`, the column on the right and the modeline's mark.
#
# ONE PROCESS PLAYS: the `ronoctua-player` unit. One decode feeds every view
# (the column, the footer, a window) through shared memory, and one audio
# output plays, so the same video in three places is in step by construction.
# The shell's doors are clients of its socket; none of them links libmpv.
#
# At rest it costs nothing: with nothing loaded no mpv exists, and the loop
# sleeps on a condition variable.
#
# EXTRACTED from the workspace (`rust.player`), for the studio's reason: its
# closure is libmpv and ffmpeg-full, never the renderer.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.player;

  # THE SOUNDS ONLY THE DECODERS OPEN, which freedesktop has no type for, the
  # player's own two files (`own`: `.rset` and `.rqueue`), and the subtitle
  # files it lays over a film (`subtitles`).
  ours = import ../../assets/player-types.nix;

  # EVERY TYPE THE PLAYER OPENS, by name — a desktop entry cannot say
  # `audio/*`, so the list is READ out of freedesktop's database at build
  # time: every `audio/` and `video/` type it has, the handful of media types
  # that live under `application/`, and ours. A hand list was this module's
  # first form, and it held fifty-one of the ninety-five — the ones somebody
  # remembered.
  #
  # Built, not evaluated: listing a directory of the store while evaluating is
  # import-from-derivation, which every evaluation of the flake would pay.
  #
  # One type is left out: `audio/x-iriver-pla`, a binary playlist with no
  # published specification, which no reader here opens.
  applicationTypes = [
    "application/ogg"
    "application/vnd.apple.mpegurl"
    "application/xspf+xml"
    "application/vnd.rn-realmedia"
    "application/vnd.ms-asf"
    "application/mxf"
    "application/x-matroska"
  ];
  refused = [ "audio/x-iriver-pla" ];

  # The type database's half of `ours`: the same shape `update-mime-database`
  # reads from every package, so a foreign program asking `xdg-mime` about a
  # `.sid` variant hears the same answer the file manager does.
  package = pkgs.writeText "ronoctua-player.xml" (
    lib.concatLines (
      [
        ''<?xml version="1.0" encoding="UTF-8"?>''
        ''<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">''
      ]
      ++ lib.concatMap (
        f:
        [
          "  <mime-type type=\"${f.mime}\">"
          "    <sub-class-of type=\"${f.under}\"/>"
          # A SUBTITLE FILE IS WORDS to a program that knows nothing of it.
          "    <generic-icon name=\"${
                if builtins.elem f ours.subtitles.invented then "text-x-generic" else "audio-x-generic"
              }\"/>"
        ]
        ++ map (g: "    <glob pattern=\"${g}\"/>") f.globs
        ++ [ "  </mime-type>" ]
      ) (ours.families ++ ours.own ++ ours.subtitles.invented)
      ++ [ "</mime-info>" ]
    )
  );

  # `ronoctua-shell play`: plays what it is given and raises the column. The
  # entry, the database package and the defaults travel as ONE package, so
  # none can be installed without the others.
  entry =
    pkgs.runCommand "ronoctua-player-desktop"
      {
        nativeBuildInputs = [ pkgs.python3 ];
        extra = lib.concatStringsSep " " (applicationTypes ++ ours.types);
        refused = lib.concatStringsSep " " refused;
      }
      ''
        python3 - ${pkgs.shared-mime-info}/share/mime <<'END'
        import os, pathlib, sys
        base = pathlib.Path(sys.argv[1])
        found = sorted(
            f"{media}/{p.name[:-4]}"
            for media in ("audio", "video")
            for p in (base / media).iterdir()
            if p.name.endswith(".xml")
        )
        refused = set(os.environ["refused"].split())
        types = [t for t in found if t not in refused] + os.environ["extra"].split()
        # A FLOOR, because a reader that stopped finding the files gives back
        # an entry that opens nothing, and nothing would say so.
        if len(found) < 80:
            sys.exit(f"only {len(found)} audio and video types were found")
        out = pathlib.Path(os.environ["out"])
        apps = out / "share/applications"
        apps.mkdir(parents=True)
        (apps / "br.ronoctua.player.desktop").write_text(
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=Player\n"
            "Comment=Plays audio and video, in the column on the right\n"
            "Exec=${ronoctua.shell}/bin/ronoctua-shell play %U\n"
            "Terminal=false\n"
            "NoDisplay=true\n"
            "Categories=AudioVideo;Audio;Video;Player;\n"
            f"MimeType={';'.join(types)};\n"
        )
        # THE DISTRIBUTION'S DEFAULTS, where the specification keeps them:
        # `$XDG_DATA_DIRS/applications/mimeapps.list`. Below the person's own
        # file and below `/etc/xdg`, so a choice anybody made wins over it.
        # THE SAME LIST, one type a line, for MPRIS `SupportedMimeTypes`:
        # the specification wants each type named, and a wildcard is not one.
        share = out / "share/ronoctua"
        share.mkdir(parents=True)
        (share / "player-types").write_text("".join(f"{t}\n" for t in types))
        (apps / "mimeapps.list").write_text(
            "[Default Applications]\n"
            + "".join(f"{t}=br.ronoctua.player.desktop\n" for t in types)
        )
        END
        install -Dm644 ${package} $out/share/mime/packages/ronoctua-player.xml
      '';
in
{
  options.ronoctua.player = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "The player: audio and video, its column, and playing in step across machines.";
    };

    # THE PORT OTHER MACHINES REACH THIS ONE ON — and only over the tailnet:
    # the daemon binds the tailnet's addresses and nothing else, and the
    # firewall opens it on the tailnet's interface and nowhere else.
    # The default is `ronoctua_player::remote::PORT`'s.
    port = lib.mkOption {
      type = lib.types.port;
      default = 45517;
      description = "The TCP port the player is joined on, over the tailnet only.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ entry ];
    # `xdg.mime` is what links `share/mime` and `share/applications` and runs
    # `update-mime-database` over them; the defaults are the entry's own file.
    xdg.mime.enable = true;

    networking.firewall.interfaces.${config.services.tailscale.interfaceName}.allowedTCPPorts = [
      cfg.port
    ];

    systemd.user.services.ronoctua-player = {
      description = "ronoctua — the player";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      environment =
        import ../player-tools.nix {
          inherit pkgs;
          # MATROSKA'S TAGS WRITTEN only where the media profile is on: its
          # tools pull Qt, which only that profile already carries
          # (`profile-minimal-closes` proves the other one stays without).
          matroska = config.ronoctua.media.enable;
        }
        # YT-DLP ONLY WHERE THE MEDIA PROFILE IS ON, for the queue's reason:
        # a path here is a reference, and it would drag yt-dlp into the
        # minimal profile through this unit.
        // lib.optionalAttrs config.ronoctua.media.enable {
          RONOCTUA_YT_DLP = "${pkgs.yt-dlp}/bin/yt-dlp";
        }
        # AND THE PO TOKEN'S PAIR, exactly when the queue has it: playing
        # YouTube anonymously needs the same attestation downloading does.
        // {
          RONOCTUA_PLAYER_PORT = toString cfg.port;
          # MPRIS: `Raise` raises the column through the shell, and
          # `SupportedMimeTypes` is the list the desktop entry names.
          RONOCTUA_SHELL = "${ronoctua.shell}/bin/ronoctua-shell";
          RONOCTUA_PLAYER_TYPES = "${entry}/share/ronoctua/player-types";
        }
        // lib.optionalAttrs (config.ronoctua.transfers.poToken == "on-demand") {
          RONOCTUA_POT_HOME = "${ronoctua.potProvider}/server";
          RONOCTUA_DENO = lib.getExe pkgs.deno;
        }
        # THE AMBIENT SOUNDS (`nix/ambience.nix`): the loops and the index the
        # daemon reads, ~42 MiB, with the player and never without it. Named
        # only where the flake has them — their originals are a Release asset
        # (`ambience-v1`), and until it is published a player has none and
        # says so on screen (`Trouble::NoAmbience`), never a silent list.
        // lib.optionalAttrs (ronoctua ? ambience) {
          RONOCTUA_AMBIENCE = "${ronoctua.ambience}";
        }
        # THE WORLD, for the directories' globe (`engine/directory.rs`): where
        # each country's name goes on a map, so a radio or TV page arrives
        # with its marks placed and a door only draws them. The same table
        # the doors draw the globe with, read at run time as they read it
        # (`atlas-is-at-run-time`) — and without it every row is said to have
        # no place on the map, never placed at a guess.
        // lib.optionalAttrs (ronoctua ? atlas) {
          RONOCTUA_ATLAS = "${ronoctua.atlas}/world.atlas";
        };
      serviceConfig = {
        ExecStart = "${ronoctua.player}/bin/ronoctua-player";
        Restart = "on-failure";
        RestartSec = 1;
        # `session.slice`, beside the footer: music chosen by a person is not
        # an application oomd may pick at random.
        Slice = "session.slice";
      };
    };
  };
}
