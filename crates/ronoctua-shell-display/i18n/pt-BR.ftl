# The texts `ronoctua-shell-display` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Telas — os monitores, a imagem deles e a luz (LEADER s d)

cmd-display-open = Telas
cmd-display-window = Telas numa janela
cmd-display-down = Uma linha abaixo
display-back = voltar
cmd-display-up = Uma linha acima
cmd-display-less = O valor sob o cursor, um passo atrás
cmd-display-more = O valor sob o cursor, um passo adiante
cmd-display-enter = Abrir, copiar ou responder
cmd-display-automatic = Deixar o compositor escolher este ajuste
cmd-display-forget = Esquecer o que foi escolhido para esta tela
cmd-display-arrange = Onde as telas ficam
group-display = Telas
display-down = abaixo
display-up = acima
display-less = menos
display-more = mais
display-leftwards = para a esquerda
display-rightwards = para a direita
display-enter = abrir
display-enter-answers = responder
display-enter-copies = copiar
display-automatic = deixar escolher
display-forget = esquecer
display-arrange = arrumar
display-title = Telas
display-heading-screens = Telas
display-heading-every = Todas elas
display-heading-picture = A imagem
display-heading-light = A luz
display-heading-session = A sessão
display-heading-arrangement = Onde elas ficam
display-heading-forgetting = Esquecer os ajustes desta tela?
display-heading-keeping = Fica assim?
display-nothing-here = Nenhuma tela conectada.
display-reading = Perguntando ao compositor…
display-no-mode = o compositor não informa nenhum
display-no-thread-short = as telas não podem ser lidas aqui
display-no-watch = esta porta não vê o arquivo mudar
display-no-thread = As telas não podem ser lidas nesta máquina; nada aqui vai mudar.
display-no-compositor = Não há compositor para perguntar, então não há o que mostrar.
display-went-away = Essa tela sumiu.
display-settings-failed = O `display.json` não parseia, então nada do que foi escolhido aparece aqui. O arquivo está intacto — conserte ou remova.
display-one-screen-only = Há uma tela só, então não há o que arrumar.
display-this-one = { $name } — esta
display-arrangement = Disposição
display-text-size = Tamanho do texto
# Sob o cursor na linha do tamanho: o que a mudança alcança na hora e o que
# espera. Diz o que foi MEDIDO — ver docs/PENDING.md — e nunca promete um
# aplicativo que ninguém observou.
display-text-reaches = o shell e as janelas dele seguem na hora; aplicativos GTK e Qt seguem conforme redesenham
display-text-reopens = { $names } ficam no tamanho antigo até reabrir
display-fact-connector = Soquete
display-fact-filed = Os ajustes dela seguem
display-fact-size = Tela
display-filed-by-itself = o monitor, em qualquer soquete que ele esteja
display-filed-silent = o soquete — o monitor não diz nada sobre si
display-filed-no-serial = o soquete — o monitor não dá série, então um igual responderia por ele
display-on = Acionada
display-resolution = Resolução
display-rate = Taxa de atualização
display-scale = Escala
display-turn = Giro
display-place = Posição
display-vary = Taxa variável
display-brightness = Brilho
display-first = Abre com o foco aqui
display-vary-cannot = o hardware não consegue
display-vary-always = sempre
display-vary-on-demand = só quando uma janela pede
display-light-is-roots = o arquivo é do root — a regra que o abre não rodou aqui
display-light-on-its-buttons = O brilho deste monitor está nos botões dele.
display-turn-and-picture = { $set } (a imagem está { $picture })
display-turn-normal = normal
display-turn-rotated-90 = um quarto de volta
display-turn-rotated-180 = de cabeça para baixo
display-turn-rotated-270 = três quartos de volta
display-turn-flipped = espelhada
display-turn-flipped-90 = espelhada, um quarto de volta
display-turn-flipped-180 = invertida na vertical
display-turn-flipped-270 = espelhada, três quartos de volta
display-value-yes = sim
display-value-no = não
display-value-off = não está sendo acionada
display-value-automatic = o compositor escolhe
display-value-percent = { $percent }%
display-value-rate = { $hz } Hz
display-value-inches = { $inches } polegadas
display-value-place = x={ $x } y={ $y }
display-value-screens = { $n ->
        [0] nenhuma tela
        [one] uma tela
       *[other] { $n } telas
    }
display-left-of = à esquerda de { $other }
display-right-of = à direita de { $other }
display-above = acima de { $other }
display-below = abaixo de { $other }
display-arrange-how = As setas levam a tela sob o cursor até a próxima borda de uma vizinha — ao lado dela, alinhada com ela — e Shift com ↑ ou ↓ a põe acima ou abaixo.
display-ask = { $what } em { $screen }.
display-ask-seconds = Volta sozinho em { $seconds }s.
display-ask-forget = Tudo o que foi escolhido para { $screen } volta ao que o compositor decidir.
display-ask-nothing-yet = Nada mudou ainda.
display-leave-it = Deixar como está
display-forget-it = Esquecer tudo
display-place-and-picture = { $set } (está em { $picture })
display-scale-and-picture = { $set } (a imagem está em { $picture })
display-screen-gone = { $screen } não está mais aí; a mudança foi junto.
display-screen-gone-alone = { $screen } não está mais aí; nada tinha sido mudado.
display-only-one-lit = sim — e a única tela acesa
display-dark-not-ours = não — e nada aqui a desligou
display-only-one-offered = a única que este monitor oferece
display-put-it-back = Voltar como estava
display-keep-it = Manter assim
display-what-power-off = A tela foi desligada
display-what-resolution = A resolução mudou
display-what-rate = A taxa de atualização mudou
display-what-scale = A escala mudou
display-what-turn = A imagem foi girada

## file info editing — everything the info panel can change

# As telas (`display.rs`): o arranjo vertical, a profundidade de cor, os
# valores gravados de uma tela apagada, e toda recusa com nome.
display-rise = pôr acima
display-sink = pôr abaixo
cmd-display-rise = A tela sob o cursor para cima, acima da vizinha
cmd-display-sink = A tela sob o cursor para baixo, abaixo da vizinha
display-depth = Profundidade de cor
display-depth-bits = { $bits } bits por cor
display-depth-asked = Profundidade pedida
display-dark-shows-filed = O compositor não está desenhando nesta tela: cada linha mostra o que está gravado para ela, e volta quando ela acender.
