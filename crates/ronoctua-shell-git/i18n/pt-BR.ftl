# The texts `ronoctua-shell-git` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

files-git-title = Histórico de { $name }
# As colunas do navegador.
files-git-kind-changes = Mudanças
files-git-kind-history = Histórico
files-git-kind-refs = Ramos e etiquetas
files-git-kind-stashes = Stashes
files-git-kind-remotes = Remotos
files-git-kind-count = { $kind } { $n }
files-git-file-history-title = Histórico de { $path }
files-git-reading = Lendo…
files-git-empty-changes = Nada mudou na árvore de trabalho.
files-git-empty-history = Ainda não há commits.
files-git-empty-refs = Nenhum ramo e nenhuma etiqueta.
files-git-empty-stashes = Nada guardado no stash.
files-git-empty-remotes = Nenhum remoto.
files-git-empty-file = Nenhum commit mexeu neste arquivo.
files-git-pick-item = Escolha algo à esquerda para ver aqui.
files-git-pick-file = Escolha um arquivo para ver o que mudou nele.
files-git-diff-no-files = Um remoto não muda arquivo nenhum.
files-git-diff-nothing = O conteúdo não mudou.
files-git-diff-staged = Preparado
files-git-diff-unstaged = Não preparado
files-git-diff-untracked = Ainda fora do repositório
files-git-diff-binary = Não é texto: não há o que mostrar linha a linha.
files-git-diff-no-content = Só os detalhes do arquivo mudaram, não o que há nele.
files-git-diff-link = Um link simbólico, e para onde ele aponta:
files-git-diff-special = Não é um arquivo — é um pipe, um socket ou um dispositivo —, então nada nele é lido.
files-git-blame-title = Quem escreveu cada linha de { $path }
files-git-files = { $n ->
        [one] 1 arquivo mudado
       *[other] { $n } arquivos mudados
    }
