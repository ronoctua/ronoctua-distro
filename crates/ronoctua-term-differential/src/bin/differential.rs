//! Differential test of VT engines — section 7.10, milestone 7's criterion.
//!
//! # Three verdicts per case
//!
//! 1. **Across engines**, on what both can be compared on: the characters and
//!    the cursor, the style bits both keep, and the answers to CPR, DSR,
//!    DECRQM and colour queries
//!    ([`ronoctua_term_differential::comparable_replies`]). A zero diff is the
//!    criterion; any difference is a divergence of interpretation, which is
//!    the most expensive bug to find later — it shows up as "the terminal goes
//!    odd sometimes".
//! 2. **Each engine against its own goldens**, for what differs by design:
//!    the style runs and EVERY reply, DA1, DA2 and XTVERSION included
//!    (`<styled>/<engine>/<case>.txt`), plus the characters and cursor both
//!    share (`<golden>/<case>.txt`). This catches a regression, which the
//!    cross-engine comparison cannot when both engines move together.
//! 3. **The window's copy against the holder's**, at every byte of the case:
//!    the Ghostty engine is snapshotted with the case cut in two, the snapshot
//!    decoded into a mirror, the rest fed to both — and the two must write out
//!    the same terminal. A cut inside a CSI, an OSC or a UTF-8 sequence is
//!    exactly where a mirror would lose it.
//!
//! There is no single-engine mode any more: Ghostty is the product engine,
//! and a run without it would be a run without the thing it tests.
//!
//! Usage:
//!
//! ```text
//! ronoctua-term-diff <corpus-dir> <golden-dir> <styled-dir>            compares
//! ronoctua-term-diff <corpus-dir> <golden-dir> <styled-dir> --record   re-records
//! ```

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use ronoctua_term::ghostty::{Decoder, disagreement};
use ronoctua_term_differential::alacritty::Alacritty;
use ronoctua_term_differential::ghostty::{self, Ghostty};
use ronoctua_term_differential::{Engine, Grid, Reply, comparable_replies};

/// The corpus grid's size.
///
/// Narrow on purpose: 20 columns make line wrapping happen in
/// case 07 with few bytes, and wrapping is where engines diverge.
const COLUMNS: u16 = 20;
const ROWS: u16 = 6;

/// The engines, each fresh for a case.
fn engines() -> Result<Vec<Box<dyn Engine>>> {
    let ghostty = Ghostty::fresh(COLUMNS, ROWS).context("the ghostty engine")?;
    Ok(vec![
        Box::new(Alacritty::fresh(COLUMNS, ROWS)),
        Box::new(ghostty),
    ])
}

