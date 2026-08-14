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
{ pkgs, ... }:

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
  environment.systemPackages =
    (with pkgs; [
      # ---- conversion ----
      ffmpeg-full
      imagemagick
      sox
      yt-dlp

      # ---- players ----
      mpv # the everyday one: one binary, keyboard-driven
      vlc # the one that opens what mpv refuses

      # ---- images ----
      libwebp
      libavif
      libheif # what a phone photo actually is
      exiftool
      oxipng

      # ---- documents ----
      poppler-utils # pdftotext, pdfimages
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
    ++ gstPlugins;

  # THE PLUGIN PATH, EXPLICITLY.
  #
  # Installing the plugins is not enough: GStreamer finds them through
  # `GST_PLUGIN_SYSTEM_PATH_1_0`, and without it an app links the library and
  # then finds no decoder — which looks like a broken file, not a missing path.
  environment.sessionVariables.GST_PLUGIN_SYSTEM_PATH_1_0 =
    pkgs.lib.makeSearchPathOutput "lib" "lib/gstreamer-1.0"
      gstPlugins;

  # HARDWARE DECODING.
  #
  # Without VA-API a 4K video decodes on the CPU: the fan spins up, the battery
  # goes, and the diagnosis becomes "this laptop is slow". `intel-media-driver`
  # covers Gen8 onward; the legacy driver covers what came before, and having
  # both costs a few MB and removes a whole class of question.
  hardware.graphics.extraPackages = with pkgs; [
    intel-media-driver
    libvdpau-va-gl
    libva-vdpau-driver
  ];
}
