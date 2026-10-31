"""The console's keyboard maps, out of xkeyboard-config (`nix/console-keymaps.nix`).

`--one <answer>`: the map of one answer (`gb`, `us(euro)`), written to `--out`,
a file. An answer `base.lst` does not offer is refused.

`--every`: the map of every answer `base.lst` offers — each layout plain, and
with each of its variants — as `--out/<answer>.map`, and `--out/left-out.txt`
naming the answers whose layout has no symbols file, which have no map.

A map is made by `ckbcomp` and read back by `loadkeys -b`. `ckbcomp` falls
back in silence (a variant the layout lacks gives the plain layout, exit 0,
and a warning): a warning that it could not find what it was asked for is a
refusal here, never a map.
"""

import argparse
import concurrent.futures
import os
import re
import subprocess
import sys

# What `ckbcomp` says when a layout has no symbols file at all.
NO_SYMBOLS = re.compile(r'Can not find file "symbols/')
# What it says when it could not find something and went on with another.
NOT_FOUND = re.compile(r'Can not find "')
ANSWER = re.compile(r"([^()\s]+)(?:\(([^()\s]+)\))?")


def offered(base_lst):
    """The answers `base.lst` offers, in its order: each layout plain, then
    each variant of each layout. Read as the installer reads it
    (`crates/ronoctua-installer/src/model.rs`, `section`): a variant's
    description starts with its layout and a colon."""
    layouts, variants = [], {}
    section = None
    with open(base_lst, encoding="utf-8") as f:
        for line in f:
            if line.startswith("!"):
                words = line[1:].split()
                section = words[0] if words else None
                continue
            fields = line.strip().split(None, 1)
            if len(fields) < 2 or not fields[0]:
                continue
            name, rest = fields
            if section == "layout":
                layouts.append(name)
            elif section == "variant" and ":" in rest:
                variants.setdefault(rest.split(":", 1)[0], []).append(name)
    answers = [(layout, "") for layout in layouts]
    for layout in layouts:
        answers += [(layout, variant) for variant in variants.get(layout, [])]
    return answers


def notation(layout, variant):
    return f"{layout}({variant})" if variant else layout


def compiled(args, layout, variant, path):
    """The map of `layout(variant)` written at `path`: None, or why not — a
    pair whose first word is `no-symbols` when the layout has no symbols
    file, `refused` for anything else."""
    # NO MODEL: ckbcomp's own (`pc104`, NixOS's `useXkbConfig` default too).
    # The session's is `pc105`, and the maps are the same bytes — all 597
    # compared on xkeyboard-config 2.48 — for three times the time.
    command = [args.ckbcomp, f"-I{args.xkb}", "-rules", "base", "-layout", layout]
    if variant:
        command += ["-variant", variant]
    with open(path, "wb") as out:
        done = subprocess.run(command, stdout=out, stderr=subprocess.PIPE, check=False)
    said = done.stderr.decode("utf-8", "replace").strip()
    if NO_SYMBOLS.search(said):
        return ("no-symbols", said)
    if done.returncode != 0 or NOT_FOUND.search(said):
        return ("refused", said or f"ckbcomp ended with {done.returncode}")
    if os.path.getsize(path) == 0:
        return ("refused", "ckbcomp wrote nothing")
    read = subprocess.run([args.loadkeys, "-b", "-u", path], stdout=subprocess.DEVNULL,
                          stderr=subprocess.PIPE, check=False)
    if read.returncode != 0:
        return ("refused", "loadkeys cannot read it: "
                + read.stderr.decode("utf-8", "replace").strip())
    return None


def first_line(said):
    lines = [line for line in said.splitlines() if not line.startswith("WARNING: Unknown")]
    return (lines or [said])[0]


def one(args, answers):
    found = ANSWER.fullmatch(args.one)
    if not found:
        sys.exit(f"`{args.one}` is no keyboard answer: a layout, its variant in parentheses "
                 "(`br(abnt2)`), as xkb writes them")
    layout, variant = found.group(1), found.group(2) or ""
    if (layout, variant) not in answers:
        what = (f"no variant `{variant}` of `{layout}`" if variant and (layout, "") in answers
                else f"no layout `{layout}`")
        sys.exit(f"`{args.one}` is not a keyboard xkeyboard-config offers: base.lst has {what}. "
                 "ronoctua.keyboard.console is an xkb layout with its variant in parentheses, "
                 "not a kbd keymap's name")
    why = compiled(args, layout, variant, args.out)
    if why is not None:
        sys.exit(f"the console's map of `{args.one}` cannot be made: {first_line(why[1])}")
    print(f"OK: the console's map of `{args.one}`")


def every(args, answers):
    os.makedirs(args.out)
    left_out, refused = [], []
    made = 0
    # Six hundred compilations of a tenth of a second each: as many at once as
    # the build has cores. The results are read in the file's order.
    cores = int(os.environ.get("NIX_BUILD_CORES") or "1") or os.cpu_count() or 1
    paths = [(layout, variant, os.path.join(args.out, notation(layout, variant) + ".map"))
             for layout, variant in answers]
    with concurrent.futures.ThreadPoolExecutor(max_workers=min(cores, 8)) as pool:
        whys = list(pool.map(lambda a: compiled(args, *a), paths))
    for (layout, variant, path), why in zip(paths, whys):
        name = notation(layout, variant)
        if why is None:
            made += 1
            continue
        if os.path.exists(path):
            os.unlink(path)
        if why[0] == "no-symbols":
            left_out.append(f"{name}\t{first_line(why[1])}")
        else:
            refused.append(f"{name}: {first_line(why[1])}")
    if refused:
        print(f"ERROR: {len(refused)} keyboards offered have no console map:", file=sys.stderr)
        for line in refused:
            print("  " + line, file=sys.stderr)
        sys.exit(1)
    if made == 0:
        sys.exit("ERROR: no console map at all: is this base.lst xkeyboard-config's?")
    with open(os.path.join(args.out, "left-out.txt"), "w", encoding="utf-8") as f:
        f.writelines(line + "\n" for line in left_out)
    print(f"OK: {made} console maps; {len(left_out)} answers have no symbols file")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--xkb", required=True)
    parser.add_argument("--ckbcomp", required=True)
    parser.add_argument("--loadkeys", required=True)
    parser.add_argument("--out", required=True)
    which = parser.add_mutually_exclusive_group(required=True)
    which.add_argument("--one")
    which.add_argument("--every", action="store_true")
    args = parser.parse_args()
    answers = offered(os.path.join(args.xkb, "rules", "base.lst"))
    if args.every:
        every(args, answers)
    else:
        one(args, answers)


if __name__ == "__main__":
    main()
