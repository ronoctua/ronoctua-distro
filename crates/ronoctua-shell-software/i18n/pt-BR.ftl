# The texts `ronoctua-shell-software` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## O menu da janela — o nome do app na modeline, clicado (LEADER w o)

# THE PERSON'S OWN PROGRAMS (LEADER i) — `software.rs`, its words
# (`software_words.rs`) and the queue's phases. A key is never text here:
# every sequence arrives as `{ $keys }`.
software-title = Programas
group-software = programas
cmd-software-open = Seus programas: procurar, instalar, experimentar e remover
cmd-software-window = Seus programas, numa janela
cmd-software-trials = Os programas em experiência
cmd-software-maintenance = Atualizações, espaço e voltar atrás, dos seus programas
cmd-software-android = O Android e os apps dele
cmd-software-launch-latest = Abrir o que foi instalado por último
software-down = descer
software-up = subir
software-enter = abrir
software-next-source = próxima fonte
software-previous-source = fonte anterior
software-less = anterior
software-more = seguinte
software-install = instalar
software-try-it = experimentar
software-launch = iniciar
software-update = atualizar
software-edit = editar
software-remove = remover
software-stop = parar
software-paste = colar
cmd-software-down = A próxima linha
cmd-software-up = A linha de cima
cmd-software-enter = Fazer o que a linha faz; num fato, copiá-lo
cmd-software-next-source = Mostrar a próxima fonte, ou a próxima prateleira de ícones
cmd-software-previous-source = Mostrar a fonte anterior, ou a prateleira de ícones anterior
cmd-software-less = O valor anterior desta linha
cmd-software-more = O próximo valor desta linha
cmd-software-install = Instalar só para você, pela fonte mostrada: declarado, para todos os seus computadores o terem — um teste fica só aqui
cmd-software-try-it = Rodar sem instalar
cmd-software-launch = Iniciar o programa
cmd-software-update = Atualizar este agora
cmd-software-edit = Mudar este web app
cmd-software-remove = Remover, depois de uma pergunta
cmd-software-stop = Parar
cmd-software-paste = Colar na busca o que foi copiado
cmd-software-web-new = Um web app a partir de um endereço
cmd-software-icon-path = Uma imagem sua como ícone do web app
cmd-software-file-path = Instalar um arquivo .AppImage, .apk ou .flatpakref
software-type-on-a-row = Para escrever aqui, use { $keys } na linha.
software-type-to-search = Digite um nome, ou o que ele faz, e a busca começa sozinha.
software-reading = Lendo o que há neste computador…
software-on = ligado
software-off = desligado
software-joined = { $first } · { $second }
software-source-all = todas as fontes
software-source-system = o sistema
software-search = Procurar
software-search-empty = digite um nome, ou o que ele faz
software-filter = Mostrando
software-heading-installed = Instalados para você
software-note-yours = O que se instala aqui é só seu. O que você declara também segue você: cada computador seu o instala na atualização dele.
software-note-none = Nada instalado por você ainda. Os programas do sistema estão no lançador — { $keys }.
software-note-none-here = Nada desta fonte.
software-heading-trying = Em experiência
software-heading-looking-after = Manutenção
software-updates = Atualizações, espaço e voltar atrás
software-updates-count = { $count ->
        [one] uma disponível
       *[other] { $count } disponíveis
    }
software-up-to-date = em dia
software-heading-more = Mais
software-web-new = Novo web app
software-web-new-value = a partir de um endereço
software-trials = Experiências
software-trials-count = { $count ->
        [0] nenhuma rodando
        [one] uma rodando
       *[other] { $count } rodando
    }
