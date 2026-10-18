# WHAT `nix run .#bench` RUNS, as a function of the programs it measures.
#
# Section 5.2's targets on real hardware (section 9.3: never under lavapipe).
# One text and two callers: the `.#bench` app, over the programs the system
# ships, and `nix run .#profile-bench` (`nix/profile-bench.nix`), over each
# release profile's programs — so the profile decision is taken on the very
# benchmark the owner runs, and the two cannot drift apart.
#
#   benches      a package whose `bin/` holds the four `ronoctua-bench-*`
#                programs (layout, shaping, instance, index)
#   frames       `nix/bench-frames.nix` over the shell to measure
#   themes       the palettes the renderer reads at run time (`rust.keyed`)
#   component    the WASM component the instantiation is measured on
{
  benches,
  frames,
  themes,
  component,
}:
''
  echo "ronoctua-bench — section 5.2"
  echo
  echo "== the frame's CPU work =="
  # The renderer's colours are data read at run time (`rust.keyed`).
  export RONOCTUA_THEMES=${themes}
  ${benches}/bin/ronoctua-bench-layout "''${1:-20000}"
  echo
  echo "== text shaping =="
  ${benches}/bin/ronoctua-bench-shaping
  echo
  echo "== component instantiation =="
  # THE BRAIN'S BUFFER, since the icon browsers left for the native
  # app: the largest component the session still instantiates.
  ${benches}/bin/ronoctua-bench-instance \
    ${component} 200
  echo
  echo "== the super index: a million names, keystroke by keystroke =="
  ${benches}/bin/ronoctua-bench-index
  echo
  echo "== frame targets, on real hardware =="
  # Section 9.3 demands they be measured here and never under lavapipe. It needs a
  # graphics session; without one, the command says what is missing instead of
  # inventing a number.
  if [ -n "''${WAYLAND_DISPLAY:-}''${DISPLAY:-}" ]; then
    ${frames}/bin/ronoctua-bench-frames "''${2:-6}" "''${3:-15}"
  else
    echo "No graphical session: the frame targets were not measured."
    echo "Run this command from inside a Wayland or X session."
  fi
''
