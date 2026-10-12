//! What the boot menu lists, and whether two systems boot the same way.
//!
//! NixOS's systemd-boot builder writes one entry per generation of the
//! system profile — `nixos-generation-<n>.conf`, and one more per
//! specialisation — whose `options` begin with `init=<toplevel>/init`. That
//! `init=` is how an entry is found for a toplevel: `bootctl list` says it
//! for every entry, and nothing else names the system an entry boots.
//!
//! The menu lists the newest `configurationLimit` generations (the store
//! module caps it at the count the trim keeps), so a generation is in the
//! menu exactly when it is among the newest that many.

use std::path::Path;

use serde::Deserialize;

use crate::System;

/// One boot entry, as `bootctl list` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its id (the file name: `nixos-generation-41.conf`).
    pub id: String,
    /// The toplevel its `init=` names, if it names one.
    pub toplevel: Option<String>,
}

/// `bootctl list --json=short`'s entry; `options` is one string in the
/// systemd this distro pins, and a list in others.
#[derive(Deserialize)]
struct Listed {
    id: String,
    #[serde(default)]
    options: Option<serde_json::Value>,
}

/// The entries `bootctl list --json=short` printed; `None` when it is not
/// that JSON.
#[must_use]
pub fn entries(json: &str) -> Option<Vec<Entry>> {
    let listed: Vec<Listed> = serde_json::from_str(json.trim()).ok()?;
    Some(
        listed
            .into_iter()
            .map(|l| {
                let options = match l.options {
                    Some(serde_json::Value::String(s)) => s,
                    Some(serde_json::Value::Array(words)) => words
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" "),
                    _ => String::new(),
                };
                Entry {
                    id: l.id,
                    toplevel: init_of(&options),
                }
            })
            .collect(),
    )
}

/// The toplevel `init=<toplevel>/init` names in a kernel command line: a
/// path straight under the store, as every NixOS entry writes it.
#[must_use]
pub fn init_of(options: &str) -> Option<String> {
    options
        .split_whitespace()
        .find_map(|w| w.strip_prefix("init="))
        .and_then(|init| init.strip_suffix("/init"))
        .filter(|t| {
            t.strip_prefix("/nix/store/")
                .is_some_and(|name| !name.is_empty() && !name.contains('/'))
        })
        .map(str::to_owned)
}

/// The generation an entry's id belongs to (`nixos-generation-41.conf`,
/// `nixos-generation-41-specialisation-x.conf` → 41), for the system
/// profile's entries only.
#[must_use]
pub fn generation_of(id: &str) -> Option<u64> {
    let rest = id.strip_prefix("nixos-generation-")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let after = &rest[digits.len()..];
    if digits.is_empty() || !(after == ".conf" || after.starts_with("-specialisation-")) {
        return None;
    }
    digits.parse().ok()
}

/// The entry that boots `toplevel`: the newest generation's, and its own
/// rather than a specialisation's.
#[must_use]
pub fn entry_of<'a>(entries: &'a [Entry], toplevel: &str) -> Option<&'a str> {
    entries
        .iter()
        .filter(|e| e.toplevel.as_deref() == Some(toplevel))
        .max_by_key(|e| {
            (
                !e.id.contains("-specialisation-"),
                generation_of(&e.id).unwrap_or(0),
            )
        })
        .map(|e| e.id.as_str())
}

/// The id the builder gives generation `n`'s entry — what is recorded when
/// `bootctl` cannot be asked.
#[must_use]
pub fn entry_of_generation(n: u64) -> String {
    format!("nixos-generation-{n}.conf")
}

/// One generation of the system profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    /// Its number (`system-<n>-link`).
    pub number: u64,
    /// Its toplevel, as the system names it; `None` for a dangling link.
    pub toplevel: Option<String>,
}

/// The system profile's generations, oldest first.
#[must_use]
pub fn generations(system: &System) -> Vec<Generation> {
    let profile = system.profile();
    let Some(dir) = profile.parent() else {
        return Vec::new();
    };
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Generation> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let number = name
                .strip_prefix("system-")?
                .strip_suffix("-link")?
                .parse()
                .ok()?;
            Some(Generation {
                number,
                toplevel: std::fs::canonicalize(e.path())
                    .ok()
                    .map(|p| system.logical(&p)),
            })
        })
        .collect();
    found.sort_by_key(|g| g.number);
    found
}

