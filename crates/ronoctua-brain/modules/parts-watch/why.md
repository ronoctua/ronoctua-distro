# Partes a caminho

## Por que um vigia

O que precisa de um download que nenhum cache público tem, o que não é livre e o que só o perfil full
leva chega depois da instalação, cada coisa por conta própria: uma **parte** (`nix/modules/parts.nix`,
`docs/ARCHITECTURE.md`, "Partes: o que chega depois da instalação"). A instalação termina sem esperar
por elas, e a distro funciona sem elas — mas a pessoa precisa saber que algo ainda está chegando, e
quando algo chegou ou precisa dela.

## Por que os registros, e não uma pergunta ao root

O realizador das partes (root) escreve um registro por parte, inteiro, renomeado para dentro de
`/var/lib/ronoctua-parts/parts`, legível por todos. O registro é o broadcast: o kernel avisa o brain
(inotify) e ler os registros é um `readdir`. Nada roda em repouso, e não existe um segundo caminho
até o root.

## A marca

Uma marca só, e só enquanto diz algo (invariante 14): uma parte que precisa da pessoa, na cor de um
aviso; senão, uma a caminho, na cor de um vigia; cada uma com quantas. O clique abre a página das
partes, onde estão o porquê, a próxima tentativa e os outros jeitos de trazer um download (entregar o
arquivo, um pendrive, um espelho).

## As palavras

Uma parte que chegou e uma que passou a precisar da pessoa são ditas uma vez cada, com o nome dela.
A primeira leitura é a base e não diz nada: ela não é notícia.
