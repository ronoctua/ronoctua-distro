#!/usr/bin/env python3
"""The release profile, chosen by measurement (nix-design §3, lane N5).

    profile-bench [--rounds N] [--out DIR] [--flake REF]
                  [--sizing PROGRAM | --max-jobs N --cores N]
                  [--variants A,B,...] [-- BENCH-ARGS...]
    profile-bench report DIR

The builds are sized as the machines and the batteries build
(`nix/build-sizing.nix`, `--sizing`; the app passes it): a variant's layer
alone, one cargo with as many compilations inside it as fit, and then its
units through the variant's own `ronoctua-sized-build` — its heavy unit (the
shell's release program) alone with the cores the memory leaves it,
everything else as many Rust units as fit. `--max-jobs` and `--cores`,
given together, build everything at exactly those numbers instead.

`run` (the default) builds each variant's whole release graph and its
`.#bench` runner (evaluated once, by flake attribute), checks that the variant's profile really
reached the plan and the third-party layer, runs the benchmark ROUNDS times
with the variants interleaved, and writes the decision. `report` recomputes
the decision from a directory a run left, building and measuring nothing.

# The rule

A variant is ACCEPTABLE when, against A (the reference):
  - every median metric (a `p50`, or a single timing) has a median over the
    rounds ≤ 1.03× A's;
  - every `p99` has a median over the rounds ≤ 1.05× A's;
  - no round failed a budget (a ceiling of the benchmark or a frame target),
    and no program of the variant failed in any round.
A difference no larger than one unit of the last printed digit is no
measured difference: the benchmark prints 0.0035 ms and 0.0036 ms, and
either is a rounding of anything between the two. Such a pass is reported
as "within the printed resolution", never hidden. `p95`, `max` and the size
of every program are reported and not judged: the rule names medians and
p99s. Neither is a metric whose program is, in the variant, A's very unit (a
per-package override leaves every other program as it was): two runs of one
file differ by noise, never by the profile.

A round is `ok`, `budget` (a budget failed), `failed` (a program of the
variant stopped with an error, a panic or a timeout, or the round stopped
with no line saying why) or `broken` (the harness could not measure: no
graphical session, a nested compositor that did not come up). `budget` and
`failed` refuse the variant; `broken` rounds are left out, and a decision
needs at least three valid rounds of the variant and of A. Every p50 and
p99 the benchmark prints is read, or the report refuses to decide.

Among the acceptable, the WINNER has the lowest cost of one edited line
reaching a release program — the critical path of the units a one-line edit
in the decision crate (the shell, by default) rebuilds, from each unit's
measured seconds — and then the lowest cold build (the layer plus every
release unit). Two costs within 5% of each other are equal: one build's time
on a shared machine moves by more than that.
"""

import argparse
import datetime
import json
import os
import re
import signal
import statistics
import subprocess
import sys
import time

MEDIAN_TOLERANCE = 1.03
P99_TOLERANCE = 1.05
COST_TIE = 0.05
REFERENCE = "A"
MIN_VALID_ROUNDS = 3
# The crates whose one-line edit is costed: a leaf, a middle, a hub, the
# brain and the shell (nix-design §6's samples).
SAMPLES = [
    "ronoctua-stick",
    "ronoctua-zip",
    "ronoctua-http",
    "ronoctua-xdg",
    "ronoctua-brain",
    "ronoctua-shell",
]
DECISION_CRATE = "ronoctua-shell"
RUN_TIMEOUT_S = 1800

# --- reading `.#bench` ------------------------------------------------------

NUMBER = r"([0-9]+(?:\.[0-9]+)?)"


def index_metrics(m):
    """One line of the index bench: its quantiles, and the p99 of its
    one-letter queries alone — the first keystroke in fast access."""
    name = re.sub(r"[^a-z]+", "-", m[1]).strip("-")
    got = {f"index-{name}-{k}-ms": v for k, v in zip(("p50", "p95", "p99", "max"), m.groups()[1:5])}
    if m[6] is not None:
        got[f"index-{name}-one-letter-p99-ms"] = m[6]
    return got


PATTERNS = [
    (
        re.compile(rf"^layout\s+(\d+) columns: p50={NUMBER} ms\s+p99={NUMBER} ms"),
        lambda m: {f"layout-{m[1]}col-p50-ms": m[2], f"layout-{m[1]}col-p99-ms": m[3]},
    ),
    (
        re.compile(rf"^\s*(upstream|ours)\s+\+ (Monospace|the exact name)\s.*?{NUMBER} ms$"),
        lambda m: {f"shaping-{m[1]}-{m[2].replace(' ', '-')}-ms": m[3]},
    ),
    (
        re.compile(rf"^(with|without)\s+memory_init_cow: p50={NUMBER} ms\s+p99={NUMBER} ms"),
        lambda m: {f"instance-{m[1]}-cow-p50-ms": m[2], f"instance-{m[1]}-cow-p99-ms": m[3]},
    ),
    (
        re.compile(rf"^indexed \d+ names .* in {NUMBER} s; the folder holds (\d+) MiB"),
        lambda m: {"index-build-s": m[1], "index-size-mib": m[2]},
    ),
    (
        re.compile(
            rf"^(names \(/f\)|names \+ contents|names typed, default)\s*: \d+ queries\s+"
            rf"p50={NUMBER} ms\s+p95={NUMBER} ms\s+p99={NUMBER} ms\s+max={NUMBER} ms"
            rf"(?:\s+\(one letter: p99={NUMBER} ms over \d+\))?"
        ),
        index_metrics,
    ),
    (
        re.compile(
            rf"^\s+(?:OK|NO)\s+(first-frame|which-key)\s+n=\s*\d+\s+p50=\s*{NUMBER}\s+"
            rf"p99=\s*{NUMBER}\s+max=\s*{NUMBER} ms"
        ),
        lambda m: {f"{m[1]}-p50-ms": m[2], f"{m[1]}-p99-ms": m[3], f"{m[1]}-max-ms": m[4]},
    ),
]

