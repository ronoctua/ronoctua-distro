# Security keys (a YubiKey, any FIDO2 key) — the machine side of what the
# belongings' targets carry.
#
# The belongings bring a person's `~/.gnupg`, their SSH keys and, in the `git`
# target, `~/.config/Yubico/u2f_keys` — the security keys they registered on
# their other machines. This module lets those work on every machine:
#
#   pcscd           the smart-card daemon GnuPG's `scdaemon` reaches an
#                   OpenPGP card through (a YubiKey is one);
#   udev rules      the keys' devices for the person at the seat
#                   (`yubikey-personalization`, `libfido2`);
#   gpg-agent       GnuPG's agent, as a user socket;
#   pam_u2f         a key may answer SUDO instead of the password — always
#                   declared, ALWAYS `sufficient` and in sudo ONLY
#                   (`pam-u2f-is-only-sufficient`).
#
# ON AND OFF ARE ROOT'S LIST, NOT A REBUILD (owner, 2026-10-11). `pam_u2f`
# trusts root's list alone (`ronoctua_security::u2f::AUTHFILE`), which only
# the sudo door's `u2f-keys` verb writes; with no line for the person it
# answers nothing and the password asks, as if it were not there. So the
# person registers, removes and turns their keys off on screen, and it holds
# at once. The person's own `~/.config/Yubico/u2f_keys` — any process of
# theirs may write it, and it travels — is never what PAM reads.
#
# SUDO ONLY, NEVER THE LOGIN: every other PAM service is born with `pam_u2f`
# off (the option extended below, as `security.nix` does for the finger),
# and the player's door lock (`ronoctua-unlock`) turns it off by name too.
# NEVER `required`: the password always answers, and a lost key never locks
# anybody out.
#
# ONE ORIGIN ON EVERY MACHINE: a FIDO credential belongs to the relying party
# it was made for, and `pam_u2f` defaults to `pam://<hostname>`. Origin and
# appid are `ronoctua_security::u2f::ORIGIN`, the same the screen's
# registration gives `pamu2fcfg`, so a key registered on one machine answers
# on all of them.
#
# A TERMINAL ONLY: a gate directly before `pam_u2f` jumps over it without a
# tty. The sudo door's conversation gives sudo a terminal of its own, and
# hears the cue below (`ronoctua_security::authority::TOUCH_CUE`) as "touch
# your key"; its password-only run has no terminal, and is never made to
# wait for a touch.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.yubikey;
  sudoRules = config.security.pam.services.sudo.rules.auth;
in
{
  options.ronoctua.yubikey = {
    enable =
      lib.mkEnableOption "security keys: smart cards, GnuPG's agent, and pam_u2f for sudo (root's list of the person's keys)"
      // {
        default = true;
      };
  };

  # NO SERVICE BUT SUDO ASKS A KEY. nixpkgs' per-service `u2f.enable` follows
  # `security.pam.u2f.enable`, which turns it on in every service at once;
  # this extends the option so each service is born with it off, and only
  # `sudo` below turns it back on. Declared outside `config` because an
  # option cannot be conditional; with the module off, `pam_u2f` is off
  # everywhere anyway.
  options.security.pam.services = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submodule {
        config.u2f.enable = lib.mkDefault false;
      }
    );
  };

  config = lib.mkIf cfg.enable {
    services.pcscd.enable = true;
    services.udev.packages = [
      pkgs.yubikey-personalization
      pkgs.libfido2
    ];
    # THE GROUP THOSE RULES NAME. `libfido2`'s stock rules assign every
    # security key `GROUP="plugdev"` (a Debian/Arch convention), a group NixOS
    # does not define — so udev-worker says "Failed to resolve group
    # 'plugdev'" for each of the hundred-odd rule lines, on every boot and
    # every trigger (a thousand lines a boot). The access they grant comes
    # from the `uaccess` tag they also carry, which hands the key to whoever
    # holds the seat; the group is vestigial. Defined empty, the clause
    # resolves and the noise stops, and no one is in a group that grants
    # nothing.
    users.groups.plugdev = { };
    programs.gnupg.agent.enable = true;

    security.pam.u2f = {
      enable = true;
      # NEVER `required`: the password always answers too.
      control = "sufficient";
      settings = {
        # Said, never a silent wait for a touch — and said in the words the
        # sudo door listens for (`ronoctua_security::authority::TOUCH_CUE`).
        cue = true;
        cue_prompt = "Touch your security key (ronoctua-sudo:touch)";
        # ROOT'S LIST, never the person's `~/.config/Yubico/u2f_keys`
        # (`ronoctua_security::u2f::AUTHFILE`). With no line for the person,
        # `pam_u2f` answers nothing.
        authfile = "/var/lib/ronoctua-security/u2f_keys";
        # ONE RELYING PARTY on every machine (`ronoctua_security::u2f::ORIGIN`).
        origin = "pam://ronoctua";
        appid = "pam://ronoctua";
      };
    };

    security.pam.services.sudo = {
      u2f.enable = true;
      # ONLY IN A TERMINAL: the line directly before `pam_u2f` jumps over it
      # when there is none (the door's password-only `sudo -S`).
      rules.auth.ronoctua-u2f-tty = {
        order = sudoRules.u2f.order - 1;
        control = "[success=ignore default=1]";
        modulePath = "${config.security.pam.package}/lib/security/pam_succeed_if.so";
        args = [
          "quiet"
          "tty"
          "=~"
          "/dev/*"
        ];
      };
    };
  };
}
