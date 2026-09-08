# `libghostty-vt` — the terminal engine, built from a PINNED COMMIT.
#
# # Why a commit and not a tag, against this project's own rule
#
# `AGENTS.md` says to pin a tag and never `main`, and the reason is good: `main`
# moves under you and a `nix flake update` brings a different API with no
# warning. It is broken here on purpose, and the cost is stated rather than
# hidden.
#
# The last released tag is `v1.3.1`, and its C headers do not let the screen
# cross the boundary: nine headers, none of them `screen.h`. The trait
# `ronoctua_term::Engine` needs `fn grid(&self) -> Grid`, so against that tag the
# engine can only be written as something that compiles and returns an empty
# grid — the failure mode `docs/PENDING.md` names.
#
# On the pinned commit there are thirty-one headers and the API covers the trait
# function by function: `ghostty_terminal_vt_write` feeds, `ghostty_terminal_grid_ref`
# plus `ghostty_grid_ref_row`/`_cell` read, `ghostty_terminal_resize` resizes.
#
# **A COMMIT, and never a branch.** Pinning `main` would mean the API changing
# under a `nix flake update`; pinning a commit means it changes only when
# somebody edits this file. Upstream marks the API "incomplete,
# work-in-progress", so that difference is the whole safety margin.
#
# # Why not `pkgs.ghostty`
#
# It packages `v1.3.1`, which is the version without the API. And its outputs are
# `out`, `man`, `shell_integration`, `terminfo` and `vim` — no `dev`, no library.
{
  lib,
  stdenv,
  fetchFromGitHub,
  # 0.16.0, and NOT the `zig_0_15` the nixpkgs Ghostty uses: the pinned commit
  # declares `minimum_zig_version = "0.16.0"` in its `build.zig.zon`, and an
  # older Zig refuses the file before compiling a line.
  zig,
}:

let
  rev = "53be7d0353640351e2d6a06725779f21a3ebe481";
  src = fetchFromGitHub {
    owner = "ghostty-org";
    repo = "ghostty";
    inherit rev;
    hash = "sha256-vpej7OEyzyizVqkxpqRlRanrb3jYOJG8td5EkhTp+Ac=";
  };

  # THE ZIG PACKAGE CACHE, as a fixed-output derivation.
  #
  # This is the network dependency `docs/PENDING.md` always recorded, and there
  # is no way around it: `build.zig.zon` names thirty-four tarballs on
  # `deps.files.ghostty.org`, and Zig fetches them itself. A FOD is how a build
  # with network stays reproducible — the hash is the promise that what comes
  # down is what came down before.
  #
  # It is NOT the `deps.nix` the nixpkgs Ghostty uses: that file is GENERATED per
  # tag by an updater of theirs, and there is no generated one for this commit.
  deps = stdenv.mkDerivation {
    pname = "ghostty-vt-deps";
    version = rev;
    inherit src;

    nativeBuildInputs = [ zig ];
    dontConfigure = true;
    dontInstall = true;
    dontFixup = true;

    buildPhase = ''
      runHook preBuild
      export ZIG_GLOBAL_CACHE_DIR="$out"
      # `--fetch=all` AND NOT `--fetch`: the plain form resolves only the
      # non-lazy dependencies, and `pkg/oniguruma`, `pkg/glslang` and
      # `pkg/spirv-cross` are lazy. The build then reaches for them inside the
      # sandbox, where there is no network, and fails with `NameServerFailure` —
      # a DNS error for a build that was supposed to need no DNS.
      zig build --fetch=all
      runHook postBuild
    '';

    outputHashMode = "recursive";
    outputHashAlgo = "sha256";
    outputHash = "sha256-Vf3OapiX01msUgnrMmnVP8WTV7YUYOMQRAzhfHuqqtY=";
  };
in
stdenv.mkDerivation {
  pname = "libghostty-vt";
  version = "1.3.2-dev+${builtins.substring 0 8 rev}";
  inherit src;

  nativeBuildInputs = [ zig ];
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    # The cache is read-only in the store, and Zig writes to it. Copying is the
    # price; linking would fail on the first write.
    export ZIG_GLOBAL_CACHE_DIR="$TMPDIR/zig-cache"
    cp -r --no-preserve=mode,ownership ${deps} "$ZIG_GLOBAL_CACHE_DIR"

    # ONLY the VT library, through the option upstream wrote for exactly this:
    # `-Demit-lib-vt` "sets defaults for a libghostty-vt-only build (disables
    # xcframework, macOS app, and docs)". Building all of Ghostty would drag in
    # GTK, the renderer and the application — none of which this needs, and each
    # of which is another way for the build to break for a reason that has
    # nothing to do with the terminal engine.
    #
    # There is NO `lib-vt` STEP, and looking for one costs a build: the library
    # is installed by the DEFAULT step, and the only `lib-vt` name in `build.zig`
    # belongs to `test-lib-vt`.
    # THE TARGET IS ZIG'S TRIPLE, not Nix's. `stdenv.hostPlatform.config` gives
    # `x86_64-unknown-linux-gnu`, and Zig reads the middle field as the OS —
    # answering `unknown OS: 'unknown'`, which sounds like a broken toolchain and
    # is only a spelling difference.
    zig build \
      -Doptimize=ReleaseFast \
      -Dtarget=${
        {
          "x86_64-linux" = "x86_64-linux-gnu";
          "aarch64-linux" = "aarch64-linux-gnu";
        }
        .${stdenv.hostPlatform.system}
      } \
      -Demit-lib-vt=true \
      --prefix "$out"
    runHook postBuild
  '';

  # The build installs into `--prefix` itself; an install phase would copy twice.
  dontInstall = true;

  # THE STATIC LIBRARY GOES, AND IT IS NOT HOUSEKEEPING: it is 1,70 GiB of
  # closure on every machine.
  #
  # `libghostty-vt.a` is the ONLY file in this output that names a Zig store
  # path (checked by grepping the whole tree), and a reference to Zig drags
  # `clang-lib` (0,79 GiB) and `llvm-lib` (0,53 GiB) with it. Since
  # `crates/ronoctua-term/build.rs` asks for `dylib=ghostty-vt`, the archive was
  # never linked against — it was paying for a compiler nobody runs.
  #
  # Deleting it rather than rewriting the paths with `remove-references-to`: a
  # static archive whose references have been blanked links, and then fails
  # somewhere else. Absent, it fails at the link line, which is the truth.
  postFixup = ''
    rm -f "$out/lib/libghostty-vt.a"
  '';

  meta = {
    description = "Ghostty's terminal engine, the VT library alone";
    homepage = "https://github.com/ghostty-org/ghostty";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
  };
}
