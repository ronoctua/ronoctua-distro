# The texts `ronoctua-files` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# ===================================================================
# O gerenciador de arquivos — LEADER . (rodapé) e a janela
#
# As explicações de permissão são a razão pela qual este arquivo passou a ter
# SELETOR. Numa pasta, `x` não é "executar", é "entrar", e `r` sem `x` lista
# nomes que não se alcança — dizer "executar" ali é dizer algo falso. A escolha
# entre as duas frases é do IDIOMA, então mora aqui e não no Rust.
# ===================================================================

# O que a primeira letra da coluna diz.
files-kind-directory = Uma pasta: um lugar, e não uma coisa.
files-kind-file = Um arquivo comum.
files-kind-symlink = Um atalho: aponta para outro caminho, e apagá-lo não apaga o alvo.
files-kind-fifo = Um cano nomeado: um programa escreve, outro lê.
files-kind-socket = Um soquete: dois programas conversando por aqui.
files-kind-block = Um dispositivo de bloco — um disco, ou parte de um.
files-kind-char = Um dispositivo de caractere.
files-perm-owner-read = { $kind ->
        [dir] { $set ->
                [yes] { $who } pode ver os nomes que há nesta pasta
               *[no] { $who } NÃO pode ver os nomes que há nesta pasta
            }
       *[file] { $set ->
                [yes] { $who } pode ler este arquivo
               *[no] { $who } NÃO pode ler este arquivo
            }
    }
files-perm-owner-write = { $kind ->
        [dir] { $set ->
                [yes] { $who } pode criar e apagar coisas dentro desta pasta
               *[no] { $who } NÃO pode criar nem apagar nada nesta pasta
            }
       *[file] { $set ->
                [yes] { $who } pode alterar este arquivo
               *[no] { $who } NÃO pode alterar este arquivo
            }
    }
files-perm-owner-execute = { $kind ->
        [dir] { $set ->
                [yes] { $who } pode ENTRAR nesta pasta
               *[no] { $who } NÃO pode entrar nesta pasta — nem no que está dentro
            }
       *[file] { $set ->
                [yes] { $who } pode rodar este arquivo como programa
               *[no] { $who } não pode rodar este arquivo
            }
    }
files-perm-group-read = { $kind ->
        [dir] { $set ->
                [yes] o grupo { $who } pode ver os nomes que há nesta pasta
               *[no] o grupo { $who } NÃO pode ver os nomes que há nesta pasta
            }
       *[file] { $set ->
                [yes] o grupo { $who } pode ler este arquivo
               *[no] o grupo { $who } NÃO pode ler este arquivo
            }
    }
files-perm-group-write = { $kind ->
        [dir] { $set ->
                [yes] o grupo { $who } pode criar e apagar coisas dentro desta pasta
               *[no] o grupo { $who } NÃO pode criar nem apagar nada nesta pasta
            }
       *[file] { $set ->
                [yes] o grupo { $who } pode alterar este arquivo
               *[no] o grupo { $who } NÃO pode alterar este arquivo
            }
    }
files-perm-group-execute = { $kind ->
        [dir] { $set ->
                [yes] o grupo { $who } pode ENTRAR nesta pasta
               *[no] o grupo { $who } NÃO pode entrar nesta pasta
            }
       *[file] { $set ->
                [yes] o grupo { $who } pode rodar este arquivo como programa
               *[no] o grupo { $who } não pode rodar este arquivo
            }
    }
files-perm-other-read = { $kind ->
        [dir] { $set ->
                [yes] qualquer pessoa da máquina pode ver os nomes desta pasta
               *[no] mais ninguém pode ver os nomes desta pasta
            }
       *[file] { $set ->
                [yes] qualquer pessoa da máquina pode ler este arquivo
               *[no] mais ninguém pode ler este arquivo
            }
    }
files-perm-other-write = { $kind ->
        [dir] { $set ->
                [yes] qualquer pessoa da máquina pode criar e apagar coisas aqui
               *[no] mais ninguém pode criar nem apagar nada aqui
            }
       *[file] { $set ->
                [yes] qualquer pessoa da máquina pode alterar este arquivo
               *[no] mais ninguém pode alterar este arquivo
            }
    }
files-perm-other-execute = { $kind ->
        [dir] { $set ->
                [yes] qualquer pessoa da máquina pode entrar nesta pasta
               *[no] mais ninguém pode entrar nesta pasta
            }
       *[file] { $set ->
                [yes] qualquer pessoa da máquina pode rodar este arquivo
               *[no] mais ninguém pode rodar este arquivo
            }
    }
