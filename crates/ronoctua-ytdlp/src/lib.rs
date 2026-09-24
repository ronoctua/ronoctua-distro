//! yt-dlp's flags that are about the DEVICE, not the person — one place for
//! the transfer queue and the player.
//!
//! The queue downloads and the player plays, and both ask YouTube through
//! yt-dlp. What makes an anonymous request work today — a PO token minted on
//! demand — is the same for both, so it lives here; who the person is (the
//! cookies, a pasted token) stays with the queue, which is the only one that
//! ever asks as someone.

/// The PO token minter, when this installation has one.
///
/// # Why this is NOT behind `Identity::Mine`
///
/// Because a PO token does not say who you are — it says that whoever is
/// asking is a real device. When there is no account, the minter binds it to a
/// visitor id it generates on the spot, so an anonymous download stays
/// anonymous and gains the attestation that YouTube increasingly demands of
/// the default clients. Gating it on identity would leave the anonymous case —
/// which is the default, and the common one — without the one thing that makes
/// it work.
///
/// The token FILE below is the opposite and stays gated: somebody pasted it,
/// so it is bound to whatever session it was minted in, and sending it with an
/// anonymous request would attach that session to it.
#[must_use]
pub fn pot_flags() -> Vec<String> {
    pot_flags_of(
        std::env::var("RONOCTUA_POT_HOME").ok().as_deref(),
        std::env::var("RONOCTUA_DENO").ok().as_deref(),
    )
}

/// The same decision, over values a test can hand it.
///
/// BOTH OR NEITHER: the minter is TypeScript and needs the runtime that reads
/// it, and half of the pair produces a plugin that finds no runtime and says so
/// once per download.
#[must_use]
pub fn pot_flags_of(home: Option<&str>, deno: Option<&str>) -> Vec<String> {
    let home = home.map(str::trim).filter(|v| !v.is_empty());
    let deno = deno.map(str::trim).filter(|v| !v.is_empty());
    let (Some(home), Some(deno)) = (home, deno) else {
        return Vec::new();
    };
    vec![
        // THE PATH AND NOT THE NAME. A systemd unit's `PATH` is minimal, and a
        // runtime looked up by name fails far from the cause — the trap
        // `AGENTS.md` records for every program this project spawns.
        "--js-runtimes".to_owned(),
        format!("deno:{deno}"),
        // A DIFFERENT extractor key from the manual token's `youtube:`, so the
        // two coexist: yt-dlp keys `--extractor-args` by extractor, and two
        // flags for the SAME key would replace one another.
        "--extractor-args".to_owned(),
        format!("youtubepot-bgutilscript:server_home={home}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_minter_is_not_gated_on_identity_and_needs_both_halves() {
        // A PO token attests the DEVICE, not the person: anonymous downloads
        // need it too, and that is the common case.
        assert_eq!(
            pot_flags_of(Some("/x/server"), Some("/y/bin/deno")),
            [
                "--js-runtimes",
                "deno:/y/bin/deno",
                "--extractor-args",
                "youtubepot-bgutilscript:server_home=/x/server"
            ]
        );
        // Half a pair is a plugin that finds no runtime and says so once per
        // download; nothing is better than that.
        assert!(pot_flags_of(Some("/x/server"), None).is_empty());
        assert!(pot_flags_of(None, Some("/y/bin/deno")).is_empty());
        assert!(pot_flags_of(Some(""), Some("")).is_empty());
        assert!(pot_flags_of(None, None).is_empty());
    }
}
