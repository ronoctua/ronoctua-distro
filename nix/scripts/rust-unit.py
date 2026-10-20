#!/usr/bin/env python3
"""One Rust compilation unit, compiled the way cargo would compile it.

    rust-unit.py <lib|script|bin|test-bins|test-run|test-one|clippy|fmt>

Run inside a derivation of `nix/rust/units.nix`, with structured attributes:
the unit's description is `unit` in `$NIX_ATTRS_JSON_FILE`, written there from
`nix/rust/plan.json` (what cargo planned) and the crate's
`[package.metadata.ronoctua]` (what the crate reads). Nothing here resolves
features, dependencies or profiles: the plan already did, with cargo.

What this does is the part of cargo that RUNS: a build script with cargo's
environment and its `cargo::` lines read back, `rustc` with the flags cargo
passes for that unit (`compiler/mod.rs`: profile, LTO, lints, `--check-cfg`,
`--extern`, `-L`), a jobserver with `$NIX_BUILD_CORES` tokens, and the test
harnesses run from the crate's own directory.

# What a unit may read

Every `RONOCTUA_*` a unit reads at compile time is declared by its crate.
After each compilation the dep-info rustc wrote is read (`# env-dep:` lines),
and after each build script its `rerun-if-env-changed` lines: a read of a name
the crate did not declare FAILS the build, saying which. A crate cannot read a
value it was not given and fall back in silence (the trap of a terminal built
without its engine): either the name is declared, and the value is given or
declared absent, or the build stops.
"""

import ctypes
import json
import os
import re
import resource
import shlex
import shutil
import subprocess
import sys
import threading
import time

STARTED = time.monotonic()


def die(message):
    print(f"rust-unit: {message}", file=sys.stderr)
    sys.exit(1)


def load(path):
    with open(path) as f:
        return json.load(f)


# What `setup` reads out of the derivation; module-level so the functions
# below can be tested with values of a test's own (`test_rust_unit.py`).
UNIT = {}
ATTRS = {}
OUT = METRICS = LAYER = None
TOP = TMP = ROOT = None
CORES = 1
PROFILES = {}
FDS, MAKEFLAGS = (), ""
# Every compiler command this unit ran, in order: a third-party unit keeps
# them (`nix-support/rustc.json`), so they can be set beside cargo's.
COMMANDS = []
_index = None
_runs = {}


def setup():
    """The unit's description (structured attributes), its outputs and the
    build's directories and cores — and the jobserver, made once."""
    global UNIT, ATTRS, OUT, METRICS, TOP, CORES, TMP, ROOT, PROFILES, LAYER, FDS, MAKEFLAGS
    path = os.environ.get("NIX_ATTRS_JSON_FILE")
    if not path:
        die("no NIX_ATTRS_JSON_FILE: this runs inside a derivation with __structuredAttrs")
    ATTRS = load(path)
    UNIT = ATTRS["unit"]
    OUT = os.environ["out"]
    METRICS = os.environ.get("metrics")
    TOP = os.environ.get("NIX_BUILD_TOP", os.getcwd())
    CORES = max(1, int(os.environ.get("NIX_BUILD_CORES", "1") or "1"))
    TMP = os.path.join(TOP, "rust-unit")
    os.makedirs(TMP, exist_ok=True)
    ROOT = os.getcwd()  # the unpacked source: the workspace's root
    PROFILES = UNIT.get("profiles", {})
    LAYER = UNIT.get("layer")
    FDS, MAKEFLAGS = jobserver()


def layer_index():
    global _index
    if _index is None:
        if not LAYER:
            die("this unit needs a third-party crate and was given no layer")
        data = load(os.path.join(LAYER, "index.json"))
        _index = data["units"]
        _runs.update(data.get("runs", {}))
    return _index


def layer_index_runs():
    layer_index()
    return _runs


def layer_unit(key):
    entry = layer_index().get(key)
    if entry is None:
        die(
            f"the layer {LAYER} has no unit {key}: the plan and the layer disagree. "
            "Regenerate the plan with `nix run .#rust-plan`."
        )
    return entry


def workspace_output(key):
    path = UNIT.get("workspace", {}).get(key)
    if path is None:
        return None
    with open(os.path.join(path, "nix-support", "unit.json")) as f:
        return path, json.load(f)


# --- the jobserver -------------------------------------------------------------


def jobserver():
    """A GNU make jobserver with one token per core beyond the one every
    process holds, so a codegen-parallel rustc and a `cc`-driving build script
    use the cores Nix gave this build and no more."""
    r, w = os.pipe()
    os.write(w, b"+" * (CORES - 1))
    os.set_inheritable(r, True)
    os.set_inheritable(w, True)
    return (r, w), f"-j --jobserver-fds={r},{w} --jobserver-auth={r},{w}"



def run(argv, env, cwd=None, capture=False, check=True, what=None):
    # The whole command, quoted for a shell: what `cargo -v` prints, so a
    # unit's command line can be read back word for word and set beside
    # cargo's (the third-party units' parity is proved that way).
    print(f"     Running `{shlex.join(argv)}`", file=sys.stderr, flush=True)
    result = subprocess.run(
        argv,
        env=env,
        cwd=cwd or ROOT,
        pass_fds=FDS,
        stdout=subprocess.PIPE if capture else None,
        stderr=None,
        text=capture,
    )
    if check and result.returncode != 0:
        die(f"{what or argv[0]} failed with status {result.returncode}")
    return result


# --- what cargo puts in the environment ---------------------------------------


def base_env():
    env = dict(os.environ)
    # NOTHING OF OURS LEAKS FROM THE DERIVATION: structured attributes export
    # no `RONOCTUA_*`, and this keeps it that way should one appear.
    for k in list(env):
        if k.startswith("RONOCTUA_") or k.startswith("CARGO_"):
            del env[k]
    env["CARGO"] = UNIT["cargo"]
    env["CARGO_MAKEFLAGS"] = MAKEFLAGS
    return env


def package_env(env, crate):
    env["CARGO_MANIFEST_DIR"] = os.path.normpath(os.path.join(ROOT, crate["dir"]))
    env["CARGO_MANIFEST_PATH"] = os.path.normpath(os.path.join(ROOT, crate["dir"], "Cargo.toml"))
    for k, v in crate["pkgEnv"].items():
        env[k] = v
    return env


def declared_env(env):
    for k, v in UNIT.get("env", {}).items():
        env[k] = v
    return env


def envify(name):
    return name.upper().replace("-", "_")


# --- flags ---------------------------------------------------------------------


def lto_args(lto):
    if lto.startswith("run:"):
        name = lto[4:]
        return ["-C", f"lto={name}" if name else "lto"]
    return {
        "off": ["-C", "lto=off", "-C", "embed-bitcode=no"],
        "both": [],
        "bitcode": ["-C", "linker-plugin-lto"],
        "object": ["-C", "embed-bitcode=no"],
    }[lto]


def is_test_mode(unit):
    return unit["mode"] in ("test", "bench")


def meta_of(key):
    import hashlib

    return hashlib.sha256((UNIT["rustc"] + "\0" + key).encode()).hexdigest()[:16]


def crate_name(unit):
    return unit["target"].replace("-", "_")


def check_cfg(crate):
    values = ", ".join(f'"{f}"' for f in crate["features"])
    return ["--check-cfg", "cfg(docsrs,test)", "--check-cfg", f"cfg(feature, values({values}))"]


def rustc_args(unit, key, crate, out_dir, emit, extra_flags=(), harness=True):
    """cargo's `build_base_args`, in its order."""
    p = PROFILES[unit["profile"]]
    # Ours relative to the workspace, as cargo passes a path package's; a
    # crate that is not ours by the absolute path of its copy in the build
    # directory, as cargo passes a registry package's (`srcRoot`).
    src = os.path.normpath(os.path.join(crate.get("srcRoot", ""), crate["dir"], unit["src"]))
    args = ["--crate-name", crate_name(unit), f"--edition={unit['edition']}", src]
    args += ["--error-format=human", "--color=never"]
    if is_test_mode(unit) and harness:
        pass
    else:
        for t in unit["crate_types"] if unit["mode"] not in ("test", "bench") else ["bin"]:
            args += ["--crate-type", t]
    args += [f"--emit={emit}"]
    # cargo's `prefer_dynamic`: a proc-macro links std dynamically, as the
    # compiler that loads it does (a build script, also "for the host", not).
    if "proc-macro" in unit["kind"]:
        args += ["-C", "prefer-dynamic"]
    opt = p["opt_level"]
    if opt != "0":
        args += ["-C", f"opt-level={opt}"]
    if p.get("panic") and p["panic"] != "unwind" and not is_test_mode(unit):
        args += ["-C", f"panic={p['panic']}"]
    args += lto_args(unit["lto"])
    if p.get("codegen_units") is not None:
        args += ["-C", f"codegen-units={p['codegen_units']}"]
    args += crate["lints"]
    debuginfo = p.get("debuginfo")
    if debuginfo not in (0, None, "0", False):
        args += ["-C", f"debuginfo={debuginfo}"]
    da, oc = bool(p.get("debug_assertions")), bool(p.get("overflow_checks"))
    if opt == "0":
        if not da:
            args += ["-C", "debug-assertions=off"]
    elif da:
        args += ["-C", "debug-assertions=on"]
    if oc != da:
        args += ["-C", f"overflow-checks={'on' if oc else 'off'}"]
    if p.get("rpath"):
        args += ["-C", "rpath"]
    if is_test_mode(unit) and harness:
        args += ["--test"]
    for f in unit["features"]:
        args += ["--cfg", f'feature="{f}"']
    args += check_cfg(crate)
    meta = meta_of(key)
    args += ["-C", f"metadata={meta}", "-C", f"extra-filename=-{meta}"]
    args += ["--out-dir", out_dir]
    strip = p.get("strip")
    if strip and strip != "none":
        args += ["-C", f"strip={strip}"]
    if TOP != "/build":
        args += [f"--remap-path-prefix={TOP}=/build"]
    args += list(extra_flags)
    return args, meta


