# How SSH agrees its keys, on every machine and on the installer — one table,
# read by `nix/modules/ssh.nix` (the installed system) and
# `nix/installer/iso.nix` (the ISO's sshd and client).
#
# POST-QUANTUM FIRST. A session recorded today can be decrypted the day a
# quantum computer breaks the key it was agreed with ("harvest now, decrypt
# later"), so the hybrids that hold against one are offered before anything
# else. Classical ones stay AFTER them: the system's ssh reaches whatever the
# person points it at, and a server with no post-quantum exchange is still a
# server they mean to reach. The belongings are the exception — their session
# is post-quantum ONLY (`ronoctua_git::PQ_KEX`, the same three as below; the
# `belongings-ssh-is-pq-only` check holds the two lists together).
#
# THE CLIENT AND THE SERVER DIFFER after the hybrids, on purpose. The client
# goes where the person sends it, so it keeps what OpenSSH offers. A server
# chooses who it lets in — the ISO's sshd takes a root login — and keeps only
# what NixOS recommends for one (`serverClassical`): putting the post-quantum
# ones first is no reason to accept the NIST curves or a 2048-bit group a
# server refused before (`ssh-kex-is-pq-first`).
{
  # In order of preference. ML-KEM-768 with X25519 is the standardised one
  # (OpenSSH 9.9); Streamlined NTRU Prime with X25519 comes under both its
  # names (OpenSSH 8.5 on, the default before 9.9) and STAYS: GitHub answers
  # it today and only offers ML-KEM from 2026-10-14.
  postQuantum = [
    "mlkem768x25519-sha256"
    "sntrup761x25519-sha512"
    "sntrup761x25519-sha512@openssh.com"
  ];

  # After them, for the CLIENT: what OpenSSH 10 itself offers — without the
  # SHA-1 ones, which nobody should still need.
  classical = [
    "curve25519-sha256"
    "curve25519-sha256@libssh.org"
    "ecdh-sha2-nistp256"
    "ecdh-sha2-nistp384"
    "ecdh-sha2-nistp521"
    "diffie-hellman-group-exchange-sha256"
    "diffie-hellman-group16-sha512"
    "diffie-hellman-group18-sha512"
    "diffie-hellman-group14-sha256"
  ];

  # After them, for a SERVER: the classical part of the list NixOS recommends
  # for sshd (its default `KexAlgorithms`) — X25519 under both names and a
  # group exchange with SHA-256 — and nothing it left out.
  serverClassical = [
    "curve25519-sha256"
    "curve25519-sha256@libssh.org"
    "diffie-hellman-group-exchange-sha256"
  ];

  # THE HOST KEYS an sshd of ours has: ed25519 alone. RSA is a key nobody
  # here needs, which NixOS would otherwise make on first boot — and which
  # `/persist` used to keep for ever (`no-rsa-host-key`).
  hostKeys = [
    {
      type = "ed25519";
      path = "/etc/ssh/ssh_host_ed25519_key";
    }
  ];
}
