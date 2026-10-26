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
  pkgs,
  ...
}:

let
  table = import ../../assets/ssh.nix;

  # WHAT ASKS FOR A KEY'S PASSPHRASE WHEN THERE IS NO TERMINAL to ask on — a
  # commit a tool makes on the person's behalf, a GUI git. ssh, finding no tty
  # and a display, runs `$SSH_ASKPASS`; this helper asks in the DISTRO'S OWN
  # bar above the modeline (`crate::sudo_bar`'s ask), over the control socket —
  # not a floating window, which no other prompt on this machine is (invariant
  # 14, the UI). Built on its own (`nix/ssh-askpass.nix`). In a terminal ssh
  # asks on the terminal and this is never run.
  askpass = import ../ssh-askpass.nix { inherit pkgs; };
in
{
  programs.ssh.kexAlgorithms = table.postQuantum ++ table.classical;

  # AN AGENT A SESSION, SO THE PASSPHRASE IS ASKED ONCE. The person signs their
  # commits with a YubiKey SSH key whose private half carries a passphrase
  # (`~/.ssh/id_yubikey_default`, `sk-ssh-ed25519`). Without an agent every
  # commit asks for that passphrase AND a touch; with one, `AddKeysToAgent`
  # gives the agent the key the first time it is used, the passphrase asked
  # then and only then, and every commit after it in the session is a touch of
  # the key alone. `startAgent` is the user service and the `SSH_AUTH_SOCK` the
  # session carries; it does not clash here, `gnupg.agent.enableSSHSupport`
  # being off (`nix/modules/yubikey.nix`).
  programs.ssh.startAgent = true;
  programs.ssh.enableAskPassword = true;
  programs.ssh.askPassword = "${askpass}/bin/ronoctua-ssh-askpass";
  programs.ssh.extraConfig = ''
    AddKeysToAgent yes
  '';

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
