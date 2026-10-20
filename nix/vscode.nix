# VS CODE AS THE DISTRO INSTALLS IT: Microsoft's build (`pkgs.vscode`) inside
# the FHS environment nixpkgs makes for it (`.fhs`, bubblewrap), with the
# machine's policy folder visible inside.
#
# WHY THE FHS ENVIRONMENT
# -----------------------
# Extensions carry programs built for other distributions: rust-analyzer's
# VSIX for linux-x64 ships `server/rust-analyzer`, linked against
# `/lib64/ld-linux-x86-64.so.2`, which NixOS does not have. Measured in
# `tests/vscode-vm.nix`, with the extension installed for real and a Rust
# project open: run on the machine, NixOS's stub loader refuses the program
# (127), and with the plain package the extension fails to run it ("Failed to
# execute … --version") and no server starts; in the FHS environment the
# server runs. The environment is CONTAINED — it is VS Code's alone, a
# namespace around it and its children — where `programs.nix-ld` would put a
# loader at that path for every program of the machine.
#
# (rust-analyzer in particular, seeing `ID=nixos` in `/etc/os-release` —
# which the environment shows too —, tries its own way round first: it copies
# its server aside and patches it with `nix-build` against `<nixpkgs>`. The
# test turns that off to measure the program itself; see the test's header.)
#
# WHY A BIND OF `/etc/vscode`
# ---------------------------
# The environment makes its own `/etc` and lets in a list of the host's
# entries (`build-fhsenv-bubblewrap`'s `etcBindEntries`), and `/etc/vscode`
# is not on it: inside, VS Code would find no policy, and its telemetry
# would be on. So the policy's folder is bound, read only — the vm test
# proves the policy read from inside.
#
# A function of `pkgs`, so the module and the vm test build the same thing
# — the test from a `pkgs` of its own that admits the unfree package.
{ pkgs }:

(pkgs.vscode.override {
  buildVscode =
    args:
    pkgs.buildVscode (
      args
      // {
        customizeFHSEnv =
          fhs:
          fhs
          // {
            extraBwrapArgs = (fhs.extraBwrapArgs or [ ]) ++ [ "--ro-bind-try /etc/vscode /etc/vscode" ];
          };
      }
    );
}).fhs
