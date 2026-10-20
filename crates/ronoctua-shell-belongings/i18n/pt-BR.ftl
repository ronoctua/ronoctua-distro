# The texts `ronoctua-shell-belongings` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# OS PERTENCES (`LEADER s v`, `crate::belongings`): a configuração da
# pessoa, guardada cifrada num repositório privado que ela indica.
group-belongings = pertences
belongings-title = Pertences
cmd-belongings-open = Pertences: a configuração guardada no seu próprio repositório
cmd-belongings-window = Pertences, numa janela
cmd-belongings-sync = Sincronizar os pertences agora: pede a senha
cmd-belongings-up = Linha anterior
cmd-belongings-down = Próxima linha
cmd-belongings-less = O valor anterior da linha
cmd-belongings-more = O próximo valor da linha
cmd-belongings-enter = O que a linha sob o cursor faz
cmd-belongings-keep-local = Manter o lado desta máquina no conflito: ele vai por cima do do repositório
cmd-belongings-take-remote = Levar o lado do repositório no conflito: o desta máquina fica guardado à parte
cmd-belongings-keep-both = Manter os dois lados: o do repositório fica ao lado deste, com o nome da outra máquina
cmd-belongings-apply = Aplicar as decisões e terminar a sincronização
cmd-belongings-abandon = Desistir da sincronização preparada: nada é escrito, e a senha é esquecida
cmd-belongings-all-local = Todos os conflitos: manter o lado desta máquina
cmd-belongings-all-remote = Todos os conflitos: levar o lado do repositório
cmd-belongings-repository = Definir o endereço do repositório
cmd-belongings-add = Guardar outro arquivo ou pasta da sua home
cmd-belongings-auth = Chegar ao repositório por uma chave SSH de implantação, ou por um token
cmd-belongings-keygen = Gerar a chave SSH de implantação desta máquina
cmd-belongings-copy-key = Copiar a metade pública da chave de implantação, para entregá-la ao repositório
cmd-belongings-token = Guardar um token do repositório, legível só por você
cmd-belongings-user = O nome de usuário que o servidor do repositório pede
cmd-belongings-passphrase = Trocar a senha: pede a atual e depois a nova duas vezes
cmd-belongings-period = De quanto em quanto tempo lembrar de sincronizar
belongings-up = subir
belongings-down = descer
belongings-less = menos
belongings-more = mais
belongings-enter = fazer
belongings-sync = sincronizar
ask-belongings-sync = Senha dos pertences
ask-belongings-repository = O endereço do repositório: git@servidor:caminho, ssh://… ou https://…
ask-belongings-add = Um arquivo ou pasta da sua home para guardar
ask-belongings-token = O token, como o servidor do repositório o deu
ask-belongings-user = O nome de usuário que o servidor pede
ask-belongings-passphrase = A senha em uso
ask-belongings-passphrase-new = A senha nova
ask-belongings-passphrase-again = A senha nova de novo
ask-belongings-sync-again = A senha de novo — um repositório novo fica com ela para sempre
about-belongings-conflict = Mostra o que difere entre os dois lados, quando os dois são texto
about-belongings-rule = Tira isto do que é guardado em todas as máquinas que você sincroniza
about-belongings-kept = Deixa de guardar isto: o arquivo fica onde está
about-belongings-excluded = Volta a guardar isto
about-belongings-never = Guarda mesmo assim: vai para todas as máquinas que você sincroniza
belongings-status = Estado
belongings-repository = Repositório
belongings-none-yet = nenhum ainda
belongings-last-sync = Última sincronização
belongings-never-here = nunca nesta máquina
belongings-next = Próximo lembrete
belongings-due-now = agora
belongings-no-reminder = sem lembretes
belongings-kept-now = Guardado agora
belongings-files-weigh = { $n ->
    [0] nenhum arquivo, { $size }
    [one] um arquivo, { $size }
   *[other] { $n } arquivos, { $size }
    }
