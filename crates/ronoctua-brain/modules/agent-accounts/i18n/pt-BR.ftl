# The LLM accounts module's own texts: its name and the line under it, the
# hints of its two marks, its page — each account with how full it is, and the
# two lines a mark changes colour at — and the questions and refusals of the
# lines typed there. The accounts' VIEW speaks in its own words (`agents-…`).
brain-task-agent-accounts = Contas de LLM
brain-task-agent-accounts-does = Uma marca por conta de LLM na modeline, também em repouso, com a cor de quanto dos limites dela foi usado. Depois que uma conta foi usada — nunca por relógio — ele conta os tokens da conta e pergunta de novo ao endpoint de uso da Anthropic, quando você permitiu.
agent-accounts-mark-claude = Conta “{ $label }” do Claude Code: a cor diz quanto dos limites dela foi usado — um clique a abre
agent-accounts-mark-codex = Conta “{ $label }” do Codex: a cor diz quanto dos limites dela foi usado — um clique a abre
agent-accounts-why-when-used = Depois que uma conta foi usada
agent-accounts-why-not-a-percent = Isso não é um número inteiro de 1 a 100.
agent-accounts-why-not-in-order = O aviso tem de vir antes do perigo.
agent-accounts-why-unreadable = O arquivo das linhas (lines.json) não lê; nada foi gravado por cima dele.
agent-accounts-why-unkept = As linhas (lines.json) não puderam ser gravadas.
agent-accounts-heading-accounts = Contas, pelo mais usado dos limites delas (%)
agent-accounts-heading-lines = Quando uma marca muda de cor
agent-accounts-row-none = Nenhuma conta ainda: acrescente uma em Contas de LLM
agent-accounts-row-unreadable = A lista de contas não lê; nada é gravado por cima dela, e nenhuma marca é desenhada
agent-accounts-row-unwatched = Sem vigia
agent-accounts-row-lines-unwatched = As linhas estão sem vigia: uma mudada em outro lugar não chega às marcas
agent-accounts-note-claude = Claude Code
agent-accounts-note-codex = Codex
agent-accounts-note-no-mark = Sem marca: a modeline tem um dígito só para as nove primeiras contas
agent-accounts-unknown = Ainda não se sabe
agent-accounts-row-warn = Aviso a partir de (%)
agent-accounts-row-danger = Perigo a partir de (%)
cmd-agent-accounts-warn-at = Diz quanto uma janela tem de estar cheia para a marca avisar
cmd-agent-accounts-danger-at = Diz quanto uma janela tem de estar cheia para a marca virar perigo
ask-agent-accounts-warn-at = Aviso a partir de quantos por cento (1–100)
ask-agent-accounts-danger-at = Perigo a partir de quantos por cento (1–100)
