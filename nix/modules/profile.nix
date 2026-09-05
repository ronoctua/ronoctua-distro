# The profile — a TABLE, and nothing else.
#
# # Why this file exists at all
#
# `ronoctua.profile` says which of the two installations a machine is, and every
# group it turns off already had a switch of its own before this file existed
# (`ronoctua.fonts.full`, `ronoctua.icons.superpack`, `ronoctua.dev.android`).
# What was missing was one place saying which switches go together.
#
# It could have been written inside `base.nix`, and it is not on purpose: the
# switches live in eight different modules, and reading them from `base.nix`
# would make `base.nix` require all eight — the exact failure its own comment
# describes for `dev.android`. Here it costs nothing, because this module is only
# ever imported alongside the whole `distroModules` list.
#
# # Why everything is `lib.mkDefault`
#
# The table is the WEAKEST voice in the room. `host.nix` (the machine's own file)
# and `vm.nix` set these keys directly, and they must go on winning — the dev VM
# turns Android off regardless of profile, for a reason written over there.
#
# # What `minimal` is NOT
#
# It is not a second distro. Everything that makes this system what it is stays:
# niri, the footer, the renderer's primary face, the keyboard, the network,
# impermanence, the terminal and `cli.nix`. What leaves is applications — the
# things one installs to USE, not the things one needs to log in.
#
# Measured on 2026-08-14, closure of `nixosConfigurations.noctua`: `full` is
# 23,2 GiB. The four groups below are ~6,5 GiB (Android), ~4,0 GiB (toolchains),
# ~4,6 GiB (font variants and icon superpack) and ~2,0 GiB (browser, editor,
# media).
{ config, lib, ... }:

let
  full = config.ronoctua.profile == "full";
in
{
  config.ronoctua = {
    # Android Studio 3,24 GiB + Flutter 0,74 + Dart 1,18 + JDK 0,46 + the SDK.
    # The single largest item in the distro.
    dev.android = lib.mkDefault full;
    # rust + rust-docs 1,3 GiB, clang/llvm 1,3, go 0,5, python/pyright 0,4, node,
    # gcc. Building THIS project does not depend on them: `nix develop` brings
    # its own toolchain, pinned, and that is where the work happens.
    dev.toolchains = lib.mkDefault full;

    # Chrome, 0,42 GiB. The one the owner named as the example: on a reduced
    # machine, `ronoctua.browser.enable = true` downloads 0,42 GiB, against the
    # 23,2 GiB of installing the full profile to get at it.
    browser.enable = lib.mkDefault full;
    # Emacs 30 and the Doom dependencies, ~0,6 GiB.
    editor.emacs = lib.mkDefault full;
    # VLC, mpv, imagemagick, ghostscript and the gstreamer plugins.
    #
    # THE DECODERS ARE NO LONGER IN HERE. `ffmpeg-full`, `exiftool` and
    # `poppler-utils` moved out from behind this switch by the owner's
    # decision: they are format SUPPORT, and a reduced machine that cannot
    # preview a video or read a photograph's metadata is not a reduced
    # distro, it is a broken one. What this line still turns off is the
    # things one installs to USE — which is the line this whole table draws.
    media.enable = lib.mkDefault full;

    # THE PRIMARY FACE IS NOT HERE, and that is the whole point of the line.
    #
    # `fonts.full` is the 27 SemiExtended variants (2,00 GiB of ttf-nerdfonts +
    # 1,39 of ttf + 0,61 of woff2). The face the renderer actually draws with
    # comes from `nix/primary-font.nix` and is installed either way — a reduced
    # machine looks like this distro, it does not look like a fallback.
    fonts.full = lib.mkDefault full;
    fonts.notoExtra = lib.mkDefault full;

    # The 0,61 GiB superpack is a PIXMAP FALLBACK for foreign applications
    # (invariant 10) — no component of the design system consults it, so a
    # reduced machine loses icons for other people's apps and nothing of its own.
    #
    # It was out of this table for a while, and honestly: turning it off removed
    # it from `environment.systemPackages` and left every byte in the closure,
    # because `ronoctua-shell` carried the store path BAKED IN by `option_env!`.
    # The shell reads it from the environment now, so the switch is real and the
    # line means what it says.
    icons.superpack = lib.mkDefault full;
    # Qt's dark platform theme brings Qt5 and Qt6 along (591 MiB measured) for
    # applications the minimal profile does not have.
    foreign.qt = lib.mkDefault full;
  };
}