belongings-last-run = Última execução
belongings-first-run = Esta máquina ainda não sincronizou com este repositório. A primeira sincronização restaura o que o repositório guarda; onde um arquivo daqui difere, o lado do repositório já vem escolhido.
belongings-outcome-synced = Em dia — escritos aqui: { $pulled }; enviados: { $pushed }; apagados: { $deleted }.
belongings-outcome-waiting = { $n ->
    [one] Esperando sua decisão sobre um conflito.
   *[other] Esperando suas decisões sobre { $n } conflitos.
    }
belongings-outcome-abandoned = A sincronização preparada foi abandonada.
belongings-prepared-gone = Uma sincronização foi preparada aqui e se perdeu — a sessão terminou desde então —, então nada espera por você: sincronize de novo.
belongings-outcome-keyed = Uma chave de implantação foi gerada.
belongings-outcome-rekeyed = A senha foi trocada.
belongings-large = { $name } pesa { $size }: toda sincronização o carrega.
belongings-unreadable = { $n ->
    [one] Um arquivo daqui não pôde ser lido e ficou como estava dos dois lados.
   *[other] { $n } arquivos daqui não puderam ser lidos e ficaram como estavam dos dois lados.
    }
belongings-unwritten = { $n ->
    [one] Um arquivo do repositório não pôde ser escrito aqui; a próxima sincronização tenta de novo.
   *[other] { $n } arquivos do repositório não puderam ser escritos aqui; a próxima sincronização tenta de novo.
    }
belongings-sync-heading = Sincronizar
belongings-sync-now = Sincronizar agora
belongings-apply = Aplicar as decisões
belongings-decided-of = { $n } de { $of } decididos
belongings-abandon = Desistir desta sincronização
belongings-waiting-note = A sincronização está preparada e espera você: nada foi escrito aqui ainda.
belongings-plan-waits = Uma sincronização está preparada e espera as suas decisões: aplique-a ou desista dela antes.
belongings-conflicts = { $n ->
    [one] Um conflito
   *[other] { $n } conflitos
    }
belongings-conflicts-note = Mudou aqui e no repositório desde a última sincronização. Decida cada um: manter o desta máquina, levar o do repositório ou — no que você acrescentou — manter os dois.
belongings-all-keep-local = Todos: manter o desta máquina
belongings-all-take-remote = Todos: levar o do repositório
belongings-undecided-left = { $n ->
    [one] Um conflito ainda está sem decisão.
   *[other] { $n } conflitos ainda estão sem decisão.
    }
