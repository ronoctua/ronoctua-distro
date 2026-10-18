# ONE DERIVATION PER RUST COMPILATION UNIT — nix-design §1.3–§1.7.
#
# `units flavour` reads the plan's graph for that flavour and gives every unit
# a derivation of its own, driven by `nix/scripts/rust-unit.py`:
#
#   deps.<key>         a third-party library or proc-macro (and its build
#                      script, run first), by the plan's key — one
#                      derivation for every graph that names the key
#   libs.<crate>       the library (and its build script, run first)
#   scripts.<crate>    the build script of a crate with no library
#   bins.<bin>         every `[[bin]]` (release and loop)
#   testBins.<crate>   the crate's test harnesses, and the programs its
#                      integration tests run (`CARGO_BIN_EXE_*`)    — test
#   testRuns.<crate>   those harnesses run, from the crate's sources  — test
#   clippy.<crate>     clippy over every target of the crate          — test
#   fmt.<crate>        rustfmt over every target root of the crate
#
# The flavours are three graphs: `release` (what ships), `loop` (what `.#dev`
# runs) and `test` (the loop profile, with the dev-dependencies' features
# unified in — `ronoctua-consent/test-support` among them, which must never
# be in a product, so the test graph is never the loop one).
#
# # What a unit sees
#
# Its SOURCES are its target's files only (§1.5): a library sees its
# `Cargo.toml`, `build.rs` and `src/` without `src/bin/**` and `src/main.rs`;
# a program in `src/bin/x.rs` sees that file and `src/bin/x/`; a test build
# adds `tests/` and `fixtures/`. So editing a test or a program's root
# recompiles that, and not the library below it.
#
# Its ENVIRONMENT is what its crate declares in `[package.metadata.ronoctua]`
# and nothing else: `env` (values from `generated`), `test-env` (the tests'
# build only, values from `testEnv`), `env-absent` (read on purpose with a
# default; no value), `run-env` (what its tests read while running, given to
# the test run of this crate and of every crate above it). Every crate used
# to see all eighty variables, so a new path for `btop` rebuilt the file
# manager; and the driver refuses a read nobody declared (dep-info's
# `env-dep:` and the build script's `rerun-if-env-changed`), so the
# declaration cannot fall behind the code.
#
# # Freshness
#
# The plan carries the hash of the skeleton it was made from; a plan whose
# hash is not the tree's is refused here, at evaluation, with the command that
# regenerates it — never a unit built from yesterday's dependencies.
{
  lib,
  stdenv,
  pkgs,
  python3,
  rustToolchain,
  src,
  plan,
  inputsHash,
  layers,
  vendor,
  # The tables a unit's declared names are looked up in.
  generated,
  # Names whose value is each crate's own, `{ <name> = crate: value; }` — the
  # text keys' directory, which holds one crate's file (`nix/text-keys.nix`).
  # A crate that declares the name gets its own value in place of
  # `generated`'s.
  perCrateEnv ? { },
  # Names a crate may declare in `env` that `generated` holds only sometimes
  # (the person's brain modules' folder).
  optionalNames,
  testEnv,
  # The names the wrapper sets at run time (`atRunTime`), which a crate's
  # tests are never given: what `run-env-absent` may name.
  atRunTimeNames,
  # What every compilation links against (`buildBase` in `nix/rust.nix`).
  nativeBuildInputs,
  buildInputs,
  # What a test run's `LD_LIBRARY_PATH` holds (`Ui::for_tests` dlopens
  # libwayland).
  testLibraryPath,
  # AN EDIT, SIMULATED (nix-design §6): `{ <crate> = { "<path in the
  # crate>" = <file>; }; }`. Every unit whose sources hold that file gets the
  # replacement instead, and no other unit changes — what a real edit of that
  # file does, at evaluation, with nothing built. `granularity-of-an-edit`
  # and `nix run .#rebuild-cost -- <file>` ask this.
  crateSourceOverrides ? { },
  # WHERE THE THIRD-PARTY CRATES COME FROM: `"units"`, one derivation per
  # third-party unit of the plan driven like ours (phase 2 of nix-design
  # §1.4, the build's way since its spike passed), or `"layers"`, the two
  # layers cargo builds (phase 1, kept only until the lanes that still read
  # the layers have moved — then it goes, with `nix/rust/layers.nix`).
  depsFrom ? "units",
  # Phase 2's inputs: each third-party package's source by `name@version`
  # (crane's per-crate download, the same derivation the vendoring links),
  # and what a third-party unit that RUNS a build script is given — what the
  # layers are given (`buildBase`), so a `-sys` crate finds its system
  # library the same way.
  depSources ? { },
  depBase ? {
    nativeBuildInputs = [ ];
    buildInputs = [ ];
    env = { };
  },
  # THE LIBRARIES A CRATE `dlopen`s, by crate name (`dlopened` in
  # `nix/rust.nix`): a release or loop program whose closure holds one of
  # these crates gets their directories in its RUNPATH, and no other does.
  dlopened ? { },
}:

