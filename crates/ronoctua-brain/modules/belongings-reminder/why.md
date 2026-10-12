# belongings-reminder

Os pertences (`LEADER s v`) são a configuração da pessoa guardada cifrada num repositório privado
dela; a sincronização é da vista e da fila (a CLI `ronoctua-belongings` como filho), e pede a
SENHA. Este módulo é só o lembrete: ele não roda nada, porque nada que a máquina faz sozinha pode
pedir coisa alguma a ninguém.

## Um vigia, e um relógio só

Ele lê os arquivos que os pertences deixam — `belongings.json` (repositório e período),
`status.json` (como terminou a última execução), `last.json` (a última sincronização) e a sessão no
diretório de runtime — quando eles mudam (inotify pelo fio de vigia do cérebro), e arma UM relógio,
para `última + período`, só enquanto há repositório, período e a sincronização ainda não venceu.
Sem repositório, nunca; com o período "nunca", nunca.

## As marcas

Uma por vez, a mais urgente primeiro:

- **decisões esperando** (cor de aviso, com quantas): uma sincronização preparada espera a pessoa
  decidir os conflitos. O clique abre os pertences, onde as decisões ficam — e não pede a senha,
  porque pedir de novo prepararia outra vez e jogaria fora o que já foi decidido. Uma sessão que um
  reinício apagou (o runtime é tmpfs) não espera nada;
- **a última falhou** (cor de aviso);
- **primeira vez** (acento): há repositório e esta máquina nunca sincronizou com ele — o que a
  primeira sincronização faz é restaurar;
- **venceu** (vigiando): passou o período desde a última.

O clique das três últimas abre os pertences pedindo a senha. Esse ato é um COMANDO do rodapé
(o id da marca o nomeia, `MarkId::opening`), não uma ação do módulo: "vá lá" e não "faça isto". O
cérebro nem vê o clique.

## Por que o nome não é só `belongings`

O id de um módulo é a palavra que ninguém de fora da pasta pode dizer (o check selado), e
"belongings" é o vocabulário inteiro da feature — o crate, a vista, a fila, os textos. O lembrete
tem nome próprio, e o resto do sistema fala dos pertences sem falar dele.

## A parte em claro vai sozinha

As declarações da pessoa (`state/person`) e os módulos do brain dela (`config/brain-modules`) são a
parte em claro dos pertences: viajam sem senha e sem identidade. Como não há o que perguntar, a
máquina pode mandá-las sozinha. O lembrete vigia essas raízes como árvores (`Source::Tree`: toda
pasta abaixo delas, as criadas depois que o cérebro subiu inclusive) e, depois de 30 s sem mudança,
inicia a `ronoctua-belongings-plain.service` (`sync-plain`), declarada no `module.nix` deste módulo.
Uma mudança no meio da espera recomeça a espera: a central de programas que grava a lista e depois
os ícones vira um envio só.

Árvore, e não a lista das pastas que existiam na partida, porque o inotify não é recursivo: numa
máquina nova o `state/person` ainda não tem `software/`, a primeira declaração o cria (isso se
ouve, na raiz) e todas as seguintes só reescrevem o `declared.json` lá dentro — que uma lista feita
na partida não ouvia nunca. Um notebook que dorme em vez de sair ficava dias sem mandar as
declarações.

Iniciar a unidade não é mandar. Uma sincronização da pessoa que segura a trava dos pertences recusa
o `sync-plain` (`Busy`), e um `sync-plain` que já roda só é juntado pelo systemd: em nenhum dos dois
casos o que mudou depois da leitura sobe. Então o lembrete guarda a mudança até a pasta de estado
dizer que uma execução terminou: se alguma pegou a trava depois do pedido (a trava ganha a hora de
quem a pega), ela leu a parte em claro depois da mudança, e acabou — mandou, ou falhou, e aí a
próxima mudança ou sincronização manda; repetir uma falha seria um laço. Se nenhuma pegou, o pedido
foi recusado ou ficou para a execução que segurava a trava, e ele pede de novo na hora, no máximo três
vezes por mudança. Uma mudança ouvida durante uma execução é uma mudança como outra: vai depois
dela, passados os 30 s.

Só manda quem tem para onde: uma máquina que já sincronizou com o repositório que está configurado
(antes da primeira sincronização não há identidade, e o repositório pode nem existir). Nunca num
ensaio: a cópia do `.#dev` não mexe no repositório de verdade. E nunca pergunta nada: o que bate dos
dois lados fica como cada lado tem, para a próxima sincronização com a pessoa.
