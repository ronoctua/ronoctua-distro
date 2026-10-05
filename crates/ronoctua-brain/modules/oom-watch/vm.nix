# The oom-watch module's subtests in `tests/brain-modules-vm.nix`: a hog in a
# small group, killed by the kernel, is one notification — with what died and
# where — and one record; and a hog oomd kills, which the footer hears twice
# (oomd's bus signal and this module's event), is ONE notification that says
# what the journal knew. The helpers (`footer_journal`, …) are that file's.
{ pkgs, ... }:

{
  # OOMD ARMED AS THE DISTRO ARMS IT (`app.slice` of the person's manager,
  # killed on pressure), and the person's manager up without a login.
  node = {
    imports = [ ../../../../nix/modules/memory.nix ];
    users.users.noctua.linger = true;
    # A KILL IN A GROUP IS A KILL, NOT A PANIC: the test machine panics on
    # any OOM by default (`vm.panic_on_oom = 2`, the test instrumentation's),
    # which takes the hog's group kill for the whole machine's — the VM
    # reboots and the driver loses its pipe. The distro leaves it at 0.
    boot.kernel.sysctl."vm.panic_on_oom" = 0;
  };

  script = ''
    with subtest("a hog killed by the kernel in a small group is one notification"):
        # A HARD CEILING and no swap, so what dies is the hog and never the
        # machine: the kernel's own OOM killer, inside the group — "Memory
        # cgroup out of memory" — and systemd's word about the unit after it.
        # CALLED `tail`: the single-binary coreutils names its process after
        # its `argv[0]` (`PR_SET_NAME`), so started by its store path it is
        # "/nix/store/5kcc" to the kernel (measured), and `exec -a` gives it
        # the name a person would read.
        machine.succeed(
            "systemd-run --unit=hog -p MemoryMax=48M -p MemorySwapMax=0 "
            "-- ${pkgs.bash}/bin/bash -c 'exec -a tail ${pkgs.coreutils}/bin/tail /dev/zero'"
        )
        machine.wait_until_succeeds(
            "journalctl -k --no-pager | grep 'Memory cgroup out of memory: Killed process' > /dev/null",
            timeout=120,
        )
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'notification decided' "
            "| grep 'catalogue=\"oom-watch-killed\"' > /dev/null",
            timeout=60,
        )
        # WHAT DIED AND WHERE: the kernel's line and the unit's, as one.
        said = footer_journal()
        assert "closed for lack of memory" in said and "what=tail" in said, said[-3000:]
        # ONCE: the two lines of the kill, and the few seconds a cursor
        # re-reads, are one notification — waited for past the pairing's
        # patience before it is counted.
        machine.sleep(10)
        decided = [
            l for l in footer_journal().splitlines()
            if "notification decided" in l and 'catalogue="oom-watch-killed"' in l
        ]
        assert len(decided) == 1, "one kill, " + str(len(decided)) + " notifications:\n" + "\n".join(decided)

    with subtest("a hog oomd kills is one notification, with what the journal knew"):
        # FROM HERE ON is what this subtest counts: the kernel's kill above
        # raised a notification of its own.
        before = len(footer_journal())
        machine.wait_for_unit("user@1000.service")
        # THE HOG, in the slice oomd may kill, stalling under its own ceiling —
        # the shape `tests/oom-vm.nix` provokes oomd with.
        machine.succeed(
            "systemd-run --user --machine=noctua@ --slice=app.slice "
            "--unit=hog --quiet --property=MemoryHigh=16M "
            "-- ${pkgs.coreutils}/bin/tail /dev/zero"
        )
        machine.wait_until_succeeds(
            "systemctl --user --machine=noctua@ is-active hog.service", timeout=30
        )
        machine.wait_until_fails(
            "systemctl --user --machine=noctua@ is-active hog.service", timeout=180
        )
        # THE BRAIN TOLD IT, with oomd's own measure: oomd's line was heard
        # (the follower reads it at `notice`) and joined to the unit's.
        machine.wait_until_succeeds(
            "journalctl -u footer --no-pager | grep 'closed for lack of memory' "
            "| grep 'what=hog.service' > /dev/null",
            timeout=60,
        )
        # PAST THE PAIRING'S PATIENCE and the bus's arrival, whichever came
        # first, before anything is counted.
        machine.sleep(10)
        said = footer_journal()[before:]
        told = [l for l in said.splitlines() if "closed for lack of memory" in l and "what=hog.service" in l]
        assert len(told) == 1, "one kill told " + str(len(told)) + " times:\n" + "\n".join(told)
        assert 'by="pressure"' in told[0] or "by=pressure" in told[0], (
            "oomd's own line was not heard, only the unit's: " + told[0]
        )
        # ONE NOTIFICATION FOR THE TWO ARRIVALS: the bus's, amended with the
        # brain's words, or the brain's, with the bus's dropped.
        decided = [
            l for l in said.splitlines()
            if "notification decided" in l
            and ('catalogue="memory-killed"' in l or 'catalogue="oom-watch-killed"' in l)
        ]
        assert len(decided) == 1, "one kill, " + str(len(decided)) + " notifications:\n" + "\n".join(decided)
        assert (
            "the brain's kill added to the bus's notification" in said
            or "oomd's kill was told by the brain already" in said
        ), "the bus's notice and the brain's event were not taken for one kill:\n" + said[-3000:]
  '';
}
