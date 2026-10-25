#!/usr/bin/env python3
"""Turns `steps.nix` (beside this file) into the update sequence — the table,
the two runners and the text keys both languages have to carry.

One source, four products:

  (a) `updates.rs`  — the Rust table, self-contained: it declares its own enums
      and its own `Step`, so nothing hand-written can drift away from it. That
      is deliberately different from `gen-notifications.py`, whose table leans
      on types written by hand and therefore needs a check comparing the two
      vocabularies. Here there is no second copy to compare.
  (b) `keys.txt`    — the text keys the i18n check then demands, so "every step
      of the sequence exists in English and Portuguese" stops depending on
      discipline. They are DERIVED from the id — `<task>-step-<id>` and
      `<task>-step-<id>-does`, the task being the brain module's id, handed in
      as `--task` — which is why "a step with no text keys" is not a refusal
      here: it cannot be written. The table carries both keys beside the id,
      and the brain sends them on the wire, so no screen builds one.
  (c) two runners   — `ronoctua-update` (the person's half, and the CLI) and
      `ronoctua-update-system` (the root half). Generated rather than
      hand-written for the usual reason: a sequence written twice is a sequence
      that stops agreeing with the table nobody re-reads.
  (d) a refusal, at BUILD time, with the step's id in the message.

# Why (d) carries more than the usual weight here

Everywhere else in this repository a bad field means something is drawn wrong.
Here it means a command runs on somebody's machine. So the refusals are not only
about spelling: a step that would install something, a step that would reach a
shell, a step that names a bare binary and ships ON — those are refused because
the table is the only place where they can still be stopped for free.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import shlex
import stat
import sys

# ---------------------------------------------------------------------------
# THE VOCABULARIES
# ---------------------------------------------------------------------------

# The five classes a step can be, and they are load-bearing: see ORDER LAWS.
#
# `upgrade-software` is the PERSON'S programs — the Nix profile, Flatpaks,
# AppImages, the apps inside Android — moved to their newer versions. It is a
# class of its own and not `refresh-cache` because the brain's page draws the
# class beside the step, and "refreshes a cache" said of a new version of
# somebody's editor is a label that lies. It moves only what is THERE: every
# argument still goes through the install refusals below, with no exception.
DOES = {
    "move-inputs",
    "apply-configuration",
    "upgrade-software",
    "refresh-cache",
    "reclaim-space",
    # THE 2026-10-05 WAVE's classes (gather, migrate, reconcile-declared,
    # publish), after its anchor.
    # ── anchor: L19 ──
    #
    # `gather` brings in, as the person and BEFORE the root half, what the
    # person declared elsewhere: the plain part of the belongings, the pins
    # moved forward, the declarations staged for root's applier. `migrate`
    # turns an old machine directory (a copy of the distro) into the
    # machine's own flake, before anything moves its inputs. (The class that
    # tried every declared entry before the rebuild, `verify`, is gone: a
    # declared package is a part, which fails alone.) `reconcile-declared` is the person's half bringing what is DECLARED and
    # nothing else (the Android apps, the imperative copies a declared one
    # replaces). `publish` is the last thing: the machine's record and the
    # plain part pushed, so the person's other machines learn what this one
    # did.
    "gather",
    "migrate",
    "reconcile-declared",
    "publish",
    # THE 2026-10-23 WAVE's class (the parts), after its anchor.
    # ── anchor: L25 ──
    #
    # `realize-parts` brings the running system's plan of parts into the
    # records and roots their derivations, so the collection after it takes
    # none of them; the realizing itself is each part's own unit, started at
    # the switch, which the update never waits for (`nix/modules/parts.nix`).
    "realize-parts",
}

# THE ORDER THE CLASSES ARE DECLARED IN, for the Rust enum: the sequence's own
# order, so the table reads the way a run goes.
DOES_ORDER = [
    "gather",
    "migrate",
    "move-inputs",
    "apply-configuration",
    "realize-parts",
    "upgrade-software",
    "refresh-cache",
    "reclaim-space",
    "reconcile-declared",
    "publish",
]

# There are exactly two failure policies and no third one. "Ask" is not among
# them: this runs from a unit, and there is no polkit agent and no terminal to
# ask in.
ON_FAILURE = {"stop", "carry-on"}

# The preconditions. `always` is honest and not a hole: some steps genuinely
# cannot be inapplicable, and inventing a directory to test would be worse.
NEEDS = {"always", "binary", "directory", "writable-file", "network"}
# THE 2026-10-05 WAVE's preconditions (file, power, free-space), after its
# anchor.
# ── anchor: L19 ──
#
#   file        this file exists (`writable-file` asks more of it: that it can
#               be written; an old layout is recognised by a file it has)
#   power       on mains, or the battery at this many per cent or more — or
#               the person said yes to this run on battery (`--on-battery`,
#               the doors' question). ONE number for the whole table: the
#               doors ask below it, and two numbers would be two questions.
#   free-space  `{ at; percent; cap-gib; }`: the filesystem holding `at` has
#               that share of itself free, never asking for more than the
#               cap. Below it the store's garbage is collected FIRST, and only
#               then is the step refused. ROOT ONLY: only root may collect
#               the store's garbage, and a person's step short of room is not
#               something the store can give it.
#   not-held    the machine's record names no hold (`ronoctua-machine hold`):
#               the step that moves the distro forward leaves it alone.
NEEDS |= {"file", "power", "free-space", "not-held"}

# The fields of a `free-space` term, and nothing else.
FREE_SPACE_FIELDS = {"at", "percent", "cap-gib"}

# WHERE THE BRAIN LOOKS, and it is spelled in exactly two places for this
# sequence: here, and `ronoctua_brain_daemon::runner::REPORTS_LEAF`. This module's
# check `…-writes-where-the-brain-reads` reads the runner's constant and the
# script this writes, and refuses a disagreement — they disagreed once, and
# every run the brain started came back `runaway` on work that had succeeded.
REPORTS_LEAF = "ronoctua-brain"

# THE BRAIN'S ID FOR THIS SEQUENCE is not written here: it is the module's
# folder's name, handed in as `--task` by `sequence.nix` (`baseNameOf ./.`,
# invariant 16). The sheet's filename, the person's switches and every text
# key are built from it.

# WHERE THE PERSON'S OWN SWITCHES LIVE, written by `ronoctua-brain` out of
# `brain.json`. The module's `STEPS_FILE` names the leaf; its check
# `…-switch-reaches-the-run` runs the script's own reader against a file
# planted there, and refuses a disagreement.
#
# `${XDG_STATE_HOME:-$HOME/.local/state}` is expanded by the SHELL and not
# here: the root half has a different home from the person's, and this table is
# the person's. The root half never reads it — see `emit_runner`, which only
# emits this layer for the person's half.
PERSON_STEPS = "${XDG_STATE_HOME:-$HOME/.local/state}/ronoctua/update-steps.conf"

FIELDS = {
    "id",
    "does",
    "root",
    "package",
    "program",
    "unit",
    "rehearsal",
    "binary",
    "run",
    "needs",
    "on-failure",
    "enable",
    # THE REHEARSAL OF A PROGRAM: the same program, with these arguments,
    # changing nothing — what a rehearsal RUNS in the step's place, as
    # `rehearsal` is for a unit. Without it a rehearsal prints the command.
    "rehearse",
    # THE REFUSALS THAT ARE NEWS AND NOT FAILURES: the program's last line on
    # standard output is `refused <word>`, and a word listed here is recorded
    # `declined-<word>` and the sequence goes on. The machine's apply refuses
    # the system whose boot trial failed until the distro moves on: nothing
    # new to apply is not an update that broke.
    "declines",
}

# The wiring the module owns: paths and unit names live THERE, once, and arrive
# here. Writing them in this file too would be the second copy.
WIRING = {
    "shell",
    "systemctl",
    "nmcli",
    "flock",
    "journalctl",
    "state-dir",
    "config",
    "system-unit",
    "system-unit-rehearsal",
    # THE 2026-10-05 WAVE's preconditions, by store path and by place:
    # `systemd-ac-power`, the kernel's power supplies, `df`, the store's
    # garbage collector, `jq` and the machine's record (the hold), and how the
    # root half finds the person's yes to a run on battery (`id` and the
    # person's name, under the runtime directories' root).
    "ac-power",
    "power-supplies",
    "df",
    "collect-garbage",
    "jq",
    "machine-state",
    "id",
    "person",
    "runtime-root",
}

# WHERE THE PERSON'S YES TO A RUN ON BATTERY IS, under the person's runtime
# directory: the doors write it after the question, `--on-battery` writes it
# from a terminal, and only an executing run reads and removes it. The
# window's `ON_BATTERY` spells the same leaf, and the module's
# `…-names-agree` holds the two together.
ON_BATTERY = "ronoctua-update/on-battery"

# HOW LONG THE ROOT HALF WAITS FOR ITS OWN LOCK, in seconds — see where the
# runner takes it (`emit_runner`).
ROOT_LOCK_WAIT_S = 30

# English kebab, as invariant 18 asks of every id.
ID = re.compile(r"^[a-z][a-z0-9]*(-[a-z][a-z0-9]*)*$")

# A systemd unit name, and nothing that could be a path or a glob.
UNIT = re.compile(r"^[a-z][a-z0-9@.-]*\.(service|target|timer)$")

# A path a precondition may test. Absolute, or one of five XDG variables — and
# after the prefix, nothing that a shell could read as anything but a path.
PATH = re.compile(
    r"^(/|\$(HOME|XDG_DATA_HOME|XDG_CONFIG_HOME|XDG_STATE_HOME|XDG_CACHE_HOME)/)"
    r"[A-Za-z0-9._/-]*$"
)
VARIABLE_PATH = re.compile(r"^\$")

# WHAT NO STEP MAY DO.
#
# The substring and not the whole word, because `install-update`,
# `--install`, `reinstall` and `install!` are all the same act with a different
# spelling. The whole-word list below catches the ones that do not contain it.
INSTALL_SUBSTRING = "install"
INSTALL_VERBS = {
    "add",
    "clone",
    "create",
    "delete",
    "get",
    "init",
    "link",
    "new",
    "purge",
    "rm",
    "remove",
    "uninstall",
    "unlink",
}

# A shell would make every refusal above negotiable: one `;` and the argument
# list stops being the whole of what runs. There is no shell in the runners —
# the program is exec'd with its arguments — and these make that irreversible.
SHELL_METACHARACTERS = set(";|&$`()<>\n\r\t*?[]{}!~\\\"'")
NOT_A_PROGRAM = {
    "sh",
    "bash",
    "zsh",
    "dash",
    "env",
    "eval",
    "exec",
    "xargs",
    "sudo",
    "doas",
    "su",
    "runuser",
    "pkexec",
    "systemd-run",
    "nix-shell",
    "ssh",
}

problems: list[str] = []


def complain(where: str, what: str) -> None:
    problems.append(f"{where}: {what}")


def rust_str(s: str) -> str:
    """A Rust string literal. The table carries ids, names and text keys only."""
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def rust_variant(name: str) -> str:
    """`move-inputs` -> `MoveInputs`."""
    return "".join(p.capitalize() for p in name.split("-"))


def read_free_space(at: str, raw: object) -> tuple[str, int, int] | None:
    """A `free-space` term's `{ at; percent; cap-gib; }`, or `None` refused.

    A SHARE OF THE DISK AND A CAP, the shape `assets/cache.nix` gives every
    size it declares: a number of gigabytes measured on one laptop is wrong on
    the next, and a share alone asks a big disk for hundreds of gigabytes.
    The path is ABSOLUTE: only a root step may carry this, and root's
    variables are not the person's.
    """
    if not isinstance(raw, dict) or set(raw) != FREE_SPACE_FIELDS:
        complain(
            at,
            f"`free-space` is {{ {'; '.join(sorted(FREE_SPACE_FIELDS))}; }}, got {raw!r}",
        )
        return None
    place, percent, cap = raw["at"], raw["percent"], raw["cap-gib"]
    if not isinstance(place, str) or not place.startswith("/") or not PATH.match(place):
        complain(at, f"`free-space.at` is an absolute path, got {place!r}")
        return None
    if isinstance(percent, bool) or not isinstance(percent, int) or not 1 <= percent <= 50:
        complain(at, f"`free-space.percent` is a share from 1 to 50 per cent, got {percent!r}")
        return None
    if isinstance(cap, bool) or not isinstance(cap, int) or not 1 <= cap <= 4096:
        complain(at, f"`free-space.cap-gib` is a whole number of GiB from 1 to 4096, got {cap!r}")
        return None
    return (place, percent, cap)


def read_needs(where: str, raw: object) -> list[tuple[str, object]]:
    """The precondition list, one `(kind, value)` per term.

    An EMPTY list is refused. A step with no precondition is a step whose author
    did not ask themselves on which machine it does not apply, and the answer to
    that question is the difference between "skipped, and here is why" and a red
    line nobody can read.
    """
    if not isinstance(raw, list) or not raw:
        complain(
            where,
            "`needs` needs at least one pre-requisite. If nothing can make "
            "the step inapplicable, write `{ always = true; }` — and say so, "
            "instead of leaving the list empty.",
        )
        return []
    out: list[tuple[str, object]] = []
    for i, term in enumerate(raw):
        at = f"{where}.needs[{i}]"
        if not isinstance(term, dict) or len(term) != 1:
            complain(at, f"a pre-requisite is ONE `{{ kind = value; }}`, got {term!r}")
            continue
        kind, value = next(iter(term.items()))
        if kind not in NEEDS:
            complain(at, f"`{kind}` does not exist (there are: {', '.join(sorted(NEEDS))})")
            continue
        if kind in ("always", "network", "not-held"):
            if value is not True:
                complain(at, f"`{kind}` only takes `true`, got {value!r}")
                continue
        elif kind == "binary":
            if not isinstance(value, str) or not ID.match(value):
                complain(at, f"`binary` is a program name, got {value!r}")
                continue
        elif kind == "power":
            # A WHOLE PER CENT, and a share a battery can have: zero would be
            # a precondition nothing fails, and a hundred one a battery
            # rarely reports.
            if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= 99:
                complain(at, f"`power` is a battery share from 1 to 99 per cent, got {value!r}")
                continue
        elif kind == "free-space":
            value = read_free_space(at, value)
            if value is None:
                continue
        else:  # directory, writable-file, file
            if not isinstance(value, str) or not PATH.match(value):
                complain(
                    at,
                    f"`{kind} = {value!r}` is neither an absolute path nor does it start with "
                    "$HOME/, $XDG_DATA_HOME/, $XDG_CONFIG_HOME/, "
                    "$XDG_STATE_HOME/ ou $XDG_CACHE_HOME/",
                )
                continue
        out.append((kind, value))
    kinds = [k for k, _ in out]
    if "always" in kinds and len(kinds) > 1:
        complain(
            where,
            "`always` together with another pre-requisite is a contradiction: either nothing can make "
            "the step inapplicable, or something can.",
        )
    return out


def read_run(where: str, raw: object, program: str, field: str = "run") -> list[str]:
    """The arguments, and every reason one of them cannot be there.

    `field` names the list in the refusal: `run`, or `rehearse` — whose
    arguments go through every refusal `run`'s do, with no exception.
    """
    if not isinstance(raw, list):
        complain(where, f"`{field}` is a list of arguments, got {raw!r}")
        return []
    out: list[str] = []
    for i, token in enumerate(raw):
        at = f"{where}.{field}[{i}]"
        if not isinstance(token, str) or not token:
            complain(at, f"an argument has to be text, got {token!r}")
            continue
        bad = SHELL_METACHARACTERS & set(token)
        if bad:
            complain(
                at,
                f"the argument {token!r} has {''.join(sorted(bad))!r} in it, which a shell "
                "would read. The steps go through no shell at all, and an argument "
                "that only makes sense with one would be a step that never works.",
            )
            continue
        lowered = token.lower()
        if INSTALL_SUBSTRING in lowered:
            complain(
                at,
                f"the argument {token!r} installs. Updating what is already there is not "
                "imperative package management; putting in what is not, is — and it is "
                "exactly what invariant 6 refuses.",
            )
            continue
        if lowered in INSTALL_VERBS:
            complain(
                at,
                f"`{token}` puts something in or takes it out. This sequence only updates what "
                f"the machine already has (there are: {', '.join(sorted(INSTALL_VERBS))}).",
            )
            continue
        out.append(token)
    if program and program.lower() in NOT_A_PROGRAM:
        complain(
            where,
            f"`{program}` runs something else instead of doing something. A step has to "
            "be the program that updates, otherwise the argument list stops "
            "being everything that runs.",
        )
    return out


def read_step(index: int, raw: object, seen: set[str]) -> dict | None:
    """One step, whole, or `None` when it is too broken to be one."""
    where = f"steps[{index}]"
    if not isinstance(raw, dict):
        complain(where, f"a step is an attrset, got {raw!r}")
        return None

    ident = raw.get("id")
    if not isinstance(ident, str) or not ID.match(ident):
        complain(where, f"the id `{ident}` is not English kebab (invariant 18)")
        return None
    where = f"steps[{index}] ({ident})"
    if ident in seen:
        # Two steps with one id is one line in `/etc/ronoctua/updates.conf`
        # deciding for both, one pair of text keys describing both, and one row
        # in the report standing for whichever ran last.
        complain(where, f"the id `{ident}` already belongs to another step")
    seen.add(ident)

    unknown = set(raw) - FIELDS
    if unknown:
        complain(
            where,
            f"a field that does not exist: {', '.join(sorted(unknown))} "
            f"(ha: {', '.join(sorted(FIELDS))})",
        )

    does = raw.get("does")
    if does not in DOES:
        complain(where, f"`does = {does!r}` does not exist (there are: {', '.join(sorted(DOES))})")
        does = None

    root = raw.get("root")
    if not isinstance(root, bool):
        complain(where, f"`root` is a boolean, got {root!r}")
        root = None

    enable = raw.get("enable")
    if not isinstance(enable, bool):
        complain(where, f"`enable` is a boolean, got {enable!r}")
        enable = None

    on_failure = raw.get("on-failure")
    if on_failure not in ON_FAILURE:
        complain(
            where,
            f"`on-failure = {on_failure!r}` does not exist "
            f"(ha: {', '.join(sorted(ON_FAILURE))})",
        )
        on_failure = None

    needs = read_needs(where, raw.get("needs"))
    if root:
        for kind, value in needs:
            if isinstance(value, str) and VARIABLE_PATH.match(value):
                # Root's `$HOME` is `/root` and its `$XDG_*` are whatever
                # systemd left behind. A step testing one of those in the system
                # unit would be testing a home nobody lives in — and it would
                # look like it had tested the person's.
                complain(
                    where,
                    f"`{kind} = {value!r}` in a root step: the variables belong to the "
                    "PERSON, and root does not have them. Use an absolute path, or drop the "
                    "`root = true`.",
                )
    elif root is False and any(kind == "free-space" for kind, _ in needs):
        # ONLY ROOT COLLECTS THE STORE'S GARBAGE, which is what this
        # precondition does before it refuses — and a person's step short of
        # room is not something a collection made on its behalf can promise.
        complain(
            where,
            "`free-space` in a step of the person's: it collects the store's garbage "
            "before refusing, and only root may. It belongs to a root step.",
        )

    # ---- WHAT IT RUNS: exactly one of three shapes ------------------------
    shapes = [k for k in ("package", "unit", "binary") if k in raw]
    kind = shapes[0] if len(shapes) == 1 else None
    if len(shapes) != 1:
        complain(
            where,
            "a step runs ONE thing: `package` + `program` (a store path), "
            "`unit` (a unit that already exists) or `binary` (a name on the "
            f"PATH). Got: {shapes or 'none'}.",
        )

    package = program = unit = binary = rehearsal = None
    run: list[str] = []
    rehearse: list[str] | None = None
    declines: list[str] = []

    if "rehearsal" in raw and kind != "unit":
        # A REHEARSAL UNIT FOR A PROGRAM would be a second command for the
        # same step, and nothing would keep the two meaning the same thing.
        complain(where, "`rehearsal` goes with `unit` only: it is the unit a rehearsal starts in its place")
    if ("rehearse" in raw or "declines" in raw) and kind == "unit":
        # A UNIT'S REHEARSAL IS A UNIT (`rehearsal`), and what a unit said
        # is its status, not a line a runner can read back.
        complain(
            where,
            "`rehearse` and `declines` go with a program (`package` or `binary`), "
            "not with `unit`: a unit rehearses through `rehearsal`",
        )

    if kind == "package":
        package, program = raw.get("package"), raw.get("program")
        if not isinstance(package, str) or not ID.match(package.replace("_", "-")):
            complain(where, f"`package` is an attribute of nixpkgs, got {package!r}")
        if not isinstance(program, str) or not ID.match(program.replace("_", "-")):
            complain(where, f"`program` is the file in bin/, got {program!r}")
            program = None
        run = read_run(where, raw.get("run", []), program or "")
    elif kind == "unit":
        unit = raw.get("unit")
        if not isinstance(unit, str) or not UNIT.match(unit):
            complain(where, f"`unit` is not a systemd unit name: {unit!r}")
        if raw.get("run"):
            complain(
                where,
                "a unit takes no argument: what it runs is declared "
                "in it, and an argument here would be ignored in silence.",
            )
        if root is False:
            # Starting a unit as the person goes through polkit, and there is no
            # agent in this session: it would be a refusal, not a prompt.
            complain(where, "only root starts a unit from here — there is no polkit agent in the session")
        if "program" in raw:
            complain(where, "`program` does not go with `unit`")
        rehearsal = raw.get("rehearsal")
        if rehearsal is not None:
            if not isinstance(rehearsal, str) or not UNIT.match(rehearsal):
                complain(where, f"`rehearsal` is not a systemd unit name: {rehearsal!r}")
                rehearsal = None
            elif rehearsal == unit:
                # THE SAME UNIT would run the step FOR REAL inside a rehearsal,
                # which is the one thing a rehearsal promises not to do.
                complain(
                    where,
                    f"`rehearsal` is `{unit}` itself: a rehearsal would run the step for real",
                )
                rehearsal = None
    elif kind == "binary":
        binary = raw.get("binary")
        if not isinstance(binary, str) or not ID.match(binary):
            complain(where, f"`binary` is a program name, got {binary!r}")
            binary = None
        run = read_run(where, raw.get("run", []), binary or "")
        if "program" in raw:
            complain(where, "`program` does not go with `binary`")

        # THE THREE REFUSALS THAT KEEP INVARIANT 6 WHOLE.
        if root:
            complain(
                where,
                "`binary` with `root = true`: a unit's PATH is minimal, and a "
                "name there fails far from the cause. Root runs a store path.",
            )
        if enable:
            complain(
                where,
                "`binary` with `enable = true`: what the flake does not put on the machine "
                "cannot be a distro default (invariant 6). Declare it switched off; "
                "whoever has the tool switches it on in one line.",
            )
        if binary and ("binary", binary) not in needs:
            complain(
                where,
                f"`binary = {binary!r}` without `{{ binary = \"{binary}\"; }}` in "
                "`needs`: without it the machine that does not have the tool gets a "
                "fails, and the right answer is a skipped step saying why.",
            )

    # ---- WHAT A REHEARSAL RUNS, AND WHAT A REFUSAL MEANS ------------------
    if kind in ("package", "binary"):
        name = (program if kind == "package" else binary) or ""
        if "rehearse" in raw:
            given = raw.get("rehearse")
            rehearse = read_run(where, given, name, "rehearse")
            if isinstance(given, list) and not given:
                # AN EMPTY ONE is the program with no argument at all — which
                # for most of these is the act itself.
                complain(where, "`rehearse` is empty: a rehearsal has to say how it changes nothing")
            elif rehearse == run:
                # THE SAME ARGUMENTS would run the step for real inside a
                # rehearsal, which is the one thing a rehearsal promises not to.
                complain(where, "`rehearse` is `run` itself: a rehearsal would run the step for real")
        if "declines" in raw:
            words = raw.get("declines")
            if (
                not isinstance(words, list)
                or not words
                or any(not isinstance(w, str) or not ID.match(w) for w in words)
                or len(set(words)) != len(words)
            ):
                complain(
                    where,
                    f"`declines` is a list of distinct kebab-case refusal words, got {words!r}",
                )
            else:
                declines = list(words)

    if None in (does, root, enable, on_failure) or kind is None:
        return None

    return {
        "id": ident,
        "does": does,
        "root": root,
        "enable": enable,
        "on-failure": on_failure,
        "needs": needs,
        "kind": kind,
        "package": package,
        "program": program,
        "unit": unit,
        "rehearsal": rehearsal,
        "binary": binary,
        "run": run,
        "rehearse": rehearse,
        "declines": declines,
    }


def check_wave_laws(steps: list[dict], moves: list[int], applies: list[int]) -> None:
    """The order laws of the 2026-10-05 wave: gather → the root half → the
    person's half, publish last, and one battery floor.

    Each refusal names the law in words a reader can grep for; the module's
    `…-laws-refuse` check builds a table that breaks each one and demands its
    sentence.
    """
    roots = [i for i, s in enumerate(steps) if s["root"]]
    first_root = roots[0] if roots else len(steps)

    def at(i: int) -> str:
        return f"steps[{i}] ({steps[i]['id']})"

    for i, s in enumerate(steps):
        does = s["does"]
        # GATHER: the person's, and before the root half — what root is about
        # to read has to be there when it starts.
        if does == "gather":
            if s["root"]:
                complain(at(i), "`gather` runs as the person: it brings in what the person declared, and root has no session to fetch it with")
            elif i > first_root:
                complain(at(i), "`gather` after the root half: what it brings would reach the machine one update late")
        elif i < first_root and not s["root"]:
            # NOTHING BUT THE GATHERING before the root half: a person's step
            # there would run against the system the root half is about to
            # replace.
            complain(at(i), f"`{does}` before the root half: only `gather` comes before it")
        # MIGRATE: root's, and before anything moves the inputs — the old
        # layout's lock is not the machine flake's.
        if does == "migrate":
            if not s["root"]:
                complain(at(i), "`migrate` is root's: it rewrites the machine directory")
            if moves and i > moves[0]:
                complain(at(i), "`migrate` after `move-inputs`: the inputs would move in a layout about to be replaced")
        # RECONCILE-DECLARED: the person's, after the rebuild — it brings what
        # the new system declares, and nothing it does not.
        if does == "reconcile-declared":
            if s["root"]:
                complain(at(i), "`reconcile-declared` is the person's: it touches only the person's own ledger")
            if applies and i < applies[0]:
                complain(at(i), "`reconcile-declared` before `apply-configuration`: it would bring what the old system declared")

    # PUBLISH LAST: the person's, and nothing after it — what is pushed is
    # what this run ended with.
    publishes = [i for i, s in enumerate(steps) if s["does"] == "publish"]
    for i in publishes:
        if steps[i]["root"]:
            complain(at(i), "`publish` is the person's: it pushes their repository")
    if publishes:
        for i in range(publishes[0], len(steps)):
            if steps[i]["does"] != "publish":
                complain(at(i), "a step after `publish`: publish is last, so what is pushed is what the run ended with")

    # ONE BATTERY FLOOR: the doors ask below it, and two would be two questions.
    floors = sorted({v for s in steps for k, v in s["needs"] if k == "power"})
    if len(floors) > 1:
        complain("steps", f"`power` names {floors}: one battery floor for the whole sequence, or the doors cannot say which they ask about")

    # THE LOCK MOVES ONLY WHEN THE REBUILD CAN BE TRIED: every battery floor
    # and every room on the disk the rebuild asks for, the step that moves the
    # inputs asks for too. Otherwise a run on a low battery, or on a disk the
    # collection could not clear, moves the lock and then skips the rebuild —
    # and the machine directory names a distro the running system was never
    # built from, which the next apply builds whole.
    if moves and applies:
        mover, applier = steps[moves[0]], steps[applies[0]]
        for kind, value in applier["needs"]:
            if kind in ("power", "free-space") and (kind, value) not in mover["needs"]:
                complain(
                    at(moves[0]),
                    f"`move-inputs` without the rebuild's `{kind}`: on a run whose rebuild that "
                    "skips, the lock would move and the machine directory would stop describing "
                    "the running system",
                )


def check_parts_laws(steps: list[dict], applies: list[int]) -> None:
    """The parts' law (2026-10-23): one step plans them, as root, right after
    the system it plans for is applied and before any space is reclaimed, and
    a part never stops an update.
    """
    plans = [i for i, s in enumerate(steps) if s["does"] == "realize-parts"]

    def at(i: int) -> str:
        return f"steps[{i}] ({steps[i]['id']})"

    if len(plans) != 1:
        complain("steps", f"there are {len(plans)} `realize-parts` steps and there has to be exactly 1: the plan is the system's, and one step reads it")
    for i in plans:
        s = steps[i]
        if not s["root"]:
            complain(at(i), "`realize-parts` is root's: it writes root's records and the collector's roots")
        if applies and i < applies[0]:
            complain(at(i), "`realize-parts` before `apply-configuration`: it would plan the parts of the system the rebuild replaces")
        if s["on-failure"] != "carry-on":
            complain(at(i), "`realize-parts` stops the update: a part never holds back the system or anything after it")
        for j, t in enumerate(steps):
            if t["does"] == "reclaim-space" and j < i:
                complain(at(j), "`reclaim-space` before `realize-parts`: the collection would take the derivations the parts are about to be built from")


def check_sequence(steps: list[dict]) -> None:
    """The laws that are about the SEQUENCE and not about any one step."""
    # ---- ORDER LAWS ------------------------------------------------------
    moves = [i for i, s in enumerate(steps) if s["does"] == "move-inputs"]
    applies = [i for i, s in enumerate(steps) if s["does"] == "apply-configuration"]
    for name, found in (("move-inputs", moves), ("apply-configuration", applies)):
        if len(found) != 1:
            complain(
                "steps",
                f"there are {len(found)} `{name}` steps and there has to be exactly 1. "
                "Two would be two truths about what the machine is.",
            )
    if len(moves) == 1 and len(applies) == 1:
        if moves[0] > applies[0]:
            complain(
                "steps",
                "`move-inputs` after `apply-configuration`: the new inputs "
                "only reach the machine on the next rebuild, and the sequence would say "
                "that updated.",
            )
        for i, s in enumerate(steps):
            if s["does"] == "reclaim-space" and i < applies[0]:
                complain(
                    f"steps[{i}] ({s['id']})",
                    "`reclaim-space` before `apply-configuration`: the sweep "
                    "would erase what the rebuild is about to use.",
                )
            # THE PERSON'S PROGRAMS MOVE AGAINST THE NEW SYSTEM. `nix profile
            # upgrade` resolves `nixpkgs` through the registry the rebuild
            # pins, and `flatpak update` should be the Flatpak the rebuild just
            # installed — before it, both would answer with the old ones.
            if s["does"] == "upgrade-software" and i < applies[0]:
                complain(
                    f"steps[{i}] ({s['id']})",
                    "`upgrade-software` before `apply-configuration`: it would "
                    "resolve against the system the rebuild is about to replace.",
                )

    # THE 2026-10-05 WAVE's laws (gather → root half → person half, publish
    # last), after its anchor.
    # ── anchor: L19 ──
    check_wave_laws(steps, moves, applies)
    # ── anchor: L25 ──
    check_parts_laws(steps, applies)

    # ---- ROOT AFTER THE GATHERING, AND NEVER INTERLEAVED ------------------
    #
    # Two managers run this sequence. Interleaving costs a round trip through
    # polkit at every switch, and a sequence whose order changes when somebody
    # flips a `root =` is a sequence nobody can predict. The person's
    # GATHERING comes before the root half and is not a person's step in
    # this sense: it is what the root half is about to read
    # (`check_wave_laws`).
    saw_person = None
    for i, s in enumerate(steps):
        if not s["root"] and s["does"] != "gather":
            saw_person = s["id"]
        # A GATHERING AFTER THE PERSON'S STEPS is not a root step, and
        # `check_wave_laws` names what is wrong with it.
        elif s["root"] and saw_person is not None:
            complain(
                f"steps[{i}] ({s['id']})",
                f"a root step after `{saw_person}`, which is not. The privileged "
                "half runs whole before the person's half; "
                "interleaving would be one trip to polkit per swap.",
            )
            break


# ---------------------------------------------------------------------------
# (a) THE RUST TABLE
# ---------------------------------------------------------------------------


def rust_needs(needs: list[tuple[str, object]]) -> str:
    parts = []
    for kind, value in needs:
        if kind in ("always", "network", "not-held"):
            parts.append(f"Needs::{rust_variant(kind)}")
        elif kind == "power":
            parts.append(f"Needs::Power({value})")
        elif kind == "free-space":
            place, percent, cap = value  # type: ignore[misc]
            parts.append(
                f"Needs::FreeSpace {{ at: {rust_str(place)}, percent: {percent}, cap_gib: {cap} }}"
            )
        else:
            parts.append(f"Needs::{rust_variant(kind)}({rust_str(str(value))})")
    return "&[" + ", ".join(parts) + "]"


def battery_floor(steps: list[dict]) -> int | None:
    """The one battery floor the table declares (`check_wave_laws` keeps it
    one), or `None` when no step needs power."""
    floors = sorted({v for s in steps for k, v in s["needs"] if k == "power"})
    return floors[0] if floors else None


def declined_words(steps: list[dict]) -> list[str]:
    """Every refusal some step reads as news, once each, sorted."""
    return sorted({w for s in steps for w in s["declines"]})


def step_keys(task: str, ident: str) -> tuple[str, str]:
    """A step's two text keys: its name, and the line saying what it does."""
    return f"{task}-step-{ident}", f"{task}-step-{ident}-does"


