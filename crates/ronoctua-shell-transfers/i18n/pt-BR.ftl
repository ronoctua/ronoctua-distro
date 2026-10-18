# The texts `ronoctua-shell-transfers` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# === A fila de transferências — `LEADER t` =================================
#
# Sete operações iam para uma thread sem dizer nada na tela: colar, apagar,
# sobrescrever, lixeira, comprimir, extrair e medir pasta. Agora todas entram na
# mesma fila, e é ela que a modeline resume e o painel detalha.
group-transfer = transferências
cmd-transfers-open = a fila de transferências
transfers-give-up-no = Não, deixar seguir
transfers-give-up-yes = Sim, desistir
transfers-empty = nada em andamento
# O sujeito, quando é mais de um.
transfers-and-more = { $name } e mais { $rest }
# Para onde vai. A seta é o único desenho aqui, e não é chrome: é a preposição.
transfers-into = → { $where }
# Quantos itens, para as operações que contam coisas e não bytes.
transfers-items = { $done } de { $total }
# ---------------------------------------------------------------------------
# A MESMA FILA, EM JANELA PRÓPRIA — `LEADER t w`, o buffer wasip2.
#
# Não é uma segunda implementação: é o mesmo socket, as mesmas linhas, com
# espaço para todas de uma vez e uma tela de detalhe que a região do rodapé não
# comporta. As teclas de dentro vivem sob `SPC m`, como as de todo buffer.
# ---------------------------------------------------------------------------
cmd-transfers-window = a fila de transferências, em janela
cmd-transfers-add = baixar o que está na área de transferência
cmd-transfers-video = baixar o vídeo do que está copiado
cmd-transfers-audio = baixar só o áudio do que está copiado
cmd-transfers-subtitles = baixar só a legenda do que está copiado
cmd-transfers-add-as-me = baixar o que está copiado, como eu
cmd-transfers-video-as-me = baixar o vídeo, como eu
cmd-transfers-audio-as-me = baixar só o áudio, como eu
cmd-transfers-subtitles-as-me = baixar só a legenda, como eu
# O grupo que custa o anonimato — e é por isso que ele é um prefixo, e não um
# interruptor guardado em outro lugar.
group-identified = como eu (custa o anonimato)
transfers-window-field-identity = quem pediu
transfers-identity-mine = você
transfers-identity-anonymous = ninguém
transfers-window-detail = a transferência, campo a campo
transfers-window-unknown = não se sabe
transfers-window-confirm-cancel = Desistir desta transferência? O que já foi escrito no destino é apagado.
transfers-window-field-engine = quem move
transfers-window-field-stage = situação
transfers-window-field-errand = operação
transfers-window-field-subject = no momento
transfers-window-field-done = feito
transfers-window-field-total = total
transfers-window-field-trouble = problema
transfers-window-field-answer = resposta
