# The machine does not freeze when memory runs out — and the desktop is not the victim.
#
# # The first test in this repository that allocates memory
#
# It does it inside a cgroup with a hard ceiling, so what is at risk is the hog
# and never the virtual machine. A test that provoked real system-wide pressure
# would be a test whose failures are indistinguishable from its successes.
#
# # What runs in `session.slice` here is a sleeper, not the compositor
#
# Building the graphical stack would double this test's cost and prove nothing
# more. `systemd-oomd` chooses its victims BY CGROUP: it only ever considers
# descendants of a monitored cgroup, and `session.slice` is a SIBLING of the
# monitored `app.slice`. What the sleeper stands for is exactly that structural
# fact — what is in that slice is outside the candidate set, whatever it is.
#
# The real compositor and the real footer are covered by
# `memory-policy-shape`, which asserts that they are in `session.slice` and that
# they never give up restarting.
#
# # Two nodes, and that is the point of the file
#
# The policy is written in PERCENTAGES so it adapts to the machine without
# anything computing anything. One node cannot prove that: a number that is
# merely present looks identical to a number that varies. Two nodes with
# different `memorySize`, one closure, and the large node's reservation has to be
# twice the small one's.
{
  pkgs,
  lib,
  ronoctua,
}:

let
  # The reservation the policy asks for, as a fraction. Read from the module's
  # own default rather than written again here — a second copy is a second thing
  # to keep in step, and this test exists to catch drift.
  minPercent = 3;

  node =
    megabytes:
    { ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/memory.nix
      ];
      _module.args.ronoctua = ronoctua;

      virtualisation.memorySize = megabytes;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-memoria";
      users.users.root.hashedPassword = lib.mkForce null;

      # THE USER MANAGER HAS TO BE UP, or `app.slice` does not exist and there is
      # nothing for `systemd-oomd` to be told about: the per-user manager is what
      # reports its managed cgroups over varlink. Lingering starts it without a
      # login, which is what a test has.
      users.users.noctua.linger = true;

      environment.systemPackages = with pkgs; [
        systemd # oomctl
        util-linux # choom, swapon
      ];
    };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-oom";

  nodes = {
    small = node 1024;
    large = node 2048;
  };

  testScript = ''
    start_all()
    for m in (small, large):
        m.wait_for_unit("multi-user.target")
        m.wait_for_unit("user@1000.service")

    with subtest("o oomd esta de pe, e nao pulado em silencio"):
        # `systemd-oomd.service` is Condition-gated on /proc/pressure/*. A failed
        # condition is not an error: the unit is SKIPPED and nothing says so.
        large.succeed("systemctl is-active systemd-oomd")
        large.succeed("test -e /proc/pressure/memory")

    with subtest("ele vigia os aplicativos, e nao o desktop"):
        dump = large.succeed("oomctl")
        print(dump)
        assert "app.slice" in dump, f"a app.slice nao esta sendo vigiada:\n{dump}"
        # The two that would put the compositor and the footer in the candidate
        # set. `session.slice` is where they live; `/` is the root slice, whose
        # candidates include everything.
        assert "session.slice" not in dump, f"a session.slice virou candidata:\n{dump}"
        assert "\n\tPath: /\n" not in dump, f"a raiz virou candidata:\n{dump}"

    # THE STAND-IN COMES UP FIRST, and not only because the next subtest reads
    # its slice: `session.slice` does not EXIST until something runs in it, so
    # reading `memory.min` before this point asks about a cgroup that has not
    # been created. On the real machine the compositor is what creates it.
    for m in (small, large):
        m.succeed(
            "systemd-run --user --machine=noctua@ --slice=session.slice "
            # BY PATH AND NOT BY NAME. Inside a systemd unit the `PATH` is
            # minimal, and this repository has paid for that lesson more than
            # once — the failure lands far from the cause, as a unit that starts
            # and is not active a moment later.
            "--unit=stand-in --quiet -- ${pkgs.coreutils}/bin/sleep infinity"
        )
        m.wait_until_succeeds(
            "systemctl --user --machine=noctua@ is-active stand-in.service", timeout=30
        )

    with subtest("a protecao do desktop chega ao fim da corrente"):
        # `memory.min` is capped by every ancestor: this file reads the EFFECTIVE
        # value at the leaf, which is the only number that says whether the four
        # drop-ins actually took.
        path = ("/sys/fs/cgroup/user.slice/user-1000.slice/"
                "user@1000.service/session.slice/memory.min")
        got = {}
        for name, m in (("small", small), ("large", large)):
            got[name] = int(m.succeed(f"cat {path}").strip())
            total = int(m.succeed(
                "awk '/^MemTotal:/ {print $2}' /proc/meminfo").strip()) * 1024
            expected = total * ${toString minPercent} / 100
            assert got[name] > 0, f"{name}: memory.min efetivo e ZERO — a corrente quebrou"
            assert abs(got[name] - expected) < expected * 0.15, (
                f"{name}: memory.min {got[name]} nao bate com "
                f"${toString minPercent}% de {total}"
            )

        # AND IT VARIES WITH THE MACHINE. One node proves the number exists; two
        # prove the percentage was resolved against THIS machine's memory, which
        # is the whole reason the policy is written in percentages.
        # The numbers go into the log: a check that proves an adaptation and
        # does not show it is a check nobody can sanity-read afterwards.
        print(f"memory.min: small={got['small']} large={got['large']}")
        ratio = got["large"] / got["small"]
        assert 1.7 < ratio < 2.3, (
            f"o dobro de RAM tinha de dar o dobro de reserva; deu {ratio:.2f}x "
            f"({got['small']} -> {got['large']})"
        )

    with subtest("o zram absorve antes do disco"):
        swaps = large.succeed("swapon --show=NAME,PRIO --noheadings")
        print(swaps)
        assert "zram" in swaps, f"sem zram, a maquina troca direto no disco:\n{swaps}"

    with subtest("o aplicativo morre, e o que esta na session.slice nao e tocado"):
        # The hog, in the slice oomd may kill. `MemoryHigh` keeps the allocation
        # bounded: what thrashes is this one cgroup, so pressure rises inside
        # `app.slice` while the virtual machine itself is never at risk.
        #
        # A SERVICE and not a scope, although real applications run in scopes: a
        # scope runs in the foreground and the test would have to background it,
        # and a backgrounded launch that quietly fails is indistinguishable from
        # one that succeeded. What matters to oomd is the same either way — a
        # leaf cgroup under `app.slice`.
        large.succeed(
            "systemd-run --user --machine=noctua@ --slice=app.slice "
            "--unit=hog --quiet --property=MemoryHigh=16M "
            "-- ${pkgs.coreutils}/bin/tail /dev/zero"
        )

        # IT IS ALIVE BEFORE IT IS DEAD, and this line is not ceremony. Without
        # it the wait below succeeds the instant the hog fails to start, and the
        # subtest passes having proved nothing — which is exactly what happened
        # the first time this test ran, in half a second.
        large.wait_until_succeeds(
            "systemctl --user --machine=noctua@ is-active hog.service", timeout=30
        )

        # `ManagedOOMMemoryPressureDurationSec` is 15 s, so the answer cannot
        # arrive before that. The margin is for oomd's poll interval.
        large.wait_until_fails(
            "systemctl --user --machine=noctua@ is-active hog.service", timeout=180
        )

        # THE ASSERTION THIS FILE EXISTS FOR: not "it came back", but "it was
        # never touched".
        assert "0" == large.succeed(
            "systemctl --user --machine=noctua@ show stand-in.service "
            "-p NRestarts --value"
        ).strip(), "o que esta na session.slice foi reiniciado, logo foi morto"
        large.succeed("systemctl --user --machine=noctua@ is-active stand-in.service")

    with subtest("foi o oomd, com os numeros configurados"):
        # NOT a grep for "killed". That word is logged by the USER MANAGER, not
        # by oomd, so matching it would pass on a kernel OOM kill too — which is
        # the outcome this whole design exists to avoid, and the one a lazy
        # assertion would call a success.
        #
        # What oomd itself writes names the slice, the measured pressure, the
        # LIMIT and the DURATION. Asserting on those proves three things at once:
        # that the decision was oomd's, that it was taken against `app.slice`,
        # and that the two configured values are the ones in force — including
        # `ManagedOOMMemoryPressureDurationSec`, a key systemd only learned in
        # 257 and would otherwise drop with a warning and no failure.
        said = large.succeed("journalctl -u systemd-oomd --no-pager")
        assert "for killing due to memory pressure" in said, (
            f"quem matou nao foi o oomd:\n{said[-2000:]}"
        )
        assert "app.slice" in said
        assert "> 50.00%" in said, f"o limite configurado nao e o que valeu:\n{said[-2000:]}"
        assert "for > 15s" in said, f"a duracao configurada nao valeu:\n{said[-2000:]}"
  '';
}
