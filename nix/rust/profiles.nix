# THE RELEASE PROFILE, CHOSEN BY MEASUREMENT — nix-design §3 (lane N5).
#
# `[profile.release]` in the root `Cargo.toml` decides two things at once: how
# fast the installed programs run, and how long one edited line takes to
# reach a release program. The second is no longer "the workspace again"
# (one derivation per unit, `nix/rust/units.nix`), and what is left of it is
# the profile: with thin LTO, every library unit stops at bitcode and every
# program's link runs the whole program's optimisation again (rustc has no
# incremental thin LTO), so one line anywhere below the shell relinks it for
# minutes. Whether that buys run time is a measurement, not a belief, and this
# file is what measures it.
#
# # The variants
#
# A to E, and `chosen`, are data in `nix/rust/profile-variants.nix` (A thin
# LTO and one codegen unit per crate, the profile until 2026-10-12; B and C
# without LTO; D sixteen units everywhere; E the shell alone in sixteen).
# Each is the tree's own manifest with the variant WRITTEN into it — another
# `lto` and `codegen-units` in `[profile.release]`, and any
# `[profile.release.package.<crate>]` it adds —, given to the two cargo runs
# that read a profile: the PLANNER (whose plan gives every unit its profile
# and its LTO, `nix/rust/planner.nix`) and the release LAYER (which compiles
# the third-party crates with it and drops what the manifest says of our own
# packages, `nix/rust/layers.nix`). Edited at evaluation, never through the
# environment: the skeleton, the plan and the layer's workspace are then byte
# for byte what the manifest makes once the winner is written into it, so a
# variant's plan IS the plan `nix run .#rust-plan` will commit and its layer
# and units ARE the derivations that ship. Writing the winner rebuilds
# nothing, and what ships is what was measured, path for path. (The chosen
# variant is the tree itself: `rust.planner`, `rust.layers.release`,
# `rust.release`.) Since the build's units are built on third-party units of
# their own and no longer on the layer (phase 2 of nix-design §1.4), a
# variant's units are still the layer's: the plan is the same, the programs
# are not the same paths (see `units` below).
#
# `loop` names its own `lto` and `codegen-units`, so its graph does not move.
# The index of the layer refuses a layer whose libraries were built with
# another LTO than the variant's plan says, and `nix run .#profile-bench`
# checks the plan's profiles and every codegen-unit count the layer's cargo
# passed: a variant that did not reach one of the two is a failed run, never
# a variant measured as another.
#
# # Why the plan is read from a build here, and nowhere else
#
# A variant's plan is cargo's answer to that manifest, and the units need it
# at evaluation to exist. Reading a derivation's output during evaluation is
# import-from-derivation, which the rest of the flake never does
# (`nix/compositor-binds.nix` says why). It is confined to what this file
# returns (`rust.profileVariants`, and `legacyPackages.<system>.profileBench`
# over it): nothing on the evaluation path of a check, a package, a system or
# an app names them — `nix run .#profile-bench` builds them by name at run
# time (`nix/profile-bench.nix`), so `nix flake check` never waits for a
# planner. The table they are made from is plain data
# (`nix/rust/profile-variants.nix`), and that is what a check reads.
# The variants are not committed plans: they would be four more two-megabyte
# files to keep current for a measurement taken once per decision.
#
# # What each variant yields
#
#   plan     the planner run over the variant's manifest (`plan.json`)
#   layer    its release layer (`layer.raw` is the cargo build)
#   units    its release units (`libs`, `scripts`, `bins`)
#   bundle   every release program, the shell wrapped as the system wraps it
#            and the graphics libraries on the rpath, as crane's `shell` was
#            (what `.#bench` measured until the units replaced crane)
#   cost     every release unit with its kind, crate, dependencies and
#            measured `metrics` — what the edit costs and the cold build are
#            computed from; building it IS the variant's cold build
{
  lib,
  pkgs,
  # `nix/rust/planner.nix` with arguments replaced (a variant's manifest).
  plannerWith,
  # The layers and the unit builder with arguments replaced (`rust.layersWith`,
  # `rust.unitsWith`).
  layersWith,
  unitsWith,
  # The root manifest, parsed (`layerArgs.rootManifest`): what each variant
  # is written into.
  rootManifest,
  # How the system wraps the shell and the painters (`nix/rust.nix`).
  shellWrapFlags,
  wrapPainters,
  # What `wgpu` dlopens, put on every program's rpath as crane did.
  graphicsLibraryPath,
}:

