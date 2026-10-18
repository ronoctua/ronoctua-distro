# THIRD-PARTY CRATES, UNIT BY UNIT — phase 2 of nix-design §1.4 (lane N6).
#
# Every third-party unit of the plan is a derivation of its own, driven like
# ours (`nix/rust/units.nix`, `depUnits`; `nix/scripts/rust-unit.py`, mode
# `dep`). These checks hold the three things that make that safe: a
# third-party unit sees nothing of ours, an edit rebuilds only what is above
# it, and a unit is compiled as cargo compiles it.
#
# Joined into `tests/checks.nix` with one line.
{
  pkgs,
  lib,
  rust,
  mkCheck,
}:

let
  plan = builtins.fromJSON (builtins.readFile ../nix/rust/plan.json);
  graphUnits = flavour: plan.graphs.${flavour}.units;
  allUnits = graphUnits "release" // graphUnits "loop" // graphUnits "test";
  isOurs = key: (allUnits.${key} or { workspace = false; }).workspace;
  isLib = u: u.mode == "build" && !(lib.elem "custom-build" u.kind);

  fail =
    what: problems:
    if problems == [ ] then
      ""
    else
      ''
        echo "ERROR: ${what}:" >&2
        ${lib.concatMapStringsSep "\n" (p: "echo ${lib.escapeShellArg "  ${p}"} >&2") problems}
        exit 1
      '';

  # --- the edits the granularity check makes, at evaluation -----------------
  # One unit of one graph, changed as `f` says; every other byte of the plan
  # is the committed one (its `inputs` too, so the builder takes it).
  editUnit =
    flavour: key: f:
    plan
    // {
      graphs = plan.graphs // {
        ${flavour} = plan.graphs.${flavour} // {
          units = plan.graphs.${flavour}.units // {
            ${key} = f plan.graphs.${flavour}.units.${key};
          };
        };
      };
    };
  probe = u: u // { features = u.features ++ [ "n6-probe" ]; };
  # EVERY DERIVATION ABOVE `key`, by the plan's own edges — with a build
  # script's compilation and run inside the derivation that runs them (a
  # library's, or a crate of ours with none: its script unit), as the units
  # are made. So a crate whose build script reads a `links` package's
  # metadata sits above that package's LIBRARY unit, which ran the script.
  # `derivations` are the keys a derivation carries.
  above =
    flavour: derivations: key:
    let
      units = graphUnits flavour;
      runOf = k: lib.findFirst (d: units.${d.key}.mode == "run-custom-build") null units.${k}.deps;
      compilesOf =
        r:
        map (d: d.key) (
          lib.filter (
            d: lib.elem "custom-build" units.${d.key}.kind && units.${d.key}.mode == "build"
          ) units.${r}.deps
        );
      # A library runs its package's build script; a crate of ours with no
      # library has a unit for the script alone (its key is the run's); a
      # program reads what the script wrote, and runs nothing.
      ownOf =
        k:
        let
          u = units.${k};
          r =
            if u.mode == "run-custom-build" then
              { key = k; }
            else if isLib u && !(lib.elem "bin" u.kind) then
              runOf k
            else
              null;
        in
        lib.unique ([ k ] ++ lib.optionals (r != null) ([ r.key ] ++ compilesOf r.key));
      own = lib.genAttrs derivations ownOf;
      owner = lib.listToAttrs (lib.concatMap (d: map (k: lib.nameValuePair k d) own.${d}) derivations);
      depsOf =
        d:
        lib.unique (
          map (k: owner.${k} or k) (
            lib.subtractLists own.${d} (lib.concatMap (k: map (x: x.key) units.${k}.deps) own.${d})
          )
        );
      edges = lib.concatMap (d: map (dep: { inherit d dep; }) (depsOf d)) derivations;
      users = lib.mapAttrs (_: es: map (e: e.d) es) (lib.groupBy (e: e.dep) edges);
    in
    map (x: x.key) (
      builtins.genericClosure {
        startSet = [ { key = owner.${key} or key; } ];
        operator = x: map (k: { key = k; }) (users.${x.key} or [ ]);
      }
    );
  # A flavour's derivations that carry a unit's key: third-party units, our
  # libraries, build scripts and programs.
  keyed =
    u:
    lib.listToAttrs (
      map (d: lib.nameValuePair d.unitKey d.drvPath) (
        lib.attrValues u.deps ++ lib.attrValues u.libs ++ lib.attrValues u.scripts ++ lib.attrValues u.bins
      )
    );
  changedBy =
    edited:
    let
      before = keyed rust.release;
      after = keyed ((rust.unitsWith { plan = edited; }).units "release");
    in
    {
      changed = lib.filter (k: before.${k} != (after.${k} or null)) (lib.attrNames before);
      keys = lib.attrNames before;
    };
  granularity =
    what: key:
    let
      r = changedBy (editUnit "release" key probe);
      expected = lib.filter (k: lib.elem k r.keys) (above "release" r.keys key);
      missing = lib.subtractLists r.changed expected;
      extra = lib.subtractLists expected r.changed;
    in
    {
      inherit (r) changed;
      problems =
        map (k: "${what}: ${k} sits above it and was not rebuilt") missing
        ++ map (k: "${what}: ${k} does not sit above it and was rebuilt") extra
        # A probe that moves nothing proves nothing.
        ++ lib.optional (!(lib.elem key r.changed)) "${what}: the edited unit itself did not change";
    };

  # A third-party library in the middle of the graph (the target's
  # `zeroize`): below some forty third-party units and eighty of ours, above
  # one.
  theirs =
    lib.findFirst
      (
        k:
        let
          u = (graphUnits "release").${k};
        in
        u.pkg == "zeroize" && u.opt == "3" && !u.workspace && isLib u
      )
      (throw "the release graph has no target library of `zeroize`")
      (lib.attrNames (graphUnits "release"));
  # A library of ours with crates of ours above it.
  ours = lib.findFirst (
    k:
    let
      u = (graphUnits "release").${k};
    in
    u.pkg == "ronoctua-xdg" && isLib u
  ) (throw "the release graph has no library of ronoctua-xdg") (lib.attrNames (graphUnits "release"));
  theirsEdit = granularity "a third-party library" theirs;
  oursEdit = granularity "a library of ours" ours;

  # --- cargo, on the spike's crates -----------------------------------------
  # The crates nix-design §1.4 names for the spike: a `links` C build with
  # `DEP_*` to its dependent (aws-lc-sys, aws-lc-rs), a system library found
  # by pkg-config (libsqlite3-sys, wayland-sys), the libc/rustix backends,
  # a proc-macro and its host chain (proc-macro2, serde_derive), and the
  # largest generated code there is (cranelift-codegen, wasmtime).
  spike = [
    "aws-lc-sys"
    "aws-lc-rs"
    "libsqlite3-sys"
    "wayland-sys"
    "rustix"
    "libc"
    "proc-macro2"
    "serde_derive"
    "cranelift-codegen"
    "wasmtime"
  ];
  spikeOf =
    flavour:
    let
      units = graphUnits flavour;
      start = lib.filter (k: !units.${k}.workspace && isLib units.${k} && lib.elem units.${k}.pkg spike) (
        lib.attrNames units
      );
      closure = map (x: x.key) (
        builtins.genericClosure {
          startSet = map (key: { inherit key; }) start;
          operator = x: map (d: { key = d.key; }) units.${x.key}.deps;
        }
      );
      # WHICH SIDE A LIBRARY IS COMPILED FOR (as `nix/rust/layers.nix`
      # finds it): under a build script or a proc-macro, the host's;
      # reached from our units without passing one, the target's.
      hostRoot = u: lib.elem "proc-macro" u.kind || (lib.elem "custom-build" u.kind && u.mode == "build");
      reach =
        startKeys: next:
        map (x: x.key) (
          builtins.genericClosure {
            startSet = map (key: { inherit key; }) startKeys;
            operator = x: map (key: { inherit key; }) (next x.key);
          }
        );
      host = reach (lib.attrNames (lib.filterAttrs (_: hostRoot) units)) (
        k: map (d: d.key) units.${k}.deps
      );
      target = reach (lib.attrNames (
        lib.filterAttrs (_: u: u.workspace && !(hostRoot u) && u.mode != "run-custom-build") units
      )) (k: map (d: d.key) (lib.filter (d: !(hostRoot units.${d.key})) units.${k}.deps));
      libs = lib.filter (k: isLib units.${k}) closure;
      request = k: { inherit (units.${k}) pkg version features; };
    in
    {
      inherit libs;
      host = map request (lib.filter (k: lib.elem k host) libs);
      target = map request (lib.filter (k: lib.elem k target) libs);
    };
  release = spikeOf "release";
  # A dependency table asking for each library exactly as the plan has it
  # (one feature set per side: two would be a workspace no manifest says).
  table =
    requests:
    lib.mapAttrs' (
      id: rs:
      let
        r = lib.head rs;
      in
      if lib.length (lib.unique (map (x: x.features) rs)) > 1 then
        throw "the spike asks for ${id} with two feature sets on one side"
      else
        lib.nameValuePair "t-${r.pkg}-${lib.replaceStrings [ "." "+" ] [ "_" "_" ] r.version}" {
          package = r.pkg;
          # Build metadata (`+…`) is no part of a requirement.
          version = "=${lib.head (lib.splitString "+" r.version)}";
          default-features = false;
          inherit (r) features;
        }
    ) (lib.groupBy (r: "${r.pkg}@${r.version}") requests);
  rootManifest = fromTOML (builtins.readFile ../Cargo.toml);
  cargoLock = fromTOML (builtins.readFile ../Cargo.lock);
  member = name: {
    inherit name;
    version = "0.0.0";
    edition = "2021";
    publish = false;
  };
  manifests = {
    "Cargo.toml" = {
      workspace = {
        resolver = rootManifest.workspace.resolver or "2";
        members = [ "deps-target" ];
        exclude = [ "deps-host" ];
      };
      # Our profiles, without what they say about our own packages.
      profile = lib.mapAttrs (
        _: p:
        let
          packages = removeAttrs (p.package or { }) (lib.attrNames plan.packages);
        in
        removeAttrs p [ "package" ] // lib.optionalAttrs (packages != { }) { package = packages; }
      ) (rootManifest.profile or { });
    };
    "deps-target/Cargo.toml" = {
      package = member "n6-spike-target";
      lib = {
        path = "lib.rs";
        doctest = false;
      };
      dependencies = table release.target // {
        n6-spike-host.path = "../deps-host";
      };
    };
    "deps-host/Cargo.toml" = {
      package = member "n6-spike-host";
      lib = {
        path = "lib.rs";
        proc-macro = true;
        test = false;
        doctest = false;
      };
      dependencies = table release.host;
    };
  };
  workspace =
    pkgs.runCommand "n6-spike-workspace"
      {
        nativeBuildInputs = [
          rust.craneLib.craneUtils
          pkgs.python3
        ];
        manifestsJson = builtins.toJSON manifests;
        lockJson = builtins.toJSON {
          inherit (cargoLock) version;
          package = lib.filter (p: p ? source) cargoLock.package;
        };
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
  # What the comparison reads of each unit: its key's plan entry and its
  # output (the commands it ran, what its build script said).
  unitsJson = pkgs.writeText "n6-spike-units.json" (
    builtins.toJSON (
      lib.listToAttrs (
        map (
          k:
          lib.nameValuePair k {
            entry = (graphUnits "release").${k};
            out = "${rust.release.deps.${k}}";
          }
        ) release.libs
      )
    )
  );
in
{
  # A THIRD-PARTY UNIT SEES NOTHING OF OURS.
  #
  # What `deps-layer-is-lean` holds for the layers, held for every
  # third-party unit of the three flavours: no `RONOCTUA_*` in its
  # environment, no name of ours declared, a crates.io package as its only
  # source, and no unit of ours among the units it compiles against. A value
  # of ours that reached one would rebuild it, and everything above it, for
  # an edit no third-party crate reads. The same reading finds the names on a
  # unit of ours, or it would ask nothing.
  third-party-units-see-nothing-of-ours =
    let
      units = rust.release.deps // rust.loop.deps // rust.test.deps;
      problems = lib.concatLists (
        lib.mapAttrsToList (
          key: d:
          let
            u = d.unit;
            leaked = lib.filter (n: lib.hasPrefix "RONOCTUA_" n) (lib.attrNames (d.env or { }));
            mine = lib.filter isOurs (
              lib.attrNames u.workspace ++ lib.attrNames u.units ++ lib.attrNames u.runs
            );
          in
          map (n: "${key}: its environment has ${n}") leaked
          ++ lib.optional (u.declared != [ ] || u.env != { }) "${key}: it declares names of ours"
          ++ lib.optional (
            !(lib.hasPrefix "cargo-package-" d.src.name)
          ) "${key}: it compiles ${d.src.name}, which is no crates.io package"
          ++ lib.optional (u.layer != null) "${key}: it reads a layer"
          ++ map (k: "${key}: it compiles against ${k}, a unit of ours") mine
        ) units
      );
      control = rust.release.libs.ronoctua-tokens.unit;
    in
    mkCheck "third-party-units-see-nothing-of-ours" [ pkgs.coreutils ] ''
      # PARSED: every third-party unit's own derivation attributes, at evaluation.
      ${fail "a third-party unit sees something of ours" problems}
      ${lib.optionalString (lib.length (lib.attrNames units) < 600) ''
        echo "ERROR: only ${toString (lib.length (lib.attrNames units))} third-party units: this check reads nothing" >&2
        exit 1
      ''}
      ${lib.optionalString (control.env == { }) ''
        echo "ERROR: ronoctua-tokens declares no value either: this reading would find none" >&2
        exit 1
      ''}
      echo "OK: ${toString (lib.length (lib.attrNames units))} third-party units see nothing of ours"
    '';

  # AN EDIT REBUILDS WHAT IS ABOVE IT, AND NOTHING ELSE.
  #
  # The plan edited at evaluation, one unit at a time, and every release
  # derivation that carries a key (third-party units, our libraries, build
  # scripts, programs) compared before and after: the changed ones must be
  # EXACTLY the edited unit and the units above it by the plan's own edges.
  # A third-party library in the middle (`zeroize`) rebuilds its dependents and
  # not one unrelated crate — a lock bump of one crate is that crate and what
  # depends on it, not the five hundred; a library of ours (`ronoctua-xdg`)
  # rebuilds no third-party unit at all.
  a-third-party-change-rebuilds-only-what-is-above =
    let
      theirsIn = lib.filter (k: !(isOurs k)) theirsEdit.changed;
      oursIn = lib.filter (k: !(isOurs k)) oursEdit.changed;
    in
    mkCheck "a-third-party-change-rebuilds-only-what-is-above" [ pkgs.coreutils ] ''
      # PARSED: the units' derivations under the committed plan and an edited one.
      ${fail "an edit rebuilt something else than what is above it" (
        theirsEdit.problems ++ oursEdit.problems
      )}
      ${fail "a library of ours rebuilt third-party units" oursIn}
      ${lib.optionalString (lib.length theirsIn < 10 || lib.length theirsEdit.changed < 50) ''
        echo "ERROR: the third-party edit moved ${toString (lib.length theirsEdit.changed)} units: it sits below too little to prove anything" >&2
        exit 1
      ''}
      echo "OK: a third-party library rebuilds the ${toString (lib.length theirsEdit.changed)} units above it (${toString (lib.length theirsIn)} of them third-party), a library of ours the ${toString (lib.length oursEdit.changed)} above it and no third-party unit"
    '';

  # A THIRD-PARTY UNIT IS COMPILED AS CARGO COMPILES IT.
  #
  # The spike's crates and everything below them (≈120 libraries of the
  # release graph), built by cargo itself in a workspace made of their plan
  # entries, and set beside the units: each library's and each build
  # script's compiler command (flag for flag, paths and hashes aside), and
  # what each build script said (cfgs, libraries, search paths, `rustc-env`,
  # `links` metadata). The driver plays the part of cargo that runs; this is
  # where a drift from cargo — a toolchain bump that changes what cargo
  # passes — shows, instead of in a program that behaves differently.
  third-party-units-are-cargo-s = rust.craneLib.mkCargoDerivation (
    rust.depBase.env
    // {
      pname = "check-third-party-units-are-cargo-s";
      version = "0";
      src = workspace;
      cargoArtifacts = null;
      cargoVendorDir = rust.planner.vendor;
      doInstallCargoArtifacts = false;
      strictDeps = true;
      nativeBuildInputs = rust.depBase.nativeBuildInputs ++ [ pkgs.python3 ];
      inherit (rust.depBase) buildInputs;
      buildPhaseCargoCommand = ''
        cargo build -v --workspace --offline --profile release \
          --message-format json-render-diagnostics > cargo.json 2> cargo.log || {
          tail -n 200 cargo.log >&2
          exit 1
        }
      '';
      installPhaseCommand = ''
        python3 ${./third-party-units.py} --units ${unitsJson} --messages cargo.json \
          --log cargo.log --target target
        touch $out
      '';
      dontFixup = true;
    }
  );
}
