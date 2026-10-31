//! The verbs end to end, on a system root of their own: a store with two or
//! three toplevels, a system profile, `/run/booted-system`, and `nix`,
//! `nix-env` and `bootctl` that are small shell programs keeping a log of
//! what they were asked. They behave as the real ones do in the ways the
//! verbs depend on: `nix-env --set` makes no generation for the newest one
//! again, `bootctl list` names each generation's entry with its `init=`,
//! `set-oneshot` keeps the variable where `efivarfs` shows it and `""`
//! unsets it. A reboot ([`World::reboot`]) is a new boot identity, the
//! booted and running links moved, and the one-shot spent — what the boot
//! loader does with it.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ronoctua_person::Stamp;
use ronoctua_person::state::{
    ApplyResult, Hold, LastGood, MachineState, NoTrial, Trial, TrialOutcome,
};

use crate::system::{self, Env};
use crate::{Applying, Machine, Places, System, Why, disk};

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
        let reference = self.path("distro/flake.lock").display().to_string();
        let sizing = self.path("bin/sizing").display().to_string();
        let units = self.path("bin/build-units").display().to_string();
        let keep = self.path("bin/keep-units").display().to_string();
        System::from_env(&Env::of(&[
            (system::SYSROOT, &root),
            (system::MACHINE_ATTR, "noctua"),
            (system::NIX, &nix),
            (system::BOOTCTL, &bootctl),
            (system::MENU_LIMIT, "5"),
            (system::TEMPLATE, &template),
            (system::REFERENCE_LOCK, &reference),
            (system::SYSTEM_LOCK, SYSTEM_LOCK),
            (system::BUILD_SIZING, &sizing),
            (system::BUILD_UNITS, &units),
            (system::KEEP_UNITS, &keep),
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

/// systemd-boot's `loader.conf`, as NixOS's builder writes it.
const LOADER_CONF: &str =
    "timeout 5\ndefault nixos-generation-1.conf\neditor 0\nconsole-mode keep\n";

/// The machine flake at the distro's `a…` and [`LOCK_B`] at `b…`, both on
/// the same `nixpkgs` (the distro moved alone: an apply switches now).
const LOCK_A: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua","nixpkgs":"nixpkgs"}},"ronoctua":{"locked":{"rev":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"nixpkgs":{"locked":{"rev":"1111111111111111111111111111111111111111"}}},"root":"root","version":7}
"#;
const LOCK_B: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua","nixpkgs":"nixpkgs"}},"ronoctua":{"locked":{"rev":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}},"nixpkgs":{"locked":{"rev":"1111111111111111111111111111111111111111"}}},"root":"root","version":7}
"#;

/// A lock that names no `nixpkgs` at all, its own or the distro's.
const LOCK_NO_NIXPKGS: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua"}},"ronoctua":{"locked":{"rev":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}},"root":"root","version":7}
"#;

/// A machine flake that rolls `nixpkgs`, as the template's does: the distro
/// at `a…` and `nixpkgs` at `1…`; [`LOCK_NIX_B`] moved both, and
/// [`LOCK_DISTRO_B`] the distro alone.
const LOCK_NIX_A: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua","nixpkgs":"nixpkgs"}},"ronoctua":{"locked":{"rev":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"nixpkgs":{"locked":{"rev":"1111111111111111111111111111111111111111"}}},"root":"root","version":7}
"#;
const LOCK_NIX_B: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua","nixpkgs":"nixpkgs"}},"ronoctua":{"locked":{"rev":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}},"nixpkgs":{"locked":{"rev":"2222222222222222222222222222222222222222"}}},"root":"root","version":7}
"#;
const LOCK_DISTRO_B: &str = r#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua","nixpkgs":"nixpkgs"}},"ronoctua":{"locked":{"rev":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}},"nixpkgs":{"locked":{"rev":"1111111111111111111111111111111111111111"}}},"root":"root","version":7}
"#;

/// What the fake sizing says: what is no unit two at a time with two cores
/// each (and, for the units' own program, more it does not read here).
const SIZING: &str =
    "total_kb=1\nceiling_kb=\nslots=4\njobs=2\ncores=2\nheavy_cores=5\nclass_light_jobs=3\n";

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
    if [ "$dry" = 1 ]; then
      if [ -e "$root/dry-fails" ]; then echo "error: cannot open the store" >&2; exit 1; fi
      if [ -e "$root/dry-says" ]; then cat "$root/dry-says" >&2; fi
      exit 0
    fi
    echo "$last" >> "$root/built"
    if [ -e "$root/build-fails" ]; then echo "error: builder failed" >&2; exit 1; fi
    if [ -e "$root/fails-without-marker" ] && [ ! -e "$root/machine/hardware-deferred.json" ]; then
      echo "error: cannot download ipu6-camera-bins" >&2; exit 1
    fi
    ln -sfn "$(cat "$root/build-result")" "$out" ;;
  eval)
    if [ -e "$root/phases.json" ]; then cat "$root/phases.json"; exit 0; fi
    echo "error: attribute 'phases' missing" >&2; exit 1 ;;
  derivation)
    if [ -e "$root/shown.json" ]; then cat "$root/shown.json"; exit 0; fi
    echo "error: no such derivation" >&2; exit 1 ;;
  store)
    if [ -e "$root/mirror-has" ]; then rm -f "$root/units-fail"; cat "$root/mirror-has"; exit 0; fi
    echo "error: unable to download" >&2; exit 1 ;;
  path-info)
    cat > "$root/path-info-asked"
    if [ -e "$root/path-info.json" ]; then cat "$root/path-info.json"; exit 0; fi
    echo "error: no path info" >&2; exit 1 ;;
  flake)
    last=""; prev=""; out=""; ref=""
    for a in "$@"; do
      [ "$prev" = "--output-lock-file" ] && out="$a"
      [ "$prev" = "--reference-lock-file" ] && ref="$a"
      prev="$a"; last="$a"
    done
    d="${{last#path:}}"
    case "$2" in
      lock)
        if [ -e "$root/lock-fails" ]; then exit 1; fi
        if [ -n "$ref" ] && [ ! -f "$ref" ]; then echo "error: opening file '$ref': No such file or directory" >&2; exit 1; fi
        cp "$root/new-lock" "${{out:-$d/flake.lock}}" ;;
      update) cp "$root/updated-lock" "$d/flake.lock" ;;
      archive)
        if [ -e "$root/sources.json" ]; then cat "$root/sources.json"; exit 0; fi
        echo "error: cannot read the lock" >&2; exit 1 ;;
    esac ;;
esac
"#
            ),
        );
        // `nix-store --query --outputs DRV`: the output `outputs/<name>`
        // names, the name being the derivation's file name.
        script(
            &s.path("bin/nix-store"),
            &format!(
                r#"#!/bin/sh
root='{root}'
echo "nix-store $*" >> "$root/calls"
for a in "$@"; do last="$a"; done
case "$2" in
  --requisites) cat "$root/requisites" 2>/dev/null || exit 1 ;;
  *) cat "$root/outputs/${{last##*/}}" 2>/dev/null || exit 1 ;;
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
if [ "$3" = "--switch-generation" ]; then
  [ -L "$dir/system-$4-link" ] || exit 1
  ln -sfn "system-$4-link" "$p"
  if [ -x "$root/bin/grub-menu" ]; then "$root/bin/grub-menu"; fi
  exit 0
fi
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
if [ -x "$root/bin/grub-menu" ]; then "$root/bin/grub-menu"; fi
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
  set-oneshot)
    printf '%s' "$2" > "$root/efi-oneshot"
    v="$root{ONESHOT}"
    mkdir -p "${{v%/*}}"
    if [ -z "$2" ]; then rm -f "$v"; else
      s="$2"
      {{ printf '\007\000\000\000'
        while [ -n "$s" ]; do r="${{s#?}}"; printf '%s\000' "${{s%"$r"}}"; s="$r"; done
        printf '\000\000'; }} > "$v"
    fi ;;
  *) exit 2 ;;
