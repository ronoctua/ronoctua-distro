# The texts `ronoctua-installer` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Installer — the six questions (section 7.1)

inst-title = Instalar o ronoctua
inst-disk = Disco de destino
inst-disk-help = Será apagado por inteiro, sem recuperação.
inst-hostname = Nome da máquina
inst-hostname-help = Começa com `noctua-`. Minúsculas, dígitos e hífen.
inst-password = Senha do usuário
inst-password-help = Para entrar no sistema depois de instalado.
inst-passphrase = Frase-secreta do disco
inst-passphrase-help = Destrava o disco quando o TPM se recusar. Guarde fora do computador.
inst-language = Idioma
inst-language-help = Da interface e do teclado.
inst-keyboard = Layout do teclado
inst-keyboard-help = Escolha o do seu teclado. E nele que voce vai digitar a frase-secreta.
inst-keyboard-variant = Variante do layout
inst-keyboard-variant-help = Separa teclados do mesmo pais. Sem variante e uma resposta valida.
inst-compose-key = Tecla de acentuacao
inst-compose-key-help = A tecla Compose: aperta e solta, depois duas teclas, e sai o acento. Sem ela, teclado sem acento nao os alcanca. O padrao e o Caps Lock, que os dois Shifts juntos assumem.
inst-system-locale = Formato do sistema
inst-system-locale-help = Data, numero e unidade. Nao e o idioma da interface, que ja foi escolhido.
inst-timezone = Fuso horario
inst-timezone-help = Onde a maquina fica. E o que decide a hora do relogio e do log.
inst-profile = Instalação
inst-profile-help = `full` traz tudo e é a recomendada. `minimal` deixa de fora Android, os compiladores além do Rust e do Common Lisp, navegador, editor, mídia e as variantes de fonte — instala em minutos, e cada grupo volta com uma linha no host.nix.
inst-resume-offset = Deslocamento do swap
inst-resume-offset-help = Calculado depois que o disco existir; não se digita.

## Installer — the steps

inst-step-key = Guardar a frase-secreta para o disko
inst-step-partition = Particionar e criptografar o disco
inst-step-flake = Copiar o flake para o sistema alvo
inst-step-host-nix = Gravar a descrição desta máquina
inst-step-facter = Detectar o hardware
inst-step-password = Gravar o hash da senha em /persist
inst-step-install = Construir e instalar o sistema
inst-step-zone = Definir o fuso horário desta máquina
inst-step-target = Criar a pasta de configuração do sistema alvo

## Installer — the interface

inst-review = Revisar antes de executar
inst-destructive = Os passos marcados apagam o disco
inst-confirm = Para confirmar, digite o caminho do disco
inst-confirm-error = Não confere com o disco escolhido
inst-to-apply = Para valer, com o disco sendo apagado: --execute
inst-dryrun = Ensaio: nada foi tocado
inst-running = Instalando
inst-done = Instalação concluída. Tire o pendrive e reinicie.
inst-failed = A instalação parou neste passo
inst-no-disk = Nenhum disco encontrado nesta máquina
inst-keys-edit = Enter segue · Esc volta · Ctrl+C sai
inst-keys-choose = ↑↓ escolhem · Enter segue · Esc volta
inst-keys-review = Enter executa · Esc volta · Ctrl+C sai
inst-keys-quit = Ctrl+C sai

## Installer — the seventh question (network)

inst-wifi = Rede Wi-Fi
inst-wifi-help = A senha fica na máquina instalada, nunca no repositório.
inst-wifi-none = sem Wi-Fi — vou usar cabo
inst-wifi-scanning = Procurando redes…
inst-wifi-empty = Nenhuma rede ao alcance
inst-wifi-password = Senha da rede
inst-wifi-password-short = a senha de uma rede WPA tem ao menos 8 caracteres
inst-step-seal-wifi = Selar a senha do Wi-Fi no TPM
inst-step-wifi = Gravar o perfil da rede Wi-Fi

## file info editing — everything the info panel can change

