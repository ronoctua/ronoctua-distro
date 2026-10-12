# SSH on the installed system: the key exchange POST-QUANTUM FIRST, and an
# sshd — if the person ever turns one on — with an ed25519 host key alone.
#
# The table is `assets/ssh.nix`, shared with the installer's ISO
# (`nix/installer/iso.nix`), so the machine and the medium that made it agree
# keys the same way. `ssh-kex-is-pq-first` reads the configuration ssh itself
# would read (`ssh -G`), on both profiles and on the ISO; `no-rsa-host-key`
# reads the host keys.
#
# WHY THE CLIENT TOO, when the belongings already ask for post-quantum only:
# the belongings' ssh reads no configuration (`-F none`) and says its own
# list. This is every OTHER ssh on the machine — a `git push`, a session to a
# server — which should offer the hybrids first and still reach a server that
# has none.
{
  config,
  lib,
  ...
}:

let
  table = import ../../assets/ssh.nix;
in
{
  programs.ssh.kexAlgorithms = table.postQuantum ++ table.classical;

  # Off on this system, and set anyway: a person who turns sshd on gets the
  # hybrids first, a server's narrower classical list after them, and no RSA
  # key made at its first start.
  services.openssh.settings.KexAlgorithms = table.postQuantum ++ table.serverClassical;
  services.openssh.hostKeys = table.hostKeys;

  # THE RSA HOST KEY A MACHINE KEPT BEFORE 2026-10-05. `/persist` no longer
  # binds it (`nix/modules/persist.nix`), so what is left there is a private
  # key nothing reads; it goes, declaratively, at the next boot.
  systemd.tmpfiles.rules = lib.mkIf config.ronoctua.persist.enable [
    "r /persist/etc/ssh/ssh_host_rsa_key - - - - -"
    "r /persist/etc/ssh/ssh_host_rsa_key.pub - - - - -"
  ];
}
