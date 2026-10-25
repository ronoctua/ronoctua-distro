# The texts `ronoctua-shell-update` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

brain-needs-root = Roda como root
# THE UPDATE, WATCHED — the modeline's mark, the leader's group for the view
# that watches a run, and the countdown's two answers that view teaches too.
#
# The mark shows only while something runs and goes on its own. The update's
# commands, its steps and the view's own words are its brain module's texts,
# in that module's folder.
group-update = atualização

## The machine's life (`ronoctua-machine`): why an apply, a migration, a bless or a hold did not do what it was asked (`machine-refused-<why>`).

machine-refused-not-root = Só root muda a máquina: nada foi feito.
machine-refused-busy = Outra mudança nesta máquina está em andamento: tente de novo quando ela acabar.
machine-refused-nothing-to-migrate = Esta máquina já tem o próprio flake: não há o que migrar.
machine-refused-build-failed = O sistema novo não foi construído. Nada mudou: a máquina roda o que rodava, e a pasta dela volta a dizer isso. Um download que ele não conseguiu baixar aparece na página das partes, com os jeitos de trazê-lo.
machine-refused-switch-failed = A troca para o sistema novo falhou. A próxima partida o tenta uma vez, e volta ao último sistema bom se ele não subir.
machine-refused-did-not-boot = Este é o sistema que não subiu quando foi tentado, ou que foi desfeito. A máquina fica no que roda até chegar uma revisão mais nova da distro.
machine-refused-last-good-would-fall-out = Mais um sistema tiraria o último bom do menu de boot. Reinicie antes, para o sistema em teste ser confirmado.
machine-refused-held = A máquina está retida numa revisão diferente da que a pasta dela nomeia: nada foi construído.
machine-refused-unreadable = Um arquivo da pasta da máquina não pode ser lido.
machine-refused-failed = Um programa de que a máquina precisava falhou; o diário diz qual.

## How the boot trial stands (`machine-trial-<outcome>`).

machine-trial-pending = O sistema novo sobe pela primeira vez no próximo reinício. Se ele não subir, a máquina volta sozinha ao último sistema bom.
machine-trial-good = O sistema novo subiu bem: agora ele é o último bom.
machine-trial-failed = O sistema novo não subiu, ou foi desfeito: a máquina está de volta no último sistema bom.
machine-trial-boot-identical = O sistema novo sobe exatamente como o último bom: não precisa de reinício para merecer confiança.

## Why the new system has no boot trial, until it has started once (`machine-no-trial-<why>`).