class Deps:
    """A unit's dependencies, resolved: workspace libraries from the units
    that built them, third-party ones from the layer."""

    def __init__(self, unit, farm_dir):
        self.externs = []
        self.native = []
        self.bins = {}
        self.run = None
        os.makedirs(farm_dir, exist_ok=True)
        for path in UNIT.get("farm", []):
            libdir = os.path.join(path, "lib")
            for name in os.listdir(libdir):
                dest = os.path.join(farm_dir, name)
                if not os.path.lexists(dest):
                    os.symlink(os.path.join(libdir, name), dest)
        self.search = ["-L", f"dependency={farm_dir}"]
        if LAYER:
            self.search += ["-L", f"dependency={os.path.join(LAYER, 'deps')}"]
        plan_units = UNIT["units"]
        for dep in unit["deps"]:
            key = dep["key"]
            if dep.get("run"):
                self.run = key
                continue
            target = plan_units.get(key)
            mode = target["mode"] if target else None
            ws = workspace_output(key)
            if mode == "run-custom-build":
                self.run = key
                continue
            if target is not None and "bin" in target["kind"]:
                self.bins[target["target"]] = key
                continue
            if ws is not None:
                path, info = ws
                rlib = os.path.join(path, info["rlib"])
                self.native += info.get("native", [])
            else:
                entry = layer_unit(key)
                files = entry["files"]
                pick = [f for f in files if f.endswith(".so")] or [f for f in files if f.endswith(".rlib")] or files
                if not pick:
                    die(f"the layer holds no library file for {key}")
                rlib = os.path.join(LAYER, pick[0])
                for n in entry.get("native", []):
                    self.native.append(n)
            prefix = "noprelude:" if dep.get("noprelude") else ""
            self.externs += ["--extern", f"{prefix}{dep['extern']}={rlib}"]

    def native_args(self, own=()):
        """`-L` for every search path of every build script below, cargo's
        artifacts (the layer's kept directories, our own `OUT_DIR`s) before
        the system's."""
        seen, inside, outside = set(), [], []
        for n in list(own) + self.native:
            kind = n.get("kind", "native")
            if "layer" in n:
                value = f"{kind}={os.path.join(LAYER, n['layer'])}"
                bucket = inside
            else:
                value = f"{kind}={n['path']}"
                bucket = inside if n.get("artifact") else outside
            if value not in seen:
                seen.add(value)
                bucket.append(value)
        args = []
        for v in inside + outside:
            args += ["-L", v]
        return args


ENV_DEP = re.compile(r"^# env-dep:([^=\s]+)")


def verify_reads(names, where):
    declared = set(UNIT.get("declared", []))
    bad = sorted(n for n in names if n.startswith("RONOCTUA_") and n not in declared)
    if bad:
        die(
            f"{where} reads {', '.join(bad)} at compile time, and {UNIT['crate']['name']} does not declare "
            f"{'it' if len(bad) == 1 else 'them'}. Add the name to `env` (a value from `generated`), "
            "`test-env` (only the tests' build) or `env-absent` (read on purpose with a default) under "
            f"`[package.metadata.ronoctua]` in {UNIT['crate']['dir']}/Cargo.toml."
        )


def read_env_deps(dep_file):
    names = set()
    if os.path.exists(dep_file):
        with open(dep_file, errors="replace") as f:
            for line in f:
                m = ENV_DEP.match(line)
                if m:
                    names.add(m.group(1))
    return names


def lib_emit(unit):
    """cargo's `--emit` for a library: a proc-macro is linked like a program
    (`requires_upstream_objects`), so it emits no metadata of its own."""
    return "dep-info,link" if "proc-macro" in unit["kind"] else "dep-info,metadata,link"


def compile_unit(unit, key, crate, out_dir, emit, env, deps, script, extra=(), driver=None, harness=True):
    os.makedirs(out_dir, exist_ok=True)
    args, meta = rustc_args(unit, key, crate, out_dir, emit, harness=harness)
    args += deps.search + deps.externs
    if "proc-macro" in unit["kind"]:
        # `proc_macro` in the extern prelude, as cargo puts it there.
        args += ["--extern", "proc_macro"]
    if crate.get("capLints"):
        # A package that is not ours is compiled the way cargo compiles a
        # dependency: its warnings are not ours to read, nor to deny.
        args += ["--cap-lints", "allow"]
    own = []
    if script:
        own = script.get("search", [])
    args += deps.native_args(own)
    if script:
        # cargo's `pass_l_flag`: the library's own compilations, or every
        # target of a package that has no library.
        lib_like = "lib" in unit["kind"] or "proc-macro" in unit["kind"] or not UNIT["crate"]["hasLib"]
        if lib_like:
            for name in script.get("libs", []):
                args += ["-l", name]
        for scope, value in script.get("linkArgs", []):
            if link_arg_applies(scope, unit):
                args += ["-C", f"link-arg={value}"]
        for cfg in script.get("cfgs", []):
            args += ["--cfg", cfg]
        for cfg in script.get("checkCfgs", []):
            args += ["--check-cfg", cfg]
        for k, v in script.get("env", {}).items():
            env[k] = v
    args += list(extra)
    program = driver or "rustc"
    COMMANDS.append([program] + args)
    run([program] + args, env, what=f"{program} {crate_name(unit)}")
    dep_file = os.path.join(out_dir, f"{crate_name(unit)}-{meta}.d")
    verify_reads(read_env_deps(dep_file), f"{crate['name']}'s {unit['target']}")
    return meta


def link_arg_applies(scope, unit):
    kind = unit["kind"]
    if scope == "all":
        return True
    if scope == "cdylib":
        return "cdylib" in unit["crate_types"] and unit["mode"] not in ("test", "bench")
    if scope == "bins":
        return "bin" in kind
    if scope.startswith("bin="):
        return "bin" in kind and unit["target"] == scope[4:]
    if scope == "tests":
        return "test" in kind
    if scope == "benches":
        return "bench" in kind
    if scope == "examples":
        return "example" in kind
    return False


def unit_env(crate, unit, out_dir_env=None):
    env = declared_env(package_env(base_env(), crate))
    env["CARGO_CRATE_NAME"] = crate_name(unit)
    if crate.get("primary", True):
        env["CARGO_PRIMARY_PACKAGE"] = "1"
    if "bin" in unit["kind"]:
        env["CARGO_BIN_NAME"] = unit["target"]
    if out_dir_env:
        env["OUT_DIR"] = out_dir_env
    return env


# --- the build script ----------------------------------------------------------


def target_cfgs():
    out = subprocess.run(["rustc", "--print", "cfg"], capture_output=True, text=True, check=True).stdout
    cfg = {}
    for line in out.splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            cfg.setdefault(k, []).append(v.strip('"'))
        else:
            cfg.setdefault(line, [])
    cfg.pop("debug_assertions", None)
    return cfg


def search_entry(value):
    """`rustc-link-search=[KIND=]PATH` as the units carry it."""
    kind, sep, path = value.partition("=")
    if not sep or "/" in kind:
        kind, path = "native", value
    return {"kind": kind, "path": path}


