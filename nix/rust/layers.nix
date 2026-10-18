# THE THIRD-PARTY LAYERS — phase 1 of nix-design §1.4.
#
# The ~480 crates of the lockfile are compiled by cargo itself, once per
# profile, and nothing of ours reaches these derivations — no `RONOCTUA_*`, no
# source of ours, no buffer —, which is what `deps-layer-is-lean` holds.
#
# # A workspace of their own, made of the plan's third-party half
#
# Every unit of ours reads a layer, so a layer must change when a third-party
# unit does and at no other time. Built over our own workspace (the planner's
# skeleton), it changed with every test file added, every path dependency
# between two of our crates and every feature of ours — and every unit of all
# three flavours with it, though cargo compiled the same five hundred crates
# the same way. So the layer is built from a workspace made at evaluation
# from the plan's THIRD-PARTY units alone:
#
#   deps-target     the one member: a library depending on every
#                   third-party library of the graph compiled for the target,
#                   each pinned to its version with exactly the features the
#                   plan gives it;
#   deps-host       a proc-macro depending the same way on every one compiled
#                   for the host (what build scripts and proc-macros link,
#                   with `build-override`). It is a DEPENDENCY of
#                   `deps-target`, never a member: cargo resolves the features
#                   of a proc-macro's dependencies apart from the target's
#                   only when the proc-macro is depended on — a member is a
#                   root, and a root's dependencies are resolved as the
#                   target's (the spike that built this saw `syn` and `semver`
#                   take the host's features on the target side);
#   deps-host-test  (loop) the same for the test graph's host side, reached
#                   only through `deps-target`'s dev-dependencies, so only
#                   `cargo test` compiles it — as only `cargo test` widens the
#                   features the dev-dependencies widen.
#
# The lockfile it starts from is the third-party half of ours, and the profiles
# are ours without the overrides of our own packages. Requesting each crate
# with the features the plan already unified changes no unification: the
# result is the plan's, crate for crate — and if cargo ever disagrees, the
# index refuses the layer, naming each unit (below).
#
# A layer keeps what a unit links and nothing else, UNCOMPRESSED (a unit reads
# a few rlibs out of it; crane's zstd'd `target/` would be unpacked whole by
# every one of eight hundred derivations): `deps/` holds the third-party
# rlibs and proc-macros, `native/` the build-script output directories their
# `-L native=` and their `links` metadata point into (`rust-layer.py`).
#
# # The index, and spike S1 on every build
#
# `index.json`, in a small derivation beside the layer, names each unit by the
# plan's key (`rust-index.py`). Its inputs are the layer and the same
# third-party half: the plan's units of ours never reach it, so a unit of ours
# that changes rebuilds itself and what is above it, never the index every
# other unit reads. Cargo's own messages are matched against the plan, unit
# for unit, and each library's LTO against the plan's: a layer whose units are
# not the plan's — a stale plan, a cargo that resolved differently — FAILS,
# instead of handing the units a crate built another way.
#
# `release` runs `cargo build` only (crane's layer also ran `check
# --all-targets` and `test --no-run`, ~4m43 nothing read). `loop` adds
# `test --no-run`: the test graph shares the loop profile, and a crate whose
# features the dev-dependencies widen is a second key beside the first.
{
  lib,
  pkgs,
  craneLib,
  python3,
  # The committed plan, parsed (`nix/rust/plan.json`).
  plan,
  # The lockfile and the root manifest, parsed.
  cargoLock,
  rootManifest,
  # The vendored third-party sources (`planner.vendor`).
  vendor,
  # What the third-party crates themselves link against and how their C is
  # built (`buildBase` in `nix/rust.nix`): the same for the layers as for
  # crane's.
  nativeBuildInputs,
  buildInputs,
  env,
}:

