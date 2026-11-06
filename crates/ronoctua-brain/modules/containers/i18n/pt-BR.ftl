# The containers module's own texts: its name and the line under it, the hint
# of its mark and its page. The containers' VIEW speaks in the same words from
# the shared files (its `namedBy` says which); these are the module's alone.
brain-task-containers = Containers rodando
brain-task-containers-does = Enquanto algum dos seus containers está rodando, uma marca na modeline mostra quantos; um clique nela abre os containers. Ele só lê o que o runtime de containers guarda para cada container rodando, então não custa nada enquanto nenhum roda.
brain-task-containers-brief = Mostra na modeline quantos dos seus containers estão rodando.
containers-mark-running = { $count ->
        [one] Um container está rodando
       *[other] { $count } containers estão rodando
    }
containers-heading-running = Rodando agora
containers-row-running = Containers rodando
containers-row-container = Container
containers-row-unwatched = Sem vigia
