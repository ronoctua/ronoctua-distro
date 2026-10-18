# The interface's texts — invariant 8.
#
# The sentences live with the code that says them: every crate that speaks
# owns `crates/<crate>/i18n/<lang>.ftl` (a key used by several crates lives in
# the nearest one they all stand on), the browser extension owns
# `extension/i18n/`, and each brain module its own folder's. `assets/i18n/`
# keeps only `languages.nix`. Two things come out of them, kept apart on
# purpose:
#
# - THE TEXTS (this derivation): `gen-i18n.py` checks every declared language
#   of every fragment — the same keys in each language of a fragment, no key
#   in two fragments, the same arguments in each value, none empty, every
#   module key inside its namespace — and writes one MERGED `<lang>.ftl` per
#   language, every fragment by its name and then every module's. Every
#   program that speaks reads them at RUN TIME, through `RONOCTUA_TEXTS`
#   (`nix/rust.nix`, `keyed`: the wrappers `--set` it). An incomplete fragment
#   is a build error HERE, before any test.
# - THE KEYS (`passthru`, `nix/text-keys.nix`): each crate's Rust `key` module,
#   written at EVALUATION out of the key NAMES of its own fragment alone. It is
#   all a compilation reads of the texts (`RONOCTUA_KEYS_DIR`, one file per
#   crate, and a unit is handed only its own), so editing a sentence rebuilds
#   no crate, and adding a key rebuilds the crate that says it.
#
# Each fragment's folder is copied into the store ON ITS OWN (never
# `"${src}/…"`, which would make every file of the tree an input of the texts).
#
# AND THE BRAIN'S MODULES' TEXTS WITH THEM (brain.md §3). A module carries its
# own `i18n/<lang>.ftl` in its folder; each folder is handed to the generator
# with the prefixes its keys may begin with. A module's keys' names join no
# crate's file: a DISTRO module's are its generated manifest's `key` (the
# update's window names its module's sentences by constant); a PERSON's never
# reach a compilation of the footer: what the footer says of a person's module
# — its name, its marks' hints, its actions' labels, its notifications — it
# names by the keys the brain's catalogue carries, read at run time
# (`nix/brain-catalogue.nix`), so a person's module changes the texts, which
# are data, and nothing every crate that speaks is compiled against.
{
  lib,
  stdenvNoCC,
  python3,
  writeTextDir,
  linkFarm,
  # The tree the crates are read from (the flake's `./.`), for their manifests
  # and fragments.
  src,
  # The browser extension's fragment, `extension/i18n`.
  extensionDir,
  # The languages, from `assets/i18n/languages.nix`. Passed in rather than
  # imported here so there is exactly one `import` of that file in the flake.
  languages,
  # The modules' text folders, `[ { id; prefixes; origin; dir; } ]`: the flake's one
  # loader's `i18nFragments`. A name nixpkgs has no package for: `callPackage`
  # fills an argument from `pkgs` first, and `fragments` is one.
  brainFragments,
  # FILES EDITED, by fragment and language: `{ <crate> = { <lang> = text; }; }`,
  # each text in place of that fragment's `<lang>.ftl`. Empty in every build of
  # the flake, and the derivation is then the one without it; what
  # `text-values-rebuild-no-rust` hands in to change one sentence and see what
  # is built again (`tests/build-granularity.nix`). Text, never a derivation:
  # the names are read at evaluation, and reading a build's output there is
  # import-from-derivation. A VALUE edit: the keys are written from the tree's
  # own fragments, so an edit that adds a name is not one this simulates.
  fileEdits ? { },
  # KEYS ADDED, by crate (`nix/text-keys.nix`'s `nameEdits`): what
  # `a-new-key-rebuilds-its-crate` hands in. The sentences are not touched:
  # the edit is to the names a crate is compiled against.
  keyEdits ? { },
}:

