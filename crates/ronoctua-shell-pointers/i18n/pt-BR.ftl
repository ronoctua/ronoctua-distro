# The texts `ronoctua-shell-pointers` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# =====================================================================
# POINTERS — `LEADER s i`: mice, touchpads and every cursor
# =====================================================================
group-pointers = ponteiros
pointers-title = Mouses e touchpads
cmd-pointers-open = Mouses e touchpads, no rodapé
cmd-pointers-window = Mouses e touchpads, numa janela
cmd-pointers-up = Linha anterior
cmd-pointers-down = Próxima linha
cmd-pointers-enter = Mudar isto, ou usar
cmd-pointers-less = O valor anterior
cmd-pointers-more = O próximo valor
cmd-pointers-forget = Esquecer este dispositivo
pointers-up = sobe
pointers-down = desce
pointers-enter = muda
pointers-less = anterior
pointers-more = próximo
pointers-forget = esquece
pointers-heading-cursors = Cursores
pointers-heading-mice = Mouses
pointers-heading-touchpads = Touchpads
pointers-heading-general = Todos os cursores
pointers-heading-away = Desconectados
pointers-about-cursors = Cada mouse e cada touchpad pode mover um cursor próprio, cada um com a sua cor, cada um focando a sua janela. Ponha dois dispositivos num cursor só para que o movam juntos.
pointers-about-mice = O que se ajusta em “Todo mouse” vale para cada mouse que não ajusta aquilo para si.
pointers-about-touchpads = O que se ajusta em “Todo touchpad” vale para cada touchpad que não ajusta aquilo para si.
pointers-about-general = O foco segue o ponteiro para todos os cursores ao mesmo tempo: o compositor tem um ajuste só para todos eles.
pointers-about-away = Dispositivos já vistos, guardados com o cursor e os ajustes deles para quando voltarem.
pointers-about-twins = Dois dispositivos do mesmo modelo têm o mesmo nome para o compositor, então sempre movem um cursor juntos.
pointers-detect = Dispositivo novo ganha cursor próprio
pointers-together = Todo mouse e touchpad move um cursor só
pointers-own-cursor = um cursor próprio
pointers-emulated = Os ponteiros da própria máquina virtual se movem
pointers-about-emulated = O tablet e os mouses da máquina virtual trazem o ponteiro do hospedeiro para dentro. Com os mouses do hospedeiro entregues a esta máquina eles ficam desligados, para nenhum cursor ficar parado; ligue-os para levar o ponteiro para fora da janela de novo.
pointers-not-drawn = não desenhado: nada nele se move
pointers-cursor = Cursor { $number }
pointers-cursor-on = Este cursor está ligado
pointers-colour = Cor
pointers-colour-value = cor { $number }
pointers-on-cursor = Move
pointers-on-cursor-value = o cursor { $number }
pointers-new-cursor = um cursor próprio
pointers-every-mouse = Todo mouse
pointers-every-touchpad = Todo touchpad
pointers-device-on = Este dispositivo está ligado
pointers-no-devices = Nenhum mouse ou touchpad está conectado.
pointers-no-compositor = O compositor não está respondendo, então nenhum dispositivo pode ser lido ou ajustado.
pointers-settings-failed = O pointers.json não pôde ser lido e foi deixado como está: as escolhas mostradas são as de fábrica até ele ser consertado ou removido.
pointers-setting-events = Desligado com um mouse conectado
pointers-setting-speed = Velocidade
pointers-setting-profile = Aceleração
pointers-setting-natural = Rolagem natural
pointers-setting-left = Canhoto
pointers-setting-tap = Tocar para clicar
pointers-setting-tap-map = Toque com dois e três dedos
pointers-setting-tap-drag = Tocar e arrastar
pointers-setting-drag-lock = O arrasto sobrevive a levantar o dedo
pointers-setting-finger-drag = Arrastar com os dedos
pointers-setting-click = Apertar clica com
pointers-setting-click-map = Aperto com dois e três dedos
pointers-setting-middle = Esquerdo e direito juntos são o botão do meio
pointers-setting-scroll = Rola com
pointers-setting-scroll-button = Botão de rolar
pointers-setting-scroll-lock = O botão de rolar fica apertado
pointers-setting-dwt = Desligado enquanto se digita
pointers-setting-dwtp = Desligado enquanto se usa o trackpoint
pointers-setting-factor = Distância da rolagem
pointers-size = Tamanho
pointers-size-value = { $pixels } px
pointers-hide-after = Esconder parado por
pointers-never = nunca
pointers-seconds = { $seconds } s
pointers-hide-typing = Esconder enquanto se digita
pointers-focus = O foco segue o ponteiro
pointers-focus-no = não, um clique foca
pointers-focus-yes = sim
pointers-focus-always = sempre
pointers-about-typing = A borda colorida marca a janela em que os teclados digitam, e só o clique do cursor 1 a move. Escolha “o último cursor a escolher uma janela” e o clique de todo cursor a move — e a digitação junto: o compositor desenha a borda só para o foco dos teclados.
pointers-typing = A digitação vai para
pointers-typing-first = a janela do cursor 1
pointers-typing-cursor = a janela do cursor { $number }
pointers-typing-latest = o último cursor a escolher uma janela
pointers-heading-keyboards = Teclados
pointers-about-keyboards = No modo Independentes cada teclado digita por um cursor próprio, com o seu layout e a sua tecla leader. Isso se ajusta na tela dos teclados.
pointers-open-keyboards = Abrir os ajustes dos teclados
pointers-flat = constante
pointers-adaptive = mais rápido com a mão mais rápida
pointers-lrm = direito, depois o do meio
pointers-lmr = o do meio, depois o direito
pointers-lock-off = não
pointers-lock-on = por um instante
pointers-lock-sticky = até o próximo toque
pointers-fingers-off = desligado
pointers-fingers-three = três dedos
pointers-fingers-four = quatro dedos
pointers-click-none = sem botões
pointers-click-areas = onde se aperta
pointers-click-fingers = quantos dedos apertam
pointers-scroll-none = nada
pointers-scroll-two = dois dedos
pointers-scroll-edge = a borda
pointers-scroll-button = um botão segurado
pointers-button-none = nenhum
pointers-button-middle = o botão do meio
pointers-button-side = o botão lateral
pointers-button-extra = o botão extra
pointers-factor-value = × { $factor }
pointers-speed-value = { $speed }
pointers-is-first = o cursor em que os teclados estão
