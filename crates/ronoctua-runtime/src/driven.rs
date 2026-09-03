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

/// The languages this system ships, and the ones every driver measures in.
///
/// Written here once. It used to be `["pt-BR", "en-GB"]` typed out in each
/// binary, which is how a third language becomes a dozen edits nobody finds.
pub const LANGUAGES: &[&str] = &["en-GB", "pt-BR"];

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

/// A label the language made longer than the buffer's budget.
pub struct TooWide {
    /// The key, as the buffer emitted it.
    pub key: String,
    /// The language it does not fit in.
    pub language: String,
    /// What that language says.
    pub text: String,
    /// How many cells it takes.
    pub cells: u16,
    /// How many the buffer reserved.
    pub budget: u16,
}

/// The text keys a frame emitted, with the budget the buffer reserved for each.
///
/// BOTH HALVES COME OUT OF THE SAME FRAME, and that is what makes the whole
/// measurement possible without a single new declaration: `Command::Text`
/// already carries the key and the `max_width` side by side. A check that read
/// the buffer's source instead would have to find a constant, and three of the
/// buffers assemble their keys at run time.
#[must_use]
pub fn keys_of(list: &ronoctua_protocol::DisplayList) -> Vec<(String, u16)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            ronoctua_protocol::Command::Text {
                text: ronoctua_protocol::Text::Key(k),
                max_width,
                ..
            } => Some((k.clone(), *max_width)),
            _ => None,
        })
        .collect()
}

/// How many CELLS a string takes, measured the way the host CUTS.
///
/// By grapheme cluster, because that is what `ronoctua_render::layout::width`
/// counts and what `elide_end` walks. The buffers count `chars()` in
/// `ronoctua_design::budget::wide`, and the two disagree on a combining mark —
/// so a check that used the buffer's ruler would answer "it fits" about a label
/// the host is about to cut. What matters here is what the SCREEN does.
#[must_use]
pub fn cells(text: &str) -> u16 {
    use unicode_segmentation::UnicodeSegmentation as _;
    u16::try_from(text.graphemes(true).count()).unwrap_or(u16::MAX)
}

/// Every label of this frame that its own budget cuts, in either language.
///
/// # Why this exists at all
///
/// A buffer cannot measure a sentence it does not have: it emits a KEY and the
/// host translates (invariant 11), so what it declares is what it RESERVED.
/// `buffers/format/src/lib.rs` says as much over `NAME_HINT`: *"a third
/// language longer than this elides, and there is no check comparing the two"*.
/// This is that check.
///
/// A budget of `0` means "to the edge of the screen" and is not a claim about
/// length, so it is not measured.
///
/// It REPORTS rather than failing, because a cut can be deliberate — the
/// transfer queue's errand column reserves eleven cells for a word that runs to
/// twenty-three, and says so. The caller holds the list of those.
pub fn too_wide(
    list: &ronoctua_protocol::DisplayList,
    i18n: &str,
    languages: &[&str],
) -> Result<Vec<TooWide>> {
    let mut out = Vec::new();
    for language in languages {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, budget) in keys_of(list) {
            if budget == 0 {
                continue;
            }
            let Some(text) = table.get(&key) else {
                anyhow::bail!("the frame emits `{key}`, which {language}.ftl does not have");
            };
            let wide = cells(text);
            if wide > budget {
                out.push(TooWide {
                    key,
                    language: (*language).to_owned(),
                    text: text.clone(),
                    cells: wide,
                    budget,
                });
            }
        }
    }
    Ok(out)
}