/// The newest generation whose toplevel is `toplevel`.
#[must_use]
pub fn generation_of_toplevel(generations: &[Generation], toplevel: &str) -> Option<u64> {
    generations
        .iter()
        .rev()
        .find(|g| g.toplevel.as_deref() == Some(toplevel))
        .map(|g| g.number)
}

/// Whether making a new generation (`adds`) would push `last_good` out of a
/// menu that lists the newest `limit` generations.
///
/// A last good generation the menu does not list NOW is not pushed out by
/// anything: there is no way back to lose, and refusing would only stop the
/// machine from ever updating again.
#[must_use]
pub fn falls_out(generations: &[Generation], adds: bool, limit: u64, last_good: &str) -> bool {
    let limit = usize::try_from(limit).unwrap_or(usize::MAX).max(1);
    let mut numbers: Vec<u64> = generations.iter().map(|g| g.number).collect();
    numbers.sort_unstable();
    let ours: Vec<u64> = generations
        .iter()
        .filter(|g| g.toplevel.as_deref() == Some(last_good))
        .map(|g| g.number)
        .collect();
    let listed =
        |numbers: &[u64]| -> Vec<u64> { numbers[numbers.len().saturating_sub(limit)..].to_vec() };
    let now = listed(&numbers);
    if !ours.iter().any(|n| now.contains(n)) {
        return false;
    }
    if adds {
        numbers.push(numbers.last().map_or(1, |n| n + 1));
    }
    let then = listed(&numbers);
    !ours.iter().any(|n| then.contains(n))
}

/// Whether `a` and `b` boot the same way: the same kernel, the same initrd
/// and the same kernel parameters. Their `init=` differs (it names each
/// system), and that half is what the switch already ran — so a new system
/// that boots like the last good one needs no reboot to be trusted.
#[must_use]
pub fn same_boot(system: &System, a: &str, b: &str) -> bool {
    let (a, b) = (system.real(a), system.real(b));
    let link = |top: &Path, name: &str| std::fs::canonicalize(top.join(name)).ok();
    let text = |top: &Path| std::fs::read(top.join("kernel-params")).ok();
    let (ka, kb) = (link(&a, "kernel"), link(&b, "kernel"));
    let (ia, ib) = (link(&a, "initrd"), link(&b, "initrd"));
    let (pa, pb) = (text(&a), text(&b));
    ka.is_some() && ia.is_some() && pa.is_some() && ka == kb && ia == ib && pa == pb
}

