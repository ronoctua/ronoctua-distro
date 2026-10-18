# The texts `ronoctua-shell-camera-words` says, in pt-BR (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

studio-trouble-no-camera = Nenhuma câmera está conectada.
studio-trouble-camera-busy = A câmera está sendo usada por { $program }.
studio-trouble-camera-silent = A câmera parou de mandar imagens; ela será tentada de novo.
studio-trouble-no-model = O modelo que encontra a pessoa não está instalado, então o fundo não pode ser desfocado, trocado nem removido.
studio-trouble-no-hand-model = Os modelos que encontram as mãos não estão instalados, então os gestos não podem ser lidos.
studio-trouble-no-backdrop = Nenhuma imagem foi escolhida para o fundo trocado (Ajustes da câmera, Efeitos).
studio-trouble-no-ffmpeg = O ffmpeg não pôde rodar, então nenhuma câmera pode ser aberta.
studio-trouble-no-screen = A tela não pode ser capturada aqui.
studio-trouble-no-window = Nenhuma janela aberta corresponde à janela desta cena.
studio-trouble-no-virtual = O dispositivo da câmera virtual não existe: o módulo do kernel não está carregado.
studio-trouble-virtual-busy = Outro programa está escrevendo na câmera virtual.
studio-trouble-encoder = A gravação não pôde continuar: { $why }
studio-trouble-stream = A transmissão parou: { $why }
studio-trouble-no-stream-target = Defina antes o endereço e a chave da transmissão (Ajustes da câmera, Transmissão).
studio-trouble-name = Esse nome já está em uso, ou é a última cena.
studio-trouble-write = Um arquivo não pôde ser escrito: { $why }

## file info editing — everything the info panel can change

studio-trouble-no-face = Um texto da cena não tem tipo de letra com que ser desenhado.
studio-trouble-rehearsal = Este é o estúdio do laço de desenvolvimento: ele ensaia, e não grava foto, gravação nem transmissão.
