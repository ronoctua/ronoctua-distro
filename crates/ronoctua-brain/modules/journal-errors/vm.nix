# The journal-errors module's subtests in `tests/brain-modules-vm.nix`: an
# error written into the real journal marks the modeline and marking it seen
# takes the mark away, a shipped filter holds its line, a plain warning stays
# off the mark, and what was written while it was off is read at a person's
# word, once. The helpers (`control`, `settled`, …) are that file's.
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
                machine.sleep(duration=timedelta(seconds=1))
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
        machine.sleep(duration=timedelta(seconds=5))
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

        # A LINE THAT MOVES NO COUNT is written with the journal's cursor, or
        # within half a minute at the latest: this waits for that and a margin.
        for _ in range(60):
            f = filtered()
            if f is not None and f.get("since", 0) >= 1:
                break
            machine.sleep(duration=timedelta(seconds=1))
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

    with subtest("a plain warning is kept in a list of its own, off the mark"):
        def kept():
            """The list as the module keeps it."""
            return json.loads(
                machine.succeed(
                    "cat /root/.local/state/ronoctua/brain/journal-errors/errors.json 2>/dev/null || echo '{}'"
                )
            )

        machine.succeed(
            "systemd-run --wait --quiet --unit=brain-modules-vm-warned "
            "${pkgs.bash}/bin/sh -c 'echo \"<4>a planted warning number 7\"'"
        )

        for _ in range(60):
            if any(g.get("message", "").startswith("a planted warning ") for g in kept().get("warnings", [])):
                break
            machine.sleep(duration=timedelta(seconds=1))
        else:
            raise AssertionError(f"the planted warning never reached its list: {kept().get('warnings')}")

        assert not any(
            "a planted warning" in g.get("message", "") for g in kept().get("groups", [])
        ), "a plain warning reached the list of errors"
        settled(lambda layout: layout and mark not in layout)

    with subtest("a read by hand is refused while on, and off it reads what was missed, once"):
        # AS THE PAGE SENDS IT: by name, with no row.
        run = {"ask": "act", "module": "journal-errors", "action": "run"}

        # ON, IT HEARS EVERYTHING AS IT HAPPENS, and nothing is left to read.
        said = brain(run)
        assert said.get("say") == "declined" and said.get("key") == "brain-why-listening", (
            f"a read by hand was not refused while on: {said}"
        )

        said = brain({"ask": "set", "task": "journal-errors", "knob": {"enabled": False}})
        assert said.get("say") not in ("refused", "declined"), f"switching it off was refused: {said}"
        row = next((t for t in said["mind"]["tasks"] if t["id"] == "journal-errors"), None)
        assert row is not None and "run" in [a["id"] for a in row["actions"]], (
            f"switched off, it offers no read by hand: {row}"
        )

        # OFF, THE JOURNAL GOES ON AND THE LIST DOES NOT.
        counted = planted()["count"]
        plant(44)
        machine.sleep(duration=timedelta(seconds=7))
        assert planted()["count"] == counted, "a module switched off heard a line"

        said = brain(run)
        assert said.get("say") not in ("refused", "declined"), f"the read by hand was refused: {said}"
        planted_is(
            "read by hand",
            lambda g: g["count"] == counted + 1 and g["message"].endswith("44"),
        )

        # AGAIN: nothing new, and what the first read read is not read twice.
        said = brain(run)
        assert said.get("say") not in ("refused", "declined"), f"the second read was refused: {said}"
        for _ in range(30):
            if kept().get("last-run", {}).get("listed") == 0:
                break
            machine.sleep(duration=timedelta(seconds=1))
        else:
            raise AssertionError(f"the second read never ended: {kept().get('last-run')}")
        assert planted()["count"] == counted + 1, "a line read by a pass was read again"

        # BACK ON, nothing is repeated: the place is past that line.
        said = brain({"ask": "set", "task": "journal-errors", "knob": {"enabled": True}})
        assert said.get("say") not in ("refused", "declined"), f"switching it on was refused: {said}"
        machine.sleep(duration=timedelta(seconds=7))
        assert planted()["count"] == counted + 1, "a line came twice when it came back on"
  '';
}
