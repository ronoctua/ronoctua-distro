//! The verbs end to end, on a system root of their own: a store with two or
//! three toplevels, a system profile, `/run/booted-system`, and `nix`,
//! `nix-env` and `bootctl` that are small shell programs keeping a log of
//! what they were asked. They behave as the real ones do in the ways the
//! verbs depend on: `nix-env --set` makes no generation for the newest one
//! again, `bootctl list` names each generation's entry with its `init=`, and
//! `set-oneshot ""` unsets the variable. A reboot ([`World::reboot`]) is a
//! new boot identity, the booted and running links moved, and the one-shot
//! spent — what the boot loader does with it.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ronoctua_person::Stamp;
use ronoctua_person::state::{
    ApplyResult, Hold, LastGood, MachineState, NoTrial, Trial, TrialOutcome,
};

use crate::system::{self, Env};
use crate::{Machine, Places, System, Why, disk};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A folder of a test's own, removed when the test ends.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new(what: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-machine-{what}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("run")).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(dir.join("run"), std::fs::Permissions::from_mode(0o755)).unwrap();
        Self(dir)
    }

    pub(crate) fn path(&self, inside: &str) -> PathBuf {
        if inside.is_empty() {
            self.0.clone()
        } else {
            self.0.join(inside)
        }
    }

    /// The system under this root, with the fake programs.
    pub(crate) fn system(&self) -> System {
        let root = self.0.display().to_string();
        let nix = self.path("bin/nix").display().to_string();
        let bootctl = self.path("bin/bootctl").display().to_string();
        let template = self.path("template").display().to_string();
        System::from_env(&Env::of(&[
            (system::SYSROOT, &root),
            (system::MACHINE_ATTR, "noctua"),
            (system::NIX, &nix),
            (system::BOOTCTL, &bootctl),
            (system::MENU_LIMIT, "5"),
            (system::TEMPLATE, &template),
            (system::SYSTEM_LOCK, SYSTEM_LOCK),
        ]))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Where a toplevel carries its lock, as the machine module puts it.
const SYSTEM_LOCK: &str = "etc/ronoctua/machine.lock";

const LOCK_A: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua"}},"ronoctua":{"locked":{"rev":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}},"root":"root","version":7}
"#;
const LOCK_B: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua"}},"ronoctua":{"locked":{"rev":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}},"root":"root","version":7}
"#;

/// A machine on its own root: system A booted and running from generation
/// 1, the machine directory locked at A's revision.
struct World {
    s: Scratch,
    m: Machine,
}

/// A stand-in program, WRITTEN BY A CHILD `/bin/sh` and never by this
/// process: a file this process holds open for writing while another test's
/// thread forks is inherited by that fork until it execs, and running the
/// file in that window fails with `ETXTBSY` ("Text file busy").
fn script(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
            "sh",
        ])
        .arg(path)
        .arg(text)
        .status()
        .unwrap();
    assert!(status.success(), "{}", path.display());
}

impl World {
    fn new(what: &str) -> Self {
        let s = Scratch::new(what);
        let root = s.path("").display().to_string();
        script(
            &s.path("bin/nix"),
            &format!(
                r#"#!/bin/sh
root='{root}'
echo "nix $*" >> "$root/calls"
case "$1" in
  build)
    out=""; dry=0; prev=""; last=""
    for a in "$@"; do
      [ "$prev" = "--out-link" ] && out="$a"
      [ "$a" = "--dry-run" ] && dry=1
      prev="$a"; last="$a"
    done
    echo "$last" >> "$root/built"
    if [ -e "$root/build-fails" ]; then echo "error: builder failed" >&2; exit 1; fi
    if [ "$dry" = 1 ]; then cat "$root/dry-says" >&2; exit 0; fi
    ln -sfn "$(cat "$root/build-result")" "$out" ;;
  flake)
    last=""; for a in "$@"; do last="$a"; done
    d="${{last#path:}}"
    case "$2" in
      lock) if [ -e "$root/lock-fails" ]; then exit 1; fi; cp "$root/new-lock" "$d/flake.lock" ;;
      update) cp "$root/updated-lock" "$d/flake.lock" ;;
    esac ;;
