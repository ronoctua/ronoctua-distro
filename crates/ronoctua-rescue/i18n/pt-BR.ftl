# The texts `ronoctua-rescue` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Rescue console — Ctrl+Alt+Del

rescue-title = Resgate
rescue-section-state = Estado
rescue-section-actions = Ações
rescue-section-windows = Janelas
rescue-section-terminals = Terminal
rescue-section-config = Configuração
rescue-section-log = Registro
rescue-section-system = Sistema

## Rescue console — the actions

rescue-restart-footer = Reiniciar o rodapé
rescue-restart-compositor = Reiniciar o compositor
rescue-restart-session = Reiniciar a sessão gráfica
rescue-restart-audio = Reiniciar o áudio
rescue-restore-keyboard = Restaurar o teclado
rescue-close-window = Fechar a janela em foco
rescue-refresh-fonts = Reconstruir o cache de fontes
rescue-open-terminal = Abrir um terminal
rescue-kill-window = Matar a janela escolhida
rescue-kill-hog = Matar quem mais come memória
rescue-edit-host = Editar a configuração desta máquina
rescue-rebuild-switch = Aplicar a configuração
rescue-rollback-now = Voltar para a geração anterior
rescue-rollback-boot = Geração anterior no próximo boot
rescue-restart-network = Reiniciar a rede
rescue-reboot = Reiniciar a máquina
rescue-poweroff = Desligar a máquina

## Rescue console — what it says

rescue-keys = ↑↓ escolhe · Enter roda · Tab troca de seção · Esc sai
rescue-running = rodando…
rescue-ran = pronto
rescue-failed = falhou
rescue-confirm = A mesma tecla de novo confirma; Esc cancela.
rescue-needs-password = pede a sua senha
rescue-read-only = leitura
rescue-generated-by = Gerado por { $source }. Editar aqui some no próximo rebuild.
rescue-no-windows = O compositor não respondeu — ou não há janela nenhuma.
rescue-empty = nada a mostrar
rescue-back = Sair do terminal devolve esta tela.
rescue-generation = geração { $number }
rescue-door-console = console
rescue-door-window = janela
