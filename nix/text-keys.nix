# THE TEXT KEYS, FROM THEIR NAMES — read at EVALUATION, one file per crate.
#
# Every crate that speaks owns its sentences, `crates/<crate>/i18n/<lang>.ftl`
# (a key used by several crates lives in the nearest one they all stand on),
# and is compiled against the NAMES alone: its `key` module
# (`src/key.rs`, an `include!` of `$RONOCTUA_KEYS_DIR/<crate>.rs`) holds one
# constant per key of its own fragment, re-exports the `key` of every crate
# it stands on that speaks — so `key::X` names any key the crate may say, and
# a file changes only its `use` line when a key moves — and gives the start
# guard its keys (`ALL`, `sets`). The sentences are data every program reads
# at run time (`RONOCTUA_TEXTS`, the `texts` derivation of `nix/i18n.nix`).
#
# So a crate's file is written here, at evaluation, out of the names of its
# own fragment and the list of the crates it stands on that speak — never
# out of a value, nor out of another crate's keys: a sentence edited changes
# no byte of any file, and a key added to one crate changes that crate's file
# alone. Each unit of `nix/rust/units.nix` is handed a directory holding ITS
# file only (`dirOf`), so its derivation depends on its own keys and nothing
# else; a devshell's `cargo`, which builds the whole workspace, gets `all`.
#
# THE SAME BYTES AS `nix/scripts/gen-keys.py`, which writes them into a
# checkout's own directory for `cargo` (the devshell and `.#dev`, rewriting
# only the files that changed, so cargo recompiles only their crates):
# `keys-agree` compares the two.
#
# A CRATE SPEAKS when it says it reads `RONOCTUA_KEYS_DIR`
# (`[package.metadata.ronoctua] env`), which it must, since its `key.rs`
# reads it. A fragment in a crate that does not speak is refused here: those
# sentences would reach the texts and no program's guard.
#
# THE SAME SHAPE AS `gen-i18n.py`'s `KEY`: a message key in column 0,
# `^([a-z][a-z0-9-]*)\s*=`. A line the generator refuses (junk, a key in one
# language only, a key in two fragments) is not this file's business: the
# `texts` derivation refuses it, by name, in the build, and a binary whose keys
# are not all in its texts is refused at its start (`ronoctua_i18n::Texts::checked`).
{
  lib,
  # The tree the crates are read from (`./.` in the flake, the plan's `src`).
  src,
  # The declared languages, the base first (`assets/i18n/languages.nix`).
  languages,
  writeTextDir,
  linkFarm,
  # NAMES ADDED, by crate: `{ <crate> = [ "<key>" ]; }`, each as if its
  # fragment declared it. Empty in every build of the flake; what
  # `a-new-key-rebuilds-its-crate` hands in to add one key to one crate and
  # see what is built again (`tests/text-fragments.nix`).
  nameEdits ? { },
}:
let
  base = builtins.head languages;
  speaks = "RONOCTUA_KEYS_DIR";

  inherit (import ./text-names.nix { inherit lib; }) namesIn;

  # `cmd-file-open` -> `CMD_FILE_OPEN`; `ronoctua-shell-kit` -> `ronoctua_shell_kit`.
  constOf = name: lib.toUpper (builtins.replaceStrings [ "-" ] [ "_" ] name);
  identOf = name: builtins.replaceStrings [ "-" ] [ "_" ] name;
  quoted = s: ''"${s}"'';

  # --- the crates, as their manifests say ----------------------------------------
  root = fromTOML (builtins.readFile (src + "/Cargo.toml"));
  members = root.workspace.members;
  manifestOf = dir: fromTOML (builtins.readFile (src + "/${dir}/Cargo.toml"));
  byName = lib.listToAttrs (
    map (
      dir:
      let
        m = manifestOf dir;
      in
      lib.nameValuePair m.package.name {
        inherit dir;
        manifest = m;
      }
    ) members
  );
  speaking = lib.filterAttrs (
    _: c: lib.elem speaks (c.manifest.package.metadata.ronoctua.env or [ ])
  ) byName;

  fragmentOf = dir: src + "/${dir}/i18n/${base}.ftl";
  hasFragment = dir: builtins.pathExists (src + "/${dir}/i18n");

  # A FRAGMENT NOBODY COMPILES is refused, by the crate's name.
  silent = lib.filter (n: hasFragment byName.${n}.dir && !(speaking ? ${n})) (lib.attrNames byName);

  # THE CRATES IT STANDS ON THAT SPEAK: its normal dependencies, by the name
  # its code uses (the key of the table). A dependency that is optional, only
  # for some targets, or renamed would need a `cfg` or another path in the
  # file — refused rather than written wrong.
  usesOf =
    name:
    let
      m = speaking.${name}.manifest;
      deps = m.dependencies or { };
      packageOf = n: spec: if builtins.isAttrs spec then spec.package or n else n;
      theirs = lib.filter (n: speaking ? ${packageOf n deps.${n}}) (lib.attrNames deps);
      odd = lib.filter (
        n:
        let
          spec = deps.${n};
        in
        builtins.isAttrs spec && ((spec.optional or false) || (spec ? package))
      ) theirs;
      targeted = lib.concatLists (
        lib.mapAttrsToList (_: t: lib.filter (n: speaking ? ${n}) (lib.attrNames (t.dependencies or { }))) (
          m.target or { }
        )
      );
    in
    if odd != [ ] then
      throw "${speaking.${name}.dir}/Cargo.toml: the crates ${lib.concatStringsSep ", " odd} speak, and an optional or renamed dependency on one is not a `key` re-export nix/text-keys.nix writes"
    else if targeted != [ ] then
      throw "${speaking.${name}.dir}/Cargo.toml: the crates ${lib.concatStringsSep ", " targeted} speak, and a dependency on one only for some targets is not a `key` re-export nix/text-keys.nix writes"
    else
      lib.sort (a: b: a < b) theirs;

  namesOf =
    name:
    let
      dir = speaking.${name}.dir;
    in
    (
      if !(hasFragment dir) then
        [ ]
      else if builtins.pathExists (fragmentOf dir) then
        namesIn (builtins.readFile (fragmentOf dir))
      else
        throw "${dir}/i18n has no ${base}.ftl: the base language's file is where a crate's keys are read from"
    )
    ++ (nameEdits.${name} or [ ]);

  # ONE CRATE'S FILE, from its name, its folder, its names and the crates it
  # stands on that speak. `ALL` is a `static` and not a `const`: a `const`
  # array is inlined at every use site.
  render =
    {
      name,
      dir,
      names,
      uses,
    }:
    let
      every = lib.sort (a: b: a < b) (lib.unique names);
    in
    ''
      // GENERATED by nix/text-keys.nix (and by nix/scripts/gen-keys.py in a checkout)
      // from the key NAMES of ${dir}/i18n/${base}.ftl. Do not edit.
      //
      // One constant per key this crate says, and the keys of every crate it stands
      // on that speaks, re-exported: `key::X` names any key the crate may say. The
      // sentences are not here: they are read at run time (`RONOCTUA_TEXTS`).

    ''
    + lib.concatMapStrings (u: "pub use ${identOf u}::key::*;\n") uses
    + lib.optionalString (uses != [ ]) "\n"
    + lib.concatMapStrings (n: "pub const ${constOf n}: &str = ${quoted n};\n") every
    + lib.optionalString (every != [ ]) "\n"
    + ''
      /// The keys this crate says, in order.
      pub static ALL: [&str; ${toString (builtins.length every)}] = [
      ${lib.concatMapStrings (n: "    ${quoted n},\n") every}];

      /// This crate's keys and those of every crate it stands on that speaks, each
      /// crate once: what a program's start guard checks its texts against
      /// (`ronoctua_i18n::Keys::of`).
      pub fn sets(into: &mut Vec<(&'static str, &'static [&'static str])>) {
          if into.iter().any(|(by, _)| *by == ${quoted name}) {
              return;
          }
          into.push((${quoted name}, &ALL));
      ${lib.concatMapStrings (u: "    ${identOf u}::key::sets(into);\n") uses}}
    '';

  rustOf =
    name:
    render {
      inherit name;
      inherit (speaking.${name}) dir;
      names = namesOf name;
      uses = usesOf name;
    };

  files = lib.mapAttrs (name: _: rustOf name) speaking;
in
assert
  lib.all (n: speaking ? ${n}) (lib.attrNames nameEdits)
  || throw "nix/text-keys.nix: nameEdits names ${lib.concatStringsSep ", " (lib.attrNames nameEdits)}, and only a crate that speaks has keys";
assert
  silent == [ ]
  || throw "${lib.concatStringsSep ", " silent} carry an i18n/ folder and do not declare ${speaks} in [package.metadata.ronoctua] env: their sentences would reach the texts and no program's start guard";
{
  inherit namesIn render;

  # Every crate that speaks, with its fragment's folder (or `null`), its names
  # and the crates it stands on that speak.
  crates = lib.mapAttrs (name: c: {
    inherit (c) dir;
    fragment = if hasFragment c.dir then src + "/${c.dir}/i18n" else null;
    names = namesOf name;
    uses = usesOf name;
  }) speaking;

  # Each crate's file, as text.
  inherit files;

  # THE DIRECTORY ONE UNIT IS HANDED: its crate's file and nothing else.
  dirOf = name: writeTextDir "${name}.rs" files.${name};

  # EVERY CRATE'S FILE IN ONE DIRECTORY, for what builds the whole workspace
  # (`generated`: a devshell entered without its hook, a lane).
  all = linkFarm "text-keys" (
    lib.mapAttrsToList (name: text: {
      name = "${name}.rs";
      path = builtins.toFile "${name}.rs" text;
    }) files
  );

  # THE LANGUAGES, the base first: `ronoctua_i18n::LANGUAGES`.
  languagesRs = builtins.toFile "languages.rs" ''
    // GENERATED at evaluation by nix/text-keys.nix from assets/i18n/languages.nix.

    /// Mandatory languages. An incomplete `.ftl` fails the build.
    ///
    /// The FIRST is the base every other is compared against.
    pub const LANGUAGES: [&str; ${toString (builtins.length languages)}] = [
    ${lib.concatMapStrings (l: "    ${quoted l},\n") languages}];
  '';
}