def emit_rust(steps: list[dict], task: str) -> str:
    """The table, whole and self-contained.

    THE COMMAND CROSSES AS BASENAMES AND NOT AS STORE PATHS, on purpose. The
    view never runs anything — it starts a unit — so the path would buy nothing,
    and it would cost twice: `AGENTS.md` records that a store path baked into
    the Rust is a reference whoever COMPILES pays for, and the person reading
    the row wants to see `nix flake update`, not 52 characters of hash first.
    """
    # THE ENUM'S ORDER IS A LIST AND THE VOCABULARY A SET: the two have to be
    # the same words, or a class would be accepted and never declared.
    if set(DOES_ORDER) != DOES or len(DOES_ORDER) != len(DOES):
        raise SystemExit(f"ERROR: DOES_ORDER {DOES_ORDER} is not the vocabulary {sorted(DOES)}")
    rows = []
    for order, s in enumerate(steps):
        if s["kind"] == "unit":
            command = ["systemctl", "start", s["unit"]]
        else:
            command = [s["program"] or s["binary"] or ""] + s["run"]
        rows.append(
            "    Step {\n"
            f"        id: {rust_str(s['id'])},\n"
            f"        order: {order},\n"
            f"        root: {'true' if s['root'] else 'false'},\n"
            f"        does: Does::{rust_variant(s['does'])},\n"
            f"        enabled: {'true' if s['enable'] else 'false'},\n"
            f"        on_failure: OnFailure::{rust_variant(s['on-failure'])},\n"
            f"        needs: {rust_needs(s['needs'])},\n"
            "        command: &["
            + ", ".join(rust_str(c) for c in command)
            + "],\n"
            f"        name_key: {rust_str(step_keys(task, s['id'])[0])},\n"
            f"        does_key: {rust_str(step_keys(task, s['id'])[1])},\n"
            "    },"
        )

    def enum(name: str, values: list[str], doc: str) -> list[str]:
        out = [
            f"/// {doc}",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            f"pub enum {name} {{",
        ]
        # EVERY VARIANT SAYS ITS WORD: the table is included into a crate
        # that asks every public item for a doc, and it is the word the
        # declaration spells.
        for v in values:
            out += [f"    /// `{v}`.", f"    {rust_variant(v)},"]
        out += [
            "}",
            "",
            f"impl {name} {{",
            "    /// The declared name, so it can go out and come back.",
            "    #[must_use]",
            "    pub fn id(self) -> &'static str {",
            "        match self {",
        ]
        out += [f"            Self::{rust_variant(v)} => {rust_str(v)}," for v in values]
        out += ["        }", "    }", "}", ""]
        return out

    out = [
        "// GENERATED by the update module's gen-updates.py. DO NOT EDIT BY HAND.",
        "//",
        "// The sequence lives in the module's steps.nix — once, there.",
        "// Editing here means losing the change at the next build.",
        "//",
        "// It declares its own types: there is no hand-written half to drift",
        "// away from, and therefore no vocabulary for a check to compare.",
        "",
    ]
    out += enum(
        "Does",
        DOES_ORDER,
        "What class of thing a step is. It decides the order laws, not the view.",
    )
    out += enum(
        "OnFailure",
        ["stop", "carry-on"],
        "What the sequence does when a step fails. There is no third answer.",
    )
    out += [
        "/// What has to be true for a step to apply. All of them, or it is skipped.",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
        "pub enum Needs {",
        "    /// Nothing can make this step inapplicable.",
        "    Always,",
        "    /// This name is on the person's own PATH.",
        "    Binary(&'static str),",
        "    /// This directory exists.",
        "    Directory(&'static str),",
        "    /// This file exists and can be written.",
        "    WritableFile(&'static str),",
        "    /// NetworkManager says the machine is connected.",
        "    Network,",
        "    /// This file exists.",
        "    File(&'static str),",
        "    /// On mains, or the battery at this many per cent or more, or the",
        "    /// person said yes to this run on battery.",
        "    Power(u8),",
        "    /// The filesystem holding `at` has this share of itself free, never",
        "    /// asking more than `cap_gib` — after the store's garbage is collected.",
        "    FreeSpace {",
        "        /// A path on the filesystem measured.",
        "        at: &'static str,",
        "        /// The share of the filesystem, in per cent.",
        "        percent: u8,",
        "        /// The most it asks for, in GiB.",
        "        cap_gib: u16,",
        "    },",
        "    /// The machine's record names no hold.",
        "    NotHeld,",
        "}",
        "",
        "impl Needs {",
        "    /// The text key that says WHICH precondition was not met.",
        "    #[must_use]",
        "    pub fn key(self) -> &'static str {",
        "        match self {",
        f'            Self::Always => "{task}-needs-always",',
        f'            Self::Binary(_) => "{task}-needs-binary",',
        f'            Self::Directory(_) => "{task}-needs-directory",',
        f'            Self::WritableFile(_) => "{task}-needs-writable-file",',
        f'            Self::Network => "{task}-needs-network",',
        f'            Self::File(_) => "{task}-needs-file",',
        f'            Self::Power(_) => "{task}-needs-power",',
        f'            Self::FreeSpace {{ .. }} => "{task}-needs-free-space",',
        f'            Self::NotHeld => "{task}-needs-not-held",',
        "        }",
        "    }",
        "",
        "    /// The same key, by the WORD a runner writes into its report",
        "    /// (`skipped-<word>`). `None` for a word that is not one of the",
        "    /// closed set: a silent fallback would draw a reason that is not the",
        "    /// one the runner gave.",
        "    #[must_use]",
        "    pub fn key_of_word(word: &str) -> Option<&'static str> {",
        "        match word {",
        *[f'            {rust_str(k)} => Some({rust_str(task + "-needs-" + k)}),' for k in sorted(NEEDS)],
        "            _ => None,",
        "        }",
        "    }",
        "",
        "    /// The path or the name the precondition is about, when it has one.",
        "    #[must_use]",
        "    pub fn about(self) -> Option<&'static str> {",
        "        match self {",
        "            Self::Binary(s) | Self::Directory(s) | Self::WritableFile(s) | Self::File(s) => {",
        "                Some(s)",
        "            }",
        "            Self::FreeSpace { at, .. } => Some(at),",
        "            Self::Always | Self::Network | Self::Power(_) | Self::NotHeld => None,",
        "        }",
        "    }",
        "}",
        "",
        "/// THE ONE BATTERY FLOOR the sequence declares, in per cent: below it, on",
        "/// battery, the steps that compile are skipped unless the person said yes",
        "/// to this run — and a door asks before starting one by hand. `None` when",
        "/// no step needs power.",
        "pub const BATTERY_FLOOR_PERCENT: Option<u8> = "
        + (f"Some({battery_floor(steps)})" if battery_floor(steps) is not None else "None")
        + ";",
        "",
        "/// The text key of a refusal a step reads as news and not as a failure",
        "/// (`declined-<word>` in its report), or `None` for a word no step",
        "/// declares: a silent fallback would draw a reason the program never gave.",
        "#[must_use]",
        "pub fn declined_key(word: &str) -> Option<&'static str> {",
        *(
            [
                "    match word {",
                *[
                    f'        {rust_str(w)} => Some({rust_str(task + "-declined-" + w)}),'
                    for w in declined_words(steps)
                ],
                "        _ => None,",
                "    }",
            ]
            # NO WORD AT ALL is a table no step declines in: a `match` with
            # only its fallback is what clippy refuses, and this says the same.
            if declined_words(steps)
            else ["    let _ = word;", "    None"]
        ),
        "}",
        "",
        "/// One step of the sequence.",
        "///",
        "/// Everything is `&'static`: the table is data in the binary, and the view",
        "/// that draws it allocates nothing to do so.",
        "#[derive(Debug, Clone, Copy)]",
        "pub struct Step {",
        "    /// Kebab-case, English. The id everywhere: text keys, the `/etc` line, the report.",
        "    pub id: &'static str,",
        "    /// Its place in the sequence, from zero. The list's own order.",
        "    pub order: u8,",
        "    /// Whether it runs in the system unit or in the person's.",
        "    pub root: bool,",
        "    /// What class of thing it is.",
        "    pub does: Does,",
        "    /// Whether the distro ships it on. `/etc/ronoctua/updates.conf` overrules it.",
        "    pub enabled: bool,",
        "    /// What the sequence does when it fails.",
        "    pub on_failure: OnFailure,",
        "    /// All of these have to hold.",
        "    pub needs: &'static [Needs],",
        "    /// What it runs, by BASENAME — for the screen, never to be executed from here.",
        "    pub command: &'static [&'static str],",
        "    /// Text key: the step's name.",
        "    pub name_key: &'static str,",
        "    /// Text key: the one line saying what it does.",
        "    pub does_key: &'static str,",
        "}",
        "",
        "/// The distro's update sequence, IN DECLARED ORDER.",
        "pub const STEPS: &[Step] = &[",
        *rows,
        "];",
        "",
    ]
    return "\n".join(out)


