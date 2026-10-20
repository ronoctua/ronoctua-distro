# The texts `ronoctua-belongings` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

belongings-invalid-empty = Nada foi digitado.
belongings-invalid-credentials-in-address = O endereço traz usuário e senha: dê só o endereço, e a chave ou o token em Acesso.
belongings-invalid-plain-http = Um endereço http:// manda o acesso às claras: use https:// ou SSH.
belongings-invalid-not-remote = Um caminho local ou file:// não é um repositório remoto.
belongings-invalid-unknown = O endereço é git@servidor:caminho, ssh://… ou https://….
belongings-refused-no-repository = Nenhum repositório definido: dê o endereço em Estado.
belongings-refused-no-passphrase = A senha não estava lá, ou outra pessoa podia ler o arquivo dela; ela não foi usada.
belongings-refused-wrong-passphrase = Essa senha não abre estes pertences.
belongings-refused-needs-access = O repositório quer um acesso que esta máquina não recebeu: uma chave de implantação ou um token, em Acesso.
belongings-refused-refused = O repositório recusou a chave ou o token desta máquina.
belongings-refused-unreachable = Não foi possível chegar ao repositório.
belongings-refused-raced = Outra máquina continuou sincronizando enquanto esta sincronizava; tente de novo.
belongings-refused-not-ours = Esse repositório não é um repositório de pertences que este sistema lê.
belongings-refused-empty = O repositório está vazio: a primeira sincronização define a senha.
belongings-refused-corrupt = Algo no repositório não se decifra ou não se lê.
belongings-refused-undecided = Um conflito ainda está sem decisão.
belongings-refused-no-session = Não há sincronização preparada para aplicar.
belongings-refused-no-tools = Esta instalação não tem git ou ssh-keygen.
belongings-refused-failed = Um arquivo daqui não pôde ser lido ou escrito; o log do sistema diz qual.
belongings-refused-busy = Outra execução dos pertences está em andamento: espere ela terminar.
belongings-refused-not-post-quantum = O servidor do repositório não ofereceu troca de chaves pós-quântica, e os pertences exigem uma: nada foi buscado nem enviado.
belongings-refused-choice-unread = A escolha de programas do instalador não se lê: nada foi recusado nem trazido de volta, e nada foi enviado.
belongings-refused-copy-unwritten = A cópia das suas declarações não pôde ser escrita onde o instalador a guarda; o log do sistema diz onde.
belongings-refused-not-let = O host do repositório ainda não pode ser consultado: permita-o na linha abaixo de Sincronizar. Nada foi buscado nem enviado.
belongings-refused-newer = O repositório está num formato mais novo do que esta máquina lê: atualize esta máquina primeiro.
belongings-invalid-unnamed-host = O host precisa ser um nome ou um endereço IPv4: um endereço IPv6 ou um nome com sublinhado não pode ter permissão de consulta.
belongings-refused-plain-too-large = Grande demais para guardar em claro.
