# containers

Enquanto algum container da pessoa roda, a modeline diz quantos; um clique abre a vista dos
containers (`LEADER s o`). Em repouso não há marca (inv. 14).

## Quem conta é o kernel

O crun mantém uma pasta por container rodando em `$XDG_RUNTIME_DIR/crun` (o caminho vem de
`ronoctua_containers::runtime_dir`) e a apaga quando o container termina. O cérebro vigia essa pasta
com inotify — pela pasta-mãe até ela existir, por dentro depois que existe — e cada mudança ouvida é
um `readdir`: a contagem é exata, custa microssegundos e nada roda em repouso.

## O que foi recusado

- **`podman events`**: um processo residente de 30 a 50 MB para dizer o que o kernel já diz.
- **O journal**: diria o que começou e o que terminou, mas não o que já rodava quando o cérebro
  subiu — precisaria de um `podman ps` na partida, e o módulo passaria a rodar podman.

## O risco

O leiaute das pastas do crun é privado dele. O `tests/containers-vm.nix` roda um container de
verdade, confere a pasta `crun/<id>` e a marca com a contagem, e confere que as duas somem quando o
container para: é lá que uma mudança do crun apareceria.

## A marca abre o app

A marca declara o comando do shell que o clique roda (`MarkId::opening`, `containers-open`): o
shell o roda pelo nome, como o `LEADER s o`, sem nomear o módulo. Uma pasta que não pode ser vigiada
é dita na página do módulo, em vez de uma contagem de zero que pareceria "nenhum rodando".