# ---------------------------------------------------------------------------
# (b) THE TEXT KEYS
# ---------------------------------------------------------------------------


def emit_keys(steps: list[dict], task: str) -> str:
    """Every key both languages owe.

    Per step: its name and the one line saying what it does. Plus the closed
    vocabularies the table itself defines — a skipped step has to be able to say
    WHICH precondition failed, and that sentence belongs to the KIND.
    """
    keys = set()
    for s in steps:
        keys.update(step_keys(task, s["id"]))
    for kind in NEEDS:
        keys.add(f"{task}-needs-{kind}")
    for policy in ON_FAILURE:
        keys.add(f"{task}-on-failure-{policy}")
    for does in DOES:
        keys.add(f"{task}-kind-{does}")
    for word in declined_words(steps):
        keys.add(f"{task}-declined-{word}")
    return "\n".join(sorted(keys)) + "\n"


# ---------------------------------------------------------------------------
# (c) THE TWO RUNNERS
# ---------------------------------------------------------------------------
#
# The verdicts the report carries. English, closed, and the same words on both
# halves: the view reads one file per half and has to understand both.
#
#   off  skipped-<kind>  rehearsed  ran  failed  stopped


def shell_path(value: str) -> str:
    """A path inside double quotes, so `$HOME` still expands.

    `shlex.quote` would put it in single quotes and the variable would arrive
    verbatim. What makes this safe is not the quoting: it is `PATH`, which has
    already refused everything a shell could read as more than a path.
    """
    return '"' + value + '"'


