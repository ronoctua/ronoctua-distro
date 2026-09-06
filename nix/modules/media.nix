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
in
{
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

  # THE VA-API DRIVERS ARE NOT UNDER THE SWITCH, at the bottom of the file: they
  # are hardware enablement and not an application. A reduced machine has the
  # same GPU, and the compositor decodes with them.
  # ONE definition, two halves — `mkMerge` and not two bindings, because an
  # attribute may only be written once in an attrset.
  #
  # The first half is unconditional (the decoders, argued above). The second is
  # the players and the rest, under the switch: `ffmpeg-full`, `exiftool` and
  # `poppler-utils` are NOT in it any more.
  config.environment.systemPackages = lib.mkMerge [
    (with pkgs; [
      ffmpeg-full
      exiftool
      poppler-utils # pdftoppm and pdfinfo — a page as a picture, and a page count
    ])
    (lib.mkIf config.ronoctua.media.enable (
      (with pkgs; [
        # ---- conversion ----
        imagemagick
        sox
        yt-dlp

        # ---- players ----
        mpv # the everyday one: one binary, keyboard-driven
        vlc # the one that opens what mpv refuses

        # ---- images ----
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
        swappy
        libwebp
        libavif
        libheif # what a phone photo actually is
        oxipng

        # ---- documents ----
        ghostscript

        # ---- audio, the session's own ----
        pavucontrol
        crosspipe # the PipeWire patchbay (helvum was removed upstream)
        playerctl # MPRIS: what a media key talks to
        alsa-utils

        # `vainfo` is how one finds out whether hardware decoding actually took.
        libva-utils
      ])
      ++ [ pkgs.gst_all_1.gstreamer ]
      ++ gstPlugins
    ))
  ];

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