/// Every label fits, except the ones the caller says are cut on purpose.
///
/// The exception list is `(key prefix, why)`, and the reason is not decoration:
/// it is what tells the next reader whether the entry is a decision or a
/// leftover. It is the same bargain `generator-flags-agree` makes with
/// `--reflink`.
pub fn labels_fit(
    list: &ronoctua_protocol::DisplayList,
    i18n: &str,
    languages: &[&str],
    on_purpose: &[(&str, &str)],
) -> Result<usize> {
    let all = too_wide(list, i18n, languages)?;
    let mut bad = Vec::new();
    for one in all {
        if on_purpose.iter().any(|(p, _)| one.key.starts_with(p)) {
            continue;
        }
        bad.push(format!(
            "in {}, `{}` = {:?} is {} cells and the buffer reserved {}",
            one.language, one.key, one.text, one.cells, one.budget
        ));
    }
    if !bad.is_empty() {
        anyhow::bail!(
            "{} label(s) will come out CUT — the budget is the buffer's own, and the\n\
             host cuts to it whatever the language says:\n  {}",
            bad.len(),
            bad.join("\n  ")
        );
    }
    Ok(keys_of(list).len())
}

thread_local! {
    /// Every keyed run this process has looked at, and its budget.
    static SEEN: std::cell::RefCell<Vec<(String, u16)>> = const {
        std::cell::RefCell::new(Vec::new())
    };
}

/// Remembers this frame's labels, to be measured when the run is over.
///
/// # Only a ROOMY frame
///
/// Note the frame the buffer was given the width it asked for, never a squeezed
/// one. A `fill` column gives way on purpose, so at forty-six columns every
/// budget is "what was left" and the host elides by design — measuring those
/// would report every buffer in the system as broken. The question here is
/// whether the buffer RESERVED enough, and that is only answerable where it got
/// what it asked for.
///
/// # Why a collector, and not a measurement per frame
///
/// Because coverage here is "which SCREENS did the driver visit", and a driver
/// visits eight or thirteen of them through one helper it already has. Calling
/// this from that helper measures all of them for one line; measuring frame by
/// frame would need the `.ftl` in the signature of everything that renders, and
/// would say the same thing eight times.
pub fn note(list: &ronoctua_protocol::DisplayList) {
    let keys = keys_of(list);
    SEEN.with_borrow_mut(|s| s.extend(keys));
}

/// Everything this run looked at fits, in every language.
///
/// It is the other half of `note`: call it once, at the end. It says how many
/// DISTINCT labels it measured, because a number that quietly went to zero —
/// the helper stopped being called, a screen stopped being visited — is the way
/// this check would fail open.
///
/// `to_the_edge` is the width the frames were rendered at. A run that reserved
/// the WHOLE line is not making a claim about length — it is saying "as far as
/// there is", which is what a budget of `0` means in the WIT and what a help
/// line at the foot of a sheet means in practice. Measuring those would report
/// every hint in the system as one cell too long.
pub fn every_label_fits(
    i18n: &str,
    languages: &[&str],
    on_purpose: &[(&str, &str)],
    to_the_edge: u16,
) -> Result<usize> {
    let seen = SEEN.with_borrow(|s| {
        let mut v = s.clone();
        v.sort_unstable();
        v.dedup();
        v
    });
    anyhow::ensure!(
        !seen.is_empty(),
        "no label was looked at: `driven::note` is not being called, and this\n\
         check would pass on a buffer with no text at all"
    );
    let mut bad = Vec::new();
    for language in languages {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, budget) in &seen {
            if *budget == 0 || *budget >= to_the_edge {
                continue;
            }
            let Some(text) = table.get(key) else {
                anyhow::bail!("a frame emits `{key}`, which {language}.ftl does not have");
            };
            let wide = cells(text);
            if wide > *budget && !on_purpose.iter().any(|(p, _)| key.starts_with(p)) {
                bad.push(format!(
                    "in {language}, `{key}` = {text:?} is {wide} cells and the buffer reserved {budget}"
                ));
            }
        }
    }
    if !bad.is_empty() {
        anyhow::bail!(
            "{} label(s) will come out CUT — the budget is the buffer's own, and the\n\
             host cuts to it whatever the language says:\n  {}",
            bad.len(),
            bad.join("\n  ")
        );
    }
    Ok(seen.len())
}