def emit_step(s: dict, wiring: dict[str, str], programs: dict[str, str]) -> list[str]:
    """One shell function per step, so the sequence reads as a list of names."""
    name = "step_" + s["id"].replace("-", "_")
    out = [
        f"# ---- {s['id']} " + "-" * max(0, 60 - len(s["id"])),
        f"{name}() {{",
        f"  local id={shlex.quote(s['id'])}",
        '  if ! turned_on "$id"; then',
        '    say "-- $id: switched off"',
        '    record "$id" off 0',
        "    return 0",
        "  fi",
    ]
    for kind, value in s["needs"]:
        if kind == "always":
            continue
        if kind == "binary":
            out += [
                f"  if ! command -v {shlex.quote(str(value))} >/dev/null 2>&1; then",
                f'    skip "$id" binary {shlex.quote(str(value))}',
                "    return 0",
                "  fi",
            ]
        elif kind == "directory":
            out += [
                f"  if [ ! -d {shell_path(str(value))} ]; then",
                f'    skip "$id" directory {shell_path(str(value))}',
                "    return 0",
                "  fi",
            ]
        elif kind == "writable-file":
            out += [
                f"  if [ ! -f {shell_path(str(value))} ] "
                f"|| [ ! -w {shell_path(str(value))} ]; then",
                f'    skip "$id" writable-file {shell_path(str(value))}',
                "    return 0",
                "  fi",
            ]
        elif kind == "network":
            out += [
                "  if ! connected; then",
                '    skip "$id" network ""',
                "    return 0",
                "  fi",
            ]
        elif kind == "file":
            out += [
                f"  if [ ! -f {shell_path(str(value))} ]; then",
                f'    skip "$id" file {shell_path(str(value))}',
                "    return 0",
                "  fi",
            ]
        elif kind == "power":
            out += [
                f"  if ! powered {int(value)}; then",  # type: ignore[arg-type]
                f'    skip "$id" power "{int(value)}%"',  # type: ignore[arg-type]
                "    return 0",
                "  fi",
            ]
        elif kind == "free-space":
            place, percent, cap = value  # type: ignore[misc]
            out += [
                f"  if ! roomy {shlex.quote(place)} {percent} {cap}; then",
                f'    skip "$id" free-space {shlex.quote(place)}',
                "    return 0",
                "  fi",
            ]
        elif kind == "not-held":
            out += [
                "  if ! unheld; then",
                '    skip "$id" not-held ""',
                "    return 0",
                "  fi",
            ]

    def program_argv(arguments: list[str]) -> list[str]:
        if s["kind"] == "package":
            return [f"{programs[s['package']]}/bin/{s['program']}"] + arguments
        return [s["binary"] or ""] + arguments

    if s["kind"] == "unit":
        argv = [wiring["systemctl"], "start", s["unit"] or ""]
    else:
        argv = program_argv(s["run"])
    quoted = " ".join(shlex.quote(a) for a in argv)

    out += [f'  say ">> $id: {quoted}"']
    if s.get("rehearsal"):
        # A UNIT WITH A REHEARSAL OF ITS OWN IS REHEARSED FOR REAL: the
        # rehearsal starts that unit — the same program changing nothing —
        # and says what it said, here, where the sequence's own output is
        # read. `systemctl start` on a oneshot returns when it is done, and
        # its exit is the rehearsal's: a step that would fail fails its
        # rehearsal too.
        #
        # WHAT IT PRINTED IS READ FROM WHERE THE JOURNAL ENDED BEFORE THE
        # START, and never through the unit. A oneshot that ended cleanly and
        # that nothing references is unloaded at once: `systemctl show` then
        # answers for a unit loaded afresh, with no invocation id, and a relay
        # keyed on that id relays nothing on every rehearsal that worked. The
        # cursor taken before the start does not depend on the unit staying
        # loaded, and `_SYSTEMD_UNIT` after it is this run's lines and no
        # other's: only this half starts the rehearsal, under its lock. The
        # `--sync` in between is what makes the lines be there: the unit's
        # output reaches journald through a stream it may not have drained
        # yet when `systemctl start` returns, and the sync waits until
        # everything written before it was.
        systemctl = shlex.quote(wiring["systemctl"])
        journalctl = shlex.quote(wiring["journalctl"])
        rehearsal = shlex.quote(s["rehearsal"])
        its_lines = shlex.quote("_SYSTEMD_UNIT=" + s["rehearsal"])
        out += [
            '  if [ "$execute" != true ]; then',
            f'    say ">> $id, rehearsed: {systemctl} start {rehearsal}"',
            "    local since=$SECONDS mark ok=0",
            f"    mark=$({journalctl} --no-pager --quiet --lines=0 --show-cursor 2>/dev/null) || mark=",
            '    case "$mark" in',
            '      "-- cursor: "?*) mark=${mark#-- cursor: } ;;',
            "      *) mark= ;;",
            "    esac",
            f"    {systemctl} start {rehearsal} || ok=1",
            '    if [ -n "$mark" ]; then',
            f"      {journalctl} --sync 2>/dev/null || true",
            f'      {journalctl} --no-pager --output=cat --after-cursor="$mark" {its_lines} 2>/dev/null || true',
            "    else",
            '      say "!! $id: the journal gave no place to read its rehearsal from"',
            "    fi",
            '    if [ "$ok" = 0 ]; then',
            '      record "$id" rehearsed $(( SECONDS - since ))',
            "      return 0",
            "    fi",
            '    record "$id" failed $(( SECONDS - since ))',
            '    say "!! $id: its rehearsal failed"',
            "    return 1",
            "  fi",
        ]
    elif s.get("rehearse") is not None:
        # A PROGRAM WITH A REHEARSAL OF ITS OWN IS REHEARSED FOR REAL, the
        # way a unit with one is: the same program, with the arguments that
        # change nothing (`apply --rehearse` measures on a copy), its output in
        # the sequence's own, its exit the rehearsal's.
        rehearsed = " ".join(shlex.quote(a) for a in program_argv(s["rehearse"]))
        out += [
            '  if [ "$execute" != true ]; then',
            f'    say ">> $id, rehearsed: {rehearsed}"',
            *[
                "  " + line
                for line in run_and_record(rehearsed, "rehearsed", "its rehearsal failed", s["declines"])
            ],
            "  fi",
        ]
    else:
        out += [
            '  if [ "$execute" != true ]; then',
            '    record "$id" rehearsed 0',
            "    return 0",
            "  fi",
        ]
    out += [
        *run_and_record(quoted, "ran", "failed", s["declines"]),
        "}",
        "",
    ]
    return out


