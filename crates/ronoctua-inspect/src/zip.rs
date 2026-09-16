//! The zip reader, and the refusal it becomes here.
//!
//! # Why this file is four lines of adapter and not two hundred of reader
//!
//! The reader moved to `ronoctua-zip`, because a REGION PACK for the atlas is
//! a zip too and the crate that bakes one cannot depend on the file manager's
//! inspector. What stayed is the translation, which is the half that is this
//! crate's business: a zip's [`ronoctua_zip::Trouble`] has two shapes and no
//! sentence, and a panel draws a [`Refusal`].
//!
//! Every call site here keeps working unchanged — `zip::members`,
//! `zip::member_bytes`, `zip::part` — which is what makes this a MOVE rather
//! than a rewrite of the six formats that read one.

pub use ronoctua_zip::{INFLATED_CEILING, Member, part};

use crate::Refusal;

/// What a zip's trouble is, said as this panel says things.
const fn refusal(t: ronoctua_zip::Trouble) -> Refusal {
    match t {
        ronoctua_zip::Trouble::Malformed => Refusal::Failed,
        ronoctua_zip::Trouble::TooBig(n) => Refusal::TooBig(n),
    }
}

/// The members of a zip, found through its central directory.
pub fn members(bytes: &[u8]) -> Result<Vec<Member>, Refusal> {
    ronoctua_zip::members(bytes).map_err(refusal)
}

/// One member's bytes, inflated with a ceiling ON THE WAY OUT.
pub fn member_bytes(bytes: &[u8], m: &Member) -> Result<Vec<u8>, Refusal> {
    ronoctua_zip::member_bytes(bytes, m).map_err(refusal)
}