esac
"#,
                ONESHOT = crate::loader::ONESHOT_VARIABLE,
            ),
        );
        // THE SIZING, as `sizing-says` says it; WHAT BUILDS THE UNITS, which
        // keeps the plan and the units it was handed (`--nix NIX PLAN WILL
        // HELD -- …`) and fails when `units-fail` exists; and WHAT KEEPS A
        // BUILD'S UNITS, which writes down what it was handed. (What a build
        // would build is the dry run's, `dry-says`.)
        std::fs::write(s.path("sizing-says"), SIZING).unwrap();
        script(
            &s.path("bin/sizing"),
            &format!("#!/bin/sh\necho sizing >> '{root}/calls'\ncat '{root}/sizing-says'\n"),
        );
        script(
            &s.path("bin/build-units"),
            &format!(
                "#!/bin/sh\necho \"build-units $*\" >> '{root}/calls'\ncp \"$3\" '{root}/units-plan'\ncp \"$4\" '{root}/units-will'\n[ ! -e '{root}/units-fail' ]\n"
            ),
        );
        script(
            &s.path("bin/keep-units"),
            &format!(
                "#!/bin/sh\necho \"keep-units $*\" >> '{root}/calls'\nset=''; prev=''\nfor a in \"$@\"; do [ \"$prev\" = --set ] && set=\"$a\"; prev=\"$a\"; last=\"$a\"; done\ncp \"$last\" \"{root}/kept-$set\"\n"
            ),
        );
        std::fs::create_dir_all(s.path("nix/store")).unwrap();
        std::fs::write(s.path("nix/store/k1"), "kernel").unwrap();
        std::fs::write(s.path("nix/store/i1"), "initrd").unwrap();
        std::fs::create_dir_all(s.path("nix/var/nix/profiles")).unwrap();
        std::fs::create_dir_all(s.path("boot/loader")).unwrap();
        std::fs::write(s.path("boot/loader/loader.conf"), LOADER_CONF).unwrap();
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
        // THE LOCK A WAS BUILT FROM is known, as on every machine whose
        // system was built by its flake or migrated: kept beside it.
        std::fs::create_dir_all(&w.m.places.state).unwrap();
        std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
        w
    }

    /// No lock of the profile's system kept beside it.
    fn forget_applied_lock(&self) {
        let _ = std::fs::remove_file(self.m.places.state.join(crate::APPLIED_LOCK));
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
        let _ = std::fs::remove_file(self.real(crate::loader::ONESHOT_VARIABLE));
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
        self.grub_menu();
    }

    /// THE SAME WORLD ON A BIOS MACHINE'S GRUB: `grub-editenv` in place of
    /// `bootctl`, keeping the trial's two variables where the fake `bootctl`
    /// keeps them (`efi-default`, `efi-oneshot`), and a `grub.cfg` written
    /// as NixOS's `install-grub.pl` writes it — the newest first, every
    /// generation in `All configurations` — whenever the profile moves.
    fn grub(what: &str) -> Self {
        let mut w = Self::new(what);
        let root = w.s.path("").display().to_string();
        script(
            &w.s.path("bin/grub-editenv"),
            &format!(
                r#"#!/bin/sh
root='{root}'
echo "grub-editenv $*" >> "$root/calls"
if [ -e "$root/no-efi" ]; then echo "error: cannot open $1" >&2; exit 1; fi
case "$2" in
  create) : > "$1" ;;
  set)
    k="${{3%%=*}}"; v="${{3#*=}}"
    case "$k" in
      ronoctua_default) printf '%s' "$v" > "$root/efi-default" ;;
      next_entry) printf '%s' "$v" > "$root/efi-oneshot" ;;
      *) exit 2 ;;
    esac ;;
  unset)
    case "$3" in
      ronoctua_default) printf '' > "$root/efi-default" ;;
      next_entry) printf '' > "$root/efi-oneshot" ;;
      *) exit 2 ;;
    esac ;;
  list)
    o="$(cat "$root/efi-oneshot" 2>/dev/null)"
    if [ -n "$o" ]; then echo "next_entry=$o"; fi
    d="$(cat "$root/efi-default" 2>/dev/null)"
    if [ -n "$d" ]; then echo "ronoctua_default=$d"; fi ;;
  *) exit 2 ;;
esac
"#
            ),
        );
        script(
            &w.s.path("bin/grub-menu"),
            &format!(
                r#"#!/bin/sh
root='{root}'
dir="$root/nix/var/nix/profiles"
cfg="$root/boot/grub/grub.cfg"
mkdir -p "$root/boot/grub"
top() {{ t="$(readlink -f "$1")"; printf '%s' "${{t#"$root"}}"; }}
{{
  printf 'menuentry "ronoctua" --class nixos {{\n  linux /k init=%s/init\n}}\n' "$(top "$dir/system")"
  printf 'submenu "ronoctua - All configurations" --class submenu {{\n'
  for l in "$dir"/system-*-link; do
    n="${{l##*/system-}}"; n="${{n%-link}}"
    printf 'menuentry "ronoctua - Configuration %s (2026-10-22 - 26.05)" --class nixos {{\n  linux /k init=%s/init quiet\n}}\n' "$n" "$(top "$l")"
  done
  printf '}}\n'
}} > "$cfg"
"#
            ),
        );
        w.m.system.bootctl = None;
        w.m.system.grub_editenv = Some(w.s.path("bin/grub-editenv"));
        w.grub_menu();
        w
    }

    /// GRUB's menu written again, when the world has one.
    fn grub_menu(&self) {
        let menu = self.s.path("bin/grub-menu");
        if menu.exists() {
            assert!(
                std::process::Command::new(&menu)
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }

    /// GRUB's id of generation `n`'s entry: its title's path.
    fn grub_entry(n: u64) -> String {
        format!("ronoctua - All configurations>ronoctua - Configuration {n} (2026-10-22 - 26.05)")
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
    let applied = w.m.apply(Applying::Now).unwrap();
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

/// FAILS IF: the update's apply (`--next-boot`) activates anything — the
/// switch is `boot`, never `switch`, and the system that runs stays —, is
/// recorded as switched or without the system it set up, or blesses at once
/// a system that boots like the last good one: nothing ran the new
/// userland, so the trial is armed for real, and the next restart tries it.
#[test]
fn an_apply_for_the_next_boot_activates_nothing_and_arms_a_real_trial() {
    let w = World::new("next-boot");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    // THE SAME KERNEL, INITRD AND PARAMETERS as a: a switch that returned 0
    // would bless it at once.
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    let line =
        w.m.run(&crate::Verb::Apply {
            how: Applying::NextBoot,
        })
        .unwrap();
    assert_eq!(line, "set for the next boot; trial pending");
    assert_eq!(w.read("switched"), "b boot\n", "nothing was switched now");
    assert_eq!(
        w.m.system.resolve(system::CURRENT).as_deref(),
        Some(a.as_str()),
        "the system that runs stays"
    );
    assert_eq!(w.profile().as_deref(), Some(b.as_str()));
    assert_eq!(w.read("efi-default"), "nixos-generation-1.conf");
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    let state = w.state();
    assert_eq!(state.trial.map(|t| t.outcome), Some(TrialOutcome::Pending));
    assert_eq!(state.last_good.unwrap().toplevel, a);
    let last = state.last_apply.unwrap();
    assert_eq!(
        (last.result, last.toplevel.as_deref()),
        (ApplyResult::Staged, Some(b.as_str()))
    );
    // THE BOOT THAT SET IT judges nothing; the restart tries it.
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Pending);
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    assert_eq!(w.state().last_good.unwrap().toplevel, b);
}

/// FAILS IF: `state.json` says `staged` or names the system an apply set up
/// — a boot trial's way back can land on a system whose bless and apply
/// refuse both —, the apply is not whole in `last-apply.json` beside it, or
/// an older system's own apply, written into `state.json` alone since, is not
/// what the record then says; or the file beside outlives the apply it is of.
#[test]
fn state_json_stays_readable_by_an_older_system() {
    let w = World::new("older-reads");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::NextBoot).unwrap();
    let told: serde_json::Value =
        serde_json::from_str(&w.snapshot(ronoctua_person::names::STATE)).unwrap();
    assert_eq!(told["last-apply"]["result"], "switched", "{told}");
    assert!(told["last-apply"].get("toplevel").is_none(), "{told}");
    let whole: serde_json::Value =
        serde_json::from_str(&w.snapshot(ronoctua_person::names::LAST_APPLY)).unwrap();
    assert_eq!(whole["last-apply"]["result"], "staged", "{whole}");
    assert_eq!(whole["last-apply"]["toplevel"], b.as_str(), "{whole}");
    let last = w.state().last_apply.unwrap();
    assert_eq!(
        (last.result, last.toplevel.as_deref()),
        (ApplyResult::Staged, Some(b.as_str()))
    );

    // AN OLDER SYSTEM, after the way back, rewrites `state.json` alone.
    let mut older = told;
    older["last-apply"] =
        serde_json::json!({"at": "2026-11-02T08:00:00Z", "result": "build-failed"});
    std::fs::write(w.m.places.state_file(), older.to_string()).unwrap();
    let last = w.state().last_apply.unwrap();
    assert_eq!(
        (last.result, last.toplevel),
        (ApplyResult::BuildFailed, None)
    );

    // A RECORD THAT NEEDS NOTHING BESIDE IT takes the file away.
    let beside = w.m.places.state.join(ronoctua_person::names::LAST_APPLY);
    w.set_state(&w.state());
    assert!(!beside.exists());

    // BUT NOT A NEWER SYSTEM'S, which this does not read: it may be of the
    // very apply `state.json` holds, and that system reads it again.
    let newer = r#"{"format": 2, "last-apply": {"at": "2026-11-02T08:00:00Z"}}"#;
    std::fs::write(&beside, newer).unwrap();
    w.set_state(&w.state());
    assert_eq!(std::fs::read_to_string(&beside).unwrap(), newer);
}

/// FAILS IF: a system set for the next boot is switched to live by the next
/// apply that is not the update's — the software centre's, the rescue's —
/// which would take, under the session, the very jump the update set aside.
#[test]
fn a_staged_system_stays_staged() {
    let w = World::new("stays-staged");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::NextBoot).unwrap();
    let c = w.toplevel("c", "quiet c");
    w.builds(&c);
    let applied = w.m.apply(Applying::Now).unwrap();
    assert_eq!(applied.result, ApplyResult::Staged);
    assert_eq!(w.read("switched"), "b boot\nc boot\n");
    assert_eq!(
        w.m.system.resolve(system::CURRENT).as_deref(),
        Some(a.as_str())
    );
    assert_eq!(w.profile().as_deref(), Some(c.as_str()));
    assert_eq!(w.read("efi-default"), "nixos-generation-1.conf");
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-3.conf");
    assert_eq!(
        w.state().last_apply.unwrap().toplevel.as_deref(),
        Some(c.as_str())
    );
    // AND ONCE IT RUNS, an apply switches now again.
    w.reboot(&c);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    let d = w.toplevel("d", "quiet d");
    w.builds(&d);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    assert!(
        w.read("switched").ends_with("d switch\n"),
        "{}",
        w.read("switched")
    );
}