/// The toplevel and every specialisation's: a boot of any of them is a boot
/// of that generation.
#[must_use]
pub fn tops_of(system: &System, toplevel: &str) -> Vec<String> {
    let mut tops = vec![toplevel.to_owned()];
    if let Ok(read) = std::fs::read_dir(system.real(toplevel).join("specialisation")) {
        for e in read.filter_map(Result::ok) {
            if let Ok(p) = std::fs::canonicalize(e.path()) {
                tops.push(system.logical(&p));
            }
        }
    }
    tops
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTED: &str = r#"[{"type":"type1","source":"esp","id":"nixos-generation-7.conf","title":"NixOS","options":"init=/nix/store/aaaa-nixos-system-7/init quiet loglevel=3 panic=10","isDefault":true},
{"type":"type1","id":"nixos-generation-7-specialisation-x.conf","options":"init=/nix/store/xxxx-spec/init quiet"},
{"type":"type1","id":"nixos-generation-6.conf","options":["init=/nix/store/bbbb-nixos-system-6/init","quiet"]},
{"type":"type1","id":"nixos-generation-5.conf","options":"init=/nix/store/aaaa-nixos-system-7/init quiet"},
{"type":"loader","id":"auto-reboot-to-firmware-setup"}]"#;

    #[test]
    fn an_entry_is_found_by_its_init() {
        let e = entries(LISTED).unwrap();
        assert_eq!(e.len(), 5);
        assert_eq!(
            entry_of(&e, "/nix/store/aaaa-nixos-system-7"),
            Some("nixos-generation-7.conf"),
            "the newest of two that boot it"
        );
        assert_eq!(
            entry_of(&e, "/nix/store/bbbb-nixos-system-6"),
            Some("nixos-generation-6.conf"),
            "options as a list"
        );
        assert_eq!(
            entry_of(&e, "/nix/store/xxxx-spec"),
            Some("nixos-generation-7-specialisation-x.conf")
        );
        assert_eq!(entry_of(&e, "/nix/store/zzzz"), None);
        assert_eq!(e[4].toplevel, None, "an entry with no init= names nothing");
        assert!(entries("not json").is_none());
    }

    #[test]
    fn a_generation_is_read_off_its_entry_alone() {
        assert_eq!(generation_of("nixos-generation-41.conf"), Some(41));
        assert_eq!(
            generation_of("nixos-generation-41-specialisation-x.conf"),
            Some(41)
        );
        for not in [
            "nixos-generation-.conf",
            "nixos-generation-41.efi",
            "nixos-work-generation-3.conf",
            "auto-windows",
        ] {
            assert_eq!(generation_of(not), None, "{not}");
        }
        assert_eq!(entry_of_generation(9), "nixos-generation-9.conf");
        assert_eq!(
            init_of("quiet init=/nix/store/a-b/init x"),
            Some("/nix/store/a-b".to_owned())
        );
        assert_eq!(init_of("init=/nix/store/a-b/sbin/init"), None);
    }

    fn gens(tops: &[(u64, &str)]) -> Vec<Generation> {
        tops.iter()
            .map(|&(number, t)| Generation {
                number,
                toplevel: Some(t.to_owned()),
            })
            .collect()
    }

    #[test]
    fn the_last_good_one_falls_out_only_from_a_full_menu() {
        // Menu of 3: 4, 5, 6 are listed. The last good one is 4.
        let g = gens(&[(3, "/s/c"), (4, "/s/good"), (5, "/s/t1"), (6, "/s/t2")]);
        assert!(falls_out(&g, true, 3, "/s/good"), "a seventh pushes 4 out");
        assert!(
            !falls_out(&g, false, 3, "/s/good"),
            "no new generation, nothing moves"
        );
        assert!(!falls_out(&g, true, 4, "/s/good"), "a menu of 4 keeps it");
        // The same toplevel twice: the newer of the two still listed.
        let twice = gens(&[(4, "/s/good"), (5, "/s/t1"), (6, "/s/good")]);
        assert!(!falls_out(&twice, true, 3, "/s/good"));
        // Not listed now: there is no way back to lose.
        let gone = gens(&[(1, "/s/good"), (5, "/s/a"), (6, "/s/b"), (7, "/s/c")]);
        assert!(!falls_out(&gone, true, 3, "/s/good"));
        assert!(!falls_out(&[], true, 3, "/s/good"));
        assert_eq!(generation_of_toplevel(&twice, "/s/good"), Some(6));
        assert_eq!(generation_of_toplevel(&twice, "/s/none"), None);
    }

    #[test]
    fn two_systems_boot_alike_when_kernel_initrd_and_parameters_agree() {
        let root = crate::tests::Scratch::new("same-boot");
        let system = root.system();
        let store = root.path("nix/store");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("k"), b"kernel").unwrap();
        std::fs::write(store.join("i"), b"initrd").unwrap();
        std::fs::write(store.join("i2"), b"initrd2").unwrap();
        let top = |name: &str, initrd: &str, params: &str| {
            let t = store.join(name);
            std::fs::create_dir_all(&t).unwrap();
            std::os::unix::fs::symlink(store.join("k"), t.join("kernel")).unwrap();
            std::os::unix::fs::symlink(store.join(initrd), t.join("initrd")).unwrap();
            std::fs::write(t.join("kernel-params"), params).unwrap();
            format!("/nix/store/{name}")
        };
        let a = top("a", "i", "quiet panic=10");
        let b = top("b", "i", "quiet panic=10");
        let c = top("c", "i2", "quiet panic=10");
        let d = top("d", "i", "quiet");
        assert!(same_boot(&system, &a, &b));
        assert!(!same_boot(&system, &a, &c), "another initrd");
        assert!(!same_boot(&system, &a, &d), "other parameters");
        assert!(
            !same_boot(&system, &a, "/nix/store/missing"),
            "nothing is not the same"
        );
    }
}
