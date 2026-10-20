# alive-lsp, the Common Lisp language server of VS Code's Alive extension.
#
# Packaged here because nixpkgs has none. It is SBCL's only (it reads
# `sb-introspect`), so it joins the SBCL set and never the libraries CCL and
# ECL read (`./default.nix`). Only the server's system is built: the `.asd`
# also declares `alive-lsp/test`, which wants `clue` and `sb-cover`, and
# nothing here asks for it.
#
# ONE PATCH, OURS (`alive-lsp-accept-once.patch`): `alive/server:start` takes
# `:accept-once t`, which closes the listener after the first client is
# accepted and keeps the server alive as long as that client's session.
# Upstream keeps accepting — every process of the machine that finds the
# port gets a session that evaluates Lisp as the person — and `alive-lsp
# --stdio`, whose bridge is the one client, asks for this instead.
{ pkgs }:

let
  version = "0.4.5";
  fetched = pkgs.fetchFromGitHub {
    owner = "nobody-famous";
    repo = "alive-lsp";
    tag = "v${version}";
    hash = "sha256-fLeWlemMFXyF+mbd2i8Z072xVpC4ppdwpIFQJEFoJdU=";
  };
in
pkgs.sbcl.buildASDFSystem {
  pname = "alive-lsp";
  inherit version;
  src = fetched;
  patches = [ ./alive-lsp-accept-once.patch ];

  lispLibs = with pkgs.sbcl.pkgs; [
    usocket
    cl-json
    bordeaux-threads
    flexi-streams
  ];

  # What was downloaded, for the garbage collector's root (`flake.nix`): the
  # patched tree is a build, not a download.
  passthru = { inherit fetched; };

  meta = {
    description = "Language server for Common Lisp, the one VS Code's Alive extension speaks to";
    homepage = "https://github.com/nobody-famous/alive-lsp";
    license = pkgs.lib.licenses.unlicense;
  };
}