let
  # THE VARIANTS, by the letters nix-design §3 gives them, and E: data.
  table = import ./profile-variants.nix;
  inherit (table) variants;

  # The manifest as it reads with a variant written into it. ABSOLUTE: the
  # variant's `lto`, `codegen-units` and package table replace the tree's, so
  # A is the old profile whatever `[profile.release]` says today (and the tree
  # keeps no package table of its own: `release-profile-is-the-measured-one`
  # holds it to the chosen variant's).
  manifestOf =
    v:
    let
      release = rootManifest.profile.release;
      packages = v.packages or { };
    in
    rootManifest
    // {
      profile = rootManifest.profile // {
        release =
          removeAttrs release [ "package" ]
          // {
            lto = if v.lto == "false" then false else v.lto;
            codegen-units = v.codegenUnits;
          }
          // lib.optionalAttrs (packages != { }) { package = packages; };
      };
    };

  variant =
    letter: v:
    let
      tag = lib.toLower letter;
      manifest = manifestOf v;
      planner = plannerWith { rootManifest = manifest; };
      planDrv = planner.plan;
      # IMPORT FROM DERIVATION, and the only one: see the header.
      plan = builtins.fromJSON (builtins.readFile "${planDrv}/plan.json");
      layers = layersWith {
        inherit plan;
        rootManifest = manifest;
      };
      # ON THE VARIANT'S LAYER, as measured: `nix run .#profile-bench` times
      # the layer and reads its cargo log for the codegen units, and the
      # units over it are what that time adds to. The build's own units are
      # built on third-party units instead (phase 2 of nix-design §1.4, the
      # same commands, `third-party-units-are-cargo-s`); measuring a variant
      # that way is the day phase 1 goes.
      units =
        (unitsWith {
          inherit plan layers;
          inherit (planner) inputsHash;
          depsFrom = "layers";
        }).units
          "release";
      programs = lib.attrValues units.bins;
      everyUnit = lib.attrValues units.libs ++ lib.attrValues units.scripts ++ programs;
      isUnit = d: (d.passthru.kind or null) != null;
      row = u: {
        inherit (u) name;
        inherit (u.passthru) kind crate;
        metrics = "${u.metrics}";
        deps = map (d: d.name) (lib.filter isUnit u.passthru.unitDeps);
      };
    in
    {
      inherit (v) lto codegenUnits;
      packages = v.packages or { };
      inherit units;
      plan = planDrv;
      layer = layers.release;
      bundle =
        pkgs.runCommand "ronoctua-profile-${tag}-bundle"
          {
            nativeBuildInputs = [
              pkgs.makeBinaryWrapper
              pkgs.patchelf
            ];
          }
          ''
            mkdir -p $out/bin
            ${lib.concatMapStrings (p: "cp ${p}/bin/* $out/bin/\n") programs}
            chmod u+w $out/bin/*
            for b in $out/bin/*; do
              patchelf --add-rpath ${graphicsLibraryPath} "$b"
            done
            wrapProgram $out/bin/ronoctua-shell --inherit-argv0 ${shellWrapFlags}
            ${wrapPainters}
          '';
      cost = pkgs.writeText "ronoctua-profile-${tag}-units.json" (
        builtins.toJSON {
          variant = letter;
          inherit (v) lto codegenUnits;
          packages = v.packages or { };
          plan = "${planDrv}/plan.json";
          layer = "${layers.release.raw}";
          units = map row everyUnit;
        }
      );
    };
in
{
  variants = lib.mapAttrs variant variants;
  # The letters in the order the decision reads them; A is the reference.
  order = lib.attrNames variants;
  # The variant `[profile.release]` is (`nix/rust/profile-variants.nix`).
  inherit (table) chosen;
}
