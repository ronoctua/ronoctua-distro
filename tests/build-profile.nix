# THE RELEASE PROFILE, CHOSEN BY MEASUREMENT — nix-design §3 (lane N5).
#
# Joined into `tests/checks.nix` by one `inherit`. `nix run .#profile-bench`
# measures the variants of `nix/rust/profiles.nix` and names the one the rule
# picks; these hold the rule's script to its tests and the manifest to the
# measurement.
#
# They read the variants' TABLE (`nix/rust/profile-variants.nix`, plain data),
# never the variants `nix/rust/profiles.nix` builds from it: each of those
# reads a plan from a build, and no check's evaluation may wait for a planner.
{
  pkgs,
  lib,
  self,
  mkCheck,
}:

let
  manifest = builtins.fromTOML (builtins.readFile (self + "/Cargo.toml"));
  release = manifest.profile.release or { };
  inherit (import ../nix/rust/profile-variants.nix) chosen variants;
  measured = variants.${chosen};
  # A variant without a per-package override has no `packages` in the table.
  measuredPackages = measured.packages or { };
  # Cargo's `lto` is a boolean or a string; the variants say it as the
  # environment does.
  ltoOf = v: if builtins.isBool v then lib.boolToString v else toString v;
  shown = v: builtins.toJSON v;
in
{
  # THE DECISION'S SCRIPT DOES WHAT ITS HEADER SAYS: it reads every number
  # `.#bench` prints (and refuses to decide over a p50 or a p99 it cannot
  # read), holds medians to 3% and p99s to 5% of A's (a difference within
  # the printed digit is no measured difference, and a program that is A's
  # own unit is not judged again), refuses a variant a budget failed in or a
  # program of which failed (leaving out only the rounds the harness could
  # not measure), reports every program's size, costs an edit by the units
  # above it, ranks by that cost and then by the cold build (refusing a layer
  # of unknown time), runs every round with its TMPDIR on disk, and refuses a
  # variant whose plan or layer is not the profile it names.
  profile-bench-passes-its-tests = mkCheck "profile-bench-passes-its-tests" [ pkgs.python3 ] ''
    # PARSED: the script, run by its own unit tests.
    cp ${../nix/scripts/profile-bench.py} profile-bench.py
    cp ${../nix/scripts/test_profile_bench.py} test_profile_bench.py
    python3 -m unittest -v test_profile_bench
    echo "OK: the profile bench reads, judges, costs and decides as its tests say"
  '';

  # WHAT SHIPS IS WHAT WAS MEASURED.
  #
  # `[profile.release]` was chosen by `nix run .#profile-bench`: the variant
  # `chosen` in `nix/rust/profile-variants.nix`. A manifest whose release
  # profile is another one — an `lto` or a `codegen-units` changed by hand,
  # or a per-package override the variant does not have (or lacking one it
  # has), making one crate compile another way — ships a profile nobody
  # measured.
  # Changing it means measuring again and moving `chosen` (and the numbers,
  # in the architecture document) with it.
  #
  # # PARSED
  #
  # By Nix's own TOML reader: a comment cannot answer for a setting.
  release-profile-is-the-measured-one =
    let
      problems =
        lib.optional (ltoOf (release.lto or false) != measured.lto)
          "[profile.release] lto is ${shown (release.lto or false)}; variant ${chosen} measured ${measured.lto}"
        ++
          lib.optional ((release.codegen-units or 16) != measured.codegenUnits)
            "[profile.release] codegen-units is ${
              shown (release.codegen-units or 16)
            }; variant ${chosen} measured ${toString measured.codegenUnits}"
        ++
          lib.optional ((release.package or { }) != measuredPackages)
            "[profile.release.package] is ${shown (release.package or { })}; variant ${chosen} measured ${shown measuredPackages}";
    in
    mkCheck "release-profile-is-the-measured-one" [ ] ''
      problems=${lib.escapeShellArg (lib.concatStringsSep "\n" problems)}
      if [ -n "$problems" ]; then
        echo "ERROR: the release profile is not the one that was measured:" >&2
        echo "$problems" | sed 's/^/  /' >&2
        echo "Measure with \`nix run .#profile-bench\` and move \`chosen\` in nix/rust/profile-variants.nix." >&2
        exit 1
      fi
      echo "OK: [profile.release] is variant ${chosen} (lto ${measured.lto}, codegen-units ${toString measured.codegenUnits}), the measured one"
    '';
}
