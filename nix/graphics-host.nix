# How to make a Nix binary find the GPU — shared by `dev` and `bench`.
#
# It exists as a file of its own because the two commands need EXACTLY the
# same setup, and two copies diverge. When they do, `bench` will measure a
# graphics path `dev` does not use, and the number stops meaning anything.
#
# None of this runs on NixOS: there the drivers live in `/run/opengl-driver/lib`
# and the binary finds them on its own. Everything here is about running the
# development loop on ANOTHER distro, which is the common case before the
# distro is installed.
#
# THE DRIVER COMES FROM NIX, AND NOT FROM THE HOST
# ------------------------------------------------
# This used to put the host's `/usr/lib64` on `LD_LIBRARY_PATH` to reach its
# `libEGL.so.1` — the nixGL approach, without the dependency. It worked until
# the host's glibc moved, and then it broke in a way that names nothing:
#
#   scroll: symbol lookup error: /usr/lib64/libc.so.6:
#         undefined symbol: __pointer_chk_guard, version GLIBC_PRIVATE
#
# `LD_LIBRARY_PATH` beats `DT_RUNPATH`, so a directory put there for ONE
# library brings every other library in it — including `libc.so.6`. The
# process then runs the host's libc against Nix's `ld.so`, and glibc's
# `GLIBC_PRIVATE` symbols only match between `libc.so.6` and the `ld.so` of the
# SAME build. Measured on this machine: host 2.44, Nix 2.42, and the symbol
# absent from the host's.
#
# There is no version to juggle. What removes the whole class is not putting
# the host's libraries on the path at all — and for AMD and Intel that costs
# nothing, because what actually talks to the card is the KERNEL driver, and
# mesa is just a userspace library that Nix has a perfectly good copy of.
# Measured: the nested compositor comes up on `wayland-2` with nothing of the host's
# on the path.
#
# NVIDIA is the exception, and it is a real one: its userspace blob has to match
# the kernel module, so it can only come from the host. That path is kept, and
# it prints what it is doing — on an NVIDIA-only machine the glibc mixing above
# is a risk somebody has to know they are taking.
#
# AND THERE IS NO LIST OF X LIBRARIES HERE ANY MORE. There used to be one, for
# a compositor that reached the X platform through `dlopen` and therefore did
# not carry those libraries in its RPATH; without the path it died in
# `EventLoopCreation` with `libXcursor.so.1: cannot open shared object file`,
# which reads like a broken package and is not. This compositor links them,
# measured with `ldd` on the built binary: `libxcb`, `libX11` and
# `libxkbcommon` all resolve through its RUNPATH. So the export went, and with
# it one more chance for the class of failure this file's header is about.
{
  lib,
  buildEnv,
  mesa,
  libgbm,
  libglvnd,
}:

let
  # Everything the graphics stack needs, from Nix: the glvnd loader, mesa's
  # vendor libraries, the DRI drivers and the Vulkan ICDs.
  graphics = buildEnv {
    name = "ronoctua-graphics";
    paths = [
      libglvnd
      mesa
      libgbm
    ];
  };

  # WHICH VULKAN ICD, BY THE PCI VENDOR OF THE CARDS PRESENT.
  #
  # Not for tidiness: the loader loads an ICD for hardware that is not there
  # SUCCESSFULLY — the library exists, so filtering by existence does not help
  # — then looks for a device, finds none, and takes its time. Measured: 650 ms
  # inside `wgpu::Instance::new`, against 29 ms for the whole startup, with no
  # error at all. The footer simply took a while to appear.
  #
  # A table of three because those are the three that exist on a desktop; a
  # vendor that is not here contributes no ICD, and the loader falls back to
  # all of them — slow and correct, rather than fast and wrong.
  #
  # INLINE, and not a shell function held in a Nix string: written as one it
  # came out escaped by `escapeShellArg` and then CALLED, so the shell looked
  # for a program whose name was the body of the `case`. It failed silently —
  # `2>/dev/null || true` — and every ICD was loaded, which is exactly the
  # 650 ms this table exists to avoid.
in
{
  inherit graphics;

  # Shell snippet that prepares the graphics environment. Use with `${...}`.
  prelude = ''
    if [ -z "''${WAYLAND_DISPLAY:-}" ] && [ -z "''${DISPLAY:-}" ]; then
      echo "ERROR: there is no graphical session." >&2
      echo "      The nested compositor runs as a WINDOW inside your" >&2
      echo "      session. With no graphical session, use 'nix run .#vm'." >&2
      exit 1
    fi

    if [ ! -d /run/opengl-driver ]; then
      # WHICH CARDS ARE ACTUALLY IN THIS MACHINE.
      vendors=""
      for v in /sys/class/drm/card*/device/vendor; do
        [ -e "$v" ] || continue
        vendors="''${vendors:+$vendors }$(cat "$v")"
      done

      # NVIDIA ALONE is the one case that needs the host's userspace: the blob
      # has to match the kernel module. Everything else uses Nix's mesa, which
      # is what removes the glibc mixing described at the top of this file.
      only_nvidia=yes
      for v in $vendors; do
        [ "$v" = "0x10de" ] || only_nvidia=no
      done
      [ -n "$vendors" ] || only_nvidia=no

      if [ "$only_nvidia" = yes ]; then
        for d in /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
          if [ -e "$d/libEGL.so.1" ]; then
            export LD_LIBRARY_PATH="$d''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            echo "NVIDIA GPU: the host's driver, from $d"
            echo "  (this mixes the host's libc with Nix's ld.so; if the" >&2
            echo "   compositor dies with 'undefined symbol ... GLIBC_PRIVATE', this is it.)" >&2
            break
          fi
        done
      else
        # THE ORDINARY PATH: everything from Nix, nothing from the host.
        export LD_LIBRARY_PATH="${graphics}/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        export LIBGL_DRIVERS_PATH="${graphics}/lib/dri"
        export __EGL_VENDOR_LIBRARY_DIRS="${graphics}/share/glvnd/egl_vendor.d"
        echo "non-NixOS host: graphics driver from Nix (mesa)"

        icds=""
        for v in $vendors; do
          case "$v" in
            0x1002) name=radeon_icd.x86_64.json ;;
            0x8086) name=intel_icd.x86_64.json ;;
            0x10de) name=nouveau_icd.x86_64.json ;;
            *) name="" ;;
          esac
          [ -n "$name" ] || continue
          j="${graphics}/share/vulkan/icd.d/$name"
          [ -e "$j" ] || continue
          case ":$icds:" in *":$j:"*) continue ;; esac
          icds="''${icds:+$icds:}$j"
        done
        if [ -n "$icds" ]; then
          export VK_ICD_FILENAMES="$icds"
          echo "ICDs Vulkan: $icds"
        fi
      fi
    fi
  '';
}