# Os três bits que não são sobre uma pessoa. A frase começa pelo que
# surpreende: quem aponta para um `s` está perguntando sobre o `s`.
files-perm-setuid = { $set ->
        [yes] Roda como { $who }, e não como quem o executou.
       *[no] O bit está ligado numa posição que não executa — quase sempre um engano.
    }
files-perm-setgid = { $kind ->
        [dir] { $set ->
                [yes] O que nascer aqui dentro herda o grupo { $group }.
               *[no] O bit está ligado numa posição que não executa.
            }
       *[file] { $set ->
                [yes] Roda com o grupo { $group }, e não com o de quem o executou.
               *[no] O bit está ligado numa posição que não executa.
            }
    }
files-perm-sticky = { $set ->
        [yes] Só quem é dono de um arquivo pode apagá-lo daqui — é o que faz o /tmp funcionar.
       *[no] O bit está ligado numa posição que não executa.
    }
# Os quatro modos que respondem "o que isto devia ser".
files-preset-file = arquivo comum — você escreve, o resto lê
files-preset-private = só você, e mais ninguém
files-preset-program = programa — todos rodam, só você altera
files-preset-private-dir = pasta só sua
files-preset-readonly = ninguém altera, nem você
# O tamanho. A vírgula está AQUI porque o separador decimal é do idioma.
files-size-bytes = { $whole } B
files-size-kib = { $whole },{ $tenth } KiB
files-size-mib = { $whole },{ $tenth } MiB
files-size-gib = { $whole },{ $tenth } GiB
files-size-tib = { $whole },{ $tenth } TiB
files-order-name = nome
files-order-size = tamanho
files-order-modified = modificado
files-order-kind = tipo
# As linhas do menu de ações. O id e a chave não podem divergir — um teste
# compara os dois, porque a linha copiada com o id trocado COMPILA e mostra as
# palavras da linha vizinha.
files-open = abrir
files-open-map = abrir o lugar num mapa
files-open-with = abrir com
files-terminal = terminal aqui
files-root-terminal = abrir como root
files-copy-path = copiar o caminho
files-copy = copiar
files-cut = recortar
files-paste = colar aqui
files-rename = renomear
files-info = informações
files-mode = permissões
files-owner = dono
files-group = grupo
files-compress = comprimir…
files-extract = extrair
files-trash = mover para a lixeira
files-delete = apagar de vez
files-shred = apagar sobrescrevendo
files-pin = pôr na barra lateral
files-unhide = trazer de volta uma linha oculta da barra
# O MENU DA BARRA LATERAL. Cada id vira `sidebar-<id>`, e o check cobra os dois.
sidebar-open = abrir
sidebar-mount = montar
sidebar-unmount = desmontar
sidebar-eject = ejetar com segurança
sidebar-information = informações
sidebar-unpin = tirar da barra lateral
sidebar-format = formatar
sidebar-fact-node = dispositivo
sidebar-fact-kind = sistema de arquivos
sidebar-fact-size = tamanho
sidebar-fact-drive = disco
# O SEGUNDO REGISTRO DE CADA AÇÃO — o que ela FAZ, para quem já sabe o que quer.
#
# O rótulo é para quem procura um verbo ("apagar de vez"); esta linha é para
# quem quer saber exatamente o que vai acontecer: qual chamada, quais opções, o
# que dá para desfazer. Aparece na linha de mensagem quando a linha está sob o
# CURSOR — não só sob o ponteiro, porque o que o mouse faz o teclado faz.
files-does-open = abre no aplicativo padrão do tipo MIME; pasta e arquivo compactado entram na listagem
# THE ONE ACTION IN THIS MENU THAT LEAVES THE MACHINE, and the sentence says
# so before the key is pressed — the same courtesy the glance's two hosts get.
files-does-open-map = abre no mapa do navegador a coordenada gravada neste arquivo — a coordenada sai desta máquina
files-does-open-with = escolhe entre os `.desktop` que declaram este tipo MIME
files-does-terminal = abre o terminal nesta pasta, como você
files-does-root-terminal = abre o terminal como root, pelo pkexec
files-does-copy-path = põe o caminho absoluto na área de transferência; não abre nada
files-does-copy = marca para copiar; colar cria a cópia pela fila, que sobrevive ao rodapé
files-does-cut = marca para mover; no mesmo sistema de arquivos é rename(2), instantâneo
files-does-paste = executa o que foi marcado; entre sistemas de arquivos é cópia e depois remoção
files-does-rename = renomeia no lugar com rename(2); não atravessa sistema de arquivos
files-does-pin = acrescenta uma linha à barra lateral, gravada em $XDG_CONFIG_HOME
files-does-unhide = passa a listar também o que começa com ponto
files-does-info = tamanho, dono, modo, tempos e tipo; não toca no conteúdo
files-does-mode = altera os bits de permissão com chmod(2)
files-does-owner = altera o dono com chown(2); pergunta a senha de root
files-does-group = altera o grupo com chown(2)
files-does-compress = cria um arquivo compactado pela fila, sem bloquear a listagem
files-does-extract = extrai para uma pasta com o nome do arquivo
files-does-restore = devolve ao caminho de origem que a lixeira gravou
files-does-trash = move para a lixeira XDG; não atravessa sistema de arquivos
files-does-delete = unlink(2) ou rmdir(2) direto: sem lixeira e sem volta
files-does-shred = sobrescreve antes de apagar; num btrfs com cópia-na-escrita isso não garante nada
# E os do menu da barra lateral, que falam com o udisks.
sidebar-does-open = entra na pasta; se for dispositivo desmontado, abre a ficha dele
sidebar-does-mount = Filesystem.Mount do udisks2 — ele escolhe o ponto e as opções
sidebar-does-unmount = Filesystem.Unmount sem forçar: recusa se algo ainda estiver aberto ali
sidebar-does-eject = desmonta, tranca de novo um disco cifrado e corta a energia (Drive.PowerOff): é o que deixa tirar
sidebar-does-information = rótulo, nó, sistema de arquivos, tamanho e disco
sidebar-does-unpin = tira a linha; padrão da distro volta oculto, o seu é removido
sidebar-does-format = abre o buffer de formatar já apontado para este alvo
files-restore = devolver de onde veio
# In a window the change is not in force anywhere: the file is the only road.
echo-setting-not-written = Não foi possível gravar { $file } ({ $why }), e nada mudou.