machine-no-trial-no-efi-variables = As variáveis de boot desta máquina não podem ser escritas: o sistema novo não tem volta além do menu de boot.
machine-no-trial-not-in-menu = O menu de boot não nomeia o sistema novo ou o último bom de um jeito que o teste possa usar: o sistema novo não tem volta além do menu de boot.
machine-no-trial-no-last-good = Ainda não se sabe de nenhum sistema que tenha subido bem nesta máquina: o sistema novo não tem volta além do menu de boot.
# O CACHE BINÁRIO DA PESSOA — uma página da vista da atualização (`c`, `LEADER s u c`),
# nas duas portas, e o que a instalação e o netrc do root dizem dele.
binary-cache-open = cache binário
cmd-binary-cache-open = O seu cache binário: o seu próprio cache no Cachix, do qual esta máquina baixa e para o qual envia depois de cada atualização
cmd-binary-cache-name = Nomeia o seu cache binário — o nome, depois a chave pública
cmd-binary-cache-key = Dá ao seu cache binário a chave pública, exatamente como o cachix.org a mostra
cmd-binary-cache-token = Dá ao seu cache binário o token de envio, guardado cifrado com os seus pertences
cmd-binary-cache-netrc = Deixa esta máquina ler o seu cache binário privado: o netrc do root, gravado pelo sudo
ask-binary-cache-name = Nome do seu cache no Cachix
ask-binary-cache-key = Chave pública de { $name }
binary-cache-heading = O seu cache binário
binary-cache-row-cache = cache
binary-cache-row-key = chave pública
binary-cache-row-visibility = quem lê
binary-cache-row-token = token de envio
binary-cache-row-access = esta máquina lê
binary-cache-row-machine = nesta máquina
binary-cache-row-last = último envio
binary-cache-row-forget-token = esquecer o token de envio nesta máquina
binary-cache-row-remove = deixar de usar este cache
binary-cache-about-cache = O seu próprio cache no Cachix — nunca o da distro, que não tem um. Esta máquina baixa dele antes do cache.nixos.org, e envia para ele depois de cada atualização que a trocou. A escolha viaja com os seus pertences para as suas outras máquinas. Escolhê-la pede o nome, depois a chave.
binary-cache-about-key = A chave de assinatura do cache, exatamente como o cachix.org a mostra na página dele: cada download dele é conferido contra esta chave. Escolhê-la a pede.
binary-cache-about-visibility = Um cache público recebe só as saídas da própria distro — nunca um caminho seu, nunca um programa não livre; qualquer um pode baixá-las. Um cache privado recebe o sistema inteiro desta máquina, e lê-lo exige o seu token. Escolhê-la alterna entre os dois.
binary-cache-about-token = O token que o Cachix dá para enviar. Fica cifrado nos seus pertences, legível só por você, e nunca aparece, nunca vai numa linha de comando nem num log. Escolhê-la pede um novo.
binary-cache-about-access = Um cache privado é lido pelo Nix desta máquina por um netrc que só o root lê, com o seu token. Ele é gravado pelo sudo quando você dá o token ou torna o cache privado; escolhê-la o grava de novo.
binary-cache-about-machine = O que esta máquina usa agora. Um cache nomeado aqui chega a esta máquina na próxima atualização, quando a cópia assinada dos seus pertences é aplicada.
binary-cache-about-last = O que o último envio levou, ou por que foi recusado. Um envio segue cada atualização que trocou esta máquina, uma vez.
binary-cache-about-forget-token = O token sai desta máquina e dos seus pertences na próxima sincronização; nada mais é enviado até você dar um novo.
binary-cache-about-remove = Esta máquina deixa de baixar do cache e de enviar para ele na próxima atualização. O cache em si, no Cachix, fica como está.
binary-cache-none = nenhum
binary-cache-key-none = nenhuma
binary-cache-public = público: as saídas da própria distro, para qualquer um
binary-cache-private = privado: o sistema desta máquina, lido com o seu token
binary-cache-token-none = nenhum: nada é enviado
binary-cache-token-kept = guardado, cifrado com os seus pertences
binary-cache-token-refused = não usado
binary-cache-copy-unreadable = não lê
binary-cache-copy-newer = de um formato mais novo
binary-cache-down = próxima linha
about-binary-cache-down = Põe o cursor na próxima linha da página do cache binário.
binary-cache-up = linha anterior
about-binary-cache-up = Põe o cursor na linha anterior da página do cache binário.
binary-cache-enter = mudar
about-binary-cache-enter = Faz o que a linha sob o cursor diz: pede um nome, uma chave ou o token, alterna quem lê o cache, grava o netrc do root, ou pergunta antes de um gesto que não volta.
binary-cache-steps = os passos
about-binary-cache-steps = De volta aos passos da atualização.
binary-cache-access-written = sim, pelo netrc do root
binary-cache-access-missing = não: o netrc do root não está gravado
binary-cache-machine-in-use = em uso, antes do cache.nixos.org
binary-cache-machine-next = a partir da próxima atualização
binary-cache-machine-leaving = ainda o de antes, até a próxima atualização
binary-cache-machine-none = nenhum cache
binary-cache-last-never = nunca
binary-cache-last-pushed = { $when }: { $paths ->
        [one] um caminho
       *[other] { $paths } caminhos
    } ({ $size })