# A p50 or a p99 as the benchmark prints one: every one of them is a number
# the rule judges, so a line that prints more than the patterns read is a
# reading this script lacks, never a number left out.
QUANTILE = re.compile(r"\bp(?:50|99)=")
READ_QUANTILE = re.compile(r"-p(?:50|99)-ms$")

# The program each metric is measured with. A metric whose program is, in a
# variant, the very unit A has (one derivation, so one binary) measures
# nothing of the variant: its difference from A is noise between two runs of
# the same file, and it is reported, not judged.
PROGRAM_OF = [
    ("layout-", "ronoctua-bench-layout"),
    ("shaping-", "ronoctua-bench-shaping"),
    ("instance-", "ronoctua-bench-instance"),
    ("index-", "ronoctua-bench-index"),
    ("first-frame-", "ronoctua-shell"),
    ("which-key-", "ronoctua-shell"),
]

# What a round says when a BUDGET failed — a number of ours past its ceiling.
BUDGET_FAILED = [
    re.compile(r"^\s+NO\s+(first-frame|which-key)\s+n="),
    re.compile(r"^ERROR: layout on its own goes past"),
    re.compile(r"^ERROR: production shaping went past"),
    re.compile(r"goes past section 5\.2's ceiling"),
    re.compile(r"^ERROR: the index missed the gate"),
]

# What a round says when the HARNESS could not measure: no graphical
# session, or a nested compositor (the same program in every variant) that
# did not come up before the variant's shell was started. Nothing of the
# variant failed, and nothing of it can be judged.
HARNESS_FAILED = [
    re.compile(r"^No graphical session: the frame targets were not measured\."),
    re.compile(r"^ERROR: the nested compositor did not come up\."),
]

# What a round says when a program OF THE VARIANT failed: a benchmark that
# stopped with an error or a panic, a shell that did not start, logged no
# milestone, or opened the leader fewer times than asked. The variant is
# refused, as for a budget: a crash of the variant's program and one of the
# machine print the same lines, and refusing is the side that never ships a
# profile whose programs fail.
PROGRAM_FAILED = [
    re.compile(r"^ERROR: "),
    re.compile(r"^Error: "),
    re.compile(r"panicked at"),
    re.compile(r"^\s*startup \d+ FAILED"),
    re.compile(r"^\s*NO\s+which-key: \d+ openings measured"),
    re.compile(r"^\s*(first-frame|which-key): not measured"),
]

# What `run_rounds` writes for a round it killed, and the exit it gives it.
ROUND_KILLED = re.compile(r"^== killed after \d+ s$")
TIMEOUT_EXIT = 124

# The metrics the frame half of the benchmark prints: a round without them did
# not measure the frames, and is not a round of the frame targets.
FRAME_METRICS = ("first-frame-p50-ms", "which-key-p50-ms")


def parse_line(line):
    got = {}
    for pattern, take in PATTERNS:
        m = pattern.search(line)
        if m:
            got.update(take(m))
    return got


def parse_log(text):
    """The metrics a `.#bench` log prints, as their printed strings."""
    got = {}
    for line in text.splitlines():
        got.update(parse_line(line))
    return got


def unread(text):
    """The lines that print a p50 or a p99 the patterns do not read: a
    number the rule would never judge."""
    missed = []
    for line in text.splitlines():
        printed = len(QUANTILE.findall(line))
        if printed and sum(1 for k in parse_line(line) if READ_QUANTILE.search(k)) < printed:
            missed.append(line.strip())
    return missed


def lines_matching(text, patterns, unless=()):
    return [
        line.strip()
        for line in text.splitlines()
        if any(p.search(line) for p in patterns) and not any(p.search(line) for p in unless)
    ]


def budget_failures(text):
    return lines_matching(text, BUDGET_FAILED)


def harness_failures(text):
    return lines_matching(text, HARNESS_FAILED)


def program_failures(text):
    """The lines that say a program of the variant failed in a round, or
    that the round hung and was killed."""
    return lines_matching(text, PROGRAM_FAILED + [ROUND_KILLED], unless=BUDGET_FAILED + HARNESS_FAILED)


def classify(text, exit_code):
    """`ok`; `budget` (a budget failed) or `failed` (a program of the
    variant failed, or the round stopped with nothing saying the harness was
    why), both of which refuse the variant; or `broken` (the harness could
    not measure: left out, never judged)."""
    if budget_failures(text):
        return "budget"
    if harness_failures(text):
        return "broken"
    if program_failures(text):
        return "failed"
    metrics = parse_log(text)
    if exit_code != 0 or not all(m in metrics for m in FRAME_METRICS):
        # Stopped, or silent about its frames, with no line saying the
        # harness was why: an unknown cause is the variant's.
        return "failed"
    return "ok"


def failure_of(text, exit_code):
    """The first line that says why a `failed` round failed."""
    found = program_failures(text)
    if found:
        return found[0]
    return f"exit {exit_code}" if exit_code != 0 else "no frame measured, and no line saying why"


def judged(metric):
    """`median`, `p99` or None (reported, not judged)."""
    if "-p99-" in metric:
        return "p99"
    if "-p95-" in metric or "-max-" in metric or metric.endswith("-mib"):
        return None
    return "median"


def quantum(printed):
    """One unit of the last printed digit: `0.0035` → 0.0001, `170` → 1."""
    if "." in printed:
        return 10.0 ** -len(printed.split(".", 1)[1])
    return 1.0