/// FAILS IF: an apply that is not the update's — while the folder's lock
/// names another `nixpkgs` than the system the profile names was built from
/// (an update whose apply never ran, killed or switched off, left it moved)
/// — switches live across that `nixpkgs`; or one that moved only the distro
/// (the owner's build switch) is held for the next boot.
#[test]
fn a_moved_nixpkgs_is_set_for_the_next_boot() {
    let w = World::new("nixpkgs-moved");
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_NIX_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_NIX_B).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Staged
    );
    assert_eq!(w.read("switched"), "b boot\n");
    assert_eq!(w.outcome(), Some(TrialOutcome::Pending));

    let w = World::new("distro-moved");
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_NIX_A).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_DISTRO_B).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    assert_eq!(w.read("switched"), "b switch\n");
}

/// FAILS IF: a `nixpkgs` the apply cannot tell — a folder's lock that names
/// none, a profile's system whose lock is not known, a tarball's that moved
/// by its hash — is taken for one that did not move, and switched to live.
#[test]
fn an_unknown_nixpkgs_is_set_for_the_next_boot() {
    let staged = |w: &World| {
        let b = w.toplevel("b", "quiet b");
        w.builds(&b);
        let result = w.m.apply(Applying::Now).unwrap().result;
        (result, w.read("switched"))
    };
    // THE FOLDER'S LOCK NAMES NONE.
    let w = World::new("no-nixpkgs");
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_NO_NIXPKGS).unwrap();
    assert_eq!(staged(&w), (ApplyResult::Staged, "b boot\n".to_owned()));
    // THE PROFILE'S SYSTEM'S LOCK IS NOT KNOWN: it carries none, and none
    // is kept beside it.
    let w = World::new("unknown-running-lock");
    w.forget_applied_lock();
    assert_eq!(staged(&w), (ApplyResult::Staged, "b boot\n".to_owned()));
    // A TARBALL locks no revision: its hash moved, or it did not.
    let tarball = |hash: &str| {
        format!(
            r#"{{"nodes":{{"root":{{"inputs":{{"ronoctua":"ronoctua","nixpkgs":"nixpkgs"}}}},"ronoctua":{{"locked":{{"rev":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}},"nixpkgs":{{"locked":{{"narHash":"{hash}","type":"tarball"}}}}}},"root":"root","version":7}}"#
        )
    };
    for (now, result, switched) in [
        ("sha256-new", ApplyResult::Staged, "b boot\n"),
        ("sha256-old", ApplyResult::Switched, "b switch\n"),
    ] {
        let w = World::new("tarball");
        std::fs::write(
            w.m.places.state.join(crate::APPLIED_LOCK),
            tarball("sha256-old"),
        )
        .unwrap();
        std::fs::write(w.m.places.dir.join("flake.lock"), tarball(now)).unwrap();
        assert_eq!(staged(&w), (result, switched.to_owned()), "{now}");
    }
}

/// FAILS IF: whether a system waits for the next boot is read off anything
/// but the profile and the system that runs — a trial that failed (the
/// profile went back), a later apply that built nothing (the profile still
/// names what waits), a system with no trial, a specialisation of the
/// profile's running.
#[test]
fn whether_a_system_waits_is_read_off_the_profile() {
    let w = World::new("waits");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let root = w.s.path("");
    let waits = |w: &World| crate::next_boot(&root, Some(&w.state()));
    assert_eq!(waits(&w), None, "nothing applied");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::NextBoot).unwrap();
    assert_eq!(
        waits(&w),
        Some(crate::NextBoot {
            toplevel: b.clone(),
            guarded: true
        })
    );
    // A LATER APPLY THAT BUILT NOTHING: b still waits.
    std::fs::write(w.s.path("build-fails"), "").unwrap();
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::BuildFailed);
    assert_eq!(
        w.state().last_apply.unwrap().result,
        ApplyResult::BuildFailed
    );
    assert_eq!(waits(&w).map(|n| n.toplevel), Some(b));
    std::fs::remove_file(w.s.path("build-fails")).unwrap();
    // THE TRIAL FAILED: the profile is back on a, and nothing waits — the
    // record's last apply still says `staged`.
    w.m.apply(Applying::NextBoot).unwrap();
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    assert_eq!(waits(&w), None);

    // NO TRIAL COULD BE ARMED: it waits, with no way back.
    let w = World::new("waits-unguarded");
    std::fs::write(w.s.path("no-efi"), "").unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::NextBoot).unwrap();
    assert_eq!(
        crate::next_boot(&w.s.path(""), Some(&w.state())),
        Some(crate::NextBoot {
            toplevel: b.clone(),
            guarded: false
        })
    );
    // A SPECIALISATION of it running is it running.
    let spec = w.toplevel("b-spec", "quiet b");
    std::fs::create_dir_all(w.real(&b).join("specialisation")).unwrap();
    std::os::unix::fs::symlink(w.real(&spec), w.real(&b).join("specialisation/x")).unwrap();
    w.link("run/current-system", &spec);
    assert_eq!(crate::next_boot(&w.s.path(""), Some(&w.state())), None);
}

/// FAILS IF: the rescue's rollback, while systems wait for the next boot,
/// goes to the generation before the profile's — one nobody booted — rather
/// than back to the system that runs; switches anything then; makes a
/// generation; or leaves the trial of what waited armed.
#[test]
fn a_rollback_while_systems_wait_goes_back_to_the_one_that_runs() {
    for now in [true, false] {
        let w = World::new("rollback-waiting");
        let a = w.m.system.resolve(system::BOOTED).unwrap();
        for name in ["b", "c"] {
            let t = w.toplevel(name, &format!("quiet {name}"));
            w.builds(&t);
            assert_eq!(
                w.m.apply(Applying::NextBoot).unwrap().result,
                ApplyResult::Staged
            );
        }
        let c = w.state().trial.unwrap().toplevel;
        let line = w.m.run(&crate::Verb::Rollback { now }).unwrap();
        assert!(line.contains("back on the system that runs"), "{line}");
        assert_eq!(w.profile().as_deref(), Some(a.as_str()), "now: {now}");
        assert_eq!(
            w.read("switched"),
            "b boot\nc boot\na boot\n",
            "nothing was switched live (now: {now})"
        );
        assert!(
            !w.s.path("nix/var/nix/profiles/system-4-link").exists(),
            "no generation was made"
        );
        let state = w.state();
        assert_eq!(
            state.trial,
            Some(Trial {
                toplevel: c,
                outcome: TrialOutcome::Failed
            })
        );
        assert_eq!(
            (w.read("efi-default"), w.read("efi-oneshot")),
            (String::new(), String::new())
        );
        assert_eq!(w.lock(), LOCK_A);
        assert_eq!(crate::next_boot(&w.s.path(""), Some(&state)), None);
    }
}

/// FAILS IF: with nothing waiting the rescue's rollback does not go to the
/// generation before — now, or from the next boot, as asked —, or goes to
/// one that is no way back: the system that runs again, or the one whose
/// trial failed.
#[test]
fn a_rollback_goes_to_the_generation_before_that_is_a_way_back() {
    for (now, switched) in [(true, "a switch\n"), (false, "a boot\n")] {
        let w = World::new("rollback-before");
        let a = w.m.system.resolve(system::BOOTED).unwrap();
        let b = w.toplevel("b", "quiet b");
        w.builds(&b);
        assert_eq!(
            w.m.apply(Applying::Now).unwrap().result,
            ApplyResult::Switched
        );
        w.link("run/current-system", &b);
        let line = w.m.run(&crate::Verb::Rollback { now }).unwrap();
        assert!(line.contains("generation 1"), "{line}");
        assert_eq!(w.profile().as_deref(), Some(a.as_str()));
        assert!(
            w.read("switched").ends_with(switched),
            "{}",
            w.read("switched")
        );
        assert_eq!(w.outcome(), Some(TrialOutcome::Failed), "b was left");
    }
    // AFTER A TRIAL THAT FAILED the profile names a again (generation 3):
    // the one before is b, which failed, and the one before it a, which
    // runs — no way back at all.
    let w = World::new("rollback-nowhere");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    let switched = w.read("switched");
    assert_eq!(w.m.rollback(true).unwrap_err().why, Why::Failed);
    assert_eq!(w.profile().as_deref(), Some(a.as_str()));
    assert_eq!(w.read("switched"), switched, "nothing was switched");
}