## file info editing — everything the info panel can change

files-remove-location = tirar o lugar onde foi feito
files-does-remove-location = pergunta antes; depois reescreve o arquivo — ou escreve uma cópia ao lado — sem a coordenada de GPS. Tirado do próprio arquivo, não se desfaz
# The archive rows of the file manager's menu (`ronoctua_files::menu`).
files-archive-test = testar o arquivo compactado
files-archive-convert = converter para outro formato…
files-archive-add = pôr o que foi copiado neste arquivo compactado
files-archive-extract = extrair aqui, ao lado do arquivo compactado
files-archive-remove = tirar do arquivo compactado
files-does-archive-test = lê cada membro e confere com as somas (7zz t, unrar t, bsdtar -t), pela fila
files-does-archive-convert = extrai numa pasta oculta ao lado e compacta de novo no formato escolhido na ficha
files-does-archive-add = 7zz a: o que foi copiado entra no topo do arquivo, cifrado com a senha dele quando tem uma
files-does-archive-extract = tira os membros escolhidos para a pasta do próprio arquivo; nada lá é substituído
files-does-archive-remove = 7zz d: reescreve o arquivo sem eles — não tem volta
sidebar-lock = trancar
sidebar-does-lock = desmonta o que saiu dele e tranca o contêiner (Encrypted.Lock); a energia continua ligada
# The file manager's menu row that checks a file with VirusTotal.
files-virustotal = checar no VirusTotal
files-does-virustotal = manda a impressão digital (SHA-256) ao VirusTotal, na janela da central de segurança; o arquivo em si só se você disser sim
# A linha do menu do gerenciador (`C-m`, `k`).
files-belongings = guardar nos pertences, ou deixar de guardar
files-does-belongings = acrescenta ao que os pertences guardam no seu repositório e devolvem numa máquina nova; quando já é guardado, tira — o arquivo fica onde está
files-private-withheld-shut = O /private não está aberto: abra-o, ou retome-o, com a senha dele para ver o que há dentro.
files-private-withheld-silent = O /private não está respondendo: foi suspenso em uso, e a senha dele o retoma.
sidebar-write-installer = gravar o instalador aqui
sidebar-does-write-installer = abre o pendrive do instalador neste drive: o drive inteiro é gravado, depois de uma pergunta cuja primeira resposta é não

## AS CHAVES DA CENTRAL DE SEGURANÇA, e a contagem da janela sem senha.

files-encrypt = cifrar…
files-does-encrypt = cifra com o age ao lado do original — para você, com uma senha ou para a chave de alguém; uma pasta vira nome.tar.age
files-decrypt = decifrar…
files-does-decrypt = abre o .age ao lado dele, nunca por cima de nada — com a senha dele, ou com a chave dos seus pertences