def program_of(metric):
    return next((program for prefix, program in PROGRAM_OF if metric.startswith(prefix)), None)


def programs(units):
    """Each program of a variant's `cost` rows, by name, as its unit's
    identity (the `metrics` output: one derivation, one path)."""
    found = {}
    for u in units:
        m = re.fullmatch(r"(.+)-(\d[^-]*)", u["name"])
        if u["kind"] == "bin" and m:
            found[m.group(1)] = u["metrics"]
    return found


def same_programs(reference_units, variant_units):
    """The programs the benchmark measures that a variant shares with the
    reference, unit for unit."""
    ref, var = programs(reference_units), programs(variant_units)
    measured = {program for _, program in PROGRAM_OF}
    return {p for p in measured if p in ref and p in var and ref[p] == var[p]}


def judge(reference_runs, variant_runs, same=frozenset()):
    """Every judged metric of a variant against the reference's, from the
    printed strings of each valid round. `same` names the programs the
    variant shares with the reference, whose metrics are not judged."""
    rows = []
    names = sorted({m for run in reference_runs for m in run})
    for metric in names:
        kind = judged(metric)
        ref = [r[metric] for r in reference_runs if metric in r]
        var = [r[metric] for r in variant_runs if metric in r]
        row = {"metric": metric, "kind": kind}
        if not ref:
            continue
        ref_median = statistics.median(float(x) for x in ref)
        row["reference"] = ref_median
        if not var:
            row.update(variant=None, ratio=None, passes=kind is None, note="not measured")
            rows.append(row)
            continue
        var_median = statistics.median(float(x) for x in var)
        q = max(quantum(x) for x in ref + var)
        ratio = var_median / ref_median if ref_median else (1.0 if var_median == 0 else float("inf"))
        row.update(variant=var_median, ratio=ratio, quantum=q)
        if kind is not None and program_of(metric) in same:
            row.update(passes=True, note="the reference's own program: not judged")
        elif kind is None:
            row.update(passes=True, note="reported, not judged")
        else:
            tolerance = MEDIAN_TOLERANCE if kind == "median" else P99_TOLERANCE
            row["tolerance"] = tolerance
            strict = var_median <= tolerance * ref_median
            resolved = var_median <= tolerance * ref_median + q + 1e-12
            row["passes"] = resolved
            row["note"] = (
                "" if strict else ("within the printed resolution" if resolved else "past the tolerance")
            )
        rows.append(row)
    return rows


# --- what an edit costs, and the cold build ---------------------------------


def cargo_elapsed(text):
    """Seconds out of cargo's `Finished … in 6m 20s` (or `in 45.30s`: cargo
    never prints hours, `75m 03s`), the last one in the text; None when there
    is none."""
    found = None
    for m in re.finditer(r"Finished .* in ((?:\d+h )?(?:\d+m )?(?:\d+(?:\.\d+)?s)?)\s*$", text, re.M):
        found = m.group(1)
    if found is None:
        return None
    total = 0.0
    for value, unit in re.findall(r"(\d+(?:\.\d+)?)([hms])", found):
        total += float(value) * {"h": 3600, "m": 60, "s": 1}[unit]
    return total


def topological(names, deps):
    order, seen = [], set()

    def visit(n, stack=()):
        if n in seen:
            return
        if n in stack:
            raise ValueError(f"a cycle through {n}")
        for d in deps.get(n, ()):
            if d in names:
                visit(d, stack + (n,))
        seen.add(n)
        order.append(n)

    for n in sorted(names):
        visit(n)
    return order


def critical_path(names, deps, seconds):
    """The longest chain of `names` through `deps`, by `seconds`."""
    names = set(names)
    finish = {}
    for n in topological(names, deps):
        below = [finish[d] for d in deps.get(n, ()) if d in names]
        finish[n] = seconds[n] + (max(below) if below else 0.0)
    return max(finish.values()) if finish else 0.0


def edit_cost(units, seconds, crate):
    """The release units one edited line in `crate` rebuilds, with their sum
    and critical path: its library and everything above it, or — for a
    crate that is only programs, as the shell is — its programs. `units` are
    the rows of a variant's `cost` file."""
    deps = {u["name"]: u["deps"] for u in units}
    libs = [u["name"] for u in units if u["kind"] == "lib" and u["crate"] == crate]
    programs = [u["name"] for u in units if u["kind"] == "bin" and u["crate"] == crate]
    start = libs or programs
    if not start:
        raise ValueError(f"{crate} has no unit in the release graph")
    affected = set(start)
    changed = True
    while changed:
        changed = False
        for u in units:
            if u["name"] not in affected and any(d in affected for d in u["deps"]):
                affected.add(u["name"])
                changed = True
    return {
        "units": sorted(affected),
        "programs": sorted(u["name"] for u in units if u["name"] in affected and u["kind"] == "bin"),
        "sum": sum(seconds[n] for n in affected),
        "critical": critical_path(affected, deps, seconds),
    }


def cold_build(units, seconds, layer_seconds):
    """The layer plus every release unit. A layer whose time is unknown is
    refused: zero would make the tie-break compare cold builds that leave
    out the one part the variants move most."""
    if layer_seconds is None:
        raise ValueError("the layer's build time is unknown")
    deps = {u["name"]: u["deps"] for u in units}
    names = [u["name"] for u in units]
    total = sum(seconds[n] for n in names)
    return {
        "layer": layer_seconds,
        "units-sum": total,
        "units-critical": critical_path(names, deps, seconds),
        "work": layer_seconds + total,
    }