belongings-undecided = sem decisão
belongings-chosen = escolhido
belongings-sides = aqui: { $here } · repositório: { $there }, mudou em { $machine }
belongings-side = { $size }, { $when }
belongings-side-deleted = apagado
belongings-diff-title = Diferença em { $name }
belongings-diff-note = As linhas que começam com - são desta máquina; as que começam com + são do repositório.
belongings-what-leaks = Tudo, menos a parte em claro mais abaixo, é cifrado com a sua senha, de forma pós-quântica, antes de sair desta máquina, cortado em pedaços. Disso, o repositório mostra quantos pedaços há, os tamanhos, que conteúdo se repete e quando cada sincronização aconteceu; nunca os nomes dos arquivos, os lugares ou o conteúdo.
belongings-added-by-you = acrescentado por você
belongings-added-elsewhere = acrescentado em outra máquina
belongings-taken-out = tirado por você
belongings-add = Guardar mais alguma coisa
belongings-large-note = O que você acrescenta é guardado inteiro, pastas e links inclusive. Acima de { $size }, a página avisa: toda sincronização o carrega.
belongings-never = Nunca guardado
belongings-access = Acesso
belongings-auth = Chega por
belongings-deploy-key = Chave de implantação
belongings-copy-it = copiar a chave pública
belongings-make-it = gerar uma
belongings-deploy-note = Dê a esta chave pública acesso de escrita ao repositório, como chave de implantação. A metade privada fica nesta máquina, legível só por você, e nunca é guardada nos pertences.
belongings-keygen-note = Esta máquina ainda não tem chave de implantação. Gerar uma leva um instante na fila; depois a metade pública aparece aqui para copiar.
belongings-token = Token
belongings-token-is-kept = guardado, legível só por você
belongings-user = Nome de usuário
belongings-token-note = O token fica num arquivo que só você lê, e chega ao git só pelo caminho — nunca numa linha de comando.
belongings-passphrase = Senha
belongings-change-passphrase = Trocar a senha
belongings-passphrase-note = A senha abre um arquivo pequeno, e esse arquivo abre o resto: trocá-la cifra de novo esse arquivo, em todas as máquinas de uma vez. Nada recupera uma senha perdida.
belongings-period = Lembrete
belongings-remind-every = Lembrar de sincronizar
belongings-needs-key = Ainda não há chave de implantação: gere uma em Acesso e dê a ela acesso de escrita ao repositório.
belongings-needs-token = Ainda não há token: guarde um em Acesso.
belongings-nothing-prepared = Não há sincronização preparada.
belongings-no-conflict-here = O cursor não está num conflito.
belongings-both-only-added = Os dois lados só podem ser mantidos no que você mesmo acrescentou.
belongings-no-difference = A diferença deste arquivo não está mais aqui; sincronize de novo para vê-la.
belongings-not-text = Este arquivo não é texto, então não há diferença para mostrar: decida pelo tamanho e pela hora.
belongings-key-already = Esta máquina já tem uma chave de implantação.
belongings-no-key-yet = Ainda não há chave de implantação.
belongings-passphrase-first = A primeira sincronização define a senha: ainda não há nenhuma para trocar.
belongings-token-kept = O token está guardado.
belongings-passphrases-differ = As duas senhas novas diferem, então nada mudou.
belongings-first-passphrases-differ = As duas senhas diferem, então nada foi enviado.
belongings-queued = Enviado para a fila: { $what }.
belongings-reason-draft = um rascunho, ou o número de um programa rodando
belongings-repair-why-unknown-variant = uma palavra que esta máquina não conhece — quase sempre de uma máquina mais nova
belongings-outcome-compacted = O histórico foi compactado: { $before } viraram { $after } aqui. O host pode guardar as cópias antigas até a própria limpeza.
belongings-outcome-nothing-to-compact = Não havia o que compactar: todas as versões do histórico são { $days ->
        [one] do último dia
       *[other] dos últimos { $days } dias
    }, e não sobrou nenhum pedaço.
belongings-consent-for = Sincronizar os pertences consulta o repositório neste host.
belongings-unsigned = mudou aqui e espera a sua senha para ser assinado
belongings-sign = Assinar agora
cmd-belongings-sign = Assinar agora, com a sua chave e sem rede, o que você mudou aqui, para que esta máquina e as outras o aceitem
ask-belongings-sign = Senha dos pertences, para assinar
belongings-signature = A sua assinatura
belongings-signature-note = O que viaja em claro é assinado com a sua chave, criada com o seu repositório e guardada sob a sua senha. Uma máquina só aceita sozinha o que essa chave assinou — a chave que ela fixou, nunca a cópia que está no repositório; o resto espera, com o motivo.
belongings-signature-key = A sua chave
belongings-signature-key-unknown = ainda não conhecida aqui: uma sincronização com a sua senha a mostra
belongings-signature-machine = Esta máquina
belongings-signature-pinned = confia na sua chave
belongings-signature-pinned-some = confia numa chave: uma sincronização com a sua senha diz se é a sua
belongings-signature-other = confia em outra chave: fixe a sua
belongings-signature-none = ainda não confia em nenhuma chave: as suas opções e os seus módulos do cérebro esperam
belongings-signature-last = Última entrega
belongings-signature-refused = recusada
belongings-signature-sync-first = Sincronize antes uma vez com a sua senha: é isso que mostra a sua chave aqui.
belongings-signature-already = Esta máquina já confia na sua chave.
belongings-pinning = Fixação
belongings-pinning-now = perguntando ao sudo…
belongings-pinning-done = esta máquina agora confia na sua chave
belongings-pinning-not = não aceita: o diário diz por quê
belongings-pinning-busy = Uma fixação já está a caminho.
belongings-pin = Fixar a sua chave nesta máquina
cmd-belongings-pin = Fixar nesta máquina, pelo sudo, a chave que a sua senha abriu: a máquina passa a aceitar sozinha o que essa chave assinou
belongings-outcome-plain-synced = A parte em claro está em dia — escritos aqui: { $pulled }; enviados: { $pushed }.
cmd-belongings-take-merged-ours = Mesclar o conflito, mantendo o lado desta máquina onde os dois conflitam
cmd-belongings-take-merged-theirs = Mesclar o conflito, pegando o lado do repositório onde os dois conflitam
belongings-reason-person-data = o que você declarou: programas, máquinas, opções
belongings-reason-brain-module = seus próprios módulos do cérebro
belongings-plain-note = Guardado em claro, sem cifra: a máquina lê isso antes de alguém digitar a senha. Quem lê o repositório lê isso; quem escreve nele não consegue fazer uma máquina aceitar uma mudança que a sua chave não assinou — ela espera, com o motivo. Um arquivo acima de { $size } fica de fora.
belongings-no-merge-here = Nenhuma mescla chegou a este conflito: mantenha um lado, ou pegue o outro.
belongings-sides-unnamed = aqui: { $here } · repositório: { $there }
belongings-plain-conflicts = { $n ->
        [one] Um arquivo dela mudou aqui e no repositório: a próxima sincronização pergunta a você.
       *[other] { $n } arquivos dela mudaram aqui e no repositório: a próxima sincronização pergunta a você.
    }
