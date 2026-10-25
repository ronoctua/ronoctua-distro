# Os textos do vigia das partes: o nome e a linha abaixo dele, as marcas, as
# duas palavras no minibuffer e a página.
brain-task-parts-watch = Partes a caminho
brain-task-parts-watch-does = Enquanto alguma parte da máquina — o que chega depois da instalação, cada uma por conta própria — está a caminho ou precisa de você, uma marca na modeline diz quantas, e um clique nela abre a página das partes. Ele lê os registros que o realizador das partes escreve, então não roda nada e não custa nada enquanto todas estão aqui.
parts-watch-mark-arriving = { $count ->
        [one] Uma parte está a caminho
       *[other] { $count } partes estão a caminho
    }
parts-watch-mark-needs-you = { $count ->
        [one] Uma parte precisa de você
       *[other] { $count } partes precisam de você
    }
notif-parts-watch-arrived-summary = { $name } chegou
notif-parts-watch-arrived-body = Já pode ser usado.
notif-parts-watch-needs-you-summary = { $name } precisa de você
notif-parts-watch-needs-you-body = O download dela continua falhando. Ela segue sendo tentada, cada vez mais espaçado; a página das partes tem outros jeitos de trazê-la.
parts-watch-heading-parts = Partes
parts-watch-row-unwatched = Sem vigia
parts-watch-phase-here = aqui
parts-watch-phase-arriving = a caminho
parts-watch-phase-waiting = esperando uma rede que possa usar
parts-watch-phase-needs-you = precisa de você
parts-watch-phase-unwanted = não é mais querida
