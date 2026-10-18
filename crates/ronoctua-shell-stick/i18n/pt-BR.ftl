# The texts `ronoctua-shell-stick` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

group-stick = pendrive do instalador
cmd-stick-open = O instalador num pendrive: a versão mais recente e os seus drives
cmd-stick-window = O instalador num pendrive, numa janela
cmd-stick-down = A próxima linha
cmd-stick-up = A linha anterior
cmd-stick-enter = Faz o que a linha diz: permitir, baixar, escolher o drive, responder
cmd-stick-reread = Pergunta de novo ao GitHub a versão mais recente, e relê os drives quando o udisks não respondeu
stick-down = baixo
stick-up = cima
stick-enter = escolher
stick-reread = perguntar de novo
stick-title = Pendrive do instalador
stick-heading-release = O instalador
stick-heading-drives = Drives
stick-heading-write = A última gravação
stick-consent-for = Saber a versão mais recente e baixar o instalador precisa do GitHub.
stick-asking = Perguntando ao GitHub a versão mais recente…
stick-retry = Perguntar de novo
stick-version = Versão
stick-published = Publicada
stick-size = Tamanho
stick-image = Imagem
stick-image-cached = baixada e conferida, no cache
stick-image-absent = ainda não baixada
stick-fetch = Só baixar
stick-fetch-value = a imagem, para o cache, sem gravar drive nenhum
stick-drives-reading = Lendo os drives…
stick-drives-down = O udisks não responde, então os drives não podem ser lidos.
stick-drives-none = Nenhum pendrive nem cartão de memória está conectado.
stick-drive-ready = { $size } · pode ser gravado
stick-drive-small = { $size } · pequeno demais para esta versão
stick-erases = Gravar apaga o drive inteiro. Primeiro vem uma pergunta, e depois a sua senha.
stick-unfit-bus = { $size } · nem USB nem cartão de memória
stick-unfit-system = { $size } · o sistema roda dele
stick-unfit-mounted = { $size } · montado
stick-unfit-open = { $size } · um volume cifrado está aberto
stick-unfit-read-only = { $size } · só leitura
stick-unfit-unnamed = { $size } · sem nome estável
stick-why-bus = Só se grava pendrive ou cartão de memória: este drive está em outro barramento.
stick-why-system = O sistema em execução usa um volume deste drive, então ele nunca é gravado.
stick-why-mounted = Um volume deste drive está montado. Desmonte-o no gerenciador de arquivos e escolha o drive de novo.
stick-why-open = Um volume cifrado deste drive está aberto. Tranque-o no gerenciador de arquivos e escolha o drive de novo.
stick-why-read-only = O drive é só leitura: a trava do pendrive ou do cartão pode estar ligada.
stick-why-unnamed = O drive não tem nome estável em /dev/disk/by-id, então a gravação não poderia ser presa a ele.
stick-small-why = O instalador não cabe neste drive. Escolha um maior.
stick-no-release-yet = A versão mais recente ainda não é conhecida, então não há o que gravar.
stick-sure = Gravar o instalador { $version } em { $drive } ({ $size })?
stick-sure-note = Tudo o que está no drive é apagado, e isso não se desfaz.
stick-no = Não, deixar o drive como está
stick-yes = Sim, apagar e gravar o instalador
stick-state = Estado
stick-phase-waiting = esperando o download terminar
stick-phase-writing = gravando: { $percent }%
stick-phase-verifying = relendo: { $percent }%
stick-phase-done = gravado e relido: o pendrive está pronto
stick-phase-failed = recusada
stick-phase-given-up = desistida
stick-refused-not-let = O GitHub não foi permitido, então nada foi perguntado.
stick-refused-offline = O GitHub não respondeu. Você está conectado?
stick-refused-download = O download não terminou, então nada foi gravado: o GitHub parou de responder, ou o cache ficou sem espaço. A linha da fila diz qual.
stick-refused-no-release = A versão mais recente não tem um instalador que esta vista entenda.
stick-refused-mismatch = O que foi baixado não confere com as checksums da versão, então nada foi gravado.
stick-refused-not-writable = O drive não pôde ser gravado: estava montado, preso, é pequeno demais, ou não é um pendrive.
stick-refused-changed = O drive mudou depois de ser escolhido, então nada foi gravado. Escolha-o de novo.
stick-refused-read-back-differs = O pendrive não guarda o que foi gravado nele. Ele pode estar falhando, ou ser menor do que diz.
stick-refused-failed = A gravação não aconteceu: a fila ou o gravador falhou. A linha da fila, ou o journal de ronoctua-stick-write.service, diz por quê.
stick-about-allow = Permite os três hosts do GitHub, para perguntar e baixar a versão mais recente
stick-about-retry = Pergunta de novo ao GitHub a versão mais recente
stick-about-fetch = Baixa e confere a imagem agora, para uma gravação depois começar na hora
stick-about-drive = Grava o instalador sobre este drive inteiro, depois de uma pergunta
stick-about-unfit = Diz por que este drive não é gravado, e o que fazer
stick-about-no = Fecha a pergunta e não grava nada
stick-about-yes = Inicia o gravador com a sua senha, e então baixa, confere e grava
stick-drive-left = O drive saiu, então a pergunta foi fechada.
stick-queued-fetch = A imagem está baixando na fila.
stick-queued-write = O gravador está esperando: a imagem baixa na fila, e então o drive é gravado e relido.
stick-starting = Iniciando o gravador…
stick-not-let-host = { $host } não está permitido: permita o GitHub nesta vista primeiro.
stick-busy-writing = Um pendrive está sendo gravado agora. Espere ele terminar (a última gravação, abaixo) antes de gravar outro.
stick-busy-waiting = Uma gravação já espera a imagem dela baixar. Desista dela na fila para escolher outro drive.
stick-busy-stopped = Um gravador ainda esperava uma imagem que ninguém baixava; ele foi mandado parar. Escolha o drive de novo daqui a pouco.
stick-no-writer = Esta máquina não tem o gravador de pendrive instalado.
stick-no-cache = Não há pasta de cache para baixar a imagem.