let
  textKeys = import ./text-keys.nix {
    inherit
      lib
      src
      languages
      writeTextDir
      linkFarm
      ;
    nameEdits = keyEdits;
  };
  base = builtins.head languages;
  extensionName = "extension";

  # EVERY FRAGMENT, by name: each speaking crate with an `i18n/` folder, and
  # the extension's.
  folders =
    lib.mapAttrs (_: c: c.fragment) (lib.filterAttrs (_: c: c.fragment != null) textKeys.crates)
    // {
      ${extensionName} = extensionDir;
    };
  copied = lib.mapAttrs (
    name: folder:
    builtins.path {
      path = folder;
      name = "texts-${name}";
    }
  ) folders;

  unknownEdits = lib.filter (n: !(folders ? ${n})) (lib.attrNames fileEdits);

  # THE NAMES OF EVERY MESSAGE THE MERGED FILES HOLD, read at evaluation: the
  # crates' (as `nix/text-keys.nix` reads them), the extension's and the
  # modules'. A file read by path — never `"${dir}"`, which would copy it and
  # hand the names its context.
  namesOf = file: if builtins.pathExists file then textKeys.namesIn (builtins.readFile file) else [ ];
  # THE BASE'S NAMES: every message outside the modules — the crates' (as
  # `nix/text-keys.nix` reads them) and the extension's. What a person's
  # module may name outside its own fragment (`nix/brain-modules.nix`,
  # `baseKeyNames`).
  baseNames =
    lib.concatLists (lib.mapAttrsToList (_: c: c.names) textKeys.crates)
    ++ (
      if fileEdits ? ${extensionName}.${base} then
        textKeys.namesIn fileEdits.${extensionName}.${base}
      else
        namesOf (extensionDir + "/${base}.ftl")
    );
  fragmentNames = f: namesOf "${f.dir}/${base}.ftl";
  # EVERY NAME: the base's and every module's, a person's included.
  names = baseNames ++ lib.concatMap fragmentNames brainFragments;

  # A fragment edited is a copy of its folder with the edited files laid over.
  laidOver = lib.concatStrings (
    lib.mapAttrsToList (
      name: langs:
      "cp -r --no-preserve=mode ${copied.${name}} edited-${name}\n"
      + lib.concatStrings (
        lib.mapAttrsToList (
          lang: text: "cp ${builtins.toFile "${lang}.ftl" text} edited-${name}/${lang}.ftl\n"
        ) langs
      )
    ) fileEdits
  );
  folderOf = name: if fileEdits ? ${name} then "edited-${name}" else "${copied.${name}}";
in
assert
  unknownEdits == [ ]
  || throw "nix/i18n.nix: fileEdits names ${lib.concatStringsSep ", " unknownEdits}, which no fragment is";
stdenvNoCC.mkDerivation {
  pname = "ronoctua-texts";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    ${laidOver}python3 ${./scripts/gen-i18n.py} $out ${lib.escapeShellArgs languages} ${
      lib.concatMapStringsSep " " (name: "--fragment ${lib.escapeShellArg "${name}=${folderOf name}"}") (
        lib.attrNames folders
      )
    } ${
      lib.concatMapStringsSep " " (
        f:
        "--module ${lib.escapeShellArg "${f.id}=${f.dir}"} --prefixes ${lib.escapeShellArg "${f.id}=${lib.concatStringsSep "," f.prefixes}"}"
      ) brainFragments
    }
    runHook postBuild
  '';

  installPhase = "true";

  passthru = {
    # THE KEYS, written at evaluation: every crate's file in one directory
    # (`RONOCTUA_KEYS_DIR` of `generated`: a devshell entered without its
    # hook), and one unit's own (`keysOf <crate>`).
    keys = textKeys.all;
    keysOf = textKeys.dirOf;
    # `ronoctua_i18n::LANGUAGES` (`RONOCTUA_LANGUAGES_RS`).
    inherit (textKeys) languagesRs;
    # The reader itself, for the checks: every crate that speaks (its folder,
    # names and the crates it stands on that speak), each file's text, and the
    # names of a `.ftl`'s text.
    inherit textKeys;
    # And every name the merged files hold, for the checks.
    inherit names;
    # The fragments' folders as the derivation reads them, by name.
    fragments = copied;
    # THE BASE'S NAMES alone, and EVERY NAME THE TEXTS HOLD (`names`) — the
    # base's and every module's, a person's included: what the brain's
    # catalogue may ask of the texts (`nix/brain-catalogue.nix`) and what a
    # person's module may name outside its own fragment
    # (`nix/brain-modules.nix`, `baseKeyNames`).
    inherit baseNames;
    allNames = names;
  };

  meta.description = "The interface's sentences, checked and merged per language, read at run time";
}
