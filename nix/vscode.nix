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
# WHY THE ENVIRONMENT SAYS `ID=linux`
# ------------------------------------
# rust-analyzer's extension reads `/etc/os-release`, and seeing `ID=nixos` —
# which the environment showed, the host's file linked in — it does not run
# the server it ships: it copies it aside and patches its loader with
# `nix-build` over `<nixpkgs>` (measured in the vm test: a build of its own
# at the first start of every version of the extension — with the global
# flake registry from the store, `nix/flake-registry.nix`, it needs no
# network — and a copy in the store for each). Inside this environment that
# patch is for nothing — the loader it replaces is here — so the environment
# shows the host's `os-release` with one line changed, `ID=linux` (the
# value os-release(5) says an unnamed system has), and the extension runs its
# own server at once, offline and with no setting. VS Code itself reads `ID`
# only to ask whether it is on Alpine, and for its telemetry, which is off.
# The file is written at each start
# under `$XDG_RUNTIME_DIR` from the host's, so everything else in it stays
# the machine's. `nixosId` keeps the host's `ID` — only for the test that
# measures the patch.
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
{
  pkgs,
  nixosId ? false,
}:

let
  inherit (pkgs) lib coreutils;
  # THE HOST'S `os-release` WITH `ID=linux`, made before the environment is
  # (`extraPreBwrapCmds` runs ahead of the loops that link the host's `/etc`
  # entries in): listed as one of the environment's own, it is bound in its
  # place and the host's is not linked.
  saysLinux = ''
    os_release="''${XDG_RUNTIME_DIR:-''${TMPDIR:-/tmp}}/vscode-fhs-os-release"
    {
      if [ -r /etc/os-release ]; then
        while IFS= read -r line || [ -n "$line" ]; do
          case "$line" in
            ID=*) printf 'ID=linux\n' ;;
            *) printf '%s\n' "$line" ;;
          esac
        done < /etc/os-release
      else
        printf 'ID=linux\n'
      fi
    } > "$os_release.$$" && ${coreutils}/bin/mv -f "$os_release.$$" "$os_release"
    etc_ignored+=(/etc/os-release)
    ro_mounts+=(--ro-bind "$os_release" /etc/os-release)
  '';
in
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
          }
          // lib.optionalAttrs (!nixosId) {
            extraPreBwrapCmds = (fhs.extraPreBwrapCmds or "") + saysLinux;
          };
      }
    );
}).fhs
