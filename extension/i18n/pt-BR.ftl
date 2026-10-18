# The texts the browser extension says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# ---------------------------------------------------------------------------
# A EXTENSÃO DO NAVEGADOR.
#
# São QUINZE chaves e o `nix/scripts/gen-extension.py` cobra todas nas duas
# línguas: uma que falte não vira texto em inglês, vira o nome da própria chave
# num botão. O popup não escreve palavra nenhuma — ele monta a tela a partir
# daqui.
# ---------------------------------------------------------------------------
ext-title = ronoctua
ext-queue-up = a fila está no ar
ext-queue-down = a fila não está no ar
ext-video = vídeo, na melhor qualidade
ext-audio = só o áudio
ext-subtitles = só a legenda
ext-scan = procurar mídia nesta página
ext-found-none = nada encontrado nesta página
ext-page-not-media = esta aba não tem endereço para baixar
ext-sent = mandado para a fila
ext-failed = a fila recusou
ext-intercept-on = downloads vão para a fila
ext-intercept-off = downloads ficam no navegador
ext-send-link = baixar este link pela ronoctua
ext-send-media = baixar a mídia desta página
ext-as-me = como você — não é anônimo
ext-anonymous = anônimo