belongings-merge-hunks = { $n ->
        [one] Mesclado, mas uma parte do texto mudou dos dois lados.
       *[other] Mesclado, mas { $n } partes do texto mudaram dos dois lados.
    }
belongings-merge-fields = { $n ->
        [one] Mesclado, mas um campo mudou dos dois lados: { $fields }
       *[other] Mesclado, mas { $n } campos mudaram dos dois lados: { $fields }
    }
# The predefined targets, by id (`assets/belongings.nix`'s `targets`): what the
# list of what goes to the repository names them.
belongings-target-ssh = suas chaves e configurações de SSH
# Beside the targets and the passphrase: what the passphrase guards now.
belongings-targets-note = Suas chaves de SSH, seu chaveiro do GnuPG e suas chaves de segurança viajam cifrados sob esta senha: agora ela os guarda também, então escolha uma senha forte.
# What a sync that brought a target's files offers ("no" drawn first).
belongings-offer-ssh-signing = Assinar seus commits e tags do git com { $key }? Toda máquina que tem esta chave vai assinar; as outras seguem sem assinatura.
belongings-offer-pam-u2f = Trazer para esta máquina as chaves de segurança que você registrou nas outras? Cada uma responde ao sudo aqui no lugar da senha, que continua valendo.
belongings-offer-vscode-extensions = { $n ->
    [one] Reinstalar a extensão do VS Code da sua lista que falta nesta máquina?
   *[other] Reinstalar as { $n } extensões do VS Code da sua lista que faltam nesta máquina?
  } Elas vêm do Visual Studio Marketplace, uma de cada vez, na fila de transferências, e o sim libera { $hosts ->
    [one] este host:
   *[other] estes { $hosts } hosts:
  }
# Your VS Code extensions on this machine: what the last install did, and what is missing.
belongings-extensions = Suas extensões do VS Code
belongings-extensions-missing = { $n ->
    [one] Uma extensão da sua lista falta aqui. Instalá-la libera { $hosts ->
        [one] este host:
       *[other] estes { $hosts } hosts:
      }
   *[other] { $n } extensões da sua lista faltam aqui. Instalá-las libera { $hosts ->
        [one] este host:
       *[other] estes { $hosts } hosts:
      }
  }
belongings-extensions-install = { $n ->
    [one] Instalar
   *[other] Instalar todas
  }
belongings-extensions-installing = Instalando na fila de transferências…
belongings-extensions-stopped = A última instalação parou antes do fim: o que entrou está instalado, e o resto continua faltando.
belongings-extensions-entered = { $n ->
    [0] Nenhuma extensão entrou.
    [one] Uma extensão entrou.
   *[other] { $n } extensões entraram.
  }
