# THE PLANNER — "cargo plans, Nix builds" (nix-design §1.2).
#
# Cargo already knows the whole build: which features each crate is unified
# with workspace-wide, which renamed or optional dependency becomes which
# `--extern`, which `[[bin]]` its `required-features` let through, which
# profile and which LTO each unit gets. Asking it once, and committing the
# answer as `nix/rust/plan.json`, is what lets every Rust compilation unit be a
# derivation of its own without a second resolver written here (crate2nix's
# per-root feature sets, buildRustCrate's own `DEP_*` naming — the rejected
# options, nix-design §1.1).
#
# `--unit-graph` is unstable, so this derivation, AND ONLY THIS ONE, runs cargo
# with `RUSTC_BOOTSTRAP=1` (owner decision 1, 2026-10-12). It compiles nothing:
# what it prints is a description, and nothing shipped is built with an
# unstable feature.
#
# # The skeleton
#
# Cargo needs the manifests, the lockfile and a file at every target root (it
# discovers `src/bin/*.rs` and `tests/*.rs` by looking). The skeleton is exactly
# that: every manifest CLEANED of what cannot change the graph (comments,
# descriptions, `[lints]`, `[package.metadata]`), the lock, and an empty stub
# at each root, found here at evaluation time. So the plan changes when a
# dependency, a feature, a profile or the set of targets does, and never when
# a line of Rust or a crate's `[package.metadata.ronoctua]` does. (The
# third-party layers are NOT built over it: they are built from the plan's
# third-party half, `nix/rust/layers.nix`, so a test file added here changes
# the plan and no layer.) (crane's own dummy
# stubs only DECLARED tests, mkDummySrc.nix:311-320, and drops
# `required-features`, which decides what exists: it is not used here.)
#
# # Freshness
#
# `inputsHash` is the hash of exactly what the skeleton is made of. The plan
# records the hash it was made from, and `nix/rust/units.nix` refuses to build
# a single unit from a plan whose hash is not the tree's: a stale plan is an
# evaluation error naming `nix run .#rust-plan`, never a unit compiled with
# yesterday's features. `rust-plan-is-current` additionally regenerates the
# whole plan and compares it byte for byte (a toolchain bump changes it with
# no manifest moving).
{
  lib,
  pkgs,
  craneLib,
  python3,
  # The repository, for its manifests and lock.
  src,
  # The features every cargo command of the build turns on
  # (`[workspace.metadata.ronoctua] build-features`).
  buildFeatures,
  # The root manifest, parsed. Another one only for the release profile's
  # variants (`nix/rust/profiles.nix`): the tree's manifest with a variant
  # written into it.
  rootManifest ? fromTOML (builtins.readFile (src + "/Cargo.toml")),
}:

