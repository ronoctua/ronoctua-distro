//! WHY NETWORKMANAGER REFUSED, IN A SENTENCE — one key per refusal.
//!
//! # Why a module of its own
//!
//! The network's view says it, and so does the Bluetooth view, whose
//! tethering goes through NetworkManager: two views, one sentence. It was
//! the network's words, so the Bluetooth view named the network's.

/// The REASON alone, by name — what an errand's own sentence ("could not
/// connect to X: …", "could not share: …") puts after its colon.
///
/// The errand's sentence says WHAT was not done; this says WHY, in the
/// person's language. The error's words go to the log, never to the line.
///
/// A refusal the network crate names itself ([`ronoctua_network::Refusal`]:
/// no phone, no VPN, not a VPN) is said by its own sentence
/// (`network_refusal::network_why_key`); every other is classified by
/// `ronoctua_network::refusal`.
#[must_use]
pub fn reason(e: &anyhow::Error, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    use ronoctua_network::refusal::Refusal;
    if let Some(why) = e
        .chain()
        .find_map(|c| c.downcast_ref::<ronoctua_network::Refusal>())
    {
        tracing::info!(?why, words = %format!("{e:#}"), "the network refused");
        return texts.t(crate::network_refusal::network_why_key(*why));
    }
    let why = ronoctua_network::refusal::refusal(e);
    tracing::info!(?why, words = %format!("{e:#}"), "the network refused");
    texts.t(match why {
        Refusal::NotAuthorized => key::NETWORK_REFUSED_AUTHORITY,
        Refusal::NotAvailable => key::NETWORK_REFUSED_ABSENT,
        Refusal::Invalid => key::NETWORK_REFUSED_INVALID,
        Refusal::NoSecrets => key::NETWORK_REFUSED_SECRETS,
        Refusal::Failed => key::NETWORK_REFUSED_FAILED,
        Refusal::TimedOut => key::NETWORK_REFUSED_TIMED_OUT,
        Refusal::NoBus => key::NETWORK_REFUSED_NO_BUS,
        Refusal::NoRadio => key::NETWORK_REFUSED_NO_RADIO,
        Refusal::OutOfRange => key::NETWORK_REFUSED_OUT_OF_RANGE,
        Refusal::NoDevice => key::NETWORK_REFUSED_NO_DEVICE,
        Refusal::NoProfile => key::NETWORK_REFUSED_NO_PROFILE,
        Refusal::ImportRefused => key::NETWORK_REFUSED_IMPORT,
        Refusal::Vault => key::NETWORK_REFUSED_VAULT,
        Refusal::Other => key::NETWORK_REFUSED_OTHER,
    })
}

/// The sentence of one refusal of the network crate — one key per variant.
#[must_use]
pub const fn network_why_key(why: ronoctua_network::Refusal) -> &'static str {
    use crate::key;
    match why {
        ronoctua_network::Refusal::NoPhone => key::NETWORK_REFUSAL_NO_PHONE,
        ronoctua_network::Refusal::NoVpn => key::NETWORK_REFUSAL_NO_VPN,
        ronoctua_network::Refusal::NotVpn => key::NETWORK_REFUSAL_NOT_VPN,
    }
}