files-git-remote-branches = Os ramos dele, da última busca
files-git-no-remote-branches = Nenhum buscado ainda.
files-git-fact-path = arquivo
files-git-fact-state = estado
files-git-fact-from = renomeado de
files-git-fact-name = nome
files-git-fact-kind = tipo
files-git-fact-upstream = segue
files-git-fact-fetch = busca de
files-git-fact-push = envia para
files-git-fact-stash = stash
files-git-fact-commit = commit
files-git-fact-author = autoria
files-git-fact-date = data
files-git-fact-parents = pais
files-git-ref-branch = um ramo
files-git-ref-remote = um ramo de remoto
files-git-ref-tag = uma etiqueta
files-git-mark-branch = ramo
files-git-mark-remote = remoto
files-git-mark-tag = etiqueta
files-git-change-added = acrescentado
files-git-change-modified = mudado
files-git-change-deleted = apagado
files-git-change-renamed = renomeado
files-git-change-copied = copiado
files-git-change-type = mudou de tipo
files-git-change-unmerged = não mesclado
files-git-state-untracked = ainda fora do repositório
files-git-state-conflicted = em conflito
files-git-state-staged = preparado: { $what }
files-git-state-unstaged = não preparado: { $what }
# O que o navegador diz depois de uma ação.
files-git-fetched = Buscado: os ramos dos remotos estão em dia.
files-git-fetch-refused = Nada foi buscado. { $why }
files-git-fetching = Já há uma busca em andamento.
files-git-no-file = Não há arquivo sob o cursor.
files-git-blame-gone = O arquivo não está mais na árvore de trabalho, então ninguém escreveu as linhas dele ali.
files-git-nothing-to-copy = Não há commit sob o cursor.
files-git-no-version = Não há versão anterior deste arquivo para abrir.
files-git-kept = Aberto { $path } como { $id } o tem — uma cópia que não pode ser mudada.
files-git-keep-failed = A cópia não pôde ser feita: { $why }
files-git-keep-why-disk = O disco não a aceitou: está cheio, ou o cache não pode ser escrito ali.
# As teclas do navegador: a palavra que a linha de atalhos mostra, e o que o
# ponteiro parado sobre ela ouve.
files-git-down = descer
files-git-up = subir
files-git-enter = abrir
files-git-next = próxima coluna
files-git-previous = coluna anterior
files-git-page-down = página abaixo
files-git-page-up = página acima
files-git-changes = mudanças
files-git-history = histórico
files-git-refs = ramos
files-git-stashes = stashes
files-git-remotes = remotos
files-git-file-history = histórico do arquivo
files-git-blame = quem escreveu
files-git-copy = copiar id
files-git-open-version = abrir esta versão
files-git-fetch = buscar
files-git-terminal = terminal
files-git-left = esquerda
files-git-right = direita
about-files-git-down = Desce o cursor uma linha; no diff, move o texto uma linha.
about-files-git-up = Sobe o cursor uma linha; no diff, move o texto uma linha.
about-files-git-enter = Entra uma coluna: de um item para o que ele é, de um arquivo para o que mudou nele.
about-files-git-next = Dá o teclado à coluna da direita.
about-files-git-previous = Dá o teclado à coluna da esquerda.
about-files-git-page-down = Desce uma tela inteira na coluna que está com o teclado.
about-files-git-page-up = Sobe uma tela inteira na coluna que está com o teclado.
about-files-git-changes = Lista o que a árvore de trabalho tem que o último commit não tem.
about-files-git-history = Lista os commits, do mais novo ao mais antigo.
about-files-git-refs = Lista os ramos, os ramos dos remotos e as etiquetas.
about-files-git-stashes = Lista o que foi guardado no stash.
about-files-git-remotes = Lista os remotos e onde cada um está.
about-files-git-file-history = Lista só os commits que mudaram o arquivo sob o cursor, atravessando as renomeações.
about-files-git-blame = Mostra quem escreveu por último cada linha do arquivo como a árvore de trabalho o tem; a mesma tecla volta ao diff.
about-files-git-copy = Copia o id do commit sob o cursor.
about-files-git-open-version = Abre o arquivo como o commit o tinha, numa cópia que não pode ser mudada.
about-files-git-fetch = Traz para cá os commits novos dos remotos, sem mudar nenhum dos seus. Nunca pede senha.
about-files-git-terminal = Abre um terminal na pasta de cima do repositório.
about-files-git-left = Move o texto do diff para a esquerda.
about-files-git-right = Move o texto do diff para a direita, para ler uma linha longa.
cmd-dev-clone = Clonar o repositório da distro numa pasta
ask-dev-clone = Clonar de { $host } em
dev-clone-cloning = { $host } agora está permitido. Clonando a distro de { $host } em { $folder }…
dev-clone-running = Um clone em { $folder } ainda está rodando.
dev-clone-done = A distro está em { $folder }.
dev-clone-failed = Nada foi clonado em { $folder }: { $why }
dev-clone-occupied = Já há algo em { $folder }: nada foi clonado.
dev-clone-no-address = Esta construção não sabe o endereço da distro, então não há o que clonar.
dev-clone-no-folder = Nenhuma pasta foi dada: nada foi clonado.
dev-clone-not-let = { $host } não pôde ser permitido, então nada foi consultado.
dev-clone-unheard = O rodapé não ouviria a resposta do clone, então ele não começou.
dev-clone-unkept-no-home = Não há pasta pessoal onde guardar o consentimento: nada foi clonado.
dev-clone-unkept-unparseable = O arquivo de consentimento (consent.json) não lê; ficou como estava, e nada foi clonado.
dev-clone-unkept-unwritable = O arquivo de consentimento não pôde ser gravado: nada foi clonado.
dev-clone-elsewhere = Clonar de { $host } (não { $from }) em
dev-clone-unread = O git não conseguiu ler para onde as suas configurações mandam o endereço da distro, então nada foi perguntado.
dev-clone-reading = Lendo para onde as suas configurações do git mandam o endereço da distro…
dev-clone-no-host = A sua configuração do git manda o endereço da distro para { $to }, que não é um host a perguntar: nada foi clonado.