esac
"#
            ),
        );
        script(
            &s.path("bin/nix-env"),
            &format!(
                r#"#!/bin/sh
root='{root}'
echo "nix-env $*" >> "$root/calls"
p="$2"; real="$root$4"; dir="${{p%/*}}"
last=0
for l in "$dir"/system-*-link; do
  [ -L "$l" ] || continue
  n="${{l##*/system-}}"; n="${{n%-link}}"
  [ "$n" -gt "$last" ] && last="$n"
done
if [ "$last" -gt 0 ] && [ "$(readlink -f "$dir/system-$last-link")" = "$(readlink -f "$real")" ]; then exit 0; fi
n=$((last + 1))
ln -s "$real" "$dir/system-$n-link"
ln -sfn "system-$n-link" "$p"
"#
            ),
        );
        script(
            &s.path("bin/bootctl"),
            &format!(
                r#"#!/bin/sh
root='{root}'
echo "bootctl $*" >> "$root/calls"
if [ "$1" = "--no-pager" ] && [ "$2" = "list" ]; then
  if [ -e "$root/no-efi" ]; then echo "Not booted with EFI" >&2; exit 1; fi
  printf '['; sep=''
  for l in "$root"/nix/var/nix/profiles/system-*-link; do
    n="${{l##*/system-}}"; n="${{n%-link}}"
    t="$(readlink -f "$l")"; t="${{t#"$root"}}"
    printf '%s{{"type":"type1","id":"nixos-generation-%s.conf","options":"init=%s/init quiet"}}' "$sep" "$n" "$t"
    sep=','
  done
  printf ']\n'
  exit 0
fi
if [ -e "$root/no-efi" ]; then echo "Failed to write LoaderEntry" >&2; exit 1; fi
case "$1" in
  set-default) printf '%s' "$2" > "$root/efi-default" ;;
  set-oneshot) printf '%s' "$2" > "$root/efi-oneshot" ;;
  *) exit 2 ;;
esac
"#
            ),
        );
        std::fs::create_dir_all(s.path("nix/store")).unwrap();
        std::fs::write(s.path("nix/store/k1"), "kernel").unwrap();
        std::fs::write(s.path("nix/store/i1"), "initrd").unwrap();
        std::fs::create_dir_all(s.path("nix/var/nix/profiles")).unwrap();
        let w = Self {
            m: Machine {
                places: Places {
                    dir: s.path("machine"),
                    state: s.path("state"),
                    run: s.path("run/ronoctua-update"),
                },
                system: s.system(),
            },
            s,
        };
        let a = w.toplevel("a", "quiet");
        w.generation(1, &a);
        w.link("run/booted-system", &a);
        w.link("run/current-system", &a);
        w.boot_as(1);
        std::fs::create_dir_all(w.m.places.dir.clone()).unwrap();
        std::fs::write(w.m.places.dir.join("flake.nix"), "{ }\n").unwrap();
        std::fs::write(w.m.places.dir.join("host.nix"), "{ ... }: { }\n").unwrap();
        std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_A).unwrap();
        w
    }

    /// A toplevel in the store: the shared kernel and initrd, these kernel
    /// parameters, and a switch that logs and exits as `switch-exit` says.
    fn toplevel(&self, name: &str, params: &str) -> String {
        let hashed = format!("{:0>32}-nixos-system-{name}", name.len());
        let top = self.s.path(&format!("nix/store/{hashed}"));
        std::fs::create_dir_all(top.join("etc/ronoctua")).unwrap();
        std::os::unix::fs::symlink(self.s.path("nix/store/k1"), top.join("kernel")).unwrap();
        std::os::unix::fs::symlink(self.s.path("nix/store/i1"), top.join("initrd")).unwrap();
        std::fs::write(top.join("kernel-params"), params).unwrap();
        let root = self.s.path("").display().to_string();
        script(
            &top.join("bin/switch-to-configuration"),
            &format!(
                "#!/bin/sh\necho \"{name} $1\" >> '{root}/switched'\nexit \"$(cat '{root}/switch-exit' 2>/dev/null || echo 0)\"\n"
            ),
        );
        std::fs::write(
            top.join("etc/ronoctua/person-applied.json"),
            format!(
                r#"{{"format":1,"machine":"noctua","distro-rev":"rev-{name}","applied":[],"skipped":[],
"declined":{{"honoured":[],"refused":[],"unknown":[],"still-present":[]}},"brain":{{"included":[],"awaiting-approval":[]}}}}"#
            ),
        )
        .unwrap();
        format!("/nix/store/{hashed}")
    }

    /// A toplevel that carries `lock`, the lock it was built from.
    fn toplevel_with_lock(&self, name: &str, params: &str, lock: &str) -> String {
        let top = self.toplevel(name, params);
        std::fs::write(self.real(&top).join(SYSTEM_LOCK), lock).unwrap();
        top
    }

    fn real(&self, logical: &str) -> PathBuf {
        self.m.system.real(logical)
    }

    /// This boot is the `n`th.
    fn boot_as(&self, n: u32) {
        let id = self.s.path("proc/sys/kernel/random/boot_id");
        std::fs::create_dir_all(id.parent().unwrap()).unwrap();
        std::fs::write(id, format!("boot-{n}\n")).unwrap();
    }

    /// The machine restarts into `into`: a new boot, the booted and running
    /// systems are it, and the boot loader spent the one-shot.
    fn reboot(&self, into: &str) {
        let now = self.read("proc/sys/kernel/random/boot_id");
        let n: u32 = now.trim().trim_start_matches("boot-").parse().unwrap();
        self.boot_as(n + 1);
        self.link("run/booted-system", into);
        self.link("run/current-system", into);
        let _ = std::fs::remove_file(self.s.path("efi-oneshot"));
    }

    fn link(&self, at: &str, logical: &str) {
        let at = self.s.path(at);
        let _ = std::fs::remove_file(&at);
        std::os::unix::fs::symlink(self.real(logical), at).unwrap();
    }

    fn generation(&self, n: u64, logical: &str) {
        let dir = self.s.path("nix/var/nix/profiles");
        std::os::unix::fs::symlink(self.real(logical), dir.join(format!("system-{n}-link")))
            .unwrap();
        let profile = dir.join("system");
        let _ = std::fs::remove_file(&profile);
        std::os::unix::fs::symlink(format!("system-{n}-link"), profile).unwrap();
    }

    fn builds(&self, logical: &str) {
        std::fs::write(
            self.s.path("build-result"),
            self.real(logical).display().to_string(),
        )
        .unwrap();
    }

    fn profile(&self) -> Option<String> {
        self.m.system.resolve(system::PROFILE)
    }

    fn read(&self, inside: &str) -> String {
        std::fs::read_to_string(self.s.path(inside)).unwrap_or_default()
    }

    fn state(&self) -> MachineState {
        self.m.state().unwrap()
    }

    fn lock(&self) -> String {
        std::fs::read_to_string(self.m.places.dir.join("flake.lock")).unwrap()
    }

    fn set_state(&self, state: &MachineState) {
        disk::write_state(&self.m.places, state).unwrap();
    }

    fn outcome(&self) -> Option<TrialOutcome> {
        self.state().trial.map(|t| t.outcome)
    }

    fn snapshot(&self, name: &str) -> String {
        std::fs::read_to_string(self.m.places.state.join(name)).unwrap_or_default()
    }
}