binary-cache-last-refused = { $when }: nada enviado
binary-cache-why-no-cache = Esta máquina não nomeia cache binário.
binary-cache-why-no-token = Não há token de envio nesta máquina.
binary-cache-why-token-open = O arquivo do token é legível por outra pessoa, então não é usado: dê o token de novo.
binary-cache-why-not-a-token = Isso não é um token do Cachix.
binary-cache-why-not-let = Um host que o envio alcança não está permitido no seu consentimento: nomear o cache de novo os permite.
binary-cache-why-nothing-new = Nenhuma atualização trocou esta máquina desde o último envio: não havia nada novo para enviar.
binary-cache-why-no-tools = Esta máquina não tem cachix ou nix onde o envio os procura.
binary-cache-why-store-failed = O Nix não respondeu sobre o store.
binary-cache-why-cachix-failed = O Cachix recusou o envio, ou ele não terminou: a saída da atualização diz por quê.
binary-cache-why-unwritable = O arquivo não pôde ser gravado.
binary-cache-why-newer = O seu arquivo de opções é de um formato mais novo do que esta máquina lê: atualize esta máquina primeiro.
binary-cache-why-unreadable = O seu arquivo de opções não lê, então não é regravado.
binary-cache-confirm-forget = Esquecer o token de envio nesta máquina? Ele sai dos seus pertences também, e nada é enviado até você dar um novo.
binary-cache-confirm-remove = Deixar de usar este cache binário nas suas máquinas? Cada uma deixa de baixar dele e de enviar para ele na próxima atualização.
binary-cache-answer-no = não, manter
binary-cache-answer-yes-forget = sim, esquecer o token
binary-cache-answer-yes-remove = sim, deixar de usar
binary-cache-said-not-a-name = Um nome do Cachix é letras minúsculas, dígitos e hífens, começando por letra ou dígito.
binary-cache-said-not-the-key = Essa chave não é deste cache: ela é o nome, .cachix.org-1: e 44 caracteres, como o cachix.org a mostra.
binary-cache-said-not-let = O cache foi nomeado, e o consentimento para os hosts dele não pôde ser guardado: nada será enviado até ser.
binary-cache-said-named = O cache é { $name }; esta máquina o usa a partir da próxima atualização.
binary-cache-said-public = O cache é público: recebe só as saídas da própria distro.
binary-cache-said-private = O cache é privado: recebe o sistema desta máquina, lido com o seu token.
binary-cache-said-token = O token de envio está guardado, cifrado com os seus pertences.
binary-cache-said-forgotten = O token de envio foi esquecido nesta máquina.
binary-cache-said-removed = Nenhum cache binário mais, a partir da próxima atualização.
binary-cache-said-netrc-written = Esta máquina lê o seu cache privado agora: o netrc do root está gravado.
binary-cache-said-netrc-removed = O netrc do root para o cache se foi: nada guarda o seu token lá mais.
binary-cache-said-netrc-footer = O netrc do root é pedido na barra do sudo, acima da modeline, se o sudo precisar de você.
# OS APPS DA DISTRO QUE SEGUEM AS PRÓPRIAS VERSÕES — uma página da vista da
# atualização (`f`, `LEADER s u f`), nas duas portas: a opção da pessoa de
# deixar a atualização de rotina levá-los além das versões que a distro testou.
distro-follow-open = seguir versões
cmd-distro-follow-open = Os apps da distro que seguem as próprias versões: se cada atualização os leva além das versões que a distro testou
distro-follow-heading = Os apps da distro que seguem as próprias versões
distro-follow-row-follow = seguir as versões
distro-follow-row-hosts = pergunta a
distro-follow-row-machine = nesta máquina
distro-follow-about-follow = Ligado, cada atualização pergunta ao GitHub pela versão mais nova de cada app abaixo — uma versão que os testes da distro nunca construíram — e, depois que a verificação da própria atualização a construiu, a põe nos seus programas, no lugar da da distro. A que não constrói fica retida e é tentada de novo em toda atualização; enquanto isso, fica a última que construiu, ou a da distro. Quando a distro leva a própria versão além da sua, a da distro volta. Desligado, as versões da distro voltam na próxima atualização. A escolha viaja assinada com os seus pertences para as suas outras máquinas; uma cuja distro é mais velha que a desta recusa o seu arquivo de opções até a distro dela ser atualizada. Escolhê-la alterna.
distro-follow-about-hosts = Seguir pergunta a estes hosts, como todo download pergunta a você antes: ligar os permite. Sem eles, a atualização diz isso, e os apps ficam nas versões que têm.
distro-follow-about-machine = Com o que esta máquina foi construída: uma mudança aqui chega a ela na próxima atualização. A sua escolha e as versões que você segue chegam a uma máquina só assinadas com a sua chave — a que não confia em nenhuma chave sua, ou ainda não recebeu a sua assinatura, fica com as da distro.
distro-follow-about-app = A versão que esta máquina roda, e de quem ela é: a da distro, como os testes dela a construíram, ou a mais nova que você segue, nos seus próprios programas. Uma mais nova só é usada depois que a verificação da atualização a construiu; a que não construiu é tentada de novo em toda atualização.
distro-follow-on = ligado
distro-follow-off = desligado
distro-follow-copy-unreadable = não lê
distro-follow-copy-newer = de um formato mais novo
distro-follow-hosts-let = { $hosts }: permitidos
distro-follow-hosts-not-let = { $hosts }: não permitidos
distro-follow-machine-in-use = segue as versões
distro-follow-machine-next = a partir da próxima atualização
distro-follow-machine-leaving = ainda segue, até a próxima atualização
distro-follow-machine-none = as versões da distro
distro-follow-app-distro = { $version }, da distro
distro-follow-app-followed = { $version }, seguida
distro-follow-app-elsewhere = não está nesta máquina
distro-follow-app-followed-but = { $version }, seguida: { $note }
distro-follow-app-distro-but = { $version }, da distro: { $note }
distro-follow-note-arriving = a { $followed } está a caminho: a página das partes diz como ela está
distro-follow-note-missing = a { $followed } não pôde ser baixada: a página das partes a traz de outro jeito
distro-follow-note-failed = a { $followed } não montou: a página das partes diz por quê
distro-follow-note-refused-id = a versão seguida não nomeia um app que segue as próprias versões
distro-follow-note-refused-url = a versão seguida não é uma versão do repositório do próprio app
distro-follow-note-refused-asset = a versão seguida é outro arquivo da versão dela
distro-follow-note-refused-version = a versão seguida não é a da tag dela
distro-follow-note-refused-hash = a versão seguida não tem resumo SHA-256
distro-follow-note-refused-duplicate = uma segunda versão seguida dele ficou de fora
distro-follow-machine-no-key = não vale aqui: esta máquina não confia em nenhuma chave sua
distro-follow-machine-unsigned = espera a sua assinatura
distro-follow-machine-unreadable = as suas versões seguidas não leem aqui
distro-follow-why-newer = O seu arquivo de opções é de um formato mais novo do que esta máquina lê: atualize esta máquina primeiro.
distro-follow-why-unreadable = O seu arquivo de opções não lê, então não é regravado.
distro-follow-why-unwritable = O arquivo não pôde ser gravado.
distro-follow-none = Nenhum app da distro nesta máquina segue as próprias versões.
distro-follow-said-on = Os apps da distro seguem as próprias versões a partir da próxima atualização.
distro-follow-said-off = As versões da distro voltam na próxima atualização.
distro-follow-said-not-let = Seguir está ligado, e permitir o GitHub no seu consentimento falhou: a atualização vai dizer que não estava permitido.
distro-follow-down = próxima linha
about-distro-follow-down = Põe o cursor na próxima linha da página dos apps que seguem as próprias versões.
distro-follow-up = linha anterior
about-distro-follow-up = Põe o cursor na linha anterior da página dos apps que seguem as próprias versões.
distro-follow-enter = mudar
about-distro-follow-enter = Liga ou desliga o seguir, na linha dele.
distro-follow-steps = os passos
about-distro-follow-steps = De volta aos passos da atualização.
distro-parts-open = partes
cmd-distro-parts-open = As partes da máquina: o que chega depois da instalação, cada uma por conta própria, e os outros jeitos de trazer um download que continua falhando
cmd-distro-parts-hand-in = Entrega um arquivo que você baixou em outro lugar, para a parte ou o sistema sob o cursor: ele só é aceito se for exatamente aquele download
cmd-distro-parts-import = Traz de um pendrive os downloads que faltam às partes e ao sistema
cmd-distro-parts-export = Grava num pendrive os downloads que esta máquina guarda para as partes e para o sistema
distro-parts-heading = O que chega depois da instalação
distro-parts-none = Esta máquina não quer nenhuma parte: tudo o que ela roda veio com a instalação.
distro-parts-row-import = trazer de um pendrive
distro-parts-row-export = gravar num pendrive
distro-parts-row-system = o sistema
distro-parts-system-failed = { $count ->
        [one] a última atualização não construiu: falta um download
       *[other] a última atualização não construiu: faltam { $count } downloads
    }
