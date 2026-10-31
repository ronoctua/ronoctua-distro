//! The words handed to every program this crate runs — pure, so what runs as
//! root is readable (and tested) without running it.
//!
//! The machine directory is always given as a `path:` flake. Without the
//! scheme Nix would take a folder that happens to hold a `.git` as a git
//! flake, and build only what git tracks; the machine directory is root's,
//! has no history, and is what it is on the disk.

use std::path::Path;

/// The experimental features the flake commands need, named on every call:
/// the machine's `nix.conf` turns them on, and a `nix` run with another
/// configuration (a test, the installer) must not depend on it.
pub const FEATURES: &str = "nix-command flakes";

/// The attribute path of the system a machine flake builds.
#[must_use]
pub fn toplevel_of(attr: &str) -> String {
    format!("nixosConfigurations.{attr}.config.system.build.toplevel")
}

/// Whether `attr` can name a `nixosConfigurations` attribute without
/// quoting: a user name (`[a-z_][a-z0-9_-]*`, at most 32 bytes).
#[must_use]
pub fn is_attr(attr: &str) -> bool {
    let mut bytes = attr.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'_'))
        && attr.len() <= 32
        && bytes.all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

/// The machine flake at `dir`, as a reference.
#[must_use]
pub fn flake(dir: &Path) -> String {
    format!("path:{}", dir.display())
}

/// Rewrite a flake command so it builds the distro from a local CHECKOUT
/// instead of the machine flake's own `ronoctua` input. The build switch's dev
/// direction (`-reference`) needs this: that input is a user-owned `git+file` repo a root
/// apply may not fetch, and it tracks a branch, so `--no-update-lock-file`
/// would refuse the re-lock. `path:<checkout>` is neither — no git to own, no
/// branch to re-resolve. The override is itself a lock change, so the lock is
/// recomputed in memory and never written. `None` leaves the words untouched,
/// which is every other apply.
#[must_use]
pub fn with_override(mut words: Vec<String>, over: Option<&Path>) -> Vec<String> {
    let Some(dir) = over else {
        return words;
    };
    for word in &mut words {
        if word == "--no-update-lock-file" {
            *word = "--no-write-lock-file".to_owned();
        }
    }
    // Before the flake reference, which every such command ends with.
    let reference = words
        .pop()
        .expect("a flake command ends with its reference");
    words.extend([
        "--override-input".to_owned(),
        "ronoctua".to_owned(),
        flake(dir),
        reference,
    ]);
    words
}

/// How many derivations a build runs at once and how many cores each is
/// given — `--max-jobs` and `--cores`, as the machine's sizing says
/// (`crate::build::Sizing`). Never Nix's own `auto` and `0`: that is every
/// core, times every core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sized {
    /// Derivations at once.
    pub jobs: u32,
    /// Cores each one is given (its `NIX_BUILD_CORES`).
    pub cores: u32,
}

impl Sized {
    fn words(self) -> [String; 4] {
        [
            "--max-jobs".to_owned(),
            self.jobs.to_string(),
            "--cores".to_owned(),
            self.cores.to_string(),
        ]
    }
}

/// `nix build` of the machine's system into `out_link`, sized, never moving
/// an input: a lock that would have to change is a failed build, not a new
/// distro revision nobody asked for.
#[must_use]
pub fn build(dir: &Path, attr: &str, out_link: &Path, sized: Sized) -> Vec<String> {
    let mut words = vec![
        "build".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--no-update-lock-file".to_owned(),
    ];
    words.extend(sized.words());
    words.extend([
        "--out-link".to_owned(),
        out_link.display().to_string(),
        format!("{}#{}", flake(dir), toplevel_of(attr)),
    ]);
    words
}

/// The attribute path of what a machine flake says about how its system is
/// built (`ronoctua.machine.phases`, `nix/modules/machine.nix`).
#[must_use]
pub fn phases_of(attr: &str) -> String {
    format!("nixosConfigurations.{attr}.config.ronoctua.machine.phases")
}

