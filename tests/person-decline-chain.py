"""THE SEEDED DECLINE CHAIN, END TO END, WITH THE REAL PROGRAMS.

A `full` installation that leaves programs out says so as the person's
declines, for that hostname alone — and a decline is honoured only where it
reaches the machine as the person's SIGNED data. Each end has its own tests;
this joins them, running the extracted programs the installer, the update
and root's units run (`ronoctua-belongings`, `ronoctua-person`,
`ronoctua-software`) against bare git repositories and folders made here,
offline:

1. THE PERSON'S REPOSITORY, made by the belongings' own `init` on their
   first machine, with a decline of one program for the host about to be
   installed (an earlier attempt's) and one of every machine (the person's
   own). That machine's key is pinned there, as its installer pinned it.
2. THE INSTALLER AND ITS FIRST BOOT, repository not empty — THEIR OWN
   PLANS: the installer's dry run (`ronoctua-install`, terminal-less) prints
   its steps and the first boot's record, and the first boot prints its own
   sequence for that record (`ronoctua-first-boot --plan`); this replays the
   steps the chain is made of, with their paths moved into this folder. The
   installer: the programs left out staged for root's applier and applied
   unsigned (no key pinned yet), and its reading of what was applied
   (`left-out-applied`, which only `--execute` runs: restated below, its
   rule the installer's). The first boot: the choice (two programs left
   out, every other one kept — the earlier attempt's included) signed into
   the repository by `plain --decline`, the same choice again pushing
   nothing, the key pinned, the copy applied by root's applier with that
   pin.
3. THE RESTORE into the new home, the first boot's step: the person's
   mirror says the same.
4. THE UPDATES' PERSON HALF THAT GATHERS, its steps as the update declares
   them (pull, pins, sign, stage — each skipped where the runner would skip
   it), with root's applier started by the stage as its unit is:
   - the first update, no identity kept after the reboot: the sign
     declines, and root keeps what the installer signed — counter unchanged;
   - the person's FIRST machine declines a program on every machine and
     pushes it signed; the next update here, still with no identity, takes
     it — fetched under this machine's pin, staged, applied — and the
     counter grows;
   - a decline the person makes on this machine, signed at the gesture:
     root takes it, and the counter grows again.
5. AN EMPTY REPOSITORY: the declines staged for root's applier, taken
   unsigned (no key pinned yet); then the first boot's sequence for an
   empty repository (`--plan … --empty yes`): the mirror gets the same
   document, `init` signs it into the new repository, the key it made is
   pinned; and the first update's staging, now held to the pin, keeps the
   declines.

OFFLINE, AND THE ADDRESS AS THE PERSON GAVE IT: every repository is reached
over the belongings' own SSH path, through a stand-in `ssh` (`RONOCTUA_SSH`,
the variable `ronoctua-git` reads it from) that serves the bare repositories
here and answers for `example.org` alone. And every person's global git
configuration here REWRITES that address (`insteadOf`, `pushInsteadOf`) to
decoy repositories: the belongings' remote calls read no global or system
configuration, so the decoys must stay empty.

IT RUNS ONLY INSIDE A NIX BUILD. It opens and forgets identities in the
kernel's per-user keyring (`@u`), and a Nix build has its own; anywhere else
that keyring is the person's session's. It refuses to start otherwise.

usage: person-decline-chain.py <steps.json> <belongings> <person> <software> <install> <first-boot>
  steps.json: the update's person half that gathers, in its order —
  [{"id", "program", "run", "enable", "declines", "needs"}, …]
  the installer reads its texts and palettes from `RONOCTUA_TEXTS` and
  `RONOCTUA_THEMES`; `RONOCTUA_MACHINE_DIR` names the machine directory.
"""

import json
import os
import pwd
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

STEPS, BELONGINGS, PERSON, SOFTWARE, INSTALL, FIRST_BOOT = sys.argv[1:7]
PROGRAMS = {
    "ronoctua-belongings": BELONGINGS,
    "ronoctua-person": PERSON,
    "ronoctua-software": SOFTWARE,
}
WORK = Path.cwd() / "decline-chain"
ME = pwd.getpwuid(os.getuid()).pw_name

# THE PERSON'S REPOSITORIES, by the addresses they gave.
HOST = "example.org"
ADDRESS = f"git@{HOST}:me/belongings.git"
EMPTY_ADDRESS = f"git@{HOST}:me/empty.git"
PASSPHRASE = "a passphrase long enough for the belongings"

# THE MACHINES (the installer names them `<user>-…`), and the programs:
# registry ids of the installer's list.
FIRST = "noctua-desk"
NEW = "noctua-laptop"
OTHER_NEW = "noctua-other"
LEFT_OUT = ["blender", "orca-slicer"]
EARLIER = "vscode"
EVERYWHERE = "android-studio"
ELSEWHERE = "gimp"
LATER = "flutter"
AT = "2026-10-20T10:00:00Z"