def program_sizes(bundle):
    """Every program of a variant's `bundle`, in bytes — a wrapped one is the
    binary behind its wrapper (`.<name>-wrapped`) — and the whole `bin/`."""
    where = os.path.join(bundle, "bin")
    plain, wrapped, total = {}, {}, 0
    for name in sorted(os.listdir(where)):
        size = os.stat(os.path.join(where, name)).st_size
        total += size
        m = re.fullmatch(r"\.(.+)-wrapped", name)
        if m:
            wrapped[m.group(1)] = size
        else:
            plain[name] = size
    return {"total": total, "programs": dict(sorted({**plain, **wrapped}.items()))}


# --- did the variant reach the build? ---------------------------------------


def check_plan(plan, lto, codegen_units, packages=None):
    """What in a variant's plan is not the variant: a release profile with
    another codegen-unit count than the variant's (or than its per-package
    override, for that package), an LTO the variant does not ask for."""
    packages = packages or {}
    problems = []
    release = plan["graphs"]["release"]["units"]
    profiles = plan["profiles"]
    for key, unit in sorted(release.items()):
        profile = profiles[unit["profile"]]
        wanted = packages.get(unit["pkg"], {}).get("codegen-units", codegen_units)
        if profile.get("codegen_units") is not None and profile["codegen_units"] != wanted:
            problems.append(f"{key}: codegen-units {profile['codegen_units']}, not {wanted}")
        if lto == "false" and (unit["lto"] == "bitcode" or unit["lto"].startswith("run:")):
            problems.append(f"{key}: LTO {unit['lto']} in a variant without LTO")
    for name in sorted(packages):
        if not any(u["pkg"] == name for u in release.values()):
            problems.append(f"the override names {name}, which the release graph does not build")
    if lto == "thin" and not any(u["lto"] == "run:thin" for u in release.values()):
        problems.append("no unit runs thin LTO in a variant with thin LTO")
    return problems


def check_layer_log(text, lto, codegen_units):
    """The same question to the cargo log of the third-party layer."""
    problems = []
    counts = sorted({int(n) for n in re.findall(r"-C codegen-units=(\d+)", text)})
    if counts != [codegen_units]:
        problems.append(f"the layer was compiled with codegen-units {counts}, not [{codegen_units}]")
    bitcode = "-C linker-plugin-lto" in text
    if lto == "thin" and not bitcode:
        problems.append("no third-party library carries bitcode, and the variant runs thin LTO")
    if lto == "false" and bitcode:
        problems.append("a third-party library carries bitcode, and the variant runs no LTO")
    return problems


# --- the decision -----------------------------------------------------------


def decide(summary):
    """`summary[letter]` holds `acceptable`, `edit` (the decision crate's
    critical path) and `cold` (`work`). Returns the winner and the ranking."""
    acceptable = [v for v in sorted(summary) if summary[v]["acceptable"]]
    if not acceptable:
        return None, []

    def better(a, b):
        ea, eb = summary[a]["edit"], summary[b]["edit"]
        if abs(ea - eb) > COST_TIE * min(ea, eb):
            return ea < eb
        ca, cb = summary[a]["cold"], summary[b]["cold"]
        if ca != cb:
            return ca < cb
        return a < b

    ranking = []
    rest = list(acceptable)
    while rest:
        best = rest[0]
        for v in rest[1:]:
            if better(v, best):
                best = v
        ranking.append(best)
        rest.remove(best)
    return ranking[0], ranking


# --- running ----------------------------------------------------------------


def log(message):
    print(f"profile-bench: {message}", flush=True)


# What one evaluation of a variant hands the builds: every `.drv` they name,
# and the outputs the run reads.
VARIANT_PATHS = (
    "v: { layer = v.layer.drvPath; sized = v.sized.drvPath; sizedOut = v.sized.outPath;"
    " cost = v.cost.drvPath; runner = v.runner.drvPath; bundle = v.bundle.drvPath;"
    " out = { cost = v.cost.outPath; runner = v.runner.outPath; bundle = v.bundle.outPath; }; }"
)


def nix_build_drvs(jobs, cores, drvs):
    """`nix build` of evaluated derivations, every output, at `jobs` and `cores`."""
    return ["nix", "build", "--no-link", "--max-jobs", str(jobs), "--option", "cores", str(cores)] + [
        f"{d}^*" for d in drvs
    ]


def variant_builds(paths, given, sized):
    """The builds of one variant, in order, as `(name, command)`: `paths` is
    what its evaluation said (`VARIANT_PATHS`).

    Given `--max-jobs` and `--cores` (`given`, a pair), every build at
    exactly those. Otherwise as every machine builds (`sized`, what the
    machine's build sizing says now): the LAYER first and alone — one cargo,
    which runs `NIX_BUILD_CORES` compilations inside it, so one derivation
    with every slot; then the variant's own `ronoctua-sized-build`, a small
    script, at the general numbers; then through it every release unit, the
    bench and the bundle — its heavy unit alone with the cores the memory
    leaves it, the rest as many Rust units at once as fit. At the general
    numbers the shell's link would run beside three release units, past the
    budget, and at one core: its measured seconds, which the edit costs and
    the cold build add up, would be no machine's."""
    products = [paths["cost"], paths["runner"], paths["bundle"]]
    if given:
        jobs, cores = given
        return [("layer", nix_build_drvs(jobs, cores, [paths["layer"]])), ("units", nix_build_drvs(jobs, cores, products))]
    return [
        ("layer", nix_build_drvs(1, sized["slots"], [paths["layer"]])),
        ("sized", nix_build_drvs(sized["jobs"], sized["cores"], [paths["sized"]])),
        (
            "units",
            [os.path.join(paths["sizedOut"], "bin", "ronoctua-sized-build"), "--no-link"] + [f"{d}^*" for d in products],
        ),
    ]