software-android-state = Sistema Android
software-android-absent = não há nesta máquina
software-android-not-prepared = não preparado
software-android-preparing = preparando
software-android-running = rodando
software-android-stopped = parado
software-advanced = Avançado
software-advanced-value = fontes, permissões, histórico
software-heading-search = Procurando um programa
software-make-web-app = Fazer um web app dele
software-open-once = Abrir uma vez, sem criar
software-install-file = Instalar este arquivo
software-not-a-package-file = Isso não é um arquivo .AppImage, .apk ou .flatpakref.
software-note-type = Digite um nome, ou o que ele faz. Um endereço vira um web app; o caminho de um .AppImage, .apk ou .flatpakref instala esse arquivo.
software-heading-results = Encontrados
software-heading-by-description = Encontrados pelo que fazem
software-note-nothing-found = Nada com esse nome. Outra grafia, ou o que ele faz, pode encontrá-lo.
software-heading-asked = Onde procurou
software-note-offline = Sem rede: mostrando o que este computador já sabe.
software-row-working = Agora
software-queued = esperando na fila
software-working-now = trabalhando
software-working = { $percent } % · { $phase }
software-row-launch = Abrir o programa
software-no-window = sem janela: roda num terminal
software-install-with = pelo { $source }
software-recommended = recomendado
software-size-download = { $size } para baixar
software-row-try = Experimentar sem instalar
software-try-terminal = abre um terminal onde se digita { $program }
software-try-window = abre numa janela e não guarda nada
software-row-traces = O que pode deixar
software-row-network = Rede na experiência
software-row-keep = Ficar com ele de vez
software-keep-value = instala o que foi experimentado, sem baixar de novo
software-row-stop = Parar a experiência
software-row-source = Fonte
software-also-in = também em { $sources }
software-row-version = Versão
software-row-size = Tamanho
software-row-update = Atualização
software-update-to = { $version } está pronta
software-row-permissions = Permissões
software-permissions-value = o que ele pode acessar
software-row-edit = Mudar este web app
software-row-remove = Remover deste computador
software-heading-about = Sobre ele
software-heading-details = Detalhes
software-row-reference = Nome dele na fonte
software-row-publisher = Feito por
software-row-licence = Licença
software-row-site = Site
software-row-needs = Precisa de
software-row-installed-size = Instalado, ocupa
software-row-permission = Pode acessar
software-row-anti-features = O F-Droid avisa sobre
software-note-trial = Experimentar roda agora e não instala nada; ele sai do disco na próxima limpeza.
software-heading-trials = O que você está experimentando
software-try-new = Experimentar algo novo
software-try-new-value = digite o nome
software-note-no-trials = Nada está em experiência.
software-trial-running = aberto há { $minutes } min
software-trial-closed = fechado · no disco até a próxima limpeza
software-heading-web = Um novo web app
software-web-address = Endereço
software-web-no-address = nenhum ainda
software-web-name = Nome
software-web-icon = Ícone no lançador
software-web-no-icon-yet = nenhum escolhido ainda
software-web-profile = Perfil do Chrome
software-web-category = Tipo
software-web-fetching = Lendo o nome e os ícones do site…
software-web-host-name = o nome fica sendo o endereço do site até alguém digitar um
software-web-create = Criar
software-web-save = Salvar as mudanças
software-web-once = Abrir uma vez, como vai ficar
software-web-remove = Remover este web app
software-web-no-icon = Escolha um ícone antes: o lançador precisa de um.
software-note-web = Um web app é o site numa janela sem nada do Chrome; os logins são os do seu perfil do Chrome.
software-heading-icon = O ícone dele
software-shelf = De onde
software-shelf-site = o próprio site
software-shelf-dashboard = Dashboard Icons
software-shelf-collection = a coleção da distro
software-shelf-file = um arquivo seu
software-icon-looking-for = Procurando
software-icon-typed = Escolher uma imagem nos arquivos
software-icon-clipboard = Usar a imagem copiada
software-icon-files = Escolher uma no gerenciador de arquivos
software-note-drop = Ou solte uma imagem aqui.
software-icon-now = Escolhido agora
software-icon-chosen = escolhido
software-note-no-icons = Nenhum ícone com esse nome aqui.
software-heading-android = Android
software-note-android = Apps Android rodam dentro de um sistema Android ao lado deste; ele custa memória enquanto roda.
software-android-image = Imagem
software-note-gapps = O Google Play pede que esta máquina seja registrada uma vez como aparelho não certificado; esta página mostra como, depois de preparar o Android.
software-android-prepare = Preparar o Android
software-note-preparing = Baixando e preparando a imagem do Android — cerca de um gigabyte, uma vez. A fila mostra; nenhuma porcentagem é inventada aqui.
software-heading-register = Registro para o Google Play
software-note-uncertified = A Play Store só funciona depois que este aparelho for registrado na página do Google para aparelhos não certificados: copie o id abaixo e cole lá.
software-android-id = Id do aparelho
software-android-id-unknown = conhecido depois que o Android rodar
software-android-register = Página do Google
software-android-windows = Janelas
software-android-one-per-app = uma por app
software-android-one-for-all = uma para o Android todo
software-note-windows-next-session = A mudança de janelas vale na próxima vez que o Android iniciar.
software-heading-android-apps = Dentro do Android
software-note-no-android-apps = Nenhum app instalado dentro do Android ainda.
software-android-apk = Instalar um arquivo APK
software-android-aurora = Google Play sem conta
software-android-aurora-value = Aurora Store, pelo F-Droid
software-android-erase = Apagar o Android
software-cannot-undo = isto não se desfaz
software-heading-updates = Atualizações
software-updates-all = Tudo
software-web-always-latest = sempre a versão do próprio site
software-follows-system = segue o sistema (nixpkgs { $rev })
software-look-again = Procurar atualizações agora
software-updates-auto = Automaticamente
software-auto-with = com a atualização do sistema — { $keys }
software-heading-space = Espaço
software-clean = Liberar espaço agora
software-clean-generations = Versões antigas da sua lista Nix
software-clean-flatpak = Runtimes Flatpak sem uso
software-clean-appimage = Sobras de AppImage
software-clean-trials = Programas experimentados
software-clean-trials-value = saem sozinhos na próxima limpeza
software-heading-generations = Voltar atrás (Nix)
software-note-no-generations = Sua lista Nix ainda não tem versão anterior.
software-generation = Versão { $n }
software-in-use = em uso
software-older-versions = Versões mais antigas
software-heading-sources = Fontes
software-online = Procurar na internet
software-online-off = desligada, junto com a busca na internet
software-switch-flathub = Flathub
software-switch-fdroid = F-Droid
software-switch-appimagehub = AppImageHub e GitHub
software-switch-dashboard = Dashboard Icons, para web apps
software-nix-index = Índice do Nix
software-prefer = Recomendar primeiro
software-ask-above = Perguntar antes de baixar
software-ask-above-size = { $size } ou mais
software-ask-never = nunca
software-heading-trial-defaults = Ao experimentar
software-default-traces = O que a experiência deixa
software-default-network = Rede na experiência lacrada
software-heading-web-defaults = Novos web apps
software-web-profile-default = Perfil do Chrome que usam
software-profile-last-used = o último que o Chrome usou
software-heading-kept = O que fica guardado
software-consents = Não livres permitidos
software-consents-count = { $count ->
        [0] nenhum
        [one] um
       *[other] { $count }
    }
