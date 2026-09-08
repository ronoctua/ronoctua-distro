# Builds the native buffers for `wasm32-wasip2` — sections 7.8 and 7.9.
#
# A derivation of its own, and not one more crane target of the host, because
# each buffer is a SEPARATE WORKSPACE: a component cannot be a member of the
# host's workspace — `rust-lld` rejects the component model's symbols. See the
# `exclude` in the root `Cargo.toml`.
#
# The vendor dir comes from the buffer's own `Cargo.lock`, through
# `importCargoLock`: the target is different and so is the dependency set, so
# reusing the host's vendor dir would drag the whole of wgpu into a component
# that does not use it.
{
  lib,
  stdenv,
  rustToolchain,
  rustPlatform,
  glyphdb,
  unicodedb,
  lucidedb,
  # The languages, from `assets/i18n/languages.nix`. The three browsers carry
  # one tag list per language and have to know which ones they carry: the
  # generator writes `tags_en`, `tags_pt`, … and this is what says how to
  # enumerate them without a JSON parser (invariant 11).
  languages,
  wit ? ../crates/ronoctua-protocol/wit,
  protocolCrate ? ../crates/ronoctua-protocol,
  designCrate ? ../crates/ronoctua-design,
  syntaxCrate ? ../crates/ronoctua-syntax,
  syntaxRs,
}:

let
  # THE MIRRORED TREE NEEDS A WORKSPACE ROOT, and finding out cost a build.
  #
  # A buffer now depends on `crates/ronoctua-design`, which depends on
  # `crates/ronoctua-protocol` — and the protocol's manifest says
  # `version.workspace = true`. Inside the derivation there is no root to
  # inherit from, and cargo fails with
  #
  #   error inheriting `edition` from workspace root manifest's
  #   `workspace.package.edition` ... failed to find a workspace root
  #
  # which names neither the crate nor the field that caused it.
  #
  # A STUB, and not a copy of the repository's root: that one lists
  # twenty-three members, twenty-one of which are not in this tree, and cargo
  # refuses a member it cannot load before it looks at anything else.
  #
  # `exclude = [ "buffers" ]` because a buffer carries its own `[workspace]`
  # and must go on being its own root — the reason is at the top of this file,
  # and in the root manifest of the repository.
  #
  # The values are the repository's, and they are duplicated on purpose rather
  # than read: this file cannot import a `Cargo.toml`, and a wrong `edition`
  # here fails the build immediately and loudly, which is the failure mode a
  # duplicate is allowed to have.
  workspaceRoot = builtins.toFile "Cargo.toml" ''
    [workspace]
    resolver = "2"
    members = [ "crates/ronoctua-protocol", "crates/ronoctua-design", "crates/ronoctua-syntax" ]
    exclude = [ "buffers" ]

    [workspace.package]
    version = "0.1.0"
    edition = "2024"
    rust-version = "1.90"
    license = "MIT"
    publish = false

    [workspace.lints.rust]
    unsafe_code = "warn"
    missing_docs = "warn"

    [workspace.lints.clippy]
    needless_collect = "warn"
    redundant_clone = "warn"
  '';

  # WHAT OF A BUFFER GOES INTO THE STORE.
  #
  # It used to be `src = ../buffers/<name>` — a raw path, unfiltered. Whatever
  # was in the directory went in: a `target/` left behind by someone running
  # `cargo build` there puts gigabytes in the store, and every edit mints a new
  # copy. That is the class the tree already paid 793 GB for, still open in this
  # corner (`lean-tree` measures the FLAKE's tree and would not see it, because
  # this path is copied on its own).
  #
  # The same criterion as `nix/rust.nix`'s `rustFont`: the sources, the manifest
  # and the lockfile. Nothing else is read by the build below.
  onlySources =
    src:
    lib.fileset.toSource {
      root = src;
      fileset = lib.fileset.unions [
        (lib.fileset.fileFilter (f: f.hasExt "rs") src)
        (src + "/Cargo.toml")
        # MAYBE MISSING, since the mirror also copies two workspace MEMBERS.
        #
        # A buffer is its own workspace and has a lockfile; `ronoctua-protocol`
        # and `ronoctua-design` are members of the repository's workspace and
        # have none. `lib.fileset.unions` refuses a path that does not exist,
        # naming the file and not the caller — so the possibility is declared
        # here rather than discovered from a trace two derivations away.
        (lib.fileset.maybeMissing (src + "/Cargo.lock"))
      ];
    };

  buffer =
    {
      name,
      src,
      extra ? { },
    }:
    stdenv.mkDerivation (
      {
        pname = "ronoctua-buffer-${name}";
        version = "1";
        src = onlySources src;

        # IT MIRRORS THE REPOSITORY LAYOUT.
        #
        # `wit_bindgen::generate!` takes the path to the `.wit` as a LITERAL —
        # there is no way to pass it through an environment variable. So either the
        # path in the code differs between the devshell and Nix (two truths), or the
        # derivation rebuilds the tree the code expects. The second option is what
        # keeps `../../crates/ronoctua-protocol/wit` true in both.
        unpackPhase = ''
          runHook preUnpack
          mkdir -p buffers/${name} crates
          cp -r ${src}/. buffers/${name}/
          # THE TWO CRATES A BUFFER LINKS, in full and not just the WIT.
          #
          # `ronoctua-design` is the layout vocabulary and it is a path
          # dependency; it in turn depends on `ronoctua-protocol`, whose CODE
          # (not only its `wit/`) is what the arithmetic is written against.
          # The `wit` argument stays separate because it is also what
          # `wit_bindgen::generate!` reads by literal path.
          cp -r ${onlySources protocolCrate}/. crates/ronoctua-protocol/
          cp -r ${onlySources designCrate}/. crates/ronoctua-design/
          # AND THE HIGHLIGHTER, which `ronoctua-design` names as an OPTIONAL
          # path dependency. Cargo resolves a path dependency whether or not
          # the feature that switches it on is enabled, so the directory has to
          # be here for every buffer — the compiled table only reaches the
          # `.wasm` of one that asks for `syntax`.
          cp -r ${onlySources syntaxCrate}/. crates/ronoctua-syntax/
          cp ${workspaceRoot} Cargo.toml
          # WRITABLE BEFORE THE WIT GOES IN, and the order is not cosmetic: a
          # `cp -r` out of the store carries the store's read-only mode, so the
          # directory just created cannot be written into and the next line
          # fails with `Permission denied` on a path it did not create.
          chmod -R +w .
          cp -r ${wit} crates/ronoctua-protocol/wit
          cd buffers/${name}
          runHook postUnpack
        '';

        cargoDeps = rustPlatform.importCargoLock {
          lockFile = src + "/Cargo.lock";
        };
        # `cargoSetupHook` looks for `Cargo.lock` in the current directory,
        # which after the `unpackPhase` above is the buffer's.
        cargoRoot = ".";

        nativeBuildInputs = [
          rustToolchain
          rustPlatform.cargoSetupHook
        ];

        # THE SYNTAX TABLE, for `ronoctua-syntax`'s build script.
        #
        # Its `build.rs` runs on the HOST even when the crate is compiled for
        # wasm, and it refuses to generate anything itself (invariant 5) — so
        # the finished table has to be named here, exactly as `nix/rust.nix`
        # names it for the host's own build.
        RONOCTUA_SYNTAX_RS = "${syntaxRs}/syntax.rs";

        buildPhase = ''
          runHook preBuild
          cargo build --target wasm32-wasip2 --release --offline
          runHook postBuild
        '';

        # THE TESTS DO NOT RUN HERE, and the reason is which BATTERY pays.
        #
        # This derivation is an input to almost everything, so switching
        # `doCheck` on here puts the cost on `.#most` — the everyday round —
        # rather than on the gate. Measured: the eight buffers go from 27.8 s to
        # 2 min 44 s, which is +2 min 16 s on a battery of sixteen minutes.
        #
        # So the tests live in `tested` below, which the `buffers-test` check
        # builds and which `tests/slow.nix` keeps out of the everyday round.
        # Same derivation, same phases, one attribute different.
        doCheck = false;

        installPhase = ''
          runHook preInstall
          mkdir -p $out/lib
          cp target/wasm32-wasip2/release/*.wasm $out/lib/
          runHook postInstall
        '';

        meta = {
          description = "Native buffer ${name} (wasm32-wasip2 component)";
          platforms = lib.platforms.linux;
        };
      }
      // extra
    );
in
lib.fix (all: {
  example = buffer {
    name = "example";
    src = ../buffers/example;
  };

  # The file editor — `LEADER f f`. It embeds nothing: the content arrives
  # through `Input::Paste` and leaves through `Effect::Save`, and the host is
  # what touches the disk (invariant 11).

  glyphs = buffer {
    name = "glyphs";
    src = ../buffers/glyphs;
    # The database comes in through a VARIABLE because the component embeds it: it
    # has no filesystem to read it from (invariant 11).
    extra.RONOCTUA_GLYPHS_JSON = "${glyphdb.db}/glyphs.json";
    # The SUBTAGS of the languages whose tags this database carries, in the same
    # order — the first is the SOURCE, English, which is where the words come
    # from. `pt-BR` becomes `pt`: a translation is of the language, not of the
    # region.
    extra.RONOCTUA_TAG_LANGUAGES = lib.concatStringsSep "," (
      map (l: lib.toLower (builtins.head (lib.splitString "-" l))) languages
    );
  };

  unicode = buffer {
    name = "unicode";
    src = ../buffers/unicode;
    extra.RONOCTUA_UNICODE_JSON = "${unicodedb.db}/unicode.json";
    # The SUBTAGS of the languages whose tags this database carries, in the same
    # order — the first is the SOURCE, English, which is where the words come
    # from. `pt-BR` becomes `pt`: a translation is of the language, not of the
    # region.
    extra.RONOCTUA_TAG_LANGUAGES = lib.concatStringsSep "," (
      map (l: lib.toLower (builtins.head (lib.splitString "-" l))) languages
    );
  };

  lucide = buffer {
    name = "lucide";
    src = ../buffers/lucide;
    extra.RONOCTUA_LUCIDE_JSON = "${lucidedb.db}/lucide.json";
    # The SUBTAGS of the languages whose tags this database carries, in the same
    # order — the first is the SOURCE, English, which is where the words come
    # from. `pt-BR` becomes `pt`: a translation is of the language, not of the
    # region.
    extra.RONOCTUA_TAG_LANGUAGES = lib.concatStringsSep "," (
      map (l: lib.toLower (builtins.head (lib.splitString "-" l))) languages
    );
  };

  # The memory panel — `LEADER s m`. It embeds NOTHING, and it is the first one
  # that could not: every number it draws is read by the host at that moment and
  # pushed in through `input::survey`. What it declares instead of a database is
  # `capability::system`.
  # THE KEYBOARDS, and the only buffer that writes something a daemon between
  # the keyboard and everything else reads. See its own header.
  keyboard = buffer {
    name = "keyboard";
    src = ../buffers/keyboard;
  };

  memory = buffer {
    name = "memory";
    src = ../buffers/memory;
  };

  # The transfer queue in a window of its own — `LEADER t w`. Like `memory` it
  # embeds nothing: every row it draws was read by the host at that moment and
  # pushed in through `input::queue`. What it declares instead of a database is
  # `capability::transfers`.
  transfers = buffer {
    name = "transfers";
    src = ../buffers/transfers;
  };

  # THE BRAIN — `LEADER b b`. The eighth, and the first whose data is not a
  # reading of the machine but a POLICY: when a task may run, what it would
  # run, and what happened last time. It embeds nothing and reads nothing; the
  # whole `mind` arrives through `input::mind`, and what it declares is
  # `capability::brain`, which exists separately from `system` because this one
  # ends with the machine rebuilding its own operating system.
  brain = buffer {
    name = "brain";
    src = ../buffers/brain;
  };

  # Formatting a disk, in three questions — `LEADER s f`. Embeds nothing and
  # reads nothing: the devices arrive through `input::rack`, already read, cut
  # and judged by the host. What it declares is `capability::disks`, which is
  # the heaviest of the five and the reason the other six do not have it.
  format = buffer {
    name = "format";
    src = ../buffers/format;
  };

  # THE SAME EIGHT, WITH THEIR OWN TESTS RUN.
  #
  # Sixty-nine tests across the eight, and until this existed not one of them
  # ran: the derivation above compiled the `.wasm` and stopped, and no check
  # under `tests/` reached them. They were green only for as long as somebody
  # remembered to run them by hand — and they could not even be run by hand
  # without knowing which variables to set, because the three browsers embed
  # their database with `include_str!(env!(…))` and do not compile without it.
  #
  # It is the trap `AGENTS.md` already records once: *"`cargo test` did not have
  # the five programs, and every ffmpeg test was EMPTY"*. There the variables
  # were missing and the tests left by the back door in silence; here they are
  # missing and the crate does not even COMPILE, which is a better failure and
  # still one nobody was made to see.
  #
  # # Why the HOST target, when a buffer is a component
  #
  # Because a test has to RUN, and this machine does not run `wasm32-wasip2`.
  # What these prove is arithmetic over a frame — where a field starts, what a
  # click divides by, whether anything reaches past the edge — and none of that
  # is what the component model changes. What only runs INSIDE a component is
  # proved by the drivers in `crates/ronoctua-runtime`, which load the real
  # `.wasm`.
  #
  # Debug and not release: the release profile carries `lto = true`, and paying
  # for a second link buys a test suite nothing.
  tested = lib.mapAttrs (
    _: d:
    d.overrideAttrs (o: {
      pname = "${o.pname}-test";
      doCheck = true;
      checkPhase = ''
        runHook preCheck
        cargo test --offline
        runHook postCheck
      '';
    })
  ) (lib.filterAttrs (n: _: n != "tested") all);
})
