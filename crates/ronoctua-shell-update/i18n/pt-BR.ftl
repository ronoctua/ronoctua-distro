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
machine-refused-build-failed = O sistema novo não foi construído. Nada mudou: a máquina roda o que rodava, e a pasta dela volta a dizer isso.
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
