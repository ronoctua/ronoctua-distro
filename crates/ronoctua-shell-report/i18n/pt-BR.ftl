# The texts `ronoctua-shell-report` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

files-no-editor = o Emacs nao esta instalado neste perfil
files-nothing-opens = nenhum aplicativo declara abrir { $type }

## file info editing — everything the info panel can change

# Why an archive's work did not happen (`ronoctua_archive::Why`).
files-archive-why-no-tool = { $name }: esta sessão não recebeu o programa de que este arquivo precisa
files-archive-why-unsupported = { $name }: este formato não faz isso — um tar não tem senha e só muda convertendo
files-archive-why-needs-password = { $name } é cifrado e precisa da senha
files-archive-why-wrong-password = Essa senha não abre { $name }
files-archive-why-corrupt = { $name } está danificado, ou não é um arquivo compactado
files-archive-why-exists = { $name } já existe; nada foi substituído
files-archive-why-unstartable = o programa para { $name } não pôde ser iniciado
files-archive-why-failed = o trabalho em { $name } não terminou
files-archive-why-stopped = o trabalho em { $name } foi parado, e o que ele tinha escrito foi apagado
security-online-vt-key = Chave do VirusTotal
