# The emacs-daemon module's own texts: its name and the line under it, the
# two things it may say on the strip, its one action, and what a refused
# start says.
brain-task-emacs-daemon = Vigiar o daemon do Emacs
brain-task-emacs-daemon-does = Procura, algumas vezes por dia, o servidor do Emacs que segura a sua sessão e a sua configuração — e um Emacs que está rodando SEM ele, que parece a mesma coisa na tela e não é: ele carregou uma cópia própria, os buffers não são os da sessão, e fechar perde eles. Lê a tabela de processos e nada mais, e a única coisa que ele chega a iniciar é o serviço da própria distro.
emacs-daemon-says-no-daemon = O daemon do Emacs não está rodando. Cada janela do editor vai carregar a configuração inteira de novo, e nenhuma delas compartilha os seus buffers.
emacs-daemon-says-not-the-client = Este Emacs não está ligado ao daemon: ele carregou uma cópia própria da configuração, e os buffers são só dele. Feche e abra o arquivo de novo pelo gerenciador de arquivos, que o abre no Emacs do daemon.
cmd-emacs-daemon-start = Iniciar
emacs-daemon-why-refused = Não deu para iniciar o daemon do Emacs; veja: journalctl --user -u emacs