def parse_script_output(text, crate):
    result = {"search": [], "libs": [], "linkArgs": [], "cfgs": [], "checkCfgs": [], "env": {}, "metadata": {}, "reruns": []}
    warnings = []
    errors = []
    for line in text.splitlines():
        if line.startswith("cargo::"):
            key, _, value = line[7:].partition("=")
            new = True
        elif line.startswith("cargo:"):
            key, _, value = line[6:].partition("=")
            new = False
        else:
            continue
        if key == "rustc-link-lib":
            result["libs"].append(value)
        elif key == "rustc-link-search":
            result["search"].append(search_entry(value))
        elif key == "rustc-flags":
            words = value.split()
            i = 0
            while i < len(words):
                w = words[i]
                if w in ("-l", "-L") and i + 1 < len(words):
                    if w == "-l":
                        result["libs"].append(words[i + 1])
                    else:
                        result["search"].append(search_entry(words[i + 1]))
                    i += 2
                    continue
                if w.startswith("-l"):
                    result["libs"].append(w[2:])
                elif w.startswith("-L"):
                    result["search"].append(search_entry(w[2:]))
                i += 1
        elif key == "rustc-cfg":
            result["cfgs"].append(value)
        elif key == "rustc-check-cfg":
            result["checkCfgs"].append(value)
        elif key == "rustc-env":
            k, _, v = value.partition("=")
            result["env"][k] = v
        elif key == "rustc-link-arg":
            result["linkArgs"].append(["all", value])
        elif key == "rustc-cdylib-link-arg":
            result["linkArgs"].append(["cdylib", value])
        elif key == "rustc-link-arg-bins":
            result["linkArgs"].append(["bins", value])
        elif key == "rustc-link-arg-bin":
            name, _, arg = value.partition("=")
            result["linkArgs"].append([f"bin={name}", arg])
        elif key == "rustc-link-arg-tests":
            result["linkArgs"].append(["tests", value])
        elif key == "rustc-link-arg-benches":
            result["linkArgs"].append(["benches", value])
        elif key == "rustc-link-arg-examples":
            result["linkArgs"].append(["examples", value])
        elif key == "warning":
            warnings.append(value)
        elif key == "error":
            errors.append(value)
        elif key == "rerun-if-env-changed":
            result["reruns"].append(value)
        elif key == "rerun-if-changed":
            pass
        elif new and key == "metadata":
            k, _, v = value.partition("=")
            result["metadata"][k] = v
        elif not new:
            result["metadata"][key] = value
        else:
            die(f"{crate['name']}'s build script printed an unknown instruction `cargo::{key}`")
    for w in warnings:
        print(f"warning: {crate['name']}@{crate['version']}: {w}", file=sys.stderr)
    if errors:
        die(f"{crate['name']}'s build script reported: " + "; ".join(errors))
    return result


LAYER_TOKEN = "@LAYER@"


def links_env(crate, keys, runs_known, layer):
    """`DEP_<links>_<KEY>` for each `links` build script run below ours, as
    cargo hands them: the metadata the layer's index recorded, with its
    target directory written as `@LAYER@/native`, put back as THIS unit's
    layer — so `DEP_AWS_LC_RS_*_INCLUDE` names aws-lc's headers here, as it
    does under cargo. A value the layer could not keep is refused, not
    handed over dangling."""
    env = {}
    for key in keys:
        entry = runs_known.get(key)
        if entry is None:
            die(
                f"{crate['name']}: its build script needs the `links` metadata of {key}, which the layer "
                "could not attribute to one run of that package"
            )
        if entry.get("unreachable"):
            build = "the layer's build that the layer" if layer is not None else "the build of the unit that ran that script, which it"
            die(
                f"{crate['name']}: its build script would be given {', '.join(entry['unreachable'])} of {key}'s "
                f"`links` metadata, which names a path of {build} did not keep"
            )
        for k, v in entry["metadata"].items():
            env[f"DEP_{envify(entry['links'])}_{envify(k)}"] = v if layer is None else v.replace(LAYER_TOKEN, layer)
    return env


def links_known(keys):
    """Where the `links` metadata of the build script runs `keys` comes from:
    the units that ran them, when the unit is given their outputs (`runs`,
    third-party crates built unit by unit) — their values already name those
    outputs —, or else the layer's index, whose values name `@LAYER@`."""
    given = UNIT.get("runs")
    if given is None:
        return (layer_index_runs() if keys else {}), LAYER
    known = {}
    for key in keys:
        entry = given.get(key)
        if entry is None:
            die(
                f"{UNIT['crate']['name']}: its build script needs the `links` metadata of {key}, and the unit "
                "was given no unit that ran that script (`runs` in nix/rust/units.nix)"
            )
        said = load(os.path.join(entry["path"], "nix-support", "unit.json")).get("script") or {}
        known[key] = {"links": entry["links"], "metadata": said.get("metadata", {}), "unreachable": said.get("unreachable", [])}
    return known, None


def run_build_script(crate, run_key, out_dir):
    """Compiles `build.rs` and runs it, as cargo does; returns what it said."""
    units = UNIT["units"]
    run_unit = units[run_key]
    compile_keys = [d["key"] for d in run_unit["deps"] if d["key"] in units and "custom-build" in units[d["key"]]["kind"] and units[d["key"]]["mode"] == "build"]
    others = [d["key"] for d in run_unit["deps"] if d["key"] not in compile_keys]
    if len(compile_keys) != 1:
        die(f"{crate['name']}: the build script run has {len(compile_keys)} compilations")
    # THE BUILD SCRIPTS OF THE `links` PACKAGES BELOW: cargo hands each one's
    # metadata to ours as `DEP_<links>_<KEY>`. A third-party one's is in the
    # unit that ran it, or in the layer's index; a workspace crate with
    # `links` would be a unit this builder does not make.
    known, layer = links_known(others)
    dep_env = links_env(crate, others, known, layer)
    compile_key = compile_keys[0]
    script_unit = units[compile_key]
    build_dir = os.path.join(TMP, "build-script")
    env = declared_env(package_env(base_env(), crate))
    # The script's own crate name (`build_script_main` for a `build =
    # "builder/main.rs"`), as cargo names it.
    env["CARGO_CRATE_NAME"] = crate_name(script_unit)
    if crate.get("primary", True):
        env["CARGO_PRIMARY_PACKAGE"] = "1"
    deps = Deps(script_unit, os.path.join(TMP, "farm-build"))
    meta = compile_unit(script_unit, compile_key, crate, build_dir, "dep-info,link", env, deps, None)
    program = os.path.join(build_dir, f"{crate_name(script_unit)}-{meta}")

    os.makedirs(out_dir, exist_ok=True)
    p = PROFILES[run_unit["profile"]]
    env = declared_env(package_env(base_env(), crate))
    for f in run_unit["features"]:
        env[f"CARGO_FEATURE_{envify(f)}"] = "1"
    for k, values in target_cfgs().items():
        env[f"CARGO_CFG_{envify(k)}"] = ",".join(values)
    # `debug_assertions` from the profile, not from `rustc --print cfg` (which
    # asks with none): as cargo sets it.
    if p.get("debug_assertions"):
        env["CARGO_CFG_DEBUG_ASSERTIONS"] = ""
    env["CARGO_CFG_FEATURE"] = ",".join(run_unit["features"])
    if crate.get("links"):
        env["CARGO_MANIFEST_LINKS"] = crate["links"]
    env["OUT_DIR"] = out_dir
    env["TARGET"] = env["HOST"] = UNIT["host"]
    env["NUM_JOBS"] = str(CORES)
    env["OPT_LEVEL"] = p["opt_level"]
    env["DEBUG"] = "true" if p.get("debuginfo") not in (0, None, "0", False) else "false"
    env["PROFILE"] = "release" if p.get("root") == "release" else "debug"
    env["RUSTC"] = "rustc"
    env["RUSTDOC"] = "rustdoc"
    env["CARGO_ENCODED_RUSTFLAGS"] = ""
    env.update(dep_env)
    result = run([program], env, cwd=os.path.join(ROOT, crate["dir"]), capture=True, check=False)
    if result.returncode != 0:
        sys.stderr.write(result.stdout or "")
        die(f"{crate['name']}'s build script failed with status {result.returncode}")
    said = parse_script_output(result.stdout, crate)
    verify_reads(said["reruns"], f"{crate['name']}'s build script")
    # A `-L` into the script's own OUT_DIR is one of cargo's artifacts.
    for entry in said["search"]:
        if entry["path"].startswith(out_dir):
            entry["artifact"] = True
    return said


# --- outputs -------------------------------------------------------------------


def write_json(path, value):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(value, f, sort_keys=True, indent=1)
        f.write("\n")