let
  ours = lib.attrNames plan.packages;

  # --- the plan's third-party half -------------------------------------------
  thirdParty = graph: lib.filterAttrs (_: u: !u.workspace) plan.graphs.${graph}.units;

  # WHICH SIDE EACH THIRD-PARTY LIBRARY IS COMPILED FOR. The host's roots are
  # the build scripts and the proc-macros; the target's, our own units that
  # are neither. A library under a host root is the host's; one our units
  # reach without passing a host root is the target's.
  sidesOf =
    graph:
    let
      units = plan.graphs.${graph}.units;
      hostRoot = u: lib.elem "proc-macro" u.kind || (lib.elem "custom-build" u.kind && u.mode == "build");
      closure =
        start: next:
        map (x: x.key) (
          builtins.genericClosure {
            startSet = map (key: { inherit key; }) start;
            operator = item: map (key: { inherit key; }) (next item.key);
          }
        );
      host = closure (lib.attrNames (lib.filterAttrs (_: hostRoot) units)) (
        k: map (d: d.key) units.${k}.deps
      );
      target = closure (lib.attrNames (
        lib.filterAttrs (_: u: u.workspace && !(hostRoot u) && u.mode != "run-custom-build") units
      )) (k: map (d: d.key) (lib.filter (d: !(hostRoot units.${d.key})) units.${k}.deps));
      # What a dependency section can name: a library, not a build script.
      library =
        k:
        let
          u = units.${k};
        in
        !u.workspace && u.mode == "build" && !(lib.elem "custom-build" u.kind);
      requests =
        side: keys:
        let
          wanted = map (k: { inherit (units.${k}) pkg version features; }) (lib.filter library keys);
        in
        lib.mapAttrsToList (
          id: rs:
          if lib.length (lib.unique (map (r: r.features) rs)) > 1 then
            throw "the ${graph} graph compiles ${id} for the ${side} with two feature sets, which one workspace cannot ask for"
          else
            lib.head rs
        ) (lib.groupBy (r: "${r.pkg}@${r.version}") wanted);
    in
    {
      host = requests "host" host;
      target = requests "target" target;
    };

  # A dependency table asking for each library exactly as the plan has it.
  table =
    requests:
    lib.listToAttrs (
      map (
        r:
        lib.nameValuePair "t-${r.pkg}-${lib.replaceStrings [ "." "+" ] [ "_" "_" ] r.version}" {
          package = r.pkg;
          # Build metadata (`+…`) is no part of a requirement; the lockfile
          # holds the one version that has it.
          version = "=${lib.head (lib.splitString "+" r.version)}";
          default-features = false;
          inherit (r) features;
        }
      ) requests
    );

  # Our profiles, without what they say about our own packages.
  profiles = lib.mapAttrs (
    _: p:
    let
      packages = removeAttrs (p.package or { }) ours;
    in
    removeAttrs p [ "package" ] // lib.optionalAttrs (packages != { }) { package = packages; }
  ) (rootManifest.profile or { });

  # The lockfile without our packages: cargo adds the layer's own, and keeps
  # every third-party version as it is.
  lock = {
    inherit (cargoLock) version;
    package = lib.filter (p: p ? source) cargoLock.package;
  };

  package = name: {
    inherit name;
    version = "0.0.0";
    edition = "2021";
    publish = false;
  };

  # THE WORKSPACE A LAYER IS BUILT FROM: `build` names the graph `cargo build`
  # compiles, `test` (the loop layer's) the graph `cargo test --no-run` does.
  workspaceOf =
    {
      flavour,
      build,
      test ? null,
    }:
    let
      b = sidesOf build;
      t = sidesOf test;
      manifests = {
        "Cargo.toml" = {
          workspace = {
            resolver = rootManifest.workspace.resolver or "2";
            members = [ "deps-target" ];
            exclude = [ "deps-host" ] ++ lib.optional (test != null) "deps-host-test";
          };
          profile = profiles;
        };
        "deps-target/Cargo.toml" = {
          package = package "ronoctua-layer-target";
          lib = {
            path = "lib.rs";
            doctest = false;
          };
          dependencies = table b.target // {
            ronoctua-layer-host.path = "../deps-host";
          };
        }
        // lib.optionalAttrs (test != null) {
          dev-dependencies = table t.target // {
            ronoctua-layer-host-test.path = "../deps-host-test";
          };
        };
        "deps-host/Cargo.toml" = {
          package = package "ronoctua-layer-host";
          lib = {
            path = "lib.rs";
            proc-macro = true;
            test = false;
            doctest = false;
          };
          dependencies = table b.host;
        };
      }
      // lib.optionalAttrs (test != null) {
        "deps-host-test/Cargo.toml" = {
          package = package "ronoctua-layer-host-test";
          lib = {
            path = "lib.rs";
            proc-macro = true;
            test = false;
            doctest = false;
          };
          dependencies = table t.host;
        };
      };
    in
    pkgs.runCommand "ronoctua-rust-deps-${flavour}-workspace"
      {
        nativeBuildInputs = [
          craneLib.craneUtils
          python3
        ];
        manifestsJson = builtins.toJSON manifests;
        lockJson = builtins.toJSON lock;
        passAsFile = [
          "manifestsJson"
          "lockJson"
        ];
      }
      ''
        mkdir -p $out
        python3 - "$manifestsJsonPath" "$lockJsonPath" "$out" <<'PY'
        import json, os, subprocess, sys
        manifests, lock, out = sys.argv[1:]
        def toml(value):
            return subprocess.run(["crane-json2toml"], input=json.dumps(value).encode(),
                                  check=True, capture_output=True).stdout
        for path, manifest in json.load(open(manifests)).items():
            dest = os.path.join(out, path)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            open(dest, "wb").write(toml(manifest))
            if path != "Cargo.toml":
                open(os.path.join(os.path.dirname(dest), "lib.rs"), "w").close()
        open(os.path.join(out, "Cargo.lock"), "wb").write(toml(json.load(open(lock))))
        PY
      '';

  layer =
    {
      flavour,
      profile,
      workspace,
      commands,
    }:
    craneLib.mkCargoDerivation (
      env
      // {
        pname = "ronoctua-rust-deps-${flavour}";
        version = "0";
        src = workspace;
        cargoArtifacts = null;
        cargoVendorDir = vendor;
        doInstallCargoArtifacts = false;
        strictDeps = true;
        nativeBuildInputs = nativeBuildInputs ++ [ python3 ];
        inherit buildInputs;
        # `-v` because the per-unit `-L native=` paths and LTO flags are
        # printed nowhere else; the index reads them out of the log. No
        # `--locked`: cargo adds the layer's own packages to the lockfile,
        # and the index refuses any third-party version that moved.
        buildPhaseCargoCommand = lib.concatMapStrings (c: ''
          cargo ${c.command} -v --workspace --offline \
            --profile ${profile} --message-format json-render-diagnostics \
            > ${c.graph}.json 2> ${c.graph}.log || {
            tail -n 300 ${c.graph}.log >&2
            exit 1
          }
        '') commands;
        installPhaseCommand = ''
          python3 ${../scripts/rust-layer.py} --target target --out $out \
            ${lib.concatMapStringsSep " " (c: "--run ${c.graph}=${c.graph}.json,${c.graph}.log") commands}
        '';
        # The rlibs are archives of LLVM bitcode or objects that the units
        # link; nothing here is a program to strip or patch.
        dontFixup = true;
        passthru = { inherit flavour workspace; };
      }
    );

  # THE INDEX, apart from the build it reads: naming five hundred units by the
  # plan's keys is seconds of work. Its plan is the third-party half of the
  # graphs the layer ran, cut here: nothing of ours is in it.
  indexed =
    flavour: graphs: raw:
    let
      units = lib.foldl' (acc: g: acc // thirdParty g) { } graphs;
      present = lib.unique (lib.mapAttrsToList (_: u: "${u.pkg}@${u.version}") units);
      half = builtins.toFile "ronoctua-rust-deps-${flavour}-plan.json" (
        builtins.toJSON {
          inherit (plan) format;
          links = lib.getAttrs (lib.filter (id: plan.links ? ${id}) present) plan.links;
          graphs = lib.genAttrs graphs (g: {
            units = thirdParty g;
          });
        }
      );
    in
    pkgs.runCommand "ronoctua-rust-deps-${flavour}-index"
      {
        nativeBuildInputs = [ python3 ];
        passthru = {
          inherit flavour raw;
          plan = half;
        };
      }
      ''
        python3 ${../scripts/rust-index.py} --third-party ${half} --raw ${raw} --out $out
      '';
in
{
  release = indexed "release" [ "release" ] (layer {
    flavour = "release";
    profile = "release";
    workspace = workspaceOf {
      flavour = "release";
      build = "release";
    };
    commands = [
      {
        command = "build";
        graph = "release";
      }
    ];
  });
  loop =
    indexed "loop"
      [
        "loop"
        "test"
      ]
      (layer {
        flavour = "loop";
        profile = "loop";
        workspace = workspaceOf {
          flavour = "loop";
          build = "loop";
          test = "test";
        };
        commands = [
          {
            command = "build";
            graph = "loop";
          }
          {
            command = "test --no-run";
            graph = "test";
          }
        ];
      });
}
