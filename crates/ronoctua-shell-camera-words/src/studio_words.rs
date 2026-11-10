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
    trouble_named(t, texts).text
}

/// The same sentence, with the NAME it quotes told — the program holding the
/// camera (`ronoctua_i18n::Arg::Name`) — for a door that says it on its line,
/// where a name is drawn in a name's ink.
#[must_use]
pub fn trouble_named(t: &Trouble, texts: &ronoctua_i18n::Texts) -> ronoctua_i18n::Marked {
    use crate::key;
    use ronoctua_i18n::Arg;
    let plain = |k: &str| texts.t_args_marked(k, &[]);
    let why = |k: &str, why: &str| texts.t_args_marked(k, &[("why", Arg::Text(why))]);
    match t {
        Trouble::NoCamera => plain(key::STUDIO_TROUBLE_NO_CAMERA),
        Trouble::CameraBusy { by } => texts.t_args_marked(
            key::STUDIO_TROUBLE_CAMERA_BUSY,
            &[("program", Arg::Name(by))],
        ),
        Trouble::CameraSilent => plain(key::STUDIO_TROUBLE_CAMERA_SILENT),
        Trouble::NoModel => plain(key::STUDIO_TROUBLE_NO_MODEL),
        Trouble::NoFace => plain(key::STUDIO_TROUBLE_NO_FACE),
        Trouble::Rehearsal => plain(key::STUDIO_TROUBLE_REHEARSAL),
        Trouble::NoHandModel => plain(key::STUDIO_TROUBLE_NO_HAND_MODEL),
        Trouble::NoBackdrop => plain(key::STUDIO_TROUBLE_NO_BACKDROP),
        Trouble::NoFfmpeg => plain(key::STUDIO_TROUBLE_NO_FFMPEG),
        Trouble::NoScreen => plain(key::STUDIO_TROUBLE_NO_SCREEN),
        Trouble::NoWindow => plain(key::STUDIO_TROUBLE_NO_WINDOW),
        Trouble::NoVirtual => plain(key::STUDIO_TROUBLE_NO_VIRTUAL),
        Trouble::VirtualBusy => plain(key::STUDIO_TROUBLE_VIRTUAL_BUSY),
        Trouble::Encoder { why: w } => why(key::STUDIO_TROUBLE_ENCODER, w),
        Trouble::Stream { why: w } => why(key::STUDIO_TROUBLE_STREAM, w),
        Trouble::NoStreamTarget => plain(key::STUDIO_TROUBLE_NO_STREAM_TARGET),
        Trouble::Name => plain(key::STUDIO_TROUBLE_NAME),
        Trouble::Write { why: w } => why(key::STUDIO_TROUBLE_WRITE, w),
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

    /// FAILS IF: the program holding the camera is said as plain words —
    /// the door's line would draw it in the sentence's ink — or the told
    /// sentence reads other words than the plain one.
    #[test]
    fn a_busy_camera_tells_who_holds_it() {
        let tx = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        let busy = Trouble::CameraBusy {
            by: "obs-studio".into(),
        };
        let told = trouble_named(&busy, &tx);
        assert_eq!(told.text, trouble_sentence(&busy, &tx));
        let names: Vec<&str> = told.names.iter().map(|r| &told.text[r.clone()]).collect();
        assert_eq!(names, ["obs-studio"], "{told:?}");
    }
}
