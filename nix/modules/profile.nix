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
# the compositor, the footer, the renderer's primary face, the keyboard, the network,
# impermanence, the terminal and `cli.nix`. What leaves is applications — the
# things one installs to USE, not the things one needs to log in.
#
# Measured on 2026-08-14, closure of `nixosConfigurations.noctua`: `full` is
# 23,2 GiB. The four groups below are ~6,5 GiB (Android), ~4,0 GiB (toolchains),
# ~4,6 GiB (font variants and icon superpack) and ~2,0 GiB (browser, editor,
# media). (Since 2026-10-05 rust and its ~1,3 GiB left the toolchains group:
# the devshell's toolchain is in both profiles — see the end of the table.)
#
# Measured on 2026-09-30, what the two newest groups ADD to it — `nix
# path-info -rS` of each package, minus every path the full profile's other
# packages already bring (their closures read from cache.nixos.org, so a
# dependency shared with one of our own packages counts as added: these are
# ceilings): Blender 2,55 GiB (its closure is 3,69, most of it LLVM and
# clang's libraries, OpenUSD and MaterialX), OrcaSlicer 0,89 GiB (2,11;
# OpenCASCADE and WebKitGTK), the two together 3,33 GiB (they share Python,
# OpenVDB and Boost); Codex 0,30 GiB and Claude Code 0,22 GiB (0,31; unfree,
# not on the cache — measured on a local build). All four: ~3,9 GiB, so the
# full profile is ~26,7 GiB with them.
#
# ESTIMATED on 2026-10-05, after the devshell's toolchain and the coding
# agents went into both profiles (the arithmetic is at the end of the table,
# after the L15 anchor): `full` ~29 GiB, `minimal` ~11,8 GiB — the figures
# `docs/INSTALL.md`'s table gives. Estimates and not measurements: neither
# toplevel was built for them, and `docs/PENDING.md` keeps the measurement.
#
# | profile   | 2026-08-14 / 09-30 (measured) | 2026-10-05 (estimated) |
# |-----------|-------------------------------|------------------------|
# | `full`    | 23,2 → 26,7 GiB               | ~29 GiB                |
# | `minimal` | 6,9 GiB                       | ~11,8 GiB              |
{ config, lib, ... }:

let
  full = config.ronoctua.profile == "full";
