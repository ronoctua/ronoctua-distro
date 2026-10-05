# The journal-errors module's own texts: its name and the line under it, its
# page, the hint of its mark, its four actions and the words it refuses with.
brain-task-journal-errors = Erros no journal
brain-task-journal-errors-does = Erros que o sistema e os seus programas escrevem no journal, agrupados: a mesma falha com outros números é uma linha, com a contagem. Uma marca na modeline conta os tipos que você ainda não viu.
journal-errors-heading-journal = Acompanhando o journal
journal-errors-heading-unseen = Ainda não vistos — escolha um para marcá-lo como visto
journal-errors-heading-earlier = Ainda não vistos, de uma inicialização anterior — escolha um para marcá-lo como visto
journal-errors-heading-seen = Vistos — escolha um para ignorar esse tipo daqui em diante
journal-errors-heading-ignored = Ignorados, com quantos desde então — escolha um para voltar a ouvi-lo
journal-errors-row-none = Nenhum erro foi escrito no journal desde que isto começou a vigiar
journal-errors-mark-unseen = { $count ->
        [one] Um tipo de erro no journal ainda não visto
       *[other] { $count } tipos de erro no journal ainda não vistos
    }
journal-errors-why-gone = Esse erro não está mais na lista
journal-errors-why-no-row = Escolha antes um erro na página: isto age sobre uma linha
journal-errors-why-unreadable = O arquivo dos tipos ignorados está com defeito e fica como está: corrija-o ou remova-o antes
cmd-journal-errors-seen = Marcar este erro como visto
cmd-journal-errors-ignore = Ignorar este tipo de erro
cmd-journal-errors-unignore = Voltar a ouvir este tipo de erro
cmd-journal-errors-seen-all = Marcar todos os erros como vistos