def run_and_record(quoted: str, worked: str, said_failed: str, declines: list[str]) -> list[str]:
    """Runs one command line and records how it ended — `worked`, `failed`,
    or, for a refusal the step reads as news, `declined-<word>`.

    WITH NO REFUSAL TO READ the command runs as it always did, its output
    straight to the journal. WITH ONE, its standard output is read back so
    its LAST LINE can be asked — `refused <word>` is how the machine's and
    the person's programs end a refusal — and printed whole afterwards: the
    progress they write goes to standard error, which still streams.

    AND A REFUSAL THAT IS NOT NEWS KEEPS ITS WORD: `failed-<word>`, so the
    view says WHY the program refused (the machine's own sentence for it)
    instead of only that it failed. A last line that is not a refusal, or a
    word that is not kebab, is plain `failed` — never a word made up.
    """
    if not declines:
        return [
            "  local began=$SECONDS",
            f"  if {quoted}; then",
            f'    record "$id" {worked} $(( SECONDS - began ))',
            "    return 0",
            "  fi",
            '  record "$id" failed $(( SECONDS - began ))',
            f'  say "!! $id: {said_failed}"',
            "  return 1",
        ]
    arms = []
    for word in declines:
        arms += [
            f"    {shlex.quote('refused ' + word)})",
            f'      record "$id" declined-{word} $(( SECONDS - began ))',
            f'      say "-- $id: declined ({word})"',
            "      return 0",
            "      ;;",
        ]
    return [
        "  local began=$SECONDS said code=0 last why=",
        f"  said=$({quoted}) || code=$?",
        '  if [ -n "$said" ]; then',
        "    printf '%s\\n' \"$said\"",
        "  fi",
        '  if [ "$code" = 0 ]; then',
        f'    record "$id" {worked} $(( SECONDS - began ))',
        "    return 0",
        "  fi",
        "  last=${said##*$'\\n'}",
        '  case "$last" in',
        *arms,
        "    'refused '*) why=${last#refused } ;;",
        "  esac",
        '  case "$why" in',
        "    '' | *[!a-z0-9-]* | -*) why=failed ;;",
        "    *) why=failed-$why ;;",
        "  esac",
        '  record "$id" "$why" $(( SECONDS - began ))',
        f'  say "!! $id: {said_failed}"',
        "  return 1",
    ]


