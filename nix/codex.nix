# CODEX, BUILT AGAINST THE SYSTEM LIBRARIES nixpkgs' OWN HOOK ASKS FOR — an
# overlay, so every consumer gets it.
#
# # Why it exists
#
# Between nixpkgs f9a75f4 (2026-11-05) and f4a03c0 (2026-11-07) the cargo setup
# hook began to send every `-sys` crate it knows to the system's library instead
# of the copy the crate would build itself: `AWS_LC_SYS_USE_SYSTEM=1`,
# `ZSTD_SYS_USE_PKG_CONFIG=1`, `LIBSQLITE3_SYS_USE_PKG_CONFIG=1`,
# `RUSTONIG_SYSTEM_LIBONIG=1`, and a `JEMALLOC_OVERRIDE` that names nothing
# unless a jemalloc is among the inputs. Codex 0.167.0 reaches five of them
# (its `Cargo.lock`: `aws-lc-sys`, `zstd-sys`, `libsqlite3-sys`, `onig_sys`,
# `tikv-jemalloc-sys`) and lists none, so its build panicked one crate at a
# time — and every system that carries Codex failed with it, the gate's VMs
# among them.
#
# # What it changes
#
# Only `buildInputs`, with those five libraries, which is what nixpkgs' other
# packages over the same crates do (`cargo-binstall`, `apt-swarm`). And only
# the ones nixpkgs' Codex lacks: once its own expression lists them all, the
# override is empty, the derivation is nixpkgs' to the byte, and the public
# cache serves it again.
#
# # Who reads it
#
# The flake's `pkgs` and the system's (`nix/modules/nixpkgs-config.nix`), as
# `nix/yt-dlp.nix`.
final: prev: {
  codex = prev.codex.overrideAttrs (
    old:
    let
      inputs = old.buildInputs or [ ];
      missing = builtins.filter (lib: !builtins.elem lib inputs) [
        final.aws-lc
        final.zstd
        final.sqlite
        final.oniguruma
        final.rust-jemalloc-sys
      ];
    in
    if missing == [ ] then { } else { buildInputs = inputs ++ missing; }
  );
}