/// FAILS IF: a hibernation spends the trial's one-shot — the resume would
/// boot it — or the resume does not put it back; or a machine that never
/// came back through its resume judges the trial it did not try as failed.
#[test]
fn a_hibernation_does_not_spend_the_trial() {
    let hibernate =
        |w: &World, resumed: bool| w.m.run(&crate::Verb::Hibernate { resumed }).unwrap();
    let w = World::new("hibernate");
    assert_eq!(hibernate(&w, false), "no trial waits: nothing put aside");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::NextBoot).unwrap();
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    assert_eq!(hibernate(&w, false), "put aside nixos-generation-2.conf");
    assert_eq!(w.read("efi-oneshot"), "", "the resume boots the default");
    assert_eq!(w.read("efi-default"), "nixos-generation-1.conf");
    assert_eq!(hibernate(&w, true), "put back nixos-generation-2.conf");
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    assert!(w.snapshot(crate::sleep::ASLEEP).is_empty(), "the note went");
    assert_eq!(w.outcome(), Some(TrialOutcome::Pending));
    // THE RESUME NEVER CAME (the power lost): the next boot was the default,
    // and the trial was not tried — it is armed again, and the restart
    // after tries it.
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    hibernate(&w, false);
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Pending);
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-2.conf");
    assert_eq!(
        w.profile().as_deref(),
        Some(b.as_str()),
        "nothing went back"
    );
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    // A ONE-SHOT THAT IS NOT THE TRIAL'S is left alone.
    let w = World::new("hibernate-other");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::NextBoot).unwrap();
    let w2 = w.m.system.bootctl.clone().unwrap();
    assert!(
        std::process::Command::new(w2)
            .args(["set-oneshot", "nixos-generation-1.conf"])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        hibernate(&w, false),
        "the one-shot is not the trial's: left as it is"
    );
    assert_eq!(w.read("efi-oneshot"), "nixos-generation-1.conf");
}

/// FAILS IF: applies set for the next boot, stacked up by a machine nobody
/// restarts, push the last good system out of the boot menu — the menu
/// lists a few generations, and every one waiting for a restart is one.
#[test]
fn staged_applies_stop_before_the_last_good_one_falls_out() {
    let w = World::new("staged-menu");
    let mut m = w.m.clone();
    m.system.menu_limit = Some("3".to_owned());
    for name in ["b", "c"] {
        let t = w.toplevel(name, &format!("quiet {name}"));
        w.builds(&t);
        assert_eq!(
            m.apply(Applying::NextBoot).unwrap().result,
            ApplyResult::Staged
        );
    }
    let d = w.toplevel("d", "quiet d");
    w.builds(&d);
    assert_eq!(
        m.apply(Applying::NextBoot).unwrap_err().why,
        Why::LastGoodWouldFallOut
    );
    let c = w.state().trial.unwrap().toplevel;
    assert_eq!(w.profile(), Some(c.clone()), "no generation was made");
    // THE RESTART the machine asked for confirms what waited, and the next
    // update has room again.
    w.reboot(&c);
    assert_eq!(m.bless().unwrap(), TrialOutcome::Good);
    assert_eq!(
        m.apply(Applying::NextBoot).unwrap().result,
        ApplyResult::Staged
    );
}

/// FAILS IF: a machine installed with no network keeps its marker once it
/// builds without it — the hardware that waited never arrives — or loses
/// it when that build fails: then the marker is back, the machine is built
/// as it was, and the apply still switches.
#[test]
fn what_waited_for_a_network_arrives_with_the_first_apply_that_builds_it() {
    let marker = |w: &World| {
        w.m.places
            .dir
            .join(ronoctua_person::names::DEFERRED_HARDWARE)
    };
    // IT BUILDS: the marker goes, for good.
    let w = World::new("deferred-arrives");
    std::fs::write(marker(&w), "{\n  \"format\": 1\n}\n").unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    assert!(
        !marker(&w).exists(),
        "the marker outlived a build without it"
    );
    // IT DOES NOT: the marker is back, the machine built as it was.
    let w = World::new("deferred-waits");
    std::fs::write(marker(&w), "{\n  \"format\": 1\n}\n").unwrap();
    std::fs::write(w.s.path("fails-without-marker"), "").unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    assert_eq!(
        std::fs::read_to_string(marker(&w)).unwrap(),
        "{\n  \"format\": 1\n}\n",
        "the marker did not come back whole"
    );
    assert_eq!(w.read("built").lines().count(), 2, "{}", w.read("built"));
    assert_eq!(w.read("switched"), "b switch\n");
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
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::BuildFailed);
    assert_eq!(w.lock(), LOCK_A, "the lock is the running system's again");
    assert_eq!(w.profile(), before);
    assert_eq!(w.read("switched"), "");
    assert_eq!(
        w.state().last_apply.unwrap().result,
        ApplyResult::BuildFailed
    );
}

/// What Nix would build for a system whose build needs a crate no cache
/// has: the download, the unit built from it and the system.
const PLANNED_WITH_A_DOWNLOAD: &str = "these 3 derivations will be built:\n  /nix/store/aaaa-rust-lib-x-0.1.0.drv\n  /nix/store/dddddddddddddddddddddddddddddddd-crate-x-0.1.0.tar.gz.drv\n  /nix/store/tttt-nixos-system-noctua.drv\n";

/// `nix derivation show` of that plan: the download pinned by its hash, and
/// two derivations that are none.
const SHOWN_WITH_A_DOWNLOAD: &str = r#"{"version":4,"derivations":{
"dddddddddddddddddddddddddddddddd-crate-x-0.1.0.tar.gz.drv":{"name":"crate-x-0.1.0.tar.gz",
  "outputs":{"out":{"hash":"sha256-AAECAw==","method":"flat"}},
  "env":{"urls":"https://static.crates.io/crates/x/x-0.1.0.crate"}},
"aaaa-rust-lib-x-0.1.0.drv":{"name":"rust-lib-x-0.1.0","outputs":{"out":{}},"env":{}},
"tttt-nixos-system-noctua.drv":{"name":"nixos-system-noctua","outputs":{"out":{}},"env":{}}}}"#;

/// The store path the crate becomes.
const CRATE_OUT: &str = "/nix/store/00000000000000000000000000000000-crate-x-0.1.0.tar.gz";

/// `w` planned to need the crate: its phases, the plan, what Nix shows of
/// it and the crate's output.
fn needs_a_download(w: &World) {
    std::fs::write(w.s.path("phases.json"), PHASES).unwrap();
    std::fs::write(w.s.path("dry-says"), PLANNED_WITH_A_DOWNLOAD).unwrap();
    std::fs::write(w.s.path("shown.json"), SHOWN_WITH_A_DOWNLOAD).unwrap();
    std::fs::create_dir_all(w.s.path("outputs")).unwrap();
    std::fs::write(
        w.s.path("outputs/dddddddddddddddddddddddddddddddd-crate-x-0.1.0.tar.gz.drv"),
        format!("{CRATE_OUT}\n"),
    )
    .unwrap();
}

/// FAILS IF: a build that failed for want of a download does not record,
/// in the machine's `state.json`, which one it was — its name, the hash the
/// flake pins, where it comes from and the store path only those bytes
/// reach —, or asks Nix the output of what is no download, or keeps the
/// list once a build worked (`apply-names-what-failed`).
#[test]
fn a_failed_build_names_the_downloads_it_could_not_fetch() {
    let w = World::new("names-what-failed");
    needs_a_download(&w);
    std::fs::write(w.s.path("units-fail"), "").unwrap();
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::BuildFailed);
    let last = w.state().last_apply.unwrap();
    assert_eq!(last.result, ApplyResult::BuildFailed);
    assert_eq!(
        last.missing,
        [ronoctua_person::frontier::Download {
            drv: "/nix/store/dddddddddddddddddddddddddddddddd-crate-x-0.1.0.tar.gz.drv".to_owned(),
            out: CRATE_OUT.to_owned(),
            name: "crate-x-0.1.0.tar.gz".to_owned(),
            hash: "sha256-AAECAw==".to_owned(),
            mode: ronoctua_person::frontier::Mode::Flat,
            urls: vec!["https://static.crates.io/crates/x/x-0.1.0.crate".to_owned()],
        }]
    );
    let asked: Vec<String> = w
        .read("calls")
        .lines()
        .filter(|l| l.starts_with("nix-store "))
        .map(str::to_owned)
        .collect();
    assert_eq!(
        asked,
        [
            "nix-store --query --outputs /nix/store/dddddddddddddddddddddddddddddddd-crate-x-0.1.0.tar.gz.drv"
        ],
        "only the download is asked its output"
    );
    // THE NEXT BUILD WORKS: nothing is missing any more, and the file says
    // nothing of it.
    std::fs::remove_file(w.s.path("units-fail")).unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    assert!(w.state().last_apply.unwrap().missing.is_empty());
    assert!(
        !std::fs::read_to_string(w.m.places.state.join(ronoctua_person::names::STATE))
            .unwrap()
            .contains("missing"),
        "an empty list is written as no key"
    );
}