fn last_good(toplevel: &str, entry: &str) -> LastGood {
    LastGood {
        toplevel: toplevel.to_owned(),
        entry: entry.to_owned(),
        distro_rev: "rev".to_owned(),
        blessed_at: Stamp::parse("2026-10-01T00:00:00Z").unwrap(),
    }
}

/// FAILS IF: an apply does not build, set the profile, switch and arm the
/// trial against the system that booted — default on the last good entry,
/// one-shot on the new one — or does not keep the lock it built from.
#[test]
fn an_apply_switches_and_arms_the_trial() {
    let w = World::new("arms");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    let applied = w.m.apply(false).unwrap();
    assert_eq!(applied.result, ApplyResult::Switched);
    assert_eq!(
        applied.trial,
        Some(Trial {
            toplevel: b.clone(),
            outcome: TrialOutcome::Pending
        })
    );
    assert_eq!(w.profile().as_deref(), Some(b.as_str()));
    assert_eq!(w.read("switched"), "b switch\n");
    assert_eq!(w.read("efi-default"), "nixos-generation-1.conf");
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    let state = w.state();
    let good = state.last_good.unwrap();
    assert_eq!(
        (good.toplevel.as_str(), good.entry.as_str()),
        (a.as_str(), "nixos-generation-1.conf")
    );
    assert_eq!(
        good.distro_rev, "rev-a",
        "read off the toplevel's own report"
    );
    assert_eq!(state.last_apply.unwrap().result, ApplyResult::Switched);
    assert_eq!(
        std::fs::read_to_string(w.m.places.state.join(crate::APPLIED_LOCK)).unwrap(),
        LOCK_A
    );
    assert!(
        w.read("built").contains("path:"),
        "the folder is built as a path: flake"
    );
    assert!(
        !w.m.places.run.join("next").exists(),
        "the out-link does not outlive the apply"
    );
    let armed = w.snapshot(crate::boots::ARMED);
    assert!(
        armed.contains("boot-1") && armed.contains(&b),
        "the boot that armed it is written down: {armed}"
    );
}

/// FAILS IF: a build that fails leaves the moved lock in the machine
/// directory, touches the profile or switches anything
/// (`apply-reverts-the-lock-on-build-failure`).
#[test]
fn a_failed_build_puts_the_running_lock_back() {
    let w = World::new("reverts");
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    std::fs::write(w.s.path("build-fails"), "").unwrap();
    let before = w.profile();
    assert_eq!(w.m.apply(false).unwrap_err().why, Why::BuildFailed);
    assert_eq!(w.lock(), LOCK_A, "the lock is the running system's again");
    assert_eq!(w.profile(), before);
    assert_eq!(w.read("switched"), "");
    assert_eq!(
        w.state().last_apply.unwrap().result,
        ApplyResult::BuildFailed
    );
}

/// FAILS IF: a system that boots like the last good one waits for a reboot
/// to be trusted, or leaves a trial armed.
#[test]
fn a_system_that_boots_alike_is_blessed_at_once() {
    let w = World::new("alike");
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    std::fs::write(w.s.path("efi-oneshot"), "nixos-generation-9.conf").unwrap();
    let applied = w.m.apply(false).unwrap();
    assert_eq!(applied.trial.unwrap().outcome, TrialOutcome::BootIdentical);
    assert_eq!(w.state().last_good.unwrap().toplevel, b);
    assert_eq!(w.read("efi-oneshot"), "", "a trial still armed is disarmed");
    assert_eq!(w.read("efi-default"), "");
}