/// Bytes a golden can show: printable text as it is, every control by name.
fn escaped(bytes: &[u8]) -> String {
    let mut out = String::new();
    for c in String::from_utf8_lossy(bytes).chars() {
        match c {
            '\x1b' => out.push_str("\\e"),
            '\x07' => out.push_str("\\a"),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out
}

/// One engine's own golden: its style runs and every reply it gave.
fn own_golden(grid: &Grid, replies: &[u8]) -> String {
    let replies = if replies.is_empty() {
        "none".to_owned()
    } else {
        escaped(replies)
    };
    format!("{}replies: {replies}\n", grid.canonical_styled())
}

/// Verdict 3: the mirror against the holder at every cut of `bytes`.
fn every_cut(bytes: &[u8]) -> Result<Option<String>> {
    for cut in 0..=bytes.len() {
        let mut holder = ghostty::terminal(COLUMNS, ROWS)?;
        holder.feed(&bytes[..cut]);
        let mut mirror = Decoder::decode(holder.snapshot()?)?;
        holder.feed(&bytes[cut..]);
        mirror.feed(&bytes[cut..]);
        if let Some(d) = disagreement(&holder, &mirror) {
            return Ok(Some(format!("cut at byte {cut}:\n{d}")));
        }
    }
    Ok(None)
}

/// Compares `current` with the golden at `file`, or keeps it to be written.
///
/// Kept, not written: a recording is written only once every case has passed
/// the other two verdicts, so a divergence found in a later case cannot leave
/// the goldens of the earlier ones rewritten over it.
fn golden(
    file: &Path,
    current: &str,
    record: Option<&mut Vec<(PathBuf, String)>>,
    failures: &mut Vec<String>,
) -> Result<()> {
    if let Some(recorded) = record {
        recorded.push((file.to_path_buf(), current.to_owned()));
        return Ok(());
    }
    let expected = std::fs::read_to_string(file).with_context(|| {
        format!(
            "the golden {} is missing.\n\
             \n\
             Make it with: ronoctua-term-diff <corpus> <golden> <styled> --record\n\
             and CHECK the result before versioning it — a wrong golden\n\
             freezes the bug instead of catching it.",
            file.display()
        )
    })?;
    if current != expected {
        failures.push(format!(
            "{}: changed.\n--- recorded ---\n{expected}\n--- now ---\n{current}",
            file.display()
        ));
    }
    Ok(())
}

fn main() -> Result<()> {
    let usage = "usage: ronoctua-term-diff <corpus> <golden> <styled> [--record]";
    let args: Vec<String> = std::env::args().skip(1).collect();
    let record = args.iter().any(|a| a == "--record");
    let positional: Vec<&String> = args.iter().filter(|a| *a != "--record").collect();
    let [corpus, golden_dir, styled_dir] = positional.as_slice() else {
        bail!("{usage}");
    };
    let (corpus, golden_dir, styled_dir) = (
        Path::new(corpus),
        Path::new(golden_dir),
        Path::new(styled_dir),
    );

    let mut cases: Vec<_> = std::fs::read_dir(corpus)
        .with_context(|| format!("could not read {}", corpus.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "vt"))
        .collect();
    // A stable order: the failure report has to be the same between runs.
    cases.sort();
    if cases.is_empty() {
        bail!("no `.vt` case in {}", corpus.display());
    }

    let names: Vec<&'static str> = engines()?.iter().map(|e| e.name()).collect();
    println!("{} cases, engines: {}", cases.len(), names.join(", "));

    let mut failures = Vec::new();
    let mut recorded = Vec::new();
    for case in &cases {
        let name = case
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let bytes = std::fs::read(case)?;

        let results: Vec<(&'static str, Grid, Vec<u8>)> = engines()?
            .into_iter()
            .map(|mut m| {
                m.feed(&bytes);
                (m.name(), m.grid(), m.replies())
            })
            .collect();

        // Verdict 1: everyone against the first, on what they share.
        let (first, grid_ref, replies_ref) = &results[0];
        for (other, g, replies) in &results[1..] {
            let mut d = grid_ref.diff(g);
            d.extend(grid_ref.style_diff(g));
            let (a, b) = (comparable_replies(replies_ref), comparable_replies(replies));
            if a != b {
                let show = |r: &[Reply]| {
                    r.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                d.push(format!("replies: [{}] against [{}]", show(&a), show(&b)));
            }
            if !d.is_empty() {
                failures.push(format!(
                    "{name}: {first} x {other} diverge:\n  {}",
                    d.join("\n  ")
                ));
            }
        }

        // Verdict 2: the shared characters, and each engine's own golden.
        golden(
            &golden_dir.join(format!("{name}.txt")),
            &grid_ref.canonical_f(),
            record.then_some(&mut recorded),
            &mut failures,
        )?;
        for (engine, grid, replies) in &results {
            golden(
                &styled_dir.join(engine).join(format!("{name}.txt")),
                &own_golden(grid, replies),
                record.then_some(&mut recorded),
                &mut failures,
            )?;
        }

        // Verdict 3: the window's copy, at every cut.
        if let Some(d) = every_cut(&bytes)? {
            failures.push(format!("{name}: the mirror and the holder diverge, {d}"));
        }
    }

    // Recording does not excuse a divergence: a golden written over two
    // engines that disagree, or over a mirror that loses bytes, would freeze
    // the bug instead of catching it.
    if !failures.is_empty() {
        bail!(
            "{} difference(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
    if record {
        for (file, current) in &recorded {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(file, current)?;
        }
        println!(
            "OK: goldens written for {} cases into {} and {}",
            cases.len(),
            golden_dir.display(),
            styled_dir.display()
        );
        return Ok(());
    }
    println!(
        "OK: ZERO diff in {} cases across {} engines, every engine on its goldens, \
         and the mirror on the holder at every byte",
        cases.len(),
        names.len()
    );
    Ok(())
}
