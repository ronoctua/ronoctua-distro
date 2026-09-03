#!/usr/bin/env python3
"""Turns `assets/updates.nix` into the update sequence — the table, the two
runners and the text keys both languages have to carry.

One source, four products:

  (a) `updates.rs`  — the Rust table, self-contained: it declares its own enums
      and its own `Step`, so nothing hand-written can drift away from it. That
      is deliberately different from `gen-notifications.py`, whose table leans
      on types written by hand and therefore needs a check comparing the two
      vocabularies. Here there is no second copy to compare.
  (b) `keys.txt`    — the text keys the i18n check then demands, so "every step
      of the sequence exists in English and Portuguese" stops depending on
      discipline. They are DERIVED from the id, which is why "a step with no
      text keys" is not a refusal here: it cannot be written.
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

# The four classes a step can be, and they are load-bearing: see ORDER LAWS.
DOES = {"move-inputs", "apply-configuration", "refresh-cache", "reclaim-space"}

# There are exactly two failure policies and no third one. "Ask" is not among
# them: this runs from a unit, and there is no polkit agent and no terminal to
# ask in.
ON_FAILURE = {"stop", "carry-on"}

# The preconditions. `always` is honest and not a hole: some steps genuinely
# cannot be inapplicable, and inventing a directory to test would be worse.
NEEDS = {"always", "binary", "directory", "writable-file", "network"}

FIELDS = {
    "id",
    "does",
    "root",
    "package",
    "program",
    "unit",
    "binary",
    "run",
    "needs",
    "on-failure",
    "enable",
}

# The wiring the module owns: paths and unit names live THERE, once, and arrive
# here. Writing them in this file too would be the second copy.
WIRING = {
    "shell",
    "systemctl",
    "nmcli",
    "flock",
    "state-dir",
    "config",
    "system-unit",
    "system-unit-rehearsal",
}

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
        if kind in ("always", "network"):
            if value is not True:
                complain(at, f"`{kind}` so aceita `true`, veio {value!r}")
                continue
        elif kind == "binary":
            if not isinstance(value, str) or not ID.match(value):
                complain(at, f"`binary` is a program name, got {value!r}")
                continue
        else:  # directory, writable-file
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


def read_run(where: str, raw: object, program: str) -> list[str]:
    """The arguments, and every reason one of them cannot be there."""
    if not isinstance(raw, list):
        complain(where, f"`run` is a list of arguments, got {raw!r}")
        return []
    out: list[str] = []
    for i, token in enumerate(raw):
        at = f"{where}.run[{i}]"
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
        complain(where, f"`root` e booleano, veio {root!r}")
        root = None

    enable = raw.get("enable")
    if not isinstance(enable, bool):
        complain(where, f"`enable` e booleano, veio {enable!r}")
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

    package = program = unit = binary = None
    run: list[str] = []

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
        "binary": binary,
        "run": run,
    }


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

    # ---- ROOT FIRST, AND NEVER INTERLEAVED -------------------------------
    #
    # Two managers run this sequence. Interleaving costs a round trip through
    # polkit at every switch, and a sequence whose order changes when somebody
    # flips a `root =` is a sequence nobody can predict.
    saw_person = None
    for i, s in enumerate(steps):
        if not s["root"]:
            saw_person = s["id"]
        elif saw_person is not None:
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
        if kind in ("always", "network"):
            parts.append(f"Needs::{rust_variant(kind)}")
        else:
            parts.append(f"Needs::{rust_variant(kind)}({rust_str(str(value))})")
    return "&[" + ", ".join(parts) + "]"


def emit_rust(steps: list[dict]) -> str:
    """The table, whole and self-contained.

    THE COMMAND CROSSES AS BASENAMES AND NOT AS STORE PATHS, on purpose. The
    view never runs anything — it starts a unit — so the path would buy nothing,
    and it would cost twice: `CLAUDE.md` records that a store path baked into
    the Rust is a reference whoever COMPILES pays for, and the person reading
    the row wants to see `nix flake update`, not 52 characters of hash first.
    """
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
            f"        name: {rust_str('update-' + s['id'])},\n"
            f"        explains: {rust_str('update-does-' + s['id'])},\n"
            "    },"
        )

    def enum(name: str, values: list[str], doc: str) -> list[str]:
        out = [
            f"/// {doc}",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]",
            f"pub enum {name} {{",
        ]
        out += [f"    {rust_variant(v)}," for v in values]
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
        "// GENERATED by nix/scripts/gen-updates.py. DO NOT EDIT BY HAND.",
        "//",
        "// The sequence lives in assets/updates.nix — once, there.",
        "// Editing here means losing the change at the next build.",
        "//",
        "// It declares its own types: there is no hand-written half to drift",
        "// away from, and therefore no vocabulary for a check to compare.",
        "",
    ]
    out += enum(
        "Does",
        ["move-inputs", "apply-configuration", "refresh-cache", "reclaim-space"],
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
        "}",
        "",
        "impl Needs {",
        "    /// The text key that says WHICH precondition was not met.",
        "    #[must_use]",
        "    pub fn key(self) -> &'static str {",
        "        match self {",
        '            Self::Always => "update-needs-always",',
        '            Self::Binary(_) => "update-needs-binary",',
        '            Self::Directory(_) => "update-needs-directory",',
        '            Self::WritableFile(_) => "update-needs-writable-file",',
        '            Self::Network => "update-needs-network",',
        "        }",
        "    }",
        "",
        "    /// The path or the name the precondition is about, when it has one.",
        "    #[must_use]",
        "    pub fn about(self) -> Option<&'static str> {",
        "        match self {",
        "            Self::Binary(s) | Self::Directory(s) | Self::WritableFile(s) => Some(s),",
        "            Self::Always | Self::Network => None,",
        "        }",
        "    }",
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
        "    pub name: &'static str,",
        "    /// Text key: the one line saying what it does.",
        "    pub explains: &'static str,",
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


def emit_keys(steps: list[dict]) -> str:
    """Every key both languages owe.

    Per step: its name and the one line saying what it does. Plus the closed
    vocabularies the table itself defines — a skipped step has to be able to say
    WHICH precondition failed, and that sentence belongs to the KIND.
    """
    keys = set()
    for s in steps:
        keys.add(f"update-{s['id']}")
        keys.add(f"update-does-{s['id']}")
    for kind in NEEDS:
        keys.add(f"update-needs-{kind}")
    for policy in ON_FAILURE:
        keys.add(f"update-on-failure-{policy}")
    for does in DOES:
        keys.add(f"update-kind-{does}")
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
        '    say "-- $id: desligado"',
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

    if s["kind"] == "unit":
        argv = [wiring["systemctl"], "start", s["unit"] or ""]
    elif s["kind"] == "package":
        argv = [f"{programs[s['package']]}/bin/{s['program']}"] + s["run"]
    else:
        argv = [s["binary"] or ""] + s["run"]
    quoted = " ".join(shlex.quote(a) for a in argv)

    out += [
        f'  say ">> $id: {quoted}"',
        '  if [ "$execute" != true ]; then',
        '    record "$id" rehearsed 0',
        "    return 0",
        "  fi",
        "  local began=$SECONDS",
        f"  if {quoted}; then",
        '    record "$id" ran $(( SECONDS - began ))',
        "    return 0",
        "  fi",
        '  record "$id" failed $(( SECONDS - began ))',
        '  say "!! $id: failed"',
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
) -> str:
    mine = [s for s in steps if s["root"] == root_half]
    defaults_on = [s["id"] for s in steps if s["enable"]]

    if root_half:
        report = f"{wiring['state-dir']}/system.report"
        lock = f"{wiring['state-dir']}/system.lock"
        who = "a metade privilegiada"
    else:
        report = '"$runtime"/user.report'
        lock = '"$runtime"/user.lock'
        who = "the sequence"

    out = [
        f"#!{wiring['shell']}",
        "# GENERATED by nix/scripts/gen-updates.py. DO NOT EDIT BY HAND.",
        "#",
        "# The sequence lives in assets/updates.nix — once, there.",
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
        # A COMMAND ON `PATH` THAT CANNOT EXPLAIN ITSELF IS A COMMAND ONE READS
        # THE SOURCE OF. There are exactly two things to say — it rehearses by
        # default, and `--execute` is what makes it act — and saying them costs
        # eight lines.
        "usage() {",
        "  printf 'usage: %s [--execute]\\n\\n' \"$(basename \"$0\")\"",
        "  printf 'With no --execute it REHEARSES: it evaluates every precondition for real and\\n'",
        "  printf 'prints what would run, without running anything. With --execute, it runs.\\n\\n'",
        "  printf 'The rehearsal does not see what only shows up when a command runs — the\\n'",
        "  printf 'installer has already paid three defects for that; see docs/PENDING.md.\\n'",
        "}",
        'for a in "$@"; do',
        '  case "$a" in',
        "    --execute) execute=true ;;",
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
        f"if ! {shlex.quote(wiring['flock'])} -n 9; then",
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
        "record() { printf '%s\\t%s\\t%s\\n' \"$1\" \"$2\" \"$3\" >> \"$report\"; }",
        "",
        "skip() {",
        '  if [ -n "$3" ]; then',
        '    say "-- $1: pulado ($2: $3)"',
        "  else",
        '    say "-- $1: pulado ($2)"',
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
        "# WHICH STEPS ARE ON. The machine's table wins and the distro's default",
        "# answers when there is none — invariant 5: the numbers live in Nix, and",
        "# this only reads them.",
        "turned_on() {",
        "  local want=$1 id state",
        f"  if [ -r {shlex.quote(wiring['config'])} ]; then",
        "    while read -r id state; do",
        '      case "$id" in "#"*|"") continue ;; esac',
        '      if [ "$id" = "$want" ]; then',
        '        [ "$state" = on ]',
        "        return",
        "      fi",
        f"    done < {shlex.quote(wiring['config'])}",
        "  fi",
        '  case "$want" in',
        "    " + ("|".join(shlex.quote(i) for i in defaults_on) if defaults_on else "'')")
        + ") return 0 ;;",
        "  esac",
        "  return 1",
        "}",
        "",
    ]

    for s in mine:
        out += emit_step(s, wiring, programs)

    out += ["# ---- the sequence " + "-" * 52, ""]

    if not root_half:
        out += [
            "# THE ROOT HALF FIRST, through the one unit polkit names. `systemctl",
            "# start` on a `Type=oneshot` returns when the work is done, which is",
            "# what makes this a sequence and not two things happening at once.",
            'if [ "$execute" = true ]; then',
            f"  privileged={shlex.quote(wiring['system-unit'])}",
            "else",
            f"  privileged={shlex.quote(wiring['system-unit-rehearsal'])}",
            "fi",
            'say "== $privileged"',
            f"if ! {shlex.quote(wiring['systemctl'])} start \"$privileged\"; then",
            '  say "!! a metade privilegiada parou; veja: journalctl -u $privileged"',
            "  record system-half failed 0",
            "  exit 1",
            "fi",
            "",
        ]

    if not mine:
        out += ['say "no step in this half"', ""]
    for s in mine:
        name = "step_" + s["id"].replace("-", "_")
        if s["on-failure"] == "stop":
            out += [
                f"if ! {name}; then",
                f"  say \"== sequence stopped at {s['id']}\"",
                "  record sequence stopped 0",
                "  exit 1",
                "fi",
            ]
        else:
            out += [f"{name} || true"]
    out += [
        "",
        'say "== fim"',
        "exit 0",
        "",
    ]
    return "\n".join(out)


# ---------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--updates", type=pathlib.Path, required=True)
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
        help="JSON: paths and unit names, from nix/modules/update.nix",
    )
    args = ap.parse_args()

    source = json.loads(args.updates.read_text(encoding="utf-8"))
    raw = source.get("steps")
    if not isinstance(raw, list) or not raw:
        print("ERROR: assets/updates.nix has no `steps`", file=sys.stderr)
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
                "Acrescente-o em nix/modules/update.nix.",
            )
        # AND THE OTHER WAY ROUND, which is the half that rots in silence: a
        # package left here after its step went away is a dependency nobody
        # needs, carried in the closure of a unit that runs as root.
        for p in sorted(set(programs) - wanted):
            complain("programs", f"`{p}` is resolved and no step uses it")

    if problems:
        print("ERROR: assets/updates.nix does not pass:", file=sys.stderr)
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
        args.out_rust.write_text(emit_rust(steps), encoding="utf-8")
        args.out_keys.write_text(emit_keys(steps), encoding="utf-8")
        keys = emit_keys(steps).split()
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
            emit_runner(steps, root_half=root_half, wiring=wiring, programs=programs),
            encoding="utf-8",
        )
        path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    person = sum(1 for s in steps if not s["root"])
    print(
        f"update: {len(steps) - person} root steps, {person} of the person's, "
        "dois roteiros"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
