# The texts `ronoctua-shell-history-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

echo-atuin-synced = O histórico foi sincronizado
echo-atuin-refused = o atuin disse: { $reason }
echo-atuin-logged-out = Saiu, e a chave foi esquecida
atuin-word-auto = como o atuin decidir
atuin-word-compact = compacta
atuin-word-directory = esta pasta
atuin-word-emacs = emacs
atuin-word-full = cheia
atuin-word-fulltext = em qualquer lugar da linha
atuin-word-fuzzy = aproximada
atuin-word-global = tudo
atuin-word-host = esta máquina
atuin-word-prefix = o começo da linha
atuin-word-return-original = o que você tinha digitado
atuin-word-return-query = o que você buscou
atuin-word-session = esta sessão
atuin-word-skim = skim
atuin-word-subl = sublime
atuin-word-vim-insert = vim, inserção
atuin-word-vim-normal = vim, normal
atuin-word-workspace = este projeto