def run_logged(command, logfile):
    """`command`, its output into `logfile`; a refusal naming it when it fails."""
    with open(logfile, "w") as log_to:
        done = subprocess.run(command, stdout=log_to, stderr=subprocess.STDOUT, text=True)
    if done.returncode != 0:
        raise SystemExit(f"profile-bench: {os.path.basename(command[0])} failed; see {logfile}")


def sized_line(text):
    """The line `ronoctua-sized-build` says how many compilations fit in, or
    None."""
    m = re.search(r"^\d+ compilations fit at once -> .*$", text, re.M)
    return m.group(0) if m else None


def read(path):
    with open(path, errors="replace") as f:
        return f.read()


# The format of a unit's `metrics` this reads (`METRICS_FORMAT` of
# `nix/scripts/rust-unit.py`): what the whole build held that the budget pays
# for, and the cores it was built with. A peak depends on its cores, so the
# report says them; a measurement of another format is of another quantity
# (1, the largest process's RSS; 2, the PSS of the build's processes, file
# pages split with the builds beside it) and is refused.
METRICS_FORMAT = 3


def unit_peak(path):
    """(peak KiB, cores) of a unit's `metrics` output; refused, naming it,
    when it is of another format than `METRICS_FORMAT` (one with no
    `format` file is 1, what the driver wrote before it said one)."""
    named = os.path.join(path, "format")
    said = read(named).strip() if os.path.exists(named) else "1"
    if said != str(METRICS_FORMAT):
        raise SystemExit(
            f"profile-bench: {path} is a measurement of format {said}, not {METRICS_FORMAT}: its peak is of "
            "another quantity, and this does not report it as one; build the variant again with the driver "
            "of this tree (nix/scripts/rust-unit.py)"
        )
    return int(read(os.path.join(path, "peak-kib"))), int(read(os.path.join(path, "cores")))


def read_json(path):
    with open(path) as f:
        return json.load(f)


def write_json(path, value):
    with open(path, "w") as f:
        json.dump(value, f, indent=2, sort_keys=True)
        f.write("\n")


def build_variant(args, letter, out):
    where = os.path.join(out, letter)
    os.makedirs(where, exist_ok=True)
    given = (args.max_jobs, args.cores) if args.max_jobs is not None else None
    # Asked again for each variant: the run lasts hours, and what is free moves.
    sized = None if given else sizing_of(args.sizing)
    jobs, cores = given or (sized["jobs"], sized["cores"])
    log(f"{letter}: evaluating")
    evaluated = os.path.join(where, "eval.log")
    with open(evaluated, "w") as err:
        done = subprocess.run(
            ["nix", "eval", "--json", "--max-jobs", str(jobs), "--option", "cores", str(cores)]
            + [f"{args.flake}#{args.attr}.{letter}", "--apply", VARIANT_PATHS],
            stdout=subprocess.PIPE,
            stderr=err,
            text=True,
        )
    if done.returncode != 0:
        raise SystemExit(f"profile-bench: nix eval failed; see {evaluated}")
    paths = json.loads(done.stdout)
    wall = {}
    for name, command in variant_builds(paths, given, sized):
        log(f"{letter}: building {'every release unit and the bench' if name == 'units' else 'the ' + name}")
        started = time.monotonic()
        logfile = os.path.join(where, "build.log" if name == "units" else f"build-{name}.log")
        run_logged(command, logfile)
        wall[name] = time.monotonic() - started
    sizing = (
        {"max-jobs": given[0], "cores": given[1]}
        if given
        else {
            "layer": {"max-jobs": 1, "cores": sized["slots"]},
            "units": sized_line(read(os.path.join(where, "build.log"))),
        }
    )
    built = {
        "cost": paths["out"]["cost"],
        "runner": paths["out"]["runner"],
        "bundle": paths["out"]["bundle"],
        "wall": {"layer": wall["layer"], "units": wall["units"]},
        "sizing": sizing,
    }
    write_json(os.path.join(where, "build.json"), built)
    return built


def verify_variant(letter, cost):
    plan = read_json(cost["plan"])
    problems = check_plan(plan, cost["lto"], cost["codegenUnits"], cost.get("packages"))
    layer_log = os.path.join(cost["layer"], "cargo", "release.log")
    problems += check_layer_log(read(layer_log), cost["lto"], cost["codegenUnits"])
    if problems:
        raise SystemExit(
            f"profile-bench: variant {letter} is not the profile it names:\n  " + "\n  ".join(problems[:20])
        )


# What `nix/bench-frames.nix` binds under `$TMPDIR` (`mktemp -d -t
# ronoctua-bench-XXXXXX`, then `$dir/ipc.sock`), and the most a Unix socket's
# path holds (`sun_path` is 108 bytes, the NUL included).
SOCKET_UNDER_TMPDIR = "/ronoctua-bench-XXXXXX/ipc.sock"
SOCKET_PATH_MAX = 107


def rounds_tmpdir(out):
    """The `TMPDIR` every round runs with: a folder of the run, on disk —
    never `/tmp`, the RAM tmpfs. Refused when the nested compositor's socket
    would not fit under it."""
    tmp = os.path.join(os.path.abspath(out), "t")
    if len(tmp) + len(SOCKET_UNDER_TMPDIR) > SOCKET_PATH_MAX:
        raise SystemExit(
            f"profile-bench: {tmp} is too long a TMPDIR: the nested compositor's socket under it "
            f"would pass the {SOCKET_PATH_MAX} bytes a Unix socket path holds; pass a shorter --out"
        )
    os.makedirs(tmp, exist_ok=True)
    return tmp


