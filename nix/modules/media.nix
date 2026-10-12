# Audio, video and images — codecs, players and the tools that convert.
#
# WHY `ffmpeg-full` AND NOT `ffmpeg`
# ----------------------------------
# The default build leaves out the codecs whose licence is awkward, and the
# symptom is the worst kind: a file that opens everywhere else and here reports
# "unknown codec". `ffmpeg-full` carries them. It costs closure, and that is the
# trade — measured against having to find out per-file which build is in use.
#
# GStreamer is here for the same reason: it is what GTK apps, the browser's
# fallbacks and any thumbnailer reach for, and half a GStreamer is a system that
# plays some files.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  gstPlugins = with pkgs.gst_all_1; [
    gst-plugins-base
    gst-plugins-good
    gst-plugins-bad
    gst-plugins-ugly
    gst-libav
    gst-vaapi
  ];
  # The player's table, asked for its Matroska half (below).
  matroska = import ../player-tools.nix {
    inherit pkgs;
    matroska = true;
  };
in
{
  # The registry this module writes its programs into (see `browser.nix` for
  # why it is imported here too).
  imports = [ ./distro-apps.nix ];

  options.ronoctua.media.enable =
    lib.mkEnableOption "the players and converters: ffmpeg-full, VLC, mpv, gstreamer (~1.0 GiB)"
    // {
      default = true;
    };

  # WHERE THE SCREENSHOT'S EDITOR IS, for the notification's `edit` action.
  #
  # From the ENVIRONMENT and not baked into the binary: a store path compiled in
  # with `option_env!` is a reference the closure carries whether the switch is
  # on or not — the measured trap `RONOCTUA_ICONS` carries a comment about, and
  # the reason this is the second variable to take this route.
  #
  # BOTH PLACES, for the reason written over there: `sessionVariables` reaches a
  # login shell, and the footer is a systemd USER UNIT, which inherits the
  # manager's environment rather than the shell's.
  #
  # Unset on a minimal install, and the action simply is not offered — an offer
  # that does nothing is worse than one that was never made.
  config.environment.sessionVariables.RONOCTUA_SWAPPY = lib.mkIf config.ronoctua.media.enable "${pkgs.swappy}/bin/swappy";
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_SWAPPY =
    lib.mkIf config.ronoctua.media.enable "${pkgs.swappy}/bin/swappy";

  # ===================================================================
  # THE DECODERS ARE NOT UNDER THE SWITCH — the owner's decision
  # ===================================================================
  #
  # `ffmpeg-full`, `exiftool` and `poppler-utils` are installed on EVERY
  # profile. They were under `ronoctua.media.enable`, which
  # `nix/modules/profile.nix` makes `full`-only, and the consequence was that a
  # reduced install could not preview a video, a phone photo or a RAW file, and
  # could not say what a file's metadata was. The owner asked for `ffmpeg-full`
  # and complete format support, and removed the ISO ceiling that was the one
  # argument on the other side.
  #
  # WHY THESE THREE AND NOT THE WHOLE LIST. This is format support, not
  # applications. `ffmpeg` decodes HEIC, AVIF, WebP, every RAW the file
  # manager will meet and every video container, and it does it in one
  # program — measured on this machine: a phone HEIC in 0,09 s, a 17 MB DNG in
  # 0,21 s, a frame out of a 355 MB mp4 in 0,17 s. `imagemagick` is NOT here
  # and that is a measurement too: `magick` DUMPED CORE on the same DNG. A
  # second converter would be a second failure mode for formats the first one
  # already reads.
  #
  # The players — mpv, VLC, the GStreamer plugins — stay behind the switch.
  # They are what one installs to USE, and that is the line `profile.nix`
  # already draws.

  # WHERE THOSE PROGRAMS ARE, for the file manager's preview and info panels.
  #
  # From the ENVIRONMENT and not baked in with `option_env!`, for the reason
  # written over `RONOCTUA_SWAPPY` above and measured twice in this repository:
  # a store path compiled in is a reference the closure carries whether it is
  # used or not. It matters here even though these are now unconditional,
  # because `ronoctua-files` also builds the binary that runs as ROOT, and that
  # closure must not learn about video.
  #
  # NOT under `mkIf`: they are installed on every profile now, and a variable
  # that pointed at a program only sometimes would put the honest-refusal path
  # on the everyday route instead of where it belongs — a broken environment.
  #
  # BOTH PLACES, for the reason written above: `sessionVariables` reaches a
  # login shell, and the footer is a systemd USER UNIT, which inherits the
  # manager's environment rather than the shell's.
  config.environment.sessionVariables.RONOCTUA_FFMPEG = "${pkgs.ffmpeg-full}/bin/ffmpeg";
  config.environment.sessionVariables.RONOCTUA_FFPROBE = "${pkgs.ffmpeg-full}/bin/ffprobe";
  config.environment.sessionVariables.RONOCTUA_EXIFTOOL = "${pkgs.exiftool}/bin/exiftool";
  config.environment.sessionVariables.RONOCTUA_PDFTOPPM = "${pkgs.poppler-utils}/bin/pdftoppm";
  config.environment.sessionVariables.RONOCTUA_PDFINFO = "${pkgs.poppler-utils}/bin/pdfinfo";
  # AND THE SPREADSHEET, which is the ONE old office format still read by a
  # program.
  #
  # `.doc`, `.xls` and `.ppt` are OLE compound files — a filesystem inside a
  # file — and nothing about them is XML. `catdoc` is 36,6 MiB measured
  # (`nix path-info -S`) and brings three programs in one package.
  #
  # RE-MEASURED AGAINST THE BINARY THIS FLAKE PINS, on the owner's own files,
  # and the earlier number in this comment was WRONG: it said `catdoc
  # sample.doc` answered 8 377 bytes, which was measured against a `catdoc`
  # from another nixpkgs. The one that ships answers **zero bytes and exit 69**,
  # with nothing on stderr — so the panel said "the file may be damaged" about
  # a perfectly good Word document, and the owner reported it twice.
  # `catppt sample.ppt` answers zero bytes with exit status zero. Only
  # `xls2csv sample.xls` works: 364 057 bytes.
  #
  # So two of the three are gone and this package stays for ONE program:
  # `ronoctua_inspect::doc` walks the piece table and `::ppt` the records,
  # neither of which needs anything installed. A spreadsheet is BIFF, which is
  # neither, and nothing in this tree reads one.
  #
  # AND NOT LIBREOFFICE, which reads all three: two to three gigabytes, and the
  # line this file already draws is that this is format support and not
  # applications.
  config.environment.sessionVariables.RONOCTUA_XLS2CSV = "${pkgs.catdoc}/bin/xls2csv";
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_FFMPEG =
    "${pkgs.ffmpeg-full}/bin/ffmpeg";
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_FFPROBE =
    "${pkgs.ffmpeg-full}/bin/ffprobe";
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_EXIFTOOL =
    "${pkgs.exiftool}/bin/exiftool";
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_PDFTOPPM =
    "${pkgs.poppler-utils}/bin/pdftoppm";
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_PDFINFO =
    "${pkgs.poppler-utils}/bin/pdfinfo";
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_XLS2CSV =
    "${pkgs.catdoc}/bin/xls2csv";
  # AND MATROSKA'S THREE, for the files panel's tag program: the footer runs
  # `ronoctua-tags` as a child, and the child reads its tools from the
  # environment it inherits. Without them a `.mkv` the player can retag was
  # "cannot be changed here: mkvpropedit" in the panel. The SAME table and the
  # same switch as the player's unit (`nix/modules/player.nix`): mkvtoolnix
  # pulls Qt, which only the media profile carries.
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_MKVEXTRACT =
    lib.mkIf config.ronoctua.media.enable matroska.RONOCTUA_MKVEXTRACT;
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_MKVMERGE =
    lib.mkIf config.ronoctua.media.enable matroska.RONOCTUA_MKVMERGE;
  config.systemd.user.services.ronoctua-footer.environment.RONOCTUA_MKVPROPEDIT =
    lib.mkIf config.ronoctua.media.enable matroska.RONOCTUA_MKVPROPEDIT;

  # PLAYERCTL IS NOT HERE, and that is the point: it moved to
  # `nix/modules/base.nix`. The media keys are bound by the compositor's config
  # unconditionally, so the program they call has to exist unconditionally too
  # — under this switch, `ronoctua.profile = "minimal"` would leave three bare
  # keys dead with nothing on screen saying why.

  # THE VA-API DRIVERS ARE NOT UNDER THE SWITCH, at the bottom of the file: they
  # are hardware enablement and not an application. A reduced machine has the
  # same GPU, and the compositor decodes with them.
  #
  # THE PROGRAMS ARE THE REGISTRY'S (`nix/modules/distro-apps.nix`), one entry
  # each, so a person can keep any of them off a machine. The decoders are in
  # the `base` group — every profile, the owner's decision argued above — and
  # declining one takes the PROGRAM off the `PATH` and nothing from the file
  # manager, which runs it by the store path in the variables above. The rest
  # is the `media` group, this module's switch.
  #
  # WHAT STAYS HERE is not a program anybody starts: the screenshot editor
  # the notification offers (by `RONOCTUA_SWAPPY`, so its action disappears
  # with this switch) and GStreamer, which GTK applications load.
  config.ronoctua.apps =
    lib.mapAttrs
      (_: package: {
        inherit package;
        group = "base";
      })
      {
        ffmpeg = pkgs.ffmpeg-full;
        inherit (pkgs) exiftool;
        # pdftoppm and pdfinfo — a page as a picture, and a page count.
        poppler-utils = pkgs.poppler-utils;
      }
    //
      lib.mapAttrs
        (
          _: e:
          {
            group = "media";
          }
          // e
        )
        {
          # ---- conversion ----
          imagemagick.package = pkgs.imagemagick;
          sox.package = pkgs.sox;
          yt-dlp.package = pkgs.yt-dlp;

          # ---- players ----
          # The everyday one: one binary, keyboard-driven.
          mpv = {
            package = pkgs.mpv;
            desktopIds = [
              "mpv.desktop"
              "umpv.desktop"
            ];
          };
          # The one that opens what mpv refuses.
          vlc = {
            package = pkgs.vlc;
            desktopIds = [ "vlc.desktop" ];
          };

          # ---- images ----
          libwebp.package = pkgs.libwebp;
          libavif.package = pkgs.libavif;
          # What a phone photo actually is.
          libheif.package = pkgs.libheif;
          oxipng.package = pkgs.oxipng;

          # ---- documents ----
          ghostscript.package = pkgs.ghostscript;

          # ---- audio, the session's own ----
          pavucontrol = {
            package = pkgs.pavucontrol;
            desktopIds = [ "org.pulseaudio.pavucontrol.desktop" ];
          };
          # The PipeWire patchbay (helvum was removed upstream).
          crosspipe = {
            package = pkgs.crosspipe;
            desktopIds = [ "io.github.dp0sk.Crosspipe.desktop" ];
          };
          alsa-utils.package = pkgs.alsa-utils;

          # `vainfo` is how one finds out whether hardware decoding actually took.
          libva-utils.package = pkgs.libva-utils;
        };

  config.environment.systemPackages = lib.mkIf config.ronoctua.media.enable (
    [
      # THE SCREENSHOT'S EDITOR, and the reason it is HERE and not in
      # `base.nix`: the notification the compositor raises after a capture
      # offers to edit it, and `swappy` is what that action starts. It is
      # Wayland-native, a few MiB, and it is the whole of the "edit" the owner
      # asked for.
      #
      # Under the `media` switch, so a minimal install simply does not offer
      # the action — see `usable_actions` in the shell: an action whose program
      # is missing DISAPPEARS from the notification rather than failing when
      # clicked.
      pkgs.swappy
      pkgs.gst_all_1.gstreamer
    ]
    ++ gstPlugins
  );

  # THE PLUGIN PATH, EXPLICITLY.
  #
  # Installing the plugins is not enough: GStreamer finds them through
  # `GST_PLUGIN_SYSTEM_PATH_1_0`, and without it an app links the library and
  # then finds no decoder — which looks like a broken file, not a missing path.
  # Under the switch too: a path pointing at plugins nobody installed is worse
  # than no path, because it looks configured.
  config.environment.sessionVariables.GST_PLUGIN_SYSTEM_PATH_1_0 =
    lib.mkIf config.ronoctua.media.enable
      (pkgs.lib.makeSearchPathOutput "lib" "lib/gstreamer-1.0" gstPlugins);

  # HARDWARE DECODING.
  #
  # Without VA-API a 4K video decodes on the CPU: the fan spins up, the battery
  # goes, and the diagnosis becomes "this laptop is slow". `intel-media-driver`
  # covers Gen8 onward; the legacy driver covers what came before, and having
  # both costs a few MB and removes a whole class of question.
  config.hardware.graphics.extraPackages = with pkgs; [
    intel-media-driver
    libvdpau-va-gl
    libva-vdpau-driver
  ];
}
