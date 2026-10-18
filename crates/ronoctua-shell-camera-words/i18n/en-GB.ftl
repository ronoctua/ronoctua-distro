# The texts `ronoctua-shell-camera-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

studio-trouble-no-camera = No camera is connected.
studio-trouble-camera-busy = The camera is being used by { $program }.
studio-trouble-camera-silent = The camera stopped sending pictures; it will be tried again.
studio-trouble-no-model = The model that finds the person is not installed, so the background cannot be blurred, replaced or removed.
studio-trouble-no-hand-model = The models that find hands are not installed, so gestures cannot be read.
studio-trouble-no-backdrop = No picture is chosen for the replaced background (Camera settings, Effects).
studio-trouble-no-ffmpeg = ffmpeg could not be run, so no camera can be opened.
studio-trouble-no-screen = The screen cannot be captured here.
studio-trouble-no-window = No open window matches this scene's window.
studio-trouble-no-virtual = The virtual camera device does not exist: the kernel module is not loaded.
studio-trouble-virtual-busy = Another program is writing to the virtual camera.
studio-trouble-encoder = The recording could not go on: { $why }
studio-trouble-stream = The stream stopped: { $why }
studio-trouble-no-stream-target = Set the stream's address and key first (Camera settings, Streaming).
studio-trouble-name = That name is already used, or it is the last scene.
studio-trouble-write = A file could not be written: { $why }

## file info editing — everything the info panel can change

studio-trouble-no-face = A text in the scene has no typeface to be drawn with.
studio-trouble-rehearsal = This is the development loop's studio: it rehearses, and writes no photo, recording or stream.