def run_rounds(args, built, out, bench_args):
    letters = list(built)
    tmp = rounds_tmpdir(out)
    env = dict(os.environ, TMPDIR=tmp)
    for r in range(1, args.rounds + 1):
        # Rotated each round: no variant always runs first, after a build, or
        # last, after the others warmed the caches.
        shift = (r - 1) % len(letters)
        for letter in letters[shift:] + letters[:shift]:
            runner = built[letter]["runner"]
            program = os.path.join(runner, "bin", f"ronoctua-bench-profile-{letter.lower()}")
            path = os.path.join(out, letter, f"run-{r}.log")
            log(f"round {r}/{args.rounds}: {letter}")
            with open(path, "w") as f:
                # The load beside it, for whoever reads a slow round later.
                f.write(
                    f"== round {r} {datetime.datetime.now(datetime.timezone.utc).isoformat()} "
                    f"load {load()} tmp {tmp}\n"
                )
                f.flush()
                # A session of its own: a round that hangs is killed with the
                # nested compositor and the footer it started, whose own trap
                # a SIGKILL never runs.
                child = subprocess.Popen(
                    [program] + bench_args, stdout=f, stderr=subprocess.STDOUT, start_new_session=True, env=env
                )
                try:
                    code = child.wait(timeout=RUN_TIMEOUT_S)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
                    f.write(f"== killed after {RUN_TIMEOUT_S} s\n")
                    code = TIMEOUT_EXIT
                f.write(f"== exit {code}\n")


def load():
    try:
        return " ".join(read("/proc/loadavg").split()[:3])
    except OSError:
        return "unknown"


def exit_of(text):
    m = re.search(r"^== exit (-?\d+)$", text, re.M)
    return int(m.group(1)) if m else 1


def report(out, letters, decision_crate=DECISION_CRATE):
    summary, rounds, judgements, costs, rows_of = {}, {}, {}, {}, {}
    for letter in letters:
        where = os.path.join(out, letter)
        runs = sorted(
            (f for f in os.listdir(where) if re.fullmatch(r"run-\d+\.log", f)),
            key=lambda f: int(f[4:-4]),
        )
        rounds[letter] = []
        for name in runs:
            text = read(os.path.join(where, name))
            missed = unread(text)
            if missed:
                raise SystemExit(
                    f"profile-bench: {os.path.join(where, name)} prints a number this script does not read, "
                    f"so the rule would never judge it: {missed[0]!r}"
                )
            code = exit_of(text)
            verdict = classify(text, code)
            rounds[letter].append(
                {
                    "log": name,
                    "verdict": verdict,
                    "metrics": parse_log(text),
                    "budget": budget_failures(text),
                    "failure": failure_of(text, code) if verdict == "failed" else None,
                }
            )
        built = read_json(os.path.join(where, "build.json"))
        if "bundle" not in built:
            raise SystemExit(
                f"profile-bench: {os.path.join(where, 'build.json')} names no bundle, so the size of the "
                "variant's programs is unknown"
            )
        cost = read_json(built["cost"])
        seconds = {u["name"]: float(read(os.path.join(u["metrics"], "seconds"))) for u in cost["units"]}
        peaks = {u["name"]: unit_peak(u["metrics"]) for u in cost["units"]}
        heaviest = max(peaks, key=lambda n: peaks[n][0])
        layer_log = os.path.join(cost["layer"], "cargo", "release.log")
        layer_seconds = cargo_elapsed(read(layer_log))
        if layer_seconds is None:
            raise SystemExit(
                f"profile-bench: variant {letter}: {layer_log} has no cargo `Finished … in` line, so the "
                "layer's build time, and the cold build, are unknown"
            )
        rows_of[letter] = cost["units"]
        costs[letter] = {
            "lto": cost["lto"],
            "codegen-units": cost["codegenUnits"],
            "packages": cost.get("packages", {}),
            "edits": {c: edit_cost(cost["units"], seconds, c) for c in SAMPLES},
            "cold": cold_build(cost["units"], seconds, layer_seconds),
            "wall": built["wall"],
            # As the build wrote it (a run before the sized builds wrote the
            # two numbers it was given).
            "sizing": built.get("sizing") or {k: built[k] for k in ("max-jobs", "cores") if k in built},
            # The heaviest unit's peak and the cores it was built with: a
            # heavy unit is built with the cores the budget leaves, so two
            # variants' peaks are compared only at the same cores.
            "peak-kib": peaks[heaviest][0],
            "peak-cores": peaks[heaviest][1],
            "peak-unit": heaviest,
            "sizes": program_sizes(built["bundle"]),
            "heaviest": sorted(seconds, key=seconds.get, reverse=True)[:5],
            "seconds": {n: seconds[n] for n in sorted(seconds, key=seconds.get, reverse=True)[:10]},
        }
        for edit in costs[letter]["edits"].values():
            edit.pop("units")
    valid = {v: [r["metrics"] for r in rounds[v] if r["verdict"] == "ok"] for v in letters}
    for letter in letters:
        failed = [r["log"] for r in rounds[letter] if r["verdict"] == "budget"]
        crashed = [r for r in rounds[letter] if r["verdict"] == "failed"]
        broken = [r["log"] for r in rounds[letter] if r["verdict"] == "broken"]
        enough = len(valid[letter]) >= MIN_VALID_ROUNDS and len(valid[REFERENCE]) >= MIN_VALID_ROUNDS
        same = same_programs(rows_of[REFERENCE], rows_of[letter]) if letter != REFERENCE else set()
        judgements[letter] = judge(valid[REFERENCE], valid[letter], same) if enough else []
        reasons = []
        if failed:
            reasons.append(f"a budget failed in {', '.join(failed)}")
        reasons += [f"its programs failed in {r['log']} ({r['failure']})" for r in crashed]
        if not enough:
            reasons.append(
                f"fewer than {MIN_VALID_ROUNDS} valid rounds ({REFERENCE}: {len(valid[REFERENCE])}, "
                f"{letter}: {len(valid[letter])})"
            )
        reasons += [
            f"{row['metric']}: {row['ratio']:.3f}× A's {row['kind']}"
            if row.get("ratio") is not None
            else f"{row['metric']}: not measured"
            for row in judgements[letter]
            if not row["passes"]
        ]
        summary[letter] = {
            "acceptable": not reasons,
            "same-programs": sorted(same),
            "reasons": reasons,
            "broken": broken,
            "failed": [r["log"] for r in crashed],
            "valid-rounds": len(valid[letter]),
            "edit": costs[letter]["edits"][decision_crate]["critical"],
            "cold": costs[letter]["cold"]["work"],
        }
    winner, ranking = decide(summary)
    result = {
        "rule": {
            "median": MEDIAN_TOLERANCE,
            "p99": P99_TOLERANCE,
            "cost-tie": COST_TIE,
            "reference": REFERENCE,
            "decision-crate": decision_crate,
        },
        "winner": winner,
        "ranking": ranking,
        "summary": summary,
        "costs": costs,
        "judgements": judgements,
        "rounds": {v: [{k: r[k] for k in ("log", "verdict", "budget", "failure")} for r in rounds[v]] for v in letters},
        "medians": {
            v: {m: statistics.median(float(r[m]) for r in valid[v] if m in r) for m in sorted({m for r in valid[v] for m in r})}
            for v in letters
        },
    }
    write_json(os.path.join(out, "report.json"), result)
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write(markdown(result, letters))
    return result