/// FAILS IF: a build that failed for want of a download the person's
/// mirror holds is not built again with it — asked by the hash the flake
/// pins, at the mirror the person named —, or the record still names it.
#[test]
fn a_mirror_brings_what_the_system_could_not_fetch() {
    let w = World::new("mirror-brings");
    needs_a_download(&w);
    std::fs::write(w.s.path("units-fail"), "").unwrap();
    std::fs::write(
        w.s.path("mirror-has"),
        format!(r#"{{"storePath":"{CRATE_OUT}"}}"#),
    )
    .unwrap();
    let person = w.m.places.dir.join(ronoctua_person::names::MACHINE_PERSON);
    std::fs::create_dir_all(&person).unwrap();
    let options = ronoctua_person::options::Options {
        download_mirrors: vec!["https://mirror.example.org".to_owned()],
        ..ronoctua_person::options::Options::default()
    };
    std::fs::write(
        person.join(ronoctua_person::names::OPTIONS),
        ronoctua_person::to_json(&options),
    )
    .unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    let asked: Vec<String> = w
        .read("calls")
        .lines()
        .filter(|l| l.starts_with("nix store prefetch-file"))
        .map(str::to_owned)
        .collect();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(
        asked[0].contains("--expected-hash sha256-AAECAw==")
            && asked[0].ends_with("https://mirror.example.org/sha256/00010203"),
        "{}",
        asked[0]
    );
    assert!(w.state().last_apply.unwrap().missing.is_empty());
}

/// FAILS IF: a build that worked does not keep the downloads its system
/// was built from — and only those the store holds that a hash pins —, or
/// asks Nix about what the system is built from through anything but the
/// system's own `.drv`.
#[test]
fn a_build_that_worked_keeps_its_downloads() {
    let w = World::new("keeps-downloads");
    needs_a_download(&w);
    std::fs::write(
        w.s.path("requisites"),
        format!(
            "/nix/store/tttt-nixos-system-noctua.drv\n{CRATE_OUT}\n/nix/store/bbbb-source\n/nix/store/qqqq-ronoctua-shell-0.1.0\n"
        ),
    )
    .unwrap();
    std::fs::write(
        w.s.path("path-info.json"),
        r#"{"info":{"00000000000000000000000000000000-crate-x-0.1.0.tar.gz":{"ca":{"method":"flat","hash":"sha256-AAECAw=="}},
"bbbb-source":{"ca":{"method":"nar","hash":"sha256-B"}},"qqqq-ronoctua-shell-0.1.0":{"ca":null}},"storeDir":"/nix/store","version":2}"#,
    )
    .unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    assert!(
        w.read("calls").contains(
            "nix-store --query --requisites --include-outputs /nix/store/tttt-nixos-system-noctua.drv"
        ),
        "{}",
        w.read("calls")
    );
    assert_eq!(
        w.read("path-info-asked"),
        format!("{CRATE_OUT}\n/nix/store/bbbb-source\n/nix/store/qqqq-ronoctua-shell-0.1.0\n"),
        "a .drv is never asked"
    );
    assert_eq!(
        w.read(&format!("kept-{}", crate::build::DOWNLOADS_SET)),
        format!("{CRATE_OUT}\n/nix/store/bbbb-source\n")
    );
}

/// FAILS IF: the hardware that waits for a network, built without again,
/// leaves no word of what it was missing: the machine switches, and its
/// record names the downloads the hardware still needs.
#[test]
fn the_hardware_that_still_waits_says_what_it_needs() {
    let w = World::new("hardware-names");
    needs_a_download(&w);
    let marker =
        w.m.places
            .dir
            .join(ronoctua_person::names::DEFERRED_HARDWARE);
    std::fs::write(&marker, "{\n  \"format\": 1\n}\n").unwrap();
    std::fs::write(w.s.path("fails-without-marker"), "").unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    assert!(marker.exists(), "the marker is back");
    let last = w.state().last_apply.unwrap();
    assert_eq!(last.result, ApplyResult::Switched);
    assert_eq!(
        last.missing
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        ["crate-x-0.1.0.tar.gz"]
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
    let applied = w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
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
    assert_eq!(
        m.apply(Applying::Now).unwrap_err().why,
        Why::LastGoodWouldFallOut
    );
    assert_eq!(
        w.profile().as_deref(),
        Some(c.as_str()),
        "no generation was made"
    );
    assert_eq!(w.lock(), LOCK_A);
    assert_eq!(w.state().last_apply.unwrap().result, ApplyResult::Refused);
    m.system.menu_limit = Some("3".to_owned());
    assert!(m.apply(Applying::Now).is_ok(), "a menu of three has room");
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
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::Held);
    assert_eq!(w.read("built"), "", "nothing was built");
    w.m.unhold().unwrap();
    assert_eq!(w.state().hold, None);
    assert!(w.m.apply(Applying::Now).is_ok());
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
    let applied = w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
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
    let applied = w.m.apply(Applying::Now).unwrap();
    assert_eq!(applied.result, ApplyResult::Switched);
    assert_eq!(w.read("switched"), "");
    assert!(!w.read("calls").contains("nix-env"));
}

/// FAILS IF: a switch that exits 4 is called a failure, or one that exits 1
/// is not — and the trial is armed in both, since the profile moved.
#[test]
fn a_switch_is_judged_by_its_exit() {
    for (how, exit, result) in [
        (Applying::Now, "4", Ok(ApplyResult::SwitchedWithFailures)),
        (Applying::Now, "1", Err(Why::SwitchFailed)),
        // SET FOR THE NEXT BOOT, a `boot` that failed is said as that: the
        // switch that never ran did not fail.
        (Applying::NextBoot, "1", Err(Why::NextBootFailed)),
    ] {
        let w = World::new("switch-exit");
        std::fs::write(w.s.path("switch-exit"), exit).unwrap();
        let b = w.toplevel("b", "quiet b");
        w.builds(&b);
        assert_eq!(w.m.apply(how).map(|a| a.result).map_err(|r| r.why), result);
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
    let applied = w.m.apply(Applying::Rehearse).unwrap();
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
    assert_eq!(m.apply(Applying::Now).unwrap_err().why, Why::NotRoot);
    assert_eq!(m.bless().unwrap_err().why, Why::NotRoot);
    assert_eq!(m.unhold().unwrap_err().why, Why::NotRoot);
    assert_eq!(m.migrate().unwrap_err().why, Why::NotRoot);
    assert_eq!(m.disarm().unwrap_err().why, Why::NotRoot);
}

/// The old layout: the distro's tree with the machine's own files in it.
fn old_layout(w: &World) {
    // THE OLD LAYOUT kept no lock of its system: the migration takes one.
    w.forget_applied_lock();
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
    std::fs::write(
        dir.join("local/approved.json"),
        "{\"format\":1,\"modules\":{}}\n",
    )
    .unwrap();
    std::fs::create_dir_all(w.s.path("template")).unwrap();
    std::fs::write(
        w.s.path("template/flake.nix"),
        "{\n  inputs.ronoctua.url = \"github:ronoctua/ronoctua-distro/release\";\n}\n",
    )
    .unwrap();
    std::fs::write(w.s.path("new-lock"), LOCK_B).unwrap();
    // The distro's own lock, the one the new lock starts from.
    std::fs::create_dir_all(w.s.path("distro")).unwrap();
    std::fs::write(w.s.path("distro/flake.lock"), LOCK_A).unwrap();
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
        std::fs::read(dir.join("local/approved.json")).unwrap(),
        b"{\"format\":1,\"modules\":{}}\n"
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
    // THE LOCK STARTED FROM THE DISTRO'S OWN, and was written beside the new
    // flake: the template's rolling nixpkgs is the tested pin.
    let next = dir.join(crate::migrate::NEXT);
    let locked = format!(
        "nix flake lock --extra-experimental-features nix-command flakes --reference-lock-file {} --output-lock-file {}/flake.lock path:{}",
        w.s.path("distro/flake.lock").display(),
        next.display(),
        next.display()
    );
    assert!(
        w.read("calls").lines().any(|l| l == locked),
        "{}",
        w.read("calls")
    );
    assert_eq!(w.m.migrate().unwrap_err().why, Why::NothingToMigrate);
}

/// FAILS IF: a migration goes on without the distro's own lock to start
/// the new one from — with no `RONOCTUA_MACHINE_REFERENCE_LOCK` it would
/// resolve the template's rolling branch on the day — or, with one that is
/// not there, leaves anything of its attempt behind.
#[test]
fn a_migration_starts_from_the_distros_lock_or_not_at_all() {
    let w = World::new("migrate-no-reference");
    old_layout(&w);
    let before = tree(&w.m.places.dir);
    let World { s, mut m } = w;
    let named = m.system.reference_lock.take().expect("the world names one");
    assert_eq!(m.migrate().unwrap_err().why, Why::Failed, "no reference");
    assert_eq!(tree(&m.places.dir), before, "no reference");
    assert!(
        !s.path("calls").exists()
            || !std::fs::read_to_string(s.path("calls"))
                .unwrap()
                .contains("flake lock"),
        "it locked with no reference"
    );
    std::fs::remove_file(&named).unwrap();
    m.system.reference_lock = Some(named);
    assert_eq!(
        m.migrate().unwrap_err().why,
        Why::BuildFailed,
        "a reference that is not there"
    );
    assert_eq!(tree(&m.places.dir), before, "a reference that is not there");
    assert!(
        !m.places.dir.join(crate::migrate::NEXT).exists(),
        "no .next left"
    );
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
    w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
    std::fs::remove_file(w.m.places.state.join(crate::boots::ARMED)).unwrap();
    w.link("run/current-system", &b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Pending);

    // SET FOR THE NEXT BOOT, the record of the arming boot lost: the system
    // that runs is the old one, and the one-shot the loader still holds
    // says no boot has tried the trial.
    let w = World::new("staged-unrecorded");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::NextBoot).unwrap();
    std::fs::remove_file(w.m.places.state.join(crate::boots::ARMED)).unwrap();
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Pending);
    assert_eq!(
        w.profile().as_deref(),
        Some(b.as_str()),
        "nothing went back"
    );
    assert!(
        !w.read("switched").contains("a boot"),
        "{}",
        w.read("switched")
    );
    // An apply in the same boot agrees, and stages what it builds.
    let c = w.toplevel("c", "quiet c");
    w.builds(&c);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Staged
    );
    std::fs::remove_file(w.m.places.state.join(crate::boots::ARMED)).unwrap();
    // AND ONCE A BOOT SPENT THE ONE-SHOT and came up on the old system, the
    // trial did not boot.
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
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
    w.m.apply(Applying::Now).unwrap();
    w.reboot(&b);
    let applied = w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
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
        let _ = w.m.apply(Applying::Now);
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
    w.forget_applied_lock();
    let a = w.toplevel_with_lock("a2", "quiet", LOCK_A);
    w.generation(2, &a);
    w.reboot(&a);
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    std::fs::write(w.s.path("build-fails"), "").unwrap();
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::BuildFailed);
    assert_eq!(w.lock(), LOCK_A);

    // Installed, the first trial ever fails.
    let w = World::new("installed-trial");
    w.forget_applied_lock();
    let a = w.toplevel_with_lock("a2", "quiet", LOCK_A);
    w.generation(2, &a);
    w.reboot(&a);
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let b = w.toplevel_with_lock("b", "quiet b", LOCK_B);
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
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
    w.m.apply(Applying::Now).unwrap();
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    // The update moves the input to the same revision again.
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    let switched = w.read("switched");
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::DidNotBoot);
    assert_eq!(w.read("switched"), switched, "nothing was switched");
    assert_eq!(w.profile().as_deref(), Some(a.as_str()));
    assert_eq!(w.lock(), LOCK_A, "the folder is put back");
    assert_eq!(w.outcome(), Some(TrialOutcome::Failed));
    assert_eq!(w.state().last_apply.unwrap().result, ApplyResult::Refused);
    // A newer revision builds another system, and that one is tried.
    let c = w.toplevel("c", "quiet c");
    w.builds(&c);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
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
    w.m.apply(Applying::Now).unwrap();
    w.reboot(&a);
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::DidNotBoot);
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
    w.m.apply(Applying::Now).unwrap();
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
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::DidNotBoot);
}