# WHAT EACH KIND OF UNIT KEEPS, and nothing else: its product. A library keeps
# its rlib (and the build script's `OUT_DIR` and what the units above it read
# of it) — a third-party library the same —, a program its binary, the test
# harnesses theirs, a verdict unit the verdict and the log. Only a third-party
# LAYER holds a `deps/` of compiled crates: a unit that kept one would be a
# second layer nobody reads, paid for in the store at every edit — what
# `leaf-builds-keep-no-artifacts` refuses, for crane's leaves and here for the
# units.
KEEPS = {
    "lib": {"lib", "out-dir", "nix-support"},
    "dep": {"lib", "out-dir", "nix-support"},
    "script": {"out-dir", "nix-support"},
    "bin": {"bin"},
    "test-bins": {"bin", "libexec", "nix-support"},
    "test-run": {"log", "verdict"},
    "test-one": {"log"},
    "clippy": {"verdict"},
    "fmt": {"verdict"},
}
# And what a library's `lib/` holds: the crate, compiled.
LIBRARY_FILES = (".rlib", ".rmeta", ".so")
MEASURED = {"format", "seconds", "peak-kib", "largest-kib", "cores"}


def kept_beyond(mode, out, metrics):
    """What a unit wrote beyond its product, as sentences (empty when it kept
    only that)."""
    if mode not in KEEPS:
        return [f"{mode} is no kind of unit this driver knows what to keep of"]
    wrong = []
    allowed = KEEPS[mode]
    present = sorted(os.listdir(out)) if os.path.isdir(out) else []
    product = ", ".join(sorted(allowed))
    for entry in present:
        if entry not in allowed:
            shown = entry + ("/" if os.path.isdir(os.path.join(out, entry)) else "")
            wrong.append(f"{shown} beside its product ({product})")
    libdir = os.path.join(out, "lib")
    if mode in ("lib", "dep") and os.path.isdir(libdir):
        for entry in sorted(os.listdir(libdir)):
            if not entry.endswith(LIBRARY_FILES) or os.path.isdir(os.path.join(libdir, entry)):
                wrong.append(f"lib/{entry}, which is not the crate compiled")
    if metrics and os.path.isdir(metrics):
        for entry in sorted(os.listdir(metrics)):
            if entry not in MEASURED:
                wrong.append(f"metrics/{entry}, which is no measurement ({', '.join(sorted(MEASURED))})")
    return wrong


def keep_only_the_product(mode):
    wrong = kept_beyond(mode, OUT, METRICS)
    if wrong:
        die(
            f"a {mode} unit keeps its product and nothing else; it wrote " + "; ".join(wrong)
            + ". Only a third-party layer holds a deps/ of compiled crates."
        )


# --- what a unit measures --------------------------------------------------------
#
# WHAT THE BUDGET PAYS FOR, OF THE WHOLE BUILD. A unit is planned at the slot
# of its class (`nix/rust/unit-classes.json`) beside other builds, and the
# slots are charged against a budget taken from `MemAvailable`
# (`sizing_measure`, nix/build-sizing.nix), which counts the clean page cache
# as available. So what a build takes from the budget is the memory no
# reclaim gives back: its anonymous pages and shmem, resident or swapped,
# and the page tables that map them. The rest of its RSS is page cache —
# the compiler's own libraries, the rlibs it maps, the output the linker
# writes —, counted available while the build holds it and shared with
# every build that maps the same files.
#
# And of everything the build runs at once: a unit that links runs `rustc`
# and the linker together, a build script runs `cc`, a test run its
# harnesses. So a thread reads `/proc/<pid>/status` of the builder (this
# driver's parent: in a sandbox, the build's first process) and of every
# process below it, and adds `RssAnon + RssShmem + VmSwap + VmPTE` of each —
# EACH LIVE PROCESS AT THE MOST IT HAS HELD. A compiler that freed what it
# held before it ran its linker gives that back to the kernel when its
# allocator decides to: before the link in one build, during it in the next
# (the shell's fast-access test harness, 337 MiB or 477). Counted at its
# most while it lives, a process is the worst of the two in every build; one
# that ended holds nothing. And the build runs with no transparent huge page
# (`no_huge_pages`): with them, the same unit held more whenever the kernel
# had huge pages free.
#
# A PROCESS IS THE PROGRAM IT RUNS (`image_of`). `rustc` starts the linker
# with a plain `fork`, and until the child's `execve` its status counts
# rustc's memory, copied on write and so held once; read in that instant —
# longer on a busy machine — and kept as the child's most for the rest of
# its life (it becomes `cc`, waiting for the linker), it counted rustc twice:
# the reviewer's link of a real `rustc` 377 to 405 MiB, 481 to 533 with its
# `execve` held 5 ms (2026-10-20), and in one round of five the loop
# `ronoctua-render-png` 1238 MiB, 1043 to 1045 in the others. So a child that
# runs what its parent runs — the same name, executable and layout — has not
# exec'd yet and is not counted (what it writes before its `execve` is lost:
# in a build, a shell's subshell); and a process's most is of the program it
# runs now, so one that exec'd starts again from what it holds.
#
# WHY NOT RSS OR PSS (docs/ARCHITECTURE.md, section "O pico é do build inteiro (M1)").
# The largest child's `ru_maxrss`, all a unit recorded before, saw one
# process, file pages included. The PSS of the tree (`smaps_rollup`) split
# each shared page with whoever else mapped it, so a unit read another
# number beside other builds than alone: the loop `ronoctua-render-png`
# 570 to 747 MiB, the loop shell 1199 to 1593 (2026-10-19). A link maps the
# rlibs it reads, and that is page cache: the loop shell's link adds some
# 665 MiB to nix-daemon's own count of anonymous memory, where its PSS said
# up to 1566 and its RSS 1.3 GiB. What a process holds anonymously is its
# own, read from counters: a unit reads the same number beside others as
# alone (22 units built five times each, docs/ARCHITECTURE.md as above).
#
# A READING IS CHEAP, SO IT IS OFTEN. `status` is counters, not a walk of the
# pages: a reading of a build's processes takes about 0.3 ms, so one every
# `SAMPLE_SECONDS` (10 ms) costs some 3 % of one core, and the wait after a
# reading is longer when it took longer, so that reading stays under
# `SAMPLE_SHARE` (5 %; outside a sandbox the listing is of every process of
# the machine). A link's peak lasts tenths of a second: read every 0.2 s,
# the loop `ronoctua-render-png` peaked 108 MiB lower at the worst phase,
# read every 20 ms, none. What a process held for less than the 10 ms
# between two readings is lost (measured: at most 14 MiB).
#
# And THE CORES it was built with: a peak depends on them (the RSS of the
# footer's release library peaked at 2279 MiB at one core, 1534 at twelve),
# and a class is a slot at one core — so a measurement says its cores, and
# `.#rust-peaks` classes a unit only by one taken at the cores its class
# builds it with.
#
# The output is versioned (`format`): a reader refuses a measurement of
# another format, which is of another quantity — 1 was the largest child's
# RSS, 2 the PSS of the tree.
METRICS_FORMAT = 3
SAMPLE_SECONDS = 0.01
SAMPLE_SHARE = 0.05
# The fields of `/proc/<pid>/status` (kB) a process holds that no reclaim
# gives back.
HELD = ("RssAnon", "RssShmem", "VmSwap", "VmPTE")
# `prctl(2)`: no transparent huge page for this process and every process it
# starts (inherited across `fork` and kept across `execve`, Linux 3.15).
PR_SET_THP_DISABLE = 41
PR_GET_THP_DISABLE = 42
HUGE_PAGES = "as the kernel says"
# The fields of `/proc/<pid>/stat` that say where `execve` laid a program out
# — its code, data and heap, its stack, its arguments and its environment
# (26-28 and 45-51) —: a `fork` copies them with the memory, an `execve` sets
# them anew. Zero where the reader may not see them, absent before Linux 3.5.
LAYOUT = (26, 27, 28, 45, 46, 47, 48, 49, 50, 51)


def no_huge_pages():
    """THE BUILD RUNS WITH NO TRANSPARENT HUGE PAGE. Where the kernel gives
    one to every large anonymous mapping (`always`), a process holds the
    whole 2 MiB of a page it touched a little of, whenever the kernel has
    one free — so the same unit held more in one build than in the next: the
    release `ronoctua-xdg` library 35 MiB without them and 46 to 55 with, and
    in one round of five every unit 10 to 26 MiB more and two of the shell's
    test harnesses 55 % more (2026-10-20). A machine of the distro gives them
    only where a program asks (`madvise`, nixpkgs' kernel), and nothing in a
    build asks: with them off, a unit holds the same on any host, what a
    machine of the distro holds, and compiles in the same time (the footer's
    release library 64.4 to 65.1 s with them, 64.7 without). Where the call
    does not exist, the build runs as the kernel says, and the log says so."""
    global HUGE_PAGES
    try:
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.prctl(PR_SET_THP_DISABLE, 1, 0, 0, 0) == 0 and libc.prctl(PR_GET_THP_DISABLE, 0, 0, 0, 0) == 1:
            HUGE_PAGES = "off"
    except (OSError, AttributeError):
        pass