# A décima quinta pergunta do instalador.
inst-belongings = Repositório dos pertences
inst-belongings-help = O repositório privado em que sua configuração fica guardada, cifrada: git@servidor:caminho, ssh://… ou https://…, nunca com senha dentro. As suas declarações são instaladas com a máquina, e as suas configurações restauradas na home dela.
inst-step-belongings = Escrever o repositório dos pertences na sua home
inst-step-plain-passphrase = Passar a senha à cópia das suas declarações
inst-step-pin = Fixar a sua chave de assinatura nesta máquina
inst-step-cache-token = Ler o token do seu cache binário privado para a instalação
inst-step-cache-netrc = Deixar esta máquina ler o seu cache binário privado
inst-belongings-access = Acesso ao repositório
inst-belongings-access-help = Esta máquina chega ao repositório com uma chave própria, feita agora e guardada só na sua home. Ela é testada de novo até ler e escrever.
inst-belongings-token-help = Digite um token que leia e escreva no repositório. Ele fica na sua home, legível só por você.
inst-belongings-passphrase = Senha dos pertences
inst-belongings-passphrase-help = A senha que abre os seus pertences. Ela é conferida no repositório antes de qualquer coisa ser tocada.
inst-belongings-passphrase-new-help = O repositório está vazio: escolha a senha que vai selar os seus pertences, com pelo menos 12 caracteres. Ela é digitada duas vezes.
inst-belongings-passphrase-again = Digite a senha de novo
inst-belongings-passphrase-differs = As duas senhas são diferentes. Digite de novo desde o começo.
inst-belongings-key-intro = Adicione esta chave ao repositório, com permissão de escrita:
inst-belongings-key-where = Onde: { $address }
inst-belongings-key-where-unknown = Onde: nas configurações do repositório, como chave de implantação que pode escrever.
inst-belongings-token-where = O token se faz aqui: { $address }
inst-access-waiting = O repositório ainda não responde a esta chave.
inst-access-reads = A chave lê o repositório, mas ainda não escreve: dê a ela permissão de escrita.
inst-access-ready = A chave lê e escreve no repositório.
inst-checking = Conferindo…
inst-network-help = A instalação precisa da rede. Ela fala com { $hosts } e com o repositório dos seus pertences; escolher uma rede, ou o cabo, é deixar.
inst-network-help-unnamed = A instalação precisa da rede: ela fala com o release da distro, com o cache binário dela e com o repositório dos seus pertences.
inst-gate-checking = Conectando e procurando uma saída…
inst-gate-open = A rede tem saída.
inst-gate-no-daemon = Nenhum serviço de rede responde neste instalador.
inst-gate-off = A rede está desligada.
inst-gate-no-link = Sem rede: ligue um cabo ou escolha uma rede Wi-Fi.
inst-gate-unconnected = Essa rede não aceitou a conexão: confira a senha ou escolha outra.
inst-gate-portal = Uma página de login fica entre esta rede e a internet: escolha outra rede.
inst-release-looking = Perguntando ao release qual instalador usar…
inst-release-newer = Há um instalador mais novo ({ $rev }). O Enter o inicia, com as respostas que você deu.
inst-release-ahead = Este instalador é mais novo que o release: a máquina é instalada com a distro que ele traz e fica com ela.
inst-release-unreachable = O release não respondeu: esta máquina é instalada com a distro deste instalador e fica com ela.
inst-release-development = Este instalador é uma versão de desenvolvimento: a máquina é instalada com a distro que ele traz e fica com ela.
inst-invalid-blank = Não pode ficar em branco.
inst-invalid-hostname = Só letras minúsculas, números e hífen; sem começar nem terminar com hífen.
inst-invalid-hostname-prefix = O nome da máquina começa com `{ $prefix }`.
inst-invalid-passphrase-short = Pelo menos 12 caracteres.
inst-invalid-disk = Esse caminho não é um disco.
inst-invalid-language = Esse idioma não está disponível.
inst-invalid-layout = Esse layout de teclado não existe.
inst-invalid-profile = A instalação é `full` ou `minimal`.
inst-invalid-access = Indique o arquivo que guarda a chave ou o token, pelo caminho completo.
inst-step-template = Escrever o flake desta máquina a partir do template
inst-step-aim = Apontar o flake para a distro deste instalador
inst-step-lock = Fixar a versão da distro com que a máquina é construída
inst-step-attribute = Perguntar ao flake qual sistema ele constrói
inst-step-plain = Buscar as suas declarações no repositório
inst-step-person = Aplicar as suas declarações a esta máquina
inst-step-verify = Experimentar cada programa que você declarou
inst-step-access = Dar à sua home o acesso ao repositório
inst-step-consent = Permitir o servidor do repositório no seu consentimento
inst-step-passphrase = Passar a senha à restauração
inst-step-restore = Restaurar os seus pertences na sua home
inst-step-init = Criar o repositório, guardando nele as configurações desta máquina
inst-step-skipped = Nada a fazer aqui.

## Installer — the eighteenth question (the programs a full installation leaves out)

inst-left-out = Programas opcionais
inst-left-out-help = Todo programa marcado é instalado, a não ser que os seus pertences o recusem em todas as máquinas. Um desmarcado não é baixado agora, e nenhuma atualização o traz, só nesta máquina — a central de programas o traz de volta.
inst-left-out-continue = Seguir com estes
inst-left-out-kept = instalar
inst-left-out-out = deixar de fora
inst-left-out-review = Fica de fora desta máquina: { $names }
inst-keys-check = ↑↓ escolhem · Enter marca ou segue · Esc volta
inst-invalid-left-out = Só programas da lista ficam de fora, cada um uma vez; ou nenhum.
inst-step-left-out = Escrever os programas que esta máquina deixa de fora
inst-step-plain-declining = Assinar no seu repositório a escolha de programas desta máquina, e buscar as suas declarações
inst-step-left-out-home = Escrever na sua home os programas deixados de fora
inst-left-out-review-kept = Um programa marcado que esta máquina deixou de fora antes volta; um que os seus pertences recusam em todas as máquinas fica de fora — a central de programas o traz de volta.
inst-left-out-dropped = Esta versão não lista mais { $ids }: nada disso fica de fora. Desmarque de novo o que deixar de fora.
inst-left-out-not-applied = As declarações aplicadas a esta máquina não dizem a escolha de programas dela; o disco não foi tocado.
inst-step-left-out-applied = Conferir que a escolha de programas desta máquina foi aplicada
inst-more-above = ↑ mais { $count }
inst-more-below = ↓ mais { $count }
inst-review-below = ↓ { $count ->
        [one] mais um passo
       *[other] mais { $count } passos
    } abaixo — { $marked ->
        [0] nenhum deles apaga o disco
        [one] um deles apaga o disco
       *[other] { $marked } deles apagam o disco
    }
inst-step-choice = Escrever quais programas esta máquina mantém e quais deixa de fora
inst-kept-everywhere = Marcados, e ainda recusados em todas as máquinas pelos seus pertences: { $names }. A central de programas os traz de volta.