/// The phases a distro would describe: the plan of four units — one below
/// the shell, the shell (alone), and a second unit built alone that the
/// system does not reach, with a unit below it —, the system, and two units
/// to keep. Its classes are a newer distro's, none this program or its
/// distro's units program ever knew, nor are their slots.
const PHASES: &str = r#"{"format":3,
"units":["release/libs/x feather named 300 /nix/store/aaaa-rust-lib-x-0.1.0.drv",
         "release/bins/ronoctua-shell giant named alone:6000+400 /nix/store/ssss-ronoctua-shell-0.1.0.drv 0",
         "release/libs/y feather carried 300 /nix/store/xxxx-rust-lib-y-0.1.0.drv",
         "release/bins/other giant default alone:6000+400 /nix/store/oooo-other-0.1.0.drv 2"],
"system":"/nix/store/tttt-nixos-system-noctua.drv",
"kept":["/nix/store/pppp-rust-lib-x-0.1.0","/nix/store/qqqq-ronoctua-shell-0.1.0"]}"#;

/// The plan's lines, as the units' program must be handed them: word for
/// word, its classes and costs as the distro wrote them.
const PLAN: &str = "release/libs/x feather named 300 /nix/store/aaaa-rust-lib-x-0.1.0.drv\nrelease/bins/ronoctua-shell giant named alone:6000+400 /nix/store/ssss-ronoctua-shell-0.1.0.drv 0\nrelease/libs/y feather carried 300 /nix/store/xxxx-rust-lib-y-0.1.0.drv\nrelease/bins/other giant default alone:6000+400 /nix/store/oooo-other-0.1.0.drv 2\n";

/// What Nix would build for a system whose shell changed, in its own words:
/// the unit below the shell, the shell and the system — not the second heavy
/// unit, which the system does not reach.
const PLANNED: &str = "these 3 derivations will be built:\n  /nix/store/aaaa-rust-lib-x-0.1.0.drv\n  /nix/store/ssss-ronoctua-shell-0.1.0.drv\n  /nix/store/tttt-nixos-system-noctua.drv\nthese 2 paths will be fetched (1.00 MiB download, 2.00 MiB unpacked):\n  /nix/store/gggg-glibc\n  /nix/store/ffff-ffmpeg\n";

/// The `nix build` lines of the log that build, in order (not the dry run).
fn builds(w: &World) -> Vec<String> {
    w.read("calls")
        .lines()
        .filter(|l| l.starts_with("nix build") && !l.contains("--dry-run"))
        .map(str::to_owned)
        .collect()
}

/// The lines the units' program was handed, or none when it never ran.
fn handed(w: &World) -> Option<(String, String)> {
    w.read("calls")
        .contains("build-units ")
        .then(|| (w.read("units-plan"), w.read("units-will")))
}

/// FAILS IF: an apply does not build the distro's units first — handing the
/// units' program the whole plan and what Nix would build for the system
/// (its units, and what is no unit: the program builds first what the units
/// are built from, and asks Nix nothing again), with the machine's own
/// `nix` and features — and then the system, sized as what is no unit; or
/// hands it a unit the system does not reach; or does not hand the units to
/// keep to what keeps them once the build worked.
#[test]
fn an_apply_builds_its_units_then_the_system_and_keeps_them() {
    let w = World::new("phases");
    std::fs::write(w.s.path("phases.json"), PHASES).unwrap();
    std::fs::write(w.s.path("dry-says"), PLANNED).unwrap();
    std::fs::write(w.s.path("sources.json"), SOURCES).unwrap();
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    let calls = w.read("calls");
    let order: Vec<&str> = calls
        .lines()
        .map(|l| {
            (
                l,
                l.split_whitespace().take(2).collect::<Vec<_>>().join(" "),
            )
        })
        .map(|(l, head)| match head.as_str() {
            "sizing" => "sizing",
            "nix eval" => "eval",
            "nix build" if l.contains("--dry-run") => "dry-run",
            "nix build" => "build",
            "build-units --nix" => "units",
            "keep-units --roots" if l.contains(" --set flake ") => "keep-sources",
            "keep-units --roots" => "keep",
            "nix flake" if l.contains(" archive ") => "sources",
            "nix-env --profile" => "profile",
            _ => "other",
        })
        .filter(|w| *w != "other")
        .collect();
    assert_eq!(
        order,
        [
            "sizing",
            "eval",
            "dry-run",
            "units",
            "build",
            "keep",
            "sources",
            "keep-sources",
            "profile"
        ],
        "{calls}"
    );
    let (plan, will) = handed(&w).unwrap();
    assert_eq!(plan, PLAN, "the units' program is handed the whole plan");
    assert_eq!(
        will,
        "/nix/store/aaaa-rust-lib-x-0.1.0.drv\n/nix/store/ssss-ronoctua-shell-0.1.0.drv\n/nix/store/tttt-nixos-system-noctua.drv\n",
        "what Nix would build for the system, whole, and not the unit it does not reach"
    );
    let units = calls
        .lines()
        .find(|l| l.starts_with("build-units"))
        .unwrap();
    assert!(
        units.contains(&format!("--nix {} ", w.s.path("bin/nix").display()))
            && units.ends_with(" -- --extra-experimental-features nix-command flakes"),
        "the units are built by the machine's own nix: {units}"
    );
    assert!(
        !units.contains("--units-only"),
        "Nix's plan answered: the program is not told to ask it again: {units}"
    );
    let built = builds(&w);
    assert_eq!(built.len(), 1, "{built:?}");
    assert!(
        built[0].contains("--max-jobs 2 --cores 2")
            && built[0].ends_with("/nix/store/tttt-nixos-system-noctua.drv^*"),
        "the system, sized as what is no unit: {}",
        built[0]
    );
    let dry = calls.lines().find(|l| l.contains("--dry-run")).unwrap();
    assert!(
        dry.ends_with(" --dry-run /nix/store/tttt-nixos-system-noctua.drv^*"),
        "Nix is asked what the system's own build builds: {dry}"
    );
    let eval = calls.lines().find(|l| l.starts_with("nix eval")).unwrap();
    assert!(
        eval.contains("--no-update-lock-file")
            && eval.ends_with("#nixosConfigurations.noctua.config.ronoctua.machine.phases"),
        "{eval}"
    );
    let keep = calls.lines().find(|l| l.starts_with("keep-units")).unwrap();
    let roots = w.m.places.state.join(crate::build::KEPT_DIR);
    assert!(
        keep.contains(&format!("--roots {} --set release", roots.display())),
        "{keep}"
    );
    assert_eq!(
        w.read("kept-release"),
        "/nix/store/pppp-rust-lib-x-0.1.0\n/nix/store/qqqq-ronoctua-shell-0.1.0\n"
    );
    assert!(
        !w.m.places.run.join("units").exists() && !w.m.places.run.join("kept-units.txt").exists(),
        "what held the units while building does not outlive the apply"
    );
    let archive = calls
        .lines()
        .find(|l| l.starts_with("nix flake archive"))
        .unwrap();
    assert!(
        archive.ends_with(&format!(
            " --no-update-lock-file --dry-run --json path:{}",
            w.m.places.dir.display()
        )),
        "the machine's flake is only measured, off its lock: {archive}"
    );
    let sources = w.m.places.state.join(crate::build::SOURCES_DIR);
    assert!(
        calls.contains(&format!("--roots {} --set flake ", sources.display())),
        "{calls}"
    );
    assert_eq!(
        w.read("kept-flake"),
        "/nix/store/dddd-source\n/nix/store/hhhh-source\n/nix/store/mmmm-source\n/nix/store/nnnn-source\n",
        "every source the archive names, an input's input too, once"
    );
    assert!(!w.m.places.run.join("kept-sources.txt").exists());
}

