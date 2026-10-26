# `ronoctua-ssh-askpass` / `ronoctua-sudo-askpass`, BUILT ON ITS OWN: what ssh
# and sudo run as their askpass to ask a secret in the footer's bar
# (`crate::sudo_bar`'s ask), not a floating window. It speaks the control
# socket's one line and prints the secret; `nix/modules/ssh.nix` names both,
# and its source is beside this file (`ssh-askpass.rs`).
#
# ONE PROGRAM, TWO NAMES: the second a symlink, so `argv[0]` still reflects the
# name invoked, which is how the helper tells its KIND (ssh vs sudo) and the
# footer titles the bar.
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
  ln -s ronoctua-ssh-askpass $out/bin/ronoctua-sudo-askpass
''