distro-parts-system-waits = { $count ->
        [one] ao hardware que espera uma rede falta um download
       *[other] ao hardware que espera uma rede faltam { $count } downloads
    }
distro-parts-system-all-here = tudo o que faltava está aqui: atualize de novo
distro-parts-phase-here = aqui
distro-parts-phase-arriving = a caminho
distro-parts-phase-retrying = { $tries ->
        [one] uma tentativa falhou; a próxima em { $when }
       *[other] { $tries } tentativas falharam; a próxima em { $when }
    }
distro-parts-phase-waiting = esperando uma rede
distro-parts-phase-metered = esperando uma rede não tarifada
distro-parts-about-metered = Numa rede tarifada nada é baixado sozinho: ela espera outra. Escolher usa esta, só nesta tentativa.
distro-parts-phase-needs-you = precisa de você: { $tries } tentativas falharam; a próxima em { $when }
distro-parts-about-part = Uma parte chega depois da instalação, por conta própria: uma que não chega não segura nenhuma outra, e a máquina funciona sem ela. Ela é tentada de novo sozinha, cada vez mais espaçado, e escolhê-la a tenta agora.
distro-parts-about-missing-by-hand = Falta { $name }, de { $hosts }: baixe em outro lugar e entregue com { $keys }, traga num pendrive de uma máquina que o tenha, ou indique um espelho que o guarde.
distro-parts-about-missing-stick = Falta { $name }, que não é um arquivo só: ele vem num pendrive de uma máquina que o tenha.
distro-parts-about-missing-here = { $name } já está aqui.
distro-parts-about-system = O que a última atualização do sistema não conseguiu baixar. A máquina roda como antes, e toda atualização tenta de novo; escolher atualiza agora.
distro-parts-about-system-waits = O que ainda falta ao hardware que espera uma rede: a máquina foi atualizada sem ele, e toda atualização tenta de novo; escolher atualiza agora.
distro-parts-about-system-all-here = Tudo o que faltava está aqui: a próxima atualização constrói com isso.
distro-parts-about-why-build-failed = A última tentativa falhou: { $detail }
distro-parts-about-why-output-differs = A última tentativa construiu algo diferente do que este sistema quer; a próxima atualização traz o certo.
distro-parts-about-why-profile-failed = A última tentativa a construiu, e ela não pôde ser posta no lugar.
distro-parts-about-why-hand-in-refused = O arquivo entregue foi recusado: { $detail }
distro-parts-about-why-eval-failed = O que você declarou não avaliou: { $detail }
distro-parts-about-import = Traz, de um pendrive que outra máquina sua preencheu, os downloads que faltam às partes e ao sistema. Só downloads entram, cada um conferido pelo conteúdo — nunca um programa construído em outro lugar. Escolher pergunta a pasta do pendrive.
distro-parts-about-export = Grava num pendrive os downloads que esta máquina guarda para as partes e para o sistema, para outra máquina trazer. Escolher pergunta a pasta do pendrive.
distro-parts-stick-last-import = { $count ->
        [one] Da última vez, um download foi trazido, { $when }.
       *[other] Da última vez, { $count } downloads foram trazidos, { $when }.
    }