/// What `nix flake archive --dry-run --json` names for a machine's flake:
/// its own source, the distro, the distro's inputs and an input's input —
/// the one the distro shares with it named twice.
const SOURCES: &str = r#"{"path":"/nix/store/mmmm-source","inputs":{"ronoctua":{"path":"/nix/store/dddd-source","inputs":{"home-manager":{"path":"/nix/store/hhhh-source","inputs":{"nixpkgs":{"path":"/nix/store/nnnn-source","inputs":{}}}},"nixpkgs":{"path":"/nix/store/nnnn-source","inputs":{}}}}}}"#;

/// FAILS IF: `keep-sources` does not root, as the apply does, every source
/// the machine's flake names (once each) under the machine's record, or says
/// it kept them when it did not; or, while an apply holds the machine —
/// whose build keeps its own —, waits for it, fails, or runs anything at
/// all; or a list `nix` could not make is a refusal.
#[test]
fn keep_sources_roots_what_the_flake_names_and_never_waits() {
    let w = World::new("keep-sources");
    std::fs::write(w.s.path("sources.json"), SOURCES).unwrap();
    assert_eq!(w.m.run(&crate::Verb::KeepSources).unwrap(), "sources kept");
    let calls = w.read("calls");
    assert!(
        calls.contains(&format!(
            "nix flake archive --extra-experimental-features nix-command flakes --no-update-lock-file --dry-run --json path:{}",
            w.m.places.dir.display()
        )),
        "{calls}"
    );
    let sources = w.m.places.state.join(crate::build::SOURCES_DIR);
    assert!(
        calls.contains(&format!(
            "keep-units --roots {} --set flake ",
            sources.display()
        )),
        "{calls}"
    );
    assert_eq!(
        w.read("kept-flake"),
        "/nix/store/dddd-source\n/nix/store/hhhh-source\n/nix/store/mmmm-source\n/nix/store/nnnn-source\n"
    );
    assert!(!w.m.places.run.join("kept-sources.txt").exists());

    let w = World::new("keep-sources-busy");
    std::fs::write(w.s.path("sources.json"), SOURCES).unwrap();
    {
        let _apply = ronoctua_person::root::Lock::take(
            &w.m.system.person_lock(),
            std::time::Duration::from_secs(5),
        )
        .unwrap();
        let started = std::time::Instant::now();
        assert_eq!(
            w.m.run(&crate::Verb::KeepSources).unwrap(),
            "sources not kept"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "an apply's machine is not waited for"
        );
    }
    assert_eq!(w.read("calls"), "", "nothing runs beside an apply");

    let w = World::new("keep-sources-unlisted");
    assert_eq!(
        w.m.run(&crate::Verb::KeepSources).unwrap(),
        "sources not kept"
    );
    assert!(!w.read("calls").contains("keep-units"));
}

/// FAILS IF: an apply whose sources could not be listed — `nix flake
/// archive` failed, or answered what this program does not read — does not
/// switch, or hands anything to what keeps them (the last build's stay); or
/// the units are not kept all the same.
#[test]
fn sources_not_listed_are_not_kept_and_switch_all_the_same() {
    for answer in [None, Some("{\"path\":\"/tmp/not-the-store\"}")] {
        let w = World::new("sources");
        std::fs::write(w.s.path("phases.json"), PHASES).unwrap();
        std::fs::write(w.s.path("dry-says"), PLANNED).unwrap();
        if let Some(json) = answer {
            std::fs::write(w.s.path("sources.json"), json).unwrap();
        }
        let b = w.toplevel("b", "quiet panic=10");
        w.builds(&b);
        assert_eq!(
            w.m.apply(Applying::Now).unwrap().result,
            ApplyResult::Switched,
            "{answer:?}"
        );
        let calls = w.read("calls");
        assert!(calls.contains("nix flake archive"), "{calls}");
        assert!(!calls.contains(" --set flake "), "{answer:?}: {calls}");
        assert!(calls.contains(" --set release "), "{answer:?}: {calls}");
    }
}

/// FAILS IF: a build that fails — its units, or the system after them —
/// hands anything to what keeps the units (the last build's stay), builds
/// the system after its units failed, or leaves the out-links that held its
/// units.
#[test]
fn a_failed_build_keeps_the_last_units() {
    for knob in ["units-fail", "build-fails"] {
        let w = World::new("phases-fail");
        std::fs::write(w.s.path("phases.json"), PHASES).unwrap();
        std::fs::write(w.s.path("dry-says"), PLANNED).unwrap();
        std::fs::write(w.s.path(knob), "").unwrap();
        assert_eq!(
            w.m.apply(Applying::Now).unwrap_err().why,
            Why::BuildFailed,
            "{knob}"
        );
        assert!(
            !w.read("calls").contains("keep-units") && !w.read("calls").contains("flake archive"),
            "{knob}: {}",
            w.read("calls")
        );
        assert!(handed(&w).is_some(), "{knob}: the units came first");
        assert_eq!(
            builds(&w).len(),
            usize::from(knob == "build-fails"),
            "{knob}: the system is built only after its units were"
        );
        assert_eq!(w.lock(), LOCK_A, "{knob}");
        assert!(!w.m.places.run.join("units").exists(), "{knob}");
    }
}

/// FAILS IF: a unit is built that Nix would not build for the system —
/// the units below the shell, or the shell, on a machine whose store holds
/// the shell's bundle (freshly installed from the ISO's seed, or fed by the
/// person's binary cache, where no unit ever is), though every one of them
/// is in the system's `.drv` closure — or a unit Nix would build below the
/// shell is left to the system's build instead of being handed over.
#[test]
fn an_apply_builds_only_what_nix_would_build() {
    // The shell's bundle in the store, the system changed: Nix builds the
    // system and nothing below it, and the units' program never runs.
    let w = World::new("phases-fresh");
    std::fs::write(w.s.path("phases.json"), PHASES).unwrap();
    std::fs::write(
        w.s.path("dry-says"),
        "this derivation will be built:\n  /nix/store/tttt-nixos-system-noctua.drv\n",
    )
    .unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    assert_eq!(
        w.m.apply(Applying::Now).unwrap().result,
        ApplyResult::Switched
    );
    let built = builds(&w);
    assert!(handed(&w).is_none(), "no unit and no shell: {built:?}");
    assert_eq!(built.len(), 1, "only the system: {built:?}");
    assert!(
        built[0].contains("--max-jobs 2 --cores 2")
            && built[0].ends_with("/nix/store/tttt-nixos-system-noctua.drv^*"),
        "{}",
        built[0]
    );
    assert!(
        w.read("calls").contains("keep-units"),
        "what the store holds is still kept"
    );

    // The shell held, a unit below it needed by another program: that unit
    // handed over, the shell not at all.
    let w = World::new("phases-unit");
    std::fs::write(w.s.path("phases.json"), PHASES).unwrap();
    std::fs::write(
        w.s.path("dry-says"),
        "these 2 derivations will be built:\n  /nix/store/aaaa-rust-lib-x-0.1.0.drv\n  /nix/store/tttt-nixos-system-noctua.drv\n",
    )
    .unwrap();
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
    assert_eq!(
        handed(&w).unwrap().1,
        "/nix/store/aaaa-rust-lib-x-0.1.0.drv\n/nix/store/tttt-nixos-system-noctua.drv\n",
        "the shell the store holds is not linked again"
    );
    assert_eq!(builds(&w).len(), 1);
}

/// FAILS IF: a distro that describes no phases (older than them) is not
/// built in one step — sized, as a flake, moving no input, handing nothing
/// to the units' program — or a dry run that does not say what the
/// system's build builds (no answer, or words this does not read) hands
/// over less than every unit of the plan, or does not tell the program that
/// they are units only (`--units-only`: it asks Nix what they are built
/// from).
#[test]
fn a_build_without_phases_or_a_plan_is_still_whole_and_sized() {
    let w = World::new("no-phases");
    let b = w.toplevel("b", "quiet");
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
    let built = builds(&w);
    assert_eq!(built.len(), 1, "{built:?}");
    assert!(
        built[0].contains("--no-update-lock-file")
            && built[0].contains("--max-jobs 2 --cores 2")
            && built[0].ends_with("#nixosConfigurations.noctua.config.system.build.toplevel"),
        "{}",
        built[0]
    );
    assert!(handed(&w).is_none(), "no phases, no units handed over");
    assert!(
        !w.read("calls").contains("keep-units"),
        "no phases, no units to keep"
    );

    for (what, knob, says) in [
        ("no-plan", "dry-fails", ""),
        (
            "unread-plan",
            "dry-says",
            "these 4 derivations will be built:\n  /nix/store/tttt-nixos-system-noctua.drv\n",
        ),
    ] {
        let w = World::new(what);
        std::fs::write(w.s.path("phases.json"), PHASES).unwrap();
        std::fs::write(w.s.path(knob), says).unwrap();
        let b = w.toplevel("b", "quiet");
        w.builds(&b);
        w.m.apply(Applying::Now).unwrap();
        assert_eq!(
            handed(&w).unwrap().1,
            "/nix/store/aaaa-rust-lib-x-0.1.0.drv\n/nix/store/ssss-ronoctua-shell-0.1.0.drv\n/nix/store/xxxx-rust-lib-y-0.1.0.drv\n/nix/store/oooo-other-0.1.0.drv\n",
            "every unit, {what}"
        );
        let call = w.read("calls");
        let units = call.lines().find(|l| l.starts_with("build-units")).unwrap();
        assert!(
            units.contains(" --units-only -- "),
            "{what}: no plan, and the program is not told the units are all it has: {units}"
        );
        assert_eq!(builds(&w).len(), 1, "{what}");
    }
}