/// `nix eval` of the build's phases, as JSON — the one evaluation of an
/// apply: it writes every `.drv` the builds after it name, and moves no
/// input.
#[must_use]
pub fn phases(dir: &Path, attr: &str) -> Vec<String> {
    vec![
        "eval".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--no-update-lock-file".to_owned(),
        "--json".to_owned(),
        format!("{}#{}", flake(dir), phases_of(attr)),
    ]
}

/// `nix build` of derivations already evaluated (`<drv>^*`, every output),
/// sized, into `out_link` (and `out_link-1`, … for more than one) — which
/// holds them while it exists.
#[must_use]
pub fn build_drvs(drvs: &[String], sized: Sized, out_link: &Path) -> Vec<String> {
    let mut words = vec![
        "build".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
    ];
    words.extend(sized.words());
    words.extend(["--out-link".to_owned(), out_link.display().to_string()]);
    words.extend(drvs.iter().map(|d| format!("{d}^*")));
    words
}

/// `nix build --dry-run` of derivations already evaluated (`<drv>^*`):
/// Nix's own plan for building them, on its standard error
/// ([`crate::dry::to_build`]) — what is missing and must be built, never
/// what the store already holds or anything only below it.
#[must_use]
pub fn dry_run_drvs(drvs: &[String]) -> Vec<String> {
    let mut words = vec![
        "build".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--dry-run".to_owned(),
    ];
    words.extend(drvs.iter().map(|d| format!("{d}^*")));
    words
}

/// `nix derivation show` of derivations already evaluated: what each is —
/// its name, its environment and, for a download, the hash its output is
/// pinned to ([`ronoctua_person::frontier::downloads`]).
#[must_use]
pub fn show(drvs: &[String]) -> Vec<String> {
    let mut words = vec![
        "derivation".to_owned(),
        "show".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
    ];
    words.extend(drvs.iter().cloned());
    words
}

/// `nix-store --query --requisites --include-outputs` of a derivation:
/// everything it is built from, and every output of those the store holds.
#[must_use]
pub fn requisites(drv: &str) -> Vec<String> {
    vec![
        "--query".to_owned(),
        "--requisites".to_owned(),
        "--include-outputs".to_owned(),
        drv.to_owned(),
    ]
}

/// `nix path-info --json` of the paths on standard input, in the format
/// [`crate::build`] reads (`--json-format 2`): what each is, its content
/// address among it.
#[must_use]
pub fn path_info_of_input() -> Vec<String> {
    vec![
        "path-info".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--json".to_owned(),
        "--json-format".to_owned(),
        "2".to_owned(),
        "--stdin".to_owned(),
    ]
}

/// `nix store prefetch-file` of one download from `url` — a file the person
/// handed in (`file://`) or a mirror — under its own name and the hash the
/// flake pins: Nix refuses other bytes, so the path that comes out can only
/// be the download's. `--unpack` for a tree's hash.
#[must_use]
pub fn prefetch(name: &str, hash: &str, unpack: bool, url: &str) -> Vec<String> {
    let mut words = vec![
        "store".to_owned(),
        "prefetch-file".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--json".to_owned(),
        "--name".to_owned(),
        name.to_owned(),
        "--expected-hash".to_owned(),
        hash.to_owned(),
    ];
    if unpack {
        words.push("--unpack".to_owned());
    }
    words.push(url.to_owned());
    words
}

/// `nix-store --query --outputs` of one derivation: the store path its
/// output becomes.
#[must_use]
pub fn outputs(drv: &str) -> Vec<String> {
    vec!["--query".to_owned(), "--outputs".to_owned(), drv.to_owned()]
}

