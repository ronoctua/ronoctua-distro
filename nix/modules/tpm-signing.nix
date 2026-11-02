# COMMITS SIGNED BY THE TPM (owner, 2026-11-02) — a key this machine's TPM
# holds, which may sign the person's commits in place of their security key,
# behind a switch only root moves.
#
# A security key asks for a touch, and a person away from the machine — over
# SSH, or an agent working for them — cannot give one. A key made INSIDE the
# TPM signs with no touch and never leaves the machine. So, while the person
# allows it, git's signer (`nix/modules/ssh.nix`) signs with it; while they do
# not, only the security key signs. `ronoctua_security::signing` says why
# each piece is so; here is what makes the switch a real one:
#
#   THE TPM IS NOT THE PERSON'S. `/dev/tpmrm0` belongs to the group `tss`
#   (`security.tpm2`), and only the agent's own user is in it — no process
#   of the person's can use the key by itself, whatever it runs.
#
#   THE KEY IS MADE ONCE, by `ronoctua-tpm-keygen`, as the agent, into
#   root's folder; the sudo door starts it (`tpm-key-create`).
#
#   THE AGENT EXISTS ONLY WHILE THE SWITCH DOES. `ssh-tpm-agent` signs with
#   the key on a socket only the person may open (`SocketUser`, `0600`); the
#   socket and the agent carry the switch's file as a condition, and the
#   sudo door writes or takes it away and starts or stops them
#   (`tpm-signing on|off`). A boot keeps the choice.
#
# NO PIN on the key: the person away has nobody to type one into the bar.
# What stands for it is the switch, which only the administrator's password
# moves.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.tpmSigning;
  places = import ../tpm-signing.nix;
  user = config.ronoctua.user;
  # THE AGENT'S OWN USER, the one in `tss` besides root.
  agentUser = "ronoctua-tpm-agent";
  agent = pkgs.ssh-tpm-agent;
  device = "/dev/tpmrm0";

  # What both the socket and the agent need to exist at all: the TPM, the
  # key, and the person's yes.
  conditions = [
    device
    places.sealedKey
    places.allowed
  ];

  # Every unit here touches the TPM and its folder, and nothing else.
  hardening = {
    User = agentUser;
    Group = agentUser;
    ProtectSystem = "strict";
    ProtectHome = true;
    PrivateTmp = true;
    PrivateNetwork = true;
    NoNewPrivileges = true;
    DevicePolicy = "closed";
    DeviceAllow = [ "${device} rw" ];
    RestrictAddressFamilies = [ "AF_UNIX" ];
    CapabilityBoundingSet = [ "" ];
  };

  # THE KEY, ONCE. Never over one that is there: a key may already be
  # registered where the person's commits are checked, and a new one would
  # orphan it. The comment names the person and the machine, read at run
  # time. stdin is `/dev/null`, so the passphrase `ssh-tpm-keygen` asks for
  # twice reads empty both times: no PIN.
  keygen = pkgs.writeShellScript "ronoctua-tpm-keygen" ''
    set -eu
    if [ -e ${places.sealedKey} ] || [ -e ${places.publicKey} ]; then
      echo "a TPM signing key is there already" >&2
      exit 1
    fi
    host=$(${pkgs.coreutils}/bin/cat /proc/sys/kernel/hostname)
    ${agent}/bin/ssh-tpm-keygen -t ecdsa -b 256 \
      -C "${user}@$host-tpm" -f ${places.keyDir}/id_ecdsa </dev/null
  '';
in
{
  options.ronoctua.tpmSigning.enable =
    lib.mkEnableOption "a TPM key that may sign the person's commits in place of their security key, behind a switch only root moves"
    // {
      default = true;
    };

  config = lib.mkIf cfg.enable {
    # `/dev/tpmrm0` to the group `tss`, `0660`: root and the agent.
    security.tpm2.enable = true;

    users.users.${agentUser} = {
      isSystemUser = true;
      group = agentUser;
      extraGroups = [ "tss" ];
      description = "The TPM signing agent";
    };
    users.groups.${agentUser} = { };

    # The key's folder: the agent's, readable by all (the public half is
    # what the person registers), inside the security centre's persisted one.
    systemd.tmpfiles.rules = [
      "d ${places.keyDir} 0755 ${agentUser} ${agentUser} -"
    ];

    systemd.services.${places.keygenUnit} = {
      description = "Makes the TPM key that may sign the person's commits";
      # ONE GESTURE OF THE PERSON per start, never restarted by itself.
      startLimitIntervalSec = 0;
      unitConfig = {
        ConditionPathExists = [ device ];
        RequiresMountsFor = [ places.keyDir ];
      };
      serviceConfig = hardening // {
        Type = "oneshot";
        ExecStart = keygen;
        ReadWritePaths = [ places.keyDir ];
        UMask = "0022";
        TimeoutStartSec = "2min";
      };
    };

    systemd.sockets.${places.agentUnit} = {
      description = "The TPM signing agent's socket, the person's alone, while the TPM may sign";
      wantedBy = [ "sockets.target" ];
      # STARTED BY THE PERSON'S GESTURE, through the sudo door.
      startLimitIntervalSec = 0;
      unitConfig = {
        ConditionPathExists = conditions;
        RequiresMountsFor = [ places.keyDir ];
      };
      socketConfig = {
        ListenStream = places.socket;
        SocketUser = user;
        SocketMode = "0600";
        DirectoryMode = "0755";
      };
    };

    systemd.services.${places.agentUnit} = {
      description = "Signs with the TPM's key for the person, while they allow it";
      requires = [ "${places.agentUnit}.socket" ];
      after = [ "${places.agentUnit}.socket" ];
      unitConfig.ConditionPathExists = conditions;
      serviceConfig = hardening // {
        # The socket comes from systemd (`LISTEN_FDS`); `-l` names it all the
        # same, as the agent wants one.
        ExecStart = "${agent}/bin/ssh-tpm-agent -l ${places.socket} --key-dir ${places.keyDir}";
        # Its own session keyring, where it would cache a PIN.
        KeyringMode = "private";
      };
    };
  };
}
