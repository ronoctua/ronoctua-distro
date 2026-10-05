# The disk-space module's own texts: its name and the line under it, the
# reason its row gives instead of a date, the hint of its mark, its page, and
# the two things it tells the footer.
brain-task-disk-space = Abrir espaço no disco
brain-task-disk-space-does = Quando o disco que guarda o sistema ou a sua pasta pessoal fica sem espaço, abre espaço na hora, sem esperar a noite nem a máquina ficar quieta: primeiro as compilações de compiladores que ninguém usa e, enquanto todas as compilações do cargo no seu cache pesarem mais do que a parte delas no disco, as compiladas há mais tempo; depois o lixo do store, até o disco voltar para onde deveria estar. Só apaga uma compilação quando nenhum cargo está compilando nela, nunca segue um link, nunca remove o que o Nix conta como raiz nem uma geração, e depois de uma tentativa que não abriu espaço suficiente espera o disco encher mais antes de tentar de novo. Enquanto o disco está quase cheio, uma marca na modeline diz isso.
disk-space-why-when-short = Com pouco espaço
disk-space-mark-full = O disco está quase cheio
disk-space-heading-now = O disco agora
disk-space-row-free = Livre agora
disk-space-row-size = Tamanho
disk-space-row-floor = Abre espaço abaixo de
disk-space-row-nearly-full = Quase cheio abaixo de
disk-space-row-unmeasured = O disco ainda não foi medido
notif-disk-space-relieved-summary = Espaço aberto no disco: { $freed } liberados
notif-disk-space-relieved-body = Agora { $free }% dele está livre.
notif-disk-space-still-short-summary = O disco continua sem espaço
notif-disk-space-still-short-body = Só { $free }% dele está livre, e nada mais podia sair sem você. O que o ocupa é seu para escolher: arquivos, programas, o histórico do seu próprio perfil ou as gerações antigas do sistema (a Memória do sistema pode apará-las). Isto é dito uma vez, até abrir espaço de novo.
