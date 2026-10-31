# THE LOCK SCREEN (2026-10-31): the checks of the two protocols it stands on —
# each the owner's approved exception to invariant 3, each in ONE file — of
# the unit that keeps the session locked whatever becomes of the footer, and
# of the finger's PAM stack; and its VM test.
#
#   - `one-session-lock`: `ext-session-lock-v1` is named by the door's
#     `session_lock.rs` alone, which still asks the lock of the compositor,
#     puts a surface of it on every screen, and opens the session with
#     `unlock` and a round trip AFTER it (a client that hangs up before the
#     compositor read the unlock leaves the session locked, every screen red).
#   - `one-idle-notify`: `ext-idle-notify-v1` is named by the footer's
#     `idle_notify.rs` alone, which asks `get_idle_notification` — the
#     request that honours idle inhibitors, so a video playing keeps the
#     screen unlocked — and never `get_input_idle_notification`, which
#     ignores them.
#   - `the-lock-outlives-the-footer`: the session's `ronoctua-lock` unit, as
#     the machine evaluates it, runs the lock's own door, is started again
#     when it dies (no start limit) and not when it refused to lock (the
#     status both Rust ends write), and is never restarted by a `switch`; and
#     the footer starts THAT unit.
#   - `the-lock-finger-is-the-finger-alone`: the finger's stack the lock asks
#     (`ronoctua_pam::FINGER_SERVICE`) is declared on the machine, and
#     answers with `pam_fprintd` alone before the stack's `pam_deny` — no
#     password, no key, nothing else.
#   - `lock-screen-vm`: the lock taken through the command, a window that
#     hears nothing under it, a wrong password refused with its sentence on
#     the screen, the right one opening the session, and the automatic lock
#     after the minute chosen (`tests/lock-screen-vm.nix`).
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  lib,
  self,
  mkCheck,
  pyPrelude,
  shellPrelude,
  scroll,
  rust,
  fonts,
}:
let
  vm = self.nixosConfigurations.vm.config;