def emit_helpers(kinds: set[str], *, root_half: bool, wiring: dict[str, str]) -> list[str]:
    """The shell functions the 2026-10-05 preconditions call — only those a
    step of THIS half needs: a function nothing calls is code shellcheck
    rightly reads as a mistake.
    """
    q = shlex.quote
    out: list[str] = []
    if "power" in kinds:
        if root_half:
            # ROOT HAS NO SESSION, AND STILL KNOWS WHOSE YES COUNTS: the one
            # person the polkit rule lets start this half (`person` in the
            # wiring). Their yes is in their own runtime directory, which only
            # they and root can write; it is TESTED, never read — it can only
            # make a declared step run on battery, never change what runs.
            yes = [
                "  local uid",
                f"  if [ \"$execute\" = true ] && uid=$({q(wiring['id'])} -u {q(wiring['person'])} 2>/dev/null) \\",
                f"    && [ -e {q(wiring['runtime-root'])}/\"$uid\"/{q(ON_BATTERY)} ]; then",
            ]
        else:
            yes = ['  if [ "$execute" = true ] && [ -e "$consent" ]; then']
        out += [
            "# MAINS, OR ENOUGH BATTERY, OR THE PERSON'S YES.",
            "#",
            "# `systemd-ac-power` answers yes on mains AND on a machine with no",
            "# battery at all. Otherwise the batteries that feed the machine (not a",
            "# mouse's: `scope=Device`) are averaged against the floor. The yes is",
            "# the person's, for this run only — the doors ask with the no first.",
            "powered() {",
            f"  if {q(wiring['ac-power'])}; then",
            "    return 0",
            "  fi",
            '  if battery_at_least "$1"; then',
            "    return 0",
            "  fi",
            *yes,
            '    say "   on battery below $1%, and the person said yes to this run"',
            "    return 0",
            "  fi",
            "  return 1",
            "}",
            "",
            "battery_at_least() {",
            "  local supply share sum=0 count=0",
            f"  for supply in {q(wiring['power-supplies'])}/*; do",
            '    [ -r "$supply/type" ] || continue',
            '    [ "$(< "$supply/type")" = Battery ] || continue',
            '    if [ -r "$supply/scope" ] && [ "$(< "$supply/scope")" = Device ]; then',
            "      continue",
            "    fi",
            '    [ -r "$supply/capacity" ] || continue',
            '    share=$(< "$supply/capacity")',
            '    case "$share" in',
            "      '' | *[!0-9]*) continue ;;",
            "    esac",
            "    sum=$(( sum + share ))",
            "    count=$(( count + 1 ))",
            "  done",
            '  [ "$count" -gt 0 ] && [ $(( sum / count )) -ge "$1" ]',
            "}",
            "",
        ]
    if "free-space" in kinds:
        out += [
            "# ROOM ON THE FILESYSTEM HOLDING $1: $2 per cent of it free, never more",
            "# than $3 GiB. 0 there is, 1 there is not, 2 it could not be measured.",
            "room() {",
            "  local size avail wanted",
            f"  read -r size avail < <({q(wiring['df'])} -P -m \"$1\" 2>/dev/null | {{",
            "    read -r _",
            "    read -r _ size _ avail _ && printf '%s %s\\n' \"$size\" \"$avail\"",
            "  }) || return 2",
            '  case "$size$avail" in',
            "    '' | *[!0-9]*) return 2 ;;",
            "  esac",
            '  wanted=$(( size * $2 / 100 ))',
            '  if [ "$wanted" -gt $(( $3 * 1024 )) ]; then',
            '    wanted=$(( $3 * 1024 ))',
            "  fi",
            '  [ "$avail" -ge "$wanted" ]',
            "}",
            "",
            "# AND WHEN THERE IS NOT, THE STORE'S GARBAGE FIRST — garbage only, never a",
            "# generation — and only then the refusal. A rehearsal says it would.",
            "roomy() {",
            "  local why=0",
            '  room "$@" || why=$?',
            '  if [ "$why" = 0 ]; then',
            "    return 0",
            "  fi",
            '  if [ "$why" = 2 ]; then',
            '    say "   the free space under $1 could not be measured"',
            "    return 1",
            "  fi",
            '  if [ "$execute" != true ]; then',
            '    say "   short of room under $1: a run would collect the store\'s garbage first"',
            "    return 1",
            "  fi",
            '  say "   short of room under $1: collecting the store\'s garbage first"',
            f"  {q(wiring['collect-garbage'])} || true",
            '  room "$@"',
            "}",
            "",
        ]
    if "not-held" in kinds:
        state = q(wiring["machine-state"])
        out += [
            "# THE MACHINE IS NOT HELD AT A REVISION (`ronoctua-machine hold`). No",
            "# record is a machine never held; a record that does not read keeps the",
            "# inputs where they are, and says so.",
            "unheld() {",
            "  local said=0",
            f"  if [ ! -e {state} ]; then",
            "    return 0",
            "  fi",
            f"  {q(wiring['jq'])} -e '.hold == null' {state} >/dev/null 2>&1 || said=$?",
            '  case "$said" in',
            "    0) return 0 ;;",
            "    1) return 1 ;;",
            "  esac",
            '  say "   the machine\'s record does not read: the inputs stay where they are"',
            "  return 1",
            "}",
            "",
        ]
    return out