def parent_of(stat):
    """The parent's pid in a `/proc/<pid>/stat` line: the second field after
    the LAST `)`, since a command's name may hold spaces and parentheses."""
    return int(stat[stat.rindex(")") + 2 :].split()[1])


def started_of(stat):
    """When a process started, in clock ticks since boot, from its
    `/proc/<pid>/stat` line (field 22), or None when the line has none:
    with its pid, which process it is, even after a pid is given again."""
    fields = stat[stat.rindex(")") + 2 :].split()
    return int(fields[19]) if len(fields) > 19 and fields[19].isdigit() else None


def image_of(stat, exe=None):
    """What program a process runs, from its `/proc/<pid>/stat` line and the
    target of its `exe` link (None where it is not readable): its name, its
    executable and where `execve` laid it out (`LAYOUT`, as many of those
    fields as the line has). A `fork` copy has its parent's. An `execve` gives
    another: of another program, or of the same one with arguments or an
    environment of other lengths — a build runs with no address
    randomization (Nix's sandbox), so the same program run with the same
    lengths is laid out the same, and is the same image."""
    fields = stat[stat.rindex(")") + 2 :].split()
    layout = tuple(fields[n - 3] for n in LAYOUT if n - 3 < len(fields))
    return stat[stat.index("(") + 1 : stat.rindex(")")], exe, layout


def build_tree(root, proc="/proc", stats=None):
    """`root` and every live process below it, at any depth, by the parents
    `/proc` says now; and into `stats`, when given, the `stat` line each
    process of the listing had. A process gone between the listing and its
    read is no process of the tree."""
    children = {}
    for entry in os.listdir(proc):
        if not entry.isdigit():
            continue
        try:
            with open(os.path.join(proc, entry, "stat")) as f:
                stat = f.read()
            parent = parent_of(stat)
        except (OSError, ValueError, IndexError):
            continue
        if stats is not None:
            stats[int(entry)] = stat
        children.setdefault(parent, []).append(int(entry))
    found, seen, todo = [root], {root}, [root]
    while todo:
        for child in children.get(todo.pop(), ()):
            if child not in seen:
                seen.add(child)
                found.append(child)
                todo.append(child)
    return found


def memory_kib(pid, proc="/proc"):
    """(KiB, how) of one process: what it holds that no reclaim gives back,
    `HELD` of its status ("held"); from what is readable when that is not —
    a kernel whose status does not split its memory (before 4.5), a
    `/proc` with no status — its whole resident set, `VmRSS` or `statm`'s,
    which is never less ("rss"); 0 for one that ended, or a zombie, whose
    status says no memory ("gone")."""
    base = os.path.join(proc, str(pid))
    fields = {}
    try:
        with open(os.path.join(base, "status")) as f:
            for line in f:
                name, _, rest = line.partition(":")
                if name in HELD or name == "VmRSS":
                    fields[name] = int(rest.split()[0])
    except FileNotFoundError:
        if not os.path.isdir(base):
            return 0, "gone"
        try:
            with open(os.path.join(base, "statm")) as f:
                return int(f.read().split()[1]) * (os.sysconf("SC_PAGE_SIZE") // 1024), "rss"
        except (OSError, ValueError, IndexError):
            return 0, "gone"
    except (OSError, ValueError, IndexError):
        return 0, "gone"
    if "RssAnon" in fields:
        return sum(fields.get(name, 0) for name in HELD), "held"
    if "VmRSS" in fields:
        return fields["VmRSS"], "rss"
    return 0, "gone"


def next_wait(took, period=SAMPLE_SECONDS, share=SAMPLE_SHARE):
    """Seconds to wait after a reading that took `took`: at least `period`,
    and enough that reading is at most `share` of the time."""
    return max(period, took * (1 - share) / share)


class Sampler:
    """The most the builder and everything below it held at once of what
    the budget pays for (`memory_kib`), each live process at the most it
    has held while running the program it runs now, and no child that has
    not exec'd since its `fork`, in KiB, read by a thread of its own at least
    `period` seconds apart and so that reading takes at most `share` of one
    core; the most one process held; and what reading cost."""

    def __init__(self, root, period=SAMPLE_SECONDS, proc="/proc", share=SAMPLE_SHARE):
        self.root, self.period, self.proc, self.share = root, period, proc, share
        self.most_kib = 0
        self.most_processes = 0
        self.largest_kib = 0
        # The most each live process has held, by (pid, when it started),
        # and the program it held it running (`image_of`).
        self.held = {}
        self.images = {}
        self.samples = 0
        self.spent = 0.0
        self.slowest = 0.0
        self.by_rss = 0
        self.copies = 0
        self.wait = period
        self._stop = threading.Event()
        self._thread = None

    def image(self, pid, stat):
        try:
            exe = os.readlink(os.path.join(self.proc, str(pid), "exe"))
        except OSError:
            exe = None
        return image_of(stat, exe)

    def sample(self):
        began = time.monotonic()
        stats = {}
        pids = build_tree(self.root, self.proc, stats)
        images = {pid: self.image(pid, stats[pid]) for pid in pids if pid in stats}
        total = 0
        held, kept = {}, {}
        for pid in pids:
            image = images.get(pid)
            if image is not None and image == images.get(parent_of(stats[pid])):
                # A FORK COPY, not yet exec'd: its status counts its
                # parent's memory, which the parent holds.
                self.copies += 1
                continue
            kib, how = memory_kib(pid, self.proc)
            if how == "gone":
                continue
            key = (pid, started_of(stats[pid]) if pid in stats else None)
            # The most of the program it runs now: an `execve` starts again.
            most = self.held.get(key, 0) if self.images.get(key) == image else 0
            held[key], kept[key] = max(most, kib), image
            total += held[key]
            self.largest_kib = max(self.largest_kib, held[key])
            if how == "rss":
                self.by_rss += 1
        # A process that ended holds nothing: its most is forgotten.
        self.held, self.images = held, kept
        took = time.monotonic() - began
        self.samples += 1
        self.spent += took
        self.slowest = max(self.slowest, took)
        self.wait = next_wait(took, self.period, self.share)
        if total > self.most_kib:
            self.most_kib, self.most_processes = total, len(held)
        return total

    def start(self):
        """The first reading, of the builder and this driver, and then the
        thread. A first reading of nothing — no process of the build is
        readable at all — is no measurement: the build stops, never writing
        a peak it did not take."""
        if self.sample() <= 0:
            die(
                f"no memory read for this build (pid {self.root}) under {self.proc}: a unit "
                "measures its build's peak there, and one it cannot measure is not written"
            )
        self._thread = threading.Thread(target=self._run, name="peak", daemon=True)
        self._thread.start()

    def _run(self):
        while not self._stop.wait(self.wait):
            self.sample()

    def stop(self):
        self._stop.set()
        if self._thread is not None:
            self._thread.join()
            self._thread = None


SAMPLER = None


def builder_pid():
    """The process the build's tree is read from: the builder that runs this
    driver (its parent: in a sandbox, the build's first process), or this
    driver when it has none it can see."""
    parent = os.getppid()
    return parent if parent > 0 else os.getpid()


def start_measuring():
    global SAMPLER
    if METRICS and SAMPLER is None:
        SAMPLER = Sampler(builder_pid())
        SAMPLER.start()


def measurement(sampled_kib, largest_kib, seconds, cores):
    """What `metrics` holds, by file: the build's peak (the tree at its most),
    the most one of its processes held, the seconds and the cores, in format
    `METRICS_FORMAT`."""
    return {
        "format": f"{METRICS_FORMAT}\n",
        "peak-kib": f"{sampled_kib}\n",
        "largest-kib": f"{largest_kib}\n",
        "seconds": f"{seconds:.2f}\n",
        "cores": f"{cores}\n",
    }


def write_metrics():
    if not METRICS:
        return
    sampler = SAMPLER or Sampler(builder_pid())
    sampler.stop()
    rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    files = measurement(sampler.most_kib, sampler.largest_kib, time.monotonic() - STARTED, CORES)
    os.makedirs(METRICS, exist_ok=True)
    for name, text in files.items():
        with open(os.path.join(METRICS, name), "w") as f:
            f.write(text)
    # What the measurement cost, in the log (`nix log`): how the sampling
    # period was chosen, and how it is checked again. The largest child's
    # RSS, file pages included, is said beside it, never measured by.
    print(
        f"rust-unit: peak {sampler.most_kib // 1024} MiB at {CORES} core(s), anonymous, shmem, swap and "
        f"page tables over {sampler.most_processes} process(es) at most, the largest process "
        f"{sampler.largest_kib // 1024} MiB (the largest child's RSS, file pages included, {rss // 1024} MiB); "
        f"{sampler.samples} readings at least {sampler.period} s apart and at most {sampler.share:.0%} of a core "
        f"took {sampler.spent:.3f} s, the slowest {sampler.slowest * 1000:.1f} ms; transparent huge pages "
        f"{HUGE_PAGES}"
        + (f"; {sampler.by_rss} by RSS" if sampler.by_rss else "")
        + (f"; {sampler.copies} reading(s) of a child not yet exec'd, not counted" if sampler.copies else ""),
        file=sys.stderr,
        flush=True,
    )


def out_dir_of(crate):
    """WHERE A PACKAGE'S `OUT_DIR` IS, in every unit that reads it: one path
    under the build directory, the same in each. The files are generated
    code `include!`d by the crate, and a panic inside them names their path —
    a store path there would make every program carry the library unit that
    ran the script (and fail its `disallowedReferences`)."""
    return os.path.join(TOP, "out-dir", crate["name"])


def script_of_package():
    """The package's build script results: run here, or read from the unit
    that ran them, its `OUT_DIR` copied back to where it was written."""
    base = UNIT.get("base")
    if not base:
        return None, None
    info = load(os.path.join(base, "nix-support", "unit.json"))
    script = info.get("script")
    out_dir = None
    if info.get("outDir"):
        out_dir = out_dir_of(UNIT["crate"])
        if not os.path.isdir(out_dir):
            shutil.copytree(os.path.join(base, "out-dir"), out_dir, symlinks=True)
            for dirpath, dirnames, filenames in os.walk(out_dir):
                for name in dirnames + filenames:
                    path = os.path.join(dirpath, name)
                    if not os.path.islink(path):
                        os.chmod(path, 0o755 if os.path.isdir(path) else 0o644)
    return script, out_dir


def scrub(paths):
    """crane's two `remove-references-to` passes: the vendored sources a
    panic message of a dependency names, and the toolchain's, become the
    `eeee…` hash, so a binary's closure is never its build."""
    hashes = set()
    vendor = UNIT.get("vendor")
    if vendor:
        hashes |= set(re.findall(r"/nix/store/([a-z0-9]{32})", vendor))
        for entry in os.listdir(vendor):
            full = os.path.join(vendor, entry)
            if os.path.isdir(full):
                for sub in os.listdir(full):
                    link = os.path.join(full, sub)
                    if os.path.islink(link):
                        hashes |= set(re.findall(r"/nix/store/([a-z0-9]{32})", os.readlink(link)))
    sysroot = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True, text=True, check=True).stdout.strip()
    hashes |= set(re.findall(r"/nix/store/([a-z0-9]{32})", sysroot))
    if not hashes:
        return
    pattern = re.compile(rb"/nix/store/(" + b"|".join(h.encode() for h in sorted(hashes)) + rb")")
    for path in paths:
        with open(path, "rb") as f:
            data = f.read()
        new = pattern.sub(b"/nix/store/" + b"e" * 32, data)
        if new != data:
            os.chmod(path, 0o755)
            with open(path, "wb") as f:
                f.write(new)


