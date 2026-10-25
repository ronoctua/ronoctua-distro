//! The command lines the realizer runs, built here so a test reads them.
//!
//! Every program by its store path (`RONOCTUA_NIX`, `RONOCTUA_SYSTEMCTL`):
//! a unit's `PATH` is minimal, and a bare name fails far from the cause.

/// `nix build` of one part: its output, not linked anywhere (the profile is
/// the root), printed so the realizer compares it with the plan's.
#[must_use]
pub fn build(nix: &str, drv: &str) -> Vec<String> {
    vec![
        nix.to_owned(),
        "build".to_owned(),
        "--no-link".to_owned(),
        "--print-out-paths".to_owned(),
        // The output of THIS derivation, never the whole of its outputs: a
        // part is one path.
        format!("{drv}^out"),
    ]
}

/// `nix-env --set` of a part's profile to `out`: a new generation, atomic.
#[must_use]
pub fn set_profile(nix_env: &str, profile: &str, out: &str) -> Vec<String> {
    vec![
        nix_env.to_owned(),
        "--profile".to_owned(),
        profile.to_owned(),
        "--set".to_owned(),
        out.to_owned(),
    ]
}

/// The part's generations trimmed to the current one and the one before it —
/// by COUNT (`+2`), never by age: `--delete-older-than` on a part that has
/// not changed in a month would take its only way back.
#[must_use]
pub fn trim_profile(nix_env: &str, profile: &str) -> Vec<String> {
    vec![
        nix_env.to_owned(),
        "--profile".to_owned(),
        profile.to_owned(),
        "--delete-generations".to_owned(),
        "+2".to_owned(),
    ]
}

/// `systemctl start` of one realizer, waiting for it to end: the planner's
/// workers take one part each, so at most as many build at once as there
/// are workers.
#[must_use]
pub fn start_realizer(systemctl: &str, id: &str) -> Vec<String> {
    vec![
        systemctl.to_owned(),
        "start".to_owned(),
        crate::names::realizer_unit(id),
    ]
}

/// `nix store prefetch-file` of one download from `url` — a file the person
/// handed in (`file://`) or a mirror — under its own name and the hash the
/// flake pins: Nix refuses other bytes, so the path that comes out can only
/// be the download's. `--unpack` for a tree's hash.
#[must_use]
pub fn prefetch(nix: &str, name: &str, hash: &str, unpack: bool, url: &str) -> Vec<String> {
    let mut argv = vec![
        nix.to_owned(),
        "store".to_owned(),
        "prefetch-file".to_owned(),
        "--json".to_owned(),
        "--name".to_owned(),
        name.to_owned(),
        "--expected-hash".to_owned(),
        hash.to_owned(),
    ];
    if unpack {
        argv.push("--unpack".to_owned());
    }
    argv.push(url.to_owned());
    argv
}

/// What the person's page runs to start one of the parts' units — never
/// asking (there is no polkit agent in the session: the rule in
/// `nix/modules/parts.nix` lets this user `start` exactly these units), and
/// waiting for a oneshot to end only when `wait`: a realizer is started and
/// left to arrive, while a hand-in, an import or an export is waited for, so
/// the page can say what came of it.
#[must_use]
pub fn start_as_the_person(systemctl: &str, unit: &str, wait: bool) -> Vec<String> {
    let mut argv = vec![
        systemctl.to_owned(),
        "--no-ask-password".to_owned(),
        "start".to_owned(),
    ];
    if !wait {
        argv.push("--no-block".to_owned());
    }
    argv.push(unit.to_owned());
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a download is added without the pinned hash — Nix would
    /// take any bytes under the download's name — or a flat file is unpacked.
    #[test]
    fn a_download_is_added_only_by_its_pinned_hash() {
        let flat = prefetch(
            "/n/nix",
            "x.tar.gz",
            "sha256-AAAA",
            false,
            "https://m.org/x.tar.gz",
        );
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
            prefetch("/n/nix", "t", "sha256-B", true, "file:///x")
                .iter()
                .any(|a| a == "--unpack")
        );
    }

    /// FAILS IF: the page's start could stop to ask a password nobody sees,
    /// or waits for a realizer — which may take an hour — on a door's thread.
    #[test]
    fn the_person_starts_a_unit_without_asking() {
        let retry = start_as_the_person("/s/systemctl", "ronoctua-part@x.service", false);
        assert_eq!(
            retry,
            [
                "/s/systemctl",
                "--no-ask-password",
                "start",
                "--no-block",
                "ronoctua-part@x.service"
            ]
        );
        let accept = start_as_the_person("/s/systemctl", crate::names::ACCEPT_UNIT, true);
        assert!(!accept.iter().any(|a| a == "--no-block"));
        assert_eq!(accept[1], "--no-ask-password");
    }

    /// FAILS IF: the trim keeps by age instead of by count, or a build asks
    /// for every output of a part's derivation — both silently change what a
    /// part keeps and fetches.
    #[test]
    fn the_commands_keep_by_count_and_build_one_output() {
        let trim = trim_profile("/x/nix-env", "/p/yt-dlp/v1");
        assert_eq!(
            trim[3..],
            ["--delete-generations".to_owned(), "+2".to_owned()]
        );
        let build = build("/x/nix", "/nix/store/a-b.drv");
        assert_eq!(
            build.last().map(String::as_str),
            Some("/nix/store/a-b.drv^out")
        );
        assert_eq!(
            start_realizer("/x/systemctl", "yt-dlp"),
            ["/x/systemctl", "start", "ronoctua-part@yt-dlp.service"]
        );
    }
}
