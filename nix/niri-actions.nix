# The compositor's action table, extracted from the PINNED niri.
#
# # Why it is generated
#
# niri exposes 141 actions and versions by calendar. Written by hand, they
# would age in silence: a removed action becomes a command that answers
# "error parsing request" — which is a `warn` in the log and nothing on screen —
# and an added one would never show up in the list.
#
# Here the source is `niri msg action --help` from the SAME binary the session
# is going to run. Bumping niri's tag updates the table along with it, and the
# i18n check then demands a translation for whatever appeared.
#
# # Why a derivation
#
# Reading this during evaluation would be import-from-derivation: the flake's
# evaluation would stop to wait for a build. As a derivation, it is only built
# when someone compiles the Rust — and the path reaches `build.rs` through an
# environment variable, like the tokens and the i18n.
#
# The SAME derivation emits the DEFAULT BINDS, from the pinned commit's
# `default-config.kdl`. They are two artefacts of the same upstream, and
# separating them would give two derivations that can point at different
# versions of niri — which is the kind of divergence that only shows up when a
# shortcut disappears.
{
  runCommand,
  python3,
  niri,
  niriSrc,
}:

runCommand "ronoctua-niri-actions" { } ''
  mkdir -p $out
  ${python3}/bin/python3 ${./scripts/gen-niri-actions.py} \
    ${niri}/bin/niri $out/niri-actions.rs
  ${python3}/bin/python3 ${./scripts/gen-default-binds.py} \
    ${niriSrc}/resources/default-config.kdl $out/binds-default.rs

  # The generator already stops below 100 actions; this is the second net, in
  # case it exits successfully and writes nothing.
  if [ ! -s $out/niri-actions.rs ] || [ ! -s $out/binds-default.rs ]; then
    echo "ERRO: uma das tabelas do niri saiu vazia." >&2
    exit 1
  fi
''
