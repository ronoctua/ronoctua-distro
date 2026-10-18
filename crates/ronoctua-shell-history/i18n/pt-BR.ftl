# The texts `ronoctua-shell-history` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# O ATUIN — O HISTÓRICO DO SHELL, E SE ELE SAI DESTA MÁQUINA.
#
# O `nix/modules/home.nix` recusava a sincronização por escrito, e o dono pediu
# a outra resposta. O que não mudou é o comando com token no argv, e é por isso
# que o `secrets_filter` e o `history_filter` são linhas DESENHADAS desta vista
# em vez de padrões que ninguém lê.
atuin-title = Histórico do shell
atuin-heading-glance = Num relance
atuin-heading-account = A conta
atuin-heading-leaving = O que sai desta máquina
atuin-heading-search = A busca
atuin-heading-keys = As teclas
atuin-heading-advanced = Raramente mexido
atuin-advanced-about = Estes mudam como a busca se comporta. Os padrões são o que a maioria quer.
atuin-absent = o atuin não está instalado nesta máquina
atuin-not-logged-in = sem conta
atuin-state = o histórico
atuin-on = ligado
atuin-off = desligado
atuin-syncing = sincronizando
atuin-not-syncing = fica nesta máquina
atuin-server = servidor
atuin-account = conta
atuin-entries = entradas
atuin-last-sync = última sincronização
atuin-mark = marca na modeline
atuin-mark-never = nunca
atuin-mark-when-off = quando não estiver sincronizando
atuin-mark-when-on = enquanto estiver sincronizando
atuin-never = ainda não
atuin-not-set = não definido
atuin-sync-now = sincronizar agora
atuin-sync = sincronizar agora
atuin-log-in = entrar num servidor
atuin-log-out = sair e esquecer a chave
atuin-show-key = mostrar a chave de criptografia
atuin-key-about = Esta chave é o que decifra o seu histórico. O servidor nunca a vê, e uma segunda máquina não sincroniza sem ela.
atuin-key-ask = Mostrar a chave na tela?
atuin-from-flake = definido pela configuração do sistema
atuin-down = próxima
atuin-up = anterior
atuin-enter = mudar
atuin-more = mais
atuin-less = menos
atuin-back = voltar
atuin-drawer = a gaveta
cmd-atuin-open = Histórico do shell
cmd-atuin-window = Histórico do shell numa janela
cmd-atuin-sync = Sincronizar o histórico do shell agora
cmd-atuin-down = Próxima linha
cmd-atuin-up = Linha anterior
cmd-atuin-enter = Mudar este ajuste
cmd-atuin-more = Girar este ajuste para a frente
cmd-atuin-less = Girar este ajuste para trás
cmd-atuin-back = Sair do histórico do shell
ask-atuin-server = O endereço do servidor de sincronização
ask-atuin-account = A conta nesse servidor
ask-atuin-password = A senha dessa conta
ask-atuin-key = A chave da sua primeira máquina — vazio se esta for a primeira
ask-atuin-setting = O valor novo
group-atuin = histórico
# The table, from `assets/atuin-options.nix` through the generator.
atuin-opt-accept-past-line-end = deixar o cursor passar do fim da linha
atuin-opt-auto-sync = sincronizar sozinho
atuin-opt-command-chaining = ler `a && b` como dois comandos
atuin-opt-common-prefix = prefixos para ignorar
atuin-opt-common-subcommands = subcomandos a manter
atuin-opt-ctrl-n-shortcuts = Ctrl+1..9 escolhem um resultado
atuin-opt-cwd-filter = pastas nunca gravadas
atuin-opt-daemon-enabled = rodar o daemon em segundo plano
atuin-opt-daemon-sync-frequency = o daemon sincroniza a cada
atuin-opt-enter-accept = Enter executa, em vez de editar
atuin-opt-exit-mode = sair da busca devolve
atuin-opt-filter-mode = buscar em
atuin-opt-history-filter = comandos nunca gravados
atuin-opt-ignored-commands = comandos a descartar
atuin-opt-inline-height = altura da busca
atuin-opt-invert = desenhar a busca de cabeça para baixo
atuin-opt-keymap-mode = as teclas da busca
atuin-opt-local-timeout = desistir do banco local após
atuin-opt-max-preview-height = altura da prévia
atuin-opt-network-connect-timeout = desistir de conectar após
atuin-opt-network-timeout = desistir do servidor após
atuin-opt-prefers-reduced-motion = menos movimento
atuin-opt-prefix-key = a tecla de prefixo da busca
atuin-opt-scroll-context-lines = linhas mantidas ao redor do cursor
atuin-opt-scroll-exits = rolar além do fim sai
atuin-opt-search-mode = como a consulta casa
atuin-opt-search-style = como a busca é desenhada
atuin-opt-secrets-filter = nunca gravar comando com cara de segredo
atuin-opt-show-help = mostrar as dicas de tecla
atuin-opt-show-preview = mostrar o comando inteiro
atuin-opt-show-tabs = mostrar as abas
atuin-opt-store-failed = gravar comandos que falharam
atuin-opt-strip-trailing-whitespace = tirar espaços no fim
atuin-opt-sync-frequency = sincronizar a cada
atuin-opt-sync-records = usar o armazenamento de registros
atuin-opt-up-key-filter = a seta para cima busca em
atuin-opt-update-check = procurar versão nova
atuin-opt-word-chars = o que conta como palavra
atuin-opt-word-jump-mode = como o salto de palavra funciona
atuin-opt-workspaces = tratar um repositório git como espaço de trabalho
atuin-unit-lines = linhas
atuin-unit-seconds = segundos