belongings-extensions-failed = { $n ->
    [one] Uma não pôde ser instalada, e o VS Code disse por quê:
   *[other] { $n } não puderam ser instaladas, e o VS Code disse por quê:
  }
belongings-extensions-refused = Não foi instalado: { $why }
belongings-extensions-refused-no-code = o VS Code não está nesta máquina
belongings-extensions-refused-newer = sua lista de extensões foi escrita por uma máquina mais nova: atualize esta primeiro
belongings-extensions-refused-malformed = sua lista de extensões não pôde ser lida, e nada foi instalado
belongings-extensions-refused-not-let = estes hosts não foram liberados, e nada foi pedido:
belongings-extensions-refused-busy = outra instalação delas estava rodando
belongings-extensions-refused-files = um arquivo do VS Code daqui não pôde ser lido ou escrito; o log do sistema diz qual
# The identity kept for the session (`keep_unlocked`).
belongings-unlock-switch = Lembrar a senha durante esta sessão
belongings-unlock-note = Guardada na memória do kernel, nunca no disco, até você sair, a máquina dormir ou passarem { $hours } horas.
belongings-unlock-kept = Lembrada até { $until }: a sincronização não pergunta nada.
# The one listing of what goes to the repository (`inventory`), its groups,
# its rows and the states of a file.
belongings-listing = O que vai para o seu repositório
belongings-counting = Contando o que vai para o repositório…
belongings-item = { $state } · { $size }
belongings-here-not-here = não nesta máquina
belongings-here-nowhere = em nenhuma máquina
belongings-never-because = nunca: { $reason }
# A predefined target: where it is kept, and what may be done about it.
belongings-target-everywhere = em todas as máquinas
belongings-target-partly-here = em todas as máquinas, parte dele não nesta
belongings-target-asked-nowhere = em nenhuma máquina, a partir da próxima sincronização
belongings-target-asked-everywhere = em todas as máquinas de novo, a partir da próxima sincronização
belongings-target-act-not-here = Não nesta máquina
belongings-target-act-nowhere = Em nenhuma máquina
belongings-target-act-here-again = Guardar de novo nesta máquina
belongings-target-act-everywhere = Guardar de novo em todas as máquinas
belongings-nowhere-question = Deixar de guardar { $target } em todas as máquinas? As cópias saem do repositório na próxima sincronização; cada outra máquina as esquece e fica com os próprios arquivos.
about-belongings-not-here = Deixa de guardar isto nesta máquina: a cópia do repositório fica para as outras
about-belongings-nowhere = Deixa de guardar isto em todas as máquinas — pergunta antes
about-belongings-everywhere = Volta a guardar isto em todas as máquinas, a partir da próxima sincronização
# Where this machine's Emacs reads its configuration, when it is not the
# one place the belongings carry (`ronoctua_belongings::emacs`): said, never
# acted on.
belongings-emacs-reads = O Emacs daqui lê { $place }, mas os seus pertences levam { $carried }: mova a sua configuração para lá e ela viaja.
belongings-emacs-hidden = { $place } não guarda nada seu, só o que o Emacs faz, e enquanto ele estiver lá o Emacs o lê no lugar de { $carried }, que os seus pertences levam.
belongings-emacs-no-file = { $place } existe mas não é um arquivo que o Emacs carregue, e enquanto ele estiver lá o Emacs lê { $folder } no lugar de { $carried }, que os seus pertences levam.
belongings-emacs-xdg = Com o seu $XDG_CONFIG_HOME, o Emacs daqui procura a configuração em { $place } e na sua home, nunca em { $carried }, que os seus pertences levam.
# A question on a page of its own, "no" first.
belongings-answer-no = Não
belongings-answer-yes = Sim
about-belongings-answer-no = Deixa tudo como está
about-belongings-answer-yes = Faz o que a pergunta diz
# What the last sync offered to turn on.
belongings-offers = Oferecido pela última sincronização
belongings-offer-turning = Ligando…
belongings-offer-taken = Ligado.
belongings-offer-refused = Não foi ligado: { $why }
about-belongings-offer-no = Deixa desligado: só volta a ser oferecido quando a chave mudar em outra máquina
about-belongings-offer-yes = Liga isto, nesta máquina
about-belongings-offer-no-extensions = Deixa de fora: só volta a ser oferecido quando sua lista mudar em outra máquina, e a página mantém uma linha que as instala
about-belongings-offer-yes-extensions = Libera os hosts acima e as instala nesta máquina, na fila de transferências
about-belongings-extensions-install = Libera os hosts acima e as instala nesta máquina, na fila de transferências, uma de cada vez
# The files a lenient read repaired.
belongings-repairs = Consertado na leitura
belongings-repairs-note = Esta máquina não conseguiu ler parte destes arquivos. O que ela não leu continua em cada arquivo como está escrito, e ela seguiu sem isso. Grave o conserto para pôr os valores desta máquina no lugar, ou deixe o arquivo como está.
belongings-repairs-newest = Esta máquina roda o sistema mais novo das suas máquinas: a próxima sincronização grava estes consertos sozinha, e os envia.
belongings-repairs-older = Outra das suas máquinas roda um sistema mais novo e pode conhecer estes valores: decida aqui.
belongings-repair-count = { $n ->
    [one] um valor
   *[other] { $n } valores
    }
