# The PO token minter — and it runs only while a token is being minted.
#
# WHAT A PO TOKEN IS FOR. Cookies say WHO is asking; a PO token says that
# whoever is asking is a real device. It is the job OAuth2 used to do before
# YouTube removed it — `yt_dlp/extractor/youtube/_base.py:655` refuses OAuth in
# so many words. yt-dlp carries the whole framework for PO tokens
# (`extractor/youtube/pot/`) and **no minter**: minting means executing Google's
# BotGuard challenge, which is obfuscated JavaScript, so it needs a JavaScript
# runtime. That is what this packages.
#
# NOTHING RUNS WHEN NOTHING IS BEING DOWNLOADED, and that is the whole reason
# this file looks the way it does. Upstream offers two shapes:
#
#   - an HTTP SERVER that stays up and caches — faster on repeats, and a process
#     resident for the life of the session;
#   - a SCRIPT, invoked once per token, which prints the token and exits.
#
# The second is the one packaged here, and the first is **deliberately left
# out**: `getpot_bgutil_http.py` is not installed, so there is no code path that
# looks for a server, no port, and nothing anybody has to remember to stop. The
# cost is a few seconds the first time a video needs a token; the tokens are
# cached under `XDG_CACHE_HOME`, so it is not paid again for hours.
#
# DENO AND NOT NODE, and it is not a preference: the Node path wants
# `build/generate_once.js`, which means compiling the TypeScript with swc — a
# build step, a toolchain, and a second thing to keep in step. Deno runs
# `src/generate_once.ts` as it stands. The plugin agrees: it scores Deno 20 and
# Node 10 and picks the higher one.
#
# `--ignore-scripts` IS LOAD-BEARING. `canvas` is in `dependencies` and is
# **never imported** by any file under `server/src` — jsdom only reaches for it
# if a page asks for a 2D context. Building it would drag Cairo, Pango and
# librsvg into this closure and run node-gyp inside the sandbox, for a module
# nothing calls.
{
  lib,
  buildNpmPackage,
  fetchFromGitHub,
  deno,
  nodejs,
}:

buildNpmPackage (finalAttrs: {
  pname = "ronoctua-pot-provider";
  version = "1.3.2";

  src = fetchFromGitHub {
    owner = "Brainicism";
    repo = "bgutil-ytdlp-pot-provider";
    tag = finalAttrs.version;
    hash = "sha256-vlhuw0Ci/xfPgLxjeW7E+Pz9Fo6yeME3cyVRf8NAAPU=";
  };

  # The npm project is the `server/` half; the Python plugin has no build.
  sourceRoot = "${finalAttrs.src.name}/server";

  npmDepsHash = "sha256-hpXVvhJm66+ETJdGAbEa/QZ4rxOYBD8RJqSItlNpoOg=";

  # See the header: no native build, and no development half — the script path
  # imports `axios`, `bgutils-js`, `commander`, `jsdom`, `proxy-agent` and
  # `youtubei.js`, all of them plain JavaScript.
  npmFlags = [
    "--ignore-scripts"
    "--omit=dev"
  ];

  # There is nothing to build: Deno reads the TypeScript.
  dontNpmBuild = true;
  dontNpmPrune = true;

  nativeBuildInputs = [ nodejs ];

  # NODE IS A BUILD TOOL HERE AND NOT A RUNTIME, and without this line it is
  # both: `npm` rewrites the shebang of every CLI inside `node_modules` to the
  # store's `node`, and those shebangs are what Nix scans. Measured — 69 MiB of
  # our own against a 302 MiB closure, and the 233 MiB difference was a runtime
  # nothing here executes, dragged in by files nobody runs.
  #
  # `nixpkgs` calls this out for us rather than leaving it to be noticed: if a
  # reference survives, the build fails naming it.
  disallowedReferences = [ nodejs ];
  dontPatchShebangs = true;

  installPhase = ''
    runHook preInstall

    # `server_home`, the shape the plugin expects: `src/` next to
    # `node_modules/`. Deno is given read and FFI permission for exactly the
    # second one, so the layout is part of the contract and not tidiness.
    mkdir -p $out/server
    cp -r src node_modules package.json $out/server/

    # THE PLUGIN, and only the script half of it.
    #
    # `getpot_bgutil_http.py` is left behind on purpose: installing it would
    # register two more providers that look for a server on a port, find
    # nothing, and say so once per download. Leaving it out is what makes "no
    # server" true rather than merely configured.
    mkdir -p $out/plugin/yt_dlp_plugins/extractor
    cp ../plugin/yt_dlp_plugins/extractor/getpot_bgutil.py \
       ../plugin/yt_dlp_plugins/extractor/getpot_bgutil_script.py \
       $out/plugin/yt_dlp_plugins/extractor/

    runHook postInstall
  '';

  # THE SHEBANGS GO BACK TO WHAT UPSTREAM SHIPS.
  #
  # These are command-line entry points of dependencies — `esparse`, `escodegen`,
  # `semver` — that this package never runs: what runs is Deno, importing the
  # library halves as modules. A `#!` line is legal in a module and is ignored
  # when one is imported, so restoring `env node` costs nothing and removes the
  # reference. `.bin` goes entirely, for the same reason.
  postFixup = ''
    rm -rf $out/server/node_modules/.bin
    grep -rlF "${nodejs}/bin/node" $out/server/node_modules 2>/dev/null | while read -r f; do
      sed -i "1s|^#!.*${nodejs}/bin/node|#!/usr/bin/env node|" "$f"
    done
    if grep -rqF "${nodejs}" $out 2>/dev/null; then
      echo "ERRO: o node ainda e citado no resultado; ele voltaria ao fecho" >&2
      exit 1
    fi
  '';

  # A namespace package has no `__init__.py`, and yt-dlp's plugin finder relies
  # on that. Saying so here is cheaper than finding out from a build that
  # produced a plugin nothing loads.
  postInstall = ''
    if [ -e $out/plugin/yt_dlp_plugins/__init__.py ]; then
      echo "ERRO: yt_dlp_plugins ganhou __init__.py e deixaria de ser namespace" >&2
      exit 1
    fi
    test -f $out/server/src/generate_once.ts
    test -d $out/server/node_modules/bgutils-js
    # And the runtime this is built for.
    ${lib.getExe deno} --version >/dev/null
  '';

  meta = {
    description = "O gerador de PO token, em modo script: roda por pedido e sai";
    homepage = "https://github.com/Brainicism/bgutil-ytdlp-pot-provider";
    license = lib.licenses.gpl3Only;
    platforms = lib.platforms.linux;
  };
})