let
  fs = lib.fileset;
  driver = ../scripts/rust-unit.py;

  fresh =
    plan.inputs == inputsHash
    || throw (
      "nix/rust/plan.json is stale: it was made from a skeleton hashed ${plan.inputs}, and the "
      + "tree's manifests, lockfile, targets or build features hash to ${inputsHash}. "
      + "Regenerate it with `nix run .#rust-plan`."
    );

  # What a crate's tests are given while they run: exactly what crane's
  # `tests` left in their environment.
  runTable = generated // testEnv;

  rootManifest = fromTOML (builtins.readFile (src + "/Cargo.toml"));
  workspacePackage = rootManifest.workspace.package or { };
  workspaceLints = rootManifest.workspace.lints or { };

  # --- the crates, as their manifests say -------------------------------------
  crateOf =
    name:
    let
      p = plan.packages.${name};
      dir = p.dir;
      root = src + "/${dir}";
      manifest = fromTOML (builtins.readFile (root + "/Cargo.toml"));
      package = manifest.package;
      field =
        f:
        let
          v = package.${f} or null;
        in
        if builtins.isAttrs v && (v.workspace or false) then workspacePackage.${f} or null else v;
      text = v: if v == null then "" else toString v;
      readme =
        let
          v = field "readme";
          found = lib.findFirst (r: builtins.pathExists (root + "/${r}")) null [
            "README.md"
            "README.txt"
            "README"
          ];
        in
        if v == false then
          ""
        else if builtins.isString v then
          v
        else
          text found;
      version = p.version;
      parts = lib.splitString "." (lib.head (lib.splitString "-" version));
      pre = lib.concatStringsSep "-" (lib.drop 1 (lib.splitString "-" version));
      meta = package.metadata.ronoctua or { };
      lints = if (manifest.lints.workspace or false) then workspaceLints else manifest.lints or { };
      levels = {
        forbid = "--forbid";
        deny = "--deny";
        warn = "--warn";
        allow = "--allow";
      };
      lintFlags =
        let
          all = lib.concatLists (
            lib.mapAttrsToList (
              tool: table:
              lib.mapAttrsToList (
                lint: cfg:
                let
                  level = if builtins.isString cfg then cfg else cfg.level;
                  priority = if builtins.isAttrs cfg then cfg.priority or 0 else 0;
                in
                {
                  inherit priority;
                  flag = "${levels.${level}}=${if tool == "rust" then lint else "${tool}::${lint}"}";
                  extra = lib.optionals (tool == "rust" && lint == "unexpected_cfgs" && builtins.isAttrs cfg) (
                    lib.concatMap (c: [
                      "--check-cfg"
                      c
                    ]) (cfg.check-cfg or [ ])
                  );
                }
              ) table
            ) lints
          );
          sorted = lib.sort (
            a: b: a.priority < b.priority || (a.priority == b.priority && a.flag < b.flag)
          ) all;
        in
        lib.concatMap (l: [ l.flag ] ++ l.extra) sorted;
      known = lib.attrNames generated ++ optionalNames;
      declared = {
        env = meta.env or [ ];
        test = meta.test-env or [ ];
        absent = meta.env-absent or [ ];
        run = meta.run-env or [ ];
        runAbsent = meta.run-env-absent or [ ];
      };
      checked =
        let
          bad =
            lib.filter (n: !(lib.elem n known)) declared.env
            ++ lib.filter (n: !(lib.elem n known || testEnv ? ${n})) declared.test;
          given = lib.filter (n: lib.elem n known) declared.absent;
          # WHAT THE TESTS READ WHILE RUNNING is given (`run-env`, a value
          # from `generated` or `testEnv`) or declared not given
          # (`run-env-absent`, a name the wrapper sets at run time and the
          # tests have always run without). A name in neither table — a typo
          # — is refused, never dropped in silence.
          runNotGiven = lib.filter (n: !(runTable ? ${n})) declared.run;
          runGiven = lib.filter (n: runTable ? ${n}) declared.runAbsent;
          runUnknown = lib.filter (n: !(lib.elem n atRunTimeNames)) (
            lib.filter (n: !(runTable ? ${n})) declared.runAbsent
          );
          unknownKeys = lib.subtractLists [
            "env"
            "test-env"
            "env-absent"
            "run-env"
            "run-env-absent"
            "beside"
            "data"
            "bins"
          ] (lib.attrNames meta);
          # A role is one of four, and names one of the crate's programs.
          roles = [
            "ship"
            "driver"
            "bench"
            "apart"
          ];
          badRoles = lib.filter (b: !(lib.elem (meta.bins.${b}) roles)) (lib.attrNames (meta.bins or { }));
          strayRoles = lib.subtractLists (binsOf name) (lib.attrNames (meta.bins or { }));
        in
        if unknownKeys != [ ] then
          throw "${dir}/Cargo.toml: [package.metadata.ronoctua] has no key ${lib.concatStringsSep ", " unknownKeys}"
        else if badRoles != [ ] then
          throw "${dir}/Cargo.toml gives ${lib.concatStringsSep ", " badRoles} a role that is not one of ${lib.concatStringsSep ", " roles}"
        else if strayRoles != [ ] then
          throw "${dir}/Cargo.toml gives a role to ${lib.concatStringsSep ", " strayRoles}, which is not one of its programs"
        else if bad != [ ] then
          throw "${dir}/Cargo.toml declares ${lib.concatStringsSep ", " bad}, which no table of nix/rust.nix has"
        else if given != [ ] then
          throw "${dir}/Cargo.toml declares ${lib.concatStringsSep ", " given} absent, and generated gives a value"
        else if runNotGiven != [ ] then
          throw "${dir}/Cargo.toml declares ${lib.concatStringsSep ", " runNotGiven} in run-env, which neither generated nor testEnv gives its tests: declare it in run-env-absent if they run without it on purpose"
        else if runGiven != [ ] then
          throw "${dir}/Cargo.toml declares ${lib.concatStringsSep ", " runGiven} in run-env-absent, and its tests are given a value (generated or testEnv): declare it in run-env"
        else if runUnknown != [ ] then
          throw "${dir}/Cargo.toml declares ${lib.concatStringsSep ", " runUnknown} in run-env-absent, which no table of nix/rust.nix has"
        else
          true;
    in
    assert checked;
    {
      inherit
        name
        dir
        root
        manifest
        meta
        declared
        ;
      links = p.links;
      features = p.features;
      lints = lintFlags;
      data = meta.data or [ ];
      beside = meta.beside or [ ];
      roles = meta.bins or { };
      pkgEnv = {
        CARGO_PKG_NAME = name;
        CARGO_PKG_VERSION = version;
        CARGO_PKG_VERSION_MAJOR = lib.elemAt parts 0;
        CARGO_PKG_VERSION_MINOR = lib.elemAt parts 1;
        CARGO_PKG_VERSION_PATCH = lib.elemAt parts 2;
        CARGO_PKG_VERSION_PRE = pre;
        CARGO_PKG_AUTHORS = lib.concatStringsSep ":" (
          if field "authors" == null then [ ] else field "authors"
        );
        CARGO_PKG_DESCRIPTION = text (field "description");
        CARGO_PKG_HOMEPAGE = text (field "homepage");
        CARGO_PKG_REPOSITORY = text (field "repository");
        CARGO_PKG_LICENSE = text (field "license");
        CARGO_PKG_LICENSE_FILE = text (field "license-file");
        CARGO_PKG_RUST_VERSION = text (field "rust-version");
        CARGO_PKG_README = readme;
      };
      # Which `[[test]]` and `[[bin]]` are not libtest harnesses.
      harness = lib.listToAttrs (
        map (t: lib.nameValuePair t.name (t.harness or true)) (
          lib.filter (t: t ? name) (
            (manifest.test or [ ]) ++ (manifest.bin or [ ]) ++ (manifest.bench or [ ])
          )
        )
      );
    };
  crates = lib.mapAttrs (name: _: crateOf name) plan.packages;

  # --- the files each target sees ----------------------------------------------
  at = crate: rel: crate.root + "/${rel}";
  maybe = crate: rel: fs.maybeMissing (at crate rel);
  # A `data` ENTRY is a path in the crate (a file, or a directory and all of
  # it), where a `*` component stands for every directory there, and a
  # trailing `**/*.<ext>` for every file with that extension under the
  # directory before it. So a crate names what it compiles and not the
  # folder around it: the brain's library compiles each module's manifest
  # and shared half (`modules/*/shared.rs`), and an edit of a module's code
  # or of its prose must not rebuild it, nor the footer above it. An entry
  # that names no FILE is refused: asked of the files the entry ends up
  # holding, not of the directories its pattern walked — a `**/*.rs` over a
  # folder whose Rust moved away, or a `**/*.rss`, walks a folder that
  # exists and names nothing, and the declaration would go on saying the
  # crate reads something.
  dataOf =
    crate:
    let
      expand =
        base: parts:
        if parts == [ ] then
          [ base ]
        else if lib.head parts == "*" then
          lib.concatMap (name: expand (base + "/${name}") (lib.tail parts)) (
            lib.attrNames (
              lib.filterAttrs (_: t: t == "directory") (
                if builtins.pathExists base then builtins.readDir base else { }
              )
            )
          )
        else
          expand (base + "/${lib.head parts}") (lib.tail parts);
      entry =
        d:
        let
          parts = lib.splitString "/" d;
          n = lib.length parts;
          byExt = n >= 2 && lib.elemAt parts (n - 2) == "**" && lib.hasPrefix "*." (lib.last parts);
          ext = lib.removePrefix "*." (lib.last parts);
          found = lib.filter builtins.pathExists (
            expand crate.root (lib.take (if byExt then n - 2 else n) parts)
          );
          set = fs.unions (if byExt then map (fs.fileFilter (f: f.hasExt ext)) found else found);
        in
        if found == [ ] || fs.toList set == [ ] then
          throw "${crate.dir}/Cargo.toml lists data `${d}`, which names no file"
        else
          set;
    in
    fs.unions (map entry crate.data);
  libSet =
    crate:
    fs.unions [
      (at crate "Cargo.toml")
      (maybe crate "build.rs")
      (fs.difference
        (fs.fileFilter (f: f.hasExt "rs" || f.hasExt "wit" || f.hasExt "wgsl") (at crate "src"))
        (
          fs.unions [
            (maybe crate "src/bin")
            (maybe crate "src/main.rs")
          ]
        )
      )
      (dataOf crate)
    ];
  scriptSet =
    crate:
    fs.unions [
      (at crate "Cargo.toml")
      (at crate "build.rs")
      (dataOf crate)
    ];
  binSet =
    crate: rootFile:
    fs.unions (
      [
        (at crate "Cargo.toml")
        (dataOf crate)
      ]
      ++ (
        if rootFile == "src/main.rs" then
          [ (at crate "src") ]
        else
          [
            (at crate rootFile)
            (maybe crate (lib.removeSuffix ".rs" rootFile))
          ]
          ++ lib.optional (baseNameOf rootFile == "main.rs") (at crate (dirOf rootFile))
      )
    );
  testSet =
    crate: roots:
    fs.unions (
      [
        (libSet crate)
        (maybe crate "tests")
        (maybe crate "fixtures")
        (maybe crate "benches")
        (maybe crate "examples")
      ]
      ++ map (binSet crate) roots
    );
  fmtSet =
    crate:
    fs.unions [
      (at crate "Cargo.toml")
      (fs.fileFilter (f: f.hasExt "rs") crate.root)
    ];
  # THE EDITS, by path in the repository: a crate's `data` may name another
  # crate's files (the brain daemon compiles the library's module folders),
  # so a file is matched against every unit's fileset, never only its own
  # crate's.
  edits = lib.concatLists (
    lib.mapAttrsToList (
      crate: files:
      let
        dir =
          (plan.packages.${crate}
            or (throw "crateSourceOverrides names ${crate}, which is not a crate of the plan")
          ).dir;
      in
      lib.mapAttrsToList (rel: file: {
        rel = "${dir}/${rel}";
        path =
          let
            p = src + "/${dir}/${rel}";
          in
          if builtins.pathExists p then
            p
          else
            throw "crateSourceOverrides edits ${dir}/${rel}, which does not exist";
        inherit file;
      }) files
    ) crateSourceOverrides
  );
  holds = fileset: e: fs.toList (fs.intersection fileset (fs.maybeMissing e.path)) != [ ];
  sourceOf =
    fileset:
    let
      base = fs.toSource {
        root = src;
        inherit fileset;
      };
      mine = lib.filter (holds fileset) edits;
    in
    if mine == [ ] then
      base
    else
      # The same tree with the edited files in it, under the same name: the
      # driver compiles and runs a crate's tests from `/build/source`.
      pkgs.runCommand "source" { } (
        ''
          cp -r --no-preserve=mode ${base} $out
        ''
        + lib.concatMapStrings (e: ''
          cp ${e.file} $out/${e.rel}
        '') mine
      );

  # --- the graphs ----------------------------------------------------------------
  graphOf = flavour: plan.graphs.${flavour};

  indexOf =
    flavour:
    let
      units = (graphOf flavour).units;
      keys = lib.attrNames units;
      ours = lib.filter (k: units.${k}.workspace) keys;
      of = pred: lib.filter (k: pred units.${k}) ours;
      byPkg = keys': lib.groupBy (k: units.${k}.pkg) keys';
      one =
        what: groups: name:
        let
          found = groups.${name} or [ ];
        in
        if lib.length found > 1 then
          throw "${name} has ${toString (lib.length found)} ${what} units in the ${flavour} graph"
        else
          lib.head (found ++ [ null ]);
      libGroups = byPkg (
        of (u: u.mode == "build" && (lib.elem "lib" u.kind || lib.elem "proc-macro" u.kind))
      );
      runGroups = byPkg (of (u: u.mode == "run-custom-build"));
    in
    {
      inherit units;
      libKey = one "library" libGroups;
      runKey = one "build-script" runGroups;
      bins = of (u: u.mode == "build" && lib.elem "bin" u.kind);
      tests = byPkg (of (u: u.mode == "test" || u.mode == "bench"));
      roots = lib.mapAttrs (_: ks: map (k: units.${k}.src) ks) (
        byPkg (of (u: u.mode != "run-custom-build" && !(lib.elem "custom-build" u.kind)))
      );
    };

  # Each graph indexed once, whoever asks.
  indexes = lib.genAttrs [
    "release"
    "loop"
    "test"
  ] indexOf;
  index = flavour: indexes.${flavour};

  layerOf = flavour: if flavour == "release" then layers.release else layers.loop;

  # Every program of a crate, by name — what `CARGO_BIN_EXE_*` names for its
  # integration tests.
  binsOf =
    let
      release = index "release";
    in
    name:
    map (k: release.units.${k}.target) (lib.filter (k: release.units.${k}.pkg == name) release.bins);

  storeName =
    flavour: kind: crate:
    {
      release = "rust-${kind}-${crate.name}-${plan.packages.${crate.name}.version}";
      loop = "rust-loop-${kind}-${crate.name}-${plan.packages.${crate.name}.version}";
      test = "rust-test-${kind}-${crate.name}-${plan.packages.${crate.name}.version}";
    }
    .${flavour};

  # THE ENVIRONMENT A COMPILATION OF `crate` IS GIVEN, and the names it may
  # read: `env` in every flavour, `test-env` in the test graph's, `env-absent`
  # declared with no value.
  compileEnv =
    flavour: crate:
    let
      table = generated // lib.optionalAttrs (flavour == "test") testEnv;
      names = crate.declared.env ++ lib.optionals (flavour == "test") crate.declared.test;
    in
    {
      env =
        lib.getAttrs (lib.filter (n: table ? ${n}) names) table
        // lib.mapAttrs (_: valueOf: valueOf crate.name) (
          lib.getAttrs (lib.filter (n: perCrateEnv ? ${n}) names) perCrateEnv
        );
      declared = names ++ crate.declared.absent;
    };

  common = {
    inherit (plan) rustc host;
    cargo = "${rustToolchain}/bin/cargo";
  };

  # THE PROFILES A UNIT COMPILES WITH, and no other: the plan's whole table
  # in every unit would make a profile that only a new unit elsewhere uses
  # rebuild them all.
  profilesOf =
    spec:
    lib.getAttrs (lib.unique (
      lib.filter (p: p != null) (
        map (u: u.profile or null) (lib.attrValues (spec.units or { }))
        ++ [
          (spec.profile or null)
          (spec.scriptProfile or null)
        ]
      )
    )) plan.profiles;

  mkUnit =
    {
      name,
      mode,
      fileset,
      spec,
      lib' ? false,
      # A PROGRAM is what a closure carries — the system's, the installer
      # seed's, the person's binary cache —, so a store that already holds it
      # hands it over and nothing below it is built (Nix's default). Every
      # other unit is only ever the input of another one on the same
      # machine: nothing is fetched for it, nor asked of a substituter.
      product ? false,
      passthru ? { },
      extra ? { },
    }:
    assert fresh;
    stdenv.mkDerivation (
      {
        inherit name;
        src = sourceOf fileset;
        __structuredAttrs = true;
        unit = common // { profiles = profilesOf spec; } // spec // { inherit mode; };
        outputs = [
          "out"
          "metrics"
        ];
        nativeBuildInputs = nativeBuildInputs ++ [
          python3
          rustToolchain
        ];
        inherit buildInputs;
        strictDeps = true;
        dontConfigure = true;
        buildPhase = ''
          runHook preBuild
          python3 ${driver} ${mode}
          runHook postBuild
        '';
        dontInstall = true;
        allowSubstitutes = product;
        dontStrip = lib';
        inherit passthru;
      }
      // extra
    );

  # --- third-party crates, unit by unit (phase 2, nix-design §1.4) ------------
  #
  # Every third-party unit of the three graphs, by its key. The key is a
  # Merkle name (features, profile, the keys of every dependency), so one key
  # is one compilation whichever graph names it: the loop and the test graph
  # share every third-party unit, and each is ONE derivation.
  thirdParty = depsFrom == "units";
  depEntries =
    let
      of = flavour: lib.filterAttrs (_: u: !u.workspace) (graphOf flavour).units;
      merged = lib.foldl' (acc: flavour: acc // of flavour) { } [
        "release"
        "loop"
        "test"
      ];
      clashes =
        lib.concatMap
          (flavour: lib.filter (k: merged.${k} != (of flavour).${k}) (lib.attrNames (of flavour)))
          [
            "release"
            "loop"
            "test"
          ];
    in
    if clashes != [ ] then
      throw "nix/rust/plan.json describes ${lib.head clashes} differently in two graphs, which one derivation cannot be"
    else
      merged;
  releaseDeps = lib.filterAttrs (_: u: !u.workspace) (graphOf "release").units;
  # A unit a dependent links: a library or a proc-macro. A build script's
  # compilation and its run are not units of their own: they run inside the
  # library's derivation, before it, as they do for our crates.
  isDepLib = u: u.mode == "build" && !(lib.elem "custom-build" u.kind);
  depLibKeys = lib.filter (k: isDepLib depEntries.${k}) (lib.attrNames depEntries);
  depLibDeps = key: lib.filter (d: isDepLib depEntries.${d.key}) depEntries.${key}.deps;
  depRunOf =
    key:
    let
      runs = lib.filter (d: depEntries.${d.key}.mode == "run-custom-build") depEntries.${key}.deps;
    in
    if runs == [ ] then
      null
    else if lib.length runs > 1 then
      throw "${key} has ${toString (lib.length runs)} build script runs"
    else
      (lib.head runs).key;
  depCompileOf =
    run:
    (lib.findFirst (
      d: lib.elem "custom-build" depEntries.${d.key}.kind && depEntries.${d.key}.mode == "build"
    ) (throw "${run}: a build script run with no compilation") depEntries.${run}.deps).key;
  # The runs of the `links` packages below a run: what cargo hands its script
  # as `DEP_<links>_*`.
  depLinksRuns =
    run:
    map (d: d.key) (
      lib.filter (d: depEntries.${d.key}.mode == "run-custom-build") depEntries.${run}.deps
    );
  # WHICH LIBRARY UNIT RUNS A BUILD SCRIPT: its package's library, the one
  # unit whose dependencies name the run (the plan has no run two libraries
  # share; a second one would run it again, to the same result).
  depRunOwner = lib.listToAttrs (
    lib.concatMap (
      k:
      let
        r = depRunOf k;
      in
      lib.optional (r != null) (lib.nameValuePair r k)
    ) depLibKeys
  );
  linksOf = key: plan.links."${depEntries.${key}.pkg}@${depEntries.${key}.version}";
  # What `-L dependency=` must find below a third-party unit: every library
  # under it, by key — an attribute set, so a library reached by two paths is
  # one entry.
  depClosure = lib.genAttrs depLibKeys (key: depBelowAll (depLibDeps key));
  depBelowAll = deps: lib.foldl' (acc: d: acc // depBelow d.key) { } deps;
  depBelow = key: depClosure.${key} // { ${key} = depUnits.${key}; };

  depUnits = lib.genAttrs depLibKeys (
    key:
    let
      u = depEntries.${key};
      id = "${u.pkg}@${u.version}";
      run = depRunOf key;
      compile = if run == null then null else depCompileOf run;
      # The libraries the library links and those its build script links
      # (built for the host): what `--extern` names.
      direct = depLibDeps key ++ lib.optionals (compile != null) (depLibDeps compile);
      linksRuns = if run == null then [ ] else depLinksRuns run;
      own = [
        key
      ]
      ++ lib.optionals (run != null) [
        run
        compile
      ];
      dir = "${u.pkg}-${u.version}";
    in
    assert fresh;
    stdenv.mkDerivation {
      # THE NAME SAYS THE PROFILE: a release unit is `rust-dep-…`, the loop
      # and the test graph's (one profile) `rust-loop-dep-…` — the mark
      # `the-loop-profile-is-never-installed` looks for.
      name = "${if releaseDeps ? ${key} then "rust-dep" else "rust-loop-dep"}-${u.pkg}-${u.version}";
      src = depSources.${id} or (throw "Cargo.lock has no registry package ${id} for the plan's ${key}");
      __structuredAttrs = true;
      unit = common // {
        mode = "dep";
        profiles = profilesOf { units = lib.getAttrs own depEntries; };
        inherit key;
        runKey = run;
        crate = {
          name = u.pkg;
          inherit (u) version;
          links = plan.links.${id} or null;
        };
        units = lib.getAttrs own depEntries;
        workspace = lib.listToAttrs (map (d: lib.nameValuePair d.key "${depUnits.${d.key}}") direct);
        runs = lib.listToAttrs (
          map (
            r:
            lib.nameValuePair r {
              path = "${depUnits.${depRunOwner.${r}}}";
              links = linksOf r;
            }
          ) linksRuns
        );
        farm = map toString (lib.attrValues (depBelowAll direct));
        layer = null;
        env = { };
        declared = [ ];
      };
      outputs = [
        "out"
        "metrics"
      ];
      # A library with no build script needs the compiler and nothing else
      # (a proc-macro links with stdenv's own `cc`); one with a script is
      # given what the layers are given, so a `-sys` crate's script finds its
      # system library — and a new SQLite rebuilds those, not every crate.
      nativeBuildInputs = [
        python3
        rustToolchain
      ]
      ++ lib.optionals (run != null) depBase.nativeBuildInputs;
      buildInputs = lib.optionals (run != null) depBase.buildInputs;
      env = lib.optionalAttrs (run != null) depBase.env;
      strictDeps = true;
      # The source in the build directory under `<name>-<version>`, as cargo
      # unpacks a registry package: what `file!()`, a panic and
      # `CARGO_MANIFEST_DIR` name — never a store path a program would carry.
      # READ-ONLY, as the vendored copy the layers compile is: a build script
      # that probes the compiler from its own directory (rustix 0.38's
      # `can_compile`) must answer what it answers there.
      unpackPhase = ''
        runHook preUnpack
        cp -r --reflink=auto -- "$src" "$NIX_BUILD_TOP/${dir}"
        chmod -R a-w -- "$NIX_BUILD_TOP/${dir}"
        cd -- "$NIX_BUILD_TOP/${dir}"
        runHook postUnpack
      '';
      dontConfigure = true;
      buildPhase = ''
        runHook preBuild
        python3 ${driver} dep
        runHook postBuild
      '';
      dontInstall = true;
      # Archives a dependent links, and a proc-macro the compiler loads:
      # nothing to strip, patch or rewrite.
      dontFixup = true;
      allowSubstitutes = false;
      passthru = {
        kind = "dep";
        crate = u.pkg;
        unitKey = key;
        unitDeps = lib.unique (
          map (d: depUnits.${d.key}) direct ++ map (r: depUnits.${depRunOwner.${r}}) linksRuns
        );
      };
    }
  );

  units =
    flavour:
    let
      g = index flavour;
      layer = if thirdParty then null else layerOf flavour;
      layers' = lib.optional (layer != null) layer;
      byKey = g.units;

      # The derivation behind a workspace unit's key.
      libDrvOf =
        key:
        let
          u = byKey.${key};
        in
        if !u.workspace then
          depUnits.${key}
        else if g.libKey u.pkg == key then
          libs.${u.pkg}
        else
          throw "${key}: a workspace unit used as a dependency that is not its crate's library (a build script's or a proc-macro's) is not built by the unit builder";
      baseOf =
        crate:
        if g.libKey crate != null then
          libs.${crate}
        else if g.runKey crate != null then
          scripts.${crate}
        else
          null;
      wsDeps =
        key: lib.filter (d: (byKey.${d.key} or { workspace = false; }).workspace) byKey.${key}.deps;
      wsLibDeps =
        key:
        lib.filter (
          d:
          (lib.elem "lib" byKey.${d.key}.kind || lib.elem "proc-macro" byKey.${d.key}.kind)
          && byKey.${d.key}.mode == "build"
        ) (wsDeps key);
      # The third-party libraries a unit links, when they are units of their
      # own (phase 2); with the layers, a unit finds them in its layer.
      depLibsOf =
        key:
        if thirdParty then
          lib.filter (d: !byKey.${d.key}.workspace && isDepLib byKey.${d.key}) byKey.${key}.deps
        else
          [ ];
      # A unit's own key and its build script's compilation: what it compiles
      # against.
      scriptCompileOf =
        runKey:
        (lib.findFirst (
          d: lib.elem "custom-build" byKey.${d.key}.kind && byKey.${d.key}.mode == "build"
        ) (throw "${runKey}: a build script run with no compilation") byKey.${runKey}.deps).key;
      withScript = keys: runKey: keys ++ lib.optional (runKey != null) (scriptCompileOf runKey);
      # Every library below a unit, for `-L dependency=`: the workspace's (by
      # crate) and, unit by unit, the third-party ones (by key) — a set, so a
      # library reached twice is one entry.
      belowOfLib = lib.mapAttrs (
        crate: _:
        let
          key = g.libKey crate;
        in
        if key == null then { } else belowOf key
      ) crates;
      belowOf =
        k:
        lib.foldl' (
          acc: d: acc // belowOfLib.${byKey.${d.key}.pkg} // { "~${byKey.${d.key}.pkg}" = libDrvOf d.key; }
        ) { } (wsLibDeps k)
        // depBelowAll (depLibsOf k);
      farmOf = keys: lib.attrValues (lib.foldl' (acc: k: acc // belowOf k) { } keys);
      farmOfLib = lib.mapAttrs (_: lib.attrValues) belowOfLib;
      workspaceOf =
        keys:
        lib.listToAttrs (
          lib.concatMap (
            k: map (d: lib.nameValuePair d.key "${libDrvOf d.key}") (wsLibDeps k ++ depLibsOf k)
          ) keys
        );
      # THE `links` METADATA A BUILD SCRIPT OF OURS IS HANDED (`DEP_*`), when
      # the third-party runs are units' (phase 2): the library unit that ran
      # each one. With the layers, the layer's index has it.
      linksRunsOf =
        runKey:
        if !thirdParty || runKey == null then
          [ ]
        else
          map (d: d.key) (
            lib.filter (
              d: !byKey.${d.key}.workspace && byKey.${d.key}.mode == "run-custom-build"
            ) byKey.${runKey}.deps
          );
      runsOf =
        runKey:
        if !thirdParty || runKey == null then
          null
        else
          lib.listToAttrs (
            map (
              r:
              lib.nameValuePair r {
                path = "${depUnits.${depRunOwner.${r}}}";
                links = linksOf r;
              }
            ) (linksRunsOf runKey)
          );
      # The units those runs are in, for `unitDeps`.
      runOwnersOf = runKey: map (r: depUnits.${depRunOwner.${r}}) (linksRunsOf runKey);
      # What no program may carry: the layer, or every third-party unit it was
      # linked from and the source each was compiled from.
      buildOf =
        farm:
        layers'
        ++ lib.optional (!thirdParty) vendor
        ++ map (d: d.src) (lib.filter (d: (d.kind or null) == "dep") farm);
      unitsOf =
        keys:
        let
          withRuns = lib.unique (keys ++ lib.concatMap (k: map (d: d.key) (wsDeps k)) keys);
          withScripts = lib.unique (
            withRuns
            ++ lib.concatMap (
              k: if byKey.${k}.mode == "run-custom-build" then map (d: d.key) byKey.${k}.deps else [ ]
            ) withRuns
          );
        in
        lib.getAttrs (lib.filter (k: byKey ? ${k}) withScripts) byKey;

      # WHAT A PROGRAM LOADS WITHOUT LINKING IT: the directories of the
      # libraries the crates in its closure `dlopen` (`dlopened`), over the
      # units that end up inside the program — never a build script's or a
      # proc-macro's, which run on the host while compiling and are no part
      # of it. Each unit's answer is computed once, whoever asks.
      linkedDeps =
        key:
        lib.filter (
          d:
          let
            du = byKey.${d.key};
          in
          du.mode != "run-custom-build"
          && !(lib.elem "custom-build" du.kind)
          && !(lib.elem "proc-macro" du.kind)
        ) byKey.${key}.deps;
      opened = lib.mapAttrs (
        key: u:
        lib.unique ((dlopened.${u.pkg} or [ ]) ++ lib.concatMap (d: opened.${d.key}) (linkedDeps key))
      ) byKey;
      crateSpec = crate: {
        inherit (crate)
          name
          dir
          links
          features
          lints
          pkgEnv
          ;
        version = plan.packages.${crate.name}.version;
        hasLib = g.libKey crate.name != null;
      };

      libs = lib.mapAttrs (
        name: crate:
        let
          key = g.libKey name;
          runKey = g.runKey name;
          e = compileEnv flavour crate;
        in
        mkUnit {
          name = storeName flavour "lib" crate;
          mode = "lib";
          fileset = libSet crate;
          lib' = true;
          spec = {
            inherit key runKey flavour;
            inherit (e) env declared;
            crate = crateSpec crate;
            units = unitsOf ([ key ] ++ lib.optional (runKey != null) runKey);
            workspace = workspaceOf (withScript [ key ] runKey);
            farm = map toString (farmOf (withScript [ key ] runKey));
            layer = if layer == null then null else "${layer}";
            runs = runsOf runKey;
          };
          passthru = {
            kind = "lib";
            inherit flavour;
            crate = name;
            unitKey = key;
            unitDeps = lib.unique (
              layers'
              ++ map (d: libDrvOf d.key) (
                lib.concatMap (k: wsLibDeps k ++ depLibsOf k) (withScript [ key ] runKey)
              )
              ++ runOwnersOf runKey
            );
          };
        }
      ) (lib.filterAttrs (name: _: g.libKey name != null) crates);

      scripts = lib.mapAttrs (
        name: crate:
        let
          runKey = g.runKey name;
          e = compileEnv flavour crate;
        in
        mkUnit {
          name = storeName flavour "build" crate;
          mode = "script";
          fileset = scriptSet crate;
          lib' = true;
          spec = {
            key = runKey;
            inherit runKey flavour;
            inherit (e) env declared;
            crate = crateSpec crate;
            units = unitsOf [ runKey ];
            workspace = workspaceOf (withScript [ ] runKey);
            farm = map toString (farmOf (withScript [ ] runKey));
            layer = if layer == null then null else "${layer}";
            runs = runsOf runKey;
          };
          passthru = {
            kind = "script";
            inherit flavour;
            crate = name;
            unitKey = runKey;
            unitDeps =
              layers' ++ map (d: libDrvOf d.key) (depLibsOf (scriptCompileOf runKey)) ++ runOwnersOf runKey;
          };
        }
      ) (lib.filterAttrs (name: _: g.libKey name == null && g.runKey name != null) crates);

      bins = lib.listToAttrs (
        map (
          key:
          let
            u = byKey.${key};
            crate = crates.${u.pkg};
            e = compileEnv flavour crate;
            base = baseOf u.pkg;
            farm = farmOf [ key ];
            role = crate.roles.${u.target} or "ship";
            # The test graph's programs run only as `CARGO_BIN_EXE_*` of a
            # test run, whose `LD_LIBRARY_PATH` already holds what a test
            # opens: they are given nothing.
            graphics = if flavour == "test" then [ ] else lib.sort (a: b: a < b) opened.${key};
            # The crates whose code is inside the program: its own and every
            # workspace library below it (the farm also holds the third-party
            # units, `kind = "dep"`, which are no crate of ours).
            inside = lib.unique (
              [ u.pkg ] ++ map (d: d.passthru.crate) (lib.filter (d: (d.kind or null) == "lib") farm)
            );
            # WHAT THE PROGRAM MAY CITE: every value the crates inside it were
            # compiled with. Nix registers as a reference only a path among a
            # derivation's inputs, and a release library is LLVM bitcode
            # (`-C linker-plugin-lto`), where a string is not bytes the scanner
            # can find: the path a library bakes with `option_env!` (git's,
            # 7zz's, the askpass, `pamu2fcfg`) reaches no library unit's
            # references, and becomes bytes only here, in the linked program —
            # where, without this, it would be a path the closure does not
            # hold. (A loop library is machine code, and its unit already
            # references what it cites; the rule is the same for both.)
            cited = lib.unique (lib.concatMap (c: lib.attrValues (compileEnv flavour crates.${c}).env) inside);
          in
          lib.nameValuePair u.target (mkUnit {
            # THE NAME SAYS THE FLAVOUR: a release program is its own name, a
            # loop one says `-loop-`, and a test-graph one (linked against
            # what the dev-dependencies turn on, `test-support` among it)
            # says `rust-test-` like every other unit of that graph — the
            # marks `the-loop-profile-is-never-installed` looks for.
            name =
              {
                release = "${u.target}-${u.version}";
                loop = "${u.target}-loop-${u.version}";
                test = "rust-test-bin-${u.target}-${u.version}";
              }
              .${flavour};
            mode = "bin";
            # The test graph's programs reach no closure: its tests run the
            # copies `testBins` builds into `libexec/`.
            product = flavour != "test";
            fileset = binSet crate u.src;
            spec = {
              inherit key flavour;
              inherit (e) env declared;
              crate = crateSpec crate;
              units = unitsOf [ key ];
              workspace = workspaceOf [ key ];
              farm = map toString farm;
              layer = if layer == null then null else "${layer}";
              base = if base == null then null else "${base}";
              # The vendored sources a dependency's panic message names, made
              # the `eeee…` hash; a third-party unit compiles its copy in the
              # build directory, and names no store path.
              vendor = if thirdParty then null else "${vendor}";
            };
            extra = {
              # A PROGRAM CARRIES NONE OF ITS BUILD: not the layer, not the
              # vendored sources a dependency's panic message names, not the
              # toolchain, not one library unit (with third-party units: not
              # one of them, nor the source it was compiled from).
              disallowedReferences = [
                rustToolchain
              ]
              ++ buildOf farm
              ++ farm
              ++ lib.optional (base != null) base;
            }
            # (Only on what can reach a machine: the test graph's programs
            # keep the derivation the plan alone gives them.)
            // lib.optionalAttrs (flavour != "test") { inherit cited; }
            # THE GRAPHICS LIBRARIES ARE LOADED, NOT LINKED: `wgpu` finds the
            # Vulkan loader and EGL, `wayland-sys` libwayland, by `dlopen`,
            # and a library no `DT_NEEDED` names is not in the RUNPATH the
            # linker writes — without this `wgpu` falls back to software in
            # silence. AFTER the fixup's own `patchelf --shrink-rpath`, which
            # would take back out what no `DT_NEEDED` uses. Only on a program
            # whose closure opens them: the rest carry none of the graphics
            # stack (crane's workspace gave all four to every program, and
            # each extracted one shrank them away again).
            // lib.optionalAttrs (graphics != [ ]) {
              postFixup = ''
                patchelf --add-rpath ${lib.concatStringsSep ":" graphics} $out/bin/${u.target}
              '';
            };
            passthru = {
              kind = "bin";
              inherit flavour role graphics;
              bin = u.target;
              crate = u.pkg;
              # The crates whose code is inside the program (what `beside` is
              # asked of).
              crates = inside;
              unitKey = key;
              unitDeps = layers' ++ farm ++ lib.optional (base != null) base;
            };
          })
        ) g.bins
      );

      testBins = lib.mapAttrs (
        name: keys:
        let
          crate = crates.${name};
          e = compileEnv flavour crate;
          binKeys = lib.unique (
            lib.concatMap (
              k:
              map (d: d.key) (
                lib.filter (d: (byKey.${d.key} or { kind = [ ]; }).kind == [ "bin" ]) byKey.${k}.deps
              )
            ) keys
          );
          base = baseOf name;
          farm = farmOf (keys ++ binKeys);
        in
        mkUnit {
          name = "rust-test-bins-${name}";
          mode = "test-bins";
          fileset = testSet crate (g.roots.${name} or [ ]);
          spec = {
            inherit flavour binKeys;
            testKeys = keys;
            inherit (e) env declared;
            crate = crateSpec crate;
            units = unitsOf (keys ++ binKeys);
            workspace = workspaceOf (keys ++ binKeys);
            farm = map toString farm;
            layer = if layer == null then null else "${layer}";
            base = if base == null then null else "${base}";
            inherit (crate) harness;
          };
          passthru = {
            kind = "test-bins";
            inherit flavour;
            crate = name;
            unitDeps = layers' ++ farm ++ lib.optional (base != null) base;
          };
        }
      ) (if flavour == "test" then g.tests else { });

      # WHAT A CRATE'S TESTS READ WHILE THEY RUN: its own `run-env` and that
      # of every workspace crate below it (a shell test reaching
      # `ronoctua_inspect::tools` reads `RONOCTUA_FFMPEG`), with the values
      # the tests were always given — `generated` and `testEnv`. Every
      # declared name has one (`crateOf` refuses the others).
      runNames = lib.mapAttrs (
        name: _:
        let
          key = g.libKey name;
          below = if key == null then [ ] else map (d: byKey.${d.key}.pkg) (wsLibDeps key);
        in
        lib.unique (crates.${name}.declared.run ++ lib.concatMap (c: runNames.${c}) below)
      ) crates;
      runNamesOf =
        name: keys:
        lib.unique (
          crates.${name}.declared.run
          ++ lib.concatMap (k: lib.concatMap (d: runNames.${byKey.${d.key}.pkg}) (wsLibDeps k)) keys
        );
      runEnvOf = name: keys: lib.getAttrs (runNamesOf name keys) runTable;

      testRuns = lib.mapAttrs (
        name: keys:
        let
          crate = crates.${name};
        in
        mkUnit {
          name = "rust-test-run-${name}";
          mode = "test-run";
          fileset = testSet crate (g.roots.${name} or [ ]);
          spec = {
            inherit flavour;
            crate = crateSpec crate;
            testBins = "${testBins.${name}}";
            runEnv = runEnvOf name keys;
            libraryPath = testLibraryPath;
          };
          passthru = {
            kind = "test-run";
            inherit flavour;
            crate = name;
            unitDeps = [ testBins.${name} ];
          };
        }
      ) (if flavour == "test" then g.tests else { });

      clippy = lib.mapAttrs (
        name: targets:
        let
          crate = crates.${name};
          e = compileEnv flavour crate;
          base = baseOf name;
          libKeys = lib.unique (
            lib.concatMap (t: map (d: d.key) (lib.filter (d: !(d.run or false)) t.deps)) targets
          );
          ours = lib.filter (k: (byKey.${k} or { workspace = false; }).workspace) libKeys;
          theirs = lib.filter (
            k: thirdParty && byKey ? ${k} && !byKey.${k}.workspace && isDepLib byKey.${k}
          ) libKeys;
          farm = lib.attrValues (
            lib.foldl' (
              acc: k: acc // belowOfLib.${byKey.${k}.pkg} // { "~${byKey.${k}.pkg}" = libDrvOf k; }
            ) { } ours
            // depBelowAll (map (key: { inherit key; }) theirs)
          );
          scriptProfile =
            let
              r = g.runKey name;
            in
            if r == null then
              null
            else
              (lib.findFirst (d: lib.elem "custom-build" byKey.${d.key}.kind) null byKey.${r}.deps).key;
        in
        mkUnit {
          name = "rust-clippy-${name}";
          mode = "clippy";
          fileset = testSet crate (g.roots.${name} or [ ]);
          spec = {
            inherit flavour targets;
            inherit (e) env declared;
            crate = crateSpec crate;
            units = unitsOf ours;
            workspace = lib.listToAttrs (map (k: lib.nameValuePair k "${libDrvOf k}") (ours ++ theirs));
            farm = map toString farm;
            layer = if layer == null then null else "${layer}";
            base = if base == null then null else "${base}";
            # Every target is checked with the test graph's profile, which a
            # test harness of the crate carries.
            profile = byKey.${lib.head g.tests.${name}}.profile;
            scriptProfile = if scriptProfile == null then null else byKey.${scriptProfile}.profile;
            bins = binsOf name;
            inherit (crate) harness;
          };
          passthru = {
            kind = "clippy";
            inherit flavour;
            crate = name;
            unitDeps = layers' ++ farm ++ lib.optional (base != null) base;
          };
        }
      ) (if flavour == "test" then plan.clippy else { });
    in
    {
      inherit
        libs
        scripts
        bins
        testBins
        testRuns
        clippy
        ;
      inherit fmt layer;
      # The third-party units of this flavour's graph, by key (phase 2;
      # empty with the layers).
      deps = lib.optionalAttrs thirdParty (
        lib.genAttrs (lib.filter (k: !byKey.${k}.workspace && isDepLib byKey.${k}) (lib.attrNames byKey)) (
          k: depUnits.${k}
        )
      );
      # What a crate's test harnesses read while running, for `testsOf`.
      runEnvOf = name: runEnvOf name (g.tests.${name} or [ ]);
    };

  # Formatting reads files and compiles nothing: one per crate, whatever the
  # flavour.
  fmt = lib.mapAttrs (
    name: crate:
    let
      g = index "test";
      keys = lib.filter (
        k: g.units.${k}.pkg == name && g.units.${k}.workspace && g.units.${k}.mode != "run-custom-build"
      ) (lib.attrNames g.units);
      roots = lib.unique (
        map (k: {
          inherit (g.units.${k}) src edition;
        }) keys
      );
    in
    mkUnit {
      name = "rust-fmt-${name}";
      mode = "fmt";
      fileset = fmtSet crate;
      spec = {
        crate = {
          inherit (crate) name dir;
        };
        inherit roots;
      };
      passthru = {
        kind = "fmt";
        crate = name;
        unitDeps = [ ];
      };
    }
  ) crates;

  # `rust.testsOf`: ONE test, or a few, out of the harness the test units
  # already built — nothing compiled again, third-party or ours. The checks
  # that used to override `rust.tests` and run `cargo test` for one test
  # (seventeen minutes for one stick test) take this.
  testsOf =
    test:
    {
      name,
      crate,
      target ? "lib",
      exact ? null,
      includeIgnored ? false,
      expect ? null,
      env ? { },
      pre ? "",
      post ? "",
    }:
    let
      c = crates.${crate};
      g = index "test";
    in
    assert fresh;
    assert g.tests ? ${crate} || throw "rust.testsOf: ${crate} has no tests";
    stdenv.mkDerivation {
      inherit name;
      src = sourceOf (testSet c (g.roots.${crate} or [ ]));
      __structuredAttrs = true;
      inherit env;
      unit = common // {
        mode = "test-one";
        crate = {
          inherit (c)
            name
            dir
            pkgEnv
            ;
        };
        testBins = "${test.testBins.${crate}}";
        runEnv = test.runEnvOf crate;
        extraEnv = env;
        libraryPath = testLibraryPath;
        inherit target expect includeIgnored;
        exact = if exact == null then null else lib.toList exact;
      };
      nativeBuildInputs = [
        python3
        rustToolchain
      ];
      dontConfigure = true;
      buildPhase = ''
        runHook preBuild
        ${pre}
        python3 ${driver} test-one
        ${post}
        runHook postBuild
      '';
      installPhase = "mkdir -p $out";
      allowSubstitutes = false;
    };
  # THE UNITS AS DATA (`legacyPackages.<system>.rustUnits`, read by
  # `nix run .#rebuild-cost` and the granularity checks): one entry per
  # derivation of the build, by an id that names it the same way in every
  # tree — `<flavour>/<kind>/<attribute>`, `fmt/<crate>`, `layer/<flavour>`,
  # `aggregate/<name>` — with what it is and which entries it is built
  # from. The derivation's path, its output's and its measurement's
  # (`metrics`) are written without their string context: reading the index
  # builds nothing.
  #
  # A third-party unit (`<flavour>/deps/<key>`) is one derivation for every
  # graph that names its key — the loop's and the test graph's share all of
  # them —, so it is ONE entry, under the first flavour that names it
  # (`loop/deps/<key>`): two would count its work twice. And the layers are
  # entries only while units are built on them (`depsFrom = "layers"`): with
  # third-party units no unit reads a layer, and an entry for one would move
  # with every lock edit as if everything stood on it.
  entries =
    {
      release,
      loop,
      test,
      layers,
      aggregates,
    }:
    let
      plain = builtins.unsafeDiscardStringContext;
      kinds = [
        "deps"
        "libs"
        "scripts"
        "bins"
        "testBins"
        "testRuns"
        "clippy"
      ];
      ofFlavour =
        flavour: set:
        lib.concatMap (
          kind:
          lib.mapAttrsToList (name: drv: {
            id = "${flavour}/${kind}/${name}";
            inherit
              drv
              kind
              flavour
              name
              ;
            crate = drv.passthru.crate;
            deps = drv.passthru.unitDeps;
          }) (set.${kind} or { })
        ) kinds;
      units' =
        ofFlavour "release" release
        ++ ofFlavour "loop" loop
        ++ ofFlavour "test" test
        ++ lib.mapAttrsToList (name: drv: {
          id = "fmt/${name}";
          inherit drv name;
          kind = "fmt";
          flavour = null;
          crate = name;
          deps = [ ];
        }) fmt
        ++ lib.optionals (!thirdParty) (
          lib.mapAttrsToList (flavour: drv: {
            id = "layer/${flavour}";
            inherit drv flavour;
            name = flavour;
            kind = "layer";
            crate = null;
            deps = [ ];
          }) layers
        )
        ++ lib.mapAttrsToList (name: a: {
          id = "aggregate/${name}";
          inherit name;
          inherit (a) drv;
          kind = "aggregate";
          flavour = null;
          crate = null;
          deps = a.members;
        }) aggregates;
      # The first id naming a derivation (`listToAttrs` keeps the first).
      idOf = lib.listToAttrs (map (u: lib.nameValuePair (plain u.drv.drvPath) u.id) units');
      named =
        u: d:
        idOf.${plain d.drvPath}
        or (throw "rustUnits: ${u.id} is built from ${d.name}, which is not an entry of the index");
    in
    lib.listToAttrs (
      map (
        u:
        lib.nameValuePair u.id {
          inherit (u)
            kind
            flavour
            crate
            name
            ;
          drv = plain u.drv.drvPath;
          out = plain u.drv.outPath;
          metrics = if u.drv ? metrics then plain u.drv.metrics.outPath else null;
          deps = lib.unique (map (named u) u.deps);
        }
      ) (lib.filter (u: u.kind != "deps" || idOf.${plain u.drv.drvPath} == u.id) units')
    );
in
{
  inherit
    units
    fmt
    crates
    # What a crate's `data` names, for the check that proves an entry naming
    # no file is refused (`data-entries-name-files`).
    dataOf
    ;
  index = entries;
  testsOf = test: testsOf test;
}
