# THE RELEASE PROFILE'S VARIANTS, AS DATA — and the one `[profile.release]` is.
#
# Plain values and no derivation, so anything may read this file at
# evaluation: `nix/rust/profiles.nix` builds each variant from it, the
# `.#profile-bench` app takes its letters from it, and
# `release-profile-is-the-measured-one` holds the manifest to `chosen`. What
# must stay off every evaluation path is what `nix/rust/profiles.nix` makes of
# it — a variant's plan is read from a build (import-from-derivation) — and
# that is `rust.profileVariants` and `legacyPackages.<system>.profileBench`,
# which only the app builds, by name, when it runs.
#
#   A  thin LTO, one codegen unit per crate   (the profile until 2026-10-12)
#   B  no LTO,   one codegen unit per crate
#   C  no LTO,   sixteen codegen units per crate
#   D  thin LTO, sixteen codegen units per crate
#   E  A, with the shell alone in sixteen codegen units (a per-package
#      override: nix-design §3 allows them)
#
# `lto` is spelt as cargo's environment spells it (`false` is rustc's
# crate-local thin LTO, not `off`); `packages` are
# `[profile.release.package.<crate>]` tables (a profile-wide `lto` has no
# per-package form). A is the reference the rule measures every other against.
{
  variants = {
    A = {
      lto = "thin";
      codegenUnits = 1;
    };
    B = {
      lto = "false";
      codegenUnits = 1;
    };
    C = {
      lto = "false";
      codegenUnits = 16;
    };
    D = {
      lto = "thin";
      codegenUnits = 16;
    };
    E = {
      lto = "thin";
      codegenUnits = 1;
      packages.ronoctua-shell.codegen-units = 16;
    };
  };

  # THE VARIANT `[profile.release]` IS, by the measurement of 2026-10-12
  # (`nix run .#profile-bench`): E, the only one besides A that the rule
  # accepted, and the cheapest edited line. Moving it is measuring again.
  chosen = "E";
}