software-history = Histórico
software-history-value = as últimas centenas de coisas feitas
software-heading-permissions-list = Permissões dos Flatpaks
software-heading-dark = Flatpaks escuros
software-dark-gtk4 = GTK 4 e libadwaita
software-dark-follows = seguem a escolha do sistema
software-dark-gtk3 = GTK 3
software-dark-gtk3-value = escuros, pelo Adwaita-dark do sistema
software-dark-gtk3-missing = claros: a extensão Adwaita-dark não está instalada
software-dark-qt = Qt e KDE
software-dark-qt-value = sem as configurações Qt da sessão
software-dark-qt-leaks = as configurações Qt da sessão chegam a eles
software-dark-forced = Forçando um tema
software-dark-none-forced = nenhum app força o GTK_THEME
software-dark-forces = força o GTK_THEME — veja as permissões
software-note-gtk-theme = O GTK_THEME faz um Flatpak perder o próprio estilo; nada aqui o define, e um app que o carrega aparece em vermelho.
software-heading-permissions = O que { $app } pode acessar
software-note-no-permissions = Não pede nada além da própria janela.
software-note-permissions-restart = Uma mudança chega ao app na próxima vez que ele abrir.
software-heading-asks = O que ele pede
software-gtk-theme-forced = forçado nele: perde o estilo próprio
software-sure-reset = Voltar { $app } ao que ele pede? Toda mudança feita aqui vai embora.
software-yes-reset = Sim, restaurar
software-sure-unset-gtk-theme = Tirar o GTK_THEME de { $app }? Ele volta ao estilo próprio, e nada aqui põe o GTK_THEME de volta.
software-yes-unset-gtk-theme = Sim, tirar
software-heading-consents = Pacotes não livres que você permitiu
software-note-consents = O Nix instala estes mesmo sem licença livre, e os atualiza com esse mesmo sim.
software-note-no-consents = Nenhum ainda.
software-allowed = permitido
software-heading-history = O que foi feito
software-note-no-history = Nada feito ainda.
software-sure-remove = Remover { $app }?
software-yes-remove = Sim, remover e manter o que ele guardou
software-yes-remove-data = Sim, e apagar também o que ele guardou
software-sure-web-remove = Remover o web app { $app }? O site e o seu login continuam no Chrome.
software-yes-remove-web = Sim, remover o web app
software-sure-unverified = Nada garante o que { $app } baixa. Instalar mesmo assim?
software-yes-trust = Sim, confio em quem fez
software-sure-arm = { $app } é feito só para chips de celular e pode não rodar aqui. Instalar mesmo assim?
software-yes-arm = Sim, instalar mesmo assim
software-sure-rollback = Voltar para a versão { $n } da sua lista Nix?
software-yes-rollback = Sim, voltar
software-sure-clean = Liberar o que nada usa? As versões antigas vão embora, e voltar para elas deixa de ser possível.
software-yes-clean = Sim, liberar
software-sure-android = Preparar o Android { $image }? Baixa uma vez e fica até você apagar.
software-yes-android = Sim, preparar
software-sure-erase-android = Apagar o Android, os apps e tudo o que eles guardaram?
software-yes-erase = Sim, apagar
software-sure-foreign = { $file } não foi verificado por nenhum catálogo e roda com as suas permissões. Instalar?
software-sure-leave = Sair sem salvar?
software-yes-leave = Sim, descartar as mudanças
software-no-such-level = “{ $words }” não nomeia nenhuma página dos programas.
software-no-copied-image = Nenhuma imagem foi copiada.
software-image-not-kept = Não foi possível gravar a imagem copiada num arquivo.

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-packages = Programas para instalar
software-pick-package = Um arquivo para instalar — um AppImage, um APK ou uma referência Flatpak