def emit_runner(
    steps: list[dict],
    *,
    root_half: bool,
    wiring: dict[str, str],
    programs: dict[str, str],
    task: str,
) -> str:
    mine = [s for s in steps if s["root"] == root_half]
    defaults_on = [s["id"] for s in steps if s["enable"]]
    floor = battery_floor(steps)
    # THE PERSON'S YES TO A RUN ON BATTERY is taken by the person's half —
    # the one a person starts — whenever some step of either half needs power.
    asks_yes = floor is not None and not root_half
    kinds = {k for s in mine for k, _ in s["needs"]}

    if root_half:
        report = f"{wiring['state-dir']}/system.report"
        lock = f"{wiring['state-dir']}/system.lock"
        who = "the privileged half"
    else:
        report = '"$runtime"/user.report'
        lock = '"$runtime"/user.lock'
        who = "the sequence"

    out = [
        f"#!{wiring['shell']}",
        "# GENERATED by the update module's gen-updates.py. DO NOT EDIT BY HAND.",
        "#",
        "# The sequence lives in the module's steps.nix — once, there.",
        "",
        "set -uo pipefail",
        "",
        "# NOTHING HAPPENS WITHOUT `--execute`, which is the installer's contract",
        "# (`nix run .#install`). Rehearsing prints the program and its arguments,",
        "# resolved, in order, after testing every precondition for real — so it",
        "# does show which steps this machine would skip and which it would run.",
        "# It does NOT show anything that only appears once a command runs: the",
        "# installer lost three defects to exactly that gap, and they are written",
        "# down in docs/PENDING.md.",
        "execute=false",
        *(["on_battery=false"] if asks_yes else []),
        # A COMMAND ON `PATH` THAT CANNOT EXPLAIN ITSELF IS A COMMAND ONE READS
        # THE SOURCE OF. There are exactly two things to say — it rehearses by
        # default, and `--execute` is what makes it act — and saying them costs
        # eight lines.
        "usage() {",
        (
            "  printf 'usage: %s [--execute [--on-battery]]\\n\\n' \"$(basename \"$0\")\""
            if asks_yes
            else "  printf 'usage: %s [--execute]\\n\\n' \"$(basename \"$0\")\""
        ),
        "  printf 'With no --execute it REHEARSES: it evaluates every precondition for real and\\n'",
        "  printf 'prints what would run, without running anything. With --execute, it runs.\\n\\n'",
        *(
            [
                f"  printf 'On battery below {floor}%% the steps that compile are skipped; --on-battery\\n'",
                "  printf 'is your yes to running them anyway, for this run only.\\n\\n'",
            ]
            if asks_yes
            else []
        ),
        "  printf 'The rehearsal does not see what only shows up when a command runs — the\\n'",
        "  printf 'installer has already paid three defects for that; see docs/PENDING.md.\\n'",
        "}",
        'for a in "$@"; do',
        '  case "$a" in',
        "    --execute) execute=true ;;",
        *(["    --on-battery) on_battery=true ;;"] if asks_yes else []),
        "    -h|--help) usage; exit 0 ;;",
        "    *)",
        "      printf 'unknown argument: %s\\n\\n' \"$a\" >&2",
        "      usage >&2",
        "      exit 2",
        "      ;;",
        "  esac",
        "done",
        "",
    ]

    if root_half:
        out += [
            "# `systemd.tmpfiles` already makes it, and this answers the machine",
            "# where the unit ran before tmpfiles did.",
            f"mkdir -p {shlex.quote(wiring['state-dir'])}",
            "",
        ]
    else:
        out += [
            "# The person's half runs in the person's session, which is the only",
            "# place a tool they installed by hand can be found. These five are",
            "# what a `systemd --user` unit does not necessarily inherit.",
            ': "${XDG_RUNTIME_DIR:=/tmp}"',
            ': "${XDG_DATA_HOME:=$HOME/.local/share}"',
            ': "${XDG_CONFIG_HOME:=$HOME/.config}"',
            ': "${XDG_STATE_HOME:=$HOME/.local/state}"',
            ': "${XDG_CACHE_HOME:=$HOME/.cache}"',
            'runtime="$XDG_RUNTIME_DIR/ronoctua-update"',
            'mkdir -p "$runtime"',
            "",
        ]

    out += [
        "# ONE AT A TIME. Two `nix flake update` on one `/etc/nixos` is a lock",
        "# file two processes are writing, and the footer's door and the",
        "# terminal's door are the same sequence.",
        "#",
        "# TAKING THE LOCK AND FAILING TO TAKE IT ARE TWO DIFFERENT ANSWERS, and",
        "# they used to be one. `exec 9>` on a directory that does not exist",
        "# leaves fd 9 unopened, `flock` then says `Bad file descriptor`, and the",
        "# script reported ALREADY RUNNING — a person reading that would go",
        "# looking for a run that was never there. Found by rehearsing the root",
        "# half outside a NixOS install, where `/run` is not writable.",
        f"if ! exec 9>{lock}; then",
        f"  printf 'could not create the lock at %s\\n' {lock} >&2",
        "  printf 'does the directory exist? what runs this is the unit, as root.\\n' >&2",
        "  exit 1",
        "fi",
        # THE ROOT HALF WAITS A MOMENT FOR ITS OWN LOCK, and the person's half
        # does not wait at all. The root half starts once root's lock of the
        # person's data is its (`ronoctua-person hold --patient`), and an
        # apply that held both lets that one go when its program ends and this
        # one when its `flock` wrapper does, a moment later: not waiting would
        # call that moment "already running".
        f"if ! {shlex.quote(wiring['flock'])} "
        + (f"-w {ROOT_LOCK_WAIT_S}" if root_half else "-n")
        + " 9; then",
        f"  printf '{who} is already running\\n' >&2",
        "  exit 1",
        "fi",
        "",
        f"report={report}",
        ': > "$report" 2>/dev/null || report=/dev/null',
        "",
        "say() { printf '%s\\n' \"$*\"; }",
        "",
        "# The report the view reads: one line per step, tab separated,",
        "# `<id> <verdict> <seconds>`. Ids and verdicts are English and closed;",
        "# the sentence on screen comes from a text key, never from here.",
        "record() { printf '%s\\t%s\\t%s\\n' \"$1\" \"$2\" \"$3\" >> \"$report\""
        + ("; }" if root_half else "; sheet; }"),
        "",
    ]

    # THE SECOND READER OF THE SAME LINES, and the reason it exists: the brain
    # watches a JSON file and NOTHING ON THE MACHINE WROTE ONE. It watched
    # `/run/ronoctua-brain/system-update.json`, the script wrote a TSV
    # somewhere else, `Report::watch` never saw the mtime move, and every
    # update the brain started came back `runaway` after two hours — on a
    # rebuild that had worked. It is derived from the TSV rather than written
    # beside it so the two can never disagree about what happened.
    if not root_half:
        out += [
            "# WHAT THE BRAIN WATCHES, rebuilt whole after every step.",
            "#",
            "# `ronoctua_brain::task::Sheet`: `began` is set once, `ended` stays",
            "# ZERO while the run is going (a moment in 1970 is not a run that",
            "# ended), and `steps` is this file's own TSV turned into records.",
            "# The brain re-reads it only when the mtime moves, and a read that",
            "# lands mid-write parses as nothing and is simply tried again.",
            "#",
            "# `ok` is false for `failed` (and `failed-<word>`, a refusal that kept",
            "# its word) and `stopped`, and true for everything",
            "# else: a step that is off or skipped did not fail, and marking it",
            "# so would report the whole update as broken because a machine has",
            "# no `rustup`. The words are closed and the ids are kebab-case, so",
            "# nothing here needs escaping.",
            "#",
            "# NO PER-STEP PROSE, because the TSV carries none: what a step said",
            "# goes to the journal. The brain names the step it stopped at,",
            "# which `ended()` argues is more than \"something went wrong\".",
            f'brain="$XDG_RUNTIME_DIR/{REPORTS_LEAF}"',
            'mkdir -p "$brain" 2>/dev/null || true',
            'if [ "$execute" = true ]; then',
            f'  sheet_file="$brain/{task}.json"',
            "else",
            f'  sheet_file="$brain/{task}-rehearsal.json"',
            "fi",
            # `$EPOCHSECONDS` and not `date`: the runner's PATH is the wiring\'s
            # store paths and nothing else, and a bare `date` would fail far
            # from its cause. Bash 5 has carried this since 2019.
            "began=$EPOCHSECONDS",
            "sheet_ended=0",
            "sheet() {",
            "  local id verdict seconds ok first=true",
            '  { printf \'{"began":%s,"ended":%s,"trouble":"","steps":[\' \\',
            '      "$began" "$sheet_ended"',
            '    while read -r id verdict seconds; do',
            '      case "$verdict" in failed|failed-*|stopped) ok=false ;; *) ok=true ;; esac',
            '      if [ "$first" = true ]; then first=false; else printf \',\'; fi',
            '      printf \'{"id":"%s","ok":%s,"seconds":%s,"trouble":""}\' \\',
            '        "$id" "$ok" "${seconds:-0}"',
            '    done < "$report"',
            "    printf ']}'",
            '  } > "$sheet_file".new 2>/dev/null || return 0',
            "  # ATOMIC, so the brain never reads half a file it could have read",
            "  # whole. `Report::sheet` tolerates a torn read, but a rename costs",
            "  # nothing and removes the retry.",
            '  mv -f "$sheet_file".new "$sheet_file" 2>/dev/null || true',
            "}",
            "# STAMPING THE END is a separate verb from writing the sheet,",
            "# because every step writes the sheet and only the way out ends",
            "# the run.",
            "finish() { sheet_ended=$EPOCHSECONDS; sheet; }",
            "sheet",
            "",
        ]

    if asks_yes:
        out += [
            "# THE PERSON'S YES TO THIS RUN ON BATTERY, which the doors write after",
            "# asking with the no first, and `--on-battery` writes from a terminal.",
            "# Only a run that ACTS honours it and takes it away, whichever way it",
            "# ends: a yes is for the run it was given to, never for the next one,",
            "# and a scheduled run never has one — it waits for the charger instead.",
            f'consent="$XDG_RUNTIME_DIR/{ON_BATTERY}"',
            'if [ "$execute" = true ]; then',
            '  if [ "$on_battery" = true ]; then',
            '    : > "$consent"',
            "  fi",
            "  trap 'rm -f \"$consent\"' EXIT",
            "fi",
            "",
        ]
    out += emit_helpers(kinds, root_half=root_half, wiring=wiring)
    out += [
        "skip() {",
        '  if [ -n "$3" ]; then',
        '    say "-- $1: skipped ($2: $3)"',
        "  else",
        '    say "-- $1: skipped ($2)"',
        "  fi",
        '  record "$1" "skipped-$2" 0',
        "}",
        "",
        "# NetworkManager is what this distro asks about the network",
        "# (`nix/modules/network.nix`), so it is what gets asked here.",
        "connected() {",
        f"  [ \"$({shlex.quote(wiring['nmcli'])} -t -f STATE general 2>/dev/null)\" "
        "= connected ]",
        "}",
        "",
        "# WHICH STEPS ARE ON, from up to three sources, most specific first.",
        "#",
        "#   1. the person's own switch, from the brain's page (`m t`);",
        "#   2. this machine's table, from `ronoctua.update.steps`;",
        "#   3. the distro's default, baked in below.",
        "#",
        "# LAYER 1 IS THE PERSON'S HALF ONLY, and that is a boundary and not an",
        "# oversight. This table lives in the person's state directory; the root",
        "# half runs as root, whose home is `/root`, and there is no honest way",
        "# for it to find out whose session started it. The ways to give it one",
        "# all end in a root-owned directory that a member of `users` may write",
        "# — which would let any account on a shared machine decide what a root",
        "# update runs. The six root steps take `ronoctua.update.steps`, and the",
        "# brain's page DRAWS them refusing and says so, which is the same answer",
        "# the Syncthing view already gives for the fourteen options the flake",
        "# fixes (invariant 6).",
        "#",
        "# The person's layer is a set of FLIPS and not a set of states: it says",
        "# which steps differ from the answer the other two give, so a step",
        "# added by a later version takes its declared default instead of being",
        "# missing from everybody's file. That is the same shape the brain keeps",
        "# it in, and it is what makes `m t` mean the same thing on a machine",
        "# whose `/etc` table was changed afterwards.",
        "#",
        "# Invariant 5 is intact: nothing here decides anything. The numbers and",
        "# the defaults live in Nix, the flips live in the person's own file, and",
        "# this reads both.",
        "#",
        "# WITHOUT LAYER 1 THE SWITCH ON THE SCREEN DID NOTHING. It wrote to",
        "# `brain.json` and nothing on the machine read it — the page offered a",
        "# key, the key wrote a file, and the sequence went on running what it",
        "# ran before, with nothing to tell anybody.",
        "turned_on() {",
        # The two extra locals belong to the person's flip, which the root
        # half does not read — and shellcheck refuses a local nothing uses.
        "  local want=$1 id state on=1" + ("" if root_half else " task step"),
        f"  if [ -r {shlex.quote(wiring['config'])} ]; then",
        "    while read -r id state; do",
        '      case "$id" in "#"*|"") continue ;; esac',
        '      if [ "$id" = "$want" ]; then',
        '        [ "$state" = on ] && on=0 || on=1',
        "        break",
        "      fi",
        f"    done < {shlex.quote(wiring['config'])}",
        "  else",
        "    default_on \"$want\" && on=0 || on=1",
        "  fi",
        *(
            []
            if root_half
            else [
                "  # AND THE PERSON'S FLIP, last, because it is the most specific.",
                f'  if [ -r "{PERSON_STEPS}" ]; then',
                "    while read -r task step; do",
                '      case "$task" in "#"*|"") continue ;; esac',
                f'      if [ "$task" = {shlex.quote(task)} ] && [ "$step" = "$want" ]; then',
                '        [ "$on" = 0 ] && on=1 || on=0',
                "        break",
                "      fi",
                f'    done < "{PERSON_STEPS}"',
                "  fi",
            ]
        ),
        '  return "$on"',
        "}",
        "",
        "# The distro's answer, used when this machine has no table of its own.",
        "default_on() {",
        '  case "$1" in',
        "    " + ("|".join(shlex.quote(i) for i in defaults_on) if defaults_on else "'')")
        + ") return 0 ;;",
        "  esac",
        "  return 1",
        "}",
        "",
    ]

    for s in mine:
        out += emit_step(s, wiring, programs)

    # WHAT THIS MACHINE DID IS PUBLISHED ON EVERY WAY OUT of the person's
    # half, and not only at the end of a run that went well. A rebuild that
    # failed is something this machine did: its record says so (`last-apply`),
    # and the person's other machines learn it from the plain part only if it
    # is pushed. So a run that stops — the root half refused, a step of the
    # person's that stops — still runs the `publish` steps before it ends.
    publishing = [] if root_half else [s for s in mine if s["does"] == "publish"]
    if publishing:
        out += [
            "# THE PUBLISHING, on a way out before the end: the same steps the",
            "# sequence ends with, each `carry-on`.",
            "publish_anyway() {",
            *[f"  step_{s['id'].replace('-', '_')} || true" for s in publishing],
            "}",
            "",
        ]

    out += ["# ---- the sequence " + "-" * 52, ""]

    def call(s: dict) -> list[str]:
        name = "step_" + s["id"].replace("-", "_")
        if s["on-failure"] == "stop":
            return [
                f"if ! {name}; then",
                f"  say \"== sequence stopped at {s['id']}\"",
                "  record sequence stopped 0",
                *(["  publish_anyway"] if publishing and s["does"] != "publish" else []),
                *([] if root_half else ["  finish"]),
                "  exit 1",
                "fi",
            ]
        return [f"{name} || true"]

    # THE GATHERING FIRST, as the person: what the root half is about to
    # read — the declarations of the person's other machines, the pins moved
    # forward, the staged copy root's applier takes — has to be there when it
    # starts.
    gathering = [s for s in mine if s["does"] == "gather"]
    mine = [s for s in mine if s["does"] != "gather"]
    for s in gathering:
        out += call(s)
    if gathering:
        out += [""]

    if not root_half:
        out += [
            "# THE ROOT HALF NEXT, through the one unit polkit names. `systemctl",
            "# start` on a `Type=oneshot` returns when the work is done, which is",
            "# what makes this a sequence and not two things happening at once.",
            'if [ "$execute" = true ]; then',
            f"  privileged={shlex.quote(wiring['system-unit'])}",
            "else",
            f"  privileged={shlex.quote(wiring['system-unit-rehearsal'])}",
            "fi",
            'say "== $privileged"',
            f"if ! {shlex.quote(wiring['systemctl'])} start \"$privileged\"; then",
            '  say "!! the privileged half stopped; see: journalctl -u $privileged"',
            "  record system-half failed 0",
            *(["  publish_anyway"] if publishing else []),
            "  finish",
            "  exit 1",
            "fi",
            "",
        ]

    if not mine and not gathering:
        out += ['say "no step in this half"', ""]
    for s in mine:
        out += call(s)
    out += [
        "",
        'say "== end"',
        # THE LAST THING, AND ON EVERY WAY OUT. `ended` staying zero is how the
        # sheet says "still going", so a run that ends without stamping it is a
        # run the brain watches until its patience is gone and then calls
        # `runaway`. That is the exact defect this whole file half exists to
        # close, and leaving one `exit` without it would reopen it for that
        # path only — which is the kind of hole nobody finds twice.
        *([] if root_half else ["finish"]),
        "exit 0",
        "",
    ]
    return "\n".join(out)


