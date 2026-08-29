# Contract test of niri's IPC — acceptance criterion of milestone 4.
#
# "The contract test passes against the pinned tag **and** against the
# `niri-next` input."
#
# WHY BOTH VERSIONS
# -----------------
# Invariant 3 forbids forking and patching niri. Upstream versions by calendar
# and releases every few months. If a release removes or renames something we
# depend on, there is no local technical way out — only stopping and reporting
# (section 16). The only way for that moment not to be a surprise is to run the
# SAME contract against upstream's `main`, months before that release becomes
# mandatory. It is what the mitigation of risk R2 asks for.
#
# Running against the pinned tag alone would prove none of that: it does not change.
#
# COST
# ----
# `niri-next` follows `main`, so it compiles from scratch on every
# `flake update`. That is ~7 min of CPU. It is the price of the early warning,
# and it is recorded here so as not to be confused with accidental slowness.
{
  pkgs,
  lib,
  niri,
  niriNext,
  ronoctua-shell,
}:

let
  xvfb = import ./niri-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-niri-contract";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      niri
      niriNext
      ronoctua-shell
    ]
    ++ xvfb.packages;

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    def contract(unit):
        """Runs the contract binary against the given unit's niri."""
        return machine.succeed(
            f"${ronoctua-shell}/bin/niri-contract {socket_of(unit)}"
        )


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()

    with subtest("the contract against the pinned tag"):
        start_niri("niri", "${niri}")
        out_text = contract("niri")
        print(out_text)
        assert "contract OK" in out_text, out_text

        # The version goes into the log on purpose: when the contract fails, the
        # first question is "against which niri?", and without this the answer
        # would require reopening the test.
        assert "niri: " in out_text, out_text

    with subtest("EVERY keymap action exists in this niri"):
        # The window and workspace shortcuts stopped being niri binds and became
        # shell commands, dispatched by ACTION NAME over IPC. The name travels as
        # a string: a `MoveColumnLeft` wrong by one letter compiles, runs, and
        # niri answers "error parsing request" — which becomes a `warn` in the log
        # and NOTHING on screen.
        #
        # Rust's `match` does not protect against that: it checks the command's
        # id, not the name on the other side. Whoever checks is this.
        out_text = machine.succeed(
            f"${ronoctua-shell}/bin/ronoctua-niri-actions {socket_of('niri')}"
        )
        print(out_text)
        assert "OK: the keymap's" in out_text, out_text

        # It brings the previous one down before starting the next: two niris on
        # the same :99 would fight over the winit window.
        stop_niri("niri")

    with subtest("the contract against niri-next"):
        # `niri-next` follows upstream's `main`. This subtest failing with the one
        # above passing is the drift warning: there is something coming in the next
        # release that breaks the boundary. See risk R2 in the ARCHITECTURE.md.
        start_niri("niri-next", "${niriNext}")
        out_text = contract("niri-next")
        print(out_text)
        assert "contract OK" in out_text, out_text

    with subtest("the two versions really are different"):
        # Without this the test would pass trivially if `niri-next` resolved to
        # the same derivation as the pinned tag — which really happens right
        # after a release, and would make the drift warning disappear silently
        # exactly when it matters least, but would also not warn that it had
        # stopped being worth anything.
        assert "${niri}" != "${niriNext}", (
            "niri and niri-next resolved to the same derivation; "
            "run `nix flake update niri-next`"
        )
  '';
}
