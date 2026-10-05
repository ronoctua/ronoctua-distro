# cache-sweep

Nada em cache fica para sempre. O dono perguntou como os caches da distro funcionavam, e a resposta
honesta era que nenhum dos dois no disco era limpo: nenhum timer, nenhuma regra do tmpfiles, nenhuma
varredura. A regra dele veio numa frase: nada em cache pode ficar para sempre. O trabalho de apagar é
do `ronoctua-cache-sweep` (o crate da varredura; o que ele leva, as travas do cargo e as raízes do
Nix estão em `docs/ARCHITECTURE.md`); o que está aqui é o QUANDO.

## O cérebro decide o QUANDO, e só isso

A varredura é uma unidade e um binário próprios (`module.nix`). Um `run` bloqueia a thread do runner,
e andar um quarto de milhão de arquivos ali seria um daemon que não desenha nada e não pode ser
parado enquanto dura. A unidade ganha `IOSchedulingClass=idle` de graça, o que importa quando a
primeira execução libera cento e cinquenta gibibytes, e continua alcançável à mão.

Não é um timer do systemd: o cérebro decide contra o portão que a pessoa configurou (memória,
processador parado, bateria). "Às quatro da manhã, se a máquina não estiver ocupada" é uma regra, e
não duas; um timer seria um segundo calendário que não sabe de carga, e os dois discordariam
justamente na noite que importava.

## Os números são do catálogo

A cadência e a janela vêm de `assets/cache.nix` pelo manifesto (`everyDays`, `partOfDay`), e o
quanto é velho demais pelo argumento da unidade (`ronoctua.cache.keepDays`, com o padrão do mesmo
arquivo). Um lugar só, porque o gerenciador de arquivos lê dois dos mesmos quatro números para
decidir quando refazer uma miniatura, e `freshDays < keepDays` é uma promessa que nenhum dos dois
crates cumpre sozinho.

Sem contagem regressiva: a varredura não interrompe nada e nada do que ela faz aparece enquanto
acontece; um minuto de aviso para apagar arquivos que ninguém abre há semanas seria uma faixa no
caminho sem decisão nenhuma. A paciência é de minutos, não de horas: duas horas são de quem compila
um sistema, e uma varredura que morreu calada só seria notada à tarde.

## Duas unidades, e não uma bandeira

`m e` na página do cérebro ensaia: a segunda unidade anda, conta e não apaga nada, que é exatamente
a pergunta que alguém quer respondida antes de deixar algo apagar cento e cinquenta gibibytes. Uma
bandeira atravessando do cérebro seria uma bandeira que outro poderia atravessar. As duas escrevem o
relatório onde o runner olha (`%t/ronoctua-brain/<id>.json` e `<id>-rehearsal.json`), e o check do
módulo lê as unidades como a máquina as recebe.

## O que ela não pode fazer

Roda como a PESSOA, nunca como root, não pergunta nada a ninguém e mexe em uma árvore só:
`$XDG_CACHE_HOME`. O sandbox da unidade é a segunda tranca na mesma porta — o único caminho gravável
do sistema de arquivos inteiro é o próprio cache (e o diretório dos relatórios, criado antes do
sandbox abrir, senão a unidade cai com 226/NAMESPACE). Não há regra do polkit e não pode haver:
uma varredura que pudesse ser convencida a rodar como root poderia apagar um sistema.

Desistir é o que a para: uma varredura abortada que continuasse apagando seria uma máquina perdendo
arquivos depois de a pessoa mandar parar, então o `Aborted` para a unidade.
