# The texts `ronoctua-shell-git-line` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

files-copied = copiado
files-not-copied = não consegui copiar

## file info editing — everything the info panel can change

# A LINHA DO GIT sob o caminho do gerenciador de arquivos, e o navegador git
# que o `C-h` abre (`git_line.rs`, `git_browser.rs`). O que vem entre aspas é
# dado: um ramo, um assunto, um caminho.
files-git = histórico
about-files-git = Abre o histórico do repositório: as mudanças, os commits, os ramos, os stashes e os remotos, ao lado do que cada um mudou.
files-git-detached = solto em { $id }
files-git-unborn = { $branch }, ainda sem commits
files-git-changed = { $n ->
        [one] 1 mudado
       *[other] { $n } mudados
    }
files-git-staged = { $n ->
        [one] 1 preparado
       *[other] { $n } preparados
    }
files-git-untracked = { $n ->
        [one] 1 não rastreado
       *[other] { $n } não rastreados
    }
files-git-conflicted = { $n ->
        [one] 1 em conflito
       *[other] { $n } em conflito
    }
files-git-clean = nada mudou
files-git-list-item = { $item },
files-git-subject = “{ $subject }”
files-git-line-explain = Em { $branch }: { $changed } mudados, { $staged } preparados, { $untracked } não rastreados. { $keys } ou um clique abre o histórico.
files-git-line-refused = Esta pasta está num repositório que o git não quis ler. { $keys } abre o histórico, que diz por quê.
files-git-age-now = agora mesmo
files-git-age-minutes = { $n ->
        [one] há um minuto
       *[other] há { $n } minutos
    }
files-git-age-hours = { $n ->
        [one] há uma hora
       *[other] há { $n } horas
    }
files-git-age-days = { $n ->
        [one] ontem
       *[other] há { $n } dias
    }
files-git-age-months = { $n ->
        [one] há um mês
       *[other] há { $n } meses
    }
files-git-age-years = { $n ->
        [one] há um ano
       *[other] há { $n } anos
    }
# Por que o git não respondeu (`ronoctua_git::Why`), uma por motivo.
files-git-why-no-git = Esta sessão não recebeu o git, então o repositório não pode ser lido.
files-git-why-no-ssh = Este remoto é alcançado por SSH e esta sessão não recebeu o ssh.
files-git-why-no-askpass = Este remoto pede um token e falta o ajudante que o entrega.
files-git-why-unstartable = O git não pôde ser iniciado.
files-git-why-failed = O git não conseguiu ler isto.
files-git-why-garbled = O git respondeu algo que este navegador não entende.
files-git-why-needs-credentials = O remoto pede um nome e uma senha ou uma chave, e buscar nunca pede nenhum dos dois.
files-git-why-refused = O remoto recusou as credenciais que recebeu.
files-git-why-unreachable = O remoto não pôde ser alcançado.
files-git-why-too-large = É maior do que uma vista mostra.
files-git-why-not-post-quantum = O servidor do remoto não ofereceu troca de chaves pós-quântica, e esta conexão exige uma: nada foi enviado.
files-git-why-no-common-key-exchange = O servidor do remoto só oferece trocas de chaves antigas, que esta máquina não aceita mais: ele foi alcançado, e nada foi enviado.