/// `ronoctua-build-units` building, of the plan in `plan`, the units `will`
/// lists (one `.drv` a line), each its own `nix build` by `nix` — taking the
/// features every call here takes —, their outputs held under `held`.
/// `will` is a dry run's whole answer (the units, and what is no unit, of
/// which the program builds first what the units are built from), or, with
/// `units_only`, units and nothing else: no dry run answered, and the
/// program asks Nix what they are built from.
#[must_use]
pub fn build_units(
    nix: &Path,
    plan: &Path,
    will: &Path,
    held: &Path,
    units_only: bool,
) -> Vec<String> {
    let mut words = vec![
        "--nix".to_owned(),
        nix.display().to_string(),
        plan.display().to_string(),
        will.display().to_string(),
        held.display().to_string(),
    ];
    if units_only {
        words.push("--units-only".to_owned());
    }
    words.extend([
        "--".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
    ]);
    words
}

/// `nix flake archive` of a machine flake, only measured, as JSON: the
/// store path of the flake and of every input, an input's inputs included,
/// read off the lock — `--dry-run` fetches nothing, and no input moves.
#[must_use]
pub fn sources(dir: &Path) -> Vec<String> {
    vec![
        "flake".to_owned(),
        "archive".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--no-update-lock-file".to_owned(),
        "--dry-run".to_owned(),
        "--json".to_owned(),
        flake(dir),
    ]
}

/// `ronoctua-keep-units` rooting, under `roots`, the set `set` of the paths
/// listed in `paths` that the store holds.
#[must_use]
pub fn keep_units(roots: &Path, set: &str, paths: &Path) -> Vec<String> {
    vec![
        "--roots".to_owned(),
        roots.display().to_string(),
        "--set".to_owned(),
        set.to_owned(),
        paths.display().to_string(),
    ]
}

/// The same build, only measured: what would be downloaded and built.
#[must_use]
pub fn dry_run(dir: &Path, attr: &str) -> Vec<String> {
    vec![
        "build".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--no-update-lock-file".to_owned(),
        "--dry-run".to_owned(),
        format!("{}#{}", flake(dir), toplevel_of(attr)),
    ]
}

/// `nix flake lock` of a machine flake that has no lock yet, STARTED FROM
/// `reference` — the distro's own lock (`--reference-lock-file`): Nix keeps
/// every input whose node there has the `original` the flake names (the
/// `nixpkgs` the machine rolls, at the distro's tested pin) and locks the
/// rest, the distro input, fresh. Locked on its own, the template's
/// `nixos-unstable` is a branch only GitHub resolves: offline the lock
/// fails, and online it jumps to the newest unstable. The lock is written
/// beside the flake, said rather than left to Nix's default.
#[must_use]
pub fn lock(dir: &Path, reference: &Path) -> Vec<String> {
    vec![
        "flake".to_owned(),
        "lock".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        "--reference-lock-file".to_owned(),
        reference.display().to_string(),
        "--output-lock-file".to_owned(),
        dir.join("flake.lock").display().to_string(),
        flake(dir),
    ]
}

/// `nix flake update` of the distro input AND `nixpkgs` — the machine rolls on
/// nixos-unstable now (owner amendment 2026-10-30), so the weekly
/// `flake-update` step moves both; done by a rehearsal on its copy.
#[must_use]
pub fn update_distro(dir: &Path) -> Vec<String> {
    vec![
        "flake".to_owned(),
        "update".to_owned(),
        "--extra-experimental-features".to_owned(),
        FEATURES.to_owned(),
        crate::DISTRO_INPUT.to_owned(),
        crate::NIXPKGS_INPUT.to_owned(),
        "--flake".to_owned(),
        flake(dir),
    ]
}

/// `nix-env` making `toplevel` the profile's newest generation (none is
/// made when it already is).
#[must_use]
pub fn set_profile(profile: &Path, toplevel: &str) -> Vec<String> {
    vec![
        "--profile".to_owned(),
        profile.display().to_string(),
        "--set".to_owned(),
        toplevel.to_owned(),
    ]
}

