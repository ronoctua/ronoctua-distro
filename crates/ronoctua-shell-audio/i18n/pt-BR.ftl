# The texts `ronoctua-shell-audio` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Syncthing (LEADER s s)

# Syncthing (LEADER s s) — one view, two doors, and the mark the
# modeline may draw. The option labels below are owed by
# `assets/syncthing-options.nix` through the generator; `syncthing-options-have-text`
# is what says so.
echo-audio-will-not-parse = O audio.json não parseia, e escrever o substituiria. Conserte ou remova o arquivo.
# =====================================================================
# SOM — `LEADER s a`, duas portas, e as marcas que a modeline desenha
# =====================================================================
group-audio = som
audio-title = Som
cmd-audio-open = Som, no rodapé
cmd-audio-window = Som, em janela
cmd-audio-up = Linha anterior
cmd-audio-down = Próxima linha
cmd-audio-enter = Usar este
cmd-audio-less = Mais baixo, ou o valor anterior
cmd-audio-more = Mais alto, ou o próximo valor
cmd-audio-mute = Silenciar, ou devolver o som
cmd-audio-default = Tocar por este de agora em diante
audio-up = subir
audio-down = descer
audio-enter = usar
audio-less = baixar
audio-more = aumentar
audio-mute = silenciar
audio-default = usar sempre
# Os títulos, na ordem em que se lê.
audio-heading-output = Tocando por
audio-heading-input = Ouvindo por
audio-heading-apps = Programas
audio-heading-card = A placa
audio-heading-settings = Ajustes
audio-chosen-elsewhere = Escolhido: { $device }, que não está aqui
audio-nothing-runs = Nenhum programa está tocando nem gravando.
audio-muted = mudo
audio-in-use = em uso
audio-profile = O que a placa está fazendo
audio-socket-out = Sai pela
audio-socket-in = Grava pela
audio-port-none = Nenhuma tomada em uso.
audio-unplugged = nada conectado
audio-card-off = Esta placa está desligada.
audio-listening-now = ouvindo agora
audio-setting-step = Uma tecla move
audio-setting-over = Permitir acima do cheio
audio-setting-flash = Mostrar a linha acima da modeline
audio-on = sim
audio-off = não
audio-step-points = { $points }%
# As tomadas para as quais temos palavra própria. O resto sai nas do PipeWire.
audio-port-speaker = Alto-falantes
audio-port-headphones = Fones de ouvido
audio-port-headset = Fone com microfone
audio-port-headset-mic = Microfone do fone
audio-port-internal-mic = Microfone interno
audio-port-microphone = Microfone
audio-port-line-in = Entrada de linha
audio-port-line-out = Saída de linha
