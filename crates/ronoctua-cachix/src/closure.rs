//! What a push may carry — pure functions of `nix path-info`'s answer.
//!
//! # A public cache is redistribution
//!
//! Whatever a PUBLIC cache holds, anybody downloads. So it receives the
//! distro's own outputs ([`crate::system::System::public_roots`]) and their
//! closure, minus ([`public`]):
//!
//! - every CONTENT-ADDRESSED path (`ca` set): a download — a source, a flake
//!   input, a release asset — or a file evaluation wrote. Whoever needs it
//!   fetches it where it came from; a cache of the person's re-publishing
//!   other people's downloads grows by what is not theirs;
//! - every UNFREE program, by the name `nix/release-unfree.nix` calls unfree
//!   (and with Android on, every name beginning with `android`): a public
//!   cache would be redistributing it, which their licences forbid;
//! - and EVERYTHING THAT REFERS TO ONE OF THEM, because `cachix push` pushes
//!   a path WITH its closure: a wrapper that names Chrome would carry Chrome
//!   along. What is left is closed under references by construction.
//!
//! A PRIVATE cache is not redistribution: it receives the system's whole
//! closure ([`private`]). And neither receives what `cache.nixos.org`
//! already serves (`crate::push` asks it, and keeps the rest).

use std::collections::{BTreeMap, BTreeSet};

use crate::Why;

/// What `nix path-info` says of one store path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Whether it is content-addressed: a download, or written by evaluation.
    pub content_addressed: bool,
    /// What it refers to (absolute store paths).
    pub references: Vec<String>,
    /// Its NAR's size, in bytes.
    pub bytes: u64,
}

/// A closure: every path, with what `nix path-info` said of it.
pub type Closure = BTreeMap<String, Info>;

/// `nix path-info --json --recursive`, in any of the shapes Nix has printed
/// it: a list of objects with `path` (before 2.19), an object keyed by path
/// (`--json-format 1`, what the push asks for), or format 2's
/// `{"info": {<base name>: …}, "version": 2}` with `ca` an object. Every path
/// absolute.
///
/// # Errors
///
/// [`Why::StoreFailed`] for text that is none of them, an empty closure, or
/// a path Nix called invalid (`null`).
pub fn closure_of(text: &str) -> Result<Closure, Why> {
    let data: serde_json::Value = serde_json::from_str(text).map_err(|_| Why::StoreFailed)?;
    let entries: Vec<(String, serde_json::Value)> = match data {
        serde_json::Value::Array(list) => list
            .into_iter()
            .map(|e| {
                let path = e
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                path.map(|p| (p, e)).ok_or(Why::StoreFailed)
            })
            .collect::<Result<_, _>>()?,
        serde_json::Value::Object(mut map) => {
            let format_2 = map.get("version").and_then(serde_json::Value::as_u64) == Some(2)
                && map.get("info").is_some_and(serde_json::Value::is_object);
            if format_2 {
                match map.remove("info") {
                    Some(serde_json::Value::Object(info)) => info.into_iter().collect(),
                    _ => return Err(Why::StoreFailed),
                }
            } else {
                map.into_iter().collect()
            }
        }
        _ => return Err(Why::StoreFailed),
    };
    if entries.is_empty() {
        return Err(Why::StoreFailed);
    }
    let mut out = Closure::new();
    for (path, info) in entries {
        if info.is_null() {
            return Err(Why::StoreFailed);
        }
        let store = info
            .get("storeDir")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("/nix/store")
            .to_owned();
        let absolute = |p: &str| {
            if p.starts_with('/') {
                p.to_owned()
            } else {
                format!("{store}/{p}")
            }
        };
        let references = info
            .get("references")
            .and_then(serde_json::Value::as_array)
            .map(|refs| {
                refs.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(absolute)
                    .collect()
            })
            .unwrap_or_default();
        out.insert(
            absolute(&path),
            Info {
                content_addressed: info.get("ca").is_some_and(|ca| !ca.is_null()),
                references,
                bytes: info
                    .get("narSize")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            },
        );
    }
    Ok(out)
}