let
  members = rootManifest.workspace.members;

  # WHAT THE GRAPH CANNOT SEE. Everything else of a manifest is kept, the
  # target tables and `required-features` included.
  dropPackage = [
    "authors"
    "badges"
    "categories"
    "description"
    "documentation"
    "exclude"
    "homepage"
    "include"
    "keywords"
    "metadata"
    "publish"
    "readme"
    "repository"
  ];
  clean =
    manifest:
    let
      withoutLints = removeAttrs manifest [
        "lints"
        "badges"
      ];
      package = lib.optionalAttrs (manifest ? package) {
        package = removeAttrs manifest.package dropPackage;
      };
      workspace = lib.optionalAttrs (manifest ? workspace) {
        workspace = removeAttrs manifest.workspace [
          "lints"
          "metadata"
        ];
      };
    in
    withoutLints // package // workspace;

  # EVERY TARGET ROOT CARGO WOULD FIND, the way it finds them: the inferred
  # places (`src/lib.rs`, `src/main.rs`, `src/bin/*.rs`, `src/bin/*/main.rs`,
  # `tests/`, `benches/`, `examples/` alike, `build.rs`) and every declared
  # `path`.
  entries = dir: if builtins.pathExists dir then builtins.readDir dir else { };
  inferredIn =
    base: dir:
    let
      found = entries (dir + "/${base}");
    in
    lib.concatLists (
      lib.mapAttrsToList (
        name: type:
        if type == "regular" && lib.hasSuffix ".rs" name then
          [ "${base}/${name}" ]
        else if type == "directory" && builtins.pathExists (dir + "/${base}/${name}/main.rs") then
          [ "${base}/${name}/main.rs" ]
        else
          [ ]
      ) found
    );
  declaredPaths =
    manifest:
    let
      paths = section: map (t: t.path) (lib.filter (t: t ? path) (manifest.${section} or [ ]));
      build = manifest.package.build or null;
    in
    lib.optional (manifest ? lib && manifest.lib ? path) manifest.lib.path
    ++ paths "bin"
    ++ paths "test"
    ++ paths "bench"
    ++ paths "example"
    ++ lib.optional (builtins.isString build) build;
  rootsOf =
    member:
    let
      dir = src + "/${member}";
      manifest = fromTOML (builtins.readFile (dir + "/Cargo.toml"));
      present = p: builtins.pathExists (dir + "/${p}");
      inferred =
        lib.filter present [
          "src/lib.rs"
          "src/main.rs"
          "build.rs"
        ]
        ++ inferredIn "src/bin" dir
        ++ inferredIn "tests" dir
        ++ inferredIn "benches" dir
        ++ inferredIn "examples" dir;
    in
    lib.sort lib.lessThan (lib.unique (inferred ++ lib.filter present (declaredPaths manifest)));

  memberManifests = lib.genAttrs members (
    member: clean (fromTOML (builtins.readFile (src + "/${member}/Cargo.toml")))
  );
  memberRoots = lib.genAttrs members rootsOf;
  # The roots that are a LIBRARY: their stub is empty, every other root's is a
  # `main` (a program needs one for cargo to take the stub as a program).
  libRootsOf =
    member:
    let
      manifest = memberManifests.${member};
    in
    [ "src/lib.rs" ] ++ lib.optional (manifest ? lib && manifest.lib ? path) manifest.lib.path;

  # WHAT THE SKELETON IS, as one value — and its hash, which the plan carries.
  description = {
    root = clean rootManifest;
    members = lib.mapAttrs (member: manifest: {
      inherit manifest;
      roots = memberRoots.${member};
    }) memberManifests;
    lock = builtins.hashFile "sha256" (src + "/Cargo.lock");
    features = buildFeatures;
  };
  inputsHash = builtins.hashString "sha256" (builtins.toJSON description);

  manifestsJson = builtins.toJSON (
    {
      "Cargo.toml" = description.root;
    }
    // lib.mapAttrs' (
      member: m: lib.nameValuePair "${member}/Cargo.toml" m.manifest
    ) description.members
  );
  stubsJson = builtins.toJSON (
    lib.concatLists (
      lib.mapAttrsToList (
        member: m:
        map (root: {
          path = "${member}/${root}";
          text = if lib.elem root (libRootsOf member) then "" else "fn main() {}\n";
        }) m.roots
      ) description.members
    )
  );

  skeleton =
    pkgs.runCommand "ronoctua-rust-skeleton"
      {
        nativeBuildInputs = [
          craneLib.craneUtils
          python3
        ];
        inherit manifestsJson stubsJson;
        passAsFile = [
          "manifestsJson"
          "stubsJson"
        ];
      }
      ''
        mkdir -p $out
        cp ${src + "/Cargo.lock"} $out/Cargo.lock
        python3 - "$manifestsJsonPath" "$stubsJsonPath" "$out" <<'PY'
        import json, os, subprocess, sys
        manifests, stubs, out = sys.argv[1:]
        for path, manifest in json.load(open(manifests)).items():
            dest = os.path.join(out, path)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            toml = subprocess.run(["crane-json2toml"], input=json.dumps(manifest).encode(),
                                  check=True, capture_output=True).stdout
            open(dest, "wb").write(toml)
        for stub in json.load(open(stubs)):
            dest = os.path.join(out, stub["path"])
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            open(dest, "w").write(stub["text"])
        PY
      '';

  # The vendored third-party sources, from the lockfile alone — the same
  # derivation crane vendors with for the workspace.
  vendor = craneLib.vendorCargoDeps { cargoLock = src + "/Cargo.lock"; };

  features = lib.concatStringsSep "," buildFeatures;

  plan = craneLib.mkCargoDerivation {
    pname = "ronoctua-rust-plan";
    version = "0";
    src = skeleton;
    cargoArtifacts = null;
    cargoVendorDir = vendor;
    doInstallCargoArtifacts = false;
    nativeBuildInputs = [ python3 ];
    buildPhaseCargoCommand = ''
      # THE ONE PLACE `RUSTC_BOOTSTRAP` IS SET. `--unit-graph` prints the
      # plan and compiles nothing.
      export RUSTC_BOOTSTRAP=1
      common=(--workspace --locked --offline --features ${lib.escapeShellArg features} --unit-graph -Z unstable-options)
      cargo build "''${common[@]}" --profile release > release.json
      cargo build "''${common[@]}" --profile loop > loop.json
      cargo test --no-run "''${common[@]}" --profile loop > test.json
      cargo check --all-targets "''${common[@]}" --profile loop > check.json
      unset RUSTC_BOOTSTRAP
      cargo metadata --format-version 1 --locked --offline > metadata.json
      rustc -vV > rustc.txt
      python3 ${../scripts/rust-plan.py} plan \
        --metadata metadata.json --manifest Cargo.toml \
        --graph release=release.json --graph loop=loop.json \
        --graph test=test.json --graph check=check.json \
        --features ${lib.escapeShellArg features} \
        --rustc rustc.txt --inputs ${inputsHash} > plan.json
    '';
    installPhaseCommand = ''
      mkdir -p $out
      cp plan.json $out/plan.json
    '';
  };
in
{
  inherit
    skeleton
    vendor
    plan
    inputsHash
    features
    ;
}