## file info editing — everything the info panel can change

software-consent-flathub = A busca do Flathub, as páginas dos programas dele e as figuras.
software-consent-fdroid = O índice do F-Droid, os ícones dele e os pacotes que ele serve.
software-consent-appimagehub = O catálogo de AppImages, e os lançamentos do GitHub para onde ele aponta.
software-consent-dashboard = O índice de ícones de onde sai a figura de um web app.
cmd-software-sign = Assinar com a senha dos pertences uma mudança do que você declarou, para que ela chegue agora a esta máquina e às outras
ask-software-sign = Senha dos pertences, para assinar esta mudança
software-declared-door = Declarados por você
software-declared-door-declined = Recusados por você
software-declared-count = { $count ->
        [one] um programa
       *[other] { $count } programas
    }
software-declared-count-out = { $count ->
        [one] um deixado de fora aqui
       *[other] { $count } deixados de fora aqui
    }
software-declared-declined-count = { $count ->
        [one] um mantido desinstalado
       *[other] { $count } mantidos desinstalados
    }
software-declared-none-yet = nada ainda
software-declared-none-declined-yet = nada
software-declared-note = O que você declara vai para todos os seus computadores: cada atualização o instala e o mantém, e ele viaja com os seus pertences.
software-declared-not-reported = O sistema deste computador foi construído antes de conhecer as suas declarações: a próxima atualização as aplica.
software-declared-none = Nada declarado ainda. Instalar só para você, na página de um programa, o declara.
software-declared-android-pending = Alguns apps Android declarados esperam o Android iniciar, e entram então.
software-declared-heading-pins = Downloads
software-declared-refresh = Procurar downloads mais novos
software-declared-refresh-value = a próxima atualização os constrói, depois de testar cada um
software-declared-refresh-one = Procurar um download mais novo
software-declared-refresh-one-value = a próxima atualização o constrói, depois de testá-lo
software-declared-file-declared = seus programas declarados
software-declared-file-declined = seus programas recusados
software-declared-unreadable-newer = O arquivo de { $file } foi escrito por uma distro mais nova, e ficou como estava: atualize este computador para lê-lo.
software-declared-unreadable-broken = O arquivo de { $file } não se lê, e ficou como estava: a próxima sincronização dos seus pertences pode consertá-lo.
software-declared-gone = Ele não está mais na sua declaração.
software-declared-row-here-now = Aqui
software-declared-row-said = O que o sistema disse
software-declared-row-why = Por quê
software-declared-row-on = Vai para
software-declared-all-computers = todos os seus computadores
software-declared-everywhere = em todos os seus computadores
software-declared-row-pin = Versão fixada
software-declared-row-download = Download
software-declared-row-edited = Última mudança
software-declared-edited-on = { $when }, em { $machine }
software-declared-heading-fixes = O que dá para fazer
software-declared-row-program = O programa
software-declared-program-value = abre a página dele
software-declared-note-waiting = A próxima atualização o instala aqui: não há nada a fazer.
software-declared-note-arriving = Ele chega sozinho, depois do sistema, como uma parte: a página das partes diz como ele está.
software-declared-row-also-here = Também neste computador
software-declared-note-source-off = Este computador não tem suporte à origem dele: um perfil reduzido não tem Android nem AppImages.
software-declared-note-update-me = Atualize este computador: uma distro mais nova escreveu esta declaração.
software-declared-row-replace = Procurar outro
software-declared-replace-value = busca programas parecidos
software-declared-row-forget = Remover de vez
software-declared-forget-value = aqui, e da sua declaração
software-declared-note-declined = Os programas da distro que você mantém desinstalados, aqui ou em todos os seus computadores: cada atualização os deixa de fora.
software-declared-none-declined = Nada recusado: todo programa da distro está instalado.
software-declared-heading-distro = Os programas da distro
software-declared-distro-installed = instalado com o sistema
software-declared-row-group = Grupo
software-declared-not-on-this-machine = O perfil deste computador não o traz.
software-declared-row-declined = Recusado
software-declared-row-bring-back = Trazer de volta
software-declared-bring-back-value = em todos os computadores, e aqui na hora
software-declared-bring-back-here = { $others ->
        [0] aqui na hora
        [one] aqui na hora; o seu outro computador continua sem ele
       *[other] aqui na hora; os seus { $others } outros computadores continuam sem ele
    }
