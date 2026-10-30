# THE ONE Claude Code OF THE DISTRO — newer than the pinned nixpkgs', and a
# single explicit pin kept apart from nixpkgs' packaging of it.
#
# # Why it exists
#
# The agent speaks to a model, and a model the CLI has never heard of it will
# not open: a Claude Code too old simply does not list the current Opus, so a
# person on this distro could not choose it. nixpkgs moves slower than the
# vendor ships, and (measured 2026-10-30) its newest `claude-code` was 2.1.187,
# three weeks and thirty-odd releases behind the 2.1.314 the vendor served
# that day — and the model of the day, `claude-opus-5-5`, became selectable
# only at 2.1.280. So the version is pinned HERE, moved on its own, the way
# `nix/yt-dlp.nix` pins yt-dlp against a nixpkgs that cannot keep YouTube's
# pace.
#
# # Why a replacement and not an override
#
# nixpkgs has already changed the SHAPE of this package once: 2.1.1 was a
# `buildNpmPackage` (an npm tarball with a vendored lock), and by 2.1.187 it
# was the prebuilt vendor binary this file follows. The distro's pinned
# nixpkgs and a machine's can sit on either side of that change, and an
# `overrideAttrs` that fits one breaks the other. So this overlay REPLACES
# `claude-code` outright with a self-contained copy of the prebuilt-binary
# recipe — depending on nothing of whatever nixpkgs happens to carry.
#
# The wrapper keeps `DISABLE_AUTOUPDATER=1`, as nixpkgs' own does: the agent
# never replaces itself behind the generation's back (invariant 6). Its
# version moves only when the pin below moves.
#
# # Who reads it
#
# The unfree parts instance (`nix/parts-pkgs.nix`), so the `claude-code` PART
# (`nix/modules/dev.nix`) and the footer's session of it get this build.
#
# # When to bump it
#
# Take the newest version from
# https://www.npmjs.com/package/@anthropic-ai/claude-code (or the GitHub
# releases), set `version`, then for x86_64-linux run
#
#   nix store prefetch-file --name claude \
#     https://downloads.claude.ai/claude-code-releases/<version>/linux-x64/claude
#
# and paste the printed `hash` below. The URL names its version, so the pin
# does not shift under a rebuild.
final: _prev:
let
  inherit (final) lib;

  version = "2.1.314";
  baseUrl = "https://downloads.claude.ai/claude-code-releases";

  # One vendor platform key and binary checksum per system the distro builds
  # for. The distro is x86_64-linux; a build for anything else fails loudly
  # rather than fetching a binary no checksum here vouches for.
  bySystem = {
    "x86_64-linux" = {
      key = "linux-x64";
      hash = "sha256-RIcEK0xPXYjUVo9orpiq4LH/rliGTPV/uGhkG8X4RZQ=";
    };
  };

  system = final.stdenvNoCC.hostPlatform.system;
  here =
    bySystem.${system} or (throw "nix/claude-code.nix: no pinned Claude Code binary for ${system}");
in
{
  claude-code = final.stdenvNoCC.mkDerivation (finalAttrs: {
    pname = "claude-code";
    inherit version;

    src = final.fetchurl {
      url = "${baseUrl}/${finalAttrs.version}/${here.key}/claude";
      sha256 = here.hash;
    };

    dontUnpack = true;
    dontBuild = true;
    # The vendor binary carries its own bun runtime; stripping it runs the
    # runtime instead of the program (nixpkgs' own note).
    dontStrip = true;

    strictDeps = true;
    nativeBuildInputs = [
      final.makeBinaryWrapper
      final.autoPatchelfHook
    ];

    installPhase = ''
      runHook preInstall

      install -Dm755 $src $out/bin/claude

      wrapProgram $out/bin/claude \
        --set DISABLE_AUTOUPDATER 1 \
        --set-default FORCE_AUTOUPDATE_PLUGINS 1 \
        --set DISABLE_INSTALLATION_CHECKS 1 \
        --set USE_BUILTIN_RIPGREP 0 \
        --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ final.alsa-lib ]} \
        --prefix PATH : ${
          lib.makeBinPath [
            # node-tree-kill wants ps(1); the docs want ripgrep; the sandbox
            # wants bubblewrap and socat.
            final.procps
            final.ripgrep
            final.bubblewrap
            final.socat
          ]
        }

      runHook postInstall
    '';

    # Proof the patched binary runs, and that it is the version pinned above.
    doInstallCheck = true;
    nativeInstallCheckInputs = [
      final.writableTmpDirAsHomeHook
      final.versionCheckHook
    ];
    versionCheckKeepEnvironment = [ "HOME" ];
    versionCheckProgramArg = "--version";

    meta = {
      description = "Agentic coding tool that lives in your terminal";
      homepage = "https://github.com/anthropics/claude-code";
      license = lib.licenses.unfree;
      sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
      platforms = lib.attrNames bySystem;
      mainProgram = "claude";
    };
  });
}