/// FAILS IF: the trial that booted is not made the last good one, or the EFI
/// default stays on the old entry (it would shadow every later rollback).
#[test]
fn a_trial_that_booted_is_blessed() {
    let w = World::new("blessed");
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    w.m.apply(false).unwrap();
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    let state = w.state();
    assert_eq!(state.trial.unwrap().outcome, TrialOutcome::Good);
    let good = state.last_good.unwrap();
    assert_eq!(
        (good.toplevel.as_str(), good.entry.as_str()),
        (b.as_str(), "nixos-generation-2.conf")
    );
    assert_eq!(w.read("efi-default"), "");
    assert_eq!(
        std::fs::read_to_string(w.m.places.state.join(crate::LAST_GOOD_LOCK)).unwrap(),
        LOCK_A
    );
    assert!(
        w.snapshot(crate::boots::ARMED).is_empty(),
        "a judged trial leaves no record of its boots"
    );
}

/// FAILS IF: after a trial that did not boot (the machine came back on the
/// last good one) the trial is not `failed`, the profile does not go back,
/// the menu is not written for what booted, or the lock of the failed
/// system stays in the machine directory.
#[test]
fn a_trial_that_did_not_boot_goes_back() {
    let w = World::new("fails");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::LAST_GOOD_LOCK), LOCK_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    w.m.apply(false).unwrap();
    assert_eq!(w.profile().as_deref(), Some(b.as_str()));
    // The one-shot was spent on b, which did not come up; a booted again.
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    let state = w.state();
    assert_eq!(state.trial.unwrap().outcome, TrialOutcome::Failed);
    assert_eq!(state.last_good.unwrap().toplevel, a);
    assert_eq!(
        w.profile().as_deref(),
        Some(a.as_str()),
        "the profile is back"
    );
    assert!(
        w.read("switched").ends_with("a boot\n"),
        "{}",
        w.read("switched")
    );
    assert_eq!(w.read("efi-default"), "");
    assert_eq!(w.lock(), LOCK_A, "the folder is the running system's again");
}