/// `nix-env` making generation `number` the profile's current one again,
/// as `--rollback` does to the one before: no generation is made.
#[must_use]
pub fn switch_generation(profile: &Path, number: u64) -> Vec<String> {
    vec![
        "--profile".to_owned(),
        profile.display().to_string(),
        "--switch-generation".to_owned(),
        number.to_string(),
    ]
}

/// `bootctl list`, as JSON.
#[must_use]
pub fn list_entries() -> Vec<String> {
    vec![
        "--no-pager".to_owned(),
        "list".to_owned(),
        "--json=short".to_owned(),
    ]
}

/// `bootctl set-default <entry>`: the EFI variable `LoaderEntryDefault`,
/// which wins over `loader.conf`'s `default`. An empty entry unsets it, and
/// `loader.conf` — the profile's newest generation — is the default again.
#[must_use]
pub fn set_default(entry: &str) -> Vec<String> {
    vec!["set-default".to_owned(), entry.to_owned()]
}

/// `bootctl set-oneshot <entry>`: `LoaderEntryOneShot`, which the boot
/// loader reads and REMOVES at the next boot — the trial is tried once.
#[must_use]
pub fn set_oneshot(entry: &str) -> Vec<String> {
    vec!["set-oneshot".to_owned(), entry.to_owned()]
}

/// `grub-editenv <block> create`: GRUB's environment block, made.
#[must_use]
pub fn grub_create(block: &std::path::Path) -> Vec<String> {
    vec![block.display().to_string(), "create".to_owned()]
}

/// `grub-editenv <block> set <name>=<value>`, or `unset <name>` for an empty
/// value — GRUB's `set-default` and `set-oneshot` (`crate::loader`).
#[must_use]
pub fn grub_set(block: &std::path::Path, name: &str, value: &str) -> Vec<String> {
    let block = block.display().to_string();
    if value.is_empty() {
        vec![block, "unset".to_owned(), name.to_owned()]
    } else {
        vec![block, "set".to_owned(), format!("{name}={value}")]
    }
}

/// `grub-editenv <block> list`: every variable of GRUB's environment block,
/// one `name=value` a line.
#[must_use]
pub fn grub_list(block: &std::path::Path) -> Vec<String> {
    vec![block.display().to_string(), "list".to_owned()]
}

/// How a toplevel is switched to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    /// Activate now and install the boot loader.
    Switch,
    /// Only install the boot loader: the next boot is it.
    Boot,
}

