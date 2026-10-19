# The texts `ronoctua-shell-keep` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

belongings-keep-local = manter o meu
belongings-take-remote = levar o deles
belongings-keep-both = manter os dois
belongings-prefs-refused = O belongings.json não se lê como configuração, então nada é escrito por cima dele: corrija à mão, ou apague.
belongings-no-places = Esta sessão não tem home ou diretório de runtime, e os pertences não têm onde morar.
belongings-decided-keep-local = manter o desta máquina
belongings-decided-take-remote = levar o do repositório
belongings-decided-keep-both = manter os dois
belongings-decide-keep-local = Manter o desta máquina
belongings-decide-take-remote = Levar o do repositório
belongings-decide-keep-both = Manter os dois
belongings-auth-ssh = uma chave SSH de implantação
belongings-auth-token = um token HTTPS
belongings-period-days = { $n ->
    [one] todo dia
   *[other] a cada { $n } dias
    }
belongings-period-never = nunca
belongings-empty-secret = Nada foi digitado.
belongings-not-written = Um arquivo dos pertences não pôde ser escrito; o log do sistema diz por quê.
belongings-never-kept = Isso nunca é guardado: é como esta máquina chega ao repositório.
belongings-outside-home = Só o que está na sua home pode ser guardado.
belongings-now-kept = { $name } agora é guardado nos pertences.
belongings-stopped-keeping = { $name } não é mais guardado nos pertences; ele fica onde está.
belongings-op-prepare = a sincronização
belongings-op-apply = a sincronização, com as suas decisões
belongings-op-abandon = desistir da sincronização
belongings-op-keygen = gerar a chave de implantação
belongings-op-rekey = a senha nova
belongings-reason-preference = as suas configurações
belongings-reason-programs = os programas que você instalou
belongings-reason-generated = refeito das suas configurações em cada máquina
belongings-reason-access = como esta máquina chega ao repositório
belongings-reason-own-state = o que os pertences guardam sobre si mesmos
belongings-reason-identity = a identidade da própria máquina: duas iguais colidiriam
belongings-reason-machine = o estado desta máquina
belongings-reason-history = um histórico desta máquina
belongings-reason-cache = um cache, refeito quando preciso
belongings-reason-system-managed = um link que o sistema cria: em outra máquina ele apontaria para o nada
belongings-repair-why-wrong-type = um valor do tipo errado
belongings-repair-why-missing = um valor que faltava
belongings-repair-why-invalid = um valor que não é permitido
belongings-op-compact = a compactação do histórico
belongings-op-sign = a assinatura da sua parte em claro
belongings-held-unsigned = retido: a parte em claro do repositório não traz assinatura
belongings-held-no-key = retido: não há aqui chave para conferir — fixe a sua abaixo
belongings-held-bad-signature = retido: não foi assinado pela sua chave
belongings-held-rolled-back = retido: uma versão mais antiga que a que esta máquina já aceitou
belongings-held-not-as-signed = retido: não é o que a sua chave assinou
belongings-root-refused-not-signed = O que foi entregue não trazia assinatura, e esta máquina ficou com os dados que tinha.
belongings-root-refused-bad-signature = Não foi assinado pela chave em que esta máquina confia, e ela ficou com os dados que tinha.
belongings-root-refused-rolled-back = Uma versão mais antiga que a que esta máquina já aceitou, e ela ficou com os dados que tinha.
belongings-kept-back-not-as-signed = mantido como esta máquina o tem: não é o que você assinou
belongings-kept-back-no-key-pinned = mantido como esta máquina o tem: não há chave fixada aqui
belongings-op-sync-plain = a sincronização da parte em claro
belongings-op-restore = a restauração
belongings-op-probe = a verificação do acesso
belongings-op-init = a primeira sincronização
belongings-op-verify-passphrase = a verificação da senha
belongings-decided-take-merged-ours = mesclado, o desta máquina onde conflitam
belongings-decided-take-merged-theirs = mesclado, o do repositório onde conflitam
belongings-decide-take-merged-ours = Mesclar, mantendo o desta máquina onde conflitam
belongings-decide-take-merged-theirs = Mesclar, pegando o do repositório onde conflitam
belongings-plain-already = Isso já viaja, em claro, com a parte em claro.
belongings-op-forget = o esquecimento da chave da sessão
belongings-target-git = as configurações do git e suas chaves de segurança registradas
belongings-target-gnupg = seu chaveiro do GnuPG
belongings-target-shell = o arquivo próprio do seu shell
belongings-target-emacs = sua configuração do Emacs
# What a target leaves out, each by its own reason. Short: the listing's
# widest value moves its column.
belongings-reason-socket = um socket de um programa que está rodando
belongings-reason-connection = uma conexão SSH compartilhada enquanto está aberta
belongings-reason-random-seed = a semente aleatória do agente: cada máquina tem a sua
belongings-reason-lock = uma trava de um programa que está rodando
belongings-reason-packages = pacotes, baixados de novo a partir da sua configuração
belongings-reason-compiled = compilado de novo a partir do que viaja
belongings-reason-editor-state = o que o Emacs guarda das sessões desta máquina
belongings-reason-editor-draft = o backup, o salvamento automático ou a trava de um editor
belongings-reason-git-folder = a pasta própria de um repositório git: um clone a traz
belongings-state-synced = em dia
belongings-state-changed = mudou
belongings-state-new = novo
belongings-state-conflict = em conflito
belongings-state-unreadable = ilegível aqui
belongings-state-repaired = consertado aqui
belongings-state-deleted = apagado aqui: sai do repositório e das suas outras máquinas
belongings-reason-too-large = grande demais para guardar em claro
