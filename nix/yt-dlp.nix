# THE ONE yt-dlp OF THE DISTRO — an overlay, so every consumer gets it.
#
# # Why it exists
#
# YouTube changes what it serves faster than nixpkgs moves, and the pinned
# nixpkgs carried yt-dlp 2026.07.04. Measured on 2026-10-04 against
# `aqz-KE-bpKQ` (Big Buck Bunny), with the PO-token plugin loaded and a GVS
# token minted: every format came from the `ANDROID_VR` client, and
# `googlevideo.com` answered 403 to a GET without `Range` and to any range past
# the first ~2 MiB. yt-dlp itself failed the same way (`HTTP Error 403`) and
# warned that it was older than 90 days. The same command under 2026.08.19
# picks the `VISIONOS` client and fetches the whole file.
#
# # What it changes
#
# Only `version` and `src`: the build, the wrapper (ffmpeg, rtmpdump, deno
# baked in by nixpkgs' `postPatch`) and the `yt-dlp-ejs` pin are nixpkgs'
# own. The `yt-dlp-ejs` version yt-dlp's `pyproject.toml` pins must be the one
# nixpkgs has (0.8.0 for both on 2026-10-04); when a bump moves it, this file
# overrides that too.
#
# # Who reads it
#
# The flake's `pkgs` and the system's (`nix/modules/nixpkgs-config.nix`), so
# the player (`RONOCTUA_YT_DLP` and mpv's `ytdl_hook`), the transfers queue,
# the brain's video measurer, fast access and `.#dev` all run the same binary — and mpv's
# own wrapper, which puts yt-dlp on its `PATH`.
#
# # When to bump it
#
# When YouTube refuses again (a 403 mid-file, "Sign in to confirm", formats
# that come back without a URL): take the newest stable tag from
# https://github.com/yt-dlp/yt-dlp/releases, set `version`, put `hash = "";`,
# build, and copy the hash the failure prints. `nix/kept-sources.nix` roots the
# source, so a garbage collection cannot take it.
final: prev: {
  yt-dlp = prev.yt-dlp.overridePythonAttrs (
    _:
    let
      version = "2026.08.19";
    in
    {
      inherit version;
      src = final.fetchzip {
        name = "yt-dlp-${version}-source";
        url = "https://github.com/yt-dlp/yt-dlp/archive/refs/tags/${version}.tar.gz";
        hash = "sha256-BM5ZeGTmHq+1xH6G/zsuCtjLgYgfRA11ya0zIHK5p4g=";
      };
    }
  );
}