/// The words of `<toplevel>/bin/switch-to-configuration`.
#[must_use]
pub fn switch(how: Switch) -> Vec<String> {
    vec![
        match how {
            Switch::Switch => "switch",
            Switch::Boot => "boot",
        }
        .to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a download is added without the pinned hash — Nix would
    /// take any bytes under the download's name — or a flat file is unpacked.
    #[test]
    fn a_download_is_added_only_by_its_pinned_hash() {
        let flat = prefetch("x.tar.gz", "sha256-AAAA", false, "https://m.org/x.tar.gz");
        let at = flat
            .iter()
            .position(|a| a == "--expected-hash")
            .expect("the hash");
        assert_eq!(flat[at + 1], "sha256-AAAA");
        assert!(!flat.iter().any(|a| a == "--unpack"));
        assert_eq!(
            flat.last().map(String::as_str),
            Some("https://m.org/x.tar.gz")
        );
        assert!(
            prefetch("t", "sha256-B", true, "file:///x")
                .iter()
                .any(|a| a == "--unpack")
        );
    }
    use std::path::PathBuf;

    /// FAILS IF: a new machine flake is locked without the distro's own
    /// lock as the reference, writes its lock anywhere but beside its
    /// `flake.nix`, or names another flake than the folder — the template's
    /// rolling `nixpkgs` would then be resolved on the day (no network: no
    /// lock; a network: the newest unstable, not the tested pin).
    #[test]
    fn a_new_flake_is_locked_from_the_distros_own() {
        let words = lock(
            Path::new("/etc/nixos/.next"),
            Path::new("/nix/store/zzzz-flake.lock"),
        );
        let after = |flag: &str| {
            words
                .iter()
                .position(|w| w == flag)
                .and_then(|i| words.get(i + 1))
                .map(String::as_str)
        };
        assert_eq!(words[..2], ["flake", "lock"]);
        assert_eq!(
            after("--reference-lock-file"),
            Some("/nix/store/zzzz-flake.lock")
        );
        assert_eq!(
            after("--output-lock-file"),
            Some("/etc/nixos/.next/flake.lock")
        );
        assert_eq!(
            words.last().map(String::as_str),
            Some("path:/etc/nixos/.next")
        );
        assert!(
            !words.iter().any(|w| w == "--update-input" || w == "update"),
            "{words:?}"
        );
    }

    const SIZED: Sized = Sized { jobs: 3, cores: 2 };

    #[test]
    fn the_build_never_moves_an_input_and_reads_the_folder_as_it_is() {
        let words = build(
            Path::new("/etc/nixos"),
            "noctua",
            Path::new("/run/ronoctua-update/next"),
            SIZED,
        );
        assert!(words.contains(&"--no-update-lock-file".to_owned()));
        assert_eq!(
            words.last().unwrap(),
            "path:/etc/nixos#nixosConfigurations.noctua.config.system.build.toplevel"
        );
        let at = words.iter().position(|w| w == "--out-link").unwrap();
        assert_eq!(words[at + 1], "/run/ronoctua-update/next");
        let jobs = words.iter().position(|w| w == "--max-jobs").unwrap();
        assert_eq!(words[jobs + 1..jobs + 4], ["3", "--cores", "2"]);
        let dry = dry_run(Path::new("/run/x"), "noctua");
        assert!(
            dry.contains(&"--dry-run".to_owned())
                && dry.contains(&"--no-update-lock-file".to_owned())
        );
        assert!(
            !dry.contains(&"--out-link".to_owned()),
            "a rehearsal leaves no root"
        );
    }

    #[test]
    fn an_override_points_ronoctua_at_a_checkout_and_stops_writing_the_lock() {
        let base = build(
            Path::new("/etc/nixos"),
            "noctua",
            Path::new("/run/next"),
            SIZED,
        );
        // With no override the words are untouched.
        assert_eq!(with_override(base.clone(), None), base);
        let over = with_override(base.clone(), Some(Path::new("/home/p/distro")));
        // The lock no longer may move AT ALL; it is recomputed, not written.
        assert!(!over.contains(&"--no-update-lock-file".to_owned()));
        assert!(over.contains(&"--no-write-lock-file".to_owned()));
        // `ronoctua` is overridden to the checkout as a `path:` — no git.
        let at = over
            .iter()
            .position(|w| w == "--override-input")
            .expect("the override is named");
        assert_eq!(over[at + 1], "ronoctua");
        assert_eq!(over[at + 2], "path:/home/p/distro");
        // The flake reference is still the last word, as nix wants it.
        assert_eq!(over.last().unwrap(), base.last().unwrap());
        // And `sources`, which keeps what was built, is rewritten the same way.
        let src = with_override(sources(Path::new("/etc/nixos")), Some(Path::new("/d")));
        assert!(src.contains(&"--no-write-lock-file".to_owned()));
        assert_eq!(src.last().unwrap(), "path:/etc/nixos");
    }

    #[test]
    fn the_update_moves_the_distro_and_nixpkgs() {
        let words = update_distro(&PathBuf::from("/run/ronoctua-update/rehearsal"));
        let at = words.iter().position(|w| w == "update").unwrap();
        assert_eq!(words[at + 3], crate::DISTRO_INPUT);
        assert_eq!(words[at + 4], crate::NIXPKGS_INPUT);
        assert_eq!(words.last().unwrap(), "path:/run/ronoctua-update/rehearsal");
    }

    #[test]
    fn an_attribute_is_a_user_name() {
        for good in ["noctua", "_x", "a-b_9"] {
            assert!(is_attr(good), "{good}");
        }
        for bad in ["", "Noctua", "9x", "a.b", "a\"b", "a b", &"x".repeat(33)] {
            assert!(!is_attr(bad), "{bad}");
        }
    }

    #[test]
    fn the_phases_are_evaluated_once_and_built_by_their_derivations() {
        let eval = phases(Path::new("/etc/nixos"), "noctua");
        assert_eq!(eval[0], "eval");
        assert!(eval.contains(&"--no-update-lock-file".to_owned()));
        assert!(eval.contains(&"--json".to_owned()));
        assert_eq!(
            eval.last().unwrap(),
            "path:/etc/nixos#nixosConfigurations.noctua.config.ronoctua.machine.phases"
        );
        let built = build_drvs(
            &["/nix/store/a.drv".to_owned(), "/nix/store/b.drv".to_owned()],
            Sized { jobs: 1, cores: 7 },
            Path::new("/run/u/next"),
        );
        assert_eq!(
            built[3..],
            [
                "--max-jobs",
                "1",
                "--cores",
                "7",
                "--out-link",
                "/run/u/next",
                "/nix/store/a.drv^*",
                "/nix/store/b.drv^*"
            ]
        );
        assert_eq!(
            dry_run_drvs(&["/nix/store/s.drv".to_owned()]),
            [
                "build",
                "--extra-experimental-features",
                FEATURES,
                "--dry-run",
                "/nix/store/s.drv^*"
            ]
        );
        for units_only in [false, true] {
            let mut want = vec![
                "--nix",
                "/nix/store/n/bin/nix",
                "/run/u/plan",
                "/run/u/will",
                "/run/u/built",
            ];
            if units_only {
                want.push("--units-only");
            }
            want.extend(["--", "--extra-experimental-features", FEATURES]);
            assert_eq!(
                build_units(
                    Path::new("/nix/store/n/bin/nix"),
                    Path::new("/run/u/plan"),
                    Path::new("/run/u/will"),
                    Path::new("/run/u/built"),
                    units_only
                ),
                want
            );
        }
        assert_eq!(
            keep_units(Path::new("/var/k"), "release", Path::new("/run/u/kept")),
            ["--roots", "/var/k", "--set", "release", "/run/u/kept"]
        );
    }

    #[test]
    fn bootctl_and_the_switch_take_fixed_words() {
        assert_eq!(set_default(""), ["set-default", ""]);
        assert_eq!(
            set_oneshot("nixos-generation-7.conf"),
            ["set-oneshot", "nixos-generation-7.conf"]
        );
        assert_eq!(list_entries(), ["--no-pager", "list", "--json=short"]);
        let block = std::path::Path::new("/boot/grub/grubenv");
        assert_eq!(grub_create(block), ["/boot/grub/grubenv", "create"]);
        assert_eq!(grub_list(block), ["/boot/grub/grubenv", "list"]);
        assert_eq!(
            grub_set(block, "next_entry", "a>b c (1)"),
            ["/boot/grub/grubenv", "set", "next_entry=a>b c (1)"]
        );
        assert_eq!(
            grub_set(block, "ronoctua_default", ""),
            ["/boot/grub/grubenv", "unset", "ronoctua_default"]
        );
        assert_eq!(switch(Switch::Boot), ["boot"]);
        assert_eq!(
            set_profile(Path::new("/nix/var/nix/profiles/system"), "/nix/store/a-b"),
            [
                "--profile",
                "/nix/var/nix/profiles/system",
                "--set",
                "/nix/store/a-b"
            ]
        );
    }
}
