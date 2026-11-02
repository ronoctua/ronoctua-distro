# WHERE THE TPM'S SIGNING KEY AND ITS SWITCH ARE — the Nix side's one place,
# read by the module that runs the agent (`nix/modules/tpm-signing.nix`) and by
# git's signer (`nix/modules/ssh.nix`). The Rust side names the same paths in
# `ronoctua_security::signing`, which the root helper and the security watch
# read them by.
rec {
  # Root's folder for the key, inside the security centre's own (persisted).
  keyDir = "/var/lib/ronoctua-security/tpm-key";
  # The TPM-wrapped private half, usable only through this machine's TPM.
  sealedKey = "${keyDir}/id_ecdsa.tpm";
  # Its public half, readable by all.
  publicKey = "${keyDir}/id_ecdsa.pub";
  # THE SWITCH: present, the TPM's key signs; absent, only the security key.
  allowed = "/var/lib/ronoctua-security/tpm-signing";
  # Where the agent listens, the person's alone.
  socket = "/run/ronoctua-tpm-agent/agent.sock";
  # The units, by name.
  agentUnit = "ronoctua-tpm-agent";
  keygenUnit = "ronoctua-tpm-keygen";
}