/// FAILS IF: a distro whose phases are in another format than this program
/// reads — older, or newer (how a newer program arrives) — is refused
/// instead of built in one step, sized and said with both numbers; or one
/// in this program's format with a line it cannot read is built at all,
/// instead of refused with the line and why, the lock put back.
#[test]
fn another_format_is_built_whole_and_a_broken_one_refused() {
    for format in [2, 4] {
        let w = World::new("other-format");
        std::fs::write(
            w.s.path("phases.json"),
            PHASES.replace(r#""format":3"#, &format!(r#""format":{format}"#)),
        )
        .unwrap();
        std::fs::write(w.s.path("dry-says"), PLANNED).unwrap();
        let b = w.toplevel("b", "quiet");
        w.builds(&b);
        let applied = w.m.apply(Applying::Now).unwrap();
        assert_eq!(applied.result, ApplyResult::Switched, "{format}");
        let built = builds(&w);
        assert_eq!(built.len(), 1, "{format}: {built:?}");
        assert!(
            built[0].contains("--max-jobs 2 --cores 2")
                && built[0].ends_with("#nixosConfigurations.noctua.config.system.build.toplevel"),
            "{format}: one step, sized: {}",
            built[0]
        );
        assert!(handed(&w).is_none(), "{format}: no units handed over");
    }

    let w = World::new("broken-format");
    std::fs::write(
        w.s.path("phases.json"),
        PHASES.replace(
            "release/libs/y feather carried 300 /nix/store/xxxx-rust-lib-y-0.1.0.drv",
            "release/libs/y /nix/store/xxxx-rust-lib-y-0.1.0.drv",
        ),
    )
    .unwrap();
    std::fs::write(w.s.path("dry-says"), PLANNED).unwrap();
    std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
    std::fs::create_dir_all(&w.m.places.state).unwrap();
    std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
    assert_eq!(w.m.apply(Applying::Now).unwrap_err().why, Why::BuildFailed);
    assert!(builds(&w).is_empty(), "nothing is built");
    assert!(handed(&w).is_none(), "no unit is handed over");
    assert_eq!(w.lock(), LOCK_A, "the lock is put back");
}

/// FAILS IF: a sizing that does not answer with counts is guessed at instead
/// of refused — before anything is built, with the lock put back.
#[test]
fn a_sizing_that_says_no_count_is_a_refusal() {
    for says in ["", "jobs=3\n", "jobs=0\ncores=1\nheavy_cores=1\n"] {
        let w = World::new("sizing");
        std::fs::write(w.s.path("sizing-says"), says).unwrap();
        std::fs::write(w.m.places.dir.join("flake.lock"), LOCK_B).unwrap();
        std::fs::create_dir_all(&w.m.places.state).unwrap();
        std::fs::write(w.m.places.state.join(crate::APPLIED_LOCK), LOCK_A).unwrap();
        assert_eq!(
            w.m.apply(Applying::Now).unwrap_err().why,
            Why::Failed,
            "{says:?}"
        );
        assert!(builds(&w).is_empty(), "{says:?}");
        assert_eq!(w.lock(), LOCK_A, "{says:?}");
    }
}

/// FAILS IF: on a BIOS machine's GRUB, an apply does not arm the trial in
/// GRUB's environment block — the default on the last good generation's
/// entry, the boot of once on the new one, each named by its title's path as
/// GRUB's `default` takes it — or the trial's verdict does not reach it: the
/// default unset when the trial booted, and when it did not, the profile back
/// and the default unset (the menu's first entry is the last good one again).
#[test]
fn the_trial_runs_on_grub_as_on_systemd_boot() {
    let w = World::grub("grub-arms");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    let applied = w.m.apply(Applying::Now).unwrap();
    assert_eq!(applied.result, ApplyResult::Switched);
    assert_eq!(
        applied.trial.map(|t| t.outcome),
        Some(TrialOutcome::Pending)
    );
    assert_eq!(w.read("efi-default"), World::grub_entry(1));
    assert_eq!(w.read("efi-oneshot"), World::grub_entry(2));
    assert!(
        w.read("calls").contains("grub-editenv"),
        "{}",
        w.read("calls")
    );
    assert!(!w.read("calls").contains("bootctl"), "{}", w.read("calls"));
    let good = w.state().last_good.unwrap();
    assert_eq!(
        (good.toplevel.as_str(), good.entry),
        (a.as_str(), World::grub_entry(1))
    );
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    assert_eq!(w.read("efi-default"), "", "the menu's first entry decides");
    assert_eq!(w.state().last_good.unwrap().entry, World::grub_entry(2));

    // SET FOR THE NEXT BOOT on GRUB: the menu written by `boot`, the trial
    // armed the same way, judged by nothing in the boot that set it — not
    // even with its record lost: `next_entry` still names it.
    let w = World::grub("grub-staged");
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    let applied = w.m.apply(Applying::NextBoot).unwrap();
    assert_eq!(applied.result, ApplyResult::Staged);
    assert_eq!(w.read("switched"), "b boot\n");
    assert_eq!(w.read("efi-default"), World::grub_entry(1));
    assert_eq!(w.read("efi-oneshot"), World::grub_entry(2));
    std::fs::remove_file(w.m.places.state.join(crate::boots::ARMED)).unwrap();
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Pending);
    assert_eq!(w.profile().as_deref(), Some(b.as_str()));
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);

    let w = World::grub("grub-fails");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
    w.reboot(&a);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    assert_eq!(
        w.profile().as_deref(),
        Some(a.as_str()),
        "the profile is back"
    );
    assert_eq!(w.read("efi-default"), "");
}

/// FAILS IF: systemd-boot is not told to restart on a start that fails
/// while a trial is armed — a trial it cannot start would hand the machine
/// to the firmware —, the line outlives the trial's verdict, good or failed
/// (it would restart forever a newest generation that cannot start, with no
/// trial at all), a line NixOS wrote is lost, or the line is written with
/// no trial armed, into a `loader.conf` that is not there, or on GRUB.
#[test]
fn only_a_trial_restarts_a_start_that_fails() {
    let posted = format!("{LOADER_CONF}{}\n", crate::loader::REBOOT_ON_ERROR);
    let conf = "boot/loader/loader.conf";
    let w = World::new("restarts");
    let a = w.m.system.resolve(system::BOOTED).unwrap();
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
    assert_eq!(w.read(conf), posted, "armed");
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Good);
    assert_eq!(w.read(conf), LOADER_CONF, "a trial that booted");
    let c = w.toplevel("c", "quiet c");
    w.builds(&c);
    w.m.apply(Applying::Now).unwrap();
    assert_eq!(w.read(conf), posted, "armed again");
    w.reboot(&b);
    assert_eq!(w.m.bless().unwrap(), TrialOutcome::Failed);
    assert_eq!(w.read(conf), LOADER_CONF, "a trial that did not boot");
    assert_ne!(a, b);

    let w = World::new("restarts-no-efi");
    std::fs::write(w.s.path("no-efi"), "").unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
    assert_eq!(w.read(conf), LOADER_CONF, "no trial, no line");

    let w = World::new("restarts-no-conf");
    std::fs::remove_file(w.s.path(conf)).unwrap();
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    let applied = w.m.apply(Applying::Now).unwrap();
    assert_eq!(
        applied.trial.map(|t| t.outcome),
        Some(TrialOutcome::Pending),
        "the trial is armed all the same"
    );
    assert!(!w.s.path(conf).exists(), "a loader.conf made from nothing");

    let w = World::grub("restarts-grub");
    let b = w.toplevel("b", "quiet b");
    w.builds(&b);
    w.m.apply(Applying::Now).unwrap();
    assert_eq!(w.read(conf), LOADER_CONF, "GRUB has no loader.conf to post");
}

/// FAILS IF: a GRUB whose environment block cannot be written arms a trial
/// it cannot keep, instead of saying so (`no-efi-variables`, said of GRUB).
#[test]
fn a_grub_that_cannot_be_written_arms_no_trial() {
    let w = World::grub("grub-unwritten");
    std::fs::write(w.s.path("no-efi"), "").unwrap();
    let b = w.toplevel("b", "quiet panic=10");
    w.builds(&b);
    let applied = w.m.apply(Applying::Now).unwrap();
    assert_eq!(applied.trial, None);
    assert_eq!(w.state().no_trial, Some(NoTrial::NoEfiVariables));
}