# Which programs the size rows of `report.md` show (all are in `report.json`).
SIZE_SHOWN = 0.01
SIZES_SHOWN = 10


def moved_programs(costs, letters):
    """The programs whose size moves from the reference's by more than
    `SIZE_SHOWN` in some variant, the reference's largest first."""
    ref = costs[REFERENCE]["sizes"]["programs"]
    moved = [
        p
        for p in ref
        if any(
            abs(costs[v]["sizes"]["programs"].get(p, 0) - ref[p]) > SIZE_SHOWN * ref[p]
            for v in letters
            if v != REFERENCE
        )
    ]
    return sorted(moved, key=lambda p: -ref[p])[:SIZES_SHOWN]


def sized(costs, letter, program):
    """A size in MB, with its change against the reference's."""
    of = (lambda v: costs[v]["sizes"]["total"]) if program is None else (
        lambda v: costs[v]["sizes"]["programs"].get(program)
    )
    size, ref = of(letter), of(REFERENCE)
    if size is None:
        return "-"
    if letter == REFERENCE or not ref:
        return f"{size / 1e6:.1f}"
    return f"{size / 1e6:.1f} ({(size - ref) / ref:+.1%})"


def markdown(result, letters):
    lines = ["# Release profile — measured", ""]
    costs, summary = result["costs"], result["summary"]
    lines.append("| | " + " | ".join(letters) + " |")
    lines.append("|---|" + "---|" * len(letters))
    lines.append("| lto, codegen-units | " + " | ".join(f"{costs[v]['lto']}, {costs[v]['codegen-units']}" for v in letters) + " |")
    lines.append(
        "| per-package | "
        + " | ".join(
            ", ".join(f"{p}: {o}" for p, o in sorted(costs[v]["packages"].items())) or "-" for v in letters
        )
        + " |"
    )
    lines.append("| acceptable | " + " | ".join("yes" if summary[v]["acceptable"] else "no" for v in letters) + " |")
    lines.append("| valid rounds | " + " | ".join(str(summary[v]["valid-rounds"]) for v in letters) + " |")
    for crate in SAMPLES:
        lines.append(
            f"| edit in {crate}: critical / sum (s) | "
            + " | ".join(f"{costs[v]['edits'][crate]['critical']:.0f} / {costs[v]['edits'][crate]['sum']:.0f}" for v in letters)
            + " |"
        )
    lines.append("| layer (s) | " + " | ".join(f"{costs[v]['cold']['layer']:.0f}" for v in letters) + " |")
    lines.append("| units: sum / critical (s) | " + " | ".join(f"{costs[v]['cold']['units-sum']:.0f} / {costs[v]['cold']['units-critical']:.0f}" for v in letters) + " |")
    lines.append("| cold work (s) | " + " | ".join(f"{costs[v]['cold']['work']:.0f}" for v in letters) + " |")
    lines.append(
        "| peak (MiB, at cores) | "
        + " | ".join(f"{costs[v]['peak-kib'] / 1024:.0f} at {costs[v]['peak-cores']}" for v in letters)
        + " |"
    )
    lines.append("| every release program (MB) | " + " | ".join(sized(costs, v, None) for v in letters) + " |")
    for program in moved_programs(costs, letters):
        lines.append(f"| {program} (MB) | " + " | ".join(sized(costs, v, program) for v in letters) + " |")
    lines.append("")
    lines.append(f"Winner: **{result['winner']}** (ranking {', '.join(result['ranking']) or 'none'}).")
    lines.append("")
    cores = {costs[v]["peak-cores"] for v in letters}
    if len(cores) > 1:
        lines.append(
            "- the peaks were measured at different cores ("
            + ", ".join(f"{v}: {costs[v]['peak-unit']} at {costs[v]['peak-cores']}" for v in letters)
            + "): a peak grows with its cores, so they are not compared"
        )
    for v in letters:
        if summary[v]["reasons"]:
            lines.append(f"- {v} not acceptable: " + "; ".join(summary[v]["reasons"]))
        if summary[v]["same-programs"]:
            lines.append(f"- {v} runs A's own {', '.join(summary[v]['same-programs'])}: their numbers are not judged")
        if summary[v]["broken"]:
            lines.append(
                f"- {v}: rounds the harness could not measure, left out: {', '.join(summary[v]['broken'])}"
            )
    lines.append("")
    lines.append(
        f"Sizes are reported, not judged: the whole `bin/` of each variant's bundle, and every program "
        f"whose size moves from {REFERENCE}'s by more than {SIZE_SHOWN:.0%} in some variant (at most "
        f"{SIZES_SHOWN}, the largest first); the tests and benches are in the bundle too."
    )
    lines.append("")
    lines.append("## Medians over the valid rounds")
    lines.append("")
    medians = result["medians"]
    names = sorted({m for v in letters for m in medians[v]})
    lines.append("| metric | " + " | ".join(letters) + " |")
    lines.append("|---|" + "---|" * len(letters))
    for m in names:
        cells = []
        for v in letters:
            value = medians[v].get(m)
            row = next((r for r in result["judgements"].get(v, []) if r["metric"] == m), None)
            mark = ""
            if row and row.get("note") == "within the printed resolution":
                mark = " ~"
            elif row and row.get("note") == "the reference's own program: not judged":
                mark = " ="
            elif row and not row["passes"]:
                mark = " ✗"
            cells.append("-" if value is None else f"{value:g}{mark}")
        lines.append(f"| {m} | " + " | ".join(cells) + " |")
    lines.append("")
    lines.append(
        "`✗` past the tolerance; `~` within the printed resolution only; `=` measured with A's own "
        "program (the same unit), not judged."
    )
    return "\n".join(lines) + "\n"


