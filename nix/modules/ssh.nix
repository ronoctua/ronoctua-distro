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

  # SUDO THROUGH THE SAME BAR. A raw `sudo` (a rebuild from a shell) asks the
  # system password through `$SUDO_ASKPASS` when it is run `-A`; point it at
  # the SAME helper under its sudo name, so the bar titles it the system
  # password, not a key (`nix/ssh-askpass.rs` reads its own name). Set always,
  # harmless: `sudo` without `-A` ignores it.
  environment.variables.SUDO_ASKPASS = "${askpass}/bin/ronoctua-sudo-askpass";

  # FORCED EVEN IN A TERMINAL, in a graphical session. The bar now grabs the
  # keyboard every time it rises, MODALLY — over a focused window, on any
  # workspace, and in Independent-keyboards mode where it once could not
  # (`crate::surface`'s `apply_geometry`, the sudo-bar exception). So no secret
  # on this machine is typed into a bare terminal line when a bar could draw
  # it: `SSH_ASKPASS_REQUIRE=force` sends ssh to the bar with a tty present,
  # and `sudo -A` sends sudo's own password there too. The agent that runs
  # `sudo` in a terminal cannot see the password — the bar reaches the person
  # instead, whatever they have in focus, which is the point of the bar.
  #
  # SCOPED TO A GRAPHICAL SESSION (`$WAYLAND_DISPLAY`): a bare console or a
  # recovery TTY has no footer to draw the bar, and forcing the askpass there
  # would make ssh and sudo unanswerable — a lockout with no way back. The
  # console keeps its terminal prompt; only a session that can draw the bar is
  # sent to it.
  environment.interactiveShellInit = ''
    if [ -n "$WAYLAND_DISPLAY" ]; then
      export SSH_ASKPASS_REQUIRE=force
      alias sudo='sudo -A'
    fi
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
