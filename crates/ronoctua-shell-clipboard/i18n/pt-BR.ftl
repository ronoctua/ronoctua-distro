# The texts `ronoctua-shell-clipboard` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Comandos do compositor — §13, "toda ação do SO tem um comando"

# --- Clipboard ---
cmd-clipboard-open = Área de transferência
cmd-clipboard-copy-of-new = Copiar de novo
cmd-clipboard-remove = Remover do histórico
cmd-clipboard-pin = Fixar / desfixar
cmd-clipboard-qrcode = Gerar QR code
cmd-clipboard-clear = Limpar o histórico
clipboard-empty = Nada foi copiado ainda
clipboard-history = Histórico
clipboard-item = Item
clipboard-heading-sync = Sincronização
clipboard-content-text = texto
clipboard-sync-state = sincronizando
clipboard-sync-server = servidor
clipboard-sync-account = conta
clipboard-sync-forget = esquecer esta conta
clipboard-sync-images = imagens também, não só texto
clipboard-sync-notify = avisar quando algo chegar
clipboard-sync-cap = maior cópia que sai daqui
clipboard-sync-cap-server = { $size }, que é o limite do servidor
clipboard-sync-cap-unset = o que o servidor aguentar
clipboard-sync-salt = sal (o mesmo em todas as máquinas)
clipboard-sync-salt-none = nenhum
clipboard-sync-rounds = rodadas com que a chave é derivada
clipboard-setting-on = ligado
clipboard-setting-off = desligado
clipboard-mark = marca na modeline
clipboard-mark-never = nunca
clipboard-mark-when-off = quando não estiver sincronizando
clipboard-mark-when-on = enquanto estiver sincronizando
clipboard-sync-on = sim
clipboard-sync-off = agora não
clipboard-sync-unset = não definido
clipboard-down = próximo
clipboard-up = anterior
clipboard-copy-of-new = copiar de novo
clipboard-remove = remover
clipboard-pin = fixar
clipboard-qrcode = código QR
clipboard-clear = limpar
clipboard-sync = sincronizar
clipboard-more = mudar
clipboard-less = voltar
cmd-clipboard-more = Gira este ajuste para a frente
cmd-clipboard-less = Gira este ajuste para trás
cmd-clipboard-down = Próxima entrada
cmd-clipboard-up = Entrada anterior
cmd-clipboard-sync = Sincronizar a área de transferência entre máquinas
ask-clipboard-server = O endereço do servidor ClipCascade
ask-clipboard-account = A conta nesse servidor
ask-clipboard-password = A senha dessa conta
clipboard-pinned = fixada
clipboard-content = Conteúdo
clipboard-types = Tipos
clipboard-size = Tamanho
echo-qr-large = O conteúdo é grande demais para um QR code
clipboard-image = Imagem
clipboard-unreadable = Copiado, e ilegível aqui: { $kind }

## file info editing — everything the info panel can change

# A janela do clipboard: a segunda tela do histórico do rodapé, seguido
# pelo socket de controle.
group-clipboard = clipboard
cmd-clipboard-window = O histórico do clipboard numa janela
clipboard-window-waiting = O histórico é do rodapé, e está a caminho.
clipboard-window-no-footer = O rodapé não está rodando, e o histórico fica nele: nada a mostrar.
clipboard-no-room = Não sobra espaço na tela para desenhá-lo: role de volta para cima, ou dê mais linhas à lista.
