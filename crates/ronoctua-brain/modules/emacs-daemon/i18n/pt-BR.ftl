# The emacs-daemon module's own texts: its name and the line under it, the
# two things it may say on the strip, its one action, and what a refused
# start says.
brain-task-emacs-daemon = Vigiar o daemon do Emacs
brain-task-emacs-daemon-does = Algumas vezes por dia, confere se o servidor do Emacs que guarda a sua sessão e a sua configuração está rodando. Também procura um Emacs rodando SEM esse servidor: na tela parece igual, mas ele carregou uma cópia própria, os buffers dele não são os da sua sessão, e fechá-lo os perde. Ele só lê a lista de processos, e a única coisa que chega a iniciar é o serviço do Emacs da própria distro.
brain-task-emacs-daemon-brief = Confere se o servidor do Emacs que guarda sua sessão está rodando, e avisa de um Emacs rodando sem ele.
emacs-daemon-says-no-daemon = O daemon do Emacs não está rodando. Cada janela do editor vai carregar a configuração inteira de novo, e nenhuma delas compartilha os seus buffers.
emacs-daemon-says-not-the-client = Este Emacs não está ligado ao daemon: ele carregou uma cópia própria da configuração, e os buffers são só dele. Feche e abra o arquivo de novo pelo gerenciador de arquivos, que o abre no Emacs do daemon.
cmd-emacs-daemon-start = Iniciar
emacs-daemon-why-refused = Não deu para iniciar o daemon do Emacs; veja: journalctl --user -u emacs