in
{
  config.ronoctua = {
    # Android Studio 3,24 GiB + Flutter 0,74 + Dart 1,18 + JDK 0,46 + the SDK.
    # The single largest item in the distro.
    dev.android = lib.mkDefault full;
    # clang/llvm 1,3, go 0,5, python/pyright 0,4, node, gcc. Building THIS
    # project does not depend on them: `nix develop` brings its own toolchain,
    # pinned, and that is where the work happens. Rust (1,3 GiB with its docs)
    # is no longer behind this switch: it IS that toolchain's compiler, on the
    # PATH in both profiles (the end of the table).
    dev.toolchains = lib.mkDefault full;

    # Chrome, 0,42 GiB. The one the owner named as the example: on a reduced
    # machine, `ronoctua.browser.enable = true` downloads 0,42 GiB, against the
    # 23,2 GiB of installing the full profile to get at it.
    browser.enable = lib.mkDefault full;
    # Emacs 30 and its daemon, ~0,5 GiB: what the package's closure adds to
    # either profile, counted as the figures above are (a ceiling). Measured
    # again for the pure-GTK build that replaced the X11 one (2026-10-19):
    # 491 MiB to `full` and 493 to `minimal`, against the X11 build's 503 and
    # 506 — GTK 3 is there already for other programs, and the X toolkit's
    # libraries (Xaw3d, Xt, m17n) leave with it. It is Emacs alone now — the
    # Doom framework's tools that rode along until then were another
    # 0,23 GiB, most of it pandoc.
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
    # THE PLAYER FOLLOWS THE MEDIA, and it is not a small thing: libmpv over
    # `ffmpeg-full` is a closure of 1 487 MiB and the decoders another 670 —
    # 1 682 MiB together, measured with `nix path-info -r`, most of it
    # `ffmpeg-full`'s own reach (python, samba, opencv, flite, qtbase). A
    # minimal system has no mpv and no VLC either; one line turns it on.
    player.enable = lib.mkDefault full;

    # THE PRIMARY FACE IS NOT HERE, and that is the whole point of the line.
    #
    # `fonts.full` is the 27 SemiExtended variants (2,00 GiB of ttf-nerdfonts +
    # 1,39 of ttf + 0,61 of woff2). The face the renderer actually draws with
    # comes from `nix/primary-font.nix` and is installed either way — a reduced
    # machine looks like this distro, it does not look like a fallback.
    fonts.full = lib.mkDefault full;

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

    # THE PERSON'S OWN PROGRAMS: two of the three supports follow the profile.
    # `appimage-run` is an FHS environment whose closure is 1,53 GB on its own,
    # and Waydroid's is 0,72 GB before any Android image — both measured with
    # `nix path-info -S`. FLATPAK IS NOT HERE, and on purpose: it is how a
    # reduced machine reaches a graphical program the flake does not carry,
    # which is the one thing a reduced machine cannot do without.
    software.appimage = lib.mkDefault full;
    software.android = lib.mkDefault full;

    # THE OFFLINE ENGLISH DICTIONARY, 17 MB converted from the Open English
    # WordNet (`nix/wordsgen.nix`). Without it English is asked online, with
    # consent, like every other language — a reduced machine loses the offline
    # answer, not the answer.
    words.offline = lib.mkDefault full;

    # THE PERSON'S CONTAINERS: Podman is a toolchain one installs to USE —
    # podman, conmon, crun, netavark, passt and `podman-compose` — and the
    # images it pulls land in the home, not the closure.
    containers.enable = lib.mkDefault full;

    # THE WORKSHOP, Blender and OrcaSlicer (~3,3 GiB): programs one installs
    # to USE. A reduced machine that opens a `.stl` gets the sentence that
    # nothing opens it, and one line in `host.nix` changes it:
    # `ronoctua.workshop.enable = true;`. (The coding agents left this line:
    # see the end of the table.)
    workshop.enable = lib.mkDefault full;

    # THE 2026-10-05 WAVE: one region per package, after its anchor.
    # ── anchor: L04 ──
    # THESE SWITCHES ARE THE GROUPS OF THE PROGRAM REGISTRY
    # (`ronoctua.apps.<id>.group`, named once in `nix/modules/distro-apps.nix`):
    # a program of the distro is on a machine when its group's switch is, and
    # a person's decline (`software/declined.json`) takes it off again —
    # `/etc/ronoctua/distro-apps.json` says which.
    # ── anchor: L13 ──
    # INCUS, the person's system containers and virtual machines: QEMU, OVMF,
    # LXC and the daemon are a toolchain one installs to USE, and the images
    # land in `/var/lib/incus`, not the closure. The owner's decision: full
    # only (2026-10-05).
    incus.enable = lib.mkDefault full;
    # ── anchor: L15 ──
    # THE CODING AGENTS, Claude Code and Codex, IN BOTH PROFILES (the owner's
    # decision, 2026-10-05): working ON the distro is what the reduced
    # profile is for too — it is the one that reinstalls in minutes — and the
    # agents are part of that work. ~0,5 GiB (measured above); a person still
    # declines either through the registry.
    dev.agents = lib.mkDefault true;
    #
    # AND THE DEVSHELL'S TOOLCHAIN, kept in the store of both (`nix/toolchain.nix`,
    # wired in `nix/modules/dev.nix`) with rust on the PATH — no switch here:
    # it is not a program one chooses, it is what `nix develop` on this
    # checkout needs. MEASURED on 2026-10-05 (`nix path-info -S` of each path
    # of its closure, minus every path already in the reduced profile's
    # packages): 5,2 GiB, of which ~0,8 GiB is Mesa and its LLVM that the
    # graphics driver of every machine already brings — so ~4,4 GiB added,
    # most of it rust with its documentation and both wasm standard libraries
    # (1,8), OVMF with the EDK2 source it keeps (1,3) and QEMU (0,94).
    # The reduced profile is then ~11,8 GiB (6,9 + 4,4 + the agents' 0,5).
    # The full one gains 2,35 GiB by the same count — rust was in it, but
    # Incus brings a QEMU and an OVMF of its own, not these — so ~29 GiB.
  };
}