# --- the modes -----------------------------------------------------------------


def mode_lib(script_only=False):
    crate = UNIT["crate"]
    key = UNIT["key"]
    units = UNIT["units"]
    unit = units[key] if not script_only else None
    run_key = UNIT.get("runKey")
    script = None
    out_dir = None
    if run_key:
        out_dir = out_dir_of(crate)
        script = run_build_script(crate, run_key, out_dir)
    info = {"crate": crate["name"], "key": key, "script": script, "outDir": bool(run_key), "native": []}
    if not script_only:
        env = unit_env(crate, unit, out_dir)
        deps = Deps(unit, os.path.join(TMP, "farm"))
        libdir = os.path.join(OUT, "lib")
        meta = compile_unit(unit, key, crate, libdir, "dep-info,metadata,link", env, deps, script)
        ext = "so" if "proc-macro" in unit["kind"] else "rlib"
        info["rlib"] = f"lib/lib{crate_name(unit)}-{meta}.{ext}"
        if not os.path.exists(os.path.join(OUT, info["rlib"])):
            die(f"rustc wrote no {info['rlib']}")
        os.remove(os.path.join(libdir, f"{crate_name(unit)}-{meta}.d"))
        native = list(script["search"]) if script else []
        for n in deps.native:
            if n not in native:
                native.append(n)
        info["native"] = native
    if out_dir:
        shutil.copytree(out_dir, os.path.join(OUT, "out-dir"), symlinks=True)
    write_json(os.path.join(OUT, "nix-support", "unit.json"), info)
    write_json(os.path.join(OUT, "nix-support", "build-script.json"), script or {})
    write_metrics()


# --- a crate that is not ours (nix-design §1.4, phase 2) ----------------------


def version_parts(version):
    """`CARGO_PKG_VERSION_{MAJOR,MINOR,PATCH,PRE}` of a semver version; the
    build metadata (`+…`) is in none of them."""
    core = version.split("+", 1)[0]
    core, _, pre = core.partition("-")
    major, minor, patch = (core.split(".") + ["0", "0", "0"])[:3]
    return major, minor, patch, pre


def manifest_features(manifest):
    """The features cargo knows a package by (`--check-cfg`), sorted: the
    `[features]` table and, as cargo adds them, one implicit feature per
    optional dependency no feature names as `dep:<name>`."""
    table = manifest.get("features", {})
    features = set(table)
    named = {v[4:] for values in table.values() for v in values if v.startswith("dep:")}
    for section_owner in [manifest] + list(manifest.get("target", {}).values()):
        for section in ("dependencies", "build-dependencies", "build_dependencies"):
            for name, spec in section_owner.get(section, {}).items():
                if isinstance(spec, dict) and spec.get("optional") and name not in named:
                    features.add(name)
    return sorted(features)


LEVELS = {"forbid": "--forbid", "deny": "--deny", "warn": "--warn", "allow": "--allow"}


def manifest_lints(manifest):
    """cargo's `lints_to_rustflags` over a package's `[lints]`: every tool but
    `cargo`, sorted by priority and then by name, descending, and after them
    the `unexpected_cfgs.check-cfg` list, each with `--check-cfg`."""
    flags, check_cfgs = [], []
    for tool, lints in sorted(manifest.get("lints", {}).items()):
        if tool == "cargo" or not isinstance(lints, dict):
            continue
        for name, config in lints.items():
            level = config if isinstance(config, str) else config.get("level", "warn")
            priority = config.get("priority", 0) if isinstance(config, dict) else 0
            option = f"{LEVELS[level]}={name if tool == 'rust' else f'{tool}::{name}'}"
            flags.append((priority, _Reverse(name), option))
            if tool == "rust" and name == "unexpected_cfgs" and isinstance(config, dict):
                check_cfgs += list(config.get("check-cfg", []))
    flags.sort()
    out = [f for _, _, f in flags]
    for c in check_cfgs:
        out += ["--check-cfg", c]
    return out


class _Reverse:
    """`std::cmp::Reverse` for a string, as cargo sorts lint names."""

    def __init__(self, value):
        self.value = value

    def __lt__(self, other):
        return self.value > other.value

    def __eq__(self, other):
        return self.value == other.value


def dep_crate(spec):
    """What cargo reads off a registry package's own `Cargo.toml` — its
    `CARGO_PKG_*`, the features it declares, its lints — read here, in the
    build, from the crate's source: at evaluation it would be a derivation's
    output read by Nix (import from derivation)."""
    import tomllib

    with open(os.path.join(ROOT, "Cargo.toml"), "rb") as f:
        manifest = tomllib.load(f)
    package = manifest.get("package", {})
    name, version = package.get("name"), package.get("version")
    if (name, version) != (spec["name"], spec["version"]):
        die(f"the source of {spec['name']}@{spec['version']} is the manifest of {name}@{version}")
    text = lambda v: "" if v is None else str(v)
    major, minor, patch, pre = version_parts(version)
    readme = package.get("readme")
    if readme is False:
        readme = ""
    elif readme is True:
        readme = "README.md"
    elif readme is None:
        readme = next((r for r in ("README.md", "README.txt", "README") if os.path.isfile(os.path.join(ROOT, r))), "")
    return {
        "name": name,
        "version": version,
        "dir": ".",
        "srcRoot": ROOT,
        "links": spec.get("links"),
        "features": manifest_features(manifest),
        "lints": manifest_lints(manifest),
        "capLints": True,
        "primary": False,
        "hasLib": True,
        "pkgEnv": {
            "CARGO_PKG_NAME": name,
            "CARGO_PKG_VERSION": version,
            "CARGO_PKG_VERSION_MAJOR": major,
            "CARGO_PKG_VERSION_MINOR": minor,
            "CARGO_PKG_VERSION_PATCH": patch,
            "CARGO_PKG_VERSION_PRE": pre,
            "CARGO_PKG_AUTHORS": ":".join(package.get("authors", [])),
            "CARGO_PKG_DESCRIPTION": text(package.get("description")),
            "CARGO_PKG_HOMEPAGE": text(package.get("homepage")),
            "CARGO_PKG_REPOSITORY": text(package.get("repository")),
            "CARGO_PKG_LICENSE": text(package.get("license")),
            "CARGO_PKG_LICENSE_FILE": text(package.get("license-file")),
            "CARGO_PKG_RUST_VERSION": text(package.get("rust-version")),
            "CARGO_PKG_README": readme,
        },
    }


