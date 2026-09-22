# THE CONTRACT TEST — the acceptance criterion of milestone 4.
#
# `ronoctua-compositor` is a promise about what a compositor does; this is what
# holds the promise to a real one. It starts a session in a VM and hands the
# socket to the binary, which exercises the TRAIT rather than the adapter's
# insides — so a second adapter, if there is ever one, passes the same binary.
#
# # THERE IS NO SECOND VERSION HERE ANY MORE, and that is a decision
#
# It used to run twice: against the pinned tag and against upstream's `main`,
# because the compositor before this one versioned by calendar and moved its
# IPC, so the only way for a removal not to be a surprise was to meet it
# months early. That cost ~7 min of CPU on every `flake update`.
#
# Half of this compositor's IPC is i3's, frozen by compatibility for a decade,
# and the half that is not — the fork's own commands — is checked at BUILD
# time, against the pinned source, by `nix/scripts/gen-deeds.py`. A verb that
# stops existing fails the build with its id in the message, which is earlier
# and cheaper than a VM.
#
# What stays here is what only a live compositor can answer: that the readings
# deserialise, that every deed is a command it accepts, that every `Ask` shape
# lands, and that a re-read validates.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-compositor-contract";

  nodes.machine = {
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;

    environment.systemPackages = [
      scroll
      ronoctua-shell
    ]
    ++ xvfb.packages;

    hardware.graphics.enable = true;
    services.dbus.enable = true;
  };

  testScript = ''
    ${xvfb.prelude}


    machine.start()
    machine.wait_for_unit("multi-user.target")
    start_xvfb()

    with subtest("the contract holds against the pinned compositor"):
        start_compositor("compositor", "${scroll}")
        out_text = machine.succeed(
            f"${ronoctua-shell}/bin/ronoctua-contract {socket_of('compositor')}"
        )
        print(out_text)
        assert "the contract holds" in out_text, out_text

        # THE VERSION GOES INTO THE LOG ON PURPOSE: when the contract fails,
        # the first question is "against which build?", and without this the
        # answer would require reopening the test.
        assert "compositor: " in out_text, out_text

        # AND THE THREE PASSES EACH SAID SOMETHING. A binary that exited zero
        # having skipped a pass would read exactly like one that passed it.
        for said in ("windows:", "outputs:", "deeds:", "asks:", "reread:"):
            assert said in out_text, f"the pass `{said}` did not report:\n{out_text}"

    # THE SYNTAX GATE IS NOT HERE, and that is a correction rather than an
    # omission.
    #
    # It was put here on a measurement that turned out to be incomplete:
    # `--validate` brings the SERVER up before it reads anything — libseat, a
    # VT, a backend, a renderer — and in a sandbox that ends in a core dump, so
    # a VM looked like the only place it could run. What was missing is the two
    # variables that take all of that away: `WLR_BACKENDS=headless` removes the
    # seat and the VT, `WLR_RENDERER=pixman` removes EGL, GBM and the DRM node.
    # With both it runs in a plain `runCommand` in about ten milliseconds.
    #
    # So it lives in `compositor-config-valid` and in
    # `display-fragment-validates`, which are CHECKS — earlier, cheaper, and
    # inside `nix flake check` where a VM is not. Both carry a negative half
    # that is RUN rather than assumed, because the exit status lies about an
    # error inside an `include`.
  '';
}
