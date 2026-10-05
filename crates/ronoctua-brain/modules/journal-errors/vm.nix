# The journal-errors module's subtest in `tests/brain-modules-vm.nix`: an
# error written into the real journal marks the modeline, and marking it seen
# takes the mark away. The helpers (`control`, `settled`, …) are that file's.
{ pkgs, ... }:

{
  script = ''
    with subtest("an error in the journal marks the modeline, and marking it seen takes the mark away"):
        import json

        mark = 'id="brain:journal-errors/unseen"'

        def planted():
            """The planted line's kind as the module keeps its list, or None: the mark alone
            would be lit by ANY error the machine writes meanwhile."""
            kept = machine.succeed(
                "cat /root/.local/state/ronoctua/brain/journal-errors/errors.json 2>/dev/null || echo '{}'"
            )
            for group in json.loads(kept).get("groups", []):
                if group.get("message", "").startswith("a planted error number "):
                    return group
            return None

        def plant(number):
            """ONE LINE AT `err`, written by a unit of its own: who said it is
            then the unit, the same both times. A `logger` straight from the
            test's shell can exit before journald reads its cgroup, and its
            line is then `backdoor.service`'s once and the tag's the next time
            — two kinds where the test plants one."""
            machine.succeed(
                "systemd-run --wait --quiet --unit=brain-modules-vm-plant "
                f"${pkgs.bash}/bin/sh -c 'echo \"<3>a planted error number {number}\"'"
            )

        def planted_is(what, test, timeout=30):
            for _ in range(timeout):
                group = planted()
                if group is not None and test(group):
                    return group
                machine.sleep(1)
            kept = machine.succeed(
                "cat /root/.local/state/ronoctua/brain/journal-errors/errors.json 2>/dev/null || echo '{}'"
            )
            rows = [g for g in json.loads(kept).get("groups", []) if "planted" in g.get("message", "")]
            raise AssertionError(f"the planted error never was {what}: {rows}")

        # WHAT THIS BOOT ALREADY WROTE AT `err` was heard on the first start
        # (the backfill), and may have marked the modeline before anything was
        # planted: seen first, so the mark below is the planted line's.
        machine.sleep(5)
        control("brain-act journal-errors seen-all")
        settled(lambda layout: layout and mark not in layout)

        # ONE LINE AT `err`: the kind of line every failing program writes.
        # The list says it is THAT line not seen yet — the mark alone could be
        # anybody's.
        plant(42)
        settled(lambda layout: mark in layout)
        planted_is("heard and not seen", lambda g: g["count"] == 1 and g["unseen"] >= 1)

        # SEEN, and the mark goes — it is not up while nothing is unseen.
        control("brain-act journal-errors seen-all")
        settled(lambda layout: layout and mark not in layout)
        planted_is("marked seen", lambda g: g["unseen"] == 0)

        # AND THE SAME KIND AGAIN, with another number, is news again — one
        # row, counted twice.
        plant(43)
        settled(lambda layout: mark in layout)
        planted_is(
            "one kind heard twice",
            lambda g: g["count"] == 2 and g["unseen"] >= 1 and g["message"].endswith("43"),
        )
        control("brain-act journal-errors seen-all")
        settled(lambda layout: layout and mark not in layout)
  '';
}
