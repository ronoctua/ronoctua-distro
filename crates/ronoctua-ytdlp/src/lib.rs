//! Every yt-dlp command line this system runs — one place for the transfer
//! queue and the player.
//!
//! The queue downloads and the player plays, and both ask YouTube through
//! yt-dlp. What makes an anonymous request work today — a PO token minted on
//! demand — is the same for both, so it lives here; who the person is (the
//! cookies, a pasted token) stays with the queue, which is the only one that
//! ever asks as someone.
//!
//! And WHERE the program is ([`program`], [`installed`]) — every caller asks
//! here, so the unit's variable is read in one place — and the two things the
//! player lists without downloading: a playlist, flat ([`flat`]), and a search
//! for what is live now ([`search_lives`]).

mod address;
mod listing;
mod program;
mod run;

pub use address::{escape, is_address, listing_address};
pub use listing::{
    DEADLINE, FLAT_MOST, Flat, FlatEntry, LIVE_FILTER, LIVES_HOST, LIVES_MOST, Live, MOST_BYTES,
    WORDS_MOST, flat, flat_arguments, flat_up_to, lives_address, lives_arguments, parse_flat,
    parse_lives, search_lives,
};
pub use program::{VARIABLE, installed, installed_of, program, program_of};

/// Why a listing did not answer, as a closed set.
///
/// No sentence here: the words are the door's, one per variant, through
/// [`Trouble::id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trouble {
    /// This installation has no yt-dlp (the reduced profile).
    Absent,
    /// yt-dlp could not be started.
    Unstartable,
    /// It did not answer inside [`DEADLINE`], and was stopped.
    Slow,
    /// It ended with a failure: no network, a removed list, an address it
    /// does not know.
    Failed,
    /// It answered something that is not the JSON asked for.
    Garbled,
    /// It answered more than [`MOST_BYTES`].
    TooLarge,
    /// The address is not a plain web address.
    NotAnAddress,
    /// The address is one video, not a list.
    NotAList,
    /// There are no words to search for.
    NothingToAsk,
}

impl Trouble {
    /// Every trouble, for a test that walks the words.
    pub const ALL: [Self; 9] = [
        Self::Absent,
        Self::Unstartable,
        Self::Slow,
        Self::Failed,
        Self::Garbled,
        Self::TooLarge,
        Self::NotAnAddress,
        Self::NotAList,
        Self::NothingToAsk,
    ];

    /// Its kebab-case name, the suffix of the key a door says it with.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Unstartable => "unstartable",
            Self::Slow => "slow",
            Self::Failed => "failed",
            Self::Garbled => "garbled",
            Self::TooLarge => "too-large",
            Self::NotAnAddress => "not-an-address",
            Self::NotAList => "not-a-list",
            Self::NothingToAsk => "nothing-to-ask",
        }
    }
}

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

    #[test]
    fn a_troubles_id_is_its_serde_name() {
        for t in Trouble::ALL {
            assert_eq!(
                serde_json::to_string(&t).unwrap(),
                format!("\"{}\"", t.id())
            );
        }
    }
}
