# How to make a Nix binary find the GPU — shared by `dev` and `bench`.
#
# It exists as a file of its own because the two commands need EXACTLY the
# same setup, and two copies diverge. When they do, `bench` will
# measure a graphics path `dev` does not use, and the number stops meaning anything.
#
# None of this runs on NixOS: there the drivers live in `/run/opengl-driver/lib`
# and the binary finds them on its own. Everything here is about running the
# development loop on ANOTHER distro, which is the common case before the distro
# is installed.
#
# The packages come in as NAMED arguments. A `...@pkgs` will not do: `callPackage`
# binds only what the function declares, and `pkgs.libx11` would give
# "attribute missing" pointing at this file, not at the call site.
{
  lib,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
  libxcb,
  libxkbcommon,
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
in
{
  inherit libsX;

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
      # NON-NIXOS HOST, part 1: the host's glvnd `libEGL.so.1` is out of
      # reach of the Nix binary, and niri panics with
      # `Egl(DisplayNotSupported)` — a message that mentions no driver at all.
      #
      # `__EGL_VENDOR_LIBRARY_DIRS` + `LIBGL_DRIVERS_PATH` are NOT enough
      # (tested); what solves it is `LD_LIBRARY_PATH`. It is what nixGL does,
      # without the dependency.
      for d in /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
        if [ -e "$d/libEGL.so.1" ]; then
          export LD_LIBRARY_PATH="$d''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
          echo "host nao-NixOS: driver grafico de $d"
          break
        fi
      done

      # NON-NIXOS HOST, part 2: only the Vulkan ICDs of hardware that EXISTS.
      #
      # Several distros install NVIDIA's ICD along with the driver, and the driver
      # along with something else. On a machine with no NVIDIA card the loader loads
      # the ICD SUCCESSFULLY (the library exists — filtering by existence does not
      # help), looks for a device, finds none, and takes its time: measured, 650 ms
      # inside `wgpu::Instance::new`, against 29 ms for the whole startup.
      #
      # There is no error at all; the footer just takes a while to appear.
      #
      # The criterion is the PCI vendor of the DRM devices present. The table
      # has one row because this is the only expensive case in practice.
      has_nvidia=nao
      for v in /sys/class/drm/card*/device/vendor; do
        [ -e "$v" ] || continue
        [ "$(cat "$v")" = "0x10de" ] && has_nvidia=sim
      done

      icds=""
      for j in /usr/share/vulkan/icd.d/*.json; do
        [ -e "$j" ] || continue
        case "$j" in
          *nvidia*) [ "$has_nvidia" = sim ] || continue ;;
        esac
        icds="''${icds:+$icds:}$j"
      done
      if [ -n "$icds" ]; then
        export VK_ICD_FILENAMES="$icds"
        echo "ICDs Vulkan: $icds"
      fi
    fi
  '';
}