# THE INSTALLER'S PLACES (`plan.rs`), and where this folder keeps them.
SCRATCH = "/run/ronoctua-install"
WORK_FLAKE = "/tmp/ronoctua-flake"
TARGET_HOME = "/mnt/home/noctua"
RECORD = "/mnt/persist/etc/ronoctua/first-boot/record.json"
# THE FIRST BOOT'S PLACES (`first_boot.rs`).
FIRST_BOOT_RUN = "/run/ronoctua-first-boot"
MACHINE_HOME = "/home/noctua"
MACHINE_LOCK = "/run/ronoctua-person/machine.lock"
OWNER_MARK = "{person-uid}"
# The machine directory, by its one name (`nix/machine.nix`), which the check
# hands over.
MACHINE_DIR = os.environ["RONOCTUA_MACHINE_DIR"]

DECLINED_REL = "plain/person/software/declined.json"
APPLY_START = "start --no-ask-password ronoctua-person-apply.service"


class Failed(Exception):
    pass


def must(ok, what):
    if not ok:
        raise Failed(what)


def inside_a_nix_build():
    """A Nix build: its own folder, its own home that is no home — and so
    its own kernel keyring."""
    top = os.environ.get("NIX_BUILD_TOP")
    return (
        bool(top)
        and os.environ.get("HOME") == "/homeless-shelter"
        and Path.cwd().resolve().is_relative_to(Path(top).resolve())
    )


def run(argv, env=None, ok=True):
    """Runs a program to its end; its exit, stdout and stderr."""
    done = subprocess.run(
        [str(a) for a in argv], env=env, capture_output=True, text=True, check=False
    )
    if ok and done.returncode != 0:
        raise Failed(
            f"{' '.join(map(str, argv))} exited {done.returncode}\n"
            f"--- stdout\n{done.stdout}--- stderr\n{done.stderr}"
        )
    return done


def last_line(text):
    lines = [l for l in text.splitlines() if l.strip()]
    return lines[-1] if lines else ""


def finished(said, what):
    """A queue order's end, as its last line says it: done, no conflict."""
    fields = last_line(said.stdout).split("\t")
    must(fields[1:] == ["done", "0"], f"{what} did not end done: {said.stdout!r}")


def private_file(path, text):
    """A file only its owner reads, as a door writes one."""
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(text)


def private_dir(path):
    path.mkdir(parents=True, exist_ok=True)
    path.chmod(0o700)
    return path


def canonical(kind, doc, at):
    """`doc` in the one form every writer of the person's data writes,
    through root's reader (`ronoctua-person read`): what it refuses is not a
    document of the person's."""
    raw = at.with_suffix(".raw")
    raw.parent.mkdir(parents=True, exist_ok=True)
    raw.write_text(json.dumps(doc))
    said = run([PERSON, "read", kind, raw])
    raw.unlink()
    return said.stdout


def read(kind, path):
    """A person's document, read by root's reader."""
    said = run([PERSON, "read", kind, path], ok=False)
    must(said.returncode == 0, f"{path} does not read as {kind}: {said.stdout}{said.stderr}")
    return json.loads(said.stdout)


def declined_on(doc, host, scoped):
    """The ids declined for `host` — by a decline of every machine too,
    unless `scoped` (`Declining::applies_to`, and `machines.is_some()`)."""
    return {
        d["id"]
        for d in doc["apps"]
        if (d["machines"] is None and not scoped) or host in (d["machines"] or [])
    }


def as_chosen(doc, host, out, kept, what):
    """THE INSTALLER'S READING of what root's applier left, RESTATED: its
    binary runs it (`executor::not_as_chosen`) only inside `--execute`. Every
    program left out declined for this machine, by a decline of it or of
    every machine; no program kept declined for it by a decline OF THIS
    MACHINE — one of every machine is the person's own, which a tick does not
    undo."""
    here = declined_on(doc, host, scoped=False)
    scoped = declined_on(doc, host, scoped=True)
    for i in out:
        must(i in here, f"{what}: {i}, left out, is not declined on {host}: {doc}")
    for i in kept:
        must(i not in scoped, f"{what}: {i}, kept, is still declined on {host}: {doc}")


def declined_everywhere(doc, ids, what):
    """The person's own declines of every machine, still there."""
    every = {d["id"] for d in doc["apps"] if d["machines"] is None}
    for i in ids:
        must(i in every, f"{what}: the person's decline of {i} on every machine is gone: {doc}")


def head_of(remote):
    said = run(["git", "--git-dir", remote, "rev-parse", "--verify", "-q", "main"], ok=False)
    return said.stdout.strip() if said.returncode == 0 else None


def repository_file(remote, rel):
    """A file at the head of a bare repository."""
    said = run(["git", "--git-dir", remote, "show", f"main:{rel}"], ok=False)
    return said.stdout if said.returncode == 0 else None