/// A store path's name, without the store and the hash. Pure.
#[must_use]
pub fn name_of(path: &str) -> &str {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.split_once('-').map_or(base, |(_, name)| name)
}

/// The package name of a store name, as `builtins.parseDrvName` reads it:
/// everything before the first dash that is followed by a character that is
/// not a letter (`google-chrome-141.0` → `google-chrome`). Pure.
#[must_use]
pub fn pname_of(name: &str) -> &str {
    let b = name.as_bytes();
    for (i, c) in b.iter().enumerate() {
        if *c == b'-' && b.get(i + 1).is_some_and(|n| !n.is_ascii_alphabetic()) {
            return &name[..i];
        }
    }
    name
}

/// Why a path may not go to a public cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Forbidden {
    /// Content-addressed: a download, or a file evaluation wrote.
    Download,
    /// An unfree program.
    Unfree,
}

/// What may be pushed, and why the rest may not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushSet {
    /// What may go, sorted.
    pub pushable: Vec<String>,
    /// What may not: the forbidden paths, and everything that refers to one.
    pub tainted: BTreeSet<String>,
    /// The forbidden paths themselves, with why.
    pub forbidden: BTreeMap<String, Forbidden>,
}

/// What a PUBLIC cache may receive of `closure` — see the module's header.
/// `unfree` holds package names (no versions); `android` adds every name
/// beginning with `android`. Pure.
#[must_use]
pub fn public(closure: &Closure, unfree: &[String], android: bool) -> PushSet {
    let unfree: BTreeSet<&str> = unfree.iter().map(String::as_str).collect();
    let mut forbidden = BTreeMap::new();
    for (path, info) in closure {
        let pname = pname_of(name_of(path));
        if info.content_addressed {
            forbidden.insert(path.clone(), Forbidden::Download);
        } else if unfree.contains(pname) || (android && pname.starts_with("android")) {
            forbidden.insert(path.clone(), Forbidden::Unfree);
        }
    }
    let mut referrers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (path, info) in closure {
        for r in &info.references {
            if r != path && closure.contains_key(r) {
                referrers.entry(r.as_str()).or_default().push(path.as_str());
            }
        }
    }
    let mut tainted: BTreeSet<String> = forbidden.keys().cloned().collect();
    let mut stack: Vec<String> = tainted.iter().cloned().collect();
    while let Some(here) = stack.pop() {
        for up in referrers.get(here.as_str()).into_iter().flatten() {
            if tainted.insert((*up).to_owned()) {
                stack.push((*up).to_owned());
            }
        }
    }
    let pushable = closure
        .keys()
        .filter(|p| !tainted.contains(*p))
        .cloned()
        .collect();
    PushSet {
        pushable,
        tainted,
        forbidden,
    }
}

/// What a PRIVATE cache may receive of `closure`: all of it. Pure.
#[must_use]
pub fn private(closure: &Closure) -> PushSet {
    PushSet {
        pushable: closure.keys().cloned().collect(),
        ..PushSet::default()
    }
}

