# The toolchain that builds this repository — ONE list, for two readers.
#
# # Who reads it
#
# `nix/devshell.nix` puts it on the devshell's PATH, and `nix/modules/dev.nix`
# keeps it in every installed system's store (`system.extraDependencies`), in
# both profiles. It used to be written once, inside the devshell, and an
# installed machine reached it only by DOWNLOADING it the first time somebody
# typed `nix develop` — a laptop that installed the distro offline, or on a
# metered line, could not start working on the distro it runs.
#
# # Why the same FILE and not the same VALUE
#
# The devshell is built from the flake's `pkgs`; a system is built from its own
# `pkgs` (`nix/modules/nixpkgs-config.nix`: the same nixpkgs, the same two
# overlays, its own unfree policy). Handing the flake's packages into the
# modules would have tied every machine's evaluation to the flake's instance.
# Instead each side calls this file with the `pkgs` it has, and the two lists
# are the SAME DERIVATIONS because the inputs are — which is a claim, and
# `devshell-is-in-the-store` is what holds it: every toolchain input of the
# devshell, output by output (`jq.dev` as well as `jq`), and its stdenv are
# among the system's kept paths, in both profiles. An overlay added to one
# side and not the other turns that check red before it turns a machine into
# a download. What the checkout generates (hooks, WIT, buffers, the `-rs`
# tables) and the programs the build bakes in (`atRunTime`) follow the
# commit: the first `nix develop` builds them, and what those builds need
# from outside (a buffer's crates) still comes from the network.
#
# # Why `extraDependencies` and not `systemPackages`
#
# The store has to HOLD these; the PATH does not have to SHOW them. Two
# Pythons, a second `pkg-config` and Mesa's tools on every shell's PATH is
# noise a person did not ask for. Rust is the exception, and it is on the PATH
# through the program registry (`ronoctua.apps.rust`, `nix/modules/dev.nix`) —
# the SAME derivation as here, so "works in the devshell, fails outside" is
# not a second compiler's doing.
{ pkgs }:

let
  # Section 7.6: native buffers and the design system compile to wasip2; the
  # `wasm32-unknown-unknown` target is for what a browser loads.
  rust = pkgs.rust-bin.stable.latest.default.override {
    extensions = [
      "rust-src" # rust-analyzer needs it to look inside std
      "rust-analyzer"
      "clippy"
      "rustfmt"
    ];
    targets = [
      "wasm32-wasip2"
      "wasm32-unknown-unknown"
    ];
  };

  # The font scripts (`nix/scripts/*.py`): subsetting, WOFF2 (brotli, zopfli)
  # and the rasteriser the coverage probes use.
  python = pkgs.python3.withPackages (ps: [
    ps.fonttools
    ps.brotli
    ps.zopfli
    ps.freetype-py
  ]);
in
rec {
  inherit rust python;

  # WHAT IS ON THE DEVSHELL'S PATH.
  tools = [
    rust
    python

    # --- Nix ---------------------------------------------------------
    pkgs.nixfmt
    pkgs.nix-tree
    pkgs.nvd

    # --- Wayland / graphics ------------------------------------------
    # Needed for the nested loop (section 7.12, speed 1).
    pkgs.wayland
    pkgs.wayland-protocols
    pkgs.libxkbcommon
    pkgs.vulkan-loader
    pkgs.vulkan-tools
    # Software Vulkan rasteriser. Section 9.3 of the plan: it is what runs
    # in the correctness tests. Performance is NEVER measured under it.
    pkgs.mesa

    # --- VM and images -----------------------------------------------
    pkgs.qemu
    pkgs.OVMF
    pkgs.swtpm # emulated TPM2, for the PCR 15 test (milestone 8)

    # --- tools -------------------------------------------------------
    # No `git-lfs` here: it was for the fonts, and they never enter git at all
    # now — they are Release assets. Nothing in this tree is an LFS object.
    pkgs.git
    pkgs.jq
    pkgs.ripgrep
    pkgs.fd
    pkgs.pkg-config
  ];

  # WHAT THE WORKSPACE LINKS, as in `nix/rust.nix`'s `buildInputs`: SQLite
  # for the catalogues and libpam for `ronoctua-pam` — build inputs of the
  # devshell, so pkg-config finds their `.pc` files the way crane's build does.
  links = [
    pkgs.sqlite
    pkgs.linux-pam
  ];

  # What the renderer's loader finds at run time inside the devshell (wgpu
  # opens them by name).
  runtimeLibraries = [
    pkgs.wayland
    pkgs.libxkbcommon
    pkgs.vulkan-loader
  ];

  # WHAT A SYSTEM KEEPS, OUTPUT BY OUTPUT, as the devshell asks for them.
  # `mkShell` hands every input — tools and links alike — through
  # `chooseDevOutputs`, so `nix develop` realises `jq.dev`, `wayland.dev`,
  # `vulkan-loader.dev` and not only their default outputs; a store holding
  # the default output alone still sends `nix develop` to the network for
  # the other (`getDev` is the default output where there is no `dev`). The
  # default outputs stay too: they are what the PATH and the loader use.
  # And the shell's own `stdenv` (mkShell's builder, with the C compiler
  # `cargo` links through): `nix develop` sources its setup before anything.
  kept = tools ++ map pkgs.lib.getDev tools ++ links ++ map pkgs.lib.getDev links ++ [ pkgs.stdenv ];
}
