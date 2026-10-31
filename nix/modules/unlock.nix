# The door lock — `ronoctua-unlock`, the PAM service `ronoctua-pam` asks.
#
# WHAT IT LOCKS: the session, while the lock screen is up (`ronoctua-shell
# lock`, `LEADER s q l`), and the player's adult catalogue — both open only
# after the person types the password of their own system account. The check
# is `ronoctua_pam::verify` in the asking process, through libpam, under THIS
# service and no other (`ronoctua_pam::SERVICE`; `pam-service-is-declared`
# holds the two names together). The catalogue's is a lock on the doors, not
# encryption: the page says so.
#
# DECLARED ON EVERY MACHINE, since the lock screen came (2026-10-31): it used to
# follow the player's switch, and a machine without the player would have had a
# lock screen nothing could open. The lock refuses to lock where the stack is
# missing (`ronoctua_pam::declared`), so a machine that lost it is a machine
# that does not lock, never one that cannot be opened.
#
# AND THE FINGER, beside it (`ronoctua-unlock-finger`, `ronoctua_pam::
# FINGER_SERVICE`): the owner asked the lock screen to take a fingerprint as
# well as the password (2026-10-31). `pam_fprintd` ALONE, `sufficient`, then
# the stack's `pam_deny`: no password in it (that is the other service's), no
# module that could answer for the finger. Only where fprintd is on; the lock
# offers the finger only where a reader and an enrolled finger are, and runs
# this stack on a thread that races the typed password. It is the second
# service `pam_fprintd` is in, beside `sudo` — `pam-fingerprint-is-gated`
# names both.
#
# PASSWORD ONLY. `pam_unix`, which a process that is not root runs through the
# setuid `unix_chkpwd` — and that helper checks the CALLER's own password and
# nobody else's. Every module that could answer for the password without it is
# switched off by name, because its default follows a machine-wide switch: a
# fingerprint reader (`fprintAuth` follows `services.fprintd.enable`), a U2F
# key, a YubiKey OTP, a PKCS#11 token, OATH, OTPW, the SSH agent. Each would
# either let the door open without the password the page asks for, or leave
# the conversation waiting for a touch nobody was told to give. An account
# with an empty password is never let in (`allowNullPassword = false`, and the
# crate passes `PAM_DISALLOW_NULL_AUTHTOK`).
#
# A DELAY AFTER A WRONG ANSWER, `pam_faildelay`: three seconds, the NixOS
# default and what a login waits. The door calls `verify` on a thread, so the
# wait is the person's, never the loop's; what it costs is a guess every three
# seconds instead of thousands.
#
# The service's name says what it does, not who uses it.
{
  config,
  lib,
  ...
}:

let
  # NOT A SESSION, in both stacks: nothing is opened, no login recorded, no
  # home mounted.
  notASession = {
    startSession = false;
    setLoginUid = false;
    updateWtmp = false;
    pamMount = false;
  };
in
{
  config = lib.mkMerge [
    {
      security.pam.services.ronoctua-unlock = notASession // {
        unixAuth = true;
        allowNullPassword = false;
        nodelay = false;
        rootOK = false;
        failDelay = {
          enable = true;
          # Microseconds.
          delay = 3000000;
        };

        fprintAuth = false;
        u2fAuth = false;
        yubicoAuth = false;
        p11Auth = false;
        oathAuth = false;
        otpwAuth = false;
        sshAgentAuth = false;
      };
    }
    (lib.mkIf config.services.fprintd.enable {
      security.pam.services.ronoctua-unlock-finger = notASession // {
        # THE FINGER AND NOTHING ELSE answers here: no password, no key, no
        # token — the password has its own service, typed on its own line.
        fprintAuth = true;
        unixAuth = false;
        rootOK = false;
        u2fAuth = false;
        yubicoAuth = false;
        p11Auth = false;
        oathAuth = false;
        otpwAuth = false;
        sshAgentAuth = false;
      };
    })
  ];
}
