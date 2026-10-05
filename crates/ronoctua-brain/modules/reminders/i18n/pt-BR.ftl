# The reminders' own texts: its name and the line under it, the alarm and the
# missed notice, its page, its actions and their questions, what it writes into
# the activity log, why it refuses — and the words an answer is read with
# (`reminders-grammar-*`: one line per class of word, the alternatives
# separated by commas).
brain-task-reminders = Lembretes e alarmes
brain-task-reminders-does = Alarmes e lembretes pedidos com as suas palavras: uma vez (“amanhã 9:00 ligar para a mãe”, “daqui a 2 h com som tirar a pizza”), todo dia, em dias da semana, num dia do mês ou numa data de todo ano, como um aniversário. Na hora ele toca na linha do cérebro, na frente de tudo, e fica lá até você dispensar ou adiar; o que venceu com a máquina desligada aparece como perdido. Um lembrete com som toca um sino algumas vezes enquanto está tocando.
reminders-mark-ringing = Um lembrete está tocando
reminders-says-due = { $more ->
        [0] { $text }
       *[other] { $text } (e mais { $more })
    }
reminders-says-missed = { $more ->
        [0] Perdido com a máquina desligada: { $text }, para { $at }
       *[other] Perdido com a máquina desligada: { $text }, para { $at } (e mais { $more })
    }
reminders-says-unreadable = { $held ->
        [0] O arquivo dos lembretes não pode ser lido, então nenhum toca: conserte ou remova
        [one] O arquivo dos lembretes não pode ser lido: conserte ou remova. O lembrete lido antes de ele quebrar ainda toca
       *[other] O arquivo dos lembretes não pode ser lido: conserte ou remova. Os { $held } lidos antes de ele quebrar ainda tocam
    }
reminders-untitled = Lembrete
reminders-why-not-understood = Não entendi quando. Tente “amanhã 9:00 ligar para a mãe”, “daqui a 2 h chá”, “toda segunda 8:30 reunião” ou “todo 12/03 aniversário da Ana”.
reminders-why-passed = Esse momento já passou.
reminders-why-no-such-day = Esse dia não existe no calendário.
reminders-why-unreadable = O arquivo dos lembretes não pode ser lido, então nada muda até ele ser consertado ou removido.
reminders-why-gone = Esse lembrete não existe mais.
reminders-why-not-ringing = Esse lembrete não está tocando.
reminders-why-full = Há lembretes demais; esqueça alguns antes.
cmd-reminders-new = Novo lembrete
ask-reminders-new = Quando, e o quê? (amanhã 9:00 ligar para a mãe)
cmd-reminders-when = Mudar quando toca
ask-reminders-when = Quando? (amanhã 9:00, toda segunda 8:30)
cmd-reminders-repeat = Mudar como se repete
cmd-reminders-sound = Ligar ou desligar o sino
cmd-reminders-forget = Esquecer este lembrete
cmd-reminders-dismiss = Dispensar este alarme
cmd-reminders-snooze = Adiar este alarme
reminders-heading-ringing = Tocando agora
reminders-heading-missed = Perdidos com a máquina desligada
reminders-heading-list = Seus lembretes
reminders-row-none = Nenhum lembrete ainda
reminders-row-file = O arquivo
reminders-row-unreadable = Não pode ser lido: conserte ou remova para seguir
reminders-row-dismiss = dispensar
reminders-row-snooze = Adiar, minutos
reminders-row-repeat = Repetir de outro jeito
reminders-row-days = Às
reminders-row-day = No dia
reminders-row-sound = Som
reminders-row-forget = Esquecer
reminders-repeat-once = Uma vez
reminders-repeat-day = Todo dia
reminders-repeat-week = Toda semana
reminders-repeat-month = Todo mês
reminders-repeat-year = Todo ano
reminders-sound-on = Toca um sino
reminders-sound-off = Em silêncio
reminders-weekday-monday = Segundas
reminders-weekday-tuesday = Terças
reminders-weekday-wednesday = Quartas
reminders-weekday-thursday = Quintas
reminders-weekday-friday = Sextas
reminders-weekday-saturday = Sábados
reminders-weekday-sunday = Domingos
reminders-noted-set = Marcado: { $text }, para { $at }
reminders-noted-rang = Tocou: { $text }
reminders-noted-missed = Perdido: { $text }, para { $at }
reminders-grammar-in = daqui a, daqui, em, dentro de
reminders-grammar-minute = min, mins, minuto, minutos, m
reminders-grammar-hour = h, hora, horas
reminders-grammar-day = dia, dias
reminders-grammar-week = semana, semanas
reminders-grammar-month = mês, meses
reminders-grammar-year = ano, anos
reminders-grammar-one = um, uma
reminders-grammar-half-hour = meia hora, meia-hora
reminders-grammar-today = hoje
reminders-grammar-tomorrow = amanhã
reminders-grammar-after-tomorrow = depois de amanhã
reminders-grammar-every = todo, toda, todos, todas, cada
reminders-grammar-daily = diariamente
reminders-grammar-weekly = semanalmente
reminders-grammar-monthly = mensalmente
reminders-grammar-yearly = anualmente
reminders-grammar-at = às, à, as, a, @
reminders-grammar-and = e, &
reminders-grammar-filler = o, os, no, na, nos, nas, de, do, da, dos, das
reminders-grammar-am = da manhã, de manhã, am
reminders-grammar-pm = da tarde, da noite, de tarde, de noite, pm
reminders-grammar-noon = meio-dia, meio dia
reminders-grammar-midnight = meia-noite, meia noite
reminders-grammar-ordinal = º, ª
reminders-grammar-monday = segunda, segunda-feira, segundas, segundas-feiras, seg
reminders-grammar-tuesday = terça, terça-feira, terças, terças-feiras, ter
reminders-grammar-wednesday = quarta, quarta-feira, quartas, quartas-feiras, qua
reminders-grammar-thursday = quinta, quinta-feira, quintas, quintas-feiras, qui
reminders-grammar-friday = sexta, sexta-feira, sextas, sextas-feiras, sex
reminders-grammar-saturday = sábado, sábados, sab
reminders-grammar-sunday = domingo, domingos, dom
reminders-grammar-january = janeiro, jan
reminders-grammar-february = fevereiro, fev
reminders-grammar-march = março, mar
reminders-grammar-april = abril, abr
reminders-grammar-may = maio, mai
reminders-grammar-june = junho, jun
reminders-grammar-july = julho, jul
reminders-grammar-august = agosto, ago
reminders-grammar-september = setembro, set
reminders-grammar-october = outubro, out
reminders-grammar-november = novembro, nov
reminders-grammar-december = dezembro, dez
reminders-grammar-sound-on = com som, com um som, com alarme, com toque
reminders-grammar-sound-off = sem som, em silêncio, silencioso, calado