/// The bytes of `paths`, by what `closure` says of each. Pure.
#[must_use]
pub fn bytes_of(closure: &Closure, paths: &[String]) -> u64 {
    paths
        .iter()
        .filter_map(|p| closure.get(p))
        .map(|i| i.bytes)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store path of `name`, with a hash made of its first letter.
    fn s(name: &str) -> String {
        format!(
            "/nix/store/{}-{name}",
            name.chars().next().unwrap().to_string().repeat(32)
        )
    }

    /// A closure shaped like a machine's: its system, `etc` and
    /// `system-path`, the person's host and user in them, a declared
    /// program, the person's brain-module shell, the distro's own shell and
    /// compositor, an unfree browser, a download.
    fn machine() -> (Closure, Vec<String>) {
        let graph: &[(&str, bool, &[&str])] = &[
            (
                "nixos-system-noctua-26.05",
                false,
                &[
                    "system-path",
                    "etc",
                    "ronoctua-shell",
                    "ronoctua-shell-my-tea",
                ],
            ),
            (
                "system-path",
                false,
                &[
                    "google-chrome-141.0.7390.65",
                    "ronoctua-shell",
                    "hello-2.12",
                ],
            ),
            ("etc", false, &["etc-hostname", "unit-script"]),
            ("etc-hostname", false, &[]),
            ("unit-script", false, &["glibc-2.40-66"]),
            ("hello-2.12", false, &["glibc-2.40-66"]),
            ("ronoctua-shell-my-tea", false, &["glibc-2.40-66"]),
            (
                "ronoctua-shell",
                false,
                &[
                    "ronoctua-0.1.0",
                    "scroll-unwrapped-1.12.21",
                    "segment-model.onnx",
                ],
            ),
            (
                "ronoctua-0.1.0",
                false,
                &["glibc-2.40-66", "ronoctua-0.1.0"],
            ),
            ("scroll-unwrapped-1.12.21", false, &["glibc-2.40-66"]),
            ("google-chrome-141.0.7390.65", false, &["glibc-2.40-66"]),
            ("glibc-2.40-66", false, &[]),
            ("segment-model.onnx", true, &[]),
            ("androidsdk", false, &[]),
        ];
        let closure = graph
            .iter()
            .map(|(name, ca, refs)| {
                (
                    s(name),
                    Info {
                        content_addressed: *ca,
                        references: refs.iter().map(|r| s(r)).collect(),
                        bytes: 1,
                    },
                )
            })
            .collect();
        (closure, vec!["google-chrome".to_owned()])
    }

    /// The closure of `roots` in `all`, as `nix path-info --recursive` gives it.
    fn under(all: &Closure, roots: &[&str]) -> Closure {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<String> = roots.iter().map(|r| s(r)).collect();
        while let Some(p) = stack.pop() {
            if seen.insert(p.clone()) {
                stack.extend(all[&p].references.iter().cloned());
            }
        }
        seen.into_iter()
            .map(|p| (p.clone(), all[&p].clone()))
            .collect()
    }

    /// FAILS IF: a public push of the distro's own outputs carries the
    /// machine's system, its `etc`, its host name, a declared program, the
    /// person's brain-module shell, an unfree program, a download or
    /// anything that refers to one — or leaves out what is the distro's and
    /// free.
    #[test]
    fn a_public_push_carries_the_distros_free_outputs_and_nothing_of_the_person() {
        let (all, unfree) = machine();
        // THE PUBLIC ROOTS ARE THE DISTRO'S SEED: the compositor and the
        // shell's binary — never the machine's system.
        let closure = under(
            &all,
            &[
                "scroll-unwrapped-1.12.21",
                "ronoctua-0.1.0",
                "ronoctua-shell",
            ],
        );
        let set = public(&closure, &unfree, false);
        let pushed: BTreeSet<&str> = set.pushable.iter().map(|p| name_of(p)).collect();
        assert_eq!(
            pushed,
            BTreeSet::from([
                "scroll-unwrapped-1.12.21",
                "ronoctua-0.1.0",
                "glibc-2.40-66"
            ])
        );
        for personal in [
            "nixos-system-noctua-26.05",
            "etc",
            "etc-hostname",
            "system-path",
            "hello-2.12",
            "ronoctua-shell-my-tea",
            "google-chrome-141.0.7390.65",
        ] {
            assert!(
                !pushed.contains(personal),
                "{personal} went to a public cache"
            );
        }
        // THE WRAPPER NAMES A DOWNLOAD: it stays out, and says why.
        assert!(set.tainted.contains(&s("ronoctua-shell")));
        assert_eq!(
            set.forbidden.get(&s("segment-model.onnx")),
            Some(&Forbidden::Download)
        );
        // AN UNFREE PROGRAM, and what refers to it, out of the whole system.
        let whole = public(&all, &unfree, false);
        assert_eq!(
            whole.forbidden.get(&s("google-chrome-141.0.7390.65")),
            Some(&Forbidden::Unfree)
        );
        assert!(whole.tainted.contains(&s("system-path")));
        assert!(whole.tainted.contains(&s("nixos-system-noctua-26.05")));
        // ANDROID ON: every `android…` name is unfree too.
        assert!(!whole.tainted.contains(&s("androidsdk")));
        assert!(
            public(&all, &unfree, true)
                .tainted
                .contains(&s("androidsdk"))
        );
        // CLOSED UNDER REFERENCES: no pushable path refers to a tainted one.
        for p in &set.pushable {
            for r in &closure[p].references {
                assert!(!set.tainted.contains(r), "{p} refers to {r}");
            }
        }
    }

    /// FAILS IF: a private push leaves out anything of the system's closure.
    #[test]
    fn a_private_push_takes_the_whole_closure() {
        let (all, _) = machine();
        let set = private(&all);
        assert_eq!(set.pushable.len(), all.len());
        assert_eq!(bytes_of(&all, &set.pushable), all.len() as u64);
    }

    /// FAILS IF: one of the three shapes `nix path-info --json` has printed
    /// reads otherwise than the others, or an invalid path (`null`) is taken.
    #[test]
    fn the_three_shapes_of_path_info_read_alike() {
        let object = format!(
            r#"{{"{a}": {{"ca": null, "references": ["{b}"], "narSize": 7}},
                "{b}": {{"ca": "fixed:r:sha256:0000", "references": [], "narSize": 3}}}}"#,
            a = s("ronoctua-shell"),
            b = s("bash-5.3")
        );
        let list = format!(
            r#"[{{"path": "{a}", "ca": null, "references": ["{b}"], "narSize": 7}},
                {{"path": "{b}", "ca": "fixed:r:sha256:0000", "references": [], "narSize": 3}}]"#,
            a = s("ronoctua-shell"),
            b = s("bash-5.3")
        );
        let base = |n: &str| s(n).rsplit('/').next().unwrap().to_owned();
        let format_2 = format!(
            r#"{{"version": 2, "storeDir": "/nix/store", "info": {{
                "{a}": {{"ca": null, "references": ["{b}"], "narSize": 7, "storeDir": "/nix/store"}},
                "{b}": {{"ca": {{"method": "nar", "hash": "sha256-AAAA"}}, "references": [], "narSize": 3, "storeDir": "/nix/store"}}}}}}"#,
            a = base("ronoctua-shell"),
            b = base("bash-5.3")
        );
        let read: Vec<Closure> = [object, list, format_2]
            .iter()
            .map(|t| closure_of(t).unwrap())
            .collect();
        assert_eq!(read[0], read[1]);
        assert_eq!(read[0], read[2]);
        assert!(read[0][&s("bash-5.3")].content_addressed);
        assert_eq!(read[0][&s("ronoctua-shell")].bytes, 7);
        assert_eq!(
            closure_of(&format!(r#"{{"{}": null}}"#, s("gone"))),
            Err(Why::StoreFailed)
        );
        assert_eq!(closure_of("{}"), Err(Why::StoreFailed));
        assert_eq!(closure_of("not json"), Err(Why::StoreFailed));
    }

    #[test]
    fn a_name_without_its_version_is_what_nix_calls_it() {
        for (name, pname) in [
            ("google-chrome-141.0.7390.65", "google-chrome"),
            ("unrar-7.2.6", "unrar"),
            ("android-studio-stable-2025.1.4.8", "android-studio-stable"),
            ("platform-tools-37.0.0", "platform-tools"),
            ("glibc-2.40-66", "glibc"),
            ("etc", "etc"),
            ("ronoctua-0.1.0", "ronoctua"),
            ("python3.12-requests-2.32", "python3.12-requests"),
        ] {
            assert_eq!(pname_of(name), pname, "{name}");
        }
        assert_eq!(name_of(&s("glibc-2.40-66")), "glibc-2.40-66");
    }
}