# What separates the words of a build script's value: blanks, the list
# separators (`a:b`, `a;b`, `a,b`), the `=` of `KEY=path` and quotes.
PATH_EDGES = r"\s:;,=\"'"
WORD_BEFORE = re.compile(r"[^" + PATH_EDGES + r"]*\Z")
PATH_GOES_ON = re.compile(r"[^/" + PATH_EDGES + r"]")
# The text glued to a path, since the last separator: nothing, a flag
# (`-I`, `-L`, `-isystem`, `@`) or a scheme's slashes (`file://`). A `/`
# with a name after it there means the match is inside some other path
# (`/nix/store/h-foo/build/…` does not name `/build`). A relative
# `foo/build/…` reads as `foo` glued to `/build/…`: the safe side, since
# the unreachable test then refuses it instead of handing it over.
GLUED = re.compile(r"[^/]*/*")


def path_spans(value, root):
    """Where `root`, or a path below it, sits inside a value, as (start, end)
    of `root` itself: wherever a path can begin (`GLUED` before it) and
    followed by `/`, a separator or the end — so `/build/x` is under
    `/build`, and `/build2` is not."""
    spans = []
    at = value.find(root)
    while at != -1:
        end = at + len(root)
        word = WORD_BEFORE.search(value, 0, at).group(0)
        if GLUED.fullmatch(word) and not PATH_GOES_ON.match(value, end):
            spans.append((at, end))
            at = value.find(root, end)
        else:
            at = value.find(root, at + 1)
    return spans


def moved_paths(value, moves):
    """`value` with each path under an `old` root rewritten under its `new`
    one, in one pass (a rewritten path is never matched again); at one place
    the longest root wins."""
    found = sorted((start, -(end - start), end, new) for old, new in moves for start, end in path_spans(value, old))
    out, at = [], 0
    for start, _, end, new in found:
        if start < at:
            continue
        out.append(value[at:start])
        out.append(new)
        at = end
    out.append(value[at:])
    return "".join(out)


def relocated(script, moves, top):
    """A build script's results as its DEPENDENTS read them: each path into
    this build's directory moved to where the unit's output keeps it (its
    `OUT_DIR` copied into `$out/out-dir`, its source in the store path it was
    unpacked from) — a bare path, a list of them, a flag's (`-I<path>`) or a
    quoted one alike. A search path or `links` value that still names the
    build directory after the moves names nothing a dependent can see: the
    search path is dropped (a dependent links the rlib, which carries what
    was bundled) and the `links` key is marked unreachable — a dependent's
    build script that would receive it is refused (`links_env`)."""
    out = dict(script)
    search = []
    for entry in script["search"]:
        moved = dict(entry, path=moved_paths(entry["path"], moves))
        if path_spans(moved["path"], top):
            continue
        search.append(moved)
    out["search"] = search
    metadata, unreachable = {}, []
    for k, v in script["metadata"].items():
        metadata[k] = moved_paths(v, moves)
        if path_spans(metadata[k], top):
            unreachable.append(k)
    out["metadata"] = metadata
    out["unreachable"] = sorted(unreachable)
    return out


def mode_dep():
    """A crate that is NOT OURS, unit for unit: its build script compiled and
    run first, when it has one, then the library — what cargo runs for a
    registry package, with cargo's environment and flags for one (`--cap-lints
    allow`, no primary package, its own manifest's `CARGO_PKG_*`)."""
    crate = dep_crate(UNIT["crate"])
    UNIT["crate"] = crate
    key = UNIT["key"]
    unit = UNIT["units"][key]
    run_key = UNIT.get("runKey")
    script = out_dir = None
    if run_key:
        out_dir = out_dir_of(crate)
        script = run_build_script(crate, run_key, out_dir)
    env = unit_env(crate, unit, out_dir)
    deps = Deps(unit, os.path.join(TMP, "farm"))
    libdir = os.path.join(OUT, "lib")
    meta = compile_unit(unit, key, crate, libdir, lib_emit(unit), env, deps, script)
    ext = "so" if "proc-macro" in unit["kind"] else "rlib"
    rlib = f"lib{crate_name(unit)}-{meta}.{ext}"
    if not os.path.exists(os.path.join(libdir, rlib)):
        die(f"rustc wrote no lib/{rlib}")
    # WHAT A DEPENDENT LINKS, and nothing else: the `.rmeta` beside the rlib
    # is the same metadata again (cargo keeps it for pipelining, which units
    # do not do) and the dep-info was read.
    for name in os.listdir(libdir):
        if name != rlib:
            os.remove(os.path.join(libdir, name))
    info = {"crate": crate["name"], "key": key, "rlib": f"lib/{rlib}", "outDir": bool(run_key), "script": None}
    native = []
    if script is not None:
        # The source's store path is the derivation's `src` (structured
        # attributes export none to the environment).
        moves = [(out_dir, os.path.join(OUT, "out-dir"))]
        if ATTRS.get("src"):
            moves.append((ROOT, ATTRS["src"]))
        info["script"] = relocated(script, moves, TOP)
        native = list(info["script"]["search"])
        for entry in native:
            if entry["path"].startswith(os.path.join(OUT, "out-dir")):
                entry["artifact"] = True
        shutil.copytree(out_dir, os.path.join(OUT, "out-dir"), symlinks=True)
    for n in deps.native:
        if n not in native:
            native.append(n)
    info["native"] = native
    write_json(os.path.join(OUT, "nix-support", "unit.json"), info)
    write_json(os.path.join(OUT, "nix-support", "build-script.json"), info["script"] or {})
    # The commands, as cargo's `-v` would print them: what
    # `third-party-units-are-cargo-s` compares, flag for flag.
    write_json(os.path.join(OUT, "nix-support", "rustc.json"), COMMANDS)
    write_metrics()


def mode_bin():
    crate = UNIT["crate"]
    key = UNIT["key"]
    unit = UNIT["units"][key]
    script, out_dir = script_of_package()
    env = unit_env(crate, unit, out_dir)
    deps = Deps(unit, os.path.join(TMP, "farm"))
    build = os.path.join(TMP, "bin")
    meta = compile_unit(unit, key, crate, build, "dep-info,link", env, deps, script)
    os.makedirs(os.path.join(OUT, "bin"), exist_ok=True)
    dest = os.path.join(OUT, "bin", unit["target"])
    shutil.copy2(os.path.join(build, f"{crate_name(unit)}-{meta}"), dest)
    scrub([dest])
    write_metrics()


def target_description(unit):
    if "test" in unit["kind"] or "bench" in unit["kind"]:
        return unit["src"]
    return f"unittests {unit['src']}"


def mode_test_bins():
    crate = UNIT["crate"]
    units = UNIT["units"]
    script, out_dir = script_of_package()
    exe = os.path.join(OUT, "libexec")
    # THE PACKAGE'S OWN PROGRAMS FIRST: an integration test is compiled with
    # `CARGO_BIN_EXE_<name>` naming them, and runs them by that path.
    bin_paths = {}
    for key in UNIT.get("binKeys", []):
        unit = units[key]
        env = unit_env(crate, unit, out_dir)
        deps = Deps(unit, os.path.join(TMP, "farm-" + unit["target"]))
        build = os.path.join(TMP, "bin-" + unit["target"])
        meta = compile_unit(unit, key, crate, build, "dep-info,link", env, deps, script)
        os.makedirs(exe, exist_ok=True)
        dest = os.path.join(exe, unit["target"])
        shutil.copy2(os.path.join(build, f"{crate_name(unit)}-{meta}"), dest)
        bin_paths[unit["target"]] = dest
    tests = []
    bindir = os.path.join(OUT, "bin")
    for key in UNIT["testKeys"]:
        unit = units[key]
        env = unit_env(crate, unit, out_dir)
        if "test" in unit["kind"] or "bench" in unit["kind"]:
            for name, path in bin_paths.items():
                env[f"CARGO_BIN_EXE_{name}"] = path
            env["CARGO_TARGET_TMPDIR"] = os.path.join(TOP, "target-tmp")
        deps = Deps(unit, os.path.join(TMP, "farm-" + key.replace("/", "_")))
        harness = UNIT.get("harness", {}).get(unit["target"], True)
        meta = compile_unit(unit, key, crate, bindir, "dep-info,link", env, deps, script, harness=harness)
        name = f"{crate_name(unit)}-{meta}"
        os.remove(os.path.join(bindir, f"{name}.d"))
        tests.append(
            {
                "binary": name,
                "target": unit["target"],
                "kind": unit["kind"],
                "describe": target_description(unit),
                "harness": harness,
                "id": target_id(unit),
            }
        )
    manifest = {
        "crate": crate["name"],
        "dir": crate["dir"],
        "root": ROOT,
        "tests": tests,
        "bins": sorted(bin_paths),
        # What the build script set with `rustc-env`: cargo hands it to the
        # programs it runs as well as to rustc (`Compilation::extra_env`).
        "scriptEnv": (script or {}).get("env", {}),
    }
    write_json(os.path.join(OUT, "nix-support", "tests.json"), manifest)
    write_metrics()