/// FAILS IF: an apply makes the generation that pushes the last good one out
/// of the boot menu.
#[test]
fn the_menu_keeps_the_last_good_one() {
    let w = World::new("menu");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let c = w.toplevel("c", "quiet c");
    w.generation(2, &c);
    let state = MachineState {
        last_good: Some(last_good(&a, "nixos-generation-1.conf")),
        ..MachineState::default()
    };
    w.set_state(&state);
    let mut m = w.m.clone();
    m.system.menu_limit = Some("2".to_owned());
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    std::fs::create_dir_all(&m.places.state).unwrap();
    std::fs::write(m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
    std::fs::write(m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    assert_eq!(m.apply(false).unwrap_err().why, Why::LastGoodWouldFallOut);
    assert_eq!(
        w.profile().as_deref(),
        Some(c.as_str()),
        "no generation was made"
    );
    assert_eq!(w.lock(), LOCK_A);
    assert_eq!(w.state().last_apply.unwrap().result, ApplyResult::Refused);
    m.system.menu_limit = Some("3".to_owned());
    assert!(m.apply(false).is_ok(), "a menu of three has room");
}

/// FAILS IF: a held machine builds a lock that names another revision, or a
/// hold is taken on a revision the machine does not run.
#[test]
fn a_hold_is_on_the_running_revision_and_holds() {
    let w = World::new("hold");
    assert_eq!(w.m.hold("bbbbbbb").unwrap_err().why, Why::Held);
    w.m.hold("aaaaaaa").unwrap();
    let hold = w.state().hold.unwrap();
    assert_eq!(hold.rev, "a".repeat(40), "the whole revision is kept");
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    assert_eq!(w.m.apply(false).unwrap_err().why, Why::Held);
    assert_eq!(w.read("built"), "", "nothing was built");
    w.m.unhold().unwrap();
    assert_eq!(w.state().hold, None);
    assert!(w.m.apply(false).is_ok());
    // Held while the lock had moved: the lock goes back to what runs.
    let mut state = w.state();
    state.hold = None;
    w.set_state(&state);
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    w.m.hold("aaaaaaaaaa").unwrap();
    assert_eq!(w.lock(), LOCK_A);
    let _ = Hold {
        rev: String::new(),
        since: Stamp::from_unix_s(0),
    };
}

/// FAILS IF: a machine whose EFI variables cannot be written fails its
/// apply, or claims a trial it could not arm.
#[test]
fn no_efi_variables_is_no_trial_and_still_an_apply() {
    let w = World::new("no-efi");
    std::fs::write(w.s.path("no-efi"), "").unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    let applied = w.m.apply(false).unwrap();
    assert_eq!(
        (applied.result, applied.trial),
        (ApplyResult::Switched, None)
    );
    assert_eq!(w.profile().as_deref(), Some(b.as_str()));
    assert_eq!(w.state().trial, None);
    assert_eq!(
        w.state().no_trial,
        Some(NoTrial::NoEfiVariables),
        "the record says why there is no way back"
    );
    // An apply that switches nothing leaves the reason where it is.
    w.link("run/current-system", &b);
    w.m.apply(false).unwrap();
    assert!(!w.read("switched").contains("b switch\nb switch"));
    assert_eq!(w.state().no_trial, Some(NoTrial::NoEfiVariables));
    // The system booted and lasted: the time it had no way back is over.
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    assert_eq!(w.state().no_trial, None);
}

/// FAILS IF: an apply of what already runs makes a generation or switches.
#[test]
fn applying_what_runs_switches_nothing() {
    let w = World::new("same");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    w.builds(&a);
    let applied = w.m.apply(false).unwrap();
    assert_eq!(applied.result, ApplyResult::Switched);
    assert_eq!(w.read("switched"), "");
    assert!(!w.read("calls").contains("nix-env"));
}

/// FAILS IF: a switch that exits 4 is called a failure, or one that exits 1
/// is not — and the trial is armed in both, since the profile moved.
#[test]
fn a_switch_is_judged_by_its_exit() {
    for (exit, result) in [
        ("4", Ok(ApplyResult::SwitchedWithFailures)),
        ("1", Err(Why::SwitchFailed)),
    ] {
        let w = World::new("switch-exit");
        std::fs::write(w.s.path("switch-exit"), exit).unwrap();
        let b = w.toplevel("b", "quiet b");
        w.builds(&b);
        assert_eq!(
            w.m.apply(false).map(|a| a.result).map_err(|r| r.why),
            result
        );
        assert_eq!(w.state().trial.unwrap().outcome, TrialOutcome::Pending);
    }
}

/// FAILS IF: a rehearsal changes the machine directory, the profile or the
/// record, or does not read what the dry run said.
#[test]
fn a_rehearsal_measures_and_changes_nothing() {
    let w = World::new("rehearse");
    std::fs::write(w.s.path("updated-lock"), LOCK_B).unwrap();
    std::fs::write(
        w.s.path("dry-says"),
        "these 2 derivations will be built:\n  /nix/store/x.drv\nthese 3 paths will be fetched (1.00 MiB download, 2.00 MiB unpacked):\n",
    )
    .unwrap();
    let applied = w.m.apply(true).unwrap();
    assert_eq!(applied.result, ApplyResult::Rehearsed);
    let r = applied.rehearsal.unwrap();
    assert_eq!((r.builds, r.download_bytes), (2, 1_048_576));
    assert_eq!(w.lock(), LOCK_A, "the update moved the copy's lock only");
    assert!(
        w.read("calls").contains("flake update"),
        "the input was moved on the copy"
    );
    assert!(!w.m.places.state_file().exists(), "no record was written");
    assert!(
        !w.m.places.run.join("rehearsal").exists(),
        "the copy is gone"
    );
    assert!(!w.read("calls").contains("nix-env"));
}

/// FAILS IF: a verb on the real system runs for somebody who is not root.
#[test]
fn the_real_system_is_roots() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    let s = Scratch::new("not-root");
    let m = Machine {
        places: Places {
            dir: s.path("machine"),
            state: s.path("state"),
            run: s.path("run"),
        },
        system: System::from_env(&Env::default()),
    };
    assert_eq!(m.apply(false).unwrap_err().why, Why::NotRoot);
    assert_eq!(m.bless().unwrap_err().why, Why::NotRoot);
    assert_eq!(m.unhold().unwrap_err().why, Why::NotRoot);
    assert_eq!(m.migrate().unwrap_err().why, Why::NotRoot);
    assert_eq!(m.disarm().unwrap_err().why, Why::NotRoot);
}

/// The old layout: the distro's tree with the machine's own files in it.
fn old_layout(w: &World) {
    let dir = &w.m.places.dir;
    std::fs::create_dir_all(dir.join("nix/modules")).unwrap();
    std::fs::write(dir.join("nix/modules/base.nix"), "{ }\n").unwrap();
    std::fs::write(dir.join("flake.nix"), "{ old = true; }\n").unwrap();
    std::fs::write(
        dir.join("host.nix"),
        "{ ... }: { networking.hostName = \"x\"; }\n",
    )
    .unwrap();
    std::fs::write(dir.join("facter.json"), "{\"version\":2}\n").unwrap();
    std::fs::create_dir_all(dir.join("secrets")).unwrap();
    std::fs::write(dir.join("secrets/secrets.yaml"), "sops: x\n").unwrap();
    std::fs::set_permissions(
        dir.join("secrets/secrets.yaml"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("person/software")).unwrap();
    std::fs::write(
        dir.join("person/software/declared.json"),
        "{\"format\":1,\"apps\":[]}\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("local")).unwrap();
    std::fs::write(dir.join("local/held.json"), "{\"format\":1,\"held\":[]}\n").unwrap();
    std::fs::create_dir_all(w.s.path("template")).unwrap();
    std::fs::write(
        w.s.path("template/flake.nix"),
        "{\n  inputs.ronoctua.url = \"github:ronoctua/ronoctua-distro/release\";\n}\n",
    )
    .unwrap();
    std::fs::write(w.s.path("new-lock"), LOCK_B).unwrap();
    // The new flake's build: the fake links it to a system of the store.
    let built = w.toplevel("migrated", "quiet");
    w.builds(&built);
}

fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir).unwrap().display().to_string(),
                    std::fs::read(&p).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}

/// FAILS IF: a migration that builds does not leave the template's flake,
/// the machine's own files byte for byte (the secrets moved to the root of
/// the flake) and the new lock — or loses the old layout, or keeps more
/// than one old layout aside (`migration-keeps-the-machine`).
#[test]
fn a_migration_keeps_the_machine() {
    let w = World::new("migrate");
    old_layout(&w);
    let before = tree(&w.m.places.dir);
    std::fs::create_dir_all(w.m.places.state.join("legacy-20200101T000000Z")).unwrap();
    let person_inode = std::fs::metadata(w.m.places.dir.join("person")).unwrap();
    let migrated = w.m.migrate().unwrap();
    let dir = &w.m.places.dir;
    let names: Vec<_> = disk::names(dir).unwrap();
    assert_eq!(
        names,
        [
            "facter.json",
            "flake.lock",
            "flake.nix",
            "host.nix",
            "local",
            "person",
            "secrets.yaml"
        ],
        "the machine's own flake and nothing of the distro"
    );
    // The person's data never moved: the same folder, the same bytes.
    use std::os::unix::fs::MetadataExt as _;
    assert_eq!(
        std::fs::metadata(dir.join("person")).unwrap().ino(),
        person_inode.ino()
    );
    assert_eq!(
        std::fs::read(dir.join("local/held.json")).unwrap(),
        b"{\"format\":1,\"held\":[]}\n"
    );
    assert!(
        w.read("calls").contains(".next"),
        "the new flake was built with the person's data beside it"
    );
    assert_eq!(
        std::fs::read(dir.join("host.nix")).unwrap(),
        b"{ ... }: { networking.hostName = \"x\"; }\n"
    );
    assert_eq!(
        std::fs::read(dir.join("facter.json")).unwrap(),
        b"{\"version\":2}\n"
    );
    assert_eq!(
        std::fs::read(dir.join("secrets.yaml")).unwrap(),
        b"sops: x\n"
    );
    let mode = std::fs::metadata(dir.join("secrets.yaml"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600, "a secret stays private");
    assert!(
        std::fs::read_to_string(dir.join("flake.nix"))
            .unwrap()
            .contains("ronoctua-distro/release")
    );
    assert_eq!(tree(&migrated.legacy), before, "the old layout, whole");
    let legacies = disk::names(&w.m.places.state)
        .unwrap()
        .into_iter()
        .filter(|n| n.to_string_lossy().starts_with("legacy-"))
        .count();
    assert_eq!(legacies, 1, "one old layout is kept");
    assert_eq!(
        w.snapshot(crate::APPLIED_LOCK),
        LOCK_B,
        "the lock the migration built is the one the folder goes back to"
    );
    assert!(
        w.read("built").contains(".next#nixosConfigurations.noctua"),
        "the NEW flake was built"
    );
    assert_eq!(w.m.migrate().unwrap_err().why, Why::NothingToMigrate);
}

/// FAILS IF: a migration that does not lock or does not build changes
/// anything of the old layout, or leaves its attempt behind.
#[test]
fn a_migration_that_does_not_build_changes_nothing() {
    for fails in ["build-fails", "lock-fails"] {
        let w = World::new("migrate-fails");
        old_layout(&w);
        std::fs::write(w.s.path(fails), "").unwrap();
        let before = tree(&w.m.places.dir);
        let names_before = disk::names(&w.m.places.dir).unwrap();
        assert_eq!(w.m.migrate().unwrap_err().why, Why::BuildFailed, "{fails}");
        assert_eq!(tree(&w.m.places.dir), before, "{fails}");
        assert_eq!(
            disk::names(&w.m.places.dir).unwrap(),
            names_before,
            "no .next left"
        );
        assert!(!w.m.places.state.exists() || disk::names(&w.m.places.state).unwrap().is_empty());
    }
}

/// FAILS IF: the distro input of a migrated machine cannot be pointed
/// elsewhere (the VM test's local distro, a machine adopted from a
/// checkout).
#[test]
fn a_migration_can_name_another_distro() {
    let w = World::new("migrate-url");
    old_layout(&w);
    let World { s: _kept, mut m } = w;
    m.system.distro = Some("path:/nix/store/abc-source".to_owned());
    m.migrate().unwrap();
    let flake = std::fs::read_to_string(m.places.dir.join("flake.nix")).unwrap();
    assert!(
        flake.contains("inputs.ronoctua.url = \"path:/nix/store/abc-source\";"),
        "{flake}"
    );
}

/// FAILS IF: a boot with no trial waiting does not make what booted the last
/// good one.
#[test]
fn a_boot_with_no_trial_blesses_what_booted() {
    let w = World::new("plain-boot");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    let good = w.state().last_good.unwrap();
    assert_eq!(
        (good.toplevel, good.entry),
        (a, "nixos-generation-1.conf".to_owned())
    );
}

/// FAILS IF: a bless in the very boot that armed the trial — the first
/// switch that brings the bless unit starts it — judges the trial (and puts
/// the machine back on a system it was never asked to leave).
#[test]
fn a_bless_in_the_boot_that_armed_the_trial_judges_nothing() {
    let w = World::new("same-boot");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Pending);
    assert_eq!(w.outcome(), Some(TrialOutcome::Pending));
    assert_eq!(
        w.profile().as_deref(),
        Some(b.as_str()),
        "nothing went back"
    );
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    assert!(
        w.snapshot(crate::boots::TRIED).is_empty(),
        "the arming boot is no try"
    );

    // With no record of the arming boot (lost, or written by a version that
    // kept none): a running system that IS the trial, another one booted.
    let w = World::new("same-boot-unrecorded");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    std::fs::remove_file(w.m.places.state.join(crate::boots::ARMED)).unwrap();
    w.link("run/current-system", &b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Pending);
}

/// FAILS IF: the trial is judged by the clock or by the boot the bless runs
/// in, rather than by the first boot after the arming one — an apply that
/// arms nothing in the trial's boot, or a bless refused as busy there, then
/// turns a system that booted well into a failed one at the next boot.
#[test]
fn a_trial_is_judged_by_the_boot_that_tried_it() {
    // An apply in the trial's boot that finds nothing to switch: it judges
    // the trial an earlier boot armed, and the bless after it agrees.
    let w = World::new("noop-apply");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    w.reboot(&b);
    let applied = w.m.apply(false).unwrap();
    assert_eq!(applied.trial.map(|t| t.outcome), Some(TrialOutcome::Good));
    assert_eq!(w.state().last_good.unwrap().toplevel, b);
    assert_eq!(w.read("efi-default"), "", "the old default is gone");
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    assert_eq!(w.outcome(), Some(TrialOutcome::Good));

    // A bless refused as busy in the trial's boot: what booted was written
    // down before the wait, and the next boot — on the EFI default, the old
    // system — is not the one that judges.
    let w = World::new("busy-bless");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    w.reboot(&b);
    let mut m = w.m.clone();
    m.system.bless_wait = Some("0".to_owned());
    {
        let _update = ronoctua_person::root::Lock::take(
            &m.system.person_lock(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(m.bless().unwrap_err().why, Why::Busy);
    }
    assert_eq!(w.outcome(), Some(TrialOutcome::Pending));
    assert!(w.snapshot(crate::boots::TRIED).contains(&b));
    // The default still names a: the next boot comes up on it. (A wait of a
    // few seconds: a child another test forks may hold a copy of the lock
    // until it execs.)
    w.reboot(&a);
    m.system.bless_wait = Some("5".to_owned());
    assert_eq!(m.bless().unwrap(), TrialOutcome::Good);
    let state = w.state();
    assert_eq!(state.last_good.unwrap().toplevel, b);
    assert_eq!(
        w.profile().as_deref(),
        Some(b.as_str()),
        "nothing was rolled back"
    );
    assert_eq!(w.read("efi-default"), "", "the next boot is the trial's");
    assert!(
        !w.read("switched").contains("a boot"),
        "{}",
        w.read("switched")
    );

    // And a trial whose boot never lasted to the bless is judged by the
    // first one that did.
    let w = World::new("crashed-boot");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    w.reboot(&b);
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
}

/// FAILS IF: a switch that did not return 0 makes the new system the last
/// good one at once because it boots like it — it must be tried by a boot.
#[test]
fn only_a_clean_switch_is_blessed_at_once() {
    for exit in ["2", "4"] {
        let w = World::new("unclean-alike");
        let a = w.m.system.resolve(system::BOOTED).unwrap();
        std::fs::write(w.s.path("switch-exit"), exit).unwrap();
        let b = w.toplevel("b", "quiet");
        w.builds(&b);
        let _ = w.m.apply(false);
        let state = w.state();
        assert_eq!(
            state.trial.map(|t| t.outcome),
            Some(TrialOutcome::Pending),
            "exit {exit}"
        );
        assert_eq!(state.last_good.unwrap().toplevel, a, "exit {exit}");
        assert_eq!(w.read("efi-default"), "nixos-generation-1.conf");
        assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    }
}

/// FAILS IF: the folder's lock is not put back when no snapshot was ever
/// taken — the first apply after an installation, the first trial that fails
/// — because the systems carry the lock they were built from; or a migrated
/// machine (whose system carries none) loses its way back on the first
/// trial.
#[test]
fn the_lock_goes_back_without_any_snapshot() {
    // Installed: a carries LOCK_A, nothing is kept beside it, the update
    // moved the folder to B, and the build fails.
    let w = World::new("installed-fails");
    let a = w.toplevel_with_lock("a2", "quiet", LOCK_A);
    w.generation(2, &a);
    w.reboot(&a);
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    std::fs::write(w.s.path("build-fails"), "").unwrap();
    assert_eq!(w.m.apply(false).unwrap_err().why, Why::BuildFailed);
    assert_eq!(w.lock(), LOCK_A);

    // Installed, the first trial ever fails.
    let w = World::new("installed-trial");
    let a = w.toplevel_with_lock("a2", "quiet", LOCK_A);
    w.generation(2, &a);
    w.reboot(&a);
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let b = w.toplevel_with_lock("b", "quiet b", LOCK_B);
    w.builds(&b);
    w.m.apply(false).unwrap();
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    assert_eq!(w.lock(), LOCK_A, "the folder is the running system's again");
    assert_eq!(w.snapshot(crate::APPLIED_LOCK), LOCK_A);

    // Migrated: the system carries nothing; the migration kept the lock it
    // built, the update moved the folder, the first trial fails.
    let w = World::new("migrated-trial");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    assert_eq!(
        w.snapshot(crate::LAST_GOOD_LOCK),
        LOCK_A,
        "the way back's lock"
    );
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    assert_eq!(w.lock(), LOCK_A);
}

/// FAILS IF: after a trial that did not boot, the next apply of the same
/// system switches into it again — or a newer one is refused too.
#[test]
fn the_system_that_did_not_boot_is_not_tried_again() {
    let w = World::new("not-again");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    // The update moves the input to the same revision again.
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let switched = w.read("switched");
    assert_eq!(w.m.apply(false).unwrap_err().why, Why::DidNotBoot);
    assert_eq!(w.read("switched"), switched, "nothing was switched");
    assert_eq!(w.profile().as_deref(), Some(a.as_str()));
    assert_eq!(w.lock(), LOCK_A, "the folder is put back");
    assert_eq!(w.outcome(), Some(TrialOutcome::Failed));
    assert_eq!(w.state().last_apply.unwrap().result, ApplyResult::Refused);
    // A newer revision builds another system, and that one is tried.
    let c = w.toplevel("c", "quiet c");
    w.builds(&c);
    assert_eq!(w.m.apply(false).unwrap().result, ApplyResult::Switched);
    assert_eq!(w.outcome(), Some(TrialOutcome::Pending));
}

/// FAILS IF: an apply that runs before this boot's bless (an update started
/// at once) does not first judge a trial the earlier boot tried — and so
/// switches into the system that did not boot.
#[test]
fn an_apply_judges_a_trial_tried_before_it() {
    let w = World::new("apply-judges");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel_with_lock("b", "quiet b", LOCK_B);
    w.builds(&b);
    w.m.apply(false).unwrap();
    w.reboot(&a);
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    assert_eq!(w.m.apply(false).unwrap_err().why, Why::DidNotBoot);
    assert_eq!(w.outcome(), Some(TrialOutcome::Failed));
    assert_eq!(w.profile().as_deref(), Some(a.as_str()), "back on a");
    assert!(
        w.read("switched").ends_with("a boot\n"),
        "{}",
        w.read("switched")
    );
}

/// FAILS IF: a rollback by hand leaves the trial armed — the next boot would
/// take the one-shot into the system the person rolled away from — or lets
/// the next apply switch into it again; or a trial the profile still names
/// loses its way back.
#[test]
fn a_rollback_by_hand_disarms_the_trial() {
    let w = World::new("disarm");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(false).unwrap();
    // Still on b: disarm leaves the trial alone.
    w.m.disarm().unwrap();
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    assert_eq!(w.outcome(), Some(TrialOutcome::Pending));
    // The rescue's rollback: the profile back on generation 1.
    let profile = w.s.path("nix/var/nix/profiles/system");
    std::fs::remove_file(&profile).unwrap();
    std::os::unix::fs::symlink("system-1-link", &profile).unwrap();
    w.m.disarm().unwrap();
    assert_eq!(w.read("efi-oneshot"), "", "the one-shot is gone");
    assert_eq!(w.read("efi-default"), "", "loader.conf decides");
    assert_eq!(w.outcome(), Some(TrialOutcome::Failed));
    assert_eq!(
        w.lock(),
        LOCK_A,
        "the folder describes what the profile names"
    );
    assert_eq!(w.profile().as_deref(), Some(a.as_str()));
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    assert_eq!(w.m.apply(false).unwrap_err().why, Why::DidNotBoot);
}
