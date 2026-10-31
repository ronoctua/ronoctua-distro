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

        # SHIPPED SWITCHED OFF (`startsOff`): with no entry of the person's in
        # `brain.json` the row reads off, and an off module hears nothing and
        # shows no mark — every wait below would wait for nothing. So it is
        # switched on first, by the one knob the row's own switch sends; the
        # brain rings its watch thread, which starts hearing the journal from
        # then on.
        # (`row`, not the harness's `rows`: the script is one scope, and that
        # one is a set.)
        row = next((t for t in mind()["tasks"] if t["id"] == "journal-errors"), None)
        assert row is not None and not row["enabled"], f"journal-errors does not ship switched off: {row}"
        said = brain({"ask": "set", "task": "journal-errors", "knob": {"enabled": True}})
        assert said.get("say") not in ("refused", "declined"), f"switching it on was refused: {said}"
        row = next((t for t in said["mind"]["tasks"] if t["id"] == "journal-errors"), None)
        assert row is not None and row["enabled"], f"journal-errors is still off: {row}"

        # WHAT THE MACHINE WRITES AT `err` from the moment it is on may mark
        # the modeline before anything is planted: seen first, so the mark
        # below is the planted line's.
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

    with subtest("a line the shipped filter holds is counted on the filter and never on the mark"):
        def filtered():
            """The shipped default filter's record in the kept list, or None."""
            kept = machine.succeed(
                "cat /root/.local/state/ronoctua/brain/journal-errors/errors.json 2>/dev/null || echo '{}'"
            )
            for f in json.loads(kept).get("filtered-since", []):
                if f.get("text") == "Ignoring duplicate name":
                    return f
            return None

        # THE DBUS NOISE THE SHIPPED FILTER IS FOR: one err line carrying the
        # snippet, written by a unit of its own as the plant above. It must be
        # counted on the filter, never become a kind, and never light the mark.
        machine.succeed(
            "systemd-run --wait --quiet --unit=brain-modules-vm-filtered "
            "${pkgs.bash}/bin/sh -c 'echo \"<3>Ignoring duplicate name org.example.Planted in a service file\"'"
        )

        for _ in range(30):
            f = filtered()
            if f is not None and f.get("since", 0) >= 1:
                break
            machine.sleep(1)
        else:
            raise AssertionError("the shipped filter never caught the planted line")

        # NOT A KIND, and the mark stays down: the filtered line is off the list.
        kept = json.loads(
            machine.succeed(
                "cat /root/.local/state/ronoctua/brain/journal-errors/errors.json"
            )
        )
        assert not any(
            "Ignoring duplicate name" in g.get("message", "") for g in kept.get("groups", [])
        ), "a filtered line became a kind"
        settled(lambda layout: layout and mark not in layout)
  '';
}