# ---------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--updates", type=pathlib.Path, required=True)
    ap.add_argument(
        "--task",
        required=True,
        help="the brain module's id: its text keys, its sheet and its switches are built from it",
    )
    ap.add_argument(
        "--emit",
        choices=("table", "scripts"),
        required=True,
        help="the Rust table and the keys, or the two scripts",
    )
    ap.add_argument("--out-rust", type=pathlib.Path)
    ap.add_argument("--out-keys", type=pathlib.Path)
    ap.add_argument("--out-dir", type=pathlib.Path)
    ap.add_argument(
        "--programs",
        type=pathlib.Path,
        help="JSON: nixpkgs attribute -> store path, resolved by Nix",
    )
    ap.add_argument(
        "--wiring",
        type=pathlib.Path,
        help="JSON: paths and unit names, from the module's module.nix",
    )
    args = ap.parse_args()

    if not ID.match(args.task):
        print(f"ERROR: --task `{args.task}` is not an English kebab id", file=sys.stderr)
        return 1
    source = json.loads(args.updates.read_text(encoding="utf-8"))
    raw = source.get("steps")
    if not isinstance(raw, list) or not raw:
        print("ERROR: steps.nix has no `steps`", file=sys.stderr)
        return 1

    seen: set[str] = set()
    read = (read_step(i, s, seen) for i, s in enumerate(raw))
    steps = [s for s in read if s is not None]
    if len(steps) == len(raw):
        check_sequence(steps)

    programs: dict[str, str] = {}
    wiring: dict[str, str] = {}
    if args.emit == "scripts":
        if not (args.programs and args.wiring and args.out_dir):
            print("ERROR: --emit scripts needs --programs, --wiring and --out-dir", file=sys.stderr)
            return 1
        programs = json.loads(args.programs.read_text(encoding="utf-8"))
        wiring = json.loads(args.wiring.read_text(encoding="utf-8"))
        missing = WIRING - set(wiring)
        if missing:
            complain("wiring", f"missing: {', '.join(sorted(missing))}")
        wanted = {s["package"] for s in steps if s["kind"] == "package"}
        for p in sorted(wanted - set(programs)):
            complain(
                "programs",
                f"no step can run: `{p}` was not resolved by Nix. "
                "Add it to the module's module.nix.",
            )
        # AND THE OTHER WAY ROUND, which is the half that rots in silence: a
        # package left here after its step went away is a dependency nobody
        # needs, carried in the closure of a unit that runs as root.
        for p in sorted(set(programs) - wanted):
            complain("programs", f"`{p}` is resolved and no step uses it")

    if problems:
        print("ERROR: steps.nix does not pass:", file=sys.stderr)
        for p in problems:
            print(f"       {p}", file=sys.stderr)
        print("", file=sys.stderr)
        print("       The table is the only source of the update sequence.", file=sys.stderr)
        print("       A wrong field here would become a command running on the", file=sys.stderr)
        print("       somebody's machine, and not a crooked design.", file=sys.stderr)
        return 1

    if args.emit == "table":
        if not (args.out_rust and args.out_keys):
            print("ERROR: --emit table needs --out-rust and --out-keys", file=sys.stderr)
            return 1
        args.out_rust.write_text(emit_rust(steps, args.task), encoding="utf-8")
        args.out_keys.write_text(emit_keys(steps, args.task), encoding="utf-8")
        keys = emit_keys(steps, args.task).split()
        print(
            f"update: {len(steps)} steps "
            f"({sum(1 for s in steps if s['enable'])} switched on), "
            f"{len(keys)} text keys required"
        )
        return 0

    args.out_dir.mkdir(parents=True, exist_ok=True)
    for filename, root_half in (
        ("ronoctua-update", False),
        ("ronoctua-update-system", True),
    ):
        path = args.out_dir / filename
        path.write_text(
            emit_runner(steps, root_half=root_half, wiring=wiring, programs=programs, task=args.task),
            encoding="utf-8",
        )
        path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    person = sum(1 for s in steps if not s["root"])
    print(
        f"update: {len(steps) - person} root steps, {person} of the person's, "
        "two scripts"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
