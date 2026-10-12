# security-watch

O vigia da central de segurança: escuta o que ela guarda e não roda nada. O que ele ouve vem de
três fontes do diário que o núcleo segue (as linhas do kernel com o prefixo `ronoctua-refused: `,
o `sudo` e as unidades do firewall em `err`) e do inotify nas duas pastas que o ajudante root
escreve (`/run/ronoctua-security`, `/var/lib/ronoctua-security`). A página da central
(`security.rs`) pede a página dele e desenha as recusadas e o sudo nas próprias seções.

## Um escudo, só enquanto algo está fora

A modeline em repouso é muda (inv. 14). O escudo aparece, na cor de perigo, só enquanto algo está
fora do esperado, e é UM: o mais alto de cinco casos, cada um com a própria frase sob o ponteiro —
o firewall falhou (o ajudante disse, ou uma unidade do firewall registrou erro depois da última
aplicação), o firewall está desligado, o sudo não pede senha (até uma hora, ou sem prazo), os escudos estão
erguidos, e conexões recusadas no último quarto de hora (o escudo conta quantas). A hora do fim da
janela sem senha é da própria modeline do rodapé, ao lado; a dica só pode contar (`count`).

O clique abre a central de segurança na seção de que o escudo fala: a janela sem senha no
Administrador, todo o resto no Firewall. Quem decide é a central (`security::section_of_mark`), que
o clique genérico das marcas do cérebro pergunta antes de abrir a página do módulo (`brain::click_of`);
a página do módulo continua listando tudo o que está fora, no `LEADER b`.

## Relógio só enquanto é preciso

Em repouso, nenhum relógio: o diário e o inotify acordam o vigia. Ele arma um só enquanto uma
recusa é recente (até ela deixar de ser), enquanto a janela sem senha está aberta (até o fim) e
enquanto o aviso está adiado. Uma janela sem prazo não arma relógio nenhum: ela só acaba pelo gesto
da pessoa, e o arquivo dela (`/var/lib/ronoctua-security/sudo-window`, que sobrevive ao reboot)
muda quando acaba — é o inotify da pasta que tira o escudo.

## O aviso das recusadas

"N conexões recusadas em wlan0": dispensar guarda até a próxima recusa, e adiar, por uma hora. O
log do kernel já vem com limite (6 por minuto, ou 60 no modo falante), então a contagem é do que
foi registrado, não de todo pacote recusado — a página diz "recusadas há pouco", não "todas".

## O sudo

Uso, senha errada e recusa. A linha "a password is required" é a primeira tentativa da própria
porta (`sudo -n`) antes de toda pergunta, e não é evento. Na primeira partida ele lê uma semana
(backfill do diário), e depois segue do cursor salvo pelo núcleo.