software-declared-back-elsewhere = Cada computador que o recusou o traz de volta por conta própria; daqui, ele volta em todos de uma vez.
software-declared-row-bring-back-everywhere = Trazer de volta em todos os computadores
software-declared-row-decline-here = Manter desinstalado aqui
software-declared-row-decline-everywhere = Manter desinstalado em todos os seus computadores
software-declared-row-for-me = Instalar só para você
software-declared-row-here = Instalar só neste computador
software-declared-here-value = não declarado: os seus outros computadores não o recebem
software-declared-row-keep = Manter em todos os seus computadores
software-declared-keep-value = declarado: cada computador seu o instala na atualização dele
software-declared-row-declared = Declarado
software-declared-sure-forget = Remover { $app } de vez?
software-declared-forget-here = Ele sai deste computador e da sua declaração.
software-declared-forget-elsewhere = Os seus outros computadores o removem na próxima atualização deles.
software-declared-forget-data = O que ele guardou fica, a não ser que você escolha apagar.
software-declared-yes-forget = Sim, remover de vez
software-declared-sure-decline-here = Manter { $app } desinstalado neste computador?
software-declared-sure-decline-everywhere = Manter { $app } desinstalado em todos os seus computadores?
software-declared-decline-rebuild = Este computador é reconstruído agora, sem ele.
software-declared-decline-others = Os seus outros computadores o deixam de fora na próxima atualização deles.
software-declared-decline-back = Recusados por você o traz de volta.
software-declared-yes-decline = Sim, manter desinstalado
software-declared-sure-bring-back-everywhere = Trazer { $app } de volta em todos os computadores?
software-declared-back-everywhere-others = Todo computador que o recusou o instala de novo na próxima atualização — inclusive um que você não tem mais, cuja recusa nada mais desfaria.
software-declared-back-everywhere-here = Este computador não muda: ele nunca o deixou de fora.
software-declared-yes-bring-back-everywhere = Sim, trazer de volta em todos
software-declared-registry-broken = A lista de programas da distro deste computador não se lê, então nenhum deles aparece aqui. A próxima atualização deste computador a escreve de novo.
software-declared-registry-newer = A lista de programas da distro deste computador foi escrita por uma distro mais nova, então nenhum deles aparece aqui: atualize este computador.
software-declared-returning = Não está mais recusado aqui: a próxima atualização deste computador o traz de volta.
software-declared-web-edit = Ele é declarado: salvar muda a sua declaração, e os seus outros computadores recebem a mudança na próxima atualização deles.
