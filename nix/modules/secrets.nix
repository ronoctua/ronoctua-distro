# Secrets — section 7.11.
#
# "sops-nix, adopted in milestone 3, is **only for non-personal, shareable
# secrets**. Chosen over agenix for supporting age + PGP + KMS and for
# encrypting the file partially. Derive the age key from the host's SSH key
# (`ssh-to-age`). Personal data does not belong here — section 9."
#
# THE LINE THAT IS NOT CROSSED
# ----------------------------
# Section 9 lists what may NOT exist in the repository, not even encrypted:
# password, password hash, passphrase, private SSH key, wifi PSK, token,
# e-mail, real name.
#
# "But it is encrypted" is not an argument. The repository is publishable
# (section 9), and a published encrypted secret is a secret with an expiry
# date — today's cipher is tomorrow's exercise. What is personal goes to
# /persist, generated at install time, outside git.
#
# What DOES belong here: a secret shareable between the machines of whoever
# installs the distro and that identifies nobody. A binary cache deploy key,
# for example.
{
  config,
  lib,
  pkgs,
  inputs,
  ...
}:

let
  cfg = config.ronoctua;

  # WHERE THE SHARED SECRETS ARE: the machine's own flake first — it is what an
  # installed machine builds from, and the distro's tree is not the machine's
  # (`nix/machine.nix`) — and the distro's `secrets/` for a system built
  # without one (the VM). Neither exists in the public repository.
  source = cfg.machine.source or null;
  ownFile = if source == null then null else source + "/${(import ../machine.nix).parts.secrets}";
  secretsFile =
    if ownFile != null && builtins.pathExists ownFile then ownFile else ../../secrets/secrets.yaml;
in
{
  imports = [ inputs.sops-nix.nixosModules.sops ];

  options.ronoctua.secrets.enable = lib.mkOption {
    type = lib.types.bool;
    # Enabled only when there is something to decrypt. An empty secrets.yaml would
    # make sops-nix fail at boot, which is worse than having no secret at all.
    default = builtins.pathExists secretsFile;
    defaultText = lib.literalExpression "secrets.yaml exists in the machine's flake (or, without one, in secrets/)";
    description = ''
      Switches sops-nix on. It switches itself on when the machine's own
      flake carries a `secrets.yaml` — or, for a system built without one,
      when the distro's `secrets/secrets.yaml` exists.
    '';
  };

  config = lib.mkMerge [
    {
      # `ssh-to-age` and `sops` available: these are the two tools section
      # 7.11's flow requires, and leaving them out would force a manual
      # step — which section 8 says to avoid.
      environment.systemPackages = with pkgs; [
        sops
        ssh-to-age
        age
      ];
    }

    (lib.mkIf cfg.secrets.enable {
      sops = {
        defaultSopsFile = secretsFile;
        # The age key is DERIVED from the host's SSH key (section 7.11). There is
        # no second key to keep, and the host's identity is already what
        # authorises decryption.
        age.sshKeyPaths = [ "/persist/etc/ssh/ssh_host_ed25519_key" ];
        # We do not generate a new key: if the host's does not exist, something is
        # wrong with /persist and failing loudly is better than decrypting with an
        # improvised key.
        age.generateKey = false;
      };

      assertions = [
        {
          assertion = config.ronoctua.persist.enable;
          message = ''
            ronoctua.secrets requires ronoctua.persist: the age key derives from the
            host's SSH key, which lives in /persist (section 7.11).
          '';
        }
      ];
    })
  ];
}
