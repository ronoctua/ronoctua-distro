# `ronoctua-ssh-askpass`, BUILT ON ITS OWN: what ssh runs as `$SSH_ASKPASS` to
# ask a key's passphrase in the footer's bar (`crate::sudo_bar`'s ask), not a
# floating window. It speaks the control socket's one line and prints the
# passphrase; `nix/modules/ssh.nix` names it, and its source is beside this
# file (`ssh-askpass.rs`).
#
# A SOURCE OF ITS OWN, with no dependency at all — `rustc` on one file, in
# seconds — so it needs no workspace, no `Cargo`, and no entry in the rust
# plan. The two things it shares with the footer, the control socket's path
# and the `askpass` word, are spelled again in the source (its comments say
# so): the footer's crates would otherwise all recompile with a helper they do
# not use, the reason `nix/git-askpass.nix` exists.
{ pkgs }:

let
  src = builtins.path {
    name = "ronoctua-ssh-askpass.rs";
    path = ./ssh-askpass.rs;
  };
in
# `runCommandCC`: rustc links through `cc`.
pkgs.runCommandCC "ronoctua-ssh-askpass" { nativeBuildInputs = [ pkgs.rustc ]; } ''
  mkdir -p $out/bin
  rustc --edition 2021 -C opt-level=2 -C strip=symbols --cap-lints warn \
    ${src} -o $out/bin/ronoctua-ssh-askpass
''
