# The texts `ronoctua-shell-lock` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# =====================================================================
# THE LOCK SCREEN — the footer's two lines over an empty desk
# =====================================================================
cmd-lock-screen = Lock the screen
# THE MODELINE: the lock, and whose session it is. `{ $user }` is the
# account's name — data, as a filename is.
lock-locked = Locked
lock-account = { $user }'s session
# THE MINIBUFFER'S QUESTION, and what its `Esc` does here: the question stays.
ask-lock-password = Password
about-lock-clear = Clears what was typed; the screen stays locked and the password is asked again.
# WHAT THE LOCK SAYS above the two lines: the password with PAM, and the
# three refusals (`ronoctua_pam::Refusal`).
lock-checking = Checking the password…
lock-wrong = Wrong password.
lock-unavailable = The password could not be checked. Try again.
lock-no-service = This machine has no password check for the lock; it cannot open from here.
# THE FINGERPRINT READER, said only where there is one with a finger enrolled.
lock-finger-waiting = touch the reader
lock-finger-missed = the finger was not taken
lock-finger-unavailable = the reader did not answer
lock-finger = the finger again
about-lock-finger = Makes the fingerprint reader wait for a touch again; a key or a click does it too.
