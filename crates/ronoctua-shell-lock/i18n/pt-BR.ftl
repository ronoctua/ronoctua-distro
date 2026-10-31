# The texts `ronoctua-shell-lock` says, in pt-BR (invariant 8).
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
cmd-lock-screen = Bloquear a tela
# THE MODELINE: the lock, and whose session it is. `{ $user }` is the
# account's name — data, as a filename is.
lock-locked = Bloqueada
lock-account = sessão de { $user }
# THE MINIBUFFER'S QUESTION.
ask-lock-password = Senha
# WHAT THE LOCK SAYS above the two lines: the password with PAM, and the
# three refusals (`ronoctua_pam::Refusal`).
lock-checking = Conferindo a senha…
lock-wrong = Senha errada.
lock-unavailable = Não deu para conferir a senha. Tente de novo.
lock-no-service = Esta máquina não tem como conferir a senha do bloqueio; ele não abre por aqui.
# THE FINGERPRINT READER, said only where there is one with a finger enrolled.
lock-finger-waiting = toque o leitor
lock-finger-missed = o dedo não foi aceito
lock-finger-unavailable = o leitor não respondeu
lock-finger = o dedo de novo
about-lock-finger = Faz o leitor de digitais esperar um toque de novo; uma tecla ou um clique também.
