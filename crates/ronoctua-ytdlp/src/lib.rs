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
//!
//! And what a video is CALLED, asked without playing it ([`title`]): the
//! name a leaf of the player's queue takes when it arrived as an address.
//!
//! And a search of YouTube for fast-access's `/y` ([`search`]), with the
//! person's pass and a stop the next key pulls.
//!
//! And what video-metrics asks of YouTube's own pages ([`youtube`]): who a
//! channel is ([`channel_of`]), a tab listed flat with approximate dates
//! ([`tab`]), and every field of one video ([`details`]) — each saying, when
//! it fails, whether yt-dlp met a bot check, a 429 or a video that is gone.

mod address;
mod listing;
mod program;
mod run;
mod search;
mod title;
pub mod youtube;

pub use address::{escape, is_address, listing_address, video_id};
pub use listing::THUMBNAIL_MOST_WIDTH;
pub use listing::{
    DEADLINE, FLAT_MOST, Flat, FlatEntry, LIVE_FILTER, LIVES_HOST, LIVES_MOST, Live, MOST_BYTES,
    WORDS_MOST, flat, flat_arguments, flat_up_to, lives_address, lives_arguments, parse_flat,
    parse_lives, search_lives,
};
pub use program::{VARIABLE, installed, installed_of, named, named_of, program, program_of};
pub use search::{
    Found, SEARCH_DEADLINE, SEARCH_HOST, SEARCH_MOST, SHORT_MOST_SECONDS, Shorts, Stop,
    parse_search, search, search_arguments,
};
pub use title::{TITLE_MOST_CHARS, parse_title, title, title_arguments, title_by};
pub use youtube::{channel_of, details, tab};

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

/// The variable naming the minter's server half (`<pot-provider>/server`).
pub const POT_HOME: &str = "RONOCTUA_POT_HOME";

/// The variable naming the JavaScript runtime the minter runs on, by path.
pub const POT_DENO: &str = "RONOCTUA_DENO";

/// The variable naming the folder yt-dlp loads the minter's plugin from.
///
/// A folder of plugin PACKAGES — the provider's root, `<pot-provider>` —
/// and not the package itself: yt-dlp's `--plugin-dirs` lists the folder's
/// children and looks for `yt_dlp_plugins` inside each. Measured with the
/// pinned yt-dlp (`-v`): `--plugin-dirs <pot-provider>/plugin` says "Plugin
/// directories: none"; `--plugin-dirs <pot-provider>` finds
/// `<pot-provider>/plugin/yt_dlp_plugins`.
pub const POT_PLUGINS: &str = "RONOCTUA_POT_PLUGINS";

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
    let var = |name: &str| std::env::var(name).ok();
    pot_flags_of(
        var(POT_HOME).as_deref(),
        var(POT_DENO).as_deref(),
        var(POT_PLUGINS).as_deref(),
    )
}

/// The same decision, over values a test can hand it.
///
/// ALL THREE OR NONE. The minter is TypeScript and needs the runtime that
/// reads it; and yt-dlp only ASKS it for a token through its plugin, which it
/// loads from a folder it is told about and from nowhere else. Measured on the
/// pinned yt-dlp: the extractor arguments and the runtime were passed, the
/// plugin folder never was, and not one token was minted — for the player or
/// the queue. Two of the three are a plugin that is never loaded, or one that
/// finds no runtime and says so once per request; nothing is better than that.
#[must_use]
pub fn pot_flags_of(home: Option<&str>, deno: Option<&str>, plugins: Option<&str>) -> Vec<String> {
    fn set(v: Option<&str>) -> Option<&str> {
        v.map(str::trim).filter(|v| !v.is_empty())
    }
    let (Some(home), Some(deno), Some(plugins)) = (set(home), set(deno), set(plugins)) else {
        return Vec::new();
    };
    vec![
        // THE PLUGIN FIRST: without it the two below are arguments to a
        // provider nobody loaded.
        "--plugin-dirs".to_owned(),
        plugins.to_owned(),
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

    /// FAILS IF: the minter's plugin folder is not handed to yt-dlp (a
    /// token is never asked for), any flag goes out while one of the three
    /// is missing, or the flags stop being pairs (mpv's `ytdl-raw-options`
    /// reads them two by two).
    #[test]
    fn the_minter_is_not_gated_on_identity_and_needs_all_three() {
        // A PO token attests the DEVICE, not the person: anonymous downloads
        // need it too, and that is the common case.
        let flags = pot_flags_of(Some("/x/server"), Some("/y/bin/deno"), Some("/x"));
        assert_eq!(
            flags,
            [
                "--plugin-dirs",
                "/x",
                "--js-runtimes",
                "deno:/y/bin/deno",
                "--extractor-args",
                "youtubepot-bgutilscript:server_home=/x/server"
            ]
        );
        assert_eq!(flags.len() % 2, 0, "every flag has its value");
        // Two of three is a plugin nobody loads, or one that finds no runtime.
        let (h, d, p) = (Some("/x/server"), Some("/y/bin/deno"), Some("/x"));
        for (home, deno, plugins) in [
            (h, d, None),
            (h, None, p),
            (None, d, p),
            (h, None, None),
            (None, d, None),
            (None, None, p),
            (Some(" "), d, p),
            (h, d, Some("")),
            (None, None, None),
        ] {
            assert!(
                pot_flags_of(home, deno, plugins).is_empty(),
                "{home:?} {deno:?} {plugins:?}"
            );
        }
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
