# Scroll — the distro's compositor, consumed upstream and UNPATCHED.
#
# # Why this one
#
# TWO REASONS, and the first is what decided it. The previous compositor drives
# a SINGLE pointer, and several pointers at once is the thing the owner's next
# applications are built on — a limitation with no way around it from outside,
# because it is the seat model itself. Scroll is a fork of sway, which is
# wlroots, which has multi-seat.
#
# The second is that its layout is the one he wanted anyway: infinite
# horizontal scrolling rather than a grid of workspaces.
#
# Everything else about it — the free-text commands, the merged `output`
# blocks, the `reload` that validates before applying — is consequence, and
# where those consequences reach our code they are written down beside the code
# they reach.
#
# # Why an override of sway, and not a package written out
#
# Scroll IS sway: the same meson project, the same forty-odd dependencies, the
# same wrapper. `sway-unwrapped` already carries all of that and is maintained
# by people who watch it, so the honest spelling of "sway with another source"
# is an override of the source. What is written out below is exactly the
# difference, and every line of it was measured, not guessed.
#
# # Why not the third-party flake
#
# `github:Diax170/scroll-flake` packages this and works. It also brings its own
# nixpkgs (nixos-unstable), which would put a SECOND nixpkgs in this project's
# lock file — and that is a second set of every library the desktop links
# against, resolved by a tree nobody here pins. The whole difference between
# that tree and ours turned out to be one patch release of libdrm (below), so
# the cost of staying on one nixpkgs is eleven lines.
#
# # WHERE WLROOTS IS
#
# Nowhere in this file, and that is not an omission. Scroll requires
# `wlroots-0.21`, which HAS NO RELEASE — the newest tag upstream is 0.20.2 — so
# it carries the whole of wlroots inside its own source tree, at
# `subprojects/wlroots`, and meson builds it as a subproject. Two consequences
# worth knowing before anyone commits to this compositor: the wlroots that runs
# is a git snapshot nobody tagged, and the dependencies BELOW are mostly
# wlroots's, not sway's — `glslang`, `libliftoff`, `libdisplay-info`, `hwdata`
# and `lcms` are there for the vendored copy and for nothing else.
{
  lib,
  sway-unwrapped,
  libdrm,
  fetchurl,
  scrollSrc,
  glslang,
  lcms,
  hwdata,
  libliftoff,
  lua5_4,
  vulkan-loader,
  xwayland,
  seatd,
  libdisplay-info,
  libxcb-render-util,
  libxcb-errors,
  libgbm,
  readline,
}:

let
  # ONE PATCH RELEASE OF libdrm, and it is the only thing this project's pinned
  # nixpkgs could not satisfy.
  #
  # Measured, from the meson log of the first build attempt:
  #
  #   wlroots| Dependency libdrm found: NO. Found 2.4.133 but need: '>=2.4.134'
  #   meson.build:49:10: ERROR: Subproject "subprojects/wlroots" required but not found.
  #
  # nixos-26.05 has 2.4.133 and nixos-unstable has 2.4.134 — that difference,
  # and nothing else, is why the third-party flake follows unstable. Bumping the
  # one library is cheaper than adopting the whole tree it lives in.
  #
  # It goes to THIS derivation and to no other: a `libdrm` overlay would rebuild
  # mesa and every graphical package in the closure, to fix a compositor that is
  # being evaluated.
  libdrmForWlroots = libdrm.overrideAttrs (old: rec {
    version = "2.4.134";
    src = fetchurl {
      url = "https://dri.freedesktop.org/libdrm/libdrm-${version}.tar.xz";
      hash = "sha256-rF500VeDDri+5Exqa/OtSXdO8N0qcr2tdKjyAwi1KpU=";
    };
  });
in
sway-unwrapped.overrideAttrs (old: {
  pname = "scroll-unwrapped";
  version = "1.12.21";
  src = scrollSrc;

  # NIXPKGS'S SWAY PATCHES DO NOT APPLY, and they must not.
  #
  # They are written against sway's tree at the revision nixpkgs pins. Applied
  # to a fork's tree they either fail to apply — which is a build error, and the
  # good case — or apply with fuzz to code that has moved, which is the bad one.
  # Invariant 3 is exactly this: consume upstream, do not carry a patch.
  patches = [ ];

  # The vendored wlroots is built with sway's warning flags, and it does not
  # compile clean under them. Upstream's own CI passes `-Wno-error` for the same
  # reason; without it the build fails on a `maybe-uninitialized` in code that
  # is not ours to fix.
  mesonFlags = old.mesonFlags ++ [ "-Dc_args=-Wno-error=maybe-uninitialized" ];

  nativeBuildInputs = old.nativeBuildInputs ++ [
    glslang # the vendored wlroots compiles its Vulkan shaders at build time
    lcms
    hwdata
    libliftoff
  ];

  # `libdrmForWlroots` FIRST, because meson resolves by pkg-config and the
  # search path follows this list: with the pinned 2.4.133 ahead of it, the
  # version check above fails again and the fallback error says "subproject not
  # found", which names neither libdrm nor the version.
  buildInputs = [
    libdrmForWlroots
  ]
  ++ old.buildInputs
  ++ [
    lua5_4 # scroll's animation and layout scripting; sway has no lua at all
    vulkan-loader
    xwayland
    seatd
    lcms
    libdisplay-info
    libxcb-render-util
    libxcb-errors
    libliftoff
    libgbm
    readline
  ];

  meta = old.meta // {
    description = "i3-compatible Wayland compositor (sway fork) with a scrolling layout";
    homepage = "https://github.com/dawsers/scroll";
    mainProgram = "scroll";
  };
})
