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
#   niri: symbol lookup error: /usr/lib64/libc.so.6:
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
# Measured: the nested niri comes up on `wayland-2` with nothing of the host's
# on the path.
#
# NVIDIA is the exception, and it is a real one: its userspace blob has to match
# the kernel module, so it can only come from the host. That path is kept, and
# it prints what it is doing — on an NVIDIA-only machine the glibc mixing above
# is a risk somebody has to know they are taking.
{
  lib,
  buildEnv,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
  libxcb,
  libxkbcommon,
  mesa,
  libgbm,
  libglvnd,
}:

let
  # The libs winit opens by `dlopen` when it falls back to the X11 platform.
  #
  # `libx11` and not `xorg.libX11`: the `xorg` set was deprecated in nixpkgs
  # 26.05 and warns on every evaluation. Five warnings per `nix run .#dev` are
  # five lines of noise in the project's most-run command.
  libsX = lib.makeLibraryPath [
    libx11
    libxcursor
    libxi
    libxrandr
    libxcb
    libxkbcommon
  ];

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
  inherit libsX graphics;

  # Shell snippet that prepares the graphics environment. Use with `${...}`.
  prelude = ''
    if [ -z "''${WAYLAND_DISPLAY:-}" ] && [ -z "''${DISPLAY:-}" ]; then
      echo "ERRO: nao ha sessao grafica." >&2
      echo "      O compositor aninhado roda como JANELA dentro da sua" >&2
      echo "      sessao. Sem sessao grafica, use 'nix run .#vm'." >&2
      exit 1
    fi

    # Host session on X11: winit goes through the X platform and opens the X libs
    # by `dlopen` — they are not in niri's RPATH, and without this it panics
    # in `EventLoopCreation` with "libXcursor.so.1: cannot open shared object
    # file", which looks like a broken package and is not.
    if [ -z "''${WAYLAND_DISPLAY:-}" ]; then
      export LD_LIBRARY_PATH="${libsX}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
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
      only_nvidia=sim
      for v in $vendors; do
        [ "$v" = "0x10de" ] || only_nvidia=nao
      done
      [ -n "$vendors" ] || only_nvidia=nao

      if [ "$only_nvidia" = sim ]; then
        for d in /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
          if [ -e "$d/libEGL.so.1" ]; then
            export LD_LIBRARY_PATH="$d''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            echo "GPU NVIDIA: driver do host, de $d"
            echo "  (isso mistura a libc do host com o ld.so do Nix; se o niri" >&2
            echo "   morrer com 'undefined symbol ... GLIBC_PRIVATE', e isto.)" >&2
            break
          fi
        done
      else
        # THE ORDINARY PATH: everything from Nix, nothing from the host.
        export LD_LIBRARY_PATH="${graphics}/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        export LIBGL_DRIVERS_PATH="${graphics}/lib/dri"
        export __EGL_VENDOR_LIBRARY_DIRS="${graphics}/share/glvnd/egl_vendor.d"
        echo "host nao-NixOS: driver grafico do Nix (mesa)"

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
