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

#[cfg(test)]
mod tests {
    use super::*;

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
