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