def sizing_of(program):
    """What the machine's build sizing (`nix/build-sizing.nix`) says now: how
    many compilations fit at once (`slots`, the layer's cargo), and how the
    builds that are no unit are spread (`jobs`, `cores`). A sizing that does
    not answer is a refusal, never a guess."""
    done = subprocess.run([program], capture_output=True, text=True)
    said = dict(line.split("=", 1) for line in done.stdout.splitlines() if "=" in line)
    try:
        sized = {k: int(said[k]) for k in ("slots", "jobs", "cores")}
    except (KeyError, ValueError):
        raise SystemExit(f"profile-bench: {program} said no `slots`, `jobs` and `cores`: {done.stderr.strip()}")
    if done.returncode != 0 or min(sized.values()) < 1:
        raise SystemExit(f"profile-bench: {program} said {sized}")
    return sized


def main(argv):
    if "--" in argv:
        cut = argv.index("--")
        argv, bench_args = argv[:cut], argv[cut + 1 :]
    else:
        bench_args = []
    parser = argparse.ArgumentParser(
        prog="profile-bench", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("command", nargs="?", default="run", choices=["run", "report"])
    parser.add_argument("dir", nargs="?", help="report: the directory a run wrote")
    parser.add_argument("--attr", required=True, help="the flake attribute holding the variants")
    parser.add_argument("--variants", required=True, help="comma-separated letters; A is the reference")
    parser.add_argument("--rounds", type=int, default=5, help="rounds of the benchmark per variant (default 5)")
    parser.add_argument("--out", help="where to write (default: $XDG_CACHE_HOME/ronoctua-distro/profile-bench/<time>)")
    parser.add_argument("--flake", help="default: git+file:// of the checkout this runs in")
    parser.add_argument("--max-jobs", type=int, help="every build's --max-jobs, with --cores (default: sized)")
    parser.add_argument("--cores", type=int, help="every build's --option cores, with --max-jobs (default: sized)")
    parser.add_argument("--sizing", help="ronoctua-build-sizing: the builds sized as every machine builds")
    args = parser.parse_args(argv)
    letters = [v for v in args.variants.split(",") if v]
    if REFERENCE not in letters:
        parser.error(f"the variants must include {REFERENCE}, the reference")
    if args.command == "report":
        if not args.dir:
            parser.error("report needs the directory a run wrote")
        result = report(args.dir, letters)
        print(read(os.path.join(args.dir, "report.md")), end="")
        return 0 if result["winner"] else 1
    if args.rounds < MIN_VALID_ROUNDS:
        parser.error(f"--rounds must be at least {MIN_VALID_ROUNDS}")
    # THE BUILDS' SIZE, asked only by a run (a report builds nothing): both
    # numbers given, or the machine's sizing — never one of them guessed.
    if (args.max_jobs is None) != (args.cores is None):
        parser.error("--max-jobs and --cores go together; without both the builds are sized (--sizing)")
    if args.max_jobs is None and not args.sizing:
        parser.error("pass --sizing (the app does), or both --max-jobs and --cores")
    if not args.flake:
        top = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True)
        if top.returncode != 0:
            parser.error("not inside a checkout: pass --flake")
        args.flake = f"git+file://{top.stdout.strip()}"
    cache = os.environ.get("XDG_CACHE_HOME") or os.path.join(os.path.expanduser("~"), ".cache")
    out = args.out or os.path.join(
        cache, "ronoctua-distro", "profile-bench", datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    os.makedirs(out, exist_ok=True)
    # Refused now, not after the hours of building.
    rounds_tmpdir(out)
    log(f"writing to {out}")
    built = {}
    for letter in letters:
        built[letter] = build_variant(args, letter, out)
        verify_variant(letter, read_json(built[letter]["cost"]))
    run_rounds(args, built, out, bench_args)
    result = report(out, letters)
    print(read(os.path.join(out, "report.md")), end="")
    return 0 if result["winner"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
