# The C ABI in front of libtorrent — `libronoctua-torrent.so`.
#
# WHY A DERIVATION OF ITS OWN, and not a `cc` crate inside `build.rs`:
# `nix/rust.nix`'s file set copies `.rs`, `.wit` and `.wgsl` into the store and
# nothing else, so a `.cpp` sitting next to the Rust would simply not exist
# inside the Nix build — and the error would appear there and not in the local
# `cargo build`, which is the exact trap the `.wgsl` comment in that file
# records. Building it here also keeps `crates/ronoctua-torrent/build.rs` a pure
# reader of one environment variable, the same shape `ronoctua-term`'s already
# has for `libghostty-vt`.
#
# WHAT IT COSTS, MEASURED. `libtorrent-rasterbar`'s own closure is 62,7 MiB —
# smaller than `rqbit`'s 69,8 MiB, and an order of magnitude under
# `qbittorrent-nox`'s 886,7 MiB, which is Qt and not the engine. What is bought
# with it is BitTorrent v2 working today: the librqbit of the pinned nixpkgs
# has no `meta version`, no `file tree` and no `piece layers` at all.
{
  lib,
  stdenv,
  pkg-config,
  libtorrent-rasterbar,
  boost,
  openssl,
}:

stdenv.mkDerivation {
  pname = "ronoctua-torrent-shim";
  version = "1";

  # ONLY THE TWO FILES. The whole repository as a source would make this
  # derivation rebuild on every edit anywhere, and it depends on neither.
  src = lib.fileset.toSource {
    root = ./torrent-shim;
    fileset = ./torrent-shim;
  };

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [
    libtorrent-rasterbar
    boost
    openssl
  ];

  # `strictDeps` so a host tool cannot wander into the target's link line.
  strictDeps = true;

  buildPhase = ''
    runHook preBuild
    # `-fvisibility=hidden` plus the `extern "C"` block is what makes the
    # exported surface exactly the twelve functions of `shim.h`. Without it the
    # library also exports every inline C++ symbol libtorrent's headers
    # instantiate, which is tens of thousands of names the Rust must never be
    # able to reach for.
    $CXX -std=c++17 -O2 -fPIC -fvisibility=hidden -shared \
      -o libronoctua-torrent.so shim.cpp \
      $(pkg-config --cflags --libs libtorrent-rasterbar) \
      -Wl,-soname,libronoctua-torrent.so
    runHook postBuild
  '';

  # THE SIZES, MEASURED BY THE C++ COMPILER AND WRITTEN DOWN.
  #
  # `#[repr(C)]` promises to ARRANGE what it was told, never to check it, and a
  # wrong width produces no diagnostic — just a register with the wrong
  # contents. This is what `torrent-layout-matches-cxx` compares the Rust's
  # constants against, and it is generated from the real header rather than
  # copied by hand, because copying by hand is how the numbers went wrong on
  # `libghostty-vt`.
  postBuild = ''
    cat > layout.cpp <<'CPP'
    #include "shim.h"
    #include <cstddef>
    #include <cstdio>
    int main(void) {
      printf("size %zu\n", sizeof(rn_status));
      printf("align %zu\n", alignof(rn_status));
      printf("hash %zu\n", offsetof(rn_status, hash));
      printf("name %zu\n", offsetof(rn_status, name));
      printf("trouble %zu\n", offsetof(rn_status, trouble));
      printf("done %zu\n", offsetof(rn_status, done));
      printf("total %zu\n", offsetof(rn_status, total));
      printf("down_rate %zu\n", offsetof(rn_status, down_rate));
      printf("up_rate %zu\n", offsetof(rn_status, up_rate));
      printf("state %zu\n", offsetof(rn_status, state));
      printf("flags %zu\n", offsetof(rn_status, flags));
      printf("peers %zu\n", offsetof(rn_status, peers));
      printf("seeds %zu\n", offsetof(rn_status, seeds));
      return 0;
    }
    CPP
    $CXX -std=c++17 -I. -o layout layout.cpp
    ./layout > layout.txt
    cat layout.txt
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p $out/lib $out/include $out/share/ronoctua
    cp libronoctua-torrent.so $out/lib/
    cp shim.h $out/include/
    cp layout.txt $out/share/ronoctua/
    runHook postInstall
  '';

  meta = {
    description = "The C boundary in front of libtorrent-rasterbar";
    platforms = lib.platforms.linux;
  };
}