def bare(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    run(["git", "init", "-q", "--bare", "-b", "main", path])
    return path


def stand_in_ssh(remotes, log):
    """`ssh` as git runs it for the belongings (`-i <key> -o … -F none`, then
    the host and the command): the bare repository under `remotes` the path
    names, served here — for `git@example.org` alone; every call logged."""
    ssh = WORK / "bin" / "ssh"
    ssh.parent.mkdir(parents=True, exist_ok=True)
    ssh.write_text(
        "#!/bin/sh\n"
        "while [ $# -gt 0 ]; do\n"
        "  case \"$1\" in\n"
        "    -[iloFpEbcmJWwDLRSQO]) shift 2 ;;\n"
        "    -*) shift ;;\n"
        "    *) break ;;\n"
        "  esac\n"
        "done\n"
        "host=$1; shift\n"
        f"echo \"$host $*\" >> '{log}'\n"
        f"[ \"$host\" = 'git@{HOST}' ] || {{ echo \"this ssh reaches {HOST} only, not $host\" >&2; exit 255; }}\n"
        "verb=${1%% *}; path=${1#* }; path=${path#\\'}; path=${path%\\'}\n"
        "case \"$verb\" in\n"
        f"  git-upload-pack|git-receive-pack) exec git \"${{verb#git-}}\" '{remotes}'/\"$path\" ;;\n"
        "esac\n"
        "echo \"this ssh serves git and nothing else: $*\" >&2\n"
        "exit 255\n"
    )
    ssh.chmod(0o755)
    return ssh


class World:
    """What every program here runs with: the stand-in `ssh`, and a global
    git configuration that rewrites the person's addresses to decoys."""

    def __init__(self):
        remotes = WORK / "remotes"
        self.remote = bare(remotes / "me" / "belongings.git")
        self.empty = bare(remotes / "me" / "empty.git")
        decoys = WORK / "decoys"
        self.decoys = [bare(decoys / "me" / "belongings.git"), bare(decoys / "me" / "empty.git")]
        self.ssh_log = WORK / "ssh.log"
        self.ssh = stand_in_ssh(remotes, self.ssh_log)
        self.gitconfig = WORK / "gitconfig"
        self.gitconfig.write_text(
            f'[url "file://{decoys}/"]\n'
            f"\tinsteadOf = git@{HOST}:\n"
            f"\tpushInsteadOf = git@{HOST}:\n"
        )

    def env(self, **more):
        env = {
            "PATH": os.environ["PATH"],
            "GIT_CONFIG_GLOBAL": str(self.gitconfig),
            "RONOCTUA_SSH": str(self.ssh),
        }
        env.update({k: str(v) for k, v in more.items()})
        return env

    def kept_off_the_decoys(self):
        """No program followed the person's rewrite: the decoys are empty,
        and every call went to the address's host."""
        for d in self.decoys:
            must(head_of(d) is None, f"a program followed the person's insteadOf to {d}")
        calls = self.ssh_log.read_text().splitlines() if self.ssh_log.exists() else []
        must(len(calls) >= 10, f"only {len(calls)} calls reached the repository's host: {calls}")
        for c in calls:
            must(c.startswith(f"git@{HOST} git-"), f"a call went elsewhere: {c}")


class Home:
    """One person's home: its roots, the repository's settings and access,
    the consent for its host and a runtime folder of its own."""

    def __init__(self, world, name, address):
        self.world = world
        self.root = WORK / name
        self.home = self.root / "home"
        self.config = self.home / ".config"
        self.state = self.home / ".local" / "state"
        self.cache = self.home / ".cache"
        self.runtime = private_dir(self.root / "run")
        self.ours = self.config / "ronoctua"
        self.ours.mkdir(parents=True, exist_ok=True)
        self.state.mkdir(parents=True, exist_ok=True)
        (self.ours / "belongings.json").write_text(json.dumps({"repository": address}) + "\n")
        let_host(self.ours)
        keys(self.ours / "belongings-keys")

    def env(self, **more):
        return self.world.env(
            HOME=self.home,
            XDG_CONFIG_HOME=self.config,
            XDG_STATE_HOME=self.state,
            XDG_CACHE_HOME=self.cache,
            XDG_RUNTIME_DIR=self.runtime,
            **more,
        )

    def mirror(self):
        """The person's declines, in their mirror."""
        return self.state / "ronoctua" / "person" / "software" / "declined.json"

    def door_passphrase(self, runtime=None):
        """The passphrase a door writes for one gesture."""
        at = private_dir(runtime or (self.runtime / "ronoctua-belongings"))
        private_file(at / "passphrase", PASSPHRASE)

    def forget(self):
        """The session's end: this instance's kept identity goes (a Nix
        build's own keyring)."""
        run([BELONGINGS, "forget"], env=self.env())


def let_host(config):
    """The consent the installer's access question gives (`let-host`)."""
    config.mkdir(parents=True, exist_ok=True)
    (config / "consent.json").write_text(json.dumps({"hosts": {HOST: True}}) + "\n")


def keys(folder):
    """An access folder: a deploy key the stand-in `ssh` never reads."""
    private_dir(folder)
    private_file(folder / "deploy", "not a key: the stand-in ssh reads none\n")
    (folder / "known_hosts").write_text("")


class Machine:
    """A machine directory (root's copy of the person's data), root's lock
    and the staging folders the applier's unit reads — under one folder."""

    def __init__(self, name):
        self.dir = WORK / name / "machine"
        self.dir.mkdir(parents=True, exist_ok=True)
        self.dir.chmod(0o755)
        self.sys = WORK / name / "sys"
        self.run = self.sys / "run" / "ronoctua-person"
        self.run.mkdir(parents=True, exist_ok=True)
        self.run.chmod(0o755)
        private_dir(self.run / "staged")
        self.lock = self.run / "machine.lock"
        self.verdict = self.run / "verdict.json"

    def declined(self):
        return self.dir / "person" / "software" / "declined.json"

    def counter(self):
        return read("signer", self.dir / "local" / "signer.json")["counter"]

    def pin(self, key_file, owner=None):
        argv = [PERSON, "pin", "--pin", key_file, "--machine", self.dir, "--lock", self.lock]
        if owner:
            argv += ["--owner", owner]
        said = run(argv)
        must(last_line(said.stdout).startswith("pinned "), f"pin said {said.stdout!r}")
        must(not Path(key_file).exists(), "the pinned key's file was not consumed")

    def apply(self, staged, owner):
        said = run(
            [PERSON, "apply", "--staged", staged, "--machine", self.dir,
             "--owner", owner, "--lock", self.lock],
            ok=False,
        )
        must(said.returncode == 0, f"root's apply refused {staged}: {said.stdout}{said.stderr}")
        return last_line(said.stdout)

    def systemctl(self):
        """`systemctl` as the person's update reaches it: the one start polkit
        lets them make, of the applier's unit — which runs root's applier on
        the folder they staged, writing its verdict (`nix/modules/
        person-apply.nix`). Anything else is a start the step must not
        make."""
        fake = self.sys / "systemctl"
        log = self.sys / "systemctl.log"
        fake.write_text(
            "#!/bin/sh\n"
            f"echo \"$*\" >> '{log}'\n"
            "case \"$*\" in\n"
            f"  '{APPLY_START}')\n"
            f"    exec '{PERSON}' apply --staged '{self.run / 'staged'}' --machine '{self.dir}'"
            f" --owner '{ME}' --verdict '{self.verdict}' --lock '{self.lock}' ;;\n"
            "esac\n"
            "echo \"this check's systemctl starts the applier's unit and nothing else: $*\" >&2\n"
            "exit 1\n"
        )
        fake.chmod(0o755)
        return fake, log

    def session(self, home):
        """The person's session on this machine: its pin is the one their
        programs read (`RONOCTUA_MACHINE_DIR`), and root's folder of the
        staging is this one's."""
        fake, _ = self.systemctl()
        return home.env(
            RONOCTUA_MACHINE_DIR=self.dir,
            RONOCTUA_SOFTWARE_SYSROOT=self.sys,
            RONOCTUA_SYSTEMCTL=fake,
        )


def person_half(steps, home, machine, expect, declined, what):
    """THE UPDATE'S PERSON HALF THAT GATHERS, as the update declares it (its
    steps in their order, each with its program and arguments): a step whose
    file or folder is not there is skipped, as the runner skips it; one that
    fails — that does not end in a refusal it declares as news — fails the
    check. Then what root's applier made of it: ONE start of its unit, and a
    verdict that answers THIS staging (its nonce) with `expect`. `declined`:
    the refusals that must be news this time, and no other."""
    _, log = machine.systemctl()
    log.unlink(missing_ok=True)
    words = set()
    for s in steps:
        if not s["enable"]:
            continue
        env = machine.session(home)
        if not needs_hold(s, env):
            continue
        must(s["program"] in PROGRAMS,
             f"the update runs `{s['id']}` with {s['program']}, which this check does not know")
        said = run([PROGRAMS[s["program"]], *s["run"]], env=env, ok=False)
        last = last_line(said.stdout)
        word = last.removeprefix("refused ") if last.startswith("refused ") else None
        must(said.returncode == 0 or word in s["declines"],
             f"{what}: the update's `{s['id']}` failed: {said.stdout}{said.stderr}")
        if said.returncode != 0:
            words.add(word)
    must(words == set(declined), f"{what}: the half declined {sorted(words)}, not {declined}")
    started = log.read_text().splitlines() if log.exists() else []
    must(started == [APPLY_START], f"{what}: the half started {started}, not the applier once")
    verdict = json.loads(machine.verdict.read_text())
    nonce = (machine.run / "staged" / "nonce").read_text()
    must(verdict.get("nonce") == nonce and verdict.get("outcome") == expect,
         f"{what}: the applier's verdict {verdict} does not answer the staging {nonce} "
         f"with {expect}")


def needs_hold(s, env):
    """Whether a step's preconditions hold here, as the runner asks them: a
    file, a folder — and the network, which this world always has."""
    for need in s["needs"]:
        ((kind, value),) = need.items()
        if kind == "file":
            if not Path(expand(value, env)).is_file():
                return False
        elif kind == "directory":
            if not Path(expand(value, env)).is_dir():
                return False
        elif kind != "network":
            raise Failed(f"the update's `{s['id']}` needs {kind}, which this check does not know")
    return True


def expand(value, env):
    """`$NAME` in a precondition, from the step's environment."""
    return re.sub(r"\$([A-Z_]+)", lambda m: env[m.group(1)], value)


def first_machine(world):
    """1. The person's repository, made by their first machine — with the
    earlier attempt's decline of the new host and their own of every
    machine — and that machine's key pinned there."""
    first = Home(world, FIRST, ADDRESS)
    mirror = first.mirror()
    mirror.parent.mkdir(parents=True, exist_ok=True)
    mirror.write_text(canonical("declined", {
        "format": 1,
        "apps": [
            {"id": EARLIER, "machines": [NEW], "edited-at": AT, "edited-on": NEW},
            {"id": EVERYWHERE, "machines": None, "edited-at": AT, "edited-on": FIRST},
        ],
    }, mirror))
    first.door_passphrase(first.runtime)
    said = run([BELONGINGS, "init", "--home", first.home, "--runtime", first.runtime,
                "--machine", FIRST, "--progress"], env=first.env())
    finished(said, "the first machine's init")
    there = repository_file(world.remote, DECLINED_REL)
    must(there is not None, "the first machine's declines did not reach the repository")
    must(json.loads(there) == json.loads(mirror.read_text()),
         "the repository's declines are not the mirror's")
    must(repository_file(world.remote, "plain/signed.json") is not None,
         "the plain part is not signed")
    machine = Machine(FIRST)
    machine.pin(first.runtime / "signer.pub")
    first.forget()
    print(f"ok 1: the person's repository, signed, with {EARLIER} declined on {NEW} "
          f"and {EVERYWHERE} everywhere")
    return first, machine


def installer_plan(world):
    """The installer's dry run for the new machine: its steps (the command
    each shows, `Step::shown`) and the first boot's record it writes."""
    answers = {
        "RONOCTUA_INST_DISK": "/dev/vda",
        "RONOCTUA_INST_HOSTNAME": NEW,
        "RONOCTUA_INST_PASSWORD": "a password for the chain",
        "RONOCTUA_INST_PASSPHRASE": "a disk passphrase for the chain",
        "RONOCTUA_INST_PROFILE": "full",
        "RONOCTUA_INST_LEFT_OUT": ",".join(LEFT_OUT),
        "RONOCTUA_INST_BELONGINGS": ADDRESS,
        "RONOCTUA_INST_BELONGINGS_ACCESS": "/run/keys/deploy",
        "RONOCTUA_INST_BELONGINGS_PASSPHRASE": PASSPHRASE,
    }
    env = world.env(HOME=WORK / "installer-home", **answers)
    for name in ("RONOCTUA_TEXTS", "RONOCTUA_THEMES"):
        env[name] = os.environ[name]
    said = run([INSTALL], env=env)
    plan = []
    for line in said.stdout.splitlines():
        m = re.match(r"^[! ] (\d+)\. .+? — (.*)$", line)
        if m:
            plan.append(m.group(2))
    must(len(plan) >= 18, f"the dry run printed {len(plan)} steps:\n{said.stdout}")
    head = f"--- {RECORD} ---\n"
    must(head in said.stdout, "the dry run prints no first boot record")
    record, _ = json.JSONDecoder().raw_decode(said.stdout.split(head, 1)[1])
    return plan, record


def shown_words(shown, places):
    """A shown command (`Step::shown`, `Act::shown`) with the places moved:
    its condition, whether the person runs it, its variables, its words."""
    for theirs, ours in places.items():
        shown = shown.replace(theirs, ours)
    only_if = None
    m = re.search(r" \[if (.+)\]$", shown)
    if m:
        only_if, shown = m.group(1), shown[: m.start()]
    as_person = shown.startswith("[")
    if as_person:
        shown = shown.split("] ", 1)[1]
    words = shlex.split(shown)
    env = {}
    while words and re.match(r"^[A-Z_]+=", words[0]):
        k, v = words.pop(0).split("=", 1)
        env[k] = v
    return only_if, as_person, env, words


def first_boot(world, machine, home, record, empty, run_dir):
    """The first boot's own sequence for `record` (`ronoctua-first-boot
    --plan`), replayed with its places moved here: the person's acts with
    their home's environment, root's with root's applier as this user."""
    record_file = run_dir.parent / f"{run_dir.name}-record.json"
    record_file.write_text(json.dumps(record))
    said = run([FIRST_BOOT, "--plan", record_file, "--empty", "yes" if empty else "no"])
    plan = [line for line in said.stdout.splitlines() if line.strip()]
    must(len(plan) >= 3, f"the first boot's plan printed {len(plan)} acts:\n{said.stdout}")
    private_dir(run_dir)
    places = {
        FIRST_BOOT_RUN: str(run_dir),
        MACHINE_DIR: str(machine.dir),
        MACHINE_HOME: str(home.home),
        MACHINE_LOCK: str(run_dir.parent / f"{run_dir.name}.lock"),
        OWNER_MARK: str(os.getuid()),
    }
    ran = {}
    report = None
    for shown in plan:
        only_if, as_person, _, words = shown_words(shown, places)
        program, args = words[0], words[1:]
        verb = args[0] if args else ""
        if only_if and not Path(only_if).exists():
            continue
        if program == "write" and args[0].endswith("passphrase"):
            private_file(Path(args[0]), PASSPHRASE)
        elif program == "write" and args[0].endswith("/choice.json"):
            Path(args[0]).write_text(json.dumps(record["choice"]) + "\n")
        elif program == "write" and args[0].endswith("/declined.json"):
            Path(args[0]).parent.mkdir(parents=True, exist_ok=True)
            Path(args[0]).write_text(canonical("declined", record["declined"], Path(args[0])))
        elif program == "ronoctua-belongings" and verb in ("plain", "restore", "init"):
            must(as_person, f"the first boot runs `{verb}` as root")
            said = run([BELONGINGS, *args], env=home.env(), ok=False)
            must(said.returncode == 0,
                 f"the first boot's `ronoctua-belongings {verb}` failed: {said.stdout}{said.stderr}")
            ran[verb] = args
            if verb == "plain":
                report = json.loads(last_line(said.stdout))
            else:
                finished(said, f"the first boot's {verb}")
        elif program == "ronoctua-person" and verb in ("pin", "apply"):
            must(not as_person, f"the first boot runs `{verb}` as the person")
            args = [ME if a == "root" else a for a in args]
            said = run([PERSON, *args], ok=False)
            must(said.returncode == 0,
                 f"the first boot's `ronoctua-person {verb}` failed: {said.stdout}{said.stderr}")
            ran[verb] = last_line(said.stdout)
        else:
            raise Failed(f"the first boot's plan has an act this chain does not know: {shown}")
    return ran, report


def installed(world, machine, home, scratch, plan, record):
    """The installer's steps the chain is made of, in the plan's order, with
    its places moved here; what an installation alone does (the disk, Nix,
    the hardware) is not this check's. Then its first boot, replayed. A step
    the chain needs that the plans no longer have fails the check."""
    places = {SCRATCH: str(scratch), WORK_FLAKE: str(machine.dir), TARGET_HOME: str(home.home)}
    ran = {}
    for shown in plan:
        only_if, _, env, words = shown_words(shown, places)
        program, args = words[0], words[1:]
        verb = args[0] if args else ""
        if only_if and not Path(only_if).exists():
            continue
        if program == "write" and args[0].endswith("/staged-declines/person/software/declined.json"):
            staged = Path(args[0])
            staged.parent.mkdir(parents=True, exist_ok=True)
            staged.write_text(canonical("declined", record["declined"], staged))
            for d in [staged.parent.parent.parent, staged.parent.parent, staged.parent]:
                d.chmod(0o755)
            ran["staged"] = staged
        elif program == "write" and args[0].endswith("/belongings.json"):
            Path(args[0]).parent.mkdir(parents=True, exist_ok=True)
            Path(args[0]).write_text(json.dumps({"repository": ADDRESS}) + "\n")
        elif program == "install-access":
            keys(Path(args[1]))
        elif program == "let-host":
            let_host(Path(args[0]))
        elif program == "ronoctua-person" and verb == "apply":
            args = [ME if a == "root" else a for a in args]
            said = run([PERSON, *args], ok=False)
            must(said.returncode == 0,
                 f"the installer's `ronoctua-person apply` failed: {said.stdout}{said.stderr}")
            ran["staged-apply"] = last_line(said.stdout)
        elif program == "left-out-applied":
            doc = read("declined", Path(args[0]))
            choice = record["choice"]
            as_chosen(doc, NEW, choice["declined"], [], "left-out-applied")
            ran["left-out-applied"] = doc
    for needed in ("staged", "staged-apply", "left-out-applied"):
        must(needed in ran, f"the installer's plan no longer runs `{needed}` where this chain needs it")
    booted, report = first_boot(world, machine, home, record, False, scratch / "first-boot-run")
    for needed in ("plain", "pin", "apply", "restore"):
        must(needed in booted, f"the first boot's plan no longer runs `{needed}` where this chain needs it")
    ran.update(booted)
    return ran, report


def installer_with_repository(world, steps):
    """2 and 3. The installer, repository not empty — its plan, replayed."""
    machine = Machine(NEW)
    home = Home(world, NEW, ADDRESS)
    scratch = WORK / "installer"
    # WHAT THE INSTALLER'S QUESTIONS MADE before the steps (`reach.rs`): the
    # scratch home with the consent for the repository's host and the
    # access made for this machine.
    let_host(scratch / "home" / ".config" / "ronoctua")
    keys(scratch / "home" / ".config" / "ronoctua" / "belongings-keys")
    private_dir(scratch / "person-run")
    plan, record = installer_plan(world)
    choice = record["choice"]
    must(record["address"] == ADDRESS and record["machine"] == NEW,
         f"the first boot's record does not name the repository and the machine: {record}")
    must(choice["machine"] == NEW and choice["declined"] == LEFT_OUT
         and EARLIER in choice["kept"] and EVERYWHERE in choice["kept"],
         f"the installer's choice is not the one asked: {choice}")
    ran, report = installed(world, machine, home, scratch, plan, record)
    # 2. THE CHOICE, in the repository and on the machine.
    there = json.loads(repository_file(world.remote, DECLINED_REL))
    as_chosen(there, NEW, choice["declined"], choice["kept"], "the repository after plain --decline")
    declined_everywhere(there, [EVERYWHERE], "the repository after plain --decline")
    must(report.get("choice") == "pushed" and not report.get("held"),
         f"the copy did not push the choice, or held something back: {report}")
    must(report.get("kept-everywhere") == [EVERYWHERE],
         f"the copy did not say the person's own decline stays: {report}")
    must(ran["apply"] == "applied", f"root's apply of the first boot's copy said {ran['apply']!r}")
    doc = read("declined", machine.declined())
    must(doc == there, "the machine's declines are not the repository's")
    declined_everywhere(doc, [EVERYWHERE], "left-out-applied")
    counter = machine.counter()
    must(counter > 0, "the pin took no counter")
    # AN INSTALLATION TRIED AGAIN with the same answer pushes nothing: the
    # plan's own copy, again, after the leftovers are cleared.
    head = head_of(world.remote)
    run_dir = scratch / "first-boot-run"
    shutil.rmtree(run_dir / "plain")
    private_file(run_dir / "plain-passphrase", PASSPHRASE)
    (run_dir / "signer.pub").unlink(missing_ok=True)
    again = run([BELONGINGS, *ran["plain"]], env=home.env())
    must(json.loads(last_line(again.stdout)).get("choice") == "already-there",
         f"the same choice again was not found there: {again.stdout}")
    must(head_of(world.remote) == head, "the same choice again moved the repository")
    print(f"ok 2: the first boot's plan signed {', '.join(LEFT_OUT)} out and {EARLIER} back on "
          f"{NEW}; the pinned apply took it (counter {counter}), and the installer's reading said its part")
    # 3. THE RESTORE'S MIRROR.
    mirror = home.mirror()
    must(mirror.is_file(), "the restore brought no declines into the mirror")
    must(mirror.read_bytes() == machine.declined().read_bytes(),
         "the mirror's declines are not, byte for byte, the machine's")
    home.forget()
    print("ok 3: the first boot's restore landed the same declines in the person's mirror")
    return machine, home, choice, counter


def updates(world, steps, machine, home, first, first_machine_, choice, counter):
    """4. The updates' person half here, and the person's first machine."""
    out, kept = choice["declined"], choice["kept"]
    # THE FIRST UPDATE, after the reboot: no identity kept, so the signing
    # declines — and root keeps what the installer signed.
    before = home.mirror().read_bytes()
    person_half(steps, home, machine, "unchanged", ["no-passphrase"], "the first update")
    must(home.mirror().read_bytes() == before, "the first update's pull changed the restore's mirror")
    as_chosen(read("declined", machine.declined()), NEW, out, kept, "after the first update")
    first_update = machine.counter()
    must(first_update == counter,
         f"an unchanged apply moved the counter: {counter} → {first_update}")
    # THE FIRST MACHINE declines a program on every machine, and pushes it
    # signed: its session fetches under its own pin, signs at the gesture.
    session = first_machine_.session(first)
    pulled = run([BELONGINGS, "plain", "pull", "--json"], env=session)
    must(not json.loads(last_line(pulled.stdout)).get("held"),
         f"the first machine held the installer's choice: {pulled.stdout}")
    mirror = first.mirror()
    mine = json.loads(mirror.read_text())
    must(EARLIER not in declined_on(mine, NEW, scoped=True),
         f"the first machine did not take the installer's choice: {mine}")
    mine["apps"].append({"id": ELSEWHERE, "machines": None,
                         "edited-at": "2026-10-21T10:00:00Z", "edited-on": FIRST})
    mirror.write_text(canonical("declined", mine, mirror))
    first.door_passphrase()
    signed = run([BELONGINGS, "sign"], env=session)
    must(last_line(signed.stdout).startswith("signed "),
         f"the first machine's sign said {signed.stdout!r}")
    pushed = run([BELONGINGS, "plain", "push", "--json"], env=session)
    went = json.loads(last_line(pushed.stdout))
    must("state/person/software/declined.json" in went.get("pushed", [])
         and not went.get("unsigned"),
         f"the first machine's decline did not go up signed: {went}")
    first.forget()
    # THE NEXT UPDATE HERE, still with no identity: what the other machine
    # signed arrives, under this machine's pin.
    person_half(steps, home, machine, "applied", ["no-passphrase"], "the other machine's decline")
    must(ELSEWHERE in declined_on(json.loads(home.mirror().read_text()), NEW, scoped=False),
         "the pull did not bring the other machine's decline into the mirror")
    doc = read("declined", machine.declined())
    as_chosen(doc, NEW, out + [ELSEWHERE], kept, "after the other machine's decline")
    declined_everywhere(doc, [EVERYWHERE, ELSEWHERE], "after the other machine's decline")
    elsewhere = machine.counter()
    must(elsewhere > first_update,
         f"the other machine's signed decline did not move the counter: {first_update} → {elsewhere}")
    # A DECLINE THE PERSON MAKES HERE, signed at the gesture (the software
    # centre writes the passphrase where `sign` reads it).
    mirror = home.mirror()
    mine = json.loads(mirror.read_text())
    mine["apps"].append({"id": LATER, "machines": [NEW],
                         "edited-at": "2026-10-22T10:00:00Z", "edited-on": NEW})
    mirror.write_text(canonical("declined", mine, mirror))
    home.door_passphrase()
    person_half(steps, home, machine, "applied", [], "the person's decline")
    doc = read("declined", machine.declined())
    as_chosen(doc, NEW, out + [ELSEWHERE, LATER], [k for k in kept if k != LATER],
              "after the person's decline")
    grown = machine.counter()
    must(grown > elsewhere, f"a newly signed staging did not move the counter: {elsewhere} → {grown}")
    home.forget()
    print(f"ok 4: the first update kept the declines (counter {counter} → {first_update}); "
          f"the other machine's signed decline arrived with no identity ({elsewhere}); "
          f"a decline signed here arrived ({grown})")


def installer_with_empty_repository(world, steps):
    """5. An empty repository: staged unsigned, then signed by `init`."""
    machine = Machine(OTHER_NEW)
    staged = WORK / "installer-empty" / "staged-declines"
    declines = {
        "format": 1,
        "apps": [{"id": i, "machines": [OTHER_NEW], "edited-at": AT, "edited-on": OTHER_NEW}
                 for i in LEFT_OUT],
    }
    doc_path = staged / "person" / "software" / "declined.json"
    text = canonical("declined", declines, doc_path)
    doc_path.write_text(text)
    for d in [staged, staged / "person", staged / "person" / "software"]:
        d.chmod(0o755)
    applied = machine.apply(staged, ME)
    must(applied == "applied", f"root's apply of the staged declines said {applied!r}")
    as_chosen(read("declined", machine.declined()), OTHER_NEW, LEFT_OUT, [],
              "left-out-applied, unsigned")
    home = Home(world, OTHER_NEW, EMPTY_ADDRESS)
    record = {"format": 1, "address": EMPTY_ADDRESS, "machine": OTHER_NEW, "empty": True,
              "checked": False, "choice": None, "declined": declines}
    booted, _ = first_boot(world, machine, home, record, True, WORK / OTHER_NEW / "first-boot-run")
    for needed in ("init", "pin"):
        must(needed in booted, f"the first boot's plan for an empty repository no longer runs `{needed}`")
    must(home.mirror().read_text() == text, "the first boot did not put the declines in the mirror")
    there = repository_file(world.empty, DECLINED_REL)
    must(there is not None and json.loads(there) == json.loads(text),
         f"init did not sign the mirror's declines into the new repository: {there}")
    home.forget()
    person_half(steps, home, machine, "unchanged", ["no-passphrase"],
                "the empty repository's first update")
    as_chosen(read("declined", machine.declined()), OTHER_NEW, LEFT_OUT, [],
              "after init and the pinned apply")
    must(machine.counter() > 0, "the pinned apply took no counter")
    print("ok 5: an empty repository's declines, staged unsigned, signed by init, "
          "kept by the pinned apply")


def main():
    if not inside_a_nix_build():
        raise Failed(
            "this runs only inside a Nix build: it opens and forgets identities in the kernel's "
            "per-user keyring, which anywhere else is your session's. Hand the programs to a "
            "build (`checks.<system>.person-decline-chain`, or one of your own over them)."
        )
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    steps = json.loads(Path(STEPS).read_text())
    world = World()
    first, first_machine_ = first_machine(world)
    machine, home, choice, counter = installer_with_repository(world, steps)
    updates(world, steps, machine, home, first, first_machine_, choice, counter)
    installer_with_empty_repository(world, steps)
    world.kept_off_the_decoys()
    print("ok 6: no program followed the person's insteadOf; every call reached the address's host")
    print("OK: the decline chain holds from the installer's choice to the updates")


if __name__ == "__main__":
    try:
        main()
    except Failed as e:
        print(f"ERROR: {e}", file=sys.stderr)
        sys.exit(1)
