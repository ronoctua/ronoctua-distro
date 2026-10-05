# The reminders' subtest in `tests/brain-modules-vm.nix`, which runs every
# module's `vm.nix` without naming any.
#
# WHAT ONLY THE VM PROVES: the whole way a reminder travels on a booted
# machine — asked for through the brain's socket in a person's words, armed
# on the one clock of the real watch thread, raised as an ALARM in the mind
# the footer reads, still there a minute and a half later with nobody
# answering, gone when dismissed, written into the activity log, and taken out
# of the file a once-only reminder lives in. The chime is asked for too; the
# VM has no sound, so what is proved about it is that ringing does not wait
# for it.
{ ... }:
{
  node = { };

  script = ''
    with subtest("reminders: a reminder due in a minute rings as an alarm and stays until dismissed"):
        # THE FOOTER, AND THE BRAIN IT WAKES, RUN AS ROOT in the harness.
        file = "/root/.config/ronoctua/brain/reminders/reminders.json"
        said = brain({
            "ask": "act",
            "module": "reminders",
            "action": "new",
            "row": None,
            "answer": "in 1 min with a sound the alarm of the VM",
        })
        assert said.get("say") not in ("refused", "declined"), f"the reminder was refused: {said}"

        def alarms():
            return [
                n["what"] for n in mind()["notices"]
                if n["module"] == "reminders" and n["what"]["level"] == "alarm"
            ]

        assert not alarms(), "it rang the moment it was asked for"
        kept = json.loads(machine.succeed(f"cat {file}"))
        assert [r["text"] for r in kept["reminders"]] == ["the alarm of the VM"], kept

        # WITHIN A MINUTE AND A BIT: the watch thread's clock, armed for it.
        retry(lambda last_try: len(alarms()) == 1, timeout_seconds=120)
        alarm = alarms()[0]
        assert alarm["dismiss"] is True, alarm
        assert alarm["snooze"] == [5, 10, 60], alarm
        assert ["text", {"text": "the alarm of the VM"}] in alarm["args"], alarm["args"]

        # NOBODY ANSWERS FOR A MINUTE AND A HALF: it is still there.
        machine.sleep(90)
        assert len(alarms()) == 1, "the alarm left by itself"

        said = brain({"ask": "dismiss", "module": "reminders", "notice": "due"})
        assert said.get("say") not in ("refused", "declined"), f"the dismissal was refused: {said}"
        retry(lambda last_try: not alarms(), timeout_seconds=30)
        kept = json.loads(machine.succeed(f"cat {file}"))
        assert kept["reminders"] == [], f"a once-only reminder stayed after it was answered: {kept}"
        rang = [
            r for r in mind()["journal"]
            if r["task"] == "reminders" and r["says"] == "reminders-noted-rang"
        ]
        assert len(rang) == 1, f"the ring was not written down once: {rang}"
  '';
}