in
{
  one-session-lock = mkCheck "one-session-lock" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # FILE-WIDE: whether ANY other file names the protocol, anywhere in it.
    # `code_of` because the needles are names in CODE; a comment may say what
    # the exception is, and the module's own path (`crate::session_lock`) is
    # ours and no needle.
    crates = pathlib.Path("${../crates}")
    needles = ("ext_session_lock", "ExtSessionLock", "SessionLock", "session_lock::v1", "toolkit::session_lock")
    wrong, looked = [], 0
    for path in sorted(crates.rglob("*.rs")):
        looked += 1
        # THE ONE FILE, in whichever shell crate holds it (`shell_name`).
        if shell_name(path) == "session_lock.rs":
            continue
        code = code_of(path.read_text())
        if any(n in code for n in needles):
            wrong.append(str(path.relative_to(crates)) + " names the session lock")
    floor(looked, 300, "Rust files read")

    door = code_of(shell_file("session_lock.rs").read_text())
    taking = must(body_of(door, "lock"), "session_lock.rs's `lock`")
    if ".lock(" not in taking:
        wrong.append("session_lock.rs no longer asks the compositor for the lock")
    placing = must(body_of(door, "place_the_lock"), "session_lock.rs's `place_the_lock`")
    if placing.count("create_lock_surface") < 2:
        wrong.append("session_lock.rs no longer puts a surface of the lock on the door's screen AND on every other")
    opening = must(body_of(door, "open_the_session"), "session_lock.rs's `open_the_session`")
    unlock, trip = opening.find(".unlock()"), opening.find(".roundtrip()")
    if unlock < 0:
        wrong.append("the session is no longer opened with `unlock`")
    elif trip < unlock:
        wrong.append("the unlock is not followed by a round trip: a process that goes first leaves the lock abandoned")
    for w in wrong:
        print("ERROR: " + w, file=sys.stderr)
    if wrong:
        sys.exit(1)
    print(f"OK: only session_lock.rs names the session lock ({looked} files), and it locks, covers and opens")
    END
  '';

  one-idle-notify = mkCheck "one-idle-notify" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, sys
    exec(open("${pyPrelude}").read())
    exec(open("${shellPrelude}").read())

    # FILE-WIDE: whether ANY other file names the protocol, anywhere in it —
    # the twin of `one-session-lock`. `code_of` because the needles are names
    # in CODE.
    crates = pathlib.Path("${../crates}")
    needles = ("ext_idle_notif", "ExtIdleNotif", "idle_notify::v1")
    wrong, looked = [], 0
    for path in sorted(crates.rglob("*.rs")):
        looked += 1
        code = code_of(path.read_text())
        # NOWHERE, the one file included: it ignores the idle inhibitors, and
        # a video playing would be locked.
        if "get_input_idle_notification" in code:
            wrong.append(str(path.relative_to(crates)) + " asks an idle that ignores the inhibitors")
        if shell_name(path) == "idle_notify.rs":
            continue
        if any(n in code for n in needles):
            wrong.append(str(path.relative_to(crates)) + " names the idle notifications")
    floor(looked, 300, "Rust files read")
    watch = code_of(shell_file("idle_notify.rs").read_text())
    asking = must(body_of(watch, "notification_for"), "idle_notify.rs's `notification_for`")
    if "get_idle_notification(" not in asking:
        wrong.append("idle_notify.rs no longer asks `get_idle_notification`, the idle that honours inhibitors")
    for w in wrong:
        print("ERROR: " + w, file=sys.stderr)
    if wrong:
        sys.exit(1)
    print(f"OK: only idle_notify.rs names the idle notifications ({looked} files), and it honours the inhibitors")
    END
  '';

  the-lock-outlives-the-footer =
    let
      unit = vm.systemd.user.services.ronoctua-lock;
      facts = pkgs.writeText "ronoctua-lock-unit.json" (
        builtins.unsafeDiscardStringContext (
          builtins.toJSON {
            exec = unit.serviceConfig.ExecStart;
            restart = unit.serviceConfig.Restart;
            prevent = toString unit.serviceConfig.RestartPreventExitStatus;
            limit = unit.startLimitIntervalSec;
            restartIfChanged = unit.restartIfChanged;
          }
        )
      );
    in
    mkCheck "the-lock-outlives-the-footer" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, pathlib, re, sys
      exec(open("${pyPrelude}").read())
      exec(open("${shellPrelude}").read())

      # PARSED: the unit, as the machine evaluates it, turned to JSON by Nix.
      unit = json.loads(pathlib.Path("${facts}").read_text())
      # FILE-WIDE: the constants the unit has to agree with, read as the
      # literals they are (`uncommented`): the lock's word and refusal, and
      # the footer's unit and refusal.
      word = re.search(r'pub const LOCK_WORD: &str = "([a-z-]+)";', uncommented(shell_file("session_lock.rs").read_text()))
      lock = re.search(r"pub const REFUSED: i32 = (\d+);", uncommented(shell_file("lock.rs").read_text()))
      footer = uncommented(shell_file("surface/locking.rs").read_text())
      floor(len(footer), 2000, "characters of the footer's locking.rs")
      footer_unit = re.search(r'const UNIT: &str = "([^"]+)";', footer)
      footer_refused = re.search(r"const REFUSED: i32 = (\d+);", footer)
      wrong = []
      if not (word and lock and footer_unit and footer_refused):
          print("ERROR: a constant this check reads went away", file=sys.stderr)
          sys.exit(1)
      if not unit["exec"].endswith("/bin/ronoctua-shell " + word.group(1)):
          wrong.append(f"the unit runs {unit['exec']!r}, not the lock's door `{word.group(1)}`")
      if footer_unit.group(1) != "ronoctua-lock.service":
          wrong.append(f"the footer starts {footer_unit.group(1)}, and the machine declares ronoctua-lock.service")
      if unit["restart"] != "on-failure":
          wrong.append(f"a lock that dies is not started again (Restart={unit['restart']}): the session stays locked with nobody to open it")
      if unit["limit"] != 0:
          wrong.append("a start limit: a few crashes would leave a locked screen nothing answers")
      if not (unit["prevent"] == lock.group(1) == footer_refused.group(1)):
          wrong.append(f"a lock that refused ends with {lock.group(1)}, the footer reads {footer_refused.group(1)}, and the unit spares {unit['prevent']}")
      if unit["restartIfChanged"]:
          wrong.append("a switch restarts the lock: a stop is a lock abandoned, every screen red")
      for w in wrong:
          print("ERROR: " + w, file=sys.stderr)
      if wrong:
          sys.exit(1)
      print("OK: the lock is a unit of its own, started again when it dies, never by a switch")
      END
    '';

  the-lock-finger-is-the-finger-alone =
    let
      texts = pkgs.writeText "ronoctua-lock-pam.json" (
        builtins.unsafeDiscardStringContext (
          builtins.toJSON (
            lib.mapAttrs (_: s: s.text) (
              lib.filterAttrs (n: _: lib.hasPrefix "ronoctua-unlock" n) vm.security.pam.services
            )
          )
        )
      );
    in
    mkCheck "the-lock-finger-is-the-finger-alone" [ pkgs.python3 ] ''
      python3 - <<'END'
      import json, pathlib, re, sys
      exec(open("${pyPrelude}").read())

      # PARSED: the stacks, as the machine evaluates them.
      stacks = json.loads(pathlib.Path("${texts}").read_text())
      # FILE-WIDE: the crate's constant, read as the literal it is.
      lib_rs = uncommented(pathlib.Path("${../crates/ronoctua-pam/src/lib.rs}").read_text())
      floor(len(lib_rs), 1500, "characters of ronoctua-pam's lib.rs")
      named = re.search(r'pub const FINGER_SERVICE: &str = "([^"]+)";', lib_rs)
      if not named:
          sys.exit("ERROR: ronoctua-pam no longer names FINGER_SERVICE")
      name = named.group(1)
      wrong = []
      stack = stacks.get(name)
      if stack is None:
          wrong.append(f"the lock asks `{name}` and the machine declares only {sorted(stacks)}")
      else:
          auth = [l.split("#", 1)[0].split() for l in stack.splitlines() if l.startswith("auth ")]
          module = lambda l: pathlib.PurePath(l[2]).name
          floor(len(auth), 2, f"auth lines in {name}")
          fingers = [l for l in auth if module(l) == "pam_fprintd.so"]
          if len(fingers) != 1 or fingers[0][1] != "sufficient":
              wrong.append(f"{name} does not ask pam_fprintd once, sufficient")
          if module(auth[-1]) != "pam_deny.so" or auth[-1][1] != "required":
              wrong.append(f"{name} does not end its auth stack in `required pam_deny.so`")
          for l in auth:
              if module(l) not in ("pam_fprintd.so", "pam_deny.so"):
                  wrong.append(f"{name} lets `{module(l)}` answer beside the finger")
          if not any(l.startswith("account ") and "pam_unix.so" in l for l in stack.splitlines()):
              wrong.append(f"{name} does not check the account with pam_unix")
      for w in wrong:
          print("ERROR: " + w, file=sys.stderr)
      if wrong:
          sys.exit(1)
      print(f"OK: `{name}` answers with the finger alone")
      END
    '';

  lock-screen-vm = import ./lock-screen-vm.nix {
    inherit pkgs lib fonts;
    inherit scroll;
    ronoctua-shell = rust.shell;
  };
}