belongings-repair-line = { $pointer }: { $why }
belongings-repair-whole-file = o arquivo inteiro
belongings-repair-leave = Deixar
belongings-repair-write = Gravar o conserto
belongings-repair-written = O conserto de { $file } foi gravado.
belongings-repair-stale = { $file } mudou desde que foi lido: o que falta consertar é perguntado de novo quando ele for lido.
about-belongings-write-fix = Grava os valores desta máquina no lugar do que ela não leu
about-belongings-leave-it = Deixa o arquivo como está, e para de perguntar até ele mudar
# The history: its weight and its compaction.
belongings-history = Histórico
belongings-weighs = Pesa
belongings-weighs-value = { $tree } de arquivos, { $history } com o histórico
belongings-weight-heavy = O histórico pesa { $history }: acima de { $recommend }, compactá-lo é recomendado.
belongings-weight-compact = Compactar o histórico é recomendado: ele pesa { $history }, muito mais do que o que guarda.
belongings-compact = Compactar o histórico
belongings-compact-keeps = guarda { $days ->
    [one] o último dia
   *[other] os últimos { $days } dias
    }
belongings-compact-note = Todo arquivo fica como está agora; as versões mais antigas saem do repositório, para todas as máquinas. O servidor pode guardar as cópias antigas até a limpeza dele.
belongings-compact-question = As versões mais antigas saem do repositório de vez, para todas as máquinas; todo arquivo como está agora fica, e o servidor pode guardar as cópias antigas até a limpeza dele. Compactar o histórico para { $days ->
    [one] o último dia?
   *[other] os últimos { $days } dias?
    }
ask-belongings-compact = A senha, para compactar o histórico
about-belongings-compact = Compacta o histórico — pergunta antes
# Whether the session remembers the passphrase.
belongings-unlock-on = sim
belongings-unlock-off = não
about-belongings-unlock = Se a sincronização pede a senha uma vez por sessão, ou toda vez
# Every field of a JSON conflict, decided on its own.
belongings-field-ours = O valor desta máquina
belongings-field-theirs = O valor do repositório
belongings-field-none = nenhum
belongings-field-chosen = { $value } · escolhido
belongings-field-before = antes: { $value }
belongings-decided-fields = mesclado campo a campo
belongings-fields-decided-of = { $n } de { $of } campos decididos
belongings-field-only-two = Um campo fica com o valor desta máquina ou com o do repositório.
about-belongings-field = Decide este campo sozinho: os dois valores dele são as linhas abaixo
about-belongings-field-ours = Fica com o valor desta máquina neste campo
about-belongings-field-theirs = Fica com o valor do repositório neste campo
