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
  wit ? ../crates/ronoctua-protocol/wit,
}:

let
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
        inherit src;

        # IT MIRRORS THE REPOSITORY LAYOUT.
        #
        # `wit_bindgen::generate!` takes the path to the `.wit` as a LITERAL —
        # there is no way to pass it through an environment variable. So either the
        # path in the code differs between the devshell and Nix (two truths), or the
        # derivation rebuilds the tree the code expects. The second option is what
        # keeps `../../crates/ronoctua-protocol/wit` true in both.
        unpackPhase = ''
          runHook preUnpack
          mkdir -p buffers/${name} crates/ronoctua-protocol
          cp -r ${src}/. buffers/${name}/
          cp -r ${wit} crates/ronoctua-protocol/wit
          chmod -R +w .
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

        buildPhase = ''
          runHook preBuild
          cargo build --target wasm32-wasip2 --release --offline
          runHook postBuild
        '';

        installPhase = ''
          runHook preInstall
          mkdir -p $out/lib
          cp target/wasm32-wasip2/release/*.wasm $out/lib/
          runHook postInstall
        '';

        meta = {
          description = "Buffer nativo ${name} (componente wasm32-wasip2)";
          platforms = lib.platforms.linux;
        };
      }
      // extra
    );
in
{
  example = buffer {
    name = "example";
    src = ../buffers/example;
  };

  # The file editor — `LEADER f f`. It embeds nothing: the content arrives
  # through `Input::Paste` and leaves through `Effect::Save`, and the host is
  # what touches the disk (invariant 11).
  editor = buffer {
    name = "editor";
    src = ../buffers/editor;
  };

  glyphs = buffer {
    name = "glyphs";
    src = ../buffers/glyphs;
    # The database comes in through a VARIABLE because the component embeds it: it
    # has no filesystem to read it from (invariant 11).
    extra.RONOCTUA_GLYPHS_JSON = "${glyphdb.db}/glyphs.json";
  };

  unicode = buffer {
    name = "unicode";
    src = ../buffers/unicode;
    extra.RONOCTUA_UNICODE_JSON = "${unicodedb.db}/unicode.json";
  };

  lucide = buffer {
    name = "lucide";
    src = ../buffers/lucide;
    extra.RONOCTUA_LUCIDE_JSON = "${lucidedb.db}/lucide.json";
  };
}