def target_id(unit):
    if "lib" in unit["kind"]:
        return "lib"
    if "bin" in unit["kind"]:
        return f"bin:{unit['target']}"
    if "test" in unit["kind"]:
        return f"test:{unit['target']}"
    return f"{'+'.join(unit['kind'])}:{unit['target']}"


def run_env(manifest):
    env = dict(os.environ)
    for k in list(env):
        if k.startswith("RONOCTUA_") or k.startswith("CARGO_"):
            del env[k]
    crate = UNIT["crate"]
    package_env(env, crate)
    env["CARGO"] = UNIT["cargo"]
    for k, v in UNIT.get("runEnv", {}).items():
        env[k] = v
    for k, v in manifest.get("scriptEnv", {}).items():
        env[k] = v
    for name in manifest.get("bins", []):
        env[f"CARGO_BIN_EXE_{name}"] = os.path.join(UNIT["testBins"], "libexec", name)
    env["RUST_TEST_THREADS"] = str(CORES)
    # cargo's dynamic search path for what it runs: the sysroot's target
    # libraries, beside what the tests dlopen (libwayland).
    sysroot = subprocess.run(["rustc", "--print", "target-libdir"], capture_output=True, text=True, check=True).stdout.strip()
    libs = [p for p in UNIT.get("libraryPath", []) if p] + [sysroot]
    if env.get("LD_LIBRARY_PATH"):
        libs.append(env["LD_LIBRARY_PATH"])
    env["LD_LIBRARY_PATH"] = ":".join(libs)
    os.makedirs(os.path.join(TOP, "target-tmp"), exist_ok=True)
    return env


def place_sources(manifest):
    """The crate's tests read `CARGO_MANIFEST_DIR` as it was at compile time;
    the sources go back to that same place."""
    if manifest["root"] != ROOT:
        die(
            f"the tests were compiled in {manifest['root']} and run in {ROOT}: a test reading "
            "`env!(\"CARGO_MANIFEST_DIR\")` would miss its fixtures. Both builds need the sandbox's /build."
        )


def runnable(name):
    """A harness, copied where it can run the way cargo runs it: from a
    writable directory of its own (`target/<profile>/deps/`). A test may write
    beside its own executable — the transfer queue's sweep test does — and a
    harness run straight from the store would find that directory
    read-only."""
    deps = os.path.join(TOP, "target", "deps")
    os.makedirs(deps, exist_ok=True)
    dest = os.path.join(deps, name)
    if not os.path.exists(dest):
        shutil.copy2(os.path.join(UNIT["testBins"], "bin", name), dest)
        os.chmod(dest, 0o755)
    return dest


def mode_test_run():
    manifest = load(os.path.join(UNIT["testBins"], "nix-support", "tests.json"))
    place_sources(manifest)
    env = run_env(manifest)
    cwd = os.path.join(ROOT, manifest["dir"])
    failed = []
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "log"), "w") as log:
        for t in manifest["tests"]:
            binary = runnable(t["binary"])
            line = f"     Running {t['describe']} (deps/{t['binary']})"
            print(line, flush=True)
            log.write(line + "\n")
            proc = subprocess.Popen([binary], cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
            for out_line in proc.stdout:
                sys.stdout.write(out_line)
                log.write(out_line)
            proc.wait()
            if proc.returncode != 0:
                failed.append(f"{t['describe']} ({t['binary']}): status {proc.returncode}")
    if failed:
        die("tests failed:\n  " + "\n  ".join(failed))
    with open(os.path.join(OUT, "verdict"), "w") as f:
        f.write(f"ok: {len(manifest['tests'])} test binaries of {manifest['crate']}\n")
    write_metrics()


def mode_test_one():
    manifest = load(os.path.join(UNIT["testBins"], "nix-support", "tests.json"))
    place_sources(manifest)
    want = UNIT["target"]
    chosen = [t for t in manifest["tests"] if t["id"] == want]
    if len(chosen) != 1:
        die(f"{manifest['crate']} has no test binary `{want}` (it has: {', '.join(t['id'] for t in manifest['tests'])})")
    t = chosen[0]
    env = run_env(manifest)
    for k, v in UNIT.get("extraEnv", {}).items():
        env[k] = v
    argv = [runnable(t["binary"])]
    exact = UNIT.get("exact")
    if exact:
        argv += list(exact) + ["--exact"]
    if UNIT.get("includeIgnored"):
        argv += ["--include-ignored"]
    print(f"     Running {t['describe']} (deps/{t['binary']}) {' '.join(argv[1:])}", flush=True)
    proc = subprocess.run(argv, cwd=os.path.join(ROOT, manifest["dir"]), env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
    sys.stdout.write(proc.stdout)
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "log"), "w") as f:
        f.write(proc.stdout)
    if proc.returncode != 0:
        die(f"{t['describe']} failed with status {proc.returncode}")
    expect = UNIT.get("expect")
    if expect and expect not in proc.stdout:
        die(
            f"the run did not say `{expect}`: the filter ran other tests than the ones asked for, or "
            "none — a green build that tested something else"
        )


def mode_clippy():
    crate = UNIT["crate"]
    script, out_dir = script_of_package()
    targets = UNIT["targets"]
    env_extra = {"CLIPPY_ARGS": "--deny__CLIPPY_HACKERY__warnings__CLIPPY_HACKERY__"}
    for i, t in enumerate(targets):
        unit = {
            "kind": t["kind"],
            "crate_types": t["crate_types"],
            "target": t["target"],
            "src": t["src"],
            "edition": t["edition"],
            "mode": "test" if t["test"] else "check",
            "profile": UNIT["profile"],
            "lto": "object",
            "features": t["features"],
            "deps": t["deps"],
        }
        is_script = "custom-build" in t["kind"]
        env = unit_env(crate, unit, None if is_script else out_dir)
        env.update(env_extra)
        if is_script:
            env["CARGO_CRATE_NAME"] = "build_script_build"
            unit["profile"] = UNIT["scriptProfile"]
        if "test" in t["kind"] or "bench" in t["kind"]:
            for name in UNIT.get("bins", []):
                env[f"CARGO_BIN_EXE_{name}"] = os.path.join(TOP, "bins", name)
            env["CARGO_TARGET_TMPDIR"] = os.path.join(TOP, "target-tmp")
        deps = Deps(unit, os.path.join(TMP, f"farm-{i}"))
        key = f"clippy/{crate['name']}/{t['target']}/{t['test']}"
        harness = UNIT.get("harness", {}).get(t["target"], True)
        compile_unit(unit, key, crate, os.path.join(TMP, f"check-{i}"), "dep-info,metadata", env, deps, None if is_script else script, driver="clippy-driver", harness=harness)
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "verdict"), "w") as f:
        f.write(f"ok: {len(targets)} targets of {crate['name']} pass clippy -D warnings\n")
    write_metrics()


def mode_fmt():
    crate = UNIT["crate"]
    failed = []
    for root in UNIT["roots"]:
        path = os.path.join(crate["dir"], root["src"])
        result = subprocess.run(["rustfmt", "--check", "--edition", root["edition"], path], cwd=ROOT)
        if result.returncode != 0:
            failed.append(path)
    if failed:
        die(f"out of format (run `nix fmt`): {', '.join(failed)}")
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "verdict"), "w") as f:
        f.write(f"ok: {len(UNIT['roots'])} roots of {crate['name']} are formatted\n")
    write_metrics()


def main():
    setup()
    no_huge_pages()
    start_measuring()
    mode = sys.argv[1] if len(sys.argv) > 1 else UNIT.get("mode")
    {
        "lib": mode_lib,
        "script": lambda: mode_lib(script_only=True),
        "dep": mode_dep,
        "bin": mode_bin,
        "test-bins": mode_test_bins,
        "test-run": mode_test_run,
        "test-one": mode_test_one,
        "clippy": mode_clippy,
        "fmt": mode_fmt,
    }.get(mode, lambda: die(f"unknown mode {mode}"))()
    keep_only_the_product(mode)


if __name__ == "__main__":
    main()
