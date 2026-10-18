# The texts `ronoctua-shell-lifeline` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# =====================================================================
# THE LINE THAT OUTLIVES THE FOOTER — when the interface itself falls
# =====================================================================
#
# The owner moved the pointer across the hint line and everything closed. He
# named two defects and only one of them was the crash: *"o outro é o erro de
# que a ronoctua-distro não sabe lidar com isso, ou seja, acaba deixando o
# usuário sem interface"*. These are the words of the answer to the second one.
#
# It is a second layer-shell row raised by systemd's `OnFailure=`, and it is not
# a second chrome (invariant 14): it is the footer's own row drawn by another
# process, alive exactly while there is no chrome at all.
lifeline-fell = A interface fechou sozinha.
lifeline-fell-and-came-back = A interface fechou sozinha e voltou.
# THE UNIT'S OWN WORDS, and they are DATA — what the machine said, the way a
# filename is. `{ $why }` is systemd's `Result=`: `core-dump`, `signal`,
# `oom-kill`, `exit-code`.
lifeline-why = { $unit } · { $why }
# AND THE UNIT ALONE, when systemd has nothing to say about it — a
# transient unit is gone by the time the line asks. A dot between one
# thing and nothing separates nothing.
lifeline-unit = { $unit }
lifeline-logs = ver os logs
lifeline-restart = reiniciar a interface
lifeline-ai = perguntar a uma IA
lifeline-close = fechar
lifeline-logs-opened = o journal está aberto num terminal
lifeline-restarting = pedi ao systemd para subir de novo
lifeline-would-not-start = o systemd não subiu; os logs dizem por quê
lifeline-no-journalctl = o journal não abriu; o journalctl não está nesta máquina
# THE NESTED LOOP HAS NO UNIT. `nix/footer-single.nix` starts the footer as an
# ordinary background child, so there is nothing for systemd to restart — and
# saying so beats answering about a unit it has never heard of.
lifeline-not-a-unit = aqui não há unidade do systemd; o laço aninhado sobe o rodapé à mão

## file info editing — everything the info panel can change

# O "pedir a uma IA" da linha de vida, pelo Claude Code: o que sairia é
# juntado, mostrado num terminal, e perguntado com o "não" primeiro.
lifeline-ai-gathering = juntando o que iria: a unidade, como terminou, o journal e o core dump
lifeline-ai-no-report = o relatório não pôde ser escrito, e nada saiu desta máquina
lifeline-ai-shown = o que iria para o Claude Code está aberto num terminal; nada saiu ainda — enviar?
lifeline-ai-no = não, fica aqui
lifeline-ai-yes = sim, perguntar ao Claude Code
lifeline-ai-kept = nada saiu desta máquina, e o relatório foi apagado
lifeline-ai-opened = o Claude Code está aberto num terminal, com o relatório
lifeline-ai-no-claude = o Claude Code não está nesta máquina; ele vem com os agentes do perfil de desenvolvimento
lifeline-ai-prompt = A interface do meu computador (a unidade de usuário do systemd { $unit }) falhou. O arquivo { $path } tem a unidade, como ela terminou, as últimas linhas do journal e o core dump. Leia, me diga o que provavelmente aconteceu e o que eu posso fazer.
lifeline-report-title = O que aconteceu com a interface
lifeline-report-unit = Unidade
lifeline-report-result = Como terminou
lifeline-report-journal = As últimas linhas do journal
lifeline-report-core = O core dump
lifeline-report-nothing = (nada)