distro-parts-stick-last-export = { $count ->
        [one] Da última vez, um download foi gravado, { $when }.
       *[other] Da última vez, { $count } downloads foram gravados, { $when }.
    }
distro-parts-stick-refused = Da última vez, { $when }, nada foi movido:
distro-parts-refused-no-folder-named = nenhuma pasta foi indicada.
distro-parts-refused-folder-missing = a pasta não existe.
distro-parts-refused-folder-through-a-link = o caminho até a pasta passa por um link.
distro-parts-refused-not-a-removable-medium = a pasta não está num pendrive nem em outra mídia removível.
distro-parts-refused-no-downloads-there = o pendrive não tem downloads gravados por uma máquina desta distro.
distro-parts-refused-unwritable = os downloads não puderam ser gravados ali.
distro-parts-said-retrying = { $name } está sendo tentado agora.
distro-parts-said-not-started = A máquina não o iniciou.
distro-parts-said-already-here = Já está aqui.
distro-parts-said-not-a-part = Ponha o cursor numa parte, ou no sistema, para entregar um arquivo para ela.
distro-parts-said-nothing-missing = Não falta nenhum download agora: escolha-a para tentar de novo.
distro-parts-said-not-one-file = O que falta não é um arquivo só: traga num pendrive.
distro-parts-said-no-file = Não há arquivo ali.
distro-parts-said-not-copied = O arquivo não pôde ser entregue.
distro-parts-said-taken = { $name } foi aceito, e a parte está sendo construída.
distro-parts-said-taken-for-the-system = { $name } foi aceito: a próxima atualização constrói com ele.
distro-parts-said-refused = { $name } foi recusado: não é aquele download.
distro-parts-said-not-named = A pasta do pendrive não pôde ser indicada à máquina.
distro-parts-said-imported = { $count ->
        [one] Um download foi trazido.
       *[other] { $count } downloads foram trazidos.
    }
