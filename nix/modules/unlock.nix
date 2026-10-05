# The door lock — `ronoctua-unlock`, the PAM service `ronoctua-pam` asks.
#
# WHAT IT LOCKS: the player's adult catalogue opens only after the person types
# the password of their own system account. The check is `ronoctua_pam::verify`
# in the door's process, through libpam, under THIS service and no other
# (`ronoctua_pam::SERVICE`; `pam-service-is-declared` holds the two names
# together). It is a lock on the doors, not encryption: the page says so.
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
# Beside the player because the player is what asks today; the service's name
# says what it does, not who uses it.
{
  config,
  lib,
  ...
}:

{
  config = lib.mkIf config.ronoctua.player.enable {
    security.pam.services.ronoctua-unlock = {
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

      # NOT A SESSION: nothing is opened, no login recorded, no home mounted.
      startSession = false;
      setLoginUid = false;
      updateWtmp = false;
      pamMount = false;
    };
  };
}
