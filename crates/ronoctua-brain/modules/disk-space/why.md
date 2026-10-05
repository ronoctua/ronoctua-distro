# disk-space

Quando o disco aperta, o cérebro abre espaço na hora; enquanto ele está quase cheio, a modeline diz
isso. O trabalho de apagar é do `ronoctua-disk-relief` (crate da varredura; os dois passos, as
travas do cargo e as raízes do Nix estão em `docs/ARCHITECTURE.md`, "Quando o disco aperta: o
cérebro abre espaço, na hora"); o que está aqui é o QUANDO, a marca e o que a pessoa ouve.

## Quem vê é o cérebro, no tique que já existe

O piso do Nix só age enquanto algo compila, e só no store; um disco cheio por qualquer outra coisa —
o cargo, antes de tudo — fica cheio até alguém ver. O pulso de cada tique já lê o `statvfs` do
sistema de arquivos do `/nix/store` e do da home (`Pulse::rooms`: o tamanho e o livre de cada um que
respondeu). Nada avisa um processo que um disco está enchendo — o inotify fala de arquivos, não de
capacidade —, então o evento é o tique, sem relógio novo.

O pulso carrega BYTES, não veredito. Ele carregava `disk_short`, e para isso o núcleo chamava este
módulo; agora quem pergunta "está curto?" e "está quase cheio?" é o módulo, sobre a mesma leitura, e
o núcleo não nomeia ninguém.

## Nunca pelo calendário, e fora do portão

A linha dele diz "quando falta disco" em vez de uma data. Ele entra por `wanted_now` — abaixo do
piso, e se o último INÍCIO foi há mais de `reliefGapMinutes`, ensaio incluído (o runner guarda
quando cada módulo começou; o diário, onde ensaio não é execução, não sabe, e o cérebro do `.#dev`,
que só ensaia, disparava a folga a cada tique). O manifesto o tira do portão: a máquina está mais
ocupada justamente enquanto um build enche o disco, e uma folga que esperasse a memória acalmar
esperaria o disco lotar. Pelo mesmo motivo ele entra na CABEÇA da fila (`Shared::queue_first`): o
runner só olha a cabeça, e atrás de uma varredura retida pelo portão ele ficaria retido também. Roda
em prioridade ociosa, então ceder é trabalho do escalonador, não de um bloqueio.

## Porcentagem com teto, duas linhas

O piso é `min(diskFloorPercent, diskFloorCapGiB)` do sistema de arquivos. Sem o teto, dez por cento
de um disco de 8 TB são 800 GB: um disco cheio sobretudo dos arquivos da pessoa estaria "sem espaço"
com centenas de gigabytes livres, e a folga rodaria de hora em hora para nada. Os números vêm de
`assets/cache.nix` pelo `nix/cache.nix`, o mesmo gerador de onde saem os argumentos da unidade
(`tables.nix` o chama com os mesmos argumentos), então o piso que o cérebro vigia e a meta que a
folga persegue não se separam.

A linha de QUASE CHEIO (`diskWarnPercent`, com teto `diskWarnCapGiB`) fica ABAIXO do piso, e o
`nix/cache.nix` recusa o build se não ficar. Abaixo do piso a folga resolve sozinha; uma marca ali
subiria a cada folga que começa e sumiria a cada uma que funciona — a modeline falando enquanto
ninguém precisa fazer nada. Abaixo da linha, a folga já fez o que podia (ou algo enche o disco mais
rápido do que ela libera), e o que ocupa o disco é decisão da pessoa.

## Uma folga que deixou o disco curto não se repete

Quando o que enche o disco é da pessoa — arquivos, programas, cinco closures do sistema —, nada mais
sai sem ela, e uma folga por hora andaria o cache e coletaria o store à toa, dizendo "continua sem
espaço" a cada vez. Depois de uma assim, a seguinte espera, além do intervalo, o disco encher MAIS
do que ela o deixou: só então há algo novo para liberar. Isso fica na memória do módulo: um cérebro
que reiniciou roda uma folga a mais, que é o erro mais barato.

## A marca

Pedido do dono (2026-09-30): o disco quase cheio fica FIXO na modeline. Enquanto algum sistema de
arquivos medido está abaixo da linha de quase cheio, o módulo publica uma marca (`full`, glifo de
armazenamento com alerta, na cor de aviso); em repouso não há nenhuma (inv. 14). Um disco que
ninguém mediu não tem marca: uma marca sobre um disco que não se leu mentiria. O clique abre a
página do módulo, que diz quanto está livre no sistema de arquivos que decide (o de menor fatia
livre), o tamanho dele e onde ficam as duas linhas; a geração da página é o instante da leitura, então
ela é pedida de novo quando há leitura nova, e só então.

## O que a pessoa ouve

O módulo mede antes e depois de uma folga de verdade e conta, pela porta comum
(`Ctx::tell`, o anel `Mind.events`), um de dois eventos: `relieved` ("espaço aberto no disco: 12,3
GB liberados") ou `still-short` — este sem `auto-read`, porque o que falta liberar é decisão da
pessoa, e dito UMA vez por aperto: outra folga que acha o mesmo não o repete, só uma que abriu espaço
antes. O rodapé diz os dois pelo caminho genérico (`events::of_brain`), sem saber de quem são. Um
ensaio não mudou nada e não diz nada. O btrfs devolve o espaço quando a transação fecha; a folga
pede `syncfs` antes de terminar, senão "liberado" sairia menor.

As etiquetas antigas (`disk-relieved`, `disk-still-short`) seguem nas notificações dos dois eventos
(`tags` no manifesto), para um filtro salvo com elas continuar pegando; o catálogo deixa um filtro
testar as etiquetas da entrada que reconheceu a notificação.

## O que ele precisa

Uma sessão gráfica: o cérebro roda numa. Uma máquina em que ninguém entra graficamente fica com o
piso do Nix durante os builds e a aparagem a cada ativação, e nada que ande um cache. As máquinas
dos testes de VM escrevem `{"quiet": true}` no `brain.json`: nada começa sozinho, e a coleta não
leva os caminhos que um teste registrou e não enraizou — sem nomear módulo nenhum.
