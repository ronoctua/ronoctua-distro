//! What every driven test of a buffer needs, in one place.
//!
//! The eight binaries beside this module all do the same three things before
//! they can assert anything: read a `.ftl`, hand the buffer the phrases it
//! declared, and compare what the screen reserved against what the language
//! actually says. Written once per binary, those three drifted — the `.ftl`
//! reader existed six times over and the comparison only twice.
//!
//! They deliberately do NOT link `ronoctua-i18n`: a driver reads the `.ftl`
//! files as the source data they are, which is what lets it check both
//! languages in one run without two bundles.

use anyhow::{Context as _, Result};
use std::collections::HashMap;

/// A `.ftl` as a `key -> text` table.
///
/// The comment lines go, and so does everything that is not `key = value`; a
/// `.ftl` selector (the plural forms) is several lines and none of them splits
/// on `" = "`, so it simply does not arrive here — which is right, because a
/// selector has no single text to measure.
pub fn ftl(path: &str) -> Result<HashMap<String, String>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("could not read {path}"))?;
    Ok(raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect())
}

/// Hands the buffer the phrases it declared, in one language.
///
/// The same thing `crates/ronoctua-shell/src/buffer.rs` does at load, and for
/// the same reason: a buffer that holds interface text has to be given it
/// before the first frame, or the test photographs the fallback.
///
/// It also REFUSES a key that is not in the `.ftl`, which is the check the
/// mechanism needs and cannot get anywhere else: `Texts::t` never fails, so a
/// key with a typo in it reaches the screen as `[the-key]` and nothing upstream
/// says a word.
pub fn hand_phrases(
    inst: &mut crate::Instance,
    d: &ronoctua_protocol::Descriptor,
    i18n: &str,
    language: &str,
) -> Result<()> {
    if d.phrases.is_empty() {
        return Ok(());
    }
    let table = ftl(&format!("{i18n}/{language}.ftl"))?;
    let mut said = Vec::new();
    for k in &d.phrases {
        let Some(text) = table.get(k) else {
            anyhow::bail!(
                "`{}` declares the phrase `{k}`, which {language}.ftl does not have.\n\
                 It would reach the screen as `[{k}]`: the host resolves a key with\n\
                 `Texts::t`, and that never fails.",
                d.name
            );
        };
        said.push(ronoctua_protocol::Phrase {
            key: k.clone(),
            text: text.clone(),
        });
    }
    inst.event(&ronoctua_protocol::Input::Phrases(said))?;
    Ok(())
}
