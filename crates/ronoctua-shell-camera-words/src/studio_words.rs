//! THE STUDIO'S REFUSALS, in words — what the camera's doors say when the
//! studio refuses, and what the footer says when it refuses the hands.
//!
//! Here because the two are the camera's crate and the footer's gestures,
//! and the footer names no feature: a sentence two of them say comes down
//! below both, once, so the two say the same thing about one refusal — into
//! a leaf of its own and not the door, which every shell crate stands on and
//! which would then recompile with the studio's daemon.

use ronoctua_studio::wire::Trouble;

/// The sentence for a trouble — every one has a key in both languages.
#[must_use]
pub fn trouble_sentence(t: &Trouble, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    match t {
        Trouble::NoCamera => texts.t(key::STUDIO_TROUBLE_NO_CAMERA),
        Trouble::CameraBusy { by } => {
            texts.t_with(key::STUDIO_TROUBLE_CAMERA_BUSY, &[("program", by)])
        }
        Trouble::CameraSilent => texts.t(key::STUDIO_TROUBLE_CAMERA_SILENT),
        Trouble::NoModel => texts.t(key::STUDIO_TROUBLE_NO_MODEL),
        Trouble::NoFace => texts.t(key::STUDIO_TROUBLE_NO_FACE),
        Trouble::Rehearsal => texts.t(key::STUDIO_TROUBLE_REHEARSAL),
        Trouble::NoHandModel => texts.t(key::STUDIO_TROUBLE_NO_HAND_MODEL),
        Trouble::NoBackdrop => texts.t(key::STUDIO_TROUBLE_NO_BACKDROP),
        Trouble::NoFfmpeg => texts.t(key::STUDIO_TROUBLE_NO_FFMPEG),
        Trouble::NoScreen => texts.t(key::STUDIO_TROUBLE_NO_SCREEN),
        Trouble::NoWindow => texts.t(key::STUDIO_TROUBLE_NO_WINDOW),
        Trouble::NoVirtual => texts.t(key::STUDIO_TROUBLE_NO_VIRTUAL),
        Trouble::VirtualBusy => texts.t(key::STUDIO_TROUBLE_VIRTUAL_BUSY),
        Trouble::Encoder { why } => texts.t_with(key::STUDIO_TROUBLE_ENCODER, &[("why", why)]),
        Trouble::Stream { why } => texts.t_with(key::STUDIO_TROUBLE_STREAM, &[("why", why)]),
        Trouble::NoStreamTarget => texts.t(key::STUDIO_TROUBLE_NO_STREAM_TARGET),
        Trouble::Name => texts.t(key::STUDIO_TROUBLE_NAME),
        Trouble::Write { why } => texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", why)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a refusal of the studio is said as a missing key's marker —
    /// the camera's line and the footer's would show `[studio-trouble-…]`.
    #[test]
    fn every_trouble_has_a_sentence() {
        let tx = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        for t in [
            Trouble::NoCamera,
            Trouble::CameraBusy { by: "x".into() },
            Trouble::CameraSilent,
            Trouble::NoModel,
            Trouble::NoFace,
            Trouble::Rehearsal,
            Trouble::NoHandModel,
            Trouble::NoBackdrop,
            Trouble::NoFfmpeg,
            Trouble::NoScreen,
            Trouble::NoWindow,
            Trouble::NoVirtual,
            Trouble::VirtualBusy,
            Trouble::Encoder { why: "x".into() },
            Trouble::Stream { why: "x".into() },
            Trouble::NoStreamTarget,
            Trouble::Name,
            Trouble::Write { why: "x".into() },
        ] {
            assert!(
                !trouble_sentence(&t, &tx).starts_with('['),
                "{t:?} has no sentence"
            );
        }
    }
}