distro-parts-said-exported = { $count ->
        [one] Um download foi gravado no pendrive.
       *[other] { $count } downloads foram gravados no pendrive.
    }
cmd-distro-parts-mirrors = Indica os espelhos que a máquina consulta para o download de uma parte quando a origem falha
ask-distro-parts-mirrors = Os endereços dos seus espelhos, separados por espaço (nada indica nenhum)
distro-parts-row-mirrors = espelhos
distro-parts-mirrors-none = nenhum
distro-parts-mirrors-unreadable = seu arquivo de opções não pode ser lido
distro-parts-mirrors-newer = seu arquivo de opções é de um formato mais novo
distro-parts-about-mirrors = Servidores seus que guardam downloads pelo hash ou pelo nome — uma pasta numa máquina da sua rede, uma cópia do tarballs.nixos.org. Quando o download de uma parte falha na origem, a máquina pergunta a eles, e só aceita um arquivo se for exatamente o que a distro fixou. Eles viajam com as suas opções e chegam a uma máquina na próxima atualização dela. Escolher pergunta os endereços.
distro-parts-why-unreadable = Seu arquivo de opções não pode ser lido, então não é reescrito.
distro-parts-why-newer = Seu arquivo de opções é de um formato mais novo do que esta máquina lê: atualize esta máquina primeiro.
distro-parts-why-unwritable = O arquivo não pôde ser gravado.
distro-parts-said-not-a-mirror = Um espelho é um endereço http ou https, sem nome de usuário, consulta nem fragmento.
distro-parts-said-too-many-mirrors = São mais espelhos do que as suas opções podem indicar.
distro-parts-said-no-mirror = Nenhum espelho agora: a próxima atualização os tira desta máquina.
distro-parts-said-mirrors = Os espelhos chegam a esta máquina na próxima atualização.
distro-parts-said-mirrors-not-let = Os espelhos chegam a esta máquina na próxima atualização; permitir os hosts deles no seu consentimento falhou.
distro-parts-pick-hand-in = O arquivo que você baixou em outro lugar
distro-parts-pick-import = O pendrive de onde trazer os downloads
distro-parts-pick-export = O pendrive onde gravar os downloads
distro-parts-about-pick-hand-in = Escolha { $name }, como você o salvou depois de baixar em outro lugar: ele só é aceito se for exatamente aquele download.
distro-parts-stick-never = ainda não
distro-parts-stick-moved = { $count ->
        [one] um download, { $when }
       *[other] { $count } downloads, { $when }
    }
distro-parts-stick-refused-short = recusado, { $when }
distro-parts-down = próxima linha
about-distro-parts-down = Põe o cursor na próxima linha da página das partes.
distro-parts-up = linha anterior
about-distro-parts-up = Põe o cursor na linha anterior da página das partes.
distro-parts-enter = tentar agora
about-distro-parts-enter = Tenta agora a parte sob o cursor; na linha de um pendrive, pergunta a pasta dele.
distro-parts-hand = entregar arquivo
about-distro-parts-hand = Entrega um arquivo que você baixou em outro lugar para a parte sob o cursor.
distro-parts-steps = os passos
about-distro-parts-steps = De volta aos passos da atualização.
