# THE BELONGINGS' HTTPS, POST-QUANTUM ONLY: the OpenSSL configuration git
# runs under (`OPENSSL_CONF`) when the belongings reach their repository with a
# token (`ronoctua_git::Tools::pq_tls`, `RONOCTUA_GIT_PQ_TLS`).
#
# git speaks HTTPS through libcurl over OpenSSL, and has no setting for the
# key-exchange groups. OpenSSL has one place for them that reaches inside
# another program: every TLS context a process makes starts from the
# `system_default` section of the file `OPENSSL_CONF` names. This one lists
# the hybrid ML-KEM groups alone, and TLS 1.3 and up, below which the hybrids
# do not exist. MEASURED on the pinned git: a server with X25519 alone, the
# NIST curves alone, or TLS 1.2 alone answers `alert handshake failure`; one
# with `X25519MLKEM768` or `SecP384r1MLKEM1024` is reached.
#
# OPENSSL IGNORES A CONFIGURATION IT CANNOT USE, WITHOUT A WORD — a missing
# file, a relative path, a group this version has no name for (all three
# measured) — and then every server is reached, classically. So the crate
# refuses a post-quantum-only token when the file is not there, and
# `belongings-https-is-pq-only` runs the pinned git under THIS file against
# servers with classical groups only and shows each one refused.
{ pkgs }:
let
  # In order of preference, by OpenSSL's names (3.5 on): ML-KEM-768 with
  # X25519 (what browsers and most hosts offer), then with P-256, then
  # ML-KEM-1024 with P-384.
  groups = [
    "X25519MLKEM768"
    "SecP256r1MLKEM768"
    "SecP384r1MLKEM1024"
  ];
in
pkgs.writeText "ronoctua-pq-only-tls.cnf" ''
  openssl_conf = openssl_init

  [openssl_init]
  ssl_conf = ssl_module

  [ssl_module]
  system_default = post_quantum_only

  [post_quantum_only]
  MinProtocol = TLSv1.3
  Groups = ${pkgs.lib.concatStringsSep ":" groups}
''
